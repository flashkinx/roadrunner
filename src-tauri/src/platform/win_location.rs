//! Aktueller Standort ueber den Windows-Standortdienst (Geolocator, WinRT).
//!
//! Ohne Freigabe unter Einstellungen > Datenschutz > Standort liefert Windows
//! keinen Wert, dann rechnet Roadrunner mit den gespeicherten Adressen.

pub use super::Permission;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::AppHandle;
use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator, PositionAccuracy};

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Laenger warten wir nicht auf eine Position, sonst haengt der Abruf.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Standort einschalten und die Freigabe anfragen (Windows zeigt dabei ggf. den
/// Datenschutzdialog).
pub fn start(_app: &AppHandle) {
    ACTIVE.store(true, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(|| {
        let _ = Geolocator::RequestAccessAsync().and_then(|op| op.join());
    });
}

pub fn stop(_app: &AppHandle) {
    ACTIVE.store(false, Ordering::SeqCst);
}

fn read_position() -> windows::core::Result<(f64, f64)> {
    let g = Geolocator::new()?;
    g.SetDesiredAccuracy(PositionAccuracy::Default)?;
    let pos = g.GetGeopositionAsync()?.join()?;
    let p = pos.Coordinate()?.Point()?.Position()?;
    Ok((p.Latitude, p.Longitude))
}

/// (Breite, Laenge) oder None.
pub async fn current(_app: &AppHandle) -> Option<(f64, f64)> {
    if !ACTIVE.load(Ordering::SeqCst) {
        return None;
    }
    let task = tauri::async_runtime::spawn_blocking(read_position);
    match tokio::time::timeout(TIMEOUT, task).await {
        Ok(Ok(Ok(p))) => Some(p),
        _ => None,
    }
}

pub async fn permission(_app: &AppHandle) -> Permission {
    if !ACTIVE.load(Ordering::SeqCst) {
        return Permission::Off;
    }
    let status = tauri::async_runtime::spawn_blocking(|| Geolocator::RequestAccessAsync().and_then(|op| op.join()))
        .await
        .ok()
        .and_then(|r| r.ok());
    match status {
        Some(s) if s == GeolocationAccessStatus::Allowed => Permission::Granted,
        Some(s) if s == GeolocationAccessStatus::Denied => Permission::Denied,
        _ => Permission::NotDetermined,
    }
}
