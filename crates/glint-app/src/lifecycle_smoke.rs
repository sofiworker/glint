//! Inject coordinator events; no actual lock, suspend or RDP operation is performed.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_core::config::Config;
use slint::{ComponentHandle, Model};
use std::{cell::RefCell, path::Path, rc::Rc};

#[derive(Default)]
pub struct Run {
    old: Option<Config>,
    retained: Option<slint::Weak<crate::probe::CaptureProbe>>,
    started: Option<std::time::Instant>,
    released: Option<slint::Weak<crate::probe::CaptureProbe>>,
    generation: u64,
    save_started: bool,
    retry_editor: Option<crate::probe::CaptureProbe>,
}
impl Run {
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if stage != 4 && app.borrow().capturing {
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                if app.borrow().lifecycle.guard.is_none() {
                    return Err("Native lifecycle subscription unavailable".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_choose_page(9);
                if !ui
                    .get_rows()
                    .iter()
                    .any(|r| r.key == "resources.sleep_release" && r.editable)
                {
                    return Err("Lifecycle release option disabled".into());
                }
                ui.invoke_change("resources.sleep_release".into(), "false".into());
                if app.borrow().session.preferences.saved() != self.old.as_ref().unwrap() {
                    return Err("Lifecycle draft changed saved policy".into());
                }
                ui.invoke_action("cancel".into());
                for (key, value) in [
                    ("capture.delay_seconds", "0"),
                    ("capture.detect_window", "false"),
                    ("resources.sleep_release", "false"),
                ] {
                    ui.invoke_change(key.into(), value.into());
                }
                ui.invoke_action("save".into());
                ui.invoke_action("capture".into());
            }
            1 => {
                let c = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Retained capture missing")?
                    .clone_strong();
                if self.save_started {
                    if c.get_busy() {
                        if self.started.unwrap().elapsed().as_secs() > 5 {
                            return Err(
                                "Hidden save preparation left retained editor permanently busy"
                                    .into(),
                            );
                        }
                        return Ok(false);
                    }
                    if c.get_closed()
                        || c.window().is_visible()
                        || c.get_status()
                            != crate::messages::text(
                                crate::messages::Message::CaptureSaveCancelled,
                                c.get_english(),
                            )
                        || c.get_object_count() != 1
                        || !c.get_can_undo()
                    {
                        return Err("Hidden save preparation lost its retained editor or cancellation feedback".into());
                    }
                    // The worker is finished: no image was published and its slot is reusable.
                    if std::fs::read_dir(dir.join("hidden-save-preparation"))
                        .map_err(|e| e.to_string())?
                        .next()
                        .is_some()
                    {
                        return Err("Hidden save preparation published an image".into());
                    }
                    ui.invoke_action("lifecycle-unlock".into());
                    if !c.window().is_visible() {
                        return Err("Retained editor did not resume".into());
                    }
                    super::capture_snapshot(&c, &dir.join("hidden-save-recovered.png"))?;
                    self.retry_editor = Some(c.clone_strong());
                    c.invoke_action("copy".into());
                    if !c.get_busy() {
                        return Err("Recovered editor could not retry output".into());
                    }
                    ui.invoke_action("lifecycle-lock".into());
                    return Ok(true);
                }
                c.set_tool(0);
                c.invoke_pointer(0, 200., 200.);
                c.invoke_pointer(2, 600., 500.);
                c.set_tool(1);
                c.invoke_pointer(0, 250., 250.);
                c.invoke_pointer(2, 350., 350.);
                self.retained = Some(c.as_weak());
                verify_interrupted_gestures(&c, ui, dir)?;
                ui.invoke_action("lifecycle-lock".into());
                ui.invoke_action("lifecycle-sleep".into());
                ui.invoke_action("lifecycle-resume".into());
                if !app.borrow().lifecycle.blocked()
                    || c.window().is_visible()
                    || c.get_object_count() != 1
                {
                    return Err("Sleep resume lost locked retained editor".into());
                }
                ui.invoke_action("capture".into());
                if app.borrow().capturing {
                    return Err("Capture started while locked".into());
                }
                ui.invoke_action("lifecycle-unlock".into());
                if app.borrow().lifecycle.blocked()
                    || !c.window().is_visible()
                    || c.get_object_count() != 1
                    || !c.get_can_undo()
                {
                    return Err("Off policy did not restore retained editor/history".into());
                }
                crate::probe::start_save_preparation_probe(
                    &c,
                    &dir.join("hidden-save-preparation"),
                )?;
                self.save_started = true;
                self.started = Some(std::time::Instant::now());
                ui.invoke_action("lifecycle-lock".into());
                return Ok(false);
            }
            2 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                if !app.borrow().lifecycle.blocked() || app.borrow().capture_focus.is_some() {
                    return Err(
                        "Hidden capture completion restored old focus or cleared lock".into(),
                    );
                }
                let editor = self
                    .retry_editor
                    .take()
                    .ok_or("Missing recovered output receipt")?;
                if !editor.get_closed() || editor.get_busy() || editor.get_completion_kind() != 1 {
                    return Err(
                        "Recovered editor copy did not finish successfully while hidden".into(),
                    );
                }
                let copied = glint_platform::read_clipboard_image()?;
                let p = ((100 * copied.width + 100) * 4) as usize;
                if (copied.width, copied.height) != (400, 300)
                    || copied.rgba[p..p + 4] != [32, 43, 66, 255]
                {
                    return Err("Recovered output lost crop or flattened redaction".into());
                }
                std::fs::write(dir.join("HIDDEN-SAVE-RECOVERY-PASSED.txt"), "Real save-directory worker completed while lifecycle-hidden; busy cleared, cancellation visible on resume, crop/mark/history preserved, no image published, output slot reusable, retry clipboard pixels include flattened redaction; late hidden copy completion does not restore focus. Lifecycle events injected; not physical lock/sleep acceptance.\n").map_err(|e|e.to_string())?;
                ui.invoke_action("lifecycle-unlock".into());
                ui.invoke_action("open".into());
                ui.invoke_change("resources.sleep_release".into(), "true".into());
                ui.invoke_action("save".into());
                ui.invoke_action("capture".into());
            }
            3 => {
                if self.released.is_none() {
                    let c = app
                        .borrow()
                        .capture
                        .as_ref()
                        .ok_or("Release capture missing")?
                        .as_weak();
                    let generation = app.borrow().lifecycle.generation;
                    self.generation = generation;
                    ui.invoke_action("lifecycle-disconnect".into());
                    ui.invoke_action("lifecycle-lock".into());
                    ui.invoke_action("lifecycle-connect".into());
                    if !app.borrow().lifecycle.blocked() || app.borrow().capture.is_some() {
                        return Err(
                        "Release failed to drop editor/source or disconnected gate cleared lock"
                            .into(),
                    );
                    }
                    self.released = Some(c);
                    self.started = Some(std::time::Instant::now());
                    return Ok(false);
                }
                if self.released.as_ref().unwrap().upgrade().is_some() {
                    if self.started.unwrap().elapsed().as_secs() > 5 {
                        return Err("Released capture component/source retained after5s".into());
                    }
                    return Ok(false);
                }
                ui.invoke_action("lifecycle-unlock".into());
                ui.invoke_action(format!("capture-finished:{}", self.generation).into());
                ui.invoke_action("capture".into());
                // Real acquisition has started on the bounded worker. Suspend before
                // its event-loop delivery even if the worker itself finishes quickly.
                if !app.borrow().capturing {
                    return Err("No real capture worker to interrupt".into());
                }
                ui.invoke_action("lifecycle-sleep".into());
                ui.invoke_action("lifecycle-resume".into());
                if !app.borrow().lifecycle.discard_ready || !app.borrow().capturing {
                    return Err("Suspend released worker slot before its result".into());
                }
                self.started = Some(std::time::Instant::now());
            }
            4 => {
                if app.borrow().capturing {
                    if self.started.unwrap().elapsed().as_secs() > 15 {
                        return Err("Interrupted capture worker failed to finish".into());
                    }
                    return Ok(false);
                }
                if app.borrow().capture.is_some()
                    || app.borrow().capture_result.is_some()
                    || app.borrow().lifecycle.discard_ready
                {
                    return Err("Late capture source reopened after resume".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_change("capture.delay_seconds".into(), "5".into());
                ui.invoke_action("save".into());
                ui.invoke_action("capture".into());
                if app.borrow().countdown.is_none() {
                    return Err("No countdown to interrupt".into());
                }
                app.borrow_mut().guide_practice = true;
                ui.invoke_action("lifecycle-lock".into());
                ui.invoke_action("lifecycle-unlock".into());
                if app.borrow().countdown.is_some()
                    || app.borrow().delay_elapsed
                    || app.borrow().capturing
                {
                    return Err("Countdown replayed after unlock".into());
                }
                if app.borrow().guide_practice || ui.get_guide_step() != 4 {
                    return Err("Interrupted practice retained an active generation".into());
                }
                ui.set_guide_step(-1);
                ui.invoke_action("delay-ready".into());
                if app.borrow().capturing {
                    return Err("Late countdown callback began capture".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_choose_page(9);
            }
            5 => {
                let disk = Config::from_toml(
                    &std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if !disk.resources.sleep_release {
                    return Err("Lifecycle policy not persisted".into());
                }
                for (key, value) in [
                    ("general.language", "en"),
                    ("appearance.theme", "dark"),
                    ("appearance.high_contrast", "true"),
                ] {
                    ui.invoke_change(key.into(), value.into());
                }
                ui.invoke_action("save".into());
                ui.window().set_size(slint::PhysicalSize::new(900, 650));
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(500., 400.),
                        delta_x: 0.,
                        delta_y: -230.,
                    });
            }
            6 => {
                super::snapshot(ui, &dir.join("lifecycle-settings-en.png"))?;
                ui.invoke_change("general.language".into(), "zh_cn".into());
                super::snapshot(ui, &dir.join("lifecycle-settings-zh.png"))?;
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Lifecycle fixture restoration failed".into());
                }
                eprintln!(
                    "Lifecycle: real WTS subscription, injected lock/disconnect/sleep independent gates, saved on/off, retained editing/history, release/drop, actual capture worker late-result discard/bounded slot, countdown cancellation/no replay and stale completion generation passed; actual lock/sleep/RDP not exercised"
                );
            }
            _ => return Err("Unknown lifecycle stage".into()),
        }
        Ok(true)
    }
}

fn verify_interrupted_gestures(
    c: &crate::probe::CaptureProbe,
    settings: &SettingsWindow,
    dir: &Path,
) -> Result<(), String> {
    // Keep both history branches, then interrupt callbacks before a pointer-up.
    let magnifier = c.get_magnifier_enabled();
    c.set_magnifier_enabled(false);
    c.invoke_pointer(0, 400., 250.);
    c.invoke_pointer(2, 450., 300.);
    c.invoke_action("undo".into());
    let region = (c.get_left(), c.get_top(), c.get_wide(), c.get_high());
    let source = c
        .get_desktop()
        .to_rgba8()
        .ok_or("Missing lifecycle frozen source")?;
    image::save_buffer(
        dir.join("lifecycle-gesture-source.png"),
        source.as_bytes(),
        source.width(),
        source.height(),
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())?;
    let slot = crate::probe::OutputSlot::acquire().ok_or("Gesture recovery fixture slot busy")?;
    for (name, tool, start, end) in [
        ("draw", 1, (450., 400.), (500., 450.)),
        ("object", 0, (280., 280.), (300., 300.)),
        ("move", 0, (550., 450.), (580., 470.)),
        ("resize", 0, (600., 400.), (650., 420.)),
        ("replace", 0, (100., 400.), (140., 450.)),
    ] {
        c.set_tool(tool);
        c.invoke_pointer(0, start.0, start.1);
        c.invoke_pointer(1, end.0, end.1);
        c.invoke_action("copy".into());
        if c.get_status()
            != crate::messages::text(
                crate::messages::Message::CaptureGesturePending,
                c.get_english(),
            )
        {
            return Err(format!(
                "Lifecycle {name} fixture did not start a pending gesture"
            ));
        }
        settings.invoke_action("lifecycle-lock".into());
        settings.invoke_action("lifecycle-unlock".into());
        c.invoke_pointer(2, end.0, end.1);
        c.invoke_action("copy".into());
        if c.get_status()
            != crate::messages::text(
                crate::messages::Message::CaptureOutputPending,
                c.get_english(),
            )
            || c.get_busy()
            || !c.window().is_visible()
            || (c.get_left(), c.get_top(), c.get_wide(), c.get_high()) != region
            || c.get_object_count() != 1
            || !c.get_can_undo()
            || !c.get_can_redo()
        {
            return Err(format!(
                "Lifecycle {name} did not discard gesture while retaining crop/marks/history: {}",
                c.get_status()
            ));
        }
        c.set_tool(1); // Hide selection/object grips for independent pixel checks.
        super::capture_snapshot(c, &dir.join(format!("lifecycle-gesture-{name}.png")))?;
    }
    drop(slot);
    c.invoke_action("redo".into());
    if c.get_object_count() != 2 {
        return Err("Lifecycle cancellation lost redo command".into());
    }
    c.invoke_action("undo".into());
    std::fs::write(dir.join("LIFECYCLE-GESTURES-PASSED.txt"), "Five pending callback gestures interrupted by coordinator lock/unlock: draw, object move, selection move/resize/replace. Crop, committed marks and both histories retained; late pointer-up ignored; output can acquire shared slot after resume. Real lock/sleep and physical capture loss not exercised.\n").map_err(|e|e.to_string())?;
    c.set_magnifier_enabled(magnifier);
    Ok(())
}
