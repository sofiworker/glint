//! Explicit read-only refresh; no image/content access or persistent log.
use crate::{
    messages::{self, Message},
    resident::AppState,
    settings::SettingsWindow,
};
use slint::ComponentHandle;
use std::sync::mpsc::{Receiver, sync_channel};
pub struct Report {
    pub environment: Result<glint_platform::DesktopEnvironment, String>,
    pub resources: Result<glint_platform::ProcessResources, String>,
}
#[derive(Default)]
pub struct State {
    pub last: Option<Report>,
    pub error: Option<String>,
    pub receive: Option<Receiver<Report>>,
    pub completed: u64,
}
pub fn start(state: &mut AppState, ui: &SettingsWindow) {
    if state.diagnostics.receive.is_some() {
        return;
    }
    let (send, receive) = sync_channel(1);
    let weak = ui.as_weak();
    match std::thread::Builder::new()
        .name("glint-diagnostics".into())
        .spawn(move || {
            let report = Report {
                environment: glint_platform::desktop_environment(),
                resources: glint_platform::process_resources(),
            };
            let _ = send.send(report);
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("diagnostics-ready".into()));
        }) {
        Ok(_) => {
            state.diagnostics.receive = Some(receive);
            state.diagnostics.error = None;
        }
        Err(error) => state.diagnostics.error = Some(error.to_string()),
    }
    sync(state, ui);
}
pub fn ready(state: &mut AppState, ui: &SettingsWindow) {
    let Some(receive) = state.diagnostics.receive.take() else {
        return;
    };
    match receive.try_recv() {
        Ok(report) => {
            let errors: Vec<_> = [
                report.environment.as_ref().err(),
                report.resources.as_ref().err(),
            ]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
            if errors.is_empty() {
                state
                    .log
                    .borrow_mut()
                    .record(glint_platform::diagnostic_log::Event::DiagnosticsReady);
                state.diagnostics.last = Some(report);
                state.diagnostics.error = None;
            } else {
                state
                    .log
                    .borrow_mut()
                    .record(glint_platform::diagnostic_log::Event::DiagnosticsFailed);
                if state.diagnostics.last.is_none() {
                    state.diagnostics.last = Some(report);
                }
                state.diagnostics.error = Some(errors.join("; "));
            }
            state.diagnostics.completed += 1;
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => state.diagnostics.receive = Some(receive),
        Err(error) => state.diagnostics.error = Some(error.to_string()),
    }
    sync(state, ui);
}
pub fn sync(state: &AppState, ui: &SettingsWindow) {
    let english = state.en();
    let tr = |id| messages::text(id, english);
    ui.set_diagnostics_title(tr(Message::DiagnosticsTitle).into());
    ui.set_diagnostics_hint(tr(Message::DiagnosticsHint).into());
    ui.set_diagnostics_refresh(tr(Message::DiagnosticsRefresh).into());
    ui.set_diagnostics_busy(state.diagnostics.receive.is_some());
    let mut lines = vec![
        tr(Message::DiagnosticsRenderer).to_owned(),
        tr(Message::DiagnosticsBackend).to_owned(),
    ];
    lines.push(
        if state
            .lifecycle
            .guard
            .as_ref()
            .is_some_and(|s| s.is_running())
        {
            tr(Message::DiagnosticsSubscribed).to_owned()
        } else {
            tr(Message::DiagnosticsUnavailable).to_owned()
        },
    );
    if let Some(report) = &state.diagnostics.last {
        match &report.environment {
            Ok(env) => {
                lines.push(format!(
                    "{}: {} · DPI awareness: {} · {}: {}",
                    tr(Message::DiagnosticsDisplays).to_owned(),
                    env.displays.len(),
                    env.dpi_awareness,
                    tr(Message::DiagnosticsRemote).to_owned(),
                    env.remote_session
                ));
                for (i, d) in env.displays.iter().enumerate() {
                    let b = d.bounds;
                    let w = d.work_area;
                    lines.push(format!(
                        "#{} {} ({}, {})–({}, {}) · {} ({}, {})–({}, {}) · DPI {}×{}{}",
                        i + 1,
                        tr(Message::DiagnosticsBounds).to_owned(),
                        b.left,
                        b.top,
                        b.right,
                        b.bottom,
                        tr(Message::DiagnosticsWorkArea).to_owned(),
                        w.left,
                        w.top,
                        w.right,
                        w.bottom,
                        d.effective_dpi.0,
                        d.effective_dpi.1,
                        if d.primary {
                            tr(Message::DiagnosticsPrimary).to_owned()
                        } else {
                            String::new()
                        }
                    ));
                }
            }
            Err(error) => lines.push(format!(
                "{}: {error}",
                tr(Message::DiagnosticsDisplayFailed).to_owned()
            )),
        }
        match &report.resources {
            Ok(r) => {
                lines.push(format!(
                    "{}: {:.1} MiB · {}: {:.1} MiB",
                    tr(Message::DiagnosticsWorkingSet).to_owned(),
                    r.working_set_bytes as f64 / 1048576.,
                    tr(Message::DiagnosticsPrivate).to_owned(),
                    r.private_bytes as f64 / 1048576.
                ));
                lines.push(format!(
                    "{}: {} · GDI: {} · USER: {}",
                    tr(Message::DiagnosticsHandles).to_owned(),
                    r.handles,
                    r.gdi_objects,
                    r.user_objects
                ));
                lines.push(format!(
                    "{}: {:.3} s",
                    tr(Message::DiagnosticsCpu).to_owned(),
                    r.cpu_100ns as f64 / 10000000.
                ));
            }
            Err(error) => lines.push(format!(
                "{}: {error}",
                tr(Message::DiagnosticsResourceFailed).to_owned()
            )),
        }
    } else {
        lines.push(tr(Message::DiagnosticsEmpty).to_owned());
    }
    if let Some(error) = &state.diagnostics.error {
        lines.push(format!("{}: {error}", tr(Message::DiagnosticsFailed)));
    }
    if state.diagnostics.receive.is_some() {
        lines.push(tr(Message::DiagnosticsLoading).to_owned());
    }
    ui.set_diagnostics_body(lines.join("\n\n").into());
}
