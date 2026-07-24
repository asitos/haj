use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, List, ListItem, LineGauge}, 
    prelude::Stylize,
    Frame,
};
use super::App;

pub fn render_popup(f: &mut Frame, app: &App) {
    if !app.is_installing {
        return;
    }

    let area = centered_rect(65, 45, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" active command ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // action text
            Constraint::Length(1), // blank spacer
            Constraint::Length(1), // thin loading bar
            Constraint::Length(1), // blank spacer
            Constraint::Min(0),    // live logs
        ].as_ref())
        .split(inner_area);

    let action_text = Paragraph::new(format!("{} {}", "✓".cyan(), app.current_action));
    f.render_widget(action_text, chunks[0]);

    let ratio = f64::from(app.progress).clamp(0.0, 100.0) / 100.0;
    
    let gauge = LineGauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(" {} ", app.current_action))
        )
        .filled_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .filled_symbol("━")
        .unfilled_symbol("─")
        .ratio(ratio);
    f.render_widget(gauge, chunks[2]);

    let log_items: Vec<ListItem> = app.transaction_logs.iter()
        .map(|log| ListItem::new(log.clone()))
        .collect();
        
    let logs_list = List::new(log_items)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::NONE)); 
        
    f.render_widget(logs_list, chunks[4]);
}

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
