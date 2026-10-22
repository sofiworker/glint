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
