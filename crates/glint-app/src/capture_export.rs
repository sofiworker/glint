//! Bounded capture completion. Only the OS file picker runs on the UI thread.
use super::{CaptureProbe, OutputSlot, output, text};
use crate::messages::Message;
use glint_core::{Annotation, Edits, Rect, config::Output};
use slint::ComponentHandle;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Clone)]
struct FileReceipt {
    path: PathBuf,
    length: u64,
    modified: Option<SystemTime>,
}
impl FileReceipt {
    fn read(path: PathBuf) -> Result<Self, String> {
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
    fn current(&self) -> bool {
        std::fs::metadata(&self.path).is_ok_and(|m| {
            m.is_file() && m.len() == self.length && m.modified().ok() == self.modified
        })
    }
}

#[derive(Default)]
pub(super) struct Receipt {
    crop: Rect,
    marks: Vec<Arc<Annotation>>,
    automatic: Option<FileReceipt>,
    manual: Option<FileReceipt>,
}
impl Receipt {
    fn select_scene(&mut self, crop: Rect, edits: &Edits) {
        if self.crop != crop
            || self.marks.len() != edits.marks().len()
            || !self
                .marks
                .iter()
                .zip(edits.marks())
                .all(|(a, b)| Arc::ptr_eq(a, b))
        {
            *self = Self {
                crop,
                marks: edits.marks().to_vec(),
                ..Self::default()
            };
        }
    }
    fn saved_files(&self) -> Vec<PathBuf> {
        self.automatic
            .iter()
            .chain(self.manual.iter())
            .filter(|f| f.current())
            .map(|f| f.path.clone())
            .collect()
    }
}

pub(super) struct Request {
    pub pin_turns: i32,
    pub pin_payload: super::PinPayloadSlot,
    pub frame: Arc<glint_platform::DesktopFrame>,
    pub crop: Rect,
    pub edits: Edits,
    pub options: Output,
    pub save: bool,
    pub pin: bool,
    pub copy: bool,
    pub path: Option<PathBuf>,
    pub replacement: Option<glint_platform::ImageReplacement>,
    pub receipt: Arc<Mutex<Receipt>>,
    pub recover_automatic: bool,
}
struct Dialog {
    directory: PathBuf,
    filename: String,
}
fn output_directory(options: &Output) -> Result<PathBuf, String> {
    Ok(if options.directory.is_empty() {
        glint_platform::pictures_directory()?.join("Glint")
    } else {
        PathBuf::from(&options.directory)
    })
}
fn output_filename(request: &Request) -> Result<String, String> {
    let (date, time) = glint_platform::local_output_timestamp();
    glint_core::output::filename(
        &request.options,
        &date,
        &time,
        request.crop.width,
        request.crop.height,
    )
    .map_err(str::to_owned)
}
fn destination(request: &Request) -> Result<PathBuf, String> {
    Ok(output_directory(&request.options)?.join(output_filename(request)?))
}
fn prepare_dialog(
    request: &Request,
    resolve_directory: impl FnOnce(&Output) -> Result<PathBuf, String>,
) -> Result<Dialog, String> {
    let filename = output_filename(request)?;
    // Recovery must not depend on the folder that caused the original failure,
    // including an unavailable Windows Pictures known folder.
    let directory = if request.recover_automatic {
        PathBuf::new()
    } else {
        let directory = resolve_directory(&request.options)?;
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        directory
    };
    Ok(Dialog {
        directory,
        filename,
    })
}

pub(super) fn start(ui: &CaptureProbe, request: Request, slot: OutputSlot) {
    ui.set_output_error_open(false);
    ui.set_output_retry_action(
        if request.recover_automatic {
            "save-without-auto"
        } else if request.save {
            "save"
        } else if request.pin && request.copy {
            "copy-and-pin"
        } else if request.pin {
            "pin"
        } else {
            "copy"
        }
        .into(),
    );
    ui.set_busy(true);
    ui.set_status(text(ui, Message::CaptureOutputBusy).into());
    if !request.save || request.path.is_some() {
        spawn_output(ui, request, slot);
        return;
    }
    let weak = ui.as_weak();
    let generation = ui.get_save_dialog_generation();
    let worker = std::thread::Builder::new()
        .name("glint-save-destination".into())
        .spawn(move || {
            let prepared = prepare_dialog(&request, output_directory);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                // Esc may have closed this editor while directory preparation was running.
                if !dialog_owner_ready(&ui, generation) {
                    return;
                }
                let chosen = prepared.and_then(|dialog| choose(&ui, &dialog, &request.options));
                if !dialog_owner_ready(&ui, generation) {
                    return;
                }
                match chosen {
                    Ok(Some(destination)) => {
                        let mut request = request;
                        request.path = Some(destination.path);
                        request.replacement = destination.replacement;
                        spawn_output(&ui, request, slot);
                    }
                    Ok(None) => {
                        ui.set_busy(false);
                        ui.set_status(text(&ui, Message::CaptureSaveCancelled).into());
                    }
                    Err(error) => {
                        // An explicit alternate save opens in the OS default folder, bypassing
                        // this failed initial directory without changing saved preferences.
                        ui.set_auto_save_failed(true);
                        failed(
                            &ui,
                            format!("{}: {error}", text(&ui, Message::CaptureDialogFailed)),
                        );
                    }
                }
            });
        });
    if let Err(error) = worker {
        worker_failed(ui, error);
    }
}

fn dialog_owner_ready(ui: &CaptureProbe, generation: i32) -> bool {
    if ui.get_closed() {
        return false;
    }
    if !ui.window().is_visible() || ui.get_save_dialog_generation() != generation {
        // Lock/sleep may hide and retain the editor. Abandon the pending chooser,
        // including after a quick resume, but leave a usable editor for retry.
        ui.set_busy(false);
        ui.set_status(text(ui, Message::CaptureSaveCancelled).into());
        return false;
    }
    true
}

fn worker_failed(ui: &CaptureProbe, error: std::io::Error) {
    failed(
        ui,
        format!("{}: {error}", text(ui, Message::CaptureOutputFailed)),
    );
}

pub(super) fn failed(ui: &CaptureProbe, details: impl Into<slint::SharedString>) {
    if ui.get_closed() {
        return;
    }
    let details = details.into();
    ui.invoke_output_failed();
    ui.set_busy(false);
    ui.set_status(details.clone());
    ui.set_output_error_details(details);
    if ui.window().is_visible() {
        ui.invoke_focus_canvas();
    }
    ui.set_output_error_open(true);
    if ui.window().is_visible() {
        ui.invoke_focus_output_error();
    }
}

#[cfg(windows)]
fn choose(
    ui: &CaptureProbe,
    dialog: &Dialog,
    options: &Output,
) -> Result<Option<glint_platform::ImageDestination>, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::WinitWindowAccessor;
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Not a Win32 window".into());
            };
            unsafe {
                glint_platform::choose_image_destination(
                    handle.hwnd.get() as *mut std::ffi::c_void,
                    &dialog.filename,
                    &dialog.directory,
                    options.format == glint_core::config::ImageFormat::Jpeg,
                    options.file_conflict == glint_core::config::FileConflict::Confirm,
                    glint_platform::ReplacementPrompt {
                        title: text(ui, Message::OutputReplaceTitle),
                        question: text(ui, Message::OutputReplaceQuestion),
                        hint: text(ui, Message::OutputReplaceHint),
                    },
                )
            }
        })
        .ok_or_else(|| "Capture window unavailable".to_owned())
        .and_then(|result| result)
}

fn spawn_output(ui: &CaptureProbe, request: Request, slot: OutputSlot) {
    // Validate a chosen extension before creating any automatic or manual output.
    if request.save
        && request
            .path
            .as_deref()
            .is_none_or(|p| glint_core::output::format_from_path(p).is_err())
    {
        failed(ui, text(ui, Message::CaptureImageFormats));
        return;
    }
    let weak = ui.as_weak();
    let worker = std::thread::Builder::new()
        .name("glint-capture-output".into())
        .spawn(move || {
            let _slot = slot;
            let result = execute(&request, glint_platform::copy_rgba).and_then(|result| {
                result
                    .map(|pixels| pin_payload(&request, pixels))
                    .transpose()
                    .map_err(|error| {
                        let saved = request
                            .receipt
                            .lock()
                            .map(|receipt| receipt.saved_files())
                            .unwrap_or_default();
                        (error, saved, false)
                    })
            });
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if ui.get_closed() {
                    return;
                }
                ui.set_busy(false);
                match result {
                    Ok(Some(payload)) => {
                        ui.set_pending_region(super::WindowRegion {
                            left: request.crop.x as f32,
                            top: request.crop.y as f32,
                            wide: request.crop.width as f32,
                            high: request.crop.height as f32,
                        });
                        let image = slint::Image::from_rgba8(payload.display.clone());
                        let Ok(mut pending) = request.pin_payload.lock() else {
                            failed(&ui, text(&ui, Message::PinFailed));
                            return;
                        };
                        *pending = Some(payload);
                        drop(pending);
                        let x = (i64::from(request.frame.x) + i64::from(request.crop.x))
                            .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                            as i32;
                        let y = (i64::from(request.frame.y) + i64::from(request.crop.y))
                            .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                            as i32;
                        ui.set_busy(true);
                        ui.set_status(text(&ui, Message::PinPreparing).into());
                        ui.invoke_request_pin(image, x, y);
                    }
                    Ok(None) => {
                        ui.set_completion_kind(if request.save { 2 } else { 1 });
                        ui.invoke_output_succeeded(
                            request.crop.x as i32,
                            request.crop.y as i32,
                            request.crop.width as i32,
                            request.crop.height as i32,
                        );
                        ui.invoke_finished();
                    }
                    Err((error, saved, automatic_failed)) => {
                        if automatic_failed {
                            ui.set_auto_save_failed(true);
                        }
                        let mut status =
                            format!("{}: {error}", text(&ui, Message::CaptureOutputFailed));
                        if !saved.is_empty() {
                            status.push_str(&format!(
                                " · {}: {}",
                                text(&ui, Message::CaptureFilesSaved),
                                saved
                                    .iter()
                                    .map(|p| p.display().to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                        }
                        failed(&ui, status);
                    }
                }
            });
        });
    if let Err(error) = worker {
        worker_failed(ui, error);
    }
}

fn pin_payload(request: &Request, pixels: Vec<u8>) -> Result<super::PinPayload, String> {
    let document = glint_core::ImageDocument::from_selection(
        &request.frame.rgba,
        request.frame.width,
        request.frame.height,
        request.crop,
        &request.edits,
    )
    .map_err(str::to_owned)?;
    let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        &pixels,
        request.crop.width,
        request.crop.height,
    );
    let turns = request.pin_turns.rem_euclid(4);
    let display = if turns == 0 {
        pixels.clone()
    } else {
        let (w, h) = if turns % 2 == 1 {
            (pixels.height(), pixels.width())
        } else {
            (pixels.width(), pixels.height())
        };
        let mut rotated = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
        glint_core::pins::rotate_rgba(
            pixels.as_bytes(),
            rotated.make_mut_bytes(),
            pixels.width(),
            pixels.height(),
            turns as u8,
        )
        .map_err(str::to_owned)?;
        rotated
    };
    Ok(super::PinPayload {
        document: Arc::new(document),
        pixels,
        display,
        turns,
    })
}

type Failure = (String, Vec<PathBuf>, bool);
fn execute(
    request: &Request,
    copy: impl FnOnce(u32, u32, Vec<u8>) -> Result<(), String>,
) -> Result<Option<Vec<u8>>, Failure> {
    let mut receipt = request
        .receipt
        .lock()
        .map_err(|_| ("Output receipt unavailable".into(), vec![], false))?;
    receipt.select_scene(request.crop, &request.edits);
    let mut automatic_failed = false;
    let result = (|| {
        let pixels = glint_core::composite_selection(
            &request.frame.rgba,
            request.frame.width,
            request.frame.height,
            request.crop,
            &request.edits,
        )?;
        if request.options.auto_save
            && !receipt.automatic.as_ref().is_some_and(FileReceipt::current)
        {
            automatic_failed = true;
            let path = destination(request)?;
            std::fs::create_dir_all(path.parent().ok_or("Missing automatic output directory")?)
                .map_err(|e| e.to_string())?;
            let path = output::save_numbered(
                &path,
                &pixels,
                request.crop.width,
                request.crop.height,
                request.options.format,
                request.options.quality,
            )?;
            receipt.automatic = Some(FileReceipt::read(path)?);
            automatic_failed = false;
        }
        if request.save {
            let path = request.path.as_ref().ok_or("No output path selected")?;
            let reusable = receipt
                .manual
                .iter()
                .chain(receipt.automatic.iter())
                .any(|f| &f.path == path && f.current());
            if !reusable {
                let actual = output::save_manual(
                    path,
                    &pixels,
                    request.crop.width,
                    request.crop.height,
                    &request.options,
                    request.replacement.as_ref(),
                )?;
                receipt.manual = Some(FileReceipt::read(actual)?);
            }
        } else if request.copy && !request.pin {
            copy(request.crop.width, request.crop.height, pixels)?;
            return Ok(None);
        } else if request.copy {
            copy(request.crop.width, request.crop.height, pixels.clone())?;
        }
        Ok::<_, String>(request.pin.then_some(pixels))
    })();
    result.map_err(|e| (e, receipt.saved_files(), automatic_failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_error_details_and_escape_preserve_the_capture_scene() {
        struct Backend;
        impl slint::platform::Platform for Backend {
            fn create_window_adapter(
                &self,
            ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError>
            {
                Ok(
                    slint::platform::software_renderer::MinimalSoftwareWindow::new(
                        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
                    ),
                )
            }
        }
        slint::platform::set_platform(Box::new(Backend)).unwrap();
        let ui = CaptureProbe::new().unwrap();
        let frame = Arc::new(glint_platform::DesktopFrame {
            x: -200,
            y: -100,
            width: 64,
            height: 64,
            rgba: vec![255; 64 * 64 * 4],
        });
        let state = super::super::prepare_capture(&ui, frame, 16, None, || {});
        ui.show().unwrap();
        ui.invoke_pointer(0, 4., 4.);
        ui.invoke_pointer(2, 60., 60.);
        ui.set_tool(1);
        ui.invoke_pointer(0, 10., 10.);
        ui.invoke_pointer(2, 30., 30.);
        assert_eq!(ui.get_region_metrics(), "56 × 56 px · (-196, -96)");
        let selection = state.borrow().selection;
        let marks = state.borrow().edits.marks().to_vec();
        let (directory, mut request) = fixture();
        std::fs::create_dir_all(&directory).unwrap();
        let blocker = directory.join("not-a-directory");
        std::fs::write(&blocker, b"keep").unwrap();
        request.options.directory = blocker.to_string_lossy().into_owned();
        let (error, _, _) =
            execute(&request, |_, _, _| panic!("Failed save must not copy")).unwrap_err();
        ui.set_busy(true);
        failed(&ui, error.clone());
        assert!(ui.get_output_error_open());
        assert_eq!(ui.get_output_error_details(), error);
        assert!(!ui.get_background_active());
        assert!(!ui.get_busy());
        ui.invoke_action("undo".into());
        assert_eq!(state.borrow().edits.marks(), marks);
        use slint::platform::{Key, WindowEvent};
        for closed in [false, true] {
            ui.window().dispatch_event(WindowEvent::KeyPressed {
                text: Key::Escape.into(),
            });
            ui.window().dispatch_event(WindowEvent::KeyReleased {
                text: Key::Escape.into(),
            });
            assert!(!ui.get_output_error_open());
            assert_eq!(ui.get_closed(), closed);
            assert_eq!(state.borrow().selection, selection);
            assert_eq!(state.borrow().edits.marks(), marks);
        }
        assert_eq!(std::fs::read(blocker).unwrap(), b"keep");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn resumed_save_dialog_rejects_old_generation_and_allows_retry() {
        struct Backend;
        impl slint::platform::Platform for Backend {
            fn create_window_adapter(
                &self,
            ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError>
            {
                Ok(
                    slint::platform::software_renderer::MinimalSoftwareWindow::new(
                        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
                    ),
                )
            }
        }
        slint::platform::set_platform(Box::new(Backend)).unwrap();
        let ui = CaptureProbe::new().unwrap();
        ui.show().unwrap();
        ui.set_busy(true);
        let pending = ui.get_save_dialog_generation();
        ui.hide().unwrap();
        ui.set_save_dialog_generation(pending.wrapping_add(1));
        ui.show().unwrap();
        // Both the directory worker and the native picker can return after
        // resume. A visible owner does not make their old request valid again.
        assert!(!dialog_owner_ready(&ui, pending));
        assert!(!ui.get_busy());
        assert_eq!(ui.get_status(), text(&ui, Message::CaptureSaveCancelled));
        assert!(!ui.get_closed());
        ui.set_busy(true);
        assert!(dialog_owner_ready(&ui, ui.get_save_dialog_generation()));
        assert!(ui.get_busy());
        ui.set_closed(true);
        ui.set_status("closed owner".into());
        assert!(!dialog_owner_ready(&ui, pending));
        assert!(ui.get_busy());
        assert_eq!(ui.get_status(), "closed owner");
    }

    fn fixture() -> (PathBuf, Request) {
        let directory = std::env::temp_dir().join(format!(
            "glint-completion-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let request = Request {
            pin_turns: 0,
            pin_payload: Default::default(),
            frame: Arc::new(glint_platform::DesktopFrame {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
                rgba: [241, 12, 222, 255].repeat(16 * 16),
            }),
            crop: Rect {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            edits: Edits::default(),
            options: Output {
                auto_save: true,
                directory: directory.to_string_lossy().into_owned(),
                filename: "Shot".into(),
                ..Output::default()
            },
            save: false,
            pin: false,
            copy: true,
            path: None,
            replacement: None,
            receipt: Default::default(),
            recover_automatic: false,
        };
        (directory, request)
    }

    #[test]
    fn alternate_save_bypasses_unavailable_folder_and_preserves_preferences() {
        let (directory, mut request) = fixture();
        request.options.directory.clear();
        let unavailable = |_: &Output| Err("Pictures known folder unavailable".into());
        assert_eq!(
            prepare_dialog(&request, unavailable).err().unwrap(),
            "Pictures known folder unavailable"
        );
        request.recover_automatic = true;
        let options = request.options.clone();
        let dialog = prepare_dialog(&request, |_| {
            panic!("Alternate save must not resolve the failed directory")
        })
        .unwrap();
        assert!(dialog.directory.as_os_str().is_empty());
        assert_eq!(dialog.filename, "Shot.png");
        assert_eq!(request.options, options);
        assert!(!directory.exists());

        // After the chooser returns a new destination, the explicit recovery
        // request skips the automatic copy and writes the retained scene there.
        std::fs::create_dir(&directory).unwrap();
        let blocked = directory.join("blocked");
        std::fs::write(&blocked, b"unrelated file").unwrap();
        request.options.directory = blocked.to_string_lossy().into_owned();
        request.options.auto_save = false;
        request.save = true;
        request.path = Some(directory.join("Recovered.png"));
        request.edits.push(glint_core::Redaction {
            bounds: request.crop,
            color: [0, 0, 0, 255],
        });
        execute(&request, |_, _, _| panic!("Save must not copy")).unwrap();
        assert_eq!(std::fs::read(&blocked).unwrap(), b"unrelated file");
        assert_eq!(
            image::open(request.path.as_ref().unwrap())
                .unwrap()
                .to_rgba8()
                .as_raw(),
            &[0, 0, 0, 255].repeat(16 * 16)
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn partial_copy_failure_retries_reuse_file_and_edits_require_new_pixels() {
        let (directory, mut request) = fixture();
        let fail = |_, _, _| Err("clipboard occupied".into());
        let error = execute(&request, fail).unwrap_err();
        assert_eq!(error.1, vec![directory.join("Shot.png")]);
        assert_eq!(
            image::open(&error.1[0])
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [241, 12, 222, 255]
        );
        execute(&request, |_, _, pixels| {
            assert_eq!(pixels[0], 241);
            Ok(())
        })
        .unwrap();
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        request.edits.push(glint_core::Redaction {
            bounds: request.crop,
            color: [0, 0, 0, 255],
        });
        execute(&request, fail).unwrap_err();
        let changed = directory.join("Shot_1.png");
        assert_eq!(
            image::open(changed).unwrap().to_rgba8().get_pixel(0, 0).0,
            [0, 0, 0, 255]
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        // Deleted receipts must not count as a saved copy on retry.
        std::fs::remove_file(directory.join("Shot_1.png")).unwrap();
        execute(&request, |_, _, _| Ok(())).unwrap();
        assert!(directory.join("Shot_1.png").exists());
        // An externally changed file must not be mistaken for this image's saved receipt.
        std::fs::write(directory.join("Shot_1.png"), b"external change").unwrap();
        let changed = execute(&request, fail).unwrap_err();
        assert_eq!(changed.1, vec![directory.join("Shot_2.png")]);
        assert_eq!(
            std::fs::read(directory.join("Shot_1.png")).unwrap(),
            b"external change"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn pin_only_retains_flat_pixels_without_copy_and_copy_and_pin_waits_for_copy_success() {
        let (directory, mut request) = fixture();
        request.pin = true;
        request.copy = false;
        let pixels = execute(&request, |_, _, _| {
            panic!("Pin-only must not write clipboard")
        })
        .unwrap()
        .unwrap();
        assert_eq!(pixels[0..4], [241, 12, 222, 255]);
        request.copy = true;
        let failure = execute(&request, |_, _, _| Err("clipboard occupied".into())).unwrap_err();
        assert_eq!(failure.1, vec![directory.join("Shot.png")]);
        let pinned = execute(&request, |w, h, p| {
            assert_eq!((w, h, p.len()), (16, 16, 1024));
            Ok(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(pinned, pixels);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn pin_payload_keeps_raw_selection_and_history_while_file_is_flattened() {
        let (directory, mut request) = fixture();
        request.pin = true;
        request.copy = false;
        request.crop = Rect {
            x: 3,
            y: 4,
            width: 8,
            height: 6,
        };
        request.edits.push(glint_core::Redaction {
            bounds: Rect {
                x: 1,
                y: 1,
                width: 3,
                height: 2,
            },
            color: [0, 0, 0, 255],
        });
        let pixels = execute(&request, |_, _, _| panic!("Pin-only must not copy"))
            .unwrap()
            .unwrap();
        let payload = pin_payload(&request, pixels).unwrap();
        assert_eq!(payload.document.dimensions(), (8, 6));
        assert_eq!(payload.document.source(), [241, 12, 222, 255].repeat(8 * 6));
        assert_eq!(
            &payload.pixels.as_bytes()[(8 + 1) * 4..(8 + 2) * 4],
            &[0, 0, 0, 255]
        );
        let file = image::open(directory.join("Shot.png")).unwrap().to_rgba8();
        assert_eq!(file.as_raw(), payload.pixels.as_bytes());
        let mut restored = payload.document.edits().clone();
        restored.undo();
        assert!(restored.marks().is_empty());
        assert_eq!(payload.document.edits().marks().len(), 1);
        assert_eq!(request.edits.marks().len(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn confirmed_manual_retry_reuses_automatic_copy_and_composites_before_replacement() {
        let (directory, mut request) = fixture();
        std::fs::create_dir_all(&directory).unwrap();
        let existing = directory.join("Manual.png");
        std::fs::write(&existing, b"prior destination").unwrap();
        request.save = true;
        request.path = Some(existing.clone());
        request.options.file_conflict = glint_core::config::FileConflict::Confirm;
        request.edits.push(glint_core::Redaction {
            bounds: request.crop,
            color: [0, 0, 0, 255],
        });
        let failed = execute(&request, |_, _, _| panic!("Save must not copy")).unwrap_err();
        assert_eq!(failed.1, vec![directory.join("Shot.png")]);
        assert_eq!(std::fs::read(&existing).unwrap(), b"prior destination");
        request.replacement = Some(glint_platform::ImageReplacement::observe(&existing).unwrap());
        execute(&request, |_, _, _| panic!("Save must not copy")).unwrap();
        assert_eq!(
            image::open(&existing).unwrap().to_rgba8().as_raw(),
            &[0, 0, 0, 255].repeat(16 * 16)
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        std::fs::remove_file(existing).unwrap();
        std::fs::remove_file(directory.join("Shot.png")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn failed_automatic_file_never_copies_and_manual_collision_can_be_retried_elsewhere() {
        let (directory, mut request) = fixture();
        std::fs::create_dir_all(&directory).unwrap();
        let blocker = directory.join("not-directory");
        std::fs::write(&blocker, b"preserve").unwrap();
        request.options.directory = blocker.to_string_lossy().into_owned();
        let failed =
            execute(&request, |_, _, _| panic!("File failure must stop copy")).unwrap_err();
        assert!(failed.1.is_empty());
        assert!(failed.2);
        request.options.directory = directory.to_string_lossy().into_owned();
        request.save = true;
        let existing = directory.join("Manual.png");
        std::fs::write(&existing, b"original").unwrap();
        request.path = Some(existing.clone());
        let failed = execute(&request, |_, _, _| panic!("Save must not copy")).unwrap_err();
        assert_eq!(failed.1, vec![directory.join("Shot.png")]);
        assert_eq!(std::fs::read(existing).unwrap(), b"original");
        request.path = Some(directory.join("Retry.jpg"));
        execute(&request, |_, _, _| panic!("Save must not copy")).unwrap();
        assert_eq!(
            image::open(request.path.as_ref().unwrap()).unwrap().width(),
            16
        );
        assert!(!directory.join("Shot_1.png").exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
