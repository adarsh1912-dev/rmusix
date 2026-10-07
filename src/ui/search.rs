use crate::app::AppState;
use crate::ui::fmt_duration;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

const SPINNER: [char; 4] = ['|', '/', '-', '\\'];

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Search: {} ", state.query));
    if state.is_searching {
        let ch = SPINNER[state.spinner_frame % SPINNER.len()];
        let items = vec![ListItem::new(Line::from(vec![Span::styled(
            format!("{ch} Searching..."),
            Style::default().fg(Color::Cyan),
        )]))];
        frame.render_widget(List::new(items).block(block), area);
        return;
    }
    if let Some(err) = &state.search_error {
        let items = vec![ListItem::new(Line::from(vec![Span::styled(
            format!("Error: {err}"),
            Style::default().fg(Color::Red),
        )]))];
        frame.render_widget(List::new(items).block(block), area);
        return;
    }
    if state.results.is_empty() {
        let items = vec![ListItem::new("No results. Press / to search.")];
        frame.render_widget(List::new(items).block(block), area);
        return;
    }
    let items: Vec<ListItem> = state
        .results
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let dur = s
                .duration
                .map(fmt_duration)
                .unwrap_or_else(|| "--:--".into());
            let prefix = if i == state.selected { "> " } else { "  " };
            let line = Line::from(vec![Span::styled(
                format!("{prefix}{} — {} + {dur}", s.title, s.channel),
                if i == state.selected {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            )]);
            ListItem::new(line)
        })
        .collect();
    frame.render_widget(List::new(items).block(block), area);
}
