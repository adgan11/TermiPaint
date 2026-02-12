# TermiPaint

![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![TUI](https://img.shields.io/badge/TUI-ratatui-blue)
![Terminal](https://img.shields.io/badge/Terminal-crossterm-purple)
![Unsafe](https://img.shields.io/badge/unsafe-none-success)

**TermiPaint** is a fast, mouse-driven terminal paint application built in Rust.
It brings core “MS Paint”-style workflows to a TUI: toolbar-based tools, click+drag drawing, shape previews, flood fill, undo/redo, and save/load.

---

## Table of Contents

- [Features](#features)
- [Tech Stack](#tech-stack)
- [Requirements](#requirements)
- [Quick Start](#quick-start)
- [Controls](#controls)
  - [Keyboard Shortcuts](#keyboard-shortcuts)
  - [Mouse Controls](#mouse-controls)
- [Saving and Loading](#saving-and-loading)
- [Project Architecture](#project-architecture)
- [Development](#development)
- [Troubleshooting](#troubleshooting)
- [Roadmap](#roadmap)

---

## Features

### Drawing & Tools
- **Pencil** (continuous freehand drawing while dragging)
- **Eraser**
- **Line** (Bresenham)
- **Rectangle** (outline + optional fill)
- **Circle/Ellipse** (outline)
- **Fill** (4-way flood fill)

### Canvas Model
- Cell-based canvas (`char + fg color + optional bg`)
- Canvas border and top toolbar layout
- Shape **preview while dragging** before commit
- Canvas preserves existing content on terminal resize (clipped/expanded)

### UX & Reliability
- Mouse support (click, drag, right-click sample, scroll color cycle)
- Undo/redo with operation batching (per stroke/shape/fill)
- History limit (last **100** operations)
- Panic-safe terminal restoration
- No unsafe Rust

---

## Tech Stack

- **Rust (stable)**
- **ratatui** for rendering/layout
- **crossterm** for terminal backend, keyboard, and mouse events
- **serde + serde_json** for file persistence

---

## Requirements

- Rust stable toolchain
- A terminal with mouse event support:
  - macOS: iTerm2, WezTerm, Kitty, Terminal.app (basic)
  - Linux: GNOME Terminal, Kitty, WezTerm, Alacritty
  - Windows: Windows Terminal

> For best behavior, run directly in a real terminal (not in a non-interactive output pane).

---

## Quick Start

```bash
git clone https://github.com/adgan11/TermiPaint.git
cd TermiPaint
cargo run --release
```

Alternative (local install):

```bash
cargo install --path .
termipaint
```

---

## Controls

### Keyboard Shortcuts

| Action | Shortcut |
|---|---|
| Quit | `q` |
| Pencil | `p` |
| Eraser | `e` |
| Line | `l` |
| Rectangle | `r` |
| Circle/Ellipse | `c` |
| Fill (Bucket) | `f` |
| Undo | `u` or `Ctrl+Z` / `Cmd+Z`* |
| Redo | `y` or `Ctrl+Y` / `Cmd+Shift+Z`* |
| Brush size | `[` (down), `]` (up) |
| Cycle brush character | `b` |
| Toggle filled rectangles | `t` |
| Color quick select | `1..8` |
| Set color to default | `0` or `d` |
| Save | `Ctrl+S` |
| Load | `Ctrl+O` |
| Cancel active shape preview | `Esc` |

\* `Cmd` combinations depend on whether your terminal forwards those key events.

### Mouse Controls

| Action | Mouse Input |
|---|---|
| Select tool/color/brush | Left click toolbar |
| Draw (Pencil/Eraser) | Left click + drag on canvas |
| Place shape (Line/Rect/Circle) | Left click + drag + release |
| Fill | Left click canvas with Fill tool |
| Sample char/color from canvas | Right click |
| Cycle colors | Scroll up/down |

---

## Saving and Loading

TermiPaint supports two formats based on extension:

### 1) JSON (`.json`) — full fidelity
Saves and loads canvas width/height and per-cell character/color data.

### 2) Plain text (any non-`.json` extension)
Saves ASCII characters only (color information is ignored).

Save/load uses an in-app prompt (`Ctrl+S` / `Ctrl+O`) where you type the file path.

---

## Project Architecture

```text
src/
├── main.rs     # app lifecycle, event loop, input handling, tool state machine
├── ui.rs       # ratatui rendering, toolbar layout, hit-testing, prompt modal
├── canvas.rs   # canvas model, paint cells, operation batching, undo/redo history
├── tools.rs    # drawing algorithms (line, rectangle, ellipse, flood fill)
└── io.rs       # JSON/ASCII save+load utilities
```

Design highlights:
- **OperationBuilder** batches edits so each stroke/shape/fill is one undo step.
- Shape tools maintain a preview state and commit only on mouse release.
- Terminal is restored cleanly on normal exit and on panic.

---

## Development

Format + check:

```bash
cargo fmt
cargo check
```

Run in debug:

```bash
cargo run
```

Run in release:

```bash
cargo run --release
```

---

## Troubleshooting

### Mouse input not working
- Ensure your terminal supports mouse reporting.
- Avoid non-interactive panes that do not allocate a proper TTY.

### App exits with terminal/device errors
- Run directly from a local interactive terminal window.
- Verify no other process is taking terminal control.

### Cmd shortcuts not triggering on macOS
- Some terminals do not forward `Cmd` combinations.
- Use `u`/`y` or `Ctrl+Z`/`Ctrl+Y` fallback shortcuts.

---

## Roadmap

- Filled ellipse mode
- Optional text tool
- Better toolbar behavior on very narrow terminals
- Polished packaging (Homebrew tap, Scoop/WinGet, release automation)

---

If you find bugs or want features, open an issue with:
- terminal emulator + OS
- reproduction steps
- expected vs actual behavior
