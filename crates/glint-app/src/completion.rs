//! One nonactivating, expiring success popup; one bounded notification submission.
use glint_core::config::Feedback;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
slint::slint! { export { CompletionToast } from "src/completion.slint"; }
#[derive(Default)]
struct State {
    ui: Option<CompletionToast>,
    timer: slint::Timer,
    ticket: u64,
    kind: i32,
    submitted: bool,
    ready: bool,
}
thread_local! { static STATE:RefCell<State> = RefCell::default(); }
static NOTIFY_BUSY: AtomicBool = AtomicBool::new(false);
pub fn dismiss() {
    STATE.with_borrow_mut(|s| {
        s.ticket = s.ticket.wrapping_add(1);
        s.timer.stop();
        if let Some(ui) = s.ui.take() {
            let _ = ui.hide();
        }
        s.ready = false;
        s.kind = 0;
        s.submitted = false;
    });
}
pub fn mode(id: i32) -> Feedback {
    match id {
        1 => Feedback::None,
        2 => Feedback::Notification,
        _ => Feedback::Toast,
    }
}
pub fn show(
    mode: Feedback,
    kind: i32,
    english: bool,
    dark: bool,
    accent: slint::Color,
    scale: f32,
    reduced: bool,
) {
    dismiss();
    if mode == Feedback::None || kind == 0 {
        return;
    }
    STATE.with_borrow_mut(|s| s.kind = kind);
    if mode == Feedback::Toast {
        brief(kind, english, dark, accent, false, scale, reduced);
        return;
    }
    let ticket = STATE.with_borrow(|s| s.ticket);
    if NOTIFY_BUSY.swap(true, Ordering::AcqRel) {
        brief(kind, english, dark, accent, true, scale, reduced);
        return;
    }
    let started = std::thread::Builder::new()
        .name("glint-notification".into())
        .spawn(move || {
            let result = glint_platform::notification::submit_completion(crate::messages::text(
                crate::messages::Message::CompletionNotification,
                english,
            ));
            NOTIFY_BUSY.store(false, Ordering::Release);
            let _ = slint::invoke_from_event_loop(move || {
                if !STATE.with_borrow(|s| s.ticket == ticket) {
                    return;
                }
                if result.is_ok() {
                    STATE.with_borrow_mut(|s| s.submitted = true);
                } else {
                    brief(kind, english, dark, accent, true, scale, reduced);
                }
            });
        });
    if started.is_err() {
        NOTIFY_BUSY.store(false, Ordering::Release);
        brief(kind, english, dark, accent, true, scale, reduced);
    }
}
fn brief(
    kind: i32,
    en: bool,
    dark: bool,
    accent: slint::Color,
    fallback: bool,
    scale: f32,
    reduced: bool,
) {
    let Ok(ui) = CompletionToast::new() else {
        return;
    };
    crate::interface_scale::install_factor(&ui, scale);
    crate::motion::install(&ui, reduced);
    ui.set_dark(dark);
    ui.set_accent(accent);
    ui.set_fallback(fallback);
    ui.set_message(
        crate::messages::text(
            match kind {
                2 => crate::messages::Message::CompletionSaved,
                3 => crate::messages::Message::CompletionPinned,
                4 => crate::messages::Message::CompletionColor,
                5 => crate::messages::Message::CompletionApplied,
                _ => crate::messages::Message::CompletionCopied,
            },
            en,
        )
        .into(),
    );
    ui.set_hint(crate::messages::text(crate::messages::Message::CompletionFallback, en).into());
    if ui.show().is_err() {
        return;
    }
    let ticket = STATE.with_borrow_mut(|s| {
        s.ui = Some(ui.clone_strong());
        s.timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_secs(3),
            dismiss,
        );
        s.ticket
    });
    let weak = ui.as_weak();
    let _ = slint::spawn_local(async move {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        let Some(ui) = weak.upgrade() else {
            return;
        };
        // Slint may create the native peer after the current callback returns.
        let Ok(window) = ui.window().winit_window().await else {
            return;
        };
        if !STATE.with_borrow(|s| s.ticket == ticket) {
            return;
        }
        window.set_skip_taskbar(true);
        let configured = (|| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Completion feedback requires Win32".into());
            };
            let native = unsafe {
                glint_platform::NativePinWindow::from_raw(handle.hwnd.get() as *mut std::ffi::c_void)
            };
            native.configure(true, true)?;
            if let Ok((x, y, w, h)) = native.work_area() {
                let size = ui.window().size();
                let margin = (20. * ui.window().scale_factor()) as i32;
                native.position(
                    x + (w as i32 - size.width as i32 - margin).max(0),
                    y + (h as i32 - size.height as i32 - margin).max(0),
                )?;
            }
            Ok::<(), String>(())
        })();
        if configured.is_err() {
            dismiss();
        } else {
            STATE.with_borrow_mut(|s| s.ready = true);
        }
    });
}

pub fn inspect() -> (bool, i32, bool) {
    STATE.with_borrow(|s| {
        (
            s.ready && s.ui.as_ref().is_some_and(|u| u.window().is_visible()),
            s.kind,
            s.submitted,
        )
    })
}
pub fn popup() -> Option<CompletionToast> {
    STATE.with_borrow(|s| {
        s.ready
            .then(|| s.ui.as_ref().map(ComponentHandle::clone_strong))
            .flatten()
    })
}
pub fn pending() -> bool {
    NOTIFY_BUSY.load(Ordering::Acquire)
}
