use glint_platform::config_file::{ConfigFile, StoreError};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn external_export_publishes_complete_bytes_without_replacing_or_leaving_temporary_files() {
    use glint_platform::config_file::{export_new, read_document};
    let directory = TestDirectory::new();
    let path = directory.file();
    export_new(&path, b"config_version=1", 1024).unwrap();
    assert_eq!(read_document(&path, 1024).unwrap(), b"config_version=1");
    assert!(export_new(&path, b"replacement", 1024).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"config_version=1");
    directory.assert_no_pending_files();
}

#[test]
fn external_read_and_export_limits_do_not_create_missing_directories_or_targets() {
    use glint_platform::config_file::{export_new, read_document};
    let directory = TestDirectory::new();
    let path = directory.file();
    assert!(matches!(
        export_new(&path, b"12345", 4),
        Err(StoreError::TooLarge)
    ));
    assert!(!path.exists());
    fs::write(&path, b"12345").unwrap();
    assert!(matches!(read_document(&path, 4), Err(StoreError::TooLarge)));
    assert!(export_new(&directory.0.join("missing/config.toml"), b"x", 100).is_err());
    assert!(!directory.0.join("missing").exists());
    directory.assert_no_pending_files();
}
struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "glint-config-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("config.toml")
    }
    fn assert_no_pending_files(&self) {
        for entry in fs::read_dir(&self.0).unwrap() {
            assert!(
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp-")
            );
        }
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        // Only remove immediate test-created files; no recursive deletion or computed parent paths.
        if let Ok(entries) = fs::read_dir(&self.0) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_dir(&self.0);
    }
}

#[test]
fn saves_replace_reliably_and_backup_tracks_last_complete_version() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let mut file = ConfigFile::open(&path, 1024).unwrap();
    assert!(!path.exists());
    assert!(file.bytes().is_none());
    file.save(b"version = 1", true).unwrap();
    assert!(file.backup_bytes().unwrap().is_none());
    for version in [2, 3] {
        let previous = fs::read(&path).unwrap();
        let next = format!("version = {version}");
        file.save(next.as_bytes(), true).unwrap();
        assert_eq!(file.backup_bytes().unwrap().unwrap(), previous);
        assert_eq!(fs::read(&path).unwrap(), next.as_bytes());
    }
    directory.assert_no_pending_files();
}

#[test]
fn external_changes_and_oversized_files_are_not_overwritten() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let mut file = ConfigFile::open(&path, 8).unwrap();
    fs::write(&path, b"external").unwrap();
    assert!(matches!(file.save(b"mine", true), Err(StoreError::Changed)));
    assert_eq!(fs::read(&path).unwrap(), b"external");
    let mut file = ConfigFile::open(&path, 8).unwrap();
    assert!(matches!(
        file.save(b"too large", true),
        Err(StoreError::TooLarge)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"external");
    fs::write(&path, b"too large").unwrap();
    assert!(matches!(
        ConfigFile::open(&path, 8),
        Err(StoreError::TooLarge)
    ));
    directory.assert_no_pending_files();
}

#[test]
fn damaged_bytes_can_be_preserved_without_destroying_the_good_backup() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let mut file = ConfigFile::open(&path, 1024).unwrap();
    file.save(b"good", true).unwrap();
    file.save(b"newer", true).unwrap();
    fs::write(&path, b"[damaged").unwrap();
    let mut file = ConfigFile::open(&path, 1024).unwrap();
    let preserved = file.preserve_current().unwrap().unwrap();
    assert_eq!(fs::read(preserved).unwrap(), b"[damaged");
    assert_eq!(fs::read(&path).unwrap(), b"[damaged");
    file.save(b"recovered", false).unwrap();
    assert_eq!(file.backup_bytes().unwrap().unwrap(), b"good");
}

#[cfg(windows)]
fn lock(path: &std::path::Path) -> fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .unwrap()
}

#[test]
#[cfg(all(windows, feature = "windows-bindings"))]
fn locked_main_or_backup_reports_failure_preserves_original_and_allows_retry() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let mut file = ConfigFile::open(&path, 1024).unwrap();
    file.save(b"first", true).unwrap();
    file.save(b"second", true).unwrap();
    let backup = directory.0.join("config.toml.bak");
    for locked_path in [&path, &backup] {
        let locked = lock(locked_path);
        assert!(file.save(b"failed", true).is_err());
        drop(locked);
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(file.bytes().unwrap(), b"second");
        directory.assert_no_pending_files();
    }
    file.save(b"retry", true).unwrap();
    assert_eq!(fs::read(path).unwrap(), b"retry");
    assert_eq!(fs::read(backup).unwrap(), b"second");
}
