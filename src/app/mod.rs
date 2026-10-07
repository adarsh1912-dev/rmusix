//! App state stub (foundation only, no logic yet).

use crate::models::{PlaybackState, Screen, Song};

/// Minimal TUI state placeholder owned by Agent 0.
#[derive(Debug, Default)]
pub struct AppState {
    /// Initial query from CLI (`rmusix [QUERY]`).
    pub initial_query: Option<String>,
    /// Current screen.
    pub screen: Option<Screen>,
    /// Search results placeholder.
    pub results: Vec<Song>,
    /// Playback placeholder.
    pub playback: Option<PlaybackState>,
}

impl AppState {
    pub fn new(initial_query: Option<String>) -> Self {
        Self {
            initial_query,
            screen: Some(Screen::Search),
            results: Vec::new(),
            playback: Some(PlaybackState::Stopped),
        }
    }
}
