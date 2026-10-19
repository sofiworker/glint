//! Opt-in normal-exit pin snapshots. Private image I/O stays off the UI thread.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use glint_core::{
    ImageDocument, Rect,
    pin_session::{self as portable, Placement},
};
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
};
pub struct Snapshot {
    pub original: SharedPixelBuffer<Rgba8Pixel>,
    pub document: Option<Arc<ImageDocument>>,
    pub placement: Placement,
}
pub struct ReadyPin {
    pub original: SharedPixelBuffer<Rgba8Pixel>,
    pub rotated: SharedPixelBuffer<Rgba8Pixel>,
    pub document: Arc<ImageDocument>,
    pub placement: Placement,
}
#[derive(Clone, Copy)]
enum Kind {
    Restore,
    Clear,
    SaveExit,
}
enum Completed {
    Loaded(Vec<ReadyPin>),
    Cleared,
    Saved,
}
#[derive(Default)]
pub struct State {
    pending_config: bool,
    hidden_for_save: bool,
    deferred: Option<String>,
    job: Option<Receiver<Result<Completed, Message>>>,
    kind: Option<Kind>,
    restoring: bool,
    pub protect_saved: bool,
    pub loaded: bool,
    pub quit_failed: bool,
    pub status: Option<Message>,
    observed: bool,
}
fn drain(state: &mut AppState, ui: &SettingsWindow) {
    if let Some(action) = state.pin_session.deferred.take() {
        let _ = ui
            .as_weak()
            .upgrade_in_event_loop(move |ui| ui.invoke_action(action.into()));
    }
}
impl State {
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.restoring
    }
}
pub fn sync(state: &AppState, ui: &SettingsWindow) {
    let tr = |id| messages::text(id, state.en());
    ui.set_session_help(tr(Message::SessionHelp).into());
    ui.set_session_clear_label(tr(Message::SessionClear).into());
    ui.set_session_restore_label(tr(Message::SessionRestore).into());
    ui.set_session_retry_label(tr(Message::SessionRetryExit).into());
    ui.set_session_exit_label(tr(Message::SessionExitWithout).into());
    ui.set_session_cancel_label(tr(Message::SessionCancel).into());
    ui.set_session_busy(state.pin_session.busy());
    ui.set_session_quit_failed(state.pin_session.quit_failed);
    ui.set_session_can_restore(
        state.session.writable
            && state.session.notice.is_empty()
            && state.session.preferences.saved().pins.restore_session
            && !state.pin_session.loaded
            && !state.pin_session.busy(),
    );
    ui.set_session_status(state.pin_session.status.map(tr).unwrap_or("").into());
}
pub fn sync_status(state: &AppState, ui: &SettingsWindow) {
    let error = state.pin_session.status.filter(|id| {
        matches!(
            id,
            Message::SessionIoFailed
                | Message::SessionRestoreFailed
                | Message::SessionSaveFailed
                | Message::SessionClearFailed
                | Message::SessionFinishFirst
        )
    });
    if state.pin_session.busy() {
        ui.set_status(messages::text(Message::SessionBusy, state.en()).into());
    } else if let Some(id) = error {
        ui.set_status(messages::text(id, state.en()).into());
    }
}
fn ready(pin: portable::Pin) -> Result<ReadyPin, Message> {
    let (w, h) = pin.document.dimensions();
    let pixels = glint_core::composite_selection(
        pin.document.source(),
        w,
        h,
        Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        },
        pin.document.edits(),
    )
    .map_err(|_| Message::SessionRestoreFailed)?;
    let original = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&pixels, w, h);
    let rotated = if pin.placement.turns == 0 {
        original.clone()
    } else {
        let image =
            image::RgbaImage::from_raw(w, h, pixels).ok_or(Message::SessionRestoreFailed)?;
        let image = match pin.placement.turns {
            1 => image::imageops::rotate90(&image),
            2 => image::imageops::rotate180(&image),
            _ => image::imageops::rotate270(&image),
        };
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            image.as_raw(),
            image.width(),
            image.height(),
        )
    };
    Ok(ReadyPin {
        original,
        rotated,
        document: pin.document,
        placement: pin.placement,
    })
}
fn spawn(
    state: &mut AppState,
    ui: &SettingsWindow,
    kind: Kind,
    work: impl FnOnce() -> Result<Completed, Message> + Send + 'static,
) -> Result<(), Message> {
    if state.pin_session.busy() {
        return Err(Message::SessionBusy);
    }
    let (send, receive) = mpsc::sync_channel(1);
    let weak = ui.as_weak();
    std::thread::Builder::new()
        .name("glint-pin-session".into())
        .spawn(move || {
            let _ = send.send(work());
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("pin-session-ready".into()));
        })
        .map_err(|_| Message::SessionIoFailed)?;
    state.pin_session.kind = Some(kind);
    state.pin_session.job = Some(receive);
    state.pin_session.status = Some(Message::SessionBusy);
    Ok(())
}
fn restore(state: &mut AppState, ui: &SettingsWindow, dir: &Path) -> Result<(), Message> {
    let dir = dir.to_owned();
    let budget =
        u64::from(state.session.preferences.saved().resources.pin_budget_mib) * 1024 * 1024;
    spawn(state, ui, Kind::Restore, move || {
        let bytes = glint_platform::pin_session_file::read(&dir, portable::MAX_BYTES)
            .map_err(|_| Message::SessionRestoreFailed)?;
        let pins = match bytes {
            Some(bytes) => portable::decode(&bytes).map_err(|_| Message::SessionRestoreFailed)?,
            None => vec![],
        };
        // Source documents plus original/rotated buffers are all charged again by Manager.
        let retained = pins
            .iter()
            .try_fold(0u64, |total, p| {
                total
                    .checked_add(p.document.retained_bytes())?
                    .checked_add(p.document.source().len() as u64 * 2)
            })
            .ok_or(Message::SessionRestoreFailed)?;
        if retained > budget {
            return Err(Message::SessionRestoreFailed);
        }
        let pins = pins.into_iter().map(ready).collect::<Result<Vec<_>, _>>()?;
        glint_platform::pin_session_file::clear_temporary(&dir)
            .map_err(|_| Message::SessionIoFailed)?;
        Ok(Completed::Loaded(pins))
    })
}
fn clear(state: &mut AppState, ui: &SettingsWindow, dir: &Path) -> Result<(), Message> {
    let dir = dir.to_owned();
    spawn(state, ui, Kind::Clear, move || {
        glint_platform::pin_session_file::clear(&dir).map_err(|_| Message::SessionClearFailed)?;
        Ok(Completed::Cleared)
    })
}
fn save_exit(state: &mut AppState, ui: &SettingsWindow, dir: &Path) -> Result<(), Message> {
    if state.capture.is_some()
        || state.capturing
        || state.clipboard_pin_pending
        || state.transfer.busy()
    {
        return Err(Message::SessionFinishFirst);
    }
    let snapshots = state
        .pins
        .borrow()
        .session_snapshot()
        .map_err(|_| Message::SessionFinishFirst)?;
    state
        .pins
        .borrow_mut()
        .hide_for_capture()
        .map_err(|_| Message::SessionIoFailed)?;
    state.pin_session.hidden_for_save = true;
    let dir = dir.to_owned();
    let result = spawn(state, ui, Kind::SaveExit, move || {
        let pins = snapshots
            .into_iter()
            .map(|s| {
                let doc = match s.document {
                    Some(d) => d,
                    None => Arc::new(
                        ImageDocument::from_selection(
                            s.original.as_bytes(),
                            s.original.width(),
                            s.original.height(),
                            Rect {
                                x: 0,
                                y: 0,
                                width: s.original.width(),
                                height: s.original.height(),
                            },
                            &Default::default(),
                        )
                        .map_err(|_| Message::SessionIoFailed)?,
                    ),
                };
                Ok(portable::Pin {
                    placement: s.placement,
                    document: doc,
                })
            })
            .collect::<Result<Vec<_>, Message>>()?;
        if pins.is_empty() {
            glint_platform::pin_session_file::clear(&dir)
                .map_err(|_| Message::SessionClearFailed)?;
        } else {
            let bytes = portable::encode(&pins).map_err(|_| Message::SessionSaveFailed)?;
            glint_platform::pin_session_file::write(&dir, &bytes, portable::MAX_BYTES)
                .map_err(|_| Message::SessionSaveFailed)?;
        }
        Ok(Completed::Saved)
    });
    if result.is_err() {
        let _ = state.pins.borrow_mut().restore_after_capture();
        state.pin_session.hidden_for_save = false;
    }
    result
}
pub fn start(state: &mut AppState, ui: &SettingsWindow, dir: &Path) {
    state.pin_session.observed = state.session.preferences.saved().pins.restore_session;
    if state.session.is_new && !state.pin_session.observed {
        return;
    }
    if !state.session.writable || !state.session.notice.is_empty() {
        state.pin_session.pending_config = true;
        state.pin_session.protect_saved = true;
        state.pin_session.status = Some(Message::SessionConfigRecovery);
        return;
    }
    let result = if state.pin_session.observed {
        restore(state, ui, dir)
    } else {
        clear(state, ui, dir)
    };
    if let Err(error) = result {
        state.pin_session.status = Some(error);
        state.pin_session.protect_saved = true;
    }
    state.sync(ui);
}
pub fn after_action(state: &mut AppState, ui: &SettingsWindow, dir: &Path) {
    if state.capture_pending
        || state.capture.is_some()
        || state.capturing
        || state.clipboard_pin_pending
        || state.transfer.busy()
    {
        return;
    }
    if state.pin_session.pending_config && state.session.writable && state.session.notice.is_empty()
    {
        state.pin_session.pending_config = false;
        start(state, ui, dir);
        return;
    }
    let enabled = state.session.preferences.saved().pins.restore_session;
    if enabled != state.pin_session.observed && !state.pin_session.busy() {
        state.pin_session.observed = enabled;
        if !enabled {
            if let Err(error) = clear(state, ui, dir) {
                state.pin_session.status = Some(error);
            }
            state.sync(ui);
        }
    }
}
fn failure(state: &mut AppState, ui: &SettingsWindow, error: Message, kind: Kind) {
    // A failed operation returns control to the error UI. Do not replay an old
    // capture/open request after a later successful retry.
    let deferred = state.pin_session.deferred.take();
    state.pin_session.status = Some(error);
    if matches!(kind, Kind::Restore) {
        state.pin_session.protect_saved = true;
    }
    if matches!(kind, Kind::SaveExit) {
        state.pin_session.quit_failed = true;
        if std::mem::take(&mut state.pin_session.hidden_for_save) {
            let _ = state.pins.borrow_mut().restore_after_capture();
        }
    }
    if matches!(kind, Kind::Clear)
        && deferred
            .as_deref()
            .is_some_and(|action| matches!(action, "quit" | "resident-exit"))
    {
        state.pin_session.quit_failed = true;
    }
    if let Some(capture) = &state.capture {
        capture.set_status(messages::text(error, capture.get_english()).into());
        return;
    }
    ui.set_workbench(false);
    ui.set_guide_step(-1);
    ui.set_category(6);
    let _ = crate::resident::show_settings(ui);
}
fn instantiate(
    shared: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    mut queue: std::collections::VecDeque<ReadyPin>,
    created: Rc<RefCell<Vec<u64>>>,
) {
    let Some(pin) = queue.pop_front() else {
        let mut state = shared.borrow_mut();
        state.pin_session.restoring = false;
        state.pin_session.loaded = true;
        state.pin_session.protect_saved = false;
        state.pin_session.status = Some(if created.borrow().is_empty() {
            Message::SessionEmpty
        } else {
            Message::SessionRestored
        });
        state.sync(ui);
        drain(&mut state, ui);
        return;
    };
    let (manager, config, en) = {
        let state = shared.borrow();
        (
            state.pins.clone(),
            state.session.preferences.saved().clone(),
            state.en(),
        )
    };
    let owner = Rc::downgrade(shared);
    let weak = ui.as_weak();
    let callback_created = created.clone();
    let started =
        crate::pins::Manager::create_restored(&manager, pin, &config, en, move |result| {
            let (Some(owner), Some(ui)) = (owner.upgrade(), weak.upgrade()) else {
                return;
            };
            let result = result.and_then(|id| {
                owner.borrow().pins.borrow_mut().recover_restored(id)?;
                Ok(id)
            });
            match result {
                Ok(_) => instantiate(&owner, &ui, queue, callback_created),
                Err(_) => rollback_restore(&owner, &ui, &callback_created),
            }
        });
    match started {
        Ok(id) => created.borrow_mut().push(id),
        Err(_) => rollback_restore(shared, ui, &created),
    }
}
fn rollback_restore(
    shared: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    created: &RefCell<Vec<u64>>,
) {
    let mut state = shared.borrow_mut();
    for id in created.borrow().iter() {
        let _ = state.pins.borrow_mut().close(*id);
    }
    state.pin_session.restoring = false;
    failure(&mut state, ui, Message::SessionRestoreFailed, Kind::Restore);
    state.sync(ui);
}
pub fn handle(
    action: &str,
    shared: &Rc<RefCell<AppState>>,
    ui: &SettingsWindow,
    dir: &Path,
) -> bool {
    if action == "pin-session-ready" {
        let (kind, result) = {
            let mut state = shared.borrow_mut();
            let Some(job) = state.pin_session.job.take() else {
                return true;
            };
            let Some(kind) = state.pin_session.kind.take() else {
                return true;
            };
            (
                kind,
                job.try_recv().unwrap_or(Err(Message::SessionIoFailed)),
            )
        };
        match result {
            Ok(Completed::Loaded(pins)) => {
                shared.borrow_mut().pin_session.restoring = true;
                instantiate(shared, ui, pins.into(), Rc::new(RefCell::new(vec![])));
            }
            Ok(Completed::Cleared) => {
                let mut state = shared.borrow_mut();
                state.pin_session.protect_saved = false;
                state.pin_session.loaded = false;
                state.pin_session.status = Some(Message::SessionCleared);
                state.sync(ui);
                drain(&mut state, ui);
            }
            Ok(Completed::Saved) => {
                let _ = slint::quit_event_loop();
            }
            Err(error) => {
                let mut state = shared.borrow_mut();
                failure(&mut state, ui, error, kind);
                state.sync(ui);
            }
        }
        return true;
    }
    let mut state = shared.borrow_mut();
    if state.pin_session.busy() {
        if matches!(
            action,
            "config-transfer-ready"
                | "clipboard-pin-ready"
                | "capture-ready"
                | "capture-finished"
                | "hotkey-error"
                | "system-changed"
        ) {
            return false;
        }
        if !matches!(state.pin_session.kind, Some(Kind::SaveExit))
            && (state.pin_session.deferred.is_none() || action == "resident-exit")
            && (matches!(
                action,
                "resident-exit"
                    | "quit"
                    | "close"
                    | "open"
                    | "workbench"
                    | "capture"
                    | "window-capture"
                    | "repeat-capture"
                    | "all-screens-capture"
                    | "current-screen-capture"
                    | "color-picker"
                    | "clipboard-pin"
            ) || action.starts_with("edit-pin:"))
        {
            state.pin_session.deferred = Some(action.to_owned());
        }
        return true;
    }
    if matches!(ui.get_modal(), 5 | 6) {
        if matches!(action, "close" | "quit") {
            ui.invoke_show_modal(0);
            return false;
        }
        if action == "continue" {
            ui.invoke_show_modal(0);
            return true;
        }
        if !matches!(
            action,
            "pin-session-clear-confirm"
                | "pin-session-exit-confirm"
                | "rescue-pins"
                | "toggle-pins"
                | "hotkey-error"
        ) {
            return true;
        }
    }
    match action {
        "resident-exit" => {
            if state.session.writable
                && state.pin_session.observed
                && !state.session.preferences.saved().pins.restore_session
            {
                state.pin_session.observed = false;
                state.pin_session.deferred = Some("resident-exit".into());
                if let Err(error) = clear(&mut state, ui, dir) {
                    failure(&mut state, ui, error, Kind::Clear);
                }
                state.sync(ui);
                return true;
            }
            if !state.session.writable
                || !state.session.preferences.saved().pins.restore_session
                || state.pin_session.protect_saved
            {
                let _ = slint::quit_event_loop();
            } else if let Err(error) = save_exit(&mut state, ui, dir) {
                failure(&mut state, ui, error, Kind::SaveExit);
            }
        }
        "pin-session-restore"
            if ui.get_modal() == 0
                && !state.pin_session.loaded
                && state.session.writable
                && state.session.notice.is_empty()
                && state.session.preferences.saved().pins.restore_session =>
        {
            if state.capture.is_some()
                || state.capturing
                || state.clipboard_pin_pending
                || state.transfer.busy()
            {
                return true;
            }
            if let Err(error) = restore(&mut state, ui, dir) {
                failure(&mut state, ui, error, Kind::Restore);
            }
        }
        "pin-session-clear" if ui.get_modal() == 0 && state.session.writable => {
            if state.capture.is_some()
                || state.capturing
                || state.clipboard_pin_pending
                || state.transfer.busy()
            {
                return true;
            }
            ui.invoke_show_modal(5);
            ui.set_modal_title(messages::text(Message::SessionClear, state.en()).into());
            ui.set_modal_body(messages::text(Message::SessionClearHint, state.en()).into());
        }
        "pin-session-clear-confirm" if ui.get_modal() == 5 => {
            if !state.session.writable {
                return true;
            }
            ui.invoke_show_modal(0);
            if let Err(error) = clear(&mut state, ui, dir) {
                failure(&mut state, ui, error, Kind::Clear);
            }
        }
        "pin-session-retry-exit" if state.pin_session.quit_failed => {
            if matches!(state.pin_session.status, Some(Message::SessionClearFailed)) {
                state.pin_session.deferred = Some("resident-exit".into());
                if let Err(error) = clear(&mut state, ui, dir) {
                    failure(&mut state, ui, error, Kind::Clear);
                }
                state.sync(ui);
                return true;
            }
            drop(state);
            let weak = ui.as_weak();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_action("quit".into());
                }
            });
            return true;
        }
        "pin-session-exit-without" if state.pin_session.quit_failed => {
            ui.invoke_show_modal(6);
            ui.set_modal_title(messages::text(Message::SessionExitWithout, state.en()).into());
            ui.set_modal_body(messages::text(Message::SessionExitHint, state.en()).into());
        }
        "pin-session-exit-confirm" if ui.get_modal() == 6 => {
            let _ = slint::quit_event_loop();
        }
        _ => return false,
    }
    state.sync(ui);
    true
}
