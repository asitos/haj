use super::{App, DashboardWidget};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, List, ListItem},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Stats Bar
            Constraint::Length(3), // Search
            Constraint::Min(0),    // Active Widget Area
            Constraint::Length(1), // Footer
        ].as_ref())
        .split(f.area());

    // 1. Header
    let header_text = vec![Line::from(Span::styled("(blah) haj", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))];
    let header = Paragraph::new(header_text).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))).alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // 2. Stats Bar with Badges
    let installed_count = app.package_list.iter().filter(|p| p.is_installed).count();
    let updates = app.package_list.iter().filter(|p| p.is_upgradable).count();
    let unread_news = app.news_items.iter().filter(|n| !app.read_news.contains(&n.link)).count();
    let has_critical = app.news_items.iter().any(|n| n.is_critical && !app.read_news.contains(&n.link));
    
    let updates_str = if updates > 0 { format!("{} updates", updates) } else { "0 updates".to_string() };
    let updates_color = if updates > 0 { Color::Green } else { Color::DarkGray };

    let (news_prefix, news_color, news_mod) = if has_critical {
        ("⚠ Manual intervention", Color::Red, Modifier::BOLD)
    } else if unread_news > 0 {
        ("●", Color::Yellow, Modifier::BOLD)
    } else {
        ("○", Color::DarkGray, Modifier::empty())
    };

    let news_string = if has_critical { format!(" {} ", news_prefix) } else { format!(" {} {} unread ", news_prefix, unread_news) };

    let orphan_color = if app.orphan_count > 0 { Color::Red } else { Color::Yellow };
    let orphan_modifier = if app.orphan_count > 0 { Modifier::BOLD } else { Modifier::empty() };

    let stats_text = Line::from(vec![
        Span::styled(format!(" pkgs: {} ", installed_count), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {} ", updates_str), Style::default().fg(updates_color).add_modifier(if updates > 0 { Modifier::BOLD } else { Modifier::empty() })),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" orphans: {} ", app.orphan_count), Style::default().fg(orphan_color).add_modifier(orphan_modifier)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" news:{} ", news_string), Style::default().fg(news_color).add_modifier(news_mod)),
    ]);

    let stats = Paragraph::new(stats_text).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))).alignment(Alignment::Center);
    f.render_widget(stats, chunks[1]);

    // 3. Search Bar
    let search_bar = Paragraph::new(" search packages... (f or /)").style(Style::default().fg(Color::DarkGray)).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(search_bar, chunks[2]);

    // 4. Dynamic Widget Area
    match app.active_widget {
        DashboardWidget::Blahaj => render_blahaj(f, app, chunks[3]),
        DashboardWidget::News => render_news(f, app, chunks[3]),
    }

    // 5. Dynamic Footer
    let footer_str = match app.active_widget {
        DashboardWidget::Blahaj => " tab widgets • / search • n news • u upgrade • c clean • q quit ",
        DashboardWidget::News => " enter open • tab widgets • / search • b blahaj • q quit ",
    };

    let footer = Paragraph::new(Span::styled(footer_str, Style::default().fg(Color::DarkGray))).alignment(Alignment::Center);
    f.render_widget(footer, chunks[4]);
}

fn render_blahaj(f: &mut Frame, app: &App, area: Rect) {
    let blahaj_box = Paragraph::new(app.dashboard_art.clone())
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(blahaj_box, area);
}

fn render_news(f: &mut Frame, app: &App, area: Rect) {
    let mut items = Vec::new();
    items.push(ListItem::new(Line::from("")));

    for news in app.news_items.iter().take(5) {
        let is_read = app.read_news.contains(&news.link);
        let (prefix, color, modifier) = if news.is_critical { ("⚠ ", Color::Red, Modifier::BOLD) }
            else if !is_read { ("● ", Color::White, Modifier::BOLD) }
            else { ("○ ", Color::DarkGray, Modifier::empty()) };

        let date_str = chrono::DateTime::parse_from_rfc2822(&news.pub_date).map(|dt| dt.format("%b %d").to_string()).unwrap_or_default();

        items.push(ListItem::new(vec![
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(prefix, Style::default().fg(color).add_modifier(modifier)),
                Span::styled(news.title.clone(), Style::default().fg(if is_read { Color::DarkGray } else { Color::White }).add_modifier(modifier)),
            ]),
            Line::from(Span::styled(format!("    {}", date_str), Style::default().fg(Color::DarkGray))),
            Line::from(""),
        ]));
    }

    items.push(ListItem::new(Line::from(Span::styled("  ────────────────────────────────────────────────────────────", Style::default().fg(Color::DarkGray)))));
    items.push(ListItem::new(Line::from(Span::styled("  Press Enter to open full News reader", Style::default().fg(Color::Cyan)))));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)).title(" Arch Linux News "));
    
    // We center the list block by shrinking it to a fixed width
    let layout = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(15), Constraint::Percentage(70), Constraint::Percentage(15)].as_ref()).split(area);
    f.render_widget(list, layout[1]);
}
