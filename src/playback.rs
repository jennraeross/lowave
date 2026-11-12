use std::io::Read;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use crate::ytdlp;

/// Fixed output format we request from ffmpeg.
const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
/// 0.5s of stereo f32 audio per chunk.
const CHUNK_SAMPLES: usize = 44_100 / 2 * 2;
/// Give up re-resolving after this many consecutive failures.
const MAX_RESOLVE_RETRIES: u32 = 8;

#[derive(Debug, Clone)]
pub enum PlaybackStatus {
    Stopped,
    Buffering(String),
    Playing(String),
    Error(String),
}

/// Owns the rodio output stream + sink on the UI thread. Decoding (ffmpeg PCM
/// pipe) runs on a per-play background thread; a generation counter makes
/// stop/play race-free, and the thread re-resolves expired googlevideo URLs
/// itself so 24/7 lives survive the ~6h URL expiry.
pub struct PlaybackController {
    _stream: Option<rodio::OutputStream>,
    sink: Option<Arc<rodio::Sink>>,
    gen: Arc<AtomicU64>,
    status: Arc<Mutex<PlaybackStatus>>,
    volume: f32,
}

impl PlaybackController {
    pub fn new(initial_volume: f32) -> Self {
        match rodio::OutputStreamBuilder::from_default_device()
            .and_then(|b| b.open_stream_or_fallback())
        {
            Ok(stream) => {
                let sink = Arc::new(rodio::Sink::connect_new(stream.mixer()));
                sink.set_volume(initial_volume.clamp(0.0, 1.0));
                Self {
                    _stream: Some(stream),
                    sink: Some(sink),
                    gen: Arc::new(AtomicU64::new(0)),
                    status: Arc::new(Mutex::new(PlaybackStatus::Stopped)),
                    volume: initial_volume,
                }
            }
            Err(e) => Self {
                _stream: None,
                sink: None,
                gen: Arc::new(AtomicU64::new(0)),
                status: Arc::new(Mutex::new(PlaybackStatus::Error(format!(
                    "no audio device: {e}"
                )))),
                volume: initial_volume,
            },
        }
    }

    pub fn play(&self, label: String, media: ytdlp::MediaKind) {
        let Some(sink) = self.sink.clone() else {
            self.set_status(PlaybackStatus::Error("no audio device".into()));
            return;
        };
        self.gen.fetch_add(1, Ordering::SeqCst);
        sink.stop();
        self.set_status(PlaybackStatus::Buffering(label.clone()));
        let my_gen = self.gen.load(Ordering::SeqCst);
        let gen = self.gen.clone();
        let status = self.status.clone();
        std::thread::spawn(move || {
            stream_into_sink(&label, &media, &sink, &status, &gen, my_gen);
        });
    }

    pub fn stop(&self) {
        self.gen.fetch_add(1, Ordering::SeqCst);
        if let Some(sink) = &self.sink {
            sink.stop();
        }
        self.set_status(PlaybackStatus::Stopped);
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
    }

    pub fn status(&self) -> PlaybackStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or(PlaybackStatus::Stopped)
    }

    fn set_status(&self, s: PlaybackStatus) {
        if let Ok(mut g) = self.status.lock() {
            *g = s;
        }
    }
}

fn stream_into_sink(
    label: &str,
    media: &ytdlp::MediaKind,
    sink: &Arc<rodio::Sink>,
    status: &Arc<Mutex<PlaybackStatus>>,
    gen: &Arc<AtomicU64>,
    my_gen: u64,
) {
    let alive = || gen.load(Ordering::SeqCst) == my_gen;
    let set = |s: PlaybackStatus| {
        if let Ok(mut g) = status.lock() {
            *g = s;
        }
    };

    let mut failures = 0u32;
    let mut fast_fails = 0u32;
    // Reconnect loop: survives per-segment stalls and googlevideo URL expiry.
    // Direct media has no expiry; the loop just replays it after a stall.
    while alive() {
        let media_url = match media {
            ytdlp::MediaKind::Direct(d) => {
                failures = 0;
                d.clone()
            }
            ytdlp::MediaKind::YouTube(url) => match ytdlp::resolve_media_url(url) {
                Ok(u) => {
                    failures = 0;
                    u
                }
                Err(e) => {
                    failures += 1;
                    if failures > MAX_RESOLVE_RETRIES {
                        set(PlaybackStatus::Error(format!("resolve: {e:#}")));
                        return;
                    }
                    set(PlaybackStatus::Buffering(format!("retry resolve ({failures})")));
                    std::thread::sleep(std::time::Duration::from_secs(2 * failures as u64));
                    continue;
                }
            },
        };
        let mut child = match ytdlp::spawn_pcm_pipe(&media_url) {
            Ok(c) => c,
            Err(e) => {
                set(PlaybackStatus::Error(format!("ffmpeg: {e:#}")));
                return;
            }
        };
        let mut stdout = match child.stdout.take() {
            Some(o) => o,
            None => {
                set(PlaybackStatus::Error("no ffmpeg stdout".into()));
                return;
            }
        };

        set(PlaybackStatus::Buffering(label.to_string()));
        let session_start = std::time::Instant::now();
        let mut pcm = vec![0u8; CHUNK_SAMPLES * 4];
        let mut playing_reported = false;
        let mut chunks_appended = 0u32;
        let mut eof = false;
        while alive() && !eof {
            // Fill one chunk (short on EOF/stall).
            let mut filled = 0;
            while filled < pcm.len() {
                match stdout.read(&mut pcm[filled..]) {
                    Ok(0) => {
                        eof = true;
                        break;
                    }
                    Ok(n) => filled += n,
                    Err(_) => {
                        eof = true;
                        break;
                    }
                }
            }
            if filled == 0 {
                break;
            }
            let floats = pcm_bytes_to_f32(&pcm[..filled]);
            if floats.iter().all(|s| *s == 0.0) {
                // Skip pure-silence padding; keep the pipe draining.
                continue;
            }
            sink.append(rodio::buffer::SamplesBuffer::new(CHANNELS, SAMPLE_RATE, floats));
            sink.play();
            chunks_appended += 1;
            if !playing_reported {
                playing_reported = true;
                set(PlaybackStatus::Playing(label.to_string()));
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        if !alive() {
            return;
        }
        // ffmpeg exiting immediately with zero audio means the source is
        // unusable (missing/unsupported file, dead URL) — error out instead
        // of spinning in the reconnect loop forever.
        if chunks_appended == 0 && session_start.elapsed() < std::time::Duration::from_secs(3) {
            fast_fails += 1;
            if fast_fails >= 3 {
                set(PlaybackStatus::Error(format!(
                    "playback failed to start — unsupported or missing file? ({label})"
                )));
                return;
            }
        } else {
            fast_fails = 0;
        }
        // EOF = expired URL or dropped live; loop around and re-resolve.
        set(PlaybackStatus::Buffering(format!("reconnecting {label}")));
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// Convert raw little-endian f32 bytes from the ffmpeg pipe to samples.
pub(crate) fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live-network smoke test: resolve -> ffmpeg PCM -> read 3s of audio.
    /// Ignored by default: YouTube serves bot-checks to datacenter IPs, so
    /// this fails in CI. Run locally with
    /// `cargo test -- --include-ignored`.
    #[ignore]
    #[test]
    fn decodes_live_audio_pipe() {
        let url = "https://www.youtube.com/watch?v=rFZHOHl-L8A";
        let media = ytdlp::resolve_media_url(url).expect("resolve media url");
        assert!(media.starts_with("https://"));
        let mut child = ytdlp::spawn_pcm_pipe(&media).expect("spawn ffmpeg");
        let mut stdout = child.stdout.take().expect("stdout");
        // ~3s of stereo f32 @44.1k.
        let want = SAMPLE_RATE as usize * 2 * 3 * 4;
        let mut buf = vec![0u8; want];
        let mut filled = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while filled < want && std::time::Instant::now() < deadline {
            match stdout.read(&mut buf[filled..]) {
                Ok(0) => std::thread::sleep(std::time::Duration::from_millis(100)),
                Ok(n) => filled += n,
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(filled > want / 2, "only got {filled}/{want} bytes");
        let floats = pcm_bytes_to_f32(&buf[..filled]);
        let energy: f32 = floats.iter().map(|s| s * s).sum::<f32>() / floats.len() as f32;
        assert!(energy > 1e-8, "decoded silence (energy {energy})");
    }

    /// Hermetic version of the pipe test: ffmpeg synthesizes a 440Hz tone,
    /// no network involved. Validates the ffmpeg pipe + f32 conversion path
    /// that `stream_into_sink` uses.
    #[test]
    fn decodes_synthetic_pcm_pipe() {
        let bin = ytdlp::find_ffmpeg().expect("find ffmpeg");
        let mut child = std::process::Command::new(&bin)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=3",
                "-vn",
                "-f",
                "f32le",
                "-ac",
                "2",
                "-ar",
                "44100",
                "-",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn ffmpeg");
        let mut stdout = child.stdout.take().expect("stdout");
        // ~3s of stereo f32 @44.1k.
        let want = SAMPLE_RATE as usize * 2 * 3 * 4;
        let mut buf = vec![0u8; want];
        let mut filled = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while filled < want && std::time::Instant::now() < deadline {
            match stdout.read(&mut buf[filled..]) {
                Ok(0) => std::thread::sleep(std::time::Duration::from_millis(50)),
                Ok(n) => filled += n,
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(filled > want / 2, "only got {filled}/{want} bytes");
        let floats = pcm_bytes_to_f32(&buf[..filled]);
        let energy: f32 = floats.iter().map(|s| s * s).sum::<f32>() / floats.len() as f32;
        assert!(energy > 1e-8, "decoded silence (energy {energy})");
    }

    /// Regression test for the local-file stall: ffmpeg used to receive
    /// HTTP-only `-reconnect` flags for local paths and exit instantly,
    /// which the reconnect loop turned into an endless "buffering" state.
    /// Encodes a real opus file, then reads it back through `spawn_pcm_pipe`.
    #[test]
    fn decodes_local_opus_file_pipe() {
        let bin = ytdlp::find_ffmpeg().expect("find ffmpeg");
        let dir = std::env::temp_dir().join("lowave-test-local");
        std::fs::create_dir_all(&dir).unwrap();
        let opus = dir.join("tone.opus");
        let st = std::process::Command::new(&bin)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-c:a",
                "libopus",
                "-y",
            ])
            .arg(&opus)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("encode opus fixture");
        assert!(st.success(), "ffmpeg libopus encoder missing?");
        let mut child =
            ytdlp::spawn_pcm_pipe(&opus.to_string_lossy()).expect("spawn ffmpeg");
        let mut stdout = child.stdout.take().expect("stdout");
        // ~2s of stereo f32 @44.1k.
        let want = SAMPLE_RATE as usize * 2 * 2 * 4;
        let mut buf = vec![0u8; want];
        let mut filled = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while filled < want && std::time::Instant::now() < deadline {
            match stdout.read(&mut buf[filled..]) {
                Ok(0) => std::thread::sleep(std::time::Duration::from_millis(50)),
                Ok(n) => filled += n,
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(filled > want / 2, "only got {filled}/{want} bytes");
        let floats = pcm_bytes_to_f32(&buf[..filled]);
        let energy: f32 = floats.iter().map(|s| s * s).sum::<f32>() / floats.len() as f32;
        assert!(energy > 1e-8, "decoded silence (energy {energy})");
    }
}
