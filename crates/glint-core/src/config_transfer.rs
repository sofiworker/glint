//! Portable TOML previews. Import never changes its baseline or applies system settings.
use crate::config::{CONFIG_VERSION, Config, ConfigError, MAX_CONFIG_BYTES};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub field: String,
    pub before: toml::Value,
    pub after: toml::Value,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub candidate: Config,
    pub changes: Vec<Change>,
    pub skipped: Vec<String>,
}

/// Configuration values only. Runtime state, image assets and permissions are not schema fields.
pub fn export(config: &Config) -> Result<String, ConfigError> {
    config.validate()?;
    let mut value = toml::Value::try_from(config).map_err(|_| ConfigError::Syntax)?;
    value["general"]
        .as_table_mut()
        .ok_or(ConfigError::Syntax)?
        .remove("onboarding_complete");
    let text = toml::to_string_pretty(&value).map_err(|_| ConfigError::Syntax)?;
    if text.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    Ok(text)
}

pub fn preview(base: &Config, text: &str) -> Result<Preview, ConfigError> {
    base.validate()?;
    if text.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge);
    }
    let input: toml::Table = toml::from_str(text.strip_prefix('\u{feff}').unwrap_or(text))
        .map_err(|_| ConfigError::Syntax)?;
    let version = input
        .get("config_version")
        .and_then(toml::Value::as_integer)
        .ok_or(ConfigError::Syntax)?;
    if version != i64::from(CONFIG_VERSION) {
        return Err(ConfigError::UnsupportedVersion(version));
    }
    let original = toml::Value::try_from(base).map_err(|_| ConfigError::Syntax)?;
    let mut accepted = BTreeMap::<String, toml::Value>::new();
    let mut skipped = Vec::new();
    for (section, fields) in input {
        if section == "config_version" {
            continue;
        }
        let Some(known) = original.get(&section).and_then(toml::Value::as_table) else {
            skipped.push(section);
            continue;
        };
        let Some(fields) = fields.as_table() else {
            skipped.push(section);
            continue;
        };
        for (field, value) in fields {
            let key = format!("{section}.{field}");
            if key == "privacy.excluded_apps"
                && value
                    .as_str()
                    .is_none_or(|raw| crate::exclusions::parse(raw).is_err())
            {
                skipped.push(key);
                continue;
            }
            if key == "general.onboarding_complete" || !known.contains_key(field) {
                skipped.push(key);
                continue;
            }
            // Decode against the baseline to check the field's Rust type/enum, but validate
            // related fields together below (in particular shortcut swaps).
            let mut probe = original.clone();
            probe[&section][field] = value.clone();
            if probe.try_into::<Config>().is_err() {
                skipped.push(key);
                continue;
            }
            accepted.insert(key, value.clone());
        }
    }
    loop {
        let mut merged = original.clone();
        for (key, value) in &accepted {
            let (section, field) = key.split_once('.').ok_or(ConfigError::Syntax)?;
            merged[section][field] = value.clone();
        }
        let candidate: Config = merged.try_into().map_err(|_| ConfigError::Syntax)?;
        match candidate.validate() {
            Ok(()) => {
                let changes = accepted
                    .into_iter()
                    .filter_map(|(field, after)| {
                        let (section, key) = field.split_once('.')?;
                        let before = original.get(section)?.get(key)?.clone();
                        (before != after).then_some(Change {
                            field,
                            before,
                            after,
                        })
                    })
                    .collect();
                skipped.sort();
                skipped.dedup();
                return Ok(Preview {
                    candidate,
                    changes,
                    skipped,
                });
            }
            Err(ConfigError::Invalid(issues)) => {
                let before = accepted.len();
                for issue in issues {
                    if issue.field.contains('.') {
                        if accepted.remove(issue.field).is_some() {
                            skipped.push(issue.field.into());
                        }
                    } else {
                        // Shortcut validation reports action IDs. Reject the imported group
                        // atomically so a duplicate never silently disables another binding.
                        let keys: Vec<_> = accepted
                            .keys()
                            .filter(|key| key.starts_with("shortcuts."))
                            .cloned()
                            .collect();
                        for key in keys {
                            accepted.remove(&key);
                            skipped.push(key);
                        }
                    }
                }
                if accepted.len() == before {
                    return Err(ConfigError::Syntax);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Theme;
    #[test]
    fn portable_roundtrip_excludes_local_marker_and_keeps_missing_preferences() {
        let mut base = Config::default();
        base.general.onboarding_complete = true;
        base.output.quality = 88;
        let text = export(&base).unwrap();
        assert!(!text.contains("onboarding_complete"));
        assert_eq!(preview(&base, &text).unwrap().candidate, base);
        let p = preview(&base, "config_version=1\n[appearance]\ntheme='dark'\n").unwrap();
        assert_eq!(p.candidate.appearance.theme, Theme::Dark);
        assert_eq!(p.candidate.output.quality, 88);
        assert!(p.candidate.general.onboarding_complete);
        assert_eq!(p.changes.len(), 1);
        assert_eq!(base.appearance.theme, Theme::Light);
    }
    #[test]
    fn invalid_unknown_and_protected_fields_preserve_baseline_without_discarding_valid_changes() {
        let base = Config::default();
        let p = preview(&base, "config_version=1\n[general]\nonboarding_complete=true\n[appearance]\ntheme='dark'\naccent='#bad'\nscale='oops'\n[capture]\ndelay_seconds=999\ninclude_cursor='yes'\n[pixels]\nraw='private'\n").unwrap();
        assert_eq!(p.candidate.appearance.theme, Theme::Dark);
        assert_eq!(p.candidate.capture, base.capture);
        assert_eq!(p.candidate.appearance.accent, base.appearance.accent);
        assert!(!p.candidate.general.onboarding_complete);
        assert_eq!(p.skipped.len(), 6);
    }
    #[test]
    fn shortcut_swaps_validate_together_and_conflicting_imports_revert_the_group() {
        let base = Config::default();
        let p = preview(
            &base,
            "config_version=1\n[shortcuts]\ncapture='Ctrl+Shift+W'\nwindow='Ctrl+Shift+S'\n",
        )
        .unwrap();
        assert_eq!(p.changes.len(), 2);
        let p = preview(&base, "config_version=1\n[shortcuts]\ncapture='Ctrl+Shift+W'\ncolor_picker='F9'\n[appearance]\ntheme='dark'\n").unwrap();
        assert_eq!(p.candidate.shortcuts, base.shortcuts);
        assert_eq!(p.changes.len(), 1);
        assert_eq!(p.skipped.len(), 2);
    }
    #[test]
    fn version_syntax_size_and_untrusted_nested_shapes_are_checked() {
        let base = Config::default();
        assert_eq!(
            preview(&base, "\u{feff}config_version=1")
                .unwrap()
                .candidate,
            base
        );
        for text in [
            "",
            "config_version='1'",
            "config_version=1\n[",
            "config_version=1\nconfig_version=1",
        ] {
            assert!(preview(&base, text).is_err());
        }
        assert!(matches!(
            preview(&base, "config_version=999"),
            Err(ConfigError::UnsupportedVersion(999))
        ));
        assert!(matches!(
            preview(&base, &"x".repeat(MAX_CONFIG_BYTES + 1)),
            Err(ConfigError::TooLarge)
        ));
        let p = preview(
            &base,
            "config_version=1\nappearance=[]\n[capture]\ndim_percent={x=5}\n",
        )
        .unwrap();
        assert_eq!(p.candidate, base);
        assert_eq!(p.skipped.len(), 2);
    }
}
