//! Actual local decoding and shared native overlay/output; paths replace picker clicks.
use crate::{
    local_image,
    messages::{self, Message},
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

#[derive(Default)]
pub struct Run {
    started: Option<Instant>,
    old: Option<glint_core::config::Config>,
    output: Option<PathBuf>,
    source: Vec<u8>,
    region: String,
    pin_count: usize,
}
impl Run {
    fn open(&mut self, name: &str, ui: &SettingsWindow, app: &Rc<RefCell<AppState>>, dir: &Path) {
        ui.invoke_action("workbench".into());
        local_image::selected(&mut app.borrow_mut(), dir.join(name));
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
                return Err("Local image timeout".into());
            }
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                self.region = format!("{:?}", app.borrow().last_region);
                self.pin_count = app.borrow().pins.borrow().ids().len();
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_change("capture.delay_seconds".into(), "3".into());
                ui.invoke_action("workbench".into());
                if ui.get_modal() != 1 || ui.get_workbench() {
                    return Err("Workbench bypassed dirty guard".into());
                }
                ui.invoke_action("continue".into());
                if ui.get_workbench() {
                    return Err("Continue opened workbench".into());
                }
                ui.invoke_action("workbench".into());
                ui.invoke_action("save-leave".into());
                if !ui.get_workbench() || app.borrow().session.preferences.is_dirty() {
                    return Err("Save did not open workbench".into());
                }
                super::snapshot(ui, &dir.join("workbench-en.png"))?;
                ui.invoke_action("workbench-output".into());
                if ui.get_workbench() || ui.get_category() != 5 {
                    return Err("Workbench output navigation failed".into());
                }
                ui.invoke_action("workbench".into());
                ui.invoke_action("workbench-privacy".into());
                if ui.get_workbench() || ui.get_category() != 7 {
                    return Err("Workbench privacy navigation failed".into());
                }
                ui.invoke_action("workbench".into());
                ui.invoke_change("general.language".into(), "zh_cn".into());
                ui.invoke_action("save".into());
                super::snapshot(ui, &dir.join("workbench-zh.png"))?;
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_action("save".into());
                std::fs::write(dir.join("invalid-image.png"), b"invalid local image")
                    .map_err(|e| e.to_string())?;
                self.open("invalid-image.png", ui, app, dir);
            }
            1 => {
                if app.borrow().capture.is_some()
                    || !ui.get_workbench()
                    || app.borrow().feedback != messages::text(Message::LocalFailed, true)
                {
                    return Err("Invalid local file was accepted or failed without recovery".into());
                }
                let raw = [31u8, 86, 137, 255].repeat(400 * 300);
                image::save_buffer_with_format(
                    dir.join("local-source.png"),
                    &raw,
                    400,
                    300,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    dir.join("local-source.jpg"),
                    &[31u8, 86, 137].repeat(400 * 300),
                    400,
                    300,
                    image::ColorType::Rgb8,
                    image::ImageFormat::Jpeg,
                )
                .map_err(|e| e.to_string())?;
                self.source =
                    std::fs::read(dir.join("local-source.png")).map_err(|e| e.to_string())?;
                std::fs::write(dir.join("local-output.png"), b"existing destination")
                    .map_err(|e| e.to_string())?;
                app.borrow_mut().capture_output = Some(dir.join("local-output.png"));
                self.open("local-source.png", ui, app, dir);
                if app.borrow().countdown.is_some() {
                    return Err("Local image applied screenshot delay".into());
                }
            }
            2 => {
                let state = app.borrow();
                let capture = state.capture.as_ref().ok_or("Local PNG did not open")?;
                if capture.get_wide() != 400.
                    || capture.get_high() != 300.
                    || state.edit_pin.is_some()
                    || capture.get_labels().pin != messages::text(Message::CapturePin, true)
                {
                    return Err(
                        "Local editor has wrong dimensions or replaces an existing pin".into(),
                    );
                }
                let (x, y) = (capture.get_left(), capture.get_top());
                capture.set_tool(6);
                capture.invoke_hover(x + 10., y + 15.);
                if capture.get_sample_position() != "(10, 15)"
                    || capture.get_loupe_position() != "(10, 15)"
                {
                    return Err("Local color coordinates are not relative to the image".into());
                }
                capture.set_tool(1);
                capture.invoke_pointer(0, x + 20., y + 20.);
                capture.invoke_pointer(2, x + 90., y + 70.);
                capture.invoke_action("undo".into());
                if capture.get_object_count() != 0 {
                    return Err("Local undo failed".into());
                }
                capture.invoke_action("redo".into());
                super::capture_snapshot(capture, &dir.join("local-image-overlay.png"))?;
                capture.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            3 => {
                let state = app.borrow();
                let capture = state
                    .capture
                    .as_ref()
                    .ok_or("Output failure discarded local editor")?;
                if capture.get_object_count() != 1
                    || std::fs::read(dir.join("local-output.png")).map_err(|e| e.to_string())?
                        != b"existing destination"
                {
                    return Err("Local save collision lost edits or overwrote file".into());
                }
                std::fs::remove_file(dir.join("local-output.png")).map_err(|e| e.to_string())?;
                capture.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            4 => {
                if app.borrow().capture.is_some() || ui.window().is_visible() {
                    return Err("Successful local save forced workbench".into());
                }
                let out = image::open(dir.join("local-output.png"))
                    .map_err(|e| e.to_string())?
                    .into_rgba8();
                if out.dimensions() != (400, 300)
                    || out.get_pixel(0, 0).0 != [31, 86, 137, 255]
                    || out.get_pixel(40, 40).0 == [31, 86, 137, 255]
                {
                    return Err("Local export did not crop canvas and composite redaction".into());
                }
                self.open("local-source.jpg", ui, app, dir);
            }
            5 => {
                let state = app.borrow();
                let capture = state.capture.as_ref().ok_or("JPEG did not open")?;
                capture.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            6 => {
                if app.borrow().capture.is_some() {
                    return Err("Local copy did not finish".into());
                }
                let clip = glint_platform::read_clipboard_image()?;
                if (clip.width, clip.height) != (400, 300) {
                    return Err("Local clipboard contains canvas matte".into());
                }
                self.open("local-source.png", ui, app, dir);
            }
            7 => {
                app.borrow()
                    .capture
                    .as_ref()
                    .ok_or("Pin input did not open")?
                    .invoke_action("pin".into());
                self.started = Some(Instant::now());
            }
            8 => {
                if app.borrow().capture.is_some()
                    || app.borrow().pins.borrow().ids().len() != self.pin_count + 1
                {
                    return Err("Local image failed to create an independent pin".into());
                }
                self.open("local-source.png", ui, app, dir);
            }
            9 => {
                app.borrow()
                    .capture
                    .as_ref()
                    .ok_or("Cancel input did not open")?
                    .invoke_action("cancel".into());
            }
            10 => {
                if app.borrow().capture.is_some()
                    || ui.window().is_visible()
                    || format!("{:?}", app.borrow().last_region) != self.region
                    || std::fs::read(dir.join("local-source.png")).map_err(|e| e.to_string())?
                        != self.source
                {
                    return Err(
                        "Local workflow changed source, repeat region or cancel route".into(),
                    );
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                app.borrow_mut().capture_output = self.output.take();
                ui.invoke_action("open".into());
                ui.invoke_action("save".into());
                ui.invoke_action("close".into());
                eprintln!(
                    "Local images: workbench dirty guard/bilingual routes, bounded PNG/JPEG decode, image-relative color/magnifier coordinates, annotation undo/redo, save collision/retry/source preservation, clipboard/new pin/cancel, no screenshot delay/region memory passed"
                );
            }
            _ => return Err("Unexpected local image stage".into()),
        }
        Ok(true)
    }
}
