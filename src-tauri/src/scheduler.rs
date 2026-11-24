//! Zeitplan: alle N Minuten abrufen, nur im Zeitfenster, nie bei gesperrtem Bildschirm.

use crate::state::{self, Kind};
use chrono::{DateTime, Duration, Local};
use std::sync::atomic::Ordering;
use tauri::AppHandle;

/// Takt der Pruefschleife. Der eigentliche Abruf folgt dem eingestellten Intervall.
const TICK: std::time::Duration = std::time::Duration::from_secs(15);
/// Nach einem Fehler frueher nochmal versuchen (z.B. Netz nach dem Aufwachen).
const RETRY_MIN: i64 = 1;

pub async fn run(app: AppHandle) {
    let st = state::shared(&app);
    let mut next_due: Option<DateTime<Local>> = None;
    let mut was_locked = false;
    let mut was_active = false;
    let mut last_interval = 0;

    loop {
        let now = Local::now();
        let cfg = st.cfg();
        let locked = crate::platform::screen_locked();
        let active = cfg.in_window(now.naive_local());
        let force = st.force.swap(false, Ordering::SeqCst);

        // Intervall geaendert: naechsten Termin neu rechnen, nicht bis zum alten warten
        if last_interval != cfg.interval_min {
            if let Some(d) = next_due {
                let prev = Duration::minutes(last_interval as i64);
                next_due = Some(d - prev + Duration::minutes(cfg.interval_min as i64));
            }
            last_interval = cfg.interval_min;
        }

        if locked && !force {
            if !was_locked {
                state::mark(&app, Kind::Paused, "Pause: Bildschirm gesperrt");
            }
        } else if !active && !force {
            // Ein Wert bleibt farbig, bis sein Intervall abgelaufen ist (auch nach Handabruf)
            let kind = st.status().kind;
            if kind != Kind::Paused && kind != Kind::Setup && next_due.is_none_or(|d| now >= d) {
                state::mark(&app, Kind::Paused, "Pause: außerhalb des Zeitfensters");
            }
        } else {
            let resumed = (was_locked && !locked) || (active && !was_active);
            let due = next_due.is_none_or(|d| now >= d);
            if force || resumed || due {
                next_due = match state::fetch(&app).await {
                    Ok(_) => Some(Local::now() + Duration::minutes(cfg.interval_min as i64)),
                    Err(_) => Some(Local::now() + Duration::minutes(RETRY_MIN)),
                };
            }
        }

        if st.dirty.load(Ordering::SeqCst) {
            st.save_now();
        }

        was_locked = locked;
        was_active = active;

        tokio::select! {
            _ = st.wake.notified() => {}
            _ = tokio::time::sleep(TICK) => {}
        }
    }
}
