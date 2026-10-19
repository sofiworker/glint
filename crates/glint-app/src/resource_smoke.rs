//! Opt-in measurement of the actual resident/capture/editor/export pipeline.
//! Idle intervals have one timer at each boundary, not a sampling/polling loop.
use crate::{resident::AppState, settings::SettingsWindow};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

const ROUNDS: u32 = 3;
const IDLE: Duration = Duration::from_secs(5);
struct Run {
    ui: slint::Weak<SettingsWindow>,
    app: Rc<RefCell<AppState>>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    phase: u8,
    round: u32,
    before: glint_platform::ProcessResources,
    input_tick: u32,
    clock: Instant,
    samples: Vec<toml::Value>,
    width: u32,
    height: u32,
    local_4k: bool,
    canvas: (u32, u32),
    endurance: bool,
    rounds: u32,
    editor_receipt: Option<crate::probe::CaptureProbe>,
    edges: Vec<(u32, u32, [u8; 4])>,
    pin_id: Option<u64>,
    pin_ready: Rc<RefCell<Option<Result<u64, String>>>>,
    pin_expected: Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
    total_clock: Instant,
    serialized_samples: String,
}
pub fn install(
    ui: &SettingsWindow,
    app: Rc<RefCell<AppState>>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    local_4k: bool,
    endurance: bool,
) -> Result<(), String> {
    if local_4k {
        // Explicit QA asset creation precedes the measurement baseline. No
        // desktop pixels are read by this mode. The production decoder still
        // reads the actual PNG through the normal local-image action below.
        let source = image::RgbaImage::from_fn(3840, 2160, |x, y| image::Rgba(fixture_pixel(x, y)));
        source
            .save(directory.join("source-4k.png"))
            .map_err(|e| e.to_string())?;
    }
    ui.invoke_action("close".into());
    if ui.window().is_visible() || app.borrow().capture.is_some() {
        return Err("Resource probe must start with only the resident tray".into());
    }
    let run = Rc::new(RefCell::new(Run {
        ui: ui.as_weak(),
        app,
        directory,
        error,
        phase: 0,
        round: 0,
        before: glint_platform::process_resources()?,
        input_tick: glint_platform::last_session_input_tick()?,
        clock: Instant::now(),
        samples: vec![],
        width: 0,
        height: 0,
        local_4k,
        canvas: (0, 0),
        endurance,
        rounds: if endurance { 30 } else { ROUNDS },
        editor_receipt: None,
        edges: vec![],
        pin_id: None,
        pin_ready: Default::default(),
        pin_expected: None,
        total_clock: Instant::now(),
        // Reserve the fixed endurance report buffer before the initial baseline.
        // Retaining hundreds of TOML object trees would itself create a trend.
        serialized_samples: String::with_capacity(if endurance { 1024 * 1024 } else { 0 }),
    }));
    // Allow startup work to settle before measuring the first idle interval.
    slint::Timer::single_shot(Duration::from_secs(2), move || tick(run));
    Ok(())
}
fn fixture_pixel(x: u32, y: u32) -> [u8; 4] {
    [(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 255]
}
impl Run {
    fn mark(&mut self, name: &str) -> Result<(), String> {
        let after = glint_platform::process_resources()?;
        let input_tick = glint_platform::last_session_input_tick()?;
        let wall_ms = self.clock.elapsed().as_secs_f64() * 1000.;
        let cpu_ms = (after.cpu_100ns - self.before.cpu_100ns) as f64 / 10000.;
        eprintln!(
            "Resource round {} {name}: wall {wall_ms:.3}ms CPU {cpu_ms:.3}ms working {} private {} handles {} GDI {} USER {}",
            self.round,
            after.working_set_bytes,
            after.private_bytes,
            after.handles,
            after.gdi_objects,
            after.user_objects
        );
        let mut row = toml::Table::new();
        row.insert("phase".into(), name.into());
        row.insert("round".into(), i64::from(self.round).into());
        row.insert(
            "session_input_changed".into(),
            (input_tick != self.input_tick).into(),
        );
        row.insert("wall_ms".into(), wall_ms.into());
        row.insert("cpu_ms".into(), cpu_ms.into());
        for (prefix, counters) in [("before", &self.before), ("after", &after)] {
            for (key, value) in [
                ("working_set_bytes", counters.working_set_bytes as u64),
                ("private_bytes", counters.private_bytes as u64),
                ("handles", u64::from(counters.handles)),
                ("gdi_objects", u64::from(counters.gdi_objects)),
                ("user_objects", u64::from(counters.user_objects)),
            ] {
                row.insert(
                    format!("{prefix}_{key}"),
                    i64::try_from(value)
                        .map_err(|_| "Resource counter overflow")?
                        .into(),
                );
            }
        }
        if self.endurance {
            let text = toml::to_string(&row).map_err(|e| e.to_string())?;
            if self.serialized_samples.len() + text.len() + 16 > self.serialized_samples.capacity()
            {
                return Err("Fixed endurance report capacity exceeded".into());
            }
            self.serialized_samples.push_str("[[samples]]\n");
            self.serialized_samples.push_str(&text);
            self.serialized_samples.push('\n');
        } else {
            self.samples.push(row.into());
        }
        self.before = after;
        self.input_tick = input_tick;
        self.clock = Instant::now();
        Ok(())
    }
    fn begin_capture(&mut self, ui: &SettingsWindow) -> Result<(), String> {
        self.round += 1;
        let path = self.directory.join(format!("capture-{}.png", self.round));
        if path.exists() {
            return Err("Resource probe output already exists".into());
        }
        self.app.borrow_mut().capture_output = Some(path);
        if self.local_4k {
            ui.invoke_action("workbench".into());
            crate::local_image::selected(
                &mut self.app.borrow_mut(),
                self.directory.join("source-4k.png"),
            );
            ui.invoke_action("local-image".into());
        } else {
            ui.invoke_action("capture".into());
        }
        self.phase = 2;
        Ok(())
    }
    fn step(&mut self) -> Result<Option<Duration>, String> {
        let ui = self
            .ui
            .upgrade()
            .ok_or("Resource settings owner disappeared")?;
        match self.phase {
            0 => {
                self.before = glint_platform::process_resources()?;
                self.input_tick = glint_platform::last_session_input_tick()?;
                self.clock = Instant::now();
                self.phase = 1;
                return Ok(Some(IDLE));
            }
            1 => {
                self.mark("resident_idle_initial")?;
                self.begin_capture(&ui)?;
            }
            2 => {
                let capture = self.app.borrow().capture.as_ref().map(|c| c.clone_strong());
                let Some(capture) = capture else {
                    if self.clock.elapsed() > Duration::from_secs(20) {
                        return Err(format!(
                            "Resource capture did not open: {}",
                            ui.get_status()
                        ));
                    }
                    return Ok(Some(Duration::from_millis(20)));
                };
                // Force a real software render; this is readiness through first
                // rendered frame, not physical input or compositor latency.
                drop(
                    capture
                        .window()
                        .take_snapshot()
                        .map_err(|e| e.to_string())?,
                );
                self.canvas = (capture.get_source_w() as u32, capture.get_source_h() as u32);
                if self.local_4k {
                    self.width = capture.get_wide() as u32;
                    self.height = capture.get_high() as u32;
                    if (self.width, self.height) != (3840, 2160) || !capture.get_selected() {
                        return Err("4K local source was resized or not selected".into());
                    }
                } else {
                    (self.width, self.height) = self.canvas;
                }
                if self.width < 200 || self.height < 200 {
                    return Err("Capture too small for resource scene".into());
                }
                self.mark(if self.local_4k {
                    "local_png_decode_through_rendered_frame"
                } else {
                    "capture_through_rendered_frame"
                })?;
                self.phase = 7;
                return Ok(Some(Duration::from_millis(500)));
            }
            3 => {
                self.mark("frozen_overlay_idle")?;
                let capture = self
                    .app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Resource editor closed")?
                    .clone_strong();
                let (w, h) = (self.width as f32, self.height as f32);
                let (left, top) = if self.local_4k {
                    (capture.get_left(), capture.get_top())
                } else {
                    capture.set_tool(0);
                    capture.invoke_pointer(0, 20., 20.);
                    capture.invoke_pointer(2, w - 20., h - 20.);
                    (0., 0.)
                };
                capture.set_tool(2);
                capture.invoke_pointer(0, left + 40., top + 80.);
                for point in 1..=600 {
                    let x = 40. + (w - 80.) * point as f32 / 600.;
                    let y = 80. + ((point % 80) as f32 / 80.) * (h - 160.);
                    capture.invoke_pointer(1, left + x, top + y);
                }
                capture.invoke_pointer(2, left + w - 40., top + 80.);
                capture.set_tool(1);
                capture.invoke_pointer(0, left + 60., top + 60.);
                capture.invoke_pointer(2, left + 180., top + 130.);
                if capture.get_object_count() != 2 || !capture.get_output_ready() {
                    return Err("Measured stroke/redaction did not commit".into());
                }
                capture.invoke_action("undo".into());
                capture.invoke_action("redo".into());
                if capture.get_object_count() != 2 {
                    return Err("Measured edit redo failed".into());
                }
                capture.invoke_canvas_left();
                drop(
                    capture
                        .window()
                        .take_snapshot()
                        .map_err(|e| e.to_string())?,
                );
                self.mark("stroke_600_moves_redaction_undo_redo_render")?;
                self.phase = 8;
                return Ok(Some(Duration::from_millis(500)));
            }
            4 => {
                self.mark("edited_overlay_idle")?;
                let capture = self
                    .app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Resource editor closed before export")?
                    .clone_strong();
                if self.endurance {
                    let source = capture
                        .get_desktop()
                        .to_rgba8()
                        .ok_or("Missing frozen source")?;
                    let (wide, high) = (self.width - 40, self.height - 40);
                    self.edges.clear();
                    let mut sample = |x: u32, y: u32| {
                        let i =
                            ((y + 20) as usize * source.width() as usize + (x + 20) as usize) * 4;
                        self.edges
                            .push((x, y, source.as_bytes()[i..i + 4].try_into().unwrap()));
                    };
                    for x in 0..wide {
                        sample(x, 0);
                        sample(x, high - 1);
                    }
                    for y in 1..high - 1 {
                        sample(0, y);
                        sample(wide - 1, y);
                    }
                    self.editor_receipt = Some(capture.clone_strong());
                }
                capture.invoke_action("save".into());
                self.phase = 5;
            }
            5 => {
                if self.app.borrow().capture.is_some() || self.app.borrow().capturing {
                    if self.clock.elapsed() > Duration::from_secs(30) {
                        return Err(
                            "Resource PNG export did not complete; inspect retained editor".into(),
                        );
                    }
                    return Ok(Some(Duration::from_millis(20)));
                }
                if self.endurance {
                    let receipt = self
                        .editor_receipt
                        .take()
                        .ok_or("Missing endurance export receipt")?;
                    if !receipt.get_closed()
                        || receipt.get_busy()
                        || receipt.get_completion_kind() != 2
                    {
                        return Err("Endurance editor left without successful PNG receipt".into());
                    }
                    drop(receipt);
                }
                self.mark("png_export_through_editor_release")?;
                let dimensions = image::image_dimensions(
                    self.directory.join(format!("capture-{}.png", self.round)),
                )
                .map_err(|e| e.to_string())?;
                let expected = if self.local_4k {
                    (3840, 2160)
                } else {
                    (self.width - 40, self.height - 40)
                };
                if dimensions != expected {
                    return Err("Measured PNG has wrong dimensions".into());
                }
                if self.local_4k {
                    let output =
                        image::open(self.directory.join(format!("capture-{}.png", self.round)))
                            .map_err(|e| e.to_string())?
                            .into_rgba8();
                    // Check every source/cover pixel outside the pen's bounding
                    // area; verify pen coverage separately without duplicating
                    // the rasterizer's algorithm.
                    let mut pen_changed = 0;
                    for (x, y, pixel) in output.enumerate_pixels() {
                        if (60..180).contains(&x) && (60..130).contains(&y) {
                            if pixel.0 != [32, 43, 66, 255] {
                                return Err(
                                    "4K solid cover pixels differ after undo/redo/export".into()
                                );
                            }
                        } else if (35..3840 - 35).contains(&x) && (75..2160 - 75).contains(&y) {
                            pen_changed += usize::from(pixel.0 != fixture_pixel(x, y));
                        } else if pixel.0 != fixture_pixel(x, y) {
                            return Err("4K untouched source pixels changed".into());
                        }
                    }
                    if pen_changed < 1000 {
                        return Err("4K pen did not reach exported pixels".into());
                    }
                    let source = image::open(self.directory.join("source-4k.png"))
                        .map_err(|e| e.to_string())?
                        .into_rgba8();
                    if source.dimensions() != (3840, 2160)
                        || source
                            .enumerate_pixels()
                            .any(|(x, y, p)| p.0 != fixture_pixel(x, y))
                    {
                        return Err("4K source file changed".into());
                    }
                }
                if ui.window().is_visible() {
                    return Err("Resource export returned to settings".into());
                }
                if self.endurance {
                    let output =
                        image::open(self.directory.join(format!("capture-{}.png", self.round)))
                            .map_err(|e| e.to_string())?
                            .into_rgba8();
                    for (x, y, rgba) in self.edges.drain(..) {
                        if output.get_pixel(x, y).0 != rgba {
                            return Err("Endurance untouched perimeter changed".into());
                        }
                    }
                    for y in 40..110 {
                        for x in 40..160 {
                            if output.get_pixel(x, y).0 != [32, 43, 66, 255] {
                                return Err(
                                    "Endurance solid cover differs after undo/redo/export".into()
                                );
                            }
                        }
                    }
                    let manager = self.app.borrow().pins.clone();
                    if manager.borrow().count() != 0 || manager.borrow().used() != 0 {
                        return Err("Endurance pin budget was not released".into());
                    }
                    let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        output.as_raw(),
                        output.width(),
                        output.height(),
                    );
                    self.pin_expected = Some(pixels.clone());
                    let ready = self.pin_ready.clone();
                    self.pin_id = Some(crate::pins::Manager::create(
                        &manager,
                        slint::Image::from_rgba8(pixels),
                        (120, 120),
                        self.app.borrow().session.preferences.saved(),
                        true,
                        move |result| *ready.borrow_mut() = Some(result),
                    )?);
                    self.phase = 10;
                    return Ok(Some(Duration::from_millis(20)));
                }
                self.phase = 9;
                return Ok(Some(Duration::from_millis(3500)));
            }
            6 => {
                self.mark("resident_idle_after_export")?;
                if self.round < self.rounds {
                    self.begin_capture(&ui)?;
                } else {
                    let mut report = toml::Table::new();
                    report.insert("schema_version".into(), 1.into());
                    report.insert(
                        "profile".into(),
                        if cfg!(debug_assertions) {
                            "debug"
                        } else {
                            "release"
                        }
                        .into(),
                    );
                    report.insert("renderer".into(), "Slint winit software".into());
                    report.insert("capture_backend".into(), "Windows GDI".into());
                    report.insert(
                        "workflow".into(),
                        if self.endurance {
                            "real_desktop_endurance"
                        } else if self.local_4k {
                            "synthetic_4k_local_png"
                        } else {
                            "real_desktop"
                        }
                        .into(),
                    );
                    if self.local_4k {
                        report.insert(
                            "capture_backend".into(),
                            "Not used; local PNG decoder".into(),
                        );
                    }
                    report.insert("canvas_width".into(), i64::from(self.canvas.0).into());
                    report.insert("canvas_height".into(), i64::from(self.canvas.1).into());
                    report.insert("source_width".into(), i64::from(self.width).into());
                    report.insert("source_height".into(), i64::from(self.height).into());
                    report.insert("rounds".into(), i64::from(self.rounds).into());
                    report.insert(
                        "total_wall_seconds".into(),
                        self.total_clock.elapsed().as_secs_f64().into(),
                    );
                    report.insert("method".into(), "Boundary counters of actual resident/capture/edit/PNG workflow. Idle intervals use one five-second timer and no probe polling, after separately recorded render/feedback settling. Session input timestamp changes are observed only at boundaries, with no keys/content collected; changed intervals are not idle evidence, unchanged does not prove application inactivity. Active capture/export readiness polls every 20ms and includes probe overhead. Rendering is forced via take_snapshot; no physical input/compositor latency. Edit events are synthetic and delivered in a burst. Save paths are predetermined; system file dialogs excluded. Memory is boundary samples, not peaks. Short windows are not long-running/leak/4K/mixed-DPI acceptance.".into());
                    if self.local_4k {
                        report.insert("method_4k".into(), "Generated 3840x2160 PNG, creation excluded from baseline; actual local decoder/padded full-source canvas/software preview/shared editor/PNG export. Selection remains whole source. Source and exported untouched/cover pixels checked after timed export; pen must change at least 1000 pixels. Verification overhead is in feedback settling, not timed export. This measures synthetic 4K image editing on the reported monitor, not physical 4K display/capture, human input or frame rate.".into());
                    }
                    if self.endurance {
                        report.insert("method_endurance".into(), "Thirty actual desktop capture/edit/PNG cycles. Each export requires successful editor completion, exact frozen-source perimeter and 8400 solid-cover pixels. Decode the actual output into one native pin, await readiness, rotate90, compare every RGBA pixel, force software rendering, close and require zero pins/budget. Extra validation/allocation overhead is included in pin creation phase. Receipt retains the closed editor until checked then drops before export sample. Pins closed before normal post-feedback/resident settling. No physical input, multiday residency, peak memory, mixed DPI or no-leak proof.".into());
                    }
                    let env = glint_platform::desktop_environment()?;
                    report.insert("remote_session".into(), env.remote_session.into());
                    report.insert("display_count".into(), (env.displays.len() as i64).into());
                    report.insert("dpi_awareness".into(), i64::from(env.dpi_awareness).into());
                    report.insert(
                        "displays".into(),
                        env.displays
                            .iter()
                            .map(|display| {
                                let mut row = toml::Table::new();
                                row.insert(
                                    "bounds".into(),
                                    [
                                        display.bounds.left,
                                        display.bounds.top,
                                        display.bounds.right,
                                        display.bounds.bottom,
                                    ]
                                    .into_iter()
                                    .map(|n| toml::Value::Integer(i64::from(n)))
                                    .collect::<Vec<_>>()
                                    .into(),
                                );
                                row.insert(
                                    "effective_dpi".into(),
                                    vec![
                                        toml::Value::Integer(i64::from(display.effective_dpi.0)),
                                        toml::Value::Integer(i64::from(display.effective_dpi.1)),
                                    ]
                                    .into(),
                                );
                                row.into()
                            })
                            .collect::<Vec<toml::Value>>()
                            .into(),
                    );
                    if self.endurance {
                        // Parse only after the last measured point, outside the
                        // resource trend. During the run storage is fixed-size.
                        let mut stored: toml::Table =
                            toml::from_str(&self.serialized_samples).map_err(|e| e.to_string())?;
                        report.insert(
                            "samples".into(),
                            stored
                                .remove("samples")
                                .ok_or("Missing endurance samples")?,
                        );
                    } else {
                        report.insert("samples".into(), std::mem::take(&mut self.samples).into());
                    }
                    let text = toml::to_string_pretty(&report).map_err(|e| e.to_string())?;
                    use std::io::Write;
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(self.directory.join("resources.toml"))
                        .map_err(|e| e.to_string())?;
                    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
                    file.sync_all().map_err(|e| e.to_string())?;
                    if self.directory.join("config.toml").exists() {
                        return Err("Resource probe unexpectedly persisted settings".into());
                    }
                    eprintln!(
                        "Actual resident/capture/edit/PNG resource probe passed {} rounds; endurance={}; boundary samples only, no physical/peak/leak/environment acceptance claim",
                        self.rounds, self.endurance
                    );
                    ui.invoke_action("quit".into());
                    return Ok(None);
                }
            }
            7 => {
                self.mark("frozen_overlay_settle")?;
                self.phase = 3;
                return Ok(Some(IDLE));
            }
            8 => {
                self.mark("edited_overlay_settle")?;
                if self.local_4k && self.round == 1 {
                    let state = self.app.borrow();
                    let capture = state.capture.as_ref().ok_or("4K preview closed")?;
                    let pixels = capture
                        .window()
                        .take_snapshot()
                        .map_err(|e| e.to_string())?;
                    image::save_buffer_with_format(
                        self.directory.join("preview-4k.png"),
                        pixels.as_bytes(),
                        pixels.width(),
                        pixels.height(),
                        image::ColorType::Rgba8,
                        image::ImageFormat::Png,
                    )
                    .map_err(|e| e.to_string())?;
                    // Keep the diagnostic snapshot encoding out of idle CPU.
                    self.before = glint_platform::process_resources()?;
                    self.input_tick = glint_platform::last_session_input_tick()?;
                    self.clock = Instant::now();
                }
                self.phase = 4;
                return Ok(Some(IDLE));
            }
            9 => {
                self.mark("feedback_release_settle")?;
                self.phase = 6;
                return Ok(Some(IDLE));
            }
            10 => {
                let Some(result) = self.pin_ready.borrow_mut().take() else {
                    if self.clock.elapsed() > Duration::from_secs(20) {
                        return Err("Endurance pin readiness timed out".into());
                    }
                    return Ok(Some(Duration::from_millis(20)));
                };
                let id = result?;
                if Some(id) != self.pin_id {
                    return Err("Endurance pin completion identity mismatch".into());
                }
                let manager = self.app.borrow().pins.clone();
                let pin = manager
                    .borrow()
                    .window(id)
                    .ok_or("Endurance pin vanished")?
                    .clone_strong();
                pin.invoke_action("rotate".into());
                self.phase = 11;
            }
            11 => {
                let manager = self.app.borrow().pins.clone();
                let id = self.pin_id.ok_or("Missing endurance pin")?;
                let pin = manager
                    .borrow()
                    .window(id)
                    .ok_or("Endurance pin vanished")?
                    .clone_strong();
                if pin.get_busy() || pin.get_quarter_turns() != 1 {
                    if self.clock.elapsed() > Duration::from_secs(20) {
                        return Err("Endurance rotation timed out".into());
                    }
                    return Ok(Some(Duration::from_millis(20)));
                }
                let source = self
                    .pin_expected
                    .take()
                    .ok_or("Missing endurance source pixels")?;
                let rotated = pin
                    .get_picture()
                    .to_rgba8()
                    .ok_or("Missing endurance rotated pixels")?;
                if (rotated.width(), rotated.height()) != (source.height(), source.width()) {
                    return Err("Endurance rotated dimensions changed".into());
                }
                for y in 0..source.height() {
                    for x in 0..source.width() {
                        let a = (y as usize * source.width() as usize + x as usize) * 4;
                        let b = (x as usize * rotated.width() as usize
                            + (source.height() - 1 - y) as usize)
                            * 4;
                        if source.as_bytes()[a..a + 4] != rotated.as_bytes()[b..b + 4] {
                            return Err("Endurance rotation changed source pixel".into());
                        }
                    }
                }
                drop(rotated);
                drop(source);
                drop(pin.window().take_snapshot().map_err(|e| e.to_string())?);
                drop(pin);
                self.mark("pin_create_rotate_render")?;
                manager.borrow_mut().close(id)?;
                self.pin_id = None;
                if manager.borrow().count() != 0 || manager.borrow().used() != 0 {
                    return Err("Endurance close retained pin budget".into());
                }
                self.mark("pin_close_release")?;
                self.phase = 9;
                return Ok(Some(Duration::from_millis(3500)));
            }
            _ => unreachable!(),
        }
        Ok(Some(Duration::from_millis(20)))
    }
}
fn tick(run: Rc<RefCell<Run>>) {
    let result = run.borrow_mut().step();
    match result {
        Ok(Some(delay)) => slint::Timer::single_shot(delay, move || tick(run)),
        Ok(None) => {}
        Err(error) => {
            *run.borrow().error.borrow_mut() = Some(format!("Resource probe: {error}"));
            let _ = slint::quit_event_loop();
        }
    }
}
