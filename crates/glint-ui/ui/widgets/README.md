# Motion-aware Fluent subset

Derived from Slint v1.18.1 internal/compiler/widgets, matching the exact Cargo pin. Fluent Button, CheckBox, ComboBox, Slider, ScrollView and shared components/styling, plus common ComboBoxBase/SliderBase, retain their upstream copyright and SPDX statements. Glint selects GPL-3.0-only; full text is ../../../../LICENSE. See the repository THIRD-PARTY-NOTICES.md. The unchanged Fluent SVGs are Copyright (c) 2020 Microsoft Corporation, MIT; see LICENSE-MIT-Fluent-icons.txt and individual .license files. UPSTREAM-REUSE.toml preserves the upstream asset declaration.

Glint modifications (2026-10-19): animation durations use the component-local Motion.reduced policy; visual properties bypass lazy animated proxies when reduced, stopping visible in-flight interpolation immediately (duration changes alone do not stop running Slint animations); unused animations with no local binding were removed; public standard Palette supplies color scheme and accent in place of SlintInternal; the ComboBox array lookup is a pure Rust callback retaining first-match/unknown-value semantics; ScrollView removes experimental @shadowable/@deprecated annotations while retaining content/viewport aliases. Glint uses those aliases without declaring shadows. LineEdit and layout boxes remain upstream standard widgets because they have no decorative property animations in this version. Text caret, user-driven scrolling, selection and drag rendering remain functional.

No experimental compiler flag, registry patch, custom runtime, background timer or dependency added. Configure the exported Motion global on each top-level component before use; this installs the lookup callback and presentation snapshot. Globals are per top-level component, not process-wide. On Slint upgrades, re-audit upstream diffs, public palette equivalence, new animations, controls/accessibility and licensing; run focused motion frames plus the full resident regression.

Upstream sources: https://github.com/slint-ui/slint/tree/v1.18.1/internal/compiler/widgets
Asset license declaration: https://github.com/slint-ui/slint/blob/v1.18.1/REUSE.toml

Component extraction (2026-10-22): moved unchanged assets and licensing into glint-ui; Motion.fast/normal now supply animation durations without changing defaults or reduced-motion bypass behavior.
