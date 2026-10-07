//! Pure ratatui rendering + key mapping. Never blocks, no IO.

pub mod help;
pub mod layout;
pub mod player;
pub mod queue;
pub mod search;

use crate::app::{AppEvent, AppState, Mode};
use crate::models::Screen;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;

pub use layout::render as render_layout;

/// Pure render entry point: `render(frame, state)`.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    render_layout(frame, area, state);
}

/// Map a key event to an [`AppEvent`] (pure, no side effects).
pub fn key_to_action(key: KeyEvent, state: &AppState) -> AppEvent {
    // Search-input mode: printable chars append, Esc/Enter exit/submit.
    if state.mode == Mode::Search {
        match key.code {
            KeyCode::Esc => return AppEvent::ExitSearchMode,
            KeyCode::Enter => return AppEvent::SearchSubmit,
            KeyCode::Backspace => return AppEvent::SearchBackspace,
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
                    return AppEvent::SearchInput(c);
                }
                return AppEvent::Noop;
            }
            _ => return AppEvent::Noop,
        }
    }
    match key.code {
        KeyCode::Char('q') => AppEvent::Quit,
        KeyCode::Char('/') => AppEvent::EnterSearchMode,
        KeyCode::Enter => AppEvent::PlaySelected,
        KeyCode::Char('a') => AppEvent::EnqueueSelected,
        KeyCode::Char(' ') => AppEvent::TogglePlayPause,
        KeyCode::Char('n') => AppEvent::Next,
        KeyCode::Char('p') => AppEvent::Prev,
        KeyCode::Left => AppEvent::SeekBackward,
        KeyCode::Right => AppEvent::SeekForward,
        KeyCode::Char('+') | KeyCode::Char('=') => AppEvent::VolumeUp,
        KeyCode::Char('-') | KeyCode::Char('_') => AppEvent::VolumeDown,
        KeyCode::Char('m') => AppEvent::ToggleMute,
        KeyCode::Tab => AppEvent::CycleScreen,
        KeyCode::Esc => AppEvent::ExitSearchMode,
        KeyCode::Char('1') => AppEvent::SwitchScreen(Screen::Search),
        KeyCode::Char('2') => AppEvent::SwitchScreen(Screen::Queue),
        KeyCode::Char('3') | KeyCode::Char('?') => AppEvent::SwitchScreen(Screen::Help),
        _ => AppEvent::Noop,
    }
}

pub fn fmt_duration(d: std::time::Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Song;
    use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Duration;

    fn key(code: KeyCode) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    fn song(title: &str) -> Song {
        Song {
            video_id: "x".into(),
            title: title.into(),
            channel: "ch".into(),
            duration: Some(Duration::from_secs(200)),
            thumbnail_url: None,
        }
    }

    fn snapshot(state: &AppState) -> String {
        let backend = TestBackend::new(60, 15);
        let mut t = Terminal::new(backend).unwrap();
        t.draw(|f| render(f, state)).unwrap();
        t.backend().to_string()
    }

    #[test]
    fn empty_state_no_panic() {
        let s = AppState::default();
        let out = snapshot(&s);
        assert!(out.contains("RMUSIX"));
    }

    #[test]
    fn loading_state_shows_spinner() {
        let s = AppState {
            is_searching: true,
            ..AppState::default()
        };
        let out = snapshot(&s);
        assert!(out.contains("Searching"));
    }

    #[test]
    fn results_state_shows_selector() {
        let s = AppState {
            results: vec![song("hello"), song("world")],
            ..AppState::default()
        };
        let out = snapshot(&s);
        assert!(out.contains("hello"));
    }

    #[test]
    fn error_state_shows_error() {
        let s = AppState {
            search_error: Some("boom".into()),
            ..AppState::default()
        };
        let out = snapshot(&s);
        assert!(out.contains("boom"));
    }

    #[test]
    fn key_bindings() {
        let s = AppState::default();
        assert_eq!(
            key_to_action(key(KeyCode::Char('/')), &s),
            AppEvent::EnterSearchMode
        );
        assert_eq!(
            key_to_action(key(KeyCode::Enter), &s),
            AppEvent::PlaySelected
        );
        assert_eq!(
            key_to_action(key(KeyCode::Char('a')), &s),
            AppEvent::EnqueueSelected
        );
        assert_eq!(
            key_to_action(key(KeyCode::Char(' ')), &s),
            AppEvent::TogglePlayPause
        );
        assert_eq!(key_to_action(key(KeyCode::Char('n')), &s), AppEvent::Next);
        assert_eq!(key_to_action(key(KeyCode::Char('p')), &s), AppEvent::Prev);
        assert_eq!(
            key_to_action(key(KeyCode::Left), &s),
            AppEvent::SeekBackward
        );
        assert_eq!(
            key_to_action(key(KeyCode::Right), &s),
            AppEvent::SeekForward
        );
        assert_eq!(
            key_to_action(key(KeyCode::Char('+')), &s),
            AppEvent::VolumeUp
        );
        assert_eq!(
            key_to_action(key(KeyCode::Char('-')), &s),
            AppEvent::VolumeDown
        );
        assert_eq!(
            key_to_action(key(KeyCode::Char('m')), &s),
            AppEvent::ToggleMute
        );
        assert_eq!(key_to_action(key(KeyCode::Tab), &s), AppEvent::CycleScreen);
        assert_eq!(key_to_action(key(KeyCode::Char('q')), &s), AppEvent::Quit);
    }
}
