//! Settings draft integration, actual TOML transaction failures, and rendered editor fixtures.
use super::*;
use glint_core::config::{Config, Theme, Tool, Toolbar, ToolbarMode, ToolbarPosition};
use slint::platform::WindowEvent;

pub(super) fn verify(
    ui: &SettingsWindow,
    app: &Rc<RefCell<AppState>>,
    directory: &std::path::Path,
) -> Result<(), String> {
    let original = app.borrow().session.preferences.saved().clone();
    let cmd = |name: &str, row, value| ui.invoke_toolbar_command(name.into(), row, value);
    ui.invoke_choose_page(3);
    cmd("mode", 0, 1);
    cmd("position", 0, 2);
    cmd("groups", 0, 0);
    cmd("visible", 8, 0);
    cmd("visible", 0, 0);
    cmd("move", 8, 1);
    let expected = app.borrow().session.preferences.draft.toolbar.clone();
    if expected.mode != ToolbarMode::Icons
        || expected.position != ToolbarPosition::Bottom
        || expected.order[1] != Tool::Picker
        || expected.hidden != vec![Tool::Picker]
        || expected.groups
        || !ui.get_dirty()
        || !ui.get_toolbar_icons()
        || ui.get_toolbar_rows().row_data(1).unwrap().tool != 8
        || app.borrow().session.preferences.saved() != &original
    {
        return Err("Settings toolbar editor bypassed draft or lost ordering/visibility".into());
    }
    ui.invoke_action("close".into());
    if ui.get_modal() != 1 {
        return Err("Toolbar-only draft did not block leaving settings".into());
    }
    cmd("reset", 0, 0);
    if app.borrow().session.preferences.draft.toolbar != expected {
        return Err("Modal allowed background toolbar edits".into());
    }
    ui.invoke_action("continue".into());
    ui.invoke_action("cancel".into());
    if app.borrow().session.preferences.is_dirty()
        || ui.get_toolbar_icons() != (original.toolbar.mode == ToolbarMode::Icons)
    {
        return Err("Settings cancel did not restore toolbar view and saved configuration".into());
    }
    cmd("mode", 0, 1);
    cmd("position", 0, 2);
    cmd("groups", 0, 0);
    cmd("visible", 8, 0);
    cmd("move", 8, 1);
    ui.invoke_action("save".into());
    let persisted = Config::from_toml(
        &std::fs::read_to_string(directory.join("config.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if persisted.toolbar != expected || app.borrow().session.preferences.is_dirty() {
        return Err("Settings toolbar save/reopen mismatch".into());
    }
    use std::os::windows::fs::OpenOptionsExt;
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(directory.join("config.toml"))
        .map_err(|e| e.to_string())?;
    cmd("mode", 0, 0);
    ui.invoke_action("save".into());
    if !app.borrow().session.preferences.is_dirty()
        || app.borrow().session.preferences.saved().toolbar != expected
        || app.borrow().session.preferences.draft.toolbar.mode != ToolbarMode::IconsAndLabels
    {
        return Err(
            "Settings failed toolbar save changed accepted snapshot or discarded draft".into(),
        );
    }
    drop(locked);
    ui.invoke_action("save".into());
    if app.borrow().session.preferences.is_dirty() {
        return Err("Settings toolbar retry failed".into());
    }
    ui.invoke_change("appearance.theme".into(), "dark".into());
    ui.invoke_change("general.language".into(), "en".into());
    cmd("reset", 0, 0);
    if app.borrow().session.preferences.draft.toolbar != Toolbar::default()
        || app.borrow().session.preferences.draft.appearance.theme != Theme::Dark
        || !ui.get_dirty()
        || !ui.get_dark()
    {
        return Err("Toolbar-only reset changed unrelated settings".into());
    }
    let draft = app.borrow().session.preferences.draft.clone();
    app.borrow_mut().session.writable = false;
    ui.invoke_choose_page(3);
    cmd("mode", 0, 1);
    cmd("reset", 0, 0);
    if app.borrow().session.preferences.draft != draft || ui.get_writable() {
        return Err("Read-only settings permitted toolbar edits".into());
    }
    app.borrow_mut().session.writable = true;
    app.borrow_mut().session.preferences.draft = original.clone();
    ui.invoke_choose_page(3);
    ui.invoke_action("save".into());
    if app.borrow().session.preferences.saved() != &original
        || app.borrow().session.preferences.is_dirty()
    {
        return Err("Toolbar fixture failed to restore original saved settings".into());
    }
    Ok(())
}

// Separate native event-loop turns: this tests a real press/drag/release sequence,
// including model updates and pointer-grab cleanup, rather than nested timer dispatch.
pub(super) fn pointer_stage(
    ui: &SettingsWindow,
    app: &Rc<RefCell<AppState>>,
    directory: &std::path::Path,
    stage: u8,
) -> Result<(), String> {
    use slint::platform::{PointerEventButton, WindowEvent};
    match stage {
        0 => {
            ui.invoke_choose_page(3);
            snapshot(ui, &directory.join("settings-toolbar-before-drag.png"))?;
            let position = slint::LogicalPosition::new(234., 362.);
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
        }
        1 => {
            let position = slint::LogicalPosition::new(234., 438.);
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            if app.borrow().session.preferences.draft.toolbar.order[2] != Tool::Select {
                snapshot(ui, &directory.join("settings-toolbar-failed-drag.png"))?;
                return Err(format!(
                    "Settings toolbar drag did not reorder through Slint events: size={:?} scale={} order={:?}",
                    ui.window().size(),
                    ui.window().scale_factor(),
                    app.borrow().session.preferences.draft.toolbar.order
                ));
            }
            ui.invoke_toolbar_command("move".into(), 2, 0);
        }
        2 => {
            snapshot(ui, &directory.join("settings-toolbar-before-click.png"))?;
            let position = slint::LogicalPosition::new(281., 666.);
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
        }
        3 => {
            let position = slint::LogicalPosition::new(281., 666.);
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            if app.borrow().session.preferences.draft.toolbar.hidden != vec![Tool::Picker] {
                return Err(format!(
                    "Settings toolbar visibility pointer mismatch: {:?}",
                    app.borrow().session.preferences.draft.toolbar.hidden
                ));
            }
            ui.invoke_toolbar_command("visible".into(), 8, 1);
        }
        _ => return Err("Unknown toolbar input stage".into()),
    }
    Ok(())
}

// Render only after the event loop has applied model notifications, widget state and OS resizing.
pub(super) fn layout_stage(
    ui: &SettingsWindow,
    directory: &std::path::Path,
    stage: u8,
) -> Result<(), String> {
    let cmd = |name: &str, row, value| ui.invoke_toolbar_command(name.into(), row, value);
    let scroll = |delta_y| {
        ui.window().dispatch_event(WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(700., 400.),
            delta_x: 0.,
            delta_y,
        })
    };
    match stage {
        0 => {
            ui.invoke_change("general.language".into(), "zh_cn".into());
            ui.invoke_change("appearance.theme".into(), "light".into());
            cmd("reset", 0, 0);
            cmd("mode", 0, 1);
            cmd("position", 0, 2);
            cmd("groups", 0, 0);
            cmd("visible", 8, 0);
            cmd("move", 8, 1);
            ui.invoke_choose_page(3);
            scroll(1000.);
        }
        1 => {
            snapshot(ui, &directory.join("settings-toolbar-top.png"))?;
            scroll(-1000.);
        }
        2 => {
            snapshot(ui, &directory.join("settings-toolbar-preview.png"))?;
            ui.invoke_change("general.language".into(), "en".into());
            ui.invoke_change("appearance.theme".into(), "dark".into());
            cmd("reset", 0, 0);
            scroll(1000.);
        }
        3 => {
            snapshot(ui, &directory.join("settings-toolbar-dark-en.png"))?;
            scroll(-1000.);
        }
        4 => {
            snapshot(ui, &directory.join("settings-toolbar-preview-dark-en.png"))?;
            ui.window().set_size(slint::LogicalSize::new(900., 650.));
        }
        5 => {
            let logical_width = ui.window().size().width as f32 / ui.window().scale_factor();
            let logical_height = ui.window().size().height as f32 / ui.window().scale_factor();
            if logical_width != 900. || logical_height != 650. {
                return Err(format!(
                    "Settings resize has not reached 900x650: {logical_width}x{logical_height}"
                ));
            }
            scroll(-1000.);
        }
        6 => {
            snapshot(ui, &directory.join("settings-toolbar-preview-900.png"))?;
            ui.invoke_action("cancel".into());
            ui.window().set_size(slint::LogicalSize::new(1040., 760.));
            scroll(1000.);
        }
        _ => return Err("Unknown toolbar layout stage".into()),
    }
    Ok(())
}
