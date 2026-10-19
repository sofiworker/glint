//! Application policy for version checks, recovery and save/rollback. No UI or OS type in Config.
use glint_core::config::{Config, ConfigError, MAX_CONFIG_BYTES, Preferences};
use glint_platform::config_file::ConfigFile;
use std::path::Path;

pub struct SettingsSession {
    pub preferences: Preferences,
    file: Option<ConfigFile>,
    pub notice: &'static str,
    pub writable: bool,
    pub is_new: bool,
    pub repair_pending: bool,
    backup_current: bool,
}
impl SettingsSession {
    /// Persist one capture toolbar transaction without saving or discarding a settings draft.
    pub fn save_toolbar(&mut self, toolbar: &glint_core::config::Toolbar) -> Result<(), String> {
        let draft = self.preferences.draft.clone();
        self.preferences.draft = self.preferences.saved().clone();
        self.preferences.draft.toolbar = toolbar.clone();
        let result = self.save(|_| Ok(()));
        self.preferences.draft = draft;
        if result.is_ok() {
            self.preferences.draft.toolbar = toolbar.clone();
        }
        result
    }
    pub fn open(path: &Path) -> Self {
        let mut result = Self {
            preferences: Preferences::new(Config::default()),
            file: None,
            notice: "",
            writable: false,
            is_new: false,
            repair_pending: false,
            backup_current: true,
        };
        let file = match ConfigFile::open(path, MAX_CONFIG_BYTES) {
            Ok(file) => file,
            Err(_) => {
                result.notice = "config.unreadable";
                return result;
            }
        };
        result.is_new = file.bytes().is_none();
        let parsed = file.bytes().map(decode).transpose();
        match parsed {
            Ok(config) => {
                result.preferences = Preferences::new(config.unwrap_or_default());
                result.writable = true;
            }
            Err(ConfigError::UnsupportedVersion(_)) => result.notice = "config.unsupported_version",
            Err(_) => {
                // Preserve the original before enabling a repair save. Startup never overwrites it.
                if file.preserve_current().is_ok() {
                    let backup = file
                        .backup_bytes()
                        .ok()
                        .flatten()
                        .and_then(|bytes| decode(&bytes).ok());
                    result.notice = if backup.is_some() {
                        "config.recovered_backup"
                    } else {
                        "config.recovered_defaults"
                    };
                    result.preferences = Preferences::new(backup.unwrap_or_default());
                    result.writable = true;
                    result.backup_current = false;
                    result.repair_pending = true;
                } else {
                    result.notice = "config.recovery_readonly";
                }
            }
        }
        result.file = Some(file);
        result
    }
    pub fn save(
        &mut self,
        mut apply: impl FnMut(&Config) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.writable {
            return Err("config.readonly".into());
        }
        let next = self
            .preferences
            .draft
            .to_toml()
            .map_err(|error| error.to_string())?;
        apply(&self.preferences.draft)?;
        if let Err(error) = self
            .file
            .as_mut()
            .ok_or("config.readonly")?
            .save(next.as_bytes(), self.backup_current)
        {
            return match apply(self.preferences.saved()) {
                Ok(()) => Err(format!("{error}; config.runtime_rolled_back")),
                Err(_) => Err(format!("{error}; config.rollback_failed")),
            };
        }
        self.preferences
            .accept_saved()
            .map_err(|error| error.to_string())?;
        self.notice = "";
        self.backup_current = true;
        self.is_new = false;
        self.repair_pending = false;
        Ok(())
    }
}
fn decode(bytes: &[u8]) -> Result<Config, ConfigError> {
    Config::from_toml(std::str::from_utf8(bytes).map_err(|_| ConfigError::Syntax)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glint_core::config::Theme;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "glint-preferences-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("config.toml")
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            if let Ok(entries) = fs::read_dir(&self.0) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    #[test]
    fn toolbar_save_preserves_unrelated_draft_and_failed_save_retains_saved_state() {
        let directory = Directory::new();
        let mut session = SettingsSession::open(&directory.file());
        session.preferences.draft.appearance.theme = Theme::Dark;
        let toolbar = glint_core::config::Toolbar {
            mode: glint_core::config::ToolbarMode::Icons,
            ..Default::default()
        };
        session.save_toolbar(&toolbar).unwrap();
        assert_eq!(session.preferences.draft.appearance.theme, Theme::Dark);
        assert_eq!(session.preferences.saved().appearance.theme, Theme::Light);
        assert_eq!(session.preferences.saved().toolbar, toolbar);
        let reopened = SettingsSession::open(&directory.file());
        assert_eq!(reopened.preferences.saved().toolbar, toolbar);
        assert_eq!(reopened.preferences.saved().appearance.theme, Theme::Light);
        fs::write(directory.file(), b"external writer").unwrap();
        assert!(session.save_toolbar(&Default::default()).is_err());
        assert_eq!(session.preferences.saved().toolbar, toolbar);
        assert_eq!(session.preferences.draft.toolbar, toolbar);
        assert_eq!(session.preferences.draft.appearance.theme, Theme::Dark);
        assert_eq!(fs::read(directory.file()).unwrap(), b"external writer");
    }
    #[test]
    fn startup_never_writes_defaults_and_future_versions_are_readonly() {
        let directory = Directory::new();
        let initial = SettingsSession::open(&directory.file());
        assert!(initial.writable && initial.is_new);
        assert!(!directory.file().exists());
        let future = b"config_version = 999\n[unknown]\nkeep = true";
        fs::write(directory.file(), future).unwrap();
        let mut session = SettingsSession::open(&directory.file());
        assert!(!session.writable);
        assert!(
            session
                .save(|_| panic!("read-only configuration must not apply runtime changes"))
                .is_err()
        );
        assert_eq!(fs::read(directory.file()).unwrap(), future);
    }
    #[test]
    fn recovery_retains_original_and_repairs_only_after_explicit_save() {
        let directory = Directory::new();
        let backup = Config::default().to_toml().unwrap();
        fs::write(directory.file(), b"broken [").unwrap();
        fs::write(directory.0.join("config.toml.bak"), backup.as_bytes()).unwrap();
        let mut session = SettingsSession::open(&directory.file());
        assert_eq!(session.notice, "config.recovered_backup");
        assert!(session.writable);
        assert!(session.repair_pending);
        assert_eq!(fs::read(directory.file()).unwrap(), b"broken [");
        assert!(
            fs::read_dir(&directory.0)
                .unwrap()
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt"))
        );
        session.save(|_| Ok(())).unwrap();
        assert!(!session.repair_pending);
        assert!(Config::from_toml(&fs::read_to_string(directory.file()).unwrap()).is_ok());
        assert_eq!(
            fs::read(directory.0.join("config.toml.bak")).unwrap(),
            backup.as_bytes()
        );
    }
    #[test]
    fn runtime_conflicts_and_write_failures_keep_saved_snapshot_and_draft() {
        let directory = Directory::new();
        let mut session = SettingsSession::open(&directory.file());
        session.preferences.draft.appearance.theme = Theme::Dark;
        assert!(session.save(|_| Err("conflict".into())).is_err());
        assert!(!directory.file().exists());
        assert_eq!(session.preferences.saved().appearance.theme, Theme::Light);
        assert!(session.preferences.is_dirty());
        fs::write(directory.file(), b"external").unwrap();
        let mut applied = Vec::new();
        let error = session
            .save(|config| {
                applied.push(config.appearance.theme);
                Ok(())
            })
            .unwrap_err();
        assert!(error.contains("runtime_rolled_back"));
        assert_eq!(applied, vec![Theme::Dark, Theme::Light]);
        assert_eq!(session.preferences.saved().appearance.theme, Theme::Light);
        assert_eq!(fs::read(directory.file()).unwrap(), b"external");
        assert!(session.preferences.is_dirty());
    }
    #[test]
    fn failed_rollback_is_reported_instead_of_claiming_runtime_was_restored() {
        let directory = Directory::new();
        let mut session = SettingsSession::open(&directory.file());
        session.preferences.draft.appearance.theme = Theme::Dark;
        fs::write(directory.file(), b"external").unwrap();
        let mut calls = 0;
        let error = session
            .save(|_| {
                calls += 1;
                if calls == 1 {
                    Ok(())
                } else {
                    Err("rollback conflict".into())
                }
            })
            .unwrap_err();
        assert!(error.contains("rollback_failed"));
        assert_eq!(calls, 2);
    }
}
