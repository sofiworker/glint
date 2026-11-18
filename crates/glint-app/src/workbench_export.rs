//! Export panel adapter and asynchronous system codec checks.
use super::*;
use glint_core::media_export::{VideoCodec, VideoContainer, VideoExport, VideoQuality};

pub(super) fn show(shared: &Rc<RefCell<Session>>, leave_after: bool) -> Result<(), String> {
    let mut s = shared.borrow_mut();
    if s.ui.get_busy() || s.text_in_progress() {
        return Ok(());
    }
    s.settle_preview()?;
    s.stop();
    s.output_flags.1 = leave_after;
    s.ui.set_has_audio(s.doc.video.is_some_and(|v| v.audio) && !s.model.state.muted);
    s.ui.set_modal(4);
    if s.doc.video.is_some() && s.ui.get_export_codec_note().is_empty() {
        s.ui.set_export_codec_note(
            tr(
                s.presentation.english,
                "正在检测本机编码器…",
                "Checking system encoders…",
            )
            .into(),
        );
        let weak = s.ui.as_weak();
        let en = s.presentation.english;
        std::thread::Builder::new()
            .name("glint-editor-codecs".into())
            .spawn(move || {
                let codecs = glint_platform::recording::export_codecs();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    for (container, values) in [
                        (0, [(0, &codecs.h264), (1, &codecs.hevc)]),
                        (2, [(0, &codecs.mov_h264), (1, &codecs.mov_hevc)]),
                        (3, [(0, &codecs.mkv_h264), (1, &codecs.mkv_hevc)]),
                    ] {
                        let names = values.iter().filter(|(_, result)| result.is_ok())
                            .map(|(id, _)| if *id == 0 { "H.264 / AVC".into() } else { "H.265 / HEVC".into() })
                            .collect::<Vec<_>>();
                        let ids = values.iter().filter(|(_, result)| result.is_ok())
                            .map(|(id, _)| *id).collect::<Vec<_>>();
                        match container {
                            0 => { ui.set_export_codecs(model(names)); ui.set_export_codec_ids(model(ids)); },
                            2 => { ui.set_mov_codecs(model(names)); ui.set_mov_codec_ids(model(ids)); },
                            _ => { ui.set_mkv_codecs(model(names)); ui.set_mkv_codec_ids(model(ids)); },
                        }
                    }
                    ui.set_webm_available(codecs.webm.is_ok());
                    ui.set_encoders_checked(true);
                    ui.set_export_codec(0);
                    ui.set_export_codec_note(tr(en, "已通过本机短片编码检测。", "Passed a local sample encode.").into());
                    let missing = tr(en,
                        "此格式或编码器在本机不可用。MOV / MKV / WebM 需安装视频格式组件（FFmpeg），方法见 README；安装后重启应用。",
                        "This format or encoder is unavailable. MOV / MKV / WebM need the FFmpeg media component; see README and restart after installation.");
                    ui.set_export_unavailable(missing.into());
                });
            })
            .map_err(|e| {
                s.ui.set_export_codec_note(e.to_string().into());
                e.to_string()
            })?;
    }
    Ok(())
}
pub(super) struct Selection {
    pub extension: &'static str,
    pub image_format: i32,
    pub jpeg_quality: u8,
    pub video: VideoExport,
    pub state: State,
}
pub(super) fn selection(s: &Session, copy: bool) -> Result<Selection, String> {
    let ui = &s.ui;
    let mut state = s.model.state.clone();
    let video = s.doc.video.is_some();
    let gif = video && ui.get_export_format() == 1;
    let container = match ui.get_export_format() {
        0 | 1 => VideoContainer::Mp4,
        2 => VideoContainer::Mov,
        3 => VideoContainer::Mkv,
        4 => VideoContainer::WebM,
        _ if !video => VideoContainer::Mp4,
        _ => return Err("Invalid video container".into()),
    };
    let codec = if video && !gif {
        if container == VideoContainer::WebM {
            if !ui.get_webm_available() {
                return Err("WebM encoder unavailable".into());
            }
            VideoCodec::Vp9
        } else {
            let ids = match container {
                VideoContainer::Mov => ui.get_mov_codec_ids(),
                VideoContainer::Mkv => ui.get_mkv_codec_ids(),
                _ => ui.get_export_codec_ids(),
            };
            match ids.row_data(ui.get_export_codec() as usize) {
                Some(0) => VideoCodec::H264,
                Some(1) => VideoCodec::Hevc,
                _ => return Err("Select an available video encoder".into()),
            }
        }
    } else {
        VideoCodec::H264
    };
    if !copy {
        let percent = *[100u32, 75, 50, 25]
            .get(ui.get_export_size() as usize)
            .ok_or("Invalid export size")?;
        let (w, h) = state.transform.dimensions();
        state.transform.size = Some(((w * percent / 100).max(1), (h * percent / 100).max(1)));
    }
    let options = VideoExport {
        container: if video {
            container
        } else {
            VideoContainer::Mp4
        },
        codec,
        fps: *(if gif {
            &[None, Some(30), Some(24), Some(15), Some(10)][..]
        } else {
            &[None, Some(60), Some(30), Some(24), Some(15), Some(10)][..]
        })
        .get(ui.get_export_fps() as usize)
        .ok_or("Invalid frame rate")?,
        quality: match ui.get_export_quality() {
            0 => VideoQuality::High,
            1 => VideoQuality::Balanced,
            2 => VideoQuality::Small,
            _ => return Err("Invalid export quality".into()),
        },
        audio_bitrate: if gif || !ui.get_has_audio() {
            None
        } else {
            *[Some(192_000), Some(128_000), Some(256_000), None]
                .get(ui.get_export_audio() as usize)
                .ok_or("Invalid audio codec")?
        },
    };
    options.validate().map_err(str::to_owned)?;
    Ok(Selection {
        extension: if video {
            if gif { "gif" } else { container.extension() }
        } else {
            crate::workbench_formats::extension(ui.get_export_format())?
        },
        image_format: ui.get_export_format(),
        jpeg_quality: ui.get_export_jpeg_quality().clamp(1, 100) as u8,
        video: options,
        state,
    })
}
