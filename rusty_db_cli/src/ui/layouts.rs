use super::{
    components::{
        base::ComponentCreateInfo,
        command::{CommandComponent, Message},
        scrollable_table::ScrollableTableComponent,
    },
    window::{Window, WindowBuilder},
};
use crate::{
    connectors::{base::TableData, mongodb::connector::MongodbConnectorBuilder},
    systems::event_system::EventManager,
    widgets::scrollable_table::ScrollableTableState,
};
use ratatui::layout::Constraint;
use std::{env, sync::Arc};

pub async fn get_table_layout() -> Window {
    let event_manager = EventManager::new();
    let (_, db_uri) = env::vars()
        .find(|(key, _)| *key == String::from("DB_URI"))
        .expect("DB_URI to be present");

    let connector = if db_uri.contains("mongodb") {
        MongodbConnectorBuilder::new(&db_uri).build().await
    } else {
        panic!("Other connectors are not implemented");
    }
    .expect("Failed to create DB connector");

    let table = ScrollableTableComponent::new(
        ComponentCreateInfo {
            constraint: Constraint::Min(0),
            data: TableData::default(),
            focusable: true,
            id: 0,
            visible: true,
            event_sender: event_manager.sender.clone(),
        },
        ScrollableTableState::default(),
        Arc::new(tokio::sync::Mutex::new(connector)),
    );

    let command = CommandComponent::new(ComponentCreateInfo {
        focusable: true,
        visible: true,
        constraint: Constraint::Length(1),
        data: Message::default(),
        id: 1,
        event_sender: event_manager.sender.clone(),
    });

    WindowBuilder::new()
        .with_component(Box::new(table))
        .with_component(Box::new(command))
        .build(event_manager)
}
