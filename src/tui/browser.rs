use super::{App, InputMode, PackageFilter};
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

    let filter_display = match app.filter {
        PackageFilter::All => "all",
        PackageFilter::Installed => "installed",
        PackageFilter::NotInstalled => "not installed",
    };

    let search_display = format!(" search (/): {}{} ", app.search_query, cursor);
    let search_bar = Paragraph::new(search_display)
        .style(Style::default().fg(border_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title_bottom(format!(" [filter: {}] (tab) ", filter_display))
                .title_alignment(Alignment::Right),
        );
    f.render_widget(search_bar, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(chunks[1]);

    let items: Vec<ListItem> = app
        .filtered_packages
        .iter()
        .map(|pkg| {
            let (icon, color) = if pkg.is_installed {
                ("✓", Color::Green)
            } else {
                (" ", Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(pkg.name.clone(), Style::default().fg(color)),
            ]))
        })
        .collect();

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
            let details_text = vec![
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
                Line::from(format!("repository: {}", selected_pkg.repo)),
                Line::from(""),
                Line::from(Span::styled(
                    "description:",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(selected_pkg.desc.clone()),
                Line::from(""),
                Line::from(Span::styled(
                    "[i] install  [r] remove  [u] update",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

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
