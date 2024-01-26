use super::base::{Component, ComponentCreateInfo, ComponentDrawInfo};
use crate::managers::event_manager::{Event, EventHandler};
use anyhow::Result;
use ratatui::widgets::Paragraph;

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

impl EventHandler for ParagraphComponent {
    fn on_event(&mut self, _event: &Event) -> Result<()> {
        Ok(())
    }
}
