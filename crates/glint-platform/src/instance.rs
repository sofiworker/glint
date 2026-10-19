//! Per-session named mutex plus an event-driven message-only activation window.
use std::{sync::mpsc, thread};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GetLastError, HANDLE, HINSTANCE,
            HWND, LPARAM, SetLastError, WAIT_OBJECT_0, WPARAM,
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{
                CreateEventW, CreateMutexW, GetCurrentThreadId, ResetEvent, SetEvent,
                WaitForSingleObject,
            },
        },
        UI::WindowsAndMessaging::*,
    },
    core::PCWSTR,
};

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub enum InstanceStart {
    Primary(ResidentInstance),
    Activated,
}
pub struct ResidentInstance {
    _mutex: OwnedHandle,
    _ready: OwnedHandle,
    thread_id: u32,
    owner: Option<thread::JoinHandle<()>>,
}
impl ResidentInstance {
    /// namespace is an app-controlled ASCII identifier, not user input or a path.
    pub fn start(
        namespace: &str,
        activate: impl Fn() + Send + 'static,
    ) -> Result<InstanceStart, String> {
        if namespace.is_empty()
            || namespace.len() > 64
            || !namespace
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("Invalid instance namespace".into());
        }
        let wide = |text: String| text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mutex_name = wide(format!("Local\\Glint-{namespace}"));
        let ready_name = wide(format!("Local\\Glint-ready-{namespace}"));
        let class_name = wide(format!("Glint.Activation.{namespace}"));
        unsafe { SetLastError(ERROR_SUCCESS) };
        let mutex = OwnedHandle(
            unsafe { CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr())) }
                .map_err(|e| format!("CreateMutexW: {e}"))?,
        );
        let existing = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let ready = OwnedHandle(
            unsafe { CreateEventW(None, true, false, PCWSTR(ready_name.as_ptr())) }
                .map_err(|e| format!("CreateEventW: {e}"))?,
        );
        if existing {
            if unsafe { WaitForSingleObject(ready.0, 3000) } != WAIT_OBJECT_0 {
                return Err("Existing Glint instance is not ready; try again".into());
            }
            let window = unsafe {
                FindWindowExW(
                    HWND_MESSAGE,
                    HWND::default(),
                    PCWSTR(class_name.as_ptr()),
                    PCWSTR::null(),
                )
            }
            .map_err(|e| format!("Find activation window: {e}"))?;
            let mut process = 0;
            unsafe {
                GetWindowThreadProcessId(window, Some(&mut process));
                if process != 0 {
                    let _ = AllowSetForegroundWindow(process);
                }
            }
            unsafe { PostMessageW(window, ACTIVATE, WPARAM(0), LPARAM(0)) }
                .map_err(|e| format!("Activate existing instance: {e}"))?;
            return Ok(InstanceStart::Activated);
        }
        unsafe { ResetEvent(ready.0) }.map_err(|e| e.to_string())?;
        let ready_raw = ready.0.0 as usize;
        let (started, response) = mpsc::sync_channel(1);
        let owner = thread::Builder::new()
            .name("glint-instance".into())
            .spawn(move || {
                let result = activation_window(&class_name);
                let (window, module) = match result {
                    Ok(result) => result,
                    Err(error) => {
                        let _ = started.send(Err(error));
                        return;
                    }
                };
                let ready = HANDLE(ready_raw as *mut std::ffi::c_void);
                let signaled = unsafe { SetEvent(ready) };
                if let Err(error) = signaled {
                    let _ = started.send(Err(error.to_string()));
                } else {
                    let _ = started.send(Ok(unsafe { GetCurrentThreadId() }));
                    let mut message = MSG::default();
                    loop {
                        let result = unsafe { GetMessageW(&mut message, HWND::default(), 0, 0) }.0;
                        if result <= 0 {
                            break;
                        }
                        if message.hwnd == window && message.message == ACTIVATE {
                            activate();
                        } else {
                            unsafe {
                                let _ = TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                        }
                    }
                }
                unsafe {
                    let _ = ResetEvent(ready);
                    let _ = DestroyWindow(window);
                    let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), module);
                }
            })
            .map_err(|e| e.to_string())?;
        match response.recv().map_err(|e| e.to_string())? {
            Ok(thread_id) => Ok(InstanceStart::Primary(Self {
                _mutex: mutex,
                _ready: ready,
                thread_id,
                owner: Some(owner),
            })),
            Err(error) => {
                let _ = owner.join();
                Err(error)
            }
        }
    }
}
const ACTIVATE: u32 = WM_APP + 50;
fn activation_window(name: &[u16]) -> Result<(HWND, HINSTANCE), String> {
    let module = HINSTANCE(
        unsafe { GetModuleHandleW(None) }
            .map_err(|e| e.to_string())?
            .0,
    );
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: module,
        lpszClassName: PCWSTR(name.as_ptr()),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(format!(
            "RegisterClassW: {}",
            windows::core::Error::from_win32()
        ));
    }
    match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            module,
            None,
        )
    } {
        Ok(window) => Ok((window, module)),
        Err(error) => {
            unsafe {
                let _ = UnregisterClassW(PCWSTR(name.as_ptr()), module);
            }
            Err(format!("Create activation window: {error}"))
        }
    }
}
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    w: WPARAM,
    l: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    unsafe { DefWindowProcW(window, message, w, l) }
}
impl Drop for ResidentInstance {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_activates_primary_and_shutdown_releases_namespace() {
        let name = format!("test-{}", std::process::id());
        let (sent, received) = mpsc::sync_channel(1);
        let InstanceStart::Primary(primary) = ResidentInstance::start(&name, move || {
            let _ = sent.try_send(());
        })
        .unwrap() else {
            panic!("expected primary");
        };
        assert!(matches!(
            ResidentInstance::start(&name, || {}).unwrap(),
            InstanceStart::Activated
        ));
        received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        drop(primary);
        assert!(matches!(
            ResidentInstance::start(&name, || {}).unwrap(),
            InstanceStart::Primary(_)
        ));
    }
}
