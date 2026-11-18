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
    let wide: Vec<u16> = crate::interop::wide(path);
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

/// A locked RGB32 sample surface with display-ordered, bounds-checked rows.
///
/// All raw-pointer handling for Media Foundation sample scanning is confined
/// here; every accessor is safe and validates against the locked length.
struct SampleSurface {
    buffer: IMFMediaBuffer,
    pointer: *const u8,
    length: usize,
    pitch: usize,
    width: u32,
    height: u32,
    bottom_up: bool,
}
impl SampleSurface {
    /// Convert one sample to contiguous RGB32 and lock it for reading.
    /// `fallback_stride` applies when the media type does not declare one.
    fn lock(sample: &IMFSample, fallback_stride: u32) -> windows::core::Result<Self> {
        let buffer = unsafe { sample.ConvertToContiguousBuffer() }?;
        let mut data = std::ptr::null_mut();
        let mut length = 0;
        // The lock stays valid until `buffer.Unlock`, which `Drop` performs.
        unsafe { buffer.Lock(&mut data, None, Some(&mut length)) }?;
        Ok(Self {
            buffer,
            pointer: data.cast(),
            length: length as usize,
            pitch: fallback_stride as usize,
            width: 0,
            height: 0,
            bottom_up: false,
        })
    }
    /// Set the expected geometry after reading it from the media type; every
    /// accessor validates against the locked length before returning rows.
    fn with_geometry(mut self, width: u32, height: u32, stride: i32) -> Self {
        self.width = width;
        self.height = height;
        self.pitch = stride.unsigned_abs() as usize;
        self.bottom_up = stride < 0;
        self
    }
    fn valid(&self) -> bool {
        self.pitch >= self.width as usize * 4
            && self.pitch.saturating_mul(self.height as usize) <= self.length
    }
    /// Display-order row bytes (BGRA, `width * 4` long).
    fn row(&self, y: u32) -> Option<&[u8]> {
        if !self.valid() || y >= self.height {
            return None;
        }
        let y = if self.bottom_up {
            self.height - 1 - y
        } else {
            y
        } as usize;
        let start = y.checked_mul(self.pitch)?;
        let end = start.checked_add(self.width as usize * 4)?;
        if end > self.length {
            return None;
        }
        // The whole locked region was validated above; `start..end` stays
        // inside `self.length`, and the borrow ends with `&self`.
        let bytes = unsafe { std::slice::from_raw_parts(self.pointer, self.length) };
        bytes.get(start..end)
    }
    /// Iterate every display pixel as `(x, y, [b, g, r, a])`.
    fn pixels(&self) -> impl Iterator<Item = (u32, u32, [u8; 4])> + '_ {
        (0..self.height).flat_map(move |y| {
            let row = self.row(y).unwrap_or(&[]);
            row.as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .map(move |(x, pixel)| (x as u32, y, *pixel))
        })
    }
}
impl Drop for SampleSurface {
    fn drop(&mut self) {
        unsafe {
            let _ = self.buffer.Unlock();
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

#[derive(Clone, Copy, Debug)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: u32,
    pub audio: bool,
}
/// A decoder belongs to one media worker. No COM objects cross thread boundaries.
pub struct VideoReader {
    source: Decoder,
    pub info: VideoInfo,
}
enum Decoder {
    Native(Box<NativeVideoReader>),
    Gif(GifReader),
}
struct GifReader {
    path: std::path::PathBuf,
    frames: image::Frames<'static>,
    elapsed: f64,
    last_start: f64,
    last: Option<std::sync::Arc<crate::DesktopFrame>>,
}
impl GifReader {
    fn stream(
        path: &Path,
    ) -> Result<image::codecs::gif::GifDecoder<std::io::BufReader<std::fs::File>>, String> {
        use image::ImageDecoder;
        let mut decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
            std::fs::File::open(path).map_err(|e| e.to_string())?,
        ))
        .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(64 * 1024 * 1024);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        decoder.set_limits(limits).map_err(|e| e.to_string())?;
        let (w, h) = decoder.dimensions();
        if w == 0 || h == 0 || u64::from(w) * u64::from(h) > glint_core::workbench::MAX_PIXELS {
            return Err("GIF exceeds editor pixel budget".into());
        }
        Ok(decoder)
    }
    fn open(path: &Path) -> Result<(Self, VideoInfo), String> {
        use image::{AnimationDecoder, ImageDecoder};
        let decoder = Self::stream(path)?;
        let (w, h) = decoder.dimensions();
        let mut duration = 0.;
        let mut count = 0u32;
        for frame in decoder.into_frames() {
            let frame = frame.map_err(|e| e.to_string())?;
            let (n, d) = frame.delay().numer_denom_ms();
            duration += (f64::from(n) / f64::from(d) / 1000.).max(0.01);
            count += 1;
            if count > 30000 || duration > 1800. {
                return Err("GIF exceeds editor duration budget".into());
            }
        }
        if count == 0 {
            return Err("GIF has no frames".into());
        }
        let info = VideoInfo {
            width: w,
            height: h,
            duration,
            fps: (f64::from(count) / duration).round().clamp(1., 100.) as u32,
            audio: false,
        };
        Ok((
            Self {
                path: path.to_owned(),
                frames: Self::stream(path)?.into_frames(),
                elapsed: 0.,
                last_start: 0.,
                last: None,
            },
            info,
        ))
    }
    fn frame_at(&mut self, time: f64) -> Result<std::sync::Arc<crate::DesktopFrame>, String> {
        use image::AnimationDecoder;
        if time >= self.last_start
            && time < self.elapsed
            && let Some(frame) = &self.last
        {
            return Ok(frame.clone());
        }
        if time < self.last_start && self.last.is_some() {
            self.frames = Self::stream(&self.path)?.into_frames();
            self.elapsed = 0.;
            self.last = None;
        }
        for frame in self.frames.by_ref() {
            let frame = frame.map_err(|e| e.to_string())?;
            let (n, d) = frame.delay().numer_denom_ms();
            self.last_start = self.elapsed;
            self.elapsed += (f64::from(n) / f64::from(d) / 1000.).max(0.01);
            let pixels = frame.into_buffer();
            let (w, h) = pixels.dimensions();
            let result = std::sync::Arc::new(crate::DesktopFrame {
                x: 0,
                y: 0,
                width: w,
                height: h,
                rgba: pixels.into_raw(),
            });
            self.last = Some(result.clone());
            if self.elapsed > time {
                return Ok(result);
            }
        }
        self.last
            .clone()
            .ok_or_else(|| "GIF has no frame at this position".into())
    }
}
impl VideoReader {
    pub fn open(path: &Path) -> Result<Self, String> {
        if path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("gif"))
        {
            let (reader, info) = GifReader::open(path)?;
            Ok(Self {
                source: Decoder::Gif(reader),
                info,
            })
        } else {
            let reader = NativeVideoReader::open(path)?;
            let info = reader.info;
            Ok(Self {
                source: Decoder::Native(Box::new(reader)),
                info,
            })
        }
    }
    pub fn frame_at(&mut self, time: f64) -> Result<std::sync::Arc<crate::DesktopFrame>, String> {
        match &mut self.source {
            Decoder::Native(reader) => reader.frame_at(time),
            Decoder::Gif(reader) => reader.frame_at(time),
        }
    }
}
struct NativeVideoReader {
    reader: IMFSourceReader,
    pub info: VideoInfo,
    last: Option<(f64, std::sync::Arc<crate::DesktopFrame>)>,
    _media: Media,
}
impl NativeVideoReader {
    pub fn open(path: &Path) -> Result<Self, String> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(|e| e.to_string())?;
        if let Err(e) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
            unsafe {
                CoUninitialize();
            }
            return Err(e.to_string());
        }
        let media = Media;
        let result = (|| -> windows::core::Result<_> {
            unsafe {
                let path = crate::interop::wide(path);
                let mut attributes = None;
                MFCreateAttributes(&mut attributes, 1)?;
                let attributes = attributes.unwrap();
                attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
                let reader = MFCreateSourceReaderFromURL(PCWSTR(path.as_ptr()), &attributes)?;
                let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
                reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
                reader.SetStreamSelection(stream, true)?;
                let audio = reader
                    .GetNativeMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, 0)
                    .is_ok();
                let native = reader.GetNativeMediaType(stream, 0)?;
                let size = native.GetUINT64(&MF_MT_FRAME_SIZE)?;
                let rate = native
                    .GetUINT64(&MF_MT_FRAME_RATE)
                    .unwrap_or((30u64 << 32) | 1);
                let duration = reader.GetPresentationAttribute(
                    MF_SOURCE_READER_MEDIASOURCE.0 as u32,
                    &MF_PD_DURATION,
                )?;
                let duration = u64::try_from(&duration)? as f64 / 10_000_000.;
                let info = VideoInfo {
                    width: (size >> 32) as u32,
                    height: size as u32,
                    duration,
                    fps: (((rate >> 32) as f64 / (rate as u32).max(1) as f64).round() as u32)
                        .clamp(1, 144),
                    audio,
                };
                if info.width == 0
                    || info.height == 0
                    || u64::from(info.width) * u64::from(info.height)
                        > glint_core::workbench::MAX_PIXELS
                    || !duration.is_finite()
                    || duration <= 0.
                    || duration > 86400.
                {
                    return Err(windows::core::Error::from(E_INVALIDARG));
                }
                let kind = MFCreateMediaType()?;
                kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                kind.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
                reader.SetCurrentMediaType(stream, None, &kind)?;
                Ok((reader, info))
            }
        })();
        let (reader, info) = result.map_err(|e| e.to_string())?;
        Ok(Self {
            reader,
            info,
            last: None,
            _media: media,
        })
    }
    pub fn frame_at(&mut self, time: f64) -> Result<std::sync::Arc<crate::DesktopFrame>, String> {
        let time = time.clamp(0., (self.info.duration - 0.0001).max(0.));
        if let Some((at, frame)) = &self.last
            && (*at - time).abs() < 0.5 / self.info.fps as f64
        {
            return Ok(frame.clone());
        }
        let seek = self
            .last
            .as_ref()
            .is_none_or(|(at, _)| time < *at || time - *at > 0.5);
        let result = (|| -> windows::core::Result<_> {
            unsafe {
                if seek {
                    self.reader.SetCurrentPosition(
                        &windows::core::GUID::zeroed(),
                        &windows::core::PROPVARIANT::from((time * 10_000_000.) as i64),
                    )?;
                }
                let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
                for _ in 0..1024 {
                    let mut sample = None;
                    let mut flags = 0;
                    let mut at = 0;
                    self.reader.ReadSample(
                        stream,
                        0,
                        None,
                        Some(&mut flags),
                        Some(&mut at),
                        Some(&mut sample),
                    )?;
                    if let Some(sample) = sample {
                        let kind = self.reader.GetCurrentMediaType(stream)?;
                        let size = kind.GetUINT64(&MF_MT_FRAME_SIZE)?;
                        let (w, h) = ((size >> 32) as u32, size as u32);
                        if (w, h) != (self.info.width, self.info.height) {
                            return Err(windows::core::Error::from(E_INVALIDARG));
                        }
                        let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(w * 4);
                        let surface = SampleSurface::lock(&sample, stride)?.with_geometry(
                            w,
                            h,
                            stride as i32,
                        );
                        if !surface.valid() {
                            return Err(windows::core::Error::from(E_INVALIDARG));
                        }
                        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
                        for (_, _, p) in surface.pixels() {
                            rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
                        }
                        let frame = std::sync::Arc::new(crate::DesktopFrame {
                            x: 0,
                            y: 0,
                            width: w,
                            height: h,
                            rgba,
                        });
                        self.last = Some((at as f64 / 10_000_000., frame.clone()));
                        if at as f64 / 10_000_000. + 0.5 / self.info.fps as f64 >= time {
                            return Ok(frame);
                        }
                    }
                    if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                        return self
                            .last
                            .as_ref()
                            .map(|(_, f)| f.clone())
                            .ok_or_else(|| windows::core::Error::from(E_FAIL));
                    }
                }
                Err(windows::core::Error::from(E_FAIL))
            }
        })();
        result.map_err(|e| e.to_string())
    }
}

pub(super) struct AudioReader {
    pub(super) reader: IMFSourceReader,
}
impl AudioReader {
    pub(super) unsafe fn open(path: &Path) -> windows::core::Result<Self> {
        unsafe {
            let path = crate::interop::wide(path);
            let reader = MFCreateSourceReaderFromURL(PCWSTR(path.as_ptr()), None)?;
            let stream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(stream, true)?;
            let kind = MFCreateMediaType()?;
            kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            kind.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            kind.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            kind.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
            kind.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            kind.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
            kind.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192000)?;
            reader.SetCurrentMediaType(stream, None, &kind)?;
            Ok(Self { reader })
        }
    }
    pub(super) unsafe fn packet(&self) -> windows::core::Result<Option<(f64, Vec<u8>)>> {
        unsafe {
            let mut sample = None;
            let mut flags = 0;
            let mut time = 0;
            self.reader.ReadSample(
                MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                0,
                None,
                Some(&mut flags),
                Some(&mut time),
                Some(&mut sample),
            )?;
            if let Some(sample) = sample {
                let buffer = sample.ConvertToContiguousBuffer()?;
                let mut data = std::ptr::null_mut();
                let mut len = 0;
                buffer.Lock(&mut data, None, Some(&mut len))?;
                if len > 4 * 1024 * 1024 {
                    let _ = buffer.Unlock();
                    return Err(windows::core::Error::from(E_INVALIDARG));
                }
                let bytes = std::slice::from_raw_parts(data, len as usize).to_vec();
                buffer.Unlock()?;
                Ok(Some((time as f64 / 10_000_000., bytes)))
            } else if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                Ok(None)
            } else {
                Ok(Some((time as f64 / 10_000_000., vec![])))
            }
        }
    }
}

/// Re-encode edited frames and source-time audio into a *new* destination.
/// The caller owns atomic publication and temporary-file cleanup.
pub fn export_edit(
    source: &Path,
    destination: &Path,
    state: &glint_core::workbench::State,
    gif: bool,
    cancelled: &std::sync::atomic::AtomicBool,
    render: impl FnMut(std::sync::Arc<crate::DesktopFrame>, f64) -> Result<image::RgbaImage, String>,
    progress: impl FnMut(f32),
) -> Result<(), String> {
    export_edit_with_options(
        source,
        destination,
        state,
        gif,
        Default::default(),
        cancelled,
        render,
        progress,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn export_edit_with_options(
    source: &Path,
    destination: &Path,
    state: &glint_core::workbench::State,
    gif: bool,
    options: glint_core::media_export::VideoExport,
    cancelled: &std::sync::atomic::AtomicBool,
    mut render: impl FnMut(std::sync::Arc<crate::DesktopFrame>, f64) -> Result<image::RgbaImage, String>,
    mut progress: impl FnMut(f32),
) -> Result<(), String> {
    use image::{
        Delay, Frame,
        codecs::gif::{GifEncoder, Repeat},
    };
    use std::sync::atomic::Ordering;
    options.validate().map_err(str::to_owned)?;
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(|e| e.to_string())?;
    if let Err(e) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
        unsafe {
            CoUninitialize();
        }
        return Err(e.to_string());
    }
    let _media = Media;
    let mut reader = VideoReader::open(source)?;
    let fps = options.frame_rate(reader.info.fps, gif);
    let step = 1. / fps as f64;
    let (w, h) = state.transform.dimensions();
    let with_audio = reader.info.audio && !state.muted && !gif && options.audio_bitrate.is_some();
    let audio = if with_audio {
        Some(unsafe { AudioReader::open(source) }.map_err(|e| e.to_string())?)
    } else {
        None
    };
    let mut encoder = if gif {
        None
    } else {
        Some(
            super::containers::ContainerEncoder::new(destination, w, h, fps, with_audio, options)
                .map_err(|e| e.to_string())?,
        )
    };
    let mut gif_encoder = if gif {
        let mut e =
            GifEncoder::new(std::fs::File::create_new(destination).map_err(|e| e.to_string())?);
        e.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
        Some(e)
    } else {
        None
    };
    let total = state.duration();
    let mut base = 0.;
    let mut count = 0u64;
    for clip in &state.clips {
        let mut packet = if let Some(a) = &audio {
            unsafe {
                a.reader.SetCurrentPosition(
                    &windows::core::GUID::zeroed(),
                    &windows::core::PROPVARIANT::from((clip.start * 10_000_000.) as i64),
                )
            }
            .map_err(|e| e.to_string())?;
            unsafe { a.packet() }.map_err(|e| e.to_string())?
        } else {
            None
        };
        let frames = (clip.duration() * fps as f64).ceil() as u64;
        for frame in 0..frames {
            if cancelled.load(Ordering::Acquire) {
                return Err("Export cancelled".into());
            }
            let local = frame as f64 * step;
            let time = clip.start + local;
            let duration = step.min(clip.end - time);
            if duration <= 0. {
                break;
            }
            let pixels = render(reader.frame_at(time)?, time)?;
            if pixels.dimensions() != (w, h) {
                return Err("Edited frame size changed during export".into());
            }
            if let Some(e) = &mut encoder {
                let mut bgra = pixels.into_raw();
                for p in bgra.as_chunks_mut::<4>().0 {
                    p.swap(0, 2);
                }
                e.video(
                    &bgra,
                    ((base + local) * 10_000_000.) as i64,
                    (duration * 10_000_000.) as i64,
                    cancelled,
                )
                .map_err(|e| e.to_string())?;
            } else if let Some(e) = &mut gif_encoder {
                e.encode_frame(Frame::from_parts(
                    pixels,
                    0,
                    0,
                    Delay::from_numer_denom_ms((duration * 1_000_000.) as u32, 1000),
                ))
                .map_err(|e| e.to_string())?;
            }
            while let Some((at, bytes)) = packet.take() {
                if at >= clip.end {
                    break;
                }
                if at > time + duration {
                    packet = Some((at, bytes));
                    break;
                }
                let left = ((clip.start - at).max(0.) * 48000.).ceil() as usize;
                let right =
                    (((clip.end - at).max(0.) * 48000.).floor() as usize).min(bytes.len() / 4);
                if right > left {
                    let mut bytes = bytes[left * 4..right * 4].to_vec();
                    for v in bytes.as_chunks_mut::<2>().0 {
                        let n = i16::from_le_bytes([v[0], v[1]]);
                        let n = (i32::from(n) * i32::from(state.volume) / 100)
                            .clamp(i16::MIN as i32, i16::MAX as i32)
                            as i16;
                        v.copy_from_slice(&n.to_le_bytes());
                    }
                    let output_frame = ((base + (at + left as f64 / 48000. - clip.start).max(0.))
                        * 48000.)
                        .round() as u64;
                    encoder
                        .as_mut()
                        .unwrap()
                        .audio(&bytes, output_frame)
                        .map_err(|e| e.to_string())?;
                }
                packet = if let Some(a) = &audio {
                    unsafe { a.packet() }.map_err(|e| e.to_string())?
                } else {
                    None
                };
            }
            count += 1;
            if count.is_multiple_of(fps as u64) {
                progress(((base + local) / total) as f32);
            }
        }
        base += clip.duration();
    }
    if let Some(e) = encoder {
        e.finish(cancelled)?;
    }
    progress(1.);
    Ok(())
}
unsafe fn decode_poster(path: &Path) -> windows::core::Result<crate::DesktopFrame> {
    unsafe {
        let path: Vec<u16> = crate::interop::wide(path);
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
                let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(width * 4);
                let surface = SampleSurface::lock(&sample, stride)?.with_geometry(
                    width,
                    height,
                    stride as i32,
                );
                let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
                for (_, _, pixel) in surface.pixels() {
                    rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
                }
                return Ok(crate::DesktopFrame {
                    x: 0,
                    y: 0,
                    width,
                    height,
                    rgba,
                });
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
    use std::sync::atomic::Ordering;

    #[test]
    fn edited_mp4_reencodes_retained_clips_and_audio_and_gif_seeks() {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
            MFStartup(MF_VERSION, MFSTARTUP_FULL).unwrap();
        }
        let _media = Media;
        let directory = std::env::temp_dir().join(format!("glint-edited-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("source.mp4");
        let destination = directory.join("edited.mp4");
        let gif = directory.join("edited.gif");
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&destination);
        let _ = std::fs::remove_file(&gif);
        let encoder = super::super::encoder::Encoder::new(&source, 96, 64, 15, true).unwrap();
        for i in 0..45u64 {
            let color = if i < 15 {
                [20, 50, 210, 255]
            } else if i < 30 {
                [50, 200, 30, 255]
            } else {
                [210, 50, 20, 255]
            };
            encoder
                .video(
                    &color.repeat(96 * 64),
                    (i * 10_000_000 / 15) as i64,
                    666_667,
                )
                .unwrap();
            let samples: Vec<_> = (0..3200)
                .flat_map(|n| {
                    let value = (((i * 3200 + n) as f64 * 440. * std::f64::consts::TAU / 48000.)
                        .sin()
                        * 2000.) as i16;
                    let [a, b] = value.to_le_bytes();
                    [a, b, a, b]
                })
                .collect();
            encoder.audio(&samples, i * 3200).unwrap();
        }
        encoder.finish().unwrap();
        let mut reader = VideoReader::open(&source).unwrap();
        assert!(reader.info.audio);
        let frame = reader.frame_at(2.2).unwrap();
        assert!(frame.rgba[2] > 160);
        let mut state = glint_core::workbench::Editor::new(96, 64, vec![], reader.info.duration)
            .unwrap()
            .state;
        state.clips = vec![
            glint_core::workbench::Clip {
                start: 0.2,
                end: 0.7,
            },
            glint_core::workbench::Clip {
                start: 2.1,
                end: 2.6,
            },
        ];
        state.volume = 50;
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let compose = |frame: std::sync::Arc<crate::DesktopFrame>, _: f64| {
            let mut pixels =
                image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone()).unwrap();
            for y in 0..16 {
                for x in 0..16 {
                    pixels.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
                }
            }
            Ok(pixels)
        };
        export_edit(
            &source,
            &destination,
            &state,
            false,
            &cancel,
            compose,
            |_| {},
        )
        .unwrap();
        let mut edited = VideoReader::open(&destination).unwrap();
        assert!(edited.info.audio);
        assert!((edited.info.duration - 1.).abs() < 0.15);
        let first = edited.frame_at(0.1).unwrap();
        let last = edited.frame_at(0.8).unwrap();
        let center = (32 * 96 + 48) * 4;
        assert!(first.rgba[center] > 160);
        assert!(last.rgba[center + 2] > 160);
        assert!(last.rgba[0] < 20 && last.rgba[1] < 20 && last.rgba[2] < 20);
        export_edit(&source, &gif, &state, true, &cancel, compose, |_| {}).unwrap();
        let mut animation = VideoReader::open(&gif).unwrap();
        assert!(!animation.info.audio);
        let last = animation.frame_at(0.8).unwrap();
        assert!(last.rgba[center + 2] > 160);
        let first = animation.frame_at(0.1).unwrap();
        assert!(first.rgba[center] > 160);
        let selected = directory.join("selected.mp4");
        let options = glint_core::media_export::VideoExport {
            fps: Some(10),
            audio_bitrate: None,
            ..Default::default()
        };
        export_edit_with_options(
            &source,
            &selected,
            &state,
            false,
            options,
            &cancel,
            compose,
            |_| {},
        )
        .unwrap();
        let selected_reader = VideoReader::open(&selected).unwrap();
        assert_eq!(selected_reader.info.fps, 10);
        assert!(!selected_reader.info.audio);
        drop(selected_reader);
        for rate in [128_000, 256_000] {
            let path = directory.join(format!("audio-{rate}.mp4"));
            let settings = glint_core::media_export::VideoExport {
                audio_bitrate: Some(rate),
                ..options
            };
            export_edit_with_options(
                &source,
                &path,
                &state,
                false,
                settings,
                &cancel,
                compose,
                |_| {},
            )
            .unwrap();
            let path: Vec<u16> = crate::interop::wide(&path);
            let input =
                unsafe { MFCreateSourceReaderFromURL(windows::core::PCWSTR(path.as_ptr()), None) }
                    .unwrap();
            let media = unsafe {
                input.GetNativeMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, 0)
            }
            .unwrap();
            assert_eq!(
                unsafe { media.GetGUID(&MF_MT_SUBTYPE) }.unwrap(),
                MFAudioFormat_AAC
            );
            assert_eq!(
                unsafe { media.GetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND) }.unwrap() * 8,
                rate
            );
        }
        let codecs = super::super::codecs::export_codecs();
        assert!(
            codecs.h264.is_ok(),
            "H.264 sample encoder failed: {:?}",
            codecs.h264
        );
        if codecs.hevc.is_ok() {
            let hevc = directory.join("hevc.mp4");
            let mut resized = state.clone();
            resized.transform.size = Some((256, 144));
            let options = glint_core::media_export::VideoExport {
                codec: glint_core::media_export::VideoCodec::Hevc,
                ..options
            };
            export_edit_with_options(
                &source,
                &hevc,
                &resized,
                false,
                options,
                &cancel,
                |frame, time| {
                    compose(frame, time).map(|pixels| {
                        image::imageops::resize(
                            &pixels,
                            256,
                            144,
                            image::imageops::FilterType::Triangle,
                        )
                    })
                },
                |_| {},
            )
            .unwrap();
            let path: Vec<u16> = crate::interop::wide(&hevc);
            let input =
                unsafe { MFCreateSourceReaderFromURL(windows::core::PCWSTR(path.as_ptr()), None) }
                    .unwrap();
            let media = unsafe {
                input.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0)
            }
            .unwrap();
            assert_eq!(
                unsafe { media.GetGUID(&MF_MT_SUBTYPE) }.unwrap(),
                MFVideoFormat_HEVC
            );
        } else {
            eprintln!(
                "HEVC sample encoding unavailable on this system: {:?}",
                codecs.hevc
            );
        }
        if let Ok(component) = super::super::containers::component_path() {
            use glint_core::media_export::{VideoCodec, VideoContainer, VideoExport};
            let ffprobe = component.with_file_name("ffprobe.exe");
            for (container, codec, available) in [
                (VideoContainer::Mov, VideoCodec::H264, &codecs.mov_h264),
                (VideoContainer::Mov, VideoCodec::Hevc, &codecs.mov_hevc),
                (VideoContainer::Mkv, VideoCodec::H264, &codecs.mkv_h264),
                (VideoContainer::Mkv, VideoCodec::Hevc, &codecs.mkv_hevc),
                (VideoContainer::WebM, VideoCodec::Vp9, &codecs.webm),
            ] {
                if let Err(error) = available {
                    eprintln!("Unavailable {container:?}/{codec:?}: {error}");
                    continue;
                }
                let destination =
                    directory.join(format!("edited-{codec:?}.{}", container.extension()));
                let options = VideoExport {
                    container,
                    codec,
                    fps: Some(10),
                    ..Default::default()
                };
                export_edit_with_options(
                    &source,
                    &destination,
                    &state,
                    false,
                    options,
                    &cancel,
                    compose,
                    |_| {},
                )
                .unwrap();
                let bytes = std::fs::read(&destination).unwrap();
                if container == VideoContainer::Mov {
                    assert_eq!(&bytes[4..8], b"ftyp");
                    assert_eq!(&bytes[8..12], b"qt  ");
                } else {
                    assert_eq!(&bytes[..4], &[0x1a, 0x45, 0xdf, 0xa3]);
                    let doc = if container == VideoContainer::WebM {
                        "webm"
                    } else {
                        "matroska"
                    };
                    assert!(bytes.windows(doc.len()).any(|w| w == doc.as_bytes()));
                }
                if ffprobe.is_file() {
                    let info = std::process::Command::new(&ffprobe)
                        .args([
                            "-v",
                            "error",
                            "-show_entries",
                            "stream=codec_name,avg_frame_rate:format=duration",
                            "-of",
                            "default=noprint_wrappers=1",
                        ])
                        .arg(&destination)
                        .output()
                        .unwrap();
                    assert!(info.status.success());
                    let info = String::from_utf8(info.stdout).unwrap();
                    assert!(
                        info.contains(match codec {
                            VideoCodec::H264 => "codec_name=h264",
                            VideoCodec::Hevc => "codec_name=hevc",
                            VideoCodec::Vp9 => "codec_name=vp9",
                        }),
                        "{info}"
                    );
                    assert!(
                        info.contains(if codec == VideoCodec::Vp9 {
                            "codec_name=opus"
                        } else {
                            "codec_name=aac"
                        }),
                        "{info}"
                    );
                    assert!(info.contains("avg_frame_rate=10/1"), "{info}");
                    let duration: f64 = info
                        .lines()
                        .find_map(|l| l.strip_prefix("duration="))
                        .unwrap()
                        .parse()
                        .unwrap();
                    assert!((duration - 1.).abs() < 0.2, "{info}");
                }
                // Decode edited pixels through the independent component, including timed concealment.
                let decoded = std::process::Command::new(&component)
                    .args(["-v", "error", "-i"])
                    .arg(&destination)
                    .args([
                        "-map",
                        "0:v:0",
                        "-frames:v",
                        "1",
                        "-f",
                        "rawvideo",
                        "-pix_fmt",
                        "rgba",
                        "pipe:1",
                    ])
                    .output()
                    .unwrap();
                assert!(decoded.status.success());
                assert!(decoded.stdout.len() >= 96 * 64 * 4);
                assert!(decoded.stdout[0] < 25 && decoded.stdout[1] < 25 && decoded.stdout[2] < 25);
                eprintln!(
                    "Verified {container:?}/{codec:?}: real container, video/audio codecs, 10 FPS, duration and flattened concealment"
                );
                std::fs::remove_file(&destination).unwrap();
            }
            // Cancellation while feeding the WebM pipe must kill the child and remove its workspace.
            if codecs.webm.is_ok() {
                let path = directory.join("fractional.webm");
                let mut fractional = state.clone();
                fractional.clips = (0..8)
                    .map(|i| glint_core::workbench::Clip {
                        start: i as f64 * 0.25,
                        end: i as f64 * 0.25 + 0.11,
                    })
                    .collect();
                let options = VideoExport {
                    container: VideoContainer::WebM,
                    codec: VideoCodec::Vp9,
                    fps: Some(10),
                    ..Default::default()
                };
                export_edit_with_options(
                    &source,
                    &path,
                    &fractional,
                    false,
                    options,
                    &cancel,
                    compose,
                    |_| {},
                )
                .unwrap();
                if ffprobe.is_file() {
                    let info = std::process::Command::new(&ffprobe)
                        .args([
                            "-v",
                            "error",
                            "-select_streams",
                            "v:0",
                            "-count_frames",
                            "-show_entries",
                            "stream=nb_read_frames",
                            "-of",
                            "csv=p=0",
                        ])
                        .arg(&path)
                        .output()
                        .unwrap();
                    assert!(info.status.success());
                    assert_eq!(
                        String::from_utf8(info.stdout).unwrap().trim(),
                        "9",
                        "Partial frames in each clip must share one output clock"
                    );
                }
                std::fs::remove_file(&path).unwrap();
                let pending = directory.join("cancel.webm");
                let options = VideoExport {
                    container: VideoContainer::WebM,
                    codec: VideoCodec::Vp9,
                    ..Default::default()
                };
                let cancelled = export_edit_with_options(
                    &source,
                    &pending,
                    &state,
                    false,
                    options,
                    &cancel,
                    |frame, time| {
                        cancel.store(true, Ordering::Release);
                        compose(frame, time)
                    },
                    |_| {},
                );
                assert!(cancelled.is_err());
                assert!(!pending.exists());
                cancel.store(false, Ordering::Release);
                assert!(!std::fs::read_dir(&directory).unwrap().any(|e| {
                    e.unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".glint-media-")
                }));
            }
            if codecs.mov_h264.is_ok() {
                let existing = directory.join("existing.mov");
                std::fs::write(&existing, b"preserve").unwrap();
                let options = VideoExport {
                    container: VideoContainer::Mov,
                    ..Default::default()
                };
                assert!(
                    export_edit_with_options(
                        &source,
                        &existing,
                        &state,
                        false,
                        options,
                        &cancel,
                        compose,
                        |_| {}
                    )
                    .is_err()
                );
                assert_eq!(std::fs::read(&existing).unwrap(), b"preserve");
                std::fs::remove_file(existing).unwrap();
                assert!(!std::fs::read_dir(&directory).unwrap().any(|e| {
                    e.unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".glint-media-")
                }));
            }
        }
        cancel.store(true, std::sync::atomic::Ordering::Release);
        assert!(
            export_edit(
                &source,
                &directory.join("cancelled.mp4"),
                &state,
                false,
                &cancel,
                compose,
                |_| {}
            )
            .is_err()
        );
        drop(animation);
        drop(edited);
        drop(reader);
        let _ = std::fs::remove_dir_all(directory);
    }

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
            let path: Vec<u16> = crate::interop::wide(path);
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
            for _frame_index in 0..3000u32 {
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
                let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(width * 4);
                let surface = SampleSurface::lock(&sample, stride).unwrap().with_geometry(
                    width,
                    height,
                    stride as i32,
                );
                coded = (width, height);
                let mut padding_hit = false;
                for (x, y, pixel) in surface.pixels() {
                    let (r, g, b) = (pixel[2], pixel[1], pixel[0]);
                    // Columns and rows past the display aperture are
                    // encoder padding from macroblock alignment.
                    if x >= 696 || y >= 318 {
                        if !(r < 8 && g < 8 && b < 8) {
                            padding_hit = true;
                            if padding.len() < 20 {
                                padding.push((x, y, [r, g, b]));
                            }
                        }
                        continue;
                    }
                    if g > 140 && r < 110 && b < 110 && g - r > 50 && g - b > 50 {
                        let entry = report.entry((x, y)).or_insert((0, [0; 3]));
                        entry.0 += 1;
                        entry.1 = [r, g, b];
                        hits += 1;
                    }
                }
                if padding_hit {
                    padding_frames += 1;
                }
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
        let _media = Media;
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
            let path_wide: Vec<u16> = crate::interop::wide(&path);
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
                let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(w * 4);
                let surface = SampleSurface::lock(&sample, stride).unwrap().with_geometry(
                    w,
                    h,
                    stride as i32,
                );
                for (x, y, pixel) in surface.pixels() {
                    let (r, g, b) = (pixel[2], pixel[1], pixel[0]);
                    if g > 140 && r < 110 && b < 110 && g - r > 50 && g - b > 50 {
                        hits += 1;
                    }
                    // The padding must replicate the frame's right edge.
                    if x >= width && y < height {
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
        let _media = Media;
        let mut artifact = 0usize;
        let mut frames = 0usize;
        let mut coded = (0u32, 0u32);
        let mut padding_mismatch = 0usize;
        let mut padding_total = 0usize;
        let mut display_green = 0usize;
        unsafe {
            let path: Vec<u16> = crate::interop::wide(path);
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
                            let edge = y * pitch + 695 * 4;
                            if edge + 3 < bytes.len()
                                && (pixel[0].abs_diff(bytes[edge]) > 20
                                    || pixel[1].abs_diff(bytes[edge + 1]) > 20
                                    || pixel[2].abs_diff(bytes[edge + 2]) > 20)
                            {
                                padding_mismatch += 1;
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
        drop(_media);
        println!(
            "frames={frames} coded={coded:?} artifact={artifact} padding-mismatch={padding_mismatch}/{padding_total} display-green={display_green}"
        );
        assert!(frames >= 100, "recording too short: {frames} frames");
        assert_eq!(artifact, 0, "green edge artifact detected");
    }
}
