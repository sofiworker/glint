//! Explicit M0 diagnostics; contains no window titles, screenshots or clipboard data.
use windows::Win32::{
    Foundation::{BOOL, ERROR_SUCCESS, GetLastError, LPARAM, RECT, SetLastError},
    Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
    System::Threading::{GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetGuiResources},
    UI::{
        HiDpi::{
            GetAwarenessFromDpiAwarenessContext, GetDpiForMonitor, GetThreadDpiAwarenessContext,
            MDT_EFFECTIVE_DPI,
        },
        WindowsAndMessaging::{GetSystemMetrics, MONITORINFOF_PRIMARY, SM_REMOTESESSION},
    },
};

/// Instantaneous process counters; no screenshot/content data or persistent log.
#[derive(Clone, Debug)]
pub struct ProcessResources {
    pub working_set_bytes: usize,
    pub private_bytes: usize,
    pub handles: u32,
    pub gdi_objects: u32,
    pub user_objects: u32,
    /// Cumulative CPU time, not utilization or an idle measurement.
    pub cpu_100ns: u64,
}
/// One boundary observation of session input time; does not inspect keys or content.
/// Equality across observations is only a session input indicator, not proof of
/// application inactivity or of input on other desktops/sessions.
pub fn last_session_input_tick() -> Result<u32, String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        Ok(info.dwTime)
    } else {
        Err("GetLastInputInfo failed".into())
    }
}
pub fn process_resources() -> Result<ProcessResources, String> {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{
                GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
            },
            Threading::{GetProcessHandleCount, GetProcessTimes},
        },
    };
    let process = unsafe { GetCurrentProcess() };
    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    let mut handles = 0;
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        GetProcessMemoryInfo(
            process,
            (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
            memory.cb,
        )
        .map_err(|e| format!("GetProcessMemoryInfo: {e}"))?;
        GetProcessHandleCount(process, &mut handles)
            .map_err(|e| format!("GetProcessHandleCount: {e}"))?;
        GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user)
            .map_err(|e| format!("GetProcessTimes: {e}"))?;
    }
    let (gdi_objects, user_objects) = gui_resource_counts()?;
    let ticks =
        |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    Ok(ProcessResources {
        working_set_bytes: memory.WorkingSetSize,
        private_bytes: memory.PrivateUsage,
        handles,
        gdi_objects,
        user_objects,
        cpu_100ns: ticks(kernel).saturating_add(ticks(user)),
    })
}

#[cfg(test)]
mod resource_tests {
    #[test]
    fn actual_process_resource_snapshot_has_valid_memory_and_monotonic_cpu() {
        let a = super::process_resources().unwrap();
        let b = super::process_resources().unwrap();
        assert!(a.working_set_bytes > 0 && a.private_bytes > 0 && a.handles > 0);
        assert!(b.cpu_100ns >= a.cpu_100ns);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenBounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl From<RECT> for ScreenBounds {
    fn from(value: RECT) -> Self {
        Self {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayInfo {
    pub bounds: ScreenBounds,
    pub work_area: ScreenBounds,
    pub primary: bool,
    /// Effective DPI in the calling thread's awareness context, not panel physical density.
    pub effective_dpi: (u32, u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopEnvironment {
    /// Win32 DPI_AWARENESS: -1 invalid, 0 unaware, 1 system aware, 2 per-monitor aware.
    pub dpi_awareness: i32,
    pub remote_session: bool,
    pub displays: Vec<DisplayInfo>,
}

#[derive(Default)]
struct Enumeration {
    displays: Vec<DisplayInfo>,
    error: Option<String>,
}

unsafe extern "system" fn monitor_callback(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // EnumDisplayMonitors is synchronous: the stack-owned Enumeration outlives every callback.
    let state = unsafe { &mut *(data.0 as *mut Enumeration) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        state.error = Some(format!(
            "GetMonitorInfoW: {}",
            windows::core::Error::from_win32()
        ));
        return BOOL(0);
    }
    let (mut x, mut y) = (0, 0);
    if let Err(error) = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) } {
        state.error = Some(format!("GetDpiForMonitor: {error}"));
        return BOOL(0);
    }
    state.displays.push(DisplayInfo {
        bounds: info.rcMonitor.into(),
        work_area: info.rcWork.into(),
        primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        effective_dpi: (x, y),
    });
    BOOL(1)
}

/// Call after the UI backend establishes DPI awareness; does not change that context itself.
pub fn desktop_environment() -> Result<DesktopEnvironment, String> {
    let mut state = Enumeration::default();
    let ok = unsafe {
        EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(monitor_callback),
            LPARAM(&mut state as *mut Enumeration as isize),
        )
    };
    if let Some(error) = state.error {
        return Err(error);
    }
    if !ok.as_bool() {
        return Err(format!(
            "EnumDisplayMonitors: {}",
            windows::core::Error::from_win32()
        ));
    }
    if state.displays.is_empty() {
        return Err("No display monitors available".into());
    }
    state
        .displays
        .sort_by_key(|display| (display.bounds.left, display.bounds.top));
    Ok(DesktopEnvironment {
        dpi_awareness: unsafe {
            GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext())
        }
        .0,
        remote_session: unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 },
        displays: state.displays,
    })
}

/// Current process's GDI and USER object counts. Zero can mean no allocated objects.
pub fn gui_resource_counts() -> Result<(u32, u32), String> {
    unsafe {
        let process = GetCurrentProcess();
        let count = |kind| {
            SetLastError(ERROR_SUCCESS);
            let count = GetGuiResources(process, kind);
            if count == 0 && GetLastError() != ERROR_SUCCESS {
                Err(format!(
                    "GetGuiResources: {}",
                    windows::core::Error::from_win32()
                ))
            } else {
                Ok(count)
            }
        };
        Ok((count(GR_GDIOBJECTS)?, count(GR_USEROBJECTS)?))
    }
}
