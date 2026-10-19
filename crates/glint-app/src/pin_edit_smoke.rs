//! Resident re-annotation transactions; real clipboard/files and native pin windows.
use crate::{pins, probe, resident::AppState, settings::SettingsWindow};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Run {
    id: u64,
    second: u64,
    started: Option<Instant>,
    original: Option<Arc<glint_core::ImageDocument>>,
    blocker: Option<glint_platform::ClipboardBlocker>,
    output: Option<PathBuf>,
    released: Vec<Weak<glint_core::ImageDocument>>,
}
fn area(ui: &probe::CaptureProbe) -> (f32, f32) {
    (ui.get_left(), ui.get_top())
}
fn mark(ui: &probe::CaptureProbe) {
    let (x, y) = area(ui);
    ui.set_tool(1);
    ui.invoke_pointer(0, x + 20., y + 20.);
    ui.invoke_pointer(2, x + 55., y + 55.);
}
fn snapshot(ui: &probe::CaptureProbe, path: &Path) -> Result<(), String> {
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
fn flat(doc: &glint_core::ImageDocument) -> Result<Vec<u8>, String> {
    let (w, h) = doc.dimensions();
    glint_core::composite_selection(
        doc.source(),
        w,
        h,
        glint_core::Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        },
        doc.edits(),
    )
    .map_err(str::to_owned)
}
impl Run {
    fn wait(&self, reason: &str) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
        {
            Err(reason.into())
        } else {
            Ok(false)
        }
    }
    pub fn step(
        &mut self,
        stage: u8,
        settings: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        directory: &Path,
    ) -> Result<bool, String> {
        let manager = app.borrow().pins.clone();
        if stage == 0 {
            self.output = app.borrow().capture_output.clone();
            let mut config = app.borrow().session.preferences.saved().clone();
            config.pins.opacity = 55;
            config.pins.zoom_percent = 100;
            config.pins.passthrough = false;
            config.pins.on_top = false;
            let raw = [97u8, 131, 169, 255].repeat(400 * 300);
            let mut edits = glint_core::Edits::default();
            edits.push(glint_core::Redaction {
                bounds: glint_core::Rect {
                    x: 80,
                    y: 70,
                    width: 140,
                    height: 70,
                },
                color: [32, 43, 66, 255],
            });
            let doc = Arc::new(
                glint_core::ImageDocument::from_selection(
                    &raw,
                    400,
                    300,
                    glint_core::Rect {
                        x: 0,
                        y: 0,
                        width: 400,
                        height: 300,
                    },
                    &edits,
                )
                .map_err(str::to_owned)?,
            );
            let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                &flat(&doc)?,
                400,
                300,
            );
            self.original = Some(doc.clone());
            self.id = pins::Manager::create_document(
                &manager,
                probe::PinPayload {
                    document: doc,
                    pixels: pixels.clone(),
                    display: pixels,
                    turns: 0,
                },
                (180, 180),
                &config,
                true,
                |_| {},
            )?;
            self.second = pins::Manager::create(
                &manager,
                slint::Image::from_rgba8(
                    slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&raw, 400, 300),
                ),
                (850, 180),
                &config,
                true,
                |_| {},
            )?;
            self.started = Some(Instant::now());
            return Ok(true);
        }
        if stage == 19 {
            if manager.borrow().count() != 0
                || manager.borrow().used() != 0
                || self.released.iter().any(|weak| weak.upgrade().is_some())
            {
                return Err("Re-annotation close retained documents or reservations".into());
            }
            return Ok(true);
        }
        let pin = manager
            .borrow()
            .window(self.id)
            .ok_or("Original fixture pin closed")?
            .clone_strong();
        match stage {
            1 => {
                if !pin.get_ready() || !manager.borrow().window(self.second).unwrap().get_ready() {
                    return self.wait("Edit fixtures not ready");
                }
                pin.invoke_action("lock".into());
                pin.invoke_action("rotate".into());
                self.started = Some(Instant::now());
            }
            2 => {
                if pin.get_busy() {
                    return self.wait("Edit fixture rotation not ready");
                }
                pin.invoke_action("menu".into());
            }
            3 => {
                pin.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(120., 150.),
                        delta_x: 0.,
                        delta_y: -10_000.,
                    });
            }
            4 => {
                let position = slint::LogicalPosition::new(140., 146.);
                for down in [true, false] {
                    pin.window().dispatch_event(if down {
                        slint::platform::WindowEvent::PointerPressed {
                            position,
                            button: slint::platform::PointerEventButton::Left,
                        }
                    } else {
                        slint::platform::WindowEvent::PointerReleased {
                            position,
                            button: slint::platform::PointerEventButton::Left,
                        }
                    });
                }
                self.started = Some(Instant::now());
            }
            5 => {
                let state = app.borrow();
                let Some(ui) = state.capture.as_ref() else {
                    return self.wait("Edit menu pointer did not open editor");
                };
                if !ui.get_pin_editing()
                    || state.edit_pin != Some(self.id)
                    || ui.get_object_count() != 1
                    || !ui.get_can_undo()
                    || ui.get_wide() != 400.
                    || ui.get_high() != 300.
                    || pin.window().is_visible()
                    || manager
                        .borrow()
                        .window(self.second)
                        .unwrap()
                        .window()
                        .is_visible()
                    || settings.window().is_visible()
                {
                    return Err(
                        "Editor lost source objects/history or exposed hidden windows".into(),
                    );
                }
                snapshot(ui, &directory.join("pin-edit-loaded.png"))?;
                let (x, y) = area(ui);
                ui.set_tool(0);
                ui.invoke_pointer(0, x + 110., y + 100.);
                ui.invoke_pointer(2, x + 110., y + 100.);
                if !ui.get_object_selected() {
                    return Err("Original annotation is not selectable".into());
                }
                ui.invoke_action("delete-object".into());
                if ui.get_object_count() != 0 {
                    return Err("Original annotation was flattened".into());
                }
                ui.invoke_action("undo".into());
                if ui.get_object_count() != 1 {
                    return Err("Original annotation delete cannot undo".into());
                }
                ui.invoke_action("redo".into());
                ui.invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            6 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Cancel did not close editor");
                }
                if !Arc::ptr_eq(
                    &manager.borrow().document(self.id).unwrap(),
                    self.original.as_ref().unwrap(),
                ) || !pin.window().is_visible()
                    || pin.get_quarter_turns() != 1
                    || !pin.get_locked()
                    || pin.get_on_top()
                    || (pin.get_alpha() - 0.55).abs() > 0.001
                {
                    return Err("Cancel changed original pin or appearance".into());
                }
                pin.invoke_action("edit".into());
                self.started = Some(Instant::now());
            }
            7 => {
                let state = app.borrow();
                let Some(ui) = state.capture.as_ref() else {
                    return self.wait("Second edit did not open");
                };
                mark(ui);
                if ui.get_object_count() != 2 {
                    return Err("New annotation not editable".into());
                }
                manager.borrow_mut().set_budget(0);
                ui.invoke_action("pin".into());
                self.started = Some(Instant::now());
            }
            8 => {
                let state = app.borrow();
                let ui = state
                    .capture
                    .as_ref()
                    .ok_or("Budget failure closed editor")?;
                if ui.get_busy() {
                    return self.wait("Budget rejection did not return");
                }
                if !ui.window().is_visible()
                    || ui.get_object_count() != 2
                    || !ui.get_status().contains("limit reached")
                    || !Arc::ptr_eq(
                        &manager.borrow().document(self.id).unwrap(),
                        self.original.as_ref().unwrap(),
                    )
                {
                    return Err(format!(
                        "Budget rejection did not retain original/editor: {}",
                        ui.get_status()
                    ));
                }
                snapshot(ui, &directory.join("pin-edit-budget.png"))?;
                manager.borrow_mut().set_budget(64);
                ui.invoke_action("pin".into());
                self.started = Some(Instant::now());
            }
            9 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Apply retry did not close editor");
                }
                let doc = manager.borrow().document(self.id).unwrap();
                if manager.borrow().count() != 2
                    || doc.edits().marks().len() != 2
                    || Arc::ptr_eq(&doc, self.original.as_ref().unwrap())
                    || pin.get_quarter_turns() != 1
                    || !pin.get_locked()
                    || pin.get_on_top()
                    || (pin.get_alpha() - 0.55).abs() > 0.001
                {
                    return Err("Apply did not replace the original rotated pin atomically".into());
                }
                let expected = image::imageops::rotate90(
                    &image::RgbaImage::from_raw(400, 300, flat(&doc)?).unwrap(),
                );
                if pin.get_picture().to_rgba8().unwrap().as_bytes() != expected.as_raw() {
                    return Err("Applied pin direction/pixels differ".into());
                }
                pin.invoke_action("edit".into());
                self.started = Some(Instant::now());
            }
            10 => {
                let state = app.borrow();
                let Some(ui) = state.capture.as_ref() else {
                    return self.wait("Copy edit did not open");
                };
                if ui.get_object_count() != 2 {
                    return Err("Applied history was lost".into());
                }
                ui.invoke_action("undo".into());
                if ui.get_object_count() != 1 {
                    return Err("Applied undo did not restore earlier scene".into());
                }
                self.blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                ui.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            11 => {
                let state = app.borrow();
                let ui = state.capture.as_ref().ok_or("Copy failure closed editor")?;
                if ui.get_busy() {
                    return self.wait("Occupied clipboard did not return");
                }
                if !ui.window().is_visible()
                    || ui.get_object_count() != 1
                    || manager
                        .borrow()
                        .document(self.id)
                        .unwrap()
                        .edits()
                        .marks()
                        .len()
                        != 2
                {
                    return Err("Copy failure modified pin or lost editing".into());
                }
                self.blocker = None;
                ui.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            12 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Copy retry did not close editor");
                }
                let copied = glint_platform::read_clipboard_image()?;
                if (copied.width, copied.height) != (400, 300)
                    || copied.rgba != flat(self.original.as_ref().unwrap())?
                    || manager
                        .borrow()
                        .document(self.id)
                        .unwrap()
                        .edits()
                        .marks()
                        .len()
                        != 2
                {
                    return Err("Editor copy did not use canvas-only source orientation".into());
                }
                app.borrow_mut().capture_output = Some(directory.join("pin-edit-save.png"));
                pin.invoke_action("edit".into());
                self.started = Some(Instant::now());
            }
            13 => {
                let state = app.borrow();
                let Some(ui) = state.capture.as_ref() else {
                    return self.wait("Save edit did not open");
                };
                ui.invoke_action("save".into());
                self.started = Some(Instant::now());
            }
            14 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Editor save did not finish");
                }
                let file = image::open(directory.join("pin-edit-save.png"))
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if file.dimensions() != (400, 300)
                    || *file.as_raw() != flat(&manager.borrow().document(self.id).unwrap())?
                {
                    return Err("Editor save included matte/rotation or changed pixels".into());
                }
                pin.invoke_action("edit".into());
                self.started = Some(Instant::now());
            }
            15 => {
                let state = app.borrow();
                let Some(ui) = state.capture.as_ref() else {
                    return self.wait("Cancellation edit did not open");
                };
                mark(ui);
                ui.invoke_action("pin".into());
                ui.invoke_action("cancel".into());
                if !ui.get_closed() {
                    return Err("Pending output cancellation did not latch editor closed".into());
                }
                self.started = Some(Instant::now());
            }
            16 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Pending cancel did not finish");
                }
                let Some(slot) = probe::OutputSlot::acquire() else {
                    return self.wait("Canceled worker has not released output slot");
                };
                drop(slot);
                if manager
                    .borrow()
                    .document(self.id)
                    .unwrap()
                    .edits()
                    .marks()
                    .len()
                    != 2
                    || !pin.window().is_visible()
                {
                    return Err("Late output applied after cancel".into());
                }
                let second = manager.borrow().window(self.second).unwrap().clone_strong();
                second.invoke_action("edit".into());
                self.started = Some(Instant::now());
            }
            17 => {
                let state = app.borrow();
                let Some(ui) = state.capture.as_ref() else {
                    return self.wait("Clipboard image edit did not open");
                };
                if ui.get_object_count() != 0 {
                    return Err("Plain image acquired phantom objects".into());
                }
                let before = (ui.get_left(), ui.get_top(), ui.get_wide(), ui.get_high());
                let (x, y) = area(ui);
                ui.set_tool(0);
                ui.invoke_pointer(0, x + 300., y + 200.);
                ui.invoke_pointer(2, x + 320., y + 220.);
                if before != (ui.get_left(), ui.get_top(), ui.get_wide(), ui.get_high()) {
                    return Err("Image editor moved its fixed source selection".into());
                }
                mark(ui);
                ui.invoke_action("pin".into());
                self.started = Some(Instant::now());
            }
            18 => {
                if app.borrow().capture.is_some() {
                    return self.wait("Plain image apply did not finish");
                }
                if manager
                    .borrow()
                    .document(self.second)
                    .unwrap()
                    .edits()
                    .marks()
                    .len()
                    != 1
                {
                    return Err("Plain image re-annotation not committed".into());
                }
                self.original = None;
                for id in [self.id, self.second] {
                    self.released
                        .push(Arc::downgrade(&manager.borrow().document(id).unwrap()));
                    manager.borrow_mut().close(id)?;
                }
                app.borrow_mut().capture_output = self.output.take();
            }
            _ => return Err("Unexpected edit smoke stage".into()),
        }
        Ok(true)
    }
}
