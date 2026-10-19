//! Actual native windows and subclass delivery; no physical dragging or monitor changes.
use crate::{pins, resident::AppState, settings::SettingsWindow};
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
    old_snap: bool,
    started: Option<Instant>,
    expected_x: i32,
    expected_y: i32,
    second_position: (i32, i32),
    foreground: usize,
}
fn state(pin: &pins::DesktopPin) -> Result<glint_platform::PinWindowState, String> {
    pins::native(pin, |w| w.state())
}
fn position(pin: &pins::DesktopPin, x: i32, y: i32, display: bool) -> Result<(), String> {
    pins::native(pin, |w| {
        w.position(x, y)?;
        w.diagnostic_geometry_event(display)
    })
}
fn shot(pin: &pins::DesktopPin, path: &Path) -> Result<(), String> {
    let image = pin.window().take_snapshot().map_err(|e| e.to_string())?;
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
impl Run {
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let manager = app.borrow().pins.clone();
        if stage == 0 {
            self.old_snap = app.borrow().session.preferences.saved().pins.snap;
            settings.invoke_change("pins.snap".into(), "true".into());
            settings.invoke_action("save".into());
            let mut config = app.borrow().session.preferences.saved().clone();
            config.pins.passthrough = false;
            config.pins.on_top = false;
            config.pins.zoom_percent = 100;
            config.pins.opacity = 55;
            let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                &[141u8, 183, 99, 255].repeat(400 * 300),
                400,
                300,
            );
            self.id = pins::Manager::create(
                &manager,
                slint::Image::from_rgba8(pixels.clone()),
                (120, 180),
                &config,
                true,
                |_| {},
            )?;
            self.second = pins::Manager::create(
                &manager,
                slint::Image::from_rgba8(pixels),
                (1050, 480),
                &config,
                true,
                |_| {},
            )?;
            self.started = Some(Instant::now());
            return Ok(true);
        }
        if stage == 9 {
            if manager.borrow().count() != 1 || manager.borrow().window(self.id).is_some() {
                return Err("Queued geometry event resurrected closed pin".into());
            }
            manager.borrow_mut().close(self.second)?;
            settings.invoke_change("pins.snap".into(), self.old_snap.to_string().into());
            settings.invoke_action("save".into());
            if manager.borrow().count() != 0 || manager.borrow().used() != 0 {
                return Err("Geometry fixtures retained reservations".into());
            }
            return Ok(true);
        }
        let pin = manager
            .borrow()
            .window(self.id)
            .ok_or("Geometry fixture closed")?
            .clone_strong();
        let other = manager
            .borrow()
            .window(self.second)
            .ok_or("Geometry target closed")?
            .clone_strong();
        match stage {
            1 => {
                if !pin.get_ready() || !other.get_ready() {
                    if self
                        .started
                        .is_some_and(|s| s.elapsed() > Duration::from_secs(15))
                    {
                        return Err("Geometry fixtures not ready".into());
                    }
                    return Ok(false);
                }
                let work = pins::native(&pin, |w| w.work_area())?;
                self.expected_x = work.0;
                self.expected_y = work.1 + 85;
                let target = state(&other)?;
                self.second_position = (target.x, target.y);
                position(&pin, self.expected_x + 1, self.expected_y, false)?;
            }
            2 => {
                let actual = state(&pin)?;
                if actual.x != self.expected_x {
                    return Err(format!(
                        "Screen-edge subclass snap failed: {} != {}",
                        actual.x, self.expected_x
                    ));
                }
                settings.invoke_change("pins.snap".into(), "false".into());
                position(&pin, self.expected_x + 1, self.expected_y, false)?;
            }
            3 => {
                if state(&pin)?.x != self.expected_x {
                    return Err("Draft disabled already-saved snapping".into());
                }
                settings.invoke_action("cancel".into());
                settings.invoke_change("pins.snap".into(), "false".into());
                settings.invoke_action("save".into());
                position(&pin, self.expected_x + 1, self.expected_y, false)?;
            }
            4 => {
                if state(&pin)?.x != self.expected_x + 1
                    || pin.get_snap_enabled()
                    || other.get_snap_enabled()
                {
                    return Err("Saved snap disable did not affect both windows".into());
                }
                settings.invoke_change("pins.snap".into(), "true".into());
                settings.invoke_action("save".into());
                pin.set_locked(true);
                position(&pin, self.expected_x + 1, self.expected_y, false)?;
            }
            5 => {
                if state(&pin)?.x != self.expected_x + 1 {
                    return Err("Locked pin was snapped".into());
                }
                pin.set_locked(false);
                let target = state(&other)?;
                let current = state(&pin)?;
                self.expected_x = target.x - current.width;
                self.expected_y = target.y + 35;
                position(&pin, self.expected_x - 1, self.expected_y, false)?;
            }
            6 => {
                let actual = state(&pin)?;
                if actual.x != self.expected_x {
                    return Err(format!(
                        "Visible window edge snap failed: {} != {}",
                        actual.x, self.expected_x
                    ));
                }
                let target = state(&other)?;
                if (target.x, target.y) != self.second_position {
                    return Err("Snap moved the target window".into());
                }
                shot(&pin, &directory.join("pin-snap.png"))?;
                pin.set_locked(true);
                self.foreground = glint_platform::foreground_window_id();
                pin.set_passthrough(true);
                pin.set_busy(true);
                pins::native(&pin, |w| w.configure(false, true))?;
                pin.hide().map_err(|e| e.to_string())?;
                position(&pin, 30000, 30000, true)?;
                pins::native(&pin, |w| w.diagnostic_geometry_event(true))?;
            }
            7 => {
                let actual = state(&pin)?;
                let work = pins::native(&pin, |w| w.work_area())?;
                if actual.x < work.0
                    || actual.y < work.1
                    || actual.x >= work.0 + work.2 as i32
                    || actual.y >= work.1 + work.3 as i32
                    || actual.visible
                    || actual.topmost
                    || !actual.click_through
                    || !actual.no_activate
                    || !pin.get_locked()
                    || !pin.get_passthrough()
                    || (pin.get_alpha() * 100.).round() as i32 != 55
                    || !pin.get_geometry_pending()
                    || glint_platform::foreground_window_id() != self.foreground
                {
                    return Err(format!(
                        "Display recovery failed: {actual:?}; work={work:?}; locked={} through={} alpha={} pending={} busy={}",
                        pin.get_locked(),
                        pin.get_passthrough(),
                        pin.get_alpha(),
                        pin.get_geometry_pending(),
                        pin.get_busy()
                    ));
                }
                pin.set_busy(false);
            }
            8 => {
                if pin.get_geometry_pending() || state(&pin)?.visible {
                    return Err(
                        "Deferred geometry recovery did not finish or showed hidden pin".into(),
                    );
                }
                pin.show().map_err(|e| e.to_string())?;
                pins::native(&pin, |w| w.configure(false, false))?;
                pin.set_passthrough(false);
                shot(&pin, &directory.join("pin-display-recovered.png"))?;
                pins::native(&pin, |w| w.diagnostic_geometry_event(true))?;
                manager.borrow_mut().close(self.id)?;
            }
            _ => return Err("Unknown pin geometry stage".into()),
        }
        Ok(true)
    }
}
