//! Explicit per-user login startup. Never writes merely because the application starts.
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, System::Registry::*},
    core::{PCWSTR, w},
};

pub struct StartupRegistration {
    key: HKEY,
    name: Vec<u16>,
    command: Vec<u8>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StartupSnapshot(Option<(REG_VALUE_TYPE, Vec<u8>)>);
impl StartupRegistration {
    pub fn current_user(
        executable: &Path,
        config_directory: Option<&Path>,
    ) -> Result<Self, String> {
        Self::open(
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            "Glint",
            executable,
            config_directory,
        )
    }
    fn open(
        key_path: PCWSTR,
        name: &str,
        executable: &Path,
        directory: Option<&Path>,
    ) -> Result<Self, String> {
        let mut key = HKEY::default();
        // Open only; constructing the coordinator does not create a registry key or startup value.
        unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                key_path,
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut key,
            )
        }
        .ok()
        .map_err(|e| format!("startup.open: {e}"))?;
        let command = (|| {
            let mut command = quoted(executable)?;
            command.extend(" --background".encode_utf16());
            if let Some(directory) = directory {
                command.extend(" --config-dir ".encode_utf16());
                command.extend(quoted(directory)?);
            }
            // Windows documents Run commands as at most 260 characters. Refuse truncation.
            if command.len() > 260 {
                return Err("startup.command_too_long".into());
            }
            command.push(0);
            Ok::<_, String>(command.into_iter().flat_map(u16::to_le_bytes).collect())
        })();
        match command {
            Ok(command) => Ok(Self {
                key,
                name: name.encode_utf16().chain(Some(0)).collect(),
                command,
            }),
            Err(error) => {
                unsafe {
                    let _ = RegCloseKey(key);
                }
                Err(error)
            }
        }
    }
    pub fn snapshot(&self) -> Result<StartupSnapshot, String> {
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0;
        let result = unsafe {
            RegQueryValueExW(
                self.key,
                PCWSTR(self.name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(StartupSnapshot(None));
        }
        result.ok().map_err(|e| format!("startup.read: {e}"))?;
        if size > 65536 {
            return Err("startup.value_too_large".into());
        }
        let mut bytes = vec![0; size as usize];
        unsafe {
            RegQueryValueExW(
                self.key,
                PCWSTR(self.name.as_ptr()),
                None,
                Some(&mut kind),
                Some(bytes.as_mut_ptr()),
                Some(&mut size),
            )
        }
        .ok()
        .map_err(|e| format!("startup.read: {e}"))?;
        bytes.truncate(size as usize);
        Ok(StartupSnapshot(Some((kind, bytes))))
    }
    pub fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        self.restore(&StartupSnapshot(
            enabled.then(|| (REG_SZ, self.command.clone())),
        ))
    }
    pub fn restore(&self, snapshot: &StartupSnapshot) -> Result<(), String> {
        let result = match &snapshot.0 {
            Some((kind, bytes)) => unsafe {
                RegSetValueExW(self.key, PCWSTR(self.name.as_ptr()), 0, *kind, Some(bytes))
            },
            None => unsafe { RegDeleteValueW(self.key, PCWSTR(self.name.as_ptr())) },
        };
        if snapshot.0.is_none() && result == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        result.ok().map_err(|e| format!("startup.write: {e}"))
    }
}
impl Drop for StartupRegistration {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.key);
        }
    }
}
fn quoted(path: &Path) -> Result<Vec<u16>, String> {
    let path: Vec<u16> = path.as_os_str().encode_wide().collect();
    if path.is_empty() || path.iter().any(|c| *c == 0 || *c == 34) {
        return Err("startup.invalid_path".into());
    }
    // Paths cannot contain quotes. Double trailing backslashes before the closing command-line quote.
    let trailing = path.iter().rev().take_while(|c| **c == 92).count();
    Ok(std::iter::once(34)
        .chain(path)
        .chain(std::iter::repeat_n(92, trailing))
        .chain(Some(34))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_value_roundtrip_and_exact_rollback() {
        // An inert HKCU test key, never the Windows Run key.
        let path: Vec<u16> = format!("Software\\GlintTests\\Startup-{}", std::process::id())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut key = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_ALL_ACCESS,
                None,
                &mut key,
                None,
            )
        }
        .ok()
        .unwrap();
        unsafe { RegCloseKey(key) }.ok().unwrap();
        {
            let registration = StartupRegistration::open(
                PCWSTR(path.as_ptr()),
                "Glint",
                Path::new("C:\\Glint folder\\glint.exe"),
                Some(Path::new("C:\\config folder\\")),
            )
            .unwrap();
            let initial = registration.snapshot().unwrap();
            assert_eq!(initial, StartupSnapshot(None));
            registration.set_enabled(true).unwrap();
            let enabled = registration.snapshot().unwrap();
            let text = String::from_utf16(
                &enabled
                    .0
                    .as_ref()
                    .unwrap()
                    .1
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            assert_eq!(
                text,
                "\"C:\\Glint folder\\glint.exe\" --background --config-dir \"C:\\config folder\\\\\"\0"
            );
            registration.set_enabled(false).unwrap();
            assert_eq!(registration.snapshot().unwrap(), initial);
            registration.restore(&enabled).unwrap();
            assert_eq!(registration.snapshot().unwrap(), enabled);
            registration.restore(&initial).unwrap();
        }
        unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr())) }
            .ok()
            .unwrap();
    }
}
