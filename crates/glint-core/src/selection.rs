//! Source-pixel selection geometry. Desktop origin and UI scaling belong to adapters.
use crate::Rect;

/// Map physical desktop bounds into the frozen source, clipping negative/outside coordinates.
pub fn desktop_region(bounds: (i32, i32, i32, i32), origin: (i32, i32), size: (u32, u32)) -> Rect {
    let (left, top, right, bottom) = bounds;
    let x = (i64::from(left) - i64::from(origin.0)).clamp(0, i64::from(size.0)) as u32;
    let y = (i64::from(top) - i64::from(origin.1)).clamp(0, i64::from(size.1)) as u32;
    let right = (i64::from(right) - i64::from(origin.0)).clamp(0, i64::from(size.0)) as u32;
    let bottom = (i64::from(bottom) - i64::from(origin.1)).clamp(0, i64::from(size.1)) as u32;
    Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}
/// Regions must already be in front-to-back order; right/bottom are exclusive pixel bounds.
pub fn first_region_at(regions: &[Rect], point: (u32, u32)) -> Option<Rect> {
    regions.iter().copied().find(|r| {
        r.width > 0
            && r.height > 0
            && point.0 >= r.x
            && point.1 >= r.y
            && u64::from(point.0) < u64::from(r.x) + u64::from(r.width)
            && u64::from(point.1) < u64::from(r.y) + u64::from(r.height)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionHit {
    Outside,
    Inside,
    Edge(Edges),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

pub fn selection_hit(rect: Rect, point: (u32, u32), tolerance: (u32, u32)) -> SelectionHit {
    if rect.width == 0 || rect.height == 0 {
        return SelectionHit::Outside;
    }
    let right = rect.x.saturating_add(rect.width);
    let bottom = rect.y.saturating_add(rect.height);
    if point.0 < rect.x.saturating_sub(tolerance.0)
        || point.0 > right.saturating_add(tolerance.0)
        || point.1 < rect.y.saturating_sub(tolerance.1)
        || point.1 > bottom.saturating_add(tolerance.1)
    {
        return SelectionHit::Outside;
    }
    // For tiny regions, select the nearer edge instead of moving both opposing edges.
    let left = point.0.abs_diff(rect.x) <= tolerance.0
        && point.0.abs_diff(rect.x) <= point.0.abs_diff(right);
    let right_edge = point.0.abs_diff(right) <= tolerance.0 && !left;
    let top = point.1.abs_diff(rect.y) <= tolerance.1
        && point.1.abs_diff(rect.y) <= point.1.abs_diff(bottom);
    let bottom_edge = point.1.abs_diff(bottom) <= tolerance.1 && !top;
    let edges = Edges {
        left,
        right: right_edge,
        top,
        bottom: bottom_edge,
    };
    if edges != Edges::default() {
        SelectionHit::Edge(edges)
    } else if point.0 >= rect.x && point.0 <= right && point.1 >= rect.y && point.1 <= bottom {
        SelectionHit::Inside
    } else {
        SelectionHit::Outside
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SelectionGesture {
    pub hit: SelectionHit,
    original: Rect,
    start: (u32, u32),
    limits: (u32, u32),
    minimum: (u32, u32),
}
impl SelectionGesture {
    pub fn begin(
        rect: Rect,
        point: (u32, u32),
        tolerance: (u32, u32),
        limits: (u32, u32),
        minimum: (u32, u32),
    ) -> Self {
        let original = rect.clipped(limits.0, limits.1);
        Self {
            hit: selection_hit(original, point, tolerance),
            original,
            start: point,
            limits,
            minimum,
        }
    }
    pub fn update(self, point: (u32, u32)) -> Rect {
        let p = (point.0.min(self.limits.0), point.1.min(self.limits.1));
        let r = self.original;
        match self.hit {
            SelectionHit::Outside => Rect::between(
                (
                    self.start.0.min(self.limits.0),
                    self.start.1.min(self.limits.1),
                ),
                p,
            ),
            SelectionHit::Inside => Rect {
                x: (i64::from(r.x) + i64::from(p.0) - i64::from(self.start.0))
                    .clamp(0, i64::from(self.limits.0 - r.width)) as u32,
                y: (i64::from(r.y) + i64::from(p.1) - i64::from(self.start.1))
                    .clamp(0, i64::from(self.limits.1 - r.height)) as u32,
                ..r
            },
            SelectionHit::Edge(edges) => {
                let (mut left, mut top, mut right, mut bottom) =
                    (r.x, r.y, r.x + r.width, r.y + r.height);
                // Relative motion keeps an edge from jumping when grabbed within its hit area.
                let dx = i64::from(p.0) - i64::from(self.start.0);
                let dy = i64::from(p.1) - i64::from(self.start.1);
                if edges.left {
                    left = (i64::from(left) + dx).clamp(
                        0,
                        i64::from(right.saturating_sub(self.minimum.0.max(1).min(right))),
                    ) as u32;
                }
                if edges.right {
                    right = (i64::from(right) + dx).clamp(
                        i64::from(left + self.minimum.0.max(1).min(self.limits.0 - left)),
                        i64::from(self.limits.0),
                    ) as u32;
                }
                if edges.top {
                    top = (i64::from(top) + dy).clamp(
                        0,
                        i64::from(bottom.saturating_sub(self.minimum.1.max(1).min(bottom))),
                    ) as u32;
                }
                if edges.bottom {
                    bottom = (i64::from(bottom) + dy).clamp(
                        i64::from(top + self.minimum.1.max(1).min(self.limits.1 - top)),
                        i64::from(self.limits.1),
                    ) as u32;
                }
                Rect {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const RECT: Rect = Rect {
        x: 40,
        y: 30,
        width: 100,
        height: 80,
    };
    #[test]
    fn desktop_mapping_clips_negative_origins_and_preserves_front_order() {
        let a = desktop_region((-150, -90, 100, 200), (-100, -50), (300, 250));
        assert_eq!(
            a,
            Rect {
                x: 0,
                y: 0,
                width: 200,
                height: 250
            }
        );
        let b = desktop_region((0, 0, 80, 80), (-100, -50), (300, 250));
        assert_eq!(
            b,
            Rect {
                x: 100,
                y: 50,
                width: 80,
                height: 80
            }
        );
        assert_eq!(first_region_at(&[b, a], (120, 70)), Some(b));
        assert_eq!(first_region_at(&[b, a], (180, 70)), Some(a));
        assert_eq!(first_region_at(&[b, a], (299, 249)), None);
        assert_eq!(
            desktop_region(
                (i32::MIN, i32::MIN, i32::MAX, i32::MAX),
                (i32::MIN, i32::MIN),
                (500, 500)
            ),
            Rect {
                x: 0,
                y: 0,
                width: 500,
                height: 500
            }
        );
        assert_eq!(
            desktop_region((500, 500, 400, 400), (0, 0), (500, 500)).width,
            0
        );
    }
    fn gesture(point: (u32, u32)) -> SelectionGesture {
        SelectionGesture::begin(RECT, point, (10, 10), (200, 150), (30, 30))
    }
    #[test]
    fn move_clamps_and_uses_original_instead_of_accumulated_deltas() {
        let g = gesture((80, 70));
        assert_eq!(g.hit, SelectionHit::Inside);
        assert_eq!(
            g.update((100, 90)),
            Rect {
                x: 60,
                y: 50,
                ..RECT
            }
        );
        assert_eq!(
            g.update((200, 150)),
            Rect {
                x: 100,
                y: 70,
                ..RECT
            }
        );
        assert_eq!(g.update((0, 0)), Rect { x: 0, y: 0, ..RECT });
        assert_eq!(g.update((80, 70)), RECT);
    }
    #[test]
    fn corners_and_edges_keep_opposite_anchors_and_enforce_minimum() {
        for (start, end, expected) in [
            (
                (40, 30),
                (0, 0),
                Rect {
                    x: 0,
                    y: 0,
                    width: 140,
                    height: 110,
                },
            ),
            (
                (140, 110),
                (200, 150),
                Rect {
                    x: 40,
                    y: 30,
                    width: 160,
                    height: 120,
                },
            ),
            (
                (40, 110),
                (200, 0),
                Rect {
                    x: 110,
                    y: 30,
                    width: 30,
                    height: 30,
                },
            ),
            (
                (140, 30),
                (0, 150),
                Rect {
                    x: 40,
                    y: 80,
                    width: 30,
                    height: 30,
                },
            ),
            (
                (45, 70),
                (55, 70),
                Rect {
                    x: 50,
                    y: 30,
                    width: 90,
                    height: 80,
                },
            ),
            (
                (80, 110),
                (80, 150),
                Rect {
                    x: 40,
                    y: 30,
                    width: 100,
                    height: 120,
                },
            ),
        ] {
            assert_eq!(gesture(start).update(end), expected);
        }
    }
    #[test]
    fn new_regions_reverse_drag_and_tiny_overflowing_inputs_remain_bounded() {
        let g = gesture((190, 140));
        assert_eq!(g.hit, SelectionHit::Outside);
        assert_eq!(
            g.update((0, 0)),
            Rect {
                x: 0,
                y: 0,
                width: 190,
                height: 140
            }
        );
        let tiny = Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        assert_eq!(
            selection_hit(tiny, (1, 1), (10, 10)),
            SelectionHit::Edge(Edges {
                right: true,
                bottom: true,
                ..Edges::default()
            })
        );
        let g = SelectionGesture::begin(
            Rect {
                x: u32::MAX - 5,
                y: 0,
                width: 50,
                height: 50,
            },
            (u32::MAX, 25),
            (10, 10),
            (u32::MAX, 50),
            (30, 30),
        );
        let result = g.update((0, 25));
        assert_eq!(result.x + result.width, u32::MAX);
        assert!(result.width <= u32::MAX - result.x);
    }
}
