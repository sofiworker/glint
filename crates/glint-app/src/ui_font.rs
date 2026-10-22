//! UI typography only. Annotation defaults and GDI text export remain independent.
use glint_core::config::UiFont;
use std::{
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

static USE_BUNDLED: AtomicBool = AtomicBool::new(false);

pub fn set_saved_choice(choice: UiFont) {
    USE_BUNDLED.store(choice == UiFont::NotoSansSc, Ordering::Relaxed);
}

pub fn selected(choice: UiFont) -> &'static str {
    match choice {
        UiFont::System => system_family(),
        UiFont::NotoSansSc => "Noto Sans SC",
    }
}

const FALLBACK: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-VF.ttf");
static SYSTEM_FAMILY: OnceLock<String> = OnceLock::new();

pub fn family() -> &'static str {
    if USE_BUNDLED.load(Ordering::Relaxed) {
        "Noto Sans SC"
    } else {
        system_family()
    }
}

fn system_family() -> &'static str {
    SYSTEM_FAMILY.get_or_init(|| {
        glint_platform::system_ui_font_family()
            .ok()
            .filter(|family| {
                glint_platform::installed_font_families()
                    .is_ok_and(|fonts| fonts.iter().any(|name| name.eq_ignore_ascii_case(family)))
            })
            .unwrap_or_else(|| "Noto Sans SC".into())
    })
}

/// Must run at process startup, before Slint, tray or worker threads exist.
/// Slint 1.18 uses SLINT_FONT_PATH to append embedded assets to its generic
/// fallback chain; register_font_from_memory alone only registers a named face.
pub fn initialize() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let directory = base.join("Glint").join("font-cache");
    std::fs::create_dir_all(&directory)?;
    // Content-derived filename lets different app versions coexist safely.
    let path = directory.join("NotoSansSC-a3041811a78c361b.ttf");
    if std::fs::read(&path).as_deref().ok() != Some(FALLBACK) {
        // Publish complete bytes atomically; never point the renderer at a partial font.
        let temporary = directory.join(format!("font-{}.tmp", std::process::id()));
        std::fs::write(&temporary, FALLBACK)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        std::fs::rename(&temporary, &path)?;
    }
    let mut paths = vec![path];
    if let Some(existing) = std::env::var_os("SLINT_FONT_PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let paths = std::env::join_paths(paths)?;
    // SAFETY: called at the start of run(), before backend initialization or
    // application thread creation. No concurrent environment readers exist yet.
    unsafe {
        std::env::set_var("SLINT_FONT_PATH", paths);
    }
    let _ = family();
    Ok(())
}
