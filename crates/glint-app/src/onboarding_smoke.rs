//! Guide verification through native callbacks, real clipboard and configuration I/O.
use crate::{resident::AppState, settings::SettingsWindow};
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Run {
    old: Option<glint_core::config::Config>,
    locked: Option<std::fs::File>,
    blocker: Option<glint_platform::ClipboardBlocker>,
    started: Option<Instant>,
}
impl Run {
    fn lock(&mut self, directory: &Path) -> Result<(), String> {
        use std::os::windows::fs::OpenOptionsExt;
        self.locked = Some(
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(directory.join("config.toml"))
                .map_err(|e| e.to_string())?,
        );
        Ok(())
    }
    fn wait(&self) -> Result<bool, String> {
        if self
            .started
            .is_some_and(|time| time.elapsed() > Duration::from_secs(20))
        {
            Err("Guide capture timed out".into())
        } else {
            Ok(false)
        }
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        hotkeys: &glint_platform::HotkeyService,
        directory: &Path,
    ) -> Result<bool, String> {
        let capture = app.borrow().capture.as_ref().map(|ui| ui.clone_strong());
        if [1, 3].contains(&stage) && capture.is_none() {
            return self.wait();
        }
        if [2, 6].contains(&stage) && (capture.is_some() || app.borrow().capturing) {
            return self.wait();
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                ui.invoke_action("guide-open".into());
                if ui.get_guide_step() != 0
                    || !app
                        .borrow()
                        .rows
                        .iter()
                        .any(|r| r.key == "general.language")
                {
                    return Err("Guide welcome/language missing".into());
                }
                ui.invoke_change("general.language".into(), "en".into());
                let theme = if app.borrow().session.preferences.saved().appearance.theme
                    == glint_core::config::Theme::Dark
                {
                    "light"
                } else {
                    "dark"
                };
                ui.invoke_change("appearance.theme".into(), theme.into());
                if !ui.get_guide_title().contains("Capture and carry on") {
                    return Err("Guide language override not live".into());
                }
                ui.invoke_action("close".into());
                if ui.get_modal() != 1 {
                    return Err("Guide skip discarded dirty draft".into());
                }
                ui.invoke_action("continue".into());
                ui.invoke_action("guide-next".into());
                ui.invoke_record("shortcuts.capture".into());
                ui.invoke_shortcut_input("\u{f716}".into(), true, true, true, false);
                ui.invoke_action("accept-key".into());
                super::snapshot(ui, &directory.join("guide-shortcuts-en.png"))?;
                ui.invoke_action("guide-next".into());
                ui.invoke_change("output.finish".into(), "copy".into());
                ui.invoke_change("output.auto_save".into(), "false".into());
                ui.invoke_action("guide-next".into());
                ui.invoke_change("appearance.theme".into(), "dark".into());
                super::snapshot(ui, &directory.join("guide-appearance-dark-en.png"))?;
                ui.invoke_action("guide-next".into());
                self.lock(directory)?;
                ui.invoke_action("guide-practice".into());
                if app.borrow().guide_practice
                    || app.borrow().capturing
                    || !app.borrow().session.preferences.is_dirty()
                {
                    return Err("Failed guide save began acquisition or discarded draft".into());
                }
                self.locked.take();
                let occupied = glint_platform::HotkeyService::new(
                    vec![glint_platform::HotkeyBinding {
                        action: 77,
                        modifiers: 7,
                        key: 0x83,
                    }],
                    |_| {},
                )?;
                ui.invoke_change("shortcuts.capture".into(), "Ctrl+Alt+Shift+F20".into());
                ui.invoke_action("guide-practice".into());
                if app.borrow().guide_practice
                    || app.borrow().capturing
                    || !app.borrow().session.preferences.is_dirty()
                {
                    return Err("Guide hotkey conflict began capture or discarded draft".into());
                }
                drop(occupied);
                ui.invoke_change("shortcuts.capture".into(), "Ctrl+Alt+Shift+F19".into());
                ui.invoke_change("capture.delay_seconds".into(), "10".into());
                ui.invoke_action("guide-practice".into());
                self.started = Some(Instant::now());
            }
            1 => {
                if app.borrow().countdown.is_some() || !app.borrow().guide_practice {
                    return Err("Practice inherited delay or lost return route".into());
                }
                if app
                    .borrow()
                    .session
                    .preferences
                    .saved()
                    .general
                    .onboarding_complete
                {
                    return Err("Practice prematurely completed guide".into());
                }
                capture.unwrap().invoke_action("cancel".into());
                self.started = Some(Instant::now());
            }
            2 => {
                if ui.get_guide_step() != 4 || app.borrow().guide_practice {
                    return Err("Practice cancel did not return to guide".into());
                }
                super::snapshot(ui, &directory.join("guide-practice-en.png"))?;
                ui.invoke_action("guide-practice".into());
                self.started = Some(Instant::now());
            }
            3 => {
                let c = capture.unwrap();
                c.set_tool(0);
                c.invoke_pointer(0, 350., 250.);
                c.invoke_pointer(2, 750., 550.);
                c.set_tool(1);
                c.invoke_pointer(0, 400., 310.);
                c.invoke_pointer(2, 550., 380.);
                self.blocker = Some(glint_platform::ClipboardBlocker::acquire()?);
                c.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            4 => {
                let c = capture.ok_or("Guide copy failure closed editor")?;
                if c.get_busy() {
                    return self.wait();
                }
                if c.get_closed() || c.get_object_count() != 1 || app.borrow().guide_succeeded {
                    return Err("Guide failure lost annotations or reported success".into());
                }
                super::capture_snapshot(&c, &directory.join("guide-copy-error.png"))?;
                self.blocker.take();
                c.invoke_action("copy".into());
                self.started = Some(Instant::now());
            }
            5 => {
                if capture.is_some() {
                    return self.wait();
                }
            }
            6 => {
                if ui.get_guide_step() != 5 || app.borrow().guide_practice {
                    return Err("Successful guide capture did not reach done".into());
                }
                super::snapshot(ui, &directory.join("guide-done-en.png"))?;
                self.lock(directory)?;
                ui.invoke_action("guide-complete".into());
                if ui.get_guide_step() != 5
                    || app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .general
                        .onboarding_complete
                    || app
                        .borrow()
                        .session
                        .preferences
                        .draft
                        .general
                        .onboarding_complete
                {
                    return Err("Failed completion persisted marker or closed guide".into());
                }
                self.locked.take();
                ui.invoke_action("guide-complete".into());
                if ui.get_guide_step() != -1
                    || !app
                        .borrow()
                        .session
                        .preferences
                        .saved()
                        .general
                        .onboarding_complete
                {
                    return Err("Guide completion was not accepted".into());
                }
                let reopened =
                    crate::preferences::SettingsSession::open(&directory.join("config.toml"));
                if crate::onboarding::initial(&reopened, false, false) {
                    return Err("Guide completion did not survive reload".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_change("appearance.theme".into(), "light".into());
                ui.invoke_action("guide-open".into());
                if ui.get_modal() != 1 {
                    return Err("Reopen guide ignored settings draft".into());
                }
                ui.invoke_action("discard-leave".into());
                if ui.get_guide_step() != 0 || app.borrow().session.preferences.is_dirty() {
                    return Err("Reopen guide did not follow discard choice".into());
                }
                ui.invoke_change("general.language".into(), "zh_cn".into());
                super::snapshot(ui, &directory.join("guide-welcome-zh.png"))?;
                ui.invoke_action("close".into());
                ui.invoke_action("discard-leave".into());
                if ui.get_guide_step() != -1 {
                    return Err("Guide skip did not return to tray".into());
                }
                let previous = hotkeys.bindings()?;
                let capture_key = *previous
                    .iter()
                    .find(|binding| binding.action == 1)
                    .ok_or("Guide capture binding missing")?;
                hotkeys.replace(vec![])?;
                let occupied = glint_platform::HotkeyService::new(vec![capture_key], |_| {})?;
                ui.invoke_action("guide-open".into());
                ui.invoke_action("close".into());
                if ui.get_guide_step() != -1 || !hotkeys.bindings()?.is_empty() {
                    return Err(
                        "Clean guide skip attempted to replace unavailable runtime bindings".into(),
                    );
                }
                drop(occupied);
                hotkeys.replace(previous)?;
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                ui.invoke_action("open".into());
                eprintln!(
                    "Native guide: six steps, language/theme preview, dirty skip/reopen, actual config lock/rollback/retry, real capture/cancel/clipboard failure/retry/done and persistent completion passed"
                );
            }
            _ => return Err("Unknown guide smoke stage".into()),
        }
        Ok(true)
    }
}
