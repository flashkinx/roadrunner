//! Windows: Anmeldeinformationsverwaltung, Sperrbildschirm, Fensterverhalten,
//! Browser, Zwischenablage.

use super::{key_from_env, SECRET_ACCOUNT, SECRET_SERVICE};
use windows::core::{HSTRING, PWSTR};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

/// Zielname in der Anmeldeinformationsverwaltung (Windows-Anmeldeinformationen,
/// generisch): "io.github.flashkinx.roadrunner/google-api-key".
fn target() -> HSTRING {
    HSTRING::from(format!("{SECRET_SERVICE}/{SECRET_ACCOUNT}"))
}

/// API-Key: Umgebungsvariable ROADRUNNER_API_KEY (fuer Tests) vor dem Speicher.
pub fn api_key() -> Option<String> {
    if let Some(k) = key_from_env() {
        return Some(k);
    }
    unsafe {
        let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
        CredReadW(&target(), CRED_TYPE_GENERIC, None, &mut cred).ok()?;
        let c = &*cred;
        let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
        CredFree(cred as *const _);
        String::from_utf8(bytes).ok().filter(|k| !k.is_empty())
    }
}

pub fn set_api_key(key: &str) -> anyhow::Result<()> {
    let key = key.trim();
    let t = target();
    unsafe {
        if key.is_empty() {
            let _ = CredDeleteW(&t, CRED_TYPE_GENERIC, None);
            return Ok(());
        }
        let mut name: Vec<u16> = format!("{SECRET_SERVICE}/{SECRET_ACCOUNT}").encode_utf16().chain(std::iter::once(0)).collect();
        let mut user: Vec<u16> = SECRET_ACCOUNT.encode_utf16().chain(std::iter::once(0)).collect();
        let mut blob = key.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(name.as_mut_ptr()),
            UserName: PWSTR(user.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        CredWriteW(&cred, 0)?;
    }
    Ok(())
}

/// Bildschirm gesperrt? Dann liegt der sichere Desktop vorn, und ein normaler
/// Prozess kann den Eingabe-Desktop nicht oeffnen.
pub fn screen_locked() -> bool {
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP,
    };
    unsafe {
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
            Ok(desk) => {
                let _ = CloseDesktop(desk);
                false
            }
            Err(_) => true,
        }
    }
}

/// Pille: nicht in Alt+Tab, und ein Klick holt die App nicht nach vorne
/// (WS_EX_NOACTIVATE), damit z.B. ein Vollbildspiel oder eine RDP-Sitzung den
/// Fokus behaelt.
pub fn make_overlay(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let Ok(hwnd) = window.hwnd() else { return };
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize);
    }
}

/// Unter Windows reicht "topmost", das setzt `set_always_on_top` bereits.
pub fn set_level(_window: &tauri::WebviewWindow, _on_top: bool) {}

/// Deckkraft macht unter Windows das CSS der Pille (die Pille malt ihren
/// Hintergrund selbst), hier ist nichts zu tun.
pub fn set_alpha(_window: &tauri::WebviewWindow, _alpha: f64) {}

/// Link im Standardbrowser oeffnen.
pub fn open_url(url: &str) -> Result<(), String> {
    std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Browser: {e}"))
}

/// Text in die Zwischenablage.
pub fn copy_text(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_string()))
        .map_err(|e| format!("Zwischenablage: {e}"))
}
