use super::base::{Component, ComponentCreateInfo};
use crate::{
    managers::connection_manager::ConnectionEvent,
    systems::event_system::{Event, EventHandler, EventPool, EventValue},
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event;
use ratatui::{style::Style, widgets::Paragraph};
use regex::Regex;
use std::sync::{Arc, Mutex};

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
}

impl CommandComponent {
    pub fn new(info: ComponentCreateInfo<Message>) -> Self {
        Self { info }
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

const COMMAND_REGEX: &str = r#"(.*) (.*)"#;

#[async_trait]
impl EventHandler for CommandComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        match &event.value {
            EventValue::OnMessage(value) => self.info.data = value.clone(),
            EventValue::OnInput(value) => {
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
                        event::KeyCode::Enter => {
                            let (command, arg0) = Regex::new(COMMAND_REGEX)?
                                .captures(&self.info.data.value)
                                .map(|m| {
                                    let command = m
                                        .get(1)
                                        .with_context(|| "First argument of command is missing")?
                                        .as_str();
                                    let arg0 = m
                                        .get(2)
                                        .with_context(|| "Second argument of command is missing")?
                                        .as_str();
                                    anyhow::Ok((command, arg0))
                                })
                                .with_context(|| "Invalid command")??;
                            match command {
                                "c" | "connect" | "use" => {
                                    pool.lock().unwrap().trigger(Event {
                                        component_id: 0,
                                        value: EventValue::OnConnection(
                                            ConnectionEvent::SwitchDatabase(arg0.to_string()),
                                        ),
                                    });
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
