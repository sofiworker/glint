//! Windows Graphics Capture -> three-frame queue -> Media Foundation H.264/AAC.
//! WASAPI runs separately, with bounded packets and a shared pause-aware clock.
use glint_core::recording::Timeline;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    time::Duration,
};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::*,
    window::Window,
};
#[path = "recording_audio.rs"]
mod audio;
#[path = "recording_encoder.rs"]
mod encoder;
#[path = "recording_output.rs"]
mod output;
pub use output::{copy_file, play_file, poster};

static BUSY: AtomicBool = AtomicBool::new(false);
struct Slot;
impl Drop for Slot {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
pub type Region = (i32, i32, u32, u32);
pub fn exclude_controller(handle: usize) -> Result<(), String> {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity(
            windows::Win32::Foundation::HWND(handle as *mut _),
            windows::Win32::UI::WindowsAndMessaging::WDA_EXCLUDEFROMCAPTURE,
        )
    }
    .map_err(|e| e.to_string())
}
pub fn open_directory(directory: &std::path::Path) -> Result<(), String> {
    let directory = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe")
        .arg(directory)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[derive(Clone)]
pub struct Source {
    pub name: String,
    pub window: bool,
    handle: usize,
    pub width: u32,
    pub height: u32,
}
pub fn sources() -> Result<Vec<Source>, String> {
    let mut sources = Vec::new();
    for monitor in Monitor::enumerate().map_err(|e| e.to_string())? {
        sources.push(Source {
            name: monitor.name().map_err(|e| e.to_string())?,
            window: false,
            handle: monitor.as_raw_hmonitor() as usize,
            width: monitor.width().map_err(|e| e.to_string())?,
            height: monitor.height().map_err(|e| e.to_string())?,
        });
    }
    for window in Window::enumerate().map_err(|e| e.to_string())? {
        let mut pid = 0;
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                windows::Win32::Foundation::HWND(window.as_raw_hwnd()),
                Some(&mut pid),
            );
        }
        if pid == std::process::id() {
            continue;
        }
        let name = window.title().unwrap_or_default();
        let width = window.width().unwrap_or_default().max(0) as u32;
        let height = window.height().unwrap_or_default().max(0) as u32;
        if !name.is_empty() && width >= 16 && height >= 16 {
            sources.push(Source {
                name,
                window: true,
                handle: window.as_raw_hwnd() as usize,
                width,
                height,
            });
        }
    }
    Ok(sources)
}
pub fn region_source(region: Region) -> Result<(Source, (u32, u32)), String> {
    use windows::Win32::{Foundation::RECT, Graphics::Gdi::*};
    let (x, y, w, h) = region;
    let right = i64::from(x) + i64::from(w);
    let bottom = i64::from(y) + i64::from(h);
    if w < 16 || h < 16 || right > i64::from(i32::MAX) || bottom > i64::from(i32::MAX) {
        return Err("Invalid recording region".into());
    }
    unsafe {
        let rect = RECT {
            left: x,
            top: y,
            right: right as i32,
            bottom: bottom as i32,
        };
        let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONULL);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if monitor.is_invalid()
            || !GetMonitorInfoW(monitor, &mut info).as_bool()
            || x < info.rcMonitor.left
            || y < info.rcMonitor.top
            || rect.right > info.rcMonitor.right
            || rect.bottom > info.rcMonitor.bottom
        {
            return Err("The recording region must fit one display".into());
        }
        Ok((
            Source {
                name: format!("{x}, {y} · {w} × {h}"),
                window: false,
                handle: monitor.0 as usize,
                width: w,
                height: h,
            },
            (
                (x - info.rcMonitor.left) as u32,
                (y - info.rcMonitor.top) as u32,
            ),
        ))
    }
}
pub struct Options {
    pub source: Source,
    pub crop: Option<(u32, u32)>,
    pub fps: u32,
    pub format: Format,
    pub cursor: bool,
    pub system_audio: bool,
    pub microphone: bool,
    pub directory: PathBuf,
    pub exclusions: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Mp4,
    Gif,
}
#[derive(Default)]
pub struct Stats {
    pub frames: AtomicU64,
    pub dropped: AtomicU64,
    pub audio_dropped: AtomicU64,
}
#[derive(Clone)]
pub(super) struct Shared {
    stop: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    clock: Arc<Mutex<Timeline>>,
    stats: Arc<Stats>,
    error: Arc<Mutex<Option<String>>>,
}
impl Shared {
    fn fail(&self, error: String) {
        self.clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pause(true);
        *self.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
        self.stop.store(true, Ordering::Release);
    }
    fn position(&self) -> Option<Duration> {
        let clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        (!clock.is_paused()).then(|| clock.elapsed())
    }
}
pub struct Recorder {
    shared: Shared,
}
impl Recorder {
    pub fn start(
        options: Options,
        ready: impl FnOnce(Result<PathBuf, String>) + Send + 'static,
    ) -> Result<Self, String> {
        if BUSY.swap(true, Ordering::AcqRel) {
            return Err("A recording is already running or finishing".into());
        }
        let slot = Slot;
        if ![15, 30, 60, 120, 144].contains(&options.fps)
            || options.source.width < 16
            || options.source.height < 16
            || u64::from(options.source.width) * u64::from(options.source.height) * 4
                > 64 * 1024 * 1024
        {
            return Err("Invalid recording dimensions or frame rate".into());
        }
        if options.format == Format::Gif
            && (options.fps > 30 || options.system_audio || options.microphone)
        {
            return Err("GIF recording supports up to 30 FPS and does not support audio".into());
        }
        let shared = Shared {
            stop: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            clock: Arc::new(Mutex::new(Timeline::default())),
            stats: Arc::new(Stats::default()),
            error: Arc::new(Mutex::new(None)),
        };
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("glint-recording".into())
            .spawn(move || {
                let _slot = slot;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| record(options, worker)))
                    .unwrap_or_else(|_| Err("The recording worker stopped unexpectedly; check the output directory for a partial file".into()));
                drop(_slot);
                ready(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { shared })
    }
    pub fn pause(&self, pause: bool) {
        self.shared
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pause(pause);
    }
    pub fn stop(&self, cancel: bool) {
        self.shared
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pause(true);
        if cancel {
            self.shared.cancel.store(true, Ordering::Release);
        }
        self.shared.stop.store(true, Ordering::Release);
    }
    pub fn elapsed(&self) -> Duration {
        self.shared
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
    }
    pub fn stats(&self) -> &Stats {
        &self.shared.stats
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
    }
}
enum Target {
    Window(usize),
    Monitor(usize),
}
impl TryInto<GraphicsCaptureItemType> for Target {
    type Error = String;
    fn try_into(self) -> Result<GraphicsCaptureItemType, String> {
        match self {
            Self::Window(handle) => {
                let item: Result<GraphicsCaptureItemType, _> =
                    Window::from_raw_hwnd(handle as *mut _).try_into();
                item.map_err(|e| e.to_string())
            }
            Self::Monitor(handle) => {
                let item: Result<GraphicsCaptureItemType, _> =
                    Monitor::from_raw_hmonitor(handle as *mut _).try_into();
                item.map_err(|e| e.to_string())
            }
        }
    }
}
struct Video {
    pixels: Vec<u8>,
}
struct Flags {
    shared: Shared,
    sender: SyncSender<Video>,
    crop: Option<(u32, u32)>,
    width: u32,
    height: u32,
    fps: u32,
    exclusions: Vec<String>,
}
struct Capture {
    flags: Flags,
    last: Option<Duration>,
}
impl GraphicsCaptureApiHandler for Capture {
    type Flags = Flags;
    type Error = String;
    fn new(ctx: Context<Flags>) -> Result<Self, String> {
        Ok(Self {
            flags: ctx.flags,
            last: None,
        })
    }
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), String> {
        let f = &self.flags;
        if f.shared.stop.load(Ordering::Acquire) {
            control.stop();
            return Ok(());
        }
        let Some(time) = f.shared.position() else {
            return Ok(());
        };
        if self.last.is_some_and(|last| {
            time.saturating_sub(last) < Duration::from_secs_f64(1. / f.fps as f64)
        }) {
            return Ok(());
        }
        let result = (|| {
            crate::check_visible_exclusions(&f.exclusions)
                .map_err(|_| "Recording blocked by the excluded-app policy".to_string())?;
            let (x, y) = f.crop.unwrap_or((0, 0));
            if f.crop.is_none()
                && (frame.width() & !1 != f.width || frame.height() & !1 != f.height)
            {
                return Err("Recording source size changed; recording stopped".into());
            }
            if x + f.width > frame.width() || y + f.height > frame.height() {
                return Err("Recording source size changed; recording stopped".into());
            }
            let buffer = frame
                .buffer_crop(x, y, x + f.width, y + f.height)
                .map_err(|e| e.to_string())?;
            let mut scratch = Vec::new();
            let pixels = buffer.as_nopadding_buffer(&mut scratch).to_vec();
            crate::check_visible_exclusions(&f.exclusions)
                .map_err(|_| "Recording blocked by the excluded-app policy".to_string())?;
            match f.sender.try_send(Video { pixels }) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(_)) => {
                    f.shared.stats.dropped.fetch_add(1, Ordering::Relaxed);
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("Recording encoder disconnected".into());
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            f.shared.fail(error);
            control.stop();
        }
        self.last = Some(time);
        Ok(())
    }
    fn on_closed(&mut self) -> Result<(), String> {
        self.flags
            .shared
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pause(true);
        self.flags.shared.stop.store(true, Ordering::Release);
        Ok(())
    }
}
struct Com;
impl Com {
    fn new() -> Result<Self, String> {
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
            .ok()
            .map_err(|e| e.to_string())?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
}
struct Media;
impl Media {
    fn new() -> Result<Self, String> {
        unsafe {
            windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_VERSION,
                0,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(Self)
    }
}
impl Drop for Media {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Media::MediaFoundation::MFShutdown();
        }
    }
}

struct StopOnDrop(Shared);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.stop.store(true, Ordering::Release);
    }
}
fn record(options: Options, shared: Shared) -> Result<PathBuf, String> {
    let _stop = StopOnDrop(shared.clone());
    let _com = Com::new()?;
    let _media = (options.format == Format::Mp4)
        .then(Media::new)
        .transpose()?;
    std::fs::create_dir_all(&options.directory).map_err(|e| e.to_string())?;
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let extension = match options.format {
        Format::Mp4 => "mp4",
        Format::Gif => "gif",
    };
    let temporary = options
        .directory
        .join(format!("Glint-{id}.recording.{extension}"));
    let destination = options.directory.join(format!("Glint-{id}.{extension}"));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = match options.format {
        Format::Mp4 => record_to(&options, shared.clone(), &temporary),
        Format::Gif => record_gif(&options, shared.clone(), &temporary),
    };
    shared.stop.store(true, Ordering::Release);
    if shared.cancel.load(Ordering::Acquire) {
        std::fs::remove_file(&temporary)
            .map_err(|e| format!("Cancel failed: {e}; {}", temporary.display()))?;
        return Ok(PathBuf::new());
    }
    if let Err(error) = result {
        return Err(format!("{error}\nPartial file: {}", temporary.display()));
    }
    std::fs::OpenOptions::new()
        .write(true)
        .open(&temporary)
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("{e}; {}", temporary.display()))?;
    // Same-directory rename with no replacement, including FAT/exFAT destinations.
    use std::os::windows::ffi::OsStrExt;
    let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        windows::Win32::Storage::FileSystem::MoveFileExW(
            windows::core::PCWSTR(from.as_ptr()),
            windows::core::PCWSTR(to.as_ptr()),
            windows::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|e| format!("{e}; recording retained at {}", temporary.display()))?;
    Ok(destination)
}
fn record_to(options: &Options, shared: Shared, path: &std::path::Path) -> Result<(), String> {
    let item: GraphicsCaptureItemType = if options.source.window {
        Target::Window(options.source.handle)
    } else {
        Target::Monitor(options.source.handle)
    }
    .try_into()?;
    let size = match &item {
        GraphicsCaptureItemType::Window((item, _))
        | GraphicsCaptureItemType::Monitor((item, _))
        | GraphicsCaptureItemType::Unknown((item, _)) => item.Size().map_err(|e| e.to_string())?,
    };
    let width = if options.crop.is_some() {
        options.source.width
    } else {
        size.Width.max(0) as u32
    } & !1;
    let height = if options.crop.is_some() {
        options.source.height
    } else {
        size.Height.max(0) as u32
    } & !1;
    if width < 16 || height < 16 || u64::from(width) * u64::from(height) * 4 > 64 * 1024 * 1024 {
        return Err("Invalid recording source size".into());
    }
    let encoder = encoder::Encoder::new(
        path,
        width,
        height,
        options.fps,
        options.system_audio || options.microphone,
    )
    .map_err(|e| e.to_string())?;
    let (send, receive) = mpsc::sync_channel(3);
    let (audio_send, audio_receive) = mpsc::sync_channel(32);
    let mut workers = Vec::new();
    for (enabled, loopback) in [(options.system_audio, true), (options.microphone, false)] {
        if enabled {
            let shared = shared.clone();
            let send = audio_send.clone();
            let failure = shared.clone();
            match std::thread::Builder::new()
                .name(
                    if loopback {
                        "glint-system-audio"
                    } else {
                        "glint-microphone"
                    }
                    .into(),
                )
                .spawn(move || audio::run(loopback, shared, send))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    failure.fail(error.to_string());
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(error.to_string());
                }
            }
        }
    }
    drop(audio_send);
    let flags = Flags {
        shared: shared.clone(),
        sender: send,
        crop: options.crop,
        width,
        height,
        fps: options.fps,
        exclusions: options.exclusions.clone(),
    };
    let start = |flags, item| {
        Capture::start_free_threaded(Settings::new(
            item,
            if options.cursor {
                CursorCaptureSettings::WithCursor
            } else {
                CursorCaptureSettings::WithoutCursor
            },
            DrawBorderSettings::WithBorder,
            SecondaryWindowSettings::Exclude,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            flags,
        ))
        .map_err(|e| e.to_string())
    };
    let item = if options.source.window {
        Target::Window(options.source.handle)
    } else {
        Target::Monitor(options.source.handle)
    };
    let control = match start(flags, item) {
        Ok(control) => control,
        Err(error) => {
            shared.fail(error.clone());
            for worker in workers {
                let _ = worker.join();
            }
            return Err(error);
        }
    };
    let result: Result<(), String> = (|| {
        let mut last: Option<Video> = None;
        let mut audio = BTreeMap::<u64, Vec<i32>>::new();
        let mut audio_frame = 0u64;
        let mut video_time = 0i64;
        let frame_time = 10_000_000 / i64::from(options.fps);
        let first_frame_deadline = std::time::Instant::now();
        while !shared.stop.load(Ordering::Acquire) && !control.is_finished() {
            match receive.recv_timeout(Duration::from_millis(10)) {
                Ok(frame) => {
                    last = Some(frame);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            let elapsed = shared
                .clock
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .elapsed();
            let media_end = (elapsed.as_nanos() / 100) as i64;
            if let Some(frame) = &last {
                if media_end - video_time > frame_time * 3 {
                    let skipped = (media_end - video_time) / frame_time - 2;
                    shared
                        .stats
                        .dropped
                        .fetch_add(skipped as u64, Ordering::Relaxed);
                    video_time += skipped * frame_time;
                }
                while video_time + frame_time <= media_end {
                    encoder
                        .video(&frame.pixels, video_time, frame_time)
                        .map_err(|e| e.to_string())?;
                    shared.stats.frames.fetch_add(1, Ordering::Relaxed);
                    video_time += frame_time;
                }
            } else if first_frame_deadline.elapsed() > Duration::from_secs(5) {
                return Err("No video frames received within five seconds".into());
            }
            let end = elapsed.as_micros() as u64 * 48000 / 1_000_000;
            for packet in audio_receive.try_iter() {
                for (i, sample) in packet.samples.iter().enumerate() {
                    let frame = packet.start + i as u64 / 2;
                    if frame < audio_frame || frame > end + 24000 {
                        continue;
                    }
                    let block = audio.entry(frame / 480).or_insert_with(|| vec![0; 960]);
                    block[(frame % 480) as usize * 2 + i % 2] += i32::from(*sample);
                }
            }
            // Retain at most 110 ms of mixed audio, never a backlog after a stalled encoder.
            if (options.system_audio || options.microphone)
                && end.saturating_sub(audio_frame) > 24000
            {
                let resume = end.saturating_sub(5280) / 480 * 480;
                shared
                    .stats
                    .audio_dropped
                    .fetch_add((resume - audio_frame) / 480, Ordering::Relaxed);
                audio_frame = resume;
                audio.retain(|block, _| *block >= audio_frame / 480);
            }
            if options.system_audio || options.microphone {
                while audio_frame + 5280 <= end {
                    write_audio(&encoder, &mut audio, audio_frame)?;
                    audio_frame += 480;
                }
            }
        }
        let end = (shared
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
            .as_nanos()
            / 100) as i64;
        if let Some(last) = last {
            encoder
                .video(&last.pixels, video_time, (end - video_time).max(1))
                .map_err(|e| e.to_string())?;
            shared.stats.frames.fetch_add(1, Ordering::Relaxed);
        }
        if shared.stats.frames.load(Ordering::Relaxed) == 0 {
            return Err("No video frames received".into());
        }
        if options.system_audio || options.microphone {
            while audio_frame < (end.max(0) as u64 * 48000 / 10_000_000) {
                write_audio(&encoder, &mut audio, audio_frame)?;
                audio_frame += 480;
            }
        }
        Ok(())
    })();
    shared.stop.store(true, Ordering::Release);
    let capture_result = control.stop().map_err(|e| e.to_string());
    for worker in workers {
        let _ = worker.join();
    }
    let finish = encoder.finish().map_err(|e| e.to_string());
    if let Some(error) = shared
        .error
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        return Err(error);
    }
    result?;
    capture_result?;
    finish
}
fn record_gif(options: &Options, shared: Shared, path: &std::path::Path) -> Result<(), String> {
    use image::{
        Delay, Frame as GifFrame, RgbaImage,
        codecs::gif::{GifEncoder, Repeat},
    };

    let width = options.source.width;
    let height = options.source.height;
    if width < 16 || height < 16 || u64::from(width) * u64::from(height) * 4 > 64 * 1024 * 1024 {
        return Err("Invalid GIF recording source size".into());
    }
    let (send, receive) = mpsc::sync_channel(3);
    let flags = Flags {
        shared: shared.clone(),
        sender: send,
        crop: options.crop,
        width,
        height,
        fps: options.fps,
        exclusions: options.exclusions.clone(),
    };
    let item = if options.source.window {
        Target::Window(options.source.handle)
    } else {
        Target::Monitor(options.source.handle)
    };
    let control = Capture::start_free_threaded(Settings::new(
        item,
        if options.cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        },
        DrawBorderSettings::WithBorder,
        SecondaryWindowSettings::Exclude,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    ))
    .map_err(|e| e.to_string())?;
    let result: Result<(), String> = (|| {
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut encoder = GifEncoder::new(file);
        encoder
            .set_repeat(Repeat::Infinite)
            .map_err(|e| e.to_string())?;
        let max_size = 1280u32;
        let scale = (max_size as f64 / width.max(height) as f64).min(1.0);
        let output_width = ((width as f64 * scale).round() as u32).max(1);
        let output_height = ((height as f64 * scale).round() as u32).max(1);
        let frame_delay = Delay::from_numer_denom_ms(1000, options.fps);
        while !shared.stop.load(Ordering::Acquire) && !control.is_finished() {
            if shared
                .clock
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .elapsed()
                >= Duration::from_secs(30)
            {
                break;
            }
            let video = match receive.recv_timeout(Duration::from_millis(10)) {
                Ok(frame) => frame,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let mut rgba = video.pixels;
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let image = RgbaImage::from_raw(width, height, rgba)
                .ok_or_else(|| "Invalid GIF frame buffer".to_string())?;
            let image = if scale < 1.0 {
                image::imageops::resize(
                    &image,
                    output_width,
                    output_height,
                    image::imageops::FilterType::Triangle,
                )
            } else {
                image
            };
            encoder
                .encode_frame(GifFrame::from_parts(image, 0, 0, frame_delay))
                .map_err(|e| e.to_string())?;
            shared.stats.frames.fetch_add(1, Ordering::Relaxed);
        }
        if shared.stats.frames.load(Ordering::Relaxed) == 0 {
            return Err("No video frames received".into());
        }
        Ok(())
    })();
    shared.stop.store(true, Ordering::Release);
    let capture_result = control.stop().map_err(|e| e.to_string());
    if let Some(error) = shared
        .error
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        return Err(error);
    }
    result?;
    capture_result
}
fn write_audio(
    encoder: &encoder::Encoder,
    blocks: &mut BTreeMap<u64, Vec<i32>>,
    start: u64,
) -> Result<(), String> {
    let samples = blocks
        .remove(&(start / 480))
        .unwrap_or_else(|| vec![0; 960]);
    let bytes: Vec<u8> = samples
        .into_iter()
        .flat_map(|v| (v.clamp(-32768, 32767) as i16).to_le_bytes())
        .collect();
    encoder.audio(&bytes, start).map_err(|e| e.to_string())
}
