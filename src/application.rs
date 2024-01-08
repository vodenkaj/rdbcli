use crate::{
    managers::{
        auth_manager::AuthManager,
        connection_manager::ConnectionManager,
        window_manager::{WindowManager, WindowManagerBuilder},
    },
    systems::event_system::{Event, EventValue},
    ui::{
        components::command::{Message, Severity},
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
    window_manager: Arc<Mutex<WindowManager>>,
    auth_manager: Arc<Mutex<AuthManager>>,
    connection_manager: Arc<Mutex<ConnectionManager>>,
}

impl App {
    pub fn new(terminal: Terminal<CrosstermBackend<Stdout>>) -> Arc<Mutex<Self>> {
        let auth_manager = Arc::new(Mutex::new(AuthManager::new()));
        let connection_manager = Arc::new(Mutex::new(ConnectionManager::new()));
        Arc::new(Mutex::new(Self {
            should_exit: false,
            mode: Mode::View,
            logs: Vec::new(),
            window_manager: WindowManagerBuilder::new()
                //.with_window(get_login_layout())
                //.with_window(get_connections_layout(auth_manager.clone()))
                .with_window(get_table_layout())
                .build(auth_manager.clone()),
            terminal: Arc::new(Mutex::new(terminal)),
            auth_manager,
            connection_manager,
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
            .lock()
            .unwrap()
            .get_focused_window()
            .lock()
            .unwrap()
            .render(WindowRenderInfo {
                terminal: self.terminal.clone(),
                mode: self.mode,
            })
    }

    pub fn on_key(&mut self, key: event::KeyEvent) {
        let event_manager;
        let focused;
        {
            let mut manager = self.window_manager.lock().unwrap();
            let window = manager.get_focused_window().lock().unwrap();
            event_manager = window.event_manager.clone();
            focused = window.focused_component_idx;
        }
        let mut events = event_manager.lock().unwrap();
        let result = events.trigger_event_sync(Event {
            component_id: focused,
            value: EventValue::OnInput(OnInputInfo {
                connection_manager: self.connection_manager.clone(),
                window_manager: self.window_manager.clone(),
                terminal: self.terminal.clone(),
                mode: self.mode,
                key,
            }),
        });

        if let Err(err) = result {
            self.log(&err.to_string());
            events
                .trigger_event_sync(Event {
                    component_id: focused,
                    value: EventValue::OnMessage(Message {
                        value: err.to_string(),
                        severity: Severity::Error,
                    }),
                })
                .unwrap();
        }
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
