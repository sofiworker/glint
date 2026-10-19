//! Samples only on input and only rebuilds the small image when its source pixel changes.
use super::CaptureProbe;
use glint_core::{MAGNIFIER_SIDE, magnifier_rgba};
use slint::{Rgba8Pixel, SharedPixelBuffer};

pub fn update(
    ui: &CaptureProbe,
    frame: &glint_platform::DesktopFrame,
    cached: &mut Option<(u32, u32)>,
    x: f32,
    y: f32,
) {
    if !ui.get_magnifier_enabled()
        || !x.is_finite()
        || !y.is_finite()
        || x < 0.0
        || y < 0.0
        || x >= frame.width as f32
        || y >= frame.height as f32
        || (ui.get_local_image()
            && (x < ui.get_left()
                || y < ui.get_top()
                || x >= ui.get_left() + ui.get_wide()
                || y >= ui.get_top() + ui.get_high()))
    {
        ui.set_loupe_valid(false);
        return;
    }
    let point = (x as u32, y as u32);
    ui.set_loupe_source_x(x);
    ui.set_loupe_source_y(y);
    if *cached != Some(point) {
        let Some(pixels) = magnifier_rgba(&frame.rgba, frame.width, frame.height, point.0, point.1)
        else {
            ui.set_loupe_valid(false);
            return;
        };
        let c = &pixels[240..244];
        ui.set_loupe_hex(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]).into());
        ui.set_loupe_position(
            format!(
                "({}, {})",
                if ui.get_local_image() {
                    i64::from(point.0) - ui.get_left() as i64
                } else {
                    i64::from(frame.x) + i64::from(point.0)
                },
                if ui.get_local_image() {
                    i64::from(point.1) - ui.get_top() as i64
                } else {
                    i64::from(frame.y) + i64::from(point.1)
                }
            )
            .into(),
        );
        ui.set_loupe_image(slint::Image::from_rgba8(
            SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                &pixels,
                MAGNIFIER_SIDE,
                MAGNIFIER_SIDE,
            ),
        ));
        *cached = Some(point);
    }
    ui.set_loupe_valid(true);
}
