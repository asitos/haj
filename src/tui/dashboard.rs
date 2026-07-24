use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::App;

pub fn render(f: &mut Frame, _app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(5),  // header (haj logo)
                Constraint::Length(3),  // quick stats
                Constraint::Length(3),  // search bar
                Constraint::Min(10),    // recent transactions
            ]
            .as_ref(),
        )
        .split(f.area());

    // 1. the welcome header
    let header_text = vec![
        Line::from(Span::styled("haj", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("fast. quiet. beautiful.", Style::default().fg(Color::DarkGray))),
    ];
    
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center);
        
    f.render_widget(header, chunks[0]);

    // 2. the search bar mockup
    let search_bar = Paragraph::new(" search packages... (press '/' to focus)")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
        
    f.render_widget(search_bar, chunks[2]);
}
