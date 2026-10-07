//! App state — Locked shape (UI-compatible).

use crate::models::{PlaybackState, Screen, Song};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Search,
}

/// Alias required by spec; identical to [`Mode`].
pub type InputMode = Mode;

#[derive(Debug)]
pub struct AppState {
    pub initial_query: Option<String>,
    pub screen: Option<Screen>,
    pub results: Vec<Song>,
    pub playback: Option<PlaybackState>,
    pub mode: Mode,
    pub query: String,
    pub selected: usize,
    pub is_searching: bool,
    pub search_error: Option<String>,
    pub queue: Vec<Song>,
    pub queue_selected: usize,
    pub current_title: Option<String>,
    pub position: Duration,
    pub total: Option<Duration>,
    pub volume: f32,
    pub muted: bool,
    pub spinner_frame: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            initial_query: None,
            screen: Some(Screen::Search),
            results: Vec::new(),
            playback: Some(PlaybackState::Stopped),
            mode: Mode::Normal,
            query: String::new(),
            selected: 0,
            is_searching: false,
            search_error: None,
            queue: Vec::new(),
            queue_selected: 0,
            current_title: None,
            position: Duration::ZERO,
            total: None,
            volume: 0.5,
            muted: false,
            spinner_frame: 0,
        }
    }
}

impl AppState {
    pub fn new(initial_query: Option<String>) -> Self {
        Self {
            query: initial_query.clone().unwrap_or_default(),
            initial_query,
            screen: Some(Screen::Search),
            results: Vec::new(),
            playback: Some(PlaybackState::Stopped),
            ..Self::default()
        }
    }

    pub fn current_screen(&self) -> Screen {
        self.screen.unwrap_or(Screen::Search)
    }

    pub fn playing(&self) -> bool {
        matches!(self.playback, Some(PlaybackState::Playing))
    }
}
