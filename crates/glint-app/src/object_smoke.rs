//! Generated-pixel object gestures and Slint key dispatch, not physical desktop input.
use super::*;
use slint::Model;
// Start the real directory worker, but let the lifecycle fixture hide its owner
// before the event loop can deliver completion. No OS chooser is injected.
pub(super) fn start_save_preparation(
    ui: &CaptureProbe,
    directory: &std::path::Path,
) -> Result<(), String> {
    let pixels = ui.get_desktop().to_rgba8().ok_or("No frozen source")?;
    let frame = Arc::new(glint_platform::DesktopFrame {
        x: 0,
        y: 0,
        width: pixels.width(),
        height: pixels.height(),
        rgba: pixels.as_bytes().to_vec(),
    });
    let slot = OutputSlot::acquire().ok_or("Save preparation fixture slot occupied")?;
    export::start(
        ui,
        export::Request {
            pin_turns: 0,
            pin_payload: Default::default(),
            frame,
            crop: Rect {
                x: 200,
                y: 200,
                width: 400,
                height: 300,
            },
            edits: Edits::default(),
            options: glint_core::config::Output {
                directory: directory.to_string_lossy().into_owned(),
                auto_save: false,
                ..Default::default()
            },
            save: true,
            pin: false,
            copy: false,
            path: None,
            replacement: None,
            receipt: Default::default(),
            recover_automatic: false,
        },
        slot,
    );
    Ok(())
}
pub(super) fn verify(directory: &std::path::Path) -> Result<(), String> {
    let rgba: Vec<u8> = (0..768u32)
        .flat_map(|y| {
            (0..1024u32).flat_map(move |x| [(x * 7) as u8, (y * 5) as u8, (x ^ y) as u8, 255])
        })
        .collect();
    let frame = Arc::new(glint_platform::DesktopFrame {
        x: 0,
        y: 0,
        width: 1024,
        height: 768,
        rgba,
    });
    verify_exit_origin(&frame, directory)?;
    let ui = CaptureProbe::new().map_err(|e| e.to_string())?;
    // Pixel-comparison fixture isolates annotation rendering from the optional loupe.
    ui.set_magnifier_enabled(false);
    ui.set_diagnostic(false);
    let state = prepare_capture(&ui, frame.clone(), 128, None, || {});
    ui.invoke_pointer(0, 60., 130.);
    ui.invoke_pointer(2, 960., 630.);
    ui.set_tool(1);
    ui.invoke_pointer(0, 120., 200.);
    ui.invoke_pointer(2, 260., 260.);
    ui.set_tool(7);
    ui.invoke_pointer(0, 300., 190.);
    ui.invoke_pointer(2, 480., 290.);
    let original = state.borrow().edits.marks().to_vec();
    verify_pointer_cancel(&ui, &state, &frame, directory)?;
    verify_picker_click(&ui, directory)?;
    verify_draft_transactions(&ui, &state, &frame, directory)?;
    ui.set_tool(0);
    ui.invoke_pointer(0, 180., 230.);
    ui.invoke_pointer(1, 340., 230.);
    if !ui.get_object_selected() || state.borrow().edits.marks() != original {
        return Err("Object live drag changed committed history".into());
    }
    let expected = state
        .borrow()
        .edits
        .preview_replace(
            0,
            original[0]
                .translated(160, 0, (900, 500))
                .ok_or("Invalid object move")?,
        )
        .ok_or("Invalid preview")?;
    compare(&ui, &frame, &expected, "live-move", directory)?;
    ui.invoke_pointer(2, 340., 230.);
    if state.borrow().edits.marks() != expected.marks()
        || state.borrow().selection
            != (Rect {
                x: 60,
                y: 130,
                width: 900,
                height: 500,
            })
    {
        return Err("Object release changed crop or lost move".into());
    }
    ui.invoke_action("undo".into());
    if state.borrow().edits.marks() != original {
        return Err("Object drag was not exactly one undo".into());
    }
    ui.invoke_action("redo".into());
    compare(&ui, &frame, &state.borrow().edits, "redo-move", directory)?;
    // Mosaic is above the redaction in this overlap. Deleting and undoing keeps z-order.
    ui.invoke_pointer(0, 350., 230.);
    ui.invoke_pointer(2, 350., 230.);
    if state.borrow().object_selected != Some(1) {
        return Err("Overlap did not select topmost object".into());
    }
    ui.invoke_focus_canvas();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed {
            text: slint::platform::Key::Delete.into(),
        });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased {
            text: slint::platform::Key::Delete.into(),
        });
    if ui.get_object_count() != 1 {
        return Err("Slint Delete key did not remove selected object".into());
    }
    compare(
        &ui,
        &frame,
        &state.borrow().edits,
        "delete-filter",
        directory,
    )?;
    ui.invoke_action("undo".into());
    if state.borrow().edits.marks() != expected.marks() {
        return Err("Deletion undo changed z-order".into());
    }
    compare(&ui, &frame, &state.borrow().edits, "undo-delete", directory)?;
    // Redaction is now partly under a later mosaic; moving it invalidates dependent cells.
    ui.invoke_pointer(0, 285., 230.);
    ui.invoke_pointer(1, 250., 240.);
    let preview = state
        .borrow()
        .object_preview
        .clone()
        .ok_or("No second move preview")?;
    compare(&ui, &frame, &preview, "dependent-filter", directory)?;
    ui.invoke_action("undo".into()); // abort preview then undo the committed first move
    if state.borrow().edits.marks() != original {
        return Err("Undo during drag did not abort preview".into());
    }
    compare(&ui, &frame, &state.borrow().edits, "abort-drag", directory)?;
    ui.invoke_action("redo".into());
    // No-op click must preserve redo, even when an object is clipped at the crop edge.
    ui.invoke_action("undo".into());
    ui.invoke_pointer(0, 180., 230.);
    ui.invoke_pointer(2, 180., 230.);
    if !ui.get_can_redo() {
        return Err("No-op object click discarded redo".into());
    }
    // Exercise the actual selected-object swatch, including its enabled state.
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(
        ui.get_toolbar_x() + 12. + 27. * 2. + 11.,
        ui.get_toolbar_y() + ui.get_toolbar_height() + 8. + 45.,
    );
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    let Annotation::Redaction(r) = state.borrow().edits.marks()[0].as_ref().clone() else {
        return Err("Redaction vanished during style edit".into());
    };
    if r.color != [19, 168, 138, 255] {
        return Err("Selected-object swatch did not change real object color".into());
    }
    compare(
        &ui,
        &frame,
        &state.borrow().edits,
        "restyled-redaction",
        directory,
    )?;
    ui.invoke_action("undo".into());
    if state.borrow().edits.marks() != original {
        return Err("Style undo changed other objects".into());
    }
    ui.invoke_action("redo".into());
    // Arrow width and head are object properties, independent of new-tool defaults.
    ui.set_tool(4);
    ui.invoke_pointer(0, 610., 280.);
    ui.invoke_pointer(2, 750., 350.);
    ui.set_tool(0);
    ui.invoke_pointer(0, 670., 310.);
    ui.invoke_pointer(2, 670., 310.);
    // Resize an earlier redaction under a later filter: preview is ephemeral,
    // release is one command, and undo restores the entire scene exactly.
    ui.invoke_pointer(0, 180., 230.);
    ui.invoke_pointer(2, 180., 230.);
    let before = state.borrow().edits.marks().to_vec();
    ui.invoke_hover(260., 260.);
    if ui.get_cursor() != 4 {
        return Err("Object corner hover did not show resize cursor".into());
    }
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: slint::LogicalPosition::new(260., 260.),
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(300., 280.),
    });
    if state.borrow().edits.marks() != before {
        return Err("Live resize changed committed history".into());
    }
    let preview = state
        .borrow()
        .object_preview
        .clone()
        .ok_or("Resize preview missing")?;
    compare(&ui, &frame, &preview, "live-resize-redaction", directory)?;
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: slint::LogicalPosition::new(300., 280.),
        button: PointerEventButton::Left,
    });
    let changed = state.borrow().edits.marks().to_vec();
    if changed[0].as_ref()
        != &before[0]
            .resized(4, 40, 20, (900, 500))
            .ok_or("Invalid expected resize")?
    {
        return Err("Resize did not change the intended object".into());
    }
    ui.invoke_action("undo".into());
    if state.borrow().edits.marks() != before {
        return Err("Resize was not one undo step".into());
    }
    compare(
        &ui,
        &frame,
        &state.borrow().edits,
        "undo-resize-redaction",
        directory,
    )?;
    ui.invoke_action("redo".into());
    if state.borrow().edits.marks() != changed {
        return Err("Resize redo differs".into());
    }
    ui.invoke_pointer(0, 670., 310.);
    ui.invoke_pointer(2, 670., 310.);
    if ui.get_object_grips().row_count() != 2 {
        return Err("Arrow does not expose two endpoints".into());
    }
    let arrow_before = state.borrow().edits.marks()[2].clone();
    ui.invoke_pointer(0, 610., 280.);
    ui.invoke_pointer(1, 600., 300.);
    ui.invoke_pointer(2, 600., 300.);
    if state.borrow().edits.marks()[2].as_ref()
        != &arrow_before
            .resized(8, -10, 20, (900, 500))
            .ok_or("Invalid expected endpoint")?
    {
        return Err("Endpoint adapter changed wrong geometry".into());
    }
    compare(
        &ui,
        &frame,
        &state.borrow().edits,
        "resized-arrow-endpoint",
        directory,
    )?;
    let changed = state.borrow().edits.marks().to_vec();
    ui.invoke_pointer(0, 600., 300.);
    ui.invoke_pointer(1, 750., 350.);
    ui.invoke_pointer(2, 750., 350.);
    if state.borrow().object_drag.is_some() || state.borrow().edits.marks() != changed {
        return Err("Collapsing arrow endpoints stuck or damaged the gesture".into());
    }
    ui.invoke_action("undo".into());
    ui.invoke_action("redo".into());
    if state.borrow().edits.marks() != changed {
        return Err("Invalid endpoint collapse changed history".into());
    }
    ui.set_tool(2);
    ui.invoke_pointer(0, 600., 450.);
    ui.invoke_pointer(1, 650., 420.);
    ui.invoke_pointer(2, 750., 460.);
    ui.set_tool(0);
    ui.invoke_pointer(0, 650., 420.);
    ui.invoke_pointer(2, 650., 420.);
    if ui.get_object_grips().row_count() != 8 {
        return Err("Pen path lacks resize handles".into());
    }
    let before = state.borrow().edits.marks().to_vec();
    ui.invoke_pointer(0, 750., 460.);
    ui.invoke_pointer(1, 790., 480.);
    ui.invoke_pointer(2, 790., 480.);
    if state.borrow().edits.marks()[3].as_ref()
        != &before[3]
            .resized(4, 40, 20, (900, 500))
            .ok_or("Invalid expected pen resize")?
    {
        return Err("Pen resize adapter did not use original geometry".into());
    }
    compare(&ui, &frame, &state.borrow().edits, "resized-pen", directory)?;
    ui.invoke_action("undo".into());
    if state.borrow().edits.marks() != before {
        return Err("Pen scaling undo lost geometry".into());
    }
    ui.invoke_action("redo".into());
    ui.invoke_pointer(0, 350., 220.);
    ui.invoke_pointer(2, 350., 220.);
    let filter_before = state.borrow().edits.marks()[1].clone();
    ui.invoke_pointer(0, 480., 240.);
    ui.invoke_pointer(1, 520., 240.);
    ui.invoke_pointer(2, 520., 240.);
    if state.borrow().edits.marks()[1].as_ref()
        != &filter_before
            .resized(3, 40, 0, (900, 500))
            .ok_or("Invalid expected filter resize")?
    {
        return Err("Filter resize changed wrong area".into());
    }
    compare(
        &ui,
        &frame,
        &state.borrow().edits,
        "resized-pixelation",
        directory,
    )?;
    let crop = state.borrow().selection;
    ui.set_tool(1);
    ui.invoke_pointer(0, 60., 130.);
    ui.invoke_pointer(2, 960., 630.);
    ui.set_tool(0);
    ui.invoke_pointer(0, 450., 360.);
    ui.invoke_pointer(2, 450., 360.);
    ui.invoke_pointer(0, 60., 130.);
    ui.invoke_pointer(1, 80., 160.);
    ui.invoke_pointer(2, 80., 160.);
    if state.borrow().selection != crop
        || state
            .borrow()
            .edits
            .marks()
            .last()
            .ok_or("Missing full-crop object")?
            .bounds()
            .x
            != 20
    {
        return Err("Object handle at crop edge changed crop instead".into());
    }
    compare(
        &ui,
        &frame,
        &state.borrow().edits,
        "resized-crop-edge-object",
        directory,
    )?;
    ui.invoke_action("delete-object".into());
    ui.invoke_pointer(0, 675., 325.);
    ui.invoke_pointer(2, 675., 325.);
    ui.invoke_object_style(slint::Color::from_rgb_u8(75, 92, 250), 8., false);
    if ui.get_object_tool() != 4 || ui.get_object_filled() || ui.get_object_stroke_width() != 8. {
        return Err("Arrow object properties did not update".into());
    }
    compare(
        &ui,
        &frame,
        &state.borrow().edits,
        "restyled-arrow",
        directory,
    )?;
    let changed = state.borrow().edits.marks().to_vec();
    ui.invoke_action("undo".into());
    ui.invoke_action("redo".into());
    if state.borrow().edits.marks() != changed {
        return Err("Arrow property undo/redo was not exact".into());
    }
    ui.invoke_pointer(0, 675., 325.);
    ui.invoke_pointer(2, 675., 325.);
    if !ui.get_object_selected()
        || ui.get_object_tool() != 4
        || ui.get_object_grips().row_count() != 2
    {
        return Err("Arrow handles missing from narrow layout fixture".into());
    }
    for size in [(1024., 768.), (320., 240.)] {
        ui.window()
            .set_size(slint::LogicalSize::new(size.0, size.1));
        let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
        if ui.get_toolbar_y() + ui.get_toolbar_total_height() > size.1 {
            return Err("Selected-object properties escape screen".into());
        }
        save_snapshot(
            directory.join(format!("synthetic-object-selected-{}.png", size.0)),
            &pixels,
        )?;
    }
    verify_rejected_drag_restore()?;
    verify_toolbar(directory, &ui, &state)?;
    Ok(())
}
fn verify_exit_origin(
    frame: &Arc<glint_platform::DesktopFrame>,
    directory: &std::path::Path,
) -> Result<(), String> {
    use slint::platform::{Key, WindowEvent};
    let mut observations = String::new();
    for (action, expected) in [
        ("cancel", CaptureExit::ApiCancel),
        ("cancel-escape", CaptureExit::EscapeKey),
        ("cancel-pointer", CaptureExit::SecondaryClick),
        ("cancel-toolbar", CaptureExit::ToolbarCancel),
        ("cancel-window", CaptureExit::WindowRequest),
    ] {
        let ui = CaptureProbe::new().map_err(|e| e.to_string())?;
        let count = Rc::new(std::cell::Cell::new(0));
        let completed = count.clone();
        prepare_capture(&ui, frame.clone(), 128, None, move || {
            completed.set(completed.get() + 1)
        });
        // This mirrors the previously ambiguous closed=true/busy=true case.
        // Recording the cause must not suppress cancellation during output.
        ui.set_busy(true);
        ui.invoke_action(action.into());
        if !ui.get_closed()
            || !ui.get_busy()
            || ui.get_exit_reason() != expected
            || count.get() != 1
        {
            return Err(format!("Busy capture cancel lost exit source: {action}"));
        }
        ui.invoke_action("cancel-window".into());
        ui.invoke_finished();
        if ui.get_exit_reason() != expected || count.get() != 1 {
            return Err("Repeated close overwrote first exit source".into());
        }
        observations.push_str(&format!("{action}={expected:?};busy=true;callbacks=1\n"));
    }
    for (kind, expected) in [
        (0, CaptureExit::Direct),
        (1, CaptureExit::Copy),
        (2, CaptureExit::Save),
        (3, CaptureExit::Pin),
        (4, CaptureExit::ColorCopy),
        (5, CaptureExit::PinUpdate),
    ] {
        let ui = CaptureProbe::new().map_err(|e| e.to_string())?;
        prepare_capture(&ui, frame.clone(), 128, None, || {});
        ui.set_completion_kind(kind);
        ui.invoke_finished();
        if ui.get_exit_reason() != expected {
            return Err(format!("Completion kind {kind} lost exit source"));
        }
        ui.invoke_action("cancel".into());
        if ui.get_exit_reason() != expected {
            return Err("Late cancel overwrote successful exit source".into());
        }
        observations.push_str(&format!("completion-{kind}={expected:?}\n"));
    }
    let ui = CaptureProbe::new().map_err(|e| e.to_string())?;
    prepare_capture(&ui, frame.clone(), 128, None, || {});
    ui.set_text_editing(true);
    ui.invoke_focus_canvas();
    for (expected_closed, expected_reason) in
        [(false, CaptureExit::Open), (true, CaptureExit::EscapeKey)]
    {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Escape.into(),
        });
        if ui.get_closed() != expected_closed
            || ui.get_exit_reason() != expected_reason
            || ui.get_text_editing()
        {
            return Err("Slint Escape exit source confused text cancel and editor exit".into());
        }
    }
    observations.push_str("Slint Escape: text cancel remains Open; second Escape exits as EscapeKey\nSynthetic callback/Slint event classification; not physical OS input or actual export for completion-kind cases.\n");
    std::fs::write(
        directory.join("CAPTURE-EXIT-ORIGINS-PASSED.txt"),
        observations,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn verify_pointer_cancel(
    ui: &CaptureProbe,
    state: &Rc<RefCell<Session>>,
    frame: &glint_platform::DesktopFrame,
    directory: &std::path::Path,
) -> Result<(), String> {
    use slint::platform::{PointerEventButton, WindowEvent};
    let original = state.borrow().edits.clone();
    let region = state.borrow().selection;
    // Keep a redo entry as well as committed objects: cancellation must not
    // create a command or silently discard the other history branch.
    ui.invoke_action("undo".into());
    let before = state.borrow().edits.clone();
    for (name, tool, start, end) in [
        ("move", 0, (180., 230.), (215., 250.)),
        ("resize", 0, (120., 200.), (105., 180.)),
        ("selection-move", 0, (510., 340.), (535., 360.)),
        ("selection-resize", 0, (60., 400.), (40., 420.)),
        ("selection-replace", 0, (20., 400.), (40., 420.)),
        ("redact", 1, (510., 340.), (610., 410.)),
        ("pen", 2, (510., 340.), (610., 410.)),
        ("rectangle", 3, (510., 340.), (610., 410.)),
        ("arrow", 4, (510., 340.), (610., 410.)),
        ("highlight", 5, (510., 340.), (610., 410.)),
        ("pixelation", 7, (510., 340.), (610., 410.)),
    ] {
        ui.set_tool(tool);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: slint::LogicalPosition::new(start.0, start.1),
            button: PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(end.0, end.1),
        });
        if state.borrow().object_preview.is_none()
            && state.borrow().draft.is_none()
            && state.borrow().selection_drag.is_none()
        {
            return Err(format!(
                "Pointer cancel {name} fixture did not start a live preview"
            ));
        }
        ui.window().dispatch_event(WindowEvent::PointerExited);
        {
            let s = state.borrow();
            if s.object_drag.is_some()
                || s.object_preview.is_some()
                || s.draft.is_some()
                || s.start.is_some()
                || s.selection_drag.is_some()
                || s.selection_before.is_some()
                || s.edits.marks() != before.marks()
                || s.edits.can_undo() != before.can_undo()
                || s.edits.can_redo() != before.can_redo()
                || s.selection != region
                || !ui.get_output_ready()
            {
                return Err(format!(
                    "Slint PointerExited did not cancel {name} without changing scene/history"
                ));
            }
        }
        compare(
            ui,
            frame,
            &before,
            &format!("pointer-cancel-{name}"),
            directory,
        )?;
        // A late release after cancellation cannot commit the abandoned gesture.
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(end.0, end.1),
            button: PointerEventButton::Left,
        });
        if state.borrow().edits.marks() != before.marks()
            || state.borrow().selection != region
            || !ui.get_can_redo()
        {
            return Err(format!("Late release committed cancelled {name}"));
        }
    }
    ui.invoke_action("redo".into());
    if state.borrow().edits.marks() != original.marks() {
        return Err("Pointer cancellation lost redo contents".into());
    }
    std::fs::write(directory.join("POINTER-CANCEL-PASSED.txt"),
        "Slint PointerPressed/Moved/Exited and late release: object move/resize, selection move/resize/replace, redaction, pen, rectangle, arrow, highlight, pixelation. Exact preview pixels restored; committed marks/crop/undo/redo unchanged. Synthetic dispatch, not physical capture-loss acceptance.\n")
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn verify_draft_transactions(
    ui: &CaptureProbe,
    state: &Rc<RefCell<Session>>,
    frame: &glint_platform::DesktopFrame,
    directory: &std::path::Path,
) -> Result<(), String> {
    let original = state.borrow().edits.clone();
    let region = state.borrow().selection;
    let _slot = OutputSlot::acquire().ok_or("Draft transaction output safety slot busy")?;
    for (name, tool) in [
        ("redact", 1),
        ("pen", 2),
        ("rect", 3),
        ("arrow", 4),
        ("highlight", 5),
        ("mosaic", 7),
    ] {
        for action in [
            "undo",
            "redo",
            "deselect-object",
            "finish",
            "copy",
            "save",
            "pin",
            "save-without-auto",
        ] {
            state.borrow_mut().edits = original.clone();
            if action == "redo" {
                ui.invoke_action("undo".into());
            }
            ui.set_tool(tool);
            ui.invoke_pointer(0, 510., 340.);
            ui.invoke_pointer(1, 610., 410.);
            if state.borrow().draft.is_none() {
                return Err(format!("No draft fixture: {name}/{action}"));
            }
            ui.set_status("".into());
            ui.set_auto_save_failed(true);
            ui.invoke_action(action.into());
            if matches!(action, "undo" | "redo" | "deselect-object") {
                if state.borrow().draft.is_some() || state.borrow().start.is_some() {
                    return Err(format!("Uncommitted draft survived {name}/{action}"));
                }
                let expected = if action == "undo" {
                    let mut e = original.clone();
                    e.undo();
                    e
                } else {
                    original.clone()
                };
                ui.invoke_pointer(2, 610., 410.);
                if state.borrow().edits.marks() != expected.marks() {
                    return Err(format!("Late release committed {name}/{action}"));
                }
                compare(
                    ui,
                    frame,
                    &expected,
                    &format!("draft-{name}-{action}"),
                    directory,
                )?;
            } else {
                let expected = text(ui, Message::CaptureGesturePending);
                if ui.get_status() != expected
                    || ui.get_busy()
                    || state.borrow().draft.is_none()
                    || state.borrow().edits.marks() != original.marks()
                {
                    return Err(format!(
                        "Output did not preserve/block live draft: {name}/{action}: {}",
                        ui.get_status()
                    ));
                }
                ui.invoke_pointer(2, 610., 410.);
                if state.borrow().edits.marks().len() != original.marks().len() + 1 {
                    return Err(format!(
                        "Blocked {name}/{action} did not commit once on release"
                    ));
                }
                ui.invoke_action("undo".into());
                if state.borrow().edits.marks() != original.marks() {
                    return Err(format!("Blocked {name}/{action} changed undo"));
                }
            }
            if state.borrow().selection != region {
                return Err("Draft action changed selection".into());
            }
        }
    }
    state.borrow_mut().edits = original;
    ui.set_auto_save_failed(false);
    refresh(
        &mut state.borrow_mut(),
        Rect {
            x: 0,
            y: 0,
            width: 900,
            height: 500,
        },
        (1024, 768),
    );
    sync(ui, &state.borrow());
    std::fs::write(directory.join("DRAFT-TRANSACTIONS-PASSED.txt"), "Six drawing tools: undo/redo/tool-change abort drafts and late release; five output actions preserve live drafts and block before shared output slot; normal release commits exactly once. Synthetic callbacks and preview pixel comparisons.\n").map_err(|e|e.to_string())?;
    Ok(())
}
fn verify_picker_click(ui: &CaptureProbe, directory: &std::path::Path) -> Result<(), String> {
    use slint::platform::{PointerEventButton, WindowEvent};
    // A real shared output slot prevents any clipboard write even on the old
    // faulty path. Its visible Pending response detects attempted publication.
    let _slot = OutputSlot::acquire().ok_or("Picker click output guard busy")?;
    // A picker default must finish region creation before clicks can confirm a
    // color. Intermediate nonempty geometry must not switch the active gesture.
    let initial = CaptureProbe::new().map_err(|e| e.to_string())?;
    let initial_state = prepare_capture(
        &initial,
        Arc::new(glint_platform::DesktopFrame {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
            rgba: [17, 34, 51, 255].repeat(640 * 480),
        }),
        128,
        None,
        || {},
    );
    initial.set_tool(6);
    for (kind, x, y) in [
        (0, 100., 100.),
        (1, 400., 300.),
        (1, 460., 340.),
        (2, 460., 340.),
    ] {
        initial.invoke_pointer(kind, x, y);
    }
    {
        let s = initial_state.borrow();
        if s.selection
            != (Rect {
                x: 100,
                y: 100,
                width: 360,
                height: 240,
            })
            || s.start.is_some()
            || s.selection_drag.is_some()
            || s.picker_pressed
            || !initial.get_picker_feedback().is_empty()
        {
            return Err(
                "Picker default interrupted region creation or copied before selection finished"
                    .into(),
            );
        }
    }
    let original_tool = ui.get_tool();
    ui.set_tool(6);
    for only in [false, true] {
        ui.set_color_only(only);
        for (name, start, cancel, end) in [
            ("release-only", None, false, (500., 400.)),
            ("cancelled", Some((500., 400.)), true, (500., 400.)),
            ("invalid-press", Some((30., 400.)), false, (500., 400.)),
            ("outside-release", Some((500., 400.)), false, (30., 400.)),
        ] {
            ui.set_picker_feedback("".into());
            if let Some((x, y)) = start {
                ui.window().dispatch_event(WindowEvent::PointerPressed {
                    position: slint::LogicalPosition::new(x, y),
                    button: PointerEventButton::Left,
                });
            }
            if cancel {
                ui.window().dispatch_event(WindowEvent::PointerExited);
            }
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position: slint::LogicalPosition::new(end.0, end.1),
                button: PointerEventButton::Left,
            });
            if ui.get_picker_busy() || ui.get_closed() || !ui.get_picker_feedback().is_empty() {
                return Err(format!("Picker {name} attempted copy (standalone={only})"));
            }
        }
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: slint::LogicalPosition::new(500., 400.),
            button: PointerEventButton::Left,
        });
        if !ui.get_picker_feedback().is_empty() {
            return Err("Picker press copied before release".into());
        }
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(510., 410.),
            button: PointerEventButton::Left,
        });
        if ui.get_picker_feedback() != text(ui, Message::CaptureOutputPending) {
            return Err("Valid picker gesture did not request copy".into());
        }
        ui.set_picker_feedback("".into());
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(510., 410.),
            button: PointerEventButton::Left,
        });
        if !ui.get_picker_feedback().is_empty() {
            return Err("Duplicate picker release attempted copy".into());
        }
    }
    ui.set_color_only(false);
    ui.set_tool(original_tool);
    std::fs::write(directory.join("PICKER-CLICK-PASSED.txt"),"Slint pointer routing in overlay/standalone mode: release-only, cancel/late release, invalid start, outside end, press-only and duplicate release cannot request copy; valid gesture requests once. Shared output slot guards real clipboard. Positive actual clipboard flow tested separately.\n").map_err(|e|e.to_string())?;
    Ok(())
}
fn verify_toolbar(
    directory: &std::path::Path,
    ui: &CaptureProbe,
    state: &Rc<RefCell<Session>>,
) -> Result<(), String> {
    use glint_core::config::{Tool, Toolbar, ToolbarMode, ToolbarPosition};
    let saved = Rc::new(RefCell::new(Toolbar::default()));
    let fail = Rc::new(std::cell::Cell::new(false));
    let target = saved.clone();
    let reject = fail.clone();
    toolbar::configure(ui, Toolbar::default(), move |next| {
        if reject.get() {
            return Err("Synthetic write failure: retry or cancel".into());
        }
        *target.borrow_mut() = next.clone();
        Ok(())
    });
    let marks = state.borrow().edits.marks().to_vec();
    let region = state.borrow().selection;
    let cmd = |name: &str, row, value| ui.invoke_toolbar_command(name.into(), row, value);
    ui.window().set_size(slint::LogicalSize::new(1024., 768.));
    ui.window().take_snapshot().map_err(|e| e.to_string())?;
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(ui.get_toolbar_x() + 20., ui.get_toolbar_y() + 20.);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Right,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Right,
    });
    if ui.get_toolbar_modal() != 1 {
        return Err("Toolbar right-click did not open customization".into());
    }
    ui.window().take_snapshot().map_err(|e| e.to_string())?;
    let position = slint::LogicalPosition::new(276., 266.);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    let position = slint::LogicalPosition::new(276., 342.);
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    if ui.get_toolbar_rows().row_data(2).unwrap().tool != 0 {
        return Err("Toolbar drag handle did not reorder via Slint pointer events".into());
    }
    cmd("move", 2, 0);
    cmd("mode", 0, 1);
    cmd("position", 0, 2);
    cmd("visible", 8, 0);
    cmd("visible", 0, 0); // Selection cannot be hidden, even via a malformed UI command.
    cmd("move", 8, 1);
    cmd("groups", 0, 0);
    if !ui.get_toolbar_icons()
        || ui.get_toolbar_hidden().row_count() != 1
        || ui.get_toolbar_hidden().row_data(0) != Some(8)
        || ui.get_toolbar_rows().row_data(1).unwrap().tool != 8
    {
        return Err("Toolbar draft did not preview mode/hiding/order".into());
    }
    ui.invoke_action("delete-object".into());
    ui.invoke_pointer(0, 10., 10.);
    ui.invoke_pointer(2, 100., 100.);
    fail.set(true);
    cmd("apply", 0, 0);
    if ui.get_toolbar_modal() != 1
        || ui.get_toolbar_feedback().is_empty()
        || *saved.borrow() != Toolbar::default()
    {
        return Err("Toolbar write failure lost draft or faked success".into());
    }
    for size in [(1024., 768.), (320., 240.)] {
        ui.window()
            .set_size(slint::LogicalSize::new(size.0, size.1));
        save_snapshot(
            directory.join(format!("synthetic-toolbar-editor-{}.png", size.0)),
            &ui.window().take_snapshot().map_err(|e| e.to_string())?,
        )?;
    }
    fail.set(false);
    cmd("apply", 0, 0);
    let config = saved.borrow().clone();
    if ui.get_toolbar_modal() != 0
        || config.mode != ToolbarMode::Icons
        || config.position != ToolbarPosition::Bottom
        || config.hidden != vec![Tool::Picker]
        || config.order[1] != Tool::Picker
        || config.groups
    {
        return Err("Toolbar apply did not commit only the draft".into());
    }
    for size in [(1024., 768.), (320., 240.)] {
        ui.window()
            .set_size(slint::LogicalSize::new(size.0, size.1));
        for position in [0, 1, 2] {
            cmd("open", 0, 0);
            cmd("position", 0, position);
            cmd("apply", 0, 0);
            if ui.get_toolbar_y() < 0.
                || ui.get_toolbar_y() + ui.get_toolbar_total_height() > size.1
            {
                return Err("Configured toolbar escapes screen".into());
            }
            save_snapshot(
                directory.join(format!("synthetic-toolbar-{}-{}.png", size.0, position)),
                &ui.window().take_snapshot().map_err(|e| e.to_string())?,
            )?;
        }
    }
    cmd("more", 0, 0);
    save_snapshot(
        directory.join("synthetic-toolbar-more-320.png"),
        &ui.window().take_snapshot().map_err(|e| e.to_string())?,
    )?;
    let position = slint::LogicalPosition::new(160., 73.);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    if ui.get_toolbar_modal() != 0 || ui.get_tool() != 6 {
        return Err("More did not select hidden picker through Slint button".into());
    }
    cmd("more", 0, 0);
    ui.invoke_focus_canvas();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed {
            text: slint::platform::Key::Escape.into(),
        });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased {
            text: slint::platform::Key::Escape.into(),
        });
    if ui.get_toolbar_modal() != 0 {
        return Err("Escape did not close More".into());
    }
    cmd("open", 0, 0);
    cmd("reset", 0, 0);
    cmd("cancel", 0, 0);
    if !ui.get_toolbar_icons() || ui.get_toolbar_hidden().row_count() != 1 {
        return Err("Cancelling toolbar reset changed the applied toolbar".into());
    }
    cmd("open", 0, 0);
    cmd("reset", 0, 0);
    cmd("apply", 0, 0);
    if *saved.borrow() != Toolbar::default()
        || ui.get_toolbar_icons()
        || state.borrow().edits.marks() != marks
        || state.borrow().selection != region
    {
        return Err("Toolbar operations changed selection/marks or failed to reset".into());
    }
    ui.set_english(true);
    ui.set_dark(true);
    style::apply_labels(ui);
    ui.invoke_apply_theme();
    cmd("open", 0, 0);
    ui.window().set_size(slint::LogicalSize::new(1024., 768.));
    if ui.get_toolbar_rows().row_data(0).unwrap().label != "Select" {
        return Err("Toolbar editor did not use the capture language".into());
    }
    save_snapshot(
        directory.join("synthetic-toolbar-editor-dark-en.png"),
        &ui.window().take_snapshot().map_err(|e| e.to_string())?,
    )?;
    cmd("cancel", 0, 0);
    Ok(())
}
fn verify_rejected_drag_restore() -> Result<(), String> {
    let frame = Arc::new(glint_platform::DesktopFrame {
        x: 0,
        y: 0,
        width: 384,
        height: 128,
        rgba: vec![255; 384 * 128 * 4],
    });
    let ui = CaptureProbe::new().map_err(|e| e.to_string())?;
    // Pixel-comparison fixture isolates annotation rendering from the optional loupe.
    ui.set_magnifier_enabled(false);
    let state = prepare_capture(&ui, frame, 128, None, || {});
    {
        let mut s = state.borrow_mut();
        s.selection = Rect {
            x: 0,
            y: 0,
            width: 384,
            height: 128,
        };
        s.edits.push(Stroke {
            points: vec![Point { x: 26., y: 90. }; 65536],
            width: 3.,
            color: [241, 12, 222, 255],
        });
        for _ in 0..14 {
            let mark = s.edits.marks()[0]
                .translated(1, 0, (384, 128))
                .ok_or("Invalid budget fixture")?;
            if s.edits.replace(0, mark).is_none() {
                return Err("Budget fixture filled too early".into());
            }
        }
        let dirty = s.edits.marks()[0].bounds();
        refresh(&mut s, dirty, (384, 128));
        sync(&ui, &s);
    }
    let before = state.borrow().edits.marks().to_vec();
    ui.invoke_pointer(0, 40., 90.);
    ui.invoke_pointer(1, 300., 90.);
    ui.invoke_pointer(2, 300., 90.);
    if state.borrow().edits.marks() != before || state.borrow().object_drag.is_some() {
        return Err("Rejected drag did not keep original object".into());
    }
    ui.set_selected(false); // Keep the source and annotation pixels visible, hide only toolbar/grips.
    let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    for (x, color) in [(40, [241, 12, 222, 255]), (300, [255; 4])] {
        let i = (90 * 384 + x) * 4;
        if pixels.as_bytes()[i..i + 4] != color {
            return Err("Budget rejection left a stale or missing preview pixel".into());
        }
    }
    Ok(())
}
fn save_snapshot(
    path: std::path::PathBuf,
    pixels: &SharedPixelBuffer<Rgba8Pixel>,
) -> Result<(), String> {
    image::save_buffer_with_format(
        path,
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())
}
fn compare(
    ui: &CaptureProbe,
    frame: &glint_platform::DesktopFrame,
    edits: &Edits,
    name: &str,
    directory: &std::path::Path,
) -> Result<(), String> {
    let expected = glint_core::composite_selection(
        &frame.rgba,
        frame.width,
        frame.height,
        Rect {
            x: 60,
            y: 130,
            width: 900,
            height: 500,
        },
        edits,
    )?;
    // Hide only the object's selection outline while comparing the rendered image.
    ui.set_tool(1);
    let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    ui.set_tool(0);
    for y in 10..490 {
        for x in 10..890 {
            let sy = (y + 130) as f32;
            if sy >= ui.get_toolbar_y() && sy < ui.get_toolbar_y() + ui.get_toolbar_total_height() {
                continue;
            }
            let a = ((y * 900 + x) * 4) as usize;
            let b = (((y + 130) * 1024 + x + 60) * 4) as usize;
            if expected[a..a + 4]
                .iter()
                .zip(&pixels.as_bytes()[b..b + 4])
                .any(|(&a, &b)| a.abs_diff(b) > 1)
            {
                return Err(format!("Object {name} preview/export mismatch at {x},{y}"));
            }
        }
    }
    save_snapshot(
        directory.join(format!("synthetic-object-{name}.png")),
        &pixels,
    )
}
