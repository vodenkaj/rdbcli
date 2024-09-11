use std::sync::Arc;

use clap::Parser;
use once_cell::sync::Lazy;
use ratatui::layout::Constraint;

use super::{
    components::{
        base::ComponentCreateInfo,
        command::{CommandComponent, Message},
        scrollable_table::ScrollableTableComponent,
        status_line::{StatusLineComponent, StatusLineData},
    },
    window::{Window, WindowBuilder},
};
use crate::{
    connectors::{
        base::{Connector, TableData},
        mongodb::connector::MongodbConnectorBuilder,
    },
    managers::event_manager::EventManager,
    widgets::scrollable_table::ScrollableTableState,
};

#[derive(clap::Parser)]
pub struct CliArgs {
    /// Value in format like this: mongodb+srv://[username:password@]host[/[defaultauthdb][?options]]
    #[clap(name = "DATABASE_URI")]
    pub database_uri: String,

    /// Enables debug logs, that are stored in $HOME/.config/rusty-db-cli/debug.log
    #[arg(long, default_value_t = false)]
    pub debug: bool,

    /// Disables storing of command history into the file located in
    /// $HOME/.config/rusty-db-cli/.command_history.txt
    #[arg(long, name="disable-command-history", default_value_t = false, action = clap::ArgAction::SetTrue)]
    pub disable_command_history: bool,
}

pub static CLI_ARGS: Lazy<CliArgs> = Lazy::new(CliArgs::parse);

pub async fn get_table_layout() -> Window {
    let event_manager = EventManager::new();

    let connector = if CLI_ARGS.database_uri.contains("mongodb") {
        MongodbConnectorBuilder::new(&CLI_ARGS.database_uri)
            .build()
            .await
    } else {
        panic!("Other connectors are not implemented");
    }
    .expect("Failed to create DB connector");

    let status_line = StatusLineComponent::new(ComponentCreateInfo {
        focusable: true,
        visible: true,
        constraint: Constraint::Length(1),
        data: StatusLineData {
            host: connector.get_info().host.clone(),
            database_name: connector.database.clone(),
        },
        id: 2,
        event_sender: event_manager.sender.clone(),
        is_focused: false,
    });

    let table = ScrollableTableComponent::new(
        ComponentCreateInfo {
            constraint: Constraint::Min(0),
            data: TableData::default(),
            focusable: true,
            id: 0,
            visible: true,
            event_sender: event_manager.sender.clone(),
            is_focused: true,
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
        is_focused: false,
    });

    WindowBuilder::new()
        .with_component(Box::new(table))
        .with_component(Box::new(status_line))
        .with_component(Box::new(command))
        .build(event_manager)
}
