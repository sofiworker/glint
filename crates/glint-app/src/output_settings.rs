//! Output folder selection is a draft transaction; filename previews perform no I/O.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::{self, SettingsWindow},
};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Default)]
pub struct State {
    running: bool,
    original: Option<String>,
    control: Option<glint_platform::FolderPickerControl>,
}
pub fn sync(state: &AppState, ui: &SettingsWindow) {
    let tr = |id| messages::text(id, state.en());
    ui.set_folder_busy(state.output_settings.running);
    ui.set_folder_label(tr(Message::OutputFolderChoose).into());
    let filename = glint_core::output::filename(
        &state.session.preferences.draft.output,
        "2026-10-18",
        "143205",
        1280,
        720,
    );
    ui.set_filename_preview(
        format!(
            "{}\n{}",
            tr(Message::OutputFilenamePreview),
            filename.unwrap_or_else(|_| tr(Message::OutputFilenameInvalid).to_owned())
        )
        .into(),
    );
}
pub(crate) fn begin(state: &mut AppState, ui: &SettingsWindow) -> bool {
    if state.output_settings.running
        || !state.session.writable
        || ui.get_modal() != 0
        || ui.get_font_open()
        || ui.get_category() != 5
        || ui.get_guide_step() >= 0
        || !ui.window().is_visible()
        || state.pin_session.busy()
        || state.capturing
        || state.capture.is_some()
        || state.transfer.busy()
        || state.local_dialog
        || ui.get_workbench()
    {
        return false;
    }
    state.output_settings.running = true;
    state.output_settings.original = Some(state.session.preferences.draft.output.directory.clone());
    state.sync(ui);
    true
}
pub(crate) fn complete(
    state: &mut AppState,
    ui: &SettingsWindow,
    result: Result<Option<PathBuf>, String>,
) {
    state.output_settings.running = false;
    state.output_settings.control.take();
    let Some(original) = state.output_settings.original.take() else {
        state.sync(ui);
        return;
    };
    if !state.session.writable
        || ui.get_category() != 5
        || !ui.window().is_visible()
        || state.session.preferences.draft.output.directory != original
    {
        state.sync(ui);
        return;
    }
    match result {
        Ok(None) => {}
        Ok(Some(path)) => {
            let config = path
                .to_str()
                .filter(|_| path.is_absolute())
                .ok_or_else(|| "Invalid output folder".to_owned())
                .and_then(|path| {
                    settings::fields::change(
                        &state.session.preferences.draft,
                        "output.directory",
                        path,
                        state.en(),
                    )
                });
            match config {
                Ok(config) => {
                    state.session.preferences.draft = config;
                    state.feedback.clear();
                }
                Err(_) => {
                    state.feedback =
                        messages::text(Message::OutputFolderFailed, state.en()).to_owned()
                }
            }
        }
        Err(_) => {
            state.feedback = messages::text(Message::OutputFolderFailed, state.en()).to_owned()
        }
    }
    state.sync(ui);
}
pub fn handle(action: &str, shared: &Rc<RefCell<AppState>>, ui: &SettingsWindow) -> bool {
    if matches!(
        action,
        "close" | "quit" | "cancel" | "reset" | "confirm-reset" | "discard-leave"
    ) {
        let control = {
            let mut state = shared.borrow_mut();
            state.output_settings.original.take();
            state.output_settings.control.clone()
        };
        if let Some(control) = control {
            let _ = control.cancel();
        }
    }
    if shared.borrow().output_settings.running
        && (matches!(
            action,
            "open"
                | "capture"
                | "window-capture"
                | "repeat-capture"
                | "all-screens-capture"
                | "current-screen-capture"
                | "color-picker"
                | "local-image"
                | "guide-practice"
                | "clipboard-pin"
                | "workbench"
                | "config-import"
                | "config-export"
                | "guide-open"
        ) || action.starts_with("edit-pin:"))
    {
        return true;
    }
    if action != "output-folder" {
        return false;
    }
    if !begin(&mut shared.borrow_mut(), ui) {
        return true;
    }
    let (initial, title) = {
        let state = shared.borrow();
        (
            state.session.preferences.draft.output.directory.clone(),
            messages::text(Message::OutputFolderChoose, state.en()).to_owned(),
        )
    };
    // Return from the Slint action before entering the native modal loop. Slint
    // removes a callback's handler while it is running, so nested invocations
    // of that same callback would otherwise be lost during Show.
    let shared = shared.clone();
    let weak = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if shared.borrow().output_settings.original.is_none() || !ui.window().is_visible() {
            complete(&mut shared.borrow_mut(), &ui, Ok(None));
            return;
        }
        let result = choose(&ui, Path::new(&initial), &title, |control| {
            shared.borrow_mut().output_settings.control = Some(control)
        });
        complete(&mut shared.borrow_mut(), &ui, result);
    });
    true
}
fn choose(
    ui: &SettingsWindow,
    initial: &Path,
    title: &str,
    opened: impl FnOnce(glint_platform::FolderPickerControl),
) -> Result<Option<PathBuf>, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Not a Win32 settings window".into());
            };
            unsafe {
                glint_platform::choose_output_directory(
                    handle.hwnd.get() as *mut std::ffi::c_void,
                    initial,
                    title,
                    opened,
                )
            }
        })
        .ok_or_else(|| "Settings window unavailable".to_owned())?
}
