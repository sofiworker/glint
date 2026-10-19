//! Opt-in, bounded, content-free diagnostic events. No caller-supplied text is accepted.
use std::{
    fs::{self, OpenOptions},
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

pub const FILE_LIMIT: u64 = 512 * 1024;
const QUEUE_LIMIT: usize = 64;
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
    fn code(self) -> &'static str {
        match self {
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
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
    Record(Event, u64),
    Clear,
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
}
impl DiagnosticLog {
    /// Construction is inert: no thread, directory or file until enabled or explicitly cleared.
    pub fn new(directory: PathBuf, notify: impl Fn() + Send + 'static) -> Self {
        Self {
            directory,
            enabled: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            status: Arc::new(Mutex::new(Status::default())),
            worker: None,
            notify: Arc::new(Mutex::new(Box::new(notify))),
        }
    }
    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn configure(&mut self, enabled: bool) {
        if self.enabled.swap(enabled, Ordering::SeqCst) != enabled {
            self.generation.fetch_add(1, Ordering::SeqCst);
            if enabled {
                self.record(Event::Enabled);
            }
        }
    }
    pub fn record(&mut self, event: Event) {
        if self.enabled.load(Ordering::SeqCst) {
            self.send(Command::Record(
                event,
                self.generation.load(Ordering::SeqCst),
            ));
        }
    }
    pub fn clear(&mut self) {
        self.send(Command::Clear);
    }
    fn send(&mut self, command: Command) {
        if self.worker.is_none() {
            let (send, receive) = sync_channel(QUEUE_LIMIT);
            let status = self.status.clone();
            let enabled = self.enabled.clone();
            let generation = self.generation.clone();
            let directory = self.directory.clone();
            let notify = self.notify.clone();
            match std::thread::Builder::new()
                .name("glint-log".into())
                .spawn(move || {
                    let sink = Sink {
                        directory,
                        limit: FILE_LIMIT,
                    };
                    for command in receive {
                        let (result, clear) = match command {
                            Command::Record(event, ticket) => {
                                if !enabled.load(Ordering::SeqCst)
                                    || generation.load(Ordering::SeqCst) != ticket
                                {
                                    continue;
                                }
                                (sink.write(event), false)
                            }
                            Command::Clear => (sink.clear(), true),
                        };
                        {
                            let mut current = status.lock().unwrap_or_else(|e| e.into_inner());
                            current.failure = result.err().map(|_| {
                                if clear {
                                    Failure::Clear
                                } else {
                                    Failure::Write
                                }
                            });
                            if current.failure.is_none() {
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
                Ok(join) => self.worker = Some(Worker { send, join }),
                Err(_) => {
                    self.status
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .failure = Some(Failure::Worker);
                    return;
                }
            }
        }
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

struct Sink {
    directory: PathBuf,
    limit: u64,
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
    fn write(&self, event: Event) -> io::Result<()> {
        if !regular(&self.directory, true)? {
            fs::create_dir(&self.directory)?;
        }
        let current = self.directory.join("events.log");
        let previous = self.directory.join("events.previous.log");
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let line = format!("{seconds} v1 {}\n", event.code());
        let exists = regular(&current, false)?;
        if exists && fs::metadata(&current)?.len() > self.limit {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if exists
            && fs::metadata(&current)?
                .len()
                .saturating_add(line.len() as u64)
                > self.limit
        {
            if regular(&previous, false)? {
                fs::remove_file(&previous)?;
            }
            fs::rename(&current, &previous)?;
        }
        let mut file = if current.try_exists()? {
            // Recheck after rotation. Refuse non-regular/reparse targets.
            regular(&current, false)?;
            OpenOptions::new().append(true).open(&current)?
        } else {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&current)?
        };
        file.write_all(line.as_bytes())?;
        file.sync_data()
    }
    fn clear(&self) -> io::Result<()> {
        if !regular(&self.directory, true)? {
            return Ok(());
        }
        for name in ["events.log", "events.previous.log"] {
            let path = self.directory.join(name);
            if regular(&path, false)? {
                fs::remove_file(path)?;
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
                directory: self.0.join("diagnostics"),
                limit: FILE_LIMIT,
            };
            let _ = sink.clear();
            let _ = fs::remove_dir(sink.directory);
            let _ = fs::remove_dir(&self.0);
        }
    }
    #[test]
    fn opt_in_off_clear_and_reenable_never_accept_content() {
        let dir = Temp::new();
        let path = dir.0.join("diagnostics");
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
            assert_eq!(fields.len(), 3);
            assert!(fields[0].parse::<u64>().is_ok());
            assert_eq!(fields[1], "v1");
            assert!(["log.enabled", "capture.completed"].contains(&fields[2]));
        }
        assert!(log.status().failure.is_none());
    }
    #[test]
    fn rotation_is_bounded_and_failure_preserves_unrelated_targets() {
        let dir = Temp::new();
        let sink = Sink {
            directory: dir.0.join("diagnostics"),
            limit: 128,
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
        let path = dir.0.join("diagnostics");
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
        let path = dir.0.join("diagnostics");
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
