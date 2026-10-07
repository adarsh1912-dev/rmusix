//! Audio stub.
//!
//! scrape-only, replaceable, no binaries.
//!
//! Playback must be pure-Rust (`rodio` + `symphonia`), fed by URLs resolved
//! via the scrape-only YouTube client. No `ffmpeg` / `mpv` subprocesses.

use std::path::PathBuf;

/// Commands sent to the audio task.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Play a remote (scraped) stream URL.
    PlayStream(String),
    /// Play a local file (cache path).
    PlayFile(PathBuf),
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
}

/// Events emitted by the audio task.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    Loading,
    Playing,
    Paused,
    Buffering,
    Finished,
    Error(String),
}
