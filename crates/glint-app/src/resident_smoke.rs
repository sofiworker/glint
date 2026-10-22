//! Native adapter verification. Does not synthesize physical keyboard or tray clicks.
#[path = "annotation_defaults_smoke.rs"]
mod annotation_defaults_smoke;
#[path = "completion_smoke.rs"]
mod completion_smoke;
#[path = "local_image_smoke.rs"]
mod local_image_smoke;
use crate::{resident::AppState, settings::SettingsWindow};
use slint::{ComponentHandle, Model};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

glint_ui::slint! {
    export component WindowSelectionFixture inherits Window {
        title: "Glint window-selection verification";
        width: 240px;
        height: 180px;
        no-frame: true;
        always-on-top: true;
        background: #426a89;
        Text { text: "Window-selection verification"; color: white; }
    }
}
struct WindowFixture(WindowSelectionFixture);
impl WindowFixture {
    fn show() -> Result<Self, String> {
        let fixture = WindowSelectionFixture::new().map_err(|e| e.to_string())?;
        fixture.show().map_err(|e| e.to_string())?;
        fixture
            .window()
            .set_position(slint::PhysicalPosition::new(160, 160));
        Ok(Self(fixture))
    }
    fn region(&self) -> Result<crate::probe::WindowRegion, String> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use slint::winit_030::WinitWindowAccessor;
        let id = self
            .0
            .window()
            .with_winit_window(|window| {
                let handle = window.window_handle().map_err(|e| e.to_string())?;
                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                    return Err("Fixture is not a Win32 window".to_owned());
                };
                Ok(handle.hwnd.get() as usize)
            })
            .ok_or("Fixture native window unavailable")??;
        let bounds = glint_platform::visible_windows()?
            .into_iter()
            .find(|w| w.id == id)
            .ok_or("Real fixture was not enumerated")?
            .bounds;
        let displays = glint_platform::desktop_environment()?.displays;
        let x = displays
            .iter()
            .map(|d| d.bounds.left)
            .min()
            .ok_or("No fixture display")?;
        let y = displays
            .iter()
            .map(|d| d.bounds.top)
            .min()
            .ok_or("No fixture display")?;
        Ok(crate::probe::WindowRegion {
            left: (bounds.left - x) as f32,
            top: (bounds.top - y) as f32,
            wide: (bounds.right - bounds.left) as f32,
            high: (bounds.bottom - bounds.top) as f32,
        })
    }
}
impl Drop for WindowFixture {
    fn drop(&mut self) {
        let _ = self.0.hide();
    }
}
#[path = "color_picker_smoke.rs"]
mod color_picker_smoke;
#[path = "config_transfer_smoke.rs"]
mod config_transfer_smoke;
#[path = "cursor_smoke.rs"]
mod cursor_smoke;
#[path = "delay_smoke.rs"]
mod delay_smoke;
#[path = "magnifier_smoke.rs"]
mod magnifier_smoke;
#[path = "onboarding_smoke.rs"]
mod onboarding_smoke;
#[path = "pin_edit_smoke.rs"]
mod pin_edit_smoke;
#[path = "pin_geometry_smoke.rs"]
mod pin_geometry_smoke;
#[path = "pin_pass_smoke.rs"]
mod pin_pass_smoke;
#[path = "pin_rotation_smoke.rs"]
mod pin_rotation_smoke;
#[path = "pin_wheel_smoke.rs"]
mod pin_wheel_smoke;
#[path = "pins_smoke.rs"]
mod pins_smoke;
#[path = "pixelation_smoke.rs"]
mod pixelation_smoke;
#[path = "repeat_smoke.rs"]
mod repeat_smoke;
#[path = "screens_smoke.rs"]
mod screens_smoke;
#[path = "settings_toolbar_smoke.rs"]
mod settings_toolbar_smoke;

#[path = "diagnostic_log_smoke.rs"]
mod diagnostic_log_smoke;
#[path = "exclusion_smoke.rs"]
mod exclusion_smoke;
#[path = "interface_scale_smoke.rs"]
mod interface_scale_smoke;

#[path = "lifecycle_smoke.rs"]
mod lifecycle_smoke;
#[path = "motion_smoke.rs"]
mod motion_smoke;
#[path = "preview_budget_smoke.rs"]
mod preview_budget_smoke;

#[path = "diagnostics_smoke.rs"]
mod diagnostics_smoke;

#[path = "font_picker_smoke.rs"]
mod font_picker_smoke;

#[path = "output_settings_smoke.rs"]
mod output_settings_smoke;

#[path = "output_conflict_smoke.rs"]
mod output_conflict_smoke;

#[path = "local_dialog_smoke.rs"]
mod local_dialog_smoke;

struct Run {
    exclusions: exclusion_smoke::Run,
    log: diagnostic_log_smoke::Run,
    window_fixture: Option<WindowFixture>,
    window_fixture_region: Option<crate::probe::WindowRegion>,
    local_dialog: local_dialog_smoke::Run,
    config_dialog: config_transfer_smoke::DialogRun,
    output_conflict: output_conflict_smoke::Run,
    output_settings: output_settings_smoke::Run,
    fonts: font_picker_smoke::Run,
    diagnostics: diagnostics_smoke::Run,
    lifecycle: lifecycle_smoke::Run,
    preview_budget: preview_budget_smoke::Run,
    focused: u16,
    motion: motion_smoke::Run,
    scale: interface_scale_smoke::Run,
    completion: completion_smoke::Run,
    annotation_defaults: annotation_defaults_smoke::Run,
    local_smoke: local_image_smoke::Run,
    config_smoke: config_transfer_smoke::Run,
    guide_smoke: onboarding_smoke::Run,
    pass_smoke: pin_pass_smoke::Run,
    rotation_smoke: pin_rotation_smoke::Run,
    edit_smoke: pin_edit_smoke::Run,
    wheel_smoke: pin_wheel_smoke::Run,
    geometry_smoke: pin_geometry_smoke::Run,
    repeat_smoke: repeat_smoke::Run,
    delay_smoke: delay_smoke::Run,
    cursor_smoke: cursor_smoke::Run,
    magnifier_smoke: magnifier_smoke::Run,
    screens_smoke: screens_smoke::Run,
    color_picker_smoke: color_picker_smoke::Run,
    pin_smoke: pins_smoke::Run,
    ui: slint::Weak<SettingsWindow>,
    app: Rc<RefCell<AppState>>,
    hotkeys: Rc<glint_platform::HotkeyService>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    phase: u16,
    toolbar_phase: u8,
    child: Option<std::process::Child>,
    child_started: std::time::Instant,
    export_rect: Option<glint_core::Rect>,
    picked_hex: String,
    picked_rgb: String,
    clipboard_blocker: Option<glint_platform::ClipboardBlocker>,
}
fn has_marker(bytes: &[u8], width: u32) -> bool {
    bytes.chunks(width as usize * 4).any(|row| {
        let mut run = 0;
        row.as_chunks::<4>().0.iter().any(|pixel| {
            run = if *pixel == [241, 12, 222, 255] {
                run + 1
            } else {
                0
            };
            run >= 40
        })
    })
}
pub fn install(
    ui: &SettingsWindow,
    state: Rc<RefCell<AppState>>,
    hotkeys: Rc<glint_platform::HotkeyService>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    focused: u16,
) {
    let run = Rc::new(RefCell::new(Run {
        log: Default::default(),
        window_fixture: None,
        window_fixture_region: None,
        local_dialog: Default::default(),
        config_dialog: Default::default(),
        output_conflict: Default::default(),
        output_settings: Default::default(),
        exclusions: Default::default(),
        fonts: Default::default(),
        diagnostics: Default::default(),
        lifecycle: Default::default(),
        preview_budget: Default::default(),
        focused,
        motion: Default::default(),
        completion: Default::default(),
        annotation_defaults: Default::default(),
        local_smoke: Default::default(),
        config_smoke: Default::default(),
        guide_smoke: Default::default(),
        pass_smoke: Default::default(),
        rotation_smoke: Default::default(),
        edit_smoke: Default::default(),
        wheel_smoke: Default::default(),
        geometry_smoke: Default::default(),
        repeat_smoke: Default::default(),
        delay_smoke: Default::default(),
        cursor_smoke: Default::default(),
        magnifier_smoke: Default::default(),
        screens_smoke: Default::default(),
        color_picker_smoke: Default::default(),
        pin_smoke: Default::default(),
        ui: ui.as_weak(),
        app: state,
        hotkeys,
        directory,
        error,
        scale: Default::default(),
        phase: focused,
        toolbar_phase: 0,
        child: None,
        child_started: std::time::Instant::now(),
        export_rect: None,
        picked_hex: String::new(),
        picked_rgb: String::new(),
        clipboard_blocker: None,
    }));
    slint::Timer::single_shot(Duration::from_millis(600), move || tick(run));
}

// Deliberately synthetic geometry, separate from the real desktop smoke below.
// No window is shown and no output success is simulated.
fn verify_window_snapshot_adapter(directory: &std::path::Path) -> Result<(), String> {
    let snapshot = |fail: bool| glint_platform::DesktopSnapshot {
        displays: vec![],
        frame: glint_platform::DesktopFrame {
            x: -320,
            y: -40,
            width: 320,
            height: 240,
            rgba: vec![255; 320 * 240 * 4],
        },
        windows: if fail {
            vec![]
        } else {
            vec![glint_platform::WindowCandidate {
                id: 1,
                bounds: glint_platform::ScreenBounds {
                    left: -300,
                    top: -10,
                    right: -100,
                    bottom: 140,
                },
                extended_frame_bounds: true,
            }]
        },
        pointer: Some((-200, 20)),
        detection_error: fail.then(|| "diagnostic enumeration failure".into()),
    };
    let ui = crate::probe::resident_capture(
        snapshot(false),
        true,
        crate::probe::Presentation::default(),
        None,
        || {},
    )?;
    if !ui.get_selected()
        || ui.get_left() != 20.0
        || ui.get_top() != 30.0
        || ui.get_wide() != 200.0
        || ui.get_high() != 150.0
    {
        return Err("Explicit window capture did not commit initial frozen geometry".into());
    }
    // Render a deliberately tiny viewport to catch controls escaping the screen,
    // then verify the normal-size bottom/right case really moves above the selection.
    for (width, height) in [(320.0, 240.0), (1024.0, 768.0)] {
        ui.window().set_size(slint::LogicalSize::new(width, height));
        ui.set_left(290.0);
        ui.set_top(190.0);
        ui.set_wide(25.0);
        ui.set_high(45.0);
        ui.set_tool(2);
        let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
        let x = ui.get_toolbar_x();
        let y = ui.get_toolbar_y();
        if x < 0.0
            || y < 0.0
            || x + ui.get_toolbar_width() > width
            || y + ui.get_toolbar_total_height() > height
        {
            return Err(format!(
                "Toolbar/properties escape {width}x{height}: ({x},{y})"
            ));
        }
        if width == 1024.0 && y + ui.get_toolbar_total_height() > 190.0 / 240.0 * height {
            return Err("Bottom-edge toolbar did not move above the selection".into());
        }
        image::save_buffer_with_format(
            directory.join(format!("synthetic-toolbar-{width:.0}.png")),
            pixels.as_bytes(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| e.to_string())?;
        ui.set_tool(6);
        ui.invoke_hover(40.0, 50.0);
        let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
        if ui.get_toolbar_y() < 0.0
            || ui.get_toolbar_y() + ui.get_toolbar_total_height() > height
            || !ui.get_sample_valid()
        {
            return Err(format!(
                "Picker panel escapes {width}x{height} or loses its source sample"
            ));
        }
        image::save_buffer_with_format(
            directory.join(format!("synthetic-picker-{width:.0}.png")),
            pixels.as_bytes(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| e.to_string())?;
        ui.set_tool(8);
        ui.invoke_pointer(0, 40.0, 50.0);
        ui.set_text_anchor_x(315.0);
        ui.set_text_anchor_y(235.0);
        if !ui.get_text_editing() {
            return Err("Text input did not open in the edge viewport".into());
        }
        capture_snapshot(
            &ui,
            &directory.join(format!("synthetic-text-editor-{width:.0}.png")),
        )?;
        ui.invoke_action("cancel-text".into());
    }
    drop(ui);
    let finished = Rc::new(std::cell::Cell::new(0));
    let notified = finished.clone();
    let ui = crate::probe::resident_capture(
        snapshot(true),
        true,
        crate::probe::Presentation::default(),
        None,
        move || notified.set(notified.get() + 1),
    )?;
    if ui.get_selected() || !ui.get_status().contains("diagnostic enumeration failure") {
        return Err("Detection failure did not expose manual-selection fallback".into());
    }
    ui.invoke_pointer(0, 20.0, 30.0);
    ui.invoke_pointer(1, 220.0, 180.0);
    ui.invoke_pointer(2, 220.0, 180.0);
    if !ui.get_selected() || ui.get_wide() != 200.0 || ui.get_high() != 150.0 {
        return Err("Detection failure prevented manual selection".into());
    }
    ui.window().take_snapshot().map_err(|e| e.to_string())?;
    // Framework input dispatch exercises the actual Slint controls and key bubbling;
    // it does not synthesize physical OS input.
    use slint::platform::{Key, PointerEventButton, WindowEvent};
    let pen = slint::LogicalPosition::new(
        ui.get_toolbar_x() + 7.0 + (ui.get_toolbar_width() - 14.0) / 4.0 * 3.5,
        ui.get_toolbar_y() + 19.0,
    );
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: pen,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: pen,
        button: PointerEventButton::Left,
    });
    if ui.get_tool() != 2 {
        return Err("Native toolbar pen button did not select the pen".into());
    }
    ui.invoke_pointer(0, 40.0, 50.0);
    ui.invoke_pointer(2, 80.0, 80.0);
    if ui.get_object_count() != 1 {
        return Err("Keyboard fixture did not draw a stroke".into());
    }
    let press =
        |text: slint::SharedString| ui.window().dispatch_event(WindowEvent::KeyPressed { text });
    let release = |text: slint::SharedString| {
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text })
    };
    press(Key::Control.into());
    press("z".into());
    release("z".into());
    if ui.get_object_count() != 0 {
        return Err("Native Ctrl+Z did not undo".into());
    }
    press(Key::Shift.into());
    press("Z".into());
    release("Z".into());
    release(Key::Shift.into());
    release(Key::Control.into());
    if ui.get_object_count() != 1 {
        return Err("Native Ctrl+Shift+Z did not redo".into());
    }
    ui.set_tool(6);
    ui.invoke_hover(40.9, 50.8);
    if !ui.get_sample_valid()
        || ui.get_sample_hex() != "#FFFFFF"
        || ui.get_sample_rgb() != "RGB(255, 255, 255)"
        || ui.get_sample_position() != "(-280, 10)"
    {
        return Err(
            "Picker did not floor source coordinates or preserve negative desktop origin".into(),
        );
    }
    ui.invoke_hover(0.0, 0.0);
    ui.invoke_pointer(0, 0.0, 0.0);
    ui.invoke_pointer(2, 0.0, 0.0);
    if ui.get_sample_valid()
        || ui.get_object_count() != 1
        || !ui.get_selected()
        || ui.get_left() != 20.0
        || ui.get_wide() != 200.0
    {
        return Err(
            "Picker outside selection changed the crop/annotations or accepted an invalid pixel"
                .into(),
        );
    }
    ui.set_tool(2);
    press(Key::Tab.into());
    release(Key::Tab.into());
    press(Key::Escape.into());
    release(Key::Escape.into());
    if finished.get() != 1 {
        return Err("Escape did not cancel after toolbar focus traversal".into());
    }
    let point = slint::LogicalPosition::new(310.0, 220.0);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: point,
        button: PointerEventButton::Right,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: point,
        button: PointerEventButton::Right,
    });
    if finished.get() != 1 {
        return Err("Repeated cancel notified completion more than once".into());
    }
    drop(ui);
    let finished = Rc::new(std::cell::Cell::new(false));
    let notified = finished.clone();
    let ui = crate::probe::resident_capture(
        snapshot(false),
        false,
        crate::probe::Presentation::default(),
        None,
        move || notified.set(true),
    )?;
    ui.window().take_snapshot().map_err(|e| e.to_string())?;
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: point,
        button: PointerEventButton::Right,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: point,
        button: PointerEventButton::Right,
    });
    if !finished.get() {
        return Err("Native right-click did not cancel the overlay".into());
    }
    Ok(())
}
fn tick(run: Rc<RefCell<Run>>) {
    let result = run.borrow_mut().step();
    match result {
        Ok(true) => {}
        Ok(false) => slint::Timer::single_shot(Duration::from_millis(180), move || tick(run)),
        Err(message) => {
            let mut state = run.borrow_mut();
            if let Some(child) = &mut state.child
                && matches!(child.try_wait(), Ok(None))
            {
                let _ = child.kill();
                let _ = child.wait();
            }
            *state.error.borrow_mut() =
                Some(format!("Resident smoke phase {}: {message}", state.phase));
            let _ = slint::quit_event_loop();
        }
    }
}
impl Run {
    fn step(&mut self) -> Result<bool, String> {
        let ui = self
            .ui
            .upgrade()
            .ok_or("Settings window vanished during verification")?;
        match self.phase {
            0 => {
                verify_window_snapshot_adapter(&self.directory)?;
                pixelation_smoke::verify(&self.directory)?;
                crate::probe::verify_objects(&self.directory)?;
                ui.invoke_change("general.language".into(), "zh_cn".into());
                ui.invoke_choose_page(8);
            }
            1 => {
                snapshot(&ui, &self.directory.join("settings-light.png"))?;
                ui.invoke_change("appearance.theme".into(), "dark".into());
                ui.invoke_change("general.language".into(), "en".into());
            }
            2 => {
                if !ui.get_dark()
                    || !ui.get_dirty()
                    || !ui.get_writable()
                    || ui.get_heading() != "Settings"
                {
                    return Err("Theme/language draft did not reach the native properties".into());
                }
                snapshot(&ui, &self.directory.join("settings-dark-en.png"))?;
                ui.invoke_action("close".into());
                if ui.get_modal() != 1 || !ui.window().is_visible() {
                    return Err("Unsaved close was not intercepted".into());
                }
            }
            3 => {
                if self.toolbar_phase == 0 {
                    snapshot(&ui, &self.directory.join("unsaved-close.png"))?;
                    ui.invoke_action("continue".into());
                    verify(&ui, &self.app, &self.hotkeys, &self.directory)?;
                }
                if self.toolbar_phase < 4 {
                    settings_toolbar_smoke::pointer_stage(
                        &ui,
                        &self.app,
                        &self.directory,
                        self.toolbar_phase,
                    )?;
                    self.toolbar_phase += 1;
                    return Ok(false);
                }
                if self.toolbar_phase == 4 {
                    settings_toolbar_smoke::verify(&ui, &self.app, &self.directory)?;
                }
                if self.toolbar_phase < 11 {
                    settings_toolbar_smoke::layout_stage(
                        &ui,
                        &self.directory,
                        self.toolbar_phase - 4,
                    )?;
                    self.toolbar_phase += 1;
                    return Ok(false);
                }
            }
            4 => {
                ui.invoke_record("shortcuts.capture".into());
                ui.invoke_shortcut_input("\u{f71a}".into(), true, true, true, false);
                ui.invoke_action("accept-key".into());
                ui.invoke_action("save".into());
                if self.app.borrow().session.preferences.is_dirty() {
                    return Err("Capture shortcut save failed".into());
                }
                if glint_platform::HotkeyService::new(
                    vec![glint_platform::HotkeyBinding {
                        action: 1,
                        modifiers: 7,
                        key: 0x86,
                    }],
                    |_| {},
                )
                .is_ok()
                {
                    return Err("Capture shortcut was not actually registered".into());
                }
                ui.invoke_record("shortcuts.window".into());
                ui.invoke_shortcut_input("\u{f71b}".into(), true, true, true, false);
                ui.invoke_action("accept-key".into());
                ui.invoke_action("save".into());
                if self
                    .app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .shortcuts
                    .window
                    != "Ctrl+Alt+Shift+F24"
                    || self.app.borrow().session.preferences.is_dirty()
                {
                    return Err(
                        "Window shortcut recorder modified the wrong binding or failed to save"
                            .into(),
                    );
                }
                if glint_platform::HotkeyService::new(
                    vec![glint_platform::HotkeyBinding {
                        action: 2,
                        modifiers: 7,
                        key: 0x87,
                    }],
                    |_| {},
                )
                .is_ok()
                {
                    return Err("Window shortcut was not actually registered".into());
                }
                ui.invoke_choose_page(0);
            }
            5 => {
                snapshot(&ui, &self.directory.join("general.png"))?;
                ui.invoke_choose_page(12);
                ui.window().set_size(slint::LogicalSize::new(900.0, 650.0));
            }
            6 => {
                snapshot(&ui, &self.directory.join("about-900.png"))?;
                ui.invoke_action("close".into());
            }
            7 => {
                use slint::winit_030::WinitWindowAccessor;
                if ui.window().is_visible()
                    || ui
                        .window()
                        .with_winit_window(|window| window.is_visible())
                        .flatten()
                        == Some(true)
                {
                    return Err("Closed settings still have a visible native window".into());
                }
                let mut command =
                    std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
                command
                    .arg("--background")
                    .arg("--config-dir")
                    .arg(&self.directory);
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000); // CREATE_NO_WINDOW: own short-lived activation child.
                self.child = Some(command.spawn().map_err(|e| e.to_string())?);
                self.child_started = std::time::Instant::now();
            }
            8 => {
                let status = self
                    .child
                    .as_mut()
                    .ok_or("Activation child missing")?
                    .try_wait()
                    .map_err(|e| e.to_string())?;
                if status.is_none() || !ui.window().is_visible() {
                    if self.child_started.elapsed() > Duration::from_secs(4) {
                        return Err(
                            "Second process did not activate the resident window and exit".into(),
                        );
                    }
                    return Ok(false);
                }
                if !status.unwrap().success() {
                    return Err("Activation child exited with an error".into());
                }
                ui.set_diagnostic_marker(true);
                self.child_started = std::time::Instant::now();
            }
            9 => {
                if self.window_fixture.is_none() {
                    let visible = glint_platform::capture_desktop()?;
                    if !has_marker(&visible.rgba, visible.width) {
                        let path = self.directory.join("marker-wait-desktop.png");
                        if !path.exists() {
                            image::save_buffer_with_format(
                                &path,
                                &visible.rgba,
                                visible.width,
                                visible.height,
                                image::ColorType::Rgba8,
                                image::ImageFormat::Png,
                            )
                            .map_err(|e| e.to_string())?;
                            snapshot(&ui, &self.directory.join("marker-wait-window.png"))?;
                            eprintln!(
                                "Waiting for diagnostic marker on actual desktop after native reactivation; assertion remains pending"
                            );
                        }
                        if self.child_started.elapsed() < Duration::from_secs(5) {
                            return Ok(false);
                        }
                        return Err("Diagnostic marker was not presented on the actual desktop within five seconds".into());
                    }
                    self.window_fixture = Some(WindowFixture::show()?);
                    // Slint defers native show/layout work until control returns
                    // to its event loop. Capture only on the next smoke tick.
                    return Ok(false);
                }
                self.window_fixture_region = Some(self.window_fixture.as_ref().unwrap().region()?);
                ui.invoke_change("appearance.theme".into(), "dark".into());
                ui.invoke_action("capture".into());
                if ui.get_modal() != 1
                    || self.app.borrow().capturing
                    || self.app.borrow().capture.is_some()
                {
                    return Err("Unsaved capture entry was not intercepted".into());
                }
                ui.invoke_action("continue".into());
                if self.app.borrow().capture_pending {
                    return Err("Continue editing retained the pending capture".into());
                }
                ui.invoke_action("capture".into());
                ui.invoke_action("discard-leave".into());
                self.child_started = std::time::Instant::now();
            }
            10 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err(format!(
                            "Window detection capture did not open: {}",
                            ui.get_status()
                        ));
                    }
                    return Ok(false);
                };
                let frozen = capture
                    .get_desktop()
                    .to_rgba8()
                    .ok_or("Cannot inspect frozen desktop")?;
                if capture.get_dark()
                    || capture.get_english()
                    || capture.get_labels().copy != "复制"
                {
                    return Err(
                        "Discarded appearance draft leaked into capture presentation".into(),
                    );
                }
                if has_marker(frozen.as_bytes(), frozen.width()) {
                    return Err("Settings marker leaked into the frozen window scene".into());
                }
                let candidates = capture.get_window_regions();
                let fixture = self
                    .window_fixture_region
                    .clone()
                    .ok_or("Missing real window fixture region")?;
                let target = candidates.iter().find(|w| *w == fixture).ok_or_else(|| {
                    format!(
                        "Real fixture region ({}, {}, {}, {}) was not in frozen candidates {:?}",
                        fixture.left,
                        fixture.top,
                        fixture.wide,
                        fixture.high,
                        candidates
                            .iter()
                            .map(|w| (w.left, w.top, w.wide, w.high))
                            .collect::<Vec<_>>()
                    )
                })?;
                let point = (
                    target.left + target.wide / 2.0,
                    target.top + target.high / 2.0,
                );
                let expected = candidates
                    .iter()
                    .find(|w| {
                        point.0 >= w.left
                            && point.0 < w.left + w.wide
                            && point.1 >= w.top
                            && point.1 < w.top + w.high
                    })
                    .ok_or("Missing overlapping window")?;
                if expected != fixture {
                    return Err("Topmost real fixture was not the frontmost frozen window".into());
                }
                let sample =
                    (((fixture.top as u32 + 10) * frozen.width() + fixture.left as u32 + 10) * 4)
                        as usize;
                if frozen.as_bytes().get(sample..sample + 4) != Some(&[66, 106, 137, 255]) {
                    return Err(
                        "Real window fixture pixels were not captured at its frozen geometry"
                            .into(),
                    );
                }
                capture.invoke_hover(point.0, point.1);
                if capture.get_selected()
                    || !capture.get_has_region()
                    || capture.get_left() != expected.left
                    || capture.get_top() != expected.top
                    || capture.get_wide() != expected.wide
                    || capture.get_high() != expected.high
                {
                    return Err("Frozen-window hover did not highlight the frontmost window without committing".into());
                }
                capture.invoke_pointer(0, point.0, point.1);
                capture.invoke_pointer(2, point.0, point.1);
                if !capture.get_selected()
                    || capture.get_left() != expected.left
                    || capture.get_top() != expected.top
                    || capture.get_wide() != expected.wide
                    || capture.get_high() != expected.high
                {
                    return Err("Single click did not commit the detected window".into());
                }
                ui.set_diagnostic_marker(false);
            }
            11 => {
                let app = self.app.borrow();
                let capture = app.capture.as_ref().ok_or("Selected window vanished")?;
                let pixels = capture
                    .window()
                    .take_snapshot()
                    .map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    self.directory.join("window-selected.png"),
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                capture.invoke_action("cancel".into());
                self.window_fixture.take();
                self.window_fixture_region.take();
            }
            12 => {
                if self.app.borrow().capture.is_some() {
                    return Err("Window selection cancellation retained the overlay".into());
                }
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            13 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err(format!(
                            "Resident capture did not open: {}",
                            ui.get_status()
                        ));
                    }
                    return Ok(false);
                };
                if ui.window().is_visible() {
                    return Err("Settings remained visible during capture".into());
                }
                let w = capture.get_source_w();
                let h = capture.get_source_h();
                let frozen = capture
                    .get_desktop()
                    .to_rgba8()
                    .ok_or("Cannot inspect frozen desktop")?;
                if has_marker(frozen.as_bytes(), frozen.width()) {
                    return Err("Settings marker leaked into the frozen desktop".into());
                }
                ui.set_diagnostic_marker(false);
                capture.invoke_pointer(0, w * 0.1, h * 0.15);
                capture.invoke_pointer(1, w * 0.6, h * 0.7);
                capture.invoke_pointer(2, w * 0.6, h * 0.7);
                capture.set_tool(1);
                capture.invoke_pointer(0, w * 0.2, h * 0.3);
                capture.invoke_pointer(1, w * 0.4, h * 0.4);
                capture.invoke_pointer(2, w * 0.4, h * 0.4);
                if capture.get_object_count() != 1 {
                    return Err("Resident redaction was not committed".into());
                }
                capture.invoke_action("undo".into());
                if capture.get_object_count() != 0 {
                    return Err("Resident undo failed".into());
                }
                capture.invoke_action("redo".into());
                for (tool, a, b) in [
                    (3, (0.30, 0.25), (0.12, 0.17)),
                    (4, (0.45, 0.20), (0.55, 0.30)),
                    (5, (0.48, 0.50), (0.56, 0.58)),
                ] {
                    capture.set_tool(tool);
                    capture.invoke_pointer(0, w * a.0, h * a.1);
                    if !capture.get_output_ready() {
                        return Err("Zero-length shape draft paused output".into());
                    }
                    capture.invoke_pointer(1, w * b.0, h * b.1);
                    capture.invoke_pointer(2, w * b.0, h * b.1);
                }
                if capture.get_object_count() != 4 {
                    return Err("Rectangle/arrow/highlight were not committed".into());
                }
                for expected in (1..=3).rev() {
                    capture.invoke_action("undo".into());
                    if capture.get_object_count() != expected {
                        return Err("Shape undo failed".into());
                    }
                }
                for expected in 2..=4 {
                    capture.invoke_action("redo".into());
                    if capture.get_object_count() != expected {
                        return Err("Shape redo failed".into());
                    }
                }
                let tiles: Vec<_> = capture
                    .get_tiles()
                    .iter()
                    // Slint's empty Image is not equal to itself. Hollow shapes
                    // and recycled undo rows legitimately contain empty images.
                    .map(|tile| (tile.picture.size().width > 0).then_some(tile.picture))
                    .collect();
                let before = (
                    capture.get_left(),
                    capture.get_top(),
                    capture.get_wide(),
                    capture.get_high(),
                );
                capture.set_tool(0);
                let center = (before.0 + before.2 / 2.0, before.1 + before.3 / 2.0);
                capture.invoke_hover(center.0, center.1);
                if capture.get_cursor() != 1 {
                    return Err("Selection interior does not use move cursor".into());
                }
                capture.invoke_pointer(0, center.0, center.1);
                capture.invoke_pointer(1, center.0 + 37.0, center.1 + 23.0);
                capture.invoke_pointer(2, center.0 + 37.0, center.1 + 23.0);
                if capture.get_left() != before.0 + 37.0
                    || capture.get_top() != before.1 + 23.0
                    || capture.get_wide() != before.2
                    || capture.get_object_count() != 4
                {
                    return Err("Selection move changed size or lost annotation".into());
                }
                let corner = (capture.get_left(), capture.get_top());
                capture.invoke_hover(corner.0, corner.1);
                if capture.get_cursor() != 4 {
                    return Err("Selection corner does not use diagonal cursor".into());
                }
                capture.invoke_pointer(0, corner.0, corner.1);
                capture.invoke_pointer(2, corner.0 - 11.0, corner.1 - 7.0);
                if capture.get_wide() != before.2 + 11.0
                    || capture.get_high() != before.3 + 7.0
                    || capture.get_object_count() != 4
                {
                    return Err(
                        "Selection resize changed opposite anchor or lost annotation".into(),
                    );
                }
                if capture
                    .get_tiles()
                    .iter()
                    .map(|tile| (tile.picture.size().width > 0).then_some(tile.picture))
                    .collect::<Vec<_>>()
                    != tiles
                {
                    return Err("Selection move/resize rerasterized annotation tiles".into());
                }
                capture.set_tool(7);
                capture.invoke_pointer(0, w * 0.35, h * 0.32);
                capture.invoke_pointer(1, w * 0.48, h * 0.46);
                capture.invoke_pointer(2, w * 0.48, h * 0.46);
                if capture.get_object_count() != 5 || !capture.get_output_ready() {
                    return Err("Real desktop pixelation was not committed".into());
                }
            }
            14 => {
                let app = self.app.borrow();
                let capture = app
                    .capture
                    .as_ref()
                    .ok_or("Capture disappeared before output")?;
                self.export_rect = Some(glint_core::Rect {
                    x: capture.get_left() as u32,
                    y: capture.get_top() as u32,
                    width: capture.get_wide() as u32,
                    height: capture.get_high() as u32,
                });
                // Compare flattened annotation pixels with transient pointer UI hidden.
                capture.invoke_canvas_left();
                let pixels = capture
                    .window()
                    .take_snapshot()
                    .map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    self.directory.join("resident-overlay.png"),
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                capture.invoke_action("save".into());
                self.child_started = std::time::Instant::now();
            }
            15 => {
                if let Some(capture) = &self.app.borrow().capture {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err(format!(
                            "Resident output did not finish: {}",
                            capture.get_status()
                        ));
                    }
                    return Ok(false);
                }
                let pixels = image::open(self.directory.join("resident-capture.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                let sample = pixels
                    .get_pixel(pixels.width() * 3 / 10, pixels.height() * 4 / 10)
                    .0;
                if sample != [32, 43, 66, 255] {
                    return Err(format!("Resident PNG redaction differs: {sample:?}"));
                }
                let rect = self.export_rect.ok_or("Missing output geometry")?;
                let preview = image::open(self.directory.join("resident-overlay.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                for y in 4..rect.height - 4 {
                    for x in 4..rect.width - 4 {
                        for (a, b) in pixels
                            .get_pixel(x, y)
                            .0
                            .into_iter()
                            .zip(preview.get_pixel(rect.x + x, rect.y + y).0)
                        {
                            if i16::from(a).abs_diff(i16::from(b)) > 1 {
                                return Err("Moved/resized preview and PNG pixels differ".into());
                            }
                        }
                    }
                }
                ui.invoke_change("capture.detect_window".into(), "false".into());
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_change("appearance.theme".into(), "dark".into());
                ui.invoke_change("appearance.accent".into(), "#168975".into());
                ui.invoke_change("appearance.high_contrast".into(), "true".into());
                ui.invoke_change("annotation.arrow_style".into(), "open".into());
                ui.invoke_action("save".into());
                if self
                    .app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .capture
                    .detect_window
                {
                    return Err("Window detection option did not save".into());
                }
                ui.invoke_action("window-capture".into());
                ui.invoke_action("capture".into());
                if !self.app.borrow().window_capture {
                    return Err("Repeated capture action replaced the pending window mode".into());
                }
                self.child_started = std::time::Instant::now();
            }
            16 => {
                if let Some(capture) = &self.app.borrow().capture {
                    if !capture.get_dark()
                        || capture.get_filled_arrows()
                        || !capture.get_english()
                        || !capture.get_contrast()
                        || capture.get_labels().copy != "Copy"
                        || capture.get_accent() != slint::Color::from_rgb_u8(0x16, 0x89, 0x75)
                        || capture.get_surface() != slint::Color::from_rgb_u8(0x1c, 0x24, 0x32)
                    {
                        return Err(
                            "Saved language/theme/accent/contrast did not reach capture".into()
                        );
                    }
                    if !self.app.borrow().window_capture
                        || capture.get_window_regions().row_count() == 0
                    {
                        return Err(
                            "Explicit window entry did not override disabled automatic detection"
                                .into(),
                        );
                    }
                    capture.invoke_pointer(0, 100.0, 100.0);
                    capture.invoke_pointer(2, 500.0, 400.0);
                    capture.set_tool(2);
                    capture.invoke_action("save".into());
                    self.child_started = std::time::Instant::now();
                } else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Second capture did not open".into());
                    }
                    return Ok(false);
                }
            }
            17 => {
                let app = self.app.borrow();
                let capture = app
                    .capture
                    .as_ref()
                    .ok_or("Failed output closed the editor")?;
                if capture.get_busy() {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Failed output did not return".into());
                    }
                    return Ok(false);
                }
                if !capture.get_selected()
                    || !capture.window().is_visible()
                    || !capture.get_status().contains("Output failed")
                {
                    return Err(
                        "Existing output did not preserve visible selection and error".into(),
                    );
                }
                let pixels = capture
                    .window()
                    .take_snapshot()
                    .map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    self.directory.join("resident-dark-en.png"),
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                capture.invoke_action("cancel".into());
                capture.invoke_action("cancel".into());
            }
            18 => {
                if self.app.borrow().capture.is_some() {
                    return Err("Cancel retained the resident capture".into());
                }
                ui.invoke_action("open".into());
                if !ui.window().is_visible() {
                    return Err("Capture completion stopped the resident event loop".into());
                }
                ui.invoke_record("shortcuts.window".into());
                ui.invoke_action("clear-key".into());
                ui.invoke_action("save".into());
                let released = glint_platform::HotkeyService::new(
                    vec![glint_platform::HotkeyBinding {
                        action: 2,
                        modifiers: 7,
                        key: 0x87,
                    }],
                    |_| {},
                )?;
                drop(released);
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            19 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Picker capture did not open".into());
                    }
                    return Ok(false);
                };
                capture.invoke_pointer(0, 100.0, 100.0);
                capture.invoke_pointer(2, 500.0, 400.0);
                capture.set_tool(1);
                capture.invoke_pointer(0, 150.0, 150.0);
                capture.invoke_pointer(2, 300.0, 300.0);
                let frozen = capture
                    .get_desktop()
                    .to_rgba8()
                    .ok_or("Missing picker source")?;
                let (point, pixel) = (170..280)
                    .find_map(|x| {
                        let rgba = glint_core::sample_rgba(
                            frozen.as_bytes(),
                            (frozen.width(), frozen.height()),
                            (x, 200),
                        )?;
                        (rgba[..3] != [32, 43, 66]).then_some(((x, 200), rgba))
                    })
                    .ok_or("No distinct original pixel to verify exclusion of redaction")?;
                self.picked_hex = format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
                self.picked_rgb = format!("RGB({}, {}, {})", pixel[0], pixel[1], pixel[2]);
                capture.set_tool(6);
                capture.invoke_hover(point.0 as f32, point.1 as f32);
                if capture.get_sample_hex() != self.picked_hex
                    || capture.get_sample_rgb() != self.picked_rgb
                    || capture.get_object_count() != 1
                    || !capture.get_sample_valid()
                {
                    return Err(
                        "Picker sampled UI/annotation pixels instead of the frozen source".into(),
                    );
                }
                capture.invoke_pointer(0, point.0 as f32, point.1 as f32);
                capture.invoke_pointer(2, point.0 as f32, point.1 as f32);
                self.child_started = std::time::Instant::now();
            }
            20..=23 => {
                let app = self.app.borrow();
                let capture = app.capture.as_ref().ok_or("Color copy closed the editor")?;
                if capture.get_picker_busy() {
                    if self.child_started.elapsed() > Duration::from_secs(15) {
                        return Err("Color copy worker did not finish".into());
                    }
                    return Ok(false);
                }
                if !capture.get_selected()
                    || capture.get_object_count() != 1
                    || !capture.window().is_visible()
                {
                    return Err("Color copy lost the visible selection or annotations".into());
                }
                if self.phase == 22 {
                    let pixels = capture
                        .window()
                        .take_snapshot()
                        .map_err(|e| e.to_string())?;
                    image::save_buffer_with_format(
                        self.directory.join("picker-occupied.png"),
                        pixels.as_bytes(),
                        pixels.width(),
                        pixels.height(),
                        image::ColorType::Rgba8,
                        image::ImageFormat::Png,
                    )
                    .map_err(|e| e.to_string())?;
                    if !capture.get_picker_feedback().contains("Color copy failed") {
                        return Err(
                            "Occupied clipboard was not reported as a real copy failure".into()
                        );
                    }
                    self.clipboard_blocker.take();
                    capture.invoke_action("copy-color-hex".into());
                } else {
                    let expected = if self.phase == 21 {
                        &self.picked_rgb
                    } else {
                        &self.picked_hex
                    };
                    if glint_platform::read_clipboard_text()? != *expected
                        || !capture.get_picker_feedback().contains("Color copied")
                    {
                        return Err("Native color copy did not write the expected text".into());
                    }
                    match self.phase {
                        20 => capture.invoke_action("copy-color-rgb".into()),
                        21 => {
                            self.clipboard_blocker =
                                Some(glint_platform::ClipboardBlocker::acquire()?);
                            capture.invoke_action("copy-color-hex".into());
                        }
                        _ => {
                            let pixels = capture
                                .window()
                                .take_snapshot()
                                .map_err(|e| e.to_string())?;
                            image::save_buffer_with_format(
                                self.directory.join("picker-native.png"),
                                pixels.as_bytes(),
                                pixels.width(),
                                pixels.height(),
                                image::ColorType::Rgba8,
                                image::ImageFormat::Png,
                            )
                            .map_err(|e| e.to_string())?;
                            capture.invoke_action("cancel".into());
                        }
                    }
                }
                self.child_started = std::time::Instant::now();
            }
            24 => {
                ui.invoke_change("annotation.text_font".into(), "Segoe UI".into());
                ui.invoke_change("annotation.text_size".into(), "36".into());
                ui.invoke_action("save".into());
                if self.app.borrow().session.preferences.is_dirty() {
                    return Err("Text defaults failed to save".into());
                }
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            25 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Text capture did not open".into());
                    }
                    return Ok(false);
                };
                if capture.get_text_font() != "Segoe UI" || capture.get_text_size() != 36 {
                    return Err("Saved text defaults did not reach capture".into());
                }
                capture.invoke_pointer(0, 100.0, 100.0);
                capture.invoke_pointer(2, 900.0, 600.0);
                capture.set_tool(8);
                capture.invoke_pointer(0, 150.0, 200.0);
                capture.set_text_value("   ".into());
                capture.invoke_action("commit-text".into());
                if capture.get_text_editing() || capture.get_object_count() != 0 {
                    return Err("Blank text committed an object".into());
                }
                capture.invoke_pointer(0, 150.0, 200.0);
                capture.set_text_font("Glint Missing Font 913".into());
                capture.set_text_value("Glint 拾光 & <text>".into());
                use slint::platform::{Key, WindowEvent};
                capture.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Return.into(),
                });
                capture.window().dispatch_event(WindowEvent::KeyReleased {
                    text: Key::Return.into(),
                });
                self.child_started = std::time::Instant::now();
            }
            26 | 27 => {
                let app = self.app.borrow();
                let capture = app.capture.as_ref().ok_or("Text closed the editor")?;
                if capture.get_text_busy() {
                    if self.child_started.elapsed() > Duration::from_secs(10) {
                        return Err("Text worker timed out".into());
                    }
                    return Ok(false);
                }
                if self.phase == 26 {
                    if !capture.get_text_editing()
                        || capture.get_object_count() != 0
                        || !capture.get_text_feedback().contains("Text failed")
                        || capture.get_text_value() != "Glint 拾光 & <text>"
                    {
                        return Err("Font failure did not retain native text input".into());
                    }
                    capture_snapshot(capture, &self.directory.join("text-font-failed.png"))?;
                    // Exercise the actual inline font field and its two-way binding.
                    use slint::platform::{Key, PointerEventButton, WindowEvent};
                    let position = slint::LogicalPosition::new(400.0, 265.0);
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
                    capture.window().dispatch_event(WindowEvent::KeyPressed {
                        text: Key::Control.into(),
                    });
                    capture
                        .window()
                        .dispatch_event(WindowEvent::KeyPressed { text: "a".into() });
                    capture
                        .window()
                        .dispatch_event(WindowEvent::KeyReleased { text: "a".into() });
                    capture.window().dispatch_event(WindowEvent::KeyReleased {
                        text: Key::Control.into(),
                    });
                    capture.window().dispatch_event(WindowEvent::KeyPressed {
                        text: "Segoe UI".into(),
                    });
                    capture.window().dispatch_event(WindowEvent::KeyReleased {
                        text: "Segoe UI".into(),
                    });
                    if capture.get_text_font() != "Segoe UI" {
                        return Err("Inline font field did not accept correction".into());
                    }
                    capture.invoke_action("commit-text".into());
                    self.child_started = std::time::Instant::now();
                } else {
                    if capture.get_text_editing()
                        || capture.get_object_count() != 1
                        || !capture.get_output_ready()
                    {
                        return Err("Text retry did not commit exactly one object".into());
                    }
                    capture.invoke_action("undo".into());
                    if capture.get_object_count() != 0 {
                        return Err("Text undo failed".into());
                    }
                    capture.invoke_action("redo".into());
                    capture.set_tool(0);
                    capture.invoke_pointer(0, 180.0, 220.0);
                    capture.invoke_pointer(1, 220.0, 250.0);
                    capture.invoke_pointer(2, 220.0, 250.0);
                    if !capture.get_object_selected()
                        || capture.get_object_left() != 90.0
                        || capture.get_object_top() != 130.0
                    {
                        return Err("Existing text could not be selected and moved".into());
                    }
                    capture.invoke_focus_canvas();
                    use slint::platform::{Key, WindowEvent};
                    capture.window().dispatch_event(WindowEvent::KeyPressed {
                        text: Key::Delete.into(),
                    });
                    capture.window().dispatch_event(WindowEvent::KeyReleased {
                        text: Key::Delete.into(),
                    });
                    if capture.get_object_count() != 0 || !capture.get_can_undo() {
                        return Err("Text deletion lost undo history".into());
                    }
                    capture.invoke_action("undo".into());
                    if capture.get_object_count() != 1 {
                        return Err("Text deletion undo failed".into());
                    }
                    capture.set_tool(0);
                    capture.invoke_pointer(0, 220.0, 250.0);
                    capture.invoke_pointer(2, 220.0, 250.0);
                    capture.invoke_action("edit-object-text".into());
                    if capture.get_text_value() != "Glint 拾光 & <text>"
                        || capture.get_text_size() != 36
                    {
                        return Err("Re-edit did not prefill original text metadata".into());
                    }
                    capture.set_text_value("cancelled edit".into());
                    capture.invoke_action("cancel-text".into());
                    capture.invoke_action("edit-object-text".into());
                    if capture.get_text_value() != "Glint 拾光 & <text>" {
                        return Err("Cancelling re-edit changed original text".into());
                    }
                    capture.set_text_value("Glint edited 拾光 & <text>".into());
                    capture.set_text_font("Glint Missing Font 914".into());
                    capture.invoke_action("commit-text".into());
                    self.child_started = std::time::Instant::now();
                }
            }
            28 => {
                let app = self.app.borrow();
                let capture = app.capture.as_ref().ok_or("Re-edit closed capture")?;
                if capture.get_text_busy() {
                    if self.child_started.elapsed() > Duration::from_secs(10) {
                        return Err("Re-edit failure timed out".into());
                    }
                    return Ok(false);
                }
                if capture.get_object_count() != 1
                    || !capture.get_text_editing()
                    || !capture.get_text_feedback().contains("Text failed")
                {
                    return Err("Re-edit failure lost original or input".into());
                }
                capture_snapshot(capture, &self.directory.join("text-reedit-failed.png"))?;
                capture.set_text_font("Segoe UI".into());
                capture.set_text_size(24);
                capture.set_mark_color(slint::Color::from_rgb_u8(75, 92, 250));
                capture.invoke_action("commit-text".into());
                self.child_started = std::time::Instant::now();
            }
            29 => {
                let app = self.app.borrow();
                let capture = app.capture.as_ref().ok_or("Re-edit retry closed capture")?;
                if capture.get_text_busy() {
                    if self.child_started.elapsed() > Duration::from_secs(10) {
                        return Err("Re-edit retry timed out".into());
                    }
                    return Ok(false);
                }
                if capture.get_object_count() != 1 || capture.get_text_editing() {
                    return Err("Re-edit retry did not replace one object".into());
                }
                capture.invoke_action("edit-object-text".into());
                if capture.get_text_value() != "Glint edited 拾光 & <text>"
                    || capture.get_text_size() != 24
                {
                    return Err("Re-edit metadata was not saved".into());
                }
                capture.invoke_action("cancel-text".into());
                capture.invoke_action("undo".into());
                capture.invoke_pointer(0, 220.0, 250.0);
                capture.invoke_pointer(2, 220.0, 250.0);
                capture.invoke_action("edit-object-text".into());
                if capture.get_text_value() != "Glint 拾光 & <text>"
                    || capture.get_text_size() != 36
                {
                    return Err("Re-edit was not exactly one undo".into());
                }
                capture.invoke_action("cancel-text".into());
                capture.invoke_action("redo".into());
                capture.set_tool(8);
                capture.invoke_pointer(0, 200.0, 300.0);
                capture.set_text_value("discard".into());
                capture.invoke_action("commit-text".into());
                use slint::platform::{Key, WindowEvent};
                capture.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Escape.into(),
                });
                capture.window().dispatch_event(WindowEvent::KeyReleased {
                    text: Key::Escape.into(),
                });
                if capture.get_text_editing()
                    || capture.get_object_count() != 1
                    || !capture.window().is_visible()
                {
                    return Err("Escape failed to cancel text while keeping capture".into());
                }
                capture_snapshot(capture, &self.directory.join("text-overlay.png"))?;
                std::fs::rename(
                    self.directory.join("resident-capture.png"),
                    self.directory.join("shapes-capture.png"),
                )
                .map_err(|e| e.to_string())?;
                capture.invoke_action("save".into());
                self.child_started = std::time::Instant::now();
            }
            30 => {
                if self.app.borrow().capture.is_some() {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Text export did not finish".into());
                    }
                    return Ok(false);
                }
                let output = image::open(self.directory.join("resident-capture.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                let preview = image::open(self.directory.join("text-overlay.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if output.dimensions() != (800, 500) {
                    return Err("Text PNG crop dimensions differ".into());
                }
                for y in 4..496 {
                    for x in 4..796 {
                        for (&a, &b) in output
                            .get_pixel(x, y)
                            .0
                            .iter()
                            .zip(preview.get_pixel(x + 100, y + 100).0.iter())
                        {
                            if a.abs_diff(b) > 1 {
                                return Err("Native text preview differs from flattened PNG".into());
                            }
                        }
                    }
                }
            }
            31 => {
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                for (key, value) in [
                    ("output.finish", "save_as"),
                    ("output.format", "jpeg"),
                    ("output.quality", "85"),
                    ("output.filename", "Export_{width}_{height}_{time}"),
                ] {
                    ui.invoke_change(key.into(), value.into());
                }
                if self.app.borrow().session.preferences.saved().output.finish
                    == glint_core::config::FinishAction::SaveAs
                {
                    return Err("Output draft leaked before save".into());
                }
                snapshot(&ui, &self.directory.join("output-settings.png"))?;
                ui.invoke_action("save".into());
                let persisted = glint_core::config::Config::from_toml(
                    &std::fs::read_to_string(self.directory.join("config.toml"))
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if persisted.output.finish != glint_core::config::FinishAction::SaveAs
                    || persisted.output.quality != 85
                    || persisted.output.format != glint_core::config::ImageFormat::Jpeg
                {
                    return Err("Output settings did not persist".into());
                }
                self.app.borrow_mut().capture_output =
                    Some(self.directory.join("resident-output.jpg"));
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            32 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("JPEG capture did not open".into());
                    }
                    return Ok(false);
                };
                capture.invoke_pointer(0, 100.0, 100.0);
                capture.invoke_pointer(2, 500.0, 400.0);
                capture.set_tool(1);
                capture.invoke_pointer(0, 150.0, 150.0);
                capture.invoke_pointer(2, 300.0, 300.0);
                capture.invoke_action("unknown-output".into());
                if capture.get_busy() {
                    return Err("Unknown action started an export".into());
                }
                use slint::platform::{Key, WindowEvent};
                capture.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Return.into(),
                });
                capture.window().dispatch_event(WindowEvent::KeyReleased {
                    text: Key::Return.into(),
                });
                self.child_started = std::time::Instant::now();
            }
            33 => {
                if self.app.borrow().capture.is_some() {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Default Enter JPEG export did not finish".into());
                    }
                    return Ok(false);
                }
                let output = image::open(self.directory.join("resident-output.jpg"))
                    .map_err(|e| e.to_string())?
                    .to_rgb8();
                if output.dimensions() != (400, 300)
                    || output
                        .get_pixel(100, 100)
                        .0
                        .iter()
                        .zip([32u8, 43, 66])
                        .any(|(a, b)| a.abs_diff(b) > 4)
                {
                    return Err("JPEG dimensions or flattened redaction differ".into());
                }
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            34 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Explicit copy capture did not open".into());
                    }
                    return Ok(false);
                };
                capture.invoke_pointer(0, 100.0, 100.0);
                capture.invoke_pointer(2, 500.0, 400.0);
                capture.invoke_action("copy".into());
                self.child_started = std::time::Instant::now();
            }
            35 => {
                if let Some(capture) = &self.app.borrow().capture {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err(format!(
                            "Explicit copy used SaveAs or failed: {}",
                            capture.get_status()
                        ));
                    }
                    return Ok(false);
                }
            }
            36 => {
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                let directory = self.directory.join("automatic");
                ui.invoke_change(
                    "output.directory".into(),
                    directory.to_string_lossy().into_owned().into(),
                );
                ui.invoke_change("output.filename".into(), "Auto_{width}x{height}".into());
                ui.invoke_change("output.auto_save".into(), "true".into());
                ui.invoke_action("save".into());
                if !self
                    .app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .output
                    .auto_save
                {
                    return Err("Automatic output option did not save".into());
                }
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            37 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Automatic output capture did not open".into());
                    }
                    return Ok(false);
                };
                capture.invoke_pointer(0, 100.0, 100.0);
                capture.invoke_pointer(2, 500.0, 400.0);
                capture.set_tool(1);
                capture.invoke_pointer(0, 150.0, 150.0);
                capture.invoke_pointer(2, 300.0, 300.0);
                self.clipboard_blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                capture.invoke_action("copy".into());
                self.child_started = std::time::Instant::now();
            }
            38 => {
                let app = self.app.borrow();
                let capture = app
                    .capture
                    .as_ref()
                    .ok_or("Partial output failure closed editor")?;
                if capture.get_busy() {
                    if self.child_started.elapsed() > Duration::from_secs(15) {
                        return Err("Automatic output/occupied clipboard timed out".into());
                    }
                    return Ok(false);
                }
                if !capture.get_status().contains("Files already saved")
                    || !capture.get_status().contains("Auto_400x300.jpg")
                    || capture.get_object_count() != 1
                    || !capture.get_selected()
                {
                    return Err(format!(
                        "Partial output receipt or edits missing: {}",
                        capture.get_status()
                    ));
                }
                let automatic = self.directory.join("automatic/Auto_400x300.jpg");
                let pixels = image::open(&automatic)
                    .map_err(|e| e.to_string())?
                    .to_rgb8();
                if pixels.dimensions() != (400, 300)
                    || pixels
                        .get_pixel(100, 100)
                        .0
                        .iter()
                        .zip([32u8, 43, 66])
                        .any(|(a, b)| a.abs_diff(b) > 4)
                {
                    return Err("Automatic JPEG did not flatten redaction".into());
                }
                capture_snapshot(
                    capture,
                    &self.directory.join("automatic-partial-failure.png"),
                )?;
                self.clipboard_blocker.take();
                capture.invoke_action("copy".into());
                self.child_started = std::time::Instant::now();
            }
            39 => {
                if let Some(capture) = &self.app.borrow().capture {
                    if self.child_started.elapsed() > Duration::from_secs(15) {
                        return Err(format!(
                            "Partial output retry failed: {}",
                            capture.get_status()
                        ));
                    }
                    return Ok(false);
                }
                if std::fs::read_dir(self.directory.join("automatic"))
                    .map_err(|e| e.to_string())?
                    .count()
                    != 1
                {
                    return Err("Unchanged retry duplicated automatic output".into());
                }
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            40 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Automatic collision capture did not open".into());
                    }
                    return Ok(false);
                };
                capture.invoke_pointer(0, 100.0, 100.0);
                capture.invoke_pointer(2, 500.0, 400.0);
                capture.invoke_action("copy".into());
                self.child_started = std::time::Instant::now();
            }
            41 => {
                if let Some(capture) = &self.app.borrow().capture {
                    if self.child_started.elapsed() > Duration::from_secs(15) {
                        return Err(format!(
                            "Automatic collision completion failed: {}",
                            capture.get_status()
                        ));
                    }
                    return Ok(false);
                }
                if !self.directory.join("automatic/Auto_400x300_1.jpg").exists()
                    || std::fs::read_dir(self.directory.join("automatic"))
                        .map_err(|e| e.to_string())?
                        .count()
                        != 2
                {
                    return Err("New capture did not number automatic output collision".into());
                }
            }
            42 => {
                ui.invoke_action("open".into());
                let blocked = self.directory.join("automatic-directory-blocker");
                std::fs::write(&blocked, b"preserve").map_err(|e| e.to_string())?;
                ui.invoke_change(
                    "output.directory".into(),
                    blocked.to_string_lossy().into_owned().into(),
                );
                ui.invoke_action("save".into());
                self.app.borrow_mut().capture_output =
                    Some(self.directory.join("recovered-output.jpg"));
                ui.invoke_action("capture".into());
                self.child_started = std::time::Instant::now();
            }
            43 => {
                let app = self.app.borrow();
                let Some(capture) = &app.capture else {
                    if self.child_started.elapsed() > Duration::from_secs(5) {
                        return Err("Directory failure capture did not open".into());
                    }
                    return Ok(false);
                };
                capture.invoke_pointer(0, 100.0, 100.0);
                capture.invoke_pointer(2, 500.0, 400.0);
                capture.invoke_action("copy".into());
                self.child_started = std::time::Instant::now();
            }
            44 => {
                let app = self.app.borrow();
                let capture = app
                    .capture
                    .as_ref()
                    .ok_or("Automatic directory failure closed editor")?;
                if capture.get_busy() {
                    if self.child_started.elapsed() > Duration::from_secs(10) {
                        return Err("Automatic directory failure timed out".into());
                    }
                    return Ok(false);
                }
                if !capture.get_auto_save_failed()
                    || !capture.get_selected()
                    || !capture.get_status().contains("Output failed")
                {
                    return Err("Automatic directory failure did not expose alternate save".into());
                }
                capture_snapshot(
                    capture,
                    &self.directory.join("automatic-directory-failed.png"),
                )?;
                capture.invoke_action("save-without-auto".into());
                self.child_started = std::time::Instant::now();
            }
            45 => {
                if let Some(capture) = &self.app.borrow().capture {
                    if self.child_started.elapsed() > Duration::from_secs(10) {
                        return Err(format!(
                            "Explicit alternate save failed: {}",
                            capture.get_status()
                        ));
                    }
                    return Ok(false);
                }
                if image::open(self.directory.join("recovered-output.jpg"))
                    .map_err(|e| e.to_string())?
                    .width()
                    != 400
                    || !self
                        .app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .output
                        .auto_save
                    || std::fs::read(self.directory.join("automatic-directory-blocker"))
                        .map_err(|e| e.to_string())?
                        != b"preserve"
                {
                    return Err(
                        "Alternate save changed config or failed to preserve blocker".into(),
                    );
                }
            }
            46 => {
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(700., 400.),
                        delta_x: 0.,
                        delta_y: 10_000.,
                    });
            }
            47 => {
                snapshot(
                    &ui,
                    &self.directory.join("automatic-settings-before-click.png"),
                )?;
                let position = slint::LogicalPosition::new(826., 362.);
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerMoved { position });
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                        position,
                        button: slint::platform::PointerEventButton::Left,
                    });
            }
            48 => {
                let position = slint::LogicalPosition::new(826., 362.);
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                        position,
                        button: slint::platform::PointerEventButton::Left,
                    });
            }
            49 => {
                if self.app.borrow().session.preferences.draft.output.auto_save
                    || !self
                        .app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .output
                        .auto_save
                {
                    return Err(
                        "Automatic output checkbox pointer did not change only the draft".into(),
                    );
                }
                snapshot(
                    &ui,
                    &self.directory.join("automatic-settings-unchecked.png"),
                )?;
                ui.invoke_action("cancel".into());
            }
            50 => {
                if !self.app.borrow().session.preferences.draft.output.auto_save {
                    return Err("Cancel did not restore automatic output checkbox".into());
                }
                snapshot(&ui, &self.directory.join("automatic-settings-restored.png"))?;
            }
            51..=80 => {
                if !self
                    .pin_smoke
                    .step((self.phase - 51) as u8, &ui, &self.app, &self.directory)?
                {
                    return Ok(false);
                }
            }
            81..=93 => {
                if !self.pass_smoke.step(
                    (self.phase - 81) as u8,
                    &ui,
                    &self.app,
                    &self.hotkeys,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            94..=104 => {
                if !self.rotation_smoke.step(
                    (self.phase - 94) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            105..=124 => {
                if !self.edit_smoke.step(
                    (self.phase - 105) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            125..=130 => {
                if !self.wheel_smoke.step(
                    (self.phase - 125) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            131..=140 => {
                if !self.geometry_smoke.step(
                    (self.phase - 131) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            141..=155 => {
                if !self.repeat_smoke.step(
                    (self.phase - 141) as u8,
                    &ui,
                    &self.app,
                    &self.hotkeys,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            156..=173 => {
                if !self.delay_smoke.step(
                    (self.phase - 156) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            174..=180 => {
                if !self.cursor_smoke.step(
                    (self.phase - 174) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            181..=185 => {
                if !self.magnifier_smoke.step(
                    (self.phase - 181) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            186..=192 => {
                if !self.screens_smoke.step(
                    (self.phase - 186) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            193..=205 => {
                if !self.color_picker_smoke.step(
                    (self.phase - 193) as u8,
                    &ui,
                    &self.app,
                    &self.hotkeys,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            206..=212 => {
                if !self.guide_smoke.step(
                    (self.phase - 206) as u8,
                    &ui,
                    &self.app,
                    &self.hotkeys,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            213..=223 => {
                if !self.config_smoke.step(
                    (self.phase - 213) as u8,
                    &ui,
                    &self.app,
                    &self.hotkeys,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            224..=234 => {
                if !self.local_smoke.step(
                    (self.phase - 224) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            235..=243 => {
                if !self.annotation_defaults.step(
                    (self.phase - 235) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            244..=254 => {
                if !self.completion.step(
                    (self.phase - 244) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            255..=264 => {
                if !self
                    .scale
                    .step((self.phase - 255) as u8, &ui, &self.app, &self.directory)?
                {
                    return Ok(false);
                }
            }
            265..=274 if self.focused != 255 => {
                if !self
                    .motion
                    .step((self.phase - 265) as u8, &ui, &self.app, &self.directory)?
                {
                    return Ok(false);
                }
            }
            275..=277 if self.focused == 0 || self.focused == 275 => {
                if !self.preview_budget.step(
                    (self.phase - 275) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            278..=284 if self.focused == 0 || self.focused == 278 => {
                if !self.lifecycle.step(
                    (self.phase - 278) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            285..=290 if self.focused == 0 || self.focused == 285 => {
                if !self.diagnostics.step(
                    (self.phase - 285) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            291..=302 if self.focused == 0 || self.focused == 291 => {
                if !self
                    .fonts
                    .step((self.phase - 291) as u8, &ui, &self.app, &self.directory)?
                {
                    return Ok(false);
                }
            }
            303..=308 if self.focused == 0 || self.focused == 303 => {
                if !self.output_settings.step(
                    (self.phase - 303) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            309..=316 if self.focused == 0 || self.focused == 309 => {
                if !self.output_conflict.step(
                    (self.phase - 309) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            317..=320 if self.focused == 0 || self.focused == 317 => {
                self.config_dialog.step(
                    (self.phase - 317) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )?;
            }
            321..=328 if self.focused == 0 || self.focused == 321 => {
                if !self.local_dialog.step(
                    (self.phase - 321) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            329..=343 if self.focused == 0 || self.focused == 329 => {
                if !self
                    .log
                    .step((self.phase - 329) as u8, &ui, &self.app, &self.directory)?
                {
                    return Ok(false);
                }
            }
            344..=351 if self.focused == 0 || self.focused == 344 => {
                if !self.exclusions.step(
                    (self.phase - 344) as u8,
                    &ui,
                    &self.app,
                    &self.directory,
                )? {
                    return Ok(false);
                }
            }
            352 if self.focused == 352 => {
                ui.invoke_choose_page(1);
                ui.invoke_change("general.language".into(), "zh_cn".into());
                ui.window().set_size(slint::PhysicalSize::new(900, 700));
                ui.invoke_record("shortcuts.window".into());
            }
            353 if self.focused == 352 => {
                snapshot(&ui, &self.directory.join("shortcut-inline-zh.png"))?;
                if ui.get_recording_key() != "shortcuts.window"
                    || !ui.get_shortcut_recorder_focused()
                {
                    return Err("Inline shortcut did not focus the selected row".into());
                }
                ui.invoke_shortcut_input("s".into(), true, false, true, false);
                if ui.get_can_accept() || !ui.get_shortcut_invalid() {
                    return Err("Inline duplicate binding was not rejected".into());
                }
                snapshot(&ui, &self.directory.join("shortcut-inline-conflict.png"))?;
                ui.invoke_shortcut_input("\u{f717}".into(), true, true, false, false);
                settings_key(&ui, "\u{9}");
            }
            354 if self.focused == 352 => {
                if !ui.get_modal_cancel_focused() {
                    return Err("Inline Tab missed Cancel".into());
                }
                settings_key(&ui, "\u{9}");
            }
            355 if self.focused == 352 => {
                if !ui.get_shortcut_clear_focused() {
                    return Err("Inline Tab missed Clear".into());
                }
                settings_key(&ui, "\u{9}");
            }
            356 if self.focused == 352 => {
                if !ui.get_shortcut_accept_focused() {
                    return Err("Inline Tab missed Confirm".into());
                }
                settings_key(&ui, "\u{9}");
            }
            357 if self.focused == 352 => {
                if !ui.get_shortcut_recorder_focused() {
                    return Err("Inline Tab did not return to recording".into());
                }
                settings_key(&ui, "\n");
            }
            358 if self.focused == 352 => {
                if !ui.get_recording_key().is_empty()
                    || self.app.borrow().session.preferences.draft.shortcuts.window
                        != "Ctrl+Alt+F20"
                    || self
                        .app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .shortcuts
                        .window
                        == "Ctrl+Alt+F20"
                {
                    return Err("Inline Enter did not confirm only the settings draft".into());
                }
                ui.invoke_record("shortcuts.window".into());
            }
            359 if self.focused == 352 => {
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                        text: "\u{1b}".into(),
                    });
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                        text: "\u{1b}".into(),
                    });
                if !ui.get_recording_key().is_empty()
                    || self.app.borrow().session.preferences.draft.shortcuts.window
                        != "Ctrl+Alt+F20"
                {
                    return Err("Inline Escape did not preserve the previous binding".into());
                }
                ui.invoke_record("shortcuts.window".into());
                ui.invoke_action("clear-key".into());
                if !self
                    .app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .shortcuts
                    .window
                    .is_empty()
                {
                    return Err("Inline optional shortcut could not be cleared".into());
                }
                ui.invoke_record("shortcuts.rescue".into());
                ui.invoke_action("clear-key".into());
                if ui.get_can_clear() || ui.get_recording_key() != "shortcuts.rescue" {
                    return Err("Inline rescue binding lost its clear protection".into());
                }
                ui.invoke_action("continue".into());
                ui.invoke_action("cancel".into());
                ui.set_guide_step(1);
                self.app.borrow().sync(&ui);
                ui.invoke_record("shortcuts.capture".into());
            }
            360 if self.focused == 352 => {
                snapshot(&ui, &self.directory.join("shortcut-inline-guide.png"))?;
                if !ui.get_shortcut_recorder_focused() {
                    return Err("Guide inline recorder lost focus".into());
                }
                ui.invoke_action("continue".into());
                ui.set_guide_step(-1);
                ui.invoke_change("general.language".into(), "en".into());
                ui.window().set_size(slint::PhysicalSize::new(720, 650));
                ui.invoke_record("shortcuts.capture".into());
            }
            361 if self.focused == 352 => {
                snapshot(&ui, &self.directory.join("shortcut-inline-en-compact.png"))?;
                ui.invoke_action("continue".into());
                ui.invoke_action("cancel".into());
                std::fs::write(self.directory.join("SHORTCUT-INLINE-PASSED.txt"),
                    "Native Slint inline row rendering, initial focus, Tab through Cancel/Clear/Confirm/recorder, duplicate rejection, Enter confirmation, Escape cancellation, optional clear, rescue protection, guide and bilingual compact snapshots passed. Not a physical keyboard or system-conflict acceptance test.\n")
                    .map_err(|e| e.to_string())?;
                ui.invoke_action("quit".into());
                return Ok(true);
            }
            _ => {
                if self.focused > 0 {
                    eprintln!(
                        "Native focused presentation probe passed; earlier resident stages were not run"
                    );
                    ui.invoke_action("quit".into());
                    return Ok(true);
                }

                eprintln!(
                    "Native resident: settings/rollback, real hotkey registration/release, cross-process activation, frozen window hover/click and explicit entry, synthetic initial-window/failure fallback, desktop capture/redaction/undo/PNG, output conflict retention, frozen-source picker HEX/RGB clipboard and occupied retry, text failure/correction/cancel/move/delete/re-edit/PNG, synthetic object move/delete/style/resize/endpoints/history/dependent pixelation preview and budget-rejection recovery, toolbar pointer drag/right-click/draft/apply/cancel/reset/failure retry/More/layout, shared settings toolbar editor/draft/locked-file retry/scroll/900px layout, cancellation and return to tray passed"
                );
                ui.invoke_action("quit".into());
                return Ok(true);
            }
        }
        self.phase += 1;
        Ok(false)
    }
}
fn capture_snapshot(ui: &crate::probe::CaptureProbe, path: &std::path::Path) -> Result<(), String> {
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
fn verify(
    ui: &SettingsWindow,
    state: &Rc<RefCell<AppState>>,
    hotkeys: &glint_platform::HotkeyService,
    directory: &std::path::Path,
) -> Result<(), String> {
    ui.invoke_action("cancel".into());
    if state.borrow().session.preferences.is_dirty() {
        return Err("Cancel did not restore saved preferences".into());
    }
    ui.invoke_record("shortcuts.settings".into());
    ui.invoke_shortcut_input("s".into(), true, false, true, false);
    if ui.get_can_accept() {
        return Err("Duplicate shortcut was accepted by the native recorder".into());
    }
    ui.invoke_shortcut_input("\u{f718}".into(), true, true, true, false);
    if !state
        .borrow()
        .session
        .preferences
        .draft
        .shortcuts
        .settings
        .is_empty()
    {
        return Err("Recording changed the draft before confirmation".into());
    }
    ui.invoke_action("accept-key".into());
    if state.borrow().session.preferences.draft.shortcuts.settings != "Ctrl+Alt+Shift+F21" {
        return Err("Native shortcut recording failed".into());
    }
    ui.invoke_action("save".into());
    if state.borrow().session.preferences.is_dirty() {
        return Err(format!("Save failed: {}", ui.get_status()));
    }
    let binding = crate::resident::settings_binding(state.borrow().session.preferences.saved())?;
    if glint_platform::HotkeyService::new(binding, |_| {}).is_ok() {
        return Err("Saved shortcut is not actually registered".into());
    }
    ui.invoke_record("shortcuts.settings".into());
    ui.invoke_shortcut_input("\u{f719}".into(), true, true, true, false);
    ui.invoke_action("accept-key".into());
    use std::os::windows::fs::OpenOptionsExt;
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(directory.join("config.toml"))
        .map_err(|e| e.to_string())?;
    ui.invoke_action("save".into());
    if !state.borrow().session.preferences.is_dirty()
        || state
            .borrow()
            .session
            .preferences
            .saved()
            .shortcuts
            .settings
            != "Ctrl+Alt+Shift+F21"
    {
        return Err("Failed file save changed the accepted settings snapshot".into());
    }
    if glint_platform::HotkeyService::new(
        crate::resident::settings_binding(state.borrow().session.preferences.saved())?,
        |_| {},
    )
    .is_ok()
    {
        return Err("Failed save did not restore the actual old hotkey".into());
    }
    let released = glint_platform::HotkeyService::new(
        crate::resident::settings_binding(&state.borrow().session.preferences.draft)?,
        |_| {},
    )?;
    drop(released);
    drop(locked);
    ui.invoke_action("save".into());
    if state.borrow().session.preferences.is_dirty()
        || state
            .borrow()
            .session
            .preferences
            .saved()
            .shortcuts
            .settings
            != "Ctrl+Alt+Shift+F22"
    {
        return Err("Retry after releasing the configuration file failed".into());
    }
    ui.invoke_action("reset".into());
    if ui.get_modal() != 2 {
        return Err("Reset did not ask for confirmation".into());
    }
    ui.invoke_action("confirm-reset".into());
    if !state.borrow().session.preferences.is_dirty() {
        return Err("Reset changed the saved snapshot prematurely".into());
    }
    ui.invoke_action("cancel".into());
    ui.invoke_record("shortcuts.settings".into());
    ui.invoke_action("clear-key".into());
    ui.invoke_action("save".into());
    if state.borrow().session.preferences.is_dirty() {
        return Err("Clearing shortcut failed to save".into());
    }
    let test = glint_platform::HotkeyService::new(
        vec![glint_platform::HotkeyBinding {
            action: 8,
            modifiers: 7,
            key: 0x85,
        }],
        |_| {},
    )?;
    drop(test);
    hotkeys.replace(vec![])?;
    let saved = glint_core::config::Config::from_toml(
        &std::fs::read_to_string(directory.join("config.toml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if saved != *state.borrow().session.preferences.saved() {
        return Err("Persisted TOML differs from saved snapshot".into());
    }
    Ok(())
}
fn snapshot(ui: &SettingsWindow, path: &std::path::Path) -> Result<(), String> {
    let image = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    image::save_buffer_with_format(
        path,
        image.as_bytes(),
        image.width(),
        image.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())
}

fn settings_key(ui: &SettingsWindow, key: &str) {
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.into() });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key.into() });
}
