use std::{env, fs, path::PathBuf};

fn main() {
    let ui = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("crate directory")).join("ui");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let (light, dark, light_accent, dark_accent) = match env::var("CARGO_CFG_TARGET_OS").as_deref()
    {
        Ok("macos") => (
            ["f5f5f7", "ffffff", "eaeaec", "1d1d1f", "606066", "dcdcde"],
            ["1e1e20", "2d2d30", "262628", "f5f5f7", "b6b6bd", "454549"],
            0x0065d1,
            0x69aaff,
        ),
        Ok("linux") => (
            ["fafafa", "ffffff", "ebebeb", "242424", "595959", "dedede"],
            ["242424", "303030", "282828", "f5f5f5", "b7b7b7", "454545"],
            0x1c71d8,
            0x78aeff,
        ),
        _ => (
            ["f6f7f9", "ffffff", "eceff3", "202124", "5c616b", "d9dde4"],
            ["202124", "292b2f", "24262a", "f1f2f4", "b0b5be", "41454d"],
            0x0067c0,
            0x75b8ff,
        ),
    };
    let mut palette = String::from("export global PlatformTheme {\n");
    for (mode, colors) in [("light", light), ("dark", dark)] {
        for (role, color) in ["background", "surface", "sidebar", "ink", "muted", "line"]
            .into_iter()
            .zip(colors)
        {
            palette.push_str(&format!(
                "    out property <color> {mode}-{role}: #{color};\n"
            ));
        }
    }
    palette.push_str("}\n");
    let platform_theme = output.join("platform_theme.slint");
    fs::write(&platform_theme, palette).expect("write platform theme");
    // Slint's inline library_path attribute accepts a literal and resolves it
    // against the compiler working directory. Embed the actual crate path so
    // consumers are independent of workspace layout and IDE working directory.
    fs::write(
        output.join("slint_macro.rs"),
        format!(
            r#"
pub const LIGHT_ACCENT: u32 = {light_accent};
pub const DARK_ACCENT: u32 = {dark_accent};
#[macro_export]
macro_rules! slint {{
    ($($tokens:tt)*) => {{
        slint::slint! {{
            #[library_path(glint_ui) = {ui:?}]
            #[library_path(glint_platform_theme) = {platform_theme:?}]
            $($tokens)*
        }}
    }};
}}
"#,
            ui = ui.to_string_lossy(),
            platform_theme = platform_theme.to_string_lossy(),
        ),
    )
    .expect("write Slint library macro");
}
