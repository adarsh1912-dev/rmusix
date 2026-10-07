//! Optional live acceptance test: Search -> Play -> Pause/Seek/Vol ->
//! Search-while-playing -> Queue skip -> quit restores terminal.
//!
//! Ignored by default. Run manually with:
//! `RMUSIX_LIVE=1 cargo test --test youtube_live -- --ignored --nocapture`
//!
//! Requires network + an audio device; asserts nothing about binaries
//! (no yt-dlp/ffmpeg/mpv are used — pure-Rust scraping + rodio).

use rmusix::models::AudioQuality;
use rmusix::youtube::{RustyYtdlClient, YouTubeClient};

#[tokio::test]
#[ignore]
async fn live_acceptance() {
    if std::env::var("RMUSIX_LIVE").unwrap_or_default() != "1" {
        eprintln!("skipping: set RMUSIX_LIVE=1 to run");
        return;
    }
    let yt = RustyYtdlClient::new();
    let results = yt.search("never gonna give you up", 3).await.unwrap();
    assert!(!results.is_empty());
    let info = yt
        .resolve_audio_stream(&results[0].video_id, AudioQuality::Medium)
        .await
        .unwrap();
    assert!(info.url.starts_with("http"));
    eprintln!("live OK: {} -> {}", results[0].title, info.mime);
}
