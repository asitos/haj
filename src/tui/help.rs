use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub fn render_popup(f: &mut Frame) {
    let area = centered_rect(60, 80, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" haj keybinds reference ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let help_text = vec![
        Line::from(Span::styled(
            "navigation",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "──────────",
            Style::default().fg(Color::DarkGray),
        )),
        format_bind("tab", "next widget"),
        format_bind("shift+tab", "previous widget"),
        format_bind("enter", "open widget"),
        format_bind("esc", "back"),
        Line::from(""),
        Line::from(Span::styled(
            "dashboard",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "─────────",
            Style::default().fg(Color::DarkGray),
        )),
        format_bind("b", "BLAHAJ :3"),
        format_bind("n", "news"),
        format_bind("u", "sync repos"),
        format_bind("t", "statistics"),
        format_bind("h", "history"),
        format_bind("g", "groups"),
        format_bind("o", "orphans"),
        format_bind("c", "clean cache"),
        Line::from(""),
        Line::from(Span::styled(
            "global",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("──────", Style::default().fg(Color::DarkGray))),
        format_bind("?", "toggle help"),
        format_bind("q", "quit"),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn format_bind(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:<14}", key), Style::default().fg(Color::Cyan)),
        Span::styled(desc.to_string(), Style::default().fg(Color::White)),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
