# Lucide UI icons

Official local SVG subset from `lucide-static` **1.77.0**. Browse icons at
https://lucide.dev/icons/categories . Original SVG bytes and `LICENSE.txt`
come from the npm package recorded in `manifest.json`; its SHA-512 integrity
was verified before importing. Per-file SHA-256 hashes pin each SVG and the
license. Keep the full ISC and Feather MIT copyright/license texts with copies.

`manifest.json` maps Glint semantic names to upstream filenames. Existing toolbar
indices remain stable. Its aliases preserve the prototype's previous names.
`python tools/sync_icons.py` generates the prototype registry first, followed by
the native SVGs and Slint registry. `python tools/sync_icons.py --check` validates
checksums and generated outputs without network access. Generated SVGs keep the
official geometry and 2-unit stroke; only native stroke color is replaced for
theme colorization. The application embeds its SVGs at compile time.

To add an icon, copy the official SVG from this exact package, record its hash
and semantic mapping, then regenerate and check the prototype and native UI.
Upgrading Lucide requires reviewing geometry, aliases and license changes and
updating the package version/integrity and all affected file hashes together.
Do not mix other icon sets or draw replacements for available Lucide icons.
