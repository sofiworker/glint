//! One-shot, front-to-back top-level window geometry. No titles or window contents are collected.
use crate::{DesktopFrame, ScreenBounds};
use std::collections::HashSet;
use windows::Win32::{Foundation::*, Graphics::Dwm::*, UI::WindowsAndMessaging::*};

#[derive(Clone, Debug)]
pub struct WindowCandidate {
    /// Opaque identity for diagnostics; never dereferenced after enumeration.
    pub id: usize,
    pub bounds: ScreenBounds,
    pub extended_frame_bounds: bool,
}
pub struct DesktopSnapshot {
    pub displays: Vec<crate::DisplayInfo>,
    pub frame: DesktopFrame,
    pub windows: Vec<WindowCandidate>,
    pub detection_error: Option<String>,
    pub pointer: Option<(i32, i32)>,
}
pub fn capture_snapshot(detect: bool) -> Result<DesktopSnapshot, String> {
    capture_snapshot_with_cursor(detect, false)
}
pub fn capture_snapshot_with_cursor(
    detect: bool,
    include_cursor: bool,
) -> Result<DesktopSnapshot, String> {
    let mut displays = crate::desktop_environment()?.displays;
    displays.sort_by_key(|d| (d.bounds.left, d.bounds.top, d.bounds.right, d.bounds.bottom));
    // Snapshot geometry before opening the overlay. Never hit-test live windows behind a frozen image.
    let detected = if detect {
        visible_windows()
    } else {
        Ok(vec![])
    };
    let (windows, detection_error) = match detected {
        Ok(mut windows) => {
            // Only external windows belong to the frozen desktop's detection map.
            // Keep visible_windows() general for pin snapping and diagnostics.
            windows.retain(|candidate| {
                let mut process = 0;
                unsafe {
                    GetWindowThreadProcessId(HWND(candidate.id as *mut _), Some(&mut process));
                }
                process != 0 && process != std::process::id()
            });
            (windows, None)
        }
        Err(error) => (vec![], Some(error)),
    };
    let mut point = POINT::default();
    let pointer = unsafe { GetCursorPos(&mut point) }
        .ok()
        .map(|_| (point.x, point.y));
    let frame = crate::win::capture_after_composition_options(include_cursor)?;
    let mut after = crate::desktop_environment()?.displays;
    after.sort_by_key(|d| (d.bounds.left, d.bounds.top, d.bounds.right, d.bounds.bottom));
    if displays != after {
        return Err("Display layout changed during capture; retry".into());
    }
    Ok(DesktopSnapshot {
        displays,
        frame,
        windows,
        detection_error,
        pointer,
    })
}

pub fn visible_windows() -> Result<Vec<WindowCandidate>, String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut window = unsafe { GetTopWindow(None) }.map_err(|e| format!("GetTopWindow: {e}"))?;
    while !window.is_invalid() {
        if seen.len() >= 4096 || !seen.insert(window.0 as usize) {
            return Err(
                "Window enumeration changed or exceeded its limit; use manual selection".into(),
            );
        }
        if let Some(candidate) = candidate(window) {
            result.push(candidate);
        }
        window = unsafe { GetWindow(window, GW_HWNDNEXT) }.unwrap_or_default();
    }
    Ok(result)
}
fn candidate(window: HWND) -> Option<WindowCandidate> {
    unsafe {
        if !IsWindowVisible(window).as_bool() || IsIconic(window).as_bool() {
            return None;
        }
        let mut class = [0u16; 64];
        let length = GetClassNameW(window, &mut class) as usize;
        let class_is = |text: &str| class[..length].iter().copied().eq(text.encode_utf16());
        let taskbar = class_is("Shell_TrayWnd") || class_is("Shell_SecondaryTrayWnd");
        if class_is("Progman") || class_is("WorkerW") {
            return None;
        }
        let extended_style = WINDOW_EX_STYLE(GetWindowLongW(window, GWL_EXSTYLE) as u32);
        // Ignore nonactivating input-pass-through helper surfaces (including the
        // Windows handwriting canvas). NOACTIVATE/TOOLWINDOW alone are valid targets.
        // Explorer taskbars may carry both flags, but are visible capture targets.
        if !taskbar && extended_style.contains(WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) {
            return None;
        }
        if extended_style.contains(WS_EX_LAYERED) {
            let mut alpha = 255u8;
            let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS::default();
            if GetLayeredWindowAttributes(window, None, Some(&mut alpha), Some(&mut flags)).is_ok()
                && flags.contains(LWA_ALPHA)
                && alpha == 0
            {
                return None;
            }
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(window, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4)
            .is_ok()
            && cloaked != 0
        {
            return None;
        }
        let mut affinity = 0;
        if GetWindowDisplayAffinity(window, &mut affinity).is_ok() && affinity != WDA_NONE.0 {
            return None;
        }
        let mut rect = RECT::default();
        // The taskbar is a shell surface, not a decorated application frame.
        // Its window rectangle is the visible bar's capture boundary.
        let extended_frame_bounds = !taskbar && DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rect as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok();
        if !extended_frame_bounds && GetWindowRect(window, &mut rect).is_err() {
            return None;
        }
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return None;
        }
        Some(WindowCandidate {
            id: window.0 as usize,
            bounds: rect.into(),
            extended_frame_bounds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::w;
    struct TestWindow(HWND);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    fn window(x: i32) -> TestWindow {
        TestWindow(
            unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                    w!("STATIC"),
                    w!("Glint geometry verification"),
                    WS_POPUP | WS_VISIBLE,
                    x,
                    200,
                    160,
                    120,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        )
    }
    #[test]
    #[ignore = "Temporarily displays two nonactivating Win32 verification windows"]
    fn real_window_order_hidden_minimized_and_capture_exclusion() {
        let foreground = crate::foreground_window_id();
        let back = window(200);
        let front = window(220);
        let list = visible_windows().unwrap();
        let a = list
            .iter()
            .position(|w| w.id == front.0.0 as usize)
            .unwrap();
        let b = list.iter().position(|w| w.id == back.0.0 as usize).unwrap();
        assert!(a < b);
        assert_eq!(
            list[a].bounds,
            ScreenBounds {
                left: 220,
                top: 200,
                right: 380,
                bottom: 320
            }
        );
        unsafe {
            let style = GetWindowLongW(front.0, GWL_EXSTYLE);
            SetWindowLongW(front.0, GWL_EXSTYLE, style | WS_EX_TRANSPARENT.0 as i32);
            assert!(
                !visible_windows()
                    .unwrap()
                    .iter()
                    .any(|w| w.id == front.0.0 as usize)
            );
            SetWindowLongW(front.0, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as i32);
            SetLayeredWindowAttributes(front.0, COLORREF(0), 0, LWA_ALPHA).unwrap();
            assert!(
                !visible_windows()
                    .unwrap()
                    .iter()
                    .any(|w| w.id == front.0.0 as usize)
            );
            SetLayeredWindowAttributes(front.0, COLORREF(0), 128, LWA_ALPHA).unwrap();
            assert!(
                visible_windows()
                    .unwrap()
                    .iter()
                    .any(|w| w.id == front.0.0 as usize)
            );
            SetWindowLongW(front.0, GWL_EXSTYLE, style);
        }
        unsafe {
            let _ = ShowWindow(front.0, SW_HIDE);
        }
        assert!(
            !visible_windows()
                .unwrap()
                .iter()
                .any(|w| w.id == front.0.0 as usize)
        );
        unsafe {
            let _ = ShowWindow(front.0, SW_SHOWNOACTIVATE);
            SetWindowDisplayAffinity(front.0, WDA_EXCLUDEFROMCAPTURE).unwrap();
        }
        assert!(
            !visible_windows()
                .unwrap()
                .iter()
                .any(|w| w.id == front.0.0 as usize)
        );
        unsafe {
            SetWindowDisplayAffinity(front.0, WDA_NONE).unwrap();
            let _ = ShowWindow(front.0, SW_SHOWMINNOACTIVE);
        }
        assert!(
            !visible_windows()
                .unwrap()
                .iter()
                .any(|w| w.id == front.0.0 as usize)
        );
        assert_eq!(crate::foreground_window_id(), foreground);
    }
}
