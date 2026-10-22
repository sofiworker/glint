//! Actual software-rendered frames and resident presentation transactions.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_core::config::Config;
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};

glint_ui::slint! {
    export { Motion } from "@glint_ui/widgets/motion.slint";
    import { Palette, Button as UpstreamButton, CheckBox as UpstreamCheckBox } from "std-widgets.slint";
    import { Button, CheckBox, ComboBox, Slider } from "@glint_ui/widgets/widgets.slint";
    export component MotionFrames inherits Window {
        width: 360px; height: 300px; background: Palette.background;
        public function apply-theme(dark: bool) { Palette.color-scheme = dark ? ColorScheme.dark : ColorScheme.light; }
        in-out property <bool> checked;
        in-out property <string> choice <=> combo.current-value;
        out property <int> index <=> combo.current-index;
        Button { x: 20px; y: 20px; width: 160px; height: 40px; text: "Motion preview"; }
        CheckBox { x: 20px; y: 85px; width: 160px; height: 40px; text: "Enabled"; checked: root.checked; }
        UpstreamButton { x: 190px; y: 20px; width: 160px; height: 40px; text: "Motion preview"; }
        UpstreamCheckBox { x: 190px; y: 85px; width: 160px; height: 40px; text: "Enabled"; }
        Slider { x: 20px; y: 145px; width: 260px; height: 30px; value: 50; }
        combo := ComboBox { x: 20px; y: 200px; width: 260px; height: 40px; model: ["First", "Second", "First"]; }
    }
}

fn frame(ui: &MotionFrames, dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    image::save_buffer_with_format(
        dir.join(name),
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())?;
    // Compare control interiors, excluding window decorations and OS cursor/focus.
    let mut sample = Vec::new();
    for y in 22..122 {
        let start = ((y * pixels.width() + 22) * 4) as usize;
        sample.extend_from_slice(&pixels.as_bytes()[start..start + 150 * 4]);
    }
    Ok(sample)
}
fn hover(ui: &MotionFrames, inside: bool) -> Result<(), String> {
    ui.window()
        .dispatch_event_with_result(slint::platform::WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(if inside { 60. } else { 330. }, 40.),
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn dwm(window: &slint::Window, expected: bool) -> Result<(), String> {
    // DWMWA_TRANSITIONS_FORCEDISABLED is a setter-only DWM attribute.
    // API acceptance is not a visual compositor-animation assertion.
    if window.with_winit_window(|_| ()).is_none() {
        return Err("Native peer missing".into());
    }
    crate::motion::preview(window, expected)
}

type FrameResult = Rc<RefCell<Option<Result<Vec<u8>, String>>>>;

#[derive(Default)]
pub struct Run {
    old: Option<Config>,
    frames: Option<MotionFrames>,
    pending: Option<FrameResult>,
    started: Option<std::time::Instant>,
    pins: Vec<u64>,
    pin: Option<u64>,
    lookup: u8,
}
impl Run {
    fn sample_later(&mut self, dir: &Path, name: &str, reduce_midway: bool) {
        let result = Rc::new(RefCell::new(None));
        self.pending = Some(result.clone());
        let weak = self.frames.as_ref().unwrap().as_weak();
        let dir = dir.to_owned();
        let name = name.to_owned();
        slint::Timer::single_shot(Duration::from_millis(40), move || {
            *result.borrow_mut() = Some(
                weak.upgrade()
                    .ok_or("Motion frame window closed".into())
                    .and_then(|ui| {
                        if reduce_midway {
                            crate::motion::configure!(&ui, Motion, true);
                        }
                        frame(&ui, &dir, &name)
                    }),
            );
        });
    }
    fn take_sample(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.pending
            .as_ref()
            .unwrap()
            .borrow_mut()
            .take()
            .transpose()
    }
    pub fn step(
        &mut self,
        stage: u8,
        ui: &SettingsWindow,
        app: &Rc<RefCell<AppState>>,
        dir: &Path,
    ) -> Result<bool, String> {
        if app.borrow().capturing || app.borrow().capture.as_ref().is_some_and(|c| c.get_busy()) {
            return Ok(false);
        }
        match stage {
            0 => {
                self.old = Some(app.borrow().session.preferences.saved().clone());
                self.pins = app.borrow().pins.borrow().ids();
                ui.invoke_action("open".into());
                ui.invoke_choose_page(8);
                if !ui
                    .get_rows()
                    .iter()
                    .any(|r| r.key == "appearance.reduce_motion" && r.editable)
                {
                    return Err("Motion option disabled".into());
                }
                ui.invoke_change("appearance.reduce_motion".into(), "false".into());
                if ui.get_reduce_motion()
                    || app.borrow().session.preferences.saved() != self.old.as_ref().unwrap()
                {
                    return Err("Motion draft isolation failed".into());
                }
                dwm(ui.window(), false)?;
                ui.invoke_action("cancel".into());
                if ui.get_reduce_motion() != self.old.as_ref().unwrap().appearance.reduce_motion {
                    return Err("Motion cancel failed".into());
                }
                for (key, value) in [
                    ("appearance.reduce_motion", "false"),
                    ("capture.delay_seconds", "0"),
                    ("capture.detect_window", "false"),
                ] {
                    ui.invoke_change(key.into(), value.into());
                }
                ui.invoke_action("save".into());
                let saved = Config::from_toml(
                    &std::fs::read_to_string(dir.join("config.toml")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if saved.appearance.reduce_motion || app.borrow().session.preferences.is_dirty() {
                    return Err("Motion save/TOML failed".into());
                }
                let frames = MotionFrames::new().map_err(|e| e.to_string())?;
                crate::motion::configure!(&frames, Motion, false);
                crate::interface_scale::install_factor(&frames, 1.);
                frames.invoke_apply_theme(false);
                frames.show().map_err(|e| e.to_string())?;
                frames.set_choice("Second".into());
                self.frames = Some(frames);
            }
            1 => {
                let frames = self.frames.as_ref().unwrap();
                if self.lookup == 0 || self.lookup == 4 {
                    let pixels = frames.window().take_snapshot().map_err(|e| e.to_string())?;
                    let scale = frames.window().scale_factor();
                    if (scale - 1.).abs() > 0.01 {
                        return Err(
                            "Motion palette comparison requires explicit 100% control scale".into(),
                        );
                    }
                    for y in 20..125 {
                        for x in 20..180 {
                            for c in 0..4 {
                                let left =
                                    pixels.as_bytes()[((y * pixels.width() + x) * 4 + c) as usize];
                                let right = pixels.as_bytes()
                                    [((y * pixels.width() + x + 170) * 4 + c) as usize];
                                if left.abs_diff(right) > 2 {
                                    return Err(format!(
                                        "Fluent public-palette rendering differs at {x},{y}: {left}/{right}"
                                    ));
                                }
                            }
                        }
                    }
                }
                match self.lookup {
                    0 => {
                        if frames.get_index() != 1 {
                            return Err("ComboBox value lookup failed".into());
                        }
                        frames.set_choice("First".into());
                    }
                    1 => {
                        if frames.get_index() != 0 {
                            return Err("ComboBox duplicate lookup failed".into());
                        }
                        frames.set_choice("Missing".into());
                    }
                    2 => {
                        if frames.get_index() != -1 || !frames.get_choice().is_empty() {
                            return Err("ComboBox unknown value did not clear".into());
                        }
                        frames.set_choice("Second".into());
                    }
                    3 => frames.invoke_apply_theme(true),
                    _ => {}
                }
                if self.lookup < 4 {
                    self.lookup += 1;
                    return Ok(false);
                }
                hover(frames, true)?;
                frames.set_checked(true);
                self.sample_later(dir, "motion-enabled-middle.png", false);
                self.started = Some(std::time::Instant::now());
            }
            2 => {
                if self.started.unwrap().elapsed() < Duration::from_millis(250) {
                    return Ok(false);
                }
                let Some(middle) = self.take_sample()? else {
                    return Ok(false);
                };
                let final_frame = frame(
                    self.frames.as_ref().unwrap(),
                    dir,
                    "motion-enabled-final.png",
                )?;
                if middle == final_frame {
                    return Err("Optional native motion has no intermediate frame".into());
                }
                let frames = self.frames.as_ref().unwrap();
                crate::motion::configure!(frames, Motion, false);
                hover(frames, false)?;
                frames.set_checked(false);
                self.sample_later(dir, "motion-reduced-middle.png", true);
            }
            3 => {
                let Some(middle) = self.take_sample()? else {
                    return Ok(false);
                };
                let final_frame = frame(
                    self.frames.as_ref().unwrap(),
                    dir,
                    "motion-reduced-final.png",
                )?;
                dwm(self.frames.as_ref().unwrap().window(), true)?;
                if middle != final_frame {
                    return Err("Reduced native controls still interpolate".into());
                }

                self.frames
                    .take()
                    .unwrap()
                    .hide()
                    .map_err(|e| e.to_string())?;
                ui.invoke_action("capture".into());
            }
            4 => {
                let state = app.borrow();
                let capture = state.capture.as_ref().ok_or("Motion capture missing")?;
                if capture.get_reduce_motion() {
                    return Err("Capture ignored saved motion".into());
                }
                capture.invoke_pointer(0, 100., 100.);
                capture.invoke_pointer(2, 500., 400.);
                capture.invoke_action("pin".into());
            }
            5 => {
                if app.borrow().capture.is_some() {
                    return Ok(false);
                }
                let pins = app.borrow().pins.clone();
                let manager = pins.borrow();
                let id = manager
                    .ids()
                    .into_iter()
                    .find(|id| !self.pins.contains(id))
                    .ok_or("Motion pin missing")?;
                let pin = manager.window(id).ok_or("Pin UI missing")?;
                if !pin.get_ready() {
                    return Ok(false);
                }
                if pin.get_reduce_motion() {
                    return Err("New pin ignored saved motion".into());
                }
                self.pin = Some(id);
                drop(manager);
                ui.invoke_action("open".into());
                ui.invoke_choose_page(8);
                ui.invoke_change("appearance.reduce_motion".into(), "true".into());
                if !ui.get_reduce_motion() {
                    return Err("Reduced motion preview failed".into());
                }
                dwm(ui.window(), true)?;
                ui.invoke_action("save".into());
                if pins.borrow().window(id).unwrap().get_reduce_motion() {
                    return Err("Saving changed existing pin presentation".into());
                }
                ui.invoke_action("capture".into());
            }
            6 => {
                let state = app.borrow();
                let capture = state.capture.as_ref().ok_or("Reduced capture missing")?;
                if !capture.get_reduce_motion() {
                    return Err("New capture did not snapshot reduced motion".into());
                }
                capture.invoke_action("cancel".into());
            }
            7 => {
                ui.invoke_action("open".into());
                ui.invoke_change("appearance.reduce_motion".into(), "false".into());
                ui.invoke_action("reset".into());
                if ui.get_modal() != 2 || ui.get_reduce_motion() {
                    return Err("Reset changed motion before confirmation".into());
                }
                ui.invoke_action("confirm-reset".into());
                if !ui.get_reduce_motion() {
                    return Err("Reset did not restore reduced default".into());
                }
                ui.invoke_action("cancel".into());
                dwm(ui.window(), true)?;
            }
            8 => {
                if let Some(id) = self.pin.take() {
                    app.borrow().pins.borrow_mut().close(id)?;
                }
                app.borrow_mut().session.preferences.draft = self.old.take().unwrap();
                ui.invoke_action("save".into());
                if app.borrow().session.preferences.is_dirty() {
                    return Err("Motion cleanup did not save original configuration".into());
                }
            }
            9 => {
                eprintln!(
                    "Reduced motion: actual software frames, optional interpolation, immediate reduced controls, DWM setter acceptance, ComboBox value/duplicate/unknown lookup, draft/cancel/reset/TOML, new capture/pin snapshots and existing pin isolation passed"
                );
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}
