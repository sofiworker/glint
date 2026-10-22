# Toolbar icons

These 17 SVG files contain the same named path data as `iconPaths` in the project's `prototype/toolbar.js`. The native toolbar embeds them at compile time through `toolbar_editor.slint`; it does not fetch icon assets at runtime or depend on the working directory.

They follow the project's GPL-3.0-only license in the repository root. This directory is distinct from `../widgets/fluent/`, whose upstream Microsoft SVGs retain their individual MIT license notices. Keep both sets of notices with corresponding source; see the root THIRD-PARTY-NOTICES.md.

When editing a toolbar icon, update the HTML prototype first, then the corresponding native SVG path. Preserve the icon name and check the path mapping before packaging.
