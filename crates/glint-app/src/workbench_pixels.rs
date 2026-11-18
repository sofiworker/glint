//! Worker-only image processing shared by image preview and video export.
use glint_core::{Edits, Rect, workbench::State};
use image::{RgbaImage, imageops};

pub fn render(
    source: &[u8],
    width: u32,
    height: u32,
    state: &State,
    time: Option<f64>,
) -> Result<RgbaImage, String> {
    let edits = Edits::from_current(
        state
            .marks
            .iter()
            .filter(|m| time.is_none_or(|t| t >= m.from && t < m.to))
            .map(|m| m.annotation.as_ref().clone())
            .collect(),
    )
    .ok_or("Annotation budget exceeded")?;
    let t = state.transform;
    let bytes = glint_core::composite(
        source,
        width,
        height,
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        &edits,
    )
    .map_err(str::to_owned)?;
    let mut image = RgbaImage::from_raw(width, height, bytes).ok_or("Invalid source pixels")?;
    image = imageops::crop_imm(&image, t.crop.x, t.crop.y, t.crop.width, t.crop.height).to_image();
    image = match t.turns % 4 {
        1 => imageops::rotate90(&image),
        2 => imageops::rotate180(&image),
        3 => imageops::rotate270(&image),
        _ => image,
    };
    if t.flip {
        imageops::flip_horizontal_in_place(&mut image);
    }
    let (w, h) = t.dimensions();
    if image.dimensions() != (w, h) {
        image = imageops::resize(&image, w, h, imageops::FilterType::Triangle);
    }
    if (t.brightness, t.contrast, t.saturation) != (100, 100, 100) {
        for p in image.pixels_mut() {
            let rgb = [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])];
            let gray = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
            for (c, v) in rgb.into_iter().enumerate() {
                let saturated = gray + (v - gray) * f32::from(t.saturation) / 100.;
                p[c] = (((saturated - 127.5) * f32::from(t.contrast) / 100. + 127.5)
                    * f32::from(t.brightness)
                    / 100.)
                    .round()
                    .clamp(0., 255.) as u8;
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glint_core::{
        Redaction,
        workbench::{Editor, Mark},
    };
    use std::sync::Arc;
    #[test]
    fn export_flattens_redaction_rotates_and_preserves_the_source() {
        let source = [200, 90, 20, 255].repeat(8);
        let original = source.clone();
        let mut e = Editor::new(4, 2, vec![], 0.).unwrap();
        let mut s = e.state.clone();
        s.marks.push(Mark {
            annotation: Arc::new(
                Redaction {
                    bounds: Rect {
                        x: 1,
                        y: 0,
                        width: 1,
                        height: 2,
                    },
                    color: [0, 0, 0, 255],
                }
                .into(),
            ),
            from: 0.,
            to: 0.,
        });
        s.transform.crop = Rect {
            x: 1,
            y: 0,
            width: 2,
            height: 2,
        };
        s.transform.turns = 1;
        e.apply(s).unwrap();
        let image = render(&source, 4, 2, &e.state, None).unwrap();
        assert_eq!(image.dimensions(), (2, 2));
        assert_eq!(&image.as_raw()[..8], &[0, 0, 0, 255, 0, 0, 0, 255]);
        assert_eq!(source, original);
        e.undo();
        assert_eq!(
            render(&source, 4, 2, &e.state, None).unwrap().into_raw(),
            source
        );
    }
}
