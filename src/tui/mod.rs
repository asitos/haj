use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    text::Text,
    widgets::ListState,
};
use std::{collections::HashSet, io, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt};
use tokio::sync::mpsc;

use crate::core;
use ansi_to_tui::IntoText;

pub mod browser;
pub mod dashboard;
pub mod transaction;

pub enum CurrentScreen {
    Dashboard,
    Browser,
}

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Clone, PartialEq)]
pub enum PackageFilter {
    All,
    Installed,
    NotInstalled,
}

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub repo: String,
    pub is_installed: bool,
}

pub enum TuiEvent {
    _Tick,
    Key(crossterm::event::KeyEvent),
    PacmanLog(String),
    PacmanProgress(u16),
    TransactionComplete,
    CloseTransaction,
    DashboardArtFrame(Text<'static>),
}

pub struct App {
    pub should_quit: bool,
    pub screen: CurrentScreen,
    pub input_mode: InputMode,
    pub filter: PackageFilter,
    pub pending_g: bool,
    pub orphan_count: usize,

    pub package_list: Vec<PackageInfo>,
    pub filtered_packages: Vec<PackageInfo>,
    pub search_query: String,
    pub list_state: ListState,

    pub is_installing: bool,
    pub current_action: String,
    pub progress: u16,
    pub transaction_logs: Vec<String>,
    pub dashboard_art: Text<'static>,
}

impl App {
    pub fn new() -> Self {
        let mut package_list = Vec::new();

        if let Ok(alpm) = core::alpm_init::init_alpm() {
            let local_db = alpm.localdb();
            let mut seen_packages = HashSet::new();

            for db in alpm.syncdbs() {
                for pkg in db.pkgs() {
                    let name = pkg.name().to_string();
                    let is_installed = local_db.pkg(name.as_str()).is_ok();

                    seen_packages.insert(name.clone());
                    package_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        desc: pkg.desc().unwrap_or("none").to_string(),
                        repo: db.name().to_string(),
                        is_installed,
                    });
                }
            }

            for pkg in local_db.pkgs() {
                let name = pkg.name().to_string();
                if !seen_packages.contains(&name) {
                    package_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        desc: pkg.desc().unwrap_or("none").to_string(),
                        repo: "local/aur".to_string(),
                        is_installed: true,
                    });
                }
            }
        }

        package_list.sort_by(|a, b| a.name.cmp(&b.name));

        let filtered_packages = package_list.clone();

        let mut list_state = ListState::default();
        if !package_list.is_empty() {
            list_state.select(Some(0));
        }

        let orphan_count = std::process::Command::new("pacman")
            .arg("-Qdtq")
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .count()
            })
            .unwrap_or(0);

        Self {
            should_quit: false,
            screen: CurrentScreen::Dashboard,
            input_mode: InputMode::Normal,
            filter: PackageFilter::All,
            pending_g: false,
            orphan_count,
            package_list,
            filtered_packages,
            search_query: String::new(),
            list_state,
            is_installing: false,
            current_action: String::from("idle"),
            progress: 0,
            transaction_logs: Vec::new(),
            dashboard_art: Text::raw(" loading art... "),
        }
    }

    pub fn update_search(&mut self) {
        let query = self.search_query.to_lowercase();

        self.filtered_packages = self
            .package_list
            .iter()
            .filter(|p| {
                let matches_query = query.is_empty()
                    || p.name.to_lowercase().contains(&query)
                    || p.desc.to_lowercase().contains(&query);
                let matches_filter = match self.filter {
                    PackageFilter::All => true,
                    PackageFilter::Installed => p.is_installed,
                    PackageFilter::NotInstalled => !p.is_installed,
                };
                matches_query && matches_filter
            })
            .cloned()
            .collect();

        self.list_state
            .select(if self.filtered_packages.is_empty() {
                None
            } else {
                Some(0)
            });
    }

    pub fn next_item(&mut self) {
        if self.filtered_packages.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.filtered_packages.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        if self.filtered_packages.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_packages.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn go_to_top(&mut self) {
        if !self.filtered_packages.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn go_to_bottom(&mut self) {
        if !self.filtered_packages.is_empty() {
            self.list_state
                .select(Some(self.filtered_packages.len() - 1));
        }
    }
}

pub async fn run() -> Result<()> {
    println!("🦈 haj requires root privileges for package management.");
    let status = std::process::Command::new("sudo").arg("-v").status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "sudo authentication failed or was cancelled."
        ));
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    std::process::exit(0);
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    <B as Backend>::Error: Send + Sync + 'static,
{
    let config = crate::config::load_config();
    let _use_3d_animation = config.animations;

    let (tx, mut rx) = mpsc::channel::<TuiEvent>(100);

    let tx_input = tx.clone();
    tokio::spawn(async move {
        loop {
            if tx_input.is_closed() {
                break;
            }

            if event::poll(Duration::from_millis(50)).unwrap_or(false)
                && let Ok(Event::Key(key)) = event::read()
                && tx_input.send(TuiEvent::Key(key)).await.is_err() {
                break;
            }
        }
    });

    let tx_art = tx.clone();
    tokio::spawn(async move {
        let use_3d_animation = config.animations;

        if use_3d_animation {
            let temp_dir = std::env::temp_dir();
            let obj_path = temp_dir.join("blahaj.obj");
            let mtl_path = temp_dir.join("blahaj.mtl");

            if !obj_path.exists() {
                let _ = std::fs::write(&obj_path, include_bytes!("../../resources/blahaj.obj"));
            }
            if !mtl_path.exists() {
                let _ = std::fs::write(&mtl_path, include_bytes!("../../resources/blahaj.mtl"));
            }

            let obj_path_str = obj_path.to_string_lossy().to_string();

            let child = tokio::process::Command::new("display3d")
                .args([&obj_path_str, "-t", "0,0.5,7.5"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn();

            match child {
                Ok(mut child_proc) => {
                    let mut stdout = child_proc.stdout.take().unwrap();
                    let mut buf = vec![0; 8192];
                    let mut frame_buffer = Vec::new();

                    while let Ok(n) = stdout.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        frame_buffer.extend_from_slice(&buf[..n]);

                        while let Some(pos) = frame_buffer.windows(3).position(|w| w == b"\x1b[H") {
                            let frame = frame_buffer[..pos].to_vec();
                            frame_buffer.drain(..=pos + 2);

                            if let Ok(text) = frame.into_text() 
                                && tx_art
                                    .send(TuiEvent::DashboardArtFrame(text))
                                    .await
                                    .is_err()
                            {
                                break;
                                
                            }
                        }
                    }
                }
                Err(_) => {
                    let _ = tx_art
                        .send(TuiEvent::DashboardArtFrame(Text::raw(
                            " error: display3d binary not found in PATH ",
                        )))
                        .await;
                }
            }
        } else {
            let art_str = std::fs::read_to_string("../../resources/ascii.txt")
                .unwrap_or_else(|_| " SHARK ASCII MISSING ".to_string());
            if let Ok(text) = art_str.into_bytes().into_text() {
                let _ = tx_art.send(TuiEvent::DashboardArtFrame(text)).await;
            }
        }
    });

    let spawn_pacman =
        |tx_channel: mpsc::Sender<TuiEvent>, args: Vec<String>, _action_name: String| {
            tokio::spawn(async move {
                let mut child = tokio::process::Command::new("sudo")
                    .args(args)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                    .expect("failed to spawn pacman");

                if let Some(stdout) = child.stdout.take() {
                    let mut reader = tokio::io::BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let clean = line.trim();
                        if clean.is_empty() {
                            continue;
                        }

                        if clean.contains("resolving dependencies")
                            || clean.contains("looking for conflicting")
                            || clean.contains("checking keyring")
                            || clean.contains("checking package integrity")
                            || clean.contains("loading package files")
                        {
                            continue;
                        }

                        let _ = tx_channel
                            .send(TuiEvent::PacmanLog(clean.to_string()))
                            .await;

                        if clean.contains('%') {
                            let parts: Vec<&str> = clean.split('%').collect();
                            if !parts.is_empty() {
                                let num_str = parts[0].split_whitespace().last().unwrap_or("0");
                                if let Ok(val) = num_str.parse::<u16>() {
                                    let _ = tx_channel.send(TuiEvent::PacmanProgress(val)).await;
                                }
                            }
                        }
                    }
                }

                let _ = child.wait().await;
                let _ = tx_channel.send(TuiEvent::PacmanProgress(100)).await;
                let _ = tx_channel.send(TuiEvent::TransactionComplete).await;

                // Wait 2 seconds so the user can read the success message, then close popup
                tokio::time::sleep(Duration::from_secs(2)).await;
                let _ = tx_channel.send(TuiEvent::CloseTransaction).await;
            });
        };

    loop {
        terminal.draw(|f| {
            match app.screen {
                CurrentScreen::Dashboard => dashboard::render(f, app),
                CurrentScreen::Browser => browser::render(f, app),
            }
            transaction::render_popup(f, app);
        })?;

        if let Some(event) = rx.recv().await {
            match event {
                TuiEvent::DashboardArtFrame(text) => {
                    app.dashboard_art = text;
                }
                TuiEvent::PacmanLog(log) => {
                    app.transaction_logs.push(log);
                    if app.transaction_logs.len() > 30 {
                        app.transaction_logs.remove(0);
                    }
                }
                TuiEvent::PacmanProgress(val) => {
                    app.progress = val.min(100);
                }
                TuiEvent::TransactionComplete => {
                    app.current_action = "changes complete ✓".to_string();
                }
                TuiEvent::CloseTransaction => {
                    app.is_installing = false;
                    app.progress = 0;

                    if let Ok(output) = std::process::Command::new("pacman").arg("-Qdtq").output() {
                        app.orphan_count = String::from_utf8_lossy(&output.stdout)
                            .split_whitespace()
                            .count();
                    }

                    if let Ok(alpm) = crate::core::alpm_init::init_alpm() {
                        let local_db = alpm.localdb();
                        for pkg in &mut app.package_list {
                            pkg.is_installed = local_db.pkg(pkg.name.as_str()).is_ok();
                        }
                        app.update_search();
                    }
                }
                TuiEvent::Key(key) => match app.screen {
                    CurrentScreen::Dashboard => match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char('/') | KeyCode::Char('f') => {
                            app.screen = CurrentScreen::Browser;
                            app.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('u') => {
                            app.is_installing = true;
                            app.current_action = "syncing & updating system...".to_string();
                            app.transaction_logs.clear();
                            app.progress = 0;
                            spawn_pacman(
                                tx.clone(),
                                vec!["pacman".into(), "-Syu".into(), "--noconfirm".into()],
                                "updating system".into(),
                            );
                        }
                        KeyCode::Char('c') => {
                            if let Ok(output) =
                                std::process::Command::new("pacman").arg("-Qdtq").output()
                            {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let orphans: Vec<String> =
                                    stdout.split_whitespace().map(|s| s.to_string()).collect();

                                app.is_installing = true;
                                app.transaction_logs.clear();
                                app.progress = 0;

                                if orphans.is_empty() {
                                    app.current_action = "system clean ✓".to_string();
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
                                    let mut args =
                                        vec!["pacman".into(), "-Rns".into(), "--noconfirm".into()];
                                    args.extend(orphans);
                                    spawn_pacman(tx.clone(), args, "cleaning orphans".into());
                                }
                            }
                        }
                        _ => {}
                    },
                    CurrentScreen::Browser => match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('x') => {
                                app.search_query.pop();
                                app.update_search();
                            }
                            KeyCode::Tab => {
                                app.filter = match app.filter {
                                    PackageFilter::All => PackageFilter::Installed,
                                    PackageFilter::Installed => PackageFilter::NotInstalled,
                                    PackageFilter::NotInstalled => PackageFilter::All,
                                };
                                app.update_search();
                            }
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Esc => app.screen = CurrentScreen::Dashboard,

                            KeyCode::Char('/') | KeyCode::Char('s') | KeyCode::Char('f') => {
                                app.input_mode = InputMode::Editing
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
                                if app.pending_g {
                                    app.go_to_top();
                                    app.pending_g = false;
                                } else {
                                    app.pending_g = true;
                                }
                            }
                            KeyCode::Char('G') => {
                                app.go_to_bottom();
                                app.pending_g = false;
                            }

                            KeyCode::Char('i') => {
                                if let Some(idx) = app.list_state.selected() 
                                    && let Some(pkg) = app.filtered_packages.get(idx) {
                                    app.is_installing = true;
                                    app.current_action = format!("installing {}...", pkg.name);
                                    app.transaction_logs.clear();
                                    app.progress = 0;
                                    spawn_pacman(
                                        tx.clone(),
                                        vec![
                                        "pacman".into(),
                                        "-S".into(),
                                        "--noconfirm".into(),
                                        pkg.name.clone(),
                                        ],
                                        "installing".into(),
                                    );
                                }
                            }

                            KeyCode::Char('r') => {
                                if let Some(idx) = app.list_state.selected()
                                    && let Some(pkg) = app.filtered_packages.get(idx) {
                                    app.is_installing = true;
                                    app.current_action = format!("tossing {}...", pkg.name);
                                    app.transaction_logs.clear();
                                    app.progress = 0;
                                    spawn_pacman(
                                        tx.clone(),
                                        vec![
                                        "pacman".into(),
                                        "-Rs".into(),
                                        "--noconfirm".into(),
                                        pkg.name.clone(),
                                        ],
                                        "removing".into(),
                                    );
                                }
                            }

                            KeyCode::Char('u') => {
                                if let Some(idx) = app.list_state.selected() 
                                    && let Some(pkg) = app.filtered_packages.get(idx) {
                                    app.is_installing = true;
                                    app.current_action = format!("updating {}...", pkg.name);
                                    app.transaction_logs.clear();
                                    app.progress = 0;
                                    spawn_pacman(
                                        tx.clone(),
                                        vec![
                                        "pacman".into(),
                                        "-S".into(),
                                        "--noconfirm".into(),
                                        pkg.name.clone(),
                                        ],
                                        "updating".into(),
                                    );
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
                },
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
