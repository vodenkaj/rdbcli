use super::{
    base::{Component, ComponentCreateInfo, ComponentDrawInfo},
    connection::ConnectionInfo,
};
use crate::{
    managers::{connection_manager::ConnectionEvent, window_manager::WindowCommand},
    systems::event_system::{Event, EventHandler, EventPool, EventValue},
};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event;
use ratatui::widgets::{List, ListItem};
use std::{
    fs::File,
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
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        match &event.value {
            EventValue::OnConnection(value) => match value {
                ConnectionEvent::Add(value) => {
                    self.info.data.push(value.clone());

                    let home_dir = home::home_dir().unwrap();
                    let file_dir = format!("{}/.rustydbcli", home_dir.to_str().unwrap());
                    let path = Path::new(&file_dir);
                    let mut file = File::open(path).unwrap();
                    //file.write_all(value.clone());
                }
                _ => {}
            },
            EventValue::OnInput(value) => {
                if self.info.id == event.component_id {
                    match value.key.code {
                        event::KeyCode::Enter => {
                            pool.lock().unwrap().trigger(Event {
                                component_id: event.component_id,
                                // TODO: Remove this hardcoded indexing
                                value: EventValue::OnConnection(ConnectionEvent::Connect(
                                    self.info.data[0].clone(),
                                )),
                            });
                            pool.lock().unwrap().trigger(Event {
                                component_id: event.component_id,
                                // TODO: Remove this hardcoded indexing
                                value: EventValue::OnWindowCommand(
                                    WindowCommand::SetFocusedWindow(2),
                                ),
                            });
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
