//! Resident click-through verification against real Win32 flags and OS hit testing.
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
    id: u64,
    second: u64,
    start: Option<Instant>,
    foreground: usize,
    blocker: Option<glint_platform::HotkeyService>,
    bindings: Vec<glint_platform::HotkeyBinding>,
}
fn click(ui: &DesktopPin, x: f32, y: f32) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}
fn snapshot(ui: &DesktopPin, path: &Path) -> Result<(), String> {
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
impl Run {
    fn wait(&self, reason: &str) -> Result<bool, String> {
        if self
            .start
            .is_some_and(|t| t.elapsed() > Duration::from_secs(15))
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
        hotkeys: &glint_platform::HotkeyService,
        directory: &Path,
    ) -> Result<bool, String> {
        let manager = app.borrow().pins.clone();
        match stage {
            0 => {
                ui.invoke_change("pins.passthrough".into(), "true".into());
                ui.invoke_action("save".into());
                let config = app.borrow().session.preferences.saved().clone();
                if !config.pins.passthrough {
                    return Err("Default click-through setting did not save".into());
                }
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &[46u8, 86, 151, 255].repeat(400 * 300),
                    400,
                    300,
                );
                self.id = pins::Manager::create(
                    &manager,
                    slint::Image::from_rgba8(pixels),
                    (180, 180),
                    &config,
                    true,
                    |_| {},
                )?;
                self.start = Some(Instant::now());
            }
            1 => {
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                if !pin.get_ready() {
                    return self.wait("Default pass fixture did not initialize");
                }
                if !pin.get_confirm_pass()
                    || !pin.get_rescue_ready()
                    || pin.get_rescue_shortcut()
                        != app.borrow().session.preferences.saved().shortcuts.rescue
                    || pins::native(&pin, |w| w.state())?.click_through
                {
                    return Err("Default pass skipped confirmation/live recovery gate".into());
                }
                snapshot(&pin, &directory.join("pin-pass-confirm.png"))?;
                click(&pin, 90., 374.);
                if pin.get_confirm_pass() || pin.get_passthrough() {
                    return Err("Cancel pass confirmation did not preserve interactivity".into());
                }
                pin.invoke_action("confirm-pass".into());
                if pin.get_passthrough() {
                    return Err("Confirmation bypass enabled pass after cancellation".into());
                }
                pin.invoke_action("pass".into());
                self.foreground = glint_platform::foreground_window_id();
            }
            2 => {
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                click(&pin, 300., 374.);
                if !pin.get_passthrough() || pin.get_confirm_pass() {
                    return Err("Enable pass pointer did not apply".into());
                }
            }
            3 => {
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                let state = pins::native(&pin, |w| w.state())?;
                if !state.click_through
                    || !state.no_activate
                    || pins::native(&pin, |w| w.receives_pointer_at(state.x + 20, state.y + 18))?
                    || glint_platform::foreground_window_id() != self.foreground
                {
                    return Err(format!(
                        "Native click-through/hit testing/focus mismatch: {state:?}"
                    ));
                }
                ui.invoke_action("toggle-pins".into());
            }
            4 => {
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                if pin.window().is_visible() {
                    return Err("Passing pin did not hide".into());
                }
                ui.invoke_action("toggle-pins".into());
            }
            5 => {
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                if !pin.window().is_visible()
                    || !pins::native(&pin, |w| w.state())?.click_through
                    || !pin.get_passthrough()
                {
                    return Err("Hide/restore lost passing pin state".into());
                }
                ui.invoke_action("capture".into());
                self.start = Some(Instant::now());
            }
            6 => {
                let Some(capture) = app
                    .borrow()
                    .capture
                    .as_ref()
                    .map(ComponentHandle::clone_strong)
                else {
                    return self.wait("Passing pin capture did not open");
                };
                if manager
                    .borrow()
                    .window(self.id)
                    .unwrap()
                    .window()
                    .is_visible()
                {
                    return Err("Passing pin was not hidden during capture".into());
                }
                capture.invoke_action("cancel".into());
                self.start = Some(Instant::now());
            }
            7 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Pass capture cancellation did not finish");
                }
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                if !pin.window().is_visible() || !pins::native(&pin, |w| w.state())?.click_through {
                    return Err("Capture cancellation lost passing pin state".into());
                }
                ui.invoke_action("hotkey-error".into());
            }
            8 => {
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                let state = pins::native(&pin, |w| w.state())?;
                if state.click_through
                    || !state.visible
                    || pin.get_passthrough()
                    || pin.get_rescue_ready()
                    || !pins::native(&pin, |w| w.receives_pointer_at(state.x + 20, state.y + 18))?
                {
                    return Err("Hotkey failure did not restore OS hit testing".into());
                }
                pin.invoke_action("pass".into());
                pin.invoke_action("confirm-pass".into());
                if pin.get_passthrough() || !pin.get_message_open() {
                    return Err("Unavailable rescue allowed pass".into());
                }
                snapshot(&pin, &directory.join("pin-pass-unavailable.png"))?;
                ui.invoke_action("save".into());
                pin.invoke_action("pass".into());
                pin.invoke_action("confirm-pass".into());
                if !pin.get_passthrough() {
                    return Err(
                        "Successful rescue re-registration did not restore pass availability"
                            .into(),
                    );
                }
                self.blocker = Some(glint_platform::HotkeyService::new(
                    vec![glint_platform::HotkeyBinding {
                        action: 1,
                        modifiers: 7,
                        key: 0x83,
                    }],
                    |_| {},
                )?);
                let previous = app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .shortcuts
                    .rescue
                    .clone();
                ui.invoke_change("shortcuts.rescue".into(), "Ctrl+Alt+Shift+F20".into());
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.saved().shortcuts.rescue != previous
                    || pin.get_rescue_shortcut() != previous
                    || !pin.get_passthrough()
                {
                    return Err(
                        "Conflict did not preserve previous usable rescue binding/pass state"
                            .into(),
                    );
                }
            }
            9 => {
                self.blocker.take();
                ui.invoke_action("save".into());
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                if pin.get_rescue_shortcut() != "Ctrl+Alt+Shift+F20"
                    || !hotkeys
                        .bindings()?
                        .iter()
                        .any(|b| b.action == 6 && b.key == 0x83 && b.modifiers == 7)
                {
                    return Err("New rescue binding was not registered/updated".into());
                }
                ui.invoke_action("rescue-pins".into());
                if pin.get_passthrough() || pins::native(&pin, |w| w.state())?.click_through {
                    return Err("Tray rescue did not clear pass".into());
                }
                pin.invoke_action("pass".into());
                self.bindings = hotkeys.bindings()?;
                hotkeys.replace(
                    self.bindings
                        .iter()
                        .copied()
                        .filter(|b| b.action != 6)
                        .collect(),
                )?;
                pin.invoke_action("confirm-pass".into());
                if pin.get_passthrough()
                    || pins::native(&pin, |w| w.state())?.click_through
                    || !pin.get_message_open()
                {
                    return Err("Rescue lost during confirmation still allowed pass".into());
                }
                let config = app.borrow().session.preferences.saved().clone();
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &[46u8, 86, 151, 255].repeat(128 * 96),
                    128,
                    96,
                );
                self.second = pins::Manager::create(
                    &manager,
                    slint::Image::from_rgba8(pixels),
                    (640, 180),
                    &config,
                    true,
                    |_| {},
                )?;
                self.start = Some(Instant::now());
            }
            10 => {
                let pin = manager.borrow().window(self.second).unwrap().clone_strong();
                if !pin.get_ready() {
                    return self.wait("Unavailable default pass fixture did not initialize");
                }
                if pin.get_passthrough()
                    || pin.get_confirm_pass()
                    || !pin.get_message_open()
                    || pins::native(&pin, |w| w.state())?.click_through
                {
                    return Err("Default pass enabled without live rescue".into());
                }
                snapshot(&pin, &directory.join("pin-pass-default-blocked.png"))?;
                hotkeys.replace(self.bindings.clone())?;
                ui.invoke_action("rescue-pins".into());
                ui.invoke_change("pins.passthrough".into(), "false".into());
                ui.invoke_change("shortcuts.rescue".into(), "Ctrl+Alt+Escape".into());
                ui.invoke_action("save".into());
                manager.borrow_mut().close(self.second)?;
                let first = manager.borrow().window(self.id).unwrap().clone_strong();
                first.invoke_action("pass".into());
                first.invoke_action("confirm-pass".into());
                if !first.get_passthrough() {
                    return Err("Pass fixture did not re-enable before capture failure test".into());
                }
                ui.invoke_action("capture".into());
                ui.invoke_action("hotkey-error".into());
                self.start = Some(Instant::now());
            }
            11 => {
                let Some(capture) = app
                    .borrow()
                    .capture
                    .as_ref()
                    .map(ComponentHandle::clone_strong)
                else {
                    return self.wait("Pass failure capture did not open");
                };
                capture.invoke_hover(350., 250.);
                if capture.get_service_notice()
                    != crate::messages::text(
                        crate::messages::Message::ShortcutServiceStopped,
                        capture.get_english(),
                    )
                {
                    return Err(
                        "Pending capture hotkey failure was not visible after opening overlay"
                            .into(),
                    );
                }
                capture.invoke_pointer(0, 100., 100.);
                capture.invoke_pointer(2, 500., 400.);
                capture.set_tool(1);
                capture.invoke_pointer(0, 150., 150.);
                capture.invoke_pointer(2, 250., 220.);
                if capture.get_object_count() != 1 {
                    return Err("Service failure prevented annotation".into());
                }
                capture.invoke_action("undo".into());
                if capture.get_object_count() != 0 || !capture.get_selected() {
                    return Err("Service failure prevented undo or erased selection".into());
                }
                let notice = crate::messages::text(
                    crate::messages::Message::ShortcutServiceStopped,
                    capture.get_english(),
                );
                if capture.get_service_notice() != notice
                    || !capture.get_info_text().starts_with(notice)
                {
                    return Err("Capture interaction erased the persistent service failure".into());
                }
                crate::resident_smoke::capture_snapshot(
                    &capture,
                    &directory.join("pin-pass-capture-service-failed.png"),
                )?;
                ui.invoke_action("hotkey-error".into());
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                let state = pins::native(&pin, |w| w.state())?;
                if state.visible
                    || state.click_through
                    || pin.get_passthrough()
                    || !capture.window().is_visible()
                {
                    return Err("Hotkey failure revealed pins or interrupted active capture".into());
                }
                ui.invoke_action("save".into());
                if !capture.get_service_notice().is_empty()
                    || manager.borrow().recovery_stopped()
                    || !capture.window().is_visible()
                {
                    return Err("Successful hotkey repair retained stale service failure".into());
                }
                ui.invoke_action("hotkey-error".into());
                if capture.get_service_notice() != notice {
                    return Err("Repeated hotkey failure lost persistent notice".into());
                }
                capture.invoke_action("cancel".into());
                self.start = Some(Instant::now());
            }
            12 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Pass failure capture cancel did not finish");
                }
                let pin = manager.borrow().window(self.id).unwrap().clone_strong();
                if !pin.window().is_visible()
                    || pins::native(&pin, |w| w.state())?.click_through
                    || pin.get_passthrough()
                {
                    return Err("Hotkey failure capture did not restore interactive pin".into());
                }
                ui.invoke_action("save".into());
                manager.borrow_mut().close(self.id)?;
                if manager.borrow().used() != 0 {
                    return Err("Pass fixture retained reservations".into());
                }
            }
            _ => return Err("Unknown pass smoke stage".into()),
        }
        Ok(true)
    }
}
