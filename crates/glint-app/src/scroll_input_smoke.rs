//! Explicit diagnostic only: exercise selection and Start through Win32 input.
use crate::probe::CaptureProbe;
use glint_platform::input_probe::{InputProbe, Pointer};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::cell::RefCell;

thread_local! {
    static INPUT: RefCell<Option<(InputProbe, u8)>> = const { RefCell::new(None) };
}

pub fn click(ui: &impl ComponentHandle, horizontal: f32, vertical: f32) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let peer = ui
        .window()
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            match handle.as_raw() {
                RawWindowHandle::Win32(handle) => Ok::<usize, String>(handle.hwnd.get() as usize),
                _ => Err("Not a Windows controller".into()),
            }
        })
        .ok_or("Native controller missing")??;
    let size = ui.window().size();
    InputProbe::new()?.pointer(
        peer,
        (size.width as f32 * horizontal) as i32,
        (size.height as f32 * vertical) as i32,
        Pointer::Click,
        true,
    )
}

pub fn select(capture: &CaptureProbe, x: f32, y: f32, w: i32, h: i32) -> Result<bool, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Some(peer) = capture.window().with_winit_window(|window| {
        let handle = window.window_handle().map_err(|e| e.to_string())?;
        match handle.as_raw() {
            RawWindowHandle::Win32(handle) => Ok::<usize, String>(handle.hwnd.get() as usize),
            _ => Err("Not a Windows capture".into()),
        }
    }) else {
        return Ok(false);
    };
    let peer = peer?;
    let size = capture.window().size();
    let scale = capture.window().scale_factor();
    let sx = size.width as f32 / capture.get_source_w();
    let sy = size.height as f32 / capture.get_source_h();
    let mut observed_phase = 0;
    INPUT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some((InputProbe::new()?, 0));
        }
        let (input, phase) = slot.as_mut().unwrap();
        observed_phase = *phase;
        match *phase {
            0 => {
                // The visible mode bar is centered, with Scrolling in the middle.
                input.pointer(
                    peer,
                    (size.width / 2) as i32,
                    (34. * scale) as i32,
                    Pointer::Click,
                    true,
                )?;
            }
            1 => input.pointer(peer, (x * sx) as i32, (y * sy) as i32, Pointer::Down, false)?,
            2 => input.pointer(
                peer,
                ((x + w as f32) * sx) as i32,
                ((y + h as f32) * sy) as i32,
                Pointer::Move,
                false,
            )?,
            3 => input.pointer(
                peer,
                ((x + w as f32) * sx) as i32,
                ((y + h as f32) * sy) as i32,
                Pointer::Up,
                false,
            )?,
            4 => {
                if !capture.get_selected()
                    || capture.get_left() != x
                    || capture.get_top() != y
                    || capture.get_wide() != w as f32
                    || capture.get_high() != h as f32
                {
                    return Err(format!(
                        "Real mouse selection mismatch: {},{},{},{} selected={}",
                        capture.get_left(),
                        capture.get_top(),
                        capture.get_wide(),
                        capture.get_high(),
                        capture.get_selected()
                    ));
                }
                let logical_width = size.width as f32 / scale;
                let logical_height = size.height as f32 / scale;
                let panel_width = 480f32.min(logical_width - 24.);
                let panel_x = (x * sx / scale)
                    .min(logical_width - panel_width - 12.)
                    .max(12.);
                let panel_y = ((y + h as f32) * sy / scale + 12.)
                    .min(logical_height - 104. - 78.)
                    .max(12.);
                input.pointer(
                    peer,
                    ((panel_x + panel_width / 4.) * scale) as i32,
                    ((panel_y + 73.) * scale) as i32,
                    Pointer::Click,
                    false,
                )?;
            }
            _ => {
                *slot = None;
                return Ok(true);
            }
        }
        *phase += 1;
        Ok(false)
    }).map_err(|error: String| format!("OS selection phase {observed_phase}: {error}; visible={} selected={} region={},{},{},{}", capture.window().is_visible(), capture.get_selected(), capture.get_left(), capture.get_top(), capture.get_wide(), capture.get_high()))
}
