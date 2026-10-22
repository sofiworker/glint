# 拾光 · Glint 标志

原创图形，以截图四角和一束暖色光表达“拾光”。许可为 GPL-3.0-only，
与项目自有素材一致，不含第三方素材、外部字体或链接资源。

| 文件 | 用途 |
| --- | --- |
| glint.svg | 透明底彩色主标志，建议 24px 及以上 |
| glint-app.svg | 带靛蓝圆角底的应用图标 |
| glint-small.svg | 为 16px / 20px 简化的图标，采用 16 单位坐标 |
| glint-mono.svg | 单色图形，默认黑色；内联 SVG 时可通过 CSS color 设置颜色 |
| preview.html | 浅色、深色及多个实际尺寸的静态预览 |

所有 SVG 都有 viewBox，可指定任意显示宽高。使用时保持正方形比例。
主色 #4B5CFA，光点 #F4AD55；应用图标内的光点使用 #FFD493。
SVG 是源素材。启用 native-shell 构建时，glint-app/build.rs 使用 resvg
生成 16/20/24/32/48/64/128/256px PNG 和多尺寸 ICO；Windows 构建通过
winresource 将 ICO 嵌入可执行文件。托盘使用 20px 小尺寸图形，窗口
使用应用图标。生成文件位于 Cargo OUT_DIR，无需运行时读取外部素材。
