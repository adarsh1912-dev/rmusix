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
fn rate_limit_messages_detected() {
    assert!(search::is_rate_limited_message(
        "HTTP 429 Too Many Requests"
    ));
    assert!(search::is_rate_limited_message(
        "search rate-limited by youtube"
    ));
    assert!(search::is_rate_limited_message("quota exceeded"));
    assert!(!search::is_rate_limited_message("video not found"));
}

#[test]
fn search_error_mapping_keeps_429_message() {
    // BodyCannotParsed carries no status; mapping must still be Search.
    let err = search::map_search_error(rusty_ytdl::VideoError::BodyCannotParsed);
    assert!(matches!(err, RmusixError::Search(_)));
    // The user-facing string for 429 must mention rate limiting.
    let msg = RmusixError::Search("Search rate-limited: HTTP 429".to_string()).to_string();
    assert!(msg.to_lowercase().contains("rate-limited") || msg.contains("429"));
}

#[test]
fn resolve_error_mapping_branches() {
    let not_found = video::map_resolve_error(rusty_ytdl::VideoError::VideoNotFound);
    assert!(matches!(not_found, RmusixError::StreamResolution(_)));
    assert!(not_found.to_string().to_lowercase().contains("not found"));

    let body = video::map_resolve_error(rusty_ytdl::VideoError::BodyCannotParsed);
    assert!(matches!(body, RmusixError::StreamResolution(_)));
}

#[test]
fn duration_and_thumbnail_helpers() {
    assert_eq!(search::duration_from_millis(0), None);
    assert!(search::duration_from_millis(1000).is_some());
    assert_eq!(search::best_thumbnail_url(&[]), None);
}
