use glint_core::scroll_capture::{Append, Fragment, ScrollSession};

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
    let mut session = ScrollSession::default();
    assert_eq!(session.append(region(100, 128)).unwrap(), Append::First);
    assert!(matches!(
        session.append(region(148, 128)).unwrap(),
        Append::Added { overlap: 80 }
    ));
    assert!(matches!(
        session.append_direction(region(52, 128), true).unwrap(),
        Append::Added { overlap: 80 }
    ));
    assert_eq!(session.compose(None).unwrap().rgba, region(52, 224).rgba);
    assert_eq!(
        session.append_direction(region(110, 128), true).unwrap(),
        Append::Duplicate
    );
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(100, 176).rgba);
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(100, 128).rgba);
}

#[test]
fn upward_candidate_preview_preserves_source_until_confirmed() {
    let mut session = ScrollSession::default();
    session.append(region(100, 128)).unwrap();
    let unrelated = Fragment::new(64, 128, [30, 50, 70, 255].repeat(64 * 128)).unwrap();
    assert_eq!(
        session.append_direction(unrelated, true).unwrap(),
        Append::NeedsReview
    );
    let candidate = session.compose(Some(40)).unwrap();
    assert_eq!(candidate.height, 216);
    assert_eq!(session.count(), 1);
    session.accept_pending(40).unwrap();
    assert_eq!(session.compose(None).unwrap().rgba, candidate.rgba);
    session.undo();
    assert_eq!(session.compose(None).unwrap().rgba, region(100, 128).rgba);
}
