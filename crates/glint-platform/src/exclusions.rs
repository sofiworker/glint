//! Conservative one-shot visibility gate, not atomic exclusion from GDI capture.
use crate::{DesktopSnapshot, ScreenBounds};
use std::collections::{HashMap, HashSet};
use windows::Win32::{
    Foundation::*, Globalization::*, System::Threading::*, UI::WindowsAndMessaging::*,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExclusionError {
    Blocked,
    Unavailable,
}

#[derive(Debug)]
pub enum GuardedCaptureError {
    Exclusion(ExclusionError),
    Capture(String),
}

/// Empty policies use the unchanged capture path. No process names escape this check.
pub fn capture_snapshot_guarded(
    detect: bool,
    cursor: bool,
    names: &[String],
) -> Result<DesktopSnapshot, GuardedCaptureError> {
    if names.is_empty() {
        return crate::capture_snapshot_with_cursor(detect, cursor)
            .map_err(GuardedCaptureError::Capture);
    }
    checked_capture(
        || check_visible_exclusions(names),
        || crate::capture_snapshot_with_cursor(detect, cursor),
    )
}

fn checked_capture<T>(
    mut check: impl FnMut() -> Result<(), ExclusionError>,
    capture: impl FnOnce() -> Result<T, String>,
) -> Result<T, GuardedCaptureError> {
    check().map_err(GuardedCaptureError::Exclusion)?;
    let image = capture().map_err(GuardedCaptureError::Capture)?;
    check().map_err(GuardedCaptureError::Exclusion)?;
    Ok(image)
}

/// Includes covered, transparent, cloaked and capture-affinity windows conservatively.
/// A changing/inaccessible process fails closed. Only minimized/hidden/offscreen windows skip.
pub fn check_visible_exclusions(names: &[String]) -> Result<(), ExclusionError> {
    if names.is_empty() {
        return Ok(());
    }
    let displays = crate::desktop_environment()
        .map_err(|_| ExclusionError::Unavailable)?
        .displays;
    let mut seen = HashSet::new();
    let mut processes = HashMap::new();
    let mut window = unsafe { GetTopWindow(None) }.map_err(|_| ExclusionError::Unavailable)?;
    while !window.is_invalid() {
        if seen.len() >= 4096 || !seen.insert(window.0 as usize) {
            return Err(ExclusionError::Unavailable);
        }
        unsafe {
            if IsWindowVisible(window).as_bool() && !IsIconic(window).as_bool() {
                let mut rect = RECT::default();
                GetWindowRect(window, &mut rect).map_err(|_| ExclusionError::Unavailable)?;
                let bounds: ScreenBounds = rect.into();
                if displays.iter().any(|d| intersects(bounds, d.bounds)) {
                    let mut pid = 0;
                    if GetWindowThreadProcessId(window, Some(&mut pid)) == 0 || pid == 0 {
                        return Err(ExclusionError::Unavailable);
                    }
                    // Winit's own per-pixel transparent event target has WS_VISIBLE
                    // and a 16x16 rectangle, but never draws application content.
                    // Restrict this exception to our process and its exact helper
                    // class. Other apps' similarly named/transparent windows count.
                    if pid == std::process::id() && own_event_target(window) {
                        window = GetWindow(window, GW_HWNDNEXT).unwrap_or_default();
                        continue;
                    }
                    let name = match processes.entry(pid) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            entry.insert(process_name(pid)?)
                        }
                    };
                    if names.iter().any(|excluded| same_name(name, excluded)) {
                        return Err(ExclusionError::Blocked);
                    }
                }
            }
            window = GetWindow(window, GW_HWNDNEXT).unwrap_or_default();
        }
    }
    Ok(())
}

fn intersects(a: ScreenBounds, b: ScreenBounds) -> bool {
    a.right > a.left
        && a.bottom > a.top
        && a.left < b.right
        && a.right > b.left
        && a.top < b.bottom
        && a.bottom > b.top
}
fn own_event_target(window: HWND) -> bool {
    let mut class = [0u16; 128];
    let length = unsafe { GetClassNameW(window, &mut class) } as usize;
    let style = WINDOW_EX_STYLE(unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32);
    style.contains(WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TOOLWINDOW)
        && class[..length]
            .iter()
            .copied()
            .eq("Winit Thread Event Target".encode_utf16())
}
fn same_name(a: &str, b: &str) -> bool {
    let a: Vec<_> = a.encode_utf16().collect();
    let b: Vec<_> = b.encode_utf16().collect();
    unsafe { CompareStringOrdinal(&a, &b, true) == CSTR_EQUAL }
}
struct Process(HANDLE);
impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn process_name(pid: u32) -> Result<String, ExclusionError> {
    let process = Process(
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|_| ExclusionError::Unavailable)?,
    );
    let mut path = vec![0u16; 32768];
    let mut length = path.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|_| ExclusionError::Unavailable)?;
    let path =
        String::from_utf16(&path[..length as usize]).map_err(|_| ExclusionError::Unavailable)?;
    path.rsplit(['\\', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or(ExclusionError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_before_capture_or_discards_after_capture() {
        let mut captured = false;
        assert!(matches!(
            checked_capture(
                || Err(ExclusionError::Blocked),
                || {
                    captured = true;
                    Ok(())
                }
            ),
            Err(GuardedCaptureError::Exclusion(ExclusionError::Blocked))
        ));
        assert!(!captured);
        let mut count = 0;
        assert!(matches!(
            checked_capture(
                || {
                    count += 1;
                    if count == 2 {
                        Err(ExclusionError::Unavailable)
                    } else {
                        Ok(())
                    }
                },
                || Ok(vec![1, 2, 3])
            ),
            Err(GuardedCaptureError::Exclusion(ExclusionError::Unavailable))
        ));
        assert_eq!(count, 2);
        assert!(same_name("程序.EXE", "程序.exe"));
        assert!(!same_name("other.exe", "程序.exe"));
    }
    #[test]
    #[ignore = "Displays a nonactivating Win32 fixture and performs real GDI capture; run serially"]
    fn real_process_window_blocks_and_hidden_window_allows_retry() {
        use windows::core::w;
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let name = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let window = Window(
            unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                    w!("STATIC"),
                    w!("Glint exclusion verification"),
                    WS_POPUP | WS_VISIBLE,
                    200,
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
        );
        assert_eq!(process_name(std::process::id()).unwrap(), name);
        let names = vec![name.to_uppercase()];
        assert_eq!(
            check_visible_exclusions(&names),
            Err(ExclusionError::Blocked)
        );
        assert!(matches!(
            capture_snapshot_guarded(false, false, &names),
            Err(GuardedCaptureError::Exclusion(ExclusionError::Blocked))
        ));
        unsafe {
            let _ = ShowWindow(window.0, SW_HIDE);
        }
        assert_eq!(check_visible_exclusions(&names), Ok(()));
        assert!(capture_snapshot_guarded(false, false, &names).is_ok());
    }
}
