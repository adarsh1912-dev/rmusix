//! Audio engine (rodio + symphonia, scrape-only, no subprocesses).

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

pub mod controls;
pub mod player;

pub use player::AudioEngine;
