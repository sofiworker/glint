//! Pure theme selection. The application supplies system state and local time.
//! No timers, system calls or persistent settings belong to this component kit.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

impl ThemeMode {
    pub fn is_dark(self, system_dark: Option<bool>) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => system_dark.unwrap_or(false),
        }
    }
}

/// A daily pair of local wall-clock boundaries, prepared for a future scheduler.
/// The caller resolves time zones, clock changes and the next wake-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeSchedule {
    light_from: u16,
    dark_from: u16,
}

impl ThemeSchedule {
    /// Minutes after midnight; equal or out-of-day boundaries are invalid.
    pub fn new(light_from: u16, dark_from: u16) -> Option<Self> {
        (light_from < 1440 && dark_from < 1440 && light_from != dark_from).then_some(Self {
            light_from,
            dark_from,
        })
    }

    /// Returns `None` for an invalid local minute. Boundaries are inclusive.
    pub fn is_dark(self, minute: u16) -> Option<bool> {
        if minute >= 1440 {
            return None;
        }
        let light = if self.light_from < self.dark_from {
            minute >= self.light_from && minute < self.dark_from
        } else {
            minute >= self.light_from || minute < self.dark_from
        };
        Some(!light)
    }

    /// Delay in wall-clock minutes to the next boundary, always positive.
    pub fn minutes_until_change(self, minute: u16) -> Option<u16> {
        if minute >= 1440 {
            return None;
        }
        Some(
            [self.light_from, self.dark_from]
                .map(|boundary| {
                    let delay = (boundary + 1440 - minute) % 1440;
                    if delay == 0 { 1440 } else { delay }
                })
                .into_iter()
                .min()
                .expect("two boundaries"),
        )
    }
}
