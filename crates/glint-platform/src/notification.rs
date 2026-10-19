//! Worker-thread Windows submission. Ok never proves visual delivery.
pub fn submit_completion(body: &str) -> Result<(), String> {
    if body.is_empty() || body.contains('\0') || body.encode_utf16().count() > 256 {
        return Err("Invalid completion notification".into());
    }
    use windows::{
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{NotificationSetting, ToastNotification, ToastNotificationManager},
        Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
        core::HSTRING,
    };
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|e| e.to_string())?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    let _apartment = Apartment;
    let notifier =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("Glint.Desktop"))
            .map_err(|e| e.to_string())?;
    if notifier.Setting().map_err(|e| e.to_string())? != NotificationSetting::Enabled {
        return Err("Windows notifications are disabled for Glint".into());
    }
    let document = XmlDocument::new().map_err(|e| e.to_string())?;
    let body = body
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    document.LoadXml(&HSTRING::from(format!("<toast duration=\"short\"><visual><binding template=\"ToastGeneric\"><text>Glint</text><text>{body}</text></binding></visual><audio silent=\"true\"/></toast>"))).map_err(|e|e.to_string())?;
    let toast = ToastNotification::CreateToastNotification(&document).map_err(|e| e.to_string())?;
    toast
        .SetTag(&HSTRING::from("completed"))
        .map_err(|e| e.to_string())?;
    toast
        .SetGroup(&HSTRING::from("glint"))
        .map_err(|e| e.to_string())?;
    notifier.Show(&toast).map_err(|e| e.to_string())
}
