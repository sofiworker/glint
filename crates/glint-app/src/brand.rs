//! Icons generated from the project's original SVG artwork during the build.
use slint::winit_030::winit;

pub fn window_icon() -> winit::window::Icon {
    let png = include_bytes!(concat!(env!("OUT_DIR"), "/glint-64.png"));
    let image = image::load_from_memory(png)
        .expect("Build-generated window icon")
        .into_rgba8();
    winit::window::Icon::from_rgba(image.into_raw(), 64, 64).expect("Window icon dimensions")
}

pub fn tray_icon() -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
    let png = include_bytes!(concat!(env!("OUT_DIR"), "/glint-20.png"));
    let image = image::load_from_memory(png)?.into_rgba8();
    Ok(tray_icon::Icon::from_rgba(image.into_raw(), 20, 20)?)
}
