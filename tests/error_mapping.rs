use rmusix::errors::RmusixError;
use rmusix::youtube::{search, video};

#[test]
fn blank_query_and_id_are_invalid_input() {
    assert!(matches!(
        search::validate_query("   "),
        Err(RmusixError::InvalidInput(_))
    ));
    assert!(matches!(
        video::validate_video_id(""),
        Err(RmusixError::InvalidInput(_))
    ));
}

#[test]
fn duration_helper() {
    assert_eq!(search::duration_from_millis(0), None);
    assert!(search::duration_from_millis(1000).is_some());
}

#[test]
fn missing_binary_hints_mention_pacman() {
    let yt = RmusixError::Config("yt-dlp not found (pacman -S yt-dlp)".to_string());
    assert!(yt.to_string().contains("pacman -S yt-dlp"));
    let mpv = RmusixError::Config("mpv not found (pacman -S mpv)".to_string());
    assert!(mpv.to_string().contains("pacman -S mpv"));
}

#[test]
fn rate_limit_maps_to_search_error() {
    for stderr in [
        "ERROR: HTTP Error 429: Too Many Requests",
        "WARNING: Travis: rate-limit exceeded",
    ] {
        match search::map_search_error(Some(1), stderr) {
            RmusixError::Search(msg) => assert!(msg.contains("rate-limited"), "got {msg}"),
            other => panic!("wrong variant: {other:?}"),
        }
    }
}

#[test]
fn no_formats_maps_to_stream_resolution() {
    let v = serde_json::json!({"formats": []});
    match rmusix::youtube::stream::select_audio_stream(&v, rmusix::models::AudioQuality::Medium) {
        Err(RmusixError::StreamResolution(msg)) => {
            assert!(msg.contains("no audio formats found"), "got {msg}")
        }
        other => panic!("wrong result: {other:?}"),
    }
}
