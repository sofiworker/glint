//! Object geometry in selection-local source pixels. No raster or UI dependencies.
use crate::{Annotation, Point, Rect, ShapeKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeHandle {
    pub id: u8,
    pub point: Point,
}
impl Annotation {
    fn resize_frame(&self) -> Option<[f64; 4]> {
        let frame = match self {
            Self::Text(_) => return None,
            Self::Shape(s) => {
                if matches!(s.kind, ShapeKind::Arrow { .. }) {
                    return None;
                }
                [
                    f64::from(s.start.x.min(s.end.x)),
                    f64::from(s.start.y.min(s.end.y)),
                    f64::from(s.start.x.max(s.end.x)),
                    f64::from(s.start.y.max(s.end.y)),
                ]
            }
            Self::Stroke(s) => s.points.iter().fold(
                [
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ],
                |a, p| {
                    [
                        a[0].min(f64::from(p.x)),
                        a[1].min(f64::from(p.y)),
                        a[2].max(f64::from(p.x)),
                        a[3].max(f64::from(p.y)),
                    ]
                },
            ),
            _ => {
                let r = self.bounds();
                [
                    f64::from(r.x),
                    f64::from(r.y),
                    f64::from(r.x) + f64::from(r.width),
                    f64::from(r.y) + f64::from(r.height),
                ]
            }
        };
        (frame.iter().all(|n| n.is_finite())
            && frame[2] >= frame[0]
            && frame[3] >= frame[1]
            && (frame[2] > frame[0] || frame[3] > frame[1]))
            .then_some(frame)
    }
    /// NW, N, NE, E, SE, S, SW, W = 0..7. Arrow start/end = 8/9.
    /// Text size uses font shaping rather than stretching an existing glyph mask.
    pub fn resize_handles(&self) -> Vec<ResizeHandle> {
        if !self.is_valid() {
            return vec![];
        }
        if let Self::Shape(s) = self
            && matches!(s.kind, ShapeKind::Arrow { .. })
        {
            return vec![
                ResizeHandle {
                    id: 8,
                    point: s.start,
                },
                ResizeHandle {
                    id: 9,
                    point: s.end,
                },
            ];
        }
        let Some([l, t, r, b]) = self.resize_frame() else {
            return vec![];
        };
        [
            (0., 0.),
            (0.5, 0.),
            (1., 0.),
            (1., 0.5),
            (1., 1.),
            (0.5, 1.),
            (0., 1.),
            (0., 0.5),
        ]
        .into_iter()
        .enumerate()
        .filter(|(id, _)| {
            if r == l {
                [1, 5].contains(id)
            } else if b == t {
                [3, 7].contains(id)
            } else {
                true
            }
        })
        .map(|(id, (x, y))| ResizeHandle {
            id: id as u8,
            point: Point {
                x: (l + (r - l) * x) as f32,
                y: (t + (b - t) * y) as f32,
            },
        })
        .collect()
    }
    /// Hit only visible grips; choose the closest when small-object grips overlap.
    pub fn resize_handle_hit(
        &self,
        p: Point,
        tolerance: (f32, f32),
        clip: (u32, u32),
    ) -> Option<u8> {
        if ![p.x, p.y, tolerance.0, tolerance.1]
            .iter()
            .all(|n| n.is_finite())
            || tolerance.0 < 0.
            || tolerance.1 < 0.
        {
            return None;
        }
        self.resize_handles()
            .into_iter()
            .filter(|g| {
                g.point.x >= 0.
                    && g.point.y >= 0.
                    && g.point.x <= clip.0 as f32
                    && g.point.y <= clip.1 as f32
                    && (g.point.x - p.x).abs() <= tolerance.0
                    && (g.point.y - p.y).abs() <= tolerance.1
            })
            .min_by(|a, b| {
                let distance = |g: &ResizeHandle| {
                    ((g.point.x - p.x) / tolerance.0.max(0.001)).powi(2)
                        + ((g.point.y - p.y) / tolerance.1.max(0.001)).powi(2)
                };
                distance(a).total_cmp(&distance(b))
            })
            .map(|g| g.id)
    }
    /// Relative motion retains the pointer's offset within a handle's hit area.
    /// Opposite edges stay fixed; crossing clamps to one source pixel. Stroke width is unchanged.
    pub fn resized(&self, handle: u8, dx: i64, dy: i64, limit: (u32, u32)) -> Option<Self> {
        if !self.is_valid() || limit.0 == 0 || limit.1 == 0 {
            return None;
        }
        if let Self::Shape(s) = self
            && matches!(s.kind, ShapeKind::Arrow { .. })
        {
            if ![8, 9].contains(&handle) {
                return None;
            }
            let mut s = s.clone();
            let p = if handle == 8 {
                &mut s.start
            } else {
                &mut s.end
            };
            p.x = (f64::from(p.x) + dx as f64).clamp(0., f64::from(limit.0)) as f32;
            p.y = (f64::from(p.y) + dy as f64).clamp(0., f64::from(limit.1)) as f32;
            let mark = Self::Shape(s);
            return mark.is_valid().then_some(mark);
        }
        if handle > 7 {
            return None;
        }
        let [l, t, r, b] = self.resize_frame()?;
        if r == l && ![1, 5].contains(&handle) || b == t && ![3, 7].contains(&handle) {
            return None;
        }
        if dx == 0 && dy == 0 {
            return Some(self.clone());
        }
        let (mut left, mut top, mut right, mut bottom) = (l, t, r, b);
        if [0, 6, 7].contains(&handle) {
            left = (l + dx as f64).clamp(0., (r - 1.).min(f64::from(limit.0)).max(0.));
        }
        if [2, 3, 4].contains(&handle) {
            right = (r + dx as f64).clamp(l + 1., f64::from(limit.0).max(l + 1.));
        }
        if [0, 1, 2].contains(&handle) {
            top = (t + dy as f64).clamp(0., (b - 1.).min(f64::from(limit.1)).max(0.));
        }
        if [4, 5, 6].contains(&handle) {
            bottom = (b + dy as f64).clamp(t + 1., f64::from(limit.1).max(t + 1.));
        }
        let mut mark = self.clone();
        let rect = Rect {
            x: left.round() as u32,
            y: top.round() as u32,
            width: (right - left).round() as u32,
            height: (bottom - top).round() as u32,
        };
        match &mut mark {
            Self::Redaction(m) => m.bounds = rect,
            Self::Pixelation(m) => m.bounds = rect,
            Self::Shape(s) => {
                let point = |p: Point| Point {
                    x: if f64::from(p.x) == l { left } else { right } as f32,
                    y: if f64::from(p.y) == t { top } else { bottom } as f32,
                };
                s.start = point(s.start);
                s.end = point(s.end);
            }
            Self::Stroke(s) => {
                for p in &mut s.points {
                    if r > l {
                        p.x = (left + (f64::from(p.x) - l) * (right - left) / (r - l)) as f32;
                    }
                    if b > t {
                        p.y = (top + (f64::from(p.y) - t) * (bottom - top) / (b - t)) as f32;
                    }
                }
            }
            Self::Text(_) => return None,
        }
        (mark.is_valid()
            && rect.x.checked_add(rect.width).is_some()
            && rect.y.checked_add(rect.height).is_some())
        .then_some(mark)
    }
}
