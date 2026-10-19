use std::path::PathBuf;
use windows::{
    Win32::{
        Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME},
        System::{
            Com::CoTaskMemFree,
            Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        },
        UI::Shell::{
            FOLDERID_Pictures, FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
        },
    },
    core::{PWSTR, w},
};

pub fn configuration_directory() -> Result<PathBuf, String> {
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None) }
        .map_err(|error| format!("SHGetKnownFolderPath: {error}"))?;
    let text = unsafe { path.to_string() };
    unsafe { CoTaskMemFree(Some(path.0.cast())) };
    text.map(|path| PathBuf::from(path).join("Glint"))
        .map_err(|_| "Invalid configuration directory encoding".into())
}

pub fn pictures_directory() -> Result<PathBuf, String> {
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_Pictures, KF_FLAG_DEFAULT, None) }
        .map_err(|error| format!("SHGetKnownFolderPath: {error}"))?;
    let text = unsafe { path.to_string() };
    unsafe { CoTaskMemFree(Some(path.0.cast())) };
    text.map(PathBuf::from)
        .map_err(|_| "Invalid Pictures directory encoding".into())
}

pub fn local_output_timestamp() -> (String, String) {
    let time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (
        format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay),
        format!("{:02}{:02}{:02}", time.wHour, time.wMinute, time.wSecond),
    )
}

pub fn preferred_ui_languages() -> Result<Vec<String>, String> {
    let (mut count, mut size) = (0, 0);
    unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, PWSTR::null(), &mut size) }
        .map_err(|error| format!("GetUserPreferredUILanguages: {error}"))?;
    if size == 0 || size > 32768 {
        return Err("Invalid preferred language buffer size".into());
    }
    let mut buffer = vec![0u16; size as usize];
    unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    }
    .map_err(|error| format!("GetUserPreferredUILanguages: {error}"))?;
    buffer.truncate(size as usize);
    buffer
        .split(|c| *c == 0)
        .filter(|tag| !tag.is_empty())
        .map(|tag| {
            String::from_utf16(tag).map_err(|_| "Invalid preferred language encoding".into())
        })
        .collect()
}

/// None is a real registry-query failure; the caller may present a light preview fallback.
pub fn system_dark_theme() -> Option<bool> {
    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    Some(value == 0)
}

pub fn show_startup_error(message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let body = message
        .chars()
        .take(1200)
        .collect::<String>()
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        MessageBoxW(
            None,
            windows::core::PCWSTR(body.as_ptr()),
            w!("Glint · 启动失败 / Startup failed"),
            MB_ICONERROR | MB_OK,
        );
    }
}
