use std::collections::{HashMap, VecDeque};

use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintColor {
    Default,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl PaintColor {
    pub fn to_ratatui(self) -> Color {
        match self {
            PaintColor::Default => Color::Reset,
            PaintColor::Black => Color::Black,
            PaintColor::Red => Color::Red,
            PaintColor::Green => Color::Green,
            PaintColor::Yellow => Color::Yellow,
            PaintColor::Blue => Color::Blue,
            PaintColor::Magenta => Color::Magenta,
            PaintColor::Cyan => Color::Cyan,
            PaintColor::White => Color::White,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PaintColor::Default => "Default",
            PaintColor::Black => "Black",
            PaintColor::Red => "Red",
            PaintColor::Green => "Green",
            PaintColor::Yellow => "Yellow",
            PaintColor::Blue => "Blue",
            PaintColor::Magenta => "Magenta",
            PaintColor::Cyan => "Cyan",
            PaintColor::White => "White",
        }
    }

    pub fn quick_palette() -> [PaintColor; 8] {
        [
            PaintColor::Black,
            PaintColor::Red,
            PaintColor::Green,
            PaintColor::Yellow,
            PaintColor::Blue,
            PaintColor::Magenta,
            PaintColor::Cyan,
            PaintColor::White,
        ]
    }

    pub fn from_quick_index(index: u8) -> Option<PaintColor> {
        let palette = Self::quick_palette();
        let idx = index.checked_sub(1)? as usize;
        palette.get(idx).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaintCell {
    pub ch: char,
    pub fg: PaintColor,
    pub bg: Option<PaintColor>,
}

impl Default for PaintCell {
    fn default() -> Self {
        Self::blank()
    }
}

impl PaintCell {
    pub fn blank() -> Self {
        Self {
            ch: ' ',
            fg: PaintColor::Default,
            bg: None,
        }
    }

    pub fn new(ch: char, fg: PaintColor) -> Self {
        Self { ch, fg, bg: None }
    }

    pub fn style(self) -> Style {
        let mut style = Style::default().fg(self.fg.to_ratatui());
        if let Some(bg) = self.bg {
            style = style.bg(bg.to_ratatui());
        }
        style
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    width: u16,
    height: u16,
    cells: Vec<PaintCell>,
}

impl Canvas {
    pub fn new(width: u16, height: u16) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            cells: vec![PaintCell::blank(); width as usize * height as usize],
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn in_bounds_i32(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }

    pub fn index(&self, x: u16, y: u16) -> usize {
        y as usize * self.width as usize + x as usize
    }

    pub fn get(&self, x: u16, y: u16) -> PaintCell {
        if x >= self.width || y >= self.height {
            return PaintCell::blank();
        }
        self.cells[self.index(x, y)]
    }

    pub fn get_i32(&self, x: i32, y: i32) -> Option<PaintCell> {
        if !self.in_bounds_i32(x, y) {
            return None;
        }
        Some(self.get(x as u16, y as u16))
    }

    pub fn set(&mut self, x: u16, y: u16, cell: PaintCell) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = self.index(x, y);
        self.cells[idx] = cell;
    }

    pub fn resize_preserve(&mut self, new_width: u16, new_height: u16) {
        let new_width = new_width.max(1);
        let new_height = new_height.max(1);

        if self.width == new_width && self.height == new_height {
            return;
        }

        let mut new_cells = vec![PaintCell::blank(); new_width as usize * new_height as usize];
        let copy_w = self.width.min(new_width);
        let copy_h = self.height.min(new_height);

        for y in 0..copy_h {
            for x in 0..copy_w {
                let old_idx = self.index(x, y);
                let new_idx = y as usize * new_width as usize + x as usize;
                new_cells[new_idx] = self.cells[old_idx];
            }
        }

        self.width = new_width;
        self.height = new_height;
        self.cells = new_cells;
    }
}

#[derive(Debug, Clone)]
pub struct CellChange {
    pub x: u16,
    pub y: u16,
    pub before: PaintCell,
    pub after: PaintCell,
}

#[derive(Debug, Clone, Default)]
pub struct Operation {
    pub changes: Vec<CellChange>,
}

impl Operation {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn apply_before(&self, canvas: &mut Canvas) {
        for change in &self.changes {
            canvas.set(change.x, change.y, change.before);
        }
    }

    pub fn apply_after(&self, canvas: &mut Canvas) {
        for change in &self.changes {
            canvas.set(change.x, change.y, change.after);
        }
    }
}

#[derive(Debug, Default)]
pub struct OperationBuilder {
    changes: HashMap<(u16, u16), CellChange>,
}

impl OperationBuilder {
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    pub fn apply(&mut self, canvas: &mut Canvas, x: i32, y: i32, new_cell: PaintCell) {
        if !canvas.in_bounds_i32(x, y) {
            return;
        }

        let ux = x as u16;
        let uy = y as u16;
        let before = canvas.get(ux, uy);
        if before == new_cell {
            return;
        }

        let key = (ux, uy);
        if let Some(change) = self.changes.get_mut(&key) {
            change.after = new_cell;
        } else {
            self.changes.insert(
                key,
                CellChange {
                    x: ux,
                    y: uy,
                    before,
                    after: new_cell,
                },
            );
        }

        canvas.set(ux, uy, new_cell);
    }

    pub fn into_operation(self) -> Operation {
        let mut changes: Vec<_> = self.changes.into_values().collect();
        changes.sort_by_key(|c| (c.y, c.x));
        Operation { changes }
    }
}

#[derive(Debug)]
pub struct History {
    undo_stack: VecDeque<Operation>,
    redo_stack: Vec<Operation>,
    capacity: usize,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            capacity,
        }
    }

    pub fn push(&mut self, op: Operation) {
        if op.is_empty() {
            return;
        }

        self.undo_stack.push_back(op);
        self.redo_stack.clear();

        while self.undo_stack.len() > self.capacity {
            self.undo_stack.pop_front();
        }
    }

    pub fn undo(&mut self, canvas: &mut Canvas) -> bool {
        let Some(op) = self.undo_stack.pop_back() else {
            return false;
        };

        op.apply_before(canvas);
        self.redo_stack.push(op);
        true
    }

    pub fn redo(&mut self, canvas: &mut Canvas) -> bool {
        let Some(op) = self.redo_stack.pop() else {
            return false;
        };

        op.apply_after(canvas);
        self.undo_stack.push_back(op);
        true
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
