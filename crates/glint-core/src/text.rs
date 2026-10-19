use crate::Rect;
use std::sync::Arc;

/// Immutable grayscale coverage, prepared by the platform text shaper/rasterizer once.
/// Kept with editable text/style metadata so UI and export use identical glyph pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphMask {
    pub width: u32,
    pub height: u32,
    pub coverage: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextAnnotation {
    pub origin: (u32, u32),
    pub text: String,
    pub font: String,
    pub size: u16,
    pub color: [u8; 4],
    pub mask: GlyphMask,
}
impl TextAnnotation {
    pub fn is_valid(&self) -> bool {
        !self.text.trim().is_empty()
            && self.text.chars().count() <= 80
            && !self.text.chars().any(char::is_control)
            && !self.font.trim().is_empty()
            && self.font.len() <= 128
            && (12..=48).contains(&self.size)
            && self.mask.width > 0
            && self.mask.height > 0
            && self.mask.coverage.len() <= 4 * 1024 * 1024
            && (self.mask.width as usize).checked_mul(self.mask.height as usize)
                == Some(self.mask.coverage.len())
            && self.origin.0.checked_add(self.mask.width).is_some()
            && self.origin.1.checked_add(self.mask.height).is_some()
    }
    pub fn bounds(&self) -> Rect {
        Rect {
            x: self.origin.0,
            y: self.origin.1,
            width: self.mask.width,
            height: self.mask.height,
        }
    }
}
