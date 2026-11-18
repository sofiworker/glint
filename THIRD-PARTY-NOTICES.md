# 第三方许可说明

Glint 自有代码与 HTML 原型采用 `GPL-3.0-only`，全文见 [LICENSE](LICENSE)。第三方代码、依赖与素材保留各自版权和许可，本项目的许可证不替代其许可证。

## Slint

- 使用版本：1.18.1，以 Cargo.lock 和实际构建依赖为准。
- 上游：https://github.com/slint-ui/slint
- 对应版本许可：https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md
- Slint 框架提供 `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` 三选一许可。
- Glint 对 Slint 框架选择 **GPL-3.0-only** 路径；本项目的 GPL 文本取自该版本上游的 `LICENSES/GPL-3.0-only.txt`。
- Slint 文档、示例和第三方资产可能有独立许可，以各文件声明及上游 REUSE 元数据为准；复制这些内容时保留相应声明。
- `crates/glint-ui/ui/widgets/` 包含 Slint v1.18.1 Fluent 控件的有限修改副本（按钮、复选框、下拉框、滑块、滚动区及公共基础），保留原版权和三选一 SPDX 声明，本项目选择 GPL-3.0-only。修改用于按窗口控制动效，并以公开接口替换内部引用；来源、修改说明与升级复核要求见该目录 README。
- 该目录 Fluent SVG 图标来自上游同版本的 `internal/compiler/widgets/fluent/`；上游 REUSE 声明为 Microsoft Corporation 版权所有、MIT 许可。图标保持原样，随附逐文件 `.license` 声明、MIT 全文 `LICENSE-MIT-Fluent-icons.txt` 及上游 REUSE 元数据。

## 项目图标与字体

- `crates/glint-ui/ui/icons/*.svg` 是本项目原创操作图标，由 `prototype/icons.js` 中的共享路径生成，按项目自有代码的 `GPL-3.0-only` 分发；不属于上述 Microsoft Fluent 图标集合，不依赖 Lucide 或字体字符。原型路径与原生资源的对应关系及生成命令见该目录 README。
- `assets/brand/*.svg` 是项目原创标志，采用 GPL-3.0-only。构建时从 SVG 生成托盘、窗口及 Windows 可执行文件的图标，不含第三方图标素材。HTML 示例桌面由原型代码绘制，不能作为原生截图资源。
- 应用界面优先使用用户的系统界面字体；系统字体仅调用，不复制分发。批注字体与界面字体策略独立。
- 内置回退字体：Noto Sans SC Variable，Copyright 2014–2021 Adobe，Reserved Font Name “Source”；采用 SIL Open Font License 1.1（OFL-1.1），允许随 GPL 应用嵌入与再分发，字体本身继续遵守 OFL。未修改上游字体。原始许可与版权：`assets/fonts/OFL-NotoSansSC.txt`；来源、固定提交及 SHA-256：`assets/fonts/provenance.json`。

## 原生程序的直接依赖

下表按当前锁定版本的 Cargo 包元数据核对；选用 Apache-2.0 / MIT 等路径与项目 GPLv3 组合，仍须保留各依赖版权和许可原文。Windows 开发包中的完整传递依赖清单由 `tools/package_windows.py` 按锁文件、实际 normal/build 图和 vendored 来源生成。

| 依赖 | 版本 | 上游 | 许可表达式 |
| --- | --- | --- | --- |
| image | 0.25.10 | https://github.com/image-rs/image | MIT OR Apache-2.0 |
| tiny-skia | 0.12.0 | https://github.com/linebender/tiny-skia | BSD-3-Clause |
| serde | 1.0.229 | https://github.com/serde-rs/serde | MIT OR Apache-2.0 |
| toml | 1.1.8+spec-1.1.0 | https://github.com/toml-rs/toml | MIT OR Apache-2.0 |
| arboard | 3.7.0 | https://github.com/1Password/arboard | MIT OR Apache-2.0 |
| windows | 0.58.0 | https://github.com/microsoft/windows-rs | MIT OR Apache-2.0 |
| tray-icon | 0.19.3 | https://github.com/tauri-apps/tray-icon | MIT OR Apache-2.0 |
| raw-window-handle | 0.6.2 | https://github.com/rust-windowing/raw-window-handle | MIT OR Apache-2.0 OR Zlib |
| tracing | 0.1.44 | https://github.com/tokio-rs/tracing | MIT |
| tracing-subscriber | 0.3.23 | https://github.com/tokio-rs/tracing | MIT |
| file-rotate | 0.8.0 | https://github.com/kstrafe/file-rotate | MIT |
| windows-capture | 2.0.1 | https://github.com/NiiightmareXD/windows-capture | MIT |
| resvg（构建依赖） | 0.48.1 | https://github.com/linebender/resvg | Apache-2.0 OR MIT |
| winresource（构建依赖） | 0.1.31 | https://github.com/BenjaminRi/winresource | MIT |

tray-icon 在 M0 贴图探针中提供实际系统托盘菜单，raw-window-handle 用于借用 live HWND 并调用平台封装验证置顶、穿透与救援。Slint 的 `unstable-winit-030` 接口用于窗口属性和生命周期衔接；Slint 固定精确版本，后续升级须重新检查该接口。

## 分发要求

分发 Glint 可执行文件时，按 GPLv3 提供对应源码及构建、安装所需的相关材料，保留版权、许可及无担保声明。收费分发仍需履行这些义务。不能仅添加本许可证便将采用本许可路径的完整应用闭源分发。

本文件记录当前采用的 Slint 许可路径，**不是全部传递依赖的许可清单**。Windows 开发包包含 `DEPENDENCIES.md` / `DEPENDENCIES.json`、原始许可与声明、Rust 标准库版权材料及 `corresponding-source.zip`。源码包保留 Cargo 标准完整 vendoring、校验文件、项目源代码、构建步骤与实际编译器版本；额外的其他平台源码不代表已实现或支持那些平台。`licenses/provenance.json` 对补充上游条款和 SPDX 参考原文记录来源与 SHA-256，打包时校验；参考条款不能替代原始版权归属，完整原始源码也随包提供。

这些材料不等于 M5 发布许可审查或环境验收已完成。发布前仍须复核实际所分发素材、编译器/系统运行库、上游附加说明与依赖变更；本机构建包不包含从系统目录复制的 Windows / Visual C++ DLL。后续引入字体、图标、OCR 模型和编解码器时同样检查与保留对应条款。

Windows 录屏使用 windows-capture 的 Windows Graphics Capture 事件后端；未使用该库的编码器（内部无界通道）。Glint 的有界消费者直接调用系统 Media Foundation H.264/AAC 编码，音频调用 WASAPI。windows-capture 的 rayon/crossbeam 传递依赖采用 MIT OR Apache-2.0；Win32/WinRT 分别使用 windows 0.58/0.62 绑定，系统媒体组件不随包复制。

工作台通过同一系统 Media Foundation 路径按实际可用性调用 H.264/AVC 或 H.265/HEVC 与 AAC；不捆绑系统编码器或扩展包；基础开发包不自动包含 FFmpeg。`image` 固定为 0.25.10，额外启用 BMP/ICO/WebP/TIFF。锁文件中的 image-webp 0.2.4（MIT OR Apache-2.0）、tiff 0.11.3（MIT）、fax 0.2.7（MIT）、half 2.7.1（MIT OR Apache-2.0）和 quick-error 2.0.1（MIT OR Apache-2.0）为纯 Rust 传递依赖，采用与 GPLv3 兼容的许可路径；原许可文件随对应源码和实际分发依赖清单保留。此处记录依赖许可，不代表完整发布审查或特定设备编码可用性承诺。

MOV/MKV/WebM 的可选视频格式组件通过独立 FFmpeg 进程调用。`tools/setup_media.py` 固定 Gyan FFmpeg 9.0.2 essentials 构建及 SHA-256，只为本机安装组件、LICENSE、README 与来源摘要，不执行系统安装。该构建声明 GPL v3（FFmpeg 配置为 `--enable-gpl --enable-version3`，采用 GPL-3.0-or-later 路径与项目 GPL-3.0-only 组合），包含 libvpx/libopus 等；来源为 https://www.gyan.dev/ffmpeg/builds/ ，FFmpeg 源码提交 https://github.com/FFmpeg/FFmpeg/commit/946fcce07b 。本次未新增 Rust crate。此安装脚本不是第三方二进制再分发包：`tools/package_windows.py` 默认不纳入该下载组件；若分发者另行打包该构建，须提供该构建及所含库的完整对应源码、构建材料与许可证，不能只附下载链接或摘要代替。
