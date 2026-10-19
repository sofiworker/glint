//! One event-driven clipboard read at a time; no image history or clipboard polling.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use glint_core::config::Config;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{Receiver, sync_channel},
};

pub struct Job {
    receive: Option<Receiver<Result<glint_platform::DesktopFrame, String>>>,
    cancelled: bool,
    preparing_pin: Option<u64>,
    config: Config,
    english: bool,
    position: (i32, i32),
}
enum Read {
    Cancelled,
    Completed(Result<glint_platform::DesktopFrame, String>),
}
impl Job {
    fn cancel(&mut self) -> Option<u64> {
        self.cancelled = true;
        self.preparing_pin.take()
    }
    fn read_ready(&mut self, blocked: bool) -> Option<Read> {
        use std::sync::mpsc::TryRecvError;
        self.cancelled |= blocked;
        let result = match self.receive.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("Clipboard worker disconnected".into()),
        };
        self.receive.take();
        Some(if self.cancelled {
            Read::Cancelled
        } else {
            Read::Completed(result)
        })
    }
}
pub fn cancel_pending(state: &mut AppState) -> Result<(), String> {
    let pin = state.clipboard_pin_job.as_mut().and_then(Job::cancel);
    // Retain the job/occupied flag until its worker or preparation callback drains.
    // Resume must not start an overlapping clipboard read or replay the old result.
    if let Some(id) = pin {
        state.pins.borrow_mut().close(id)?;
    }
    Ok(())
}
pub fn start(state: &mut AppState, ui: &SettingsWindow) -> Result<(), String> {
    crate::completion::dismiss();
    let config = state.session.preferences.saved().clone();
    let english = crate::settings::english(config.general.language, &state.languages);
    let position = glint_platform::cursor_position()?;
    let (send, receive) = sync_channel(1);
    let weak = ui.as_weak();
    std::thread::Builder::new()
        .name("glint-clipboard-pin".into())
        .spawn(move || {
            let _ = send.send(glint_platform::read_clipboard_image());
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("clipboard-pin-ready".into()));
        })
        .map_err(|e| e.to_string())?;
    state.clipboard_pin_pending = true;
    state.clipboard_pin_job = Some(Job {
        receive: Some(receive),
        cancelled: false,
        preparing_pin: None,
        config,
        english,
        position,
    });
    state.feedback = messages::text(Message::PinClipboardReading, english).into();
    Ok(())
}
fn finish_cancelled(
    app: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    english: bool,
    created: Option<u64>,
) {
    let mut state = app.borrow_mut();
    state.clipboard_pin_job.take();
    state.clipboard_pin_pending = false;
    let close = created.map_or(Ok(()), |id| state.pins.borrow_mut().close(id));
    state.feedback = close
        .err()
        .unwrap_or_else(|| messages::text(Message::PinClipboardCancelled, english).into());
    state.sync(ui);
}
fn finish(
    app: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    english: bool,
    feedback: glint_core::config::Feedback,
    scale: f32,
    reduced: bool,
    result: Result<u64, String>,
) {
    let cancelled = {
        let state = app.borrow();
        state.lifecycle.blocked()
            || state
                .clipboard_pin_job
                .as_ref()
                .is_some_and(|job| job.cancelled)
    };
    if cancelled {
        finish_cancelled(app, ui, english, result.ok());
        return;
    }
    let failed = result.is_err();
    {
        let mut state = app.borrow_mut();
        state.clipboard_pin_job.take();
        state.clipboard_pin_pending = false;
        state.feedback = match result {
            Ok(_) => messages::text(Message::PinClipboardCreated, english).into(),
            Err(error) => format!(
                "{}: {error}",
                messages::text(Message::PinClipboardFailed, english)
            ),
        };
        state.sync(ui);
    }
    if !failed {
        crate::completion::show(
            feedback,
            3,
            english,
            ui.get_dark(),
            ui.get_accent(),
            scale,
            reduced,
        );
    }
    if failed {
        let _ = crate::resident::show_settings(ui);
    }
}
pub fn ready(app: &Rc<RefCell<AppState>>, ui: &SettingsWindow) {
    let (read, mut config, english, position) = {
        let mut state = app.borrow_mut();
        let blocked = state.lifecycle.blocked();
        let Some(job) = state.clipboard_pin_job.as_mut() else {
            return;
        };
        let Some(read) = job.read_ready(blocked) else {
            return;
        };
        (read, job.config.clone(), job.english, job.position)
    };
    let result = if let Read::Completed(result) = read {
        result
    } else {
        finish_cancelled(app, ui, english, None);
        return;
    };
    let image = match result {
        Ok(frame) => slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
            &frame.rgba,
            frame.width,
            frame.height,
        )),
        Err(error) => {
            finish(
                app,
                ui,
                english,
                config.output.feedback,
                crate::interface_scale::factor(config.appearance.scale),
                config.appearance.reduce_motion,
                Err(error),
            );
            return;
        }
    };
    let manager = app.borrow().pins.clone();
    // Defaults belong to the invocation; the latest saved budget remains authoritative.
    config.resources.pin_budget_mib = app
        .borrow()
        .session
        .preferences
        .saved()
        .resources
        .pin_budget_mib;
    let weak = Rc::downgrade(app);
    let weak_ui = ui.as_weak();
    let feedback = config.output.feedback;
    let scale = crate::interface_scale::factor(config.appearance.scale);
    let reduced = config.appearance.reduce_motion;
    let result =
        crate::pins::Manager::create(&manager, image, position, &config, english, move |result| {
            if let (Some(app), Some(ui)) = (weak.upgrade(), weak_ui.upgrade()) {
                finish(&app, &ui, english, feedback, scale, reduced, result);
            }
        });
    match result {
        Ok(id) => {
            if let Some(job) = app.borrow_mut().clipboard_pin_job.as_mut() {
                job.preparing_pin = Some(id);
            }
        }
        Err(error) => finish(app, ui, english, feedback, scale, reduced, Err(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type ImageResult = Result<glint_platform::DesktopFrame, String>;

    fn fixture() -> (std::sync::mpsc::SyncSender<ImageResult>, Job) {
        let (send, receive) = sync_channel(1);
        (
            send,
            Job {
                receive: Some(receive),
                cancelled: false,
                preparing_pin: None,
                config: Config::default(),
                english: false,
                position: (0, 0),
            },
        )
    }
    fn image() -> ImageResult {
        Ok(glint_platform::DesktopFrame {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            rgba: vec![24, 48, 96, 255],
        })
    }
    #[test]
    fn suspend_discards_late_image_and_error_even_after_resume() {
        for result in [image(), Err("clipboard occupied".into())] {
            let (send, mut job) = fixture();
            assert_eq!(job.cancel(), None);
            assert!(job.read_ready(true).is_none());
            assert!(
                job.receive.is_some(),
                "Keep the occupied worker until it drains"
            );
            send.send(result).unwrap();
            assert!(matches!(job.read_ready(false), Some(Read::Cancelled)));
            assert!(job.receive.is_none());
            assert!(job.read_ready(false).is_none());
        }
        let (send, mut job) = fixture();
        send.send(image()).unwrap();
        assert!(matches!(job.read_ready(true), Some(Read::Cancelled)));
    }
    #[test]
    fn early_ready_keeps_worker_and_preparing_pin_is_cancelled_once() {
        let (send, mut job) = fixture();
        assert!(job.read_ready(false).is_none());
        send.send(image()).unwrap();
        match job.read_ready(false) {
            Some(Read::Completed(Ok(frame))) => assert_eq!(frame.rgba, [24, 48, 96, 255]),
            _ => panic!("Live invocation must retain the successful image"),
        }
        job.preparing_pin = Some(42);
        assert_eq!(job.cancel(), Some(42));
        assert_eq!(job.cancel(), None);
        assert!(job.cancelled);
        assert!(job.read_ready(false).is_none());
    }
}
