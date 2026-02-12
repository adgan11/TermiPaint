use std::collections::HashSet;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    canvas::{Canvas, PaintCell, PaintColor},
    tools::{Point, Tool},
};

pub const TOOLBAR_HEIGHT: u16 = 4;
pub const BRUSH_CHOICES: [char; 7] = ['#', '@', '.', '*', '+', '%', ' '];

#[derive(Debug, Clone, Copy)]
pub enum ToolbarAction {
    SelectTool(Tool),
    SelectBrushChar(char),
    SelectColor(PaintColor),
    ToggleFilledShapes,
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub terminal: Rect,
    pub toolbar_outer: Rect,
    pub toolbar_inner: Rect,
    pub tool_row: Rect,
    pub bottom_row: Rect,
    pub brush_area: Rect,
    pub color_area: Rect,
    pub status_area: Rect,
    pub canvas_outer: Rect,
    pub canvas_inner: Rect,
    pub tool_hits: Vec<(Rect, Tool)>,
    pub brush_hits: Vec<(Rect, char)>,
    pub color_hits: Vec<(Rect, PaintColor)>,
    pub fill_toggle_hit: Option<Rect>,
}

impl Default for UiState {
    fn default() -> Self {
        let rect = Rect::new(0, 0, 0, 0);
        Self {
            terminal: rect,
            toolbar_outer: rect,
            toolbar_inner: rect,
            tool_row: rect,
            bottom_row: rect,
            brush_area: rect,
            color_area: rect,
            status_area: rect,
            canvas_outer: rect,
            canvas_inner: rect,
            tool_hits: Vec::new(),
            brush_hits: Vec::new(),
            color_hits: Vec::new(),
            fill_toggle_hit: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct PreviewStyle {
    pub ch: char,
    pub fg: PaintColor,
    pub erase: bool,
}

pub struct PromptView<'a> {
    pub title: &'a str,
    pub input: &'a str,
}

pub struct RenderContext<'a> {
    pub canvas: &'a Canvas,
    pub current_tool: Tool,
    pub brush_char: char,
    pub brush_size: u8,
    pub color: PaintColor,
    pub filled_shapes: bool,
    pub hover: Option<Point>,
    pub preview_points: &'a [Point],
    pub preview_style: Option<PreviewStyle>,
    pub status: &'a str,
    pub file_name: Option<&'a str>,
    pub prompt: Option<PromptView<'a>>,
}

pub fn build_ui_state(area: Rect) -> UiState {
    let mut ui = UiState {
        terminal: area,
        ..UiState::default()
    };

    let toolbar_height = if area.height >= 4 {
        TOOLBAR_HEIGHT.min(area.height.saturating_sub(1))
    } else {
        area.height.saturating_sub(1).max(1)
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(toolbar_height), Constraint::Min(1)])
        .split(area);

    ui.toolbar_outer = chunks[0];
    ui.canvas_outer = chunks[1];
    ui.toolbar_inner = inner_with_borders(ui.toolbar_outer);
    ui.canvas_inner = inner_with_borders(ui.canvas_outer);

    let row_constraints = if ui.toolbar_inner.height >= 2 {
        vec![Constraint::Length(1), Constraint::Length(1)]
    } else {
        vec![Constraint::Length(1)]
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(ui.toolbar_inner);

    ui.tool_row = rows[0];
    ui.bottom_row = if rows.len() > 1 { rows[1] } else { rows[0] };

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(30),
            Constraint::Length(34),
            Constraint::Min(10),
        ])
        .split(ui.bottom_row);

    ui.brush_area = bottom_chunks[0];
    ui.color_area = bottom_chunks[1];
    ui.status_area = bottom_chunks[2];

    ui.tool_hits = build_tool_hits(ui.tool_row);
    ui.fill_toggle_hit = build_fill_toggle_hit(ui.tool_row);
    ui.brush_hits = build_brush_hits(ui.brush_area);
    ui.color_hits = build_color_hits(ui.color_area);

    ui
}

pub fn toolbar_action_at(ui: &UiState, column: u16, row: u16) -> Option<ToolbarAction> {
    for (rect, tool) in &ui.tool_hits {
        if rect_contains(*rect, column, row) {
            return Some(ToolbarAction::SelectTool(*tool));
        }
    }

    if let Some(rect) = ui.fill_toggle_hit {
        if rect_contains(rect, column, row) {
            return Some(ToolbarAction::ToggleFilledShapes);
        }
    }

    for (rect, brush) in &ui.brush_hits {
        if rect_contains(*rect, column, row) {
            return Some(ToolbarAction::SelectBrushChar(*brush));
        }
    }

    for (rect, color) in &ui.color_hits {
        if rect_contains(*rect, column, row) {
            return Some(ToolbarAction::SelectColor(*color));
        }
    }

    None
}

pub fn mouse_to_canvas(ui: &UiState, column: u16, row: u16) -> Option<Point> {
    if !rect_contains(ui.canvas_inner, column, row) {
        return None;
    }

    Some(Point {
        x: (column - ui.canvas_inner.x) as i32,
        y: (row - ui.canvas_inner.y) as i32,
    })
}

pub fn render(f: &mut Frame, ui: &UiState, ctx: &RenderContext<'_>) {
    let toolbar_block = Block::default().title(" TermiPaint ").borders(Borders::ALL);
    f.render_widget(toolbar_block, ui.toolbar_outer);

    render_tool_row(f, ui, ctx);
    render_brush_row(f, ui, ctx);
    render_color_row(f, ui, ctx);
    render_status(f, ui, ctx);

    render_canvas(f, ui, ctx);

    if let Some(prompt) = &ctx.prompt {
        render_prompt(f, ui.terminal, prompt);
    }
}

fn render_tool_row(f: &mut Frame, ui: &UiState, ctx: &RenderContext<'_>) {
    let mut spans = Vec::new();

    for tool in Tool::all() {
        let label = tool_button_label(tool);
        let mut style = Style::default();
        if tool == ctx.current_tool {
            style = style.add_modifier(Modifier::BOLD | Modifier::REVERSED);
        }
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }

    let fill_label = fill_toggle_label(ctx.filled_shapes);
    let fill_style = if ctx.filled_shapes {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    spans.push(Span::styled(fill_label, fill_style));

    let line = Line::from(spans);
    f.render_widget(Paragraph::new(line), ui.tool_row);
}

fn render_brush_row(f: &mut Frame, ui: &UiState, ctx: &RenderContext<'_>) {
    let mut spans = vec![Span::styled(
        "Brush ",
        Style::default().add_modifier(Modifier::BOLD),
    )];

    for ch in BRUSH_CHOICES {
        let label = brush_button_label(ch);
        let mut style = Style::default();
        if ch == ctx.brush_char {
            style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
        }
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }

    spans.push(Span::raw(format!("Size:{}", ctx.brush_size)));

    f.render_widget(Paragraph::new(Line::from(spans)), ui.brush_area);
}

fn render_color_row(f: &mut Frame, ui: &UiState, ctx: &RenderContext<'_>) {
    let mut spans = vec![Span::styled(
        "Color ",
        Style::default().add_modifier(Modifier::BOLD),
    )];

    let default_style = if ctx.color == PaintColor::Default {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    spans.push(Span::styled(color_button_label_default(), default_style));
    spans.push(Span::raw(" "));

    for (idx, color) in PaintColor::quick_palette().iter().copied().enumerate() {
        let mut style = Style::default().fg(color.to_ratatui());
        if color == ctx.color {
            style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
        }
        spans.push(Span::styled(color_button_label_index(idx + 1), style));
        spans.push(Span::raw(" "));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), ui.color_area);
}

fn render_status(f: &mut Frame, ui: &UiState, ctx: &RenderContext<'_>) {
    let position = ctx
        .hover
        .map(|p| format!("{},{}", p.x, p.y))
        .unwrap_or_else(|| "-".to_string());

    let file_part = ctx
        .file_name
        .map(|f| format!("File:{} ", f))
        .unwrap_or_default();

    let status = format!(
        "{}Tool:{} Brush:'{}' Size:{} Color:{} Pos:{} | q quit u/y or Ctrl/Cmd+Z undo, Ctrl+Y/Cmd+Shift+Z redo, Ctrl+S/Ctrl+O",
        file_part,
        ctx.current_tool.name(),
        printable_char(ctx.brush_char),
        ctx.brush_size,
        ctx.color.name(),
        position
    );

    let full = if ctx.status.is_empty() {
        status
    } else {
        format!("{} | {}", status, ctx.status)
    };

    f.render_widget(Paragraph::new(full), ui.status_area);
}

fn render_canvas(f: &mut Frame, ui: &UiState, ctx: &RenderContext<'_>) {
    let canvas_title = " Canvas ";
    let canvas_block = Block::default().title(canvas_title).borders(Borders::ALL);
    f.render_widget(canvas_block, ui.canvas_outer);

    let mut preview_set = HashSet::new();
    for p in ctx.preview_points {
        if p.x >= 0
            && p.y >= 0
            && p.x < ctx.canvas.width() as i32
            && p.y < ctx.canvas.height() as i32
        {
            preview_set.insert((p.x as u16, p.y as u16));
        }
    }

    let mut lines = Vec::with_capacity(ctx.canvas.height() as usize);

    for y in 0..ctx.canvas.height() {
        let mut spans = Vec::with_capacity(ctx.canvas.width() as usize);

        for x in 0..ctx.canvas.width() {
            let mut cell = ctx.canvas.get(x, y);
            let is_preview = preview_set.contains(&(x, y));

            if is_preview {
                if let Some(preview_style) = ctx.preview_style {
                    if preview_style.erase {
                        cell = PaintCell::blank();
                    } else {
                        cell = PaintCell::new(preview_style.ch, preview_style.fg);
                    }
                }
            }

            let mut style = cell.style();
            if is_preview {
                style = style.add_modifier(Modifier::UNDERLINED);
            }

            if let Some(hover) = ctx.hover {
                if hover.x == x as i32 && hover.y == y as i32 {
                    style = style.add_modifier(Modifier::REVERSED);
                }
            }

            spans.push(Span::styled(cell.ch.to_string(), style));
        }

        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), ui.canvas_inner);
}

fn render_prompt(f: &mut Frame, area: Rect, prompt: &PromptView<'_>) {
    let width = area.width.min(70).max(20);
    let popup = centered_rect(width, 5, area);

    f.render_widget(Clear, popup);

    let block = Block::default().title(prompt.title).borders(Borders::ALL);
    let inner = inner_with_borders(popup);
    f.render_widget(block, popup);

    let help = Line::from(vec![
        Span::raw("> "),
        Span::styled(prompt.input, Style::default().add_modifier(Modifier::BOLD)),
    ]);

    f.render_widget(Paragraph::new(help), inner);
}

fn build_tool_hits(area: Rect) -> Vec<(Rect, Tool)> {
    let mut hits = Vec::new();
    let mut x = area.x;
    let y = area.y;
    let right = area.x.saturating_add(area.width);

    for tool in Tool::all() {
        let label = tool_button_label(tool);
        let w = label.chars().count() as u16;
        if x.saturating_add(w) > right {
            break;
        }
        hits.push((Rect::new(x, y, w, 1), tool));
        x = x.saturating_add(w + 1);
    }

    hits
}

fn build_fill_toggle_hit(area: Rect) -> Option<Rect> {
    let mut x = area.x;
    for tool in Tool::all() {
        let w = tool_button_label(tool).chars().count() as u16;
        x = x.saturating_add(w + 1);
    }

    let label = fill_toggle_label(false);
    let w = label.chars().count() as u16;
    let right = area.x.saturating_add(area.width);

    if x.saturating_add(w) > right {
        return None;
    }

    Some(Rect::new(x, area.y, w, 1))
}

fn build_brush_hits(area: Rect) -> Vec<(Rect, char)> {
    let mut hits = Vec::new();
    let mut x = area.x.saturating_add("Brush ".chars().count() as u16);
    let y = area.y;
    let right = area.x.saturating_add(area.width);

    for ch in BRUSH_CHOICES {
        let label = brush_button_label(ch);
        let w = label.chars().count() as u16;
        if x.saturating_add(w) > right {
            break;
        }
        hits.push((Rect::new(x, y, w, 1), ch));
        x = x.saturating_add(w + 1);
    }

    hits
}

fn build_color_hits(area: Rect) -> Vec<(Rect, PaintColor)> {
    let mut hits = Vec::new();
    let mut x = area.x.saturating_add("Color ".chars().count() as u16);
    let y = area.y;
    let right = area.x.saturating_add(area.width);

    let default_label = color_button_label_default();
    let default_w = default_label.chars().count() as u16;
    if x.saturating_add(default_w) <= right {
        hits.push((Rect::new(x, y, default_w, 1), PaintColor::Default));
        x = x.saturating_add(default_w + 1);
    }

    for (idx, color) in PaintColor::quick_palette().iter().copied().enumerate() {
        let label = color_button_label_index(idx + 1);
        let w = label.chars().count() as u16;
        if x.saturating_add(w) > right {
            break;
        }
        hits.push((Rect::new(x, y, w, 1), color));
        x = x.saturating_add(w + 1);
    }

    hits
}

fn tool_button_label(tool: Tool) -> String {
    format!("[{}]", tool.short_label())
}

fn fill_toggle_label(filled: bool) -> String {
    if filled {
        "[Filled:On(T)]".to_string()
    } else {
        "[Filled:Off(T)]".to_string()
    }
}

fn brush_button_label(ch: char) -> String {
    format!("[{}]", printable_char(ch))
}

fn color_button_label_default() -> String {
    "[D]".to_string()
}

fn color_button_label_index(index: usize) -> String {
    format!("[{}]", index)
}

fn printable_char(ch: char) -> String {
    if ch == ' ' {
        "␠".to_string()
    } else {
        ch.to_string()
    }
}

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    let right = rect.x.saturating_add(rect.width);
    let bottom = rect.y.saturating_add(rect.height);
    x >= rect.x && x < right && y >= rect.y && y < bottom
}

fn inner_with_borders(rect: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width.saturating_sub(2)).max(3);
    let h = height.min(area.height.saturating_sub(2)).max(3);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}
