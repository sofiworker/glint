//! Optional local FFmpeg muxing; WebM receives edited pixels directly, without H.264 staging.
use glint_core::media_export::{VideoCodec, VideoContainer, VideoExport};
use std::{
    ffi::OsString,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

pub fn component_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("GLINT_FFMPEG") {
        let path = PathBuf::from(path);
        if path.is_absolute() && path.is_file() {
            return std::fs::canonicalize(path).map_err(|e| e.to_string());
        }
        return Err("GLINT_FFMPEG must point to an existing absolute ffmpeg.exe path".into());
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for dir in [
            Some(dir),
            dir.ancestors().find(|p| {
                p.file_name()
                    .is_some_and(|n| n == "debug" || n == "release")
            }),
        ]
        .into_iter()
        .flatten()
        {
            let path = dir.join("media/ffmpeg.exe");
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        if directory.is_absolute() {
            let path = directory.join("ffmpeg.exe");
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err("MOV / MKV / WebM require the optional FFmpeg media component (see README)".into())
}

/// One child, at most one queued frame, bounded stderr, and cancellation during pipe writes.
struct Process {
    child: Child,
    input: Option<mpsc::SyncSender<Vec<u8>>>,
    written: mpsc::Receiver<Result<(), String>>,
    writer: Option<JoinHandle<()>>,
    errors: Option<JoinHandle<Vec<u8>>>,
}
impl Process {
    fn start(args: &[OsString], pipe: bool) -> Result<Self, String> {
        let mut child = Command::new(component_path()?)
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-n"])
            .args(args)
            .stdin(if pipe { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or("Media component stderr unavailable")?;
        let errors = std::thread::spawn(move || {
            let mut tail = Vec::new();
            let mut buf = [0; 2048];
            while let Ok(n) = stderr.read(&mut buf) {
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(&buf[..n]);
                if tail.len() > 8192 {
                    tail.drain(..tail.len() - 8192);
                }
            }
            tail
        });
        let (send, receive) = mpsc::sync_channel::<Vec<u8>>(1);
        let (done, written) = mpsc::sync_channel(1);
        let writer = child.stdin.take().map(|mut stdin| {
            std::thread::spawn(move || {
                while let Ok(bytes) = receive.recv() {
                    let result = stdin.write_all(&bytes).map_err(|e| e.to_string());
                    let failed = result.is_err();
                    if done.send(result).is_err() || failed {
                        break;
                    }
                }
            })
        });
        Ok(Self {
            child,
            input: if pipe { Some(send) } else { None },
            written,
            writer,
            errors: Some(errors),
        })
    }
    fn write(&mut self, bytes: &[u8], cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Acquire) {
            return Err("Export cancelled".into());
        }
        self.input
            .as_ref()
            .ok_or("Media input closed")?
            .send(bytes.to_vec())
            .map_err(|_| "Media input closed")?;
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Export cancelled".into());
            }
            match self.written.recv_timeout(Duration::from_millis(25)) {
                Ok(r) => return r,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Media input closed".into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
    fn finish(mut self, cancel: &AtomicBool) -> Result<(), String> {
        self.input.take();
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Export cancelled".into());
            }
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                let text = self
                    .errors
                    .take()
                    .and_then(|e| e.join().ok())
                    .unwrap_or_default();
                // Some FFmpeg builds return zero for `-n` refusing an existing file.
                // At error log level, diagnostics must also be clear before publication.
                return if status.success() && text.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "Media component failed: {}",
                        String::from_utf8_lossy(&text)
                    ))
                };
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
        if let Some(e) = self.errors.take() {
            let _ = e.join();
        }
    }
}
pub(super) struct Workspace(PathBuf);
impl Workspace {
    pub fn new(parent: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..64 {
            let path = parent.join(format!(
                ".glint-media-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Cannot reserve media workspace".into())
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        for name in [
            "video.mp4",
            "video.webm",
            "audio.raw",
            "probe.mov",
            "probe.mkv",
            "probe.webm",
        ] {
            let _ = std::fs::remove_file(self.0.join(name));
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}
enum Video {
    Native(super::encoder::Encoder),
    WebM(Process),
}
pub(super) struct ContainerEncoder {
    video: Option<Video>,
    audio: Option<File>,
    workspace: Option<Workspace>,
    destination: PathBuf,
    options: VideoExport,
    frame_bytes: usize,
    fps: u32,
    frames_sent: u64,
    end: i64,
}
impl ContainerEncoder {
    pub fn new(
        destination: &Path,
        w: u32,
        h: u32,
        fps: u32,
        audio: bool,
        options: VideoExport,
    ) -> Result<Self, String> {
        options.validate().map_err(str::to_owned)?;
        if destination.try_exists().map_err(|e| e.to_string())? {
            return Err("Video destination exists; choose a new filename".into());
        }
        let extra = options.container != VideoContainer::Mp4;
        let workspace = if extra {
            Some(Workspace::new(
                destination.parent().ok_or("Missing media directory")?,
            )?)
        } else {
            None
        };
        let mut pcm = None;
        let video = if options.container == VideoContainer::WebM {
            let dir = workspace.as_ref().unwrap().path();
            if audio {
                pcm = Some(File::create_new(dir.join("audio.raw")).map_err(|e| e.to_string())?);
            }
            let args: Vec<OsString> = [
                "-f",
                "rawvideo",
                "-pixel_format",
                "bgra",
                "-video_size",
                &format!("{w}x{h}"),
                "-framerate",
                &fps.to_string(),
                "-i",
                "pipe:0",
                "-an",
                "-c:v",
                "libvpx-vp9",
                "-b:v",
                &options.video_bitrate(w, h, fps).to_string(),
                "-deadline",
                "good",
                "-cpu-used",
                "4",
                "-row-mt",
                "1",
                "-threads",
                "2",
                "-pix_fmt",
                "yuv420p",
                "-f",
                "webm",
            ]
            .into_iter()
            .map(Into::into)
            .chain([dir.join("video.webm").into_os_string()])
            .collect();
            Video::WebM(Process::start(&args, true)?)
        } else {
            let path = workspace
                .as_ref()
                .map(|d| d.path().join("video.mp4"))
                .unwrap_or_else(|| destination.to_owned());
            Video::Native(
                super::encoder::Encoder::new_selected(&path, w, h, fps, audio, true, Some(options))
                    .map_err(|e| e.to_string())?,
            )
        };
        Ok(Self {
            video: Some(video),
            audio: pcm,
            workspace,
            destination: destination.to_owned(),
            options,
            frame_bytes: w as usize * h as usize * 4,
            fps,
            frames_sent: 0,
            end: 0,
        })
    }
    pub fn video(
        &mut self,
        bytes: &[u8],
        time: i64,
        duration: i64,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        if bytes.len() != self.frame_bytes {
            return Err("Invalid edited frame size".into());
        }
        self.end = time + duration;
        match self.video.as_mut().ok_or("Media encoder closed")? {
            Video::Native(v) => v.video(bytes, time, duration).map_err(|e| e.to_string()),
            Video::WebM(v) => {
                // Raw video has a uniform clock. Keep one clock across clip boundaries,
                // so a partial final frame in every clip cannot accumulate A/V drift.
                while i128::from(self.frames_sent) * 10_000_000
                    < i128::from(self.end) * i128::from(self.fps)
                {
                    v.write(bytes, cancel)?;
                    self.frames_sent += 1;
                }
                Ok(())
            }
        }
    }
    pub fn audio(&mut self, bytes: &[u8], frame: u64) -> Result<(), String> {
        if let Some(Video::Native(v)) = &self.video {
            return v.audio(bytes, frame).map_err(|e| e.to_string());
        }
        if frame > 24 * 60 * 60 * 48000
            || bytes.len() > 4 * 1024 * 1024
            || !bytes.len().is_multiple_of(4)
        {
            return Err("Invalid audio packet".into());
        }
        if let Some(f) = &mut self.audio {
            f.seek(SeekFrom::Start(frame * 4))
                .and_then(|_| f.write_all(bytes))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn finish(mut self, cancel: &AtomicBool) -> Result<(), String> {
        match self.video.take().ok_or("Media encoder closed")? {
            Video::Native(v) => v.finish().map_err(|e| e.to_string())?,
            Video::WebM(v) => v.finish(cancel)?,
        }
        let Some(workspace) = &self.workspace else {
            return Ok(());
        };
        let dir = workspace.path();
        let webm = self.options.container == VideoContainer::WebM;
        let input = dir.join(if webm { "video.webm" } else { "video.mp4" });
        let mut args: Vec<OsString> = vec!["-i".into(), input.into_os_string()];
        let with_audio = self.audio.is_some();
        if let Some(file) = self.audio.take() {
            file.set_len((self.end.max(0) as u64 * 48000 / 10_000_000) * 4)
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            drop(file);
            args.extend(
                ["-f", "s16le", "-ar", "48000", "-ac", "2", "-i"]
                    .into_iter()
                    .map(OsString::from),
            );
            args.push(dir.join("audio.raw").into_os_string());
        }
        args.extend(
            [
                "-map",
                "0:v:0",
                "-map",
                if with_audio { "1:a:0" } else { "0:a?" },
                "-c:v",
                "copy",
                "-c:a",
                if webm { "libopus" } else { "copy" },
            ]
            .into_iter()
            .map(OsString::from),
        );
        if webm && with_audio {
            args.extend([
                "-b:a".into(),
                self.options
                    .audio_bitrate
                    .unwrap_or(192_000)
                    .to_string()
                    .into(),
            ]);
        }
        args.extend(
            ["-map_metadata", "-1", "-map_chapters", "-1", "-t"]
                .into_iter()
                .map(OsString::from),
        );
        args.push(format!("{:.7}", self.end as f64 / 10_000_000.).into());
        if self.options.container == VideoContainer::Mov {
            args.extend(["-movflags", "+faststart"].into_iter().map(OsString::from));
            if self.options.codec == VideoCodec::Hevc {
                args.extend(["-tag:v", "hvc1"].into_iter().map(OsString::from));
            }
        }
        args.extend([
            "-f".into(),
            match self.options.container {
                VideoContainer::Mov => "mov",
                VideoContainer::Mkv => "matroska",
                VideoContainer::WebM => "webm",
                VideoContainer::Mp4 => unreachable!(),
            }
            .into(),
            self.destination.clone().into_os_string(),
        ]);
        Process::start(&args, false)?.finish(cancel)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.destination)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
