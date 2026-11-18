# Glint action icons

These 44 project-owned SVGs share one geometry source in `prototype/icons.js`. They use a 24-unit viewBox, 1.8-unit strokes, round caps and round joins. UI components color them with their semantic theme colors; normal controls display them at 18px.

After editing the HTML prototype's icon source, run `python tools/sync_icons.py` to generate these SVGs and `../components/icons.slint`. Run `python tools/sync_icons.py --check` to detect stale native assets. The native application embeds these resources at compile time; it does not fetch assets at runtime or depend on the working directory.

Use named `GlintIcons` properties in new controls. `ToolbarAssets` in `toolbar_editor.slint` retains the existing 17 toolbar indices so that customization, ordering and actions keep their current mapping.

They follow the project's GPL-3.0-only license in the repository root. This directory is distinct from `../widgets/fluent/`, whose upstream Microsoft SVGs retain their individual MIT license notices. Keep both sets of notices with corresponding source; see the root THIRD-PARTY-NOTICES.md.

Do not edit generated SVGs or `icons.slint` directly. Preserve icon names and check the path mapping before packaging. These icons do not require Lucide or a Unicode glyph fallback.
