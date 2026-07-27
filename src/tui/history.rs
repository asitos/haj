use crate::tui::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(f.area());

    let items: Vec<ListItem> = app.history_items.iter().map(|item| {
        let color = if item.contains("installed") { Color::Green }
                    else if item.contains("upgraded") { Color::Cyan }
                    else if item.contains("removed") { Color::Red }
                    else { Color::White };

        ListItem::new(Line::from(Span::styled(item.clone(), Style::default().fg(color))))
    }).collect();

    let list = List::new(items)
        .block(Block::default().title(" recent pacman history ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, chunks[0], &mut app.history_state);

    let footer = Paragraph::new(Span::styled(" j/k move • esc back • q quit ", Style::default().fg(Color::DarkGray))).alignment(Alignment::Center);
    f.render_widget(footer, chunks[1]);
}
