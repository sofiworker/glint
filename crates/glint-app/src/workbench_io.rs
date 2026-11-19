//! Local media loading, platform pickers, glyph and output workers.
use super::*;
static OPENING: AtomicBool = AtomicBool::new(false);
struct OpenSlot;
impl Drop for OpenSlot {
    fn drop(&mut self) {
        OPENING.store(false, Ordering::Release);
    }
}

pub(super) fn text_commit(shared: &Rc<RefCell<Session>>) -> Result<(), String> {
    let s = shared.borrow_mut();
    if !s.ui.get_text_editing() || s.ui.get_busy() {
        return Ok(());
    }
    let value = s.ui.get_text_value().trim().to_owned();
    if value.is_empty() {
        s.ui.set_text_editing(false);
        return Ok(());
    }
    if value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(tr(
            s.presentation.english,
            "文字最多 80 个字符，且不能包含换行。",
            "Use up to 80 characters on a single line.",
        )
        .into());
    }
    let origin = (s.text_origin.x as u32, s.text_origin.y as u32);
    let size = s.ui.get_font_size() as u16;
    let c = s.ui.get_mark_color();
    let color = [c.red(), c.green(), c.blue(), 255];
    let font = crate::ui_font::family().to_owned();
    s.ui.set_busy(true);
    let weak = s.ui.as_weak();
    let pending = s.text_result.clone();
    std::thread::Builder::new()
        .name("glint-editor-text".into())
        .spawn(move || {
            let result =
                glint_platform::rasterize_text(&value, &font, size).map(|mask| TextAnnotation {
                    origin,
                    text: value,
                    font,
                    size,
                    color,
                    mask: glint_core::GlyphMask {
                        width: mask.width,
                        height: mask.height,
                        coverage: mask.coverage.into(),
                    },
                });
            *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("text-ready".into()));
        })
        .map_err(|e| {
            s.ui.set_busy(false);
            e.to_string()
        })?;
    Ok(())
}

pub(super) fn leave(shared: &Rc<RefCell<Session>>) -> Result<(), String> {
    let pending = {
        let mut s = shared.borrow_mut();
        s.ui.set_modal(0);
        s.leave.take()
    };
    match pending {
        Some(Leave::Close) => {
            CURRENT.with_borrow_mut(|c| {
                c.take();
            });
        }
        Some(Leave::Open) => return choose_file(shared),
        Some(Leave::Replace(d, p)) => return create(d, p),
        Some(Leave::Quit(owner)) => {
            shutdown();
            let _ = owner.upgrade_in_event_loop(|ui| ui.invoke_action("quit".into()));
        }
        None => {}
    }
    Ok(())
}
fn window_handle(ui: &WorkbenchWindow) -> Result<*mut std::ffi::c_void, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    ui.window()
        .with_winit_window(|w| {
            let h = w.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(h) = h.as_raw() else {
                return Err("Win32 editor required".into());
            };
            Ok(h.hwnd.get() as *mut _)
        })
        .ok_or("Editor window unavailable")?
}
pub(super) fn choose_file(shared: &Rc<RefCell<Session>>) -> Result<(), String> {
    let ui = shared.borrow().ui.clone_strong();
    ui.set_busy(true);
    let shared = shared.clone();
    slint::Timer::single_shot(Duration::ZERO, move || {
        let result = window_handle(&ui)
            .and_then(|owner| unsafe { glint_platform::choose_editor_file(owner, false) });
        ui.set_busy(false);
        match result {
            Ok(Some(path)) => {
                let p = shared.borrow().presentation.clone();
                load_file(path, p, Some(Rc::downgrade(&shared)));
            }
            Ok(None) => {}
            Err(e) => ui.set_status(e.into()),
        }
    });
    Ok(())
}
pub fn open_file(path: PathBuf, presentation: Presentation) {
    load_file(path, presentation, None);
}
pub fn open_clipboard(presentation: Presentation) {
    load_document(presentation, None, || {
        let frame = glint_platform::read_clipboard_image()?;
        let image = ImageDocument::from_selection(
            &frame.rgba, frame.width, frame.height,
            glint_core::Rect { x: 0, y: 0, width: frame.width, height: frame.height },
            &glint_core::Edits::default(),
        ).map_err(str::to_owned)?;
        Ok(Document::capture(Arc::new(image)))
    });
}
fn load_file(path: PathBuf, presentation: Presentation, existing: Option<Weak<RefCell<Session>>>) {
    load_document(presentation, existing, move || decode(&path));
}
fn load_document(
    presentation: Presentation,
    existing: Option<Weak<RefCell<Session>>>,
    read: impl FnOnce() -> Result<Document, String> + Send + 'static,
) {
    if OPENING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        if let Some(s) = CURRENT.with_borrow(Clone::clone) {
            s.borrow().ui.set_status(
                tr(
                    presentation.english,
                    "正在打开文件，请稍后。",
                    "A file is already opening.",
                )
                .into(),
            );
        }
        return;
    }
    let slot = OpenSlot;
    let ui = existing
        .as_ref()
        .and_then(Weak::upgrade)
        .map(|s| s.borrow().ui.as_weak());
    if let Some(s) = existing.as_ref().and_then(Weak::upgrade) {
        s.borrow().ui.set_busy(true);
    }
    let guarded = ui.is_some();
    let worker = std::thread::Builder::new()
        .name("glint-editor-open".into())
        .spawn(move || {
            let _slot = slot;
            let result = read();
            let _ = slint::invoke_from_event_loop(move || {
                let ui = ui.and_then(|w| w.upgrade());
                if guarded && ui.is_none() {
                    return;
                }
                if let Some(ui) = ui {
                    ui.set_busy(false);
                }
                if !DESKTOP_ALLOWED.get() {
                    return;
                }
                match result {
                    Ok(doc) => {
                        let result = if guarded {
                            create(doc, presentation)
                        } else {
                            open(doc, presentation)
                        };
                        if let Err(e) = result
                            && let Some(s) = CURRENT.with_borrow(Clone::clone)
                        {
                            s.borrow().ui.set_status(e.into());
                        }
                    }
                    Err(e) => {
                        if let Some(s) = CURRENT.with_borrow(Clone::clone) {
                            s.borrow().ui.set_status(e.into());
                        } else {
                            glint_platform::show_startup_error(&e);
                        }
                    }
                }
            });
        });
    if let Err(e) = worker {
        if let Some(s) = existing.as_ref().and_then(Weak::upgrade) {
            let s = s.borrow();
            s.ui.set_busy(false);
            s.ui.set_status(e.to_string().into());
        } else {
            glint_platform::show_startup_error(&e.to_string());
        }
    }
}
pub(crate) fn decode(path: &Path) -> Result<Document, String> {
    let identity = file_identity(path)?;
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let (source, w, h, video) =
        if matches!(extension.as_str(), "mp4" | "wmv" | "mov" | "m4v" | "gif") {
            let mut reader = glint_platform::recording::VideoReader::open(path)?;
            let f = reader.frame_at(0.)?;
            (f.rgba.clone(), f.width, f.height, Some(reader.info))
        } else {
            let i = crate::workbench_formats::decode(path)?;
            let (w, h) = i.dimensions();
            (i.into_raw(), w, h, None)
        };
    let image = ImageDocument::from_selection(
        &source,
        w,
        h,
        Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        },
        &Edits::default(),
    )
    .map_err(str::to_owned)?;
    if file_identity(path)? != identity {
        return Err("The source file changed while opening it".into());
    }
    Ok(Document {
        image: Arc::new(image),
        name,
        path: Some(path.to_owned()),
        video,
        file_identity: Some(identity),
    })
}
fn file_identity(path: &Path) -> Result<(u64, Option<std::time::SystemTime>), String> {
    let m = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() {
        return Err("Select a regular media file".into());
    }
    Ok((m.len(), m.modified().ok()))
}

pub(super) fn output(shared: &Rc<RefCell<Session>>, kind: &str) -> Result<(), String> {
    let mut s = shared.borrow_mut();
    if s.ui.get_busy() {
        return Ok(());
    }
    if s.ui.get_text_editing() {
        return Err(tr(
            s.presentation.english,
            "先完成文字编辑。",
            "Finish editing the text first.",
        )
        .into());
    }
    if let Some(preview) = s.preview.take() {
        s.model.apply(preview).map_err(str::to_owned)?;
    }
    s.stop();
    s.crop = None;
    s.ui.set_category(if s.doc.video.is_some() { 5 } else { 1 });
    let leave_after = kind == "save-leave";
    let copy = kind == "copy";
    s.output_flags = (copy, leave_after);
    if copy && s.doc.video.is_some() {
        return Ok(());
    }
    let selection = super::export::selection(&s, copy)?;
    s.ui.set_modal(0);
    let slot = if s.doc.video.is_none() {
        Some(crate::probe::OutputSlot::acquire().ok_or("Another image output is still running")?)
    } else {
        None
    };
    s.cancel.store(false, Ordering::Release);
    s.ui.set_busy(true);
    s.ui.set_status(
        tr(
            s.presentation.english,
            "正在处理… Esc 取消导出",
            "Processing… Esc cancels export",
        )
        .into(),
    );
    let shared = shared.clone();
    slint::Timer::single_shot(Duration::ZERO, move || {
        let result = (|| -> Result<_, String> {
            let (ui, doc, presentation, cancel) = {
                let s = shared.borrow();
                (
                    s.ui.clone_strong(),
                    s.doc.clone(),
                    s.presentation.clone(),
                    s.cancel.clone(),
                )
            };
            let destination = if copy {
                None
            } else {
                let name = format!(
                    "{}-edited.{}",
                    Path::new(&doc.name)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    selection.extension
                );
                let suggested = doc
                    .path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(|p| p.join(&name))
                    .unwrap_or_else(|| PathBuf::from(&presentation.output.directory).join(name));
                let path = unsafe {
                    glint_platform::choose_editor_export(
                        window_handle(&ui)?,
                        &suggested.to_string_lossy(),
                        selection.extension,
                    )
                }?;
                if let Some(path) = path {
                    if doc.path.as_ref().is_some_and(|p| same_path(p, &path)) {
                        return Err(tr(
                            presentation.english,
                            "请选择其他文件名，保留原文件。",
                            "Choose a different name to preserve the original.",
                        )
                        .into());
                    }
                    if doc.video.is_some() {
                        Some(glint_platform::ImageDestination {
                            path,
                            replacement: None,
                        })
                    } else {
                        unsafe {
                            glint_platform::confirm_image_destination(
                                window_handle(&ui)?,
                                path,
                                glint_platform::ReplacementPrompt {
                                    title: tr(presentation.english, "保存副本", "Save a copy"),
                                    question: tr(
                                        presentation.english,
                                        "替换现有文件？",
                                        "Replace the existing file?",
                                    ),
                                    hint: tr(
                                        presentation.english,
                                        "取消会保留当前编辑。",
                                        "Cancel keeps the current edits.",
                                    ),
                                },
                            )
                        }?
                    }
                } else {
                    None
                }
            };
            if !copy && destination.is_none() {
                return Ok(None);
            }
            Ok(Some((
                doc.clone(),
                selection,
                destination,
                cancel.clone(),
                ui.as_weak(),
                presentation.english,
            )))
        })();
        let request = match result {
            Ok(Some(r)) => r,
            Ok(None) => {
                shared.borrow().ui.set_busy(false);
                shared.borrow().ui.set_status("".into());
                shared.borrow().ui.set_modal(4);
                return;
            }
            Err(e) => {
                let s = shared.borrow();
                s.ui.set_busy(false);
                s.ui.set_status(e.into());
                return;
            }
        };
        let (doc, selection, destination, cancel, weak, en) = request;
        let state = selection.state;
        let pending = shared.borrow().output_result.clone();
        let spawn = std::thread::Builder::new()
            .name("glint-editor-output".into())
            .spawn(move || {
                let _slot = slot;
                let result = (|| {
                    if doc.video.is_some() {
                        if let Some(identity) = doc.file_identity
                            && file_identity(doc.path.as_ref().ok_or("Video path unavailable")?)?
                                != identity
                        {
                            return Err(
                                "The source video changed. Reopen it before exporting".into()
                            );
                        }
                        let path = &destination.as_ref().unwrap().path;
                        if path.exists() {
                            return Err(
                                "Video destination already exists; choose a new filename".into()
                            );
                        }
                        let gif = path
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("gif"));
                        let temp = path.with_file_name(format!(
                            ".glint-editor-{}-{}.{}",
                            std::process::id(),
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos(),
                            selection.extension
                        ));
                        let result = glint_platform::recording::export_edit_with_options(
                            doc.path.as_ref().ok_or("Video path unavailable")?,
                            &temp,
                            &state,
                            gif,
                            selection.video,
                            &cancel,
                            |frame, time| {
                                crate::workbench_pixels::render(
                                    &frame.rgba,
                                    frame.width,
                                    frame.height,
                                    &state,
                                    Some(time),
                                )
                            },
                            |v| {
                                let _ = weak.upgrade_in_event_loop(move |ui| {
                                    ui.set_status(
                                        format!(
                                            "{} {}%",
                                            tr(en, "导出中", "Exporting"),
                                            (v * 100.) as u32
                                        )
                                        .into(),
                                    )
                                });
                            },
                        );
                        let result = result.and_then(|()| {
                            if cancel.load(Ordering::Acquire) {
                                return Err("Export cancelled".into());
                            }
                            if glint_platform::publish_new_image(&temp, path)? {
                                Ok(())
                            } else {
                                Err("Video destination exists; choose another filename".into())
                            }
                        });
                        let _ = std::fs::remove_file(&temp);
                        result?;
                    } else {
                        let (w, h) = doc.image.dimensions();
                        let image = crate::workbench_pixels::render(
                            doc.image.source(),
                            w,
                            h,
                            &state,
                            None,
                        )?;
                        if cancel.load(Ordering::Acquire) {
                            return Err("Output cancelled".into());
                        }
                        if copy {
                            glint_platform::copy_rgba(
                                image.width(),
                                image.height(),
                                image.into_raw(),
                            )?;
                        } else {
                            let d = destination.as_ref().unwrap();
                            crate::workbench_formats::save(
                                &d.path,
                                &image,
                                selection.image_format,
                                selection.jpeg_quality,
                                d.replacement.as_ref(),
                                &cancel,
                            )?;
                        }
                    }
                    Ok::<_, String>(())
                })();
                *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("output-ready".into()));
            });
        if let Err(e) = spawn {
            let s = shared.borrow();
            s.ui.set_busy(false);
            s.ui.set_status(e.to_string().into());
        }
    });
    Ok(())
}
fn same_path(a: &Path, b: &Path) -> bool {
    let a = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_owned());
    let b = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_owned());
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

pub(super) fn labels(en: bool) -> EditorLabels {
    macro_rules! l {
        ($zh:literal,$en:literal) => {
            tr(en, $zh, $en).into()
        };
    }
    EditorLabels {
        clean: l!("未修改", "Unmodified"),
        dirty: l!("有未保存的修改", "Unsaved changes"),
        copy: l!("复制图片 · Ctrl C", "Copy image · Ctrl C"),
        save: l!("保存副本", "Save a copy"),
        export: l!("导出视频", "Export video"),
        crop: l!("裁剪", "Crop"),
        shape: l!("标注", "Annotate"),
        text: l!("文字", "Text"),
        redact: l!("遮盖", "Cover"),
        adjust: l!("调整", "Adjust"),
        clip: l!("剪辑", "Clips"),
        undo: l!("撤销 · Ctrl Z", "Undo · Ctrl Z"),
        redo: l!("重做 · Ctrl Shift Z", "Redo · Ctrl Shift Z"),
        compare: l!("按住查看原图", "Hold to compare"),
        more: l!("更多操作", "More"),
        select: l!("选择", "Select"),
        rect: l!("矩形", "Rectangle"),
        arrow: l!("箭头", "Arrow"),
        pen: l!("画笔", "Pen"),
        highlight: l!("高亮", "Highlight"),
        solid: l!("实心遮盖", "Solid cover"),
        mosaic: l!("像素化", "Pixelate"),
        ratio: l!("比例", "Ratio"),
        free: l!("自由", "Free"),
        rotate: l!("旋转 90°", "Rotate 90°"),
        flip: l!("水平翻转", "Flip"),
        cancel: l!("取消", "Cancel"),
        apply: l!("应用", "Apply"),
        brightness: l!("亮度", "Brightness"),
        contrast: l!("对比度", "Contrast"),
        saturation: l!("饱和度", "Saturation"),
        reset: l!("还原", "Reset"),
        duplicate: l!("复制对象", "Duplicate"),
        delete: l!("删除", "Delete"),
        hint: l!("滚轮缩放 · 空格拖动", "Wheel to zoom · Space to pan"),
        fit: l!("适应", "Fit"),
        resize: l!("调整尺寸", "Resize"),
        objects: l!("标注对象", "Annotation objects"),
        open: l!("打开其他文件…", "Open another file…"),
        close: l!("关闭", "Close"),
        discard: l!("放弃修改", "Discard"),
        unsaved: l!("保存这次修改？", "Save your changes?"),
        unsaved_hint: l!(
            "保存为副本，原文件会保留。",
            "Save a copy to preserve the original file."
        ),
        width: l!("宽", "Width"),
        height: l!("高", "Height"),
        linked: l!("锁定宽高比", "Lock aspect ratio"),
        original: l!("原图", "Original"),
        split: l!("分割", "Split"),
        play: l!("播放", "Play"),
        pause: l!("暂停", "Pause"),
        audio: l!("声音", "Audio"),
        mute: l!("静音", "Mute"),
        from: l!("开始", "From"),
        to: l!("结束", "To"),
        empty: l!("暂无标注", "No annotations"),
        crop_hint: l!(
            "拖动边角裁剪 · Enter 应用 · Esc 取消",
            "Drag crop handles · Enter to apply · Esc to cancel"
        ),
        text_hint: l!("Enter 完成 · Esc 取消", "Enter to finish · Esc to cancel"),
        frame_back: l!("上一帧", "Previous frame"),
        frame_next: l!("下一帧", "Next frame"),
        zoom_in: l!("放大", "Zoom in"),
        zoom_out: l!("缩小", "Zoom out"),
        volume: l!("音量", "Volume"),
        format: l!("格式", "Format"),
        codec: l!("视频编码", "Video codec"),
        quality: l!("质量", "Quality"),
        fps: l!("帧率", "Frame rate"),
        size: l!("导出尺寸", "Output size"),
        audio_codec: l!("声音", "Audio"),
        no_audio: l!("无声音", "No audio"),
        source_fps: l!("跟随源文件", "Follow source"),
        export_note: l!(
            "帧率不高于源文件，尺寸会补齐至 16 像素倍数。",
            "Frame rate never exceeds the source. Dimensions are padded to multiples of 16."
        ),
        gif_note: l!(
            "GIF 无声音，最高 30 FPS，且不高于源文件。",
            "GIF is silent, up to 30 FPS without exceeding the source."
        ),
        webm_note: l!(
            "VP9 + Opus；帧率不高于源文件。",
            "VP9 + Opus. Frame rate never exceeds the source."
        ),
        image_note: l!(
            "WebP 为无损编码。重新生成像素，不保留源文件元数据。",
            "WebP is lossless. Encodes flattened pixels without source metadata."
        ),
        high: l!("高质量", "High"),
        balanced: l!("均衡", "Balanced"),
        small: l!("较小文件", "Smaller file"),
    }
}
