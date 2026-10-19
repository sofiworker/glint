//! Inject picker results at the production boundary; selected PNG is decoded normally.
use crate::{
    local_image,
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Run {
    draft: Option<glint_core::config::Config>,
    disk: Option<Vec<u8>>,
    old_path: Option<PathBuf>,
    feedback: String,
    region: String,
    visits: Rc<Cell<u32>>,
    started: Option<Instant>,
}
fn config_bytes(dir: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(dir.join("config.toml")) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}
impl Run {
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if app.borrow().capturing {
            if self
                .started
                .is_some_and(|t| t.elapsed() > Duration::from_secs(20))
            {
                return Err("Deferred local image decode timeout".into());
            }
            return Ok(false);
        }
        match stage {
            0 => {
                self.draft = Some(app.borrow().session.preferences.draft.clone());
                self.disk = config_bytes(dir)?;
                self.old_path = app.borrow().local_path.clone();
                self.feedback = app.borrow().feedback.clone();
                self.region = format!("{:?}", app.borrow().last_region);
                ui.invoke_action("workbench".into());
                if !ui.get_workbench() || !ui.window().is_visible() || ui.get_modal() != 0 {
                    return Err("Local picker fixture could not open workbench".into());
                }
                let visits = self.visits.clone();
                local_image::queue_selection(app, ui, move |ui| {
                    visits.set(visits.get() + 1);
                    ui.invoke_action("capture".into());
                    ui.invoke_action("quit".into());
                    ui.invoke_action("rescue-pins".into());
                    ui.invoke_action("hotkey-error".into());
                    Ok(None)
                });
                local_image::queue_selection(app, ui, |_| panic!("Duplicate local picker"));
                if self.visits.get() != 0 || !app.borrow().local_dialog {
                    return Err("Local picker did not defer/reserve its slot".into());
                }
            }
            1 => {
                if self.visits.get() != 1
                    || app.borrow().local_dialog
                    || !ui.get_workbench()
                    || app.borrow().capture.is_some()
                    || app.borrow().quit_pending
                    || app.borrow().feedback
                        != messages::text(Message::ShortcutServiceStopped, app.borrow().en())
                    || app.borrow().local_path != self.old_path
                {
                    return Err("Local cancel lost context or nested recovery/guards".into());
                }
                local_image::queue_selection(app, ui, |_| Err("injected picker failure".into()));
            }
            2 => {
                if app.borrow().local_dialog
                    || app.borrow().feedback
                        != messages::text(Message::LocalFailed, app.borrow().en())
                    || !ui.get_workbench()
                {
                    return Err("Local picker error is not retryable".into());
                }
                local_image::queue_selection(app, ui, |_| panic!("Hidden queued local picker"));
                ui.hide().map_err(|e| e.to_string())?;
            }
            3 => {
                if app.borrow().local_dialog || ui.window().is_visible() {
                    return Err("Hidden queued local picker reopened or remained busy".into());
                }
                ui.invoke_action("workbench".into());
                let path = dir.join("deferred-local.png");
                image::save_buffer_with_format(
                    &path,
                    &[27, 83, 145, 255].repeat(80 * 60),
                    80,
                    60,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                local_image::queue_selection(app, ui, move |_| Ok(Some(path)));
                self.started = Some(Instant::now());
            }
            4 => {
                let capture = app
                    .borrow()
                    .capture
                    .as_ref()
                    .ok_or("Deferred selection did not open PNG")?
                    .clone_strong();
                if app.borrow().local_dialog
                    || capture.get_wide() != 80.
                    || capture.get_high() != 60.
                {
                    return Err("Deferred selection opened wrong image geometry".into());
                }
                capture.invoke_action("cancel".into());
            }
            5 => {
                if app.borrow().capture.is_some() {
                    return Err("Deferred local editor cancel did not release capture".into());
                }
                ui.invoke_action("workbench".into());
                let path = dir.join("deferred-local.png");
                local_image::queue_selection(app, ui, move |ui| {
                    ui.hide().map_err(|e| e.to_string())?;
                    Ok(Some(path))
                });
            }
            6 => {
                if app.borrow().local_dialog
                    || ui.window().is_visible()
                    || app.borrow().capture.is_some()
                    || app.borrow().local_path.is_some()
                {
                    return Err("Late local choice reopened a hidden owner".into());
                }
                ui.invoke_action("workbench".into());
                let shared = app.clone();
                let path = dir.join("deferred-local.png");
                local_image::queue_selection(app, ui, move |_| {
                    // Model a session transition at the OS dialog boundary.
                    let mut state = shared.borrow_mut();
                    state.lifecycle.generation = state.lifecycle.generation.wrapping_add(1);
                    Ok(Some(path))
                });
            }
            7 => {
                if app.borrow().local_dialog
                    || app.borrow().capture.is_some()
                    || app.borrow().local_path.is_some()
                    || app.borrow().session.preferences.draft != *self.draft.as_ref().unwrap()
                    || config_bytes(dir)? != self.disk
                    || format!("{:?}", app.borrow().last_region) != self.region
                {
                    return Err(
                        "Stale session choice changed editor/draft/disk/repeat region".into(),
                    );
                }
                app.borrow_mut().local_path = self.old_path.take();
                app.borrow_mut().feedback = self.feedback.clone();
                ui.invoke_action("close".into());
                eprintln!(
                    "Local dialog: deferred/duplicate/cancel/failure retry, nested capture+quit guards/rescue+hotkey-error recovery, hidden queued+late owner, stale session rejection, actual selected PNG decode/editor cancel and draft/disk/repeat-region isolation passed; picker results injected, physical file selection not exercised"
                );
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}
