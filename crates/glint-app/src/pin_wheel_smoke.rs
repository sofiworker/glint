//! Framework wheel routing, saved preferences, and reservation failure recovery.
use crate::{pins, resident::AppState, settings::SettingsWindow};
use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
};
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
    started: Option<Instant>,
    old_mode: bool,
    used: u64,
}
fn wheel(pin: &pins::DesktopPin, delta: f32, control: bool) {
    if control {
        pin.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Control.into(),
        });
    }
    pin.window().dispatch_event(WindowEvent::PointerScrolled {
        position: slint::LogicalPosition::new(100., 85.),
        delta_x: 0.,
        delta_y: delta,
    });
    if control {
        pin.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Control.into(),
        });
    }
}
fn check(pin: &pins::DesktopPin, zoom: i32, opacity: i32, reason: &str) -> Result<(), String> {
    if pin.get_zoom() != zoom || (pin.get_alpha() * 100.).round() as i32 != opacity {
        return Err(format!(
            "{reason}: expected {zoom}/{opacity}, got {}/{}",
            pin.get_zoom(),
            pin.get_alpha() * 100.
        ));
    }
    Ok(())
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
            self.old_mode = app.borrow().session.preferences.saved().pins.wheel
                == glint_core::config::PinWheel::Opacity;
            settings.invoke_change("pins.wheel".into(), "zoom".into());
            settings.invoke_action("save".into());
            let mut config = app.borrow().session.preferences.saved().clone();
            config.pins.passthrough = false;
            config.pins.zoom_percent = 100;
            config.pins.opacity = 50;
            let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                &[99u8, 141, 183, 255].repeat(400 * 300),
                400,
                300,
            );
            self.id = pins::Manager::create(
                &manager,
                slint::Image::from_rgba8(pixels.clone()),
                (120, 150),
                &config,
                true,
                |_| {},
            )?;
            self.second = pins::Manager::create(
                &manager,
                slint::Image::from_rgba8(pixels),
                (800, 150),
                &config,
                true,
                |_| {},
            )?;
            self.started = Some(Instant::now());
            return Ok(true);
        }
        let pin = manager
            .borrow()
            .window(self.id)
            .ok_or("Wheel fixture closed")?
            .clone_strong();
        let other = manager
            .borrow()
            .window(self.second)
            .ok_or("Other wheel fixture closed")?
            .clone_strong();
        match stage {
            1 => {
                if !pin.get_ready() || !other.get_ready() {
                    if self
                        .started
                        .is_some_and(|s| s.elapsed() > Duration::from_secs(15))
                    {
                        return Err("Wheel fixtures not ready".into());
                    }
                    return Ok(false);
                }
                wheel(&pin, 20., false);
                wheel(&pin, 20., false);
                check(&pin, 100, 50, "Fine wheel must wait")?;
                wheel(&other, 20., false);
                check(&other, 100, 50, "Independent remainder")?;
                wheel(&pin, 20., false);
                check(&pin, 125, 50, "Fine wheel accumulated")?;
                wheel(&pin, -60., false);
                check(&pin, 100, 50, "Reverse wheel")?;
                wheel(&pin, 60., true);
                check(&pin, 100, 55, "Ctrl modifier routing")?;
                wheel(&pin, -600., true);
                check(&pin, 100, 20, "Opacity lower bound")?;
                wheel(&pin, 600., true);
                wheel(&pin, 600., true);
                check(&pin, 100, 100, "Opacity upper bound")?;
                wheel(&pin, 600., false);
                check(&pin, 200, 100, "Zoom upper bound")?;
                wheel(&pin, -600., false);
                check(&pin, 1, 100, "Zoom lower bound")?;
                pin.set_locked(true);
                wheel(&pin, 60., false);
                check(&pin, 26, 100, "Position lock preserves wheel")?;
                pin.invoke_action("larger".into());
                pin.invoke_action("larger".into());
                pin.invoke_action("larger".into());
                check(&pin, 101, 100, "Button and wheel share zoom")?;
            }
            2 => {
                settings.invoke_change("pins.wheel".into(), "opacity".into());
                wheel(&pin, -60., false);
                check(&pin, 76, 100, "Draft must not affect existing pin")?;
                settings.invoke_action("cancel".into());
                wheel(&pin, 60., false);
                check(&pin, 101, 100, "Cancelled mapping")?;
                settings.invoke_change("pins.wheel".into(), "opacity".into());
                settings.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty()
                    || !pin.get_wheel_opacity()
                    || !other.get_wheel_opacity()
                {
                    return Err("Saved mapping did not update both pins".into());
                }
            }
            3 => {
                wheel(&pin, -60., false);
                check(&pin, 101, 95, "Reverse mapping opacity")?;
                wheel(&pin, -60., true);
                check(&pin, 76, 95, "Reverse mapping Ctrl zoom")?;
                check(&other, 100, 50, "Other pin unchanged")?;
                pin.set_busy(true);
                wheel(&pin, 60., false);
                check(&pin, 76, 95, "Busy wheel ignored")?;
                pin.set_busy(false);
                pin.set_message_open(true);
                wheel(&pin, 60., true);
                check(&pin, 76, 95, "Message scrolling stays separate")?;
                pin.set_message_open(false);
                pin.set_confirm_pass(true);
                wheel(&pin, 60., true);
                check(&pin, 76, 95, "Confirmation blocks image wheel")?;
                pin.set_confirm_pass(false);
                pin.set_menu_open(true);
                wheel(&pin, -600., false);
                check(&pin, 76, 95, "Menu scrolling stays separate")?;
                pin.set_menu_open(false);
                pin.window().dispatch_event(WindowEvent::PointerScrolled {
                    position: slint::LogicalPosition::new(100., 85.),
                    delta_x: 120.,
                    delta_y: 0.,
                });
                check(&pin, 76, 95, "Horizontal scroll ignored")?;
                self.used = manager.borrow().used();
                manager.borrow_mut().set_budget(0);
                wheel(&pin, 60., true);
                check(&pin, 76, 95, "Budget rejection preserves zoom")?;
                if manager.borrow().used() != self.used || !pin.get_status().contains("limit") {
                    return Err("Wheel budget rejection changed charge or hid failure".into());
                }
                let position = slint::LogicalPosition::new(
                    90.,
                    pin.window().size().height as f32 / pin.window().scale_factor() - 58.,
                );
                pin.window().dispatch_event(WindowEvent::PointerPressed {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                });
                pin.window().dispatch_event(WindowEvent::PointerReleased {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                });
                if !pin.get_message_open() || pin.get_details().is_empty() || pin.get_output_error()
                {
                    return Err("Wheel failure details inaccessible or offered output retry".into());
                }
                let snapshot = pin.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    directory.join("pin-wheel-budget.png"),
                    snapshot.as_bytes(),
                    snapshot.width(),
                    snapshot.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                pin.set_message_open(false);
            }
            4 => {
                wheel(&pin, -60., true);
                check(&pin, 51, 95, "Over-budget shrink allowed")?;
                wheel(&pin, 60., true);
                check(&pin, 51, 95, "Over-budget re-expansion rejected")?;
                manager.borrow_mut().set_budget(256);
                wheel(&pin, 60., true);
                check(&pin, 76, 95, "Budget restored retry")?;
                if pin.get_status().contains("limit") || !pin.get_details().is_empty() {
                    return Err("Successful wheel retry retained old budget error".into());
                }
                let snapshot = pin.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    directory.join("pin-wheel.png"),
                    snapshot.as_bytes(),
                    snapshot.width(),
                    snapshot.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                pin.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            5 => {
                if pin.get_busy() {
                    if self
                        .started
                        .is_some_and(|s| s.elapsed() > Duration::from_secs(15))
                    {
                        return Err("Wheel source copy pending".into());
                    }
                    return Ok(false);
                }
                let image = glint_platform::read_clipboard_image()?;
                if image.width != 400
                    || image.height != 300
                    || image.rgba != [99u8, 141, 183, 255].repeat(400 * 300)
                {
                    return Err("Wheel changed copied source pixels".into());
                }
                settings.invoke_change(
                    "pins.wheel".into(),
                    if self.old_mode { "opacity" } else { "zoom" }.into(),
                );
                settings.invoke_action("save".into());
                manager.borrow_mut().close(self.id)?;
                manager.borrow_mut().close(self.second)?;
                if manager.borrow().used() != 0 || manager.borrow().count() != 0 {
                    return Err("Wheel fixtures retained reservations".into());
                }
            }
            _ => return Err("Unknown wheel stage".into()),
        }
        Ok(true)
    }
}
