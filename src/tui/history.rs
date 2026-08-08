use crate::tui::{App, CurrentScreen, HistoryFilter, InputMode, TxAction};
use chrono::Datelike;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

const COLOR_INSTALL: Color = Color::Green;
const COLOR_UPGRADE: Color = Color::Cyan;
const COLOR_REMOVE: Color = Color::Red;
const COLOR_WARNING: Color = Color::Yellow;
const COLOR_GRAY: Color = Color::DarkGray;

pub fn render(f: &mut Frame, app: &mut App) {
    if app.history_expanded {
        render_expanded(f, app);
        return;
    }

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // stats header
                Constraint::Length(3), // search bar
                Constraint::Min(0),    // split layout
                Constraint::Length(3), // footer
            ]
            .as_ref(),
        )
        .split(f.area());

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today_count = app
        .transactions
        .iter()
        .filter(|t| t.timestamp.starts_with(&today))
        .count();
    let installs = app
        .transactions
        .iter()
        .filter(|t| t.primary_action == TxAction::Install)
        .count();
    let upgrades = app
        .transactions
        .iter()
        .filter(|t| t.primary_action == TxAction::Upgrade)
        .count();
    let removals = app
        .transactions
        .iter()
        .filter(|t| t.primary_action == TxAction::Remove)
        .count();

    let stats_line = Line::from(vec![
        Span::styled(
            format!(" transactions: {} ", app.transactions.len()),
            Style::default().fg(Color::White),
        ),
        Span::styled(" | ", Style::default().fg(COLOR_GRAY)),
        Span::styled(
            format!(" today: {today_count} "),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled(" | ", Style::default().fg(COLOR_GRAY)),
        Span::styled(
            format!(" installs: {installs} "),
            Style::default().fg(COLOR_INSTALL),
        ),
        Span::styled(" | ", Style::default().fg(COLOR_GRAY)),
        Span::styled(
            format!(" upgrades: {upgrades} "),
            Style::default().fg(COLOR_UPGRADE),
        ),
        Span::styled(" | ", Style::default().fg(COLOR_GRAY)),
        Span::styled(
            format!(" removals: {removals} "),
            Style::default().fg(COLOR_REMOVE),
        ),
    ]);
    f.render_widget(
        Paragraph::new(stats_line)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" history ")
                    .border_style(Style::default().fg(COLOR_GRAY)),
            )
            .alignment(Alignment::Center),
        main_chunks[0],
    );

    let (border_color, cursor) = match app.history_input_mode {
        InputMode::Editing => (Color::Cyan, "█"),
        InputMode::Normal => (COLOR_GRAY, ""),
    };
    let search_display = format!(" search (/): {}{} ", app.history_search_query, cursor);

    let filter_str = match app.history_filter {
        HistoryFilter::All => "all",
        HistoryFilter::Installs => "installs",
        HistoryFilter::Upgrades => "upgrades",
        HistoryFilter::Removals => "removals",
        HistoryFilter::Failures => "failures",
    };

    f.render_widget(
        Paragraph::new(search_display)
            .style(Style::default().fg(border_color))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .title_bottom(format!(" [filter: {filter_str} (tab)] "))
                    .title_alignment(Alignment::Right),
            ),
        main_chunks[1],
    );

    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(main_chunks[2]);

    if app.filtered_transactions.is_empty() {
        f.render_widget(
            Paragraph::new("no matching transactions found.")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL)),
            split[0],
        );
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title(" transaction details "),
            split[1],
        );
    } else {
        let items: Vec<ListItem> = app
            .filtered_transactions
            .iter()
            .map(|tx| {
                let (icon, label, color) = match tx.primary_action {
                    TxAction::Install => ("✓", "install", COLOR_INSTALL),
                    TxAction::Upgrade => ("▲", "upgrade", COLOR_UPGRADE),
                    TxAction::Remove => ("✗", "remove", COLOR_REMOVE),
                    _ => ("○", "transaction", Color::White),
                };

                let status_icon = if tx.is_success {
                    Span::raw("")
                } else {
                    Span::styled(" !", Style::default().fg(COLOR_WARNING))
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!(" {icon} {label} "),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("({} pkgs)", tx.packages.len()),
                            Style::default().fg(COLOR_GRAY),
                        ),
                        status_icon,
                    ]),
                    Line::from(Span::styled(
                        format!("   {}", format_relative_timestamp(&tx.timestamp)),
                        Style::default().fg(COLOR_GRAY),
                    )),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_stateful_widget(list, split[0], &mut app.history_state);

        if let Some(idx) = app.history_state.selected()
            && let Some(tx) = app.filtered_transactions.get(idx)
        {
            let mut details = vec![
                Line::from(Span::styled(
                    match tx.primary_action {
                        TxAction::Install => "install",
                        TxAction::Upgrade => "upgrade",
                        TxAction::Remove => "remove",
                        _ => "system transaction",
                    },
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "──────────────────────────────────────────────",
                    Style::default().fg(COLOR_GRAY),
                )),
                Line::from(vec![
                    Span::styled("date      ", Style::default().fg(Color::LightMagenta)),
                    Span::raw(format_relative_timestamp(&tx.timestamp)),
                ]),
                Line::from(vec![
                    Span::styled("result    ", Style::default().fg(Color::LightMagenta)),
                    Span::styled(
                        if tx.is_success { "success" } else { "failed" },
                        Style::default().fg(if tx.is_success {
                            COLOR_INSTALL
                        } else {
                            COLOR_WARNING
                        }),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "packages",
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                )),
            ];

            for pkg in &tx.packages {
                let icon = match pkg.action {
                    TxAction::Install => Span::styled("  ✓ ", Style::default().fg(COLOR_INSTALL)),
                    TxAction::Upgrade => Span::styled("  ↑ ", Style::default().fg(COLOR_UPGRADE)),
                    TxAction::Remove => Span::styled("  ✗ ", Style::default().fg(COLOR_REMOVE)),
                    _ => Span::raw("  - "),
                };

                details.push(Line::from(vec![
                    icon,
                    Span::styled(pkg.name.clone(), Style::default().fg(Color::White)),
                ]));

                if let Some(old_v) = &pkg.old_version {
                    details.push(Line::from(vec![
                        Span::styled(format!("      {old_v} "), Style::default().fg(COLOR_GRAY)),
                        Span::styled("→ ", Style::default().fg(COLOR_UPGRADE)),
                        Span::styled(pkg.new_version.clone(), Style::default().fg(COLOR_INSTALL)),
                    ]));
                } else {
                    details.push(Line::from(Span::styled(
                        format!("      {}", pkg.new_version),
                        Style::default().fg(COLOR_GRAY),
                    )));
                }
            }

            if !tx.warnings.is_empty() {
                details.push(Line::from(""));
                details.push(Line::from(Span::styled(
                    "warnings",
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                )));
                for w in &tx.warnings {
                    details.push(Line::from(vec![
                        Span::styled("  ! ", Style::default().fg(COLOR_WARNING)),
                        Span::raw(w),
                    ]));
                }
            }

            if !tx.hooks.is_empty() {
                details.push(Line::from(""));
                details.push(Line::from(Span::styled(
                    "hooks",
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                )));
                for h in tx.hooks.iter().take(5) {
                    details.push(Line::from(Span::styled(
                        format!("  {h}"),
                        Style::default().fg(COLOR_GRAY),
                    )));
                }
                if tx.hooks.len() > 5 {
                    details.push(Line::from(Span::styled(
                        format!("  ... and {} more", tx.hooks.len() - 5),
                        Style::default().fg(COLOR_GRAY),
                    )));
                }
            }

            f.render_widget(
                Paragraph::new(details)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" execution details "),
                    )
                    .wrap(Wrap { trim: true }),
                split[1],
            );
        }
    }

    let footer = Paragraph::new(Span::styled(
        " j/k/g/G nav • enter expand • / search • tab filter • r reload • esc back • q quit ",
        Style::default().fg(COLOR_GRAY),
    ))
    .alignment(Alignment::Center);
    f.render_widget(footer, main_chunks[3]);
}

fn render_expanded(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(f.area());

    if let Some(idx) = app.history_state.selected()
        && let Some(tx) = app.filtered_transactions.get(idx)
    {
        let lines: Vec<Line> = tx
            .raw_log
            .iter()
            .map(|l| Line::from(Span::raw(l)))
            .collect();
        f.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(format!(" raw log: {} ", tx.timestamp)),
            ),
            chunks[0],
        );
    }

    f.render_widget(
        Paragraph::new(Span::styled(" esc back ", Style::default().fg(COLOR_GRAY)))
            .alignment(Alignment::Center),
        chunks[1],
    );
}

pub fn handle_key(key: crossterm::event::KeyEvent, app: &mut App) {
    if app.history_expanded {
        if let KeyCode::Esc | KeyCode::Enter = key.code {
            app.history_expanded = false;
        }
        return;
    }

    match app.history_input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('/') => app.history_input_mode = InputMode::Editing,
            KeyCode::Char('r') => app.refresh_state(),
            KeyCode::Enter => app.history_expanded = true,
            KeyCode::Tab => {
                app.history_filter = match app.history_filter {
                    HistoryFilter::All => HistoryFilter::Installs,
                    HistoryFilter::Installs => HistoryFilter::Upgrades,
                    HistoryFilter::Upgrades => HistoryFilter::Removals,
                    HistoryFilter::Removals => HistoryFilter::Failures,
                    HistoryFilter::Failures => HistoryFilter::All,
                };
                app.update_history_filter();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !app.filtered_transactions.is_empty() {
                    let i = match app.history_state.selected() {
                        Some(i) => {
                            if i >= app.filtered_transactions.len() - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    app.history_state.select(Some(i));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !app.filtered_transactions.is_empty() {
                    let i = match app.history_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                app.filtered_transactions.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    app.history_state.select(Some(i));
                }
            }
            KeyCode::Char('g') => {
                if !app.filtered_transactions.is_empty() {
                    app.history_state.select(Some(0));
                }
            }
            KeyCode::Char('G') if !app.filtered_transactions.is_empty() => {
                app.history_state
                    .select(Some(app.filtered_transactions.len() - 1));
            }
            _ => {}
        },
        InputMode::Editing => match key.code {
            KeyCode::Esc | KeyCode::Enter => app.history_input_mode = InputMode::Normal,
            KeyCode::Backspace => {
                app.history_search_query.pop();
                app.update_history_filter();
            }
            KeyCode::Char(c) => {
                app.history_search_query.push(c);
                app.update_history_filter();
            }
            _ => {}
        },
    }
}

fn format_relative_timestamp(ts: &str) -> String {
    if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M") {
        let local_dt = chrono::Local::now();
        let today = local_dt.date_naive();
        let tx_date = naive_dt.date();

        if tx_date == today {
            format!("today at {}", naive_dt.format("%H:%M"))
        } else if tx_date == today.pred_opt().unwrap_or(today) {
            format!("yesterday at {}", naive_dt.format("%H:%M"))
        } else if tx_date.year() == today.year() {
            naive_dt.format("%b %d, %H:%M").to_string()
        } else {
            naive_dt.format("%b %d, %Y").to_string()
        }
    } else {
        ts.to_string()
    }
}
