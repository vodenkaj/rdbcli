use std::sync::{Arc, Mutex};

use anyhow::{Ok, Result};
use async_trait::async_trait;
use crossterm::event;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    managers::{connection_manager::ConnectionEvent, window_manager::WindowCommand},
    systems::event_system::{Event, EventHandler, EventPool, EventValue},
};

use super::base::{Component, ComponentCreateInfo, ComponentDrawInfo};

#[derive(Clone, Default)]
pub struct ConnectionInfo {
    pub uri: String,
    pub database_type: DatabaseType,
}

#[derive(Clone, Default)]
enum DatabaseType {
    #[default]
    Mongo,
    PostgreSQL,
}

enum FocusedInput {
    Type,
    Uri,
}

pub struct ConnectionComponent {
    info: ComponentCreateInfo<ConnectionInfo>,
    focused: FocusedInput,
}

impl ConnectionComponent {
    pub fn new(info: ComponentCreateInfo<ConnectionInfo>) -> Self {
        Self {
            info,
            focused: FocusedInput::Type,
        }
    }

    fn reset(&mut self) {
        self.info.visible = false;
        self.info.data = ConnectionInfo::default();
        self.focused = FocusedInput::Type;
    }
}

impl Component for ConnectionComponent {
    fn set_visibility(&mut self, visible: bool) -> bool {
        self.info.visible = visible;
        visible
    }

    fn is_visible(&self) -> bool {
        self.info.visible
    }

    fn get_constraint(&self) -> Constraint {
        self.info.constraint
    }

    fn draw(&mut self, info: ComponentDrawInfo) {
        let block = Block::default()
            .title("Add connection")
            .borders(Borders::ALL);
        let area = centered_rect(60, 20, info.area);
        info.frame.render_widget(Clear, area);
        let uri_value = format!("Uri: {}", self.info.data.uri);

        let text = vec![
            Line::from(vec![
                Span::raw("Type:"),
                Span::styled(
                    "Mongo",
                    if let DatabaseType::Mongo = self.info.data.database_type {
                        Style::new().green()
                    } else {
                        Style::new()
                    },
                ),
                Span::styled(
                    "PostgreSQL",
                    if let DatabaseType::PostgreSQL = self.info.data.database_type {
                        Style::new().green()
                    } else {
                        Style::new()
                    },
                ),
            ]),
            Line::from(uri_value),
        ];

        info.frame
            .render_widget(Paragraph::new(text).block(block), area);
    }
}

#[async_trait]
impl EventHandler for ConnectionComponent {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        if let EventValue::OnInput(value) = &event.value {
            match self.focused {
                FocusedInput::Type => match value.key.code {
                    event::KeyCode::Enter | event::KeyCode::Down => {
                        self.focused = FocusedInput::Uri
                    }
                    event::KeyCode::Right => {
                        self.info.data.database_type = DatabaseType::PostgreSQL
                    }
                    event::KeyCode::Left => self.info.data.database_type = DatabaseType::Mongo,
                    _ => {}
                },
                FocusedInput::Uri => match value.key.code {
                    event::KeyCode::Char(ch) => {
                        self.info.data.uri += &ch.to_string();
                    }
                    event::KeyCode::Backspace => {
                        self.info.data.uri.pop();
                    }
                    event::KeyCode::Up => self.focused = FocusedInput::Type,
                    event::KeyCode::Enter => {
                        value
                            .connection_manager
                            .lock()
                            .unwrap()
                            .connections
                            .push(self.info.data.clone());
                        pool.lock().unwrap().trigger(Event {
                            component_id: 1,
                            value: EventValue::OnConnection(ConnectionEvent::Add(
                                self.info.data.clone(),
                            )),
                        });
                        pool.lock().unwrap().trigger(Event {
                            component_id: 1,
                            value: EventValue::OnWindowCommand(WindowCommand::SetFocusedComponent(
                                0,
                            )),
                        });
                        self.reset();
                    }
                    _ => {}
                },
            }
        }
        Ok(())
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
