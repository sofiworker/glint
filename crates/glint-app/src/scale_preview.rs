//! Temporary UI scale confirmation. Unconfirmed values never reach saved preferences.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_core::config::Scale;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct State {
    previous: Option<Scale>,
    deadline: Option<Instant>,
    timer: slint::Timer,
}

fn publish(state: &AppState, ui: &SettingsWindow) {
    let seconds = state
        .scale_preview
        .deadline
        .map(|deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_secs()
                .saturating_add(1)
                .min(15)
        })
        .unwrap_or(0);
    let en = ui.get_font_english();
    ui.set_modal_title(
        if en {
            "Keep this interface scale?"
        } else {
            "保留此缩放比例？"
        }
        .into(),
    );
    ui.set_modal_body(if en {
        format!("Check that the interface is easy to use. Reverting in {seconds} seconds unless confirmed. Keep retains the preview in your draft; Save settings to persist it.")
    } else {
        format!("请确认界面是否清晰且易于操作。{seconds} 秒内未确认将自动还原。保留后仍需保存设置才会永久生效。")
    }.into());
    ui.set_scale_keep_label(
        if en {
            "Keep this scale"
        } else {
            "保留此比例"
        }
        .into(),
    );
    ui.set_scale_revert_label(if en { "Revert" } else { "还原" }.into());
}

fn resolve(state: &mut AppState, ui: &SettingsWindow, keep: bool) {
    state.scale_preview.timer.stop();
    state.scale_preview.deadline = None;
    if let Some(previous) = state.scale_preview.previous.take() {
        if !keep {
            state.session.preferences.draft.appearance.scale = previous;
        }
        ui.invoke_show_modal(0);
        state.sync(ui);
    }
}

pub fn begin(
    state: &mut AppState,
    shared: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    previous: Scale,
) {
    state.scale_preview.previous = Some(previous);
    state.scale_preview.deadline = Some(Instant::now() + Duration::from_secs(15));
    publish(state, ui);
    ui.invoke_show_modal(8);
    let owner: Weak<RefCell<AppState>> = Rc::downgrade(shared);
    let weak_ui = ui.as_weak();
    state.scale_preview.timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            let (Some(owner), Some(ui)) = (owner.upgrade(), weak_ui.upgrade()) else {
                return;
            };
            let mut state = owner.borrow_mut();
            if !ui.window().is_visible()
                || state
                    .scale_preview
                    .deadline
                    .is_none_or(|deadline| Instant::now() >= deadline)
            {
                resolve(&mut state, &ui, false);
            } else {
                publish(&state, &ui);
            }
        },
    );
}

pub fn handle(action: &str, shared: &Rc<RefCell<AppState>>, ui: &SettingsWindow) -> bool {
    let mut state = shared.borrow_mut();
    if state.scale_preview.previous.is_none() {
        return false;
    }
    let keep = action == "scale-keep"
        && state
            .scale_preview
            .deadline
            .is_some_and(|deadline| Instant::now() < deadline);
    resolve(&mut state, ui, keep);
    // Closing, capture and lifecycle events may proceed after reverting. Save
    // also proceeds with the reverted value, never an unconfirmed preview.
    matches!(
        action,
        "scale-keep" | "continue" | "cancel" | "scale-revert"
    )
}
