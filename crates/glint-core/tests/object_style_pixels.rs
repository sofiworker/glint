use glint_core::{
    Annotation, Edits, GlyphMask, Pixelation, Point, Rect, Redaction, Shape, ShapeKind, Stroke,
    TextAnnotation, composite_selection,
};
use std::sync::Arc;

#[test]
fn applicable_style_fields_preserve_geometry_and_share_glyphs() {
    let shape = Shape {
        kind: ShapeKind::Arrow { filled: true },
        start: Point { x: 20., y: 30. },
        end: Point { x: 100., y: 70. },
        width: 3.,
        color: [240, 20, 20, 255],
    };
    let mark = Annotation::from(shape.clone())
        .restyled([10, 180, 20, 255], 8., false)
        .unwrap();
    let Annotation::Shape(s) = mark else { panic!() };
    assert_eq!((s.start, s.end), (shape.start, shape.end));
    assert_eq!(s.width, 8.);
    assert_eq!(s.kind, ShapeKind::Arrow { filled: false });
    assert!(
        Annotation::from(shape)
            .restyled([0; 4], f32::NAN, false)
            .is_none()
    );
    let text = TextAnnotation {
        origin: (20, 20),
        text: "shared".into(),
        font: "font".into(),
        size: 22,
        color: [255; 4],
        mask: GlyphMask {
            width: 4,
            height: 4,
            coverage: vec![255; 16].into(),
        },
    };
    let Annotation::Text(changed) = Annotation::from(text.clone())
        .restyled([1, 2, 3, 255], 0., false)
        .unwrap()
    else {
        panic!()
    };
    assert!(Arc::ptr_eq(&text.mask.coverage, &changed.mask.coverage));
    assert_eq!(text.origin, changed.origin);
    assert_eq!(text.text, changed.text);
    let redaction = Annotation::from(Redaction {
        bounds: Rect {
            x: 2,
            y: 2,
            width: 10,
            height: 10,
        },
        color: [0; 4],
    });
    let Annotation::Redaction(r) = redaction.restyled([1, 2, 3, 0], 0., false).unwrap() else {
        panic!()
    };
    assert_eq!(r.color, [1, 2, 3, 255]);
    assert!(
        Annotation::from(Pixelation { bounds: r.bounds })
            .restyled([0; 4], 3., true)
            .is_none()
    );
}

#[test]
fn restyle_damage_covers_width_growth_and_history_restores_filtered_pixels() {
    let source = vec![255; 128 * 128 * 4];
    let crop = Rect {
        x: 0,
        y: 0,
        width: 128,
        height: 128,
    };
    let mut edits = Edits::default();
    edits.push(Stroke {
        points: vec![Point { x: 30., y: 30. }, Point { x: 85., y: 70. }],
        width: 2.,
        color: [255, 0, 0, 255],
    });
    let before = composite_selection(&source, 128, 128, crop, &edits).unwrap();
    let mark = edits.marks()[0]
        .restyled([0, 0, 255, 255], 12., true)
        .unwrap();
    let damage = edits.replace(0, mark).unwrap();
    let after = composite_selection(&source, 128, 128, crop, &edits).unwrap();
    assert_ne!(before, after);
    for (i, (a, b)) in before
        .as_chunks::<4>()
        .0
        .iter()
        .zip(after.as_chunks::<4>().0.iter())
        .enumerate()
    {
        if a != b {
            let (x, y) = ((i % 128) as u32, (i / 128) as u32);
            assert!(
                x >= damage.x
                    && y >= damage.y
                    && x < damage.x + damage.width
                    && y < damage.y + damage.height
            );
        }
    }
    edits.undo();
    assert_eq!(
        composite_selection(&source, 128, 128, crop, &edits).unwrap(),
        before
    );
    edits.redo();
    assert_eq!(
        composite_selection(&source, 128, 128, crop, &edits).unwrap(),
        after
    );
    let mut edits = Edits::default();
    edits.push(Redaction {
        bounds: Rect {
            x: 24,
            y: 24,
            width: 5,
            height: 5,
        },
        color: [0, 0, 0, 255],
    });
    edits.push(Pixelation {
        bounds: Rect {
            x: 16,
            y: 16,
            width: 32,
            height: 32,
        },
    });
    let before = composite_selection(&source, 128, 128, crop, &edits).unwrap();
    let mark = edits.marks()[0]
        .restyled([255, 0, 0, 255], 3., true)
        .unwrap();
    edits.replace(0, mark).unwrap();
    let after = composite_selection(&source, 128, 128, crop, &edits).unwrap();
    let i = (17 * 128 + 17) * 4;
    assert_ne!(&before[i..i + 4], &after[i..i + 4]);
    edits.undo();
    assert_eq!(
        composite_selection(&source, 128, 128, crop, &edits).unwrap(),
        before
    );
    edits.redo();
    assert_eq!(
        composite_selection(&source, 128, 128, crop, &edits).unwrap(),
        after
    );
}
