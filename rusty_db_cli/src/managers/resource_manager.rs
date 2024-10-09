use std::any::Any;

use super::event_manager::{EventHandler, ResourceEvent};

pub struct ResourceManager {
    pub resources: Vec<Box<dyn Resource>>,
}

pub trait Resource: EventHandler + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
        }
    }

    pub fn on_event(&mut self, event: ResourceEvent) -> anyhow::Result<()> {
        match event {
            super::event_manager::ResourceEvent::Add(event_add) => {
                self.resources.push(event_add);
            }
            super::event_manager::ResourceEvent::Update(event_update) => {
                if self.resources.is_empty() {
                    self.resources.push(event_update);
                } else {
                    self.resources[0] = event_update;
                }
            }
        }

        Ok(())
    }
}
