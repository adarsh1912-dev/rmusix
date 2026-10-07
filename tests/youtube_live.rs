//! Live acceptance: one search + one resolve (no playback assert beyond URL non-empty).
//!
//! Requires `yt-dlp` + `mpv` binaries and network. Ignored by default:
//!
//! ```sh
//! RMUSIX_LIVE=1 cargo test --test youtube_live -- --ignored --nocapture
//! ```

use rmusix::config::AppConfig;
use rmusix::models::AudioQuality;
use rmusix::youtube::{YouTubeClient, YtDlpClient};

#[tokio::test]
#[ignore]
async fn live_acceptance() {
    if std::env::var("RMUSIX_LIVE").unwrap_or_default() != "1" {
        eprintln!("skipping: set RMUSIX_LIVE=1 to run");
        return;
    }
    let cfg = AppConfig::load().unwrap_or_default();
    for (bin, resolve) in [("yt-dlp", cfg.resolve_ytdlp()), ("mpv", cfg.resolve_mpv())] {
        if resolve.is_err() {
            eprintln!("skipping: {bin} binary not installed");
            return;
        }
    }
    let yt = YtDlpClient::new();
    let results = yt.search("never gonna give you up", 3).await.unwrap();
    assert!(!results.is_empty(), "search returned no results");
    let info = yt
        .resolve_audio_stream(&results[0].video_id, AudioQuality::Medium)
        .await
        .unwrap();
    assert!(
        !info.url.trim().is_empty(),
        "resolved URL must be non-empty"
    );
    eprintln!("live OK: {} -> {}", results[0].title, info.mime);
}
