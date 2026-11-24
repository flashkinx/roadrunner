//! Menueleisten-Symbol, Menues und Fenster.

use crate::ampel::Color;
use crate::config::{Appearance, Direction, ManualOverride};
use crate::state::{self, Kind, Status};
use chrono::Local;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

const TRAY_ID: &str = "ampel";
/// Hoehe der Mini-Ampel bei 100 %, in Punkt.
pub const MINI_H: f64 = 30.0;

/// Breite der Mini-Ampel bei 100 %, je nachdem was angezeigt wird (Punkt).
/// Nur Ampel = Kreis. Die Werte passen zu ui/mini.css.
pub fn mini_width(cfg: &crate::config::Config) -> f64 {
    match (cfg.show_duration, cfg.show_arrival) {
        (false, false) => MINI_H,
        (true, false) => 104.0,
        (false, true) => 84.0,
        (true, true) => 154.0,
    }
}

/// Fenstergroesse setzen, ohne die Position anzufassen.
pub fn size_mini(w: &tauri::WebviewWindow, cfg: &crate::config::Config) {
    let s = cfg.pill_scale;
    let _ = w.set_size(tauri::LogicalSize::new(mini_width(cfg) * s, MINI_H * s));
}

/// Groesse aendern; steht die Pille in der rechten Bildschirmhaelfte, bleibt ihr
/// rechter Rand stehen, damit sie nicht aus dem Bild waechst.
fn resize_mini_anchored(w: &tauri::WebviewWindow, cfg: &crate::config::Config) {
    let s = cfg.pill_scale;
    if let (Ok(pos), Ok(size), Ok(Some(mon)), Ok(sf)) =
        (w.outer_position(), w.outer_size(), w.current_monitor(), w.scale_factor())
    {
        let new_w = (mini_width(cfg) * s * sf).round() as i32;
        let old_w = size.width as i32;
        let mid = mon.position().x + mon.size().width as i32 / 2;
        if new_w != old_w && pos.x + old_w / 2 > mid {
            let _ = w.set_position(tauri::PhysicalPosition::new(pos.x + old_w - new_w, pos.y));
        }
    }
    size_mini(w, cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_with_minutes() {
        let blank = dot_with_minutes(Color::Green, None);
        let with = dot_with_minutes(Color::Green, Some(52));
        assert_eq!((with.width(), with.height()), (32, 32));
        let white = |img: &Image| img.rgba().chunks(4).filter(|p| p == &[255, 255, 255, 255]).count();
        assert_eq!(white(&blank), 0);
        assert!(white(&with) > 40, "Ziffern fehlen");
        // auf Gelb dunkle Schrift
        let y = dot_with_minutes(Color::Yellow, Some(7));
        assert!(y.rgba().chunks(4).any(|p| p == [28, 28, 30, 255]));
        // ab 100 Minuten "99+" statt Ueberlauf
        let big = dot_with_minutes(Color::Red, Some(123));
        assert!(white(&big) > white(&dot_with_minutes(Color::Red, Some(99))));
    }

    #[test]
    fn widths_follow_options() {
        let mut c = crate::config::Config::default();
        assert_eq!(mini_width(&c), 104.0);
        c.show_duration = false;
        assert_eq!(mini_width(&c), MINI_H);
        c.show_arrival = true;
        assert_eq!(mini_width(&c), 84.0);
        c.show_duration = true;
        assert_eq!(mini_width(&c), 154.0);
    }
}

/// Runder Punkt in Ampelfarbe als Menueleisten-Symbol (22 pt bei 2x).
fn dot(color: Color) -> Image<'static> {
    const S: usize = 44;
    let [r, g, b] = color.rgb();
    let c = (S as f32 - 1.0) / 2.0;
    let radius = 13.0_f32;
    let mut px = vec![0u8; S * S * 4];
    for y in 0..S {
        for x in 0..S {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let a = (radius - d + 0.5).clamp(0.0, 1.0);
            let i = (y * S + x) * 4;
            px[i] = r;
            px[i + 1] = g;
            px[i + 2] = b;
            px[i + 3] = (a * 255.0) as u8;
        }
    }
    Image::new_owned(px, S as u32, S as u32)
}

/// 5x7-Pixelschrift fuer Ziffern (je Zeile 5 Bit, linkes Pixel = Bit 4).
const DIGITS: [[u8; 7]; 10] = [
    [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
    [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
    [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
    [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
    [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
    [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
    [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
    [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
    [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
    [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
];

/// Taskleisten-Symbol fuer Windows (32 x 32): Kreis in Ampelfarbe mit den
/// Minuten darin, weil Windows neben dem Symbol keinen Text zeigen kann.
#[cfg_attr(not(windows), allow(dead_code))]
fn dot_with_minutes(color: Color, minutes: Option<i64>) -> Image<'static> {
    const S: usize = 32;
    let [r, g, b] = color.rgb();
    let c = (S as f32 - 1.0) / 2.0;
    let mut px = vec![0u8; S * S * 4];
    for y in 0..S {
        for x in 0..S {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let a = (15.5 - d + 0.5).clamp(0.0, 1.0);
            let i = (y * S + x) * 4;
            px[i..i + 4].copy_from_slice(&[r, g, b, (a * 255.0) as u8]);
        }
    }
    let Some(m) = minutes else { return Image::new_owned(px, S as u32, S as u32) };
    let ink: [u8; 3] = if color == Color::Yellow { [28, 28, 30] } else { [255, 255, 255] };
    let shown = m.clamp(0, 99) as usize;
    let digits: Vec<usize> = if shown >= 10 { vec![shown / 10, shown % 10] } else { vec![shown] };
    // Ziffern doppelt gross (10 x 14), 2 px Abstand, mittig
    let width = digits.len() * 10 + (digits.len() - 1) * 2 + if m > 99 { 4 } else { 0 };
    let x0 = (S - width) / 2;
    let y0 = (S - 14) / 2;
    let mut set = |x: usize, y: usize| {
        if x < S && y < S {
            let i = (y * S + x) * 4;
            px[i..i + 4].copy_from_slice(&[ink[0], ink[1], ink[2], 255]);
        }
    };
    for (n, d) in digits.iter().enumerate() {
        let ox = x0 + n * 12;
        for (row, bits) in DIGITS[*d].iter().enumerate() {
            for col in 0..5 {
                if bits & (0x10 >> col) != 0 {
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        set(ox + col * 2 + dx, y0 + row * 2 + dy);
                    }
                }
            }
        }
    }
    if m > 99 {
        // kleines Plus oben rechts: "99+"
        let ox = x0 + digits.len() * 12;
        for k in 0..3 {
            set(ox + k, y0 + 1);
            set(ox + 1, y0 + k);
        }
    }
    Image::new_owned(px, S as u32, S as u32)
}

/// Symbol je Plattform: macOS Punkt (Minuten stehen als Text daneben),
/// Windows Punkt mit Minuten darin.
fn tray_icon(s: &Status) -> Image<'static> {
    #[cfg(windows)]
    {
        let m = if s.kind == Kind::Setup { None } else { s.minutes };
        dot_with_minutes(s.color, m)
    }
    #[cfg(not(windows))]
    {
        dot(s.color)
    }
}

fn tray_title(s: &Status) -> String {
    match (s.kind, s.minutes) {
        (Kind::Setup, _) => " Einrichten".into(),
        (_, Some(m)) => format!(" {m}′"),
        (_, None) => " –".into(),
    }
}

fn summary(s: &Status) -> String {
    match (s.minutes, &s.message) {
        (Some(m), msg) => {
            let delay = s.delay_min.filter(|d| *d > 0).map(|d| format!(" (+{d} Verkehr)")).unwrap_or_default();
            let line = format!("{} → {}: {m} min{delay}", s.origin, s.dest);
            match msg {
                Some(msg) => format!("{line} · {msg}"),
                None => format!("{line} · Stand {}", s.updated_at.clone().unwrap_or_default()),
            }
        }
        (None, Some(msg)) => msg.clone(),
        (None, None) => "Noch kein Wert".into(),
    }
}

pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let st = state::shared(app);
    let cfg = st.cfg();
    let status = st.status();
    let today = Local::now().format("%Y-%m-%d").to_string();
    let manual = cfg.manual.as_ref().filter(|m| m.date == today).map(|m| m.direction);

    let info = MenuItem::with_id(app, "info", summary(&status), false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Einstellungen …", true, Some("CmdOrCtrl+,"))?;
    let refresh = MenuItem::with_id(app, "refresh", "Jetzt aktualisieren", true, Some("CmdOrCtrl+R"))?;
    let auto = CheckMenuItem::with_id(app, "dir_auto", "Richtung automatisch", true, manual.is_none(), None::<&str>)?;
    let to_work = CheckMenuItem::with_id(
        app,
        "dir_work",
        format!("Heute fix: nach {}", cfg.work_name),
        true,
        manual == Some(Direction::ToWork),
        None::<&str>,
    )?;
    let to_home = CheckMenuItem::with_id(
        app,
        "dir_home",
        format!("Heute fix: nach {}", cfg.home_name),
        true,
        manual == Some(Direction::ToHome),
        None::<&str>,
    )?;
    let mini = CheckMenuItem::with_id(app, "toggle_mini", "Mini-Ampel zeigen", true, cfg.show_mini, None::<&str>)?;
    let top = CheckMenuItem::with_id(app, "toggle_top", "Immer im Vordergrund", true, cfg.always_on_top, None::<&str>)?;
    let dur = CheckMenuItem::with_id(app, "toggle_duration", "Fahrzeit zeigen", true, cfg.show_duration, None::<&str>)?;
    let arr = CheckMenuItem::with_id(app, "toggle_arrival", "Ankunftszeit zeigen", true, cfg.show_arrival, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Roadrunner beenden", true, Some("CmdOrCtrl+Q"))?;
    let sep = || PredefinedMenuItem::separator(app);

    Menu::with_items(
        app,
        &[&info, &sep()?, &refresh, &open, &sep()?, &auto, &to_work, &to_home, &sep()?, &mini, &dur, &arr, &top, &sep()?, &quit],
    )
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(dot(Color::Grey))
        .icon_as_template(false)
        .title(" –")
        .tooltip("Roadrunner")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .build(app)?;
    Ok(())
}

pub fn update_tray(app: &AppHandle, s: &Status) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let _ = tray.set_icon(Some(tray_icon(s)));
    let _ = tray.set_title(Some(tray_title(s)));
    let _ = tray.set_tooltip(Some(summary(s)));
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.unminimize();
        // Windows laesst Hintergrundprozesse kein Fenster nach vorne holen
        // (Foreground Lock). Kurz "topmost" setzen und wieder loesen hebt es an.
        #[cfg(windows)]
        {
            let _ = w.set_always_on_top(true);
            let _ = w.set_always_on_top(false);
        }
        let _ = w.set_focus();
    }
}

/// Sichtbarkeit, Ebene, Groesse, Deckkraft und Erscheinungsbild anpassen.
pub fn apply_mini(app: &AppHandle) {
    let cfg = state::shared(app).cfg();
    let theme = match cfg.appearance {
        Appearance::System => None,
        Appearance::Light => Some(tauri::Theme::Light),
        Appearance::Dark => Some(tauri::Theme::Dark),
    };
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.set_theme(theme);
    }
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.set_theme(theme);
        let _ = w.set_always_on_top(cfg.always_on_top);
        crate::platform::set_level(&w, cfg.always_on_top);
        crate::platform::set_alpha(&w, cfg.pill_opacity);
        resize_mini_anchored(&w, &cfg);
        // macOS: Popover-Glas folgt hell/dunkel (HudWindow waere immer dunkel).
        // Windows: die Pille malt ihren Hintergrund im CSS selbst.
        #[cfg(target_os = "macos")]
        {
            let s = cfg.pill_scale;
            let _ = w.set_effects(
                tauri::window::EffectsBuilder::new()
                    .effect(tauri::window::Effect::Popover)
                    .state(tauri::window::EffectState::Active)
                    .radius(MINI_H * s / 2.0)
                    .build(),
            );
        }
        if cfg.show_mini {
            let _ = w.show();
        } else {
            let _ = w.hide();
        }
    }
}

pub fn set_manual(app: &AppHandle, dir: Option<Direction>) {
    let st = state::shared(app);
    {
        let mut c = st.cfg.lock().unwrap();
        c.manual = dir.map(|direction| ManualOverride { direction, date: Local::now().format("%Y-%m-%d").to_string() });
    }
    st.save_now();
    st.kick();
    let _ = tauri::Emitter::emit(app, "config", st.cfg());
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn toggle(app: &AppHandle, f: impl FnOnce(&mut crate::config::Config)) {
    let st = state::shared(app);
    f(&mut st.cfg.lock().unwrap());
    st.save_now();
    apply_mini(app);
    update_tray(app, &st.status());
    let _ = tauri::Emitter::emit(app, "config", st.cfg());
}

pub fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open" => open_settings(app),
        "refresh" => state::shared(app).kick(),
        "dir_auto" => set_manual(app, None),
        "dir_work" => set_manual(app, Some(Direction::ToWork)),
        "dir_home" => set_manual(app, Some(Direction::ToHome)),
        "toggle_mini" => toggle(app, |c| c.show_mini = !c.show_mini),
        "toggle_top" => toggle(app, |c| c.always_on_top = !c.always_on_top),
        "toggle_duration" => toggle(app, |c| c.show_duration = !c.show_duration),
        "toggle_arrival" => toggle(app, |c| c.show_arrival = !c.show_arrival),
        "quit" => {
            state::shared(app).save_now();
            app.exit(0);
        }
        _ => {}
    }
}
