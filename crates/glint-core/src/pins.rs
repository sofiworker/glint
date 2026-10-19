//! Conservative source and window-buffer accounting; no platform window handles.
use std::collections::HashMap;

/// Accumulate logical pixel deltas (Slint maps one wheel line to 60 pixels).
/// Each pin owns its remainder; changing mode or direction discards old input.
#[derive(Default)]
pub struct WheelSteps {
    remainder: f32,
    opacity: bool,
}
impl WheelSteps {
    pub fn push(&mut self, delta: f32, opacity: bool) -> i32 {
        if !delta.is_finite() || delta == 0. {
            return 0;
        }
        if self.opacity != opacity || self.remainder.signum() != delta.signum() {
            self.remainder = 0.;
        }
        self.opacity = opacity;
        self.remainder += delta.clamp(-600., 600.);
        let steps = (self.remainder / 60.).trunc() as i32;
        self.remainder -= steps as f32 * 60.;
        steps
    }
}

#[cfg(test)]
mod wheel_tests {
    use super::WheelSteps;
    #[test]
    fn fine_input_direction_and_mode_are_independent() {
        let mut wheel = WheelSteps::default();
        assert_eq!(wheel.push(20., false), 0);
        assert_eq!(wheel.push(20., false), 0);
        assert_eq!(wheel.push(20., false), 1);
        assert_eq!(wheel.push(50., false), 0);
        assert_eq!(wheel.push(-20., false), 0);
        assert_eq!(wheel.push(-40., false), -1);
        assert_eq!(wheel.push(50., false), 0);
        assert_eq!(wheel.push(20., true), 0);
        assert_eq!(wheel.push(40., true), 1);
        assert_eq!(WheelSteps::default().push(20., true), 0);
    }
    #[test]
    fn malformed_and_extreme_input_is_bounded() {
        let mut wheel = WheelSteps::default();
        assert_eq!(wheel.push(f32::NAN, false), 0);
        assert_eq!(wheel.push(f32::INFINITY, false), 0);
        assert_eq!(wheel.push(0., false), 0);
        assert_eq!(wheel.push(f32::MAX, false), 10);
        assert_eq!(wheel.push(-f32::MAX, false), -10);
        assert_eq!(wheel.push(120., false), 2);
    }
}

#[derive(Default)]
pub struct PinBudget {
    limit: u64,
    charges: HashMap<u64, u64>,
}
impl PinBudget {
    pub fn set_limit(&mut self, bytes: u64) {
        self.limit = bytes;
    }
    pub fn used(&self) -> u64 {
        self.charges.values().sum()
    }
    pub fn count(&self) -> usize {
        self.charges.len()
    }
    pub fn reserve(&mut self, id: u64, charge: u64) -> Result<(), &'static str> {
        let previous = self.charges.get(&id).copied().unwrap_or(0);
        let total = self
            .used()
            .checked_sub(previous)
            .and_then(|n| n.checked_add(charge))
            .ok_or("pins.budget")?;
        if charge == 0
            || (total > self.limit && charge > previous)
            || (!self.charges.contains_key(&id) && self.count() >= 64)
        {
            return Err("pins.budget");
        }
        self.charges.insert(id, charge);
        Ok(())
    }
    pub fn release(&mut self, id: u64) {
        self.charges.remove(&id);
    }
}

/// Two source representations plus two render surfaces, with control/row overhead.
/// This is a reservation estimate, not a claim about measured process RSS.
pub fn charge(source: (u32, u32), window: (u32, u32)) -> Option<u64> {
    if [source.0, source.1, window.0, window.1].contains(&0) {
        return None;
    }
    u64::from(source.0)
        .checked_mul(source.1.into())?
        .checked_mul(8)?
        .checked_add(
            u64::from(window.0)
                .checked_mul(window.1.into())?
                .checked_mul(8)?,
        )?
        .checked_add(32 * 1024)
}

/// Lossless clockwise quarter turns. The destination is allocated by the caller.
/// Keeping the source immutable avoids repeated resampling during pin rotation.
pub fn rotate_rgba(
    source: &[u8],
    destination: &mut [u8],
    width: u32,
    height: u32,
    turns: u8,
) -> Result<(), &'static str> {
    let (w, h) = (width as usize, height as usize);
    let length = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(4))
        .ok_or("pins.image")?;
    if length == 0 || source.len() != length || destination.len() != length {
        return Err("pins.image");
    }
    let turns = turns % 4;
    let out_w = if turns % 2 == 1 { h } else { w };
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match turns {
                1 => (h - 1 - y, x),
                2 => (w - 1 - x, h - 1 - y),
                3 => (y, w - 1 - x),
                _ => (x, y),
            };
            let from = (y * w + x) * 4;
            let to = (dy * out_w + dx) * 4;
            destination[to..to + 4].copy_from_slice(&source[from..from + 4]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_create_or_resize_keeps_previous_reservations_and_close_releases() {
        let mut budget = PinBudget::default();
        budget.set_limit(100);
        budget.reserve(1, 50).unwrap();
        budget.reserve(2, 40).unwrap();
        assert!(budget.reserve(3, 11).is_err());
        assert!(budget.reserve(1, 61).is_err());
        assert_eq!((budget.count(), budget.used()), (2, 90));
        budget.release(1);
        budget.reserve(3, 60).unwrap();
        budget.set_limit(80); // Existing windows survive a lower user-selected limit.
        assert!(budget.reserve(4, 1).is_err());
        budget.reserve(3, 59).unwrap(); // Releasing memory remains possible above the new limit.
        budget.release(2);
        assert_eq!(budget.used(), 59);
        assert!(charge((u32::MAX, u32::MAX), (1, 1)).is_none());
        assert!(charge((0, 1), (1, 1)).is_none());
    }

    #[test]
    fn asymmetric_rgba_rotation_keeps_channels_alpha_and_four_turn_identity() {
        let source: Vec<u8> = (1..=6).flat_map(|n| [n, n + 10, n + 20, n + 30]).collect();
        for (turns, expected) in [
            (0, [1, 2, 3, 4, 5, 6]),
            (1, [4, 1, 5, 2, 6, 3]),
            (2, [6, 5, 4, 3, 2, 1]),
            (3, [3, 6, 2, 5, 1, 4]),
            (4, [1, 2, 3, 4, 5, 6]),
        ] {
            let mut destination = vec![0; source.len()];
            rotate_rgba(&source, &mut destination, 3, 2, turns).unwrap();
            let expected: Vec<u8> = expected
                .into_iter()
                .flat_map(|n| [n, n + 10, n + 20, n + 30])
                .collect();
            assert_eq!(destination, expected);
        }
        assert!(rotate_rgba(&source, &mut [0; 4], 3, 2, 1).is_err());
        assert!(rotate_rgba(&[], &mut [], 0, 0, 1).is_err());
    }
}
