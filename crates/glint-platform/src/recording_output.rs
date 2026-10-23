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
