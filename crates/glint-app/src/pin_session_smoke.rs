//! Sequential process probes: caller starts seed/restore/failure/clear in separate runs.
use crate::{
    pins::{self, Manager},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
struct Run {
    mode: String,
    phase: u8,
    started: Instant,
    ui: slint::Weak<SettingsWindow>,
    app: Rc<RefCell<AppState>>,
    dir: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    ids: Vec<u64>,
    lock: Option<std::fs::File>,
    original: Vec<u8>,
}
pub fn install(
    mode: String,
    ui: &SettingsWindow,
    app: Rc<RefCell<AppState>>,
    dir: PathBuf,
    error: Rc<RefCell<Option<String>>>,
) {
    let run = Rc::new(RefCell::new(Run {
        mode,
        phase: 0,
        started: Instant::now(),
        ui: ui.as_weak(),
        app,
        dir,
        error,
        ids: vec![],
        lock: None,
        original: vec![],
    }));
    slint::Timer::single_shot(Duration::from_millis(300), move || tick(run));
}
fn tick(run: Rc<RefCell<Run>>) {
    let outcome = run.borrow_mut().step();
    match outcome {
        Ok(true) => {}
        Ok(false) => slint::Timer::single_shot(Duration::from_millis(150), move || tick(run)),
        Err(error) => {
            *run.borrow().error.borrow_mut() = Some(error);
            let _ = slint::quit_event_loop();
        }
    }
}
fn shot(ui: &SettingsWindow, path: PathBuf) -> Result<(), String> {
    let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    image::save_buffer_with_format(
        path,
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())
}
impl Run {
    fn step(&mut self) -> Result<bool, String> {
        if self.started.elapsed() > Duration::from_secs(30) {
            return Err(format!(
                "Session process probe {} phase {} timed out",
                self.mode, self.phase
            ));
        }
        let ui = self.ui.upgrade().ok_or("Probe settings closed")?;
        if self.app.borrow().pin_session.busy() {
            return Ok(false);
        }
        let manager = self.app.borrow().pins.clone();
        if matches!(self.mode.as_str(), "seed" | "off") {
            match self.phase {
                0 => {
                    let mut config = self.app.borrow().session.preferences.saved().clone();
                    config.general.onboarding_complete = true;
                    config.general.language = glint_core::config::Language::En;
                    config.appearance.theme = glint_core::config::Theme::Dark;
                    config.shortcuts.capture.clear();
                    config.shortcuts.window.clear();
                    config.shortcuts.last_region.clear();
                    config.shortcuts.pin_clipboard.clear();
                    config.shortcuts.toggle_pins.clear();
                    config.shortcuts.color_picker.clear();
                    config.shortcuts.settings.clear();
                    config.shortcuts.rescue = "Ctrl+Alt+Shift+F18".into();
                    config.pins.restore_session = self.mode == "seed";
                    config.pins.opacity = 55;
                    config.pins.zoom_percent = 75;
                    config.pins.on_top = false;
                    config.pins.passthrough = false;
                    self.app.borrow_mut().session.preferences.draft = config.clone();
                    ui.invoke_action("save".into());
                    if self.app.borrow().session.preferences.is_dirty() {
                        return Err("Probe settings did not save".into());
                    }
                    let raw = [31u8, 86, 137, 255].repeat(400 * 300);
                    let mut edits = glint_core::Edits::default();
                    edits.push(glint_core::Redaction {
                        bounds: glint_core::Rect {
                            x: 20,
                            y: 20,
                            width: 60,
                            height: 40,
                        },
                        color: [2, 3, 4, 255],
                    });
                    let doc = Arc::new(
                        glint_core::ImageDocument::from_selection(
                            &raw,
                            400,
                            300,
                            glint_core::Rect {
                                x: 0,
                                y: 0,
                                width: 400,
                                height: 300,
                            },
                            &edits,
                        )
                        .map_err(str::to_owned)?,
                    );
                    let pixels = glint_core::composite_selection(
                        &raw,
                        400,
                        300,
                        glint_core::Rect {
                            x: 0,
                            y: 0,
                            width: 400,
                            height: 300,
                        },
                        &edits,
                    )
                    .map_err(str::to_owned)?;
                    let id = Manager::create_document(
                        &manager,
                        crate::probe::PinPayload {
                            pixels: SharedPixelBuffer::clone_from_slice(&pixels, 400, 300),
                            display: SharedPixelBuffer::clone_from_slice(&pixels, 400, 300),
                            turns: 0,
                            document: doc,
                        },
                        (60, 70),
                        &config,
                        true,
                        |_| {},
                    )?;
                    self.ids.push(id);
                    let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        &[200u8, 63, 99, 255].repeat(160 * 120),
                        160,
                        120,
                    );
                    let id = Manager::create(
                        &manager,
                        slint::Image::from_rgba8(pixels),
                        (160, 170),
                        &config,
                        true,
                        |_| {},
                    )?;
                    self.ids.push(id);
                }
                1 => {
                    if self
                        .ids
                        .iter()
                        .any(|id| manager.borrow().window(*id).is_none_or(|w| !w.get_ready()))
                    {
                        return Ok(false);
                    }
                    let pin = manager.borrow().window(self.ids[0]).unwrap().clone_strong();
                    pin.invoke_action("rotate".into());
                }
                2 => {
                    let pin = manager
                        .borrow()
                        .window(self.ids[0])
                        .ok_or("Seed pin missing")?
                        .clone_strong();
                    if pin.get_busy() {
                        return Ok(false);
                    }
                    if pin.get_quarter_turns() != 1 {
                        return Err("Seed rotation failed".into());
                    }
                    pin.invoke_action("lock".into());
                    pin.invoke_change_opacity(45.);
                    pins::native(&pin, |w| w.position(-100_000, 50_000))?;
                    let second = manager
                        .borrow()
                        .window(self.ids[1])
                        .ok_or("Second seed pin missing")?
                        .clone_strong();
                    second.invoke_action("pass".into());
                    second.invoke_action("confirm-pass".into());
                    second.invoke_action("hide".into());
                    ui.invoke_action("quit".into());
                    return Ok(true);
                }
                _ => return Err("Unexpected seed phase".into()),
            }
        } else if self.mode == "corrupt" || self.mode == "config-recovery" {
            if !self.app.borrow().pin_session.protect_saved || !manager.borrow().ids().is_empty() {
                return Err("Corrupt session was accepted or lost overwrite protection".into());
            }
            ui.invoke_action("quit".into());
            return Ok(true);
        } else {
            if self.phase == 0 {
                self.ids = manager.borrow().ids();
                if self.ids.len() != 2 || !self.app.borrow().pin_session.loaded {
                    return Err("Restart did not restore both pins".into());
                }
                let pins = manager.borrow();
                let first = pins.window(self.ids[0]).unwrap();
                let second = pins.window(self.ids[1]).unwrap();
                let doc = pins
                    .document(self.ids[0])
                    .ok_or("Restored objects missing")?;
                if first.get_quarter_turns() != 1
                    || !first.get_locked()
                    || first.get_on_top()
                    || (first.get_alpha() - 0.45).abs() > 0.001
                    || first.get_passthrough()
                    || second.get_passthrough()
                    || !first.window().is_visible()
                    || !second.window().is_visible()
                    || doc.edits().marks().len() != 1
                    || doc.edits().can_undo()
                    || doc.source() != [31u8, 86, 137, 255].repeat(400 * 300)
                {
                    return Err("Restart lost source/objects/window properties or restored click-through/history".into());
                }
                let flat = glint_core::composite_selection(
                    doc.source(),
                    400,
                    300,
                    glint_core::Rect {
                        x: 0,
                        y: 0,
                        width: 400,
                        height: 300,
                    },
                    doc.edits(),
                )
                .map_err(str::to_owned)?;
                let expected =
                    image::imageops::rotate90(&image::RgbaImage::from_raw(400, 300, flat).unwrap());
                if first.get_picture().to_rgba8().unwrap().as_bytes() != expected.as_raw() {
                    return Err("Restored rotation/annotations differ from source composite".into());
                }
                pins::native(first, |w| {
                    let s = w.state()?;
                    let (x, y, ww, wh) = w.work_area()?;
                    if s.x < x || s.y < y || s.x >= x + ww as i32 || s.y >= y + wh as i32 {
                        return Err("Restored off-screen pin was not recovered".into());
                    }
                    Ok(())
                })?;
                drop(pins);
                ui.invoke_action("open".into());
                ui.invoke_choose_page(6);
                ui.window().set_size(slint::PhysicalSize::new(900, 650));
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(640., 400.),
                        delta_x: 0.,
                        delta_y: -10000.,
                    });
                if self.mode == "failure" || self.mode == "without" {
                    self.original =
                        std::fs::read(self.dir.join("pins.session")).map_err(|e| e.to_string())?;
                    use std::os::windows::fs::OpenOptionsExt;
                    self.lock = Some(
                        std::fs::OpenOptions::new()
                            .read(true)
                            .share_mode(0)
                            .open(self.dir.join("pins.session"))
                            .map_err(|e| e.to_string())?,
                    );
                    ui.invoke_action("quit".into());
                } else if self.mode == "clear" {
                    ui.invoke_action("pin-session-clear".into());
                    if ui.get_modal() != 5 {
                        return Err("Clear did not confirm scope".into());
                    }
                    ui.invoke_action("capture".into());
                    if ui.get_modal() != 5 || self.app.borrow().capturing {
                        return Err("Capture discarded session clear confirmation".into());
                    }
                    ui.invoke_action("continue".into());
                    if !self.dir.join("pins.session").exists() {
                        return Err("Cancel deleted session".into());
                    }
                    ui.invoke_action("pin-session-clear".into());
                    ui.invoke_action("pin-session-clear-confirm".into());
                }
            } else if self.mode == "failure" || self.mode == "without" {
                if self.phase == 1 {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(640., 400.),
                            delta_x: 0.,
                            delta_y: -10000.,
                        });
                    self.phase += 1;
                    return Ok(false);
                }
                if !self.app.borrow().pin_session.quit_failed
                    || manager
                        .borrow()
                        .ids()
                        .iter()
                        .any(|id| !manager.borrow().window(*id).unwrap().window().is_visible())
                {
                    return Err("Locked file failure discarded/kept hidden pins or quit".into());
                }
                self.lock.take();
                if std::fs::read(self.dir.join("pins.session")).map_err(|e| e.to_string())?
                    != self.original
                {
                    return Err("Failed session save changed prior bytes".into());
                }
                shot(&ui, self.dir.join("session-save-failed.png"))?;
                ui.invoke_action("pin-session-exit-without".into());
                if ui.get_modal() != 6 {
                    return Err("Exit without save did not confirm loss".into());
                }
                ui.invoke_action("continue".into());
                if self.mode == "without" {
                    ui.invoke_action("pin-session-exit-without".into());
                    ui.invoke_action("pin-session-exit-confirm".into());
                } else {
                    ui.invoke_action("pin-session-retry-exit".into());
                }
                return Ok(true);
            } else if self.mode == "clear" {
                if self.dir.join("pins.session").exists() || manager.borrow().ids().len() != 2 {
                    return Err("Clear kept assets or closed current pins".into());
                }
                ui.invoke_change("pins.restore_session".into(), "false".into());
                ui.invoke_action("save".into());
                ui.invoke_action("quit".into());
                return Ok(true);
            } else if self.mode == "empty" {
                for id in &self.ids {
                    let pin = manager.borrow().window(*id).unwrap().clone_strong();
                    pin.invoke_action("close".into());
                }
                ui.invoke_action("quit".into());
                return Ok(true);
            } else if self.mode == "restore" {
                if self.phase == 1 {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(640., 400.),
                            delta_x: 0.,
                            delta_y: -10000.,
                        });
                    self.phase += 1;
                    return Ok(false);
                }
                match self.phase {
                    2 => {
                        shot(&ui, self.dir.join("session-restored.png"))?;
                        ui.invoke_change("general.language".into(), "zh_cn".into());
                    }
                    3 => {
                        ui.window()
                            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                                position: slint::LogicalPosition::new(640., 400.),
                                delta_x: 0.,
                                delta_y: -10000.,
                            });
                    }
                    4 => {
                        shot(&ui, self.dir.join("session-restored-zh.png"))?;
                        ui.invoke_action("pin-session-clear".into());
                        if ui.get_modal() != 5 {
                            return Err("Chinese clear confirmation missing".into());
                        }
                    }
                    5 => {
                        shot(&ui, self.dir.join("session-clear-confirm-zh.png"))?;
                        ui.invoke_action("continue".into());
                        ui.invoke_action("cancel".into());
                        ui.invoke_action("quit".into());
                        return Ok(true);
                    }
                    _ => return Err("Unexpected restore phase".into()),
                }
            } else {
                return Err("Unknown session probe mode".into());
            }
        }
        self.phase += 1;
        Ok(false)
    }
}
