use super::components::base::{Component, ComponentDrawInfo};
use crate::{
    application::Mode,
    managers::{connection_manager::ConnectionManager, window_manager::WindowManager},
    systems::event_system::{Event, EventHandler, EventManager, EventPool, EventValue},
};
use anyhow::Result;
use crossterm::event;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    Terminal,
};
use std::{
    collections::HashMap,
    io::Stdout,
    sync::{Arc, Mutex},
};

pub struct WindowRenderInfo {
    pub terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    pub mode: Mode,
}

pub struct WindowBuilder {
    components: Vec<Arc<Mutex<dyn Component>>>,
    keybinds: HashMap<event::KeyCode, Box<dyn Fn(&mut Window) + Send + Sync>>,
}

impl EventHandler for Window {
    fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        if let EventValue::OnInput(value) = &event.value {
            match value.key.code {
                event::KeyCode::Char(ch) => {
                    if let Some(handler) = self.keybinds.remove(&value.key.code) {
                        handler(self);
                        self.keybinds.insert(value.key.code, handler);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl WindowBuilder {
    pub fn new() -> Self {
        WindowBuilder {
            components: Vec::new(),
            keybinds: HashMap::new(),
        }
    }

    pub fn with_component(mut self, component: Arc<Mutex<dyn Component>>) -> Self {
        self.components.push(component);
        self
    }

    pub fn build(self, event_manager: Arc<Mutex<EventManager>>) -> Window {
        if self.components.len() <= 0 {
            panic!("Cannot build window without any component");
        }

        Window {
            id: 0,
            event_manager,
            components: self.components,
            focused_component_idx: 0,
            keybinds: self.keybinds,
        }
    }
}

pub struct Window {
    id: usize,
    pub event_manager: Arc<Mutex<EventManager>>,
    components: Vec<Arc<Mutex<dyn Component>>>,
    pub focused_component_idx: usize,
    keybinds: HashMap<event::KeyCode, Box<dyn Fn(&mut Self) + Send + Sync>>,
}

impl Window {
    pub fn with_keybind(
        &mut self,
        bind: event::KeyCode,
        action: Box<dyn Fn(&mut Self) + Send + Sync>,
    ) {
        self.keybinds.insert(bind, action);
    }

    pub fn render(&mut self, info: WindowRenderInfo) {
        info.terminal
            .lock()
            .unwrap()
            .draw(|f| {
                match info.mode {
                    Mode::View | Mode::Input => {
                        let mut components: Vec<_> = self
                            .components
                            .iter_mut()
                            .map(|c| match c.lock() {
                                Ok(value) => value,
                                Err(value) => {
                                    let guard = value.into_inner();
                                    // TODO: Logger?
                                    println!("Thread recovered from mutex poisoning");
                                    guard
                                }
                            })
                            .filter(|w| w.is_visible())
                            .collect();
                        let constraints: Vec<Constraint> =
                            components.iter().map(|w| w.get_constraint()).collect();
                        let chunks = Layout::default()
                            .direction(ratatui::layout::Direction::Vertical)
                            .constraints(constraints)
                            .split(f.size());

                        for (pos, component) in components.iter_mut().enumerate() {
                            component.draw(ComponentDrawInfo {
                                frame: f,
                                area: chunks[pos],
                            });
                        }
                    }
                }
            })
            .unwrap();
    }

    pub async fn on_key(&mut self, info: OnInputInfo) -> anyhow::Result<()> {
        self.event_manager
            .lock()
            .unwrap()
            .trigger_event_sync(Event {
                component_id: self.focused_component_idx,
                value: EventValue::OnInput(info),
            })?;
        Ok(())
    }

    fn get_focused_component(&mut self) -> Arc<Mutex<dyn Component>> {
        self.components[self.focused_component_idx].clone()
    }
}

pub struct OnInputInfo {
    pub window_manager: Arc<Mutex<WindowManager>>,
    pub connection_manager: Arc<Mutex<ConnectionManager>>,
    pub terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    pub mode: Mode,
    pub key: event::KeyEvent,
}
