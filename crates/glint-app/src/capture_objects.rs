//! Object gestures retain an immutable original and commit one history command on release.
use super::*;

pub(super) struct Drag {
    index: usize,
    start: (u32, u32),
    original: Arc<Annotation>,
    handle: Option<u8>,
}
pub(super) fn clear(s: &mut Session) -> Rect {
    let dirty = s
        .object_preview
        .as_ref()
        .map(|e| {
            e.marks()
                .iter()
                .map(|m| m.bounds())
                .fold(Rect::default(), Rect::union)
        })
        .unwrap_or_default();
    s.object_selected = None;
    s.object_drag = None;
    s.object_preview = None;
    dirty
}
pub(super) fn damage(s: &Session, damage: Rect) -> Rect {
    if s.preview_scene
        || s.edits
            .marks()
            .iter()
            .any(|m| matches!(m.as_ref(), Annotation::Pixelation(_)))
    {
        s.edits
            .marks()
            .iter()
            .map(|m| m.bounds())
            .fold(damage, Rect::union)
    } else {
        damage
    }
}
pub(super) fn pointer(
    ui: &CaptureProbe,
    s: &mut Session,
    kind: i32,
    p: (u32, u32),
    dimensions: (u32, u32),
) -> bool {
    if kind == 0 {
        let selected = s.object_selected;
        let dirty = clear(s);
        if dirty.width > 0 {
            let dirty = damage(s, dirty);
            refresh(s, dirty, dimensions);
        }
        if ui.get_tool() != 0 {
            return false;
        }
        let local = Point {
            x: p.0 as f32 - s.selection.x as f32,
            y: p.1 as f32 - s.selection.y as f32,
        };
        let tolerance = selection_metrics(ui, dimensions).0;
        let grip = selected.and_then(|i| {
            s.edits
                .marks()
                .get(i)
                .and_then(|m| {
                    m.resize_handle_hit(
                        local,
                        (tolerance.0 as f32 * 0.6, tolerance.1 as f32 * 0.6),
                        (s.selection.width, s.selection.height),
                    )
                })
                .map(|h| (i, h))
        });
        let hit = if let Some((index, _)) = grip {
            s.edits.marks().get(index).map(|m| (index, m))
        } else {
            if selection_hit(s.selection, p, tolerance) != SelectionHit::Inside {
                return false;
            }
            s.edits
                .marks()
                .iter()
                .enumerate()
                .rev()
                .find(|(_, m)| m.hit_test(local, tolerance.0.max(tolerance.1) as f32 / 2.0))
        };
        let Some((index, mark)) = hit else {
            return false;
        };
        s.object_selected = Some(index);
        s.object_drag = Some(Drag {
            index,
            start: p,
            original: mark.clone(),
            handle: grip.map(|(_, h)| h),
        });
        s.start = None;
        s.selection_drag = None;
    }
    let Some(d) = s.object_drag.as_ref() else {
        return false;
    };
    let index = d.index;
    let dx = i64::from(p.0) - i64::from(d.start.0);
    let dy = i64::from(p.1) - i64::from(d.start.1);
    let limits = (s.selection.width, s.selection.height);
    let mark = if let Some(handle) = d.handle {
        d.original.resized(handle, dx, dy, limits)
    } else {
        d.original.translated(dx, dy, limits)
    };
    // Coincident arrow ends are invalid. Keep the last valid preview and still
    // complete pointer-up, so an attempted collapse cannot leave a stuck gesture.
    let mark = mark.unwrap_or_else(|| {
        s.object_preview.as_ref().unwrap_or(&s.edits).marks()[index]
            .as_ref()
            .clone()
    });
    let previous = s.object_preview.as_ref().unwrap_or(&s.edits).marks()[index].bounds();
    let mut dirty = previous.union(mark.bounds());
    let mut rejected = false;
    if kind == 2 {
        // A rejected command restores the original object, including pixels that
        // the live preview already cleared outside its latest bounding box.
        dirty = dirty.union(d.original.bounds());
        rejected = s.edits.replace(index, mark).is_none();
        s.object_preview = None;
        s.object_drag = None;
    } else {
        s.object_preview = s.edits.preview_replace(index, mark);
    }
    dirty = damage(s, dirty);
    refresh(s, dirty, dimensions);
    sync(ui, s);
    if rejected {
        ui.set_status(text(ui, Message::CaptureMarkRejected).into());
    }
    true
}
pub(super) fn hover_cursor(s: &Session, p: Point, tolerance: (u32, u32)) -> Option<i32> {
    let index = s.object_selected?;
    let id = s.edits.marks().get(index)?.resize_handle_hit(
        Point {
            x: p.x - s.selection.x as f32,
            y: p.y - s.selection.y as f32,
        },
        (tolerance.0 as f32 * 0.6, tolerance.1 as f32 * 0.6),
        (s.selection.width, s.selection.height),
    )?;
    Some(match id {
        0 | 4 => 4,
        2 | 6 => 5,
        1 | 5 => 3,
        3 | 7 => 2,
        _ => 0,
    })
}
pub(super) fn sync_grips(ui: &CaptureProbe, s: &Session) {
    let grips = s
        .object_selected
        .and_then(|i| s.object_preview.as_ref().unwrap_or(&s.edits).marks().get(i))
        .map(|m| m.resize_handles())
        .unwrap_or_default()
        .into_iter()
        .filter(|g| {
            g.point.x >= 0.
                && g.point.y >= 0.
                && g.point.x <= s.selection.width as f32
                && g.point.y <= s.selection.height as f32
        })
        .map(|g| ObjectGrip {
            x: g.point.x,
            y: g.point.y,
        })
        .collect::<Vec<_>>();
    ui.set_object_grips(ModelRc::from(Rc::new(slint::VecModel::from(grips))));
}
pub(super) fn delete(ui: &CaptureProbe, s: &mut Session, dimensions: (u32, u32)) {
    let Some(index) = s.object_selected else {
        return;
    };
    let Some(dirty) = s.edits.remove(index) else {
        ui.set_status(text(ui, Message::CaptureMarkRejected).into());
        return;
    };
    let dirty = dirty.union(clear(s));
    let dirty = damage(s, dirty);
    refresh(s, dirty, dimensions);
    sync(ui, s);
}
pub(super) fn sync_style(ui: &CaptureProbe, s: &Session) {
    let mark = s.object_selected.and_then(|i| s.edits.marks().get(i));
    let (tool, color, width, filled) = match mark.map(|m| m.as_ref()) {
        Some(Annotation::Text(t)) => (8, t.color, 3., true),
        Some(Annotation::Redaction(r)) => (1, r.color, 3., true),
        Some(Annotation::Pixelation(_)) => (7, [0; 4], 3., true),
        Some(Annotation::Stroke(t)) => (2, t.color, t.width, true),
        Some(Annotation::Shape(t)) => (
            match t.kind {
                ShapeKind::Rectangle => 3,
                ShapeKind::Arrow { .. } => 4,
                ShapeKind::Highlight => 5,
            },
            t.color,
            t.width,
            !matches!(t.kind, ShapeKind::Arrow { filled: false }),
        ),
        _ => (0, [0; 4], 3., true),
    };
    ui.set_object_tool(tool);
    ui.set_object_color(slint::Color::from_argb_u8(
        color[3], color[0], color[1], color[2],
    ));
    ui.set_object_stroke_width(width);
    ui.set_object_filled(filled);
}
pub(super) fn restyle(
    ui: &CaptureProbe,
    s: &mut Session,
    color: slint::Color,
    width: f32,
    filled: bool,
    dimensions: (u32, u32),
) {
    if ui.get_busy() || ui.get_text_editing() || s.object_drag.is_some() {
        return;
    }
    let Some(index) = s.object_selected else {
        return;
    };
    let Some(original) = s.edits.marks().get(index) else {
        return;
    };
    let Some(mark) = original.restyled(
        [color.red(), color.green(), color.blue(), color.alpha()],
        width,
        filled,
    ) else {
        return;
    };
    let Some(dirty) = s.edits.replace(index, mark) else {
        ui.set_status(text(ui, Message::CaptureMarkRejected).into());
        return;
    };
    let dirty = damage(s, dirty);
    refresh(s, dirty, dimensions);
    sync(ui, s);
}
