//! UI stub (foundation only, no rendering logic yet).

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;

/// Placeholder render entry point for later agents.
#[allow(dead_code)]
pub fn render(_frame: &mut Frame, _area: Rect, _state: &AppState) {
    // Intentionally empty: UI implementation belongs to a later agent.
}
