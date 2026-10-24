use glint_core::scroll_capture::{Append, Fragment, ScrollSequence};

fn region(start: usize) -> Fragment {
    let mut pixels = Vec::new();
    for y in start..start + 128 {
        for x in 0..64usize {
            let seed = (y as u32).wrapping_mul(1_664_525) ^ (x as u32).wrapping_mul(1_013_904_223);
            pixels.extend_from_slice(&[
                (seed >> 3) as u8,
                (seed >> 13) as u8,
                (seed >> 23) as u8,
                255,
            ]);
        }
    }
    Fragment::new(64, 128, pixels).unwrap()
}
#[test]
fn failed_match_keeps_pixels_and_later_capture_can_recover() {
    let mut session = ScrollSequence::default();
    session.capture(region(100), false).unwrap();
    assert_eq!(
        session
            .capture(
                Fragment::new(64, 128, [30, 50, 70, 255].repeat(64 * 128)).unwrap(),
                false
            )
            .unwrap(),
        Append::NeedsReview
    );
    // A further disconnected source stays next to the unresolved join.
    assert_eq!(
        session
            .capture(
                Fragment::new(64, 128, [10, 120, 200, 255].repeat(64 * 128)).unwrap(),
                false
            )
            .unwrap(),
        Append::NeedsReview
    );
    assert_eq!(session.unresolved_count(), 2);
    assert_eq!(session.dimensions(), Some((64, 128)));
    session.accept_pending(0).unwrap();
    assert_eq!(session.compose(None).unwrap().height, 256);
    session.discard_pending();
    assert_eq!(session.unresolved_count(), 0);
}
#[test]
fn repeated_failed_matches_are_retained_and_reviewable() {
    let mut session = ScrollSequence::default();
    session.capture(region(100), false).unwrap();
    for shade in [30, 50, 70] {
        assert_eq!(
            session
                .capture(
                    Fragment::new(64, 128, [shade, 0, 0, 255].repeat(64 * 128)).unwrap(),
                    false
                )
                .unwrap(),
            Append::NeedsReview
        );
    }
    assert_eq!(session.unresolved_count(), 3);
    for shade in [30, 50, 70] {
        assert_eq!(session.compose(Some(0)).unwrap().rgba[128 * 64 * 4], shade);
        session.discard_pending();
    }
    assert_eq!(session.unresolved_count(), 0);
}

#[test]
fn unresolved_frames_share_budget_and_are_preserved_on_limit() {
    let mut session = ScrollSequence::default();
    session.capture(region(100), false).unwrap();
    for shade in 0..127u8 {
        session
            .capture(
                Fragment::new(64, 128, [shade, 0, 0, 255].repeat(64 * 128)).unwrap(),
                false,
            )
            .unwrap();
    }
    assert_eq!(session.unresolved_count(), 127);
    let before = session.compose(Some(0)).unwrap().rgba;
    assert_eq!(
        session.capture(
            Fragment::new(64, 128, [200, 0, 0, 255].repeat(64 * 128)).unwrap(),
            false
        ),
        Err(glint_core::scroll_capture::StitchError::Budget)
    );
    assert_eq!(session.unresolved_count(), 127);
    assert!(before == session.compose(Some(0)).unwrap().rgba);
}
