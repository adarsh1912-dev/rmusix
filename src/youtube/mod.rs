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

pub const SEARCH_TIMEOUT: Duration = Duration::from_secs(60);
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(45);
/// Back-compat alias (was the single 15s timeout).
pub const REQUEST_TIMEOUT: Duration = RESOLVE_TIMEOUT;

/// Flags added to every yt-dlp call. `--socket-timeout 10` must stay
/// below both outer timeouts so the child fails fast on stalled sockets.
pub const COMMON_FLAGS: &[&str] = &[
    "--no-update",
    "--no-warnings",
    "--no-progress",
    "--retries",
    "1",
    "--socket-timeout",
    "10",
];

#[derive(Debug, Clone)]
pub struct YtDlpClient {
    pub bin: String,
    pub timeout: Duration,
    pub search_timeout: Duration,
}

impl YtDlpClient {
    pub fn new() -> Self {
        Self {
            bin: "yt-dlp".to_string(),
            timeout: RESOLVE_TIMEOUT,
            search_timeout: SEARCH_TIMEOUT,
        }
    }

    pub fn with_bin(bin: impl Into<String>) -> Self {
        Self {
            bin: bin.into(),
            timeout: RESOLVE_TIMEOUT,
            search_timeout: SEARCH_TIMEOUT,
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
            timeout: RESOLVE_TIMEOUT,
            search_timeout: SEARCH_TIMEOUT,
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
        // Flat playlist = single list request (~2s for 10). Full --dump-json
        // fetches every video's formats (~25s). Full metadata arrives on
        // Enter via resolve (-J) anyway, so list speed wins here.
        let mut args: Vec<&str> = vec![
            &spec,
            "--flat-playlist",
            "--dump-single-json",
            "--no-playlist",
        ];
        args.extend_from_slice(COMMON_FLAGS);
        let output = run_ytdlp(&self.bin, &args, self.search_timeout, "search").await?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Ok(search::parse_flat_single_json(&stdout, limit));
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

/// Spawn yt-dlp with `kill_on_drop(true)` so a timed-out call never leaves
/// an orphan process. `op` is "search" or "resolve" for error messages.
pub(crate) async fn run_ytdlp(
    bin: &str,
    args: &[&str],
    timeout: Duration,
    op: &str,
) -> Result<std::process::Output, RmusixError> {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(args);
    // kill_on_drop ensures a timed-out call never leaves an orphan yt-dlp:
    // dropping the `output()` future on timeout kills the child.
    cmd.kill_on_drop(true);
    match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(map_spawn_error(e)),
        Err(_) => Err(timeout_error(op, timeout)),
    }
}

pub(crate) fn timeout_error(op: &str, timeout: Duration) -> RmusixError {
    let secs = timeout.as_secs();
    match op {
        "search" => RmusixError::Network(format!(
            "yt-dlp search timed out after {secs}s (try shorter query / check network)"
        )),
        _ => RmusixError::Network(format!(
            "yt-dlp resolve timed out after {secs}s (try again / check network)"
        )),
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

    #[tokio::test]
    async fn timeout_kills_child_quickly() {
        let start = std::time::Instant::now();
        let err = run_ytdlp("sleep", &["30"], Duration::from_millis(100), "search")
            .await
            .expect_err("sleep must time out");
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "timed-out spawn must return fast, took {elapsed:?}"
        );
        assert!(err.to_string().contains("search"));
        assert!(err.to_string().contains("timed out"));
        // No orphan: the timed-out `sleep 30` must be gone.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut probe = tokio::process::Command::new("pgrep");
        probe.args(["-f", "sleep 30"]);
        let out = probe.output().await.expect("pgrep must run");
        assert!(
            !out.status.success(),
            "timed-out child must leave no `sleep 30` process"
        );
    }

    #[test]
    fn timeout_errors_name_op_and_secs() {
        let search = timeout_error("search", SEARCH_TIMEOUT).to_string();
        assert!(search.contains("search"), "got {search}");
        assert!(search.contains("60s"), "got {search}");
        let resolve = timeout_error("resolve", RESOLVE_TIMEOUT).to_string();
        assert!(resolve.contains("resolve"), "got {resolve}");
        assert!(resolve.contains("45s"), "got {resolve}");
    }

    #[test]
    fn socket_timeout_stays_below_outer() {
        let socket: u64 = COMMON_FLAGS
            .windows(2)
            .find(|w| w[0] == "--socket-timeout")
            .and_then(|w| w[1].parse().ok())
            .expect("common flags must include --socket-timeout");
        assert!(Duration::from_secs(socket) < SEARCH_TIMEOUT);
        assert!(Duration::from_secs(socket) < RESOLVE_TIMEOUT);
        for flag in ["--no-update", "--no-warnings", "--no-progress", "--retries"] {
            assert!(COMMON_FLAGS.contains(&flag), "missing {flag}");
        }
    }
}
