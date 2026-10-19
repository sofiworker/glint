//! Platform-independent preferences, source-pixel geometry and non-destructive edits.
mod annotations;
mod document;
mod edits;
pub use document::ImageDocument;
mod objects;
pub mod pin_session;
mod resize;
pub use edits::Edits;
pub use resize::ResizeHandle;
mod color;
mod magnifier;
pub use magnifier::{MAGNIFIER_SIDE, magnifier_rgba};
mod text;
pub use color::sample_rgba;
pub use text::{GlyphMask, TextAnnotation};
mod selection;
pub use selection::{
    Edges, SelectionGesture, SelectionHit, desktop_region, first_region_at, selection_hit,
};
pub mod config;
pub mod config_transfer;
pub mod exclusions;
pub mod output;
pub mod pin_geometry;
pub mod pins;
pub mod region_memory;
pub mod screen_capture;
pub use annotations::{
    ANNOTATION_TILE_SIZE, Annotation, Pixelation, Point, Redaction, SceneSource, Shape, ShapeKind,
    Stroke, render_annotations, render_scene,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn intersection(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Self {
            x,
            y,
            width: self
                .x
                .saturating_add(self.width)
                .min(other.x.saturating_add(other.width))
                .saturating_sub(x),
            height: self
                .y
                .saturating_add(self.height)
                .min(other.y.saturating_add(other.height))
                .saturating_sub(y),
        }
    }
    pub fn union(self, other: Self) -> Self {
        if self.width == 0 || self.height == 0 {
            return other;
        }
        if other.width == 0 || other.height == 0 {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: self
                .x
                .saturating_add(self.width)
                .max(other.x.saturating_add(other.width))
                .saturating_sub(x),
            height: self
                .y
                .saturating_add(self.height)
                .max(other.y.saturating_add(other.height))
                .saturating_sub(y),
        }
    }
    pub fn between(a: (u32, u32), b: (u32, u32)) -> Self {
        Self {
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            width: a.0.abs_diff(b.0),
            height: a.1.abs_diff(b.1),
        }
    }
    pub fn clipped(self, width: u32, height: u32) -> Self {
        let x = self.x.min(width);
        let y = self.y.min(height);
        Self {
            x,
            y,
            width: self.width.min(width - x),
            height: self.height.min(height - y),
        }
    }
}

/// Composites annotations in object order; output contains flattened RGBA pixels only.
pub fn composite(
    source: &[u8],
    width: u32,
    height: u32,
    crop: Rect,
    edits: &Edits,
) -> Result<Vec<u8>, &'static str> {
    composite_impl(source, width, height, crop, edits, false)
}

/// Composite selection-local annotations over the requested source crop. Moving a selection
/// changes source pixels, while its annotation objects and canonical raster grid stay unchanged.
pub fn composite_selection(
    source: &[u8],
    width: u32,
    height: u32,
    crop: Rect,
    edits: &Edits,
) -> Result<Vec<u8>, &'static str> {
    composite_impl(source, width, height, crop, edits, true)
}
fn composite_impl(
    source: &[u8],
    width: u32,
    height: u32,
    crop: Rect,
    edits: &Edits,
    local: bool,
) -> Result<Vec<u8>, &'static str> {
    if (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        != Some(source.len())
    {
        return Err("Invalid RGBA buffer length");
    }
    let crop = crop.clipped(width, height);
    if crop.width == 0 || crop.height == 0 {
        return Err("Empty selection");
    }
    let layer_crop = if local {
        Rect { x: 0, y: 0, ..crop }
    } else {
        crop
    };
    if edits
        .marks()
        .iter()
        .any(|m| matches!(m.as_ref(), Annotation::Pixelation(_)))
    {
        let mut pixels = render_scene(
            layer_crop,
            edits.marks().iter().map(|m| m.as_ref()),
            SceneSource {
                rgba: source,
                width,
                height,
                origin: if local { (crop.x, crop.y) } else { (0, 0) },
            },
        )?;
        for pixel in pixels.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            if alpha > 0 && alpha < 255 {
                for value in &mut pixel[..3] {
                    *value = ((u32::from(*value) * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        return Ok(pixels);
    }
    let mut output =
        Vec::with_capacity((u64::from(crop.width) * u64::from(crop.height) * 4) as usize);
    for y in crop.y..crop.y + crop.height {
        let offset = ((u64::from(y) * u64::from(width) + u64::from(crop.x)) * 4) as usize;
        output.extend_from_slice(&source[offset..offset + crop.width as usize * 4]);
    }
    let layer = render_annotations(layer_crop, edits.marks().iter().map(|mark| mark.as_ref()))?;
    for (target, over) in output
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(layer.as_chunks::<4>().0)
    {
        let alpha = u32::from(over[3]);
        if alpha == 0 {
            continue;
        }
        let source_alpha = u32::from(target[3]);
        let out_alpha = alpha + (source_alpha * (255 - alpha) + 127) / 255;
        for c in 0..3 {
            let source_premul = (u32::from(target[c]) * source_alpha + 127) / 255;
            let premul = u32::from(over[c]) + (source_premul * (255 - alpha) + 127) / 255;
            target[c] = ((premul * 255 + out_alpha / 2) / out_alpha).min(255) as u8;
        }
        target[3] = out_alpha as u8;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crop_redaction_and_undo_redo_agree() {
        let source = vec![250; 64];
        let crop = Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        };
        let mut edits = Edits::default();
        edits.push(Redaction {
            bounds: Rect {
                x: 2,
                y: 0,
                width: 9,
                height: 9,
            },
            color: [1, 2, 3, 0],
        });
        let redacted = composite(&source, 4, 4, crop, &edits).unwrap();
        assert_eq!(&redacted[4..8], &[1, 2, 3, 255]);
        assert_eq!(&redacted[0..4], &[250; 4]);
        edits.undo();
        assert_eq!(
            composite(&source, 4, 4, crop, &edits).unwrap(),
            vec![250; 16]
        );
        edits.redo();
        assert_eq!(composite(&source, 4, 4, crop, &edits).unwrap(), redacted);
        assert_eq!(source, vec![250; 64]);
        edits.undo();
        edits.push(Redaction {
            bounds: crop,
            color: [0; 4],
        });
        edits.redo();
        assert_eq!(edits.marks().len(), 1);
    }
    #[test]
    fn rejects_invalid_buffers_and_empty_crop() {
        assert!(composite(&[], u32::MAX, u32::MAX, Rect::default(), &Edits::default()).is_err());
        assert!(composite(&[], 5, 5, Rect::default(), &Edits::default()).is_err());
        assert!(
            composite(
                &[0; 16],
                2,
                2,
                Rect {
                    x: u32::MAX,
                    y: 0,
                    width: 5,
                    height: 2
                },
                &Edits::default()
            )
            .is_err()
        );
        assert_eq!(
            Rect::between((9, 8), (2, 3)),
            Rect {
                x: 2,
                y: 3,
                width: 7,
                height: 5
            }
        );
    }
}
