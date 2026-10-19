//! Reusable asynchronous font chooser. Selection only updates the owner's draft.
use crate::messages::{self, Message};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{Receiver, sync_channel},
};

#[derive(Default)]
pub(crate) struct State {
    names: Vec<String>,
    query: String,
    error: Option<String>,
    receive: Option<Receiver<Result<Vec<String>, String>>>,
}
pub(crate) trait View: ComponentHandle + 'static {
    fn begin(&self, value: &str);
    fn focus_search(&self);
    fn opened(&self) -> bool;
    fn open(&self, value: bool);
    fn allowed(&self) -> bool;
    fn subscribe(&self, callback: impl Fn(SharedString, SharedString) + 'static);
    fn dispatch(&self, action: &str);
    fn update(&self, state: &State);
    fn choose(&self, name: &str);
    fn focus(&self);
}
pub fn install<T: View>(ui: &T) {
    let state = Rc::new(RefCell::new(State::default()));
    let weak = ui.as_weak();
    ui.subscribe(move |action, value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        match action.as_str() {
            "open" if ui.allowed() && !ui.opened() => {
                state.query.clear();
                ui.begin(&value);
                ui.open(true);
                start(&ui, &mut state);
            }
            "retry" if ui.opened() => start(&ui, &mut state),
            "filter" if ui.opened() => state.query = value.chars().take(80).collect(),
            "cancel" => {
                ui.open(false);
                ui.focus();
            }
            "choose" if ui.opened() && ui.allowed() && state.receive.is_none() => {
                if state.names.iter().any(|n| n == value.as_str()) {
                    ui.open(false);
                    ui.choose(&value);
                    ui.focus();
                }
            }
            "ready" => {
                if let Some(receive) = state.receive.take() {
                    match receive.try_recv() {
                        Ok(Ok(names)) => {
                            state.names = names;
                            state.error = None;
                        }
                        Ok(Err(error)) => state.error = Some(error),
                        Err(std::sync::mpsc::TryRecvError::Empty) => state.receive = Some(receive),
                        Err(error) => state.error = Some(error.to_string()),
                    }
                }
            }
            _ => return,
        }
        ui.update(&state);
        if action == "open" {
            ui.focus_search();
        }
    });
}
fn start<T: View>(ui: &T, state: &mut State) {
    if state.receive.is_some() {
        return;
    }
    let (send, receive) = sync_channel(1);
    let weak = ui.as_weak();
    match std::thread::Builder::new()
        .name("glint-fonts".into())
        .spawn(move || {
            let _ = send.send(glint_platform::installed_font_families());
            let _ = weak.upgrade_in_event_loop(|ui| ui.dispatch("ready"));
        }) {
        Ok(_) => {
            state.receive = Some(receive);
            state.error = None;
        }
        Err(error) => state.error = Some(error.to_string()),
    }
}

macro_rules! view {
    ($ty:ty, $english:expr, $allowed:expr, $choose:expr, $focus:expr) => {
        impl View for $ty {
            fn begin(&self, value: &str) {
                self.set_font_current(value.into());
                self.set_font_query("".into());
            }
            fn focus_search(&self) {
                self.invoke_focus_font();
            }
            fn opened(&self) -> bool {
                self.get_font_open()
            }
            fn open(&self, value: bool) {
                self.set_font_open(value);
            }
            fn allowed(&self) -> bool {
                ($allowed)(self)
            }
            fn subscribe(&self, callback: impl Fn(SharedString, SharedString) + 'static) {
                self.on_font_command(callback);
            }
            fn dispatch(&self, action: &str) {
                self.invoke_font_command(action.into(), "".into());
            }
            fn choose(&self, name: &str) {
                ($choose)(self, name);
            }
            fn focus(&self) {
                ($focus)(self);
            }
            fn update(&self, state: &State) {
                let en = ($english)(self);
                let tr = |id| messages::text(id, en);
                self.set_font_query(state.query.clone().into());
                let query = state.query.to_lowercase();
                let names: Vec<SharedString> = state
                    .names
                    .iter()
                    .filter(|n| n.to_lowercase().contains(&query))
                    .map(|n| n.into())
                    .collect();
                let empty = names.is_empty();
                self.set_font_names(ModelRc::from(Rc::new(VecModel::from(names))));
                self.set_font_busy(state.receive.is_some());
                self.set_font_failed(state.error.is_some());
                self.set_font_title(tr(Message::FontTitle).into());
                self.set_font_search(tr(Message::FontSearch).into());
                self.set_font_cancel(tr(Message::FontCancel).into());
                self.set_font_retry(tr(Message::FontRetry).into());
                let status = if let Some(error) = &state.error {
                    format!("{}: {error}", tr(Message::FontFailed))
                } else {
                    tr(if state.receive.is_some() {
                        Message::FontLoading
                    } else if empty {
                        Message::FontEmpty
                    } else {
                        Message::FontHint
                    })
                    .to_owned()
                };
                self.set_font_status(status.into());
            }
        }
    };
}
view!(
    crate::settings::SettingsWindow,
    |ui: &crate::settings::SettingsWindow| ui.get_font_english(),
    |ui: &crate::settings::SettingsWindow| ui.get_writable()
        && ui.get_modal() == 0
        && ui.get_category() == 4,
    |ui: &crate::settings::SettingsWindow, name: &str| ui
        .invoke_change("annotation.text_font".into(), name.into()),
    |ui: &crate::settings::SettingsWindow| ui.invoke_focus_recorder()
);
view!(
    crate::probe::CaptureProbe,
    |ui: &crate::probe::CaptureProbe| ui.get_english(),
    |ui: &crate::probe::CaptureProbe| ui.get_text_editing()
        && !ui.get_text_busy()
        && !ui.get_busy(),
    |ui: &crate::probe::CaptureProbe, name: &str| ui.set_text_font(name.into()),
    |ui: &crate::probe::CaptureProbe| ui.invoke_focus_text()
);
