//! Explicit QA feature only. Real input is restricted to this process's visible windows.
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::ClientToScreen,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

#[derive(Clone, Copy)]
pub enum Pointer {
    Move,
    Down,
    Up,
    Click,
}

/// Fixed QA gestures; arbitrary keyboard injection while dragging stays forbidden.
pub enum DragKey {
    Undo,
    Redo,
    Finish,
    NextFocus,
    Activate,
}

/// Never used by normal native-shell builds. Aborts if focus, pointer ownership,
/// user-held modifiers/buttons or pointer position cease to satisfy the guard.
pub struct InputProbe {
    original: POINT,
    last: Option<POINT>,
    left: bool,
    keys: Vec<u16>,
    unicode: Vec<u16>,
}
fn owned(id: usize) -> Result<HWND, String> {
    let window = HWND(id as *mut std::ffi::c_void);
    let mut pid = 0;
    if id == 0
        || unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) } == 0
        || pid != std::process::id()
    {
        return Err("Input probe target is not a live window owned by this process".into());
    }
    Ok(window)
}
fn mouse(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}
fn key(code: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(code),
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS::default()
                },
                ..Default::default()
            },
        },
    }
}
fn unicode_key(unit: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE
                    | if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS::default()
                    },
                ..Default::default()
            },
        },
    }
}
impl InputProbe {
    /// QA-only real input to the application's exact owned replacement prompt.
    /// The fixture hook changes only the isolated QA file after observation, to
    /// exercise the production stale-authorization failure without mocking it.
    pub fn replacement_confirmation(
        &mut self,
        owner: usize,
        title: &str,
        approve: bool,
        fixture: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let owner = owned(owner)?;
        let start = std::time::Instant::now();
        let dialog = loop {
            self.idle()?;
            let foreground = unsafe { GetForegroundWindow() };
            owned(foreground.0 as usize)?;
            if foreground != owner {
                let mut class = [0u16; 64];
                let length = unsafe { GetClassNameW(foreground, &mut class) } as usize;
                if unsafe { GetWindow(foreground, GW_OWNER) }.map_err(|e| e.to_string())? != owner
                    || String::from_utf16_lossy(&class[..length]) != "#32770"
                {
                    return Err("Replacement input encountered a different owned window".into());
                }
                let mut text = [0u16; 256];
                let length = unsafe { GetWindowTextW(foreground, &mut text) } as usize;
                if String::from_utf16_lossy(&text[..length]) == title {
                    break foreground;
                }
            }
            if start.elapsed() > std::time::Duration::from_secs(15) {
                return Err("Owned replacement confirmation did not appear".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        std::thread::sleep(std::time::Duration::from_millis(150));
        let focus = |expected| -> Result<(), String> {
            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            let thread = unsafe { GetWindowThreadProcessId(dialog, None) };
            unsafe { GetGUIThreadInfo(thread, &mut info) }.map_err(|e| e.to_string())?;
            if !unsafe { IsChild(dialog, info.hwndFocus) }.as_bool()
                || unsafe { GetDlgCtrlID(info.hwndFocus) } != expected
            {
                return Err("Replacement confirmation button does not own expected focus".into());
            }
            Ok(())
        };
        focus(IDCANCEL.0)?; // Production default must remain the safe Cancel button.
        fixture()?;
        if approve {
            self.chord(dialog.0 as usize, &[VK_LEFT.0])?;
            std::thread::sleep(std::time::Duration::from_millis(100));
            focus(IDOK.0)?;
        }
        self.chord(dialog.0 as usize, &[VK_RETURN.0])
    }
    /// Explicit QA only: wait for the foreground standard dialog owned directly
    /// by the live capture HWND. The UI thread may be blocked in GetSaveFileNameW.
    pub fn save_dialog(&mut self, owner: usize, path: Option<&str>) -> Result<(), String> {
        let owner = owned(owner)?;
        let start = std::time::Instant::now();
        let dialog = loop {
            self.idle()?;
            let foreground = unsafe { GetForegroundWindow() };
            if foreground != owner {
                owned(foreground.0 as usize)?;
                let mut class = [0u16; 64];
                let length = unsafe { GetClassNameW(foreground, &mut class) } as usize;
                if unsafe { GetWindow(foreground, GW_OWNER) }.map_err(|e| e.to_string())? != owner
                    || String::from_utf16_lossy(&class[..length]) != "#32770"
                {
                    return Err("Foreground is not the capture's owned standard dialog".into());
                }
                break foreground.0 as usize;
            }
            if start.elapsed() > std::time::Duration::from_secs(15) {
                return Err("Timed out waiting for owned Save As dialog".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        // Let the common dialog initialize its filename control before input.
        std::thread::sleep(std::time::Duration::from_millis(300));
        let Some(path) = path else {
            return self.chord(dialog, &[VK_ESCAPE.0]);
        };
        let units: Vec<_> = path.encode_utf16().collect();
        if units.is_empty() || units.len() > 1024 || units.iter().any(|c| *c < 32) {
            return Err("Invalid QA output path".into());
        }
        self.chord(dialog, &[VK_MENU.0, 0x4E])?; // Standard filename mnemonic, Alt+N.
        std::thread::sleep(std::time::Duration::from_millis(150));
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let thread = unsafe { GetWindowThreadProcessId(owned(dialog)?, None) };
        unsafe { GetGUIThreadInfo(thread, &mut info) }.map_err(|e| e.to_string())?;
        let mut class = [0u16; 64];
        let length = unsafe { GetClassNameW(info.hwndFocus, &mut class) } as usize;
        if !unsafe { IsChild(owned(dialog)?, info.hwndFocus) }.as_bool()
            || !String::from_utf16_lossy(&class[..length]).eq_ignore_ascii_case("Edit")
        {
            return Err("Save filename edit does not own focus; no text sent".into());
        }
        self.chord(dialog, &[VK_CONTROL.0, 0x41])?;
        self.idle()?;
        if !self.foreground(dialog)? {
            return Err("Save dialog lost foreground before filename entry".into());
        }
        let inputs: Vec<_> = units
            .into_iter()
            .flat_map(|unit| {
                [false, true].map(|up| INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wScan: unit,
                            dwFlags: KEYEVENTF_UNICODE
                                | if up {
                                    KEYEVENTF_KEYUP
                                } else {
                                    KEYBD_EVENT_FLAGS::default()
                                },
                            ..Default::default()
                        },
                    },
                })
            })
            .collect();
        if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize
            != inputs.len()
        {
            return Err("Windows did not accept the complete QA filename".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        self.chord(dialog, &[VK_RETURN.0])
    }
    pub fn new() -> Result<Self, String> {
        let mut original = POINT::default();
        unsafe { GetCursorPos(&mut original) }.map_err(|e| e.to_string())?;
        let result = Self {
            original,
            last: None,
            left: false,
            keys: vec![],
            unicode: vec![],
        };
        result.idle()?;
        Ok(result)
    }
    fn idle(&self) -> Result<(), String> {
        for code in [
            VK_CONTROL,
            VK_SHIFT,
            VK_MENU,
            VK_LWIN,
            VK_RWIN,
            VK_LBUTTON,
            VK_RBUTTON,
            VK_MBUTTON,
            VK_XBUTTON1,
            VK_XBUTTON2,
        ] {
            if code == VK_LBUTTON && self.left {
                continue;
            }
            if unsafe { GetAsyncKeyState(i32::from(code.0)) } as u16 & 0x8000 != 0 {
                return Err("Input probe stopped: a modifier or mouse button is held".into());
            }
        }
        if let Some(last) = self.last {
            let mut point = POINT::default();
            unsafe { GetCursorPos(&mut point) }.map_err(|e| e.to_string())?;
            if (i64::from(point.x) - i64::from(last.x)).abs() > 1
                || (i64::from(point.y) - i64::from(last.y)).abs() > 1
            {
                return Err("Input probe stopped: pointer moved outside the probe".into());
            }
        }
        Ok(())
    }
    pub fn foreground(&self, id: usize) -> Result<bool, String> {
        Ok(unsafe { GetForegroundWindow() } == owned(id)?)
    }
    /// Read-only hit test to choose an uncovered point before a guarded activation.
    pub fn hits(&self, id: usize, x: i32, y: i32) -> Result<bool, String> {
        let window = owned(id)?;
        if !unsafe { IsWindowVisible(window) }.as_bool() || unsafe { IsIconic(window) }.as_bool() {
            return Ok(false);
        }
        let mut rect = RECT::default();
        unsafe { GetClientRect(window, &mut rect) }.map_err(|e| e.to_string())?;
        if x < 0 || y < 0 || x >= rect.right || y >= rect.bottom {
            return Ok(false);
        }
        let mut point = POINT { x, y };
        if !unsafe { ClientToScreen(window, &mut point) }.as_bool() {
            return Err("Cannot locate owned hit-test point".into());
        }
        Ok(unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) } == window)
    }
    /// `activate` permits only an initial click on the owned window, even when
    /// another process is foreground. Every target point must hit this root HWND.
    pub fn pointer(
        &mut self,
        id: usize,
        x: i32,
        y: i32,
        action: Pointer,
        activate: bool,
    ) -> Result<(), String> {
        let window = owned(id)?;
        self.idle()?;
        if !unsafe { IsWindowVisible(window) }.as_bool() || unsafe { IsIconic(window) }.as_bool() {
            return Err("Input target is hidden/minimized".into());
        }
        if !activate {
            owned(unsafe { GetForegroundWindow() }.0 as usize)?;
        }
        if activate && (!matches!(action, Pointer::Click) || self.left) {
            return Err("Activation requires one unheld owned-window click".into());
        }
        let mut rect = RECT::default();
        unsafe { GetClientRect(window, &mut rect) }.map_err(|e| e.to_string())?;
        if x < 0 || y < 0 || x >= rect.right || y >= rect.bottom {
            return Err("Input point is outside the owned client area".into());
        }
        let mut point = POINT { x, y };
        if !unsafe { ClientToScreen(window, &mut point) }.as_bool() {
            return Err("Cannot locate owned input point".into());
        }
        let hit = unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) };
        if hit != window {
            return Err("Input point is covered by a different window; no input sent".into());
        }
        let capture = unsafe { GetCapture() };
        if !capture.is_invalid() && capture != window {
            return Err("A different window owns pointer capture".into());
        }
        if matches!(action, Pointer::Down | Pointer::Click) && self.left {
            return Err("Probe already holds left button".into());
        }
        if matches!(action, Pointer::Up) && !self.left {
            return Err("Probe does not hold left button".into());
        }
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        if width <= 0 || height <= 0 {
            return Err("Invalid virtual desktop for input".into());
        }
        let normalize = |value: i32, origin: i32, span: i32| {
            (((2 * (i64::from(value) - i64::from(origin)) + 1) * 65536) / (2 * i64::from(span)))
                .clamp(0, 65535) as i32
        };
        let mut inputs = vec![mouse(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            normalize(point.x, left, width),
            normalize(point.y, top, height),
        )];
        if matches!(action, Pointer::Down | Pointer::Click) {
            inputs.push(mouse(MOUSEEVENTF_LEFTDOWN, 0, 0));
            self.left = true;
        }
        if matches!(action, Pointer::Up | Pointer::Click) {
            inputs.push(mouse(MOUSEEVENTF_LEFTUP, 0, 0));
        }
        if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize
            != inputs.len()
        {
            return Err("Windows did not accept all guarded pointer inputs".into());
        }
        if matches!(action, Pointer::Up | Pointer::Click) {
            self.left = false;
        }
        self.last = Some(point);
        Ok(())
    }
    /// QA-only committed text, independent of the active keyboard/IME layout.
    /// The caller must also observe focus on its exact application text field.
    pub fn text(&mut self, id: usize, value: &str) -> Result<(), String> {
        self.idle()?;
        if self.left || !self.foreground(id)? {
            return Err("Text target does not own foreground or drag is active".into());
        }
        if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
            return Err("Invalid QA text".into());
        }
        self.unicode = value.encode_utf16().collect();
        let inputs: Vec<_> = self
            .unicode
            .iter()
            .flat_map(|unit| [unicode_key(*unit, false), unicode_key(*unit, true)])
            .collect();
        if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize
            != inputs.len()
        {
            return Err("Windows did not accept all guarded Unicode inputs".into());
        }
        self.unicode.clear();
        Ok(())
    }
    /// A bounded real title-bar drag. Run off the UI thread: Windows may enter
    /// a nested move loop until the worker releases its own mouse button.
    pub fn window_drag(
        &mut self,
        id: usize,
        start: (i32, i32),
        delta: (i32, i32),
    ) -> Result<(), String> {
        if !(-160..=160).contains(&delta.0) || !(-160..=160).contains(&delta.1) || delta == (0, 0) {
            return Err("Invalid bounded QA drag".into());
        }
        let window = owned(id)?;
        let mut anchor = POINT {
            x: start.0,
            y: start.1,
        };
        let foreground = unsafe { GetForegroundWindow() };
        owned(foreground.0 as usize)?;
        if !unsafe { ClientToScreen(window, &mut anchor) }.as_bool() {
            return Err("Cannot locate QA drag origin".into());
        }
        self.pointer(id, start.0, start.1, Pointer::Down, false)?;
        for step in 1..=8 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if unsafe { GetForegroundWindow() } != foreground {
                return Err("Pin drag changed the original foreground".into());
            }
            let mut origin = POINT::default();
            if !unsafe { ClientToScreen(owned(id)?, &mut origin) }.as_bool() {
                return Err("QA drag window disappeared".into());
            }
            self.pointer(
                id,
                anchor.x + delta.0 * step / 8 - origin.x,
                anchor.y + delta.1 * step / 8 - origin.y,
                Pointer::Move,
                false,
            )?;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut origin = POINT::default();
        if !unsafe { ClientToScreen(owned(id)?, &mut origin) }.as_bool() {
            return Err("QA drag window disappeared before release".into());
        }
        self.pointer(
            id,
            anchor.x + delta.0 - origin.x,
            anchor.y + delta.1 - origin.y,
            Pointer::Up,
            false,
        )
    }
    /// Real wheel input over an owned hit-tested surface; optional Ctrl is held
    /// only within the one packet batch and tracked for partial-send cleanup.
    /// Run off the UI thread so it can process the initial pointer movement.
    pub fn wheel(
        &mut self,
        id: usize,
        x: i32,
        y: i32,
        delta: i32,
        control: bool,
    ) -> Result<(), String> {
        if delta == 0 || !(-960..=960).contains(&delta) || self.left {
            return Err("Invalid bounded QA wheel input".into());
        }
        self.pointer(id, x, y, Pointer::Move, false)?;
        std::thread::sleep(std::time::Duration::from_millis(75));
        self.pointer(id, x, y, Pointer::Move, false)?;
        let mut inputs = Vec::new();
        if control {
            self.keys.push(VK_CONTROL.0);
            inputs.push(key(VK_CONTROL.0, false));
        }
        let mut event = mouse(MOUSEEVENTF_WHEEL, 0, 0);
        event.Anonymous.mi.mouseData = delta as u32;
        inputs.push(event);
        if control {
            inputs.push(key(VK_CONTROL.0, true));
        }
        if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize
            != inputs.len()
        {
            return Err("Windows did not accept all guarded wheel inputs".into());
        }
        self.keys.clear();
        Ok(())
    }
    pub fn chord(&mut self, id: usize, codes: &[u16]) -> Result<(), String> {
        self.idle()?;
        if self.left || !self.foreground(id)? {
            return Err("Keyboard target does not own foreground or drag is active".into());
        }
        self.send_chord(codes)
    }
    /// Only an already owned, injected drag may receive these fixed QA keys.
    /// Recheck the actual button, foreground, hit target and capture before input.
    pub fn drag_key(&mut self, id: usize, action: DragKey) -> Result<(), String> {
        self.idle()?;
        let window = owned(id)?;
        let point = self
            .last
            .ok_or("Drag key has no guarded pointer position")?;
        let capture = unsafe { GetCapture() };
        if !self.left
            || unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16 & 0x8000 == 0
            || !self.foreground(id)?
            || !unsafe { IsWindowVisible(window) }.as_bool()
            || unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) } != window
            || (!capture.is_invalid() && capture != window)
        {
            return Err("Drag key lost its injected button or owned input target".into());
        }
        self.send_chord(match action {
            DragKey::Undo => &[VK_CONTROL.0, 0x5A],
            DragKey::Redo => &[VK_CONTROL.0, VK_SHIFT.0, 0x5A],
            DragKey::Finish => &[VK_RETURN.0],
            DragKey::NextFocus => &[VK_TAB.0],
            DragKey::Activate => &[VK_SPACE.0],
        })
    }
    fn send_chord(&mut self, codes: &[u16]) -> Result<(), String> {
        if codes.is_empty()
            || codes.len() > 6
            || codes
                .iter()
                .enumerate()
                .any(|(i, k)| *k == 0 || *k > 254 || codes[..i].contains(k))
        {
            return Err("Invalid probe key chord".into());
        }
        for code in codes {
            if unsafe { GetAsyncKeyState(i32::from(*code)) } as u16 & 0x8000 != 0 {
                return Err("Requested key is already held; no input sent".into());
            }
        }
        let inputs: Vec<_> = codes
            .iter()
            .map(|k| key(*k, false))
            .chain(codes.iter().rev().map(|k| key(*k, true)))
            .collect();
        self.keys = codes.to_vec();
        if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize
            != inputs.len()
        {
            return Err("Windows did not accept all guarded keyboard inputs".into());
        }
        self.keys.clear();
        Ok(())
    }
}
impl Drop for InputProbe {
    fn drop(&mut self) {
        let mut releases: Vec<_> = self.keys.iter().rev().map(|k| key(*k, true)).collect();
        releases.extend(
            self.unicode
                .iter()
                .rev()
                .map(|unit| unicode_key(*unit, true)),
        );
        if self.left {
            releases.push(mouse(MOUSEEVENTF_LEFTUP, 0, 0));
        }
        if !releases.is_empty() {
            unsafe { SendInput(&releases, std::mem::size_of::<INPUT>() as i32) };
        }
        if let Some(last) = self.last {
            let mut point = POINT::default();
            if unsafe { GetCursorPos(&mut point) }.is_ok()
                && point.x == last.x
                && point.y == last.y
                && owned(unsafe { GetForegroundWindow() }.0 as usize).is_ok()
            {
                let _ = unsafe { SetCursorPos(self.original.x, self.original.y) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreign_and_invalid_targets_are_rejected_before_any_input() {
        assert!(owned(0).is_err());
        assert!(owned(unsafe { GetDesktopWindow() }.0 as usize).is_err());
    }
}
