# Glint action icons

These 51 Lucide SVGs share the pinned upstream source and semantic mapping in `assets/icons/lucide/manifest.json` (lucide-static 1.77.0). They use Lucide's 24-unit viewBox, 2-unit strokes, round caps and round joins. UI components color them with their semantic theme colors; normal controls display them at 18px.

After adding an upstream SVG and its checksum/mapping to the manifest, run `python tools/sync_icons.py` to generate `prototype/icons.js`, these SVGs and `../components/icons.slint`. Run `python tools/sync_icons.py --check` to verify source checksums and detect stale generated assets. The native application embeds these resources at compile time; it does not fetch assets at runtime or depend on the working directory.

Use named `GlintIcons` properties in new controls. `ToolbarAssets` in `toolbar_editor.slint` retains the existing 17 toolbar indices so that customization, ordering and actions keep their current mapping.

Lucide artwork retains its upstream ISC license and Feather-derived artwork retains MIT terms. Full copyright and license texts are in `assets/icons/lucide/LICENSE.txt`; see also the root THIRD-PARTY-NOTICES.md. The same named assets supply checkbox marks, dropdowns, numeric inputs and scrollbar arrows. The historical Microsoft SVG files in `../widgets/fluent/` remain with their original MIT notices for provenance and are no longer referenced by Glint controls.

Do not edit generated SVGs, `prototype/icons.js` or `icons.slint` directly. Preserve semantic icon names and toolbar indices. Use Lucide for new UI icons; do not add hand-drawn action icons or Unicode glyph fallbacks.
