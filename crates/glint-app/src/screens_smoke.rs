//! Frozen screen-target geometry fixtures and real resident capture/output checks.
use crate::{capture_screen, probe, resident::AppState, settings::SettingsWindow};
use glint_core::{Rect, screen_capture::ScreenTarget};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

fn verify_synthetic(directory: &Path) -> Result<(), String> {
    use glint_platform::{DesktopFrame, DesktopSnapshot, DisplayInfo, ScreenBounds};
    let display = |bounds, dpi| DisplayInfo {
        bounds,
        work_area: bounds,
        primary: false,
        effective_dpi: (dpi, dpi),
    };
    let bounds = |left, top, right, bottom| ScreenBounds {
        left,
        top,
        right,
        bottom,
    };
    for (case, pointer, expected) in [
        (
            "left",
            Some((-1, 0)),
            Some(Rect {
                x: 0,
                y: 0,
                width: 600,
                height: 700,
            }),
        ),
        (
            "right",
            Some((0, 100)),
            Some(Rect {
                x: 600,
                y: 100,
                width: 600,
                height: 600,
            }),
        ),
        ("missing", None, None),
        ("gap", Some((1, 50)), None),
    ] {
        let snapshot = DesktopSnapshot {
            frame: DesktopFrame {
                x: -600,
                y: 0,
                width: 1200,
                height: 700,
                rgba: [230, 240, 250, 255].repeat(1200 * 700),
            },
            displays: vec![
                display(bounds(-600, 0, 0, 700), 96),
                display(bounds(0, 100, 600, 700), 144),
            ],
            pointer,
            windows: vec![],
            detection_error: None,
        };
        let initial = capture_screen::initial(&snapshot, ScreenTarget::Current);
        if initial != expected {
            return Err(format!("Synthetic {case} physical screen mapping changed"));
        }
        let (ui, _) = probe::resident_capture_region(
            snapshot,
            false,
            initial,
            probe::Presentation {
                magnifier: false,
                english: true,
                ..Default::default()
            },
            None,
            || {},
        )?;
        ui.set_repeat_notice(
            crate::messages::text(
                capture_screen::notice(ScreenTarget::Current, initial.is_some()),
                true,
            )
            .into(),
        );
        if ui.get_selected() != expected.is_some()
            || ui.get_object_count() != 0
            || ui.get_can_undo()
        {
            return Err("Synthetic screen adapter kept stale selection/edits".into());
        }
        if let Some(r) = expected {
            if (ui.get_left(), ui.get_top(), ui.get_wide(), ui.get_high())
                != (r.x as f32, r.y as f32, r.width as f32, r.height as f32)
            {
                return Err("Synthetic screen UI rescaled physical pixels".into());
            }
        } else {
            if !ui.get_repeat_notice().contains("manually") {
                return Err("Missing screen fallback message".into());
            }
            ui.invoke_pointer(0, 40., 150.);
            ui.invoke_pointer(2, 240., 350.);
            if !ui.get_selected() || ui.get_wide() != 200. {
                return Err("Screen failure prevented manual selection".into());
            }
        }
        super::capture_snapshot(&ui, &directory.join(format!("screen-synthetic-{case}.png")))?;
    }
    Ok(())
}

#[derive(Default)]
pub struct Run {
    old: Option<glint_core::config::Config>,
    output: Option<PathBuf>,
    remembered: Option<glint_core::region_memory::RememberedRegion>,
    pixels: Vec<u8>,
    size: (u32, u32),
    started: Option<Instant>,
    blocker: Option<glint_platform::ClipboardBlocker>,
    output_editor: Option<probe::CaptureProbe>,
}
impl Run {
    fn start(&mut self, settings: &SettingsWindow, action: &str) {
        self.started = Some(Instant::now());
        settings.invoke_action(action.into());
    }
    fn wait(&self) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
        {
            Err("Screen capture/output timed out".into())
        } else {
            Ok(false)
        }
    }
    fn expected_current(ui: &probe::CaptureProbe) -> Result<Rect, String> {
        let env = glint_platform::desktop_environment()?;
        let bounds: Vec<_> = env
            .displays
            .iter()
            .map(|d| [d.bounds.left, d.bounds.top, d.bounds.right, d.bounds.bottom])
            .collect();
        let origin = (
            bounds.iter().map(|b| b[0]).min().ok_or("No displays")?,
            bounds.iter().map(|b| b[1]).min().unwrap(),
        );
        glint_core::screen_capture::screen_region(
            ScreenTarget::Current,
            origin,
            (ui.get_source_w() as u32, ui.get_source_h() as u32),
            &bounds,
            Some(glint_platform::cursor_position()?),
        )
        .ok_or("Current test pointer has no display".into())
    }
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let capture = app.borrow().capture.as_ref().map(|c| c.clone_strong());
        if [1, 3, 4].contains(&stage) && capture.is_none() {
            return self.wait();
        }
        if [2, 5].contains(&stage) && (capture.is_some() || app.borrow().capturing) {
            return self.wait();
        }
        match stage {
            0 => {
                verify_synthetic(directory)?;
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                self.remembered = app.borrow().last_region.clone();
                for (k, v) in [
                    ("capture.delay_seconds", "0"),
                    ("output.auto_save", "false"),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                app.borrow_mut().capture_output = Some(directory.join("screen-all.png"));
                let value =
                    (!app.borrow().session.preferences.saved().capture.dimensions).to_string();
                settings.invoke_change("capture.dimensions".into(), value.into());
                self.start(settings, "all-screens-capture");
                if settings.get_modal() != 1
                    || !app.borrow().capture_pending
                    || app.borrow().screen_capture != Some(ScreenTarget::All)
                {
                    return Err("Dirty screen entry lost its target or guard".into());
                }
                settings.invoke_action("discard-leave".into());
            }
            1 => {
                let ui = capture.unwrap();
                let src = ui.get_desktop().to_rgba8().ok_or("No screen source")?;
                self.size = (src.width(), src.height());
                if !ui.get_selected()
                    || (ui.get_left(), ui.get_top(), ui.get_wide(), ui.get_high())
                        != (0., 0., src.width() as f32, src.height() as f32)
                    || ui.get_object_count() != 0
                    || ui.get_can_undo()
                {
                    return Err("All screens did not select the frozen source".into());
                }
                if !ui.get_repeat_notice().contains("All screens") {
                    return Err("All-screen notice missing".into());
                }
                ui.set_tool(1);
                ui.invoke_pointer(0, 200., 200.);
                ui.invoke_pointer(2, 300., 270.);
                ui.invoke_action("undo".into());
                if ui.get_object_count() != 0 {
                    return Err("Screen redaction undo failed".into());
                }
                ui.invoke_action("redo".into());
                let mut edits = glint_core::Edits::default();
                edits.push(glint_core::Redaction {
                    bounds: Rect {
                        x: 200,
                        y: 200,
                        width: 100,
                        height: 70,
                    },
                    color: [32, 43, 66, 255],
                });
                self.pixels = glint_core::composite_selection(
                    src.as_bytes(),
                    src.width(),
                    src.height(),
                    Rect {
                        x: 0,
                        y: 0,
                        width: src.width(),
                        height: src.height(),
                    },
                    &edits,
                )?;
                super::capture_snapshot(&ui, &directory.join("screen-all-native.png"))?;
                self.output_editor = Some(ui.clone_strong());
                ui.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            2 => {
                let editor = self
                    .output_editor
                    .take()
                    .ok_or("Missing all-screen output receipt")?;
                if !editor.get_closed() || editor.get_busy() || editor.get_completion_kind() != 2 {
                    return Err(format!(
                        "All-screen editor left before successful save: closed={} busy={} completion={} exit={:?}",
                        editor.get_closed(),
                        editor.get_busy(),
                        editor.get_completion_kind(),
                        editor.get_exit_reason()
                    ));
                }
                if editor.get_exit_reason() != probe::CaptureExit::Save {
                    return Err("All-screen save closed with incorrect exit provenance".into());
                }
                std::fs::write(
                    directory.join("screen-all-exit.txt"),
                    format!(
                        "exit={:?}\nclosed={}\nbusy={}\ncompletion={}\n",
                        editor.get_exit_reason(),
                        editor.get_closed(),
                        editor.get_busy(),
                        editor.get_completion_kind()
                    ),
                )
                .map_err(|e| e.to_string())?;
                let p = image::open(directory.join("screen-all.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if p.dimensions() != self.size || p.as_raw() != &self.pixels {
                    return Err("All-screen PNG differs from frozen annotated source".into());
                }
                settings.invoke_change("capture.delay_seconds".into(), "1".into());
                settings.invoke_action("save".into());
                self.start(settings, "current-screen-capture");
                if app.borrow().countdown.as_ref().map(|c| c.action.as_str())
                    != Some("current-screen-capture")
                {
                    return Err("Screen delay lost current target".into());
                }
                settings.invoke_action("all-screens-capture".into());
                if app.borrow().screen_capture != Some(ScreenTarget::Current) {
                    return Err("Duplicate entry changed pending screen target".into());
                }
            }
            3 => {
                if self.started.unwrap().elapsed() < Duration::from_secs(1) {
                    return Err("Screen acquired before delay".into());
                }
                let ui = capture.unwrap();
                let r = Self::expected_current(&ui)?;
                if !ui.get_selected()
                    || (ui.get_left(), ui.get_top(), ui.get_wide(), ui.get_high())
                        != (r.x as f32, r.y as f32, r.width as f32, r.height as f32)
                    || ui.get_object_count() != 0
                {
                    return Err("Current screen geometry or clean edits mismatch".into());
                }
                let before = (ui.get_wide(), ui.get_high());
                ui.set_tool(0);
                ui.invoke_pointer(0, r.x as f32, r.y as f32);
                ui.invoke_pointer(2, r.x as f32 + 20., r.y as f32 + 20.);
                if (ui.get_wide(), ui.get_high()) != (before.0 - 20., before.1 - 20.) {
                    return Err("Screen selection could not be adjusted".into());
                }
                let src = ui.get_desktop().to_rgba8().unwrap();
                let crop = Rect {
                    x: ui.get_left() as u32,
                    y: ui.get_top() as u32,
                    width: ui.get_wide() as u32,
                    height: ui.get_high() as u32,
                };
                self.size = (crop.width, crop.height);
                self.pixels = glint_core::composite_selection(
                    src.as_bytes(),
                    src.width(),
                    src.height(),
                    crop,
                    &Default::default(),
                )?;
                super::capture_snapshot(&ui, &directory.join("screen-current-native.png"))?;
                self.blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                ui.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            4 => {
                let ui = capture.unwrap();
                if ui.get_busy() {
                    return self.wait();
                }
                if !ui.get_status().starts_with(crate::messages::text(
                    crate::messages::Message::CaptureOutputFailed,
                    ui.get_english(),
                )) || !ui.get_selected()
                {
                    return Err("Failed screen copy discarded editing".into());
                }
                super::capture_snapshot(&ui, &directory.join("screen-copy-failed.png"))?;
                self.blocker.take();
                ui.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            5 => {
                let p = glint_platform::read_clipboard_image()?;
                if (p.width, p.height) != self.size || p.rgba != self.pixels {
                    return Err("Screen clipboard retry differs from adjusted frozen source".into());
                }
                self.start(settings, "all-screens-capture");
                if app.borrow().countdown.as_ref().map(|c| c.action.as_str())
                    != Some("all-screens-capture")
                {
                    return Err("All-screen delay route missing".into());
                }
                settings.invoke_action("delay-cancel".into());
            }
            6 => {
                if self.started.unwrap().elapsed() < Duration::from_millis(1400) {
                    return Ok(false);
                }
                if app.borrow().capture.is_some()
                    || app.borrow().capturing
                    || app.borrow().countdown.is_some()
                {
                    return Err("Cancelled screen countdown still acquired desktop".into());
                }
                let old = self.old.take().unwrap();
                for (k, v) in [
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
                    return Err("Screen fixture preferences not restored".into());
                }
                eprintln!(
                    "Screen entry: all/current source geometry, synthetic negative/mixed-DPI/gap/missing pointer fallback, dirty guard, editable redaction/history/PNG, real clipboard conflict/retry, delay/duplicate/cancel passed"
                );
            }
            _ => return Err("Unknown screen stage".into()),
        }
        Ok(true)
    }
}
