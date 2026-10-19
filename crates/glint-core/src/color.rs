/// Read one source pixel. No UI scaling, mask, annotation or color conversion is applied.
pub fn sample_rgba(source: &[u8], size: (u32, u32), point: (u32, u32)) -> Option<[u8; 4]> {
    let width = usize::try_from(size.0).ok()?;
    let height = usize::try_from(size.1).ok()?;
    if point.0 >= size.0
        || point.1 >= size.1
        || width.checked_mul(height)?.checked_mul(4)? != source.len()
    {
        return None;
    }
    let index = usize::try_from(point.1)
        .ok()?
        .checked_mul(width)?
        .checked_add(usize::try_from(point.0).ok()?)?
        .checked_mul(4)?;
    source.get(index..index.checked_add(4)?)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_pixel_bounds_and_overflow_are_checked_without_clamping() {
        let source = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 128];
        assert_eq!(sample_rgba(&source, (2, 2), (0, 0)), Some([1, 2, 3, 255]));
        assert_eq!(
            sample_rgba(&source, (2, 2), (1, 1)),
            Some([10, 11, 12, 128])
        );
        assert_eq!(sample_rgba(&source, (2, 2), (2, 0)), None);
        assert_eq!(sample_rgba(&source, (2, 2), (0, 2)), None);
        assert_eq!(sample_rgba(&source[..15], (2, 2), (0, 0)), None);
        assert_eq!(sample_rgba(&[], (u32::MAX, u32::MAX), (0, 0)), None);
        assert_eq!(sample_rgba(&[], (0, 0), (0, 0)), None);
    }
}
