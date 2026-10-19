use glint_core::{Annotation, Edits, Point, Rect, Shape, ShapeKind, composite, render_annotations};

fn shape(kind: ShapeKind, a: (f32, f32), b: (f32, f32), width: f32) -> Shape {
    Shape {
        kind,
        start: Point { x: a.0, y: a.1 },
        end: Point { x: b.0, y: b.1 },
        width,
        color: [20, 80, 160, 255],
    }
}
fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
    &pixels[(y * width + x) * 4..(y * width + x + 1) * 4]
}

#[test]
fn rectangle_hollow_highlight_translucent_and_arrows_directional() {
    let crop = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 80,
    };
    let rect = Annotation::from(shape(ShapeKind::Rectangle, (60.0, 60.0), (20.0, 20.0), 4.0));
    let pixels = render_annotations(crop, [&rect]).unwrap();
    assert_eq!(pixel(&pixels, 80, 40, 40), &[0; 4]);
    assert_eq!(pixel(&pixels, 80, 20, 40), &[20, 80, 160, 255]);
    let mut edits = Edits::default();
    edits.push(shape(ShapeKind::Highlight, (20.0, 20.0), (60.0, 60.0), 3.0));
    let highlighted = composite(&vec![255; 80 * 80 * 4], 80, 80, crop, &edits).unwrap();
    let p = pixel(&highlighted, 80, 40, 40);
    assert!(p[0] > 140 && p[0] < 160 && p[1] > p[0] && p[2] > p[1] && p[3] == 255);
    assert_eq!(pixel(&highlighted, 80, 10, 10), &[255; 4]);
    let open = Annotation::from(shape(
        ShapeKind::Arrow { filled: false },
        (10.0, 40.0),
        (65.0, 40.0),
        2.0,
    ));
    let filled = Annotation::from(shape(
        ShapeKind::Arrow { filled: true },
        (10.0, 40.0),
        (65.0, 40.0),
        2.0,
    ));
    let a = render_annotations(crop, [&open]).unwrap();
    let b = render_annotations(crop, [&filled]).unwrap();
    assert_eq!(pixel(&a, 80, 52, 44), &[0; 4]);
    assert!(pixel(&b, 80, 52, 44)[3] > 0);
    assert!(pixel(&a, 80, 30, 40)[3] > 0);
    assert_eq!(pixel(&a, 80, 5, 44), &[0; 4]);
}

#[test]
fn shape_bounds_cover_pixels_and_crop_grid_is_exact_in_all_directions() {
    let whole = Rect {
        x: 0,
        y: 0,
        width: 384,
        height: 384,
    };
    let cases = [
        shape(ShapeKind::Rectangle, (300.25, 260.75), (100.5, 120.5), 12.0),
        shape(ShapeKind::Highlight, (80.5, 110.5), (280.0, 270.0), 3.0),
        shape(
            ShapeKind::Arrow { filled: false },
            (130.5, 130.5),
            (280.5, 280.5),
            32.0,
        ),
        shape(
            ShapeKind::Arrow { filled: true },
            (280.5, 280.5),
            (130.5, 130.5),
            8.0,
        ),
        shape(
            ShapeKind::Arrow { filled: false },
            (130.0, 250.0),
            (250.0, 130.0),
            128.0,
        ),
        shape(
            ShapeKind::Arrow { filled: true },
            (250.0, 130.0),
            (130.0, 250.0),
            5.0,
        ),
        shape(
            ShapeKind::Arrow { filled: false },
            (1.0, 1.0),
            (2.0, 1.5),
            8.0,
        ),
    ];
    for shape in cases {
        let mark = Annotation::from(shape);
        let bounds = mark.bounds();
        let pixels = render_annotations(whole, [&mark]).unwrap();
        for y in 0..384 {
            for x in 0..384 {
                if pixel(&pixels, 384, x, y)[3] > 0 {
                    assert!(
                        x as u32 >= bounds.x
                            && y as u32 >= bounds.y
                            && (x as u32) < bounds.x + bounds.width
                            && (y as u32) < bounds.y + bounds.height
                    );
                }
            }
        }
        let crop = Rect {
            x: 117,
            y: 113,
            width: 161,
            height: 149,
        };
        let small = render_annotations(crop, [&mark]).unwrap();
        for y in 0..149 {
            for x in 0..161 {
                assert_eq!(
                    pixel(&small, 161, x, y),
                    pixel(&pixels, 384, x + 117, y + 113)
                );
            }
        }
    }
}

#[test]
fn invalid_shapes_keep_redo_and_shape_history_restores_exact_pixels() {
    let mut edits = Edits::default();
    let crop = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 80,
    };
    edits.push(shape(ShapeKind::Rectangle, (10.0, 10.0), (60.0, 60.0), 3.0));
    edits.push(shape(ShapeKind::Highlight, (20.0, 20.0), (70.0, 70.0), 3.0));
    let expected = render_annotations(crop, edits.marks().iter().map(|m| m.as_ref())).unwrap();
    edits.undo();
    assert!(!edits.push(shape(ShapeKind::Rectangle, (0.0, 0.0), (10.0, 0.0), 3.0)));
    assert!(!edits.push(shape(
        ShapeKind::Arrow { filled: true },
        (0.0, 0.0),
        (f32::NAN, 10.0),
        3.0
    )));
    assert!(!edits.push(shape(
        ShapeKind::Highlight,
        (0.0, 0.0),
        (10.0, 10.0),
        f32::INFINITY
    )));
    assert!(edits.can_redo());
    edits.redo();
    assert_eq!(
        render_annotations(crop, edits.marks().iter().map(|m| m.as_ref())).unwrap(),
        expected
    );
}
