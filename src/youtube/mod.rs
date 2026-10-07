//! YouTube client (scrape-only, replaceable, no binaries).
//!
//! Implementations must resolve search + audio streams via Innertube scraping
//! (e.g. `rusty_ytdl`), without API keys and without spawning external
//! binaries (`yt-dlp` / `ffmpeg` / `mpv` are forbidden).

pub mod search;
pub mod stream;
pub mod video;

use std::time::Duration;

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo, Song};

/// Scrape-only YouTube source. Replaceable for tests/mocks.
pub trait YouTubeClient: Send + Sync {
    /// Search YouTube for `query`, returning up to `limit` songs.
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Song>, RmusixError>;

    /// Resolve a playable audio stream for `video_id` at the requested `quality`.
    async fn resolve_audio_stream(
        &self,
        video_id: &str,
        quality: AudioQuality,
    ) -> Result<AudioStreamInfo, RmusixError>;
}

/// Single request timeout reused for all YouTube scraping.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Pure-Rust Innertube scraper backed by `rusty_ytdl`.
///
/// Reuses one [`reqwest::Client`] (15s timeout) for both search and video
/// resolution. No API key, no files, no subprocesses.
#[derive(Debug, Clone)]
pub struct RustyYtdlClient {
    http: reqwest::Client,
}

impl RustyYtdlClient {
    /// Build a client with a 15s-timeout [`reqwest::Client`].
    pub fn new() -> Self {
        Self::try_new().unwrap_or_else(|_| Self {
            http: reqwest::Client::new(),
        })
    }

    /// Fallible constructor; fails only if the HTTP backend cannot initialise.
    pub fn try_new() -> Result<Self, RmusixError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| RmusixError::Network(format!("failed to build http client: {e}")))?;
        Ok(Self { http })
    }

    /// Build a client around an existing [`reqwest::Client`].
    ///
    /// The supplied client is cloned (shared connection pool) into the
    /// Innertube backends on each request, so a single pool is reused.
    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Shared [`reqwest::Client`] (primarily for tests).
    pub fn http_client(&self) -> &reqwest::Client {
        &self.http
    }
}

impl Default for RustyYtdlClient {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeClient for RustyYtdlClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Song>, RmusixError> {
        let normalized = search::validate_query(query)?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let backend = search::backend_with_client(&self.http)?;
        let fut = search::search_with_api(&backend, &normalized, limit);
        match tokio::time::timeout(REQUEST_TIMEOUT, fut).await {
            Ok(res) => res,
            Err(_) => Err(RmusixError::Network("search timed out".to_string())),
        }
    }

    async fn resolve_audio_stream(
        &self,
        video_id: &str,
        quality: AudioQuality,
    ) -> Result<AudioStreamInfo, RmusixError> {
        let normalized = video::validate_video_id(video_id)?;
        let fut = video::fetch_audio_info(&self.http, &normalized, quality);
        match tokio::time::timeout(REQUEST_TIMEOUT, fut).await {
            Ok(res) => res,
            Err(_) => Err(RmusixError::Network(
                "stream resolution timed out".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_query_is_invalid_input_without_network() {
        let client = RustyYtdlClient::new();
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
        let client = RustyYtdlClient::new();
        let out = client
            .search("some song", 0)
            .await
            .expect("zero limit must succeed without network");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn empty_video_id_is_invalid_input() {
        let client = RustyYtdlClient::new();
        let res = client
            .resolve_audio_stream("   ", AudioQuality::Medium)
            .await;
        assert!(matches!(res, Err(RmusixError::InvalidInput(_))));
    }
}
