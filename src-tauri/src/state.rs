//! Gemeinsamer Zustand, Abruf einer Route und Verteilung an Fenster und Menueleiste.

use crate::ampel::{self, Color};
use crate::config::{Basis, Config, Direction};
use crate::direction::{self, Leg};
use crate::google::{self, RouteResult};
use chrono::{Local, Timelike};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Noch nie abgefragt
    Loading,
    /// Aktueller Wert
    Ok,
    /// Letzter Abruf gescheitert, angezeigt wird der alte Wert
    Stale,
    /// Ausserhalb des Zeitfensters oder Bildschirm gesperrt
    Paused,
    /// Adressen oder API-Key fehlen
    Setup,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub kind: Kind,
    pub color: Color,
    pub minutes: Option<i64>,
    pub delay_min: Option<i64>,
    pub distance_km: Option<f64>,
    pub direction: Option<Direction>,
    pub manual: bool,
    pub from_location: bool,
    pub origin: String,
    pub dest: String,
    /// "HH:MM" des letzten erfolgreichen Abrufs
    pub updated_at: Option<String>,
    pub message: Option<String>,
    pub usage_count: u32,
    pub free_quota: u32,
    pub basis: Basis,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            kind: Kind::Loading,
            color: Color::Grey,
            minutes: None,
            delay_min: None,
            distance_km: None,
            direction: None,
            manual: false,
            from_location: false,
            origin: String::new(),
            dest: String::new(),
            updated_at: None,
            message: None,
            usage_count: 0,
            free_quota: 5000,
            basis: Basis::Total,
        }
    }
}

pub struct Shared {
    pub cfg: Mutex<Config>,
    pub status: Mutex<Status>,
    pub last: Mutex<Option<(Leg, RouteResult, String)>>,
    pub wake: Notify,
    /// Beim naechsten Durchlauf sofort abrufen, auch ausserhalb des Zeitfensters.
    pub force: AtomicBool,
    /// Konfiguration hat ungespeicherte Aenderungen (Fensterposition, Zaehler).
    pub dirty: AtomicBool,
    pub http: reqwest::Client,
}

impl Shared {
    pub fn new() -> Arc<Self> {
        let cfg = Config::load();
        let status = Status { usage_count: cfg.usage.count, free_quota: cfg.free_quota, basis: cfg.basis, ..Default::default() };
        Arc::new(Self {
            cfg: Mutex::new(cfg),
            status: Mutex::new(status),
            last: Mutex::new(None),
            wake: Notify::new(),
            force: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            http: google::client(),
        })
    }

    pub fn cfg(&self) -> Config {
        self.cfg.lock().unwrap().clone()
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn kick(&self) {
        self.force.store(true, Ordering::SeqCst);
        self.wake.notify_one();
    }

    pub fn save_now(&self) {
        let cfg = self.cfg();
        if let Err(e) = cfg.save() {
            eprintln!("Konfiguration nicht gespeichert: {e}");
        }
        self.dirty.store(false, Ordering::SeqCst);
    }
}

pub fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

fn build_status(cfg: &Config, leg: &Leg, r: &RouteResult, updated_at: &str) -> Status {
    let minutes = ampel::to_minutes(r.duration_s);
    let delay = ampel::to_minutes(r.duration_s.saturating_sub(r.static_s));
    let value = match cfg.basis {
        Basis::Total => minutes,
        Basis::Delay => delay,
    };
    Status {
        kind: Kind::Ok,
        color: ampel::evaluate(value, &cfg.thresholds()),
        minutes: Some(minutes),
        delay_min: Some(delay),
        distance_km: Some((r.distance_m as f64 / 100.0).round() / 10.0),
        direction: Some(leg.direction),
        manual: leg.manual,
        from_location: leg.from_location,
        origin: leg.origin.label.clone(),
        dest: leg.dest.label.clone(),
        updated_at: Some(updated_at.to_string()),
        message: None,
        usage_count: cfg.usage.count,
        free_quota: cfg.free_quota,
        basis: cfg.basis,
    }
}

/// Status setzen und an beide Fenster und die Menueleiste verteilen.
pub fn publish(app: &AppHandle, status: Status) {
    let st = shared(app);
    {
        let mut cur = st.status.lock().unwrap();
        if *cur == status {
            return;
        }
        *cur = status.clone();
    }
    let _ = app.emit("status", &status);
    crate::ui::update_tray(app, &status);
}

/// Alten Wert behalten, aber als nicht aktuell (grau) kennzeichnen.
pub fn mark(app: &AppHandle, kind: Kind, message: impl Into<String>) {
    let st = shared(app);
    let cfg = st.cfg();
    let mut s = st.status();
    s.kind = kind;
    s.color = Color::Grey;
    s.message = Some(message.into());
    s.usage_count = cfg.usage.count;
    s.free_quota = cfg.free_quota;
    s.basis = cfg.basis;
    publish(app, s);
}

/// Nach Aenderung der Schwellen: Farbe aus dem letzten Ergebnis neu berechnen, ohne Abruf.
pub fn recolor(app: &AppHandle) {
    let st = shared(app);
    let cfg = st.cfg();
    let last = st.last.lock().unwrap().clone();
    let cur = st.status();
    match last {
        Some((leg, r, at)) if cur.kind == Kind::Ok => publish(app, build_status(&cfg, &leg, &r, &at)),
        _ => {
            let mut s = cur;
            s.basis = cfg.basis;
            s.free_quota = cfg.free_quota;
            publish(app, s);
        }
    }
}

/// Eine Abfrage. Ok(true) = Wert geholt, Ok(false) = nicht eingerichtet, Err = Fehler.
pub async fn fetch(app: &AppHandle) -> Result<bool, String> {
    let st = shared(app);
    let cfg = st.cfg();

    let Some(key) = crate::macos::api_key() else {
        mark(app, Kind::Setup, "Google-API-Key fehlt");
        return Ok(false);
    };

    let here = if cfg.use_location { crate::location::current(app).await } else { None };
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let Some(leg) = direction::decide(&cfg, now.hour() * 60 + now.minute(), &today, here) else {
        mark(app, Kind::Setup, "Adressen fehlen");
        return Ok(false);
    };

    {
        let mut c = st.cfg.lock().unwrap();
        c.count_request(&now.format("%Y-%m").to_string());
    }
    st.dirty.store(true, Ordering::SeqCst);

    match google::compute_route(&st.http, &key, &leg.origin, &leg.dest).await {
        Ok(r) => {
            let at = Local::now().format("%H:%M").to_string();
            let cfg = st.cfg();
            *st.last.lock().unwrap() = Some((leg.clone(), r, at.clone()));
            publish(app, build_status(&cfg, &leg, &r, &at));
            Ok(true)
        }
        Err(e) => {
            let msg = e.to_string();
            let s = st.status();
            let shown = match &s.updated_at {
                Some(t) => format!("{msg} · Stand {t}"),
                None => msg.clone(),
            };
            mark(app, Kind::Stale, shown);
            Err(msg)
        }
    }
}
