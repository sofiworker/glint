//! Explicitly enabled WASAPI endpoints. Silence buffers never dereference null.
use super::{Com, Shared};
use std::sync::{atomic::Ordering, mpsc::SyncSender};
use windows::Win32::{
    Foundation::*,
    Media::Audio::*,
    System::{Com::*, Threading::*},
};

pub(super) struct Packet {
    pub start: u64,
    pub samples: Vec<i16>,
}
struct Event(HANDLE);
impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Stream(IAudioClient);
impl Drop for Stream {
    fn drop(&mut self) {
        unsafe {
            let _ = self.0.Stop();
        }
    }
}

pub(super) fn run(loopback: bool, shared: Shared, send: SyncSender<Packet>) {
    if let Err(error) = capture(loopback, &shared, send) {
        shared.fail(format!(
            "{}: {error}",
            if loopback {
                "System audio"
            } else {
                "Microphone"
            }
        ));
    }
}
fn capture(loopback: bool, shared: &Shared, send: SyncSender<Packet>) -> Result<(), String> {
    let _com = Com::new()?;
    let work = || -> windows::core::Result<()> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator
                .GetDefaultAudioEndpoint(if loopback { eRender } else { eCapture }, eConsole)?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
            let format = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16,
                nChannels: 2,
                nSamplesPerSec: 48000,
                nAvgBytesPerSec: 192000,
                nBlockAlign: 4,
                wBitsPerSample: 16,
                cbSize: 0,
            };
            let flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
                | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                | if loopback {
                    AUDCLNT_STREAMFLAGS_LOOPBACK
                } else {
                    0
                };
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, &format, None)?;
            let event = Event(CreateEventW(None, false, false, None)?);
            client.SetEventHandle(event.0)?;
            let reader: IAudioCaptureClient = client.GetService()?;
            client.Start()?;
            let _stream = Stream(client);
            while !shared.stop.load(Ordering::Acquire) {
                let wait = WaitForSingleObject(event.0, 100);
                if wait == WAIT_FAILED {
                    return Err(windows::core::Error::from_win32());
                }
                while reader.GetNextPacketSize()? > 0 {
                    let mut pointer = std::ptr::null_mut();
                    let mut frames = 0;
                    let mut flags = 0;
                    reader.GetBuffer(&mut pointer, &mut frames, &mut flags, None, None)?;
                    let packet = if frames <= 48000 {
                        shared.position().map(|position| {
                            let samples = if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0
                                || pointer.is_null()
                            {
                                vec![0; frames as usize * 2]
                            } else {
                                std::slice::from_raw_parts(
                                    pointer as *const i16,
                                    frames as usize * 2,
                                )
                                .to_vec()
                            };
                            Packet {
                                start: (position.as_micros() as u64 * 48000 / 1_000_000)
                                    .saturating_sub(u64::from(frames)),
                                samples,
                            }
                        })
                    } else {
                        None
                    };
                    reader.ReleaseBuffer(frames)?;
                    if flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0 {
                        shared.stats.audio_dropped.fetch_add(1, Ordering::Relaxed);
                    }
                    if let Some(packet) = packet
                        && send.try_send(packet).is_err()
                    {
                        shared.stats.audio_dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Ok(())
        }
    };
    work().map_err(|e| e.to_string())
}
