use super::*;
use std::ffi::c_void;
struct Bitmap {
    handle: HBITMAP,
    bits: *mut u8,
    width: usize,
    height: usize,
}
impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.handle);
        }
    }
}
impl Bitmap {
    fn rgba(width: usize, height: usize, pixel: [u8; 4]) -> Self {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut::<c_void>();
        let handle =
            unsafe { CreateDIBSection(HDC::default(), &info, DIB_RGB_COLORS, &mut bits, None, 0) }
                .unwrap();
        let bits = bits.cast::<u8>();
        let bytes = unsafe { std::slice::from_raw_parts_mut(bits, width * height * 4) };
        for p in bytes.as_chunks_mut::<4>().0 {
            *p = [pixel[2], pixel[1], pixel[0], pixel[3]];
        }
        Self {
            handle,
            bits,
            width,
            height,
        }
    }
    fn mask(height: usize, rows: &[u8]) -> Self {
        let handle = unsafe { CreateBitmap(4, height as i32, 1, 1, Some(rows.as_ptr().cast())) };
        assert!(!handle.is_invalid());
        Self {
            handle,
            bits: std::ptr::null_mut(),
            width: 4,
            height,
        }
    }
    fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        assert!(x < self.width && y < self.height);
        unsafe {
            GdiFlush().unwrap();
            let p = std::slice::from_raw_parts(self.bits.add((y * self.width + x) * 4), 4);
            [p[2], p[1], p[0]]
        }
    }
}
struct Surface {
    dc: HDC,
    bitmap: Bitmap,
    previous: HGDIOBJ,
}
impl Surface {
    fn new(pixel: [u8; 4]) -> Self {
        let bitmap = Bitmap::rgba(8, 8, pixel);
        let dc = unsafe { CreateCompatibleDC(HDC::default()) };
        assert!(!dc.is_invalid());
        let previous = unsafe { SelectObject(dc, bitmap.handle) };
        assert!(!previous.is_invalid());
        Self {
            dc,
            bitmap,
            previous,
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteDC(self.dc);
        }
    }
}
fn color(pixel: [u8; 4]) -> Icon {
    let bitmap = Bitmap::rgba(4, 4, pixel);
    let mask = Bitmap::mask(4, &[0; 8]);
    Icon(
        unsafe {
            CreateIconIndirect(&ICONINFO {
                fIcon: false.into(),
                xHotspot: 2,
                yHotspot: 1,
                hbmMask: mask.handle,
                hbmColor: bitmap.handle,
            })
        }
        .unwrap(),
    )
}
fn mono() -> Icon {
    // AND rows, then XOR rows. Row 1: transparent, black, white, invert.
    let mask = Bitmap::mask(
        8,
        &[
            0xF0, 0, 0x90, 0, 0xF0, 0, 0xF0, 0, 0, 0, 0x30, 0, 0, 0, 0, 0,
        ],
    );
    Icon(
        unsafe {
            CreateIconIndirect(&ICONINFO {
                fIcon: false.into(),
                xHotspot: 2,
                yHotspot: 1,
                hbmMask: mask.handle,
                hbmColor: HBITMAP::default(),
            })
        }
        .unwrap(),
    )
}
#[test]
fn color_alpha_hotspot_negative_origin_and_clipping() {
    let icon = color([200, 80, 20, 255]);
    let cursor = Cursor::copy(icon.0, (-997, -197)).unwrap();
    assert_eq!(cursor.hotspot, (2, 1));
    let surface = Surface::new([30, 60, 90, 255]);
    cursor.draw(surface.dc, (-1000, -200)).unwrap();
    assert_eq!(surface.bitmap.pixel(0, 2), [30, 60, 90]);
    assert_eq!(surface.bitmap.pixel(1, 2), [200, 80, 20]);
    assert_eq!(surface.bitmap.pixel(4, 5), [200, 80, 20]);
    assert_eq!(surface.bitmap.pixel(5, 5), [30, 60, 90]);
    // CreateIconIndirect accepts straight color and prepares its internal alpha.
    let alpha = color([255, 0, 0, 128]);
    let cursor = Cursor::copy(alpha.0, (2, 1)).unwrap();
    let surface = Surface::new([30, 60, 90, 255]);
    cursor.draw(surface.dc, (0, 0)).unwrap();
    let p = surface.bitmap.pixel(0, 0);
    for (actual, expected) in p.into_iter().zip([143i16, 30, 45]) {
        assert!((i16::from(actual) - expected).abs() <= 1, "alpha={p:?}");
    }
    let cursor = Cursor::copy(icon.0, (-1000, -200)).unwrap();
    let surface = Surface::new([30, 60, 90, 255]);
    cursor.draw(surface.dc, (-1000, -200)).unwrap();
    assert_eq!(surface.bitmap.pixel(0, 0), [200, 80, 20]);
    assert_eq!(surface.bitmap.pixel(1, 2), [200, 80, 20]);
    assert_eq!(surface.bitmap.pixel(2, 0), [30, 60, 90]);
    let mut cursor = cursor;
    cursor.position = (i32::MAX, 0);
    assert!(cursor.draw(surface.dc, (i32::MIN, 0)).is_err());
}
#[test]
fn monochrome_transparent_black_white_and_inverse_preserve_desktop() {
    let icon = mono();
    let cursor = Cursor::copy(icon.0, (-998, -198)).unwrap();
    let surface = Surface::new([30, 60, 90, 255]);
    cursor.draw(surface.dc, (-1000, -200)).unwrap();
    assert_eq!(surface.bitmap.pixel(0, 2), [30, 60, 90]);
    assert_eq!(surface.bitmap.pixel(1, 2), [0, 0, 0]);
    assert_eq!(surface.bitmap.pixel(2, 2), [255, 255, 255]);
    assert_eq!(surface.bitmap.pixel(3, 2), [225, 195, 165]);
    assert_eq!(surface.bitmap.pixel(7, 7), [30, 60, 90]);
}
#[test]
#[ignore = "Process-wide GDI/USER counts require a serial, filtered test run"]
fn repeated_real_cursor_copies_release_icon_info_bitmaps() {
    let raw = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap();
    let icon = HICON(raw.0);
    {
        let cursor = Cursor::copy(icon, (3, 3)).unwrap();
        let surface = Surface::new([30, 60, 90, 255]);
        cursor.draw(surface.dc, (0, 0)).unwrap();
    }
    let before = crate::gui_resource_counts().unwrap();
    for _ in 0..128 {
        let cursor = Cursor::copy(icon, (3, 3)).unwrap();
        let surface = Surface::new([30, 60, 90, 255]);
        cursor.draw(surface.dc, (0, 0)).unwrap();
    }
    assert_eq!(crate::gui_resource_counts().unwrap(), before);
}

#[test]
#[ignore = "Temporarily shows a fixture, injects one guarded click into it and changes the OS cursor"]
fn actual_desktop_cursor_option_includes_and_excludes_known_os_cursor() {
    use windows::{
        Win32::{
            Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::Input::KeyboardAndMouse::*,
        },
        core::w,
    };
    fn mouse(flags: MOUSE_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        }
    }
    struct Fixture {
        window: HWND,
        instance: HINSTANCE,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe {
                if !self.window.is_invalid() {
                    let _ = DestroyWindow(self.window);
                }
                let _ = UnregisterClassW(w!("Glint.CursorCaptureVerification"), self.instance);
            }
        }
    }
    struct Restore {
        cursor: HCURSOR,
        shows: u8,
        focus: Option<crate::FocusTarget>,
        window: HWND,
        release_left: bool,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            unsafe {
                if self.release_left {
                    SendInput(
                        &[mouse(MOUSEEVENTF_LEFTUP)],
                        std::mem::size_of::<INPUT>() as i32,
                    );
                }
                // A user who moved focus elsewhere during the test takes priority.
                if GetForegroundWindow() == self.window
                    && let Some(focus) = &self.focus
                {
                    let _ = focus.restore();
                }
                SetCursor(self.cursor);
                for _ in 0..self.shows {
                    ShowCursor(false);
                }
            }
        }
    }
    let icon = color([13, 211, 77, 255]);
    let mut original = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetCursorInfo(&mut original) }.unwrap();
    let mut restore = Restore {
        cursor: original.hCursor,
        shows: 0,
        focus: crate::remember_foreground(),
        window: HWND::default(),
        release_left: false,
    };
    let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }.unwrap().0);
    unsafe extern "system" fn procedure(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
    let class = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: instance,
        hCursor: HCURSOR(icon.0.0),
        hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
        lpszClassName: w!("Glint.CursorCaptureVerification"),
        ..Default::default()
    };
    assert_ne!(unsafe { RegisterClassW(&class) }, 0);
    let mut fixture = Fixture {
        window: HWND::default(),
        instance,
    };
    fixture.window = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class.lpszClassName,
            w!("Glint cursor verification"),
            WS_POPUP | WS_VISIBLE,
            original.ptScreenPos.x - 40,
            original.ptScreenPos.y - 40,
            100,
            100,
            None,
            None,
            instance,
            None,
        )
    }
    .unwrap();
    restore.window = fixture.window;
    unsafe {
        let _ = UpdateWindow(fixture.window);
        // Background processes cannot assume foreground activation permission.
        // Deliver real input only when this test window owns the current point.
        for button in [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2] {
            assert_eq!(
                GetAsyncKeyState(i32::from(button.0)) as u16 & 0x8000,
                0,
                "Mouse button held; cursor verification did not inject input"
            );
        }
        let mut point = POINT::default();
        GetCursorPos(&mut point).unwrap();
        assert_eq!(
            (point.x, point.y),
            (original.ptScreenPos.x, original.ptScreenPos.y),
            "Cursor moved before fixture input; no click injected"
        );
        assert_eq!(
            WindowFromPoint(point),
            fixture.window,
            "Fixture does not own input point; no click injected"
        );
        let sent = SendInput(
            &[mouse(MOUSEEVENTF_LEFTDOWN), mouse(MOUSEEVENTF_LEFTUP)],
            std::mem::size_of::<INPUT>() as i32,
        );
        restore.release_left = sent == 1;
        assert_eq!(sent, 2, "Could not inject the fixture click");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut released = false;
        loop {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                released |= message.hwnd == fixture.window && message.message == WM_LBUTTONUP;
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if released && GetForegroundWindow() == fixture.window {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Fixture did not receive its click and foreground activation"
            );
            MsgWaitForMultipleObjects(None, false, 20, QS_ALLINPUT);
        }
        SendMessageW(
            fixture.window,
            WM_SETCURSOR,
            WPARAM(fixture.window.0 as usize),
            LPARAM((WM_MOUSEMOVE as isize) << 16 | 1),
        );
        SetCursor(HCURSOR(icon.0.0));
    }
    loop {
        restore.shows += 1;
        if unsafe { ShowCursor(true) } >= 0 {
            break;
        }
        assert!(restore.shows < 16, "OS cursor could not be made visible");
    }
    let mut state = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetCursorInfo(&mut state) }.unwrap();
    assert_eq!(state.hCursor.0, icon.0.0, "OS replaced the test cursor");
    assert_eq!(state.flags, CURSOR_SHOWING);
    let included = crate::capture_snapshot_with_cursor(false, true).unwrap();
    let excluded = crate::capture_snapshot_with_cursor(false, false).unwrap();
    let x = i64::from(state.ptScreenPos.x) - i64::from(included.frame.x) - 2;
    let y = i64::from(state.ptScreenPos.y) - i64::from(included.frame.y) - 1;
    let mut checked = 0;
    let mut excluded_different = false;
    for row in y..y + 4 {
        for col in x..x + 4 {
            if col < 0
                || row < 0
                || col >= i64::from(included.frame.width)
                || row >= i64::from(included.frame.height)
            {
                continue;
            }
            let i = ((row as usize * included.frame.width as usize) + col as usize) * 4;
            assert_eq!(&included.frame.rgba[i..i + 4], &[13, 211, 77, 255]);
            excluded_different |= excluded.frame.rgba[i..i + 3] != [13, 211, 77];
            checked += 1;
        }
    }
    assert!(checked > 0);
    assert!(
        excluded_different,
        "Disabled option still contains the test cursor"
    );
}
