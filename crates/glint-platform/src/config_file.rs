//! Bounded configuration-file I/O. Parsing and version policy belong to glint-core / app.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Read an external document without repair, backup or directory creation.
pub fn read_document(path: &Path, limit: usize) -> Result<Vec<u8>, StoreError> {
    read_bounded(path, limit)?.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound).into())
}

/// Publish a complete external document, without replacing any existing target.
pub fn export_new(path: &Path, bytes: &[u8], limit: usize) -> Result<(), StoreError> {
    if bytes.len() > limit {
        return Err(StoreError::TooLarge);
    }
    let pending = PendingFile::write(path, bytes)?;
    replace(&pending.path, path, false)?;
    Ok(())
}

/// Atomically replace an application-owned asset; no recovery copy retains old pixels.
pub fn replace_asset(path: &Path, bytes: &[u8], limit: usize) -> Result<(), StoreError> {
    if bytes.len() > limit {
        return Err(StoreError::TooLarge);
    }
    let pending = PendingFile::write(path, bytes)?;
    replace(&pending.path, path, path.try_exists()?)?;
    Ok(())
}

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    TooLarge,
    Changed,
}
impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "config.io: {:?}", error.kind()),
            Self::TooLarge => f.write_str("config.too_large"),
            Self::Changed => f.write_str("config.changed_externally"),
        }
    }
}
impl std::error::Error for StoreError {}

pub struct ConfigFile {
    path: PathBuf,
    baseline: Option<Vec<u8>>,
    limit: usize,
}
impl ConfigFile {
    /// Reads once. Does not create a file or directory, repair damage or write defaults.
    pub fn open(path: impl Into<PathBuf>, limit: usize) -> Result<Self, StoreError> {
        let path = path.into();
        let baseline = read_bounded(&path, limit)?;
        Ok(Self {
            path,
            baseline,
            limit,
        })
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        self.baseline.as_deref()
    }
    pub fn backup_bytes(&self) -> Result<Option<Vec<u8>>, StoreError> {
        read_bounded(&suffix(&self.path, ".bak"), self.limit)
    }
    /// Caller must validate the schema and apply/rollback system settings before accepting a save.
    /// Set backup_current=false only when replacing damaged bytes already preserved separately.
    pub fn save(&mut self, bytes: &[u8], backup_current: bool) -> Result<(), StoreError> {
        if bytes.len() > self.limit {
            return Err(StoreError::TooLarge);
        }
        self.check_unchanged()?;
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let pending = PendingFile::write(&self.path, bytes)?;
        if backup_current && let Some(current) = &self.baseline {
            let backup = suffix(&self.path, ".bak");
            let pending_backup = PendingFile::write(&backup, current)?;
            replace(&pending_backup.path, &backup, backup.try_exists()?)?;
        }
        self.check_unchanged()?;
        replace(&pending.path, &self.path, self.baseline.is_some())?;
        // Update the in-memory baseline only after the OS reports successful replacement.
        self.baseline = Some(bytes.to_vec());
        Ok(())
    }
    /// Preserve malformed original bytes before offering recovery; never overwrite a prior copy.
    pub fn preserve_current(&self) -> Result<Option<PathBuf>, StoreError> {
        self.check_unchanged()?;
        let Some(bytes) = &self.baseline else {
            return Ok(None);
        };
        let mut saved = PendingFile::write(&suffix(&self.path, ".corrupt"), bytes)?;
        let path = saved.path.clone();
        // This is a deliberate retained copy, not a save of the active preferences.
        saved.remove_on_drop = false;
        Ok(Some(path))
    }
    fn check_unchanged(&self) -> Result<(), StoreError> {
        if read_bounded(&self.path, self.limit)? != self.baseline {
            Err(StoreError::Changed)
        } else {
            Ok(())
        }
    }
}

fn read_bounded(path: &Path, limit: usize) -> Result<Option<Vec<u8>>, StoreError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if file.metadata()?.len() > limit as u64 {
        return Err(StoreError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(StoreError::TooLarge);
    }
    Ok(Some(bytes))
}
fn suffix(path: &Path, value: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(value);
    PathBuf::from(name)
}
struct PendingFile {
    path: PathBuf,
    remove_on_drop: bool,
}
impl PendingFile {
    fn write(target: &Path, bytes: &[u8]) -> Result<Self, StoreError> {
        for _ in 0..32 {
            let path = suffix(
                target,
                &format!(
                    ".tmp-{}-{}",
                    std::process::id(),
                    NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
                ),
            );
            let file = OpenOptions::new().write(true).create_new(true).open(&path);
            match file {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
                Ok(mut file) => {
                    let pending = Self {
                        path,
                        remove_on_drop: true,
                    };
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    drop(file);
                    return Ok(pending);
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "No unused temporary configuration filename",
        )
        .into())
    }
}
impl Drop for PendingFile {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(all(windows, feature = "windows-bindings"))]
fn replace(from: &Path, to: &Path, exists: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACE_FILE_FLAGS, ReplaceFileW,
        },
        core::PCWSTR,
    };
    let source: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        if exists {
            ReplaceFileW(
                PCWSTR(target.as_ptr()),
                PCWSTR(source.as_ptr()),
                PCWSTR::null(),
                REPLACE_FILE_FLAGS(0),
                None,
                None,
            )
        } else {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(target.as_ptr()),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    }
    .map_err(|error| io::Error::from_raw_os_error(error.code().0 & 0xffff))
}

#[cfg(not(all(windows, feature = "windows-bindings")))]
fn replace(from: &Path, to: &Path, exists: bool) -> io::Result<()> {
    if exists {
        fs::rename(from, to)
    } else {
        fs::hard_link(from, to)?;
        fs::remove_file(from)
    }
}
