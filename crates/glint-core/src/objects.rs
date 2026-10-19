use crate::{Annotation, Point, ShapeKind};
impl Annotation {
    /// Change applicable appearance fields without touching geometry or text glyphs.
    pub fn restyled(&self, color: [u8; 4], width: f32, filled: bool) -> Option<Self> {
        let mut mark = self.clone();
        match &mut mark {
            Self::Text(t) => t.color = color,
            Self::Redaction(r) => r.color = [color[0], color[1], color[2], 255],
            Self::Stroke(s) => {
                s.color = color;
                s.width = width;
            }
            Self::Shape(s) => {
                s.color = color;
                if s.kind != ShapeKind::Highlight {
                    s.width = width;
                }
                if matches!(s.kind, ShapeKind::Arrow { .. }) {
                    s.kind = ShapeKind::Arrow { filled };
                }
            }
            Self::Pixelation(_) => return None,
        }
        mark.is_valid().then_some(mark)
    }
    /// Reverse-order callers pick the visually topmost object. Hollow shapes retain
    /// an empty interior so clicking through their centre can move the selection.
    pub fn hit_test(&self, p: Point, tolerance: f32) -> bool {
        let b = self.bounds();
        if !tolerance.is_finite()
            || tolerance < 0.0
            || !p.x.is_finite()
            || !p.y.is_finite()
            || p.x < b.x as f32 - tolerance
            || p.y < b.y as f32 - tolerance
            || p.x > b.x.saturating_add(b.width) as f32 + tolerance
            || p.y > b.y.saturating_add(b.height) as f32 + tolerance
        {
            return false;
        }
        let segment = |a: Point, b: Point, width: f32| {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let len = dx * dx + dy * dy;
            let t = if len > 0.0 {
                ((p.x - a.x) * dx + (p.y - a.y) * dy) / len
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy) <= width / 2.0 + tolerance
        };
        match self {
            Self::Stroke(s) => {
                s.points.windows(2).any(|v| segment(v[0], v[1], s.width))
                    || s.points.len() == 1 && segment(s.points[0], s.points[0], s.width)
            }
            Self::Shape(s) if s.kind == ShapeKind::Rectangle => {
                let a = s.start;
                let b = s.end;
                segment(a, Point { x: b.x, y: a.y }, s.width)
                    || segment(Point { x: b.x, y: a.y }, b, s.width)
                    || segment(b, Point { x: a.x, y: b.y }, s.width)
                    || segment(Point { x: a.x, y: b.y }, a, s.width)
            }
            Self::Shape(s) if matches!(s.kind, ShapeKind::Arrow { .. }) => {
                let (a, b) = s.wings();
                let cross = |a: Point, b: Point, c: Point| {
                    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
                };
                let inside = matches!(s.kind, ShapeKind::Arrow { filled: true }) && {
                    let v = [cross(a, s.end, p), cross(s.end, b, p), cross(b, a, p)];
                    v.iter().all(|x| *x >= 0.0) || v.iter().all(|x| *x <= 0.0)
                };
                inside
                    || segment(s.start, s.end, s.width)
                    || segment(a, s.end, s.width)
                    || segment(b, s.end, s.width)
            }
            _ => true,
        }
    }
    /// Move in annotation coordinates, keeping the object's bounding box in the
    /// current selection when it fits. A larger clipped object keeps its position.
    pub fn translated(&self, dx: i64, dy: i64, limit: (u32, u32)) -> Option<Self> {
        let bounds = self.bounds();
        let shift = |delta: i64, start: u32, size: u32, limit: u32| {
            if size > limit {
                0
            } else {
                delta.clamp(
                    -i64::from(start),
                    (i64::from(limit - size) - i64::from(start)).max(0),
                )
            }
        };
        let dx = shift(dx, bounds.x, bounds.width, limit.0);
        let dy = shift(dy, bounds.y, bounds.height, limit.1);
        let mut moved = self.clone();
        let point = |p: &mut Point| {
            p.x += dx as f32;
            p.y += dy as f32;
        };
        match &mut moved {
            Self::Text(t) => {
                t.origin.0 = (i64::from(t.origin.0) + dx) as u32;
                t.origin.1 = (i64::from(t.origin.1) + dy) as u32;
            }
            Self::Redaction(r) => {
                r.bounds.x = (i64::from(r.bounds.x) + dx) as u32;
                r.bounds.y = (i64::from(r.bounds.y) + dy) as u32;
            }
            Self::Pixelation(r) => {
                r.bounds.x = (i64::from(r.bounds.x) + dx) as u32;
                r.bounds.y = (i64::from(r.bounds.y) + dy) as u32;
            }
            Self::Stroke(s) => s.points.iter_mut().for_each(point),
            Self::Shape(s) => {
                point(&mut s.start);
                point(&mut s.end);
            }
        }
        moved.is_valid().then_some(moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rect, Redaction, Shape};
    #[test]
    fn hollow_hit_and_arrow_empty_space() {
        let mut s = Shape {
            kind: ShapeKind::Rectangle,
            start: Point { x: 20., y: 20. },
            end: Point { x: 100., y: 100. },
            width: 3.,
            color: [0; 4],
        };
        assert!(!Annotation::from(s.clone()).hit_test(Point { x: 60., y: 60. }, 5.));
        assert!(Annotation::from(s.clone()).hit_test(Point { x: 20., y: 60. }, 5.));
        s.kind = ShapeKind::Arrow { filled: true };
        assert!(!Annotation::from(s.clone()).hit_test(Point { x: 25., y: 90. }, 5.));
        assert!(Annotation::from(s).hit_test(Point { x: 60., y: 60. }, 5.));
    }
    #[test]
    fn moves_clamp_without_click_jumps_for_clipped_objects() {
        let m: Annotation = Redaction {
            bounds: Rect {
                x: 80,
                y: 20,
                width: 30,
                height: 40,
            },
            color: [0; 4],
        }
        .into();
        assert_eq!(m.translated(0, 0, (100, 100)).unwrap(), m);
        let moved = m.translated(-1000, 1000, (100, 100)).unwrap();
        assert_eq!(
            moved.bounds(),
            Rect {
                x: 0,
                y: 60,
                width: 30,
                height: 40
            }
        );
        let s: Annotation = Shape {
            kind: ShapeKind::Rectangle,
            start: Point { x: 0., y: 0. },
            end: Point { x: 50., y: 50. },
            width: 3.,
            color: [0; 4],
        }
        .into();
        assert!(s.translated(-1000, -1000, (100, 100)).unwrap().is_valid());
        assert!(!m.hit_test(
            Point {
                x: f32::NAN,
                y: 20.
            },
            5.
        ));
    }
}
