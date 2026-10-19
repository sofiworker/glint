//! Real fresh desktop captures, geometry-only session memory and output failure retention.
use crate::{pins, probe, resident::AppState, settings::SettingsWindow};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Run {
    started: Option<Instant>,
    remembered: Option<glint_core::region_memory::RememberedRegion>,
    old: Option<glint_core::config::Config>,
    output: Option<PathBuf>,
    pin: u64,
    blocker: Option<glint_platform::ClipboardBlocker>,
}
fn shot(ui: &probe::CaptureProbe, path: &Path) -> Result<(), String> {
    let p = ui.window().take_snapshot().map_err(|e| e.to_string())?;
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
fn select(ui: &probe::CaptureProbe, x: f32, y: f32) {
    ui.set_tool(0);
    ui.invoke_pointer(0, x, y);
    ui.invoke_pointer(2, x + 400., y + 300.);
}
impl Run {
    fn wait(&self, reason: &str) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|s| s.elapsed() > Duration::from_secs(20))
        {
            Err(reason.into())
        } else {
            Ok(false)
        }
    }
    fn start(&mut self, ui: &SettingsWindow) {
        self.started = Some(Instant::now());
        ui.invoke_action("repeat-capture".into());
    }
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        hotkeys: &glint_platform::HotkeyService,
        directory: &Path,
    ) -> Result<bool, String> {
        let manager = app.borrow().pins.clone();
        if stage == 0 {
            self.old = Some(app.borrow().session.preferences.saved().clone());
            self.output = app.borrow().capture_output.clone();
            app.borrow_mut().capture_output = None;
            app.borrow_mut().last_region = None;
            settings.invoke_change("capture.remember_region".into(), "true".into());
            settings.invoke_change("pins.hide_on_capture".into(), "false".into());
            settings.invoke_change("output.auto_save".into(), "false".into());
            settings.invoke_action("save".into());
            settings.invoke_record("shortcuts.last_region".into());
            if settings.get_modal() != 3 || !settings.get_can_clear() {
                return Err("Repeat hotkey recorder missing or mandatory".into());
            }
            settings.invoke_action("clear-key".into());
            settings.invoke_action("save".into());
            if hotkeys.bindings()?.iter().any(|b| b.action == 3) {
                return Err("Cleared repeat shortcut remains registered".into());
            }
            settings.invoke_change("shortcuts.last_region".into(), "Ctrl+Shift+R".into());
            settings.invoke_action("save".into());
            if !hotkeys
                .bindings()?
                .iter()
                .any(|b| b.action == 3 && b.key == u32::from(b'R'))
            {
                return Err("Repeat shortcut was not registered".into());
            }
            self.start(settings);
            return Ok(true);
        }
        let capture = app.borrow().capture.as_ref().map(|c| c.clone_strong());
        if [2, 6, 8, 10, 12, 14].contains(&stage) && (capture.is_some() || app.borrow().capturing) {
            return self.wait("Capture completion did not return to tray");
        }
        if [1, 4, 5, 7, 9, 11, 13].contains(&stage) && capture.is_none() {
            return self.wait("Repeat/editor did not open");
        }
        match stage {
            1 => {
                let capture = capture.unwrap();
                if capture.get_selected() || capture.get_repeat_notice().is_empty() {
                    return Err("Empty memory did not fall back visibly".into());
                }
                select(&capture, 120., 200.);
                capture.set_tool(2);
                capture.invoke_pointer(0, 150., 250.);
                capture.invoke_pointer(2, 220., 300.);
                capture.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            2 => {
                self.remembered = app.borrow().last_region.clone();
                if self.remembered.is_none() {
                    return Err("Successful desktop output did not remember source geometry".into());
                }
                let copied = glint_platform::read_clipboard_image()?;
                if (copied.width, copied.height) != (400, 300) {
                    return Err("Initial region export dimensions wrong".into());
                }
                let mut config = app.borrow().session.preferences.saved().clone();
                config.pins.opacity = 100;
                config.pins.zoom_percent = 100;
                config.pins.on_top = true;
                config.pins.passthrough = false;
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &[10u8, 210, 90, 255].repeat(400 * 300),
                    400,
                    300,
                );
                self.pin = pins::Manager::create(
                    &manager,
                    slint::Image::from_rgba8(pixels),
                    (120, 200),
                    &config,
                    true,
                    |_| {},
                )?;
                self.started = Some(Instant::now());
            }
            3 => {
                if !manager
                    .borrow()
                    .window(self.pin)
                    .ok_or("Fresh-source fixture closed")?
                    .get_ready()
                {
                    return self.wait("Fresh-source fixture not ready");
                }
                self.start(settings);
            }
            4 => {
                let capture = capture.unwrap();
                if !capture.get_selected()
                    || (
                        capture.get_left(),
                        capture.get_top(),
                        capture.get_wide(),
                        capture.get_high(),
                    ) != (120., 200., 400., 300.)
                    || capture.get_object_count() != 0
                    || capture.get_can_undo()
                {
                    return Err("Repeat selection/source objects were not reset".into());
                }
                let pixels = capture
                    .get_desktop()
                    .to_rgba8()
                    .ok_or("Missing fresh desktop")?;
                let displays = glint_platform::desktop_environment()?.displays;
                let left = displays.iter().map(|d| d.bounds.left).min().unwrap();
                let top = displays.iter().map(|d| d.bounds.top).min().unwrap();
                let index =
                    (((300 - top) as usize * pixels.width() as usize) + (200 - left) as usize) * 4;
                if pixels.as_bytes().get(index..index + 3) != Some(&[10, 210, 90]) {
                    return Err(format!(
                        "Repeat did not capture current reference pixels: {:?}",
                        pixels.as_bytes().get(index..index + 4)
                    ));
                }
                shot(&capture, &directory.join("repeat-selected.png"))?;
                select(&capture, 220., 250.);
                self.blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                capture.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            5 => {
                let capture = capture.unwrap();
                if capture.get_busy() {
                    return self.wait("Repeat clipboard failure pending");
                }
                if !capture.get_status().starts_with(crate::messages::text(
                    crate::messages::Message::CaptureOutputFailed,
                    capture.get_english(),
                )) || app.borrow().last_region != self.remembered
                    || capture.get_closed()
                {
                    return Err("Failed output replaced memory or discarded editor".into());
                }
                self.blocker.take();
                capture.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            6 => {
                if app.borrow().last_region != self.remembered {
                    return Err("Cancel replaced remembered region".into());
                }
                settings.invoke_change("capture.remember_region".into(), "false".into());
                settings.invoke_action("save".into());
                if app.borrow().last_region.is_some() {
                    return Err("Disabling region memory did not clear it".into());
                }
                self.start(settings);
            }
            7 => {
                let capture = capture.unwrap();
                if capture.get_selected()
                    || capture.get_repeat_notice()
                        != crate::messages::text(
                            crate::messages::Message::RepeatOff,
                            capture.get_english(),
                        )
                {
                    return Err("Disabled memory did not fall back visibly".into());
                }
                select(&capture, 180., 150.);
                capture.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            8 => {
                settings.invoke_change("capture.remember_region".into(), "true".into());
                settings.invoke_action("save".into());
                app.borrow_mut().capture_output = Some(directory.join("repeat-saved.png"));
                self.start(settings);
            }
            9 => {
                let capture = capture.unwrap();
                if capture.get_selected()
                    || capture.get_repeat_notice()
                        != crate::messages::text(
                            crate::messages::Message::RepeatEmpty,
                            capture.get_english(),
                        )
                {
                    return Err("Re-enabled memory restored a cleared region".into());
                }
                select(&capture, 120., 200.);
                capture.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            10 => {
                let image =
                    image::open(directory.join("repeat-saved.png")).map_err(|e| e.to_string())?;
                if (image.width(), image.height()) != (400, 300)
                    || app.borrow().last_region.is_none()
                {
                    return Err("Successful PNG did not remember exact crop".into());
                }
                self.remembered = app.borrow().last_region.clone();
                let env = glint_platform::desktop_environment()?;
                let mut displays: Vec<_> = env
                    .displays
                    .iter()
                    .map(|d| glint_core::region_memory::DisplayGeometry {
                        bounds: [d.bounds.left, d.bounds.top, d.bounds.right, d.bounds.bottom],
                        dpi: d.effective_dpi,
                    })
                    .collect();
                displays[0].dpi.0 += 1;
                let left = displays.iter().map(|d| d.bounds[0]).min().unwrap();
                let top = displays.iter().map(|d| d.bounds[1]).min().unwrap();
                let right = displays.iter().map(|d| d.bounds[2]).max().unwrap();
                let bottom = displays.iter().map(|d| d.bounds[3]).max().unwrap();
                let layout = glint_core::region_memory::CaptureLayout::new(
                    (left, top),
                    ((right - left) as u32, (bottom - top) as u32),
                    displays,
                )
                .unwrap();
                app.borrow_mut().last_region = glint_core::region_memory::RememberedRegion::new(
                    layout,
                    glint_core::Rect {
                        x: 120,
                        y: 200,
                        width: 400,
                        height: 300,
                    },
                );
                self.start(settings);
            }
            11 => {
                let capture = capture.unwrap();
                if capture.get_selected() || !capture.get_repeat_notice().contains("DPI") {
                    return Err("Changed display mapping reused unsafe coordinates".into());
                }
                shot(&capture, &directory.join("repeat-changed.png"))?;
                capture.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            12 => {
                app.borrow_mut().last_region = self.remembered.clone();
                let pin = manager.borrow().window(self.pin).unwrap().clone_strong();
                pin.invoke_action("edit".into());
                self.started = Some(Instant::now());
            }
            13 => {
                let capture = capture.unwrap();
                if !capture.get_pin_editing() {
                    return Err("Repeat isolation editor not loaded".into());
                }
                let x = capture.get_left() + 30.;
                let y = capture.get_top() + 30.;
                capture.set_tool(2);
                capture.invoke_pointer(0, x, y);
                capture.invoke_pointer(2, x + 100., y + 50.);
                capture.invoke_action("pin".into());
                self.started = Some(Instant::now());
            }
            14 => {
                if app.borrow().last_region != self.remembered {
                    return Err("Pin editing overwrote desktop repeat geometry".into());
                }
                manager.borrow_mut().close(self.pin)?;
                app.borrow_mut().capture_output = self.output.take();
                let old = self.old.take().unwrap();
                for (key, value) in [
                    (
                        "capture.remember_region",
                        old.capture.remember_region.to_string(),
                    ),
                    ("pins.hide_on_capture", old.pins.hide_on_capture.to_string()),
                    ("output.auto_save", old.output.auto_save.to_string()),
                    ("shortcuts.last_region", old.shortcuts.last_region),
                ] {
                    settings.invoke_change(key.into(), value.into());
                }
                settings.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Repeat fixture settings did not restore".into());
                }
            }
            _ => return Err("Unknown repeat stage".into()),
        }
        Ok(true)
    }
}
