use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    widgets::ListState,
    text::Text, 
    Terminal,
};
use std::{io, time::Duration};
use tokio::sync::mpsc; 
use tokio::io::AsyncReadExt; 

use crate::core;
use ansi_to_tui::IntoText;

pub mod browser;
pub mod dashboard;
pub mod transaction;

pub enum CurrentScreen {
    Dashboard,
    Browser,
}

pub enum TuiEvent {
    Tick,
    Key(crossterm::event::KeyEvent),
    PacmanLog(String),
    PacmanProgress(u16),
    TransactionComplete,
    DashboardArtFrame(Text<'static>),
}

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub repo: String,
}

pub struct App {
    pub should_quit: bool,
    pub screen: CurrentScreen,
    
    pub pending_g: bool,
    
    pub package_list: Vec<PackageInfo>,
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
            for pkg in local_db.pkgs() {
                package_list.push(PackageInfo {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    desc: pkg.desc().unwrap_or("none").to_string(),
                    repo: "local".to_string(),
                });
            }
        }

        package_list.sort_by(|a, b| a.name.cmp(&b.name));
        
        let mut list_state = ListState::default();
        if !package_list.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            should_quit: false,
            screen: CurrentScreen::Dashboard,
            pending_g: false, 
            package_list,
            list_state,
            is_installing: false,
            current_action: String::from("idle"),
            progress: 0,
            transaction_logs: Vec::new(),
            dashboard_art: Text::raw(" loading art... "),
        }
    }
    
    pub fn next_item(&mut self) {
        if self.package_list.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.package_list.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        if self.package_list.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.package_list.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn go_to_top(&mut self) {
        if !self.package_list.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn go_to_bottom(&mut self) {
        if !self.package_list.is_empty() {
            self.list_state.select(Some(self.package_list.len() - 1));
        }
    }
}

pub async fn run() -> Result<()> {
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

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> 
where
    <B as Backend>::Error: Send + Sync + 'static,
{
    let (tx, mut rx) = mpsc::channel::<TuiEvent>(100);

    let tx_input = tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    let _ = tx_input.send(TuiEvent::Key(key)).await;
                }
            }
        }
    });

    let tx_art = tx.clone();
    tokio::spawn(async move {
        let use_3d_animation = true; 

        if use_3d_animation {
            let mut child = tokio::process::Command::new("display3d")
                .args(&["./resources/blahaj.obj", "-t", "0,0,5.5"]) 
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            if let Ok(mut child_proc) = child {
                let mut stdout = child_proc.stdout.take().unwrap();
                let mut buf = vec![0; 8192];
                let mut frame_buffer = Vec::new();

                while let Ok(n) = stdout.read(&mut buf).await {
                    if n == 0 { break; }
                    frame_buffer.extend_from_slice(&buf[..n]);

                    while let Some(pos) = frame_buffer.windows(3).position(|w| w == b"\x1b[H") {
                        let frame = frame_buffer[..pos].to_vec();
                        frame_buffer.drain(..=pos + 2); 

                        if let Ok(text) = frame.into_text() {
                            let _ = tx_art.send(TuiEvent::DashboardArtFrame(text)).await;
                        }
                    }
                }
            }
        } else {
            let art_str = std::fs::read_to_string("./resources/ascii.txt").unwrap_or_else(|_| " SHARK ASCII MISSING ".to_string());
            if let Ok(text) = art_str.into_bytes().into_text() {
                let _ = tx_art.send(TuiEvent::DashboardArtFrame(text)).await;
            }
        }
    });

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
                TuiEvent::Key(key) => {
                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char('/') | KeyCode::Char('s') => app.screen = CurrentScreen::Browser,
                        KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
                        
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
                            app.is_installing = !app.is_installing;
                            app.current_action = "resolving dependencies...".to_string();
                            app.progress = 45;
                        }
                        _ => { app.pending_g = false; }
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
