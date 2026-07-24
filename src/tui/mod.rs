use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    widgets::ListState,
    Terminal,
};
use std::{io, time::Duration};

use crate::core;

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
}

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub repo: String,
}

/// the central state of the application.
pub struct App {
    pub should_quit: bool,
    pub screen: CurrentScreen,

    // browser state
    pub package_list: Vec<PackageInfo>,
    pub list_state: ListState,

    // vim state
    pub pending_g: bool,
    
    // transaction state
    pub is_installing: bool,
    pub current_action: String,
    pub progress: u16,
    pub transaction_logs: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        // placeholder packages for testing
        // let packages = vec![
        //     "firefox".to_string(), "kitty".to_string(), "htop".to_string(),
        //     "neovim".to_string(), "linux".to_string(), "mesa".to_string(),
        //     "wayland".to_string(), "hyprland".to_string(), "rust".to_string(),
        //     "gcc".to_string(), "git".to_string(), "curl".to_string(),
        // ];

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
        } else {
            // fallback
            package_list.push(PackageInfo {
                name: "error".to_string(),
                version: "0.0.0".to_string(),
                desc: "failed to load alpm database. are you on arch?".to_string(),
                repo: "unknown".to_string(),
            });
        }

        package_list.sort_by(|a, b| a.name.cmp(&b.name));
        
        let mut list_state = ListState::default();
        if !package_list.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            should_quit: false,
            screen: CurrentScreen::Dashboard,
            package_list,
            list_state,
            is_installing: false,
            current_action: String::from("idle"),
            progress: 0,
            transaction_logs: Vec::new(),
            pending_g: false;
        }
    }

    pub fn next_item(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.package_list.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
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
    loop {
        terminal.draw(|f| {
            match app.screen {
                CurrentScreen::Dashboard => dashboard::render(f, app),
                CurrentScreen::Browser => browser::render(f, app),
            }

            transaction::render_popup(f, app);
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    
                    KeyCode::Char('/') | KeyCode::Char('s') => app.screen = CurrentScreen::Browser,

                    // scrolling (j/k)
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.next_item();
                        app.pending_g = false;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.previous_item();
                        app.pending_g = false;
                    }
                    // top/bottom (gg/G)
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
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
