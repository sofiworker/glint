//! Paths substitute picker clicks; confirmation and file publication have separate platform tests.
use crate::{local_image, pins, resident::AppState, settings::SettingsWindow};
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
    output: Option<PathBuf>,
    started: Option<Instant>,
    pin: u64,
    scrolled: bool,
}
impl Run {
    fn open(&mut self, ui: &SettingsWindow, app: &Rc<RefCell<AppState>>, dir: &Path) {
        ui.invoke_action("workbench".into());
        local_image::selected(&mut app.borrow_mut(), dir.join("conflict-source.png"));
        ui.invoke_action("local-image".into());
        self.started = Some(Instant::now());
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if app.borrow().capturing || app.borrow().capture.as_ref().is_some_and(|c| c.get_busy()) {
            if self
                .started
                .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
            {
                return Err("Conflict output timeout".into());
            }
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                ui.window().set_size(slint::PhysicalSize::new(900, 650));
                let saved = app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .output
                    .file_conflict;
                ui.invoke_change("output.file_conflict".into(), "confirm".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .output
                    .file_conflict
                    != saved
                {
                    return Err("Conflict choice bypassed draft".into());
                }
                ui.invoke_action("cancel".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Conflict cancellation lost baseline".into());
                }
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_change("output.file_conflict".into(), "number".into());
                ui.invoke_change("output.auto_save".into(), "false".into());
                ui.invoke_change("output.feedback".into(), "none".into());
                ui.invoke_action("save".into());
                let disk = glint_core::config::Config::from_toml(
                    &std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if disk.output.file_conflict != glint_core::config::FileConflict::Number {
                    return Err("Conflict Save/TOML failed".into());
                }
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(650., 400.),
                        delta_x: 0.,
                        delta_y: -10000.,
                    });
                image::save_buffer_with_format(
                    dir.join("conflict-source.png"),
                    &[31, 86, 137, 255].repeat(400 * 300),
                    400,
                    300,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                std::fs::write(dir.join("conflict-manual.png"), b"existing destination")
                    .map_err(|e| e.to_string())?;
                app.borrow_mut().capture_output = Some(dir.join("conflict-manual.png"));
            }
            1 => {
                if !self.scrolled {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(650., 400.),
                            delta_x: 0.,
                            delta_y: -10000.,
                        });
                    self.scrolled = true;
                    return Ok(false);
                }
                super::snapshot(ui, &dir.join("output-conflict-en.png"))?;
                self.open(ui, app, dir);
            }
            2 | 4 => {
                let capture = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Missing conflict editor")?
                    .clone_strong();
                let (x, y) = (capture.get_left(), capture.get_top());
                capture.set_tool(1);
                capture.invoke_pointer(0, x + 20., y + 20.);
                capture.invoke_pointer(2, x + 90., y + 70.);
                capture.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            3 => {
                if app.borrow().capture.is_some() {
                    return Err("Numbered save did not finish".into());
                }
                if std::fs::read(dir.join("conflict-manual.png")).map_err(|e| e.to_string())?
                    != b"existing destination"
                {
                    return Err("Number policy replaced original".into());
                }
                let pixels = image::open(dir.join("conflict-manual_1.png"))
                    .map_err(|e| e.to_string())?
                    .into_rgba8();
                if pixels.dimensions() != (400, 300)
                    || pixels.get_pixel(0, 0).0 != [31, 86, 137, 255]
                    || pixels.get_pixel(40, 40).0 == [31, 86, 137, 255]
                {
                    return Err("Numbered output lost source/redaction".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_change("output.file_conflict".into(), "confirm".into());
                ui.invoke_action("save".into());
                self.open(ui, app, dir);
            }
            5 => {
                let capture = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Unapproved replacement discarded editor")?
                    .clone_strong();
                if capture.get_object_count() != 1
                    || capture.get_status().is_empty()
                    || std::fs::read(dir.join("conflict-manual.png")).map_err(|e| e.to_string())?
                        != b"existing destination"
                {
                    return Err("Unapproved replacement changed file or lost edits".into());
                }
                super::capture_snapshot(&capture, &dir.join("output-conflict-retained.png"))?;
                capture.invoke_action("cancel".into());
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                ui.invoke_change("general.language".into(), "zh_cn".into());
                ui.invoke_change("output.file_conflict".into(), "number".into());
                ui.invoke_action("save".into());
                ui.window().set_size(slint::PhysicalSize::new(650, 500));
                self.scrolled = false;
            }
            6 => {
                if !self.scrolled {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(400., 250.),
                            delta_x: 0.,
                            delta_y: -10000.,
                        });
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(400., 250.),
                            delta_x: 0.,
                            delta_y: 300.,
                        });
                    self.scrolled = true;
                    return Ok(false);
                }
                super::snapshot(ui, &dir.join("output-conflict-zh-compact.png"))?;
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &[31, 86, 137, 255].repeat(80 * 60),
                    80,
                    60,
                );
                let manager = app.borrow().pins.clone();
                let config = app.borrow().session.preferences.saved().clone();
                self.pin = pins::Manager::create(
                    &manager,
                    slint::Image::from_rgba8(pixels),
                    (700, 250),
                    &config,
                    false,
                    |_| {},
                )?;
                std::fs::write(dir.join("conflict-pin.png"), b"existing pin file")
                    .map_err(|e| e.to_string())?;
                manager
                    .borrow_mut()
                    .diagnostic_choice(Some(dir.join("conflict-pin.png")));
                let pin = manager
                    .borrow()
                    .window(self.pin)
                    .ok_or("Missing conflict pin")?
                    .clone_strong();
                pin.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            7 => {
                let manager = app.borrow().pins.clone();
                let pin = manager
                    .borrow()
                    .window(self.pin)
                    .ok_or("Missing saved pin")?
                    .clone_strong();
                if pin.get_busy() {
                    if self
                        .started
                        .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
                    {
                        return Err("Pin numbered save timeout".into());
                    }
                    return Ok(false);
                }
                if pin.get_output_error()
                    || std::fs::read(dir.join("conflict-pin.png")).map_err(|e| e.to_string())?
                        != b"existing pin file"
                {
                    return Err("Pin conflict policy failed".into());
                }
                let pixels = image::open(dir.join("conflict-pin_1.png"))
                    .map_err(|e| e.to_string())?
                    .into_rgba8();
                if pixels.dimensions() != (80, 60) || pixels.get_pixel(0, 0).0 != [31, 86, 137, 255]
                {
                    return Err("Numbered pin output changed source".into());
                }
                manager.borrow_mut().close(self.pin)?;
                app.borrow_mut().capture_output = self.output.take();
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                ui.invoke_action("close".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Conflict fixture did not restore config".into());
                }
                eprintln!(
                    "Output conflicts: draft/cancel/TOML, real numbered composited capture and pin pixels, unapproved overwrite rejection with retained editing and bilingual compact settings passed; selected paths are synthetic, physical confirmation not exercised"
                );
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}
