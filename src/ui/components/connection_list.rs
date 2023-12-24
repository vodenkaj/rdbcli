use super::{
    base::{Component, ComponentCreateInfo, ComponentDrawInfo},
    connection::ConnectionInfo,
};
use crate::{
    managers::connection_manager::ConnectionEvent,
    systems::event_system::{Event, EventHandler, EventPool, EventValue},
};
use async_trait::async_trait;
use ratatui::widgets::{List, ListItem};
use std::{
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};

pub struct ConnectionListComponent {
    info: ComponentCreateInfo<Vec<ConnectionInfo>>,
}

impl ConnectionListComponent {
    pub fn new(info: ComponentCreateInfo<Vec<ConnectionInfo>>) -> Self {
        Self { info }
    }
}

impl Component for ConnectionListComponent {
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
        let items: Vec<ListItem> = self
            .info
            .data
            .clone()
            .iter()
            .map(|i| ListItem::new(i.uri.clone()))
            .collect();
        info.frame.render_widget(List::new(items), info.area);
    }
}

#[async_trait]
impl EventHandler for ConnectionListComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) {
        if let EventValue::OnConnection(value) = &event.value {
            match value {
                ConnectionEvent::Add(value) => {
                    self.info.data.push(value.clone());

                    let home_dir = home::home_dir().unwrap();
                    let file_dir = format!("{}/.rustydbcli", home_dir.to_str().unwrap());
                    let path = Path::new(&file_dir);
                    let mut file = File::open(path).unwrap();
                    //file.write_all(value.clone());
                }
            }
        }
    }
}
