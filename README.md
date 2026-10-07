# rmusix

Terminal YouTube music player in Rust. Search YouTube, stream audio-only, control playback from a keyboard-driven TUI.

![screenshot placeholder — TUI with search results, queue, and player bar](docs/screenshot.png)

## Features

- YouTube search (scrape-only, no API key)
- Audio-only streaming (no downloads, in-memory buffer)
- Play queue: enqueue, skip next/prev, reorder
- Playback controls: play/pause, ±5s seek, volume, mute
- Search-while-playing; `q`/Ctrl-C always restores the terminal

## Requirements

- Rust stable toolchain (`cargo`)
- ALSA dev libraries on Linux (e.g. `libasound2-dev` / `alsa-lib-devel`)
- No YouTube API key needed
- No extra binaries needed: no yt-dlp, ffmpeg, or mpv (pure-Rust scraping + rodio/symphonia)

## Install

```sh
git clone <repo-url> rmusix
cargo run -- "never gonna give you up"
```

Or with no initial query:

```sh
cargo run
```

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

- `src/youtube/` — scrape-only Innertube client (`rusty_ytdl`): search + audio-stream resolution, shared 15s-timeout `reqwest` client. No API key, no subprocesses.
- `src/audio/` — rodio engine on a dedicated thread; async command/event channels; in-memory fetch + symphonia decode; headless fallback when no audio device exists.
- `src/queue/` — in-memory `QueueManager` (push, next/prev, remove, move, play_at).
- `src/app/` — `AppState` + `App::handle` pure transitions + tokio run loop (crossterm `EventStream`, mpsc, 250ms ticker).
- `src/ui/` — pure ratatui rendering (`render(frame, state)`) + `key_to_action` mapping.
- `src/config/` — TOML config at `$CONFIG_DIR/rmusix/config.toml` (volume, search_limit, quality).

## Dev

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Live acceptance (one manual run, needs network + audio):

```sh
RMUSIX_LIVE=1 cargo test --test youtube_live -- --ignored --nocapture
```

Manual script: Search → Play → Pause/Seek/Vol → Search-while-playing → Queue skip → `q` restores terminal.

## Troubleshooting

- **HTTP 429 / "Search rate-limited"**: YouTube is throttling scraping. Wait a few minutes and retry; reduce repeated searches.
- **No audio device**: the engine keeps running headless (decode still verified, no playback). On Linux install ALSA libs and check `aplay -l`.
- **Search empty / stream errors**: check network access to youtube.com / googlevideo.com; blank queries and ids are rejected as invalid input.

## Limitations + Legal

- Subject to YouTube's Terms of Service; scraping may break or be rate-limited at any time.
- Stream-only: no downloads, no permanent files (in-memory buffer only).
- Audio quality depends on what YouTube serves for a given video; seeking depends on decoder support (fallback keeps playing).
