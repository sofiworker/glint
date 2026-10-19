//! Real event-loop delay, cancellation, source freshness and capture routing.
use crate::{pins, resident::AppState, settings::SettingsWindow};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Run {
    pending_interrupt: bool,
    old: Option<glint_core::config::Config>,
    started: Option<Instant>,
    pin: u64,
    countdown: Option<crate::capture_delay::CaptureCountdown>,
    remembered: Option<glint_core::region_memory::RememberedRegion>,
}
fn picture(color: [u8; 4]) -> slint::Image {
    slint::Image::from_rgba8(
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &color.repeat(400 * 300),
            400,
            300,
        ),
    )
}
fn shot(window: &slint::Window, path: &Path) -> Result<(), String> {
    let p = window.take_snapshot().map_err(|e| e.to_string())?;
    image::save_buffer_with_format(
        path,
        p.as_bytes(),
        p.width(),
        p.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())
}
impl Run {
    fn elapsed(&self) -> Duration {
        self.started.unwrap().elapsed()
    }
    fn wait(&self, reason: &str) -> Result<bool, String> {
        if self.elapsed() > Duration::from_secs(20) {
            Err(reason.into())
        } else {
            Ok(false)
        }
    }
    fn start(&mut self, settings: &SettingsWindow, action: &str) {
        self.started = Some(Instant::now());
        settings.invoke_action(action.into());
    }
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let manager = app.borrow().pins.clone();
        let capture = app.borrow().capture.as_ref().map(|c| c.clone_strong());
        if ([4, 8, 10, 15, 17].contains(&stage) || (stage == 12 && !self.pending_interrupt))
            && (capture.is_some() || app.borrow().capturing)
        {
            return self.wait("Delay capture did not finish");
        }
        if [3, 9, 11, 14, 16].contains(&stage) && capture.is_none() {
            let state = app.borrow();
            return self.wait(&format!("Delayed editor did not open: countdown={}, capturing={}, elapsed={}, dirty={}, feedback={}", state.countdown.is_some(), state.capturing, state.delay_elapsed, state.session.preferences.is_dirty(), state.feedback));
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.remembered = app.borrow().last_region.clone();
                for (k, v) in [
                    ("capture.delay_seconds", "2"),
                    ("general.language", "en"),
                    ("pins.hide_on_capture", "false"),
                    ("output.auto_save", "false"),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                let mut config = app.borrow().session.preferences.saved().clone();
                config.pins.opacity = 100;
                config.pins.zoom_percent = 100;
                config.pins.on_top = true;
                config.pins.passthrough = false;
                self.pin = pins::Manager::create(
                    &manager,
                    picture([220, 40, 70, 255]),
                    (120, 200),
                    &config,
                    true,
                    |_| {},
                )?;
                self.started = Some(Instant::now());
            }
            1 => {
                if !manager
                    .borrow()
                    .window(self.pin)
                    .ok_or("Missing delay fixture")?
                    .get_ready()
                {
                    return self.wait("Delay fixture not ready");
                }
                self.start(settings, "capture");
                let state = app.borrow();
                if state.capture.is_some()
                    || state.capture_result.is_some()
                    || !state.capturing
                    || state.countdown.is_none()
                {
                    return Err("Countdown started desktop acquisition early".into());
                }
            }
            2 => {
                if self.elapsed() < Duration::from_millis(500) {
                    return Ok(false);
                }
                let ui = app
                    .borrow()
                    .countdown
                    .as_ref()
                    .ok_or("Missing active countdown")?
                    .ui
                    .clone_strong();
                if !ui.window().is_visible() || capture.is_some() {
                    return Err("Countdown visibility/acquisition wrong".into());
                }
                shot(ui.window(), &directory.join("delay-en.png"))?;
                manager
                    .borrow()
                    .window(self.pin)
                    .unwrap()
                    .set_picture(picture([10, 210, 90, 255]));
                self.countdown = Some(ui);
                settings.invoke_action("window-capture".into());
                if app.borrow().countdown.as_ref().unwrap().action != "capture" {
                    return Err("Concurrent trigger replaced countdown".into());
                }
            }
            3 => {
                let capture = capture.unwrap();
                if self.elapsed() < Duration::from_secs(2) || app.borrow().countdown.is_some() {
                    return Err("Countdown expired before saved delay".into());
                }
                let ui = self.countdown.take().unwrap();
                if ui.window().is_visible() {
                    return Err("Countdown entered acquired desktop".into());
                }
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                ui.window()
                    .with_winit_window(|window| -> Result<(), String> {
                        let handle = window.window_handle().map_err(|e| e.to_string())?;
                        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                            return Err("Not Win32".into());
                        };
                        let native = unsafe {
                            glint_platform::NativePinWindow::from_raw(
                                handle.hwnd.get() as *mut std::ffi::c_void
                            )
                        };
                        if native.state()?.visible {
                            return Err("Countdown HWND still visible".into());
                        }
                        Ok(())
                    })
                    .ok_or("Missing countdown HWND")??;
                let pixels = capture
                    .get_desktop()
                    .to_rgba8()
                    .ok_or("No delayed source")?;
                let env = glint_platform::desktop_environment()?;
                let left = env.displays.iter().map(|d| d.bounds.left).min().unwrap();
                let top = env.displays.iter().map(|d| d.bounds.top).min().unwrap();
                let i =
                    (((300 - top) as usize * pixels.width() as usize) + (200 - left) as usize) * 4;
                if pixels.as_bytes().get(i..i + 3) != Some(&[10, 210, 90]) {
                    return Err(format!(
                        "Delayed source froze before expiry: {:?}",
                        pixels.as_bytes().get(i..i + 4)
                    ));
                }
                shot(capture.window(), &directory.join("delay-fresh.png"))?;
                capture.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            4 => {
                settings.invoke_change("general.language".into(), "zh_cn".into());
                settings.invoke_action("save".into());
                if app.borrow().session.preferences.saved().general.language
                    != glint_core::config::Language::ZhCn
                {
                    return Err("Chinese countdown fixture did not save language".into());
                }
                self.start(settings, "repeat-capture");
            }
            5 => {
                if self.elapsed() < Duration::from_millis(400) {
                    return Ok(false);
                }
                let ui = app
                    .borrow()
                    .countdown
                    .as_ref()
                    .ok_or("Repeat lost delay")?
                    .ui
                    .clone_strong();
                if ui.get_heading()
                    != crate::messages::text(crate::messages::Message::DelayTitle, false)
                {
                    return Err("Countdown did not use saved Chinese language".into());
                }
                shot(ui.window(), &directory.join("delay-zh.png"))?;
                use slint::platform::{PointerEventButton, WindowEvent};
                let position = slint::LogicalPosition::new(220., 210.);
                ui.window().dispatch_event(WindowEvent::PointerPressed {
                    position,
                    button: PointerEventButton::Left,
                });
                ui.window().dispatch_event(WindowEvent::PointerReleased {
                    position,
                    button: PointerEventButton::Left,
                });
                if app.borrow().countdown.is_some()
                    || app.borrow().capturing
                    || ui.window().is_visible()
                {
                    return Err("Countdown cancel button failed".into());
                }
                self.started = Some(Instant::now());
            }
            6 => {
                if self.elapsed() < Duration::from_millis(2300) {
                    return Ok(false);
                }
                if capture.is_some()
                    || app.borrow().capturing
                    || app.borrow().last_region != self.remembered
                {
                    return Err("Cancelled countdown later captured or changed memory".into());
                }
                self.start(settings, "window-capture");
            }
            7 => {
                if self.elapsed() < Duration::from_millis(300) {
                    return Ok(false);
                }
                let ui = app
                    .borrow()
                    .countdown
                    .as_ref()
                    .ok_or("Window lost delay")?
                    .ui
                    .clone_strong();
                use slint::platform::{Key, WindowEvent};
                ui.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Escape.into(),
                });
                ui.window().dispatch_event(WindowEvent::KeyReleased {
                    text: Key::Escape.into(),
                });
                if app.borrow().countdown.is_some() || app.borrow().capturing {
                    return Err("Countdown Escape failed".into());
                }
                self.started = Some(Instant::now());
            }
            8 => {
                if self.elapsed() < Duration::from_millis(2300) {
                    return Ok(false);
                }
                self.start(settings, "window-capture");
            }
            9 => {
                let capture = capture.unwrap();
                if self.elapsed() < Duration::from_secs(2) || !app.borrow().window_capture {
                    return Err("Delayed window routing failed".into());
                }
                capture.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            10 => {
                self.start(settings, "repeat-capture");
            }
            11 => {
                let capture = capture.unwrap();
                if self.elapsed() < Duration::from_secs(2)
                    || !app.borrow().repeat_capture
                    || capture.get_repeat_notice().is_empty()
                {
                    return Err("Delayed repeat routing failed".into());
                }
                capture.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            12 => {
                if !self.pending_interrupt {
                    self.start(settings, "capture");
                    settings.invoke_action("open".into());
                    if app.borrow().countdown.is_some()
                        || app.borrow().capturing
                        || !settings.window().is_visible()
                    {
                        return Err("Open settings did not cancel pending countdown".into());
                    }
                    self.start(settings, "capture");
                    self.pending_interrupt = true;
                    return Ok(false);
                }
                if self.elapsed() < Duration::from_millis(400) {
                    return Ok(false);
                }
                // Inject just the internal expiry event to exercise cancellation
                // between hiding the countdown and queued acquisition. Real timer
                // duration and source freshness are verified in stages 1-3/8-11.
                settings.invoke_action("delay-ready".into());
                if !app.borrow().delay_elapsed || !app.borrow().capturing {
                    return Err(format!(
                        "Queued acquisition state missing: {}",
                        app.borrow().feedback
                    ));
                }
                settings.invoke_action("open".into());
                if app.borrow().delay_elapsed || app.borrow().capturing {
                    return Err("Opening settings did not invalidate queued acquisition".into());
                }
                self.started = Some(Instant::now());
            }
            13 => {
                if self.elapsed() < Duration::from_millis(2300) {
                    return Ok(false);
                }
                if capture.is_some() || app.borrow().capturing {
                    return Err("Interrupted countdown fired late".into());
                }
                settings.invoke_change("capture.delay_seconds".into(), "0".into());
                settings.invoke_action("cancel".into());
                if app.borrow().session.preferences.draft.capture.delay_seconds != 2 {
                    return Err("Cancel did not restore saved delay".into());
                }
                settings.invoke_change("capture.delay_seconds".into(), "0".into());
                settings.invoke_action("save".into());
                self.start(settings, "capture");
                if app.borrow().countdown.is_some() {
                    return Err("Zero delay opened countdown".into());
                }
            }
            14 => {
                capture.unwrap().invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            15 => {
                settings.invoke_change("capture.delay_seconds".into(), "2".into());
                settings.invoke_action("save".into());
                self.start(settings, &format!("edit-pin:{}", self.pin));
            }
            16 => {
                if app.borrow().countdown.is_some() || app.borrow().edit_pin != Some(self.pin) {
                    return Err("Pin re-edit incorrectly delayed desktop acquisition".into());
                }
                capture.unwrap().invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            17 => {
                manager.borrow_mut().close(self.pin)?;
                let old = self.old.take().unwrap();
                let language = match old.general.language {
                    glint_core::config::Language::En => "en",
                    glint_core::config::Language::ZhCn => "zh_cn",
                    glint_core::config::Language::System => "system",
                };
                for (k, v) in [
                    (
                        "capture.delay_seconds",
                        old.capture.delay_seconds.to_string(),
                    ),
                    ("general.language", language.into()),
                    ("pins.hide_on_capture", old.pins.hide_on_capture.to_string()),
                    ("output.auto_save", old.output.auto_save.to_string()),
                ] {
                    settings.invoke_change(k.into(), v.into());
                }
                settings.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty()
                    || app.borrow().last_region != self.remembered
                {
                    return Err("Delay fixture restoration failed".into());
                }
            }
            _ => return Err("Unknown delay stage".into()),
        }
        Ok(true)
    }
}
