//! Real completion, clipboard contention, native popup lifetime and Windows submission.
use crate::{completion, resident::AppState, settings::SettingsWindow};
use glint_core::config::Config;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Run {
    old: Option<Config>,
    output: Option<PathBuf>,
    blocker: Option<glint_platform::ClipboardBlocker>,
    started: Option<Instant>,
}
fn region(c: &crate::probe::CaptureProbe) {
    c.invoke_pointer(0, 200., 200.);
    c.invoke_pointer(1, 600., 500.);
    c.invoke_pointer(2, 600., 500.);
}
fn shot(ui: &completion::CompletionToast, path: &Path) -> Result<(), String> {
    let p = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    image::save_buffer_with_format(
        path,
        p.as_bytes(),
        p.width(),
        p.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())
}
impl Run {
    fn begin(&mut self, ui: &SettingsWindow) {
        ui.invoke_action("capture".into());
        self.started = Some(Instant::now());
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
        {
            return Err(format!("Completion stage {stage} timed out"));
        }
        if app.borrow().capturing || app.borrow().capture.as_ref().is_some_and(|c| c.get_busy()) {
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.output = app.borrow().capture_output.clone();
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                for (k, v) in [
                    ("general.language", "en"),
                    ("capture.delay_seconds", "0"),
                    ("capture.detect_window", "false"),
                    ("annotation.tool", "select"),
                    ("output.feedback", "none"),
                    ("output.format", "png"),
                ] {
                    ui.invoke_change(k.into(), v.into());
                }
                ui.invoke_action("save".into());
                self.begin(ui);
            }
            1 => {
                let c = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("None-mode capture missing")?
                    .clone_strong();
                region(&c);
                self.blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                c.invoke_action("copy".into());
            }
            2 => {
                let c = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("None-mode failure lost editor")?
                    .clone_strong();
                if c.get_completion_kind() != 0
                    || completion::inspect().0
                    || !c.get_status().contains("Output failed")
                {
                    return Err("None feedback hid clipboard failure or announced success".into());
                }
                self.blocker.take();
                c.invoke_action("copy".into());
            }
            3 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                if completion::inspect() != (false, 0, false) {
                    return Err("None mode displayed success".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_change("output.feedback".into(), "toast".into());
                ui.invoke_action("save".into());
                self.begin(ui);
            }
            4 => {
                let c = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Cancel capture missing")?
                    .clone_strong();
                c.invoke_action("cancel".into());
                if completion::inspect().0 {
                    return Err("Cancel announced success".into());
                }
                app.borrow_mut().capture_output = Some(dir.join("completion-toast.png"));
            }
            5 => {
                if app.borrow().capture.is_none() {
                    self.begin(ui);
                    return Ok(false);
                }
                let c = app.borrow().capture.as_ref().unwrap().clone_strong();
                region(&c);
                c.invoke_action("save".into());
            }
            6 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                let Some(popup) = completion::popup() else {
                    return Ok(false);
                };
                if completion::inspect().1 != 2 || popup.get_fallback() {
                    return Err("Wrong success feedback".into());
                }
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                popup
                    .window()
                    .with_winit_window(|w| {
                        let handle = w.window_handle().map_err(|e| e.to_string())?;
                        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                            return Err("Not Win32".into());
                        };
                        let n = unsafe {
                            glint_platform::NativePinWindow::from_raw(
                                handle.hwnd.get() as *mut std::ffi::c_void
                            )
                        };
                        let s = n.state()?;
                        if !s.no_activate || !s.click_through || n.is_foreground() {
                            return Err(format!(
                                "Completion popup invalid: {s:?}, foreground={}",
                                n.is_foreground()
                            ));
                        }
                        Ok::<(), String>(())
                    })
                    .ok_or("Missing completion peer")??;
                shot(&popup, &dir.join("completion-toast-en.png"))?;
                self.started = Some(Instant::now());
            }
            7 => {
                if completion::inspect().0 {
                    return Ok(false);
                }
                if completion::inspect().1 != 0 {
                    return Err("Expired popup retained live success state".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_change("output.feedback".into(), "notification".into());
                ui.invoke_action("save".into());
                app.borrow_mut().capture_output = Some(dir.join("completion-notification.png"));
                self.begin(ui);
            }
            8 => {
                let c = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Notification capture missing")?
                    .clone_strong();
                region(&c);
                c.invoke_action("save".into());
            }
            9 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                let (visible, kind, submitted) = completion::inspect();
                if !visible && !submitted {
                    return Ok(false);
                }
                if kind != 2 {
                    return Err("Notification completion lost successful operation".into());
                }
                if let Some(p) = completion::popup() {
                    if !p.get_fallback() {
                        return Err("Failed system notification did not explain fallback".into());
                    }
                    shot(&p, &dir.join("completion-notification-fallback.png"))?;
                }
                eprintln!(
                    "Actual Windows notification submission: accepted={submitted}, brief_fallback={visible}; visual OS delivery not asserted"
                );
                completion::show(
                    glint_core::config::Feedback::Notification,
                    1,
                    true,
                    true,
                    ui.get_accent(),
                    ui.get_interface_scale(),
                    ui.get_reduce_motion(),
                );
                completion::dismiss();
                self.started = Some(Instant::now());
            }
            10 => {
                if completion::pending() {
                    return Ok(false);
                }
                if completion::inspect() != (false, 0, false) {
                    return Err("Late notification resurrected dismissed feedback".into());
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                app.borrow_mut().capture_output = self.output.take();
                ui.invoke_action("open".into());
                ui.invoke_action("save".into());
                ui.invoke_action("close".into());
                completion::dismiss();
                eprintln!(
                    "Completion feedback: None keeps real clipboard failure/retry, cancel silence, save toast with native nonactivation/click-through, real three-second expiry, actual Windows submission/fallback, late-result dismissal passed"
                );
            }
            _ => return Err("Unexpected completion stage".into()),
        }
        Ok(true)
    }
}
