//! Versioned, platform-independent preferences. Values use stable IDs, never translated UI text.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CONFIG_VERSION: u32 = 1;
pub const MAX_CONFIG_BYTES: usize = 256 * 1024;

macro_rules! choice {
    ($name:ident { $first:ident $(, $variant:ident)* $(,)? }) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { #[default] $first, $($variant),* }
    };
}
choice!(Language { System, ZhCn, En });
choice!(Theme {
    Light,
    Dark,
    System
});
choice!(UiFont { System, NotoSansSc });
choice!(Scale {
    System,
    Percent100,
    Percent125,
    Percent150,
    Percent175
});
choice!(FinishAction {
    Copy,
    CopyAndPin,
    SaveAs
});
choice!(ImageFormat { Png, Jpeg });
choice!(FileConflict {
    Reject,
    Number,
    Confirm
});
choice!(Feedback {
    Toast,
    None,
    Notification
});
choice!(ToolbarMode {
    IconsAndLabels,
    Icons
});
choice!(ToolbarPosition {
    Follow,
    Top,
    Bottom
});
choice!(ArrowStyle { Filled, Open });
choice!(RedactStyle { Solid, Pixelate });
choice!(PinWheel { Zoom, Opacity });
choice!(CaptureSource { Auto, Gdi });
choice!(Tool {
    Select,
    Rect,
    Arrow,
    Pen,
    Text,
    Highlight,
    Redact,
    Mosaic,
    Picker
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub config_version: u32,
    pub general: General,
    pub shortcuts: Shortcuts,
    pub capture: Capture,
    pub toolbar: Toolbar,
    pub annotation: AnnotationDefaults,
    pub output: Output,
    pub pins: Pins,
    pub privacy: Privacy,
    pub appearance: Appearance,
    pub resources: Resources,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: CONFIG_VERSION,
            general: General::default(),
            shortcuts: Shortcuts::default(),
            capture: Capture::default(),
            toolbar: Toolbar::default(),
            annotation: AnnotationDefaults::default(),
            output: Output::default(),
            pins: Pins::default(),
            privacy: Privacy::default(),
            appearance: Appearance::default(),
            resources: Resources::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub language: Language,
    pub start_on_login: bool,
    pub start_in_tray: bool,
    pub close_to_tray: bool,
    pub single_instance: bool,
    pub check_updates: bool,
    pub onboarding_complete: bool,
}
impl Default for General {
    fn default() -> Self {
        Self {
            language: Language::System,
            start_on_login: false,
            start_in_tray: true,
            close_to_tray: true,
            single_instance: true,
            check_updates: false,
            onboarding_complete: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Shortcuts {
    /// An empty string explicitly clears an optional default binding.
    pub capture: String,
    pub window: String,
    pub last_region: String,
    pub pin_clipboard: String,
    pub toggle_pins: String,
    pub rescue: String,
    pub color_picker: String,
    pub settings: String,
}
impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            capture: "Ctrl+Shift+S".into(),
            window: "Ctrl+Shift+W".into(),
            last_region: "Ctrl+Shift+R".into(),
            pin_clipboard: "Ctrl+Shift+P".into(),
            toggle_pins: "Ctrl+Shift+H".into(),
            rescue: "Ctrl+Alt+Escape".into(),
            color_picker: String::new(),
            settings: String::new(),
        }
    }
}
impl Shortcuts {
    fn raw_bindings(&self) -> [(&'static str, &str); 8] {
        [
            ("capture", &self.capture),
            ("window", &self.window),
            ("last_region", &self.last_region),
            ("pin_clipboard", &self.pin_clipboard),
            ("toggle_pins", &self.toggle_pins),
            ("rescue", &self.rescue),
            ("color_picker", &self.color_picker),
            ("settings", &self.settings),
        ]
        .map(|(action, value)| (action, value.as_str()))
    }
    pub fn bindings(&self) -> [(&'static str, Option<&str>); 8] {
        self.raw_bindings().map(|(action, value)| {
            (
                action,
                if value.trim().is_empty() && action != "rescue" {
                    None
                } else {
                    Some(value)
                },
            )
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Capture {
    pub magnifier: bool,
    pub dimensions: bool,
    pub detect_window: bool,
    pub detect_elements: bool,
    pub include_cursor: bool,
    pub remember_region: bool,
    pub delay_seconds: u8,
    pub dim_percent: u8,
}
impl Default for Capture {
    fn default() -> Self {
        Self {
            magnifier: true,
            dimensions: true,
            detect_window: true,
            detect_elements: false,
            include_cursor: false,
            remember_region: true,
            delay_seconds: 0,
            dim_percent: 52,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Toolbar {
    /// 0: grid, 1: horizontal scrolling, 2: legacy fixed width (displayed as single row), 3: single row.
    pub layout: u8,
    pub columns: u8,
    pub rows: u8,
    pub width: u16,
    pub mode: ToolbarMode,
    pub position: ToolbarPosition,
    pub groups: bool,
    pub show_customize: bool,
    pub order: Vec<Tool>,
    pub hidden: Vec<Tool>,
}
impl Default for Toolbar {
    fn default() -> Self {
        Self {
            layout: 3,
            columns: 8,
            rows: 2,
            width: 640,
            mode: ToolbarMode::Icons,
            position: ToolbarPosition::Follow,
            groups: true,
            show_customize: true,
            order: vec![
                Tool::Select,
                Tool::Rect,
                Tool::Arrow,
                Tool::Pen,
                Tool::Text,
                Tool::Highlight,
                Tool::Redact,
                Tool::Mosaic,
                Tool::Picker,
            ],
            hidden: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnnotationDefaults {
    pub tool: Tool,
    pub stroke_width: u8,
    pub color: String,
    pub text_size: u8,
    pub text_font: String,
    pub arrow_style: ArrowStyle,
    pub redact_style: RedactStyle,
    pub keep_objects: bool,
}
impl Default for AnnotationDefaults {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            stroke_width: 3,
            color: "#ed6571".into(),
            text_size: 22,
            text_font: "Microsoft YaHei UI".into(),
            arrow_style: ArrowStyle::Filled,
            redact_style: RedactStyle::Solid,
            keep_objects: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Output {
    pub finish: FinishAction,
    pub format: ImageFormat,
    pub quality: u8,
    /// Empty selects the platform Pictures/Glint directory at use time, not a translated path.
    pub directory: String,
    pub filename: String,
    pub file_conflict: FileConflict,
    pub auto_save: bool,
    pub feedback: Feedback,
    pub strip_metadata: bool,
}
impl Default for Output {
    fn default() -> Self {
        Self {
            finish: FinishAction::Copy,
            format: ImageFormat::Png,
            quality: 90,
            directory: String::new(),
            filename: "Glint_{date}_{time}".into(),
            file_conflict: FileConflict::Reject,
            auto_save: false,
            feedback: Feedback::Toast,
            strip_metadata: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pins {
    pub on_top: bool,
    pub opacity: u8,
    pub zoom_percent: u16,
    pub wheel: PinWheel,
    pub snap: bool,
    pub passthrough: bool,
    pub restore_session: bool,
    pub hide_on_capture: bool,
}
impl Default for Pins {
    fn default() -> Self {
        Self {
            on_top: true,
            opacity: 100,
            zoom_percent: 100,
            wheel: PinWheel::Zoom,
            snap: true,
            passthrough: false,
            restore_session: false,
            hide_on_capture: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Privacy {
    pub history: bool,
    pub history_disk: bool,
    pub keep_days: u8,
    pub history_budget_mib: u16,
    pub excluded_apps: String,
    pub diagnostics: bool,
    pub clear_on_exit: bool,
}
impl Default for Privacy {
    fn default() -> Self {
        Self {
            history: false,
            history_disk: false,
            keep_days: 7,
            history_budget_mib: 128,
            excluded_apps: String::new(),
            diagnostics: false,
            clear_on_exit: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub theme: Theme,
    pub ui_font: UiFont,
    pub accent: String,
    pub scale: Scale,
    pub reduce_motion: bool,
    pub high_contrast: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            ui_font: UiFont::System,
            accent: "#4b5cfa".into(),
            scale: Scale::System,
            reduce_motion: true,
            high_contrast: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Resources {
    pub idle_pause: bool,
    pub preview_budget_mib: u16,
    pub pin_budget_mib: u16,
    pub capture_source: CaptureSource,
    pub sleep_release: bool,
}
impl Default for Resources {
    fn default() -> Self {
        Self {
            idle_pause: true,
            preview_budget_mib: 128,
            pin_budget_mib: 256,
            capture_source: CaptureSource::Auto,
            sleep_release: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationIssue {
    pub field: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    TooLarge,
    Syntax,
    UnsupportedVersion(i64),
    Invalid(Vec<ValidationIssue>),
}
impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => f.write_str("config.too_large"),
            Self::Syntax => f.write_str("config.invalid_toml"),
            Self::UnsupportedVersion(version) => write!(f, "config.unsupported_version: {version}"),
            Self::Invalid(issues) => {
                write!(f, "config.validation_failed: {} issue(s)", issues.len())
            }
        }
    }
}
impl std::error::Error for ConfigError {}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        if text.len() > MAX_CONFIG_BYTES {
            return Err(ConfigError::TooLarge);
        }
        let value: toml::Value = toml::from_str(text).map_err(|_| ConfigError::Syntax)?;
        let version = match value.get("config_version") {
            None => i64::from(CONFIG_VERSION),
            Some(value) => value.as_integer().ok_or(ConfigError::Syntax)?,
        };
        // No deployed schema precedes v1. Refuse older/future schemas instead of inventing a migration.
        if version != i64::from(CONFIG_VERSION) {
            return Err(ConfigError::UnsupportedVersion(version));
        }
        let config: Self = value.try_into().map_err(|_| ConfigError::Syntax)?;
        config.validate()?;
        Ok(config)
    }
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        self.validate()?;
        let encoded = toml::to_string_pretty(self).map_err(|_| ConfigError::Syntax)?;
        if encoded.len() > MAX_CONFIG_BYTES {
            return Err(ConfigError::TooLarge);
        }
        Ok(encoded)
    }
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.config_version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(i64::from(
                self.config_version,
            )));
        }
        let mut issues = Vec::new();
        let mut check = |ok: bool, field, message| {
            if !ok {
                issues.push(ValidationIssue { field, message });
            }
        };
        check(
            self.capture.delay_seconds <= 10,
            "capture.delay_seconds",
            "config.out_of_range",
        );
        check(
            (20..=80).contains(&self.capture.dim_percent),
            "capture.dim_percent",
            "config.out_of_range",
        );
        check(
            (1..=12).contains(&self.annotation.stroke_width),
            "annotation.stroke_width",
            "config.out_of_range",
        );
        check(
            (12..=48).contains(&self.annotation.text_size),
            "annotation.text_size",
            "config.out_of_range",
        );
        check(
            valid_hex(&self.annotation.color),
            "annotation.color",
            "config.invalid_color",
        );
        check(
            !self.annotation.text_font.trim().is_empty()
                && self.annotation.text_font.len() <= 256
                && !self.annotation.text_font.chars().any(char::is_control),
            "annotation.text_font",
            "config.invalid_text",
        );
        check(
            (60..=100).contains(&self.output.quality),
            "output.quality",
            "config.out_of_range",
        );
        check(
            self.output.directory.len() <= 8192
                && !self.output.directory.chars().any(char::is_control),
            "output.directory",
            "config.invalid_path",
        );
        check(
            valid_filename_template(&self.output.filename),
            "output.filename",
            "config.invalid_filename_template",
        );
        check(
            (20..=100).contains(&self.pins.opacity),
            "pins.opacity",
            "config.out_of_range",
        );
        check(
            (25..=200).contains(&self.pins.zoom_percent),
            "pins.zoom_percent",
            "config.out_of_range",
        );
        check(
            (1..=30).contains(&self.privacy.keep_days),
            "privacy.keep_days",
            "config.out_of_range",
        );
        check(
            (32..=512).contains(&self.privacy.history_budget_mib),
            "privacy.history_budget_mib",
            "config.out_of_range",
        );
        check(
            self.privacy.excluded_apps.len() <= 16384 && !self.privacy.excluded_apps.contains('\0'),
            "privacy.excluded_apps",
            "config.invalid_text",
        );
        check(
            valid_hex(&self.appearance.accent),
            "appearance.accent",
            "config.invalid_color",
        );
        check(
            (64..=512).contains(&self.resources.preview_budget_mib),
            "resources.preview_budget_mib",
            "config.out_of_range",
        );
        check(
            (64..=512).contains(&self.resources.pin_budget_mib),
            "resources.pin_budget_mib",
            "config.out_of_range",
        );
        check(
            self.toolbar.order.len() == 9
                && self.toolbar.order.iter().collect::<HashSet<_>>().len() == 9,
            "toolbar.order",
            "config.invalid_tool_order",
        );
        check(
            !self.toolbar.hidden.contains(&Tool::Select)
                && self.toolbar.hidden.len() <= 8
                && self.toolbar.hidden.iter().collect::<HashSet<_>>().len()
                    == self.toolbar.hidden.len(),
            "toolbar.hidden",
            "config.required_tool_hidden",
        );
        check(
            self.toolbar.layout <= 3
                && (2..=20).contains(&self.toolbar.columns)
                && (1..=6).contains(&self.toolbar.rows)
                && (100..=1200).contains(&self.toolbar.width),
            "toolbar.layout",
            "config.out_of_range",
        );
        let mut chords = HashSet::new();
        for (action, value) in self.shortcuts.raw_bindings() {
            check(
                value.len() <= 80 && !value.chars().any(char::is_control),
                action,
                "config.invalid_shortcut",
            );
        }
        for (action, value) in self.shortcuts.bindings() {
            if let Some(value) = value {
                match KeyChord::parse(value) {
                    Some(chord) => {
                        check(
                            chord.control
                                || chord.alt
                                || chord.shift
                                || chord.meta
                                || (chord.key.len() > 1 && chord.key.starts_with('F')),
                            action,
                            "config.shortcut_requires_modifier",
                        );
                        check(chords.insert(chord), action, "config.duplicate_shortcut");
                    }
                    None => check(false, action, "config.invalid_shortcut"),
                }
            }
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(issues))
        }
    }
}

pub fn valid_hex(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

pub(crate) fn valid_filename_template(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 180
        || value.ends_with([' ', '.'])
        || value
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return false;
    }
    let mut remaining = value.to_owned();
    for variable in ["{date}", "{time}", "{width}", "{height}"] {
        remaining = remaining.replace(variable, "1");
    }
    if remaining.contains(['{', '}']) {
        return false;
    }
    let stem = remaining
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// A portable normalized chord. OS availability is validated separately during registration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: String,
}
impl KeyChord {
    pub fn parse(value: &str) -> Option<Self> {
        if value.len() > 80 {
            return None;
        }
        let mut chord = Self {
            control: false,
            alt: false,
            shift: false,
            meta: false,
            key: String::new(),
        };
        for part in value.split('+').map(|p| p.trim().to_ascii_uppercase()) {
            let modifier = match part.as_str() {
                "CTRL" | "CONTROL" => Some(&mut chord.control),
                "ALT" => Some(&mut chord.alt),
                "SHIFT" => Some(&mut chord.shift),
                "WIN" | "META" | "SUPER" => Some(&mut chord.meta),
                _ => None,
            };
            if let Some(modifier) = modifier {
                if *modifier {
                    return None;
                }
                *modifier = true;
                continue;
            }
            if !chord.key.is_empty() {
                return None;
            }
            let key = if part == "ESC" { "ESCAPE" } else { &part };
            let named = matches!(
                key,
                "ESCAPE"
                    | "SPACE"
                    | "TAB"
                    | "ENTER"
                    | "BACKSPACE"
                    | "DELETE"
                    | "INSERT"
                    | "HOME"
                    | "END"
                    | "PAGEUP"
                    | "PAGEDOWN"
                    | "LEFT"
                    | "RIGHT"
                    | "UP"
                    | "DOWN"
            );
            let function = key
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
            let character = key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric();
            if !(named || function || character) {
                return None;
            }
            chord.key = if function {
                format!("F{}", key[1..].parse::<u8>().ok()?)
            } else {
                key.into()
            };
        }
        if chord.key.is_empty() {
            None
        } else {
            Some(chord)
        }
    }
}

/// UI draft previews read `draft`; running services continue to read `saved` until commit succeeds.
#[derive(Clone, Debug)]
pub struct Preferences {
    saved: Config,
    pub draft: Config,
}
impl Preferences {
    pub fn new(config: Config) -> Self {
        Self {
            draft: config.clone(),
            saved: config,
        }
    }
    pub fn saved(&self) -> &Config {
        &self.saved
    }
    pub fn is_dirty(&self) -> bool {
        self.saved != self.draft
    }
    pub fn cancel(&mut self) {
        self.draft = self.saved.clone();
    }
    pub fn restore_defaults(&mut self) {
        let onboarding_complete = self.draft.general.onboarding_complete;
        self.draft = Config::default();
        self.draft.general.onboarding_complete = onboarding_complete;
    }
    /// Call only after registration and persistence succeed. Validation alone is not a save.
    pub fn accept_saved(&mut self) -> Result<(), ConfigError> {
        self.draft.validate()?;
        self.saved = self.draft.clone();
        Ok(())
    }
}
