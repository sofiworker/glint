//! Thread-owned RegisterHotKey registrations, blocking OS message loop, transactional replacement.
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

/// Win32 virtual key and modifier bits; no Win32 types escape this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub action: u32,
    pub modifiers: u32,
    pub key: u32,
}

#[derive(Debug)]
pub enum HotkeyEvent {
    Pressed(u32),
    Error(String),
}

enum Request {
    Replace(Vec<HotkeyBinding>, mpsc::SyncSender<Result<(), String>>),
    Snapshot(mpsc::SyncSender<Vec<HotkeyBinding>>),
}

struct Registrations(Vec<(i32, HotkeyBinding)>);

impl Drop for Registrations {
    fn drop(&mut self) {
        for (id, _) in &self.0 {
            // Registrations are only constructed, changed and dropped on their owner thread.
            unsafe {
                let _ = UnregisterHotKey(HWND::default(), *id);
            }
        }
    }
}

impl Registrations {
    fn replace(&mut self, bindings: Vec<HotkeyBinding>) -> Result<(), String> {
        let mut actions = HashSet::new();
        let mut chords = HashSet::new();
        if bindings.len() > 64 {
            return Err("At most 64 hotkeys can be registered".into());
        }
        for binding in &bindings {
            if binding.key == 0 || binding.key > 254 || binding.modifiers & !15 != 0 {
                return Err(format!("Invalid hotkey for action {}", binding.action));
            }
            if !actions.insert(binding.action) || !chords.insert((binding.modifiers, binding.key)) {
                return Err("Duplicate hotkey or action; previous bindings are unchanged".into());
            }
        }
        let same_chord =
            |a: &HotkeyBinding, b: &HotkeyBinding| a.key == b.key && a.modifiers == b.modifiers;
        let mut fresh = Registrations(Vec::new());
        let mut next = Vec::new();
        for binding in bindings {
            if let Some((id, _)) = self.0.iter().find(|(_, old)| same_chord(old, &binding)) {
                next.push((*id, binding));
                continue;
            }
            let id = (1..=0xbfff)
                .find(|id| {
                    !self
                        .0
                        .iter()
                        .chain(fresh.0.iter())
                        .any(|(used, _)| used == id)
                })
                .ok_or("No hotkey registration IDs available")?;
            // Keep every old registration until all new chords have been acquired.
            unsafe {
                RegisterHotKey(
                    HWND::default(),
                    id,
                    HOT_KEY_MODIFIERS(binding.modifiers) | MOD_NOREPEAT,
                    binding.key,
                )
                .map_err(|e| {
                    format!(
                        "Hotkey action {} could not be registered: {e}; previous bindings retained",
                        binding.action
                    )
                })?;
            }
            fresh.0.push((id, binding));
            next.push((id, binding));
        }
        for (id, _) in &self.0 {
            if !next.iter().any(|(kept, _)| kept == id) {
                unsafe {
                    let _ = UnregisterHotKey(HWND::default(), *id);
                }
            }
        }
        // Ownership of the new registrations passes to self; failed acquisitions above drop only fresh.
        fresh.0.clear();
        self.0 = next;
        Ok(())
    }
}

const COMMAND: u32 = WM_APP + 7;

pub struct HotkeyService {
    stopping: Arc<AtomicBool>,
    thread_id: u32,
    requests: mpsc::SyncSender<Request>,
    owner: Option<thread::JoinHandle<()>>,
}

impl HotkeyService {
    pub fn new(
        bindings: Vec<HotkeyBinding>,
        handler: impl Fn(HotkeyEvent) + Send + 'static,
    ) -> Result<Self, String> {
        let (requests, receiver) = mpsc::sync_channel(1);
        let (ready, response) = mpsc::sync_channel(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let shutting_down = stopping.clone();
        let owner = thread::Builder::new()
            .name("glint-hotkeys".into())
            .spawn(move || {
                let mut message = MSG::default();
                // Force creation of this thread's message queue before exposing its ID.
                unsafe {
                    let _ = PeekMessageW(&mut message, HWND::default(), 0, 0, PM_NOREMOVE);
                }
                let mut registrations = Registrations(Vec::new());
                match registrations.replace(bindings) {
                    Ok(()) => {
                        let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                }
                loop {
                    let result = unsafe { GetMessageW(&mut message, HWND::default(), 0, 0) }.0;
                    if result == 0 {
                        if !shutting_down.load(Ordering::Acquire) {
                            handler(HotkeyEvent::Error(
                                "Hotkey message loop stopped unexpectedly".into(),
                            ));
                        }
                        break;
                    }
                    if result == -1 {
                        handler(HotkeyEvent::Error("Hotkey GetMessageW failed".into()));
                        break;
                    }
                    if message.message == COMMAND {
                        match receiver.recv() {
                            Ok(Request::Replace(bindings, response)) => {
                                let _ = response.send(registrations.replace(bindings));
                            }
                            Ok(Request::Snapshot(response)) => {
                                let _ = response.send(
                                    registrations
                                        .0
                                        .iter()
                                        .map(|(_, binding)| *binding)
                                        .collect(),
                                );
                            }
                            Err(_) => break,
                        }
                    } else if message.message == WM_HOTKEY
                        && let Some((_, binding)) = registrations
                            .0
                            .iter()
                            .find(|(id, _)| *id as usize == message.wParam.0)
                    {
                        // Ignore a queued event for an ID reused with a different chord.
                        let key = (message.lParam.0 as u32 >> 16) & 0xffff;
                        let modifiers = message.lParam.0 as u32 & 15;
                        if key == binding.key && modifiers == binding.modifiers {
                            handler(HotkeyEvent::Pressed(binding.action));
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        match response.recv().map_err(|e| e.to_string())? {
            Ok(thread_id) => Ok(Self {
                stopping,
                thread_id,
                requests,
                owner: Some(owner),
            }),
            Err(error) => {
                let _ = owner.join();
                Err(error)
            }
        }
    }

    /// Query the live owner thread's actual registrations, including transactional rollback.
    pub fn bindings(&self) -> Result<Vec<HotkeyBinding>, String> {
        if unsafe { GetCurrentThreadId() } == self.thread_id {
            return Err("Hotkey query cannot block its own thread".into());
        }
        let (response, result) = mpsc::sync_channel(1);
        self.requests
            .try_send(Request::Snapshot(response))
            .map_err(|e| e.to_string())?;
        unsafe { PostThreadMessageW(self.thread_id, COMMAND, WPARAM(0), LPARAM(0)) }
            .map_err(|e| e.to_string())?;
        result.recv().map_err(|e| e.to_string())
    }

    /// Register new chords first. Any conflict leaves the complete previous set registered.
    pub fn replace(&self, bindings: Vec<HotkeyBinding>) -> Result<(), String> {
        if unsafe { GetCurrentThreadId() } == self.thread_id {
            return Err("Hotkey replacement cannot block its own event-handler thread".into());
        }
        let (response, result) = mpsc::sync_channel(1);
        self.requests
            .try_send(Request::Replace(bindings, response))
            .map_err(|e| e.to_string())?;
        unsafe { PostThreadMessageW(self.thread_id, COMMAND, WPARAM(0), LPARAM(0)) }
            .map_err(|e| e.to_string())?;
        result.recv().map_err(|e| e.to_string())?
    }
}

impl Drop for HotkeyService {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let quit_posted =
            unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.is_ok();
        if let Some(owner) = self.owner.take() {
            if unsafe { GetCurrentThreadId() } != self.thread_id
                && (quit_posted || owner.is_finished())
            {
                let _ = owner.join();
            } else if !quit_posted && !owner.is_finished() {
                // Avoid hanging the UI on a failed OS wakeup. Process termination remains the final cleanup.
                eprintln!("Glint: unable to wake hotkey owner for shutdown");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stopped_loop_reports_error_and_snapshot_disconnects() {
        let (send, receive) = mpsc::sync_channel(2);
        let service = HotkeyService::new(vec![], move |event| {
            let _ = send.try_send(event);
        })
        .unwrap();
        assert!(service.bindings().unwrap().is_empty());
        unsafe { PostThreadMessageW(service.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.unwrap();
        assert!(matches!(
            receive
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::Error(_)
        ));
        assert!(service.bindings().is_err());
    }
    #[test]
    #[ignore = "Registers real Windows Ctrl+Alt+Shift+F21/F22/F23 bindings temporarily"]
    fn real_registration_conflict_rollback_reassignment_and_release() {
        let chord = |action, key| HotkeyBinding {
            action,
            modifiers: 7,
            key,
        };
        let a = chord(11, 0x84); // F21
        let b = chord(22, 0x85); // F22
        let c = chord(33, 0x86); // F23
        let (events, received) = mpsc::sync_channel(4);
        let service = HotkeyService::new(vec![a], move |event| {
            let _ = events.try_send(event);
        })
        .unwrap();
        let blocker = HotkeyService::new(vec![c], |_| {}).unwrap();
        assert_eq!(service.bindings().unwrap(), vec![a]);
        // A fresh chord is acquired before a later conflict. Both it and the failed chord must roll back.
        assert!(service.replace(vec![b, c]).is_err());
        assert_eq!(service.bindings().unwrap(), vec![a]);
        assert!(
            HotkeyService::new(vec![a], |_| {}).is_err(),
            "old chord was lost on conflict"
        );
        let temporary =
            HotkeyService::new(vec![b], |_| {}).expect("partially registered chord was leaked");
        drop(temporary);
        assert!(service.replace(vec![a, a]).is_err());
        service.replace(vec![a, b]).unwrap();
        // Swap action mappings without unregistering their already acquired chords.
        service
            .replace(vec![chord(22, a.key), chord(11, b.key)])
            .unwrap();
        unsafe {
            PostThreadMessageW(
                service.thread_id,
                WM_HOTKEY,
                WPARAM(1),
                LPARAM(((a.key << 16) | 7) as isize),
            )
            .unwrap();
        }
        assert!(matches!(
            received
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::Pressed(22)
        ));
        drop(service);
        drop(blocker);
        let reacquired =
            HotkeyService::new(vec![a, b, c], |_| {}).expect("drop leaked a registered hotkey");
        drop(reacquired);
    }
}
