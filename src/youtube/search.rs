//! yt-dlp search (subprocess backend).

use crate::errors::RmusixError;
use crate::models::Song;

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

/// Stub: filter helper kept for API compat; real search via yt-dlp (Agent 1).
pub fn songs_from_results(results: Vec<Song>, limit: usize) -> Vec<Song> {
    results.into_iter().take(limit).collect()
}

pub fn duration_from_millis(ms: u64) -> Option<std::time::Duration> {
    if ms == 0 {
        None
    } else {
        Some(std::time::Duration::from_millis(ms))
    }
}

/// Parse one `--dump-json` ytsearch line into a [`Song`].
/// Each line is a full video JSON object.
pub fn song_from_json(v: &serde_json::Value) -> Option<Song> {
    let video_id = v.get("id")?.as_str()?;
    if video_id.trim().is_empty() {
        return None;
    }
    let title = v
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Unknown title")
        .to_string();
    let channel = v
        .get("uploader")
        .or_else(|| v.get("channel"))
        .or_else(|| v.get("uploader_id"))
        .and_then(|c| c.as_str())
        .unwrap_or("Unknown channel")
        .to_string();
    let duration = v
        .get("duration")
        .and_then(serde_json::Value::as_f64)
        .filter(|d| *d > 0.0)
        .map(|d| std::time::Duration::from_secs_f64(d));
    let thumbnail_url = v
        .get("thumbnail")
        .and_then(|t| t.as_str())
        .map(str::to_string);
    Some(Song {
        video_id: video_id.to_string(),
        title,
        channel,
        duration,
        thumbnail_url,
    })
}

/// Parse `--dump-json` JSON-LINES stdout into songs, truncated to `limit`.
pub fn parse_search_lines(stdout: &str, limit: usize) -> Vec<Song> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        if out.len() >= limit {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(song) = song_from_json(&v) {
            out.push(song);
        }
    }
    out
}

/// Map a failed yt-dlp search invocation to an error.
/// HTTP 429 / "rate limit" in stderr becomes a rate-limited [`RmusixError::Search`].
pub fn map_search_error(exit_code: Option<i32>, stderr: &str) -> RmusixError {
    let lower = stderr.to_lowercase();
    if lower.contains("429")
        || lower.contains("rate-limit")
        || lower.contains("rate limit")
        || lower.contains("too many requests")
    {
        return RmusixError::Search("rate-limited by YouTube (HTTP 429)".to_string());
    }
    RmusixError::Search(format!(
        "yt-dlp search failed (exit={exit_code:?}): {}",
        stderr.trim().chars().take(300).collect::<String>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE_A: &str = r#"{"id":"dQw4w9WgXcQ","title":"Never Gonna Give You Up","uploader":"Rick Astley","duration":212,"thumbnail":"http://img/a.jpg"}"#;
    const LINE_B: &str = r#"{"id":"abc123","title":"Song B","channel":"Chan B","duration":180.5,"thumbnail":"http://img/b.jpg"}"#;

    #[test]
    fn validate_query_rejects_blank() {
        for q in ["", "   ", "\n\t "] {
            assert!(matches!(
                validate_query(q),
                Err(RmusixError::InvalidInput(_))
            ));
        }
        assert_eq!(validate_query("  hello  ").expect("valid"), "hello");
    }

    #[test]
    fn duration_zero_is_none_otherwise_millis() {
        assert_eq!(duration_from_millis(0), None);
        assert_eq!(
            duration_from_millis(225_000),
            Some(std::time::Duration::from_millis(225_000))
        );
    }

    #[test]
    fn parses_json_lines_and_truncates() {
        let stdout = format!("{LINE_A}\nnot json\n{LINE_B}\n");
        let all = parse_search_lines(&stdout, 10);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].video_id, "dQw4w9WgXcQ");
        assert_eq!(all[0].channel, "Rick Astley");
        assert_eq!(all[0].duration, Some(std::time::Duration::from_secs(212)));
        let one = parse_search_lines(&stdout, 1);
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn rate_limit_maps_to_search_error() {
        let err = map_search_error(Some(1), "ERROR: HTTP Error 429: Too Many Requests");
        match err {
            RmusixError::Search(msg) => assert!(msg.contains("rate-limited")),
            other => panic!("wrong variant: {other:?}"),
        }
    }
}
