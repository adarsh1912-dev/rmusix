use crate::app::AppState;
use crate::ui::fmt_duration;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

const SPINNER: [char; 4] = ['|', '/', '-', '\\'];

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let icon = if state.playing() { "⏸" } else { "▶" };
    let title = state.current_title.as_deref().unwrap_or("—");
    let pos = match state.total {
        Some(t) => format!(" {}/{}", fmt_duration(state.position), fmt_duration(t)),
        None => format!(" {}", fmt_duration(state.position)),
    };
    let vol = if state.muted {
        "Muted".to_string()
    } else {
        format!("Vol {}%", (state.volume * 100.0).round() as u32)
    };
    let spin = if state.is_searching {
        format!(" {}", SPINNER[state.spinner_frame % SPINNER.len()])
    } else {
        String::new()
    };
    let text = if let Some(crate::models::PlaybackState::Error(e)) = &state.playback {
        format!("Error: {e}")
    } else {
        format!("{icon} {title}{pos}  {vol}{spin}")
    };
    let p = Paragraph::new(text).style(Style::default().fg(Color::Yellow));
    frame.render_widget(p.block(Block::default().borders(Borders::ALL)), area);
}
