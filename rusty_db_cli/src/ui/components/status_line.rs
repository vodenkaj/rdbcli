use ratatui::widgets::Paragraph;

use super::base::{Component, ComponentCreateInfo};
use crate::{
    connectors::base::DatabaseKind,
    managers::event_manager::{ConnectionEvent, Event, EventHandler},
};

pub struct StatusLineComponent {
    info: ComponentCreateInfo<StatusLineData>,
}

pub struct StatusLineData {
    pub host: String,
    pub database_name: String,
    pub database_kind: DatabaseKind,
}

impl Default for StatusLineData {
    fn default() -> Self {
        Self {
            host: "unknown".to_string(),
            database_name: "unknown".to_string(),
            database_kind: DatabaseKind::Unknown,
        }
    }
}

impl Component for StatusLineComponent {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

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
    fn as_mut_event_handler(&mut self) -> &mut dyn EventHandler {
        self
    }
    fn on_event(&mut self, event: &Event) -> anyhow::Result<()> {
        if let Event::OnConnection(ConnectionEvent::SwitchDatabase(value)) = event {
            self.info.data.database_name = value.clone();
        } else if let Event::OnConnection(ConnectionEvent::SwitchConnection(info)) = event {
            self.info.data.host = info.host.clone();
            self.info.data.database_name = info.database.clone();
            self.info.data.database_kind = info.kind.clone();
        }
        Ok(())
    }
}

impl StatusLineComponent {
    pub fn new(info: ComponentCreateInfo<StatusLineData>) -> Self {
        Self { info }
    }

    fn get_status_string(&self) -> String {
        let database_name = format!("{} {}", self.get_database_icon(), self.info.data.host);

        [database_name, self.info.data.database_name.clone()].join(" | ")
    }

    fn get_database_icon(&self) -> String {
        match self.info.data.database_kind {
            DatabaseKind::MongoDB => "".to_string(),
            DatabaseKind::PostgresSQL => "🐘".to_string(),
            DatabaseKind::Unknown => "❓".to_string(),
        }
    }
}
