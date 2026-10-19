//! Pin output shares capture's global task slot and lossless source pixels.
use crate::{
    messages::{self, Message},
    pins::DesktopPin,
    probe::{OutputSlot, output},
};
use glint_core::config::Output;
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer, winit_030::WinitWindowAccessor};
use std::path::PathBuf;

pub type DiagnosticChoice = Option<Option<PathBuf>>;
fn tr(ui: &DesktopPin, id: Message) -> &'static str {
    messages::text(id, ui.get_english())
}
pub fn failed(ui: &DesktopPin, error: String) {
    ui.set_output_error(true);
    ui.set_busy(false);
    ui.set_status(tr(ui, Message::PinOutputFailed).into());
    ui.set_details(format!("{}\n\n{error}", tr(ui, Message::PinOutputFailed)).into());
    ui.set_message_open(true);
}
fn finish(
    ui: &DesktopPin,
    result: Result<Option<PathBuf>, String>,
    save: bool,
    feedback: glint_core::config::Feedback,
) {
    ui.set_busy(false);
    match result {
        Ok(Some(path)) => {
            crate::completion::show(
                feedback,
                2,
                ui.get_english(),
                ui.get_dark(),
                ui.get_accent(),
                ui.get_interface_scale(),
                ui.get_reduce_motion(),
            );
            ui.set_status(tr(ui, Message::PinSaved).into());
            ui.set_details(path.display().to_string().into());
        }
        Ok(None) => {
            if !save {
                crate::completion::show(
                    feedback,
                    1,
                    ui.get_english(),
                    ui.get_dark(),
                    ui.get_accent(),
                    ui.get_interface_scale(),
                    ui.get_reduce_motion(),
                );
            }
            ui.set_status(
                tr(
                    ui,
                    if save {
                        Message::PinOutputCancelled
                    } else {
                        Message::PinCopied
                    },
                )
                .into(),
            );
            ui.set_details("".into());
        }
        Err(error) => failed(ui, error),
    }
}
pub fn start(
    ui: &DesktopPin,
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    options: Output,
    save: bool,
    elsewhere: bool,
    choice: DiagnosticChoice,
) {
    if ui.get_busy() {
        return;
    }
    crate::completion::dismiss();
    ui.set_last_save(save);
    let Some(slot) = OutputSlot::acquire() else {
        failed(ui, tr(ui, Message::CaptureOutputPending).into());
        return;
    };
    ui.set_busy(true);
    ui.set_output_error(false);
    ui.set_message_open(false);
    ui.set_menu_open(false);
    ui.set_status(tr(ui, Message::CaptureOutputBusy).into());
    if !save || choice.is_some() {
        match choice {
            Some(None) if save => finish(ui, Ok(None), true, options.feedback),
            _ => write(ui, pixels, options, choice.flatten(), None, slot),
        }
        return;
    }
    let weak = ui.as_weak();
    let result = std::thread::Builder::new()
        .name("glint-pin-destination".into())
        .spawn(move || {
            let prepared = (|| {
                let (date, time) = glint_platform::local_output_timestamp();
                let filename = glint_core::output::filename(
                    &options,
                    &date,
                    &time,
                    pixels.width(),
                    pixels.height(),
                )?;
                let directory = if elsewhere {
                    PathBuf::new()
                } else {
                    let directory = if options.directory.is_empty() {
                        glint_platform::pictures_directory()?.join("Glint")
                    } else {
                        PathBuf::from(&options.directory)
                    };
                    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                    directory
                };
                Ok::<_, String>((filename, directory))
            })();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if !ui.window().is_visible() {
                    finish(&ui, Ok(None), true, options.feedback);
                    return;
                }
                let chosen =
                    prepared.and_then(|(name, directory)| choose(&ui, &name, &directory, &options));
                if !ui.window().is_visible() {
                    finish(&ui, Ok(None), true, options.feedback);
                    return;
                }
                match chosen {
                    Ok(Some(destination)) => write(
                        &ui,
                        pixels,
                        options,
                        Some(destination.path),
                        destination.replacement,
                        slot,
                    ),
                    Ok(None) => finish(&ui, Ok(None), true, options.feedback),
                    Err(error) => failed(&ui, error),
                }
            });
        });
    if let Err(error) = result {
        failed(ui, error.to_string());
    }
}
fn choose(
    ui: &DesktopPin,
    name: &str,
    directory: &std::path::Path,
    options: &Output,
) -> Result<Option<glint_platform::ImageDestination>, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    ui.window()
        .with_winit_window(|w| {
            let handle = w.window_handle().map_err(|e| e.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Not a Win32 pin".into());
            };
            unsafe {
                glint_platform::choose_image_destination(
                    handle.hwnd.get() as *mut std::ffi::c_void,
                    name,
                    directory,
                    options.format == glint_core::config::ImageFormat::Jpeg,
                    options.file_conflict == glint_core::config::FileConflict::Confirm,
                    glint_platform::ReplacementPrompt {
                        title: tr(ui, Message::OutputReplaceTitle),
                        question: tr(ui, Message::OutputReplaceQuestion),
                        hint: tr(ui, Message::OutputReplaceHint),
                    },
                )
            }
        })
        .ok_or("Pin window unavailable")?
}
fn write(
    ui: &DesktopPin,
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    options: Output,
    path: Option<PathBuf>,
    replacement: Option<glint_platform::ImageReplacement>,
    slot: OutputSlot,
) {
    let weak = ui.as_weak();
    let save = path.is_some();
    let result = std::thread::Builder::new()
        .name("glint-pin-output".into())
        .spawn(move || {
            let result = if let Some(path) = path {
                output::save_manual(
                    &path,
                    pixels.as_bytes(),
                    pixels.width(),
                    pixels.height(),
                    &options,
                    replacement.as_ref(),
                )
                .map(Some)
            } else {
                glint_platform::copy_rgba(
                    pixels.width(),
                    pixels.height(),
                    pixels.as_bytes().to_vec(),
                )
                .map(|()| None)
            };
            drop(slot);
            let _ =
                weak.upgrade_in_event_loop(move |ui| finish(&ui, result, save, options.feedback));
        });
    if let Err(error) = result {
        failed(ui, error.to_string());
    }
}
