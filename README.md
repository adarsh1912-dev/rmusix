# rmusix

Terminal YouTube music player in Rust. Search YouTube, stream audio-only, control playback from a keyboard-driven TUI.

![screenshot placeholder — TUI with search results, queue, and player bar](docs/screenshot.png)

## Features

- YouTube search via `yt-dlp` subprocess (`ytsearch`, `--dump-json`, no API key)
- Audio-only streaming: `yt-dlp` resolves the stream URL, `mpv` plays it (no downloads, no permanent files)
- Play queue: enqueue, skip next/prev, reorder
- Playback controls: play/pause, ±5s seek, volume, mute
- Search-while-playing; `q`/Ctrl-C always restores the terminal

## Requirements

- Rust stable toolchain (`cargo`)
- Runtime binaries (both required at startup):
  - `mpv`
  - `yt-dlp`

  ```sh
  pacman -S mpv yt-dlp
  ```

  Debian/Ubuntu: `sudo apt install mpv yt-dlp`. macOS: `brew install mpv yt-dlp`.
- No YouTube API key needed.

## Install

Single build command; `mpv` + `yt-dlp` must be on `PATH` (or pointed at via config/env below) at **runtime**, not just at build time:

```sh
git clone <repo-url> rmusix
cargo build --release
./target/release/rmusix "blinding lights"
```

Or run directly:

```sh
cargo run --release -- "blinding lights"
```

Or with no initial query:

```sh
cargo run --release
```

Binary paths can be overridden in `$CONFIG_DIR/rmusix/config.toml` (`ytdlp_path`, `mpv_path`) or via `RMUSIX_YTDLP` / `RMUSIX_MPV` env vars (env wins). Missing binaries exit with code 2 and an install hint.

## Keys

| Key | Action |
|-----|--------|
| `Space` | Play / pause |
| `n` / `p` | Next / previous in queue |
| `←` / `→` | Seek ∓5s |
| `+` / `-` (`=` / `_` also work) | Volume up / down |
| `m` | Mute toggle |
| `q` | Quit (restores terminal) |
| `/` | Enter search mode |
| `a` | Enqueue selected result |
| `Enter` | Play selected result |
| `Esc` | Back / exit search mode |
| `Tab`, `1` / `2` / `3` | Cycle / switch Search, Queue, Help screens |
| Ctrl-C | Quit (restores terminal) |

## CLI

```sh
rmusix "query"
```

A single optional positional argument seeds the initial search. Omitting it starts with an empty search box.

## Architecture

```text
                    ┌─────────────────────────┐
  user query ──────▶│ src/youtube (yt-dlp     │
                    │ subprocess: ytsearch -- │
                    │ dump-json → Song list)  │
                    └────────────┬────────────┘
                                 │ video id + quality
                                 ▼
                    ┌─────────────────────────┐
                    │ yt-dlp -J → formats[]   │
                    │ pick audio-only by abr  │
                    │ (Low=min/Med/High=max), │
                    │ fallback bestaudio -g   │
                    └────────────┬────────────┘
                                 │ stream URL
                                 ▼
                    ┌─────────────────────────┐
  AudioCommand ────▶│ src/audio (MpvEngine):  │
  Play/Pause/Seek/  │ spawn `mpv --idle --    │
  Volume/Mute/Stop  │ input-ipc-server=<sock> │
                    │ JSON IPC over Unix      │
                    │ socket (loadfile, set_  │
  AudioEvent ◀──────│ property, seek, observe │
  Position/Duration/│ time-pos/duration/eof)  │
  Finished/Error    └─────────────────────────┘
```

- `src/youtube/` — `YtDlpClient`: search + audio-stream resolution via `yt-dlp` subprocess JSON (15s timeout). No API key. Process spawning (`Command`) is allowed **only** in `src/youtube` + `src/audio`.
- `src/audio/` — `MpvEngine` over Unix-socket IPC on a tokio task; async command/event channels; volume/mute memory; missing-binary errors stay alive as `AudioEvent::Error`.
- `src/queue/` — in-memory `QueueManager` (push, next/prev, remove, move, play_at).
- `src/app/` — `AppState` + `App::handle` pure transitions + tokio run loop (crossterm `EventStream`, mpsc, 250ms ticker); owns startup wiring (config load, binary checks, socket path).
- `src/ui/` — pure ratatui rendering (`render(frame, state)`) + `key_to_action` mapping.
- `src/config/` — TOML config at `$CONFIG_DIR/rmusix/config.toml` (volume, search_limit, quality, ytdlp_path, mpv_path).

## Dev

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Live acceptance (one manual run, needs `yt-dlp` + `mpv` + network; asserts one search + one resolve, URL non-empty, no playback assert):

```sh
RMUSIX_LIVE=1 cargo test --test youtube_live -- --ignored --nocapture
```

Manual script: Search → Play → Pause/Seek/Vol → Search-while-playing → Queue skip → `q` restores terminal.

## Troubleshooting

- **Binary missing (`yt-dlp not found` / `mpv not found`, exit 2)**: install both (`pacman -S mpv yt-dlp`) or point config/env at them (`ytdlp_path`/`mpv_path`, `RMUSIX_YTDLP`/`RMUSIX_MPV`).
- **`yt-dlp` version drift (search/resolve suddenly fails)**: YouTube changes break old `yt-dlp` builds — upgrade it (`yt-dlp -U`, or `pacman -Syu yt-dlp`) and retry.
- **Socket permission / `mpv socket not ready`**: the IPC socket lives at `$XDG_RUNTIME_DIR/rmusix-*.sock` (else `/tmp/rmusix-$UID`); stale sockets are removed on startup/shutdown. If mpv can't bind there, check dir permissions and that no stale `mpv --input-ipc-server=<sock>` instance holds it.
- **HLS seek limits**: live/LL-HLS streams may not support ±5s relative seek — playback continues; seeking snaps to the nearest seekable point or is ignored by mpv.
- **HTTP 429 / "Search rate-limited"**: YouTube is throttling. Wait a few minutes and retry; reduce repeated searches.
- **No audio device**: mpv still runs and decodes; check system volume / device (`aplay -l` on Linux).

## Limitations + Legal

- Subject to YouTube's Terms of Service; scraping/subprocess resolution may break or be rate-limited at any time.
- Stream-only: no downloads, no permanent files (in-memory URL handoff to mpv only).
- Audio quality depends on what YouTube serves for a given video; seeking depends on stream/decoder support (fallback keeps playing).
