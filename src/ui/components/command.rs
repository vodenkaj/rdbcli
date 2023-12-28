use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use ratatui::widgets::Paragraph;

use crate::systems::event_system::{Event, EventHandler, EventPool, EventValue};

use super::base::{Component, ComponentCreateInfo};

pub struct CommandComponent {
    info: ComponentCreateInfo<String>,
}

impl CommandComponent {
    pub fn new(info: ComponentCreateInfo<String>) -> Self {
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
        info.frame
            .render_widget(Paragraph::new(self.info.data.clone()), info.area);
    }
}

#[async_trait]
impl EventHandler for CommandComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        if let EventValue::OnError(value) = &event.value {
            self.info.data = value.clone();
        }
        Ok(())
    }
}
