//! Real configuration I/O and draft/runtime isolation; file paths stand in for OS picker selection.
use crate::{
    config_transfer,
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct DialogRun {
    draft: Option<glint_core::config::Config>,
    baseline: Option<Vec<u8>>,
    visits: Rc<std::cell::Cell<u32>>,
}
impl DialogRun {
    fn config_bytes(directory: &Path) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(directory.join("config.toml")) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<(), String> {
        match stage {
            0 => {
                ui.invoke_action("open".into());
                self.draft = Some(app.borrow().session.preferences.draft.clone());
                self.baseline = Self::config_bytes(directory)?;
                ui.invoke_action("config-export".into());
                if ui.get_modal() != 4 {
                    return Err("Dialog regression could not open export preview".into());
                }
                let visits = self.visits.clone();
                config_transfer::queue_selection(app, ui, directory, move |ui, export| {
                    assert!(export);
                    visits.set(visits.get() + 1);
                    // Simulate reentrant app actions at the native modal boundary.
                    // Global capture stays blocked; real hotkey-service errors stay visible.
                    ui.invoke_action("capture".into());
                    ui.invoke_action("hotkey-error".into());
                    Ok(None)
                });
                config_transfer::queue_selection(app, ui, directory, |_, _| {
                    panic!("Duplicate configuration picker entered")
                });
                if self.visits.get() != 0 || !app.borrow().transfer.busy() {
                    return Err("Configuration picker was not deferred/reserved".into());
                }
            }
            1 => {
                if self.visits.get() != 1
                    || app.borrow().transfer.busy()
                    || ui.get_modal() != 4
                    || app.borrow().capturing
                    || app.borrow().feedback
                        != messages::text(Message::ShortcutServiceStopped, app.borrow().en())
                {
                    return Err("Deferred cancel lost the preview or reentrant action".into());
                }
                config_transfer::queue_selection(app, ui, directory, |_, _| {
                    Err("injected picker failure".into())
                });
            }
            2 => {
                if app.borrow().transfer.busy()
                    || ui.get_modal() != 4
                    || ui.get_transfer_status()
                        != messages::text(Message::TransferExportFailed, app.borrow().en())
                {
                    return Err("Picker failure did not retain a retryable preview".into());
                }
                config_transfer::queue_selection(app, ui, directory, |_, _| {
                    panic!("Hidden window launched a queued configuration picker")
                });
                ui.hide().map_err(|e| e.to_string())?;
            }
            3 => {
                if app.borrow().transfer.busy()
                    || ui.window().is_visible()
                    || app.borrow().session.preferences.draft != *self.draft.as_ref().unwrap()
                    || Self::config_bytes(directory)? != self.baseline
                {
                    return Err(
                        "Hidden queued picker changed draft/disk or reopened settings".into(),
                    );
                }
                ui.invoke_action("config-transfer-cancel".into());
                if app.borrow().transfer.open() || ui.get_modal() != 0 {
                    return Err("Canceled deferred picker retained its modal reservation".into());
                }
                eprintln!(
                    "Configuration dialog: deferred boundary, duplicate reservation, reentrant capture guard/hotkey-error delivery, cancel/failure retry, hidden-window suppression and draft/disk isolation passed; picker results injected, physical file selection not exercised"
                );
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Run {
    old: Option<glint_core::config::Config>,
    draft: Option<glint_core::config::Config>,
    baseline: Vec<u8>,
    bindings: Vec<glint_platform::HotkeyBinding>,
    export: String,
    started: Option<Instant>,
}
impl Run {
    fn select(
        &mut self,
        name: &str,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) {
        config_transfer::selected(&mut app.borrow_mut(), ui, directory.join(name), directory);
        self.started = Some(Instant::now());
    }
    fn fixture(&self, directory: &Path, key: &str) -> Result<(), String> {
        std::fs::write(directory.join("import.toml"), format!("config_version=1\n[appearance]\naccent='#168975'\n[output]\nquality=93\n[shortcuts]\ncapture='{key}'\n[general]\nonboarding_complete=true\n[capture]\ndelay_seconds=999\n")).map_err(|e| e.to_string())
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        hotkeys: &glint_platform::HotkeyService,
        directory: &Path,
    ) -> Result<bool, String> {
        if stage != 0 && app.borrow().transfer.busy() {
            if self
                .started
                .is_some_and(|time| time.elapsed() > Duration::from_secs(20))
            {
                return Err("Config transfer timeout".into());
            }
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.bindings = hotkeys.bindings()?;
                self.baseline =
                    std::fs::read(directory.join("config.toml")).map_err(|e| e.to_string())?;
                self.export = glint_core::config_transfer::export(self.old.as_ref().unwrap())
                    .map_err(|e| e.to_string())?;
                ui.invoke_change("general.language".into(), "en".into());
                let quality = if app.borrow().session.preferences.saved().output.quality == 87 {
                    "88"
                } else {
                    "87"
                };
                ui.invoke_change("output.quality".into(), quality.into());
                self.draft = Some(app.borrow().session.preferences.draft.clone());
                ui.invoke_choose_page(12);
                app.borrow_mut().session.writable = false;
                app.borrow().sync(ui);
                ui.invoke_action("config-export".into());
                ui.invoke_action("config-import".into());
                if ui.get_modal() != 0 {
                    return Err("Read-only recovery offered a misleading portable snapshot".into());
                }
                app.borrow_mut().session.writable = true;
                app.borrow().sync(ui);
                ui.invoke_action("config-export".into());
                if ui.get_modal() != 4 || ui.get_transfer_body() != self.export {
                    return Err("Export did not preview the saved snapshot".into());
                }
                ui.invoke_action("capture".into());
                if ui.get_modal() != 4 || app.borrow().capturing {
                    return Err("Global capture discarded export preview".into());
                }
                super::snapshot(ui, &directory.join("config-export-en.png"))?;
                std::fs::write(
                    directory.join("existing.toml"),
                    b"existing external document",
                )
                .map_err(|e| e.to_string())?;
                self.select("existing.toml", ui, app, directory);
            }
            1 => {
                if ui.get_modal() != 4
                    || ui.get_transfer_status()
                        != messages::text(Message::TransferExportFailed, true)
                    || std::fs::read(directory.join("existing.toml")).map_err(|e| e.to_string())?
                        != b"existing external document"
                {
                    return Err("Export collision overwrote a file or discarded preview".into());
                }
                self.select("config.toml", ui, app, directory);
                if app.borrow().transfer.busy() {
                    return Err("Export accepted the active config destination".into());
                }
                self.select("portable.toml", ui, app, directory);
            }
            2 => {
                let bytes =
                    std::fs::read(directory.join("portable.toml")).map_err(|e| e.to_string())?;
                if bytes != self.export.as_bytes()
                    || self.export.contains("onboarding_complete")
                    || ui.get_modal() != 0
                    || app.borrow().session.preferences.draft != *self.draft.as_ref().unwrap()
                    || std::fs::read(directory.join("config.toml")).map_err(|e| e.to_string())?
                        != self.baseline
                {
                    return Err("Export changed app/draft settings or included local marker".into());
                }
                self.fixture(directory, "Ctrl+Alt+Shift+F19")?;
                ui.invoke_action("config-import".into());
                self.select("import.toml", ui, app, directory);
            }
            3 => {
                if !ui.get_transfer_can_apply()
                    || !ui.get_transfer_body().contains("capture.delay_seconds")
                    || !ui
                        .get_transfer_body()
                        .contains("general.onboarding_complete")
                    || app.borrow().session.preferences.draft != *self.draft.as_ref().unwrap()
                {
                    return Err("Import preview mutated draft or omitted skipped fields".into());
                }
                super::snapshot(ui, &directory.join("config-import-preview-en.png"))?;
                ui.invoke_action("close".into());
                if ui.get_modal() != 1
                    || app.borrow().transfer.open()
                    || app.borrow().session.preferences.draft != *self.draft.as_ref().unwrap()
                {
                    return Err("Import window close lost preview/draft exit semantics".into());
                }
                ui.invoke_action("continue".into());
                ui.invoke_action("config-import".into());
                self.select("import.toml", ui, app, directory);
            }
            4 => {
                let size = if app.borrow().session.preferences.draft.annotation.text_size == 31 {
                    "32"
                } else {
                    "31"
                };
                ui.invoke_change("annotation.text_size".into(), size.into());
                let changed = app.borrow().session.preferences.draft.clone();
                ui.invoke_action("config-transfer-apply".into());
                if ui.get_transfer_can_apply()
                    || ui.get_transfer_status() != messages::text(Message::TransferStale, true)
                    || app.borrow().session.preferences.draft != changed
                {
                    return Err("Stale import overwrote a changed draft".into());
                }
                super::snapshot(ui, &directory.join("config-import-stale.png"))?;
                ui.invoke_action("config-transfer-cancel".into());
                app.borrow_mut().session.preferences.draft = self.draft.as_ref().unwrap().clone();
                ui.invoke_action("config-import".into());
                self.select("import.toml", ui, app, directory);
            }
            5 => {
                ui.invoke_action("config-transfer-apply".into());
                if ui.get_modal() != 0
                    || app.borrow().session.preferences.draft.output.quality != 93
                    || app.borrow().session.preferences.saved() != self.old.as_ref().unwrap()
                    || hotkeys.bindings()? != self.bindings
                {
                    return Err(
                        "Import confirmation applied runtime/disk settings prematurely".into(),
                    );
                }
                ui.invoke_action("cancel".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Cancel could not undo the imported draft".into());
                }
                self.fixture(directory, "Ctrl+Alt+Shift+F20")?;
                ui.invoke_action("config-import".into());
                self.select("import.toml", ui, app, directory);
            }
            6 => {
                let occupied = glint_platform::HotkeyService::new(
                    vec![glint_platform::HotkeyBinding {
                        action: 77,
                        modifiers: 7,
                        key: 0x83,
                    }],
                    |_| {},
                )?;
                ui.invoke_action("config-transfer-apply".into());
                ui.invoke_action("save".into());
                if !app.borrow().session.preferences.is_dirty()
                    || hotkeys.bindings()? != self.bindings
                    || app.borrow().session.preferences.saved() != self.old.as_ref().unwrap()
                {
                    return Err(
                        "Imported shortcut conflict discarded draft or working bindings".into(),
                    );
                }
                drop(occupied);
                use std::os::windows::fs::OpenOptionsExt;
                let locked = std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(directory.join("config.toml"))
                    .map_err(|e| e.to_string())?;
                ui.invoke_action("save".into());
                if !app.borrow().session.preferences.is_dirty()
                    || hotkeys.bindings()? != self.bindings
                {
                    return Err(
                        "Failed imported file save did not roll back runtime bindings".into(),
                    );
                }
                drop(locked);
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty()
                    || !hotkeys
                        .bindings()?
                        .iter()
                        .any(|key| key.action == 1 && key.key == 0x83)
                    || app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .general
                        .onboarding_complete
                        != self.old.as_ref().unwrap().general.onboarding_complete
                {
                    return Err("Import save retry or local marker preservation failed".into());
                }
                std::fs::write(
                    directory.join("oversize.toml"),
                    vec![b'x'; glint_core::config::MAX_CONFIG_BYTES + 1],
                )
                .map_err(|e| e.to_string())?;
                ui.invoke_action("config-import".into());
                self.select("oversize.toml", ui, app, directory);
            }
            7 => {
                if ui.get_transfer_status()
                    != messages::text(Message::TransferTooLarge, app.borrow().en())
                    || ui.get_transfer_can_apply()
                {
                    return Err("Oversize import did not reject safely".into());
                }
                std::fs::write(directory.join("future.toml"), b"config_version=999")
                    .map_err(|e| e.to_string())?;
                self.select("future.toml", ui, app, directory);
            }
            8 => {
                if ui.get_transfer_status()
                    != messages::text(Message::TransferVersion, app.borrow().en())
                {
                    return Err("Future import version accepted".into());
                }
                std::fs::write(directory.join("invalid.toml"), [0xff, 0xfe, 0x01])
                    .map_err(|e| e.to_string())?;
                self.select("invalid.toml", ui, app, directory);
            }
            9 => {
                ui.invoke_change("general.language".into(), "zh_cn".into());
                if ui.get_transfer_status() != messages::text(Message::TransferInvalid, false)
                    || ui.get_transfer_can_apply()
                {
                    return Err("Invalid UTF-8 import was accepted or failed localization".into());
                }
                super::snapshot(ui, &directory.join("config-invalid-zh.png"))?;
                self.select("import.toml", ui, app, directory);
                ui.invoke_action("config-transfer-cancel".into());
                ui.invoke_action("config-import".into());
                if ui.get_modal() != 0 {
                    return Err("Cancelled import allowed a second simultaneous read".into());
                }
            }
            10 => {
                if ui.get_modal() != 0 || app.borrow().transfer.open() {
                    return Err("Late import result reopened a cancelled preview".into());
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty()
                    || hotkeys.bindings()? != self.bindings
                {
                    return Err("Transfer smoke cleanup failed".into());
                }
                eprintln!(
                    "Native configuration transfer: saved export snapshot/no overwrite/live-file protection, bounded TOML/UTF8/version checks, skipped fields, preview/cancel/stale guard/draft merge, actual occupied shortcut and locked-file rollback/retry, cancellation/late-result isolation passed"
                );
            }
            _ => return Err("Unknown config transfer stage".into()),
        }
        Ok(true)
    }
}
