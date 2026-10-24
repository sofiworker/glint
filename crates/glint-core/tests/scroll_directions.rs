use glint_core::scroll_capture::{Append, Fragment, ScrollSequence};

fn region(start: usize, height: usize) -> Fragment {
    let mut pixels = Vec::new();
    for y in start..start + height {
        for x in 0..64usize {
            // A stable, nonrepeating, detailed document texture.
            let seed = (y as u32).wrapping_mul(1_664_525) ^ (x as u32).wrapping_mul(1_013_904_223);
            pixels.extend_from_slice(&[
                (seed >> 3) as u8,
                (seed >> 13) as u8,
                (seed >> 23) as u8,
                255,
            ]);
        }
    }
    Fragment::new(64, height as u32, pixels).unwrap()
}

#[test]
fn grows_both_ends_revisits_and_undoes_in_operation_order() {
    let mut session = ScrollSequence::default();
    assert_eq!(session.append(region(100, 128)).unwrap(), Append::First);
    assert!(matches!(
        session.append(region(148, 128)).unwrap(),
        Append::Added { overlap: 80 }
    ));
    assert!(matches!(
        session.capture(region(52, 128), true).unwrap(),
        Append::Added { overlap: 80 }
    ));
    assert_eq!(session.compose(None).unwrap().rgba, region(52, 224).rgba);
    assert_eq!(
        session.capture(region(110, 128), true).unwrap(),
        Append::Duplicate
    );
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(100, 176).rgba);
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(100, 128).rgba);
}

#[test]
fn upward_candidate_preview_preserves_source_until_confirmed() {
    let mut session = ScrollSequence::default();
    session.append(region(100, 128)).unwrap();
    let unrelated = Fragment::new(64, 128, [30, 50, 70, 255].repeat(64 * 128)).unwrap();
    assert_eq!(
        session.capture(unrelated, true).unwrap(),
        Append::NeedsReview
    );
    let candidate = session.compose(Some(40)).unwrap();
    assert_eq!(candidate.height, 216);
    assert_eq!(session.count(), 2);
    session.accept_pending(40).unwrap();
    assert_eq!(session.compose(None).unwrap().rgba, candidate.rgba);
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(100, 128).rgba);
}

#[test]
fn up_then_down_revisits_visited_rows_and_extends_below() {
    let mut session = ScrollSequence::default();
    assert_eq!(session.append(region(100, 128)).unwrap(), Append::First);
    assert!(matches!(
        session.capture(region(52, 128), true).unwrap(),
        Append::Added { overlap: 80 }
    ));
    // Scrolling back down over captured rows must not duplicate them.
    assert_eq!(
        session.capture(region(90, 128), true).unwrap(),
        Append::Duplicate
    );
    assert!(matches!(
        session.capture(region(148, 128), false).unwrap(),
        Append::Added { overlap: 80 }
    ));
    assert_eq!(session.compose(None).unwrap().rgba, region(52, 224).rgba);
    assert_eq!(session.unresolved_count(), 0);
}

#[test]
fn down_then_up_revisits_and_extends_above() {
    let mut session = ScrollSequence::default();
    assert_eq!(session.append(region(100, 128)).unwrap(), Append::First);
    assert!(matches!(
        session.append(region(148, 128)).unwrap(),
        Append::Added { overlap: 80 }
    ));
    assert_eq!(
        session.capture(region(110, 128), true).unwrap(),
        Append::Duplicate
    );
    assert!(matches!(
        session.capture(region(52, 128), true).unwrap(),
        Append::Added { overlap: 80 }
    ));
    assert_eq!(session.compose(None).unwrap().rgba, region(52, 224).rgba);
    assert_eq!(session.unresolved_count(), 0);
}

#[test]
fn undo_after_bidirectional_scroll_removes_frames_in_capture_order() {
    let mut session = ScrollSequence::default();
    session.append(region(100, 128)).unwrap();
    session.capture(region(52, 128), true).unwrap();
    assert_eq!(
        session.capture(region(90, 128), true).unwrap(),
        Append::Duplicate
    );
    assert!(matches!(
        session.capture(region(148, 128), false).unwrap(),
        Append::Added { overlap: 80 }
    ));
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(52, 176).rgba);
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(100, 128).rgba);
}
