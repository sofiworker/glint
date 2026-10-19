//! Resident pin integration, using actual desktop frames and native window state.
use crate::{
    pins::{self, DesktopPin},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Run {
    first: u64,
    second: u64,
    used: u64,
    started: Option<Instant>,
    clipboard_blocker: Option<glint_platform::ClipboardBlocker>,
    copy_started: bool,
    finish_keys_sent: bool,
}
fn shot(ui: &DesktopPin, path: &Path) -> Result<(), String> {
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
fn click(ui: &DesktopPin, x: f32, y: f32, button: slint::platform::PointerEventButton) {
    use slint::platform::WindowEvent;
    let position = slint::LogicalPosition::new(x, y);
    ui.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    ui.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
}
impl Run {
    fn waiting(&self, reason: &str) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|s| s.elapsed() > Duration::from_secs(15))
        {
            Err(reason.into())
        } else {
            Ok(false)
        }
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let manager = app.borrow().pins.clone();
        match stage {
            0 => {
                ui.invoke_action("open".into());
                ui.invoke_record("shortcuts.rescue".into());
                if ui.get_modal() != 3 || ui.get_can_clear() {
                    return Err("Rescue shortcut recorder is missing or allows clearing".into());
                }
                ui.invoke_action("clear-key".into());
                if ui.get_modal() != 3
                    || app
                        .borrow()
                        .session
                        .preferences
                        .draft
                        .shortcuts
                        .rescue
                        .is_empty()
                {
                    return Err("Mandatory rescue binding was cleared".into());
                }
                ui.invoke_action("continue".into());
                ui.invoke_record("shortcuts.toggle_pins".into());
                ui.invoke_action("clear-key".into());
                if !app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .shortcuts
                    .toggle_pins
                    .is_empty()
                {
                    return Err("Optional pin toggle shortcut did not clear".into());
                }
                ui.invoke_action("cancel".into());
                ui.invoke_record("shortcuts.pin_clipboard".into());
                if ui.get_modal() != 3 || !ui.get_can_clear() {
                    return Err("Clipboard pin shortcut recorder unavailable".into());
                }
                ui.invoke_action("clear-key".into());
                if !app
                    .borrow()
                    .session
                    .preferences
                    .draft
                    .shortcuts
                    .pin_clipboard
                    .is_empty()
                {
                    return Err("Clipboard pin optional shortcut did not clear".into());
                }
                ui.invoke_action("cancel".into());
                for (key, value) in [
                    ("output.auto_save", "false"),
                    ("output.finish", "copy_and_pin"),
                    ("pins.opacity", "75"),
                    ("pins.on_top", "true"),
                    ("pins.zoom_percent", "100"),
                    ("resources.pin_budget_mib", "64"),
                ] {
                    ui.invoke_change(key.into(), value.into());
                }
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.saved().output.finish
                    != glint_core::config::FinishAction::CopyAndPin
                {
                    return Err("Copy-and-pin option did not save".into());
                }
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            1 => {
                let state = app.borrow();
                let Some(capture) = &state.capture else {
                    return self.waiting("Pin capture did not open");
                };
                capture.invoke_pointer(0, 100., 100.);
                capture.invoke_pointer(2, 500., 400.);
                capture.set_tool(1);
                capture.invoke_pointer(0, 150., 150.);
                capture.invoke_pointer(2, 300., 300.);
                use slint::platform::{Key, WindowEvent};
                capture.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Return.into(),
                });
                capture.window().dispatch_event(WindowEvent::KeyReleased {
                    text: Key::Return.into(),
                });
                self.started = Some(Instant::now());
            }
            2 => {
                if app.borrow().capture.is_some() {
                    return self.waiting("Copy-and-pin did not finish capture");
                }
                if manager.borrow().count() != 1 {
                    return Err("Copy-and-pin did not create exactly one resident pin".into());
                }
                self.first = manager.borrow().ids()[0];
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                let document = manager
                    .borrow()
                    .document(self.first)
                    .ok_or("Screenshot pin lost editable document")?;
                if document.dimensions() != (400, 300)
                    || document.edits().marks().len() != 1
                    || !document.edits().can_undo()
                {
                    return Err("Screenshot pin lost cropped source, object or undo history".into());
                }
                let source_len = document.source().len();
                if source_len != 400 * 300 * 4 || manager.borrow().used() <= source_len as u64 * 3 {
                    return Err(
                        "Pin raw source was not cropped or retained data was not charged".into(),
                    );
                }
                let composite = glint_core::composite_selection(
                    document.source(),
                    400,
                    300,
                    glint_core::Rect {
                        x: 0,
                        y: 0,
                        width: 400,
                        height: 300,
                    },
                    document.edits(),
                )
                .map_err(str::to_owned)?;
                if composite
                    != pin
                        .get_picture()
                        .to_rgba8()
                        .ok_or("Missing pin pixels")?
                        .as_bytes()
                {
                    return Err("Editable pin document does not reproduce exported pixels".into());
                }
                let native = pins::native(&pin, |w| w.state())?;
                if !native.visible
                    || !native.topmost
                    || !native.no_activate
                    || native.click_through
                    || pins::native(&pin, |w| Ok(w.is_foreground()))?
                {
                    return Err(format!("Resident pin native state is wrong: {native:?}"));
                }
                let pixels = pin.get_picture().to_rgba8().ok_or("Missing pin bitmap")?;
                if (pixels.width(), pixels.height()) != (400, 300)
                    || pixels.as_bytes()[(100 * 400 + 100) * 4..(100 * 400 + 100) * 4 + 4]
                        != [32, 43, 66, 255]
                {
                    return Err("Resident pin did not use flattened capture pixels".into());
                }
                if (pin.get_alpha() - 0.75).abs() > 0.001 {
                    return Err("Saved pin opacity did not apply".into());
                }
                self.used = manager.borrow().used();
                let copied = glint_platform::read_clipboard_image()?;
                if (copied.width, copied.height) != (400, 300)
                    || copied.rgba[(100 * 400 + 100) * 4..(100 * 400 + 100) * 4 + 4]
                        != [32, 43, 66, 255]
                {
                    return Err("Copy-and-pin did not write matching clipboard pixels".into());
                }
                shot(&pin, &directory.join("resident-pin-first.png"))?;
                click(&pin, 342., 17., slint::platform::PointerEventButton::Left);
                if !pin.get_menu_open() {
                    return Err("Pin header menu button did not receive pointer input".into());
                }
                click(&pin, 342., 17., slint::platform::PointerEventButton::Left);
                click(&pin, 80., 80., slint::platform::PointerEventButton::Right);
                if !pin.get_menu_open() {
                    return Err("Pin image right-click did not open menu".into());
                }
                pin.invoke_action("menu".into());
                glint_platform::copy_text("glint-pin-only-marker".into())?;
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            3 => {
                let state = app.borrow();
                let Some(capture) = &state.capture else {
                    return self.waiting("Second pin capture did not open");
                };
                if manager
                    .borrow()
                    .window(self.first)
                    .unwrap()
                    .window()
                    .is_visible()
                {
                    return Err("Existing pin was not hidden during capture".into());
                }
                capture.invoke_pointer(0, 120., 120.);
                capture.invoke_pointer(2, 360., 280.);
                capture.invoke_action("pin".into());
                self.started = Some(Instant::now());
            }
            4 => {
                if app.borrow().capture.is_some() {
                    return self.waiting("Explicit pin did not finish capture");
                }
                if manager.borrow().count() != 2
                    || glint_platform::read_clipboard_text()? != "glint-pin-only-marker"
                {
                    return Err("Explicit pin replaced another pin or copied to clipboard".into());
                }
                self.second = manager
                    .borrow()
                    .ids()
                    .into_iter()
                    .find(|id| *id != self.first)
                    .unwrap();
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if !pin.window().is_visible() {
                    return Err("First pin did not restore after capture".into());
                }
                self.used = manager.borrow().used();
                pin.invoke_change_opacity(55.);
                pin.invoke_action("lock".into());
                pin.invoke_action("top".into());
                if !pin.get_locked()
                    || pin.get_on_top()
                    || pins::native(&pin, |w| w.state())?.topmost
                    || (pin.get_alpha() - 0.55).abs() > 0.001
                {
                    return Err("Pin lock/top/opacity did not apply".into());
                }
                pin.invoke_action("smaller".into());
                if manager.borrow().used() >= self.used {
                    return Err("Pin shrink did not reduce reservation".into());
                }
            }
            5 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                shot(&pin, &directory.join("resident-pin-scaled.png"))?;
                pin.invoke_action("larger".into());
                ui.invoke_action("toggle-pins".into());
            }
            6 => {
                if manager
                    .borrow()
                    .ids()
                    .iter()
                    .any(|id| manager.borrow().window(*id).unwrap().window().is_visible())
                {
                    return Err("Hide-all left a resident pin visible".into());
                }
                ui.invoke_action("toggle-pins".into());
            }
            7 => {
                if manager
                    .borrow()
                    .ids()
                    .iter()
                    .any(|id| !manager.borrow().window(*id).unwrap().window().is_visible())
                {
                    return Err("Restore-all left a resident pin hidden".into());
                }
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                pin.window()
                    .set_position(slint::PhysicalPosition::new(-6000, -6000));
                pin.invoke_action("hide".into());
                ui.invoke_action("rescue-pins".into());
            }
            8 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                let native = pins::native(&pin, |w| w.state())?;
                // This fixture runs on the recorded single monitor at physical origin (0,0).
                if !native.visible
                    || !native.no_activate
                    || !native.topmost
                    || native.x < 0
                    || native.y < 0
                {
                    return Err(format!(
                        "Pin rescue did not recover native window: {native:?}"
                    ));
                }
                pin.invoke_action("menu".into());
                shot(&pin, &directory.join("resident-pin-menu.png"))?;
                pin.invoke_action("close".into());
                if manager.borrow().count() != 1 || manager.borrow().used() >= self.used {
                    return Err("Closing pin did not release its reservation".into());
                }
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            9 => {
                let state = app.borrow();
                let Some(capture) = &state.capture else {
                    return self.waiting("Budget failure capture did not open");
                };
                capture.invoke_pointer(0, 100., 100.);
                capture.invoke_pointer(2, 500., 400.);
                let before = manager.borrow().used();
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(4096, 2160);
                capture.invoke_request_pin(slint::Image::from_rgba8(pixels.clone()), 100, 100);
                if manager.borrow().count() != 1
                    || manager.borrow().used() != before
                    || !capture.get_selected()
                    || !capture.get_status().contains("Pin document missing")
                {
                    return Err(format!(
                        "Missing document rejection lost editor or existing pin: {}",
                        capture.get_status()
                    ));
                }
                let document = glint_core::ImageDocument::from_selection(
                    pixels.as_bytes(),
                    4096,
                    2160,
                    glint_core::Rect {
                        x: 0,
                        y: 0,
                        width: 4096,
                        height: 2160,
                    },
                    &glint_core::Edits::default(),
                )
                .map_err(str::to_owned)?;
                let rejected = pins::Manager::create_document(
                    &manager,
                    crate::probe::PinPayload {
                        document: std::sync::Arc::new(document),
                        display: pixels.clone(),
                        turns: 0,
                        pixels,
                    },
                    (100, 100),
                    state.session.preferences.saved(),
                    true,
                    |_| {},
                );
                if !rejected.is_err_and(|error| error.contains("limit reached"))
                    || manager.borrow().count() != 1
                    || manager.borrow().used() != before
                    || !capture.window().is_visible()
                {
                    return Err("Editable document budget rejection changed the pin/editor".into());
                }
                capture.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            10 => {
                if app.borrow().capture.is_some() {
                    return self.waiting("Budget rejection cancel did not finish");
                }
                if !manager
                    .borrow()
                    .window(self.second)
                    .unwrap()
                    .window()
                    .is_visible()
                {
                    return Err("Existing pin did not restore after failed pin/cancel".into());
                }
                let ids = manager.borrow().ids();
                for id in ids {
                    let pin = manager.borrow().window(id).unwrap().clone_strong();
                    let width = pin.window().size().width as f32 / pin.window().scale_factor();
                    click(
                        &pin,
                        width - 22.,
                        17.,
                        slint::platform::PointerEventButton::Left,
                    );
                }
                if manager.borrow().count() != 0 || manager.borrow().used() != 0 {
                    return Err("Resident pin shutdown retained reservations".into());
                }
                glint_platform::copy_text("glint-no-image".into())?;
                ui.invoke_action("close".into());
                ui.invoke_action("clipboard-pin".into());
                ui.invoke_action("clipboard-pin".into());
                ui.invoke_action("capture".into());
                if !app.borrow().clipboard_pin_pending
                    || app.borrow().capture.is_some()
                    || app.borrow().capturing
                {
                    return Err("Clipboard read did not bound duplicate requests/capture".into());
                }
                self.started = Some(Instant::now());
            }
            11 => {
                if app.borrow().clipboard_pin_pending {
                    return self.waiting("Empty clipboard pin did not finish");
                }
                if manager.borrow().count() != 0
                    || !ui.window().is_visible()
                    || !app.borrow().feedback.contains("Cannot pin clipboard")
                {
                    return Err("No-image clipboard failure was hidden or created a pin".into());
                }
                let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    directory.join("clipboard-pin-no-image.png"),
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                glint_platform::copy_rgba(64, 48, [17, 93, 181, 255].repeat(64 * 48))?;
                self.clipboard_blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                ui.invoke_action("clipboard-pin".into());
                self.started = Some(Instant::now());
            }
            12 => {
                if app.borrow().clipboard_pin_pending {
                    return self.waiting("Occupied clipboard read did not finish");
                }
                if manager.borrow().count() != 0
                    || !app.borrow().feedback.contains("Cannot pin clipboard")
                {
                    return Err("Occupied clipboard read reported success".into());
                }
                self.clipboard_blocker.take();
                ui.invoke_action("close".into());
                ui.invoke_action("clipboard-pin".into());
                self.started = Some(Instant::now());
            }
            13 => {
                if app.borrow().clipboard_pin_pending {
                    return self.waiting("Clipboard pin retry did not finish");
                }
                if manager.borrow().count() != 1 || ui.window().is_visible() {
                    return Err(format!(
                        "Clipboard pin retry failed: {}",
                        app.borrow().feedback
                    ));
                }
                self.first = manager.borrow().ids()[0];
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                let pixels = pin
                    .get_picture()
                    .to_rgba8()
                    .ok_or("Clipboard pin bitmap missing")?;
                if (pixels.width(), pixels.height()) != (64, 48)
                    || pixels.as_bytes() != [17, 93, 181, 255].repeat(64 * 48)
                    || pins::native(&pin, |w| Ok(w.is_foreground()))?
                {
                    return Err("Clipboard pin pixels/focus differ after retry".into());
                }
                shot(&pin, &directory.join("clipboard-pin-created.png"))?;
                ui.invoke_action("clipboard-pin".into());
                self.started = Some(Instant::now());
            }
            14 => {
                if app.borrow().clipboard_pin_pending {
                    return self.waiting("Second clipboard pin did not finish");
                }
                if manager.borrow().count() != 2 {
                    return Err("Second clipboard pin replaced first".into());
                }
                self.used = manager.borrow().used();
                if let Err(error) =
                    glint_platform::copy_rgba(4096, 2160, [10, 20, 30, 255].repeat(4096 * 2160))
                {
                    return self
                        .waiting(&format!("Cannot prepare large clipboard fixture: {error}"));
                }
                ui.invoke_action("clipboard-pin".into());
                self.started = Some(Instant::now());
            }
            15 => {
                if app.borrow().clipboard_pin_pending {
                    return self.waiting("Clipboard budget rejection did not finish");
                }
                if manager.borrow().count() != 2
                    || manager.borrow().used() != self.used
                    || !app.borrow().feedback.contains("limit reached")
                {
                    return Err("Clipboard budget rejection lost existing pins or status".into());
                }
                if let Err(error) = glint_platform::copy_text("glint-clipboard-after-budget".into())
                {
                    return self
                        .waiting(&format!("Cannot replace large clipboard fixture: {error}"));
                }
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            16 => {
                let state = app.borrow();
                let Some(capture) = &state.capture else {
                    return self.waiting("Post clipboard failure capture did not open");
                };
                let capture = capture.clone_strong();
                drop(state);
                ui.invoke_action("clipboard-pin".into());
                if app.borrow().clipboard_pin_pending {
                    return Err("Clipboard pin started during capture".into());
                }
                capture.invoke_action("cancel".into());
                let ids = manager.borrow().ids();
                for id in ids {
                    manager.borrow_mut().close(id)?;
                }
                if manager.borrow().used() != 0 {
                    return Err("Clipboard pins retained reservations after close".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                ui.invoke_action("cancel".into());
            }
            17 => {
                crate::resident_smoke::snapshot(
                    ui,
                    &directory.join("completion-choice-before-click.png"),
                )?;
                use slint::platform::{PointerEventButton, WindowEvent};
                let position = slint::LogicalPosition::new(720., 255.);
                ui.window().dispatch_event(WindowEvent::PointerPressed {
                    position,
                    button: PointerEventButton::Left,
                });
                ui.window().dispatch_event(WindowEvent::PointerReleased {
                    position,
                    button: PointerEventButton::Left,
                });
            }
            18 => {
                use slint::platform::{Key, WindowEvent};
                if !self.finish_keys_sent {
                    for text in [
                        slint::SharedString::from(Key::DownArrow),
                        Key::Return.into(),
                    ] {
                        ui.window()
                            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
                        ui.window()
                            .dispatch_event(WindowEvent::KeyReleased { text });
                    }
                    self.finish_keys_sent = true;
                    self.started = Some(Instant::now());
                    return Ok(false);
                }
                if app.borrow().session.preferences.draft.output.finish
                    != glint_core::config::FinishAction::SaveAs
                {
                    let path = directory.join("completion-choice-pending.png");
                    if !path.exists() {
                        crate::resident_smoke::snapshot(ui, &path)?;
                        eprintln!(
                            "Waiting for completion choice input: draft={:?}, size={:?}, scale={}",
                            app.borrow().session.preferences.draft.output.finish,
                            ui.window().size(),
                            ui.window().scale_factor()
                        );
                    }
                    return self.waiting("Completion choice pointer/keyboard did not edit draft");
                }
                ui.invoke_action("cancel".into());
            }
            19 => {
                if app.borrow().session.preferences.draft.output.finish
                    != glint_core::config::FinishAction::CopyAndPin
                {
                    return Err("Cancel did not restore completion action".into());
                }
                let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    directory.join("settings-completion-restored.png"),
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
            }
            20 => {
                let config = app.borrow().session.preferences.saved().clone();
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &[23u8, 91, 177, 255].repeat(128 * 96),
                    128,
                    96,
                );
                self.first = pins::Manager::create(
                    &manager,
                    slint::Image::from_rgba8(pixels),
                    (180, 180),
                    &config,
                    true,
                    |_| {},
                )?;
                self.started = Some(Instant::now());
            }
            21 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if !pin.get_ready() {
                    return self.waiting("Pin output fixture did not initialize");
                }
                self.used = manager.borrow().used();
                pin.invoke_change_opacity(33.);
                pin.invoke_action("smaller".into());
                self.used = manager.borrow().used();
                self.clipboard_blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                let slot = crate::probe::OutputSlot::acquire()
                    .ok_or("Global output slot unexpectedly occupied")?;
                if crate::probe::OutputSlot::acquire().is_some()
                    || crate::probe::OutputSlot::acquire().is_some()
                {
                    return Err("Rejected output slot acquisition released its live owner".into());
                }
                drop(slot);
                pin.invoke_action("menu".into());
                self.started = Some(Instant::now());
            }
            22 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if !self.copy_started {
                    click(&pin, 160., 182., slint::platform::PointerEventButton::Left);
                    if !pin.get_busy() {
                        return Err("Pin copy menu pointer did not start output".into());
                    }
                    self.copy_started = true;
                    self.started = Some(Instant::now());
                    return Ok(false);
                }
                if pin.get_busy() {
                    return self.waiting("Occupied pin copy did not finish");
                }
                if !pin.get_message_open()
                    || !pin.get_details().contains("Output failed")
                    || manager.borrow().used() != self.used
                    || !pin.window().is_visible()
                {
                    return Err("Failed pin copy lost window/reservation or error details".into());
                }
                shot(&pin, &directory.join("pin-copy-failed.png"))?;
                self.clipboard_blocker.take();
                click(&pin, 230., 184., slint::platform::PointerEventButton::Left);
                if !pin.get_busy() {
                    return Err("Pin error retry pointer did not start copy".into());
                }
                self.started = Some(Instant::now());
            }
            23 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if pin.get_busy() {
                    return self.waiting("Pin copy retry did not finish");
                }
                let copied = glint_platform::read_clipboard_image()?;
                if (copied.width, copied.height) != (128, 96)
                    || copied.rgba != [23, 91, 177, 255].repeat(128 * 96)
                    || (pin.get_alpha() - 0.33).abs() > 0.001
                    || pin.get_zoom() != 75
                {
                    return Err(
                        "Pin copy included window zoom/opacity or wrong source pixels".into(),
                    );
                }
                manager.borrow_mut().diagnostic_choice(None);
                pin.invoke_action("save".into());
                if pin.get_busy()
                    || !pin.get_status().contains("cancelled")
                    || !pin.window().is_visible()
                {
                    return Err("Cancelled pin save did not preserve pin".into());
                }
                manager
                    .borrow_mut()
                    .diagnostic_choice(Some(directory.join("pin-output.png")));
                pin.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            24 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if pin.get_busy() {
                    return self.waiting("Pin PNG save did not finish");
                }
                let saved = image::open(directory.join("pin-output.png"))
                    .map_err(|e| e.to_string())?
                    .into_rgba8();
                if saved.dimensions() != (128, 96)
                    || saved.as_raw() != &[23, 91, 177, 255].repeat(128 * 96)
                    || !pin.window().is_visible()
                {
                    return Err("Pin PNG differs from source".into());
                }
                manager
                    .borrow_mut()
                    .diagnostic_choice(Some(directory.join("pin-output.png")));
                pin.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            25 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if pin.get_busy() {
                    return self.waiting("Pin file conflict did not finish");
                }
                if !pin.get_message_open() || manager.borrow().used() != self.used {
                    return Err("Pin conflict lost recovery/reservation".into());
                }
                shot(&pin, &directory.join("pin-save-conflict.png"))?;
                manager
                    .borrow_mut()
                    .diagnostic_choice(Some(directory.join("pin-output.jpg")));
                pin.invoke_action("save-elsewhere".into());
                self.started = Some(Instant::now());
            }
            26 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if pin.get_busy() {
                    return self.waiting("Pin JPEG save did not finish");
                }
                let saved = image::open(directory.join("pin-output.jpg"))
                    .map_err(|e| e.to_string())?
                    .into_rgb8();
                if saved.dimensions() != (128, 96)
                    || saved
                        .get_pixel(50, 50)
                        .0
                        .iter()
                        .zip([23i16, 91, 177])
                        .any(|(a, b)| (i16::from(*a) - b).abs() > 4)
                {
                    return Err("Pin JPEG pixels/resolution differ".into());
                }
                manager
                    .borrow_mut()
                    .diagnostic_choice(Some(directory.join("pin-output.webp")));
                pin.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            27 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if pin.get_busy() {
                    return self.waiting("Pin extension validation did not finish");
                }
                if !pin.get_message_open() || directory.join("pin-output.webp").exists() {
                    return Err("Unsupported pin extension was accepted".into());
                }
                let blocker = directory.join("pin-directory-blocker");
                std::fs::write(&blocker, b"keep").map_err(|e| e.to_string())?;
                let mut options = app.borrow().session.preferences.saved().output.clone();
                options.directory = blocker.display().to_string();
                manager.borrow_mut().set_output_options(&options);
                pin.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            28 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if pin.get_busy() {
                    return self.waiting("Pin initial directory failure did not finish");
                }
                if !pin.get_message_open()
                    || manager.borrow().count() != 1
                    || manager.borrow().used() != self.used
                {
                    return Err("Pin directory failure lost window or retry".into());
                }
                manager
                    .borrow_mut()
                    .diagnostic_choice(Some(directory.join("pin-recovered.png")));
                click(&pin, 230., 184., slint::platform::PointerEventButton::Left);
                if !pin.get_busy() {
                    return Err("Pin save-elsewhere retry pointer failed".into());
                }
                self.started = Some(Instant::now());
            }
            29 => {
                let pin = manager.borrow().window(self.first).unwrap().clone_strong();
                if pin.get_busy() {
                    return self.waiting("Pin alternate save did not finish");
                }
                if !directory.join("pin-recovered.png").is_file()
                    || std::fs::read(directory.join("pin-directory-blocker"))
                        .map_err(|e| e.to_string())?
                        != b"keep"
                    || !pin.window().is_visible()
                {
                    return Err("Pin alternate save failed or modified blocked path".into());
                }
                manager.borrow_mut().close(self.first)?;
                if manager.borrow().used() != 0 {
                    return Err("Pin output retained window reservation after close".into());
                }
            }
            _ => return Err("Unknown pin smoke stage".into()),
        }
        Ok(true)
    }
}
