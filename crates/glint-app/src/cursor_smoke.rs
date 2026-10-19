//! Real resident cursor-enabled capture, frozen preview and flattened outputs.
use crate::{probe, resident::AppState, settings::SettingsWindow};
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
    remembered: Option<glint_core::region_memory::RememberedRegion>,
    pixels: Vec<u8>,
    size: (u32, u32),
    started: Option<Instant>,
}
impl Run {
    fn wait(&self) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|s| s.elapsed() > Duration::from_secs(20))
        {
            Err("Cursor capture/output did not finish".into())
        } else {
            Ok(false)
        }
    }
    fn selected_source(&mut self, ui: &probe::CaptureProbe) -> Result<(), String> {
        let src = ui
            .get_desktop()
            .to_rgba8()
            .ok_or("No cursor-enabled desktop source")?;
        let env = glint_platform::desktop_environment()?;
        let left = env.displays.iter().map(|d| d.bounds.left).min().unwrap();
        let top = env.displays.iter().map(|d| d.bounds.top).min().unwrap();
        let point = glint_platform::cursor_position()?;
        let x = (i64::from(point.0) - i64::from(left) - 60)
            .clamp(0, i64::from(src.width().saturating_sub(200))) as u32;
        let y = (i64::from(point.1) - i64::from(top) - 60)
            .clamp(0, i64::from(src.height().saturating_sub(160))) as u32;
        ui.set_tool(0);
        ui.invoke_pointer(0, x as f32, y as f32);
        ui.invoke_pointer(2, (x + 200) as f32, (y + 160) as f32);
        if !ui.get_selected() {
            return Err("Cursor export region not selected".into());
        }
        self.size = (200, 160);
        self.pixels.clear();
        for row in y..y + 160 {
            let start = ((row * src.width() + x) * 4) as usize;
            self.pixels
                .extend_from_slice(&src.as_bytes()[start..start + 800]);
        }
        Ok(())
    }
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let capture = app.borrow().capture.as_ref().map(|c| c.clone_strong());
        if [1, 3, 5].contains(&stage) && capture.is_none() {
            return self.wait();
        }
        if [2, 4, 6].contains(&stage) && (capture.is_some() || app.borrow().capturing) {
            return self.wait();
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                self.remembered = app.borrow().last_region.clone();
                for (k, v) in [
                    ("capture.include_cursor", "true"),
                    ("capture.delay_seconds", "0"),
                    ("output.auto_save", "false"),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                settings.invoke_change("capture.include_cursor".into(), "false".into());
                settings.invoke_action("cancel".into());
                if !app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .capture
                    .include_cursor
                    || !app
                        .borrow()
                        .session
                        .preferences
                        .draft
                        .capture
                        .include_cursor
                {
                    return Err("Cursor save/cancel failed".into());
                }
                app.borrow_mut().capture_output = Some(directory.join("cursor-source.png"));
                self.started = Some(Instant::now());
                settings.invoke_action("capture".into());
            }
            1 => {
                let ui = capture.unwrap();
                self.selected_source(&ui)?;
                let p = ui.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    directory.join("cursor-native.png"),
                    p.as_bytes(),
                    p.width(),
                    p.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                ui.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            2 => {
                let p = glint_platform::read_clipboard_image()?;
                if (p.width, p.height) != self.size || p.rgba != self.pixels {
                    return Err(
                        "Cursor-enabled frozen preview differs from clipboard pixels".into(),
                    );
                }
                self.started = Some(Instant::now());
                settings.invoke_action("capture".into());
            }
            3 => {
                let ui = capture.unwrap();
                self.selected_source(&ui)?;
                ui.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            4 => {
                let p = image::open(directory.join("cursor-source.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if (p.width(), p.height()) != self.size || p.as_raw() != &self.pixels {
                    return Err("Cursor-enabled source differs from PNG pixels".into());
                }
                settings.invoke_change("capture.include_cursor".into(), "false".into());
                settings.invoke_action("save".into());
                self.started = Some(Instant::now());
                settings.invoke_action("capture".into());
            }
            5 => {
                if app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .capture
                    .include_cursor
                {
                    return Err("Cursor disabled setting not applied".into());
                }
                capture.unwrap().invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            6 => {
                let old = self.old.take().unwrap();
                for (k, v) in [
                    (
                        "capture.include_cursor",
                        old.capture.include_cursor.to_string(),
                    ),
                    (
                        "capture.delay_seconds",
                        old.capture.delay_seconds.to_string(),
                    ),
                    ("output.auto_save", old.output.auto_save.to_string()),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                app.borrow_mut().capture_output = self.output.take();
                app.borrow_mut().last_region = self.remembered.take();
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Cursor fixture preferences did not restore".into());
                }
            }
            _ => return Err("Unknown cursor stage".into()),
        }
        Ok(true)
    }
}
