use ratatui::widgets::Paragraph;

use super::base::{Component, ComponentCreateInfo};
use crate::managers::event_manager::{ConnectionEvent, Event, EventHandler};

pub struct StatusLineComponent {
    info: ComponentCreateInfo<StatusLineData>,
}

pub struct StatusLineData {
    pub host: String,
    pub database_name: String,
}

impl Component for StatusLineComponent {
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
            .render_widget(Paragraph::new(self.get_status_string()), info.area);
    }
}

impl EventHandler for StatusLineComponent {
    fn on_event(&mut self, event: &Event) -> anyhow::Result<()> {
        if let Event::OnConnection(ConnectionEvent::SwitchDatabase(value)) = event {
            self.info.data.database_name = value.clone();
        } else if let Event::OnConnection(ConnectionEvent::SwitchConnection(host, db)) = event {
            self.info.data.host = host.clone();
            self.info.data.database_name = db.clone();
        }
        Ok(())
    }
}

impl StatusLineComponent {
    pub fn new(info: ComponentCreateInfo<StatusLineData>) -> Self {
        Self { info }
    }

    fn get_status_string(&self) -> String {
        let database_name = format!(" {}", self.info.data.host);

        [database_name, self.info.data.database_name.clone()].join(" | ")
    }
}
