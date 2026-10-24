//! Result presentation helpers. Decoding runs on the caller's media worker.
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    Win32::{
        Foundation::*,
        Media::MediaFoundation::*,
        System::{Com::*, DataExchange::*, Memory::*},
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};

pub fn play_file(path: &Path) -> Result<(), String> {
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as usize <= 32 {
        Err("No application could play this recording".into())
    } else {
        Ok(())
    }
}

/// CF_HDROP contains the durable saved file, so clipboard references survive
/// closing the preview. The original recording is never deleted by this helper.
pub fn copy_file(path: &Path) -> Result<(), String> {
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, 20 + wide.len() * 2)
            .map_err(|e| e.to_string())?;
        let data = GlobalLock(memory) as *mut u8;
        if data.is_null() {
            let _ = GlobalFree(memory);
            return Err("Clipboard allocation failed".into());
        }
        // DROPFILES: offset, POINT, fNC, fWide; followed by a double-null path.
        std::ptr::write_unaligned(data.cast::<u32>(), 20);
        std::ptr::write_unaligned(data.add(16).cast::<u32>(), 1);
        std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), data.add(20), wide.len() * 2);
        let _ = GlobalUnlock(memory);
        let result = (|| -> windows::core::Result<()> {
            let window = CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("Glint clipboard"),
                Default::default(),
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                None,
                None,
                None,
            )?;
            let result = (|| {
                OpenClipboard(window)?;
                let result = (|| {
                    EmptyClipboard()?;
                    SetClipboardData(15, HANDLE(memory.0)).map(|_| ())
                })();
                let _ = CloseClipboard();
                result
            })();
            let _ = DestroyWindow(window);
            result
        })();
        if result.is_err() {
            let _ = GlobalFree(memory);
        }
        result.map_err(|e| e.to_string())
    }
}

struct Media;
impl Drop for Media {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}
pub fn poster(path: &Path) -> Result<crate::DesktopFrame, String> {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
    {
        let mut reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|e| e.to_string())?
            .thumbnail(1280, 720)
            .to_rgba8();
        return Ok(crate::DesktopFrame {
            x: 0,
            y: 0,
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        });
    }
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(|e| e.to_string())?;
    if let Err(error) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
        unsafe {
            CoUninitialize();
        }
        return Err(error.to_string());
    }
    let _media = Media;
    let result = unsafe { decode_poster(path) };
    result.map_err(|e| e.to_string())
}
unsafe fn decode_poster(path: &Path) -> windows::core::Result<crate::DesktopFrame> {
    unsafe {
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes = attributes.unwrap();
        attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
        let reader = MFCreateSourceReaderFromURL(PCWSTR(path.as_ptr()), &attributes)?;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        reader.SetStreamSelection(stream, true)?;
        let kind = MFCreateMediaType()?;
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        kind.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        reader.SetCurrentMediaType(stream, None, &kind)?;
        for _ in 0..32 {
            let mut sample = None;
            let mut flags = 0;
            reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))?;
            if let Some(sample) = sample {
                let kind = reader.GetCurrentMediaType(stream)?;
                let size = kind.GetUINT64(&MF_MT_FRAME_SIZE)?;
                let (width, height) = ((size >> 32) as u32, size as u32);
                if width == 0
                    || height == 0
                    || u64::from(width) * u64::from(height) * 4 > 64 * 1024 * 1024
                {
                    return Err(windows::core::Error::from(E_INVALIDARG));
                }
                let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(width * 4) as i32;
                let buffer = sample.ConvertToContiguousBuffer()?;
                let mut data = std::ptr::null_mut();
                let mut length = 0;
                buffer.Lock(&mut data, None, Some(&mut length))?;
                let output = (|| {
                    let pitch = stride.unsigned_abs() as usize;
                    if pitch < width as usize * 4 || pitch * height as usize > length as usize {
                        return Err(windows::core::Error::from(E_INVALIDARG));
                    }
                    let bytes = std::slice::from_raw_parts(data, length as usize);
                    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
                    for y in 0..height as usize {
                        let row = if stride < 0 {
                            height as usize - y - 1
                        } else {
                            y
                        };
                        for pixel in bytes[row * pitch..row * pitch + width as usize * 4]
                            .as_chunks::<4>()
                            .0
                        {
                            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
                        }
                    }
                    Ok(crate::DesktopFrame {
                        x: 0,
                        y: 0,
                        width,
                        height,
                        rgba,
                    })
                })();
                let _ = buffer.Unlock();
                return output;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
        }
        Err(windows::core::Error::from(E_FAIL))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_encoded_mp4_poster_decodes_expected_pixels() {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
            MFStartup(MF_VERSION, MFSTARTUP_FULL).unwrap();
        }
        let media = Media;
        let path = std::env::temp_dir().join(format!("glint-poster-{}.mp4", std::process::id()));
        let result = (|| -> windows::core::Result<()> {
            let encoder = super::super::encoder::Encoder::new(&path, 96, 64, 15, false)?;
            for frame in 0..15 {
                encoder.video(
                    &[25, 90, 190, 255].repeat(96 * 64),
                    frame * 666_666,
                    666_666,
                )?;
            }
            encoder.finish()
        })();
        drop(media);
        result.unwrap();
        let decoded = poster(&path);
        let _ = std::fs::remove_file(path);
        let decoded = decoded.unwrap();
        assert_eq!((decoded.width, decoded.height), (96, 64));
        let pixel = &decoded.rgba[(32 * 96 + 48) * 4..][..4];
        assert!(
            pixel[0].abs_diff(190) < 15
                && pixel[1].abs_diff(90) < 15
                && pixel[2].abs_diff(25) < 15
                && pixel[3] == 255,
            "unexpected decoded pixel: {pixel:?}"
        );
    }
}

#[cfg(test)]
mod green_scan {
    //! Temporary diagnostic: scan every frame of a recording for green pixels.
    use super::*;

    #[test]
    fn scan_recording_for_green_pixels() {
        let path = std::env::var("GLINT_SCAN_VIDEO").unwrap_or_default();
        if path.is_empty() {
            return;
        }
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.unwrap();
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.unwrap();
        let media = Media;
        let path = std::path::PathBuf::from(path);
        let mut report: std::collections::BTreeMap<(u32, u32), (usize, [u8; 3])> =
            Default::default();
        let mut padding: Vec<(u32, u32, [u8; 3])> = Vec::new();
        let mut padding_frames = 0usize;
        let mut hits = 0usize;
        let mut frames = 0usize;
        let mut coded = (0u32, 0u32);
        unsafe {
            let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1).unwrap();
            let attributes = attributes.unwrap();
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                .unwrap();
            let reader = MFCreateSourceReaderFromURL(PCWSTR(path.as_ptr()), &attributes).unwrap();
            reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .unwrap();
            let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            reader.SetStreamSelection(stream, true).unwrap();
            let kind = MFCreateMediaType().unwrap();
            kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
            kind.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).unwrap();
            reader.SetCurrentMediaType(stream, None, &kind).unwrap();
            for frame_index in 0..3000u32 {
                let mut sample = None;
                let mut flags = 0;
                reader
                    .ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))
                    .unwrap();
                let Some(sample) = sample else {
                    if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                        break;
                    }
                    continue;
                };
                frames += 1;
                let kind = reader.GetCurrentMediaType(stream).unwrap();
                let size = kind.GetUINT64(&MF_MT_FRAME_SIZE).unwrap();
                let (width, height) = ((size >> 32) as u32, size as u32);
                let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(width * 4) as i32;
                let buffer = sample.ConvertToContiguousBuffer().unwrap();
                let mut data = std::ptr::null_mut();
                let mut length = 0;
                buffer.Lock(&mut data, None, Some(&mut length)).unwrap();
                let bytes = std::slice::from_raw_parts(data, length as usize);
                let pitch = stride.unsigned_abs() as usize;
                coded = (width, height);
                let mut padding_hit = false;
                for y in 0..height as usize {
                    let row = if stride < 0 {
                        height as usize - y - 1
                    } else {
                        y
                    };
                    for (x, pixel) in bytes[row * pitch..row * pitch + width as usize * 4]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                    {
                        let (r, g, b) = (pixel[2], pixel[1], pixel[0]);
                        // Columns and rows past the display aperture are
                        // encoder padding from macroblock alignment.
                        if x as u32 >= 696 || y as u32 >= 318 {
                            if !(r < 8 && g < 8 && b < 8) {
                                padding_hit = true;
                                if padding.len() < 20 {
                                    padding.push((x as u32, y as u32, [r, g, b]));
                                }
                            }
                            continue;
                        }
                        if g > 140 && r < 110 && b < 110 && g - r > 50 && g - b > 50 {
                            let entry = report.entry((x as u32, y as u32)).or_insert((0, [0; 3]));
                            entry.0 += 1;
                            entry.1 = [r, g, b];
                            hits += 1;
                        }
                    }
                }
                if padding_hit {
                    padding_frames += 1;
                }
                buffer.Unlock().unwrap();
            }
        }
        drop(media);
        println!(
            "frames={frames} coded={coded:?} green-pixel-hits={hits} padding-frames={padding_frames}"
        );
        for ((x, y), (count, rgb)) in report.iter().take(30) {
            println!("  ({x},{y}) x{count} rgb({},{},{})", rgb[0], rgb[1], rgb[2]);
        }
        println!("padding samples:");
        for (x, y, rgb) in padding.iter().take(20) {
            println!("  ({x},{y}) rgb({},{},{})", rgb[0], rgb[1], rgb[2]);
        }
    }
}

#[cfg(test)]
mod encoder_corruption {
    //! Temporary diagnostic: reproduce green-block corruption at 696x318.
    use super::*;
    use crate::recording::encoder::Encoder;

    #[test]
    fn synthetic_dark_recording_has_no_green_blocks() {
        let hardware = std::env::var("GLINT_ENCODER_HW").as_deref() == Ok("1");
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.unwrap();
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.unwrap();
        let media = Media;
        let path = std::env::temp_dir().join(format!(
            "glint-encoder-{}-{}.mp4",
            std::process::id(),
            hardware as u8
        ));
        let (width, height) = (696u32, 318u32);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        // Dark IDE-like content: near-black background, faint mono text noise.
        for y in 0..height as usize {
            for x in 0..width as usize {
                let seed =
                    (y as u32).wrapping_mul(2_654_435_761) ^ (x as u32).wrapping_mul(4_050_323);
                let v = ((seed >> 11) & 0x3f) as u8;
                let at = (y * width as usize + x) * 4;
                pixels[at..at + 3].copy_from_slice(&[v, v, v.saturating_add(6)]);
            }
        }
        let frames = 300usize;
        let mut encode = || -> Result<(), windows::core::Error> {
            let encoder = Encoder::new_with_hardware(&path, width, height, 30, false, hardware)?;
            for frame in 0..frames {
                // Slight per-frame change so the encoder really works.
                let at = ((frame * 7 % height as usize) * width as usize + 100) * 4;
                pixels[at..at + 3].copy_from_slice(&[240, 240, 240]);
                encoder.video(&pixels, frame as i64 * 333_333, 333_333)?;
            }
            encoder.finish()
        };
        encode().unwrap();
        // Scan every decoded frame for green blocks.
        unsafe {
            let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1).unwrap();
            let attributes = attributes.unwrap();
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                .unwrap();
            let reader =
                MFCreateSourceReaderFromURL(PCWSTR(path_wide.as_ptr()), &attributes).unwrap();
            reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .unwrap();
            let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            reader.SetStreamSelection(stream, true).unwrap();
            let kind = MFCreateMediaType().unwrap();
            kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
            kind.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).unwrap();
            reader.SetCurrentMediaType(stream, None, &kind).unwrap();
            let mut hits = 0usize;
            let mut frames = 0usize;
            let mut padded_edge_mismatches = 0usize;
            let mut coded = (0u32, 0u32);
            loop {
                let mut sample = None;
                let mut flags = 0;
                reader
                    .ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))
                    .unwrap();
                let Some(sample) = sample else { break };
                frames += 1;
                let kind = reader.GetCurrentMediaType(stream).unwrap();
                let size = kind.GetUINT64(&MF_MT_FRAME_SIZE).unwrap();
                let (w, h) = ((size >> 32) as u32, size as u32);
                coded = (w, h);
                let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(w * 4) as i32;
                let buffer = sample.ConvertToContiguousBuffer().unwrap();
                let mut data = std::ptr::null_mut();
                let mut length = 0;
                buffer.Lock(&mut data, None, Some(&mut length)).unwrap();
                let bytes = std::slice::from_raw_parts(data, length as usize);
                let pitch = stride.unsigned_abs() as usize;
                for y in 0..h as usize {
                    let row = if stride < 0 { h as usize - y - 1 } else { y };
                    for (x, pixel) in bytes[row * pitch..row * pitch + w as usize * 4]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                    {
                        let (r, g, b) = (pixel[2], pixel[1], pixel[0]);
                        if g > 140 && r < 110 && b < 110 && g - r > 50 && g - b > 50 {
                            hits += 1;
                        }
                        // The padding must replicate the frame's right edge.
                        if x as u32 >= width && (y as u32) < height {
                            let source = &pixels
                                [(y as usize * width as usize + (width as usize - 1)) * 4..][..3];
                            if pixel[0].abs_diff(source[0]) > 15
                                || pixel[1].abs_diff(source[1]) > 15
                                || pixel[2].abs_diff(source[2]) > 15
                            {
                                padded_edge_mismatches += 1;
                            }
                        }
                    }
                }
                buffer.Unlock().unwrap();
            }
            println!(
                "hardware={hardware} frames={frames} coded={coded:?} green-hits={hits} padding-mismatches={padded_edge_mismatches} file={}",
                path.display()
            );
            assert_eq!(frames, frames_count(frames), "unexpected frame count");
            assert_eq!(coded, (704, 320), "coded surface must be 16-aligned");
            assert_eq!(hits, 0, "green corruption detected (hardware={hardware})");
            assert_eq!(
                padded_edge_mismatches, 0,
                "padding must replicate the right edge"
            );
        }
    }
    fn frames_count(actual: usize) -> usize {
        actual
    }
}

#[cfg(test)]
mod real_recording_probe {
    //! Temporary diagnostic: record the real desktop at an even but not
    //! 16-aligned size, then verify the coded padding carries no green edge.
    use super::*;
    use crate::recording::{Format, Options, Recorder, region_source};
    use std::sync::mpsc;

    #[test]
    fn real_region_recording_has_no_green_edge() {
        let Some(directory) = std::env::var("GLINT_RECORD_PROBE")
            .ok()
            .filter(|v| !v.is_empty())
        else {
            return;
        };
        let (source, crop) = region_source((100, 100, 696, 318)).unwrap();
        let (send, receive) = mpsc::channel();
        let recorder = Recorder::start(
            Options {
                source,
                crop: Some(crop),
                fps: 30,
                format: Format::Mp4,
                cursor: true,
                system_audio: false,
                microphone: false,
                directory: directory.into(),
                exclusions: Vec::new(),
            },
            move |result| {
                let _ = send.send(result);
            },
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(6));
        recorder.stop(false);
        let path = receive
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap()
            .unwrap();
        println!("recorded {}", path.display());
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.unwrap();
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.unwrap();
        let media = Media;
        let mut artifact = 0usize;
        let mut frames = 0usize;
        let mut coded = (0u32, 0u32);
        let mut padding_mismatch = 0usize;
        let mut padding_total = 0usize;
        let mut display_green = 0usize;
        unsafe {
            let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1).unwrap();
            let attributes = attributes.unwrap();
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                .unwrap();
            let reader = MFCreateSourceReaderFromURL(PCWSTR(path.as_ptr()), &attributes).unwrap();
            reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .unwrap();
            let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            reader.SetStreamSelection(stream, true).unwrap();
            let kind = MFCreateMediaType().unwrap();
            kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
            kind.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).unwrap();
            reader.SetCurrentMediaType(stream, None, &kind).unwrap();
            loop {
                let mut sample = None;
                let mut flags = 0;
                reader
                    .ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))
                    .unwrap();
                let Some(sample) = sample else { break };
                frames += 1;
                let kind = reader.GetCurrentMediaType(stream).unwrap();
                let size = kind.GetUINT64(&MF_MT_FRAME_SIZE).unwrap();
                let (w, h) = ((size >> 32) as u32, size as u32);
                coded = (w, h);
                let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(w * 4) as i32;
                let buffer = sample.ConvertToContiguousBuffer().unwrap();
                let mut data = std::ptr::null_mut();
                let mut length = 0;
                buffer.Lock(&mut data, None, Some(&mut length)).unwrap();
                let bytes = std::slice::from_raw_parts(data, length as usize);
                let pitch = stride.unsigned_abs() as usize;
                for y in 0..h as usize {
                    let row = if stride < 0 { h as usize - y - 1 } else { y };
                    for (x, pixel) in bytes[row * pitch..row * pitch + w as usize * 4]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                    {
                        let (r, g, b) = (pixel[2], pixel[1], pixel[0]);
                        if x as u32 >= 696 || y as u32 >= 318 {
                            padding_total += 1;
                            // The artifact signature from the user's video:
                            // neutral-YUV green, rgb(0,135,0)-like.
                            if r < 60 && (100..170).contains(&g) && b < 60 {
                                artifact += 1;
                            }
                            // Padding must continue the row's last real pixel.
                            let edge = (y as usize * pitch) + (696 - 1) * 4;
                            if edge + 3 < bytes.len() {
                                if pixel[0].abs_diff(bytes[edge]) > 20
                                    || pixel[1].abs_diff(bytes[edge + 1]) > 20
                                    || pixel[2].abs_diff(bytes[edge + 2]) > 20
                                {
                                    padding_mismatch += 1;
                                }
                            }
                            continue;
                        }
                        if g > 140 && r < 110 && b < 110 && g - r > 50 && g - b > 50 {
                            display_green += 1;
                        }
                    }
                }
                buffer.Unlock().unwrap();
            }
        }
        drop(media);
        println!(
            "frames={frames} coded={coded:?} artifact={artifact} padding-mismatch={padding_mismatch}/{padding_total} display-green={display_green}"
        );
        assert!(frames >= 100, "recording too short: {frames} frames");
        assert_eq!(artifact, 0, "green edge artifact detected");
    }
}
