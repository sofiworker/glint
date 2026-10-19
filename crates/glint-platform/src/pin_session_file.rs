//! Private session file and exact owned temporary-file cleanup. Never recursive.
use std::{
    fs, io,
    path::{Path, PathBuf},
};
pub const NAME: &str = "pins.session";
pub fn path(directory: &Path) -> PathBuf {
    directory.join(NAME)
}
pub fn read(
    directory: &Path,
    limit: usize,
) -> Result<Option<Vec<u8>>, super::config_file::StoreError> {
    match super::config_file::read_document(&path(directory), limit) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(super::config_file::StoreError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
fn owned_temp(name: &str) -> bool {
    name.strip_prefix("pins.session.tmp-")
        .and_then(|s| s.split_once('-'))
        .is_some_and(|(pid, seq)| pid.parse::<u32>().is_ok() && seq.parse::<u64>().is_ok())
}
pub fn clear_temporary(directory: &Path) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_str().is_some_and(owned_temp) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}
pub fn clear(directory: &Path) -> io::Result<()> {
    match fs::remove_file(path(directory)) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    clear_temporary(directory)
}
pub fn write(
    directory: &Path,
    bytes: &[u8],
    limit: usize,
) -> Result<(), super::config_file::StoreError> {
    fs::create_dir_all(directory)?;
    clear_temporary(directory)?;
    super::config_file::replace_asset(&path(directory), bytes, limit)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replaces_without_backups_and_clears_only_exact_owned_files() {
        let dir = std::env::temp_dir().join(format!("glint-session-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        write(&dir, b"old", 100).unwrap();
        write(&dir, b"new", 100).unwrap();
        assert_eq!(read(&dir, 100).unwrap().unwrap(), b"new");
        fs::write(dir.join("pins.session.tmp-12-34"), b"unfinished").unwrap();
        fs::write(dir.join("pins.session.tmp-not-owned"), b"keep").unwrap();
        fs::write(dir.join("photo.png"), b"exported").unwrap();
        clear(&dir).unwrap();
        assert!(read(&dir, 100).unwrap().is_none());
        assert!(!dir.join("pins.session.tmp-12-34").exists());
        assert!(dir.join("pins.session.tmp-not-owned").exists());
        assert!(dir.join("photo.png").exists());
        fs::remove_file(dir.join("pins.session.tmp-not-owned")).unwrap();
        fs::remove_file(dir.join("photo.png")).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
