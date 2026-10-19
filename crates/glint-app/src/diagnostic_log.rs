//! Settings adapter for opt-in event logs. Never forwards user/error strings to the writer.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_platform::diagnostic_log::{DiagnosticLog, Event};
use slint::ComponentHandle;
use std::{cell::RefCell, path::Path, rc::Rc};

pub fn capture_exit(log: &mut DiagnosticLog, capture: &crate::probe::CaptureProbe) {
    use crate::probe::CaptureExit;
    use glint_platform::diagnostic_log::CaptureExitCause as Cause;
    let cause = match capture.get_exit_reason() {
        CaptureExit::Open => Cause::Open,
        CaptureExit::ApiCancel => Cause::ApiCancel,
        CaptureExit::EscapeKey => Cause::Escape,
        CaptureExit::SecondaryClick => Cause::SecondaryClick,
        CaptureExit::ToolbarCancel => Cause::ToolbarCancel,
        CaptureExit::WindowRequest => Cause::WindowRequest,
        CaptureExit::Copy => Cause::Copy,
        CaptureExit::Save => Cause::Save,
        CaptureExit::Pin => Cause::Pin,
        CaptureExit::ColorCopy => Cause::ColorCopy,
        CaptureExit::PinUpdate => Cause::PinUpdate,
        CaptureExit::Direct => Cause::Direct,
    };
    log.record(if capture.get_completion_kind() > 0 {
        Event::CaptureCompleted
    } else {
        Event::CaptureCancelled
    });
    log.record(Event::CaptureExit(cause));
    if capture.get_busy() {
        log.record(Event::CaptureClosedBusy);
    }
}

#[derive(Clone, Copy)]
enum TextId {
    Help,
    On,
    Off,
    Error,
    Clear,
    Retry,
    Title,
    Confirm,
}
fn text(id: TextId, en: bool) -> &'static str {
    use TextId::*;
    match (id, en) {
        (Help, false) => {
            "仅保存时间与固定事件代码，不含图像、正文、路径、窗口标题或配置值。位置：配置目录 / diagnostics；最多两份 512 KiB 文件，循环替换。关闭停止新记录，已有文件保留至手动清理。"
        }
        (Help, true) => {
            "Records only timestamps and fixed event codes, never images, text, paths, window titles or configuration values. Location: configuration directory / diagnostics. At most two 512 KiB files rotate. Turning logging off preserves existing files until cleared."
        }
        (On, false) => "记录已开启（已保存的设置）。",
        (On, true) => "Logging is on (saved preference).",
        (Off, false) => "记录已关闭（已保存的设置），不创建新日志。",
        (Off, true) => "Logging is off (saved preference); no new logs are created.",
        (Error, false) => "日志操作失败或队列已满；当前任务不受影响。可重试或清理日志。",
        (Error, true) => {
            "Log operation failed or the queue is full. The current task is preserved. Retry or clear the logs."
        }
        (Clear, false) => "清理诊断日志",
        (Clear, true) => "Clear diagnostic logs",
        (Retry, false) => "重试记录",
        (Retry, true) => "Retry logging",
        (Title, false) => "清理诊断日志？",
        (Title, true) => "Clear diagnostic logs?",
        (Confirm, false) => {
            "仅删除两份诊断日志，不删除截图、贴图会话或配置。开启记录时，之后的事件会重新创建日志。"
        }
        (Confirm, true) => {
            "Delete only the two diagnostic logs. Captures, pin sessions and configuration are preserved. If logging is on, later events create logs again."
        }
    }
}
pub fn new(directory: &Path, ui: &SettingsWindow) -> RefCell<DiagnosticLog> {
    let weak = ui.as_weak();
    RefCell::new(DiagnosticLog::new(
        directory.join("diagnostics"),
        move || {
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("log-ready".into()));
        },
    ))
}
pub fn sync(state: &AppState, ui: &SettingsWindow) {
    let enabled = state.session.preferences.saved().privacy.diagnostics;
    let mut log = state.log.borrow_mut();
    log.configure(enabled);
    let status = log.status();
    let en = state.en();
    ui.set_log_help(text(TextId::Help, en).into());
    ui.set_log_clear(text(TextId::Clear, en).into());
    ui.set_log_retry(text(TextId::Retry, en).into());
    ui.set_log_enabled(enabled);
    ui.set_log_failed(status.failure.is_some());
    let mut body = text(if enabled { TextId::On } else { TextId::Off }, en).to_owned();
    if status.failure.is_some() {
        body.push('\n');
        body.push_str(text(TextId::Error, en));
    }
    body.push_str(&if en {
        format!(
            "\nWritten this run: {}; cleared: {}; dropped: {}.",
            status.written, status.cleared, status.dropped
        )
    } else {
        format!(
            "\n本次运行已记录：{}；已清理：{}；丢弃：{}。",
            status.written, status.cleared, status.dropped
        )
    });
    ui.set_log_status(body.into());
}
pub fn handle(action: &str, state: &Rc<RefCell<AppState>>, ui: &SettingsWindow) -> bool {
    if !matches!(
        action,
        "log-ready" | "log-clear" | "log-clear-confirm" | "log-retry"
    ) {
        return false;
    }
    let state = state.borrow();
    match action {
        "log-ready" => {}
        "log-clear" if ui.get_modal() == 0 && ui.get_guide_step() < 0 && !ui.get_font_open() => {
            ui.set_modal_title(text(TextId::Title, state.en()).into());
            ui.set_modal_body(text(TextId::Confirm, state.en()).into());
            ui.invoke_show_modal(7);
        }
        "log-clear-confirm" if ui.get_modal() == 7 => {
            ui.invoke_show_modal(0);
            state.log.borrow_mut().clear();
        }
        "log-retry" if ui.get_modal() == 0 && !ui.get_font_open() => {
            state.log.borrow_mut().record(Event::Retry)
        }
        _ => {}
    }
    sync(&state, ui);
    true
}
