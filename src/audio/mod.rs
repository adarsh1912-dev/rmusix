//! Audio stub.
//!
//! scrape-only, replaceable, no binaries.
//!
//! Playback must be pure-Rust (`rodio` + `symphonia`), fed by URLs resolved
//! via the scrape-only YouTube client. No `ffmpeg` / `mpv` subprocesses.

use crate::models::AudioStreamInfo;
use std::time::Duration;

/// Commands sent to the audio task.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Play a resolved (scraped) stream.
    Play(AudioStreamInfo),
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    SeekForward(Duration),
    SeekBackward(Duration),
    ToggleMute,
}

/// Events emitted by the audio task.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    Position(Duration),
    Duration(Option<Duration>),
    Finished,
    Buffering(bool),
    Error(String),
}
