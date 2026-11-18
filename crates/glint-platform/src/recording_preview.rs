//! Bounded PCM preview. The output device clock starts the canvas playback clock.
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::*,
    Media::{Audio::*, MediaFoundation::*},
    System::{Com::*, Threading::*},
};

static BUSY: AtomicBool = AtomicBool::new(false);
pub struct AudioPreview {
    stop: Arc<AtomicBool>,
    started: Arc<Mutex<Option<Instant>>>,
    error: Arc<Mutex<Option<String>>>,
}
impl Drop for AudioPreview {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl AudioPreview {
    pub fn start(
        path: PathBuf,
        state: glint_core::workbench::State,
        position: f64,
    ) -> Result<Self, String> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Audio preview is still stopping")?;
        let this = Self {
            stop: Arc::new(AtomicBool::new(false)),
            started: Arc::new(Mutex::new(None)),
            error: Arc::new(Mutex::new(None)),
        };
        let stop = this.stop.clone();
        let started = this.started.clone();
        let error = this.error.clone();
        let spawn = std::thread::Builder::new()
            .name("glint-editor-audio".into())
            .spawn(move || {
                struct Slot;
                impl Drop for Slot {
                    fn drop(&mut self) {
                        BUSY.store(false, Ordering::Release);
                    }
                }
                let _slot = Slot;
                let result = play(&path, &state, position, &stop, &started);
                if let Err(e) = result {
                    *error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e);
                }
            });
        if let Err(e) = spawn {
            BUSY.store(false, Ordering::Release);
            return Err(e.to_string());
        }
        Ok(this)
    }
    pub fn elapsed(&self) -> Option<Duration> {
        self.started
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .map(|at| at.elapsed())
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).take()
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
struct Device {
    wave: HWAVEOUT,
    event: HANDLE,
}
impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            waveOutReset(self.wave);
            waveOutClose(self.wave);
            let _ = CloseHandle(self.event);
        }
    }
}
impl Device {
    fn new() -> Result<Self, String> {
        unsafe {
            let event = CreateEventW(None, false, false, None).map_err(|e| e.to_string())?;
            let format = WAVEFORMATEX {
                wFormatTag: 1,
                nChannels: 2,
                nSamplesPerSec: 48000,
                nAvgBytesPerSec: 192000,
                nBlockAlign: 4,
                wBitsPerSample: 16,
                cbSize: 0,
            };
            let mut wave = HWAVEOUT::default();
            let e = waveOutOpen(
                Some(&mut wave),
                WAVE_MAPPER,
                &format,
                event.0 as usize,
                0,
                CALLBACK_EVENT,
            );
            if e != 0 {
                let _ = CloseHandle(event);
                return Err(format!("Audio output unavailable ({e})"));
            }
            Ok(Self { wave, event })
        }
    }
    fn write(
        &self,
        bytes: &mut [u8],
        stop: &AtomicBool,
        started: &Mutex<Option<Instant>>,
    ) -> Result<(), String> {
        unsafe {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let mut header = WAVEHDR {
                lpData: windows::core::PSTR(bytes.as_mut_ptr()),
                dwBufferLength: bytes.len() as u32,
                ..Default::default()
            };
            let size = std::mem::size_of::<WAVEHDR>() as u32;
            let e = waveOutPrepareHeader(self.wave, &mut header, size);
            if e != 0 {
                return Err(format!("Audio prepare failed ({e})"));
            }
            let e = waveOutWrite(self.wave, &mut header, size);
            if e == 0 {
                let mut clock = started.lock().unwrap_or_else(|e| e.into_inner());
                if clock.is_none() {
                    *clock = Some(Instant::now());
                }
                drop(clock);
            }
            while e == 0 && std::ptr::read_volatile(&raw const header.dwFlags) & WHDR_DONE == 0 {
                if stop.load(Ordering::Acquire) {
                    waveOutReset(self.wave);
                    break;
                }
                let _ = WaitForSingleObject(self.event, 20);
            }
            let _ = waveOutUnprepareHeader(self.wave, &mut header, size);
            if e == 0 {
                Ok(())
            } else {
                Err(format!("Audio playback failed ({e})"))
            }
        }
    }
}
fn play(
    path: &std::path::Path,
    state: &glint_core::workbench::State,
    position: f64,
    stop: &AtomicBool,
    started: &Mutex<Option<Instant>>,
) -> Result<(), String> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(|e| e.to_string())?;
    if let Err(e) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
        unsafe {
            CoUninitialize();
        }
        return Err(e.to_string());
    }
    let _media = Media;
    let reader = unsafe { super::output::AudioReader::open(path) }.map_err(|e| e.to_string())?;
    let device = Device::new()?;
    let mut offset = 0.;
    for clip in &state.clips {
        let skip = (position - offset).max(0.);
        offset += clip.duration();
        if skip >= clip.duration() {
            continue;
        }
        let start = clip.start + skip;
        unsafe {
            reader.reader.SetCurrentPosition(
                &windows::core::GUID::zeroed(),
                &windows::core::PROPVARIANT::from((start * 10_000_000.) as i64),
            )
        }
        .map_err(|e| e.to_string())?;
        let mut written = 0u64;
        let wanted = ((clip.end - start) * 48000.).round() as u64;
        while written < wanted && !stop.load(Ordering::Acquire) {
            let Some((time, bytes)) = unsafe { reader.packet() }.map_err(|e| e.to_string())? else {
                break;
            };
            let left = ((start - time).max(0.) * 48000.).ceil() as usize;
            let right =
                (((clip.end - time).max(0.) * 48000.).floor() as usize).min(bytes.len() / 4);
            if right <= left {
                if time >= clip.end {
                    break;
                }
                continue;
            }
            let at = ((time + left as f64 / 48000. - start).max(0.) * 48000.).round() as u64;
            while written < at.min(wanted) && !stop.load(Ordering::Acquire) {
                let n = (at.min(wanted) - written).min(2400);
                device.write(&mut vec![0; n as usize * 4], stop, started)?;
                written += n;
            }
            let left = left.saturating_add(written.saturating_sub(at) as usize);
            if right > left {
                let mut pcm = bytes[left * 4..right * 4].to_vec();
                for v in pcm.as_chunks_mut::<2>().0 {
                    let n = i16::from_le_bytes([v[0], v[1]]);
                    let n = (i32::from(n) * i32::from(state.volume) / 100)
                        .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                    v.copy_from_slice(&n.to_le_bytes());
                }
                for chunk in pcm.chunks_mut(9600) {
                    device.write(chunk, stop, started)?;
                    written += (chunk.len() / 4) as u64;
                }
            }
        }
        while written < wanted && !stop.load(Ordering::Acquire) {
            let n = (wanted - written).min(2400);
            device.write(&mut vec![0; n as usize * 4], stop, started)?;
            written += n;
        }
    }
    Ok(())
}
