//! Auxiliary local images: bounded decoding off the UI thread, no desktop capture.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    rc::Rc,
};

const FILE_LIMIT: u64 = 64 * 1024 * 1024;
const PIXEL_LIMIT: u64 = 64 * 1024 * 1024;

pub fn prepare(
    path: &Path,
    work: (i32, i32, u32, u32),
    scale: f32,
) -> Result<crate::pin_edit::Prepared, String> {
    let file = std::fs::File::open(path).map_err(|_| "Cannot read image file")?;
    let metadata = file.metadata().map_err(|_| "Cannot inspect image file")?;
    if !metadata.is_file() {
        return Err("Not a regular image file".into());
    }
    if metadata.len() > FILE_LIMIT {
        return Err("Image file exceeds 64 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read image file")?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err("Image file exceeds 64 MiB".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Invalid image")?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err("Only PNG and JPEG images are supported".into());
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(PIXEL_LIMIT);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "Invalid image or decode budget exceeded")?;
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) * 4 > PIXEL_LIMIT {
        return Err("Decoded image exceeds 64 MiB".into());
    }
    let rgba = image.into_rgba8();
    let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), w, h);
    crate::pin_edit::Source {
        pixels,
        document: None,
        work,
        scale,
        turns: 0,
    }
    .prepare()
}

pub fn sync(state: &AppState, ui: &SettingsWindow) {
    let tr = |id| messages::text(id, state.en());
    ui.set_workbench_title(tr(Message::Workbench).into());
    ui.set_workbench_hint(tr(Message::WorkbenchHint).into());
    ui.set_local_open_label(tr(Message::LocalOpen).into());
    ui.set_workbench_capture_label(tr(Message::WorkbenchCapture).into());
    ui.set_workbench_settings_label(tr(Message::WorkbenchPrivacy).into());
    ui.set_workbench_output_label(tr(Message::WorkbenchOutput).into());
    ui.set_workbench_note(tr(Message::WorkbenchNote).into());
    if ui.get_workbench() {
        ui.set_heading(tr(Message::Workbench).into());
        ui.set_subtitle(tr(Message::WorkbenchHint).into());
        ui.set_status(state.feedback.clone().into());
    }
}

/// Shared production path after the native picker returns (also exercised by smoke).
pub fn selected(state: &mut AppState, path: PathBuf) {
    state.local_path = Some(path);
}

pub(crate) fn queue_selection(
    shared: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    picker: impl FnOnce(&SettingsWindow) -> Result<Option<PathBuf>, String> + 'static,
) {
    let generation = {
        let mut state = shared.borrow_mut();
        if state.local_dialog
            || !ui.window().is_visible()
            || !ui.get_workbench()
            || ui.get_modal() != 0
            || state.capture.is_some()
            || state.capturing
            || state.clipboard_pin_pending
            || state.transfer.busy()
            || state.lifecycle.blocked()
        {
            return;
        }
        state.local_dialog = true;
        state.lifecycle.generation
    };
    let shared = shared.clone();
    let weak = ui.as_weak();
    // Return the Slint action handler before entering the native modal loop.
    // Nested actions must reach the guards/recovery paths below, not disappear.
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        let Some(ui) = weak.upgrade() else {
            shared.borrow_mut().local_dialog = false;
            return;
        };
        let current = |state: &AppState| {
            ui.window().is_visible()
                && ui.get_workbench()
                && ui.get_modal() == 0
                && !state.lifecycle.blocked()
                && state.lifecycle.generation == generation
        };
        let active = current(&shared.borrow());
        // Hold no application RefCell borrow across the native modal loop.
        let result = if active { picker(&ui) } else { Ok(None) };
        let chosen = {
            let mut state = shared.borrow_mut();
            state.local_dialog = false;
            if !current(&state) {
                false
            } else {
                match result {
                    Ok(Some(path)) => {
                        selected(&mut state, path);
                        true
                    }
                    Ok(None) => false,
                    Err(_) => {
                        state.feedback = messages::text(Message::LocalFailed, state.en()).into();
                        false
                    }
                }
            }
        };
        shared.borrow().sync(&ui);
        if chosen {
            ui.invoke_action("local-image".into());
        }
    });
}

fn choose(ui: &SettingsWindow) -> Result<Option<PathBuf>, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Not a Win32 window".into());
            };
            unsafe {
                glint_platform::choose_local_image_path(handle.hwnd.get() as *mut std::ffi::c_void)
            }
        })
        .unwrap_or_else(|| Err("Window unavailable".into()))
}

pub fn handle(action: &str, shared: &Rc<RefCell<AppState>>, ui: &SettingsWindow) -> bool {
    if shared.borrow().local_dialog {
        // Rescue and service failure recovery remain usable during file choice.
        return !matches!(action, "rescue-pins" | "toggle-pins" | "hotkey-error");
    }
    if action != "open-local" {
        return false;
    }
    queue_selection(shared, ui, choose);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_png_jpeg_decode_preserves_pixels_and_rejects_invalid_and_oversize() {
        let root = std::env::temp_dir().join(format!("glint-local-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("source.dat");
        let raw = [21u8, 63, 117, 128].repeat(6);
        image::save_buffer_with_format(
            &path,
            &raw,
            3,
            2,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        let prepared = prepare(&path, (-1200, 0, 1200, 800), 1.).unwrap();
        let doc = glint_core::ImageDocument::from_selection(
            &prepared.frame.rgba,
            prepared.frame.width,
            prepared.frame.height,
            prepared.selection,
            &prepared.edits,
        )
        .unwrap();
        assert_eq!(doc.source(), raw);
        assert_eq!(prepared.frame.x, -1200);
        image::save_buffer_with_format(
            &path,
            &[21u8, 63, 117].repeat(6),
            3,
            2,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();
        assert_eq!(
            prepare(&path, (0, 0, 1200, 800), 1.)
                .unwrap()
                .selection
                .width,
            3
        );
        std::fs::write(&path, b"not an image").unwrap();
        assert!(prepare(&path, (0, 0, 1200, 800), 1.).is_err());
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(FILE_LIMIT + 1).unwrap();
        drop(file);
        assert!(prepare(&path, (0, 0, 1200, 800), 1.).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
