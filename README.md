# lowave

Cross-platform lofi livestream radio with a record-player UI. Lists live
streams from Lofi Girl (`/@LofiGirl/streams`) plus your own YouTube URLs —
no API key needed. Tested on macOS; Windows and Linux build in CI.

## How it works

- Stream listing via `yt-dlp --flat-playlist -J`.
- Audio path: `yt-dlp -g` resolves the stream → `ffmpeg` decodes HLS/DASH to
  raw `f32le` stereo 44.1k PCM → `rodio` plays it. ffmpeg reconnect flags plus
  URL re-resolving survive stalls and ~6h googlevideo URL expiry.
- UI: walnut plinth, spinning vinyl with the video thumbnail as the label,
  auto-drop tonearm, chunky hardware play button + volume fader.
- The spectrum is decorative; the disc spins at a calm fixed rate.

## Requirements

`yt-dlp` and `ffmpeg` must be installed (on `PATH`, or placed next to the
`lowave` binary):

| OS      | Install                                          |
| ------- | ------------------------------------------------ |
| macOS   | `brew install yt-dlp ffmpeg`                     |
| Windows | `winget install yt-dlp.yt-dlp Gyan.FFmpeg` (or `choco install yt-dlp ffmpeg -y`) |
| Linux   | `sudo apt install ffmpeg` + `pip install --user yt-dlp` (Debian/Ubuntu) |

Linux also needs GUI/audio build dependencies before `cargo build` works:

```
sudo apt install build-essential pkg-config curl \
  libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libssl-dev
```

## Run

```
cargo run --release
```

Config + volume + last-played stream persist to the OS config dir as
`lowave/streams.toml` (`%APPDATA%` on Windows, `~/.config` on Linux);
downloaded thumbnails are cached under `lowave/art/`.

Note: release builds on Windows hide the console window; `cargo run`
(debug) keeps it for log output.

## Tests (need network + the two binaries above)

```
cargo test
```

## License

MIT — see `LICENSE-MIT`. Dependency licenses: overwhelmingly MIT/Apache-2.0;
`symphonia`/`option-ext` are MPL-2.0 (file-level copyleft, your code stays
MIT) and `webpki-roots` cert data is CDLA-Permissive-2.0. `yt-dlp` and
`ffmpeg` run as separate processes and are not linked into the binary.
