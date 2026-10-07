//! yt-dlp/mpv subprocess backend — Command allowed ONLY here.
//!
//! YouTube client via yt-dlp subprocess JSON. No API keys.

pub mod search;
pub mod stream;
pub mod video;

use std::time::Duration;

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo, Song};

#[allow(async_fn_in_trait)]
pub trait YouTubeClient: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Song>, RmusixError>;
    async fn resolve_audio_stream(
        &self,
        video_id: &str,
        quality: AudioQuality,
    ) -> Result<AudioStreamInfo, RmusixError>;
}

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct YtDlpClient {
    pub ytdlp_path: String,
}

impl YtDlpClient {
    pub fn new() -> Self {
        Self {
            ytdlp_path: "yt-dlp".to_string(),
        }
    }

    pub fn with_path(path: impl Into<String>) -> Self {
        Self {
            ytdlp_path: path.into(),
        }
    }
}

impl Default for YtDlpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Back-compat alias for the pre-subprocess client name.
pub type RustyYtdlClient = YtDlpClient;

impl YouTubeClient for YtDlpClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Song>, RmusixError> {
        let normalized = search::validate_query(query)?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let _ = normalized;
        Err(RmusixError::Config(
            "yt-dlp search not yet implemented".to_string(),
        ))
    }

    async fn resolve_audio_stream(
        &self,
        video_id: &str,
        quality: AudioQuality,
    ) -> Result<AudioStreamInfo, RmusixError> {
        let _ = video::validate_video_id(video_id)?;
        let _ = quality;
        Err(RmusixError::Config(
            "yt-dlp resolve not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_query_is_invalid_input_without_network() {
        let client = YtDlpClient::new();
        for q in ["", "   ", "\n\t "] {
            let err = client
                .search(q, 5)
                .await
                .expect_err("empty query must fail");
            assert!(
                matches!(err, RmusixError::InvalidInput(_)),
                "expected InvalidInput, got {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn zero_limit_returns_empty_without_network() {
        let client = YtDlpClient::new();
        let out = client
            .search("some song", 0)
            .await
            .expect("zero limit must succeed without network");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn empty_video_id_is_invalid_input() {
        let client = YtDlpClient::new();
        let res = client
            .resolve_audio_stream("   ", AudioQuality::Medium)
            .await;
        assert!(matches!(res, Err(RmusixError::InvalidInput(_))));
    }
}
