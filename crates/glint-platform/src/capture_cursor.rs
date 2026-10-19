//! Draw the sampled OS cursor into the captured GDI surface, before RGBA readback.
//! DrawIconEx preserves color alpha and monochrome AND/XOR (including inversion).
use windows::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};

struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.0);
        }
    }
}
struct Info(ICONINFO);
impl Drop for Info {
    fn drop(&mut self) {
        unsafe {
            if !self.0.hbmColor.is_invalid() {
                let _ = DeleteObject(self.0.hbmColor);
            }
            if !self.0.hbmMask.is_invalid() {
                let _ = DeleteObject(self.0.hbmMask);
            }
        }
    }
}
struct Cursor {
    icon: Icon,
    position: (i32, i32),
    hotspot: (u32, u32),
}
impl Cursor {
    fn copy(icon: HICON, position: (i32, i32)) -> Result<Self, String> {
        let icon = Icon(unsafe { CopyIcon(icon) }.map_err(|e| format!("Copy cursor: {e}"))?);
        let mut info = Info(ICONINFO::default());
        unsafe { GetIconInfo(icon.0, &mut info.0) }
            .map_err(|e| format!("Read cursor hotspot: {e}"))?;
        Ok(Self {
            icon,
            position,
            hotspot: (info.0.xHotspot, info.0.yHotspot),
        })
    }
    fn draw(&self, dc: HDC, origin: (i32, i32)) -> Result<(), String> {
        let x = i64::from(self.position.0) - i64::from(origin.0) - i64::from(self.hotspot.0);
        let y = i64::from(self.position.1) - i64::from(origin.1) - i64::from(self.hotspot.1);
        let x = i32::try_from(x).map_err(|_| "Cursor x coordinate overflow")?;
        let y = i32::try_from(y).map_err(|_| "Cursor y coordinate overflow")?;
        // Zero dimensions without DI_DEFAULTSIZE use the resource's actual size.
        // The bitmap's clip region handles cursors partially outside the desktop.
        unsafe { DrawIconEx(dc, x, y, self.icon.0, 0, 0, 0, HBRUSH::default(), DI_NORMAL) }
            .map_err(|e| format!("Compose cursor: {e}"))
    }
}

pub(super) fn draw_current(dc: HDC, origin: (i32, i32)) -> Result<(), String> {
    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetCursorInfo(&mut info) }.map_err(|e| format!("Sample cursor: {e}"))?;
    if info.flags.0 & CURSOR_SHOWING.0 == 0 {
        return Ok(());
    }
    // Copy the borrowed system cursor. Never destroy or retain its shared handle.
    Cursor::copy(
        HICON(info.hCursor.0),
        (info.ptScreenPos.x, info.ptScreenPos.y),
    )?
    .draw(dc, origin)
}

#[cfg(test)]
#[path = "capture_cursor_tests.rs"]
mod tests;
