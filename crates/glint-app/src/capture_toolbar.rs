//! Toolbar-only draft. Applying never touches the capture session or unrelated preferences.
use super::{CaptureProbe, ToolbarRow};
use glint_core::config::{Toolbar, ToolbarMode};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

use crate::toolbar::{index, items, position};

fn publish(ui: &CaptureProbe, config: &Toolbar) {
    ui.set_toolbar_icons(config.mode == ToolbarMode::Icons);
    ui.set_toolbar_position(position(config));
    ui.set_toolbar_groups(config.groups);
    let visible = items(config);
    let hidden: Vec<i32> = config
        .order
        .iter()
        .filter(|t| config.hidden.contains(t))
        .map(|t| index(*t))
        .collect();
    // Retain delegates while focus-out callbacks are still being delivered.
    ui.set_toolbar_items(crate::toolbar::update_model(
        ui.get_toolbar_items(),
        visible,
    ));
    ui.set_toolbar_hidden(crate::toolbar::update_model(
        ui.get_toolbar_hidden(),
        hidden,
    ));
    let names = crate::toolbar::names(ui.get_english());
    ui.set_toolbar_editor_labels(crate::toolbar::labels(ui.get_english()));
    ui.set_toolbar_names(ModelRc::new(VecModel::from(names.clone())));
    ui.set_toolbar_rows(crate::toolbar::update_model(
        ui.get_toolbar_rows(),
        config
            .order
            .iter()
            .map(|t| ToolbarRow {
                label: names[index(*t) as usize].clone(),
                tool: index(*t),
                shown: !config.hidden.contains(t),
            })
            .collect::<Vec<_>>(),
    ));
}

pub fn configure(
    ui: &CaptureProbe,
    config: Toolbar,
    save: impl Fn(&Toolbar) -> Result<(), String> + 'static,
) {
    publish(ui, &config);
    let state = Rc::new(RefCell::new((config.clone(), config)));
    let weak = ui.as_weak();
    ui.on_toolbar_command(move |command, row, value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_color_only() || ui.get_busy() || ui.get_text_busy() || ui.get_output_error_open()
        {
            return;
        }
        let mut state = state.borrow_mut();
        match command.as_str() {
            "open" => {
                if ui.get_text_editing() {
                    return;
                }
                state.1 = state.0.clone();
                ui.set_toolbar_feedback("".into());
                // Move focus while background scopes are still enabled: disabled
                // Slint scopes ignore FocusOut and can retain a stale has-focus.
                ui.invoke_focus_canvas();
                ui.set_toolbar_modal(1);
                ui.invoke_focus_toolbar_dialog();
            }
            "more" => {
                ui.invoke_focus_canvas();
                ui.set_toolbar_modal(2);
                ui.invoke_focus_toolbar_dialog();
                return;
            }
            "cancel" => {
                state.1 = state.0.clone();
                ui.set_toolbar_modal(0);
                ui.invoke_focus_canvas();
            }
            "apply" => match save(&state.1) {
                Ok(()) => {
                    state.0 = state.1.clone();
                    ui.set_toolbar_modal(0);
                    ui.invoke_focus_canvas();
                }
                Err(error) => {
                    ui.set_toolbar_feedback(error.into());
                }
            },
            _ => {
                if crate::toolbar::edit(&mut state.1, command.as_str(), row, value).is_err() {
                    return;
                }
            }
        }
        publish(&ui, &state.1);
    });
}
