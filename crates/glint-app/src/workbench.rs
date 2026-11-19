//! Native v7 single-document workbench. UI-thread state, bounded/coalesced workers.
use glint_core::{
    Annotation, Edits, ImageDocument, Pixelation, Point, Rect, Redaction, Shape, ShapeKind, Stroke,
    TextAnnotation,
    workbench::{Editor, Mark, State, Transform},
};
use slint::{ComponentHandle, Model, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

glint_ui::slint! {
    export { Theme } from "@glint_ui/theme.slint";
    export { Motion } from "@glint_ui/widgets/motion.slint";
    export { WorkbenchWindow, CaptureResultWindow, EditorLabels, EditorObject, EditorClip, EditorTimedMark, EditorGrip } from "src/workbench.slint";
}
#[path = "workbench_export.rs"]
mod export;
#[path = "workbench_io.rs"]
mod io;
pub(crate) use io::decode;
use io::{choose_file, labels, leave, output, text_commit};
pub use io::{open_clipboard, open_file};
#[path = "workbench_smoke.rs"]
pub mod smoke;
#[derive(Clone)]
pub struct Presentation {
    pub english: bool,
    pub dark: bool,
    pub theme: glint_core::config::Theme,
    pub contrast: bool,
    pub accent: slint::Color,
    pub scale: glint_core::config::Scale,
    pub reduced: bool,
    pub output: glint_core::config::Output,
}
impl Presentation {
    pub fn from_resident(s: &crate::resident::AppState) -> Self {
        let saved =
            crate::probe::Presentation::from_config(s.session.preferences.saved(), &s.languages);
        Self::from_capture(&saved)
    }

    pub fn from_capture(saved: &crate::probe::Presentation) -> Self {
        Self {
            english: saved.english,
            dark: saved.dark,
            theme: saved.theme,
            contrast: saved.contrast,
            accent: saved.accent,
            scale: saved.interface_scale,
            reduced: saved.reduce_motion,
            output: saved.output.clone(),
        }
    }
}
#[derive(Clone)]
pub struct Document {
    pub image: Arc<ImageDocument>,
    pub name: String,
    pub path: Option<PathBuf>,
    pub video: Option<glint_platform::recording::VideoInfo>,
    pub file_identity: Option<(u64, Option<std::time::SystemTime>)>,
}
impl Document {
    pub fn capture(image: Arc<ImageDocument>) -> Self {
        Self {
            image,
            name: format!(
                "Glint-{}.png",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            ),
            path: None,
            video: None,
            file_identity: None,
        }
    }
}
enum Leave {
    Close,
    Open,
    Replace(Document, Presentation),
    Quit(slint::Weak<crate::settings::SettingsWindow>),
}
struct Job {
    state: State,
    time: f64,
    compare: bool,
    ticket: i32,
}
struct Worker {
    latest: Arc<Mutex<Option<Job>>>,
    wake: mpsc::SyncSender<()>,
    stopped: Arc<AtomicBool>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
    }
}
impl Worker {
    fn new(ui: &WorkbenchWindow, doc: Document) -> Result<Self, String> {
        let latest: Arc<Mutex<Option<Job>>> = Arc::new(Mutex::new(None));
        let slot = latest.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let (wake, receive) = mpsc::sync_channel(1);
        let weak = ui.as_weak();
        std::thread::Builder::new()
            .name("glint-editor-preview".into())
            .spawn(move || {
                let mut video = doc
                    .path
                    .as_ref()
                    .filter(|_| doc.video.is_some())
                    .map(|p| glint_platform::recording::VideoReader::open(p));
                while receive.recv().is_ok() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let Some(job) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() else {
                        continue;
                    };
                    let result = (|| {
                        let frame = if let Some(reader) = &mut video {
                            Some(reader.as_mut().map_err(|e| e.clone())?.frame_at(job.time)?)
                        } else {
                            None
                        };
                        let (bytes, w, h) = frame
                            .as_ref()
                            .map(|f| (f.rgba.as_slice(), f.width, f.height))
                            .unwrap_or_else(|| {
                                let (w, h) = doc.image.dimensions();
                                (doc.image.source(), w, h)
                            });
                        let state = if job.compare {
                            State {
                                transform: Transform::new(w, h),
                                marks: vec![],
                                ..job.state.clone()
                            }
                        } else {
                            job.state
                        };
                        crate::workbench_pixels::render(
                            bytes,
                            w,
                            h,
                            &state,
                            doc.video.map(|_| job.time),
                        )
                    })();
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        if ui.get_render_ticket() != job.ticket {
                            return;
                        }
                        match result {
                            Ok(image) => ui.set_picture(slint::Image::from_rgba8(
                                slint::SharedPixelBuffer::clone_from_slice(
                                    image.as_raw(),
                                    image.width(),
                                    image.height(),
                                ),
                            )),
                            Err(e) => {
                                ui.set_status(e.into());
                                ui.invoke_action("preview-failed".into());
                            }
                        }
                        ui.set_rendered_ticket(job.ticket);
                    });
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            latest,
            wake,
            stopped,
        })
    }
    fn request(&self, job: Job) {
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(job);
        let _ = self.wake.try_send(());
    }
}
enum Drag {
    Object {
        before: State,
        index: usize,
        start: Point,
        handle: Option<u8>,
    },
    Draw {
        before: State,
        start: Point,
        points: Vec<Point>,
    },
    Crop {
        before: Rect,
        start: Point,
        handle: u8,
    },
}
struct Session {
    ui: WorkbenchWindow,
    doc: Document,
    model: Editor,
    presentation: Presentation,
    worker: Worker,
    selected: Option<usize>,
    clip: usize,
    preview: Option<State>,
    crop: Option<State>,
    crop_rect: Rect,
    drag: Option<Drag>,
    text_origin: Point,
    text_index: Option<usize>,
    leave: Option<Leave>,
    timer: slint::Timer,
    clock: Option<(Instant, f64)>,
    cancel: Arc<AtomicBool>,
    text_result: Arc<Mutex<Option<Result<TextAnnotation, String>>>>,
    output_result: Arc<Mutex<Option<Result<(), String>>>>,
    output_flags: (bool, bool),
    audio: Option<glint_platform::recording::AudioPreview>,
    desktop_hidden: bool,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.timer.stop();
        let _ = self.ui.hide();
    }
}
thread_local! {
    static CURRENT:RefCell<Option<Rc<RefCell<Session>>>>=const{RefCell::new(None)};
    static DESKTOP_ALLOWED:std::cell::Cell<bool>=const{std::cell::Cell::new(true)};
}

pub fn open_handoff(doc: Document, presentation: Presentation) -> Result<(), String> {
    if let Some(current) = CURRENT.with_borrow(Clone::clone)
        && current.borrow().model.dirty()
    {
        return Err(tr(
            presentation.english,
            "请先保存或关闭编辑器中的当前文件。",
            "Save or close the current file in the editor first.",
        )
        .into());
    }
    open(doc, presentation)
}

pub fn open(doc: Document, presentation: Presentation) -> Result<(), String> {
    if !DESKTOP_ALLOWED.get() {
        return Err("Desktop unavailable".into());
    }
    if let Some(current) = CURRENT.with_borrow(Clone::clone) {
        let mut s = current.borrow_mut();
        if s.ui.get_busy() {
            return Err(tr(
                s.presentation.english,
                "工作台正在处理，请稍后重试。",
                "The editor is busy. Try again shortly.",
            )
            .into());
        }
        if s.text_in_progress() {
            return Err(s.ui.get_status().to_string());
        }
        s.settle_preview()?;
        if s.model.dirty() {
            s.leave = Some(Leave::Replace(doc, presentation));
            s.ui.set_modal(1);
            s.ui.show().map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    create(doc, presentation)
}
fn create(doc: Document, presentation: Presentation) -> Result<(), String> {
    let (w, h) = doc.image.dimensions();
    let model = Editor::new(
        w,
        h,
        doc.image.edits().marks().to_vec(),
        doc.video.map_or(0., |v| v.duration),
    )
    .map_err(str::to_owned)?;
    let ui = crate::motion::create_settings(WorkbenchWindow::new).map_err(|e| e.to_string())?;
    ui.set_labels(labels(presentation.english));
    ui.set_filename(doc.name.clone().into());
    ui.set_dark(presentation.dark);
    ui.set_contrast_mode(presentation.contrast);
    ui.set_accent(presentation.accent);
    ui.set_interface_scale(crate::interface_scale::factor(presentation.scale));
    crate::interface_scale::install_with_theme(
        &ui,
        presentation.scale,
        true,
        presentation.theme,
        |ui, dark| ui.set_dark(dark),
    );
    crate::motion::configure!(&ui, Motion, presentation.reduced);
    crate::motion::install(&ui, presentation.reduced);
    ui.set_video(doc.video.is_some());
    ui.set_category(if doc.video.is_some() { 5 } else { 1 });
    let worker = Worker::new(&ui, doc.clone())?;
    let s = Rc::new(RefCell::new(Session {
        ui: ui.clone_strong(),
        doc,
        model,
        presentation,
        worker,
        selected: None,
        clip: 0,
        preview: None,
        crop: None,
        crop_rect: Rect::default(),
        drag: None,
        text_origin: Point { x: 0., y: 0. },
        text_index: None,
        leave: None,
        timer: slint::Timer::default(),
        clock: None,
        cancel: Arc::new(AtomicBool::new(false)),
        text_result: Arc::new(Mutex::new(None)),
        output_result: Arc::new(Mutex::new(None)),
        output_flags: (false, false),
        audio: None,
        desktop_hidden: false,
    }));
    let weak = Rc::downgrade(&s);
    ui.on_action(move |a| {
        if let Some(s) = weak.upgrade()
            && let Err(e) = action(&s, a.as_str())
        {
            s.borrow().ui.set_status(e.into());
        }
    });
    let weak = Rc::downgrade(&s);
    ui.on_value(move |field, v| {
        if let Some(s) = weak.upgrade() {
            let result = s.borrow_mut().value(field.as_str(), v);
            if let Err(e) = result {
                s.borrow().ui.set_status(e.into());
            }
        }
    });
    let weak = Rc::downgrade(&s);
    ui.on_pointer(move |kind, x, y, unit| {
        if let Some(s) = weak.upgrade() {
            s.borrow_mut().pointer(kind, Point { x, y }, unit);
        }
    });
    let weak = Rc::downgrade(&s);
    ui.on_edit_text(move || {
        if let Some(s) = weak.upgrade() {
            s.borrow_mut().begin_text(true);
        }
    });
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_action("close".into());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    s.borrow_mut().sync();
    s.borrow().render();
    ui.show().map_err(|e| e.to_string())?;
    ui.invoke_focus_canvas();
    let weak = ui.as_weak();
    let _ = slint::spawn_local(async move {
        if let Some(ui) = weak.upgrade()
            && let Ok(w) = ui.window().winit_window().await
        {
            w.focus_window();
        }
    });
    CURRENT.with_borrow_mut(|c| *c = Some(s));
    Ok(())
}
pub fn dismiss_result_for_desktop() {
    crate::result_float::dismiss();
}
pub fn hide_for_desktop() {
    DESKTOP_ALLOWED.set(false);
    dismiss_result_for_desktop();
    if let Some(s) = CURRENT.with_borrow(Clone::clone) {
        let mut s = s.borrow_mut();
        s.stop();
        s.drag = None;
        s.preview = None;
        s.crop = None;
        s.cancel.store(true, Ordering::Release);
        s.desktop_hidden = s.ui.window().is_visible();
        s.ui.set_comparing(false);
        let _ = s.ui.hide();
    }
}
pub fn resume_for_desktop() {
    DESKTOP_ALLOWED.set(true);
    if let Some(s) = CURRENT.with_borrow(Clone::clone) {
        let mut s = s.borrow_mut();
        if std::mem::take(&mut s.desktop_hidden) {
            let _ = s.ui.show();
        }
    }
}
pub fn shutdown() {
    crate::result_float::dismiss();
    CURRENT.with_borrow_mut(|c| {
        c.take();
    });
}
/// Resident exit observes the same dirty guard as the editor's title-bar close.
pub fn request_quit(owner: &crate::settings::SettingsWindow) -> bool {
    let Some(current) = CURRENT.with_borrow(Clone::clone) else {
        return false;
    };
    let mut s = current.borrow_mut();
    s.stop();
    if s.ui.get_busy() {
        s.cancel.store(true, Ordering::Release);
        s.ui.set_status(
            tr(
                s.presentation.english,
                "正在取消处理，完成后可再次退出。",
                "Stopping the current task. Exit again when it finishes.",
            )
            .into(),
        );
        if DESKTOP_ALLOWED.get() {
            let _ = s.ui.show();
        }
        return true;
    }
    if s.text_in_progress() {
        return true;
    }
    if let Err(error) = s.settle_preview() {
        s.ui.set_status(error.into());
        return true;
    }
    if s.model.dirty() {
        s.leave = Some(Leave::Quit(owner.as_weak()));
        s.ui.set_modal(1);
        if DESKTOP_ALLOWED.get() {
            let _ = s.ui.show();
        }
        return true;
    }
    drop(s);
    shutdown();
    false
}
fn tr<'a>(en: bool, zh: &'a str, english: &'a str) -> &'a str {
    if en { english } else { zh }
}
impl Session {
    fn text_in_progress(&self) -> bool {
        if !self.ui.get_text_editing() {
            return false;
        }
        self.ui.set_status(
            tr(
                self.presentation.english,
                "请先按 Enter 完成文字，或 Esc 取消。",
                "Press Enter to finish the text, or Esc to cancel.",
            )
            .into(),
        );
        self.ui.invoke_focus_text();
        true
    }
    fn settle_preview(&mut self) -> Result<(), String> {
        if let Some(next) = self.preview.take() {
            self.commit(next)?;
        }
        Ok(())
    }
    fn effective(&self) -> &State {
        self.preview
            .as_ref()
            .or(self.crop.as_ref())
            .unwrap_or(&self.model.state)
    }
    fn stop(&mut self) {
        self.timer.stop();
        self.audio.take();
        self.clock = None;
        self.ui.set_playing(false);
    }
    fn render(&self) {
        let ticket = self.ui.get_render_ticket().wrapping_add(1);
        self.ui.set_render_ticket(ticket);
        let time = self
            .effective()
            .source_time(f64::from(self.ui.get_position()));
        self.worker.request(Job {
            state: self.effective().clone(),
            time,
            compare: self.ui.get_comparing(),
            ticket,
        });
    }
    fn commit(&mut self, next: State) -> Result<(), String> {
        self.model.apply(next).map_err(str::to_owned)?;
        self.preview = None;
        self.sync();
        self.render();
        Ok(())
    }
    fn sync(&mut self) {
        let s = self.effective().clone();
        let (w, h) = if self.ui.get_comparing() {
            self.doc.image.dimensions()
        } else {
            s.transform.dimensions()
        };
        self.ui.set_iw(w as f32);
        self.ui.set_ih(h as f32);
        self.ui.set_dirty(self.model.dirty());
        self.ui.set_can_undo(self.model.can_undo());
        self.ui.set_can_redo(self.model.can_redo());
        self.ui.set_brightness(s.transform.brightness as i32);
        self.ui.set_contrast(s.transform.contrast as i32);
        self.ui.set_saturation(s.transform.saturation as i32);
        self.ui.set_volume(s.volume as i32);
        self.ui.set_muted(s.muted);
        self.ui.set_duration(s.duration() as f32);
        self.ui.set_position(
            self.ui
                .get_position()
                .min((s.duration() - 0.0001).max(0.) as f32),
        );
        self.ui.set_time_label(
            format!(
                "{} / {}",
                time_label(f64::from(self.ui.get_position())),
                time_label(s.duration())
            )
            .into(),
        );
        let format = Path::new(&self.doc.name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("RGBA")
            .to_ascii_uppercase();
        self.ui
            .set_media_info(format!("{format} · {w} × {h}").into());
        let selected = self.selected.and_then(|i| s.marks.get(i));
        self.ui.set_selected(selected.is_some());
        self.ui.set_grips(model(
            selected
                .map(|m| {
                    m.annotation
                        .resize_handles()
                        .into_iter()
                        .map(|g| {
                            let p = s.transform.project(g.point);
                            EditorGrip { x: p.x, y: p.y }
                        })
                        .collect()
                })
                .unwrap_or_default(),
        ));
        if let Some(mark) = selected {
            let b = mark.annotation.bounds();
            let a = s.transform.project(Point {
                x: b.x as f32,
                y: b.y as f32,
            });
            let b = s.transform.project(Point {
                x: (b.x + b.width) as f32,
                y: (b.y + b.height) as f32,
            });
            self.ui.set_selected_x(a.x.min(b.x));
            self.ui.set_selected_y(a.y.min(b.y));
            self.ui.set_selected_w((a.x - b.x).abs());
            self.ui.set_selected_h((a.y - b.y).abs());
            self.ui.set_mark_from(mark.from as f32);
            self.ui.set_mark_to(mark.to as f32);
        } else if let Some(c) = s.clips.get(self.clip) {
            self.ui.set_mark_from(c.start as f32);
            self.ui.set_mark_to(c.end as f32);
        }
        let en = self.presentation.english;
        self.ui.set_objects(model(
            s.marks
                .iter()
                .enumerate()
                .map(|(i, m)| EditorObject {
                    label: format!("{} · {}", i + 1, kind_name(m.annotation.as_ref(), en)).into(),
                    selected: self.selected == Some(i),
                })
                .collect(),
        ));
        let mut offset = 0.;
        let duration = s.duration().max(0.01);
        self.ui.set_clips(model(
            s.clips
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let left = offset / duration;
                    offset += c.duration();
                    EditorClip {
                        left: left as f32,
                        wide: (c.duration() / duration) as f32,
                        label: format!("{}  {}–{}", i + 1, time_label(c.start), time_label(c.end))
                            .into(),
                        selected: self.clip == i,
                    }
                })
                .collect(),
        ));
        let mut timed = vec![];
        let mut offset = 0.;
        for clip in &s.clips {
            for (i, m) in s.marks.iter().enumerate() {
                let start = m.from.max(clip.start);
                let end = m.to.min(clip.end);
                if end > start {
                    timed.push(EditorTimedMark {
                        left: ((offset + start - clip.start) / duration) as f32,
                        wide: ((end - start) / duration) as f32,
                        label: kind_name(m.annotation.as_ref(), en).into(),
                        selected: self.selected == Some(i),
                        index: i as i32,
                    });
                }
            }
            offset += clip.duration();
        }
        self.ui.set_timed_marks(model(timed));
        self.ui.set_crop_x(self.crop_rect.x as f32);
        self.ui.set_crop_y(self.crop_rect.y as f32);
        self.ui.set_crop_w(self.crop_rect.width as f32);
        self.ui.set_crop_h(self.crop_rect.height as f32);
    }
    fn pointer(&mut self, kind: i32, p: Point, unit: f32) {
        if self.ui.get_comparing() || self.ui.get_busy() {
            return;
        }
        if kind == 3 {
            self.drag = None;
            self.preview = None;
            self.sync();
            self.render();
            return;
        }
        let (w, h) = self.effective().transform.dimensions();
        let p = Point {
            x: p.x.clamp(0., w as f32),
            y: p.y.clamp(0., h as f32),
        };
        let source = self.effective().transform.unproject(p);
        let source = Point {
            x: source
                .x
                .clamp(0., self.doc.image.dimensions().0 as f32 - 1.),
            y: source
                .y
                .clamp(0., self.doc.image.dimensions().1 as f32 - 1.),
        };
        let tolerance =
            8. / unit.max(0.01) * self.effective().transform.crop.width as f32 / w as f32;
        if kind == 0 {
            self.stop();
            self.ui.set_status("".into());
            if self.crop.is_some() {
                let b = self.crop_rect;
                let tol = 10. / unit.max(0.01);
                let mut handle = 0;
                if (p.x - b.x as f32).abs() < tol {
                    handle |= 1;
                }
                if (p.x - (b.x + b.width) as f32).abs() < tol {
                    handle |= 2;
                }
                if (p.y - b.y as f32).abs() < tol {
                    handle |= 4;
                }
                if (p.y - (b.y + b.height) as f32).abs() < tol {
                    handle |= 8;
                }
                self.drag = Some(Drag::Crop {
                    before: b,
                    start: p,
                    handle,
                });
                return;
            }
            let before = self.model.state.clone();
            let handle = self.selected.and_then(|i| {
                before.marks.get(i)?.annotation.resize_handle_hit(
                    source,
                    (tolerance, tolerance),
                    self.doc.image.dimensions(),
                )
            });
            let time = self
                .model
                .state
                .source_time(f64::from(self.ui.get_position()));
            let hit = if handle.is_some() {
                self.selected
            } else {
                before.marks.iter().rposition(|m| {
                    (self.doc.video.is_none() || time >= m.from && time < m.to)
                        && m.annotation.hit_test(source, tolerance)
                })
            };
            if self.ui.get_tool() == 0 || hit.is_some() {
                self.selected = hit;
                if let Some(index) = hit {
                    self.drag = Some(Drag::Object {
                        before,
                        index,
                        start: source,
                        handle,
                    });
                    self.read_style(index);
                }
            } else if self.ui.get_tool() == 4 {
                self.text_origin = source;
                self.text_index = None;
                self.ui.set_text_value("".into());
                self.begin_text(false);
            } else {
                self.selected = None;
                self.drag = Some(Drag::Draw {
                    before,
                    start: source,
                    points: vec![source],
                });
            }
            self.sync();
            return;
        }
        let Some(mut drag) = self.drag.take() else {
            return;
        };
        match &mut drag {
            Drag::Object {
                before,
                index,
                start,
                handle,
            } => {
                let dx = (source.x - start.x).round() as i64;
                let dy = (source.y - start.y).round() as i64;
                let original = &before.marks[*index].annotation;
                let mark = if let Some(handle) = handle {
                    original.resized(*handle, dx, dy, self.doc.image.dimensions())
                } else {
                    original.translated(dx, dy, self.doc.image.dimensions())
                };
                if let Some(mark) = mark {
                    let mut s = before.clone();
                    s.marks[*index].annotation = Arc::new(mark);
                    self.preview = Some(s);
                }
            }
            Drag::Draw {
                before,
                start,
                points,
            } => {
                if points.len() < 4096
                    && points
                        .last()
                        .is_none_or(|q| (q.x - source.x).hypot(q.y - source.y) > 1.)
                {
                    points.push(source);
                }
                if let Some(mark) = self.draw(*start, source, points) {
                    let mut s = before.clone();
                    let duration = self.doc.video.map_or(0., |v| v.duration);
                    s.marks.push(Mark {
                        annotation: Arc::new(mark),
                        from: 0.,
                        to: duration,
                    });
                    self.preview = Some(s);
                }
            }
            Drag::Crop {
                before,
                start,
                handle,
            } => {
                let dx = (p.x - start.x).round() as i64;
                let dy = (p.y - start.y).round() as i64;
                let mut left = i64::from(before.x);
                let mut top = i64::from(before.y);
                let mut right = left + i64::from(before.width);
                let mut bottom = top + i64::from(before.height);
                if *handle == 0 {
                    left = (left + dx).clamp(0, i64::from(w - before.width));
                    top = (top + dy).clamp(0, i64::from(h - before.height));
                    right = left + i64::from(before.width);
                    bottom = top + i64::from(before.height);
                } else {
                    if *handle & 1 != 0 {
                        left = (left + dx).clamp(0, right - 1);
                    }
                    if *handle & 2 != 0 {
                        right = (right + dx).clamp(left + 1, i64::from(w));
                    }
                    if *handle & 4 != 0 {
                        top = (top + dy).clamp(0, bottom - 1);
                    }
                    if *handle & 8 != 0 {
                        bottom = (bottom + dy).clamp(top + 1, i64::from(h));
                    }
                }
                let mut bw = (right - left) as u32;
                let mut bh = (bottom - top) as u32;
                if let Some(ratio) = ratio(self.ui.get_crop_ratio()) {
                    bw = bw.min((bh as f32 * ratio).round().max(1.) as u32);
                    bh = (bw as f32 / ratio).round().max(1.) as u32;
                    if *handle & 1 != 0 {
                        left = right - i64::from(bw);
                    }
                    if *handle & 4 != 0 {
                        top = bottom - i64::from(bh);
                    }
                }
                self.crop_rect = Rect {
                    x: left as u32,
                    y: top as u32,
                    width: bw.min(w - left as u32),
                    height: bh.min(h - top as u32),
                };
            }
        }
        if kind == 2 {
            if let Some(next) = self.preview.take()
                && let Err(error) = self.model.apply(next)
            {
                self.ui.set_status(error.into());
            }
            if matches!(drag, Drag::Draw { .. }) {
                self.selected = self.model.state.marks.len().checked_sub(1);
            }
        } else {
            self.drag = Some(drag);
        }
        self.sync();
        self.render();
    }
    fn draw(&self, start: Point, end: Point, points: &[Point]) -> Option<Annotation> {
        let c = self.ui.get_mark_color();
        let color = [c.red(), c.green(), c.blue(), 255];
        let width = self.ui.get_line_width() as f32;
        let bounds = Rect::between(
            (start.x as u32, start.y as u32),
            (end.x as u32, end.y as u32),
        );
        let mark = match self.ui.get_tool() {
            1 | 2 | 5 => Annotation::from(Shape {
                kind: match self.ui.get_tool() {
                    2 => ShapeKind::Arrow { filled: true },
                    5 => ShapeKind::Highlight,
                    _ => ShapeKind::Rectangle,
                },
                start,
                end,
                color,
                width,
            }),
            3 => Annotation::from(Stroke {
                points: points.to_vec(),
                color,
                width,
            }),
            6 => Annotation::from(Redaction { bounds, color }),
            7 => Annotation::from(Pixelation { bounds }),
            _ => return None,
        };
        mark.is_valid().then_some(mark)
    }
    fn read_style(&self, index: usize) {
        let m = self.model.state.marks[index].annotation.as_ref();
        let (color, width) = match m {
            Annotation::Shape(s) => (s.color, s.width),
            Annotation::Stroke(s) => (s.color, s.width),
            Annotation::Redaction(r) => (r.color, 4.),
            Annotation::Text(t) => {
                self.ui.set_font_size(t.size as i32);
                (t.color, 4.)
            }
            Annotation::Pixelation(_) => return,
        };
        self.ui.set_mark_color(slint::Color::from_argb_u8(
            color[3], color[0], color[1], color[2],
        ));
        self.ui.set_line_width(width.round() as i32);
    }
    fn begin_text(&mut self, existing: bool) {
        if existing {
            let Some(i) = self.selected else {
                return;
            };
            let Annotation::Text(t) = self.model.state.marks[i].annotation.as_ref() else {
                return;
            };
            self.text_origin = Point {
                x: t.origin.0 as f32,
                y: t.origin.1 as f32,
            };
            self.text_index = Some(i);
            self.ui.set_text_value(t.text.clone().into());
        }
        let p = self.model.state.transform.project(self.text_origin);
        self.ui.set_text_x(p.x);
        self.ui.set_text_y(p.y);
        self.ui.set_text_editing(true);
        self.ui.invoke_focus_text();
    }
    fn value(&mut self, field: &str, v: f32) -> Result<(), String> {
        if self.ui.get_busy() {
            return Ok(());
        }
        match field {
            "compare" => {
                self.ui.set_comparing(v != 0.);
                self.sync();
                self.render();
            }
            "seek" => {
                self.stop();
                self.ui
                    .set_position(v.clamp(0., self.model.state.duration() as f32));
                self.sync();
                self.render();
            }
            "brightness" | "contrast" | "saturation" | "volume" => {
                self.stop();
                let mut s = self
                    .preview
                    .take()
                    .unwrap_or_else(|| self.model.state.clone());
                match field {
                    "brightness" => s.transform.brightness = v.round() as i16,
                    "contrast" => s.transform.contrast = v.round() as i16,
                    "saturation" => s.transform.saturation = v.round() as i16,
                    _ => s.volume = v.round() as u16,
                };
                self.preview = Some(s);
                self.sync();
                self.render();
            }
            "ratio" => {
                self.ui.set_crop_ratio(v as i32);
                self.reset_crop();
                self.sync();
            }
            "line" | "font" => {
                if field == "line" {
                    self.ui.set_line_width(v as i32);
                } else {
                    self.ui.set_font_size(v as i32);
                }
                if field == "font"
                    && self.selected.is_some_and(|i| {
                        matches!(
                            self.model.state.marks[i].annotation.as_ref(),
                            Annotation::Text(_)
                        )
                    })
                {
                    self.begin_text(true);
                } else {
                    self.restyle()?;
                }
            }
            "resize-width" | "resize-height" => {
                let (w, h) = self.model.state.transform.dimensions();
                let v = (v.round() as i32).clamp(1, 16384);
                if field == "resize-width" {
                    self.ui.set_resize_width(v);
                    if self.ui.get_linked() {
                        self.ui.set_resize_height(
                            (v as f64 * h as f64 / w as f64).round().clamp(1., 16384.) as i32,
                        );
                    }
                } else {
                    self.ui.set_resize_height(v);
                    if self.ui.get_linked() {
                        self.ui.set_resize_width(
                            (v as f64 * w as f64 / h as f64).round().clamp(1., 16384.) as i32,
                        );
                    }
                }
            }
            "from" | "to" => {
                let mut s = self.model.state.clone();
                let duration = self.doc.video.map_or(0., |v| v.duration);
                if let Some(i) = self.selected {
                    let mark = &mut s.marks[i];
                    if field == "from" {
                        mark.from = f64::from(v).clamp(0., mark.to);
                    } else {
                        mark.to = f64::from(v).clamp(mark.from, duration);
                    }
                } else if let Some(c) = s.clips.get_mut(self.clip) {
                    if field == "from" {
                        c.start = f64::from(v).clamp(0., (c.end - 0.01).max(0.));
                    } else {
                        c.end = f64::from(v).clamp(c.start + 0.01, duration);
                    }
                }
                self.commit(s)?;
            }
            _ => {}
        }
        Ok(())
    }
    fn restyle(&mut self) -> Result<(), String> {
        if let Some(i) = self.selected {
            let c = self.ui.get_mark_color();
            if let Some(mark) = self.model.state.marks[i].annotation.restyled(
                [c.red(), c.green(), c.blue(), 255],
                self.ui.get_line_width() as f32,
                true,
            ) {
                let mut s = self.model.state.clone();
                s.marks[i].annotation = Arc::new(mark);
                self.commit(s)?;
            }
        }
        Ok(())
    }
    fn reset_crop(&mut self) {
        let (w, h) = self.effective().transform.dimensions();
        let (mut bw, mut bh) = ((w as f32 * 0.84) as u32, (h as f32 * 0.8) as u32);
        if let Some(r) = ratio(self.ui.get_crop_ratio()) {
            bw = bw.min((bh as f32 * r) as u32);
            bh = (bw as f32 / r) as u32;
        }
        self.crop_rect = Rect {
            x: (w - bw) / 2,
            y: (h - bh) / 2,
            width: bw.max(1),
            height: bh.max(1),
        };
    }
}
fn ratio(v: i32) -> Option<f32> {
    match v {
        1 => Some(1.),
        2 => Some(4. / 3.),
        3 => Some(16. / 9.),
        _ => None,
    }
}
fn model<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(values)).into()
}
fn time_label(s: f64) -> String {
    format!("{:02}:{:04.1}", (s / 60.).floor() as u64, s % 60.)
}
fn kind_name(m: &Annotation, en: bool) -> &'static str {
    match m {
        Annotation::Text(_) => tr(en, "文字", "Text"),
        Annotation::Redaction(_) => tr(en, "实心遮盖", "Solid cover"),
        Annotation::Pixelation(_) => tr(en, "像素化", "Pixelate"),
        Annotation::Stroke(_) => tr(en, "画笔", "Pen"),
        Annotation::Shape(s) => match s.kind {
            ShapeKind::Rectangle => tr(en, "矩形", "Rectangle"),
            ShapeKind::Arrow { .. } => tr(en, "箭头", "Arrow"),
            ShapeKind::Highlight => tr(en, "高亮", "Highlight"),
        },
    }
}

fn action(shared: &Rc<RefCell<Session>>, a: &str) -> Result<(), String> {
    if a == "preview-failed" {
        shared.borrow_mut().stop();
        return Ok(());
    }
    if a == "text-ready" {
        let mut s = shared.borrow_mut();
        let result = s
            .text_result
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(result) = result {
            s.ui.set_busy(false);
            let mark = result?;
            let mut next = s.model.state.clone();
            if let Some(i) = s.text_index {
                next.marks[i].annotation = Arc::new(mark.into());
            } else {
                let duration = s.doc.video.map_or(0., |v| v.duration);
                next.marks.push(Mark {
                    annotation: Arc::new(mark.into()),
                    from: 0.,
                    to: duration,
                });
                s.selected = Some(next.marks.len() - 1);
            }
            s.commit(next)?;
            s.ui.set_text_editing(false);
            s.ui.invoke_focus_canvas();
        }
        return Ok(());
    }
    if a == "output-ready" {
        let mut s = shared.borrow_mut();
        let result = s
            .output_result
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(result) = result {
            s.ui.set_busy(false);
            result?;
            let (copy, leave_after) = s.output_flags;
            if !copy {
                s.model.saved();
            }
            s.ui.set_status(
                tr(
                    s.presentation.english,
                    if copy {
                        "已复制到剪贴板"
                    } else {
                        "已保存副本"
                    },
                    if copy {
                        "Copied to clipboard"
                    } else {
                        "Copy saved"
                    },
                )
                .into(),
            );
            s.sync();
            if leave_after {
                drop(s);
                return leave(shared);
            }
        }
        return Ok(());
    }
    if matches!(a, "save" | "save-leave") {
        return export::show(shared, a == "save-leave");
    }
    if a == "export-confirm" {
        let leave_after = shared.borrow().output_flags.1;
        return output(shared, if leave_after { "save-leave" } else { "save" });
    }
    if a == "copy" {
        return output(shared, a);
    }
    if a == "text-commit" {
        return text_commit(shared);
    }
    if a == "discard" {
        return leave(shared);
    }
    if a == "open-picker" {
        return choose_file(shared);
    }
    let mut s = shared.borrow_mut();
    if s.ui.get_busy() {
        if a == "escape" {
            s.cancel.store(true, Ordering::Release);
        }
        return Ok(());
    }
    s.ui.set_status("".into());
    if let Some(c) = a.strip_prefix("category:") {
        if s.ui.get_text_editing() {
            s.ui.set_status(
                tr(
                    s.presentation.english,
                    "请先按 Enter 完成文字，或 Esc 取消。",
                    "Press Enter to finish the text, or Esc to cancel.",
                )
                .into(),
            );
            return Ok(());
        }
        if let Some(preview) = s.preview.take() {
            s.model.apply(preview).map_err(str::to_owned)?;
        }
        s.stop();
        s.crop = None;
        s.drag = None;
        s.selected = None;
        let (c, tool) = match c.split_once(':') {
            Some((category, tool)) => (
                category.parse::<i32>().map_err(|e| e.to_string())?,
                Some(tool.parse::<i32>().map_err(|e| e.to_string())?),
            ),
            None => (c.parse::<i32>().map_err(|e| e.to_string())?, None),
        };
        s.ui.set_category(c);
        s.ui.set_tool(tool.unwrap_or(match c {
            2 => 4,
            3 => 6,
            _ => 0,
        }));
        if c == 0 {
            s.crop = Some(s.model.state.clone());
            s.ui.set_crop_ratio(0);
            s.ui.set_zoom(0.);
            s.ui.set_pan_x(0.);
            s.ui.set_pan_y(0.);
            s.reset_crop();
        }
        s.sync();
        s.render();
        return Ok(());
    }
    if let Some(v) = a.strip_prefix("tool:") {
        s.selected = None;
        s.ui.set_tool(v.parse::<i32>().unwrap_or(0));
        s.sync();
        return Ok(());
    }
    if let Some(v) = a.strip_prefix("color:") {
        if let Some(c) = parse_color(v) {
            s.ui.set_mark_color(c);
            s.restyle()?;
        }
        return Ok(());
    }
    if let Some(v) = a.strip_prefix("object:") {
        s.selected = v.parse().ok().filter(|i| *i < s.model.state.marks.len());
        s.ui.set_tool(0);
        s.ui.set_category(1);
        s.crop = None;
        if let Some(i) = s.selected {
            s.read_style(i);
        }
        s.sync();
        return Ok(());
    }
    if let Some(v) = a.strip_prefix("clip:") {
        s.clip = v.parse().unwrap_or(0);
        s.selected = None;
        let p = s
            .model
            .state
            .clips
            .iter()
            .take(s.clip)
            .map(|c| c.duration())
            .sum::<f64>();
        s.ui.set_position(p as f32);
        s.sync();
        s.render();
        return Ok(());
    }
    match a {
        "modal-cancel" => {
            s.leave = None;
            s.ui.set_modal(0);
            s.ui.invoke_focus_canvas();
        }
        "close" | "open" => {
            if s.text_in_progress() {
                return Ok(());
            }
            s.settle_preview()?;
            s.stop();
            s.crop = None;
            s.preview = None;
            s.ui.set_comparing(false);
            s.leave = Some(if a == "close" {
                Leave::Close
            } else {
                Leave::Open
            });
            if s.model.dirty() {
                s.ui.set_modal(1);
                return Ok(());
            }
            drop(s);
            return leave(shared);
        }
        "escape" => {
            if s.ui.get_more() {
                s.ui.set_more(false);
            } else if s.ui.get_text_editing() {
                s.ui.set_text_editing(false);
                s.ui.invoke_focus_canvas();
            } else if s.ui.get_modal() > 0 {
                s.ui.set_modal(0);
                s.leave = None;
            } else if s.crop.is_some() {
                s.crop = None;
                s.ui.set_category(1);
            } else {
                s.selected = None;
                s.drag = None;
                s.preview = None;
                s.ui.set_more(false);
            }
            s.ui.set_comparing(false);
        }
        "undo" | "redo" => {
            s.stop();
            s.crop = None;
            s.preview = None;
            s.selected = None;
            s.ui.set_category(1);
            if a == "undo" {
                s.model.undo();
            } else {
                s.model.redo();
            }
        }
        "rotate" | "flip" => {
            if let Some(mut crop) = s.crop.take() {
                if a == "rotate" {
                    crop.transform.turns = (crop.transform.turns + 1) % 4;
                    crop.transform.size = None;
                } else {
                    crop.transform.flip = !crop.transform.flip;
                }
                s.crop = Some(crop);
                s.reset_crop();
            }
        }
        "crop-cancel" => {
            s.crop = None;
            s.ui.set_category(1);
        }
        "crop-apply" => {
            if let Some(mut draft) = s.crop.take() {
                draft.transform = draft.transform.cropped(s.crop_rect).ok_or("Invalid crop")?;
                s.model.apply(draft).map_err(str::to_owned)?;
                s.ui.set_category(1);
                s.ui.set_zoom(0.);
                s.ui.set_pan_x(0.);
                s.ui.set_pan_y(0.);
            }
        }
        "adjust-commit" => {
            if let Some(preview) = s.preview.take() {
                s.model.apply(preview).map_err(str::to_owned)?;
            }
        }
        "adjust-reset" => {
            let mut next = s.model.state.clone();
            next.transform.brightness = 100;
            next.transform.contrast = 100;
            next.transform.saturation = 100;
            s.commit(next)?;
        }
        "delete" => {
            if let Some(i) = s.selected.take() {
                let mut next = s.model.state.clone();
                next.marks.remove(i);
                s.commit(next)?;
            }
        }
        "duplicate" => {
            if let Some(i) = s.selected {
                let mut next = s.model.state.clone();
                let mut mark = next.marks[i].clone();
                if let Some(m) = mark.annotation.translated(16, 16, s.doc.image.dimensions()) {
                    mark.annotation = Arc::new(m);
                    next.marks.push(mark);
                    s.selected = Some(next.marks.len() - 1);
                    s.commit(next)?;
                }
            }
        }
        "text-cancel" => {
            s.ui.set_text_editing(false);
            s.ui.invoke_focus_canvas();
        }
        "resize-dialog" => {
            let (w, h) = s.model.state.transform.dimensions();
            s.ui.set_resize_width(w as i32);
            s.ui.set_resize_height(h as i32);
            s.ui.set_modal(2);
        }
        "resize-apply" => {
            let mut next = s.model.state.clone();
            next.transform.size = Some((
                s.ui.get_resize_width() as u32,
                s.ui.get_resize_height() as u32,
            ));
            s.commit(next)?;
            s.ui.set_modal(0);
            s.ui.set_zoom(0.);
        }
        "split" => {
            s.stop();
            let mut next = s.model.state.clone();
            if next.split(f64::from(s.ui.get_position())) {
                s.commit(next)?;
            }
        }
        "delete-clip" => {
            s.stop();
            let mut next = s.model.state.clone();
            if next.clips.len() > 1 && s.clip < next.clips.len() {
                next.clips.remove(s.clip);
                s.clip = s.clip.saturating_sub(1);
                s.commit(next)?;
            } else {
                s.ui.set_status(
                    tr(
                        s.presentation.english,
                        "至少保留一个片段。",
                        "Keep at least one clip.",
                    )
                    .into(),
                );
            }
        }
        "mute" => {
            s.stop();
            let mut next = s.model.state.clone();
            next.muted = !next.muted;
            s.commit(next)?;
        }
        "frame-back" | "frame-next" => {
            s.stop();
            let fps = s.doc.video.map_or(30, |v| v.fps) as f32;
            let p = s.ui.get_position()
                + if a == "frame-back" {
                    -1. / fps
                } else {
                    1. / fps
                };
            s.ui.set_position(p.clamp(0., (s.model.state.duration() - 0.0001).max(0.) as f32));
        }
        "play" => {
            if s.doc.video.is_none() {
                return Ok(());
            }
            if s.clock.is_some() {
                s.stop();
            } else {
                if f64::from(s.ui.get_position()) >= s.model.state.duration() - 0.04 {
                    s.ui.set_position(0.);
                }
                s.clock = Some((Instant::now(), f64::from(s.ui.get_position())));
                if s.doc.video.is_some_and(|v| v.audio) && !s.model.state.muted {
                    s.audio = Some(glint_platform::recording::AudioPreview::start(
                        s.doc.path.clone().ok_or("Video path unavailable")?,
                        s.model.state.clone(),
                        f64::from(s.ui.get_position()),
                    )?);
                }
                s.ui.set_playing(true);
                let weak: Weak<RefCell<Session>> = Rc::downgrade(shared);
                s.timer.start(
                    slint::TimerMode::Repeated,
                    Duration::from_millis(33),
                    move || {
                        if let Some(s) = weak.upgrade() {
                            let mut s = s.borrow_mut();
                            if let Some((clock, base)) = s.clock {
                                if let Some(error) = s.audio.as_ref().and_then(|a| a.error()) {
                                    s.stop();
                                    s.ui.set_status(error.into());
                                    return;
                                }
                                let elapsed = if let Some(audio) = &s.audio {
                                    let Some(elapsed) = audio.elapsed() else {
                                        return;
                                    };
                                    elapsed
                                } else {
                                    clock.elapsed()
                                };
                                let p = base + elapsed.as_secs_f64();
                                s.ui.set_position(p as f32);
                                if p >= s.model.state.duration() {
                                    s.stop();
                                }
                                s.sync();
                                s.render();
                            }
                        }
                    },
                );
            }
        }
        _ => {}
    }
    s.sync();
    s.render();
    Ok(())
}
fn parse_color(s: &str) -> Option<slint::Color> {
    let n = *[0x4b5cfa, 0xef5260, 0xf0b93a, 0x2ba68b, 0x202b42, 0xffffff]
        .get(s.parse::<usize>().ok()?)?;
    Some(slint::Color::from_rgb_u8(
        (n >> 16) as u8,
        (n >> 8) as u8,
        n as u8,
    ))
}
