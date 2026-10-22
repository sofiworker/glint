//! Real opt-in log I/O through settings/capture/diagnostic callbacks, in an isolated directory.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_platform::diagnostic_log::{Event, Failure};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Run {
    old: Option<glint_core::config::Config>,
    started: Option<Instant>,
    bytes: Vec<u8>,
    config: Vec<u8>,
}
impl Run {
    fn waiting(&self) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
        {
            Err("Diagnostic log probe timed out".into())
        } else {
            Ok(false)
        }
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let logs = directory.join("logs");
        let current = logs.join("events.log");
        let change = |value| ui.invoke_change("privacy.diagnostics".into(), value);
        let scroll = || {
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                    position: slint::LogicalPosition::new(700., 400.),
                    delta_x: 0.,
                    delta_y: -4000.,
                })
        };
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                if app.borrow().session.preferences.saved().privacy.diagnostics || logs.exists() {
                    return Err("Default logging wrote assets".into());
                }
                ui.invoke_choose_page(7);
                change("true".into());
                if !app.borrow().session.preferences.draft.privacy.diagnostics
                    || ui.get_log_enabled()
                    || logs.exists()
                {
                    return Err("Log draft leaked into runtime".into());
                }
                ui.invoke_action("cancel".into());
                if app.borrow().session.preferences.draft.privacy.diagnostics {
                    return Err("Log draft cancel failed".into());
                }
                change("true".into());
                ui.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            1 => {
                if app.borrow().log.borrow().status().written < 2 {
                    return self.waiting();
                }
                self.config =
                    std::fs::read(directory.join("config.toml")).map_err(|e| e.to_string())?;
                if !ui.get_log_enabled()
                    || !app.borrow().session.preferences.saved().privacy.diagnostics
                {
                    return Err("Saved log preference not effective".into());
                }
                verify_lines(&current)?;
                std::fs::write(logs.join("unrelated.txt"), b"keep fixture")
                    .map_err(|e| e.to_string())?;
                ui.invoke_change("general.language".into(), "zh_cn".into());
                scroll();
            }
            2 => {
                super::snapshot(ui, &directory.join("log-privacy-zh.png"))?;
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_change("appearance.theme".into(), "dark".into());
                ui.invoke_action("diagnostics-refresh".into());
                scroll();
            }
            3 => {
                if !read(&current)?.contains("diagnostics.ready") {
                    return self.waiting();
                }
                super::snapshot(ui, &directory.join("log-privacy-dark-en.png"))?;
                ui.invoke_action("cancel".into());
                ui.invoke_change(
                    "output.filename".into(),
                    "PRIVATE-LOG-SENTINEL-{date}".into(),
                );
                use std::os::windows::fs::OpenOptionsExt;
                let locked = std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(directory.join("config.toml"))
                    .map_err(|e| e.to_string())?;
                ui.invoke_action("save".into());
                if !app.borrow().session.preferences.is_dirty() {
                    return Err("Failed save discarded log-test draft".into());
                }
                drop(locked);
                if std::fs::read(directory.join("config.toml")).map_err(|e| e.to_string())?
                    != self.config
                {
                    return Err("Failed save replaced accepted config".into());
                }
                self.started = Some(Instant::now());
            }
            4 => {
                if !read(&current)?.contains("settings.save_failed") {
                    return self.waiting();
                }
                ui.invoke_action("cancel".into());
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            5 => {
                let Some(capture) = app.borrow().capture.as_ref().map(|c| c.clone_strong()) else {
                    return self.waiting();
                };
                // Exercise fixed diagnostic routing, not a simulated successful export
                // or a claim that an actual output worker is busy in this fixture.
                capture.invoke_output_failed();
                capture.set_busy(true);
                capture.invoke_action("cancel".into());
            }
            6 => {
                if app.borrow().capture.is_some()
                    || !read(&current)?.contains("capture.cancelled")
                    || !read(&current)?.contains("capture.exit.api_cancel")
                    || !read(&current)?.contains("capture.closed.busy")
                    || !read(&current)?.contains("capture.output.failed")
                {
                    return self.waiting();
                }
                verify_lines(&current)?;
                change("false".into());
                ui.invoke_action("cancel".into());
                if !ui.get_log_enabled() {
                    return Err("Cancel disabled accepted logging".into());
                }
                change("false".into());
                ui.invoke_action("save".into());
                self.bytes = std::fs::read(&current).map_err(|e| e.to_string())?;
                app.borrow().log.borrow_mut().record(Event::Retry);
            }
            7 => {
                if ui.get_log_enabled()
                    || std::fs::read(&current).map_err(|e| e.to_string())? != self.bytes
                {
                    return Err("Disabled logging wrote a new event".into());
                }
                ui.invoke_action("log-clear".into());
                if ui.get_modal() != 7 {
                    return Err("Log clear lacked confirmation".into());
                }
                ui.invoke_action("continue".into());
                if ui.get_modal() != 0
                    || std::fs::read(&current).map_err(|e| e.to_string())? != self.bytes
                {
                    return Err("Clear cancel modified logs".into());
                }
                ui.invoke_action("log-clear".into());
                ui.invoke_action("log-clear-confirm".into());
                self.started = Some(Instant::now());
            }
            8 => {
                if app.borrow().log.borrow().status().cleared < 1 {
                    return self.waiting();
                }
                if current.exists()
                    || logs.join("events.log.1").exists()
                    || std::fs::read(logs.join("unrelated.txt")).map_err(|e| e.to_string())?
                        != b"keep fixture"
                {
                    return Err("Log clear removed unrelated assets or retained log".into());
                }
                std::fs::create_dir(&current).map_err(|e| e.to_string())?;
                change("true".into());
                ui.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            9 => {
                if app.borrow().log.borrow().status().failure != Some(Failure::Write) {
                    return self.waiting();
                }
                if !ui.get_log_failed() || !current.is_dir() {
                    return Err("Log write failure was hidden or destroyed target".into());
                }
                scroll();
                super::snapshot(ui, &directory.join("log-write-failed.png"))?;
                ui.invoke_action("log-clear".into());
                ui.invoke_action("log-clear-confirm".into());
            }
            10 => {
                if app.borrow().log.borrow().status().failure != Some(Failure::Clear) {
                    return self.waiting();
                }
                if !current.is_dir() {
                    return Err("Failed clear removed unexpected target".into());
                }
                std::fs::remove_dir(&current).map_err(|e| e.to_string())?;
                ui.invoke_action("log-retry".into());
                self.started = Some(Instant::now());
            }
            11 => {
                if !current.is_file() || !read(&current)?.contains("log.retry") {
                    return self.waiting();
                }
                if ui.get_log_failed() {
                    return Err("Successful log retry left stale error".into());
                }
                verify_lines(&current)?;
                change("false".into());
                ui.invoke_action("save".into());
                ui.invoke_action("log-clear".into());
                ui.invoke_action("log-clear-confirm".into());
                self.started = Some(Instant::now());
            }
            12 => {
                if app.borrow().log.borrow().status().cleared < 2 {
                    return self.waiting();
                }
                app.borrow_mut().session.preferences.draft =
                    self.old.take().ok_or("Missing original log settings")?;
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() || current.exists() {
                    return Err("Log fixture restore failed".into());
                }
                std::fs::remove_file(logs.join("unrelated.txt")).map_err(|e| e.to_string())?;
                ui.invoke_change("general.language".into(), "en".into());
                ui.window().set_size(slint::LogicalSize::new(900., 650.));
                scroll();
            }
            13 => {
                scroll();
            }
            14 => {
                super::snapshot(ui, &directory.join("log-privacy-compact-en.png"))?;
                ui.invoke_action("cancel".into());
                ui.window().set_size(slint::LogicalSize::new(1040., 760.));
                eprintln!(
                    "Diagnostic logs: default-off/draft/cancel/save, real configuration-lock failure/capture-cancel/diagnostic events, allowlisted content, disable, confirmed clear/unrelated preservation, write/clear failure and retry, bilingual compact layout passed"
                );
            }
            _ => return Err("Unknown diagnostic log stage".into()),
        }
        Ok(true)
    }
}
fn read(path: &PathBuf) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}
fn verify_lines(path: &PathBuf) -> Result<(), String> {
    let body = read(path)?;
    for line in body.lines() {
        let fields: Vec<_> = line.split(' ').collect();
        if fields.len() != 6
            || fields[0].parse::<u64>().is_err()
            || fields[1] != "v2"
            || fields[3]
                .strip_prefix("pid=")
                .and_then(|v| v.parse::<u32>().ok())
                .is_none()
            || fields[4]
                .strip_prefix("thread=ThreadId(")
                .and_then(|v| v.strip_suffix(')'))
                .and_then(|v| v.parse::<u64>().ok())
                .is_none()
            || ![
                "level=ERROR",
                "level=WARN",
                "level=INFO",
                "level=DEBUG",
                "level=TRACE",
            ]
            .contains(&fields[5])
            || ![
                "app.panic",
                "app.shutdown",
                "app.startup",
                "app.startup_failed",
                "capture.cancelled",
                "capture.closed.busy",
                "capture.completed",
                "capture.exit.api_cancel",
                "capture.exit.color_copy",
                "capture.exit.copy",
                "capture.exit.direct",
                "capture.exit.escape",
                "capture.exit.open",
                "capture.exit.pin",
                "capture.exit.pin_update",
                "capture.exit.save",
                "capture.exit.secondary_click",
                "capture.exit.toolbar_cancel",
                "capture.exit.window_request",
                "capture.failed",
                "capture.output.failed",
                "capture.ready",
                "capture.requested",
                "diagnostics.failed",
                "diagnostics.ready",
                "hotkey.failed",
                "hotkey.pressed",
                "input.cursor_restricted",
                "input.cursor_unrestricted",
                "input.pointer_captured",
                "input.pointer_released",
                "recording.started",
                "recording.paused",
                "recording.resumed",
                "recording.completed",
                "recording.cancelled",
                "recording.failed",
                "log.enabled",
                "log.retry",
                "scroll.acquired",
                "scroll.cancelled",
                "scroll.queued",
                "scroll.ready",
                "scroll.started",
                "settings.blurred",
                "settings.focused",
                "settings.save_failed",
                "settings.saved",
                "settings.shown",
                "system.connected",
                "system.disconnected",
                "system.locked",
                "system.resumed",
                "system.sleeping",
                "system.unlocked",
                "window.blurred",
                "window.focused",
            ]
            .contains(&fields[2])
        {
            return Err("Diagnostic log contains non-allowlisted content".into());
        }
    }
    if body.is_empty() {
        return Err("Diagnostic log has no actual events".into());
    }
    Ok(())
}
