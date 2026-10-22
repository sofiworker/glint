//! OS integrations. Desktop coordinates are physical pixels.
#[cfg(all(windows, feature = "windows-bindings"))]
mod clipboard_diagnostic;
pub mod config_file;
pub mod diagnostic_log;
#[cfg(all(windows, feature = "windows-bindings"))]
mod fonts;
#[cfg(all(windows, feature = "windows-bindings", feature = "input-probe"))]
pub mod input_probe;
#[cfg(all(windows, feature = "windows-bindings"))]
pub mod notification;
pub mod pin_session_file;
#[cfg(all(windows, feature = "windows-bindings"))]
mod text_raster;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use clipboard_diagnostic::ClipboardBlocker;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use fonts::{installed_font_families, system_ui_font_family};
#[cfg(all(windows, feature = "windows-bindings"))]
mod folder_dialog;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use folder_dialog::{FolderPickerControl, choose_output_directory};
#[cfg(all(windows, feature = "windows-bindings"))]
pub use text_raster::{TextMask, rasterize_text};
#[cfg(feature = "clipboard")]
pub fn copy_text(text: String) -> Result<(), String> {
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_text(text)
        .map_err(|e| e.to_string())
}
#[cfg(feature = "clipboard")]
pub fn read_clipboard_text() -> Result<String, String> {
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .get_text()
        .map_err(|e| e.to_string())
}
pub struct DesktopFrame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[cfg(feature = "clipboard")]
pub fn read_clipboard_image() -> Result<DesktopFrame, String> {
    let image = arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .get_image()
        .map_err(|e| e.to_string())?;
    let width = u32::try_from(image.width).map_err(|_| "Clipboard image too wide")?;
    let height = u32::try_from(image.height).map_err(|_| "Clipboard image too high")?;
    let expected = image
        .width
        .checked_mul(image.height)
        .and_then(|n| n.checked_mul(4))
        .ok_or("Clipboard image too large")?;
    if width == 0 || height == 0 || expected > 128 * 1024 * 1024 || expected != image.bytes.len() {
        return Err("Invalid or oversized clipboard image".into());
    }
    Ok(DesktopFrame {
        x: 0,
        y: 0,
        width,
        height,
        rgba: image.bytes.into_owned(),
    })
}

#[cfg(feature = "clipboard")]
pub fn copy_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Result<(), String> {
    if (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        != Some(pixels.len())
    {
        return Err("Invalid clipboard RGBA buffer".into());
    }
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(pixels),
        })
        .map_err(|e| e.to_string())
}

#[cfg(all(windows, feature = "windows-bindings"))]
mod capture_cursor;
#[cfg(all(windows, feature = "windows-bindings"))]
mod win;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use win::{capture_after_composition, capture_desktop, capture_region_guarded};

#[cfg(all(windows, feature = "windows-bindings"))]
mod exclusions;
#[cfg(all(windows, feature = "windows-bindings"))]
mod window_detection;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use exclusions::{
    ExclusionError, GuardedCaptureError, capture_snapshot_guarded, check_visible_exclusions,
};
#[cfg(all(windows, feature = "windows-bindings"))]
pub use window_detection::{
    DesktopSnapshot, WindowCandidate, capture_snapshot, capture_snapshot_with_cursor,
    visible_windows,
};

#[cfg(all(windows, feature = "windows-bindings"))]
mod diagnostics;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use diagnostics::{
    DesktopEnvironment, DisplayInfo, ProcessResources, ScreenBounds, desktop_environment,
    gui_resource_counts, last_session_input_tick, process_resources,
};

#[cfg(all(windows, feature = "windows-bindings"))]
mod preferences;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use preferences::{
    configuration_directory, local_output_timestamp, pictures_directory, preferred_ui_languages,
    show_startup_error, system_dark_theme,
};

#[cfg(all(windows, feature = "windows-bindings"))]
mod instance;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use instance::{InstanceStart, ResidentInstance};
#[cfg(all(windows, feature = "windows-bindings"))]
mod lifecycle;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use lifecycle::{LifecycleEvent, LifecycleEvents, LifecycleService};

#[cfg(all(windows, feature = "windows-bindings"))]
mod startup;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use startup::{StartupRegistration, StartupSnapshot};

#[cfg(all(windows, feature = "windows-bindings"))]
mod save_dialog;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use save_dialog::choose_image_path;
#[cfg(all(windows, feature = "windows-bindings"))]
mod image_destination;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use image_destination::{
    ImageDestination, ImageReplacement, ReplacementPrompt, choose_image_destination,
    publish_new_image,
};
#[cfg(all(windows, feature = "windows-bindings"))]
mod config_dialog;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use config_dialog::{choose_config_path, choose_local_image_path};

#[cfg(all(windows, feature = "windows-bindings"))]
mod hotkeys;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use hotkeys::{HotkeyBinding, HotkeyEvent, HotkeyService};

#[cfg(all(windows, feature = "windows-bindings"))]
mod pin_events;
#[cfg(all(windows, feature = "windows-bindings"))]
mod pin_window;
#[cfg(all(windows, feature = "windows-bindings"))]
pub use pin_events::{PinGeometryEvent, PinWindowEvents};
#[cfg(all(windows, feature = "windows-bindings"))]
pub use pin_window::{
    FocusTarget, NativePinWindow, PinDrag, PinWindowState, cursor_position, foreground_window_id,
    remember_foreground,
};

#[cfg(not(all(windows, feature = "windows-bindings")))]
pub fn capture_desktop() -> Result<DesktopFrame, String> {
    Err("Desktop capture is currently implemented only on Windows".into())
}
#[cfg(all(windows, feature = "recording"))]
pub mod recording;
