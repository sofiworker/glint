//! Synchronous Media Foundation encoding. Owned exclusively by the bounded consumer.
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    Win32::Media::MediaFoundation::*,
    core::{PCWSTR, Result},
};

pub struct Encoder {
    writer: IMFSinkWriter,
    video: u32,
    audio: Option<u32>,
}
fn pair(a: u32, b: u32) -> u64 {
    (u64::from(a) << 32) | u64::from(b)
}
impl Encoder {
    pub fn new(path: &Path, width: u32, height: u32, fps: u32, audio: bool) -> Result<Self> {
        unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 3)?;
            let attributes = attributes.unwrap();
            attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let writer = MFCreateSinkWriterFromURL(PCWSTR(path.as_ptr()), None, &attributes)?;
            let output = MFCreateMediaType()?;
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            output.SetUINT32(
                &MF_MT_AVG_BITRATE,
                (u64::from(width) * u64::from(height) * u64::from(fps) / 6)
                    .clamp(2_000_000, 40_000_000) as u32,
            )?;
            output.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            output.SetUINT64(&MF_MT_FRAME_SIZE, pair(width, height))?;
            output.SetUINT64(&MF_MT_FRAME_RATE, pair(fps, 1))?;
            output.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pair(1, 1))?;
            let video = writer.AddStream(&output)?;
            let input = MFCreateMediaType()?;
            output.CopyAllItems(&input)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            input.SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4)?;
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
        self.sample(self.video, bytes, time, duration)
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
