//! Native form schema. Stable config paths identify controls; translations never identify state.
use super::SettingRow;
use glint_core::config::Config;
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

pub const CATEGORIES: [(&str, &str); 13] = [
    ("常规", "General"),
    ("快捷键", "Shortcuts"),
    ("截图", "Capture"),
    ("截图工具条", "Capture toolbar"),
    ("标注与打码", "Annotations & redaction"),
    ("输出与保存", "Output & saving"),
    ("桌面贴图", "Desktop pins"),
    ("历史与隐私", "History & privacy"),
    ("外观与配色", "Appearance"),
    ("资源与兼容", "Resources & compatibility"),
    ("平台与权限", "Platform & permissions"),
    ("后续能力", "Planned capabilities"),
    ("关于", "About"),
];
type Choice = (&'static str, &'static str, &'static str);
#[derive(Clone, Copy)]
pub enum Kind {
    Boolean,
    Choice(&'static [Choice]),
    Text,
    Number(u16, u16),
    Hotkey,
    Info,
}
pub struct Field {
    pub key: &'static str,
    pub zh: &'static str,
    pub en: &'static str,
    pub kind: Kind,
}
const LANGUAGE: &[Choice] = &[
    ("system", "自动 / 跟随系统", "Automatic / Follow system"),
    ("zh_cn", "简体中文", "简体中文"),
    ("en", "English", "English"),
];
const UI_FONT: &[Choice] = &[
    ("system", "跟随系统（默认）", "Follow system (default)"),
    ("noto_sans_sc", "内置 Noto Sans SC", "Bundled Noto Sans SC"),
];
const LOG_LEVEL: &[Choice] = &[
    ("error", "Error · 仅错误", "Error · Errors only"),
    ("warn", "Warn · 警告及错误", "Warn · Warnings and errors"),
    ("info", "Info · 常规（默认）", "Info · Normal (default)"),
    ("debug", "Debug · 详细阶段", "Debug · Detailed stages"),
    (
        "trace",
        "Trace · 输入状态诊断",
        "Trace · Input state diagnostics",
    ),
];
const THEME: &[Choice] = &[
    ("light", "雾白 / 浅色", "Mist / Light"),
    ("dark", "石墨 / 深色", "Graphite / Dark"),
    ("system", "跟随系统", "Follow system"),
];
const ACCENT: &[Choice] = &[
    ("#4b5cfa", "靛蓝", "Indigo"),
    ("#168975", "松绿", "Pine"),
    ("#2869d8", "湖蓝", "Lake"),
    ("#9a5688", "紫灰", "Mauve"),
];
const MODE: &[Choice] = &[
    ("icons_and_labels", "图标＋文字", "Icons + labels"),
    ("icons", "仅图标", "Icons only"),
];
const POSITION: &[Choice] = &[
    ("follow", "跟随选区", "Follow selection"),
    ("top", "固定顶部", "Fixed top"),
    ("bottom", "固定底部", "Fixed bottom"),
];
const TOOL: &[Choice] = &[
    ("select", "选择 / 移动选区", "Select / move region"),
    ("rect", "矩形", "Rectangle"),
    ("arrow", "箭头", "Arrow"),
    ("pen", "画笔", "Pen"),
    ("text", "文字", "Text"),
    (
        "redact",
        "打码（使用优先方式）",
        "Redaction (preferred method)",
    ),
];
const FINISH: &[Choice] = &[
    ("copy", "复制并退出浮层", "Copy and close overlay"),
    ("copy_and_pin", "复制并贴到桌面", "Copy and pin"),
    (
        "save_as",
        "保存到文件（每次询问）",
        "Save to file (ask each time)",
    ),
];
const FORMAT: &[Choice] = &[("png", "PNG", "PNG"), ("jpeg", "JPEG", "JPEG")];
const FILE_CONFLICT: &[Choice] = &[
    (
        "reject",
        "保留已有文件并报错",
        "Keep existing file and report error",
    ),
    ("number", "自动编号", "Add a number"),
    ("confirm", "每次确认覆盖", "Confirm each replacement"),
];
const FEEDBACK: &[Choice] = &[
    ("toast", "轻提示", "Brief feedback"),
    ("none", "不提示", "None"),
    ("notification", "系统通知", "System notification"),
];
const SCALE: &[Choice] = &[
    ("system", "跟随系统", "Follow system"),
    ("percent100", "100%", "100%"),
    ("percent125", "125%", "125%"),
    ("percent150", "150%", "150%"),
    ("percent175", "175%", "175%"),
];
const ARROW: &[Choice] = &[
    ("filled", "实心箭头", "Filled arrow"),
    ("open", "开放箭头", "Open arrow"),
];
const REDACT: &[Choice] = &[
    ("solid", "实心遮盖", "Solid cover"),
    ("pixelate", "像素化（视觉处理）", "Pixelation (visual only)"),
];
const WHEEL: &[Choice] = &[
    (
        "zoom",
        "滚轮缩放 · Ctrl 调透明度",
        "Zoom · Ctrl changes opacity",
    ),
    (
        "opacity",
        "滚轮调透明度 · Ctrl 缩放",
        "Opacity · Ctrl zooms",
    ),
];
const SOURCE: &[Choice] = &[
    ("auto", "自动（平台能力优先）", "Automatic"),
    ("gdi", "GDI 兼容截图", "GDI capture"),
];
macro_rules! fields { ($(($key:literal, $zh:literal, $en:literal, $kind:expr)),* $(,)?) => { &[$(Field { key:$key, zh:$zh, en:$en, kind:$kind }),*] }; }
use Kind::*;
pub const FIELDS: [&[Field]; 10] = [
    fields![
        (
            "general.start_on_login",
            "登录后启动拾光",
            "Start Glint on login",
            Boolean
        ),
        (
            "general.start_in_tray",
            "启动后直接留在托盘",
            "Start in the tray",
            Boolean
        ),
        (
            "general.close_to_tray",
            "关闭设置后留在托盘",
            "Keep running when settings close",
            Boolean
        ),
        (
            "general.single_instance",
            "只允许一个实例",
            "Single instance",
            Boolean
        ),
        (
            "general.language",
            "界面语言",
            "Interface language",
            Choice(LANGUAGE)
        ),
        (
            "general.check_updates",
            "自动检查更新",
            "Check for updates automatically",
            Boolean
        )
    ],
    fields![
        (
            "shortcuts.capture",
            "区域截图 / 呼起浮层",
            "Capture region",
            Hotkey
        ),
        ("shortcuts.window", "直接检测窗口", "Capture window", Hotkey),
        (
            "shortcuts.last_region",
            "重复上次区域",
            "Repeat last region",
            Hotkey
        ),
        (
            "shortcuts.pin_clipboard",
            "贴出剪贴板图像",
            "Pin clipboard image",
            Hotkey
        ),
        (
            "shortcuts.toggle_pins",
            "隐藏 / 恢复全部贴图",
            "Hide / show all pins",
            Hotkey
        ),
        (
            "shortcuts.rescue",
            "救援：取消穿透并显示贴图",
            "Rescue pins",
            Hotkey
        ),
        ("shortcuts.color_picker", "取色", "Color picker", Hotkey),
        ("shortcuts.settings", "打开设置", "Open settings", Hotkey),
        (
            "shortcuts.scroll_capture",
            "长截图（待验收）",
            "Scrolling capture (pending acceptance)",
            Hotkey
        ),
        (
            "shortcuts.recording",
            "打开录屏",
            "Open screen recorder",
            Hotkey
        ),
        (
            "shortcuts.recording_pause",
            "暂停 / 继续录制",
            "Pause / resume recording",
            Hotkey
        ),
        (
            "shortcuts.recording_stop",
            "停止录制并保存 / 取消倒计时",
            "Stop and save / cancel countdown",
            Hotkey
        )
    ],
    fields![
        (
            "capture.magnifier",
            "显示像素放大镜",
            "Pixel magnifier",
            Boolean
        ),
        (
            "capture.dimensions",
            "显示选区尺寸和坐标",
            "Selection size and coordinates",
            Boolean
        ),
        (
            "capture.detect_window",
            "自动检测窗口边界",
            "Detect window boundaries",
            Boolean
        ),
        (
            "capture.detect_elements",
            "检测窗口内的子元素",
            "Detect window elements",
            Boolean
        ),
        (
            "capture.remember_region",
            "记住上次区域",
            "Remember last region",
            Boolean
        ),
        (
            "capture.include_cursor",
            "包含鼠标指针",
            "Include mouse pointer",
            Boolean
        ),
        (
            "capture.delay_seconds",
            "截图前延迟（秒）",
            "Capture delay (seconds)",
            Number(0, 10)
        ),
        (
            "capture.dim_percent",
            "选区外遮罩强度（%）",
            "Outside dimming (%)",
            Number(20, 80)
        )
    ],
    fields![
        ("toolbar.mode", "显示模式", "Display mode", Choice(MODE)),
        (
            "toolbar.position",
            "工具条定位",
            "Toolbar position",
            Choice(POSITION)
        ),
        (
            "toolbar.groups",
            "显示分组线",
            "Show group separators",
            Boolean
        ),
        ("toolbar.order", "工具顺序", "Tool order", Info),
        ("toolbar.hidden", "隐藏至更多", "Move to More", Info)
    ],
    fields![
        ("annotation.tool", "默认工具", "Default tool", Choice(TOOL)),
        (
            "annotation.stroke_width",
            "默认线宽（px）",
            "Stroke width (px)",
            Number(1, 12)
        ),
        ("annotation.color", "默认标注颜色", "Annotation color", Text),
        (
            "annotation.text_size",
            "文字字号（px）",
            "Text size (px)",
            Number(12, 48)
        ),
        ("annotation.text_font", "文字字体", "Text font", Text),
        (
            "annotation.arrow_style",
            "箭头样式",
            "Arrow style",
            Choice(ARROW)
        ),
        (
            "annotation.redact_style",
            "优先打码方式",
            "Preferred redaction",
            Choice(REDACT)
        ),
        (
            "annotation.keep_objects",
            "完成前保持对象可编辑",
            "Keep objects editable",
            Boolean
        )
    ],
    fields![
        (
            "output.finish",
            "完成后的默认动作",
            "Default completion action",
            Choice(FINISH)
        ),
        (
            "output.auto_save",
            "完成时额外保存文件副本",
            "Save an additional copy",
            Boolean
        ),
        (
            "output.feedback",
            "完成反馈",
            "Completion feedback",
            Choice(FEEDBACK)
        ),
        ("output.format", "图片格式", "Image format", Choice(FORMAT)),
        (
            "output.quality",
            "有损格式质量（%）",
            "Lossy quality (%)",
            Number(60, 100)
        ),
        ("output.directory", "保存目录", "Save directory", Text),
        ("output.filename", "文件名模板", "Filename template", Text),
        (
            "output.file_conflict",
            "手动保存重名时",
            "When a manual save name exists",
            Choice(FILE_CONFLICT)
        ),
        (
            "output.strip_metadata",
            "不保留非必要元数据",
            "Strip unnecessary metadata",
            Boolean
        )
    ],
    fields![
        (
            "pins.on_top",
            "默认始终置顶",
            "Always on top by default",
            Boolean
        ),
        (
            "pins.opacity",
            "初始不透明度（%）",
            "Initial opacity (%)",
            Number(20, 100)
        ),
        (
            "pins.zoom_percent",
            "初始显示比例（%）",
            "Initial zoom (%)",
            Number(25, 200)
        ),
        ("pins.wheel", "滚轮动作", "Mouse wheel", Choice(WHEEL)),
        (
            "pins.snap",
            "靠近屏幕或窗口边缘时吸附",
            "Snap to screen or window edges",
            Boolean
        ),
        (
            "pins.hide_on_capture",
            "截图时临时隐藏贴图",
            "Hide pins during capture",
            Boolean
        ),
        (
            "pins.passthrough",
            "新贴图默认鼠标穿透",
            "Click through new pins",
            Boolean
        ),
        (
            "pins.restore_session",
            "重启后恢复贴图",
            "Restore pins after restart",
            Boolean
        )
    ],
    fields![
        (
            "privacy.history",
            "保留本次会话截图历史",
            "Keep session history",
            Boolean
        ),
        (
            "privacy.history_disk",
            "允许历史写入磁盘",
            "Store history on disk",
            Boolean
        ),
        (
            "privacy.keep_days",
            "磁盘历史保留天数",
            "History retention (days)",
            Number(1, 30)
        ),
        (
            "privacy.history_budget_mib",
            "历史源图内存预算（MiB）",
            "History memory (MiB)",
            Number(32, 512)
        ),
        (
            "privacy.clear_on_exit",
            "退出后清空临时截图缓存",
            "Clear temporary captures on exit",
            Boolean
        ),
        (
            "privacy.excluded_apps",
            "排除程序列表",
            "Excluded applications",
            Text
        ),
        (
            "privacy.diagnostics",
            "允许保存诊断日志",
            "Save diagnostic logs",
            Boolean
        ),
        (
            "privacy.log_level",
            "日志等级",
            "Log level",
            Choice(LOG_LEVEL)
        ),
        (
            "privacy.log_file_size_kib",
            "单文件大小上限（KiB）",
            "File size limit (KiB)",
            Number(64, 16384)
        ),
        (
            "privacy.log_backups",
            "轮转备份份数（不含当前）",
            "Rotated backups (excludes current)",
            Number(0, 20)
        )
    ],
    fields![
        ("appearance.theme", "主题", "Theme", Choice(THEME)),
        (
            "appearance.ui_font",
            "应用字体",
            "Application font",
            Choice(UI_FONT)
        ),
        (
            "appearance.accent",
            "强调色",
            "Accent color",
            Choice(ACCENT)
        ),
        (
            "appearance.scale",
            "界面缩放",
            "Interface scale",
            Choice(SCALE)
        ),
        (
            "appearance.reduce_motion",
            "减少动态效果",
            "Reduce motion",
            Boolean
        ),
        (
            "appearance.high_contrast",
            "提高边界对比度",
            "Increase border contrast",
            Boolean
        )
    ],
    fields![
        (
            "resources.capture_source",
            "Windows 捕获源",
            "Windows capture source",
            Choice(SOURCE)
        ),
        (
            "resources.idle_pause",
            "静止时不持续请求刷新",
            "Pause redraws when idle",
            Boolean
        ),
        (
            "resources.sleep_release",
            "锁屏 / 休眠时释放捕获资源",
            "Release capture resources on sleep",
            Boolean
        ),
        (
            "resources.preview_budget_mib",
            "编辑预览内存预算（MiB）",
            "Preview memory (MiB)",
            Number(64, 512)
        ),
        (
            "resources.pin_budget_mib",
            "所有贴图总预算（MiB）",
            "Pin memory budget (MiB)",
            Number(64, 512)
        )
    ],
];

pub fn editable(key: &str) -> bool {
    matches!(
        key,
        "general.language"
            | "general.start_in_tray"
            | "general.start_on_login"
            | "shortcuts.settings"
            | "shortcuts.capture"
            | "shortcuts.window"
            | "shortcuts.last_region"
            | "shortcuts.pin_clipboard"
            | "shortcuts.toggle_pins"
            | "shortcuts.rescue"
            | "shortcuts.color_picker"
            | "capture.detect_window"
            | "capture.remember_region"
            | "capture.delay_seconds"
            | "capture.include_cursor"
            | "capture.magnifier"
            | "capture.dimensions"
            | "capture.dim_percent"
            | "toolbar.mode"
            | "toolbar.position"
            | "toolbar.groups"
            | "annotation.arrow_style"
            | "annotation.tool"
            | "annotation.stroke_width"
            | "annotation.color"
            | "annotation.redact_style"
            | "annotation.text_font"
            | "annotation.text_size"
            | "output.finish"
            | "output.feedback"
            | "output.auto_save"
            | "output.format"
            | "output.quality"
            | "output.directory"
            | "output.filename"
            | "output.file_conflict"
            | "pins.on_top"
            | "pins.opacity"
            | "pins.zoom_percent"
            | "pins.wheel"
            | "pins.snap"
            | "pins.hide_on_capture"
            | "pins.passthrough"
            | "pins.restore_session"
            | "resources.pin_budget_mib"
            | "resources.preview_budget_mib"
            | "resources.sleep_release"
            | "privacy.diagnostics"
            | "privacy.log_level"
            | "privacy.log_file_size_kib"
            | "privacy.log_backups"
            | "privacy.excluded_apps"
            | "appearance.ui_font"
            | "appearance.theme"
            | "appearance.scale"
            | "appearance.reduce_motion"
            | "appearance.accent"
            | "appearance.high_contrast"
    )
}

/// These behaviors are implemented invariants of the first version. Reserved
/// configuration values remain portable, but do not override runtime policy.
fn fixed_behavior(key: &str, english: bool) -> Option<&'static str> {
    Some(match key {
        "general.close_to_tray" => label(
            english,
            "关闭设置收起到托盘；退出请使用托盘菜单。",
            "Closing settings returns to the tray. Use the tray menu to quit.",
        ),
        "general.single_instance" => label(
            english,
            "重复启动激活已有实例。",
            "Starting again activates the existing instance.",
        ),
        "annotation.keep_objects" => label(
            english,
            "完成前保留独立标注对象，可选择、调整与撤销。",
            "Keep annotation objects selectable, editable and undoable until completion.",
        ),
        "output.strip_metadata" => label(
            english,
            "导出仅含合成像素，不复制原文件元数据或编辑对象。",
            "Export composited pixels without original file metadata or edit objects.",
        ),
        "resources.idle_pause" => label(
            english,
            "只在输入、图像或窗口状态变化时更新。",
            "Update on input, image or window state changes.",
        ),
        _ => return None,
    })
}
pub fn label(english: bool, zh: &'static str, en: &'static str) -> &'static str {
    if english { en } else { zh }
}

// Section boundaries follow prototype/app.js. Keep them separate from TOML
// keys and values, so grouping never becomes persisted configuration state.
fn section_heading(key: &str, english: bool) -> &'static str {
    let (zh, en) = match key {
        "general.start_on_login" => ("启动与窗口", "Startup & windows"),
        "general.language" => ("语言与维护", "Language & maintenance"),
        "capture.magnifier" => ("选区与定位", "Selection & positioning"),
        "capture.include_cursor" => ("捕获选项", "Capture options"),
        "annotation.tool" => ("工具默认值", "Tool defaults"),
        "annotation.redact_style" => ("遮盖与编辑", "Redaction & editing"),
        "output.finish" => ("完成动作", "Completion"),
        "output.format" => ("文件", "Files"),
        "pins.on_top" => ("默认行为", "Default behavior"),
        "pins.hide_on_capture" => ("可见性与恢复", "Visibility & recovery"),
        "privacy.history" => ("保存策略", "Retention policy"),
        "privacy.excluded_apps" => ("边界与诊断", "Boundaries & diagnostics"),
        "privacy.diagnostics" => ("日志", "Logging"),
        "resources.capture_source" => ("渲染与捕获", "Rendering & capture"),
        "resources.preview_budget_mib" => ("内存预算", "Memory budgets"),
        _ => return "",
    };
    label(english, zh, en)
}

pub fn build(
    config: &Config,
    category: usize,
    english: bool,
    writable: bool,
) -> Result<Vec<SettingRow>, String> {
    let serialized = toml::Value::try_from(config).map_err(|_| "config.serialize")?;
    let mut rows = Vec::new();
    if let Some(fields) = FIELDS.get(category) {
        for field in *fields {
            let (section, key) = field.key.split_once('.').ok_or("config.field")?;
            let value = serialized
                .get(section)
                .and_then(|section| section.get(key))
                .ok_or("config.field")?;
            let mut row = SettingRow {
                key: field.key.into(),
                section: section_heading(field.key, english).into(),
                title: label(english, field.zh, field.en).into(),
                editable: writable && editable(field.key),
                ..Default::default()
            };
            row.description = if editable(field.key) {
                ""
            } else {
                label(english, "暂不可用", "Unavailable")
            }
            .into();
            row.description = match field.key {
                "shortcuts.scroll_capture" => label(english, "已重构：滚动期间自动拼接并实时预览，结束后进入标注输出。正在真实环境验收，如遇问题请反馈。默认未绑定。", "Rewritten: frames stitch with a live preview while scrolling; finishing opens the annotation output. Real-environment acceptance is in progress; please report issues. Unbound by default."),
                "shortcuts.recording" => label(english, "默认未绑定。打开录屏配置；选择目标并确认后才倒计时录制。", "Unbound by default. Opens recorder setup; select a source and confirm before the countdown starts."),
                "shortcuts.recording_pause" => label(english, "默认未绑定，仅在正在录制或已暂停时生效。", "Unbound by default. Only applies to an active or paused recording."),
                "shortcuts.recording_stop" => label(english, "默认未绑定。停止当前录制并保存；倒计时中按下则取消，未录制时不执行动作。", "Unbound by default. Stop and save the current recording, or cancel its countdown. Does nothing when idle."),
                "resources.capture_source" => label(english, "当前使用 GDI 截图，暂不支持切换捕获源。", "Captures with GDI. Switching capture sources is unavailable."),
                "privacy.excluded_apps" => label(english, "Windows：完整 .exe 文件名，以英文分号、逗号或换行分隔，最多 128 项；不支持路径或通配符。保存后在桌面采集前后检查可见窗口（含被遮挡窗口），命中或检查失败则拒绝整次截图。最小化或关闭目标窗口后重试。两次检查不能覆盖间隙出现的内容，不是持续隐私保护；不影响本地图片或贴图再编辑。", "Windows: full .exe names separated by semicolons, commas or newlines; up to 128 entries, no paths or wildcards. After Save, visible windows (including covered windows) are checked before and after desktop capture. A match or failed check rejects the whole capture; minimize or close the target and retry. Two checks cannot cover content appearing between them; this is not continuous privacy protection. Local images and pin editing are unaffected."),
                "privacy.diagnostics" => label(english, "默认关闭，保存后生效。仅记录时间及固定事件代码；容量、位置、失败提示和清理入口见下方。", "Off by default; Save applies. Records only timestamps and fixed event codes. Limits, location, failures and clearing are shown below."),
                "privacy.log_level" => label(english, "所选等级包含更严重事件。Debug 增加阶段细节，Trace 增加只读输入状态；不记录具体按键或图像。保存后生效。", "Includes more severe events. Debug adds stage details; Trace adds read-only input state. No actual keys or images. Save applies."),
                "privacy.log_file_size_kib" => label(english, "64–16384 KiB，默认 512 KiB。保存后用于后续写入和轮转，不截断已有日志。", "64–16384 KiB; default 512 KiB. Save applies to subsequent writes and rotation; existing logs are not truncated."),
                "privacy.log_backups" => label(english, "0–20 份，不含当前 events.log。默认 1 份备份，共 2 份；设为 0 仅保留当前日志。日志开启时保存降低份数会清理多余备份；关闭则在下次启用写入时清理。", "0–20, excluding events.log. Default 1 backup, 2 files total; 0 keeps only the current log. When logging is active, saving a lower count removes excess backups; otherwise cleanup occurs on the next write after enabling."),
                "resources.sleep_release" => label(english, "锁屏、断开会话或休眠时取消倒计时及未完成截图；关闭时隐藏并保留已有编辑。进行中的导出仍可能完成。", "Cancel countdowns and unfinished captures on lock, disconnect or sleep. When off, hide and retain existing edits. An export in progress may still finish."),
                "resources.preview_budget_mib" => label(english, "保存后用于新截图及图片再编辑的标注像素分块；超限保留编辑，可撤销或重新框选。不含源图、导出与渲染器内存。", "Saved limit applies to annotation pixel tiles in new captures and image editors. Over budget keeps edits; undo or reselect to recover. Excludes source images, export and renderer memory."),
                "appearance.reduce_motion" => label(english, "控件状态立即切换；不改变选区拖动、绘制或倒计时。保存后用于新浮层与贴图。", "Switch control states immediately; selection, drawing and countdown timing stay unchanged. Save applies to new overlays and pins."),
                "appearance.scale" => label(english, "先预览缩放，再选择保留或还原；15 秒未确认自动恢复原比例。保留后仍需保存设置。只缩放界面，不改变截图像素。", "Preview the scale, then keep or revert it. Without confirmation, the previous scale returns after 15 seconds. Save settings to persist it. Image pixels are unchanged."),
                "appearance.ui_font" => label(english, "仅影响应用界面，不影响批注文字。跟随系统时，缺失字体回退到内置 Noto Sans SC。此处即时预览；保存后用于新窗口，取消恢复。", "Applies only to the app UI, not annotations. Follow system falls back to bundled Noto Sans SC when needed. Preview here; Save applies to new windows, Cancel restores the saved font."),
                "appearance.theme" => label(english, "设置预览草稿；保存用于新窗口。选择跟随系统的浮层和贴图响应系统深浅色变化，不改变图像或位置。", "Preview the draft here; Save applies to new windows. Overlays and pins created with System follow light/dark changes without altering images or positions."),
                "annotation.tool" => label(english, "保存后用于下一次截图或图片编辑；仍先框选，再使用默认工具。", "Saved defaults apply to the next capture or image editor. Select a region before using the default tool."),
                "annotation.color" => label(english, "使用 #RRGGBB；用于矩形、箭头、画笔、文字和高亮。实心遮盖保持不透明深色。", "Use #RRGGBB for rectangles, arrows, pen, text and highlights. Solid cover stays opaque dark."),
                "annotation.redact_style" => label(english, "默认工具为打码时使用此方式；工具条两种工具仍分别可选。敏感内容优先实心遮盖。", "Used when the default tool is redaction. Both toolbar tools remain available. Use solid cover for sensitive content."),
                "output.finish" => label(english, "Enter / 双击使用已保存动作；贴图成功后才离开浮层。", "Enter / double click uses the saved action; pin creation must succeed before closing."),
                "output.feedback" => label(english, "仅控制成功提示；失败保留任务。Windows 通知提交失败时回退轻提示；最终展示由系统控制。", "Controls success feedback; failures keep the task. Failed Windows notification submission falls back to brief feedback; Windows controls delivery."),
                "output.auto_save" => label(english, "先保存副本；重名自动编号。复制失败保留编辑，未修改重试复用副本。", "Save a numbered copy first. Failed copy keeps edits; unchanged retry reuses the file."),
                "output.quality" => label(english, "仅 JPEG 使用；PNG 始终无损。JPEG 透明区域合成到白底。", "JPEG only; PNG is lossless. JPEG transparency is flattened onto white."),
                "output.directory" => label(english, "留空使用系统图片目录下的 Glint；另存为时仍可更改。", "Empty uses Glint in the system Pictures folder; Save As can change it."),
                "output.filename" => label(english, "支持 {date}、{time}、{width}、{height}；扩展名自动添加。", "Supports {date}, {time}, {width}, {height}; extension is added."),
                "output.file_conflict" => label(english, "自动副本始终编号；取消或失败保留当前编辑。覆盖需每次确认，文件已变化时请重新选择。", "Additional copies always use numbers. Cancel or failure keeps edits. Each replacement needs confirmation; reselect if the file changed."),
                _ => row.description.as_str(),
            }.into();
            if field.key == "privacy.excluded_apps"
                && glint_core::exclusions::parse(&config.privacy.excluded_apps).is_err()
            {
                row.description = format!(
                    "{} {}",
                    row.description,
                    crate::messages::text(crate::messages::Message::ExclusionInvalid, english)
                )
                .into();
            }
            if let Some(description) = fixed_behavior(field.key, english) {
                row.description = if value.as_bool() == Some(false) {
                    format!(
                        "{description} {}",
                        label(english, "配置中的关闭值当前不生效；保留该值，不会自动改写。", "The configured off value has no effect in this version. It is preserved without rewriting the configuration.")
                    ).into()
                } else {
                    description.into()
                };
            }
            row.text = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string())
                .into();
            match field.kind {
                Boolean => {
                    row.kind = 0;
                    row.checked = value.as_bool().ok_or("config.field_type")?
                        || fixed_behavior(field.key, english).is_some();
                }
                Choice(options) => {
                    row.kind = 1;
                    row.selected = options
                        .iter()
                        .position(|option| Some(option.0) == value.as_str())
                        .map(|index| index as i32)
                        .unwrap_or(-1);
                    let strings: Vec<_> = options
                        .iter()
                        .map(|option| SharedString::from(label(english, option.1, option.2)))
                        .collect();
                    row.options = ModelRc::from(Rc::new(VecModel::from(strings)));
                    if field.key == "appearance.accent" {
                        row.kind = 6;
                        let colors = options
                            .iter()
                            .map(|option| {
                                let rgb = u32::from_str_radix(&option.0[1..], 16)
                                    .expect("constant accent colors");
                                slint::Color::from_rgb_u8(
                                    (rgb >> 16) as u8,
                                    (rgb >> 8) as u8,
                                    rgb as u8,
                                )
                            })
                            .collect::<Vec<_>>();
                        row.colors = ModelRc::from(Rc::new(VecModel::from(colors)));
                    }
                }
                Text => row.kind = 2,
                Number(min, max) => {
                    row.kind = if matches!(
                        field.key,
                        "privacy.log_file_size_kib" | "privacy.log_backups"
                    ) {
                        7
                    } else {
                        3
                    };
                    row.number = value.as_integer().ok_or("config.field_type")? as f32;
                    row.minimum = min as f32;
                    row.maximum = max as f32;
                }
                Hotkey => row.kind = 4,
                Info => {
                    row.kind = 5;
                    let tools = [
                        ("select", "选区", "Select"),
                        ("rect", "矩形", "Rectangle"),
                        ("arrow", "箭头", "Arrow"),
                        ("pen", "画笔", "Pen"),
                        ("text", "文字", "Text"),
                        ("highlight", "高亮", "Highlight"),
                        ("redact", "遮盖", "Cover"),
                        ("mosaic", "像素化", "Pixelate"),
                        ("picker", "取色", "Picker"),
                    ];
                    if let Some(values) = value.as_array() {
                        let text = values
                            .iter()
                            .filter_map(|value| {
                                tools
                                    .iter()
                                    .find(|tool| Some(tool.0) == value.as_str())
                                    .map(|tool| label(english, tool.1, tool.2))
                            })
                            .collect::<Vec<_>>()
                            .join(" · ");
                        row.text = if text.is_empty() {
                            label(english, "无", "None").into()
                        } else {
                            text.into()
                        };
                    }
                }
            }
            rows.push(row);
        }
    } else {
        let sections = match category {
            10 => vec![(
                "Windows",
                "Windows",
                "普通桌面截图无需管理员权限。受保护内容和安全桌面可能无法捕获。",
                "Desktop capture needs no administrator access. Protected content and secure desktops may not be captured.",
            )],
            11 => vec![
                (
                    "滚动长截图（待真实环境验收）",
                    "Scrolling capture (pending acceptance)",
                    "已按飞书/微信式交互重构：可见选区边框、右侧实时拼接预览、滚动期间连续采集与稳定收尾、结束进入标注输出。等待真实环境验收，历史问题需按反馈继续确认。",
                    "Rewritten with the Feishu/WeChat-style flow: a visible selection ring, a live stitched preview, continuous capture with stability-based burst ends while scrolling, and annotation output after finishing. Awaiting real-environment acceptance; historical issues still need confirmation from feedback.",
                ),
                (
                    "本地 OCR",
                    "Local OCR",
                    "暂不可用。从图片中识别文字。",
                    "Unavailable. Recognize text in images.",
                ),
                (
                    "录屏",
                    "Screen recording",
                    "Windows：从托盘或此页打开录屏，选择屏幕或窗口；区域录屏从截图选区的更多工具进入。MP4 支持最高 144 FPS 与可选音频；GIF 限制为 30 FPS、30 秒、最长边 1280 像素且无音频。",
                    "Windows: open the recorder here or from the tray and choose a display or window. For a region, use More tools in capture. MP4 supports up to 144 FPS and optional audio; GIF is limited to 30 FPS, 30 seconds, 1280 px maximum edge and no audio.",
                ),
            ],
            _ => vec![
                (
                    "拾光 · Glint 0.0.0",
                    "Glint 0.0.0",
                    "截图、标注与桌面贴图。",
                    "Screen capture, annotation and desktop pins.",
                ),
                (
                    "许可与第三方",
                    "License & third parties",
                    "Glint 与 Slint 采用 GPL-3.0-only 路径。tiny-skia: BSD-3-Clause；serde、toml、image、arboard、windows、tray-icon: MIT / Apache-2.0。内置界面回退字体 Noto Sans SC: SIL OFL 1.1。完整声明见随源码的 THIRD-PARTY-NOTICES.md。",
                    "Glint and Slint use GPL-3.0-only. tiny-skia: BSD-3-Clause; serde, toml, image, arboard, windows and tray-icon: MIT / Apache-2.0. Bundled UI fallback Noto Sans SC: SIL OFL 1.1. See THIRD-PARTY-NOTICES.md supplied with source.",
                ),
            ],
        };
        for (zh, en, body_zh, body_en) in sections {
            rows.push(SettingRow {
                title: label(english, zh, en).into(),
                text: label(english, body_zh, body_en).into(),
                kind: 5,
                ..Default::default()
            });
        }
    }
    Ok(rows)
}

pub fn change(config: &Config, key: &str, input: &str, english: bool) -> Result<Config, String> {
    let field = FIELDS
        .iter()
        .flat_map(|fields| *fields)
        .find(|field| field.key == key)
        .ok_or("config.field")?;
    if !editable(key) {
        return Err("config.capability_not_connected".into());
    }
    let value = match field.kind {
        Number(min, max) => {
            let number = input.parse::<u16>().map_err(|_| "config.field_type")?;
            if !(min..=max).contains(&number) {
                return Err("config.field_range".into());
            }
            toml::Value::Integer(i64::from(number))
        }
        Boolean => toml::Value::Boolean(input.parse().map_err(|_| "config.field_type")?),
        Choice(options) => toml::Value::String(
            options
                .iter()
                .find(|option| input == option.0 || input == label(english, option.1, option.2))
                .ok_or("config.choice")?
                .0
                .into(),
        ),
        Text | Hotkey => toml::Value::String(input.into()),
        _ => return Err("config.capability_not_connected".into()),
    };
    let mut serialized = toml::Value::try_from(config).map_err(|_| "config.serialize")?;
    let (section, field) = key.split_once('.').ok_or("config.field")?;
    *serialized
        .get_mut(section)
        .and_then(|table| table.get_mut(field))
        .ok_or("config.field")? = value;
    serialized
        .try_into()
        .map_err(|_| "config.field_type".into())
}
