use super::{App, DashboardWidget};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    
    // 1. Title & Subtitle
    let title_str = include_str!("../../resources/title.txt");
    let mut header_lines: Vec<Line> = title_str
        .lines()
        .map(|line| {
            Line::from(Span::styled(
                line,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    
    header_lines.push(Line::from(""));
    header_lines.push(Line::from(Span::styled(
        "bla(haj) :3",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    )));
    header_lines.push(Line::from(""));

    let header_height = header_lines.len() as u16;
    let show_shark = app.active_widget == DashboardWidget::Blahaj;
    
    let shark_height_fixed = if show_shark { 28 } else { 0 };
    let stats_actions_height = 4; // 1 spacer + 1 stats + 1 spacer + 1 actions
    let total_fixed = header_height + shark_height_fixed + stats_actions_height;
    
    // Only apply padding if there's excess space to center it all
    let padding = if area.height > total_fixed { (area.height - total_fixed) / 2 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(padding),           // [0] dynamic top padding
            Constraint::Length(header_height),     // [1] title + subtitle
            if show_shark { Constraint::Min(0) } else { Constraint::Length(0) }, // [2] Shark
            Constraint::Length(1),                 // [3] spacer
            Constraint::Length(1),                 // [4] stats
            Constraint::Length(1),                 // [5] spacer between stats and actions
            Constraint::Length(1),                 // [6] actions
            Constraint::Length(padding),           // [7] bottom padding
        ])
        .split(area);

    let header = Paragraph::new(header_lines).alignment(Alignment::Center);
    f.render_widget(header, chunks[1]);

    if show_shark && chunks[2].height > 0 {
        let art_height = app.dashboard_art.lines.len() as u16;
        let chunk_height = chunks[2].height;
        let v_pad = if chunk_height > art_height { (chunk_height - art_height) / 2 } else { 0 };
        
        let inner_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(v_pad),
                Constraint::Length(art_height),
                Constraint::Min(0),
            ])
            .split(chunks[2]);

        let blahaj_box = Paragraph::new(app.dashboard_art.clone())
            .alignment(Alignment::Center);
        f.render_widget(blahaj_box, inner_layout[1]);
    }

    // 3. Stats
    let installed_count = app.package_list.iter().filter(|p| p.is_installed).count();
    let updates = app.package_list.iter().filter(|p| p.is_upgradable).count();
    let unread_news = app
        .news_items
        .iter()
        .filter(|n| !app.read_news.contains(&n.link))
        .count();
    let has_critical = app
        .news_items
        .iter()
        .any(|n| n.is_critical && !app.read_news.contains(&n.link));

    let updates_str = if updates > 0 {
        format!("󰚰 {} updates", updates)
    } else {
        "󰚰 0 updates".to_string()
    };
    let updates_color = if updates > 0 { Color::LightGreen } else { Color::DarkGray };

    let news_str = if has_critical {
        "󰎞 !! manual intervention !!".to_string()
    } else {
        format!("󰎞 {} unread news", unread_news)
    };
    let news_color = if has_critical { Color::LightRed } else if unread_news > 0 { Color::LightYellow } else { Color::DarkGray };

    let orphan_color = if app.orphan_count > 0 { Color::LightRed } else { Color::DarkGray };

    let stats_text = Line::from(vec![
        Span::styled(format!("󰏗 {} pkgs", installed_count), Style::default().fg(Color::LightCyan)),
        Span::styled("   ·   ", Style::default().fg(Color::DarkGray)),
        Span::styled(updates_str, Style::default().fg(updates_color)),
        Span::styled("   ·   ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("󰆴 {} orphans", app.orphan_count), Style::default().fg(orphan_color)),
        Span::styled("   ·   ", Style::default().fg(Color::DarkGray)),
        Span::styled(news_str, Style::default().fg(news_color)),
    ]);

    let stats = Paragraph::new(stats_text).alignment(Alignment::Center);
    f.render_widget(stats, chunks[4]);

    // 4. Actions
    let actions = vec![
        ("search", "f / /"),
        ("news", "n"),
        ("toggle shark", "b"),
        ("help", "?"),
        ("quit", "q"),
    ];
    let mut action_spans = Vec::new();
    for (idx, (label, key)) in actions.iter().enumerate() {
        if idx > 0 {
            action_spans.push(Span::raw("    "));
        }
        action_spans.push(Span::styled(*label, Style::default().fg(Color::White)));
        action_spans.push(Span::styled(" [", Style::default().fg(Color::DarkGray)));
        action_spans.push(Span::styled(*key, Style::default().fg(Color::Yellow).bold()));
        action_spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));
    }
    let actions_p = Paragraph::new(Line::from(action_spans)).alignment(Alignment::Center);
    f.render_widget(actions_p, chunks[6]);
}
