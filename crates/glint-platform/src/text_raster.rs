//! Windows font linking/shaping into a grayscale mask. No text or font files are logged.
use windows::{
    Win32::{
        Foundation::{COLORREF, RECT},
        Graphics::Gdi::*,
    },
    core::PCWSTR,
};

pub struct TextMask {
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<u8>,
}
struct Handles {
    dc: HDC,
    font: HFONT,
    bitmap: HBITMAP,
    old_font: HGDIOBJ,
    old_bitmap: HGDIOBJ,
}
impl Drop for Handles {
    fn drop(&mut self) {
        unsafe {
            if !self.old_font.is_invalid() {
                SelectObject(self.dc, self.old_font);
            }
            if !self.old_bitmap.is_invalid() {
                SelectObject(self.dc, self.old_bitmap);
            }
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font);
            }
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap);
            }
            if !self.dc.is_invalid() {
                let _ = DeleteDC(self.dc);
            }
        }
    }
}

pub fn rasterize_text(text: &str, family: &str, size: u16) -> Result<TextMask, String> {
    if text.trim().is_empty()
        || text.chars().count() > 80
        || text.chars().any(char::is_control)
        || !(12..=48).contains(&size)
        || family.trim().is_empty()
        || family.encode_utf16().count() > 31
        || family.contains('\0')
    {
        return Err("Invalid single-line text, font or size".into());
    }
    let face: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
    let mut text: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let mut h = Handles {
            dc: CreateCompatibleDC(None),
            font: HFONT::default(),
            bitmap: HBITMAP::default(),
            old_font: HGDIOBJ::default(),
            old_bitmap: HGDIOBJ::default(),
        };
        if h.dc.is_invalid() {
            return Err("Cannot create text device context".into());
        }
        h.font = CreateFontW(
            -i32::from(size),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            u32::from(DEFAULT_CHARSET.0),
            u32::from(OUT_DEFAULT_PRECIS.0),
            u32::from(CLIP_DEFAULT_PRECIS.0),
            u32::from(ANTIALIASED_QUALITY.0),
            0,
            PCWSTR(face.as_ptr()),
        );
        if h.font.is_invalid() {
            return Err("Cannot create system font".into());
        }
        h.old_font = SelectObject(h.dc, h.font);
        if h.old_font.is_invalid() {
            return Err("Cannot select system font".into());
        }
        let mut actual = [0u16; 128];
        let n = GetTextFaceW(h.dc, Some(&mut actual));
        let actual = String::from_utf16_lossy(&actual[..(n.max(1) as usize - 1).min(actual.len())]);
        if !actual.eq_ignore_ascii_case(family) {
            return Err("Requested system font is unavailable; choose an installed font".into());
        }
        let mut bounds = RECT::default();
        let flags = DT_SINGLELINE | DT_NOPREFIX | DT_NOCLIP;
        if DrawTextW(h.dc, &mut text, &mut bounds, flags | DT_CALCRECT) == 0 {
            return Err("Cannot measure system text".into());
        }
        // Padding protects overhanging glyphs and the final antialiased column.
        let width = bounds.right.saturating_add(i32::from(size) / 2 + 4).max(1) as u32;
        let height = bounds.bottom.saturating_add(4).max(1) as u32;
        if width > 8192 || height > 256 || u64::from(width) * u64::from(height) > 4 * 1024 * 1024 {
            return Err("Text mask exceeds resource budget".into());
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        h.bitmap = CreateDIBSection(h.dc, &info, DIB_RGB_COLORS, &mut bits, None, 0)
            .map_err(|_| "Cannot allocate text mask")?;
        h.old_bitmap = SelectObject(h.dc, h.bitmap);
        if h.old_bitmap.is_invalid() || bits.is_null() {
            return Err("Cannot select text mask".into());
        }
        std::ptr::write_bytes(bits, 0, width as usize * height as usize * 4);
        SetTextColor(h.dc, COLORREF(0xffffff));
        SetBkColor(h.dc, COLORREF(0));
        SetBkMode(h.dc, TRANSPARENT);
        let mut target = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        if DrawTextW(h.dc, &mut text, &mut target, flags) == 0 {
            return Err("Cannot draw system text".into());
        }
        // GDI batches drawing; synchronize before reading DIB memory.
        let _ = GdiFlush();
        let bgra =
            std::slice::from_raw_parts(bits.cast::<u8>(), width as usize * height as usize * 4);
        let coverage: Vec<u8> = bgra
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0].max(p[1]).max(p[2]))
            .collect();
        if !coverage.iter().any(|&a| a > 0) {
            return Err("System text produced no visible glyphs".into());
        }
        Ok(TextMask {
            width,
            height,
            coverage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_system_font_masks_and_failed_font_requests_release_gdi_handles() {
        let mask = rasterize_text("Glint & <test>", "Segoe UI", 22).unwrap();
        assert_eq!(
            mask.coverage.len(),
            mask.width as usize * mask.height as usize
        );
        assert!(mask.coverage.iter().any(|&a| a > 0));
        let before = crate::gui_resource_counts().unwrap().0;
        for _ in 0..32 {
            assert!(rasterize_text("Glint", "Segoe UI", 48).is_ok());
            assert!(rasterize_text("Glint", "Glint Missing Font 913", 22).is_err());
        }
        assert!(crate::gui_resource_counts().unwrap().0 <= before + 1);
        assert!(rasterize_text("a\nb", "Segoe UI", 22).is_err());
        assert!(rasterize_text(&"a".repeat(81), "Segoe UI", 22).is_err());
    }
}
