use std::{fmt::Display, io::Stdout, sync::Arc};

use crossterm::event::{self};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::Mutex;

use crate::{
    connectors::{
        base::{get_connector, Connector},
        mongodb::connector::ConnectorResource,
    },
    log_error,
    managers::{
        event_manager::{ConnectionEvent, Event, EventHandler, EventManager},
        resource_manager::ResourceManager,
        window_manager::WindowManager,
    },
    ui::{
        components::command::{Message, Severity},
        window::{OnInputInfo, WindowRenderInfo},
    },
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
    pub terminal: Arc<std::sync::Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    window_manager: WindowManager,
    resource_manager: ResourceManager,
    event_manager: EventManager,
}

type TerminalTyped = Terminal<CrosstermBackend<Stdout>>;

impl App {
    pub fn new(
        terminal: TerminalTyped,
        window_manager: WindowManager,
        event_manager: EventManager,
    ) -> Arc<Mutex<Self>> {
        let cloned_sender = event_manager.sender.clone();

        let resource_manager = ResourceManager::new();

        event_manager
            .sender
            .send(Event::OnAsyncEvent(tokio::spawn(async move {
                let original_connector = get_connector().await;
                let info = original_connector.get_info().clone();
                let connector = Some(Arc::new(Mutex::new(original_connector)));

                cloned_sender.send(Event::OnResourceEvent(
                    crate::managers::event_manager::ResourceEvent::Add(Box::new(
                        ConnectorResource {
                            connector,
                            event_sender: cloned_sender.clone(),
                        },
                    )),
                ));

                cloned_sender
                    .send(Event::OnMessage(Message {
                        value: format!("Connection switched to '{}'", &info.host),
                        severity: Severity::Info,
                    }))
                    .unwrap();

                cloned_sender
                    .send(Event::OnConnection(ConnectionEvent::SwitchConnection(
                        info.clone(),
                    )))
                    .unwrap();
            })));

        Arc::new(Mutex::new(Self {
            should_exit: false,
            mode: Mode::View,
            logs: Vec::new(),
            window_manager,
            terminal: Arc::new(std::sync::Mutex::new(terminal)),
            resource_manager,
            event_manager,
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
        let focused_window = self.window_manager.get_focused_window();

        focused_window.render(WindowRenderInfo {
            terminal: self.terminal.clone(),
            mode: self.mode,
            event_manager: &mut self.event_manager,
        });
    }

    pub fn on_update(&mut self) {
        let focused_window = self.window_manager.get_focused_window();

        let mut event_handlers: Vec<_> = focused_window
            .components
            .iter_mut()
            .map(|c| Box::new(c.as_mut_event_handler()))
            .collect();

        match self
            .event_manager
            .pool(&mut event_handlers, &mut self.resource_manager)
        {
            Ok(should_quit) => {
                self.should_exit = should_quit;
            }
            Err(err) => {
                log_error!(self.event_manager.sender, Some(err))
            }
        }
    }

    pub fn on_key(&mut self, key: event::KeyEvent) {
        self.event_manager.sender.send(Event::OnInput(OnInputInfo {
            terminal: self.terminal.clone(),
            mode: self.mode,
            key,
        }));

        match self.mode {
            Mode::View => {
                if let event::KeyCode::Char(':') = key.code {
                    self.set_mode(Mode::Input);
                }
            }
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
                .send(Event::OnMessage($crate::ui::components::command::Message {
                    value: err.to_string(),
                    severity: $crate::ui::components::command::Severity::Error,
                }))
                .unwrap();
        }
    };
}
