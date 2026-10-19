//! Bounded Windows text rasterization. Callbacks keep the live draft on actual failures.
use super::{CaptureProbe, Message, Session, refresh, sync, text};
use glint_core::{GlyphMask, TextAnnotation};
use slint::ComponentHandle;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub type ResultSlot = Arc<Mutex<Option<(u64, Result<TextAnnotation, String>)>>>;
static TEXT_WORKER: AtomicBool = AtomicBool::new(false);
struct WorkerSlot;
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        TEXT_WORKER.store(false, Ordering::Release);
    }
}

pub fn cancel(ui: &CaptureProbe, state: &mut Session) {
    state.text_generation = state.text_generation.wrapping_add(1);
    state.text_anchor = None;
    state.text_target = None;
    ui.set_text_editing(false);
    ui.set_text_feedback("".into());
    ui.invoke_focus_canvas();
}

pub fn edit(ui: &CaptureProbe, state: &mut Session) {
    if ui.get_text_editing() || ui.get_text_busy() || state.object_drag.is_some() {
        return;
    }
    let Some(index) = state.object_selected else {
        return;
    };
    let Some(glint_core::Annotation::Text(mark)) =
        state.edits.marks().get(index).map(|m| m.as_ref())
    else {
        return;
    };
    state.text_generation = state.text_generation.wrapping_add(1);
    state.text_target = Some(index);
    state.text_anchor = Some(mark.origin);
    ui.set_text_anchor_x((state.selection.x + mark.origin.0) as f32);
    ui.set_text_anchor_y((state.selection.y + mark.origin.1) as f32);
    ui.set_text_value(mark.text.clone().into());
    ui.set_text_font(mark.font.clone().into());
    ui.set_text_size(i32::from(mark.size));
    ui.set_mark_color(slint::Color::from_argb_u8(
        mark.color[3],
        mark.color[0],
        mark.color[1],
        mark.color[2],
    ));
    ui.set_text_feedback("".into());
    ui.set_text_editing(true);
    ui.invoke_focus_text();
}

pub fn commit(ui: &CaptureProbe, state: &mut Session) {
    if !ui.get_text_editing() || ui.get_text_busy() {
        return;
    }
    let value = ui.get_text_value().trim().to_string();
    if value.is_empty() {
        cancel(ui, state);
        return;
    }
    if value.chars().count() > 80 || value.chars().any(char::is_control) {
        ui.set_text_feedback(text(ui, Message::CaptureTextLimit).into());
        return;
    }
    let Some(origin) = state.text_anchor else {
        return;
    };
    let color = ui.get_mark_color();
    let color = [color.red(), color.green(), color.blue(), color.alpha()];
    // A colour-only edit reuses its existing glyphs and needs no font work.
    if let Some(index) = state.text_target
        && let Some(glint_core::Annotation::Text(original)) =
            state.edits.marks().get(index).map(|m| m.as_ref())
        && original.text == value
        && original.font == ui.get_text_font().as_str()
        && i32::from(original.size) == ui.get_text_size()
    {
        let mut mark = original.clone();
        mark.color = color;
        finish(
            ui,
            state,
            mark,
            (ui.get_source_w() as u32, ui.get_source_h() as u32),
        );
        return;
    }
    if TEXT_WORKER.swap(true, Ordering::AcqRel) {
        ui.set_text_feedback(text(ui, Message::CaptureOutputPending).into());
        return;
    }
    let slot = WorkerSlot;
    let family = ui.get_text_font().to_string();
    let size = ui.get_text_size() as u16;
    let generation = state.text_generation;
    let pending = state.text_result.clone();
    let weak = ui.as_weak();
    ui.set_text_busy(true);
    ui.set_text_feedback(text(ui, Message::CaptureTextRendering).into());
    let worker = std::thread::Builder::new()
        .name("glint-text-raster".into())
        .spawn(move || {
            let result =
                glint_platform::rasterize_text(&value, &family, size).map(|mask| TextAnnotation {
                    origin,
                    text: value,
                    font: family,
                    size,
                    color,
                    mask: GlyphMask {
                        width: mask.width,
                        height: mask.height,
                        coverage: mask.coverage.into(),
                    },
                });
            *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some((generation, result));
            drop(slot);
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("text-ready".into()));
        });
    if let Err(error) = worker {
        ui.set_text_busy(false);
        ui.set_text_feedback(format!("{}: {error}", text(ui, Message::CaptureTextFailed)).into());
        ui.invoke_focus_text();
    }
}

pub fn ready(ui: &CaptureProbe, state: &mut Session, dimensions: (u32, u32)) {
    let result = state
        .text_result
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    ui.set_text_busy(false);
    let Some((generation, result)) = result else {
        return;
    };
    if generation != state.text_generation || !ui.get_text_editing() {
        return;
    }
    match result {
        Ok(mark) => {
            finish(ui, state, mark, dimensions);
        }
        Err(error) => {
            ui.set_text_feedback(
                format!("{}: {error}", text(ui, Message::CaptureTextFailed)).into(),
            );
            ui.invoke_focus_text();
        }
    }
}

fn finish(ui: &CaptureProbe, state: &mut Session, mark: TextAnnotation, dimensions: (u32, u32)) {
    let dirty = if let Some(index) = state.text_target {
        state.edits.replace(index, mark)
    } else {
        let dirty = mark.bounds();
        state.edits.push(mark).then_some(dirty)
    };
    let Some(dirty) = dirty else {
        ui.set_text_feedback(text(ui, Message::CaptureMarkRejected).into());
        return;
    };
    let dirty = super::objects::damage(state, dirty);
    refresh(state, dirty, dimensions);
    sync(ui, state);
    cancel(ui, state);
}
