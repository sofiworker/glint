use glint_core::{
    Annotation, Edits, Pixelation, Rect, Redaction, SceneSource, composite, composite_selection,
    render_annotations, render_scene,
};

fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn source(w: u32, h: u32) -> Vec<u8> {
    (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| [x as u8, y as u8, (x ^ y) as u8, 255]))
        .collect()
}
fn pixel(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * width + x) * 4) as usize;
    bytes[at..at + 4].try_into().unwrap()
}

#[test]
fn blocks_average_real_pixels_and_only_inside_the_mark() {
    let source = source(40, 40);
    let mut edits = Edits::default();
    assert!(edits.push(Pixelation {
        bounds: rect(3, 5, 22, 20)
    }));
    let pixels = composite(&source, 40, 40, rect(0, 0, 40, 40), &edits).unwrap();
    assert_eq!(pixel(&pixels, 40, 3, 5), [9, 10, 7, 255]);
    assert_eq!(pixel(&pixels, 40, 15, 15), pixel(&pixels, 40, 3, 5));
    assert_eq!(pixel(&pixels, 40, 16, 5)[..2], [20, 10]);
    assert_eq!(pixel(&pixels, 40, 2, 5), pixel(&source, 40, 2, 5));
    assert_eq!(pixel(&pixels, 40, 25, 24), pixel(&source, 40, 25, 24));
    let mark = Pixelation {
        bounds: rect(0, 0, 2, 2),
    }
    .into();
    assert!(render_annotations(rect(0, 0, 40, 40), [&mark]).is_err());
}

#[test]
fn object_order_never_reveals_redacted_source_and_history_is_exact() {
    let source = source(32, 32);
    let mut edits = Edits::default();
    edits.push(Redaction {
        bounds: rect(0, 0, 32, 32),
        color: [32, 43, 66, 0],
    });
    let redacted = composite(&source, 32, 32, rect(0, 0, 32, 32), &edits).unwrap();
    edits.push(Pixelation {
        bounds: rect(0, 0, 32, 32),
    });
    assert_eq!(
        composite(&source, 32, 32, rect(0, 0, 32, 32), &edits).unwrap(),
        redacted
    );
    edits.push(Redaction {
        bounds: rect(7, 7, 2, 2),
        color: [200, 0, 0, 255],
    });
    let before = composite(&source, 32, 32, rect(0, 0, 32, 32), &edits).unwrap();
    assert_eq!(pixel(&before, 32, 7, 7), [200, 0, 0, 255]);
    edits.undo();
    edits.undo();
    edits.undo();
    assert_eq!(
        composite(&source, 32, 32, rect(0, 0, 32, 32), &edits).unwrap(),
        source
    );
    assert!(!edits.push(Pixelation {
        bounds: rect(u32::MAX, 0, 2, 1)
    }));
    assert!(edits.can_redo());
    edits.redo();
    edits.redo();
    edits.redo();
    assert_eq!(
        composite(&source, 32, 32, rect(0, 0, 32, 32), &edits).unwrap(),
        before
    );
}

#[test]
fn arbitrary_crops_tiles_and_moved_selection_use_identical_source_coordinates() {
    let source = source(350, 280);
    let mut edits = Edits::default();
    edits.push(Pixelation {
        bounds: rect(7, 9, 261, 230),
    });
    edits.push(Redaction {
        bounds: rect(110, 120, 23, 19),
        color: [1, 2, 3, 255],
    });
    edits.push(Pixelation {
        bounds: rect(115, 112, 40, 40),
    });
    let full = composite(&source, 350, 280, rect(0, 0, 350, 280), &edits).unwrap();
    for crop in [
        rect(127, 125, 140, 99),
        rect(3, 5, 132, 135),
        rect(129, 129, 1, 1),
    ] {
        let pixels = composite(&source, 350, 280, crop, &edits).unwrap();
        for y in 0..crop.height {
            for x in 0..crop.width {
                assert_eq!(
                    pixel(&pixels, crop.width, x, y),
                    pixel(&full, 350, x + crop.x, y + crop.y)
                );
            }
        }
    }
    for origin in [(0, 0), (23, 31), (70, 40)] {
        let crop = rect(origin.0, origin.1, 280, 240);
        let exported = composite_selection(&source, 350, 280, crop, &edits).unwrap();
        let rendered = render_scene(
            rect(0, 0, 280, 240),
            edits.marks().iter().map(|m| m.as_ref()),
            SceneSource {
                rgba: &source,
                width: 350,
                height: 280,
                origin,
            },
        )
        .unwrap();
        assert_eq!(exported, rendered);
    }
}

#[test]
fn premultiplied_alpha_is_averaged_and_invalid_sources_rejected() {
    let mut source = vec![255, 0, 0, 255];
    source.extend_from_slice(&[0, 255, 0, 0]);
    let mark = Annotation::from(Pixelation {
        bounds: rect(0, 0, 2, 1),
    });
    let context = SceneSource {
        rgba: &source,
        width: 2,
        height: 1,
        origin: (0, 0),
    };
    let rendered = render_scene(rect(0, 0, 2, 1), [&mark], context).unwrap();
    assert_eq!(rendered, vec![128, 0, 0, 128, 128, 0, 0, 128]);
    let mut edits = Edits::default();
    edits.push(mark);
    assert_eq!(
        composite(&source, 2, 1, rect(0, 0, 2, 1), &edits).unwrap(),
        vec![255, 0, 0, 128, 255, 0, 0, 128]
    );
    assert!(
        render_scene(
            rect(0, 0, 2, 1),
            [],
            SceneSource {
                width: 3,
                ..context
            }
        )
        .is_err()
    );
}

#[test]
fn incremental_perimeter_covers_every_changed_pixel_for_growth_shrink_and_reverse() {
    let source = source(350, 280);
    for (before, after) in [
        (rect(3, 5, 200, 200), rect(3, 5, 210, 210)),
        (rect(3, 5, 210, 210), rect(3, 5, 200, 200)),
        (rect(3, 5, 210, 210), rect(17, 19, 190, 190)),
        (rect(3, 5, 210, 210), rect(220, 230, 30, 30)),
        (rect(200, 200, 0, 0), rect(190, 190, 10, 10)),
    ] {
        let raster = |bounds| {
            let mut edits = Edits::default();
            edits.push(Pixelation { bounds });
            composite(&source, 350, 280, rect(0, 0, 350, 280), &edits).unwrap()
        };
        let a = raster(before);
        let b = raster(after);
        let damage = Pixelation::changed_regions(before, after);
        for y in 0..280 {
            for x in 0..350 {
                if pixel(&a, 350, x, y) != pixel(&b, 350, x, y) {
                    assert!(
                        damage.iter().any(|r| x >= r.x
                            && y >= r.y
                            && x < r.x + r.width
                            && y < r.y + r.height),
                        "Changed pixel ({x},{y}) omitted"
                    );
                }
            }
        }
    }
    assert!(Pixelation::changed_regions(rect(0, 0, 1, 1), rect(0, 0, 1, 1)).is_empty());
}
