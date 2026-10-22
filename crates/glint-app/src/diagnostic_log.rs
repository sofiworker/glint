//! Settings adapter for opt-in event logs. Never forwards user/error strings to the writer.
use crate::{resident::AppState, settings::SettingsWindow};
use glint_platform::diagnostic_log::{DiagnosticLog, Event};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

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
    Developer,
}
fn text(id: TextId, en: bool) -> &'static str {
    use TextId::*;
    match (id, en) {
        (Developer, false) => {
            "本次开发启动通过 GLINT_DIAGNOSTICS=1 临时开启日志；不修改已保存开关。移除该环境变量后恢复设置控制。"
        }
        (Developer, true) => {
            "GLINT_DIAGNOSTICS=1 enables logs for this developer run without changing saved preferences. Remove it to use the saved setting."
        }
        (Help, false) => {
            "使用 tracing 记录毫秒时间、进程号与固定事件代码，不含按键内容、图像、正文、路径、窗口标题或配置值。位置：配置目录 / logs；日志等级、单文件大小和备份份数保存后生效；file-rotate 保留当前 events.log 和配置数量的轮转备份。关闭停止新记录，已有文件保留至手动清理。"
        }
        (Help, true) => {
            "Uses tracing to record millisecond timestamps, process IDs and fixed event codes, never keys, images, text, paths, window titles or configuration values. Location: configuration directory / logs; level, file size and backup count apply after Save; file-rotate retains events.log plus the configured number of backups. Turning logging off preserves existing files until cleared."
        }
        (On, false) => "记录已开启。",
        (On, true) => "Logging is on.",
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
            "仅清理 logs 目录中的当前日志和轮转备份，不删除截图、贴图会话或配置。开启记录时，之后的事件会重新创建日志。"
        }
        (Confirm, true) => {
            "Delete only the current log and rotated backups in the logs directory. Captures, pin sessions and configuration are preserved. If logging is on, later events create logs again."
        }
    }
}
pub fn attach(mut log: DiagnosticLog, ui: &SettingsWindow) -> RefCell<DiagnosticLog> {
    let weak = ui.as_weak();
    log.set_notifier(move || {
        let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("log-ready".into()));
    });
    RefCell::new(log)
}
pub fn configure(log: &mut DiagnosticLog, privacy: &glint_core::config::Privacy) {
    use glint_core::config::LogLevel;
    use glint_platform::diagnostic_log::{Level, Options};
    log.set_options(Options {
        level: match privacy.log_level {
            LogLevel::Error => Level::Error,
            LogLevel::Warn => Level::Warn,
            LogLevel::Info => Level::Info,
            LogLevel::Debug => Level::Debug,
            LogLevel::Trace => Level::Trace,
        },
        file_size_kib: privacy.log_file_size_kib,
        backups: privacy.log_backups,
    });
    log.configure(privacy.diagnostics);
}
pub fn sync(state: &AppState, ui: &SettingsWindow) {
    let mut log = state.log.borrow_mut();
    let privacy = &state.session.preferences.saved().privacy;
    configure(&mut log, privacy);
    let enabled = log.enabled();
    let status = log.status();
    let en = state.en();
    ui.set_log_help(text(TextId::Help, en).into());
    ui.set_log_clear(text(TextId::Clear, en).into());
    ui.set_log_retry(text(TextId::Retry, en).into());
    ui.set_log_enabled(enabled);
    ui.set_log_failed(status.failure.is_some());
    let mut body = text(if enabled { TextId::On } else { TextId::Off }, en).to_owned();
    if log.forced() {
        body.push('\n');
        body.push_str(text(TextId::Developer, en));
    }
    body.push_str(&if en {
        format!("\nSaved: {:?}; {} KiB per file; {} backups + 1 current = {} files. Reducing backups removes excess files while logging is active, or on the next write when enabled.", privacy.log_level, privacy.log_file_size_kib, privacy.log_backups, u16::from(privacy.log_backups) + 1)
    } else {
        format!("\n已保存：{:?}；单文件 {} KiB；{} 份备份 ＋ 1 份当前，共 {} 份。降低份数时，日志开启则清理多余备份；关闭则在下次启用写入时清理。", privacy.log_level, privacy.log_file_size_kib, privacy.log_backups, u16::from(privacy.log_backups) + 1)
    });
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
