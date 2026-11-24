//! Einstellungen der Roadrunner, als JSON unter
//! ~/Library/Application Support/io.github.flashkinx.roadrunner/config.json.

use chrono::{Datelike, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub label: String,
    pub lat: f64,
    pub lng: f64,
}

/// Worauf sich die Schwellen beziehen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Basis {
    /// Gesamtfahrzeit mit Verkehr.
    Total,
    /// Verzoegerung gegenueber der Fahrzeit ohne Verkehr.
    Delay,
}

/// Hell, dunkel oder wie das System.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    ToWork,
    ToHome,
}

/// Bis `green_max` gruen, bis `yellow_max` gelb, darueber rot (Minuten, inklusive).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Thresholds {
    pub green_max: i64,
    pub yellow_max: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DayWindow {
    pub enabled: bool,
    /// "HH:MM"
    pub start: String,
    /// "HH:MM"
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManualOverride {
    pub direction: Direction,
    /// Gilt nur an diesem Tag (YYYY-MM-DD), ab Mitternacht wieder automatisch.
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// "YYYY-MM"
    pub month: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub home: Option<Place>,
    pub work: Option<Place>,
    pub home_name: String,
    pub work_name: String,
    pub use_location: bool,

    pub basis: Basis,
    pub total: Thresholds,
    pub delay: Thresholds,

    /// Vor dieser Uhrzeit Richtung Firma, danach Richtung Zuhause ("HH:MM").
    pub switch_time: String,
    /// Umkreis, in dem der Standort als "zu Hause" bzw. "in der Firma" gilt.
    pub radius_m: u32,

    pub interval_min: u32,
    pub always_active: bool,
    /// Montag bis Sonntag, genau sieben Eintraege.
    pub schedule: Vec<DayWindow>,

    pub always_on_top: bool,
    pub show_mini: bool,
    /// Groesse der Mini-Ampel, 1.0 = 100 x 30 pt, Bereich 0.8 bis 2.0.
    pub pill_scale: f64,
    /// Deckkraft der Mini-Ampel, 0.3 bis 1.0.
    pub pill_opacity: f64,
    /// In der Pille neben der Ampel die Fahrzeit (mit Verzoegerung) zeigen.
    pub show_duration: bool,
    /// In der Pille die Ankunftszeit bei Abfahrt jetzt zeigen.
    pub show_arrival: bool,
    pub appearance: Appearance,
    pub autostart: bool,
    /// Freikontingent der Routes-API je Monat, nur fuer die Warnung in der UI.
    pub free_quota: u32,

    // Laufzeitzustand, wird von der Einstellungsansicht nicht ueberschrieben.
    pub mini_pos: Option<(i32, i32)>,
    pub manual: Option<ManualOverride>,
    pub usage: Usage,
}

impl Default for Config {
    fn default() -> Self {
        let weekday = DayWindow { enabled: true, start: "06:00".into(), end: "20:00".into() };
        let weekend = DayWindow { enabled: false, start: "08:00".into(), end: "20:00".into() };
        Self {
            home: None,
            work: None,
            home_name: "Zuhause".into(),
            work_name: "Firma".into(),
            use_location: false,
            basis: Basis::Total,
            total: Thresholds { green_max: 45, yellow_max: 55 },
            delay: Thresholds { green_max: 5, yellow_max: 15 },
            switch_time: "12:00".into(),
            radius_m: 500,
            interval_min: 5,
            always_active: false,
            schedule: vec![
                weekday.clone(),
                weekday.clone(),
                weekday.clone(),
                weekday.clone(),
                weekday,
                weekend.clone(),
                weekend,
            ],
            always_on_top: true,
            show_mini: true,
            pill_scale: 1.0,
            pill_opacity: 1.0,
            show_duration: true,
            show_arrival: false,
            appearance: Appearance::System,
            autostart: false,
            free_quota: 5000,
            mini_pos: None,
            manual: None,
            usage: Usage::default(),
        }
    }
}

/// "HH:MM" -> Minuten seit Mitternacht.
pub fn parse_hm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl Config {
    /// Standard: ~/Library/Application Support/io.github.flashkinx.roadrunner/.
    /// ROADRUNNER_CONFIG_DIR verlegt die Konfiguration, z.B. fuer eine
    /// Demo-Instanz mit neutralen Adressen (Screenshots).
    pub fn path() -> PathBuf {
        if let Some(dir) = std::env::var_os("ROADRUNNER_CONFIG_DIR").filter(|d| !d.is_empty()) {
            return PathBuf::from(dir).join("config.json");
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("io.github.flashkinx.roadrunner")
            .join("config.json")
    }

    pub fn load() -> Self {
        let mut cfg: Config = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        cfg.normalize();
        cfg
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    /// Unsinnige Werte geradeziehen, statt die App daran scheitern zu lassen.
    pub fn normalize(&mut self) {
        let d = Config::default();
        self.interval_min = self.interval_min.clamp(1, 120);
        self.radius_m = self.radius_m.clamp(50, 20_000);
        self.pill_scale = if self.pill_scale.is_finite() { (self.pill_scale * 10.0).round() / 10.0 } else { 1.0 };
        self.pill_scale = self.pill_scale.clamp(0.8, 2.0);
        self.pill_opacity = if self.pill_opacity.is_finite() { (self.pill_opacity * 20.0).round() / 20.0 } else { 1.0 };
        self.pill_opacity = self.pill_opacity.clamp(0.3, 1.0);
        if self.schedule.len() != 7 {
            self.schedule = d.schedule.clone();
        }
        for w in &mut self.schedule {
            if parse_hm(&w.start).is_none() {
                w.start = "06:00".into();
            }
            if parse_hm(&w.end).is_none() {
                w.end = "20:00".into();
            }
        }
        if parse_hm(&self.switch_time).is_none() {
            self.switch_time = d.switch_time;
        }
        for t in [&mut self.total, &mut self.delay] {
            if t.yellow_max < t.green_max {
                std::mem::swap(&mut t.green_max, &mut t.yellow_max);
            }
        }
        if self.home_name.trim().is_empty() {
            self.home_name = d.home_name;
        }
        if self.work_name.trim().is_empty() {
            self.work_name = d.work_name;
        }
    }

    pub fn thresholds(&self) -> Thresholds {
        match self.basis {
            Basis::Total => self.total,
            Basis::Delay => self.delay,
        }
    }

    pub fn switch_minutes(&self) -> u32 {
        parse_hm(&self.switch_time).unwrap_or(12 * 60)
    }

    /// Liegt `now` im Aktivzeitfenster des jeweiligen Wochentags?
    pub fn in_window(&self, now: NaiveDateTime) -> bool {
        if self.always_active {
            return true;
        }
        let day = now.weekday().num_days_from_monday() as usize;
        let Some(w) = self.schedule.get(day) else { return false };
        if !w.enabled {
            return false;
        }
        let (Some(start), Some(end)) = (parse_hm(&w.start), parse_hm(&w.end)) else {
            return false;
        };
        let t = now.hour() * 60 + now.minute();
        if start <= end {
            start <= t && t < end
        } else {
            // Fenster ueber Mitternacht, z.B. 22:00 bis 02:00
            t >= start || t < end
        }
    }

    /// Uebernimmt die Felder aus der Einstellungsansicht, behaelt den Laufzeitzustand.
    pub fn merged_from_ui(&self, ui: Config) -> Config {
        let mut c = ui;
        c.mini_pos = self.mini_pos;
        c.manual = self.manual.clone();
        c.usage = self.usage.clone();
        c.normalize();
        c
    }

    /// Haben sich Felder geaendert, die eine neue Routenabfrage brauchen?
    pub fn route_changed(&self, other: &Config) -> bool {
        self.home != other.home
            || self.work != other.work
            || self.use_location != other.use_location
            || self.switch_time != other.switch_time
            || self.radius_m != other.radius_m
    }

    /// Zaehler fuer den laufenden Monat erhoehen, Monatswechsel setzt zurueck.
    pub fn count_request(&mut self, month: &str) {
        if self.usage.month != month {
            self.usage = Usage { month: month.to_string(), count: 0 };
        }
        self.usage.count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
    }

    #[test]
    fn hm_parsing() {
        assert_eq!(parse_hm("06:30"), Some(390));
        assert_eq!(parse_hm("24:00"), None);
        assert_eq!(parse_hm("7"), None);
    }

    #[test]
    fn window_default() {
        let c = Config::default();
        // 19.11.2026 ist ein Donnerstag
        assert!(c.in_window(at(2026, 11, 19, 7, 0)));
        assert!(!c.in_window(at(2026, 11, 19, 5, 59)));
        assert!(!c.in_window(at(2026, 11, 19, 20, 0)));
        // Samstag standardmaessig aus
        assert!(!c.in_window(at(2026, 11, 21, 10, 0)));
    }

    #[test]
    fn window_over_midnight() {
        let mut c = Config::default();
        c.schedule[3] = DayWindow { enabled: true, start: "22:00".into(), end: "02:00".into() };
        assert!(c.in_window(at(2026, 11, 19, 23, 0)));
        assert!(c.in_window(at(2026, 11, 19, 1, 0)));
        assert!(!c.in_window(at(2026, 11, 19, 12, 0)));
    }

    #[test]
    fn usage_rolls_over() {
        let mut c = Config::default();
        c.count_request("2026-11");
        c.count_request("2026-11");
        assert_eq!(c.usage.count, 2);
        c.count_request("2026-12");
        assert_eq!(c.usage.count, 1);
    }

    #[test]
    fn swapped_thresholds_fixed() {
        let mut c = Config::default();
        c.total = Thresholds { green_max: 60, yellow_max: 40 };
        c.normalize();
        assert_eq!(c.total, Thresholds { green_max: 40, yellow_max: 60 });
    }
}
