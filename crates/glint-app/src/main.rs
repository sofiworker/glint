#![cfg_attr(
    all(windows, feature = "native-shell", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(all(feature = "native-shell", windows))]
mod appearance;
#[cfg(all(feature = "native-shell", windows))]
mod brand;
#[cfg(all(feature = "native-shell", windows))]
mod capture_delay;
#[cfg(all(feature = "native-shell", windows))]
mod capture_screen;
#[cfg(all(feature = "native-shell", windows))]
mod clipboard_pin;
#[cfg(all(feature = "native-shell", windows))]
mod completion;
#[cfg(all(feature = "native-shell", windows))]
mod config_transfer;
#[cfg(all(feature = "native-shell", windows))]
mod diagnostic_log;
#[cfg(all(feature = "native-shell", windows))]
mod environment_probe;
#[cfg(all(feature = "native-shell", windows))]
mod font_picker;
#[cfg(all(feature = "input-probe", windows))]
mod input_smoke;
#[cfg(all(feature = "native-shell", windows))]
mod interface_scale;
#[cfg(all(feature = "native-shell", windows))]
mod lifecycle;
#[cfg(all(feature = "native-shell", windows))]
mod local_image;
#[cfg(all(feature = "native-shell", windows))]
mod messages;
#[cfg(all(feature = "native-shell", windows))]
mod motion;
#[cfg(all(feature = "native-shell", windows))]
mod onboarding;
#[cfg(all(feature = "native-shell", windows))]
mod output_settings;
#[cfg(all(feature = "native-shell", windows))]
mod pin_edit;
#[cfg(all(feature = "native-shell", windows))]
mod pin_output;
#[cfg(all(feature = "native-shell", windows))]
mod pin_probe;
#[cfg(all(feature = "native-shell", windows))]
mod pin_session;
#[cfg(all(feature = "native-shell", windows))]
mod pin_session_smoke;
#[cfg(all(feature = "native-shell", windows))]
mod pins;
#[cfg(all(feature = "native-shell", windows))]
mod preferences;
#[cfg(feature = "native-shell")]
mod probe;
#[cfg(all(feature = "native-shell", windows))]
mod recording;
#[cfg(all(feature = "native-shell", windows))]
mod resident;
#[cfg(all(feature = "native-shell", windows))]
mod resident_smoke;
#[cfg(all(feature = "native-shell", windows))]
mod resource_smoke;
#[cfg(all(feature = "native-shell", windows))]
mod runtime_diagnostics;
#[cfg(all(feature = "native-shell", windows))]
mod scale_preview;
#[cfg(all(feature = "native-shell", windows))]
mod scroll_capture;
#[cfg(all(feature = "native-shell", windows))]
mod settings;
#[cfg(all(feature = "native-shell", windows))]
mod settings_policy_smoke;
#[cfg(all(feature = "native-shell", windows))]
mod toolbar;
#[cfg(all(feature = "native-shell", windows))]
mod ui_font;

fn main() {
    #[cfg(feature = "native-shell")]
    if let Err(error) = run() {
        glint_platform::diagnostic_log::emit(glint_platform::diagnostic_log::Event::StartupFailed);
        eprintln!("Glint: {error}");
        #[cfg(windows)]
        if !std::env::args().any(|arg| {
            arg.starts_with("--m0-")
                || arg.starts_with("--m1-")
                || arg.starts_with("--m4-")
                || arg == "--m5-resources"
                || arg == "--m5-resources-4k"
                || arg == "--m5-endurance"
                || arg == "--m5-input-smoke"
                || arg == "--m5-dialog-input-smoke"
                || arg == "--m5-dialog-conflict-input-smoke"
                || arg == "--pin-session-probe"
        }) {
            glint_platform::show_startup_error(&error.to_string());
        }
        std::process::exit(1);
    }
    #[cfg(not(feature = "native-shell"))]
    println!(
        "拾光 · Glint：使用 cargo run -p glint-app --features native-shell 启动原生常驻与设置。"
    );
}

#[cfg(feature = "native-shell")]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    ui_font::initialize()?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(windows)]
    if !args.first().is_some_and(|arg| arg.starts_with("--m0-")) {
        return resident::run(&args);
    }
    probe::run()
}
