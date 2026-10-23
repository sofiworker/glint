//! Borrowed native pin-window operations. Keep HWND lifetime and Slint ownership in the app.
use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};
use windows::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

/// UI-thread pointer-capture guard for dragging without native caption activation.
/// It must not outlive the window borrowed by NativePinWindow.
pub struct PinDrag {
    window: HWND,
    origin: (i32, i32),
    pointer: POINT,
    _ui_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl PinDrag {
    pub fn move_to_cursor(&self) -> Result<(), String> {
        if !unsafe { IsWindow(self.window) }.as_bool() || unsafe { GetCapture() } != self.window {
            return Err("Pin drag pointer capture was lost".into());
        }
        let mut current = POINT::default();
        unsafe { GetCursorPos(&mut current) }.map_err(|e| e.to_string())?;
        let x = self
            .origin
            .0
            .saturating_add(current.x.saturating_sub(self.pointer.x));
        let y = self
            .origin
            .1
            .saturating_add(current.y.saturating_sub(self.pointer.y));
        unsafe {
            SetWindowPos(
                self.window,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|e| e.to_string())
    }
}
impl Drop for PinDrag {
    fn drop(&mut self) {
        if unsafe { GetCapture() } == self.window {
            let _ = unsafe { ReleaseCapture() };
        }
    }
}

#[derive(Debug)]
pub struct PinWindowState {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub topmost: bool,
    pub click_through: bool,
    pub no_activate: bool,
    pub visible: bool,
}

pub struct NativePinWindow(HWND);

#[cfg(all(test, feature = "input-probe"))]
mod focus_tests {
    use super::*;
    use crate::input_probe::{InputProbe, Pointer};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::core::w;

    struct Fixtures {
        previous: FocusTarget,
        windows: Vec<HWND>,
        instance: HINSTANCE,
    }
    impl Drop for Fixtures {
        fn drop(&mut self) {
            let _ = self.previous.restore();
            for window in &self.windows {
                let _ = unsafe { DestroyWindow(*window) };
            }
            let _ = unsafe {
                UnregisterClassW(w!("Glint.FocusRestorationVerification"), self.instance)
            };
        }
    }
    fn pump() {
        let mut message = MSG::default();
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    #[test]
    #[ignore = "Displays two owned windows and uses guarded real mouse input; requires a visible external foreground window and idle desktop"]
    fn restoration_respects_external_focus_and_hidden_target() {
        let previous = remember_foreground().expect("An external foreground window is required");
        let original = foreground_window_id();
        let cursor = cursor_position().unwrap();
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }.unwrap().0);
        unsafe extern "system" fn procedure(
            window: HWND,
            message: u32,
            w: WPARAM,
            l: LPARAM,
        ) -> LRESULT {
            unsafe { DefWindowProcW(window, message, w, l) }
        }
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: w!("Glint.FocusRestorationVerification"),
            ..Default::default()
        };
        assert_ne!(unsafe { RegisterClassW(&class) }, 0);
        let mut fixtures = Fixtures {
            previous,
            windows: vec![],
            instance,
        };
        for x in [80, 320] {
            let window = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    class.lpszClassName,
                    w!("Glint focus restoration verification"),
                    WS_POPUP,
                    x,
                    80,
                    200,
                    140,
                    None,
                    None,
                    instance,
                    None,
                )
            }
            .unwrap();
            fixtures.windows.push(window);
            let _ = unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
        }
        pump();
        assert_eq!(
            foreground_window_id(),
            original,
            "Fixture creation activated a window"
        );
        let target = FocusTarget {
            window: fixtures.windows[0],
            process: std::process::id(),
        };
        assert!(
            !target.restore(),
            "An external foreground must not be replaced"
        );
        assert_eq!(foreground_window_id(), original);
        let mut input = InputProbe::new().unwrap();
        input
            .pointer(fixtures.windows[1].0 as usize, 30, 30, Pointer::Click, true)
            .unwrap();
        for _ in 0..100 {
            pump();
            if foreground_window_id() == fixtures.windows[1].0 as usize {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(foreground_window_id(), fixtures.windows[1].0 as usize);
        let _ = unsafe { ShowWindow(target.window, SW_HIDE) };
        assert!(!target.restore(), "Hidden target must not be revived");
        assert_eq!(foreground_window_id(), fixtures.windows[1].0 as usize);
        let _ = unsafe { ShowWindow(target.window, SW_SHOWNOACTIVATE) };
        assert!(target.restore(), "Own foreground should permit restoration");
        assert_eq!(foreground_window_id(), target.window.0 as usize);
        assert!(target.restore(), "An already restored target is successful");
        drop(input); // Restore pointer while an owned window still holds foreground.
        assert_eq!(
            cursor_position().unwrap(),
            cursor,
            "Probe must restore the pointer"
        );
        assert!(
            fixtures.previous.restore(),
            "Restore the originally active external window"
        );
        assert_eq!(foreground_window_id(), original);
        assert!(
            !target.restore(),
            "Do not steal it back after external focus returns"
        );
        assert_eq!(foreground_window_id(), original);
    }
}

/// Opaque identity for focus diagnostics; no window text or application content is collected.
pub fn foreground_window_id() -> usize {
    unsafe { GetForegroundWindow().0 as usize }
}
pub fn cursor_position() -> Result<(i32, i32), String> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }.map_err(|e| e.to_string())?;
    Ok((point.x, point.y))
}

pub struct FocusTarget {
    window: HWND,
    process: u32,
}
pub fn remember_foreground() -> Option<FocusTarget> {
    let window = unsafe { GetForegroundWindow() };
    let mut process = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut process));
    }
    if window.is_invalid() || process == std::process::id() {
        None
    } else {
        Some(FocusTarget { window, process })
    }
}
impl FocusTarget {
    /// Best effort only: respect an application the user selected while capture
    /// or output was in flight. Never revive a hidden/minimized previous target.
    pub fn restore(&self) -> bool {
        let mut process = 0;
        unsafe {
            GetWindowThreadProcessId(self.window, Some(&mut process));
            if !IsWindow(self.window).as_bool()
                || process != self.process
                || !IsWindowVisible(self.window).as_bool()
                || IsIconic(self.window).as_bool()
            {
                return false;
            }
            let foreground = GetForegroundWindow();
            if foreground == self.window {
                return true;
            }
            let mut foreground_process = 0;
            GetWindowThreadProcessId(foreground, Some(&mut foreground_process));
            if foreground_process != std::process::id() {
                return false;
            }
            SetForegroundWindow(self.window).as_bool()
        }
    }
}

impl NativePinWindow {
    pub fn geometry_events(
        &self,
        callback: impl Fn(crate::PinGeometryEvent) + 'static,
    ) -> Result<crate::PinWindowEvents, String> {
        self.state()?;
        unsafe { crate::PinWindowEvents::install(self.0, callback) }
    }
    pub fn snap_targets(&self) -> Result<Vec<crate::ScreenBounds>, String> {
        Ok(crate::visible_windows()?
            .into_iter()
            .filter(|w| w.id != self.0.0 as usize)
            .map(|w| w.bounds)
            .collect())
    }
    /// Move without showing, activating, resizing or changing the z-order.
    pub fn position(&self, x: i32, y: i32) -> Result<(), String> {
        self.state()?;
        unsafe {
            SetWindowPos(
                self.0,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|e| e.to_string())
    }
    pub fn begin_drag(&self) -> Result<PinDrag, String> {
        let state = self.state()?;
        if !state.visible || state.click_through {
            return Err("Pin is not available for dragging".into());
        }
        let capture = unsafe { GetCapture() };
        if !capture.is_invalid() && capture != self.0 {
            return Err("Another window owns pointer capture".into());
        }
        let mut pointer = POINT::default();
        unsafe { GetCursorPos(&mut pointer) }.map_err(|e| e.to_string())?;
        unsafe { SetCapture(self.0) };
        if unsafe { GetCapture() } != self.0 {
            return Err("Cannot capture pin drag pointer".into());
        }
        Ok(PinDrag {
            window: self.0,
            origin: (state.x, state.y),
            pointer,
            _ui_thread: Default::default(),
        })
    }
    /// Diagnostic delivery through the installed Win32 subclass; does not alter
    /// the actual display topology or synthesize physical input.
    pub fn diagnostic_geometry_event(&self, display_change: bool) -> Result<(), String> {
        self.state()?;
        unsafe {
            SendMessageW(
                self.0,
                if display_change {
                    WM_DISPLAYCHANGE
                } else {
                    WM_EXITSIZEMOVE
                },
                WPARAM(0),
                LPARAM(0),
            );
        }
        Ok(())
    }
    pub fn is_foreground(&self) -> bool {
        unsafe { GetForegroundWindow() == self.0 }
    }
    /// Prepare a still-hidden capture overlay before its first visible frame.
    /// It accepts keyboard focus, but never adds a taskbar button or DWM transition.
    pub fn prepare_capture_overlay(&self) -> Result<(), String> {
        self.state()?;
        self.disable_transitions()?;
        unsafe {
            let old = GetWindowLongPtrW(self.0, GWL_EXSTYLE);
            let style = (old as u32 | WS_EX_TOOLWINDOW.0)
                & !(WS_EX_APPWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TRANSPARENT.0);
            SetLastError(WIN32_ERROR(0));
            let previous = SetWindowLongPtrW(self.0, GWL_EXSTYLE, style as isize);
            let error = GetLastError();
            if previous == 0 && error.0 != 0 {
                return Err(format!("Capture window style failed: {error:?}"));
            }
            if let Err(error) = SetWindowPos(
                self.0,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            ) {
                SetWindowLongPtrW(self.0, GWL_EXSTYLE, old);
                return Err(error.to_string());
            }
        }
        Ok(())
    }
    /// Match the frozen desktop exactly, including the taskbar and negative origins.
    /// Use a positioned overlay, never maximization or a monitor work-area rectangle.
    pub fn align_capture_desktop(
        &self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.state()?;
        let width = i32::try_from(width).map_err(|e| e.to_string())?;
        let height = i32::try_from(height).map_err(|e| e.to_string())?;
        unsafe {
            SetWindowPos(
                self.0,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
            .map_err(|e| e.to_string())?;
            let mut client = RECT::default();
            GetClientRect(self.0, &mut client).map_err(|e| e.to_string())?;
            let mut origin = POINT::default();
            if !ClientToScreen(self.0, &mut origin).as_bool() {
                return Err("Cannot locate capture client area".into());
            }
            if origin.x != x
                || origin.y != y
                || client.right - client.left != width
                || client.bottom - client.top != height
            {
                return Err("Capture overlay does not match frozen desktop bounds".into());
            }
        }
        Ok(())
    }
    /// Request foreground directly, without injecting Alt or requesting attention.
    /// Windows may deny activation; callers must leave a click-to-focus path.
    pub fn focus_without_attention(&self) -> Result<bool, String> {
        self.state()?;
        Ok(self.is_foreground() || unsafe { SetForegroundWindow(self.0) }.as_bool())
    }
    /// Synchronously hide on the HWND owner thread and finish that thread's DWM batch.
    /// A DwmFlush on the capture worker alone cannot flush UI-thread window commands.
    pub fn hide_for_capture(&self) -> Result<(), String> {
        self.disable_transitions()?;
        unsafe { SetWindowDisplayAffinity(self.0, WDA_EXCLUDEFROMCAPTURE) }
            .map_err(|e| format!("Exclude settings from capture: {e}"))?;
        unsafe {
            let _ = ShowWindow(self.0, SW_HIDE);
        }
        self.flush_composition()
    }
    /// Keep the controller visible while Windows omits it from screen capture.
    /// Call once when creating the window, never toggle affinity per frame.
    pub fn exclude_from_capture(&self) -> Result<(), String> {
        unsafe { SetWindowDisplayAffinity(self.0, WDA_EXCLUDEFROMCAPTURE) }
            .map_err(|e| format!("Exclude capture controller: {e}"))
    }
    /// Finish visibility changes on the window's UI thread before sampling desktop geometry.
    pub fn flush_composition(&self) -> Result<(), String> {
        use windows::Win32::Graphics::Dwm::{DwmFlush, DwmIsCompositionEnabled};
        if unsafe { DwmIsCompositionEnabled() }
            .map_err(|e| e.to_string())?
            .as_bool()
        {
            unsafe { DwmFlush() }.map_err(|e| format!("DwmFlush UI: {e}"))?;
        }
        Ok(())
    }
    pub fn allow_capture(&self) -> Result<(), String> {
        unsafe { SetWindowDisplayAffinity(self.0, WDA_NONE) }
            .map_err(|e| format!("Restore window capture affinity: {e}"))
    }
    /// Disable compositor show/hide animation before a capture hides an application window.
    pub fn disable_transitions(&self) -> Result<(), String> {
        self.set_transitions_disabled(true)
    }
    /// Set compositor animation policy. Capture hiding may independently force it off.
    pub fn set_transitions_disabled(&self, value: bool) -> Result<(), String> {
        use windows::Win32::Graphics::Dwm::{
            DWMWA_TRANSITIONS_FORCEDISABLED, DwmIsCompositionEnabled, DwmSetWindowAttribute,
        };
        if !unsafe { DwmIsCompositionEnabled() }
            .map_err(|e| e.to_string())?
            .as_bool()
        {
            return Ok(());
        }
        let disabled = BOOL::from(value);
        unsafe {
            DwmSetWindowAttribute(
                self.0,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                (&disabled as *const BOOL).cast(),
                std::mem::size_of::<BOOL>() as u32,
            )
        }
        .map_err(|e| format!("DwmSetWindowAttribute: {e}"))
    }
    /// # Safety
    /// `handle` must refer to a live window owned by the calling UI thread. The wrapper
    /// must not outlive that window, nor be used after Slint hides/destroys its native peer.
    pub unsafe fn from_raw(handle: *mut std::ffi::c_void) -> Self {
        Self(HWND(handle))
    }

    pub fn state(&self) -> Result<PinWindowState, String> {
        unsafe {
            if !IsWindow(self.0).as_bool() {
                return Err("Pin native window no longer exists".into());
            }
            let mut rect = RECT::default();
            GetWindowRect(self.0, &mut rect).map_err(|e| e.to_string())?;
            let style = GetWindowLongPtrW(self.0, GWL_EXSTYLE) as u32;
            Ok(PinWindowState {
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
                topmost: style & WS_EX_TOPMOST.0 != 0,
                click_through: style & WS_EX_TRANSPARENT.0 != 0,
                no_activate: style & WS_EX_NOACTIVATE.0 != 0,
                visible: IsWindowVisible(self.0).as_bool(),
            })
        }
    }

    /// Query the OS hit-test result without synthesizing any mouse input.
    pub fn receives_pointer_at(&self, x: i32, y: i32) -> Result<bool, String> {
        self.state()?;
        Ok(unsafe { WindowFromPoint(POINT { x, y }) == self.0 })
    }

    /// No focus activation, no taskbar button. Caller must provide a live rescue path before enabling passthrough.
    pub fn configure(&self, topmost: bool, click_through: bool) -> Result<(), String> {
        self.state()?;
        unsafe {
            let old = GetWindowLongPtrW(self.0, GWL_EXSTYLE);
            let mut style =
                (old as u32 | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
            if click_through {
                style |= WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0;
            } else {
                style &= !WS_EX_TRANSPARENT.0;
            }
            SetLastError(WIN32_ERROR(0));
            let previous = SetWindowLongPtrW(self.0, GWL_EXSTYLE, style as isize);
            let error = GetLastError();
            if previous == 0 && error.0 != 0 {
                return Err(format!("Pin window style failed: {error:?}"));
            }
            let result = SetWindowPos(
                self.0,
                if topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            if let Err(error) = result {
                SetWindowLongPtrW(self.0, GWL_EXSTYLE, old);
                return Err(error.to_string());
            }
        }
        Ok(())
    }

    pub fn work_area(&self) -> Result<(i32, i32, u32, u32), String> {
        unsafe {
            let monitor = MonitorFromWindow(self.0, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return Err("Cannot find pin work area".into());
            }
            let work = info.rcWork;
            Ok((
                work.left,
                work.top,
                (work.right - work.left).max(0) as u32,
                (work.bottom - work.top).max(0) as u32,
            ))
        }
    }
    /// Clear passthrough, show without activation, and move inside the nearest current work area.
    pub fn rescue(&self) -> Result<(), String> {
        self.configure(true, false)?;
        let state = self.state()?;
        unsafe {
            let monitor = MonitorFromWindow(self.0, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return Err("Cannot find a work area for pin rescue".into());
            }
            let work = info.rcWork;
            let x = state
                .x
                .clamp(work.left, (work.right - state.width).max(work.left));
            let y = state
                .y
                .clamp(work.top, (work.bottom - state.height).max(work.top));
            SetWindowPos(
                self.0,
                HWND_TOPMOST,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
