//! Live capture boundary and event-driven scroll input. No pixels or UI types.
use std::{
    cell::RefCell,
    sync::{OnceLock, mpsc},
    thread,
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

pub type Region = (i32, i32, u32, u32);
struct Paint {
    region: RECT,
}
unsafe extern "system" fn paint_window(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let dc = unsafe { BeginPaint(hwnd, &mut ps) };
        let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Paint };
        if !state.is_null() {
            let mut bounds = RECT::default();
            let _ = unsafe { GetClientRect(hwnd, &mut bounds) };
            let dark = unsafe { CreateSolidBrush(COLORREF(0x00100c08)) };
            unsafe {
                FillRect(dc, &bounds, dark);
            }
            let _ = unsafe { DeleteObject(dark) };
        }
        let _ = unsafe { EndPaint(hwnd, &ps) };
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wp, lp) }
}

/// Click-through shade outside the live region. The inside is a native window
/// region hole, so scrolling/clicking reaches the actual source application.
pub struct LiveCaptureFrame {
    hwnd: HWND,
    _paint: Box<Paint>,
}
impl LiveCaptureFrame {
    pub fn new(region: Region) -> Result<Self, String> {
        Self::create(region, true, true)
    }
    /// Prepare the boundary without replacing the frozen selection yet.
    pub fn prepare(region: Region) -> Result<Self, String> {
        // The native region hole covers every source pixel. The shade/border
        // outside it need no capture exclusion and remain visible in sharing.
        Self::create(region, false, false)
    }
    fn create(region: Region, visible: bool, excluded: bool) -> Result<Self, String> {
        static CLASS: OnceLock<Result<(), String>> = OnceLock::new();
        CLASS
            .get_or_init(|| {
                let module = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(paint_window),
                    hInstance: HINSTANCE(module.0),
                    lpszClassName: w!("Glint.LiveCaptureBoundary"),
                    ..Default::default()
                };
                if unsafe { RegisterClassW(&wc) } == 0 {
                    return Err("Could not register live capture boundary".into());
                }
                Ok(())
            })
            .clone()?;
        let (x, y, w, h) = region;
        if w < 32 || h < 64 {
            return Err("Invalid live capture region".into());
        }
        let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        let paint = Box::new(Paint {
            region: RECT {
                left: x - vx,
                top: y - vy,
                right: (i64::from(x) + i64::from(w) - i64::from(vx))
                    .try_into()
                    .map_err(|_| "Region overflow")?,
                bottom: (i64::from(y) + i64::from(h) - i64::from(vy))
                    .try_into()
                    .map_err(|_| "Region overflow")?,
            },
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                w!("Glint.LiveCaptureBoundary"),
                w!("Glint capture boundary"),
                WS_POPUP,
                vx,
                vy,
                vw,
                vh,
                None,
                None,
                HINSTANCE(GetModuleHandleW(None).map_err(|e| e.to_string())?.0),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        let frame = Self {
            hwnd,
            _paint: paint,
        };
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                (&*frame._paint as *const Paint) as isize,
            );
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 95, LWA_ALPHA)
                .map_err(|e| e.to_string())?;
            let outer = CreateRectRgn(0, 0, vw, vh);
            let r = &frame._paint.region;
            let hole = CreateRectRgn(r.left, r.top, r.right, r.bottom);
            CombineRgn(outer, outer, hole, RGN_DIFF);
            let _ = DeleteObject(hole);
            if SetWindowRgn(hwnd, outer, false) == 0 {
                let _ = DeleteObject(outer);
                return Err("Could not create live capture hole".into());
            }
            if excluded {
                SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
                    .map_err(|e| format!("Exclude live selection from capture: {e}"))?;
            }
            if visible {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
        }
        Ok(frame)
    }
    pub fn hide(&self) {
        let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
    }
    pub fn show(&self) {
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };
        // Other topmost windows may have appeared after this one; the boundary
        // must stay above every source window for the whole live session.
        let _ = unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }
    pub fn visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) }.as_bool()
    }
}
impl Drop for LiveCaptureFrame {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Fully opaque selection ring for the live region. Colour-keyed transparency
/// keeps the ring crisp on any desktop, while the interior stays invisible and
/// click-through, so the source page keeps receiving wheel and pointer input.
pub struct LiveBorder {
    hwnd: HWND,
    _paint: Box<BorderPaint>,
}
const BORDER: i32 = 3;
const BORDER_KEY: COLORREF = COLORREF(0x00040302);
/// Prototype default accent (#4b5cfa) as 0x00RRGGBB.
const DEFAULT_ACCENT: u32 = 0x4b5cfa;

struct BorderPaint {
    inner: RECT,
    accent: COLORREF,
}

unsafe extern "system" fn border_window(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let dc = unsafe { BeginPaint(hwnd, &mut ps) };
        let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const BorderPaint };
        if !state.is_null() {
            let mut bounds = RECT::default();
            let _ = unsafe { GetClientRect(hwnd, &mut bounds) };
            let key = unsafe { CreateSolidBrush(BORDER_KEY) };
            let accent = unsafe { CreateSolidBrush((*state).accent) };
            unsafe {
                FillRect(dc, &bounds, key);
                let r = &(*state).inner;
                for rect in [
                    RECT {
                        left: 0,
                        top: 0,
                        right: bounds.right,
                        bottom: r.top,
                    },
                    RECT {
                        left: 0,
                        top: r.bottom,
                        right: bounds.right,
                        bottom: bounds.bottom,
                    },
                    RECT {
                        left: 0,
                        top: r.top,
                        right: r.left,
                        bottom: r.bottom,
                    },
                    RECT {
                        left: r.right,
                        top: r.top,
                        right: bounds.right,
                        bottom: r.bottom,
                    },
                ] {
                    FillRect(dc, &rect, accent);
                }
            }
            let _ = unsafe { DeleteObject(key) };
            let _ = unsafe { DeleteObject(accent) };
        }
        let _ = unsafe { EndPaint(hwnd, &ps) };
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wp, lp) }
}

impl LiveBorder {
    /// `accent` is 0x00RRGGBB. The ring lies entirely outside `region`, so
    /// source frames never contain it and no capture exclusion is needed.
    pub fn new(region: Region, accent: u32) -> Result<Self, String> {
        static CLASS: OnceLock<Result<(), String>> = OnceLock::new();
        CLASS
            .get_or_init(|| {
                let module = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(border_window),
                    hInstance: HINSTANCE(module.0),
                    lpszClassName: w!("Glint.LiveCaptureBorder"),
                    ..Default::default()
                };
                if unsafe { RegisterClassW(&wc) } == 0 {
                    return Err("Could not register live capture border".into());
                }
                Ok(())
            })
            .clone()?;
        let (x, y, w, h) = region;
        if w < 32 || h < 64 {
            return Err("Invalid live capture region".into());
        }
        let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        let left = (x - BORDER).max(vx);
        let top = (y - BORDER).max(vy);
        let right =
            (i64::from(x) + i64::from(w) + i64::from(BORDER)).min(i64::from(vx) + i64::from(vw));
        let bottom =
            (i64::from(y) + i64::from(h) + i64::from(BORDER)).min(i64::from(vy) + i64::from(vh));
        let width = (right - i64::from(left)).max(1) as i32;
        let height = (bottom - i64::from(top)).max(1) as i32;
        let paint = Box::new(BorderPaint {
            inner: RECT {
                left: x - left,
                top: y - top,
                right: x - left + w as i32,
                bottom: y - top + h as i32,
            },
            // Callers pass 0x00RRGGBB; COLORREF stores 0x00BBGGRR.
            accent: {
                let value = if accent == 0 { DEFAULT_ACCENT } else { accent };
                COLORREF(((value & 0xff) << 16) | (value & 0xff00) | ((value >> 16) & 0xff))
            },
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                w!("Glint.LiveCaptureBorder"),
                w!("Glint capture border"),
                WS_POPUP,
                left,
                top,
                width,
                height,
                None,
                None,
                HINSTANCE(GetModuleHandleW(None).map_err(|e| e.to_string())?.0),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                (&*paint as *const BorderPaint) as isize,
            );
            SetLayeredWindowAttributes(hwnd, BORDER_KEY, 0, LWA_COLORKEY)
                .map_err(|e| format!("Colour-key the live capture border: {e}"))?;
        }
        Ok(Self {
            hwnd,
            _paint: paint,
        })
    }
    pub fn hide(&self) {
        let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
    }
    pub fn show(&self) {
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };
        let _ = unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }
    pub fn visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) }.as_bool()
    }
}
impl Drop for LiveBorder {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    #[test]
    fn visible_excluded_controller_does_not_enter_gdi_capture() {
        let source = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!(""),
                WS_POPUP | WINDOW_STYLE(6),
                100,
                100,
                180,
                160,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        struct Owned(HWND);
        impl Drop for Owned {
            fn drop(&mut self) {
                let _ = unsafe { DestroyWindow(self.0) };
            }
        }
        let _source = Owned(source);
        let _ = unsafe { ShowWindow(source, SW_SHOWNOACTIVATE) };
        unsafe { windows::Win32::Graphics::Dwm::DwmFlush() }.unwrap();
        let displays = crate::desktop_environment().unwrap().displays;
        let baseline = crate::capture_region_guarded((120, 120, 100, 100), &displays, &[]).unwrap();
        let controller = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!(""),
                WS_POPUP | WINDOW_STYLE(4),
                110,
                110,
                140,
                130,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        let _controller = Owned(controller);
        unsafe { SetWindowDisplayAffinity(controller, WDA_EXCLUDEFROMCAPTURE) }.unwrap();
        let _ = unsafe { ShowWindow(controller, SW_SHOWNOACTIVATE) };
        unsafe { windows::Win32::Graphics::Dwm::DwmFlush() }.unwrap();
        for _ in 0..3 {
            let captured =
                crate::capture_region_guarded((120, 120, 100, 100), &displays, &[]).unwrap();
            assert!(unsafe { IsWindowVisible(controller) }.as_bool());
            assert!(
                baseline.rgba == captured.rgba,
                "Visible excluded controller changed source pixels"
            );
        }
    }
}

struct Watch {
    region: Region,
    callback: Box<dyn Fn(i16)>,
}
thread_local! {static WATCH:RefCell<Option<Watch>>=const {RefCell::new(None)};}
unsafe extern "system" fn wheel_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && wp.0 == WM_MOUSEWHEEL as usize {
        let event = unsafe { &*(lp.0 as *const MSLLHOOKSTRUCT) };
        WATCH.with(|slot| {
            if let Some(watch) = slot.borrow().as_ref() {
                let (x, y, w, h) = watch.region;
                if event.pt.x >= x
                    && event.pt.y >= y
                    && i64::from(event.pt.x) < i64::from(x) + i64::from(w)
                    && i64::from(event.pt.y) < i64::from(y) + i64::from(h)
                {
                    (watch.callback)((event.mouseData >> 16) as i16);
                }
            }
        });
    }
    unsafe { CallNextHookEx(None, code, wp, lp) }
}
pub struct ScrollWatch {
    id: u32,
    thread: Option<thread::JoinHandle<()>>,
}
impl ScrollWatch {
    pub fn new(region: Region, callback: impl Fn(i16) + Send + 'static) -> Result<Self, String> {
        let (send, receive) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("glint-scroll-events".into())
            .spawn(move || {
                let installed = (|| -> Result<_, String> {
                    let mut message = MSG::default();
                    unsafe {
                        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                    }
                    let hook = unsafe {
                        SetWindowsHookExW(
                            WH_MOUSE_LL,
                            Some(wheel_hook),
                            HINSTANCE(GetModuleHandleW(None).map_err(|e| e.to_string())?.0),
                            0,
                        )
                    }
                    .map_err(|e| e.to_string())?;
                    Ok((hook, unsafe { GetCurrentThreadId() }))
                })();
                let (hook, id) = match installed {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = send.send(Err(error));
                        return;
                    }
                };
                WATCH.with(|slot| {
                    *slot.borrow_mut() = Some(Watch {
                        region,
                        callback: Box::new(callback),
                    })
                });
                if send.send(Ok(id)).is_ok() {
                    let mut message = MSG::default();
                    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                        unsafe {
                            let _ = TranslateMessage(&message);
                            DispatchMessageW(&message);
                        }
                    }
                }
                let _ = unsafe { UnhookWindowsHookEx(hook) };
                WATCH.with(|slot| slot.borrow_mut().take());
            })
            .map_err(|e| e.to_string())?;
        let id = receive.recv().map_err(|_| "Scroll listener failed")??;
        Ok(Self {
            id,
            thread: Some(thread),
        })
    }
}
impl Drop for ScrollWatch {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Binds automatic scrolling to the original visible window, without moving the
/// pointer or injecting global input. A changed/hidden target stops the session.
pub struct ScrollSource {
    window: HWND,
    root: HWND,
    process: u32,
    point: POINT,
    wheel_delta: u16,
}
impl ScrollSource {
    pub fn new(region: Region) -> Result<Self, String> {
        let point = POINT {
            x: region.0.saturating_add((region.2 / 2) as i32),
            y: region.1.saturating_add((region.3 / 2) as i32),
        };
        let window = unsafe { WindowFromPoint(point) };
        let root = unsafe { GetAncestor(window, GA_ROOT) };
        let mut process = 0;
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut process));
        }
        if window.0.is_null() || process == std::process::id() {
            return Err("No scrollable source window at the selected region".into());
        }
        Ok(Self {
            window,
            root,
            process,
            point,
            // One full wheel notch can move ~200px in Chromium. Preserve
            // overlap in short selections by requesting a smaller wheel delta.
            wheel_delta: (region.3 / 6).clamp(10, 120) as u16,
        })
    }
    /// Legacy controls may accumulate partial deltas before moving. Allow at
    /// least one complete notch plus a further attempt before declaring idle.
    pub fn idle_step_limit(&self) -> u8 {
        (120u16.div_ceil(self.wheel_delta) + 1) as u8
    }
    pub fn scroll_down(&self) -> Result<(), String> {
        let mut process = 0;
        unsafe {
            GetWindowThreadProcessId(self.window, Some(&mut process));
        }
        if process != self.process
            || !unsafe { IsWindowVisible(self.window) }.as_bool()
            || unsafe { GetForegroundWindow() } != self.root
        {
            return Err("Source window changed or lost focus; automatic scrolling stopped".into());
        }
        let delta = (-(self.wheel_delta as i16)) as u16 as usize;
        let position = ((self.point.y as u16 as u32) << 16) | self.point.x as u16 as u32;
        unsafe {
            PostMessageW(
                self.window,
                WM_MOUSEWHEEL,
                WPARAM(delta << 16),
                LPARAM(position as isize),
            )
        }
        .map_err(|e| e.to_string())
    }
}
