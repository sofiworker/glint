//! Opt-in effective-policy display verification, including preserved imported off values.
use crate::{resident::AppState, settings::SettingsWindow};
use slint::{ComponentHandle, Model};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

struct Run {
    ui: slint::Weak<SettingsWindow>,
    app: Rc<RefCell<AppState>>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    original: glint_core::config::Config,
    saved: glint_core::config::Config,
    file: Option<Vec<u8>>,
    scene: usize,
    snapshot: bool,
}
const SCENES: &[(usize, bool)] = &[
    (0, false),
    (4, false),
    (5, false),
    (9, false),
    (0, true),
    (4, true),
    (5, true),
    (9, true),
];
const FIELDS: &[(usize, &str)] = &[
    (0, "general.close_to_tray"),
    (0, "general.single_instance"),
    (4, "annotation.keep_objects"),
    (5, "output.strip_metadata"),
    (9, "resources.idle_pause"),
];
pub fn install(
    ui: &SettingsWindow,
    app: Rc<RefCell<AppState>>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
) {
    let original = app.borrow().session.preferences.draft.clone();
    let saved = app.borrow().session.preferences.saved().clone();
    let file = std::fs::read(directory.join("config.toml")).ok();
    let run = Rc::new(RefCell::new(Run {
        ui: ui.as_weak(),
        app,
        directory,
        error,
        original,
        saved,
        file,
        scene: 0,
        snapshot: false,
    }));
    slint::Timer::single_shot(Duration::from_millis(600), move || tick(run));
}
impl Run {
    fn step(&mut self) -> Result<bool, String> {
        let ui = self.ui.upgrade().ok_or("Settings owner closed")?;
        if self.scene == SCENES.len() {
            self.app.borrow_mut().session.preferences.draft = self.original.clone();
            self.app.borrow().sync(&ui);
            ui.invoke_action("cancel".into());
            let state = self.app.borrow();
            if state.session.preferences.saved() != &self.saved
                || state.session.preferences.draft != self.original
                || std::fs::read(self.directory.join("config.toml")).ok() != self.file
                || state.capture.is_some()
                || state.capturing
            {
                return Err("Policy display mutated saved/draft/file/capture state".into());
            }
            drop(state);
            eprintln!(
                "Native fixed settings: imported off values, effective checked/disabled controls, rejected edits, bilingual layouts and configuration isolation passed"
            );
            ui.invoke_action("quit".into());
            return Ok(true);
        }
        let (category, english) = SCENES[self.scene];
        if !self.snapshot {
            let preview=glint_core::config_transfer::preview(&self.original,
                "config_version=1\n[general]\nclose_to_tray=false\nsingle_instance=false\n[annotation]\nkeep_objects=false\n[output]\nstrip_metadata=false\n[resources]\nidle_pause=false\n").map_err(|e|e.to_string())?;
            {
                let mut state = self.app.borrow_mut();
                state.session.preferences.draft = preview.candidate;
                state.session.preferences.draft.general.language = if english {
                    glint_core::config::Language::En
                } else {
                    glint_core::config::Language::ZhCn
                };
            }
            ui.invoke_action("open".into());
            ui.invoke_choose_page(category as i32);
            ui.window().set_size(slint::PhysicalSize::new(900, 650));
            let before = self.app.borrow().session.preferences.draft.clone();
            for (_, key) in FIELDS.iter().filter(|(c, _)| *c == category) {
                ui.invoke_change((*key).into(), "true".into());
                let row = self
                    .app
                    .borrow()
                    .rows
                    .iter()
                    .find(|r| r.key == *key)
                    .ok_or("Policy row missing")?;
                if !row.checked
                    || row.editable
                    || !row.description.contains(if english {
                        "configured off value has no effect"
                    } else {
                        "关闭值当前不生效"
                    })
                {
                    return Err(format!("Wrong effective policy row: {key}"));
                }
            }
            if self.app.borrow().session.preferences.draft != before {
                return Err("Disabled policy edit changed draft".into());
            }
            self.snapshot = true;
        } else {
            ui.window()
                .dispatch_event_with_result(slint::platform::WindowEvent::PointerScrolled {
                    position: slint::LogicalPosition::new(if english { 500. } else { 650. }, 350.),
                    delta_x: 0.,
                    delta_y: if category == 9 { 10000. } else { -10000. },
                })
                .map_err(|e| e.to_string())?;
            let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
            image::save_buffer_with_format(
                self.directory.join(format!(
                    "policy-{category}-{}.png",
                    if english { "en" } else { "zh" }
                )),
                pixels.as_bytes(),
                pixels.width(),
                pixels.height(),
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .map_err(|e| e.to_string())?;
            self.scene += 1;
            self.snapshot = false;
        }
        Ok(false)
    }
}
fn tick(run: Rc<RefCell<Run>>) {
    let result = run.borrow_mut().step();
    match result {
        Ok(false) => slint::Timer::single_shot(Duration::from_millis(100), move || tick(run)),
        Ok(true) => {}
        Err(error) => {
            *run.borrow().error.borrow_mut() = Some(format!("Settings policy smoke: {error}"));
            let _ = slint::quit_event_loop();
        }
    }
}
