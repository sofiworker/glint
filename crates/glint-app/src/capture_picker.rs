//! O(1) frozen-source sampling and bounded asynchronous text clipboard writes.
use super::{CaptureProbe, Message, OUTPUT_BUSY, OutputSlot, text};
use glint_core::Rect;
use slint::ComponentHandle;

/// Only a completed, uncancelled gesture that starts and ends on a source
/// pixel can publish. Keyboard/button confirmation uses `copy` directly.
pub fn pointer(
    ui: &CaptureProbe,
    frame: &glint_platform::DesktopFrame,
    region: Rect,
    pressed: &mut bool,
    kind: i32,
    point: (f32, f32),
) {
    preview(ui, frame, region, point.0, point.1);
    if kind == 0 {
        *pressed = ui.get_sample_valid();
    } else if kind == 2 {
        let confirmed = std::mem::take(pressed);
        if confirmed && ui.get_sample_valid() {
            copy(ui, false);
        }
    }
}

pub fn preview(
    ui: &CaptureProbe,
    frame: &glint_platform::DesktopFrame,
    region: Rect,
    x: f32,
    y: f32,
) {
    let valid = x.is_finite()
        && y.is_finite()
        && x >= region.x as f32
        && y >= region.y as f32
        && x < (u64::from(region.x) + u64::from(region.width)) as f32
        && y < (u64::from(region.y) + u64::from(region.height)) as f32;
    let color = valid
        .then(|| {
            glint_core::sample_rgba(
                &frame.rgba,
                (frame.width, frame.height),
                (x as u32, y as u32),
            )
        })
        .flatten();
    ui.set_sample_valid(color.is_some());
    if let Some(color) = color {
        let hex = format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
        // Avoid invalidating text/rendering when motion stays in the same source pixel.
        if ui.get_sample_hex() != hex {
            ui.set_sample_hex(hex.into());
        }
        let rgb = format!("RGB({}, {}, {})", color[0], color[1], color[2]);
        if ui.get_sample_rgb() != rgb {
            ui.set_sample_rgb(rgb.into());
        }
        ui.set_sample_color(slint::Color::from_rgb_u8(color[0], color[1], color[2]));
        let position = format!(
            "({}, {})",
            if ui.get_local_image() {
                x as i64 - region.x as i64
            } else {
                i64::from(frame.x) + x as i64
            },
            if ui.get_local_image() {
                y as i64 - region.y as i64
            } else {
                i64::from(frame.y) + y as i64
            }
        );
        if ui.get_sample_position() != position {
            ui.set_sample_position(position.into());
        }
    }
}

pub fn copy(ui: &CaptureProbe, rgb: bool) {
    if ui.get_picker_busy() || ui.get_busy() {
        return;
    }
    if !ui.get_sample_valid() {
        ui.set_picker_feedback(text(ui, Message::CaptureColorOutside).into());
        return;
    }
    if OUTPUT_BUSY.swap(true, std::sync::atomic::Ordering::AcqRel) {
        ui.set_picker_feedback(text(ui, Message::CaptureOutputPending).into());
        return;
    }
    let slot = OutputSlot;
    let value = if rgb {
        ui.get_sample_rgb()
    } else {
        ui.get_sample_hex()
    }
    .to_string();
    ui.set_picker_busy(true);
    ui.set_picker_feedback(text(ui, Message::CaptureColorCopying).into());
    let weak = ui.as_weak();
    let worker = std::thread::Builder::new()
        .name("glint-color-copy".into())
        .spawn(move || {
            let result = glint_platform::copy_text(value.clone());
            // Release before posting completion: the UI can retry as soon as it receives the result.
            drop(slot);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if ui.get_closed() {
                    return;
                }
                ui.set_picker_busy(false);
                let copied = result.is_ok();
                let feedback = match result {
                    Ok(()) => format!("{}: {value}", text(&ui, Message::CaptureColorCopied)),
                    Err(error) => {
                        format!("{}: {error}", text(&ui, Message::CaptureColorCopyFailed))
                    }
                };
                ui.set_picker_feedback(feedback.clone().into());
                ui.set_status(feedback.into());
                if copied && ui.get_color_only() {
                    ui.set_completion_kind(4);
                    ui.invoke_finished();
                }
            });
        });
    if let Err(error) = worker {
        ui.set_picker_busy(false);
        let feedback = format!("{}: {error}", text(ui, Message::CaptureColorCopyFailed));
        ui.set_picker_feedback(feedback.clone().into());
        ui.set_status(feedback.into());
    }
}
