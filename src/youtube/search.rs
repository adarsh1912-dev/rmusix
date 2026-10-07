//! Innertube search: validation, backend construction, result mapping.
//!
//! Pure-Rust scraping via `rusty_ytdl`; no API key, no subprocesses.

use std::time::Duration;

use crate::errors::RmusixError;
use crate::models::Song;

/// Build an Innertube search backend reusing the shared HTTP client.
pub fn backend_with_client(
    http: &reqwest::Client,
) -> Result<rusty_ytdl::search::YouTube, RmusixError> {
    let options = rusty_ytdl::RequestOptions {
        client: Some(http.clone()),
        ..Default::default()
    };
    rusty_ytdl::search::YouTube::new_with_options(&options).map_err(map_search_error)
}

/// Reject empty queries; return the trimmed query otherwise.
pub fn validate_query(query: &str) -> Result<String, RmusixError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(RmusixError::InvalidInput(
            "search query must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Execute a search with an existing backend; never panics.
///
/// Empty results map to `Ok(vec![])`. Transport failures are mapped to
/// `Search` / `Network` (see [`map_search_error`]).
pub async fn search_with_api(
    api: &rusty_ytdl::search::YouTube,
    query: &str,
    limit: usize,
) -> Result<Vec<Song>, RmusixError> {
    let normalized = validate_query(query)?;
    if limit == 0 {
        return Ok(Vec::new());
    }
    let capped = limit.min(50) as u64;
    let options = rusty_ytdl::search::SearchOptions {
        limit: capped,
        search_type: rusty_ytdl::search::SearchType::Video,
        safe_search: false,
    };
    let results = api
        .search(normalized, Some(&options))
        .await
        .map_err(map_search_error)?;
    Ok(songs_from_results(results, limit))
}

/// Map `rusty_ytdl` search failures to [`RmusixError`] without panicking.
///
/// - HTTP 429 / rate-limit hints → `Search("Search rate-limited…")`
/// - timeouts / deadlines → `Network(…)`
/// - everything else → `Search(…)`
pub fn map_search_error(e: rusty_ytdl::VideoError) -> RmusixError {
    let msg = e.to_string();
    if is_rate_limited_message(&msg) {
        return RmusixError::Search(format!("Search rate-limited: {msg}"));
    }
    if is_timeout_message(&msg) {
        return RmusixError::Network(format!("search network timeout: {msg}"));
    }
    if let rusty_ytdl::VideoError::Reqwest(inner) = &e {
        if inner.is_timeout() {
            return RmusixError::Network(format!("search network timeout: {msg}"));
        }
        if let Some(status) = inner.status()
            && status.as_u16() == 429
        {
            return RmusixError::Search(format!("Search rate-limited: {msg}"));
        }
    }
    RmusixError::Search(msg)
}

/// True when `msg` looks like an HTTP 429 / rate-limit signal.
pub fn is_rate_limited_message(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("429")
        || lower.contains("too many requests")
        || lower.contains("rate-limit")
        || lower.contains("rate limit")
        || lower.contains("rate_limited")
        || lower.contains("quota exceeded")
}

fn is_timeout_message(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("deadline exceeded")
        || lower.contains("operation timed out")
        || lower.contains("elapsed")
}

/// Convert raw [`rusty_ytdl`] results into [`Song`]s, honouring `limit`.
///
/// Only `Video` results become songs; channels/playlists are skipped.
/// Entries with blank ids are skipped. Never panics.
pub fn songs_from_results(
    results: Vec<rusty_ytdl::search::SearchResult>,
    limit: usize,
) -> Vec<Song> {
    let mut out = Vec::new();
    for item in results {
        if out.len() >= limit {
            break;
        }
        let video = match item {
            rusty_ytdl::search::SearchResult::Video(v) => v,
            _ => continue,
        };
        if let Some(song) = song_from_search_video(&video) {
            out.push(song);
        }
    }
    out
}

/// Map one search video to a [`Song`]; `None` when the id is blank.
pub fn song_from_search_video(video: &rusty_ytdl::search::Video) -> Option<Song> {
    let id = video.id.trim();
    if id.is_empty() {
        return None;
    }
    Some(Song {
        video_id: id.to_string(),
        title: video.title.clone(),
        channel: video.channel.name.clone(),
        duration: duration_from_millis(video.duration),
        thumbnail_url: best_thumbnail_url(&video.thumbnails),
    })
}

/// `rusty_ytdl` reports search durations in milliseconds; `0` means unknown.
pub fn duration_from_millis(ms: u64) -> Option<Duration> {
    if ms == 0 {
        None
    } else {
        Some(Duration::from_millis(ms))
    }
}

/// Pick the largest-area thumbnail with a non-blank URL.
pub fn best_thumbnail_url(thumbnails: &[rusty_ytdl::Thumbnail]) -> Option<String> {
    let mut best: Option<&rusty_ytdl::Thumbnail> = None;
    let mut best_area: u64 = 0;
    for thumb in thumbnails {
        if thumb.url.trim().is_empty() {
            continue;
        }
        let area = thumb.width.saturating_mul(thumb.height);
        let is_better = match best {
            None => true,
            Some(_) => area > best_area,
        };
        if is_better {
            best_area = area;
            best = Some(thumb);
        }
    }
    best.map(|t| t.url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_ytdl::search::{Channel, SearchResult, Video};

    fn channel_named(name: &str) -> Channel {
        Channel {
            id: "chan".to_string(),
            name: name.to_string(),
            url: String::new(),
            icon: Vec::new(),
            verified: false,
            subscribers: 0,
        }
    }

    fn video_fixture(id: &str, duration_ms: u64) -> Video {
        Video {
            id: id.to_string(),
            url: format!("https://www.youtube.com/watch?v={id}"),
            title: "Title".to_string(),
            description: String::new(),
            duration: duration_ms,
            duration_raw: "3:45".to_string(),
            thumbnails: vec![
                rusty_ytdl::Thumbnail {
                    width: 120,
                    height: 90,
                    url: "https://i.ytimg.com/small.jpg".to_string(),
                },
                rusty_ytdl::Thumbnail {
                    width: 480,
                    height: 360,
                    url: "https://i.ytimg.com/large.jpg".to_string(),
                },
            ],
            channel: channel_named("Some Channel"),
            uploaded_at: None,
            views: 0,
        }
    }

    #[test]
    fn validate_query_rejects_blank() {
        for q in ["", "   ", "\n\t "] {
            assert!(matches!(
                validate_query(q),
                Err(RmusixError::InvalidInput(_))
            ));
        }
        assert_eq!(
            validate_query("  hello  ").expect("valid query"),
            "hello".to_string()
        );
    }

    #[test]
    fn duration_zero_is_none_otherwise_millis() {
        assert_eq!(duration_from_millis(0), None);
        assert_eq!(
            duration_from_millis(225_000),
            Some(Duration::from_millis(225_000))
        );
        assert_eq!(duration_from_millis(1), Some(Duration::from_millis(1)));
    }

    #[test]
    fn mapping_keeps_video_fields_and_best_thumbnail() {
        let song =
            song_from_search_video(&video_fixture("abc123XYZ_-", 225_000)).expect("fixture has id");
        assert_eq!(song.video_id, "abc123XYZ_-");
        assert_eq!(song.title, "Title");
        assert_eq!(song.channel, "Some Channel");
        assert_eq!(song.duration, Some(Duration::from_millis(225_000)));
        assert_eq!(
            song.thumbnail_url.as_deref(),
            Some("https://i.ytimg.com/large.jpg")
        );
    }

    #[test]
    fn mapping_skips_blank_ids_and_respects_limit() {
        let results = vec![
            SearchResult::Video(video_fixture("", 10)),
            SearchResult::Video(video_fixture("id1", 10)),
            SearchResult::Video(video_fixture("id2", 0)),
            SearchResult::Video(video_fixture("id3", 30)),
        ];
        let songs = songs_from_results(results, 2);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].video_id, "id1");
        assert_eq!(songs[1].video_id, "id2");
        assert_eq!(songs[1].duration, None);
    }

    #[test]
    fn empty_results_map_to_empty_vec() {
        assert!(songs_from_results(Vec::new(), 10).is_empty());
    }

    #[test]
    fn error_mapping_429_is_search_rate_limited() {
        let err = map_search_error(rusty_ytdl::VideoError::BodyCannotParsed);
        assert!(matches!(err, RmusixError::Search(_)));
        assert!(is_rate_limited_message("HTTP 429 Too Many Requests"));
        assert!(is_rate_limited_message("search rate-limited by youtube"));
        assert!(!is_rate_limited_message("video not found"));
    }

    #[test]
    fn timeout_messages_detected() {
        assert!(is_timeout_message("request timed out"));
        assert!(is_timeout_message("operation timeout"));
        assert!(is_timeout_message("deadline exceeded"));
        assert!(!is_timeout_message("video not found"));
    }

    #[test]
    fn thumbnails_blank_or_empty_yield_none() {
        assert_eq!(best_thumbnail_url(&[]), None);
        let blanks = vec![rusty_ytdl::Thumbnail {
            width: 100,
            height: 100,
            url: "   ".to_string(),
        }];
        assert_eq!(best_thumbnail_url(&blanks), None);
    }
}
