use super::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, LineGauge, Paragraph, Wrap},
};

pub fn render_popup(f: &mut Frame, app: &App) {
    if !app.is_installing {
        return;
    }

    let area = centered_rect(55, 35, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" active command [q to abort] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(1), // action text
                Constraint::Length(1), // blank spacer
                Constraint::Length(1), // thin loading bar
                Constraint::Length(1), // blank spacer
                Constraint::Min(0),    // live logs
            ]
            .as_ref(),
        )
        .split(inner_area);

    let action_text = Paragraph::new(format!("{} {}", "✓".cyan(), app.current_action));
    f.render_widget(action_text, chunks[0]);

    let ratio = f64::from(app.progress).clamp(0.0, 100.0) / 100.0;

    let gauge = LineGauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(" {} ", app.current_action)),
        )
        .filled_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .filled_symbol("━")
        .unfilled_symbol("─")
        .ratio(ratio);
    f.render_widget(gauge, chunks[2]);

    let log_lines: Vec<Line> = app
        .transaction_logs
        .iter()
        .map(|log| Line::from(log.clone()))
        .collect();

    let logs_paragraph = Paragraph::new(log_lines)
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(logs_paragraph, chunks[4]);
}

pub fn render_confirm_popup(f: &mut Frame, app: &App) {
    if !app.show_prompt {
        return;
    }

    let area = centered_rect(40, 20, f.area());
    f.render_widget(Clear, area);

    let action_color = if app.prompt_type == "install" { Color::Green } else { Color::Red };
    
    let block = Block::default()
        .title(" confirm transaction ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(action_color));

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw(format!("Proceed to {} ", app.prompt_type)),
            Span::styled(format!("{}", app.prompt_targets.len()), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" package(s)?"),
        ]),
        Line::from(""),
        Line::from(Span::styled(" [Y]es   [N]o ", Style::default().fg(Color::DarkGray))),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(paragraph, area);
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
