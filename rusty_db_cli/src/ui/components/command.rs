use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    process::Command,
    thread,
};

use anyhow::{Context, Result};
use crossterm::event;
use ratatui::{style::Style, widgets::Paragraph};
use regex::Regex;

use super::base::{Component, ComponentCreateInfo};
use crate::{
    managers::event_manager::{ConnectionEvent, Event, EventHandler},
    utils::external_editor::HISTORY_FILE,
};

#[derive(Default, Clone)]
pub enum Severity {
    #[default]
    Normal,
    Info,
    Error,
}

#[derive(Default, Clone)]
pub struct Message {
    pub severity: Severity,
    pub value: String,
}

pub struct CommandComponent {
    info: ComponentCreateInfo<Message>,
    history: Vec<String>,
    history_index: usize,
}

impl CommandComponent {
    pub fn new(info: ComponentCreateInfo<Message>) -> Self {
        let mut handle = File::open(HISTORY_FILE.to_string()).unwrap();
        let mut buffer = String::new();

        handle.read_to_string(&mut buffer).unwrap();

        Self {
            info,
            history: buffer
                .split('\n')
                .collect::<HashSet<&str>>()
                .into_iter()
                .filter_map(|s| {
                    if s.is_empty() {
                        return None;
                    }
                    return Some(s.to_string());
                })
                .collect(),
            history_index: 0,
        }
    }
}

impl Component for CommandComponent {
    fn get_constraint(&self) -> ratatui::prelude::Constraint {
        self.info.constraint
    }

    fn is_visible(&self) -> bool {
        self.info.visible
    }

    fn set_visibility(&mut self, visible: bool) -> bool {
        self.info.visible = visible;
        visible
    }

    fn draw(&mut self, info: super::base::ComponentDrawInfo) {
        let mut style = Style::default();
        if matches!(self.info.data.severity, Severity::Error) {
            style = style.fg(ratatui::style::Color::Red);
        }

        info.frame.render_widget(
            Paragraph::new(self.info.data.value.clone()).style(style),
            info.area,
        );
    }
}

// Not bug proof
const COMMAND_REGEX: &str = r#"^([^ ]*) ((!\((.*)\))|(.*))"#;

impl EventHandler for CommandComponent {
    fn on_event(&mut self, event: &Event) -> Result<()> {
        match event {
            Event::OnMessage(value) => self.info.data = value.clone(),
            Event::OnInput(value) => {
                if matches!(value.mode, crate::application::Mode::Input) {
                    if !matches!(self.info.data.severity, Severity::Normal) {
                        self.info.data = Message::default()
                    }

                    match value.key.code {
                        event::KeyCode::Char(value) => {
                            self.info.data.value += &value.to_string();
                        }
                        event::KeyCode::Backspace => {
                            self.info.data.value.pop();
                        }
                        event::KeyCode::Up => {
                            if let Some(history) = self.history.get(self.history_index) {
                                self.info.data.value.clone_from(history);
                                self.history_index += 1;
                            }
                        }
                        event::KeyCode::Down => {
                            if self.history_index != 0 {
                                self.history_index -= 1;

                                if let Some(history) = self.history.get(self.history_index) {
                                    self.info.data.value.clone_from(history);
                                }
                            }
                        }
                        event::KeyCode::Enter => {
                            let (command, arg0) = Regex::new(COMMAND_REGEX)?
                                .captures(&self.info.data.value)
                                .map(|m| {
                                    let command = m
                                        .get(1)
                                        .with_context(|| "First argument of command is missing")?
                                        .as_str();

                                    let arg0 = m
                                        .get(5)
                                        .map(|r| r.as_str().to_string())
                                        .or_else(|| {
                                            let command = m.get(4)?;
                                            let arg = Command::new("zsh")
                                                .arg("-ci")
                                                .arg(command.as_str())
                                                .output()
                                                .ok()?;

                                            Some(
                                                std::str::from_utf8(&arg.stdout)
                                                    .ok()?
                                                    .trim()
                                                    .to_string(),
                                            )
                                        })
                                        .with_context(|| "Argument of command is missing")?;

                                    anyhow::Ok((command, arg0))
                                })
                                .with_context(|| "Invalid command")??;

                            let issued_command = self.info.data.value.clone();
                            thread::spawn(move || {
                                let mut handle = OpenOptions::new()
                                    .append(true)
                                    .open(HISTORY_FILE.to_string())
                                    .unwrap();
                                handle
                                    .write_all(format!("{}\n", issued_command).as_bytes())
                                    .unwrap();
                            });

                            match command {
                                "use" => {
                                    self.info.event_sender.send(Event::OnConnection(
                                        ConnectionEvent::SwitchDatabase(arg0.to_string()),
                                    ))?;
                                    self.info.data.value = String::new();
                                }
                                "connect" => {
                                    self.info.event_sender.send(Event::OnConnection(
                                        ConnectionEvent::Connect(arg0.to_string()),
                                    ))?;
                                    self.info.data.value = String::new();
                                }
                                _ => {
                                    self.info.data = Message {
                                        value: String::from("Command not found"),
                                        severity: Severity::Error,
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
