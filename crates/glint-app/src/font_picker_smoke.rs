//! Actual installed families and native text raster/export; callback-driven UI probe.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_core::config::Config;
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Instant,
};

#[derive(Default)]
pub struct Run {
    old: Option<Config>,
    output: Option<PathBuf>,
    started: Option<Instant>,
    family: String,
    draft: Option<Config>,
    expected: Vec<u8>,
    compact_scrolled: bool,
}
impl Run {
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if self.started.is_some_and(|t| t.elapsed().as_secs() > 20) {
            return Err("Font chooser probe timed out".into());
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                ui.invoke_action("open".into());
                ui.invoke_choose_page(4);
                for (k, v) in [
                    ("general.language", "en"),
                    ("appearance.theme", "light"),
                    ("capture.delay_seconds", "0"),
                    ("capture.detect_window", "false"),
                    ("output.format", "png"),
                    ("output.auto_save", "false"),
                ] {
                    ui.invoke_change(k.into(), v.into());
                }
                self.draft = Some(app.borrow().session.preferences.draft.clone());
                ui.window().set_size(slint::PhysicalSize::new(900, 650));
                let current = app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .annotation
                    .text_font
                    .clone();
                ui.invoke_font_command("open".into(), current.into());
                ui.invoke_font_command("open".into(), "".into());
                self.started = Some(Instant::now());
            }
            1 => {
                if ui.get_font_busy() {
                    return Ok(false);
                }
                let actual = glint_platform::installed_font_families()?;
                if ui.get_font_failed()
                    || ui
                        .get_font_names()
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        != actual
                {
                    return Err("Font list differs from actual installed families".into());
                }
                self.family = actual
                    .iter()
                    .find(|s| s.as_str() == "Consolas")
                    .unwrap_or(&actual[0])
                    .clone();
                ui.invoke_font_command("choose".into(), "Not an installed font XYZ".into());
                if !ui.get_font_open() {
                    return Err("Chooser accepted an unlisted font".into());
                }
                ui.invoke_font_command("filter".into(), "NO-SUCH-FONT-XYZ".into());
                if ui.get_font_names().row_count() != 0 {
                    return Err("Font search no-match failed".into());
                }
                ui.invoke_font_command("filter".into(), self.family.to_uppercase().into());
                if !ui.get_font_names().iter().any(|s| s == self.family) {
                    return Err("Font search is not case insensitive".into());
                }
            }
            2 => {
                super::snapshot(ui, &dir.join("font-settings-en.png"))?;
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                        text: slint::platform::Key::Escape.into(),
                    });
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                        text: slint::platform::Key::Escape.into(),
                    });
                if ui.get_font_open() {
                    return Err("Escape did not close font modal".into());
                }
                if app.borrow().session.preferences.draft != *self.draft.as_ref().unwrap() {
                    return Err("Cancel font selection changed draft".into());
                }
                ui.invoke_font_command("open".into(), "".into());
                self.started = Some(Instant::now());
            }
            3 => {
                if ui.get_font_busy() {
                    return Ok(false);
                }
                ui.invoke_font_command("choose".into(), self.family.clone().into());
                if ui.get_font_open()
                    || app.borrow().session.preferences.draft.annotation.text_font != self.family
                    || app.borrow().session.preferences.saved() != self.old.as_ref().unwrap()
                {
                    return Err("Font draft or saved isolation failed".into());
                }
                ui.invoke_action("cancel".into());
                app.borrow_mut().session.writable = false;
                app.borrow_mut().sync(ui);
                ui.invoke_font_command("open".into(), "".into());
                if ui.get_font_open() {
                    return Err("Read-only settings opened editable font chooser".into());
                }
                app.borrow_mut().session.writable = true;
                app.borrow_mut().sync(ui);
                for (k, v) in [
                    ("general.language", "zh_cn"),
                    ("capture.delay_seconds", "0"),
                    ("capture.detect_window", "false"),
                    ("output.format", "png"),
                    ("output.auto_save", "false"),
                ] {
                    ui.invoke_change(k.into(), v.into());
                }
                ui.invoke_action("save".into());
                app.borrow_mut().capture_output = Some(dir.join("font-text.png"));
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            4 => {
                if app.borrow().capturing {
                    return Ok(false);
                }
                let c = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("No font capture")?
                    .clone_strong();
                c.set_tool(0);
                c.invoke_pointer(0, 200., 200.);
                c.invoke_pointer(2, 600., 500.);
                c.set_tool(8);
                c.invoke_pointer(0, 240., 240.);
                c.set_text_value("Font draft Glint".into());
                c.invoke_font_command("open".into(), c.get_text_font());
                self.started = Some(Instant::now());
            }
            5 => {
                let c = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Font editor vanished")?
                    .clone_strong();
                if c.get_font_busy() {
                    return Ok(false);
                }
                if c.get_font_failed()
                    || c.get_font_names().row_count() == 0
                    || c.get_text_value() != "Font draft Glint"
                    || c.get_wide() != 400.
                {
                    return Err("Capture chooser lost text or selection".into());
                }
                c.invoke_font_command("filter".into(), self.family.clone().into());
            }
            6 => {
                let c = app.borrow().capture.as_ref().unwrap().clone_strong();
                super::capture_snapshot(&c, &dir.join("font-capture-zh.png"))?;
                c.invoke_action("cancel".into());
                if c.get_font_open()
                    || !c.get_text_editing()
                    || c.get_text_value() != "Font draft Glint"
                {
                    return Err("Escape chooser canceled text draft".into());
                }
                c.invoke_font_command("open".into(), c.get_text_font());
                self.started = Some(Instant::now());
            }
            7 => {
                let c = app.borrow().capture.as_ref().unwrap().clone_strong();
                if c.get_font_busy() {
                    return Ok(false);
                }
                c.invoke_font_command("choose".into(), self.family.clone().into());
                if c.get_text_font() != self.family || c.get_font_open() {
                    return Err("Capture font choice failed".into());
                }
                c.invoke_action("commit-text".into());
                self.started = Some(Instant::now());
            }
            8 => {
                let c = app.borrow().capture.as_ref().unwrap().clone_strong();
                if c.get_text_busy() {
                    return Ok(false);
                }
                if c.get_text_editing() || c.get_object_count() != 1 {
                    return Err(format!(
                        "Actual chosen-font raster failed: {}",
                        c.get_text_feedback()
                    ));
                }
                let mask = glint_platform::rasterize_text(
                    "Font draft Glint",
                    &self.family,
                    c.get_text_size() as u16,
                )?;
                let color = c.get_mark_color();
                let mut edits = glint_core::Edits::default();
                edits.push(glint_core::TextAnnotation {
                    origin: (40, 40),
                    text: "Font draft Glint".into(),
                    font: self.family.clone(),
                    size: c.get_text_size() as u16,
                    color: [color.red(), color.green(), color.blue(), color.alpha()],
                    mask: glint_core::GlyphMask {
                        width: mask.width,
                        height: mask.height,
                        coverage: mask.coverage.into(),
                    },
                });
                let source = c.get_desktop().to_rgba8().ok_or("No font source")?;
                self.expected = glint_core::composite_selection(
                    source.as_bytes(),
                    source.width(),
                    source.height(),
                    glint_core::Rect {
                        x: 200,
                        y: 200,
                        width: 400,
                        height: 300,
                    },
                    &edits,
                )
                .map_err(str::to_owned)?;
                c.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            9 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                let image = image::open(dir.join("font-text.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if image.dimensions() != (400, 300) || image.as_raw() != &self.expected {
                    return Err("Chosen font export differs from actual glyph composition".into());
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                app.borrow_mut().capture_output = self.output.take();
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Font probe config did not restore".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_choose_page(4);
                ui.window().set_size(slint::PhysicalSize::new(650, 500));
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(400., 250.),
                        delta_x: 0.,
                        delta_y: -720.,
                    });
            }
            10 => {
                if !self.compact_scrolled {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(400., 250.),
                            delta_x: 0.,
                            delta_y: -260.,
                        });
                    self.compact_scrolled = true;
                    return Ok(false);
                }
                super::snapshot(ui, &dir.join("font-settings-row-compact.png"))?;
                let current = app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .annotation
                    .text_font
                    .clone();
                ui.invoke_font_command("open".into(), current.into());
                // Cancel before the UI event loop can deliver the worker result.
                ui.invoke_font_command("cancel".into(), "".into());
                ui.invoke_action("close".into());
                self.started = Some(Instant::now());
            }
            11 => {
                if ui.get_font_busy() {
                    return Ok(false);
                }
                if ui.get_font_open()
                    || ui.window().is_visible()
                    || app.borrow().session.preferences.is_dirty()
                {
                    return Err(
                        "Late font query reopened a canceled chooser or changed settings".into(),
                    );
                }
                self.started = None;
                eprintln!(
                    "Font picker: actual installed families, bounded async request, case-insensitive search/no-match, unlisted rejection, cancel/draft/saved/read-only isolation, capture text/selection retention and exact chosen-font PNG passed"
                );
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}
