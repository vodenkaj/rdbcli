use std::{
    cmp,
    sync::{Arc, Mutex},
};

use super::{
    base::{Component, ComponentCreateInfo, ComponentDrawInfo},
    command::{Message, Severity},
};
use crate::{
    connectors::{
        base::{Connector, DatabaseData, PaginationInfo, TableData, LIMIT},
        mongodb::connector::{
            MongodbConnectorBuilder, DATE_TO_STRING_REGEX, OBJECT_ID_TO_STRING_REGEX,
        },
    },
    managers::connection_manager::ConnectionEvent,
    systems::event_system::{Event, EventHandler, EventPool, EventValue},
    types::{HorizontalDirection, VerticalDirection},
    utils::external_editor::EXTERNAL_EDITOR,
    widgets::scrollable_table::{ScrollableTable, ScrollableTableState},
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use crossterm::event;
use mongodb::bson::Bson;
use ratatui::layout::Constraint;
use regex::Regex;

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
            pagination: PaginationInfo {
                start: 0,
                limit: LIMIT,
            },
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
        if offset == LIMIT as usize && matches!(dir, VerticalDirection::Down) {
            self.vertical_offset = 1;
            self.pagination.start += LIMIT - 1;
            self.state.reset();
            self.refetch_data().await.unwrap();
        }
        if offset == 1
            && matches!(dir, VerticalDirection::Up)
            && self.pagination.start > 0
            && (self.pagination.start % (LIMIT - 1)).to_string() == "0"
        {
            self.vertical_offset = (LIMIT - 1) as i32;
            self.state
                .set_vertical_offset((self.vertical_offset - 10) as usize);
            self.state.set_vertical_select(10);
            self.pagination.start -= LIMIT - 1;
            self.refetch_data().await.unwrap();
        }
    }

    async fn refetch_data(&mut self) -> anyhow::Result<()> {
        let data = self
            .connector
            .as_ref()
            .with_context(|| "Connector must be initilized")?
            .get_data(&self.query, &self.pagination)
            .await?;
        self.data = Arc::new(data);
        self.info.data = TableData::from(self.data.clone());
        self.horizontal_offset_max = self.info.data.header.cells.len() as i32 - 1;
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
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        match &event.value {
            EventValue::OnConnection(value) => match value {
                ConnectionEvent::Connect(value) => self.set_connector(Box::new(
                    MongodbConnectorBuilder::new(&value.uri).build().await?,
                )),
                ConnectionEvent::SwitchDatabase(value) => {
                    self.connector.as_mut().unwrap().set_database(value);
                    pool.lock().unwrap().trigger(Event {
                        component_id: 0,
                        value: EventValue::OnMessage(Message {
                            value: format!("Database switched to '{}'", value),
                            severity: Severity::Info,
                        }),
                    });
                }
                _ => {}
            },
            EventValue::OnInput(value) => {
                if matches!(value.mode, crate::application::Mode::View) {
                    match value.key.code {
                        event::KeyCode::Char('i') => {
                            EXTERNAL_EDITOR.edit_value(&mut self.query).unwrap();
                            self.refetch_data().await?;
                            value.terminal.lock().unwrap().clear()?;
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
                            if self.data.len() > 0 {
                                let mut data = self.data[self.state.get_vertical_select() - 1
                                    + self.state.get_vertical_offset()]
                                .clone();
                                resolve(&mut data);
                                EXTERNAL_EDITOR
                                    .edit_value(&mut serde_json::to_string_pretty(&data)?)?;
                            }
                        }
                        _ => {}
                    }
                }
            }
            EventValue::DatabaseData(value) => {
                self.info.data = TableData::from(value.clone());
                self.horizontal_offset_max = (self.info.data.header.cells.len() - 1) as i32;
                self.vertical_offset_max = self.info.data.rows.len() as i32;
            }
            _ => {}
        }
        Ok(())
    }
}

fn resolve(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(str) => {
            if let Some(result) = Regex::new(DATE_TO_STRING_REGEX).unwrap().captures(str) {
                let raw_date = result.get(2).unwrap().as_str().to_string();

                let date_time = match NaiveDate::parse_from_str(&raw_date, "%Y-%m-%d") {
                    Ok(parsed_date) => {
                        // Create a NaiveDateTime at midnight for the given date
                        NaiveDateTime::new(
                            parsed_date,
                            NaiveTime::from_num_seconds_from_midnight_opt(0, 0).unwrap(),
                        )
                    }
                    Err(e) => {
                        panic!("Failed to parse date: {}", e);
                    }
                };

                let date = DateTime::from_timestamp(date_time.timestamp(), 0).unwrap();
                *value = serde_json::from_str(&date.to_rfc3339()).unwrap();
            } else if let Some(result) =
                Regex::new(OBJECT_ID_TO_STRING_REGEX).unwrap().captures(str)
            {
                let raw_object_id = result.get(2).unwrap().as_str().to_string();
                *value = serde_json::from_str(&raw_object_id).unwrap();
            }
        }
        serde_json::Value::Array(array) => array.iter_mut().for_each(resolve),
        serde_json::Value::Object(obj) => obj.values_mut().for_each(|v| {
            if v.is_object() {
                let bson = Bson::try_from(v.clone()).unwrap();
                if let Some(date) = bson.as_datetime() {
                    *v = serde_json::Value::String(date.try_to_rfc3339_string().unwrap());
                } else {
                    resolve(v);
                }
            }
        }),
        _ => {}
    }
}
