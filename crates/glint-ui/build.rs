use std::{env, fs, path::PathBuf};

fn main() {
    let ui = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("crate directory")).join("ui");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    // Slint's inline library_path attribute accepts a literal and resolves it
    // against the compiler working directory. Embed the actual crate path so
    // consumers are independent of workspace layout and IDE working directory.
    fs::write(
        output.join("slint_macro.rs"),
        format!(
            r#"
#[macro_export]
macro_rules! slint {{
    ($($tokens:tt)*) => {{
        slint::slint! {{
            #[library_path(glint_ui) = {ui:?}]
            $($tokens)*
        }}
    }};
}}
"#,
            ui = ui.to_string_lossy(),
        ),
    )
    .expect("write Slint library macro");
}
