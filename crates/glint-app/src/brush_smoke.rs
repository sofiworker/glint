//! M0 measures callback-to-completed-redraw latency, not physical input or scanout latency.
use super::{CaptureProbe, Session};
use slint::{
    ComponentHandle, Model,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Samples {
    next: usize,
    pending: Option<Instant>,
    frames: Vec<f64>,
    callbacks: Vec<f64>,
    finished: bool,
}

fn point(index: usize, count: usize, tool: i32) -> (f32, f32) {
    let phase = index as f32 / count as f32;
    if tool == 7 {
        return (240.0 + phase * 3000.0, 300.0 + phase * 1400.0);
    }
    (
        240.0 + phase * 3000.0,
        950.0 + (phase * std::f32::consts::TAU * 3.0).sin() * 550.0,
    )
}

pub fn install(
    ui: &CaptureProbe,
    state: Rc<RefCell<Session>>,
    source: Arc<glint_platform::DesktopFrame>,
    path: String,
    count: usize,
    error: Rc<RefCell<Option<String>>>,
) {
    let tool = if ui.get_tool() == 7 { 7 } else { 2 };
    let samples = Rc::new(RefCell::new(Samples::default()));
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_, event| {
        if matches!(event, winit::event::WindowEvent::RedrawRequested) {
            // Slint's winit adapter calls draw() after this event filter. Our queued callback
            // runs on a later event-loop turn, after that draw/present invocation returns.
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_frame_completed());
        }
        EventResult::Propagate
    });
    let weak = ui.as_weak();
    let measured = samples.clone();
    let failure = error.clone();
    ui.on_frame_completed(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut measured = measured.borrow_mut();
        if measured.finished {
            return;
        }
        let Some(begin) = measured.pending.take() else {
            return;
        };
        measured.frames.push(begin.elapsed().as_secs_f64() * 1000.0);
        if measured.next < count {
            let (x, y) = point(measured.next + 1, count, tool);
            measured.next += 1;
            measured.pending = Some(Instant::now());
            let begin = Instant::now();
            ui.invoke_pointer(1, x, y);
            measured
                .callbacks
                .push(begin.elapsed().as_secs_f64() * 1000.0);
        } else {
            measured.finished = true;
            let (x, y) = point(count, count, tool);
            ui.invoke_pointer(2, x, y);
            let result = verify(&ui, &state, &source, &path, &measured);
            if let Err(message) = result {
                *failure.borrow_mut() = Some(message);
            }
            let _ = slint::quit_event_loop();
        }
    });
    let weak = ui.as_weak();
    let active = samples.clone();
    slint::Timer::single_shot(Duration::from_millis(500), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        ui.invoke_pointer(0, 100.0, 120.0);
        ui.invoke_pointer(2, 3500.0, 1870.0);
        ui.set_tool(tool);
        let (x, y) = point(0, count, tool);
        active.borrow_mut().pending = Some(Instant::now());
        ui.invoke_pointer(0, x, y);
    });
    slint::Timer::single_shot(Duration::from_secs(15 + count as u64 / 30), move || {
        if !samples.borrow().finished {
            *error.borrow_mut() =
                Some("Annotation redraw probe timed out; no completed redraw evidence".into());
            let _ = slint::quit_event_loop();
        }
    });
}

fn percentile(values: &[f64], fraction: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() as f64 * fraction).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1)]
}

fn verify(
    ui: &CaptureProbe,
    state: &Rc<RefCell<Session>>,
    source: &glint_platform::DesktopFrame,
    path: &str,
    samples: &Samples,
) -> Result<(), String> {
    let tool_name = if ui.get_tool() == 7 {
        "pixelation"
    } else {
        "brush"
    };
    let s = state.borrow();
    if s.preview_error.is_some() || s.edits.marks().len() != 1 {
        return Err(format!(
            "Annotation did not commit one valid object: {:?}",
            s.preview_error
        ));
    }
    let raster_ms = s.preview.raster_ms;
    let raster_bytes = s.preview.updated_bytes;
    let tile_updates = s.preview.update_count;
    let tile_rows = s.preview.model.row_count();
    let crop = s.selection;
    let begin = Instant::now();
    let expected =
        glint_core::composite_selection(&source.rgba, source.width, source.height, crop, &s.edits)?;
    let export_ms = begin.elapsed().as_secs_f64() * 1000.0;
    drop(s);
    // An explicit final full snapshot verifies pixels. It is excluded from interactive timing.
    let snapshot = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    if snapshot.width() != source.width || snapshot.height() != source.height {
        return Err("Annotation snapshot/source size mismatch".into());
    }
    let mut mismatch = 0;
    for y in 3..crop.height - 3 {
        for x in 3..crop.width - 3 {
            let output = ((y * crop.width + x) * 4) as usize;
            let preview = (((y + crop.y) * snapshot.width() + x + crop.x) * 4) as usize;
            for c in 0..4 {
                if (i16::from(expected[output + c]) - i16::from(snapshot.as_bytes()[preview + c]))
                    .abs()
                    > 1
                {
                    mismatch += 1;
                }
            }
        }
    }
    if mismatch > 0 {
        return Err(format!(
            "Annotation preview/export differed in {mismatch} channels"
        ));
    }
    ui.invoke_action("undo".into());
    {
        let s = state.borrow();
        if !s.edits.marks().is_empty() {
            return Err("Annotation undo left an object behind".into());
        }
        if s.preview
            .model
            .iter()
            .any(|tile| tile.picture.size().width > 0)
        {
            return Err("Annotation undo left a visible tile behind".into());
        }
    }
    ui.invoke_action("redo".into());
    {
        let s = state.borrow();
        if glint_core::composite_selection(
            &source.rgba,
            source.width,
            source.height,
            crop,
            &s.edits,
        )? != expected
        {
            return Err("Annotation redo changed exported pixels".into());
        }
    }
    image::save_buffer_with_format(
        path,
        snapshot.as_bytes(),
        snapshot.width(),
        snapshot.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())?;
    let p50 = percentile(&samples.frames, 0.5);
    let p95 = percentile(&samples.frames, 0.95);
    let max = percentile(&samples.frames, 1.0);
    let callback95 = percentile(&samples.callbacks, 0.95);
    let json = format!(
        "{{\n  \"source\": \"synthetic-3840x2160\",\n  \"frames\": {},\n  \"callback_to_redraw_p50_ms\": {p50},\n  \"callback_to_redraw_p95_ms\": {p95},\n  \"callback_to_redraw_max_ms\": {max},\n  \"callback_p95_ms\": {callback95},\n  \"tile_updates\": {tile_updates},\n  \"tile_raster_bytes\": {raster_bytes},\n  \"tile_rows\": {tile_rows},\n  \"tile_raster_total_ms\": {raster_ms},\n  \"composite_ms\": {export_ms},\n  \"preview_export_mismatch_channels\": {mismatch},\n  \"frame_ms\": {:?}\n}}\n",
        samples.frames.len(),
        samples.frames
    );
    std::fs::write(format!("{path}.json"), json).map_err(|e| e.to_string())?;
    eprintln!(
        "M0 4K {tool_name}: {} completed redraws, p50 {p50:.2} / p95 {p95:.2} / max {max:.2} ms; callback p95 {callback95:.2} ms; {tile_updates} tile updates, {tile_rows} rows, {raster_bytes} raster bytes, {raster_ms:.2} ms raster total; composite {export_ms:.2} ms; preview/export + undo/redo passed",
        samples.frames.len()
    );
    Ok(())
}
