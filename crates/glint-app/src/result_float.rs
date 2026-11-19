//! One short-lived capture result. No history files and no focus activation.
use crate::workbench::{CaptureResultWindow, Document, Presentation};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, time::Duration};

pub struct Pending {
    pub document: Document,
    pub thumbnail: image::RgbaImage,
}
struct Float {
    ui: CaptureResultWindow,
    _document: Document,
    _timer: slint::Timer,
}
thread_local! {
    static FLOAT:RefCell<Option<Float>>=const{RefCell::new(None)};
}
pub fn prepare_document(document: Document) -> Result<Pending, String> {
    let image_document = &document.image;
    let (w, h) = image_document.dimensions();
    if u64::from(w) * u64::from(h) > glint_core::workbench::MAX_PIXELS {
        return Err("Image exceeds editor pixel budget".into());
    }
    let pixels = glint_core::composite(
        image_document.source(),
        w,
        h,
        glint_core::Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        },
        image_document.edits(),
    )
    .map_err(str::to_owned)?;
    let image = image::RgbaImage::from_raw(w, h, pixels).ok_or("Invalid capture thumbnail")?;
    Ok(Pending {
        document,
        thumbnail: image::imageops::thumbnail(&image, 440, 228),
    })
}
pub fn dismiss() {
    FLOAT.with_borrow_mut(|f| {
        if let Some(f) = f.take() {
            let _ = f.ui.hide();
        }
    });
}
pub fn show(result: Pending, presentation: Presentation, saved: bool) -> Result<(), String> {
    dismiss();
    let ui = crate::motion::create_settings(CaptureResultWindow::new).map_err(|e| e.to_string())?;
    ui.set_dark(presentation.dark);
    ui.set_contrast_mode(presentation.contrast);
    ui.set_accent(presentation.accent);
    ui.set_ui_font_family(crate::ui_font::family().into());
    ui.set_interface_scale(crate::interface_scale::factor(presentation.scale));
    crate::interface_scale::install_with_theme(
        &ui,
        presentation.scale,
        true,
        presentation.theme,
        |ui, dark| ui.set_dark(dark),
    );
    crate::motion::install(&ui, presentation.reduced);
    ui.set_picture(slint::Image::from_rgba8(
        slint::SharedPixelBuffer::clone_from_slice(
            result.thumbnail.as_raw(),
            result.thumbnail.width(),
            result.thumbnail.height(),
        ),
    ));
    let en = presentation.english;
    ui.set_message(
        if saved {
            if en { "Saved" } else { "已保存" }
        } else if en {
            "Copied to clipboard"
        } else {
            "已复制到剪贴板"
        }
        .into(),
    );
    ui.set_kind(
        if result.document.video.is_some() {
            if en { "Recording" } else { "录屏" }
        } else if en {
            "Screenshot"
        } else {
            "截图"
        }
        .into(),
    );
    ui.set_hint(
        if en {
            "Click to edit ↗"
        } else {
            "点击编辑 ↗"
        }
        .into(),
    );
    let document = result.document.clone();
    let weak = ui.as_weak();
    ui.on_open(
        move || match crate::workbench::open(document.clone(), presentation.clone()) {
            Ok(()) => dismiss(),
            Err(e) => {
                if let Some(ui) = weak.upgrade() {
                    ui.set_message(e.into());
                }
            }
        },
    );
    ui.on_dismiss(dismiss);
    ui.show().map_err(|e| e.to_string())?;
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_secs(10),
        dismiss,
    );
    FLOAT.with_borrow_mut(|f| {
        *f = Some(Float {
            ui: ui.clone_strong(),
            _document: result.document,
            _timer: timer,
        })
    });
    let weak = ui.as_weak();
    let _ = slint::spawn_local(async move {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Ok(window) = ui.window().winit_window().await else {
            return;
        };
        window.set_skip_taskbar(true);
        let result = (|| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Win32 thumbnail required".into());
            };
            let native =
                unsafe { glint_platform::NativePinWindow::from_raw(handle.hwnd.get() as *mut _) };
            native.configure(true, false)?;
            let (x, y, w, h) = native.work_area()?;
            let size = ui.window().size();
            let margin = (20. * ui.window().scale_factor()) as i32;
            native.position(
                x + (w as i32 - size.width as i32 - margin).max(0),
                y + (h as i32 - size.height as i32 - margin).max(0),
            )
        })();
        if result.is_err() {
            dismiss();
        }
    });
    Ok(())
}

pub fn smoke_snapshot(path: &std::path::Path) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    FLOAT.with_borrow(|f| {
        let f = f.as_ref().ok_or("Thumbnail missing")?;
        f.ui.window()
            .with_winit_window(|w| -> Result<(), String> {
                let h = w.window_handle().map_err(|e| e.to_string())?;
                let RawWindowHandle::Win32(h) = h.as_raw() else {
                    return Err("Not Win32".into());
                };
                let native =
                    unsafe { glint_platform::NativePinWindow::from_raw(h.hwnd.get() as *mut _) };
                let state = native.state()?;
                if !state.no_activate || state.click_through || !state.topmost {
                    return Err(format!("Thumbnail window policy failed: {state:?}"));
                }
                Ok(())
            })
            .ok_or("Thumbnail native peer missing")??;
        let p = f.ui.window().take_snapshot().map_err(|e| e.to_string())?;
        image::save_buffer(
            path,
            p.as_bytes(),
            p.width(),
            p.height(),
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
    })
}
