use crate::messages::{self, Message};
use glint_core::config::{Config, Language, Theme};
use slint::{Model, ModelRc, VecModel};
use std::rc::Rc;
glint_ui::slint! { export { Motion } from "@glint_ui/widgets/motion.slint"; export { SettingsWindow, SettingRow } from "src/settings.slint"; export { ToolbarRow } from "@glint_ui/components/toolbar_editor.slint"; }
#[path = "settings_fields.rs"]
pub mod fields;

pub fn detect_language(preferred: &[String]) -> Option<Language> {
    glint_ui::i18n::Locale::detect(preferred).map(|locale| match locale {
        glint_ui::i18n::Locale::En => Language::En,
        glint_ui::i18n::Locale::ZhCn => Language::ZhCn,
    })
}
pub fn english(language: Language, preferred: &[String]) -> bool {
    match language {
        Language::En => true,
        Language::ZhCn => false,
        Language::System => detect_language(preferred) != Some(Language::ZhCn),
    }
}

pub fn sync(
    ui: &SettingsWindow,
    config: &Config,
    model: &Rc<VecModel<SettingRow>>,
    preferred: &[String],
    writable: bool,
    dirty: bool,
) -> Result<(), String> {
    crate::motion::configure!(ui, Motion, config.appearance.reduce_motion);
    ui.global::<Motion>()
        .set_ui_font_family(crate::ui_font::selected(config.appearance.ui_font).into());
    crate::motion::preview(ui.window(), config.appearance.reduce_motion)?;
    let en = english(config.general.language, preferred);
    let scale = crate::interface_scale::factor(config.appearance.scale);
    ui.set_interface_scale(scale);
    crate::interface_scale::preview(ui.window(), scale)?;
    let tr = |id| messages::text(id, en);
    ui.set_font_english(en);
    ui.set_font_button(tr(Message::FontTitle).into());
    let system_dark = glint_platform::system_dark_theme();
    let dark = crate::appearance::dark(config.appearance.theme, system_dark);
    ui.set_dark(dark);
    ui.invoke_apply_theme(dark);
    ui.set_contrast(config.appearance.high_contrast);
    if glint_core::config::valid_hex(&config.appearance.accent)
        && let Ok(rgb) = u32::from_str_radix(&config.appearance.accent[1..], 16)
    {
        ui.set_accent(slint::Color::from_rgb_u8(
            (rgb >> 16) as u8,
            (rgb >> 8) as u8,
            rgb as u8,
        ));
    }
    // Preserve category delegates and keyboard focus when a page refreshes.
    ui.set_categories(crate::toolbar::update_model(
        ui.get_categories(),
        fields::CATEGORIES
            .iter()
            .map(|(zh, english)| slint::SharedString::from(fields::label(en, zh, english)))
            .collect::<Vec<_>>(),
    ));
    ui.set_heading(tr(Message::Settings).into());
    ui.set_subtitle(tr(Message::SettingsSubtitle).into());
    ui.set_close_label(tr(Message::CloseToTray).into());
    ui.set_save_label(tr(Message::SaveSettings).into());
    ui.set_cancel_label(tr(Message::CancelChanges).into());
    ui.set_reset_label(tr(Message::RestoreDefaults).into());
    ui.set_unbound_label(tr(Message::Unbound).into());
    ui.set_continue_label(tr(Message::ContinueEditing).into());
    ui.set_discard_label(tr(Message::DiscardAndLeave).into());
    ui.set_save_leave_label(tr(Message::SaveAndLeave).into());
    ui.set_clear_label(tr(Message::ClearBinding).into());
    ui.set_accept_key_label(tr(Message::AcceptShortcut).into());
    ui.set_stage_label(tr(Message::DevelopmentStatus).into());
    ui.set_page_note(tr(Message::DraftExplanation).into());
    ui.set_writable(writable);
    let names = crate::toolbar::names(en);
    ui.set_toolbar_rows(crate::toolbar::update_model(
        ui.get_toolbar_rows(),
        config
            .toolbar
            .order
            .iter()
            .map(|tool| ToolbarRow {
                tool: crate::toolbar::index(*tool),
                label: names[crate::toolbar::index(*tool) as usize].clone(),
                shown: !config.toolbar.hidden.contains(tool),
            })
            .collect::<Vec<_>>(),
    ));
    ui.set_toolbar_items(ModelRc::new(VecModel::from(crate::toolbar::items(
        &config.toolbar,
    ))));
    ui.set_toolbar_names(ModelRc::new(VecModel::from(names)));
    ui.set_toolbar_labels(crate::toolbar::labels(en));
    ui.set_toolbar_icons(config.toolbar.mode == glint_core::config::ToolbarMode::Icons);
    ui.set_toolbar_position(crate::toolbar::position(&config.toolbar));
    ui.set_toolbar_groups(config.toolbar.groups);
    ui.set_toolbar_layout(if config.toolbar.layout == 2 {
        3
    } else {
        config.toolbar.layout as i32
    });
    ui.set_toolbar_customize_visible(config.toolbar.show_customize);
    ui.set_toolbar_columns(config.toolbar.columns as i32);
    ui.set_toolbar_grid_rows(config.toolbar.rows as i32);
    ui.set_toolbar_width_setting(config.toolbar.width as i32);

    ui.set_dirty(dirty);
    ui.set_status(
        if config.appearance.theme == Theme::System && system_dark.is_none() {
            tr(Message::SystemThemeUnavailable)
        } else if dirty {
            tr(Message::UnsavedChanges)
        } else {
            ""
        }
        .into(),
    );
    if ui.get_guide_step() >= 0 || ui.get_workbench() {
        return Ok(());
    }
    let mut next = fields::build(
        config,
        ui.get_category().clamp(0, 12) as usize,
        en,
        writable,
    )?;
    if config.appearance.theme == Theme::System
        && system_dark.is_none()
        && let Some(row) = next.iter_mut().find(|row| row.key == "appearance.theme")
    {
        row.description = tr(Message::SystemThemeUnavailable).into();
    }
    if ui.get_category() == 0
        && let Some(row) = next.iter_mut().find(|row| row.key == "general.language")
    {
        row.description = format!(
            "{} {} · {}",
            tr(Message::SystemPreference),
            if preferred.is_empty() {
                "—".into()
            } else {
                preferred.join(", ")
            },
            if config.general.language != Language::System {
                tr(Message::ManualLanguage)
            } else if detect_language(preferred).is_none() {
                tr(Message::LanguageFallback)
            } else {
                tr(Message::AutomaticLanguage)
            }
        )
        .into();
    }
    if model.row_count() == next.len()
        && model.iter().zip(&next).all(|(old, new)| old.key == new.key)
    {
        for (index, row) in next.into_iter().enumerate() {
            model.set_row_data(index, row);
        }
    } else {
        model.set_vec(next);
    }
    Ok(())
}

pub fn modal(ui: &SettingsWindow, kind: i32, en: bool, detail: &str) {
    let tr = |id| messages::text(id, en);
    ui.invoke_show_modal(kind);
    ui.set_modal_title(
        match kind {
            1 => tr(Message::UnsavedTitle),
            2 => tr(Message::ResetTitle),
            _ => tr(Message::RecordShortcutTitle),
        }
        .into(),
    );
    ui.set_modal_body(
        match kind {
            1 => tr(Message::UnsavedBody),
            2 => tr(Message::ResetBody),
            _ => detail,
        }
        .into(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_preferences_match_prototype_fallback_and_override_rules() {
        let tags = |values: &[&str]| {
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        };
        for values in [vec!["zh-CN"], vec!["zh-Hans-SG"], vec!["fr-FR", "zh_CN"]] {
            assert_eq!(detect_language(&tags(&values)), Some(Language::ZhCn));
        }
        for values in [
            vec!["en-US"],
            vec!["zh-TW", "en-GB"],
            vec!["zh-Hant", "en-US", "zh-CN"],
        ] {
            assert_eq!(detect_language(&tags(&values)), Some(Language::En));
        }
        assert_eq!(detect_language(&tags(&["zh-HK", "fr-FR"])), None);
        assert!(!english(Language::ZhCn, &tags(&["en-US"])));
        assert!(english(Language::En, &tags(&["zh-CN"])));
        assert!(english(Language::System, &[]));
    }
    #[test]
    fn all_forms_have_schema_fields_and_disabled_capabilities_cannot_be_changed() {
        let config = Config::default();
        for category in 0..13 {
            for language in [false, true] {
                assert!(
                    !fields::build(&config, category, language, true)
                        .unwrap()
                        .is_empty()
                );
            }
        }
        assert!(
            fields::change(&config, "general.start_on_login", "true", false)
                .unwrap()
                .general
                .start_on_login
        );
        assert!(fields::change(&config, "privacy.history_disk", "true", false).is_err());
        assert_eq!(
            fields::change(&config, "annotation.text_size", "18", false)
                .unwrap()
                .annotation
                .text_size,
            18
        );
        assert!(fields::change(&config, "annotation.text_size", "49", false).is_err());
    }
    #[test]
    fn imported_reserved_off_values_show_effective_behavior_without_mutating_configuration() {
        let config = glint_core::config_transfer::preview(
            &Config::default(),
            "config_version=1\n[general]\nclose_to_tray=false\nsingle_instance=false\n[annotation]\nkeep_objects=false\n[output]\nstrip_metadata=false\n[resources]\nidle_pause=false\n",
        ).unwrap().candidate;
        let before = config.clone();
        for (category, key) in [
            (0, "general.close_to_tray"),
            (0, "general.single_instance"),
            (4, "annotation.keep_objects"),
            (5, "output.strip_metadata"),
            (9, "resources.idle_pause"),
        ] {
            for english in [false, true] {
                for writable in [false, true] {
                    let rows = fields::build(&config, category, english, writable).unwrap();
                    let row = rows.iter().find(|row| row.key == key).unwrap();
                    assert!(row.checked && !row.editable);
                    assert!(row.description.contains(if english {
                        "configured off value has no effect"
                    } else {
                        "关闭值当前不生效"
                    }));
                    assert!(fields::change(&config, key, "true", english).is_err());
                }
            }
        }
        assert_eq!(config, before);
        let exported = glint_core::config_transfer::export(&config).unwrap();
        assert_eq!(
            glint_core::config_transfer::preview(&Config::default(), &exported)
                .unwrap()
                .candidate,
            config
        );
        let planned = fields::build(&config, 0, true, true).unwrap();
        let updates = planned
            .iter()
            .find(|row| row.key == "general.check_updates")
            .unwrap();
        assert!(!updates.checked && !updates.editable);
        assert!(updates.description.contains("Unavailable"));
    }
}
