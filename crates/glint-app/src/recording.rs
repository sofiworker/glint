//! Recording presentation and lifecycle. Pixels and encoding stay in glint-platform.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_platform::{
    diagnostic_log::{Event, emit},
    recording::{self as platform, Recorder, Source},
};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

glint_ui::slint! {
    export { Motion } from "@glint_ui/widgets/motion.slint";
    export { RecordingWindow } from "src/recording.slint";
}
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
}
impl Drop for Run {
    fn drop(&mut self) {
        if let Some(control) = self.picker.take() {
            let _ = control.cancel();
        }
        let _ = self.ui.hide();
    }
}
fn tr(en: bool, zh: &str, english: &str) -> slint::SharedString {
    if en { english } else { zh }.into()
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
    crate::interface_scale::install_factor(&ui, owner.get_interface_scale());
    ui.set_dark(owner.get_dark());
    ui.set_accent(owner.get_accent());
    ui.set_region(region.is_some());
    ui.set_sources(source_model(&sources, en));
    let configured = &state.session.preferences.saved().output.directory;
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
    ui.set_status(tr(
        en,
        "准备录屏。系统声音与麦克风默认关闭。",
        "Ready. System audio and microphone are off by default.",
    ));
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
    state.screen_recording = Some(Run {
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
    });
    Ok(())
}
fn start(run: &mut Run, owner: &SettingsWindow, exclusions: Vec<String>) -> Result<(), String> {
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
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
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
        run.countdown = 0;
        if let Some(recorder) = &run.recorder {
            recorder.stop(false);
            run.ui.set_active(false);
            run.ui.set_busy(true);
        } else {
            run.timer.stop();
            run.ui.set_busy(false);
        }
    }
}
pub fn handle(action: &str, shared: &Rc<RefCell<AppState>>, owner: &SettingsWindow) -> bool {
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
                if let Some(capture) = &state.capture {
                    let _ = capture.hide();
                    capture.invoke_action("cancel".into());
                }
            } else if state.capture.is_some() {
                return Err("Finish the current screenshot first".into());
            }
            open(&mut state, owner, region)
        });
        if let Err(error) = result {
            state.feedback = error;
            state.sync(owner);
            let _ = owner.show();
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
    let Some(run) = &mut state.screen_recording else {
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
                run.countdown = 3;
                run.ui.set_busy(true);
                run.ui.set_can_open(false);
                run.ui
                    .set_status(tr(en, "3 秒后开始录屏…", "Recording starts in 3 seconds…"));
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
                run.countdown -= 1;
                if run.countdown == 0 {
                    run.timer.stop();
                    if blocked {
                        run.ui.set_busy(false);
                        return Err("Desktop unavailable".into());
                    }
                    start(
                        run,
                        owner,
                        exclusions.map_err(|_| "Invalid excluded-app policy")?,
                    )?;
                } else {
                    run.ui.set_status(
                        format!(
                            "{} {}",
                            run.countdown,
                            if en {
                                "seconds…"
                            } else {
                                "秒后开始录屏…"
                            }
                        )
                        .into(),
                    );
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
            "record:stop" | "record:cancel" | "record:close" | "quit" | "resident-exit" => {
                run.countdown = 0;
                if let Some(recorder) = &run.recorder {
                    recorder.stop(action == "record:cancel");
                    run.ui.set_active(false);
                    run.ui.set_busy(true);
                    run.ui
                        .set_status(tr(en, "正在结束录制…", "Finishing recording…"));
                } else {
                    run.timer.stop();
                    run.ui.set_busy(false);
                    if action == "record:close" {
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
                    let stats = recorder.stats();
                    run.ui.set_statistics(
                        format!(
                            "{:02}:{:02}:{:02} · {} {} · {} {} · {} {}",
                            seconds / 3600,
                            seconds / 60 % 60,
                            seconds % 60,
                            stats.frames.load(Ordering::Relaxed),
                            if en { "frames" } else { "帧" },
                            stats.dropped.load(Ordering::Relaxed),
                            if en { "video dropped" } else { "视频丢帧" },
                            stats.audio_dropped.load(Ordering::Relaxed),
                            if en {
                                "audio discontinuities/drops"
                            } else {
                                "音频不连续/丢包"
                            }
                        )
                        .into(),
                    );
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
                run.timer.stop();
                run.recorder.take();
                run.ui.set_active(false);
                run.ui.set_busy(false);
                run.ui.set_paused(false);
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
                        run.ui.set_status(
                            format!(
                                "{}\n{}",
                                if en { "Saved:" } else { "已保存：" },
                                path.display()
                            )
                            .into(),
                        );
                        run.ui.set_can_open(true);
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
                        run.ui.set_can_open(run.directory.is_some());
                    }
                }
                let _ = run.ui.show();
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
            _ => {}
        }
        Ok(())
    })();
    if let Err(error) = result
        && let Some(run) = &state.screen_recording
    {
        run.ui.set_status(error.into());
        if run.recorder.is_none() {
            run.ui.set_busy(false);
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
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
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
