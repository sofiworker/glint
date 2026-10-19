//! Signed physical window geometry, independent of UI and platform handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Bounds {
    fn valid(self) -> bool {
        self.right > self.left && self.bottom > self.top
    }
}
/// Choose the nearest edge within the threshold; work-area edges win ties.
/// Window edges only attract when the rectangles overlap on the other axis.
pub fn snap_position(
    pin: Bounds,
    work: Bounds,
    windows: &[Bounds],
    threshold: i32,
) -> Option<(i32, i32)> {
    if !pin.valid() || !work.valid() || threshold < 0 {
        return None;
    }
    let width = i64::from(pin.right) - i64::from(pin.left);
    let height = i64::from(pin.bottom) - i64::from(pin.top);
    let mut x = (i64::from(pin.left), i64::from(threshold) + 1);
    let mut y = (i64::from(pin.top), i64::from(threshold) + 1);
    let choose = |best: &mut (i64, i64), original: i32, candidate: i64| {
        let distance = (candidate - i64::from(original)).abs();
        if distance <= i64::from(threshold) && distance < best.1 && i32::try_from(candidate).is_ok()
        {
            *best = (candidate, distance);
        }
    };
    for edge in [i64::from(work.left), i64::from(work.right) - width] {
        choose(&mut x, pin.left, edge);
    }
    for edge in [i64::from(work.top), i64::from(work.bottom) - height] {
        choose(&mut y, pin.top, edge);
    }
    for target in windows.iter().filter(|b| b.valid()) {
        if pin.top < target.bottom && pin.bottom > target.top {
            for edge in [target.left, target.right] {
                choose(&mut x, pin.left, i64::from(edge));
                choose(&mut x, pin.left, i64::from(edge) - width);
            }
        }
        if pin.left < target.right && pin.right > target.left {
            for edge in [target.top, target.bottom] {
                choose(&mut y, pin.top, i64::from(edge));
                choose(&mut y, pin.top, i64::from(edge) - height);
            }
        }
    }
    Some((x.0 as i32, y.0 as i32))
}
/// Keep the title and controls reachable after a work-area change, even for
/// a window larger than the remaining monitor. Never changes visibility.
pub fn recover_position(pin: Bounds, work: Bounds) -> Option<(i32, i32)> {
    if !pin.valid() || !work.valid() {
        return None;
    }
    let x = i64::from(pin.left).clamp(
        i64::from(work.left),
        (i64::from(work.right) - (i64::from(pin.right) - i64::from(pin.left)))
            .max(i64::from(work.left)),
    );
    let y = i64::from(pin.top).clamp(
        i64::from(work.top),
        (i64::from(work.bottom) - (i64::from(pin.bottom) - i64::from(pin.top)))
            .max(i64::from(work.top)),
    );
    Some((x as i32, y as i32))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn b(x: i32, y: i32, w: i32, h: i32) -> Bounds {
        Bounds {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        }
    }
    #[test]
    fn negative_monitor_edges_and_nearest_window_targets() {
        let work = b(-1920, -200, 1920, 1080);
        assert_eq!(
            snap_position(b(-1913, -195, 400, 300), work, &[], 12),
            Some((-1920, -200))
        );
        assert_eq!(
            snap_position(b(-407, 575, 400, 300), work, &[], 12),
            Some((-400, 580))
        );
        assert_eq!(
            snap_position(b(-1500, 40, 400, 300), work, &[b(-1093, 80, 300, 500)], 12),
            Some((-1493, 40))
        );
        assert_eq!(
            snap_position(b(-1500, 40, 400, 300), work, &[b(-1093, 500, 300, 100)], 12),
            Some((-1500, 40))
        );
        assert_eq!(
            snap_position(
                b(-1000, 40, 400, 300),
                work,
                &[b(-1004, 60, 300, 400), b(-998, 60, 300, 400)],
                12
            ),
            Some((-998, 40))
        );
    }
    #[test]
    fn recover_oversized_offscreen_and_overflowing_spans() {
        assert_eq!(
            recover_position(b(5000, 6000, 400, 300), b(-1000, 0, 1000, 700)),
            Some((-400, 400))
        );
        assert_eq!(
            recover_position(b(-4000, -2000, 2000, 1000), b(0, 0, 800, 600)),
            Some((0, 0))
        );
        let extreme = Bounds {
            left: i32::MIN,
            top: i32::MIN,
            right: i32::MAX,
            bottom: i32::MAX,
        };
        assert_eq!(
            recover_position(extreme, b(-100, 10, 800, 600)),
            Some((-100, 10))
        );
        assert_eq!(snap_position(b(0, 0, 0, 50), extreme, &[], 12), None);
        assert_eq!(snap_position(b(20, 20, 40, 50), extreme, &[], -1), None);
    }
}
