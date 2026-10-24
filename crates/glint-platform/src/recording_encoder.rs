//! Synchronous Media Foundation encoding. Owned exclusively by the bounded consumer.
use std::{cell::RefCell, os::windows::ffi::OsStrExt, path::Path};
use windows::{
    Win32::Media::MediaFoundation::*,
    core::{PCWSTR, Result},
};

pub struct Encoder {
    writer: IMFSinkWriter,
    video: u32,
    audio: Option<u32>,
    width: u32,
    height: u32,
    coded_width: u32,
    coded_height: u32,
    frame_bytes: usize,
    padded: RefCell<Vec<u8>>,
}
fn pair(a: u32, b: u32) -> u64 {
    (u64::from(a) << 32) | u64::from(b)
}
/// H.264 encodes macroblock-aligned surfaces; a region smaller than that gets
/// padding the encoder leaves uninitialised, which decodes as a bright green
/// edge in players that ignore the crop metadata. Extending the frame with its
/// own edge pixels keeps every coded pixel defined.
fn aligned(size: u32) -> u32 {
    size.div_ceil(16) * 16
}
impl Encoder {
    pub fn new(path: &Path, width: u32, height: u32, fps: u32, audio: bool) -> Result<Self> {
        Self::new_with_hardware(path, width, height, fps, audio, true)
    }
    pub fn new_with_hardware(
        path: &Path,
        width: u32,
        height: u32,
        fps: u32,
        audio: bool,
        hardware: bool,
    ) -> Result<Self> {
        let coded_width = aligned(width);
        let coded_height = aligned(height);
        unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 3)?;
            let attributes = attributes.unwrap();
            attributes.SetUINT32(
                &MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
                u32::from(hardware),
            )?;
            let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let writer = MFCreateSinkWriterFromURL(PCWSTR(path.as_ptr()), None, &attributes)?;
            let output = MFCreateMediaType()?;
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            output.SetUINT32(
                &MF_MT_AVG_BITRATE,
                (u64::from(coded_width) * u64::from(coded_height) * u64::from(fps) / 6)
                    .clamp(2_000_000, 40_000_000) as u32,
            )?;
            output.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            output.SetUINT64(&MF_MT_FRAME_SIZE, pair(coded_width, coded_height))?;
            output.SetUINT64(&MF_MT_FRAME_RATE, pair(fps, 1))?;
            output.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pair(1, 1))?;
            let video = writer.AddStream(&output)?;
            let input = MFCreateMediaType()?;
            output.CopyAllItems(&input)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            input.SetUINT32(&MF_MT_DEFAULT_STRIDE, coded_width * 4)?;
            writer.SetInputMediaType(video, &input, None)?;
            let audio = if audio {
                let output = MFCreateMediaType()?;
                output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                output.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
                output.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
                output.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
                output.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
                output.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24000)?;
                let index = writer.AddStream(&output)?;
                let input = MFCreateMediaType()?;
                input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
                input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
                input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
                input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
                input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
                input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192000)?;
                writer.SetInputMediaType(index, &input, None)?;
                Some(index)
            } else {
                None
            };
            writer.BeginWriting()?;
            Ok(Self {
                writer,
                video,
                audio,
                width,
                height,
                coded_width,
                coded_height,
                frame_bytes: width as usize * height as usize * 4,
                padded: RefCell::new(Vec::new()),
            })
        }
    }
    fn sample(&self, index: u32, bytes: &[u8], time: i64, duration: i64) -> Result<()> {
        unsafe {
            let buffer = MFCreateMemoryBuffer(bytes.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(bytes.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(duration.max(1))?;
            self.writer.WriteSample(index, &sample)
        }
    }
    pub fn video(&self, bytes: &[u8], time: i64, duration: i64) -> Result<()> {
        if bytes.len() != self.frame_bytes {
            return Err(windows::core::Error::from(
                windows::Win32::Foundation::E_INVALIDARG,
            ));
        }
        if self.coded_width == self.width && self.coded_height == self.height {
            return self.sample(self.video, bytes, time, duration);
        }
        // Extend the frame with its own right/bottom edge pixels so the coded
        // surface contains no undefined region. Row-major RGB32: replicate the
        // last pixel of each row, then replicate the final row.
        let (w, h, cw, ch) = (
            self.width as usize,
            self.height as usize,
            self.coded_width as usize,
            self.coded_height as usize,
        );
        let mut padded = self.padded.borrow_mut();
        padded.clear();
        padded.resize(cw * ch * 4, 0);
        for row in 0..h {
            let src = row * w * 4;
            let dst = row * cw * 4;
            padded[dst..dst + w * 4].copy_from_slice(&bytes[src..src + w * 4]);
            let last = &bytes[src + (w - 1) * 4..src + w * 4];
            for x in w..cw {
                padded[dst + x * 4..dst + x * 4 + 4].copy_from_slice(last);
            }
        }
        let last_row_at = (h - 1) * cw * 4;
        let last_row = padded[last_row_at..last_row_at + cw * 4].to_vec();
        for row in h..ch {
            padded[row * cw * 4..row * cw * 4 + cw * 4].copy_from_slice(&last_row);
        }
        self.sample(self.video, &padded, time, duration)
    }
    pub fn audio(&self, bytes: &[u8], frame: u64) -> Result<()> {
        if let Some(index) = self.audio {
            self.sample(
                index,
                bytes,
                (frame * 10_000_000 / 48000) as i64,
                (bytes.len() as u64 / 4 * 10_000_000 / 48000) as i64,
            )?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<()> {
        unsafe { self.writer.Finalize() }
    }
}
