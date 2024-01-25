use crate::{
    systems::event_system::{Event, EventHandler},
    ui::window::Window,
};
use anyhow::Result;
use std::collections::HashMap;

pub enum WindowCommand {
    SetFocusedWindow(usize),
    SetFocusedComponent(usize),
}

pub struct WindowManager {
    pub windows: HashMap<usize, Window>,
    pub focused_window: usize,
}

impl WindowManager {
    pub fn get_focused_window(&mut self) -> &mut Window {
        self.windows.get_mut(&self.focused_window).unwrap()
    }
}

impl EventHandler for WindowManager {
    fn on_event(&mut self, event: &Event) -> Result<()> {
        if let Event::OnWindowCommand(cmd) = &event {
            match cmd {
                WindowCommand::SetFocusedWindow(value) => {
                    self.focused_window = value.clone();
                }
                WindowCommand::SetFocusedComponent(value) => {
                    self.get_focused_window().focused_component_idx = value.clone();
                }
            }
        }
        Ok(())
    }
}

pub struct WindowManagerBuilder {
    windows: HashMap<usize, Window>,
    idx: usize,
}

impl WindowManagerBuilder {
    pub fn new() -> WindowManagerBuilder {
        Self {
            windows: HashMap::new(),
            idx: 0,
        }
    }

    pub fn with_window(mut self, window: Window) -> Self {
        self.windows.insert(self.idx.clone(), window);
        self.idx += 1;
        self
    }

    pub fn build(mut self) -> WindowManager {
        WindowManager {
            windows: self.windows,
            focused_window: 0,
        }
    }
}
