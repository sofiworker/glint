// Extended English catalog, kept separate from locale detection and rendering.
Object.assign(english,{
'采集已结束，统一调整未对齐片段。':'Capture finished. Adjust unmatched joins together.',
'本段待对齐，采集继续；可滚回重试或稍后处理。':'Join pending; capture continues. Scroll back to retry or review later.',
'截图模式':'Capture mode',
'滚动截图':'Scrolling capture',
'录屏':'Recording',
'开始滚动截图':'Start scrolling capture',
'开始录制':'Start recording',
'鼠标指针':'Cursor',
'系统声音':'System audio',
'麦克风':'Microphone',
'框选区域仅包含可滚动内容，避开固定页头、滚动条及动画。':'Select only scrolling content, avoiding fixed headers, scrollbars and animation.',
'Glint · 原型示意':'Glint · Prototype sample',
'项目进展记录':'Project progress',
'本周工作':'This week',
'方案与进度':'Plans and progress',
'数据回顾':'Data review',
'下一步计划':'Next steps',
'内容仅用于交互演示，不读取真实屏幕。':'Illustrative content; the real screen is not captured.',
'保留重要信息，让内容自然连接。':'Keep the details and connect the content.',
'滚动查看后续内容，右侧同步展示拼接效果。':'Scroll for more content; the stitched result appears on the right.',
'滚动采集区域':'Live scrolling region',
'拼接结果预览':'Stitched image preview',
'内容没有对齐，已暂停采集。':'The join needs review. Capture is paused.',
'↓ 自动滚动':'↓ Auto scroll',
'Ⅱ 停止滚动':'Ⅱ Stop scrolling',
'自动滚动演示':'Automatic scrolling demo',
'缓慢滚动选区，自动拼接':'Scroll slowly within the selection to stitch',
'结束采集':'Finish capture',
'继续采集':'Resume capture',
'编辑长截图':'Edit long screenshot',
'保存长截图':'Save long screenshot',
'取消滚动截图':'Cancel scrolling capture',
'复制长截图':'Copy long screenshot',
'录屏预览':'Recording preview',
'录屏预览 · MP4':'Recording preview · MP4',
'录屏预览 · GIF':'Recording preview · GIF',
'原型演示，不生成视频文件':'Prototype only; no video file is generated',
'录制进度':'Playback position',
'复制到剪贴板':'Copy to clipboard',
'Ⅱ 暂停':'Ⅱ Pause',
'▶ 继续':'▶ Resume',
'结束录屏':'End recording',
'放弃录制':'Discard recording',
'原型演示':'Prototype demo',
'请先撤销标注，再切换采集模式。':'Undo annotations before changing capture mode.',
'交互演示，不生成文件或修改剪贴板。':'Prototype only; files and clipboard remain unchanged.',
'原型仅展示播放入口，不模拟视频播放成功。':'The prototype shows the playback entry only; it does not play a video.',
'保存已取消；选区与标注已保留，可继续编辑或重试。':'Save cancelled. Selection and annotations are preserved; continue editing or retry.',
'请先松开鼠标完成当前操作，再输出。':'Release the mouse to finish the current operation before exporting.',
'跟随系统主题时，截图浮层、贴图和倒计时响应系统深浅色变化；手动主题不受影响。不改变截图像素、标注或贴图位置。':'When following the system theme, overlays, pins and countdowns respond to light/dark changes. Manual themes stay unchanged. Capture pixels, annotations and pin positions stay intact.',
'首版固定开启：关闭设置收起到托盘；退出请使用托盘菜单。':'Always on in this version: closing settings returns to the tray. Use the tray menu to quit.',
'首版固定开启：重复启动激活已有实例。':'Always on in this version: starting again activates the existing instance.',
'首版固定开启：完成前保留独立标注对象，可选择、调整与撤销。':'Always on in this version: keep annotation objects selectable, editable and undoable until completion.',
'首版固定开启：导出仅含合成像素，不复制原文件元数据或编辑对象。':'Always on in this version: export composited pixels without original file metadata or edit objects.',
'首版固定开启：只在输入、图像或窗口状态变化时更新。':'Always on in this version: update on input, image or window state changes.',
'配置中的关闭值当前不生效；保留该值，不会自动改写。':'The configured off value has no effect in this version. It is preserved without rewriting the configuration.',
'手动保存重名时':'When a manual save name exists',
'保留已有文件并报错':'Keep existing file and report error',
'自动编号':'Add a number',
'每次确认覆盖':'Confirm each replacement',
'自动副本始终编号；取消或失败保留当前编辑。':'Additional copies always use numbers. Cancel or failure keeps current edits.',
'模拟文件重名':'Simulate existing file',
'文件已存在':'File already exists',
'将自动编号，不覆盖已有文件。':'A number will be added without replacing existing files.',
'覆盖会替换已有文件。请确认后继续。':'Replacement changes the existing file. Confirm to continue.',
'已保留已有文件，请返回并选择其他文件名。':'The existing file is preserved. Return and choose another name.',
'原型仅模拟冲突，不写入文件。':'The prototype simulates conflicts without writing files.',
'返回保存':'Return to save',
'编号保存':'Save with a number',
'确认覆盖':'Confirm replacement',
'选择保存目录':'Choose output folder',
'原型仅模拟选择目录，不访问或创建文件夹。选择仅更新当前设置草稿，保存后生效；取消或失败保留原值。':'This prototype simulates folder selection without accessing or creating folders. Choosing updates only the settings draft; Save applies it. Cancel or failure keeps the original value.',
'无法选择目录，可重试或取消后手动输入。':'Cannot choose a folder. Retry or cancel and type a path.',
'模拟选择失败':'Simulate selection failure',
'选择示例目录':'Choose example folder',

'选择字体':'Choose font',
'搜索字体':'Search fonts',
'没有匹配字体，可修改搜索或取消后手动输入。':'No matching fonts. Change the search or cancel and type a name.',
'原型仅列出示例字体，不读取本机字体。原生版按需读取系统字体，选择仅更新当前草稿。':'The prototype lists example fonts without reading this computer. The native app reads system fonts on request; choosing updates only the current draft.',

'默认关闭，保存后生效。仅记录时间及固定事件代码，不记录图像、正文、文件路径、窗口标题或配置值。':'Off by default; Save applies. Records only timestamps and fixed event codes, never images, text, paths, window titles or configuration values.',
'日志位于配置目录的 diagnostics 文件夹，最多两份 512 KiB 文件，循环替换。关闭后停止新记录，已有文件保留至手动清理；清理不改变设置或当前编辑。记录失败可在此重试。':'Logs are in diagnostics under the configuration directory. At most two 512 KiB files rotate. Turning logging off stops new records; existing files remain until cleared. Clearing preserves settings and current edits. Failed recording can be retried here.',
'重试记录':'Retry logging',
'原型仅演示重试，不写入本机日志。':'The prototype only demonstrates retrying; no local logs are written.',
'清理诊断日志':'Clear diagnostic logs',
'清理诊断日志？':'Clear diagnostic logs?',
'仅删除两份诊断日志，不删除截图、贴图会话或配置。开启记录时，之后的事件会重新创建日志。':'Delete only the two diagnostic logs. Captures, pin sessions and configuration are preserved. If logging is on, later events create logs again.',
'确认清理日志':'Confirm clearing logs',
'原型仅演示清理，不删除本机文件。':'The prototype only demonstrates clearing; no local files are deleted.',
'运行诊断':'Runtime diagnostics',
'分别查看界面渲染器、截图后端、显示器 / DPI 与进程资源。刷新不采集截图、窗口标题或剪贴板，不保存或上传诊断。':'Inspect UI renderer, capture backend, displays / DPI and process resources separately. Refresh does not capture pixels, window titles or clipboard content, and does not save or upload diagnostics.',
'刷新诊断':'Refresh diagnostics',
'HTML 不读取本机诊断。原生版在此显示真实查询结果；失败保留上次快照并允许重试。':'HTML does not read local diagnostics. The native app shows actual query results here; failures retain the previous snapshot and allow retry.',
'尚未查询。快照仅代表刷新时刻，不证明环境兼容性。':'Not queried. A snapshot describes the refresh time and does not establish environment compatibility.',

'模拟锁屏 / 休眠':'Simulate lock / sleep',
'截图已暂停':'Capture paused',
'此处模拟锁屏、会话断开或休眠，不执行真实系统操作。':'This simulates lock, session disconnect or sleep without a real system operation.',
'模拟恢复':'Simulate resume',
'本次截图已结束，请重新截图。':'This capture ended. Start a new capture.',

  '仅控制成功提示，失败仍保留当前任务。系统通知提交失败时回退轻提示；是否展示由 Windows 控制。不显示图片、正文或路径。':'Controls success feedback only; failures keep the current task. Failed notification submission falls back to brief feedback. Windows controls delivery. No images, text content or paths are shown.',
  '系统通知（模拟）':'System notification (simulation)',
  '打码（使用优先方式）':'Redaction (preferred method)',
  '保存后用于下一次截图或图片编辑；仍先框选，再使用默认工具。':'Saved defaults apply to the next capture or image editor. Select a region before using the default tool.',
  '默认工具为打码时使用此方式；工具条中两种工具仍分别可选。敏感内容优先实心遮盖。':'Used when the default tool is redaction. Both tools remain available separately. Use solid cover for sensitive content.',
  '删除已保存贴图会话？':'Delete saved pin session?',
  '删除已保存贴图会话':'Delete saved pin session',
  '删除保存的会话':'Delete saved session',
  '仅删除保存的会话及未完成的会话临时文件，不关闭当前贴图，不删除主动导出的图片。恢复仍开启时，下次正常退出会再次保存当前贴图。':'Delete only the saved session and unfinished session files. Current pins and exported images stay. If restoration remains enabled, the next normal exit saves the current pins again.',
  '无法保存贴图会话':'Cannot save pin session',
  '保留当前贴图和上次成功保存的会话。可以重试退出，或不保存本次贴图退出。原型仅模拟文件失败。':'Keep current pins and the last successful session. Retry exit or exit without saving the current pins. Prototype simulation only.',
  '不保存本次贴图退出':'Exit without saving pins',
  '重试退出':'Retry exit',
  '模拟恢复':'Simulate restore',
  '模拟保存失败':'Simulate save failure',
  '已删除示例会话，不影响当前贴图（模拟）。':'Sample session deleted; current pins stay (simulation).',
  '正常退出时保存当前贴图（模拟）。':'Save current pins on normal exit (simulation).',
  '不保存本次贴图，保留上次成功会话（模拟）。':'Keep the last successful session without saving current pins (simulation).',
  '没有保存的示例会话。':'No saved sample session.',
  '恢复示意：当前对象可编辑，旧撤销历史不恢复；窗口可见且不穿透。':'Restore example: current objects remain editable; old undo history is omitted. Pins are visible and not click-through.',
  '会话恢复默认关闭，独立于历史。正常退出时保存源图（包含被遮盖内容）、当前对象与窗口状态，上限 256 MiB。不保存旧撤销历史。恢复后窗口可见且不穿透。关闭并保存设置时删除已保存会话。强制结束时保留上次成功会话。':'Session restore is off by default and separate from history. Normal exit saves source pixels (including covered content), current objects and window state, up to 256 MiB. Old undo history is omitted. Restored pins are visible and not click-through. Disable and save to delete the saved session. Forced exit keeps the last successful session.',
  '独立于截图历史；开启并保存后，正常退出时才保存贴图会话。':'Separate from screenshot history. Enable and save to keep a pin session on normal exit.',
  '本地图片 · 整图标注 · 另存或贴图 · Esc 回到桌面':'Local image · Annotate the whole image · Save as or pin · Esc returns to desktop',
  'PNG/JPEG 文件 ≤ 64 MiB · RGBA ≤ 64 MiB · 画布 ≤ 128 MiB':'PNG/JPEG file ≤ 64 MiB · decoded RGBA ≤ 64 MiB · canvas ≤ 128 MiB',
  '无法打开图片，请重新选择 PNG/JPEG。':'Cannot open image. Choose another PNG/JPEG.',
  '本地图片编辑示意：复制、另存或贴图；不覆盖原文件，取消回到桌面。':'Local image editing example: copy, save as or pin; preserve the source, cancel to the desktop.',
  '导出已保存配置':'Export saved configuration',
  '仅导出已保存的设置与快捷键，不包含当前草稿、引导状态、截图像素、历史资产或系统授权。':'Export saved settings and shortcuts only. Excludes the current draft, onboarding state, image pixels, history assets and system permissions.',
  '示例预览，不写入真实文件。':'Sample preview; no actual file is written.',
  '导出失败，预览保留，可重试或另选文件。':'Export failed. Preview preserved; retry or choose another file.',
  '模拟导出失败':'Simulate export failure',
  '模拟选择导出文件':'Simulate choosing an export file',
  '导入配置':'Import configuration',
  '选择 TOML 文件 → 格式 / 版本校验 → 变更预览 → 合并到草稿。保存设置时再重新注册系统热键，失败保留原可用绑定。':'Choose TOML → validate format / version → preview → merge into draft. Save settings to register system shortcuts; failures keep working bindings.',
  '仅模拟文件选择，不读取真实文件。':'File selection simulation only; no actual file is read.',
  '选择无效示例':'Choose invalid sample',
  '选择有效示例':'Choose valid sample',
  '无法导入配置':'Cannot import configuration',
  '文件超过 256 KiB、格式错误或版本不受支持时拒绝导入。当前草稿与已保存配置保持原状。':'Reject files over 256 KiB, malformed files and unsupported versions. Current draft and saved configuration are preserved.',
  '重新选择':'Choose again',
  '配置导入预览':'Configuration import preview',
  '确认后合并到当前设置草稿，不会立即写盘或修改系统热键。无效 / 未知字段保留原值；快捷键冲突时保留原整组绑定。':'Confirm to merge into the current draft. No immediate file writes or shortcut registration. Invalid / unknown fields keep current values; shortcut conflicts keep the entire binding group.',
  '已跳过：capture.delay_seconds = 999（无效）。引导状态与系统授权不导入。':'Skipped: capture.delay_seconds = 999 (invalid). Onboarding state and system permissions are not imported.',
  '合并到草稿':'Merge into draft',
  '配置导出完成（模拟，未写入文件）。':'Configuration exported (simulation; no file written).',
  '已捕获的示意指针':'Sample captured pointer',
  '截图倒计时':'Capture countdown',
  '倒计时结束后才冻结桌面；窗口截图可在此期间将指针移到目标窗口。':'The desktop stays live until capture. Move the pointer to the target window if needed.',
  '取消截图（Esc）':'Cancel capture (Esc)',
'区域记忆已关闭，请手动框选。':'Region memory is off. Select a region manually.',
'本次运行尚无成功截图，请手动框选。':'No successful capture in this session. Select a region manually.',
'显示器布局或 DPI 已变化，请重新框选。':'Display layout or DPI changed. Select a new region.',
'已恢复上次区域；重新采集画面，不带入旧标注。':'Previous region restored on a fresh capture, without old annotations.',
'救援热键失效':'Rescue shortcut stopped working',
'救援入口不可用':'Rescue is unavailable',
'救援快捷键不可用，未开启穿透。已有贴图保持可点击；修复快捷键后再试，仍可从托盘恢复。':'The rescue shortcut is unavailable, so click-through stays off. Pins remain interactive. Fix the shortcut and retry; tray rescue remains available.',
'救援快捷键失效，已取消全部贴图穿透。请修复快捷键；托盘救援仍可显示并找回贴图。此处仅模拟。':'The rescue shortcut failed. Click-through was cleared for all pins. Fix the shortcut; tray rescue can still show and recover pins. This only simulates failure.',
'保存贴图':'Save pin',
'模拟保存贴图':'Simulate saving pin',
'模拟贴图输出失败':'Simulate pin output failure',
'贴图输出失败':'Pin output failed',
'使用图像原始像素和当前旋转方向，不包含窗口缩放、透明度或控件。保存成功或取消后，贴图均保持打开。':'Use source pixels in the current rotation, excluding window zoom, opacity and controls. The pin stays open after saving or cancelling.',
'已模拟保存贴图；贴图保持打开，没有写入文件。':'Simulated pin saved; it remains open. No file was written.',
'已模拟复制贴图；使用图像原始像素和当前旋转方向，不包含窗口缩放、透明度或控件。':'Simulated pin copy using source pixels in the current rotation, excluding window zoom, opacity and controls.',
'保留当前贴图、缩放和透明度。可重新复制，或另选位置保存；导出使用图像原始像素，不包含窗口控件、显示缩放或窗口透明度。此处仅模拟失败。':'Keep the pin, zoom and opacity. Retry copying or save elsewhere. Export original image pixels without controls, display zoom or window opacity. This only simulates failure.',
'无法贴出剪贴板图像':'Cannot pin clipboard image',
'剪贴板中没有可读取的图像，或正被其他应用占用。已有贴图保留；复制一张图像后，再从托盘或快捷键重试。此处仅模拟失败。':'The clipboard has no readable image, or another app is using it. Existing pins remain. Copy an image and retry from the tray or shortcut. This only simulates failure.',
'操作此贴图':'Select this pin',
'独立贴图示意，不写入真实桌面。':'Independent sample pin; the real desktop is unchanged.',
'多张贴图独立关闭；隐藏和救援作用于全部贴图。':'Close pins independently; hide and rescue affect all pins.',
'自动副本目录不可写':'Automatic copy folder is not writable',
'改为另存为（不写自动副本）':'Save As (skip automatic copy this time)',
'保留当前编辑；可重试，或改为另存为并跳过本次自动副本。不改变已保存设置。':'Keep edits. Retry, or Save As and skip this automatic copy. Saved settings stay unchanged.',
'默认关闭；开启后先保存副本再完成复制/保存。重名自动编号，失败保留编辑，未改动重试复用已保存副本。':'Off by default. Save a numbered copy before completing copy/save. Keep edits on failure; unchanged retries reuse the saved copy.',
'自动副本已保存，剪贴板失败':'Automatic copy saved; clipboard failed',
'副本已保存，复制失败':'File saved; copy failed',
'模拟自动副本已保存，剪贴板被占用。选区与标注保留；未修改重试不重复保存副本。原型不写入真实文件。':'Simulated file saved, clipboard occupied. Keep selection and annotations; unchanged retry does not save a duplicate. The prototype writes no real files.',
'取消文字':'Cancel text',
'单击输入':'Click to enter',
'字号':'Size',
'单行文字，最多 80 字符；Enter 放置，Esc 取消文字。':'Single line, up to 80 characters. Enter places it; Esc cancels text.',
'16 px 方块示意；原生版按标注顺序像素化已合成画面，不保证保密。':'16 px sample blocks. The native app pixelates the composite in object order; confidentiality is not guaranteed.',
'示例 #4B5CFA · RGB(75, 92, 250)':'Sample #4B5CFA · RGB(75, 92, 250)',
'复制 HEX':'Copy HEX','复制 RGB':'Copy RGB',
'原图像素（不含标注）；单击模拟复制 HEX':'Original pixels (without annotations); click to simulate copying HEX',
'RGB(75, 92, 250) 示例；未写入剪贴板。':'Sample RGB(75, 92, 250); clipboard unchanged.',
'HEX 示例 #4B5CFA；未写入剪贴板。':'Sample HEX #4B5CFA; clipboard unchanged.',
 '截图：':'Capture:','标注':'Annotate','按':'Press','呼起截图；也可点击下面按钮模拟。':'to capture, or use the button below.','救援':'Rescue','贴图 01 · 置顶参考':'Pin 01 · Always on top','刚刚截取 / 示意内容':'Just captured / Sample content','把重点留下':'Keep what matters','不透明度':'Opacity','贴在桌面，继续参考。':'Keep a reference on your desktop.','拖动贴图标题栏；右键或“···”查看更多操作。':'Drag the title bar. Right-click or use “···” for more actions.','隐藏全部贴图':'Hide all pins','救援 / 取消穿透':'Rescue / Disable click-through','贴图设置':'Pin settings','当前原型最多展示一张；分组、多图管理在后续原生实现。':'This prototype shows one pin. Groups and multiple pins are planned for the native app.',
 '可保留的次要入口；截图、打码、复制和贴图无需经过这里。':'An optional entry. Capture, redact, copy and pin without this window.','打开本地图片':'Open image','历史策略':'History policy','当前历史 默认关闭。下方仅显示示例条目，不读取你的实际图像。':'History is off by default. Items below are samples, not your images.','浮层中编辑':'Edit in overlay','流程边界':'Workflow boundaries','从历史重新编辑也使用独立浮层；不会把常规截图引导到这个窗口。':'History editing also uses an overlay. Normal captures never require this window.','输出默认动作':'Default output action','清空示例历史':'Clear sample history','异常与恢复流程':'Errors and recovery','让失败留在当前任务附近，不静默丢失截图，也不强制跳到主窗口。':'Handle failures in context. Do not lose captures or force a main-window detour.','截图与系统':'Capture and system','快捷键注册冲突':'Shortcut registration conflict','截图源不可用 / 受保护内容':'Capture unavailable / Protected content','截图中显示器 / DPI 改变':'Display / DPI changed during capture','屏幕访问权限被拒绝':'Screen access denied','输出与贴图':'Output and pins','剪贴板被占用，复制失败':'Clipboard busy; copy failed','保存目录无权限 / 磁盘已满':'Cannot write / Disk full','贴图内存预算不足':'Pin memory budget exceeded','穿透 / 隐藏后找不到贴图':'Recover hidden or click-through pins','回到截图主流程':'Return to capture','查看设置':'View settings',
 '点击组合键重新录入。此原型只验证录入和冲突交互，不注册系统热键。':'Click a shortcut to change it. This prototype records keys and detects local conflicts; it does not register system shortcuts.','穿透救援热键不可清空。冲突时不覆盖已有绑定，保留托盘入口。不同系统的可用组合键与保留键需在对应平台验证。':'The rescue shortcut cannot be cleared. Keep existing bindings when conflicts occur. Reserved keys require validation on each platform.','演示系统占用冲突':'Demo system conflict',
 '确定选区后直接在原处编辑，工作台不参与必经流程。':'Edit directly after selecting. No workspace step required.','选区与定位':'Selection and targeting','显示像素放大镜':'Show pixel magnifier','跟随指针查看像素；按需刷新，静止时不持续重绘。':'Inspect pixels near the pointer. Refresh on demand, not continuously when still.','显示选区尺寸和坐标':'Show dimensions and coordinates','导出采用源像素，界面缩放不改变导出尺寸。':'Export uses source pixels. UI scaling does not change output dimensions.','自动检测窗口边界':'Detect window boundaries','移动指针高亮，单击选中，仍可调整区域。':'Hover to highlight, click to select, then adjust if needed.','检测窗口内的子元素':'Detect elements inside windows','计划通过平台可用能力增强，失败回退到窗口 / 手动区域。':'Use platform support where available. Fall back to window or manual selection.','记住上次区域':'Remember last region','仅保留区域几何信息，不代表自动保留截图。':'Remember geometry only, not captured pixels.','捕获选项':'Capture options','包含鼠标指针':'Include pointer','默认关闭；系统捕获方式不同，需原生实现合成。':'Off by default. Pointer composition depends on the capture method.','截图前延迟':'Capture delay','选区外遮罩强度':'Outside-selection dimming','跨屏 / 混合 DPI 的选区、负坐标、屏幕缩放与边缘手柄应独立验收。最终工具条按可用空间放在选区下方、上方或屏幕边缘。':'Validate multi-display selection, negative coordinates, scaling and resize handles separately. Place tools below, above or near screen edges as space permits.','查看选区与原地工具条':'Preview selection and inline toolbar',
 '所有操作留在截图浮层；一次拖动对应一步撤销。':'All editing stays in the overlay. One drag is one undo step.','工具默认值':'Tool defaults','默认工具':'Default tool','选择 / 移动选区':'Select / Move region','默认线宽':'Default stroke width','默认标注颜色':'Default annotation color','HEX 色值，例如 #ED6571。':'HEX color, for example #ED6571.','箭头样式':'Arrow style','实心箭头':'Filled arrow','开放箭头':'Open arrow','文字字体':'Text font','最终版本使用系统字体列表；不复用旧富文本编辑器。':'The native app uses system fonts, not the previous rich-text editor.','文字字号':'Text size','遮盖与编辑':'Redaction and editing','优先打码方式':'Preferred redaction method','分享敏感内容时优先使用实心遮盖。':'Prefer solid redaction before sharing sensitive content.','像素化（视觉处理）':'Pixelation (visual only)','完成前保持对象可编辑':'Keep objects editable before finishing','原型演示绘制 / 撤销；原生首版还需选中、移动与改样式。':'Prototype supports drawing and undo. Object selection, movement and restyling are native milestones.','复制和常规图片导出必须得到合成像素，不附带可还原原图的编辑图层。像素化不保证保密；会话中保留原图也不等同安全擦除。':'Copy and standard export must flatten pixels without recoverable source layers. Pixelation is not secure redaction. Keeping a source in memory is not secure erasure.','试用浮层标注':'Try inline annotation',
 '默认 Enter = 复制并离开，不打开主窗口。':'Default Enter action: copy and close, without a main window.','完成动作':'Completion','完成后的默认动作':'Default completion action','完成时额外保存文件副本':'Also save a file on completion','默认关闭；开启后才使用下方目录与命名规则。':'Off by default. Enable to use the folder and naming rules below.','完成反馈':'Completion feedback','轻提示':'Brief toast','不提示':'None','系统通知':'System notification','文件':'Files','图片格式':'Image format','有损格式质量':'Lossy quality','PNG 不使用此项。':'Does not apply to PNG.','保存目录':'Save folder','原型路径；不会访问或创建真实目录。':'Sample path. No actual folder is accessed or created.','文件名模板':'File name template','支持 {date}、{time}、{width}、{height}。':'Supports {date}, {time}, {width} and {height}.','不保留非必要元数据':'Strip unnecessary metadata','常规导出不携带原图编辑对象或隐藏原图。':'Standard exports do not contain editable objects or hidden sources.','预览：':'Preview:','选择目录（原型）':'Choose folder (prototype)','预览保存 / 复制面板':'Preview output dialog',
 '贴图是独立桌面窗口，截图结束后继续参考，不打开工作台。':'Pins are independent reference windows. No workspace opens after capture.','默认始终置顶':'Always on top by default','初始不透明度':'Initial opacity','初始显示比例':'Initial scale','不改变原始像素。':'Does not change source pixels.','滚轮动作':'Mouse wheel behavior','滚轮缩放 · Ctrl + 滚轮调透明度':'Wheel: zoom · Ctrl + wheel: opacity','滚轮调透明度 · Ctrl + 滚轮缩放':'Wheel: opacity · Ctrl + wheel: zoom','贴图靠近屏幕或窗口边缘时吸附':'Snap pins to screen or window edges','拖动结束后在 12 px 内吸附；不自动隐藏。显示器变化时保留状态并找回位置。':'Snap within 12 px when dragging ends. Keep pins visible. Recover positions after display changes while preserving pin state.','可见性与恢复':'Visibility and recovery','开始截图时临时隐藏已有贴图':'Temporarily hide pins during capture','结束或取消后恢复，避免把参考图再次截入。':'Restore after completion or cancellation. Avoid capturing reference pins.','新贴图默认鼠标穿透':'Click-through by default','默认关闭；开启前需要确认救援入口。':'Off by default. Confirm rescue access before enabling.','重启后恢复贴图':'Restore pins after restart','会保存图片到本地，需要用户明确启用磁盘历史 / 贴图存储。':'Requires explicit permission to store pin images on disk.','查看贴图与右键交互':'Preview pin controls',
 '默认不保存截图历史。区域记忆与图像保存是两件事。':'History is off by default. Remembering a region is separate from saving pixels.','保存策略':'Storage policy','默认关闭；打开后允许辅助工作台查看本次截图。':'Off by default. Enable to see session captures in the optional workspace.','允许历史写入磁盘':'Allow history on disk','默认关闭；开启后需说明目录、保留时间、清理方式。':'Off by default. Show storage location, retention and cleanup policy before enabling.','磁盘历史保留天数':'Disk history retention','只在磁盘历史开启时生效。':'Applies only when disk history is enabled.','历史源图内存预算':'Source history memory budget','退出后清空临时截图缓存':'Clear temporary captures on exit','关闭时也不能无限保留匿名临时文件。':'Disabling this must not permit unlimited temporary storage.','边界与诊断':'Exclusions and diagnostics','排除程序列表':'Excluded applications','原型示例：password-manager.exe；最终按平台进程 / 窗口匹配。':'Example: password-manager.exe. The native app matches processes or windows.','允许保存诊断日志':'Allow diagnostic logs','默认关闭；即使开启也不记录截图像素、OCR 内容或剪贴板正文。':'Off by default. Logs never include capture pixels, OCR results or clipboard contents.','清空历史与临时图片':'Clear history and temporary images','查看辅助历史入口':'View optional history workspace','原型只使用固定示例数据，不读取真实剪贴板、文件、进程或桌面。设置仅保存在当前浏览器 localStorage 中。':'This prototype uses fixed samples, not your clipboard, files, processes or screen. Settings are stored only in browser localStorage.',
 '减少装饰，明确状态；主题不覆盖截图内容本身的颜色。':'Keep controls simple and states clear. Themes do not change capture colors.','默认靛蓝 #4B5CFA；仅用于主要操作、选中与焦点。':'Default indigo #4B5CFA for primary actions, selection and focus.','界面缩放':'Interface scale','图标＋文字 / 仅图标、显示与排序、位置统一在工具条设置。':'Configure icons, labels, visibility, order and position in toolbar settings.','不播放循环动画；必要转场可以关闭。':'No looping animations. Optional transitions can be disabled.','提高边界对比度':'Increase edge contrast','增强工具条、选区和贴图边界，避免只依赖颜色。':'Stronger toolbar, selection and pin edges. Do not rely only on color.','控件预览':'Control preview','雾白':'Mist','石墨':'Graphite','HTML 中即时预览明暗和强调色。界面缩放、高对比和动画偏好仅为原生版本配置规划。':'Light, dark and accent colors preview live. Scale, contrast and motion settings are planned for the native app.',
 '后台由系统事件唤醒，不轮询按键；截图与贴图资源按需创建。':'Wake on system events, not key polling. Allocate capture and pin resources on demand.','渲染与捕获':'Rendering and capture','UI 渲染后端':'UI renderer','首轮以已验证的 Slint software 为兼容基线。':'Use the validated Slint software renderer as the initial compatibility baseline.','Windows 捕获源':'Windows capture source','UI 渲染后端和屏幕捕获方式独立。WGC / DXGI 的具体选型先验证，不假定自动回退已实现。':'UI rendering and capture are independent. Validate WGC / DXGI choices; automatic fallback is not yet implemented.','自动（平台能力优先）':'Automatic (platform capability first)','GDI 兼容截图':'GDI compatibility capture','WGC / DXGI（待验证）':'WGC / DXGI (to validate)','静止时不持续请求刷新':'No continuous redraw when idle','只在输入、图像或窗口状态变化时更新。':'Update on input, image or window state changes only.','锁屏 / 休眠时释放捕获资源':'Release capture resources on lock / sleep','锁屏、断开会话或休眠时取消倒计时及未完成截图编辑，恢复后手动重新截图。关闭时仅隐藏并保留已有编辑；进行中的导出可能仍完成。':'Cancel countdowns and unfinished capture edits on lock, session disconnect or sleep; start a new capture after resuming. When off, hide and retain existing edits. An export already in progress may still complete.','内存预算':'Memory budgets','编辑预览内存预算':'Edit preview budget','保存后用于新截图及图片再编辑的标注像素分块；超限保留编辑，可撤销或重新框选。不含源图、导出与渲染器内存。':'Saved limit applies to annotation pixel tiles in new captures and image editors. Over budget keeps edits; undo or reselect to recover. Excludes source images, export and renderer memory.','所有贴图总预算':'Total pin budget','超限时提示关闭或降低预览分辨率。':'Suggest closing pins or lowering preview resolution when over budget.','旧 Demo 本机托盘空闲：工作集约 27.5 MiB；12 秒未观察到 CPU 时间增加。仅为旧验证参考，不是本项目承诺或当前 HTML 的进程读数。':'Previous demo: about 27.5 MiB working set while in tray, no CPU time increase observed over 12 seconds. This is past test data, not a Glint guarantee or live HTML reading.','查看验证边界':'View validation limits',
 'Windows → macOS → Linux，逐个平台验证，不以框架跨平台代替功能验收。':'Windows → macOS → Linux. Validate each platform; framework portability is not feature acceptance.','能力':'Capability','Windows · 首发':'Windows · First','macOS · 第二阶段':'macOS · Second','Linux · 第三阶段':'Linux · Third','截图浮层 / 贴图':'Capture overlay / Pins','优先开发':'Implement first','原生窗口 / 多屏':'Native windows / Multiple displays','X11 / Wayland 分开评估':'Evaluate X11 / Wayland separately','系统快捷键':'Global shortcuts','平台注册机制':'Platform registration','桌面 / portal 支持不同':'Desktop / portal dependent','截图源':'Capture source','兼容路径 + 现代捕获验证':'Compatibility + modern capture validation','权限失败':'Permission failures','错误提示与手动区域':'Error feedback and manual selection','屏幕录制授权 / 重试':'Screen Recording permission / Retry','portal 拒绝 / 合成器限制':'Portal denial / Compositor limits','以上是路线与原型状态，不表示任一平台的原生代码已实现。特别是 Wayland，截图确认与窗口控制可能无法达到 Windows 的无感体验。':'These are roadmap and prototype states, not implemented native features. Wayland permissions and window control may prevent the seamless Windows experience.',
 '先把截图、标注和贴图做稳定，再扩展；首发不预加载模型或编码器。':'Stabilize capture, annotation and pins first. No models or encoders preloaded at launch.','截图能力之后':'After basic capture','滚动长截图':'Scrolling capture','作为截图阶段的增强里程碑：手动滚动拼接 → 接缝检查 → 编辑。自动滚动需单独验证应用兼容性。':'Later capture milestone: manual scroll stitching → seam review → edit. Automatic scrolling requires app compatibility validation.','查看长截图流程':'View scrolling capture flow','截图稳定后':'After capture is stable','本地 OCR':'Local OCR','框选 → 按需加载本地模型 → 文本侧栏 → 复制 / 关闭；无上传开关的隐式变化。':'Select → load local model on demand → text panel → copy / close. No implicit upload changes.','查看 OCR 流程':'View OCR flow','最后阶段':'Final stage','录屏 / GIF / 录后编辑':'Recording / GIF / Post-editing','区域或窗口 → 音频选择 → 录制控制条 → 录后剪辑 / 遮盖 / 导出。与截图状态机隔离。':'Region or window → audio → recording controls → trim / redact / export. Separate from capture sessions.','查看录屏规划':'View recording roadmap',
 '关于拾光 Glint':'About Glint','Glint 是微光，拾光是把屏幕上的一刻留下。名称暂定。':'Glint means a brief light: keep a moment from your screen. Working name.','当前版本：交互原型 / 工程骨架':'Current stage: interactive prototype / project scaffold','业务功能尚未开发。HTML 仅验证流程与设置，不是未来应用的 WebView 界面。':'No native product features yet. HTML validates workflows and settings, not a future WebView UI.','技术栈：Rust 2024 + Slint 1.18.1；首轮 software CPU UI；系统能力使用原生平台接口。':'Stack: Rust 2024 + Slint 1.18.1. Software CPU UI first, with native platform APIs.','跨平台路线：Windows → macOS → Linux。OCR / 录屏依赖尚未引入。':'Roadmap: Windows → macOS → Linux. No OCR or recording dependencies yet.','About Slint / 许可':'About Slint / License','查看配置导出原型':'Preview configuration export','查看配置导入原型':'Preview configuration import','本地验证材料与开发说明在项目 .local/ 中，已加入 Git 忽略；未创建任何提交。':'Local validation and development notes are in .local/, ignored by Git. No commits created.',
 '模拟：系统中另一个应用占用了这组按键。原有可用绑定保持不变；你可以换一组，或继续使用托盘截图。':'Simulation: another app owns this shortcut. Keep the working binding. Choose another or use the tray.','可按修饰键 + 字母，或 F1–F12。Esc 取消。浏览器保留键可能无法拦截，系统注册仍需原生校验。':'Use modifiers + letter or F1–F12. Esc cancels. Browser-reserved keys may be unavailable. Native registration still needs validation.','在截图浮层上直接完成保存，成功后按设置退出；取消会保留选区和标注。':'Save directly from the overlay. Close after success; cancel keeps the selection and annotations.','导出：已合成的图像像素，不附原图图层。':'Output: flattened pixels, without source layers.','原型不写入真实文件。':'No actual file is written.','贴图操作':'Pin actions','这些操作围绕独立贴图窗口，不需要打开工作台。':'Actions apply to the independent pin. No workspace required.','原地继续标注':'Continue annotation in place','锁定 / 解锁位置':'Lock / Unlock position','开启鼠标穿透':'Enable click-through','旋转 90°':'Rotate 90°','复制贴图':'Copy pin','保存贴图':'Save pin','隐藏贴图':'Hide pin','关闭贴图':'Close pin','开启穿透前，先记住救援方式':'Know the rescue action before enabling click-through','穿透后鼠标可点到贴图后方窗口。按':'Click-through lets you interact with windows behind the pin. Press','或从托盘“恢复贴图并取消穿透”恢复。':'or use Tray → Show pins and disable click-through.','原型只模拟状态，不改变浏览器窗口穿透。':'Simulation only; the browser window does not become click-through.','已了解，开启':'Understood, enable','清空历史与临时图片？':'Clear history and temporary images?','不影响用户主动保存到其他位置的文件；真实版本需明确列出清理范围。原型只清空示例列表。':'Files you explicitly saved elsewhere are unaffected. The native app must state the cleanup scope. This demo clears samples only.','清空':'Clear','恢复到默认主题、快捷键与隐私策略；仍需点击“保存设置”才能应用。不会删除用户主动保存的图片。':'Restore default theme, shortcuts and privacy settings. Save to apply. Your saved images are unaffected.','热键与托盘会停止。尚有贴图，真实版本应按恢复策略提示保存。':'Shortcuts and tray will stop. Pins are still open; the native app should offer saving based on recovery policy.','原型只模拟退出状态，不关闭浏览器。':'Simulated exit only. The browser stays open.','Windows · 权限 / 能力提示':'Windows · Permissions / Capabilities','拒绝时不循环弹窗；保留取消、了解原因和主动重试。Wayland 的某些限制无法由应用绕过。':'No repeated prompts after denial. Allow cancel, explanation and manual retry. Some Wayland restrictions cannot be bypassed.','模拟拒绝':'Simulate denial','模拟授权后重试':'Simulate permission and retry',
 '设置入口':'Settings entry','完成动作：':'Completion action:','自动保存：':'Auto-save:','截图历史：':'Capture history:','取消穿透':'Disable click-through','拖动排序':'Drag to reorder','关闭对话框':'Close dialog','演示固定色值':'Fixed sample color','主色只用于选择、焦点和主要操作。':'Accent color is used for selection, focus and primary actions only.'
});

Object.assign(english,{
'示意窗口 / 项目备忘录\u3000\u3000—\u3000□\u3000×':'Sample window / Project notes    — □ ×',
'救援：Ctrl + Alt + Escape；或托盘 → 恢复全部贴图并取消穿透。禁止出现完全无法点回来的贴图。':'Rescue: Ctrl + Alt + Escape, or Tray → Show pins and disable click-through. Pins must always remain recoverable.'
});

Object.assign(english,{"已选中标注 · 拖动移动，Delete 删除；单击空白取消选中。":"Selected annotation: drag to move, Delete to remove; click blank space to deselect.","删除标注":"Delete annotation"});

Object.assign(english,{"单击标注选中并拖动；空白处移动选区，边缘手柄调整大小；双击使用默认完成动作。":"Click an annotation to select and drag it; blank space moves the selection, edge handles resize it. Double-click to finish.","已演示选中、移动、删除与撤销；对象尺寸与属性修改尚待补齐。":"Selection, movement, deletion and undo are demonstrated; object resizing and property editing are still pending."});

Object.assign(english,{"编辑文字":"Edit text"});

Object.assign(english,{"拖动移动文字 · 编辑文字可调整字号 · Delete 删除":"Drag text to move; Edit text changes font size; Delete removes it.","拖动移动标注 · 手柄调整形状 / 箭头端点 · Delete 删除":"Drag to move; use handles for shape size / arrow endpoints; Delete removes it."});

Object.assign(english,{"使用图像原始像素和当前旋转方向，不包含窗口缩放、透明度或控件。":"Use source image pixels in the current rotation, without window zoom, opacity or controls."});

Object.assign(english,{
'应用到原贴图':'Apply to original pin',
'已应用到原贴图，保留位置、旋转和外观（模拟）。':'Applied to the original pin; position, rotation and appearance kept (simulation).',
'已输出本次编辑；原贴图保持不变（模拟）。':'Output completed; the original pin stays unchanged (simulation).',
'按原图方向编辑；应用替换原贴图并保留显示旋转，复制/保存仅输出当前画布。':'Edit in the source orientation. Apply replaces this pin and keeps its display rotation. Copy/save outputs this canvas only.',
'原图方向编辑 · 应用到原贴图 · 取消保留原图 · 无需进入主窗口；遮盖直接绘制在选区中。':'Edit in source orientation · Apply to original pin · Cancel keeps the original · Annotate directly in this overlay.'
});

Object.assign(english,{'单击并拖动标注，手柄调整对象大小；原图范围固定。双击使用已保存完成动作。':'Select and drag an annotation; use its handles to resize. Image bounds stay fixed. Double-click uses the saved completion action.'});

Object.assign(english,{'模拟应用失败':'Simulate apply failure','无法应用到贴图':'Cannot apply to pin','原贴图保持不变，保留本次编辑。内存预算不足时，可关闭其他贴图或调整预算后重试。此处仅模拟失败。':'Keep the original pin and current edits. If the memory budget is full, close another pin or adjust the budget, then retry. This only simulates failure.'});

Object.assign(english,{"\u793a\u4f8b\u50cf\u7d20":"Sample pixels"});

Object.assign(english,{
"\u5168\u5c4f\u622a\u56fe\uff08\u6240\u6709\u5c4f\u5e55\uff09":"Capture all screens",
"\u5f53\u524d\u5c4f\u5e55\u622a\u56fe":"Capture current screen",
"\u5df2\u9009\u62e9\u5168\u5c4f\uff08\u6240\u6709\u5c4f\u5e55\uff09\uff0c\u4ecd\u53ef\u8c03\u6574\u548c\u6807\u6ce8\u3002":"All screens selected; adjust or annotate here.",
"\u5df2\u9009\u62e9\u6307\u9488\u6240\u5728\u5c4f\u5e55\uff0c\u4ecd\u53ef\u8c03\u6574\u548c\u6807\u6ce8\u3002":"Pointer screen selected; adjust or annotate here."
});

Object.assign(english,{
"\u72ec\u7acb\u53d6\u8272":"Pick a color",
"\u72ec\u7acb\u53d6\u8272\u00b7\u5355\u51fb\u6216 Enter \u590d\u5236 HEX\uff0c\u6210\u529f\u540e\u79bb\u5f00\uff1bEsc \u53d6\u6d88\u3002":"Pick a color: click or Enter copies HEX and exits on success; Esc cancels.",
"\u6a21\u62df\u590d\u5236\u5931\u8d25":"Simulate copy failure",
"\u590d\u5236\u5931\u8d25\uff0c\u5f53\u524d\u53d6\u8272\u5df2\u4fdd\u7559\uff0c\u53ef\u91cd\u8bd5\u3002":"Copy failed. Keep the current sample and retry."
});

Object.assign(english,{"\u51bb\u7ed3\u539f\u56fe\u53d6\u8272\uff0c\u4e0d\u751f\u6210\u56fe\u7247\u6216\u5386\u53f2\u3002":"Sample frozen pixels; no image output or history."});

Object.assign(english,{"100% \u5bf9\u5e94 96 DPI\uff1b\u8ddf\u968f\u7cfb\u7edf\u4f7f\u7528\u6240\u5728\u5c4f\u5e55\u7684 DPI\u3002\u53ea\u7f29\u653e\u754c\u9762\uff0c\u4e0d\u6539\u53d8\u622a\u56fe\u5bfc\u51fa\u5c3a\u5bf8\u6216\u8d34\u56fe\u56fe\u50cf\u7f29\u653e\u3002": "100% corresponds to 96 DPI; Follow system uses the current display DPI. Only controls scale; export dimensions and pin image zoom stay independent.", "HTML \u63a7\u4ef6\u9884\u89c8\u6f14\u793a\u7f29\u653e\u3002\u539f\u751f\u8bbe\u7f6e\u5373\u65f6\u9884\u89c8\uff0c\u53d6\u6d88\u6062\u590d\uff1b\u4fdd\u5b58\u540e\u7528\u4e8e\u4e0b\u4e00\u6b21\u622a\u56fe\u3001\u65b0\u8d34\u56fe\u4e0e\u53cd\u9988\u3002\u5df2\u6709\u8d34\u56fe\u4fdd\u6301\u5f53\u524d\u663e\u793a\u3002\u51cf\u5c11\u52a8\u6548\u4ecd\u5c5e\u540e\u7eed\u63a5\u5165\u3002": "HTML demonstrates scaling in the control preview. Native settings preview immediately; Cancel restores the saved scale. Save applies to the next capture, new pins and feedback. Existing pins keep their presentation. Reduced motion is still planned."});

Object.assign(english,{"\u5f00\u542f\u540e\u63a7\u4ef6\u60ac\u505c\u4e0e\u72b6\u6001\u989c\u8272\u7acb\u5373\u5207\u6362\uff1b\u4e0d\u5f71\u54cd\u9009\u533a\u62d6\u52a8\u3001\u7ed8\u5236\u6216\u5012\u8ba1\u65f6\u3002": "Switch hover and state colors immediately. Selection dragging, drawing and countdown timing stay unchanged."});

Object.assign(english,{"HTML \u63a7\u4ef6\u9884\u89c8\u6f14\u793a\u7f29\u653e\u3002\u539f\u751f\u8bbe\u7f6e\u5373\u65f6\u9884\u89c8\uff0c\u53d6\u6d88\u6062\u590d\uff1b\u4fdd\u5b58\u540e\u7528\u4e8e\u4e0b\u4e00\u6b21\u622a\u56fe\u3001\u65b0\u8d34\u56fe\u4e0e\u53cd\u9988\u3002\u5df2\u6709\u8d34\u56fe\u4fdd\u6301\u5f53\u524d\u663e\u793a\u3002\u51cf\u5c11\u52a8\u6548\u540c\u6837\u652f\u6301\u8349\u7a3f\u9884\u89c8\uff1b\u5df2\u6709\u8d34\u56fe\u4fdd\u6301\u539f\u6709\u504f\u597d\u3002": "HTML demonstrates scaling in the control preview. Native settings preview immediately; Cancel restores saved preferences. Save applies to the next capture, new pins and feedback. Existing pins keep their presentation, including motion preferences."});

Object.assign(english,{"\u5168\u5c40\u70ed\u952e\u670d\u52a1\u5df2\u505c\u6b62\u3002\u8bf7\u4ece\u6258\u76d8\u64cd\u4f5c\u5e76\u91cd\u542f\u5e94\u7528\u3002": "Global shortcuts have stopped. Use the tray and restart the app."});

Object.assign(english,{"Windows：输入完整程序文件名（如 password-manager.exe），用英文分号、逗号或换行分隔，最多 128 项；不接受路径或通配符。保存后，桌面采集前后检查可见窗口（含被遮挡窗口）；命中或检查失败则拒绝整次截图。最小化或关闭目标窗口后重试。两次检查不能覆盖间隙出现的内容，不是持续隐私保护；不影响本地图片或贴图再编辑。": "Windows: enter full executable names (for example password-manager.exe), separated by semicolons, commas or newlines; up to 128 entries. Paths and wildcards are not accepted. After Save, visible windows (including covered windows) are checked before and after desktop capture. A match or failed check rejects the whole capture. Minimize or close the target window and retry. Two checks cannot cover content appearing between them; this is not continuous privacy protection. Local images and pin editing are unaffected.", "原型检查场景（不读取真实进程）：": "Prototype check scenario (does not read real processes):", "模拟无命中": "Simulate no match", "模拟命中排除程序": "Simulate excluded application", "模拟检查失败": "Simulate check failure", "排除列表格式无效": "Invalid exclusion list", "截图已阻止": "Capture blocked", "请输入完整 .exe 文件名，用英文分号、逗号或换行分隔；不接受路径、通配符或超过 128 项。保存未生效，草稿已保留。": "Enter full .exe names separated by semicolons, commas or newlines; no paths, wildcards or more than 128 entries. Nothing was saved. Your draft is preserved.", "已保存的排除列表格式无效。请修正或清空列表并保存后重试。": "The saved exclusion list is invalid. Correct or clear it, save, then retry.", "检测到排除程序的可见窗口，已拒绝整次截图。最小化或关闭目标窗口后重试，或修改列表并保存。": "A visible window belongs to an excluded application. The whole capture was rejected. Minimize or close the target window and retry, or edit the list and save.", "无法完成排除程序检查，已拒绝整次截图。请重试，或修改列表并保存；不要依赖此检查持续保护内容。": "The exclusion check could not finish. The whole capture was rejected. Retry, or edit the list and save; do not rely on these checks for continuous protection."});

Object.assign(english,{"选择标注文字使用的字体。": "Choose a font for annotation text.", "留空使用系统图片目录下的 Glint。": "Empty uses Glint in the system Pictures folder.", "以下功能暂不可用。": "These features are currently unavailable.", "暂不可用": "Unavailable", "将滚动内容拼接为一张长图。": "Stitch scrolling content into a long image.", "从图片中识别并复制文字。": "Recognize and copy text from images.", "录制屏幕，导出视频或 GIF。": "Record the screen and export video or GIF.", "把屏幕上的一刻留下。": "Keep a moment from your screen.", "拾光 · Glint 0.0.0": "Glint 0.0.0", "截图、标注与桌面贴图。": "Screen capture, annotation and desktop pins.", "导出配置": "Export configuration", "导入配置": "Import configuration"});

Object.assign(english,{"按下组合键，确认后保存设置。Esc 取消。":"Press a shortcut, confirm, then save settings. Esc cancels."});

Object.assign(english,{"布局":"Layout","网格":"Grid","单行滚动":"Horizontal scrolling","固定行宽":"Fixed row width","列数":"Columns","行数":"Rows","行宽(px)":"Row width (px)"});

Object.assign(english,{"单行完整显示":"Single row (all tools)"});

Object.assign(english,{"选区、取消和完成入口始终保留，自定义可隐藏。":"Selection, cancel and finish remain available; Customize can be hidden."});

Object.assign(english,{"最大宽度(px)":"Maximum width (px)","输入最大宽度；实际宽度会按完整按钮向下对齐，避免露出半个图标。超出部分可横向滚动查看。":"Enter the maximum width. The toolbar rounds down to fit whole buttons, so icons are never cut in half. Scroll horizontally to see the remaining tools."});

Object.assign(english,{"应用界面默认使用系统字体，缺失时回退到内置 Noto Sans SC；批注字体单独设置。":"The app uses the system UI font, with bundled Noto Sans SC as fallback. Annotation fonts are configured separately."});

Object.assign(english,{"应用字体":"Application font","内置 Noto Sans SC":"Bundled Noto Sans SC","只影响应用界面，不影响批注文字。跟随系统时，缺失字体回退到内置 Noto Sans SC。":"Applies to the application UI only, not annotations. Follow system falls back to bundled Noto Sans SC when needed."});

Object.assign(english,{"保留此缩放比例？":"Keep this interface scale?","请确认界面是否清晰且易于操作。15 秒后未确认将自动还原；保留后仍需保存设置。":"Check that the interface is easy to use. Revert automatically after 15 seconds unless confirmed; Save settings after keeping the scale.","还原":"Revert","保留此比例":"Keep this scale"});
Object.assign(english,{
 '向下滚动左侧内容，再采集下一段；右侧持续更新拼接结果。':'Scroll the source, then capture the next fragment. The stitched result stays visible on the right.',
 '原型示意 · 不采集真实屏幕':'Prototype only · No desktop capture',
 '拼接结果预览':'Stitched preview','长图总览':'Overview','待确认接缝':'Proposed seam','拼接片段':'Fragment',
 '项目记录':'Project notes','内容与进度':'Content and progress',
 '此处仅示意长图内容；原生版展示实际采集并拼接的像素。':'Sample content only. The native app shows actual captured and stitched pixels.',
 '继续采集时，右侧预览随拼接结果增长，可滚动检查细节。':'The preview grows with each capture. Scroll to inspect details.'
});
Object.assign(english, {
  '部分内容未能拼接，当前展示已对齐部分。片段已保留，可继续采集或主动修复。': 'Some content could not be joined. Showing the aligned portion; source fragments are retained. Continue capturing or adjust the joins.',
  '部分内容未能拼接，片段已保留。可输出已对齐部分或主动修复。': 'Some content could not be joined. Source fragments are retained. Export the aligned portion or adjust the joins.',
  '收起修复': 'Hide adjustments',
  '调整拼接': 'Adjust joins',
  '编辑已对齐部分': 'Edit aligned portion',
  '保存已对齐部分': 'Save aligned portion',
  '复制已对齐部分': 'Copy aligned portion'
});
Object.assign(english, {
  '滚动截图（未完成）': 'Scrolling (incomplete)',
  '滚动长截图（未完成 · 严重 Bug）': 'Scrolling capture (incomplete; severe bugs)',
  '未完成 · 严重 Bug': 'Incomplete; severe bugs',
  '未完成 · 严重 Bug：选区及预览可能不显示，采集与拼接结果不可靠。': 'Incomplete; severe bugs. Selection and preview may be missing; capture and stitching are unreliable.'
});
