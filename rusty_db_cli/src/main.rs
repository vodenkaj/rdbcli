use core::time;
use std::{
    io::{self},
    time::Duration,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, Terminal};
use rusty_db_cli::{
    application::App,
    managers::{event_manager::EventManager, window_manager::WindowManagerBuilder},
    ui::layouts::{get_table_layout, CLI_ARGS},
};

#[tokio::main]
async fn main() {
    CLI_ARGS.debug;

    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend).unwrap();
    term.clear().unwrap();

    let event_manager = EventManager::new();

    let app = App::new(
        term,
        WindowManagerBuilder::new()
            .with_window(get_table_layout(event_manager.sender.clone()))
            .build(),
        event_manager,
    );

    loop {
        {
            let mut handle = app.lock().await;
            handle.render();

            if event::poll(Duration::from_secs(0)).unwrap() {
                if let Event::Key(key) = event::read().unwrap() {
                    handle.on_key(key);
                }
            }

            handle.on_update();

            if handle.should_exit {
                break;
            }
        }

        tokio::time::sleep(time::Duration::from_millis(10)).await;
    }

    disable_raw_mode().unwrap();
    let app_guard = app.lock().await;
    let mut term_guard = app_guard.terminal.lock().unwrap();
    execute!(
        term_guard.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();
}
