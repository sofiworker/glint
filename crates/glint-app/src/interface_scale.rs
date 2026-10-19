//! Per-window presentation scale. Physical capture/pin pixels never use a global override.
use glint_core::config::{Scale, Theme};
use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
type ThemePolicy<T> = (Theme, fn(&T, bool));

pub fn factor(scale: Scale) -> f32 {
    match scale {
        Scale::System => 0.,
        Scale::Percent100 => 1.,
        Scale::Percent125 => 1.25,
        Scale::Percent150 => 1.5,
        Scale::Percent175 => 1.75,
    }
}
pub fn apply(window: &slint::Window, explicit: f32) -> Result<(), String> {
    let native = window.with_winit_window(|w| w.scale_factor() as f32);
    let Some(native) = native else {
        return Ok(());
    };
    let scale = if explicit > 0. { explicit } else { native };
    if (window.scale_factor() - scale).abs() > 0.001 {
        window
            .dispatch_event_with_result(slint::platform::WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
            })
            .map_err(|e| e.to_string())?;
        // Keep the physical desktop/image extent; only the logical control space changes.
        let size = window.size();
        window
            .dispatch_event_with_result(slint::platform::WindowEvent::Resized {
                size: slint::LogicalSize::from_physical(size, scale),
            })
            .map_err(|e| e.to_string())?;
        window.request_redraw();
    }
    Ok(())
}
pub fn event(
    window: &slint::Window,
    event: &winit::event::WindowEvent,
    explicit: f32,
) -> EventResult {
    if explicit > 0. && matches!(event, winit::event::WindowEvent::ScaleFactorChanged { .. }) {
        // An explicit preference survives a monitor-DPI event. System mode propagates it.
        if let Err(error) = apply(window, explicit) {
            eprintln!("Interface scale: {error}");
        }
        EventResult::PreventDefault
    } else {
        EventResult::Propagate
    }
}
pub fn install_factor<T: ComponentHandle + 'static>(ui: &T, explicit: f32) {
    install_inner(ui, explicit, true, None);
}
/// Install one winit event filter for both policies; installing a second filter
/// would replace the first and break explicit DPI scaling on monitor changes.
pub fn install_with_theme<T: ComponentHandle + 'static>(
    ui: &T,
    scale: Scale,
    resize: bool,
    preference: Theme,
    apply: fn(&T, bool),
) {
    install_inner(ui, factor(scale), resize, Some((preference, apply)));
}
pub(crate) fn theme_event<T: ComponentHandle + 'static>(
    ui: &slint::Weak<T>,
    preference: Theme,
    event: &winit::event::WindowEvent,
    apply: fn(&T, bool),
) {
    if preference == Theme::System
        && let winit::event::WindowEvent::ThemeChanged(theme) = event
    {
        let dark = *theme == winit::window::Theme::Dark;
        // Defer Slint mutation until the native event filter has returned.
        let _ = ui.upgrade_in_event_loop(move |ui| apply(&ui, dark));
    }
}
pub fn preview(window: &slint::Window, explicit: f32) -> Result<(), String> {
    let old_scale = window.scale_factor();
    let old_size = window.size();
    apply(window, explicit)?;
    let ratio = window.scale_factor() / old_scale;
    if (ratio - 1.).abs() > 0.001 {
        let mut width = (old_size.width as f32 * ratio).round() as u32;
        let mut height = (old_size.height as f32 * ratio).round() as u32;
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        window.with_winit_window(|w| {
            if let Ok(handle) = w.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
                && let Ok((_, _, wide, high)) = unsafe {
                    glint_platform::NativePinWindow::from_raw(
                        handle.hwnd.get() as *mut std::ffi::c_void
                    )
                }
                .work_area()
            {
                width = width.min(wide.saturating_sub(24));
                height = height.min(high.saturating_sub(48));
            }
        });
        window.set_size(slint::PhysicalSize::new(width, height));
        window.with_winit_window(|w| {
            if let Ok(handle) = w.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                let native = unsafe {
                    glint_platform::NativePinWindow::from_raw(
                        handle.hwnd.get() as *mut std::ffi::c_void
                    )
                };
                if let (Ok(state), Ok((x, y, wide, high))) = (native.state(), native.work_area()) {
                    let _ = native.position(
                        state.x.clamp(x, x + wide.saturating_sub(width) as i32),
                        state
                            .y
                            .clamp(y, y + high.saturating_sub(height + 32) as i32),
                    );
                }
            }
        });
    }
    Ok(())
}
fn install_inner<T: ComponentHandle + 'static>(
    ui: &T,
    explicit: f32,
    resize: bool,
    theme: Option<ThemePolicy<T>>,
) {
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |window, e| {
        if let Some((preference, apply)) = theme {
            theme_event(&weak, preference, e, apply);
        }
        event(window, e, explicit)
    });
    let weak = ui.as_weak();
    let _ = slint::spawn_local(async move {
        if let Some(ui) = weak.upgrade()
            && ui.window().winit_window().await.is_ok()
            && let Err(error) = if resize {
                preview(ui.window(), explicit)
            } else {
                apply(ui.window(), explicit)
            }
        {
            eprintln!("Interface scale: {error}");
        }
    });
}
