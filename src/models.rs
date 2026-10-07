use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A single YouTube track.
#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    pub video_id: String,
    pub title: String,
    pub channel: String,
    pub duration: Option<Duration>,
    pub thumbnail_url: Option<String>,
}

impl Song {
    pub fn youtube_url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.video_id)
    }
}

/// Playback lifecycle.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Loading,
    Playing,
    Paused,
    Buffering,
    Error(String),
}

/// Active TUI screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Search,
    Queue,
    Help,
}

/// Resolved playable stream (scrape-only, no external binaries).
#[derive(Debug, Clone)]
pub struct AudioStreamInfo {
    pub url: String,
    pub mime: String,
    pub bitrate: u32,
}

/// Requested audio quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_url_formats_correctly() {
        let song = Song {
            video_id: "dQw4w9WgXcQ".to_string(),
            title: "t".to_string(),
            channel: "c".to_string(),
            duration: None,
            thumbnail_url: None,
        };
        assert_eq!(
            song.youtube_url(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
    }
}
