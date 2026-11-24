//! Google Routes API (Fahrzeit mit Verkehr) und Places API (New) fuer die Adresssuche.

use crate::config::Place;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

const ROUTES_URL: &str = "https://routes.googleapis.com/directions/v2:computeRoutes";
const AUTOCOMPLETE_URL: &str = "https://places.googleapis.com/v1/places:autocomplete";
const DETAILS_URL: &str = "https://places.googleapis.com/v1/places/";

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Roadrunner/0.1")
        .build()
        .expect("HTTP-Client")
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteResult {
    /// Fahrzeit mit aktuellem Verkehr
    pub duration_s: u64,
    /// Fahrzeit ohne Verkehr
    pub static_s: u64,
    pub distance_m: u64,
}

fn lat_lng(p: &Place) -> serde_json::Value {
    json!({ "location": { "latLng": { "latitude": p.lat, "longitude": p.lng } } })
}

/// "1234s" -> 1234
fn parse_seconds(s: &str) -> Option<u64> {
    let s = s.strip_suffix('s')?;
    s.parse::<f64>().ok().map(|v| v.round() as u64)
}

/// Fehlerantwort von Google in einen verstaendlichen Satz uebersetzen.
fn api_error(status: reqwest::StatusCode, body: &str) -> anyhow::Error {
    let v: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let msg = v["error"]["message"].as_str().unwrap_or("").to_string();
    let code = v["error"]["status"].as_str().unwrap_or("");
    let hint = match (status.as_u16(), code) {
        (400, "INVALID_ARGUMENT") if msg.contains("API key") => "API-Key ungültig",
        (400, _) => "Anfrage abgelehnt",
        (403, _) => "API-Key nicht für diese API freigeschaltet",
        (429, _) => "Kontingent erschöpft",
        (500..=599, _) => "Google-Dienst gestört",
        _ => "Fehler bei Google",
    };
    if msg.is_empty() {
        anyhow!("{hint} (HTTP {status})")
    } else {
        anyhow!("{hint}: {msg}")
    }
}

pub async fn compute_route(http: &reqwest::Client, key: &str, from: &Place, to: &Place) -> Result<RouteResult> {
    let body = json!({
        "origin": lat_lng(from),
        "destination": lat_lng(to),
        "travelMode": "DRIVE",
        "routingPreference": "TRAFFIC_AWARE",
        "languageCode": "de-AT",
        "units": "METRIC"
    });
    let resp = http
        .post(ROUTES_URL)
        .header("X-Goog-Api-Key", key)
        .header("X-Goog-FieldMask", "routes.duration,routes.staticDuration,routes.distanceMeters")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("Keine Verbindung zu Google ({e})"))?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    parse_route(&text)
}

fn parse_route(text: &str) -> Result<RouteResult> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    let r = v["routes"].get(0).ok_or_else(|| anyhow!("Keine Route gefunden"))?;
    let duration_s = r["duration"].as_str().and_then(parse_seconds).ok_or_else(|| anyhow!("Antwort ohne Fahrzeit"))?;
    let static_s = r["staticDuration"].as_str().and_then(parse_seconds).unwrap_or(duration_s);
    let distance_m = r["distanceMeters"].as_u64().unwrap_or(0);
    Ok(RouteResult { duration_s, static_s, distance_m })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub place_id: String,
    pub main: String,
    pub secondary: String,
}

#[derive(Deserialize)]
struct AcResponse {
    #[serde(default)]
    suggestions: Vec<AcSuggestion>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcSuggestion {
    place_prediction: Option<AcPrediction>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcPrediction {
    place_id: String,
    text: AcText,
    structured_format: Option<AcStructured>,
}
#[derive(Deserialize)]
struct AcText {
    text: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcStructured {
    main_text: AcText,
    secondary_text: Option<AcText>,
}

/// Adressvorschlaege beim Tippen. `bias`: Umkreis um diesen Punkt bevorzugen.
pub async fn autocomplete(
    http: &reqwest::Client,
    key: &str,
    input: &str,
    session: &str,
    bias: (f64, f64),
) -> Result<Vec<Suggestion>> {
    let body = json!({
        "input": input,
        "sessionToken": session,
        "languageCode": "de",
        "regionCode": "at",
        "locationBias": { "circle": { "center": { "latitude": bias.0, "longitude": bias.1 }, "radius": 50000.0 } }
    });
    let resp = http
        .post(AUTOCOMPLETE_URL)
        .header("X-Goog-Api-Key", key)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("Keine Verbindung zu Google ({e})"))?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    let r: AcResponse = serde_json::from_str(&text)?;
    Ok(r
        .suggestions
        .into_iter()
        .filter_map(|s| s.place_prediction)
        .map(|p| match p.structured_format {
            Some(sf) => Suggestion {
                place_id: p.place_id,
                main: sf.main_text.text,
                secondary: sf.secondary_text.map(|t| t.text).unwrap_or_default(),
            },
            None => Suggestion { place_id: p.place_id, main: p.text.text, secondary: String::new() },
        })
        .collect())
}

/// Koordinaten zu einem Vorschlag holen. Beendet die Autocomplete-Sitzung.
pub async fn place_details(http: &reqwest::Client, key: &str, place_id: &str, session: &str) -> Result<Place> {
    if !place_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(anyhow!("Ungültige Place-ID"));
    }
    let resp = http
        .get(format!("{DETAILS_URL}{place_id}"))
        .query(&[("sessionToken", session), ("languageCode", "de")])
        .header("X-Goog-Api-Key", key)
        .header("X-Goog-FieldMask", "formattedAddress,location")
        .send()
        .await
        .map_err(|e| anyhow!("Keine Verbindung zu Google ({e})"))?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let lat = v["location"]["latitude"].as_f64().ok_or_else(|| anyhow!("Ort ohne Koordinaten"))?;
    let lng = v["location"]["longitude"].as_f64().ok_or_else(|| anyhow!("Ort ohne Koordinaten"))?;
    let label = v["formattedAddress"].as_str().unwrap_or("").to_string();
    Ok(Place { label, lat, lng })
}

/// "48.2085, 16.3731" (auch mit Semikolon oder Leerzeichen) -> Koordinaten.
pub fn parse_coordinates(s: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = s
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 2 {
        return None;
    }
    let lat: f64 = parts[0].parse().ok()?;
    let lng: f64 = parts[1].parse().ok()?;
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng)).then_some((lat, lng))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_route_response() {
        let r = parse_route(r#"{"routes":[{"distanceMeters":31250,"duration":"2712s","staticDuration":"2290s"}]}"#).unwrap();
        assert_eq!(r, RouteResult { duration_s: 2712, static_s: 2290, distance_m: 31250 });
        assert!(parse_route(r#"{}"#).is_err());
    }

    #[test]
    fn coordinates() {
        assert_eq!(parse_coordinates("48.2085, 16.3731"), Some((48.2085, 16.3731)));
        assert_eq!(parse_coordinates("48.2085;16.3731"), Some((48.2085, 16.3731)));
        assert_eq!(parse_coordinates(" 48.2085 16.3731 "), Some((48.2085, 16.3731)));
        assert_eq!(parse_coordinates("Stephansplatz 1"), None);
        assert_eq!(parse_coordinates("91, 16"), None);
    }
}
