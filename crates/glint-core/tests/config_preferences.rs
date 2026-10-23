use glint_core::config::*;

#[test]
fn scrolling_and_recording_shortcuts_migrate_roundtrip_and_detect_conflicts() {
    let mut config = Config::from_toml("[shortcuts]\ncapture='Ctrl+Shift+S'").unwrap();
    assert!(
        config.shortcuts.bindings()[8..]
            .iter()
            .all(|(_, key)| key.is_none())
    );
    config.shortcuts.scroll_capture = "Ctrl+Alt+F9".into();
    config.shortcuts.recording = "Ctrl+Alt+F10".into();
    config.shortcuts.recording_pause = "Ctrl+Alt+F11".into();
    config.shortcuts.recording_stop = "Ctrl+Alt+F12".into();
    assert_eq!(
        Config::from_toml(&config.to_toml().unwrap()).unwrap(),
        config
    );
    for action in [
        "scroll_capture",
        "recording",
        "recording_pause",
        "recording_stop",
    ] {
        let mut value: toml::Value = toml::from_str(&config.to_toml().unwrap()).unwrap();
        value["shortcuts"][action] = toml::Value::String(" ctrl + shift + s ".into());
        assert!(
            matches!(Config::from_toml(&toml::to_string(&value).unwrap()),
            Err(ConfigError::Invalid(ref issues)) if issues.iter().any(|issue| issue.message == "config.duplicate_shortcut"))
        );
    }
    config.shortcuts.scroll_capture.clear();
    config.shortcuts.recording.clear();
    config.shortcuts.recording_pause.clear();
    config.shortcuts.recording_stop.clear();
    assert!(
        Config::from_toml(&config.to_toml().unwrap())
            .unwrap()
            .shortcuts
            .bindings()[8..]
            .iter()
            .all(|(_, key)| key.is_none())
    );
}

#[test]
fn defaults_roundtrip_and_keep_privacy_opt_in() {
    let config = Config::from_toml("").unwrap();
    assert_eq!(config, Config::default());
    assert!(!config.general.start_on_login);
    assert!(!config.privacy.history && !config.privacy.history_disk && !config.privacy.diagnostics);
    assert!(!config.output.auto_save && !config.pins.restore_session && !config.pins.passthrough);
    assert_eq!(config.output.finish, FinishAction::Copy);
    assert_eq!(config.appearance.theme, Theme::Light);
    assert_eq!(
        Config::from_toml(&config.to_toml().unwrap()).unwrap(),
        config
    );
    assert_eq!(
        Config::from_toml("[capture]\ninclude_cursor = true")
            .unwrap()
            .capture
            .dim_percent,
        52
    );
}

#[test]
fn cleared_default_hotkeys_survive_restarting_and_duplicates_use_normalized_chords() {
    let mut config = Config::default();
    config.shortcuts.capture.clear();
    let reloaded = Config::from_toml(&config.to_toml().unwrap()).unwrap();
    assert!(reloaded.shortcuts.bindings()[0].1.is_none());
    assert_eq!(reloaded, config);
    config.shortcuts.settings = " ctrl + shift + w ".into();
    assert!(
        matches!(config.validate(), Err(ConfigError::Invalid(ref issues)) if issues.iter().any(|issue| issue.message == "config.duplicate_shortcut"))
    );
    config.shortcuts.settings.clear();
    for plain in ["A", "F", "Enter", "Space"] {
        config.shortcuts.settings = plain.into();
        assert!(config.validate().is_err());
    }
    config.shortcuts.settings.clear();
    config.shortcuts.color_picker = " ".repeat(81);
    assert!(config.validate().is_err());
    config.shortcuts.color_picker.clear();
    config.shortcuts.rescue.clear();
    assert!(config.validate().is_err());
    assert_eq!(
        KeyChord::parse("Control + Alt + Esc"),
        KeyChord::parse("alt+ctrl+Escape")
    );
    assert_eq!(KeyChord::parse("F01"), KeyChord::parse("F1"));
    for bad in [
        "",
        "Ctrl",
        "Ctrl+Ctrl+S",
        "Ctrl+?",
        "F25",
        "Ctrl+S+W",
        "Ctrl++S",
    ] {
        assert!(KeyChord::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn malformed_future_and_unknown_settings_are_never_silently_rewritten() {
    assert_eq!(
        Config::from_toml("config_version = 99\n[future]\nsecret = true"),
        Err(ConfigError::UnsupportedVersion(99))
    );
    assert_eq!(
        Config::from_toml("config_version = 0"),
        Err(ConfigError::UnsupportedVersion(0))
    );
    assert_eq!(
        Config::from_toml("[general]\nstart_logni = true"),
        Err(ConfigError::Syntax)
    );
    assert_eq!(Config::from_toml("language = ["), Err(ConfigError::Syntax));
    assert_eq!(
        Config::from_toml(&" ".repeat(MAX_CONFIG_BYTES + 1)),
        Err(ConfigError::TooLarge)
    );
    for snippet in [
        "[output]\nfilename = '../private'",
        "[output]\nfilename = 'CON.png'",
        "[output]\nfilename = '{unknown}'",
        "[output]\nquality = 0",
        "[annotation]\ncolor = '#xyz123'",
        "[toolbar]\nhidden = ['select']",
        "[toolbar]\norder = ['select']",
    ] {
        assert!(Config::from_toml(snippet).is_err(), "{snippet}");
    }
}

#[test]
fn previews_cancel_and_restore_do_not_change_running_preferences_before_commit() {
    let mut original = Config::default();
    original.general.onboarding_complete = true;
    let mut preferences = Preferences::new(original.clone());
    preferences.draft.appearance.theme = Theme::Dark;
    assert!(preferences.is_dirty());
    assert_eq!(preferences.saved(), &original);
    preferences.cancel();
    assert!(!preferences.is_dirty());
    preferences.draft.annotation.stroke_width = 0;
    assert!(preferences.accept_saved().is_err());
    assert_eq!(preferences.saved(), &original);
    preferences.restore_defaults();
    assert!(preferences.draft.general.onboarding_complete);
    preferences.draft.appearance.theme = Theme::Dark;
    preferences.accept_saved().unwrap();
    assert!(!preferences.is_dirty());
    assert_eq!(preferences.saved().appearance.theme, Theme::Dark);
}
