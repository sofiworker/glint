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
            w!("PNG / JPEG\0*.png;*.jpg;*.jpeg\0\0")
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
