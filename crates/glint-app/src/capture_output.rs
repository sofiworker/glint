//! Flattened file output, run by the bounded capture output worker.
use glint_core::config::ImageFormat;
use std::{io::Write, path::Path};

/// Manual saves share one policy across capture, local images and desktop pins.
pub fn save_manual(
    path: &Path,
    pixels: &[u8],
    width: u32,
    height: u32,
    options: &glint_core::config::Output,
    replacement: Option<&glint_platform::ImageReplacement>,
) -> Result<std::path::PathBuf, String> {
    use glint_core::config::FileConflict;
    let format = glint_core::output::format_from_path(path)?;
    validate(pixels, width, height, options.quality)?;
    if options.file_conflict == FileConflict::Number {
        return save_numbered(path, pixels, width, height, format, options.quality);
    }
    if options.file_conflict == FileConflict::Confirm
        && let Some(replacement) = replacement
    {
        let pending = PendingImage::encode(path, pixels, (width, height), format, options.quality)?;
        replacement.publish(&pending.0, path)?;
        return Ok(path.to_owned());
    }
    // Without explicit approval, publication never replaces a file that appeared
    // after choosing the destination.
    save_new(path, pixels, width, height, format, options.quality)?;
    Ok(path.to_owned())
}

fn validate(pixels: &[u8], width: u32, height: u32, quality: u8) -> Result<(), String> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(pixels.len()) || !(60..=100).contains(&quality)
    {
        return Err("Invalid output pixels or quality".into());
    }
    Ok(())
}

pub fn save_new(
    path: &Path,
    pixels: &[u8],
    width: u32,
    height: u32,
    format: ImageFormat,
    quality: u8,
) -> Result<(), String> {
    validate(pixels, width, height, quality)?;
    let pending = PendingImage::encode(path, pixels, (width, height), format, quality)?;
    if !glint_platform::publish_new_image(&pending.0, path)? {
        return Err("Output file already exists".into());
    }
    Ok(())
}

/// Claim names atomically; two concurrent saves can never overwrite one another.
pub fn save_numbered(
    path: &Path,
    pixels: &[u8],
    width: u32,
    height: u32,
    format: ImageFormat,
    quality: u8,
) -> Result<std::path::PathBuf, String> {
    validate(pixels, width, height, quality)?;
    let pending = PendingImage::encode(path, pixels, (width, height), format, quality)?;
    for number in 0..10_000 {
        let candidate = numbered_path(path, number)?;
        if glint_platform::publish_new_image(&pending.0, &candidate)? {
            return Ok(candidate);
        }
    }
    Err("Output filename collision limit reached".into())
}

struct PendingImage(std::path::PathBuf);
impl Drop for PendingImage {
    fn drop(&mut self) {
        // A successful move consumed the name; errors must not leave partial pixels.
        let _ = std::fs::remove_file(&self.0);
    }
}
impl PendingImage {
    fn write(
        path: &Path,
        encode: impl FnOnce(std::fs::File) -> Result<(), String>,
    ) -> Result<Self, String> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let parent = path.parent().ok_or("Missing output directory")?;
        for _ in 0..64 {
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = parent.join(format!(".glint-image-{}-{id}.tmp", std::process::id()));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    let pending = Self(path);
                    encode(file)?;
                    return Ok(pending);
                }
                // Do not delete a pre-existing file belonging to another writer.
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("Cannot reserve a temporary image file".into())
    }
    fn encode(
        path: &Path,
        pixels: &[u8],
        dimensions: (u32, u32),
        format: ImageFormat,
        quality: u8,
    ) -> Result<Self, String> {
        Self::write(path, |file| {
            encode_created(file, pixels, dimensions, format, quality)
        })
    }
}

fn numbered_path(path: &Path, number: u32) -> Result<std::path::PathBuf, String> {
    if number == 0 {
        return Ok(path.to_owned());
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid output filename")?;
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or("Invalid output extension")?;
    let suffix = format!("_{number}.{extension}");
    let available = 255usize
        .checked_sub(suffix.encode_utf16().count())
        .ok_or("Output extension too long")?;
    let mut used = 0;
    let prefix: String = stem
        .chars()
        .take_while(|c| {
            used += c.len_utf16();
            used <= available
        })
        .collect();
    Ok(path.with_file_name(format!("{prefix}{suffix}")))
}

fn encode_created(
    file: std::fs::File,
    pixels: &[u8],
    (width, height): (u32, u32),
    format: ImageFormat,
    quality: u8,
) -> Result<(), String> {
    (|| {
        let mut writer = std::io::BufWriter::new(file);
        match format {
            ImageFormat::Png => image::write_buffer_with_format(
                &mut writer,
                pixels,
                width,
                height,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .map_err(|e| e.to_string())?,
            ImageFormat::Jpeg => {
                // JPEG has no alpha. Blend against white, rather than revealing hidden RGB in transparent pixels.
                let rgb: Vec<u8> = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|p| {
                        let a = u32::from(p[3]);
                        [p[0], p[1], p[2]]
                            .map(|c| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8)
                    })
                    .collect();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality)
                    .encode(&rgb, width, height, image::ExtendedColorType::Rgb8)
                    .map_err(|e| e.to_string())?;
            }
        }
        writer.flush().map_err(|e| e.to_string())?;
        writer.get_ref().sync_all().map_err(|e| e.to_string())
    })()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_and_failed_images_never_publish_and_late_collisions_keep_both_images() {
        let directory = std::env::temp_dir().join(format!(
            "glint-image-publication-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("image.png");
        let pixels = [29, 61, 127, 255].repeat(4);
        let failed = PendingImage::write(&path, |mut file| {
            file.write_all(b"incomplete PNG").unwrap();
            assert!(
                !path.exists(),
                "Partial encoding must not use the final filename"
            );
            drop(file);
            let temporary = std::fs::read_dir(&directory)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            // Force a real OS write error during encoding/flush, after partial bytes.
            encode_created(
                std::fs::File::open(temporary).unwrap(),
                &pixels,
                (2, 2),
                ImageFormat::Png,
                90,
            )
        });
        assert!(failed.is_err());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);

        let pending = PendingImage::encode(&path, &pixels, (2, 2), ImageFormat::Png, 90).unwrap();
        assert!(
            !path.exists(),
            "Only a completed publication may create the final name"
        );
        assert_eq!(
            image::load_from_memory(&std::fs::read(&pending.0).unwrap())
                .unwrap()
                .to_rgba8()
                .as_raw(),
            &pixels
        );
        // A destination created after encoding is never silently overwritten.
        std::fs::write(&path, b"late competing file").unwrap();
        assert!(!glint_platform::publish_new_image(&pending.0, &path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"late competing file");
        assert!(pending.0.exists());
        let alternate = directory.join("image_1.png");
        assert!(glint_platform::publish_new_image(&pending.0, &alternate).unwrap());
        drop(pending);
        assert_eq!(
            image::open(&alternate).unwrap().to_rgba8().as_raw(),
            &pixels
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);

        // A failed final move must release its encoded temporary file as well.
        let pending = PendingImage::encode(&path, &pixels, (2, 2), ImageFormat::Jpeg, 90).unwrap();
        use std::os::windows::fs::OpenOptionsExt;
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&pending.0)
            .unwrap();
        assert!(
            glint_platform::publish_new_image(&pending.0, &directory.join("locked.jpg")).is_err()
        );
        assert!(!directory.join("locked.jpg").exists());
        drop(locked);
        drop(pending);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn manual_conflict_policy_preserves_old_bytes_and_publishes_exact_pixels() {
        use glint_core::config::{FileConflict, Output};
        use std::os::windows::fs::OpenOptionsExt;
        let directory = std::env::temp_dir().join(format!(
            "glint-manual-policy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("existing.png");
        std::fs::write(&path, b"prior bytes").unwrap();
        let pixels = [25, 61, 219, 255].repeat(4);
        let mut options = Output::default();
        assert!(save_manual(&path, &pixels, 2, 2, &options, None).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"prior bytes");
        options.file_conflict = FileConflict::Number;
        let actual = save_manual(&path, &pixels, 2, 2, &options, None).unwrap();
        assert_eq!(actual, directory.join("existing_1.png"));
        assert_eq!(image::open(&actual).unwrap().to_rgba8().as_raw(), &pixels);
        assert_eq!(std::fs::read(&path).unwrap(), b"prior bytes");
        options.file_conflict = FileConflict::Confirm;
        assert!(save_manual(&path, &pixels, 2, 2, &options, None).is_err());
        let approval = glint_platform::ImageReplacement::observe(&path).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        assert!(save_manual(&path, &pixels, 2, 2, &options, Some(&approval)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"prior bytes");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        drop(lock);
        save_manual(&path, &pixels, 2, 2, &options, Some(&approval)).unwrap();
        assert_eq!(image::open(&path).unwrap().to_rgba8().as_raw(), &pixels);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        assert!(save_manual(&path, &pixels, 2, 2, &options, Some(&approval)).is_err());
        let approval = glint_platform::ImageReplacement::observe(&path).unwrap();
        assert!(save_manual(&path, &pixels[..4], 2, 2, &options, Some(&approval)).is_err());
        assert_eq!(image::open(&path).unwrap().to_rgba8().as_raw(), &pixels);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(actual).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn collision_suffix_keeps_unicode_names_within_component_limit() {
        let path = std::path::PathBuf::from(format!("{}.png", "😀".repeat(125)));
        let numbered = numbered_path(&path, 9999).unwrap();
        let name = numbered.file_name().unwrap().to_str().unwrap();
        assert!(name.encode_utf16().count() <= 255);
        assert!(name.ends_with("_9999.png"));
    }

    #[test]
    fn concurrent_numbered_saves_claim_unique_files_and_preserve_existing_target() {
        let directory = std::env::temp_dir().join(format!(
            "glint-numbered-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("same.png");
        std::fs::write(&path, b"existing").unwrap();
        let workers: Vec<_> = (0..8)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let pixels = [i, 0, 0, 255].repeat(4);
                    let saved = save_numbered(&path, &pixels, 2, 2, ImageFormat::Png, 90).unwrap();
                    assert_eq!(
                        image::open(&saved).unwrap().to_rgba8().get_pixel(0, 0).0,
                        [i, 0, 0, 255]
                    );
                    saved
                })
            })
            .collect();
        let paths: std::collections::HashSet<_> =
            workers.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(paths.len(), 8);
        assert_eq!(std::fs::read(&path).unwrap(), b"existing");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn jpeg_is_flattened_and_conflicts_and_invalid_inputs_preserve_files() {
        let path = std::env::temp_dir().join(format!(
            "glint-jpeg-{}-{}.jpg",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(save_new(&path, &[0; 4], 2, 2, ImageFormat::Jpeg, 90).is_err());
        assert!(save_new(&path, &[0; 16], 2, 2, ImageFormat::Jpeg, 59).is_err());
        assert!(!path.exists());
        // Fully transparent hidden red must export as white, rather than disclose RGB.
        let pixels = [255, 0, 0, 0].repeat(32 * 32);
        save_new(&path, &pixels, 32, 32, ImageFormat::Jpeg, 100).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let decoded = image::open(&path).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (32, 32));
        assert!(decoded.pixels().all(|p| p.0.iter().all(|v| *v >= 254)));
        assert!(save_new(&path, &[0; 16], 2, 2, ImageFormat::Jpeg, 90).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(&path).unwrap();

        let pixels: Vec<_> = (0..64 * 64)
            .flat_map(|n| {
                [
                    (n * 73 % 256) as u8,
                    (n * 31 % 256) as u8,
                    (n * 113 % 256) as u8,
                    255,
                ]
            })
            .collect();
        save_new(&path, &pixels, 64, 64, ImageFormat::Jpeg, 60).unwrap();
        let low = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        save_new(&path, &pixels, 64, 64, ImageFormat::Jpeg, 100).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > low.len() as u64);
        std::fs::remove_file(&path).unwrap();
    }
}
