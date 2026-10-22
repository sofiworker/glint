//! Explicit input-probe feature: real SendInput -> WM_HOTKEY -> Slint pointer/key paths.
//! Capture uses real input; settings setup, pending import, later recorder reopens and cleanup use callbacks.
use crate::{probe::CaptureProbe, resident::AppState, settings::SettingsWindow};
use glint_platform::input_probe::{DragKey, InputProbe, Pointer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

glint_ui::slint! {
    export component InputFixture inherits Window {
        title: "Glint owned input verification";
        width: 640px; height: 400px; no-frame: true; always-on-top: true;
        background: #426a89;
        Text {x:20px;y:20px;text:"Glint input verification";color:white;}
    }
}
fn id<C: ComponentHandle>(ui: &C) -> Result<usize, String> {
    ui.window()
        .with_winit_window(|w| {
            let raw = w.window_handle().map_err(|e| e.to_string())?;
            match raw.as_raw() {
                RawWindowHandle::Win32(h) => Ok(h.hwnd.get() as usize),
                _ => Err("Input probe needs Win32".into()),
            }
        })
        .ok_or("Native input peer not ready")?
}
fn logical<C: ComponentHandle>(ui: &C, x: f32, y: f32) -> (i32, i32) {
    let scale = ui.window().scale_factor();
    ((x * scale).round() as i32, (y * scale).round() as i32)
}
struct Run {
    ui: slint::Weak<SettingsWindow>,
    app: Rc<RefCell<AppState>>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    old: glint_core::config::Config,
    fixture: InputFixture,
    input: Option<InputProbe>,
    phase: u16,
    navigation_target: i32,
    draft_checked: bool,
    guide_checked: bool,
    toolbar_walk_checked: bool,
    toolbar_walk_tabs: usize,
    guide_saved: Option<glint_core::config::Config>,
    guide_disk: Option<Vec<u8>>,
    guide_accent: String,
    exit_editor: Option<CaptureProbe>,
    since: Instant,
    rect: glint_core::Rect,
    expected: Vec<u8>,
    pin: Option<u64>,
    pin_position: (i32, i32),
    region: String,
    settings_draft: Option<glint_core::config::Config>,
    settings_file: Option<Vec<u8>>,
    settings_saved: Option<glint_core::config::Config>,
    dialog_mode: bool,
    conflict_mode: bool,
    dialog_worker: Option<std::thread::JoinHandle<(InputProbe, Result<(), String>)>>,
}
pub fn install(
    ui: &SettingsWindow,
    app: Rc<RefCell<AppState>>,
    directory: PathBuf,
    error: Rc<RefCell<Option<String>>>,
    dialog_mode: bool,
    conflict_mode: bool,
) -> Result<(), String> {
    let fixture = InputFixture::new().map_err(|e| e.to_string())?;
    let old = app.borrow().session.preferences.saved().clone();
    let run = Rc::new(RefCell::new(Run {
        ui: ui.as_weak(),
        app,
        directory,
        error,
        old,
        fixture,
        input: Some(InputProbe::new()?),
        phase: 0,
        navigation_target: 0,
        draft_checked: false,
        guide_checked: false,
        toolbar_walk_checked: false,
        toolbar_walk_tabs: 0,
        guide_saved: None,
        guide_disk: None,
        guide_accent: String::new(),
        exit_editor: None,
        since: Instant::now(),
        rect: Default::default(),
        expected: vec![],
        pin: None,
        pin_position: (0, 0),
        region: String::new(),
        settings_draft: None,
        settings_file: None,
        settings_saved: None,
        dialog_mode,
        conflict_mode,
        dialog_worker: None,
    }));
    slint::Timer::single_shot(Duration::from_millis(700), move || tick(run));
    Ok(())
}
fn tick(run: Rc<RefCell<Run>>) {
    let result = run.borrow_mut().step();
    match result {
        Ok(true) => {}
        Ok(false) => slint::Timer::single_shot(Duration::from_millis(120), move || tick(run)),
        Err(message) => {
            let mut run = run.borrow_mut();
            run.input.take(); // Release only our own injected held inputs before closing any windows.
            let _ = run.fixture.hide();
            if let Some(c) = run.app.borrow().capture.as_ref()
                && let Ok(pixels) = c.window().take_snapshot()
            {
                let _ = image::save_buffer(
                    run.directory.join("input-failed.png"),
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                );
            }
            if let Some(ui) = run.ui.upgrade() {
                let _ = run.settings_snapshot(&ui);
            }
            *run.error.borrow_mut() = Some(format!("Native input phase {}: {message}", run.phase));
            let _ = slint::quit_event_loop();
        }
    }
}
impl Run {
    fn navigation_snapshot(&self, name: &str) -> Result<(), String> {
        let state = self.app.borrow();
        for (kind, config) in [
            ("draft", &state.session.preferences.draft),
            ("saved", state.session.preferences.saved()),
        ] {
            std::fs::write(
                self.directory
                    .join(format!("navigation-{name}-{kind}.toml")),
                toml::to_string(config).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        let disk = std::fs::read(self.directory.join("config.toml")).map_err(|e| e.to_string())?;
        std::fs::write(
            self.directory.join(format!("navigation-{name}-disk.toml")),
            disk,
        )
        .map_err(|e| e.to_string())
    }
    fn toolbar_draft_observation(&self, c: &CaptureProbe, name: &str) -> Result<(), String> {
        let rows = c.get_toolbar_rows();
        let rows = (0..rows.row_count())
            .map(|i| {
                let r = rows.row_data(i).unwrap();
                format!("{}:{}", r.tool, r.shown)
            })
            .collect::<Vec<_>>()
            .join(",");
        let saved = toml::to_string(self.app.borrow().session.preferences.saved())
            .map_err(|e| e.to_string())?;
        let disk = std::fs::read(self.directory.join("config.toml")).map_err(|e| e.to_string())?;
        if name != "before"
            && (std::fs::read(self.directory.join("toolbar-draft-before.toml"))
                .ok()
                .as_deref()
                != Some(saved.as_bytes())
                || std::fs::read(self.directory.join("toolbar-disk-before.toml")).ok()
                    != Some(disk.clone()))
        {
            return Err(
                "Toolbar keyboard draft modified saved preferences or disk before Apply".into(),
            );
        }
        std::fs::write(
            self.directory.join(format!("toolbar-draft-{name}.toml")),
            saved,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            self.directory.join(format!("toolbar-disk-{name}.toml")),
            disk,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(self.directory.join(format!("toolbar-controls-{name}.txt")),format!("icons={}\ngroups={}\nrows={rows}\nobjects={}\nselected={}\ncan_undo={}\nmodal={}\n",c.get_toolbar_icons(),c.get_toolbar_groups(),c.get_object_count(),c.get_selected(),c.get_can_undo(),c.get_toolbar_modal())).map_err(|e|e.to_string())?;
        Ok(())
    }
    fn assert_capture_exit(
        &mut self,
        name: &str,
        expected: crate::probe::CaptureExit,
    ) -> Result<bool, String> {
        if self.capture().is_some() || self.app.borrow().capturing {
            return self.waiting();
        }
        let c = self
            .exit_editor
            .take()
            .ok_or("Capture exit observation lost editor")?;
        let foreground = self
            .input
            .as_ref()
            .unwrap()
            .foreground(id(&self.fixture)?)?;
        if !c.get_closed()
            || c.get_busy()
            || c.get_completion_kind() != 0
            || c.get_exit_reason() != expected
            || !foreground
        {
            return Err(format!(
                "Real capture exit {name} mismatched: reason={:?} closed={} busy={} completion={} fixture_foreground={foreground}",
                c.get_exit_reason(),
                c.get_closed(),
                c.get_busy(),
                c.get_completion_kind()
            ));
        }
        std::fs::write(self.directory.join(format!("capture-exit-input-{name}.json")),format!("{{\"reason\":\"{:?}\",\"closed\":true,\"busy\":false,\"completion\":0,\"fixture_foreground\":true}}\n",c.get_exit_reason())).map_err(|e|e.to_string())?;
        Ok(true)
    }
    fn guide_next_click(&mut self, ui: &SettingsWindow) -> Result<(), String> {
        let size = ui.window().size();
        let scale = ui.window().scale_factor();
        self.settings_click(
            ui,
            size.width as f32 / scale - 90.,
            size.height as f32 / scale - 34.,
            false,
        )
    }
    fn guide_close_click(&mut self, ui: &SettingsWindow) -> Result<(), String> {
        let size = ui.window().size();
        let scale = ui.window().scale_factor();
        self.settings_click(ui, size.width as f32 / scale - 110., 45., false)
    }
    fn guide_color_click(&mut self, ui: &SettingsWindow) -> Result<(), String> {
        let row = self
            .app
            .borrow()
            .rows
            .iter()
            .find(|row| row.key == "appearance.accent")
            .ok_or("Guide accent row missing")?;
        let index = (row.selected as usize + 1) % row.colors.row_count();
        let color = row
            .colors
            .row_data(index)
            .ok_or("Guide alternate accent missing")?;
        self.guide_accent = format!(
            "#{:02x}{:02x}{:02x}",
            color.red(),
            color.green(),
            color.blue()
        );
        // Find the rendered alternate circle, rather than guessing form/scroll geometry.
        let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
        let (w, h) = (pixels.width() as usize, pixels.height() as usize);
        if ui.window().size() != slint::PhysicalSize::new(w as u32, h as u32) {
            return Err("Guide swatch snapshot differs from physical client size".into());
        }
        let hit = (100..h - 100)
            .find_map(|y| {
                (12..w - 12).find_map(|x| {
                    [-8isize, 0, 8]
                        .into_iter()
                        .all(|dy| {
                            [-8isize, 0, 8].into_iter().all(|dx| {
                                let p = pixels.as_slice()
                                    [(y.wrapping_add_signed(dy)) * w + x.wrapping_add_signed(dx)];
                                p.r == color.red()
                                    && p.g == color.green()
                                    && p.b == color.blue()
                                    && p.a == 255
                            })
                        })
                        .then_some((x, y))
                })
            })
            .ok_or("Rendered alternate accent circle not found")?;
        std::fs::write(
            self.directory.join("guide-swatch-target.json"),
            format!(
                "{{\"x\":{},\"y\":{},\"accent\":\"{}\",\"index\":{}}}\n",
                hit.0, hit.1, self.guide_accent, index
            ),
        )
        .map_err(|e| e.to_string())?;
        self.input.as_mut().unwrap().pointer(
            id(ui)?,
            hit.0 as i32,
            hit.1 as i32,
            Pointer::Click,
            false,
        )
    }
    fn draft_observation(&self, c: &CaptureProbe, count: i32) -> Result<(), String> {
        if c.get_object_count() != count
            || c.get_busy()
            || c.get_left() != self.rect.x as f32
            || c.get_top() != self.rect.y as f32
            || c.get_wide() != 480.
            || c.get_high() != 320.
        {
            return Err(format!(
                "Held draft phase {} changed scene/crop or started output",
                self.phase
            ));
        }
        let pixels = c.window().take_snapshot().map_err(|e| e.to_string())?;
        image::save_buffer(
            self.directory
                .join(format!("held-draft-{}.png", self.phase)),
            pixels.as_bytes(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(self.directory.join(format!("held-draft-{}.json",self.phase)),format!("{{\"phase\":{},\"objects\":{},\"undo\":{},\"redo\":{},\"tool\":{},\"x\":{},\"y\":{},\"width\":480,\"height\":320,\"source_width\":{},\"source_height\":{}}}\n",self.phase,count,c.get_can_undo(),c.get_can_redo(),c.get_tool(),self.rect.x,self.rect.y,c.get_source_w(),c.get_source_h())).map_err(|e|e.to_string())
    }
    fn start_dialog(&mut self, c: &CaptureProbe, save: bool) -> Result<(), String> {
        let owner = id(c)?;
        let path = if save {
            let path = self.directory.join("input-拾光.png");
            let path = path.to_str().ok_or("QA path is not Unicode")?;
            Some(path.strip_prefix(r"\\?\").unwrap_or(path).to_owned())
        } else {
            None
        };
        self.toolbar(c, 11)?;
        let mut input = self.input.take().ok_or("Input worker already active")?;
        let conflict = self.conflict_mode && save;
        let approve = self.phase != 32;
        let change_fixture = self.phase == 33;
        let fixture_path = self.directory.join("input-拾光.png");
        self.dialog_worker = Some(std::thread::spawn(move || {
            let result = input.save_dialog(owner, path.as_deref()).and_then(|()| {
                if !conflict {
                    return Ok(());
                }
                input.replacement_confirmation(owner, "Confirm replacement", approve, || {
                    if change_fixture {
                        std::fs::write(
                            fixture_path,
                            b"QA destination changed after replacement prompt observation",
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    Ok(())
                })
            });
            eprintln!("Owned Save As input result: {result:?}");
            (input, result)
        }));
        Ok(())
    }
    fn dialog_done(&mut self) -> Result<bool, String> {
        let worker = self.dialog_worker.as_ref().ok_or("No dialog worker")?;
        if !worker.is_finished() {
            return Ok(false);
        }
        let (input, result) = self
            .dialog_worker
            .take()
            .unwrap()
            .join()
            .map_err(|_| "Dialog worker panicked")?;
        self.input = Some(input);
        result?;
        Ok(true)
    }
    fn finish(&mut self, ui: &SettingsWindow) -> Result<(), String> {
        self.input.take();
        let _ = self.fixture.hide();
        self.app.borrow_mut().session.preferences.draft = self.old.clone();
        ui.invoke_action("save".into());
        if self.app.borrow().session.preferences.is_dirty() {
            return Err("Input probe could not restore its isolated bindings".into());
        }
        Ok(())
    }
    fn waiting(&self) -> Result<bool, String> {
        if self.since.elapsed() > Duration::from_secs(20) {
            Err(format!(
                "Timed out waiting for OS input result; resident feedback: {}",
                self.app.borrow().feedback
            ))
        } else {
            Ok(false)
        }
    }
    fn capture(&self) -> Option<CaptureProbe> {
        self.app.borrow().capture.as_ref().map(|c| c.clone_strong())
    }
    fn pin(&self) -> Result<crate::pins::DesktopPin, String> {
        let state = self.app.borrow();
        let pins = state.pins.borrow();
        pins.window(self.pin.ok_or("No input-test pin")?)
            .map(|p| p.clone_strong())
            .ok_or("Input pin vanished".into())
    }
    fn fixture_click(&mut self) -> Result<(), String> {
        let id = id(&self.fixture)?;
        for (x, y) in [(30., 60.), (610., 60.), (610., 370.), (30., 370.)] {
            let (x, y) = logical(&self.fixture, x, y);
            if self.input.as_ref().unwrap().hits(id, x, y)? {
                return self
                    .input
                    .as_mut()
                    .unwrap()
                    .pointer(id, x, y, Pointer::Click, true);
            }
        }
        Err("Owned fixture is fully covered; no activation input sent".into())
    }
    fn hotkey(&mut self, key: u16) -> Result<(), String> {
        self.input
            .as_mut()
            .unwrap()
            .chord(id(&self.fixture)?, &[0x11, 0x12, 0x10, key])
    }
    fn assert_candidate(&self, ui: &SettingsWindow, expected: &str) -> Result<(), String> {
        if ui.get_modal() != 3 || !ui.get_can_accept() || self.app.borrow().candidate != expected {
            return Err("Shortcut recorder candidate/focus state differs".into());
        }
        self.assert_settings(ui)
    }
    fn assert_settings(&self, ui: &SettingsWindow) -> Result<(), String> {
        let state = self.app.borrow();
        if Some(&state.session.preferences.draft) != self.settings_draft.as_ref()
            || Some(state.session.preferences.saved()) != self.settings_saved.as_ref()
            || std::fs::read(self.directory.join("config.toml")).ok() != self.settings_file
            || state.capture.is_some()
            || state.capturing
        {
            return Err(
                "Settings confirmation changed draft, saved configuration, disk or capture".into(),
            );
        }
        std::fs::write(
            self.directory
                .join(format!("settings-modal-{}.txt", self.phase)),
            format!(
                "modal={}\ncancel_focused={}\naction_focused={}\ndraft_saved_disk_unchanged=true\n",
                ui.get_modal(),
                ui.get_modal_cancel_focused(),
                ui.get_modal_action_focused()
            ),
        )
        .map_err(|e| e.to_string())
    }
    fn settings_snapshot(&self, ui: &SettingsWindow) -> Result<(), String> {
        let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
        image::save_buffer(
            self.directory
                .join(format!("settings-modal-{}.png", self.phase)),
            pixels.as_bytes(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
    }
    fn settings_click(
        &mut self,
        ui: &SettingsWindow,
        x: f32,
        y: f32,
        activate: bool,
    ) -> Result<(), String> {
        let (x, y) = logical(ui, x, y);
        self.input
            .as_mut()
            .unwrap()
            .pointer(id(ui)?, x, y, Pointer::Click, activate)
    }
    fn source_pointer(
        &mut self,
        c: &CaptureProbe,
        x: u32,
        y: u32,
        action: Pointer,
    ) -> Result<(), String> {
        let size = c.window().size();
        let px =
            (f64::from(x) * f64::from(size.width) / f64::from(c.get_source_w())).round() as i32;
        let py =
            (f64::from(y) * f64::from(size.height) / f64::from(c.get_source_h())).round() as i32;
        self.input
            .as_mut()
            .unwrap()
            .pointer(id(c)?, px, py, action, false)
    }
    fn toolbar(&mut self, c: &CaptureProbe, catalog: i32) -> Result<(), String> {
        let items = c.get_toolbar_items();
        let index = (0..items.row_count())
            .find(|i| items.row_data(*i) == Some(catalog))
            .ok_or("Required toolbar item hidden")?;
        // Geometry mirrors the rendered toolbar grid; no activation callback is invoked.
        let width = c.get_toolbar_width();
        let columns = if c.get_toolbar_icons() {
            ((width - 14.) / 40.).floor() as usize
        } else if width >= 1070. {
            12
        } else if width >= 680. {
            7
        } else {
            4
        };
        let columns = columns.min(items.row_count()).max(1);
        let row = if width < 380. { 26. } else { 36. };
        let x = c.get_toolbar_x()
            + 7.
            + (index % columns) as f32 * (width - 14.) / columns as f32
            + ((width - 14.) / columns as f32 - 4.) / 2.;
        let y = c.get_toolbar_y() + 7. + (index / columns) as f32 * row + (row - 4.) / 2.;
        let (x, y) = logical(c, x, y);
        self.input
            .as_mut()
            .unwrap()
            .pointer(id(c)?, x, y, Pointer::Click, false)
    }
    fn start_pin_drag(&mut self, p: &crate::pins::DesktopPin) -> Result<(), String> {
        let peer = id(p)?;
        let mut input = self.input.take().ok_or("Pin drag input worker busy")?;
        self.dialog_worker = Some(std::thread::spawn(move || {
            let result = input.window_drag(peer, (40, 16), (60, 40));
            (input, result)
        }));
        Ok(())
    }
    fn pin_wheel(
        &mut self,
        p: &crate::pins::DesktopPin,
        delta: i32,
        control: bool,
        menu: bool,
    ) -> Result<(), String> {
        let (w, h) = Self::pin_size(p);
        let (x, y) = logical(p, w / 2., if menu { 150. } else { 34. + (h - 108.) / 2. });
        let peer = id(p)?;
        let mut input = self.input.take().ok_or("Pin wheel input worker busy")?;
        self.dialog_worker = Some(std::thread::spawn(move || {
            let result = input.wheel(peer, x, y, delta, control);
            (input, result)
        }));
        Ok(())
    }
    fn assert_pin_input(&self, p: &crate::pins::DesktopPin, turns: i32) -> Result<(), String> {
        let state = crate::pins::native(p, |n| n.state())?;
        let mut expected = vec![0u8; self.expected.len()];
        glint_core::pins::rotate_rgba(&self.expected, &mut expected, 480, 320, turns as u8)
            .map_err(str::to_owned)?;
        let (w, h) = if turns % 2 == 0 {
            (480, 320)
        } else {
            (320, 480)
        };
        let pixels = p
            .get_picture()
            .to_rgba8()
            .ok_or("Pin source pixels absent")?;
        std::fs::write(self.directory.join(format!("pin-observation-{}.txt",self.phase)),format!("x={}\ny={}\nexpected_x={}\nexpected_y={}\nno_activate={}\ntopmost={}\nclick_through={}\nforeground_retained={}\nturns={}\nsource_matches={}\nsource_width={}\nsource_height={}\n",state.x,state.y,self.pin_position.0,self.pin_position.1,state.no_activate,state.topmost,state.click_through,self.input.as_ref().unwrap().foreground(id(&self.fixture)?)?,p.get_quarter_turns(),pixels.as_bytes()==expected,pixels.width(),pixels.height())).map_err(|e|e.to_string())?;
        if state.x != self.pin_position.0
            || state.y != self.pin_position.1
            || !state.no_activate
            || !state.topmost
            || state.click_through
            || !self
                .input
                .as_ref()
                .unwrap()
                .foreground(id(&self.fixture)?)?
            || p.get_quarter_turns() != turns
            || pixels.width() != w
            || pixels.height() != h
            || pixels.as_bytes() != expected
        {
            return Err(
                "Pin gesture changed position/source pixels/native flags/previous foreground"
                    .into(),
            );
        }
        std::fs::write(self.directory.join(format!("pin-gesture-{}.txt",self.phase)),format!("x={}\ny={}\nzoom={}\nalpha={}\nlocked={}\nturns={}\nwidth={}\nheight={}\nforeground_retained=true\npixels_match=true\n",state.x,state.y,p.get_zoom(),p.get_alpha(),p.get_locked(),turns,w,h)).map_err(|e|e.to_string())
    }
    fn pin_click(&mut self, p: &crate::pins::DesktopPin, x: f32, y: f32) -> Result<(), String> {
        let (x, y) = logical(p, x, y);
        self.input
            .as_mut()
            .unwrap()
            .pointer(id(p)?, x, y, Pointer::Click, false)
    }
    fn pin_size(p: &crate::pins::DesktopPin) -> (f32, f32) {
        let s = p.window().size();
        let scale = p.window().scale_factor();
        (s.width as f32 / scale, s.height as f32 / scale)
    }
    fn step(&mut self) -> Result<bool, String> {
        let ui = self.ui.upgrade().ok_or("Input settings owner closed")?;
        if matches!(self.phase, 167 | 168 | 170 | 173 | 176 | 179 | 183 | 184)
            && self.input.is_none()
            && !self.dialog_done()?
        {
            return self.waiting();
        }
        match self.phase {
            0 => {
                for (key, value) in [
                    ("general.language", "en"),
                    ("capture.detect_window", "false"),
                    ("capture.delay_seconds", "0"),
                    ("capture.include_cursor", "false"),
                    ("annotation.tool", "select"),
                    ("toolbar.mode", "icons"),
                    ("output.finish", "copy"),
                    ("output.feedback", "none"),
                    ("output.auto_save", "false"),
                    ("pins.hide_on_capture", "true"),
                    ("pins.passthrough", "false"),
                    ("pins.snap", "false"),
                    ("pins.wheel", "zoom"),
                    ("pins.restore_session", "false"),
                    ("shortcuts.capture", "Ctrl+Alt+Shift+F21"),
                    ("shortcuts.pin_clipboard", "Ctrl+Alt+Shift+F22"),
                    ("shortcuts.rescue", "Ctrl+Alt+Shift+F23"),
                    ("shortcuts.toggle_pins", "Ctrl+Alt+Shift+F24"),
                ] {
                    ui.invoke_change(key.into(), value.into());
                }
                // Phase 6 opens More before both clipboard and Save As paths.
                // Keep that shared prerequisite identical in every input mode.
                ui.invoke_toolbar_command("visible".into(), 8, 0);
                ui.invoke_action("save".into());
                if self.app.borrow().session.preferences.is_dirty() {
                    return Err(
                        "Input probe setup could not save/register its isolated chords".into(),
                    );
                }
                if self.dialog_mode {
                    ui.invoke_change(
                        "output.directory".into(),
                        self.directory.to_string_lossy().into_owned().into(),
                    );
                    ui.invoke_change("output.format".into(), "png".into());
                    if self.conflict_mode {
                        ui.invoke_change("output.file_conflict".into(), "confirm".into());
                        std::fs::write(
                            self.directory.join("input-拾光.png"),
                            b"QA preexisting file must survive cancelled replacement",
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    ui.invoke_action("save".into());
                    if self.app.borrow().session.preferences.is_dirty() {
                        return Err("Dialog setup failed".into());
                    }
                }
                let _ = ui.hide();
                self.fixture.show().map_err(|e| e.to_string())?;
                self.fixture
                    .window()
                    .set_position(slint::PhysicalPosition::new(80, 80));
            }
            1 => self.fixture_click()?,
            2 => {
                if !self
                    .input
                    .as_ref()
                    .unwrap()
                    .foreground(id(&self.fixture)?)?
                {
                    return self.waiting();
                }
                self.hotkey(0x84)?;
            }
            3 => {
                let Some(c) = self.capture() else {
                    return self.waiting();
                };
                if !self.input.as_ref().unwrap().foreground(id(&c)?)? {
                    return self.waiting();
                }
                if c.get_source_w() < 900. || c.get_source_h() < 700. {
                    return Err("Input probe requires at least a 900x700 source; current environment not tested".into());
                }
                self.rect = glint_core::Rect {
                    x: c.get_source_w() as u32 / 4,
                    y: c.get_source_h() as u32 / 3,
                    width: 480,
                    height: 320,
                };
                let source = c
                    .get_desktop()
                    .to_rgba8()
                    .ok_or("Input capture lacks frozen pixels")?;
                self.expected.clear();
                for y in self.rect.y..self.rect.y + self.rect.height {
                    let begin = ((y * source.width() + self.rect.x) * 4) as usize;
                    self.expected.extend_from_slice(
                        &source.as_bytes()[begin..begin + self.rect.width as usize * 4],
                    );
                }
                image::save_buffer(
                    self.directory.join("input-frozen-crop.png"),
                    &self.expected,
                    480,
                    320,
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                self.source_pointer(&c, self.rect.x, self.rect.y, Pointer::Down)?;
            }
            4 => {
                let c = self.capture().ok_or("Capture closed during OS drag")?;
                self.source_pointer(&c, self.rect.x + 480, self.rect.y + 320, Pointer::Move)?;
            }
            5 => {
                let c = self.capture().ok_or("Capture closed during OS drag")?;
                self.source_pointer(&c, self.rect.x + 480, self.rect.y + 320, Pointer::Up)?;
            }
            6 => {
                let c = self.capture().ok_or("Capture closed after OS selection")?;
                if !c.get_selected()
                    || c.get_left() != self.rect.x as f32
                    || c.get_top() != self.rect.y as f32
                    || c.get_wide() != 480.
                    || c.get_high() != 320.
                {
                    return Err(format!(
                        "OS selection geometry differs: {},{},{},{}",
                        c.get_left(),
                        c.get_top(),
                        c.get_wide(),
                        c.get_high()
                    ));
                }
                self.toolbar(&c, 14)?;
                self.phase = 187;
            }
            188 => {
                let c = self
                    .capture()
                    .ok_or("Capture closed in actual picker More menu")?;
                if c.get_toolbar_modal() != 2 {
                    return self.waiting();
                }
                if c.get_toolbar_hidden().row_count() != 1
                    || c.get_toolbar_hidden().row_data(0) != Some(8)
                {
                    return Err("Picker More fixture differs from saved hidden tool".into());
                }
                let size = c.window().size();
                let scale = c.window().scale_factor();
                let (w, h) = (size.width as f32 / scale, size.height as f32 / scale);
                let (x, y) = logical(&c, w / 2., (h - 420f32.min(h - 24.)) / 2. + 61.);
                self.input
                    .as_mut()
                    .unwrap()
                    .pointer(id(&c)?, x, y, Pointer::Click, false)?;
                self.phase = 184;
            }
            185 => {
                let c = self
                    .capture()
                    .ok_or("Capture closed before real picker click")?;
                if c.get_tool() != 6 {
                    return self.waiting();
                }
                self.source_pointer(&c, self.rect.x + 40, self.rect.y + 40, Pointer::Down)?;
            }
            186 => {
                let c = self.capture().ok_or("Picker press closed image capture")?;
                if c.get_picker_busy() || !c.get_picker_feedback().is_empty() {
                    return Err("Real picker press attempted copy before release".into());
                }
                self.source_pointer(&c, self.rect.x + 40, self.rect.y + 40, Pointer::Up)?;
            }
            187 => {
                let c = self.capture().ok_or("Image picker copy closed editor")?;
                if c.get_picker_busy() {
                    return self.waiting();
                }
                let offset = (40 * 480 + 40) * 4;
                let color = &self.expected[offset..offset + 3];
                let expected = format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
                let actual = glint_platform::read_clipboard_text()?;
                if actual != expected
                    || c.get_object_count() != 0
                    || c.get_can_undo()
                    || c.get_can_redo()
                    || c.get_left() != self.rect.x as f32
                    || c.get_top() != self.rect.y as f32
                    || c.get_wide() != 480.
                    || c.get_high() != 320.
                    || !c.get_output_ready()
                {
                    return Err(
                        "Real picker click changed image/history or copied wrong source HEX".into(),
                    );
                }
                std::fs::write(self.directory.join("picker-color-input.json"),format!("{{\"x\":40,\"y\":40,\"expected\":\"{expected}\",\"actual\":\"{actual}\",\"editor_retained\":true,\"history_unchanged\":true}}\n")).map_err(|e|e.to_string())?;
                std::fs::write(self.directory.join("PICKER-ACTUAL-INPUT-PASSED.txt"),"Guarded real toolbar picker click and canvas Down/Up: press does not copy, release copies exact frozen-source HEX to real clipboard; image editor/crop/history retained.\n").map_err(|e|e.to_string())?;
                self.toolbar(&c, 6)?;
                self.phase = 6;
            }
            189 | 194 | 198 | 203 => {
                let c = self
                    .capture()
                    .ok_or("Held draft lost capture before Down")?;
                self.source_pointer(&c, self.rect.x + 280, self.rect.y + 150, Pointer::Down)?;
            }
            190 | 195 | 199 | 204 => {
                let c = self
                    .capture()
                    .ok_or("Held draft lost capture before Move")?;
                self.source_pointer(&c, self.rect.x + 370, self.rect.y + 220, Pointer::Move)?;
            }
            191 | 196 | 200 | 205 | 206 => {
                let c = self.capture().ok_or("Held draft key closed capture")?;
                if self.phase == 191 {
                    self.draft_observation(&c, 1)?;
                }
                if self.phase == 206 && c.get_toolbar_focus_index() != 0 {
                    return Err("Held-draft Tab did not focus the selection tool".into());
                }
                let action = match self.phase {
                    191 => DragKey::Undo,
                    196 => DragKey::Redo,
                    200 => DragKey::Finish,
                    205 => DragKey::NextFocus,
                    _ => DragKey::Activate,
                };
                self.input.as_mut().unwrap().drag_key(id(&c)?, action)?;
            }
            192 | 197 | 201 | 207 => {
                let c = self.capture().ok_or("Held draft action closed capture")?;
                self.draft_observation(&c, if self.phase == 192 { 0 } else { 1 })?;
                if self.phase == 201 {
                    let status = crate::messages::text(
                        crate::messages::Message::CaptureGesturePending,
                        c.get_english(),
                    );
                    let offset = (40 * 480 + 40) * 4;
                    let color = &self.expected[offset..offset + 3];
                    let sentinel = format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
                    if c.get_status() != status
                        || glint_platform::read_clipboard_text()? != sentinel
                    {
                        return Err(
                            "Held Enter did not block output and retain picker clipboard".into(),
                        );
                    }
                }
                if self.phase == 207 && c.get_tool() != 0 {
                    return Err("Held Space did not activate selection tool".into());
                }
                self.source_pointer(&c, self.rect.x + 370, self.rect.y + 220, Pointer::Up)?;
            }
            193 => {
                let c = self
                    .capture()
                    .ok_or("Late Up closed capture after draft undo")?;
                if c.get_object_count() != 0 || !c.get_can_redo() {
                    return Err("Late Up committed abandoned draft after Undo".into());
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&c)?, &[0x11, 0x10, 0x5A])?;
            }
            202 => {
                let c = self
                    .capture()
                    .ok_or("Release after blocked Enter closed capture")?;
                self.draft_observation(&c, 2)?;
                self.input.as_mut().unwrap().chord(id(&c)?, &[0x11, 0x5A])?;
            }
            208 => {
                let c = self
                    .capture()
                    .ok_or("Late Up after tool change closed capture")?;
                self.draft_observation(&c, 1)?;
                if !c.get_can_undo() || !c.get_can_redo() {
                    return Err("Held tool change discarded undo/redo".into());
                }
                self.toolbar(&c, 6)?;
            }
            209 => {
                let c = self.capture().ok_or("Held draft cleanup lost capture")?;
                if c.get_tool() != 1 {
                    return self.waiting();
                }
                self.input.as_mut().unwrap().chord(id(&c)?, &[0x11, 0x5A])?;
            }
            210 => {
                let c = self
                    .capture()
                    .ok_or("Held draft cleanup undo lost capture")?;
                if c.get_object_count() != 0 {
                    return Err("Held draft cleanup undo did not restore source".into());
                }
                self.source_pointer(&c, self.rect.x + 80, self.rect.y + 70, Pointer::Down)?;
            }
            211 | 212 => {
                let c = self
                    .capture()
                    .ok_or("Held draft fixture rebuild lost capture")?;
                self.source_pointer(
                    &c,
                    self.rect.x + 180,
                    self.rect.y + 130,
                    if self.phase == 211 {
                        Pointer::Move
                    } else {
                        Pointer::Up
                    },
                )?;
            }
            213 => {
                let c = self
                    .capture()
                    .ok_or("Held draft fixture rebuild closed capture")?;
                self.draft_observation(&c, 1)?;
                if !c.get_can_undo() || c.get_can_redo() {
                    return Err(
                        "Held draft fixture did not restore original history boundary".into(),
                    );
                }
                std::fs::write(self.directory.join("HELD-DRAFT-INPUT-PASSED.txt"),"Guarded real Down/Move then Ctrl+Z, Ctrl+Shift+Z, Enter, Tab/Space and late Up. Undo/redo/tool activation discard drafts; blocked Enter preserves editor/clipboard and release commits exactly once. Original one-cover history restored through real undo and redraw before existing regression.\n").map_err(|e|e.to_string())?;
                self.draft_checked = true;
                self.phase = 11;
            }
            7 => {
                let c = self.capture().ok_or("Capture closed before cover")?;
                if c.get_tool() != 1 {
                    return self.waiting();
                }
                self.source_pointer(&c, self.rect.x + 80, self.rect.y + 70, Pointer::Down)?;
            }
            8 => {
                let c = self.capture().ok_or("Capture closed during cover")?;
                self.source_pointer(&c, self.rect.x + 180, self.rect.y + 130, Pointer::Move)?;
            }
            9 => {
                let c = self.capture().ok_or("Capture closed during cover")?;
                self.source_pointer(&c, self.rect.x + 180, self.rect.y + 130, Pointer::Up)?;
            }
            10 => {
                let c = self.capture().ok_or("Capture closed after cover")?;
                if c.get_object_count() != 1 || !c.get_can_undo() {
                    return self.waiting();
                }
                self.input.as_mut().unwrap().chord(id(&c)?, &[0x11, 0x5A])?;
            }
            11 => {
                let c = self.capture().ok_or("Capture closed during undo")?;
                if c.get_object_count() != 0 || !c.get_can_redo() {
                    return self.waiting();
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&c)?, &[0x11, 0x10, 0x5A])?;
            }
            12 => {
                let c = self.capture().ok_or("Capture closed during redo")?;
                if c.get_object_count() != 1 || !c.get_can_undo() {
                    return self.waiting();
                }
                if !self.draft_checked {
                    c.set_magnifier_enabled(false);
                    self.phase = 189;
                    self.since = Instant::now();
                    return Ok(false);
                }
                for y in 70..130 {
                    for x in 80..180 {
                        let i = (y * 480 + x) * 4;
                        self.expected[i..i + 4].copy_from_slice(&[32, 43, 66, 255]);
                    }
                }
                if self.dialog_mode {
                    self.start_dialog(&c, false)?;
                    self.phase = 29; // Next tick checks the actual cancellation.
                } else {
                    self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?;
                    self.phase = 39; // Verify real keyboard traversal before copying.
                }
            }
            13 => {
                if self.capture().is_some() || self.app.borrow().capturing {
                    return self.waiting();
                }
                if ui.window().is_visible() {
                    return Err("Normal screenshot completion opened settings".into());
                }
                let actual = glint_platform::read_clipboard_image()?;
                if actual.width != 480 || actual.height != 320 || actual.rgba != self.expected {
                    return Err(
                        "Real clipboard pixels differ from frozen crop plus opaque cover".into(),
                    );
                }
                image::save_buffer(
                    self.directory.join("input-clipboard.png"),
                    &actual.rgba,
                    480,
                    320,
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                self.region = format!("{:?}", self.app.borrow().last_region);
                self.fixture_click()?;
            }
            14 => {
                if !self
                    .input
                    .as_ref()
                    .unwrap()
                    .foreground(id(&self.fixture)?)?
                {
                    return self.waiting();
                }
                self.hotkey(0x85)?;
            }
            15 => {
                let ids = self.app.borrow().pins.borrow().ids();
                if ids.is_empty() {
                    return self.waiting();
                }
                if ids.len() != 1 {
                    return Err("Clipboard input created an unexpected number of pins".into());
                }
                self.pin = Some(ids[0]);
                let p = self.pin()?;
                if !p.get_ready() {
                    return self.waiting();
                }
                if p.get_picture()
                    .to_rgba8()
                    .ok_or("Pin has no pixels")?
                    .as_bytes()
                    != self.expected
                {
                    return Err("Real clipboard pin changed pixels".into());
                }
                if !self
                    .input
                    .as_ref()
                    .unwrap()
                    .foreground(id(&self.fixture)?)?
                {
                    return Err("Clipboard pin stole keyboard focus".into());
                }
                self.phase = 159;
            }
            160 => {
                let p = self.pin()?;
                let state = crate::pins::native(&p, |n| n.state())?;
                self.pin_position = (state.x, state.y);
                self.start_pin_drag(&p)?;
            }
            161 => {
                if !self.dialog_done()? {
                    return self.waiting();
                }
                let p = self.pin()?;
                let state = crate::pins::native(&p, |n| n.state())?;
                if state.x != self.pin_position.0 + 60 || state.y != self.pin_position.1 + 40 {
                    return Err(format!(
                        "Actual pin drag mismatch: before={:?}, after=({}, {})",
                        self.pin_position, state.x, state.y
                    ));
                }
                self.pin_position = (state.x, state.y);
                self.assert_pin_input(&p, 0)?;
                let (w, _) = Self::pin_size(&p);
                self.pin_click(&p, w - 60., 16.)?;
            }
            162 | 181 => {
                let p = self.pin()?;
                if !p.get_menu_open() {
                    return Err("Pin lock menu did not open".into());
                }
                let (w, _) = Self::pin_size(&p);
                self.pin_click(&p, w / 2., 106.)?;
            }
            163 => {
                let p = self.pin()?;
                if !p.get_locked() {
                    return Err("Actual pin Lock click failed".into());
                }
                self.start_pin_drag(&p)?;
            }
            164 => {
                if !self.dialog_done()? {
                    return self.waiting();
                }
                let p = self.pin()?;
                self.assert_pin_input(&p, 0)?;
                let (w, h) = Self::pin_size(&p);
                self.pin_click(&p, w - 35., h - 20.)?;
            }
            165 => {
                let p = self.pin()?;
                self.assert_pin_input(&p, 0)?;
                if p.get_zoom() != 125 || !p.get_locked() {
                    return Err("Locked pin Plus did not zoom to125".into());
                }
                let (w, h) = Self::pin_size(&p);
                self.pin_click(&p, w - 145., h - 20.)?;
            }
            166 => {
                let p = self.pin()?;
                if p.get_zoom() != 100 {
                    return Err("Pin Minus did not restore100%".into());
                }
                self.pin_wheel(&p, 120, false, false)?;
            }
            167 => {
                let p = self.pin()?;
                self.assert_pin_input(&p, 0)?;
                if p.get_zoom() != 125 {
                    return Err("Real pin wheel did not zoom to125%".into());
                }
                self.pin_wheel(&p, -120, true, false)?;
            }
            168 => {
                let p = self.pin()?;
                self.assert_pin_input(&p, 0)?;
                if p.get_zoom() != 125 || (p.get_alpha() - 0.95).abs() > 0.001 {
                    return Err("Ctrl wheel failed to adjust opacity alone".into());
                }
                let (w, _) = Self::pin_size(&p);
                self.pin_click(&p, w - 60., 16.)?;
            }
            169 | 172 | 175 | 178 => {
                let p = self.pin()?;
                if !p.get_menu_open() {
                    return Err("Actual rotation menu absent".into());
                }
                self.pin_wheel(&p, -960, false, true)?;
            }
            170 | 173 | 176 | 179 => {
                // Native wheel delivery and the ScrollView's settling animation
                // must finish before targeting the bottom-scrolled Rotate row.
                if self.since.elapsed() < Duration::from_millis(450) {
                    return self.waiting();
                }
                let p = self.pin()?;
                if p.get_zoom() != 125 || (p.get_alpha() - 0.95).abs() > 0.001 {
                    return Err("Menu scroll changed pin image scale/alpha".into());
                }
                let shot = p.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer(
                    self.directory.join(format!("pin-menu-{}.png", self.phase)),
                    shot.as_bytes(),
                    shot.width(),
                    shot.height(),
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                self.pin_click(&p, 140., 108.)?;
            }
            171 | 174 | 177 | 180 => {
                let p = self.pin()?;
                if p.get_busy() {
                    return self.waiting();
                }
                let turns = match self.phase {
                    171 => 1,
                    174 => 2,
                    177 => 3,
                    _ => 0,
                };
                self.assert_pin_input(&p, turns)?;
                let pixels = p
                    .get_picture()
                    .to_rgba8()
                    .ok_or("Pin source pixels absent")?;
                image::save_buffer(
                    self.directory
                        .join(format!("pin-source-{}.png", turns * 90)),
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                if !p.get_locked() || p.get_zoom() != 125 || (p.get_alpha() - 0.95).abs() > 0.001 {
                    return Err("Rotation changed pin lock, zoom or opacity".into());
                }
                let shot = p.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer(
                    self.directory.join(format!("pin-input-{}.png", turns * 90)),
                    shot.as_bytes(),
                    shot.width(),
                    shot.height(),
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                let (w, _) = Self::pin_size(&p);
                self.pin_click(&p, w - 60., 16.)?;
            }
            182 => {
                let p = self.pin()?;
                if p.get_locked() {
                    return Err("Pin Unlock click failed".into());
                }
                self.pin_wheel(&p, -120, false, false)?;
            }
            183 => {
                let p = self.pin()?;
                if p.get_zoom() != 100 {
                    return Err("Pin wheel did not restore100%".into());
                }
                self.pin_wheel(&p, 120, true, false)?;
            }
            184 => {
                let p = self.pin()?;
                self.assert_pin_input(&p, 0)?;
                if p.get_zoom() != 100 || (p.get_alpha() - 1.).abs() > 0.001 || p.get_locked() {
                    return Err("Pin input did not restore initial presentation".into());
                }
                std::fs::write(self.directory.join("PIN-GESTURES-INPUT-PASSED.txt"),"Guarded real title drag60x40; locked drag cannot move; Plus/Minus and native wheel/Ctrl wheel while locked; real menu scrolling and four quarter-turn clicks; exact source pixels/orientation; original foreground retained; zoom/alpha/lock restored. Snap disabled only in isolated QA preferences; not physical-human/mixedDPI/all-gesture acceptance.\n").map_err(|e|e.to_string())?;
                let (w, _) = Self::pin_size(&p);
                self.pin_click(&p, w - 60., 16.)?;
                self.phase = 15;
            }
            16 => {
                let p = self.pin()?;
                if !p.get_menu_open() {
                    return self.waiting();
                }
                let (w, _) = Self::pin_size(&p);
                self.pin_click(&p, w / 2., 268.)?;
            }
            17 => {
                let p = self.pin()?;
                if !p.get_confirm_pass() {
                    return self.waiting();
                }
                let (w, h) = Self::pin_size(&p);
                self.pin_click(&p, w * 0.75, h - 37.)?;
            }
            18 => {
                let p = self.pin()?;
                if !p.get_passthrough() {
                    return self.waiting();
                }
                if !crate::pins::native(&p, |n| n.state())?.click_through {
                    return Err("Pass-through UI has no native flag".into());
                }
                self.fixture_click()?;
            }
            19 => {
                if !self
                    .input
                    .as_ref()
                    .unwrap()
                    .foreground(id(&self.fixture)?)?
                {
                    return self.waiting();
                }
                self.hotkey(0x86)?;
            }
            20 => {
                let p = self.pin()?;
                if p.get_passthrough() || crate::pins::native(&p, |n| n.state())?.click_through {
                    return self.waiting();
                }
                self.hotkey(0x87)?;
            }
            21 => {
                let p = self.pin()?;
                if crate::pins::native(&p, |n| n.state())?.visible {
                    return self.waiting();
                }
                self.hotkey(0x87)?;
            }
            22 => {
                let p = self.pin()?;
                if !crate::pins::native(&p, |n| n.state())?.visible {
                    return self.waiting();
                }
                self.hotkey(0x84)?;
            }
            23 => {
                let Some(c) = self.capture() else {
                    return self.waiting();
                };
                if !self.input.as_ref().unwrap().foreground(id(&c)?)? {
                    return self.waiting();
                }
                if crate::pins::native(&self.pin()?, |n| n.state())?.visible {
                    return Err("Actual recapture did not hide pin".into());
                }
                self.input.as_mut().unwrap().chord(id(&c)?, &[0x1B])?;
            }
            24 => {
                if self.capture().is_some() || self.app.borrow().capturing {
                    return self.waiting();
                }
                let p = self.pin()?;
                if !crate::pins::native(&p, |n| n.state())?.visible {
                    return self.waiting();
                }
                if self.region != format!("{:?}", self.app.borrow().last_region) {
                    return Err("Escape changed successful region memory".into());
                }
                self.fixture_click()?;
            }
            25 => {
                let p = self.pin()?;
                let (w, _) = Self::pin_size(&p);
                self.pin_click(&p, w - 20., 16.)?;
            }
            26 => {
                if self.app.borrow().pins.borrow().count() != 0 {
                    return self.waiting();
                }
                if self.app.borrow().pins.borrow().used() != 0 {
                    return Err("Actual pin close retained its budget".into());
                }
                self.fixture.hide().map_err(|e| e.to_string())?;
                ui.invoke_action("open".into());
                ui.invoke_choose_page(0);
                ui.window().set_size(slint::LogicalSize::new(1040., 760.));
                ui.invoke_change("capture.delay_seconds".into(), "1".into());
                self.settings_draft = Some(self.app.borrow().session.preferences.draft.clone());
                self.settings_saved = Some(self.app.borrow().session.preferences.saved().clone());
                self.settings_file = std::fs::read(self.directory.join("config.toml")).ok();
                self.phase = 259;
            }
            260 => {
                if !self.input.as_ref().unwrap().foreground(id(&ui)?)? {
                    return self.waiting();
                }
                ui.invoke_show_modal(0);
                self.navigation_snapshot("before")?;
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            261 | 263 => {
                let focus = ui.get_navigation_focus_index();
                std::fs::write(
                    self.directory.join(format!(
                        "navigation-focus-{}-{}.json",
                        self.phase, self.navigation_target
                    )),
                    format!(
                        "{{\"target\":{},\"focus\":{focus},\"category\":{}}}\n",
                        self.navigation_target,
                        ui.get_category()
                    ),
                )
                .map_err(|e| e.to_string())?;
                if focus != self.navigation_target {
                    self.settings_snapshot(&ui)?;
                    return Err(format!(
                        "Tab could not reach settings category {}: focus={focus}",
                        self.navigation_target
                    ));
                }
                self.input.as_mut().unwrap().chord(
                    id(&ui)?,
                    if self.navigation_target % 2 == 0 {
                        &[0x0D]
                    } else {
                        &[0x20]
                    },
                )?;
            }
            262 | 264 => {
                if ui.get_category() != self.navigation_target
                    || ui.get_navigation_focus_index() != self.navigation_target
                    || ui.get_modal() != 0
                {
                    return Err(format!(
                        "Settings category activation lost page/focus: target={} page={} focus={}",
                        self.navigation_target,
                        ui.get_category(),
                        ui.get_navigation_focus_index()
                    ));
                }
                self.assert_settings(&ui)?;
                self.navigation_snapshot(&format!("{}-{}", self.phase, self.navigation_target))?;
                std::fs::write(self.directory.join(format!("navigation-page-{}-{}.json",self.phase,self.navigation_target)),format!("{{\"target\":{},\"focus\":{},\"category\":{},\"draft_saved_disk_unchanged\":true}}\n",self.navigation_target,ui.get_navigation_focus_index(),ui.get_category())).map_err(|e|e.to_string())?;
                if self.phase == 262 && self.navigation_target < 12 {
                    self.navigation_target += 1;
                    self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
                    self.phase = 261;
                } else if self.navigation_target > 0 {
                    self.navigation_target -= 1;
                    self.input
                        .as_mut()
                        .unwrap()
                        .chord(id(&ui)?, &[0x10, 0x09])?;
                    self.phase = 263;
                } else {
                    std::fs::write(self.directory.join("SETTINGS-NAVIGATION-INPUT-PASSED.txt"),"Real Tab/Enter traverses all13 settings categories and ShiftTab/Enter returns through12. Category focus retained across refresh; draft/saved/disk/capture unchanged. Initial header focus and dirty-draft setup use existing callbacks; no physical-user acceptance.\n").map_err(|e|e.to_string())?;
                    self.phase = 270;
                }
                self.since = Instant::now();
                return Ok(false);
            }
            270 => self.settings_click(&ui, 80., 126. + 2. * 40., false)?,
            271 => {
                if ui.get_category() != 2 {
                    return Err("Scroll fixture did not open Capture settings".into());
                }
                let (x, y) = logical(&ui, 600., 400.);
                let peer = id(&ui)?;
                let mut input = self.input.take().ok_or("Settings wheel worker busy")?;
                self.dialog_worker = Some(std::thread::spawn(move || {
                    let result = input.wheel(peer, x, y, -960, false);
                    (input, result)
                }));
            }
            272 => {
                if self.input.is_none() && !self.dialog_done()? {
                    return self.waiting();
                }
                if self.since.elapsed() < Duration::from_millis(450) {
                    return self.waiting();
                }
                let y = ui.get_page_scroll_y();
                if y >= -100. {
                    return Err(format!("Real settings wheel did not scroll long page: {y}"));
                }
                self.navigation_snapshot("scrolled")?;
                std::fs::write(
                    self.directory.join("settings-scroll-before.json"),
                    format!("{{\"category\":2,\"scroll\":{y}}}\n"),
                )
                .map_err(|e| e.to_string())?;
                self.settings_click(&ui, 80., 126. + 2. * 40., false)?;
                self.phase = 274;
            }
            273 => {
                if self.since.elapsed() < Duration::from_millis(450) {
                    return self.waiting();
                }
                let y = ui.get_page_scroll_y();
                std::fs::write(
                    self.directory.join("settings-scroll-after.json"),
                    format!(
                        "{{\"category\":{},\"scroll\":{y},\"focus\":{}}}\n",
                        ui.get_category(),
                        ui.get_navigation_focus_index()
                    ),
                )
                .map_err(|e| e.to_string())?;
                self.settings_snapshot(&ui)?;
                if ui.get_category() != 4 || y.abs() > 0.5 {
                    return Err(format!(
                        "Changing settings category retained previous scroll: category={} scroll={y}",
                        ui.get_category()
                    ));
                }
                self.assert_settings(&ui)?;
                self.navigation_snapshot("changed-page")?;
                self.settings_click(&ui, 80., 126., false)?;
            }
            274 => {
                if ui.get_category() != 0 || ui.get_page_scroll_y().abs() > 0.5 {
                    return Err("Return General did not start at top".into());
                }
                self.assert_settings(&ui)?;
                std::fs::write(self.directory.join("SETTINGS-SCROLL-INPUT-PASSED.txt"),"Real owned wheel scrolls Capture settings down, actual category click opens Annotation at top, then General at top; existing draft/saved/disk preserved.\n").map_err(|e|e.to_string())?;
                self.phase = 84;
            }
            275 => {
                if self.since.elapsed() < Duration::from_millis(450) {
                    return self.waiting();
                }
                let y = ui.get_page_scroll_y();
                if ui.get_category() != 2 || y >= -100. {
                    return Err("Reactivating current settings category reset scroll".into());
                }
                self.assert_settings(&ui)?;
                self.navigation_snapshot("same-page")?;
                std::fs::write(
                    self.directory.join("settings-scroll-same.json"),
                    format!("{{\"category\":2,\"scroll\":{y}}}\n"),
                )
                .map_err(|e| e.to_string())?;
                self.settings_click(&ui, 80., 126. + 4. * 40., false)?;
                self.phase = 272;
            }
            85 => self.settings_click(&ui, 400., 30., true)?,
            86 => self.settings_click(&ui, 650., 710., false)?,
            87..=98 | 103..=114 | 119..=130 => {
                let expected_modal = if self.phase < 100 {
                    2
                } else if self.phase < 118 {
                    1
                } else {
                    4
                };
                if ui.get_modal() != expected_modal {
                    return Err("Expected settings confirmation absent".into());
                }
                let first = matches!(self.phase, 87 | 103 | 119);
                if first && !ui.get_modal_cancel_focused() {
                    return Err("Settings confirmation did not initially focus safe Cancel".into());
                }
                if !ui.get_modal_cancel_focused() && !ui.get_modal_action_focused() {
                    return Err("Settings Tab escaped the confirmation controls".into());
                }
                self.assert_settings(&ui)?;
                if first {
                    self.settings_snapshot(&ui)?;
                }
                let reverse = matches!(self.phase, 93..=98 | 109..=114 | 125..=130);
                let keys: &[u16] = if reverse { &[0x10, 0x09] } else { &[0x09] };
                self.input.as_mut().unwrap().chord(id(&ui)?, keys)?;
            }
            99 | 115 | 131 => {
                self.assert_settings(&ui)?;
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x1B])?;
            }
            100 | 116 | 132 => {
                if ui.get_modal() != 0 {
                    return Err("Settings Escape did not close confirmation".into());
                }
                self.assert_settings(&ui)?;
                if self.phase == 100 {
                    self.settings_click(&ui, 650., 710., false)?;
                } else if self.phase == 116 {
                    self.settings_click(&ui, 930., 44., false)?;
                } else {
                    ui.invoke_action("config-import".into());
                }
            }
            101 | 117 | 133 => {
                if !ui.get_modal_cancel_focused() {
                    return Err("Reopened settings confirmation lost Cancel focus".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?;
            }
            102 | 118 | 134 => {
                if ui.get_modal() != 0 {
                    return Err("Enter on safe Cancel did not close confirmation".into());
                }
                self.assert_settings(&ui)?;
                if self.phase == 102 {
                    self.settings_click(&ui, 930., 44., false)?;
                } else if self.phase == 118 {
                    ui.invoke_action("config-import".into());
                } else {
                    ui.invoke_choose_page(1);
                }
            }
            135 => {
                // Header -> 13 category buttons -> Capture and Window recorders.
                for _ in 0..15 {
                    self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
                }
            }
            136 => self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?,
            137 => {
                if ui.get_modal() != 3
                    || !ui.get_shortcut_recorder_focused()
                    || self.app.borrow().recording != "shortcuts.window"
                {
                    return Err("Keyboard could not open window shortcut recording".into());
                }
                self.assert_settings(&ui)?;
                self.settings_snapshot(&ui)?;
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, &[0x11, 0x12, 0x83])?;
            }
            138 => {
                self.assert_candidate(&ui, "Ctrl+Alt+F20")?;
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            139 => {
                self.assert_candidate(&ui, "Ctrl+Alt+F20")?;
                if !ui.get_modal_cancel_focused() {
                    return Err("Shortcut Tab missed Cancel".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            140 => {
                if !ui.get_shortcut_clear_focused() {
                    return Err("Shortcut Tab missed Clear".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            141 => {
                if !ui.get_shortcut_accept_focused() {
                    return Err("Shortcut Tab missed Accept".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?;
            }
            142 => {
                self.settings_draft.as_mut().unwrap().shortcuts.window = "Ctrl+Alt+F20".into();
                if ui.get_modal() != 0 {
                    return Err("Keyboard shortcut acceptance did not close".into());
                }
                self.assert_settings(&ui)?;
                ui.invoke_record("shortcuts.window".into());
            }
            143 => {
                if !ui.get_shortcut_recorder_focused() {
                    return Err("Shortcut reopen lost recorder focus".into());
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, &[0x11, 0x12, 0x10, 0x84])?;
            }
            144 => {
                if ui.get_modal() != 3
                    || ui.get_can_accept()
                    || !self.app.borrow().candidate.is_empty()
                    || ui.get_modal_body().as_str()
                        != crate::messages::text(crate::messages::Message::ShortcutInvalid, true)
                {
                    return Err(
                        "Registered duplicate chord interrupted recording or became acceptable"
                            .into(),
                    );
                }
                self.assert_settings(&ui)?;
                self.settings_snapshot(&ui)?;
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, &[0x11, 0x09])?;
            }
            145 => {
                self.assert_candidate(&ui, "Ctrl+Tab")?;
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            146 => {
                if !ui.get_modal_cancel_focused() {
                    return Err("Shortcut Ctrl+Tab prevented ordinary Tab navigation".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?;
            }
            147 => {
                if ui.get_modal() != 0 {
                    return Err("Shortcut focused Cancel Enter did not close".into());
                }
                self.assert_settings(&ui)?;
                ui.invoke_record("shortcuts.window".into());
            }
            148 => {
                if !ui.get_shortcut_recorder_focused() {
                    return Err("Shortcut clear setup lost recorder".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            149 => {
                if !ui.get_shortcut_clear_focused() {
                    return Err("Shortcut Clear not keyboard reachable without candidate".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?;
            }
            150 => {
                self.settings_draft
                    .as_mut()
                    .unwrap()
                    .shortcuts
                    .window
                    .clear();
                if ui.get_modal() != 0 {
                    return Err("Shortcut Clear did not close".into());
                }
                self.assert_settings(&ui)?;
                ui.invoke_record("shortcuts.rescue".into());
            }
            151 => {
                if !ui.get_shortcut_recorder_focused() || ui.get_can_clear() {
                    return Err("Rescue recorder clear protection incorrect".into());
                }
                self.settings_snapshot(&ui)?;
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, &[0x10, 0x09])?;
            }
            152 => {
                if !ui.get_modal_cancel_focused() {
                    return Err("Rescue reverse navigation did not skip disabled actions".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            153 => {
                if !ui.get_shortcut_recorder_focused() {
                    return Err("Rescue navigation did not return to recording".into());
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, &[0x11, 0x12, 0x83])?;
            }
            154 => {
                self.assert_candidate(&ui, "Ctrl+Alt+F20")?;
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, &[0x10, 0x09])?;
            }
            155 => {
                if !ui.get_shortcut_accept_focused() {
                    return Err("Rescue reverse navigation did not reach Accept".into());
                }
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x1B])?;
            }
            156 => {
                if ui.get_modal() != 0 {
                    return Err("Shortcut Escape from Accept did not close".into());
                }
                if !self.guide_checked {
                    self.assert_settings(&ui)?;
                    self.app.borrow_mut().session.preferences.cancel();
                    ui.invoke_action("guide-open".into());
                    self.phase = 214;
                    self.since = Instant::now();
                    return Ok(false);
                }
                self.finish(&ui)?;
                std::fs::write(self.directory.join("SHORTCUT-RECORDER-INPUT-PASSED.txt"),"Guarded real input: Tab/Enter opens window shortcut recorder; Ctrl+Alt+F20 entry; Tab to Cancel/Clear/Accept; Enter draft acceptance; registered duplicate rejected without capture; Ctrl+Tab records; focused Cancel Enter preserves draft; optional Clear Enter; rescue cannot clear; reverse wrap skips disabled actions; Escape from Accept preserves rescue. Draft/saved/disk comparisons retained. Reopens after first use use callbacks; not physical-human/all-shortcut/OS-occupancy acceptance.\n").map_err(|e|e.to_string())?;
                std::fs::write(self.directory.join("SETTINGS-MODAL-INPUT-PASSED.txt"),"Guarded reset/unsaved/pending-import modal Tab/Esc/Enter cancel and unchanged draft/saved/disk observations passed. Import opened by callback without selecting a file.\n").map_err(|e|e.to_string())?;
                std::fs::write(self.directory.join("INPUT-PROBE-PASSED.txt"),"Guarded capture/toolbar/text/clipboard/pin/settings modal and shortcut-recorder workflow passed. Scoped observations retained; not physical-human/environment acceptance.\n").map_err(|e|e.to_string())?;
                ui.invoke_action("quit".into());
                return Ok(true);
            }
            214 => {
                if ui.get_guide_step() != 0 || ui.get_workbench() || ui.get_modal() != 0 {
                    return Err("Guide review did not open clean welcome".into());
                }
                let size = ui.window().size();
                let scale = ui.window().scale_factor();
                self.settings_click(
                    &ui,
                    size.width as f32 / scale - 90.,
                    size.height as f32 / scale - 34.,
                    false,
                )?;
            }
            215 => {
                if ui.get_guide_step() != 1 {
                    return self.waiting();
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, &[0x10, 0x09])?;
            }
            216 => self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?,
            217 => {
                if ui.get_guide_step() != 0 {
                    return self.waiting();
                }
                self.settings_snapshot(&ui)?;
                self.input.as_mut().unwrap().chord(id(&ui)?, &[0x09])?;
            }
            218 => self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?,
            219 => {
                if ui.get_guide_step() != 1 {
                    return self.waiting();
                }
                self.guide_saved = Some(self.app.borrow().session.preferences.saved().clone());
                self.guide_disk = Some(
                    std::fs::read(self.directory.join("config.toml")).map_err(|e| e.to_string())?,
                );
                std::fs::write(
                    self.directory.join("guide-before.toml"),
                    self.guide_disk.as_ref().unwrap(),
                )
                .map_err(|e| e.to_string())?;
                self.guide_next_click(&ui)?;
            }
            220 => {
                if ui.get_guide_step() != 2 {
                    return self.waiting();
                }
                self.guide_next_click(&ui)?;
            }
            221 => {
                if ui.get_guide_step() != 3 {
                    return self.waiting();
                }
                self.settings_snapshot(&ui)?;
            }
            222 => self.guide_color_click(&ui)?,
            223 | 225 => {
                let state = self.app.borrow();
                let mut expected = self.guide_saved.as_ref().unwrap().clone();
                expected.appearance.accent = self.guide_accent.clone();
                if state.session.preferences.draft != expected
                    || state.session.preferences.saved() != self.guide_saved.as_ref().unwrap()
                    || std::fs::read(self.directory.join("config.toml")).ok() != self.guide_disk
                    || ui.get_modal() != 0
                    || ui.get_guide_step() != 3
                {
                    return Err(
                        "Real swatch click/Continue did not retain only the draft accent".into(),
                    );
                }
                std::fs::write(
                    self.directory
                        .join(format!("guide-draft-{}.toml", self.phase)),
                    toml::to_string(&state.session.preferences.draft).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                std::fs::write(
                    self.directory
                        .join(format!("guide-disk-{}.toml", self.phase)),
                    std::fs::read(self.directory.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                drop(state);
                self.settings_snapshot(&ui)?;
                self.guide_close_click(&ui)?;
            }
            224 | 226 => {
                if ui.get_modal() != 1 || !ui.get_modal_cancel_focused() {
                    return Err("Dirty guide exit did not focus Continue".into());
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&ui)?, if self.phase == 224 { &[0x0D] } else { &[0x09] })?;
            }
            227 => self.input.as_mut().unwrap().chord(id(&ui)?, &[0x0D])?,
            228 | 230 => {
                let state = self.app.borrow();
                let mut expected = self.guide_saved.as_ref().unwrap().clone();
                expected.general.onboarding_complete = true;
                if ui.window().is_visible()
                    || ui.get_workbench()
                    || ui.get_guide_step() != -1
                    || state.session.preferences.is_dirty()
                    || state.session.preferences.saved() != &expected
                    || state.session.preferences.draft != expected
                {
                    return Err("Real guide skip did not discard draft and return to tray".into());
                }
                drop(state);
                std::fs::write(
                    self.directory
                        .join(format!("guide-disk-{}.toml", self.phase)),
                    std::fs::read(self.directory.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if self.phase == 228 {
                    ui.invoke_action("guide-open".into());
                } else {
                    std::fs::write(self.directory.join("GUIDE-KEYBOARD-INPUT-PASSED.txt"),"Guarded real Next click, ShiftTab/Enter Back, Tab/Enter Next after Back becomes disabled; real appearance swatch click changes only draft; dirty header exit Continue/Discard via Tab/Enter; clean skip hides to tray. Guide review opened by callback, no physical tray click acceptance.\n").map_err(|e|e.to_string())?;
                    self.guide_checked = true;
                    self.fixture.show().map_err(|e| e.to_string())?;
                    self.phase = 230;
                }
            }
            229 => {
                if ui.get_guide_step() != 0 {
                    return self.waiting();
                }
                self.guide_close_click(&ui)?;
            }
            231 => self.fixture_click()?,
            232 | 235 | 238 => {
                if !self
                    .input
                    .as_ref()
                    .unwrap()
                    .foreground(id(&self.fixture)?)?
                {
                    return self.waiting();
                }
                self.hotkey(0x84)?;
            }
            233 | 236 | 239 => {
                let Some(c) = self.capture() else {
                    return self.waiting();
                };
                if !self.input.as_ref().unwrap().foreground(id(&c)?)? {
                    return self.waiting();
                }
                self.exit_editor = Some(c.clone_strong());
                match self.phase {
                    233 => self.input.as_mut().unwrap().chord(id(&c)?, &[0x1B])?,
                    236 => self.input.as_mut().unwrap().chord(id(&c)?, &[0x12, 0x73])?,
                    _ => self.source_pointer(&c, self.rect.x, self.rect.y, Pointer::Down)?,
                }
            }
            234 => {
                if !self.assert_capture_exit("escape", crate::probe::CaptureExit::EscapeKey)? {
                    return Ok(false);
                }
            }
            237 => {
                if !self.assert_capture_exit("window", crate::probe::CaptureExit::WindowRequest)? {
                    return Ok(false);
                }
            }
            240 | 241 => {
                let c = self.capture().ok_or("Capture exit selection disappeared")?;
                self.source_pointer(
                    &c,
                    self.rect.x + 480,
                    self.rect.y + 320,
                    if self.phase == 240 {
                        Pointer::Move
                    } else {
                        Pointer::Up
                    },
                )?;
            }
            242 => {
                let c = self.capture().ok_or("Capture toolbar cancel lost editor")?;
                if !c.get_selected() {
                    return self.waiting();
                }
                self.toolbar(&c, 13)?;
            }
            243 => {
                if !self.assert_capture_exit("toolbar", crate::probe::CaptureExit::ToolbarCancel)? {
                    return Ok(false);
                }
                let actual = glint_platform::read_clipboard_image()?;
                if actual.width != 480
                    || actual.height != 320
                    || actual.rgba != self.expected
                    || ui.window().is_visible()
                {
                    return Err(
                        "Real capture cancellation changed clipboard or opened settings".into(),
                    );
                }
                std::fs::write(self.directory.join("CAPTURE-EXIT-INPUT-PASSED.txt"),"Guarded real hotkey capture then Escape, AltF4 owned-window close, and selection + toolbar Cancel. Distinct exit origins, original foreground and clipboard retained; settings/workbench remain hidden.\n").map_err(|e|e.to_string())?;
                self.phase = 155;
            }
            30 => {
                if self.input.is_none() && !self.dialog_done()? {
                    return self.waiting();
                }
                let c = self.capture().ok_or("Cancel discarded the capture")?;
                if c.get_busy() {
                    return self.waiting();
                }
                if !c.get_selected()
                    || c.get_object_count() != 1
                    || !c.get_can_undo()
                    || c.get_left() != self.rect.x as f32
                    || c.get_top() != self.rect.y as f32
                    || c.get_wide() != 480.
                    || c.get_high() != 320.
                    || (!self.conflict_mode && self.directory.join("input-拾光.png").exists())
                {
                    return Err(
                        "Dialog cancel changed selection, annotation, history or output".into(),
                    );
                }
                let expected = crate::messages::text(
                    crate::messages::Message::CaptureSaveCancelled,
                    c.get_english(),
                );
                if c.get_status() != expected {
                    return Err("Cancelled save retained stale exporting feedback".into());
                }
                std::fs::write(
                    self.directory.join("save-cancel-status.txt"),
                    c.get_status(),
                )
                .map_err(|e| e.to_string())?;
                self.input.as_mut().unwrap().chord(id(&c)?, &[0x11, 0x5A])?;
            }
            31 => {
                let c = self
                    .capture()
                    .ok_or("Capture closed during post-cancel undo")?;
                if c.get_object_count() != 0 || !c.get_can_redo() {
                    return self.waiting();
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&c)?, &[0x11, 0x10, 0x5A])?;
            }
            32 => {
                let c = self
                    .capture()
                    .ok_or("Capture closed during post-cancel redo")?;
                if c.get_object_count() != 1 || !c.get_can_undo() {
                    return self.waiting();
                }
                self.start_dialog(&c, true)?;
            }
            33 => {
                if self.input.is_none() && !self.dialog_done()? {
                    return self.waiting();
                }
                if self.conflict_mode {
                    let c = self.capture().ok_or("Replacement cancel closed editor")?;
                    if c.get_busy() {
                        return self.waiting();
                    }
                    self.assert_conflict_scene(&c, false)?;
                    self.start_dialog(&c, true)?;
                } else {
                    return self.finish_dialog(&ui);
                }
            }
            34 => {
                if self.input.is_none() && !self.dialog_done()? {
                    return self.waiting();
                }
                let c = self
                    .capture()
                    .ok_or("Changed destination failure closed editor")?;
                if c.get_busy() {
                    return self.waiting();
                }
                self.assert_conflict_scene(&c, true)?;
                if !c.get_status().contains("Destination changed") {
                    return Err(format!(
                        "Expected actual stale-destination error, got: {}",
                        c.get_status()
                    ));
                }
                std::fs::write(
                    self.directory.join("conflict-failure-status.txt"),
                    c.get_status().as_str(),
                )
                .map_err(|e| e.to_string())?;
                self.input.as_mut().unwrap().chord(id(&c)?, &[0x11, 0x5A])?;
            }
            35 => {
                let c = self
                    .capture()
                    .ok_or("Failure recovery undo closed editor")?;
                if c.get_object_count() != 0 || !c.get_can_redo() {
                    return self.waiting();
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&c)?, &[0x11, 0x10, 0x5A])?;
            }
            36 => {
                let c = self
                    .capture()
                    .ok_or("Failure recovery redo closed editor")?;
                if c.get_object_count() != 1 || !c.get_can_undo() {
                    return self.waiting();
                }
                self.start_dialog(&c, true)?;
            }
            37 => {
                if self.input.is_none() && !self.dialog_done()? {
                    return self.waiting();
                }
                return self.finish_dialog(&ui);
            }
            40 | 41 => {
                let c = self.capture().ok_or("Keyboard traversal closed capture")?;
                let expected = i32::from(self.phase == 41);
                if c.get_toolbar_focus_index() != expected || !c.get_toolbar_hint_visible() {
                    return self.waiting();
                }
                let label = if expected == 0 {
                    c.get_labels().select
                } else {
                    c.get_labels().rectangle
                };
                if c.get_toolbar_hint_label() != label {
                    return Err("Keyboard tooltip does not match focused action".into());
                }
                self.input
                    .as_mut()
                    .unwrap()
                    .chord(id(&c)?, &[if expected == 0 { 0x09 } else { 0x20 }])?;
            }
            42 => {
                let c = self.capture().ok_or("Space activation closed capture")?;
                if c.get_tool() != 3 || c.get_toolbar_focus_index() != -1 {
                    return self.waiting();
                }
                self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?;
            }
            43..=57 => {
                let c = self.capture().ok_or("Keyboard traversal closed editor")?;
                // Redo (visible index 10) is disabled after redo; Tab must skip it.
                let expected = match self.phase {
                    43..=52 => i32::from(self.phase - 43),
                    _ => i32::from(self.phase - 42),
                };
                if c.get_toolbar_focus_index() != expected || !c.get_toolbar_hint_visible() {
                    return self.waiting();
                }
                if c.get_toolbar_hint_label().is_empty()
                    || c.get_toolbar_hint_x() < 0.
                    || c.get_toolbar_hint_y() < 0.
                    || c.get_toolbar_hint_y() + 24.
                        > c.window().size().height as f32 / c.window().scale_factor()
                    || c.get_object_count() != 1
                {
                    return Err("Focused toolbar name/bounds or retained annotation invalid".into());
                }
                std::fs::write(
                    self.directory.join(format!("keyboard-{}.txt", self.phase)),
                    format!(
                        "visible_index={expected}\nlabel={}\nx={}\ny={}\n",
                        c.get_toolbar_hint_label(),
                        c.get_toolbar_hint_x(),
                        c.get_toolbar_hint_y()
                    ),
                )
                .map_err(|e| e.to_string())?;
                if self.phase == 57 {
                    let pixels = c.window().take_snapshot().map_err(|e| e.to_string())?;
                    image::save_buffer(
                        self.directory.join("keyboard-customize.png"),
                        pixels.as_bytes(),
                        pixels.width(),
                        pixels.height(),
                        image::ColorType::Rgba8,
                    )
                    .map_err(|e| e.to_string())?;
                }
                let codes: &[u16] = if self.phase == 57 { &[0x0D] } else { &[0x09] };
                self.input.as_mut().unwrap().chord(id(&c)?, codes)?;
                if self.phase == 57 {
                    self.phase = 59;
                }
            }
            244 | 245 => {
                let c = self.capture().ok_or("Toolbar interior walk lost capture")?;
                if c.get_toolbar_modal() != 1 || c.get_object_count() != 1 || !c.get_selected() {
                    return Err("Toolbar interior walk changed editor scene".into());
                }
                let forward = self.phase == 244;
                let reached = if forward {
                    c.get_toolbar_dialog_reset_focused()
                } else {
                    c.get_toolbar_dialog_mode_focused()
                };
                if !reached {
                    if self.toolbar_walk_tabs >= 80 {
                        return Err("Toolbar interior focus traversal did not reach target".into());
                    }
                    self.input
                        .as_mut()
                        .unwrap()
                        .chord(id(&c)?, if forward { &[0x09] } else { &[0x10, 0x09] })?;
                    self.toolbar_walk_tabs += 1;
                    return Ok(false);
                }
                let scroll = c.get_toolbar_editor_scroll();
                let height = c.get_toolbar_editor_visible_height();
                let (top, bottom) = if forward { (502., 532.) } else { (0., 30.) };
                let name = if forward { "reset" } else { "mode" };
                std::fs::write(self.directory.join(format!("toolbar-interior-{name}.json")),format!("{{\"focused\":true,\"tabs\":{},\"scroll\":{scroll},\"height\":{height},\"top\":{top},\"bottom\":{bottom}}}\n",self.toolbar_walk_tabs)).map_err(|e|e.to_string())?;
                let shot = c.window().take_snapshot().map_err(|e| e.to_string())?;
                image::save_buffer(
                    self.directory.join(format!("toolbar-interior-{name}.png")),
                    shot.as_bytes(),
                    shot.width(),
                    shot.height(),
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                if top + scroll < -0.5 || bottom + scroll > height + 0.5 {
                    return Err(format!(
                        "Focused toolbar {name} is clipped: top={top} bottom={bottom} scroll={scroll} height={height}"
                    ));
                }
                self.toolbar_walk_tabs = 0;
                if forward {
                    self.input.as_mut().unwrap().chord(id(&c)?, &[0x0D])?;
                    self.phase = 246;
                } else {
                    self.toolbar_walk_checked = true;
                    self.phase = 62;
                    std::fs::write(self.directory.join("TOOLBAR-INTERIOR-INPUT-PASSED.txt"),"Guarded real Tab/Space toggles groups and rectangle visibility; Enter moves selection down and back up, then activates Reset; ShiftTab returns to mode. Focus targets fully visible; selection/annotation retained and saved/disk unchanged. Existing following Enter Cancel restores original toolbar.\n").map_err(|e|e.to_string())?;
                }
                return Ok(false);
            }
            246..=251 => {
                let c = self.capture().ok_or("Toolbar keyboard edit lost capture")?;
                if c.get_toolbar_modal() != 1
                    || !c.get_selected()
                    || c.get_object_count() != 1
                    || !c.get_can_undo()
                {
                    return Err("Toolbar keyboard edit lost scene/history".into());
                }
                let row = |i| {
                    c.get_toolbar_rows()
                        .row_data(i)
                        .ok_or("Toolbar row missing")
                };
                match self.phase {
                    246 => {
                        if c.get_toolbar_icons()
                            || !c.get_toolbar_groups()
                            || (0..9).any(|i| match row(i) {
                                Ok(r) => r.tool != i as i32 || !r.shown,
                                Err(_) => true,
                            })
                        {
                            return Err("Keyboard Reset did not restore toolbar defaults".into());
                        }
                        self.toolbar_draft_observation(&c, "reset")?;
                        self.phase = 245;
                        return Ok(false);
                    }
                    247 => {
                        for _ in 0..2 {
                            self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?;
                        }
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x20])?;
                    }
                    248 => {
                        if c.get_toolbar_groups() {
                            return Err("Space did not toggle toolbar groups".into());
                        }
                        self.toolbar_draft_observation(&c, "groups")?;
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?;
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x0D])?;
                    }
                    249 => {
                        if row(0)?.tool != 1 || row(1)?.tool != 0 {
                            return Err("Keyboard Down did not reorder selection".into());
                        }
                        self.toolbar_draft_observation(&c, "down")?;
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?;
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x0D])?;
                    }
                    250 => {
                        if row(0)?.tool != 0 || row(1)?.tool != 1 {
                            return Err("Keyboard Up did not restore selection order".into());
                        }
                        self.toolbar_draft_observation(&c, "up")?;
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x10, 0x09])?;
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x20])?;
                    }
                    251 => {
                        if row(1)?.shown {
                            return Err("Space did not hide toolbar rectangle".into());
                        }
                        self.toolbar_draft_observation(&c, "hidden")?;
                        self.phase = 244;
                        return Ok(false);
                    }
                    _ => unreachable!(),
                }
            }
            60..=84 => {
                let c = self.capture().ok_or("Toolbar dialog discarded capture")?;
                if c.get_object_count() != 1 || !c.get_can_undo() || !c.get_selected() {
                    return Err("Toolbar dialog changed capture edits/history".into());
                }
                let closed = matches!(self.phase, 65 | 67 | 70 | 73..=84);
                let modal = if closed {
                    0
                } else if matches!(self.phase, 71 | 72) {
                    2
                } else {
                    1
                };
                if c.get_toolbar_modal() != modal {
                    return self.waiting();
                }
                if !closed
                    && (c.get_background_active()
                        || c.get_toolbar_hint_visible()
                        || c.get_toolbar_focus_index() >= 0)
                {
                    return Err(format!(
                        "Toolbar dialog left background state: active={}, hint={}, toolbar_focus={}, cancel={}, apply={}, mode={}",
                        c.get_background_active(),
                        c.get_toolbar_hint_visible(),
                        c.get_toolbar_focus_index(),
                        c.get_toolbar_dialog_cancel_focused(),
                        c.get_toolbar_dialog_apply_focused(),
                        c.get_toolbar_dialog_mode_focused()
                    ));
                }
                let focus_ok = match self.phase {
                    60 | 64 | 66 | 68 | 71 => c.get_toolbar_dialog_cancel_focused(),
                    61 | 63 | 69 => c.get_toolbar_dialog_apply_focused(),
                    62 => c.get_toolbar_dialog_mode_focused(),
                    72 => !c.get_toolbar_dialog_cancel_focused(),
                    73 => c.get_tool() == 6,
                    74 => c.get_tool() == 8,
                    75 | 79 => c.get_text_editing() && c.get_text_input_focused(),
                    76 => {
                        c.get_text_editing()
                            && c.get_text_value().as_str().eq_ignore_ascii_case("x")
                    }
                    77 => c.get_text_editing() && c.get_text_cancel_focused(),
                    78 | 82 => !c.get_text_editing() && !c.get_text_busy(),
                    80 => {
                        c.get_text_editing()
                            && c.get_text_value().as_str().eq_ignore_ascii_case("y")
                    }
                    81 => !c.get_text_editing() && !c.get_text_busy(),
                    83 => c.get_tool() == 3,
                    84 => {
                        c.get_toolbar_focus_index() == 13
                            && c.get_toolbar_hint_label() == c.get_labels().copy
                    }
                    _ => c.get_background_active(),
                };
                if !focus_ok {
                    if matches!(self.phase, 76 | 80) {
                        let value = c.get_text_value();
                        std::fs::write(
                            self.directory
                                .join(format!("draft-observation-{}.txt", self.phase)),
                            format!(
                                "characters={}\nediting={}\nbusy={}\n",
                                value.chars().count(),
                                c.get_text_editing(),
                                c.get_text_busy()
                            ),
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    return self.waiting();
                }
                if self.phase == 62 && !self.toolbar_walk_checked {
                    self.toolbar_draft_observation(&c, "before")?;
                    self.phase = 247;
                    self.toolbar_walk_tabs = 0;
                    return Ok(false);
                }
                if self.phase == 65 {
                    self.toolbar_draft_observation(&c, "cancel")?;
                    if !c.get_toolbar_icons()
                        || !c.get_toolbar_groups()
                        || c.get_toolbar_hidden().row_count() != 1
                        || c.get_toolbar_hidden().row_data(0) != Some(8)
                    {
                        return Err("Keyboard Cancel did not restore original toolbar".into());
                    }
                }
                std::fs::write(
                    self.directory
                        .join(format!("dialog-keyboard-{}.txt", self.phase)),
                    format!(
                        "modal={}\nbackground={}\ncancel={}\napply={}\nmode={}\ntext_editing={}\ntext_busy={}\ntext_cancel_focused={}\nobjects={}\n",
                        c.get_toolbar_modal(),
                        c.get_background_active(),
                        c.get_toolbar_dialog_cancel_focused(),
                        c.get_toolbar_dialog_apply_focused(),
                        c.get_toolbar_dialog_mode_focused(),
                        c.get_text_editing(), c.get_text_busy(), c.get_text_cancel_focused(), c.get_object_count()
                    ),
                )
                .map_err(|e| e.to_string())?;
                match self.phase {
                    60 | 61 | 68 | 71 => self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?,
                    62 | 63 => self.input.as_mut().unwrap().chord(id(&c)?, &[0x10, 0x09])?,
                    64 | 69 | 72 | 77 | 84 => {
                        self.input.as_mut().unwrap().chord(id(&c)?, &[0x0D])?
                    }
                    65 | 67 => self.toolbar(&c, 15)?,
                    66 => self.input.as_mut().unwrap().chord(id(&c)?, &[0x1B])?,
                    70 => self.toolbar(&c, 14)?,
                    73 => self.toolbar(&c, 4)?,
                    74 | 78 => {
                        self.source_pointer(&c, self.rect.x + 30, self.rect.y + 30, Pointer::Down)?;
                        self.source_pointer(&c, self.rect.x + 30, self.rect.y + 30, Pointer::Up)?;
                    }
                    75 => self.input.as_mut().unwrap().text(id(&c)?, "x")?,
                    76 => {
                        for _ in 0..5 {
                            self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?;
                        }
                    }
                    79 => self.input.as_mut().unwrap().text(id(&c)?, "y")?,
                    80 => self.input.as_mut().unwrap().chord(id(&c)?, &[0x1B])?,
                    81 => self.toolbar(&c, 1)?,
                    82 => {}
                    83 => {
                        for _ in 0..13 {
                            self.input.as_mut().unwrap().chord(id(&c)?, &[0x09])?;
                        }
                    }
                    _ => unreachable!(),
                }
                if self.phase == 84 {
                    self.phase = 12;
                }
            }
            _ => return Err("Unknown input phase".into()),
        }
        if (135..=155).contains(&self.phase) && !self.guide_checked {
            std::fs::write(self.directory.join(format!("shortcut-keyboard-{}.txt",self.phase)),format!("modal={}\nrecorder={}\ncancel={}\nclear={}\naccept={}\ncan_clear={}\ncan_accept={}\n",ui.get_modal(),ui.get_shortcut_recorder_focused(),ui.get_modal_cancel_focused(),ui.get_shortcut_clear_focused(),ui.get_shortcut_accept_focused(),ui.get_can_clear(),ui.get_can_accept())).map_err(|e|e.to_string())?;
        }
        std::fs::write(
            self.directory.join("input-progress.txt"),
            format!("completed_phase={}\n", self.phase),
        )
        .map_err(|e| e.to_string())?;
        self.phase += 1;
        self.since = Instant::now();
        Ok(false)
    }
    fn assert_conflict_scene(&self, c: &CaptureProbe, changed: bool) -> Result<(), String> {
        let expected: &[u8] = if changed {
            b"QA destination changed after replacement prompt observation"
        } else {
            b"QA preexisting file must survive cancelled replacement"
        };
        let actual =
            std::fs::read(self.directory.join("input-拾光.png")).map_err(|e| e.to_string())?;
        if actual != expected
            || !c.get_selected()
            || c.get_object_count() != 1
            || !c.get_can_undo()
            || c.get_left() != self.rect.x as f32
            || c.get_top() != self.rect.y as f32
            || c.get_wide() != 480.
            || c.get_high() != 320.
        {
            return Err(
                "Replacement cancel/failure changed destination, geometry, object or history"
                    .into(),
            );
        }
        std::fs::write(
            self.directory.join(if changed {
                "conflict-failure-preserved.bin"
            } else {
                "conflict-cancel-preserved.bin"
            }),
            actual,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn finish_dialog(&mut self, ui: &SettingsWindow) -> Result<bool, String> {
        if self.capture().is_some() || self.app.borrow().capturing {
            return self.waiting();
        }
        if ui.window().is_visible() {
            return Err("Saving opened settings".into());
        }
        let actual = image::open(self.directory.join("input-拾光.png"))
            .map_err(|e| e.to_string())?
            .to_rgba8();
        if actual.width() != 480 || actual.height() != 320 || actual.as_raw() != &self.expected {
            return Err("Native dialog PNG differs from frozen crop and cover".into());
        }
        self.finish(ui)?;
        let marker = if self.conflict_mode {
            "DIALOG-CONFLICT-INPUT-PASSED.txt"
        } else {
            "DIALOG-INPUT-PASSED.txt"
        };
        std::fs::write(self.directory.join(marker), "Real OS input: capture, selection, cover, undo/redo, Save As Escape cancel retaining geometry/object/history, post-cancel undo/redo, reopen Save As, Unicode absolute filename input, Enter save, exact PNG pixels, successful closure without settings. No preselected output path or dialog result callbacks.\n").map_err(|e| e.to_string())?;
        if self.conflict_mode {
            std::fs::write(self.directory.join("CONFLICT-STEPS-PASSED.txt"), "Real owned replacement MessageBox: Cancel initially focused; Enter cancels and preserves original bytes/scene/history. Reopen and observe; QA fixture changes destination before actual Left/Enter approval. Production publish rejects changed stamp; changed bytes and editor/history retained. Real undo/redo, reopen picker and replacement prompt, actual approval reauthorizes observed new file, final exact flattened PNG and closure.\n").map_err(|e| e.to_string())?;
        }
        eprintln!(
            "Native owned Save As input passed through phase {}",
            self.phase
        );
        ui.invoke_action("quit".into());
        Ok(true)
    }
}
