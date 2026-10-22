//! Opt-in tracing collection with a bounded worker and file-rotate retention.
//! Only allowlisted event codes cross the tracing boundary; arbitrary fields are ignored.
use std::{
    fs::{self},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread::JoinHandle,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing_subscriber::{Layer, layer::SubscriberExt};

pub const FILE_LIMIT: u64 = 512 * 1024;
/// One active file plus this many rotated backups.
pub const BACKUP_COUNT: usize = 1;
pub const MAX_BACKUPS: usize = 20;
const QUEUE_LIMIT: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
impl Level {
    fn rank(self) -> u8 {
        match self {
            Self::Error => 1,
            Self::Warn => 2,
            Self::Info => 3,
            Self::Debug => 4,
            Self::Trace => 5,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub level: Level,
    pub file_size_kib: u16,
    pub backups: u8,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            level: Level::Info,
            file_size_kib: (FILE_LIMIT / 1024) as u16,
            backups: BACKUP_COUNT as u8,
        }
    }
}
#[derive(Clone, Copy)]
pub enum CaptureExitCause {
    Open,
    ApiCancel,
    Escape,
    SecondaryClick,
    ToolbarCancel,
    WindowRequest,
    Copy,
    Save,
    Pin,
    ColorCopy,
    PinUpdate,
    Direct,
}
#[derive(Clone, Copy)]
pub enum Event {
    Startup,
    SettingsShown,
    SettingsFocused,
    SettingsBlurred,
    HotkeyPressed,
    HotkeyFailed,
    CaptureRequested,
    ScrollStarted,
    ScrollQueued,
    ScrollAcquired,
    ScrollReady,
    ScrollCancelled,
    WindowFocused,
    WindowBlurred,
    PointerCaptured,
    PointerReleased,
    CursorRestricted,
    CursorUnrestricted,
    Panic,
    StartupFailed,
    Locked,
    Unlocked,
    Sleeping,
    Resumed,
    Disconnected,
    Connected,

    Enabled,
    Retry,
    SettingsSaved,
    SettingsSaveFailed,
    CaptureReady,
    CaptureFailed,
    CaptureCompleted,
    CaptureCancelled,
    CaptureExit(CaptureExitCause),
    CaptureClosedBusy,
    CaptureOutputFailed,
    DiagnosticsReady,
    DiagnosticsFailed,
    Shutdown,
}
impl Event {
    fn level(self) -> Level {
        match self {
            Self::Panic
            | Self::StartupFailed
            | Self::CaptureFailed
            | Self::CaptureOutputFailed
            | Self::SettingsSaveFailed
            | Self::DiagnosticsFailed
            | Self::HotkeyFailed => Level::Error,
            Self::CursorRestricted | Self::CaptureClosedBusy => Level::Warn,
            Self::ScrollQueued | Self::ScrollAcquired => Level::Debug,
            Self::PointerCaptured | Self::PointerReleased | Self::CursorUnrestricted => {
                Level::Trace
            }
            _ => Level::Info,
        }
    }
    fn code(self) -> &'static str {
        match self {
            Self::Startup => "app.startup",
            Self::SettingsShown => "settings.shown",
            Self::SettingsFocused => "settings.focused",
            Self::SettingsBlurred => "settings.blurred",
            Self::HotkeyPressed => "hotkey.pressed",
            Self::HotkeyFailed => "hotkey.failed",
            Self::CaptureRequested => "capture.requested",
            Self::ScrollStarted => "scroll.started",
            Self::ScrollQueued => "scroll.queued",
            Self::ScrollAcquired => "scroll.acquired",
            Self::ScrollReady => "scroll.ready",
            Self::ScrollCancelled => "scroll.cancelled",
            Self::WindowFocused => "window.focused",
            Self::WindowBlurred => "window.blurred",
            Self::PointerCaptured => "input.pointer_captured",
            Self::PointerReleased => "input.pointer_released",
            Self::CursorRestricted => "input.cursor_restricted",
            Self::CursorUnrestricted => "input.cursor_unrestricted",
            Self::Panic => "app.panic",
            Self::StartupFailed => "app.startup_failed",
            Self::Locked => "system.locked",
            Self::Unlocked => "system.unlocked",
            Self::Sleeping => "system.sleeping",
            Self::Resumed => "system.resumed",
            Self::Disconnected => "system.disconnected",
            Self::Connected => "system.connected",
            Self::Enabled => "log.enabled",
            Self::Retry => "log.retry",
            Self::SettingsSaved => "settings.saved",
            Self::SettingsSaveFailed => "settings.save_failed",
            Self::CaptureReady => "capture.ready",
            Self::CaptureFailed => "capture.failed",
            Self::CaptureCompleted => "capture.completed",
            Self::CaptureCancelled => "capture.cancelled",
            Self::CaptureClosedBusy => "capture.closed.busy",
            Self::CaptureOutputFailed => "capture.output.failed",
            Self::CaptureExit(cause) => match cause {
                CaptureExitCause::Open => "capture.exit.open",
                CaptureExitCause::ApiCancel => "capture.exit.api_cancel",
                CaptureExitCause::Escape => "capture.exit.escape",
                CaptureExitCause::SecondaryClick => "capture.exit.secondary_click",
                CaptureExitCause::ToolbarCancel => "capture.exit.toolbar_cancel",
                CaptureExitCause::WindowRequest => "capture.exit.window_request",
                CaptureExitCause::Copy => "capture.exit.copy",
                CaptureExitCause::Save => "capture.exit.save",
                CaptureExitCause::Pin => "capture.exit.pin",
                CaptureExitCause::ColorCopy => "capture.exit.color_copy",
                CaptureExitCause::PinUpdate => "capture.exit.pin_update",
                CaptureExitCause::Direct => "capture.exit.direct",
            },
            Self::DiagnosticsReady => "diagnostics.ready",
            Self::DiagnosticsFailed => "diagnostics.failed",
            Self::Shutdown => "app.shutdown",
        }
    }
    fn from_code(code: &str) -> Option<Self> {
        match code {
            "app.startup" => Some(Self::Startup),
            "settings.shown" => Some(Self::SettingsShown),
            "settings.focused" => Some(Self::SettingsFocused),
            "settings.blurred" => Some(Self::SettingsBlurred),
            "hotkey.pressed" => Some(Self::HotkeyPressed),
            "hotkey.failed" => Some(Self::HotkeyFailed),
            "capture.requested" => Some(Self::CaptureRequested),
            "scroll.started" => Some(Self::ScrollStarted),
            "scroll.queued" => Some(Self::ScrollQueued),
            "scroll.acquired" => Some(Self::ScrollAcquired),
            "scroll.ready" => Some(Self::ScrollReady),
            "scroll.cancelled" => Some(Self::ScrollCancelled),
            "window.focused" => Some(Self::WindowFocused),
            "window.blurred" => Some(Self::WindowBlurred),
            "input.pointer_captured" => Some(Self::PointerCaptured),
            "input.pointer_released" => Some(Self::PointerReleased),
            "input.cursor_restricted" => Some(Self::CursorRestricted),
            "input.cursor_unrestricted" => Some(Self::CursorUnrestricted),
            "app.panic" => Some(Self::Panic),
            "app.startup_failed" => Some(Self::StartupFailed),
            "system.locked" => Some(Self::Locked),
            "system.unlocked" => Some(Self::Unlocked),
            "system.sleeping" => Some(Self::Sleeping),
            "system.resumed" => Some(Self::Resumed),
            "system.disconnected" => Some(Self::Disconnected),
            "system.connected" => Some(Self::Connected),
            "log.enabled" => Some(Self::Enabled),
            "log.retry" => Some(Self::Retry),
            "settings.saved" => Some(Self::SettingsSaved),
            "settings.save_failed" => Some(Self::SettingsSaveFailed),
            "capture.ready" => Some(Self::CaptureReady),
            "capture.failed" => Some(Self::CaptureFailed),
            "capture.completed" => Some(Self::CaptureCompleted),
            "capture.cancelled" => Some(Self::CaptureCancelled),
            "capture.closed.busy" => Some(Self::CaptureClosedBusy),
            "capture.output.failed" => Some(Self::CaptureOutputFailed),
            "diagnostics.ready" => Some(Self::DiagnosticsReady),
            "diagnostics.failed" => Some(Self::DiagnosticsFailed),
            "app.shutdown" => Some(Self::Shutdown),
            "capture.exit.open" => Some(Self::CaptureExit(CaptureExitCause::Open)),
            "capture.exit.api_cancel" => Some(Self::CaptureExit(CaptureExitCause::ApiCancel)),
            "capture.exit.escape" => Some(Self::CaptureExit(CaptureExitCause::Escape)),
            "capture.exit.secondary_click" => {
                Some(Self::CaptureExit(CaptureExitCause::SecondaryClick))
            }
            "capture.exit.toolbar_cancel" => {
                Some(Self::CaptureExit(CaptureExitCause::ToolbarCancel))
            }
            "capture.exit.window_request" => {
                Some(Self::CaptureExit(CaptureExitCause::WindowRequest))
            }
            "capture.exit.copy" => Some(Self::CaptureExit(CaptureExitCause::Copy)),
            "capture.exit.save" => Some(Self::CaptureExit(CaptureExitCause::Save)),
            "capture.exit.pin" => Some(Self::CaptureExit(CaptureExitCause::Pin)),
            "capture.exit.color_copy" => Some(Self::CaptureExit(CaptureExitCause::ColorCopy)),
            "capture.exit.pin_update" => Some(Self::CaptureExit(CaptureExitCause::PinUpdate)),
            "capture.exit.direct" => Some(Self::CaptureExit(CaptureExitCause::Direct)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Subscriber,
    Worker,
    QueueFull,
    Write,
    Clear,
}
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub failure: Option<Failure>,
    pub written: u64,
    pub cleared: u64,
    pub dropped: u64,
}
enum Command {
    Record(Event, u64, u128, std::thread::ThreadId),
    Clear,
    Maintain,
}
struct Worker {
    send: SyncSender<Command>,
    join: JoinHandle<()>,
}
pub struct DiagnosticLog {
    directory: PathBuf,
    enabled: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    status: Arc<Mutex<Status>>,
    worker: Option<Worker>,
    notify: Arc<Mutex<Box<dyn Fn() + Send>>>,
    collector: Collector,
    dispatch: tracing::Dispatch,
    forced: bool,
    options: Arc<Mutex<Options>>,
}
impl DiagnosticLog {
    /// Construction is inert: no thread, directory or file until enabled or explicitly cleared.
    pub fn new(directory: PathBuf, notify: impl Fn() + Send + 'static) -> Self {
        let enabled = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(AtomicU64::new(0));
        let status = Arc::new(Mutex::new(Status::default()));
        let options = Arc::new(Mutex::new(Options::default()));
        let collector = Collector {
            enabled: enabled.clone(),
            generation: generation.clone(),
            status: status.clone(),
            send: Arc::new(Mutex::new(None)),
            options: options.clone(),
        };
        let dispatch =
            tracing::Dispatch::new(tracing_subscriber::registry().with(collector.clone()));
        Self {
            directory,
            enabled,
            generation,
            status,
            worker: None,
            notify: Arc::new(Mutex::new(Box::new(notify))),
            collector,
            dispatch,
            forced: false,
            options,
        }
    }
    /// The application's fixed-code tracing stream. Dependency messages are excluded.
    pub fn install_tracing(&self) {
        if tracing::dispatcher::set_global_default(self.dispatch.clone()).is_err() {
            self.status
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .failure = Some(Failure::Subscriber);
            return;
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            emit(Event::Panic); // Never persist panic payloads, paths or annotation text.
            previous(info);
        }));
    }
    pub fn set_notifier(&mut self, notify: impl Fn() + Send + 'static) {
        *self.notify.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(notify);
    }
    /// Explicit developer launch override; never changes the saved preference.
    pub fn enable_for_run(&mut self) {
        self.forced = true;
        self.configure(true);
    }
    pub fn forced(&self) -> bool {
        self.forced
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn set_options(&mut self, options: Options) {
        let options = Options {
            level: options.level,
            file_size_kib: options.file_size_kib.clamp(64, 16384),
            backups: options.backups.min(MAX_BACKUPS as u8),
        };
        let changed = {
            let mut previous = self.options.lock().unwrap_or_else(|e| e.into_inner());
            let changed = *previous != options;
            *previous = options;
            changed
        };
        if changed && self.enabled() {
            self.send(Command::Maintain);
        }
    }
    pub fn configure(&mut self, enabled: bool) {
        let enabled = enabled || self.forced;
        if self.enabled.swap(enabled, Ordering::SeqCst) != enabled {
            self.generation.fetch_add(1, Ordering::SeqCst);
            if enabled {
                self.record(Event::Enabled);
            }
        }
    }
    pub fn record(&mut self, event: Event) {
        if self.enabled.load(Ordering::SeqCst) {
            self.ensure_worker();
            tracing::dispatcher::with_default(&self.dispatch, || emit(event));
        }
    }
    pub fn clear(&mut self) {
        self.send(Command::Clear);
    }
    fn ensure_worker(&mut self) {
        if self.worker.is_none() {
            let (send, receive) = sync_channel(QUEUE_LIMIT);
            let status = self.status.clone();
            let enabled = self.enabled.clone();
            let generation = self.generation.clone();
            let directory = self.directory.clone();
            let notify = self.notify.clone();
            let options = self.options.clone();
            match std::thread::Builder::new()
                .name("glint-log".into())
                .spawn(move || {
                    for command in receive {
                        let current_options = *options.lock().unwrap_or_else(|e| e.into_inner());
                        let sink = Sink {
                            directory: directory.clone(),
                            limit: u64::from(current_options.file_size_kib) * 1024,
                            backups: usize::from(current_options.backups),
                        };
                        let (result, kind) = match command {
                            Command::Record(event, ticket, timestamp, thread) => {
                                if !enabled.load(Ordering::SeqCst)
                                    || generation.load(Ordering::SeqCst) != ticket
                                    || event.level().rank() > current_options.level.rank()
                                {
                                    continue;
                                }
                                (sink.write_at(event, timestamp, thread), Some(false))
                            }
                            Command::Clear => (sink.clear(), Some(true)),
                            Command::Maintain => (sink.maintain(), None),
                        };
                        {
                            let mut current = status.lock().unwrap_or_else(|e| e.into_inner());
                            current.failure = result.err().map(|_| {
                                if kind == Some(true) {
                                    Failure::Clear
                                } else {
                                    Failure::Write
                                }
                            });
                            if current.failure.is_none()
                                && let Some(clear) = kind
                            {
                                if clear {
                                    current.cleared += 1;
                                } else {
                                    current.written += 1;
                                }
                            }
                        }
                        if let Ok(notify) = notify.lock() {
                            notify();
                        }
                    }
                }) {
                Ok(join) => {
                    *self
                        .collector
                        .send
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(send.clone());
                    self.worker = Some(Worker { send, join });
                }
                Err(_) => {
                    self.status
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .failure = Some(Failure::Worker);
                }
            }
        }
    }
    fn send(&mut self, command: Command) {
        self.ensure_worker();
        if let Some(worker) = &self.worker
            && worker.send.try_send(command).is_err()
        {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            status.failure = Some(Failure::QueueFull);
            status.dropped += 1;
        }
    }
    /// Called after leaving the UI event loop. Drain the bounded queue and release file resources.
    pub fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.collector
                .send
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            drop(worker.send);
            let _ = worker.join.join();
        }
    }
}
impl Drop for DiagnosticLog {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Call sites supply a typed event, never arbitrary strings or input contents.
pub fn emit(event: Event) {
    match event.level() {
        Level::Error => tracing::error!(target: "glint::diagnostic", code = event.code()),
        Level::Warn => tracing::warn!(target: "glint::diagnostic", code = event.code()),
        Level::Info => tracing::info!(target: "glint::diagnostic", code = event.code()),
        Level::Debug => tracing::debug!(target: "glint::diagnostic", code = event.code()),
        Level::Trace => tracing::trace!(target: "glint::diagnostic", code = event.code()),
    }
}

/// Read-only snapshot on focus changes, never a mouse hook or polling loop.
/// Persist only boolean states; cursor coordinates and window identities stay private.
#[cfg(all(windows, feature = "windows-bindings"))]
pub fn record_pointer_state() {
    use windows::Win32::{
        Foundation::RECT,
        UI::{Input::KeyboardAndMouse::GetCapture, WindowsAndMessaging::*},
    };
    emit(if unsafe { GetCapture() }.is_invalid() {
        Event::PointerReleased
    } else {
        Event::PointerCaptured
    });
    let mut clip = RECT::default();
    if unsafe { GetClipCursor(&mut clip) }.is_ok() {
        let restricted = unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            clip.left > x
                || clip.top > y
                || clip.right < x + GetSystemMetrics(SM_CXVIRTUALSCREEN)
                || clip.bottom < y + GetSystemMetrics(SM_CYVIRTUALSCREEN)
        };
        emit(if restricted {
            Event::CursorRestricted
        } else {
            Event::CursorUnrestricted
        });
    }
}

#[derive(Clone)]
struct Collector {
    enabled: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    status: Arc<Mutex<Status>>,
    send: Arc<Mutex<Option<SyncSender<Command>>>>,
    options: Arc<Mutex<Options>>,
}
#[derive(Default)]
struct CodeVisitor(Option<Event>);
impl tracing::field::Visit for CodeVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "code" {
            self.0 = Event::from_code(value);
        }
    }
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}
impl<S: tracing::Subscriber> Layer<S> for Collector {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().target() != "glint::diagnostic" || !self.enabled.load(Ordering::SeqCst)
        {
            return;
        }
        let mut visitor = CodeVisitor::default();
        event.record(&mut visitor);
        let Some(code) = visitor.0 else {
            return;
        };
        if code.level().rank()
            > self
                .options
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .level
                .rank()
        {
            return;
        }
        let ticket = self.generation.load(Ordering::SeqCst);
        let send = self.send.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(send) = send.as_ref()
            && send
                .try_send(Command::Record(
                    code,
                    ticket,
                    timestamp(),
                    std::thread::current().id(),
                ))
                .is_err()
        {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            status.failure = Some(Failure::QueueFull);
            status.dropped += 1;
        }
    }
}

struct Sink {
    directory: PathBuf,
    limit: u64,
    backups: usize,
}
fn regular(path: &Path, directory: bool) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(io::ErrorKind::InvalidInput.into());
                }
            }
            if metadata.file_type().is_symlink()
                || metadata.is_dir() != directory
                || (!directory && !metadata.is_file())
            {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
impl Sink {
    fn maintain(&self) -> io::Result<()> {
        if !regular(&self.directory, true)? {
            return Ok(());
        }
        for index in self.backups + 1..=MAX_BACKUPS {
            let path = self.directory.join(format!("events.log.{index}"));
            if regular(&path, false)? {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn write(&self, event: Event) -> io::Result<()> {
        self.write_at(event, timestamp(), std::thread::current().id())
    }
    fn write_at(
        &self,
        event: Event,
        seconds: u128,
        thread: std::thread::ThreadId,
    ) -> io::Result<()> {
        if !regular(&self.directory, true)? {
            fs::create_dir(&self.directory)?;
        }
        self.maintain()?;
        let current = self.directory.join("events.log");
        // Validate managed paths before letting the rotation crate touch them.
        regular(&current, false)?;
        for index in 1..=self.backups {
            regular(&self.directory.join(format!("events.log.{index}")), false)?;
        }
        let line = format!(
            "{seconds} v2 {} pid={} thread={thread:?} level={}\n",
            event.code(),
            std::process::id(),
            event.level().name()
        );
        if line.len() as u64 > self.limit {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let length = if current.exists() {
            fs::metadata(&current)?.len()
        } else {
            0
        };
        // Rotate before writing a whole record, keeping lines intact and both files bounded.
        let mut file = file_rotate::FileRotate::new(
            &current,
            file_rotate::suffix::AppendCount::new(self.backups),
            file_rotate::ContentLimit::None,
            file_rotate::compression::Compression::None,
            None,
        );
        let rotate = length.saturating_add(line.len() as u64) > self.limit;
        if rotate {
            file.rotate()?;
        }
        file.write_all(line.as_bytes())?;
        file.flush()?;
        // file-rotate 0.8 may silently skip writes when its internal open failed.
        // Verify publication and force the bounded record to disk before reporting success.
        let expected = if rotate { 0 } else { length } + line.len() as u64;
        if fs::metadata(&current)?.len() != expected {
            return Err(io::Error::other("Log record was not published"));
        }
        fs::OpenOptions::new()
            .append(true)
            .open(&current)?
            .sync_data()
    }

    fn clear(&self) -> io::Result<()> {
        if regular(&self.directory, true)? {
            for path in std::iter::once(self.directory.join("events.log")).chain(
                (1..=MAX_BACKUPS).map(|index| self.directory.join(format!("events.log.{index}"))),
            ) {
                if regular(&path, false)? {
                    fs::remove_file(path)?;
                }
            }
        }
        // Never recursively remove the directory or unrelated files.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::atomic::AtomicU64, time::Duration};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "glint-log-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let sink = Sink {
                directory: self.0.join("logs"),
                limit: FILE_LIMIT,
                backups: BACKUP_COUNT,
            };
            let _ = sink.clear();
            let _ = fs::remove_dir(sink.directory);
            let _ = fs::remove_dir(&self.0);
        }
    }
    #[test]
    fn opt_in_off_clear_and_reenable_never_accept_content() {
        let dir = Temp::new();
        let path = dir.0.join("logs");
        let (send, receive) = sync_channel(16);
        let mut log = DiagnosticLog::new(path.clone(), move || {
            let _ = send.send(());
        });
        log.record(Event::CaptureOutputFailed);
        assert!(!path.exists());
        assert!(log.worker.is_none());
        log.configure(true);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        log.record(Event::SettingsSaveFailed);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        let before = fs::read(path.join("events.log")).unwrap();
        log.configure(false);
        log.record(Event::CaptureClosedBusy);
        assert_eq!(fs::read(path.join("events.log")).unwrap(), before);
        fs::write(path.join("unrelated.txt"), b"keep").unwrap();
        log.clear();
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(!path.join("events.log").exists());
        assert_eq!(log.status().cleared, 1);
        assert_eq!(fs::read(path.join("unrelated.txt")).unwrap(), b"keep");
        fs::remove_file(path.join("unrelated.txt")).unwrap();
        log.configure(true);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        log.record(Event::CaptureCompleted);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        log.shutdown();
        let text = fs::read_to_string(path.join("events.log")).unwrap();
        for line in text.lines() {
            let fields: Vec<_> = line.split(' ').collect();
            assert_eq!(fields.len(), 6);
            assert!(fields[0].parse::<u64>().is_ok());
            assert_eq!(fields[1], "v2");
            assert!(["log.enabled", "capture.completed"].contains(&fields[2]));
        }
        assert!(log.status().failure.is_none());
    }
    #[test]
    fn rotation_is_bounded_and_failure_preserves_unrelated_targets() {
        let dir = Temp::new();
        let sink = Sink {
            directory: dir.0.join("logs"),
            limit: 128,
            backups: BACKUP_COUNT,
        };
        for _ in 0..40 {
            sink.write(Event::SettingsSaveFailed).unwrap();
        }
        let files: Vec<_> = fs::read_dir(&sink.directory)
            .unwrap()
            .map(|e| e.unwrap())
            .collect();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|e| e.metadata().unwrap().len() <= 128));
        sink.clear().unwrap();
        fs::create_dir(sink.directory.join("events.log")).unwrap();
        assert!(sink.write(Event::CaptureReady).is_err());
        assert!(sink.clear().is_err());
        assert!(sink.directory.join("events.log").is_dir());
        fs::remove_dir(sink.directory.join("events.log")).unwrap();
    }
    #[test]
    fn bounded_queue_never_blocks_producer_and_disable_discards_pending_records() {
        let dir = Temp::new();
        let path = dir.0.join("logs");
        let (entered, ready) = sync_channel(1);
        let (release, gate) = sync_channel(1);
        let first = AtomicBool::new(true);
        let mut log = DiagnosticLog::new(path.clone(), move || {
            if first.swap(false, Ordering::SeqCst) {
                entered.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        });
        log.configure(true);
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        for _ in 0..=QUEUE_LIMIT {
            log.record(Event::CaptureReady);
        }
        assert_eq!(log.status().failure, Some(Failure::QueueFull));
        assert_eq!(log.status().dropped, 1);
        log.configure(false);
        // Re-enabling before the worker resumes must not revive old queued events.
        log.configure(true);
        assert_eq!(log.status().dropped, 2);
        release.send(()).unwrap();
        log.shutdown();
        assert_eq!(log.status().written, 1);
        assert_eq!(
            fs::read_to_string(path.join("events.log"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
    #[test]
    fn worker_failure_is_observable_and_explicit_retry_recovers() {
        let dir = Temp::new();
        let path = dir.0.join("logs");
        fs::write(&path, b"not a directory").unwrap();
        let (send, receive) = sync_channel(16);
        let mut log = DiagnosticLog::new(path.clone(), move || {
            let _ = send.send(());
        });
        log.configure(true);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(log.status().failure, Some(Failure::Write));
        assert_eq!(fs::read(&path).unwrap(), b"not a directory");
        fs::remove_file(&path).unwrap();
        log.record(Event::DiagnosticsReady);
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(log.status().failure.is_none());
        assert_eq!(log.status().written, 1);
        log.shutdown();
    }
}
