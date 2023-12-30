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

const SWITCH_DATABASE_REGEX: &str = r#"c ([A-z0-9\-]+)"#;

#[async_trait]
impl EventHandler for CommandComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        match &event.value {
            EventValue::OnMessage(value) => self.info.data = value.clone(),
            EventValue::OnInput(value) => {
                if matches!(value.mode, crate::application::Mode::Input) {
                    if !matches!(self.info.data.severity, Severity::Normal) {
                        self.info.data = Message::default();
                    }

                    match value.key.code {
                        event::KeyCode::Char(value) => {
                            self.info.data.value += &value.to_string();
                        }
                        event::KeyCode::Backspace => {
                            self.info.data.value.pop();
                        }
                        event::KeyCode::Enter => {
                            let database = Regex::new(SWITCH_DATABASE_REGEX)?
                                .captures(&self.info.data.value)
                                .map(|m| anyhow::Ok(m.get(1).unwrap().as_str()))
                                .with_context(|| {
                                    format!("'{}' is not valid database name", self.info.data.value)
                                })?
                                .unwrap();
                            pool.lock().unwrap().trigger(Event {
                                component_id: 0,
                                value: EventValue::OnConnection(ConnectionEvent::SwitchDatabase(
                                    database.to_string(),
                                )),
                            });
                            self.info.data.value = String::new();
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
