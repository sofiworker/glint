//! In-memory editable image, independent of UI and platform handles.
use crate::{Edits, Rect, composite_selection};
use std::sync::Arc;

#[derive(Clone)]
pub struct ImageDocument {
    width: u32,
    height: u32,
    source: Arc<[u8]>,
    edits: Edits,
}
impl ImageDocument {
    /// Retain only the selected raw source pixels. Annotation coordinates are already
    /// selection-local; retaining the whole desktop would retain unrelated content.
    pub fn from_selection(
        source: &[u8],
        width: u32,
        height: u32,
        crop: Rect,
        edits: &Edits,
    ) -> Result<Self, &'static str> {
        if crop.width == 0 || crop.height == 0 || crop.clipped(width, height) != crop {
            return Err("Invalid document crop");
        }
        let source = composite_selection(source, width, height, crop, &Edits::default())?;
        Ok(Self {
            width: crop.width,
            height: crop.height,
            source: source.into(),
            edits: edits.clone(),
        })
    }
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn source(&self) -> &[u8] {
        &self.source
    }
    pub fn edits(&self) -> &Edits {
        &self.edits
    }
    /// Conservative retained data accounting; shared versions are counted once per document.
    /// This estimate is not process RSS or a serialization format.
    pub fn retained_bytes(&self) -> u64 {
        self.source.len() as u64 + self.edits.retained_bytes() + std::mem::size_of::<Self>() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Redaction;
    #[test]
    fn selection_keeps_raw_pixels_local_objects_and_independent_undo() {
        let source: Vec<u8> = (0..12).flat_map(|n| [n, 10, 20, 255]).collect();
        let crop = Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        };
        let mut edits = Edits::default();
        edits.push(Redaction {
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 2,
            },
            color: [0, 0, 0, 255],
        });
        let document = ImageDocument::from_selection(&source, 4, 3, crop, &edits).unwrap();
        assert_eq!(document.dimensions(), (2, 2));
        assert_eq!(
            document.source(),
            &[
                5, 10, 20, 255, 6, 10, 20, 255, 9, 10, 20, 255, 10, 10, 20, 255
            ]
        );
        assert!(std::sync::Arc::ptr_eq(
            &edits.marks()[0],
            &document.edits().marks()[0]
        ));
        let mut reopened = document.edits().clone();
        reopened.remove(0).unwrap();
        let entire = Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };
        assert_eq!(
            composite_selection(document.source(), 2, 2, entire, &reopened).unwrap(),
            document.source()
        );
        reopened.undo();
        let flat = composite_selection(document.source(), 2, 2, entire, &reopened).unwrap();
        assert_eq!(&flat[..4], &[0, 0, 0, 255]);
        assert_eq!(edits.marks().len(), 1);
        assert_eq!(document.edits().marks().len(), 1);
        assert!(document.retained_bytes() > document.source().len() as u64);
        assert!(
            ImageDocument::from_selection(
                &source,
                4,
                3,
                Rect {
                    x: 3,
                    y: 1,
                    width: 2,
                    height: 2
                },
                &edits
            )
            .is_err()
        );
    }
}
