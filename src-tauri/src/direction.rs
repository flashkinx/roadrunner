//! Welche Strecke wird abgefragt: Richtung, Start und Ziel.

use crate::config::{Config, Direction, Place};

#[derive(Debug, Clone, PartialEq)]
pub struct Leg {
    pub direction: Direction,
    pub origin: Place,
    pub dest: Place,
    /// Richtung wurde von Hand gewaehlt.
    pub manual: bool,
    /// Start ist der aktuelle Standort, nicht eine der gespeicherten Adressen.
    pub from_location: bool,
}

/// Entfernung in Metern (Haversine).
pub fn distance_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let r = 6_371_000.0_f64;
    let (la1, lo1) = (a.0.to_radians(), a.1.to_radians());
    let (la2, lo2) = (b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2)
        + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

/// Strecke bestimmen.
///
/// * `minutes_now`: Minuten seit Mitternacht (lokal)
/// * `today`: "YYYY-MM-DD", fuer die Gueltigkeit der Handwahl
/// * `here`: aktueller Standort, falls verwendet und bekannt
pub fn decide(cfg: &Config, minutes_now: u32, today: &str, here: Option<(f64, f64)>) -> Option<Leg> {
    let home = cfg.home.clone()?;
    let work = cfg.work.clone()?;
    let home = Place { label: cfg.home_name.clone(), ..home };
    let work = Place { label: cfg.work_name.clone(), ..work };

    let by_time = if minutes_now < cfg.switch_minutes() { Direction::ToWork } else { Direction::ToHome };
    let manual = cfg.manual.as_ref().filter(|m| m.date == today).map(|m| m.direction);

    let radius = cfg.radius_m as f64;
    let near = |p: &Place, h: (f64, f64)| distance_m((p.lat, p.lng), h) <= radius;

    let (auto_dir, here_place) = match here {
        Some(h) if near(&home, h) => (Direction::ToWork, None),
        Some(h) if near(&work, h) => (Direction::ToHome, None),
        Some(h) => (
            by_time,
            Some(Place { label: "Standort".into(), lat: h.0, lng: h.1 }),
        ),
        None => (by_time, None),
    };
    let direction = manual.unwrap_or(auto_dir);
    let (dest, other) = match direction {
        Direction::ToWork => (work, home),
        Direction::ToHome => (home, work),
    };

    // Start: aktueller Ort, solange er nicht schon am Ziel liegt; sonst die andere Adresse.
    let at_dest = here.is_some_and(|h| near(&dest, h));
    let (origin, from_location) = match here_place {
        Some(p) if !at_dest => (p, true),
        _ => (other, false),
    };

    Some(Leg { direction, origin, dest, manual: manual.is_some(), from_location })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ManualOverride;

    fn cfg() -> Config {
        Config {
            home: Some(Place { label: "x".into(), lat: 48.2085, lng: 16.3731 }), // Wien, Stephansplatz
            work: Some(Place { label: "y".into(), lat: 48.2047, lng: 15.6256 }), // St. Poelten
            ..Config::default()
        }
    }

    #[test]
    fn needs_both_addresses() {
        let mut c = cfg();
        c.work = None;
        assert!(decide(&c, 420, "2026-11-19", None).is_none());
    }

    #[test]
    fn by_time_without_location() {
        let c = cfg();
        let m = decide(&c, 7 * 60, "2026-11-19", None).unwrap();
        assert_eq!(m.direction, Direction::ToWork);
        assert_eq!(m.origin.label, "Zuhause");
        let a = decide(&c, 16 * 60, "2026-11-19", None).unwrap();
        assert_eq!(a.direction, Direction::ToHome);
        assert_eq!(a.dest.label, "Zuhause");
    }

    #[test]
    fn location_overrides_time() {
        let c = cfg();
        // nachmittags, aber zu Hause: zur Firma
        let l = decide(&c, 16 * 60, "2026-11-19", Some((48.2086, 16.3732))).unwrap();
        assert_eq!(l.direction, Direction::ToWork);
        assert!(!l.from_location);
        // unterwegs: vom Standort, Richtung nach Uhrzeit
        let u = decide(&c, 16 * 60, "2026-11-19", Some((48.20, 16.00))).unwrap();
        assert_eq!(u.direction, Direction::ToHome);
        assert!(u.from_location);
        assert_eq!(u.origin.label, "Standort");
    }

    #[test]
    fn manual_only_today() {
        let mut c = cfg();
        c.manual = Some(ManualOverride { direction: Direction::ToHome, date: "2026-11-19".into() });
        assert_eq!(decide(&c, 7 * 60, "2026-11-19", None).unwrap().direction, Direction::ToHome);
        assert_eq!(decide(&c, 7 * 60, "2026-11-20", None).unwrap().direction, Direction::ToWork);
    }

    #[test]
    fn manual_while_at_destination_uses_other_address() {
        let mut c = cfg();
        c.manual = Some(ManualOverride { direction: Direction::ToHome, date: "2026-11-19".into() });
        let l = decide(&c, 7 * 60, "2026-11-19", Some((48.2086, 16.3732))).unwrap();
        assert_eq!(l.origin.label, "Firma");
        assert!(!l.from_location);
    }

    #[test]
    fn distance_sane() {
        let d = distance_m((48.2085, 16.3731), (48.2047, 15.6256));
        assert!((54_000.0..57_000.0).contains(&d), "{d}");
    }
}
