//! Lossless source rotation, real rendered quadrants, clipboard and PNG verification.
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
    started: Option<Instant>,
    used: u64,
}
fn source() -> image::RgbaImage {
    image::RgbaImage::from_fn(480, 240, |x, y| {
        image::Rgba(match (x < 240, y < 120) {
            (true, true) => [241, 12, 22, 255],
            (false, true) => [13, 221, 32, 255],
            (true, false) => [24, 42, 231, 255],
            _ => [243, 213, 14, 255],
        })
    })
}
fn expected(turns: i32) -> image::RgbaImage {
    let source = source();
    match turns {
        1 => image::imageops::rotate90(&source),
        2 => image::imageops::rotate180(&source),
        3 => image::imageops::rotate270(&source),
        _ => source,
    }
}
fn check(pin: &pins::DesktopPin, turns: i32, path: &Path) -> Result<(), String> {
    if pin.get_quarter_turns() != turns {
        return Err("Pin quarter turn did not commit".into());
    }
    let expected = expected(turns);
    let pixels = pin
        .get_picture()
        .to_rgba8()
        .ok_or("Missing rotated preview")?;
    if (pixels.width(), pixels.height()) != expected.dimensions()
        || pixels.as_bytes() != expected.as_raw()
    {
        return Err("Rotated preview differs from independent imageops reference".into());
    }
    let screenshot = pin.window().take_snapshot().map_err(|e| e.to_string())?;
    let x0 = (screenshot.width() as f32 - pin.get_picture_wide()) / 2.;
    let y0 = 36. + (screenshot.height() as f32 - 112. - pin.get_picture_high()) / 2.;
    for (fx, fy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
        let x = (x0 + pin.get_picture_wide() * fx) as usize;
        let y = (y0 + pin.get_picture_high() * fy) as usize;
        let offset = (y * screenshot.width() as usize + x) * 4;
        let wanted = expected.get_pixel(
            (expected.width() as f32 * fx) as u32,
            (expected.height() as f32 * fy) as u32,
        );
        if screenshot.as_bytes()[offset..offset + 4] != wanted.0 {
            return Err(format!(
                "Software-rendered rotation quadrant mismatch at {x},{y}"
            ));
        }
    }
    image::save_buffer_with_format(
        path,
        screenshot.as_bytes(),
        screenshot.width(),
        screenshot.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())
}
impl Run {
    fn wait(&self) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(15))
        {
            Err("Pin rotation timed out".into())
        } else {
            Ok(false)
        }
    }
    pub fn step(
        &mut self,
        stage: u8,
        _ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let manager = app.borrow().pins.clone();
        if stage == 0 {
            let mut config = app.borrow().session.preferences.saved().clone();
            config.pins.opacity = 100;
            config.pins.zoom_percent = 100;
            config.pins.passthrough = false;
            let image = source();
            let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                image.as_raw(),
                480,
                240,
            );
            let document = glint_core::ImageDocument::from_selection(
                image.as_raw(),
                480,
                240,
                glint_core::Rect {
                    x: 0,
                    y: 0,
                    width: 480,
                    height: 240,
                },
                &glint_core::Edits::default(),
            )
            .map_err(str::to_owned)?;
            self.id = pins::Manager::create_document(
                &manager,
                crate::probe::PinPayload {
                    document: std::sync::Arc::new(document),
                    pixels: pixels.clone(),
                    display: pixels.clone(),
                    turns: 0,
                },
                (180, 180),
                &config,
                true,
                |_| {},
            )?;
            self.second = pins::Manager::create(
                &manager,
                slint::Image::from_rgba8(pixels),
                (850, 180),
                &config,
                true,
                |_| {},
            )?;
            self.started = Some(Instant::now());
            return Ok(true);
        }
        if stage == 10 {
            if manager.borrow().used() != 0 || manager.borrow().count() != 0 {
                return Err("Rotation close leaked pin reservation".into());
            }
            let Some(slot) = crate::probe::OutputSlot::acquire() else {
                return self.wait();
            };
            drop(slot);
            return Ok(true);
        }
        let pin = manager
            .borrow()
            .window(self.id)
            .ok_or("Rotation fixture closed")?
            .clone_strong();
        if !pin.get_ready() || pin.get_busy() {
            return self.wait();
        }
        match stage {
            1 => {
                if !manager.borrow().window(self.second).unwrap().get_ready() {
                    return self.wait();
                }
                self.used = manager.borrow().used();
                let slot =
                    crate::probe::OutputSlot::acquire().ok_or("Output slot already occupied")?;
                pin.invoke_action("rotate".into());
                if pin.get_busy()
                    || pin.get_quarter_turns() != 0
                    || manager.borrow().used() != self.used
                {
                    return Err("Busy output changed pin orientation or budget".into());
                }
                drop(slot);
                manager.borrow_mut().set_budget(0);
                pin.invoke_action("rotate".into());
                if pin.get_busy()
                    || pin.get_quarter_turns() != 0
                    || manager.borrow().used() != self.used
                {
                    return Err("Rejected rotation changed original pin".into());
                }
                manager.borrow_mut().set_budget(64);
                pin.invoke_action("menu".into());
            }
            2 => {
                pin.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(150., 150.),
                        delta_x: 0.,
                        delta_y: -10_000.,
                    });
            }
            3 => {
                let shot = pin.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    directory.join("pin-rotation-menu.png"),
                    shot.as_bytes(),
                    shot.width(),
                    shot.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                let pos = slint::LogicalPosition::new(140., 108.);
                pin.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                        position: pos,
                        button: slint::platform::PointerEventButton::Left,
                    });
                pin.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                        position: pos,
                        button: slint::platform::PointerEventButton::Left,
                    });
                if !pin.get_busy() {
                    return Err("Scrolled rotate menu pointer did not start worker".into());
                }
                pin.invoke_action("rotate".into()); // A second request cannot start another worker.
                pin.invoke_action("larger".into());
                self.started = Some(Instant::now());
            }
            4 => {
                check(&pin, 1, &directory.join("pin-rotation-90.png"))?;
                let document = manager
                    .borrow()
                    .document(self.id)
                    .ok_or("Rotation dropped editable source")?;
                if document.dimensions() != (480, 240) || document.source() != source().as_raw() {
                    return Err("Pin rotation changed original document direction or pixels".into());
                }
                if manager
                    .borrow()
                    .window(self.second)
                    .unwrap()
                    .get_quarter_turns()
                    != 0
                {
                    return Err("Rotation changed a different pin".into());
                }
                pin.invoke_change_opacity(33.);
                pin.invoke_action("smaller".into());
                pin.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            5 => {
                let copied = glint_platform::read_clipboard_image()?;
                let wanted = expected(1);
                if (copied.width, copied.height) != wanted.dimensions()
                    || copied.rgba != *wanted.as_raw()
                {
                    return Err(
                        "Rotated clipboard includes window scale/alpha or wrong pixels".into(),
                    );
                }
                manager
                    .borrow_mut()
                    .diagnostic_choice(Some(directory.join("pin-rotation-90-output.png")));
                pin.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            6 => {
                let saved = image::open(directory.join("pin-rotation-90-output.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if saved != expected(1) {
                    return Err("Rotated PNG source pixels differ".into());
                }
                pin.invoke_change_opacity(100.);
                pin.invoke_action("rotate".into());
                self.started = Some(Instant::now());
            }
            7 | 8 => {
                let turns = i32::from(stage) - 5;
                check(
                    &pin,
                    turns,
                    &directory.join(format!("pin-rotation-{}.png", turns * 90)),
                )?;
                pin.invoke_action("rotate".into());
                self.started = Some(Instant::now());
            }
            9 => {
                check(&pin, 0, &directory.join("pin-rotation-360.png"))?;
                pin.invoke_action("rotate".into());
                pin.invoke_action("close".into()); // Close with a worker still pending.
                manager.borrow_mut().close(self.second)?;
                self.started = Some(Instant::now());
            }
            _ => return Err("Unexpected rotation stage".into()),
        }
        Ok(true)
    }
}
