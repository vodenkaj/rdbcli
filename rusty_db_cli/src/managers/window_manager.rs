use super::auth_manager::AuthManager;
use crate::{
    systems::event_system::{Event, EventHandler, EventPool, EventType, EventValue},
    ui::window::Window,
};
use anyhow::Result;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub enum WindowCommand {
    SetFocusedWindow(usize),
    SetFocusedComponent(usize),
}

pub struct WindowManager {
    pub windows: HashMap<usize, Arc<Mutex<Window>>>,
    pub focused_window: usize,
}

impl WindowManager {
    pub fn get_focused_window(&mut self) -> &mut Arc<Mutex<Window>> {
        self.windows.get_mut(&self.focused_window).unwrap()
    }
}

impl EventHandler for WindowManager {
    fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) -> Result<()> {
        if let EventValue::OnWindowCommand(cmd) = &event.value {
            match cmd {
                WindowCommand::SetFocusedWindow(value) => {
                    self.focused_window = value.clone();
                }
                WindowCommand::SetFocusedComponent(value) => {
                    self.get_focused_window()
                        .lock()
                        .unwrap()
                        .focused_component_idx = value.clone();
                }
            }
        }
        Ok(())
    }
}

pub struct WindowManagerBuilder {
    windows: HashMap<usize, Arc<Mutex<Window>>>,
    idx: usize,
}

impl WindowManagerBuilder {
    pub fn new() -> WindowManagerBuilder {
        Self {
            windows: HashMap::new(),
            idx: 0,
        }
    }

    pub fn with_window(mut self, window: Arc<Mutex<Window>>) -> Self {
        self.windows.insert(self.idx.clone(), window);
        self.idx += 1;
        self
    }

    pub fn build(mut self, auth_manager: Arc<Mutex<AuthManager>>) -> Arc<Mutex<WindowManager>> {
        let manager = Arc::new(Mutex::new(WindowManager {
            windows: self.windows,
            focused_window: 0,
        }));

        for window in manager.lock().unwrap().windows.values_mut() {
            let lock = window.lock().unwrap();
            let mut events = lock.event_manager.lock().unwrap();
            events.subscribe(window.clone(), EventType::OnInput);
            events.subscribe(manager.clone(), EventType::OnWindowCommand);
            events.subscribe(auth_manager.clone(), EventType::OnAuthCommand);
        }

        manager
    }
}
