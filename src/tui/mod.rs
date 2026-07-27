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

#[derive(Clone, PartialEq, Debug)]
pub enum PackageFilter {
    All,
    Installed,
    NotInstalled,
    Updates,
    Aur,
    Repositories,
    Repo(String),
}

impl std::fmt::Display for PackageFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Installed => write!(f, "installed"),
            Self::NotInstalled => write!(f, "not installed"),
            Self::Updates => write!(f, "updates"),
            Self::Aur => write!(f, "aur"),
            Self::Repositories => write!(f, "repositories"),
            Self::Repo(name) => write!(f, "repo:{}", name),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum SortMode {
    Alphabetical,
    Installed,
    Repository,
    Relevance,
}

impl std::fmt::Display for SortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Alphabetical => write!(f, "alphabetical"),
            Self::Installed => write!(f, "installed"),
            Self::Repository => write!(f, "repository"),
            Self::Relevance => write!(f, "relevance"),
        }
    }
}

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub repo: String,
    pub is_installed: bool,
    pub is_upgradable: bool,
    pub size_mb: f64,
}

pub enum TuiEvent {
    _Tick,
    Key(crossterm::event::KeyEvent),
    PacmanLog(String),
    UpdateAction(String), 
    PacmanProgress(u16),
    TransactionComplete,
    TransactionFailed,
    CloseTransaction,
    DashboardArtFrame(Text<'static>),
}

pub struct App {
    pub should_quit: bool,
    pub screen: CurrentScreen,
    pub input_mode: InputMode,
    
    pub filters: Vec<PackageFilter>,
    pub filter_idx: usize,
    pub sort_mode: SortMode,
    
    pub pending_g: bool,
    pub orphan_count: usize,

    pub package_list: Vec<PackageInfo>,
    pub filtered_packages: Vec<PackageInfo>,
    pub search_query: String,
    pub list_state: ListState,
    pub selected_packages: HashSet<String>,

    pub show_prompt: bool,
    pub prompt_type: String,
    pub prompt_targets: Vec<String>,

    pub is_installing: bool,
    pub current_action: String,
    pub progress: u16,
    pub transaction_logs: Vec<String>,
    pub dashboard_art: Text<'static>,

    pub abort_tx: Option<mpsc::Sender<()>>,
}

impl App {
    pub fn new() -> Self {
        let mut dynamic_filters = vec![
            PackageFilter::All,
            PackageFilter::Installed,
            PackageFilter::NotInstalled,
            PackageFilter::Updates,
            PackageFilter::Aur,
            PackageFilter::Repositories,
        ];

        if let Ok(alpm) = core::alpm_init::init_alpm() {
            for db in alpm.syncdbs() {
                dynamic_filters.push(PackageFilter::Repo(db.name().to_string()));
            }
        }

        let mut app = Self {
            should_quit: false,
            screen: CurrentScreen::Dashboard,
            input_mode: InputMode::Normal,
            filters: dynamic_filters,
            filter_idx: 0,
            sort_mode: SortMode::Relevance,
            pending_g: false,
            orphan_count: 0,
            package_list: Vec::new(),
            filtered_packages: Vec::new(),
            search_query: String::new(),
            list_state: ListState::default(),
            selected_packages: HashSet::new(),
            show_prompt: false,
            prompt_type: String::new(),
            prompt_targets: Vec::new(),
            is_installing: false,
            current_action: String::from("idle"),
            progress: 0,
            transaction_logs: Vec::new(),
            dashboard_art: Text::raw(" loading art... "),
            abort_tx: None,
        };

        app.refresh_state();
        app
    }

    pub fn refresh_state(&mut self) {
        if let Ok(output) = std::process::Command::new("pacman").arg("-Qdtq").output() {
            self.orphan_count = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .count();
        } else {
            self.orphan_count = 0;
        }

        if let Ok(alpm) = core::alpm_init::init_alpm() {
            let local_db = alpm.localdb();
            let mut seen_packages = HashSet::new();
            let mut new_list = Vec::new();

            for db in alpm.syncdbs() {
                for pkg in db.pkgs() {
                    let name = pkg.name().to_string();
                    let local_pkg = local_db.pkg(name.as_str());
                    let is_installed = local_pkg.is_ok();
                    let mut is_upgradable = false;

                    if let Ok(l_pkg) = local_pkg {
                        if alpm::vercmp(pkg.version().to_string(), l_pkg.version().to_string()) == std::cmp::Ordering::Greater {
                            is_upgradable = true;
                        }
                    }

                    seen_packages.insert(name.clone());
                    new_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        desc: pkg.desc().unwrap_or("none").to_string(),
                        repo: db.name().to_string(),
                        is_installed,
                        is_upgradable,
                        size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }

            for pkg in local_db.pkgs() {
                let name = pkg.name().to_string();
                if !seen_packages.contains(&name) {
                    new_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        desc: pkg.desc().unwrap_or("none").to_string(),
                        repo: "local/aur".to_string(),
                        is_installed: true,
                        is_upgradable: false,
                        size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }

            new_list.sort_by(|a, b| a.name.cmp(&b.name));
            self.package_list = new_list;
        }

        self.update_search();
    }

    pub fn update_search(&mut self) {
        let query = self.search_query.to_lowercase();
        let current_filter = &self.filters[self.filter_idx];

        self.filtered_packages = self
            .package_list
            .iter()
            .filter(|p| {
                let matches_query = query.is_empty() || p.name.to_lowercase().contains(&query);
                
                let matches_filter = match current_filter {
                    PackageFilter::All => true,
                    PackageFilter::Installed => p.is_installed,
                    PackageFilter::NotInstalled => !p.is_installed,
                    PackageFilter::Updates => p.is_upgradable,
                    PackageFilter::Aur => p.repo == "local/aur",
                    PackageFilter::Repositories => p.repo != "local/aur",
                    PackageFilter::Repo(name) => &p.repo == name,
                };
                matches_query && matches_filter
            })
            .cloned()
            .collect();

        match self.sort_mode {
            SortMode::Alphabetical => {
                self.filtered_packages.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortMode::Repository => {
                self.filtered_packages.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name)));
            }
            SortMode::Installed => {
                self.filtered_packages.sort_by(|a, b| b.is_installed.cmp(&a.is_installed).then(a.name.cmp(&b.name)));
            }
            SortMode::Relevance => {
                if query.is_empty() {
                    self.filtered_packages.sort_by(|a, b| a.name.cmp(&b.name));
                } else {
                    self.filtered_packages.sort_by(|a, b| {
                        let a_name = a.name.to_lowercase();
                        let b_name = b.name.to_lowercase();
                        let a_exact = a_name == query;
                        let b_exact = b_name == query;
                        let a_starts = a_name.starts_with(&query);
                        let b_starts = b_name.starts_with(&query);

                        b_exact.cmp(&a_exact)
                            .then(b_starts.cmp(&a_starts))
                            .then(a_name.len().cmp(&b_name.len()))
                            .then(a.name.cmp(&b_name))
                    });
                }
            }
        }

        if self.filtered_packages.is_empty() {
            self.list_state.select(None);
        } else {
            let current_idx = self.list_state.selected().unwrap_or(0);
            if current_idx >= self.filtered_packages.len() {
                self.list_state.select(Some(self.filtered_packages.len() - 1));
            } else {
                self.list_state.select(Some(current_idx));
            }
        }
    }

    pub fn next_item(&mut self) {
        if self.filtered_packages.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.filtered_packages.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        if self.filtered_packages.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.filtered_packages.len() - 1 } else { i - 1 },
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
            self.list_state.select(Some(self.filtered_packages.len() - 1));
        }
    }
}

pub async fn run() -> Result<()> {
    println!("🦈 haj requires root privileges for package management.");
    let status = std::process::Command::new("sudo").arg("-v").status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("sudo authentication failed or was cancelled."));
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
    let _use_3d_animation = config.general.animations;

    let (tx, mut rx) = mpsc::channel::<TuiEvent>(100);

    let tx_input = tx.clone();
    tokio::spawn(async move {
        loop {
            if tx_input.is_closed() { break; }
            if event::poll(Duration::from_millis(50)).unwrap_or(false)
                && let Ok(Event::Key(key)) = event::read()
                && tx_input.send(TuiEvent::Key(key)).await.is_err()
            {
                break;
            }
        }
    });

    let tx_art = tx.clone();
    tokio::spawn(async move {
        let use_3d_animation = config.general.animations;

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
                        if n == 0 { break; }
                        frame_buffer.extend_from_slice(&buf[..n]);

                        while let Some(pos) = frame_buffer.windows(3).position(|w| w == b"\x1b[H") {
                            let frame = frame_buffer[..pos].to_vec();
                            frame_buffer.drain(..=pos + 2);

                            if let Ok(text) = frame.into_text()
                                && tx_art.send(TuiEvent::DashboardArtFrame(text)).await.is_err()
                            {
                                break;
                            }
                        }
                    }
                }
                Err(_) => {
                    let _ = tx_art.send(TuiEvent::DashboardArtFrame(Text::raw(" error: display3d binary not found in PATH "))).await;
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
        |tx_channel: mpsc::Sender<TuiEvent>, mut args: Vec<String>, _action_name: String, mut abort_rx: mpsc::Receiver<()>| {
            tokio::spawn(async move {
                if !args.contains(&"--color=never".to_string()) {
                    args.push("--color=never".into());
                }

                let mut child_res = tokio::process::Command::new("sudo")
                    .args(args)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true)
                    .spawn();

                if let Ok(mut child) = child_res {
                    let pid = child.id();
                    let stdout = child.stdout.take().unwrap();
                    let stderr = child.stderr.take().unwrap();

                    let abort_task = tokio::spawn(async move {
                        if let Some(()) = abort_rx.recv().await {
                            if let Some(p) = pid {
                                let _ = tokio::process::Command::new("sudo")
                                    .args(["kill", "-INT", &p.to_string()])
                                    .status()
                                    .await;
                            }
                        }
                    });

                    let tx_out = tx_channel.clone();
                    let out_task = tokio::spawn(async move {
                        let mut reader = tokio::io::BufReader::new(stdout).lines();
                        let mut in_hook_phase = false;

                        while let Ok(Some(line)) = reader.next_line().await {
                            let clean = line.trim();
                            if clean.is_empty() { continue; }
                            let lower_clean = clean.to_lowercase();

                            if lower_clean.contains("resolving dependencies") || lower_clean.contains("conflicting packages") {
                                let _ = tx_out.send(TuiEvent::UpdateAction("resolving package dependencies...".into())).await;
                            } else if lower_clean.contains("checking keys") || lower_clean.contains("checking package integrity") {
                                let _ = tx_out.send(TuiEvent::UpdateAction("verifying package integrity...".into())).await;
                            } else if lower_clean.contains("loading package files") {
                                let _ = tx_out.send(TuiEvent::UpdateAction("loading package files...".into())).await;
                            } else if lower_clean.contains("checking for file conflicts") {
                                let _ = tx_out.send(TuiEvent::UpdateAction("checking for file conflicts...".into())).await;
                            } else if lower_clean.contains("checking available disk space") {
                                let _ = tx_out.send(TuiEvent::UpdateAction("checking available disk space...".into())).await;
                            } else if lower_clean.contains("retrieving packages") {
                                let _ = tx_out.send(TuiEvent::UpdateAction("downloading packages...".into())).await;
                            } else if lower_clean.contains("processing package changes") {
                                let _ = tx_out.send(TuiEvent::UpdateAction("processing package changes...".into())).await;
                            } else if clean.starts_with('(') && (lower_clean.contains(") upgrading") || lower_clean.contains(") installing") || lower_clean.contains(") removing")) {
                                if let Some(idx_end) = clean.find(')') {
                                    let counter = &clean[..=idx_end];
                                    let action = if lower_clean.contains("installing") { "installing" }
                                                 else if lower_clean.contains("removing") { "removing" }
                                                 else { "upgrading" };
                                    let _ = tx_out.send(TuiEvent::UpdateAction(format!("{} packages {}...", counter, action))).await;
                                }
                            } else if lower_clean.contains("running pre-transaction hooks") {
                                in_hook_phase = true;
                                let _ = tx_out.send(TuiEvent::UpdateAction("running pre-transaction hooks...".into())).await;
                            } else if lower_clean.contains("running post-transaction hooks") {
                                in_hook_phase = true;
                                let _ = tx_out.send(TuiEvent::UpdateAction("running post-transaction hooks...".into())).await;
                            } else if in_hook_phase && (clean.starts_with("==> Building image") || clean.starts_with("==> Install DKMS") || clean.starts_with("==> Generating")) {
                                let _ = tx_out.send(TuiEvent::UpdateAction(clean.to_string())).await;
                            }

                            if lower_clean.contains("error:") || lower_clean.contains("warning:") || lower_clean.contains("failed") {
                                let _ = tx_out.send(TuiEvent::PacmanLog(clean.to_string())).await;
                            } else if in_hook_phase && (lower_clean.contains("missing") || lower_clean.contains("not found")) {
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
                            if clean.is_empty() { continue; }
                            let _ = tx_err.send(TuiEvent::PacmanLog(format!("error: {}", clean))).await;
                        }
                    });

                    let _ = tokio::join!(out_task, err_task);
                    
                    let status = child.wait().await.unwrap_or_else(|_| {
                        std::os::unix::process::ExitStatusExt::from_raw(1)
                    });
                    
                    abort_task.abort();

                    let _ = tx_channel.send(TuiEvent::PacmanProgress(100)).await;
                    
                    if status.success() {
                        let _ = tx_channel.send(TuiEvent::TransactionComplete).await;
                        tokio::time::sleep(Duration::from_secs(2)).await; 
                    } else {
                        let _ = tx_channel.send(TuiEvent::TransactionFailed).await;
                        tokio::time::sleep(Duration::from_secs(5)).await; 
                    }
                } else {
                    let _ = tx_channel.send(TuiEvent::TransactionFailed).await;
                    let _ = tx_channel.send(TuiEvent::PacmanLog("fatal: failed to spawn sudo process".into())).await;
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
            }
            transaction::render_popup(f, app);
            transaction::render_confirm_popup(f, app);
        })?;

        if let Some(event) = rx.recv().await {
            match event {
                TuiEvent::DashboardArtFrame(text) => app.dashboard_art = text,
                TuiEvent::PacmanLog(log) => {
                    app.transaction_logs.push(log);
                    if app.transaction_logs.len() > 30 { app.transaction_logs.remove(0); }
                }
                TuiEvent::UpdateAction(action) => {
                    app.current_action = action;
                }
                TuiEvent::PacmanProgress(val) => app.progress = val.min(100),
                TuiEvent::TransactionComplete => app.current_action = "changes complete ✓".to_string(),
                TuiEvent::TransactionFailed => app.current_action = "transaction failed ❌".to_string(),
                
                TuiEvent::CloseTransaction => {
                    app.is_installing = false;
                    app.progress = 0;
                    app.refresh_state();
                }
                TuiEvent::Key(key) => {
                    if app.is_installing {
                        if let KeyCode::Char('q') = key.code {
                            if let Some(tx_abort) = app.abort_tx.take() {
                                let _ = tx_abort.try_send(());
                                app.current_action = "aborting safely...".to_string();
                                app.transaction_logs.push("".into());
                                app.transaction_logs.push("==> user triggered abort. sending SIGINT to pacman...".into());
                            }
                        }
                        continue;
                    }

                    if app.show_prompt {
                        match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                                app.show_prompt = false;
                                app.is_installing = true;
                                app.transaction_logs.clear();
                                app.progress = 0;
                                
                                app.current_action = if app.prompt_targets.len() > 1 {
                                    format!("{}ing {} packages...", app.prompt_type, app.prompt_targets.len())
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
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
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
                            KeyCode::Char('/') | KeyCode::Char('f') => {
                                app.screen = CurrentScreen::Browser;
                                app.input_mode = InputMode::Editing;
                            }
                            KeyCode::Char('u') => {
                                app.is_installing = true;
                                app.current_action = "syncing & updating system...".to_string();
                                app.transaction_logs.clear();
                                app.progress = 0;
                                
                                let (abort_tx, abort_rx) = mpsc::channel(1);
                                app.abort_tx = Some(abort_tx);
                                spawn_pacman(
                                    tx.clone(),
                                    vec!["pacman".into(), "-Syu".into(), "--noconfirm".into()],
                                    "updating system".into(),
                                    abort_rx
                                );
                            }
                            KeyCode::Char('c') => {
                                if let Ok(output) = std::process::Command::new("pacman").arg("-Qdtq").output() {
                                    let stdout = String::from_utf8_lossy(&output.stdout);
                                    let orphans: Vec<String> = stdout.split_whitespace().map(|s| s.to_string()).collect();

                                    app.is_installing = true;
                                    app.transaction_logs.clear();
                                    app.progress = 0;

                                    if orphans.is_empty() {
                                        app.current_action = "system clean ✓".to_string();
                                        app.progress = 100;
                                        app.transaction_logs.push("no orphaned packages to remove.".to_string());

                                        let tx_clone = tx.clone();
                                        tokio::spawn(async move {
                                            tokio::time::sleep(Duration::from_secs(2)).await;
                                            let _ = tx_clone.send(TuiEvent::CloseTransaction).await;
                                        });
                                    } else {
                                        app.current_action = format!("sweeping {} orphans...", orphans.len());
                                        let mut args = vec!["pacman".into(), "-Rns".into(), "--noconfirm".into()];
                                        args.extend(orphans);
                                        
                                        let (abort_tx, abort_rx) = mpsc::channel(1);
                                        app.abort_tx = Some(abort_tx);
                                        spawn_pacman(tx.clone(), args, "cleaning orphans".into(), abort_rx);
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
                                    app.filter_idx = (app.filter_idx + 1) % app.filters.len();
                                    app.update_search();
                                }
                                KeyCode::BackTab => {
                                    app.filter_idx = if app.filter_idx == 0 { app.filters.len() - 1 } else { app.filter_idx - 1 };
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

                                KeyCode::Char(' ') => {
                                    if let Some(idx) = app.list_state.selected() {
                                        if let Some(pkg) = app.filtered_packages.get(idx) {
                                            let name = pkg.name.clone();
                                            if app.selected_packages.contains(&name) {
                                                app.selected_packages.remove(&name);
                                            } else {
                                                app.selected_packages.insert(name);
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('c') => {
                                    app.selected_packages.clear();
                                }

                                KeyCode::Char('i') => {
                                    let targets: Vec<String> = if !app.selected_packages.is_empty() {
                                        app.selected_packages.iter().cloned().collect()
                                    } else if let Some(idx) = app.list_state.selected()
                                        && let Some(pkg) = app.filtered_packages.get(idx)
                                    {
                                        vec![pkg.name.clone()]
                                    } else {
                                        vec![]
                                    };

                                    if !targets.is_empty() {
                                        app.prompt_targets = targets;
                                        app.prompt_type = "install".to_string();
                                        app.show_prompt = true;
                                    }
                                }

                                KeyCode::Char('r') => {
                                    let targets: Vec<String> = if !app.selected_packages.is_empty() {
                                        app.selected_packages.iter().cloned().collect()
                                    } else if let Some(idx) = app.list_state.selected()
                                        && let Some(pkg) = app.filtered_packages.get(idx)
                                    {
                                        vec![pkg.name.clone()]
                                    } else {
                                        vec![]
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
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
