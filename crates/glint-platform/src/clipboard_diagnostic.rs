//! Explicit native-smoke diagnostic; never used by normal capture/clipboard operations.
use std::{marker::PhantomData, rc::Rc};
use windows::{
    Win32::{
        Foundation::HWND,
        System::DataExchange::{CloseClipboard, OpenClipboard},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

/// Holds the real clipboard open on the calling thread to verify another worker's
/// failure/retry path without emptying or writing clipboard contents. Not Send/Sync.
pub struct ClipboardBlocker(HWND, PhantomData<Rc<()>>);
impl ClipboardBlocker {
    pub fn acquire() -> Result<Self, String> {
        // Use a distinct non-null message-only HWND. Two OpenClipboard(NULL)
        // callers do not reliably model contention on this Windows environment.
        let window = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("Glint clipboard diagnostic"),
                Default::default(),
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                None,
                None,
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        if let Err(error) = unsafe { OpenClipboard(window) } {
            let _ = unsafe { DestroyWindow(window) };
            return Err(error.to_string());
        }
        Ok(Self(window, PhantomData))
    }
}
impl Drop for ClipboardBlocker {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
        let _ = unsafe { DestroyWindow(self.0) };
    }
}
