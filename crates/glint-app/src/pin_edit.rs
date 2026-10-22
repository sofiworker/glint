//! Build an image-only annotation canvas on the bounded background worker.
use glint_core::{Edits, ImageDocument, Rect};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::sync::Arc;
pub struct Source {
    pub pixels: SharedPixelBuffer<Rgba8Pixel>,
    pub document: Option<Arc<ImageDocument>>,
    pub work: (i32, i32, u32, u32),
    pub scale: f32,
    pub turns: i32,
}
pub struct Prepared {
    pub frame: glint_platform::DesktopFrame,
    pub selection: Rect,
    pub edits: Edits,
    pub window_size: (u32, u32),
    pub turns: i32,
}
pub enum Input {
    Desktop(glint_platform::DesktopSnapshot),
    Pin(Prepared),
    Local(Prepared),
    Scroll(Prepared),
}
impl Source {
    pub fn prepare(self) -> Result<Prepared, String> {
        let doc = match self.document {
            Some(doc) => doc,
            None => Arc::new(
                ImageDocument::from_selection(
                    self.pixels.as_bytes(),
                    self.pixels.width(),
                    self.pixels.height(),
                    Rect {
                        x: 0,
                        y: 0,
                        width: self.pixels.width(),
                        height: self.pixels.height(),
                    },
                    &Edits::default(),
                )
                .map_err(str::to_owned)?,
            ),
        };
        let (w, h) = doc.dimensions();
        let (x, y, ww, wh) = self.work;
        let scale = self.scale.max(1.);
        if ww < 640 || wh < 480 {
            return Err("Work area is too small for the annotation toolbar".into());
        }
        let ratio = ((ww as f32 - 80. * scale).max(100.) / w as f32)
            .min((wh as f32 - 350. * scale).max(100.) / h as f32)
            .min(4.);
        let (cw, ch) = (
            (ww as f32 / ratio).ceil() as u32,
            (wh as f32 / ratio).ceil() as u32,
        );
        if cw < w || ch < h {
            return Err("Invalid annotation canvas dimensions".into());
        }
        let (left, top) = ((cw - w) / 2, (72. * scale / ratio).ceil() as u32);
        if top.checked_add(h).is_none_or(|bottom| bottom > ch) {
            return Err("Image does not fit annotation canvas".into());
        }
        let length = (cw as usize)
            .checked_mul(ch as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n <= 128 * 1024 * 1024)
            .ok_or("Annotation canvas exceeds 128 MiB")?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length).map_err(|e| e.to_string())?;
        rgba.resize(length, 0);
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[24, 32, 44, 255]);
        }
        for row in 0..h as usize {
            let target = ((top as usize + row) * cw as usize + left as usize) * 4;
            let from = row * w as usize * 4;
            rgba[target..target + w as usize * 4]
                .copy_from_slice(&doc.source()[from..from + w as usize * 4]);
        }
        Ok(Prepared {
            frame: glint_platform::DesktopFrame {
                x,
                y,
                width: cw,
                height: ch,
                rgba,
            },
            selection: Rect {
                x: left,
                y: top,
                width: w,
                height: h,
            },
            edits: doc.edits().clone(),
            window_size: (ww, wh),
            turns: self.turns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canvas_preserves_source_pixels_objects_and_negative_work_origin() {
        let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            &[61u8, 72, 83, 255].repeat(80 * 60),
            80,
            60,
        );
        let mut edits = Edits::default();
        edits.push(glint_core::Redaction {
            bounds: Rect {
                x: 10,
                y: 10,
                width: 20,
                height: 15,
            },
            color: [0, 0, 0, 255],
        });
        let document = ImageDocument::from_selection(
            pixels.as_bytes(),
            80,
            60,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 60,
            },
            &edits,
        )
        .unwrap();
        let prepared = Source {
            pixels,
            document: Some(Arc::new(document)),
            work: (-1600, -100, 1600, 900),
            scale: 1.,
            turns: 3,
        }
        .prepare()
        .unwrap();
        assert_eq!((prepared.frame.x, prepared.frame.y), (-1600, -100));
        assert_eq!(
            (prepared.selection.width, prepared.selection.height),
            (80, 60)
        );
        let restored = ImageDocument::from_selection(
            &prepared.frame.rgba,
            prepared.frame.width,
            prepared.frame.height,
            prepared.selection,
            &prepared.edits,
        )
        .unwrap();
        assert_eq!(restored.source(), [61u8, 72, 83, 255].repeat(80 * 60));
        assert!(Arc::ptr_eq(&restored.edits().marks()[0], &edits.marks()[0]));
        assert_eq!(prepared.turns, 3);
    }
    #[test]
    fn extreme_canvas_is_rejected_before_allocation_and_plain_image_has_no_objects() {
        let pixels = SharedPixelBuffer::<Rgba8Pixel>::new(32_000, 1);
        assert!(
            Source {
                pixels,
                document: None,
                work: (0, 0, 1600, 900),
                scale: 1.,
                turns: 0
            }
            .prepare()
            .is_err()
        );
        let pixels = SharedPixelBuffer::<Rgba8Pixel>::new(2, 3);
        let prepared = Source {
            pixels,
            document: None,
            work: (0, 0, 1600, 900),
            scale: 1.,
            turns: 0,
        }
        .prepare()
        .unwrap();
        assert!(prepared.edits.marks().is_empty());
        assert_eq!(
            (prepared.selection.width, prepared.selection.height),
            (2, 3)
        );
    }
}
