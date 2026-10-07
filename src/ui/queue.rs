use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(" Queue ");
    if state.queue.is_empty() {
        frame.render_widget(
            List::new(vec![ListItem::new("Queue is empty. Press a to enqueue.")]).block(block),
            area,
        );
        return;
    }
    let items: Vec<ListItem> = state
        .queue
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let cur = i == state.queue_selected;
            let prefix = if cur { "> " } else { "  " };
            ListItem::new(Line::from(vec![Span::styled(
                format!("{prefix}{} — {}", s.title, s.channel),
                if cur {
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            )]))
        })
        .collect();
    frame.render_widget(List::new(items).block(block), area);
}
