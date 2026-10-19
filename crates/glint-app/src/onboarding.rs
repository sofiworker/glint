//! Six-step resident guide. Uses the real settings draft and capture pipeline.
use crate::{
    resident::AppState,
    settings::{self, SettingsWindow},
};
use slint::Model;

pub fn initial(
    session: &crate::preferences::SettingsSession,
    explicit: bool,
    background: bool,
) -> bool {
    !explicit
        && !background
        && session.writable
        && session.notice.is_empty()
        && !session.preferences.saved().general.onboarding_complete
}

pub fn sync(state: &AppState, ui: &SettingsWindow) -> Result<(), String> {
    let en = state.en();
    let tr = |zh, english| settings::fields::label(en, zh, english);
    ui.set_guide_open_label(tr("重新查看首次引导", "Review first-run guide").into());
    let step = ui.get_guide_step();
    if !(0..=5).contains(&step) {
        return Ok(());
    }
    let titles = [
        ("截完就走，不打断当前工作。", "Capture and carry on."),
        ("选一组顺手的快捷键", "Choose your shortcuts"),
        ("截完以后，怎样最顺手？", "What happens when you finish?"),
        ("选一个舒服的外观", "Choose your appearance"),
        ("用一次，就知道了。", "Try a real capture"),
        ("准备好了，回到你的工作。", "Ready. Back to your work."),
    ];
    let bodies = [
        (
            "热键呼起 → 框选 → 原地标注与打码 → 复制或贴图 → 离开。\n关闭设置后只留下托盘与热键；默认不自动保存截图、不收集历史、不上传内容。语言跟随系统，也可手动覆盖。",
            "Shortcut → select → annotate and redact in place → copy or pin → leave.\nClosing settings leaves the tray and shortcuts ready. Automatic copies and history are off by default; images stay local. Follow the system language or choose an override.",
        ),
        (
            "点击组合键录入，也可清除可选绑定。保存并演练或完成时执行真实系统注册；冲突或保存失败保留原可用绑定和当前草稿，托盘截图始终可用。",
            "Click a shortcut to record it, or clear optional bindings. Save and practice or Finish registers the real system shortcuts. Conflicts or save failures keep the working bindings and draft; tray capture remains available.",
        ),
        (
            "Enter / 双击按默认动作完成。开机启动与自动副本需要主动开启，默认关闭。历史尚未接入，保持关闭。所有修改在保存并演练或完成时应用；稍后设置可保存或放弃。",
            "Enter / double-click uses the default finish action. Login startup and automatic copies require opt-in and are off by default. History is not connected and stays off. Changes apply when you save and practice or finish. Later lets you save or discard them.",
        ),
        (
            "主题、强调色与减少动效即时预览，不改变截图源像素。开启减少动效后，控件悬停与状态颜色立即切换；不影响选区拖动、绘制或倒计时。保存后用于新浮层与贴图，已有贴图保持当前偏好。",
            "Preview the theme, accent and reduced motion without changing source pixels. Reduced motion switches hover and state colors immediately; selection, drawing and countdown timing stay unchanged. Save applies to new overlays and pins; existing pins keep their preference.",
        ),
        (
            "保存当前选择后进入真实桌面截图。拖动框选，在浮层中选择箭头或实心遮盖，再复制、保存或贴图。成功后回到完成页；Esc 取消返回本页，失败保留编辑。也可跳过练习。\nWindows 普通桌面截图不要求管理员权限；受保护内容和安全桌面不保证可捕获。仅在主动截图时采集，不预先请求录音或录屏权限。",
            "Save your choices and capture the real desktop. Drag a region, add an arrow or solid cover in the overlay, then copy, save or pin. Success returns to the final step; Esc returns here and output failures preserve edits. Practice is optional.\nOrdinary Windows desktop capture needs no administrator rights. Protected content and secure desktops are not guaranteed. Capture starts only on request; no microphone or recording permission is requested.",
        ),
        (
            "完成后收起到托盘。截图后直接离开浮层，无须返回工作台。可以随时从托盘打开设置、重新查看引导或使用救援入口。未绑定快捷键时也能使用托盘截图。",
            "Finish to keep Glint in the tray. Captures finish directly in the overlay. Open settings, review this guide or rescue pins from the tray. Tray capture works even without a shortcut.",
        ),
    ];
    ui.set_heading(tr("拾光 · 首次引导", "Glint · First-run guide").into());
    ui.set_subtitle(
        tr(
            "欢迎 → 热键 → 默认行为 → 外观 → 试一次 → 完成",
            "Welcome → Shortcuts → Behavior → Appearance → Practice → Done",
        )
        .into(),
    );
    ui.set_close_label(tr("稍后设置", "Set up later").into());
    let (zh, english) = titles[step as usize];
    ui.set_guide_title(format!("{}. {}", step + 1, tr(zh, english)).into());
    let (zh, english) = bodies[step as usize];
    ui.set_page_note(tr(zh, english).into());
    ui.set_guide_back_label(tr("上一步", "Back").into());
    ui.set_guide_next_label(
        if step == 5 {
            tr("开始使用 · 收起到托盘", "Finish · Keep in tray")
        } else {
            tr("下一步", "Next")
        }
        .into(),
    );
    ui.set_guide_practice_label(tr("保存并进入截图演练", "Save and practice capture").into());
    ui.set_guide_skip_label(tr("跳过演练", "Skip practice").into());
    let config = &state.session.preferences.draft;
    let keys: &[&str] = match step {
        0 => &["general.language"],
        1 => &[
            "shortcuts.capture",
            "shortcuts.pin_clipboard",
            "shortcuts.rescue",
        ],
        2 => &[
            "output.finish",
            "general.start_on_login",
            "output.auto_save",
            "output.directory",
            "privacy.history",
        ],
        3 => &[
            "appearance.theme",
            "appearance.accent",
            "appearance.reduce_motion",
        ],
        5 => &[
            "shortcuts.capture",
            "output.finish",
            "output.auto_save",
            "privacy.history",
        ],
        _ => &[],
    };
    let mut rows = Vec::new();
    for category in [0, 1, 5, 7, 8] {
        rows.extend(
            settings::fields::build(config, category, en, state.session.writable)?
                .into_iter()
                .filter(|row| keys.contains(&row.key.as_str())),
        );
    }
    rows.sort_by_key(|row| keys.iter().position(|key| *key == row.key.as_str()));
    if step == 5 {
        for row in &mut rows {
            row.editable = false;
        }
    }
    // Preserve the existing model identity and avoid losing keyboard focus on each edit.
    if state.rows.row_count() == rows.len()
        && state.rows.iter().zip(&rows).all(|(a, b)| a.key == b.key)
    {
        for (index, row) in rows.into_iter().enumerate() {
            state.rows.set_row_data(index, row);
        }
    } else {
        state.rows.set_vec(rows);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_unfinished_preferences_still_show_guide_but_background_and_recovery_do_not() {
        let path = std::env::temp_dir().join(format!(
            "glint-guide-{}-missing/config.toml",
            std::process::id()
        ));
        let mut session = crate::preferences::SettingsSession::open(&path);
        assert!(initial(&session, false, false));
        session.is_new = false;
        assert!(initial(&session, false, false));
        assert!(!initial(&session, true, false));
        assert!(!initial(&session, false, true));
        session.preferences.draft.general.onboarding_complete = true;
        session.preferences.accept_saved().unwrap();
        assert!(!initial(&session, false, false));
        session.preferences.draft.general.onboarding_complete = false;
        session.preferences.accept_saved().unwrap();
        session.notice = "config.recovered_defaults";
        assert!(!initial(&session, false, false));
    }
}
