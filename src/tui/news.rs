use super::{App, InputMode, NewsFocus};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(4),
            ]
            .as_ref(),
        )
        .split(f.area());

    render_search_bar(f, app, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(chunks[1]);

    render_headlines(f, app, content_chunks[0]);
    render_article(f, app, content_chunks[1]);
    render_footer(f, app, chunks[2]);
}

fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let (border_color, cursor) = match app.input_mode {
        InputMode::Editing => (Color::Cyan, "█"),
        InputMode::Normal => (Color::DarkGray, ""),
    };

    let search_display = if app.news_search_query.is_empty() {
        format!("search (/):  {} ", cursor)
    } else {
        format!("search (/): {}{} ", app.news_search_query, cursor)
    };

    let search_bar = Paragraph::new(search_display)
        .style(Style::default().fg(border_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );
    f.render_widget(search_bar, area);
}

fn render_headlines(f: &mut Frame, app: &mut App, area: Rect) {
    let page_size = super::NEWS_PAGE_SIZE;
    let start_idx = (app.news_page.saturating_sub(1)) * page_size;
    let end_idx = (start_idx + page_size).min(app.filtered_news.len());

    let page_items = if start_idx < app.filtered_news.len() {
        &app.filtered_news[start_idx..end_idx]
    } else {
        &[]
    };

    let items: Vec<ListItem> = page_items
        .iter()
        .map(|news| {
            let is_read = app.read_news.contains(&news.link);
            let (prefix, title_color, modifier) = if news.is_critical {
                ("!! ", Color::Red, Modifier::BOLD)
            } else if !is_read {
                ("● ", Color::White, Modifier::BOLD)
            } else {
                ("○ ", Color::DarkGray, Modifier::empty())
            };

            let date_str = chrono::DateTime::parse_from_rfc2822(&news.pub_date)
                .map(|dt| dt.format("published %b %d, %Y").to_string())
                .unwrap_or_else(|_| news.pub_date.clone());

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(title_color).add_modifier(modifier),
                    ),
                    Span::styled(
                        news.title.clone(),
                        Style::default()
                            .fg(if is_read {
                                Color::DarkGray
                            } else {
                                Color::White
                            })
                            .add_modifier(modifier),
                    ),
                ]),
                Line::from(Span::styled(
                    format!("  {}", date_str),
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
            ])
        })
        .collect();

    let is_focused = app.news_focus == NewsFocus::List;
    let list_border = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let mut title = if is_focused {
        " headlines ● ".to_string()
    } else {
        " headlines ".to_string()
    };
    if app.news_last_updated == "cached" {
        title = format!(" headlines (cached) {} ", if is_focused { "●" } else { "" });
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(list_border))
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut app.news_list_state);
}

fn render_article(f: &mut Frame, app: &mut App, area: Rect) {
    let is_focused = app.news_focus == NewsFocus::Article;
    let article_border = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    if app.is_fetching_news {
        render_loading(f, area, article_border);
        return;
    }

    if !app.news_error.is_empty() && app.filtered_news.is_empty() {
        let err = Paragraph::new(format!(
            "\n\n  error: {}\n\n  press r to retry",
            app.news_error
        ))
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(article_border))
                .title(" reading "),
        );
        f.render_widget(err, area);
        return;
    }

    if app.filtered_news.is_empty() {
        render_empty_state(f, area, article_border);
        return;
    }

    if let Some(selected_idx) = app.news_list_state.selected() {
        let page_size = super::NEWS_PAGE_SIZE;
        let actual_idx = (app.news_page.saturating_sub(1)) * page_size + selected_idx;
        if let Some(article) = app.filtered_news.get(actual_idx) {
            let total = app.filtered_news.len();
            let title = if is_focused {
                format!(" reading ● {}/{} ", actual_idx + 1, total)
            } else {
                format!(" reading • {}/{} ", actual_idx + 1, total)
            };

            let query = app.news_search_query.clone();
            let article_lines = format_article(article, &query);

            let visible_height = area.height.saturating_sub(2) as usize;
            let max_scroll = article_lines.len().saturating_sub(visible_height);
            app.news_scroll = app.news_scroll.min(max_scroll as u16);

            let scroll_pct = if max_scroll == 0 {
                100
            } else {
                ((app.news_scroll as f32 / max_scroll as f32) * 100.0) as u16
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(article_border))
                .title(title)
                .title_bottom(format!(" {}% ", scroll_pct))
                .title_alignment(Alignment::Right);

            let paragraph = Paragraph::new(article_lines)
                .wrap(Wrap { trim: false })
                .scroll((app.news_scroll, 0))
                .block(block);
            f.render_widget(paragraph, area);
        }
    }
}

fn format_article<'a>(article: &super::NewsItem, search_query: &str) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    if article.is_critical
        && article
            .description
            .to_lowercase()
            .contains("manual intervention")
    {
        lines.push(Line::from(Span::styled(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " ⚠ manual intervention required",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " read carefully before updating your system.",
            Style::default().fg(Color::White),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(""));
    }

    let date_str = chrono::DateTime::parse_from_rfc2822(&article.pub_date)
        .map(|dt| dt.format("%b %d, %Y").to_string())
        .unwrap_or_else(|_| article.pub_date.clone());

    lines.push(Line::from(Span::styled(
        article.title.clone(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        "──────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "published",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        date_str,
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    let mut in_code = false;
    for raw_line in article.description.lines() {
        let mut current_text = raw_line.to_string();

        if current_text.contains("<code>") {
            in_code = true;
            current_text = current_text.replace("<code>", "");
        }
        if current_text.contains("</code>") {
            in_code = false;
            current_text = current_text.replace("</code>", "");
        }

        if is_command(&current_text) {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "────────────────────────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "command",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            let mut parts = current_text.splitn(2, ' ');
            let cmd = parts.next().unwrap_or("");
            let args = parts.next().unwrap_or("");

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}", cmd),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", args), Style::default().fg(Color::Gray)),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "────────────────────────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        } else {
            let style = if in_code {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Gray)
            };
            let spans = highlight_search(&current_text, search_query, style);
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn highlight_search<'a>(text: &str, query: &str, base_style: Style) -> Vec<Span<'a>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }
    let mut spans = Vec::new();
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let mut last_idx = 0;

    for (idx, _) in text_lower.match_indices(&query_lower) {
        if idx > last_idx {
            spans.push(Span::styled(text[last_idx..idx].to_string(), base_style));
        }
        spans.push(Span::styled(
            text[idx..idx + query.len()].to_string(),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ));
        last_idx = idx + query.len();
    }
    if last_idx < text.len() {
        spans.push(Span::styled(text[last_idx..].to_string(), base_style));
    }
    spans
}

fn is_command(line: &str) -> bool {
    let l = line.trim();
    l.starts_with("pacman ")
        || l.starts_with("systemctl ")
        || l.starts_with("mkinitcpio ")
        || l.starts_with("grub-install ")
        || l.starts_with("chown ")
}

fn render_loading(f: &mut Frame, area: Rect, border: Color) {
    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from("  fetching latest arch news…"),
        Line::from(""),
        Line::from(Span::styled(
            "  ⠋ downloading feed",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  ⠙ parsing articles",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  ⠹ building cache",
            Style::default().fg(Color::Cyan),
        )),
    ];
    let loading = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border))
            .title(" reading "),
    );
    f.render_widget(loading, area);
}

fn render_empty_state(f: &mut Frame, area: Rect, border: Color) {
    let empty = Paragraph::new("\n\n  no articles matched\n\n  press esc to clear search")
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border))
                .title(" reading "),
        );
    f.render_widget(empty, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let unread_count = app
        .news_items
        .iter()
        .filter(|n| !app.read_news.contains(&n.link))
        .count();
    let show_action =
        app.current_action.starts_with("copied ") || app.current_action.starts_with("no command ");

    let footer_spans = if show_action {
        vec![Span::styled(
            app.current_action.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![Span::styled(
            " j/k/g/G nav • ] next page • [ prev page • / search • tab focus • r refresh • y copy link • c copy command • o browser • esc back ",
            Style::default().fg(Color::DarkGray),
        )]
    };
    let top_line = Line::from(footer_spans);

    let divider = Line::from(Span::styled(
        "───────────────────────────────────────────────────────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    ));

    let last_up = if app.news_last_updated == "cached" {
        "cached"
    } else {
        &app.news_last_updated
    };

    let page_size = super::NEWS_PAGE_SIZE;
    let total_count = if app.news_search_query.is_empty() {
        app.news_total_count
    } else {
        app.filtered_news.len()
    };
    let max_pages = total_count.div_ceil(page_size);
    let start_idx = (app.news_page.saturating_sub(1)) * page_size + 1;
    let end_idx = if app.news_search_query.is_empty() {
        (start_idx + page_size - 1).min(app.news_total_count)
    } else {
        (start_idx + page_size - 1).min(app.filtered_news.len())
    };

    let page_info = if app.filtered_news.is_empty() && !app.news_search_query.is_empty() {
        "0 articles".to_string()
    } else {
        format!(
            "page {}/{} ({}-{} of {})",
            app.news_page,
            max_pages.max(1),
            start_idx,
            end_idx,
            total_count
        )
    };

    let bottom_line = Line::from(vec![
        Span::styled(page_info, Style::default().fg(Color::White)),
        Span::styled("    updated ", Style::default().fg(Color::DarkGray)),
        Span::styled(last_up, Style::default().fg(Color::White)),
        Span::styled(
            format!("    {} unread ", unread_count),
            Style::default().fg(if unread_count > 0 {
                Color::Cyan
            } else {
                Color::DarkGray
            }),
        ),
    ]);

    let paragraph =
        Paragraph::new(vec![top_line, divider, bottom_line]).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}
