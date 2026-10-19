//! Scale UI input/rendering while independently checking source-pixel output.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_core::{
    Edits, Rect, Redaction,
    config::{Config, Theme},
};
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Run {
    compact_scrolled: bool,
    old: Option<Config>,
    output: Option<PathBuf>,
    pins: Vec<u64>,
    new_pin: Option<u64>,
    expected: Vec<u8>,
    started: Option<Instant>,
    size: Option<slint::PhysicalSize>,
    capture_theme: u8,
    pin_theme: u8,
    countdown_theme: u8,
    pin_geometry: String,
}
// Feed ThemeChanged into the same queued adapter used by the winit filter.
// This does not alter the machine's registry theme or claim an actual OS toggle.
fn theme_cycle<T: ComponentHandle + 'static>(
    ui: &T,
    phase: &mut u8,
    get: fn(&T) -> bool,
    apply: fn(&T, bool),
    dir: &Path,
    prefix: &str,
) -> Result<bool, String> {
    use slint::winit_030::winit::{event::WindowEvent, window::Theme as NativeTheme};
    let send = |preference, dark| {
        crate::interface_scale::theme_event(
            &ui.as_weak(),
            preference,
            &WindowEvent::ThemeChanged(if dark {
                NativeTheme::Dark
            } else {
                NativeTheme::Light
            }),
            apply,
        )
    };
    match *phase {
        0 => send(Theme::System, true),
        1 => {
            if !get(ui) {
                return Err(format!("{prefix} did not follow dark event"));
            }
            shot(ui, &dir.join(format!("theme-{prefix}-dark.png")))?;
            send(Theme::System, false);
        }
        2 => {
            if get(ui) {
                return Err(format!("{prefix} did not follow light event"));
            }
            shot(ui, &dir.join(format!("theme-{prefix}-light.png")))?;
            send(Theme::Light, true);
            send(Theme::Dark, true);
        }
        3 => {
            if get(ui) {
                return Err(format!("{prefix} explicit policy followed an event"));
            }
            eprintln!(
                "Theme event adapter: {prefix} follows queued dark/light events; explicit policies ignore them"
            );
            return Ok(true);
        }
        _ => unreachable!(),
    }
    *phase += 1;
    Ok(false)
}
fn scale(window: &slint::Window, expected: f32) -> Result<(), String> {
    if (window.scale_factor() - expected).abs() > 0.01 {
        return Err(format!(
            "Scale {} differs from {expected}",
            window.scale_factor()
        ));
    }
    Ok(())
}
fn shot(ui: &impl ComponentHandle, path: &Path) -> Result<(), String> {
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
fn drag(c: &crate::probe::CaptureProbe, a: (f32, f32), b: (f32, f32)) -> Result<(), String> {
    use slint::platform::{PointerEventButton, WindowEvent};
    // Dispatch through Slint hit-testing, in logical scene coordinates. Source points
    // land in pixel centers to avoid float rounding at integer boundaries.
    let s = c.window().scale_factor();
    let p = |v: (f32, f32)| slint::LogicalPosition::new((v.0 + 0.5) / s, (v.1 + 0.5) / s);
    for e in [
        WindowEvent::PointerMoved { position: p(a) },
        WindowEvent::PointerPressed {
            position: p(a),
            button: PointerEventButton::Left,
        },
        WindowEvent::PointerMoved { position: p(b) },
        WindowEvent::PointerReleased {
            position: p(b),
            button: PointerEventButton::Left,
        },
    ] {
        c.window()
            .dispatch_event_with_result(e)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
impl Run {
    fn begin(&mut self, ui: &SettingsWindow, app: &Rc<RefCell<AppState>>, dir: &Path, name: &str) {
        app.borrow_mut().capture_output = Some(dir.join(name));
        ui.invoke_action("capture".into());
        self.started = Some(Instant::now());
    }
    fn select(&self, c: &crate::probe::CaptureProbe, s: f32) -> Result<(), String> {
        scale(c.window(), s)?;
        let source = c.get_desktop().to_rgba8().ok_or("Source missing")?;
        if c.window().size() != slint::PhysicalSize::new(source.width(), source.height()) {
            return Err("UI scale changed frozen physical extent".into());
        }
        drag(c, (200., 200.), (600., 500.))?;
        if (c.get_left(), c.get_top(), c.get_wide(), c.get_high()) != (200., 200., 400., 300.) {
            return Err(format!(
                "Scaled pointer produced wrong source selection: {},{},{},{}",
                c.get_left(),
                c.get_top(),
                c.get_wide(),
                c.get_high()
            ));
        }
        Ok(())
    }
    fn expect(&mut self, c: &crate::probe::CaptureProbe, edits: &Edits) -> Result<(), String> {
        let source = c.get_desktop().to_rgba8().ok_or("Source missing")?;
        self.expected = glint_core::composite_selection(
            source.as_bytes(),
            source.width(),
            source.height(),
            Rect {
                x: 200,
                y: 200,
                width: 400,
                height: 300,
            },
            edits,
        )
        .map_err(str::to_owned)?;
        Ok(())
    }
    fn verify(&self, dir: &Path, name: &str) -> Result<(), String> {
        let p = image::open(dir.join(name))
            .map_err(|e| e.to_string())?
            .to_rgba8();
        if p.dimensions() != (400, 300) || p.as_raw() != &self.expected {
            return Err("Scaled UI changed exported pixels".into());
        }
        Ok(())
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(30))
        {
            return Err(format!("Interface scale stage{stage} timed out"));
        }
        if (app.borrow().capturing && stage != 8)
            || app.borrow().capture.as_ref().is_some_and(|c| c.get_busy())
        {
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                self.pins = app.borrow().pins.borrow().ids();
                self.size = Some(ui.window().size());
                ui.invoke_action("open".into());
                ui.invoke_choose_page(8);
                if !ui
                    .get_rows()
                    .iter()
                    .any(|r| r.key == "appearance.scale" && r.editable)
                {
                    return Err("Scale field disabled".into());
                }
                ui.invoke_change("appearance.scale".into(), "percent125".into());
                self.started = Some(Instant::now());
            }
            1 => {
                scale(ui.window(), 1.25)?;
                shot(ui, &dir.join("scale-settings-125.png"))?;
                if app.borrow().session.preferences.saved() != self.old.as_ref().unwrap() {
                    return Err("Scale preview saved configuration".into());
                }
                ui.invoke_action("cancel".into());
                let original =
                    crate::interface_scale::factor(self.old.as_ref().unwrap().appearance.scale);
                let expected = if original > 0. {
                    original
                } else {
                    ui.window()
                        .with_winit_window(|w| w.scale_factor() as f32)
                        .ok_or("Settings peer missing")?
                };
                scale(ui.window(), expected)?;
                for (k, v) in [
                    ("general.language", "en"),
                    ("appearance.scale", "percent150"),
                    ("appearance.theme", "system"),
                    ("capture.delay_seconds", "0"),
                    ("capture.detect_window", "false"),
                    ("capture.magnifier", "false"),
                    ("annotation.tool", "select"),
                    ("output.feedback", "toast"),
                    ("output.format", "png"),
                ] {
                    ui.invoke_change(k.into(), v.into());
                }
                ui.invoke_action("save".into());
                let loaded = glint_core::config::Config::from_toml(
                    &std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if loaded.appearance.scale != glint_core::config::Scale::Percent150 {
                    return Err("Scale did not persist".into());
                }
                self.begin(ui, app, dir, "scale150.png");
            }
            2 => {
                let Some(c) = app
                    .borrow()
                    .capture
                    .as_ref()
                    .map(ComponentHandle::clone_strong)
                else {
                    return Ok(false);
                };
                if (c.window().scale_factor() - 1.5).abs() > 0.01 {
                    return Ok(false);
                }
                if self.capture_theme == 0 {
                    self.select(&c, 1.5)?;
                    c.set_tool(1);
                    drag(&c, (300., 300.), (400., 380.))?;
                    if c.get_object_count() != 1 {
                        return Err("Scaled redaction pointer missed canvas".into());
                    }
                    c.invoke_action("undo".into());
                    if c.get_object_count() != 0 {
                        return Err("Scaled undo failed".into());
                    }
                    c.invoke_action("redo".into());
                    c.set_tool(8);
                    c.invoke_pointer(0, 250., 250.);
                    c.set_text_value("Theme draft".into());
                }
                if c.get_object_count() != 1
                    || !c.get_text_editing()
                    || c.get_text_value() != "Theme draft"
                {
                    return Err("Theme event lost annotations or active text draft".into());
                }
                if !theme_cycle(
                    &c,
                    &mut self.capture_theme,
                    |c| c.get_dark(),
                    crate::probe::apply_system_theme,
                    dir,
                    "capture",
                )? {
                    return Ok(false);
                }
                c.invoke_action("cancel-text".into());
                c.set_tool(1);
                let mut edits = Edits::default();
                edits.push(Redaction {
                    bounds: Rect {
                        x: 100,
                        y: 100,
                        width: 100,
                        height: 80,
                    },
                    color: [32, 43, 66, 255],
                });
                self.expect(&c, &edits)?;
                c.invoke_canvas_left();
                shot(&c, &dir.join("scale-capture-150.png"))?;
                c.invoke_action("save".into());
            }
            3 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                self.verify(dir, "scale150.png")?;
                let Some(p) = crate::completion::popup() else {
                    return Ok(false);
                };
                scale(p.window(), 1.5)?;
                shot(&p, &dir.join("scale-toast-150.png"))?;
                self.begin(ui, app, dir, "unused-pin-scale.png");
            }
            4 => {
                let Some(c) = app
                    .borrow()
                    .capture
                    .as_ref()
                    .map(ComponentHandle::clone_strong)
                else {
                    return Ok(false);
                };
                if (c.window().scale_factor() - 1.5).abs() > 0.01 {
                    return Ok(false);
                }
                self.select(&c, 1.5)?;
                c.invoke_action("pin".into());
            }
            5 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                let manager = app.borrow().pins.clone();
                let id = manager
                    .borrow()
                    .ids()
                    .into_iter()
                    .find(|id| !self.pins.contains(id))
                    .ok_or("Scaled pin missing")?;
                let pin = manager
                    .borrow()
                    .window(id)
                    .ok_or("Scaled pin closed")?
                    .clone_strong();
                if !pin.get_ready() {
                    return Ok(false);
                }
                scale(pin.window(), 1.5)?;
                if pin.get_zoom() != 100
                    || (pin.get_picture_wide() * 1.5 - 400.).abs() > 1.
                    || (pin.get_picture_high() * 1.5 - 300.).abs() > 1.
                {
                    return Err("UI scale changed pin image zoom".into());
                }
                let geometry = format!(
                    "{:?}/{:?}/{}/{}/{}/{}/{}/{}",
                    pin.window().position(),
                    pin.window().size(),
                    pin.get_zoom(),
                    pin.get_alpha(),
                    pin.get_locked(),
                    pin.get_on_top(),
                    pin.get_quarter_turns(),
                    pin.get_passthrough()
                );
                if self.pin_theme == 0 {
                    self.pin_geometry = geometry.clone();
                }
                if geometry != self.pin_geometry {
                    return Err("Theme event moved/resized/changed pin state".into());
                }
                if !theme_cycle(
                    &pin,
                    &mut self.pin_theme,
                    |p| p.get_dark(),
                    crate::pins::apply_system_theme,
                    dir,
                    "pin",
                )? {
                    return Ok(false);
                }
                shot(&pin, &dir.join("scale-pin-150.png"))?;
                self.new_pin = Some(id);
                ui.invoke_action("open".into());
                ui.invoke_change("appearance.scale".into(), "percent175".into());
                ui.invoke_action("save".into());
                shot(ui, &dir.join("scale-settings-175.png"))?;
                self.begin(ui, app, dir, "scale175.png");
            }
            6 => {
                let Some(c) = app
                    .borrow()
                    .capture
                    .as_ref()
                    .map(ComponentHandle::clone_strong)
                else {
                    return Ok(false);
                };
                if (c.window().scale_factor() - 1.75).abs() > 0.01 {
                    return Ok(false);
                }
                self.select(&c, 1.75)?;
                self.expect(&c, &Edits::default())?;
                c.invoke_action("save".into());
            }
            7 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                self.verify(dir, "scale175.png")?;
                let manager = app.borrow().pins.clone();
                scale(
                    manager
                        .borrow()
                        .window(self.new_pin.unwrap())
                        .ok_or("Pin missing")?
                        .window(),
                    1.5,
                )?;
                let Some(p) = crate::completion::popup() else {
                    return Ok(false);
                };
                scale(p.window(), 1.75)?;
                ui.invoke_action("open".into());
                ui.invoke_change("capture.delay_seconds".into(), "2".into());
                ui.invoke_action("save".into());
                self.begin(ui, app, dir, "unused-scale-delay.png");
            }
            8 => {
                {
                    let state = app.borrow();
                    let countdown = state.countdown.as_ref().ok_or("Scaled countdown missing")?;
                    if (countdown.ui.window().scale_factor() - 1.75).abs() > 0.01 {
                        return Ok(false);
                    }
                    if !theme_cycle(
                        &countdown.ui,
                        &mut self.countdown_theme,
                        |c| c.get_dark(),
                        crate::capture_delay::apply_system_theme,
                        dir,
                        "countdown",
                    )? {
                        return Ok(false);
                    }
                    shot(&countdown.ui, &dir.join("scale-countdown-175.png"))?;
                }
                ui.invoke_action("delay-cancel".into());
                ui.invoke_action("open".into());
                ui.window().set_size(slint::PhysicalSize::new(1138, 700));
            }
            9 => {
                scale(ui.window(), 1.75)?;
                if !self.compact_scrolled {
                    shot(ui, &dir.join("scale-settings-compact-175.png"))?;
                    ui.window()
                        .dispatch_event_with_result(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(400., 200.),
                            delta_x: 0.,
                            delta_y: -200.,
                        })
                        .map_err(|e| e.to_string())?;
                    self.compact_scrolled = true;
                    return Ok(false);
                }
                shot(ui, &dir.join("scale-settings-compact-scrolled-175.png"))?;
                ui.invoke_action("config-export".into());
                if ui.get_modal() != 4 {
                    return Err("Compact configuration preview missing".into());
                }
                shot(ui, &dir.join("scale-transfer-compact-175.png"))?;
                let p = slint::LogicalPosition::new(
                    100.,
                    ui.window().size().height as f32 / ui.window().scale_factor() - 75.,
                );
                for e in [
                    slint::platform::WindowEvent::PointerMoved { position: p },
                    slint::platform::WindowEvent::PointerPressed {
                        position: p,
                        button: slint::platform::PointerEventButton::Left,
                    },
                    slint::platform::WindowEvent::PointerReleased {
                        position: p,
                        button: slint::platform::PointerEventButton::Left,
                    },
                ] {
                    ui.window()
                        .dispatch_event_with_result(e)
                        .map_err(|e| e.to_string())?;
                }
                if ui.get_modal() != 0 {
                    return Err("Scaled compact preview cancel missed its button".into());
                }
                ui.invoke_change("appearance.scale".into(), "system".into());
                ui.invoke_action("save".into());
                let actual = ui
                    .window()
                    .with_winit_window(|w| w.scale_factor() as f32)
                    .ok_or("Settings peer missing")?;
                scale(ui.window(), actual)?;
                ui.invoke_change("appearance.scale".into(), "percent175".into());
                ui.invoke_action("reset".into());
                if ui.get_modal() != 2 {
                    return Err("Scale reset did not request confirmation".into());
                }
                scale(ui.window(), 1.75)?;
                ui.invoke_action("confirm-reset".into());
                scale(ui.window(), actual)?;

                if app.borrow().capture.is_some() || app.borrow().countdown.is_some() {
                    return Err("Scale delay cancel retained capture".into());
                }
                if let Some(id) = self.new_pin.take() {
                    app.borrow().pins.borrow_mut().close(id)?;
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                app.borrow_mut().capture_output = self.output.take();
                ui.invoke_action("open".into());
                ui.invoke_action("save".into());
                ui.invoke_action("close".into());
                if let Some(size) = self.size.take() {
                    ui.window().set_size(size);
                }
                crate::completion::dismiss();
                eprintln!(
                    "Interface scale: draft/cancel/reset/system/TOML, real Slint pointer mapping at150/175%, source-sized redaction undo/redo and exact PNG pixels, pin UI vs image zoom, existing pin isolation, completion/countdown and cancellation passed"
                );
            }
            _ => return Err("Unknown scale stage".into()),
        }
        Ok(true)
    }
}
