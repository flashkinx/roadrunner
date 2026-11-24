//! Testmodus ohne Oberflaeche:
//!
//!   roadrunner --probe                       beide Richtungen mit den gespeicherten Adressen
//!   roadrunner --probe --place "Adresse"     Adresssuche, erster Treffer mit Koordinaten
//!   roadrunner --probe --from LAT,LNG --to LAT,LNG
//!
//! API-Key aus dem Schluesselbund oder aus ROADRUNNER_API_KEY.
//! Zaehlt nicht in den Monatszaehler der App.

use crate::ampel;
use crate::config::{Config, Place};
use crate::google;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn coord(s: &str, label: &str) -> Option<Place> {
    google::parse_coordinates(s).map(|(lat, lng)| Place { label: label.into(), lat, lng })
}

pub fn run(args: &[String]) -> i32 {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("Runtime");
    rt.block_on(async {
        let Some(key) = crate::macos::api_key() else {
            eprintln!("Kein API-Key (Schlüsselbund oder ROADRUNNER_API_KEY).");
            return 2;
        };
        let http = google::client();
        let cfg = Config::load();

        if let Some(q) = arg(args, "--place") {
            let session = format!("probe-{}", std::process::id());
            let bias = cfg.home.as_ref().map(|p| (p.lat, p.lng)).unwrap_or((48.2082, 16.3738));
            match google::autocomplete(&http, &key, &q, &session, bias).await {
                Ok(list) if !list.is_empty() => {
                    for s in &list {
                        println!("  {} | {}", s.main, s.secondary);
                    }
                    match google::place_details(&http, &key, &list[0].place_id, &session).await {
                        Ok(p) => println!("Erster Treffer: {} -> {:.6}, {:.6}", p.label, p.lat, p.lng),
                        Err(e) => {
                            eprintln!("Details: {e}");
                            return 1;
                        }
                    }
                    0
                }
                Ok(_) => {
                    eprintln!("Keine Treffer");
                    1
                }
                Err(e) => {
                    eprintln!("Suche: {e}");
                    1
                }
            }
        } else {
            let pairs: Vec<(Place, Place)> = match (arg(args, "--from"), arg(args, "--to")) {
                (Some(f), Some(t)) => match (coord(&f, "Start"), coord(&t, "Ziel")) {
                    (Some(a), Some(b)) => vec![(a, b)],
                    _ => {
                        eprintln!("Koordinaten nicht lesbar");
                        return 2;
                    }
                },
                _ => match (cfg.home.clone(), cfg.work.clone()) {
                    (Some(h), Some(w)) => {
                        let h = Place { label: cfg.home_name.clone(), ..h };
                        let w = Place { label: cfg.work_name.clone(), ..w };
                        vec![(h.clone(), w.clone()), (w, h)]
                    }
                    _ => {
                        eprintln!("Keine Adressen gespeichert, --from/--to angeben");
                        return 2;
                    }
                },
            };
            let t = cfg.thresholds();
            for (a, b) in pairs {
                match google::compute_route(&http, &key, &a, &b).await {
                    Ok(r) => {
                        let m = ampel::to_minutes(r.duration_s);
                        let d = ampel::to_minutes(r.duration_s.saturating_sub(r.static_s));
                        let v = match cfg.basis {
                            crate::config::Basis::Total => m,
                            crate::config::Basis::Delay => d,
                        };
                        println!(
                            "{} -> {}: {} min mit Verkehr ({} s), {} min ohne ({} s), Verzögerung {} min, {:.1} km, Ampel {:?}",
                            a.label,
                            b.label,
                            m,
                            r.duration_s,
                            ampel::to_minutes(r.static_s),
                            r.static_s,
                            d,
                            r.distance_m as f64 / 1000.0,
                            ampel::evaluate(v, &t)
                        );
                    }
                    Err(e) => {
                        eprintln!("{} -> {}: {e}", a.label, b.label);
                        return 1;
                    }
                }
            }
            0
        }
    })
}
