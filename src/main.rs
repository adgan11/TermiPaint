mod canvas;
mod io;
mod tools;
mod ui;

use std::{collections::HashSet, io as stdio, path::PathBuf, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};

use crate::{
    canvas::{Canvas, History, OperationBuilder, PaintCell, PaintColor},
    tools::{
        bresenham_line, brush_points, ellipse_points, flood_fill_points, rectangle_points, Point,
        Tool,
    },
    ui::{PreviewStyle, ToolbarAction, UiState},
};

const UNDO_LIMIT: usize = 100;

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    install_panic_hook();

    let mut stdout = stdio::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let initial_size = terminal.size()?;
    let initial_area = Rect::new(0, 0, initial_size.width, initial_size.height);
    let initial_ui = ui::build_ui_state(initial_area);
    let mut app = App::new(
        initial_ui.canvas_inner.width.max(1),
        initial_ui.canvas_inner.height.max(1),
    );
    app.last_ui = initial_ui;

    let tick_rate = Duration::from_millis(16);

    loop {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        let ui_state = ui::build_ui_state(area);
        app.resize_to_fit(&ui_state);
        app.last_ui = ui_state.clone();

        let preview_points = app.preview_points();
        let render_ctx = ui::RenderContext {
            canvas: &app.canvas,
            current_tool: app.tool,
            brush_char: app.brush_char,
            brush_size: app.brush_size,
            color: app.color,
            filled_shapes: app.filled_shapes,
            hover: app.hover,
            preview_points: &preview_points,
            preview_style: app.preview_style(),
            status: &app.status,
            file_name: app.current_file_name(),
            prompt: app.prompt_view(),
        };

        terminal.draw(|f| ui::render(f, &ui_state, &render_ctx))?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    if app.handle_key(key) {
                        break;
                    }
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Resize(_, _) => {
                    // Handled in next frame by recomputing UI and canvas size.
                }
                Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
            }
        }
    }

    Ok(())
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn install_panic_hook() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        previous_hook(panic_info);
    }));
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut stdout = stdio::stdout();
    let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
}

#[derive(Debug, Clone, Copy)]
struct DrawSpec {
    tool: Tool,
    ch: char,
    color: PaintColor,
    size: u8,
}

enum MouseMode {
    Idle,
    FreeDrag {
        last: Point,
        spec: DrawSpec,
        builder: OperationBuilder,
    },
    ShapeDrag {
        start: Point,
        current: Point,
        spec: DrawSpec,
        tool: Tool,
        filled: bool,
    },
}

enum PromptState {
    None,
    Save(String),
    Load(String),
}

struct App {
    canvas: Canvas,
    tool: Tool,
    brush_char: char,
    brush_size: u8,
    color: PaintColor,
    filled_shapes: bool,
    hover: Option<Point>,
    mouse_mode: MouseMode,
    history: History,
    status: String,
    prompt: PromptState,
    current_file: Option<PathBuf>,
    last_ui: UiState,
}

impl App {
    fn new(canvas_width: u16, canvas_height: u16) -> Self {
        Self {
            canvas: Canvas::new(canvas_width, canvas_height),
            tool: Tool::Pencil,
            brush_char: '#',
            brush_size: 1,
            color: PaintColor::White,
            filled_shapes: false,
            hover: None,
            mouse_mode: MouseMode::Idle,
            history: History::new(UNDO_LIMIT),
            status: "Ready".to_string(),
            prompt: PromptState::None,
            current_file: None,
            last_ui: UiState::default(),
        }
    }

    fn resize_to_fit(&mut self, ui_state: &UiState) {
        let width = ui_state.canvas_inner.width.max(1);
        let height = ui_state.canvas_inner.height.max(1);
        self.canvas.resize_preserve(width, height);
    }

    fn current_file_name(&self) -> Option<&str> {
        self.current_file
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
    }

    fn current_draw_spec(&self) -> DrawSpec {
        DrawSpec {
            tool: self.tool,
            ch: self.brush_char,
            color: self.color,
            size: self.brush_size,
        }
    }

    fn prompt_view(&self) -> Option<ui::PromptView<'_>> {
        match &self.prompt {
            PromptState::Save(input) => Some(ui::PromptView {
                title:
                    "Save file (JSON if .json, otherwise ASCII) - Enter to confirm, Esc to cancel",
                input,
            }),
            PromptState::Load(input) => Some(ui::PromptView {
                title: "Load file (.json or ASCII) - Enter to confirm, Esc to cancel",
                input,
            }),
            PromptState::None => None,
        }
    }

    fn prompt_is_active(&self) -> bool {
        !matches!(self.prompt, PromptState::None)
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }

        if self.prompt_is_active() {
            self.handle_prompt_key(key);
            return false;
        }

        if is_undo_shortcut(key) {
            self.perform_undo();
            return false;
        }

        if is_redo_shortcut(key) {
            self.perform_redo();
            return false;
        }

        if has_shortcut_modifier(key.modifiers) {
            match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.open_save_prompt();
                    return false;
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    self.open_load_prompt();
                    return false;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char(ch) => {
                let ch = ch.to_ascii_lowercase();
                match ch {
                    'q' => return true,
                    'p' => self.tool = Tool::Pencil,
                    'e' => self.tool = Tool::Eraser,
                    'l' => self.tool = Tool::Line,
                    'r' => self.tool = Tool::Rectangle,
                    'c' => self.tool = Tool::Circle,
                    'f' => self.tool = Tool::Fill,
                    'u' => self.perform_undo(),
                    'y' => self.perform_redo(),
                    '[' => {
                        self.brush_size = self.brush_size.saturating_sub(1).max(1);
                    }
                    ']' => {
                        self.brush_size = (self.brush_size + 1).min(3);
                    }
                    't' => {
                        self.filled_shapes = !self.filled_shapes;
                    }
                    'b' => self.cycle_brush_char(true),
                    '0' | 'd' => self.color = PaintColor::Default,
                    '1'..='8' => {
                        let idx = (ch as u8) - b'0';
                        if let Some(color) = PaintColor::from_quick_index(idx) {
                            self.color = color;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Esc => {
                if matches!(self.mouse_mode, MouseMode::ShapeDrag { .. }) {
                    self.mouse_mode = MouseMode::Idle;
                    self.status = "Shape cancelled".to_string();
                }
            }
            _ => {}
        }

        false
    }

    fn perform_undo(&mut self) {
        if self.history.undo(&mut self.canvas) {
            self.status = "Undo".to_string();
        } else {
            self.status = "Nothing to undo".to_string();
        }
    }

    fn perform_redo(&mut self) {
        if self.history.redo(&mut self.canvas) {
            self.status = "Redo".to_string();
        } else {
            self.status = "Nothing to redo".to_string();
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.prompt = PromptState::None;
                self.status = "Prompt cancelled".to_string();
                return;
            }
            KeyCode::Enter => {
                self.commit_prompt();
                return;
            }
            KeyCode::Backspace => {
                if let Some(input) = self.prompt_input_mut() {
                    input.pop();
                }
                return;
            }
            KeyCode::Char(c) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    if let Some(input) = self.prompt_input_mut() {
                        input.push(c);
                    }
                }
            }
            _ => {}
        }
    }

    fn prompt_input_mut(&mut self) -> Option<&mut String> {
        match &mut self.prompt {
            PromptState::Save(input) | PromptState::Load(input) => Some(input),
            PromptState::None => None,
        }
    }

    fn open_save_prompt(&mut self) {
        let default_name = self
            .current_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "canvas.json".to_string());
        self.prompt = PromptState::Save(default_name);
    }

    fn open_load_prompt(&mut self) {
        let default_name = self
            .current_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "canvas.json".to_string());
        self.prompt = PromptState::Load(default_name);
    }

    fn commit_prompt(&mut self) {
        let prompt = std::mem::replace(&mut self.prompt, PromptState::None);
        match prompt {
            PromptState::Save(input) => {
                let path = io::parse_path(&input, "canvas.json");
                match io::save_canvas(&path, &self.canvas) {
                    Ok(()) => {
                        self.current_file = Some(path.clone());
                        self.status = format!("Saved {}", path.display());
                    }
                    Err(err) => {
                        self.status = format!("Save failed: {err}");
                    }
                }
            }
            PromptState::Load(input) => {
                let path = io::parse_path(&input, "canvas.json");
                match io::load_canvas(&path) {
                    Ok(mut loaded) => {
                        let width = self.canvas.width();
                        let height = self.canvas.height();
                        loaded.resize_preserve(width, height);
                        self.canvas = loaded;
                        self.history.clear();
                        self.current_file = Some(path.clone());
                        self.status = format!("Loaded {}", path.display());
                    }
                    Err(err) => {
                        self.status = format!("Load failed: {err}");
                    }
                }
            }
            PromptState::None => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.prompt_is_active() {
            return;
        }

        let column = mouse.column;
        let row = mouse.row;
        self.hover = ui::mouse_to_canvas(&self.last_ui, column, row);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(action) = ui::toolbar_action_at(&self.last_ui, column, row) {
                    self.apply_toolbar_action(action);
                    return;
                }

                if let Some(point) = self.hover {
                    self.begin_left_draw(point);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(point) = self.hover {
                    self.drag_left_draw(point);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.finish_left_draw(self.hover);
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(point) = self.hover {
                    self.sample_cell(point);
                }
            }
            MouseEventKind::ScrollUp => self.cycle_color(true),
            MouseEventKind::ScrollDown => self.cycle_color(false),
            MouseEventKind::Moved => {}
            _ => {}
        }
    }

    fn apply_toolbar_action(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::SelectTool(tool) => {
                self.tool = tool;
                self.status = format!("Tool: {}", tool.name());
            }
            ToolbarAction::SelectBrushChar(ch) => {
                self.brush_char = ch;
                self.status = format!("Brush char: {}", printable_char(ch));
            }
            ToolbarAction::SelectColor(color) => {
                self.color = color;
                self.status = format!("Color: {}", color.name());
            }
            ToolbarAction::ToggleFilledShapes => {
                self.filled_shapes = !self.filled_shapes;
                self.status = if self.filled_shapes {
                    "Rectangle fill enabled".to_string()
                } else {
                    "Rectangle fill disabled".to_string()
                };
            }
        }
    }

    fn begin_left_draw(&mut self, point: Point) {
        let spec = self.current_draw_spec();

        match self.tool {
            Tool::Pencil | Tool::Eraser => {
                let mut builder = OperationBuilder::new();
                apply_point_with_spec(&mut self.canvas, &mut builder, point, spec);
                self.mouse_mode = MouseMode::FreeDrag {
                    last: point,
                    spec,
                    builder,
                };
            }
            Tool::Line | Tool::Rectangle | Tool::Circle => {
                self.mouse_mode = MouseMode::ShapeDrag {
                    start: point,
                    current: point,
                    spec,
                    tool: self.tool,
                    filled: self.filled_shapes,
                };
            }
            Tool::Fill => {
                let mut builder = OperationBuilder::new();
                self.apply_fill(point, spec, &mut builder);
                self.commit_builder(builder);
            }
        }
    }

    fn drag_left_draw(&mut self, point: Point) {
        let canvas = &mut self.canvas;

        match &mut self.mouse_mode {
            MouseMode::Idle => {}
            MouseMode::FreeDrag {
                last,
                spec,
                builder,
            } => {
                for p in bresenham_line(*last, point) {
                    apply_point_with_spec(canvas, builder, p, *spec);
                }
                *last = point;
            }
            MouseMode::ShapeDrag { current, .. } => {
                *current = point;
            }
        }
    }

    fn finish_left_draw(&mut self, maybe_end: Option<Point>) {
        let mode = std::mem::replace(&mut self.mouse_mode, MouseMode::Idle);

        match mode {
            MouseMode::Idle => {}
            MouseMode::FreeDrag { builder, .. } => {
                self.commit_builder(builder);
            }
            MouseMode::ShapeDrag {
                start,
                current,
                spec,
                tool,
                filled,
            } => {
                let end = maybe_end.unwrap_or(current);
                let base_points = shape_points(tool, start, end, filled);
                let mut builder = OperationBuilder::new();

                for point in base_points {
                    apply_point_with_spec(&mut self.canvas, &mut builder, point, spec);
                }

                self.commit_builder(builder);
            }
        }
    }

    fn apply_fill(&mut self, point: Point, spec: DrawSpec, builder: &mut OperationBuilder) {
        let Some(target) = self.canvas.get_i32(point.x, point.y) else {
            return;
        };

        let replacement = if spec.tool == Tool::Eraser {
            PaintCell::blank()
        } else {
            PaintCell::new(spec.ch, spec.color)
        };

        let points = flood_fill_points(&self.canvas, point, target, replacement);
        for p in points {
            builder.apply(&mut self.canvas, p.x, p.y, replacement);
        }
    }

    fn commit_builder(&mut self, builder: OperationBuilder) {
        let operation = builder.into_operation();
        if !operation.is_empty() {
            self.history.push(operation);
        }
    }

    fn sample_cell(&mut self, point: Point) {
        let Some(cell) = self.canvas.get_i32(point.x, point.y) else {
            return;
        };

        if cell.ch != ' ' {
            self.brush_char = cell.ch;
        }
        self.color = cell.fg;
        self.status = format!(
            "Sampled '{}' / {}",
            printable_char(self.brush_char),
            self.color.name()
        );
    }

    fn preview_points(&self) -> Vec<Point> {
        let MouseMode::ShapeDrag {
            start,
            current,
            spec,
            tool,
            filled,
        } = self.mouse_mode
        else {
            return Vec::new();
        };

        let base_points = shape_points(tool, start, current, filled);
        if spec.size <= 1 {
            return base_points;
        }

        let mut set = HashSet::new();
        let mut out = Vec::new();
        for point in base_points {
            for brush in brush_points(point, spec.size) {
                if set.insert((brush.x, brush.y)) {
                    out.push(brush);
                }
            }
        }
        out
    }

    fn preview_style(&self) -> Option<PreviewStyle> {
        match self.mouse_mode {
            MouseMode::ShapeDrag { spec, .. } => Some(PreviewStyle {
                ch: spec.ch,
                fg: spec.color,
                erase: spec.tool == Tool::Eraser,
            }),
            _ => None,
        }
    }

    fn cycle_color(&mut self, forward: bool) {
        let palette = PaintColor::quick_palette();

        let mut idx = palette.iter().position(|c| *c == self.color).unwrap_or(0);
        if forward {
            idx = (idx + 1) % palette.len();
        } else if idx == 0 {
            idx = palette.len() - 1;
        } else {
            idx -= 1;
        }

        self.color = palette[idx];
    }

    fn cycle_brush_char(&mut self, forward: bool) {
        let choices = ui::BRUSH_CHOICES;
        let mut idx = choices
            .iter()
            .position(|ch| *ch == self.brush_char)
            .unwrap_or(0);

        if forward {
            idx = (idx + 1) % choices.len();
        } else if idx == 0 {
            idx = choices.len() - 1;
        } else {
            idx -= 1;
        }

        self.brush_char = choices[idx];
    }
}

fn has_shortcut_modifier(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER)
}

fn is_undo_shortcut(key: KeyEvent) -> bool {
    if !has_shortcut_modifier(key.modifiers) || key.modifiers.contains(KeyModifiers::SHIFT) {
        return false;
    }

    match key.code {
        KeyCode::Char(ch) => ch.eq_ignore_ascii_case(&'z'),
        _ => false,
    }
}

fn is_redo_shortcut(key: KeyEvent) -> bool {
    if !has_shortcut_modifier(key.modifiers) {
        return false;
    }

    match key.code {
        KeyCode::Char(ch) => {
            let lower = ch.to_ascii_lowercase();
            lower == 'y' || (lower == 'z' && key.modifiers.contains(KeyModifiers::SHIFT))
        }
        _ => false,
    }
}

fn apply_point_with_spec(
    canvas: &mut Canvas,
    builder: &mut OperationBuilder,
    point: Point,
    spec: DrawSpec,
) {
    let draw_cell = if spec.tool == Tool::Eraser {
        PaintCell::blank()
    } else {
        PaintCell::new(spec.ch, spec.color)
    };

    for p in brush_points(point, spec.size) {
        builder.apply(canvas, p.x, p.y, draw_cell);
    }
}

fn shape_points(tool: Tool, start: Point, end: Point, filled: bool) -> Vec<Point> {
    match tool {
        Tool::Line => bresenham_line(start, end),
        Tool::Rectangle => rectangle_points(start, end, filled),
        Tool::Circle => ellipse_points(start, end),
        _ => Vec::new(),
    }
}

fn printable_char(ch: char) -> String {
    if ch == ' ' {
        "␠".to_string()
    } else {
        ch.to_string()
    }
}
