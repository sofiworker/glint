//! External configuration preview. Workers do bounded I/O; only confirmation edits the draft.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use glint_core::{
    config::{Config, ConfigError, MAX_CONFIG_BYTES},
    config_transfer as portable,
};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::mpsc::{self, Receiver},
};

struct Prepared {
    base: Config,
    plan: portable::Preview,
}
enum Mode {
    Export(String),
    Import(Option<Box<Prepared>>),
}
enum Completed {
    Imported(Box<Prepared>),
    Exported,
}
#[derive(Default)]
pub struct State {
    mode: Option<Mode>,
    job: Option<Receiver<Result<Completed, Message>>>,
    dialog: bool,
    error: Option<Message>,
}
impl State {
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.dialog
    }
    pub fn open(&self) -> bool {
        self.mode.is_some()
    }
}
fn abbreviated(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}
pub fn sync(state: &AppState, ui: &SettingsWindow) {
    let tr = |id| messages::text(id, state.en());
    ui.set_transfer_import_label(tr(Message::TransferImport).into());
    ui.set_transfer_export_label(tr(Message::TransferExport).into());
    ui.set_transfer_select_label(tr(Message::TransferSelect).into());
    ui.set_transfer_apply_label(tr(Message::TransferApply).into());
    ui.set_transfer_cancel_label(tr(Message::TransferCancel).into());
    ui.set_transfer_busy(state.transfer.busy());
    let Some(mode) = &state.transfer.mode else {
        return;
    };
    let export = matches!(mode, Mode::Export(_));
    ui.set_transfer_title(
        tr(if export {
            Message::TransferExport
        } else {
            Message::TransferImport
        })
        .into(),
    );
    ui.set_transfer_hint(
        tr(if export {
            Message::TransferExportHint
        } else {
            Message::TransferImportHint
        })
        .into(),
    );
    ui.set_transfer_can_apply(false);
    let body = match mode {
        Mode::Export(text) => text.clone(),
        Mode::Import(None) => String::new(),
        Mode::Import(Some(prepared)) => {
            ui.set_transfer_can_apply(
                !prepared.plan.changes.is_empty()
                    && prepared.base == state.session.preferences.draft
                    && !state.transfer.busy(),
            );
            let mut text = prepared
                .plan
                .changes
                .iter()
                .map(|change| format!("{}\n  {} → {}", change.field, change.before, change.after))
                .collect::<Vec<_>>()
                .join("\n\n");
            if text.is_empty() {
                text.push_str(tr(Message::TransferNoChanges));
            }
            if !prepared.plan.skipped.is_empty() {
                text.push_str(&format!(
                    "\n\n{} ({})\n",
                    tr(Message::TransferSkipped),
                    prepared.plan.skipped.len()
                ));
                // Bound rendering even for a file containing thousands of unknown keys.
                for key in prepared.plan.skipped.iter().take(100) {
                    text.push_str(&format!("• {}\n", abbreviated(&format!("{key:?}"), 120)));
                }
                if prepared.plan.skipped.len() > 100 {
                    text.push('…');
                }
            }
            text
        }
    };
    ui.set_transfer_body(body.into());
    ui.set_transfer_status(
        if state.transfer.busy() {
            tr(Message::TransferBusy)
        } else {
            state.transfer.error.map(tr).unwrap_or("")
        }
        .into(),
    );
}
fn error_id(error: ConfigError) -> Message {
    match error {
        ConfigError::TooLarge => Message::TransferTooLarge,
        ConfigError::UnsupportedVersion(_) => Message::TransferVersion,
        _ => Message::TransferInvalid,
    }
}
fn spawn(
    state: &mut AppState,
    ui: &SettingsWindow,
    failure: Message,
    work: impl FnOnce() -> Result<Completed, Message> + Send + 'static,
) -> Result<(), Message> {
    if state.transfer.busy() {
        return Err(Message::TransferBusy);
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    let weak = ui.as_weak();
    std::thread::Builder::new()
        .name("glint-config-transfer".into())
        .spawn(move || {
            let _ = sender.send(work());
            let _ =
                weak.upgrade_in_event_loop(|ui| ui.invoke_action("config-transfer-ready".into()));
        })
        .map_err(|_| failure)?;
    state.transfer.job = Some(receiver);
    state.transfer.error = None;
    Ok(())
}
pub(crate) fn read(
    state: &mut AppState,
    ui: &SettingsWindow,
    path: PathBuf,
) -> Result<(), Message> {
    if !matches!(state.transfer.mode, Some(Mode::Import(_))) {
        return Err(Message::TransferInvalid);
    }
    if state.transfer.busy() {
        return Err(Message::TransferBusy);
    }
    let base = state.session.preferences.draft.clone();
    state.transfer.mode = Some(Mode::Import(None));
    spawn(state, ui, Message::TransferReadFailed, move || {
        let bytes = glint_platform::config_file::read_document(&path, MAX_CONFIG_BYTES).map_err(
            |e| match e {
                glint_platform::config_file::StoreError::TooLarge => Message::TransferTooLarge,
                _ => Message::TransferReadFailed,
            },
        )?;
        let text = std::str::from_utf8(&bytes).map_err(|_| Message::TransferInvalid)?;
        let plan = portable::preview(&base, text).map_err(error_id)?;
        Ok(Completed::Imported(Box::new(Prepared { base, plan })))
    })?;
    Ok(())
}
pub(crate) fn write(
    state: &mut AppState,
    ui: &SettingsWindow,
    path: PathBuf,
    directory: &Path,
) -> Result<(), Message> {
    let Some(Mode::Export(text)) = &state.transfer.mode else {
        return Err(Message::TransferExportFailed);
    };
    // Export must not create/replace the live configuration or its recovery files.
    let protected_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name.starts_with("config.toml") || name.starts_with("pins.session")
        });
    if protected_name && path.parent() == Some(directory) {
        return Err(Message::TransferExportFailed);
    }
    let text = text.clone();
    let directory = directory.to_owned();
    spawn(state, ui, Message::TransferExportFailed, move || {
        if protected_name
            && path.parent().and_then(|p| p.canonicalize().ok()) == directory.canonicalize().ok()
        {
            return Err(Message::TransferExportFailed);
        }
        glint_platform::config_file::export_new(&path, text.as_bytes(), MAX_CONFIG_BYTES)
            .map_err(|_| Message::TransferExportFailed)?;
        Ok(Completed::Exported)
    })
}
fn choose(ui: &SettingsWindow, export: bool) -> Result<Option<PathBuf>, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Not a Win32 window".into());
            };
            unsafe {
                glint_platform::choose_config_path(
                    handle.hwnd.get() as *mut std::ffi::c_void,
                    export,
                )
            }
        })
        .unwrap_or_else(|| Err("Settings window unavailable".into()))
}

pub(crate) fn selected(state: &mut AppState, ui: &SettingsWindow, path: PathBuf, directory: &Path) {
    let result = if matches!(state.transfer.mode, Some(Mode::Export(_))) {
        write(state, ui, path, directory)
    } else {
        read(state, ui, path)
    };
    if let Err(error) = result {
        state.transfer.error = Some(error);
    }
    state.sync(ui);
}

pub(crate) fn queue_selection(
    shared: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    directory: &Path,
    picker: impl FnOnce(&SettingsWindow, bool) -> Result<Option<PathBuf>, String> + 'static,
) {
    let export = {
        let mut state = shared.borrow_mut();
        if !state.transfer.open() || state.transfer.busy() || !ui.window().is_visible() {
            return;
        }
        let export = matches!(state.transfer.mode, Some(Mode::Export(_)));
        state.transfer.dialog = true;
        state.sync(ui);
        export
    };
    let shared = shared.clone();
    let weak = ui.as_weak();
    let directory = directory.to_owned();
    // Slint temporarily removes an action handler while invoking it. Enter the
    // native modal loop only after it is restored, so nested actions reach the
    // existing modal guards (and rescue/hotkey-error handlers).
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        let Some(ui) = weak.upgrade() else {
            shared.borrow_mut().transfer.dialog = false;
            return;
        };
        let active = {
            let state = shared.borrow();
            state.transfer.open() && ui.get_modal() == 4 && ui.window().is_visible()
        };
        // Hold no application borrow across the native modal loop.
        let chosen = if active {
            picker(&ui, export)
        } else {
            Ok(None)
        };
        let mut state = shared.borrow_mut();
        state.transfer.dialog = false;
        if state.transfer.open() && ui.get_modal() == 4 && ui.window().is_visible() {
            match chosen {
                Ok(Some(path)) => selected(&mut state, &ui, path, &directory),
                Ok(None) => {}
                Err(_) => {
                    state.transfer.error = Some(if export {
                        Message::TransferExportFailed
                    } else {
                        Message::TransferReadFailed
                    })
                }
            }
        }
        state.sync(&ui);
    });
}

pub fn handle(
    action: &str,
    shared: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    directory: &Path,
) -> bool {
    if action == "config-transfer-select" {
        queue_selection(shared, ui, directory, choose);
        return true;
    }
    let mut state = shared.borrow_mut();
    match action {
        "config-import" | "config-export" => {
            if state.transfer.busy()
                || state.capture.is_some()
                || state.capturing
                || ui.get_modal() > 0
                || ui.get_guide_step() >= 0
            {
                return true;
            }
            if !state.session.writable {
                return true;
            }
            state.transfer.mode = if action == "config-export" {
                match portable::export(state.session.preferences.saved()) {
                    Ok(text) => Some(Mode::Export(text)),
                    Err(_) => {
                        state.feedback =
                            messages::text(Message::TransferExportFailed, state.en()).into();
                        None
                    }
                }
            } else {
                Some(Mode::Import(None))
            };
            state.transfer.error = None;
            if state.transfer.open() {
                ui.invoke_show_modal(4);
            }
        }
        "config-transfer-ready" => {
            let Some(receiver) = state.transfer.job.take() else {
                return true;
            };
            match receiver.try_recv() {
                Ok(result) if state.transfer.open() => match result {
                    Ok(Completed::Imported(prepared)) => {
                        state.transfer.error = (prepared.base != state.session.preferences.draft)
                            .then_some(Message::TransferStale);
                        state.transfer.mode = Some(Mode::Import(Some(prepared)));
                    }
                    Ok(Completed::Exported) => {
                        state.transfer.mode = None;
                        ui.invoke_show_modal(0);
                        state.feedback =
                            messages::text(Message::TransferExportDone, state.en()).into();
                    }
                    Err(error) => state.transfer.error = Some(error),
                },
                Err(mpsc::TryRecvError::Empty) => state.transfer.job = Some(receiver),
                Err(mpsc::TryRecvError::Disconnected) => {
                    state.transfer.error = Some(Message::TransferReadFailed)
                }
                _ => {}
            }
        }
        "config-transfer-apply" => {
            if state.transfer.busy() || !state.session.writable {
                return true;
            }
            if let Some(Mode::Import(Some(prepared))) = &state.transfer.mode {
                if prepared.base == state.session.preferences.draft {
                    let next = prepared.plan.candidate.clone();
                    state.session.preferences.draft = next;
                    state.transfer.mode = None;
                    ui.invoke_show_modal(0);
                    state.feedback = messages::text(Message::TransferDraftReady, state.en()).into();
                } else {
                    state.transfer.error = Some(Message::TransferStale);
                }
            }
        }
        "config-transfer-cancel" | "continue" if ui.get_modal() == 4 => {
            if state.transfer.dialog {
                return true;
            }
            state.transfer.mode = None;
            state.transfer.error = None;
            ui.invoke_show_modal(0);
        }
        "close" | "quit" if ui.get_modal() == 4 => {
            if state.transfer.dialog {
                return true;
            }
            state.transfer.mode = None;
            state.transfer.error = None;
            ui.invoke_show_modal(0);
            state.sync(ui);
            // Continue through the normal dirty-settings guard and resident exit path.
            return false;
        }
        // Global capture/settings/quit actions must not silently discard this preview.
        _ if ui.get_modal() == 4
            && !matches!(action, "rescue-pins" | "toggle-pins" | "hotkey-error") =>
        {
            let _ = crate::resident::show_settings(ui);
        }
        _ => return false,
    }
    state.sync(ui);
    true
}
