use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::Result;
use tokio::{task::JoinHandle, time};

use super::resource_manager::{Resource, ResourceManager};
use crate::{
    connectors::base::{ConnectorInfo, DatabaseFetchResult, PaginationInfo},
    log_error,
    managers::window_manager::WindowCommand,
    ui::{components::command::Message, window::OnInputInfo},
};

pub enum ConnectionEvent {
    Connect(String),
    SwitchConnection(ConnectorInfo),
    SwitchDatabase(String),
}

#[derive(Clone)]
pub struct QueryEvent {
    pub query: String,
    pub pagination: PaginationInfo,
}

pub enum ResourceEvent {
    Add(Box<dyn Resource>),
    Update(Box<dyn Resource>),
}

pub enum Event {
    OnInput(OnInputInfo),
    OnMessage(Message),
    DatabaseData(DatabaseFetchResult),
    OnQuery(QueryEvent),
    OnWindowCommand(WindowCommand),
    OnConnection(ConnectionEvent),
    OnAsyncEvent(JoinHandle<()>),
    OnResourceEvent(ResourceEvent),
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
    OnResourceEvent,
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
            Event::OnResourceEvent(_) => EventType::OnResourceEvent,
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
    fn as_mut_event_handler(&mut self) -> &mut dyn EventHandler;
}

impl EventManager {
    pub fn new() -> Self {
        let (sender, receiver) = channel();

        let async_events: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));

        let cloned_async_events = async_events.clone();
        let cloned_sender = sender.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;

                let event = cloned_async_events.lock().unwrap().pop();
                let cloned_sender_2 = cloned_sender.clone();
                if let Some(event) = event {
                    tokio::spawn(async move {
                        if let Err(e) = event.await {
                            log_error!(cloned_sender_2, Some(e));
                        }
                    });
                }
            }
        });

        Self {
            sender,
            receiver,
            async_events,
        }
    }

    pub fn pool(
        &mut self,
        handlers: &mut [Box<&mut (impl EventHandler + ?Sized)>],
        resource_manager: &mut ResourceManager,
    ) -> Result<bool> {
        let mut should_quit = false;

        while let Ok(event) = self.receiver.try_recv() {
            if let Event::OnResourceEvent(resource_event) = event {
                resource_manager.on_event(resource_event)?;
                continue;
            }

            for handler in handlers.iter_mut() {
                handler.on_event(&event)?
            }

            for handler in resource_manager.resources.iter_mut() {
                handler.on_event(&event)?
            }

            if let Event::OnQuit() = event {
                should_quit = true;
            }
        }

        Ok(should_quit)
    }

    //pub fn pool_component(&mut self, handlers: &mut Vec<Box<dyn Component>>) -> Result<bool> {
    //    let mut should_quit = false;
    //    while let Ok(event) = self.receiver.try_recv() {
    //        for handler in handlers.iter_mut() {
    //            handler.on_event(&event)?
    //        }

    //        if let Event::OnQuit() = event {
    //            should_quit = true;
    //        }
    //    }

    //    Ok(should_quit)
    //}

    //pub fn pool_resource(&mut self, handlers: &mut Vec<Box<dyn Resource>>) -> Result<bool> {
    //    let mut should_quit = false;
    //    while let Ok(event) = self.receiver.try_recv() {
    //        for handler in handlers.iter_mut() {
    //            handler.on_event(&event)?
    //        }

    //        if let Event::OnQuit() = event {
    //            should_quit = true;
    //        }
    //    }

    //    Ok(should_quit)
    //}

    pub fn trigger(&self, event: JoinHandle<()>) {
        self.async_events.lock().unwrap().push(event);
    }
}
