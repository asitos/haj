use super::{App, InputMode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.area());

    let (border_color, cursor) = match app.input_mode {
        InputMode::Editing => (Color::Cyan, "█"),
        InputMode::Normal => (Color::DarkGray, ""),
    };

    let filter_display = app.filters[app.filter_idx].to_string();
    let sort_display = app.sort_mode.to_string();

    let search_display = format!(" search (/): {}{} ", app.search_query, cursor);
    let search_bar = Paragraph::new(search_display)
        .style(Style::default().fg(border_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title_bottom(format!(
                    " [sort: {} (S)] [filter: {} (tab)] ",
                    sort_display, filter_display
                ))
                .title_alignment(Alignment::Right),
        );
    f.render_widget(search_bar, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(chunks[1]);

    let items = app.filtered_packages.iter().map(|pkg| {
        let is_queued = app.selected_packages.contains(&pkg.name);
        let queue_icon = if is_queued { "[x] " } else { "[ ] " };
        let queue_color = if is_queued {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let (icon, color) = if pkg.is_upgradable {
            ("↑ ", Color::Cyan)
        } else if pkg.is_installed {
            ("✓ ", Color::Green)
        } else {
            ("  ", Color::White)
        };

        ListItem::new(Line::from(vec![
            Span::styled(queue_icon, Style::default().fg(queue_color)),
            Span::styled(
                icon,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(pkg.name.as_str(), Style::default().fg(color)),
        ]))
    });

    let list_title = format!(" packages ({}) ", app.filtered_packages.len());

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, content_chunks[0], &mut app.list_state);

    if let Some(selected_idx) = app.list_state.selected() {
        if let Some(selected_pkg) = app.filtered_packages.get(selected_idx) {
            let mut details_text = vec![
                Line::from(vec![
                    Span::styled(
                        selected_pkg.name.clone(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" v{}", selected_pkg.version)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        format!("{:<15}", "repository:"),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(selected_pkg.repo.clone()),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("{:<15}", "size:"),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(format!("{:.2} MB", selected_pkg.size_mb)),
                ]),
            ];

            if selected_pkg.is_upgradable {
                details_text.push(Line::from(vec![
                    Span::styled(
                        format!("{:<15}", "status:"),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        "update available ↑",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            details_text.push(Line::from(""));
            details_text.push(Line::from(Span::styled(
                "description:",
                Style::default().fg(Color::DarkGray),
            )));
            details_text.push(Line::from(selected_pkg.desc.clone()));
            details_text.push(Line::from(""));

            if !app.selected_packages.is_empty() {
                details_text.push(Line::from(Span::styled(
                    format!(
                        "queued for transaction: {} packages",
                        app.selected_packages.len()
                    ),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                details_text.push(Line::from(""));
            }

            details_text.push(Line::from(Span::styled(
                "[space] toggle queue  [c] clear queue",
                Style::default().fg(Color::Magenta),
            )));
            details_text.push(Line::from(Span::styled(
                "[i] install queue  [r] remove queue",
                Style::default().fg(Color::DarkGray),
            )));

            let details = Paragraph::new(details_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" details ")
                        .title_bottom(" j/k:nav gg/G:jump f:search x:del esc:back q:quit ")
                        .title_alignment(Alignment::Right),
                )
                .wrap(Wrap { trim: true });

            f.render_widget(details, content_chunks[1]);
        }
    } else {
        let empty_state = Paragraph::new("no packages found.")
            .block(Block::default().borders(Borders::ALL).title(" details "))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty_state, content_chunks[1]);
    }
}
