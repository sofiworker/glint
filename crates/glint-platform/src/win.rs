use super::DesktopFrame;
use windows::Win32::{Foundation::HWND, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

struct CaptureHandles {
    screen: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl Drop for CaptureHandles {
    fn drop(&mut self) {
        // Restore selected object before deleting its bitmap and device contexts.
        unsafe {
            if !self.previous.is_invalid() {
                SelectObject(self.memory, self.previous);
            }
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap);
            }
            if !self.memory.is_invalid() {
                let _ = DeleteDC(self.memory);
            }
            if !self.screen.is_invalid() {
                ReleaseDC(HWND::default(), self.screen);
            }
        }
    }
}

pub fn capture_desktop() -> Result<DesktopFrame, String> {
    capture_desktop_options(false)
}
fn capture_desktop_options(include_cursor: bool) -> Result<DesktopFrame, String> {
    unsafe {
        capture_area(
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
            include_cursor,
        )
    }
}

/// Source-pixel capture of one screen-contained scrolling viewport.
pub fn capture_region_guarded(
    region: (i32, i32, u32, u32),
    expected: &[crate::DisplayInfo],
    exclusions: &[String],
) -> Result<DesktopFrame, String> {
    let before = crate::desktop_environment()?.displays;
    if before != expected {
        return Err("Display layout changed".into());
    }
    let (x, y, w, h) = region;
    if w < 32
        || h < 64
        || u64::from(w) * u64::from(h) * 4 > 64 * 1024 * 1024
        || !before.iter().any(|d| {
            x >= d.bounds.left
                && y >= d.bounds.top
                && i64::from(x) + i64::from(w) <= i64::from(d.bounds.right)
                && i64::from(y) + i64::from(h) <= i64::from(d.bounds.bottom)
        })
    {
        return Err("Select a scrolling region inside one display".into());
    }
    crate::check_visible_exclusions(exclusions).map_err(|_| "Capture excluded or unavailable")?;
    use windows::Win32::Graphics::Dwm::{DwmFlush, DwmIsCompositionEnabled};
    if unsafe { DwmIsCompositionEnabled() }
        .map_err(|e| e.to_string())?
        .as_bool()
    {
        unsafe { DwmFlush() }.map_err(|e| e.to_string())?;
    }
    let frame = capture_area(x, y, w as i32, h as i32, false)?;
    crate::check_visible_exclusions(exclusions).map_err(|_| "Capture excluded or unavailable")?;
    if crate::desktop_environment()?.displays != before {
        return Err("Display layout changed".into());
    }
    Ok(frame)
}

fn capture_area(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    include_cursor: bool,
) -> Result<DesktopFrame, String> {
    // All GDI resources are thread-local and released on every error path.
    unsafe {
        if width <= 0 || height <= 0 {
            return Err("No desktop display available".into());
        }
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or("Desktop exceeds 512 MiB capture budget")?;
        let mut h = CaptureHandles {
            screen: GetDC(HWND::default()),
            memory: HDC::default(),
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
        };
        if h.screen.is_invalid() {
            return Err("GetDC failed".into());
        }
        h.memory = CreateCompatibleDC(h.screen);
        if h.memory.is_invalid() {
            return Err("CreateCompatibleDC failed".into());
        }
        h.bitmap = CreateCompatibleBitmap(h.screen, width, height);
        if h.bitmap.is_invalid() {
            return Err("CreateCompatibleBitmap failed".into());
        }
        h.previous = SelectObject(h.memory, h.bitmap);
        if h.previous.is_invalid() {
            return Err("SelectObject failed".into());
        }
        BitBlt(
            h.memory,
            0,
            0,
            width,
            height,
            h.screen,
            x,
            y,
            SRCCOPY | CAPTUREBLT,
        )
        .map_err(|e| format!("BitBlt: {e}"))?;
        if include_cursor {
            crate::capture_cursor::draw_current(h.memory, (x, y))?;
        }
        SelectObject(h.memory, h.previous);
        h.previous = HGDIOBJ::default();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut rgba = vec![0; bytes];
        if GetDIBits(
            h.memory,
            h.bitmap,
            0,
            height as u32,
            Some(rgba.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        ) != height
        {
            return Err("GetDIBits returned incomplete desktop pixels".into());
        }
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        Ok(DesktopFrame {
            x,
            y,
            width: width as u32,
            height: height as u32,
            rgba,
        })
    }
}

/// Capture only after prior hide operations have reached the desktop compositor.
pub fn capture_after_composition() -> Result<DesktopFrame, String> {
    capture_after_composition_options(false)
}
pub(super) fn capture_after_composition_options(
    include_cursor: bool,
) -> Result<DesktopFrame, String> {
    use windows::Win32::Graphics::Dwm::{DwmFlush, DwmIsCompositionEnabled};
    let composited = unsafe { DwmIsCompositionEnabled() }
        .map_err(|e| format!("DwmIsCompositionEnabled: {e}"))?;
    if composited.as_bool() {
        unsafe { DwmFlush() }.map_err(|e| format!("DwmFlush: {e}"))?;
    }
    capture_desktop_options(include_cursor)
}
