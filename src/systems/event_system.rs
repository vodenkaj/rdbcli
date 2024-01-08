use crate::{
    connectors::base::DatabaseData,
    managers::{
        auth_manager::AuthCommand, connection_manager::ConnectionEvent,
        window_manager::WindowCommand,
    },
    ui::{components::command::Message, window::OnInputInfo},
};
use anyhow::Result;
use core::time;
use futures::executor::block_on;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
};

pub struct Event {
    pub component_id: usize,
    pub value: EventValue,
}

pub enum EventValue {
    OnInput(OnInputInfo),
    OnMessage(Message),
    DatabaseData(Arc<DatabaseData>),
    OnQuery(String),
    OnAuthCommand(AuthCommand),
    OnWindowCommand(WindowCommand),
    OnConnection(ConnectionEvent),
}

#[derive(Eq, Hash, PartialEq, Debug)]
pub enum EventType {
    OnInput,
    DatabaseData,
    OnQuery,
    OnWindowCommand,
    OnAuthCommand,
    OnConnection,
    OnMessage,
}

impl Event {
    pub fn get_type(&self) -> EventType {
        match self.value {
            EventValue::OnInput(_) => EventType::OnInput,
            EventValue::DatabaseData(_) => EventType::DatabaseData,
            EventValue::OnQuery(_) => EventType::OnQuery,
            EventValue::OnAuthCommand(_) => EventType::OnAuthCommand,
            EventValue::OnWindowCommand(_) => EventType::OnWindowCommand,
            EventValue::OnConnection(_) => EventType::OnConnection,
            EventValue::OnMessage(_) => EventType::OnMessage,
        }
    }
}

#[derive(Default)]
pub struct EventPool {
    events: Vec<Arc<Event>>,
}

impl EventPool {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn trigger(&mut self, event: Event) {
        self.events.push(Arc::new(event));
    }
}

#[derive(Default)]
pub struct EventManager {
    pub handlers: HashMap<EventType, Vec<Arc<Mutex<dyn EventHandler>>>>,
    pub pool: Arc<Mutex<EventPool>>,
}

pub trait EventHandler: Send {
    fn on_event(&mut self, event: (&Event, Arc<Mutex<EventPool>>)) -> Result<()>;
}

impl EventManager {
    pub fn new() -> Arc<Mutex<Self>> {
        let manager = Arc::new(Mutex::new(Self::default()));
        let cloned = manager.clone();
        thread::spawn(move || loop {
            cloned.lock().expect("Event manager to be poisoned").pool();
            thread::sleep(time::Duration::from_millis(10));
        });

        manager
    }

    fn pool(&mut self) {
        let events;
        {
            let mut guard = self.pool.lock().unwrap();
            events = guard.events.clone();
            guard.events.clear();
        }
        for event in events {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: Arc<Event>) -> Result<()> {
        if let Some(handlers) = self.handlers.get_mut(&event.get_type()) {
            for handler in handlers.iter_mut() {
                handler
                    .lock()
                    .unwrap()
                    .on_event((&event, self.pool.clone()))?
            }
        }
        Ok(())
    }

    pub fn subscribe(&mut self, handler: Arc<Mutex<dyn EventHandler>>, event_type: EventType) {
        self.handlers.entry(event_type).or_default().push(handler);
    }

    pub async fn trigger_event_async(&mut self, event: Event) {
        self.pool.lock().unwrap().events.push(Arc::new(event));
    }

    pub fn trigger_event_sync(&mut self, event: Event) -> Result<()> {
        self.handle_event(Arc::new(event))
    }
}
