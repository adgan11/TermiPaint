use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::canvas::{Canvas, PaintCell, PaintColor};

pub fn save_canvas(path: &Path, canvas: &Canvas) -> Result<()> {
    match extension_lower(path).as_deref() {
        Some("json") => save_json(path, canvas),
        _ => save_ascii(path, canvas),
    }
}

pub fn load_canvas(path: &Path) -> Result<Canvas> {
    match extension_lower(path).as_deref() {
        Some("json") => load_json(path),
        _ => load_ascii(path),
    }
}

pub fn parse_path(input: &str, fallback: &str) -> PathBuf {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return PathBuf::from(fallback);
    }
    PathBuf::from(trimmed)
}

fn save_json(path: &Path, canvas: &Canvas) -> Result<()> {
    let text =
        serde_json::to_string_pretty(canvas).context("failed to serialize canvas to JSON")?;
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

fn load_json(path: &Path) -> Result<Canvas> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON file {}", path.display()))?;
    let canvas = serde_json::from_str::<Canvas>(&text)
        .with_context(|| format!("failed to parse JSON file {}", path.display()))?;
    Ok(canvas)
}

fn save_ascii(path: &Path, canvas: &Canvas) -> Result<()> {
    let mut out = String::new();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            out.push(canvas.get(x, y).ch);
        }
        if y + 1 < canvas.height() {
            out.push('\n');
        }
    }
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

fn load_ascii(path: &Path) -> Result<Canvas> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read ASCII file {}", path.display()))?;

    let lines: Vec<&str> = if text.is_empty() {
        vec![""]
    } else {
        text.lines().collect()
    };

    let height = lines.len().max(1) as u16;
    let width = lines
        .iter()
        .map(|line| line.chars().count() as u16)
        .max()
        .unwrap_or(1)
        .max(1);

    let mut canvas = Canvas::new(width, height);

    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            canvas.set(
                x as u16,
                y as u16,
                PaintCell {
                    ch,
                    fg: PaintColor::Default,
                    bg: None,
                },
            );
        }
    }

    Ok(canvas)
}

fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
}
