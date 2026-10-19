//! Native filesystem folder picker. Selection performs no application file writes.
use std::{
    ffi::{OsString, c_void},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::*,
        UI::Shell::*,
    },
    core::PCWSTR,
};

struct Apartment;
impl Apartment {
    fn enter() -> Result<Self, String> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
            .ok()
            .map_err(|e| format!("CoInitializeEx: {e}"))?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct TaskString(windows::core::PWSTR);
impl Drop for TaskString {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0.0.cast()));
        }
    }
}

/// UI-thread control for dismissing an owned modal picker during settings close.
#[derive(Clone)]
pub struct FolderPickerControl {
    dialog: IFileOpenDialog,
    _apartment: std::rc::Rc<Apartment>,
}
impl FolderPickerControl {
    pub fn cancel(&self) -> Result<(), String> {
        unsafe {
            self.dialog
                .Close(windows::core::HRESULT::from_win32(ERROR_CANCELLED.0))
        }
        .map_err(|e| e.to_string())
    }
}

fn prepare(initial: &Path, title: &str) -> Result<IFileOpenDialog, String> {
    if initial.as_os_str().encode_wide().any(|c| c == 0) || title.contains('\0') {
        return Err("Invalid folder dialog input".into());
    }
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("FileOpenDialog: {e}"))?;
        dialog
            .SetOptions(
                dialog.GetOptions().map_err(|e| e.to_string())?
                    | FOS_PICKFOLDERS
                    | FOS_FORCEFILESYSTEM
                    | FOS_PATHMUSTEXIST
                    | FOS_NOCHANGEDIR
                    | FOS_DONTADDTORECENT,
            )
            .map_err(|e| e.to_string())?;
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        dialog
            .SetTitle(PCWSTR(title.as_ptr()))
            .map_err(|e| e.to_string())?;
        if !initial.as_os_str().is_empty() {
            let initial: Vec<u16> = initial.as_os_str().encode_wide().chain(Some(0)).collect();
            // A missing/invalid initial folder is only a hint; the OS default remains usable.
            if let Ok(item) =
                SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(initial.as_ptr()), None)
            {
                let _ = dialog.SetFolder(&item);
            }
        }
        Ok(dialog)
    }
}

/// On the owner's UI thread. The caller must release application-state borrows
/// before Show: the native modal message loop can dispatch application callbacks.
/// # Safety
/// `owner` must be a live HWND owned by this thread throughout this call.
pub unsafe fn choose_output_directory(
    owner: *mut c_void,
    initial: &Path,
    title: &str,
    opened: impl FnOnce(FolderPickerControl),
) -> Result<Option<PathBuf>, String> {
    let apartment = std::rc::Rc::new(Apartment::enter()?);
    let dialog = prepare(initial, title)?;
    opened(FolderPickerControl {
        dialog: dialog.clone(),
        _apartment: apartment.clone(),
    });
    unsafe {
        if let Err(error) = dialog.Show(HWND(owner)) {
            return if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                Ok(None)
            } else {
                Err(format!("Folder dialog: {error}"))
            };
        }
        let item = dialog.GetResult().map_err(|e| e.to_string())?;
        let name = TaskString(
            item.GetDisplayName(SIGDN_FILESYSPATH)
                .map_err(|e| e.to_string())?,
        );
        let path = PathBuf::from(OsString::from_wide(name.0.as_wide()));
        if !path.is_absolute() {
            return Err("Folder picker returned a non-absolute filesystem path".into());
        }
        Ok(Some(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    thread_local! { static CANCEL: RefCell<Option<FolderPickerControl>> = const { RefCell::new(None) }; }
    unsafe extern "system" fn cancel_timer(_: HWND, _: u32, _: usize, _: u32) {
        let control = CANCEL.with(|slot| slot.borrow().clone());
        if let Some(control) = control {
            let _ = control.cancel();
        }
    }
    #[test]
    #[ignore = "Temporarily opens a real native folder dialog and cancels it through its COM control"]
    fn real_folder_dialog_cancel_keeps_working_directory() {
        use windows::{
            Win32::{Foundation::HINSTANCE, UI::WindowsAndMessaging::*},
            core::w,
        };
        let before = std::env::current_dir().unwrap();
        unsafe {
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Glint folder owner"),
                WINDOW_STYLE::default(),
                0,
                0,
                100,
                100,
                None,
                None,
                HINSTANCE::default(),
                None,
            )
            .unwrap();
            let timer = SetTimer(None, 0, 150, Some(cancel_timer));
            assert_ne!(timer, 0);
            let result = choose_output_directory(
                owner.0,
                &before,
                "Glint folder cancellation probe",
                |control| CANCEL.with(|slot| *slot.borrow_mut() = Some(control)),
            );
            let _ = KillTimer(None, timer);
            CANCEL.with(|slot| slot.borrow_mut().take());
            let _ = DestroyWindow(owner);
            assert_eq!(result.unwrap(), None);
        }
        assert_eq!(std::env::current_dir().unwrap(), before);
    }
    #[test]
    fn real_com_folder_dialog_options_initial_folder_and_cleanup() {
        // No Show here: verifies actual COM construction/configuration without a visible dialog.
        let before = std::env::current_dir().unwrap();
        let _apartment = Apartment::enter().unwrap();
        let dialog = prepare(&before, "Glint folder probe").unwrap();
        unsafe {
            let options = dialog.GetOptions().unwrap();
            for flag in [
                FOS_PICKFOLDERS,
                FOS_FORCEFILESYSTEM,
                FOS_PATHMUSTEXIST,
                FOS_NOCHANGEDIR,
                FOS_DONTADDTORECENT,
            ] {
                assert!(options.contains(flag));
            }
            let folder = dialog.GetFolder().unwrap();
            let name = TaskString(folder.GetDisplayName(SIGDN_FILESYSPATH).unwrap());
            assert_eq!(PathBuf::from(OsString::from_wide(name.0.as_wide())), before);
        }
        assert_eq!(std::env::current_dir().unwrap(), before);
        assert!(prepare(Path::new(""), "bad\0title").is_err());
        let missing = before.join(".local/nonexistent-folder-picker-hint");
        assert!(!missing.exists());
        assert!(prepare(&missing, "Glint missing hint").is_ok());
        assert!(!missing.exists());
    }
}
