//! Event-driven countdown. No desktop image or capture worker exists until expiry.
use crate::{
    messages::{self, Message},
    settings::SettingsWindow,
};
use glint_core::config::{Config, Theme};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::time::{Duration, Instant};
slint::slint! { export { Motion } from "src/widgets/motion.slint"; export { CaptureCountdown } from "src/capture_delay.slint"; }

pub struct Run {
    pub ui: CaptureCountdown,
    timer: slint::Timer,
    pub action: String,
}
impl Run {
    pub fn start(
        settings: &SettingsWindow,
        config: &Config,
        languages: &[String],
        action: &str,
    ) -> Result<Self, String> {
        let ui = CaptureCountdown::new().map_err(|e| e.to_string())?;
        crate::motion::configure!(&ui, Motion, config.appearance.reduce_motion);
        crate::motion::install(&ui, config.appearance.reduce_motion);
        crate::interface_scale::install_with_theme(
            &ui,
            config.appearance.scale,
            true,
            config.appearance.theme,
            apply_system_theme,
        );
        let english = crate::settings::english(config.general.language, languages);
        ui.set_heading(messages::text(Message::DelayTitle, english).into());
        ui.set_hint(messages::text(Message::DelayHint, english).into());
        ui.set_cancel_label(messages::text(Message::DelayCancel, english).into());
        ui.set_seconds(i32::from(config.capture.delay_seconds));
        ui.set_dark(match config.appearance.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => glint_platform::system_dark_theme().unwrap_or(false),
        });
        let rgb =
            u32::from_str_radix(&config.appearance.accent[1..], 16).map_err(|e| e.to_string())?;
        ui.set_accent(slint::Color::from_rgb_u8(
            (rgb >> 16) as u8,
            (rgb >> 8) as u8,
            rgb as u8,
        ));
        let weak = settings.as_weak();
        ui.on_cancel(move || {
            if let Some(settings) = weak.upgrade() {
                settings.invoke_action("delay-cancel".into());
            }
        });
        let weak = settings.as_weak();
        ui.window().on_close_requested(move || {
            if let Some(settings) = weak.upgrade() {
                settings.invoke_action("delay-cancel".into());
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
        ui.show().map_err(|e| e.to_string())?;
        let weak = ui.as_weak();
        slint::spawn_local(async move {
            if let Some(ui) = weak.upgrade()
                && let Ok(window) = ui.window().winit_window().await
                && ui.window().is_visible()
            {
                use slint::winit_030::winit::platform::windows::WindowExtWindows;
                window.set_skip_taskbar(true);
                // Position using the actual peer's DPI and nearest monitor work area.
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if let Ok(handle) = window.window_handle()
                    && let RawWindowHandle::Win32(handle) = handle.as_raw()
                {
                    let native = unsafe {
                        glint_platform::NativePinWindow::from_raw(
                            handle.hwnd.get() as *mut std::ffi::c_void
                        )
                    };
                    if let Ok((x, y, w, _)) = native.work_area() {
                        let size = ui.window().size();
                        let _ = native.position(
                            x + (w as i32 - size.width as i32).max(0) / 2,
                            y + (40. * ui.window().scale_factor()) as i32,
                        );
                    }
                }
                window.set_window_level(slint::winit_030::winit::window::WindowLevel::AlwaysOnTop);
                window.focus_window();
            }
        })
        .map_err(|e| e.to_string())?;
        let deadline =
            Instant::now() + Duration::from_secs(u64::from(config.capture.delay_seconds));
        let timer = slint::Timer::default();
        let weak = ui.as_weak();
        let owner = settings.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(1),
            move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs_f64()
                    .ceil() as i32;
                ui.set_seconds(remaining);
                if remaining == 0
                    && let Some(settings) = owner.upgrade()
                {
                    settings.invoke_action("delay-ready".into());
                }
            },
        );
        Ok(Self {
            ui,
            timer,
            action: action.into(),
        })
    }
    /// Hide and flush on the HWND thread before allowing desktop acquisition.
    pub fn finish(&self) -> Result<(), String> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        self.timer.stop();
        self.ui
            .window()
            .with_winit_window(|window| {
                let handle = window.window_handle().map_err(|e| e.to_string())?;
                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                    return Err("Not a Win32 countdown".into());
                };
                unsafe {
                    glint_platform::NativePinWindow::from_raw(
                        handle.hwnd.get() as *mut std::ffi::c_void
                    )
                }
                .hide_for_capture()
            })
            .ok_or("Countdown window is unavailable")??;
        self.ui.hide().map_err(|e| e.to_string())
    }
}
pub(crate) fn apply_system_theme(ui: &CaptureCountdown, dark: bool) {
    ui.set_dark(dark);
}
impl Drop for Run {
    fn drop(&mut self) {
        self.timer.stop();
        let _ = self.ui.hide();
    }
}
