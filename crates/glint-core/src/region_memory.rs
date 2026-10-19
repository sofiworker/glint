//! Session-only geometry for repeat capture. Contains no pixels or annotations.
use crate::Rect;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DisplayGeometry {
    pub bounds: [i32; 4],
    pub dpi: (u32, u32),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureLayout {
    pub origin: (i32, i32),
    pub size: (u32, u32),
    displays: Vec<DisplayGeometry>,
}
impl CaptureLayout {
    pub fn new(
        origin: (i32, i32),
        size: (u32, u32),
        mut displays: Vec<DisplayGeometry>,
    ) -> Option<Self> {
        if displays.is_empty()
            || size.0 == 0
            || size.1 == 0
            || displays.iter().any(|d| {
                d.bounds[2] <= d.bounds[0]
                    || d.bounds[3] <= d.bounds[1]
                    || d.dpi.0 == 0
                    || d.dpi.1 == 0
            })
        {
            return None;
        }
        displays.sort();
        let left = displays.iter().map(|d| d.bounds[0]).min()?;
        let top = displays.iter().map(|d| d.bounds[1]).min()?;
        let right = displays.iter().map(|d| d.bounds[2]).max()?;
        let bottom = displays.iter().map(|d| d.bounds[3]).max()?;
        if (left, top) != origin
            || i64::from(right) - i64::from(left) != i64::from(size.0)
            || i64::from(bottom) - i64::from(top) != i64::from(size.1)
        {
            return None;
        }
        Some(Self {
            origin,
            size,
            displays,
        })
    }
    fn contains(&self, r: Rect) -> bool {
        r.width > 0
            && r.height > 0
            && r.x.checked_add(r.width).is_some_and(|x| x <= self.size.0)
            && r.y.checked_add(r.height).is_some_and(|y| y <= self.size.1)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RememberedRegion {
    layout: CaptureLayout,
    region: Rect,
}
impl RememberedRegion {
    pub fn new(layout: CaptureLayout, region: Rect) -> Option<Self> {
        layout.contains(region).then_some(Self { layout, region })
    }
    pub fn recall(&self, layout: &CaptureLayout) -> Option<Rect> {
        (self.layout == *layout && layout.contains(self.region)).then_some(self.region)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn layout() -> CaptureLayout {
        CaptureLayout::new(
            (-1000, -200),
            (2000, 1000),
            vec![
                DisplayGeometry {
                    bounds: [-1000, -200, 0, 800],
                    dpi: (96, 96),
                },
                DisplayGeometry {
                    bounds: [0, -200, 1000, 800],
                    dpi: (144, 144),
                },
            ],
        )
        .unwrap()
    }
    #[test]
    fn mixed_dpi_negative_origin_order_and_changed_mapping() {
        let key = layout();
        let r = Rect {
            x: 800,
            y: 30,
            width: 400,
            height: 200,
        };
        let remembered = RememberedRegion::new(key.clone(), r).unwrap();
        let mut reordered = key.clone();
        reordered.displays.reverse();
        let reordered =
            CaptureLayout::new(reordered.origin, reordered.size, reordered.displays).unwrap();
        assert_eq!(remembered.recall(&reordered), Some(r));
        let mut changed = key.clone();
        changed.displays[1].dpi = (96, 96);
        assert_eq!(remembered.recall(&changed), None);
        let mut changed = key;
        changed.origin.0 = 0;
        assert_eq!(remembered.recall(&changed), None);
    }
    #[test]
    fn rejects_empty_overflowing_regions_and_inconsistent_layouts() {
        for r in [
            Rect::default(),
            Rect {
                x: u32::MAX,
                y: 0,
                width: 2,
                height: 2,
            },
            Rect {
                x: 1950,
                y: 0,
                width: 100,
                height: 10,
            },
        ] {
            assert!(RememberedRegion::new(layout(), r).is_none());
        }
        assert!(CaptureLayout::new((0, 0), (1, 1), vec![]).is_none());
        assert!(
            CaptureLayout::new(
                (0, 0),
                (100, 100),
                vec![DisplayGeometry {
                    bounds: [0, 0, 50, 100],
                    dpi: (96, 96)
                }]
            )
            .is_none()
        );
    }
}
