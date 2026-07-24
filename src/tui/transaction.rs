use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Gauge, Paragraph},
    prelude::Stylize,
    Frame,
};
use super::App;

pub fn render_popup(f: &mut Frame, app: &App) {
    if !app.is_installing {
        return;
    }

    // calculate a centered popup window (60% w, 40% h)
    let area = centered_rect(60, 40, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" transaction active ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // current action text
            Constraint::Length(3), // gauge/progress bar
            Constraint::Min(0),    // logs
        ].as_ref())
        .split(inner_area);

    // render the action text
    let action_text = Paragraph::new(format!("{} {}", "✓".cyan(), app.current_action));
    f.render_widget(action_text, chunks[0]);

    // render the beautiful ratatui gauge (the bun/cargo aesthetic)
    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .percent(app.progress)
        .label(format!("{}%", app.progress));
    
    f.render_widget(gauge, chunks[1]);
}

// helper function to center the popup
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ].as_ref())
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ].as_ref())
        .split(popup_layout[1])[1]
}
