//! Offline yt-dlp fixture tests: no binaries, no network.
//! Covers search-line parsing + resolve ranking via `select_audio_stream`.

use rmusix::models::AudioQuality;
use rmusix::youtube::YouTubeClient;
use rmusix::youtube::search::{map_search_error, parse_search_lines};
use rmusix::youtube::stream::select_audio_stream;

const LINE_A: &str = r#"{"id":"dQw4w9WgXcQ","title":"Never Gonna Give You Up","uploader":"Rick Astley","duration":212,"thumbnail":"http://img/a.jpg"}"#;
const LINE_B: &str = r#"{"id":"oHg5SJYRHA0","title":"Crazy Frog","channel":"Frog","duration":178}"#;

#[test]
fn parses_search_lines_skips_garbage_and_truncates() {
    let stdout = format!("{LINE_A}\nnot json\n\n{LINE_B}\n");
    let all = parse_search_lines(&stdout, 10);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].video_id, "dQw4w9WgXcQ");
    assert_eq!(all[0].title, "Never Gonna Give You Up");
    assert_eq!(all[1].video_id, "oHg5SJYRHA0");
    assert_eq!(parse_search_lines(&stdout, 1).len(), 1);
    assert!(parse_search_lines("", 10).is_empty());
}

fn formats_fixture() -> serde_json::Value {
    serde_json::json!({"formats": [
        {"url":"http://x/low","acodec":"opus","vcodec":"none","ext":"webm","abr":48},
        {"url":"http://x/med","acodec":"opus","vcodec":"none","ext":"webm","abr":128},
        {"url":"http://x/high","acodec":"opus","vcodec":"none","ext":"webm","abr":256},
        {"url":"http://x/av","acodec":"aac","vcodec":"avc1","ext":"mp4","tbr":512}
    ]})
}

#[test]
fn resolve_ranking_min_mid_max_prefers_audio_only() {
    assert_eq!(
        select_audio_stream(&formats_fixture(), AudioQuality::Low)
            .unwrap()
            .url,
        "http://x/low"
    );
    assert_eq!(
        select_audio_stream(&formats_fixture(), AudioQuality::Medium)
            .unwrap()
            .url,
        "http://x/med"
    );
    let high = select_audio_stream(&formats_fixture(), AudioQuality::High).unwrap();
    assert_eq!(high.url, "http://x/high"); // not the muxed 512 tbr entry
}

#[test]
fn no_formats_is_stream_resolution_error() {
    let v = serde_json::json!({"formats": [
        {"acodec":"none","vcodec":"avc1","ext":"mp4"},
        {"acodec":"opus","vcodec":"none","ext":"webm"}
    ]});
    let err = select_audio_stream(&v, AudioQuality::Medium).unwrap_err();
    assert!(matches!(
        err,
        rmusix::errors::RmusixError::StreamResolution(_)
    ));
    assert!(err.to_string().contains("no audio formats found"));
}

#[test]
fn http_429_maps_to_rate_limited_search_error() {
    for stderr in [
        "ERROR: HTTP Error 429: Too Many Requests",
        "WARNING: rate-limit reached, retrying",
    ] {
        let err = map_search_error(Some(1), stderr);
        assert!(matches!(err, rmusix::errors::RmusixError::Search(_)));
        assert!(err.to_string().contains("rate-limited"));
    }
}

#[tokio::test]
async fn missing_ytdlp_binary_is_config_error() {
    let client = rmusix::youtube::YtDlpClient::with_bin("/nonexistent/yt-dlp-xyz");
    let err = client.search("hello", 5).await.unwrap_err();
    assert!(matches!(err, rmusix::errors::RmusixError::Config(_)));
    assert!(err.to_string().contains("yt-dlp not found"));
}
