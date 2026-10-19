//! Independent color capture: real hotkey rollback, source samples, clipboard and cancellation.
use crate::{
    messages::{self, Message},
    probe,
    resident::AppState,
    settings::SettingsWindow,
};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

fn synthetic(directory: &Path) -> Result<(), String> {
    for pointer in [None, Some((-50, -25))] {
        let frame = glint_platform::DesktopFrame {
            x: -100,
            y: -50,
            width: 640,
            height: 480,
            rgba: [17, 34, 51, 255].repeat(640 * 480),
        };
        let (ui, _) = probe::resident_color_picker(
            glint_platform::DesktopSnapshot {
                frame,
                displays: vec![],
                windows: vec![],
                detection_error: None,
                pointer,
            },
            probe::Presentation {
                english: true,
                ..Default::default()
            },
            || {},
        )?;
        if !ui.get_color_only()
            || ui.get_tool() != 6
            || ui.get_pin_enabled()
            || ui.get_object_count() != 0
            || ui.get_sample_valid() != pointer.is_some()
        {
            return Err("Independent picker initial state mismatch".into());
        }
        if pointer.is_some()
            && (ui.get_sample_hex() != "#112233" || ui.get_sample_position() != "(-50, -25)")
        {
            return Err("Independent picker source/global-coordinate mismatch".into());
        }
        ui.invoke_hover(f32::NAN, 4.);
        ui.invoke_action("finish".into());
        if ui.get_closed()
            || ui.get_picker_busy()
            || ui.get_picker_feedback() != messages::text(Message::CaptureColorOutside, true)
        {
            return Err("Invalid picker point attempted image output".into());
        }
        ui.invoke_toolbar_command("open".into(), 0, 0);
        if ui.get_toolbar_modal() != 0 {
            return Err("Independent picker opened image toolbar editor".into());
        }
        ui.invoke_action("save".into());
        ui.invoke_action("pin".into());
        ui.invoke_action("copy".into());
        if ui.get_busy() || ui.get_closed() {
            return Err("Independent picker accepted image output".into());
        }
        ui.invoke_hover(50., 25.);
        super::capture_snapshot(
            &ui,
            &directory.join(if pointer.is_some() {
                "color-picker-synthetic.png"
            } else {
                "color-picker-missing-pointer.png"
            }),
        )?;
    }
    Ok(())
}

#[derive(Default)]
pub struct Run {
    old: Option<glint_core::config::Config>,
    output: Option<PathBuf>,
    remembered: Option<glint_core::region_memory::RememberedRegion>,
    started: Option<Instant>,
    blocker: Option<glint_platform::ClipboardBlocker>,
    expected: String,
    cancelled: Option<probe::CaptureProbe>,
}
impl Run {
    fn wait(&self) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
        {
            Err("Independent picker capture/output timed out".into())
        } else {
            Ok(false)
        }
    }
    fn start(&mut self, settings: &SettingsWindow) {
        self.started = Some(Instant::now());
        settings.invoke_action("color-picker".into());
    }
    fn sample(
        &mut self,
        ui: &probe::CaptureProbe,
        rgb: bool,
        x: u32,
        y: u32,
    ) -> Result<(), String> {
        if !ui.get_color_only() || ui.get_tool() != 6 || ui.get_pin_enabled() {
            return Err("Independent picker mode was lost".into());
        }
        let source = ui
            .get_desktop()
            .to_rgba8()
            .ok_or("Missing frozen picker source")?;
        let c =
            glint_core::sample_rgba(source.as_bytes(), (source.width(), source.height()), (x, y))
                .ok_or("Invalid test point")?;
        ui.invoke_hover(x as f32, y as f32);
        let hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
        let value = format!("RGB({}, {}, {})", c[0], c[1], c[2]);
        if !ui.get_sample_valid() || ui.get_sample_hex() != hex || ui.get_sample_rgb() != value {
            return Err("Independent picker differs from frozen source".into());
        }
        self.expected = if rgb { value } else { hex };
        Ok(())
    }
    fn copied(&self, app: &Rc<RefCell<AppState>>, directory: &Path) -> Result<(), String> {
        if glint_platform::read_clipboard_text()? != self.expected {
            return Err("Independent picker clipboard differs from frozen source".into());
        }
        if directory.join("must-not-image.png").exists()
            || app.borrow().last_region != self.remembered
        {
            return Err("Color copy changed image output or repeat memory".into());
        }
        Ok(())
    }
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        hotkeys: &glint_platform::HotkeyService,
        directory: &Path,
    ) -> Result<bool, String> {
        let capture = app.borrow().capture.as_ref().map(|c| c.clone_strong());
        if [1, 2, 4, 6, 9, 11].contains(&stage) && capture.is_none() {
            return self.wait();
        }
        if [3, 5, 7, 10, 12].contains(&stage) && (capture.is_some() || app.borrow().capturing) {
            return self.wait();
        }
        match stage {
            0 => {
                synthetic(directory)?;
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                self.remembered = app.borrow().last_region.clone();
                settings.invoke_record("shortcuts.color_picker".into());
                if settings.get_modal() != 3 || !settings.get_can_clear() {
                    return Err("Optional color hotkey recorder unavailable".into());
                }
                settings.invoke_action("clear-key".into());
                settings.invoke_action("save".into());
                if hotkeys.bindings()?.iter().any(|b| b.action == 7) {
                    return Err("Cleared picker key remains registered".into());
                }
                settings
                    .invoke_change("shortcuts.color_picker".into(), "Ctrl+Alt+Shift+F19".into());
                settings.invoke_action("save".into());
                if !hotkeys
                    .bindings()?
                    .iter()
                    .any(|b| b.action == 7 && b.key == 0x82 && b.modifiers == 7)
                {
                    return Err("Picker hotkey was not registered".into());
                }
                let occupied = glint_platform::HotkeyService::new(
                    vec![glint_platform::HotkeyBinding {
                        action: 77,
                        modifiers: 7,
                        key: 0x83,
                    }],
                    |_| {},
                )?;
                settings
                    .invoke_change("shortcuts.color_picker".into(), "Ctrl+Alt+Shift+F20".into());
                settings.invoke_action("save".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .shortcuts
                    .color_picker
                    != "Ctrl+Alt+Shift+F19"
                    || !hotkeys
                        .bindings()?
                        .iter()
                        .any(|b| b.action == 7 && b.key == 0x82)
                {
                    return Err("Occupied picker key lost the prior usable binding".into());
                }
                drop(occupied);
                settings.invoke_action("cancel".into());
                std::fs::write(directory.join("not-an-output-directory"), b"fixture")
                    .map_err(|e| e.to_string())?;
                for (k, v) in [
                    ("capture.delay_seconds", "0".to_string()),
                    ("capture.include_cursor", "true".into()),
                    ("output.auto_save", "true".into()),
                    ("output.finish", "save_as".into()),
                    (
                        "output.directory",
                        directory
                            .join("not-an-output-directory")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                app.borrow_mut().capture_output = Some(directory.join("must-not-image.png"));
                let value =
                    (!app.borrow().session.preferences.saved().capture.dimensions).to_string();
                settings.invoke_change("capture.dimensions".into(), value.into());
                self.start(settings);
                if settings.get_modal() != 1
                    || !app.borrow().capture_pending
                    || !app.borrow().color_picker
                {
                    return Err("Dirty settings lost independent picker route".into());
                }
                settings.invoke_action("discard-leave".into());
            }
            1 => {
                let ui = capture.unwrap();
                self.sample(&ui, true, 400, 400)?;
                super::capture_snapshot(&ui, &directory.join("color-picker-native.png"))?;
                self.blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                ui.invoke_action("finish".into());
                self.started = Some(Instant::now());
            }
            2 => {
                let ui = capture.unwrap();
                if ui.get_picker_busy() {
                    return self.wait();
                }
                if !ui.get_picker_feedback().starts_with(messages::text(
                    Message::CaptureColorCopyFailed,
                    ui.get_english(),
                )) || ui.get_closed()
                {
                    return Err("Failed color copy did not retain the picker".into());
                }
                super::capture_snapshot(&ui, &directory.join("color-picker-copy-failed.png"))?;
                self.blocker.take();
                ui.invoke_action("copy-color-rgb".into());
                self.started = Some(Instant::now());
            }
            3 => {
                self.copied(app, directory)?;
                settings.invoke_change("capture.delay_seconds".into(), "1".into());
                settings.invoke_action("save".into());
                self.start(settings);
                if app.borrow().countdown.as_ref().map(|c| c.action.as_str())
                    != Some("color-picker")
                {
                    return Err("Picker delay route missing".into());
                }
                settings.invoke_action("capture".into());
                if !app.borrow().color_picker {
                    return Err("Duplicate image capture replaced picker countdown".into());
                }
            }
            4 => {
                if self.started.unwrap().elapsed() < Duration::from_secs(1) {
                    return Err("Picker acquired before delay".into());
                }
                let ui = capture.unwrap();
                self.sample(&ui, false, 500, 400)?;
                ui.invoke_pointer(0, 500., 400.);
                ui.invoke_pointer(2, 500., 400.);
                self.started = Some(Instant::now());
            }
            5 => {
                self.copied(app, directory)?;
                settings.invoke_change("capture.delay_seconds".into(), "0".into());
                settings.invoke_action("save".into());
                self.started = Some(Instant::now());
                settings.invoke_action("capture".into());
            }
            6 => {
                let ui = capture.unwrap();
                if ui.get_color_only() {
                    return Err("Normal capture inherited independent picker mode".into());
                }
                ui.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            7 => {
                settings.invoke_change("capture.delay_seconds".into(), "1".into());
                settings.invoke_action("save".into());
                self.start(settings);
                settings.invoke_action("delay-cancel".into());
            }
            8 => {
                if self.started.unwrap().elapsed() < Duration::from_millis(1400) {
                    return Ok(false);
                }
                if app.borrow().capture.is_some()
                    || app.borrow().countdown.is_some()
                    || app.borrow().capturing
                {
                    return Err("Cancelled picker countdown still acquired source".into());
                }
                settings.invoke_change("capture.delay_seconds".into(), "0".into());
                settings.invoke_action("save".into());
                self.start(settings);
            }
            9 => {
                let ui = capture.unwrap();
                self.sample(&ui, false, 400, 400)?;
                self.blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                ui.invoke_action("copy-color-hex".into());
                if !ui.get_picker_busy() {
                    return Err("Picker clipboard worker did not start".into());
                }
                ui.invoke_action("cancel".into());
                self.cancelled = Some(ui);
                self.started = Some(Instant::now());
            }
            10 => {
                self.blocker.take();
                self.start(settings);
            }
            11 => {
                let old = self.cancelled.take().unwrap();
                let ui = capture.unwrap();
                if !old.get_closed()
                    || !old.get_picker_feedback().starts_with(messages::text(
                        Message::CaptureColorCopying,
                        old.get_english(),
                    ))
                    || ui.get_closed()
                {
                    return Err("Late picker result changed a closed/new session".into());
                }
                self.sample(&ui, false, 400, 400)?;
                ui.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            12 => {
                let old = self.old.take().unwrap();
                for (k, v) in [
                    ("shortcuts.color_picker", old.shortcuts.color_picker),
                    (
                        "capture.delay_seconds",
                        old.capture.delay_seconds.to_string(),
                    ),
                    (
                        "capture.include_cursor",
                        old.capture.include_cursor.to_string(),
                    ),
                    ("output.auto_save", old.output.auto_save.to_string()),
                    (
                        "output.finish",
                        match old.output.finish {
                            glint_core::config::FinishAction::Copy => "copy",
                            glint_core::config::FinishAction::CopyAndPin => "copy_and_pin",
                            glint_core::config::FinishAction::SaveAs => "save_as",
                        }
                        .into(),
                    ),
                    ("output.directory", old.output.directory),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                app.borrow_mut().capture_output = self.output.take();
                app.borrow_mut().last_region = self.remembered.take();
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Picker fixture preferences not restored".into());
                }
                eprintln!(
                    "Independent picker: optional hotkey clear/register/occupied rollback, source HEX/RGB, invalid sample/signed origin, dirty guard, clipboard failure/retry, copy-only Enter/click, delay/cancel/duplicate, late-result isolation and no image output/repeat memory passed"
                );
            }
            _ => return Err("Unknown independent picker stage".into()),
        }
        Ok(true)
    }
}
