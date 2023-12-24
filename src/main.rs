use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, Terminal};
use rusty_db_cli::application::App;
use std::{
    io::{self},
    sync::{Arc, Mutex},
    time::Duration,
};

#[tokio::main]
async fn main() {
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let term = Terminal::new(backend).unwrap();
    let app = App::new(term).await;

    loop {
        let mut handle = app.lock().unwrap();
        handle.render().await;

        if event::poll(Duration::from_secs(0)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                handle.on_key(key).await;
            }
        }

        if handle.should_exit {
            break;
        }
    }

    disable_raw_mode().unwrap();
    let app_guard = app.lock().unwrap();
    let mut term_guard = app_guard.terminal.lock().unwrap();
    execute!(
        term_guard.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();
}
