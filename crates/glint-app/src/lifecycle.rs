//! Lifecycle policy is event driven; no desktop/input polling.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
pub struct State {
    locked: bool,
    disconnected: bool,
    sleeping: bool,
    pub discard_ready: bool,
    pub generation: u64,
    pub guard: Option<glint_platform::LifecycleService>,
}
impl State {
    pub fn blocked(&self) -> bool {
        self.locked || self.disconnected || self.sleeping
    }
}
pub fn install(ui: &SettingsWindow, state: Rc<RefCell<AppState>>) {
    let receiver = ui.as_weak();
    let installed = glint_platform::LifecycleService::start(move |event| {
        let action = match event {
            glint_platform::LifecycleEvent::Locked => "lifecycle-lock",
            glint_platform::LifecycleEvent::Unlocked => "lifecycle-unlock",
            glint_platform::LifecycleEvent::Disconnected => "lifecycle-disconnect",
            glint_platform::LifecycleEvent::Connected => "lifecycle-connect",
            glint_platform::LifecycleEvent::Sleeping => "lifecycle-sleep",
            glint_platform::LifecycleEvent::Resumed => "lifecycle-resume",
            glint_platform::LifecycleEvent::Stopped => "lifecycle-unavailable",
        };
        let _ = receiver.upgrade_in_event_loop(move |ui| ui.invoke_action(action.into()));
    });
    match installed {
        Ok(guard) => state.borrow_mut().lifecycle.guard = Some(guard),
        Err(error) => {
            let mut state = state.borrow_mut();
            state.feedback = format!(
                "{}: {error}",
                messages::text(Message::LifecycleUnavailable, state.en())
            );
            state.sync(ui);
        }
    }
}

pub fn handle(action: &str, state: &mut AppState, ui: &SettingsWindow) -> bool {
    if action == "lifecycle-unavailable" {
        state.feedback = messages::text(Message::LifecycleUnavailable, state.en()).into();
        state.sync(ui);
        return true;
    }
    let was_blocked = state.lifecycle.blocked();
    match action {
        "lifecycle-lock" => state.lifecycle.locked = true,
        "lifecycle-unlock" => state.lifecycle.locked = false,
        "lifecycle-disconnect" => state.lifecycle.disconnected = true,
        "lifecycle-connect" => state.lifecycle.disconnected = false,
        "lifecycle-sleep" => state.lifecycle.sleeping = true,
        "lifecycle-resume" => state.lifecycle.sleeping = false,
        _ => return false,
    }
    if state.lifecycle.blocked() && !was_blocked {
        state.capture_settings_visible = false;
        let clipboard_error = crate::clipboard_pin::cancel_pending(state).err();
        let had_capture = state.capture.is_some() || state.capturing || state.capture_pending;
        let had_countdown = state.countdown.is_some() || state.delay_elapsed;
        crate::completion::dismiss();
        state.countdown.take();
        state.delay_ticket = state.delay_ticket.wrapping_add(1);
        state.delay_elapsed = false;
        state.capture_pending = false;
        // Keep a worker in its single occupied slot until its result arrives. Do not
        // launch an overlapping capture or reuse this source after a suspend boundary.
        state.lifecycle.discard_ready |= state.capture_result.is_some();
        if state.capture_result.is_none() {
            state.capturing = false;
        }
        if state.session.preferences.saved().resources.sleep_release {
            state.lifecycle.generation = state.lifecycle.generation.wrapping_add(1);
            if let Some(capture) = state.capture.take() {
                let _ = capture.hide();
            }
            state.local_edit = false;
            state.edit_pin = None;
            state.capture_focus.take();
            if std::mem::take(&mut state.guide_practice) {
                state.guide_succeeded = false;
                ui.set_guide_step(4);
            }
            if had_capture {
                state.feedback = messages::text(Message::LifecycleReleased, state.en()).into();
            }
        } else if let Some(capture) = &state.capture {
            // Invalidate pending directory/picker callbacks even if the session
            // resumes before they return. Visibility alone loses that boundary.
            capture
                .set_save_dialog_generation(capture.get_save_dialog_generation().wrapping_add(1));
            // Retained editors must not depend on the window backend emitting a
            // pointer-cancel when hidden. A late release after resume is not a commit.
            capture.invoke_pointer(3, 0., 0.);
            let _ = capture.hide();
        }
        if state.capture.is_none() {
            state.capture_focus.take();
            if had_countdown || state.lifecycle.discard_ready {
                state.feedback = messages::text(Message::LifecycleReleased, state.en()).into();
            }
        }
        if let Some(error) = clipboard_error {
            state.feedback = error;
        }
    } else if was_blocked && !state.lifecycle.blocked() {
        // Every new capture enumerates current displays and DPI through the capture
        // backend; no automatic old capture/countdown replay on resume.
        if let Some(capture) = &state.capture {
            if let Err(error) = capture.show() {
                state.feedback = error.to_string();
            }
        } else if let Err(error) = state.pins.borrow_mut().restore_after_capture() {
            state.feedback = error;
        }
        state.languages = glint_platform::preferred_ui_languages().unwrap_or_default();
    }
    state.sync(ui);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_lock_disconnect_and_sleep_gates_cannot_resume_each_other() {
        let mut state = State {
            locked: true,
            sleeping: true,
            disconnected: true,
            ..Default::default()
        };
        state.sleeping = false;
        assert!(state.blocked());
        state.locked = false;
        assert!(state.blocked());
        state.disconnected = false;
        assert!(!state.blocked());
    }
}
