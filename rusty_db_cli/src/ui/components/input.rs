use anyhow::Result;
use crossterm::event;
use ratatui::widgets::Paragraph;

use super::base::{Component, ComponentCreateInfo, ComponentDrawInfo};
use crate::{
    application::Mode,
    managers::event_manager::{Event, EventHandler},
};

pub struct InputComponent {
    info: ComponentCreateInfo<String>,
    hidden: bool,
}

impl InputComponent {
    pub fn new(info: ComponentCreateInfo<String>, hidden: bool) -> Self {
        Self { info, hidden }
    }
}

impl Component for InputComponent {
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
        let value = if self.hidden {
            self.info
                .data
                .clone()
                .into_bytes()
                .iter()
                .map(|_| "*")
                .collect()
        } else {
            self.info.data.clone()
        };
        info.frame.render_widget(Paragraph::new(value), info.area);
    }
}

impl EventHandler for InputComponent {
    fn on_event(&mut self, event: &Event) -> Result<()> {
        if let Event::OnInput(value) = event {
            if let Mode::Input = value.mode {
                match value.key.code {
                    event::KeyCode::Char(ch) => {
                        self.info.data += &ch.to_string();
                    }
                    event::KeyCode::Backspace => {
                        self.info.data.pop();
                    }
                    event::KeyCode::Enter => {
                        //pool.lock()
                        //    .unwrap()
                        //    .trigger(Event::OnCommand(self.info.data.clone()));
                        self.info.data = String::new();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
