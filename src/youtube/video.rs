//! yt-dlp video resolution stub (subprocess backend).

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo};

/// Reject blank ids; return the trimmed id otherwise.
pub fn validate_video_id(video_id: &str) -> Result<String, RmusixError> {
    let trimmed = video_id.trim();
    if trimmed.is_empty() {
        return Err(RmusixError::InvalidInput(
            "video id must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Stub: real resolution via yt-dlp (Agent 1).
pub async fn fetch_audio_info(
    _ytdlp_path: &str,
    video_id: &str,
    _quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    let _ = validate_video_id(video_id)?;
    Err(RmusixError::Config(
        "yt-dlp resolve not yet implemented".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_blank_ids() {
        for id in ["", "   ", "\n "] {
            assert!(matches!(
                validate_video_id(id),
                Err(RmusixError::InvalidInput(_))
            ));
        }
        assert_eq!(
            validate_video_id("  dQw4w9WgXcQ  ").expect("valid"),
            "dQw4w9WgXcQ"
        );
    }
}
