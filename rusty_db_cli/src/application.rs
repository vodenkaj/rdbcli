use crate::{
    managers::window_manager::{WindowManager, WindowManagerBuilder},
    systems::event_system::Event,
    ui::{
        layouts::get_table_layout,
        window::{OnInputInfo, WindowRenderInfo},
    },
};
use crossterm::event;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    fmt::Display,
    io::Stdout,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy)]
pub enum Mode {
    View,
    Input,
}

pub struct App {
    pub should_exit: bool,
    pub mode: Mode,
    logs: Vec<String>,
    pub terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    window_manager: WindowManager,
}

impl App {
    pub async fn new(terminal: Terminal<CrosstermBackend<Stdout>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            should_exit: false,
            mode: Mode::View,
            logs: Vec::new(),
            window_manager: WindowManagerBuilder::new()
                .with_window(get_table_layout().await)
                .build(),
            terminal: Arc::new(Mutex::new(terminal)),
        }))
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        match self.mode {
            Mode::View => {}
            Mode::Input => {}
        }
    }

    fn log(&mut self, value: &str) {
        self.logs.push(value.to_string())
    }

    fn callback_or_log<T, E>(&mut self, res: Result<T, E>, callback: impl FnOnce(T))
    where
        E: Display,
    {
        match res {
            Ok(value) => {
                callback(value);
            }
            Err(value) => self.log(&value.to_string()),
        }
    }

    pub fn render(&mut self) {
        self.window_manager
            .get_focused_window()
            .render(WindowRenderInfo {
                terminal: self.terminal.clone(),
                mode: self.mode,
            })
    }

    pub fn on_key(&mut self, key: event::KeyEvent) {
        self.window_manager
            .get_focused_window()
            .on_key(Event::OnInput(OnInputInfo {
                terminal: self.terminal.clone(),
                mode: self.mode,
                key,
            }));

        match self.mode {
            Mode::View => match key.code {
                event::KeyCode::Char('q') => {
                    self.should_exit = true;
                }
                event::KeyCode::Char(':') => {
                    self.set_mode(Mode::Input);
                }
                _ => {}
            },
            Mode::Input => match key.code {
                event::KeyCode::Enter => {
                    self.set_mode(Mode::View);
                }
                event::KeyCode::Esc => self.set_mode(Mode::View),
                _ => {}
            },
        }
    }
}

#[macro_export]
macro_rules! log_error {
    ($event_sender:expr, $err:expr) => {
        if let Some(err) = $err {
            $event_sender
                .send(Event::OnMessage(Message {
                    value: err.to_string(),
                    severity: Severity::Error,
                }))
                .unwrap();
        }
    };
}
