//! Betriebssystem-spezifisches hinter einer gemeinsamen Schnittstelle.
//!
//! Jede Plattform liefert:
//! - `api_key()` / `set_api_key()`: sicherer Speicher (macOS-Schluesselbund,
//!   Windows-Anmeldeinformationsverwaltung)
//! - `screen_locked()`: Bildschirm gesperrt, dann keine Abfragen
//! - `make_overlay()`, `set_level()`, `set_alpha()`: Verhalten der Pille
//! - `open_url()`, `copy_text()`: Link im Browser, Text in die Zwischenablage
//! - `location`: aktueller Standort

use serde::Serialize;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
pub use mac::*;
#[cfg(target_os = "macos")]
#[path = "mac_location.rs"]
pub mod location;

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;
#[cfg(windows)]
#[path = "win_location.rs"]
pub mod location;

/// Stand der Standortfreigabe.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    NotDetermined,
    Denied,
    Granted,
    Off,
}

/// Dienst- bzw. Zielname im sicheren Speicher.
pub const SECRET_SERVICE: &str = "io.github.flashkinx.roadrunner";
pub const SECRET_ACCOUNT: &str = "google-api-key";

/// API-Key aus ROADRUNNER_API_KEY (fuer Tests), sonst None.
pub fn key_from_env() -> Option<String> {
    std::env::var("ROADRUNNER_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}
