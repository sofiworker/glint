//! Shared native capture adapter and independent M0 validation entry points.
use crate::messages::{self, Message};
use glint_core::{
    Annotation, Edits, Pixelation, Point, Rect, Redaction, SelectionGesture, SelectionHit, Shape,
    ShapeKind, Stroke, desktop_region, first_region_at, selection_hit,
};
use slint::winit_030::winit;
use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};

static OUTPUT_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(crate) struct OutputSlot;
impl OutputSlot {
    pub(crate) fn acquire() -> Option<Self> {
        if OUTPUT_BUSY.swap(true, std::sync::atomic::Ordering::AcqRel) {
            None
        } else {
            Some(Self)
        }
    }
}
impl Drop for OutputSlot {
    fn drop(&mut self) {
        OUTPUT_BUSY.store(false, std::sync::atomic::Ordering::Release);
    }
}
#[path = "preview.rs"]
mod preview;
use preview::Preview;
#[path = "brush_smoke.rs"]
mod brush_smoke;
#[path = "capture_export.rs"]
mod export;
#[path = "capture_output.rs"]
pub(crate) mod output;
#[path = "capture_style.rs"]
mod style;
#[path = "capture_toolbar.rs"]
mod toolbar;
pub use style::Presentation;
pub(crate) use style::apply_system_theme;
pub use toolbar::configure as configure_toolbar;
#[cfg(windows)]
#[path = "object_smoke.rs"]
mod object_smoke;
#[path = "capture_objects.rs"]
mod objects;
#[cfg(windows)]
pub(crate) fn verify_objects(directory: &std::path::Path) -> Result<(), String> {
    object_smoke::verify(directory)
}
#[cfg(windows)]
pub(crate) fn start_save_preparation_probe(
    ui: &CaptureProbe,
    directory: &std::path::Path,
) -> Result<(), String> {
    object_smoke::start_save_preparation(ui, directory)
}
#[path = "capture_magnifier.rs"]
mod magnifier;
#[path = "capture_picker.rs"]
mod picker;
#[path = "capture_text.rs"]
mod text_entry;

slint::slint! { export { Motion } from "src/widgets/motion.slint"; export { CaptureProbe, CaptureExit, WindowRegion, LayerTile, CaptureLabels, ObjectGrip } from "src/capture.slint"; export { ToolbarRow } from "src/toolbar_editor.slint"; }

#[derive(Clone)]
pub struct PinPayload {
    pub document: Arc<glint_core::ImageDocument>,
    pub pixels: SharedPixelBuffer<Rgba8Pixel>,
    pub display: SharedPixelBuffer<Rgba8Pixel>,
    pub turns: i32,
}
pub type PinPayloadSlot = Arc<std::sync::Mutex<Option<PinPayload>>>;
pub fn accept_pin_output(ui: &CaptureProbe) {
    let r = ui.get_pending_region();
    ui.invoke_output_succeeded(r.left as i32, r.top as i32, r.wide as i32, r.high as i32);
}

fn text(ui: &CaptureProbe, id: Message) -> &'static str {
    messages::text(id, ui.get_english())
}

#[derive(Default)]
struct Session {
    fixed_selection: bool,
    loupe_point: Option<(u32, u32)>,
    picker_pressed: bool,
    pin_payload: PinPayloadSlot,
    output: glint_core::config::Output,
    output_receipt: Arc<std::sync::Mutex<export::Receipt>>,
    object_selected: Option<usize>,
    object_drag: Option<objects::Drag>,
    object_preview: Option<Edits>,
    text_anchor: Option<(u32, u32)>,
    text_target: Option<usize>,
    text_generation: u64,
    text_result: text_entry::ResultSlot,
    source: Option<Arc<glint_platform::DesktopFrame>>,
    preview_origin: (u32, u32),
    preview_scene: bool,
    source_origin: (i32, i32),
    selection: Rect,
    edits: Edits,
    start: Option<(u32, u32)>,
    selection_drag: Option<SelectionGesture>,
    // Object/history references only: never snapshot the full desktop pixels.
    selection_before: Option<(Rect, Edits)>,
    windows: Vec<Rect>,
    window_hover: Option<Rect>,
    window_click: Option<Rect>,
    detection_error: Option<String>,
    draft: Option<Annotation>,
    preview: Preview,
    preview_error: Option<&'static str>,
}

fn sync_geometry(ui: &CaptureProbe, state: &Session) {
    let actual = state.selection;
    let r = if actual.width == 0 || actual.height == 0 {
        state.window_hover.unwrap_or(actual)
    } else {
        actual
    };
    ui.set_left(r.x as f32);
    ui.set_top(r.y as f32);
    ui.set_wide(r.width as f32);
    ui.set_high(r.height as f32);
    ui.set_selected(actual.width > 0 && actual.height > 0);
    ui.set_has_region(r.width > 0 && r.height > 0);
    ui.set_region_metrics(if state.fixed_selection {
        format!("{} × {} px", r.width, r.height).into()
    } else {
        format!(
            "{} × {} px · ({}, {})",
            r.width,
            r.height,
            i64::from(state.source_origin.0) + i64::from(r.x),
            i64::from(state.source_origin.1) + i64::from(r.y),
        )
        .into()
    });
}

fn sync(ui: &CaptureProbe, state: &Session) {
    sync_geometry(ui, state);
    let actual = state.selection;
    let r = if actual.width == 0 || actual.height == 0 {
        state.window_hover.unwrap_or(actual)
    } else {
        actual
    };
    ui.set_object_count(state.edits.marks().len() as i32);
    ui.set_can_undo(state.edits.can_undo());
    ui.set_can_redo(state.edits.can_redo());
    let object = state
        .object_selected
        .and_then(|i| {
            state
                .object_preview
                .as_ref()
                .unwrap_or(&state.edits)
                .marks()
                .get(i)
        })
        .map(|m| m.bounds());
    ui.set_object_selected(object.is_some());
    objects::sync_style(ui, state);
    objects::sync_grips(ui, state);
    let b = object.unwrap_or_default().intersection(Rect {
        x: 0,
        y: 0,
        width: actual.width,
        height: actual.height,
    });
    ui.set_object_left(b.x as f32);
    ui.set_object_top(b.y as f32);
    ui.set_object_wide(b.width as f32);
    ui.set_object_high(b.height as f32);
    ui.set_output_ready(state.preview_error.is_none());
    if let Some(error) = state.preview_error {
        ui.set_status(if error == preview::BUDGET_ERROR {
            text(ui, Message::CapturePreviewBudget).into()
        } else {
            format!("{}: {error}", text(ui, Message::CapturePreviewFailed)).into()
        });
        return;
    }
    if !ui.get_selected() {
        ui.set_status(
            if let Some(error) = &state.detection_error {
                format!("{}: {error}", text(ui, Message::CaptureWindowUnavailable))
            } else if ui.get_has_region() && ui.get_dimensions() {
                format!(
                    "{} × {} px · {}",
                    r.width,
                    r.height,
                    text(ui, Message::CaptureWindowHint)
                )
            } else {
                text(
                    ui,
                    if ui.get_has_region() {
                        Message::CaptureWindowHint
                    } else {
                        Message::CaptureDragHint
                    },
                )
                .into()
            }
            .into(),
        );
        return;
    }
    if !ui.get_dimensions() {
        ui.set_status(
            text(
                ui,
                if state.fixed_selection {
                    Message::PinEditTitle
                } else if state.output.finish == glint_core::config::FinishAction::SaveAs {
                    Message::CaptureEditHintSave
                } else {
                    Message::CaptureEditHint
                },
            )
            .into(),
        );
        return;
    }
    if state.fixed_selection {
        ui.set_status(
            format!(
                "{} × {} px · {}",
                r.width,
                r.height,
                text(
                    ui,
                    if ui.get_local_image() {
                        Message::LocalTitle
                    } else {
                        Message::PinEditTitle
                    }
                )
            )
            .into(),
        );
        return;
    }
    ui.set_status(
        format!(
            "{} × {} px · ({}, {}) · {}{}",
            r.width,
            r.height,
            i64::from(state.source_origin.0) + i64::from(r.x),
            i64::from(state.source_origin.1) + i64::from(r.y),
            text(
                ui,
                if state.output.finish == glint_core::config::FinishAction::SaveAs {
                    Message::CaptureEditHintSave
                } else {
                    Message::CaptureEditHint
                }
            ),
            if ui.get_diagnostic() { " · M0" } else { "" }
        )
        .into(),
    );
}

fn refresh(state: &mut Session, damage: Rect, dimensions: (u32, u32)) {
    let edits = state.object_preview.as_ref().unwrap_or(&state.edits);
    let scene = edits
        .marks()
        .iter()
        .map(|m| m.as_ref())
        .chain(state.draft.as_ref())
        .any(|m| matches!(m, Annotation::Pixelation(_)));
    let origin = (state.selection.x, state.selection.y);
    // Switching layer mode or moving a source-dependent filter changes all visible tiles.
    let damage = if scene != state.preview_scene || (scene && origin != state.preview_origin) {
        edits
            .marks()
            .iter()
            .map(|m| m.bounds())
            .chain(state.draft.as_ref().map(Annotation::bounds))
            .fold(damage, Rect::union)
    } else {
        damage
    };
    let source = scene
        .then_some(state.source.as_ref())
        .flatten()
        .map(|frame| glint_core::SceneSource {
            rgba: &frame.rgba,
            width: frame.width,
            height: frame.height,
            origin,
        });
    state.preview_error = state
        .preview
        .refresh_scene(edits, state.draft.as_ref(), damage, dimensions, source)
        .err();
    if state.preview_error.is_none() {
        state.preview_scene = scene;
        state.preview_origin = origin;
    }
}

fn save_png_new(
    path: &std::path::Path,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<(), String> {
    output::save_new(
        path,
        pixels,
        width,
        height,
        glint_core::config::ImageFormat::Png,
        90,
    )
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(windows)]
    if matches!(
        args.first().map(String::as_str),
        Some("--m0-pin" | "--m0-pin-smoke" | "--m0-pin-conflict-smoke")
    ) {
        return crate::pin_probe::run(&args);
    }
    if !matches!(
        args.first().map(String::as_str),
        Some(
            "--m0-overlay"
                | "--m0-capture"
                | "--m0-smoke"
                | "--m0-4k-smoke"
                | "--m0-brush-smoke"
                | "--m0-pixelation-smoke"
                | "--m0-environment"
        )
    ) {
        println!(
            "Glint M0: --m0-overlay [output.png] / --m0-capture output.png / --m0-smoke snapshot.png [observe-ms] / --m0-4k-smoke snapshot.png [observe-ms] / --m0-brush-smoke snapshot.png [samples] / --m0-environment [iterations]。尚非完整常驻应用。"
        );
        return Ok(());
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_window_attributes_hook(|attributes| {
            attributes
                .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
                .with_active(true)
        })
        .select()?;
    // Create winit first so Windows DPI awareness is established before physical GDI metrics.
    let ui = CaptureProbe::new()?;
    #[cfg(windows)]
    if args[0] == "--m0-environment" {
        let iterations = args
            .get(1)
            .map(|s| s.parse::<usize>())
            .transpose()?
            .unwrap_or(32)
            .clamp(1, 1000);
        return crate::environment_probe::run(iterations);
    }
    let begin = Instant::now();
    let frame = Arc::new(
        if matches!(
            args[0].as_str(),
            "--m0-4k-smoke" | "--m0-brush-smoke" | "--m0-pixelation-smoke"
        ) {
            let mut rgba = vec![0; 3840 * 2160 * 4];
            for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                *pixel = [(index % 251) as u8, 180, 220, 255];
            }
            glint_platform::DesktopFrame {
                x: 0,
                y: 0,
                width: 3840,
                height: 2160,
                rgba,
            }
        } else {
            glint_platform::capture_desktop().map_err(std::io::Error::other)?
        },
    );
    eprintln!(
        "Source (GDI desktop or explicit synthetic 4K mode): {}x{}, origin ({}, {}), {} bytes, {:.2} ms",
        frame.width,
        frame.height,
        frame.x,
        frame.y,
        frame.rgba.len(),
        begin.elapsed().as_secs_f64() * 1000.0
    );
    if args[0] == "--m0-capture" {
        let path = args
            .get(1)
            .ok_or("--m0-capture requires an explicit PNG path")?;
        save_png_new(
            std::path::Path::new(path),
            &frame.rgba,
            frame.width,
            frame.height,
        )
        .map_err(std::io::Error::other)?;
        return Ok(());
    }
    let smoke_source = frame.clone();
    let state = prepare_capture(
        &ui,
        frame,
        128,
        args.get(1).map(std::path::PathBuf::from),
        || {
            let _ = slint::quit_event_loop();
        },
    );
    let brush_state = state;
    ui.show()?;
    let smoke_error = Rc::new(RefCell::new(None));
    if args[0] == "--m0-brush-smoke" || args[0] == "--m0-pixelation-smoke" {
        if args[0] == "--m0-pixelation-smoke" {
            ui.set_tool(7);
        }
        let path = args
            .get(1)
            .ok_or("--m0-brush-smoke requires snapshot.png")?
            .clone();
        let samples = args
            .get(2)
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(120)
            .clamp(16, 2000);
        brush_smoke::install(
            &ui,
            brush_state,
            smoke_source.clone(),
            path,
            samples,
            smoke_error.clone(),
        );
    }
    if args[0] == "--m0-smoke" || args[0] == "--m0-4k-smoke" {
        let path = args
            .get(1)
            .ok_or("--m0-smoke requires snapshot.png")?
            .clone();
        let weak = ui.as_weak();
        let error = smoke_error.clone();
        let observe_ms = args
            .get(2)
            .map(|s| s.parse::<u64>())
            .transpose()?
            .unwrap_or(500)
            .clamp(500, 30_000);
        slint::Timer::single_shot(std::time::Duration::from_millis(observe_ms), move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let smoke_begin = Instant::now();
            // Same adapter as pointer events; not evidence of OS input routing.
            let w = smoke_source.width as f32;
            let h = smoke_source.height as f32;
            ui.invoke_pointer(0, w * 0.1, h * 0.15);
            ui.invoke_pointer(1, w * 0.65, h * 0.65);
            ui.invoke_pointer(2, w * 0.65, h * 0.65);
            ui.set_tool(1);
            ui.invoke_pointer(0, w * 0.2, h * 0.3);
            ui.invoke_pointer(1, w * 0.4, h * 0.4);
            ui.invoke_pointer(2, w * 0.4, h * 0.4);
            assert_eq!(ui.get_object_count(), 1);
            ui.invoke_action("undo".into());
            assert_eq!(ui.get_object_count(), 0);
            ui.invoke_action("redo".into());
            assert_eq!(ui.get_object_count(), 1);
            eprintln!(
                "M0 selection/redaction/undo/redo adapter: {:.2} ms",
                smoke_begin.elapsed().as_secs_f64() * 1000.0
            );
            let render_begin = Instant::now();
            let result = ui
                .window()
                .take_snapshot()
                .map_err(|e| e.to_string())
                .and_then(|pixels| {
                    eprintln!(
                        "M0 software snapshot render: {:.2} ms",
                        render_begin.elapsed().as_secs_f64() * 1000.0
                    );
                    if pixels.width() != smoke_source.width
                        || pixels.height() != smoke_source.height
                    {
                        return Err(format!(
                            "Snapshot/source size mismatch: {}x{} vs {}x{}",
                            pixels.width(),
                            pixels.height(),
                            smoke_source.width,
                            smoke_source.height
                        ));
                    }
                    let sample = |fx: f32, fy: f32| {
                        let x = (w * fx) as usize;
                        let y = (h * fy) as usize;
                        let offset = (y * smoke_source.width as usize + x) * 4;
                        (
                            &pixels.as_bytes()[offset..offset + 4],
                            &smoke_source.rgba[offset..offset + 4],
                        )
                    };
                    if sample(0.3, 0.35).0 != [32, 43, 66, 255] {
                        return Err("Redaction render differs from exported opaque color".into());
                    }
                    let (rendered, source) = sample(0.5, 0.5);
                    if rendered != source {
                        return Err("Selected interior does not match source pixels".into());
                    }
                    // Check all four mask regions to catch default item-centering regressions.
                    for (x, y) in [(0.8, 0.1), (0.8, 0.8), (0.05, 0.5), (0.8, 0.5)] {
                        let (rendered, source) = sample(x, y);
                        for c in 0..3 {
                            let expected = ((source[c] as f32 * 122.0
                                + [10.0, 20.0, 39.0][c] * 133.0)
                                / 255.0)
                                .round() as i16;
                            if (i16::from(rendered[c]) - expected).abs() > 2 {
                                return Err("Outside mask pixel mismatch".into());
                            }
                        }
                    }
                    image::save_buffer_with_format(
                        &path,
                        pixels.as_bytes(),
                        pixels.width(),
                        pixels.height(),
                        image::ColorType::Rgba8,
                        image::ImageFormat::Png,
                    )
                    .map_err(|e| e.to_string())
                });
            eprintln!(
                "M0 software render + pixel checks + PNG: {result:?}, {:.2} ms",
                render_begin.elapsed().as_secs_f64() * 1000.0
            );
            if let Err(message) = result {
                *error.borrow_mut() = Some(message);
            }
            let _ = slint::quit_event_loop();
        });
    }
    slint::run_event_loop()?;
    if let Some(error) = smoke_error.borrow_mut().take() {
        return Err(std::io::Error::other(error).into());
    }
    Ok(())
}

/// Creates a capture window in the existing Slint event loop. The owner retains the returned
/// component until completion; dropping it releases the frozen source and annotation tiles.
#[cfg(windows)]
pub fn resident_capture(
    snapshot: glint_platform::DesktopSnapshot,
    window_mode: bool,
    presentation: Presentation,
    output_path: Option<std::path::PathBuf>,
    complete: impl Fn() + 'static,
) -> Result<CaptureProbe, String> {
    resident_capture_document(snapshot, window_mode, presentation, output_path, complete)
        .map(|(ui, _)| ui)
}

#[cfg(windows)]
pub fn resident_capture_document(
    snapshot: glint_platform::DesktopSnapshot,
    window_mode: bool,
    presentation: Presentation,
    output_path: Option<std::path::PathBuf>,
    complete: impl Fn() + 'static,
) -> Result<(CaptureProbe, PinPayloadSlot), String> {
    resident_capture_region(
        snapshot,
        window_mode,
        None,
        presentation,
        output_path,
        complete,
    )
}
#[cfg(windows)]
pub fn resident_color_picker(
    snapshot: glint_platform::DesktopSnapshot,
    presentation: Presentation,
    complete: impl Fn() + 'static,
) -> Result<(CaptureProbe, PinPayloadSlot), String> {
    let region = Rect {
        x: 0,
        y: 0,
        width: snapshot.frame.width,
        height: snapshot.frame.height,
    };
    let pointer = snapshot.pointer.map(|(x, y)| {
        (
            (i64::from(x) - i64::from(snapshot.frame.x)) as f32,
            (i64::from(y) - i64::from(snapshot.frame.y)) as f32,
        )
    });
    let (ui, payload) =
        resident_capture_region(snapshot, false, Some(region), presentation, None, complete)?;
    ui.set_color_only(true);
    ui.set_tool(6);
    ui.set_pin_enabled(false);
    ui.set_status(text(&ui, Message::ColorPickerHint).into());
    if let Some((x, y)) = pointer {
        ui.invoke_hover(x, y);
    }
    Ok((ui, payload))
}
#[cfg(windows)]
pub fn resident_capture_region(
    snapshot: glint_platform::DesktopSnapshot,
    window_mode: bool,
    initial_region: Option<Rect>,
    presentation: Presentation,
    output_path: Option<std::path::PathBuf>,
    complete: impl Fn() + 'static,
) -> Result<(CaptureProbe, PinPayloadSlot), String> {
    let ui = crate::motion::create_capture(CaptureProbe::new).map_err(|e| e.to_string())?;
    ui.set_diagnostic(false);
    let output = presentation.output.clone();
    let preview_budget = presentation.preview_budget_mib;
    presentation.apply(&ui);
    let frame = snapshot.frame;
    let origin = (frame.x, frame.y);
    let dimensions = (frame.width, frame.height);

    let state = prepare_capture(&ui, Arc::new(frame), preview_budget, output_path, complete);
    {
        let mut state = state.borrow_mut();
        state.output = output;
        state.windows = snapshot
            .windows
            .iter()
            .map(|w| {
                desktop_region(
                    (w.bounds.left, w.bounds.top, w.bounds.right, w.bounds.bottom),
                    origin,
                    dimensions,
                )
            })
            .filter(|r| r.width > 0 && r.height > 0)
            .collect();
        state.detection_error = snapshot.detection_error;
        ui.set_window_regions(ModelRc::from(Rc::new(slint::VecModel::from(
            state
                .windows
                .iter()
                .map(|r| WindowRegion {
                    left: r.x as f32,
                    top: r.y as f32,
                    wide: r.width as f32,
                    high: r.height as f32,
                })
                .collect::<Vec<_>>(),
        ))));
        state.window_hover = snapshot.pointer.and_then(|(x, y)| {
            let x = i64::from(x) - i64::from(origin.0);
            let y = i64::from(y) - i64::from(origin.1);
            if x < 0 || y < 0 || x >= i64::from(dimensions.0) || y >= i64::from(dimensions.1) {
                None
            } else {
                first_region_at(&state.windows, (x as u32, y as u32))
            }
        });
        // Window capture starts with a hover preview, not a committed region.
        // Committing here prevents hover updates for every subsequent window.
        if window_mode && state.windows.is_empty() && state.detection_error.is_none() {
            state.detection_error = Some("No window detected; drag to select a region".into());
        }
        if let Some(region) = initial_region {
            if region.width == 0
                || region.height == 0
                || region
                    .x
                    .checked_add(region.width)
                    .is_none_or(|x| x > dimensions.0)
                || region
                    .y
                    .checked_add(region.height)
                    .is_none_or(|y| y > dimensions.1)
            {
                return Err("Repeated region is outside source pixels".into());
            }
            state.selection = region;
            state.window_hover = None;
        }
        sync(&ui, &state);
    }
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_action("cancel-window".into());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let payload = state.borrow().pin_payload.clone();
    Ok((ui, payload))
}

#[cfg(windows)]
pub fn resident_pin_editor(
    local: bool,
    prepared: crate::pin_edit::Prepared,
    presentation: Presentation,
    output_path: Option<std::path::PathBuf>,
    complete: impl Fn() + 'static,
) -> Result<(CaptureProbe, PinPayloadSlot), String> {
    let ui = crate::motion::create_capture(CaptureProbe::new).map_err(|e| e.to_string())?;
    ui.set_local_image(local);
    ui.set_diagnostic(false);
    let output = presentation.output.clone();
    let preview_budget = presentation.preview_budget_mib;
    presentation.apply(&ui);
    let dimensions = (prepared.frame.width, prepared.frame.height);
    let state = prepare_capture(
        &ui,
        Arc::new(prepared.frame),
        preview_budget,
        output_path,
        complete,
    );
    let payload = {
        let mut state = state.borrow_mut();
        state.fixed_selection = true;
        state.output = output;
        state.selection = prepared.selection;
        state.edits = prepared.edits;
        let dirty = Rect {
            x: 0,
            y: 0,
            width: prepared.selection.width,
            height: prepared.selection.height,
        };
        refresh(&mut state, dirty, dimensions);
        sync(&ui, &state);
        state.pin_payload.clone()
    };
    ui.set_pin_turns(prepared.turns);
    ui.set_pin_editing(true);
    let mut labels = ui.get_labels();
    if local {
        labels.title = messages::text(Message::LocalTitle, ui.get_english()).into();
    }
    if !local {
        labels.pin = messages::text(Message::PinEditApply, ui.get_english()).into();
    }
    labels.select_hint = messages::text(Message::PinEditSelectHint, ui.get_english()).into();
    ui.set_labels(labels);
    let previous_notice = ui.get_appearance_notice();
    let hint = messages::text(
        if local {
            Message::LocalHint
        } else {
            Message::PinEditHint
        },
        ui.get_english(),
    );
    ui.set_appearance_notice(if previous_notice.is_empty() {
        hint.into()
    } else {
        format!("{previous_notice}\n{hint}").into()
    });
    ui.window().set_size(slint::PhysicalSize::new(
        prepared.window_size.0,
        prepared.window_size.1,
    ));
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_action("cancel-window".into());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    Ok((ui, payload))
}

fn selection_metrics(ui: &CaptureProbe, dimensions: (u32, u32)) -> ((u32, u32), (u32, u32)) {
    let physical = ui.window().size();
    let scale = ui.window().scale_factor();
    let ratio = (
        dimensions.0 as f32 * scale / physical.width.max(1) as f32,
        dimensions.1 as f32 * scale / physical.height.max(1) as f32,
    );
    (
        (
            (10.0 * ratio.0).ceil() as u32,
            (10.0 * ratio.1).ceil() as u32,
        ),
        (
            (30.0 * ratio.0).ceil() as u32,
            (30.0 * ratio.1).ceil() as u32,
        ),
    )
}

fn prepare_capture(
    ui: &CaptureProbe,
    frame: Arc<glint_platform::DesktopFrame>,
    preview_budget_mib: u16,
    output_path: Option<std::path::PathBuf>,
    complete: impl Fn() + 'static,
) -> Rc<RefCell<Session>> {
    #[cfg(windows)]
    crate::font_picker::install(ui);
    style::apply_labels(ui);
    toolbar::configure(ui, glint_core::config::Toolbar::default(), |_| {
        Err("Toolbar persistence is unavailable in this diagnostic session".into())
    });
    ui.invoke_apply_theme();
    ui.set_source_w(frame.width as f32);
    ui.set_source_h(frame.height as f32);
    ui.set_desktop(slint::Image::from_rgba8(
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&frame.rgba, frame.width, frame.height),
    ));
    ui.window()
        .set_size(slint::PhysicalSize::new(frame.width, frame.height));
    ui.window()
        .set_position(slint::PhysicalPosition::new(frame.x, frame.y));
    let state = Rc::new(RefCell::new(Session {
        preview: Preview::with_budget_mib(preview_budget_mib),
        source: Some(frame.clone()),
        source_origin: (frame.x, frame.y),
        ..Session::default()
    }));
    ui.set_tiles(ModelRc::from(state.borrow().preview.model.clone()));
    sync(ui, &state.borrow());
    let pointer_state = state.clone();
    let dimensions = (frame.width, frame.height);
    let style_state = state.clone();
    let weak = ui.as_weak();
    ui.on_object_style(move |color, width, filled| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_closed() || ui.get_output_error_open() {
                return;
            }
            objects::restyle(
                &ui,
                &mut style_state.borrow_mut(),
                color,
                width,
                filled,
                dimensions,
            );
        }
    });
    let weak = ui.as_weak();
    ui.on_canvas_left({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_loupe_valid(false);
            }
        }
    });
    let hover_state = state.clone();
    let hover_frame = frame.clone();
    ui.on_hover(move |x, y| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_closed() || ui.get_output_error_open() {
            return;
        }
        let metrics = selection_metrics(&ui, dimensions);
        let mut state = hover_state.borrow_mut();
        magnifier::update(&ui, &hover_frame, &mut state.loupe_point, x, y);
        if (ui.get_tool() == 6 && state.selection.width > 0 && state.selection.height > 0)
            || ui.get_color_only()
        {
            picker::preview(&ui, &hover_frame, state.selection, x, y);
            return;
        }
        if state.selection.width == 0 || state.selection.height == 0 {
            let candidate = first_region_at(&state.windows, (x.max(0.0) as u32, y.max(0.0) as u32));
            if candidate != state.window_hover {
                state.window_hover = candidate;
                sync(&ui, &state);
            }
        }
        if ui.get_tool() == 0
            && let Some(cursor) = objects::hover_cursor(&state, Point { x, y }, metrics.0)
        {
            ui.set_cursor(cursor);
            return;
        }
        if state.fixed_selection && ui.get_tool() == 0 {
            ui.set_cursor(0);
            return;
        }
        ui.set_cursor(
            match selection_hit(
                state.selection,
                (x.max(0.0) as u32, y.max(0.0) as u32),
                metrics.0,
            ) {
                SelectionHit::Outside => 0,
                SelectionHit::Inside => 1,
                SelectionHit::Edge(edges) => {
                    match (edges.left || edges.right, edges.top || edges.bottom) {
                        (true, true) => {
                            if edges.left == edges.top {
                                4
                            } else {
                                5
                            }
                        }
                        (true, false) => 2,
                        _ => 3,
                    }
                }
            },
        );
    });
    let weak = ui.as_weak();
    let picker_frame = frame.clone();
    // Only selection geometry is coalesced. Pen/annotation input must retain
    // every sample, and release/cancel must never wait for a queued movement.
    let selection_move = Rc::new(std::cell::Cell::new(None::<(f32, f32)>));
    let selection_timer = slint::Timer::default();
    ui.on_pointer(move |kind, x, y| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_closed() {
            return;
        }
        let mut s = pointer_state.borrow_mut();
        if kind == 1 && s.selection_drag.is_some() && s.draft.is_none() {
            selection_move.set(Some((x, y)));
            if !selection_timer.running() {
                let latest = selection_move.clone();
                let weak = ui.as_weak();
                selection_timer.start(
                    slint::TimerMode::SingleShot,
                    std::time::Duration::from_millis(16),
                    move || {
                        if let Some((x, y)) = latest.take()
                            && let Some(ui) = weak.upgrade()
                            && !ui.get_closed()
                        {
                            ui.invoke_pointer(4, x, y);
                        }
                    },
                );
            }
            return;
        }
        if matches!(kind, 0 | 2 | 3) {
            selection_timer.stop();
            selection_move.set(None);
        }
        let kind = if kind == 4 { 1 } else { kind };
        if kind == 3 {
            // Cancellation is not a release: discard only uncommitted work.
            // Handle it before disabled/modal/text guards, since losing input
            // while a control is disabled can itself produce this event.
            let damage = discard_pointer(&mut s);
            refresh(&mut s, damage, dimensions);
            sync(&ui, &s);
            return;
        }
        magnifier::update(&ui, &picker_frame, &mut s.loupe_point, x, y);
        if ui.get_text_editing() || ui.get_text_busy() || ui.get_toolbar_modal() > 0 || ui.get_output_error_open() { return; }
        if ui.get_color_only() || (ui.get_tool() == 6 && s.selection.width > 0 && s.selection.height > 0 && s.selection_drag.is_none()) {
            let selection = s.selection;
            picker::pointer(&ui, &picker_frame, selection, &mut s.picker_pressed, kind, (x,y));
            return;
        }
        s.picker_pressed = false;
        if ui.get_tool() == 8 && kind == 0 && x.is_finite() && y.is_finite()
            && x >= s.selection.x as f32 && y >= s.selection.y as f32
            && x < (s.selection.x+s.selection.width) as f32 && y < (s.selection.y+s.selection.height) as f32 {
            s.text_generation = s.text_generation.wrapping_add(1);
            s.text_anchor = Some((x as u32-s.selection.x,y as u32-s.selection.y));
            ui.set_text_anchor_x(x); ui.set_text_anchor_y(y);
            ui.set_text_value("".into());ui.set_text_feedback("".into());ui.set_text_editing(true);
            ui.invoke_focus_text();
            return;
        }
        let p = (
            x.max(0.0).min(dimensions.0 as f32) as u32,
            y.max(0.0).min(dimensions.1 as f32) as u32,
        );
        if objects::pointer(&ui,&mut s,kind,p,dimensions) {return;}
        if kind == 0 {
            s.start = Some(p);
            s.selection_drag = None;
            s.window_click = None;
            s.window_hover = None;
            let inside = s.selection.width > 0
                && p.0 >= s.selection.x
                && p.1 >= s.selection.y
                && p.0 < s.selection.x + s.selection.width
                && p.1 < s.selection.y + s.selection.height;
            s.draft = if inside && ui.get_tool() == 7 {
                Some(Pixelation { bounds: Rect::default() }.into())
            } else if inside && ui.get_tool() == 1 {
                Some(
                    Redaction {
                        bounds: Rect::default(),
                        color: [32, 43, 66, 255],
                    }
                    .into(),
                )
            } else if inside && ui.get_tool() == 2 {
                let color = ui.get_mark_color();
                Some(
                    Stroke {
                        points: vec![Point {
                            x: (x - s.selection.x as f32).max(0.0),
                            y: (y - s.selection.y as f32).max(0.0),
                        }],
                        width: ui.get_stroke_width() as f32,
                        color: [color.red(), color.green(), color.blue(), color.alpha()],
                    }
                    .into(),
                )
            } else if inside && (3..=5).contains(&ui.get_tool()) {
                let point = Point { x: (x - s.selection.x as f32).max(0.0), y: (y - s.selection.y as f32).max(0.0) };
                let color = ui.get_mark_color();
                Some(Shape {
                    kind: match ui.get_tool() {
                        3 => ShapeKind::Rectangle,
                        4 => ShapeKind::Arrow { filled: ui.get_filled_arrows() },
                        _ => ShapeKind::Highlight,
                    },
                    start: point, end: point, width: ui.get_stroke_width() as f32,
                    color: [color.red(), color.green(), color.blue(), color.alpha()],
                }.into())
            } else {
                None
            };
            if s.draft.is_none() {
                if s.fixed_selection { s.start=None; sync(&ui,&s); return; }
                let (tolerance, minimum) = selection_metrics(&ui, dimensions);
                let gesture = SelectionGesture::begin(if ui.get_tool() == 0 { s.selection } else { Rect::default() }, p, tolerance, dimensions, minimum);
                s.selection_before = Some((s.selection, s.edits.clone()));
                if gesture.hit == SelectionHit::Outside {
                    s.window_click = first_region_at(&s.windows,p);
                    s.edits = Edits::default();
                    s.preview.clear();
                    s.preview_error = None;
                    ui.set_tiles(ModelRc::from(s.preview.model.clone()));
                }
                s.selection_drag = Some(gesture);
            }
        }
        let Some(start) = s.start else {
            return;
        };
        if s.draft.is_some() {
            let selection = s.selection;
            let end = (
                p.0.clamp(selection.x, selection.x + selection.width) - selection.x,
                p.1.clamp(selection.y, selection.y + selection.height) - selection.y,
            );
            let mut filter_damage = None;
            let mut damage = match s.draft.as_mut().unwrap() {
                Annotation::Text(mark) => mark.bounds(), // Text commits through its inline editor.
                Annotation::Pixelation(mark) => {
                    let before = mark.bounds;
                    mark.bounds = Rect::between((start.0 - selection.x, start.1 - selection.y), end);
                    filter_damage = Some(Pixelation::changed_regions(before, mark.bounds));
                    before.union(mark.bounds)
                }
                Annotation::Shape(shape) => {
                    let before = shape.bounds();
                    shape.end = Point { x: end.0 as f32, y: end.1 as f32 };
                    before.union(shape.bounds())
                }
                Annotation::Redaction(mark) => {
                    let before = mark.bounds;
                    mark.bounds = Rect::between((start.0 - selection.x, start.1 - selection.y), end);
                    before.union(mark.bounds)
                }
                Annotation::Stroke(stroke) => {
                    let point = Point {
                        x: x.clamp(selection.x as f32, (selection.x + selection.width) as f32) - selection.x as f32,
                        y: y.clamp(selection.y as f32, (selection.y + selection.height) as f32) - selection.y as f32,
                    };
                    let previous = *stroke.points.last().unwrap();
                    if stroke.points.len() < 65536
                        && (previous.x != point.x || previous.y != point.y)
                    {
                        stroke.points.push(point);
                    }
                    Stroke::segment_bounds(previous, point, stroke.width)
                }
            };
            let mut rejected = false;
            let point_budget = matches!(s.draft.as_ref(), Some(Annotation::Stroke(stroke)) if stroke.points.len() >= 65536);
            if kind == 2 || point_budget {
                let mark = s.draft.take().unwrap();
                let bounds = mark.bounds();
                rejected = !s.edits.push(mark);
                // Accepted drafts already have the same preview pixels. Only rejection needs
                // to clear the entire stroke rather than refreshing its last segment.
                if rejected { damage = damage.union(bounds); }
                s.start = None;
            }
            if let Some(regions) = filter_damage.filter(|_| !rejected) {
                for region in regions { refresh(&mut s, region, dimensions); }
            } else {
                refresh(&mut s, damage, dimensions);
            }
            sync(&ui, &s);
            if rejected {
                ui.set_status(
                    text(&ui, Message::CaptureMarkRejected).into(),
                );
            } else if point_budget {
                ui.set_status(text(&ui, Message::CapturePointBudget).into());
            }
        } else {
            if let Some(gesture) = s.selection_drag { s.selection = gesture.update(p); }
            if kind == 2 {
                let tolerance = selection_metrics(&ui, dimensions).0;
                if start.0.abs_diff(p.0) <= (tolerance.0*2/5).max(1) && start.1.abs_diff(p.1) <= (tolerance.1*2/5).max(1)
                    && let Some(window) = s.window_click.take() { s.selection=window; }
                s.start = None;
                s.selection_drag = None;
                s.selection_before = None;
            }
            if s.preview_scene { refresh(&mut s, Rect::default(), dimensions); }
            if kind == 1 && s.object_selected.is_none() {
                sync_geometry(&ui, &s);
            } else {
                sync(&ui, &s);
            }
        }
    });
    let weak = ui.as_weak();
    let finished = std::cell::Cell::new(false);
    let finished_ui = ui.as_weak();
    ui.on_finished(move || {
        if !finished.replace(true) {
            if let Some(ui) = finished_ui.upgrade() {
                if ui.get_exit_reason() == CaptureExit::Open {
                    ui.set_exit_reason(match ui.get_completion_kind() {
                        1 => CaptureExit::Copy,
                        2 => CaptureExit::Save,
                        3 => CaptureExit::Pin,
                        4 => CaptureExit::ColorCopy,
                        5 => CaptureExit::PinUpdate,
                        _ => CaptureExit::Direct,
                    });
                }
                ui.set_closed(true);
                ui.invoke_closed_event();
                let _ = ui.hide();
            }
            complete();
        }
    });
    let action_state = state.clone();
    ui.on_action(move |action| {
        let state = &action_state;
        let Some(ui) = weak.upgrade() else {
            return;
        };
        // A queued event may retain the old component after its owner closes it.
        // A finished session cannot edit, acquire an output slot, or export again.
        if ui.get_closed() {
            return;
        }
        let action = if ui.get_output_error_open() {
            match action.as_str() {
                "dismiss-output-error"
                | "cancel"
                | "cancel-escape"
                | "cancel-pointer"
                | "cancel-toolbar"
                | "cancel-window" => {
                    ui.set_output_error_open(false);
                    ui.invoke_focus_canvas();
                    return;
                }
                "retry-output" => {
                    ui.set_output_error_open(false);
                    ui.invoke_focus_canvas();
                    ui.get_output_retry_action()
                }
                "copy" | "save" | "save-without-auto" | "pin" | "copy-and-pin" => {
                    ui.set_output_error_open(false);
                    ui.invoke_focus_canvas();
                    action
                }
                _ => return,
            }
        } else {
            action
        };
        let cancel_origin = match action.as_str() {
            "cancel" => Some(CaptureExit::ApiCancel),
            "cancel-escape" => Some(CaptureExit::EscapeKey),
            "cancel-pointer" => Some(CaptureExit::SecondaryClick),
            "cancel-toolbar" => Some(CaptureExit::ToolbarCancel),
            "cancel-window" => Some(CaptureExit::WindowRequest),
            _ => None,
        };
        let action = if cancel_origin.is_some() {
            slint::SharedString::from("cancel")
        } else {
            action
        };
        if ui.get_font_open() {
            if action == "cancel" {
                ui.invoke_font_command("cancel".into(), "".into());
            }
            return;
        }
        if ui.get_toolbar_modal() > 0 {
            if action == "cancel" {
                ui.invoke_toolbar_command("cancel".into(), 0, 0);
            }
            return;
        }
        if action == "cancel" {
            if ui.get_text_editing() {
                text_entry::cancel(&ui, &mut state.borrow_mut());
                return;
            }
            if !ui.get_closed() {
                ui.set_exit_reason(cancel_origin.unwrap_or(CaptureExit::ApiCancel));
            }
            ui.invoke_finished();
            return;
        }
        if ui.get_color_only() {
            if action == "finish" || action == "copy-color-hex" || action == "copy-color-rgb" {
                state.borrow_mut().picker_pressed = false;
                picker::copy(&ui, action == "copy-color-rgb");
            }
            return;
        }
        if action == "cancel-text" {
            text_entry::cancel(&ui, &mut state.borrow_mut());
            return;
        }
        if action == "commit-text" {
            text_entry::commit(&ui, &mut state.borrow_mut());
            return;
        }
        if action == "text-ready" {
            text_entry::ready(&ui, &mut state.borrow_mut(), dimensions);
            return;
        }
        if ui.get_text_editing() {
            return;
        }
        if ui.get_busy() {
            return;
        }
        if action == "deselect-object" {
            let mut s = state.borrow_mut();
            let dirty = discard_pointer(&mut s).union(objects::clear(&mut s));
            let dirty = objects::damage(&s, dirty);
            refresh(&mut s, dirty, dimensions);
            sync(&ui, &s);
            return;
        }
        if action == "delete-object" {
            objects::delete(&ui, &mut state.borrow_mut(), dimensions);
            return;
        }
        if action == "edit-object-text" {
            text_entry::edit(&ui, &mut state.borrow_mut());
            return;
        }
        if action == "copy-color-hex" || action == "copy-color-rgb" {
            state.borrow_mut().picker_pressed = false;
            picker::copy(&ui, action == "copy-color-rgb");
            return;
        }
        if action == "undo" || action == "redo" {
            let mut s = state.borrow_mut();
            let preview_damage = discard_pointer(&mut s).union(objects::clear(&mut s));
            let damage = if action == "undo" {
                s.edits.undo()
            } else {
                s.edits.redo()
            };
            let damage = objects::damage(&s, damage.union(preview_damage));
            refresh(&mut s, damage, dimensions);
            sync(&ui, &s);
            return;
        }
        let s = state.borrow();
        if pointer_pending(&s) {
            ui.set_status(text(&ui, Message::CaptureGesturePending).into());
            return;
        }
        if s.preview_error.is_some() {
            sync(&ui, &s);
            return;
        }
        if s.selection.width == 0 || s.selection.height == 0 {
            return;
        }
        let skip_automatic = action == "save-without-auto" && ui.get_auto_save_failed();
        let action = match action.as_str() {
            "finish" => match s.output.finish {
                glint_core::config::FinishAction::Copy => "copy",
                glint_core::config::FinishAction::SaveAs => "save",
                glint_core::config::FinishAction::CopyAndPin => "copy-and-pin",
            },
            "copy" => "copy",
            "save" => "save",
            "pin" => "pin",
            "copy-and-pin" => "copy-and-pin",
            "save-without-auto" if skip_automatic => "save",
            _ => return,
        };
        if matches!(action, "pin" | "copy-and-pin") && !ui.get_pin_enabled() {
            ui.set_status(text(&ui, Message::CapturePinUnavailable).into());
            return;
        }
        if action == "save" && output_path.is_none() && ui.get_diagnostic() {
            ui.set_status(text(&ui, Message::CaptureProbePath).into());
            return;
        }
        // Bound output work across sessions too: Esc can close an editor while its worker finishes.
        let Some(output_slot) = OutputSlot::acquire() else {
            ui.set_status(text(&ui, Message::CaptureOutputPending).into());
            return;
        };
        export::start(
            &ui,
            export::Request {
                frame: frame.clone(),
                crop: s.selection,
                edits: s.edits.clone(),
                options: glint_core::config::Output {
                    auto_save: s.output.auto_save && !skip_automatic,
                    ..s.output.clone()
                },
                save: action == "save",
                pin: matches!(action, "pin" | "copy-and-pin"),
                copy: action != "pin",
                path: output_path.clone(),
                replacement: None,
                receipt: s.output_receipt.clone(),
                recover_automatic: skip_automatic,
                pin_payload: s.pin_payload.clone(),
                pin_turns: ui.get_pin_turns(),
            },
            output_slot,
        );
    });
    state
}

fn pointer_pending(s: &Session) -> bool {
    s.start.is_some()
        || s.draft.is_some()
        || s.object_drag.is_some()
        || s.selection_drag.is_some()
        || s.picker_pressed
}

fn discard_pointer(s: &mut Session) -> Rect {
    let mut damage = s
        .object_preview
        .as_ref()
        .map(|edits| {
            edits
                .marks()
                .iter()
                .map(|m| m.bounds())
                .fold(Rect::default(), Rect::union)
        })
        .unwrap_or_default()
        .union(s.draft.as_ref().map(Annotation::bounds).unwrap_or_default());
    if let Some((selection, edits)) = s.selection_before.take() {
        s.selection = selection;
        s.edits = edits;
        damage = s
            .edits
            .marks()
            .iter()
            .map(|m| m.bounds())
            .fold(damage, Rect::union);
    }
    s.object_drag = None;
    s.object_preview = None;
    s.picker_pressed = false;
    s.draft = None;
    s.start = None;
    s.selection_drag = None;
    s.window_click = None;
    objects::damage(s, damage)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_capture_rejects_late_edit_and_output_events() {
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
        let ui = CaptureProbe::new().unwrap();
        let count = Rc::new(std::cell::Cell::new(0));
        let completed = count.clone();
        let frame = Arc::new(glint_platform::DesktopFrame {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
            rgba: vec![255; 64 * 64 * 4],
        });
        let state = prepare_capture(&ui, frame, 16, None, move || {
            completed.set(completed.get() + 1);
        });
        ui.invoke_pointer(0, 4., 4.);
        ui.invoke_pointer(2, 60., 60.);
        ui.set_tool(1);
        ui.invoke_pointer(0, 10., 10.);
        ui.invoke_pointer(2, 30., 30.);
        assert_eq!(state.borrow().edits.marks().len(), 1);
        let selection = state.borrow().selection;
        let marks = state.borrow().edits.marks().to_vec();
        ui.invoke_action("cancel-escape".into());
        assert!(ui.get_closed());
        // The old owner can survive in queued callbacks after the resident drops it.
        ui.invoke_action("undo".into());
        ui.invoke_pointer(0, 32., 32.);
        ui.invoke_pointer(2, 45., 45.);
        assert_eq!(state.borrow().selection, selection);
        assert_eq!(state.borrow().edits.marks(), marks);
        for action in ["copy", "save", "finish", "cancel-window"] {
            ui.invoke_action(action.into());
        }
        assert!(!ui.get_busy());
        assert_eq!(ui.get_completion_kind(), 0);
        assert_eq!(ui.get_exit_reason(), CaptureExit::EscapeKey);
        assert_eq!(count.get(), 1);
        assert!(OutputSlot::acquire().is_some());
    }
    #[test]
    fn flattened_png_roundtrip_and_conflict_preserves_existing_file() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "glint-output-test-{}-{unique}.png",
            std::process::id()
        ));
        let mut edits = Edits::default();
        edits.push(Redaction {
            bounds: Rect {
                x: 1,
                y: 0,
                width: 1,
                height: 2,
            },
            color: [32, 43, 66, 0],
        });
        let pixels = glint_core::composite(
            &[255; 16],
            2,
            2,
            Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            &edits,
        )
        .unwrap();
        save_png_new(&path, &pixels, 2, 2).unwrap();
        let encoded = std::fs::read(&path).unwrap();
        let decoded = image::load_from_memory(&encoded).unwrap().to_rgba8();
        assert_eq!(decoded.as_raw(), &pixels);
        assert_eq!(decoded.get_pixel(1, 1).0, [32, 43, 66, 255]);
        assert!(save_png_new(&path, &[0; 16], 2, 2).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), encoded);
        std::fs::remove_file(path).unwrap();
    }
}
