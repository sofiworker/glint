//! Installed GDI families; no font files or text contents are read or persisted.
use std::collections::BTreeMap;
use windows::Win32::{Foundation::LPARAM, Graphics::Gdi::*};

#[derive(Default)]
struct Families {
    names: BTreeMap<String, String>,
    overflow: bool,
}
unsafe extern "system" fn collect(
    font: *const LOGFONTW,
    _: *const TEXTMETRICW,
    _: u32,
    data: LPARAM,
) -> i32 {
    // EnumFontFamiliesExW is synchronous; both pointers are valid for this callback.
    let family = unsafe { &*font };
    let state = unsafe { &mut *(data.0 as *mut Families) };
    let end = family
        .lfFaceName
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(family.lfFaceName.len());
    if let Ok(name) = String::from_utf16(&family.lfFaceName[..end])
        && !name.is_empty()
        && !name.starts_with('@')
        && !name.chars().any(char::is_control)
    {
        state.names.entry(name.to_lowercase()).or_insert(name);
        if state.names.len() > 4096 {
            state.overflow = true;
            return 0;
        }
    }
    1
}
pub fn installed_font_families() -> Result<Vec<String>, String> {
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return Err("CreateCompatibleDC: font enumeration failed".into());
        }
        let filter = LOGFONTW {
            lfCharSet: DEFAULT_CHARSET,
            ..Default::default()
        };
        let mut families = Families::default();
        EnumFontFamiliesExW(
            dc,
            &filter,
            Some(collect),
            LPARAM(&mut families as *mut Families as isize),
            0,
        );
        let _ = DeleteDC(dc);
        if families.overflow {
            return Err("Font enumeration exceeds 4096 families".into());
        }
        if families.names.is_empty() {
            return Err("No installed GDI font families returned".into());
        }
        Ok(families.names.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn actual_families_are_unique_bounded_and_can_be_rasterized() {
        let fonts = super::installed_font_families().unwrap();
        assert!(!fonts.is_empty() && fonts.len() <= 4096);
        let mut previous = String::new();
        for font in &fonts {
            assert!(!font.starts_with('@') && font.encode_utf16().count() <= 31);
            let key = font.to_lowercase();
            assert!(key > previous);
            previous = key;
        }
        assert!(crate::rasterize_text("Glint", &fonts[0], 22).is_ok());
    }
}

/// The user's Windows message/UI font, independent of annotation preferences.
pub fn system_ui_font_family() -> Result<String, String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        SystemParametersInfoW,
    };
    let size = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: size,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            size,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .map_err(|e| format!("System UI font: {e}"))?;
    }
    let face = metrics.lfMessageFont.lfFaceName;
    let end = face.iter().position(|&c| c == 0).unwrap_or(face.len());
    let family = String::from_utf16(&face[..end]).map_err(|e| e.to_string())?;
    if family.is_empty() {
        return Err("System UI font is empty".into());
    }
    Ok(family)
}
