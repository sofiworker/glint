//! Synthetic large source verifies real capture/editor budget rejection, not physical 4K support.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_core::{Edits, Rect, Redaction, config::Config};
use slint::{ComponentHandle, Model};
use std::{cell::RefCell, path::Path, rc::Rc};

#[derive(Default)]
pub struct Run {
    old: Option<Config>,
    scrolled: bool,
}

fn scroll(ui: &SettingsWindow) -> Result<(), String> {
    ui.window()
        .dispatch_event_with_result(slint::platform::WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(500., 400.),
            delta_x: 0.,
            delta_y: -360.,
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn drag(ui: &crate::probe::CaptureProbe, a: (f32, f32), b: (f32, f32)) {
    ui.invoke_pointer(0, a.0, a.1);
    ui.invoke_pointer(1, b.0, b.1);
    ui.invoke_pointer(2, b.0, b.1);
}

fn verify_editors(config: &Config, dir: &Path) -> Result<(), String> {
    let frame = || glint_platform::DesktopFrame {
        x: 0,
        y: 0,
        width: 4480,
        height: 4352,
        rgba: vec![211; 4480 * 4352 * 4],
    };
    let completed = Rc::new(RefCell::new(false));
    let closed = completed.clone();
    let (capture, _) = crate::probe::resident_capture_region(
        glint_platform::DesktopSnapshot {
            frame: frame(),
            windows: vec![],
            displays: vec![],
            pointer: None,
            detection_error: None,
        },
        false,
        None,
        crate::probe::Presentation::from_config(config, &[]),
        None,
        move || *closed.borrow_mut() = true,
    )?;
    capture.set_tool(0);
    drag(&capture, (128., 128.), (4352., 4224.));
    capture.set_tool(1);
    drag(&capture, (200., 200.), (250., 250.));
    if !capture.get_output_ready() || capture.get_object_count() != 1 {
        return Err("Small annotation did not fit configured preview budget".into());
    }
    let before = capture
        .get_tiles()
        .row_data(0)
        .ok_or("No small preview")?
        .picture
        .to_rgba8()
        .ok_or("No preview pixels")?;
    drag(&capture, (130., 130.), (4350., 4222.));
    if capture.get_output_ready()
        || capture.get_object_count() != 2
        || capture.get_tiles().row_count() != 1
    {
        return Err("Configured64MiB failed to reject66MiB tiles atomically".into());
    }
    let after = capture
        .get_tiles()
        .row_data(0)
        .unwrap()
        .picture
        .to_rgba8()
        .ok_or("Lost previous preview")?;
    if before.as_bytes() != after.as_bytes() {
        return Err("Rejected preview changed existing visible pixels".into());
    }
    capture.invoke_action("copy".into());
    if capture.get_busy() || *completed.borrow() || capture.get_object_count() != 2 {
        return Err("Preview failure exported or discarded editing".into());
    }
    capture.invoke_action("undo".into());
    if !capture.get_output_ready() || capture.get_object_count() != 1 {
        return Err("Undo did not recover preview budget failure".into());
    }
    capture.invoke_action("redo".into());
    if capture.get_output_ready() || capture.get_object_count() != 2 {
        return Err("Redo bypassed preview budget".into());
    }
    capture.set_tool(0);
    drag(&capture, (10., 10.), (4400., 4300.));
    if !capture.get_output_ready() || capture.get_object_count() != 0 {
        return Err("Reselect did not release failed preview/history".into());
    }
    capture.set_tool(1);
    drag(&capture, (130., 130.), (4350., 4222.));
    if capture.get_output_ready() || capture.get_object_count() != 1 {
        return Err("Reselect silently restored fixed128MiB budget".into());
    }
    capture
        .window()
        .set_size(slint::PhysicalSize::new(1120, 800));
    super::capture_snapshot(&capture, &dir.join("preview-budget-rejected.png"))?;
    capture.invoke_action("cancel".into());
    drop(capture);

    let mut edits = Edits::default();
    edits.push(Redaction {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 4224,
            height: 4096,
        },
        color: [17, 29, 53, 255],
    });
    for local in [false, true] {
        let (editor, _) = crate::probe::resident_pin_editor(
            local,
            crate::pin_edit::Prepared {
                frame: frame(),
                selection: Rect {
                    x: 128,
                    y: 128,
                    width: 4224,
                    height: 4096,
                },
                edits: edits.clone(),
                window_size: (1120, 800),
                turns: 0,
            },
            crate::probe::Presentation::from_config(config, &[]),
            None,
            || {},
        )?;
        if editor.get_output_ready() || editor.get_object_count() != 1 {
            return Err(
                "Image reeditor ignored saved preview budget or lost existing objects".into(),
            );
        }
        editor.invoke_action("undo".into());
        if !editor.get_output_ready() || editor.get_object_count() != 0 {
            return Err("Reeditor could not undo preexisting over-budget annotation".into());
        }
        editor.invoke_action("cancel".into());
    }
    Ok(())
}

impl Run {
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                ui.invoke_action("open".into());
                ui.invoke_choose_page(9);
                if !ui
                    .get_rows()
                    .iter()
                    .any(|r| r.key == "resources.preview_budget_mib" && r.editable)
                {
                    return Err("Preview budget setting disabled".into());
                }
                ui.invoke_change("resources.preview_budget_mib".into(), "64".into());
                if app.borrow().session.preferences.saved() != self.old.as_ref().unwrap() {
                    return Err("Preview budget draft changed saved config".into());
                }
                ui.invoke_action("cancel".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Preview budget cancel failed".into());
                }
                ui.invoke_change("resources.preview_budget_mib".into(), "64".into());
                ui.invoke_change("general.language".into(), "zh_cn".into());
                ui.invoke_action("save".into());
                let disk = Config::from_toml(
                    &std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if disk.resources.preview_budget_mib != 64
                    || app.borrow().session.preferences.is_dirty()
                {
                    return Err("Preview budget save/reload failed".into());
                }
                verify_editors(&disk, dir)?;
                scroll(ui)?;
            }
            1 => {
                super::snapshot(ui, &dir.join("preview-budget-settings-zh.png"))?;
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_change("resources.preview_budget_mib".into(), "512".into());
                ui.invoke_action("save".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .resources
                    .preview_budget_mib
                    != 512
                {
                    return Err("Maximum preview budget failed to save".into());
                }
                ui.invoke_action("reset".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .resources
                    .preview_budget_mib
                    != 512
                {
                    return Err("Reset skipped confirmation".into());
                }
                ui.invoke_action("confirm-reset".into());
                if app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .resources
                    .preview_budget_mib
                    != 128
                    || app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .resources
                        .preview_budget_mib
                        != 512
                {
                    return Err("Preview budget reset transaction failed".into());
                }
                ui.invoke_action("cancel".into());
            }
            2 => {
                if !self.scrolled {
                    scroll(ui)?;
                    self.scrolled = true;
                    return Ok(false);
                }
                super::snapshot(ui, &dir.join("preview-budget-settings-en.png"))?;
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Preview budget fixture restoration failed".into());
                }
                eprintln!(
                    "Preview budget: editable64-512/default128, draft/cancel/reset/TOML, actual synthetic66MiB tile rejection with retained pixels/objects/output guard, undo/redo/reselect and pin/local reedit recovery passed; not physical4K/resource-peak acceptance"
                );
            }
            _ => return Err("Unknown preview budget stage".into()),
        }
        Ok(true)
    }
}
