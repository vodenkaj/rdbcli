use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::Result;
use mongodb::event::command::ConnectionInfo;
use tokio::{task::JoinHandle, time};

use crate::{
    connectors::base::DatabaseFetchResult,
    managers::window_manager::WindowCommand,
    ui::{
        components::{base::Component, command::Message},
        window::OnInputInfo,
    },
};

pub enum ConnectionEvent {
    Add(ConnectionInfo),
    Connect(String),
    SwitchConnection(String, String),
    SwitchDatabase(String),
}

pub enum Event {
    OnInput(OnInputInfo),
    OnMessage(Message),
    DatabaseData(DatabaseFetchResult),
    OnQuery(String),
    OnWindowCommand(WindowCommand),
    OnConnection(ConnectionEvent),
    OnAsyncEvent(JoinHandle<()>),
    OnQuit(),
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
    AsyncEvent,
    OnQuit,
}

impl Event {
    pub fn get_type(&self) -> EventType {
        match self {
            Event::OnInput(_) => EventType::OnInput,
            Event::DatabaseData(_) => EventType::DatabaseData,
            Event::OnQuery(_) => EventType::OnQuery,
            Event::OnWindowCommand(_) => EventType::OnWindowCommand,
            Event::OnConnection(_) => EventType::OnConnection,
            Event::OnMessage(_) => EventType::OnMessage,
            Event::OnAsyncEvent(_) => EventType::AsyncEvent,
            Event::OnQuit() => EventType::OnQuit,
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

pub struct EventManager {
    pub sender: Sender<Event>,
    receiver: Receiver<Event>,
    async_events: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

pub trait EventHandler {
    fn on_event(&mut self, event: &Event) -> Result<()>;
}

impl EventManager {
    pub fn new() -> Self {
        let (sender, receiver) = channel();

        let async_events: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));

        let cloned_async_events = async_events.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;

                let event = cloned_async_events.lock().unwrap().pop();
                if let Some(event) = event {
                    event.await;
                }
            }
        });

        Self {
            sender,
            receiver,
            async_events,
        }
    }

    pub fn pool(&mut self, handlers: &mut Vec<Box<dyn Component>>) -> Result<bool> {
        let mut should_quit = false;
        while let Ok(event) = self.receiver.try_recv() {
            for handler in handlers.iter_mut() {
                handler.on_event(&event)?
            }

            if let Event::OnQuit() = event {
                should_quit = true;
            }
        }

        Ok(should_quit)
    }

    pub fn trigger(&self, event: JoinHandle<()>) {
        self.async_events.lock().unwrap().push(event);
    }
}
