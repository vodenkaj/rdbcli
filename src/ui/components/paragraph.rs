use std::sync::{Arc, Mutex};
use crate::systems::event_system::{Event, EventHandler, EventPool};
use anyhow::Result;
use async_trait::async_trait;
use ratatui::widgets::Paragraph;

use super::base::{Component, ComponentCreateInfo, ComponentDrawInfo};

pub struct ParagraphComponent {
    info: ComponentCreateInfo<String>,
}

impl ParagraphComponent {
    pub fn new(info: ComponentCreateInfo<String>) -> Self {
        Self { info }
    }
}

impl Component for ParagraphComponent {
    fn set_visibility(&mut self, visible: bool) -> bool {
        self.info.visible = visible;
        visible
    }

    fn is_visible(&self) -> bool {
        self.info.visible
    }
    fn get_constraint(&self) -> ratatui::prelude::Constraint {
        self.info.constraint
    }

    fn draw(&mut self, info: ComponentDrawInfo) {
        info.frame
            .render_widget(Paragraph::new(self.info.data.clone()), info.area);
    }
}

#[async_trait]
impl EventHandler for ParagraphComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        Ok(())
    }
}
