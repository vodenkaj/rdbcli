use core::time;
use std::{
    io::{self},
    thread,
    time::Duration,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, Terminal};
use rusty_db_cli::{
    application::wait_for_app_initialization, managers::window_manager::WindowManagerBuilder,
    ui::layouts::get_table_layout,
};
use tokio::task;

#[tokio::main]
async fn main() {
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend).unwrap();
    term.clear();

    let app = wait_for_app_initialization(
        task::spawn(async {
            WindowManagerBuilder::new()
                .with_window(get_table_layout().await)
                .build()
        }),
        term,
    )
    .await;

    loop {
        let mut handle = app.lock().unwrap();
        handle.render();

        if event::poll(Duration::from_secs(0)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                handle.on_key(key);
            }
        }

        if handle.should_exit {
            break;
        }
        thread::sleep(time::Duration::from_millis(10));
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
