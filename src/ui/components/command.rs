use std::sync::{Arc, Mutex};
use anyhow::Result;
use async_trait::async_trait;
use ratatui::{style::Style, widgets::Paragraph};
use crate::systems::event_system::{Event, EventHandler, EventPool, EventValue};
use super::base::{Component, ComponentCreateInfo};

#[derive(Default)]
pub enum Severity {
    #[default]
    Normal,
    Error,
}

#[derive(Default)]
pub struct Message {
    severity: Severity,
    value: String,
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

#[async_trait]
impl EventHandler for CommandComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        if let EventValue::OnError(value) = &event.value {
            self.info.data = Message {
                value: value.clone(),
                severity: Severity::Error,
            }
        }
        Ok(())
    }
}
