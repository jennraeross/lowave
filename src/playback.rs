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

    pub fn play(&self, url: String) {
        let Some(sink) = self.sink.clone() else {
            self.set_status(PlaybackStatus::Error("no audio device".into()));
            return;
        };
        self.gen.fetch_add(1, Ordering::SeqCst);
        sink.stop();
        self.set_status(PlaybackStatus::Buffering(url.clone()));
        let my_gen = self.gen.load(Ordering::SeqCst);
        let gen = self.gen.clone();
        let status = self.status.clone();
        std::thread::spawn(move || {
            stream_into_sink(&url, &sink, &status, &gen, my_gen);
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
    url: &str,
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
    // Reconnect loop: survives per-segment stalls and googlevideo URL expiry.
    while alive() {
        let media_url = match ytdlp::resolve_media_url(url) {
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

        set(PlaybackStatus::Buffering(url.to_string()));
        let mut pcm = vec![0u8; CHUNK_SAMPLES * 4];
        let mut playing_reported = false;
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
            let floats: Vec<f32> = pcm[..filled]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            if floats.iter().all(|s| *s == 0.0) {
                // Skip pure-silence padding; keep the pipe draining.
                continue;
            }
            sink.append(rodio::buffer::SamplesBuffer::new(CHANNELS, SAMPLE_RATE, floats));
            sink.play();
            if !playing_reported {
                playing_reported = true;
                set(PlaybackStatus::Playing(url.to_string()));
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        if !alive() {
            return;
        }
        // EOF = expired URL or dropped live; loop around and re-resolve.
        set(PlaybackStatus::Buffering(format!("reconnecting {url}")));
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headless smoke test: resolve -> ffmpeg PCM -> read 3s of audio.
    /// Requires network + yt-dlp + ffmpeg.
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
        let floats: Vec<f32> = buf[..filled]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let energy: f32 = floats.iter().map(|s| s * s).sum::<f32>() / floats.len() as f32;
        assert!(energy > 1e-8, "decoded silence (energy {energy})");
    }
}
