//! Bounded sampling of frozen source pixels, independent of display overlays.
pub const MAGNIFIER_SIDE: u32 = 11;
pub type MagnifierPixels = [u8; 11 * 11 * 4];

/// Transparent padding represents pixels outside the captured desktop.
pub fn magnifier_rgba(
    source: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> Option<MagnifierPixels> {
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if source.len() != expected || x >= width || y >= height {
        return None;
    }
    let mut pixels = [0; 11 * 11 * 4];
    for row in 0..11usize {
        for col in 0..11usize {
            let sx = i64::from(x) + col as i64 - 5;
            let sy = i64::from(y) + row as i64 - 5;
            if sx >= 0 && sy >= 0 && sx < i64::from(width) && sy < i64::from(height) {
                let from = (sy as usize * width as usize + sx as usize) * 4;
                let to = (row * 11 + col) * 4;
                pixels[to..to + 4].copy_from_slice(&source[from..from + 4]);
            }
        }
    }
    Some(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_exact_source_and_pads_edges_without_repeating_them() {
        let source: Vec<u8> = (0..13 * 13).flat_map(|i| [i as u8, 31, 72, 255]).collect();
        let p = magnifier_rgba(&source, 13, 13, 6, 6).unwrap();
        assert_eq!(&p[0..4], &source[14 * 4..15 * 4]);
        assert_eq!(&p[60 * 4..61 * 4], &source[84 * 4..85 * 4]);
        assert_eq!(&p[120 * 4..], &source[154 * 4..155 * 4]);
        let edge = magnifier_rgba(&source, 13, 13, 0, 0).unwrap();
        assert_eq!(&edge[..60 * 4], &[0; 60 * 4]);
        assert_eq!(&edge[60 * 4..61 * 4], &source[..4]);
        let edge = magnifier_rgba(&source, 13, 13, 12, 12).unwrap();
        assert_eq!(&edge[61 * 4..], &[0; 60 * 4]);
    }
    #[test]
    fn rejects_bad_lengths_empty_images_outside_points_and_overflow() {
        for (w, h, x, y) in [(0, 0, 0, 0), (1, 1, 0, 0), (u32::MAX, u32::MAX, 0, 0)] {
            assert!(magnifier_rgba(&[], w, h, x, y).is_none());
        }
        assert!(magnifier_rgba(&[0; 4], 1, 1, 1, 0).is_none());
        assert!(magnifier_rgba(&[0; 4], 1, 1, 0, u32::MAX).is_none());
        assert_eq!(
            magnifier_rgba(&[1, 2, 3, 255], 1, 1, 0, 0).unwrap()[240..244],
            [1, 2, 3, 255]
        );
    }
}
