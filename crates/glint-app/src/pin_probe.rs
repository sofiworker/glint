//! M0 transparent pin/tray/hotkey integration probe; not yet the production pin manager.
use glint_platform::{HotkeyBinding, HotkeyEvent, HotkeyService, NativePinWindow};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer};
use std::{cell::RefCell, rc::Rc, time::Duration};
use tray_icon::{
    TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

slint::slint! {
    import { Button, HorizontalBox, Slider } from "std-widgets.slint";
    export component PinProbe inherits Window {
        title: "拾光 · Glint — M0 透明贴图";
        no-frame: true;
        background: transparent;
        width: 560px; height: 380px;
        in property <image> picture;
        in-out property <float> alpha: 1;
        in-out property <bool> on-top: true;
        in-out property <bool> locked: false;
        in-out property <bool> passthrough: false;
        in-out property <bool> rescue-ready: false;
        in-out property <bool> confirm-pass: false;
        in-out property <string> status: "M0 · 拖动标题栏 · 托盘可恢复贴图";
        callback action(string);
        callback change-opacity(float);
        callback drag;
        Rectangle {
            x: 8px; y: 8px; width: parent.width - 16px; height: parent.height - 16px;
            opacity: root.alpha;
            background: transparent;
            Rectangle {
                x: 0px; y: 0px; width: parent.width; height: 34px;
                background: #edf0f6; border-width: 1px; border-color: #4b5cfa;
                TouchArea { enabled: !root.locked; mouse-cursor: move; pointer-event(event) => {
                    if (event.kind == PointerEventKind.down && event.button == PointerEventButton.left) { root.drag(); }
                } }
                Text { x: 10px; width: 130px; text: root.locked ? "贴图 · 已锁定" : "贴图 · M0 验证"; color: #202b42; vertical-alignment: center; }
                HorizontalBox {
                    x: 145px; width: parent.width - 150px; spacing: 4px; padding: 3px;
                    Button { text: root.on-top ? "置顶 ✓" : "置顶"; clicked => { root.action("top"); } }
                    Button { text: root.locked ? "解锁" : "锁定"; clicked => { root.locked = !root.locked; } }
                    Button { text: "穿透"; enabled: root.rescue-ready; clicked => { root.confirm-pass = true; } }
                    Button { text: "隐藏"; clicked => { root.action("hide"); } }
                }
            }
            Image { x: 0px; y: 34px; width: parent.width; height: parent.height - 108px; source: root.picture; image-fit: contain; }
            Rectangle {
                x: 0px; y: parent.height - 74px; width: parent.width; height: 74px; background: #ffffff;
                Text { x: 10px; y: 0px; width: parent.width - 20px; height: 30px; text: root.status; color: #202b42; overflow: elide; font-size: 12px; }
                HorizontalBox {
                    x: 8px; y: 30px; width: parent.width - 16px; height: 40px; padding: 4px;
                    Text { text: "不透明度"; color: #69758b; }
                    Slider { minimum: 20; maximum: 100; value: root.alpha * 100; changed(value) => { root.change-opacity(value); } }
                    Button { text: "−"; clicked => { root.action("smaller"); } }
                    Button { text: "+"; clicked => { root.action("larger"); } }
                }
            }
        }
        if root.confirm-pass: Rectangle {
            x: 20px; y: 65px; width: parent.width - 40px; height: 150px; background: #ffffff;
            border-width: 1px; border-color: #4b5cfa; border-radius: 7px;
            Text { x: 16px; y: 12px; width: parent.width - 32px; height: 70px;
                text: "开启鼠标穿透？\n开启后无法点击贴图。可使用托盘救援或 Ctrl+Alt+Esc 恢复。";
                wrap: word-wrap; color: #202b42;
            }
            HorizontalBox { y: 90px; height: 44px; padding: 6px;
                Button { text: "取消"; clicked => { root.confirm-pass = false; } }
                Button { text: "开启穿透"; enabled: root.rescue-ready; clicked => { root.action("enable-pass"); } }
            }
        }
    }
    export component Backdrop inherits Window {
        title: "Glint M0 composition backdrop";
        no-frame: true;
        background: #406080;
        width: 560px; height: 380px;
    }
}

fn native<T>(
    ui: &PinProbe,
    operation: impl FnOnce(NativePinWindow) -> Result<T, String>,
) -> Result<T, String> {
    native_window(ui.window(), operation)
}

fn native_window<T>(
    window: &slint::Window,
    operation: impl FnOnce(NativePinWindow) -> Result<T, String>,
) -> Result<T, String> {
    let mut operation = Some(operation);
    window
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Pin needs a Win32 window".into());
            };
            // This wrapper is borrowed only inside the live winit window access callback.
            let pin =
                unsafe { NativePinWindow::from_raw(handle.hwnd.get() as *mut std::ffi::c_void) };
            operation.take().unwrap()(pin)
        })
        .ok_or_else(|| "Pin has no live native window".to_string())?
}

fn configure(ui: &PinProbe, passthrough: bool, topmost: bool) -> Result<(), String> {
    native(ui, |pin| pin.configure(topmost, passthrough))?;
    ui.set_passthrough(passthrough);
    ui.set_on_top(topmost);
    ui.set_confirm_pass(false);
    Ok(())
}

fn rescue(ui: &PinProbe) -> Result<(), String> {
    ui.show().map_err(|e| e.to_string())?;
    native(ui, |pin| pin.rescue())?;
    ui.set_passthrough(false);
    ui.set_on_top(true);
    ui.set_confirm_pass(false);
    ui.set_status("已恢复贴图并取消穿透".into());
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let conflict = args[0] == "--m0-pin-conflict-smoke";
    let smoke = args[0] == "--m0-pin-smoke" || conflict;
    if smoke && args.get(1).is_none() {
        return Err("--m0-pin-smoke requires snapshot.png".into());
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_window_attributes_hook(|attributes| {
            attributes.with_active(false).with_transparent(true)
        })
        .select()?;
    let ui = PinProbe::new()?;
    let (picture, width, height) = if smoke {
        let mut pixels = vec![0; 240 * 120 * 4];
        for y in 10..110 {
            for x in 10..230 {
                let i = (y * 240 + x) * 4;
                pixels[i..i + 4].copy_from_slice(&[237, 101, 113, 255]);
            }
        }
        (pixels, 240, 120)
    } else if let Some(path) = args.get(1) {
        let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        let image = reader.decode()?;
        let (width, height) = (image.width(), image.height());
        if u64::from(width) * u64::from(height) * 4 > 128 * 1024 * 1024 {
            return Err("M0 pin exceeds 128 MiB budget".into());
        }
        (image.to_rgba8().into_raw(), width, height)
    } else {
        let frame = glint_platform::capture_desktop().map_err(std::io::Error::other)?;
        let crop = glint_core::Rect {
            x: frame.width / 4,
            y: frame.height / 4,
            width: (frame.width / 2).max(1),
            height: (frame.height / 2).max(1),
        };
        (
            glint_core::composite(
                &frame.rgba,
                frame.width,
                frame.height,
                crop,
                &glint_core::Edits::default(),
            )?,
            crop.width,
            crop.height,
        )
    };
    ui.set_picture(slint::Image::from_rgba8(
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&picture, width, height),
    ));
    let weak = ui.as_weak();
    ui.on_action(move |action| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let result = match action.as_str() {
            "hide" => ui.hide().map_err(|e| e.to_string()),
            "show" | "rescue" => rescue(&ui),
            "top" => configure(&ui, ui.get_passthrough(), !ui.get_on_top()),
            "enable-pass" if ui.get_rescue_ready() => configure(&ui, true, ui.get_on_top()),
            "larger" | "smaller" => {
                let factor = if action == "larger" { 1.1 } else { 1.0 / 1.1 };
                let size = ui.window().size();
                ui.window().set_size(slint::PhysicalSize::new(
                    (size.width as f32 * factor).clamp(420.0, 1800.0) as u32,
                    (size.height as f32 * factor).clamp(285.0, 1200.0) as u32,
                ));
                Ok(())
            }
            "quit" => slint::quit_event_loop().map_err(|e| e.to_string()),
            _ => Ok(()),
        };
        if let Err(error) = result {
            ui.set_status(format!("操作失败：{error}").into());
        }
    });
    let weak = ui.as_weak();
    ui.on_change_opacity(move |value| {
        if let Some(ui) = weak.upgrade() {
            ui.set_alpha((value / 100.0).clamp(0.2, 1.0));
        }
    });
    let weak = ui.as_weak();
    ui.on_drag(move || {
        if let Some(ui) = weak.upgrade()
            && !ui.get_locked()
        {
            let result = ui.window().with_winit_window(|window| window.drag_window());
            if let Some(Err(error)) = result {
                ui.set_status(format!("无法拖动：{error}").into());
            }
        }
    });
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.hide();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    // Register rescue before enabling any passthrough controls. A conflict is visible and keeps passthrough disabled.
    let conflict_owner = if conflict {
        Some(
            HotkeyService::new(
                vec![HotkeyBinding {
                    action: 1,
                    modifiers: 3,
                    key: 0x1b,
                }],
                |_| {},
            )
            .map_err(std::io::Error::other)?,
        )
    } else {
        None
    };
    let weak = ui.as_weak();
    let hotkeys = HotkeyService::new(
        vec![
            HotkeyBinding {
                action: 1,
                modifiers: 3,
                key: 0x1b,
            }, // Ctrl+Alt+Esc rescue, as in HTML.
            HotkeyBinding {
                action: 2,
                modifiers: 6,
                key: u32::from(b'H'),
            }, // Ctrl+Shift+H hide/show.
        ],
        move |event| {
            let _ = weak.upgrade_in_event_loop(move |ui| match event {
                HotkeyEvent::Pressed(1) => ui.invoke_action("rescue".into()),
                HotkeyEvent::Pressed(2) => {
                    if native(&ui, |pin| pin.state())
                        .map(|s| s.visible)
                        .unwrap_or(false)
                    {
                        ui.invoke_action("hide".into());
                    } else {
                        ui.invoke_action("show".into());
                    }
                }
                HotkeyEvent::Error(error) => {
                    ui.set_rescue_ready(false);
                    let _ = rescue(&ui);
                    ui.set_status(format!("热键失效，已取消穿透：{error}").into());
                }
                _ => {}
            });
        },
    );
    match &hotkeys {
        Ok(_) => {
            ui.set_rescue_ready(true);
            ui.set_status("Ctrl+Alt+Esc 救援 · Ctrl+Shift+H 隐藏/恢复 · M0".into());
        }
        Err(error) => ui.set_status(format!("热键注册失败，已禁用穿透：{error}").into()),
    }
    let menu = Menu::new();
    let show = MenuItem::with_id("show", "恢复贴图", true, None);
    let hide = MenuItem::with_id("hide", "隐藏贴图", true, None);
    let rescue_item = MenuItem::with_id("rescue", "救援：显示并取消穿透", true, None);
    let quit = MenuItem::with_id("quit", "退出 M0 验证", true, None);
    menu.append_items(&[
        &show,
        &hide,
        &rescue_item,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;
    let weak = ui.as_weak();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = event.id.0;
        let _ = weak.upgrade_in_event_loop(move |ui| ui.invoke_action(action.into()));
    }));
    let weak = ui.as_weak();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(event, TrayIconEvent::DoubleClick { .. }) {
            let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_action("rescue".into()));
        }
    }));
    // Code-native icon uses the prototype accent; no external bitmap assets.
    let mut icon = vec![0; 16 * 16 * 4];
    for y in 2..14 {
        for x in 2..14 {
            let i = (y * 16 + x) * 4;
            icon[i..i + 4].copy_from_slice(&[75, 92, 250, 255]);
        }
    }
    let _tray = TrayIconBuilder::new()
        .with_tooltip("拾光 Glint · M0 贴图验证")
        .with_icon(tray_icon::Icon::from_rgba(icon, 16, 16)?)
        .with_menu(Box::new(menu))
        .build()?;
    let backdrop = if smoke { Some(Backdrop::new()?) } else { None };
    if let Some(backdrop) = &backdrop {
        backdrop
            .window()
            .set_size(slint::PhysicalSize::new(560, 380));
        backdrop
            .window()
            .set_position(slint::PhysicalPosition::new(80, 100));
        backdrop.show()?;
    }
    ui.window().set_size(slint::PhysicalSize::new(560, 380));
    ui.window()
        .set_position(slint::PhysicalPosition::new(80, 100));
    let focus_before_show = glint_platform::foreground_window_id();
    ui.show()?;
    // Winit creates its native peer after the event loop resumes, not during initial show().
    let weak = ui.as_weak();
    let backdrop_weak = backdrop.as_ref().map(ComponentHandle::as_weak);
    slint::spawn_local(async move {
        if let Some(ui) = weak.upgrade() {
            if let Some(backdrop) = backdrop_weak.and_then(|weak| weak.upgrade()) {
                let result = backdrop
                    .window()
                    .winit_window()
                    .await
                    .map(drop)
                    .map_err(|e| e.to_string())
                    .and_then(|()| {
                        native_window(backdrop.window(), |pin| pin.configure(true, false))
                    });
                if let Err(error) = result {
                    ui.set_rescue_ready(false);
                    ui.set_status(error.into());
                    return;
                }
            }
            let result = ui
                .window()
                .winit_window()
                .await
                .map(drop)
                .map_err(|e| e.to_string())
                .and_then(|()| configure(&ui, false, true));
            if let Err(error) = result {
                ui.set_rescue_ready(false);
                ui.set_status(format!("贴图初始化失败：{error}").into());
            }
        }
    })?;
    let error = Rc::new(RefCell::new(None));
    if smoke {
        let observe_ms = args
            .get(2)
            .map(|s| s.parse::<u64>())
            .transpose()?
            .unwrap_or(500)
            .clamp(500, 30_000);
        schedule_smoke(
            &ui,
            args[1].clone(),
            error.clone(),
            focus_before_show,
            conflict,
            observe_ms,
        );
    }
    slint::run_event_loop_until_quit()?;
    // Restore interactivity even on an early loop exit, before tearing down tray and rescue binding.
    let _ = native(&ui, |pin| pin.configure(false, false));
    drop(hotkeys);
    drop(conflict_owner);
    if let Some(error) = error.borrow_mut().take() {
        return Err(std::io::Error::other(error).into());
    }
    drop(backdrop);
    Ok(())
}

fn schedule_smoke(
    ui: &PinProbe,
    path: String,
    error: Rc<RefCell<Option<String>>>,
    focus_before_show: usize,
    conflict: bool,
    observe_ms: u64,
) {
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(observe_ms), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let result = (|| -> Result<(), String> {
            if !ui.get_rescue_ready() && !conflict {
                return Err(ui.get_status().to_string());
            }
            let state = native(&ui, |pin| pin.state())?;
            if !state.topmost || !state.no_activate || !state.visible || state.click_through {
                return Err(format!("Incorrect initial native pin flags: {state:?}"));
            }
            if conflict {
                if ui.get_rescue_ready() || !ui.get_status().contains("热键注册失败") {
                    return Err("Hotkey conflict was not shown in the pin UI".into());
                }
                ui.invoke_action("enable-pass".into());
                if native(&ui, |pin| pin.state())?.click_through {
                    return Err("Conflict allowed unsafe click-through".into());
                }
                ui.invoke_action("rescue".into());
                eprintln!(
                    "M0 conflict: real rescue-key registration conflict visible, click-through blocked, tray rescue action available"
                );
            }
            if glint_platform::foreground_window_id() != focus_before_show {
                return Err("Pin show changed foreground window".into());
            }
            let desktop = glint_platform::capture_desktop()?;
            let sample = |dx: i32, dy: i32| -> Result<&[u8], String> {
                let x = state.x + dx - desktop.x;
                let y = state.y + dy - desktop.y;
                if x < 0 || y < 0 || x >= desktop.width as i32 || y >= desktop.height as i32 {
                    return Err("Pin test is outside the captured desktop".into());
                }
                let i = (y as usize * desktop.width as usize + x as usize) * 4;
                Ok(&desktop.rgba[i..i + 4])
            };
            if sample(2, 2)? != [64, 96, 128, 255] {
                return Err(format!(
                    "OS-composited transparent margin differs from backdrop: {:?}",
                    sample(2, 2)?
                ));
            }
            if sample(state.width / 2, state.height / 2)? != [237, 101, 113, 255] {
                return Err(format!(
                    "OS-composited pin image pixel mismatch: {:?}",
                    sample(state.width / 2, state.height / 2)?
                ));
            }
            let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
            if pixels.as_bytes()[3] != 0 {
                return Err("Software pin margin is not transparent".into());
            }
            image::save_buffer_with_format(
                &path,
                pixels.as_bytes(),
                pixels.width(),
                pixels.height(),
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .map_err(|e| e.to_string())?;
            ui.invoke_change_opacity(60.0);
            if (ui.get_alpha() - 0.6).abs() > 0.001 {
                return Err("Opacity callback did not apply".into());
            }
            Ok(())
        })();
        if let Err(message) = result {
            *error.borrow_mut() = Some(message);
            let _ = slint::quit_event_loop();
            return;
        }
        if conflict {
            let _ = slint::quit_event_loop();
        } else {
            schedule_opacity_and_rescue(&ui, error, focus_before_show);
        }
    });
}

fn schedule_opacity_and_rescue(
    ui: &PinProbe,
    error: Rc<RefCell<Option<String>>>,
    focus_before_show: usize,
) {
    let weak = ui.as_weak();
    // Let the real window repaint before sampling the OS-composited alpha blend.
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let result = (|| -> Result<(), String> {
            let state = native(&ui, |pin| pin.state())?;
            let point = (state.x + state.width / 2, state.y + state.height / 2);
            let desktop = glint_platform::capture_desktop()?;
            let x = point.0 - desktop.x;
            let y = point.1 - desktop.y;
            if x < 0 || y < 0 || x >= desktop.width as i32 || y >= desktop.height as i32 {
                return Err("Opacity test point outside desktop".into());
            }
            let offset = (y as usize * desktop.width as usize + x as usize) * 4;
            let actual = &desktop.rgba[offset..offset + 3];
            for (actual, expected) in actual.iter().zip([168i16, 99, 119]) {
                if (i16::from(*actual) - expected).abs() > 2 {
                    return Err(format!("OS-composited opacity mismatch: {actual}"));
                }
            }
            if !native(&ui, |pin| pin.receives_pointer_at(point.0, point.1))? {
                return Err("Pin did not receive normal OS hit testing".into());
            }
            ui.invoke_action("enable-pass".into());
            if !native(&ui, |pin| pin.state())?.click_through {
                return Err("Native click-through flag missing".into());
            }
            if native(&ui, |pin| pin.receives_pointer_at(point.0, point.1))? {
                return Err("Click-through pin still receives OS hit testing".into());
            }
            ui.invoke_action("rescue".into());
            let rescued = native(&ui, |pin| pin.state())?;
            if rescued.click_through || !rescued.topmost || !rescued.visible {
                return Err(format!("Rescue failed: {rescued:?}"));
            }
            if !native(&ui, |pin| pin.receives_pointer_at(point.0, point.1))? {
                return Err("Rescued pin did not recover OS hit testing".into());
            }
            ui.window().with_winit_window(|window| {
                window.set_outer_position(slint::winit_030::winit::dpi::PhysicalPosition::new(
                    -30000, -30000,
                ))
            });
            ui.invoke_action("rescue".into());
            let moved = native(&ui, |pin| pin.state())?;
            if moved.x < desktop.x
                || moved.y < desktop.y
                || moved.x >= desktop.x + desktop.width as i32
                || moved.y >= desktop.y + desktop.height as i32
            {
                return Err(format!(
                    "Offscreen pin rescue did not return to desktop: {moved:?}"
                ));
            }
            ui.invoke_action("top".into());
            if native(&ui, |pin| pin.state())?.topmost {
                return Err("Topmost toggle did not clear native flag".into());
            }
            ui.invoke_action("top".into());
            if !native(&ui, |pin| pin.state())?.topmost {
                return Err("Topmost toggle did not restore native flag".into());
            }
            ui.invoke_action("hide".into());
            // Slint destroys its native peer when hidden; do not reuse any borrowed HWND.
            ui.invoke_action("show".into());
            let restored = native(&ui, |pin| pin.state())?;
            if restored.click_through || !restored.visible || !restored.no_activate {
                return Err(format!("Restored pin flags incorrect: {restored:?}"));
            }
            if glint_platform::foreground_window_id() != focus_before_show {
                return Err("Pin rescue changed foreground window".into());
            }
            eprintln!(
                "M0 pin: OS-composited transparency/opacity, foreground preservation, native topmost/noactivate and hit testing through passthrough/rescue, offscreen rescue, hide/restore callbacks passed"
            );
            Ok(())
        })();
        if let Err(message) = result {
            *error.borrow_mut() = Some(message);
        }
        let _ = slint::quit_event_loop();
    });
}
