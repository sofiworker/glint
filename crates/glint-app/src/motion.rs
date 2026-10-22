//! Per-component control policy; source pixels and input geometry never animate.
use slint::Model;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};

thread_local! {
    static CREATING_CAPTURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CREATING_SETTINGS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Settings have an opaque background; only desktop pins need transparent composition.
pub fn create_settings<T>(create: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CREATING_SETTINGS.with(|settings| settings.set(self.0));
        }
    }
    let _restore = Restore(CREATING_SETTINGS.with(|settings| settings.replace(true)));
    create()
}

/// Slint calls its attributes hook synchronously when constructing the adapter,
/// before the native peer exists. Scope the policy to this component only.
pub fn create_capture<T>(create: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CREATING_CAPTURE.with(|capture| capture.set(self.0));
        }
    }
    let _restore = Restore(CREATING_CAPTURE.with(|capture| capture.replace(true)));
    create()
}

pub fn window_attributes(
    attributes: winit::window::WindowAttributes,
) -> winit::window::WindowAttributes {
    use winit::platform::windows::WindowAttributesExtWindows;
    let attributes = attributes.with_window_icon(Some(crate::brand::window_icon()));
    if CREATING_CAPTURE.with(|capture| capture.get()) {
        // The frozen desktop already fills the window. Desktop-pin transparency
        // enables DWM blur-behind at HWND creation and is unnecessary here.
        attributes
            .with_active(false)
            .with_transparent(false)
            .with_decorations(false)
            .with_fullscreen(None)
            .with_maximized(false)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
            .with_skip_taskbar(true)
            .with_undecorated_shadow(false)
    } else {
        attributes
            .with_active(false)
            .with_transparent(!CREATING_SETTINGS.with(|settings| settings.get()))
    }
}

pub fn preview(window: &slint::Window, reduced: bool) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window
        .with_winit_window(|w| {
            if let Ok(handle) = w.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                unsafe {
                    glint_platform::NativePinWindow::from_raw(
                        handle.hwnd.get() as *mut std::ffi::c_void
                    )
                }
                .set_transitions_disabled(reduced)
            } else {
                Err("Window has no Win32 handle".into())
            }
        })
        .unwrap_or(Ok(()))
}

pub fn install<T: ComponentHandle + 'static>(ui: &T, reduced: bool) {
    if let Err(error) = preview(ui.window(), reduced) {
        eprintln!("Motion policy: {error}");
    }
    if ui.window().with_winit_window(|_| ()).is_some() {
        return;
    }
    let weak = ui.as_weak();
    if let Err(error) = slint::spawn_local(async move {
        if let Some(ui) = weak.upgrade()
            && ui.window().winit_window().await.is_ok()
            && let Err(error) = preview(ui.window(), reduced)
        {
            eprintln!("Motion policy: {error}");
        }
    }) {
        eprintln!("Motion policy: {error}");
    }
}

pub fn find_index(model: slint::ModelRc<slint::SharedString>, value: slint::SharedString) -> i32 {
    model
        .iter()
        .position(|candidate| candidate == value)
        .and_then(|index| i32::try_from(index).ok())
        .unwrap_or(-1)
}

macro_rules! configure {
    ($ui:expr, $global:ty, $reduced:expr) => {{
        use slint::ComponentHandle as _;
        let policy = $ui.global::<$global>();
        policy.on_find_index(crate::motion::find_index);
        policy.set_ui_font_family(crate::ui_font::family().into());
        policy.set_reduced($reduced);
    }};
}
pub(crate) use configure;
