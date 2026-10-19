//! Initial selection for a frozen physical-pixel desktop; no logical-DPI scaling.
use crate::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenTarget {
    All,
    Current,
}

/// Current means the display containing the sampled pointer. Missing/stale geometry
/// or a pointer in a desktop gap returns None, so the UI can offer manual selection.
pub fn screen_region(
    target: ScreenTarget,
    origin: (i32, i32),
    size: (u32, u32),
    displays: &[[i32; 4]],
    pointer: Option<(i32, i32)>,
) -> Option<Rect> {
    if size.0 == 0 || size.1 == 0 {
        return None;
    }
    if target == ScreenTarget::All {
        return Some(Rect {
            x: 0,
            y: 0,
            width: size.0,
            height: size.1,
        });
    }
    let (px, py) = pointer?;
    let &[left, top, right, bottom] = displays
        .iter()
        .find(|b| b[0] <= px && px < b[2] && b[1] <= py && py < b[3])?;
    let x = i64::from(left) - i64::from(origin.0);
    let y = i64::from(top) - i64::from(origin.1);
    let width = i64::from(right) - i64::from(left);
    let height = i64::from(bottom) - i64::from(top);
    if x < 0
        || y < 0
        || width <= 0
        || height <= 0
        || x + width > i64::from(size.0)
        || y + height > i64::from(size.1)
    {
        return None;
    }
    Some(Rect {
        x: x as u32,
        y: y as u32,
        width: width as u32,
        height: height as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_origins_offset_monitors_and_exclusive_edges_use_source_pixels() {
        let displays = [[-1920, -300, 0, 780], [0, 0, 2560, 1440]];
        let region = |point| {
            screen_region(
                ScreenTarget::Current,
                (-1920, -300),
                (4480, 1740),
                &displays,
                point,
            )
        };
        assert_eq!(
            region(Some((-1, 0))),
            Some(Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080
            })
        );
        assert_eq!(
            region(Some((0, 0))),
            Some(Rect {
                x: 1920,
                y: 300,
                width: 2560,
                height: 1440
            })
        );
        assert_eq!(
            region(Some((2559, 1439))),
            Some(Rect {
                x: 1920,
                y: 300,
                width: 2560,
                height: 1440
            })
        );
        for point in [
            None,
            Some((0, -1)),
            Some((-1921, 0)),
            Some((2560, 0)),
            Some((0, 1440)),
        ] {
            assert!(region(point).is_none());
        }
        assert_eq!(
            screen_region(
                ScreenTarget::All,
                (-1920, -300),
                (4480, 1740),
                &displays,
                None
            ),
            Some(Rect {
                x: 0,
                y: 0,
                width: 4480,
                height: 1740
            })
        );
    }
    #[test]
    fn stale_geometry_does_not_clip_or_invent_a_display() {
        for displays in [
            vec![],
            vec![[0, 0, 0, 100]],
            vec![[-1, 0, 100, 100]],
            vec![[0, 0, 101, 100]],
            vec![[0, 0, 100, 101]],
        ] {
            assert!(
                screen_region(
                    ScreenTarget::Current,
                    (0, 0),
                    (100, 100),
                    &displays,
                    Some((0, 0))
                )
                .is_none()
            );
        }
        assert!(screen_region(ScreenTarget::All, (0, 0), (0, 100), &[], None).is_none());
        assert_eq!(
            screen_region(
                ScreenTarget::Current,
                (i32::MIN, i32::MIN),
                (u32::MAX, u32::MAX),
                &[[i32::MIN, i32::MIN, i32::MAX, i32::MAX]],
                Some((0, 0))
            ),
            Some(Rect {
                x: 0,
                y: 0,
                width: u32::MAX,
                height: u32::MAX
            })
        );
    }
}
