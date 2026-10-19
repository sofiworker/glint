//! Generated-pixel adapter checks, distinct from real desktop capture in resident_smoke.
use glint_core::{Edits, Pixelation, Rect, Redaction};
use slint::ComponentHandle;

pub fn verify(directory: &std::path::Path) -> Result<(), String> {
    let rgba: Vec<u8> = (0..480u32)
        .flat_map(|y| {
            (0..640u32).flat_map(move |x| [(x * 7) as u8, (y * 5) as u8, (x ^ y) as u8, 255])
        })
        .collect();
    let frame = glint_platform::DesktopFrame {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
        rgba: rgba.clone(),
    };
    let ui = crate::probe::resident_capture(
        glint_platform::DesktopSnapshot {
            displays: vec![],
            frame,
            windows: vec![],
            pointer: None,
            detection_error: None,
        },
        false,
        crate::probe::Presentation {
            magnifier: false,
            ..Default::default()
        },
        None,
        || {},
    )?;
    ui.invoke_pointer(0, 40.0, 40.0);
    ui.invoke_pointer(2, 440.0, 340.0);
    ui.set_tool(1);
    ui.invoke_pointer(0, 80.0, 80.0);
    ui.invoke_pointer(2, 160.0, 160.0);
    ui.set_tool(7);
    ui.invoke_pointer(0, 56.0, 56.0);
    ui.invoke_pointer(1, 320.0, 240.0);
    ui.invoke_pointer(2, 320.0, 240.0);
    let mut edits = Edits::default();
    edits.push(Redaction {
        bounds: Rect {
            x: 40,
            y: 40,
            width: 80,
            height: 80,
        },
        color: [32, 43, 66, 255],
    });
    edits.push(Pixelation {
        bounds: Rect {
            x: 16,
            y: 16,
            width: 264,
            height: 184,
        },
    });
    for (name, origin) in [
        ("initial", (40, 40)),
        ("moved", (100, 80)),
        ("restored", (100, 80)),
    ] {
        if name == "moved" {
            ui.set_tool(0);
            ui.invoke_pointer(0, 400.0, 290.0);
            ui.invoke_pointer(1, 460.0, 330.0);
            ui.invoke_pointer(2, 460.0, 330.0);
        } else if name == "restored" {
            ui.invoke_action("undo".into());
            ui.invoke_action("undo".into());
            if ui.get_object_count() != 0 {
                return Err("Pixelation undo retained objects".into());
            }
            ui.invoke_action("redo".into());
            ui.invoke_action("redo".into());
        }
        if !ui.get_output_ready()
            || ui.get_object_count() != 2
            || ui.get_left() != origin.0 as f32
            || ui.get_top() != origin.1 as f32
        {
            return Err("Pixelation gesture/history/move lost edits or output readiness".into());
        }
        let expected = glint_core::composite_selection(
            &rgba,
            640,
            480,
            Rect {
                x: origin.0,
                y: origin.1,
                width: 400,
                height: 300,
            },
            &edits,
        )?;
        let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
        // Toolbar can overlap the lower edge on this deliberately small viewport.
        // This interior includes every pixelation cell and the full redaction.
        for y in 24..220 {
            for x in 8..380 {
                let absolute_y = (y + origin.1) as f32;
                if absolute_y >= ui.get_toolbar_y()
                    && absolute_y < ui.get_toolbar_y() + ui.get_toolbar_total_height()
                {
                    continue;
                }
                let a = ((y * 400 + x) * 4) as usize;
                let b = (((y + origin.1) * 640 + x + origin.0) * 4) as usize;
                if expected[a..a + 4]
                    .iter()
                    .zip(&pixels.as_bytes()[b..b + 4])
                    .any(|(&a, &b)| i16::from(a).abs_diff(i16::from(b)) > 1)
                {
                    return Err(format!(
                        "Pixelation {name} preview/export mismatch at ({x},{y})"
                    ));
                }
            }
        }
        image::save_buffer_with_format(
            directory.join(format!("synthetic-pixelation-{name}.png")),
            pixels.as_bytes(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
