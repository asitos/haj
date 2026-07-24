use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::App;

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(5),  // header (haj logo)
                Constraint::Length(3),  // quick stats
                Constraint::Length(3),  // search bar
                Constraint::Min(0),    // spinning blahaj :3
            ]
            .as_ref(),
        )
        .split(f.area());

    let header_text = vec![
        Line::from(Span::styled("(blah)haj", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("fast. quiet. beautiful.", Style::default().fg(Color::DarkGray))),
    ];
    
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center);
        
    f.render_widget(header, chunks[0]);

    let orphan_color = if app.orphan_count > 0 { Color::Red } else { Color::Yellow };
    let orphan_modifier = if app.orphan_count > 0 { Modifier::BOLD } else { Modifier::empty() };

    let stats_text = Line::from(vec![
        Span::styled(format!(" total packages: {} ", app.package_list.len()), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" orphans: {} ", app.orphan_count), Style::default().fg(orphan_color).add_modifier(orphan_modifier)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(" last sync: today ", Style::default().fg(Color::Blue)),
    ]);

    let stats = Paragraph::new(stats_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center);
    f.render_widget(stats, chunks[1]);

    let search_bar = Paragraph::new(" search packages... (press '/' to focus)")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(search_bar, chunks[2]);

    let blahaj_box = Paragraph::new(app.dashboard_art.clone())
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(blahaj_box, chunks[3]);
}
