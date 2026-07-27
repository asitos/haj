use super::{App, InputMode, NewsFocus, RedditItem};
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

    render_post_list(f, app, content_chunks[0]);
    render_post_body(f, app, content_chunks[1]);
    render_footer(f, app, chunks[2]);
}

fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let (border_color, cursor) = match app.input_mode {
        InputMode::Editing => (Color::Cyan, "█"),
        InputMode::Normal => (Color::DarkGray, ""),
    };

    let search_display = if app.reddit_search_query.is_empty() {
        format!(" 🔎 {} ", cursor)
    } else {
        format!(" 🔎 {}{} ", app.reddit_search_query, cursor)
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

fn render_post_list(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .filtered_reddit
        .iter()
        .map(|post| {
            let score_color = if post.score > 500 {
                Color::Yellow
            } else {
                Color::DarkGray
            };
            let mut title_spans = vec![Span::styled(
                format!("▲ {:<4} ", post.score),
                Style::default().fg(score_color),
            )];

            let icon = if post.url.contains("v.redd.it")
                || post.post_hint.as_deref() == Some("hosted:video")
            {
                "🎥 "
            } else if post.post_hint.as_deref() == Some("image")
                || post.url.ends_with(".jpg")
                || post.url.ends_with(".png")
            {
                "📷 "
            } else if post.url.contains("gallery") {
                "🖼  "
            } else if !post.selftext.is_empty() {
                "📰 "
            } else {
                "🔗 "
            };
            title_spans.push(Span::raw(icon));

            if post.pinned {
                title_spans.push(Span::styled(
                    "[PINNED] ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if post.nsfw {
                title_spans.push(Span::styled(
                    "[NSFW] ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            }

            title_spans.push(Span::styled(
                post.title.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));

            let time_str = {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64();
                let diff = now - post.created_utc;
                if diff < 3600.0 {
                    format!("{:.0} mins ago", diff / 60.0)
                } else if diff < 86400.0 {
                    format!("{:.0} hours ago", diff / 3600.0)
                } else {
                    format!("{:.0} days ago", diff / 86400.0)
                }
            };

            ListItem::new(vec![
                Line::from(title_spans),
                Line::from(vec![
                    Span::styled(
                        format!("        u/{:<15}", post.author),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{} • {} comments", time_str, post.num_comments),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(""),
            ])
        })
        .collect();

    let is_focused = app.reddit_focus == NewsFocus::List;
    let list_border = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let mut title = if is_focused {
        " Hot Posts ● ".to_string()
    } else {
        " Hot Posts ".to_string()
    };
    if app.reddit_last_updated == "cached" {
        title = format!(" Hot Posts (cached) {} ", if is_focused { "●" } else { "" });
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

    f.render_stateful_widget(list, area, &mut app.reddit_list_state);
}

fn render_post_body(f: &mut Frame, app: &mut App, area: Rect) {
    let is_focused = app.reddit_focus == NewsFocus::Article;
    let article_border = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    if app.is_fetching_reddit {
        let loading = Paragraph::new(
            "\n\n  Fetching latest posts from r/blahaj…\n  ⠋ Connecting to Reddit API",
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(article_border))
                .title(" Viewing "),
        );
        f.render_widget(loading, area);
        return;
    }

    if !app.reddit_error.is_empty() && app.filtered_reddit.is_empty() {
        let err = Paragraph::new(format!(
            "\n\n  Error: {}\n\n  Press r to retry",
            app.reddit_error
        ))
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(article_border))
                .title(" Viewing "),
        );
        f.render_widget(err, area);
        return;
    }

    if app.filtered_reddit.is_empty() {
        let empty = Paragraph::new("\n\n  No posts matched\n\n  Press Esc to clear search")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(article_border))
                    .title(" Viewing "),
            );
        f.render_widget(empty, area);
        return;
    }

    if let Some(selected_idx) = app.reddit_list_state.selected() {
        if let Some(post) = app.filtered_reddit.get(selected_idx) {
            let total = app.filtered_reddit.len();
            let title = if is_focused {
                format!(" Viewing ● {}/{} ", selected_idx + 1, total)
            } else {
                format!(" Viewing • {}/{} ", selected_idx + 1, total)
            };

            let query = app.reddit_search_query.clone();
            let lines = format_reddit_post(post, &query, app.show_reddit_image);

            let visible_height = area.height.saturating_sub(2) as usize;
            let max_scroll = lines.len().saturating_sub(visible_height);
            app.reddit_scroll = app.reddit_scroll.min(max_scroll as u16);

            let scroll_pct = if max_scroll == 0 {
                100
            } else {
                ((app.reddit_scroll as f32 / max_scroll as f32) * 100.0) as u16
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(article_border))
                .title(title)
                .title_bottom(format!(" {}% ", scroll_pct))
                .title_alignment(Alignment::Right);

            let paragraph = Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((app.reddit_scroll, 0))
                .block(block);
            f.render_widget(paragraph, area);
        }
    }
}

fn format_reddit_post<'a>(
    post: &RedditItem,
    search_query: &str,
    show_image: bool,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    if show_image
        && (post.thumbnail.starts_with("http")
            || post.url.ends_with(".jpg")
            || post.url.ends_with(".png")
            || post.url.contains("gallery"))
    {
        lines.push(Line::from(Span::styled(
            " ┌──────────────────────────┐",
            Style::default().fg(Color::Magenta),
        )));
        lines.push(Line::from(Span::styled(
            " │                          │",
            Style::default().fg(Color::Magenta),
        )));
        lines.push(Line::from(Span::styled(
            " │      IMAGE PREVIEW       │",
            Style::default().fg(Color::Magenta),
        )));
        lines.push(Line::from(Span::styled(
            " │                          │",
            Style::default().fg(Color::Magenta),
        )));
        lines.push(Line::from(Span::styled(
            " └──────────────────────────┘",
            Style::default().fg(Color::Magenta),
        )));
        lines.push(Line::from(Span::styled(
            format!(" {}", post.url),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " [i] Toggle image display   [o] Open externally",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        post.title.clone(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));

    if let Some(flair) = &post.link_flair_text {
        lines.push(Line::from(Span::styled(
            format!("  [{}]", flair),
            Style::default().fg(Color::Cyan),
        )));
    }

    lines.push(Line::from(Span::styled(
        "──────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Author:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("u/{}", post.author),
            Style::default().fg(Color::Cyan),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Score:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("▲ {}", post.score),
            Style::default().fg(Color::Yellow),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Replies: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            post.num_comments.to_string(),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    let mut in_code_block = false;
    for raw_line in post.selftext.lines() {
        let trimmed = raw_line.trim_start();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if in_code_block {
                lines.push(Line::from(Span::styled(
                    "────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(Span::styled(
                    "Code",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
            } else {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            continue;
        }

        if in_code_block {
            lines.push(Line::from(Span::styled(
                format!("  {}", raw_line),
                Style::default().fg(Color::Cyan),
            )));
            continue;
        }

        // Basic Markdown parsing mapped to Ratatui spans
        let mut spans = Vec::new();
        if trimmed.starts_with("# ") {
            spans.push(Span::styled(
                trimmed[2..].to_string(),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if trimmed.starts_with("## ") {
            spans.push(Span::styled(
                trimmed[3..].to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if trimmed.starts_with("> ") {
            spans.push(Span::styled("┃ ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                trimmed[2..].to_string(),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            ));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            spans.push(Span::styled("• ", Style::default().fg(Color::Magenta)));
            spans.push(Span::styled(
                trimmed[2..].to_string(),
                Style::default().fg(Color::Gray),
            ));
        } else {
            spans.push(Span::styled(
                trimmed.to_string(),
                Style::default().fg(Color::Gray),
            ));
        }

        if search_query.is_empty() {
            lines.push(Line::from(spans));
        } else {
            // Case-insensitive highlighting engine mapping over the pre-styled chunks
            let mut highlighted_spans = Vec::new();
            let query_lower = search_query.to_lowercase();
            for span in spans {
                let text_lower = span.content.to_lowercase();
                let mut last_idx = 0;
                for (idx, _) in text_lower.match_indices(&query_lower) {
                    if idx > last_idx {
                        highlighted_spans.push(Span::styled(
                            span.content[last_idx..idx].to_string(),
                            span.style,
                        ));
                    }
                    highlighted_spans.push(Span::styled(
                        span.content[idx..idx + search_query.len()].to_string(),
                        Style::default().fg(Color::Black).bg(Color::Yellow),
                    ));
                    last_idx = idx + search_query.len();
                }
                if last_idx < span.content.len() {
                    highlighted_spans.push(Span::styled(
                        span.content[last_idx..].to_string(),
                        span.style,
                    ));
                }
            }
            lines.push(Line::from(highlighted_spans));
        }
    }
    lines
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let top_line = Line::from(Span::styled(
        " j/k move • / search • tab focus • r refresh • o post • c comments • i image • esc back ",
        Style::default().fg(Color::DarkGray),
    ));

    let divider = Line::from(Span::styled(
        "───────────────────────────────────────────────────────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    ));

    let mut err_str = String::new();
    if !app.reddit_error.is_empty() {
        err_str = format!(" [Error: {}] ", app.reddit_error);
    }

    let last_up = if app.reddit_last_updated == "cached" {
        "cached"
    } else {
        &app.reddit_last_updated
    };
    let bottom_line = Line::from(vec![
        Span::styled(
            format!(" {} posts", app.filtered_reddit.len()),
            Style::default().fg(Color::White),
        ),
        Span::styled(err_str, Style::default().fg(Color::Red)),
        Span::styled("    Updated ", Style::default().fg(Color::DarkGray)),
        Span::styled(last_up, Style::default().fg(Color::White)),
    ]);

    let paragraph =
        Paragraph::new(vec![top_line, divider, bottom_line]).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}
