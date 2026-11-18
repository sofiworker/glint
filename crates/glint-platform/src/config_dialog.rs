use std::{
    ffi::{OsString, c_void},
    os::windows::ffi::OsStringExt,
    path::PathBuf,
};
use windows::{
    Win32::{Foundation::HWND, UI::Controls::Dialogs::*},
    core::{PWSTR, w},
};

/// Native file picker, on the owner's UI thread. No file is read or written here.
/// # Safety
/// `owner` must be a live HWND belonging to this thread throughout the dialog.
pub unsafe fn choose_config_path(
    owner: *mut c_void,
    export: bool,
) -> Result<Option<PathBuf>, String> {
    unsafe { choose_path(owner, export, false) }
}

/// Select a local PNG/JPEG on the owner's UI thread, without reading the file.
/// # Safety
/// `owner` must be a live HWND belonging to this thread throughout the dialog.
pub unsafe fn choose_local_image_path(owner: *mut c_void) -> Result<Option<PathBuf>, String> {
    unsafe { choose_path(owner, false, true) }
}

/// Workbench input, or a new MP4/GIF output path. File I/O runs on media workers.
/// # Safety
/// `owner` must be a live HWND on this thread until the picker returns.
pub unsafe fn choose_editor_file(
    owner: *mut c_void,
    save: bool,
) -> Result<Option<PathBuf>, String> {
    let mut buffer = vec![0u16; 32768];
    if save {
        let name: Vec<_> = "Glint-edited.mp4".encode_utf16().collect();
        buffer[..name.len()].copy_from_slice(&name);
    }
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner),
        lpstrFilter: if save {
            w!("MP4 (*.mp4)\0*.mp4\0GIF (*.gif)\0*.gif\0\0")
        } else {
            w!(
                "Images / Videos\0*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.tif;*.tiff;*.ico;*.mp4;*.mov;*.m4v;*.wmv;*.gif\0\0"
            )
        },
        nFilterIndex: 1,
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        Flags: OFN_EXPLORER
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_HIDEREADONLY
            | if save {
                OPEN_FILENAME_FLAGS(0)
            } else {
                OFN_FILEMUSTEXIST
            },
        ..Default::default()
    };
    let ok = if save {
        unsafe { GetSaveFileNameW(&mut dialog) }
    } else {
        unsafe { GetOpenFileNameW(&mut dialog) }
    };
    if !ok.as_bool() {
        let e = unsafe { CommDlgExtendedError() }.0;
        return if e == 0 {
            Ok(None)
        } else {
            Err(format!("editor_dialog: {e:#x}"))
        };
    }
    let length = buffer
        .iter()
        .position(|c| *c == 0)
        .ok_or("Invalid editor path")?;
    let mut path = PathBuf::from(OsString::from_wide(&buffer[..length]));
    if save && path.extension().is_none() {
        path.set_extension(if dialog.nFilterIndex == 2 {
            "gif"
        } else {
            "mp4"
        });
    }
    Ok(Some(path))
}

/// Choose a path for exactly the format selected in the native export panel.
/// # Safety
/// `owner` must be a live HWND on this UI thread throughout the dialog.
pub unsafe fn choose_editor_export(
    owner: *mut c_void,
    name: &str,
    extension: &str,
) -> Result<Option<PathBuf>, String> {
    if ![
        "png", "jpg", "webp", "bmp", "tiff", "mp4", "gif", "mov", "mkv", "webm",
    ]
    .contains(&extension)
        || name.contains('\0')
    {
        return Err("Invalid export name or format".into());
    }
    let filename = std::path::Path::new(name).with_extension(extension);
    let text: Vec<u16> = filename
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .collect();
    if text.len() >= 32768 {
        return Err("Export filename too long".into());
    }
    let mut buffer = vec![0u16; 32768];
    buffer[..text.len()].copy_from_slice(&text);
    let filter: Vec<u16> = format!(
        "{} (*.{} )\0*.{}\0\0",
        extension.to_ascii_uppercase(),
        extension,
        extension
    )
    .encode_utf16()
    .collect();
    let defext: Vec<u16> = extension.encode_utf16().chain(Some(0)).collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner),
        lpstrFilter: windows::core::PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrDefExt: windows::core::PCWSTR(defext.as_ptr()),
        Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_HIDEREADONLY,
        ..Default::default()
    };
    if !unsafe { GetSaveFileNameW(&mut dialog) }.as_bool() {
        let error = unsafe { CommDlgExtendedError() }.0;
        return if error == 0 {
            Ok(None)
        } else {
            Err(format!("export_dialog: {error:#x}"))
        };
    }
    let len = buffer
        .iter()
        .position(|v| *v == 0)
        .ok_or("Invalid export path")?;
    let path = PathBuf::from(OsString::from_wide(&buffer[..len]));
    let suffix = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if suffix != extension
        && !(extension == "jpg" && suffix == "jpeg")
        && !(extension == "tiff" && suffix == "tif")
    {
        return Err("Filename extension must match the selected export format".into());
    }
    Ok(Some(path))
}

unsafe fn choose_path(
    owner: *mut c_void,
    export: bool,
    image: bool,
) -> Result<Option<PathBuf>, String> {
    let mut buffer = vec![0u16; 32768];
    if export {
        let name: Vec<_> = "Glint-settings.toml".encode_utf16().collect();
        buffer[..name.len()].copy_from_slice(&name);
    }
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner),
        lpstrFilter: if image {
            w!("Images\0*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.tif;*.tiff;*.ico\0\0")
        } else {
            w!("TOML (*.toml)\0*.toml\0\0")
        },
        nFilterIndex: 1,
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrDefExt: if image { w!("png") } else { w!("toml") },
        Flags: OFN_EXPLORER
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_HIDEREADONLY
            | if export {
                OPEN_FILENAME_FLAGS(0)
            } else {
                OFN_FILEMUSTEXIST
            },
        ..Default::default()
    };
    let accepted = if export {
        unsafe { GetSaveFileNameW(&mut dialog) }
    } else {
        unsafe { GetOpenFileNameW(&mut dialog) }
    };
    if accepted.as_bool() {
        let length = buffer
            .iter()
            .position(|c| *c == 0)
            .ok_or("Invalid configuration path")?;
        Ok(Some(PathBuf::from(OsString::from_wide(&buffer[..length]))))
    } else {
        let error = unsafe { CommDlgExtendedError() }.0;
        if error == 0 {
            Ok(None)
        } else {
            Err(format!("config_dialog: {error:#x}"))
        }
    }
}
