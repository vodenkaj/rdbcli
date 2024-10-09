use std::{any::Any, sync::mpsc::Sender};

use ratatui::{
    layout::{Constraint, Rect},
    Frame,
};

use crate::managers::event_manager::{Event, EventHandler};

pub struct ComponentCreateInfo<T> {
    pub id: usize,
    pub constraint: Constraint,
    pub data: T,
    pub focusable: bool,
    pub visible: bool,
    pub event_sender: Sender<Event>,
    pub is_focused: bool,
}

pub trait Component: EventHandler + Send {
    fn get_constraint(&self) -> Constraint;
    fn is_visible(&self) -> bool;
    fn set_visibility(&mut self, visible: bool) -> bool;
    fn draw(&mut self, info: ComponentDrawInfo);
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct ComponentDrawInfo<'a, 'b> {
    pub frame: &'a mut Frame<'b>,
    pub area: Rect,
}
