use crossterm::style::Stylize;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

pub struct ProgressBar {
    message: Arc<Mutex<String>>,
    stop: Arc<Mutex<bool>>,
    handle: Option<JoinHandle<()>>,
}

impl ProgressBar {
    pub fn new_spinner() -> Self {
        ProgressBar {
            message: Arc::new(Mutex::new(String::new())),
            stop: Arc::new(Mutex::new(false)),
            handle: None,
        }
    }

    pub fn enable_steady_tick(&mut self, _duration: Duration) {
        let msg_clone = Arc::clone(&self.message);
        let stop_clone = Arc::clone(&self.stop);
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

        self.handle = Some(tokio::spawn(async move {
            let mut i = 0;
            loop {
                if *stop_clone
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                {
                    break;
                }
                let msg = msg_clone
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                print!("\r\x1B[K{} {}", frames[i].cyan(), msg);
                let _ = std::io::stdout().flush();
                i = (i + 1) % frames.len();
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
        }));
    }

    pub fn set_message(&self, message: String) {
        *self
            .message
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = message;
    }

    pub fn finish_and_clear(&self) {
        *self
            .stop
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        print!("\r\x1B[K");
        let _ = std::io::stdout().flush();
    }
}

pub fn spinner(message: &str) -> ProgressBar {
    let mut pb = ProgressBar::new_spinner();
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}
