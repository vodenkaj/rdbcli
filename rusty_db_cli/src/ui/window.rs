use std::{
    collections::HashMap,
    io::Stdout,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use crossterm::event;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    Terminal,
};

use super::components::base::{Component, ComponentDrawInfo};
use crate::{
    application::Mode,
    managers::event_manager::{Event, EventHandler, EventManager},
};

pub struct WindowRenderInfo {
    pub terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    pub mode: Mode,
}

pub struct WindowBuilder {
    components: Vec<Box<dyn Component>>,
    keybinds: HashMap<event::KeyCode, Box<dyn Fn(&mut Window) + Send + Sync>>,
}

impl EventHandler for Window {
    fn on_event(&mut self, event: &Event) -> Result<()> {
        if let Event::OnInput(value) = &event {
            match value.key.code {
                event::KeyCode::Char(_ch) => {
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

    pub fn with_component(mut self, component: Box<dyn Component>) -> Self {
        self.components.push(component);
        self
    }

    pub fn build(self, event_manager: EventManager) -> Window {
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
    pub event_manager: EventManager,
    components: Vec<Box<dyn Component>>,
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
        self.event_manager.pool(&mut self.components);

        info.terminal
            .lock()
            .unwrap()
            .draw(|f| match info.mode {
                Mode::View | Mode::Input => {
                    let mut components: Vec<_> = self
                        .components
                        .iter_mut()
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
            })
            .unwrap();
    }

    pub fn on_key(&mut self, event: Event) {
        self.event_manager.sender.send(event).unwrap();
        self.event_manager.pool(&mut self.components).unwrap();
    }
}

pub struct OnInputInfo {
    pub terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    pub mode: Mode,
    pub key: event::KeyEvent,
}
