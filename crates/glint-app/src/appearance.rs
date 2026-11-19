//! Configuration-to-presentation mapping; system queries stay in the app/platform.
use glint_core::config::Theme;
use glint_ui::theme::ThemeMode;

pub fn dark(theme: Theme, system_dark: Option<bool>) -> bool {
    let mode = match theme {
        Theme::Light => ThemeMode::Light,
        Theme::Dark => ThemeMode::Dark,
        Theme::System => ThemeMode::System,
    };
    mode.is_dark(system_dark)
}

pub fn accent(value: &str, dark: bool) -> slint::Color {
    let rgb = value
        .strip_prefix('#')
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .unwrap_or(if dark {
            glint_ui::DARK_ACCENT
        } else {
            glint_ui::LIGHT_ACCENT
        });
    slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}
