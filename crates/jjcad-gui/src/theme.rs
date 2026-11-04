//! Farben, Schriften und Grundstile der Benutzeroberfläche.
//!
//! **Ein helles und ein dunkles Design (T0102).** Die Farben stehen in genau
//! zwei Tabellen ([`DUNKEL`], [`HELL`]); `Palette::…()` liefert den Wert des
//! gerade eingestellten Designs. Kein Zeichenweg hält eine Farbe fest — sonst
//! bliebe beim Umschalten ein Teil der Oberfläche im alten Design stehen.
//! Die Wahl selbst liegt in `prefs.json` (`Design`), Vorgabe ist „System“:
//! dann gilt das helle oder dunkle Farbschema des Betriebssystems.

use egui::{Color32, FontFamily, FontId, Frame, Margin, Rounding, Stroke, TextStyle, Visuals};
use jjcad_core::persist::Design;
use std::sync::atomic::{AtomicBool, Ordering};

/// Alle Farben eines Designs. Eine Zeile je Aufgabe — Flächen von außen nach
/// innen, dann Schrift, dann die Bedienfarben.
#[derive(Clone, Copy)]
struct Farben {
    bg: Color32,
    panel: Color32,
    panel_alt: Color32,
    ribbon: Color32,
    border: Color32,
    border_soft: Color32,
    /// Rand eines Eingabefelds (etwas kräftiger als `border_soft`)
    feld_rand: Color32,
    text: Color32,
    text_dim: Color32,
    text_faint: Color32,
    accent: Color32,
    accent_soft: Color32,
    accent_text: Color32,
    danger: Color32,
    ok: Color32,
    paper: Color32,
    paper_shadow: Color32,
    select: Color32,
    hover: Color32,
    track: Color32,
    grid_minor: Color32,
    grid_major: Color32,
    /// Fläche gesperrter Tasten im Tastenbild
    taste_gesperrt: Color32,
    /// Beschriftung gesperrter Tasten
    taste_gesperrt_text: Color32,
    /// Fläche der Beschriftung am Zeiger (dunkle bzw. helle Box)
    hinweis_flaeche: Color32,
    /// Fläche des schwebenden Kachels beim Ziehen aus der Hotbar
    schwebend: Color32,
}

/// Bisheriges Aussehen: dunkle Flächen, helle Schrift.
const DUNKEL: Farben = Farben {
    bg: Color32::from_rgb(0x12, 0x14, 0x18),
    panel: Color32::from_rgb(0x1A, 0x1D, 0x22),
    panel_alt: Color32::from_rgb(0x21, 0x25, 0x2B),
    ribbon: Color32::from_rgb(0x17, 0x1A, 0x1F),
    border: Color32::from_rgb(0x2C, 0x32, 0x3A),
    border_soft: Color32::from_rgb(0x24, 0x29, 0x30),
    feld_rand: Color32::from_rgb(0x35, 0x3B, 0x43),
    text: Color32::from_rgb(0xE7, 0xE9, 0xEC),
    text_dim: Color32::from_rgb(0x99, 0xA2, 0xAD),
    text_faint: Color32::from_rgb(0x6C, 0x75, 0x81),
    accent: Color32::from_rgb(0xE3, 0xA2, 0x3D),
    accent_soft: Color32::from_rgb(0x3A, 0x2E, 0x1B),
    accent_text: Color32::from_rgb(0x1A, 0x15, 0x0C),
    danger: Color32::from_rgb(0xD8, 0x60, 0x4F),
    ok: Color32::from_rgb(0x5A, 0xB5, 0x82),
    paper: Color32::from_rgb(0xFB, 0xFA, 0xF6),
    paper_shadow: Color32::from_rgb(0x0A, 0x0B, 0x0D),
    select: Color32::from_rgb(0x2F, 0x8C, 0xF0),
    hover: Color32::from_rgb(0x59, 0xA8, 0xFF),
    track: Color32::from_rgb(0x24, 0xB6, 0xA0),
    grid_minor: Color32::from_rgb(0xE9, 0xE7, 0xE0),
    grid_major: Color32::from_rgb(0xD8, 0xD5, 0xCB),
    taste_gesperrt: Color32::from_rgb(0x2A, 0x2F, 0x36),
    taste_gesperrt_text: Color32::from_rgb(0x5C, 0x64, 0x6E),
    hinweis_flaeche: Color32::from_rgba_premultiplied(24, 27, 32, 235),
    schwebend: Color32::from_rgba_premultiplied(30, 34, 40, 230),
};

/// Helles Design: helle Flächen, dunkle Schrift. Die Zeichenfläche bleibt in
/// beiden Designs Papier (weiß) — nur die Bedienoberfläche wechselt, damit die
/// Zeichnung so aussieht wie im Druck.
const HELL: Farben = Farben {
    bg: Color32::from_rgb(0xE4, 0xE7, 0xEB),
    panel: Color32::from_rgb(0xF7, 0xF8, 0xFA),
    panel_alt: Color32::from_rgb(0xEB, 0xEE, 0xF2),
    ribbon: Color32::from_rgb(0xFC, 0xFC, 0xFD),
    border: Color32::from_rgb(0xC3, 0xC9, 0xD1),
    border_soft: Color32::from_rgb(0xD7, 0xDC, 0xE2),
    feld_rand: Color32::from_rgb(0xC9, 0xCF, 0xD6),
    text: Color32::from_rgb(0x1C, 0x20, 0x26),
    text_dim: Color32::from_rgb(0x53, 0x5B, 0x66),
    text_faint: Color32::from_rgb(0x7E, 0x86, 0x91),
    accent: Color32::from_rgb(0xA9, 0x6C, 0x0B),
    accent_soft: Color32::from_rgb(0xF7, 0xE7, 0xC4),
    accent_text: Color32::from_rgb(0xFF, 0xFB, 0xF2),
    danger: Color32::from_rgb(0xBE, 0x3A, 0x2B),
    ok: Color32::from_rgb(0x2C, 0x8A, 0x5C),
    paper: Color32::from_rgb(0xFB, 0xFA, 0xF6),
    paper_shadow: Color32::from_rgb(0x9A, 0xA1, 0xAA),
    select: Color32::from_rgb(0x1F, 0x6F, 0xD2),
    hover: Color32::from_rgb(0x1B, 0x77, 0xE0),
    track: Color32::from_rgb(0x0E, 0x8B, 0x79),
    // Raster und Achsen liegen auf dem Papier — in beiden Designs gleich.
    grid_minor: Color32::from_rgb(0xE9, 0xE7, 0xE0),
    grid_major: Color32::from_rgb(0xD8, 0xD5, 0xCB),
    taste_gesperrt: Color32::from_rgb(0xAE, 0xB6, 0xC0),
    taste_gesperrt_text: Color32::from_rgb(0x9A, 0xA2, 0xAC),
    hinweis_flaeche: Color32::from_rgba_premultiplied(241, 243, 246, 245),
    schwebend: Color32::from_rgba_premultiplied(255, 255, 255, 240),
};

/// Steht das Design auf hell? Prozessweit wie die Sprache (T0103) — alle
/// Zeichenwege lesen denselben Wert.
static HELL_AKTIV: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Design des **eigenen Fadens**, sobald dieser es selbst umgestellt hat.
    ///
    /// In der Anwendung gibt es nur einen Zeichenfaden, dort ist beides
    /// dasselbe. Die Tests laufen dagegen in Fäden nebeneinander und stellen
    /// das Design um; ein Faden, der zeichnet und danach `Palette::…()` liest,
    /// sah dann zwei Designs (gezeichnet `#2A2F36`, erwartet `#AEB6C0` —
    /// `dialogs::tests_tastatur::gesperrte_tasten_haben_eigene_flaeche` fiel
    /// beim Ausliefern zufällig aus, T0099). Fäden ohne eigene Wahl lesen
    /// weiter den prozessweiten Wert.
    static HELL_IM_FADEN: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Gilt das helle Design in diesem Faden?
fn hell_jetzt() -> bool {
    HELL_IM_FADEN
        .with(|z| z.get())
        .unwrap_or_else(|| HELL_AKTIV.load(Ordering::Relaxed))
}

/// Design setzen — prozessweit **und** für den aufrufenden Faden.
fn setze_hell(hell: bool) {
    HELL_AKTIV.store(hell, Ordering::Relaxed);
    HELL_IM_FADEN.with(|z| z.set(Some(hell)));
}

/// Letzte Prüfung des Betriebssystem-Farbschemas: (Zeitpunkt, war es hell?).
/// Ohne Drosselung würde jede Bildschleife die Konfigurationsdateien lesen.
static LETZTE_PRUEFUNG: std::sync::Mutex<(f64, bool)> = std::sync::Mutex::new((0.0, false));

fn farben() -> Farben {
    if hell_jetzt() {
        HELL
    } else {
        DUNKEL
    }
}

/// Die Farben des eingestellten Designs.
///
/// Aufruf über `Palette::bg()` & Co. — bewusst Methoden statt Konstanten: die
/// Werte hängen an der Einstellung und dürfen nicht in `const`-Kontexte
/// wandern (dort bliebe das Design beim Umschalten stehen).
pub struct Palette;

impl Palette {
    pub fn bg() -> Color32 {
        farben().bg
    }
    pub fn panel() -> Color32 {
        farben().panel
    }
    pub fn panel_alt() -> Color32 {
        farben().panel_alt
    }
    pub fn ribbon() -> Color32 {
        farben().ribbon
    }
    pub fn border() -> Color32 {
        farben().border
    }
    pub fn border_soft() -> Color32 {
        farben().border_soft
    }
    pub fn feld_rand() -> Color32 {
        farben().feld_rand
    }
    pub fn text() -> Color32 {
        farben().text
    }
    pub fn text_dim() -> Color32 {
        farben().text_dim
    }
    pub fn text_faint() -> Color32 {
        farben().text_faint
    }
    pub fn accent() -> Color32 {
        farben().accent
    }
    pub fn accent_soft() -> Color32 {
        farben().accent_soft
    }
    pub fn accent_text() -> Color32 {
        farben().accent_text
    }
    pub fn danger() -> Color32 {
        farben().danger
    }
    pub fn ok() -> Color32 {
        farben().ok
    }
    pub fn paper() -> Color32 {
        farben().paper
    }
    pub fn paper_shadow() -> Color32 {
        farben().paper_shadow
    }
    pub fn select() -> Color32 {
        farben().select
    }
    pub fn hover() -> Color32 {
        farben().hover
    }
    pub fn track() -> Color32 {
        farben().track
    }
    pub fn grid_minor() -> Color32 {
        farben().grid_minor
    }
    pub fn grid_major() -> Color32 {
        farben().grid_major
    }
    pub fn taste_gesperrt() -> Color32 {
        farben().taste_gesperrt
    }
    pub fn taste_gesperrt_text() -> Color32 {
        farben().taste_gesperrt_text
    }
    pub fn hinweis_flaeche() -> Color32 {
        farben().hinweis_flaeche
    }
    pub fn schwebend() -> Color32 {
        farben().schwebend
    }
}

/// Ist das gerade gezeichnete Design hell?
pub fn ist_hell() -> bool {
    hell_jetzt()
}

/// Auflösen der Wahl: `System` folgt dem Betriebssystem, Hell und Dunkel sind
/// fest. Reine Rechnung — damit prüfbar ohne Fenster.
pub fn aufloesen(design: Design, system_hell: bool) -> bool {
    match design {
        Design::Hell => true,
        Design::Dunkel => false,
        Design::System => system_hell,
    }
}

/// Will das Betriebssystem ein helles Farbschema?
///
/// Zuerst die Meldung der Fensterbibliothek (`ctx.system_theme()`) — sie kommt
/// auf Windows und macOS an. Unter Linux meldet winit nichts (`None`), dort
/// entscheiden die Einstellungen des Desktops: KDE (`kdeglobals`), GTK
/// (`settings.ini`), dann die Umgebung. Ohne Auskunft bleibt es dunkel — das
/// bisherige Aussehen.
pub fn system_ist_hell(ctx: &egui::Context) -> bool {
    if let Some(theme) = ctx.system_theme() {
        return theme == egui::Theme::Light;
    }
    betriebssystem_ist_hell()
}

#[cfg(target_os = "linux")]
fn betriebssystem_ist_hell() -> bool {
    if let Some(hell) = kde_ist_hell() {
        return hell;
    }
    if let Some(hell) = gtk_ist_hell() {
        return hell;
    }
    if let Ok(gtk) = std::env::var("GTK_THEME") {
        let gtk = gtk.to_lowercase();
        if gtk.contains("dark") {
            return false;
        }
        if gtk.contains("light") {
            return true;
        }
    }
    // `COLORFGBG` = „Vordergrund;Hintergrund“ in ANSI-Nummern: 7/15 ist Weiß.
    if let Ok(fg) = std::env::var("COLORFGBG") {
        if let Some(bg) = fg.rsplit(';').next() {
            if let Ok(n) = bg.trim().parse::<u32>() {
                return n == 7 || n == 15;
            }
        }
    }
    false
}

#[cfg(not(target_os = "linux"))]
fn betriebssystem_ist_hell() -> bool {
    false
}

/// Konfigurationsverzeichnis des Nutzers (`$XDG_CONFIG_HOME` oder `~/.config`).
#[cfg(target_os = "linux")]
fn konfig_datei(relativ: &str) -> Option<std::path::PathBuf> {
    let basis = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(basis.join(relativ))
}

/// KDE schreibt die Hintergrundfarbe der Fenster nach `kdeglobals`. Helle
/// Fläche = helles Design; das ist genauer als der Name des Farbschemas
/// („BreezeDark“ enthält „dark“, ein eigenes Schema vielleicht nicht).
#[cfg(target_os = "linux")]
fn kde_ist_hell() -> Option<bool> {
    let text = std::fs::read_to_string(konfig_datei("kdeglobals")?).ok()?;
    let mut abschnitt = String::new();
    let mut farbschema = None;
    for zeile in text.lines() {
        let zeile = zeile.trim();
        if zeile.starts_with('[') {
            abschnitt = zeile.trim_matches(['[', ']']).to_string();
            continue;
        }
        if abschnitt == "Colors:Window" {
            if let Some(wert) = zeile.strip_prefix("BackgroundNormal=") {
                if let Some(hell) = rgb_ist_hell(wert) {
                    return Some(hell);
                }
            }
        }
        if abschnitt == "General" {
            if let Some(wert) = zeile.strip_prefix("ColorScheme=") {
                farbschema = Some(wert.trim().to_lowercase());
            }
        }
    }
    let name = farbschema?;
    if name.contains("dark") {
        Some(false)
    } else if name.contains("light") {
        Some(true)
    } else {
        None
    }
}

/// `r,g,b` → ist die Fläche hell? (Wahrnehmungshelligkeit, nicht Mittelwert.)
#[cfg(target_os = "linux")]
fn rgb_ist_hell(wert: &str) -> Option<bool> {
    let teile: Vec<f32> = wert
        .split(',')
        .filter_map(|t| t.trim().parse::<f32>().ok())
        .collect();
    if teile.len() < 3 {
        return None;
    }
    let h = 0.299 * teile[0] + 0.587 * teile[1] + 0.114 * teile[2];
    Some(h > 128.0)
}

/// GTK: „prefer dark“ in `settings.ini` (GNOME, XFCE, …). Ohne Datei oder
/// ohne Aussage `None`.
#[cfg(target_os = "linux")]
fn gtk_ist_hell() -> Option<bool> {
    for rel in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
        let Ok(text) = std::fs::read_to_string(konfig_datei(rel)?) else {
            continue;
        };
        let mut dunkel = None;
        for zeile in text.lines() {
            let zeile = zeile.trim();
            if let Some(wert) = zeile.strip_prefix("gtk-application-prefer-dark-theme=") {
                dunkel = Some(!wert.trim().eq_ignore_ascii_case("true"));
            } else if let Some(wert) = zeile.strip_prefix("gtk-theme-name=") {
                if dunkel.is_none() {
                    dunkel = Some(!wert.to_lowercase().contains("dark"));
                }
            }
        }
        if let Some(hell) = dunkel {
            return Some(hell);
        }
    }
    None
}

/// Design umstellen — gilt ab dem nächsten Bild, ohne Neustart.
pub fn set_design(design: Design, ctx: &egui::Context) {
    let system_hell = system_ist_hell(ctx);
    setze_hell(aufloesen(design, system_hell));
    merke_pruefung(ctx, system_hell);
    stil(ctx);
    ctx.request_repaint();
}

fn merke_pruefung(ctx: &egui::Context, system_hell: bool) {
    let jetzt = ctx.input(|i| i.time);
    if let Ok(mut zelle) = LETZTE_PRUEFUNG.lock() {
        *zelle = (jetzt, system_hell);
    }
}

/// Bei der Wahl „System“: hat das Betriebssystem sein Farbschema gewechselt?
///
/// Höchstens alle zwei Sekunden nachsehen — die Konfigurationsdateien zu lesen
/// kostet mehr als ein Bild. Wird nur bei `Design::System` gerufen.
pub fn system_nachziehen(ctx: &egui::Context) {
    let jetzt = ctx.input(|i| i.time);
    let Ok(mut zelle) = LETZTE_PRUEFUNG.lock() else {
        return;
    };
    if jetzt - zelle.0 < 2.0 {
        return;
    }
    let system_hell = system_ist_hell(ctx);
    zelle.0 = jetzt;
    if system_hell != zelle.1 {
        zelle.1 = system_hell;
        setze_hell(system_hell);
        drop(zelle);
        stil(ctx);
        ctx.request_repaint();
    }
}


pub fn paper_frame() -> Frame {
    Frame::none().fill(Palette::panel()).inner_margin(Margin::same(0.0))
}

pub fn card() -> Frame {
    Frame::none()
        .fill(Palette::panel_alt())
        .rounding(Rounding::same(8.0))
        .stroke(Stroke::new(1.0, Palette::border_soft()))
        .inner_margin(Margin::symmetric(10.0, 8.0))
}

pub fn dialog_frame() -> Frame {
    Frame::none()
        .fill(Palette::panel())
        .rounding(Rounding::same(12.0))
        .stroke(Stroke::new(1.0, Palette::border()))
        .inner_margin(Margin::same(18.0))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 12.0),
            blur: 32.0,
            spread: 0.0,
            color: Color32::from_black_alpha(140),
        })
}

/// Schriften und Grundstil einrichten — einmal beim Start.
pub fn install(ctx: &egui::Context) {
    schriften(ctx);
    stil(ctx);
}

/// Die Schriften. Sie hängen nicht am Design und werden nur beim Start gesetzt
/// (ein erneutes `set_fonts` würde die Texturen neu bauen).
fn schriften(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Systemschrift laden (falls vorhanden), sonst egui-Standardschrift
    for path in [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("app".to_owned(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "app".to_owned());
            break;
        }
    }
    for path in [
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        "C:\\Windows\\Fonts\\segoeuib.ttf",
        "C:\\Windows\\Fonts\\arialbd.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("app-bold".to_owned(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(FontFamily::Name("bold".into()))
                .or_default()
                .push("app-bold".to_owned());
            break;
        }
    }
    // Schriften für Texte der Zeichnung: je Familie ein eigener Satz, damit
    // „Serifen", „Schmal", „Rund" und die technischen Schriften auch auf der
    // Zeichenfläche so aussehen wie im Druck. Die Daten kommen aus dem Kern:
    // Systemschriften und die mitgelieferten DIN 1451 / EN ISO 3098.
    for font in jjcad_core::model::TextFont::ALL {
        for (stil, anhang) in [
            (jjcad_core::fonts::FontStyle::Regular, ""),
            (jjcad_core::fonts::FontStyle::Bold, "_fett"),
        ] {
            let Some(bytes) = jjcad_core::fonts::ttf_bytes(font, stil) else {
                continue;
            };
            let name = format!("schrift_{}{}", font.key(), anhang);
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes.to_vec()).into());
            fonts
                .families
                .entry(egui::FontFamily::Name(name.clone().into()))
                .or_default()
                .push(name);
        }
    }
    ctx.set_fonts(fonts);
}

/// Der Stil des eingestellten Designs: Flächen, Rahmen, Schriftfarben und die
/// Maße der Bedienelemente. Läuft beim Start **und** bei jedem Designwechsel.
pub fn stil(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(19.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();

    let mut v = if ist_hell() {
        Visuals::light()
    } else {
        Visuals::dark()
    };
    v.override_text_color = Some(Palette::text());
    v.panel_fill = Palette::panel();
    v.window_fill = Palette::panel();
    v.extreme_bg_color = Palette::bg();
    v.faint_bg_color = Palette::panel_alt();
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(8.0);
    v.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 10.0),
        blur: 30.0,
        spread: 0.0,
        color: Color32::from_black_alpha(150),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: Color32::from_black_alpha(120),
    };
    v.window_stroke = Stroke::new(1.0, Palette::border());
    v.selection.bg_fill = Palette::accent_soft();
    v.selection.stroke = Stroke::new(1.0, Palette::accent());
    v.widgets.noninteractive.bg_fill = Palette::panel();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Palette::border_soft());
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Palette::text_dim());
    v.widgets.inactive.bg_fill = Palette::panel_alt();
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::border_soft());
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, Palette::text());
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x2B, 0x31, 0x39);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::border());
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Palette::text());
    v.widgets.active.bg_fill = Palette::accent_soft();
    v.widgets.active.bg_stroke = Stroke::new(1.0, Palette::accent());
    v.widgets.active.fg_stroke = Stroke::new(1.0, Palette::accent());
    v.widgets.open.bg_fill = Palette::panel_alt();
    v.widgets.open.bg_stroke = Stroke::new(1.0, Palette::border());
    v.button_frame = true;
    v.striped = false;
    v.slider_trailing_fill = true;
    // Einheitliche Feld-Optik (Dropdowns, Buttons): flach, dünne Kontur, gleiche Rundung
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(4.0);
        w.bg_fill = Palette::bg();
        w.weak_bg_fill = Palette::bg();
    }
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::feld_rand());
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, Palette::text());
    v.widgets.hovered.bg_fill = Palette::panel_alt();
    v.widgets.hovered.weak_bg_fill = Palette::panel_alt();
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::accent());
    v.widgets.open.bg_fill = Palette::panel_alt();
    v.widgets.open.weak_bg_fill = Palette::panel_alt();
    v.widgets.open.bg_stroke = Stroke::new(1.0, Palette::accent());
    v.widgets.active.bg_fill = Palette::panel_alt();
    v.widgets.active.weak_bg_fill = Palette::panel_alt();

    style.visuals = v;

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(28.0, 24.0);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.animation_time = 0.10;
    // egui hält zwei Stile (hell/dunkel) und wählt je Bild den zum Farbschema
    // des Betriebssystems. Unser Design bringt seine Farben selbst mit, also
    // bekommt **beide** Plätze derselben Stil. Steht er nur im gerade aktiven
    // Platz, zeichnet egui auf einem Rechner mit **hellem** Systemdesign seine
    // eigenen hellen Widgets über unsere dunklen Flächen: weiße Eingabefelder,
    // helle Trennstreifen zwischen den Panels (Windows 11, gemeldet als
    // „weiße Balken, sobald das System auf hell steht“).
    let stil = std::sync::Arc::new(style);
    ctx.options_mut(|o| {
        o.dark_style = stil.clone();
        o.light_style = stil;
    });

    // `JJCAD_UI_FEATHER=0` schaltet die weiche Kante der Flächen ab. Auf
    // Rechnern, deren Treiber an halbtransparenten Kanten helle Fugen zeigt
    // (Windows 11 mit krummer Skalierung), ist das ein Notausgang ohne neuen
    // Build: die Flächen enden dann hart auf dem Pixel.
    if std::env::var("JJCAD_UI_FEATHER").ok().as_deref() == Some("0") {
        ctx.tessellation_options_mut(|o| o.feathering = false);
    }
}

/// Kleine Überschrift in Panels.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.5)
            .color(Palette::text_faint())
            .strong(),
    );
    ui.add_space(2.0);
}

pub fn bold(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(size)
        .family(FontFamily::Name("bold".into()))
}

pub fn dim(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(11.5)
        .color(Palette::text_dim())
}

pub fn faint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(11.0)
        .color(Palette::text_faint())
}

pub fn mono(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .monospace()
        .size(11.5)
        .color(Palette::text_dim())
}

/// Einheitlicher "Chip"/Badge.
/// Wie [`chip`], aber mit fester Widget-ID — nötig für Kontextmenüs, weil sich
/// automatische IDs verschieben, sobald sich die Anzahl der Widgets davor ändert.
pub fn chip_id(ui: &mut egui::Ui, id: &str, text: &str, active: bool) -> egui::Response {
    ui.push_id(id, |ui| chip(ui, text, active)).inner
}

pub fn chip(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let (bg, fg, stroke) = if active {
        (Palette::accent_soft(), Palette::accent(), Stroke::new(1.0, Palette::accent()))
    } else {
        (Palette::panel_alt(), Palette::text_dim(), Stroke::new(1.0, Palette::border_soft()))
    };
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(11.5),
        fg,
    );
    let size = egui::vec2(galley.size().x + 16.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(11.0), bg);
        p.rect_stroke(rect, Rounding::same(11.0), stroke);
        p.galley(
            rect.center() - galley.size() / 2.0,
            galley,
            fg,
        );
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wahrnehmungshelligkeit einer Farbe (0…255) — Grundlage der Prüfungen
    /// „hell ist heller als dunkel“.
    fn helligkeit(c: Color32) -> f32 {
        0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
    }

    /// Ein helles Farbschema des Betriebssystems darf das eingestellte Design
    /// nicht umschalten.
    ///
    /// egui hält zwei Stile (hell/dunkel) und wählt sie je Bild nach der
    /// Meldung des Systems. Steht unser Stil nur im dunklen Platz, zeichnet
    /// egui auf einem Rechner mit **hellem** Systemdesign seine eigenen hellen
    /// Widgets über unsere dunklen Flächen: weiße Eingabefelder und helle
    /// Trennstreifen zwischen den Panels (gemeldet von Windows 11).
    #[test]
    fn helles_systemdesign_ueberschreibt_das_design_nicht() {
        let ctx = egui::Context::default();
        setze_hell(false);
        install(&ctx);
        // Ein Bild, in dem das System „hell“ meldet — so kommt es bei Windows
        // und macOS an, unter Linux bleibt die Meldung aus.
        let _ = ctx.run(
            egui::RawInput {
                system_theme: Some(egui::Theme::Light),
                ..Default::default()
            },
            |_ctx| {},
        );
        let v = ctx.style().visuals.clone();
        assert_eq!(
            v.panel_fill,
            Palette::panel(),
            "Panelfläche kommt nicht aus dem Design"
        );
        assert_eq!(
            v.extreme_bg_color,
            Palette::bg(),
            "Feldhintergrund folgt dem Systemdesign statt dem Design"
        );
        assert_eq!(
            v.widgets.inactive.bg_fill,
            Palette::bg(),
            "Widget-Fläche folgt dem Systemdesign statt dem Design"
        );
        // Gegenprobe mit dunklem Systemdesign: derselbe Stil.
        let _ = ctx.run(
            egui::RawInput {
                system_theme: Some(egui::Theme::Dark),
                ..Default::default()
            },
            |_ctx| {},
        );
        assert_eq!(
            ctx.style().visuals.panel_fill,
            Palette::panel(),
            "dunkles Systemdesign verliert das Design"
        );
    }

    /// T0102: „System“ folgt dem Betriebssystem, Hell und Dunkel sind fest.
    /// Ohne diese Rechnung hinge die Vorgabe an einem Zufall des Systems.
    #[test]
    fn design_aufloesen_folgt_dem_system() {
        assert!(aufloesen(Design::System, true), "System hell → hell");
        assert!(!aufloesen(Design::System, false), "System dunkel → dunkel");
        assert!(aufloesen(Design::Hell, false), "Hell überstimmt das System");
        assert!(!aufloesen(Design::Dunkel, true), "Dunkel überstimmt das System");
    }

    /// Beide Farbsätze sind vollständig und unterscheiden sich dort, wo das
    /// Design wirkt: Flächen, Schrift, Auswahlfarben. Papier und Raster bleiben
    /// gleich — die Zeichnung soll in beiden Designs wie im Druck aussehen.
    #[test]
    fn beide_designs_haben_eigene_flaechen_und_schrift() {
        // Helles Design: Flächen hell, Schrift dunkel — und umgekehrt.
        assert!(
            helligkeit(HELL.bg) > helligkeit(DUNKEL.bg) + 100.0,
            "die Fläche des hellen Designs ist nicht deutlich heller"
        );
        assert!(
            helligkeit(HELL.panel) > helligkeit(DUNKEL.panel) + 100.0,
            "die Panelfläche des hellen Designs ist nicht deutlich heller"
        );
        assert!(
            helligkeit(HELL.text) < helligkeit(DUNKEL.text) - 100.0,
            "die Schrift des hellen Designs ist nicht deutlich dunkler"
        );
        // Lesbarkeit: Schrift gegen Fläche (grober Abstand, kein Farbton).
        for (name, f) in [("hell", HELL), ("dunkel", DUNKEL)] {
            let abstand = (helligkeit(f.text) - helligkeit(f.bg)).abs();
            assert!(
                abstand > 90.0,
                "{name}: Schrift und Fläche liegen zu nah beieinander ({abstand:.0})"
            );
            // Das Papier ist die Zeichenfläche — heller als die Bedienfläche,
            // sonst wäre die Zeichnung der Hintergrund.
            assert!(
                helligkeit(f.paper) > helligkeit(f.bg),
                "{name}: das Papier hebt sich nicht von der Fläche ab"
            );
            assert_eq!(f.paper, DUNKEL.paper, "{name}: das Papier wechselt das Design");
        }
        // Gesperrte Tasten bleiben als Sperre erkennbar (eigene Fläche).
        // Im dunklen Design ist die Fläche heller, im hellen dunkler als die
        // freie Taste — gemessen am Betrag der Summe der Kanäle.
        let summe = |c: Color32| c.r() as i32 + c.g() as i32 + c.b() as i32;
        for (name, f) in [("hell", HELL), ("dunkel", DUNKEL)] {
            assert!(
                (summe(f.taste_gesperrt) - summe(f.bg)).abs() > 40,
                "{name}: gesperrte Tasten sind nicht abgesetzt"
            );
            // Die Beschriftung gesperrter Tasten tritt zurück: ihr Abstand zur
            // Panelfläche ist kleiner als der von `text_faint`.
            let abstand = |c: Color32| (summe(c) - summe(f.panel)).abs();
            assert!(
                abstand(f.taste_gesperrt_text) < abstand(f.text_faint),
                "{name}: die Beschriftung gesperrter Tasten ist nicht schwächer"
            );
        }
    }

    /// T0102: Umschalten tauscht die Farben sofort aus — `Palette::…()` liefert
    /// danach die Werte des anderen Designs. Der Test stellt den Ausgangszustand
    /// wieder her: die Wahl ist prozessweit (wie die Sprache, T0103).
    #[test]
    fn umschalten_tauscht_die_farben() {
        let ctx = egui::Context::default();
        set_design(Design::Dunkel, &ctx);
        let (dunkel_bg, dunkel_text, papier) = (Palette::bg(), Palette::text(), Palette::paper());
        assert!(!ist_hell());

        set_design(Design::Hell, &ctx);
        assert!(ist_hell(), "das helle Design wurde nicht übernommen");
        assert_ne!(Palette::bg(), dunkel_bg, "die Fläche blieb im alten Design");
        assert!(helligkeit(Palette::bg()) > helligkeit(dunkel_bg));
        assert!(helligkeit(Palette::text()) < helligkeit(dunkel_text));
        assert_eq!(Palette::paper(), papier, "das Papier wechselt mit");

        set_design(Design::Dunkel, &ctx);
        assert!(!ist_hell());
        assert_eq!(Palette::bg(), dunkel_bg, "zurück im dunklen Design");
        assert_eq!(Palette::text(), dunkel_text);
    }
}
