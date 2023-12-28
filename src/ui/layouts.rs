use super::{
    components::{
        base::{Component, ComponentCreateInfo},
        connection::{ConnectionComponent, ConnectionInfo},
        connection_list::ConnectionListComponent,
        input::InputComponent,
        login::LoginComponent,
        paragraph::ParagraphComponent,
        scrollable_table::ScrollableTableComponent,
    },
    window::{Window, WindowBuilder},
};
use crate::{
    connectors::{
        base::TableData,
        mongodb::connector::{MongodbConnector, MongodbConnectorBuilder},
    },
    managers::auth_manager::AuthManager,
    systems::event_system::{EventManager, EventType},
    widgets::scrollable_table::ScrollableTableState,
};
use crossterm::event;
use ratatui::layout::Constraint;
use std::{
    env,
    fs::File,
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};

pub async fn get_table_layout() -> Arc<Mutex<Window>> {
    let event_manager = EventManager::new();
    let mut events = event_manager.lock().unwrap();
    let (_, db_uri) = env::vars()
        .find(|(key, _)| *key == String::from("DB_URI"))
        .expect("DB_URI to be present");

    let connector = if db_uri.contains("mongodb") {
        MongodbConnectorBuilder::new(&db_uri)
            .build()
            .await
            .expect("Mongodb connector to be build")
    } else {
        //TODO: POSTGRES
        MongodbConnectorBuilder::new(&db_uri)
            .build()
            .await
            .expect("Mongodb connector to be build")
    };

    let table = Arc::new(Mutex::new(ScrollableTableComponent::new(
        ComponentCreateInfo {
            constraint: Constraint::Min(0),
            data: TableData::default(),
            focusable: true,
            id: 0,
            visible: true,
        },
        ScrollableTableState::default(),
        Box::new(connector),
    )));
    events.subscribe(table.clone(), EventType::DatabaseData);
    events.subscribe(table.clone(), EventType::OnInput);

    let command = Arc::new(Mutex::new(InputComponent::new(
        ComponentCreateInfo {
            focusable: true,
            visible: true,
            constraint: Constraint::Length(1),
            data: String::new(),
            id: 1,
        },
        false,
    )));
    events.subscribe(command.clone(), EventType::OnInput);

    Arc::new(Mutex::new(
        WindowBuilder::new()
            .with_component(table)
            .with_component(command)
            .build(event_manager.clone()),
    ))
}

pub fn get_connections_layout(auth_manager: Arc<Mutex<AuthManager>>) -> Arc<Mutex<Window>> {
    let event_manager = EventManager::new();
    let mut events = event_manager.lock().unwrap();
    // TODO: load connections from file
    let auth_manager_guard = auth_manager.lock().unwrap();
    let password = auth_manager_guard.get_password().unwrap();

    let home_dir = home::home_dir().unwrap();
    let file_dir = format!("{}/.rustydbcli", home_dir.to_str().unwrap());
    let path = Path::new(&file_dir);
    if !path.try_exists().unwrap() {
        File::create(path).unwrap();
    }
    let mut buf = Vec::new();
    File::open(path).unwrap().read_to_end(&mut buf).unwrap();

    let list = Arc::new(Mutex::new(ConnectionListComponent::new(
        ComponentCreateInfo {
            focusable: true,
            visible: true,
            constraint: Constraint::Length(0),
            data: Vec::new(),
            id: 0,
        },
    )));
    events.subscribe(list.clone(), EventType::OnConnectionAdd);
    events.subscribe(list.clone(), EventType::OnInput);

    let add_connection = Arc::new(Mutex::new(ConnectionComponent::new(ComponentCreateInfo {
        visible: false,
        constraint: Constraint::Length(0),
        data: ConnectionInfo::default(),
        focusable: true,
        id: 1,
    })));
    events.subscribe(add_connection.clone(), EventType::OnInput);

    let cloned_comp = add_connection.clone();
    let cloned_comp_2 = add_connection.clone();

    let window = Arc::new(Mutex::new(
        WindowBuilder::new()
            .with_component(list)
            .with_component(add_connection)
            .build(event_manager.clone()),
    ));
    window.lock().unwrap().with_keybind(
        event::KeyCode::Char('a'),
        Box::new(move |window| {
            cloned_comp.lock().unwrap().set_visibility(true);
            window.focused_component_idx = 1;
        }),
    );
    window.lock().unwrap().with_keybind(
        event::KeyCode::Esc,
        Box::new(move |window| {
            cloned_comp_2.lock().unwrap().set_visibility(false);
            window.focused_component_idx = 0;
        }),
    );

    window
}

pub fn get_login_layout() -> Arc<Mutex<Window>> {
    let event_manager = EventManager::new();
    let mut events = event_manager.lock().unwrap();

    let input = Arc::new(Mutex::new(LoginComponent::new(
        ComponentCreateInfo {
            visible: true,
            focusable: true,
            constraint: Constraint::Length(1),
            data: String::new(),
            id: 0,
        },
        true,
    )));
    events.subscribe(input.clone(), EventType::OnInput);

    let paragraph = Arc::new(Mutex::new(ParagraphComponent::new(ComponentCreateInfo {
        visible: true,
        focusable: false,
        constraint: Constraint::Length(1),
        data: String::from("Log in - press ':' and type your system password"),
        id: 1,
    })));

    Arc::new(Mutex::new(
        WindowBuilder::new()
            .with_component(paragraph)
            .with_component(input)
            .build(event_manager.clone()),
    ))
}
