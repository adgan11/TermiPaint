use std::collections::{HashSet, VecDeque};

use crate::canvas::{Canvas, PaintCell};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Pencil,
    Eraser,
    Line,
    Rectangle,
    Circle,
    Fill,
}

impl Tool {
    pub const fn all() -> [Tool; 6] {
        [
            Tool::Pencil,
            Tool::Eraser,
            Tool::Line,
            Tool::Rectangle,
            Tool::Circle,
            Tool::Fill,
        ]
    }

    pub const fn name(self) -> &'static str {
        match self {
            Tool::Pencil => "Pencil",
            Tool::Eraser => "Eraser",
            Tool::Line => "Line",
            Tool::Rectangle => "Rectangle",
            Tool::Circle => "Circle",
            Tool::Fill => "Fill",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Tool::Pencil => "Pencil(P)",
            Tool::Eraser => "Eraser(E)",
            Tool::Line => "Line(L)",
            Tool::Rectangle => "Rect(R)",
            Tool::Circle => "Circle(C)",
            Tool::Fill => "Fill(F)",
        }
    }
}

pub fn brush_points(center: Point, size: u8) -> Vec<Point> {
    let radius = size.saturating_sub(1) as i32;
    let mut points = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            points.push(Point::new(center.x + dx, center.y + dy));
        }
    }
    points
}

pub fn bresenham_line(start: Point, end: Point) -> Vec<Point> {
    let mut points = Vec::new();

    let mut x0 = start.x;
    let mut y0 = start.y;
    let x1 = end.x;
    let y1 = end.y;

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        points.push(Point::new(x0, y0));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            if x0 == x1 {
                // no-op
            } else {
                err += dy;
                x0 += sx;
            }
        }
        if e2 <= dx {
            if y0 == y1 {
                // no-op
            } else {
                err += dx;
                y0 += sy;
            }
        }
    }

    points
}

pub fn rectangle_points(start: Point, end: Point, filled: bool) -> Vec<Point> {
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);

    let mut points = Vec::new();

    if filled {
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                points.push(Point::new(x, y));
            }
        }
        return points;
    }

    for x in min_x..=max_x {
        points.push(Point::new(x, min_y));
        points.push(Point::new(x, max_y));
    }
    for y in min_y..=max_y {
        points.push(Point::new(min_x, y));
        points.push(Point::new(max_x, y));
    }

    dedup_points(points)
}

pub fn ellipse_points(start: Point, end: Point) -> Vec<Point> {
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);

    let rx = ((max_x - min_x) / 2).abs();
    let ry = ((max_y - min_y) / 2).abs();
    let cx = min_x + rx;
    let cy = min_y + ry;

    if rx == 0 && ry == 0 {
        return vec![Point::new(cx, cy)];
    }

    if rx == 0 {
        return (min_y..=max_y).map(|y| Point::new(cx, y)).collect();
    }

    if ry == 0 {
        return (min_x..=max_x).map(|x| Point::new(x, cy)).collect();
    }

    let rx = rx as i64;
    let ry = ry as i64;
    let cx = cx as i64;
    let cy = cy as i64;

    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let two_rx2 = 2 * rx2;
    let two_ry2 = 2 * ry2;

    let mut x: i64 = 0;
    let mut y: i64 = ry;

    let mut px: i64 = 0;
    let mut py: i64 = two_rx2 * y;

    let mut points: Vec<Point> = Vec::new();

    let mut p = ry2 - (rx2 * ry) + (rx2 / 4);

    while px < py {
        plot_ellipse_points(&mut points, cx, cy, x, y);

        x += 1;
        px += two_ry2;

        if p < 0 {
            p += ry2 + px;
        } else {
            y -= 1;
            py -= two_rx2;
            p += ry2 + px - py;
        }
    }

    let mut p2 = ry2 * (x * x + x) + (ry2 / 4) + rx2 * (y - 1) * (y - 1) - rx2 * ry2;

    while y >= 0 {
        plot_ellipse_points(&mut points, cx, cy, x, y);

        y -= 1;
        py -= two_rx2;

        if p2 > 0 {
            p2 += rx2 - py;
        } else {
            x += 1;
            px += two_ry2;
            p2 += rx2 - py + px;
        }
    }

    dedup_points(points)
}

pub fn flood_fill_points(
    canvas: &Canvas,
    start: Point,
    target: PaintCell,
    replacement: PaintCell,
) -> Vec<Point> {
    if target == replacement || !canvas.in_bounds_i32(start.x, start.y) {
        return Vec::new();
    }

    let width = canvas.width() as usize;
    let height = canvas.height() as usize;
    let mut visited = vec![false; width * height];
    let mut queue = VecDeque::new();
    let mut out = Vec::new();

    queue.push_back(start);

    while let Some(p) = queue.pop_front() {
        if !canvas.in_bounds_i32(p.x, p.y) {
            continue;
        }

        let x = p.x as usize;
        let y = p.y as usize;
        let idx = y * width + x;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        let current = canvas.get(x as u16, y as u16);
        if current != target {
            continue;
        }

        out.push(Point::new(p.x, p.y));

        queue.push_back(Point::new(p.x + 1, p.y));
        queue.push_back(Point::new(p.x - 1, p.y));
        queue.push_back(Point::new(p.x, p.y + 1));
        queue.push_back(Point::new(p.x, p.y - 1));
    }

    out
}

fn plot_ellipse_points(points: &mut Vec<Point>, cx: i64, cy: i64, x: i64, y: i64) {
    points.push(Point::new((cx + x) as i32, (cy + y) as i32));
    points.push(Point::new((cx - x) as i32, (cy + y) as i32));
    points.push(Point::new((cx + x) as i32, (cy - y) as i32));
    points.push(Point::new((cx - x) as i32, (cy - y) as i32));
}

fn dedup_points(points: Vec<Point>) -> Vec<Point> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(points.len());
    for p in points {
        if seen.insert((p.x, p.y)) {
            out.push(p);
        }
    }
    out
}
