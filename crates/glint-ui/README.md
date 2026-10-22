# Glint UI

Glint 自有的原生 Slint 组件库，GPL-3.0-only。不引入第三方组件包。
基础 Fluent 控件来自项目原有的 Slint 1.18.1 修改副本，版权与素材许可随源码保留。

## 边界

- `ui/widgets/`：按钮、勾选、下拉、滑块、滚动区与减少动效适配。
- `ui/components/`：强调按钮、受控勾选与选择框、带标签勾选、设置项容器、行内快捷键录入、字体选择器、工具条编辑器。
- `ui/theme.slint`：每个顶层组件独立的主题快照及语义颜色、圆角。默认值沿用 HTML 原型。
- `ui/widgets/motion.slint`：字体、减少动效开关及 fast/normal 时长。关闭装饰动画时直接读取目标属性，避免仍在进行的插值。
- `src/i18n.rs`：语言协商；组件文案由属性、Labels 结构传入。应用文案 ID 与中英文目录留在 glint-app，不向组件库引入业务。
- `src/theme.rs`：纯主题选择与日内时间区间计算。调用方提供系统主题与本地时间，库内没有定时器、平台查询和配置读写。

组件仅保存焦点、展开、搜索等呈现状态，通过属性和 callback 对接应用。
截图采集、热键注册、字体枚举、配置草稿/保存、输出等副作用继续由 glint-app/glint-platform 负责。
glint-core 不依赖此库、Slint 或 Win32。

## 接入

Rust 调用方依赖 glint-ui 和与项目一致的 Slint，通过宏注册稳定的库名称：

```rust,ignore
glint_ui::slint! {
    export { Motion, Theme } from "@glint_ui/glint-ui.slint";
    export { MyWindow } from "src/my_window.slint";
}
```

Slint 页面使用统一入口：

```slint
import { Button, FormCard, Theme } from "@glint_ui/glint-ui.slint";
```

也可在使用 slint-build 的工程中将 `slint_library_paths()` 传给
`CompilerConfiguration::with_library_paths`。当前工程保留 inline 宏，不新增编译器依赖。
宏内库路径由组件 crate 在构建时解析，不依赖启动目录或硬编码用户路径。
该库的 Slint 源码及素材需与 crate 一起分发，不需要运行时读取源码目录。

每个顶层窗口需要独立配置 Motion（包括 find-index 回调）；应用现有
`motion::configure!` 保留这一职责。主题使用 `Theme.apply(dark, contrast, accent)`，
页面的 dark/contrast/accent 变化会同步到其自身全局，不改变截图像素。

## 后续扩展

自定义样式可以覆盖 Theme 的明暗语义颜色、圆角，以及 Motion 时长；当前不提供
CSS 加载器、样式文件导入或动画脚本系统。Fluent 基础控件内部细分状态色仍保留
原有色板，不宣称任意自定义配色已覆盖所有控件状态。

定时明暗切换仅准备 `ThemeSchedule` 的纯策略 API，尚无设置项、持久化或自动运行。
后续应先补 HTML 原型与配置迁移，再在应用层按下一边界调度单次唤醒，并处理
时区变化、用户修改时间及休眠恢复；不要让每个组件轮询时钟。

本次是组件和呈现策略拆分，尚未把整个应用协调器改成完整 MVI/MVVM。
后续页面拆分应使用有类型的状态和动作，避免把配置 key、平台对象和副作用搬入组件。
