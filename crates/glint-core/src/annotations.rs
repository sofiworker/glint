use crate::Rect;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Transform};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redaction {
    pub bounds: Rect,
    pub color: [u8; 4],
}

/// A source-dependent filter. Blocks align with the selection-local 16px grid.
/// Averages the already composited pixels, never replacing prior edits with raw source pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixelation {
    pub bounds: Rect,
}
impl Pixelation {
    /// Cells wholly inside both rectangles have the same input and output. Refresh only
    /// the changed perimeter, including partial cells whose averaging area has changed.
    pub fn changed_regions(before: Rect, after: Rect) -> Vec<Rect> {
        if before == after {
            return vec![];
        }
        let union = before.union(after);
        let common = before.intersection(after);
        let left = common.x.saturating_add(15) / 16 * 16;
        let top = common.y.saturating_add(15) / 16 * 16;
        let right = common.x.saturating_add(common.width) / 16 * 16;
        let bottom = common.y.saturating_add(common.height) / 16 * 16;
        if left >= right || top >= bottom {
            return vec![union];
        }
        [
            Rect {
                x: union.x,
                y: union.y,
                width: union.width,
                height: top - union.y,
            },
            Rect {
                x: union.x,
                y: bottom,
                width: union.width,
                height: union.y.saturating_add(union.height) - bottom,
            },
            Rect {
                x: union.x,
                y: top,
                width: left - union.x,
                height: bottom - top,
            },
            Rect {
                x: right,
                y: top,
                width: union.x.saturating_add(union.width) - right,
                height: bottom - top,
            },
        ]
        .into_iter()
        .filter(|r| r.width > 0 && r.height > 0)
        .collect()
    }
}

#[derive(Clone, Copy)]
pub struct SceneSource<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
    /// Source pixel corresponding to annotation coordinate (0, 0).
    pub origin: (u32, u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub points: Vec<Point>,
    pub width: f32,
    pub color: [u8; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Arrow { filled: bool },
    Highlight,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub start: Point,
    pub end: Point,
    pub width: f32,
    pub color: [u8; 4],
}
impl Shape {
    pub(crate) fn wings(&self) -> (Point, Point) {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let angle = dy.atan2(dx);
        // Match the prototype's 18px, +/-0.5 radian head, bounded for short arrows.
        let length = dx.hypot(dy).min(18.0);
        let point = |a: f32| Point {
            x: self.end.x - length * a.cos(),
            y: self.end.y - length * a.sin(),
        };
        (point(angle - 0.5), point(angle + 0.5))
    }
    pub fn bounds(&self) -> Rect {
        // The open head's acute miter extends beyond a round cap by ~1.043 widths.
        let width = if matches!(self.kind, ShapeKind::Arrow { .. }) {
            self.width * 2.2
        } else {
            self.width
        };
        let mut bounds = Stroke::segment_bounds(self.start, self.end, width);
        if matches!(self.kind, ShapeKind::Arrow { .. }) {
            let (a, b) = self.wings();
            bounds = bounds.union(Stroke::segment_bounds(a, b, width));
        }
        bounds
    }
}

impl Stroke {
    pub fn segment_bounds(a: Point, b: Point, width: f32) -> Rect {
        let radius = width / 2.0 + 2.0; // antialiasing fringe and rounded caps
        let x = (a.x.min(b.x) - radius).floor().max(0.0) as u32;
        let y = (a.y.min(b.y) - radius).floor().max(0.0) as u32;
        let right = (a.x.max(b.x) + radius).ceil().max(0.0) as u32;
        let bottom = (a.y.max(b.y) + radius).ceil().max(0.0) as u32;
        Rect {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Text(crate::TextAnnotation),
    Redaction(Redaction),
    Pixelation(Pixelation),
    Stroke(Stroke),
    Shape(Shape),
}
impl From<crate::TextAnnotation> for Annotation {
    fn from(value: crate::TextAnnotation) -> Self {
        Self::Text(value)
    }
}
impl From<Pixelation> for Annotation {
    fn from(value: Pixelation) -> Self {
        Self::Pixelation(value)
    }
}
impl From<Shape> for Annotation {
    fn from(value: Shape) -> Self {
        Self::Shape(value)
    }
}
impl From<Redaction> for Annotation {
    fn from(value: Redaction) -> Self {
        Self::Redaction(value)
    }
}
impl From<Stroke> for Annotation {
    fn from(value: Stroke) -> Self {
        Self::Stroke(value)
    }
}

impl Annotation {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Text(mark) => mark.is_valid(),
            Self::Pixelation(mark) => {
                mark.bounds.width > 0
                    && mark.bounds.height > 0
                    && mark.bounds.x.checked_add(mark.bounds.width).is_some()
                    && mark.bounds.y.checked_add(mark.bounds.height).is_some()
            }
            Self::Redaction(mark) => mark.bounds.width > 0 && mark.bounds.height > 0,
            Self::Stroke(stroke) => {
                !stroke.points.is_empty()
                    && stroke.points.len() <= 65536
                    && stroke.width.is_finite()
                    && (1.0..=128.0).contains(&stroke.width)
                    && stroke.points.iter().all(|p| {
                        p.x.is_finite()
                            && p.y.is_finite()
                            && (0.0..=1_000_000.0).contains(&p.x)
                            && (0.0..=1_000_000.0).contains(&p.y)
                    })
            }
            Self::Shape(shape) => {
                [shape.start, shape.end].iter().all(|p| {
                    p.x.is_finite()
                        && p.y.is_finite()
                        && (0.0..=1_000_000.0).contains(&p.x)
                        && (0.0..=1_000_000.0).contains(&p.y)
                }) && shape.width.is_finite()
                    && (1.0..=128.0).contains(&shape.width)
                    && match shape.kind {
                        ShapeKind::Arrow { .. } => shape.start != shape.end,
                        _ => shape.start.x != shape.end.x && shape.start.y != shape.end.y,
                    }
            }
        }
    }
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Text(mark) => mark.bounds(),
            Self::Pixelation(mark) => mark.bounds,
            Self::Redaction(mark) => mark.bounds,
            Self::Shape(shape) => shape.bounds(),
            Self::Stroke(stroke) => stroke
                .points
                .iter()
                .map(|p| Stroke::segment_bounds(*p, *p, stroke.width))
                .fold(Rect::default(), Rect::union),
        }
    }
    pub(crate) fn points_len(&self) -> usize {
        if let Self::Stroke(stroke) = self {
            stroke.points.len()
        } else {
            0
        }
    }
}

/// Canonical source-pixel tile size. Export and preview must use the same raster clip grid.
pub const ANNOTATION_TILE_SIZE: u32 = 128;

/// Render premultiplied RGBA on the canonical grid, then crop. A rasterizer's clipping can affect
/// antialiasing, so independently rendering arbitrary crop sizes does not guarantee identical pixels.
pub fn render_annotations<'a>(
    crop: Rect,
    marks: impl IntoIterator<Item = &'a Annotation>,
) -> Result<Vec<u8>, &'static str> {
    render_impl(crop, marks, None)
}

/// Full scene on the same canonical grid as annotation-only rendering, premultiplied RGBA.
/// Source is borrowed; no full-size image copies are needed for individual tile updates.
pub fn render_scene<'a>(
    crop: Rect,
    marks: impl IntoIterator<Item = &'a Annotation>,
    source: SceneSource<'_>,
) -> Result<Vec<u8>, &'static str> {
    if (source.width as usize)
        .checked_mul(source.height as usize)
        .and_then(|n| n.checked_mul(4))
        != Some(source.rgba.len())
    {
        return Err("Invalid scene source RGBA buffer");
    }
    render_impl(crop, marks, Some(source))
}

fn render_impl<'a>(
    crop: Rect,
    marks: impl IntoIterator<Item = &'a Annotation>,
    source: Option<SceneSource<'_>>,
) -> Result<Vec<u8>, &'static str> {
    if crop.width == 0
        || crop.height == 0
        || u64::from(crop.width) * u64::from(crop.height) > 128 * 1024 * 1024
    {
        return Err("Annotation surface exceeds 512 MiB budget or is empty");
    }
    let marks: Vec<_> = marks.into_iter().collect();
    if marks.iter().any(|mark| !mark.is_valid()) {
        return Err("Invalid annotation geometry");
    }
    if source.is_none() && marks.iter().any(|m| matches!(m, Annotation::Pixelation(_))) {
        return Err("Pixelation requires a scene source");
    }
    let right = crop
        .x
        .checked_add(crop.width)
        .ok_or("Annotation crop overflow")?;
    let bottom = crop
        .y
        .checked_add(crop.height)
        .ok_or("Annotation crop overflow")?;
    let tile = ANNOTATION_TILE_SIZE;
    if crop.x.is_multiple_of(tile)
        && crop.y.is_multiple_of(tile)
        && crop.width == tile
        && crop.height == tile
    {
        return raster_tile(crop, &marks, source);
    }
    let mut output = vec![0; crop.width as usize * crop.height as usize * 4];
    for ty in crop.y / tile..=(bottom - 1) / tile {
        for tx in crop.x / tile..=(right - 1) / tile {
            let region = Rect {
                x: tx * tile,
                y: ty * tile,
                width: tile,
                height: tile,
            };
            let visible = region.intersection(crop);
            let pixels = raster_tile(region, &marks, source)?;
            for y in visible.y..visible.y + visible.height {
                let dst = ((y - crop.y) as usize * crop.width as usize
                    + (visible.x - crop.x) as usize)
                    * 4;
                let src =
                    ((y - region.y) as usize * tile as usize + (visible.x - region.x) as usize) * 4;
                let count = visible.width as usize * 4;
                output[dst..dst + count].copy_from_slice(&pixels[src..src + count]);
            }
        }
    }
    Ok(output)
}

fn raster_tile(
    crop: Rect,
    marks: &[&Annotation],
    source: Option<SceneSource<'_>>,
) -> Result<Vec<u8>, &'static str> {
    let mut canvas =
        Pixmap::new(crop.width, crop.height).ok_or("Cannot allocate annotation surface")?;
    if let Some(source) = source {
        for y in 0..crop.height {
            for x in 0..crop.width {
                let sx = u64::from(source.origin.0) + u64::from(crop.x) + u64::from(x);
                let sy = u64::from(source.origin.1) + u64::from(crop.y) + u64::from(y);
                if sx >= u64::from(source.width) || sy >= u64::from(source.height) {
                    continue;
                }
                let at = ((sy * u64::from(source.width) + sx) * 4) as usize;
                let to = ((y * crop.width + x) * 4) as usize;
                let alpha = u32::from(source.rgba[at + 3]);
                for c in 0..3 {
                    canvas.data_mut()[to + c] =
                        ((u32::from(source.rgba[at + c]) * alpha + 127) / 255) as u8;
                }
                canvas.data_mut()[to + 3] = alpha as u8;
            }
        }
    }
    let transform = Transform::from_translate(-(crop.x as f32), -(crop.y as f32));
    for &mark in marks {
        let bounds = mark.bounds().intersection(crop);
        if bounds.width == 0 || bounds.height == 0 {
            continue;
        }
        let mut paint = Paint::default();
        match mark {
            Annotation::Text(mark) => {
                for y in bounds.y..bounds.y + bounds.height {
                    for x in bounds.x..bounds.x + bounds.width {
                        let from =
                            ((y - mark.origin.1) * mark.mask.width + x - mark.origin.0) as usize;
                        let to = (((y - crop.y) * crop.width + x - crop.x) * 4) as usize;
                        let a = (u32::from(mark.mask.coverage[from]) * u32::from(mark.color[3])
                            + 127)
                            / 255;
                        for c in 0..3 {
                            let over = (u32::from(mark.color[c]) * a + 127) / 255;
                            canvas.data_mut()[to + c] =
                                (over + (u32::from(canvas.data()[to + c]) * (255 - a) + 127) / 255)
                                    .min(255) as u8;
                        }
                        canvas.data_mut()[to + 3] =
                            (a + (u32::from(canvas.data()[to + 3]) * (255 - a) + 127) / 255) as u8;
                    }
                }
            }
            Annotation::Pixelation(_) => pixelate(&mut canvas, crop, bounds),
            Annotation::Shape(shape) => {
                let alpha = if shape.kind == ShapeKind::Highlight {
                    (f32::from(shape.color[3]) * 0.45).round() as u8
                } else {
                    shape.color[3]
                };
                paint.set_color_rgba8(shape.color[0], shape.color[1], shape.color[2], alpha);
                let mut path = PathBuilder::new();
                match shape.kind {
                    ShapeKind::Rectangle | ShapeKind::Highlight => {
                        if let Some(rect) = tiny_skia::Rect::from_ltrb(
                            shape.start.x.min(shape.end.x),
                            shape.start.y.min(shape.end.y),
                            shape.start.x.max(shape.end.x),
                            shape.start.y.max(shape.end.y),
                        ) {
                            path.push_rect(rect);
                        }
                    }
                    ShapeKind::Arrow { filled } => {
                        let (a, b) = shape.wings();
                        path.move_to(shape.start.x, shape.start.y);
                        path.line_to(shape.end.x, shape.end.y);
                        if !filled {
                            path.move_to(a.x, a.y);
                            path.line_to(shape.end.x, shape.end.y);
                            path.line_to(b.x, b.y);
                        }
                    }
                }
                if let Some(path) = path.finish() {
                    if shape.kind == ShapeKind::Highlight {
                        canvas.fill_path(&path, &paint, FillRule::Winding, transform, None);
                    } else {
                        canvas.stroke_path(
                            &path,
                            &paint,
                            &tiny_skia::Stroke {
                                width: shape.width,
                                line_cap: if matches!(shape.kind, ShapeKind::Arrow { .. }) {
                                    LineCap::Round
                                } else {
                                    LineCap::Butt
                                },
                                line_join: LineJoin::Miter,
                                ..Default::default()
                            },
                            transform,
                            None,
                        );
                    }
                }
                if matches!(shape.kind, ShapeKind::Arrow { filled: true }) {
                    let (a, b) = shape.wings();
                    let mut head = PathBuilder::new();
                    head.move_to(a.x, a.y);
                    head.line_to(shape.end.x, shape.end.y);
                    head.line_to(b.x, b.y);
                    head.close();
                    if let Some(path) = head.finish() {
                        canvas.fill_path(&path, &paint, FillRule::Winding, transform, None);
                    }
                }
            }
            Annotation::Redaction(mark) => {
                paint.set_color_rgba8(mark.color[0], mark.color[1], mark.color[2], 255);
                if let Some(rect) = tiny_skia::Rect::from_xywh(
                    mark.bounds.x as f32,
                    mark.bounds.y as f32,
                    mark.bounds.width as f32,
                    mark.bounds.height as f32,
                ) {
                    canvas.fill_rect(rect, &paint, transform, None);
                }
            }
            Annotation::Stroke(stroke) => {
                paint.set_color_rgba8(
                    stroke.color[0],
                    stroke.color[1],
                    stroke.color[2],
                    stroke.color[3],
                );
                let mut path = PathBuilder::new();
                let first = stroke.points[0];
                if stroke.points.len() == 1 {
                    path.push_circle(first.x, first.y, stroke.width / 2.0);
                    if let Some(path) = path.finish() {
                        canvas.fill_path(&path, &paint, FillRule::Winding, transform, None);
                    }
                } else {
                    path.move_to(first.x, first.y);
                    for point in &stroke.points[1..] {
                        path.line_to(point.x, point.y);
                    }
                    if let Some(path) = path.finish() {
                        canvas.stroke_path(
                            &path,
                            &paint,
                            &tiny_skia::Stroke {
                                width: stroke.width,
                                line_cap: LineCap::Round,
                                line_join: LineJoin::Round,
                                ..Default::default()
                            },
                            transform,
                            None,
                        );
                    }
                }
            }
        }
    }
    Ok(canvas.take())
}

fn pixelate(canvas: &mut Pixmap, crop: Rect, bounds: Rect) {
    // 16 divides the canonical tile size, so every sample cell is entirely in one tile.
    // Only pixels inside the mark are averaged, including partial edge cells.
    const BLOCK: u32 = 16;
    for by in bounds.y / BLOCK..=(bounds.y + bounds.height - 1) / BLOCK {
        for bx in bounds.x / BLOCK..=(bounds.x + bounds.width - 1) / BLOCK {
            let cell = Rect {
                x: bx * BLOCK,
                y: by * BLOCK,
                width: BLOCK,
                height: BLOCK,
            }
            .intersection(bounds);
            let mut sum = [0u32; 4];
            for y in cell.y..cell.y + cell.height {
                for x in cell.x..cell.x + cell.width {
                    let at = (((y - crop.y) * crop.width + x - crop.x) * 4) as usize;
                    for (s, &value) in sum.iter_mut().zip(&canvas.data()[at..at + 4]) {
                        *s += u32::from(value);
                    }
                }
            }
            let count = cell.width * cell.height;
            let average = sum.map(|s| ((s + count / 2) / count) as u8);
            for y in cell.y..cell.y + cell.height {
                for x in cell.x..cell.x + cell.width {
                    let at = (((y - crop.y) * crop.width + x - crop.x) * 4) as usize;
                    canvas.data_mut()[at..at + 4].copy_from_slice(&average);
                }
            }
        }
    }
}
