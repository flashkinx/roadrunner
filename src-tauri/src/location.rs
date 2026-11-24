//! Aktueller Standort ueber CoreLocation (am Mac per WLAN-Ortung).
//!
//! Kein Delegate: der CLLocationManager laeuft auf dem Hauptthread, und wir lesen
//! bei Bedarf `location` (die zuletzt ermittelte Position) aus.

use objc2::rc::Retained;
use objc2_core_location::{kCLLocationAccuracyHundredMeters, CLAuthorizationStatus, CLLocationManager};
use serde::Serialize;
use std::cell::RefCell;
use tauri::AppHandle;

thread_local! {
    static MANAGER: RefCell<Option<Retained<CLLocationManager>>> = const { RefCell::new(None) };
}

/// Positionen aelter als das gelten als unbekannt.
const MAX_AGE_S: f64 = 30.0 * 60.0;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    NotDetermined,
    Denied,
    Granted,
    Off,
}

/// Manager anlegen und Berechtigung anfragen. Mehrfachaufruf ist harmlos.
pub fn start(app: &AppHandle) {
    let _ = app.run_on_main_thread(|| {
        MANAGER.with(|m| {
            let mut m = m.borrow_mut();
            if m.is_none() {
                let mgr = unsafe { CLLocationManager::new() };
                unsafe {
                    mgr.setDesiredAccuracy(kCLLocationAccuracyHundredMeters);
                    mgr.requestWhenInUseAuthorization();
                    mgr.startUpdatingLocation();
                }
                *m = Some(mgr);
            }
        });
    });
}

/// Standortabfrage beenden (Standort in den Einstellungen abgeschaltet).
pub fn stop(app: &AppHandle) {
    let _ = app.run_on_main_thread(|| {
        MANAGER.with(|m| {
            if let Some(mgr) = m.borrow_mut().take() {
                unsafe { mgr.stopUpdatingLocation() };
            }
        });
    });
}

async fn on_main<T: Send + 'static>(app: &AppHandle, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .ok()?;
    rx.await.ok()
}

/// (Breite, Laenge), falls eine hinreichend frische Position vorliegt.
pub async fn current(app: &AppHandle) -> Option<(f64, f64)> {
    on_main(app, || {
        MANAGER.with(|m| {
            let m = m.borrow();
            let mgr = m.as_ref()?;
            let loc = unsafe { mgr.location() }?;
            let age = -unsafe { loc.timestamp() }.timeIntervalSinceNow();
            if age > MAX_AGE_S {
                return None;
            }
            let c = unsafe { loc.coordinate() };
            Some((c.latitude, c.longitude))
        })
    })
    .await
    .flatten()
}

pub async fn permission(app: &AppHandle) -> Permission {
    on_main(app, || {
        MANAGER.with(|m| {
            let m = m.borrow();
            let Some(mgr) = m.as_ref() else { return Permission::Off };
            let s = unsafe { mgr.authorizationStatus() };
            if s == CLAuthorizationStatus::NotDetermined {
                Permission::NotDetermined
            } else if s == CLAuthorizationStatus::Denied || s == CLAuthorizationStatus::Restricted {
                Permission::Denied
            } else {
                Permission::Granted
            }
        })
    })
    .await
    .unwrap_or(Permission::Off)
}
