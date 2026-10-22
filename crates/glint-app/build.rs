use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=../../assets/brand/glint-app.svg");
    println!("cargo:rerun-if-changed=../../assets/brand/glint-small.svg");
    if env::var_os("CARGO_FEATURE_NATIVE_SHELL").is_none() {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let app = fs::read("../../assets/brand/glint-app.svg").expect("Application SVG");
    let small = fs::read("../../assets/brand/glint-small.svg").expect("Small SVG");
    let mut entries = Vec::new();
    for size in [16u32, 20, 24, 32, 48, 64, 128, 256] {
        let svg = if size <= 20 { &small } else { &app };
        let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default())
            .expect("Valid brand SVG");
        let mut pixels = resvg::tiny_skia::Pixmap::new(size, size).expect("Icon pixel buffer");
        let scale = size as f32 / tree.size().width();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixels.as_mut(),
        );
        let png = pixels.encode_png().expect("Encode application icon");
        fs::write(output.join(format!("glint-{size}.png")), &png).expect("Write icon PNG");
        entries.push((size, png));
    }
    // ICO directory followed by one PNG image per size. 0 encodes a 256px side.
    let mut ico = vec![0, 0, 1, 0];
    ico.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * entries.len() as u32;
    for (size, png) in &entries {
        ico.extend_from_slice(&[*size as u8, *size as u8, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in entries {
        ico.extend_from_slice(&png);
    }
    let icon = output.join("glint.ico");
    fs::write(&icon, ico).expect("Write Windows icon");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon(icon.to_str().expect("Icon path"))
            .set("ProductName", "Glint")
            .set("FileDescription", "Glint - capture and pin")
            .set("LegalCopyright", "Glint contributors; GPL-3.0-only")
            .compile()
            .expect("Embed Windows application icon");
    }
}
