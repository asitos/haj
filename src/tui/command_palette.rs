use crate::tui::app::{ActiveScreen, Transition};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_popup(f: &mut Frame, buffer: &str) {
    let area = centered_rect(50, 10, f.area());
    f.render_widget(Clear, area);

    let display_text = format!("> {}█", buffer);
    let block = Block::default()
        .title(" run command ")
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(Line::from(display_text))
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

pub fn handle_key(key: crossterm::event::KeyEvent, buffer: &mut String) -> Transition {
    match key.code {
        KeyCode::Esc => Transition::CloseCommandPalette,
        KeyCode::Backspace => {
            buffer.pop();
            Transition::None
        }
        KeyCode::Enter => {
            let cmd = buffer.trim().to_lowercase();
            buffer.clear();
            parse_command(&cmd)
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            Transition::None
        }
        _ => Transition::None,
    }
}

fn parse_command(cmd: &str) -> Transition {
    match cmd {
        "update" | "upgrade" => Transition::SwitchScreen(ActiveScreen::Updates),
        "stats" => Transition::SwitchScreen(ActiveScreen::Stats),
        "orphan" | "orphans" => Transition::SwitchScreen(ActiveScreen::Orphans),
        "clean" => Transition::SwitchScreen(ActiveScreen::Clean),
        "history" => Transition::SwitchScreen(ActiveScreen::History),
        "browser" => Transition::SwitchScreen(ActiveScreen::Browser),
        "quit" | "exit" => Transition::Quit,
        _ => Transition::CloseCommandPalette, 
    }
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
