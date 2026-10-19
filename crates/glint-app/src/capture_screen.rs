//! Tray capture targets, resolved against the same snapshot as the frozen source.
use glint_core::{
    Rect,
    screen_capture::{ScreenTarget, screen_region},
};
pub fn target(action: &str) -> Option<ScreenTarget> {
    match action {
        "all-screens-capture" => Some(ScreenTarget::All),
        "current-screen-capture" => Some(ScreenTarget::Current),
        _ => None,
    }
}
pub fn action(target: ScreenTarget) -> &'static str {
    match target {
        ScreenTarget::All => "all-screens-capture",
        ScreenTarget::Current => "current-screen-capture",
    }
}
pub fn initial(snapshot: &glint_platform::DesktopSnapshot, target: ScreenTarget) -> Option<Rect> {
    let displays: Vec<_> = snapshot
        .displays
        .iter()
        .map(|d| [d.bounds.left, d.bounds.top, d.bounds.right, d.bounds.bottom])
        .collect();
    screen_region(
        target,
        (snapshot.frame.x, snapshot.frame.y),
        (snapshot.frame.width, snapshot.frame.height),
        &displays,
        snapshot.pointer,
    )
}
pub fn notice(target: ScreenTarget, selected: bool) -> crate::messages::Message {
    use crate::messages::Message;
    if !selected {
        Message::ScreenSelectionUnavailable
    } else {
        match target {
            ScreenTarget::All => Message::AllScreensSelected,
            ScreenTarget::Current => Message::CurrentScreenSelected,
        }
    }
}
