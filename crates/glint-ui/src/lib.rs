//! Shared native presentation components and policies.
//!
//! Slint sources live in `ui/`. This crate has no domain, platform or renderer
//! dependency; callers own windows, configuration, effects and translated copy.

pub mod i18n;
pub mod theme;

include!(concat!(env!("OUT_DIR"), "/slint_macro.rs"));

/// Library map accepted by `slint_build::CompilerConfiguration`.
/// Inline callers use `glint_ui::slint!` with their own Slint dependency.
pub fn slint_library_paths() -> std::collections::HashMap<String, std::path::PathBuf> {
    [(
        "glint_ui".into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"),
    )]
    .into()
}
