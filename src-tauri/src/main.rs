//! Roadrunner: Fahrzeit mit Verkehr zwischen Zuhause und Firma als Ampel.

mod ampel;
mod config;
mod direction;
mod google;
mod platform;
mod probe;
mod scheduler;
mod state;
mod ui;

use config::{Config, Direction, Place};
use platform::location;
use google::Suggestion;
use state::{Shared, Status};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

type CmdResult<T> = Result<T, String>;

#[tauri::command]
fn get_config(app: AppHandle) -> Config {
    state::shared(&app).cfg()
}

#[tauri::command]
fn save_config(app: AppHandle, cfg: Config) -> CmdResult<Config> {
    let st = state::shared(&app);
    let (old, new) = {
        let mut c = st.cfg.lock().unwrap();
        let old = c.clone();
        *c = old.merged_from_ui(cfg);
        (old, c.clone())
    };
    new.save().map_err(|e| e.to_string())?;

    ui::apply_mini(&app);
    if old.autostart != new.autostart {
        let al = app.autolaunch();
        let r = if new.autostart { al.enable() } else { al.disable() };
        r.map_err(|e| format!("Autostart: {e}"))?;
    }
    if new.use_location && !old.use_location {
        location::start(&app);
    } else if !new.use_location && old.use_location {
        location::stop(&app);
    }
    if old.route_changed(&new) {
        st.kick();
    } else {
        state::recolor(&app);
        st.wake.notify_one();
    }
    let _ = app.emit("config", &new);
    Ok(new)
}

#[tauri::command]
fn get_status(app: AppHandle) -> Status {
    state::shared(&app).status()
}

#[tauri::command]
fn refresh_now(app: AppHandle) {
    state::shared(&app).kick();
}

#[tauri::command]
fn set_direction(app: AppHandle, mode: String) {
    let dir = match mode.as_str() {
        "toWork" => Some(Direction::ToWork),
        "toHome" => Some(Direction::ToHome),
        _ => None,
    };
    ui::set_manual(&app, dir);
}

#[tauri::command]
fn has_api_key() -> bool {
    platform::api_key().is_some()
}

#[tauri::command]
fn set_api_key(app: AppHandle, key: String) -> CmdResult<()> {
    platform::set_api_key(&key).map_err(|e| format!("Sicherer Speicher: {e}"))?;
    state::shared(&app).kick();
    Ok(())
}

#[tauri::command]
async fn autocomplete(app: AppHandle, input: String, session: String) -> CmdResult<Vec<Suggestion>> {
    let key = platform::api_key().ok_or("Zuerst den Google-API-Key hinterlegen")?;
    let st = state::shared(&app);
    let cfg = st.cfg();
    let bias = cfg.home.as_ref().map(|p| (p.lat, p.lng)).unwrap_or((48.2082, 16.3738));
    google::autocomplete(&st.http, &key, &input, &session, bias)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn place_details(app: AppHandle, place_id: String, session: String) -> CmdResult<Place> {
    let key = platform::api_key().ok_or("Zuerst den Google-API-Key hinterlegen")?;
    let st = state::shared(&app);
    google::place_details(&st.http, &key, &place_id, &session)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn parse_coordinates(text: String) -> Option<Place> {
    google::parse_coordinates(&text).map(|(lat, lng)| Place { label: format!("{lat:.5}, {lng:.5}"), lat, lng })
}

#[tauri::command]
async fn location_permission(app: AppHandle) -> location::Permission {
    location::permission(&app).await
}

#[tauri::command]
async fn current_location(app: AppHandle) -> Option<Place> {
    location::start(&app);
    location::current(&app)
        .await
        .map(|(lat, lng)| Place { label: format!("{lat:.5}, {lng:.5}"), lat, lng })
}

/// Link aus der Anleitung im Standardbrowser oeffnen. Nur Google-Seiten,
/// damit die Oberflaeche nichts Beliebiges starten kann.
#[tauri::command]
fn open_url(url: String) -> CmdResult<()> {
    const ALLOWED: [&str; 2] = ["https://console.cloud.google.com/", "https://developers.google.com/maps/"];
    if !ALLOWED.iter().any(|p| url.starts_with(p)) || url.contains(char::is_whitespace) {
        return Err("Adresse nicht erlaubt".into());
    }
    platform::open_url(&url)
}

/// Text in die Zwischenablage (pbcopy), fuer den Claude-Auftrag in der Anleitung.
#[tauri::command]
fn copy_text(text: String) -> CmdResult<()> {
    platform::copy_text(&text)
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    ui::open_settings(&app);
}

/// Seite, die die Einstellungen beim Oeffnen zeigen (ROADRUNNER_PAGE:
/// status, route, ampel, refresh, general, guide). Fuer Screenshots.
#[tauri::command]
fn start_page() -> Option<String> {
    std::env::var("ROADRUNNER_PAGE").ok().filter(|p| !p.is_empty())
}

#[tauri::command]
fn context_menu(app: AppHandle, window: tauri::Window) -> CmdResult<()> {
    let menu = ui::build_menu(&app).map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let handle = app.handle().clone();
    let st = state::shared(&handle);
    let cfg = st.cfg();

    ui::build_tray(&handle)?;

    if let Some(mini) = app.get_webview_window("mini") {
        // Erst die richtige Groesse, dann die gespeicherte Position: sonst
        // verschiebt die Rechtsbuendigkeit in apply_mini die Pille bei jedem Start
        ui::size_mini(&mini, &cfg);
        if let Some((x, y)) = cfg.mini_pos {
            let _ = mini.set_position(PhysicalPosition::new(x, y));
        } else if let Ok(Some(m)) = mini.primary_monitor() {
            // Erststart: oben rechts unter der Menueleiste
            let size = m.size();
            let scale = m.scale_factor();
            let x = size.width as f64 - (ui::mini_width(&cfg) * cfg.pill_scale + 24.0) * scale;
            let y = 44.0 * scale;
            let _ = mini.set_position(PhysicalPosition::new(x as i32, y as i32));
        }
        platform::make_overlay(&mini);
    }
    ui::apply_mini(&handle);

    if cfg.use_location {
        location::start(&handle);
    }
    if cfg.home.is_none() || cfg.work.is_none() || platform::api_key().is_none() {
        ui::open_settings(&handle);
    }

    // Beim Start sofort einen Wert holen, auch ausserhalb des Zeitfensters
    st.kick();
    // Screenshot-Hilfen: Einstellungen sofort zeigen bzw. das Menue der Pille oeffnen
    if start_page().is_some() {
        ui::open_settings(&handle);
    }
    if std::env::var("ROADRUNNER_MENU").is_ok_and(|v| v == "1") {
        let h = handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            if let (Some(w), Ok(menu)) = (h.get_webview_window("mini"), ui::build_menu(&h)) {
                let _ = w.popup_menu(&menu);
            }
        });
    }
    tauri::async_runtime::spawn(scheduler::run(handle));
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--probe") {
        std::process::exit(probe::run(&args));
    }

    let shared: Arc<Shared> = Shared::new();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_status,
            refresh_now,
            set_direction,
            has_api_key,
            set_api_key,
            autocomplete,
            place_details,
            parse_coordinates,
            location_permission,
            current_location,
            open_settings,
            start_page,
            open_url,
            copy_text,
            context_menu,
        ])
        .on_menu_event(ui::on_menu)
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } if window.label() == "settings" => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Moved(p) if window.label() == "mini" => {
                let st = state::shared(window.app_handle());
                st.cfg.lock().unwrap().mini_pos = Some((p.x, p.y));
                st.dirty.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            _ => {}
        })
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("Roadrunner konnte nicht starten");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = event {
            // Fenster schliessen beendet die App nicht, nur "Beenden" im Menue
            if code.is_none() {
                api.prevent_exit();
            } else {
                state::shared(app).save_now();
            }
        }
    });
}
