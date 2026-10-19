//! Actual read-only system queries; only the error delivery is synthetic.
use crate::{resident::AppState, settings::SettingsWindow};
use slint::ComponentHandle;
use std::{cell::RefCell, path::Path, rc::Rc, time::Instant};

#[derive(Default)]
pub struct Run {
    config: Vec<u8>,
    started: Option<Instant>,
    completed: u64,
    body: String,
    environment: Option<glint_platform::DesktopEnvironment>,
}
impl Run {
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if self.started.is_some_and(|t| t.elapsed().as_secs() > 12) {
            return Err("Diagnostics refresh timed out".into());
        }
        match stage {
            0 => {
                self.config = std::fs::read(dir.join("config.toml")).unwrap_or_default();
                ui.invoke_action("open".into());
                ui.invoke_choose_page(10);
                ui.window().set_size(slint::PhysicalSize::new(900, 650));
                if app.borrow().diagnostics.last.is_some()
                    || app.borrow().diagnostics.receive.is_some()
                {
                    return Err("Navigation performed an unsolicited diagnostics query".into());
                }
                self.environment = Some(glint_platform::desktop_environment()?);
                self.completed = app.borrow().diagnostics.completed;
                ui.invoke_action("diagnostics-refresh".into());
                ui.invoke_action("diagnostics-refresh".into());
                if !ui.get_diagnostics_busy() {
                    return Err("Diagnostics worker was not marked busy".into());
                }
                self.started = Some(Instant::now());
            }
            1 => {
                let state = app.borrow();
                if state.diagnostics.receive.is_some() {
                    return Ok(false);
                }
                self.started = None;
                if state.diagnostics.completed != self.completed + 1
                    || state.diagnostics.error.is_some()
                {
                    return Err("Duplicate refresh or real query failed".into());
                }
                let report = state
                    .diagnostics
                    .last
                    .as_ref()
                    .ok_or("No diagnostics report")?;
                if report.environment.as_ref().map_err(Clone::clone)?
                    != self.environment.as_ref().unwrap()
                {
                    return Err(
                        "Worker display/DPI report differs from UI thread environment".into(),
                    );
                }
                let r = report.resources.as_ref().map_err(Clone::clone)?;
                if r.working_set_bytes == 0
                    || r.private_bytes == 0
                    || r.handles == 0
                    || ui.get_diagnostics_busy()
                {
                    return Err("Actual process resources are invalid".into());
                }
                self.body = ui.get_diagnostics_body().to_string();
                drop(state);
                super::snapshot(ui, &dir.join("diagnostics-initial.png"))?;
                // Deliver a failed query through the same completion handler.
                let (send, recv) = std::sync::mpsc::sync_channel(1);
                send.send(crate::runtime_diagnostics::Report {
                    environment: Err("injected display query failure".into()),
                    resources: Err("injected resource query failure".into()),
                })
                .unwrap();
                app.borrow_mut().diagnostics.receive = Some(recv);
                ui.invoke_action("diagnostics-ready".into());
                if !ui.get_diagnostics_body().starts_with(&self.body)
                    || app.borrow().diagnostics.error.is_none()
                    || ui.get_diagnostics_busy()
                {
                    return Err("Failed diagnostics refresh lost previous snapshot or retry".into());
                }
                self.completed = app.borrow().diagnostics.completed;
                ui.invoke_change("general.language".into(), "en".into());
            }
            2 => {
                if app.borrow().diagnostics.completed != self.completed
                    || !ui.get_diagnostics_body().contains("Working set")
                {
                    return Err(
                        "Language preview queried again or did not translate snapshot".into(),
                    );
                }
                super::snapshot(ui, &dir.join("diagnostics-en-error.png"))?;
                ui.window()
                    .dispatch_event_with_result(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(650., 400.),
                        delta_x: 0.,
                        delta_y: -300.,
                    })
                    .map_err(|e| e.to_string())?;
            }
            3 => {
                super::snapshot(ui, &dir.join("diagnostics-en-scrolled.png"))?;
                ui.invoke_action("cancel".into());
                ui.invoke_change("general.language".into(), "zh_cn".into());
                ui.invoke_choose_page(10);
                ui.window()
                    .dispatch_event_with_result(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(650., 400.),
                        delta_x: 0.,
                        delta_y: 2000.,
                    })
                    .map_err(|e| e.to_string())?;
            }
            4 => {
                if !ui.get_diagnostics_body().contains("工作集")
                    || app.borrow().diagnostics.completed != self.completed
                {
                    return Err("Chinese relabel failed or queried again".into());
                }
                super::snapshot(ui, &dir.join("diagnostics-zh.png"))?;
                ui.invoke_action("cancel".into());
                self.completed = app.borrow().diagnostics.completed;
                ui.invoke_action("diagnostics-refresh".into());
                ui.invoke_action("close".into());
                self.started = Some(Instant::now());
            }
            5 => {
                let state = app.borrow();
                if state.diagnostics.receive.is_some() {
                    return Ok(false);
                }
                self.started = None;
                if state.diagnostics.completed != self.completed + 1
                    || state.diagnostics.error.is_some()
                    || ui.window().is_visible()
                {
                    return Err("Retry failed or completion reopened settings".into());
                }
                if state.session.preferences.is_dirty()
                    || state.capture.is_some()
                    || state.capturing
                {
                    return Err("Read-only diagnostics changed settings or capture state".into());
                }
                if std::fs::read(dir.join("config.toml")).unwrap_or_default() != self.config {
                    return Err("Diagnostics wrote configuration".into());
                }
                eprintln!(
                    "Native diagnostics: real display/DPI/process counters, one bounded refresh, synthetic failure retains snapshot, bilingual relabel without query, config isolation and hidden completion passed"
                );
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}
