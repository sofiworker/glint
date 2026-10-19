//! Saved annotation preferences through real resident capture and PNG composition.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_core::{Edits, Pixelation, Point, Rect, Redaction, Stroke, config::Config};
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Run {
    old: Option<Config>,
    output: Option<PathBuf>,
    expected: Vec<u8>,
    started: Option<Instant>,
}
fn drag(ui: &crate::probe::CaptureProbe, a: (f32, f32), b: (f32, f32)) {
    ui.invoke_pointer(0, a.0, a.1);
    ui.invoke_pointer(1, b.0, b.1);
    ui.invoke_pointer(2, b.0, b.1);
}
fn selection() -> Rect {
    Rect {
        x: 200,
        y: 200,
        width: 400,
        height: 300,
    }
}
impl Run {
    fn begin(&mut self, ui: &SettingsWindow, app: &Rc<RefCell<AppState>>, dir: &Path, name: &str) {
        app.borrow_mut().capture_output = Some(dir.join(name));
        ui.invoke_action("capture".into());
        self.started = Some(Instant::now());
    }
    fn expect(&mut self, ui: &crate::probe::CaptureProbe, edits: &Edits) -> Result<(), String> {
        let source = ui.get_desktop().to_rgba8().ok_or("Missing frozen pixels")?;
        self.expected = glint_core::composite_selection(
            source.as_bytes(),
            source.width(),
            source.height(),
            selection(),
            edits,
        )
        .map_err(str::to_owned)?;
        ui.invoke_action("save".into());
        Ok(())
    }
    fn verify(&self, dir: &Path, name: &str) -> Result<(), String> {
        let image = image::open(dir.join(name))
            .map_err(|e| e.to_string())?
            .to_rgba8();
        if image.dimensions() != (400, 300) || image.as_raw() != &self.expected {
            return Err("Saved annotation defaults differ from expected source composition".into());
        }
        Ok(())
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
                return Err("Annotation defaults timed out".into());
            }
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                ui.invoke_action("open".into());
                ui.invoke_choose_page(4);
                for key in [
                    "annotation.tool",
                    "annotation.stroke_width",
                    "annotation.color",
                    "annotation.redact_style",
                ] {
                    if !ui.get_rows().iter().any(|r| r.key == key && r.editable) {
                        return Err("Annotation default control is disabled".into());
                    }
                }
                for (key, value) in [
                    ("general.language", "en"),
                    ("capture.delay_seconds", "0"),
                    ("capture.detect_window", "false"),
                    ("annotation.tool", "pen"),
                    ("annotation.stroke_width", "11"),
                    ("annotation.color", "#123ABC"),
                ] {
                    ui.invoke_change(key.into(), value.into());
                }
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Valid annotation defaults failed to save".into());
                }
                let disk = Config::from_toml(
                    &std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if disk.annotation.stroke_width != 11 || disk.annotation.color != "#123ABC" {
                    return Err("Annotation defaults not persisted".into());
                }
            }
            1 => {
                super::snapshot(ui, &dir.join("annotation-defaults-en.png"))?;
                ui.invoke_change("annotation.stroke_width".into(), "1".into());
                ui.invoke_action("cancel".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .annotation
                    .stroke_width
                    != 11
                {
                    return Err("Cancel lost saved line width".into());
                }
                ui.invoke_change("annotation.color".into(), "invalid color".into());
                ui.invoke_action("save".into());
                if !app.borrow().session.preferences.is_dirty()
                    || app.borrow().session.preferences.saved().annotation.color != "#123ABC"
                {
                    return Err("Invalid annotation color replaced saved defaults".into());
                }
                ui.invoke_action("cancel".into());
                ui.invoke_change("annotation.tool".into(), "redact".into());
                ui.invoke_action("capture".into());
                if ui.get_modal() != 1 || app.borrow().capturing {
                    return Err("Annotation draft bypassed dirty capture guard".into());
                }
                ui.invoke_action("continue".into());
                self.begin(ui, app, dir, "annotation-pen.png");
                ui.invoke_action("discard-leave".into());
            }
            2 => {
                let capture = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Default pen capture missing")?
                    .clone_strong();
                if capture.get_selected()
                    || capture.get_tool() != 2
                    || capture.get_stroke_width() != 11
                    || capture.get_mark_color() != slint::Color::from_rgb_u8(18, 58, 188)
                {
                    return Err("Capture used draft or hardcoded annotation defaults".into());
                }
                drag(&capture, (200., 200.), (600., 500.));
                if !capture.get_selected() || capture.get_object_count() != 0 {
                    return Err("Default drawing tool prevented initial region selection".into());
                }
                drag(&capture, (230., 240.), (400., 320.));
                capture.invoke_action("undo".into());
                if capture.get_object_count() != 0 {
                    return Err("Default stroke undo failed".into());
                }
                capture.invoke_action("redo".into());
                if capture.get_object_count() != 1 {
                    return Err("Default stroke redo failed".into());
                }
                capture.invoke_action("canvas-leave".into());
                super::capture_snapshot(&capture, &dir.join("annotation-default-pen.png"))?;
                // Dispatch through the real Slint hit-test tree to select the saved HEX swatch.
                capture.set_mark_color(slint::Color::from_rgb_u8(237, 101, 113));
                let position = slint::LogicalPosition::new(
                    capture.get_toolbar_x() + 23.,
                    capture.get_toolbar_y() + capture.get_toolbar_height() + 31.,
                );
                use slint::platform::{PointerEventButton, WindowEvent};
                capture
                    .window()
                    .dispatch_event(WindowEvent::PointerMoved { position });
                capture
                    .window()
                    .dispatch_event(WindowEvent::PointerPressed {
                        position,
                        button: PointerEventButton::Left,
                    });
                capture
                    .window()
                    .dispatch_event(WindowEvent::PointerReleased {
                        position,
                        button: PointerEventButton::Left,
                    });
                if capture.get_mark_color() != slint::Color::from_rgb_u8(18, 58, 188) {
                    return Err("Saved custom HEX swatch is not reachable".into());
                }
                let mut edits = Edits::default();
                edits.push(Stroke {
                    points: vec![Point { x: 30., y: 40. }, Point { x: 200., y: 120. }],
                    width: 11.,
                    color: [18, 58, 188, 255],
                });
                self.expect(&capture, &edits)?;
            }
            3 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                self.verify(dir, "annotation-pen.png")?;
                ui.invoke_action("open".into());
                ui.invoke_change("annotation.tool".into(), "redact".into());
                ui.invoke_change("annotation.redact_style".into(), "pixelate".into());
                ui.invoke_change("annotation.stroke_width".into(), "1".into());
                ui.invoke_action("save".into());
                self.begin(ui, app, dir, "annotation-mosaic.png");
            }
            4 => {
                let capture = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Default mosaic capture missing")?
                    .clone_strong();
                if capture.get_tool() != 7 || capture.get_stroke_width() != 1 {
                    return Err("Preferred redaction was not adopted".into());
                }
                drag(&capture, (200., 200.), (600., 500.));
                drag(&capture, (230., 240.), (400., 320.));
                let mut edits = Edits::default();
                edits.push(Pixelation {
                    bounds: Rect {
                        x: 30,
                        y: 40,
                        width: 170,
                        height: 80,
                    },
                });
                self.expect(&capture, &edits)?;
            }
            5 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                self.verify(dir, "annotation-mosaic.png")?;
                self.begin(ui, app, dir, "annotation-solid.png");
            }
            6 => {
                let capture = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Explicit solid capture missing")?
                    .clone_strong();
                if capture.get_tool() != 7 {
                    return Err("Session tool change persisted over default".into());
                }
                drag(&capture, (200., 200.), (600., 500.));
                capture.set_tool(1);
                drag(&capture, (230., 240.), (400., 320.));
                let mut edits = Edits::default();
                edits.push(Redaction {
                    bounds: Rect {
                        x: 30,
                        y: 40,
                        width: 170,
                        height: 80,
                    },
                    color: [32, 43, 66, 255],
                });
                self.expect(&capture, &edits)?;
            }
            7 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                self.verify(dir, "annotation-solid.png")?;
                ui.invoke_action("open".into());
                ui.invoke_choose_page(4);
                ui.invoke_change("general.language".into(), "zh_cn".into());
            }
            8 => {
                super::snapshot(ui, &dir.join("annotation-defaults-zh.png"))?;
                ui.invoke_action("reset".into());
                ui.invoke_action("confirm-reset".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .annotation
                    .stroke_width
                    != 3
                    || app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .annotation
                        .stroke_width
                        != 1
                {
                    return Err("Reset bypassed annotation draft".into());
                }
                ui.invoke_action("cancel".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .annotation
                    .stroke_width
                    != 1
                {
                    return Err("Cancel reset lost saved defaults".into());
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                app.borrow_mut().capture_output = self.output.take();
                ui.invoke_action("save".into());
                ui.invoke_action("close".into());
                eprintln!(
                    "Annotation defaults: editable fields, TOML persistence, invalid color retention, draft/cancel/reset/dirty guard, initial selection under default pen/mosaic, exact pen/mosaic/explicit-solid PNG pixels and stroke undo/redo passed"
                );
            }
            _ => return Err("Unexpected annotation default stage".into()),
        }
        Ok(true)
    }
}
