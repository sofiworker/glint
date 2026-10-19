//! Resident-owned desktop pins. All window operations run on the Slint UI thread.
use crate::messages::{self, Message};
use glint_core::{config::Config, pins::PinBudget};
use glint_platform::NativePinWindow;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet},
    rc::Rc,
};
type RecoveryProbe = Rc<dyn Fn() -> Result<Vec<glint_platform::HotkeyBinding>, String>>;

slint::slint! { export { Motion } from "src/widgets/motion.slint"; export { DesktopPin, PinLabels } from "src/pin.slint"; }
pub(crate) fn apply_system_theme(ui: &DesktopPin, dark: bool) {
    ui.set_dark(dark);
}
fn action_failed(ui: &DesktopPin, error: &str) {
    let detail = format!(
        "{}: {error}",
        messages::text(Message::PinActionFailed, ui.get_english())
    );
    ui.set_status(detail.clone().into());
    ui.set_details(detail.into());
    // An unrelated action failure must not retain a previous output retry.
    ui.set_output_error(false);
    ui.set_menu_open(false);
    ui.set_confirm_pass(false);
    ui.set_message_open(true);
}
struct ImageInput {
    picture: slint::Image,
    restored: Option<(SharedPixelBuffer<Rgba8Pixel>, i32)>,
}
struct Pin {
    events: Option<glint_platform::PinWindowEvents>,
    geometry_queue: Rc<Cell<u8>>,
    resize_failed: bool,
    wheel: glint_core::pins::WheelSteps,
    ui: DesktopPin,
    original: SharedPixelBuffer<Rgba8Pixel>,
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    rotation: Option<Rotation>,
    document: Option<std::sync::Arc<glint_core::ImageDocument>>,
}
impl Pin {
    fn extra_bytes(&self) -> u64 {
        self.document.as_ref().map_or(0, |d| d.retained_bytes())
    }
}

struct Rotation {
    layout: Layout,
    old_charge: u64,
    new_charge: u64,
    turns: i32,
}

struct Layout {
    size: (u32, u32),
    image: (f32, f32),
    zoom: i32,
}
#[derive(Default)]
pub struct Manager {
    edit_handler: Option<Rc<dyn Fn(u64)>>,
    recovery_probe: Option<RecoveryProbe>,
    recovery_binding: Option<glint_platform::HotkeyBinding>,
    recovery_shortcut: String,
    recovery_enabled: bool,
    tray_ready: bool,
    output_options: glint_core::config::Output,
    diagnostic_choice: crate::pin_output::DiagnosticChoice,
    entries: BTreeMap<u64, Pin>,
    budget: PinBudget,
    next_id: u64,
    capture_hidden: HashSet<u64>,
}
pub fn native<T>(
    ui: &DesktopPin,
    operation: impl FnOnce(NativePinWindow) -> Result<T, String>,
) -> Result<T, String> {
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Pin requires Win32".into());
            };
            operation(unsafe {
                NativePinWindow::from_raw(handle.hwnd.get() as *mut std::ffi::c_void)
            })
        })
        .ok_or_else(|| "Pin window unavailable".to_owned())?
}
impl Manager {
    pub fn set_snap(&mut self, enabled: bool) {
        for pin in self.entries.values() {
            pin.ui.set_snap_enabled(enabled);
        }
    }
    fn queue_geometry(
        owner: std::rc::Weak<RefCell<Self>>,
        id: u64,
        ui: slint::Weak<DesktopPin>,
        queue: Rc<Cell<u8>>,
        event: glint_platform::PinGeometryEvent,
    ) {
        let previous = queue.get();
        queue.set(
            previous
                | if event == glint_platform::PinGeometryEvent::DisplayChanged {
                    2
                } else {
                    1
                },
        );
        if previous != 0 {
            return;
        }
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let display_change = queue.replace(0) & 2 != 0;
            if let (Some(owner), Some(ui)) = (owner.upgrade(), ui.upgrade()) {
                if !owner.borrow().entries.contains_key(&id) {
                    return;
                }
                let result = owner.borrow_mut().geometry(id, display_change);
                if let Err(error) = result {
                    action_failed(&ui, &error);
                }
            }
        });
    }
    fn geometry(&mut self, id: u64, display_change: bool) -> Result<(), String> {
        use glint_core::pin_geometry::{Bounds, recover_position, snap_position};
        let pin = self.entries.get(&id).ok_or("Pin closed")?;
        if !pin.ui.get_ready() {
            return Ok(());
        }
        let bounds = |s: glint_platform::PinWindowState| Bounds {
            left: s.x,
            top: s.y,
            right: s.x.saturating_add(s.width),
            bottom: s.y.saturating_add(s.height),
        };
        let work_bounds = |(x, y, w, h): (i32, i32, u32, u32)| Bounds {
            left: x,
            top: y,
            right: x.saturating_add(w.min(i32::MAX as u32) as i32),
            bottom: y.saturating_add(h.min(i32::MAX as u32) as i32),
        };
        if display_change {
            let busy = pin.ui.get_busy();
            let zoom = pin.ui.get_zoom();
            pin.ui.set_geometry_pending(busy);
            let resized = if busy { Ok(()) } else { self.resize(id, zoom) };
            let pin = self.entries.get(&id).ok_or("Pin closed")?;
            let position = native(&pin.ui, |w| {
                Ok((bounds(w.state()?), work_bounds(w.work_area()?)))
            })?;
            let (x, y) = recover_position(position.0, position.1).ok_or("Invalid pin work area")?;
            native(&pin.ui, |w| {
                // Winit may rewrite extended styles while hiding or changing
                // DPI. Restore the accepted pin state without showing it.
                w.configure(pin.ui.get_on_top(), pin.ui.get_passthrough())?;
                w.position(x, y)
            })?;
            resized
        } else {
            if !pin.ui.get_snap_enabled()
                || pin.ui.get_locked()
                || pin.ui.get_busy()
                || pin.ui.get_passthrough()
            {
                return Ok(());
            }
            let (current, work, targets) = native(&pin.ui, |w| {
                Ok((
                    bounds(w.state()?),
                    work_bounds(w.work_area()?),
                    w.snap_targets()?,
                ))
            })?;
            let targets: Vec<_> = targets
                .into_iter()
                .map(|w| Bounds {
                    left: w.left,
                    top: w.top,
                    right: w.right,
                    bottom: w.bottom,
                })
                .collect();
            let threshold = (12. * pin.ui.window().scale_factor()).round().max(1.) as i32;
            let (x, y) = snap_position(current, work, &targets, threshold)
                .ok_or("Invalid pin snap geometry")?;
            native(&pin.ui, |w| w.position(x, y))
        }
    }
    pub fn set_wheel(&mut self, mode: glint_core::config::PinWheel) {
        let opacity = mode == glint_core::config::PinWheel::Opacity;
        for pin in self.entries.values_mut() {
            if pin.ui.get_wheel_opacity() != opacity {
                pin.ui.set_wheel_opacity(opacity);
                pin.wheel = Default::default();
            }
        }
    }
    fn wheel(&mut self, id: u64, delta: f32, control: bool) -> Result<(), String> {
        let pin = self.entries.get_mut(&id).ok_or("Pin closed")?;
        if !pin.ui.get_ready()
            || pin.ui.get_busy()
            || pin.ui.get_menu_open()
            || pin.ui.get_message_open()
            || pin.ui.get_confirm_pass()
            || pin.ui.get_passthrough()
        {
            pin.wheel = Default::default();
            return Ok(());
        }
        let opacity = pin.ui.get_wheel_opacity() != control;
        let steps = pin.wheel.push(delta, opacity);
        if steps == 0 {
            return Ok(());
        }
        if opacity {
            pin.ui
                .set_alpha((pin.ui.get_alpha() + steps as f32 * 0.05).clamp(0.2, 1.));
            Ok(())
        } else {
            let zoom = pin.ui.get_zoom() + steps * 25;
            self.resize(id, zoom)
        }
    }
    pub fn set_edit_handler(&mut self, handler: Rc<dyn Fn(u64)>) {
        self.edit_handler = Some(handler);
    }
    pub fn edit_source(&self, id: u64) -> Result<crate::pin_edit::Source, String> {
        let pin = self.entries.get(&id).ok_or("Pin closed")?;
        if !pin.ui.get_ready() || pin.ui.get_busy() {
            return Err("Pin is busy".into());
        }
        Ok(crate::pin_edit::Source {
            pixels: pin.original.clone(),
            document: pin.document.clone(),
            work: native(&pin.ui, |w| w.work_area())?,
            scale: pin.ui.window().scale_factor(),
            turns: pin.ui.get_quarter_turns(),
        })
    }
    pub fn replace_document(
        &mut self,
        id: u64,
        payload: crate::probe::PinPayload,
    ) -> Result<(), String> {
        let pin = self.entries.get_mut(&id).ok_or("Original pin closed")?;
        if pin.ui.get_busy() {
            return Err("Original pin is busy".into());
        }
        if payload.document.dimensions() != (payload.pixels.width(), payload.pixels.height())
            || payload.turns != pin.ui.get_quarter_turns()
        {
            return Err("Pin document changed during editing".into());
        }
        let dimensions = payload.document.dimensions();
        let expected = if payload.turns % 2 == 1 {
            (dimensions.1, dimensions.0)
        } else {
            dimensions
        };
        if (payload.display.width(), payload.display.height()) != expected {
            return Err("Rotated pin dimensions do not match document".into());
        }
        let layout = Self::layout(
            &pin.ui,
            (payload.display.width(), payload.display.height()),
            pin.ui.get_zoom(),
        );
        let charge = glint_core::pins::charge(payload.document.dimensions(), layout.size)
            .and_then(|n| n.checked_add(payload.document.retained_bytes()))
            .ok_or("pins.budget")?;
        self.budget
            .reserve(id, charge)
            .map_err(|_| messages::text(Message::PinBudget, pin.ui.get_english()))?;
        pin.ui
            .set_picture(slint::Image::from_rgba8(payload.display.clone()));
        pin.original = payload.pixels;
        pin.pixels = payload.display;
        pin.document = Some(payload.document);
        Self::apply_layout(&pin.ui, &layout);
        pin.ui
            .set_status(messages::text(Message::PinHint, pin.ui.get_english()).into());
        pin.ui.set_details("".into());
        Ok(())
    }
    pub fn set_recovery_probe(&mut self, probe: RecoveryProbe) {
        self.recovery_probe = Some(probe);
        self.recovery_enabled = true;
    }
    pub fn set_tray_ready(&mut self, ready: bool) {
        self.tray_ready = ready;
    }
    fn recovery_available(&self) -> bool {
        self.tray_ready
            && self.recovery_enabled
            && self.recovery_binding.is_some_and(|expected| {
                self.recovery_probe
                    .as_ref()
                    .is_some_and(|probe| probe().is_ok_and(|bindings| bindings.contains(&expected)))
            })
    }
    pub fn configure_recovery(
        &mut self,
        shortcut: &str,
        binding: Option<glint_platform::HotkeyBinding>,
    ) {
        self.recovery_shortcut = shortcut.into();
        self.recovery_binding = binding;
        let available = self.recovery_available();
        for pin in self.entries.values() {
            pin.ui.set_rescue_shortcut(shortcut.into());
            pin.ui.set_rescue_ready(available);
            if !available {
                pin.ui.set_confirm_pass(false);
                if pin.ui.get_passthrough() {
                    match native(&pin.ui, |w| w.configure(pin.ui.get_on_top(), false)) {
                        Ok(()) => {
                            pin.ui.set_passthrough(false);
                            pin.ui.set_status(
                                messages::text(
                                    Message::PinRecoveryUnavailable,
                                    pin.ui.get_english(),
                                )
                                .into(),
                            );
                        }
                        Err(error) => pin.ui.set_status(error.into()),
                    }
                }
            }
        }
    }
    pub fn recovery_failed(&mut self, show_pins: bool) -> Result<(), String> {
        self.recovery_enabled = false;
        for pin in self.entries.values() {
            pin.ui.set_rescue_ready(false);
            pin.ui.set_confirm_pass(false);
        }
        if show_pins {
            return self.rescue_all();
        }
        let mut failure = None;
        for pin in self.entries.values() {
            match native(&pin.ui, |w| w.configure(pin.ui.get_on_top(), false)) {
                Ok(()) => pin.ui.set_passthrough(false),
                Err(error) => failure = Some(error),
            }
        }
        failure.map_or(Ok(()), Err)
    }
    pub fn recovery_repaired(&mut self) {
        self.recovery_enabled = true;
    }
    pub fn recovery_stopped(&self) -> bool {
        !self.recovery_enabled && self.recovery_probe.is_some()
    }
    fn unavailable(ui: &DesktopPin) {
        ui.set_confirm_pass(false);
        ui.set_rescue_ready(false);
        ui.set_output_error(false);
        ui.set_status(messages::text(Message::PinRecoveryUnavailable, ui.get_english()).into());
        ui.set_details(ui.get_status());
        ui.set_message_open(true);
    }
    pub fn set_output_options(&mut self, options: &glint_core::config::Output) {
        self.output_options = options.clone();
    }
    /// Explicit native-smoke picker replacement; normal UI always uses the system picker.
    pub fn diagnostic_choice(&mut self, choice: Option<std::path::PathBuf>) {
        self.diagnostic_choice = Some(choice);
    }
    pub fn count(&self) -> usize {
        self.entries.len()
    }
    pub fn used(&self) -> u64 {
        self.budget.used()
    }
    pub fn set_budget(&mut self, mib: u16) {
        self.budget.set_limit(u64::from(mib) * 1024 * 1024);
    }
    pub fn window(&self, id: u64) -> Option<&DesktopPin> {
        self.entries.get(&id).map(|p| &p.ui)
    }
    pub fn document(&self, id: u64) -> Option<std::sync::Arc<glint_core::ImageDocument>> {
        self.entries.get(&id).and_then(|p| p.document.clone())
    }
    pub fn ids(&self) -> Vec<u64> {
        self.entries.keys().copied().collect()
    }
    pub fn close(&mut self, id: u64) -> Result<(), String> {
        if let Some(pin) = self.entries.get(&id) {
            pin.ui.hide().map_err(|e| e.to_string())?;
        }
        self.entries.remove(&id);
        self.capture_hidden.remove(&id);
        self.budget.release(id);
        Ok(())
    }
    fn layout(ui: &DesktopPin, dimensions: (u32, u32), requested: i32) -> Layout {
        let scale = ui.window().scale_factor().max(1.);
        let monitor = native(ui, |w| w.work_area()).ok();
        let max_w = monitor.map_or(1600., |m| (m.2 as f32 - 40. * scale).max(320. * scale));
        let max_h = monitor.map_or(1000., |m| (m.3 as f32 - 40. * scale).max(220. * scale));
        let (w, h) = (dimensions.0 as f32, dimensions.1 as f32);
        let ratio = (requested.clamp(1, 200) as f32 / 100.)
            .min((max_w - 4. * scale) / w)
            .min((max_h - 108. * scale) / h)
            .max(0.001);
        let image_w = (w * ratio).round().max(1.);
        let image_h = (h * ratio).round().max(1.);
        let size = (
            (image_w + 4. * scale).max(320. * scale) as u32,
            (image_h + 108. * scale).max(220. * scale) as u32,
        );
        Layout {
            size,
            image: (image_w / scale, image_h / scale),
            zoom: (ratio * 100.).round().max(1.) as i32,
        }
    }
    fn apply_layout(ui: &DesktopPin, layout: &Layout) {
        ui.window()
            .set_size(slint::PhysicalSize::new(layout.size.0, layout.size.1));
        ui.set_picture_wide(layout.image.0);
        ui.set_picture_high(layout.image.1);
        ui.set_zoom(layout.zoom);
    }
    fn resize(&mut self, id: u64, requested: i32) -> Result<(), String> {
        let pin = self.entries.get_mut(&id).ok_or("Pin closed")?;
        if pin.ui.get_busy() {
            return Ok(());
        }
        let layout = Self::layout(
            &pin.ui,
            (pin.pixels.width(), pin.pixels.height()),
            requested,
        );
        let size = layout.size;
        let charge = glint_core::pins::charge((pin.pixels.width(), pin.pixels.height()), size)
            .and_then(|charge| charge.checked_add(pin.extra_bytes()))
            .ok_or("pins.budget")?;
        if self.budget.reserve(id, charge).is_err() {
            pin.resize_failed = true;
            return Err(messages::text(Message::PinBudget, pin.ui.get_english()).into());
        }
        Self::apply_layout(&pin.ui, &layout);
        if pin.resize_failed {
            pin.resize_failed = false;
            pin.ui
                .set_status(messages::text(Message::PinHint, pin.ui.get_english()).into());
            pin.ui.set_details("".into());
        }
        Ok(())
    }
    fn rotate(owner: &Rc<RefCell<Self>>, id: u64) -> Result<(), String> {
        let mut manager = owner.borrow_mut();
        let pin = manager.entries.get(&id).ok_or("Pin closed")?;
        if !pin.ui.get_ready() || pin.ui.get_busy() {
            return Ok(());
        }
        let slot = crate::probe::OutputSlot::acquire().ok_or_else(|| {
            messages::text(Message::CaptureOutputPending, pin.ui.get_english()).to_owned()
        })?;
        let turns = (pin.ui.get_quarter_turns() + 1) % 4;
        let source = pin.original.clone();
        let dimensions = if turns % 2 == 1 {
            (source.height(), source.width())
        } else {
            (source.width(), source.height())
        };
        let layout = Self::layout(&pin.ui, dimensions, pin.ui.get_zoom());
        let current_size = pin.ui.window().size();
        let old_charge = glint_core::pins::charge(
            (source.width(), source.height()),
            (current_size.width, current_size.height),
        )
        .and_then(|charge| charge.checked_add(pin.extra_bytes()))
        .ok_or("pins.budget")?;
        let new_charge = glint_core::pins::charge((source.width(), source.height()), layout.size)
            .and_then(|charge| charge.checked_add(pin.extra_bytes()))
            .ok_or("pins.budget")?;
        // The old display stays alive while the worker builds one new pixel buffer.
        let pending_charge = old_charge
            .max(new_charge)
            .checked_add(source.as_bytes().len() as u64)
            .ok_or("pins.budget")?;
        let ui = pin.ui.clone_strong();
        manager
            .budget
            .reserve(id, pending_charge)
            .map_err(|_| messages::text(Message::PinBudget, ui.get_english()))?;
        ui.set_busy(true);
        ui.set_menu_open(false);
        ui.set_message_open(false);
        ui.set_details("".into());
        ui.set_output_error(false);
        ui.set_status(messages::text(Message::PinRotating, ui.get_english()).into());
        manager.entries.get_mut(&id).unwrap().rotation = Some(Rotation {
            layout,
            old_charge,
            new_charge,
            turns,
        });
        let weak_ui = ui.as_weak();
        let result = std::thread::Builder::new()
            .name("glint-pin-rotate".into())
            .spawn(move || {
                let result = if turns == 0 {
                    Ok(source)
                } else {
                    let mut rotated =
                        SharedPixelBuffer::<Rgba8Pixel>::new(dimensions.0, dimensions.1);
                    glint_core::pins::rotate_rgba(
                        source.as_bytes(),
                        rotated.make_mut_bytes(),
                        source.width(),
                        source.height(),
                        turns as u8,
                    )
                    .map(|()| rotated)
                };
                let _ = weak_ui.upgrade_in_event_loop(move |ui| {
                    let _slot = slot;
                    match result {
                        Ok(pixels) => {
                            ui.set_rotation_result(slint::Image::from_rgba8(pixels));
                            ui.invoke_action("rotation-ready".into());
                        }
                        Err(error) => {
                            ui.invoke_action("rotation-failed".into());
                            action_failed(&ui, error);
                        }
                    }
                });
            });
        if let Err(error) = result {
            let _ = manager.budget.reserve(id, old_charge);
            manager.entries.get_mut(&id).unwrap().rotation = None;
            ui.set_busy(false);
            return Err(error.to_string());
        }
        Ok(())
    }
    pub fn create(
        owner: &Rc<RefCell<Self>>,
        image: slint::Image,
        position: (i32, i32),
        config: &Config,
        english: bool,
        done: impl FnOnce(Result<u64, String>) + 'static,
    ) -> Result<u64, String> {
        Self::create_inner(
            owner,
            ImageInput {
                picture: image,
                restored: None,
            },
            None,
            position,
            config,
            english,
            done,
        )
    }
    pub fn create_document(
        owner: &Rc<RefCell<Self>>,
        payload: crate::probe::PinPayload,
        position: (i32, i32),
        config: &Config,
        english: bool,
        done: impl FnOnce(Result<u64, String>) + 'static,
    ) -> Result<u64, String> {
        Self::create_inner(
            owner,
            ImageInput {
                picture: slint::Image::from_rgba8(payload.pixels),
                restored: Some((payload.display, payload.turns)),
            },
            Some(payload.document),
            position,
            config,
            english,
            done,
        )
    }
    pub fn create_restored(
        owner: &Rc<RefCell<Self>>,
        ready: crate::pin_session::ReadyPin,
        config: &Config,
        english: bool,
        done: impl FnOnce(Result<u64, String>) + 'static,
    ) -> Result<u64, String> {
        let p = ready.placement;
        let mut config = config.clone();
        config.pins.on_top = p.on_top;
        config.pins.opacity = p.opacity;
        config.pins.zoom_percent = p.zoom;
        config.pins.passthrough = false;
        let id = Self::create_inner(
            owner,
            ImageInput {
                picture: slint::Image::from_rgba8(ready.original),
                restored: Some((ready.rotated, i32::from(p.turns))),
            },
            Some(ready.document),
            (p.x, p.y),
            &config,
            english,
            done,
        )?;
        if let Some(ui) = owner.borrow().window(id) {
            ui.set_locked(p.locked);
        }
        Ok(id)
    }
    pub fn recover_restored(&mut self, id: u64) -> Result<(), String> {
        self.geometry(id, true)
    }
    pub fn session_snapshot(&self) -> Result<Vec<crate::pin_session::Snapshot>, String> {
        self.entries
            .values()
            .map(|pin| {
                if !pin.ui.get_ready() || pin.ui.get_busy() {
                    return Err("Pin is busy".into());
                }
                let position = native(&pin.ui, |w| w.state())?;
                Ok(crate::pin_session::Snapshot {
                    original: pin.original.clone(),
                    document: pin.document.clone(),
                    placement: glint_core::pin_session::Placement {
                        x: position.x,
                        y: position.y,
                        zoom: pin.ui.get_zoom() as u16,
                        opacity: (pin.ui.get_alpha() * 100.).round() as u8,
                        turns: pin.ui.get_quarter_turns() as u8,
                        on_top: pin.ui.get_on_top(),
                        locked: pin.ui.get_locked(),
                    },
                })
            })
            .collect()
    }
    fn create_inner(
        owner: &Rc<RefCell<Self>>,
        image: ImageInput,
        document: Option<std::sync::Arc<glint_core::ImageDocument>>,
        position: (i32, i32),
        config: &Config,
        english: bool,
        done: impl FnOnce(Result<u64, String>) + 'static,
    ) -> Result<u64, String> {
        let original = image.picture.to_rgba8().ok_or("Missing pin pixels")?;
        let (pixels, turns) = image.restored.unwrap_or_else(|| (original.clone(), 0));
        let image = slint::Image::from_rgba8(pixels.clone());
        if pixels.width() == 0 || pixels.height() == 0 {
            return Err("Empty pin image".into());
        }
        if document
            .as_ref()
            .is_some_and(|d| d.dimensions() != (original.width(), original.height()))
        {
            return Err("Pin document dimensions do not match preview".into());
        }
        let ui = DesktopPin::new().map_err(|e| e.to_string())?;
        let id = {
            let mut manager = owner.borrow_mut();
            manager.set_budget(config.resources.pin_budget_mib);
            manager.next_id = manager
                .next_id
                .checked_add(1)
                .ok_or("Pin identity exhausted")?;
            let id = manager.next_id;
            let initial_charge =
                glint_core::pins::charge((pixels.width(), pixels.height()), (320, 220))
                    .and_then(|charge| {
                        charge.checked_add(document.as_ref().map_or(0, |d| d.retained_bytes()))
                    })
                    .ok_or("pins.budget")?;
            manager
                .budget
                .reserve(id, initial_charge)
                .map_err(|_| messages::text(Message::PinBudget, english))?;
            id
        };
        ui.set_identity(id as i32);
        ui.set_quarter_turns(turns);
        ui.set_english(english);
        ui.set_wheel_opacity(config.pins.wheel == glint_core::config::PinWheel::Opacity);
        ui.set_snap_enabled(config.pins.snap);
        let tr = |id| messages::text(id, english).into();
        ui.set_labels(PinLabels {
            pin: tr(Message::PinLabel),
            locked: tr(Message::PinLocked),
            close: tr(Message::PinClose),
            top: tr(Message::PinTop),
            unlock: tr(Message::PinUnlock),
            lock: tr(Message::PinLock),
            hide: tr(Message::PinHide),
            planned: tr(Message::PinPlanned),
            copy: tr(Message::PinCopy),
            save: tr(Message::PinSave),
            retry: tr(Message::PinRetry),
            dismiss: tr(Message::PinDismiss),
            pass: tr(Message::PinPass),
            pass_off: tr(Message::PinPassOff),
            pass_title: tr(Message::PinPassTitle),
            pass_body: tr(Message::PinPassBody),
            pass_enable: tr(Message::PinPassEnable),
            cancel: tr(Message::PinPassCancel),
            edit: tr(Message::PinEdit),
            rotate: tr(Message::PinRotate),
        });
        ui.set_dark(
            matches!(config.appearance.theme, glint_core::config::Theme::Dark)
                || (matches!(config.appearance.theme, glint_core::config::Theme::System)
                    && glint_platform::system_dark_theme().unwrap_or(false)),
        );
        let rgb = u32::from_str_radix(config.appearance.accent.trim_start_matches('#'), 16)
            .unwrap_or(0x4b5cfa);
        ui.set_accent(slint::Color::from_rgb_u8(
            (rgb >> 16) as u8,
            (rgb >> 8) as u8,
            rgb as u8,
        ));
        ui.set_picture(image);
        crate::motion::configure!(&ui, Motion, config.appearance.reduce_motion);
        crate::motion::install(&ui, config.appearance.reduce_motion);
        crate::interface_scale::install_with_theme(
            &ui,
            config.appearance.scale,
            false,
            config.appearance.theme,
            apply_system_theme,
        );
        ui.set_interface_scale(crate::interface_scale::factor(config.appearance.scale));
        ui.set_alpha(f32::from(config.pins.opacity) / 100.);
        ui.set_on_top(config.pins.on_top);
        ui.set_status(messages::text(Message::PinPreparing, english).into());
        ui.window()
            .set_position(slint::PhysicalPosition::new(position.0, position.1));
        let weak = Rc::downgrade(owner);
        let weak_ui = ui.as_weak();
        let geometry_queue = Rc::new(Cell::new(0));
        let action_queue = geometry_queue.clone();
        ui.on_action(move |action| {
            if let (Some(manager), Some(ui)) = (weak.upgrade(), weak_ui.upgrade()) {
                let result = if action == "geometry-ready" {
                    Self::queue_geometry(
                        Rc::downgrade(&manager),
                        id,
                        ui.as_weak(),
                        action_queue.clone(),
                        glint_platform::PinGeometryEvent::DisplayChanged,
                    );
                    Ok(())
                } else if action == "rotate" {
                    Self::rotate(&manager, id)
                } else if action == "edit" {
                    let handler = manager.borrow().edit_handler.clone();
                    ui.set_menu_open(false);
                    if let Some(handler) = handler {
                        handler(id);
                        Ok(())
                    } else {
                        Err("Resident editor unavailable".into())
                    }
                } else {
                    manager.borrow_mut().action(id, action.as_str())
                };
                if let Err(error) = result {
                    action_failed(&ui, &error);
                }
            }
        });
        let weak = Rc::downgrade(owner);
        let weak_ui = ui.as_weak();
        ui.on_wheel(move |delta, control| {
            if let (Some(manager), Some(ui)) = (weak.upgrade(), weak_ui.upgrade()) {
                let result = manager.borrow_mut().wheel(id, delta, control);
                if let Err(error) = result {
                    action_failed(&ui, &error);
                }
            }
        });
        let weak = ui.as_weak();
        ui.on_change_opacity(move |value| {
            if let Some(ui) = weak.upgrade() {
                ui.set_alpha((value / 100.).clamp(0.2, 1.));
            }
        });
        let weak = ui.as_weak();
        let drag_owner = Rc::downgrade(owner);
        let drag_queue = geometry_queue.clone();
        let drag = RefCell::new(None::<Rc<glint_platform::PinDrag>>);
        ui.on_drag(move |phase| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = match phase {
                0 if ui.get_ready()
                    && !ui.get_locked()
                    && !ui.get_busy()
                    && !ui.get_passthrough() =>
                {
                    let previous = drag.borrow_mut().take();
                    drop(previous);
                    native(&ui, |window| window.begin_drag()).map(|state| {
                        *drag.borrow_mut() = Some(Rc::new(state));
                    })
                }
                1 if !ui.get_locked() && !ui.get_passthrough() && ui.get_ready() => {
                    let active = drag.borrow().clone();
                    active.map_or(Ok(()), |drag| drag.move_to_cursor())
                }
                _ => {
                    let previous = drag.borrow_mut().take();
                    let finished = previous.is_some();
                    drop(previous);
                    if finished {
                        Self::queue_geometry(
                            drag_owner.clone(),
                            id,
                            ui.as_weak(),
                            drag_queue.clone(),
                            glint_platform::PinGeometryEvent::MoveFinished,
                        );
                    }
                    Ok(())
                }
            };
            if let Err(error) = result {
                let previous = drag.borrow_mut().take();
                drop(previous);
                action_failed(&ui, &error);
            }
        });
        let weak = Rc::downgrade(owner);
        ui.window().on_close_requested(move || {
            if let Some(manager) = weak.upgrade() {
                let _ = manager.borrow_mut().close(id);
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
        owner.borrow_mut().entries.insert(
            id,
            Pin {
                events: None,
                geometry_queue,
                resize_failed: false,
                wheel: Default::default(),
                ui: ui.clone_strong(),
                original,
                pixels,
                rotation: None,
                document,
            },
        );
        if let Err(error) = ui.show() {
            let _ = owner.borrow_mut().close(id);
            return Err(error.to_string());
        }
        let weak = Rc::downgrade(owner);
        let zoom = i32::from(config.pins.zoom_percent);
        let default_pass = config.pins.passthrough;
        if let Err(error) = slint::spawn_local(async move {
            let result = ui
                .window()
                .winit_window()
                .await
                .map(drop)
                .map_err(|e| e.to_string())
                .and_then(|()| {
                    let owner = weak.upgrade().ok_or("Pin manager closed")?;
                    if !owner.borrow().entries.contains_key(&id) {
                        return Err("Pin closed during preparation".into());
                    }
                    native(&ui, |w| w.configure(ui.get_on_top(), false))?;
                    owner.borrow_mut().resize(id, zoom)?;
                    native(&ui, |w| {
                        if ui.get_on_top() {
                            w.rescue()?;
                        }
                        Ok(())
                    })?;
                    ui.set_ready(true);
                    let queue = owner
                        .borrow()
                        .entries
                        .get(&id)
                        .ok_or("Pin closed")?
                        .geometry_queue
                        .clone();
                    let hook_owner = Rc::downgrade(&owner);
                    let hook_ui = ui.as_weak();
                    let events = native(&ui, |w| {
                        w.geometry_events(move |event| {
                            Self::queue_geometry(
                                hook_owner.clone(),
                                id,
                                hook_ui.clone(),
                                queue.clone(),
                                event,
                            )
                        })
                    })?;
                    owner
                        .borrow_mut()
                        .entries
                        .get_mut(&id)
                        .ok_or("Pin closed")?
                        .events = Some(events);
                    ui.set_status(messages::text(Message::PinHint, english).into());
                    let manager = owner.borrow();
                    let available = manager.recovery_available();
                    ui.set_rescue_ready(available);
                    ui.set_rescue_shortcut(manager.recovery_shortcut.clone().into());
                    if default_pass {
                        if available {
                            ui.set_confirm_pass(true);
                        } else {
                            Self::unavailable(&ui);
                        }
                    }
                    Ok(id)
                });
            if result.is_err()
                && let Some(owner) = weak.upgrade()
            {
                let _ = owner.borrow_mut().close(id);
            }
            done(result);
        }) {
            let _ = owner.borrow_mut().close(id);
            return Err(error.to_string());
        }
        Ok(id)
    }
    fn action(&mut self, id: u64, action: &str) -> Result<(), String> {
        if action == "close" {
            return self.close(id);
        }
        if matches!(action, "rotation-ready" | "rotation-failed") {
            let pin = self.entries.get_mut(&id).ok_or("Pin closed")?;
            let Some(rotation) = pin.rotation.take() else {
                return Ok(());
            };
            if action == "rotation-ready" {
                let pixels = pin
                    .ui
                    .get_rotation_result()
                    .to_rgba8()
                    .ok_or("Missing rotated pixels")?;
                let _ = self.budget.reserve(id, rotation.new_charge);
                pin.ui.set_picture(pin.ui.get_rotation_result());
                pin.pixels = pixels;
                Self::apply_layout(&pin.ui, &rotation.layout);
                pin.ui.set_quarter_turns(rotation.turns);
                pin.ui
                    .set_status(messages::text(Message::PinRotated, pin.ui.get_english()).into());
            } else {
                let _ = self.budget.reserve(id, rotation.old_charge);
            }
            pin.ui.set_rotation_result(slint::Image::default());
            pin.ui.set_busy(false);
            return Ok(());
        }
        let pin = self.entries.get(&id).ok_or("Pin closed")?;
        if matches!(action, "pass" | "confirm-pass" | "cancel-pass") {
            pin.ui.set_menu_open(false);
            if action == "cancel-pass" {
                pin.ui.set_confirm_pass(false);
                return Ok(());
            }
            if pin.ui.get_busy() {
                return Ok(());
            }
            if action == "pass" && pin.ui.get_passthrough() {
                native(&pin.ui, |w| w.configure(pin.ui.get_on_top(), false))?;
                pin.ui.set_passthrough(false);
                return Ok(());
            }
            if action == "confirm-pass" && !pin.ui.get_confirm_pass() {
                return Ok(());
            }
            if !self.recovery_available() {
                Self::unavailable(&pin.ui);
                return Ok(());
            }
            pin.ui.set_rescue_ready(true);
            pin.ui
                .set_rescue_shortcut(self.recovery_shortcut.clone().into());
            if action == "pass" {
                pin.ui.set_message_open(false);
                pin.ui.set_confirm_pass(true);
            } else {
                native(&pin.ui, |w| w.configure(pin.ui.get_on_top(), true))?;
                pin.ui.set_passthrough(true);
                pin.ui.set_confirm_pass(false);
                pin.ui.set_status(
                    messages::text(Message::PinPassActive, pin.ui.get_english()).into(),
                );
            }
            return Ok(());
        }
        if matches!(action, "copy" | "save" | "save-elsewhere") {
            crate::pin_output::start(
                &pin.ui,
                pin.pixels.clone(),
                self.output_options.clone(),
                action != "copy",
                action == "save-elsewhere",
                self.diagnostic_choice.take(),
            );
            return Ok(());
        }
        if action == "menu" {
            pin.ui.set_rescue_ready(self.recovery_available());
            pin.ui.set_menu_open(!pin.ui.get_menu_open());
            return Ok(());
        }
        pin.ui.set_menu_open(false);
        match action {
            "lock" => pin.ui.set_locked(!pin.ui.get_locked()),
            "top" => {
                let top = !pin.ui.get_on_top();
                native(&pin.ui, |w| w.configure(top, pin.ui.get_passthrough()))?;
                pin.ui.set_on_top(top);
            }
            "hide" => pin.ui.hide().map_err(|e| e.to_string())?,
            "larger" | "smaller" => {
                let zoom = pin.ui.get_zoom() + if action == "larger" { 25 } else { -25 };
                self.resize(id, zoom.max(1))?;
            }
            _ => return Err("Unknown pin action".into()),
        }
        Ok(())
    }
    pub fn toggle_all(&mut self) -> Result<(), String> {
        let available = self.recovery_available();
        let visible = self.entries.values().any(|p| p.ui.window().is_visible());
        for pin in self.entries.values() {
            if visible {
                pin.ui.hide().map_err(|e| e.to_string())?;
            } else {
                pin.ui.show().map_err(|e| e.to_string())?;
                let through = pin.ui.get_passthrough() && available;
                native(&pin.ui, |w| w.configure(pin.ui.get_on_top(), through))?;
                pin.ui.set_passthrough(through);
            }
        }
        Ok(())
    }
    pub fn rescue_all(&mut self) -> Result<(), String> {
        let mut failure = None;
        for pin in self.entries.values() {
            let result = pin
                .ui
                .show()
                .map_err(|e| e.to_string())
                .and_then(|()| native(&pin.ui, |w| w.rescue()));
            if let Err(error) = result {
                failure = Some(error);
                continue;
            }
            pin.ui.set_passthrough(false);
            pin.ui.set_confirm_pass(false);
            pin.ui.set_message_open(false);
            pin.ui.set_on_top(true);
            pin.ui.set_menu_open(false);
            pin.ui
                .set_status(messages::text(Message::PinRescued, pin.ui.get_english()).into());
        }
        failure.map_or(Ok(()), Err)
    }
    pub fn hide_for_capture(&mut self) -> Result<(), String> {
        for (id, pin) in &self.entries {
            if pin.ui.window().is_visible() {
                if let Err(error) = native(&pin.ui, |w| w.hide_for_capture())
                    .and_then(|()| pin.ui.hide().map_err(|e| e.to_string()))
                {
                    let _ = self.restore_after_capture();
                    return Err(error);
                }
                self.capture_hidden.insert(*id);
            }
        }
        Ok(())
    }
    pub fn restore_after_capture(&mut self) -> Result<(), String> {
        let available = self.recovery_available();
        let ids: Vec<_> = self.capture_hidden.iter().copied().collect();
        let mut error = None;
        for id in ids {
            if let Some(pin) = self.entries.get(&id) {
                let restored = pin.ui.show().map_err(|e| e.to_string()).and_then(|()| {
                    native(&pin.ui, |w| {
                        w.allow_capture()?;
                        let through = pin.ui.get_passthrough() && available;
                        w.configure(pin.ui.get_on_top(), through)?;
                        pin.ui.set_passthrough(through);
                        Ok(())
                    })
                });
                if let Err(e) = restored {
                    error = Some(e);
                    continue;
                }
            }
            self.capture_hidden.remove(&id);
        }
        error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_rejection_exposes_details_without_reusing_save_retry_or_changing_pin() {
        struct Backend;
        impl slint::platform::Platform for Backend {
            fn create_window_adapter(
                &self,
            ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
                Ok(
                    slint::platform::software_renderer::MinimalSoftwareWindow::new(
                        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
                    ),
                )
            }
        }
        slint::platform::set_platform(Box::new(Backend)).unwrap();
        for english in [false, true] {
            let ui = DesktopPin::new().unwrap();
            ui.set_english(english);
            ui.set_ready(true);
            ui.set_zoom(25);
            ui.set_alpha(0.7);
            ui.set_locked(true);
            ui.window().set_size(slint::PhysicalSize::new(320, 220));
            let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                &[24, 48, 96, 255].repeat(640 * 480),
                640,
                480,
            );
            ui.set_picture(slint::Image::from_rgba8(pixels.clone()));
            let mut manager = Manager::default();
            let charge = glint_core::pins::charge((640, 480), (320, 220)).unwrap();
            manager.budget.set_limit(charge);
            manager.budget.reserve(1, charge).unwrap();
            manager.entries.insert(
                1,
                Pin {
                    events: None,
                    geometry_queue: Default::default(),
                    resize_failed: false,
                    wheel: Default::default(),
                    ui: ui.clone_strong(),
                    original: pixels.clone(),
                    pixels,
                    rotation: None,
                    document: None,
                },
            );
            // A prior failed save left a retry action; this resize failure must
            // explain its own cause without offering to retry that old save.
            ui.set_output_error(true);
            ui.set_last_save(true);
            ui.set_menu_open(true);
            let error = manager.action(1, "larger").unwrap_err();
            action_failed(&ui, &error);
            assert!(ui.get_message_open());
            assert!(ui.get_details().contains(&error));
            assert!(!ui.get_output_error());
            assert!(!ui.get_menu_open());
            assert_eq!(ui.window().size(), slint::PhysicalSize::new(320, 220));
            assert_eq!(ui.get_zoom(), 25);
            assert_eq!(ui.get_alpha(), 0.7);
            assert!(ui.get_locked());
            assert_eq!(manager.used(), charge);
            assert_eq!(
                ui.get_picture().to_rgba8().unwrap().as_bytes(),
                &[24, 48, 96, 255].repeat(640 * 480)
            );
            // Dismissing the message does not close or alter the pin.
            ui.set_message_open(false);
            assert_eq!(manager.count(), 1);
            manager.close(1).unwrap();
            assert_eq!(manager.used(), 0);
        }
    }
}
