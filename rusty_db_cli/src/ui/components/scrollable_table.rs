use std::{cmp, collections::HashSet, fs::File, io::Read, sync::Arc};

use anyhow::Result;
use crossterm::event;
use rand::Rng;
use ratatui::{layout::Constraint, widgets::Paragraph};
use rusty_db_cli_mongo::interpreter::InterpreterError;
use tokio::sync::Mutex;

use super::{
    base::{Component, ComponentCreateInfo, ComponentDrawInfo},
    command::{Message, Severity},
};
use crate::{
    connectors::base::{Connector, DatabaseData, Object, PaginationInfo, TableData, LIMIT},
    log_error,
    managers::event_manager::{ConnectionEvent, Event, EventHandler},
    try_from,
    types::{HorizontalDirection, VerticalDirection},
    utils::external_editor::{FileType, EXTERNAL_EDITOR, MONGO_QUERY_FILE},
    widgets::scrollable_table::{Row, ScrollableTable, ScrollableTableState},
};

pub struct ScrollableTableComponent {
    info: ComponentCreateInfo<TableData<'static>>,
    data: DatabaseData,
    is_fetching: bool,
    state: ScrollableTableState,
    query: String,
    connector: Arc<Mutex<dyn Connector>>,
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
        conn: Arc<Mutex<dyn Connector>>,
    ) -> Self {
        let mut handle =
            File::open(MONGO_QUERY_FILE.to_string()).expect("Failed to read query file");
        let mut query = String::new();
        handle
            .read_to_string(&mut query)
            .expect("Failed to read query file");
        Self {
            is_fetching: false,
            query,
            data: DatabaseData(Vec::new()),
            info,
            state,
            connector: conn,
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

    pub fn reset_state(&mut self) {
        self.state.reset();
        self.horizontal_offset = 0;
        self.vertical_offset = 0;
    }

    pub fn set_connector(&mut self, conn: Arc<Mutex<dyn Connector>>) {
        // TODO: This is ugly, the get_table_layout fn should instead accept builder struct
        self.connector = conn;
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

    pub fn spawn_next_data(&mut self) {
        let (cloned_conn, cloned_query, cloned_pagination, event_sender) = (
            self.connector.clone(),
            self.query.clone(),
            self.pagination,
            self.info.event_sender.clone(),
        );
        self.is_fetching = true;
        tokio::spawn(async move {
            let result = cloned_conn
                .lock()
                .await
                .get_data(cloned_query, cloned_pagination)
                .await;
            match result {
                Ok(data) => {
                    event_sender.send(Event::DatabaseData(data)).unwrap();
                }
                Err(err) => {
                    log_error!(event_sender, Some(err));
                }
            };
        });
    }

    pub fn handle_next_vertical_movement(&mut self, dir: VerticalDirection) {
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
            self.pagination.start += (LIMIT - 1) as u64;
            self.state.reset();
            self.state
                .set_horizontal_offset(self.horizontal_offset as usize);
            self.spawn_next_data();
        }
        if offset == 1
            && matches!(dir, VerticalDirection::Up)
            && self.pagination.start > 0
            && (self.pagination.start % (LIMIT - 1) as u64).to_string() == "0"
        {
            self.vertical_offset = (LIMIT - 1) as i32;
            self.state
                .set_vertical_offset((self.vertical_offset - 10) as usize);
            self.state.set_vertical_select(10);
            self.pagination.start -= (LIMIT - 1) as u64;
            self.spawn_next_data();
        }
    }

    fn set_data(&mut self, data: DatabaseData) -> anyhow::Result<()> {
        self.data = data;
        self.info.data = TableData::from(self.data.clone());
        self.horizontal_offset_max = self.info.data.header.cells.len() as i32 - 1;
        self.vertical_offset_max = self.info.data.rows.len() as i32;
        // TODO: We should keep order of the fields between refteches
        self.calculate_cell_widths();
        Ok(())
    }

    fn calculate_cell_widths(&mut self) {
        self.state.cell_widths = self
            .info
            .data
            .header
            .cells
            .iter()
            .enumerate()
            .map(|(idx, cell)| {
                if self.info.data.header.cells.len() - 1 == idx {
                    // Last cell should take rest of the remaining space
                    return u16::MAX;
                }
                let value_widths = self
                    .info
                    .data
                    .rows
                    .iter()
                    .map(|r| r.cells[idx].content.width() as u16)
                    .collect::<Vec<_>>();

                let mut cell_width: u16 = 0;
                let mut size = 0;
                for width in value_widths.iter() {
                    if width >= &100 {
                        continue;
                    }
                    if let Some(value) = cell_width.checked_add(*width) {
                        cell_width = value;
                        size += 1;
                    }
                }
                let cell_avg_width = cell_width.checked_div(size).unwrap_or(0);
                let header_cell_width = cmp::min(cell.content.width(), 30) as u16;

                cmp::max(header_cell_width, cell_avg_width)
            })
            .collect::<Vec<_>>();
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
        match self.is_fetching {
            true => {
                let random_state: String = (0..rand::thread_rng().gen_range(0..3))
                    .map(|_| ".")
                    .collect();
                info.frame.render_widget(
                    Paragraph::new(format!("Quering{}", random_state)),
                    info.area,
                )
            }
            false => {
                info.frame.render_stateful_widget(
                    ScrollableTable::new(
                        self.info.data.rows.clone(),
                        self.info.data.header.clone(),
                    ),
                    info.area,
                    &mut self.state,
                );
            }
        }
    }

    fn get_constraint(&self) -> Constraint {
        self.info.constraint
    }
}

impl EventHandler for ScrollableTableComponent {
    fn on_event(&mut self, event: &Event) -> Result<()> {
        match event {
            Event::OnConnection(value) => match value {
                ConnectionEvent::SwitchDatabase(value) => {
                    let connector = self.connector.clone();
                    let cloned_value = value.clone();
                    let cloned_sender = self.info.event_sender.clone();
                    let result = self
                        .info
                        .event_sender
                        .send(Event::OnAsyncEvent(tokio::spawn(async move {
                            match connector.lock().await.set_database(&cloned_value).await {
                                Ok(_) => {
                                    cloned_sender
                                        .send(Event::OnMessage(Message {
                                            value: format!(
                                                "Database switched to '{}'",
                                                &cloned_value
                                            ),
                                            severity: Severity::Info,
                                        }))
                                        .unwrap();
                                }
                                Err(e) => {
                                    cloned_sender
                                        .send(Event::OnMessage(Message {
                                            value: e.to_string(),
                                            severity: Severity::Error,
                                        }))
                                        .unwrap();
                                }
                            }
                        })));
                    log_error!(self.info.event_sender, result.err());
                }
                ConnectionEvent::Connect(value) => {
                    let connector = self.connector.clone();
                    let cloned_value = value.clone();
                    let cloned_sender = self.info.event_sender.clone();
                    self.info
                        .event_sender
                        .send(Event::OnAsyncEvent(tokio::spawn(async move {
                            match connector
                                .lock()
                                .await
                                .set_connection(cloned_value.clone())
                                .await
                            {
                                Ok(info) => {
                                    cloned_sender
                                        .send(Event::OnMessage(Message {
                                            value: format!(
                                                "Connection switched to '{}'",
                                                &info.host.clone()
                                            ),
                                            severity: Severity::Info,
                                        }))
                                        .unwrap();
                                    cloned_sender
                                        .send(Event::OnConnection(
                                            ConnectionEvent::SwitchConnection(
                                                info.host.clone(),
                                                info.database.clone(),
                                            ),
                                        ))
                                        .unwrap()
                                }
                                Err(e) => {
                                    log_error!(cloned_sender, Some(e));
                                }
                            };
                        })));
                }
                _ => (),
            },
            Event::OnInput(value) => {
                if matches!(value.mode, crate::application::Mode::View) {
                    match value.key.code {
                        event::KeyCode::Char('i') => {
                            let original_query = self.query.clone();
                            self.query = EXTERNAL_EDITOR.edit_file(&MONGO_QUERY_FILE).unwrap();
                            if original_query == self.query {
                                return Ok(());
                            }
                            self.reset_state();
                            self.pagination.reset();
                            self.spawn_next_data();
                            value.terminal.lock().unwrap().clear()?;
                        }
                        event::KeyCode::Char('r') => {
                            self.reset_state();
                            self.pagination.reset();
                            self.spawn_next_data();
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
                        }
                        event::KeyCode::Up | event::KeyCode::Char('k') => {
                            self.handle_next_vertical_movement(VerticalDirection::Up)
                        }
                        event::KeyCode::Enter => {
                            if self.data.len() > 0 {
                                let data = self.data[self.state.get_vertical_select() - 1
                                    + self.state.get_vertical_offset()]
                                .clone();
                                EXTERNAL_EDITOR.edit_value(
                                    &mut serde_json::to_string_pretty(
                                        &Into::<serde_json::Value>::into(data),
                                    )?,
                                    FileType::Json,
                                )?;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::DatabaseData(value) => {
                log_error!(self.info.event_sender, self.set_data(value.clone()).err());
                self.is_fetching = false;
            }
            _ => {}
        }
        Ok(())
    }
}

impl<'a> From<DatabaseData> for TableData<'a> {
    fn from(value: DatabaseData) -> Self {
        let mut header = Row::default();
        let mut body = Vec::new();

        if !value.is_empty() {
            let mut unique_keys = value
                .iter()
                .fold(HashSet::new(), |mut acc, value| {
                    acc.extend(value.keys().cloned());

                    acc
                })
                .into_iter()
                .collect::<Vec<String>>();
            unique_keys.sort_by_key(|a| a.len());

            body = value
                .into_iter()
                .map(|value| {
                    //TODO: Error handling
                    let mut obj = try_from!(<Object>(value)).unwrap();

                    Row::new(unique_keys.iter().fold(Vec::new(), |mut acc, key| {
                        if obj.contains_key(key) {
                            acc.push(
                                Into::<serde_json::Value>::into(obj.remove(key).unwrap())
                                    .to_string(),
                            );
                        } else {
                            acc.push("".to_string());
                        }

                        acc
                    }))
                })
                .collect::<Vec<Row>>();
            header = Row::new(unique_keys.clone());
        }

        TableData { header, rows: body }
    }
}
