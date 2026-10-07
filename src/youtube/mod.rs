//! YouTube client stub.
//!
//! scrape-only, replaceable, no binaries.
//!
//! Implementations must resolve search + audio streams via Innertube scraping
//! (e.g. `rusty_ytdl`), without API keys and without spawning external
//! binaries (`yt-dlp` / `ffmpeg` / `mpv` are forbidden).

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
