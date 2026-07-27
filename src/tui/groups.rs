use crate::tui::{App, CurrentScreen, InputMode};
use crate::tui::GroupSortMode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crossterm::event::KeyCode;

pub fn render(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Statistics Header
            Constraint::Length(3), // Search Bar
            Constraint::Min(0),    // Split Layout
            Constraint::Length(3), // Footer
        ].as_ref())
        .split(f.area());

    // 1. Statistics Header
    let _total_groups = app.groups.len();
    let installed_groups = app.groups.iter().filter(|g| !g.packages.is_empty() && g.packages.iter().all(|p| p.1)).count();
    let partial_groups = app.groups.iter().filter(|g| {
        let inst = g.packages.iter().filter(|p| p.1).count();
        inst > 0 && inst < g.packages.len()
    }).count();
    let total_pkgs_in_groups: usize = app.groups.iter().map(|g| g.packages.len()).sum();

    let stats_line = Line::from(vec![
        Span::styled(format!(" Groups Installed : {} ", installed_groups + partial_groups), Style::default().fg(Color::Green)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" Packages in Groups : {} ", total_pkgs_in_groups), Style::default().fg(Color::Cyan)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" Fully Installed : {} ", installed_groups), Style::default().fg(Color::Green)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" Partial : {} ", partial_groups), Style::default().fg(Color::Yellow)),
    ]);
    f.render_widget(
        Paragraph::new(stats_line).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray))).alignment(Alignment::Center),
        main_chunks[0]
    );

    // 2. Search Bar
    let (border_color, cursor) = match app.group_input_mode {
        InputMode::Editing => (Color::Cyan, "█"),
        InputMode::Normal => (Color::DarkGray, ""),
    };
    let search_display = format!(" Search (/): {}{} ", app.group_search_query, cursor);
    f.render_widget(
        Paragraph::new(search_display).style(Style::default().fg(border_color)).block(
            Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color))
                .title_bottom(format!(" [sort: a=alpha, p=count, i=completion (S)] "))
                .title_alignment(Alignment::Right)
        ),
        main_chunks[1]
    );

    // 3. Split Layout
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(main_chunks[2]);

    if app.filtered_groups.is_empty() {
        let empty_msg = vec![
            Line::from(""),
            Line::from(Span::styled("      /\\_/\\ ", Style::default().fg(Color::Magenta))),
            Line::from(Span::styled("     ( •.• )", Style::default().fg(Color::Magenta))),
            Line::from(Span::styled("     > 📦 < ", Style::default().fg(Color::Magenta))),
            Line::from(""),
            Line::from(Span::styled("No matching package groups.", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("Press Esc to clear search.", Style::default().fg(Color::DarkGray))),
        ];
        f.render_widget(Paragraph::new(empty_msg).block(Block::default().borders(Borders::ALL).title(" Groups ")).alignment(Alignment::Center), split[0]);
        f.render_widget(Paragraph::new("").block(Block::default().borders(Borders::ALL).title(" Group Information ")), split[1]);
    } else {
        let items: Vec<ListItem> = app.filtered_groups.iter().map(|group| {
            let total = group.packages.len();
            let installed = group.packages.iter().filter(|p| p.1).count();
            
            let status_color = if installed == total && total > 0 { Color::Green }
                               else if installed > 0 { Color::Yellow }
                               else { Color::Red };

            let fav_icon = if group.is_favorite { "★ " } else { "📦 " };

            ListItem::new(Line::from(vec![
                Span::styled(fav_icon, Style::default().fg(if group.is_favorite { Color::Yellow } else { Color::Cyan })),
                Span::styled(format!("{:<15}", group.name), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:>5}", total), Style::default().fg(Color::DarkGray)),
            ]))
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(" Groups ({}) ", app.filtered_groups.len())))
            .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, split[0], &mut app.group_state);

        if let Some(idx) = app.group_state.selected() {
            if let Some(group) = app.filtered_groups.get(idx) {
                let total = group.packages.len();
                let installed = group.packages.iter().filter(|p| p.1).count();
                let pct = if total > 0 { (installed as f64 / total as f64) * 100.0 } else { 0.0 };

                let mut details = vec![
                    Line::from(vec![
                        Span::styled(group.name.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled(if group.is_favorite { " ★ Favorite" } else { "" }, Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("Statistics", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD))),
                    Line::from(format!("  Packages   : {}", total)),
                    Line::from(format!("  Installed  : {} / {}", installed, total)),
                    Line::from(format!("  Completion : {:.0}%", pct)),
                    Line::from(format!("  Progress   : {}", draw_bar(pct, 25))),
                    Line::from(""),
                    Line::from(Span::styled("Description", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD))),
                    Line::from(format!("  {}", group.description)),
                    Line::from(""),
                    Line::from(Span::styled("Package Preview (First 20)", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD))),
                ];

                for (pkg, is_inst) in group.packages.iter().take(20) {
                    let icon = if *is_inst { "✓ " } else { "○ " };
                    let color = if *is_inst { Color::Green } else { Color::DarkGray };
                    details.push(Line::from(vec![
                        Span::styled(format!("  {}", icon), Style::default().fg(color)),
                        Span::styled(pkg.clone(), Style::default().fg(color)),
                    ]));
                }

                if group.packages.len() > 20 {
                    details.push(Line::from(Span::styled(format!("  ... and {} more packages", group.packages.len() - 20), Style::default().fg(Color::DarkGray))));
                }

                let info_block = Paragraph::new(details)
                    .block(Block::default().borders(Borders::ALL).title(" Group Information "))
                    .wrap(Wrap { trim: true });

                f.render_widget(info_block, split[1]);
            }
        }
    }

    let footer = Paragraph::new(Span::styled(" j/k Move • Enter Browse • / Search • i Install • r Remove • Space Favorite • Esc Back ", Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center);
    f.render_widget(footer, main_chunks[3]);
}

fn draw_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

pub fn handle_key(key: crossterm::event::KeyEvent, app: &mut App) {
    match app.group_input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('/') => {
                app.group_input_mode = InputMode::Editing;
            }
            KeyCode::Char(' ') => {
                if let Some(idx) = app.group_state.selected() {
                    if let Some(selected_group) = app.filtered_groups.get(idx) {
                        let name = selected_group.name.clone();
                        if let Some(g) = app.groups.iter_mut().find(|x| x.name == name) {
                            g.is_favorite = !g.is_favorite;
                        }
                        app.update_group_filter();
                    }
                }
            }
            KeyCode::Char('S') => {
                app.group_sort_mode = match app.group_sort_mode {
                    GroupSortMode::Alphabetical => GroupSortMode::PackageCount,
                    GroupSortMode::PackageCount => GroupSortMode::InstallCompletion,
                    GroupSortMode::InstallCompletion => GroupSortMode::Alphabetical,
                };
                app.update_group_filter();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !app.filtered_groups.is_empty() {
                    let i = match app.group_state.selected() {
                        Some(i) => if i >= app.filtered_groups.len() - 1 { 0 } else { i + 1 },
                        None => 0,
                    };
                    app.group_state.select(Some(i));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !app.filtered_groups.is_empty() {
                    let i = match app.group_state.selected() {
                        Some(i) => if i == 0 { app.filtered_groups.len() - 1 } else { i - 1 },
                        None => 0,
                    };
                    app.group_state.select(Some(i));
                }
            }
            _ => {}
        },
        InputMode::Editing => match key.code {
            KeyCode::Esc | KeyCode::Enter => app.group_input_mode = InputMode::Normal,
            KeyCode::Backspace => {
                app.group_search_query.pop();
                app.update_group_filter();
            }
            KeyCode::Char(c) => {
                app.group_search_query.push(c);
                app.update_group_filter();
            }
            _ => {}
        }
    }
}
