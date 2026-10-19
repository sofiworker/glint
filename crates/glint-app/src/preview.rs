//! Sparse annotation-coordinate pixel tiles; mouse motion never copies the desktop source image.
use super::LayerTile;
use glint_core::{Annotation, Edits, Rect};
use slint::{Model, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{collections::HashMap, rc::Rc, time::Instant};

pub const TILE: u32 = glint_core::ANNOTATION_TILE_SIZE;
pub const BUDGET_ERROR: &str = "Preview tile budget reached; current edits are retained";
pub struct Preview {
    pub model: Rc<VecModel<LayerTile>>,
    rows: HashMap<(u32, u32), usize>,
    free_rows: Vec<usize>,
    pub updated_bytes: u64,
    pub update_count: u64,
    pub raster_ms: f64,
    budget_bytes: usize,
}

impl Default for Preview {
    fn default() -> Self {
        Self::with_budget_mib(128)
    }
}

impl Preview {
    pub fn with_budget_mib(mib: u16) -> Self {
        Self {
            model: Default::default(),
            rows: Default::default(),
            free_rows: Vec::new(),
            updated_bytes: 0,
            update_count: 0,
            raster_ms: 0.,
            budget_bytes: usize::from(mib) * 1024 * 1024,
        }
    }

    /// Release all tiles without discarding this editor's saved budget snapshot.
    pub fn clear(&mut self) {
        self.model.set_vec(Vec::new());
        self.rows.clear();
        self.free_rows.clear();
    }

    #[cfg(test)]
    pub fn refresh(
        &mut self,
        edits: &Edits,
        draft: Option<&Annotation>,
        damage: Rect,
        dimensions: (u32, u32),
    ) -> Result<(), &'static str> {
        self.refresh_scene(edits, draft, damage, dimensions, None)
    }

    pub fn refresh_scene(
        &mut self,
        edits: &Edits,
        draft: Option<&Annotation>,
        damage: Rect,
        dimensions: (u32, u32),
        source: Option<glint_core::SceneSource<'_>>,
    ) -> Result<(), &'static str> {
        // A zero-length drag is not yet a valid shape. It must not pause output
        // or erase existing pixels while the pointer waits for its first movement.
        let draft = draft.filter(|mark| mark.is_valid());
        let damage = damage.clipped(dimensions.0, dimensions.1);
        if damage.width == 0 || damage.height == 0 {
            return Ok(());
        }
        let begin = Instant::now();
        let bounds: Vec<_> = edits
            .marks()
            .iter()
            .map(|m| m.bounds())
            .chain(draft.map(Annotation::bounds))
            .collect();
        let mut dirty = Vec::new();
        for ty in damage.y / TILE..=(damage.y + damage.height - 1) / TILE {
            for tx in damage.x / TILE..=(damage.x + damage.width - 1) / TILE {
                let crop = Rect {
                    x: tx * TILE,
                    y: ty * TILE,
                    width: TILE,
                    height: TILE,
                }
                .clipped(dimensions.0, dimensions.1);
                let active = bounds.iter().any(|bounds| {
                    let overlap = bounds.intersection(crop);
                    overlap.width > 0 && overlap.height > 0
                });
                dirty.push(((tx, ty), crop, active));
            }
        }
        // Preflight the whole update before mutating the visible model. Undo returns rows
        // to the pool so editing another region cannot consume an ever-growing cache.
        let additions = dirty
            .iter()
            .filter(|(key, _, active)| *active && !self.rows.contains_key(key))
            .count();
        let removals = dirty
            .iter()
            .filter(|(key, _, active)| !*active && self.rows.contains_key(key))
            .count();
        if self.rows.len() + additions - removals > self.budget_bytes / (TILE * TILE * 4) as usize {
            return Err(BUDGET_ERROR);
        }
        for (key, _, active) in &dirty {
            if !active && let Some(row) = self.rows.remove(key) {
                self.model.set_row_data(row, LayerTile::default());
                self.free_rows.push(row);
            }
        }
        for (key, crop, active) in dirty {
            if !active {
                continue;
            }
            let marks = edits.marks().iter().map(|m| m.as_ref()).chain(draft);
            let bytes = if let Some(source) = source {
                glint_core::render_scene(crop, marks, source)?
            } else {
                glint_core::render_annotations(crop, marks)?
            };
            let picture = if bytes.iter().all(|b| *b == 0) {
                slint::Image::default()
            } else {
                slint::Image::from_rgba8_premultiplied(
                    SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        &bytes,
                        crop.width,
                        crop.height,
                    ),
                )
            };
            let tile = LayerTile {
                x: crop.x as f32,
                y: crop.y as f32,
                w: crop.width as f32,
                h: crop.height as f32,
                picture,
            };
            if let Some(row) = self.rows.get(&key) {
                self.model.set_row_data(*row, tile);
            } else if let Some(row) = self.free_rows.pop() {
                self.model.set_row_data(row, tile);
                self.rows.insert(key, row);
            } else {
                let row = self.model.row_count();
                self.model.push(tile);
                self.rows.insert(key, row);
            }
            self.updated_bytes += u64::from(crop.width) * u64::from(crop.height) * 4;
            self.update_count += 1;
        }
        self.raster_ms += begin.elapsed().as_secs_f64() * 1000.0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glint_core::Redaction;

    #[test]
    fn oversized_update_keeps_previous_preview_unchanged() {
        let mut preview = Preview::default();
        let mut edits = Edits::default();
        let small = Rect {
            x: 0,
            y: 0,
            width: 16,
            height: 16,
        };
        assert!(edits.push(Redaction {
            bounds: small,
            color: [0, 0, 0, 255]
        }));
        preview.refresh(&edits, None, small, (262272, 128)).unwrap();
        let large = Rect {
            x: 0,
            y: 0,
            width: 262272,
            height: 128,
        };
        let draft = Annotation::from(Redaction {
            bounds: large,
            color: [255, 0, 0, 255],
        });
        assert!(
            preview
                .refresh(&edits, Some(&draft), large, (262272, 128))
                .is_err()
        );
        assert_eq!(preview.model.row_count(), 1);
        assert_eq!(preview.rows.len(), 1);
        assert_eq!(preview.update_count, 1);
    }

    #[test]
    fn undo_releases_pixels_and_reuses_rows_in_other_regions() {
        let mut preview = Preview::default();
        let mut edits = Edits::default();
        for x in [0, 128, 256, 384] {
            let bounds = Rect {
                x,
                y: 0,
                width: 16,
                height: 16,
            };
            assert!(edits.push(Redaction {
                bounds,
                color: [0, 0, 0, 255]
            }));
            preview.refresh(&edits, None, bounds, (512, 128)).unwrap();
            assert_eq!(preview.model.row_count(), 1);
            assert_eq!(preview.rows.len(), 1);
            let damage = edits.undo();
            preview.refresh(&edits, None, damage, (512, 128)).unwrap();
            assert!(preview.rows.is_empty());
            assert_eq!(preview.model.row_data(0).unwrap().picture.size().width, 0);
        }
    }

    #[test]
    fn configured_budget_is_atomic_and_survives_clear_and_undo() {
        // Small internal limit exercises the same tile preflight without a large allocation.
        let mut preview = Preview::with_budget_mib(1);
        let dimensions = (TILE * 17, TILE);
        let at_limit = Rect {
            x: 0,
            y: 0,
            width: TILE * 16,
            height: TILE,
        };
        let full = Rect {
            width: dimensions.0,
            ..at_limit
        };
        let mut edits = Edits::default();
        assert!(edits.push(Redaction {
            bounds: at_limit,
            color: [17, 29, 53, 255]
        }));
        preview.refresh(&edits, None, at_limit, dimensions).unwrap();
        assert_eq!(preview.rows.len(), 16);
        let count = preview.update_count;
        let before = preview
            .model
            .row_data(0)
            .unwrap()
            .picture
            .to_rgba8()
            .unwrap();
        assert!(edits.push(Redaction {
            bounds: full,
            color: [255, 0, 0, 255]
        }));
        assert_eq!(
            preview.refresh(&edits, None, full, dimensions),
            Err(BUDGET_ERROR)
        );
        assert_eq!(preview.update_count, count);
        assert_eq!(preview.rows.len(), 16);
        assert_eq!(
            preview
                .model
                .row_data(0)
                .unwrap()
                .picture
                .to_rgba8()
                .unwrap()
                .as_bytes(),
            before.as_bytes()
        );
        let damage = edits.undo();
        preview.refresh(&edits, None, damage, dimensions).unwrap();
        assert_eq!(preview.rows.len(), 16);
        preview.clear();
        assert_eq!(preview.model.row_count(), 0);
        assert_eq!(
            preview.refresh(
                &edits,
                Some(&Annotation::from(Redaction {
                    bounds: full,
                    color: [255; 4]
                })),
                full,
                dimensions
            ),
            Err(BUDGET_ERROR)
        );
        assert_eq!(preview.model.row_count(), 0);
    }

    #[test]
    fn each_supported_budget_rejects_overflow_before_rasterizing() {
        for mib in [64, 128, 512] {
            let mut preview = Preview::with_budget_mib(mib);
            let tiles = usize::from(mib) * 1024 * 1024 / (TILE * TILE * 4) as usize;
            let bounds = Rect {
                x: 0,
                y: 0,
                width: (tiles as u32 + 1) * TILE,
                height: TILE,
            };
            let draft = Annotation::from(Redaction {
                bounds,
                color: [255; 4],
            });
            assert_eq!(
                preview.refresh(
                    &Edits::default(),
                    Some(&draft),
                    bounds,
                    (bounds.width, bounds.height)
                ),
                Err(BUDGET_ERROR)
            );
            assert_eq!(preview.update_count, 0);
            assert_eq!(preview.model.row_count(), 0);
        }
    }
}
