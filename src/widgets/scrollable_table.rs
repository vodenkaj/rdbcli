use std::cmp;

use ratatui::{
    prelude::{Buffer, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, StatefulWidget, Widget},
};

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Cell<'a> {
    content: Text<'a>,
    style: Style,
}

impl<'a, T> From<T> for Cell<'a>
where
    T: Into<Text<'a>>,
{
    fn from(content: T) -> Cell<'a> {
        Cell {
            content: content.into(),
            style: Style::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Row<'a> {
    pub cells: Vec<Cell<'a>>,
    height: u16,
    style: Style,
    bottom_margin: u16,
}

impl<'a> Row<'a> {
    /// Creates a new [`Row`] from an iterator where items can be converted to a [`Cell`].
    pub fn new<T>(cells: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<Cell<'a>>,
    {
        Self {
            height: 1,
            cells: cells.into_iter().map(Into::into).collect(),
            style: Style::default(),
            bottom_margin: 0,
        }
    }

    /// Returns the total height of the row.
    fn total_height(&self) -> u16 {
        self.height.saturating_add(self.bottom_margin)
    }
}

pub struct ScrollableTable<'a> {
    rows: Vec<Row<'a>>,
    block: Block<'a>,
    header: Row<'a>,
}

pub struct ScrollableTableState {
    horizontal_offset: usize,
    vertical_offset: usize,
    vertical_select: usize,
}

impl ScrollableTableState {
    pub fn set_vertical_select(&mut self, idx: usize) {
        self.vertical_select = idx;
    }

    pub fn set_horizontal_offset(&mut self, offset: usize) {
        self.horizontal_offset = offset;
    }

    pub fn set_vertical_offset(&mut self, offset: usize) {
        self.vertical_offset = offset;
    }

    pub fn get_vertical_select(&self) -> usize {
        self.vertical_select
    }

    pub fn get_vertical_offset(&self) -> usize {
        self.vertical_offset
    }

    pub fn reset(&mut self) {
        self.set_horizontal_offset(0);
        self.set_vertical_offset(0);
        self.set_vertical_select(1);
    }
}

impl<'a> ScrollableTable<'a> {
    pub fn new(rows: Vec<Row<'a>>, header: Row<'a>) -> Self {
        Self {
            rows,
            block: Block::default(),
            header,
        }
    }
}

impl<'a> Default for ScrollableTable<'a> {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            block: Block::default(),
            header: Row::default(),
        }
    }
}

impl Default for ScrollableTableState {
    fn default() -> Self {
        Self {
            horizontal_offset: 0,
            vertical_offset: 0,
            vertical_select: 1,
        }
    }
}

impl<'a> StatefulWidget for ScrollableTable<'a> {
    type State = ScrollableTableState;

    fn render(
        mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        if area.area() == 0 {
            return;
        }

        let table_area = self.block.inner(area);
        self.block.render(area, buf);

        buf.set_style(area, Style::default());

        render_row(
            &self.header,
            Rect {
                x: 0,
                y: 0,
                width: table_area.right(),
                height: 1,
            },
            buf,
            state,
        );
        for (i, table_row) in self
            .rows
            .iter_mut()
            .skip(state.vertical_offset)
            .enumerate()
            .take(area.bottom() as usize - 1)
        {
            let table_row_area = Rect {
                x: 0,
                y: (i + 1) as u16,
                width: table_area.right(),
                height: table_row.total_height(),
            };
            render_row(table_row, table_row_area, buf, state)
        }
    }
}
fn render_row<'a>(row: &Row<'a>, area: Rect, buf: &mut Buffer, state: &ScrollableTableState) {
    let style = match state.vertical_select > 0 && area.y as usize == state.vertical_select {
        true => Style::default().bg(Color::Yellow).fg(Color::Black),
        false => Style::default(),
    };
    buf.set_style(area, style);
    for (x, cell) in row
        .cells
        .iter()
        .skip(state.horizontal_offset)
        .enumerate()
        .take((area.width / 10) as usize)
    {
        for (i, line) in cell.content.lines.iter().enumerate() {
            let area = Rect {
                x: (10 * x) as u16,
                y: area.y + i as u16,
                width: 9,
                height: row.total_height(),
            };
            buf.set_style(area, cell.style);
            buf.set_line(area.x, area.y + i as u16, line, area.width);
        }
    }
}

impl<'a> Widget for ScrollableTable<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let mut state = ScrollableTableState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

pub struct ScrollableTableHandler {
    horizontal_offset: i32,
    vertical_offset: i32,
    horizontal_offset_max: i32,
    vertical_offset_max: i32,
}

pub enum VerticalDirection {
    Up,
    Down,
}

pub enum HorizontalDirection {
    Left,
    Right,
}

impl ScrollableTableHandler {
    pub fn handle_next_horizontal_movement(
        &mut self,
        state: &mut ScrollableTableState,
        dir: HorizontalDirection,
    ) {
        match dir {
            HorizontalDirection::Right => {
                self.horizontal_offset =
                    cmp::min(self.horizontal_offset + 1, self.horizontal_offset_max);
            }
            HorizontalDirection::Left => {
                self.horizontal_offset = cmp::max(self.horizontal_offset - 1, 0);
            }
        }

        state.set_horizontal_offset(self.horizontal_offset as usize);
    }

    pub fn handle_next_vertical_movement(
        &mut self,
        state: &mut ScrollableTableState,
        dir: VerticalDirection,
    ) {
        let current_select = state.get_vertical_select();

        match dir {
            VerticalDirection::Down => {
                self.vertical_offset = cmp::min(self.vertical_offset + 1, self.vertical_offset_max);
            }
            VerticalDirection::Up => {
                self.vertical_offset = cmp::max(self.vertical_offset - 1, 1);
            }
        }

        if self.vertical_offset > 10 {
            state.set_vertical_offset((self.vertical_offset - 10) as usize);
        } else {
            state.set_vertical_select(self.vertical_offset as usize);
        }
    }
}
