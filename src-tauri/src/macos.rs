//! macOS-spezifisches: Schluesselbund, Sperrbildschirm, Fensterverhalten.

const SERVICE: &str = "io.github.flashkinx.roadrunner";
const ACCOUNT: &str = "google-api-key";

/// API-Key: Umgebungsvariable ROADRUNNER_API_KEY (fuer Tests) vor Schluesselbund.
pub fn api_key() -> Option<String> {
    if let Ok(k) = std::env::var("ROADRUNNER_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k.trim().to_string());
        }
    }
    security_framework::passwords::get_generic_password(SERVICE, ACCOUNT)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|k| !k.is_empty())
}

pub fn set_api_key(key: &str) -> anyhow::Result<()> {
    let key = key.trim();
    if key.is_empty() {
        let _ = security_framework::passwords::delete_generic_password(SERVICE, ACCOUNT);
        return Ok(());
    }
    security_framework::passwords::set_generic_password(SERVICE, ACCOUNT, key.as_bytes())?;
    Ok(())
}

mod cg {
    use core_foundation::dictionary::CFDictionaryRef;
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
    }
}

/// Bildschirm gesperrt oder Sitzung nicht im Vordergrund (Benutzerwechsel)?
pub fn screen_locked() -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    let raw = unsafe { cg::CGSessionCopyCurrentDictionary() };
    if raw.is_null() {
        // Kein Sitzungsdictionary: z.B. waehrend des Ruhezustands
        return true;
    }
    let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_create_rule(raw) };
    let flag = |name: &str| -> Option<bool> {
        dict.find(CFString::new(name))
            .and_then(|v| v.downcast::<CFBoolean>())
            .map(bool::from)
    };
    let locked = flag("CGSSessionScreenIsLocked").unwrap_or(false);
    let on_console = flag("kCGSSessionOnConsoleKey").unwrap_or(true);
    locked || !on_console
}

/// Fensterebene setzen: "immer im Vordergrund" auf Statusebene (25) statt nur
/// "floating" (3), damit die Pille auch ueber Vollbild-Apps wie einer
/// Parallels-VM bleibt. Muss nach `set_always_on_top` laufen, das setzt sonst
/// wieder auf floating zurueck.
pub fn set_level(window: &tauri::WebviewWindow, on_top: bool) {
    use objc2_app_kit::{NSNormalWindowLevel, NSStatusWindowLevel, NSWindow};
    let Ok(ptr) = window.ns_window() else { return };
    let ptr = ptr as usize;
    let _ = window.run_on_main_thread(move || {
        let ns: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
        let level = if on_top { NSStatusWindowLevel } else { NSNormalWindowLevel };
        ns.setLevel(level);
        if on_top && ns.isVisible() {
            ns.orderFrontRegardless();
        }
    });
}

/// Mini-Fenster auf allen Spaces und auch ueber Vollbild-Apps zeigen.
///
/// Ueber den Vollbild-Space einer anderen App (z.B. Parallels im
/// macOS-Vollbild) legt macOS nur Panels, keine normalen Fenster. Deshalb wird
/// das von Tauri erzeugte Fenster zur Laufzeit in ein nicht aktivierendes
/// NSPanel umgewandelt (gleicher Weg wie das Plugin tauri-nspanel). Nicht
/// aktivierend heisst: ein Klick auf die Pille holt die App nicht nach vorne
/// und nimmt der VM nicht den Fokus.
pub fn make_overlay(window: &tauri::WebviewWindow) {
    use objc2::runtime::AnyObject;
    use objc2::ClassType;
    use objc2_app_kit::{NSPanel, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask};
    let Ok(ptr) = window.ns_window() else { return };
    let w = window.clone();
    let ptr = ptr as usize;
    let _ = w.run_on_main_thread(move || {
        unsafe {
            objc2::ffi::object_setClass(ptr as *mut AnyObject, NSPanel::class());
        }
        let panel: &NSPanel = unsafe { &*(ptr as *const NSPanel) };
        let ns: &NSWindow = panel;
        ns.setStyleMask(ns.styleMask() | NSWindowStyleMask::NonactivatingPanel);
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        ns.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns.setHidesOnDeactivate(false);
        ns.setCanHide(false);
    });
}

/// Deckkraft des ganzen Fensters (Glas und Inhalt), 0.3 bis 1.0.
pub fn set_alpha(window: &tauri::WebviewWindow, alpha: f64) {
    use objc2_app_kit::NSWindow;
    let Ok(ptr) = window.ns_window() else { return };
    let ptr = ptr as usize;
    let _ = window.run_on_main_thread(move || {
        let ns: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
        ns.setAlphaValue(alpha.clamp(0.3, 1.0));
    });
}
