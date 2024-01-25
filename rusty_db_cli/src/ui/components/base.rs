use std::sync::mpsc::Sender;
use crate::systems::event_system::{EventHandler, Event};
use ratatui::{
    layout::{Constraint, Rect},
    Frame,
};

pub struct ComponentCreateInfo<T> {
    pub id: usize,
    pub constraint: Constraint,
    pub data: T,
    pub focusable: bool,
    pub visible: bool,
    pub event_sender: Sender<Event>
}

pub trait Component: EventHandler {
    fn get_constraint(&self) -> Constraint;
    fn is_visible(&self) -> bool;
    fn set_visibility(&mut self, visible: bool) -> bool;
    fn draw(&mut self, info: ComponentDrawInfo);
    fn as_event_handler(&self) -> &dyn EventHandler where Self: std::marker::Sized {
        self
    }
}

pub struct ComponentDrawInfo<'a, 'b> {
    pub frame: &'a mut Frame<'b>,
    pub area: Rect,
}
