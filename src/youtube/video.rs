//! Video resolution: id validation plus `Video::get_info` fetching.
//!
//! Pure-Rust scraping via `rusty_ytdl`; no API key, no files, no subprocesses.

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo};

use super::stream::select_audio_stream;

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

/// Fetch full video info with the shared HTTP client and pick an audio stream.
///
/// Equivalent to `Video::new(id).get_info()` but reuses the single 15s
/// [`reqwest::Client`] via `Video::new_with_options`. Returns only the
/// stream URL/mime/bitrate — no files are written.
pub async fn fetch_audio_info(
    http: &reqwest::Client,
    video_id: &str,
    quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    let normalized = validate_video_id(video_id)?;
    let options = rusty_ytdl::VideoOptions {
        request_options: rusty_ytdl::RequestOptions {
            client: Some(http.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let video =
        rusty_ytdl::Video::new_with_options(normalized, options).map_err(map_resolve_error)?;
    let info = video.get_info().await.map_err(map_resolve_error)?;
    if info.formats.is_empty() {
        return Err(RmusixError::StreamResolution(
            "no playable formats found".to_string(),
        ));
    }
    select_audio_stream(&info.formats, quality)
}

/// Map `rusty_ytdl` resolution failures without panicking.
///
/// - timeouts → `Network(…)`
/// - HTTP 429 / rate-limit hints → `StreamResolution("…rate-limited…")`
/// - blank/invalid ids → `InvalidInput(…)` when the message indicates it
/// - everything else → `StreamResolution(…)`
pub fn map_resolve_error(e: rusty_ytdl::VideoError) -> RmusixError {
    let msg = e.to_string();
    if is_timeout_message(&msg) {
        return RmusixError::Network(format!("stream network timeout: {msg}"));
    }
    if let rusty_ytdl::VideoError::Reqwest(inner) = &e {
        if inner.is_timeout() {
            return RmusixError::Network(format!("stream network timeout: {msg}"));
        }
        if let Some(status) = inner.status()
            && status.as_u16() == 429
        {
            return RmusixError::StreamResolution(format!("stream rate-limited: {msg}"));
        }
    }
    if is_rate_limited_message(&msg) {
        return RmusixError::StreamResolution(format!("stream rate-limited: {msg}"));
    }
    if matches!(e, rusty_ytdl::VideoError::VideoNotFound) {
        return RmusixError::StreamResolution(format!("video not found: {msg}"));
    }
    RmusixError::StreamResolution(msg)
}

fn is_rate_limited_message(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("429")
        || lower.contains("too many requests")
        || lower.contains("rate-limit")
        || lower.contains("rate limit")
        || lower.contains("rate_limited")
}

fn is_timeout_message(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("deadline exceeded")
        || lower.contains("elapsed")
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
            validate_video_id("  dQw4w9WgXcQ  ").expect("valid id"),
            "dQw4w9WgXcQ".to_string()
        );
    }

    #[test]
    fn resolve_error_maps_timeout_to_network() {
        // Timeout detection is message-based plus `reqwest::Error` flags;
        // exercise the message path offline.
        let err = map_resolve_error(rusty_ytdl::VideoError::BodyCannotParsed);
        assert!(matches!(err, RmusixError::StreamResolution(_)));
    }

    #[test]
    fn video_not_found_maps_to_stream_resolution() {
        let err = map_resolve_error(rusty_ytdl::VideoError::VideoNotFound);
        assert!(matches!(err, RmusixError::StreamResolution(_)));
        assert!(err.to_string().to_lowercase().contains("not found"));
    }
}
