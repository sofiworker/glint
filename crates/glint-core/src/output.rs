//! Platform-independent export naming. Calendar values come from the platform at output time.
use crate::config::{ImageFormat, Output};

pub fn extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
    }
}

pub fn format_from_path(path: &std::path::Path) -> Result<ImageFormat, &'static str> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok(ImageFormat::Png),
        Some("jpg" | "jpeg") => Ok(ImageFormat::Jpeg),
        _ => Err("output.unsupported_extension"),
    }
}
pub fn filename(
    output: &Output,
    date: &str,
    time: &str,
    width: u32,
    height: u32,
) -> Result<String, &'static str> {
    if !crate::config::valid_filename_template(&output.filename)
        || width == 0
        || height == 0
        || date.len() != 10
        || !date.bytes().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        || time.len() != 6
        || !time.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("output.invalid_filename");
    }
    let name = output
        .filename
        .replace("{date}", date)
        .replace("{time}", time)
        .replace("{width}", &width.to_string())
        .replace("{height}", &height.to_string());
    // Expansion must still fit a Windows file name, including its extension.
    let name = format!("{name}.{}", extension(output.format));
    if name.encode_utf16().count() > 255 {
        return Err("output.filename_too_long");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_conflict_defaults_migrate_and_portable_previews_validate_choices() {
        use crate::config::{Config, FileConflict};
        let base = Config::default();
        let old = base
            .to_toml()
            .unwrap()
            .replace("file_conflict = \"reject\"\n", "");
        assert_eq!(
            Config::from_toml(&old).unwrap().output.file_conflict,
            FileConflict::Reject
        );
        for (id, expected) in [
            ("reject", FileConflict::Reject),
            ("number", FileConflict::Number),
            ("confirm", FileConflict::Confirm),
        ] {
            let input = format!("config_version = 1\n[output]\nfile_conflict = '{id}'\n");
            let preview = crate::config_transfer::preview(&base, &input).unwrap();
            assert_eq!(preview.candidate.output.file_conflict, expected);
            let roundtrip = crate::config_transfer::export(&preview.candidate).unwrap();
            assert_eq!(
                Config::from_toml(&roundtrip).unwrap().output.file_conflict,
                expected
            );
        }
        let invalid = crate::config_transfer::preview(
            &base,
            "config_version = 1\n[output]\nfile_conflict = 'silent_overwrite'\n",
        )
        .unwrap();
        assert_eq!(invalid.candidate, base);
        assert_eq!(invalid.skipped, vec!["output.file_conflict"]);
    }

    #[test]
    fn naming_and_extensions_follow_output_settings() {
        let mut options = Output {
            filename: "Shot_{date}_{time}_{width}x{height}".into(),
            format: ImageFormat::Jpeg,
            ..Output::default()
        };
        assert_eq!(
            filename(&options, "2026-10-19", "123456", 3840, 2160).unwrap(),
            "Shot_2026-10-19_123456_3840x2160.jpg"
        );
        for name in ["shot.jpg", "shot.JPEG", "shot.JpG"] {
            assert_eq!(
                format_from_path(std::path::Path::new(name)),
                Ok(ImageFormat::Jpeg)
            );
        }
        assert_eq!(
            format_from_path(std::path::Path::new("shot.PNG")),
            Ok(ImageFormat::Png)
        );
        for name in ["shot", "shot.webp", "shot.png.exe"] {
            assert!(format_from_path(std::path::Path::new(name)).is_err());
        }
        for template in ["../shot", "{unknown}", "CON", "shot."] {
            options.filename = template.into();
            assert!(filename(&options, "2026-10-19", "123456", 2, 2).is_err());
        }
        options.filename = "{width}".repeat(25);
        assert!(filename(&options, "2026-10-19", "123456", u32::MAX, 2).is_ok());
        options.filename = "{width}".repeat(26);
        assert!(filename(&options, "2026-10-19", "123456", u32::MAX, 2).is_err());
    }
}
