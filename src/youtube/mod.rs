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
    pub bin: String,
    pub timeout: Duration,
}

impl YtDlpClient {
    pub fn new() -> Self {
        Self {
            bin: "yt-dlp".to_string(),
            timeout: REQUEST_TIMEOUT,
        }
    }

    pub fn with_bin(bin: impl Into<String>) -> Self {
        Self {
            bin: bin.into(),
            timeout: REQUEST_TIMEOUT,
        }
    }

    /// Back-compat constructor name.
    pub fn with_path(path: impl Into<String>) -> Self {
        Self::with_bin(path)
    }

    /// Back-compat field accessor.
    pub fn ytdlp_path(&self) -> &str {
        &self.bin
    }
}

impl Default for YtDlpClient {
    fn default() -> Self {
        let bin = crate::config::settings::AppConfig::load()
            .map(|c| c.ytdlp_path)
            .unwrap_or_else(|_| "yt-dlp".to_string());
        Self {
            bin,
            timeout: REQUEST_TIMEOUT,
        }
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
        ensure_binary(&self.bin)?;
        let spec = format!("ytsearch{limit}:{normalized}");
        let mut cmd = tokio::process::Command::new(&self.bin);
        cmd.args([
            &spec,
            "--dump-json",
            "--no-playlist",
            "--socket-timeout",
            "15",
        ]);
        let output = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| RmusixError::Network("yt-dlp timed out".to_string()))?
            .map_err(|e| map_spawn_error(e))?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Ok(search::parse_search_lines(&stdout, limit));
        }
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(search::map_search_error(output.status.code(), &stderr))
    }

    async fn resolve_audio_stream(
        &self,
        video_id: &str,
        quality: AudioQuality,
    ) -> Result<AudioStreamInfo, RmusixError> {
        video::fetch_audio_info_with_timeout(&self.bin, video_id, quality, self.timeout).await
    }
}

fn ensure_binary(bin: &str) -> Result<(), RmusixError> {
    if bin.contains('/') {
        if std::path::Path::new(bin).exists() {
            return Ok(());
        }
    } else if which::which(bin).is_ok() {
        return Ok(());
    }
    Err(RmusixError::Config(
        "yt-dlp not found (pacman -S yt-dlp)".to_string(),
    ))
}

fn map_spawn_error(e: std::io::Error) -> RmusixError {
    if e.kind() == std::io::ErrorKind::NotFound {
        RmusixError::Config("yt-dlp not found (pacman -S yt-dlp)".to_string())
    } else {
        RmusixError::Network(e.to_string())
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

    #[tokio::test]
    async fn missing_binary_is_config_error() {
        let client = YtDlpClient::with_bin("/nonexistent/yt-dlp");
        let err = client
            .search("hello", 5)
            .await
            .expect_err("missing bin must fail");
        assert!(
            matches!(err, RmusixError::Config(_)),
            "expected Config, got {err:?}"
        );
        assert!(err.to_string().contains("yt-dlp not found"));
    }
}
