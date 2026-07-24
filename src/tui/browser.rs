use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    prelude::Stylize,
    Frame,
};
use super::App;

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.area());

    let search_bar = Paragraph::new(" search: ")
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(search_bar, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(chunks[1]);

    let items: Vec<ListItem> = app
        .package_list
        .iter()
        .map(|pkg| ListItem::new(pkg.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" packages "))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, content_chunks[0], &mut app.list_state);

    let selected_idx = app.list_state.selected().unwrap_or(0);
    let selected_pkg = &app.package_list[selected_idx];

    let details_text = vec![
        Line::from(vec![Span::styled(selected_pkg.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from("version: 1.0.0-1 (mock)"),
        Line::from("repository: extra"),
        Line::from(""),
        Line::from("description:"),
        Line::from("this is a beautiful mock description for the tui."),
        Line::from(""),
        Line::from(Span::styled("[i] install  [r] remove", Style::default().fg(Color::DarkGray))),
    ];

    let details = Paragraph::new(details_text)
        .block(Block::default().borders(Borders::ALL).title(" details "));

    f.render_widget(details, content_chunks[1]);
}
