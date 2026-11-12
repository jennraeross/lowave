# lowave

Cross-platform lofi livestream radio with an analog / Winamp-style UI.

## How it works

- Lists live streams from Lofi Girl (`/@LofiGirl/streams`) plus your own
  YouTube URLs via `yt-dlp --flat-playlist -J` (no API key needed).
- Audio path: `yt-dlp -g` resolves the stream → `ffmpeg` decodes HLS/DASH to
  raw `f32le` stereo 44.1k PCM → `rodio` plays it. ffmpeg reconnect flags plus
  URL re-resolving survive stalls and ~6h googlevideo URL expiry.
- Spectrum bars are decorative (fake analyser), per V1 scope.

## Requirements

`yt-dlp` and `ffmpeg` on `PATH` (or next to the binary):

```
brew install yt-dlp ffmpeg      # macOS
winget install yt-dlp ffmpeg    # Windows
```

## Run

```
cargo run
```

Config + volume persist to the OS config dir as `lowave/streams.toml`.

## Tests (need network)

```
cargo test
```
