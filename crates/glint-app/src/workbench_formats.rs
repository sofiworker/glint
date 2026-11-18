//! Bounded raster input and flattened, atomic editor copies.
use image::{ImageDecoder, ImageFormat, RgbaImage};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

pub const IMAGE_FORMATS: [ImageFormat; 5] = [
    ImageFormat::Png,
    ImageFormat::Jpeg,
    ImageFormat::WebP,
    ImageFormat::Bmp,
    ImageFormat::Tiff,
];
pub fn extension(index: i32) -> Result<&'static str, String> {
    ["png", "jpg", "webp", "bmp", "tiff"]
        .get(index as usize)
        .copied()
        .ok_or_else(|| "Unsupported image export format".into())
}
pub fn decode(path: &Path) -> Result<RgbaImage, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Image file exceeds 64 MiB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    if !reader
        .format()
        .is_some_and(|f| IMAGE_FORMATS.contains(&f) || f == ImageFormat::Ico)
    {
        return Err("Use PNG, JPEG, WebP, BMP, TIFF, ICO or a supported local video".into());
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > glint_core::workbench::MAX_PIXELS
        || decoder.total_bytes() > 64 * 1024 * 1024
    {
        return Err("Decoded image exceeds editor budget".into());
    }
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}
struct Pending(PathBuf);
impl Drop for Pending {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub fn save(
    path: &Path,
    pixels: &RgbaImage,
    format_index: i32,
    quality: u8,
    replacement: Option<&glint_platform::ImageReplacement>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let format = *IMAGE_FORMATS
        .get(format_index as usize)
        .ok_or("Invalid image format")?;
    if !(1..=100).contains(&quality) {
        return Err("Invalid JPEG quality".into());
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().ok_or("Missing output directory")?;
    let (pending, file) = (0..64)
        .find_map(|_| {
            let temp = dir.join(format!(
                ".glint-editor-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
            {
                Ok(file) => Some(Ok((Pending(temp), file))),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(e) => Some(Err(e.to_string())),
            }
        })
        .ok_or("Unable to reserve image output")??;
    let mut writer = std::io::BufWriter::new(file);
    if format == ImageFormat::Jpeg {
        let rgb: Vec<u8> = pixels
            .pixels()
            .flat_map(|p| {
                let a = u32::from(p[3]);
                [p[0], p[1], p[2]].map(|c| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8)
            })
            .collect();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality)
            .encode(
                &rgb,
                pixels.width(),
                pixels.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| e.to_string())?;
    } else {
        image::write_buffer_with_format(
            &mut writer,
            pixels.as_raw(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
            format,
        )
        .map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())?;
    writer.get_ref().sync_all().map_err(|e| e.to_string())?;
    drop(writer);
    if cancel.load(Ordering::Acquire) {
        return Err("Image export cancelled".into());
    }
    if let Some(replacement) = replacement {
        replacement.publish(&pending.0, path)?;
    } else if !glint_platform::publish_new_image(&pending.0, path)? {
        return Err("Destination exists; choose another filename".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raster_copies_roundtrip_alpha_and_never_publish_cancelled_output() {
        let dir = std::env::temp_dir().join(format!(
            "glint-formats-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let pixels = RgbaImage::from_pixel(32, 16, image::Rgba([33, 91, 180, 117]));
        let cancel = AtomicBool::new(false);
        for i in 0..5 {
            let path = dir.join(format!("image.{}", extension(i).unwrap()));
            save(&path, &pixels, i, 95, None, &cancel).unwrap();
            let decoded = decode(&path).unwrap();
            assert_eq!(decoded.dimensions(), (32, 16));
            if i != 1 {
                assert_eq!(decoded, pixels);
            } else {
                assert_eq!(decoded.get_pixel(0, 0)[3], 255);
                assert!(decoded.get_pixel(0, 0)[0] > 140);
            }
            let original = std::fs::read(&path).unwrap();
            assert!(save(&path, &pixels, i, 95, None, &cancel).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            std::fs::remove_file(path).unwrap();
        }
        let path = dir.join("cancelled.webp");
        cancel.store(true, Ordering::Release);
        assert!(save(&path, &pixels, 2, 90, None, &cancel).is_err());
        assert!(!path.exists());
        assert!(std::fs::read_dir(&dir).unwrap().next().is_none());
        let ico = dir.join("icon.ico");
        pixels.save_with_format(&ico, ImageFormat::Ico).unwrap();
        assert_eq!(decode(&ico).unwrap(), pixels);
        std::fs::remove_file(ico).unwrap();
        let jpeg = dir.join("rotated.jpg");
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(pixels.clone())
            .to_rgb8()
            .write_to(&mut encoded, ImageFormat::Jpeg)
            .unwrap();
        let raw = encoded.into_inner();
        // EXIF Orientation=6: source width/height must swap before editing.
        let exif = [
            b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0,
            0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut oriented = raw[..2].to_vec();
        oriented.extend_from_slice(&[0xff, 0xe1, 0, 34]);
        oriented.extend_from_slice(&exif);
        oriented.extend_from_slice(&raw[2..]);
        std::fs::write(&jpeg, oriented).unwrap();
        assert_eq!(decode(&jpeg).unwrap().dimensions(), (16, 32));
        std::fs::remove_file(jpeg).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
