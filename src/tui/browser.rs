use super::{App, InputMode};
use crate::tui::{BrowserCache, BrowserTab};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};

const COLOR_AUR: Color = Color::Magenta;
const COLOR_CORE: Color = Color::Cyan;
const COLOR_EXTRA: Color = Color::Green;
const COLOR_MULTI: Color = Color::Yellow;
const COLOR_HEADING: Color = Color::LightMagenta;
const COLOR_TEXT: Color = Color::White;
const COLOR_MUTED: Color = Color::DarkGray;

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(4),
                Constraint::Min(0),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.area());

    render_header(f, app, chunks[0]);

    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(chunks[1]);

    render_list(f, app, body_chunks[0]);
    render_details(f, app, body_chunks[1]);
    render_footer(f, app, chunks[2]);
}

fn render_header(f: &mut Frame, app: &mut App, area: Rect) {
    let (border_color, cursor) = match app.input_mode {
        InputMode::Editing => (Color::Cyan, "█"),
        InputMode::Normal => (COLOR_MUTED, ""),
    };

    let search_line = Line::from(vec![
        Span::styled(
            " search (/): ",
            Style::default()
                .fg(COLOR_HEADING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}{}", app.search_query, cursor),
            Style::default().fg(COLOR_TEXT),
        ),
    ]);

    let repo_line = Line::from(vec![
        Span::styled(
            " filter: ",
            Style::default()
                .fg(COLOR_HEADING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} (tab)", app.filters[app.filter_idx]),
            Style::default().fg(Color::Cyan),
        ),
    ]);

    let sort_line = Line::from(vec![
        Span::styled(
            " sort:   ",
            Style::default()
                .fg(COLOR_HEADING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} (S)", app.sort_mode),
            Style::default().fg(Color::Cyan),
        ),
    ]);

    let header_block = Paragraph::new(vec![search_line, repo_line, sort_line]).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(border_color)),
    );

    f.render_widget(header_block, area);
}

fn render_list(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .filtered_packages
        .iter()
        .map(|pkg| {
            let is_queued = app.is_package_selected(&pkg.name);
            let queue_indicator = if is_queued {
                Span::styled(" ■ ", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("   ")
            };

            let status_icon = if pkg.is_upgradable {
                Span::styled(
                    "↑ ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else if pkg.is_installed {
                Span::styled(
                    "✓ ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("○ ", Style::default().fg(COLOR_MUTED))
            };

            let repo_color = match pkg.repo.as_str() {
                "core" => COLOR_CORE,
                "extra" => COLOR_EXTRA,
                "multilib" => COLOR_MULTI,
                "local/aur" | "aur" => COLOR_AUR,
                _ => COLOR_TEXT,
            };

            ListItem::new(Line::from(vec![
                queue_indicator,
                status_icon,
                Span::styled(format!("{:<30}", pkg.name), Style::default().fg(repo_color)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_MUTED))
                .title(format!(" packages ({}) ", app.filtered_packages.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_details(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(area);

    let tab_titles = vec![
        Line::from(Span::styled(
            "[1] overview",
            Style::default().fg(COLOR_TEXT),
        )),
        Line::from(Span::styled(
            "[2] dependencies",
            Style::default().fg(COLOR_TEXT),
        )),
        Line::from(Span::styled("[3] files", Style::default().fg(COLOR_TEXT))),
        Line::from(Span::styled("[4] queue", Style::default().fg(COLOR_TEXT))),
    ];

    let tab_idx = match app.browser_tab {
        BrowserTab::Overview => 0,
        BrowserTab::Dependencies => 1,
        BrowserTab::Files => 2,
        BrowserTab::Queue => 3,
    };

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_MUTED)),
        )
        .select(tab_idx)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled(" | ", Style::default().fg(COLOR_MUTED)));

    f.render_widget(tabs, chunks[0]);

    if app.browser_tab == BrowserTab::Queue {
        render_queue_tab(f, app, chunks[1]);
        return;
    }

    if let Some(idx) = app.list_state.selected()
        && let Some(pkg) = app.filtered_packages.get(idx)
    {
        if let Some(cache) = app.browser_caches.get(&pkg.name) {
            if cache.loading_info && app.browser_tab != BrowserTab::Files {
                let loading_ui = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  fetching package metadata...",
                        Style::default().fg(Color::Cyan),
                    )),
                    Line::from("  ██████░░░░░"),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(COLOR_MUTED)),
                );
                f.render_widget(loading_ui, chunks[1]);
            } else if cache.loading_files && app.browser_tab == BrowserTab::Files {
                let loading_ui = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  reading file list...",
                        Style::default().fg(Color::Cyan),
                    )),
                    Line::from("  ██████░░░░░"),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(COLOR_MUTED)),
                );
                f.render_widget(loading_ui, chunks[1]);
            } else {
                match app.browser_tab {
                    BrowserTab::Overview => render_overview_tab(f, app, pkg, cache, chunks[1]),
                    BrowserTab::Dependencies => render_deps_tab(f, app, cache, chunks[1]),
                    BrowserTab::Files => render_files_tab(f, app, cache, chunks[1]),
                    _ => {}
                }
            }
        } else {
            f.render_widget(
                Paragraph::new("waiting for cache...").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(COLOR_MUTED)),
                ),
                chunks[1],
            );
        }
    }
}

fn render_overview_tab(
    f: &mut Frame,
    app: &App,
    pkg: &crate::core::package::PackageModel,
    cache: &BrowserCache,
    area: Rect,
) {
    let mut details = vec![
        Line::from(vec![
            Span::styled(
                pkg.name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" v{}", pkg.version),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
    ];

    for line in cache.info.lines() {
        if let Some((key, val)) = line.split_once(':') {
            let key_str = key.trim();
            if key_str == "depends on" || key_str == "optional deps" || key_str == "required by" {
                continue;
            }
            details.push(Line::from(vec![
                Span::styled(format!("{key_str:<18}"), Style::default().fg(COLOR_HEADING)),
                Span::raw(format!(": {}", val.trim())),
            ]));
        }
    }

    let start = app.browser_scroll as usize;
    let end = (start + area.height.saturating_sub(2) as usize).min(details.len());
    let visible = if start < details.len() {
        details[start..end].to_vec()
    } else {
        vec![]
    };

    f.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_MUTED)),
        ),
        area,
    );
}

fn render_deps_tab(f: &mut Frame, app: &App, cache: &BrowserCache, area: Rect) {
    let mut details = vec![];
    for dep in &cache.dependencies {
        details.push(Line::from(vec![
            Span::styled("  ↳ ", Style::default().fg(COLOR_MUTED)),
            Span::styled(dep.clone(), Style::default().fg(COLOR_TEXT)),
        ]));
    }

    if details.is_empty() {
        details.push(Line::from(Span::styled(
            "no dependency information available.",
            Style::default().fg(COLOR_MUTED),
        )));
    }

    let start = app.browser_scroll as usize;
    let end = (start + area.height.saturating_sub(2) as usize).min(details.len());
    let visible = if start < details.len() {
        details[start..end].to_vec()
    } else {
        vec![]
    };

    f.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_MUTED)),
        ),
        area,
    );
}

fn render_files_tab(f: &mut Frame, app: &App, cache: &BrowserCache, area: Rect) {
    let start = app.browser_scroll as usize;
    let end = (start + area.height.saturating_sub(2) as usize).min(cache.files.len());

    let visible_lines: Vec<Line> = if start < cache.files.len() {
        cache.files[start..end]
            .iter()
            .map(|f| Line::from(Span::raw(f.clone())))
            .collect()
    } else {
        vec![]
    };

    f.render_widget(
        Paragraph::new(visible_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_MUTED)),
        ),
        area,
    );
}

fn render_queue_tab(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  install / remove queue",
            Style::default()
                .fg(COLOR_HEADING)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let selected_count = app.get_selected_count();

    if selected_count == 0 {
        lines.push(Line::from(Span::styled(
            "  queue is empty.",
            Style::default().fg(COLOR_MUTED),
        )));
    } else {
        use crate::tui::SelectionMode;

        lines.push(Line::from(vec![
            Span::styled("  mode: ", Style::default().fg(COLOR_HEADING)),
            Span::raw(if app.selection_mode == SelectionMode::AllVisible {
                "all visible"
            } else {
                "explicit"
            }),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  packages: ", Style::default().fg(COLOR_HEADING)),
            Span::raw(selected_count.to_string()),
        ]));
        lines.push(Line::from(""));

        if app.selection_mode == SelectionMode::AllVisible {
            if app.deselected_packages.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  no exceptions.",
                    Style::default().fg(COLOR_MUTED),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "  exceptions:",
                    Style::default().fg(COLOR_HEADING),
                )));
                for name in app.deselected_packages.iter().take(50) {
                    lines.push(Line::from(vec![
                        Span::styled("  • ", Style::default().fg(Color::Red)),
                        Span::raw(name.clone()),
                    ]));
                }
                if app.deselected_packages.len() > 50 {
                    lines.push(Line::from(Span::styled(
                        format!("  ... and {} more", app.deselected_packages.len() - 50),
                        Style::default().fg(COLOR_MUTED),
                    )));
                }
            }
        } else {
            for name in app.selected_packages.iter().take(50) {
                lines.push(Line::from(vec![
                    Span::styled("  ■ ", Style::default().fg(Color::Yellow)),
                    Span::raw(name.clone()),
                ]));
            }
            if app.selected_packages.len() > 50 {
                lines.push(Line::from(Span::styled(
                    format!("  ... and {} more", app.selected_packages.len() - 50),
                    Style::default().fg(COLOR_MUTED),
                )));
            }
        }
    }

    let start = app.browser_scroll as usize;
    let end = (start + area.height.saturating_sub(2) as usize).min(lines.len());
    let visible = if start < lines.len() {
        lines[start..end].to_vec()
    } else {
        vec![]
    };

    f.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_MUTED)),
        ),
        area,
    );
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let stats = format!(
        " total: {} | queued: {} | updates: {} ",
        app.package_list.len(),
        app.get_selected_count(),
        app.updates_count
    );
    let keys = " j/k/g/G:nav • x:del • ctrl-u/d:scroll • space:toggle • a:all/none • i:install • r:remove ";
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    f.render_widget(
        Paragraph::new(Span::styled(keys, Style::default().fg(COLOR_MUTED)))
            .alignment(Alignment::Left),
        layout[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled(stats, Style::default().fg(Color::Cyan)))
            .alignment(Alignment::Right),
        layout[1],
    );
}
