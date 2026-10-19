//! Synthetic picker results exercise the actual draft boundary; native COM has separate tests.
use crate::{output_settings as output, resident::AppState, settings::SettingsWindow};
use glint_core::config::Config;
use slint::ComponentHandle;
use std::{cell::RefCell, path::Path, rc::Rc};

#[derive(Default)]
pub struct Run {
    old: Option<Config>,
    bytes: Vec<u8>,
    scrolled: bool,
}
impl Run {
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
                self.bytes = std::fs::read(dir.join("config.toml")).unwrap_or_default();
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
                ui.window().set_size(slint::PhysicalSize::new(900, 650));
                ui.invoke_change("general.language".into(), "en".into());
                ui.invoke_change(
                    "output.filename".into(),
                    "Shot_{date}_{time}_{width}x{height}".into(),
                );
                ui.invoke_change("output.format".into(), "jpeg".into());
                if !ui
                    .get_filename_preview()
                    .ends_with("Shot_2026-10-18_143205_1280x720.jpg")
                {
                    return Err("Filename preview differs from real naming logic".into());
                }
                let before = app.borrow().session.preferences.draft.clone();
                if !output::begin(&mut app.borrow_mut(), ui)
                    || output::begin(&mut app.borrow_mut(), ui)
                    || !ui.get_folder_busy()
                {
                    return Err("Folder picker was not exclusive".into());
                }
                output::complete(&mut app.borrow_mut(), ui, Ok(None));
                if app.borrow().session.preferences.draft != before || ui.get_folder_busy() {
                    return Err("Cancel changed output settings".into());
                }
                output::begin(&mut app.borrow_mut(), ui);
                output::complete(
                    &mut app.borrow_mut(),
                    ui,
                    Err("injected native folder error".into()),
                );
                if app.borrow().session.preferences.draft != before || ui.get_status().is_empty() {
                    return Err("Folder failure lost draft or feedback".into());
                }
                output::begin(&mut app.borrow_mut(), ui);
                output::complete(
                    &mut app.borrow_mut(),
                    ui,
                    Ok(Some(Path::new("relative/path").to_owned())),
                );
                if app.borrow().session.preferences.draft != before {
                    return Err("Invalid relative folder was accepted".into());
                }
                let selected = dir.join("output-folder-selected");
                if selected.exists() {
                    return Err("Folder fixture already exists".into());
                }
                output::begin(&mut app.borrow_mut(), ui);
                output::complete(&mut app.borrow_mut(), ui, Ok(Some(selected.clone())));
                if app.borrow().session.preferences.draft.output.directory
                    != selected.to_str().unwrap()
                    || selected.exists()
                    || app.borrow().session.preferences.saved() != self.old.as_ref().unwrap()
                {
                    return Err("Selected directory was not draft-only or created a folder".into());
                }
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(650., 400.),
                        delta_x: 0.,
                        delta_y: -10000.,
                    });
            }
            1 => {
                if !self.scrolled {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(650., 400.),
                            delta_x: 0.,
                            delta_y: -10000.,
                        });
                    self.scrolled = true;
                    return Ok(false);
                }
                super::snapshot(ui, &dir.join("output-settings-preview-en.png"))?;
                ui.invoke_action("cancel".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Cancel settings did not restore folder/template".into());
                }
                if std::fs::read(dir.join("config.toml")).unwrap_or_default() != self.bytes {
                    return Err("Folder selection wrote configuration without Save".into());
                }
                output::begin(&mut app.borrow_mut(), ui);
                ui.invoke_action("cancel".into());
                output::complete(
                    &mut app.borrow_mut(),
                    ui,
                    Ok(Some(dir.join("canceled-result"))),
                );
                if app.borrow().session.preferences.is_dirty() || ui.get_folder_busy() {
                    return Err("Canceled picker accepted a late result".into());
                }
                output::begin(&mut app.borrow_mut(), ui);
                ui.invoke_change("output.directory".into(), "C:/manual-newer-value".into());
                output::complete(
                    &mut app.borrow_mut(),
                    ui,
                    Ok(Some(dir.join("stale-result"))),
                );
                if app.borrow().session.preferences.draft.output.directory
                    != "C:/manual-newer-value"
                {
                    return Err("Picker overwrote a newer directory edit".into());
                }
                ui.invoke_action("cancel".into());
                app.borrow_mut().session.writable = false;
                app.borrow().sync(ui);
                if output::begin(&mut app.borrow_mut(), ui) {
                    return Err("Read-only config opened a writable folder chooser".into());
                }
                app.borrow_mut().session.writable = true;
                app.borrow().sync(ui);
                let selected = dir.join("output-folder-saved");
                output::begin(&mut app.borrow_mut(), ui);
                output::complete(&mut app.borrow_mut(), ui, Ok(Some(selected.clone())));
                ui.invoke_action("save".into());
                let saved = glint_core::config::Config::from_toml(
                    &std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if saved.output.directory != selected.to_str().unwrap()
                    || selected.exists()
                    || app.borrow().session.preferences.is_dirty()
                {
                    return Err("Saving selected folder failed or eagerly created it".into());
                }
                ui.invoke_change("general.language".into(), "zh_cn".into());
                ui.window().set_size(slint::PhysicalSize::new(650, 500));
                self.scrolled = false;
            }
            2 => {
                if !self.scrolled {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(400., 250.),
                            delta_x: 0.,
                            delta_y: -10000.,
                        });
                    self.scrolled = true;
                    return Ok(false);
                }
                super::snapshot(ui, &dir.join("output-settings-preview-zh-compact.png"))?;
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                        position: slint::LogicalPosition::new(400., 250.),
                        delta_x: 0.,
                        delta_y: 380.,
                    });
            }
            3 => {
                super::snapshot(ui, &dir.join("output-settings-folder-zh-compact.png"))?;
                ui.invoke_action("cancel".into());
                output::begin(&mut app.borrow_mut(), ui);
                ui.invoke_action("close".into());
                output::complete(
                    &mut app.borrow_mut(),
                    ui,
                    Ok(Some(dir.join("closed-result"))),
                );
                if ui.window().is_visible() || app.borrow().session.preferences.is_dirty() {
                    return Err("Late folder completion reopened or changed settings".into());
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Output settings fixture did not restore config".into());
                }
                ui.invoke_action("open".into());
                ui.invoke_choose_page(5);
            }
            4 => {
                // Cancel the real queued action before its modal loop begins.
                ui.invoke_action("output-folder".into());
                if !ui.get_folder_busy() {
                    return Err("Folder action did not reserve its modal slot".into());
                }
                ui.invoke_action("close".into());
            }
            5 => {
                if ui.get_folder_busy()
                    || ui.window().is_visible()
                    || app.borrow().session.preferences.is_dirty()
                {
                    return Err(
                        "Queued canceled folder action remained busy or reopened settings".into(),
                    );
                }
                eprintln!(
                    "Output settings: synthetic native-choice boundary validates cancel/failure/retry/absolute-path/draft/TOML/read-only/stale+late isolation; real queued-action cancellation and filename-token preview PNG/JPEG plus bilingual compact layout passed; physical folder selection not exercised"
                );
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}
