use crate::tui::App;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

const COLOR_HEADING: Color = Color::Cyan;
const COLOR_LABEL: Color = Color::LightMagenta;
const COLOR_VALUE: Color = Color::White;
const COLOR_HEALTHY: Color = Color::Green;
const COLOR_WARNING: Color = Color::Yellow;
const COLOR_BAD: Color = Color::Red;

pub fn render(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(3),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.area());

    f.render_widget(
        Paragraph::new(Span::styled(
            "system health & statistics",
            Style::default()
                .fg(COLOR_HEADING)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        main_chunks[0],
    );

    let row1 = split_row(main_chunks[1]);
    let row2 = split_row(main_chunks[2]);
    let row3 = split_row(main_chunks[3]);

    let installed_count = app.package_list.iter().filter(|p| p.is_installed).count();
    let updates_count = app.package_list.iter().filter(|p| p.is_upgradable).count();
    let deps_count = installed_count.saturating_sub(app.explicit_count);
    let total_size_mb: f64 = app
        .package_list
        .iter()
        .filter(|p| p.is_installed)
        .map(|p| p.size_mb)
        .sum();
    let total_size_str = format!("{:.1} gib", total_size_mb / 1024.0);

    let mut health = 100i32;
    health -= app.orphan_count as i32 * 3;
    health -= updates_count as i32;
    let health = health.clamp(0, 100) as u16;
    let health_color = if health > 90 {
        COLOR_HEALTHY
    } else if health > 70 {
        COLOR_WARNING
    } else {
        COLOR_BAD
    };

    let pkg_lines = vec![
        format_stat("installed", &installed_count.to_string(), COLOR_VALUE),
        format_stat("explicit", &app.explicit_count.to_string(), COLOR_VALUE),
        format_stat("dependencies", &deps_count.to_string(), COLOR_VALUE),
        format_stat(
            "orphans",
            &format!(
                "{} {}",
                app.orphan_count,
                if app.orphan_count == 0 { "✓" } else { "!" }
            ),
            if app.orphan_count == 0 {
                COLOR_HEALTHY
            } else {
                COLOR_WARNING
            },
        ),
    ];
    f.render_widget(
        Paragraph::new(pkg_lines).block(create_card(" packages ")),
        row1[0],
    );

    let storage_lines = vec![
        format_stat("installed size", &total_size_str, COLOR_VALUE),
        format_stat("cache size", &app.cache_size, COLOR_VALUE),
        format_stat("free space", &app.free_space, COLOR_VALUE),
    ];
    f.render_widget(
        Paragraph::new(storage_lines).block(create_card(" storage ")),
        row1[1],
    );

    let updates_lines = vec![
        format_stat(
            "pending updates",
            &format!(
                "{} {}",
                updates_count,
                if updates_count == 0 { "✓" } else { "⬆" }
            ),
            if updates_count == 0 {
                COLOR_HEALTHY
            } else {
                COLOR_WARNING
            },
        ),
        format_stat("last sync", &app.last_sync, COLOR_VALUE),
        format_stat("pacman db", "healthy ✓", COLOR_HEALTHY),
    ];
    f.render_widget(
        Paragraph::new(updates_lines).block(create_card(" updates ")),
        row2[0],
    );

    let sys_lines = vec![
        format_stat("kernel", &app.kernel, COLOR_VALUE),
        format_stat("uptime", &app.uptime, COLOR_VALUE),
    ];
    f.render_widget(
        Paragraph::new(sys_lines).block(create_card(" system ")),
        row2[1],
    );

    let exp_pct = if installed_count > 0 {
        (app.explicit_count as f64 / installed_count as f64) * 100.0
    } else {
        0.0
    };
    let dist_lines = vec![
        Line::from(vec![Span::styled(
            format!("{:<15}", "system health"),
            Style::default().fg(COLOR_LABEL),
        )]),
        Line::from(vec![
            Span::styled(
                draw_bar(f64::from(health), 20),
                Style::default().fg(health_color),
            ),
            Span::styled(format!(" {health}%"), Style::default().fg(COLOR_VALUE)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{:<15}", "explicit"),
                Style::default().fg(COLOR_LABEL),
            ),
            Span::styled(
                format!(" ({})", app.explicit_count),
                Style::default().fg(COLOR_VALUE),
            ),
        ]),
        Line::from(vec![
            Span::styled(draw_bar(exp_pct, 20), Style::default().fg(COLOR_HEADING)),
            Span::styled(format!(" {exp_pct:.0}%"), Style::default().fg(COLOR_VALUE)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(dist_lines).block(create_card(" diagnostics ")),
        row3[0],
    );

    let mut sorted_pkgs = app
        .package_list
        .iter()
        .filter(|p| p.is_installed)
        .collect::<Vec<_>>();
    sorted_pkgs.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut top_pkgs = Vec::new();
    for pkg in sorted_pkgs.iter().take(4) {
        top_pkgs.push(format_stat(
            &pkg.name,
            &format!("{:.0} mb", pkg.size_mb),
            COLOR_VALUE,
        ));
    }
    f.render_widget(
        Paragraph::new(top_pkgs).block(create_card(" largest packages ")),
        row3[1],
    );

    // Status Banner
    let status_msg = if app.orphan_count > 0 {
        Span::styled(
            format!(" ! {} orphan packages can be removed.", app.orphan_count),
            Style::default().fg(COLOR_WARNING),
        )
    } else if updates_count > 0 {
        Span::styled(
            format!(" ⬆ {updates_count} packages can be upgraded."),
            Style::default().fg(COLOR_HEADING),
        )
    } else {
        Span::styled(
            " :3c nothing needs attention.",
            Style::default().fg(COLOR_HEALTHY),
        )
    };

    let status_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(
        Paragraph::new(status_msg)
            .block(status_block)
            .alignment(Alignment::Left),
        main_chunks[4],
    );

    // Contextual Footer
    let footer_text = format!(
        " r refresh • h history • ? help • esc back     last refreshed: {}",
        app.last_refreshed
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            footer_text,
            Style::default().fg(Color::DarkGray),
        ))
        .alignment(Alignment::Center),
        main_chunks[5],
    );
}

fn split_row(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(area)
}

fn create_card(title: &str) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(COLOR_HEADING),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn format_stat(label: &str, value: &str, val_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<18}"), Style::default().fg(COLOR_LABEL)),
        Span::styled(
            format!("{value:>14}"),
            Style::default().fg(val_color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn draw_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
