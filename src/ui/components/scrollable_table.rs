use std::{
    cmp,
    sync::{Arc, Mutex},
};

use super::base::{Component, ComponentCreateInfo, ComponentDrawInfo};
use crate::{
    connectors::base::{Connector, TableData},
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
    state: ScrollableTableState,
    query: String,
    connector: Box<dyn Connector>,
    horizontal_offset: i32,
    vertical_offset: i32,
    horizontal_offset_max: i32,
    vertical_offset_max: i32,
}

impl ScrollableTableComponent {
    pub fn new(
        info: ComponentCreateInfo<TableData<'static>>,
        state: ScrollableTableState,
        connector: Box<dyn Connector>,
    ) -> Self {
        Self {
            query: String::new(),
            info,
            state,
            connector,
            horizontal_offset: 0,
            vertical_offset: 0,
            horizontal_offset_max: 0,
            vertical_offset_max: 0,
        }
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
            self.state
                .set_vertical_select(self.vertical_offset as usize);
        }
    }

    async fn refetch_data(&mut self) -> anyhow::Result<()> {
        let data = self.connector.get_data(&self.query).await?;
        self.info.data = TableData::from(Arc::new(data));
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
                    self.handle_next_vertical_movement(VerticalDirection::Down);
                }
                event::KeyCode::Up | event::KeyCode::Char('k') => {
                    self.handle_next_vertical_movement(VerticalDirection::Up);
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
