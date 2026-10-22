use super::Message;

pub fn text(id: Message) -> Option<&'static str> {
    match id {
        Message::RecordingPlainRegion => {
            Some("请先框选至少 32 × 64 像素的实时桌面区域，且不要添加批注；本地图片不能用于录屏。")
        }
        Message::ScrollTitle => Some("滚动长截图"),
        Message::ScrollHint => Some(
            "手动向下滚动，等待内容静止，再按截图快捷键或点击“采集下一段”。至少保留四分之一屏重叠；选区避开固定页头、侧栏和滚动条。不自动滚动。",
        ),
        Message::ScrollCapture => Some("采集下一段"),
        Message::ScrollUndo => Some("撤销上一段"),
        Message::ScrollStop => Some("停止并检查"),
        Message::ScrollEdit => Some("确认并标注"),
        Message::ScrollOverlap => Some("重叠高度(px)"),
        Message::ScrollAccept => Some("确认接缝"),
        Message::ScrollDiscard => Some("丢弃待确认片段"),
        Message::ScrollPreview => Some("预览接缝"),
        Message::ScrollResume => Some("继续采集"),
        Message::ScrollBusy => Some("正在采集 / 拼接…"),
        Message::ScrollReady => Some("片段保留在内存中"),
        Message::ScrollDuplicate => Some("已跳过完全相同的片段。向下滚动后重试。"),
        Message::ScrollUncertain => Some(
            "匹配不可靠，待确认片段已保留。调整重叠高度并预览接缝，可确认、丢弃该片段或结束已有结果。",
        ),
        Message::ScrollFailed => Some("采集 / 拼接失败，已有片段保留。可重试或结束已有结果。"),
        Message::ScrollBudget => {
            Some("已达到源片段/结果 64 MiB 或 128 段上限。可撤销上一段或结束已有结果。")
        }
        Message::ScrollPlainRegion => {
            Some("请从单个屏幕内、尚未标注的桌面选区开始长截图，区域至少 32 × 64 像素。")
        }
        Message::ScrollReviewHint => {
            Some("滚动检查接缝，彩色线仅用于预览。进入标注后用滚轮查看长图；复制/保存保持源像素。")
        }
        Message::CaptureFailureLabel => Some("截图失败"),
        Message::CaptureFocusUnavailable => Some("点击截图浮层后即可框选；Esc 取消。"),
        Message::ExclusionInvalid => Some(
            "排除列表格式无效。请输入完整 .exe 文件名，用英文分号、逗号或换行分隔（最多 128 项），不接受路径或通配符。请修正或清空列表并保存后截图。",
        ),
        Message::ExclusionBlocked => Some(
            "截图已阻止：排除程序存在可见窗口（包括被遮挡窗口）。请最小化或关闭目标窗口后重试，或修改排除列表并保存。",
        ),
        Message::ExclusionUnavailable => Some(
            "截图已阻止：无法完成排除程序检查。请重试，或修改排除列表并保存。此检查不提供持续隐私保护。",
        ),
        Message::CompletionCopied => Some("已复制到剪贴板。"),
        Message::CompletionNotification => Some("操作已完成，图像仅在本机处理。"),
        Message::CompletionSaved => Some("图片已保存。"),
        Message::CompletionPinned => Some("已生成桌面贴图。"),
        Message::CompletionColor => Some("颜色已复制。"),
        Message::CompletionApplied => Some("贴图修改已应用。"),
        Message::CompletionFallback => Some("Windows 系统通知提交失败，已回退为轻提示。"),
        Message::SessionConfigRecovery => Some(
            "配置需要恢复，暂不读取或改写已保存贴图资产。确认并保存修复后的设置，再自动恢复或清理。",
        ),
        Message::SessionFinishFirst => {
            Some("请先完成或取消当前截图/贴图操作，再重试退出；当前编辑保留。")
        }
        Message::SessionHelp => Some(
            "默认关闭，独立于历史。正常托盘退出时保存源图（含被遮盖内容）、当前标注和窗口状态，上限 256 MiB；不保存旧撤销历史。恢复后可编辑、可见且不穿透。关闭并保存设置会删除已保存会话。崩溃或强制结束仅保留上次成功会话。",
        ),
        Message::SessionClear => Some("删除已保存贴图会话…"),
        Message::SessionRestore => Some("重试恢复会话"),
        Message::SessionRetryExit => Some("重试退出"),
        Message::SessionExitWithout => Some("不保存本次贴图退出"),
        Message::SessionCancel => Some("取消"),
        Message::SessionBusy => Some("正在处理贴图会话，请稍候…"),
        Message::SessionIoFailed => Some("无法读写贴图会话，当前贴图保留。请检查目录权限后重试。"),
        Message::SessionRestoreFailed => Some(
            "无法恢复会话：数据无效、版本不支持、超出贴图预算或窗口创建失败。原会话保留；可重试或删除。退出时不会覆盖尚未恢复的会话。",
        ),
        Message::SessionSaveFailed => {
            Some("保存会话失败，当前贴图和上次成功会话保留。请重试，或明确选择不保存本次贴图退出。")
        }
        Message::SessionClearFailed => {
            Some("删除保存的会话失败，可能仍有图片文件留在配置目录。请检查权限后重试清理。")
        }
        Message::SessionEmpty => Some("没有保存的贴图会话。"),
        Message::SessionRestored => Some("贴图会话已恢复；窗口可见且不穿透，旧撤销历史未恢复。"),
        Message::SessionCleared => Some("保存的会话及会话临时文件已删除。当前贴图保持打开。"),
        Message::SessionClearHint => Some(
            "仅删除配置目录内的已保存贴图会话及未完成的会话临时文件，不关闭当前贴图，不删除主动导出的图片。恢复仍开启时，下次正常退出会重新保存当前贴图。",
        ),
        Message::SessionExitHint => {
            Some("退出而不保存当前贴图，保留上次成功保存的会话。尚未保存的新贴图和修改将丢失。")
        }
        Message::Workbench => Some("辅助工作台"),
        Message::WorkbenchCapture => Some("区域截图"),
        Message::WorkbenchPrivacy => Some("历史与隐私"),
        Message::WorkbenchOutput => Some("输出设置"),
        Message::WorkbenchHint => {
            Some("打开本地图片，在独立浮层中标注。正常截图直接使用托盘或快捷键。")
        }
        Message::WorkbenchNote => Some(
            "支持 PNG/JPEG。文件上限 64 MiB，解码后 RGBA 上限 64 MiB，画布上限 128 MiB。打开图片不会覆盖原文件，可在浮层中复制、另存或贴图。历史浏览尚未实现。",
        ),
        Message::LocalOpen => Some("打开本地图片…"),
        Message::LocalTitle => Some("本地图片 · Enter 使用已保存完成动作 · Esc 取消"),
        Message::LocalHint => {
            Some("本地图片 · 可复制、另存或生成新贴图，保留原文件。Esc 回到桌面。")
        }
        Message::LocalFailed => {
            Some("无法打开图片。请选择可读取且在内存预算内的 PNG/JPEG 后重试。")
        }
        Message::TransferCancel => Some("取消预览"),
        Message::TransferImport => Some("导入配置"),
        Message::TransferExport => Some("导出已保存配置"),
        Message::TransferImportHint => Some(
            "选择不超过 256 KiB 的 TOML 文件，预览后合并到当前草稿。保存设置时再注册快捷键和应用登录启动；失败保留原可用设置。无效/未知字段保留原值，快捷键冲突时保留原整组绑定。引导状态与系统授权不导入。",
        ),
        Message::TransferExportHint => Some(
            "预览已保存设置与快捷键，不包含未保存草稿、引导状态、截图像素、历史资产或系统授权。路径及隐私策略属于配置值，会随文件导出。请选择新文件；不会覆盖已有文件。",
        ),
        Message::TransferSelect => Some("选择文件…"),
        Message::TransferApply => Some("合并到草稿"),
        Message::TransferSkipped => Some("已跳过字段（保留原值）"),
        Message::TransferNoChanges => Some("没有需要更改的设置。"),
        Message::TransferReadFailed => Some("无法读取配置文件，当前草稿保留；请选择其他文件。"),
        Message::TransferInvalid => Some("UTF-8/TOML 无效或缺少有效版本，当前草稿保留。"),
        Message::TransferVersion => Some("不支持此配置版本，当前草稿保留。"),
        Message::TransferTooLarge => Some("配置超过 256 KiB，当前草稿保留。"),
        Message::TransferExportFailed => {
            Some("导出失败，已有文件与应用配置保留；请选择新文件重试。")
        }
        Message::TransferExportDone => Some("配置已导出，应用设置未改变。"),
        Message::TransferDraftReady => {
            Some("导入变更已合并到草稿；请检查并保存设置后应用，取消修改可恢复已保存设置。")
        }
        Message::TransferStale => Some("预览后草稿已改变，请重新选择文件并查看新预览。"),
        Message::TransferBusy => Some("正在处理配置，请稍候；关闭预览不会撤销已开始的文件写入。"),
        Message::ColorPickerEntry => Some("独立取色"),
        Message::ColorPickerHint => {
            Some("冻结桌面取色：单击或 Enter 复制 HEX，成功后离开；Esc 取消。")
        }
        Message::AllScreensCapture => Some("全屏截图（所有屏幕）"),
        Message::CurrentScreenCapture => Some("当前屏幕截图"),
        Message::AllScreensSelected => Some("已选择全屏（所有屏幕），仍可调整和标注。"),
        Message::CurrentScreenSelected => Some("已选择指针所在屏幕，仍可调整和标注。"),
        Message::ScreenSelectionUnavailable => Some("无法从本次采集确定屏幕，请拖动手动框选。"),
        Message::DelayTitle => Some("截图倒计时"),
        Message::DelayHint => {
            Some("倒计时结束后才冻结桌面；窗口截图可在此期间将指针移到目标窗口。")
        }
        Message::DelayCancel => Some("取消截图（Esc）"),
        Message::RepeatOff => Some("区域记忆已关闭，请手动框选。"),
        Message::RepeatEmpty => Some("本次运行尚无成功截图，请手动框选。"),
        Message::RepeatChanged => Some("显示器布局或 DPI 已变化，请重新框选。"),
        Message::RepeatReady => Some("已恢复上次区域；重新采集画面，不带入旧标注。"),
        Message::ToolbarCustomize => Some("自定义"),
        Message::ToolbarTitle => Some("自定义截图工具条"),
        Message::ToolbarMore => Some("更多工具"),
        Message::ToolbarBoth => Some("图标＋文字"),
        Message::ToolbarIcons => Some("仅图标（悬停显示名称）"),
        Message::ToolbarFollow => Some("跟随选区 / 自动避让"),
        Message::ToolbarTop => Some("固定屏幕顶部"),
        Message::ToolbarBottom => Some("固定屏幕底部"),
        Message::ToolbarGroups => Some("显示分组线"),
        Message::ToolbarOrderHint => Some("拖动 ≡ 或用箭头排序；取消勾选的工具仍可从更多使用。"),
        Message::ToolbarPreviewHint => Some("浮层即时预览；应用、取消和恢复默认均保留选区与标注。"),
        Message::ToolbarApply => Some("应用"),
        Message::ToolbarReset => Some("恢复默认"),
        Message::ToolbarPreview => Some("即时预览"),
        Message::ToolbarMode => Some("显示模式"),
        Message::ToolbarPosition => Some("工具条定位"),
        Message::ToolbarLayout => Some("布局"),
        Message::ToolbarGrid => Some("网格"),
        Message::ToolbarScroll => Some("单行滚动"),
        Message::ToolbarSingleRow => Some("单行完整显示"),
        Message::ToolbarFixedWidth => Some("固定行宽"),
        Message::ToolbarColumns => Some("列数"),
        Message::ToolbarRows => Some("行数"),
        Message::ToolbarWidth => Some("最大宽度 (px)"),
        Message::ToolbarWidthHint => Some(
            "输入最大宽度；实际宽度会按完整按钮向下对齐，避免露出半个图标。超出部分可横向滚动查看。",
        ),
        Message::ToolbarSettingsPreviewHint => {
            Some("仅外观预览；保存设置或应用自定义后生效。选区与标注保持不变。")
        }
        Message::CapturePicker => Some("取色"),
        Message::CaptureText => Some("文字"),
        Message::CaptureTextHint => Some("单击输入文字 · Enter 放置，Esc 取消文字"),
        Message::CaptureTextPlace => Some("放置"),
        Message::CaptureTextCancel => Some("取消文字"),
        Message::CaptureTextSize => Some("字号"),
        Message::CaptureTextFont => Some("字体"),
        Message::CaptureTextLimit => Some("单行文字，最多 80 字符"),
        Message::CaptureTextRendering => Some("正在生成文字…"),
        Message::CaptureTextFailed => Some("文字生成失败，已保留输入，可重试"),
        Message::CapturePixelate => Some("像素化"),
        Message::CapturePixelateHint => Some("16 px 方块 · 视觉处理，不保证保密"),
        Message::CaptureMagnifier => Some("冻结原图像素"),
        Message::CapturePickerHint => Some("原图像素（不含标注）· 单击复制 HEX"),
        Message::CaptureCopyHex => Some("复制 HEX"),
        Message::CaptureCopyRgb => Some("复制 RGB"),
        Message::CaptureColorCopying => Some("正在复制颜色…"),
        Message::CaptureColorCopied => Some("颜色已复制"),
        Message::CaptureColorCopyFailed => Some("颜色复制失败，编辑已保留；可重试"),
        Message::CaptureColorOutside => Some("请在选区内悬停，读取原图像素（不含标注）。"),
        Message::CaptureTitle => Some("拾光 · Glint — 截图"),
        Message::CaptureDiagnosticTitle => Some("拾光 · Glint — M0 截图浮层"),
        Message::CaptureSelect => Some("选区"),
        Message::CapturePen => Some("画笔"),
        Message::CaptureRedact => Some("遮盖"),
        Message::CaptureRectangle => Some("矩形"),
        Message::CaptureArrow => Some("箭头"),
        Message::CaptureHighlight => Some("高亮"),
        Message::CaptureRectangleHint => Some("在选区内拖动绘制矩形"),
        Message::CaptureArrowHint => Some("从箭尾拖动到箭头位置"),
        Message::CaptureHighlightHint => Some("拖动高亮重点 · 半透明，不遮蔽内容"),
        Message::CaptureUndo => Some("撤销"),
        Message::CaptureRedo => Some("重做"),
        Message::CaptureSave => Some("保存"),
        Message::CaptureFinish => Some("完成"),
        Message::CaptureCopy => Some("复制"),
        Message::CaptureCancel => Some("取消"),
        Message::CaptureStrokeWidth => Some("线宽"),
        Message::CapturePenHint => Some("拖动画笔 · 圆头笔迹"),
        Message::CaptureObjectHint => Some("拖动移动标注 · 手柄调整形状 / 箭头端点 · Delete 删除"),
        Message::CaptureObjectTextHint => Some("拖动移动文字 · 编辑文字可调整字号 · Delete 删除"),
        Message::CaptureObjectDelete => Some("删除标注"),
        Message::CaptureArrowFilled => Some("实心箭头"),
        Message::CaptureArrowOpen => Some("开放箭头"),
        Message::CaptureTextEdit => Some("编辑文字"),
        Message::CaptureSelectHint => {
            Some("单击标注选中并拖动；空白处移动选区，边缘手柄调整大小；双击使用已保存的完成动作。")
        }
        Message::CaptureRedactHint => Some("拖动覆盖敏感内容，导出时将实心遮盖合成到图像中。"),
        Message::CaptureDragHint => Some("拖动框选 · Esc 取消"),
        Message::CaptureWindowHint => Some("单击选中窗口，或拖动框选 · Esc 取消"),
        Message::CaptureEditHint => Some("原地标注 · Enter 复制 · Esc 取消"),
        Message::CaptureEditHintSave => Some("原地标注 · Enter 保存 · Esc 取消"),
        Message::CapturePinUnavailable => {
            Some("复制并贴图尚未接入；编辑已保留，可改用复制或保存。")
        }
        Message::CaptureWindowUnavailable => Some("窗口检测不可用，可拖动框选"),
        Message::DiagnosticsRenderer => Some("界面渲染：Slint · winit · software（CPU）"),
        Message::DiagnosticsBackend => Some("截图后端：GDI（独立于界面渲染）；WGC / DXGI 尚未实现"),
        Message::DiagnosticsSubscribed => Some("锁屏 / 休眠通知：已订阅"),
        Message::DiagnosticsUnavailable => Some("锁屏 / 休眠通知：不可用"),
        Message::DiagnosticsDisplays => Some("显示器"),
        Message::DiagnosticsRemote => Some("远程会话"),
        Message::DiagnosticsBounds => Some("边界"),
        Message::DiagnosticsWorkArea => Some("工作区"),
        Message::DiagnosticsPrimary => Some(" · 主屏"),
        Message::DiagnosticsDisplayFailed => Some("显示器查询失败"),
        Message::DiagnosticsWorkingSet => Some("工作集"),
        Message::DiagnosticsPrivate => Some("私有提交"),
        Message::DiagnosticsHandles => Some("句柄"),
        Message::DiagnosticsCpu => Some("进程累计 CPU 时间（非占用率）"),
        Message::DiagnosticsResourceFailed => Some("资源查询失败"),
        Message::FontTitle => Some("选择字体"),
        Message::FontSearch => Some("搜索字体"),
        Message::FontCancel => Some("取消"),
        Message::FontRetry => Some("重试"),
        Message::FontFailed => Some("无法读取系统字体，可重试或取消后手动输入"),
        Message::FontLoading => Some("正在读取系统字体…"),
        Message::FontEmpty => Some("没有匹配字体，可修改搜索或取消后手动输入。"),
        Message::FontHint => Some("选择仅更新当前草稿；字体覆盖范围由系统决定。"),
        Message::OutputFolderChoose => Some("选择目录"),
        Message::OutputFolderFailed => Some("无法选择目录，已保留原值；可重试或手动输入。"),
        Message::OutputFilenamePreview => {
            Some("文件名预览（示例：1280×720 px，2026-10-18 14:32:05）")
        }
        Message::OutputFilenameInvalid => Some("模板无效，请修改文件名模板。"),
        Message::OutputReplaceTitle => Some("确认覆盖"),
        Message::OutputReplaceQuestion => Some("是否覆盖此已有文件？"),
        Message::OutputReplaceHint => Some("取消会保留当前图片编辑。"),
        Message::DiagnosticsTitle => Some("运行诊断"),
        Message::DiagnosticsHint => Some(
            "刷新仅查询显示器与进程计数，不采集截图、窗口标题或剪贴板，不保存或上传。快照不证明环境兼容性，也不代表资源峰值或空闲占用。",
        ),
        Message::DiagnosticsRefresh => Some("刷新诊断"),
        Message::DiagnosticsEmpty => Some("尚未查询。刷新后查看当前显示器与进程资源。"),
        Message::DiagnosticsLoading => Some("正在查询诊断，保留上次快照。"),
        Message::DiagnosticsFailed => Some("刷新失败，可重试；已有快照会保留"),
        Message::LifecycleUnavailable => Some("锁屏/休眠通知不可用，自动释放截图尚未启用"),
        Message::LifecycleReleased => {
            Some("锁屏、断开会话或休眠已结束本次截图，请重新截图；进行中的导出仍可能完成")
        }
        Message::CapturePreviewFailed => Some("预览失败，输出已暂停；可撤销恢复"),
        Message::CapturePreviewBudget => {
            Some("标注预览超出预算，编辑已保留、输出已暂停；可撤销或重新框选恢复")
        }
        Message::CaptureMarkRejected => {
            Some("标注为空或已达到对象/笔迹预算，本次标注未添加；已有编辑已保留。")
        }
        Message::CapturePointBudget => Some("本条笔画已达到点数上限并结束；可继续绘制新笔画。"),
        Message::CaptureProbePath => Some("M0 保存需要启动时指定 output.png；当前编辑已保留。"),
        Message::CaptureOutputPending => Some("上一次输出仍在结束；当前编辑已保留，请稍后重试。"),
        Message::CaptureGesturePending => Some("请先松开鼠标完成当前操作，再输出。"),
        Message::CaptureImageFormats => Some("请使用 .png、.jpg 或 .jpeg 扩展名；编辑已保留。"),
        Message::CaptureDialogFailed => Some("无法打开保存窗口，编辑已保留"),
        Message::CaptureOutputBusy => Some("正在合成与输出…失败会保留当前编辑。"),
        Message::CaptureOutputFailed => Some("输出失败，编辑已保留"),
        Message::CaptureSaveCancelled => Some("保存已取消；选区与标注已保留，可继续编辑或重试。"),
        Message::CaptureFilesSaved => Some("已保存文件（未修改重试时复用）"),
        Message::CaptureSaveElsewhere => Some("改为另存为（不写自动副本）"),
        Message::CapturePin => Some("贴图"),
        Message::PinPreparing => Some("正在准备桌面贴图…"),
        Message::PinPass => Some("开启鼠标穿透…"),
        Message::PinPassOff => Some("取消鼠标穿透"),
        Message::PinPassTitle => Some("请记住贴图救援方式"),
        Message::PinPassBody => Some(
            "开启后鼠标会点到贴图后方窗口。使用下方救援快捷键，或托盘 → 恢复贴图并取消穿透，可重新显示贴图并恢复点击。",
        ),
        Message::PinPassEnable => Some("开启"),
        Message::PinPassCancel => Some("取消"),
        Message::PinPassActive => Some("已开启穿透，使用救援快捷键或托盘恢复操作。"),
        Message::PinRecoveryUnavailable => {
            Some("救援快捷键不可用，未开启穿透。请修复快捷键后重试；托盘救援仍可恢复贴图。")
        }
        Message::PinRescued => Some("贴图已找回，鼠标穿透已取消。"),
        Message::PinActionFailed => Some("无法调整贴图，图像已保留"),
        Message::PinCopy => Some("复制图像"),
        Message::PinSave => Some("保存图像…"),
        Message::PinRetry => Some("重试 / 另选位置"),
        Message::PinDismiss => Some("关闭详情"),
        Message::PinOutputFailed => Some("输出失败，贴图已保留；可重新复制或另选位置保存。"),
        Message::PinCopied => Some("已按原始分辨率复制图像。"),
        Message::PinSaved => Some("图像已保存，点击此处查看路径。"),
        Message::PinOutputCancelled => Some("已取消保存，贴图保持打开。"),
        Message::PinClipboardReading => Some("正在读取剪贴板图像…"),
        Message::PinClipboardCancelled => {
            Some("锁屏、断开会话或休眠已取消本次剪贴板贴图；需要时请重新触发。")
        }
        Message::PinClipboardCreated => Some("剪贴板图像已贴到桌面。"),
        Message::PinClipboardFailed => {
            Some("无法贴出剪贴板图像，已有贴图保留。复制图像后，可从托盘或快捷键重试")
        }
        Message::PinHint => Some("拖动标题栏；可从托盘隐藏或恢复贴图。"),
        Message::PinFailed => Some("贴图失败，编辑已保留；此前的复制/文件步骤可能已完成"),
        Message::PinBudget => Some("贴图数量或内存预留已达上限；可关闭贴图或提高预算。"),
        Message::PinLabel => Some("贴图"),
        Message::PinLocked => Some("已锁定"),
        Message::PinClose => Some("关闭贴图"),
        Message::PinTop => Some("切换置顶"),
        Message::PinUnlock => Some("解锁位置"),
        Message::PinLock => Some("锁定位置"),
        Message::PinHide => Some("隐藏贴图（可从托盘恢复）"),
        Message::PinPlanned => Some(
            "复制/保存采用原始像素和当前旋转方向，不包含窗口缩放、透明度或控件。重新标注可继续编辑原图与标注对象。",
        ),
        Message::PinRotate => Some("顺时针旋转 90°"),
        Message::PinEdit => Some("原地继续标注"),
        Message::PinEditApply => Some("应用"),
        Message::PinEditTitle => Some("贴图重新标注 · Enter 使用已保存完成动作 · Esc 取消"),
        Message::PinEditSelectHint => {
            Some("单击并拖动标注，手柄调整对象大小；原图范围固定。双击使用已保存完成动作。")
        }
        Message::PinEditHint => Some(
            "按原图方向编辑。应用替换原贴图并保留旋转和外观；复制/保存仅输出当前画布，取消保留原图。",
        ),
        Message::PinRotating => Some("正在旋转，就绪前保留当前贴图…"),
        Message::PinRotated => Some("已旋转，复制/保存采用此方向和原始分辨率。"),
        Message::SystemThemeUnavailable => Some("无法读取系统主题，暂用浅色；可手动选择主题。"),
        Message::UnsavedChanges => Some("有未保存的修改"),
        Message::RecoveredBackup => {
            Some("配置已损坏：原件已保留，当前载入上次有效备份；保存后才修复文件。")
        }
        Message::RecoveredDefaults => {
            Some("配置已损坏：原件已保留，当前使用默认草稿；保存后才修复文件。")
        }
        Message::UnsupportedConfig => Some("配置版本不受支持，已保护原文件；当前只能查看默认值。"),
        Message::UnreadableConfig => {
            Some("配置无法读取或无法保留损坏原件；请检查访问权限后重新打开。当前只读。")
        }
        Message::Settings => Some("设置"),
        Message::SettingsSubtitle => Some("只在需要时打开；截图与标注不经过这里。"),
        Message::CloseToTray => Some("关闭 · 回到托盘"),
        Message::SaveSettings => Some("保存设置"),
        Message::CancelChanges => Some("取消修改"),
        Message::RestoreDefaults => Some("恢复默认"),
        Message::Unbound => Some("未绑定"),
        Message::ContinueEditing => Some("继续编辑"),
        Message::DiscardAndLeave => Some("放弃并离开"),
        Message::SaveAndLeave => Some("保存并离开"),
        Message::ClearBinding => Some("清除绑定"),
        Message::AcceptShortcut => Some("使用此组合"),
        Message::DevelopmentStatus => Some("截图 · 标注 · 贴图"),
        Message::DraftExplanation => Some("修改后点击“保存设置”应用。"),
        Message::SystemPreference => Some("系统首选："),
        Message::ManualLanguage => Some("手动选择优先"),
        Message::LanguageFallback => Some("无对应语言包，回退 English"),
        Message::AutomaticLanguage => Some("自动匹配"),
        Message::UnsavedTitle => Some("设置尚未保存"),
        Message::ResetTitle => Some("恢复默认设置？"),
        Message::RecordShortcutTitle => Some("按下新的组合键"),
        Message::UnsavedBody => {
            Some("保存后离开、放弃本次修改，或继续编辑。保存失败时将保留当前草稿。")
        }
        Message::ResetBody => Some("仅恢复设置草稿；保存前不会修改已保存配置。"),
        Message::ShortcutRegistrationFailed => {
            Some("全局热键注册失败。可从托盘截图或打开设置重新绑定。")
        }
        Message::OptionUnavailable => Some("此项当前不可更改。"),
        Message::RecordShortcutBody => {
            Some("按下组合键或功能键。Esc 取消，Tab 切换按钮；确认后保存设置。")
        }
        Message::ShortcutInvalid => Some("组合键无效或与其他动作重复，请重新录入。"),
        Message::ShortcutCandidate => Some("Enter 确认，Esc 取消。保存设置后生效。"),
        Message::ShortcutServiceStopped => Some("全局热键服务已停止。请从托盘操作并重启应用。"),
        Message::SettingsSaved => Some("设置已保存。"),
        Message::SaveRollbackFailed => {
            Some("保存失败且系统设置回滚失败。草稿已保留；请检查登录启动项和热键后重试。")
        }
        Message::SaveFailed => {
            Some("保存失败，草稿已保留。请检查快捷键占用、登录启动项或配置文件访问权限后重试。")
        }
    }
}
