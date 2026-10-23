//! Event-driven scrolling capture. One bounded worker owns the stitch
//! session while busy; late completions cannot revive a closed capture.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use glint_core::{
    Edits, Rect,
    scroll_capture::{Append, Fragment, ScrollSequence as ScrollSession, StitchError},
};
use slint::winit_030::WinitWindowAccessor;
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
    export { ScrollCaptureWindow, ScrollControlWindow, ScrollLabels } from "src/scroll_capture.slint";
    export component ScrollSequenceFixture inherits Window {
        no-frame: true; always-on-top: true;
        width: 192px; height: 256px;
        in property <bool> changed;
        in property <bool> document;
        in property <image> source;
        in-out property <int> offset;
        callback scrolled(int);
        changed offset => { root.scrolled(root.offset); }
        background: root.changed ? #b63251 : #246d92;
        if !root.document: Rectangle {
            for n in 5: Rectangle { x: 12px; y: 12px + n * 45px; width: 168px; height: 20px; background: root.changed ? #e79d4f : #92d3e5; }
            Text { text: root.changed ? "Source frame B" : "Source frame A"; color: white; }
        }
        if root.document: Rectangle {
            clip: true;
            Image { y: -root.offset * (parent.width / 1px) / 192 * 1px; width: parent.width; height: 768 * (parent.width / 1px) / 192 * 1px; source: root.source; image-fit: fill; }
            TouchArea { scroll-event(event) => {
                if event.delta-y < 0px { root.offset = min(498,root.offset+83); }
                if event.delta-y > 0px { root.offset = max(0,root.offset - 83); }
                return accept;
            } }
        }
    }
}

#[cfg(feature = "input-probe")]
#[path = "scroll_input_smoke.rs"]
mod input_smoke;

thread_local! {
    static SMOKE_FIXTURE: RefCell<Option<ScrollSequenceFixture>> = const { RefCell::new(None) };
    static SMOKE_REFERENCE: RefCell<Option<Fragment>> = const { RefCell::new(None) };
    static SMOKE_SOURCE: RefCell<Option<SourceFixtureProcess>> = const { RefCell::new(None) };
    static SMOKE_DIRECTORY: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
    static SMOKE_VISIBLE_WAIT: std::cell::RefCell<Option<std::time::Instant>> = const { std::cell::RefCell::new(None) };
    static SMOKE_FAILURE_ARMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SMOKE_FAIL_START: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SMOKE_RETRY_STAGE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

struct SourceFixtureProcess(std::process::Child, std::path::PathBuf);
impl Drop for SourceFixtureProcess {
    fn drop(&mut self) {
        let _ = std::fs::write(self.1.join("source-stop.txt"), "stop");
        for _ in 0..20 {
            if self.0.try_wait().is_ok_and(|status| status.is_some()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Isolated source process for the explicit native scrolling probe. It bypasses
/// resident/single-instance setup and never enables self-capture in production.
pub fn source_fixture(directory: &str) -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(directory);
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let fixture = ScrollSequenceFixture::new()?;
    let reference = smoke_document();
    fixture.set_document(true);
    fixture.set_source(slint::Image::from_rgba8(
        SharedPixelBuffer::clone_from_slice(&reference.rgba, 192, 768),
    ));
    let display = glint_platform::desktop_environment()?
        .displays
        .into_iter()
        .next()
        .ok_or("No display")?;
    fixture.window().set_position(slint::PhysicalPosition::new(
        display.work_area.left + 100,
        display.work_area.top + 100,
    ));
    fixture
        .window()
        .set_size(slint::PhysicalSize::new(192, 256));
    let path = directory.join("source-offset.txt");
    fixture.on_scrolled(move |offset| {
        let _ = std::fs::write(&path, offset.to_string());
    });
    fixture.show()?;
    fixture
        .window()
        .with_winit_window(|window| window.focus_window());
    let start = std::time::Instant::now();
    let weak_fixture = fixture.as_weak();
    let mut ready = false;
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(200),
        move || {
            if !ready && let Some(fixture) = weak_fixture.upgrade() {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let focused = fixture
                    .window()
                    .with_winit_window(|window| {
                        let focused = window.window_handle().ok().is_some_and(|handle| {
                            matches!(handle.as_raw(), RawWindowHandle::Win32(handle)
                            if handle.hwnd.get() as usize == glint_platform::foreground_window_id())
                        });
                        if !focused {
                            window.focus_window();
                        }
                        focused
                    })
                    .unwrap_or(false);
                if focused {
                    ready = std::fs::write(directory.join("source-ready.txt"), "0").is_ok();
                }
            }
            if directory.join("source-stop.txt").exists()
                || start.elapsed() > Duration::from_secs(40)
            {
                let _ = slint::quit_event_loop();
            }
        },
    );
    slint::run_event_loop()?;
    Ok(())
}

fn smoke_document() -> Fragment {
    const DIGITS: [[u8; 7]; 10] = [
        [14, 17, 19, 21, 25, 17, 14],
        [4, 12, 4, 4, 4, 4, 14],
        [14, 17, 1, 2, 4, 8, 31],
        [30, 1, 1, 14, 1, 1, 30],
        [2, 6, 10, 18, 31, 2, 2],
        [31, 16, 16, 30, 1, 1, 30],
        [14, 16, 16, 30, 17, 17, 14],
        [31, 1, 2, 4, 8, 8, 8],
        [14, 17, 17, 14, 17, 17, 14],
        [14, 17, 17, 15, 1, 1, 14],
    ];
    let mut rgba = vec![255; 192 * 768 * 4];
    for y in 0..768usize {
        if !(2..16).contains(&(y % 20)) {
            continue;
        }
        let text = format!("{:04}{:06}", y / 20, (y / 20) * 37);
        for (i, digit) in text.bytes().enumerate() {
            for x in 0..10 {
                if DIGITS[(digit - b'0') as usize][(y % 20 - 2) / 2] & (1 << (4 - x / 2)) != 0 {
                    let at = (y * 192 + 12 + i * 16 + x) * 4;
                    rgba[at..at + 3].copy_from_slice(&[24, 35, 52]);
                }
            }
        }
    }
    Fragment::new(192, 768, rgba).unwrap()
}

/// External source rendezvous is confined to the explicitly invoked diagnostic.
fn smoke_external_spec(directory: &std::path::Path) -> Result<Option<[i32; 6]>, String> {
    if !directory.join("external-source.txt").exists() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(directory.join("source-spec.txt")).map_err(|e| e.to_string())?;
    let values = text
        .split_whitespace()
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let values: [i32; 6] = values
        .try_into()
        .map_err(|_| "Expected six source dimensions")?;
    if values[2] < 32 || values[3] < 64 || values[4] <= 0 || values[5] != values[3] + values[4] {
        return Err("Invalid external source dimensions".into());
    }
    Ok(Some(values))
}

fn smoke_select(shared: &Rc<RefCell<AppState>>, region: Option<[i32; 6]>) -> Result<bool, String> {
    let Some(capture) = shared
        .borrow()
        .capture
        .as_ref()
        .map(ComponentHandle::clone_strong)
    else {
        return Ok(false);
    };
    let environment = glint_platform::desktop_environment()?;
    let display = environment.displays.first().ok_or("No display")?;
    let origin_x = environment
        .displays
        .iter()
        .map(|d| d.bounds.left)
        .min()
        .unwrap();
    let origin_y = environment
        .displays
        .iter()
        .map(|d| d.bounds.top)
        .min()
        .unwrap();
    let [x, y, width, height, _, _] = region.unwrap_or([
        display.work_area.left + 100,
        display.work_area.top + 100,
        192,
        256,
        0,
        0,
    ]);
    let x = (x - origin_x) as f32;
    let y = (y - origin_y) as f32;
    capture.set_tool(0);
    if capture.get_capture_mode() != 1 {
        return Err("Real scrolling entry did not select scrolling mode".into());
    }
    #[cfg(feature = "input-probe")]
    if std::env::var("GLINT_SCROLL_PROBE_INPUT").as_deref() == Ok("1") {
        return input_smoke::select(&capture, x, y, width, height);
    }
    capture.invoke_pointer(0, x, y);
    capture.invoke_pointer(2, x + width as f32, y + height as f32);
    capture.invoke_action("scroll-start".into());
    Ok(true)
}

/// Explicit isolated resident probe: real GDI frames and production lifecycle.
pub fn smoke_stage(
    stage: u16,
    settings: &SettingsWindow,
    shared: &Rc<RefCell<AppState>>,
    directory: &std::path::Path,
) -> Result<bool, String> {
    match stage {
        0 => {
            SMOKE_DIRECTORY.with(|slot| {
                *slot.borrow_mut() = (std::env::var("GLINT_SCROLL_PROBE_FRAMES").as_deref()
                    == Ok("1"))
                .then(|| directory.to_owned())
            });
            settings.set_guide_step(-1);
            let fixture = ScrollSequenceFixture::new().map_err(|e| e.to_string())?;
            let display = glint_platform::desktop_environment()?
                .displays
                .into_iter()
                .next()
                .ok_or("No display")?;
            fixture.window().set_position(slint::PhysicalPosition::new(
                display.work_area.left + 100,
                display.work_area.top + 100,
            ));
            fixture
                .window()
                .set_size(slint::PhysicalSize::new(192, 256));
            fixture.show().map_err(|e| e.to_string())?;
            SMOKE_FIXTURE.with(|slot| *slot.borrow_mut() = Some(fixture));
            settings.invoke_action("scroll-open".into());
        }
        1 => match SMOKE_RETRY_STAGE.with(|stage| stage.get()) {
            0 => {
                if !SMOKE_FAILURE_ARMED.with(|armed| armed.replace(true)) {
                    SMOKE_FAIL_START.with(|fail| fail.set(true));
                }
                if smoke_select(shared, None)? {
                    SMOKE_RETRY_STAGE.with(|stage| stage.set(1));
                }
                return Ok(false);
            }
            1 => {
                let state = shared.borrow();
                let capture = state
                    .capture
                    .as_ref()
                    .ok_or("Selection lost during startup failure")?;
                if !capture
                    .get_service_notice()
                    .contains("Controlled scrolling startup failure")
                {
                    return Ok(false);
                }
                if !capture.window().is_visible()
                    || !capture.get_selected()
                    || state.scroll_capture.is_some()
                {
                    return Err("Failed startup did not restore the frozen selection".into());
                }
                std::fs::write(directory.join("SCROLL-STARTUP-RECOVERY-PASSED.txt"), "Failure injected after hiding the frozen selection. Same selection becomes visible again, actual error is visible, retry starts without returning to settings.\n").map_err(|e| e.to_string())?;
                SMOKE_RETRY_STAGE.with(|stage| stage.set(2));
                capture.invoke_action("scroll-start".into());
                return Ok(true);
            }
            _ => return smoke_select(shared, None),
        },
        2 => {
            let state = shared.borrow();
            let Some(run) = &state.scroll_capture else {
                if let Some(capture) = &state.capture {
                    let notice = capture.get_service_notice();
                    if !notice.is_empty()
                        && !notice.contains("Controlled scrolling startup failure")
                    {
                        return Err(format!("Retry did not start live capture: {notice}"));
                    }
                }
                return Ok(false);
            };
            std::fs::write(directory.join("live-start-state.txt"), format!("worker_pending={} affinity={} session_frames={:?} preview_visible={} controls_visible={} boundary_visible={} busy={} status={}\n",
                run.result.is_some(),run.capture_affinity,run.session.as_ref().map(|s|s.count()),run.ui.window().is_visible(),run.controls.window().is_visible(),run._frame.visible(),run.ui.get_busy(),run.ui.get_status())).map_err(|e|e.to_string())?;
            if run.result.is_some() || !run.capture_affinity {
                return Ok(false);
            }
            if run.session.as_ref().map_or(0, |s| s.count()) != 1 {
                return Err("First real source frame missing".into());
            }
            if !smoke_verify_live(run, directory, "native-live-initial.png")? {
                return Ok(false);
            }
            let action = format!("scroll:{}:wheel:down", run.ticket);
            drop(state);
            SMOKE_FIXTURE.with(|slot| {
                if let Some(fixture) = slot.borrow().as_ref() {
                    fixture.set_changed(true);
                }
            });
            settings.invoke_action(action.into());
        }
        3 => {
            let state = shared.borrow();
            let Some(run) = &state.scroll_capture else {
                return Err("Scrolling session disappeared during acquisition".into());
            };
            if run.session.as_ref().map_or(0, |s| s.count()) < 2 {
                return Ok(false);
            }
            if run.ui.get_review() || run.controls.get_stopped() {
                return Err("Unmatched frame interrupted capture".into());
            }
            if !run.ui.get_pending() {
                return Err("Fixture did not exercise an unresolved join".into());
            }
            save_smoke_snapshot(&run.ui, &directory.join("scroll-acquiring.png"))?;
            let action = format!("scroll:{}:stop", run.ticket);
            drop(state);
            settings.invoke_action(action.into());
        }
        4 => {
            let state = shared.borrow();
            let Some(run) = &state.scroll_capture else {
                return Err("Scrolling session vanished before review".into());
            };
            if run.result.is_some() || !run.ui.get_review() {
                return Ok(false);
            }
            if run.sampling.running() || run.automatic_timer.running() {
                return Err("Acquisition timer survived finish".into());
            }
            save_smoke_snapshot(&run.ui, &directory.join("scroll-finished-review.png"))?;
            #[cfg(feature = "input-probe")]
            if std::env::var("GLINT_SCROLL_PROBE_INPUT").as_deref() == Ok("1") {
                input_smoke::click(
                    &run.ui,
                    0.7,
                    1. - 25. * run.ui.window().scale_factor()
                        / run.ui.window().size().height as f32,
                )?;
                return Ok(true);
            }
            let action = format!("scroll:{}:cancel", run.ticket);
            drop(state);
            settings.invoke_action(action.into());
        }
        5 => {
            let state = shared.borrow();
            if state.capture.is_some() || state.scroll_capture.is_some() {
                return Ok(false);
            }
            if settings.window().is_visible() {
                return Err("Cancel reopened settings instead of leaving capture".into());
            }
            SMOKE_FIXTURE.with(|slot| {
                if let Some(fixture) = slot.borrow_mut().take() {
                    let _ = fixture.hide();
                }
            });
            std::fs::write(directory.join("SCROLL-SEQUENCE-PASSED.txt"), "Real native GDI frame acquisition; unresolved joins do not show review while capturing; finish waits for work then exposes batch review; one cancel drops scrolling and frozen capture without reopening settings. No physical wheel or third-party-page compatibility claim.\n").map_err(|e|e.to_string())?;
        }
        6 => {
            if directory.join("external-source.txt").exists() {
                let fixture = ScrollSequenceFixture::new().map_err(|e| e.to_string())?;
                fixture.show().map_err(|e| e.to_string())?;
                fixture
                    .window()
                    .with_winit_window(|window| window.focus_window());
                SMOKE_FIXTURE.with(|slot| *slot.borrow_mut() = Some(fixture));
                std::fs::write(directory.join("source-start.txt"), "start")
                    .map_err(|e| e.to_string())?;
                return Ok(true);
            }
            use std::os::windows::process::CommandExt;
            let child =
                std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                    .arg("--m0-scroll-source-fixture")
                    .arg(directory)
                    .creation_flags(0x08000000)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            SMOKE_SOURCE.with(|slot| {
                *slot.borrow_mut() = Some(SourceFixtureProcess(child, directory.to_owned()))
            });
            let mut reference = smoke_document();
            reference.height = 754;
            reference.rgba.truncate(192 * 754 * 4);
            SMOKE_REFERENCE.with(|slot| *slot.borrow_mut() = Some(reference));
        }
        7 => {
            if !directory.join("source-ready.txt").exists() {
                if directory.join("external-source.txt").exists() {
                    if directory.join("source-page-ready.txt").exists() {
                        SMOKE_FIXTURE.with(|slot| {
                            if let Some(fixture) = slot.borrow().as_ref() {
                                let _ = fixture.hide();
                            }
                        });
                        std::fs::write(directory.join("source-fixture-hidden.txt"), "hidden")
                            .map_err(|e| e.to_string())?;
                    } else if SMOKE_SOURCE.with(|slot| slot.borrow().is_none()) {
                        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                        let focused = SMOKE_FIXTURE.with(|slot| slot.borrow().as_ref().is_some_and(|fixture| {
                            fixture.window().with_winit_window(|window| {
                                let focused = window.window_handle().ok().is_some_and(|handle|
                                    matches!(handle.as_raw(), RawWindowHandle::Win32(handle) if handle.hwnd.get() as usize == glint_platform::foreground_window_id()));
                                if !focused { window.focus_window(); }
                                focused
                            }).unwrap_or(false)
                        }));
                        if focused {
                            use std::os::windows::process::CommandExt;
                            let mode =
                                std::fs::read_to_string(directory.join("external-source.txt"))
                                    .map_err(|e| e.to_string())?;
                            let child = std::process::Command::new("node")
                                .arg(".local/qa/scroll-browser-native.cjs")
                                .arg(directory)
                                .arg(mode.trim())
                                .creation_flags(0x08000000)
                                .spawn()
                                .map_err(|e| e.to_string())?;
                            SMOKE_SOURCE.with(|slot| {
                                *slot.borrow_mut() =
                                    Some(SourceFixtureProcess(child, directory.to_owned()))
                            });
                        }
                    }
                }
                return Ok(false);
            }
            if let Some([_, _, width, _, _, height]) = smoke_external_spec(directory)? {
                let pixels = std::fs::read(directory.join("source-reference.rgba"))
                    .map_err(|e| e.to_string())?;
                let reference = Fragment::new(width as u32, height as u32, pixels)
                    .map_err(|e| format!("{e:?}"))?;
                SMOKE_REFERENCE.with(|slot| *slot.borrow_mut() = Some(reference));
            }
            settings.invoke_action("scroll-open".into());
        }
        8 => {
            return smoke_select(shared, smoke_external_spec(directory)?);
        }
        9 => {
            let state = shared.borrow();
            let Some(run) = &state.scroll_capture else {
                return Ok(false);
            };
            if run.result.is_some() || !run.capture_affinity {
                return Ok(false);
            }
            if run.session.as_ref().map_or(0, |s| s.count()) != 1 {
                return Err("Document baseline missing".into());
            }
            if !smoke_verify_live(run, directory, "native-live-document.png")? {
                return Ok(false);
            }
            #[cfg(feature = "input-probe")]
            if std::env::var("GLINT_SCROLL_PROBE_INPUT").as_deref() == Ok("1") {
                input_smoke::click(&run.controls, 0.25, 0.5)?;
                return Ok(true);
            }
            let action = format!("scroll:{}:auto", run.ticket);
            drop(state);
            settings.invoke_action(action.into());
        }
        10 => {
            let state = shared.borrow();
            let Some(run) = &state.scroll_capture else {
                return Err("Automatic session vanished".into());
            };
            if run.result.is_some() || run.controls.get_automatic() {
                return Ok(false);
            }
            let offset = std::fs::read_to_string(directory.join("source-offset.txt"))
                .ok()
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            let expected_offset = smoke_external_spec(directory)?.map_or(498, |v| v[4]);
            if offset != expected_offset {
                return Err(format!(
                    "Native wheel stopped at {offset}: {}",
                    run.ui.get_status()
                ));
            }
            if !smoke_verify_live(run, directory, "native-live-stitched.png")? {
                return Ok(false);
            }
            let session = run.session.as_ref().ok_or("No settled document")?;
            save_smoke_snapshot(&run.ui, &directory.join("automatic-document-preview.png"))?;
            let result = session.compose(None).map_err(|e| format!("{e:?}"))?;
            image::save_buffer(
                directory.join("automatic-document-composed.png"),
                &result.rgba,
                result.width,
                result.height,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
            if session.unresolved_count() != 0 || run.ui.get_repair_mode() {
                return Err("Automatic document requested seam repair".into());
            }
            let exact = SMOKE_REFERENCE.with(|slot| {
                let slot = slot.borrow();
                let expected = slot.as_ref().unwrap();
                result.width == expected.width
                    && result.height == expected.height
                    && result.rgba == expected.rgba
            });
            if !exact {
                return Err(format!(
                    "Native joined pixels differ: {} x {}",
                    result.width, result.height
                ));
            }
            save_smoke_snapshot(&run.ui, &directory.join("automatic-document-preview.png"))?;
            #[cfg(feature = "input-probe")]
            if std::env::var("GLINT_SCROLL_PROBE_INPUT").as_deref() == Ok("1") {
                input_smoke::click(
                    &run.ui,
                    0.9,
                    1. - 25. * run.ui.window().scale_factor()
                        / run.ui.window().size().height as f32,
                )?;
                return Ok(true);
            }
            let action = format!("scroll:{}:copy", run.ticket);
            drop(state);
            settings.invoke_action(action.into());
        }
        11 => {
            let state = shared.borrow();
            if state.scroll_capture.is_some() || state.capture.is_some() || state.capturing {
                return Ok(false);
            }
            drop(state);
            let copied = glint_platform::read_clipboard_image()?;
            let exact = SMOKE_REFERENCE.with(|slot| {
                let slot = slot.borrow();
                let expected = slot.as_ref().unwrap();
                copied.width == expected.width
                    && copied.height == expected.height
                    && copied.rgba == expected.rgba
            });
            if !exact {
                return Err(format!(
                    "Clipboard pixels differ: {} x {}",
                    copied.width, copied.height
                ));
            }
            if directory.join("external-source.txt").exists() {
                std::fs::write(directory.join("source-stop.txt"), "stop")
                    .map_err(|e| e.to_string())?;
                let exit = SMOKE_SOURCE
                    .with(|slot| slot.borrow_mut().as_mut().map(|child| child.0.try_wait()))
                    .ok_or("Browser helper missing")?
                    .map_err(|e| e.to_string())?;
                let Some(exit) = exit else {
                    return Ok(false);
                };
                if !exit.success() {
                    return Err(format!("Browser helper exited: {exit}"));
                }
            }
            image::save_buffer(
                directory.join("automatic-document-clipboard.png"),
                &copied.rgba,
                copied.width,
                copied.height,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
            SMOKE_FIXTURE.with(|slot| {
                if let Some(fixture) = slot.borrow_mut().take() {
                    let _ = fixture.hide();
                }
            });
            SMOKE_REFERENCE.with(|slot| slot.borrow_mut().take());
            let _ = std::fs::write(directory.join("source-stop.txt"), "stop");
            SMOKE_SOURCE.with(|slot| slot.borrow_mut().take());
            let report = format!(
                "Native GDI capture and production auto state machine; target receives WM_MOUSEWHEEL; one Copy action without seam confirmation; actual clipboard {}x{} matches every reference RGBA byte. Source: {}. Not physical-button, physical-wheel or mixed-DPI acceptance.\n",
                copied.width,
                copied.height,
                if directory.join("external-source.txt").exists() {
                    "external browser fixture"
                } else {
                    "native numeric fixture"
                }
            );
            #[cfg(feature = "input-probe")]
            if std::env::var("GLINT_SCROLL_PROBE_INPUT").as_deref() == Ok("1") {
                std::fs::write(directory.join("SCROLL-REAL-INPUT-PASSED.txt"), "Guarded OS input: click Scrolling mode; mouse Down/Move/Up selects exact source pixels; click Start; click Cancel; reopen; select browser; click Start; click Auto; click Copy once; actual clipboard matches all reference pixels. Initial entry and injected-failure retry still use callbacks. No general third-party compatibility or mixed-DPI acceptance.\n").map_err(|e| e.to_string())?;
            }
            std::fs::write(directory.join("SCROLL-AUTO-COPY-PASSED.txt"), report)
                .map_err(|e| e.to_string())?;
        }
        _ => return Err("Unknown scrolling probe stage".into()),
    }
    Ok(true)
}

fn save_smoke_snapshot(ui: &ScrollCaptureWindow, path: &std::path::Path) -> Result<(), String> {
    let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    image::save_buffer(
        path,
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}

/// Observe the actual native windows and desktop composition, not just an
/// offscreen Slint snapshot. Acquisition is idle at both diagnostic call sites.
fn smoke_verify_live(
    run: &Run,
    directory: &std::path::Path,
    filename: &str,
) -> Result<bool, String> {
    fn native(window: &slint::Window) -> Result<glint_platform::NativePinWindow, String> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        window
            .with_winit_window(|window| {
                let handle = window.window_handle().map_err(|e| e.to_string())?;
                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                    return Err("Not a Win32 live window".into());
                };
                Ok(unsafe {
                    glint_platform::NativePinWindow::from_raw(handle.hwnd.get() as *mut _)
                })
            })
            .ok_or("Live window missing")?
    }
    let preview = native(run.ui.window())?;
    let controls = native(run.controls.window())?;
    let panel = preview.state()?;
    let toolbar = controls.state()?;
    if !run._frame.visible()
        || !panel.visible
        || !toolbar.visible
        || !panel.topmost
        || !toolbar.topmost
    {
        return Err(format!(
            "Live selection/preview/controls not visibly topmost: {panel:?}, {toolbar:?}"
        ));
    }
    if !run.ui.get_has_picture() || panel.x < run.region.0 + run.region.2 as i32 {
        return Err("Right-hand native preview missing or not beside test selection".into());
    }
    if !preview.receives_pointer_at(panel.x + panel.width / 2, panel.y + panel.height / 2)? {
        return Err("Native preview is covered by another window".into());
    }
    let x = (run.region.0 - 6)
        .min(panel.x)
        .min(toolbar.x)
        .max(run.work.0);
    let y = (run.region.1 - 6)
        .min(panel.y)
        .min(toolbar.y)
        .max(run.work.1);
    let right = (run.region.0 + run.region.2 as i32 + 6)
        .max(panel.x + panel.width)
        .max(toolbar.x + toolbar.width)
        .min(run.work.0 + run.work.2 as i32);
    let bottom = (run.region.1 + run.region.3 as i32 + 6)
        .max(panel.y + panel.height)
        .max(toolbar.y + toolbar.height)
        .min(run.work.1 + run.work.3 as i32);
    let ready = SMOKE_VISIBLE_WAIT.with(|slot| {
        let mut slot = slot.borrow_mut();
        match *slot {
            Some(start) => start.elapsed() >= Duration::from_millis(500),
            None => {
                *slot = Some(std::time::Instant::now());
                false
            }
        }
    });
    if !ready {
        return Ok(false);
    }
    SMOKE_VISIBLE_WAIT.with(|slot| slot.borrow_mut().take());
    // Capture the normal production composition. No affinity overrides are
    // permitted here: the evidence must show what desktop sharing actually sees.
    preview.flush_composition()?;
    let frame = glint_platform::capture_region_guarded(
        (x, y, (right - x) as u32, (bottom - y) as u32),
        &run.displays,
        &[],
    )?;
    image::save_buffer(
        directory.join(filename),
        &frame.rgba,
        frame.width,
        frame.height,
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(directory.join(format!("{filename}.txt")), format!("Selection {:?}; preview {panel:?}; controls {toolbar:?}; picture {}x{}; native preview receives pointer. Normal production display affinity, without diagnostic overrides.\n",run.region,run.ui.get_picture_width(),run.ui.get_picture_height())).map_err(|e|e.to_string())?;
    Ok(true)
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
    controls: ScrollControlWindow,
    _frame: glint_platform::LiveCaptureFrame,
    _watch: glint_platform::ScrollWatch,
    source: Result<glint_platform::ScrollSource, String>,
    debounce: slint::Timer,
    sampling: slint::Timer,
    automatic_timer: slint::Timer,
    dirty: bool,
    auto_sampling: bool,
    auto_progress: bool,
    auto_stable: u8,
    auto_attempts: u8,
    auto_idle_steps: u8,
    capture_affinity: bool,
    upward: bool,
    output: Option<String>,
    finish_requested: bool,
    output_requested: bool,
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
        let _ = self.controls.hide();
    }
}

enum Task {
    Capture,
    Undo,
    Discard,
    Accept(u32),
    Preview(Option<u32>),
    Edit,
    ReviewNext,
    Finish,
}
struct Preview {
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    seams: Vec<f32>,
    pending_seam: Option<f32>,
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

fn labels(english: bool) -> ScrollLabels {
    ScrollLabels {
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
    }
}

fn live_preview(session: &ScrollSession, overlap: u32) -> Result<Preview, StitchError> {
    preview(
        session,
        session
            .pending_height()
            .map(|h| overlap.min(h.saturating_sub(1))),
    )
}

fn preview(session: &ScrollSession, overlap: Option<u32>) -> Result<Preview, StitchError> {
    let (frame, source_height) = session.thumbnail(overlap)?;
    let seams = session.seams();
    Ok(Preview {
        pending_seam: session
            .pending_seam()
            .map(|y| y as f32 * frame.height as f32 / source_height as f32),
        pixels: SharedPixelBuffer::clone_from_slice(&frame.rgba, frame.width, frame.height),
        seams: seams
            .into_iter()
            .map(|y| y as f32 * frame.height as f32 / source_height as f32)
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

fn configure_controller(
    window: &slint::Window,
    region: (i32, i32, u32, u32),
) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Not a Windows controller".into());
            };
            let native =
                unsafe { glint_platform::NativePinWindow::from_raw(handle.hwnd.get() as *mut _) };
            native.configure(true, false)?;
            let panel = native.state()?;
            let intersects = i64::from(panel.x) < i64::from(region.0) + i64::from(region.2)
                && i64::from(panel.x) + i64::from(panel.width) > i64::from(region.0)
                && i64::from(panel.y) < i64::from(region.1) + i64::from(region.3)
                && i64::from(panel.y) + i64::from(panel.height) > i64::from(region.1);
            if intersects {
                native.exclude_from_capture()
            } else {
                native.allow_capture()
            }
        })
        .ok_or("Capture controller not initialized")?
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
    ui.set_english(english);
    ui.set_dark(capture.get_dark());
    ui.set_accent(capture.get_accent());
    ui.set_labels(labels(english));
    ui.set_region_info(format!("{w} × {h} px · ({x}, {y})").into());
    ui.set_status(tr(Message::ScrollBusy, english));
    ui.set_busy(true);
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
    let frame = glint_platform::LiveCaptureFrame::prepare(region)?;
    let controls =
        crate::motion::create_settings(ScrollControlWindow::new).map_err(|e| e.to_string())?;
    controls.set_english(english);
    let owner = settings.as_weak();
    controls.on_action(move |action| {
        if let Some(owner) = owner.upgrade() {
            owner.invoke_action(format!("scroll:{ticket}:{action}").into());
        }
    });
    let owner = settings.as_weak();
    let watch = glint_platform::ScrollWatch::new(region, move |delta| {
        let _ = owner.upgrade_in_event_loop(move |owner| {
            owner.invoke_action(
                format!(
                    "scroll:{ticket}:wheel:{}",
                    if delta > 0 { "up" } else { "down" }
                )
                .into(),
            );
        });
    })?;
    let scale = ui.window().scale_factor();
    let panel_width = (300. * scale) as u32;
    let panel_height = (h + (80. * scale) as u32)
        .clamp((300. * scale) as u32, (720. * scale) as u32)
        .min(work.3.saturating_sub(24));
    ui.window()
        .set_size(slint::PhysicalSize::new(panel_width, panel_height));
    let right = i64::from(region.0) + i64::from(region.2) + 12;
    let work_right = i64::from(work.0) + i64::from(work.2);
    let left = i64::from(region.0) - i64::from(panel_width) - 12;
    // Prefer beside the live selection; if neither side fits, use the work-area
    // edge. Windows excludes the visible controllers from later acquisitions.
    let x = if right + i64::from(panel_width) <= work_right {
        right
    } else if left >= i64::from(work.0) {
        left
    } else {
        (work_right - i64::from(panel_width)).max(i64::from(work.0))
    };
    ui.window().set_position(slint::PhysicalPosition::new(
        x as i32,
        region
            .1
            .max(work.1 + 12)
            .min(work.1 + work.3.saturating_sub(panel_height + 12) as i32),
    ));
    controls.window().set_position(slint::PhysicalPosition::new(
        region
            .0
            .max(work.0)
            .min((i64::from(work.0) + i64::from(work.2) - (310. * scale) as i64) as i32),
        (i64::from(region.1) + i64::from(region.3) + 8)
            .min(i64::from(work.1) + i64::from(work.3) - (52. * scale) as i64) as i32,
    ));
    // Keep the frozen selection until both native controller windows exist.
    ui.show().map_err(|e| e.to_string())?;
    if let Err(error) = controls.show() {
        let _ = ui.hide();
        return Err(error.to_string());
    }
    let weak_ui = ui.as_weak();
    let weak_controls = controls.as_weak();
    let owner = settings.as_weak();
    if let Err(error) = slint::spawn_local(async move {
        let (Some(ui), Some(controls)) = (weak_ui.upgrade(), weak_controls.upgrade()) else {
            return;
        };
        let ready = ui.window().winit_window().await.is_ok()
            && controls.window().winit_window().await.is_ok();
        if let Some(owner) = owner.upgrade() {
            owner.invoke_action(
                format!(
                    "scroll:{ticket}:{}",
                    if ready {
                        "initialize"
                    } else {
                        "initialize-failed"
                    }
                )
                .into(),
            );
        }
    }) {
        let _ = ui.hide();
        let _ = controls.hide();
        return Err(error.to_string());
    }
    state.scroll_capture = Some(Run {
        ui,
        controls,
        _frame: frame,
        _watch: watch,
        source: Err("Capture is initializing".into()),
        debounce: slint::Timer::default(),
        sampling: slint::Timer::default(),
        automatic_timer: slint::Timer::default(),
        dirty: false,
        auto_sampling: false,
        auto_progress: false,
        auto_stable: 0,
        auto_attempts: 0,
        auto_idle_steps: 0,
        capture_affinity: false,
        upward: false,
        output: None,
        finish_requested: false,
        output_requested: false,
        ticket,
        region,
        displays,
        exclusions,
        work,
        session: Some(ScrollSession::default()),
        result: None,
        target: None,
        english,
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    Ok(())
}

fn queue(run: &mut Run, settings: &SettingsWindow, task: Task) {
    if !run.capture_affinity {
        return;
    }
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
    let region = run.region;
    // Pixel evidence is opt-in and restricted to the explicit diagnostic run.
    let evidence = SMOKE_DIRECTORY.with(|slot| slot.borrow().clone());
    let displays = run.displays.clone();
    let exclusions = run.exclusions.clone();
    let work = run.work;
    let ticket = run.ticket;
    let overlap = run.ui.get_overlap().max(0) as u32;
    let upward = run.upward;
    let reviewing = run.controls.get_stopped() && run.ui.get_repair_mode();
    let (send, receive) = mpsc::sync_channel(1);
    run.result = Some(receive);
    let owner = settings.as_weak();
    let cancelled = run.cancelled.clone();
    // Controllers stay visible; automatic scrolling verifies stability over frames.
    slint::Timer::single_shot(Duration::ZERO, move || {
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
                            if let Some(directory) = &evidence {
                                static FRAME_NUMBER: AtomicU64 = AtomicU64::new(0);
                                let index = FRAME_NUMBER.fetch_add(1, Ordering::Relaxed);
                                let _ = image::save_buffer(
                                    directory.join(format!("native-source-{index:03}.png")),
                                    &frame.rgba,
                                    frame.width,
                                    frame.height,
                                    image::ColorType::Rgba8,
                                );
                            }
                            match session.capture(
                                Fragment::new(frame.width, frame.height, frame.rgba)?,
                                upward,
                            )? {
                                Append::First | Append::Added { .. } => {}
                                Append::Duplicate => notice = Message::ScrollDuplicate,
                                Append::NeedsReview => notice = Message::ScrollUncertain,
                            }
                        }
                        Task::Undo => session.undo(),
                        Task::Discard => session.discard_pending(),
                        Task::Accept(overlap) => session.accept_pending(overlap)?,
                        Task::ReviewNext => session.review_next(),
                        Task::Finish => {
                            session.retry_alignment();
                            notice = Message::ScrollReviewHint;
                        }
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
                // Every explicit acquisition/edit publishes a fresh bounded
                // preview. Never compose full images on pointer/scroll events.
                if prepared.is_none() && image.is_none() && session.count() > 0 {
                    match if reviewing {
                        live_preview(&session, overlap)
                    } else {
                        preview(&session, None)
                    } {
                        Ok(updated) => image = Some(updated),
                        Err(_) => notice = Message::ScrollBudget,
                    }
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
    });
}

/// Called before the resident coordinator borrows AppState for ordinary actions.
pub fn handle(action: &str, shared: &Rc<RefCell<AppState>>, settings: &SettingsWindow) -> bool {
    if action == "scroll-open" && shared.borrow().scroll_capture.is_none() {
        // Release AppState before callbacks: selection validation and startup own it.
        let capture = {
            let state = shared.borrow();
            if state.lifecycle.blocked() || state.capturing || settings.get_modal() != 0 {
                return true;
            }
            state.capture.as_ref().map(ComponentHandle::clone_strong)
        };
        if let Some(capture) = capture {
            capture.invoke_action(
                if capture.get_capture_mode() == 1 && capture.get_selected() {
                    "scroll-start"
                } else {
                    "mode-scroll"
                }
                .into(),
            );
        } else {
            let settings = settings.as_weak();
            let shared = shared.clone();
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(settings) = settings.upgrade() {
                    settings.invoke_action("capture".into());
                    shared.borrow_mut().capture_mode = 1;
                }
            });
        }
        return true;
    }
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
            match start(&mut state, settings, (*x, *y, *w as u32, *h as u32)) {
                Ok(()) => {}
                Err(error) => {
                    if let Some(capture) = &state.capture {
                        let notice = format!(
                            "{}: {error}",
                            messages::text(Message::ScrollFailed, capture.get_english())
                        );
                        capture.set_service_notice(notice.into());
                        let _ = capture.show();
                    }
                }
            }
        }
        return true;
    }
    let capture_hotkey =
        matches!(action, "capture" | "scroll-open") && shared.borrow().scroll_capture.is_some();
    let Some(command) = action.strip_prefix("scroll:") else {
        if capture_hotkey {
            let mut state = shared.borrow_mut();
            if !state.lifecycle.blocked()
                && let Some(run) = &mut state.scroll_capture
            {
                run.ui.set_review(false);
                run.controls.set_stopped(false);
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
            let mut state = shared.borrow_mut();
            if !state.lifecycle.blocked()
                && let Some(run) = &mut state.scroll_capture
            {
                run.ui.window().set_position(slint::PhysicalPosition::new(
                    run.work.0 + 20,
                    run.work.1 + 20,
                ));
                // Moving the panel can make it overlap the source. Reapply its
                // capture policy before accepting another acquisition.
                if run.capture_affinity
                    && let Err(error) = configure_controller(run.ui.window(), run.region)
                {
                    run.capture_affinity = false;
                    run.automatic_timer.stop();
                    run.controls.set_automatic(false);
                    run.ui.set_status(error.into());
                }
                // First show and capture policy are initialized asynchronously.
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
    if matches!(command, "initialize" | "initialize-failed") {
        if run.capture_affinity {
            return true;
        }
        let initialized = (|| -> Result<(), String> {
            if command == "initialize-failed" {
                return Err("Native preview window initialization failed".into());
            }
            configure_controller(run.ui.window(), run.region)?;
            configure_controller(run.controls.window(), run.region)?;
            Ok(())
        })();
        let initialized = initialized.and_then(|()| {
            if let Some(capture) = &state.capture {
                capture.hide().map_err(|e| e.to_string())?;
            }
            if SMOKE_FAIL_START.with(|fail| fail.replace(false)) {
                return Err("Controlled scrolling startup failure".into());
            }
            if let Some(target) = &state.capture_focus {
                let _ = target.restore();
            }
            let run = state.scroll_capture.as_mut().ok_or("Capture cancelled")?;
            run.target = glint_platform::remember_foreground();
            run.source = glint_platform::ScrollSource::new(run.region);
            run._frame.show();
            run.capture_affinity = true;
            queue(run, settings, Task::Capture);
            Ok(())
        });
        if let Err(error) = initialized {
            state.scroll_capture.take();
            if let Some(capture) = &state.capture {
                capture.set_service_notice(
                    format!(
                        "{}: {error}",
                        messages::text(Message::ScrollFailed, capture.get_english())
                    )
                    .into(),
                );
                let _ = capture.show();
            }
        }
        return true;
    }
    if command == "cancel" {
        glint_platform::diagnostic_log::emit(
            glint_platform::diagnostic_log::Event::ScrollCancelled,
        );
        state.scroll_capture.take();
        state.capture_settings_visible = false;
        let capture = state.capture.as_ref().map(ComponentHandle::clone_strong);
        drop(state);
        if let Some(capture) = capture {
            capture.invoke_action("cancel".into());
        }
        return true;
    }
    if command == "ready" {
        glint_platform::diagnostic_log::emit(glint_platform::diagnostic_log::Event::ScrollReady);
        let result = run.result.take().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = result {
            if let Some(prepared) = result.prepared {
                state.scroll_output = run.output.take();
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
                let settings = settings.as_weak();
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(settings) = settings.upgrade() {
                        settings.invoke_action("capture-ready".into());
                    }
                });
                return true;
            }
            run.ui.set_busy(false);
            if matches!(result.notice, Message::ScrollBudget | Message::ScrollFailed) {
                run.automatic_timer.stop();
                run.controls.set_automatic(false);
            }
            run.ui
                .set_pending(result.session.pending_height().is_some());
            run.ui
                .set_unresolved(result.session.unresolved_count() as i32);
            run.ui.set_join_position(
                result
                    .session
                    .review_position()
                    .map(|(a, b)| format!("{a} → {b} · {}", result.session.unresolved_count()))
                    .unwrap_or_default()
                    .into(),
            );
            run.ui.set_can_undo(
                result.session.count() > 1 || result.session.pending_height().is_some(),
            );
            run.ui.set_can_finish(result.session.count() > 0);
            run.controls.set_ready(result.session.count() > 0);
            if matches!(result.notice, Message::ScrollReviewHint) {
                run.ui.set_review(true);
                run.controls.set_stopped(true);
            }
            let dimensions = result.session.dimensions().unwrap_or_default();
            run.ui.set_status(
                format!(
                    "{} · {} · {} × {} px",
                    messages::text(
                        if !run.controls.get_stopped()
                            && matches!(result.notice, Message::ScrollUncertain)
                        {
                            Message::ScrollReady
                        } else {
                            result.notice
                        },
                        run.english
                    ),
                    result.session.count(),
                    dimensions.0,
                    dimensions.1
                )
                .into(),
            );
            if run.ui.get_review() && run.ui.get_pending() {
                run.ui.set_status(
                    (if run.english {
                        "Incomplete join; verified range shown, sources retained"
                    } else {
                        "部分内容未拼接，显示已对齐范围，原片段已保留"
                    })
                    .into(),
                );
            }
            if run.auto_sampling && run.controls.get_automatic() {
                run.auto_attempts = run.auto_attempts.saturating_add(1);
                if matches!(result.notice, Message::ScrollDuplicate) {
                    run.auto_stable = run.auto_stable.saturating_add(1);
                } else {
                    run.auto_stable = 0;
                    if matches!(
                        result.notice,
                        Message::ScrollReady | Message::ScrollUncertain
                    ) {
                        run.auto_progress = true;
                    }
                }
                // Stability describes source motion, independently of alignment.
                // A retained pending join must not freeze auto scrolling forever.
                if run.auto_stable >= 2 {
                    run.auto_sampling = false;
                    run.auto_idle_steps = if run.auto_progress {
                        0
                    } else {
                        run.auto_idle_steps.saturating_add(1)
                    };
                    let idle_limit = run
                        .source
                        .as_ref()
                        .map_or(2, |source| source.idle_step_limit());
                    if run.auto_idle_steps >= idle_limit {
                        run.controls.set_automatic(false);
                        run.ui.set_status(
                            (if run.english {
                                "No further movement detected; finish or continue manually"
                            } else {
                                "未检测到继续滚动，可结束或手动继续"
                            })
                            .into(),
                        );
                    }
                } else if run.auto_attempts >= 16 {
                    run.auto_sampling = false;
                    run.controls.set_automatic(false);
                    run.ui.set_status((if run.english { "Page has not settled. Auto scroll paused; sources retained. Continue manually or retry." } else { "页面尚未稳定，已暂停自动滚动并保留片段，可手动继续或重试。" }).into());
                }
            }
            if let Some(preview) = result.preview {
                let pending_seam = preview.pending_seam;
                run.ui.set_picture_width(preview.pixels.width() as f32);
                run.ui.set_picture_height(preview.pixels.height() as f32);
                run.ui.set_picture(slint::Image::from_rgba8(preview.pixels));
                run.ui.set_has_picture(true);
                run.ui
                    .set_seams(ModelRc::new(VecModel::from(preview.seams)));
                if run.controls.get_stopped()
                    && run.ui.get_repair_mode()
                    && run.ui.get_pending()
                    && let Some(seam) = pending_seam
                {
                    let ui = run.ui.as_weak();
                    slint::Timer::single_shot(Duration::from_millis(20), move || {
                        if let Some(ui) = ui.upgrade() {
                            ui.invoke_reveal_seam(seam);
                        }
                    });
                } else if run.upward {
                    run.ui.invoke_reveal_first();
                } else {
                    run.ui.invoke_reveal_latest();
                }
            } else {
                // Never display a stale result after undoing/changing a join.
                run.ui.set_picture(slint::Image::default());
                run.ui.set_has_picture(false);
            }
            run.session = Some(result.session);
            if !run.ui.window().is_visible() {
                // Affinity is installed once, before any subsequent acquisition.
                let shown = run
                    .ui
                    .show()
                    .map_err(|e| e.to_string())
                    .and_then(|_| run.controls.show().map_err(|e| e.to_string()))
                    .and_then(|_| configure_controller(run.ui.window(), run.region))
                    .and_then(|_| configure_controller(run.controls.window(), run.region));
                run.capture_affinity = shown.is_ok();
                if let Err(error) = shown {
                    run.controls.set_stopped(true);
                    run.controls.set_automatic(false);
                    run.ui.set_status(error.into());
                }
                if let Some(target) = &run.target {
                    let _ = target.restore();
                }
            }
            if run.finish_requested {
                if run.dirty {
                    run.dirty = false;
                    queue(run, settings, Task::Capture);
                } else {
                    run.finish_requested = false;
                    queue(run, settings, Task::Finish);
                }
            } else if run.ui.get_review() && run.output_requested {
                run.output_requested = false;
                if run.ui.get_pending() {
                    // A single result-level recovery state. Never force serial
                    // seam confirmations or silently copy an incomplete image.
                    run.output = None;
                    run.ui.set_repair_mode(false);
                } else {
                    queue(run, settings, Task::Edit);
                }
            } else if run.dirty && !run.controls.get_stopped() {
                run.dirty = false;
                queue(run, settings, Task::Capture);
            } else if run.controls.get_automatic() {
                schedule_auto(run, settings);
            }
        }
        return true;
    }
    if let Some(direction) = command.strip_prefix("wheel:") {
        if run.controls.get_stopped() {
            return true;
        }
        run.upward = direction == "up";
        run.dirty = true;
        if !run.sampling.running() {
            let owner = settings.as_weak();
            let ticket = run.ticket;
            run.sampling.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(120),
                move || {
                    if let Some(owner) = owner.upgrade() {
                        owner.invoke_action(format!("scroll:{ticket}:sample").into());
                    }
                },
            );
        }
        let owner = settings.as_weak();
        let ticket = run.ticket;
        run.debounce.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(360),
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.invoke_action(format!("scroll:{ticket}:settled").into());
                }
            },
        );
        return true;
    }
    if command == "auto" {
        if run.controls.get_stopped() {
            return true;
        }
        let enabled = !run.controls.get_automatic();
        run.controls.set_automatic(enabled);
        run.automatic_timer.stop();
        run.auto_sampling = false;
        run.auto_idle_steps = 0;
        if enabled {
            run.upward = false;
            schedule_auto(run, settings);
        }
        return true;
    }
    if command == "auto-step" {
        if !run.controls.get_automatic() || run.controls.get_stopped() {
            return true;
        }
        if let Some(target) = &run.target {
            let _ = target.restore();
        }
        let result = run
            .source
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|source| source.scroll_down());
        if let Err(error) = result {
            run.controls.set_automatic(false);
            run.ui.set_status(error.into());
            return true;
        }
        // Observe successive frames before issuing another wheel step.
        run.auto_sampling = true;
        run.auto_progress = false;
        run.auto_stable = 0;
        run.auto_attempts = 0;
        run.dirty = false;
        schedule_auto(run, settings);
        return true;
    }
    let output = matches!(command, "edit" | "save" | "copy");
    if command == "stop" || (output && !run.ui.get_review()) {
        if output {
            run.output_requested = true;
            run.output = (command != "edit").then(|| command.to_owned());
        }
        run.dirty |= run.auto_sampling;
        run.auto_sampling = false;
        run.controls.set_stopped(true);
        run.controls.set_automatic(false);
        run.automatic_timer.stop();
        run.debounce.stop();
        run.sampling.stop();
        run.finish_requested = true;
        if run.result.is_none() {
            if run.dirty {
                run.dirty = false;
                queue(run, settings, Task::Capture);
            } else {
                run.finish_requested = false;
                queue(run, settings, Task::Finish);
            }
        }
        return true;
    }
    if command == "sample" && !run.controls.get_stopped() {
        if run.result.is_none() {
            run.dirty = false;
            queue(run, settings, Task::Capture);
        } else {
            run.dirty = true;
        }
        return true;
    }
    if command == "settled" {
        run.sampling.stop();
    }
    if run.result.is_some() {
        return true;
    }
    let Some(_session) = &run.session else {
        return true;
    };
    let overlap = run.ui.get_overlap().max(0) as u32;
    let task = match command {
        "settled" if !run.controls.get_stopped() => {
            run.dirty = false;
            Task::Capture
        }
        "capture" => Task::Capture,
        "repair" => {
            run.ui.set_repair_mode(true);
            Task::Preview(Some(overlap))
        }
        "review-next" => Task::ReviewNext,
        "undo" => Task::Undo,
        "discard" => Task::Discard,
        "accept" => Task::Accept(overlap),
        "preview" => Task::Preview(Some(overlap)),
        "stop" => Task::Preview(None),
        "edit" | "save" | "copy" => {
            run.controls.set_stopped(true);
            run.controls.set_automatic(false);
            run.automatic_timer.stop();
            run.debounce.stop();
            run.sampling.stop();
            run.output_requested = false;
            run.output = (command != "edit").then(|| command.to_owned());
            Task::Edit
        }
        "resume" => {
            run.ui.set_review(false);
            run.ui.set_repair_mode(false);
            run.output_requested = false;
            run.output = None;
            run.controls.set_stopped(false);
            return true;
        }
        _ => return true,
    };
    queue(run, settings, task);
    true
}

fn schedule_auto(run: &Run, settings: &SettingsWindow) {
    let owner = settings.as_weak();
    let ticket = run.ticket;
    let sampling = run.auto_sampling;
    run.automatic_timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(if sampling { 120 } else { 350 }),
        move || {
            if let Some(owner) = owner.upgrade() {
                let command = if sampling { "sample" } else { "auto-step" };
                owner.invoke_action(format!("scroll:{ticket}:{command}").into());
            }
        },
    );
}

/// Deterministic native layout probe: generated pixels only, no desktop capture.
pub fn preview_smoke(
    path: &str,
    english: bool,
    pending: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = ScrollSession::default();
    for shade in [35u8, 95, 155] {
        let mut rgba = vec![255; 320 * 640 * 4];
        for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let y = index / 320;
            let color = if (y / 48) % 2 == 0 {
                [shade, 120, 210, 255]
            } else {
                [235, 240, 250, 255]
            };
            pixel.copy_from_slice(&color);
        }
        session
            .append(Fragment::new(320, 640, rgba).map_err(|e| format!("{e:?}"))?)
            .map_err(|e| format!("{e:?}"))?;
        if session.pending_height().is_some() {
            session.accept_pending(160).map_err(|e| format!("{e:?}"))?;
        }
    }
    if pending {
        session
            .append(
                Fragment::new(320, 640, [225, 180, 80, 255].repeat(320 * 640))
                    .map_err(|e| format!("{e:?}"))?,
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    let picture = live_preview(&session, 160).map_err(|e| format!("{e:?}"))?;
    let pending_seam = picture.seams.last().copied().unwrap_or_default();
    let ui = crate::motion::create_settings(ScrollCaptureWindow::new)?;
    ui.set_english(english);
    ui.set_labels(labels(english));
    ui.set_status("Synthetic layout probe · 3 fragments · 320 × 1600 px".into());
    ui.set_accent(slint::Color::from_rgb_u8(75, 92, 250));
    ui.set_can_finish(true);
    ui.set_can_undo(true);
    ui.set_pending(pending);
    ui.set_review(pending);
    ui.set_repair_mode(pending);
    ui.set_maximum_overlap(639);
    ui.set_overlap(160);
    ui.set_picture_width(picture.pixels.width() as f32);
    ui.set_picture_height(picture.pixels.height() as f32);
    ui.set_picture(slint::Image::from_rgba8(picture.pixels));
    ui.set_has_picture(true);
    ui.set_seams(ModelRc::new(VecModel::from(picture.seams)));
    ui.show()?;
    let weak = ui.as_weak();
    let path = path.to_owned();
    let result = Rc::new(RefCell::new(Ok(())));
    let output = result.clone();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        let captured = (|| -> Result<(), String> {
            let ui = weak.upgrade().ok_or("Preview closed")?;
            if pending {
                ui.invoke_reveal_seam(pending_seam);
            } else {
                ui.invoke_reveal_latest();
            }
            if ui.get_preview_offset() >= 0. {
                return Err("Long preview did not reveal the latest fragment".into());
            }
            let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
            image::save_buffer(
                &path,
                pixels.as_bytes(),
                pixels.width(),
                pixels.height(),
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())
        })();
        *output.borrow_mut() = captured;
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop()?;
    result.borrow_mut().clone().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn solid(shade: u8) -> Fragment {
        Fragment::new(32, 64, [shade, 0, 0, 255].repeat(32 * 64)).unwrap()
    }
    #[test]
    fn preview_tracks_retained_frames_and_post_capture_join_edits() {
        let mut session = ScrollSession::default();
        session.append(solid(20)).unwrap();
        assert_eq!(live_preview(&session, 16).unwrap().pixels.height(), 64);
        assert_eq!(session.append(solid(80)).unwrap(), Append::NeedsReview);
        let proposed = live_preview(&session, 16).unwrap();
        assert_eq!(proposed.pixels.height(), 112);
        assert_eq!(proposed.seams, vec![64.]);
        assert_eq!(session.count(), 2);
        assert_eq!(
            live_preview(&session, u32::MAX).unwrap().pixels.height(),
            65
        );
        session.discard_pending();
        assert_eq!(live_preview(&session, 16).unwrap().pixels.height(), 64);
        session.append(solid(80)).unwrap();
        session.accept_pending(16).unwrap();
        let accepted = live_preview(&session, 16).unwrap();
        assert_eq!(accepted.pixels.as_bytes(), proposed.pixels.as_bytes());
        session.undo();
        let undone = live_preview(&session, 16).unwrap();
        assert_eq!(undone.pixels.height(), 64);
        assert!(undone.seams.is_empty());
    }
}
