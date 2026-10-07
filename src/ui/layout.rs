use crate::app::AppState;
use crate::models::Screen;
use crate::ui::{fmt_duration, help, player, queue, search};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);
    render_header(frame, chunks[0], state);
    match state.current_screen() {
        Screen::Search => search::render(frame, chunks[1], state),
        Screen::Queue => queue::render(frame, chunks[1], state),
        Screen::Help => help::render(frame, chunks[1]),
    }
    player::render(frame, chunks[2], state);
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let mode = match state.current_screen() {
        Screen::Search => "SEARCH",
        Screen::Queue => "QUEUE",
        Screen::Help => "HELP",
    };
    let status = if state.is_searching {
        " [searching...]".to_string()
    } else if let Some(e) = &state.search_error {
        format!(" [error: {e}]")
    } else if !state.query.is_empty() {
        format!(" [query: {}]", state.query)
    } else {
        String::new()
    };
    let text = format!("RMUSIX | {mode}{status}");
    let p = Paragraph::new(text).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let block = Block::default().borders(Borders::ALL);
    frame.render_widget(p.block(block), area);
}

#[allow(dead_code)]
fn fmt_pos(state: &AppState) -> String {
    match state.total {
        Some(t) => format!("{}/{}", fmt_duration(state.position), fmt_duration(t)),
        None => fmt_duration(state.position),
    }
}
