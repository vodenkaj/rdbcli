use super::base::{Component, ComponentCreateInfo, ComponentDrawInfo};
use crate::{
    application::Mode,
    managers::auth_manager::AuthCommand,
    systems::event_system::{Event, EventHandler, EventPool, EventValue},
};
use async_trait::async_trait;
use crossterm::event;
use ratatui::widgets::Paragraph;
use std::sync::{Arc, Mutex};

pub struct LoginComponent {
    info: ComponentCreateInfo<String>,
}

impl LoginComponent {
    pub fn new(info: ComponentCreateInfo<String>, hidden: bool) -> Self {
        Self { info }
    }
}

impl Component for LoginComponent {
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
        let value: String = self
            .info
            .data
            .clone()
            .into_bytes()
            .iter()
            .map(|_| "*")
            .collect();
        info.frame.render_widget(Paragraph::new(value), info.area);
    }
}

#[async_trait]
impl EventHandler for LoginComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) {
        if let EventValue::OnInput(value) = &event.value {
            if let Mode::Input = value.mode {
                match value.key.code {
                    event::KeyCode::Char(ch) => {
                        self.info.data += &ch.to_string();
                    }
                    event::KeyCode::Backspace => {
                        self.info.data.pop();
                    }
                    event::KeyCode::Enter => {
                        pool.lock().unwrap().trigger(Event {
                            component_id: event.component_id,
                            value: EventValue::OnAuthCommand(AuthCommand::Login(
                                self.info.data.clone(),
                            )),
                        });
                        self.info.data = String::new();
                    }
                    _ => {}
                }
            }
        }
    }
}
