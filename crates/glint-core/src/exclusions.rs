//! Portable syntax only. Window/process inspection belongs to glint-platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidExclusions;

/// Keep legacy raw config readable; callers validate new edits/imports separately.
pub fn parse(raw: &str) -> Result<Vec<String>, InvalidExclusions> {
    if raw.len() > 16384 {
        return Err(InvalidExclusions);
    }
    let mut names = Vec::<String>::new();
    let mut entries = 0;
    for name in raw
        .split([';', ',', '\r', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        entries += 1;
        if entries > 128
            || name.encode_utf16().count() > 255
            || !name
                .get(name.len().saturating_sub(4)..)
                .is_some_and(|s| s.eq_ignore_ascii_case(".exe"))
            || name
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        {
            return Err(InvalidExclusions);
        }
        if !names.iter().any(|s| s.eq_ignore_ascii_case(name)) {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_names_without_paths_and_bounds_work() {
        assert_eq!(parse(" \r\n , ; ").unwrap(), Vec::<String>::new());
        assert_eq!(
            parse(" Password-Manager.exe;OTHER.EXE, password-manager.EXE\n程序.exe").unwrap(),
            ["Password-Manager.exe", "OTHER.EXE", "程序.exe"]
        );
        for raw in [
            "C:\\secret.exe",
            "../secret.exe",
            "*.exe",
            "secret",
            "secret.exe\0",
            "x\t.exe",
            "x.exe|y.exe",
        ] {
            assert!(parse(raw).is_err(), "{raw:?}");
        }
        assert!(parse(&vec!["x.exe"; 128].join(";")).is_ok());
        assert!(parse(&vec!["x.exe"; 129].join(";")).is_err());
        assert!(parse(&format!("{}.exe", "x".repeat(251))).is_ok());
        assert!(parse(&format!("{}.exe", "x".repeat(252))).is_err());
    }
    #[test]
    fn legacy_raw_value_survives_loading_and_unrelated_import() {
        let mut base = crate::config::Config::default();
        base.privacy.excluded_apps = "C:/legacy.exe".into();
        assert!(base.validate().is_ok());
        let preview = crate::config_transfer::preview(
            &base,
            "config_version=1\n[appearance]\ntheme='dark'\n[privacy]\nexcluded_apps='*.exe'\n",
        )
        .unwrap();
        assert_eq!(
            preview.candidate.privacy.excluded_apps,
            base.privacy.excluded_apps
        );
        assert_eq!(preview.skipped, ["privacy.excluded_apps"]);
        assert_eq!(preview.changes.len(), 1);
        let preview = crate::config_transfer::preview(
            &base,
            "config_version=1\n[privacy]\nexcluded_apps='valid.exe'\n",
        )
        .unwrap();
        assert_eq!(preview.candidate.privacy.excluded_apps, "valid.exe");
    }
}
