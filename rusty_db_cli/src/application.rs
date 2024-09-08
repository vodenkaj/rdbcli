use std::{
    fmt::Display,
    io::Stdout,
    sync::{Arc, Mutex},
    time::Duration,
};

use crossterm::event;
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::{task::JoinHandle, time::sleep};

use crate::{
    managers::{event_manager::Event, window_manager::WindowManager},
    ui::window::{OnInputInfo, WindowRenderInfo},
    widgets::throbber::{get_throbber_data, Throbber},
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

type TerminalTyped = Terminal<CrosstermBackend<Stdout>>;

impl App {
    pub fn new(terminal: TerminalTyped, window_manager: WindowManager) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            should_exit: false,
            mode: Mode::View,
            logs: Vec::new(),
            window_manager,
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
                .send(Event::OnMessage($crate::ui::components::command::Message {
                    value: err.to_string(),
                    severity: $crate::ui::components::command::Severity::Error,
                }))
                .unwrap();
        }
    };
}

type ArcApp = Arc<Mutex<App>>;

pub async fn wait_for_app_initialization(
    mut future: JoinHandle<WindowManager>,
    mut terminal: TerminalTyped,
) -> ArcApp {
    let (steps, mut state) = get_throbber_data();
    loop {
        tokio::select! {
            res  = &mut future => {
                let window_manager = res.unwrap();

                return App::new(terminal, window_manager)
            }
            _ = sleep(Duration::from_millis(10)) => {

        terminal
            .draw(|f| {
                f.render_stateful_widget(Throbber::new(steps.clone(), Some("Establishing connection with the database...".to_string())), f.size(), &mut state);
            })
            .unwrap();
                }
        }
    }
}
