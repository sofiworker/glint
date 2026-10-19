use std::{
    ffi::{OsString, c_void},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use windows::{
    Win32::{Foundation::HWND, UI::Controls::Dialogs::*},
    core::{PWSTR, w},
};

/// Called on the owner's UI thread. The borrowed owner must remain alive for the dialog.
/// Cancellation is Ok(None); unlike an output error it does not discard the capture session.
/// # Safety
/// `owner` must be a valid HWND belonging to this thread for the duration of this call.
pub unsafe fn choose_image_path(
    owner: *mut c_void,
    filename: &str,
    directory: &Path,
    jpeg: bool,
) -> Result<Option<PathBuf>, String> {
    let mut buffer = vec![0u16; 32768];
    let initial: Vec<_> = filename.encode_utf16().collect();
    if initial.contains(&0) || initial.len() >= buffer.len() {
        return Err("Invalid save filename".into());
    }
    buffer[..initial.len()].copy_from_slice(&initial);
    let directory: Vec<_> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner),
        lpstrFilter: w!("PNG (*.png)\0*.png\0JPEG (*.jpg; *.jpeg)\0*.jpg;*.jpeg\0\0"),
        nFilterIndex: if jpeg { 2 } else { 1 },
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrInitialDir: if directory.len() == 1 {
            windows::core::PCWSTR::null()
        } else {
            windows::core::PCWSTR(directory.as_ptr())
        },
        // The app's selected policy handles numbering/rejection; replacement uses a
        // separate explicit confirmation after observing the existing file.
        Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_HIDEREADONLY,
        ..Default::default()
    };
    if unsafe { GetSaveFileNameW(&mut dialog) }.as_bool() {
        let length = buffer
            .iter()
            .position(|c| *c == 0)
            .ok_or("Invalid save path")?;
        let mut path = PathBuf::from(OsString::from_wide(&buffer[..length]));
        if path.extension().is_none() {
            path.set_extension(if dialog.nFilterIndex == 2 {
                "jpg"
            } else {
                "png"
            });
        }
        Ok(Some(path))
    } else {
        let error = unsafe { CommDlgExtendedError() }.0;
        if error == 0 {
            Ok(None)
        } else {
            Err(format!("save_dialog: {error:#x}"))
        }
    }
}
