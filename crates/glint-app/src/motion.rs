//! Per-component control policy; source pixels and input geometry never animate.
use slint::Model;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

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
        policy.set_reduced($reduced);
    }};
}
pub(crate) use configure;
