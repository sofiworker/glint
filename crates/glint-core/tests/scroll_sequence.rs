use glint_core::scroll_capture::{Append, Fragment, ScrollSequence};

fn document(start: usize, height: usize) -> Fragment {
    let mut pixels = Vec::new();
    for y in start..start + height {
        for x in 0..96usize {
            let seed = (y as u32).wrapping_mul(1_664_525) ^ (x as u32).wrapping_mul(1_013_904_223);
            pixels.extend_from_slice(&[
                (seed >> 3) as u8,
                (seed >> 13) as u8,
                (seed >> 23) as u8,
                255,
            ]);
        }
    }
    Fragment::new(96, height as u32, pixels).unwrap()
}

#[test]
fn alignment_failure_does_not_break_next_adjacent_pair_or_frame_order() {
    let mut sequence = ScrollSequence::default();
    sequence.capture(document(0, 128), false).unwrap();
    assert_eq!(
        sequence.capture(document(240, 128), false).unwrap(),
        Append::NeedsReview
    );
    assert_eq!(
        sequence.capture(document(288, 128), false).unwrap(),
        Append::Added { overlap: 80 }
    );
    assert_eq!(sequence.count(), 3);
    assert_eq!(sequence.unresolved_count(), 1);
    assert_eq!(sequence.review_position(), Some((1, 2)));
    let preview = sequence.compose(None).unwrap();
    assert_eq!(preview.height, 128);
    assert!(preview.rgba[..96 * 128 * 4] == document(0, 128).rgba);
    let retained = sequence.compose(Some(0)).unwrap();
    assert!(retained.rgba[96 * 128 * 4..] == document(240, 176).rgba);
    sequence.retry_alignment();
    assert_eq!(sequence.count(), 3);
    assert_eq!(sequence.unresolved_count(), 1);
    sequence.accept_pending(0).unwrap();
    assert!(sequence.compose(None).unwrap().rgba == retained.rgba);
}

#[test]
fn central_matching_ignores_scrollbars_and_small_changing_regions() {
    let mut sequence = ScrollSequence::default();
    let first = document(0, 160);
    let mut second = document(48, 160);
    for y in 0..160usize {
        for x in [0, 1, 94, 95] {
            second.rgba[(y * 96 + x) * 4..(y * 96 + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    for y in 50..54usize {
        for x in 20..45usize {
            second.rgba[(y * 96 + x) * 4] = 255;
        }
    }
    sequence.capture(first, false).unwrap();
    assert_eq!(
        sequence.capture(second, false).unwrap(),
        Append::Added { overlap: 112 }
    );
}

#[test]
fn uncertain_frames_are_all_kept_until_post_capture_review() {
    let mut sequence = ScrollSequence::default();
    for shade in [20, 70, 130] {
        sequence
            .capture(
                Fragment::new(96, 128, [shade, 0, 0, 255].repeat(96 * 128)).unwrap(),
                false,
            )
            .unwrap();
    }
    assert_eq!(sequence.count(), 3);
    assert_eq!(sequence.unresolved_count(), 2);
    assert_eq!(sequence.compose(None).unwrap().height, 128);
    assert_eq!(sequence.compose(Some(0)).unwrap().height, 384);
    assert_eq!(sequence.review_position(), Some((1, 2)));
    sequence.review_next();
    assert_eq!(sequence.review_position(), Some((2, 3)));
    sequence.accept_pending(32).unwrap();
    assert_eq!(sequence.review_position(), Some((1, 2)));
    sequence.discard_pending();
    assert_eq!(sequence.count(), 2);
    assert_eq!(sequence.unresolved_count(), 1);
    assert_eq!(sequence.compose(None).unwrap().height, 128);
    assert_eq!(sequence.compose(Some(0)).unwrap().rgba[96 * 128 * 4], 130);
}

#[test]
fn up_down_and_undo_preserve_acquisition_operations() {
    let mut sequence = ScrollSequence::default();
    sequence.capture(document(100, 128), false).unwrap();
    sequence.capture(document(148, 128), false).unwrap();
    sequence.capture(document(52, 128), true).unwrap();
    assert!(sequence.compose(None).unwrap().rgba == document(52, 224).rgba);
    sequence.undo();
    assert!(sequence.compose(None).unwrap().rgba == document(100, 176).rgba);
    sequence.undo();
    assert!(sequence.compose(None).unwrap().rgba == document(100, 128).rgba);
}

#[test]
fn live_thumbnail_samples_ordered_sources_without_full_composition() {
    let mut sequence = ScrollSequence::default();
    sequence.capture(document(0, 128), false).unwrap();
    sequence.capture(document(240, 128), false).unwrap();
    sequence.capture(document(288, 128), false).unwrap();
    for proposed in [None, Some(32)] {
        let full = sequence.compose(proposed).unwrap();
        let (thumbnail, height) = sequence.thumbnail(proposed).unwrap();
        assert_eq!(height, full.height);
        assert_eq!(
            (thumbnail.width, thumbnail.height),
            (full.width, full.height)
        );
        assert!(thumbnail.rgba == full.rgba);
    }
}

#[test]
fn source_frame_limit_does_not_discard_last_frame_or_result() {
    let mut sequence = ScrollSequence::default();
    for shade in 0..128u8 {
        sequence
            .capture(
                Fragment::new(32, 64, [shade, 0, 0, 255].repeat(32 * 64)).unwrap(),
                false,
            )
            .unwrap();
    }
    assert_eq!(sequence.count(), 128);
    let before = sequence.compose(None).unwrap();
    assert_eq!(
        sequence.capture(
            Fragment::new(32, 64, [200, 0, 0, 255].repeat(32 * 64)).unwrap(),
            false
        ),
        Err(glint_core::scroll_capture::StitchError::Budget)
    );
    assert_eq!(sequence.count(), 128);
    assert!(before.rgba == sequence.compose(None).unwrap().rgba);
}

// Rasterized decimal text, not random texture: whitespace and similar rows
// exercise the ambiguity checks which sparse RGB samples previously rejected.
fn text_document(start: usize, height: usize) -> Fragment {
    const DIGITS: [[u8; 7]; 10] = [
        [14, 17, 19, 21, 25, 17, 14],
        [4, 12, 4, 4, 4, 4, 14],
        [14, 17, 1, 2, 4, 8, 31],
        [30, 1, 1, 14, 1, 1, 30],
        [2, 6, 10, 18, 31, 2, 2],
        [31, 16, 16, 30, 1, 1, 30],
        [14, 16, 16, 30, 17, 17, 14],
        [31, 1, 2, 4, 8, 8, 8],
        [14, 17, 17, 14, 17, 17, 14],
        [14, 17, 17, 15, 1, 1, 14],
    ];
    let mut rgba = vec![255; 256 * height * 4];
    for y in 0..height {
        let source_y = y + start;
        let line = source_y / 20;
        let glyph_y = source_y % 20;
        if !(2..16).contains(&glyph_y) {
            continue;
        }
        let text = format!("{line:05}{:06}", line * 37);
        for (i, digit) in text.bytes().enumerate() {
            for x in 0..10 {
                if DIGITS[(digit - b'0') as usize][(glyph_y - 2) / 2] & (1 << (4 - x / 2)) != 0 {
                    let at = (y * 256 + 12 + i * 16 + x) * 4;
                    rgba[at..at + 3].copy_from_slice(&[24, 35, 52]);
                }
            }
        }
    }
    Fragment::new(256, height as u32, rgba).unwrap()
}

#[test]
fn text_matches_exact_integer_offsets_and_exports_original_pixels() {
    for shift in [37, 48, 83] {
        let mut sequence = ScrollSequence::default();
        sequence.append(text_document(0, 192)).unwrap();
        assert_eq!(
            sequence.append(text_document(shift, 192)).unwrap(),
            Append::Added {
                overlap: 192 - shift as u32
            }
        );
        assert_eq!(
            sequence.compose(None).unwrap().rgba,
            text_document(0, 192 + shift).rgba
        );
    }
}

#[test]
fn fixed_header_and_footer_appear_once_without_resampling_content() {
    let decorate = |mut frame: Fragment, top: bool, bottom: bool| {
        if top {
            for pixel in frame.rgba[..24 * 256 * 4].as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&[10, 40, 80, 255]);
            }
        }
        if bottom {
            for pixel in frame.rgba[(frame.height as usize - 20) * 256 * 4..]
                .as_chunks_mut::<4>()
                .0
            {
                pixel.copy_from_slice(&[80, 40, 10, 255]);
            }
        }
        frame
    };
    let mut sequence = ScrollSequence::default();
    for start in [0, 48, 96] {
        sequence
            .append(decorate(text_document(start, 192), true, true))
            .unwrap();
    }
    assert_eq!(sequence.unresolved_count(), 0);
    let output = sequence.compose(None).unwrap();
    assert_eq!(output.height, 288);
    assert_eq!(
        output.rgba,
        decorate(text_document(0, 288), true, true).rgba
    );
}

#[test]
fn cursor_changes_do_not_create_a_new_viewport_or_seam() {
    let mut sequence = ScrollSequence::default();
    sequence.append(text_document(0, 192)).unwrap();
    let mut next = text_document(0, 192);
    for y in 90..94 {
        for x in 200..204 {
            next.rgba[(y * 256 + x) * 4..(y * 256 + x) * 4 + 3].fill(0);
        }
    }
    assert_eq!(sequence.append(next).unwrap(), Append::Duplicate);
    assert_eq!(sequence.count(), 1);
}

#[test]
fn reacquiring_real_overlap_recovers_a_transient_frame_automatically() {
    let mut sequence = ScrollSequence::default();
    sequence.append(text_document(0, 192)).unwrap();
    sequence
        .append(Fragment::new(256, 192, [80, 30, 10, 255].repeat(256 * 192)).unwrap())
        .unwrap();
    assert_eq!(sequence.unresolved_count(), 1);
    sequence.append(text_document(48, 192)).unwrap();
    assert_eq!(sequence.unresolved_count(), 0);
    assert_eq!(sequence.count(), 2);
    assert_eq!(
        sequence.compose(None).unwrap().rgba,
        text_document(0, 240).rgba
    );
}

#[test]
fn manual_large_overlap_remains_valid_before_an_automatic_join() {
    let mut sequence = ScrollSequence::default();
    sequence
        .append(Fragment::new(96, 128, [80, 30, 10, 255].repeat(96 * 128)).unwrap())
        .unwrap();
    sequence.append(document(0, 128)).unwrap();
    assert!(matches!(
        sequence.compose(Some(u32::MAX)),
        Err(glint_core::scroll_capture::StitchError::InvalidOverlap)
    ));
    assert_eq!(
        sequence.append(document(1, 128)).unwrap(),
        Append::Added { overlap: 127 }
    );
    let proposed = sequence.compose(Some(127)).unwrap();
    sequence.accept_pending(127).unwrap();
    let accepted = sequence.compose(None).unwrap();
    assert_eq!(accepted.height, 130);
    assert_eq!(accepted.rgba, proposed.rgba);
}

#[test]
fn repeated_pattern_requires_evidence_instead_of_guessing_a_period() {
    let periodic = |offset| {
        let mut frame = document(0, 128);
        let pattern = document(0, 64);
        for y in 0..128usize {
            let start = (y + offset) % 16 * 96 * 4;
            frame.rgba[y * 96 * 4..(y + 1) * 96 * 4]
                .copy_from_slice(&pattern.rgba[start..start + 96 * 4]);
        }
        frame
    };
    let mut sequence = ScrollSequence::default();
    sequence.append(periodic(0)).unwrap();
    assert_eq!(sequence.append(periodic(5)).unwrap(), Append::NeedsReview);
    assert_eq!(sequence.count(), 2);
    assert_eq!(sequence.compose(None).unwrap().height, 128);
}

#[test]
fn frame_limit_compacts_only_verified_redundancy_without_changing_output() {
    let mut sequence = ScrollSequence::default();
    for step in 0..130 {
        sequence.append(document(step * 3, 128)).unwrap();
    }
    assert!(sequence.count() <= 128);
    assert_eq!(sequence.unresolved_count(), 0);
    assert_eq!(sequence.compose(None).unwrap().rgba, document(0, 515).rgba);
}
