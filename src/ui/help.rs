use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

const HELP: &str = "/ search mode | Up/Down/j/k select | Enter play | a enqueue | Space play/pause\nn/p next/prev | Left/Right ∓5s | +/- vol | m mute\nTab screens | 1/2/3 screens | Esc back | q quit";

pub fn render(frame: &mut Frame, area: Rect) {
    let p = Paragraph::new(HELP).block(Block::default().borders(Borders::ALL).title(" Help "));
    frame.render_widget(p, area);
}
