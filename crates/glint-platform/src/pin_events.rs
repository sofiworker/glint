//! UI-thread subclass guard. Notifications enqueue application work, never mutate
//! Slint state while Windows is dispatching its window procedure.
use windows::Win32::{
    Foundation::*,
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinGeometryEvent {
    MoveFinished,
    DisplayChanged,
}
struct Handler(std::rc::Rc<dyn Fn(PinGeometryEvent)>);
pub struct PinWindowEvents {
    window: HWND,
    handler: Box<Handler>,
}
impl PinWindowEvents {
    /// # Safety
    /// The live HWND belongs to this thread. Drop this guard on the same thread;
    /// its callback should enqueue application work to avoid window-procedure reentrancy.
    pub unsafe fn install(
        window: HWND,
        callback: impl Fn(PinGeometryEvent) + 'static,
    ) -> Result<Self, String> {
        let handler = Box::new(Handler(std::rc::Rc::new(callback)));
        let pointer = (&*handler as *const Handler) as usize;
        if !unsafe { SetWindowSubclass(window, Some(procedure), pointer, pointer) }.as_bool() {
            return Err("Cannot install pin geometry notifications".into());
        }
        Ok(Self { window, handler })
    }
}
impl Drop for PinWindowEvents {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveWindowSubclass(
                self.window,
                Some(procedure),
                (&*self.handler as *const Handler) as usize,
            );
        }
    }
}
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // WS_EX_NOACTIVATE alone does not cover every caption/client interaction.
    // Deliver the click while explicitly retaining the current foreground.
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if message == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(window, Some(procedure), id);
        }
    }
    let event = match message {
        WM_EXITSIZEMOVE => Some(PinGeometryEvent::MoveFinished),
        WM_DISPLAYCHANGE | WM_DPICHANGED => Some(PinGeometryEvent::DisplayChanged),
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => {
            Some(PinGeometryEvent::DisplayChanged)
        }
        _ => None,
    };
    if let Some(event) = event {
        // Never unwind across the foreign callback boundary.
        // The guard may be dropped by a reentrant callback. Keep its callable
        // alive independently and never read subclass data after invoking it.
        let callback = unsafe { (*(data as *const Handler)).0.clone() };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(event)));
    }
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    #[test]
    fn subclass_can_be_removed_from_its_callback_without_retaining_it() {
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let window = Window(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    windows::core::w!("STATIC"),
                    None,
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        );
        let holder: Rc<RefCell<Option<PinWindowEvents>>> = Rc::new(RefCell::new(None));
        let weak = Rc::downgrade(&holder);
        let calls = Rc::new(Cell::new(0));
        let called = calls.clone();
        let guard = unsafe {
            PinWindowEvents::install(window.0, move |_| {
                called.set(called.get() + 1);
                weak.upgrade().unwrap().borrow_mut().take();
            })
        }
        .unwrap();
        *holder.borrow_mut() = Some(guard);
        unsafe {
            SendMessageW(window.0, WM_EXITSIZEMOVE, WPARAM(0), LPARAM(0));
        }
        assert_eq!(calls.get(), 1);
        assert!(holder.borrow().is_none());
        unsafe {
            SendMessageW(window.0, WM_EXITSIZEMOVE, WPARAM(0), LPARAM(0));
        }
        assert_eq!(calls.get(), 1);
    }
}
