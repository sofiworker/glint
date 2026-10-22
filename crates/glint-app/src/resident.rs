//! Native resident coordinator: settings, tray, bindings and independent capture sessions.
use crate::{
    messages::{self, Message},
    preferences::SettingsSession,
    settings::{self, SettingRow, SettingsWindow},
};
use glint_core::config::{Config, KeyChord};
use glint_platform::{HotkeyBinding, HotkeyEvent, HotkeyService, InstanceStart, ResidentInstance};
use slint::{
    ComponentHandle, ModelRc, VecModel,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
    path::PathBuf,
    rc::Rc,
};
use tray_icon::{
    TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

pub struct AppState {
    pub log: Rc<RefCell<glint_platform::diagnostic_log::DiagnosticLog>>,
    pub output_settings: crate::output_settings::State,
    pub diagnostics: crate::runtime_diagnostics::State,
    pub lifecycle: crate::lifecycle::State,
    pub pin_session: crate::pin_session::State,
    pub workbench_pending: bool,
    pub local_path: Option<PathBuf>,
    pub local_dialog: bool,
    pub local_edit: bool,
    pub transfer: crate::config_transfer::State,
    pub guide_practice: bool,
    pub guide_succeeded: bool,
    pub guide_pending_open: bool,
    pub countdown: Option<crate::capture_delay::Run>,
    pub delay_elapsed: bool,
    pub delay_ticket: u64,
    pub last_region: Option<glint_core::region_memory::RememberedRegion>,
    pub repeat_capture: bool,
    pub color_picker: bool,
    pub screen_capture: Option<glint_core::screen_capture::ScreenTarget>,
    pub edit_pin: Option<u64>,
    pub clipboard_pin_pending: bool,
    pub clipboard_pin_job: Option<crate::clipboard_pin::Job>,
    pub pins: Rc<RefCell<crate::pins::Manager>>,
    pub capture_focus: Option<glint_platform::FocusTarget>,
    pub capture_settings_visible: bool,
    pub scale_preview: crate::scale_preview::State,
    pub scroll_capture: Option<crate::scroll_capture::Run>,
    pub screen_recording: Option<crate::recording::Run>,
    pub capture: Option<crate::probe::CaptureProbe>,
    pub capturing: bool,
    pub capture_output: Option<PathBuf>,
    pub capture_pending: bool,
    pub window_capture: bool,
    pub capture_result: Option<std::sync::mpsc::Receiver<Result<crate::pin_edit::Input, String>>>,
    pub session: SettingsSession,
    pub rows: Rc<VecModel<SettingRow>>,
    pub languages: Vec<String>,
    pub feedback: String,
    pub recording: String,
    pub candidate: String,
    pub quit_pending: bool,
    pub tray_labels: Option<TrayLabels>,
}
pub struct TrayLabels {
    workbench: MenuItem,
    screen_recording: MenuItem,
    repeat: MenuItem,
    clipboard: MenuItem,
    open: MenuItem,
    capture: MenuItem,
    window: MenuItem,
    all_screens: MenuItem,
    current_screen: MenuItem,
    color_picker: MenuItem,
    toggle: MenuItem,
    rescue: MenuItem,
    quit: MenuItem,
}
impl AppState {
    pub fn en(&self) -> bool {
        settings::english(
            self.session.preferences.draft.general.language,
            &self.languages,
        )
    }
    fn message(&self, id: Message) -> String {
        messages::text(id, self.en()).into()
    }
    pub fn sync(&self, ui: &SettingsWindow) {
        ui.set_recording_label(
            settings::fields::label(self.en(), "打开录屏…", "Open screen recorder…").into(),
        );
        crate::ui_font::set_saved_choice(self.session.preferences.saved().appearance.ui_font);
        crate::diagnostic_log::sync(self, ui);
        crate::output_settings::sync(self, ui);
        crate::runtime_diagnostics::sync(self, ui);
        if let Some(capture) = &self.capture {
            capture.set_service_notice(if self.pins.borrow().recovery_stopped() {
                messages::text(Message::ShortcutServiceStopped, capture.get_english()).into()
            } else {
                "".into()
            });
        }

        self.pins
            .borrow_mut()
            .set_snap(self.session.preferences.saved().pins.snap);
        self.pins
            .borrow_mut()
            .set_wheel(self.session.preferences.saved().pins.wheel);
        self.pins.borrow_mut().configure_recovery(
            &self.session.preferences.saved().shortcuts.rescue,
            chord_binding(&self.session.preferences.saved().shortcuts.rescue, 6)
                .ok()
                .and_then(|v| v.into_iter().next()),
        );
        self.pins
            .borrow_mut()
            .set_output_options(&self.session.preferences.saved().output);
        ui.set_can_save(self.session.preferences.is_dirty() || self.session.repair_pending);
        if let Some(TrayLabels {
            workbench,
            screen_recording,
            repeat,
            clipboard,
            open,
            capture,
            window,
            all_screens,
            current_screen,
            color_picker,
            toggle,
            rescue,
            quit,
        }) = &self.tray_labels
        {
            let english = settings::english(
                self.session.preferences.saved().general.language,
                &self.languages,
            );
            workbench.set_text(messages::text(Message::Workbench, english));
            screen_recording.set_text(settings::fields::label(english, "录屏…", "Record screen…"));
            clipboard.set_text(settings::fields::label(
                english,
                "贴出剪贴板图像",
                "Pin clipboard image",
            ));
            open.set_text(settings::fields::label(english, "设置…", "Settings…"));
            capture.set_text(settings::fields::label(
                english,
                "区域截图",
                "Capture region",
            ));
            window.set_text(settings::fields::label(
                english,
                "窗口截图",
                "Capture window",
            ));
            all_screens.set_text(messages::text(Message::AllScreensCapture, english));
            color_picker.set_text(messages::text(Message::ColorPickerEntry, english));
            current_screen.set_text(messages::text(Message::CurrentScreenCapture, english));
            repeat.set_text(settings::fields::label(
                english,
                "重复上次区域",
                "Repeat last region",
            ));
            quit.set_text(settings::fields::label(english, "退出拾光", "Quit Glint"));
            toggle.set_text(settings::fields::label(
                english,
                "隐藏 / 恢复全部贴图",
                "Hide / restore all pins",
            ));
            rescue.set_text(settings::fields::label(
                english,
                "恢复贴图并取消穿透",
                "Rescue desktop pins",
            ));
        }
        if let Err(error) = settings::sync(
            ui,
            &self.session.preferences.draft,
            &self.rows,
            &self.languages,
            self.session.writable && !self.pin_session.busy(),
            self.session.preferences.is_dirty(),
        ) {
            ui.set_status(error.into());
            return;
        }
        if let Err(error) = crate::onboarding::sync(self, ui) {
            ui.set_status(error.into());
            return;
        }
        crate::config_transfer::sync(self, ui);
        crate::pin_session::sync(self, ui);
        crate::local_image::sync(self, ui);
        if !self.feedback.is_empty() {
            ui.set_status(self.feedback.as_str().into());
        } else if !self.session.notice.is_empty() {
            ui.set_status(
                self.message(match self.session.notice {
                    "config.recovered_backup" => Message::RecoveredBackup,
                    "config.recovered_defaults" => Message::RecoveredDefaults,
                    "config.unsupported_version" => Message::UnsupportedConfig,
                    _ => Message::UnreadableConfig,
                })
                .into(),
            );
        }
        crate::pin_session::sync_status(self, ui);
    }
}

pub fn settings_binding(config: &Config) -> Result<Vec<HotkeyBinding>, String> {
    chord_binding(&config.shortcuts.settings, 8)
}
fn capture_layout(
    snapshot: &glint_platform::DesktopSnapshot,
) -> Option<glint_core::region_memory::CaptureLayout> {
    use glint_core::region_memory::{CaptureLayout, DisplayGeometry};
    CaptureLayout::new(
        (snapshot.frame.x, snapshot.frame.y),
        (snapshot.frame.width, snapshot.frame.height),
        snapshot
            .displays
            .iter()
            .map(|d| DisplayGeometry {
                bounds: [d.bounds.left, d.bounds.top, d.bounds.right, d.bounds.bottom],
                dpi: d.effective_dpi,
            })
            .collect(),
    )
}
fn runtime_bindings(config: &Config) -> Result<Vec<HotkeyBinding>, String> {
    let mut bindings = settings_binding(config)?;
    bindings.extend(chord_binding(&config.shortcuts.capture, 1)?);
    bindings.extend(chord_binding(&config.shortcuts.window, 2)?);
    bindings.extend(chord_binding(&config.shortcuts.last_region, 3)?);
    bindings.extend(chord_binding(&config.shortcuts.pin_clipboard, 4)?);
    bindings.extend(chord_binding(&config.shortcuts.toggle_pins, 5)?);
    bindings.extend(chord_binding(&config.shortcuts.rescue, 6)?);
    bindings.extend(chord_binding(&config.shortcuts.color_picker, 7)?);
    Ok(bindings)
}
fn chord_binding(text: &str, action: u32) -> Result<Vec<HotkeyBinding>, String> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    let chord = KeyChord::parse(text).ok_or("Invalid settings shortcut")?;
    let key = match chord.key.as_str() {
        "ESCAPE" => 0x1b,
        "SPACE" => 0x20,
        "TAB" => 9,
        "ENTER" => 13,
        "BACKSPACE" => 8,
        "DELETE" => 0x2e,
        "INSERT" => 0x2d,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        key if key.len() > 1 && key.starts_with('F') => {
            0x6f + key[1..]
                .parse::<u32>()
                .map_err(|_| "Invalid function key")?
        }
        key if key.len() == 1 => u32::from(key.as_bytes()[0]),
        _ => return Err("Unsupported settings shortcut".into()),
    };
    Ok(vec![HotkeyBinding {
        action,
        modifiers: u32::from(chord.alt)
            | (u32::from(chord.control) << 1)
            | (u32::from(chord.shift) << 2)
            | (u32::from(chord.meta) << 3),
        key,
    }])
}

fn recorded_binding<'a>(config: &'a mut Config, key: &str) -> Option<&'a mut String> {
    match key {
        "shortcuts.settings" => Some(&mut config.shortcuts.settings),
        "shortcuts.capture" => Some(&mut config.shortcuts.capture),
        "shortcuts.window" => Some(&mut config.shortcuts.window),
        "shortcuts.last_region" => Some(&mut config.shortcuts.last_region),
        "shortcuts.pin_clipboard" => Some(&mut config.shortcuts.pin_clipboard),
        "shortcuts.toggle_pins" => Some(&mut config.shortcuts.toggle_pins),
        "shortcuts.rescue" => Some(&mut config.shortcuts.rescue),
        "shortcuts.color_picker" => Some(&mut config.shortcuts.color_picker),
        _ => None,
    }
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut directory = None;
    let mut explicit_settings = false;
    let mut background = false;
    let mut smoke = false;
    let mut scale_smoke = false;
    let mut motion_smoke = false;
    let mut budget_smoke = false;
    let mut lifecycle_smoke = false;
    let mut diagnostics_smoke = false;
    let mut log_smoke = false;
    let mut exclusion_smoke = false;
    let mut input_smoke = false;
    let mut fonts_smoke = false;
    let mut output_settings_smoke = false;
    let mut output_conflict_smoke = false;
    let mut config_dialog_smoke = false;
    let mut local_dialog_smoke = false;
    let mut resource_smoke = false;
    let mut resource_4k = false;
    let mut endurance = false;
    let mut settings_policy_smoke = false;
    let mut shortcuts_smoke = false;
    let mut session_probe = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--pin-session-probe" => {
                index += 1;
                let mode = args.get(index).ok_or("Missing session probe mode")?;
                if !matches!(
                    mode.as_str(),
                    "seed"
                        | "off"
                        | "restore"
                        | "failure"
                        | "without"
                        | "clear"
                        | "corrupt"
                        | "config-recovery"
                        | "empty"
                ) {
                    return Err("Unknown session probe mode".into());
                }
                session_probe = Some(mode.clone());
                explicit_settings = true;
            }
            "--settings" => explicit_settings = true,
            "--m5-input-smoke" | "--m5-dialog-input-smoke" | "--m5-dialog-conflict-input-smoke" => {
                if !cfg!(feature = "input-probe") {
                    return Err(
                        "--m5-input-smoke requires the explicit input-probe Cargo feature".into(),
                    );
                }
                input_smoke = true;
                smoke = true;
                index += 1;
                directory = Some(PathBuf::from(
                    args.get(index)
                        .ok_or("Input probe requires an empty directory")?,
                ));
            }
            "--background" => background = true,
            "--config-dir"
            | "--m1-smoke"
            | "--m4-scale-smoke"
            | "--m4-motion-smoke"
            | "--m4-preview-budget-smoke"
            | "--m4-lifecycle-smoke"
            | "--m4-diagnostics-smoke"
            | "--m4-log-smoke"
            | "--m4-exclusion-smoke"
            | "--m4-fonts-smoke"
            | "--m4-output-settings-smoke"
            | "--m4-config-dialog-smoke"
            | "--m4-local-dialog-smoke"
            | "--m5-resources"
            | "--m5-resources-4k"
            | "--m5-endurance"
            | "--m4-settings-policy-smoke"
            | "--m4-shortcuts-smoke"
            | "--m4-output-conflict-smoke" => {
                scale_smoke = args[index] == "--m4-scale-smoke";
                shortcuts_smoke = args[index] == "--m4-shortcuts-smoke";
                motion_smoke = args[index] == "--m4-motion-smoke";
                budget_smoke = args[index] == "--m4-preview-budget-smoke";
                output_settings_smoke = args[index] == "--m4-output-settings-smoke";
                output_conflict_smoke = args[index] == "--m4-output-conflict-smoke";
                config_dialog_smoke = args[index] == "--m4-config-dialog-smoke";
                local_dialog_smoke = args[index] == "--m4-local-dialog-smoke";
                resource_4k = args[index] == "--m5-resources-4k";
                endurance = args[index] == "--m5-endurance";
                settings_policy_smoke = args[index] == "--m4-settings-policy-smoke";
                resource_smoke = args[index] == "--m5-resources" || resource_4k || endurance;
                fonts_smoke = args[index] == "--m4-fonts-smoke";
                diagnostics_smoke = args[index] == "--m4-diagnostics-smoke";
                log_smoke = args[index] == "--m4-log-smoke";
                exclusion_smoke = args[index] == "--m4-exclusion-smoke";
                lifecycle_smoke = args[index] == "--m4-lifecycle-smoke";
                smoke = args[index] == "--m1-smoke"
                    || scale_smoke
                    || motion_smoke
                    || budget_smoke
                    || lifecycle_smoke
                    || diagnostics_smoke
                    || log_smoke
                    || exclusion_smoke
                    || fonts_smoke
                    || output_settings_smoke
                    || config_dialog_smoke
                    || local_dialog_smoke
                    || resource_smoke
                    || settings_policy_smoke
                    || shortcuts_smoke
                    || output_conflict_smoke;
                index += 1;
                directory = Some(PathBuf::from(
                    args.get(index)
                        .ok_or("Configuration directory argument missing")?,
                ));
            }
            other => return Err(format!("Unknown resident argument: {other}").into()),
        }
        index += 1;
    }
    if session_probe.is_some() && (smoke || directory.is_none()) {
        return Err("Session probes require an explicit isolated --config-dir and cannot combine with --m1-smoke".into());
    }
    if input_smoke && args.iter().filter(|arg| arg.starts_with("--m")).count() != 1 {
        return Err("Input probes require exactly one QA mode".into());
    }
    let directory = match directory {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()?.join(path)
            }
        }
        None => glint_platform::configuration_directory()?,
    };
    if smoke {
        std::fs::create_dir_all(&directory)?;
        if std::fs::read_dir(&directory)?.next().is_some() {
            return Err(
                "Native validation requires an empty directory; existing files are never overwritten"
                    .into(),
            );
        }
    }
    if let Some(mode) = session_probe.as_deref() {
        let marker = directory.join(".pin-session-probe");
        if matches!(mode, "seed" | "off") {
            std::fs::create_dir_all(&directory)?;
            if std::fs::read_dir(&directory)?.next().is_some() {
                return Err("Session seed/off probes require an empty isolated directory".into());
            }
            std::fs::write(&marker, b"Glint session probe v1")?;
        } else if std::fs::read(&marker).ok().as_deref()
            != Some(b"Glint session probe v1".as_slice())
        {
            return Err("Session probes require a directory initialized by the seed probe".into());
        }
    }
    let directory = directory.canonicalize().unwrap_or(directory);
    let session = SettingsSession::open(&directory.join("config.toml"));
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        // Slint invokes this before applying Window title/properties. Settings and
        // capture explicitly focus their peers; a new desktop pin never does.
        .with_winit_window_attributes_hook(crate::motion::window_attributes)
        .with_winit_custom_application_handler(crate::motion::WindowDiagnostics)
        .select()?;
    let ui = crate::motion::create_settings(SettingsWindow::new)?;
    crate::font_picker::install(&ui);
    let namespace = instance_namespace(&directory)?;
    let weak = ui.as_weak();
    let _instance = match ResidentInstance::start(&namespace, move || {
        let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("open".into()));
    })? {
        InstanceStart::Activated => return Ok(()),
        InstanceStart::Primary(instance) => instance,
    };
    // Only the primary instance may open and rotate the shared log files.
    let mut diagnostic =
        glint_platform::diagnostic_log::DiagnosticLog::new(directory.join("logs"), || {});
    diagnostic.install_tracing();
    crate::diagnostic_log::configure(&mut diagnostic, &session.preferences.saved().privacy);
    if std::env::var("GLINT_DIAGNOSTICS").as_deref() == Ok("1") {
        diagnostic.enable_for_run();
    }
    diagnostic.record(glint_platform::diagnostic_log::Event::Startup);

    let guide_initial =
        !smoke && crate::onboarding::initial(&session, explicit_settings, background);
    if guide_initial {
        ui.set_guide_step(0);
    }
    let show_initial = smoke
        || guide_initial
        || explicit_settings
        || !session.notice.is_empty()
        || (!background && (session.is_new || !session.preferences.saved().general.start_in_tray));
    let state = Rc::new(RefCell::new(AppState {
        log: Rc::new(crate::diagnostic_log::attach(diagnostic, &ui)),
        output_settings: Default::default(),
        diagnostics: Default::default(),
        lifecycle: Default::default(),
        pin_session: Default::default(),
        workbench_pending: false,
        local_path: None,
        local_dialog: false,
        local_edit: false,
        transfer: Default::default(),
        guide_practice: false,
        guide_succeeded: false,
        guide_pending_open: false,
        last_region: None,
        repeat_capture: false,
        color_picker: false,
        screen_capture: None,
        edit_pin: None,
        clipboard_pin_pending: false,
        clipboard_pin_job: None,
        pins: Rc::new(RefCell::new(crate::pins::Manager::default())),
        capture_focus: None,
        capture_settings_visible: false,
        scale_preview: Default::default(),
        scroll_capture: None,
        screen_recording: None,
        countdown: None,
        delay_elapsed: false,
        delay_ticket: 0,
        capture: None,
        capturing: false,
        capture_output: (smoke && !input_smoke).then(|| directory.join("resident-capture.png")),
        capture_pending: false,
        window_capture: false,
        capture_result: None,
        session,
        rows: Rc::new(VecModel::default()),
        languages: glint_platform::preferred_ui_languages().unwrap_or_default(),
        feedback: String::new(),
        recording: String::new(),
        candidate: String::new(),
        quit_pending: false,
        tray_labels: None,
    }));
    ui.set_rows(ModelRc::from(state.borrow().rows.clone()));
    let editor_ui = ui.as_weak();
    state
        .borrow()
        .pins
        .borrow_mut()
        .set_edit_handler(Rc::new(move |id| {
            if let Some(ui) = editor_ui.upgrade() {
                ui.invoke_action(format!("edit-pin:{id}").into());
            }
        }));
    let weak = ui.as_weak();
    let hotkeys = Rc::new(HotkeyService::new(vec![], move |event| {
        glint_platform::diagnostic_log::emit(match &event {
            HotkeyEvent::Pressed(_) => glint_platform::diagnostic_log::Event::HotkeyPressed,
            HotkeyEvent::Error(_) => glint_platform::diagnostic_log::Event::HotkeyFailed,
        });
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if ui.get_modal() == 3
                && let HotkeyEvent::Pressed(action) = &event
                && *action != 6
            {
                ui.invoke_registered_shortcut(*action as i32);
                return;
            }
            match event {
                HotkeyEvent::Pressed(2) => ui.invoke_action("window-capture".into()),
                HotkeyEvent::Pressed(3) => ui.invoke_action("repeat-capture".into()),
                HotkeyEvent::Pressed(1) => ui.invoke_action("capture".into()),
                HotkeyEvent::Pressed(8) => ui.invoke_action("open".into()),
                HotkeyEvent::Pressed(4) => ui.invoke_action("clipboard-pin".into()),
                HotkeyEvent::Pressed(5) => ui.invoke_action("toggle-pins".into()),
                HotkeyEvent::Pressed(7) => ui.invoke_action("color-picker".into()),
                HotkeyEvent::Pressed(6) => ui.invoke_action("rescue-pins".into()),
                HotkeyEvent::Error(_) => ui.invoke_action("hotkey-error".into()),
                _ => {}
            }
        });
    })?);
    let recovery_service = Rc::downgrade(&hotkeys);
    state
        .borrow()
        .pins
        .borrow_mut()
        .set_recovery_probe(Rc::new(move || {
            recovery_service
                .upgrade()
                .ok_or("Hotkey service unavailable")?
                .bindings()
        }));
    {
        let mut state = state.borrow_mut();
        if runtime_bindings(state.session.preferences.saved())
            .and_then(|bindings| hotkeys.replace(bindings))
            .is_err()
        {
            state.feedback = state.message(Message::ShortcutRegistrationFailed);
        }
        state.sync(&ui);
    }
    let shared = state.clone();
    let weak = ui.as_weak();
    ui.on_choose_page(move |category| {
        if weak.upgrade().is_some_and(|ui| ui.get_font_open()) {
            return;
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_category(category.clamp(0, 12));
            shared.borrow().sync(&ui);
        }
    });
    let shared = state.clone();
    let weak = ui.as_weak();
    ui.on_change(move |key, value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut state = shared.borrow_mut();
        if !state.session.writable || state.pin_session.busy() {
            return;
        }
        if ui.get_font_open() || ui.get_modal() != 0 {
            return;
        }
        let previous_scale = state.session.preferences.draft.appearance.scale;
        match settings::fields::change(
            &state.session.preferences.draft,
            key.as_str(),
            value.as_str(),
            state.en(),
        ) {
            Ok(config) => {
                state.session.preferences.draft = config;
                state.feedback.clear();
            }
            Err(_) => state.feedback = state.message(Message::OptionUnavailable),
        }
        state.sync(&ui);
        if key == "appearance.scale"
            && state.session.preferences.draft.appearance.scale != previous_scale
        {
            crate::scale_preview::begin(&mut state, &shared, &ui, previous_scale);
        }
    });
    let shared = state.clone();
    let weak = ui.as_weak();
    ui.on_toolbar_command(move |command, row, value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut state = shared.borrow_mut();
        if !state.session.writable || state.pin_session.busy() || ui.get_modal() != 0 {
            return;
        }
        if crate::toolbar::edit(
            &mut state.session.preferences.draft.toolbar,
            command.as_str(),
            row,
            value,
        )
        .is_ok()
        {
            state.feedback.clear();
        } else {
            state.feedback = state.message(Message::OptionUnavailable);
        }
        state.sync(&ui);
    });
    let shared = state.clone();
    let weak = ui.as_weak();
    ui.on_record(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut state = shared.borrow_mut();
        if !state.session.writable
            || state.pin_session.busy()
            || !matches!(
                key.as_str(),
                "shortcuts.settings"
                    | "shortcuts.capture"
                    | "shortcuts.window"
                    | "shortcuts.last_region"
                    | "shortcuts.pin_clipboard"
                    | "shortcuts.toggle_pins"
                    | "shortcuts.rescue"
                    | "shortcuts.color_picker"
            )
        {
            return;
        }
        let can_clear = key != "shortcuts.rescue";
        ui.set_recording_key(key.clone());
        ui.set_shortcut_value("".into());
        ui.set_shortcut_invalid(false);
        state.recording = key.into();
        state.candidate.clear();
        ui.set_can_clear(can_clear);
        ui.set_can_accept(false);
        settings::modal(
            &ui,
            3,
            state.en(),
            &state.message(Message::RecordShortcutBody),
        );
    });
    let shared = state.clone();
    let weak = ui.as_weak();
    ui.on_shortcut_input(move |text, control, alt, shift, meta| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_modal() != 3 {
            return;
        }
        if text == "\u{1b}" {
            ui.invoke_action("continue".into());
            return;
        }
        let Some(key) = key_name(text.as_str()) else {
            return;
        };
        let mut parts = Vec::new();
        if control {
            parts.push("Ctrl".to_owned());
        }
        if alt {
            parts.push("Alt".to_owned());
        }
        if shift {
            parts.push("Shift".to_owned());
        }
        if meta {
            parts.push("Win".to_owned());
        }
        parts.push(key);
        let mut state = shared.borrow_mut();
        update_shortcut_candidate(&mut state, &ui, parts.join("+"));
    });
    let shared = state.clone();
    let weak = ui.as_weak();
    ui.on_registered_shortcut(move |action| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_modal() != 3 || !ui.get_shortcut_recorder_focused() {
            return;
        }
        let mut state = shared.borrow_mut();
        let bindings = state.session.preferences.saved().shortcuts.bindings();
        let Some((_, Some(chord))) = usize::try_from(action)
            .ok()
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| bindings.get(index))
        else {
            return;
        };
        let candidate = (*chord).to_owned();
        update_shortcut_candidate(&mut state, &ui, candidate);
    });
    let shared = state.clone();
    let weak = ui.as_weak();
    let registrations = hotkeys.clone();
    let action_directory = directory.clone();
    ui.on_action(move |action| {
        if matches!(
            action.as_str(),
            "capture"
                | "window-capture"
                | "repeat-capture"
                | "all-screens-capture"
                | "current-screen-capture"
                | "color-picker"
                | "local-image"
                | "quit"
                | "resident-exit"
        ) || action.starts_with("edit-pin:")
        {
            crate::completion::dismiss();
        }
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if matches!(
            action.as_str(),
            "capture"
                | "window-capture"
                | "repeat-capture"
                | "all-screens-capture"
                | "current-screen-capture"
        ) {
            glint_platform::diagnostic_log::emit(
                glint_platform::diagnostic_log::Event::CaptureRequested,
            );
        }
        if crate::recording::handle(action.as_str(), &shared, &ui) {
            return;
        }
        if crate::scroll_capture::handle(action.as_str(), &shared, &ui) {
            return;
        }
        if crate::scale_preview::handle(action.as_str(), &shared, &ui) {
            return;
        }
        if crate::output_settings::handle(action.as_str(), &shared, &ui) {
            return;
        }
        if crate::diagnostic_log::handle(action.as_str(), &shared, &ui) {
            return;
        }
        if ui.get_font_open() && matches!(action.as_str(), "close" | "quit" | "cancel") {
            ui.invoke_font_command("cancel".into(), "".into());
        }
        if ui.get_font_open()
            && matches!(
                action.as_str(),
                "save"
                    | "save-leave"
                    | "reset"
                    | "confirm-reset"
                    | "config-import"
                    | "config-export"
                    | "guide-open"
            )
        {
            return;
        }
        if action == "diagnostics-refresh" {
            crate::runtime_diagnostics::start(&mut shared.borrow_mut(), &ui);
            return;
        }
        if action == "diagnostics-ready" {
            crate::runtime_diagnostics::ready(&mut shared.borrow_mut(), &ui);
            return;
        }
        if crate::lifecycle::handle(action.as_str(), &mut shared.borrow_mut(), &ui) {
            return;
        }
        if shared.borrow().lifecycle.blocked()
            && (matches!(
                action.as_str(),
                "capture"
                    | "window-capture"
                    | "repeat-capture"
                    | "all-screens-capture"
                    | "current-screen-capture"
                    | "color-picker"
                    | "local-image"
                    | "guide-practice"
                    | "delay-ready"
                    | "clipboard-pin"
            ) || action.starts_with("edit-pin:"))
        {
            return;
        }
        let action = if let Some(generation) = action.strip_prefix("capture-finished:") {
            if generation.parse::<u64>().ok() != Some(shared.borrow().lifecycle.generation) {
                return;
            }
            slint::SharedString::from("capture-finished")
        } else {
            action
        };
        if crate::pin_session::handle(action.as_str(), &shared, &ui, &action_directory) {
            return;
        }
        if crate::local_image::handle(action.as_str(), &shared, &ui) {
            return;
        }
        let previous_scale = shared.borrow().session.preferences.draft.appearance.scale;
        if crate::config_transfer::handle(action.as_str(), &shared, &ui, &action_directory) {
            let mut state = shared.borrow_mut();
            if state.session.preferences.draft.appearance.scale != previous_scale {
                crate::scale_preview::begin(&mut state, &shared, &ui, previous_scale);
            }
            return;
        }
        if action == "clipboard-pin-ready" {
            crate::clipboard_pin::ready(&shared, &ui);
            crate::pin_session::after_action(&mut shared.borrow_mut(), &ui, &action_directory);
            return;
        }
        let edit_id = action
            .strip_prefix("edit-pin:")
            .and_then(|id| id.parse::<u64>().ok());
        let mut state = shared.borrow_mut();
        if (state.countdown.is_some() || state.delay_elapsed)
            && matches!(action.as_str(), "open" | "close" | "quit")
        {
            state.countdown.take();
            state.capturing = false;
            state.delay_elapsed = false;
            if let Some(focus) = state.capture_focus.take() {
                let _ = focus.restore();
            }
        }
        match action.as_str() {
            "workbench" if !state.capturing && state.capture.is_none() && ui.get_modal() == 0 => {
                if state.session.preferences.is_dirty() {
                    state.workbench_pending = true;
                    settings::modal(&ui, 1, state.en(), "");
                } else {
                    ui.set_guide_step(-1);
                    ui.set_workbench(true);
                    state.feedback.clear();
                    let _ = show_settings(&ui);
                }
            }
            "guide-open"
                if state.session.writable && !state.capturing && state.capture.is_none() =>
            {
                if state.session.preferences.is_dirty() {
                    state.guide_pending_open = true;
                    settings::modal(&ui, 1, state.en(), "");
                } else {
                    ui.set_workbench(false);
                    ui.set_guide_step(0);
                    let _ = show_settings(&ui);
                }
            }
            "guide-back" | "guide-next" if ui.get_guide_step() >= 0 && ui.get_modal() == 0 => {
                let step = ui.get_guide_step() + if action == "guide-back" { -1 } else { 1 };
                ui.set_guide_step(step.clamp(0, 5));
            }
            "guide-practice"
                if ui.get_guide_step() == 4 && !state.capturing && state.capture.is_none() =>
            {
                let english = state.en();
                match save_settings(
                    &mut state.session,
                    &registrations,
                    &action_directory,
                    english,
                ) {
                    Ok(()) => {
                        state.feedback.clear();
                        state.guide_practice = true;
                        state.guide_succeeded = false;
                        let owner = Rc::downgrade(&shared);
                        let weak = ui.as_weak();
                        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                            if let (Some(owner), Some(ui)) = (owner.upgrade(), weak.upgrade()) {
                                let ready = {
                                    let state = owner.borrow();
                                    state.guide_practice
                                        && ui.get_guide_step() == 4
                                        && ui.get_modal() == 0
                                };
                                if ready {
                                    ui.invoke_action("capture".into());
                                }
                            }
                        });
                    }
                    Err(_) => state.feedback = state.message(Message::SaveFailed),
                }
            }
            "guide-complete" if ui.get_guide_step() == 5 => {
                if finish_guide(&mut state, &ui, &registrations, &action_directory, true).is_ok() {
                    leave(&ui, false);
                }
            }
            "delay-cancel" => {
                if state.countdown.take().is_none() && !state.delay_elapsed {
                    return;
                }
                state.capturing = false;
                state.delay_elapsed = false;
                if std::mem::take(&mut state.capture_settings_visible) {
                    let _ = show_settings(&ui);
                }
                if let Some(focus) = state.capture_focus.take() {
                    let _ = focus.restore();
                }
            }
            "delay-ready" => {
                let Some(countdown) = state.countdown.take() else {
                    return;
                };
                state.capturing = false;
                if let Err(error) = countdown.finish() {
                    state.feedback = error;
                    if let Some(focus) = state.capture_focus.take() {
                        let _ = focus.restore();
                    }
                    let _ = show_settings(&ui);
                } else {
                    state.delay_elapsed = true;
                    state.capturing = true;
                    let ticket = state.delay_ticket;
                    let next = countdown.action.clone();
                    drop(countdown);
                    drop(state);
                    // Slint callbacks cannot recursively invoke the same callback.
                    // Queue acquisition after the countdown action has returned.
                    let owner = Rc::downgrade(&shared);
                    let weak = ui.as_weak();
                    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                        let Some(ui) = weak.upgrade() else {
                            return;
                        };
                        let Some(owner) = owner.upgrade() else {
                            return;
                        };
                        {
                            let mut state = owner.borrow_mut();
                            if !state.delay_elapsed || state.delay_ticket != ticket {
                                return;
                            }
                            state.capturing = false;
                        }
                        ui.invoke_action(next.into());
                    });
                    return;
                }
            }
            "clipboard-pin"
                if !state.capturing && state.capture.is_none() && !state.clipboard_pin_pending =>
            {
                if let Err(error) = crate::clipboard_pin::start(&mut state, &ui) {
                    state.feedback =
                        format!("{}: {error}", state.message(Message::PinClipboardFailed));
                    let _ = show_settings(&ui);
                }
            }
            action
                if matches!(
                    action,
                    "capture"
                        | "window-capture"
                        | "repeat-capture"
                        | "all-screens-capture"
                        | "current-screen-capture"
                        | "color-picker"
                        | "local-image"
                ) || edit_id.is_some() =>
            {
                if state.capture.is_some() || state.capturing || state.clipboard_pin_pending {
                    return;
                }
                if action == "local-image" && state.local_path.is_none() {
                    return;
                }
                state.local_edit = action == "local-image";
                if !state.local_edit {
                    state.local_path = None;
                }
                state.window_capture = action == "window-capture";
                state.repeat_capture = action == "repeat-capture";
                state.color_picker = action == "color-picker";
                state.screen_capture = crate::capture_screen::target(action);
                state.edit_pin = edit_id;
                if state.session.preferences.is_dirty() {
                    state.capture_pending = true;
                    state.quit_pending = false;
                    settings::modal(&ui, 1, state.en(), "");
                } else {
                    ui.invoke_show_modal(0);
                    state.recording.clear();
                    state.candidate.clear();
                    let delayed = std::mem::take(&mut state.delay_elapsed);
                    if !delayed {
                        state.capture_settings_visible = ui.window().is_visible();
                        state.capture_focus = glint_platform::remember_foreground();
                    }
                    if !state.local_edit
                        && edit_id.is_none()
                        && !state.guide_practice
                        && !delayed
                        && state.session.preferences.saved().capture.delay_seconds > 0
                    {
                        let started = ui.hide().map_err(|e| e.to_string()).and_then(|_| {
                            crate::capture_delay::Run::start(
                                &ui,
                                state.session.preferences.saved(),
                                &state.languages,
                                action,
                            )
                        });
                        match started {
                            Ok(countdown) => {
                                state.delay_ticket = state.delay_ticket.wrapping_add(1);
                                state.countdown = Some(countdown);
                                state.capturing = true;
                            }
                            Err(error) => {
                                state.feedback = error;
                                if let Some(focus) = state.capture_focus.take() {
                                    let _ = focus.restore();
                                }
                                let _ = show_settings(&ui);
                            }
                        }
                        return;
                    }
                    let edit_source = if let Some(id) = edit_id {
                        let source = state.pins.borrow().edit_source(id);
                        match source.and_then(|source| {
                            crate::probe::OutputSlot::acquire()
                                .map(|slot| (source, slot))
                                .ok_or("Output task is busy".into())
                        }) {
                            Ok(source) => Some(source),
                            Err(error) => {
                                if let Some(pin) = state.pins.borrow().window(id) {
                                    pin.set_status(error.clone().into());
                                }
                                state.feedback = error;
                                state.edit_pin = None;
                                return;
                            }
                        }
                    } else {
                        None
                    };
                    let local_source = if state.local_edit {
                        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                        let work = ui
                            .window()
                            .with_winit_window(|window| {
                                let handle = window.window_handle().map_err(|e| e.to_string())?;
                                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                                    return Err("Not a Win32 window".into());
                                };
                                unsafe {
                                    glint_platform::NativePinWindow::from_raw(
                                        handle.hwnd.get() as *mut std::ffi::c_void
                                    )
                                }
                                .work_area()
                            })
                            .unwrap_or_else(|| Err("Window unavailable".into()));
                        match work {
                            Ok(work) => Some((
                                state.local_path.take().unwrap(),
                                work,
                                ui.window().scale_factor(),
                            )),
                            Err(_) => {
                                state.feedback = state.message(Message::LocalFailed);
                                state.sync(&ui);
                                return;
                            }
                        }
                    } else {
                        None
                    };
                    let hidden = if edit_id.is_some()
                        || state.session.preferences.saved().pins.hide_on_capture
                    {
                        state.pins.borrow_mut().hide_for_capture()
                    } else {
                        Ok(())
                    };
                    if let Err(error) = hidden {
                        state.feedback = error;
                        state.sync(&ui);
                        return;
                    }
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    let disabled = ui
                        .window()
                        .with_winit_window(|window| {
                            let handle = window.window_handle().map_err(|e| e.to_string())?;
                            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                                return Err("Not a Win32 window".into());
                            };
                            unsafe {
                                glint_platform::NativePinWindow::from_raw(
                                    handle.hwnd.get() as *mut std::ffi::c_void
                                )
                            }
                            .disable_transitions()
                        })
                        .unwrap_or(Ok(()));
                    if let Err(error) = disabled {
                        let _ = state.pins.borrow_mut().restore_after_capture();
                        state.feedback =
                            format!("无法隐藏设置窗口 / Cannot hide settings: {error}");
                        state.sync(&ui);
                        return;
                    }
                    // Let Slint update its visibility and renderer state together.
                    // Settings are actually hidden, so capture exclusion is unnecessary
                    // and must not invalidate their software-rendered DWM surface.
                    let hidden = ui.hide().map_err(|e| e.to_string()).and_then(|_| {
                        ui.window()
                            .with_winit_window(|window| {
                                let handle = window.window_handle().map_err(|e| e.to_string())?;
                                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                                    return Err("Not a Win32 window".into());
                                };
                                unsafe {
                                    glint_platform::NativePinWindow::from_raw(
                                        handle.hwnd.get() as *mut std::ffi::c_void
                                    )
                                }
                                .flush_composition()
                            })
                            .unwrap_or(Ok(()))
                    });
                    if let Err(error) = hidden {
                        let _ = state.pins.borrow_mut().restore_after_capture();
                        state.feedback = error;
                        state.capture_settings_visible = false;
                        let _ = show_settings(&ui);
                        return;
                    }
                    state.lifecycle.generation = state.lifecycle.generation.wrapping_add(1);
                    state.capturing = true;
                    let (send, receive) = std::sync::mpsc::sync_channel(1);
                    state.capture_result = Some(receive);
                    let weak = ui.as_weak();
                    let detect = !state.color_picker
                        && (state.window_capture
                            || state.session.preferences.saved().capture.detect_window);
                    let include_cursor = !state.color_picker
                        && state.session.preferences.saved().capture.include_cursor;
                    let exclusions = glint_core::exclusions::parse(
                        &state.session.preferences.saved().privacy.excluded_apps,
                    );
                    let english = settings::english(
                        state.session.preferences.saved().general.language,
                        &state.languages,
                    );
                    // Keep a sender for startup failure, so capture-ready performs
                    // the same affinity/pin/focus recovery as capture failure.
                    let startup_failure = send.clone();
                    let worker = std::thread::Builder::new()
                        .name("glint-capture-prepare".into())
                        .spawn(move || {
                            let result = if let Some((path, work, scale)) = local_source {
                                crate::local_image::prepare(&path, work, scale)
                                    .map(crate::pin_edit::Input::Local)
                            } else if let Some((source, _slot)) = edit_source {
                                source.prepare().map(crate::pin_edit::Input::Pin)
                            } else {
                                exclusions
                                    .map_err(|_| {
                                        crate::messages::text(Message::ExclusionInvalid, english)
                                            .to_owned()
                                    })
                                    .and_then(|names| {
                                        glint_platform::capture_snapshot_guarded(
                                            detect,
                                            include_cursor,
                                            &names,
                                        )
                                        .map_err(|error| {
                                            match error {
                                            glint_platform::GuardedCaptureError::Capture(error) => {
                                                error
                                            }
                                            glint_platform::GuardedCaptureError::Exclusion(
                                                error,
                                            ) => crate::messages::text(
                                                match error {
                                                    glint_platform::ExclusionError::Blocked => {
                                                        Message::ExclusionBlocked
                                                    }
                                                    glint_platform::ExclusionError::Unavailable => {
                                                        Message::ExclusionUnavailable
                                                    }
                                                },
                                                english,
                                            )
                                            .to_owned(),
                                        }
                                        })
                                    })
                                    .map(crate::pin_edit::Input::Desktop)
                            };
                            let _ = send.send(result);
                            let _ = weak.upgrade_in_event_loop(|ui| {
                                ui.invoke_action("capture-ready".into())
                            });
                        });
                    if let Err(error) = worker {
                        let _ = startup_failure.send(Err(error.to_string()));
                        let _ = ui.as_weak().upgrade_in_event_loop(|ui| {
                            ui.invoke_action("capture-ready".into());
                        });
                    }
                }
            }
            "capture-ready" => {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if state.lifecycle.discard_ready {
                    state.capture_result.take();
                    state.capturing = false;
                    state.lifecycle.discard_ready = false;
                    if !state.lifecycle.blocked() {
                        let _ = state.pins.borrow_mut().restore_after_capture();
                    }
                    state.sync(&ui);
                    return;
                }
                state.capturing = false;
                let frame = state
                    .capture_result
                    .take()
                    .ok_or("Missing capture result".to_owned())
                    .and_then(|receive| receive.try_recv().map_err(|e| e.to_string()))
                    .and_then(|result| result);
                // Preserve desktop geometry independently of the window adapter's
                // initial decorated size and monitor work-area adjustments.
                let desktop_bounds = frame.as_ref().ok().and_then(|input| match input {
                    crate::pin_edit::Input::Desktop(snapshot) => Some((
                        snapshot.frame.x,
                        snapshot.frame.y,
                        snapshot.frame.width,
                        snapshot.frame.height,
                    )),
                    _ => None,
                });
                let weak = ui.as_weak();
                let layout = frame.as_ref().ok().and_then(|input| match input {
                    crate::pin_edit::Input::Desktop(snapshot) => capture_layout(snapshot),
                    _ => None,
                });
                let initial = if let Some(target) = state.screen_capture {
                    frame.as_ref().ok().and_then(|input| match input {
                        crate::pin_edit::Input::Desktop(snapshot) => {
                            crate::capture_screen::initial(snapshot, target)
                        }
                        _ => None,
                    })
                } else if state.repeat_capture
                    && state.session.preferences.saved().capture.remember_region
                {
                    layout
                        .as_ref()
                        .and_then(|layout| state.last_region.as_ref()?.recall(layout))
                } else {
                    None
                };
                let repeat_notice = if state.repeat_capture {
                    Some(
                        if !state.session.preferences.saved().capture.remember_region {
                            Message::RepeatOff
                        } else if state.last_region.is_none() {
                            Message::RepeatEmpty
                        } else if initial.is_none() {
                            Message::RepeatChanged
                        } else {
                            Message::RepeatReady
                        },
                    )
                } else {
                    None
                };
                let capture_notice = state
                    .screen_capture
                    .map(|target| crate::capture_screen::notice(target, initial.is_some()))
                    .or(repeat_notice);
                let opened = frame
                    .and_then(|input| {
                        let presentation = crate::probe::Presentation::from_config(
                            state.session.preferences.saved(),
                            &state.languages,
                        );
                        let generation = state.lifecycle.generation;
                        let complete = move || {
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                ui.invoke_action(format!("capture-finished:{generation}").into())
                            });
                        };
                        match input {
                            crate::pin_edit::Input::Desktop(frame) if state.color_picker => {
                                crate::probe::resident_color_picker(frame, presentation, complete)
                            }
                            crate::pin_edit::Input::Desktop(frame) => {
                                crate::probe::resident_capture_region(
                                    frame,
                                    state.window_capture,
                                    initial,
                                    presentation,
                                    state.capture_output.clone(),
                                    complete,
                                )
                            }
                            crate::pin_edit::Input::Local(prepared) => {
                                crate::probe::resident_pin_editor(
                                    true,
                                    prepared,
                                    presentation,
                                    state.capture_output.clone(),
                                    complete,
                                )
                            }
                            crate::pin_edit::Input::Scroll(prepared) => {
                                let (capture, payload) = crate::probe::resident_pin_editor(
                                    true,
                                    prepared,
                                    presentation,
                                    state.capture_output.clone(),
                                    complete,
                                )?;
                                capture.set_scroll_editor(true);
                                capture.set_magnifier_enabled(false);
                                let mut labels = capture.get_labels();
                                labels.title =
                                    messages::text(Message::ScrollTitle, capture.get_english())
                                        .into();
                                capture.set_labels(labels);
                                Ok((capture, payload))
                            }
                            crate::pin_edit::Input::Pin(prepared) => {
                                crate::probe::resident_pin_editor(
                                    false,
                                    prepared,
                                    presentation,
                                    state.capture_output.clone(),
                                    complete,
                                )
                            }
                        }
                    })
                    .and_then(|(capture, pin_payload)| {
                        let owner = ui.as_weak();
                        capture.on_record_start(move |x, y, width, height| {
                            let _ = owner.upgrade_in_event_loop(move |ui| {
                                ui.invoke_action(
                                    format!("record-region:{x}:{y}:{width}:{height}").into(),
                                )
                            });
                        });
                        let owner = ui.as_weak();
                        capture.on_scroll_start(move |x, y, width, height| {
                            let _ = owner.upgrade_in_event_loop(move |ui| {
                                ui.invoke_action(
                                    format!("scroll-start:{x}:{y}:{width}:{height}").into(),
                                );
                            });
                        });
                        let log = state.log.clone();
                        let receipt = capture.as_weak();
                        capture.on_closed_event(move || {
                            if let Some(capture) = receipt.upgrade() {
                                crate::diagnostic_log::capture_exit(
                                    &mut log.borrow_mut(),
                                    &capture,
                                );
                            }
                        });
                        let log = state.log.clone();
                        capture.on_output_failed(move || {
                            log.borrow_mut()
                                .record(glint_platform::diagnostic_log::Event::CaptureOutputFailed);
                        });
                        if let Some(notice) = capture_notice {
                            capture.set_repeat_notice(
                                messages::text(notice, capture.get_english()).into(),
                            );
                        }
                        let owner = Rc::downgrade(&shared);
                        capture.on_output_succeeded(move |x, y, width, height| {
                            let (Some(owner), Some(layout)) = (owner.upgrade(), layout.as_ref())
                            else {
                                return;
                            };
                            if [x, y, width, height].iter().any(|n| *n < 0) {
                                return;
                            }
                            let mut state = owner.borrow_mut();
                            if state.guide_practice {
                                state.guide_succeeded = true;
                            }
                            if state.session.preferences.saved().capture.remember_region
                                && let Some(region) =
                                    glint_core::region_memory::RememberedRegion::new(
                                        layout.clone(),
                                        glint_core::Rect {
                                            x: x as u32,
                                            y: y as u32,
                                            width: width as u32,
                                            height: height as u32,
                                        },
                                    )
                            {
                                state.last_region = Some(region);
                            }
                        });
                        let owner = Rc::downgrade(&shared);
                        crate::probe::configure_toolbar(
                            &capture,
                            state.session.preferences.saved().toolbar.clone(),
                            move |toolbar| {
                                owner
                                    .upgrade()
                                    .ok_or("Resident unavailable")?
                                    .borrow_mut()
                                    .session
                                    .save_toolbar(toolbar)
                            },
                        );
                        capture.set_pin_enabled(!state.color_picker);
                        let manager = state.pins.clone();
                        let owner = Rc::downgrade(&shared);
                        let weak = capture.as_weak();
                        let editing = state.edit_pin;
                        capture.on_request_pin(move |_image, x, y| {
                            let (Some(capture), Some(owner)) = (weak.upgrade(), owner.upgrade())
                            else {
                                return;
                            };
                            let config = owner.borrow().session.preferences.saved().clone();
                            let english = capture.get_english();
                            let weak_capture = capture.as_weak();
                            let pin_owner = Rc::downgrade(&manager);
                            let payload = pin_payload
                                .lock()
                                .map_err(|_| "Pin document unavailable".to_owned())
                                .and_then(|mut slot| {
                                    slot.take().ok_or("Pin document missing".into())
                                });
                            let payload = match payload {
                                Ok(payload) => payload,
                                Err(error) => {
                                    capture.set_busy(false);
                                    capture.set_status(
                                        format!(
                                            "{}: {error}",
                                            messages::text(Message::PinFailed, english)
                                        )
                                        .into(),
                                    );
                                    return;
                                }
                            };
                            if let Some(id) = editing {
                                let result = manager.borrow_mut().replace_document(id, payload);
                                capture.set_busy(false);
                                match result {
                                    Ok(()) => {
                                        crate::probe::accept_pin_output(&capture);
                                        capture.set_completion_kind(5);
                                        capture.invoke_finished();
                                    }
                                    Err(error) => capture.set_status(
                                        format!(
                                            "{}: {error}",
                                            messages::text(Message::PinFailed, english)
                                        )
                                        .into(),
                                    ),
                                }
                                return;
                            }
                            let created = crate::pins::Manager::create_document(
                                &manager,
                                payload,
                                (x, y),
                                &config,
                                english,
                                move |result| {
                                    if let Some(capture) = weak_capture.upgrade() {
                                        capture.set_busy(false);
                                        match result {
                                            Ok(id) => {
                                                if capture.window().is_visible() {
                                                    crate::probe::accept_pin_output(&capture);
                                                    capture.set_completion_kind(3);
                                                    capture.invoke_finished();
                                                } else if let Some(owner) = pin_owner.upgrade() {
                                                    let _ = owner.borrow_mut().close(id);
                                                }
                                            }
                                            Err(error) => capture.set_status(
                                                format!(
                                                    "{}: {error}",
                                                    messages::text(Message::PinFailed, english)
                                                )
                                                .into(),
                                            ),
                                        }
                                    } else if let Ok(id) = result
                                        && let Some(owner) = pin_owner.upgrade()
                                    {
                                        let _ = owner.borrow_mut().close(id);
                                    }
                                },
                            );
                            if let Err(error) = created {
                                capture.set_busy(false);
                                capture.set_status(
                                    format!(
                                        "{}: {error}",
                                        messages::text(Message::PinFailed, english)
                                    )
                                    .into(),
                                );
                            }
                        });
                        let weak = capture.as_weak();
                        let owner = Rc::downgrade(&shared);
                        let settings_ui = ui.as_weak();
                        let generation = state.lifecycle.generation;
                        slint::spawn_local(async move {
                            let Some(capture) = weak.upgrade() else {
                                return;
                            };
                            let opened: Result<(), String> = async {
                                // Obtain the hidden native peer first. Showing before preparation
                                // lets the compositor animate a full-desktop window into place.
                                let window = capture
                                    .window()
                                    .winit_window()
                                    .await
                                    .map_err(|e| e.to_string())?;
                                let Some(owner) = owner.upgrade() else {
                                    return Ok(());
                                };
                                {
                                    let state = owner.borrow();
                                    if state.lifecycle.generation != generation
                                        || state.lifecycle.blocked()
                                        || state.capture.is_none()
                                        || capture.get_closed()
                                    {
                                        return Ok(());
                                    }
                                }
                                let handle = window.window_handle().map_err(|e| e.to_string())?;
                                let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                                    return Err("Capture window has no Win32 handle".into());
                                };
                                let native = unsafe {
                                    glint_platform::NativePinWindow::from_raw(
                                        handle.hwnd.get() as *mut std::ffi::c_void
                                    )
                                };
                                window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
                                native.prepare_capture_overlay()?;
                                if let Some((x, y, width, height)) = desktop_bounds {
                                    capture
                                        .window()
                                        .set_position(slint::PhysicalPosition::new(x, y));
                                    capture
                                        .window()
                                        .set_size(slint::PhysicalSize::new(width, height));
                                    native.align_capture_desktop(x, y, width, height)?;
                                }
                                capture.show().map_err(|e| e.to_string())?;
                                // winit's Windows focus_window() injects an Alt press/release.
                                // That interferes with Alt-based capture shortcuts and menus.
                                if !native.focus_without_attention()? {
                                    let hint = messages::text(
                                        Message::CaptureFocusUnavailable,
                                        capture.get_english(),
                                    );
                                    let previous = capture.get_service_notice();
                                    capture.set_service_notice(if previous.is_empty() {
                                        hint.into()
                                    } else {
                                        format!("{previous}\n{hint}").into()
                                    });
                                }
                                Ok(())
                            }
                            .await;
                            if let Err(error) = opened
                                && let Some(ui) = settings_ui.upgrade()
                                && let Some(owner) = owner.upgrade()
                            {
                                let current = owner.borrow().lifecycle.generation == generation;
                                if !current {
                                    return;
                                }
                                capture.set_status(error.into());
                                ui.invoke_action("capture-show-failed".into());
                            }
                        })
                        .map_err(|e| e.to_string())?;
                        Ok(capture)
                    });
                match opened {
                    Ok(capture) => {
                        state
                            .log
                            .borrow_mut()
                            .record(glint_platform::diagnostic_log::Event::CaptureReady);
                        state.capture = Some(capture);
                    }
                    Err(error) => {
                        state
                            .log
                            .borrow_mut()
                            .record(glint_platform::diagnostic_log::Event::CaptureFailed);
                        let _ = state.pins.borrow_mut().restore_after_capture();
                        if let Some(focus) = state.capture_focus.take() {
                            let _ = focus.restore();
                        }
                        state.feedback = if state.local_edit {
                            state.message(Message::LocalFailed)
                        } else {
                            format!("{}: {error}", state.message(Message::CaptureFailureLabel))
                        };
                        if state.local_edit {
                            ui.set_workbench(true);
                            ui.set_guide_step(-1);
                        }
                        let _ = show_settings(&ui);
                    }
                }
            }
            "capture-show-failed" => {
                let detail = state.capture.as_ref().map(|capture| capture.get_status());
                if let Some(capture) = state.capture.take() {
                    let _ = capture.hide();
                }
                let _ = state.pins.borrow_mut().restore_after_capture();
                if let Some(focus) = state.capture_focus.take() {
                    let _ = focus.restore();
                }
                if state.local_edit {
                    ui.set_workbench(true);
                    ui.set_guide_step(-1);
                } else if std::mem::take(&mut state.guide_practice) {
                    state.guide_succeeded = false;
                    ui.set_guide_step(4);
                }
                state.local_edit = false;
                state.edit_pin = None;
                state.feedback = format!(
                    "{}: {}",
                    state.message(Message::CaptureFailureLabel),
                    detail.unwrap_or_default(),
                );
                let _ = show_settings(&ui);
            }
            "capture-finished" => {
                state.scroll_capture.take();
                let blocked = state.lifecycle.blocked();
                let restore_settings = std::mem::take(&mut state.capture_settings_visible);
                // External focus restoration checks ownership before hiding the overlay.
                if let Some(focus) = state.capture_focus.take()
                    && !blocked
                    && !restore_settings
                {
                    let _ = focus.restore();
                }
                state.local_edit = false;
                state.edit_pin = None;
                if let Some(capture) = state.capture.take() {
                    let _ = capture.hide();
                    if !blocked {
                        crate::completion::show(
                            crate::completion::mode(capture.get_feedback_mode()),
                            capture.get_completion_kind(),
                            capture.get_english(),
                            capture.get_dark(),
                            capture.get_accent(),
                            capture.get_interface_scale(),
                            capture.get_reduce_motion(),
                        );
                    }
                }
                let restored = if blocked {
                    Ok(())
                } else {
                    state.pins.borrow_mut().restore_after_capture()
                };
                if let Err(error) = restored {
                    state.feedback = error;
                }
                // Restore settings through Slint after the full-desktop overlay is hidden.
                // A raw HWND show would bypass Slint's dirty-region/visibility state.
                if restore_settings
                    && !blocked
                    && !state.guide_practice
                    && let Err(error) = show_settings(&ui)
                {
                    state.feedback = error;
                }
                if std::mem::take(&mut state.guide_practice) {
                    ui.set_guide_step(if state.guide_succeeded { 5 } else { 4 });
                    state.guide_succeeded = false;
                    if !blocked {
                        let _ = show_settings(&ui);
                    }
                }
            }
            "toggle-pins" | "rescue-pins" if !state.capturing && state.capture.is_none() => {
                let result = if action == "toggle-pins" {
                    state.pins.borrow_mut().toggle_all()
                } else {
                    state.pins.borrow_mut().rescue_all()
                };
                if let Err(error) = result {
                    state.feedback = error;
                    let _ = show_settings(&ui);
                }
            }
            "open" | "workbench-privacy" | "workbench-output" => {
                ui.set_workbench(false);
                if action == "workbench-privacy" {
                    ui.set_category(7);
                }
                if action == "workbench-output" {
                    ui.set_category(5);
                }
                state.languages = glint_platform::preferred_ui_languages().unwrap_or_default();
                if let Err(error) = show_settings(&ui) {
                    state.feedback = error;
                }
            }
            "hotkey-error" => {
                state.feedback = state.message(Message::ShortcutServiceStopped);
                let capture_active = state.capturing || state.capture.is_some();
                let recovered = state.pins.borrow_mut().recovery_failed(!capture_active);
                if let Err(error) = recovered {
                    state.feedback.push_str(&format!("; {error}"));
                }
                if !capture_active {
                    let _ = show_settings(&ui);
                }
            }

            "close" | "quit" => {
                if state.capture.is_none() && !state.capturing {
                    state.guide_practice = false;
                }
                state.local_path = None;
                state.capture_pending = false;
                state.quit_pending = action == "quit";
                if state.session.preferences.is_dirty() {
                    settings::modal(&ui, 1, state.en(), "");
                } else if ui.get_guide_step() < 0
                    || finish_guide(&mut state, &ui, &registrations, &action_directory, false)
                        .is_ok()
                {
                    leave(&ui, state.quit_pending);
                }
            }
            "continue" => {
                state.workbench_pending = false;
                state.local_path = None;
                state.guide_pending_open = false;
                state.edit_pin = None;
                state.capture_pending = false;
                ui.invoke_show_modal(0);
                state.recording.clear();
                state.candidate.clear();
                state.quit_pending = false;
            }
            "cancel" => {
                state.session.preferences.cancel();
                state.feedback.clear();
            }
            "reset" => settings::modal(&ui, 2, state.en(), ""),
            "confirm-reset" => {
                let previous_scale = state.session.preferences.draft.appearance.scale;
                state.session.preferences.restore_defaults();
                state.feedback.clear();
                ui.invoke_show_modal(0);
                state.sync(&ui);
                if state.session.preferences.draft.appearance.scale != previous_scale {
                    crate::scale_preview::begin(&mut state, &shared, &ui, previous_scale);
                }
            }
            "clear-key" => {
                if state.recording == "shortcuts.rescue" {
                    return;
                }
                let recording = state.recording.clone();
                if let Some(binding) =
                    recorded_binding(&mut state.session.preferences.draft, &recording)
                {
                    binding.clear();
                }
                state.recording.clear();
                state.feedback.clear();
                ui.invoke_show_modal(0);
            }
            "accept-key" => {
                if ui.get_modal() == 3 && !state.candidate.is_empty() {
                    let mut next = state.session.preferences.draft.clone();
                    let Some(binding) = recorded_binding(&mut next, &state.recording) else {
                        return;
                    };
                    *binding = state.candidate.clone();
                    if next.validate().is_ok() {
                        state.session.preferences.draft = next;
                        state.recording.clear();
                        state.candidate.clear();
                        state.feedback.clear();
                        ui.invoke_show_modal(0);
                    }
                }
            }
            "discard-leave" => {
                state.session.preferences.cancel();
                ui.invoke_show_modal(0);
                if ui.get_guide_step() < 0
                    || finish_guide(&mut state, &ui, &registrations, &action_directory, false)
                        .is_ok()
                {
                    leave(&ui, state.quit_pending);
                }
            }
            "save" | "save-leave" => {
                let english = state.en();
                let result = if action == "save-leave" && ui.get_guide_step() >= 0 {
                    finish_guide(&mut state, &ui, &registrations, &action_directory, true)
                } else {
                    save_settings(
                        &mut state.session,
                        &registrations,
                        &action_directory,
                        english,
                    )
                };
                match result {
                    Ok(()) => {
                        crate::diagnostic_log::configure(
                            &mut state.log.borrow_mut(),
                            &state.session.preferences.saved().privacy,
                        );
                        state
                            .log
                            .borrow_mut()
                            .record(glint_platform::diagnostic_log::Event::SettingsSaved);
                        if !state.session.preferences.saved().capture.remember_region {
                            state.last_region = None;
                        }
                        state.pins.borrow_mut().recovery_repaired();
                        state
                            .pins
                            .borrow_mut()
                            .set_budget(state.session.preferences.saved().resources.pin_budget_mib);
                        state.feedback = state.message(Message::SettingsSaved);
                        if action == "save-leave" {
                            ui.invoke_show_modal(0);
                            leave(&ui, state.quit_pending);
                        }
                    }
                    Err(error) => {
                        state
                            .log
                            .borrow_mut()
                            .record(glint_platform::diagnostic_log::Event::SettingsSaveFailed);
                        state.feedback = if error.contains("rollback_failed") {
                            state.message(Message::SaveRollbackFailed)
                        } else {
                            state.message(Message::SaveFailed)
                        };
                    }
                }
            }
            _ => {}
        }
        state.sync(&ui);
        crate::pin_session::after_action(&mut state, &ui, &action_directory);
        if state.workbench_pending && ui.get_modal() == 0 && !state.session.preferences.is_dirty() {
            state.workbench_pending = false;
            ui.set_guide_step(-1);
            ui.set_workbench(true);
            state.sync(&ui);
            let _ = show_settings(&ui);
        }
        if state.guide_pending_open && ui.get_modal() == 0 && !state.session.preferences.is_dirty()
        {
            state.guide_pending_open = false;
            ui.set_guide_step(0);
            state.sync(&ui);
            let _ = show_settings(&ui);
        }
        if state.capture_pending && ui.get_modal() == 0 && !state.session.preferences.is_dirty() {
            state.capture_pending = false;
            let action = if state.local_edit {
                "local-image".to_owned()
            } else if let Some(id) = state.edit_pin {
                format!("edit-pin:{id}")
            } else if state.color_picker {
                "color-picker".to_owned()
            } else if state.window_capture {
                "window-capture".to_owned()
            } else if state.repeat_capture {
                "repeat-capture".to_owned()
            } else if let Some(target) = state.screen_capture {
                crate::capture_screen::action(target).to_owned()
            } else {
                "capture".to_owned()
            };
            let _ = ui
                .as_weak()
                .upgrade_in_event_loop(move |ui| ui.invoke_action(action.into()));
        }
    });
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_action("close".into());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |window, event| {
        match event {
            winit::event::WindowEvent::Focused(true) => glint_platform::diagnostic_log::emit(
                glint_platform::diagnostic_log::Event::SettingsFocused,
            ),
            winit::event::WindowEvent::Focused(false) => glint_platform::diagnostic_log::emit(
                glint_platform::diagnostic_log::Event::SettingsBlurred,
            ),
            _ => {}
        }
        if matches!(event, winit::event::WindowEvent::ThemeChanged(_)) {
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("system-changed".into()));
        }
        if let Some(ui) = weak.upgrade() {
            crate::interface_scale::event(window, event, ui.get_interface_scale())
        } else {
            EventResult::Propagate
        }
    });
    crate::lifecycle::install(&ui, state.clone());
    let english = state.borrow().en();
    let menu = Menu::new();
    let open = MenuItem::with_id(
        "open",
        settings::fields::label(english, "设置…", "Settings…"),
        true,
        None,
    );
    let workbench = MenuItem::with_id(
        "workbench",
        messages::text(Message::Workbench, english),
        true,
        None,
    );
    let capture = MenuItem::with_id(
        "capture",
        settings::fields::label(english, "区域截图", "Capture region"),
        true,
        None,
    );
    let screen_recording = MenuItem::with_id(
        "record-open",
        settings::fields::label(english, "录屏…", "Record screen…"),
        true,
        None,
    );
    let window = MenuItem::with_id(
        "window-capture",
        settings::fields::label(english, "窗口截图", "Capture window"),
        true,
        None,
    );
    let all_screens = MenuItem::with_id(
        "all-screens-capture",
        messages::text(Message::AllScreensCapture, english),
        true,
        None,
    );
    let current_screen = MenuItem::with_id(
        "current-screen-capture",
        messages::text(Message::CurrentScreenCapture, english),
        true,
        None,
    );
    let color_picker = MenuItem::with_id(
        "color-picker",
        messages::text(Message::ColorPickerEntry, english),
        true,
        None,
    );
    let clipboard = MenuItem::with_id(
        "clipboard-pin",
        settings::fields::label(english, "贴出剪贴板图像", "Pin clipboard image"),
        true,
        None,
    );
    let repeat = MenuItem::with_id(
        "repeat-capture",
        settings::fields::label(english, "重复上次区域", "Repeat last region"),
        true,
        None,
    );
    let quit = MenuItem::with_id(
        "quit",
        settings::fields::label(english, "退出拾光", "Quit Glint"),
        true,
        None,
    );
    let toggle = MenuItem::with_id(
        "toggle-pins",
        settings::fields::label(english, "隐藏 / 恢复全部贴图", "Hide / restore all pins"),
        true,
        None,
    );
    let rescue = MenuItem::with_id(
        "rescue-pins",
        settings::fields::label(english, "恢复贴图并取消穿透", "Rescue desktop pins"),
        true,
        None,
    );
    menu.append_items(&[
        &capture,
        &screen_recording,
        &window,
        &all_screens,
        &current_screen,
        &color_picker,
        &repeat,
        &clipboard,
        &toggle,
        &rescue,
        &PredefinedMenuItem::separator(),
        &open,
        &workbench,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;
    state.borrow_mut().tray_labels = Some(TrayLabels {
        workbench,
        screen_recording,
        repeat,
        clipboard,
        open,
        capture,
        window,
        all_screens,
        current_screen,
        color_picker,
        toggle,
        rescue,
        quit,
    });
    let weak = ui.as_weak();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = event.id.0;
        let _ = weak.upgrade_in_event_loop(move |ui| ui.invoke_action(action.into()));
    }));
    let weak = ui.as_weak();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(event, TrayIconEvent::DoubleClick { .. }) {
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("open".into()));
        }
    }));
    let _tray = TrayIconBuilder::new()
        .with_tooltip("拾光 Glint")
        .with_icon(crate::brand::tray_icon()?)
        .with_menu(Box::new(menu))
        .build()?;
    state.borrow().pins.borrow_mut().set_tray_ready(true);
    crate::pin_session::start(&mut state.borrow_mut(), &ui, &directory);
    if show_initial {
        show_settings(&ui)?;
    }
    let smoke_error = Rc::new(RefCell::new(None));
    #[cfg(feature = "input-probe")]
    if input_smoke {
        crate::input_smoke::install(
            &ui,
            state.clone(),
            directory.clone(),
            smoke_error.clone(),
            args.iter().any(|arg| {
                matches!(
                    arg.as_str(),
                    "--m5-dialog-input-smoke" | "--m5-dialog-conflict-input-smoke"
                )
            }),
            args.iter()
                .any(|arg| arg == "--m5-dialog-conflict-input-smoke"),
        )?;
    }
    if let Some(mode) = session_probe {
        crate::pin_session_smoke::install(
            mode,
            &ui,
            state.clone(),
            directory.clone(),
            smoke_error.clone(),
        );
    }
    if settings_policy_smoke {
        crate::settings_policy_smoke::install(
            &ui,
            state.clone(),
            directory.clone(),
            smoke_error.clone(),
        );
    } else if resource_smoke {
        crate::resource_smoke::install(
            &ui,
            state.clone(),
            directory.clone(),
            smoke_error.clone(),
            resource_4k,
            endurance,
        )?;
    } else if smoke && !input_smoke {
        crate::resident_smoke::install(
            &ui,
            state.clone(),
            hotkeys.clone(),
            directory,
            smoke_error.clone(),
            if shortcuts_smoke {
                352
            } else if exclusion_smoke {
                344
            } else if log_smoke {
                329
            } else if local_dialog_smoke {
                321
            } else if config_dialog_smoke {
                317
            } else if output_conflict_smoke {
                309
            } else if output_settings_smoke {
                303
            } else if fonts_smoke {
                291
            } else if diagnostics_smoke {
                285
            } else if lifecycle_smoke {
                278
            } else if budget_smoke {
                275
            } else if motion_smoke {
                265
            } else if scale_smoke {
                255
            } else {
                0
            },
        );
    }
    let loop_result = slint::run_event_loop_until_quit();
    state
        .borrow()
        .log
        .borrow_mut()
        .record(glint_platform::diagnostic_log::Event::Shutdown);
    state.borrow().log.borrow_mut().shutdown();
    // Stop/join the hidden notification window before releasing application state.
    state.borrow_mut().lifecycle.guard.take();
    loop_result?;
    if let Err(error) = hotkeys.replace(vec![]) {
        eprintln!("Glint hotkey shutdown: {error}");
    }
    MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
    if let Some(error) = smoke_error.borrow_mut().take() {
        return Err(error.into());
    }
    Ok(())
}
fn finish_guide(
    state: &mut AppState,
    ui: &SettingsWindow,
    hotkeys: &HotkeyService,
    directory: &std::path::Path,
    apply_draft: bool,
) -> Result<(), String> {
    let previous = state.session.preferences.draft.general.onboarding_complete;
    state.session.preferences.draft.general.onboarding_complete = true;
    let english = state.en();
    // A clean/discarded skip only records that the guide was seen. In particular,
    // an unavailable default hotkey must not prevent returning to the tray.
    let result = if apply_draft {
        save_settings(&mut state.session, hotkeys, directory, english)
    } else {
        state.session.save(|_| Ok(()))
    };
    match result {
        Ok(()) => {
            ui.set_guide_step(-1);
            if apply_draft {
                state.feedback.clear();
            }
            Ok(())
        }
        Err(error) => {
            state.session.preferences.draft.general.onboarding_complete = previous;
            state.feedback = state.message(Message::SaveFailed);
            Err(error)
        }
    }
}
fn leave(ui: &SettingsWindow, quit: bool) {
    if quit {
        let _ = ui
            .as_weak()
            .upgrade_in_event_loop(|ui| ui.invoke_action("resident-exit".into()));
    } else {
        let _ = ui.hide();
    }
}
fn save_settings(
    session: &mut SettingsSession,
    hotkeys: &HotkeyService,
    directory: &std::path::Path,
    english: bool,
) -> Result<(), String> {
    if session.preferences.draft.privacy.excluded_apps
        != session.preferences.saved().privacy.excluded_apps
        && glint_core::exclusions::parse(&session.preferences.draft.privacy.excluded_apps).is_err()
    {
        return Err(crate::messages::text(Message::ExclusionInvalid, english).to_owned());
    }
    if session.preferences.draft.general.start_on_login
        == session.preferences.saved().general.start_on_login
    {
        return session.save(|config| hotkeys.replace(runtime_bindings(config)?));
    }
    let registration = glint_platform::StartupRegistration::current_user(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        Some(directory),
    )?;
    let previous = registration.snapshot()?;
    save_runtime(
        session,
        |config| hotkeys.replace(runtime_bindings(config)?),
        |enabled| registration.set_enabled(enabled),
        || registration.restore(&previous),
    )
}
fn save_runtime(
    session: &mut SettingsSession,
    mut shortcuts: impl FnMut(&Config) -> Result<(), String>,
    mut set_startup: impl FnMut(bool) -> Result<(), String>,
    mut restore_startup: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let mut applying = true;
    session.save(|config| {
        if applying {
            set_startup(config.general.start_on_login)?;
            applying = false;
            if let Err(error) = shortcuts(config) {
                return match restore_startup() {
                    Ok(()) => Err(error),
                    Err(_) => Err(format!("{error}; startup.rollback_failed")),
                };
            }
            Ok(())
        } else {
            // File save failed. Restore exact prior registry bytes, including any external value.
            let startup = restore_startup();
            let bindings = shortcuts(config);
            startup.and(bindings)
        }
    })
}
pub(crate) fn show_settings(ui: &SettingsWindow) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    ui.show().map_err(|e| e.to_string())?;
    glint_platform::diagnostic_log::emit(glint_platform::diagnostic_log::Event::SettingsShown);
    // Showing an existing software-rendered window marks its surface dirty,
    // but needs a redraw request even if no settings property has changed.
    ui.window().request_redraw();
    let weak = ui.as_weak();
    slint::spawn_local(async move {
        if let Some(ui) = weak.upgrade()
            && let Ok(window) = ui.window().winit_window().await
        {
            // A queued show can outlive the user's next capture/hide request.
            if !ui.window().is_visible() {
                return;
            }
            if let Err(error) =
                crate::interface_scale::preview(ui.window(), ui.get_interface_scale())
            {
                eprintln!("Interface scale: {error}");
            }
            if let Err(error) = crate::motion::preview(ui.window(), ui.get_reduce_motion()) {
                eprintln!("Motion policy: {error}");
            }
            if let Ok(handle) = window.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                let native = unsafe {
                    glint_platform::NativePinWindow::from_raw(
                        handle.hwnd.get() as *mut std::ffi::c_void
                    )
                };
                if let Err(error) = native.focus_without_attention() {
                    eprintln!("Settings focus: {error}");
                }
            }
            // Scale/affinity restoration can replace or invalidate the backing
            // surface. Schedule a fresh frame after those operations as well.
            ui.window().request_redraw();
        }
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}
// Both focused key events and Windows-owned global chords use the same draft validation.
fn update_shortcut_candidate(state: &mut AppState, ui: &SettingsWindow, candidate: String) {
    let mut next = state.session.preferences.draft.clone();
    let Some(binding) = recorded_binding(&mut next, &state.recording) else {
        return;
    };
    *binding = candidate.clone();
    if next.validate().is_err() {
        state.candidate.clear();
        ui.set_can_accept(false);
        ui.set_shortcut_value(candidate.into());
        ui.set_shortcut_invalid(true);
        ui.set_modal_body(state.message(Message::ShortcutInvalid).into());
        return;
    }
    state.candidate = candidate;
    ui.set_can_accept(true);
    ui.set_shortcut_value(state.candidate.as_str().into());
    ui.set_shortcut_invalid(false);
    ui.set_modal_body(state.message(Message::ShortcutCandidate).into());
}

fn key_name(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let key = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(match key {
        '\u{8}' => "Backspace".into(),
        '\u{9}' => "Tab".into(),
        '\n' | '\r' => "Enter".into(),
        '\u{1b}' => "Escape".into(),
        ' ' => "Space".into(),
        '\u{7f}' => "Delete".into(),
        '\u{f700}' => "Up".into(),
        '\u{f701}' => "Down".into(),
        '\u{f702}' => "Left".into(),
        '\u{f703}' => "Right".into(),
        '\u{f704}'..='\u{f71b}' => format!("F{}", key as u32 - 0xf704 + 1),
        '\u{f727}' => "Insert".into(),
        '\u{f729}' => "Home".into(),
        '\u{f72b}' => "End".into(),
        '\u{f72c}' => "PageUp".into(),
        '\u{f72d}' => "PageDown".into(),
        key if key.is_ascii_alphanumeric() => key.to_ascii_uppercase().into(),
        _ => return None,
    })
}

// Resolve existing ancestors without creating the first-run configuration directory.
// Otherwise saving for the first time changes an ordinary path into a canonical
// verbatim Windows path and silently gives the same directory a second mutex.
fn instance_namespace(directory: &std::path::Path) -> std::io::Result<String> {
    use std::path::Component;
    let absolute = std::path::absolute(directory)?;
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Prefix(_) | Component::RootDir => resolved.push(component.as_os_str()),
            Component::Normal(name) => {
                resolved.push(name);
                match resolved.canonicalize() {
                    Ok(canonical) => resolved = canonical,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    resolved
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
        .hash(&mut hash);
    Ok(format!("resident-{:016x}", hash.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_run_directory_creation_keeps_live_instance_identity() {
        let base = std::env::temp_dir().join(format!(
            "glint-instance-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&base).unwrap();
        let directory = base.join("new").join("config");
        let before = instance_namespace(&directory).unwrap();
        assert!(
            !directory.exists(),
            "Identity calculation must not create configuration directories"
        );
        let (sent, received) = std::sync::mpsc::sync_channel(1);
        let InstanceStart::Primary(primary) = ResidentInstance::start(&before, move || {
            let _ = sent.try_send(());
        })
        .unwrap() else {
            panic!("Expected primary");
        };
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("config.toml"), b"config_version=1\n").unwrap();
        let after = instance_namespace(&directory.canonicalize().unwrap()).unwrap();
        assert_eq!(
            before, after,
            "First settings save must not create a second instance namespace"
        );
        assert!(matches!(
            ResidentInstance::start(&after, || {}).unwrap(),
            InstanceStart::Activated
        ));
        received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert_eq!(
            after,
            instance_namespace(&base.join("new/config/../config/.")).unwrap()
        );
        let upper = directory.to_string_lossy().to_uppercase();
        assert_eq!(
            after,
            instance_namespace(std::path::Path::new(&upper)).unwrap()
        );
        assert_ne!(after, instance_namespace(&base.join("other")).unwrap());
        drop(primary);
        std::fs::remove_dir_all(base).unwrap();
    }
    use std::{cell::Cell, fs, os::windows::fs::OpenOptionsExt};

    #[test]
    fn startup_and_shortcuts_rollback_together_when_file_is_locked() {
        let directory =
            std::env::temp_dir().join(format!("glint-runtime-save-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let file = directory.join("config.toml");
        let mut session = SettingsSession::open(&file);
        session.save(|_| Ok(())).unwrap();
        let saved = session.preferences.saved().clone();
        session.preferences.draft.general.start_on_login = true;
        session.preferences.draft.shortcuts.settings = "Ctrl+Alt+F".into();
        assert_eq!(
            settings_binding(&session.preferences.draft).unwrap()[0].key,
            u32::from(b'F')
        );
        let startup = Cell::new(false);
        let binding = RefCell::new(saved.shortcuts.settings.clone());
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        assert!(
            save_runtime(
                &mut session,
                |config| {
                    *binding.borrow_mut() = config.shortcuts.settings.clone();
                    Ok(())
                },
                |enabled| {
                    startup.set(enabled);
                    Ok(())
                },
                || {
                    startup.set(false);
                    Ok(())
                },
            )
            .unwrap_err()
            .contains("runtime_rolled_back")
        );
        assert!(!startup.get());
        assert_eq!(*binding.borrow(), saved.shortcuts.settings);
        assert_eq!(session.preferences.saved(), &saved);
        assert!(session.preferences.draft.general.start_on_login);
        drop(lock);
        assert!(
            save_runtime(
                &mut session,
                |_| Err("occupied".into()),
                |enabled| {
                    startup.set(enabled);
                    Ok(())
                },
                || {
                    startup.set(false);
                    Ok(())
                },
            )
            .is_err()
        );
        assert!(!startup.get());
        assert!(
            save_runtime(
                &mut session,
                |_| Ok(()),
                |enabled| {
                    startup.set(enabled);
                    Ok(())
                },
                || Ok(()),
            )
            .is_ok()
        );
        assert!(startup.get());
        assert!(session.preferences.saved().general.start_on_login);
        fs::remove_dir_all(directory).unwrap();
    }
}
