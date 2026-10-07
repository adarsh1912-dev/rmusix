//! yt-dlp search validation (subprocess backend).

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
