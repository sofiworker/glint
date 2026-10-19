//! Controlled real-process gate and recovery through production resident callbacks.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};
slint::slint! {
    export component ExclusionFixture inherits Window {
        title: "Glint exclusion verification";
        width: 160px; height: 120px;
        background: #705050;
    }
}
#[derive(Default)]
pub struct Run {
    old: Option<glint_core::config::Config>,
    fixture: Option<ExclusionFixture>,
    started: Option<Instant>,
    region: String,
    output_before: Option<Vec<u8>>,
}
impl Run {
    fn wait(&self) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
        {
            Err("Exclusion probe timed out".into())
        } else {
            Ok(false)
        }
    }
    fn denied(&self, app: &Rc<RefCell<AppState>>) -> Result<bool, String> {
        let state = app.borrow();
        if state.capturing || state.countdown.is_some() {
            return self.wait();
        }
        if state.capture.is_some()
            || !state
                .feedback
                .contains(messages::text(Message::ExclusionBlocked, true))
            || self.region != format!("{:?}", state.last_region)
        {
            return Err(format!(
                "Exclusion denial changed capture/region or lacked recovery feedback: {}",
                state.feedback
            ));
        }
        if state
            .capture_output
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            != self.output_before
        {
            return Err("Rejected desktop capture changed the export file".into());
        }
        Ok(true)
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.region = format!("{:?}", app.borrow().last_region);
                self.output_before = app
                    .borrow()
                    .capture_output
                    .as_ref()
                    .and_then(|p| std::fs::read(p).ok());
                ui.invoke_choose_page(7);
                ui.invoke_change("privacy.excluded_apps".into(), "C:/invalid.exe".into());
                ui.invoke_action("save".into());
                if !app.borrow().session.preferences.is_dirty()
                    || app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .privacy
                        .excluded_apps
                        == "C:/invalid.exe"
                {
                    return Err("Invalid exclusion edit was accepted".into());
                }
                ui.invoke_action("cancel".into());
                let name = std::env::current_exe()
                    .map_err(|e| e.to_string())?
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                ui.invoke_change("privacy.excluded_apps".into(), name.clone().into());
                ui.invoke_action("cancel".into());
                if app.borrow().session.preferences.draft.privacy.excluded_apps
                    != self.old.as_ref().unwrap().privacy.excluded_apps
                {
                    return Err("Exclusion cancel discarded baseline".into());
                }
                let fixture = ExclusionFixture::new().map_err(|e| e.to_string())?;
                fixture.show().map_err(|e| e.to_string())?;
                self.fixture = Some(fixture);
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_change("privacy.excluded_apps".into(), name.to_uppercase().into());
                ui.invoke_change("capture.delay_seconds".into(), "0".into());
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Valid exclusion save failed".into());
                }
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            1 => {
                if !self.denied(app)? {
                    return Ok(false);
                }
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(700., 400.),
                        delta_x: 0.,
                        delta_y: -2000.,
                    });
                super::snapshot(ui, &dir.join("exclusion-blocked-en.png"))?;
                ui.invoke_change("general.language".into(), "zh_cn".into());
                super::snapshot(ui, &dir.join("exclusion-settings-zh.png"))?;
                ui.invoke_action("cancel".into());
                ui.invoke_change("capture.delay_seconds".into(), "1".into());
                ui.invoke_action("save".into());
                ui.invoke_action("window-capture".into());
                if app.borrow().countdown.is_none() {
                    return Err("Exclusion probe did not start countdown".into());
                }
                self.started = Some(Instant::now());
            }
            2 => {
                if !self.denied(app)? {
                    return Ok(false);
                }
            }
            3 => {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                self.fixture
                    .as_ref()
                    .unwrap()
                    .window()
                    .with_winit_window(|window| {
                        let handle = window.window_handle().map_err(|e| e.to_string())?;
                        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                            return Err("Not a Win32 fixture".to_owned());
                        };
                        unsafe {
                            glint_platform::NativePinWindow::from_raw(
                                handle.hwnd.get() as *mut std::ffi::c_void
                            )
                        }
                        .hide_for_capture()
                    })
                    .ok_or("Fixture peer unavailable")??;
                self.fixture
                    .as_ref()
                    .unwrap()
                    .hide()
                    .map_err(|e| e.to_string())?;
                ui.invoke_change("capture.delay_seconds".into(), "0".into());
                ui.invoke_action("save".into());
                ui.invoke_action("capture".into());
                self.started = Some(Instant::now());
            }
            4 => {
                let Some(capture) = app.borrow().capture.as_ref().map(|c| c.clone_strong()) else {
                    if !app.borrow().capturing {
                        return Err(format!(
                            "Hidden-fixture retry did not open capture: {}",
                            app.borrow().feedback
                        ));
                    }
                    return self.wait();
                };
                capture.invoke_action("cancel".into());
            }
            5 => {
                if app.borrow().capture.is_some() {
                    return self.wait();
                }
                self.fixture
                    .as_ref()
                    .unwrap()
                    .show()
                    .map_err(|e| e.to_string())?;
                image::RgbaImage::from_pixel(320, 200, image::Rgba([40, 80, 120, 255]))
                    .save(dir.join("exclusion-local.png"))
                    .map_err(|e| e.to_string())?;
                ui.invoke_action("workbench".into());
                crate::local_image::selected(
                    &mut app.borrow_mut(),
                    dir.join("exclusion-local.png"),
                );
                ui.invoke_action("local-image".into());
                self.started = Some(Instant::now());
            }
            6 => {
                let Some(capture) = app.borrow().capture.as_ref().map(|c| c.clone_strong()) else {
                    return self.wait();
                };
                if !app.borrow().local_edit {
                    return Err("Exclusion local bypass used desktop capture".into());
                }
                capture.invoke_action("cancel".into());
            }
            7 => {
                if app.borrow().capture.is_some() {
                    return self.wait();
                }
                self.fixture
                    .take()
                    .unwrap()
                    .hide()
                    .map_err(|e| e.to_string())?;
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Exclusion probe config restore failed".into());
                }
                ui.set_workbench(false);
                std::fs::write(dir.join("EXCLUSIONS-PASSED.txt"),"Invalid edit/cancel, saved ordinal process-name match, real visible-window denial, delayed-window denial, hidden-window GDI retry and local-image bypass passed. No atomic privacy guarantee; no physical input or other display environments tested.\n").map_err(|e|e.to_string())?;
            }
            _ => return Err("Unknown exclusion probe stage".into()),
        }
        Ok(true)
    }
}
