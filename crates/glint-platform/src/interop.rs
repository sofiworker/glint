//! Audited Win32 interop helpers. Raw-pointer and conversion patterns that
//! previously repeated across modules live here, each with a documented
//! invariant, so the rest of the platform can stay on safe code.

use std::os::windows::ffi::OsStrExt;
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW},
};

/// Null-terminated UTF-16 for Win32 string parameters.
pub(crate) fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

/// Stores one `T` in a window's `GWLP_USERDATA` slot for its procedure.
///
/// The window procedure owns the [`WindowSlot`] as part of the struct that
/// outlives every message, so `attach` before the first message and keep the
/// slot alive for the whole window lifetime; `get` then borrows the value for
/// the duration of a procedure call.
pub(crate) struct WindowSlot<T>(Box<T>);

impl<T> WindowSlot<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(Box::new(value))
    }

    /// Borrow the stored value for configuration before the first message.
    pub(crate) fn value(&self) -> &T {
        &self.0
    }

    /// Publish the value to the window. Call once, before the window receives
    /// its first message; the slot must not move afterwards.
    pub(crate) fn attach(&self, window: HWND) {
        // The pointer is only written here and only read through `get`; the
        // boxed value is owned by this slot, which outlives the procedure.
        unsafe {
            SetWindowLongPtrW(
                window,
                GWLP_USERDATA,
                (&*self.0 as *const T) as *const std::ffi::c_void as isize,
            );
        }
    }

    /// Borrow the attached value inside a window procedure.
    ///
    /// # Safety contract
    /// The window must have been `attach`ed with the same `T`, and the value
    /// must still be alive (the slot outlives the procedure call). This holds
    /// for every procedure in this crate, whose slots live beside the handle.
    pub(crate) fn get(window: HWND) -> Option<&'static T> {
        // Reading the slot the owning struct attached for this exact procedure.
        unsafe {
            let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const T;
            pointer.as_ref()
        }
    }
}
