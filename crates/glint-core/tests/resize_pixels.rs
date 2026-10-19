use glint_core::{
    Annotation, Edits, Pixelation, Point, Rect, Redaction, Shape, ShapeKind, Stroke,
    composite_selection,
};
fn rectangle() -> Annotation {
    Redaction {
        bounds: Rect {
            x: 20,
            y: 30,
            width: 80,
            height: 60,
        },
        color: [0, 0, 0, 255],
    }
    .into()
}

#[test]
fn eight_handles_keep_opposite_edges_and_minimum_without_flipping() {
    let mark = rectangle();
    assert_eq!(mark.resize_handles().len(), 8);
    for id in 0..8 {
        let resized = mark.resized(id, 10, 15, (200, 200)).unwrap();
        let r = resized.bounds();
        assert_eq!(r.x, if [0, 6, 7].contains(&id) { 30 } else { 20 });
        assert_eq!(r.y, if [0, 1, 2].contains(&id) { 45 } else { 30 });
        assert_eq!(
            r.x + r.width,
            if [2, 3, 4].contains(&id) { 110 } else { 100 }
        );
        assert_eq!(
            r.y + r.height,
            if [4, 5, 6].contains(&id) { 105 } else { 90 }
        );
        assert_eq!(mark.resized(id, 0, 0, (200, 200)).unwrap(), mark);
    }
    assert_eq!(
        mark.resized(0, i64::MAX, i64::MAX, (200, 200))
            .unwrap()
            .bounds(),
        Rect {
            x: 99,
            y: 89,
            width: 1,
            height: 1
        }
    );
    assert_eq!(
        mark.resized(4, i64::MAX, i64::MAX, (200, 200))
            .unwrap()
            .bounds(),
        Rect {
            x: 20,
            y: 30,
            width: 180,
            height: 170
        }
    );
    assert_eq!(
        mark.resized(0, i64::MIN, i64::MIN, (200, 200))
            .unwrap()
            .bounds(),
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 90
        }
    );
    assert!(mark.resized(8, 5, 5, (200, 200)).is_none());
    assert!(mark.resized(0, 0, 0, (0, 200)).is_none());
    let shape: Annotation = Shape {
        kind: ShapeKind::Rectangle,
        start: Point { x: 100., y: 90. },
        end: Point { x: 20., y: 30. },
        width: 3.,
        color: [255; 4],
    }
    .into();
    let Annotation::Shape(s) = shape.resized(0, 10, 15, (200, 200)).unwrap() else {
        panic!()
    };
    assert_eq!(s.start, Point { x: 100., y: 90. });
    assert_eq!(s.end, Point { x: 30., y: 45. });
}

#[test]
fn arrow_endpoint_and_hit_testing_handle_clipping_and_invalid_collapse() {
    let arrow: Annotation = Shape {
        kind: ShapeKind::Arrow { filled: false },
        start: Point { x: 10., y: 15. },
        end: Point { x: 90., y: 70. },
        width: 3.,
        color: [255, 0, 0, 255],
    }
    .into();
    assert_eq!(
        arrow
            .resize_handles()
            .iter()
            .map(|h| h.id)
            .collect::<Vec<_>>(),
        vec![8, 9]
    );
    let Annotation::Shape(m) = arrow.resized(8, -30, 20, (100, 100)).unwrap() else {
        panic!()
    };
    assert_eq!(m.start, Point { x: 0., y: 35. });
    assert_eq!(m.end, Point { x: 90., y: 70. });
    assert_eq!(m.width, 3.);
    assert!(arrow.resized(8, 80, 55, (100, 100)).is_none());
    assert!(arrow.resized(0, 10, 10, (100, 100)).is_none());
    assert_eq!(
        arrow.resize_handle_hit(Point { x: 12., y: 17. }, (6., 6.), (100, 100)),
        Some(8)
    );
    assert_eq!(
        arrow.resize_handle_hit(Point { x: 90., y: 70. }, (6., 6.), (50, 50)),
        None
    );
    assert_eq!(
        arrow.resize_handle_hit(Point { x: f32::NAN, y: 0. }, (6., 6.), (100, 100)),
        None
    );
    let tiny: Annotation = Redaction {
        bounds: Rect {
            x: 10,
            y: 10,
            width: 2,
            height: 2,
        },
        color: [0; 4],
    }
    .into();
    assert_eq!(
        tiny.resize_handle_hit(Point { x: 12., y: 12. }, (6., 6.), (100, 100)),
        Some(4)
    );
}

#[test]
fn pen_path_scales_from_original_geometry_without_stretching_width() {
    let pen: Annotation = Stroke {
        points: vec![
            Point { x: 20., y: 30. },
            Point { x: 60., y: 60. },
            Point { x: 100., y: 90. },
        ],
        width: 5.,
        color: [255; 4],
    }
    .into();
    let Annotation::Stroke(s) = pen.resized(4, 80, 60, (300, 300)).unwrap() else {
        panic!()
    };
    assert_eq!(s.width, 5.);
    assert_eq!(
        s.points,
        vec![
            Point { x: 20., y: 30. },
            Point { x: 100., y: 90. },
            Point { x: 180., y: 150. }
        ]
    );
    let horizontal: Annotation = Stroke {
        points: vec![Point { x: 10., y: 20. }, Point { x: 40., y: 20. }],
        width: 3.,
        color: [255; 4],
    }
    .into();
    assert_eq!(
        horizontal
            .resize_handles()
            .iter()
            .map(|h| h.id)
            .collect::<Vec<_>>(),
        vec![3, 7]
    );
    let Annotation::Stroke(s) = horizontal.resized(3, 20, 80, (100, 100)).unwrap() else {
        panic!()
    };
    assert_eq!(s.points[1], Point { x: 60., y: 20. });
    assert!(horizontal.resized(1, 0, 10, (100, 100)).is_none());
    let dot: Annotation = Stroke {
        points: vec![Point { x: 10., y: 10. }],
        width: 3.,
        color: [255; 4],
    }
    .into();
    assert!(dot.resize_handles().is_empty());
}

#[test]
fn resize_history_preserves_order_and_dependent_pixelation_pixels() {
    let source = vec![255; 128 * 128 * 4];
    let crop = Rect {
        x: 0,
        y: 0,
        width: 128,
        height: 128,
    };
    let mut edits = Edits::default();
    edits.push(Redaction {
        bounds: Rect {
            x: 24,
            y: 24,
            width: 4,
            height: 4,
        },
        color: [0, 0, 0, 255],
    });
    edits.push(Pixelation {
        bounds: Rect {
            x: 16,
            y: 16,
            width: 64,
            height: 64,
        },
    });
    let before = composite_selection(&source, 128, 128, crop, &edits).unwrap();
    let mark = edits.marks()[0].resized(4, 30, 30, (128, 128)).unwrap();
    edits.replace(0, mark).unwrap();
    let after = composite_selection(&source, 128, 128, crop, &edits).unwrap();
    assert_ne!(before, after);
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
    edits.undo();
    let mark = edits.marks()[0].resized(0, 0, 0, (128, 128)).unwrap();
    edits.replace(0, mark).unwrap();
    assert!(edits.can_redo());
}
