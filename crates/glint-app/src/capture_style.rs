//! Saved presentation and annotation defaults for one capture session; source pixels stay intact.
use super::{CaptureLabels, CaptureProbe};
use crate::messages::{self, Message};
use glint_core::config::{ArrowStyle, Config, RedactStyle, Theme, Tool};

pub struct Presentation {
    pub preview_budget_mib: u16,
    pub reduce_motion: bool,
    pub interface_scale: glint_core::config::Scale,
    pub english: bool,
    pub magnifier: bool,
    pub dimensions: bool,
    pub dim_percent: u8,
    pub dark: bool,
    pub theme: Theme,
    pub contrast: bool,
    pub accent: slint::Color,
    pub notice: String,
    pub filled_arrows: bool,
    pub text_font: String,
    pub text_size: u16,
    pub output: glint_core::config::Output,
    pub tool: i32,
    pub stroke_width: u8,
    pub mark_color: slint::Color,
    pub feedback_mode: i32,
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            preview_budget_mib: 128,
            reduce_motion: true,
            interface_scale: glint_core::config::Scale::System,
            english: false,
            magnifier: true,
            dimensions: true,
            dim_percent: 52,
            dark: false,
            theme: Theme::Light,
            contrast: false,
            accent: slint::Color::from_rgb_u8(0x4b, 0x5c, 0xfa),
            notice: String::new(),
            filled_arrows: true,
            text_font: "Microsoft YaHei UI".into(),
            text_size: 22,
            output: Default::default(),
            tool: 0,
            stroke_width: 3,
            mark_color: slint::Color::from_rgb_u8(0xed, 0x65, 0x71),
            feedback_mode: 0,
        }
    }
}
impl Presentation {
    pub fn from_config(config: &Config, preferred: &[String]) -> Self {
        let english = crate::settings::english(config.general.language, preferred);
        let system_dark = glint_platform::system_dark_theme();
        let dark = match config.appearance.theme {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::System => system_dark.unwrap_or(false),
        };
        let mut result = Self {
            theme: config.appearance.theme,
            preview_budget_mib: config.resources.preview_budget_mib,
            reduce_motion: config.appearance.reduce_motion,
            interface_scale: config.appearance.scale,
            english,
            magnifier: config.capture.magnifier,
            dimensions: config.capture.dimensions,
            dim_percent: config.capture.dim_percent,
            dark,
            contrast: config.appearance.high_contrast,
            filled_arrows: config.annotation.arrow_style == ArrowStyle::Filled,
            text_font: config.annotation.text_font.clone(),
            text_size: u16::from(config.annotation.text_size),
            output: config.output.clone(),
            tool: match config.annotation.tool {
                Tool::Select => 0,
                Tool::Redact if config.annotation.redact_style == RedactStyle::Pixelate => 7,
                Tool::Redact => 1,
                Tool::Pen => 2,
                Tool::Rect => 3,
                Tool::Arrow => 4,
                Tool::Highlight => 5,
                Tool::Picker => 6,
                Tool::Mosaic => 7,
                Tool::Text => 8,
            },
            stroke_width: config.annotation.stroke_width,
            feedback_mode: match config.output.feedback {
                glint_core::config::Feedback::Toast => 0,
                glint_core::config::Feedback::None => 1,
                glint_core::config::Feedback::Notification => 2,
            },
            ..Self::default()
        };
        if glint_core::config::valid_hex(&config.appearance.accent)
            && let Ok(rgb) = u32::from_str_radix(&config.appearance.accent[1..], 16)
        {
            result.accent =
                slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        }
        if config.appearance.theme == Theme::System && system_dark.is_none() {
            result.notice = messages::text(Message::SystemThemeUnavailable, english).into();
        }
        if glint_core::config::valid_hex(&config.annotation.color)
            && let Ok(rgb) = u32::from_str_radix(&config.annotation.color[1..], 16)
        {
            result.mark_color =
                slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        }
        result
    }
    pub fn apply(self, ui: &CaptureProbe) {
        crate::motion::configure!(ui, super::Motion, self.reduce_motion);
        crate::motion::install(ui, self.reduce_motion);
        crate::interface_scale::install_with_theme(
            ui,
            self.interface_scale,
            false,
            self.theme,
            apply_system_theme,
        );
        ui.set_interface_scale(crate::interface_scale::factor(self.interface_scale));
        ui.set_english(self.english);
        ui.set_magnifier_enabled(self.magnifier);
        ui.set_dimensions(self.dimensions);
        ui.set_dim_percent(i32::from(self.dim_percent));
        ui.set_dark(self.dark);
        ui.set_contrast(self.contrast);
        ui.set_accent(self.accent);
        ui.set_appearance_notice(self.notice.into());
        ui.set_filled_arrows(self.filled_arrows);
        ui.set_text_font(self.text_font.into());
        ui.set_text_size(i32::from(self.text_size));
        ui.set_tool(self.tool);
        ui.set_stroke_width(i32::from(self.stroke_width));
        ui.set_mark_color(self.mark_color);
        ui.set_default_mark_color(self.mark_color);
        ui.set_feedback_mode(self.feedback_mode);
    }
}
pub(crate) fn apply_system_theme(ui: &CaptureProbe, dark: bool) {
    ui.set_dark(dark);
    ui.set_appearance_notice("".into());
    ui.invoke_apply_theme();
}
pub fn apply_labels(ui: &CaptureProbe) {
    ui.set_output_error_title(
        messages::text(Message::CaptureOutputFailed, ui.get_english()).into(),
    );
    ui.set_output_error_dismiss(messages::text(Message::PinDismiss, ui.get_english()).into());
    ui.set_output_error_retry(messages::text(Message::FontRetry, ui.get_english()).into());
    ui.set_font_button(messages::text(Message::FontTitle, ui.get_english()).into());
    let tr = |id| messages::text(id, ui.get_english()).into();
    ui.set_labels(CaptureLabels {
        color_picker: tr(Message::ColorPickerEntry),
        magnifier: tr(Message::CaptureMagnifier),
        pin: tr(Message::CapturePin),
        save_elsewhere: tr(Message::CaptureSaveElsewhere),
        toolbar_customize: tr(Message::ToolbarCustomize),
        toolbar_title: tr(Message::ToolbarTitle),
        toolbar_more: tr(Message::ToolbarMore),
        toolbar_both: tr(Message::ToolbarBoth),
        toolbar_icons: tr(Message::ToolbarIcons),
        toolbar_follow: tr(Message::ToolbarFollow),
        toolbar_top: tr(Message::ToolbarTop),
        toolbar_bottom: tr(Message::ToolbarBottom),
        toolbar_groups: tr(Message::ToolbarGroups),
        toolbar_order_hint: tr(Message::ToolbarOrderHint),
        toolbar_preview_hint: tr(Message::ToolbarPreviewHint),
        toolbar_apply: tr(Message::ToolbarApply),
        toolbar_reset: tr(Message::ToolbarReset),
        title: tr(if ui.get_diagnostic() {
            Message::CaptureDiagnosticTitle
        } else {
            Message::CaptureTitle
        }),
        select: tr(Message::CaptureSelect),
        pen: tr(Message::CapturePen),
        redact: tr(Message::CaptureRedact),
        rectangle: tr(Message::CaptureRectangle),
        arrow: tr(Message::CaptureArrow),
        highlight: tr(Message::CaptureHighlight),
        undo: tr(Message::CaptureUndo),
        redo: tr(Message::CaptureRedo),
        save: tr(Message::CaptureSave),
        copy: tr(Message::CaptureCopy),
        cancel: tr(Message::CaptureCancel),
        stroke: tr(Message::CaptureStrokeWidth),
        pen_hint: tr(Message::CapturePenHint),
        select_hint: tr(Message::CaptureSelectHint),
        object_hint: tr(Message::CaptureObjectHint),
        object_text_hint: tr(Message::CaptureObjectTextHint),
        object_delete: tr(Message::CaptureObjectDelete),
        arrow_filled: tr(Message::CaptureArrowFilled),
        arrow_open: tr(Message::CaptureArrowOpen),
        text_edit: tr(Message::CaptureTextEdit),
        redact_hint: tr(Message::CaptureRedactHint),
        rectangle_hint: tr(Message::CaptureRectangleHint),
        arrow_hint: tr(Message::CaptureArrowHint),
        highlight_hint: tr(Message::CaptureHighlightHint),
        picker: tr(Message::CapturePicker),
        text: tr(Message::CaptureText),
        text_hint: tr(Message::CaptureTextHint),
        text_place: tr(Message::CaptureTextPlace),
        text_cancel: tr(Message::CaptureTextCancel),
        text_size: tr(Message::CaptureTextSize),
        text_font: tr(Message::CaptureTextFont),
        text_limit: tr(Message::CaptureTextLimit),
        pixelate: tr(Message::CapturePixelate),
        pixelate_hint: tr(Message::CapturePixelateHint),
        picker_hint: tr(Message::CapturePickerHint),
        copy_hex: tr(Message::CaptureCopyHex),
        copy_rgb: tr(Message::CaptureCopyRgb),
    });
}
