use std::{
    cmp,
    sync::{Arc, Mutex},
};

use super::base::{Component, ComponentCreateInfo, ComponentDrawInfo};
use crate::{
    connectors::{
        base::{Connector, DatabaseData, PaginationInfo, TableData, LIMIT},
        mongodb::connector::MongodbConnectorBuilder,
    },
    managers::connection_manager::ConnectionEvent,
    systems::event_system::{Event, EventHandler, EventPool, EventValue},
    types::{HorizontalDirection, VerticalDirection},
    utils::external_editor::EXTERNAL_EDITOR,
    widgets::scrollable_table::{ScrollableTable, ScrollableTableState},
};
use anyhow::Ok;
use async_trait::async_trait;
use crossterm::event;
use ratatui::layout::Constraint;

pub struct ScrollableTableComponent {
    info: ComponentCreateInfo<TableData<'static>>,
    data: Arc<DatabaseData>,
    state: ScrollableTableState,
    query: String,
    connector: Option<Box<dyn Connector>>,
    horizontal_offset: i32,
    vertical_offset: i32,
    horizontal_offset_max: i32,
    vertical_offset_max: i32,
    pagination: PaginationInfo,
}

impl ScrollableTableComponent {
    pub fn new(
        info: ComponentCreateInfo<TableData<'static>>,
        state: ScrollableTableState,
        conn: Box<dyn Connector>,
    ) -> Self {
        Self {
            query: String::new(),
            data: Arc::new(DatabaseData(Vec::new())),
            info,
            state,
            connector: Some(conn),
            horizontal_offset: 0,
            vertical_offset: 0,
            horizontal_offset_max: 0,
            vertical_offset_max: 0,
            pagination: (0, LIMIT + 1),
        }
    }

    pub fn set_connector(&mut self, conn: Box<dyn Connector>) {
        // TODO: This is ugly, the get_table_layout fn should instead accept builder struct
        self.connector = Some(conn);
    }

    pub fn handle_next_horizontal_movement(&mut self, dir: HorizontalDirection) {
        match dir {
            HorizontalDirection::Right => {
                self.horizontal_offset =
                    cmp::min(self.horizontal_offset + 1, self.horizontal_offset_max);
            }
            HorizontalDirection::Left => {
                self.horizontal_offset = cmp::max(self.horizontal_offset - 1, 0);
            }
        }

        self.state
            .set_horizontal_offset(self.horizontal_offset as usize);
    }

    pub async fn handle_next_vertical_movement(&mut self, dir: VerticalDirection) {
        // TODO: Does not work, fix this :)
        match dir {
            VerticalDirection::Down => {
                self.vertical_offset = cmp::min(self.vertical_offset + 1, self.vertical_offset_max);
            }
            VerticalDirection::Up => {
                self.vertical_offset = cmp::max(self.vertical_offset - 1, 1);
            }
        }

        if self.vertical_offset > 10 {
            self.state
                .set_vertical_offset((self.vertical_offset - 10) as usize);
        } else {
            self.state.set_vertical_offset(0);
            self.state
                .set_vertical_select(self.vertical_offset as usize);
        }
        let offset = self.state.get_vertical_offset() + self.state.get_vertical_select();
        if offset >= 50 && matches!(dir, VerticalDirection::Down) {
            self.state.reset();
            self.vertical_offset = 1;
            self.pagination.0 += 49;
            self.refetch_data().await.unwrap();
        }
        if offset == 1
            && matches!(dir, VerticalDirection::Up)
            && self.pagination.0 > 0
            && (self.pagination.0 % 49).to_string() == "0"
        {
            self.vertical_offset = 49;
            self.state
                .set_vertical_offset((self.vertical_offset - 10) as usize);
            self.state.set_vertical_select(10);
            self.pagination.0 -= 49;
            self.refetch_data().await.unwrap();
        }
    }

    async fn refetch_data(&mut self) -> anyhow::Result<()> {
        let data = self
            .connector
            .as_ref()
            .expect("Connector to be initilized")
            .get_data(&self.query, self.pagination)
            .await?;
        self.data = Arc::new(data);
        self.info.data = TableData::from(self.data.clone());
        self.horizontal_offset_max = (self.info.data.header.cells.len() - 1) as i32;
        self.vertical_offset_max = self.info.data.rows.len() as i32;
        Ok(())
    }
}

impl Component for ScrollableTableComponent {
    fn set_visibility(&mut self, visible: bool) -> bool {
        self.info.visible = visible;
        visible
    }

    fn is_visible(&self) -> bool {
        self.info.visible
    }

    fn draw(&mut self, info: ComponentDrawInfo) {
        info.frame.render_stateful_widget(
            ScrollableTable::new(self.info.data.rows.clone(), self.info.data.header.clone()),
            info.area,
            &mut self.state,
        );
    }

    fn get_constraint(&self) -> Constraint {
        self.info.constraint
    }
}

#[async_trait]
impl EventHandler for ScrollableTableComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) {
        match &event.value {
            EventValue::OnConnection(value) => match value {
                ConnectionEvent::Connect(value) => self.set_connector(Box::new(
                    MongodbConnectorBuilder::new(&value.uri)
                        .build()
                        .await
                        .unwrap(),
                )),
                _ => {}
            },
            EventValue::OnInput(value) => match value.key.code {
                event::KeyCode::Char('i') => {
                    EXTERNAL_EDITOR.edit_value(&mut self.query).unwrap();
                    self.refetch_data().await;
                    value.terminal.lock().unwrap().clear();
                }
                event::KeyCode::Left | event::KeyCode::Char('h') => {
                    self.handle_next_horizontal_movement(HorizontalDirection::Left)
                }
                event::KeyCode::Right | event::KeyCode::Char('l') => {
                    self.handle_next_horizontal_movement(HorizontalDirection::Right)
                }
                event::KeyCode::Down | event::KeyCode::Char('j') => {
                    self.handle_next_vertical_movement(VerticalDirection::Down)
                        .await;
                }
                event::KeyCode::Up | event::KeyCode::Char('k') => {
                    self.handle_next_vertical_movement(VerticalDirection::Up)
                        .await;
                }
                event::KeyCode::Enter => {
                    EXTERNAL_EDITOR
                        .edit_value(
                            &mut self.data[self.state.get_vertical_select() - 1
                                + self.state.get_vertical_offset()]
                            .to_string(),
                        )
                        .unwrap();
                }
                _ => {}
            },
            EventValue::DatabaseData(value) => {
                self.info.data = TableData::from(value.clone());
                self.horizontal_offset_max = (self.info.data.header.cells.len() - 1) as i32;
                self.vertical_offset_max = self.info.data.rows.len() as i32;
            }
            _ => {}
        }
    }
}
