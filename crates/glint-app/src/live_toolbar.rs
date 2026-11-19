//! Position the floating controls in physical pixels on the capture display.

pub fn position(
    window: &slint::Window,
    work: (i32, i32, u32, u32),
    region: (i32, i32, u32, u32),
    size: slint::LogicalSize,
    scale: f32,
) -> i64 {
    let width = ((size.width * scale).round() as u32).min(work.2);
    let height = ((size.height * scale).round() as u32).min(work.3);
    let gap = (8. * scale).round() as i64;
    let top = i64::from(work.1);
    let last_top = top + i64::from(work.3 - height);
    let below = i64::from(region.1) + i64::from(region.3) + gap;
    let y = if below <= last_top {
        below.max(top)
    } else {
        (i64::from(region.1) - i64::from(height) - gap).clamp(top, last_top)
    };
    let left = i64::from(work.0);
    let x = (i64::from(region.0) + (i64::from(region.2) - i64::from(width)) / 2)
        .clamp(left, left + i64::from(work.2 - width));
    let size = slint::PhysicalSize::new(width, height);
    if window.size() != size {
        window.set_size(size);
    }
    let position = slint::PhysicalPosition::new(x as i32, y as i32);
    if window.position() != position {
        window.set_position(position);
    }
    x + i64::from(width)
}
