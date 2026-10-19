//! Saved display preferences, real frozen-source sampling and export separation.
use crate::{resident::AppState, settings::SettingsWindow};
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
    started: Option<Instant>,
}
impl Run {
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let capture = app.borrow().capture.as_ref().map(|c| c.clone_strong());
        if ((stage == 1 || stage == 3) && capture.is_none())
            || ((stage == 2 || stage == 4) && (capture.is_some() || app.borrow().capturing))
        {
            return if self
                .started
                .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
            {
                Err("Display capture/output timed out".into())
            } else {
                Ok(false)
            };
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                self.remembered = app.borrow().last_region.clone();
                for (k, v) in [
                    ("capture.magnifier", "true"),
                    ("capture.dimensions", "true"),
                    ("capture.dim_percent", "80"),
                    ("capture.delay_seconds", "0"),
                    ("output.auto_save", "false"),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                for (k, v) in [
                    ("capture.magnifier", "false"),
                    ("capture.dimensions", "false"),
                    ("capture.dim_percent", "20"),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("cancel".into());
                let a = app.borrow();
                let c = &a.session.preferences.draft.capture;
                if !c.magnifier || !c.dimensions || c.dim_percent != 80 {
                    return Err("Display setting cancel failed".into());
                }
                drop(a);
                app.borrow_mut().capture_output = Some(directory.join("magnifier-source.png"));
                settings.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            1 => {
                let ui = capture.unwrap();
                if !ui.get_magnifier_enabled() || !ui.get_dimensions() || ui.get_dim_percent() != 80
                {
                    return Err("Saved display presentation missing".into());
                }
                let src = ui.get_desktop().to_rgba8().ok_or("Missing source")?;
                for (x, y) in [
                    (0., 0.),
                    (src.width() as f32 - 1., 0.),
                    (0., src.height() as f32 - 1.),
                    (src.width() as f32 - 1., src.height() as f32 - 1.),
                    (800., 400.),
                ] {
                    ui.invoke_hover(x, y);
                    let expected = glint_core::magnifier_rgba(
                        src.as_bytes(),
                        src.width(),
                        src.height(),
                        x as u32,
                        y as u32,
                    )
                    .unwrap();
                    let actual = ui
                        .get_loupe_image()
                        .to_rgba8()
                        .ok_or("Missing magnifier pixels")?;
                    if actual.as_bytes() != expected || !ui.get_loupe_visible() {
                        return Err("Frozen-source magnifier pixel/visibility mismatch".into());
                    }
                    if ui.get_loupe_x() < 0.
                        || ui.get_loupe_y() < 0.
                        || ui.get_loupe_x() + 176. > ui.window().size().width as f32
                        || ui.get_loupe_y() + 184. > ui.window().size().height as f32
                    {
                        return Err("Magnifier escaped viewport".into());
                    }
                }
                ui.invoke_hover(800.4, 400.4);
                if ui.get_loupe_position().is_empty() {
                    return Err("Missing source coordinates".into());
                }
                ui.invoke_canvas_left();
                if ui.get_loupe_visible() {
                    return Err("Magnifier did not hide on canvas leave".into());
                }
                ui.invoke_hover(800., 400.);
                if !ui.get_loupe_visible() {
                    return Err("Cached sample did not reappear".into());
                }
                ui.invoke_hover(f32::NAN, 400.);
                if ui.get_loupe_visible() {
                    return Err("Invalid sample remained visible".into());
                }
                ui.invoke_pointer(0, 200., 200.);
                ui.invoke_pointer(2, 600., 500.);
                ui.invoke_hover(400., 400.);
                if !ui.get_status().contains("400 × 300 px") {
                    return Err("Dimensions missing".into());
                }
                for position in 0..3 {
                    ui.set_toolbar_position(position);
                    ui.invoke_hover(400., 400.);
                    let (x, y) = (ui.get_loupe_x(), ui.get_loupe_y());
                    if !ui.get_loupe_visible()
                        || !(x + 180. <= ui.get_toolbar_x()
                            || x >= ui.get_toolbar_x() + ui.get_toolbar_width() + 4.
                            || y + 188. <= ui.get_toolbar_y()
                            || y >= ui.get_toolbar_y() + ui.get_toolbar_total_height() + 4.)
                    {
                        return Err("Magnifier overlaps toolbar".into());
                    }
                }
                ui.set_toolbar_position(0);
                super::capture_snapshot(&ui, &directory.join("magnifier-native.png"))?;
                self.pixels.clear();
                for y in 200..500 {
                    let i = ((y * src.width() + 200) * 4) as usize;
                    self.pixels.extend_from_slice(&src.as_bytes()[i..i + 1600]);
                }
                ui.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            2 => {
                let p = image::open(directory.join("magnifier-source.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if p.dimensions() != (400, 300) || p.as_raw() != &self.pixels {
                    return Err("Display UI leaked into exported source".into());
                }
                for (k, v) in [
                    ("capture.magnifier", "false"),
                    ("capture.dimensions", "false"),
                    ("capture.dim_percent", "20"),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                settings.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            3 => {
                let ui = capture.unwrap();
                ui.invoke_pointer(0, 200., 200.);
                ui.invoke_pointer(2, 600., 500.);
                ui.invoke_hover(400., 400.);
                if ui.get_magnifier_enabled()
                    || ui.get_dimensions()
                    || ui.get_dim_percent() != 20
                    || ui.get_loupe_visible()
                    || ui.get_status().contains("px")
                {
                    return Err("Disabled display settings ignored".into());
                }
                super::capture_snapshot(&ui, &directory.join("magnifier-disabled.png"))?;
                ui.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            4 => {
                let old = self.old.take().unwrap();
                for (k, v) in [
                    ("capture.magnifier", old.capture.magnifier.to_string()),
                    ("capture.dimensions", old.capture.dimensions.to_string()),
                    ("capture.dim_percent", old.capture.dim_percent.to_string()),
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
                    return Err("Display fixture did not restore".into());
                }
                eprintln!(
                    "Display preferences: exact frozen 11x11 source/edges, invalid input/leave/cache, toolbar avoidance, dimensions/dimming, PNG pixel equality passed"
                );
            }
            _ => return Err("Unknown display stage".into()),
        }
        Ok(true)
    }
}
