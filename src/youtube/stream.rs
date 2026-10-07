//! yt-dlp format picking stub (subprocess backend).

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo};

/// Stub: real picking via yt-dlp JSON (Agent 1).
pub fn select_audio_stream(
    _formats_json: &serde_json::Value,
    _quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    Err(RmusixError::Config(
        "yt-dlp format picking not yet implemented".to_string(),
    ))
}
