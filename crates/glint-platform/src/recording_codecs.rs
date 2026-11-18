//! Check the actual system encode path, using generated pixels only.
use glint_core::media_export::{VideoCodec, VideoContainer, VideoExport};
use std::{path::PathBuf, sync::OnceLock};
use windows::Win32::{Media::MediaFoundation::*, System::Com::*};

#[derive(Clone, Debug)]
pub struct ExportCodecs {
    pub h264: Result<(), String>,
    pub hevc: Result<(), String>,
    pub mov_h264: Result<(), String>,
    pub mov_hevc: Result<(), String>,
    pub mkv_h264: Result<(), String>,
    pub mkv_hevc: Result<(), String>,
    pub webm: Result<(), String>,
}
struct ProbeFile(PathBuf);
impl Drop for ProbeFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub fn export_codecs() -> ExportCodecs {
    static CACHE: OnceLock<ExportCodecs> = OnceLock::new();
    CACHE
        .get_or_init(|| ExportCodecs {
            h264: probe(VideoContainer::Mp4, VideoCodec::H264),
            hevc: probe(VideoContainer::Mp4, VideoCodec::Hevc),
            mov_h264: extended_probe(VideoContainer::Mov, VideoCodec::H264),
            mov_hevc: extended_probe(VideoContainer::Mov, VideoCodec::Hevc),
            mkv_h264: extended_probe(VideoContainer::Mkv, VideoCodec::H264),
            mkv_hevc: extended_probe(VideoContainer::Mkv, VideoCodec::Hevc),
            webm: extended_probe(VideoContainer::WebM, VideoCodec::Vp9),
        })
        .clone()
}
fn extended_probe(container: VideoContainer, codec: VideoCodec) -> Result<(), String> {
    super::containers::component_path()?;
    probe(container, codec)
}
fn probe(container: VideoContainer, codec: VideoCodec) -> Result<(), String> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(|e| e.to_string())?;
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _com = Com;
    unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|e| e.to_string())?;
    struct Media;
    impl Drop for Media {
        fn drop(&mut self) {
            unsafe {
                let _ = MFShutdown();
            }
        }
    }
    let _media = Media;
    let dir = std::env::temp_dir().join(format!(
        "glint-codec-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let _dir = Directory(dir.clone());
    let file = ProbeFile(dir.join(format!("probe.{}", container.extension())));
    let options = VideoExport {
        codec,
        container,
        ..Default::default()
    };
    let mut encoder =
        super::containers::ContainerEncoder::new(&file.0, 256, 144, 30, true, options)
            .map_err(|e| e.to_string())?;
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = cancel.clone();
    let (done, wait) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if wait
            .recv_timeout(std::time::Duration::from_secs(20))
            .is_err()
        {
            flag.store(true, std::sync::atomic::Ordering::Release);
        }
    });
    let frame = [30, 90, 180, 255].repeat(256 * 144);
    for i in 0..4 {
        encoder
            .video(&frame, i * 10_000_000 / 30, 10_000_000 / 30, &cancel)
            .map_err(|e| e.to_string())?;
    }
    encoder
        .audio(&vec![0; 6400 * 4], 0)
        .map_err(|e| e.to_string())?;
    let result = encoder.finish(&cancel);
    let _ = done.send(());
    let _ = watchdog.join();
    result?;
    if std::fs::metadata(&file.0).map_err(|e| e.to_string())?.len() == 0 {
        return Err("Encoder produced no file".into());
    }
    Ok(())
}
