use crate::systems::event_system::EventHandler;
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
}

pub trait Component: EventHandler {
    fn get_constraint(&self) -> Constraint;
    fn is_visible(&self) -> bool;
    fn set_visibility(&mut self, visible: bool) -> bool;
    fn draw(&mut self, info: ComponentDrawInfo);
}

pub struct ComponentDrawInfo<'a, 'b> {
    pub frame: &'a mut Frame<'b>,
    pub area: Rect,
}
