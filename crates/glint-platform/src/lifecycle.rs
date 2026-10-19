//! UI-thread lifecycle subscription. Callbacks must enqueue work, never reenter Slint.
use std::rc::Rc;
use windows::Win32::{
    Foundation::*,
    System::RemoteDesktop::*,
    UI::{Shell::*, WindowsAndMessaging::*},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    Stopped,
    Locked,
    Unlocked,
    Disconnected,
    Connected,
    Sleeping,
    Resumed,
}

/// Hidden top-level window with its own blocking message loop, available even
/// when no settings/Slint window has ever been opened. Never a message-only HWND:
/// power broadcasts are delivered to top-level windows.
pub struct LifecycleService {
    thread_id: u32,
    owner: Option<std::thread::JoinHandle<()>>,
    #[cfg(test)]
    window: usize,
}
impl LifecycleService {
    pub fn is_running(&self) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|owner| !owner.is_finished())
    }
    pub fn start(callback: impl Fn(LifecycleEvent) + Send + 'static) -> Result<Self, String> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let owner = std::thread::Builder::new()
            .name("glint-lifecycle".into())
            .spawn(move || {
                let window = match unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        windows::core::w!("STATIC"),
                        None,
                        WS_POPUP,
                        0,
                        0,
                        0,
                        0,
                        None,
                        None,
                        None,
                        None,
                    )
                } {
                    Ok(window) => window,
                    Err(error) => {
                        let _ = send.send(Err(error.to_string()));
                        return;
                    }
                };
                let callback = Rc::new(callback);
                let handler = callback.clone();
                let guard = match unsafe {
                    LifecycleEvents::install_raw(window.0, move |event| handler(event))
                } {
                    Ok(guard) => guard,
                    Err(error) => {
                        let _ = send.send(Err(error));
                        unsafe {
                            let _ = DestroyWindow(window);
                        }
                        return;
                    }
                };
                let id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
                if send.send(Ok((id, window.0 as usize))).is_ok() {
                    let mut message = MSG::default();
                    loop {
                        let result = unsafe { GetMessageW(&mut message, HWND::default(), 0, 0) }.0;
                        if result == -1 {
                            callback(LifecycleEvent::Stopped);
                            break;
                        }
                        if result == 0 {
                            break;
                        }
                        unsafe {
                            let _ = TranslateMessage(&message);
                            DispatchMessageW(&message);
                        }
                    }
                }
                drop(guard);
                unsafe {
                    let _ = DestroyWindow(window);
                }
            })
            .map_err(|e| e.to_string())?;
        match receive.recv().map_err(|e| e.to_string())? {
            Ok((thread_id, _window)) => Ok(Self {
                thread_id,
                owner: Some(owner),
                #[cfg(test)]
                window: _window,
            }),
            Err(error) => {
                let _ = owner.join();
                Err(error)
            }
        }
    }
}
impl Drop for LifecycleService {
    fn drop(&mut self) {
        let posted =
            unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.is_ok();
        if let Some(owner) = self.owner.take()
            && (posted || owner.is_finished())
        {
            let _ = owner.join();
        }
    }
}

struct Handler(Rc<dyn Fn(LifecycleEvent)>);
pub struct LifecycleEvents {
    window: HWND,
    handler: Box<Handler>,
}
impl LifecycleEvents {
    /// # Safety
    /// HWND must be a live top-level window owned by this thread. Drop on this thread
    /// before destroying the window. Callback must enqueue work to avoid reentrancy.
    pub unsafe fn install_raw(
        raw: *mut std::ffi::c_void,
        callback: impl Fn(LifecycleEvent) + 'static,
    ) -> Result<Self, String> {
        let window = HWND(raw);
        let handler = Box::new(Handler(Rc::new(callback)));
        let pointer = (&*handler as *const Handler) as usize;
        unsafe { WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) }
            .map_err(|e| format!("Session notification registration: {e}"))?;
        if !unsafe { SetWindowSubclass(window, Some(procedure), pointer, pointer) }.as_bool() {
            let _ = unsafe { WTSUnRegisterSessionNotification(window) };
            return Err("Cannot install lifecycle window notifications".into());
        }
        Ok(Self { window, handler })
    }
}
impl Drop for LifecycleEvents {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveWindowSubclass(
                self.window,
                Some(procedure),
                (&*self.handler as *const Handler) as usize,
            );
            let _ = WTSUnRegisterSessionNotification(self.window);
        }
    }
}
fn decode(message: u32, value: usize) -> Option<LifecycleEvent> {
    use LifecycleEvent::*;
    match (message, value as u32) {
        (WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK) => Some(Locked),
        (WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK) => Some(Unlocked),
        (WM_WTSSESSION_CHANGE, WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT) => {
            Some(Disconnected)
        }
        (WM_WTSSESSION_CHANGE, WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT) => Some(Connected),
        (WM_POWERBROADCAST, PBT_APMSUSPEND) => Some(Sleeping),
        (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) => Some(Resumed),
        _ => None,
    }
}
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(window, Some(procedure), id);
        }
    }
    if let Some(event) = decode(message, w.0) {
        let callback = unsafe { (*(data as *const Handler)).0.clone() };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(event)));
    }
    unsafe { DefSubclassProc(window, message, w, l) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_receives_without_any_application_window_and_releases_its_window() {
        let (send, receive) = std::sync::mpsc::channel();
        let service = LifecycleService::start(move |event| {
            let _ = send.send(event);
        })
        .unwrap();
        let window = HWND(service.window as *mut _);
        assert!(!unsafe { IsWindowVisible(window) }.as_bool());
        unsafe {
            SendMessageW(
                window,
                WM_WTSSESSION_CHANGE,
                WPARAM(WTS_REMOTE_DISCONNECT as usize),
                LPARAM(0),
            );
        }
        assert_eq!(
            receive
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            LifecycleEvent::Disconnected
        );
        drop(service);
        assert!(!unsafe { IsWindow(window) }.as_bool());
    }
    #[test]
    fn registered_window_delivers_and_guard_removes_notifications() {
        use std::cell::RefCell;
        let window = unsafe {
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
        .unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let received = events.clone();
        let guard = unsafe {
            LifecycleEvents::install_raw(window.0, move |e| received.borrow_mut().push(e))
        }
        .unwrap();
        // Native message delivery exercises subclass mapping, not actual lock/sleep.
        for (message, value) in [
            (WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK),
            (WM_POWERBROADCAST, PBT_APMSUSPEND),
            (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC),
            (WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK),
        ] {
            unsafe {
                SendMessageW(window, message, WPARAM(value as usize), LPARAM(0));
            }
        }
        assert_eq!(
            *events.borrow(),
            vec![
                LifecycleEvent::Locked,
                LifecycleEvent::Sleeping,
                LifecycleEvent::Resumed,
                LifecycleEvent::Unlocked
            ]
        );
        drop(guard);
        unsafe {
            SendMessageW(
                window,
                WM_WTSSESSION_CHANGE,
                WPARAM(WTS_SESSION_LOCK as usize),
                LPARAM(0),
            );
        }
        assert_eq!(events.borrow().len(), 4);
        unsafe { DestroyWindow(window) }.unwrap();
    }
}
