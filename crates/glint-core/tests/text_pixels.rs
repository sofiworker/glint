use glint_core::{
    Annotation, Edits, GlyphMask, Rect, TextAnnotation, composite, render_annotations,
};

fn text() -> TextAnnotation {
    TextAnnotation {
        origin: (127, 127),
        text: "Glint 拾光".into(),
        font: "Test font metadata".into(),
        size: 22,
        color: [200, 100, 50, 128],
        mask: GlyphMask {
            width: 2,
            height: 2,
            coverage: vec![0, 64, 128, 255].into(),
        },
    }
}

#[test]
fn coverage_alpha_object_order_and_canonical_crop_are_identical() {
    let mark = Annotation::from(text());
    let crop = Rect {
        x: 127,
        y: 127,
        width: 2,
        height: 2,
    };
    let layer = render_annotations(crop, [&mark]).unwrap();
    assert_eq!(
        layer,
        vec![0, 0, 0, 0, 25, 13, 6, 32, 50, 25, 13, 64, 100, 50, 25, 128]
    );
    let mut edits = Edits::default();
    edits.push(text());
    let source = vec![0; 130 * 130 * 4];
    let output = composite(&source, 130, 130, crop, &edits).unwrap();
    assert_eq!(output[15], 128);
    let all = render_annotations(
        Rect {
            x: 0,
            y: 0,
            width: 130,
            height: 130,
        },
        edits.marks().iter().map(|m| m.as_ref()),
    )
    .unwrap();
    for y in 0..2usize {
        for x in 0..2usize {
            assert_eq!(
                layer[(y * 2 + x) * 4..(y * 2 + x + 1) * 4],
                all[((y + 127) * 130 + x + 127) * 4..((y + 127) * 130 + x + 128) * 4]
            );
        }
    }
    edits.undo();
    assert!(edits.marks().is_empty());
    edits.redo();
    assert_eq!(composite(&source, 130, 130, crop, &edits).unwrap(), output);
}

#[test]
fn invalid_text_and_bad_mask_keep_redo_and_shared_glyphs_do_not_copy() {
    let mut edits = Edits::default();
    let mark = text();
    let glyphs = mark.mask.coverage.clone();
    edits.push(mark.clone());
    edits.undo();
    for invalid in [
        TextAnnotation {
            text: " ".into(),
            ..mark.clone()
        },
        TextAnnotation {
            text: "a\nb".into(),
            ..mark.clone()
        },
        TextAnnotation {
            text: "a".repeat(81),
            ..mark.clone()
        },
        TextAnnotation {
            origin: (u32::MAX, 0),
            ..mark.clone()
        },
        TextAnnotation {
            size: 0,
            ..mark.clone()
        },
        TextAnnotation {
            mask: GlyphMask {
                width: 2,
                height: 3,
                ..mark.mask.clone()
            },
            ..mark.clone()
        },
    ] {
        assert!(!edits.push(invalid));
        assert!(edits.can_redo());
    }
    edits.redo();
    let Annotation::Text(restored) = edits.marks()[0].as_ref() else {
        panic!("wrong object")
    };
    assert!(std::sync::Arc::ptr_eq(&glyphs, &restored.mask.coverage));
    assert_eq!(restored.text, mark.text);
}
