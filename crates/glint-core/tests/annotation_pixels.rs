use glint_core::{
    Annotation, Edits, Point, Rect, Redaction, Stroke, composite, render_annotations,
};

#[test]
fn tiles_match_full_surface_across_strokes_caps_alpha_and_crop_origins() {
    let marks = [
        Annotation::from(Stroke {
            points: vec![
                Point { x: 24.25, y: 38.5 },
                Point {
                    x: 165.75,
                    y: 150.25,
                },
                Point { x: 290.5, y: 50.25 },
            ],
            width: 8.0,
            color: [237, 101, 113, 255],
        }),
        Stroke {
            points: vec![Point { x: 150.25, y: 38.5 }],
            width: 5.0,
            color: [75, 92, 250, 150],
        }
        .into(),
        Redaction {
            bounds: Rect {
                x: 120,
                y: 110,
                width: 40,
                height: 30,
            },
            color: [32, 43, 66, 0],
        }
        .into(),
    ];
    let whole = Rect {
        x: 13,
        y: 17,
        width: 290,
        height: 190,
    };
    let expected = render_annotations(whole, &marks).unwrap();
    for y in (whole.y..whole.y + whole.height).step_by(64) {
        for x in (whole.x..whole.x + whole.width).step_by(64) {
            let crop = Rect {
                x,
                y,
                width: 64,
                height: 64,
            }
            .intersection(whole);
            let tile = render_annotations(crop, &marks).unwrap();
            for ty in 0..crop.height {
                for tx in 0..crop.width {
                    let full_offset =
                        (((y + ty - whole.y) * whole.width + x + tx - whole.x) * 4) as usize;
                    let tile_offset = ((ty * crop.width + tx) * 4) as usize;
                    for c in 0..4 {
                        assert_eq!(
                            expected[full_offset + c],
                            tile[tile_offset + c],
                            "tile seam mismatch at {}, {} channel {c}: full {}, tile {}",
                            x + tx,
                            y + ty,
                            expected[full_offset + c],
                            tile[tile_offset + c]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn object_order_history_and_source_alpha_survive_flattening() {
    let source = vec![255; 64 * 64 * 4];
    let crop = Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    let mut edits = Edits::default();
    assert!(edits.push(Stroke {
        points: vec![Point { x: 10.0, y: 32.0 }, Point { x: 54.0, y: 32.0 }],
        width: 8.0,
        color: [237, 101, 113, 255]
    }));
    let stroke_only = composite(&source, 64, 64, crop, &edits).unwrap();
    let center = ((32 * 64 + 32) * 4) as usize;
    assert_eq!(&stroke_only[center..center + 4], &[237, 101, 113, 255]);
    assert!(edits.push(Redaction {
        bounds: Rect {
            x: 25,
            y: 25,
            width: 14,
            height: 14
        },
        color: [32, 43, 66, 0]
    }));
    let covered = composite(&source, 64, 64, crop, &edits).unwrap();
    assert_eq!(&covered[center..center + 4], &[32, 43, 66, 255]);
    edits.undo();
    assert_eq!(
        composite(&source, 64, 64, crop, &edits).unwrap(),
        stroke_only
    );
    edits.redo();
    assert_eq!(composite(&source, 64, 64, crop, &edits).unwrap(), covered);
    assert_eq!(source, vec![255; 64 * 64 * 4]);
    let transparent = vec![0; source.len()];
    let rgba = composite(&transparent, 64, 64, crop, &edits).unwrap();
    assert_eq!(&rgba[..4], &[0; 4]);
    assert_eq!(&rgba[center..center + 4], &[32, 43, 66, 255]);
}

#[test]
fn invalid_stroke_preserves_redo_and_single_point_is_a_round_dot() {
    let mut edits = Edits::default();
    let dot = Stroke {
        points: vec![Point { x: 10.5, y: 10.5 }],
        width: 5.0,
        color: [1, 2, 3, 255],
    };
    assert!(edits.push(dot.clone()));
    edits.undo();
    assert!(!edits.push(Stroke {
        points: vec![Point {
            x: f32::NAN,
            y: 0.0
        }],
        ..dot.clone()
    }));
    assert!(edits.can_redo());
    edits.redo();
    let pixels = render_annotations(
        Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        },
        edits.marks().iter().map(|m| m.as_ref()),
    )
    .unwrap();
    let center = (10 * 20 + 10) * 4;
    assert_eq!(&pixels[center..center + 4], &[1, 2, 3, 255]);
    assert_eq!(&pixels[..4], &[0; 4]);
}
#[test]
fn annotation_crop_rejects_coordinate_overflow() {
    assert!(
        glint_core::render_annotations(
            glint_core::Rect {
                x: u32::MAX - 127,
                y: 0,
                width: 128,
                height: 128
            },
            std::iter::empty(),
        )
        .is_err()
    );
}
#[test]
fn selection_local_objects_keep_pixels_when_crop_moves_and_returns_from_clipping() {
    use glint_core::{Edits, Point, Rect, Redaction, Stroke, composite_selection};
    let mut edits = Edits::default();
    assert!(edits.push(Stroke {
        points: vec![
            Point { x: 2.25, y: 6.5 },
            Point {
                x: 190.75,
                y: 178.25
            }
        ],
        width: 5.0,
        color: [200, 70, 20, 173]
    }));
    assert!(edits.push(Redaction {
        bounds: Rect {
            x: 120,
            y: 80,
            width: 25,
            height: 30
        },
        color: [32, 43, 66, 0]
    }));
    let mut source = vec![0u8; 400 * 300 * 4];
    for pixel in source.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    let first = Rect {
        x: 17,
        y: 23,
        width: 240,
        height: 210,
    };
    let moved = Rect {
        x: 71,
        y: 83,
        ..first
    };
    let a = composite_selection(&source, 400, 300, first, &edits).unwrap();
    let b = composite_selection(&source, 400, 300, moved, &edits).unwrap();
    assert_eq!(
        a, b,
        "moving the source crop must not change local AA or redaction"
    );
    let small = composite_selection(
        &source,
        400,
        300,
        Rect {
            width: 60,
            height: 50,
            ..moved
        },
        &edits,
    )
    .unwrap();
    for y in 0..50 {
        assert_eq!(
            &small[y * 60 * 4..(y + 1) * 60 * 4],
            &a[y * 240 * 4..y * 240 * 4 + 60 * 4]
        );
    }
    assert_eq!(
        composite_selection(&source, 400, 300, first, &edits).unwrap(),
        a
    );
    let offset = (90 * 240 + 130) * 4;
    assert_eq!(&a[offset..offset + 4], &[32, 43, 66, 255]);
    // The legacy source-global compositor remains available for independent core callers.
    assert_ne!(
        glint_core::composite(&source, 400, 300, moved, &edits).unwrap(),
        a
    );
    source[(83 * 400 + 71) * 4] = 99;
    assert_eq!(
        composite_selection(&source, 400, 300, moved, &edits).unwrap()[0],
        99
    );
    assert_eq!(
        composite_selection(&source, 400, 300, first, &edits).unwrap()[0],
        0
    );
}
