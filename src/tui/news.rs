use super::{App, InputMode, NewsFocus};
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
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.area());

    let (border_color, cursor) = match app.input_mode {
        InputMode::Editing => (Color::Cyan, "█"),
        InputMode::Normal => (Color::DarkGray, ""),
    };

    let search_display = format!(" search (/): {}{} ", app.news_search_query, cursor);
    let search_bar = Paragraph::new(search_display)
        .style(Style::default().fg(border_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(" arch linux news ")
                .title_alignment(Alignment::Left),
        );
    f.render_widget(search_bar, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(chunks[1]);

    let items: Vec<ListItem> = app.filtered_news.iter().map(|news| {
        let is_read = app.read_news.contains(&news.link);
        
        let prefix = if news.is_critical { "!! " } else { "► " };
        let title_color = if news.is_critical { Color::Red } else if !is_read { Color::Cyan } else { Color::DarkGray };
        let modifier = if !is_read { Modifier::BOLD } else { Modifier::empty() };

        let date_str = chrono::DateTime::parse_from_rfc2822(&news.pub_date)
            .map(|dt| dt.format("%b %d, %Y").to_string())
            .unwrap_or_else(|_| news.pub_date.clone());

        ListItem::new(vec![
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(title_color).add_modifier(modifier)),
                Span::styled(news.title.clone(), Style::default().fg(title_color).add_modifier(modifier)),
            ]),
            Line::from(Span::styled(format!("  {}", date_str), Style::default().fg(Color::DarkGray))),
            Line::from(""), 
        ])
    }).collect();

    let list_border = if app.news_focus == NewsFocus::List { Color::Cyan } else { Color::DarkGray };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(list_border)).title(" headlines "))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, content_chunks[0], &mut app.news_list_state);

    let article_border = if app.news_focus == NewsFocus::Article { Color::Cyan } else { Color::DarkGray };
    
    if app.is_fetching_news {
        let loading = Paragraph::new("\n\n  fetching latest arch News...\n  ⠋ downloading feed\n  ⠙ parsing articles")
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(article_border)).title(" reading "));
        f.render_widget(loading, content_chunks[1]);
    } else if let Some(selected_idx) = app.news_list_state.selected() {
        if let Some(article) = app.filtered_news.get(selected_idx) {
            let mut article_lines = Vec::new();

            article_lines.push(Line::from(Span::styled(article.title.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
            article_lines.push(Line::from(Span::styled(format!("published: {}", article.pub_date), Style::default().fg(Color::DarkGray))));
            article_lines.push(Line::from(Span::styled("─".repeat(content_chunks[1].width as usize - 4), Style::default().fg(Color::DarkGray))));
            article_lines.push(Line::from(""));

            let mut in_code = false;
            let clean_desc = article.description
                .replace("<p>", "").replace("</p>", "\n\n")
                .replace("<li>", "• ").replace("</li>", "\n")
                .replace("<ul>", "").replace("</ul>", "\n")
                .replace("<br>", "\n").replace("<br/>", "\n")
                .replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");

            for raw_line in clean_desc.lines() {
                let mut line_spans = Vec::new();
                let mut current_text = raw_line.to_string();

                if current_text.contains("<code>") { in_code = true; current_text = current_text.replace("<code>", ""); }
                if current_text.contains("</code>") { in_code = false; current_text = current_text.replace("</code>", ""); }

                while let Some(start) = current_text.find('<') {
                    if let Some(end) = current_text[start..].find('>') {
                        current_text.replace_range(start..start + end + 1, "");
                    } else { break; }
                }

                let style = if in_code || raw_line.contains("pacman -") {
                    Style::default().fg(Color::Magenta) 
                } else {
                    Style::default().fg(Color::Gray)
                };

                let query = app.news_search_query.to_lowercase();
                if !query.is_empty() && current_text.to_lowercase().contains(&query) {
                    let parts: Vec<&str> = current_text.split_terminator(&query).collect();
                    for (i, part) in parts.iter().enumerate() {
                        line_spans.push(Span::styled(part.to_string(), style));
                        if i < parts.len() - 1 {
                            line_spans.push(Span::styled(query.clone(), Style::default().fg(Color::Black).bg(Color::Yellow)));
                        }
                    }
                } else {
                    line_spans.push(Span::styled(current_text, style));
                }

                article_lines.push(Line::from(line_spans));
            }

            let max_scroll = article_lines.len().saturating_sub(content_chunks[1].height as usize);
            app.news_scroll = app.news_scroll.min(max_scroll as u16);

            let paragraph = Paragraph::new(article_lines)
                .wrap(Wrap { trim: false })
                .scroll((app.news_scroll, 0))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(article_border)).title(" reading "));

            f.render_widget(paragraph, content_chunks[1]);
        }
    } else {
        let empty = Paragraph::new("no articles found.\ntry another search.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(article_border)).title(" reading "));
        f.render_widget(empty, content_chunks[1]);
    }

    let unread_count = app.news_items.iter().filter(|n| !app.read_news.contains(&n.link)).count();
    let stats = format!(" {} articles | updated {} | {} unread ", app.filtered_news.len(), app.news_last_updated, unread_count);
    
    let footer_text = vec![
        Span::raw(" j/k:nav • enter/tab:focus • f:search • r:refresh • y:copy • o:browser • esc:back "),
        Span::styled(stats, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ];
    
    let footer = Paragraph::new(Line::from(footer_text)).alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);
}
