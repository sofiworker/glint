//! A replacement is authorized for one observed regular file, never for a later retry.
use std::{
    fs::File,
    os::windows::ffi::OsStrExt,
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
};
use windows::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, HANDLE},
    Storage::FileSystem::*,
};

/// Publish a completed image in the same directory without replacing any target.
/// A collision returns false so callers may try their next numbered name.
pub fn publish_new_image(pending: &Path, destination: &Path) -> Result<bool, String> {
    if pending == destination || pending.parent() != destination.parent() {
        return Err("Image publication requires distinct names in one directory".into());
    }
    let source: Vec<u16> = pending.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    if source[..source.len() - 1].contains(&0) || target[..target.len() - 1].contains(&0) {
        return Err("Invalid image publication path".into());
    }
    match unsafe {
        MoveFileExW(
            windows::core::PCWSTR(source.as_ptr()),
            windows::core::PCWSTR(target.as_ptr()),
            MOVEFILE_WRITE_THROUGH,
        )
    } {
        Ok(()) => Ok(true),
        Err(error)
            if [ERROR_FILE_EXISTS.0, ERROR_ALREADY_EXISTS.0]
                .iter()
                .any(|code| error.code() == windows::core::HRESULT::from_win32(*code)) =>
        {
            Ok(false)
        }
        Err(error) => Err(error.to_string()),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    volume: u32,
    index: u64,
    size: u64,
    written: u64,
}
fn stamp(path: &Path) -> Result<Stamp, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Replacement requires a regular file".into());
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(|e| e.to_string())?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err("Replacement of a reparse point is not supported".into());
    }
    Ok(Stamp {
        volume: info.dwVolumeSerialNumber,
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        size: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
        written: (u64::from(info.ftLastWriteTime.dwHighDateTime) << 32)
            | u64::from(info.ftLastWriteTime.dwLowDateTime),
    })
}

#[derive(Debug)]
pub struct ImageReplacement {
    path: PathBuf,
    observed: Stamp,
}
impl ImageReplacement {
    /// Snapshot before asking the user. This alone is not permission to replace.
    pub fn observe(path: &Path) -> Result<Self, String> {
        Ok(Self {
            path: path.to_owned(),
            observed: stamp(path)?,
        })
    }
    /// Caller must have received explicit approval for this observation.
    /// The pending image is already fully encoded and synced in the same directory.
    pub fn publish(&self, pending: &Path, destination: &Path) -> Result<(), String> {
        if destination != self.path
            || pending.parent() != destination.parent()
            || stamp(destination)? != self.observed
        {
            return Err("Destination changed; choose and confirm it again".into());
        }
        let source: Vec<u16> = pending.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // ReplaceFileW merges old named streams into the new file. Image output
        // must publish only the newly encoded file, including its stream set.
        unsafe {
            MoveFileExW(
                windows::core::PCWSTR(source.as_ptr()),
                windows::core::PCWSTR(target.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|e| e.to_string())
    }
}

pub struct ImageDestination {
    pub path: PathBuf,
    pub replacement: Option<ImageReplacement>,
}

fn authorize(
    path: PathBuf,
    confirm: bool,
    approve: impl FnOnce(&Path) -> Result<bool, String>,
) -> Result<Option<ImageDestination>, String> {
    let replacement = if confirm && path.try_exists().map_err(|e| e.to_string())? {
        let observed = ImageReplacement::observe(&path)?;
        if !approve(&path)? {
            return Ok(None);
        }
        Some(observed)
    } else {
        None
    };
    Ok(Some(ImageDestination { path, replacement }))
}

pub struct ReplacementPrompt<'a> {
    pub title: &'a str,
    pub question: &'a str,
    pub hint: &'a str,
}

/// # Safety
/// `owner` is a live HWND owned by this UI thread throughout both modal dialogs.
pub unsafe fn choose_image_destination(
    owner: *mut std::ffi::c_void,
    filename: &str,
    directory: &Path,
    jpeg: bool,
    confirm: bool,
    prompt: ReplacementPrompt<'_>,
) -> Result<Option<ImageDestination>, String> {
    let Some(path) = (unsafe { crate::choose_image_path(owner, filename, directory, jpeg) })?
    else {
        return Ok(None);
    };
    authorize(path, confirm, |path| unsafe { ask(owner, path, &prompt) })
}

unsafe fn ask(
    owner: *mut std::ffi::c_void,
    path: &Path,
    prompt: &ReplacementPrompt<'_>,
) -> Result<bool, String> {
    use windows::{
        Win32::{Foundation::HWND, UI::WindowsAndMessaging::*},
        core::PCWSTR,
    };
    if [prompt.title, prompt.question, prompt.hint]
        .iter()
        .any(|s| s.contains('\0'))
    {
        return Err("Invalid confirmation text".into());
    }
    let message = format!(
        "{}\n\n{}\n\n{}",
        prompt.question,
        path.display(),
        prompt.hint
    );
    let message: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = prompt.title.encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        MessageBoxW(
            HWND(owner),
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OKCANCEL | MB_ICONWARNING | MB_DEFBUTTON2,
        )
    };
    if result.0 == 0 {
        Err(windows::core::Error::from_win32().to_string())
    } else {
        Ok(result == IDOK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::*,
    };
    thread_local! { static ANSWER: std::cell::Cell<Option<(HWND, usize)>> = const { std::cell::Cell::new(None) }; }
    unsafe extern "system" fn answer_timer(_: HWND, _: u32, _: usize, _: u32) {
        ANSWER.with(|slot| {
            if let Some((owner, answer)) = slot.get() {
                let popup = unsafe { GetLastActivePopup(owner) };
                if popup != owner && unsafe { IsWindowVisible(popup) }.as_bool() {
                    let _ = unsafe { PostMessageW(popup, WM_COMMAND, WPARAM(answer), LPARAM(0)) };
                }
            }
        });
    }
    #[test]
    #[ignore = "Opens two real owned Windows confirmation dialogs and answers through targeted messages"]
    fn actual_owned_confirmation_cancel_and_accept() {
        use windows::{Win32::Foundation::HINSTANCE, core::w};
        unsafe {
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Glint replacement probe owner"),
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
            for (answer, expected, english) in [
                (IDCANCEL.0 as usize, false, true),
                (IDOK.0 as usize, true, false),
            ] {
                ANSWER.with(|slot| slot.set(Some((owner, answer))));
                let timer = SetTimer(None, 0, 150, Some(answer_timer));
                assert_ne!(timer, 0);
                let prompt = if english {
                    ReplacementPrompt {
                        title: "Confirm replacement",
                        question: "Replace this existing file?",
                        hint: "Cancel keeps the current image edits.",
                    }
                } else {
                    ReplacementPrompt {
                        title: "确认覆盖",
                        question: "是否覆盖此已有文件？",
                        hint: "取消会保留当前图片编辑。",
                    }
                };
                let result = ask(
                    owner.0,
                    Path::new("C:/Glint-validation-existing.png"),
                    &prompt,
                );
                let _ = KillTimer(None, timer);
                ANSWER.with(|slot| slot.set(None));
                assert_eq!(result.unwrap(), expected);
            }
            let _ = DestroyWindow(owner);
        }
    }
    #[test]
    fn cancellation_and_changed_file_never_publish_and_success_replaces_without_backup() {
        let dir = std::env::temp_dir().join(format!(
            "glint-replace-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let target = dir.join("old.png");
        let pending = dir.join("pending.tmp");
        std::fs::write(&target, b"old").unwrap();
        let old_stream = dir.join("old.png:GlintValidation");
        std::fs::write(&old_stream, b"old image private stream").unwrap();
        std::fs::write(&pending, b"new image").unwrap();
        assert!(
            authorize(target.clone(), true, |_| Err(
                "confirmation unavailable".into()
            ))
            .is_err()
        );
        assert!(
            authorize(target.clone(), true, |_| Ok(false))
                .unwrap()
                .is_none()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        let approved = authorize(target.clone(), true, |_| Ok(true))
            .unwrap()
            .unwrap()
            .replacement
            .unwrap();
        std::fs::write(&target, b"externally changed").unwrap();
        assert!(approved.publish(&pending, &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"externally changed");
        let approved = authorize(target.clone(), true, |_| Ok(true))
            .unwrap()
            .unwrap()
            .replacement
            .unwrap();
        assert!(approved.publish(&pending, &dir.join("wrong.png")).is_err());
        approved.publish(&pending, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new image");
        assert!(
            !old_stream.exists(),
            "Replacement retained an old image alternate stream"
        );
        assert!(!pending.exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        assert!(authorize(dir.clone(), true, |_| panic!()).is_err());
        assert!(
            authorize(dir.join("new.png"), true, |_| panic!())
                .unwrap()
                .unwrap()
                .replacement
                .is_none()
        );
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
