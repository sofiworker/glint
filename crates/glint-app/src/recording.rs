//! Recording presentation and lifecycle. Pixels and encoding stay in glint-platform.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_platform::{
    diagnostic_log::{Event, emit},
    recording::{self as platform, Recorder, Source},
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

glint_ui::slint! {
    export { Motion } from "@glint_ui/widgets/motion.slint";
    export { RecordingWindow, RecordingCountdown } from "src/recording.slint";
}
type PosterResult = (PathBuf, Result<crate::result_float::Pending, String>);
thread_local! { static POSTER: RefCell<Option<PosterResult>> = const { RefCell::new(None) }; }
pub struct Run {
    ui: RecordingWindow,
    sources: Vec<Source>,
    crop: Option<(u32, u32)>,
    recorder: Option<Recorder>,
    result: Arc<Mutex<Option<Result<PathBuf, String>>>>,
    timer: slint::Timer,
    countdown: u8,
    quit: bool,
    picker: Option<glint_platform::FolderPickerControl>,
    directory: Option<PathBuf>,
    file: Option<PathBuf>,
    editor_document: Option<crate::workbench::Document>,
    frame: Option<glint_platform::LiveCaptureFrame>,
    border: Option<glint_platform::LiveBorder>,
    countdown_ui: Option<RecordingCountdown>,
    live_region: Option<platform::Region>,
}
impl Run {
    fn new(
        ui: RecordingWindow,
        sources: Vec<Source>,
        crop: Option<(u32, u32)>,
        live_region: Option<platform::Region>,
    ) -> Self {
        Self {
            ui,
            sources,
            crop,
            recorder: None,
            result: Arc::new(Mutex::new(None)),
            timer: slint::Timer::default(),
            countdown: 0,
            quit: false,
            picker: None,
            directory: None,
            file: None,
            editor_document: None,
            frame: None,
            border: None,
            countdown_ui: None,
            live_region,
        }
    }

    fn clear_countdown(&mut self) {
        self.timer.stop();
        self.countdown = 0;
        if let Some(badge) = self.countdown_ui.take() {
            let _ = badge.hide();
        }
        self.frame.take();
    }

    fn set_countdown(&mut self, seconds: u8) {
        self.countdown = seconds;
        if let Some(badge) = &self.countdown_ui {
            badge.set_seconds(i32::from(seconds));
        }
        self.ui.set_status(
            if self.ui.get_english() {
                format!("Recording starts in {seconds}s")
            } else {
                format!("即将录制 · {seconds} 秒")
            }
            .into(),
        );
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        self.clear_countdown();
        if let Some(control) = self.picker.take() {
            let _ = control.cancel();
        }
        let _ = self.ui.hide();
    }
}
fn tr(en: bool, zh: &str, english: &str) -> slint::SharedString {
    if en { english } else { zh }.into()
}

fn place_recording_result(ui: &RecordingWindow) {
    ui.set_compact(false);
    let window = ui.window();
    let scale = window.scale_factor();
    let preferred_width = (680. * scale).round() as u32;
    let preferred_height = (500. * scale).round() as u32;
    let work = window
        .with_winit_window(|peer| {
            let handle = peer.window_handle().ok()?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return None;
            };
            let native =
                unsafe { glint_platform::NativePinWindow::from_raw(handle.hwnd.get() as *mut _) };
            // Resolve the monitor while the toolbar is still near the recording region.
            let work = native.work_area().ok()?;
            peer.set_decorations(true);
            let outer = peer.outer_size();
            let inner = peer.inner_size();
            Some((
                work,
                outer.width.saturating_sub(inner.width),
                outer.height.saturating_sub(inner.height),
            ))
        })
        .flatten();
    if let Some(((left, top, width, height), frame_width, frame_height)) = work {
        window.set_size(slint::PhysicalSize::new(
            preferred_width.min(width.saturating_sub(frame_width)),
            preferred_height.min(height.saturating_sub(frame_height)),
        ));
        window.with_winit_window(|peer| {
            let outer = peer.outer_size();
            window.set_position(slint::PhysicalPosition::new(
                left + (width.saturating_sub(outer.width) / 2) as i32,
                top + (height.saturating_sub(outer.height) / 2) as i32,
            ));
        });
    } else {
        window.set_size(slint::PhysicalSize::new(preferred_width, preferred_height));
    }
}

fn source_model(sources: &[Source], en: bool) -> ModelRc<slint::SharedString> {
    Rc::new(VecModel::from(
        sources
            .iter()
            .map(|s| {
                format!(
                    "{} · {} ({} × {})",
                    if s.window {
                        if en { "Window" } else { "窗口" }
                    } else if en {
                        "Display"
                    } else {
                        "屏幕"
                    },
                    s.name,
                    s.width,
                    s.height
                )
                .into()
            })
            .collect::<Vec<_>>(),
    ))
    .into()
}
fn open(
    state: &mut AppState,
    owner: &SettingsWindow,
    region: Option<platform::Region>,
) -> Result<(), String> {
    if let Some(run) = &state.screen_recording {
        return run.ui.show().map_err(|e| e.to_string());
    }
    if state.lifecycle.blocked() || state.capturing || state.scroll_capture.is_some() {
        return Err(
            "Recording is unavailable during capture or while the desktop is locked".into(),
        );
    }
    let (sources, crop) = if let Some(region) = region {
        let (source, crop) = platform::region_source(region)?;
        (vec![source], Some(crop))
    } else {
        (platform::sources()?, None)
    };
    let ui = crate::motion::create_settings(RecordingWindow::new).map_err(|e| e.to_string())?;
    crate::motion::configure!(&ui, Motion, true);
    crate::motion::install(&ui, true);
    let en = state.en();
    ui.set_english(en);
    ui.set_dark(owner.get_dark());
    ui.set_accent(owner.get_accent());
    ui.set_region(region.is_some());
    ui.set_sources(source_model(&sources, en));
    let saved = state.session.preferences.saved();
    let defaults = &saved.recording;
    let gif = defaults.format == glint_core::config::VideoFormat::Gif;
    ui.set_format(i32::from(gif));
    ui.set_cursor(defaults.cursor);
    ui.set_system_audio(!gif && defaults.system_audio);
    ui.set_microphone(!gif && defaults.microphone);
    if let Some(region) = region {
        let display = glint_platform::desktop_environment()?
            .displays
            .into_iter()
            .find(|display| {
                let bounds = display.bounds;
                (bounds.left..bounds.right).contains(&region.0)
                    && (bounds.top..bounds.bottom).contains(&region.1)
            })
            .ok_or("Recording display changed")?;
        let work = display.work_area;
        let preferred_scale = owner.get_interface_scale();
        let scale = if preferred_scale > 0. {
            preferred_scale
        } else {
            display.effective_dpi.0 as f32 / 96.
        };
        crate::interface_scale::install_fixed_factor(&ui, preferred_scale);
        ui.set_compact(true);
        crate::live_toolbar::position(
            ui.window(),
            (
                work.left,
                work.top,
                (work.right - work.left) as u32,
                (work.bottom - work.top) as u32,
            ),
            region,
            slint::LogicalSize::new(ui.get_toolbar_width(), ui.get_toolbar_height()),
            scale,
        );
    } else {
        crate::interface_scale::install_factor(&ui, owner.get_interface_scale());
    }
    let configured = &saved.output.directory;
    let directory = if configured.trim().is_empty() {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or("User profile unavailable")?
            .join("Videos")
            .join("Glint")
    } else {
        PathBuf::from(configured)
    };
    ui.set_directory(directory.to_string_lossy().as_ref().into());
    ui.set_status(tr(en, "准备录屏。", "Ready to record."));
    let weak = owner.as_weak();
    ui.on_action(move |action| {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_action(format!("record:{action}").into());
        }
    });
    let weak = owner.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_action("record:close".into());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    ui.show().map_err(|e| e.to_string())?;
    state.screen_recording = Some(Run::new(ui, sources, crop, region));
    Ok(())
}
fn start(run: &mut Run, owner: &SettingsWindow, exclusions: Vec<String>) -> Result<(), String> {
    run.clear_countdown();
    if run.ui.get_format() == 1 {
        if run.ui.get_fps() > 1 {
            return Err("GIF supports only 15 or 30 FPS".into());
        }
        if run.ui.get_system_audio() || run.ui.get_microphone() {
            return Err("Turn off system audio and microphone to record GIF".into());
        }
    }
    let source = run
        .sources
        .get(run.ui.get_source().max(0) as usize)
        .ok_or("Select a recording source")?
        .clone();
    let directory = PathBuf::from(run.ui.get_directory().as_str());
    if !directory.is_absolute() {
        return Err("Choose an absolute save directory".into());
    }
    // Hide the controller from native capture, with a real failure if unavailable.
    run.ui
        .window()
        .with_winit_window(|window| -> Result<(), String> {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Not a Windows recorder".into());
            };
            platform::exclude_controller(handle.hwnd.get() as usize)
        })
        .ok_or("Recording window unavailable")??;
    let options = platform::Options {
        source,
        crop: run.crop,
        fps: [15, 30, 60, 120, 144][run.ui.get_fps().clamp(0, 4) as usize],
        format: if run.ui.get_format() == 1 {
            platform::Format::Gif
        } else {
            platform::Format::Mp4
        },
        cursor: run.ui.get_cursor(),
        system_audio: run.ui.get_system_audio(),
        microphone: run.ui.get_microphone(),
        directory: directory.clone(),
        exclusions,
    };
    let weak = owner.as_weak();
    let result = run.result.clone();
    let recorder = Recorder::start(options, move |completed| {
        *result.lock().unwrap_or_else(|e| e.into_inner()) = Some(completed);
        let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("record:ready".into()));
    })?;
    let _ = owner.hide();
    emit(Event::RecordingStarted);
    run.recorder = Some(recorder);
    run.directory = Some(directory);
    run.ui.set_active(true);
    run.ui.set_busy(false);
    run.ui.set_paused(false);
    run.ui.set_status(tr(
        run.ui.get_english(),
        "正在等待第一帧…",
        "Waiting for the first frame…",
    ));
    let weak = owner.as_weak();
    run.timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.invoke_action("record:tick".into());
            }
        },
    );
    Ok(())
}
pub fn stop_for_desktop(state: &mut AppState) {
    if let Some(run) = &mut state.screen_recording {
        run.clear_countdown();
        run.border.take();
        let _ = run.ui.hide();
        if let Some(recorder) = &run.recorder {
            recorder.stop(false);
            run.ui.set_active(false);
            run.ui.set_busy(true);
        } else {
            run.ui.set_busy(false);
        }
    }
}
pub fn handle(action: &str, shared: &Rc<RefCell<AppState>>, owner: &SettingsWindow) -> bool {
    if action == "record-open" && shared.borrow().screen_recording.is_none() {
        let capture = {
            let state = shared.borrow();
            if state.lifecycle.blocked() || state.capturing || state.scroll_capture.is_some() {
                return true;
            }
            state.capture.as_ref().map(ComponentHandle::clone_strong)
        };
        if let Some(capture) = capture {
            capture.invoke_action(
                if capture.get_capture_mode() == 2 && capture.get_selected() {
                    "record-start"
                } else {
                    "mode-record"
                }
                .into(),
            );
        } else {
            let owner = owner.as_weak();
            let shared = shared.clone();
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(owner) = owner.upgrade() {
                    owner.invoke_action("capture".into());
                    shared.borrow_mut().capture_mode = 2;
                }
            });
        }
        return true;
    }
    if action == "record:shortcut-stop" {
        let command = shared.borrow().screen_recording.as_ref().and_then(|run| {
            if run.countdown > 0 {
                Some("record:cancel")
            } else if run.ui.get_active() {
                Some("record:stop")
            } else {
                None
            }
        });
        if let Some(command) = command {
            owner.invoke_action(command.into());
        }
        return true;
    }
    let opening = action == "record-open" || action.starts_with("record-region:");
    if opening {
        let region = action
            .strip_prefix("record-region:")
            .map(|s| {
                let parts: Vec<_> = s.split(':').collect();
                if parts.len() != 4 {
                    return Err("Invalid region".to_owned());
                }
                Ok((
                    parts[0].parse().map_err(|_| "Invalid x")?,
                    parts[1].parse().map_err(|_| "Invalid y")?,
                    parts[2].parse().map_err(|_| "Invalid width")?,
                    parts[3].parse().map_err(|_| "Invalid height")?,
                ))
            })
            .transpose();
        let mut state = shared.borrow_mut();
        let selection_options = state.capture.as_ref().map(|capture| {
            (
                capture.get_record_format(),
                capture.get_record_cursor(),
                capture.get_record_system_audio(),
                capture.get_record_microphone(),
            )
        });
        let mut begin = false;
        let result = region.and_then(|region| {
            if region.is_some()
                && state.screen_recording.as_ref().is_some_and(|run| {
                    run.recorder.is_some() || run.countdown > 0 || run.ui.get_picking()
                })
            {
                return Err("A recording is already running or being configured".into());
            }
            if region.is_some() {
                state.screen_recording.take();
                state.capture_settings_visible = false;
                // The overlay's cancel re-enters the action dispatcher, which
                // borrows AppState again; defer it until this borrow ends.
                let capture = state.capture.as_ref().map(ComponentHandle::clone_strong);
                if let Some(capture) = capture {
                    let _ = capture.hide();
                    slint::Timer::single_shot(Duration::ZERO, move || {
                        capture.invoke_action("cancel".into());
                    });
                }
            } else if state.capture.is_some() {
                return Err("Finish the current screenshot first".into());
            }
            open(&mut state, owner, region)?;
            if region.is_some()
                && let Some(run) = &state.screen_recording
            {
                if let Some((format, cursor, audio, microphone)) = selection_options {
                    run.ui.set_format(format);
                    run.ui.set_cursor(cursor);
                    run.ui.set_system_audio(format == 0 && audio);
                    run.ui.set_microphone(format == 0 && microphone);
                }
                begin = true;
            }
            Ok(())
        });
        if let Err(error) = result {
            state.feedback = error;
            state.sync(owner);
            let _ = owner.show();
        }
        drop(state);
        if begin {
            let owner = owner.as_weak();
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(owner) = owner.upgrade() {
                    owner.invoke_action("record:start".into());
                }
            });
        }
        return true;
    }
    let quit = matches!(action, "quit" | "resident-exit");
    if !action.starts_with("record:") && !quit {
        return false;
    }
    if quit
        && !shared
            .borrow()
            .screen_recording
            .as_ref()
            .is_some_and(|run| run.recorder.is_some())
    {
        return false;
    }
    // Native modal picker must be entered outside the application-state borrow.
    if action == "record:directory" {
        choose_directory(shared, owner);
        return true;
    }
    let mut state = shared.borrow_mut();
    let exclusions =
        glint_core::exclusions::parse(&state.session.preferences.saved().privacy.excluded_apps);
    let blocked = state.lifecycle.blocked();
    let countdown_seconds = state
        .session
        .preferences
        .saved()
        .recording
        .countdown_seconds;
    let presentation = crate::workbench::Presentation::from_resident(&state);
    let Some(run) = &mut state.screen_recording else {
        if action == "record:poster" {
            POSTER.with(|slot| {
                slot.borrow_mut().take();
            });
        }
        return action.starts_with("record:");
    };
    let en = run.ui.get_english();
    if quit {
        run.quit = true;
    }
    let mut close = false;
    let mut quit_after = false;
    let result: Result<(), String> = (|| {
        match action {
            "record:refresh"
                if run.recorder.is_none() && run.countdown == 0 && run.crop.is_none() =>
            {
                run.sources = platform::sources()?;
                run.ui.set_sources(source_model(&run.sources, en));
                run.ui.set_source(0);
            }
            "record:start"
                if run.recorder.is_none()
                    && run.countdown == 0
                    && !run.ui.get_picking()
                    && !run.ui.get_busy() =>
            {
                if blocked {
                    return Err("Desktop unavailable".into());
                }
                if run.border.is_none()
                    && let Some(region) = run.live_region
                {
                    let accent = run.ui.get_accent();
                    let color =
                        u32::from_be_bytes([0, accent.red(), accent.green(), accent.blue()]);
                    let border = glint_platform::LiveBorder::new(region, color)?;
                    border.show();
                    run.border = Some(border);
                    if countdown_seconds > 0 {
                        run.frame = Some(glint_platform::LiveCaptureFrame::new(region)?);
                        if let Some(border) = &run.border {
                            border.show();
                        }
                        let badge = crate::motion::create_settings(RecordingCountdown::new)
                            .map_err(|error| error.to_string())?;
                        let size = ((68. * run.ui.window().scale_factor()).round() as u32)
                            .min(region.2)
                            .min(region.3);
                        badge
                            .window()
                            .set_size(slint::PhysicalSize::new(size, size));
                        badge.window().set_position(slint::PhysicalPosition::new(
                            region.0 + (region.2 - size) as i32 / 2,
                            region.1 + (region.3 - size) as i32 / 2,
                        ));
                        run.countdown_ui
                            .insert(badge)
                            .show()
                            .map_err(|err| err.to_string())?;
                        run.ui
                            .window()
                            .with_winit_window(|window| {
                                let handle = window.window_handle().map_err(|e| e.to_string())?;
                                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                                    return Err("Not a Windows recorder".into());
                                };
                                unsafe {
                                    glint_platform::NativePinWindow::from_raw(
                                        handle.hwnd.get() as *mut _
                                    )
                                }
                                .raise_topmost()
                            })
                            .ok_or("Recording window unavailable")??;
                    }
                }
                run.ui.set_busy(true);
                run.ui.set_can_open(false);
                run.ui.set_statistics("".into());
                if countdown_seconds == 0 {
                    start(
                        run,
                        owner,
                        exclusions.map_err(|_| "Invalid excluded-app policy")?,
                    )?;
                    return Ok(());
                }
                run.set_countdown(countdown_seconds);
                let weak = owner.as_weak();
                run.timer.start(
                    slint::TimerMode::Repeated,
                    Duration::from_secs(1),
                    move || {
                        if let Some(ui) = weak.upgrade() {
                            ui.invoke_action("record:countdown".into());
                        }
                    },
                );
            }
            "record:countdown" if run.countdown > 0 => {
                if run.countdown == 1 {
                    if blocked {
                        return Err("Desktop unavailable".into());
                    }
                    start(
                        run,
                        owner,
                        exclusions.map_err(|_| "Invalid excluded-app policy")?,
                    )?;
                } else {
                    run.set_countdown(run.countdown - 1);
                }
            }
            "record:pause" if run.ui.get_active() => {
                let pause = !run.ui.get_paused();
                if let Some(recorder) = &run.recorder {
                    recorder.pause(pause);
                    emit(if pause {
                        Event::RecordingPaused
                    } else {
                        Event::RecordingResumed
                    });
                    run.ui.set_paused(pause);
                }
            }
            "record:stop" | "record:cancel" | "record:close" | "quit" | "resident-exit"
                if !run.ui.get_finishing() =>
            {
                run.border.take();
                if let Some(recorder) = &run.recorder {
                    recorder.stop(action == "record:cancel");
                    run.ui.set_active(false);
                    run.ui.set_busy(true);
                    run.ui.set_finishing(true);
                    run.ui
                        .set_status(tr(en, "正在结束录制…", "Finishing recording…"));
                } else {
                    run.clear_countdown();
                    run.ui.set_busy(false);
                    if action == "record:close" || action == "record:cancel" {
                        close = true;
                        return Ok(());
                    }
                    run.ui
                        .set_status(tr(en, "已取消倒计时。", "Countdown cancelled."));
                }
            }
            "record:tick" => {
                if let Some(recorder) = &run.recorder {
                    let seconds = recorder.elapsed().as_secs();
                    let hours = seconds / 3600;
                    let minutes = seconds / 60 % 60;
                    let seconds = seconds % 60;
                    let time = if hours > 0 {
                        format!("{hours:02}:{minutes:02}:{seconds:02}")
                    } else {
                        format!("{minutes:02}:{seconds:02}")
                    };
                    run.ui.set_statistics(time.into());
                    if run.ui.get_active() {
                        run.ui.set_status(if run.ui.get_paused() {
                            tr(en, "已暂停。", "Paused.")
                        } else {
                            tr(en, "录制中。", "Recording.")
                        });
                    }
                }
            }
            "record:ready" => {
                let Some(result) = run.result.lock().unwrap_or_else(|e| e.into_inner()).take()
                else {
                    return Ok(());
                };
                run.clear_countdown();
                run.recorder.take();
                run.border.take();
                run.ui.set_finishing(false);
                run.ui.set_active(false);
                run.ui.set_busy(false);
                run.ui.set_paused(false);
                place_recording_result(&run.ui);
                emit(match &result {
                    Ok(path) if path.as_os_str().is_empty() => Event::RecordingCancelled,
                    Ok(_) => Event::RecordingCompleted,
                    Err(_) => Event::RecordingFailed,
                });
                let failed = result.is_err();
                match result {
                    Ok(path) if path.as_os_str().is_empty() => run.ui.set_status(tr(
                        en,
                        "录制已放弃，临时文件已删除。",
                        "Recording discarded; temporary file removed.",
                    )),
                    Ok(path) => {
                        run.file = Some(path.clone());
                        let preview_owner = owner.as_weak();
                        let poster_path = path.clone();
                        let poster_worker = std::thread::Builder::new()
                            .name("glint-recording-poster".into())
                            .spawn(move || {
                                let result = crate::workbench::decode(&poster_path)
                                    .and_then(crate::result_float::prepare_document);
                                let _ = preview_owner.upgrade_in_event_loop(move |owner| {
                                    // A path-specific action prevents a late poster from updating a newer run.
                                    POSTER.with(|slot| {
                                        *slot.borrow_mut() = Some((poster_path, result))
                                    });
                                    owner.invoke_action("record:poster".into());
                                });
                            });
                        run.ui.set_status(
                            format!(
                                "{}\n{}",
                                if en { "Saved:" } else { "已保存：" },
                                path.display()
                            )
                            .into(),
                        );
                        run.ui.set_can_open(true);
                        run.ui.set_can_edit(false);
                        run.editor_document = None;
                        if let Err(error) = poster_worker {
                            run.ui.set_status(
                                format!(
                                    "{}\n{error}\n{}",
                                    tr(
                                        en,
                                        "无法读取预览，视频已保留。",
                                        "Could not prepare preview; the recording is saved."
                                    ),
                                    path.display()
                                )
                                .into(),
                            );
                            if !blocked {
                                let _ = run.ui.show();
                            }
                        } else if !blocked {
                            let _ = run.ui.show();
                        }
                    }
                    Err(error) => {
                        run.ui.set_status(
                            format!(
                                "{}\n{error}",
                                if en {
                                    "Recording failed. Any partial-file location is shown below."
                                } else {
                                    "录屏失败。若有临时文件，其位置显示在下方。"
                                }
                            )
                            .into(),
                        );
                        run.ui.set_can_open(false);
                    }
                }
                if !blocked && !run.ui.get_can_open() {
                    let _ = run.ui.show();
                }
                if run.quit && !failed {
                    close = true;
                    quit_after = true;
                }
                run.quit = false;
            }
            "record:open" => {
                if let Some(directory) = &run.directory {
                    platform::open_directory(directory)?;
                }
            }
            "record:poster" => {
                if let Some((path, frame)) = POSTER.with(|slot| slot.borrow_mut().take())
                    && run.file.as_ref() == Some(&path)
                {
                    match frame {
                        Ok(result) => {
                            let thumbnail = result.thumbnail;
                            run.ui.set_picture(slint::Image::from_rgba8(
                                slint::SharedPixelBuffer::clone_from_slice(
                                    thumbnail.as_raw(),
                                    thumbnail.width(),
                                    thumbnail.height(),
                                ),
                            ));
                            run.editor_document = Some(result.document);
                            run.ui.set_can_edit(true);
                            if !blocked {
                                let _ = run.ui.show();
                            }
                        }
                        Err(error) => {
                            run.ui.set_status(format!("{}\n{error}", tr(en, "预览读取失败，视频文件已保留，可使用系统播放器打开。", "Preview failed; the saved file can still be opened in the system player.")).into());
                            if !blocked {
                                let _ = run.ui.show();
                            }
                        }
                    }
                }
            }
            "record:play" => {
                if let Some(path) = &run.file {
                    platform::play_file(path)?;
                }
            }
            "record:edit" => {
                if !blocked && let Some(document) = &run.editor_document {
                    crate::workbench::open_handoff(document.clone(), presentation.clone())?;
                    close = true;
                }
            }
            "record:copy" => {
                if let Some(path) = &run.file {
                    platform::copy_file(path)?;
                    run.ui
                        .set_status(tr(en, "已复制视频文件。", "Video file copied."));
                }
            }
            _ => {}
        }
        Ok(())
    })();
    if let Err(error) = result
        && let Some(run) = &mut state.screen_recording
    {
        run.ui.set_status(error.into());
        if run.recorder.is_none() {
            run.ui.set_busy(false);
            run.clear_countdown();
            run.border.take();
            run.ui.set_compact(false);
            let scale = run.ui.window().scale_factor();
            run.ui.window().set_size(slint::PhysicalSize::new(
                (680. * scale) as u32,
                (500. * scale) as u32,
            ));
        }
    }
    if close {
        state.screen_recording.take();
    }
    if quit_after {
        let weak = owner.as_weak();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(ui) = weak.upgrade() {
                ui.invoke_action("quit".into());
            }
        });
    }
    true
}
fn choose_directory(shared: &Rc<RefCell<AppState>>, owner: &SettingsWindow) {
    let Some(run) = shared
        .borrow()
        .screen_recording
        .as_ref()
        .map(|r| r.ui.clone_strong())
    else {
        return;
    };
    if run.get_active() || run.get_busy() || run.get_picking() {
        return;
    }
    run.set_picking(true);
    let shared = shared.clone();
    let weak = owner.as_weak();
    slint::Timer::single_shot(Duration::ZERO, move || {
        let Some(_owner) = weak.upgrade() else {
            return;
        };
        let result = run
            .window()
            .with_winit_window(|window| -> Result<Option<PathBuf>, String> {
                let handle = window.window_handle().map_err(|e| e.to_string())?;
                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                    return Err("Not a Windows recorder".into());
                };
                unsafe {
                    glint_platform::choose_output_directory(
                        handle.hwnd.get() as *mut _,
                        std::path::Path::new(run.get_directory().as_str()),
                        if run.get_english() {
                            "Recording directory"
                        } else {
                            "录屏保存目录"
                        },
                        |control| {
                            if let Some(run) = &mut shared.borrow_mut().screen_recording {
                                run.picker = Some(control);
                            }
                        },
                    )
                }
            })
            .unwrap_or_else(|| Err("Recording window unavailable".into()));
        if let Some(current) = &mut shared.borrow_mut().screen_recording {
            current.picker.take();
        }
        run.set_picking(false);
        match result {
            Ok(Some(path)) => run.set_directory(path.to_string_lossy().as_ref().into()),
            Err(error) => run.set_status(error.into()),
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::winit_030::winit::{
        event_loop::EventLoop, platform::windows::EventLoopBuilderExtWindows,
    };

    #[test]
    fn regional_controls_stay_anchored_and_countdown_cleans_up() {
        let mut event_loop = EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
        event_loop.with_any_thread(true);
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("software".into())
            .with_winit_event_loop_builder(event_loop)
            .select()
            .unwrap();

        let badge = RecordingCountdown::new().unwrap();
        badge.show().unwrap();
        let controls = RecordingWindow::new().unwrap();
        controls.set_compact(true);
        let work = (0, 0, 800, 600);
        let region = (360, 340, 360, 250);
        let size =
            slint::LogicalSize::new(controls.get_toolbar_width(), controls.get_toolbar_height());
        crate::live_toolbar::position(controls.window(), work, region, size, 1.);
        controls.show().unwrap();
        slint::spawn_local(async move {
            let window = badge.window().winit_window().await.unwrap();
            assert_eq!(window.is_visible(), Some(true));

            let toolbar = controls.window().winit_window().await.unwrap();
            let expected = (390, 288, 300, 44);
            let geometry = || {
                let position = toolbar.outer_position().unwrap();
                let size = toolbar.inner_size();
                (position.x, position.y, size.width, size.height)
            };
            assert_eq!(geometry(), expected);
            let mut run = Run::new(controls.clone_strong(), Vec::new(), None, Some(region));
            run.countdown_ui = Some(badge.clone_strong());
            run.set_countdown(3);
            let first = controls.window().take_snapshot().unwrap();
            run.set_countdown(2);
            assert_eq!(badge.get_seconds(), 2);
            assert_eq!(controls.get_status(), "即将录制 · 2 秒");
            let second = controls.window().take_snapshot().unwrap();
            let row_bytes = first.width() as usize * 4;
            assert!(
                first
                    .as_bytes()
                    .chunks_exact(row_bytes)
                    .zip(second.as_bytes().chunks_exact(row_bytes))
                    .any(|(before, after)| before[..240 * 4] != after[..240 * 4]),
                "the visible countdown text on the left must update"
            );
            run.timer
                .start(slint::TimerMode::Repeated, Duration::from_secs(1), || {});
            run.clear_countdown();
            controls.set_active(true);
            controls.set_statistics("01:00:00".into());
            controls.set_paused(true);
            controls.window().take_snapshot().unwrap();
            assert_eq!(geometry(), expected);
            assert_eq!(run.countdown, 0);
            assert!(!run.timer.running());
            assert!(run.countdown_ui.is_none());
            assert!(!badge.window().is_visible());
            assert_eq!(window.is_visible(), Some(false));

            let work = glint_platform::desktop_environment()
                .unwrap()
                .displays
                .into_iter()
                .find(|display| {
                    (display.bounds.left..display.bounds.right).contains(&expected.0)
                        && (display.bounds.top..display.bounds.bottom).contains(&expected.1)
                })
                .unwrap()
                .work_area;
            controls.set_active(false);
            controls.set_can_open(true);
            place_recording_result(&controls);
            let result_position = toolbar.outer_position().unwrap();
            let result_size = toolbar.outer_size();
            assert!(result_position.x >= work.left && result_position.y >= work.top);
            assert!(result_position.x + result_size.width as i32 <= work.right);
            assert!(result_position.y + result_size.height as i32 <= work.bottom);
            assert!(
                (2 * result_position.x + result_size.width as i32 - work.left - work.right).abs()
                    <= 1
            );
            assert!(
                (2 * result_position.y + result_size.height as i32 - work.top - work.bottom).abs()
                    <= 1
            );
            slint::quit_event_loop().unwrap();
        })
        .unwrap();
        slint::run_event_loop_until_quit().unwrap();
    }
}
