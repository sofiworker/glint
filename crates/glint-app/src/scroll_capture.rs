//! Manual, event-driven scrolling capture. One bounded worker owns the stitch
//! session while busy; late completions cannot revive a closed capture.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use glint_core::{
    Edits, Rect,
    scroll_capture::{Append, Fragment, ScrollSession, StitchError},
};
use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

glint_ui::slint! {
    export { Motion } from "@glint_ui/widgets/motion.slint";
    export { ScrollCaptureWindow, ScrollLabels } from "src/scroll_capture.slint";
}

static NEXT: AtomicU64 = AtomicU64::new(1);
static WORKER_BUSY: AtomicBool = AtomicBool::new(false);
struct WorkerSlot;
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        WORKER_BUSY.store(false, Ordering::Release);
    }
}

pub struct Run {
    ui: ScrollCaptureWindow,
    ticket: u64,
    region: (i32, i32, u32, u32),
    displays: Vec<glint_platform::DisplayInfo>,
    exclusions: Vec<String>,
    work: (i32, i32, u32, u32),
    session: Option<ScrollSession>,
    result: Option<mpsc::Receiver<Completion>>,
    target: Option<glint_platform::FocusTarget>,
    english: bool,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Run {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.ui.hide();
    }
}

enum Task {
    Capture,
    Undo,
    Discard,
    Accept(u32),
    Preview(Option<u32>),
    Edit,
}
struct Preview {
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    seams: Vec<f32>,
}
struct Completion {
    session: ScrollSession,
    notice: Message,
    preview: Option<Preview>,
    prepared: Option<crate::pin_edit::Prepared>,
}

fn tr(id: Message, english: bool) -> slint::SharedString {
    messages::text(id, english).into()
}

fn preview(session: &ScrollSession, overlap: Option<u32>) -> Result<Preview, StitchError> {
    let frame = session.compose(overlap)?;
    let mut seams = session.seams();
    if overlap.is_some()
        && let Some((_, h)) = session.dimensions()
    {
        seams.push(h);
    }
    let ratio = (480. / f64::from(frame.width))
        .min((2_000_000. / (f64::from(frame.width) * f64::from(frame.height))).sqrt())
        .min(1.);
    let w = (f64::from(frame.width) * ratio).round().max(1.) as u32;
    let h = (f64::from(frame.height) * ratio).round().max(1.) as u32;
    let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)
        .ok_or(StitchError::InvalidFrame)?;
    let small = image::imageops::resize(&image, w, h, image::imageops::FilterType::Triangle);
    Ok(Preview {
        pixels: SharedPixelBuffer::clone_from_slice(small.as_raw(), w, h),
        seams: seams
            .into_iter()
            .map(|y| y as f32 * h as f32 / image.height() as f32)
            .collect(),
    })
}

fn prepare(
    session: &ScrollSession,
    work: (i32, i32, u32, u32),
) -> Result<crate::pin_edit::Prepared, StitchError> {
    let frame = session.compose(None)?;
    // Keep source pixels and modest margins. The editor scrolls this canvas;
    // it never expands a portrait image to the desktop's aspect ratio.
    let (w, h) = (frame.width + 80, frame.height + 220);
    let length = u64::from(w) * u64::from(h) * 4;
    if length > 96 * 1024 * 1024 {
        return Err(StitchError::Budget);
    }
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(length as usize)
        .map_err(|_| StitchError::Budget)?;
    rgba.resize(length as usize, 0);
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&[24, 32, 44, 255]);
    }
    for y in 0..frame.height as usize {
        let from = y * frame.width as usize * 4;
        let to = ((y + 140) * w as usize + 40) * 4;
        rgba[to..to + frame.width as usize * 4]
            .copy_from_slice(&frame.rgba[from..from + frame.width as usize * 4]);
    }
    Ok(crate::pin_edit::Prepared {
        frame: glint_platform::DesktopFrame {
            x: work.0,
            y: work.1,
            width: w,
            height: h,
            rgba,
        },
        selection: Rect {
            x: 40,
            y: 140,
            width: frame.width,
            height: frame.height,
        },
        edits: Edits::default(),
        window_size: (work.2, work.3),
        turns: 0,
    })
}

fn start(
    state: &mut AppState,
    settings: &SettingsWindow,
    region: (i32, i32, u32, u32),
) -> Result<(), String> {
    glint_platform::diagnostic_log::emit(glint_platform::diagnostic_log::Event::ScrollStarted);
    let capture = state.capture.as_ref().ok_or("No active capture")?;
    let displays = glint_platform::desktop_environment()?.displays;
    let (x, y, w, h) = region;
    let display = displays
        .iter()
        .find(|d| {
            x >= d.bounds.left
                && y >= d.bounds.top
                && i64::from(x) + i64::from(w) <= i64::from(d.bounds.right)
                && i64::from(y) + i64::from(h) <= i64::from(d.bounds.bottom)
        })
        .ok_or("Region must fit one display")?;
    if w < 32 || h < 64 || u64::from(w) * u64::from(h) * 4 > 64 * 1024 * 1024 {
        return Err("Invalid scrolling region".into());
    }
    let b = display.work_area;
    let work = (
        b.left,
        b.top,
        (b.right - b.left) as u32,
        (b.bottom - b.top) as u32,
    );
    let english = capture.get_english();
    let ui = crate::motion::create_settings(ScrollCaptureWindow::new).map_err(|e| e.to_string())?;
    crate::motion::configure!(&ui, Motion, true);
    crate::motion::install(&ui, true);
    crate::interface_scale::install_factor(&ui, capture.get_interface_scale());
    ui.set_dark(capture.get_dark());
    ui.set_accent(capture.get_accent());
    ui.set_labels(ScrollLabels {
        title: tr(Message::ScrollTitle, english),
        hint: tr(Message::ScrollHint, english),
        capture: tr(Message::ScrollCapture, english),
        undo: tr(Message::ScrollUndo, english),
        stop: tr(Message::ScrollStop, english),
        edit: tr(Message::ScrollEdit, english),
        cancel: tr(Message::CaptureCancel, english),
        overlap: tr(Message::ScrollOverlap, english),
        accept: tr(Message::ScrollAccept, english),
        discard: tr(Message::ScrollDiscard, english),
        preview: tr(Message::ScrollPreview, english),
        resume: tr(Message::ScrollResume, english),
    });
    ui.set_maximum_overlap(h as i32 - 1);
    ui.set_overlap((h / 3) as i32);
    let ticket = NEXT.fetch_add(1, Ordering::Relaxed);
    let owner = settings.as_weak();
    ui.on_action(move |action| {
        if let Some(owner) = owner.upgrade() {
            owner.invoke_action(format!("scroll:{ticket}:{action}").into());
        }
    });
    let owner = settings.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(owner) = owner.upgrade() {
            owner.invoke_action(format!("scroll:{ticket}:cancel").into());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let exclusions =
        glint_core::exclusions::parse(&state.session.preferences.saved().privacy.excluded_apps)
            .map_err(|_| "Invalid exclusions")?;
    capture.hide().map_err(|e| e.to_string())?;
    if let Some(target) = &state.capture_focus {
        let _ = target.restore();
    }
    let target = glint_platform::remember_foreground();
    ui.window().set_position(slint::PhysicalPosition::new(
        work.0 + (work.2 as i32 - 500).max(0),
        work.1 + 20,
    ));
    // Defer the first show until capture finishes, so the panel cannot enter frame 1.
    state.scroll_capture = Some(Run {
        ui,
        ticket,
        region,
        displays,
        exclusions,
        work,
        session: Some(ScrollSession::default()),
        result: None,
        target,
        english,
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    Ok(())
}

fn queue(run: &mut Run, settings: &SettingsWindow, task: Task) {
    if run.result.is_some() {
        return;
    }
    if WORKER_BUSY.swap(true, Ordering::AcqRel) {
        run.ui
            .set_status(tr(Message::CaptureOutputPending, run.english));
        let _ = run.ui.show();
        return;
    }
    let slot = WorkerSlot;
    let Some(session) = run.session.take() else {
        return;
    };
    glint_platform::diagnostic_log::emit(glint_platform::diagnostic_log::Event::ScrollQueued);
    run.ui.set_busy(true);
    run.ui.set_status(tr(Message::ScrollBusy, run.english));
    let capture = matches!(task, Task::Capture);
    if capture {
        if run.ui.hide().is_err() {
            run.session = Some(session);
            run.ui.set_busy(false);
            run.ui.set_status(tr(Message::ScrollFailed, run.english));
            return;
        }
        if let Some(target) = &run.target {
            let _ = target.restore();
        }
    }
    let region = run.region;
    let displays = run.displays.clone();
    let exclusions = run.exclusions.clone();
    let work = run.work;
    let ticket = run.ticket;
    let (send, receive) = mpsc::sync_channel(1);
    run.result = Some(receive);
    let owner = settings.as_weak();
    let cancelled = run.cancelled.clone();
    // One short compositor settle delay per explicit capture; never a sampling loop.
    slint::Timer::single_shot(
        if capture {
            Duration::from_millis(160)
        } else {
            Duration::ZERO
        },
        move || {
            if cancelled.load(Ordering::Acquire) {
                return;
            }
            let retained = Arc::new(Mutex::new(Some(session)));
            let worker_session = retained.clone();
            let failure = send.clone();
            let failed_owner = owner.clone();
            let worker = std::thread::Builder::new()
                .name("glint-scroll-capture".into())
                .spawn(move || {
                    let _slot = slot;
                    let Some(mut session) = worker_session
                        .lock()
                        .ok()
                        .and_then(|mut value| value.take())
                    else {
                        return;
                    };
                    if cancelled.load(Ordering::Acquire) {
                        return;
                    }
                    let mut notice = Message::ScrollReady;
                    let mut image = None;
                    let mut prepared = None;
                    let result: Result<(), StitchError> = (|| {
                        match task {
                            Task::Capture => {
                                let frame = match glint_platform::capture_region_guarded(
                                    region,
                                    &displays,
                                    &exclusions,
                                ) {
                                    Ok(frame) => frame,
                                    Err(_) => {
                                        notice = Message::ScrollFailed;
                                        return Ok(());
                                    }
                                };
                                if cancelled.load(Ordering::Acquire) {
                                    return Ok(());
                                }
                                let _ = owner.upgrade_in_event_loop(move |ui| {
                                    ui.invoke_action(format!("scroll:{ticket}:acquired").into());
                                });
                                match session.append(Fragment::new(
                                    frame.width,
                                    frame.height,
                                    frame.rgba,
                                )?)? {
                                    Append::First | Append::Added { .. } => {}
                                    Append::Duplicate => notice = Message::ScrollDuplicate,
                                    Append::NeedsReview => notice = Message::ScrollUncertain,
                                }
                            }
                            Task::Undo => session.undo(),
                            Task::Discard => session.discard_pending(),
                            Task::Accept(overlap) => session.accept_pending(overlap)?,
                            Task::Preview(overlap) => {
                                image = Some(preview(&session, overlap)?);
                                notice = if overlap.is_some() {
                                    Message::ScrollUncertain
                                } else {
                                    Message::ScrollReviewHint
                                };
                            }
                            Task::Edit => prepared = Some(prepare(&session, work)?),
                        }
                        Ok(())
                    })();
                    if let Err(error) = result {
                        notice = if error == StitchError::Budget {
                            Message::ScrollBudget
                        } else {
                            Message::ScrollFailed
                        };
                    }
                    let _ = send.send(Completion {
                        session,
                        notice,
                        preview: image,
                        prepared,
                    });
                    // Release the global slot before delivering the next UI action.
                    drop(_slot);
                    let _ = owner.upgrade_in_event_loop(move |ui| {
                        ui.invoke_action(format!("scroll:{ticket}:ready").into())
                    });
                });
            if worker.is_err() {
                if let Some(session) = retained.lock().ok().and_then(|mut value| value.take()) {
                    let _ = failure.send(Completion {
                        session,
                        notice: Message::ScrollFailed,
                        preview: None,
                        prepared: None,
                    });
                }
                let _ = failed_owner.upgrade_in_event_loop(move |ui| {
                    ui.invoke_action(format!("scroll:{ticket}:ready").into())
                });
            }
        },
    );
}

/// Called before the resident coordinator borrows AppState for ordinary actions.
pub fn handle(action: &str, shared: &Rc<RefCell<AppState>>, settings: &SettingsWindow) -> bool {
    if let Some(region) = action.strip_prefix("scroll-start:") {
        let parts: Vec<_> = region
            .split(':')
            .filter_map(|n| n.parse::<i32>().ok())
            .collect();
        let mut state = shared.borrow_mut();
        if state.scroll_capture.is_some() || state.lifecycle.blocked() {
            return true;
        }
        if let [x, y, w, h] = parts.as_slice()
            && *w > 0
            && *h > 0
        {
            if start(&mut state, settings, (*x, *y, *w as u32, *h as u32)).is_ok() {
                if let Some(run) = &mut state.scroll_capture {
                    queue(run, settings, Task::Capture);
                }
            } else if let Some(capture) = &state.capture {
                capture.set_service_notice(tr(Message::ScrollPlainRegion, capture.get_english()));
            }
        }
        return true;
    }
    let capture_hotkey = action == "capture" && shared.borrow().scroll_capture.is_some();
    let Some(command) = action.strip_prefix("scroll:") else {
        if capture_hotkey {
            let mut state = shared.borrow_mut();
            if !state.lifecycle.blocked()
                && let Some(run) = &mut state.scroll_capture
                && run
                    .session
                    .as_ref()
                    .is_some_and(|s| s.pending_height().is_none())
            {
                run.ui.set_review(false);
                queue(run, settings, Task::Capture);
            }
            return true;
        }
        // Keep settings/other capture modes from entering the live capture region.
        // Tray Settings also serves as recovery if the panel was moved offscreen.
        if shared.borrow().scroll_capture.is_some()
            && matches!(
                action,
                "open"
                    | "window-capture"
                    | "repeat-capture"
                    | "all-screens-capture"
                    | "current-screen-capture"
                    | "color-picker"
                    | "local-image"
                    | "quit"
                    | "rescue-pins"
            )
        {
            let state = shared.borrow();
            if let Some(run) = &state.scroll_capture
                && !state.lifecycle.blocked()
            {
                run.ui.window().set_position(slint::PhysicalPosition::new(
                    run.work.0 + 20,
                    run.work.1 + 20,
                ));
                // Do not show during the short pre-capture delay. Acquired / ready
                // restores it after GDI has taken the source pixels.
                if run.result.is_none() {
                    let _ = run.ui.show();
                }
            }
            return true;
        }
        return false;
    };
    let Some((ticket, command)) = command.split_once(':') else {
        return true;
    };
    let mut state = shared.borrow_mut();
    if state.lifecycle.blocked() {
        return true;
    }
    let Some(run) = &mut state.scroll_capture else {
        return true;
    };
    if ticket.parse::<u64>().ok() != Some(run.ticket) {
        return true;
    }
    if command == "acquired" {
        glint_platform::diagnostic_log::emit(glint_platform::diagnostic_log::Event::ScrollAcquired);
        let _ = run.ui.show();
        return true;
    }
    if command == "cancel" {
        glint_platform::diagnostic_log::emit(
            glint_platform::diagnostic_log::Event::ScrollCancelled,
        );
        state.scroll_capture.take();
        if let Some(capture) = &state.capture {
            let _ = capture.show();
        }
        return true;
    }
    if command == "ready" {
        glint_platform::diagnostic_log::emit(glint_platform::diagnostic_log::Event::ScrollReady);
        let result = run.result.take().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = result {
            if let Some(prepared) = result.prepared {
                let (send, receive) = mpsc::sync_channel(1);
                let _ = send.send(Ok(crate::pin_edit::Input::Scroll(prepared)));
                state.scroll_capture.take();
                if let Some(capture) = state.capture.take() {
                    let _ = capture.hide();
                }
                state.capture_result = Some(receive);
                state.capturing = true;
                // Do not invoke the old capture's finished callback: this is one
                // continuing capture, with the original focus/pin restoration.
                drop(state);
                settings.invoke_action("capture-ready".into());
                return true;
            }
            run.ui.set_busy(false);
            run.ui
                .set_pending(result.session.pending_height().is_some());
            run.ui.set_can_undo(
                result.session.count() > 1 || result.session.pending_height().is_some(),
            );
            run.ui.set_can_finish(result.session.count() > 0);
            let dimensions = result.session.dimensions().unwrap_or_default();
            run.ui.set_status(
                format!(
                    "{} · {} · {} × {} px",
                    messages::text(result.notice, run.english),
                    result.session.count(),
                    dimensions.0,
                    dimensions.1
                )
                .into(),
            );
            if let Some(preview) = result.preview {
                run.ui.set_picture_width(preview.pixels.width() as f32);
                run.ui.set_picture_height(preview.pixels.height() as f32);
                run.ui.set_picture(slint::Image::from_rgba8(preview.pixels));
                run.ui
                    .set_seams(ModelRc::new(VecModel::from(preview.seams)));
                run.ui.set_review(true);
                let scale = run.ui.window().scale_factor();
                run.ui.window().set_size(slint::PhysicalSize::new(
                    (470. * scale) as u32,
                    ((680. * scale) as u32)
                        .min(run.work.3.saturating_sub(40))
                        .max(280),
                ));
            } else {
                // Never display a stale result after undoing/changing a join.
                run.ui.set_review(false);
                run.ui.set_picture(slint::Image::default());
                if run.ui.get_pending() {
                    let scale = run.ui.window().scale_factor();
                    run.ui.window().set_size(slint::PhysicalSize::new(
                        (470. * scale) as u32,
                        ((450. * scale) as u32)
                            .min(run.work.3.saturating_sub(40))
                            .max(280),
                    ));
                }
            }
            run.session = Some(result.session);
            let _ = run.ui.show();
        }
        return true;
    }
    if run.result.is_some() {
        return true;
    }
    let Some(session) = &run.session else {
        return true;
    };
    let overlap = run.ui.get_overlap().max(0) as u32;
    let task = match command {
        "capture" if session.pending_height().is_none() => Task::Capture,
        "undo" => Task::Undo,
        "discard" => Task::Discard,
        "accept" => Task::Accept(overlap),
        "preview" => Task::Preview(Some(overlap)),
        "stop" => Task::Preview(None),
        "edit" if session.pending_height().is_none() => Task::Edit,
        "resume" => {
            run.ui.set_review(false);
            return true;
        }
        _ => return true,
    };
    queue(run, settings, task);
    true
}
