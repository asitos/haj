use super::{
    App, AsyncBufReadExt, AsyncReadExt, BrowserCache, BrowserTab, CurrentScreen, DashboardWidget,
    GroupSortMode, InputMode, IntoText, NEWS_PAGE_SIZE, NewsFocus, PackageFilter, SelectionMode,
    SortMode, Text, TuiEvent, browser, dashboard, fetch_arch_news, fetch_article_body, groups,
    help, history, news, stats, transaction,
};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    <B as Backend>::Error: Send + Sync + 'static,
{
    let config = crate::config::load_config();
    let (tx, mut rx) = mpsc::channel::<TuiEvent>(100);

    fetch_arch_news(tx.clone(), 1);

    // Keep sudo timestamp alive in the background to prevent credential expiration
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(45)).await;
            let _ = tokio::process::Command::new("sudo")
                .args(["-n", "-v"])
                .status()
                .await;
        }
    });

    let tx_input = tx.clone();
    tokio::spawn(async move {
        loop {
            if tx_input.is_closed() {
                break;
            }
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    let _ = tx_input.send(TuiEvent::Key(key)).await;
                }
            } else {
                let _ = tx_input.try_send(TuiEvent::_Tick);
            }
        }
    });

    let tx_art = tx.clone();
    tokio::spawn(async move {
        if config.general.animations {
            let temp_dir = std::env::temp_dir();
            let obj_path = temp_dir.join("blahaj.obj");
            let mtl_path = temp_dir.join("blahaj.mtl");

            if !obj_path.exists() {
                let _ = std::fs::write(&obj_path, include_bytes!("../../resources/blahaj.obj"));
            }
            if !mtl_path.exists() {
                let _ = std::fs::write(&mtl_path, include_bytes!("../../resources/blahaj.mtl"));
            }

            let child = tokio::process::Command::new("display3d")
                .args([&obj_path.to_string_lossy(), "-t", "0,0.5,7.5"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn();

            if let Ok(mut child_proc) = child {
                let mut stdout = if let Some(s) = child_proc.stdout.take() {
                    s
                } else {
                    return;
                };
                let mut buf = vec![0; 8192];
                let mut frame_buffer = Vec::new();
                let mut first_frame = true;

                loop {
                    let read_fut = stdout.read(&mut buf);
                    let read_res = if first_frame {
                        tokio::time::timeout(Duration::from_secs(2), read_fut)
                            .await
                            .unwrap_or(Ok(0))
                    } else {
                        read_fut.await
                    };

                    match read_res {
                        Ok(0) => break,
                        Ok(n) => {
                            first_frame = false;
                            frame_buffer.extend_from_slice(&buf[..n]);
                            while let Some(pos) =
                                frame_buffer.windows(3).position(|w| w == b"\x1b[H")
                            {
                                let frame = frame_buffer[..pos].to_vec();
                                frame_buffer.drain(..=pos + 2);
                                if let Ok(text) = frame.into_text() {
                                    let _ = tx_art.send(TuiEvent::DashboardArtFrame(text)).await;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        let fallback_str = include_str!("../../resources/title.txt");
        let text = fallback_str
            .as_bytes()
            .into_text()
            .unwrap_or_else(|_| Text::raw(fallback_str));
        let _ = tx_art.send(TuiEvent::DashboardArtFrame(text)).await;
    });

    let spawn_pacman = |tx_channel: mpsc::Sender<TuiEvent>,
                        mut args: Vec<String>,
                        _action_name: String,
                        mut abort_rx: mpsc::Receiver<()>| {
        tokio::spawn(async move {
            if !args.contains(&"--color=never".to_string()) {
                args.push("--color=never".into());
            }

            let is_root = crate::core::is_root();
            let (cmd_name, final_args) = if is_root {
                ("pacman".to_string(), args[1..].to_vec())
            } else {
                ("sudo".to_string(), args)
            };

            let child_res = tokio::process::Command::new(cmd_name)
                .args(final_args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn();

            if let Ok(mut child) = child_res {
                let pid = child.id();
                let stdout = if let Some(s) = child.stdout.take() {
                    s
                } else {
                    return;
                };
                let stderr = if let Some(s) = child.stderr.take() {
                    s
                } else {
                    return;
                };

                let tx_out = tx_channel.clone();
                let out_task = tokio::spawn(async move {
                    let mut reader = tokio::io::BufReader::new(stdout).lines();
                    let mut in_hook_phase = false;

                    while let Ok(Some(line)) = reader.next_line().await {
                        let clean = line.trim();
                        if clean.is_empty() {
                            continue;
                        }
                        let lower_clean = clean.to_lowercase();

                        if lower_clean.contains("resolving dependencies")
                            || lower_clean.contains("conflicting packages")
                        {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "resolving package dependencies...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("checking keys")
                            || lower_clean.contains("checking package integrity")
                        {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "verifying package integrity...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("loading package files") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction("loading package files...".into()))
                                .await;
                        } else if lower_clean.contains("checking for file conflicts") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "checking for file conflicts...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("checking available disk space") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "checking available disk space...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("retrieving packages") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction("downloading packages...".into()))
                                .await;
                        } else if lower_clean.contains("processing package changes") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "processing package changes...".into(),
                                ))
                                .await;
                        } else if clean.starts_with('(')
                            && (lower_clean.contains(") upgrading")
                                || lower_clean.contains(") installing")
                                || lower_clean.contains(") removing"))
                        {
                            if let Some(idx_end) = clean.find(')') {
                                let counter = &clean[..=idx_end];
                                let action = if lower_clean.contains("installing") {
                                    "installing"
                                } else if lower_clean.contains("removing") {
                                    "removing"
                                } else {
                                    "upgrading"
                                };
                                let _ = tx_out
                                    .send(TuiEvent::UpdateAction(format!(
                                        "{counter} packages {action}..."
                                    )))
                                    .await;
                            }
                        } else if lower_clean.contains("running pre-transaction hooks") {
                            in_hook_phase = true;
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "running pre-transaction hooks...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("running post-transaction hooks") {
                            in_hook_phase = true;
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "running post-transaction hooks...".into(),
                                ))
                                .await;
                        } else if in_hook_phase
                            && (clean.starts_with("==> Building image")
                                || clean.starts_with("==> Install DKMS")
                                || clean.starts_with("==> Generating"))
                        {
                            let _ = tx_out.send(TuiEvent::UpdateAction(clean.to_string())).await;
                        }

                        if lower_clean.contains("error:")
                            || lower_clean.contains("warning:")
                            || lower_clean.contains("failed")
                            || clean.starts_with("::")
                            || lower_clean.contains("up to date")
                            || lower_clean.contains("downloading")
                            || (in_hook_phase
                                && (lower_clean.contains("missing")
                                    || lower_clean.contains("not found")))
                        {
                            let _ = tx_out.send(TuiEvent::PacmanLog(clean.to_string())).await;
                        }

                        if clean.contains('%') {
                            let parts: Vec<&str> = clean.split('%').collect();
                            if !parts.is_empty() {
                                let num_str = parts[0].split_whitespace().last().unwrap_or("0");
                                if let Ok(val) = num_str.parse::<u16>() {
                                    let _ = tx_out.send(TuiEvent::PacmanProgress(val)).await;
                                }
                            }
                        }
                    }
                });

                let tx_err = tx_channel.clone();
                let err_task = tokio::spawn(async move {
                    let mut reader = tokio::io::BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let clean = line.trim();
                        if clean.is_empty() {
                            continue;
                        }
                        let _ = tx_err
                            .send(TuiEvent::PacmanLog(format!("error: {clean}")))
                            .await;
                    }
                });

                let mut status = None;
                let mut aborted = false;

                tokio::select! {
                    () = async {
                        let _ = tokio::join!(out_task, err_task);
                    } => {
                        status = Some(child.wait().await.unwrap_or_else(|_| std::os::unix::process::ExitStatusExt::from_raw(1)));
                    }
                    Some(()) = abort_rx.recv() => {
                        aborted = true;
                        if let Some(p) = pid {
                            let _ = tokio::process::Command::new("sudo")
                                .args(["-n", "kill", "-INT", &p.to_string()])
                                .status()
                                .await;

                            tokio::select! {
                                s = child.wait() => {
                                    status = s.ok();
                                }
                                () = tokio::time::sleep(Duration::from_millis(500)) => {
                                    let _ = tokio::process::Command::new("sudo")
                                        .args(["-n", "kill", "-KILL", &p.to_string()])
                                        .status()
                                        .await;
                                    tokio::select! {
                                        s = child.wait() => {
                                            status = s.ok();
                                        }
                                        () = tokio::time::sleep(Duration::from_millis(200)) => {}
                                    }
                                }
                            }
                        }
                    }
                }

                let _ = tx_channel.send(TuiEvent::PacmanProgress(100)).await;

                let success = status.is_some_and(|s| s.success());
                if success && !aborted {
                    let _ = tx_channel.send(TuiEvent::TransactionComplete).await;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                } else {
                    let _ = tx_channel.send(TuiEvent::TransactionFailed).await;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            } else {
                let _ = tx_channel.send(TuiEvent::TransactionFailed).await;
                let _ = tx_channel
                    .send(TuiEvent::PacmanLog(
                        "fatal: failed to spawn sudo process".into(),
                    ))
                    .await;
                tokio::time::sleep(Duration::from_secs(4)).await;
            }

            let _ = tx_channel.send(TuiEvent::CloseTransaction).await;
        });
    };

    loop {
        terminal.draw(|f| {
            match app.screen {
                CurrentScreen::Dashboard => dashboard::render(f, app),
                CurrentScreen::Browser => browser::render(f, app),
                CurrentScreen::News => news::render(f, app),
                CurrentScreen::Stats => stats::render(f, app),
                CurrentScreen::History => history::render(f, app),
                CurrentScreen::Groups => groups::render(f, app),
            }
            transaction::render_popup(f, app);
            transaction::render_confirm_popup(f, app);

            if app.show_help {
                help::render_popup(f);
            }
        })?;

        if let Some(event) = rx.recv().await {
            match event {
                TuiEvent::_Tick => {
                    if app.screen == CurrentScreen::Dashboard
                        && app.active_widget != DashboardWidget::Blahaj
                        && app.last_activity.elapsed() > Duration::from_secs(10)
                    {
                        let unread_news = app
                            .news_items
                            .iter()
                            .filter(|n| !app.read_news.contains(&n.link))
                            .count();
                        let updates = app.package_list.iter().filter(|p| p.is_upgradable).count();
                        if unread_news == 0 && updates == 0 {
                            app.active_widget = DashboardWidget::Blahaj;
                        }
                    }
                }

                TuiEvent::DashboardArtFrame(text) => app.dashboard_art = text,
                TuiEvent::PacmanLog(log) => {
                    app.transaction_logs.push(log);
                    if app.transaction_logs.len() > 30 {
                        app.transaction_logs.remove(0);
                    }
                }
                TuiEvent::UpdateAction(action) => app.current_action = action,
                TuiEvent::PacmanProgress(val) => app.progress = val.min(100),
                TuiEvent::TransactionComplete => {
                    app.current_action = "changes complete".to_string();
                }
                TuiEvent::TransactionFailed => app.current_action = "changes failed X".to_string(),
                TuiEvent::CloseTransaction => {
                    app.is_installing = false;
                    app.progress = 0;
                    app.refresh_state();
                }

                TuiEvent::NewsFetched(items, time) => {
                    app.is_fetching_news = false;
                    app.news_error.clear();
                    app.news_items = items;
                    app.news_last_updated = time;
                    app.update_news_search(false);

                    if let Some(idx) = app.news_list_state.selected() {
                        let page_size = NEWS_PAGE_SIZE;
                        let actual_idx = (app.news_page.saturating_sub(1)) * page_size + idx;
                        if let Some(item) = app.filtered_news.get(actual_idx)
                            && (item.description == "loading article content..."
                                || item.description.is_empty())
                        {
                            fetch_article_body(tx.clone(), item.link.clone());
                        }
                    }
                }
                TuiEvent::NewsFetchFailed(err) => {
                    app.is_fetching_news = false;
                    app.news_error = err;
                }
                TuiEvent::NewsBodyFetched(link, description) => {
                    if let Some(item) = app.news_items.iter_mut().find(|n| n.link == link) {
                        item.description = description.clone();
                    }
                    if let Some(item) = app.filtered_news.iter_mut().find(|n| n.link == link) {
                        item.description = description.clone();
                    }
                    let home = std::env::var("HOME")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_default();
                    let cache_path = home.join(".cache/haj/news.json");
                    if let Ok(cache_data) = serde_json::to_string(&app.news_items) {
                        let _ = std::fs::write(cache_path, cache_data);
                    }
                }
                TuiEvent::NewsTotalCount(total) => {
                    app.news_total_count = total;
                }

                TuiEvent::BrowserInfoLoaded(pkg, info, deps) => {
                    if let Some(cache) = app.browser_caches.get_mut(&pkg) {
                        cache.info = info;
                        cache.dependencies = deps;
                        cache.loading_info = false;
                    }
                }
                TuiEvent::BrowserFilesLoaded(pkg, files) => {
                    if let Some(cache) = app.browser_caches.get_mut(&pkg) {
                        cache.files = files;
                        cache.loading_files = false;
                    }
                }

                TuiEvent::Key(key) => {
                    let prev_pkg = app.browser_pkg_name.clone();
                    let prev_tab = app.browser_tab;
                    app.last_activity = Instant::now();

                    if app.screen == CurrentScreen::News
                        && key.code != KeyCode::Char('c')
                        && (app.current_action.starts_with("copied ")
                            || app.current_action.starts_with("no command "))
                    {
                        app.current_action.clear();
                    }

                    if app.show_help {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q' | '?') => {
                                app.show_help = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if let KeyCode::Char('?') = key.code {
                        app.show_help = true;
                        continue;
                    }

                    if app.is_installing {
                        if let KeyCode::Char('q') = key.code
                            && let Some(tx_abort) = app.abort_tx.take()
                        {
                            let _ = tx_abort.try_send(());
                            app.current_action = "aborting safely...".to_string();
                            app.transaction_logs.push(String::new());
                            app.transaction_logs.push(
                                "==> user triggered abort. sending sigint to pacman...".into(),
                            );
                        }
                        continue;
                    }

                    if app.show_prompt {
                        match key.code {
                            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                                app.show_prompt = false;
                                app.is_installing = true;
                                app.transaction_logs.clear();
                                app.progress = 0;

                                app.current_action = if app.prompt_targets.len() > 1 {
                                    format!(
                                        "{}ing {} packages...",
                                        app.prompt_type,
                                        app.prompt_targets.len()
                                    )
                                } else {
                                    format!("{}ing {}...", app.prompt_type, app.prompt_targets[0])
                                };

                                let mut args = vec!["pacman".into()];
                                if app.prompt_type == "install" {
                                    args.extend(vec!["-S".into(), "--noconfirm".into()]);
                                } else {
                                    args.extend(vec!["-Rs".into(), "--noconfirm".into()]);
                                }
                                args.extend(app.prompt_targets.clone());

                                let (abort_tx, abort_rx) = mpsc::channel(1);
                                app.abort_tx = Some(abort_tx);
                                spawn_pacman(tx.clone(), args, app.prompt_type.clone(), abort_rx);

                                app.selected_packages.clear();
                                app.prompt_targets.clear();
                            }
                            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                                app.show_prompt = false;
                                app.prompt_targets.clear();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match app.screen {
                        CurrentScreen::Dashboard => match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Char('/' | 'f') => {
                                app.screen = CurrentScreen::Browser;
                                app.input_mode = InputMode::Editing;
                            }

                            KeyCode::Char('u') => {
                                app.is_installing = true;
                                app.current_action = "syncing repositories...".to_string();
                                app.transaction_logs.clear();
                                app.progress = 0;

                                let (abort_tx, abort_rx) = mpsc::channel(1);
                                app.abort_tx = Some(abort_tx);
                                spawn_pacman(
                                    tx.clone(),
                                    vec!["pacman".into(), "-Sy".into(), "--noconfirm".into()],
                                    "syncing repositories".into(),
                                    abort_rx,
                                );
                            }

                            KeyCode::Char('o') => {
                                if let Ok(output) =
                                    std::process::Command::new("pacman").arg("-Qdtq").output()
                                {
                                    let stdout = String::from_utf8_lossy(&output.stdout);
                                    let orphans: Vec<String> = stdout
                                        .split_whitespace()
                                        .map(std::string::ToString::to_string)
                                        .collect();

                                    app.is_installing = true;
                                    app.transaction_logs.clear();
                                    app.progress = 0;

                                    if orphans.is_empty() {
                                        app.current_action = "system clean".to_string();
                                        app.progress = 100;
                                        app.transaction_logs
                                            .push("no orphaned packages to remove.".to_string());

                                        let tx_clone = tx.clone();
                                        tokio::spawn(async move {
                                            tokio::time::sleep(Duration::from_secs(2)).await;
                                            let _ = tx_clone.send(TuiEvent::CloseTransaction).await;
                                        });
                                    } else {
                                        app.current_action =
                                            format!("sweeping {} orphans...", orphans.len());
                                        let mut args = vec![
                                            "pacman".into(),
                                            "-Rns".into(),
                                            "--noconfirm".into(),
                                        ];
                                        args.extend(orphans);

                                        let (abort_tx, abort_rx) = mpsc::channel(1);
                                        app.abort_tx = Some(abort_tx);
                                        spawn_pacman(
                                            tx.clone(),
                                            args,
                                            "cleaning orphans".into(),
                                            abort_rx,
                                        );
                                    }
                                }
                            }
                            KeyCode::Char('c') => {
                                app.is_installing = true;
                                app.current_action = "cleaning package cache...".to_string();
                                app.transaction_logs.clear();
                                app.progress = 0;

                                let _ = std::process::Command::new("sudo")
                                    .args(["sh", "-c", "rm -rf /var/cache/pacman/pkg/download-*"])
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .status();

                                let (abort_tx, abort_rx) = mpsc::channel(1);
                                app.abort_tx = Some(abort_tx);
                                spawn_pacman(
                                    tx.clone(),
                                    vec!["pacman".into(), "-Sc".into(), "--noconfirm".into()],
                                    "cleaning cache".into(),
                                    abort_rx,
                                );
                            }

                            KeyCode::Char('n') => {
                                app.screen = CurrentScreen::News;
                                if let Some(idx) = app.news_list_state.selected() {
                                    let page_size = NEWS_PAGE_SIZE;
                                    let actual_idx =
                                        (app.news_page.saturating_sub(1)) * page_size + idx;
                                    if let Some(item) = app.filtered_news.get(actual_idx)
                                        && (item.description == "loading article content..."
                                            || item.description.is_empty())
                                    {
                                        fetch_article_body(tx.clone(), item.link.clone());
                                    }
                                }
                            }
                            KeyCode::Char('t') => app.screen = CurrentScreen::Stats,
                            KeyCode::Char('h') => app.screen = CurrentScreen::History,
                            KeyCode::Char('g') => app.screen = CurrentScreen::Groups,
                            KeyCode::Char('b') => {
                                if app.active_widget == DashboardWidget::Blahaj {
                                    app.active_widget = DashboardWidget::News; // used as hidden state
                                } else {
                                    app.active_widget = DashboardWidget::Blahaj;
                                }
                            }
                            KeyCode::Enter => match app.active_widget {
                                DashboardWidget::News => {
                                    app.screen = CurrentScreen::News;
                                    if let Some(idx) = app.news_list_state.selected() {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let actual_idx =
                                            (app.news_page.saturating_sub(1)) * page_size + idx;
                                        if let Some(item) = app.filtered_news.get(actual_idx)
                                            && (item.description == "loading article content..."
                                                || item.description.is_empty())
                                        {
                                            fetch_article_body(tx.clone(), item.link.clone());
                                        }
                                    }
                                }
                                DashboardWidget::Blahaj => app.screen = CurrentScreen::Browser,
                            },
                            KeyCode::Tab => {
                                app.active_widget = match app.active_widget {
                                    DashboardWidget::Blahaj => DashboardWidget::News,
                                    DashboardWidget::News => DashboardWidget::Blahaj,
                                };
                            }
                            KeyCode::BackTab => {
                                app.active_widget = match app.active_widget {
                                    DashboardWidget::Blahaj => DashboardWidget::News,
                                    DashboardWidget::News => DashboardWidget::Blahaj,
                                };
                            }
                            _ => {}
                        },

                        CurrentScreen::News => match app.input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Char('/' | 'f' | 's') => {
                                    app.input_mode = InputMode::Editing;
                                }
                                KeyCode::Char('r') => {
                                    app.is_fetching_news = true;
                                    app.news_page = 1;
                                    fetch_arch_news(tx.clone(), 1);
                                }
                                KeyCode::Tab | KeyCode::Enter => {
                                    app.news_focus = if app.news_focus == NewsFocus::List {
                                        NewsFocus::Article
                                    } else {
                                        NewsFocus::List
                                    };
                                }
                                KeyCode::Char(']') => {
                                    let page_size = NEWS_PAGE_SIZE;
                                    let total_count = if app.news_search_query.is_empty() {
                                        app.news_total_count
                                    } else {
                                        app.filtered_news.len()
                                    };
                                    let max_pages = total_count.div_ceil(page_size);
                                    if app.news_page < max_pages {
                                        app.news_page += 1;
                                        app.news_list_state.select(Some(0));
                                        app.news_scroll = 0;

                                        let needed_index = app.news_page * page_size;
                                        if needed_index > app.news_items.len()
                                            && app.news_items.len() < app.news_total_count
                                        {
                                            let next_web_page = (app.news_items.len() / 50) + 1;
                                            app.is_fetching_news = true;
                                            fetch_arch_news(tx.clone(), next_web_page);
                                        } else if let Some(idx) = app.news_list_state.selected() {
                                            let actual_idx = (app.news_page - 1) * page_size + idx;
                                            if let Some(item) = app.filtered_news.get(actual_idx)
                                                && (item.description
                                                    == "loading article content..."
                                                    || item.description.is_empty())
                                            {
                                                fetch_article_body(tx.clone(), item.link.clone());
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('[') => {
                                    if app.news_page > 1 {
                                        app.news_page -= 1;
                                        app.news_list_state.select(Some(0));
                                        app.news_scroll = 0;

                                        let page_size = NEWS_PAGE_SIZE;
                                        if let Some(idx) = app.news_list_state.selected() {
                                            let actual_idx = (app.news_page - 1) * page_size + idx;
                                            if let Some(item) = app.filtered_news.get(actual_idx)
                                                && (item.description
                                                    == "loading article content..."
                                                    || item.description.is_empty())
                                            {
                                                fetch_article_body(tx.clone(), item.link.clone());
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('o') => {
                                    if let Some(idx) = app.news_list_state.selected() {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let actual_idx = (app.news_page - 1) * page_size + idx;
                                        if let Some(item) = app.filtered_news.get(actual_idx) {
                                            let url = item.link.clone();
                                            tokio::spawn(async move {
                                                let _ = tokio::process::Command::new("xdg-open")
                                                    .arg(&url)
                                                    .output()
                                                    .await;
                                            });
                                        }
                                    }
                                }
                                KeyCode::Char('y') => {
                                    if let Some(idx) = app.news_list_state.selected() {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let actual_idx = (app.news_page - 1) * page_size + idx;
                                        if let Some(item) = app.filtered_news.get(actual_idx) {
                                            let link = item.link.clone();
                                            let link_clone = link.clone();
                                            tokio::spawn(async move {
                                                if std::process::Command::new("wl-copy")
                                                    .arg(&link_clone)
                                                    .output()
                                                    .is_err()
                                                    && let Ok(mut child) =
                                                        std::process::Command::new("xclip")
                                                            .args(["-selection", "clipboard"])
                                                            .stdin(std::process::Stdio::piped())
                                                            .spawn()
                                                {
                                                    if let Some(mut stdin) = child.stdin.take() {
                                                        use std::io::Write;
                                                        let _ =
                                                            stdin.write_all(link_clone.as_bytes());
                                                    }
                                                    let _ = child.wait();
                                                }
                                            });
                                            app.current_action =
                                                format!("copied \"{link}\" to clipboard");
                                        }
                                    }
                                }
                                KeyCode::Char('c') => {
                                    if let Some(idx) = app.news_list_state.selected() {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let actual_idx = (app.news_page - 1) * page_size + idx;
                                        if let Some(item) = app.filtered_news.get(actual_idx) {
                                            let mut cmds = Vec::new();
                                            for line in item.description.lines() {
                                                let cleaned = line
                                                    .replace("<code>", "")
                                                    .replace("</code>", "");
                                                let mut l = cleaned.trim();
                                                if l.starts_with('#') || l.starts_with('$') {
                                                    l = l[1..].trim();
                                                }
                                                if l.starts_with("pacman ")
                                                    || l.starts_with("systemctl ")
                                                    || l.starts_with("mkinitcpio ")
                                                    || l.starts_with("grub-install ")
                                                    || l.starts_with("chown ")
                                                {
                                                    cmds.push(l.to_string());
                                                }
                                            }
                                            let cmd_to_copy = cmds.join("\n");
                                            if cmd_to_copy.is_empty() {
                                                app.current_action =
                                                    "no command found to copy".to_string();
                                            } else {
                                                let cmd_clone = cmd_to_copy.clone();
                                                tokio::spawn(async move {
                                                    if std::process::Command::new("wl-copy")
                                                        .arg(&cmd_clone)
                                                        .output()
                                                        .is_err()
                                                        && let Ok(mut child) =
                                                            std::process::Command::new("xclip")
                                                                .args(["-selection", "clipboard"])
                                                                .stdin(std::process::Stdio::piped())
                                                                .spawn()
                                                    {
                                                        if let Some(mut stdin) = child.stdin.take()
                                                        {
                                                            use std::io::Write;
                                                            let _ = stdin
                                                                .write_all(cmd_clone.as_bytes());
                                                        }
                                                        let _ = child.wait();
                                                    }
                                                });
                                                if cmds.len() > 1 {
                                                    app.current_action = format!(
                                                        "copied {} commands to clipboard",
                                                        cmds.len()
                                                    );
                                                } else {
                                                    app.current_action = format!(
                                                        "copied \"{cmd_to_copy}\" to clipboard"
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if app.news_focus == NewsFocus::List {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let start_idx =
                                            (app.news_page.saturating_sub(1)) * page_size;
                                        let end_idx =
                                            (start_idx + page_size).min(app.filtered_news.len());
                                        let displayed_count = end_idx.saturating_sub(start_idx);

                                        let i = match app.news_list_state.selected() {
                                            Some(i) => {
                                                if i >= displayed_count.saturating_sub(1) {
                                                    0
                                                } else {
                                                    i + 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.news_list_state.select(Some(i));
                                        app.news_scroll = 0;

                                        let mut link_and_should_fetch = None;
                                        let actual_idx = start_idx + i;
                                        if let Some(item) = app.filtered_news.get(actual_idx) {
                                            let is_loading = item.description
                                                == "loading article content..."
                                                || item.description.is_empty();
                                            link_and_should_fetch =
                                                Some((item.link.clone(), is_loading));
                                        }
                                        if let Some((link, is_loading)) = link_and_should_fetch {
                                            app.mark_news_read(link.clone());
                                            if is_loading {
                                                fetch_article_body(tx.clone(), link);
                                            }
                                        }
                                    } else {
                                        app.news_scroll = app.news_scroll.saturating_add(1);
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.news_focus == NewsFocus::List {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let start_idx =
                                            (app.news_page.saturating_sub(1)) * page_size;
                                        let end_idx =
                                            (start_idx + page_size).min(app.filtered_news.len());
                                        let displayed_count = end_idx.saturating_sub(start_idx);

                                        let i = match app.news_list_state.selected() {
                                            Some(i) => {
                                                if i == 0 {
                                                    displayed_count.saturating_sub(1)
                                                } else {
                                                    i - 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.news_list_state.select(Some(i));
                                        app.news_scroll = 0;

                                        let mut link_and_should_fetch = None;
                                        let actual_idx = start_idx + i;
                                        if let Some(item) = app.filtered_news.get(actual_idx) {
                                            let is_loading = item.description
                                                == "loading article content..."
                                                || item.description.is_empty();
                                            link_and_should_fetch =
                                                Some((item.link.clone(), is_loading));
                                        }
                                        if let Some((link, is_loading)) = link_and_should_fetch {
                                            app.mark_news_read(link.clone());
                                            if is_loading {
                                                fetch_article_body(tx.clone(), link);
                                            }
                                        }
                                    } else {
                                        app.news_scroll = app.news_scroll.saturating_sub(1);
                                    }
                                }
                                KeyCode::PageDown => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = app.news_scroll.saturating_add(15);
                                    }
                                }
                                KeyCode::PageUp => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = app.news_scroll.saturating_sub(15);
                                    }
                                }
                                KeyCode::Home => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = 0;
                                    }
                                }
                                KeyCode::End => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = 999;
                                    }
                                }
                                KeyCode::Char('g') => {
                                    if app.news_focus == NewsFocus::List {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let start_idx =
                                            (app.news_page.saturating_sub(1)) * page_size;
                                        app.news_list_state.select(Some(0));
                                        app.news_scroll = 0;

                                        let mut link_and_should_fetch = None;
                                        let actual_idx = start_idx;
                                        if let Some(item) = app.filtered_news.get(actual_idx) {
                                            let is_loading = item.description
                                                == "loading article content..."
                                                || item.description.is_empty();
                                            link_and_should_fetch =
                                                Some((item.link.clone(), is_loading));
                                        }
                                        if let Some((link, is_loading)) = link_and_should_fetch {
                                            app.mark_news_read(link.clone());
                                            if is_loading {
                                                fetch_article_body(tx.clone(), link);
                                            }
                                        }
                                    } else {
                                        app.news_scroll = 0;
                                    }
                                }
                                KeyCode::Char('G') => {
                                    if app.news_focus == NewsFocus::List {
                                        let page_size = NEWS_PAGE_SIZE;
                                        let start_idx =
                                            (app.news_page.saturating_sub(1)) * page_size;
                                        let end_idx =
                                            (start_idx + page_size).min(app.filtered_news.len());
                                        let displayed_count = end_idx.saturating_sub(start_idx);
                                        let i = displayed_count.saturating_sub(1);
                                        app.news_list_state.select(Some(i));
                                        app.news_scroll = 0;

                                        let mut link_and_should_fetch = None;
                                        let actual_idx = start_idx + i;
                                        if let Some(item) = app.filtered_news.get(actual_idx) {
                                            let is_loading = item.description
                                                == "loading article content..."
                                                || item.description.is_empty();
                                            link_and_should_fetch =
                                                Some((item.link.clone(), is_loading));
                                        }
                                        if let Some((link, is_loading)) = link_and_should_fetch {
                                            app.mark_news_read(link.clone());
                                            if is_loading {
                                                fetch_article_body(tx.clone(), link);
                                            }
                                        }
                                    } else {
                                        app.news_scroll = 999;
                                    }
                                }
                                _ => {}
                            },
                            InputMode::Editing => match key.code {
                                KeyCode::Esc | KeyCode::Enter => app.input_mode = InputMode::Normal,
                                KeyCode::Char('l')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.news_search_query.clear();
                                    app.update_news_search(true);
                                }
                                KeyCode::Backspace | KeyCode::Delete => {
                                    app.news_search_query.pop();
                                    app.update_news_search(true);
                                }
                                KeyCode::Char(c) => {
                                    app.news_search_query.push(c);
                                    app.update_news_search(true);
                                }
                                _ => {}
                            },
                        },
                        CurrentScreen::Stats => match key.code {
                            KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Char('r') => {
                                app.refresh_state();
                            }
                            KeyCode::Char('h') => app.screen = CurrentScreen::History,
                            _ => {}
                        },
                        CurrentScreen::History => history::handle_key(key, app),

                        CurrentScreen::Groups => match app.group_input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Char('/') => app.group_input_mode = InputMode::Editing,
                                KeyCode::Char(' ') => {
                                    if let Some(idx) = app.group_state.selected()
                                        && let Some(selected_group) = app.filtered_groups.get(idx)
                                    {
                                        let name = selected_group.name.clone();
                                        if let Some(g) =
                                            app.groups.iter_mut().find(|x| x.name == name)
                                        {
                                            g.is_favorite = !g.is_favorite;
                                        }
                                        app.update_group_filter();
                                    }
                                }
                                KeyCode::Char('S') => {
                                    app.group_sort_mode = match app.group_sort_mode {
                                        GroupSortMode::Alphabetical => GroupSortMode::PackageCount,
                                        GroupSortMode::PackageCount => {
                                            GroupSortMode::InstallCompletion
                                        }
                                        GroupSortMode::InstallCompletion => {
                                            GroupSortMode::Alphabetical
                                        }
                                    };
                                    app.update_group_filter();
                                }
                                KeyCode::Char('i') => {
                                    if let Some(idx) = app.group_state.selected()
                                        && let Some(selected_group) = app.filtered_groups.get(idx)
                                    {
                                        app.prompt_targets = selected_group
                                            .packages
                                            .iter()
                                            .map(|p| p.0.clone())
                                            .collect();
                                        if !app.prompt_targets.is_empty() {
                                            app.prompt_type = "install".to_string();
                                            app.show_prompt = true;
                                        }
                                    }
                                }
                                KeyCode::Char('r') => {
                                    if let Some(idx) = app.group_state.selected()
                                        && let Some(selected_group) = app.filtered_groups.get(idx)
                                    {
                                        let installed_pkgs: Vec<String> = selected_group
                                            .packages
                                            .iter()
                                            .filter(|p| p.1)
                                            .map(|p| p.0.clone())
                                            .collect();
                                        if !installed_pkgs.is_empty() {
                                            app.prompt_targets = installed_pkgs;
                                            app.prompt_type = "remove".to_string();
                                            app.show_prompt = true;
                                        }
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(idx) = app.group_state.selected()
                                        && let Some(selected_group) = app.filtered_groups.get(idx)
                                    {
                                        let gname = selected_group.name.clone();
                                        if let Some(pos) = app
                                            .filters
                                            .iter()
                                            .position(|f| *f == PackageFilter::Group(gname.clone()))
                                        {
                                            app.filter_idx = pos;
                                        } else {
                                            app.filters.push(PackageFilter::Group(gname.clone()));
                                            app.filter_idx = app.filters.len() - 1;
                                        }
                                        app.screen = CurrentScreen::Browser;
                                        app.update_search();
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if !app.filtered_groups.is_empty() {
                                        let i = match app.group_state.selected() {
                                            Some(i) => {
                                                if i >= app.filtered_groups.len() - 1 {
                                                    0
                                                } else {
                                                    i + 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.group_state.select(Some(i));
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if !app.filtered_groups.is_empty() {
                                        let i = match app.group_state.selected() {
                                            Some(i) => {
                                                if i == 0 {
                                                    app.filtered_groups.len() - 1
                                                } else {
                                                    i - 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.group_state.select(Some(i));
                                    }
                                }
                                KeyCode::Char('g') => {
                                    if !app.filtered_groups.is_empty() {
                                        app.group_state.select(Some(0));
                                    }
                                }
                                KeyCode::Char('G') if !app.filtered_groups.is_empty() => {
                                    app.group_state.select(Some(app.filtered_groups.len() - 1));
                                }
                                _ => {}
                            },
                            InputMode::Editing => match key.code {
                                KeyCode::Esc | KeyCode::Enter => {
                                    app.group_input_mode = InputMode::Normal;
                                }
                                KeyCode::Backspace | KeyCode::Delete => {
                                    app.group_search_query.pop();
                                    app.update_group_filter();
                                }
                                KeyCode::Char('l')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.group_search_query.clear();
                                    app.update_group_filter();
                                }
                                KeyCode::Char(c) => {
                                    app.group_search_query.push(c);
                                    app.update_group_filter();
                                }
                                _ => {}
                            },
                        },

                        CurrentScreen::Browser => match app.input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Char('1') => app.browser_tab = BrowserTab::Overview,
                                KeyCode::Char('2') => app.browser_tab = BrowserTab::Dependencies,
                                KeyCode::Char('3') => app.browser_tab = BrowserTab::Files,
                                KeyCode::Char('4') => app.browser_tab = BrowserTab::Queue,
                                KeyCode::Char('a') => {
                                    app.selection_mode = match app.selection_mode {
                                        SelectionMode::Explicit => SelectionMode::AllVisible,
                                        SelectionMode::AllVisible => SelectionMode::Explicit,
                                    };
                                    app.selected_packages.clear();
                                    app.deselected_packages.clear();
                                }
                                KeyCode::Char('A' | 'c') => {
                                    app.selection_mode = SelectionMode::Explicit;
                                    app.selected_packages.clear();
                                    app.deselected_packages.clear();
                                }
                                KeyCode::PageDown => {
                                    for _ in 0..15 {
                                        app.next_item();
                                    }
                                }
                                KeyCode::PageUp => {
                                    for _ in 0..15 {
                                        app.previous_item();
                                    }
                                }
                                KeyCode::Char('d')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.browser_scroll = app.browser_scroll.saturating_add(15);
                                }
                                KeyCode::Char('u')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.browser_scroll = app.browser_scroll.saturating_sub(15);
                                }
                                KeyCode::Char('x') => {
                                    app.search_query.pop();
                                    app.update_search();
                                }
                                KeyCode::Tab => {
                                    app.filter_idx = (app.filter_idx + 1) % app.filters.len();
                                    app.update_search();
                                }
                                KeyCode::BackTab => {
                                    app.filter_idx = if app.filter_idx == 0 {
                                        app.filters.len() - 1
                                    } else {
                                        app.filter_idx - 1
                                    };
                                    app.update_search();
                                }
                                KeyCode::Char('S') => {
                                    app.sort_mode = match app.sort_mode {
                                        SortMode::Alphabetical => SortMode::Installed,
                                        SortMode::Installed => SortMode::Repository,
                                        SortMode::Repository => SortMode::Relevance,
                                        SortMode::Relevance => SortMode::Alphabetical,
                                    };
                                    app.update_search();
                                }
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Esc => app.screen = CurrentScreen::Dashboard,

                                KeyCode::Char('/' | 's' | 'f') => {
                                    app.input_mode = InputMode::Editing;
                                }

                                KeyCode::Down | KeyCode::Char('j') => {
                                    app.next_item();
                                    app.pending_g = false;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    app.previous_item();
                                    app.pending_g = false;
                                }
                                KeyCode::Char('g') => {
                                    app.go_to_top();
                                    app.pending_g = false;
                                }
                                KeyCode::Char('G') => {
                                    app.go_to_bottom();
                                    app.pending_g = false;
                                }

                                KeyCode::Char(' ') => {
                                    if let Some(idx) = app.list_state.selected()
                                        && let Some(pkg) = app.filtered_packages.get(idx)
                                    {
                                        let name = pkg.name.clone();
                                        match app.selection_mode {
                                            SelectionMode::Explicit => {
                                                if !app.selected_packages.remove(&name) {
                                                    app.selected_packages.insert(name);
                                                }
                                            }
                                            SelectionMode::AllVisible => {
                                                if !app.deselected_packages.remove(&name) {
                                                    app.deselected_packages.insert(name);
                                                }
                                            }
                                        }
                                    }
                                }

                                KeyCode::Char('i') => {
                                    let targets: Vec<String> = match app.selection_mode {
                                        SelectionMode::Explicit => {
                                            if !app.selected_packages.is_empty() {
                                                app.selected_packages.iter().cloned().collect()
                                            } else if let Some(idx) = app.list_state.selected() {
                                                if let Some(pkg) = app.filtered_packages.get(idx) {
                                                    vec![pkg.name.clone()]
                                                } else {
                                                    vec![]
                                                }
                                            } else {
                                                vec![]
                                            }
                                        }
                                        SelectionMode::AllVisible => app
                                            .filtered_packages
                                            .iter()
                                            .filter(|p| !app.deselected_packages.contains(&p.name))
                                            .map(|p| p.name.clone())
                                            .collect(),
                                    };

                                    if !targets.is_empty() {
                                        app.prompt_targets = targets;
                                        app.prompt_type = "install".to_string();
                                        app.show_prompt = true;
                                    }
                                }

                                KeyCode::Char('r') => {
                                    let targets: Vec<String> = match app.selection_mode {
                                        SelectionMode::Explicit => {
                                            if !app.selected_packages.is_empty() {
                                                app.selected_packages.iter().cloned().collect()
                                            } else if let Some(idx) = app.list_state.selected() {
                                                if let Some(pkg) = app.filtered_packages.get(idx) {
                                                    vec![pkg.name.clone()]
                                                } else {
                                                    vec![]
                                                }
                                            } else {
                                                vec![]
                                            }
                                        }
                                        SelectionMode::AllVisible => app
                                            .filtered_packages
                                            .iter()
                                            .filter(|p| !app.deselected_packages.contains(&p.name))
                                            .map(|p| p.name.clone())
                                            .collect(),
                                    };

                                    if !targets.is_empty() {
                                        app.prompt_targets = targets;
                                        app.prompt_type = "remove".to_string();
                                        app.show_prompt = true;
                                    }
                                }
                                _ => {
                                    app.pending_g = false;
                                }
                            },

                            InputMode::Editing => match key.code {
                                KeyCode::Esc | KeyCode::Enter => app.input_mode = InputMode::Normal,
                                KeyCode::Backspace | KeyCode::Delete => {
                                    app.search_query.pop();
                                    app.update_search();
                                }
                                KeyCode::Char(c) => {
                                    app.search_query.push(c);
                                    app.update_search();
                                }
                                _ => {}
                            },
                        },
                    }

                    if app.screen == CurrentScreen::Browser
                        && let Some(idx) = app.list_state.selected()
                        && let Some(pkg) = app.filtered_packages.get(idx)
                    {
                        let new_pkg = pkg.name.clone();
                        let tab_changed = prev_tab != app.browser_tab;

                        if prev_pkg != new_pkg
                            || (tab_changed && app.browser_tab == BrowserTab::Files)
                        {
                            if prev_pkg != new_pkg {
                                app.browser_pkg_name = new_pkg.clone();
                                app.browser_scroll = 0;
                            }

                            if app.browser_caches.len() > 64 {
                                app.browser_caches.clear();
                            }

                            let cache =
                                app.browser_caches
                                    .entry(new_pkg.clone())
                                    .or_insert_with(|| BrowserCache {
                                        info: String::new(),
                                        files: Vec::new(),
                                        dependencies: Vec::new(),
                                        loading_info: false,
                                        loading_files: false,
                                    });

                            if cache.info.is_empty() && !cache.loading_info {
                                cache.loading_info = true;
                                let tx_clone = tx.clone();
                                let pkg_name = new_pkg.clone();
                                tokio::spawn(async move {
                                    let mut cmd = tokio::process::Command::new("pacman");
                                    cmd.arg("-Qi").arg(&pkg_name);
                                    let mut info = String::new();
                                    if let Ok(res) = cmd.output().await {
                                        if res.status.success() {
                                            info = String::from_utf8_lossy(&res.stdout).to_string();
                                        } else {
                                            let mut cmd2 = tokio::process::Command::new("pacman");
                                            cmd2.arg("-Si").arg(&pkg_name);
                                            if let Ok(res2) = cmd2.output().await {
                                                info = String::from_utf8_lossy(&res2.stdout)
                                                    .to_string();
                                            }
                                        }
                                    }

                                    let mut deps = Vec::new();
                                    for line in info.lines() {
                                        if (line.starts_with("Depends On")
                                            || line.starts_with("Optional Deps")
                                            || line.starts_with("Required By"))
                                            && let Some((_, val)) = line.split_once(':')
                                        {
                                            for dep in val.split_whitespace() {
                                                if dep != "None" {
                                                    deps.push(dep.to_string());
                                                }
                                            }
                                        }
                                    }
                                    let _ = tx_clone
                                        .send(TuiEvent::BrowserInfoLoaded(pkg_name, info, deps))
                                        .await;
                                });
                            }

                            if app.browser_tab == BrowserTab::Files
                                && cache.files.is_empty()
                                && !cache.loading_files
                            {
                                cache.loading_files = true;
                                let tx_clone = tx.clone();
                                let pkg_name = new_pkg.clone();
                                tokio::spawn(async move {
                                    let mut cmd = tokio::process::Command::new("pacman");
                                    cmd.arg("-Ql").arg(&pkg_name);
                                    let mut files = Vec::new();
                                    if let Ok(res) = cmd.output().await {
                                        if res.status.success() {
                                            files = String::from_utf8_lossy(&res.stdout)
                                                .lines()
                                                .map(|l| {
                                                    l.replace(&pkg_name, "").trim().to_string()
                                                })
                                                .collect();
                                        } else {
                                            files.push("no files available (package might not be installed).".to_string());
                                        }
                                    }
                                    let _ = tx_clone
                                        .send(TuiEvent::BrowserFilesLoaded(pkg_name, files))
                                        .await;
                                });
                            }
                        }
                    }
                }
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
