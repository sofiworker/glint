//! Shared toolbar ordering, visibility and editor labels. No capture or persistence side effects.
use crate::messages::{self, Message};
use glint_core::config::{Tool, Toolbar, ToolbarMode, ToolbarPosition};
use slint::{Model, ModelRc, SharedString, VecModel};

/// Keep delegate identities stable while a checkbox/drag callback is still releasing its grab.
pub fn update_model<T: Clone + PartialEq + 'static>(model: ModelRc<T>, next: Vec<T>) -> ModelRc<T> {
    if let Some(rows) = model.as_any().downcast_ref::<VecModel<T>>() {
        if rows.row_count() == next.len() {
            for (index, row) in next.into_iter().enumerate() {
                if rows.row_data(index).as_ref() != Some(&row) {
                    rows.set_row_data(index, row);
                }
            }
        } else {
            rows.set_vec(next);
        }
        model
    } else {
        ModelRc::new(VecModel::from(next))
    }
}

pub const TOOLS: [Tool; 9] = [
    Tool::Select,
    Tool::Rect,
    Tool::Arrow,
    Tool::Pen,
    Tool::Text,
    Tool::Highlight,
    Tool::Redact,
    Tool::Mosaic,
    Tool::Picker,
];
pub fn index(tool: Tool) -> i32 {
    TOOLS.iter().position(|t| *t == tool).unwrap_or(0) as i32
}
pub fn position(toolbar: &Toolbar) -> i32 {
    match toolbar.position {
        ToolbarPosition::Follow => 0,
        ToolbarPosition::Top => 1,
        ToolbarPosition::Bottom => 2,
    }
}
pub fn items(toolbar: &Toolbar) -> Vec<i32> {
    let mut items = Vec::new();
    if toolbar.show_customize { items.push(15); }
    items.push(0);
    items.extend(toolbar.order.iter()
        .filter(|t| **t != Tool::Select && !toolbar.hidden.contains(t))
        .map(|t| index(*t)));
    if !toolbar.hidden.is_empty() {
        items.push(14);
    }
    items.extend([9, 10, 16, 11, 13, 12]);
    items
}
pub fn names(english: bool) -> Vec<SharedString> {
    [
        Message::CaptureSelect,
        Message::CaptureRectangle,
        Message::CaptureArrow,
        Message::CapturePen,
        Message::CaptureText,
        Message::CaptureHighlight,
        Message::CaptureRedact,
        Message::CapturePixelate,
        Message::CapturePicker,
        Message::CaptureUndo,
        Message::CaptureRedo,
        Message::CaptureSave,
        Message::CaptureFinish,
        Message::CaptureCancel,
        Message::ToolbarMore,
        Message::ToolbarCustomize,
        Message::CapturePin,
    ]
    .map(|id| messages::text(id, english).into())
    .to_vec()
}
// Indices are the shared ToolbarEditor string interface, independent of displayed language.
pub fn labels(english: bool) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from([
        Message::ToolbarBoth,
        Message::ToolbarIcons,
        Message::ToolbarFollow,
        Message::ToolbarTop,
        Message::ToolbarBottom,
        Message::ToolbarGroups,
        Message::ToolbarOrderHint,
        Message::ToolbarPreview,
        Message::ToolbarReset,
        Message::ToolbarSettingsPreviewHint,
        Message::ToolbarMode,
        Message::ToolbarPosition,
        Message::ToolbarLayout,
        Message::ToolbarGrid,
        Message::ToolbarScroll,
        Message::ToolbarFixedWidth,
        Message::ToolbarColumns,
        Message::ToolbarRows,
        Message::ToolbarWidth,
        Message::ToolbarSingleRow,
        Message::ToolbarWidthHint,
    ].map(|id| messages::text(id, english).into()).to_vec()))
}

pub fn edit(
    toolbar: &mut Toolbar,
    command: &str,
    row: i32,
    value: i32,
) -> Result<(), &'static str> {
    match command {
        "reset" => *toolbar = Toolbar::default(),
        "layout" if matches!(value, 0 | 1 | 3) => toolbar.layout = value as u8,
        "customize-visible" if matches!(value, 0 | 1) => toolbar.show_customize = value == 1,
        "columns" if (2..=20).contains(&value) => toolbar.columns = value as u8,
        "rows" if (1..=6).contains(&value) => toolbar.rows = value as u8,
        "width" if (100..=1200).contains(&value) => toolbar.width = value as u16,
        "mode" => {
            toolbar.mode = match value {
                0 => ToolbarMode::IconsAndLabels,
                1 => ToolbarMode::Icons,
                _ => return Err("toolbar.mode"),
            }
        }
        "position" => {
            toolbar.position = match value {
                0 => ToolbarPosition::Follow,
                1 => ToolbarPosition::Top,
                2 => ToolbarPosition::Bottom,
                _ => return Err("toolbar.position"),
            }
        }
        "groups" => {
            toolbar.groups = match value {
                0 => false,
                1 => true,
                _ => return Err("toolbar.groups"),
            }
        }
        "visible" => {
            if !matches!(value, 0 | 1) {
                return Err("toolbar.visible");
            }
            let tool = *toolbar.order.get(row as usize).ok_or("toolbar.row")?;
            toolbar.hidden.retain(|t| *t != tool);
            if value == 0 && tool != Tool::Select {
                toolbar.hidden.push(tool);
            }
        }
        "move" => {
            let len = toolbar.order.len();
            if row < 0 || row as usize >= len || value < 0 || value as usize >= len {
                return Err("toolbar.row");
            }
            let tool = toolbar.order.remove(row as usize);
            toolbar.order.insert(value as usize, tool);
        }
        _ => return Err("toolbar.command"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_keep_selection_available_and_invalid_events_do_not_change_draft() {
        let mut toolbar = Toolbar::default();
        edit(&mut toolbar, "visible", 0, 0).unwrap();
        edit(&mut toolbar, "visible", 8, 0).unwrap();
        edit(&mut toolbar, "move", 8, 1).unwrap();
        assert_eq!(toolbar.order[1], Tool::Picker);
        assert_eq!(toolbar.hidden, vec![Tool::Picker]);
        let items = items(&toolbar);
        for required in [0, 12, 13, 14, 15] {
            assert!(items.contains(&required));
        }
        let expected = toolbar.clone();
        for (command, row, value) in [
            ("move", -1, 0),
            ("move", 0, 9),
            ("visible", 9, 0),
            ("visible", 1, 3),
            ("mode", 0, 8),
            ("position", 0, 3),
            ("groups", 0, -1),
            ("unknown", 0, 0),
        ] {
            assert!(edit(&mut toolbar, command, row, value).is_err());
            assert_eq!(toolbar, expected);
        }
        edit(&mut toolbar, "reset", 0, 0).unwrap();
        assert_eq!(toolbar, Toolbar::default());
    }
}
