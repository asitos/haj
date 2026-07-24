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
    PacmanProgress(u16), // 0 to 100
    TransactionComplete,
}

/// the central state of the application.
pub struct App {
    pub should_quit: bool,
    pub screen: CurrentScreen,

    // browser state
    pub package_list: Vec<String>,
    pub list_state: ListState,
    
    // transaction state
    pub is_installing: bool,
    pub current_action: String,
    pub progress: u16,
    pub transaction_logs: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        // placeholder packages for testing
        let packages = vec![
            "firefox".to_string(), "kitty".to_string(), "htop".to_string(),
            "neovim".to_string(), "linux".to_string(), "mesa".to_string(),
            "wayland".to_string(), "hyprland".to_string(), "rust".to_string(),
            "gcc".to_string(), "git".to_string(), "curl".to_string(),
        ];

        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        Self {
            should_quit: false,
            screen: CurrentScreen::Dashboard,
            package_list: packages,
            list_state,
            is_installing: false,
            current_action: String::from("idle"),
            progress: 0,
            transaction_logs: Vec::new(),
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
                    
                    KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
                    
                    KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous_item(),
                    
                    KeyCode::Char('i') => {
                        app.is_installing = !app.is_installing;
                        app.current_action = "resolving dependencies...".to_string();
                        app.progress = 45;
                    } 
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
