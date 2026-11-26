//! Anwendungszustand und Fensteraufbau von JoJa-CAD.

use crate::canvas;
use crate::dialogs::{self, FileDialog, FileMode};
use crate::hotbar;
use crate::inventory;
use crate::panels;
use crate::systemdialog::Systemdialog;
use crate::theme::{self, Palette};
use crate::widgets;
use egui::{Align, Color32, FontId, Frame, Layout, Margin, Pos2, Rect, RichText, Rounding, Sense, Stroke, Vec2};
use jjcad_core::actions::{self, ActionGroup};
use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{BBox, Vec2 as V2};
use jjcad_core::model::*;
use jjcad_core::persist::{self, DragMode, PanelSeite, UserPrefs};
use jjcad_core::render::DisplayList;
use jjcad_core::session::Session;
use jjcad_core::sitzung;
use jjcad_core::style::{Color, FillPattern};
use jjcad_core::units::{format_number, Unit};
use jjcad_core::{dxf, pdf, raster};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const TOOLBAR_H: f32 = 38.0;
/// Gesamthöhe der Ribbon-Leiste — für **alle** Reiter gleich (t_92b03440).
///
/// Setzt sich zusammen aus dem Innenabstand `RIBBON_PAD` oben und unten und der
/// Inhaltshöhe `RIBBON_CONTENT_H`. Der Inhalt selbst ist immer `RIBBON_GROUP_H`
/// hoch (Knopfreihe mit Beschriftung); kürzere Reiter werden zentriert, höhere
/// würden beschnitten statt das Panel zu vergrößern.
pub const RIBBON_H: f32 = 80.0;
pub const RIBBON_PAD: f32 = 5.0;
pub const RIBBON_CONTENT_H: f32 = RIBBON_H - 2.0 * RIBBON_PAD;
/// Höhe einer Ribbon-Gruppe: 54 px Knopfreihe + 2 px Abstand + 11 px Beschriftung.
pub const RIBBON_GROUP_H: f32 = 67.0;
pub const HEADER_H: f32 = 38.0;
pub const DOCTABS_H: f32 = 34.0;
/// Breite einer Seitenleiste mit den Panel-Symbolen (T0203). Beide Seiten
/// tragen dieselben Icons; die Leiste erscheint, sobald ein Panel eingeklappt
/// ist.
pub const LEISTE_W: f32 = 28.0;
/// Höhe der einen unteren Leiste: Slot-Kante (50 px) + 2 × 5 px Rand
pub const HOTBAR_H: f32 = 60.0;
/// Breite eines selbst gezeichneten Fensterknopfs (T0098) — ohne Titelleiste
/// sitzen Minimieren, Vergrößern und Schließen oben rechts in der Kopfzeile.
pub const FENSTERKNOPF_W: f32 = 46.0;
/// Breite der Zone an den Fensterkanten, in der ein Zug die Größe ändert
/// (T0098). Schmal genug, um die Zeichenfläche nicht zu stören, breit genug,
/// um sie ohne Zielen zu treffen — 5 px waren praktisch nicht auffindbar.
pub const FENSTERKANTE_PX: f32 = 8.0;
/// Kantenlänge der Ecken, in denen zwei Richtungen zugleich greifen.
pub const FENSTERECKE_PX: f32 = 24.0;
/// Breite des Streifens, in dem JoJa-CAD ohne Titelleiste den Fensterschatten
/// selbst zeichnet (T0147).
///
/// Der Fenstermanager zeichnet für ein Fenster ohne Dekoration keinen Schatten
/// mehr; das Fenster lag danach ohne Abhebung auf dem Hintergrund. Der Streifen
/// bleibt durchsichtig (Fenster mit `with_transparent`, `clear_color` mit
/// Alpha 0) und trägt den weichen Schatten; die Oberfläche liegt darin.
pub const SCHATTEN_RAND: f32 = 16.0;
/// Wie weit der Fensterschatten nach unten wandert (T0147) — ein Schatten ohne
/// Versatz wirkt wie ein Rahmen um das Fenster.
pub const SCHATTEN_VERSATZ: f32 = 4.0;
/// Verweildauer, bis die Kante unter dem Zeiger aufleuchtet (T0098, Turn 7).
///
/// Ohne Verzögerung blitzte die Markierung bei jeder schnellen Zeigerbewegung
/// über den Rand auf und lenkte ab. Sie erscheint erst, wenn der Zeiger
/// bewusst am Rand stehen bleibt.
pub const KANTEN_VERWEILDAUER: f64 = 0.1;
/// Nachlauf der Markierung nach dem Ende eines Zuges: sie bleibt nur, solange
/// sich die Fenstergröße ändert, und fällt kurz danach ab. Das Loslassen kommt
/// bei einem Ziehen am Rand nicht immer an (der Fenstermanager hält den Zeiger),
/// deshalb hängt das Ende an der Größenänderung und nicht am Tastenereignis.
pub const ZUG_NACHLAUF_S: f64 = 0.6;
/// Not-Aus: länger als das kann ein Zug am Rand nicht dauern — danach fällt die
/// Markierung auf jeden Fall ab (auch wenn die Größe weiter zappelt).
pub const ZUG_MAX_S: f64 = 20.0;

/// Auf ganze Bildschirm-Pixel runden.
///
/// Bei krummer Skalierung (125 %, 150 %, 175 %) liegen Panelkanten sonst auf
/// halben Pixeln: die Fläche endet dort halbtransparent, und zwischen zwei
/// Flächen bleibt eine 1 px breite Fuge, durch die der Grund scheint. Auf
/// ganze Pixel gerundete Höhen und Breiten schließen sie.
pub fn auf_pixel(ctx: &egui::Context, wert: f32) -> f32 {
    let ppp = ctx.pixels_per_point();
    if ppp <= 0.0 {
        return wert;
    }
    (wert * ppp).round() / ppp
}

/// Weg eines einzelnen Schwenk-Schritts in Bildpunkten (T0056): ein Aufruf der
/// Funktion rückt die Ansicht sichtbar weiter, ohne sie springen zu lassen.
/// Die Einstellung „Geschwindigkeit der Ansicht“ skaliert den Schritt über
/// `UserPrefs::pan_speed`.
///
/// T0146: Das **Grundtempo** ist um 50 % erhöht (vorher 60 px). Der Faktor
/// bleibt bei 1 × die Vorgabe — die Erhöhung wirkt damit auch für gespeicherte
/// Einstellungen, ohne dass der Nutzer den Faktor nachziehen muss.
pub const SCHWENK_SCHRITT_PX: f32 = 90.0;

/// Tempo des **kontinuierlichen** Schwenks in Bildpunkten je Sekunde (T0101):
/// solange eine Schwenkfunktion gedrückt bleibt, wandert die Ansicht mit
/// diesem Tempo weiter. `UserPrefs::pan_speed` skaliert auch dieses Tempo —
/// ein Faktor gilt damit für beide Bewegungsarten. T0146: 480 → 720 px/s
/// (dieselben 50 % wie beim Schritt).
pub const SCHWENK_TEMPO_PX_PRO_S: f32 = 720.0;

/// Längste Bilddauer, die der kontinuierliche Schwenk verrechnet (T0101):
/// nach einem Stocken (Fenster verdeckt, Neuzeichnen) soll die Ansicht nicht
/// um die stehengebliebene Zeit springen.
pub const SCHWENK_DT_MAX_S: f32 = 0.1;

/// Richtung einer Schwenkfunktion in Bildschirmkoordinaten (x rechts, y unten):
/// `view.pan_left` = (−1, 0), `view.pan_right` = (1, 0), `view.pan_up` =
/// (0, −1), `view.pan_down` = (0, 1). Eine Quelle für den Einzelschritt und
/// den Dauerlauf (T0101).
pub fn schwenk_richtung(action: &str) -> Option<(f32, f32)> {
    match action {
        "view.pan_left" => Some((-1.0, 0.0)),
        "view.pan_right" => Some((1.0, 0.0)),
        "view.pan_up" => Some((0.0, -1.0)),
        "view.pan_down" => Some((0.0, 1.0)),
        _ => None,
    }
}

/// Taste zu einem Namen der Tastaturbelegung (T0101). Buchstaben und Symbole
/// kommen aus [`EINZELTASTEN`], `"1"`…`"0"` sind die Hotbar-Ziffern. Ein
/// `"shift+"`-Vorsatz wird vom Aufrufer abgetrennt.
fn taste_zu_label(label: &str) -> Option<egui::Key> {
    if let Some((key, _)) = EINZELTASTEN.iter().find(|(_, l)| *l == label) {
        return Some(*key);
    }
    Some(match label {
        "1" => egui::Key::Num1,
        "2" => egui::Key::Num2,
        "3" => egui::Key::Num3,
        "4" => egui::Key::Num4,
        "5" => egui::Key::Num5,
        "6" => egui::Key::Num6,
        "7" => egui::Key::Num7,
        "8" => egui::Key::Num8,
        "9" => egui::Key::Num9,
        "0" => egui::Key::Num0,
        _ => return None,
    })
}

/// Zeichen, die **keine eigene `egui::Key`-Taste** haben — Umlaute, `ß` und
/// jedes weitere Sonderzeichen einer fremden Belegung (T0121).
///
/// `egui::Key` ist die US-Lage: die Taste `ä` kommt dort als `Key::Quote` an,
/// das Zeichen selbst nur als `Event::Text`. Solche Zeichen wertet
/// [`getippte_zeichen`] aus, statt sie über eine Tastentabelle zu suchen.
fn ist_textzeichen(label: &str) -> bool {
    !EINZELTASTEN.iter().any(|(_, l)| *l == label)
}

/// Die in diesem Bild **getippten** Zeichen ohne eigene Taste, klein
/// geschrieben und ohne Doppel (T0121).
///
/// Grundlage der Belegung für Umlaute und andere Sonderzeichen: sie kommen in
/// jeder Tastaturbelegung als `Event::Text` an, unabhängig davon, welche
/// physische Taste sie erzeugt. Gesperrte Zeichen (`fest_belegt`, z. B. `#`)
/// bleiben außen vor — sie gehören dem Programm, nicht der Belegung.
fn getippte_zeichen(ctx: &egui::Context) -> Vec<String> {
    let mut zeichen: Vec<String> = Vec::new();
    ctx.input(|i| {
        for e in &i.events {
            let egui::Event::Text(t) = e else { continue };
            for c in t.chars() {
                let z = c.to_lowercase().to_string();
                if ist_textzeichen(&z)
                    && persist::fest_belegt(&z).is_none()
                    && !zeichen.contains(&z)
                {
                    zeichen.push(z);
                }
            }
        }
    });
    zeichen
}

/// Name einer Taste in der Belegung: `"ä"` bzw. `"shift+ä"` (T0121) — dieselbe
/// Form, die `UserPrefs::bind` speichert.
fn zeichen_label(zeichen: &str, umschalt: bool) -> String {
    if umschalt {
        format!("shift+{zeichen}")
    } else {
        zeichen.to_string()
    }
}

/// Die Tasten, deren Funktion aus der **Tastaturbelegung** kommt — Buchstaben
/// und Symbole (T0056, `docs/benutzer/TASTEN.md`). Ziffern gehören der Hotbar,
/// Steuerungstasten und Kombinationen dem festen Ablauf im Code; keine dieser
/// Tasten steht deshalb in dieser Liste.
///
/// **Eine** Quelle für die Kürzel-Auswertung (`handle_shortcuts`) — sonst legt
/// man eine Taste in der Belegung um und die eine Hälfte des Programms hört
/// nicht darauf.
pub const EINZELTASTEN: &[(egui::Key, &str)] = &[
    (egui::Key::A, "a"),
    (egui::Key::B, "b"),
    (egui::Key::C, "c"),
    (egui::Key::D, "d"),
    (egui::Key::E, "e"),
    (egui::Key::F, "f"),
    (egui::Key::G, "g"),
    (egui::Key::H, "h"),
    (egui::Key::I, "i"),
    (egui::Key::J, "j"),
    (egui::Key::K, "k"),
    (egui::Key::L, "l"),
    (egui::Key::M, "m"),
    (egui::Key::N, "n"),
    (egui::Key::O, "o"),
    (egui::Key::P, "p"),
    (egui::Key::Q, "q"),
    (egui::Key::R, "r"),
    (egui::Key::S, "s"),
    (egui::Key::T, "t"),
    (egui::Key::U, "u"),
    (egui::Key::V, "v"),
    (egui::Key::W, "w"),
    (egui::Key::X, "x"),
    (egui::Key::Y, "y"),
    (egui::Key::Z, "z"),
    // Symbole mit eigener Taste: die übrigen Zeichen des Tastenbilds (`#`, `<`,
    // `´`) sind zweite Ebene einer anderen Taste und deshalb nicht belegbar
    // (`persist::FestBelegt::ZweiteEbene`).
    (egui::Key::Comma, ","),
    (egui::Key::Period, "."),
    (egui::Key::Minus, "-"),
    (egui::Key::Plus, "+"),
];

// ---------------------------------------------------------------------------
// Ansicht / Reiter
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TabKind {
    Model,
    Sheet(Id),
    /// Blockeditor: die Kennung der Blockdefinition (T0137). Ein Reiter je
    /// offenem Block; bearbeitet wird der Block als dritter Raum.
    Block(String),
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Weltkoordinate in der Bildmitte
    pub center: V2,
    /// Pixel pro Millimeter
    pub zoom: f64,
    /// Drehung der Zeichnungsansicht im Bogenmaß, mathematisch positiv
    /// (0 = Ausgangsorientierung). Gilt nur für die Anzeige: Modell,
    /// Speicherdatei und Ausgaben bleiben ungedreht.
    pub rot: f64,
}

impl Default for View {
    fn default() -> Self {
        Self {
            center: V2::new(5000.0, 4000.0),
            zoom: 0.08,
            rot: 0.0,
        }
    }
}

/// Dreht einen Vektor um `angle` (Bogenmaß, mathematisch positiv).
fn rot_vec(v: V2, angle: f64) -> V2 {
    let (sin, cos) = angle.sin_cos();
    V2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

impl View {
    /// Ist die Ansicht gegenüber der Ausgangsorientierung gedreht?
    pub fn ist_gedreht(&self) -> bool {
        self.rot.abs() > 1e-9
    }

    /// Dreht die Ansicht um `angle` um den Weltpunkt `about` und hält diesen
    /// Punkt an seiner Bildschirmposition — das ist das Drehzentrum, das der
    /// Nutzer angeklickt hat.
    pub fn rotate_about(&mut self, about: V2, angle: f64) {
        let alt = self.rot;
        self.rot = alt + angle;
        self.center = about - rot_vec(about - self.center, alt - self.rot);
    }

    /// Setzt die Ansicht auf die Ausgangsorientierung zurück. Der Weltpunkt in
    /// der Bildmitte bleibt stehen, damit die Zeichnung nicht wegspringt.
    pub fn zurueck_drehen(&mut self) {
        self.rot = 0.0;
    }

    /// Bildschirmversatz (px, y nach unten) → Weltversatz (mm). Hängt am
    /// Drehwinkel: Verschieben und Schwenken laufen immer dem Zeiger nach.
    pub fn screen_delta_to_world(&self, d: Vec2) -> V2 {
        let v = V2::new(d.x as f64 / self.zoom, -(d.y as f64) / self.zoom);
        rot_vec(v, -self.rot)
    }

    /// Aus dem gespeicherten Sitzungswert (T0130).
    pub fn von_wert(a: &sitzung::Ansicht) -> Self {
        Self {
            center: a.mitte,
            zoom: a.zoom,
            rot: a.drehung,
        }
    }

    /// Als gespeicherter Sitzungswert (T0130).
    pub fn als_wert(self) -> sitzung::Ansicht {
        sitzung::Ansicht {
            mitte: self.center,
            zoom: self.zoom,
            drehung: self.rot,
        }
    }
}

/// Kurzform für [`View::als_wert`] in `sitzung_sammeln`.
fn ansicht_als_wert(v: View) -> sitzung::Ansicht {
    v.als_wert()
}

/// Fensterlage aus den Meldungen von egui (T0141).
///
/// Die **Größe** kommt aus der Bildschirmfläche (`bild`), die jede Plattform
/// meldet (egui-winit setzt sie aus `window.inner_size()`). Die **Lage** kommt
/// nur aus den Fensterrechtecken (`inner`/`outer`, globale Koordinaten in
/// Punkten) — unter Wayland sind beide `None`, weil xdg-shell keine
/// Fensterposition kennt. Dann bleibt `x`/`y` leer und wenigstens die Größe
/// wird gemerkt; `None` heißt: gar nichts Brauchbares (z. B. Fenster minimiert).
fn lage_aus(
    inner: Option<egui::Rect>,
    outer: Option<egui::Rect>,
    bild: egui::Rect,
) -> Option<sitzung::Fensterlage> {
    // egui ohne Fensterangabe (minimiert, Testkontext) meldet 10 000 × 10 000
    // Punkte (`InputState::default`) — das ist der Platzhalter, keine Größe.
    const PLATZHALTER: f32 = 10_000.0;
    if inner.is_none() && bild.width() >= PLATZHALTER && bild.height() >= PLATZHALTER {
        return None;
    }
    let (breite, hoehe) = inner
        .map(|r| (r.width(), r.height()))
        .unwrap_or((bild.width(), bild.height()));
    // Die Außenkante ist die Fensterposition, die `with_position` erwartet;
    // die Innenkante taugt als Rückfall, weil sie ebenfalls global ist.
    let pos = outer.map(|r| r.min).or_else(|| inner.map(|r| r.min));
    let lage = sitzung::Fensterlage {
        breite,
        hoehe,
        x: pos.map(|p| p.x).unwrap_or(0.0),
        y: pos.map(|p| p.y).unwrap_or(0.0),
        mit_lage: pos.is_some(),
        maximiert: false,
        vollbild: false,
    };
    lage.brauchbar().then_some(lage)
}

/// `true`, wenn der Fenstermanager für ein Fenster ohne Dekoration keinen
/// Schatten zeichnet (T0147).
///
/// Unter **Wayland** zeichnet KWin für ein Fenster mit clientseitiger
/// Dekoration keinen Schatten — dort muss JoJa-CAD ihn selbst malen. Unter
/// **X11** tut KWin es (gemessen: derselbe Schatten mit und ohne Titelleiste);
/// dort bleibt es beim Schatten des Fenstermanagers, denn ohne Compositor
/// hätte der durchsichtige Rand keinen Grund und sähe als dunkler Rahmen aus.
/// Windows und macOS kennen kein X11 und bekommen den eigenen Schatten.
fn schatten_selbst_zeichnen() -> bool {
    let x11 = cfg!(unix)
        && std::env::var_os("DISPLAY").is_some()
        && std::env::var_os("WAYLAND_DISPLAY").is_none();
    !x11
}

/// Toleranz auf der **halben** Bildschirmlänge beim Erkennen eines angedockten
/// Fensters (T0177).
///
/// Das Fenstermanagement legt ein angedocktes Fenster an die **Arbeitsfläche**,
/// nicht an den ganzen Bildschirm: gemessen unter KWin 6.3
/// (`kwin_wayland --virtual`, 1600 × 1000) ergibt „Fenster nach links
/// andocken“ 799 × 972 — halbe Breite (1 px daneben), Höhe um die Bedienleiste
/// gekürzt. Fünf Prozent decken auch Kachel-Lücken und krumme Skalierung ab.
const SCHATTEN_DOCK_TOLERANZ: f32 = 0.05;

/// Wie viel der **anderen** Bildschirmlänge ein angedocktes Fenster füllen muss
/// (T0177).
///
/// Zehn Prozent Abstand, weil ein angedocktes Fenster an die Arbeitsfläche
/// stößt: bei 1080 Bildpunkten Höhe und einer 60 px hohen Leiste bleiben 94,4 %.
const SCHATTEN_DOCK_FAST_GANZ: f32 = 0.90;

/// Hat das Fenstermanagement das Fenster an den Bildschirmrand gelegt (T0177)?
///
/// Ohne diese Erkennung bleibt beim Andocken ein Abstand in Schattenbreite: der
/// durchsichtige Streifen für den selbst gezeichneten Schatten (T0147) gehört
/// zum Fenster, das Fenstermanagement rechnet ihn mit, und die Oberfläche ist um
/// seine Breite kleiner als die angedockte Fläche. Gemeldet wird der Zustand
/// nicht — winit kennt unter Wayland kein „angedockt“ (`xdg-toplevel` führt
/// dafür `tiled`-Zustände, egui reicht sie nicht durch), und eine Fensterlage
/// gibt es dort ebenfalls nicht. Erkannt wird er deshalb an der **Größe** im
/// Verhältnis zum Bildschirm: eine Richtung ist die halbe Bildschirmlänge (an
/// eine Seite angedockt), die andere reicht bis an die Arbeitsfläche; beides
/// zugleich ist ein Viertel. Ein von Hand genau so groß gezogenes Fenster
/// verliert den Schatten ebenfalls; das ist der Preis dafür, den Rand überhaupt
/// zu erkennen.
fn fenster_angedockt(fenster: Vec2, monitor: Option<Vec2>) -> bool {
    let Some(monitor) = monitor else {
        return false;
    };
    if monitor.x < 2.0 || monitor.y < 2.0 {
        return false;
    }
    // „Andocken“ heißt: **eine** Richtung ist die halbe Bildschirmlänge
    // (Viertel: beide), die andere reicht bis an die Arbeitsfläche. Beides
    // zusammen ist nötig, sonst sähe das Vorgabefenster auf einem großen
    // Bildschirm wie angedockt aus — 1532 px breit sind auf 3072 px genau die
    // Hälfte, aber nur 57 % der Höhe. Die volle Bildschirmgröße bleibt außen
    // vor: die meldet der Fenstermanager als „maximiert“.
    let halb = |a: f32, b: f32| (a - b * 0.5).abs() <= (b * 0.5 * SCHATTEN_DOCK_TOLERANZ).max(2.0);
    let fast_ganz = |a: f32, b: f32| a >= b * SCHATTEN_DOCK_FAST_GANZ;
    (halb(fenster.x, monitor.x) && (fast_ganz(fenster.y, monitor.y) || halb(fenster.y, monitor.y)))
        || (halb(fenster.y, monitor.y)
            && (fast_ganz(fenster.x, monitor.x) || halb(fenster.x, monitor.x)))
}

/// Wie lange der Startzustand des Fensters (Vollbild/maximiert) wiederholt
/// wird, bis der Fenstermanager ihn annimmt (T0141, Turn 2).
const FENSTER_START_GEDULD: f64 = 3.0;

/// Läuft ein Demo-/Prüfzustand (`JJCAD_DEMO_UI`)?
///
/// Dann fasst die Sitzung weder lesend noch schreibend zu: ein Prüfbild
/// (`scripts/qa-shot.py`) soll die Reiter des Nutzers nicht ersetzen und beim
/// Beenden auch nicht überschreiben (T0130).
pub fn ist_demo_start() -> bool {
    std::env::var_os("JJCAD_DEMO_UI").is_some()
}

/// Schlüssel der zwischengespeicherten Anzeigeliste.
///
/// Die Anzeigeliste hängt vom Bearbeitungsraum ab (`Document::space`): im
/// Modell stehen die Zeichnungsobjekte, im Papierraum Blattrahmen und
/// Schriftfeld. Beides sind völlig verschiedene Listen bei gleicher Revision —
/// ohne den Raum im Schlüssel wird ein Papierraum-Stand als Modell gezeichnet
/// und die Zeichnung ist unsichtbar (T0007).
#[derive(Clone, Copy, Debug)]
pub struct DrawCacheKey {
    pub rev: u64,
    /// Pixel pro Millimeter — Strichmuster und Kantenbreiten hängen daran.
    pub zoom: f64,
    /// Beschriftungsmaßstab (Papier-mm → Welt-mm)
    pub annot: f64,
    /// Bearbeitungsraum: `None` = Modell, `Some(Blatt)` = Papierraum
    pub space: Option<Id>,
    /// Läuft der Blockeditor? Die Liste hängt am Block, nicht an `space` —
    /// ohne das Kennzeichen bliebe beim Wechsel in einen Blockeditor die
    /// Zeichnung des vorigen Raums stehen.
    pub block: bool,
    /// Kurventoleranz in Bildschirm-pixeln (T0197): die Splines entstehen in
    /// dieser Auflösung, ein geänderter Wert muss die Liste neu bauen.
    pub kurven: f64,
}

/// Faktor, um den sich der Zoom ändern darf, ohne die Anzeigeliste neu zu
/// bauen (T0175).
///
/// Seit die Splines in der Auflösung der Anzeige entstehen, hängt die Liste am
/// Zoom. Ohne Hysterese baute jedes Bild beim Zoomen die Liste neu (gemessen
/// 0,3–1,3 ms bei Musterzeichnungen). Ein Viertel Spielraum reicht: die
/// Pfeilhöhe wächst dadurch höchstens auf 0,19 px (0,15 px × 1,25).
pub const ZOOM_HYSTERESE: f64 = 1.25;

/// Höchster Zoom (px je Modell-mm) im **Blockeditor** (T0231).
///
/// Ein Block ist klein: ein neutrales Punktsymbol misst eine Einheit. Mit der
/// Grenze der Zeichnung (80 px/mm) wäre es im Editor nur wenige Pixel groß und
/// nicht zu bearbeiten — der Editor zoomt deshalb bis auf Fenstergröße hinein.
pub const BLOCK_ZOOM_MAX: f64 = 4000.0;

/// Liegen zwei Zooms innerhalb der Hysterese?
fn zoom_passt(a: f64, b: f64) -> bool {
    if !(a.is_finite() && b.is_finite()) {
        return false;
    }
    let (a, b) = (a.max(1e-9), b.max(1e-9));
    let r = a / b;
    r <= ZOOM_HYSTERESE && r >= 1.0 / ZOOM_HYSTERESE
}

impl DrawCacheKey {
    /// Gleiche Zeichnung, gleiche Ansicht, gleicher Raum?
    ///
    /// Der Zoom wird mit **Hysterese** verglichen ([`ZOOM_HYSTERESE`]): die
    /// Liste entsteht in der Auflösung, in der sie gebaut wurde, und bleibt
    /// stehen, solange sich der Zoom nicht um mehr als ein Viertel ändert.
    pub fn matches(&self, other: &Self) -> bool {
        self.rev == other.rev
            && zoom_passt(self.zoom, other.zoom)
            && (self.annot - other.annot).abs() < 1e-9
            && self.space == other.space
            && self.block == other.block
            && (self.kurven - other.kurven).abs() < 1e-9
    }
}

pub struct DocTab {
    pub session: Session,
    pub tab: TabKind,
    pub view_model: View,
    pub sheet_views: HashMap<Id, View>,
    /// Ansichten der offenen Blockeditoren (T0137), je Blockkennung.
    pub block_views: HashMap<String, View>,
    /// Offene Blockeditoren in Reihenfolge des Öffnens — ihre Reiter stehen
    /// im Streifen. Geschlossen wird über das Kreuz oder „Speichern“.
    pub offene_bloecke: Vec<String>,
    /// Blätter, deren Reiter **geschlossen** wurde (T0218). Das Blatt bleibt
    /// in der Zeichnung und steht weiter im Blattmanager; ein Klick dort holt
    /// den Reiter zurück. Gehört zur Ansicht, nicht zur Datei.
    pub geschlossene_blaetter: Vec<Id>,
    /// Stand der Blockdefinition beim Öffnen: Schließen ohne Speichern stellt
    /// ihn wieder her, damit ein versehentlich angetippter Reiter nichts
    /// verändert.
    pub block_original: HashMap<String, jjcad_core::model::BlockDef>,
    pub rev: u64,
    pub cache: Option<(DrawCacheKey, DisplayList)>,
    pub hover: Option<Id>,
}

#[derive(Clone)]
pub struct Tool {
    pub action: String,
    pub pts: Vec<V2>,
    /// Pfad (T0229): Rolle jedes Klicks im Punktpuffer. Ohne Werkzeug leer.
    pub schritte: Vec<jjcad_core::pfad::PfadSchritt>,
    /// Pfad-Werkzeug: nächste Strecke als Bogen? Umschalt schaltet um.
    pub bogen: bool,
    /// Erstes angeklicktes Objekt bei Werkzeugen, die ein **Paar** brauchen
    /// (Verschneiden, T0082). Der Punktpuffer `pts` reicht dafür nicht: die
    /// Action braucht die Kennung, nicht die Klickstelle.
    pub obj: Option<Id>,
    /// Zweites Objekt desselben Paares (T0247). Abrunden und Phase kürzen
    /// **beide** Linien; `pts` trägt danach nur noch den Eckpunkt, damit der
    /// getippte Radius dieselbe Längen-Eingabe benutzt wie jedes Werkzeug.
    pub obj2: Option<Id>,
    /// Klickstelle zum ersten bzw. zweiten Objekt (T0247): sie entscheidet,
    /// welche Seite der Linie stehen bleibt. Ohne sie gilt die längere Seite.
    pub pick_a: Option<V2>,
    pub pick_b: Option<V2>,
    /// Knick einer Polylinie (T0247): der Index des Scheitelpunkts in `obj`.
    /// Ein Klick auf den Knick wählt die Ecke in **einem** Schritt; `pts` trägt
    /// dann den Scheitelpunkt als Eckpunkt.
    pub knoten: Option<usize>,
    pub selecting: bool,
    pub phase: ToolPhase,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolPhase {
    Collecting,
    TextInput,
}

/// Eine Hilfslinie, die der Nutzer während der Eingabe eines Punktes legt
/// (T0230).
///
/// Sie entsteht in **zwei Schritten**: der Rechtsklick schaltet die Eingabe ein
/// (`App::hilfslinie_modus`), der nächste Linksklick oder eine **getippte
/// Länge** setzt ihren zweiten Endpunkt. Danach steht sie wie eine normale
/// Linie in der Zeichnung — mit zwei Endpunkten und Fußpunkt-Fang
/// (`SnapKind::Tracking`) —, bis das Werkzeug endet oder der Nutzer sie mit
/// einem weiteren Rechtsklick wegnimmt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hilfslinie {
    /// Fester Endpunkt: der Bezugspunkt des Zuges beim Anlegen (bzw. der erste
    /// Klick, wenn es noch keinen gibt).
    pub a: V2,
    /// Zweiter Endpunkt — die Länge ist der Abstand der beiden.
    pub b: V2,
}

impl Hilfslinie {
    /// Die Strecke der Hilfslinie.
    pub fn strecke(&self) -> (V2, V2) {
        (self.a, self.b)
    }

    /// Länge in mm — die Zahl, die beim Eingeben und an der Linie steht.
    pub fn laenge(&self) -> f64 {
        self.a.dist(self.b)
    }
}

/// Einfügemodus für Rechteck und Ellipse.
///
/// `Achsenparallel` ist der bisherige Weg (zwei gegenüberliegende Ecken bzw.
/// Mittelpunkt und Achsen), `Basislinie` legt die Lage über eine Linie fest:
/// beim Rechteck Basislinie + Höhe, bei der Ellipse Mittelachse + Breite. Damit
/// entstehen beide Objekte auch in beliebiger Drehlage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Einfuegemodus {
    /// Achsenparallel (Rechteck über Ecken, Ellipse über Mittelpunkt/Achsen).
    Achsenparallel,
    /// Über eine Linie beliebiger Lage (Basislinie bzw. Mittelachse).
    Basislinie,
}

impl Einfuegemodus {
    pub const ALL: [Einfuegemodus; 2] = [Einfuegemodus::Achsenparallel, Einfuegemodus::Basislinie];

    /// Beschriftung im Dropdown — je Werkzeug verschieden, weil die Linie eine
    /// andere Bedeutung hat (Basislinie des Rechtecks, Mittelachse der Ellipse).
    pub fn label(self, tool_id: &str) -> &'static str {
        match (self, tool_id) {
            (Einfuegemodus::Achsenparallel, "draw.ellipse") => {
                rb("ui.einfuegemodus.mittelpunkt")
            }
            (Einfuegemodus::Achsenparallel, _) => rb("ui.einfuegemodus.ecken"),
            (Einfuegemodus::Basislinie, "draw.ellipse") => rb("ui.einfuegemodus.mittelachse"),
            (Einfuegemodus::Basislinie, _) => rb("ui.einfuegemodus.basislinie"),
        }
    }
}

/// Eingabemodus des Bogens (`draw.arc`).
///
/// `DreiPunkte` ist der bisherige Weg: Start, Ende und ein Punkt auf dem Bogen
/// legen den Kreis fest. `Mittelpunkt` fragt Mittelpunkt, Startpunkt und
/// Endpunkt ab — der Startpunkt bestimmt zugleich den Radius, der Bogen läuft
/// von dort im mathematisch positiven Drehsinn zum Endpunkt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bogenmodus {
    /// Start, Ende, Punkt auf dem Bogen.
    DreiPunkte,
    /// Mittelpunkt, Startpunkt (Radius), Endpunkt.
    Mittelpunkt,
}

impl Bogenmodus {
    pub const ALL: [Bogenmodus; 2] = [Bogenmodus::DreiPunkte, Bogenmodus::Mittelpunkt];

    /// Beschriftung im Dropdown und im Ribbon-Menü.
    pub fn label(self) -> &'static str {
        match self {
            Bogenmodus::DreiPunkte => rb("ui.bogenmodus.drei_punkte"),
            Bogenmodus::Mittelpunkt => rb("ui.bogenmodus.mittelpunkt_start_ende"),
        }
    }
}

/// Eingabemodus einer neuen Maßkette (T0154).
///
/// Früher lagen Einzelmaß und Maßkette als zwei Werkzeuge nebeneinander. Jetzt
/// gibt es **ein** Werkzeug mit drei Wegen:
///
/// * `Waagerecht` / `Senkrecht` — zuerst die Lage der Maßlinie, danach
///   beliebig viele Messpunkte (mindestens zwei).
/// * `Direkt` — die Messpunkte bestimmen die Richtung (auch schräg); ein
///   weiterer Klick legt die Maßlinie parallel dazu fest, weitere Punkte
///   hängen sich an.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DimModus {
    Waagerecht,
    Senkrecht,
    Direkt,
}

impl DimModus {
    pub const ALL: [DimModus; 3] = [DimModus::Waagerecht, DimModus::Senkrecht, DimModus::Direkt];

    /// Messachse dieses Modus — dieselbe Angabe, die das Eigenschaften-Panel
    /// unter „Richtung“ zeigt.
    pub fn axis(self) -> DimAxis {
        match self {
            DimModus::Waagerecht => DimAxis::X,
            DimModus::Senkrecht => DimAxis::Y,
            DimModus::Direkt => DimAxis::Aligned,
        }
    }

    /// Modus zu einem gespeicherten Winkel — die Umkehrung von
    /// [`DimModus::axis`] für die Schnellauswahl (T0166). Eine schräg gedrehte
    /// Kette entsteht wieder im Modus „Direkt“.
    pub fn aus_winkel(angle: f64) -> Self {
        match DimAxis::aus_winkel(angle) {
            DimAxis::X => DimModus::Waagerecht,
            DimAxis::Y => DimModus::Senkrecht,
            DimAxis::Aligned => DimModus::Direkt,
        }
    }

    /// Werkzeug, das diesen Modus eingibt — je Modus eine eigene
    /// Schaltfläche mit eigenem Icon (T0154, Turn 3).
    pub fn werkzeug(self) -> &'static str {
        match self {
            DimModus::Waagerecht => "dim.chain.h",
            DimModus::Senkrecht => "dim.chain.v",
            DimModus::Direkt => "dim.chain.d",
        }
    }

    /// Modus eines Werkzeugs — `None` für alles, was keine Maßkette ist.
    pub fn aus_werkzeug(id: &str) -> Option<Self> {
        match id {
            "dim.chain.h" => Some(DimModus::Waagerecht),
            "dim.chain.v" => Some(DimModus::Senkrecht),
            "dim.chain.d" => Some(DimModus::Direkt),
            _ => None,
        }
    }

    /// Der um 90° gedrehte Modus (T0250): waagerecht ↔ senkrecht.
    ///
    /// „Direkt“ hat keine feste Richtung — dort gibt es nichts zu drehen.
    pub fn gedreht(self) -> Option<Self> {
        match self {
            DimModus::Waagerecht => Some(DimModus::Senkrecht),
            DimModus::Senkrecht => Some(DimModus::Waagerecht),
            DimModus::Direkt => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DimModus::Waagerecht => rb("ui.dimmode.waagerecht"),
            DimModus::Senkrecht => rb("ui.dimmode.senkrecht"),
            DimModus::Direkt => rb("ui.dimmode.direkt"),
        }
    }
}

#[derive(Clone)]
pub struct ToolOptions {
    pub wall_thickness: f64,
    pub wall_align: jjcad_core::style::WallAlign,
    pub wall_hatch: Option<jjcad_core::style::FillPattern>,
    pub wall_height: f64,
    pub door_width: f64,
    pub door_kind: DoorKind,
    pub hinge: f64,
    pub swing_in: bool,
    /// Lage des Basispunktes einer neuen Tür (T0091): Links/Mitte/Rechts.
    pub door_base: DoorBase,
    pub window_width: f64,
    pub window_sill: f64,
    /// Fensterhöhe in mm — gehört wie Breite und Brüstung zu den Eigenschaften,
    /// die beim Erzeugen schon feststehen (gleiche Zeile wie beim Bearbeiten).
    pub window_height: f64,
    /// Höhe des nächsten Volumens in mm (Körperhöhe, nicht Geschosshöhe).
    pub volume_height: f64,
    /// Muster des nächsten Volumens — eigene Werte, damit ein Körper nicht die
    /// Schraffur der Flächenwerkzeuge erbt (Standard: volle Fläche).
    pub volume_pattern: FillPattern,
    pub volume_angle: f64,
    pub volume_scale: f64,
    /// Lage des Fensters in der Wanddicke (Links/Mittig/Rechts).
    pub window_align: jjcad_core::style::WallAlign,
    pub stair_width: f64,
    pub stair_steps: u32,
    /// Treppenhöhe (Geschosshöhe) in mm — Grundlage der Schrittmaßregel.
    pub stair_height: f64,
    /// Stufenzahl aus Länge und Höhe bestimmen (statt fester Zahl).
    pub stair_auto: bool,
    /// Treppenschnitt (zwei Läufe übereinander) für die nächste Treppe.
    pub stair_cut: bool,
    pub text_height: f64,
    /// Drehung des nächsten Textes in Grad (0 = waagerecht). Gilt für das
    /// erzeugte Objekt; das Eingabefeld in der Zeichenfläche bleibt dabei
    /// bildschirmparallel — wie in Textprogrammen üblich.
    pub text_angle: f64,
    /// Ausrichtung des nächsten Textfelds (waagerecht: Linksbündig/Zentriert/
    /// Rechtsbündig, senkrecht: Oben/Mitte/Unten). Der Klickpunkt ist der
    /// Einfügepunkt dieser Ausrichtung.
    pub text_align: TextAlignH,
    pub text_valign: TextAlignV,
    /// Farbe des nächsten Textes (None = Layerfarbe)
    pub text_color: Option<Color>,
    /// Auszeichnung des nächsten Textes
    pub text_bold: bool,
    pub text_italic: bool,
    pub text_underline: bool,
    pub text_superscript: bool,
    /// Schriftart des nächsten Textes (bleibt für das nächste Element stehen)
    pub text_font: TextFont,
    /// Zeilenabstand des nächsten Textes als Vielfaches der Zeilenhöhe der
    /// Schrift (1 = Zeilenhöhe der Schrift selbst).
    pub text_line_spacing: f64,
    /// Umrandung des nächsten Textes dauerhaft anzeigen (T0126).
    pub text_frame: bool,
    /// Runde Umrandung: Ecken mit dem Radius der halben Schrifthöhe.
    pub text_frame_round: bool,
    /// Hintergrundfarbe des nächsten Textes (None = keine Fläche).
    pub text_background: Option<Color>,
    /// Textgröße des nächsten Raumstempels in mm.
    pub room_height: f64,
    /// Überschrift über dem Flächeninhalt des nächsten Raumstempels.
    pub room_heading: String,
    pub dim_axis: Option<DimAxis>,
    /// Eigener Maßtext des nächsten Maßes (leer = automatisch berechnet).
    pub dim_text: String,
    /// Verlängerung zu den Messpunkten beim nächsten Maß (T0226) — Vorgabe
    /// aus, wie die Vorgabe des Objekts.
    pub dim_ext: bool,
    /// Punktsymbol an den Messpunkten des nächsten Maßes (T0227) — Block-ID aus
    /// der Kategorie „Punkt Symbole“, leer = kein Symbol. Vorgabe ist der
    /// Schrägstrich (`model::DIM_SYMBOL_VORGABE`).
    pub dim_symbol: String,
    pub offset_dist: f64,
    /// Gemerkter Abstand des Werkzeugs „Parallele" (T0200).
    ///
    /// `None` = noch nicht gesetzt: der erste Klick nimmt den Abstand vom
    /// Mauszeiger. Danach bleibt der Wert stehen, solange das Werkzeug läuft —
    /// so entstehen weitere Parallelen im selben Abstand (`App::start_tool`
    /// setzt ihn beim Start zurück).
    pub parallel_dist: Option<f64>,
    pub elevation_z: f64,
    /// Lage des Höhenwertes der nächsten Grundriss-Kote (T0196).
    pub elevation_text_pos: jjcad_core::model::ElevationTextPos,
    pub fill_pattern: FillPattern,
    /// Muster des Farbeimers. Eigener Wert, damit der Eimer mit Vollfläche
    /// startet (wie ein Farbeimer im Malprogramm), ohne dass die anderen
    /// Füll-Werkzeuge ihr Muster wechseln.
    pub paint_pattern: FillPattern,
    /// Musterabstand in mm
    pub fill_scale: f64,
    /// Drehung des Musters (Bogenmaß, 0 = waagerecht)
    pub fill_angle: f64,
    /// Umrandung der Füllfläche beim **Punkte setzen** (`draw.fill.points`,
    /// `draw.fill.boundary`) — hier gibt es keine vorhandene Kante, deshalb an.
    pub fill_border_points: bool,
    /// Umrandung beim **Kontur anklicken** (`draw.fill`): die Umgrenzung liegt
    /// schon auf einer Linie oder Wand, deshalb aus.
    pub fill_border_outline: bool,
    /// Umrandung beim **Farbeimer** (`draw.fill.paint`): die Grenze ist die
    /// Wand, eine zweite Kante würde sie dicker machen — deshalb aus.
    pub fill_border_paint: bool,
    pub block_id: Option<String>,
    pub block_rot: f64,
    pub block_scale: f64,
    /// Einfügemodus des Rechtecks (`draw.rect`)
    pub rect_mode: Einfuegemodus,
    /// Einfügemodus der Ellipse (`draw.ellipse`)
    pub ellipse_mode: Einfuegemodus,
    /// Eingabemodus des Bogens (`draw.arc`)
    pub arc_mode: Bogenmodus,
    /// Das Bild, das `draw.image` als Nächstes einsetzt. Die Datei wird beim
    /// Auswählen gelesen und als Base64 mitgeführt — die Action schickt sie
    /// unverändert ins Dokument (`EntityKind::Image`).
    pub bild: Option<BildQuelle>,
    /// Seitenverhältnis des nächsten Bildes sperren (Vorgabe an). Gesperrt
    /// folgt die Höhe der Breite und dem Verhältnis der Datei.
    pub bild_lock: bool,
    /// Punktsymbol am Anfang des nächsten Punktes bzw. der nächsten Linie
    /// (T0210) — Block-ID aus der Kategorie „Punkt Symbole“.
    pub symbol_start: Option<String>,
    /// Punktsymbol am Ende der nächsten Linie (T0210).
    pub symbol_end: Option<String>,
}

/// Ausgewählte Bilddatei für `draw.image` (Anzeigename und Base64-Daten).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BildQuelle {
    pub name: String,
    pub data: String,
}

impl ToolOptions {
    /// Umrandung des angegebenen Erstellmodus — der Wert, an den das
    /// Eigenschaften-Panel seine Checkbox bindet. Eine Stelle für die
    /// Zuordnung Modus → Feld, damit Panel, Vorschau und Erzeugung nicht
    /// auseinanderlaufen.
    pub fn fill_border_for(&self, tool_id: &str) -> bool {
        match tool_id {
            "draw.fill" | "draw.hatch" => self.fill_border_outline,
            "draw.fill.paint" => self.fill_border_paint,
            _ => self.fill_border_points,
        }
    }

    /// Wie [`ToolOptions::fill_border_for`], aber schreibbar (Checkbox).
    pub fn fill_border_for_mut(&mut self, tool_id: &str) -> &mut bool {
        match tool_id {
            "draw.fill" | "draw.hatch" => &mut self.fill_border_outline,
            "draw.fill.paint" => &mut self.fill_border_paint,
            _ => &mut self.fill_border_points,
        }
    }

    pub fn from_doc(doc: &Document) -> Self {
        Self {
            wall_thickness: doc.settings.wall_thickness,
            wall_align: jjcad_core::style::WallAlign::Center,
            wall_hatch: None,
            wall_height: 2750.0,
            door_width: doc.settings.door_width,
            door_kind: DoorKind::Hinge,
            hinge: 1.0,
            swing_in: true,
            door_base: DoorBase::Left,
            window_width: doc.settings.window_width,
            window_sill: 900.0,
            window_height: 1250.0,
            volume_height: 2750.0,
            volume_pattern: FillPattern::Solid,
            volume_angle: 0.0,
            volume_scale: 100.0,
            window_align: jjcad_core::style::WallAlign::Center,
            stair_width: doc.settings.stair_width,
            stair_steps: 17,
            stair_height: jjcad_core::stair::HOEHE_VORGABE_MM,
            stair_auto: true,
            stair_cut: true,
            text_height: doc.settings.text_height,
            text_angle: 0.0,
            text_align: TextAlignH::Left,
            text_valign: TextAlignV::Middle,
            text_color: None,
            text_bold: false,
            text_italic: false,
            text_underline: false,
            text_superscript: false,
            text_font: TextFont::default(),
            text_line_spacing: 1.0,
            text_frame: false,
            text_frame_round: false,
            text_background: None,
            room_height: doc.settings.text_height,
            room_heading: "Zimmer".to_string(),
            dim_axis: None,
            dim_text: String::new(),
            dim_ext: false,
            dim_symbol: jjcad_core::model::DIM_SYMBOL_VORGABE.to_string(),
            offset_dist: doc.settings.wall_thickness,
            parallel_dist: None,
            elevation_z: 0.0,
            elevation_text_pos: jjcad_core::model::ElevationTextPos::default(),
            fill_pattern: FillPattern::Lines,
            paint_pattern: FillPattern::Solid,
            fill_scale: 100.0,
            fill_angle: std::f64::consts::FRAC_PI_4,
            fill_border_points: true,
            fill_border_outline: false,
            fill_border_paint: false,
            block_id: None,
            block_rot: 0.0,
            block_scale: 1.0,
            rect_mode: Einfuegemodus::Achsenparallel,
            ellipse_mode: Einfuegemodus::Achsenparallel,
            arc_mode: Bogenmodus::DreiPunkte,
            bild: None,
            bild_lock: true,
            symbol_start: None,
            symbol_end: None,
        }
    }

    /// Einfügemodus des laufenden Werkzeugs (Rechteck bzw. Ellipse).
    pub fn einfuegemodus(&self, tool_id: &str) -> Einfuegemodus {
        match tool_id {
            "draw.ellipse" => self.ellipse_mode,
            _ => self.rect_mode,
        }
    }

    /// Wie [`ToolOptions::einfuegemodus`], aber schreibbar (Dropdown).
    pub fn einfuegemodus_mut(&mut self, tool_id: &str) -> &mut Einfuegemodus {
        match tool_id {
            "draw.ellipse" => &mut self.ellipse_mode,
            _ => &mut self.rect_mode,
        }
    }
}

/// Werkzeuge der Ribbon-Gruppe „Bearbeiten" im Reiter „Ändern".
///
/// „Auswählen" steht bewusst nicht in der Liste: Klicken auf der Zeichenfläche
/// wählt ohnehin aus, der Knopf war ohne Funktion (t_2b62367d). Der Befehl
/// `modify.select` bleibt für Tastatur, Inventar und MCP erhalten.
/// Werkzeuge des Reiters „Ändern" — nur Id und Symbol; die Beschriftung kommt
/// beim Zeichnen aus dem Sprachkatalog (T0103), damit sie der Sprache folgt.
pub const RIBBON_BEARBEITEN: [(&str, &str); 5] = [
    ("modify.move", "move"),
    // Kopieren ist eine geteilte Schaltfläche (siehe `copy_group_button`)
    ("modify.copy", "copy-base"),
    ("modify.rotate", "rotate"),
    ("modify.mirror", "mirror"),
    ("modify.scale", "scale"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RibbonTab {
    Datei,
    Zeichnen,
    Architektur,
    Aendern,
    Bemassung,
    Bloecke,
    Blatt,
    Ansicht,
}

impl RibbonTab {
    pub const ALL: [RibbonTab; 8] = [
        RibbonTab::Datei,
        RibbonTab::Zeichnen,
        RibbonTab::Architektur,
        RibbonTab::Aendern,
        RibbonTab::Bemassung,
        RibbonTab::Bloecke,
        RibbonTab::Blatt,
        RibbonTab::Ansicht,
    ];
    /// Katalogschlüssel der Reiterbeschriftung (T0103).
    pub fn key(self) -> &'static str {
        match self {
            RibbonTab::Datei => "ribbon.tab.start",
            RibbonTab::Zeichnen => "ribbon.tab.zeichnen",
            RibbonTab::Architektur => "ribbon.tab.architektur",
            RibbonTab::Aendern => "ribbon.tab.aendern",
            RibbonTab::Bemassung => "ribbon.tab.bemassung",
            RibbonTab::Bloecke => "ribbon.tab.bloecke",
            RibbonTab::Blatt => "ribbon.tab.blatt",
            RibbonTab::Ansicht => "ribbon.tab.ansicht",
        }
    }

    pub fn label(self) -> &'static str {
        jjcad_core::i18n::tr(self.key())
    }
    pub fn group(self) -> ActionGroup {
        match self {
            RibbonTab::Datei => ActionGroup::Datei,
            RibbonTab::Zeichnen => ActionGroup::Zeichnen,
            RibbonTab::Architektur => ActionGroup::Architektur,
            RibbonTab::Aendern => ActionGroup::Aendern,
            RibbonTab::Bemassung => ActionGroup::Bemassung,
            RibbonTab::Bloecke => ActionGroup::Bloecke,
            RibbonTab::Blatt => ActionGroup::Blatt,
            RibbonTab::Ansicht => ActionGroup::Ansicht,
        }
    }
}

/// Farbe/Stärke/Typ für das nächste zu zeichnende Element (bleibt erhalten).
#[derive(Clone)]
pub struct PendingStyle {
    pub color: Option<jjcad_core::style::Color>,
    pub fill: Option<jjcad_core::style::Color>,
    /// Füllung aus (T0242): das nächste Objekt bekommt keine Füllfläche.
    /// Geht als `fill: "keine"` an die Erzeuger-Action.
    pub no_fill: bool,
    pub lineweight: Option<f64>,
    pub line_type: Option<jjcad_core::style::LineType>,
    /// Dämmung (Linienart „Dämmung"): Bandhöhe H in mm
    pub loop_width: Option<f64>,
    /// Dämmung: Schlaufenform r
    pub loop_shape: Option<f64>,
    /// Ziel-Ebene für das nächste Element (T0202): ausdrücklich im
    /// Eigenschaften-Panel gewählt. `None` heißt „die Ebene des Typs“
    /// (`App::werkzeug_ziel_ebene`).
    pub layer: Option<Id>,
}

impl Default for PendingStyle {
    fn default() -> Self {
        // T0125: **keine** feste Stärke. Ein neues Objekt erbt die Breite
        // seiner Ebene — und die kommt bei einer neuen Zeichnung aus den
        // Voreinstellungen (0,18 mm). Der frühere Wert 0,35 hier hat jede
        // Einstellung überschrieben: `canvas::run` reicht `pending.lineweight`
        // an jede Erzeuger-Action weiter, sobald es gesetzt ist.
        Self {
            color: None,
            fill: None,
            no_fill: false,
            lineweight: None,
            line_type: None,
            loop_width: None,
            loop_shape: None,
            layer: None,
        }
    }
}

/// Seitenwahl beim PDF-Einfügen (`file.import.pdf`).
#[derive(Clone, Debug)]
pub struct PdfImportWahl {
    pub pfad: PathBuf,
    /// Seiten im PDF (1-basiert gezählt)
    pub seiten: usize,
    pub seite: usize,
    /// Als ein Element (Block) einfügen statt als Einzelobjekte
    pub als_block: bool,
}

/// T0204: Eigenschaften eines Blocks — Name und Gruppe, bearbeitet im
/// Fenster „Block-Eigenschaften“ (Rechtsklick in der Blockbibliothek).
///
/// Die Gruppe ist die Überschrift, unter der der Block in der Bibliothek
/// steht; eine neue Gruppe entsteht, indem ein Block sie bekommt.
#[derive(Clone, Debug)]
pub struct BlockEigenschaften {
    /// Kennung des Blocks (`usr:…`, `prj:…`) — die Kennung bleibt beim
    /// Umbenennen gleich.
    pub block: String,
    /// Eingabepuffer Name
    pub name: String,
    /// Gewählte Gruppe
    pub gruppe: String,
    /// Eingabe einer neuen Gruppe (leer = keine angelegt)
    pub neue_gruppe: String,
    /// Größe der Blockgeometrie (T0231): feste Größe (1:1 in der Zeichnung)
    /// oder Papiergröße (so groß wie gedruckt, wächst mit dem Maßstab).
    pub groesse: jjcad_core::model::BlockGroesse,
}

/// R2.3: Die Zieldatei wurde außerhalb von JoJa-CAD geändert, während im
/// Sicherungsordner ungespeicherte Arbeit liegt. Der Benutzer entscheidet,
/// welche Fassung weiterverwendet wird.
#[derive(Clone, Debug)]
pub struct Konflikt {
    /// Reiter, den die Entscheidung betrifft.
    pub reiter: usize,
    /// Dokumentkennung (Schlüssel des Standes im Pool).
    pub datei: String,
    /// Zieldatei auf der Platte.
    pub pfad: String,
    /// Zyklus und Zeit des Standes mit der ungespeicherten Arbeit.
    pub zyklus: u64,
    pub zeit: String,
    /// Anzeigename der Zeichnung.
    pub name: String,
    /// Im Bild, in dem der Konflikt entsteht, darf kein Knopf den Klick
    /// annehmen — sonst entscheidet der Klick, der den Start auslöst, mit.
    pub neu: bool,
}


/// T0223: Zustand des Fensters „Wiederherstellen“ — gewählte Zeichnung,
/// gewählter Stand und die einmal je Auswahl geladene Vorschau.
#[derive(Default)]
pub struct Wiederherstellen {
    /// Kennung der gewählten Zeichnung (Dokumentkennung im Pool).
    pub datei: Option<String>,
    /// Zyklus des gewählten Standes.
    pub zyklus: Option<u64>,
    /// Geladener Stand (Zyklus, Inhalt) — erspart das Lesen je Bild.
    pub vorschau: Option<(u64, jjcad_core::model::Document)>,
    /// Grund, warum der Stand nicht lesbar war.
    pub fehler: Option<String>,
    /// Flächen der Zeichnungsliste im letzten Bild (Prüfweg).
    pub last_liste: Vec<(String, egui::Rect)>,
    /// Flächen der Punkte im Diagramm im letzten Bild (Prüfweg).
    pub last_punkte: Vec<(u64, egui::Rect)>,
    /// Fläche der Vorschau im letzten Bild (Prüfweg).
    pub last_vorschau: Option<egui::Rect>,
}

/// T0235: laufende Wahl des Export-Bereichs für das PNG. Der erste Klick
/// setzt die eine Ecke der Diagonalen, der zweite die andere.
pub struct PngWahl {
    pub erster: Option<jjcad_core::geom::Vec2>,
}

/// T0235: Einstellungen des einen PNG-Exports im Fenster „PNG exportieren".
///
/// Die **Breite** in Pixeln gibt der Nutzer vor; die Höhe folgt aus dem
/// Seitenverhältnis des gewählten Bereichs (`raster::png_verhaeltnis`).
#[derive(Clone, Copy, Debug)]
pub struct PngExport {
    pub breite_px: u32,
    /// Weißer Grund; aus = das PNG behält einen durchsichtigen Grund.
    pub papier: bool,
}

pub struct UiState {
    pub pending: PendingStyle,
    pub ribbon: RibbonTab,
    pub inventory_open: bool,
    pub inventory_tab: usize,
    pub layers_open: bool,
    pub props_open: bool,
    /// Blattmanager (T0218): eigene Liste **aller** Blätter, rechts angedockt.
    pub blatt_open: bool,
    /// Panels und Ribbon eingeklappt
    pub layers_collapsed: bool,
    pub props_collapsed: bool,
    /// Blattmanager auf die Leiste eingeklappt (T0218) — wie Layer und
    /// Eigenschaften; das Symbol in der Leiste klappt ihn wieder aus.
    pub blatt_collapsed: bool,
    /// T0203: Seite, auf der das jeweilige Panel liegt. Beide Panels dürfen
    /// auf beiden Seiten stehen; die Leisten tragen stets dieselben Icons.
    pub layers_side: PanelSeite,
    pub props_side: PanelSeite,
    pub blatt_side: PanelSeite,
    /// T0222: Reihenfolge der **ausgeklappten** Panels, hinten das zuletzt
    /// benutzte. Höchstens zwei dürfen je Seite ausgeklappt sein; bei einem
    /// dritten gibt das vorderste (am längsten unbenutzte) auf.
    pub panel_benutzt: Vec<SeitenPanel>,
    /// T0233: Stand der drei Panels im vorigen Bild. Ribbon-Schalter,
    /// Objektmenü und Demo-Zustände schreiben `*_open` direkt; der Übergang
    /// zu → offen wird daran erkannt und zählt als Benutzung.
    pub panel_offen_vorher: [bool; 3],
    /// T0203: Lage der Symbole in den Seitenleisten im zuletzt gezeichneten
    /// Bild — (Panel, Seite, Fläche). Prüfwert der Tests.
    pub last_leisten_icons: Vec<(SeitenPanel, PanelSeite, Rect)>,
    /// T0212: Lage der beiden Knöpfe im Kopf des Layer-Panels im zuletzt
    /// gezeichneten Bild, von links nach rechts. Prüfwert der Tests: der Kopf
    /// trägt nur „Neuer Layer“ und „Einklappen“.
    pub last_layer_kopf: Vec<Rect>,
    /// T0218: Lage der Blattzeilen im Blattmanager — (Blatt, Namensfläche,
    /// Häkchenfläche) im zuletzt gezeichneten Bild. Prüfwert der Tests.
    pub last_blattzeilen: Vec<(Id, Rect, Rect)>,
    pub ribbon_collapsed: bool,
    pub keymap_open: bool,
    pub settings_open: bool,
    /// Gewählte Kategorie im Einstellungsfenster (Index in
    /// `dialogs::EINSTELLUNGSKATEGORIEN`)
    pub settings_kat: usize,
    /// Suchtext über der Kategorienliste im Einstellungsfenster
    pub settings_suche: String,
    /// T0127: Eingabepuffer der drei Speicherorte, indexgleich zu
    /// `persist::SpeicherortFeld::ALL`.
    pub speicherorte: [String; 3],
    /// Sind die Puffer aus den Einstellungen gefüllt? Wird beim Schließen des
    /// Einstellungsfensters zurückgesetzt.
    pub speicherorte_geladen: bool,
    /// T0162: Eingabepuffer der Vorlage für neue Dateien.
    pub vorlage: String,
    /// Lage von Suchfeld, Kategorienliste und Einstellungsseite im zuletzt
    /// gezeichneten Einstellungsfenster (Regressionstest T0061)
    pub last_settings_suche: Option<Rect>,
    pub last_settings_nav: Option<Rect>,
    pub last_settings_pane: Option<Rect>,
    /// Fläche des ganzen Einstellungsfensters im zuletzt gezeichneten Bild.
    /// T0135: Die Größe muss in allen Kategorien dieselbe sein.
    pub last_settings_fenster: Option<Rect>,
    pub help_open: bool,
    pub about_open: bool,
    pub sheet_dialog: bool,
    pub project_dialog: bool,
    /// Laufende Texteingabe in der Zeichnung (T0019)
    pub texteingabe: Option<crate::textedit::Texteingabe>,
    pub block_name_dialog: Option<(String, bool)>,
    /// T0204: Fenster „Block-Eigenschaften" (Name und Gruppe eines Blocks)
    pub block_eigenschaften: Option<BlockEigenschaften>,
    /// Hotbar-Vorlage: (None = neu, Some(index) = umbenennen), Name
    pub preset_dialog: Option<(Option<usize>, String)>,
    /// Fenster „Hotbar-Vorlagen" offen (Auswahl der Leiste)
    pub hotbar_picker_open: bool,
    pub file_dialog: Option<FileDialog>,
    /// Laufender Systemdialog (Betriebssystem) samt dem Modus, für den er
    /// geöffnet wurde
    pub systemdialog: Option<(FileMode, Systemdialog)>,
    /// Seite für „PDF einfügen" bereits gewählt
    pub pdf_import: Option<PdfImportWahl>,
    /// T0235: gemerkter Bereich für den PNG-Export (Welt-mm als x0, y0, x1, y1);
    /// `None` = ganze Zeichnung. Er gilt für den **nächsten** Export und
    /// bleibt danach stehen.
    pub png_bereich: Option<[f64; 4]>,
    /// T0235: laufende Wahl der Diagonalen für den PNG-Bereich.
    pub png_wahl: Option<PngWahl>,
    /// T0235: Fenster „PNG exportieren" offen.
    pub png_export_offen: bool,
    /// T0235: Einstellungen des PNG-Exports (Breite, Hintergrund).
    pub png_einstellungen: PngExport,
    /// Schwebende Dateien für den Prüfzustand `dateidrop` (T0115): ein echter
    /// Drag an Drop lässt sich headless nicht erzeugen.
    pub drop_demo: Option<Vec<PathBuf>>,
    /// Auszeichnung, die für das **nächste getippte Zeichen** gilt (Umschalter
    /// ohne Markierung im Textfeld), samt dem Text, zu dem sie gehört.
    pub text_format_next: Option<(Id, jjcad_core::text::RunStyle)>,
    pub show_grid: bool,
    pub lineweights: bool,
    /// Vollbild gewünscht (maßgeblich ist der Fensterzustand, siehe
    /// `App::is_fullscreen`)
    pub fullscreen: bool,
    /// Linienstärken über den Maßstab skalieren (gedruckte Breite)
    pub lineweights_scaled: bool,
    /// Faktor der Linienstärke im Modus „Bildschirm (konstant)“ (T0139);
    /// 1,0 = Vorgabe. Im Modus „Papier (maßstabsgetreu)“ ohne Wirkung.
    pub lineweight_factor: f32,
    /// Zieh-Modus für Objekte und Griffe
    pub drag_mode: DragMode,
    /// Zieh-Modus des Auswahlfensters (T0145) — eigene Einstellung, damit
    /// „Zwei Klicks“ nur das Rechteck betrifft und nicht die Griffe.
    pub select_mode: DragMode,
    /// Zuletzt benutzter Eingabemodus je Button (Fläche / Schraffur)
    pub fill_input: FillInput,
    pub hatch_input: FillInput,
    pub volume_input: FillInput,
    /// Zuletzt benutzte Kopierfunktion (Zwischenablage / mit Basispunkt)
    pub copy_input: CopyInput,
    /// Welcher der beiden Knöpfe das laufende Füllwerkzeug gestartet hat
    /// (None = kein Füllwerkzeug aktiv). Nötig, weil beide Knöpfe dieselbe
    /// Action starten können und sonst beide hervorgehoben wären.
    pub fill_active: Option<FillKind>,
    /// Offenes Kontextmenü (Objekt, Zeichenfläche oder Chip)
    pub menu: Option<MenuState>,
    /// Auswahl eines Referenzobjekts für "Vor/Hinter Element"
    /// (Some(true) = davor, Some(false) = dahinter)
    pub order_pick: Option<bool>,
    pub bind_slot: Option<usize>,
    /// Taste, die auf eine Funktion wartet (T0072): Wie ein leerer Hotbar-Slot
    /// öffnet ein Klick auf eine freie Taste das Inventar im Reiter
    /// „Funktionen"; die gewählte Funktion landet auf dieser Taste.
    pub bind_key: Option<String>,
    pub slot_drag: Option<(usize, String)>,
    pub keymap_selected: Option<String>,
    /// Name für „Layout speichern …" (T0076) — `Some` = das Eingabefenster steht.
    pub keymap_name: Option<String>,
    /// Ziel-Layout, auf das gewechselt werden soll, während die aktuelle
    /// Belegung ungespeichert ist (T0076): Solange das Feld steht, fragt die
    /// Rückfrage „Abbrechen / Verwerfen / Speichern".
    pub keymap_wechsel: Option<usize>,
    /// Lage des zuletzt gezogenen Tastaturfensters (T0076). `None` = zentriert;
    /// die Kopfleiste ist die Ziehfläche, weil das Fenster keine Titelleiste hat.
    pub keymap_pos: Option<egui::Pos2>,
    /// Fläche des zuletzt gezeichneten Tastaturfensters und Lage des
    /// Informationssymbols darin (Regressionstest T0076: Breite = Tastenbild,
    /// „i" in der oberen Leiste links).
    pub last_keymap_rect: Option<Rect>,
    pub last_keymap_info: Option<Rect>,
    /// Flächen der Bedienelemente der Layoutzeile (Regressionstest T0076:
    /// Auswahlfeld und Knöpfe haben dieselbe Höhe).
    pub last_keymap_zeile: Option<Vec<(String, Rect)>>,
    /// T0215: Die **Ebene**, die das Tastenbild zeigt und bearbeitet. Die drei
    /// Steuerungstasten darüber wirken wie Schalter: Umschalt an → auf jeder
    /// Taste steht die Umschalt-Belegung (`shift+c`), Strg an → `ctrl+c`.
    /// Unabhängig von der Tastatur, damit sich eine Ebene in Ruhe einrichten
    /// lässt.
    pub kombi_ansicht: jjcad_core::persist::Kombination,
    /// T0215: Das kleine Feld neben der Hotbar, das die gehaltene Ebene zeigt
    /// (leer bei der Grundbelegung).
    pub last_kombi_icon: Option<Rect>,
    /// T0215: Die drei Ebenen-Schalter über dem Tastenbild samt ihren Flächen
    /// (Reihenfolge: Umschalt, Strg, Alt).
    pub last_kombi_schalter: Vec<(jjcad_core::persist::Kombination, Rect)>,
    /// T0215: Die Ebene, die die Hotbar im letzten Bild gezeigt hat. Ein Klick
    /// auf einen Slot belegt **diese** Variante — auch dort, wo kein
    /// Tastaturzustand zur Hand ist (`App::bind_slot_action`).
    pub hotbar_kombi: jjcad_core::persist::Kombination,
    /// Zuletzt gewählte Funktion für die Tastaturbelegung (T0064): Solange das
    /// Fenster offen ist, merkt es sich, was im Ribbon (oder über Hotbar und
    /// Taste) gestartet wurde — ein Klick auf eine leere Taste legt sie dort
    /// ab, genau wie die Hotbar es macht. Beim Schließen wird der Merker
    /// geleert, damit keine alte Funktion unbemerkt auf einer Taste landet.
    pub keymap_funktion: Option<String>,
    pub search: String,
    /// Neuer Layer: Name im Eingabefeld; `new_layer_plan` ist der Plan, in
    /// dem er entstehen soll (T0202).
    pub new_layer_dialog: Option<String>,
    pub new_layer_plan: Option<Id>,
    /// Neuer Plan: Name im Eingabefeld (T0202).
    pub new_plan_dialog: Option<String>,
    pub color_popup: Option<Id>,
    /// T0213: Rückfrage beim Löschen eines Layers, der noch Objekte enthält —
    /// (zu löschender Layer, gewähltes Ziel).
    pub layer_delete: Option<(Id, Id)>,
    /// T0219: Rückfrage beim Löschen eines Blocks — Kennung des Blocks.
    /// Steht Instanzen in der Zeichnung, bietet der Dialog an, sie vorher zu
    /// zerlegen; erst dann lässt sich der Block löschen.
    pub block_delete: Option<String>,
    pub rename_layer: Option<(Id, String)>,
    /// Wurde der Fokus im Umbenennen-Feld schon gesetzt? (T0206)
    pub rename_layer_fokus: bool,
    /// Umbenennen eines Plans (T0202).
    pub rename_plan: Option<(Id, String)>,
    /// Zwischenablage: Elemente **und** Basispunkt (siehe `ghost::Clipboard`)
    pub clipboard: crate::ghost::Clipboard,
    pub clipboard_offset: f64,
    /// System-Zwischenablage für **Bilder** (T0112) — Strg+C legt ein
    /// einzelnes Bild als reine Bilddaten ab, Strg+V übernimmt eines von
    /// außen. Die Verbindung lebt, solange das Programm läuft.
    pub bildablage: crate::bildablage::BildAblage,
    /// Datei-Reiter, der auf die Rückfrage „ungespeicherte Änderungen“ wartet
    pub confirm_close: Option<usize>,
    /// Reiter, der nach dem Sichern geschlossen wird (T0143): „Speichern und
    /// schließen“ ohne Speicherort öffnet zuerst den Dateidialog; erst wenn
    /// dort ein Ziel gewählt ist, geht der Reiter zu. Bricht der Nutzer den
    /// Dateidialog ab, bleibt der Merker nicht stehen und der Reiter offen.
    pub schliessen_nach_speichern: Option<usize>,
    pub last_canvas: Rect,
    /// Zeichenfläche und Fensterfläche des **vorigen** Bildes (T0184). Daraus
    /// erkennt `canvas::show`, ob sich nur die Zeichenfläche geändert hat
    /// (Seiten-Panel auf oder zu) — dann bleibt die Zeichnung an ihrem
    /// Bildschirmplatz. `None` im ersten Bild: da gibt es noch nichts zu halten.
    pub letzte_flaeche: Option<(Rect, Rect)>,
    /// Kennzahlen des Pools für die Einstellungsseite:
    /// (Zeit, Bytes, Stände, Dateien).
    pub sicherung_info: Option<(f64, u64, usize, usize)>,
    /// T0188: Entwurf der Sicherungseinstellungen. Die Seite schreibt nur hier
    /// hinein; erst „Übernehmen“ macht daraus Einstellungen.
    pub sicherung_entwurf: Option<jjcad_core::sicherung::Einstellungen>,
    /// T0188: Warnung vor dem Übernehmen — (Entwurf, Zahl der Stände, die
    /// dabei endgültig gelöscht würden).
    pub sicherung_warnung: Option<(jjcad_core::sicherung::Einstellungen, usize)>,
    /// T0188: Rückfrage vor dem Aufräumen — Zahl der Stände ohne Zeichnung.
    pub sicherung_aufraeumen: Option<usize>,
    /// Wie `sicherung_warnung_neu`, für die Rückfrage vor dem Aufräumen.
    pub sicherung_aufraeumen_neu: bool,
    /// T0188: Der Warndialog wurde gerade erst geöffnet. Im selben Bild darf
    /// sein Knopf den Klick nicht annehmen — sonst bestätigt der Klick, der die
    /// Warnung auslöst, sie gleich mit (Prüfbild: 10 Stände ohne Rückfrage weg).
    pub sicherung_warnung_neu: bool,
    /// T0223: Fenster „Wiederherstellen“ — `Some`, solange es offen ist.
    pub wiederherstellen: Option<Wiederherstellen>,
    /// R2.3: Meldung, dass die Zieldatei außerhalb von JoJa-CAD geändert wurde
    /// und der Benutzer entscheiden muss, welche Fassung weiterverwendet wird.
    pub konflikt: Option<Konflikt>,
    /// Höhe des zuletzt gezeichneten Ribbon-Inhalts (Regressionstest t_92b03440)
    pub last_ribbon_content_h: f32,
    /// Lage der Hotbar-Slots in der unteren Leiste (Regressionstest T0043:
    /// die Slots müssen mittig in der Leiste stehen)
    pub last_slots: Option<Rect>,
    /// Rechtecke der Blatt-Chips im Reiterstreifen (T0073) — geprüft wird, dass
    /// die Umrisse zweier Chips sich nicht berühren (`tests_mehrere_dateien`).
    pub last_blattrechtecke: Option<Vec<Rect>>,
    /// Lage der linken und rechten Schaltergruppe der unteren Leiste — geprüft
    /// wird der Abstand zum Fensterrand (`tests_untere_leiste`).
    pub last_kopf_links: Option<Rect>,
    pub last_kopf_rechts: Option<Rect>,
    /// Lage der beiden Schalter „Verschieben“ und „Drehpunkt“ in der rechten
    /// Gruppe der unteren Leiste (T0108) — der Test klickt sie an.
    pub last_chip_verschieben: Option<Rect>,
    pub last_chip_drehpunkt: Option<Rect>,
    /// Welche physische Taste erzeugt welches Zeichen? `egui::Key` kennt keine
    /// Umlaute; die Zuordnung wird beim Tippen gelernt (T0121) und trägt den
    /// Dauerlauf des Schwenks, der `keys_down` liest.
    pub zeichen_taste: HashMap<char, egui::Key>,
    /// Tasten des Tastenbilds der Tastaturbelegung in Fensterkoordinaten
    /// (Regressionstest T0045: gesperrte Tasten dürfen sich nicht belegen lassen)
    pub last_tasten: Option<Vec<(&'static str, &'static str, Rect)>>,
    /// Plätze des Inventar-Lagers in Fensterkoordinaten samt ihrem Index —
    /// geprüft wird, dass die mittlere Maustaste genau den getroffenen Platz
    /// leert (T0088, wie bei den Hotbar-Slots).
    pub last_lager: Option<Vec<(usize, Rect)>>,
    /// T0188: gewählte Unterkategorie der Blockbibliothek — 0 = System,
    /// 1 = Benutzer, 2 = Projekt (Reihenfolge wie im Reiterstreifen).
    pub block_quelle: usize,
    /// Flächen der drei Unterkategorie-Reiter im zuletzt gezeichneten Bild
    /// (Index wie `block_quelle`) — der Prüfweg für den Reiterwechsel.
    pub last_block_reiter: Vec<(usize, Rect)>,
    /// Breite des zuletzt gezeichneten Ribbon-Inhalts
    pub last_ribbon_content_w: f32,
    /// Rechtecke der selbst gezeichneten Fensterknöpfe (T0098) in der
    /// Reihenfolge Minimieren, Vergrößern/Wiederherstellen, Schließen —
    /// `None`, solange die Titelleiste des Fenstermanagers da ist.
    pub last_fenster_knoepfe: Option<[Rect; 3]>,
    /// Zonen zum Ziehen der Fensterkanten (T0098) mit ihrer Richtung
    pub last_fensterkanten: Option<Vec<(&'static str, Rect)>>,
    /// Lage des sichtbaren Größen-Griffs in der Ecke unten rechts (T0098)
    pub last_fenster_griff: Option<Rect>,
    /// Fensterausschnitt **vor** dem Schattenrand (T0147) — egui behält den
    /// zuletzt gesetzten Bildschirmausschnitt, wenn eine Eingabe keinen nennt;
    /// ohne den gemerkten Außenrand schrumpfte die Fläche Bild für Bild.
    pub fenster_aussen: Option<Rect>,
    /// Kante unter dem Zeiger samt Zeitpunkt des Ankommens (T0098, Turn 7) —
    /// daraus entsteht die Verzögerung, bis die Kante aufleuchtet.
    pub kanten_verweil: Option<(&'static str, f64)>,
    /// Kante, die in diesem Bild hervorgehoben wird (nach der Verweildauer
    /// oder während eines laufenden Zuges)
    pub kanten_leuchten: Option<&'static str>,
    /// Kante, an der gerade die Größe gezogen wird (T0098, Turn 9): die
    /// Markierung bleibt stehen, auch wenn der Zeiger den Rand verlässt.
    pub kanten_zug: Option<&'static str>,
    /// Zeitpunkt der letzten Änderung während des Zuges (Nachlauf)
    pub kanten_zug_seit: f64,
    /// Gerundete Fenstergröße beim letzten Bild des Zuges — ändert sie sich,
    /// läuft der Nachlauf neu
    pub kanten_zug_groesse: (f32, f32),
    /// Beginn des Zuges (Not-Aus nach `ZUG_MAX_S`)
    pub kanten_zug_start: f64,
    /// Zeitpunkt der letzten Zeigerbewegung (T0098, Turn 11)
    pub zeiger_bewegt_seit: f64,
    pub zoom_edit: Option<String>,
    pub typed: String,
    /// Signatur der Auswahl für die letzte Zeichnung (Panel-Aktualisierung)
    pub sel_stamp: u64,
    /// Richtung eines **gedrückt gehaltenen** Schwenk-Slots der Hotbar (T0101).
    /// Die Leiste setzt sie beim Zeichnen, `App::schwenk_fortschritt` liest und
    /// löscht sie im selben Bild — so läuft der Dauerlauf nur, solange die
    /// Maustaste auf dem Slot liegt.
    pub schwenk_zeiger: Option<(f32, f32)>,
    /// Ansichtsfenster, dessen **Ansichtsbereich** gerade eingestellt wird
    /// (T0158). Solange der Schalter im Eigenschaften-Panel an ist, wirken
    /// Mausrad und Mitteltaste auf dieses Fenster statt auf das Blatt:
    /// Verschieben ändert die Modellmitte, Zoomen den Maßstab.
    pub vp_edit: Option<Id>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            pending: PendingStyle::default(),
            ribbon: RibbonTab::Start_default(),
            inventory_open: false,
            inventory_tab: 0,
            block_quelle: 0,
            last_block_reiter: Vec::new(),
            layers_open: true,
            props_open: true,
            blatt_open: true,
            layers_collapsed: false,
            props_collapsed: false,
            blatt_collapsed: false,
            layers_side: PanelSeite::Links,
            props_side: PanelSeite::Rechts,
            blatt_side: PanelSeite::Rechts,
            panel_benutzt: Vec::new(),
            // T0233: Im ersten Bild gibt es keinen Übergang zu erkennen.
            panel_offen_vorher: [true, true, true],
            last_leisten_icons: Vec::new(),
            last_layer_kopf: Vec::new(),
            last_blattzeilen: Vec::new(),
            ribbon_collapsed: false,
            keymap_open: false,
            settings_open: false,
            settings_kat: 0,
            settings_suche: String::new(),
            speicherorte: Default::default(),
            speicherorte_geladen: false,
            vorlage: String::new(),
            last_settings_suche: None,
            last_settings_nav: None,
            last_settings_pane: None,
            last_settings_fenster: None,
            help_open: false,
            about_open: false,
            sheet_dialog: false,
            project_dialog: false,
            texteingabe: None,
            block_name_dialog: None,
            block_eigenschaften: None,
            preset_dialog: None,
            hotbar_picker_open: false,
            file_dialog: None,
            systemdialog: None,
            pdf_import: None,
            png_bereich: None,
            png_wahl: None,
            png_export_offen: false,
            png_einstellungen: PngExport {
                breite_px: 2400,
                papier: true,
            },
            drop_demo: None,
            text_format_next: None,
            show_grid: true,
            lineweights: true,
            fullscreen: false,
            lineweights_scaled: false,
            lineweight_factor: 1.0,
            drag_mode: DragMode::HoldAndDrag,
            select_mode: DragMode::HoldAndDrag,
            fill_input: FillInput::Points,
            hatch_input: FillInput::Contour,
            // Programmstandard ist das Einfügen per Punkten (wie bei der Fläche)
            volume_input: FillInput::Points,
            copy_input: CopyInput::BasePoint,
            fill_active: None,
            menu: None,
            order_pick: None,
            bind_slot: None,
            bind_key: None,
            slot_drag: None,
            keymap_selected: None,
            keymap_name: None,
            keymap_wechsel: None,
            keymap_pos: None,
            last_keymap_rect: None,
            last_keymap_info: None,
            last_keymap_zeile: None,
            kombi_ansicht: jjcad_core::persist::Kombination::KEINE,
            last_kombi_icon: None,
            last_kombi_schalter: Vec::new(),
            hotbar_kombi: jjcad_core::persist::Kombination::KEINE,
            keymap_funktion: None,
            search: String::new(),
            new_layer_dialog: None,
            new_layer_plan: None,
            new_plan_dialog: None,
            color_popup: None,
            layer_delete: None,
            block_delete: None,
            rename_layer: None,
            rename_layer_fokus: false,
            rename_plan: None,
            clipboard: crate::ghost::Clipboard::default(),
            clipboard_offset: 20.0,
            // Tests fassen die Systemablage nicht an: sie gehört dem Rechner.
            bildablage: if cfg!(test) {
                crate::bildablage::BildAblage::ohne_systemablage()
            } else {
                crate::bildablage::BildAblage::neu()
            },
            confirm_close: None,
            schliessen_nach_speichern: None,
            last_canvas: Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)),
            letzte_flaeche: None,
            sicherung_info: None,
            sicherung_entwurf: None,
            sicherung_warnung: None,
            sicherung_aufraeumen: None,
            sicherung_warnung_neu: false,
            wiederherstellen: None,
            sicherung_aufraeumen_neu: false,
            konflikt: None,
            last_ribbon_content_h: 0.0,
            last_slots: None,
            last_blattrechtecke: None,
            last_kopf_links: None,
            last_kopf_rechts: None,
            last_chip_verschieben: None,
            last_chip_drehpunkt: None,
            zeichen_taste: HashMap::new(),
            last_tasten: None,
            last_lager: None,
            last_ribbon_content_w: 0.0,
            last_fenster_knoepfe: None,
            last_fensterkanten: None,
            last_fenster_griff: None,
            fenster_aussen: None,
            kanten_verweil: None,
            kanten_leuchten: None,
            kanten_zug: None,
            kanten_zug_seit: 0.0,
            kanten_zug_groesse: (0.0, 0.0),
            kanten_zug_start: 0.0,
            zeiger_bewegt_seit: 0.0,
            zoom_edit: None,
            typed: String::new(),
            sel_stamp: 0,
            schwenk_zeiger: None,
            vp_edit: None,
        }
    }
}

impl RibbonTab {
    #[allow(non_snake_case)]
    fn Start_default() -> RibbonTab {
        RibbonTab::Datei
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Build-Kennung: Version aus Cargo.toml + Git-Commit-Zähler (build.rs).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD: &str = env!("JJCAD_BUILD");
pub const BUILD_HASH: &str = env!("JJCAD_HASH");
/// Auslieferungsnummer (build-live.sh), leer bei lokalen Builds
pub const SHIP: &str = env!("JJCAD_SHIP");

/// Kennung eines ausgelieferten Builds: "s3" oder "b12" (lokaler Build).
pub fn build_id() -> String {
    if SHIP.is_empty() {
        format!("b{BUILD}")
    } else {
        format!("s{SHIP}")
    }
}

/// Anzeige der Build-Kennung, z. B. "v0.2.42 (9b34136)".
/// Kurzform für einen Katalogtext (T0103) — hält die Ribbon-Zeilen lesbar.
pub fn rb(key: &'static str) -> &'static str {
    jjcad_core::i18n::tr(key)
}

/// Beschriftung einer Action in der eingestellten Sprache (T0103). Der Text
/// wird beim Zeichnen nachgeschlagen — eine Umschaltung wirkt ohne Neustart.
pub fn aktion_label(id: &str) -> &'static str {
    jjcad_core::actions::action(id)
        .map(|d| d.label())
        .unwrap_or("")
}

pub fn build_label() -> String {
    format!("v{} ({} · {})", VERSION, build_id(), BUILD_HASH)
}

/// Fenstertitel inklusive Build-Nummer.
pub fn window_title() -> String {
    format!("JoJa-CAD v{} · {}", VERSION, build_id())
}

/// Laufende Verschiebung der Auswahl mit der Maus.
pub struct MoveDrag {
    pub start: V2,
    pub cur: V2,
    /// Frame, in dem die Verschiebung begann (verhindert sofortiges Beenden
    /// im Zwei-Klick-Modus)
    pub frame: f64,
    pub orig: Vec<jjcad_core::model::Entity>,
    /// Undo-Snapshot erst beim ersten echten Verschieben anlegen
    pub recorded: bool,
}

/// Eingabemodus für Füllflächen und Schraffuren (beide erzeugen dasselbe Objekt).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillInput {
    /// Geschlossene Kontur anklicken (übernimmt deren Umriss)
    Contour,
    /// Punkte setzen, Enter schließt
    Points,
    /// Farbeimer: Klick in die Fläche, Umriss wird ermittelt
    Paint,
}

/// Wofür die Einfüge-Modi gelten: Füllfläche, Schraffur oder Volumen.
///
/// Dieselben drei Eingaben (Kontur anklicken, Punkte setzen, Farbeimer) für
/// alle drei — nur die Action und die Muster-Vorgabe unterscheiden sich.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillKind {
    Area,
    Hatch,
    Volume,
}

impl FillKind {
    pub fn action(self, mode: FillInput) -> &'static str {
        match (self, mode) {
            (FillKind::Volume, FillInput::Contour) => "draw.volume",
            (FillKind::Volume, FillInput::Points) => "draw.volume.points",
            (FillKind::Volume, FillInput::Paint) => "draw.volume.paint",
            // Fläche und Schraffur teilen sich die Actions; die Schraffur
            // unterscheidet sich nur in der Muster-Vorgabe.
            (_, FillInput::Contour) => "draw.fill",
            (_, FillInput::Points) => "draw.fill.points",
            (_, FillInput::Paint) => "draw.fill.paint",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FillKind::Area => rb("ui.fillkind.flaeche"),
            FillKind::Hatch => rb("ui.fillkind.schraffur"),
            FillKind::Volume => rb("ui.fillkind.volumen"),
        }
    }

    /// Symbol der Schaltfläche: das Element selbst. Beim Volumen immer der
    /// Würfel — der Einfügemodus steht im Menü und in der Beschriftung.
    pub fn button_icon(self, mode: FillInput) -> &'static str {
        match self {
            FillKind::Volume => "volume",
            _ => mode.icon(self == FillKind::Hatch),
        }
    }

    pub fn pattern(self) -> jjcad_core::style::FillPattern {
        match self {
            FillKind::Hatch => jjcad_core::style::FillPattern::Lines,
            _ => jjcad_core::style::FillPattern::Solid,
        }
    }
}

impl FillInput {
    pub const ALL: [FillInput; 3] = [Self::Contour, Self::Points, Self::Paint];

    pub fn label(self) -> &'static str {
        match self {
            Self::Contour => rb("ui.fillinput.kontur"),
            Self::Points => rb("ui.fillinput.punkte"),
            Self::Paint => rb("ui.fillinput.farbeimer"),
        }
    }

    pub fn icon(self, hatch: bool) -> &'static str {
        match (self, hatch) {
            (Self::Contour, false) => "fill-contour",
            (Self::Points, false) => "fill-points",
            (Self::Paint, false) => "fill-paint",
            (Self::Contour, true) => "hatch-contour",
            (Self::Points, true) => "hatch-points",
            (Self::Paint, true) => "hatch-paint",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Contour => "contour",
            Self::Points => "points",
            Self::Paint => "paint",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "points" => Self::Points,
            "paint" => Self::Paint,
            _ => Self::Contour,
        }
    }
}

/// Kopierfunktion der geteilten Schaltfläche „Kopieren".
///
/// Zwei Funktionen, ein Knopf: „Kopieren" legt die Auswahl in die Zwischenablage
/// (Strg+C), „Kopieren mit Basispunkt" setzt eine Kopie versetzt ab
/// (Strg+Umschalt+C). Der Pfeil wählt — wie bei den Flächen — den Modus, das
/// Icon zeigt ihn (Basispunkt: kleiner Punkt unten links).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CopyInput {
    /// Nur in die Zwischenablage (Einfügen mit Vorschau)
    Clipboard,
    /// Kopie mit Basispunkt und Zielpunkt absetzen
    BasePoint,
}

impl CopyInput {
    pub const ALL: [CopyInput; 2] = [Self::Clipboard, Self::BasePoint];

    pub fn action(self) -> &'static str {
        match self {
            Self::Clipboard => "edit.copy",
            Self::BasePoint => "modify.copy",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Clipboard => rb("ui.copyinput.kopieren"),
            Self::BasePoint => rb("ui.copyinput.basispunkt"),
        }
    }

    /// Anzeige des Tastenkürzels im Aufklappmenü
    pub fn key_label(self) -> &'static str {
        match self {
            Self::Clipboard => "Strg+C",
            Self::BasePoint => "Strg+Umschalt+C",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Clipboard => "copy",
            Self::BasePoint => "copy-base",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Clipboard => "copy",
            Self::BasePoint => "copy_base",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "copy" => Self::Clipboard,
            _ => Self::BasePoint,
        }
    }
}

/// Wurde Strg+V gerade **losgelassen**? (T0112)
///
/// egui-winit wertet den Tastendruck von Strg+V selbst aus und verschluckt ihn:
/// es fragt nur **Text** aus der Zwischenablage ab und schickt bei einem reinen
/// Bild gar kein Ereignis. Das Loslassen kommt dagegen als gewöhnliches
/// Tastenereignis durch — es ist damit der einzige Anhaltspunkt für „der Nutzer
/// wollte einfügen“.
fn strg_v_losgelassen(ctx: &egui::Context) -> bool {
    ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key {
                    key: egui::Key::V,
                    pressed: false,
                    modifiers,
                    ..
                } if modifiers.command
            )
        })
    })
}

pub struct App {
    pub tabs: Vec<DocTab>,
    /// Selbstdiagnose für Anzeigefehler (F12 nimmt ein Bildschirmfoto auf).
    pub diag: crate::diag::Diagnose,
    /// Kennung des eigenen Fensters für den Systemdialog (None in Tests, die
    /// ohne Fenster laufen)
    pub elternfenster: Option<crate::systemdialog::Elternfenster>,
    pub active: usize,
    pub prefs: UserPrefs,
    pub options: ToolOptions,
    pub tool: Option<Tool>,
    pub ui: UiState,
    pub pointer_world: Option<V2>,
    pub snap_label: Option<String>,
    pub drag_pan: Option<Pos2>,
    pub select_drag: Option<(V2, V2, bool)>,
    /// Laufendes Auswahlfenster im Zwei-Klick-Modus (T0145): erste Ecke, zweite
    /// Ecke und Umschalt-Zusatz. Die zweite Ecke folgt dem Zeiger, bis der
    /// nächste Klick die Auswahl übernimmt (anders als `select_drag` ohne
    /// gedrückte Taste).
    pub select_zwei: Option<(V2, V2, bool)>,
    pub mouse_world: Option<V2>,
    pub last_frame: f64,
    pub status_msg: String,
    /// Meldungen unten links in der Zeichenfläche (Log, mit T geöffnet)
    pub chat: crate::chat::Chat,
    pub new_tab_menu: bool,
    pub started_at: f64,
    pub ctx: egui::Context,
    /// Vorschau vor dem Platzieren (Block oder Zwischenablage)
    pub ghost: Option<crate::ghost::Ghost>,
    pub grip_drag: Option<crate::handles::GripDrag>,
    /// Griff unter dem Zeiger (für Hervorhebung und Vorrang beim Klicken)
    pub hover_grip: Option<(Id, crate::handles::GripKind)>,
    /// Laufende Maus-Verschiebung der Auswahl
    pub move_drag: Option<MoveDrag>,
    /// Frame, in dem der Griff-Drag begann (Zwei-Klick-Modus)
    pub grip_start_frame: f64,
    /// Farbeimer-Vorschau: gemerkte Zeigerposition und die dazu ermittelte
    /// Umgrenzung. Rechnet erst nach kurzem Stillstand (`canvas::PAINT_RUHE`),
    /// sonst würde jede Zeigerbewegung eine Flutfüllung starten.
    pub paint: crate::canvas::PaintPreview,
    /// Erweiterter Punktfang: Punkte, über denen der Zeiger lange genug stand.
    /// Von ihnen gehen Hilfslinien aus, auf denen der nächste Punkt fängt.
    /// Höchstens zwei — damit lässt sich x vom einen und y vom anderen nehmen.
    pub halt: Vec<jjcad_core::snap::HaltPunkt>,
    /// Verweil-Erkennung, die `halt` füllt.
    pub verweil: crate::canvas::VerweilFang,
    /// Vom Nutzer gelegte Hilfslinie während der Eingabe (T0230). `None` =
    /// keine; sie bleibt stehen, bis das Werkzeug endet oder ein weiterer
    /// Rechtsklick sie wegnimmt.
    pub hilfslinie: Option<Hilfslinie>,
    /// Läuft die Eingabe einer Hilfslinie? Der Rechtsklick schaltet sie ein,
    /// der nächste Klick (oder eine getippte Länge + Enter) setzt die Strecke.
    pub hilfslinie_modus: bool,
    /// Ankerpunkt der laufenden Eingabe, wenn es noch keinen Bezugspunkt gibt
    /// (erster Punkt eines Objekts): der erste Klick legt ihn fest.
    pub hilfslinie_anker: Option<V2>,
    /// Leistungsanzeige (Umschalt+F3): Messwerte je Bild und ihr Verlauf
    pub perf: crate::perf::Perf,
    /// Objekte einer laufenden Mehrfachauswahl, für die das Eigenschaften-Panel
    /// gerade zeichnet. Solange hier mehr als ein Objekt steht, schickt
    /// `props::setzen` jede Eigenschaftsänderung an **alle** diese Objekte —
    /// so gilt jede Eigenschaft eines einzelnen Objekts auch für die Auswahl.
    /// Außerhalb des Panel-Zeichnens leer.
    pub mehrfach_ziel: Vec<Id>,
    /// Sperrdatei der Datenwurzel (T0127). Sie lebt so lange wie die
    /// Anwendung und verhindert eine zweite Schreibinstanz.
    pub sperre: Option<jjcad_core::daten::Sperre>,
    /// Sicherung (T0129): der Pool. Wird beim ersten Bedarf geöffnet — Tests,
    /// die nur zeichnen, fassen so den Datenordner des Nutzers nicht an.
    pub sicherung: Option<jjcad_core::sicherung::Sicherung>,
    /// Zeitpunkt der letzten Sicherungsprüfung (egui-Zeit in Sekunden).
    pub sicherung_zuletzt: f64,
    /// T0135: Kanal für eine zweite Instanz — Zeichnung als Reiter öffnen oder
    /// das Fenster nach vorn holen.
    pub instanz: Option<jjcad_core::instanz::Dienst>,
    /// T0224: Ein Auftrag von außen (Zeichnung geöffnet, Fenster zeigen) soll
    /// das Fenster in den Vordergrund holen. Der Wunsch wirkt über einige
    /// Bilder, weil ein Fenstermanager ihn auch ablehnen darf — dann meldet
    /// der letzte Schritt Aufmerksamkeit an. `None` = nichts zu tun.
    pub nach_vorn: Option<NachVorn>,
    /// Ablage der Blockdatei für Blöcke, die direkt entstehen (T0142).
    /// Nur für Tests: `None` heißt „wie eingestellt“ (`prefs.blocks_dir()`).
    /// Ohne diesen Weg schriebe ein Test in die Benutzerbibliothek des Nutzers.
    #[cfg(test)]
    pub(crate) test_blockordner: Option<std::path::PathBuf>,
    /// Nur für Tests: `false` unterdrückt das Speichern der Voreinstellungen,
    /// damit ein Testlauf nicht die `prefs.json` des Nutzers verändert.
    #[cfg(test)]
    pub(crate) test_prefs_speichern: bool,
    /// Zuletzt gemeldete Fenstergröße **im Normalzustand** (T0141, Turn 2).
    ///
    /// Im Vollbild und maximiert meldet das Fenster die Bildschirmgröße; die
    /// taugt nicht als Fenstergröße für den nächsten Start — nach dem Verlassen
    /// des Vollbilds stünde ein bildschirmgroßes Fenster da. Deshalb wird der
    /// letzte normale Stand mitgeführt (`fenster_merken`) und beim Sammeln der
    /// Sitzung anstelle der aktuellen Größe eingesetzt.
    pub fenster_normal: Option<sitzung::Fensterlage>,
    /// `true`, wenn JoJa-CAD den Fensterschatten selbst zeichnet (T0147).
    ///
    /// Unter **Wayland** zeichnet KWin für ein Fenster mit clientseitiger
    /// Dekoration keinen Schatten — dort muss die Anwendung ihn selbst malen.
    /// Unter **X11** tut KWin es (gemessen: derselbe Schatten mit und ohne
    /// Titelleiste), und ohne Compositor gäbe es für den durchsichtigen Rand
    /// keinen Grund. Der Wert steht beim Start fest
    /// ([`schatten_selbst_zeichnen`]) und ist in Tests setzbar.
    pub schatten_im_rand: bool,
    /// Startzustand des Fensters (T0141, Turn 2): Vollbild oder maximiert, aus
    /// der Sitzung. Wird im **ersten Bild** gesetzt und dann verbraucht — beim
    /// Erzeugen des Fensters nimmt der Fenstermanager den Wunsch nicht
    /// zuverlässig an (unter X11 verwirft KWin die ClientMessage an ein noch
    /// nicht abgebildetes Fenster; gemessen: `_NET_WM_STATE` blieb leer).
    pub fenster_start: Option<sitzung::Fensterlage>,
    /// Erster Versuch, den Startzustand zu setzen (egui-Zeit). Danach wird
    /// höchstens `FENSTER_START_GEDULD` Sekunden wiederholt.
    pub fenster_start_zeit: Option<f64>,
}

/// Ein Seitenpanel, das auf **beiden** Seiten liegen kann (T0203).
///
/// Die Leisten beider Seiten zeigen immer dieselben Symbole — Layer,
/// Eigenschaften und Blattmanager (T0218). Welches Panel wo liegt, steht in
/// `UiState::layers_side` / `UiState::props_side` / `UiState::blatt_side`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeitenPanel {
    Layer,
    Eigenschaften,
    Blatt,
}

impl SeitenPanel {
    /// Alle Panels in der Reihenfolge der Leisten — auf beiden Seiten gleich.
    pub const ALLE: [SeitenPanel; 3] = [
        SeitenPanel::Layer,
        SeitenPanel::Eigenschaften,
        SeitenPanel::Blatt,
    ];

    /// Symbol in der Leiste.
    pub fn icon(self) -> &'static str {
        match self {
            SeitenPanel::Layer => "layers",
            SeitenPanel::Eigenschaften => "properties",
            SeitenPanel::Blatt => "sheet",
        }
    }

    /// Breite des ausgeklappten Panels (232 bzw. 292 px).
    pub fn breite(self) -> f32 {
        match self {
            SeitenPanel::Layer => 232.0,
            SeitenPanel::Eigenschaften => 292.0,
            SeitenPanel::Blatt => 232.0,
        }
    }
}

/// T0224: Wie lange ein Auftrag von außen das Fenster nach vorn holen darf.
///
/// Ein Fenstermanager darf einen Fokuswunsch ablehnen (Zen-Regel, anderes
/// Vollbildfenster); dann ist Wiederholen zwecklos. Nach dieser Zeit gibt die
/// Anwendung auf, damit sie den Nutzer nicht dauerhaft bedrängt.
pub const NACH_VORN_DAUER: f64 = 3.0;

/// T0224: Ab wann der abgelehnte Wunsch als Aufmerksamkeit gemeldet wird.
///
/// Unter **Wayland** ist `ViewportCommand::Focus` wirkungslos (winit reicht
/// dort nichts weiter); die Aktivierung entscheidet der Compositor. Er kann sie
/// erst nach einem sichtbaren Anliegen gewähren — `RequestUserAttention` legt
/// dafür ein `xdg_activation`-Anliegen vor. Kurz warten, damit ein
/// Fenstermanager, der den Fokus sofort gibt, kein Aufblinken der Taskleiste
/// erzeugt.
pub const NACH_VORN_MELDEN: f64 = 0.25;

/// T0224: Ein noch offener Wunsch, das Fenster nach vorn zu holen.
#[derive(Clone, Debug)]
pub struct NachVorn {
    /// Zeitpunkt des Auftrags (egui-Zeit in Sekunden).
    pub seit: f64,
    /// `true`, sobald Aufmerksamkeit angemeldet wurde.
    pub gemeldet: bool,
    /// `xdg-activation`-Token des Aufrufers, falls eines mitkam (Wayland).
    pub token: Option<String>,
    /// `true`, sobald das Token beim Compositor vorgelegt wurde — es gilt
    /// einmal und wird nicht wiederholt.
    pub nativ: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Klick-Toleranz: egui verwirft einen Klick, sobald sich der Zeiger zwischen
        // Drücken und Loslassen mehr als 6 px bewegt — beim schnellen Klicken passiert
        // das leicht. 10 px sind für Maus und Touchpad unkritisch.
        cc.egui_ctx
            .options_mut(|o| o.input_options.max_click_dist = 10.0);
        let mut app = Self::with_ctx(cc.egui_ctx.clone());
        // Fensterkennung: der Systemdialog erscheint damit als Fenster dieser
        // Anwendung (bei KDE über dem Hauptfenster) statt frei zu schweben.
        app.elternfenster = crate::systemdialog::Elternfenster::aus_eframe(cc);
        app
    }

    /// Baut die App auf einem vorhandenen egui-Kontext — ohne Fenster, damit
    /// Dialoge in Tests headless gezeichnet werden können.
    pub fn with_ctx(ctx: egui::Context) -> Self {
        theme::install(&ctx);
        // Tests fassen den Datenordner des Nutzers nicht an: eigene Vorgaben
        // und ein eigener Sicherungsordner je App. Ohne das schrieb der
        // Testlauf über Tage hunderte Stände in den echten Pool (T0188).
        let prefs = if false && cfg!(test) {
            let mut p = UserPrefs::default();
            p.sicherungen_pfad = std::env::temp_dir()
                .join(format!(
                    "jjcad-test-sicherungen-{}-{}",
                    std::process::id(),
                    {
                        use std::sync::atomic::{AtomicU64, Ordering};
                        static N: AtomicU64 = AtomicU64::new(0);
                        N.fetch_add(1, Ordering::Relaxed)
                    }
                ))
                .to_string_lossy()
                .to_string();
            p
        } else {
            UserPrefs::load()
        };
        // T0103: die gespeicherte Sprache gilt ab dem ersten Bild.
        jjcad_core::i18n::set_language(prefs.language);
        // T0102: ebenso das Design — bei „System“ entscheidet das
        // Betriebssystem (hell/dunkel), sonst die gespeicherte Wahl.
        theme::set_design(prefs.design, &ctx);
        // T0111: und der Farbakzent (Vorgabe Bernstein) — er färbt alle
        // Akzentstellen der Bedienung ab dem ersten Bild.
        theme::set_akzent(prefs.farbakzent, &ctx);
        // T0181: eigene Zeichenfarben (Hintergründe, Vorschauen, Auswahl,
        // Fang) — gelten ab dem ersten Bild, fehlende Rollen folgen dem Design.
        theme::set_zeichenfarben(&theme::farben_aus_prefs(&prefs), &ctx);
        // T0165: Kantenglättung der Zeichnung (Einstellungen → Grafik) — gilt
        // ab dem ersten Bild, damit die Flächen nicht erst nach einem Wechsel
        // hart oder weich werden.
        theme::set_kantenglaettung(prefs.kantenglaettung, prefs.kantenweichheit, &ctx);
        // T0165: Leistungsanzeige nur, wenn sie beim letzten Mal offen war.
        let perf_anzeige = prefs.perf_anzeige;
        let mut s = Session::new();
        // T0125: eine neue Zeichnung trägt die eingestellte Linienstärke je
        // Objektart (Vorgabe 0,18 mm für alle).
        s.doc.setze_standard_breiten(&prefs.layer_weights);
        s.doc.name = "Unbenannt".into();
        // App-weite Vorliebe für den Wandkontur-Fang übernehmen, damit die
        // Einstellung auch ohne gespeicherte Zeichnung einen Neustart überlebt.
        s.doc.settings.snap.wall_edges = prefs.snap_wall_edges;
        let tab = DocTab {
            session: s,
            tab: TabKind::Model,
            view_model: View::default(),
            sheet_views: HashMap::new(),
            block_views: HashMap::new(),
            offene_bloecke: Vec::new(),
            geschlossene_blaetter: Vec::new(),
            block_original: HashMap::new(),
            rev: 0,
            cache: None,
            hover: None,
        };
        let options = ToolOptions::from_doc(&tab.session.doc);
        let show_grid = tab.session.doc.settings.show_grid;
        let lineweights = prefs.show_lineweights;
        let lineweights_scaled = prefs.lineweights_scaled;
        let lineweight_factor = prefs.lineweight_factor;
        let drag_mode = prefs.drag_mode;
        // T0145: eigener Zieh-Modus für das Auswahlfenster.
        let select_mode = prefs.select_mode;
        let layers_open = prefs.show_layers;
        let props_open = prefs.show_properties;
        let blatt_open = prefs.show_blattmanager;
        // T0203: Die gewählte Seite der beiden Panels übersteht den Neustart.
        let layers_side = prefs.layers_side;
        let props_side = prefs.props_side;
        let blatt_side = prefs.blatt_side;
        let mut app = Self {
            tabs: vec![tab],
            diag: crate::diag::Diagnose::new(),
            elternfenster: None,
            active: 0,
            prefs,
            options,
            tool: None,
            ui: UiState::default(),
            pointer_world: None,
            snap_label: None,
            drag_pan: None,
            select_drag: None,
            select_zwei: None,
            mouse_world: None,
            last_frame: 0.0,
            status_msg: String::new(),
            chat: crate::chat::Chat::neu(),
            new_tab_menu: false,
            started_at: 0.0,
            ctx: ctx.clone(),
            ghost: None,
            grip_drag: None,
            hover_grip: None,
            move_drag: None,
            grip_start_frame: 0.0,
            paint: crate::canvas::PaintPreview::default(),
            halt: Vec::new(),
            verweil: crate::canvas::VerweilFang::default(),
            hilfslinie: None,
            hilfslinie_modus: false,
            hilfslinie_anker: None,
            perf: crate::perf::Perf::default(),
            mehrfach_ziel: Vec::new(),
            sperre: None,
            sicherung: None,
            sicherung_zuletzt: 0.0,
            instanz: None,
            nach_vorn: None,
            #[cfg(test)]
            test_blockordner: None,
            #[cfg(test)]
            test_prefs_speichern: true,
            fenster_normal: None,
            schatten_im_rand: schatten_selbst_zeichnen(),
            fenster_start: None,
            fenster_start_zeit: None,
        };
        app.perf.an = perf_anzeige;
        app.ui.show_grid = show_grid;
        app.ui.lineweights = lineweights;
        app.ui.lineweights_scaled = lineweights_scaled;
        app.ui.lineweight_factor = lineweight_factor;
        app.ui.drag_mode = drag_mode;
        app.ui.select_mode = select_mode;
        app.ui.copy_input = CopyInput::from_key(&app.prefs.copy_input);
        app.ui.layers_open = layers_open;
        app.ui.props_open = props_open;
        app.ui.blatt_open = blatt_open;
        // T0218: Die Prüfläufe rechnen mit der vollen Zeichenfläche. Ein
        // Blattmanager aus der echten `prefs.json` verschöbe sie; Tests
        // starten deshalb ohne ihn und schalten ihn bei Bedarf selbst ein.
        if cfg!(test) {
            app.ui.blatt_open = false;
        }
        app.ui.layers_side = layers_side;
        app.ui.props_side = props_side;
        app.ui.blatt_side = blatt_side;
        app.fit_view();
        app.started_at = 0.0;
        // T0127: Blöcke aus der Benutzerbibliothek der Datenwurzel.
        app.benutzerbloecke_einhaengen();
        app
    }

    /// Erlaubt das Setzen eines Startzustands für Screenshots/automatisierte UI-Tests:
    /// Prüfhilfe (T0090): eine Füllfläche in Metern anlegen.
    fn demo_flaeche(&mut self, pts: [[f64; 2]; 4]) -> Option<u64> {
        let r = self.session_mut().execute(
            "draw.fill.points",
            &serde_json::json!({
                "pts": pts,
                "pattern": "lines",
                "angle": 45.0,
                "scale": 0.25,
                "border": true,
                "fill": "#d8e4f0",
            }),
        );
        r.ok().and_then(|v| v["created"][0].as_u64())
    }

    /// Prüfhilfe (T0204): Benutzerblock „Tisch“ samt eingesetzter Instanz im
    /// Modell — Prüfzustände `blockmenue`, `blockpanel` und
    /// `blockeigenschaften` zeigen damit Block-Eigenschaften.
    fn demo_benutzerblock(&mut self) -> Option<u64> {
        let layer = self.doc().current_layer;
        if self.doc().block("usr:tisch").is_none() {
            let mut kreis = Entity::new(
                1,
                layer,
                EntityKind::Circle {
                    c: jjcad_core::geom::v(0.0, 0.0),
                    r: 600.0,
                },
            );
            kreis.by_block = true;
            self.doc_mut().blocks.push(jjcad_core::model::BlockDef {
                id: "usr:tisch".into(),
                name: "Tisch".into(),
                category: "Projekt".into(),
                source: jjcad_core::model::BlockSource::User,
                description: String::new(),
                entities: vec![kreis],
                icon: None,
                base: V2::ZERO,
                groesse: BlockGroesse::Fest,
            });
        }
        if let Some(id) = self
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::BlockRef { .. }))
            .map(|e| e.id)
        {
            return Some(id);
        }
        let id = self.doc_mut().new_id();
        self.doc_mut().add_entity(Entity::new(
            id,
            layer,
            EntityKind::BlockRef {
                block: "usr:tisch".into(),
                pos: V2::ZERO,
                rot: 0.0,
                scale: V2::new(1.0, 1.0),
                mirror: false,
            },
        ));
        Some(id)
    }

    /// Prüfhilfe (T0090): Ausschnitt auf die 8 × 3 m große Probe legen.
    fn demo_ausschnitt(&mut self) {
        let mut v = self.current_view();
        v.center = V2::new(4000.0, 1500.0);
        v.zoom = 0.08;
        self.set_view(v);
    }

    /// `JJCAD_DEMO_UI=arch,inv,sheet,wall,text` — für die Verschiebe-Vorschau
    /// zusätzlich `moveline`, `movewall`, `movedoor`, `moveall` (Auswahl setzen
    /// und Werkzeug „Verschieben" mit Basispunkt starten) sowie `selall` (nur
    /// auswählen, zum Prüfen des direkten Ziehens) sowie `dateidialog` (offener
    /// Systemdialog mit Abdunklung, ohne echten Dialog).
    pub fn apply_demo_ui(&mut self) {
        // Demoobjekte liegen auf der im Layer-Panel gewählten Ebene — dieselbe
        // Ebene, die ein Werkzeug übergeben würde (T0039).
        let ebene = self.doc().current_layer;
        let Ok(spec) = std::env::var("JJCAD_DEMO_UI") else { return };
        for part in spec.split(',') {
            match part.trim() {
                // Leere Zeichnung (eine Datei ohne Beispielinhalt) — für
                // Prüfbilder der Vorgaben.
                "neudatei" | "neue_datei" => {
                    self.new_document();
                    self.fit_view();
                }
                // T0244: Linienwerkzeug mit gesetztem Anfangspunkt — das Feld am
                // Zeiger zeigt Länge und Winkel; `laengenfeldtippen` setzt
                // zusätzlich eine getippte Zahl, damit dasselbe Feld
                // hervorgehoben erscheint.
                "laengenfeld" | "laengenfeldtippen" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.start_tool("draw.line");
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(jjcad_core::geom::Vec2::new(0.0, 0.0));
                    }
                    if part.trim() == "laengenfeldtippen" {
                        self.ui.typed = "1,5".into();
                    }
                }
                "arch" => self.ui.ribbon = RibbonTab::Architektur,
                "draw" => self.ui.ribbon = RibbonTab::Zeichnen,
                "dim" => self.ui.ribbon = RibbonTab::Bemassung,
                // T0235: Fenster „PNG exportieren" offen.
                "png" => {
                    self.ui.ribbon = RibbonTab::Datei;
                    self.png_export_oeffnen();
                }
                "bloecke" | "blocks" => self.ui.ribbon = RibbonTab::Bloecke,
                "blatt" | "sheet" => self.ui.ribbon = RibbonTab::Blatt,
                // T0218: Blattmanager mit drei Blättern — zwei im Plansatz,
                // der Reiter des zweiten geschlossen (bleibt in der Liste).
                "blattmanager" => {
                    self.ui.ribbon = RibbonTab::Blatt;
                    self.ui.blatt_open = true;
                    for i in 2..=3 {
                        let _ = self.session_mut().execute(
                            "sheet.new",
                            &serde_json::json!({"name": format!("Blatt {i}")}),
                        );
                    }
                    let ids: Vec<Id> = self.doc().sheets.iter().map(|s| s.id).collect();
                    if ids.len() >= 3 {
                        for id in [ids[0], ids[1]] {
                            let _ = self.session_mut().execute(
                                "sheet.update",
                                &serde_json::json!({"sheet": id, "plansatz": true}),
                            );
                        }
                        self.blatt_tab_schliessen(ids[1]);
                        self.switch_tab(TabKind::Sheet(ids[2]));
                    }
                }
                "ansicht" => self.ui.ribbon = RibbonTab::Ansicht,
                // Drehen der Zeichnungsansicht: Werkzeug läuft (Aufforderung
                // „Zentrum klicken") bzw. die Ansicht ist schon gedreht.
                "drehen" => {
                    self.ui.ribbon = RibbonTab::Ansicht;
                    self.start_tool("view.rotate_tool");
                }
                "gedreht" => {
                    self.ui.ribbon = RibbonTab::Ansicht;
                    let mut v = self.current_view();
                    let mitte = v.center;
                    v.rotate_about(mitte, 30f64.to_radians());
                    self.set_view(v);
                }
                // T0203: Seitenpanels auf beiden Seiten. `leisten` klappt beide
                // ein (beide Leisten mit denselben Symbolen), `panelslinks` /
                // `panelsrechts` legen beide ausgeklappt auf eine Seite.
                "leisten" => {
                    self.ui.layers_open = true;
                    self.ui.props_open = true;
                    self.ui.blatt_open = true;
                    self.ui.layers_collapsed = true;
                    self.ui.props_collapsed = true;
                    self.ui.blatt_collapsed = true;
                }
                "panelslinks" => {
                    self.ui.layers_open = true;
                    self.ui.props_open = true;
                    self.ui.layers_collapsed = false;
                    self.ui.props_collapsed = false;
                    self.ui.layers_side = PanelSeite::Links;
                    self.ui.props_side = PanelSeite::Links;
                }
                // T0222: Alle drei Panels auf **einer** Seite. Die Grenze von
                // zwei ausgeklappten Panels greift, das dritte liegt als Symbol
                // in der Leiste.
                "panelsdrei" => {
                    self.ui.layers_open = true;
                    self.ui.props_open = true;
                    self.ui.blatt_open = true;
                    self.ui.layers_collapsed = false;
                    self.ui.props_collapsed = false;
                    self.ui.blatt_collapsed = false;
                    self.ui.layers_side = PanelSeite::Links;
                    self.ui.props_side = PanelSeite::Links;
                    self.ui.blatt_side = PanelSeite::Links;
                }
                "panelsrechts" => {
                    self.ui.layers_open = true;
                    self.ui.props_open = true;
                    self.ui.layers_collapsed = false;
                    self.ui.props_collapsed = false;
                    self.ui.layers_side = PanelSeite::Rechts;
                    self.ui.props_side = PanelSeite::Rechts;
                }
                // Zoomstufe in Prozent (`zoom:600`) — Prüfbilder der Schrift im
                // hohen Zoom (T0114).
                s if s.starts_with("zoom:") => {
                    if let Ok(prozent) = s[5..].trim().parse::<f64>() {
                        let mut v = self.current_view();
                        v.zoom = (prozent / 100.0).clamp(1e-5, self.zoom_max());
                        self.set_view(v);
                        // Die ersten Bilder passen sonst selbst ein und
                        // überschreiben die Zoomstufe.
                        self.started_at = 3.0;
                    }
                }
                // T0129: Prüfzustand der Sicherung — drei Stände anlegen und die
                // Einstellungsseite „Sicherung“ öffnen. Für Prüfbilder zeigt
                // `JJCAD_DATA_DIR` auf ein Wegwerf-Verzeichnis.
                "sicherung" => {
                    let einst = self.prefs.sicherung_einstellungen();
                    if self.sicherung.is_none() {
                        self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                            &self.prefs.sicherungen_dir(),
                        ));
                    }
                    if let Some(mut pool) = self.sicherung.take() {
                        for i in 0..3 {
                            self.doc_mut().name = format!("Wohnhaus {}", i + 1);
                            let _ = pool.sichern(&mut self.doc_mut(), &einst);
                        }
                        self.sicherung = Some(pool);
                    }
                    self.ui.settings_open = true;
                    self.ui.settings_kat = crate::dialogs::EINSTELLUNGSKATEGORIEN
                        .iter()
                        .position(|k| k.key == "settings.category_sicherung")
                        .unwrap_or(0);
                }
                // T0223: Prüfzustand des Fensters „Wiederherstellen“ — zwei
                // Zeichnungen mit Verlauf im Sicherungsordner, die erste
                // gewählt und ein mittlerer Stand in der Vorschau. Für
                // Prüfbilder zeigt `JJCAD_DATA_DIR` auf ein Wegwerf-Verzeichnis.
                "wiederherstellen" | "restore" => {
                    let einst = self.prefs.sicherung_einstellungen();
                    if self.sicherung.is_none() {
                        self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                            &self.prefs.sicherungen_dir(),
                        ));
                    }
                    if let Some(mut pool) = self.sicherung.take() {
                        // Ein Zimmer aus vier Wänden, dann je Stand eine
                        // Trennwand dazu — die Vorschau zeigt damit einen
                        // erkennbaren Grundriss, der von Stand zu Stand wächst.
                        let raum = [
                            ([0.0, 0.0], [5200.0, 0.0]),
                            ([5200.0, 0.0], [5200.0, 3800.0]),
                            ([5200.0, 3800.0], [0.0, 3800.0]),
                            ([0.0, 3800.0], [0.0, 0.0]),
                        ];
                        self.doc_mut().name = "Wohnhaus".into();
                        for (a, b) in raum {
                            let _ = self.session_mut().execute(
                                "draw.wall",
                                &serde_json::json!({
                                    "a": a, "b": b, "thickness": 0.24
                                }),
                            );
                        }
                        for i in 0..8 {
                            let _ = self.session_mut().execute(
                                "draw.wall",
                                &serde_json::json!({
                                    "a": [0.0, 900.0 * (i + 1) as f64],
                                    "b": [5200.0, 900.0 * (i + 1) as f64],
                                    "thickness": 0.12
                                }),
                            );
                            let _ = pool.sichern(&mut self.doc_mut(), &einst);
                        }
                        // Zweite Zeichnung: eine Garage mit eigener Kennung.
                        let mut zweite = jjcad_core::model::Document::new();
                        zweite.name = "Garage".into();
                        zweite.id = "d2e0c0de-0000-4000-8000-00000000c0de".into();
                        for (a, b) in [
                            ([0.0, 0.0], [3000.0, 0.0]),
                            ([3000.0, 0.0], [3000.0, 2000.0]),
                            ([3000.0, 2000.0], [0.0, 2000.0]),
                            ([0.0, 2000.0], [0.0, 0.0]),
                        ] {
                            let _ = jjcad_core::actions::execute(
                                &mut zweite,
                                "draw.wall",
                                &serde_json::json!({
                                    "a": a, "b": b, "thickness": 0.24
                                }),
                            );
                        }
                        for i in 0..4 {
                            let _ = jjcad_core::actions::execute(
                                &mut zweite,
                                "draw.wall",
                                &serde_json::json!({
                                    "a": [0.0, 500.0 * (i + 1) as f64],
                                    "b": [3000.0, 500.0 * (i + 1) as f64],
                                    "thickness": 0.12
                                }),
                            );
                            let _ = pool.sichern(&mut zweite, &einst);
                        }
                        self.sicherung = Some(pool);
                    }
                    self.oeffne_wiederherstellen();
                    let dateien = self.wiederherstellen_dateien();
                    if let Some(w) = self.ui.wiederherstellen.as_mut() {
                        w.datei = dateien.first().map(|d| d.datei.clone());
                    }
                    // Mittlerer Stand der gewählten Zeichnung als Vorschau.
                    if let Some(id) = self.ui.wiederherstellen.as_ref().and_then(|w| w.datei.clone())
                    {
                        let zyklen: Vec<u64> = self
                            .sicherung_pool()
                            .staende_chronologisch(&id)
                            .iter()
                            .map(|s| s.zyklus)
                            .collect();
                        if let Some(z) = zyklen.get(zyklen.len() / 2).copied() {
                            if let Some(w) = self.ui.wiederherstellen.as_mut() {
                                w.zyklus = Some(z);
                            }
                        }
                    }
                    self.wiederherstellen_vorschau_nachziehen();
                }
                // R2.3: Prüfzustand des Konfliktdialogs. Der Reiter trägt keine
                // echten Dateien — geprüft wird die Darstellung des Fensters.
                "konflikt" => {
                    self.ui.konflikt = Some(Konflikt {
                        reiter: self.active,
                        datei: self.doc().id.clone(),
                        pfad: self.doc().path.clone().unwrap_or_default(),
                        zyklus: 42,
                        zeit: "2026-11-16T09:15:00".into(),
                        name: self.doc().name.clone(),
                        neu: false,
                    });
                }
                // T0202: Layer-Manager mit Plänen — zwei weitere Pläne, der
                // erste eingeklappt, im zweiten eine Wand.
                "plaene" | "layerplaene" => {
                    // T0232: Jeder Plan trägt seinen eigenen Maßstab — die
                    // Prüfzeichnung zeigt je Plan eine Wand derselben Stärke,
                    // damit die Strichbreite vergleichbar ist.
                    for (name, massstab) in [("Erdgeschoss", 200.0), ("Obergeschoss", 50.0)] {
                        let _ = self.session_mut().execute(
                            "plan.create",
                            &serde_json::json!({"name": name, "scale": massstab}),
                        );
                    }
                    for i in 0..self.doc().plans.len() {
                        let pid = self.doc().plans[i].id;
                        let layer = self
                            .doc()
                            .layers_of_plan(pid)
                            .iter()
                            .find(|l| l.name == "Architektur")
                            .map(|l| l.id);
                        if let Some(layer) = layer {
                            // Modell in Metern — 5 m Wand je Plan, 0,8 m Abstand.
                            let y = i as f64 * 0.8;
                            let _ = self.session_mut().execute(
                                "draw.wall",
                                &serde_json::json!({
                                    "a": [0.0, y], "b": [5.0, y],
                                    "thickness": 0.24, "layer": layer
                                }),
                            );
                        }
                    }
                    // Maßstabsgetreue Strichstärken zeigen den Unterschied.
                    self.ui.lineweights_scaled = true;
                    if let Some(plan) = self.doc().plans.first() {
                        let pid = plan.id;
                        let _ = self.session_mut().execute(
                            "plan.update",
                            &serde_json::json!({"plan": pid, "collapsed": true}),
                        );
                    }
                    self.ui.layers_open = true;
                    self.ui.layers_collapsed = false;
                }
                // T0202: ein Plan ausgeblendet, einer gesperrt — Prüfbild der
                // Plan-Schalter (Auge/Schloss in der Kopfzeile).
                "planaus" | "plangesperrt" => {
                    for name in ["Erdgeschoss", "Obergeschoss"] {
                        let _ = self
                            .session_mut()
                            .execute("plan.create", &serde_json::json!({"name": name}));
                    }
                    let ids: Vec<u64> = self.doc().plans.iter().map(|p| p.id).collect();
                    if let Some(p) = ids.get(1) {
                        let _ = self.session_mut().execute(
                            "plan.update",
                            &serde_json::json!({"plan": p, "locked": true}),
                        );
                    }
                    if let Some(p) = ids.get(2) {
                        let _ = self.session_mut().execute(
                            "plan.update",
                            &serde_json::json!({"plan": p, "visible": false}),
                        );
                    }
                    if let Some(p) = ids.first() {
                        let _ = self.session_mut().execute(
                            "plan.update",
                            &serde_json::json!({"plan": p, "collapsed": true}),
                        );
                    }
                    self.ui.layers_open = true;
                    self.ui.layers_collapsed = false;
                }
                "vollbild" | "fullscreen" => self.ui.fullscreen = true,
                "aendern" | "change" => self.ui.ribbon = RibbonTab::Aendern,
                "inv" | "inventar" => self.ui.inventory_open = true,
                // Mehrere Dateien: drei Reiter mit Namen (Screenshot-Prüfung T0073)
                "dateien" | "mehrere" => {
                    self.new_document();
                    self.doc_mut().name = "Schnitt".into();
                    // Eine Linie macht den Reiter „ungespeichert“ (Änderungspunkt).
                    self.session_mut()
                        .execute(
                            "draw.line",
                            &serde_json::json!({"a": [0.0, 0.0], "b": [4000.0, 2000.0]}),
                        )
                        .ok();
                    self.new_document();
                    self.doc_mut().name = "Ansicht".into();
                    self.active = 0;
                    self.datei_nachziehen();
                    self.fit_view();
                }
                // Datei schwebt über dem Fenster (Drag an Drop, T0115):
                // der Hinweis samt Ziel des Imports. `dateidropfalsch` zeigt
                // den Fall eines nicht importierbaren Formats.
                s if s.starts_with("dateidrop") => {
                    let name = if s.ends_with("falsch") {
                        "Notizen.txt"
                    } else {
                        "samples/Wohnhaus.jjcad"
                    };
                    self.ui.drop_demo = Some(vec![PathBuf::from(name)]);
                }
                // T0213: Rückfrage beim Löschen eines Layers mit Objekten —
                // Prüfbild des Popups samt Zielauswahl.
                "layerloeschen" | "layerdialog" => {
                    self.ui.layers_open = true;
                    self.ui.layers_collapsed = false;
                    let quelle = self
                        .doc()
                        .layer_id_by_name("Zeichnung")
                        .unwrap_or(self.doc().current_layer);
                    let _ = self.session_mut().execute(
                        "draw.line",
                        &serde_json::json!({
                            "a": [0.0, 0.0],
                            "b": [2000.0, 0.0],
                            "layer": quelle,
                        }),
                    );
                    if let Some(ziel) = self
                        .doc()
                        .layers_of_plan(
                            self.doc().layer(quelle).map(|l| l.plan).unwrap_or(0),
                        )
                        .into_iter()
                        .find(|l| l.id != quelle)
                        .map(|l| l.id)
                    {
                        self.ui.layer_delete = Some((quelle, ziel));
                    }
                }
                "schliessdialog" => {
                    self.new_document();
                    self.session_mut()
                        .execute(
                            "draw.line",
                            &serde_json::json!({"a": [0.0, 0.0], "b": [0.0, 1000.0]}),
                        )
                        .ok();
                    self.ui.confirm_close = Some(1);
                }
                // T0137: Blockeditor als Prüfbild — je ein Block der Datei und
                // des Benutzers (nur im Speicher, keine Datei). Beide Reiter
                // stehen im Streifen: der Dateiblock in der Blattzone, der
                // Benutzerblock in der eigenen Gruppe.
                "blockeditor" | "blockedit" => {
                    let mut anlegen = |app: &mut App, id: &str, name: &str, quelle: jjcad_core::model::BlockSource| {
                        if app.doc().block(id).is_some() {
                            return;
                        }
                        let layer = app.doc().current_layer;
                        let mut ents = Vec::new();
                        let mut rahmen = Entity::new(
                            1,
                            layer,
                            EntityKind::Polyline {
                                verts: vec![
                                    (-900.0, -600.0),
                                    (900.0, -600.0),
                                    (900.0, 600.0),
                                    (-900.0, 600.0),
                                ]
                                .into_iter()
                                .map(|(x, y)| jjcad_core::geom::Vertex::new(jjcad_core::geom::v(x, y)))
                                .collect(),
                                closed: true,
                            },
                        );
                        rahmen.by_block = true;
                        ents.push(rahmen);
                        let mut kreis = Entity::new(
                            2,
                            layer,
                            EntityKind::Circle {
                                c: jjcad_core::geom::v(0.0, 250.0),
                                r: 300.0,
                            },
                        );
                        kreis.by_block = true;
                        ents.push(kreis);
                        let mut linie = Entity::new(
                            3,
                            layer,
                            EntityKind::Line {
                                a: jjcad_core::geom::v(-900.0, -600.0),
                                b: jjcad_core::geom::v(900.0, 600.0),
                            },
                        );
                        linie.by_block = true;
                        ents.push(linie);
                        app.doc_mut().blocks.push(jjcad_core::model::BlockDef {
                            id: id.to_string(),
                            name: name.to_string(),
                            category: "Prüfblocks".to_string(),
                            source: quelle,
                            description: jjcad_core::i18n::tr("gui.app.pruefblock").to_string(),
                            entities: ents,
                            icon: None,
                            base: V2::ZERO,
                            groesse: BlockGroesse::Fest,
                        });
                    };
                    anlegen(
                        self,
                        "prj:demo-block",
                        "Prüfblock Datei",
                        jjcad_core::model::BlockSource::Project,
                    );
                    anlegen(
                        self,
                        "usr:demo-block",
                        "Prüfblock Benutzer",
                        jjcad_core::model::BlockSource::User,
                    );
                    self.oeffne_block_editor("prj:demo-block");
                    self.oeffne_block_editor("usr:demo-block");
                    let mut v = self.current_view();
                    v.center = V2::new(0.0, 100.0);
                    v.zoom = 0.42;
                    self.set_view(v);
                    // Die ersten Bilder würden sonst selbst einpassen.
                    self.started_at = 3.0;
                }
                // T0231: Punktsymbol im Blockeditor — die Geometrie ist auf
                // Einheitsgröße 1 normiert, der Block trägt keine Maßeinheit
                // und erscheint deshalb in Zeichnungsgröße (Maßstab 1:100).
                "punktsymbolblock" | "punktsymboledit" => {
                    let id = "prj:pkt-pfeil-demo";
                    if self.doc().block(id).is_none() {
                        if let Some(vorlage) = self.doc().block("sys:pkt-pfeil").cloned() {
                            let mut b = vorlage;
                            b.id = id.to_string();
                            b.source = jjcad_core::model::BlockSource::Project;
                            b.name = "Pfeil (Block)".to_string();
                            self.doc_mut().blocks.push(b);
                        }
                    }
                    self.oeffne_block_editor(id);
                }
                // T0185: Rechtsklickmenü einer eingesetzten Blockinstanz —
                // zeigt den Eintrag „Block bearbeiten“ (nur eigene Blöcke).
                "blockmenue" => {
                    if let Some(id) = self.demo_benutzerblock() {
                        let mut v = self.current_view();
                        v.center = V2::ZERO;
                        v.zoom = 0.35;
                        // `set_view` schließt ein offenes Menü — erst danach öffnen.
                        self.set_view(v);
                        self.open_menu(MenuKind::Object(id), egui::Pos2::new(420.0, 260.0));
                        // Sonst passt die Ansicht in den ersten Bildern ein und
                        // schließt das Menü (T0180).
                        self.started_at = 3.0;
                    }
                }
                // T0204: Ausgewählter Block — das Eigenschaften-Panel zeigt
                // Name und Gruppe des Blocks.
                "blockpanel" => {
                    if let Some(id) = self.demo_benutzerblock() {
                        self.session_mut().selection = vec![id];
                        self.ui.props_open = true;
                    }
                }
                // T0204: Fenster „Block-Eigenschaften“ über der Bibliothek.
                "blockeigenschaften" => {
                    self.demo_benutzerblock();
                    self.ui.inventory_open = true;
                    self.ui.inventory_tab = 1;
                    self.ui.block_quelle = 1;
                    self.oeffne_block_eigenschaften("usr:tisch");
                }
                // T0219: Rückfrage beim Löschen eines Blocks, der noch
                // eingesetzt ist — Hinweis samt Angebot, die Instanzen zu
                // zerlegen. Prüfbild der Rückfrage.
                "blockloeschen" | "blockdelete" => {
                    self.demo_benutzerblock();
                    // Zwei Instanzen machen den Hinweis anschaulich — die
                    // Vorlage kann schon eine mitbringen (`demo_benutzerblock`).
                    let layer = self.doc().current_layer;
                    let vorhanden = self.doc().block_verwendungen("usr:tisch").len();
                    for i in vorhanden..2 {
                        let id = self.doc_mut().new_id();
                        self.doc_mut().add_entity(Entity::new(
                            id,
                            layer,
                            EntityKind::BlockRef {
                                block: "usr:tisch".into(),
                                pos: V2::new((i as f64 + 1.0) * 2500.0, 0.0),
                                rot: 0.0,
                                scale: V2::new(1.0, 1.0),
                                mirror: false,
                            },
                        ));
                    }
                    self.ui.inventory_open = true;
                    self.ui.inventory_tab = 1;
                    self.ui.block_quelle = 1;
                    self.block_loeschen_anfragen("usr:tisch");
                }
                "blocks-tab" => {
                    self.ui.inventory_open = true;
                    self.ui.inventory_tab = 1;
                }
                // T0188: Prüfbild der drei Unterkategorien der Blockbibliothek —
                // `blockreiter:benutzer` bzw. `blockreiter:projekt` öffnet den
                // Reiter „Blöcke“ gleich auf der genannten Quelle.
                s if s.starts_with("blockreiter:") => {
                    self.ui.inventory_open = true;
                    self.ui.inventory_tab = 1;
                    self.ui.block_quelle = match s["blockreiter:".len()..].trim() {
                        "benutzer" => 1,
                        "projekt" => 2,
                        _ => 0,
                    };
                }
                "functions" => {
                    self.ui.inventory_open = true;
                    self.ui.inventory_tab = 2;
                }
                "keymap" => self.ui.keymap_open = true,
                // Leistungsanzeige offen (Screenshot-Prüfung T0068)
                "perf" | "leistung" => self.perf.an = true,
                // Culling-Prüfbetrieb (Screenshot-Prüfung T0167): Anzeige offen
                // und der Bereich um ein Viertel kleiner.
                "culling" => {
                    self.perf.an = true;
                    self.perf.culling_verkleinern = true;
                }
                // Chat-Protokoll mit Beispielmeldungen (Screenshot-Prüfung)
                "chat" => {
                    for t in [
                        "Polylinie: Startpunkt klicken",
                        jjcad_core::i18n::tr("gui.app.demo_naechster_punkt_klicken"),
                        "Polylinie abgeschlossen",
                        "Kreis: Mittelpunkt klicken",
                    ] {
                        self.chat.sagen(0.0, t);
                    }
                    self.chat.offen = true;
                }
                // Langes Protokoll (Blättern/Screenshot-Prüfung)
                "chatlang" => {
                    for i in 1..=40 {
                        self.chat.sagen(0.0, jjcad_core::i18n::t_args_owned("gui.app.meldung_x_polylinie_naechster_punkt_klicken", &[("i", format!("{}", i))]));
                    }
                    self.chat.offen = true;
                }
                // Hinweis im geschlossenen Chat (verblassende Meldung)
                "chathinweis" => {
                    self.start_tool("draw.polyline");
                    self.chat.sagen(0.0, jjcad_core::i18n::tr("gui.app.polylinie_startpunkt_klicken_chat"));
                }
                // Seitenwahl „PDF einfügen" offen (Screenshot-Prüfung)
                "pdfseite" => {
                    let p = [
                        "samples/Grundriss.pdf",
                        "../../samples/Grundriss.pdf",
                    ]
                    .iter()
                    .map(std::path::PathBuf::from)
                    .find(|p| p.exists());
                    if let Some(pfad) = p {
                        self.pdf_import_oeffnen(pfad);
                    }
                }
                // Text mit gemischter Auszeichnung (Screenshot-Prüfung der
                // Textformatierung: fett, kursiv, unterstrichen, hochgestellt)
                "textruns" => {
                    let id = self.doc_mut().new_id();
                    let layer = self.doc().current_layer;
                    let runs = vec![
                        jjcad_core::model::TextRun {
                            text: "Raum ".into(),
                            ..Default::default()
                        },
                        jjcad_core::model::TextRun {
                            text: "Wohnen".into(),
                            bold: true,
                            ..Default::default()
                        },
                        jjcad_core::model::TextRun {
                            text: " · ".into(),
                            ..Default::default()
                        },
                        jjcad_core::model::TextRun {
                            text: jjcad_core::i18n::tr("gui.app.demo_kueche").into(),
                            italic: true,
                            ..Default::default()
                        },
                        jjcad_core::model::TextRun {
                            text: " · ".into(),
                            ..Default::default()
                        },
                        jjcad_core::model::TextRun {
                            text: "32,4".into(),
                            underline: true,
                            ..Default::default()
                        },
                        jjcad_core::model::TextRun {
                            text: " m".into(),
                            ..Default::default()
                        },
                        jjcad_core::model::TextRun {
                            text: "2".into(),
                            superscript: true,
                            ..Default::default()
                        },
                    ];
                    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                    let mut e = Entity::new(
                        id,
                        layer,
                        EntityKind::Text {
                            p: jjcad_core::geom::v(2000.0, 3000.0),
                            text,
                            height: 300.0,
                            angle: 0.0,
                            align: TextAlignH::Left,
                            valign: TextAlignV::Bottom,
                            bold: false,
                            italic: false,
                            underline: false,
                            superscript: false,
                            runs,
                            font: TextFont::default(),
                            line_spacing: 1.0,
                            frame: false,
                            frame_round: false,
                            background: None,
                        },
                    );
                    e.style.color = Some(jjcad_core::style::Color::BLACK);
                    self.doc_mut().add_entity(e);
                }
                // Textfelder mit Umrandung und Hintergrund (T0126):
                // eckig, rund (Oval) und mit Fläche — Prüfbild der drei Formen.
                "textrahmen" => {
                    let layer = self.doc().current_layer;
                    let formen = [
                        ("Umrandung", false, None),
                        ("Runde Umrandung", true, None),
                        ("Hintergrund", false, Some(jjcad_core::style::Color::rgb(255, 240, 160))),
                        // Mehrzeilig: Viertelkreise mit dem Radius einer Zeile
                        // statt eines Ovals über den ganzen Block (T0126, Turn 7).
                        ("Zweizeilig\nrunde Ecken", true, None),
                    ];
                    for (i, (txt, rund, flaeche)) in formen.iter().enumerate() {
                        let id = self.doc_mut().new_id();
                        let mut e = Entity::new(
                            id,
                            layer,
                            EntityKind::Text {
                                p: jjcad_core::geom::v(1200.0, 3000.0 - i as f64 * 800.0),
                                text: (*txt).into(),
                                height: 250.0,
                                angle: 0.0,
                                align: TextAlignH::Left,
                                valign: TextAlignV::Bottom,
                                bold: false,
                                italic: false,
                                underline: false,
                                superscript: false,
                                runs: Vec::new(),
                                font: TextFont::default(),
                                line_spacing: 1.0,
                                frame: true,
                                frame_round: *rund,
                                background: *flaeche,
                            },
                        );
                        e.style.color = Some(jjcad_core::style::Color::BLACK);
                        self.doc_mut().add_entity(e);
                    }
                    self.fit_view();
                }
                // Ausgewählter Text mit gemischter Auszeichnung, Panel sichtbar
                // (Screenshot-Prüfung der Textbearbeitung, T0032)
                "textpanel" => {
                    let id = self.doc_mut().new_id();
                    let layer = self.doc().current_layer;
                    let runs = vec![
                        jjcad_core::model::TextRun::plain("Raum "),
                        jjcad_core::model::TextRun {
                            text: "Wohnen".into(),
                            bold: true,
                            ..Default::default()
                        },
                    ];
                    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                    let mut e = Entity::new(
                        id,
                        layer,
                        EntityKind::Text {
                            p: jjcad_core::geom::v(1000.0, 1000.0),
                            text,
                            height: 250.0,
                            angle: 0.0,
                            align: TextAlignH::Left,
                            valign: TextAlignV::Bottom,
                            bold: false,
                            italic: false,
                            underline: false,
                            superscript: false,
                            runs,
                            font: TextFont::default(),
                            line_spacing: 1.0,
                            frame: false,
                            frame_round: false,
                            background: None,
                        },
                    );
                    e.style.color = Some(jjcad_core::style::Color::BLACK);
                    self.doc_mut().add_entity(e);
                    self.ui.props_open = true;
                    self.session_mut().select(vec![id]);
                    self.fit_view();
                }
                // Prüfhilfe T0034: je eine Zeile pro Schriftart, dazu ein
                // zerlegter Text (Umrisse als Linien) — Screenshot-Prüfung.
                "schriften" => {
                    let layer = self.doc().current_layer;
                    for (i, font) in jjcad_core::model::TextFont::ALL.iter().enumerate() {
                        let id = self.doc_mut().new_id();
                        let mut e = Entity::new(
                            id,
                            layer,
                            EntityKind::Text {
                                p: jjcad_core::geom::v(2000.0, 4900.0 - i as f64 * 430.0),
                                text: format!("{} · Wohnen 12,4 m²", font.text()),
                                height: 250.0,
                                angle: 0.0,
                                align: TextAlignH::Left,
                                valign: TextAlignV::Bottom,
                                bold: false,
                                italic: false,
                                underline: false,
                                superscript: false,
                                runs: Vec::new(),
                                font: *font,
                                line_spacing: 1.0,
                                frame: false,
                                frame_round: false,
                                background: None,
                            },
                        );
                        e.style.color = Some(jjcad_core::style::Color::BLACK);
                        self.doc_mut().add_entity(e);
                    }
                    // Ein Text, der schon in Linien zerlegt ist (Serifen).
                    let id = self.doc_mut().new_id();
                    let mut e = Entity::new(
                        id,
                        layer,
                        EntityKind::Text {
                            p: jjcad_core::geom::v(2000.0, 400.0),
                            text: "Zerlegt".into(),
                            height: 400.0,
                            angle: 0.0,
                            align: TextAlignH::Left,
                            valign: TextAlignV::Bottom,
                            bold: false,
                            italic: false,
                            underline: false,
                            superscript: false,
                            runs: Vec::new(),
                            font: jjcad_core::model::TextFont::Serif,
                            line_spacing: 1.0,
                            frame: false,
                            frame_round: false,
                            background: None,
                        },
                    );
                    e.style.color = Some(jjcad_core::style::Color::BLACK);
                    self.doc_mut().add_entity(e);
                    let _ = self
                        .session_mut()
                        .execute("modify.explode", &serde_json::json!({"ids": [id]}));
                    self.fit_view();
                }
                // Offener Systemdialog (nur der Zustand, kein echter Dialog):
                // zeigt Abdunklung und Hinweis für Screenshots.
                "dateidialog" => {
                    self.ui.systemdialog = Some((
                        FileMode::Open,
                        Systemdialog::mit_ausfuehrung(|| {
                            std::thread::sleep(std::time::Duration::from_secs(120));
                            None
                        }),
                    ));
                }
                // Prüffläche: eine Füllfläche über der Zeichnung, damit das
                // Eigenschaften-Panel der Fläche prüfbar ist (mit `flaechenpanel`
                // wird sie ausgewählt).
                "flaeche" => {
                    let layer = self.doc().current_layer;
                    let id = self.doc_mut().new_id();
                    let e = Entity::new(
                        id,
                        layer,
                        EntityKind::Fill {
                            boundary: vec![
                                jjcad_core::geom::v(2000.0, 1000.0),
                                jjcad_core::geom::v(4000.0, 1000.0),
                                jjcad_core::geom::v(4000.0, 3000.0),
                                jjcad_core::geom::v(2000.0, 3000.0),
                            ],
                            holes: Vec::new(),
                            pattern: jjcad_core::style::FillPattern::Lines,
                            pattern_angle: std::f64::consts::FRAC_PI_4,
                            pattern_scale: 100.0,
                            border: true,
                        },
                    );
                    self.doc_mut().add_entity(e);
                }
                // Text an Ort und Stelle bearbeiten (Panel-Prüfung: derselbe
                // Aufbau wie bei Auswahl, nicht das frühere Kurzpanel).
                "textbearbeiten" => {
                    self.ui.props_open = true;
                    let id = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Text { .. }))
                        .map(|e| e.id);
                    match id {
                        Some(id) => {
                            crate::textedit::start_entitaet(self, id);
                        }
                        None => {
                            let p = jjcad_core::geom::v(2000.0, 2000.0);
                            let layer = self.doc().current_layer;
                            let id = self.doc_mut().new_id();
                            let e = Entity::new(
                                id,
                                layer,
                                EntityKind::Text {
                                    p,
                                    text: "Wohnen".into(),
                                    height: 250.0,
                                    angle: 0.0,
                                    align: TextAlignH::Left,
                                    valign: TextAlignV::Bottom,
                                    bold: false,
                                    italic: false,
                                    underline: false,
                                    superscript: false,
                                    runs: Vec::new(),
                                    font: jjcad_core::model::TextFont::Sans,
                                    line_spacing: 1.0,
                                    frame: false,
                                    frame_round: false,
                                    background: None,
                                },
                            );
                            self.doc_mut().add_entity(e);
                            crate::textedit::start_entitaet(self, id);
                        }
                    }
                }
                // Prüfmaß: ein Maß über der Zeichnung (mit `dimpanel` auswählen).
                "mass" => {
                    let _ = self.session_mut().execute(
                        "dim.linear",
                        &serde_json::json!({"a": [0.0, 0.0], "b": [3.0, 0.0],
                                            "line_pos": [1.5, -0.6]}),
                    );
                }
                // Prüfbild der Höhenkoten (T0096): Grundriss-Kote mit
                // Kreis-Symbol und Schnitt-Kote mit Dreiecken. Die Höhen der
                // Schnitt-Kote folgen aus der Y-Differenz zum ersten Punkt.
                "kote" | "hoehenkoten" => {
                    let u = self.unit();
                    let ax = |x: f64, y: f64| [u.from_mm(x), u.from_mm(y)];
                    let _ = self.session_mut().execute(
                        "dim.elevation.plan",
                        &serde_json::json!({"p": ax(600.0, 800.0), "z": u.from_mm(0.0)}),
                    );
                    let _ = self.session_mut().execute(
                        "dim.elevation.section",
                        &serde_json::json!({"pts": [
                            ax(3000.0, 0.0), ax(4000.0, 1200.0),
                            ax(5000.0, 2600.0), ax(6000.0, 3500.0)
                        ]}),
                    );
                    self.ui.ribbon = RibbonTab::Bemassung;
                }
                // Tangentialfang (T0157): Kreis und Linie, Linienwerkzeug mit
                // erstem Punkt auf der Kreisbahn. Der Zeiger rastet auf der
                // Senkrechten zum Radius ein — Prüfbild mit
                // `x11-drive.py hover` auf diese Tangente.
                // Lotfang (T0216): waagerechte Linie, Linienwerkzeug mit
                // erstem Punkt auf ihr. Der Zeiger rastet auf der Senkrechten
                // ein, das Winkelquadrat steht im Schnittpunkt — Prüfbild mit
                // `x11-drive.py hover` auf diese Senkrechte.
                "lotfang" | "lotrecht" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.new_document();
                    self.session_mut()
                        .execute("doc.settings", &serde_json::json!({"input_unit": "mm"}))
                        .unwrap();
                    let _ = self.session_mut().execute(
                        "draw.line",
                        &serde_json::json!({
                            "a": [0.0, 0.0], "b": [6000.0, 0.0], "layer": "Wände"
                        }),
                    );
                    self.start_tool("draw.line");
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(V2::new(3000.0, 0.0));
                    }
                    let mut v = self.current_view();
                    v.center = V2::new(3000.0, 700.0);
                    v.zoom = 0.12;
                    self.set_view(v);
                    self.started_at = 3.0;
                }
                // Lotfang auf der **Wandkante** (T0216, Turn 3): erster Punkt
                // auf der Längskante einer 240 mm dicken Wand. Der Zeiger
                // rastet auf deren Senkrechter ein — Prüfbild mit
                // `x11-drive.py hover` auf diese Senkrechte.
                "lotwand" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.new_document();
                    self.session_mut()
                        .execute("doc.settings", &serde_json::json!({"input_unit": "mm"}))
                        .unwrap();
                    let _ = self.session_mut().execute(
                        "draw.wall",
                        &serde_json::json!({
                            "a": [0.0, 0.0], "b": [6000.0, 0.0], "thickness": 240.0
                        }),
                    );
                    self.start_tool("draw.line");
                    if let Some(t) = self.tool.as_mut() {
                        // Längskante bei y = +120.
                        t.pts.push(V2::new(3000.0, 120.0));
                    }
                    let mut v = self.current_view();
                    v.center = V2::new(3000.0, 700.0);
                    v.zoom = 0.12;
                    self.set_view(v);
                    self.started_at = 3.0;
                }
                // Lotrecht × Nächster Punkt (T0243): waagerechte Vorlage für das Lot
                // und eine zweite waagerechte Linie als Ziel. Der Zeiger rastet auf
                // der **Kreuzung** beider Bedingungen ein — Prüfbild mit
                // `x11-drive.py hover` auf (3000; 2500).
                "lotkreuzung" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.new_document();
                    self.session_mut()
                        .execute("doc.settings", &serde_json::json!({ "input_unit": "mm" }))
                        .unwrap();
                    let _ = self.session_mut().execute(
                        "draw.line",
                        &serde_json::json!({
                            "a": [0.0, 0.0], "b": [6000.0, 0.0], "layer": "Wände"
                        }),
                    );
                    let _ = self.session_mut().execute(
                        "draw.line",
                        &serde_json::json!({
                            "a": [1000.0, 2500.0], "b": [4500.0, 2500.0], "layer": "Wände"
                        }),
                    );
                    self.start_tool("draw.line");
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(V2::new(3000.0, 0.0));
                    }
                    let mut v = self.current_view();
                    v.center = V2::new(3000.0, 1400.0);
                    v.zoom = 0.09;
                    self.set_view(v);
                    self.started_at = 3.0;
                }
                "tangential" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    // Frische Zeichnung: sie rechnet in Metern, die Punkte des
                    // Werkzeugs stehen dagegen in mm.
                    self.new_document();
                    let _ = self.session_mut().execute(
                        "draw.circle",
                        &serde_json::json!({"c": [0.0, 0.0], "r": 2.0, "layer": "Wände"}),
                    );
                    let _ = self.session_mut().execute(
                        "draw.line",
                        &serde_json::json!({
                            "a": [-3.0, -3.0], "b": [3.0, -3.0], "layer": "Wände"
                        }),
                    );
                    self.start_tool("draw.line");
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(V2::new(2000.0, 0.0));
                    }
                    let mut v = self.current_view();
                    v.center = V2::new(0.0, 0.0);
                    v.zoom = 0.12;
                    self.set_view(v);
                    self.started_at = 3.0;
                }
                // Schnitt-Kote platzieren: Werkzeug läuft, zwei Punkte sind
                // gesetzt — das Prüfbild zeigt die Vorschau am Zeiger (T0122).
                "koteschnitt" => {
                    self.ui.ribbon = RibbonTab::Bemassung;
                    self.start_tool("dim.elevation.section");
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(V2::new(3000.0, 0.0));
                        t.pts.push(V2::new(4000.0, 1200.0));
                    }
                    // Nahansicht auf die gesetzten Punkte, damit die Vorschau
                    // im Prüfbild lesbar ist.
                    let mut v = self.current_view();
                    v.center = V2::new(3600.0, 900.0);
                    v.zoom = 0.35;
                    self.set_view(v);
                    self.started_at = 3.0;
                }
                // Lage des Höhenwertes an der Grundriss-Kote (T0196): vier
                // Koten nebeneinander, je eine Lage — Prüfbild für Oben,
                // Unten, Links und Rechts.
                "kotepositionen" => {
                    self.ui.ribbon = RibbonTab::Bemassung;
                    self.new_document();
                    let u = self.unit();
                    let ax = |x: f64, y: f64| [u.from_mm(x), u.from_mm(y)];
                    for (i, pos) in ["top", "bottom", "left", "right"].iter().enumerate() {
                        let x = 1000.0 + i as f64 * 1800.0;
                        let _ = self.session_mut().execute(
                            "dim.elevation.plan",
                            &serde_json::json!({
                                "p": ax(x, 0.0),
                                "z": u.from_mm(900.0),
                                "text_pos": pos
                            }),
                        );
                    }
                    let mut v = self.current_view();
                    v.center = V2::new(3700.0, 0.0);
                    // Der Höhenwert trägt den Beschriftungsmaßstab (1:100),
                    // ist also 250 mm hoch — bei 0,08 px/mm passen alle vier
                    // Koten nebeneinander ins Bild.
                    v.zoom = 0.08;
                    self.set_view(v);
                    self.started_at = 3.0;
                }
                // Grundriss-Kote auswählen: Eigenschaften-Panel mit dem Feld
                // „Höhe“.
                "selkoteplan" => {
                    self.ui.props_open = true;
                    let vorhanden = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::ElevationPlan { .. }))
                        .map(|e| e.id);
                    let id = match vorhanden {
                        Some(id) => Some(id),
                        None => {
                            let u = self.unit();
                            self.session_mut()
                                .execute(
                                    "dim.elevation.plan",
                                    &serde_json::json!({
                                        "p": [u.from_mm(1200.0), u.from_mm(2600.0)],
                                        "z": u.from_mm(0.0)
                                    }),
                                )
                                .ok()
                                .and_then(|r| r["created"][0].as_u64())
                        }
                    };
                    if let Some(id) = id {
                        self.session_mut().select(vec![id]);
                    }
                }
                // Schnitt-Kote auswählen: Eigenschaften-Panel mit Punktzahl und
                // den gerechneten Höhen.
                "selkote" => {
                    self.ui.props_open = true;
                    // Vorhandene Schnitt-Kote der Zeichnung wählen (Prüfbild mit
                    // Panel); nur ohne eine solche eine anlegen.
                    let vorhanden = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::ElevationSection { .. }))
                        .map(|e| e.id);
                    let id = match vorhanden {
                        Some(id) => Some(id),
                        None => {
                            let u = self.unit();
                            let ax = |x: f64, y: f64| [u.from_mm(x), u.from_mm(y)];
                            self.session_mut()
                                .execute(
                                    "dim.elevation.section",
                                    &serde_json::json!({"pts": [
                                        ax(3000.0, 0.0), ax(4000.0, 1200.0),
                                        ax(5000.0, 2600.0), ax(6000.0, 3500.0)
                                    ]}),
                                )
                                .ok()
                                .and_then(|r| r["created"][0].as_u64())
                        }
                    };
                    if let Some(id) = id {
                        self.session_mut().select(vec![id]);
                    }
                }
                // Radiusbemaßung (T0252): Kreis mit Maßlinie vom Mittelpunkt,
                // Pfeil am Bogen und der Maßzahl außerhalb — ausgewählt, damit
                // das Panel Radius, Winkel und Maßtext zeigt.
                "radiusmass" | "radien" => {
                    self.new_document();
                    // Der Blattmanager liegt sonst rechts statt des Panels —
                    // `App::new` hat die Einstellungen vorher in `UiState`
                    // kopiert (T0242).
                    self.ui.blatt_open = false;
                    self.ui.props_open = true;
                    self.ui.ribbon = RibbonTab::Bemassung;
                    let kreis = self
                        .session_mut()
                        .execute(
                            "draw.circle",
                            &serde_json::json!({"c": [0.0, 0.0], "r": 1.5}),
                        )
                        .ok()
                        .and_then(|r| r["created"][0].as_u64());
                    if let Some(id) = kreis {
                        let antwort = self.session_mut().execute(
                            "dim.radius",
                            &serde_json::json!({"ids": [id], "p": [1.5, 0.0]}),
                        );
                        if let Some(dim) = antwort.ok().and_then(|r| r["created"][0].as_u64()) {
                            self.session_mut().select(vec![dim]);
                        }
                    }
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = jjcad_core::geom::Vec2::new(1300.0, 200.0);
                    v.zoom = 0.15;
                    self.set_view(v);
                }
                // Radiusbemaßung, Werkzeug läuft: die Vorschau hängt am
                // Zeiger (Prüfbild mit der Maus über dem Kreis).
                "radiusmasswerkzeug" => {
                    self.new_document();
                    let _ = self.session_mut().execute(
                        "draw.circle",
                        &serde_json::json!({"c": [0.0, 0.0], "r": 1.5}),
                    );
                    self.start_tool("dim.radius");
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = jjcad_core::geom::Vec2::ZERO;
                    v.zoom = 0.15;
                    self.set_view(v);
                }
                // Maßkette anlegen und auswählen: Basispunkt-Griffe, Raute an
                // der Maßlinie und der Abstand im Eigenschaften-Panel in einem
                // Bild (T0055).
                "masskette" => {
                    // Die Id kommt aus der Antwort — `entities.last()` ist seit
                    // dem Zeichenrang nicht mehr das zuletzt Erzeugte (T0048).
                    let antwort = self.session_mut().execute(
                        "dim.chain",
                        &serde_json::json!({"pts": [[0.0, -1.2], [2.0, -1.2],
                                                    [3.6, -1.2], [6.0, -1.2]],
                                            "line_pos": [3.0, -2.0], "axis": "x"}),
                    );
                    self.ui.props_open = true;
                    if let Some(id) = antwort
                        .ok()
                        .and_then(|r| r["created"][0].as_u64())
                    {
                        self.session_mut().select(vec![id]);
                    }
                }
                // Maßkette im Modus „Waagerecht“: das Werkzeug läuft und
                // zeigt die exemplarische Kette an der Lage der Maßlinie
                // (T0154). Der Zeiger steht in der Bildmitte.
                "masskettemodus" | "masskettewaagerecht" => {
                    self.ui.ribbon = RibbonTab::Bemassung;
                    self.waehle_dim_modus(DimModus::Waagerecht);
                    self.started_at = 3.0;
                }
                // Richtung während der Eingabe drehen (T0250): das Werkzeug
                // läuft waagerecht, „Drehen“ schaltet auf senkrecht — die
                // Vorschau zeigt die Kette in der neuen Richtung.
                "masskettewechsel" => {
                    self.ui.ribbon = RibbonTab::Bemassung;
                    self.waehle_dim_modus(DimModus::Waagerecht);
                    self.start_tool("modify.rotate");
                    self.started_at = 3.0;
                }
                // Maßkette „Direkt“: zwei Messpunkte stehen, der nächste Klick
                // setzt die Maßlinie parallel — die Vorschau hängt am Zeiger
                // (Bildmitte).
                "masskettestellung" => {
                    self.ui.ribbon = RibbonTab::Bemassung;
                    self.waehle_dim_modus(DimModus::Direkt);
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(jjcad_core::geom::Vec2::new(0.0, 0.0));
                        t.pts.push(jjcad_core::geom::Vec2::new(3000.0, 2000.0));
                    }
                    self.started_at = 3.0;
                }
                // Bestehende Maßkette auswählen: das Panel zeigt Richtung,
                // Maßtext, Abstand und die Zahl der Messpunkte (T0154).
                "selmasskette" => {
                    self.ui.props_open = true;
                    self.ui.ribbon = RibbonTab::Bemassung;
                    let vorhanden = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Dim { .. }))
                        .map(|e| e.id);
                    let id = match vorhanden {
                        Some(id) => Some(id),
                        None => self
                            .session_mut()
                            .execute(
                                "dim.chain",
                                &serde_json::json!({"pts": [[0.0, -1200.0], [2000.0, -1200.0],
                                                            [3600.0, -1200.0], [6000.0, -1200.0]],
                                                    "line_pos": [3000.0, -2000.0], "axis": "x"}),
                            )
                            .ok()
                            .and_then(|r| r["created"][0].as_u64()),
                    };
                    if let Some(id) = id {
                        self.session_mut().select(vec![id]);
                    }
                }
                // Gedrehte Maßkette (T0166): eine waagerechte Kette wird um
                // 30° gedreht — Maßlinie, Hilfslinien und Maßtext laufen
                // schräg, das Panel zeigt den Winkel.
                "masskettengedreht" => {
                    self.ui.props_open = true;
                    self.ui.ribbon = RibbonTab::Bemassung;
                    // Werte in der Eingabeeinheit der Zeichnung (Meter).
                    let antwort = self.session_mut().execute(
                        "dim.chain",
                        &serde_json::json!({"pts": [[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]],
                                            "line_pos": [2.5, -0.9], "axis": "x"}),
                    );
                    if let Some(id) = antwort.ok().and_then(|r| r["created"][0].as_u64()) {
                        let _ = self.session_mut().execute(
                            "modify.rotate",
                            &serde_json::json!({"ids": [id], "center": [0.0, 0.0], "angle": 30.0}),
                        );
                        self.session_mut().select(vec![id]);
                    }
                }
                // Punkte in falscher Reihenfolge gesetzt: die Kette sortiert
                // sie beim Anlegen (T0154, Turn 3).
                "massketteunsortiert" => {
                    self.ui.props_open = true;
                    self.ui.ribbon = RibbonTab::Bemassung;
                    let antwort = self.session_mut().execute(
                        "dim.chain",
                        &serde_json::json!({"pts": [[0.0, -1.2], [3.6, -1.2],
                                                    [2.0, -1.2], [6.0, -1.2],
                                                    [1.2, -1.2]],
                                            "line_pos": [3.0, -2.0], "axis": "x"}),
                    );
                    if let Some(id) = antwort.ok().and_then(|r| r["created"][0].as_u64()) {
                        self.session_mut().select(vec![id]);
                    }
                }
                // Messpunkt nachträglich zwischen seine Nachbarn gezogen: die
                // Kette sortiert ihn ein, die Maßzahlen gehören zu den neuen
                // Nachbarn (T0195) — 1,00 m / 4,00 m statt 2,00 m / 4,00 m.
                "massketteeinsortiert" => {
                    // Frische Zeichnung: im Bild steht nur die Kette.
                    self.new_document();
                    self.ui.props_open = true;
                    self.ui.ribbon = RibbonTab::Bemassung;
                    let antwort = self.session_mut().execute(
                        "dim.chain",
                        &serde_json::json!({"pts": [[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]],
                                            "line_pos": [2.5, -0.9], "axis": "x"}),
                    );
                    if let Some(id) = antwort.ok().and_then(|r| r["created"][0].as_u64()) {
                        if let Some(e) = self.doc_mut().entity_mut(id) {
                            if let Some(neu) = crate::handles::apply_grip(
                                e,
                                crate::handles::GripKind::Vert(2),
                                jjcad_core::geom::Vec2::new(1000.0, 0.0),
                            ) {
                                *e = neu;
                            }
                        }
                        self.session_mut().select(vec![id]);
                    }
                    // Ansicht auf die Kette stellen (Bildmitte 2,50 m; das
                    // automatische Einpassen der ersten Bilder aushebeln).
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = jjcad_core::geom::Vec2::new(2500.0, -450.0);
                    v.zoom = 0.15;
                    self.set_view(v);
                }
                // Höhenangaben unter einer Maßkette (T0253): die Angaben
                // hängen am Abschnitt und stehen unter der Maßlinie.
                // `hoehenangabewerkzeug` zeigt zusätzlich das offene Textfeld
                // unter dem zweiten Abschnitt.
                "hoehenangabe" | "hoehenangabewerkzeug" => {
                    self.new_document();
                    self.ui.props_open = true;
                    self.ui.ribbon = RibbonTab::Bemassung;
                    let antwort = self.session_mut().execute(
                        "dim.chain",
                        &serde_json::json!({"pts": [[0.0, -1.2], [2.0, -1.2],
                                                    [3.6, -1.2], [6.0, -1.2]],
                                            "line_pos": [3.0, -2.0], "axis": "x"}),
                    );
                    if let Some(id) = antwort.ok().and_then(|r| r["created"][0].as_u64()) {
                        let _ = self.session_mut().execute(
                            "dim.height.set",
                            &serde_json::json!({"ids": [id], "index": 0, "text": "+3,00"}),
                        );
                        let _ = self.session_mut().execute(
                            "dim.height.set",
                            &serde_json::json!({"ids": [id], "index": 2, "text": "+1,20"}),
                        );
                        self.session_mut().select(vec![id]);
                        // Das Werkzeug-Bild zeigt das offene Feld unter dem
                        // zweiten Abschnitt — wie nach dem Klick.
                        if part.trim() == "hoehenangabewerkzeug" {
                            crate::textedit::start_hoehe(self, id, 1);
                        }
                    }
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = jjcad_core::geom::Vec2::new(3000.0, -1400.0);
                    v.zoom = 0.15;
                    self.set_view(v);
                }
                // „Punkt hinzufügen“ mit gewählter Maßkette: der nächste Klick
                // setzt den Messpunkt (zweiter Schritt, T0154 Turn 3).
                "punktwerkzeug" => {
                    self.ui.props_open = true;
                    self.ui.ribbon = RibbonTab::Bemassung;
                    let vorhanden = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Dim { .. }))
                        .map(|e| e.id);
                    if let Some(id) = vorhanden {
                        self.session_mut().select(vec![id]);
                        self.start_tool("dim.point.add");
                        if let Some(t) = self.tool.as_mut() {
                            t.obj = Some(id);
                        }
                    }
                }
                // Raumstempel (T0097): ein Stempel über einem 4 × 3 m großen
                // Raum, ausgewählt — die Ecken der unsichtbaren Fläche und der
                // Textgriff sind damit im Bild.
                "raumstempel" => {
                    let antwort = self.session_mut().execute(
                        "dim.room",
                        &serde_json::json!({"pts": [[-2.0, -1.5], [2.0, -1.5],
                                                    [2.0, 1.5], [-2.0, 1.5]]}),
                    );
                    self.ui.props_open = true;
                    if let Some(id) = antwort
                        .ok()
                        .and_then(|r| r["created"][0].as_u64())
                    {
                        self.session_mut().select(vec![id]);
                    }
                    // Ansicht auf den ganzen Raum stellen: „Einpassen" kennt
                    // nur das Textfeld (die Fläche ist unsichtbar), das Bild
                    // zeigte sonst die Schrift riesig und ohne Eckgriffe.
                    // Das Start-Einpassen der ersten Bilder wird übersprungen.
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = jjcad_core::geom::Vec2::new(0.0, 0.0);
                    v.zoom = 0.15;
                    self.set_view(v);
                }
                // Werkzeug läuft: Punktfolge bis zur dritten Ecke, damit die
                // Aufforderung „Enter setzt den Stempel" im Chat steht.
                "raumstempelwerkzeug" => {
                    self.start_tool("dim.room");
                    if let Some(t) = self.tool.as_mut() {
                        t.pts = vec![
                            jjcad_core::geom::Vec2::new(-2000.0, -1500.0),
                            jjcad_core::geom::Vec2::new(2000.0, -1500.0),
                            jjcad_core::geom::Vec2::new(2000.0, 1500.0),
                        ];
                    }
                }
                // Zwei Ecken gesetzt (T0220): die Vorschau läuft als Polygonzug
                // bis zum Zeiger — wie bei den Füllflächen; Enter/ESC macht aus
                // den zwei Punkten das achsenparallele Rechteck.
                "raumstempelrechteck" => {
                    self.start_tool("dim.room");
                    if let Some(t) = self.tool.as_mut() {
                        t.pts = vec![
                            jjcad_core::geom::Vec2::new(-2000.0, -1500.0),
                            jjcad_core::geom::Vec2::new(2000.0, 1500.0),
                        ];
                    }
                }
                // T0144: Rasterart und -modus im Prüfbild — `raster:isometrisch`,
                // `raster:dimetrie`, `raster:trimetrie`, `raster:fest`.
                s if s.starts_with("raster:") => {
                    let wahl = &s["raster:".len()..];
                    let mut p = serde_json::json!({});
                    match wahl {
                        "isometrisch" | "iso" => p["grid_kind"] = serde_json::json!("isometrisch"),
                        "dimetrie" | "dim" => p["grid_kind"] = serde_json::json!("dimetrie"),
                        "trimetrie" | "tri" => p["grid_kind"] = serde_json::json!("trimetrie"),
                        "fest" => p["grid_mode"] = serde_json::json!("fest"),
                        _ => {}
                    }
                    let _ = self.session_mut().execute("doc.settings", &p);
                }
                "settings" => self.ui.settings_open = true,
                // T0098: Fenster ohne Titelleiste — die drei Fensterknöpfe
                // stehen dann oben rechts in der Kopfzeile (Prüfbild).
                "fensterrahmen" | "titelleiste" => {
                    self.prefs.fensterrahmen = jjcad_core::persist::Fensterrahmen::OhneTitelleiste;
                }
                // Einstellungen mit gewählter Kategorie (`settings:2` oder
                // `settings:darstellung`, T0178) oder mit Suchbegriff
                // (`settings:Fenster`) — Prüfbild für T0061.
                s if s.starts_with("settings:") => {
                    self.ui.settings_open = true;
                    let wahl = &s["settings:".len()..];
                    let schluessel = format!("settings.category_{wahl}");
                    match wahl.parse::<usize>() {
                        Ok(n) => {
                            self.ui.settings_kat =
                                n.min(crate::dialogs::EINSTELLUNGSKATEGORIEN.len() - 1)
                        }
                        Err(_) => {
                            match crate::dialogs::EINSTELLUNGSKATEGORIEN
                                .iter()
                                .position(|k| k.key == schluessel)
                            {
                                Some(i) => self.ui.settings_kat = i,
                                None => self.ui.settings_suche = wahl.to_string(),
                            }
                        }
                    }
                }
                "help" => self.ui.help_open = true,
                // Über-Dialog mit Markenzeichen (Screenshot-Prüfung T0083)
                "ueber" | "about" => self.ui.about_open = true,
                // Erweiterter Punktfang sichtbar machen: Werkzeug läuft, ein
                // Wandende ist gehalten (Verweilen vorgetäuscht) — die
                // Hilfslinien stehen dann im Screenshot.
                "halte" | "hilfslinie" => {
                    self.start_tool("draw.line");
                    let ecke = self.doc().entities.iter().find_map(|e| match &e.kind {
                        EntityKind::Wall { b, .. } => Some(*b),
                        _ => None,
                    });
                    if let Some(p) = ecke {
                        if let Some(hit) = jjcad_core::snap::find_snap(
                            self.doc(),
                            p,
                            60.0,
                            None,
                            &[],
                        ) {
                            self.halt.push(jjcad_core::snap::HaltPunkt::neu(self.doc(), &hit));
                        }
                    }
                }
                // Winkelhalbierender Fang (T0150): eine neue Linie beginnt im
                // **Schnittpunkt zweier Linien** — von diesem Scheitel geht die
                // Halbierende als Führungslinie aus. Gesucht wird der erste
                // Schnittpunkt, der auf beiden Strecken liegt (echte Ecke).
                "halbierende" | "winkelhalbierende" | "halbierendehalte"
                | "winkelhalbierendehalte" | "halbierendezwischen"
                | "winkelhalbierendezwischen" => {
                    let kanten: Vec<(V2, V2)> = self
                        .doc()
                        .ents()
                        .iter()
                        .filter_map(|e| match &e.kind {
                            EntityKind::Line { a, b } => Some((*a, *b)),
                            EntityKind::Wall { a, b, .. } => Some((*a, *b)),
                            _ => None,
                        })
                        .collect();
                    let scheitel = (0..kanten.len())
                        .flat_map(|i| (i + 1..kanten.len()).map(move |j| (i, j)))
                        .find_map(|(i, j)| {
                            let (a1, b1) = kanten[i];
                            let (a2, b2) = kanten[j];
                            let p = jjcad_core::geom::line_line(a1, b1, a2, b2, true, true)?;
                            let grenze = jjcad_core::snap::WINKELHALBIERENDE_SCHEITEL_MM;
                            (jjcad_core::geom::dist_point_segment(p, a1, b1) <= grenze
                                && jjcad_core::geom::dist_point_segment(p, a2, b2) <= grenze)
                                .then_some(p)
                        });
                    // Gegenprobe zum Prüfen (T0150): ein Punkt *zwischen* den
                    // Linien ist kein Scheitel — dort darf keine Halbierende
                    // erscheinen. Zustand: `halbierendezwischen`.
                    let punkt = if part.contains("zwischen") && kanten.len() >= 2 {
                        let mitte = |(a, b): (V2, V2)| (a + b) * 0.5;
                        Some((mitte(kanten[0]) + mitte(kanten[1])) * 0.5)
                    } else {
                        scheitel
                    };
                    if let Some(p) = punkt {
                        self.start_tool("draw.line");
                        if part.contains("halte") {
                            // Zweite Lage: der Scheitel ist für den
                            // **erweiterten Punktfang** gehalten — von ihm geht
                            // die Halbierende aus, ohne gesetzten Startpunkt.
                            self.halt.push(jjcad_core::snap::HaltPunkt {
                                point: p,
                                kind: jjcad_core::snap::SnapKind::Endpoint,
                                entity: None,
                                richtung: None,
                            });
                        } else if let Some(t) = self.tool.as_mut() {
                            t.pts.push(p);
                        }
                    }
                }
                // Prüfhilfe T0027: Treppe nach Schrittmaß anlegen — Lauflinie
                // 4,64 m, Breite 1,00 m, Höhe 2,752 m ergeben 16 Stufen
                // (17,2 / 29). `seltreppe` wählt sie zusätzlich aus, damit das
                // Eigenschaften-Panel die Maße zeigt.
                "treppenschnitt" => {
                    // Prüfhilfe: oberer Teil der Treppe gestrichelt (eigene
                    // Linien je Treppenteil), unterer durchgezogen.
                    let u = self.unit().mm();
                    let neu = self
                        .session_mut()
                        .execute(
                            "draw.stair",
                            &serde_json::json!({
                                "a": [0.0, 0.0],
                                "b": [0.0, 4640.0 / u],
                                "width": 1000.0 / u,
                                "height": 2752.0 / u,
                                "auto": true,
                            }),
                        )
                        .ok()
                        .and_then(|r| r["created"][0].as_u64());
                    if let Some(id) = neu {
                        let _ = self.session_mut().execute(
                            "modify.set",
                            &serde_json::json!({"ids": [id], "type_oben": "dashed",
                                                 "weight_oben": 0.18}),
                        );
                        let _ = self
                            .session_mut()
                            .execute(
                                "draw.stair.label",
                                &serde_json::json!({ "id": id, "layer": ebene }),
                            );
                    }
                    self.fit_view();
                }
                "treppe" | "seltreppe" => {
                    let u = self.unit().mm();
                    let neu = self
                        .session_mut()
                        .execute(
                            "draw.stair",
                            &serde_json::json!({
                                "a": [0.0, 0.0],
                                "b": [0.0, 4640.0 / u],
                                "width": 1000.0 / u,
                                "height": 2752.0 / u,
                                "auto": true,
                            }),
                        )
                        .ok()
                        .and_then(|r| r["created"][0].as_u64());
                    if let Some(id) = neu {
                        // Beschriftung gleich mit — das ist der Zustand nach
                        // dem Knopf „Beschriftung erzeugen".
                        let _ = self
                            .session_mut()
                            .execute(
                                "draw.stair.label",
                                &serde_json::json!({ "id": id, "layer": ebene }),
                            );
                        if part.trim() == "seltreppe" {
                            self.session_mut().selection = vec![id];
                        }
                    }
                    self.fit_view();
                }
                // Erstes Objekt der Art auswählen — Prüfhilfe für das
                // Eigenschaften-Panel (Screenshot-Vergleich Erzeugen/Bearbeiten).
                "wandpanel" | "tuerpanel" | "fensterpanel" | "flaechenpanel" | "dimpanel" => {
                    self.ui.props_open = true;
                    let name = part.trim();
                    let passt = |k: &EntityKind| match name {
                        "tuerpanel" => matches!(k, EntityKind::Door { .. }),
                        "fensterpanel" => matches!(k, EntityKind::Window { .. }),
                        "flaechenpanel" => matches!(k, EntityKind::Fill { .. }),
                        "dimpanel" => matches!(k, EntityKind::Dim { .. }),
                        _ => matches!(k, EntityKind::Wall { .. }),
                    };
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| passt(&e.kind))
                        .map(|e| e.id)
                    {
                        self.session_mut().select(vec![id]);
                    }
                }
                // Prüfhilfe T0242: zwei Wände mit Mauerwerk — die untere ohne
                // Füllung; sie ist ausgewählt, das Panel zeigt den Zustand
                // „Keine Füllung“.
                "ohnefuellung" | "keinefuellung" => {
                    let u = self.unit().mm();
                    let mut letzte = None;
                    for (y, ohne) in [(0.0_f64, false), (2000.0_f64, true)] {
                        let mut p = serde_json::json!({
                            "a": [0.0, y / u],
                            "b": [4000.0 / u, y / u],
                            "thickness": 300.0 / u,
                            "hatch": "mauerwerk",
                        });
                        if ohne {
                            p["fill"] = serde_json::json!("keine");
                        }
                        if let Ok(r) = self.session_mut().execute("draw.wall", &p) {
                            letzte = r["created"][0].as_u64();
                        }
                    }
                    if let Some(id) = letzte {
                        self.session_mut().select(vec![id]);
                    }
                    // Das Eigenschaften-Panel soll im Bild stehen — der
                    // Blattmanager belegt sonst die rechte Seite (T0233).
                    self.ui.blatt_open = false;
                    self.ui.props_open = true;
                    self.fit_view();
                }
                // Prüfhilfe T0242: zwei Füllflächen mit Mauerwerk — die rechte
                // ohne Füllung (nur Umrandung); sie ist ausgewählt.
                "flaecheohnefuellung" | "flaechekeinefuellung" => {
                    let u = self.unit().mm();
                    let mut letzte = None;
                    for (x0, ohne) in [(0.0_f64, false), (3000.0_f64, true)] {
                        let mut p = serde_json::json!({
                            "boundary": [
                                [x0 / u, 0.0],
                                [(x0 + 2000.0) / u, 0.0],
                                [(x0 + 2000.0) / u, 1500.0 / u],
                                [x0 / u, 1500.0 / u],
                            ],
                            "pattern": "mauerwerk",
                            "border": true,
                        });
                        if ohne {
                            p["fill"] = serde_json::json!("keine");
                        }
                        if let Ok(r) = self.session_mut().execute("draw.fill", &p) {
                            letzte = r["created"][0].as_u64();
                        }
                    }
                    if let Some(id) = letzte {
                        self.session_mut().select(vec![id]);
                    }
                    self.ui.blatt_open = false;
                    self.ui.props_open = true;
                    self.fit_view();
                }
                "wall" => self.start_tool("draw.wall"),
                "paint" | "farbeimer" => self.start_tool("draw.fill.paint"),
                "door" => self.start_tool("draw.door"),
                "window" => self.start_tool("draw.window"),
                "dimtool" => self.start_tool("dim.chain"),
                // Parallele (T0200): eine Linie als Vorlage wählen und das
                // Werkzeug starten — die Vorschau folgt dem Zeiger.
                "parallele" | "parallel" => {
                    if !self.doc().entities.iter().any(|e| matches!(e.kind, EntityKind::Line { .. })) {
                        let _ = self.session_mut().execute(
                            "draw.line",
                            &serde_json::json!({"a": [1.0, 1.0], "b": [6.0, 1.0]}),
                        );
                    }
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().select(vec![id]);
                    }
                    self.start_tool("draw.parallel");
                }
                // Abrunden und Phase (T0247): zwei Linien bilden eine Ecke,
                // das Werkzeug hat beide gewählt — es fehlt nur noch das Maß.
                "abrunden" | "fillet" | "phase" | "chamfer" => {
                    let u = self.unit().mm();
                    let mut ids: Vec<Id> = Vec::new();
                    for (a, b) in [
                        ([500.0_f64, 500.0_f64], [4500.0_f64, 500.0_f64]),
                        ([500.0, 500.0], [500.0, 4500.0]),
                    ] {
                        if let Ok(r) = self.session_mut().execute(
                            "draw.line",
                            &serde_json::json!({
                                "a": [a[0] / u, a[1] / u],
                                "b": [b[0] / u, b[1] / u]
                            }),
                        ) {
                            if let Some(id) = r["created"][0].as_u64() {
                                ids.push(id);
                            }
                        }
                    }
                    let werkzeug = if matches!(part.trim(), "phase" | "chamfer") {
                        "modify.chamfer"
                    } else {
                        "modify.fillet"
                    };
                    self.ui.ribbon = RibbonTab::Aendern;
                    self.start_tool(werkzeug);
                    if ids.len() == 2 {
                        if let Some(t) = self.tool.as_mut() {
                            t.obj = Some(ids[0]);
                            t.obj2 = Some(ids[1]);
                            t.pick_a = Some(jjcad_core::geom::Vec2::new(2500.0, 500.0));
                            t.pick_b = Some(jjcad_core::geom::Vec2::new(500.0, 2500.0));
                            t.pts = vec![jjcad_core::geom::Vec2::new(500.0, 500.0)];
                        }
                        self.session_mut().select(ids);
                    }
                    self.ui.blatt_open = false;
                    self.ui.props_open = true;
                    self.fit_view();
                }
                // Knick einer Polylinie (T0247): die Ecke ist mit dem
                // Scheitelpunkt schon gewählt, es fehlt nur das Maß.
                "knick" | "abrundenknick" | "phaseknick" | "pfadknick"
                | "abrundenpfad" | "phasepfad" => {
                    let u = self.unit().mm();
                    // Pfad oder Polylinie: beide tragen einen Knick, den das
                    // Werkzeug mit einem Klick wählt (T0247).
                    let pfad = part.trim().contains("pfad");
                    let werkzeug = if part.trim().contains("phase") {
                        "modify.chamfer"
                    } else {
                        "modify.fillet"
                    };
                    let antwort = self.session_mut().execute(
                        if pfad { "draw.path" } else { "draw.polyline" },
                        &serde_json::json!({
                            "pts": [[500.0 / u, 500.0 / u], [4500.0 / u, 500.0 / u],
                                    [4500.0 / u, 4500.0 / u]],
                            "closed": false
                        }),
                    );
                    self.ui.ribbon = RibbonTab::Aendern;
                    self.start_tool(werkzeug);
                    if let Ok(r) = antwort {
                        if let Some(id) = r["created"][0].as_u64() {
                            if let Some(t) = self.tool.as_mut() {
                                t.obj = Some(id);
                                t.knoten = Some(1);
                                t.pts = vec![jjcad_core::geom::Vec2::new(4500.0, 500.0)];
                            }
                            self.session_mut().select(vec![id]);
                        }
                    }
                    self.ui.blatt_open = false;
                    self.ui.props_open = true;
                    self.fit_view();
                }
                // Pfad-Werkzeug (T0229): zeigt den Werkzeugzustand und wählt
                // den Pfad, damit Vorschau und Panel im Bild stehen.
                "pfad" | "path" => {
                    let antwort = self.session_mut().execute(
                        "draw.path",
                        &serde_json::json!({"pts": [[0.0, 0.0], [2.0, 0.0], [2.5, 1.0], [2.0, 2.0], [0.0, 2.0]],
                                            "schritte": ["start", "line", "arc_point", "arc_end", "line"],
                                            "closed": true}),
                    );
                    if let Ok(v) = antwort {
                        if let Some(id) = v.get("created").and_then(|c| c.get(0)).and_then(|x| x.as_u64()) {
                            self.session_mut().select(vec![id]);
                        }
                    }
                }
                "pfadtool" | "pathtool" => {
                    self.start_tool("draw.path");
                    self.ui.ribbon = RibbonTab::Zeichnen;
                }
                "stairtool" => self.start_tool("draw.stair"),
                "kreistool" => self.start_tool("draw.circle"),
                "circlepanel" => {
                    self.ui.props_open = true;
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Circle { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().select(vec![id]);
                    }
                }
                "volumenpanel" => {
                    self.ui.props_open = true;
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Volume { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().select(vec![id]);
                    }
                }
                "text" => self.start_tool("draw.text"),
                // Prüfhilfe: Füllwerkzeug über den jeweiligen Knopf starten
                "fillarea" => {
                    let m = self.ui.fill_input;
                    self.start_fill_input(m, FillKind::Area);
                }
                "fillhatch" => {
                    let m = self.ui.hatch_input;
                    self.start_fill_input(m, FillKind::Hatch);
                }
                "volumen" | "volume" => {
                    let m = self.ui.volume_input;
                    self.start_fill_input(m, FillKind::Volume);
                }
                // Kopierfunktion der geteilten Schaltfläche (Screenshot-Prüfung)
                "copyclip" => self.ui.copy_input = CopyInput::Clipboard,
                // Prüfhilfe T0021: die erste Wand kopieren und die Einfüge-Vorschau
                // zeigen — sie steht als Akzentlinie über der Originalwand.
                // Hilfslinien-Eingabe (T0230): Linien-Werkzeug, die Eingabe
                // ist eingeschaltet — die gestrichelte Vorschau läuft vom
                // letzten Punkt zum Zeiger und schreibt ihre Länge daneben.
                "hilfslinieeingabe" | "hilfsliniezeichnen" => {
                    self.start_tool("draw.line");
                    let anfang = V2::new(-2000.0, -1000.0);
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(anfang);
                    }
                    self.hilfslinie_modus = true;
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = V2::new(0.0, 0.0);
                    v.zoom = 0.2;
                    self.set_view(v);
                }
                // Abstand zu einer bestehenden Wand (T0230, Turn 5): die
                // Wand-Funktion läuft, die Hilfslinie steht 1 m **senkrecht**
                // auf der bestehenden Wand — ihr Ende ist der Punkt, an dem
                // die neue Wand ansetzt.
                "hilfslinieabstand" | "hilfslinienabstand" => {
                    let _ = self.session_mut().execute(
                        "draw.wall",
                        // `draw.wall` nimmt die Eingabeeinheit (Meter): 0,24
                        // sind 240 mm Dicke.
                        &serde_json::json!({"a": [-2.0, -1.0], "b": [1.0, -1.0], "thickness": 0.24}),
                    );
                    self.start_tool("draw.wall");
                    let anker = V2::new(-500.0, -1000.0);
                    let ende = V2::new(-500.0, 0.0);
                    self.hilfslinie = Some(Hilfslinie { a: anker, b: ende });
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = V2::new(-500.0, -500.0);
                    v.zoom = 0.35;
                    self.set_view(v);
                }
                // Stehende Hilfslinie (T0230): 1-m-Strecke am Wandende, mit
                // ihren Endkreuzen und der Länge; der Stift steht auf ihrem
                // Ende, der nächste Klick zeichnet die zweite Wand.
                "hilfsliniegesetzt" | "hilfslinielinie" => {
                    self.start_tool("draw.wall");
                    let anfang = V2::new(-2000.0, -1000.0);
                    if let Some(t) = self.tool.as_mut() {
                        t.pts.push(anfang);
                    }
                    let _ = self.session_mut().execute(
                        "draw.wall",
                        &serde_json::json!({"a": [-2.0, -1.0], "b": [-1.0, -1.0]}),
                    );
                    if let Some(t) = self.tool.as_mut() {
                        t.pts = vec![V2::new(-1000.0, -1000.0)];
                    }
                    self.hilfslinie = Some(Hilfslinie {
                        a: V2::new(-1000.0, -1000.0),
                        b: V2::new(-1000.0, 0.0),
                    });
                    self.started_at = 3.0;
                    let mut v = self.current_view();
                    v.center = V2::new(-1000.0, -500.0);
                    v.zoom = 0.25;
                    self.set_view(v);
                }
                "wandvorschau" => {
                    let wand = self
                        .doc()
                        .ents()
                        .iter()
                        .position(|e| matches!(e.kind, EntityKind::Wall { .. }));
                    if let Some(i) = wand {
                        let id = self.doc().ents()[i].id;
                        let bb = jjcad_core::render::entity_bbox(self.doc(), &self.doc().ents()[i].clone());
                        self.merke_zwischenablage(&[id], None);
                        self.start_ghost_paste();
                        if let Some(g) = self.ghost.as_mut() {
                            if bb.is_valid() {
                                g.pos = V2::new(bb.min.x, bb.max.y + 600.0);
                            }
                        }
                    }
                }
                // Prüfhilfe T0156: Einfüge-Vorschau eines **Blocks** — für das
                // Bild der Miniatur am Zeiger (Vorschau verlässt die Fläche).
                // Der größte Block, damit die Miniatur im Bild zu erkennen ist.
                "blockvorschau" | "blockminiatur" => {
                    let block = self
                        .doc()
                        .blocks
                        .iter()
                        .filter(|b| b.bbox().is_valid())
                        .max_by(|a, b| {
                            let (x, y) = (a.bbox(), b.bbox());
                            (x.width() * x.height())
                                .partial_cmp(&(y.width() * y.height()))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|b| b.id.clone());
                    if let Some(id) = block {
                        self.start_ghost_block(id);
                    }
                }
                // Prüfhilfe T0070: Einfüge-Vorschau um 90° gedreht, Anker am
                // Zeiger. Das Bild zeigt, ob die Vorschau oben rechts bleibt
                // (Drehpunkt „Mittelpunkt") oder um das Fadenkreuz springt
                // („Basispunkt").
                "drehvorschau" => {
                    let wand = self
                        .doc()
                        .ents()
                        .iter()
                        .position(|e| matches!(e.kind, EntityKind::Wall { .. }));
                    if let Some(i) = wand {
                        let id = self.doc().ents()[i].id;
                        self.merke_zwischenablage(&[id], None);
                        self.start_ghost_paste();
                        if let Some(g) = self.ghost.as_mut() {
                            g.rotate(90.0);
                        }
                    }
                }
                // Prüfhilfe T0025: ersten Text kopieren und die Einfüge-Vorschau
                // zeigen. Der Text muss in der Vorschau stehen — vorher fiel er
                // in `canvas::ghost_shape` durch und nur der Anker war zu sehen.
                "textvorschau" => {
                    let text = self
                        .doc()
                        .ents()
                        .iter()
                        .position(|e| matches!(e.kind, EntityKind::Text { .. }));
                    if let Some(i) = text {
                        let e = self.doc().ents()[i].clone();
                        let bb = jjcad_core::render::entity_bbox(self.doc(), &e);
                        self.merke_zwischenablage(&[e.id], None);
                        self.start_ghost_paste();
                        if let Some(g) = self.ghost.as_mut() {
                            if bb.is_valid() {
                                g.pos = V2::new(bb.min.x, bb.min.y - 900.0);
                            }
                        }
                    }
                }
                "copybase" => self.ui.copy_input = CopyInput::BasePoint,
                // Prüfhilfe: erste Tür auswählen, damit das Eigenschaften-Panel
                // im Screenshot belegbar ist
                "seldoor" => {
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Door { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // Prüfhilfe T0022: ersten Kreis bzw. ersten Bogen auswählen —
                // Eigenschaften-Panel mit Radius (und bei Bogen den beiden
                // Endwinkeln) sowie die Griffe an den Bogenenden zeigen.
                "selkreis" | "selcircle" => {
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Circle { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                "selbogen" | "selarc" => {
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Arc { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // Erste Dämmungslinie auswählen: Eigenschaften-Panel mit Breite
                // und Schlaufenform zeigen (T0120).
                "seldaemmung" => {
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| {
                            self.doc().effective_line_type(e)
                                == jjcad_core::style::LineType::Insulation
                        })
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                "selwall" => {
                    // erste Wand auswählen: Eigenschaften-Panel der Wand zeigen
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Wall { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // T0193: erste Wand **und** eine freie Linie auswählen — das
                // Eigenschaften-Panel zeigt dann nur die gemeinsamen Zeilen.
                // Die Wohnhaus-Vorlage enthält keine Linie, deshalb entsteht
                // eine; die Auswahl stellt sich danach aus Wand und Linie
                // zusammen.
                "selgemischt" => {
                    let wand = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Wall { .. }))
                        .map(|e| e.id);
                    let linie = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| {
                            matches!(
                                e.kind,
                                EntityKind::Line { .. } | EntityKind::Polyline { .. }
                            )
                        })
                        .map(|e| e.id)
                        .or_else(|| {
                            self.session_mut()
                                .execute(
                                    "draw.line",
                                    &serde_json::json!({"a": [1.0, 3.0], "b": [3.0, 3.0]}),
                                )
                                .ok()
                                .and_then(|a| a["created"][0].as_u64())
                        });
                    if let (Some(w), Some(l)) = (wand, linie) {
                        self.session_mut().selection = vec![w, l];
                    }
                }
                "selffill" | "selfill" => {
                    // erste Füllfläche auswählen: Eigenschaften-Panel der
                    // Füllfläche (Muster, Winkel, Abstand, Umrandung) zeigen
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Fill { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                "selvolume" | "selvolumen" => {
                    // erstes Volumen auswählen: Eigenschaften-Panel mit Höhe,
                    // Fläche und Volumen (m³) zeigen
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Volume { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // Diagonaler Einfügemodus: Rechteck-Werkzeug läuft, im
                // Eigenschaften-Panel steht „Einfügemodus · Basislinie + Höhe".
                "diagonal" | "diagonale" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.options.rect_mode = Einfuegemodus::Basislinie;
                    self.start_tool("draw.rect");
                }
                // Dasselbe für die Ellipse (Mittelachse + Breite).
                "diagonalellipse" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.options.ellipse_mode = Einfuegemodus::Basislinie;
                    self.start_tool("draw.ellipse");
                }
                // Bogen über Mittelpunkt, Startpunkt und Endpunkt (T0085).
                "bogenmittelpunkt" | "bogenmitte" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.options.arc_mode = Bogenmodus::Mittelpunkt;
                    self.start_tool("draw.arc");
                }
                // T0100: erstes Bild auswählen — Eigenschaften-Panel und der
                // blaue Auswahlton über dem Bild.
                "selbild" => {
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Image { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // T0100: Bildwerkzeug mit dem Bild aus der Zeichnung starten —
                // Vorschau und Panelzeilen im Bild.
                "bild" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    if let Some(EntityKind::Image { name, data, .. }) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Image { .. }))
                        .map(|e| e.kind.clone())
                    {
                        self.options.bild = Some(BildQuelle { name, data });
                        self.start_tool("draw.image");
                    }
                }
                // T0210: Punkte mit Punktsymbolen und eine Linie mit Marken an
                // beiden Enden — das Eigenschaften-Panel zeigt die Symbolzeilen.
                "punktsymbol" => {
                    for (i, sym) in ["sys:pkt-kreuz", "sys:pkt-pfeil", "sys:pkt-kote-grundriss"]
                        .iter()
                        .enumerate()
                    {
                        let x = 2.0 + i as f64 * 2.0;
                        let _ = self.session_mut().execute(
                            "draw.point",
                            &serde_json::json!({"p": [x, 4.0], "symbol": sym}),
                        );
                    }
                    if let Ok(antwort) = self.session_mut().execute(
                        "draw.line",
                        &serde_json::json!({
                            "a": [2.0, 2.0],
                            "b": [6.0, 2.0],
                            "start_symbol": "sys:pkt-punkt",
                            "end_symbol": "sys:pkt-pfeilspitze",
                        }),
                    ) {
                        if let Some(id) = antwort["created"][0].as_u64() {
                            self.session_mut().selection = vec![id];
                        }
                    }
                    // Derselbe Block als **eingesetzte Instanz** (T0210, Turn 3):
                    // er muss so groß sein wie das Symbol am Punkt daneben.
                    let _ = self.session_mut().execute(
                        "block.insert",
                        &serde_json::json!({"block": "sys:pkt-kreuz", "pos": [2.0, 6.0]}),
                    );
                }
                "selpoly" => {
                    // erste (geschlossene) Polylinie auswählen: Eigenschaften-
                    // Panel mit Länge, Fläche, Stützpunkten, Geschlossenheit
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Polyline { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                "seltext" => {
                    // ersten Text auswählen: der Rahmen mit den Fangpunkten
                    // muss im Screenshot sichtbar sein (T0010)
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Text { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // Farbeimer-Prüffall (t_9cd971c5): den Raum unten links wirklich
                // füllen und die Fläche auswählen. Im Screenshot muss der
                // Wandrahmen über der Fläche liegen, und das Eigenschaften-Panel
                // zeigt, an welcher Stelle der Zeichenreihenfolge sie steht.
                "fillpaint" => {
                    let u = self.unit();
                    let klick = V2::new(1500.0, 2600.0);
                    let r = self.session_mut().execute(
                        "draw.fill.paint",
                        &serde_json::json!({
                            "p": [u.from_mm(klick.x), u.from_mm(klick.y)],
                            "pattern": "solid",
                            "fill": "#f0c8c8",
                        }),
                    );
                    if let Ok(v) = r {
                        if let Some(id) = v["created"][0].as_u64() {
                            self.session_mut().selection = vec![id];
                        }
                    }
                }
                "fill" => self.start_tool("draw.fill"),
                // Umriss abfahren: Punkte vorgeben, damit die Vorschau (Fläche
                // halbtransparent, Muster aus dem Panel) im Screenshot steht.
                "fillboundary" => {
                    self.start_tool("draw.fill.boundary");
                    if let Some(t) = &mut self.tool {
                        t.pts = vec![
                            V2::new(2000.0, 2000.0),
                            V2::new(7000.0, 2000.0),
                            V2::new(7000.0, 5000.0),
                            V2::new(4500.0, 6500.0),
                        ];
                    }
                }
                // Freie Punktfolge: derselbe Vorschau-Zustand wie oben, nur über
                // das zweite Füll-Werkzeug — so lässt sich prüfen, dass beide
                // Werkzeuge gleich aussehen.
                "fillpoints" => {
                    self.start_tool("draw.fill.points");
                    if let Some(t) = &mut self.tool {
                        t.pts = vec![
                            V2::new(2000.0, 2000.0),
                            V2::new(7000.0, 2000.0),
                            V2::new(7000.0, 5000.0),
                            V2::new(4500.0, 6500.0),
                        ];
                    }
                }
                // Rechteck aus der Diagonale (T0131): **ein** Punkt gesetzt, der
                // zweite steht am Zeiger — die Vorschau zeigt das fertige
                // Rechteck samt Muster und Fläche.
                "filldiagonale" | "diagonalflaeche" => {
                    self.start_tool("draw.fill.points");
                    if let Some(t) = &mut self.tool {
                        t.pts = vec![V2::new(2000.0, 2000.0)];
                    }
                }
                // Rechteckige Fläche aus der Diagonale, fertig und ausgewählt
                // (T0131): zeigt die vier Eck- und vier Kantengriffe.
                "rechteckflaeche" | "diagonalrechteck" => {
                    let u = self.unit();
                    let r = self.session_mut().execute(
                        "draw.fill.points",
                        &serde_json::json!({
                            "pts": [[u.from_mm(2000.0), u.from_mm(2000.0)],
                                    [u.from_mm(7000.0), u.from_mm(5000.0)]],
                            "pattern": "lines",
                        }),
                    );
                    if let Ok(v) = r {
                        if let Some(id) = v["created"][0].as_u64() {
                            self.session_mut().selection = vec![id];
                        }
                    }
                }
                // Rechteckige Polylinie aus dem Rechteck-Werkzeug (T0133):
                // ausgewählt mit vier Eck- und vier Kantengriffen; `schiefviereck`
                // zieht eine Ecke schräg, dann ist die Form kein Rechteck mehr und
                // die Kantengriffe müssen fehlen.
                "rechteckgriffe" | "schiefviereck" => {
                    // Frische Zeichnung, damit im Bild nur die Polylinie steht.
                    self.new_document();
                    let u = self.unit();
                    let r = self.session_mut().execute(
                        "draw.rect",
                        &serde_json::json!({
                            "a": [u.from_mm(2000.0), u.from_mm(2000.0)],
                            "b": [u.from_mm(7000.0), u.from_mm(5000.0)],
                        }),
                    );
                    if let Ok(v) = r {
                        if let Some(id) = v["created"][0].as_u64() {
                            if part.trim() == "schiefviereck" {
                                if let Some(e) = self.doc_mut().entity_mut(id) {
                                    if let Some(neu) = crate::handles::apply_grip(
                                        e,
                                        crate::handles::GripKind::Vert(1),
                                        V2::new(7600.0, 2600.0),
                                    ) {
                                        *e = neu;
                                    }
                                }
                            }
                            self.session_mut().selection = vec![id];
                            let mut view = self.current_view();
                            view.center = V2::new(4500.0, 3500.0);
                            view.zoom = 0.12;
                            self.set_view(view);
                        }
                    }
                }
                // Farbeimer-Prüffall (F1b): den Raum unten links füllen, so wie
                // es der Farbeimer tun wird — mit Inseln. Im Screenshot müssen
                // Schornstein und Treppe frei bleiben (kein Muster darin).
                "fillhole" => {
                    let klick = V2::new(1500.0, 2600.0);
                    let grenze = jjcad_core::boundary::detect_boundary(
                        self.doc(),
                        klick,
                        &Default::default(),
                    );
                    if let Ok(g) = grenze {
                        let u = self.unit();
                        let ring = |r: &[V2]| -> Vec<[f64; 2]> {
                            r.iter().map(|p| [u.from_mm(p.x), u.from_mm(p.y)]).collect()
                        };
                        let holes: Vec<Vec<[f64; 2]>> = g.holes.iter().map(|h| ring(h)).collect();
                        let r = self.session_mut().execute(
                            "draw.fill.boundary",
                            &serde_json::json!({
                                "boundary": ring(&g.outer),
                                "holes": holes,
                                "pattern": "lines",
                                "angle": 45.0,
                                "scale": u.from_mm(250.0),
                                "border": false,
                                "fill": "#d8e4f0",
                            }),
                        );
                        if let Ok(v) = r {
                            if let Some(id) = v["created"][0].as_u64() {
                                self.session_mut().selection = vec![id];
                            }
                        }
                        self.mark_changed();
                    }
                }
                "selwalls" => {
                    // erste zwei Wände auswählen: Panel der Mehrfachauswahl
                    let ids: Vec<u64> = self
                        .doc()
                        .entities
                        .iter()
                        .filter(|e| matches!(e.kind, EntityKind::Wall { .. }))
                        .take(2)
                        .map(|e| e.id)
                        .collect();
                    self.session_mut().selection = ids;
                }
                "unitmm" => self.session_mut().doc.settings.input_unit = Unit::Mm,
                "unitcm" => self.session_mut().doc.settings.input_unit = Unit::Cm,
                "unitm" => self.session_mut().doc.settings.input_unit = Unit::M,
                "wallcolors" => {
                    // Prüfzustand: Linie und Füllfläche der ersten Wand in
                    // zwei deutlich verschiedenen Farben
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Wall { .. }))
                        .map(|e| e.id)
                    {
                        let _ = self.tab_mut().session.execute(
                            "modify.set",
                            &serde_json::json!({
                                "ids": [id],
                                "color": "#FF0000",
                                "fill": "#2E5FA3"
                            }),
                        );
                        // nicht auswählen: die Auswahl färbt blau ein und
                        // verdeckt genau die beiden Farben, die hier zählen
                        self.session_mut().selection.clear();
                    }
                }
                // Prüfhilfe: Verschieben-Werkzeug starten (Basispunkt-Fang an
                // der ausgewählten Wand im Screenshot belegbar)
                "movetool" => self.start_tool("modify.move"),
                // Stutzen läuft: Prüfbild der Abschnitts-Hervorhebung (T0075)
                // Zerlegen ohne Auswahl: Klick-Werkzeug mit roter Hervorhebung (T0093)
                "zerlegen" | "explode" => self.start_tool("modify.explode"),
                "stutzen" | "trim" => self.start_tool("modify.trim"),
                // Verlängern läuft: Prüfbild der Strecke, die angesetzt würde (T0081)
                "verlaengern" | "extend" => self.start_tool("modify.extend"),
                // Verschneiden läuft (T0082): das erste Objekt ist gemerkt,
                // damit die Vorschau auf dem zweiten im Prüfbild zu sehen ist.
                "verschneiden" | "intersect" => {
                    self.start_tool("modify.intersect");
                    let erstes = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Line { .. } | EntityKind::Wall { .. }))
                        .map(|e| e.id);
                    if let Some(t) = self.tool.as_mut() {
                        t.obj = erstes;
                    }
                }
                // Flächen schneiden (T0090): frische Zeichnung mit einer
                // Füllfläche, entlang einer Punktfolge geteilt — Prüfbild des
                // Ergebnisses samt Auswahl.
                "flaechenschnitt" | "areasplit" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.new_document();
                    let id = self.demo_flaeche([[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]);
                    if let Some(id) = id {
                        let _ = self.session_mut().execute(
                            "modify.area.split",
                            &serde_json::json!({
                                "ids": [id],
                                "pts": [[2.0, -1.0], [2.0, 4.0]],
                            }),
                        );
                        self.session_mut().selection = self
                            .doc()
                            .entities
                            .iter()
                            .filter(|e| matches!(e.kind, EntityKind::Fill { .. }))
                            .map(|e| e.id)
                            .collect();
                    }
                    self.demo_ausschnitt();
                }
                // Flächen vereinigen (T0090): zwei angrenzende Füllflächen,
                // danach eine — Prüfbild des Ergebnisses.
                "flaechenvereinigung" | "areaunion" => {
                    self.ui.ribbon = RibbonTab::Zeichnen;
                    self.new_document();
                    let mut ids = Vec::new();
                    if let Some(id) =
                        self.demo_flaeche([[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]])
                    {
                        ids.push(id);
                    }
                    if let Some(id) =
                        self.demo_flaeche([[4.0, 0.0], [8.0, 0.0], [8.0, 3.0], [4.0, 3.0]])
                    {
                        ids.push(id);
                    }
                    let _ = self
                        .session_mut()
                        .execute("modify.area.union", &serde_json::json!({"ids": ids}));
                    self.session_mut().selection = self
                        .doc()
                        .entities
                        .iter()
                        .filter(|e| matches!(e.kind, EntityKind::Fill { .. }))
                        .map(|e| e.id)
                        .collect();
                    self.demo_ausschnitt();
                }
                // T0060: Schnellauswahl aktiv — der nächste Klick startet den
                // Erzeuger des angeklickten Objekts.
                "schnellauswahl" | "quickselect" => self.start_tool("modify.quickselect"),
                "selwindow" => {
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Window { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // Prüfhilfe: erste Blockinstanz auswählen (Auswahlfarbe belegen)
                "selblock" => {
                    if let Some(id) = self
                        .doc()
                        .entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::BlockRef { .. }))
                        .map(|e| e.id)
                    {
                        self.session_mut().selection = vec![id];
                    }
                }
                // Prüffall T0004/T0239: Papierraum der Druckvorschau mit
                // ausgewähltem Planrahmen (seit T0239 **ein** Block aus
                // Abdeckfläche, Rahmen und Schriftfeld).
                "plankopf" => {
                    if let Some(sid) = self.doc().sheets.first().map(|s| s.id) {
                        self.tab_mut().tab = TabKind::Sheet(sid);
                        self.sync_space();
                        let feld = self
                            .doc()
                            .sheets
                            .first()
                            .and_then(|s| {
                                s.entities
                                    .iter()
                                    .find(|e| matches!(e.kind, EntityKind::BlockRef { .. }))
                                    .map(|e| e.id)
                            });
                        if let Some(eid) = feld {
                            self.session_mut().selection = vec![eid];
                        }
                    }
                }
                "sheetview" => {
                    if let Some(s) = self.doc().sheets.first().map(|s| s.id) {
                        self.switch_tab(TabKind::Sheet(s));
                    }
                }
                // Prüfbild: Ansichtsfenster im Papierraum ausgewählt — es zeigt
                // dann seine vier Eckgriffe wie jedes andere Objekt (T0109).
                "selviewport" => {
                    if let Some(s) = self.doc().sheets.first().map(|s| s.id) {
                        self.switch_tab(TabKind::Sheet(s));
                        if let Some(id) = self
                            .doc()
                            .sheet(s)
                            .and_then(|sh| sh.viewports().next().map(|(e, _)| e.id))
                        {
                            self.session_mut().select(vec![id]);
                        }
                    }
                }
                // Prüfbild T0158: Ansichtsbereich eines Ansichtsfensters wird
                // eingestellt — Fenster ausgewählt, Schalter an, Rahmen markiert.
                "ansichtsbereich" => {
                    if let Some(s) = self.doc().sheets.first().map(|s| s.id) {
                        self.switch_tab(TabKind::Sheet(s));
                        if let Some(id) = self
                            .doc()
                            .sheet(s)
                            .and_then(|sh| sh.viewports().next().map(|(e, _)| e.id))
                        {
                            self.session_mut().select(vec![id]);
                            self.ui.vp_edit = Some(id);
                        }
                    }
                }
                "toolpreview" => {
                    self.start_tool("draw.wall");
                    if let Some(t) = &mut self.tool {
                        t.pts.push(V2::new(0.0, 0.0));
                    }
                }
                // Prüfhilfe Verschiebe-Vorschau: Auswahl setzen, Werkzeug
                // „Verschieben" starten und den Basispunkt in die Auswahlmitte
                // legen — die Vorschau hängt dann am Zeiger.
                "moveline" => {
                    // Prüfzeichnung enthält keine Linie — eine anlegen
                    let _ = self.session_mut().execute(
                        "draw.line",
                        &serde_json::json!({"a": [1.0, 1.0], "b": [3.0, 2.0]}),
                    );
                    let ids: Vec<Id> = self
                        .doc()
                        .entities
                        .iter()
                        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
                        .map(|e| e.id)
                        .rev()
                        .take(1)
                        .collect();
                    self.start_move_preview(ids);
                }
                "movewall" => {
                    let ids: Vec<Id> = self
                        .doc()
                        .entities
                        .iter()
                        .filter(|e| matches!(e.kind, EntityKind::Wall { .. }))
                        .map(|e| e.id)
                        .take(1)
                        .collect();
                    self.start_move_preview(ids);
                }
                "movedoor" => {
                    let ids: Vec<Id> = self
                        .doc()
                        .entities
                        .iter()
                        .filter(|e| {
                            matches!(e.kind, EntityKind::Door { .. } | EntityKind::Window { .. })
                        })
                        .map(|e| e.id)
                        .collect();
                    self.start_move_preview(ids);
                }
                "moveall" => {
                    let ids: Vec<Id> = self.doc().ents().iter().map(|e| e.id).collect();
                    self.start_move_preview(ids);
                }
                // Auswahl setzen und „Kopieren mit Basispunkt" starten: der Chat
                // fragt nur noch nach dem Basispunkt, es entsteht keine Kopie.
                "copybasetool" => {
                    let ids: Vec<Id> = self.doc().ents().iter().map(|e| e.id).collect();
                    self.session_mut().select(ids);
                    self.start_tool("modify.copy");
                }
                // Nur auswählen (für die Prüfung des direkten Ziehens)
                "selall" => {
                    let ids: Vec<Id> = self.doc().ents().iter().map(|e| e.id).collect();
                    self.session_mut().select(ids);
                }
                // T0189: Rechtsklickmenü einer **Mehrfachauswahl** — der Kopf
                // nennt die Anzahl der Objekte, alle Einträge gelten für die
                // ganze Auswahl.
                "mehrfachmenue" => {
                    let ids: Vec<Id> = self.doc().ents().iter().map(|e| e.id).collect();
                    if let Some(erstes) = ids.first().copied() {
                        self.session_mut().select(ids);
                        self.open_menu(MenuKind::Object(erstes), egui::Pos2::new(420.0, 260.0));
                        // Sonst passt die Ansicht in den ersten Bildern ein und
                        // schließt das Menü (T0180).
                        self.started_at = 3.0;
                    }
                }
                _ => {}
            }
        }
    }

    /// Prüfhilfe: Auswahl setzen, „Verschieben" starten, Basispunkt in die
    /// Auswahlmitte legen (Vorschau folgt dem Zeiger).
    fn start_move_preview(&mut self, ids: Vec<Id>) {
        if ids.is_empty() {
            return;
        }
        let mut bb = BBox::EMPTY;
        for id in &ids {
            if let Some(e) = self.doc().entity(*id) {
                bb.union(jjcad_core::render::entity_bbox(self.doc(), e));
            }
        }
        let base = if bb.is_valid() { bb.center() } else { V2::ZERO };
        self.session_mut().selection = ids;
        self.start_tool("modify.move");
        if let Some(t) = &mut self.tool {
            t.pts.push(base);
        }
    }

    // ---- Zugriffe ----

    pub fn tab(&self) -> &DocTab {
        &self.tabs[self.active]
    }

    pub fn tab_mut(&mut self) -> &mut DocTab {
        &mut self.tabs[self.active]
    }

    pub fn session(&self) -> &Session {
        &self.tab().session
    }

    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.tab_mut().session
    }

    pub fn doc(&self) -> &Document {
        &self.session().doc
    }

    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.session_mut().doc
    }

    pub fn selection(&self) -> &[Id] {
        &self.session().selection
    }

    /// Reiter wechseln und den Bearbeitungsraum **im selben Frame** nachziehen.
    ///
    /// `sync_space` läuft sonst erst zu Beginn des nächsten Frames. Dann sähe die
    /// Zeichenfläche im Wechselframe noch den alten Raum und legte dessen
    /// Anzeigeliste unter dem Schlüssel des neuen Raums ab. Nach dem Zoom in der
    /// Druckvorschau blieb so der Blattrahmen im Modell stehen und die Zeichnung
    /// war unsichtbar, bis die Ansicht bewegt wurde (T0007).
    pub fn switch_tab(&mut self, kind: TabKind) {
        if self.tab().tab == kind {
            return;
        }
        self.tab_mut().tab = kind.clone();
        self.tool = None;
        // T0145: ein offenes Zwei-Klick-Rechteck gehört zum alten Raum.
        self.select_zwei = None;
        self.session_mut().clear_selection();
        self.sync_space();
        match kind {
            TabKind::Model => {
                if self.tab().view_model.zoom < 1e-4 {
                    self.fit_view();
                }
            }
            TabKind::Sheet(id) => {
                if !self.tab().sheet_views.contains_key(&id) {
                    self.fit_view();
                }
            }
            TabKind::Block(id) => {
                if !self.tab().block_views.contains_key(&id) {
                    self.fit_view();
                }
            }
        }
    }

    /// Kennung des gerade bearbeiteten Blocks (None = Modell/Papierraum).
    pub fn aktiver_block(&self) -> Option<String> {
        match &self.tab().tab {
            TabKind::Block(id) => Some(id.clone()),
            _ => None,
        }
    }

    /// Blockeditor öffnen (T0137). Nur **eigene** Blöcke — Systemblöcke sind
    /// schreibgeschützt und werden mit einer Meldung abgelehnt. Der eine Weg
    /// für Bibliothek, Tab und Tests.
    pub fn oeffne_block_editor(&mut self, id: &str) -> bool {
        let Some(block) = self.doc().block(id).cloned() else {
            self.toast(jjcad_core::i18n::tr("gui.app.block_nicht_gefunden"));
            return false;
        };
        if block.source == jjcad_core::model::BlockSource::System {
            self.toast(jjcad_core::i18n::tr("gui.app.systemblock_schreibgeschuetzt"));
            return false;
        }
        if !self.tab().offene_bloecke.iter().any(|b| b == id) {
            self.tab_mut().offene_bloecke.push(id.to_string());
            // Ausgangsstand für „ohne Speichern schließen“.
            self.tab_mut()
                .block_original
                .insert(id.to_string(), block.clone());
        }
        self.switch_tab(TabKind::Block(id.to_string()));
        if !self.tab().block_views.contains_key(id) {
            self.fit_view();
        }
        true
    }

    /// T0204: Fenster „Block-Eigenschaften“ für einen Block der Bibliothek.
    ///
    /// Systemblöcke sind schreibgeschützt — sie haben kein Fenster (dieselbe
    /// Regel wie „Bearbeiten“ im Rechtsklickmenü).
    pub fn oeffne_block_eigenschaften(&mut self, id: &str) -> bool {
        let Some(block) = self.doc().block(id).cloned() else {
            self.toast(jjcad_core::i18n::tr("gui.app.block_nicht_gefunden"));
            return false;
        };
        if block.source == jjcad_core::model::BlockSource::System {
            self.toast(jjcad_core::i18n::tr("gui.app.systemblock_schreibgeschuetzt"));
            return false;
        }
        self.ui.block_eigenschaften = Some(BlockEigenschaften {
            block: block.id.clone(),
            name: block.name.clone(),
            gruppe: block.category.clone(),
            neue_gruppe: String::new(),
            groesse: block.groesse,
        });
        true
    }

    /// Übernimmt Name und Gruppe aus dem Fenster „Block-Eigenschaften“.
    ///
    /// Der Weg ist die Action `block.update` — dieselbe, die MCP und der
    /// Blockeditor benutzen. Bei Benutzerblöcken schreibt sie die `.jjblk`
    /// mit (Umbenennen = neue Datei, alte weg).
    pub fn block_eigenschaften_uebernehmen(&mut self) -> bool {
        let Some(wahl) = self.ui.block_eigenschaften.clone() else {
            return false;
        };
        let name = wahl.name.trim().to_string();
        if name.is_empty() {
            self.toast(jjcad_core::i18n::tr("gui.app.block_name_fehlt"));
            return false;
        }
        // Eine neu angelegte Gruppe sticht die gewählte — sie ist die
        // Eingabe, die der Nutzer gerade gemacht hat.
        let gruppe = if wahl.neue_gruppe.trim().is_empty() {
            wahl.gruppe.trim().to_string()
        } else {
            wahl.neue_gruppe.trim().to_string()
        };
        let dir = self.prefs.blocks_dir().to_string_lossy().to_string();
        match self.session_mut().execute(
            "block.update",
            &serde_json::json!({
                "block": wahl.block,
                "name": name,
                "category": gruppe,
                "groesse": wahl.groesse.key(),
                "dir": dir,
            }),
        ) {
            Ok(_) => {
                self.ui.block_eigenschaften = None;
                self.mark_changed();
                self.toast(jjcad_core::i18n::tr("gui.app.block_gespeichert"));
                true
            }
            Err(e) => {
                self.toast(e);
                false
            }
        }
    }

    /// T0217: Kopie eines Blocks in die Benutzerblöcke — der Weg des
    /// Rechtsklickmenüs in der Blockbibliothek. Systemblöcke sind
    /// schreibgeschützt; ihre Kopie ist ein Benutzerblock und damit
    /// bearbeitbar. Die Bibliothek springt auf den Reiter „Benutzerblöcke“,
    /// damit die Kopie sofort zu sehen ist.
    pub fn block_kopieren(&mut self, id: &str) -> bool {
        let dir = self.prefs.blocks_dir().to_string_lossy().to_string();
        match self
            .session_mut()
            .execute("block.copy", &serde_json::json!({ "block": id, "dir": dir }))
        {
            Ok(_) => {
                self.ui.inventory_tab = 1;
                self.ui.block_quelle = 1; // Benutzerblöcke (BLOCK_QUELLEN)
                self.mark_changed();
                self.toast(jjcad_core::i18n::tr("gui.app.block_kopiert"));
                true
            }
            Err(e) => {
                self.toast(e);
                false
            }
        }
    }

    /// T0219: Löschen eines Blocks anfragen — der Weg des Rechtsklickmenüs in
    /// der Blockbibliothek.
    ///
    /// Gelöscht wird **nie** sofort: erst die Rückfrage (`ui.block_delete`),
    /// die auch auf noch vorhandene Instanzen hinweist. Systemblöcke sind
    /// schreibgeschützt und haben keinen Löschweg (wie „Bearbeiten“).
    pub fn block_loeschen_anfragen(&mut self, id: &str) -> bool {
        let Some(block) = self.doc().block(id).cloned() else {
            self.toast(jjcad_core::i18n::tr("gui.app.block_nicht_gefunden"));
            return false;
        };
        if block.source == jjcad_core::model::BlockSource::System {
            self.toast(jjcad_core::i18n::tr("gui.app.systemblock_schreibgeschuetzt"));
            return false;
        }
        self.ui.block_delete = Some(block.id);
        true
    }

    /// Führt das Löschen aus der Rückfrage aus (T0219).
    ///
    /// `zerlegen` schickt `explode` an `block.delete`: die Action zerlegt dann
    /// zuerst **alle** Instanzen (Modell, Blätter, andere Blöcke) und löscht
    /// den Block danach. Ohne das bleibt ein Block mit Instanzen stehen.
    pub fn block_loeschen(&mut self, zerlegen: bool) -> bool {
        let Some(id) = self.ui.block_delete.clone() else {
            return false;
        };
        let dir = self.prefs.blocks_dir().to_string_lossy().to_string();
        match self.session_mut().execute(
            "block.delete",
            &serde_json::json!({ "block": id, "dir": dir, "explode": zerlegen }),
        ) {
            Ok(_) => {
                self.ui.block_delete = None;
                // Ein offener Editor dieses Blocks hat keinen Block mehr.
                self.tab_mut().offene_bloecke.retain(|x| x != &id);
                self.tab_mut().block_original.remove(&id);
                self.tab_mut().block_views.remove(&id);
                if self.aktiver_block().as_deref() == Some(id.as_str()) {
                    self.switch_tab(TabKind::Model);
                }
                self.mark_changed();
                self.toast(jjcad_core::i18n::tr(if zerlegen {
                    "gui.app.block_zerlegt_und_geloescht"
                } else {
                    "gui.app.block_geloescht"
                }));
                true
            }
            Err(e) => {
                self.toast(e);
                false
            }
        }
    }

    /// Blockeditor schließen. Ohne `speichern` wird der Stand vom Öffnen
    /// wiederhergestellt — ein Reiterkreuz verwirft, „Speichern“ übernimmt.
    pub fn schliesse_block_editor(&mut self, id: &str, speichern: bool) {
        if !speichern {
            let original = self.tab_mut().block_original.remove(id);
            if let Some(orig) = original {
                if let Some(b) = self.doc_mut().block_mut(id) {
                    *b = orig;
                }
                self.doc_mut().modified = true;
                self.mark_changed();
            }
        } else {
            self.tab_mut().block_original.remove(id);
        }
        self.tab_mut().offene_bloecke.retain(|b| b != id);
        self.tab_mut().block_views.remove(id);
        if self.aktiver_block().as_deref() == Some(id) {
            self.switch_tab(TabKind::Model);
        }
    }

    /// „Speichern“ im Blockeditor (T0137): Änderungen übernehmen, bei einem
    /// Benutzerblock die `.jjblk`-Datei nachziehen und den Reiter schließen.
    ///
    /// Der Block selbst ist bereits im Dokument geändert (der Blockeditor ist
    /// ein Bearbeitungsraum auf denselben Objekten) — hier wird nur noch die
    /// Datei geschrieben und der Raum verlassen.
    pub fn block_editor_speichern(&mut self) -> bool {
        let Some(id) = self.aktiver_block() else {
            return false;
        };
        let Some(block) = self.doc().block(&id).cloned() else {
            return false;
        };
        if block.source == jjcad_core::model::BlockSource::User {
            // T0128: Die Datei ist die Wahrheit der Benutzerbibliothek.
            let dir = self.prefs.blocks_dir();
            if let Err(grund) = jjcad_core::bloecke::schreibe_block(&dir, &block) {
                self.toast(grund);
                return false;
            }
        }
        self.doc_mut().modified = true;
        self.schliesse_block_editor(&id, true);
        self.toast(jjcad_core::i18n::tr("gui.app.block_gespeichert"));
        true
    }

    /// Bearbeitungsraum diesem Reiter nachziehen (Modell ↔ Papierraum).
    /// Wird einmal je Frame aufgerufen; die Action-Schicht liest ihn aus dem
    /// Dokument, damit GUI, Tastatur und MCP denselben Weg gehen.
    pub fn sync_space(&mut self) {
        let block = self.aktiver_block();
        let raum = match &self.tab().tab {
            TabKind::Model => None,
            TabKind::Sheet(id) => Some(*id),
            TabKind::Block(_) => None,
        };
        // Ein gelöschter Block lässt seinen Reiter auf das Modell zurückfallen.
        if let Some(bid) = &block {
            if self.doc().block(bid).is_none() {
                self.tab_mut().tab = TabKind::Model;
                self.sync_space();
                return;
            }
        }
        let block_gleich = self.doc().block_space == block;
        // T0158: Der Schalter „Ansichtsbereich anpassen“ hängt am ausgewählten
        // Ansichtsfenster. Endet die Auswahl (Klick daneben, Raumwechsel), endet
        // er mit — sonst liefe eine unsichtbare Einstellung weiter, während das
        // Panel den Schalter gar nicht mehr zeigt.
        if let Some(id) = self.ui.vp_edit {
            if raum.is_none() || !self.session().selection.contains(&id) {
                self.ui.vp_edit = None;
            }
        }
        if block_gleich && self.session().space() == raum {
            return;
        }
        self.session_mut().clear_selection();
        self.tool = None;
        self.ghost = None;
        self.tab_mut().hover = None;
        // Die Anzeigeliste des alten Raums gilt hier nicht mehr.
        self.tab_mut().cache = None;
        match &block {
            Some(bid) => {
                self.doc_mut().set_block_space(Some(bid));
            }
            None => self.session_mut().set_space(raum),
        }
        // Der Papierraum ist in Blatt-Millimetern gezeichnet: dort ist eine
        // Texthöhe von 2,5 mm richtig, nicht die 250 mm des Modells.
        let modell_hoehe = self.doc().settings.text_height;
        if self.doc().in_paper_space() {
            self.options.text_height = 2.5;
        } else if (self.options.text_height - 2.5).abs() < 1e-9 {
            self.options.text_height = modell_hoehe;
        }
    }

    /// Läuft die Bearbeitung im Papierraum (Druckvorschau)?
    pub fn im_papierraum(&self) -> bool {
        self.doc().in_paper_space()
    }

    pub fn unit(&self) -> Unit {
        // T0228: Jeder Bearbeitungsraum trägt seine eigene Eingabeeinheit —
        // das Modell in Metern, der Papierraum (und ein Block in Papiergröße)
        // in Millimetern. Die eine Abfrage steht in `Document::input_unit`;
        // die Oberfläche rechnet die Werte damit um und core liest sie damit
        // wieder ein, sonst läge eine 240-mm-Wand im Blatt bei 240 m.
        self.doc().input_unit()
    }

    /// Ebene, die ein Werkzeug ausdrücklich an die Befehls-Schicht geben muss
    /// (T0039, T0202).
    ///
    /// Die vier Standardlayer eines Plans sind festen Objektarten zugeordnet
    /// (`actions::standard_ebene`): Texte und Bemaßung gehören zu
    /// „Beschriftung“, Wände zu „Architektur“ usw. Ein Werkzeug darf diese
    /// Zuordnung nicht mit der zuletzt angeklickten Ebene überschreiben.
    /// Nur eine **eigene** Ebene (nicht eine der vier) sticht sie — wer sie im
    /// Layer-Manager wählt, zeichnet bewusst dorthin. Eine Aktion ohne
    /// Standardebene (z. B. `modify.*`) bekommt immer die aktuelle Ebene.
    ///
    /// `None` heißt: nichts übergeben, die Befehls-Schicht nimmt die
    /// typgerechte Standardebene im Plan des aktuellen Layers.
    pub fn werkzeug_ebene(&self, action: &str) -> Option<Id> {
        let doc = self.doc();
        let eigene = doc
            .layer(doc.current_layer)
            .map(|l| {
                !jjcad_core::style::LAYER_DEFAULTS
                    .iter()
                    .any(|(n, _, _)| n.eq_ignore_ascii_case(&l.name))
            })
            .unwrap_or(false);
        if eigene || jjcad_core::actions::standard_ebene(action).is_none() {
            Some(doc.current_layer)
        } else {
            None
        }
    }

    /// Die Ebene, die ein **neues** Objekt der Art `standard` bekommt — die
    /// eine Anzeige im Eigenschaften-Panel (T0202).
    ///
    /// Reihenfolge: ausdrückliche Wahl im Panel (`pending.layer`), dann eine
    /// **eigene** aktuelle Ebene, dann die Standardebene des Typs im Plan des
    /// aktuellen Layers, zuletzt die aktuelle Ebene. So zeigt das Panel beim
    /// Platzieren dieselbe Ebene, auf der das Objekt danach liegt.
    pub fn werkzeug_ziel_ebene(&self, standard: &str) -> Id {
        let doc = self.doc();
        if let Some(l) = self.ui.pending.layer {
            if doc.layer(l).is_some() {
                return l;
            }
        }
        let eigene = doc
            .layer(doc.current_layer)
            .map(|l| {
                !jjcad_core::style::LAYER_DEFAULTS
                    .iter()
                    .any(|(n, _, _)| n.eq_ignore_ascii_case(&l.name))
            })
            .unwrap_or(false);
        if eigene {
            return doc.current_layer;
        }
        let plan = doc.aktueller_plan();
        doc.layers
            .iter()
            .find(|l| {
                Some(l.plan) == plan && l.name.eq_ignore_ascii_case(standard)
            })
            .map(|l| l.id)
            .unwrap_or(doc.current_layer)
    }

    /// Meldung an den Nutzer: steht unten links im Chat (und in `status_msg`
    /// für die Tests). Ein eigener Toast ist damit entfallen — eine Rückmeldung,
    /// eine Stelle.
    pub fn toast(&mut self, msg: impl Into<String>) {
        self.mitteilung(msg);
    }

    /// Meldung in Chat und Statusmeldung schreiben.
    pub fn mitteilung(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.status_msg = msg.clone();
        let jetzt = self.ctx.input(|i| i.time);
        self.chat.sagen(jetzt, msg);
    }

    /// Wie [`App::mitteilung`], aber nur bei geändertem Text — für
    /// Werkzeug-Aufforderungen, die in jedem Frame anliegen.
    pub fn mitteilung_wenn_neu(&mut self, msg: &str) {
        self.status_msg = msg.to_string();
        let jetzt = self.ctx.input(|i| i.time);
        self.chat.sagen_wenn_neu(jetzt, msg);
    }

    pub fn mark_changed(&mut self) {
        let t = self.tab_mut();
        t.rev += 1;
        t.cache = None;
    }

    // ---- Zeichenreihenfolge ----
    // Eine Stelle für alle Aufrufer (Kontextmenü, Ribbon): die Rechnung selbst
    // steckt in der Action `modify.order`, hier wird nur die Auswahl übergeben.

    /// Schiebt die Auswahl ganz nach vorn (zuletzt gezeichnet) bzw. nach hinten.
    pub fn order_selection_to(&mut self, front: bool) {
        let ids = self.selection().to_vec();
        if ids.is_empty() {
            self.toast(jjcad_core::i18n::tr("gui.app.auswahl_leer"));
            return;
        }
        self.order_to(front, ids);
    }

    /// Verschiebt genau diese Objekte an den Anfang/das Ende der Objektliste.
    pub fn order_to(&mut self, front: bool, ids: Vec<Id>) {
        if ids.is_empty() {
            return;
        }
        let where_ = if front { "front" } else { "back" };
        let _ = self
            .session_mut()
            .execute("modify.order", &serde_json::json!({"ids": ids, "where": where_}));
        self.mark_changed();
        self.toast(if front { jjcad_core::i18n::tr("gui.app.ganz_nach_vorn") } else { jjcad_core::i18n::tr("gui.app.ganz_nach_hinten") });
    }

    /// „Vor/Hinter Element“: der nächste Klick in die Zeichenfläche bestimmt das
    /// Referenzobjekt (ausgewertet in `canvas`). Braucht eine Auswahl.
    pub fn order_pick_start(&mut self, above: bool) {
        if self.selection().is_empty() {
            self.toast(jjcad_core::i18n::tr("gui.app.reihenfolge_erst_ein_element_waehlen"));
            return;
        }
        self.ui.order_pick = Some(above);
        self.status_msg = if above {
            jjcad_core::i18n::tr("gui.app.element_anklicken_vor_das_die_auswahl_soll_esc_bricht_ab")
        } else {
            jjcad_core::i18n::tr("gui.app.element_anklicken_hinter_das_die_auswahl_soll_esc_bricht")
        }
        .into();
    }

    // ---- Ansicht ----

    pub fn current_view(&self) -> View {
        let t = self.tab();
        match &t.tab {
            TabKind::Model => t.view_model,
            TabKind::Sheet(id) => *t.sheet_views.get(id).unwrap_or(&View {
                center: V2::new(0.0, 0.0),
                zoom: 2.2,
                rot: 0.0,
            }),
            // Der Block ist in Millimetern um seinen Basispunkt gezeichnet.
            TabKind::Block(id) => *t.block_views.get(id).unwrap_or(&View {
                center: V2::new(0.0, 0.0),
                zoom: 0.35,
                rot: 0.0,
            }),
        }
    }

    pub fn set_view(&mut self, v: View) {
        // T0180: Eine bewegte Ansicht lässt das Kontextmenü nicht stehen — es
        // hängt an einer Bildschirmstelle, die Zeichnung darunter wandert weg.
        // Der eine Ort dafür ist hier: Schwenk, Zoom, Einpassen und Drehen
        // laufen alle durch `set_view`.
        self.ui.menu = None;
        let active = self.active;
        let tabkind = self.tabs[active].tab.clone();
        let t = &mut self.tabs[active];
        match tabkind {
            TabKind::Model => t.view_model = v,
            TabKind::Sheet(id) => {
                t.sheet_views.insert(id, v);
            }
            TabKind::Block(id) => {
                t.block_views.insert(id, v);
            }
        }
        t.cache = None;
    }

    /// Hält die Zeichnung an ihrem Bildschirmplatz, wenn sich die Zeichenfläche
    /// ändert (T0184).
    ///
    /// Die Ansicht hängt an der **Mitte** der Zeichenfläche (`screen_to_world`).
    /// Öffnet sich ein Seiten-Panel, wandert diese Mitte und die Zeichnung
    /// rutschte zur Seite. Deshalb wird der Weltpunkt am linken oberen
    /// Fensterrand festgehalten: die Fläche wächst oder schrumpft nur in eine
    /// Richtung, die Objekte bleiben stehen. Gerechnet wird mit der **Fläche**,
    /// nicht mit dem Panel — damit gilt es für jedes Panel an jeder Seite und
    /// für jede Breite, auch für künftige.
    ///
    /// `alt`/`neu` sind die Zeichenflächen, `anker` ist der feste Bildschirmpunkt
    /// (linke obere Ecke der Fensterfläche).
    pub fn ansicht_verankern(&mut self, alt: Rect, neu: Rect, anker: Pos2) {
        let v = self.current_view();
        // Weltpunkt, der bisher am Anker stand …
        let w = screen_to_world(alt, v, anker);
        // … und der Versatz, den `screen_to_world` mit der neuen Fläche bildet.
        let d = rot_vec(
            V2::new(
                (anker.x - neu.center().x) as f64 / v.zoom,
                -((anker.y - neu.center().y) as f64) / v.zoom,
            ),
            -v.rot,
        );
        let mut neu_v = v;
        neu_v.center = V2::new(w.x - d.x, w.y - d.y);
        self.set_view(neu_v);
    }

    /// Ansichtsfenster, dessen **Ansichtsbereich** gerade eingestellt wird
    /// (T0158) — Blattkennung und Fenster.
    ///
    /// Nur im Papierraum und nur für ein vorhandenes Fenster; ohne Auswahl
    /// schaltet [`App::sync_space`] den Schalter ab.
    pub fn vp_edit_fenster(&self) -> Option<(Id, jjcad_core::model::Viewport)> {
        let id = self.ui.vp_edit?;
        let sid = match &self.tab().tab {
            TabKind::Sheet(sid) => *sid,
            _ => return None,
        };
        let vp = self.doc().sheet(sid)?.viewport(id)?.clone();
        Some((sid, vp))
    }

    /// Ansichtsbereich eines Fensters setzen (T0158): Maßstab und/oder
    /// Modellmitte über `modify.set` — derselbe Weg wie das
    /// Eigenschaften-Panel, damit Panel und Mausbewegung nie auseinanderlaufen.
    pub fn vp_setzen(&mut self, id: Id, scale: Option<f64>, center: Option<V2>) {
        let u = self.unit();
        let mut p = serde_json::json!({ "ids": [id] });
        if let Some(s) = scale {
            p["scale"] = serde_json::json!(s);
        }
        if let Some(c) = center {
            p["center"] = serde_json::json!([u.from_mm(c.x), u.from_mm(c.y)]);
        }
        let _ = self.session_mut().execute("modify.set", &p);
        self.mark_changed();
    }

    /// Ist das Fenster im Vollbild? Maßgeblich ist der Fensterzustand: beendet
    /// die Fensterverwaltung das Vollbild, zeigt der Knopf das sofort an und
    /// `ui.fullscreen` bleibt nur Rückfall für Oberflächen ohne Rückmeldung
    /// (headless, erster Frame).
    pub fn is_fullscreen(&self, ctx: &egui::Context) -> bool {
        ctx.input(|i| i.viewport().fullscreen).unwrap_or(self.ui.fullscreen)
    }

    /// Vollbild ein- oder ausschalten (F11 und Knopf im Reiter „Ansicht").
    ///
    /// `ViewportCommand::Fullscreen` macht das Fenster rahmenlos und bildschirm-
    /// füllend — der Weg, den eframe/winit vorsieht; kein eigener Fensterrahmen.
    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        let want = !self.is_fullscreen(ctx);
        self.ui.fullscreen = want;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(want));
    }

    /// Ansicht auf Inhalt einpassen.
    pub fn fit_view(&mut self) {
        // T0158: „Ansichtsbereich anpassen“ — „alles zeigen“ passt die Zeichnung
        // ins Ansichtsfenster ein, nicht das Blatt.
        if let Some((sid, vp)) = self.vp_edit_fenster() {
            let _ = self.session_mut().execute(
                "sheet.fit_viewport",
                &serde_json::json!({ "sheet": sid, "viewport": vp.id }),
            );
            self.mark_changed();
            return;
        }
        let rect = self.ui.last_canvas;
        let (w, h) = (rect.width().max(50.0) as f64, rect.height().max(50.0) as f64);
        let tab = self.tab();
        let (bbox, is_sheet): (BBox, bool) = match &tab.tab {
            TabKind::Model | TabKind::Block(_) => {
                let mut b = jjcad_core::render::visible_bbox(&tab.session.doc);
                if !b.is_valid() || b.width() < 1.0 {
                    b = if matches!(&tab.tab, TabKind::Block(_)) {
                        BBox::new(V2::new(-1000.0, -1000.0), V2::new(1000.0, 1000.0))
                    } else {
                        BBox::new(V2::new(0.0, 0.0), V2::new(12000.0, 9000.0))
                    };
                }
                (b, false)
            }
            TabKind::Sheet(id) => match tab.session.doc.sheet(*id) {
                Some(s) => (s.bbox(), true),
                None => (BBox::new(V2::new(0.0, 0.0), V2::new(420.0, 297.0)), true),
            },
        };
        let bw = bbox.width().max(1.0);
        let bh = bbox.height().max(1.0);
        let margin = if is_sheet { 46.0 } else { 70.0 };
        let zoom = ((w - margin).max(60.0) / bw).min((h - margin).max(60.0) / bh);
        // Im Blockeditor ist die Grenze der Zeichnung zu flach: ein Block misst
        // oft nur eine Einheit, das Fenster soll ihn trotzdem groß zeigen (T0231).
        let grenze = if matches!(&tab.tab, TabKind::Block(_)) {
            BLOCK_ZOOM_MAX
        } else {
            60.0
        };
        let v = View {
            center: bbox.center(),
            zoom: zoom.clamp(1e-4, grenze),
            ..self.current_view()
        };
        self.set_view(v);
    }

    /// Obergrenze des Zooms für den aktiven Reiter (T0231): im Blockeditor
    /// darf es viel tiefer hineingehen als in der Zeichnung, weil ein Block
    /// klein ist (ein Punktsymbol misst eine Einheit).
    pub fn zoom_max(&self) -> f64 {
        match self.tab().tab {
            TabKind::Block(_) => BLOCK_ZOOM_MAX,
            _ => 80.0,
        }
    }

    pub fn zoom_by(&mut self, factor: f64) {
        let mut v = self.current_view();
        v.zoom = (v.zoom * factor).clamp(1e-5, self.zoom_max());
        self.set_view(v);
    }

    /// Aktionen aus Ribbon, Hotbar, Tastatur oder Menü.
    pub fn run_action(&mut self, ctx: &egui::Context, id: &str) {
        // T0116: Ein Hotbar-Slot kann einen **Block** tragen (`block:<id>`).
        // Die Zifferntaste läuft über diesen Weg — ohne den Zweig hier wäre der
        // Slot nur per Maus zu gebrauchen (`actions::action` kennt ihn nicht).
        if let Some(block) = id.strip_prefix("block:") {
            self.start_ghost_block(block.to_string());
            return;
        }
        let Some(def) = actions::action(id) else {
            return;
        };
        // Tastaturbelegung (T0064): Solange ihr Fenster offen ist, merkt sich
        // das Programm die zuletzt gewählte Funktion. Ein Klick auf eine leere
        // Taste legt sie dort ab — dasselbe Verhalten wie bei der Hotbar.
        // „Tastaturbelegung" selbst ist keine belegbare Funktion.
        if self.ui.keymap_open && id != "view.keymap" {
            self.ui.keymap_funktion = Some(id.to_string());
        }
        // Die Schnellauswahl ist ein **Schalter** (T0060): läuft ein Werkzeug,
        // beendet derselbe Aufruf es (wie ein Abbruch); läuft keines, wird sie
        // selbst aktiv. Deshalb vor dem allgemeinen Werkzeugstart.
        if id == "modify.quickselect" {
            self.schnellauswahl_umschalten();
            return;
        }
        // Flächen vereinigen (T0090): wirkt sofort auf die Auswahl — kein
        // Klickwerkzeug, deshalb hier vor dem Werkzeugzweig.
        if id == "modify.area.union" {
            let ids = self.selection().to_vec();
            if ids.len() < 2 {
                self.toast(jjcad_core::i18n::tr("gui.app.vereinigen_mindestens_zwei_flaechen_waehlen"));
                return;
            }
            match self
                .session_mut()
                .execute("modify.area.union", &serde_json::json!({"ids": ids}))
            {
                Ok(v) => {
                    self.mark_changed();
                    let neu = v
                        .get("created")
                        .and_then(|c| c.get(0))
                        .and_then(|x| x.as_u64());
                    self.session_mut().select(neu.into_iter().collect());
                    self.toast(jjcad_core::i18n::tr("gui.app.fuellflaeche_vereinigt"));
                }
                Err(e) => self.toast(e),
            }
            return;
        }
        // Verbinden (T0229): wirkt sofort auf die Auswahl — kein Klickwerkzeug.
        if id == "path.connect" {
            let ids = self.selection().to_vec();
            if ids.len() < 2 {
                self.toast(jjcad_core::i18n::tr("gui.app.verbinden_zwei_objekte_waehlen"));
                return;
            }
            match self
                .session_mut()
                .execute("path.connect", &serde_json::json!({ "ids": ids }))
            {
                Ok(v) => {
                    self.mark_changed();
                    let neu = v
                        .get("created")
                        .and_then(|c| c.get(0))
                        .and_then(|x| x.as_u64());
                    self.session_mut().select(neu.into_iter().collect());
                    self.toast(jjcad_core::i18n::tr("gui.app.objekte_verbunden"));
                }
                Err(e) => self.toast(e),
            }
            return;
        }
        // Werkzeuge
        if def.tool
            || matches!(
                id,
                "block.insert" | "modify.select" | "modify.erase" | "modify.trim" | "modify.extend" | "modify.explode"
            )
        {
            self.start_tool(id);
            return;
        }
        match id {
            // Ein neuer Reiter — die aktuelle Zeichnung bleibt offen (T0073)
            "file.new" => self.new_document(),
            "file.open" => self.open_file_dialog(FileMode::Open),
            // T0186: Im Blockeditor gehört „Speichern“ (Strg+S, Ribbon, Menü)
            // dem Block, nicht der Zeichnung. Gespeichert wird die
            // Blockdefinition (bei Benutzerblöcken samt `.jjblk`), danach
            // schließt der Reiter. Die `.jjcad`-Datei auf der Platte bleibt
            // unberührt — vorher schrieb Strg+S die ganze Zeichnung.
            "file.save" | "file.save_as" if self.aktiver_block().is_some() => {
                self.block_editor_speichern();
            }
            "file.save" => self.save_current(false),
            "file.save_as" => self.open_file_dialog(FileMode::SaveJjcad),
            // T0129: Stände der Sicherung ansehen und als neues Dokument öffnen.
            // T0129: kein Wiederherstellen-Fenster mehr — der Sicherungsordner
            // ist die Wahrheit und wird im Dateimanager geöffnet.
            "file.restore" => self.sicherungsordner_oeffnen(),
            "file.export_pdf" | "sheet.pdf" => self.open_file_dialog(FileMode::ExportPdf),
            "file.export_dxf" => self.open_file_dialog(FileMode::ExportDxf),
            "file.import_dxf" => self.open_file_dialog(FileMode::ImportDxf),
            "file.import.pdf" => self.open_file_dialog(FileMode::ImportPdf),
            "file.import_image" => self.open_file_dialog(FileMode::ImportImage),
            // T0235: **ein** Weg für den Bild-Export — das Fenster fragt
            // Bereich, Auflösung und Hintergrund ab.
            "file.export_png" => self.png_export_oeffnen(),
            "edit.undo" => {
                if self.session_mut().undo() {
                    self.mark_changed();
                    self.toast(jjcad_core::i18n::tr("gui.app.verlauf_zurueck"));
                }
            }
            "edit.redo" => {
                if self.session_mut().redo() {
                    self.mark_changed();
                    self.toast(jjcad_core::i18n::tr("gui.app.verlauf_wiederholt"));
                }
            }
            "edit.copy" => self.copy_selection(false),
            "modify.erase" => {
                let ids = self.selection().to_vec();
                if ids.is_empty() {
                    self.start_tool("modify.select");
                    self.toast(jjcad_core::i18n::tr("gui.app.loeschen_erst_objekte_waehlen"));
                } else {
                    let _ = self.session_mut().execute("modify.erase", &serde_json::json!({ "ids": ids }));
                    self.mark_changed();
                }
            }
            // Zerlegen mit Auswahl läuft über `start_tool` (T0093) — hier nur
            // noch der Blockbefehl aus Menü/Inventar.
            "block.explode" => {
                let ids = self.selection().to_vec();
                if ids.is_empty() {
                    self.start_tool("modify.explode");
                } else {
                    match self
                        .session_mut()
                        .execute("modify.explode", &serde_json::json!({ "ids": ids }))
                    {
                        Ok(_) => {
                            self.mark_changed();
                            self.toast(jjcad_core::i18n::tr("gui.app.objekt_zerlegt"));
                        }
                        Err(e) => self.toast(e),
                    }
                }
            }
            "block.create" => {
                if self.selection().is_empty() {
                    self.toast(jjcad_core::i18n::tr("gui.app.auswahl_leer"));
                } else {
                    self.ui.block_name_dialog = Some((String::new(), false));
                }
            }
            // Inventar als Toggle: offen → zu, zu → offen (Taste E, Ribbon, Hotbar-Kachel)
            "view.inventory" => self.toggle_inventory(),
            "block.manager" => {
                self.ui.inventory_open = true;
                self.ui.inventory_tab = 1;
            }
            "view.layers" => {
                self.ui.layers_open = !self.ui.layers_open;
                // T0222: Wiedereinschalten holt das Panel aus der Leiste.
                if self.ui.layers_open {
                    self.ui.layers_collapsed = false;
                    self.panels_begrenzen();
                }
            }
            // Vollbild: Fenster ohne Rahmen, gleicher Weg für F11 und Ribbon
            "view.fullscreen" => self.toggle_fullscreen(ctx),
            "view.keymap" => self.ui.keymap_open = true,
            "view.settings" => self.ui.settings_open = true,
            "view.help" => self.ui.help_open = true,
            "view.hotbar_next" => {
                self.prefs.next_preset();
                let name = self
                    .prefs
                    .hotbar_presets
                    .get(self.prefs.active_preset)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                self.toast(jjcad_core::i18n::t_args_owned("gui.app.hotbar_x", &[("name", format!("{}", name))]));
                let _ = self.prefs.save();
            }
            "edit.repeat" => {
                if let Some(last) = self.session().last_action.clone() {
                    if actions::action(&last).map(|d| d.tool).unwrap_or(false) {
                        self.start_tool(&last);
                    } else {
                        let params = self.session().last_params.clone().unwrap_or(serde_json::json!({}));
                        match self.session_mut().execute(&last, &params) {
                            Ok(_) => {
                                self.mark_changed();
                                self.toast(jjcad_core::i18n::t_args_owned("gui.app.verlauf_wiederholt_x", &[("wert1", format!("{}", actions::action(&last).map(|d| d.label()).unwrap_or("")))]));
                            }
                            Err(e) => self.toast(e),
                        }
                    }
                }
            }
            "view.zoom_extents" => self.fit_view(),
            "view.zoom_in" => self.zoom_by(1.25),
            "view.zoom_out" => self.zoom_by(0.8),
            // Umsetzen der Ansicht: der Aufruf kommt aus der Hotbar, von der
            // Taste und aus dem Ribbon gleich (T0089) und richtet sich nach der
            // Bewegungsart (T0101) — ein Schritt oder ein Dauerlauf.
            "view.pan_left" => self.schwenk_funktion(-1.0, 0.0),
            "view.pan_right" => self.schwenk_funktion(1.0, 0.0),
            "view.pan_up" => self.schwenk_funktion(0.0, -1.0),
            "view.pan_down" => self.schwenk_funktion(0.0, 1.0),
            "sheet.new" => {
                let n = self.doc().sheets.len() + 1;
                let paper = self
                    .doc()
                    .sheets
                    .first()
                    .map(|s| s.paper)
                    .unwrap_or(jjcad_core::style::PaperSize::A3);
                let landscape = self.doc().sheets.first().map(|s| s.landscape).unwrap_or(true);
                let r = self.session_mut().execute(
                    "sheet.new",
                    &serde_json::json!({"paper": paper.label(), "landscape": landscape, "name": format!("Blatt {n}")}),
                );
                match r {
                    Ok(v) => {
                        if let Some(id) = v.get("sheet").and_then(|x| x.as_u64()) {
                            self.tab_mut().tab = TabKind::Sheet(id);
                        }
                        self.mark_changed();
                        self.fit_view();
                    }
                    Err(e) => self.toast(e),
                }
            }
            "sheet.add_viewport" => {
                if let TabKind::Sheet(sid) = self.tab().tab.clone() {
                    let r = self.session_mut().execute(
                        "sheet.add_viewport",
                        &serde_json::json!({"sheet": sid, "show_frame": true}),
                    );
                    match r {
                        Ok(antwort) => {
                            // Das neue Fenster ist ausgewählt — wie ein frisch
                            // gezeichnetes Objekt (T0109).
                            if let Some(id) = antwort
                                .get("viewport")
                                .and_then(|v| v.as_u64())
                            {
                                self.session_mut().select(vec![id]);
                            }
                            self.mark_changed();
                            self.toast(jjcad_core::i18n::tr("gui.app.ansichtsfenster_angelegt"));
                        }
                        Err(e) => self.toast(e),
                    }
                }
            }
            "dim.style" => self.ui.settings_open = true,
            "doc.project" => self.ui.project_dialog = true,
            "select.all" => {
                let ids: Vec<Id> = self.doc().ents().iter().map(|e| e.id).collect();
                self.session_mut().select(ids);
                self.start_tool("modify.select");
            }
            "view.properties" => self.ui.props_open = !self.ui.props_open,
            _ => {
                let _ = ctx;
            }
        }
    }

    /// Zerlegt alles, was in der Auswahl steht (T0093).
    ///
    /// „Zerlegen“ mit Auswahl ist keine Werkzeug-Phase, sondern eine
    /// Ausführung: ein Aufruf, ein Ergebnis. Erst **ohne** Auswahl startet
    /// das Klick-Werkzeug ([`App::start_tool`]), das jedes angeklickte Objekt
    /// auflöst und es vorher rot hervorhebt.
    fn zerlege_auswahl(&mut self) {
        let ids = self.selection().to_vec();
        match self
            .session_mut()
            .execute("modify.explode", &serde_json::json!({ "ids": ids }))
        {
            Ok(_) => {
                // Die Kennungen der zerlegten Objekte gibt es nicht mehr —
                // sonst zeigte das Eigenschaften-Panel gelöschte Objekte.
                self.session_mut().clear_selection();
                self.mark_changed();
                self.mitteilung(jjcad_core::i18n::tr("gui.app.auswahl_zerlegt"));
            }
            Err(e) => self.toast(e),
        }
    }

    /// Vorlage des Werkzeugs „Parallele" (T0200): genau eine gewählte Linie
    /// oder Polylinie.
    ///
    /// Mehrere oder fremde Objekte in der Auswahl ergeben `None` — dann ist
    /// nicht eindeutig, wozu die Parallele gehört, und der erste Klick im
    /// Werkzeug wählt die Vorlage.
    pub fn parallel_quelle_aus_auswahl(&self) -> Option<Id> {
        let mut treffer = self.selection().iter().filter(|id| {
            matches!(
                self.doc().entity(**id).map(|e| &e.kind),
                Some(EntityKind::Line { .. }) | Some(EntityKind::Polyline { .. })
            )
        });
        let erste = *treffer.next()?;
        if treffer.next().is_some() {
            return None;
        }
        Some(erste)
    }

    /// T0250: „Drehen“ während einer laufenden Maßkette schaltet zwischen
    /// waagerecht und senkrecht um.
    ///
    /// Der Werkzeugpuffer bleibt stehen — die Lage der Maßlinie und die
    /// Messpunkte liegen in beiden Modi an derselben Stelle (`pts[0]` ist die
    /// Maßlinie, alles Weitere Messpunkte). Der nächste Klick bzw. ESC legt die
    /// Kette damit in der neuen Richtung an. Meldet, ob umgeschaltet wurde; ein
    /// „Direkt“-Maß hat keine feste Richtung und bleibt unberührt.
    fn masskette_drehen(&mut self) -> bool {
        let Some(neu) = self
            .tool
            .as_ref()
            .and_then(|t| DimModus::aus_werkzeug(&t.action))
            .and_then(DimModus::gedreht)
        else {
            return false;
        };
        if let Some(t) = self.tool.as_mut() {
            t.action = neu.werkzeug().to_string();
        }
        self.options.dim_axis = Some(neu.axis());
        self.mitteilung(match neu {
            DimModus::Senkrecht => jjcad_core::i18n::tr("gui.app.masskette_senkrecht"),
            _ => jjcad_core::i18n::tr("gui.app.masskette_waagerecht"),
        });
        true
    }

    pub fn start_tool(&mut self, id: &str) {
        if actions::action(id).is_none() {
            return;
        }
        // T0237: „Drehen“ und „Spiegeln“ gehören der **Funktion**, nicht dem
        // Tastendruck. Läuft die Einfüge-Vorschau, hat die Funktion den Inhalt
        // der Zwischenablage zu drehen — egal, ob sie über Ribbon, Hotbar,
        // Eigenschaften-Panel oder Tastatur kommt. Vorher drehte nur das
        // Tastenereignis die Vorschau; ein Klick brach das Einfügen ab (T0094)
        // und startete das Werkzeug auf der Auswahl.
        if self.ghost.is_some() {
            match id {
                "modify.rotate" => {
                    self.rotate_ghost(App::ghost_drehgrad(false));
                    return;
                }
                "modify.mirror" => {
                    self.flip_ghost();
                    return;
                }
                _ => {}
            }
        }
        // T0250: Während eine Maßkette platziert wird, schaltet die Funktion
        // „Drehen“ zwischen waagerecht und senkrecht um — sie startet nicht
        // das Drehwerkzeug. Die gesetzten Punkte bleiben dabei stehen.
        if id == "modify.rotate" && self.masskette_drehen() {
            return;
        }
        // T0180: Eine neue Zeichenfunktion schließt das offene Kontextmenü. Der
        // Klick auf Ribbon/Hotbar täte das ohnehin (Klick außerhalb), ein
        // Tastenkürzel oder eine Hotbar-Ziffer nicht — dort blieb das Menü über
        // dem startenden Werkzeug stehen.
        self.ui.menu = None;
        // T0230: Eine Hilfslinie gehört zu **einem** Lauf des Werkzeugs.
        self.hilfslinie = None;
        self.hilfslinie_modus = false;
        self.hilfslinie_anker = None;
        // T0094: Eine andere Funktion beendet das Einfügen. Sonst liefen Vorschau
        // und Werkzeug gleichzeitig — der nächste Klick setzte die Kopie ab,
        // während das Werkzeug auf seinen Punkt wartete.
        self.cancel_ghost();
        // Zerlegen (T0093): steht etwas in der Auswahl, wird es sofort zerlegt.
        // Das Werkzeug gibt es nur ohne Auswahl — dort zerlegt jeder Klick das
        // Objekt unter dem Zeiger (vorher hebt die Vorschau es rot hervor).
        if id == "modify.explode" && !self.selection().is_empty() {
            self.zerlege_auswahl();
            return;
        }
        // Tastaturbelegung (T0064): auch ein Werkzeug aus Hotbar oder Inventar
        // ist damit die „aktive Funktion" für die nächste leere Taste.
        if self.ui.keymap_open {
            self.ui.keymap_funktion = Some(id.to_string());
        }
        self.ui.fill_active = None;
        // Optionen aus Dokument aktualisieren, falls noch nie gesetzt
        self.tool = Some(Tool {
            action: id.to_string(),
            pts: Vec::new(),
            schritte: Vec::new(),
            bogen: false,
            obj: None,
            obj2: None,
            pick_a: None,
            pick_b: None,
            knoten: None,
            selecting: self.session().selection.is_empty()
                && matches!(
                    id,
                    "modify.move"
                        | "modify.copy"
                        | "modify.rotate"
                        | "modify.mirror"
                        | "modify.scale"
                        | "modify.offset"
                ),
            phase: ToolPhase::Collecting,
        });
        // Parallele (T0200): Der gemerkte Abstand gehört zu **einem** Lauf des
        // Werkzeugs und beginnt leer. Steht schon genau eine Linie oder
        // Polylinie in der Auswahl, ist sie die Vorlage (der Nutzer hat sie
        // dann bereits „ausgewählt"); sonst wählt sie der erste Klick.
        if id == "draw.parallel" {
            self.options.parallel_dist = None;
            if let Some(quelle) = self.parallel_quelle_aus_auswahl() {
                if let Some(t) = self.tool.as_mut() {
                    t.obj = Some(quelle);
                }
            }
        }
        // Aufforderung im Chat-Schema (`<Werkzeug> · <Schritt>`). Die
        // Zeichenfläche meldet in jedem Frame denselben Text; dort wird die
        // Wiederholung unterdrückt.
        if let Some(t) = self.tool.clone() {
            let hinweis = commands_hint(&t, &self.options);
            self.mitteilung(hinweis);
        }
        if self.ui.bind_slot.is_some() {
            // Aktives Werkzeug mit Slot verknüpfen
            let slot = self.ui.bind_slot.take().unwrap();
            self.bind_slot_action(slot, Some(id.to_string()));
            self.tool = None;
            // T0074: Der Slot ist vergeben — die Funktionsliste, aus der gewählt
            // wurde, hat ihren Zweck erfüllt und schließt sich.
            self.close_inventory();
            return;
        }
        // Auswahlmodus bei Aktionen, die eine Auswahl brauchen
        if let Some(t) = &mut self.tool {

        }
        if !matches!(
            id,
            "modify.move" | "modify.copy" | "modify.rotate" | "modify.mirror" | "modify.scale" | "modify.offset"
        ) {
            // andere Werkzeuge beenden eine bestehende Auswahl nicht
        }
    }

    /// ESC: Punktfolge abschließen statt alles zu verwerfen.
    ///
    /// Polylinie, Maßkette, Füllfläche und Schraffur (Modus „Punkte setzen")
    /// bestätigen mit ESC genau wie mit Enter — derselbe Abschluss, ein Pfad.
    /// Verworfen wird nur eine unfertige Punktfolge — ein unfertiger erster
    /// Punkt; der harte Abbruch dieser Werkzeuge liegt auf Entf/Backspace.
    fn escape_pressed(&mut self) {
        // Hilfslinien-Eingabe (T0230): ESC nimmt nur sie zurück, das Werkzeug
        // läuft weiter — sonst wäre ein versehentlicher Rechtsklick nur noch
        // über einen Werkzeugabbruch zu lösen.
        if self.hilfslinie_modus {
            self.hilfslinie_modus = false;
            self.hilfslinie_anker = None;
            return;
        }
        // Maßketten-Eingabe: **ESC bestätigt** die Eingabe wie Enter (T0154,
        // Turn 5) — die Punkte sind gesetzt, ein Abbruch würde sie
        // wegwerfen. Nur ohne jeden Klick gibt es nichts zu bestätigen, dann
        // bricht ESC wie bisher ab.
        if let Some(t) = self.tool.as_ref() {
            if DimModus::aus_werkzeug(&t.action).is_some() {
                if t.pts.is_empty() {
                    self.cancel_tool();
                } else {
                    // `finish_tool` legt die Kette an und meldet selbst, wenn
                    // noch Messpunkte fehlen.
                    crate::canvas::finish_tool(self, false);
                }
                return;
            }
            // Messpunkte ändern: die Klicks sind bereits ausgeführt, ESC
            // beendet nur die Reihe — die Auswahl bleibt stehen, damit
            // sichtbar bleibt, welche Maßkette bearbeitet wurde.
            if matches!(t.action.as_str(), "dim.point.add" | "dim.point.remove") {
                self.tool = None;
                return;
            }
        }
        let finisheable = self
            .tool
            .as_ref()
            .map(|t| {
                let punktfolge = matches!(
                    t.action.as_str(),
                    // Raumstempel: ESC **bestätigt** die Punktfolge wie Enter
                    // (T0220) — bisher brach ESC den Stempel ab, obwohl die
                    // Ecken schon gesetzt waren.
                    "draw.polyline" | "draw.spline" | "draw.path" | "modify.area.split" | "dim.room"
                ) || crate::canvas::ist_fuell_punkte(&t.action);
                punktfolge && t.pts.len() >= 2
            })
            .unwrap_or(false);
        if finisheable {
            // Die Ergebniszeile (`<Objekt> · <Zustand>`) schreibt der
            // Befehlsaufruf selbst — hier keine zweite Meldung.
            crate::canvas::finish_tool(self, false);
            return;
        }
        self.cancel_tool();
    }

    /// Läuft ein Füll-/Schraffur-Werkzeug im Modus „Punkte setzen"?
    ///
    /// Nur dort brechen Entf und Backspace die Eingabe ab; alle anderen
    /// Werkzeuge und Modi behalten ihre bisherige Tastenbehandlung.
    fn fill_points_running(&self) -> bool {
        self.tool
            .as_ref()
            .map(|t| crate::canvas::ist_fuell_punkte(&t.action))
            .unwrap_or(false)
    }

    /// Inventar ein-/ausblenden — ein Druck auf die Taste der Aktion
    /// `view.inventory` (Standard: E) schaltet zwischen offen und geschlossen.
    pub fn toggle_inventory(&mut self) {
        if self.ui.inventory_open {
            self.close_inventory();
        } else {
            self.ui.inventory_open = true;
        }
    }

    /// Schließt das Inventar. Gemeinsamer Pfad für Taste E, Schließen-Button
    /// und Fensterkreuz, damit kein Zustand aus dem Inventar hängen bleibt.
    pub fn close_inventory(&mut self) {
        self.ui.inventory_open = false;
        // Ein offenes Belegen (Hotbar-Slot oder Taste) endet mit dem Inventar —
        // sonst landete die nächste gewählte Funktion in einem unsichtbaren Ziel.
        self.ui.bind_key = None;
        // Nur Zieh-Vorgänge, die im Inventar begonnen haben, verwerfen —
        // eine Hotbar-Umbelegung per Ziehen läuft unabhängig weiter.
        if matches!(self.ui.slot_drag, Some((usize::MAX, _))) {
            self.ui.slot_drag = None;
        }
    }

    /// Schnellauswahl (T0060): ein Aufruf schaltet sie.
    ///
    /// Läuft bereits eine Funktion, wirkt sie wie ein **Abbruch** — danach ist
    /// keine Funktion aktiv. Läuft keine, wird die Schnellauswahl selbst aktiv;
    /// der nächste Klick auf ein Objekt startet dessen Erzeugerwerkzeug
    /// (`actions::erzeuger_fuer`). Bewusst nicht über ESC: der schließt eine
    /// Punktfolge ab, hier wird verworfen.
    pub fn schnellauswahl_umschalten(&mut self) {
        if self.tool.is_some() {
            self.cancel_tool();
            return;
        }
        // T0094: Auch das Einfügen ist eine laufende Eingabe. Die Schnellauswahl
        // bricht sie ab und wird dabei **nicht** selbst aktiv — genau wie beim
        // Werkzeug-Abbruch oben.
        if self.ghost.is_some() {
            self.cancel_ghost();
            return;
        }
        self.start_tool("modify.quickselect");
    }

    /// Ist die Schnellauswahl das laufende Werkzeug? Dann hebt die
    /// Zeichenfläche das Objekt unter dem Zeiger hervor, obwohl ein Werkzeug
    /// aktiv ist.
    pub fn schnellauswahl_aktiv(&self) -> bool {
        self.tool_id() == Some("modify.quickselect")
    }

    pub fn cancel_tool(&mut self) {
        self.hilfslinie = None;
        self.hilfslinie_modus = false;
        self.hilfslinie_anker = None;
        if self.tool.is_some() {
            self.tool = None;
            self.mitteilung(jjcad_core::i18n::tr("gui.app.abgebrochen"));
        }
        if self.ghost.take().is_some() {
            self.mitteilung(jjcad_core::i18n::tr("gui.app.platzieren_abgebrochen"));
        }
        // Laufendes Ziehen der Auswahl zurücknehmen (keine halb verschobene
        // Geometrie zurücklassen)
        self.cancel_move_drag();
        self.cancel_grip_drag();
        self.session_mut().clear_selection();
        self.select_drag = None;
        self.select_zwei = None;
    }

    pub fn bind_slot_action(&mut self, slot: usize, action: Option<String>) {
        // T0215: Belegt wird der Slot der Ebene, die die Hotbar gerade zeigt.
        self.prefs.set_slot_fuer(self.ui.hotbar_kombi, slot, action);
        let name = self.prefs.hotbar_presets[self.prefs.active_preset].name.clone();
        let _ = self.prefs.save();
        self.toast(jjcad_core::i18n::t_args_owned("gui.app.hotbar_slot_x_belegt_x", &[("wert1", format!("{}", slot + 1)), ("name", format!("{}", name))]));
    }

    // ---- Dateien ----

    /// Dateien, die gerade über dem Fenster schweben — die eine Quelle für den
    /// Hinweis beim Ziehen (T0115). Der Prüfzustand `dateidrop` setzt sie von
    /// Hand, weil ein echter Zug headless nicht zu erzeugen ist.
    pub fn schwebende_dateien(&self, ctx: &egui::Context) -> Vec<PathBuf> {
        if let Some(demo) = &self.ui.drop_demo {
            return demo.clone();
        }
        ctx.input(|i| {
            i.raw
                .hovered_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        })
    }

    /// Im Fenster abgelegte Dateien annehmen (Drag an Drop, T0115).
    ///
    /// Die Prüfung des Formats und der Aufruf des passenden Imports stehen in
    /// [`dialogs::dateien_fallen_gelassen`] — hier wird nur der Eingang gelesen.
    fn dateien_ablegen(&mut self, ctx: &egui::Context) {
        let pfade: Vec<PathBuf> = ctx.input_mut(|i| {
            let dateien = std::mem::take(&mut i.raw.dropped_files);
            if !dateien.is_empty() {
                // Nach dem Ablegen schwebt nichts mehr: der Hinweis verschwindet
                // sofort, auch wenn das System kein „HoveredFileCancelled" schickt.
                i.raw.hovered_files.clear();
            }
            dateien.into_iter().filter_map(|f| f.path).collect()
        });
        if !pfade.is_empty() {
            dialogs::dateien_fallen_gelassen(self, &pfade);
        }
    }

    pub fn open_file_dialog(&mut self, mode: FileMode) {
        // Ein bereits offener Systemdialog bleibt; nicht doppelt öffnen.
        if self.ui.systemdialog.is_some() {
            return;
        }
        if self.prefs.dateidialog_system() && crate::systemdialog::verfuegbar() {
            let (dir, base) = self.starthilfe_fuer(mode);
            let anfrage = mode.anfrage(dir, &base);
            self.ui.systemdialog =
                Some((mode, Systemdialog::starten(anfrage, self.elternfenster.as_ref())));
            return;
        }
        self.open_own_file_dialog(mode);
    }

    /// Der eingebaute Dateidialog (auch der Rückfall, wenn der Systemdialog
    /// nicht zustande kommt).
    pub fn open_own_file_dialog(&mut self, mode: FileMode) {
        let (start_dir, base) = self.starthilfe_fuer(mode);
        self.ui.file_dialog = Some(FileDialog::new(mode, start_dir, &base));
    }

    /// Vorschlagsname für den Dateidialog. Ein Tastaturlayout (T0076) heißt wie
    /// das Layout, nicht wie die Zeichnung.
    fn starthilfe_fuer(&self, mode: FileMode) -> (PathBuf, String) {
        let (dir, base) = self.starthilfe();
        if matches!(mode, FileMode::ImportKeymap | FileMode::ExportKeymap) {
            let name = self
                .prefs
                .keymap_aktiv()
                .map(|i| self.prefs.keymap_presets[i].name.clone())
                .unwrap_or_else(|| jjcad_core::i18n::tr("gui.dialogs.tastaturlayout").to_string());
            return (dir, name);
        }
        (dir, base)
    }

    /// Startordner (letzter Ordner) und Name der Zeichnung für Dateidialoge.
    fn starthilfe(&self) -> (PathBuf, String) {
        let start_dir = self
            .doc()
            .path
            .as_ref()
            .and_then(|p| PathBuf::from(p).parent().map(|x| x.to_path_buf()))
            .or_else(|| {
                let l = self.prefs.last_dir.clone();
                if l.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(l))
                }
            })
            .unwrap_or_else(|| std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(".")));
        let base = self.doc().name.clone();
        (start_dir, base)
    }

    pub fn save_current(&mut self, save_as: bool) {
        if save_as || self.doc().path.is_none() {
            self.open_file_dialog(FileMode::SaveJjcad);
            return;
        }
        let path = PathBuf::from(self.doc().path.clone().unwrap());
        match self.session_mut().save(&path) {
            Ok(_) => {
                self.prefs.add_recent(&path.to_string_lossy());
                let _ = self.prefs.save();
                // T0129: der neueste Stand ist jetzt der Dateiinhalt.
                let aktiv = self.active;
                self.sicherung_dateistand(aktiv);
                self.toast(jjcad_core::i18n::t_args_owned("gui.app.datei_gespeichert_x", &[("wert1", format!("{}", path.display()))]));
            }
            Err(e) => self.toast(e),
        }
    }

    /// Blöcke aus der Benutzerbibliothek (`<Wurzel>/blocks`, T0127) in die
    /// aktive Zeichnung übernehmen. Vorhandene Kennungen und Namen bleiben
    /// stehen — die Systembibliothek gewinnt.
    ///
    /// Eine unlesbare Blockdatei wird gemeldet (T0128) und übersprungen; sie
    /// reißt die übrige Bibliothek nicht mit.
    pub fn benutzerbloecke_einhaengen(&mut self) {
        let dir = self.prefs.blocks_dir();
        let (geladen, meldungen) = jjcad_core::daten::lade_benutzerbloecke_mit_meldungen(&dir);
        if !meldungen.is_empty() {
            let mut grund = meldungen[0].clone();
            if meldungen.len() > 1 {
                grund = format!("{grund} (+{})", meldungen.len() - 1);
            }
            self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.blockdatei_uebersprungen_x",
                &[("wert1", grund)],
            ));
        }
        if geladen.is_empty() {
            return;
        }
        let doc = &mut self.tabs[self.active].session.doc;
        for b in geladen {
            let vorhanden = doc
                .blocks
                .iter()
                .any(|x| x.id == b.id || x.name.eq_ignore_ascii_case(&b.name));
            if !vorhanden {
                doc.blocks.push(b);
            }
        }
    }

    /// Öffnet die Datenwurzel im Dateimanager des Betriebssystems (T0127,
    /// Menü Datei → „Datenordner öffnen“).
    pub fn datenordner_oeffnen(&mut self) {
        let pfad = jjcad_core::daten::wurzel();
        let _ = std::fs::create_dir_all(&pfad);
        let befehl = if cfg!(windows) {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(befehl).arg(&pfad).spawn() {
            Ok(_) => self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.datenordner_geoeffnet_x",
                &[("wert1", pfad.display().to_string())],
            )),
            Err(e) => self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.datenordner_nicht_geoeffnet_x",
                &[("wert1", e.to_string())],
            )),
        }
    }

    /// Den Sicherungsordner im Dateimanager öffnen (ersetzt den früheren
    /// Wiederherstellen-Dialog: der Ordner ist die Wahrheit).
    pub fn sicherungsordner_oeffnen(&mut self) {
        let pfad = self.prefs.sicherungen_dir();
        let _ = std::fs::create_dir_all(&pfad);
        let befehl = if cfg!(windows) {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(befehl).arg(&pfad).spawn() {
            Ok(_) => self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.sicherungsordner_geoeffnet_x",
                &[("wert1", pfad.display().to_string())],
            )),
            Err(e) => self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.datenordner_nicht_geoeffnet_x",
                &[("wert1", e.to_string())],
            )),
        }
    }

    /// T0135: Dienst starten, der Aufträge einer zweiten Instanz entgegennimmt.
    ///
    /// Die zweite Instanz reicht ihre Zeichnung hierher weiter, statt eine
    /// Meldung zu zeigen. `wecken` lässt die Oberfläche neu zeichnen, sonst
    /// bliebe die geöffnete Zeichnung bis zur nächsten Eingabe unsichtbar.
    pub fn instanzdienst_starten(&mut self, ctx: &egui::Context) {
        let wecken = {
            let c = ctx.clone();
            move || c.request_repaint()
        };
        self.instanz = jjcad_core::instanz::dienst_starten(&jjcad_core::daten::wurzel(), wecken);
    }

    /// Verarbeitet die Aufträge einer zweiten Instanz (T0135, T0224).
    ///
    /// Eine Zeichnung wird als Reiter geöffnet, ohne Datei kommt nur das
    /// Fenster nach vorn. **Beides** holt das Fenster nach vorn: vor T0224 tat
    /// das nur der Auftrag ohne Datei — beim Doppelklick im Dateibrowser wurde
    /// die Zeichnung zwar geöffnet, blieb aber hinter dem Browser.
    pub fn auftraege_bearbeiten(
        &mut self,
        ctx: &egui::Context,
        auftraege: Vec<jjcad_core::instanz::Auftrag>,
        jetzt: f64,
    ) {
        for a in auftraege {
            match a {
                jjcad_core::instanz::Auftrag::Oeffnen { pfad, token } => {
                    self.load_document(&pfad);
                    self.fenster_nach_vorn(ctx, jetzt, token);
                }
                jjcad_core::instanz::Auftrag::Zeigen { token } => {
                    self.fenster_nach_vorn(ctx, jetzt, token)
                }
            }
        }
    }

    /// Will das Fenster nach vorn holen (T0224).
    ///
    /// Drei Schritte, weil kein einzelner auf allen Systemen genügt:
    /// `Minimized(false)` holt ein eingeklapptes Fenster zurück (ein
    /// minimiertes Fenster nimmt keinen Fokus an), `Focus` gibt den Wunsch an
    /// den Fenstermanager (X11: `_NET_ACTIVE_WINDOW`, Windows und macOS: ihr
    /// jeweiliger Weg). Der Wunsch wird nicht einmalig gestellt, sondern von
    /// [`App::nach_vorn_betreuen`] über einige Bilder nachgehalten.
    pub fn fenster_nach_vorn(
        &mut self,
        ctx: &egui::Context,
        jetzt: f64,
        token: Option<String>,
    ) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.nach_vorn = Some(NachVorn {
            seit: jetzt,
            gemeldet: false,
            token,
            nativ: false,
        });
    }

    /// Übernimmt das Aktivierungstoken des eigenen Starts (T0224).
    ///
    /// Wird jjcad frisch aus dem Dateibrowser gestartet, liegt das Token in der
    /// Umgebung; eframe gibt es nicht an die Fenstererzeugung weiter. Hier wird
    /// es dem ersten Bild mitgegeben, damit [`App::nach_vorn_betreuen`] es
    /// vorlegt.
    pub fn start_mit_token(&mut self, token: Option<String>) {
        if token.is_some() {
            self.nach_vorn = Some(NachVorn {
                seit: 0.0,
                gemeldet: false,
                token,
                nativ: false,
            });
        }
    }

    /// Hält einen offenen Wunsch „nach vorn“ nach (T0224).
    ///
    /// Solange das Fenster den Fokus nicht hat, wird der Wunsch wiederholt;
    /// nach [`NACH_VORN_MELDEN`] meldet die Anwendung zusätzlich Aufmerksamkeit
    /// an. Unter **Wayland** ist `Focus` wirkungslos — dort entscheidet der
    /// Compositor, und `RequestUserAttention` ist der einzige Weg, ihm ein
    /// Anliegen vorzulegen (`xdg_activation`). Hat das Fenster den Fokus
    /// (oder ist [`NACH_VORN_DAUER`] verstrichen), endet der Wunsch.
    pub fn nach_vorn_betreuen(&mut self, ctx: &egui::Context, jetzt: f64) -> Option<String> {
        let Some(mut stand) = self.nach_vorn.clone() else {
            return None;
        };
        if ctx.input(|i| i.focused) {
            self.nach_vorn = None;
            return None;
        }
        let alter = jetzt - stand.seit;
        if alter > NACH_VORN_DAUER {
            self.nach_vorn = None;
            return None;
        }
        // Das Token gilt einmal: hier zurückgeben, der Aufrufer legt es dem
        // Compositor vor (er hat das Fenster, dieses Modul nicht).
        let mut vorzulegen = None;
        if !stand.nativ {
            stand.nativ = true;
            vorzulegen = stand.token.clone();
            self.nach_vorn = Some(stand.clone());
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        if !stand.gemeldet && alter >= NACH_VORN_MELDEN {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Critical,
            ));
            stand.gemeldet = true;
            self.nach_vorn = Some(stand);
        }
        // Ohne Eingabe käme kein Bild — der Wunsch braucht aber Bilder, um
        // nachgehalten zu werden.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        vorzulegen
    }

    /// Sicherung (T0129): Pool öffnen, falls noch nicht geschehen.
    pub fn sicherung_pool(&mut self) -> &mut jjcad_core::sicherung::Sicherung {
        if self.sicherung.is_none() {
            self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                &self.prefs.sicherungen_dir(),
            ));
        }
        self.sicherung
            .as_mut()
            .expect("Pool wurde eben geöffnet")
    }

    /// Sicherung (T0129): Ist das Intervall abgelaufen, sichert der Pool den
    /// Stand **aller** offenen Dokumente, deren Inhalt sich geändert hat.
    ///
    /// Die Zieldatei wird dabei nicht angefasst — geschrieben wird nur in den
    /// Pool. Der Aufruf läuft aus `update`; Tests rufen ihn mit eigener Zeit
    /// und eigenem Pool auf.
    pub fn sicherung_tick(&mut self, jetzt: f64) {
        let einst = self.prefs.sicherung_einstellungen();
        let abstand = (einst.intervall_min.max(1) as f64) * 60.0;
        if jetzt - self.sicherung_zuletzt < abstand {
            return;
        }
        self.sicherung_zuletzt = jetzt;
        let mut fehler: Option<String> = None;
        // Der Pool wird für die Schleife ausgeliehen: `sicherung_pool` borgt
        // `self` mut, die Reiter brauchen es ebenfalls.
        if self.sicherung.is_none() {
            self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                &self.prefs.sicherungen_dir(),
            ));
        }
        if let Some(mut pool) = self.sicherung.take() {
            for tab in &mut self.tabs {
                if let Err(e) = pool.sichern(&mut tab.session.doc, &einst) {
                    fehler = Some(e);
                }
            }
            self.sicherung = Some(pool);
        }
        if let Some(grund) = fehler {
            self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.sicherung_fehler_x",
                &[("grund", grund)],
            ));
        }
    }

    /// Sichert **sofort** alle Reiter mit ungespeicherten Änderungen (T0129) —
    /// unabhängig vom eingestellten Intervall. Wird beim Beenden gerufen, damit
    /// der nächste Start die Arbeit wiederfindet; die Zieldatei bleibt unberührt.
    pub fn sicherung_jetzt(&mut self) {
        let einst = self.prefs.sicherung_einstellungen();
        let mut fehler: Option<String> = None;
        if self.sicherung.is_none() {
            self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                &self.prefs.sicherungen_dir(),
            ));
        }
        if let Some(mut pool) = self.sicherung.take() {
            for tab in &mut self.tabs {
                if !tab.session.doc.modified {
                    continue;
                }
                if let Err(e) = pool.sichern(&mut tab.session.doc, &einst) {
                    fehler = Some(e);
                }
            }
            self.sicherung = Some(pool);
        }
        if let Some(grund) = fehler {
            eprintln!("Sicherung beim Beenden: {grund}");
        }
    }

    /// Nach dem Speichern (und nach „Verwerfen“): der neueste Stand ist der
    /// Inhalt der Zieldatei (T0129). Beim Wiederherstellen gewinnt dann die
    /// Datei — ungespeicherte Stände bleiben nur, solange es sie gibt.
    pub fn sicherung_dateistand(&mut self, idx: usize) {
        let einst = self.prefs.sicherung_einstellungen();
        if self.sicherung.is_none() {
            self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                &self.prefs.sicherungen_dir(),
            ));
        }
        if let Some(mut pool) = self.sicherung.take() {
            if let Some(tab) = self.tabs.get_mut(idx) {
                let _ = pool.sichern_dateistand(&mut tab.session.doc, &einst);
            }
            self.sicherung = Some(pool);
        }
    }

    /// T0223: Das Fenster „Wiederherstellen“ öffnen (Einstellungen ▸ Sicherung).
    ///
    /// Der Pool wird dabei **neu** vom Datenträger gelesen: im Fenster soll
    /// stehen, was im Sicherungsordner liegt — nicht der Stand vom Start.
    pub fn oeffne_wiederherstellen(&mut self) {
        self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
            &self.prefs.sicherungen_dir(),
        ));
        self.ui.wiederherstellen = Some(Wiederherstellen::default());
    }

    /// T0223: Die Zeichnungen im Sicherungsordner — ein Eintrag je Zeichnung
    /// mit der Zahl ihrer Stände.
    pub fn wiederherstellen_dateien(&mut self) -> Vec<jjcad_core::sicherung::DateiEintrag> {
        self.sicherung_pool().zeichnungen()
    }

    /// T0223: Den Sicherungsordner neu einlesen, ohne die Auswahl zu verlieren.
    pub fn wiederherstellen_neu_laden(&mut self) {
        self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
            &self.prefs.sicherungen_dir(),
        ));
        self.ui.sicherung_info = None;
        if let Some(w) = self.ui.wiederherstellen.as_mut() {
            w.vorschau = None;
            w.fehler = None;
        }
        self.wiederherstellen_vorschau_nachziehen();
    }

    /// T0223: Den Inhalt des gewählten Standes für die Vorschau laden.
    ///
    /// Gelesen wird **einmal je Auswahl**: der Zustand merkt sich Zyklus und
    /// Dokument, das Fenster fragt in jedem Bild nur nach dem Unterschied.
    pub fn wiederherstellen_vorschau_nachziehen(&mut self) {
        let Some(w) = self.ui.wiederherstellen.as_ref() else {
            return;
        };
        let ziel = w.zyklus;
        let schon = w.vorschau.as_ref().map(|(z, _)| *z);
        if schon == ziel {
            return;
        }
        let ergebnis: Result<Option<(u64, jjcad_core::model::Document)>, String> = match ziel {
            None => Ok(None),
            Some(z) => {
                let pool = self.sicherung_pool();
                match pool.stand_mit_zyklus(z).cloned() {
                    Some(stand) => pool.laden(&stand).map(|d| Some((z, d))),
                    None => Err(jjcad_core::i18n::tr("wiederherstellen.stand_fehlt").to_string()),
                }
            }
        };
        if let Some(w) = self.ui.wiederherstellen.as_mut() {
            match ergebnis {
                Ok(v) => {
                    w.vorschau = v;
                    w.fehler = None;
                }
                Err(e) => {
                    w.vorschau = None;
                    w.fehler = Some(jjcad_core::i18n::t_args_owned(
                        "wiederherstellen.fehler",
                        &[("grund", e)],
                    ));
                }
            }
        }
    }

    /// T0223: Einen Stand als Reiter öffnen — „diese Zeichnung wieder öffnen“.
    ///
    /// Der Stand trägt den Speicherort der Zieldatei; der Reiter bekommt ihn
    /// mit und gilt als **geändert**, damit „Speichern“ die Datei bewusst
    /// überschreibt und das Schließen nachfragt (wie
    /// [`App::sitzung_wiederherstellen`]). Liegt die Zieldatei schon in einem
    /// Reiter, wird er ersetzt, statt einen zweiten auf denselben Pfad zu legen.
    /// Liefert den Zeitstempel des Standes für die Meldung.
    pub fn wiederherstellen_stand_oeffnen(&mut self, zyklus: u64) -> Result<String, String> {
        let (stand, doc) = {
            let pool = self.sicherung_pool();
            let stand = pool.stand_mit_zyklus(zyklus).cloned().ok_or_else(|| {
                jjcad_core::i18n::tr("wiederherstellen.stand_fehlt").to_string()
            })?;
            let doc = pool.laden(&stand)?;
            (stand, doc)
        };
        let mut doc = doc;
        doc.merke_standard_breiten(&self.prefs.layer_weights);
        doc.modified = true;
        let ziel = if stand.pfad.trim().is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(&stand.pfad))
        };
        doc.path = ziel.as_ref().map(|p| p.to_string_lossy().to_string());
        if doc.name.trim().is_empty() {
            doc.name = stand.name.clone();
        }
        // Die Zieldatei liegt schon offen: der Stand ersetzt ihren Inhalt im
        // vorhandenen Reiter. Ein zweiter Reiter auf denselben Pfad würde beim
        // Speichern den ersten überschreiben.
        if let Some(pfad) = &ziel {
            if let Some(i) = self.reiter_der_datei(pfad) {
                crate::textedit::beenden(self);
                let tab = &mut self.tabs[i];
                tab.session = Session::with_doc(doc);
                tab.tab = TabKind::Model;
                tab.rev += 1;
                tab.cache = None;
                tab.hover = None;
                if i != self.active {
                    self.switch_doc(i);
                } else {
                    self.datei_nachziehen();
                }
                self.ui.sicherung_info = None;
                return Ok(stand.zeit.clone());
            }
        }
        let tab = DocTab {
            session: Session::with_doc(doc),
            tab: TabKind::Model,
            view_model: View::default(),
            sheet_views: HashMap::new(),
            block_views: HashMap::new(),
            offene_bloecke: Vec::new(),
            geschlossene_blaetter: Vec::new(),
            block_original: HashMap::new(),
            rev: 0,
            cache: None,
            hover: None,
        };
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.datei_nachziehen();
        self.fit_view();
        self.ui.sicherung_info = None;
        Ok(stand.zeit.clone())
    }

    /// T0188: Den Entwurf der Sicherungseinstellungen übernehmen.
    ///
    /// Ginge dabei etwas verloren, wird **nicht** übernommen, sondern die
    /// Warnung gestellt (`UiState::sicherung_warnung`) — gelöscht wird erst
    /// nach der Bestätigung.
    pub fn sicherung_uebernehmen(&mut self) {
        let Some(ent) = self.ui.sicherung_entwurf else {
            return;
        };
        if ent == self.prefs.sicherung_einstellungen() {
            self.ui.sicherung_entwurf = None;
            return;
        }
        let verlust = self.sicherung_pool().wuerde_verlieren(&ent);
        if verlust > 0 {
            self.ui.sicherung_warnung = Some((ent, verlust));
            self.ui.sicherung_warnung_neu = true;
            return;
        }
        self.sicherung_anwenden(ent);
    }

    /// Schreibt die Einstellungen, sichert sie und dünnt sofort aus.
    pub fn sicherung_anwenden(&mut self, ent: jjcad_core::sicherung::Einstellungen) {
        self.prefs.sicherung_verlauf = ent.verlauf;
        self.prefs.sicherung_plaetze = ent.plaetze;
        self.prefs.sicherung_stunden = ent.stunden;
        self.prefs.sicherung_intervall = ent.intervall_min;
        let _ = self.prefs.save();
        let weg = self.sicherung_pool().ausduennen(&ent);
        self.ui.sicherung_entwurf = None;
        self.ui.sicherung_warnung = None;
        self.ui.sicherung_info = None;
        if weg > 0 {
            self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.sicherung_geloescht_x",
                &[("anzahl", weg.to_string())],
            ));
        }
    }

    /// T0188: Stände löschen, deren Zeichnung es nicht mehr gibt.
    pub fn sicherung_aufraeumen_jetzt(&mut self) {
        let zyklen: Vec<u64> = self
            .sicherung_pool()
            .ohne_zeichnung()
            .iter()
            .map(|s| s.zyklus)
            .collect();
        let weg = self.sicherung_pool().entferne_zyklen(&zyklen);
        self.ui.sicherung_aufraeumen = None;
        self.ui.sicherung_info = None;
        self.toast(jjcad_core::i18n::t_args_owned(
            "gui.app.sicherung_aufgeraeumt_x",
            &[("anzahl", weg.to_string())],
        ));
    }

    /// „Verwerfen“ beim Schließen eines Reiters (R3.5): der letzte Zustand der
    /// verworfenen Arbeit wird als Stand abgelegt und als `verworfen`
    /// markiert; danach wird der Dateiinhalt als neuester Stand geschrieben,
    /// damit beim nächsten Öffnen die Datei gilt.
    pub fn verwerfen_stand(&mut self, idx: usize) {
        let Some(pfad) = self
            .tabs
            .get(idx)
            .and_then(|t| t.session.doc.path.clone())
        else {
            return;
        };
        let einst = self.prefs.sicherung_einstellungen();
        if self.sicherung.is_none() {
            self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                &self.prefs.sicherungen_dir(),
            ));
        }
        if let Some(mut pool) = self.sicherung.take() {
            // Zuerst die verworfene Arbeit sichern — der Reiter trägt sie noch.
            if let Some(tab) = self.tabs.get_mut(idx) {
                let _ = pool.sichern_verworfen(&mut tab.session.doc, &einst);
            }
            if let Ok(mut doc) = jjcad_core::persist::load_document(Path::new(&pfad)) {
                let _ = pool.sichern_dateistand(&mut doc, &einst);
            }
            self.sicherung = Some(pool);
        }
    }

    /// R2.3: Entscheidung im Konflikt um eine extern geänderte Zieldatei.
    ///
    /// `stand_nehmen = true`: die ungespeicherte Arbeit aus dem
    /// Sicherungsordner gewinnt, der Reiter gilt als geändert. `false`: die
    /// Datei gewinnt, ihr Inhalt wird als neuester Stand festgehalten — sonst
    /// fragte der nächste Start erneut.
    pub fn konflikt_entscheiden(&mut self, stand_nehmen: bool) {
        let Some(k) = self.ui.konflikt.take() else {
            return;
        };
        let einst = self.prefs.sicherung_einstellungen();
        if self.sicherung.is_none() {
            self.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(
                &self.prefs.sicherungen_dir(),
            ));
        }
        let Some(mut pool) = self.sicherung.take() else {
            return;
        };
        if stand_nehmen {
            let stand = pool
                .staende_von(&k.datei)
                .into_iter()
                .find(|s| s.zyklus == k.zyklus)
                .cloned();
            if let Some(stand) = stand {
                if let Ok(mut doc) = pool.laden(&stand) {
                    doc.path = Some(k.pfad.clone());
                    doc.name = k.name.clone();
                    doc.modified = true;
                    doc.merke_standard_breiten(&self.prefs.layer_weights);
                    if let Some(tab) = self.tabs.get_mut(k.reiter) {
                        tab.session = Session::with_doc(doc);
                        tab.tab = TabKind::Model;
                        tab.rev += 1;
                        tab.cache = None;
                    }
                }
            }
        } else if let Ok(mut doc) = jjcad_core::persist::load_document(Path::new(&k.pfad)) {
            let _ = pool.sichern_dateistand(&mut doc, &einst);
        }
        self.sicherung = Some(pool);
    }

    pub fn load_document(&mut self, path: &PathBuf) {
        crate::textedit::beenden(self);
        // Dieselbe Datei nur einmal öffnen: zwei Reiter auf denselben Pfad
        // überschreiben sich beim Speichern gegenseitig.
        if let Some(i) = self.reiter_der_datei(path) {
            self.switch_doc(i);
            self.toast(jjcad_core::i18n::t_args_owned("gui.app.datei_bereits_offen_x", &[("wert1", format!("{}", path.file_name().unwrap_or_default().to_string_lossy()))]));
            return;
        }
        match Session::open(path) {
            Ok(mut s) => {
                // T0125: Voreinstellungen nur **merken** — die Ebenen der
                // geöffneten Zeichnung behalten ihre Breiten.
                s.doc.merke_standard_breiten(&self.prefs.layer_weights);
                // T0129: Liegt für diese Zeichnung ungespeicherte Arbeit im
                // Sicherungspool, gilt sie — sonst würde ein Doppelklick auf die
                // Datei die Arbeit von eben stillschweigend verwerfen.
                let id = s.doc.id.clone();
                let stand_doc = {
                    let pool = self.sicherung_pool();
                    ungespeicherter_stand(pool, &id, path)
                };
                if let Some((_stand, mut doc)) = stand_doc {
                    // R2.3: Wurde die Datei inzwischen von außen geändert, gilt
                    // der Klick des Benutzers auf **diese** Datei; der Stand
                    // bleibt im Pool liegen und ist dort einzeln zu öffnen.
                    if !self.sicherung_pool().extern_geaendert(&id, path) {
                        doc.path = Some(path.to_string_lossy().to_string());
                        doc.name = s.doc.name.clone();
                        doc.modified = true;
                        doc.merke_standard_breiten(&self.prefs.layer_weights);
                        s = Session::with_doc(doc);
                    }
                }
                let tab = DocTab {
                    session: s,
                    tab: TabKind::Model,
                    view_model: View::default(),
                    sheet_views: HashMap::new(),
                    block_views: HashMap::new(),
                    offene_bloecke: Vec::new(),
                    geschlossene_blaetter: Vec::new(),
                    block_original: HashMap::new(),
                    rev: 0,
                    cache: None,
                    hover: None,
                };
                // Unbenutzten Startreiter ersetzen statt einen zweiten anzulegen
                let pristine = self.tabs.len() == 1
                    && self.tabs[0].session.doc.entities.is_empty()
                    && self.tabs[0].session.doc.path.is_none()
                    && !self.tabs[0].session.doc.modified;
                if pristine {
                    self.tabs[0] = tab;
                    self.active = 0;
                } else {
                    self.tabs.push(tab);
                    self.active = self.tabs.len() - 1;
                }
                self.datei_nachziehen();
                self.prefs.add_recent(&path.to_string_lossy());
                if let Some(p) = path.parent() {
                    self.prefs.last_dir = p.to_string_lossy().to_string();
                }
                let _ = self.prefs.save();
                self.fit_view();
                self.benutzerbloecke_einhaengen();
                self.toast(jjcad_core::i18n::t_args_owned("gui.app.datei_geoeffnet_x", &[("wert1", format!("{}", path.file_name().unwrap_or_default().to_string_lossy()))]));
            }
            Err(e) => self.toast(e),
        }
    }

    /// Liegt dieser Pfad schon in einem Reiter? Verglichen wird der echte Pfad
    /// (`canonicalize`), damit „./a.jjcad“ und „/x/a.jjcad“ derselbe Reiter sind.
    fn reiter_der_datei(&self, path: &PathBuf) -> Option<usize> {
        let ziel = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        self.tabs.iter().position(|t| {
            t.session
                .doc
                .path
                .as_ref()
                .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p)) == ziel)
                .unwrap_or(false)
        })
    }

    /// Einen neuen Datei-Reiter anlegen. Die bisherige Zeichnung bleibt in
    /// ihrem Reiter stehen — „Neu“ schließt nichts mehr (T0073).
    ///
    /// T0162: Ist in den Einstellungen eine Vorlage angegeben, beginnt die
    /// neue Datei als **Kopie** dieser Datei — Objekte, Ebenen, Blätter,
    /// Blöcke und Zeichnungseinstellungen kommen aus der Vorlage, der Pfad
    /// bleibt leer. Eine unlesbare Vorlage wird gemeldet und die Datei beginnt
    /// leer; „Neu“ darf weder still scheitern noch hängen bleiben.
    pub fn new_document(&mut self) {
        crate::textedit::beenden(self);
        let mut s = Session::new();
        let mut aus_vorlage = false;
        let mut vorlage_fehler = false;
        if let Some(pfad) = self.prefs.vorlage_datei() {
            match jjcad_core::persist::lade_vorlage(&pfad) {
                Ok(doc) => {
                    s = Session::with_doc(doc);
                    aus_vorlage = true;
                }
                // Der Grund steht nach der Erfolgsmeldung in der Statuszeile —
                // sonst überschriebe „Datei · neu“ die einzige Spur des Fehlers.
                Err(_) => vorlage_fehler = true,
            }
        }
        if aus_vorlage {
            // Die Ebenen der Vorlage behalten ihre Breiten — wie beim Öffnen
            // einer Datei werden die Voreinstellungen nur **gemerkt**.
            s.doc.merke_standard_breiten(&self.prefs.layer_weights);
        } else {
            // T0125: „Neu“ bekommt die eingestellte Linienstärke je Objektart.
            s.doc.setze_standard_breiten(&self.prefs.layer_weights);
            s.doc.settings.snap.wall_edges = self.prefs.snap_wall_edges;
        }
        s.doc.name = self.freier_dateiname("Unbenannt");
        let tab = DocTab {
            session: s,
            tab: TabKind::Model,
            view_model: View::default(),
            sheet_views: HashMap::new(),
            block_views: HashMap::new(),
            offene_bloecke: Vec::new(),
            geschlossene_blaetter: Vec::new(),
            block_original: HashMap::new(),
            rev: 0,
            cache: None,
            hover: None,
        };
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.datei_nachziehen();
        self.fit_view();
        self.benutzerbloecke_einhaengen();
        self.toast(jjcad_core::i18n::tr("gui.app.datei_neu"));
        if vorlage_fehler {
            self.toast(jjcad_core::i18n::tr("settings.vorlage_nicht_lesbar"));
        }
    }

    /// T0125: Voreinstellungen der Linienstärke übernehmen.
    ///
    /// Eine **leere, ungespeicherte** Zeichnung bekommt sie sofort — sie ist
    /// noch kein Plan, und der Nutzer soll die Wirkung ohne „Neu“ sehen. Eine
    /// geöffnete Zeichnung merkt sie nur für **neue** Standardebenen; ihre
    /// vorhandenen Ebenen bleiben unverändert.
    pub fn voreinstellungen_uebernehmen(&mut self) {
        let breiten = self.prefs.layer_weights.clone();
        let leer = self.doc().entities.is_empty()
            && self.doc().path.is_none()
            && !self.doc().modified;
        if leer {
            self.doc_mut().setze_standard_breiten(&breiten);
        } else {
            self.doc_mut().merke_standard_breiten(&breiten);
        }
    }

    /// Name für eine neue, noch ungespeicherte Zeichnung, der im Reiterstreifen
    /// nicht doppelt vorkommt („Unbenannt“, „Unbenannt 2“, …).
    fn freier_dateiname(&self, basis: &str) -> String {
        let belegt = |name: &str| self.tabs.iter().any(|t| t.session.doc.name == name);
        if !belegt(basis) {
            return basis.to_string();
        }
        for n in 2..=999 {
            let kandidat = format!("{basis} {n}");
            if !belegt(&kandidat) {
                return kandidat;
            }
        }
        basis.to_string()
    }

    /// Auf einen anderen Datei-Reiter wechseln.
    pub fn switch_doc(&mut self, idx: usize) {
        if idx >= self.tabs.len() || idx == self.active {
            return;
        }
        crate::textedit::beenden(self);
        self.active = idx;
        self.datei_nachziehen();
    }

    /// Alles, was an der aktiven Datei hängt, nach einem Reiterwechsel
    /// nachziehen: Werkzeug, Vorschau und Züge gehören zur alten Datei und
    /// werden verworfen; Werkzeugvorgaben, Raster und Bearbeitungsraum kommen
    /// aus der neuen. Die Auswahl bleibt je Datei erhalten (sie steckt in der
    /// `Session` des Reiters).
    fn datei_nachziehen(&mut self) {
        self.tool = None;
        self.ghost = None;
        self.select_drag = None;
        self.select_zwei = None;
        self.grip_drag = None;
        self.move_drag = None;
        self.halt.clear();
        self.paint = crate::canvas::PaintPreview::default();
        self.ui.order_pick = None;
        self.options = ToolOptions::from_doc(self.doc());
        self.ui.show_grid = self.doc().settings.show_grid;
        self.sync_space();
    }

    /// Datei-Reiter schließen. Der letzte hinterlässt eine leere Zeichnung,
    /// statt die Anwendung zu beenden.
    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        crate::textedit::beenden(self);
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.active = 0;
            self.new_document();
            return;
        }
        if idx < self.active {
            self.active -= 1;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        self.datei_nachziehen();
    }

    /// Schließen anfordern: weicht der Reiter von seiner Zieldatei ab, fragt
    /// der Dialog „Datei schließen“ nach (`UiState::confirm_close`), sonst zu.
    ///
    /// Maßgeblich ist der **Inhalt**, nicht der Merker `Document::modified`
    /// (T0130): ein Reiter aus dem Sicherungspool ist ungespeichert, obwohl ihn
    /// niemand angefasst hat, und ein gespeicherter Reiter mit von Hand
    /// gesetztem Merker soll trotzdem ohne Rückfrage schließen.
    pub fn request_close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        if self.weicht_von_zieldatei(idx) {
            self.ui.confirm_close = Some(idx);
        } else {
            self.close_tab(idx);
        }
    }

    /// Weicht der Reiter von seiner Zieldatei ab? (T0130)
    ///
    /// Mit Zieldatei: Vergleich des Inhalts gegen die Datei auf der Platte
    /// (gleiche Platzierung, gleiche Einstellungen — der Vergleich läuft über
    /// die gespeicherte Form). Eine fehlende oder unlesbare Datei gilt als
    /// Abweichung: dann gibt es keinen Stand, gegen den der Reiter gleich wäre.
    ///
    /// Ohne Zieldatei gibt es nichts, wogegen er gleich sein könnte — er weicht
    /// ab, sobald er **Inhalt** hat. Eine frische leere Zeichnung schließt
    /// damit ohne Rückfrage, ein wiederhergestellter ungespeicherter Reiter
    /// fragt nach.
    pub fn weicht_von_zieldatei(&self, idx: usize) -> bool {
        let Some(tab) = self.tabs.get(idx) else {
            return false;
        };
        let doc = &tab.session.doc;
        let Some(pfad) = doc.path.as_deref() else {
            return !doc.entities.is_empty();
        };
        match persist::load_document(Path::new(pfad)) {
            Ok(datei) => match (persist::to_json(doc), persist::to_json(&datei)) {
                (Ok(jetzt), Ok(gespeichert)) => jetzt != gespeichert,
                _ => true,
            },
            Err(_) => true,
        }
    }

    /// Datei speichern und den Reiter schließen. Ohne Speicherort gibt es nichts
    /// zu speichern — der Dialog bietet dann nur „Verwerfen“ an.
    pub fn close_tab_speichern(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        let Some(pfad) = self.tabs[idx].session.doc.path.clone() else {
            return;
        };
        let pfad = PathBuf::from(pfad);
        match self.tabs[idx].session.save(&pfad) {
            Ok(_) => {
                self.prefs.add_recent(&pfad.to_string_lossy());
                let _ = self.prefs.save();
                // T0129: der neueste Stand ist jetzt der Dateiinhalt — vor dem
                // Schließen, damit der Reiter nicht mit ungespeichertem Stand geht.
                self.sicherung_dateistand(idx);
                self.toast(jjcad_core::i18n::t_args_owned("gui.app.datei_gespeichert_x", &[("wert1", format!("{}", pfad.display()))]));
                self.close_tab(idx);
            }
            Err(e) => self.toast(e),
        }
    }

    /// „Speichern und schließen“ aus der Rückfrage vor dem Schließen (T0143).
    ///
    /// Mit Speicherort wird sofort dorthin geschrieben. Eine Zeichnung **ohne**
    /// Speicherort (neue Datei, aus dem Sicherungspool geholt) kann nicht still
    /// gesichert werden — hier fragt der Dateidialog nach dem Ziel. Der Reiter
    /// wird dafür aktiv, damit der Dialog auf **ihn** zeigt; geschlossen wird er
    /// erst nach dem Sichern (`dialogs::datei_gewaehlt`), ein Abbruch lässt ihn
    /// offen.
    pub fn close_tab_speichern_mit_ziel(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        if self.tabs[idx].session.doc.path.is_some() {
            self.close_tab_speichern(idx);
            return;
        }
        self.switch_doc(idx);
        self.ui.schliessen_nach_speichern = Some(idx);
        self.open_file_dialog(FileMode::SaveJjcad);
    }

    // ---- Sitzung (T0130) ----

    /// Sammelt den Zustand für `sitzung.json`: Reiter in Reihenfolge (Kennung,
    /// Zieldatei, Name), je Reiter Ansicht und aktives Blatt, dazu der aktive
    /// Reiter und die Fensterlage.
    ///
    /// Auswahl, Undo-Verlauf, Werkzeugwahl und Zwischenablage gehören bewusst
    /// **nicht** dazu — sie sind Arbeit, nicht Zeichnung.
    pub fn sitzung_sammeln(&self) -> sitzung::Sitzung {
        let reiter = self
            .tabs
            .iter()
            .map(|t| {
                let ansicht = match &t.tab {
                    TabKind::Model => t.view_model,
                    TabKind::Sheet(id) => *t.sheet_views.get(id).unwrap_or(&t.view_model),
                    // Ein Blockeditor gehört zur Sitzung der Datei, nicht zu
                    // ihrem Fensterzustand — die Ansicht zählt hier nicht.
                    TabKind::Block(_) => t.view_model,
                };
                sitzung::Reiter {
                    id: t.session.doc.id.clone(),
                    datei: t.session.doc.path.clone(),
                    name: t.session.doc.name.clone(),
                    ansicht: ansicht_als_wert(ansicht),
                    blatt: match &t.tab {
                        TabKind::Model => None,
                        TabKind::Sheet(id) => Some(*id),
                        TabKind::Block(_) => None,
                    },
                }
            })
            .collect();
        sitzung::Sitzung {
            fenster: self.fensterlage(),
            aktiv: self.active,
            reiter,
            ..sitzung::Sitzung::default()
        }
    }

    /// Fenstergröße, -lage und -zustand, wie egui sie meldet (T0141).
    /// `None`, solange es kein Fenster gibt (Tests, headless) — dann bleibt die
    /// gespeicherte Lage stehen, statt von einem leeren Wert überschrieben zu
    /// werden.
    ///
    /// Unter Wayland meldet das Fenstersystem keine Position; dann steht die
    /// Größe in der Sitzung und `x`/`y` bleiben ohne Merker.
    ///
    /// Im Vollbild und maximiert kommt die **Größe** aus `fenster_normal`: die
    /// aktuelle wäre die Bildschirmgröße.
    fn fensterlage(&self) -> Option<sitzung::Fensterlage> {
        let (inner, outer, vollbild, maximiert) = self.ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.inner_rect,
                vp.outer_rect,
                vp.fullscreen.unwrap_or(self.ui.fullscreen),
                vp.maximized.unwrap_or(false),
            )
        });
        let mut lage = lage_aus(inner, outer, self.fenster_ausschnitt())?;
        lage.vollbild = vollbild;
        // Ein Vollbild meldet unter KWin oft zusätzlich „maximiert“; beim
        // Wiederherstellen zählt der Vollbild-Zustand.
        lage.maximiert = maximiert && !vollbild;
        if vollbild || maximiert {
            if let Some(normal) = self.fenster_normal {
                lage = sitzung::Fensterlage {
                    breite: normal.breite,
                    hoehe: normal.hoehe,
                    x: normal.x,
                    y: normal.y,
                    mit_lage: normal.mit_lage,
                    maximiert: lage.maximiert,
                    vollbild: lage.vollbild,
                };
            }
        }
        Some(lage)
    }

    /// Merkt sich die Fenstergröße im Normalzustand (T0141, Turn 2) — nicht im
    /// Vollbild und nicht maximiert, dort ist sie die Bildschirmgröße. Läuft
    /// jedes Bild in [`App::ui`], damit beim Beenden der letzte normale Stand
    /// bereitsteht.
    fn fenster_merken(&mut self) {
        let (inner, outer, vollbild, maximiert) = self.ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.inner_rect,
                vp.outer_rect,
                vp.fullscreen.unwrap_or(self.ui.fullscreen),
                vp.maximized.unwrap_or(false),
            )
        });
        if vollbild || maximiert {
            return;
        }
        if let Some(lage) = lage_aus(inner, outer, self.fenster_ausschnitt()) {
            self.fenster_normal = Some(lage);
        }
    }

    /// Übernimmt die gespeicherte Fensterlage als „normalen“ Stand (T0141,
    /// Turn 2). Ein Start im Vollbild soll beim Beenden die vorige
    /// Fenstergröße behalten, nicht die Bildschirmgröße.
    pub fn setze_fensterlage(&mut self, lage: Option<sitzung::Fensterlage>) {
        let Some(l) = lage else { return };
        if !l.brauchbar() {
            return;
        }
        if l.vollbild || l.maximiert {
            self.fenster_start = Some(l);
        }
        self.fenster_normal = Some(sitzung::Fensterlage {
            maximiert: false,
            vollbild: false,
            ..l
        });
    }

    /// Setzt den Startzustand (Vollbild/maximiert) aus der Sitzung (T0141,
    /// Turn 2).
    ///
    /// Beim **Erzeugen** des Fensters nimmt der Fenstermanager den Wunsch nicht
    /// an: winit schickt die `_NET_WM_STATE`-Nachricht an ein noch nicht
    /// abgebildetes Fenster, KWin verwirft sie (gemessen — das Vollbild klappt
    /// dort nur, weil winit es bis zur Sichtbarkeit zurückhält). Deshalb wird
    /// der Wunsch in den ersten Bildern wiederholt, bis der gemeldete Zustand
    /// stimmt; nach [`FENSTER_START_GEDULD`] Sekunden ist Schluss, damit ein
    /// Fenstermanager ohne Maximieren nicht endlos belästigt wird. Sobald der
    /// Zustand steht, ist der Wunsch verbraucht — sonst ließe sich das Fenster
    /// nicht wieder verkleinern.
    fn fenster_start_anwenden(&mut self, ctx: &egui::Context) {
        let Some(ziel) = self.fenster_start else {
            return;
        };
        let (vollbild, maximiert) = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.fullscreen.unwrap_or(false),
                vp.maximized.unwrap_or(false),
            )
        });
        let erreicht = if ziel.vollbild { vollbild } else { maximiert };
        if erreicht {
            self.fenster_start = None;
            self.fenster_start_zeit = None;
            return;
        }
        let jetzt = ctx.input(|i| i.time);
        let start = *self.fenster_start_zeit.get_or_insert(jetzt);
        if jetzt - start > FENSTER_START_GEDULD {
            self.fenster_start = None;
            self.fenster_start_zeit = None;
            return;
        }
        if ziel.vollbild {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
    }

    /// Schreibt die Sitzung nach `<Wurzel>/sitzung.json` (beim Beenden).
    pub fn sitzung_schreiben(&mut self) -> Result<(), String> {
        let s = self.sitzung_sammeln();
        sitzung::speichern(&jjcad_core::daten::sitzung_datei(), &s)
    }

    /// Stellt die Reiter einer Sitzung her (T0130) und liefert die Meldungen
    /// für Reiter, die nicht wiederhergestellt werden konnten.
    ///
    /// Reihenfolge: Zieldatei von der Platte. Fehlt sie (oder ist sie
    /// unlesbar), kommt der neueste Stand aus dem Sicherungspool als
    /// **unbenannte, ungespeicherte** Zeichnung. Fehlt auch der Stand, wird der
    /// Reiter weggelassen und gemeldet. Bleibt nichts übrig, behält die
    /// Anwendung ihre frische leere Zeichnung.
    pub fn sitzung_wiederherstellen(
        &mut self,
        s: &sitzung::Sitzung,
        pool_dir: &Path,
    ) -> Vec<String> {
        let mut meldungen = Vec::new();
        let mut tabs: Vec<DocTab> = Vec::new();
        let pool = jjcad_core::sicherung::Sicherung::oeffnen(pool_dir);

        for r in &s.reiter {
            // 1. Zieldatei von der Platte. Ein Fehler wird noch nicht gemeldet:
            //    vielleicht liegt der Stand im Sicherungspool.
            let mut geladen: Option<Session> = None;
            let mut datei_grund: Option<String> = None;
            let mut konflikt: Option<Konflikt> = None;
            if let Some(pfad) = r.datei.as_deref() {
                match Session::open(Path::new(pfad)) {
                    Ok(mut sess) => {
                        // T0129: Gibt es einen neueren Stand mit **ungespeicherter**
                        // Arbeit, gilt sein Inhalt — der Pfad bleibt, damit
                        // „Speichern“ die Zieldatei überschreibt.
                        if let Some((stand, mut doc)) =
                            ungespeicherter_stand(&pool, &r.id, Path::new(pfad))
                        {
                            // R2.3: Hat eine fremde Anwendung die Zieldatei
                            // seither verändert, entscheidet der Benutzer.
                            // Bis dahin gilt die Datei — nichts wird
                            // überschrieben.
                            if pool.extern_geaendert(&r.id, Path::new(pfad)) {
                                konflikt = Some(Konflikt {
                                    reiter: tabs.len(),
                                    datei: r.id.clone(),
                                    pfad: pfad.to_string(),
                                    zyklus: stand.zyklus,
                                    zeit: stand.zeit.clone(),
                                    name: r.name.clone(),
                                    neu: true,
                                });
                            } else {
                                doc.path = Some(pfad.to_string());
                                doc.name = sess.doc.name.clone();
                                doc.modified = true;
                                doc.merke_standard_breiten(&self.prefs.layer_weights);
                                sess = Session::with_doc(doc);
                            }
                        }
                        sess.doc.merke_standard_breiten(&self.prefs.layer_weights);
                        geladen = Some(sess);
                    }
                    Err(grund) => datei_grund = Some(grund),
                }
            }

            // 2. Ohne lesbare Zieldatei: neuester Stand im Sicherungspool.
            let sess = match geladen {
                Some(sess) => sess,
                None => {
                    let stand = pool.neuester(&r.id).cloned();
                    match stand {
                        Some(stand) => match pool.laden(&stand) {
                            Ok(mut doc) => {
                                doc.path = None;
                                doc.name = "Unbenannt".into();
                                doc.modified = false;
                                doc.merke_standard_breiten(&self.prefs.layer_weights);
                                Session::with_doc(doc)
                            }
                            Err(grund) => {
                                meldungen.push(jjcad_core::i18n::t_args_owned(
                                    "gui.app.sitzung_reiter_weggefallen_x",
                                    &[("name", r.name.clone()), ("grund", grund)],
                                ));
                                continue;
                            }
                        },
                        None => {
                            // Genau **eine** Meldung je Reiter: der Grund der
                            // Zieldatei, sonst „kein Stand vorhanden“.
                            let grund = datei_grund.unwrap_or_else(|| {
                                jjcad_core::i18n::tr("gui.app.sitzung_stand_fehlt").to_string()
                            });
                            meldungen.push(jjcad_core::i18n::t_args_owned(
                                "gui.app.sitzung_reiter_weggefallen_x",
                                &[("name", r.name.clone()), ("grund", grund)],
                            ));
                            continue;
                        }
                    }
                }
            };

            // 3. Ansicht und aktives Blatt.
            let view = if r.ansicht.brauchbar() {
                View::von_wert(&r.ansicht)
            } else {
                View::default()
            };
            let mut tab = DocTab {
                session: sess,
                tab: TabKind::Model,
                view_model: view,
                sheet_views: HashMap::new(),
                block_views: HashMap::new(),
                offene_bloecke: Vec::new(),
                geschlossene_blaetter: Vec::new(),
                block_original: HashMap::new(),
                rev: 0,
                cache: None,
                hover: None,
            };
            if let Some(blatt) = r.blatt {
                if tab.session.doc.sheet(blatt).is_some() {
                    tab.tab = TabKind::Sheet(blatt);
                    tab.sheet_views.insert(blatt, view);
                }
            }
            tabs.push(tab);
            // R2.3: Konflikte werden nacheinander gefragt; der Reiterindex
            // stimmt, weil die Reiter in Sitzungsreihenfolge entstehen.
            if konflikt.is_some() && self.ui.konflikt.is_none() {
                self.ui.konflikt = konflikt;
            }
        }

        if !tabs.is_empty() {
            self.tabs = tabs;
            self.active = s.aktiv.min(self.tabs.len() - 1);
            self.datei_nachziehen();
            // Die wiederhergestellte Ansicht gilt ab dem ersten Bild. Ohne das
            // passt `update` die ersten Bilder selbst ein und überschriebe sie.
            self.started_at = 3.0;
        }
        meldungen
    }

    pub fn export_pdf_to(&mut self, path: PathBuf) {
        let idx = match &self.tab().tab {
            TabKind::Sheet(id) => self.doc().sheets.iter().position(|s| s.id == *id),
            TabKind::Model | TabKind::Block(_) => None,
        };
        let res = if let Some(i) = idx {
            pdf::export_pdf(self.doc(), &path, Some(i))
        } else {
            pdf::export_all_sheets_pdf(self.doc(), &path)
        };
        match res {
            Ok(_) => self.toast(jjcad_core::i18n::t_args_owned("gui.app.pdf_exportiert_x", &[("wert1", format!("{}", path.display()))])),
            Err(e) => self.toast(e),
        }
    }

    /// Öffnet den Speicherdialog für den **Plansatz** (T0218). Ohne Häkchen
    /// gibt es nichts zu exportieren — dann eine Meldung statt einer leeren PDF.
    pub fn plansatz_exportieren(&mut self) {
        if self.plansatz_ids().is_empty() {
            self.toast(jjcad_core::i18n::tr("gui.app.plansatz_leer"));
            return;
        }
        self.open_file_dialog(crate::dialogs::FileMode::ExportPlansatz);
    }

    /// Schreibt die Blätter mit Plansatz-Häkchen als **eine** mehrseitige PDF.
    pub fn export_plansatz_to(&mut self, path: PathBuf) {
        let ids = self.plansatz_ids();
        match pdf::export_sheets_pdf(self.doc(), &path, &ids) {
            Ok(_) => self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.pdf_exportiert_x",
                &[("wert1", format!("{}", path.display()))],
            )),
            Err(e) => self.toast(e),
        }
    }

    pub fn export_dxf_to(&mut self, path: PathBuf) {
        match dxf::export_dxf(self.doc(), &path) {
            Ok(_) => self.toast(jjcad_core::i18n::t_args_owned("gui.app.dxf_exportiert_x", &[("wert1", format!("{}", path.display()))])),
            Err(e) => self.toast(e),
        }
    }

    /// T0235: Bereich für den PNG-Export über die Diagonale wählen.
    ///
    /// Der Bereich ist immer ein Rechteck: der erste Klick setzt die eine Ecke
    /// der Diagonalen, der zweite die andere. Danach fragt der Speicherdialog
    /// nach dem Ziel. Das läuft neben dem Action-Register, weil es keine
    /// Modelländerung ist, sondern eine Frage des Exports an die Oberfläche.
    pub fn start_png_bereich(&mut self) {
        // Ein laufendes Werkzeug würde die Klicks für sich beanspruchen.
        self.tool = None;
        self.ghost = None;
        // Das Fenster gibt die Fläche für die Diagonale frei; kommt der zweite
        // Klick, ist es wieder da (dort steht dann der Bereich).
        self.ui.png_export_offen = false;
        self.ui.png_bereich = None;
        self.ui.png_wahl = Some(PngWahl { erster: None });
        self.mitteilung(jjcad_core::i18n::tr("gui.canvas.png_bereich_diagonale"));
    }

    /// T0235: Fenster „PNG exportieren" öffnen — der **eine** Weg, ein Bild
    /// zu exportieren. Bereich, Auflösung und Hintergrund stehen dort.
    pub fn png_export_oeffnen(&mut self) {
        self.ui.png_wahl = None;
        self.ui.png_export_offen = true;
    }

    /// T0235: der Speicherort ist gewählt — schreiben mit den Einstellungen
    /// aus dem Fenster. Der Bereich bleibt für den nächsten Export stehen.
    pub fn export_png_to(&mut self, path: PathBuf) {
        let sheet = match &self.tab().tab {
            TabKind::Sheet(id) => self.doc().sheets.iter().position(|s| s.id == *id),
            TabKind::Model | TabKind::Block(_) => None,
        };
        let e = self.ui.png_einstellungen;
        match raster::render_png_auswahl(
            self.doc(),
            self.ui.png_bereich,
            sheet,
            e.breite_px,
            e.papier,
            &path,
        ) {
            Ok(_) => self.toast(jjcad_core::i18n::t_args_owned(
                "gui.app.png_exportiert_x",
                &[("wert1", format!("{}", path.display()))],
            )),
            Err(e) => self.toast(e),
        }
    }

    /// PDF ausgewählt: Seitenzahl lesen und die Seitenwahl öffnen.
    pub fn pdf_import_oeffnen(&mut self, path: PathBuf) {
        match jjcad_core::pdf_read::read_pdf(&path) {
            Ok(pdf) => {
                self.ui.pdf_import = Some(PdfImportWahl {
                    pfad: path,
                    seiten: pdf.pages.len(),
                    seite: 1,
                    als_block: true,
                });
            }
            Err(e) => self.toast(jjcad_core::i18n::t_args_owned("gui.app.pdf_x", &[("e", format!("{}", e))])),
        }
    }

    /// Bilddatei einlesen und das Werkzeug `draw.image` starten.
    ///
    /// Die Datei wird **jetzt** geprüft und eingebettet — ein unlesbares Bild
    /// fällt beim Auswählen auf und nicht erst als leeres Rechteck. Das
    /// Werkzeug fragt danach die beiden gegenüberliegenden Ecken ab.
    pub fn bild_importieren(&mut self, pfad: std::path::PathBuf) {
        match jjcad_core::bild::lade_datei(&pfad) {
            Ok((name, data)) => {
                self.prefs.last_dir = pfad
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let _ = self.prefs.save();
                self.options.bild = Some(BildQuelle {
                    name: name.clone(),
                    data,
                });
                self.start_tool("draw.image");
                self.toast(jjcad_core::i18n::t_args_owned(
                    "gui.app.bild_x_ecken_klicken",
                    &[("name", name)],
                ));
            }
            Err(e) => self.toast(e),
        }
    }

    /// Gewählte Seite einfügen und das Ergebnis melden.
    pub fn pdf_import_einfuegen(&mut self, wahl: &PdfImportWahl) {
        let res = self.session_mut().execute(
            "file.import.pdf",
            &serde_json::json!({
                "path": wahl.pfad.to_string_lossy(),
                "page": wahl.seite,
                "as_block": wahl.als_block,
            }),
        );
        match res {
            Ok(v) => {
                self.mark_changed();
                // Eingefügtes Blatt zeigen: das Papier liegt in
                // Blattkoordinaten (0…420 mm) und wäre sonst winzig.
                if let Some(id) = v["created"].as_array().and_then(|a| a.first()).and_then(|x| x.as_u64()) {
                    self.session_mut().select(vec![id]);
                }
                self.fit_view();
                let objekte = v["entities"].as_u64().unwrap_or(0);
                let mut text = jjcad_core::i18n::t_args_owned("gui.app.pdf_x_objekte_uebernommen_x_seite_x", &[("wert1", format!("{}", objekte)), ("wert2", format!("{}", wahl
                        .pfad
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default())), ("wert3", format!("{}", wahl.seite))]);
                let warn: Vec<String> = v["warnings"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|w| {
                                Some(format!(
                                    "{} {}",
                                    w["art"].as_str()?,
                                    w["anzahl"].as_u64()?
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if !warn.is_empty() {
                    text.push_str(jjcad_core::i18n::tr("gui.app.nicht_uebernommen"));
                    text.push_str(&warn.join(", "));
                }
                self.toast(text);
            }
            Err(e) => self.toast(jjcad_core::i18n::t_args_owned("gui.app.pdf_x", &[("e", format!("{}", e))])),
        }
    }

    pub fn import_dxf_from(&mut self, path: PathBuf) {
        match dxf::import_dxf(&path) {
            Ok(res) => {
                let n = res.entities.len();
                let warn = res.warnings.len();
                let merged = dxf::import::merge_into(self.doc_mut(), res);
                self.mark_changed();
                self.fit_view();
                let mut msg = jjcad_core::i18n::t_args_owned("gui.app.dxf_x_objekte_importiert", &[("n", format!("{}", n))]);
                if warn > 0 {
                    msg.push_str(&jjcad_core::i18n::t_args_owned("gui.app.x_hinweise", &[("warn", format!("{}", warn))]));
                }
                if let Some(w) = merged.warnings.first() {
                    msg.push_str(&format!(" – {w}"));
                }
                self.toast(msg);
            }
            Err(e) => self.toast(e),
        }
    }

    // ---- Tastatur ----

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Welche physische Taste welches Zeichen erzeugt, lernt das Programm
        // beim Tippen (T0121) — der Dauerlauf des Schwenks liest `keys_down`
        // und braucht die Taste, nicht das Zeichen. Vor jeder Auswertung.
        self.lerne_zeichen_tasten(ctx);
        let typing = ctx.wants_keyboard_input();
        let chat_offen = self.chat.offen;
        // F11 schaltet das Vollbild — unabhängig von Tippeingabe und Werkzeug,
        // damit man auch aus einem laufenden Werkzeug heraus wieder herausk kommt.
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.toggle_fullscreen(ctx);
            return;
        }
        // Umschalt+F3 schaltet die Leistungsanzeige (T0068) — wie das Vollbild
        // unabhängig von Werkzeug und Tippeingabe: man braucht sie auch, wenn
        // ein Werkzeug läuft und gerade etwas schlecht reagiert.
        if ctx.input(|i| i.key_pressed(egui::Key::F3) && i.modifiers.shift) {
            self.perf.an = !self.perf.an;
            self.toast(if self.perf.an { jjcad_core::i18n::tr("gui.app.leistung_an") } else { jjcad_core::i18n::tr("gui.app.leistung_aus") });
            return;
        }
        // Text wird direkt in der Zeichnung geschrieben: solange geschrieben
        // wird, gehört die Tastatur dem Text — keine Kürzel, kein Esc-Abbruch.
        // Esc übernimmt und der Rechtsklick bricht ab; beides wertet
        // `textedit::show` aus.
        if self.ui.texteingabe.is_some() {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.chat.offen {
                // ESC schließt zuerst das Protokoll, nicht das Werkzeug
                self.chat.schliessen();
                ctx.memory_mut(|m| m.stop_text_input());
                return;
            }
            if self.ui.order_pick.is_some() {
                self.ui.order_pick = None;
                self.toast(jjcad_core::i18n::tr("gui.app.reihenfolge_kein_objekt_gewaehlt"));
            } else if self.ui.bind_key.is_some() {
                self.ui.bind_key = None;
                self.toast(jjcad_core::i18n::tr("gui.app.tastatur_belegen_abgebrochen"));
            } else if self.ui.bind_slot.is_some() {
                self.ui.bind_slot = None;
                self.toast(jjcad_core::i18n::tr("gui.app.hotbar_verknuepfen_abgebrochen"));
            } else if self.select_zwei.take().is_some() {
                // T0145: nur das offene Auswahlfenster schließen — die
                // bestehende Auswahl bleibt stehen.
                self.toast(jjcad_core::i18n::tr("gui.app.auswahl_abgebrochen"));
            } else {
                self.escape_pressed();
            }
        }
        // ^ (Taste unter ESC) öffnet bzw. schließt das Chat-Protokoll unten
        // links. Schließen gilt auch bei fokussierter Eingabezeile, sonst käme
        // man mit der Tastatur nicht mehr aus dem Feld heraus.
        if ctx.input(|i| i.key_pressed(crate::chat::TASTE_OEFFNEN))
            && (self.chat.offen || !typing)
        {
            if self.chat.offen {
                self.chat.schliessen();
                ctx.memory_mut(|m| m.stop_text_input());
            } else {
                self.chat.umschalten();
            }
            return;
        }
        // Solange das Protokoll offen ist, gehört die Tastatur dem Chat:
        // keine Werkzeugkürzel, kein Zeichnen im Hintergrund.
        if chat_offen {
            return;
        }
        let (ctrl, shift, alt) =
            ctx.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
        // T0215: Die gehaltenen Steuerungstasten wählen die **Ebene** der
        // Belegung. Jede Kombination aus Umschalt, Strg und Alt trägt ihre
        // eigene Taste je Funktion und ihre eigene Hotbar.
        let lage = jjcad_core::persist::Kombination::aus_modifikatoren(shift, ctrl, alt);
        // Strg+C/X/V kommen von egui als Ereignisse, nicht als Tastendruck
        let (ev_copy, ev_cut, ev_paste) = ctx.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            )
        });
        if !typing {
            // Die Zwischenablage kommt bei egui als **Ereignis**, nicht als
            // Tastendruck — sie folgt trotzdem der Belegung ihrer Ebene (T0215):
            // steht dort nichts, gilt das bisherige Verhalten.
            if ev_copy {
                // Strg+Umschalt+C kommt bei egui ebenfalls als Kopier-Ereignis
                // an (egui-winit prüft nur Strg).
                match self.prefs.action_for_kombination(lage, "c") {
                    Some(a) => self.run_action(ctx, &a),
                    None if shift => self.start_tool("modify.copy"),
                    None => self.run_action(ctx, "edit.copy"),
                }
            }
            if ev_cut {
                match self.prefs.action_for_kombination(lage, "x") {
                    Some(a) => self.run_action(ctx, &a),
                    None => self.copy_selection(true),
                }
            }
            if ev_paste {
                // T0142: Strg+Umschalt+V fügt die Zwischenablage als **Block**
                // ein — der Block entsteht dabei ohne Namensdialog.
                match self.prefs.action_for_kombination(lage, "v") {
                    Some(a) => self.run_action(ctx, &a),
                    None if shift => self.einfuegen_als_block(),
                    None => self.einfuegen(),
                }
            } else if strg_v_losgelassen(ctx) && !self.ui.bildablage.hat_text() {
                // egui-winit verschluckt Strg+V, wenn die Zwischenablage
                // **nur** ein Bild enthält (es fragt ausschließlich Text ab) —
                // dann kommt kein `Event::Paste` an. Das Loslassen der Taste
                // kommt durch und dient als Auslöser. Liegt zusätzlich Text
                // vor, hat `Event::Paste` schon eingefügt.
                self.einfuegen();
            }
        }
        let mut run: Option<String> = None;

        // Vorschau: drehen, spiegeln, setzen. Die beiden Griffe hängen an den
        // **Funktionen** „Drehen" (`modify.rotate`) und „Spiegeln"
        // (`modify.mirror`) — nicht an festen Tasten (T0056). Wer die Funktion
        // umbelegt, dreht die Vorschau mit derselben Taste wie die Auswahl.
        if self.ghost.is_some() && !typing {
            if self.taste_der_funktion(ctx, "modify.rotate") {
                // Drehen im Uhrzeigersinn (negativer Winkel), Umschalt gegen den
                // Uhrzeigersinn — im Modell zeigt +y nach oben.
                self.rotate_ghost(App::ghost_drehgrad(shift));
            }
            if self.taste_der_funktion(ctx, "modify.mirror") {
                self.flip_ghost();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.commit_ghost();
                return;
            }
            // T0094: Jede **andere** Funktion bricht das Einfügen ab. Der Druck
            // läuft unten weiter und startet sie über `start_tool`; die Vorschau
            // verschwindet dabei, statt neben dem Werkzeug stehen zu bleiben.
            // Früher kehrte die Auswertung hier immer zurück — eine Funktionstaste
            // wirkte während des Einfügens gar nicht, und ein Klick auf Ribbon
            // oder Hotbar ließ beides gleichzeitig laufen.
        }

        // Wand platzieren: Tab wechselt die Ausrichtung
        {
            if let Some(t) = self.tool.clone() {
                if t.action == "draw.wall"
                    && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
                {
                    self.options.wall_align = match self.options.wall_align {
                        jjcad_core::style::WallAlign::Center => jjcad_core::style::WallAlign::Left,
                        jjcad_core::style::WallAlign::Left => jjcad_core::style::WallAlign::Right,
                        jjcad_core::style::WallAlign::Right => jjcad_core::style::WallAlign::Center,
                    };
                    self.toast(jjcad_core::i18n::t_args_owned("gui.app.wandausrichtung_x", &[("wert1", format!("{}", self.options.wall_align.text()))]));
                    ctx.memory_mut(|m| m.stop_text_input());
                    return;
                }
            }
        }

        // Tür platzieren: R = Öffnungsrichtung, F = Anschlag spiegeln
        if !typing {
            if let Some(t) = self.tool.clone() {
                if t.action == "draw.door" {
                    if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                        self.options.swing_in = !self.options.swing_in;
                        self.toast(if self.options.swing_in {
                            jjcad_core::i18n::tr("gui.app.oeffnet_zur_wandinnenseite")
                        } else {
                            jjcad_core::i18n::tr("gui.app.oeffnet_zur_wandaussenseite")
                        });
                        return;
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                        self.options.hinge = -self.options.hinge;
                        self.toast(if self.options.hinge >= 0.0 {
                            jjcad_core::i18n::tr("gui.app.anschlag_rechts")
                        } else {
                            jjcad_core::i18n::tr("gui.app.anschlag_links")
                        });
                        return;
                    }
                }
            }
        }

        // T0215: Jede Kombination aus Umschalt, Strg und Alt ist eine eigene
        // **Ebene** der Tastaturbelegung — dieselbe Taste trägt je Ebene eine
        // Funktion (`c` = Stutzen, `ctrl+c` = Kopieren). Ausgewertet wird
        // ausschließlich über `prefs.action_for_kombination`: was dort nicht
        // steht, hat keine Wirkung.
        if !typing {
            // Zeichen ohne eigene Taste (Umlaute, ß, …): sie kommen nur als
            // Text an (T0121) — mit dem Vorsatz der Ebene.
            let sonderzeichen = getippte_zeichen(ctx)
                .into_iter()
                .find_map(|z| self.prefs.action_for_kombination(lage, &z));
            if let Some(action) = sonderzeichen.clone() {
                run = Some(action);
            }
            // Einzeltasten über die Belegung ihrer Ebene: der Vorsatz macht aus
            // „c" ein „ctrl+c" bzw. „shift+c" (T0056/T0215).
            for (key, label) in EINZELTASTEN {
                if sonderzeichen.is_none() && ctx.input(|i| i.key_pressed(*key)) {
                    // Auch der Ansichtsschwenk ist eine gewöhnliche Funktion
                    // (T0089): der Druck löst sie einmal aus, gehalten
                    // wiederholt das Betriebssystem den Druck wie bei jeder
                    // anderen Taste.
                    if let Some(action) = self.prefs.action_for_kombination(lage, label) {
                        run = Some(action);
                    }
                }
            }
            // Ziffer → Slot **der gehaltenen Ebene** (T0215): mit Strg liegt
            // eine eigene Hotbar auf den Ziffern. `hotbar::taste_slot` ist die
            // eine Zuordnung („0" ist der letzte Slot).
            // Eine getippte Länge gehört dem Werkzeug **oder** der
            // Hilfslinien-Eingabe (T0230): dort dürfen die Ziffern keine
            // Funktion aus der Hotbar starten.
            let tool_waiting = self.tool.as_ref().map(|t| !t.pts.is_empty()).unwrap_or(false)
                || self.hilfslinie_modus;
            let slots = self.prefs.slots_fuer(lage);
            for (key, label) in [
                (egui::Key::Num1, "1"),
                (egui::Key::Num2, "2"),
                (egui::Key::Num3, "3"),
                (egui::Key::Num4, "4"),
                (egui::Key::Num5, "5"),
                (egui::Key::Num6, "6"),
                (egui::Key::Num7, "7"),
                (egui::Key::Num8, "8"),
                (egui::Key::Num9, "9"),
                (egui::Key::Num0, "0"),
            ] {
                if !tool_waiting && ctx.input(|i| i.key_pressed(key)) {
                    if let Some(slot) = crate::hotbar::taste_slot(label) {
                        if let Some(Some(action)) = slots.get(slot) {
                            run = Some(action.clone());
                        }
                    }
                }
            }
            // Die festen Tasten (`Esc`, `Enter`, `Entf`, `F1`, `F5` …) gelten
            // ohne Steuerungstaste — mit Strg gehört die Taste der Belegung.
            if lage.ist_leer() {
                if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                    if self.tool.is_none() {
                        run = Some("edit.repeat".into());
                    }
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Enter))
                    && !tool_waiting
                    && !self.hilfslinie_modus
                {
                    self.tool_enter();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                    // Füllfläche/Schraffur im Modus „Punkte setzen": Entf und
                    // Backspace brechen die Eingabe ab — ESC bestätigt dort.
                    if self.ui.typed.is_empty() && self.fill_points_running() {
                        self.cancel_tool();
                        self.toast(jjcad_core::i18n::tr("gui.app.fuellflaeche_verworfen"));
                    } else {
                        let ids = self.selection().to_vec();
                        if !ids.is_empty() {
                            let _ = self.session_mut().execute("modify.erase", &serde_json::json!({"ids": ids}));
                            self.session_mut().clear_selection();
                            self.mark_changed();
                        }
                    }
                }
                if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
                    self.ui.help_open = true;
                }
                // T0159: F5 zeigt die ganze Zeichnung — dieselbe Funktion wie der
                // Knopf „Alles zeigen“ (`view.zoom_extents` → `App::fit_view`).
                if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
                    run = Some("view.zoom_extents".into());
                }
            }
        }
        if let Some(id) = run {
            self.run_action(ctx, &id);
        }
    }

    /// Ist eine Taste gedrückt, die in der Belegung auf `action` steht? Damit
    /// hängen Sonderfunktionen an einer **Funktion** statt an einer festen
    /// Taste (T0056): drehen/spiegeln der Einfüge-Vorschau folgt „Drehen" und
    /// „Spiegeln" aus der Tastaturbelegung.
    ///
    /// Der Druck wird dabei **verbraucht** (T0094): sonst läse ihn die
    /// Auswertung weiter unten noch einmal und startete dieselbe Funktion als
    /// Werkzeug — das Einfügen bräche ab, obwohl der Nutzer nur drehen wollte.
    fn taste_der_funktion(&self, ctx: &egui::Context, action: &str) -> bool {
        let lage = ctx.input(|i| i.modifiers);
        let mut getroffen = false;
        for (key, label) in EINZELTASTEN {
            if self.prefs.action_for_key(label).as_deref() != Some(action) {
                continue;
            }
            if !ctx.input(|i| i.key_pressed(*key)) {
                continue;
            }
            ctx.input_mut(|i| {
                i.consume_key(lage, *key);
            });
            getroffen = true;
        }
        // Zeichen ohne eigene Taste (Umlaute, ß, …) — sie kommen nur als Text
        // an (T0121). Der Druck wird ebenfalls verbraucht, sonst startete die
        // Auswertung unten dieselbe Funktion und bräche das Einfügen ab.
        let treffer: Vec<String> = getippte_zeichen(ctx)
            .into_iter()
            .filter(|z| {
                self.prefs
                    .action_for_key(&zeichen_label(z, lage.shift))
                    .as_deref()
                    == Some(action)
            })
            .collect();
        if !treffer.is_empty() {
            ctx.input_mut(|i| {
                i.events.retain(|e| match e {
                    egui::Event::Text(t) => {
                        let klein = t.to_lowercase();
                        !treffer.iter().any(|z| klein.contains(z.as_str()))
                    }
                    _ => true,
                });
            });
            getroffen = true;
        }
        getroffen
    }

    /// Lernt, welche physische Taste welches Zeichen erzeugt (T0121).
    ///
    /// `egui::Key` kennt keine Umlaute: die Taste `ä` kommt als `Key::Quote`
    /// (US-Lage) an, das Zeichen nur als `Event::Text`. Beides steht im selben
    /// Bild — daraus entsteht die Zuordnung, die den Dauerlauf des Schwenks
    /// trägt; der liest `keys_down` und braucht die Taste, nicht das Zeichen.
    fn lerne_zeichen_tasten(&mut self, ctx: &egui::Context) {
        let (gedrueckt, texte) = ctx.input(|i| {
            let gedrueckt: Vec<egui::Key> = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => Some(*key),
                    _ => None,
                })
                .collect();
            let texte: Vec<String> = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .collect();
            (gedrueckt, texte)
        });
        // Nur eine eindeutige Paarung lernen: genau eine gedrückte Taste und
        // Zeichen, die keine eigene Taste haben.
        if gedrueckt.len() != 1 {
            return;
        }
        for text in texte {
            for c in text.chars() {
                if ist_textzeichen(&c.to_string()) {
                    let z = c.to_lowercase().next().unwrap_or(c);
                    self.ui.zeichen_taste.insert(z, gedrueckt[0]);
                }
            }
        }
    }

    /// Ist die Taste, die `zeichen` erzeugt, gerade gedrückt? Für Zeichen mit
    /// eigener `egui::Key`-Taste direkt, für Umlaute und andere Sonderzeichen
    /// über die beim Tippen gelernte Zuordnung (T0121).
    fn taste_gedrueckt(&self, zeichen: &str, keys: &std::collections::HashSet<egui::Key>) -> bool {
        if let Some(key) = taste_zu_label(zeichen) {
            return keys.contains(&key);
        }
        let mut chars = zeichen.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => self
                .ui
                .zeichen_taste
                .get(&c.to_lowercase().next().unwrap_or(c))
                .map_or(false, |k| keys.contains(k)),
            _ => false,
        }
    }

    /// Schiebt die Ansicht um einen Schritt (`SCHWENK_SCHRITT_PX` Bildpunkte) in
    /// Bildschirmrichtung — die Umrechnung läuft über die gedrehte Ansicht,
    /// damit der Schub auch bei gedrehter Zeichnung der Taste folgt.
    /// **Der einzige** Weg des sprunghaften Schwenks (T0101); Taste, Hotbar und
    /// Ribbon rufen ihn über die Funktion `view.pan_*` auf (T0089).
    pub fn schwenk_schritt(&mut self, dx: f32, dy: f32) {
        let mut v = self.current_view();
        // T0067: Die Einstellung „Geschwindigkeit der Ansicht“ skaliert den Schritt.
        let px = SCHWENK_SCHRITT_PX * self.prefs.pan_speed;
        let w = v.screen_delta_to_world(Vec2::new(dx * px, dy * px));
        // T0158: Bei „Ansichtsbereich anpassen“ schiebt derselbe Schritt den
        // Ausschnitt im Ansichtsfenster statt das Blatt.
        if let Some((_, vp)) = self.vp_edit_fenster() {
            let center = crate::canvas::viewport_schwenken(&vp, w);
            self.vp_setzen(vp.id, None, Some(center));
            return;
        }
        v.center += w;
        self.set_view(v);
    }

    /// Schiebt die Ansicht um `dt` Sekunden Fahrt in Bildschirmrichtung
    /// (T0101) — dieselbe Umrechnung wie [`App::schwenk_schritt`], nur mit
    /// einem Weg aus dem Tempo statt aus dem festen Schritt.
    pub fn schwenk_strecke(&mut self, dt: f32, dx: f32, dy: f32) {
        let mut v = self.current_view();
        let px = SCHWENK_TEMPO_PX_PRO_S * self.prefs.pan_speed * dt;
        let w = v.screen_delta_to_world(Vec2::new(dx * px, dy * px));
        // T0158: derselbe Weg wie beim Schritt — im Ansichtsbereich wandert der
        // Ausschnitt, nicht das Blatt.
        if let Some((_, vp)) = self.vp_edit_fenster() {
            let center = crate::canvas::viewport_schwenken(&vp, w);
            self.vp_setzen(vp.id, None, Some(center));
            return;
        }
        v.center += w;
        self.set_view(v);
    }

    /// Ein Aufruf der Schwenkfunktion aus Taste, Hotbar, Ribbon oder Menü
    /// (T0101). Sprunghaft: ein Schritt. Kontinuierlich: **kein** Schritt —
    /// den Weg rechnet allein [`App::schwenk_fortschritt`] aus der gedrückten
    /// Funktion und der Bilddauer. Ein Schritt daneben zählte doppelt und hing
    /// an der Tastenwiederholung des Betriebssystems: die Ansicht lief dann
    /// schneller als das eingestellte Tempo.
    pub fn schwenk_funktion(&mut self, dx: f32, dy: f32) {
        if !self.prefs.pan_mode.kontinuierlich() {
            self.schwenk_schritt(dx, dy);
        }
    }

    /// Kontinuierliches Verschieben (T0101): einmal je Bild aufgerufen, nachdem
    /// die Leisten gezeichnet sind (die Hotbar meldet dort einen gehaltenen
    /// Druck) und bevor die Zeichenfläche zeichnet.
    ///
    /// Bewegt wird nur, wenn die Einstellung „Ansicht verschieben“ auf
    /// „Kontinuierlich“ steht und eine Schwenkfunktion gehalten wird — Taste,
    /// Hotbar-Ziffer oder gedrückter Slot. Das Tempo ist
    /// [`SCHWENK_TEMPO_PX_PRO_S`] × `pan_speed`, die Strecke hängt an der
    /// echten Bilddauer (gedeckelt, damit ein Stocken nicht springt).
    pub fn schwenk_fortschritt(&mut self, ctx: &egui::Context) {
        let (dx, dy) = self.schwenk_gehalten(ctx);
        // Der gemerkte Zeigerdruck gilt nur für **dieses** Bild; die Hotbar
        // setzt ihn beim Zeichnen neu.
        self.ui.schwenk_zeiger = None;
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, SCHWENK_DT_MAX_S);
        if dt > 0.0 {
            self.schwenk_strecke(dt, dx, dy);
        }
        // Ohne Zeigerbewegung käme kein neues Bild — der Dauerlauf muss sich
        // selbst anfordern, sonst steht die Ansicht nach einem Bild.
        ctx.request_repaint();
    }

    /// Richtung der gerade gehaltenen Schwenkfunktionen in Bildschirmkoordinaten
    /// (T0101). Gegenläufige Funktionen heben sich auf, je Achse höchstens 1.
    ///
    /// Gelesen werden alle drei Wege, über die eine Schwenkfunktion erreichbar
    /// ist: die Tasten der Tastaturbelegung, die Ziffern der Hotbar-Slots und
    /// ein gedrückt gehaltener Slot (`UiState::schwenk_zeiger`). Bei offenem
    /// Protokoll, laufender Texteingabe, Systemdialog oder ohne Fensterfokus
    /// schwenkt nichts — dort gehört die Tastatur dem Text bzw. einem anderen
    /// Fenster.
    fn schwenk_gehalten(&self, ctx: &egui::Context) -> (f32, f32) {
        if !self.prefs.pan_mode.kontinuierlich()
            || self.chat.offen
            || self.ui.systemdialog.is_some()
            || ctx.wants_keyboard_input()
            || !ctx.input(|i| i.focused)
        {
            return (0.0, 0.0);
        }
        let (shift, strg, keys) = ctx.input(|i| {
            (
                i.modifiers.shift,
                i.modifiers.command,
                i.keys_down.clone(),
            )
        });
        // Mit Strg (Befehlstaste) sind die Kombinationen gemeint, nicht der
        // Schwenk: `Strg+S` speichert und darf die Ansicht nicht schieben
        // (T0089 — dort scheiterte schon ein Dauerlauf an derselben Stelle).
        if strg {
            return (0.0, 0.0);
        }
        let mut dx = 0.0f32;
        let mut dy = 0.0f32;
        // Tasten der Tastaturbelegung (Buchstaben und Symbole, `shift+…`).
        for aktion in [
            "view.pan_left",
            "view.pan_right",
            "view.pan_up",
            "view.pan_down",
        ] {
            let gedrueckt = self.prefs.keys_for(aktion).iter().any(|taste| {
                let (umschalt, zeichen) = match taste.strip_prefix("shift+") {
                    Some(rest) => (true, rest),
                    None => (false, taste.as_str()),
                };
                umschalt == shift && self.taste_gedrueckt(zeichen, &keys)
            });
            if gedrueckt {
                if let Some((rx, ry)) = schwenk_richtung(aktion) {
                    dx += rx;
                    dy += ry;
                }
            }
        }
        // Hotbar: gehaltene Ziffer und gedrückt gehaltener Slot.
        let slots = self.prefs.slots();
        for (slot, belegt) in slots.iter().enumerate() {
            let label = crate::hotbar::slot_taste(slot);
            if !taste_zu_label(&label).map_or(false, |key| keys.contains(&key)) {
                continue;
            }
            if let Some(aktion) = belegt {
                if let Some((rx, ry)) = schwenk_richtung(aktion) {
                    dx += rx;
                    dy += ry;
                }
            }
        }
        if let Some((rx, ry)) = self.ui.schwenk_zeiger {
            dx += rx;
            dy += ry;
        }
        (dx.clamp(-1.0, 1.0), dy.clamp(-1.0, 1.0))
    }

    fn tool_enter(&mut self) {
        if let Some(tool) = self.tool.clone() {
            match tool.action.as_str() {
                "draw.polyline" | "draw.spline" | "draw.path" | "dim.chain" => {
                    if tool.pts.len() >= 2 {
                        canvas::finish_tool(self, true);
                    }
                }
                _ => {
                    if tool.selecting && !self.selection().is_empty() {
                        // Schritt nach der Auswahl aus dem Werkzeug selbst — nicht
                        // fest „Verschieben" (gilt auch für Kopieren, Drehen …).
                        let mut folge = tool.clone();
                        folge.selecting = false;
                        if let Some(t) = &mut self.tool {
                            t.selecting = false;
                        }
                        let hinweis = commands_hint(&folge, &self.options);
                        self.mitteilung(hinweis);
                    }
                }
            }
        }
    }

    /// Legt Objekte samt Basispunkt in die Zwischenablage.
    ///
    /// Ohne Basispunkt (`None`) gilt die untere linke Ecke der Auswahl — das ist
    /// der automatische Basispunkt der normalen Kopierfunktion. „Kopieren mit
    /// Basispunkt" übergibt den geklickten Punkt.
    pub fn merke_zwischenablage(&mut self, ids: &[Id], base: Option<V2>) {
        // Türen und Fenster gehören zur Wand: wird eine Wand kopiert, wandern
        // sie mit in die Zwischenablage (T0207). Sonst klebte die Kopie ohne
        // Öffnungen in der Zeichnung, die alten blieben am Original.
        let mut ids: Vec<Id> = ids.to_vec();
        {
            let doc = self.doc();
            for extra in jjcad_core::actions::carried_openings(doc, &ids) {
                if !ids.contains(&extra) {
                    ids.push(extra);
                }
            }
        }
        let ents: Vec<Entity> = {
            let doc = self.doc();
            doc.ents()
                .iter()
                .filter(|e| ids.contains(&e.id))
                .cloned()
                .collect()
        };
        if ents.is_empty() {
            return;
        }
        let base = base.or_else(|| {
            // Ein **einzelnes** Textfeld führt seinen Einfügepunkt als
            // Basispunkt mit: ein „Zentriert/Mitte" gesetzter Text landet beim
            // Einfügen damit genau mittig auf dem Zielpunkt, nicht mit seiner
            // linken unteren Rahmenecke.
            if let [e] = ents.as_slice() {
                if let EntityKind::Text { p, .. } = &e.kind {
                    return Some(*p);
                }
            }
            let doc = self.doc();
            let mut bb = BBox::EMPTY;
            for e in &ents {
                bb.union(jjcad_core::render::entity_bbox(doc, e));
            }
            if bb.is_valid() {
                Some(bb.min)
            } else {
                None
            }
        });
        self.ui.clipboard = crate::ghost::Clipboard { ents, base };
    }

    pub fn copy_selection(&mut self, cut: bool) {
        let ids = self.selection().to_vec();
        self.merke_zwischenablage(&ids, None);
        if self.ui.clipboard.is_empty() {
            return;
        }
        // T0112: genau **ein** Bild geht als reine Bilddatei in die
        // System-Zwischenablage, damit andere Programme es einfügen können.
        // Dann keine Textmeldung ablegen — sie würde das Bild überschreiben.
        let als_bild = match self.bild_der_auswahl() {
            Some(bild) => self.ui.bildablage.lege(&bild).is_ok(),
            None => false,
        };
        if !als_bild {
            let msg = jjcad_core::i18n::t_args_owned("gui.app.joja_cad_zwischenablage_x_objekte", &[("wert1", format!("{}", self.ui.clipboard.len()))]);
            self.ctx.copy_text(msg);
        }
        if cut {
            let _ = self.session_mut().execute("modify.erase", &serde_json::json!({"ids": ids}));
            self.session_mut().clear_selection();
            self.mark_changed();
        }
        self.toast(if cut { jjcad_core::i18n::tr("gui.app.auswahl_ausgeschnitten") } else { jjcad_core::i18n::tr("gui.app.auswahl_kopiert") });
    }

    /// Das Bild der Auswahl, wenn genau **ein** Objekt ausgewählt ist und
    /// dieses ein Bild ist (T0112).
    ///
    /// Nur dann geht die Auswahl als reine Bilddatei in die
    /// System-Zwischenablage — bei mehreren Objekten oder einer anderen
    /// Objektart bleibt es bei der eigenen Zwischenablage.
    pub fn bild_der_auswahl(&self) -> Option<jjcad_core::bild::Bild> {
        let ids = self.selection();
        if ids.len() != 1 {
            return None;
        }
        let doc = self.doc();
        let e = doc.ents().iter().find(|e| e.id == ids[0])?;
        let EntityKind::Image { data, .. } = &e.kind else {
            return None;
        };
        jjcad_core::bild::dekodiere(data).map(|b| (*b).clone())
    }

    /// Fügt ein, was in der Zwischenablage liegt (T0112): ein **Bild** hat
    /// Vorrang vor der eigenen Objekt-Zwischenablage.
    pub fn einfuegen(&mut self) {
        if self.bild_einfuegen() {
            return;
        }
        self.start_ghost_paste();
    }

    /// Übernimmt ein Bild aus der System-Zwischenablage und startet
    /// `draw.image`. `false`, wenn dort keines liegt — oder wenn es das Bild
    /// ist, das wir selbst gerade kopiert haben (dann gilt die eigene
    /// Zwischenablage mit Lage und Größe).
    pub fn bild_einfuegen(&mut self) -> bool {
        let Some(bild) = self.ui.bildablage.hole() else {
            return false;
        };
        if bild.eigen && !self.ui.clipboard.is_empty() {
            return false;
        }
        self.bild_aus_pixeln(bild.breite, bild.hoehe, &bild.rgba);
        true
    }

    /// Baut aus rohen Pixeln ein einbettbares Bild und startet das
    /// Bildwerkzeug — derselbe Weg wie nach einem Datei-Import.
    pub fn bild_aus_pixeln(&mut self, breite: u32, hoehe: u32, rgba: &[u8]) {
        match jjcad_core::bild::aus_rgba(breite, hoehe, rgba) {
            Ok((name, data)) => {
                self.options.bild = Some(BildQuelle { name: name.clone(), data });
                self.start_tool("draw.image");
                self.toast(jjcad_core::i18n::t_args_owned(
                    "gui.app.bild_x_ecken_klicken",
                    &[("name", name)],
                ));
            }
            Err(e) => self.toast(e),
        }
    }

    pub fn paste_clipboard(&mut self) {
        // T0112: liegt ein Bild in der System-Zwischenablage, wird es
        // eingefügt — auch über den Knopf „Einfügen“.
        if self.bild_einfuegen() {
            return;
        }
        if self.ui.clipboard.is_empty() {
            return;
        }
        let d = V2::new(self.ui.clipboard_offset, -self.ui.clipboard_offset);
        let mut new_ids = Vec::new();
        let clip = self.ui.clipboard.ents.clone();
        for e in clip {
            let mut ne = jjcad_core::transform::transform_entity(&e, &jjcad_core::geom::Affine::translate(d));
            ne.id = self.doc_mut().new_id();
            new_ids.push(ne.id);
            self.doc_mut().ents_mut().push(ne);
        }
        self.mark_changed();
        self.session_mut().select(new_ids);
        self.ui.clipboard_offset += 20.0;
        self.toast(jjcad_core::i18n::tr("gui.app.auswahl_eingefuegt"));
    }

    // ---- Datei- und Blattreiter ----

    /// Reiterstreifen in **zwei Zonen** (T0073):
    ///
    /// Links die offenen **Dateien** — je Zeichnung ein Reiter mit Dateisymbol auf
    /// eigenem, dunklerem Grund; der aktive steht als Karte mit Akzentlinie darin.
    /// Rechts, in einer abgesetzten Fläche, die **Blätter** der aktiven Datei als
    /// kleinere Chips samt `+` für ein neues Blatt. Die beiden Zonen unterscheiden
    /// sich in Grund, Größe und Form; Beschriftungen („Dateien“/„Blätter“) sind
    /// bewusst nicht da — sie erklärten, was die Formen schon sagen.
    ///
    /// „Neu“ hängt einen Datei-Reiter an, statt die aktuelle Zeichnung zu
    /// ersetzen — der bisherige Stand bleibt im eigenen Reiter stehen.
    fn doc_tabs(&mut self, ui: &mut egui::Ui) {
        let mut wechsle_datei: Option<usize> = None;
        let mut schliesse_datei: Option<usize> = None;
        let mut wechsle_reiter: Option<TabKind> = None;
        let mut schliesse_blatt: Option<Id> = None;
        let mut schliesse_block: Option<String> = None;
        let mut neues_blatt = false;
        let dateien: Vec<(usize, String, bool)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.session.doc.name.clone(), t.session.doc.modified))
            .collect();
        let aktiv = self.active;
        let mehrere_dateien = dateien.len() > 1;
        let reiter: Vec<(TabKind, String, bool)> = {
            let t = self.tab();
            let mut v = vec![(TabKind::Model, jjcad_core::i18n::tr("gui.app.modell").to_string(), false)];
            // T0218: Ein geschlossener Blattreiter bleibt geschlossen, das Blatt
            // steht weiter in der Zeichnung und im Blattmanager.
            for s in t
                .session
                .doc
                .sheets
                .iter()
                .filter(|s| !t.geschlossene_blaetter.contains(&s.id))
            {
                v.push((TabKind::Sheet(s.id), s.name.clone(), true));
            }
            v
        };
        // Offene Blockeditoren, nach Herkunft getrennt (T0137): Blöcke der
        // Datei stehen in der Blattzone, Benutzerblöcke in einer eigenen
        // Gruppe dahinter.
        let (projekt_bloecke, nutzer_bloecke): (Vec<(String, String)>, Vec<(String, String)>) = {
            let t = self.tab();
            let mut p = Vec::new();
            let mut u = Vec::new();
            for id in &t.offene_bloecke {
                if let Some(b) = t.session.doc.block(id) {
                    match b.source {
                        jjcad_core::model::BlockSource::User => u.push((id.clone(), b.name.clone())),
                        _ => p.push((id.clone(), b.name.clone())),
                    }
                }
            }
            (p, u)
        };
        let aktuell = self.tab().tab.clone();
        // Luft zwischen den Blatt-Chips: ohne sie stoßen die Umrisse zweier
        // Chips aneinander und überlappen um ein bis drei Pixel.
        const BLATT_ABSTAND: f32 = 4.0;
        let mut blatt_rechtecke: Vec<Rect> = Vec::new();

        ui.horizontal_centered(|ui| {
            // Abstände hier von Hand: die Zonen sollen ohne egui-Polster anliegen.
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(6.0);

            // ---- Zone 1: Dateien (eigener, dunklerer Grund) ----
            let breiten: Vec<f32> = dateien
                .iter()
                .map(|(_, name, _)| Self::datei_breite(ui, name, mehrere_dateien))
                .collect();
            let breite = 12.0 + breiten.iter().sum::<f32>() + 9.0;
            let (zone, _) = ui.allocate_exact_size(Vec2::new(breite, DOCTABS_H), Sense::hover());
            // Der Grund der Dateizone beginnt am Fensterrand — ein Band, kein Kasten
            // mitten im Streifen.
            let band = Rect::from_min_max(Pos2::new(ui.max_rect().left(), zone.top()), zone.max);
            ui.painter().rect_filled(band, Rounding::ZERO, Palette::bg());
            ui.painter().line_segment(
                [
                    Pos2::new(band.left(), band.bottom() - 0.5),
                    Pos2::new(band.right(), band.bottom() - 0.5),
                ],
                Stroke::new(1.0_f32, Palette::border_soft()),
            );
            let mut datei_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(zone)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            datei_ui.spacing_mut().item_spacing.x = 0.0;
            datei_ui.add_space(12.0);
            for ((idx, name, geaendert), breite) in dateien.iter().zip(breiten) {
                let (geklickt, geschlossen) = Self::datei_reiter(
                    &mut datei_ui,
                    name,
                    breite,
                    *idx == aktiv,
                    mehrere_dateien,
                    *geaendert,
                    egui::Id::new(("datei", *idx)),
                );
                if geschlossen {
                    schliesse_datei = Some(*idx);
                } else if geklickt {
                    wechsle_datei = Some(*idx);
                }
            }

            // Trenner an der Kante der Dateizone: die beiden Bereiche stoßen
            // nicht bloß aneinander, sie sind durch eine Linie geschieden.
            ui.painter().line_segment(
                [
                    Pos2::new(zone.right() - 1.0, zone.top() + 6.0),
                    Pos2::new(zone.right() - 1.0, zone.bottom() - 6.0),
                ],
                Stroke::new(1.0_f32, Palette::border()),
            );
            ui.add_space(8.0);

            // ---- Zone 2: Blätter der aktiven Datei (eigene Fläche) ----
            let blattbreiten: Vec<f32> = reiter
                .iter()
                .map(|(_, name, closable)| Self::blatt_breite(ui, name, *closable))
                .collect();
            let blockbreiten: Vec<f32> = projekt_bloecke
                .iter()
                .map(|(_, name)| Self::blatt_breite(ui, name, true))
                .collect();
            let anzahl = blattbreiten.len() + blockbreiten.len();
            let tray_breite = 12.0
                + blattbreiten.iter().sum::<f32>()
                + blockbreiten.iter().sum::<f32>()
                + BLATT_ABSTAND * (anzahl.saturating_sub(1)) as f32
                + 8.0
                + 20.0
                + 10.0;
            let (tray, _) = ui.allocate_exact_size(
                Vec2::new(tray_breite, DOCTABS_H - 6.0),
                Sense::hover(),
            );
            ui.painter().rect_filled(tray, Rounding::same(9.0), Palette::panel());
            ui.painter().rect_stroke(
                tray,
                Rounding::same(9.0),
                Stroke::new(1.0_f32, Palette::border_soft()),
            );
            let mut blatt_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(tray)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            blatt_ui.spacing_mut().item_spacing.x = 0.0;
            blatt_ui.add_space(12.0);
            for (i, ((kind, name, closable), breite)) in
                reiter.iter().zip(blattbreiten).enumerate()
            {
                if i > 0 {
                    blatt_ui.add_space(BLATT_ABSTAND);
                }
                let icon = match kind {
                    TabKind::Model => "layers",
                    TabKind::Sheet(_) => "sheet",
                    TabKind::Block(_) => "block",
                };
                let (geklickt, geschlossen, rechteck) = Self::blatt_chip(
                    &mut blatt_ui,
                    name,
                    icon,
                    breite,
                    aktuell == *kind,
                    *closable,
                    egui::Id::new(("blatt", name)),
                );
                blatt_rechtecke.push(rechteck);
                if geschlossen {
                    if let TabKind::Sheet(id) = kind {
                        schliesse_blatt = Some(*id);
                    }
                } else if geklickt {
                    wechsle_reiter = Some(kind.clone());
                }
            }
            // Blöcke **dieser Datei** stehen im selben Oval wie die Blätter.
            for ((bid, name), breite) in projekt_bloecke.iter().zip(blockbreiten) {
                blatt_ui.add_space(BLATT_ABSTAND);
                let kind = TabKind::Block(bid.clone());
                let (geklickt, geschlossen, rechteck) = Self::blatt_chip(
                    &mut blatt_ui,
                    name,
                    "block",
                    breite,
                    aktuell == kind,
                    true,
                    egui::Id::new(("block", bid)),
                );
                blatt_rechtecke.push(rechteck);
                if geschlossen {
                    schliesse_block = Some(bid.clone());
                } else if geklickt {
                    wechsle_reiter = Some(kind);
                }
            }
            blatt_ui.add_space(6.0);
            let (r, resp) = blatt_ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
            if resp.hovered() {
                blatt_ui
                    .painter()
                    .rect_filled(r, Rounding::same(5.0), Palette::panel_alt());
            }
            crate::icons::paint(
                blatt_ui.painter(),
                r.shrink(4.0),
                "plus",
                if resp.hovered() { Palette::text() } else { Palette::text_faint() },
            );
            if resp.clicked() {
                neues_blatt = true;
            }
            resp.on_hover_text(jjcad_core::i18n::tr("gui.app.neues_blatt"));

            // ---- Zone 3: Blöcke des Benutzers (eigene Gruppe) ----
            // Sie gehören zur Bibliothek, nicht zur Datei — deshalb eine eigene
            // Fläche hinter der Blattzone (T0137).
            if !nutzer_bloecke.is_empty() {
                ui.add_space(8.0);
                let breiten: Vec<f32> = nutzer_bloecke
                    .iter()
                    .map(|(_, name)| Self::blatt_breite(ui, name, true))
                    .collect();
                let breite = 10.0
                    + breiten.iter().sum::<f32>()
                    + BLATT_ABSTAND * (breiten.len().saturating_sub(1)) as f32
                    + 10.0;
                let (tray, _) = ui.allocate_exact_size(
                    Vec2::new(breite, DOCTABS_H - 6.0),
                    Sense::hover(),
                );
                ui.painter().rect_filled(tray, Rounding::same(9.0), Palette::panel_alt());
                ui.painter().rect_stroke(
                    tray,
                    Rounding::same(9.0),
                    Stroke::new(1.0_f32, Palette::border()),
                );
                let mut nutzer_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(tray)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                nutzer_ui.spacing_mut().item_spacing.x = 0.0;
                nutzer_ui.add_space(10.0);
                for (i, ((bid, name), breite)) in
                    nutzer_bloecke.iter().zip(breiten).enumerate()
                {
                    if i > 0 {
                        nutzer_ui.add_space(BLATT_ABSTAND);
                    }
                    let kind = TabKind::Block(bid.clone());
                    let (geklickt, geschlossen, rechteck) = Self::blatt_chip(
                        &mut nutzer_ui,
                        name,
                        "block",
                        breite,
                        aktuell == kind,
                        true,
                        egui::Id::new(("block", bid)),
                    );
                    blatt_rechtecke.push(rechteck);
                    if geschlossen {
                        schliesse_block = Some(bid.clone());
                    } else if geklickt {
                        wechsle_reiter = Some(kind);
                    }
                }
            }
        });
        if !blatt_rechtecke.is_empty() {
            self.ui.last_blattrechtecke = Some(blatt_rechtecke);
        }

        // Erst schließen, dann wechseln: ein geschlossener Reiter darf nicht
        // als Ziel übrig bleiben.
        if let Some(idx) = wechsle_datei {
            self.switch_doc(idx);
        }
        if let Some(idx) = schliesse_datei {
            self.request_close_tab(idx);
        }
        if let Some(kind) = wechsle_reiter {
            self.switch_tab(kind);
        }
        if let Some(id) = schliesse_block {
            // Reiterkreuz = verwerfen (der Öffnungsstand kommt zurück).
            self.schliesse_block_editor(&id, false);
        }
        if let Some(id) = schliesse_blatt {
            // T0218: Das Kreuz schließt nur den **Reiter**. Das Blatt bleibt in
            // der Zeichnung (und im Blattmanager erreichbar) — gelöscht wird
            // hier nichts mehr.
            self.blatt_tab_schliessen(id);
        }
        if neues_blatt {
            self.run_action(ui.ctx(), "sheet.new");
        }
    }

    /// Ist der Reiter dieses Blatts offen? (T0218)
    ///
    /// Das Schließen eines Reiters löscht kein Blatt mehr — es merkt die
    /// Kennung in `DocTab::geschlossene_blaetter`. Der Blattmanager listet
    /// **alle** Blätter und holt den Reiter per Klick zurück.
    pub fn blatt_tab_offen(&self, id: Id) -> bool {
        !self.tab().geschlossene_blaetter.contains(&id)
    }

    /// Schließt den **Reiter** eines Blatts; das Blatt bleibt in der Zeichnung.
    pub fn blatt_tab_schliessen(&mut self, id: Id) {
        if self.blatt_tab_offen(id) {
            self.tab_mut().geschlossene_blaetter.push(id);
        }
        if self.tab().tab == TabKind::Sheet(id) {
            self.switch_tab(TabKind::Model);
        }
    }

    /// Öffnet den Reiter eines Blatts und wechselt dorthin (T0218).
    pub fn blatt_oeffnen(&mut self, id: Id) {
        self.tab_mut().geschlossene_blaetter.retain(|x| *x != id);
        self.switch_tab(TabKind::Sheet(id));
    }

    /// Kennungen der Blätter im **Plansatz** — in der Reihenfolge der
    /// Blattliste, also so, wie die PDF blättert (T0218).
    pub fn plansatz_ids(&self) -> Vec<Id> {
        self.doc()
            .sheets
            .iter()
            .filter(|s| s.plansatz)
            .map(|s| s.id)
            .collect()
    }

    /// Breite eines Datei-Reiters — dieselben Polster wie in `datei_reiter`.
    fn datei_breite(ui: &egui::Ui, name: &str, schliessbar: bool) -> f32 {
        let galley = ui.painter().layout_no_wrap(
            name.to_string(),
            FontId::proportional(12.0),
            Palette::text(),
        );
        (galley.size().x + 30.0 + if schliessbar { 26.0 } else { 12.0 }).max(78.0)
    }

    /// Breite eines Blatt-Chips — dieselben Polster wie in `blatt_chip`.
    fn blatt_breite(ui: &egui::Ui, name: &str, schliessbar: bool) -> f32 {
        let galley = ui.painter().layout_no_wrap(
            name.to_string(),
            FontId::proportional(11.0),
            Palette::text(),
        );
        (galley.size().x + 24.0 + if schliessbar { 22.0 } else { 10.0 }).max(54.0)
    }

    /// Ein **Datei**-Reiter: volle Streifenhöhe, Dateisymbol, Name. Der aktive
    /// liegt als Karte mit abgerundeter Oberkante im Streifen und trägt unten eine
    /// Akzentlinie; ein Punkt links vom Symbol heißt „ungespeichert“.
    /// Gibt (angeklickt, Kreuz getroffen) zurück.
    fn datei_reiter(
        ui: &mut egui::Ui,
        name: &str,
        breite: f32,
        aktiv: bool,
        schliessbar: bool,
        geaendert: bool,
        id: egui::Id,
    ) -> (bool, bool) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(breite, DOCTABS_H), Sense::hover());
        let resp = ui.interact(rect, id, Sense::click());
        let karte = Rect::from_min_max(rect.min + Vec2::new(0.0, 4.0), rect.max);
        let rundung = Rounding {
            nw: 7.0,
            ne: 7.0,
            sw: 0.0,
            se: 0.0,
        };
        if aktiv {
            ui.painter().rect_filled(karte, rundung, Palette::panel_alt());
            ui.painter().line_segment(
                [
                    Pos2::new(karte.left() + 8.0, karte.bottom() - 1.0),
                    Pos2::new(karte.right() - 8.0, karte.bottom() - 1.0),
                ],
                Stroke::new(2.0, Palette::accent()),
            );
        } else if resp.hovered() {
            ui.painter().rect_filled(karte, rundung, Palette::panel());
        }
        if geaendert {
            ui.painter().circle_filled(
                Pos2::new(rect.left() + 7.0, rect.center().y),
                2.2,
                Palette::accent(),
            );
        }
        crate::icons::paint(
            ui.painter(),
            Rect::from_min_size(
                Pos2::new(rect.left() + 13.0, rect.center().y - 7.0),
                Vec2::splat(14.0),
            ),
            "datei",
            if aktiv { Palette::accent() } else { Palette::text_faint() },
        );
        let farbe = if aktiv { Palette::text() } else { Palette::text_dim() };
        let galley = ui
            .painter()
            .layout_no_wrap(name.to_string(), FontId::proportional(12.0), farbe);
        ui.painter().galley(
            Pos2::new(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0),
            galley,
            farbe,
        );
        let geschlossen = schliessbar && Self::kreuz(ui, rect, id);
        if geschlossen {
            (false, true)
        } else {
            (resp.clicked(), false)
        }
    }

    /// Ein **Blatt**-Chip: kleinere Höhe, kleinerer Text, eigene Fläche in der
    /// Blattzone. Der aktive Chip trägt einen Akzentumriss um die ganze Fläche.
    /// Gibt (angeklickt, Kreuz getroffen, eigenes Rechteck) zurück — die Fläche
    /// braucht `doc_tabs` für die Abstandsprüfung.
    fn blatt_chip(
        ui: &mut egui::Ui,
        name: &str,
        icon: &str,
        breite: f32,
        aktiv: bool,
        schliessbar: bool,
        id: egui::Id,
    ) -> (bool, bool, Rect) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(breite, 22.0), Sense::hover());
        let resp = ui.interact(rect, id, Sense::click());
        if aktiv {
            ui.painter().rect_filled(rect, Rounding::same(6.0), Palette::panel_alt());
            ui.painter().rect_stroke(
                rect,
                Rounding::same(6.0),
                Stroke::new(1.0_f32, Palette::accent()),
            );
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, Rounding::same(6.0), Palette::panel_alt());
        }
        crate::icons::paint(
            ui.painter(),
            Rect::from_min_size(
                Pos2::new(rect.left() + 7.0, rect.center().y - 6.0),
                Vec2::splat(12.0),
            ),
            icon,
            if aktiv { Palette::accent() } else { Palette::text_faint() },
        );
        let farbe = if aktiv { Palette::text() } else { Palette::text_dim() };
        let galley = ui
            .painter()
            .layout_no_wrap(name.to_string(), FontId::proportional(11.0), farbe);
        ui.painter().galley(
            Pos2::new(rect.left() + 22.0, rect.center().y - galley.size().y / 2.0),
            galley,
            farbe,
        );
        let geschlossen = schliessbar && Self::kreuz(ui, rect, id);
        if geschlossen {
            (false, true, rect)
        } else {
            (resp.clicked(), false, rect)
        }
    }

    /// Schließen-Kreuz am rechten Rand eines Reiters; `true`, wenn es getroffen wurde.
    fn kreuz(ui: &mut egui::Ui, rect: Rect, id: egui::Id) -> bool {
        let xr = Rect::from_center_size(
            Pos2::new(rect.right() - 12.0, rect.center().y),
            Vec2::splat(16.0),
        );
        let xresp = ui.interact(xr, id.with("schliessen"), Sense::click());
        let c = if xresp.hovered() { Palette::danger() } else { Palette::text_faint() };
        ui.painter().line_segment(
            [xr.center() - Vec2::splat(3.5), xr.center() + Vec2::splat(3.5)],
            Stroke::new(1.4, c),
        );
        ui.painter().line_segment(
            [xr.center() + Vec2::new(3.5, -3.5), xr.center() + Vec2::new(-3.5, 3.5)],
            Stroke::new(1.4, c),
        );
        xresp.clicked()
    }

    // ---- Kopfzeile + Ribbon ----

    fn header(&mut self, ui: &mut egui::Ui) {
        // T0098: Ohne Titelleiste ist die Kopfzeile der Griff, mit dem das
        // Fenster über den Desktop wandert.
        self.fensterkopf_griff(ui);
        let ohne_titelleiste = self.fenster_ohne_titelleiste();
        ui.horizontal_centered(|ui| {
            ui.add_space(10.0);
            // Markenzeichen (T0083): dasselbe Bild wie Fenstericon und Über-Dialog,
            // 24 px wie der bisher gezeichnete Platzhalter (Kopfzeile bleibt gleich hoch).
            let marke = crate::icon::textur_klein(ui.ctx());
            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                marke.id(),
                Vec2::new(30.0, 30.0),
            )));
            ui.add_space(4.0);
            ui.label(theme::bold(jjcad_core::i18n::tr("gui.app.joja_cad"), 14.0));
            ui.label(
                RichText::new(jjcad_core::i18n::t_args_owned("gui.app.vx", &[("wert1", format!("{}", env!("CARGO_PKG_VERSION")))]))
                    .size(9.5)
                    .color(Palette::text_faint()),
            );
            ui.add_space(14.0);
            widgets::vline(ui, 24.0);
            ui.add_space(8.0);

            // Ribbon-Reiter
            for rt in RibbonTab::ALL {
                let active = self.ui.ribbon == rt;
                let galley = ui.painter().layout_no_wrap(
                    rt.label().to_string(),
                    FontId::proportional(12.5),
                    if active { Palette::text() } else { Palette::text_dim() },
                );
                let w = galley.size().x + 22.0;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, HEADER_H - 4.0), Sense::click());
                if active {
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            Pos2::new(rect.left(), rect.bottom() - 2.0),
                            Pos2::new(rect.right(), rect.bottom()),
                        ),
                        Rounding::ZERO,
                        Palette::accent(),
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, Rounding::same(6.0), Palette::panel());
                }
                ui.painter().galley(
                    Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0 - 1.0),
                    galley,
                    if active { Palette::text() } else { Palette::text_dim() },
                );
                if resp.clicked() {
                    self.ui.ribbon = rt;
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // T0098: Ohne Titelleiste stehen die drei Fensterknöpfe ganz
                // rechts (im Rechts-nach-links-Layout ist der zuerst gelegte
                // der äußerste, also Schließen).
                if ohne_titelleiste {
                    self.fenster_knoepfe(ui);
                    widgets::vline(ui, 20.0);
                }
                ui.add_space(8.0);
                let (ric, rtip) = if self.ui.ribbon_collapsed {
                    ("chevron_down", jjcad_core::i18n::tr("gui.app.ribbon_ausklappen"))
                } else {
                    ("chevron_up", jjcad_core::i18n::tr("gui.app.ribbon_einklappen"))
                };
                if widgets::icon_button(ui, ric, 26.0, self.ui.ribbon_collapsed, rtip).clicked() {
                    self.ui.ribbon_collapsed = !self.ui.ribbon_collapsed;
                }
                if widgets::icon_button(ui, "help", 26.0, false, jjcad_core::i18n::tr("gui.app.hilfe_f1")).clicked() {
                    self.ui.help_open = true;
                }
                if widgets::icon_button(ui, "keyboard", 26.0, self.ui.keymap_open, jjcad_core::i18n::tr("gui.app.tastaturbelegung")).clicked() {
                    self.ui.keymap_open = !self.ui.keymap_open;
                }
                if widgets::icon_button(ui, "settings", 26.0, self.ui.settings_open, jjcad_core::i18n::tr("gui.app.einstellungen")).clicked() {
                    self.ui.settings_open = !self.ui.settings_open;
                }
                ui.add_space(6.0);
                widgets::vline(ui, 20.0);
                let can_redo = self.session().history.can_redo();
                let can_undo = self.session().history.can_undo();
                if widgets::icon_button(ui, "redo", 26.0, false, jjcad_core::i18n::tr("gui.app.wiederholen_strg_y")).clicked() && can_redo {
                    self.run_action(ui.ctx(), "edit.redo");
                }
                if widgets::icon_button(ui, "undo", 26.0, false, jjcad_core::i18n::tr("gui.app.rueckgaengig_strg_z")).clicked() && can_undo {
                    self.run_action(ui.ctx(), "edit.undo");
                }
                ui.add_space(6.0);
                let name = self.doc().name.clone();
                let modified = self.doc().modified;
                ui.label(
                    RichText::new(format!("{}{}", name, if modified { " •" } else { "" }))
                        .size(11.5)
                        .color(Palette::text_dim()),
                );
            });
        });
    }

    /// `true`, wenn JoJa-CAD ein Fenster ohne Titelleiste führt (T0098).
    pub fn fenster_ohne_titelleiste(&self) -> bool {
        self.prefs.fensterrahmen.ohne_titelleiste()
    }

    /// Der Fensterausschnitt **vor** dem Schattenrand, nur lesend (T0147).
    ///
    /// Die gespeicherte Fenstergröße ist die des ganzen Fensters, nicht die der
    /// sichtbaren Fläche — sonst schrumpfte das Fenster bei jedem Start um den
    /// Streifen.
    fn fenster_ausschnitt(&self) -> Rect {
        self.ui
            .fenster_aussen
            .unwrap_or_else(|| self.ctx.screen_rect())
    }

    /// Fensterfläche und Fensterschatten (T0147).
    ///
    /// Ohne Titelleiste zeichnet der Fenstermanager keinen Schatten mehr — das
    /// Fenster lag ohne Abhebung auf dem Hintergrund. JoJa-CAD zeichnet ihn
    /// deshalb selbst: ein Streifen von [`SCHATTEN_RAND`] Punkten am Fensterrand
    /// bleibt durchsichtig (Fenster mit `with_transparent`, `clear_color` mit
    /// Alpha 0), die Oberfläche liegt darin, und im Streifen liegt der weiche
    /// Schatten.
    ///
    /// Den kleineren Bildschirmausschnitt setzt [`eframe::App::raw_input_hook`]
    /// — Panels, Dialoge, Menüs und die Fensterkanten rechnen damit von selbst
    /// auf der sichtbaren Fläche.
    fn fensterflaeche(&mut self, ctx: &egui::Context) {
        let innen = ctx.screen_rect();
        let aussen = self.ui.fenster_aussen.unwrap_or(innen);
        if aussen != innen {
            // Der Schatten liegt **unter** der Oberfläche und reicht über die
            // sichtbare Fläche hinaus — eigener Maler mit dem ganzen Fenster
            // als Beschnitt, sonst schnitte der verkleinerte Ausschnitt ihn ab.
            egui::Painter::new(ctx.clone(), egui::LayerId::background(), aussen).add(
                egui::epaint::Shadow {
                    offset: egui::vec2(0.0, SCHATTEN_VERSATZ),
                    blur: 2.0 * SCHATTEN_RAND,
                    spread: 0.0,
                    color: egui::Color32::from_black_alpha(120),
                }
                .as_shape(innen, egui::Rounding::ZERO),
            );
        }
        // Grundfläche unter allem: bleibt zwischen zwei Flächen ein Pixel
        // unbedeckt (krumme Bildschirmskalierung), steht dort die Fensterfarbe
        // und nicht der Grund des Fenstersystems.
        egui::Painter::new(ctx.clone(), egui::LayerId::background(), aussen)
            .rect_filled(innen, egui::Rounding::ZERO, Palette::bg());
    }

    /// `true`, wenn der Fenstermanager das Fenster als maximiert meldet (T0119).
    ///
    /// Maßgeblich ist der **Fensterzustand**, nicht ein eigener Schalter: nur so
    /// stimmt die Bedienung mit dem überein, was das Betriebssystem annimmt.
    /// Ein maximiertes Fenster füllt den Bildschirm — dort darf die Größe nicht
    /// gezogen werden, sonst ändert sich die Zeichnung, während der
    /// Fenstermanager weiter „bildschirmfüllend“ meldet.
    pub fn fenster_maximiert(ctx: &egui::Context) -> bool {
        ctx.input(|i| i.viewport().maximized.unwrap_or(false))
    }

    /// Griff der Kopfzeile (T0098).
    ///
    /// Ohne Titelleiste gibt es keinen Fenstergriff: ein Zug auf die Kopfzeile
    /// verschiebt das Fenster (`ViewportCommand::StartDrag` — winit schickt
    /// `_NET_WM_MOVERESIZE` unter X11, `xdg_toplevel.move` unter Wayland und
    /// `WM_NCLBUTTONDOWN`/`HTCAPTION` unter Windows), ein Doppelklick
    /// vergrößert und stellt wieder her.
    ///
    /// Die Fläche wird **vor** Reitern und Knöpfen angelegt und liegt damit
    /// darunter: egui nimmt bei überlappenden Elementen das zuletzt angelegte,
    /// die Schaltflächen der Kopfzeile bleiben also bedienbar.
    fn fensterkopf_griff(&mut self, ui: &mut egui::Ui) {
        if !self.fenster_ohne_titelleiste() {
            return;
        }
        let griff = ui.interact(
            ui.max_rect(),
            egui::Id::new("fensterkopf_griff"),
            Sense::click_and_drag(),
        );
        let ctx = ui.ctx().clone();
        if griff.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            // Den Zug sofort beenden: der Fenstermanager übernimmt jetzt den
            // Zeiger und schickt das Loslassen an niemanden. egui bliebe sonst
            // für immer „am Ziehen“ (kein neuer `drag_started`), und jeder
            // zweite Zug auf die Kopfzeile verschöbe das Fenster nicht (T0098).
            ctx.stop_dragging();
        }
        if griff.double_clicked() {
            let maximiert = Self::fenster_maximiert(&ctx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximiert));
        }
    }

    /// Die drei Fensterknöpfe oben rechts (T0098) — Minimieren, Vergrößern
    /// (bzw. Wiederherstellen, wenn das Fenster schon groß ist) und Schließen.
    fn fenster_knoepfe(&mut self, ui: &mut egui::Ui) {
        let hoehe = ui.available_height().max(HEADER_H);
        let maximiert = Self::fenster_maximiert(ui.ctx());
        let mut rects = [Rect::NOTHING; 3];
        // Die drei Knöpfe stoßen aneinander wie in einer Titelleiste — ohne
        // Abstand dazwischen (egui setzt sonst 8 px).
        let abstand = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 0.0;

        // Im Rechts-nach-links-Layout zuerst gelegt = am weitesten rechts.
        let (r, resp) = Self::fenster_knopf(
            ui,
            "close",
            jjcad_core::i18n::tr("gui.app.fenster_schliessen"),
            hoehe,
            true,
        );
        rects[2] = r;
        if resp.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let (icon, tip) = if maximiert {
            (
                "window_restore",
                jjcad_core::i18n::tr("gui.app.fenster_wiederherstellen"),
            )
        } else {
            (
                "window_max",
                jjcad_core::i18n::tr("gui.app.fenster_vergroessern"),
            )
        };
        let (r, resp) = Self::fenster_knopf(ui, icon, tip, hoehe, false);
        rects[1] = r;
        if resp.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximiert));
        }
        let (r, resp) = Self::fenster_knopf(
            ui,
            "window_min",
            jjcad_core::i18n::tr("gui.app.fenster_minimieren"),
            hoehe,
            false,
        );
        rects[0] = r;
        if resp.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        ui.spacing_mut().item_spacing.x = abstand;
        self.ui.last_fenster_knoepfe = Some(rects);
    }

    /// Ein Fensterknopf: volle Höhe der Kopfzeile, ohne Rundung, Hover-Fläche;
    /// Schließen färbt rot.
    fn fenster_knopf(
        ui: &mut egui::Ui,
        icon: &str,
        tooltip: &str,
        hoehe: f32,
        schliessen: bool,
    ) -> (Rect, egui::Response) {
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(FENSTERKNOPF_W, hoehe), Sense::click());
        let hover_flaeche = if schliessen {
            Palette::danger()
        } else {
            Palette::panel_alt()
        };
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(rect, Rounding::ZERO, hover_flaeche.gamma_multiply(0.55));
            }
            crate::icons::paint(
                p,
                Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
                icon,
                if resp.hovered() {
                    Palette::text()
                } else {
                    Palette::text_dim()
                },
            );
        }
        (rect, resp.on_hover_text(tooltip))
    }

    /// Ziehen an den Fensterkanten (T0098).
    ///
    /// Ohne Titelleiste fehlt der Rahmen, an dem der Fenstermanager sonst die
    /// Größe ändert. Acht schmale Zonen in `Order::Foreground` nehmen den Druck
    /// an und schicken `BeginResize`; winit leitet das an den Fenstermanager
    /// weiter (`_NET_WM_MOVERESIZE` unter X11, `xdg_toplevel.resize` unter
    /// Wayland, `WM_NCLBUTTONDOWN` unter Windows). Die Zonen liegen über allen
    /// Flächen, damit ein Zug am Rand nicht zugleich in der Zeichnung landet.
    fn fensterkanten(&mut self, ctx: &egui::Context) {
        self.ui.last_fensterkanten = None;
        // T0119: Ein maximiertes Fenster füllt den Bildschirm — der
        // Fenstermanager hält es dort fest. Ein Größe-Zug an der Kante würde
        // die Zeichnung verkleinern, während das Betriebssystem weiter
        // „bildschirmfüllend“ meldet; die Bedienung liefe auseinander. Also
        // gibt es Kantenzonen nur im wiederhergestellten Fenster.
        if !self.fenster_ohne_titelleiste() || self.ui.fullscreen || Self::fenster_maximiert(ctx) {
            // Keine Zonen heißt auch: keine Markierung, kein laufender Zug.
            self.ui.kanten_verweil = None;
            self.ui.kanten_leuchten = None;
            self.ui.kanten_zug = None;
            return;
        }
        let f = ctx.screen_rect();
        let b = FENSTERKANTE_PX;
        let e = FENSTERECKE_PX;
        if f.width() <= 4.0 * e || f.height() <= 4.0 * e {
            return;
        }
        // Die drei Fensterknöpfe stehen in der Ecke oben rechts und haben
        // Vorrang: dort darf weder ein Griff liegen noch das Größe-Ziehen
        // beginnen, sonst wäre „Schließen“ nicht mehr zu treffen (T0098, Turn 5).
        let knoepfe = self.ui.last_fenster_knoepfe.unwrap_or([Rect::NOTHING; 3]);
        let knopf_links = knoepfe
            .iter()
            .map(|r| r.left())
            .fold(f32::INFINITY, f32::min)
            .min(f.right() - e);
        let knopf_unten = knoepfe[0].bottom().min(f.bottom() - e);
        let zonen: [(&'static str, Rect, egui::ResizeDirection, egui::CursorIcon); 7] = [
            (
                "nw",
                Rect::from_min_max(f.min, egui::pos2(f.left() + e, f.top() + e)),
                egui::ResizeDirection::NorthWest,
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                "sw",
                Rect::from_min_max(
                    egui::pos2(f.left(), f.bottom() - e),
                    egui::pos2(f.left() + e, f.bottom()),
                ),
                egui::ResizeDirection::SouthWest,
                egui::CursorIcon::ResizeNeSw,
            ),
            (
                "so",
                Rect::from_min_max(
                    egui::pos2(f.right() - e, f.bottom() - e),
                    egui::pos2(f.right(), f.bottom()),
                ),
                egui::ResizeDirection::SouthEast,
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                "n",
                Rect::from_min_max(
                    egui::pos2(f.left() + e, f.top()),
                    egui::pos2(knopf_links, f.top() + b),
                ),
                egui::ResizeDirection::North,
                egui::CursorIcon::ResizeVertical,
            ),
            (
                "s",
                Rect::from_min_max(
                    egui::pos2(f.left() + e, f.bottom() - b),
                    egui::pos2(f.right() - e, f.bottom()),
                ),
                egui::ResizeDirection::South,
                egui::CursorIcon::ResizeVertical,
            ),
            (
                "w",
                Rect::from_min_max(
                    egui::pos2(f.left(), f.top() + e),
                    egui::pos2(f.left() + b, f.bottom() - e),
                ),
                egui::ResizeDirection::West,
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                "o",
                Rect::from_min_max(
                    egui::pos2(f.right() - b, f.top() + e.max(knopf_unten)),
                    egui::pos2(f.right(), f.bottom() - e),
                ),
                egui::ResizeDirection::East,
                egui::CursorIcon::ResizeHorizontal,
            ),
        ];
        // Zeiger und Druck **einmal** aus den Rohdaten lesen: die Zonen sind
        // schmale Streifen, und `Response::hovered` hängt an der Reihenfolge der
        // Ebenen — über der Zeichenfläche wäre die Kante sonst kaum zu treffen.
        let (zeiger, gedrueckt, bewegt) = ctx.input(|i| {
            (
                i.pointer.interact_pos(),
                i.pointer.primary_pressed(),
                i.pointer.delta() != Vec2::ZERO,
            )
        });
        if bewegt {
            self.ui.zeiger_bewegt_seit = ctx.input(|i| i.time);
        }
        let ueber_knopf = |p: Pos2| knoepfe.iter().any(|r| r.contains(p));
        let mut lagen = Vec::with_capacity(zonen.len());
        for (name, rect, richtung, cursor) in zonen {
            // Die Zone liegt als eigene Fläche über allem: ein Druck am Rand
            // darf nicht zugleich in der Zeichnung landen.
            egui::Area::new(egui::Id::new(("fensterkante", name)))
                .fixed_pos(rect.min)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.allocate_exact_size(rect.size(), Sense::click_and_drag())
                });
            if zeiger.map_or(false, |p| rect.contains(p) && !ueber_knopf(p)) {
                ctx.set_cursor_icon(cursor);
                // Nur der Druck beginnt den Zug — ein Dauerlauf über
                // `is_pointer_button_down_on` würde ihn jeden Frame neu starten.
                if gedrueckt {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(richtung));
                    // Der Zug gehört zu dieser Kante, bis die Taste losgelassen
                    // wird (Turn 9) — die Markierung bleibt dabei stehen.
                    self.ui.kanten_zug = Some(name);
                    self.ui.kanten_zug_seit = ctx.input(|i| i.time);
                    self.ui.kanten_zug_start = self.ui.kanten_zug_seit;
                    self.ui.kanten_zug_groesse = (f.width().round(), f.height().round());
                }
            }
            lagen.push((name, rect));
        }

        // Sichtbare Andeutung: die Kante unter dem Zeiger wird hervorgehoben,
        // unten rechts steht ein Größen-Griff. Ohne diese Anzeige war die
        // Fläche unsichtbar — sie wurde schlicht nicht gefunden (T0098).
        //
        // Die Markierung erscheint **erst nach kurzem Verweilen** (Turn 7): ein
        // Zeiger, der nur über den Rand streicht, soll nichts aufblitzen lassen.
        let jetzt = ctx.input(|i| i.time);
        let kante_unter_zeiger = zeiger.and_then(|p| {
            lagen
                .iter()
                .find(|(name, rect)| name.len() == 1 && rect.contains(p) && !ueber_knopf(p))
                .map(|(name, _)| *name)
        });
        match (kante_unter_zeiger, self.ui.kanten_verweil) {
            (Some(neu), Some((alt, _))) if neu == alt => {}
            (Some(neu), _) => self.ui.kanten_verweil = Some((neu, jetzt)),
            (None, _) => self.ui.kanten_verweil = None,
        }
        // Ende des Zuges: Loslassen gesehen oder Taste nicht mehr gedrückt.
        // **Nicht** am „Zeiger verlässt das Fenster“ aufhängen: beim Ziehen
        // übernimmt der Fenstermanager den Zeiger und meldet ihn als weg
        // (`has_pointer() == false`) — die Markierung verschwände mitten im
        // Zug. Als Sicherheitsnetz gegen ein verschlucktes Loslassen fällt sie
        // nach einigen Sekunden von selbst ab.
        let (taste_unten, losgelassen) =
            ctx.input(|i| (i.pointer.primary_down(), i.pointer.any_released()));
        if losgelassen || !taste_unten {
            self.ui.kanten_zug = None;
        }
        if self.ui.kanten_zug.is_some() {
            // Die Markierung hängt an der Größenänderung: solange der
            // Fenstermanager zieht, ändert sich die Größe, und jede Änderung
            // verlängert den Nachlauf. Bleibt sie stehen, ist der Zug vorbei —
            // dann fällt die Markierung nach `ZUG_NACHLAUF_S` ab. Verglichen
            // werden **gerundete** Pixel: bei krummer Skalierung zappelt die
            // Punktgröße sonst um Bruchteile und der Nachlauf liefe nie ab.
            let groesse = (f.width().round(), f.height().round());
            if groesse != self.ui.kanten_zug_groesse {
                self.ui.kanten_zug_groesse = groesse;
                self.ui.kanten_zug_seit = jetzt;
            }
            // Wandert der Zeiger frisch bewegt weit von der gezogenen Kante weg,
            // ist der Zug vorbei — das endet die Markierung sofort, auch wenn
            // das Loslassen verschluckt wurde.
            if jetzt - self.ui.zeiger_bewegt_seit < 0.2 {
                if let (Some(name), Some(p)) = (self.ui.kanten_zug, zeiger) {
                    let abstand = match name {
                        "n" => p.y - f.top(),
                        "s" => f.bottom() - p.y,
                        "w" => p.x - f.left(),
                        _ => f.right() - p.x,
                    };
                    if abstand > 40.0 {
                        self.ui.kanten_zug = None;
                    }
                }
            }
            if jetzt - self.ui.kanten_zug_seit >= ZUG_NACHLAUF_S
                || jetzt - self.ui.kanten_zug_start >= ZUG_MAX_S
            {
                self.ui.kanten_zug = None;
            }
            if self.ui.kanten_zug.is_some() {
                // Ohne Anforderung käme kein Bild mehr — der Nachlauf würde nie
                // ausgewertet und die Markierung bliebe stehen.
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                    (ZUG_NACHLAUF_S - (jetzt - self.ui.kanten_zug_seit)).max(0.02),
                ));
            }
        }
        let leuchten = self.ui.kanten_zug.or_else(|| {
            self.ui
                .kanten_verweil
                .filter(|(_, seit)| jetzt - seit >= KANTEN_VERWEILDAUER)
                .map(|(name, _)| name)
        });
        self.ui.kanten_leuchten = leuchten;
        if leuchten.is_none() {
            if let Some((_, seit)) = self.ui.kanten_verweil {
                // Ohne Anforderung käme kein Bild mehr, solange der Zeiger ruht —
                // die Markierung erschiene erst beim nächsten Klick.
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                    (KANTEN_VERWEILDAUER - (jetzt - seit)).max(0.0),
                ));
            }
        }
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("fensterkanten_anzeige"),
        ));
        if let Some(name) = leuchten {
            let strich = match name {
                "n" => [
                    egui::pos2(f.left() + e, f.top() + 1.0),
                    egui::pos2(f.right() - e, f.top() + 1.0),
                ],
                "s" => [
                    egui::pos2(f.left() + e, f.bottom() - 1.0),
                    egui::pos2(f.right() - e, f.bottom() - 1.0),
                ],
                "w" => [
                    egui::pos2(f.left() + 1.0, f.top() + e),
                    egui::pos2(f.left() + 1.0, f.bottom() - e),
                ],
                _ => [
                    egui::pos2(f.right() - 1.0, f.top() + e),
                    egui::pos2(f.right() - 1.0, f.bottom() - e),
                ],
            };
            // Zurückhaltend: dünner und halbtransparent statt voller Akzentfarbe.
            painter.line_segment(
                strich,
                egui::Stroke::new(1.5, Palette::accent().gamma_multiply(0.55)),
            );
        }
        let griff = Rect::from_min_size(
            egui::pos2(f.right() - FENSTERECKE_PX, f.bottom() - FENSTERECKE_PX),
            Vec2::splat(FENSTERECKE_PX),
        );
        for k in 0..3 {
            let v = 4.0 + k as f32 * 5.0;
            painter.line_segment(
                [
                    egui::pos2(griff.right() - 3.0 - v, griff.bottom() - 3.0),
                    egui::pos2(griff.right() - 3.0, griff.bottom() - 3.0 - v),
                ],
                egui::Stroke::new(1.2, Palette::text_faint()),
            );
        }
        self.ui.last_fenster_griff = Some(griff);
        self.ui.last_fensterkanten = Some(lagen);
    }

    /// Zeichnet die Ribbon-Leiste in einen **festen** Inhaltsbereich.
    ///
    /// Ohne diesen festen Rahmen bestimmte der Inhalt die Panelhöhe: der Reiter
    /// „Ansicht" (vier Fangzeilen) war höher als `RIBBON_H`, der Rahmen wuchs
    /// darüber hinaus, die folgenden Panels rutschten nach unten und der
    /// ungezeichnete Streifen darunter erschien als schwarzer Balken
    /// (t_92b03440). Jetzt ist der Bereich `RIBBON_CONTENT_H` hoch, wird
    /// beschnitten und der Gruppenblock (`RIBBON_GROUP_H`) darin zentriert.
    fn ribbon(&mut self, ui: &mut egui::Ui) {
        let area = ui.available_rect_before_wrap();
        let dy = ((RIBBON_CONTENT_H - RIBBON_GROUP_H) * 0.5).max(0.0);
        let block = Rect::from_min_size(
            Pos2::new(area.left(), area.top() + dy),
            Vec2::new(area.width(), RIBBON_CONTENT_H - dy),
        );
        let mut content = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(block)
                .layout(Layout::top_down(Align::Min)),
        );
        content.set_clip_rect(area);
        self.ribbon_content(&mut content);
        // Für den Regressionstest: tatsächliche Inhaltshöhe dieses Reiters.
        self.ui.last_ribbon_content_h = content.min_rect().height();
        self.ui.last_ribbon_content_w = content.min_rect().width();
    }

    fn ribbon_content(&mut self, ui: &mut egui::Ui) {
        let tab = self.ui.ribbon;
        ui.horizontal(|ui| {
            // Enger als der egui-Standard: der Reiter „Ansicht" hat fünf
            // Gruppen und muss in die Mindestfensterbreite 1080 px passen
            // (Test `reiter_ansicht_passt_in_die_mindestbreite`).
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.add_space(8.0);
            match tab {
                RibbonTab::Datei => {
                    self.group(ui, "ribbon.group.zeichnung", |ui, app| {
                        if widgets::ribbon_button(ui, "new", rb("ribbon.new"), false).clicked() {
                            app.run_action(ui.ctx(), "file.new");
                        }
                        if widgets::ribbon_button(ui, "open", rb("ribbon.open"), false).clicked() {
                            app.run_action(ui.ctx(), "file.open");
                        }
                        if widgets::ribbon_button(ui, "save", rb("ribbon.save"), false).clicked() {
                            app.run_action(ui.ctx(), "file.save");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.ausgabe", |ui, app| {
                        if widgets::ribbon_button(ui, "pdf", rb("ribbon.pdf"), false).clicked() {
                            app.run_action(ui.ctx(), "file.export_pdf");
                        }
                        if widgets::ribbon_button(ui, "dxf", rb("ribbon.dxf"), false).clicked() {
                            app.run_action(ui.ctx(), "file.export_dxf");
                        }
                        // T0235: **ein** Export-Weg — der Knopf öffnet das
                        // Fenster mit Bereich, Auflösung und Hintergrund.
                        if widgets::ribbon_button(ui, "image", rb("ribbon.png"), false).clicked() {
                            app.run_action(ui.ctx(), "file.export_png");
                        }
                        if widgets::ribbon_button(ui, "import", rb("ribbon.import"), false).clicked() {
                            app.run_action(ui.ctx(), "file.import_dxf");
                        }
                        if widgets::ribbon_button(ui, "pdf", rb("ribbon.pdf_insert"), false).clicked() {
                            app.run_action(ui.ctx(), "file.import.pdf");
                        }
                        if widgets::ribbon_button(ui, "image", rb("ribbon.bild_einfuegen"), false).clicked() {
                            app.run_action(ui.ctx(), "file.import_image");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.zwischenablage", |ui, app| {
                        // Kopieren ist eine geteilte Schaltfläche: Hauptfläche
                        // führt den zuletzt gewählten Modus aus, der Pfeil
                        // öffnet die Auswahl (Zwischenablage / Basispunkt).
                        app.copy_group_button(ui);
                        if widgets::ribbon_button(ui, "import", rb("ribbon.paste"), false).clicked() {
                            app.paste_clipboard();
                        }
                        if widgets::ribbon_button(ui, "erase", rb("ribbon.cut"), false).clicked() {
                            app.copy_selection(true);
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.projekt", |ui, app| {
                        if widgets::ribbon_button(ui, "info", rb("ribbon.project"), false).clicked() {
                            app.ui.project_dialog = true;
                        }
                        if widgets::ribbon_button(ui, "settings", rb("ribbon.settings"), false).clicked() {
                            app.ui.settings_open = true;
                        }
                        // T0127: alle Nutzdaten liegen in einer Wurzel — hier
                        // landet der Dateimanager darin.
                        if widgets::ribbon_button(ui, "datei", rb("ribbon.datenordner"), false)
                            .on_hover_text(jjcad_core::daten::pfad_text(&jjcad_core::daten::wurzel()))
                            .clicked()
                        {
                            app.datenordner_oeffnen();
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.ansicht", |ui, app| {
                        if widgets::ribbon_button(ui, "inventory", rb("ribbon.inventory"), app.ui.inventory_open).clicked() {
                            app.run_action(ui.ctx(), "view.inventory");
                        }
                        if widgets::ribbon_button(ui, "keyboard", rb("ribbon.keys"), app.ui.keymap_open).clicked() {
                            app.ui.keymap_open = !app.ui.keymap_open;
                        }
                        if widgets::ribbon_button(ui, "hotbar", rb("ribbon.hotbar"), false).clicked() {
                            app.run_action(ui.ctx(), "view.hotbar_next");
                        }
                    });
                }
                RibbonTab::Zeichnen => {
                    // Vierzehn Schaltflächen in vier Gruppen — mit dem
                    // Standardabstand von 5 px braucht der Reiter 1128 px und
                    // schnitte bei der Mindestfensterbreite (1080 px) rechts ab
                    // (T0200). Enger gestellt misst er wieder unter 1080.
                    ui.spacing_mut().item_spacing.x = 2.0;
                    self.group(ui, "ribbon.group.grundelemente", |ui, app| {
                        for (id, icon) in [
                            ("draw.line", "line"),
                            ("draw.polyline", "polyline"),
                            ("draw.parallel", "parallel"),
                            ("draw.spline", "spline"),
                            ("draw.rect", "rect"),
                            ("draw.circle", "circle"),
                            ("draw.arc", "arc"),
                        ] {
                            let label = aktion_label(id);
                            // Polylinie und Pfad teilen sich einen Platz: der
                            // Pfeil öffnet die Wahl, der Knopf startet den
                            // zuletzt benutzten (T0229). So bleibt der Reiter
                            // in der Mindestbreite.
                            if id == "draw.polyline" {
                                app.polyline_group_button(ui);
                                continue;
                            }
                            // Rechteck trägt zwei Einfügemodi — der Pfeil am
                            // Knopf öffnet sie (Über Ecken / Basislinie + Höhe).
                            if id == "draw.rect" {
                                app.einfuege_group_button(ui, "draw.rect");
                                continue;
                            }
                            // Der Bogen trägt zwei Eingabemodi (drei Punkte /
                            // Mittelpunkt, Start, Ende) — der Pfeil öffnet sie.
                            if id == "draw.arc" {
                                app.bogen_group_button(ui);
                                continue;
                            }
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.fuellen", |ui, app| {
                        // Zwei geteilte Schaltflächen: Fläche und Schraffur.
                        // Der Pfeil öffnet die Eingabemodi, der Knopf startet
                        // den zuletzt benutzten — das Icon zeigt ihn an.
                        app.fill_group_button(ui, FillKind::Area);
                        app.fill_group_button(ui, FillKind::Hatch);
                        // Flächen bearbeiten (T0090): Vereinigen wirkt auf die
                        // Auswahl, Schneiden fragt eine Punktfolge ab.
                        if widgets::ribbon_button(ui, "union", rb("ribbon.union"), false)
                            .on_hover_text(
                                jjcad_core::i18n::tr("gui.app.zwei_oder_mehr_angrenzende_fuellflaechen_zu_einer_verein"),
                            )
                            .clicked()
                        {
                            app.run_action(ui.ctx(), "modify.area.union");
                        }
                        if widgets::ribbon_button(
                            ui,
                            "split",
                            rb("ribbon.split"),
                            app.tool_id() == Some("modify.area.split"),
                        )
                        .on_hover_text(jjcad_core::i18n::tr("gui.app.flaeche_entlang_einer_punktfolge_in_zwei_teile_schneiden"))
                        .clicked()
                        {
                            app.start_tool("modify.area.split");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.weitere", |ui, app| {
                        for (id, icon) in [
                            ("draw.ellipse", "ellipse"),
                            ("draw.point", "point"),
                            ("draw.text", "text"),
                        ] {
                            let label = aktion_label(id);
                            // Ellipse ebenfalls mit zwei Einfügemodi (Über
                            // Mittelpunkt / Mittelachse + Breite).
                            if id == "draw.ellipse" {
                                app.einfuege_group_button(ui, "draw.ellipse");
                                continue;
                            }
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                }
                RibbonTab::Architektur => {
                    self.group(ui, "ribbon.group.bauteile", |ui, app| {
                        for (id, icon) in [
                            ("draw.wall", "wall"),
                            ("draw.door", "door"),
                            ("draw.window", "window"),
                            ("draw.stair", "stair"),
                        ] {
                            let label = aktion_label(id);
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                        // Volumen mit denselben drei Einfüge-Modi wie die Fläche
                        app.fill_group_button(ui, FillKind::Volume);
                    });
                }
                RibbonTab::Aendern => {
                    // Mit „Abrunden" und „Phase" (T0247) trägt die Gruppe
                    // „Geometrie" neun Schaltflächen; der engere Abstand hält
                    // den Reiter bei der Standardbreite 1500 px zusammen.
                    ui.spacing_mut().item_spacing.x = 2.0;
                    self.group(ui, "ribbon.group.bearbeiten", |ui, app| {
                        for (id, icon) in RIBBON_BEARBEITEN {
                            let label = aktion_label(id);
                            // Kopieren ist eine geteilte Schaltfläche (zwei
                            // Kopierfunktionen) — Platz und Reihenfolge bleiben
                            // wie in der Liste vorgegeben.
                            if id == "modify.copy" {
                                app.copy_group_button(ui);
                                continue;
                            }
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.geometrie", |ui, app| {
                        for (id, icon) in [
                            ("modify.offset", "offset"),
                            ("modify.trim", "trim"),
                            ("modify.extend", "extend"),
                            ("modify.intersect", "intersect"),
                            // Ecke bearbeiten (T0247): Abrundung mit Bogen,
                            // Phase mit gerader Fase.
                            ("modify.fillet", "fillet"),
                            ("modify.chamfer", "chamfer"),
                            ("modify.explode", "explode"),
                            ("modify.erase", "erase"),
                            ("path.connect", "polyline"),
                        ] {
                            let label = aktion_label(id);
                            // Verbinden ist kein Klickwerkzeug: es arbeitet
                            // sofort auf der Auswahl (T0229).
                            if id == "path.connect" {
                                if widgets::ribbon_button(ui, icon, label, false)
                                    .on_hover_text(jjcad_core::i18n::tr(actions::action(id).map(|a| a.hint()).unwrap_or("")))
                                    .clicked()
                                {
                                    app.run_action(ui.ctx(), id);
                                }
                                continue;
                            }
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    // Zeichenreihenfolge: dieselben vier Funktionen wie im
                    // Objekt-Kontextmenü, hier für die aktuelle Auswahl.
                    self.group(ui, "ribbon.group.reihenfolge", |ui, app| {
                        for (icon, label, tip, front) in [
                            (
                                "order-front",
                                jjcad_core::i18n::tr("gui.app.vordergrund"),
                                jjcad_core::i18n::tr("gui.app.auswahl_ganz_nach_vorn"),
                                true,
                            ),
                            (
                                "order-back",
                                jjcad_core::i18n::tr("gui.app.hintergrund"),
                                jjcad_core::i18n::tr("gui.app.auswahl_ganz_nach_hinten"),
                                false,
                            ),
                        ] {
                            if widgets::ribbon_button(ui, icon, label, false)
                                .on_hover_text(tip)
                                .clicked()
                            {
                                app.order_selection_to(front);
                            }
                        }
                        for (icon, label, tip, above) in [
                            (
                                "order-above",
                                jjcad_core::i18n::tr("gui.app.vor_element"),
                                jjcad_core::i18n::tr("gui.app.auswahl_vor_ein_element"),
                                true,
                            ),
                            (
                                "order-below",
                                jjcad_core::i18n::tr("gui.app.hinter_element"),
                                jjcad_core::i18n::tr("gui.app.auswahl_hinter_ein_element"),
                                false,
                            ),
                        ] {
                            let active = app.ui.order_pick == Some(above);
                            if widgets::ribbon_button(ui, icon, label, active)
                                .on_hover_text(tip)
                                .clicked()
                            {
                                app.order_pick_start(above);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.sonstiges", |ui, app| {
                        for (id, icon) in [
                            ("modify.quickselect", "quickselect"),
                            ("edit.undo", "undo"),
                            ("edit.redo", "redo"),
                        ] {
                            let label = aktion_label(id);
                            let aktiv = app.tool_id() == Some(id);
                            if widgets::ribbon_button(ui, icon, label, aktiv).clicked() {
                                app.run_action(ui.ctx(), id);
                            }
                        }
                    });
                }
                RibbonTab::Bemassung => {
                    self.group(ui, "ribbon.group.masse", |ui, app| {
                        // **Ein** Element für jede Bemaßung (T0154): je
                        // Eingabemodus eine eigene Schaltfläche mit eigenem
                        // Icon — kein Dropdown (Turn 3).
                        for (id, icon) in [
                            ("dim.chain.h", "dimchain"),
                            ("dim.chain.v", "dim-v"),
                            ("dim.chain.d", "dim-d"),
                        ] {
                            let label = aktion_label(id);
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id))
                                .on_hover_text(actions::action(id).map(|a| a.hint()).unwrap_or(""))
                                .clicked()
                            {
                                app.waehle_dim_modus(
                                    DimModus::aus_werkzeug(id).unwrap_or(DimModus::Direkt),
                                );
                            }
                        }
                        for (id, icon) in [
                            ("dim.radius", "radius"),
                            ("dim.elevation.plan", "elevation"),
                            ("dim.elevation.section", "elevation-section"),
                            ("dim.room", "room"),
                        ] {
                            let label = aktion_label(id);
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    // Messpunkte einer bestehenden Maßkette ändern (T0154):
                    // Punkt anklicken — er kommt in die Kette bzw. geht heraus.
                    self.group(ui, "ribbon.group.massketten", |ui, app| {
                        for (id, icon) in [
                            ("dim.height", "dim-height"),
                            ("dim.point.add", "point"),
                            ("dim.point.remove", "erase"),
                        ] {
                            let label = aktion_label(id);
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                        // Maßeinheit, Textgröße, Maßstab und Bogensymbole stehen
                        // ausschließlich im Einstellungen-Dialog (Abschnitt
                        // „Bemaßung") — eine Stelle für jeden Wert.
                        if widgets::ribbon_button(ui, "settings", rb("ribbon.settings"), false).clicked() {
                            app.ui.settings_open = true;
                        }
                    });
                }
                RibbonTab::Bloecke => {
                    self.group(ui, "ribbon.group.bibliothek", |ui, app| {
                        if widgets::ribbon_button(ui, "library", rb("ribbon.block_manager"), app.ui.inventory_open && app.ui.inventory_tab == 1).clicked() {
                            app.ui.inventory_open = true;
                            app.ui.inventory_tab = 1;
                        }
                        if widgets::ribbon_button(ui, "block", rb("ribbon.block_insert"), app.tool_id() == Some("block.insert")).clicked() {
                            app.start_tool("block.insert");
                        }
                        if widgets::ribbon_button(ui, "blocknew", rb("ribbon.block_from_selection"), false).clicked() {
                            app.run_action(ui.ctx(), "block.create");
                        }
                        if widgets::ribbon_button(ui, "explode", rb("ribbon.explode"), false).clicked() {
                            app.run_action(ui.ctx(), "modify.explode");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.schnellzugriff", |ui, app| {
                        let blocks: Vec<(String, String)> = app
                            .doc()
                            .blocks
                            .iter()
                            .take(5)
                            .map(|b| (b.id.clone(), b.name.clone()))
                            .collect();
                        for (id, name) in blocks {
                            let resp = widgets::ribbon_button(ui, "block", &name, app.options.block_id.as_deref() == Some(id.as_str()));
                            if resp.clicked() {
                                app.options.block_id = Some(id.clone());
                                app.start_tool("block.insert");
                            }
                        }
                    });
                }
                RibbonTab::Blatt => {
                    self.group(ui, "ribbon.group.blaetter", |ui, app| {
                        if widgets::ribbon_button(ui, "sheet", rb("ribbon.sheet_new"), false).clicked() {
                            app.run_action(ui.ctx(), "sheet.new");
                        }
                        if widgets::ribbon_button(ui, "viewport", rb("ribbon.viewport"), app.tool_id() == Some("sheet.viewport_tool")).clicked() {
                            if matches!(&app.tab().tab, TabKind::Sheet(_)) {
                                app.start_tool("sheet.viewport_tool");
                                app.toast(jjcad_core::i18n::tr("gui.app.ansichtsfenster_rechteck_aufziehen"));
                            } else {
                                app.toast(jjcad_core::i18n::tr("gui.app.blatt_zuerst_ein_blatt_oeffnen"));
                            }
                        }
                        if widgets::ribbon_button(ui, "pdf", rb("ribbon.sheet_pdf"), false).clicked() {
                            app.run_action(ui.ctx(), "sheet.pdf");
                        }
                        // T0218: Blattmanager ein-/ausblenden.
                        if widgets::ribbon_button(ui, "sheet", rb("ribbon.blattmanager"), app.ui.blatt_open && !app.ui.blatt_collapsed).clicked() {
                            // T0218: Der Knopf zeigt den Manager auch dann,
                            // wenn er auf die Leiste eingeklappt ist.
                            if app.ui.blatt_open && !app.ui.blatt_collapsed {
                                app.ui.blatt_open = false;
                            } else {
                                app.ui.blatt_open = true;
                                app.ui.blatt_collapsed = false;
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    // Papierraum: dieselben Zeichenwerkzeuge wie im Modell.
                    // Der Plankopf besteht aus normalen Linien und Textfeldern.
                    self.group(ui, "ribbon.group.papierraum", |ui, app| {
                        if matches!(&app.tab().tab, TabKind::Sheet(_)) {
                            for (icon, action) in [
                                ("line", "draw.line"),
                                ("rect", "draw.rect"),
                                ("text", "draw.text"),
                            ] {
                                let label = aktion_label(action);
                                if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(action)).clicked() {
                                    app.start_tool(action);
                                }
                            }
                        } else {
                            ui.label(theme::dim(jjcad_core::i18n::tr("gui.app.blatt_oeffnen_dann_zeichnen")));
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.blatteinstellungen", |ui, app| {
                        if let TabKind::Sheet(id) = app.tab().tab.clone() {
                            let mut paper = app.doc().sheet(id).map(|s| s.paper).unwrap_or_default();
                            let mut landscape = app.doc().sheet(id).map(|s| s.landscape).unwrap_or(true);
                            let mut scale = app.doc().sheet(id).map(|s| s.scale).unwrap_or(100.0);
                            ui.vertical(|ui| {
                                // Zwei Zeilen à 24 px (Abstand 6) = 54 px — wie eine
                                // Knopfreihe, damit der Reiter dieselbe Höhe hat.
                                ui.horizontal(|ui| {
                                    ui.label(theme::dim(jjcad_core::i18n::tr("gui.app.format")));
                                    egui::ComboBox::from_id_salt("paper")
                                        .width(64.0)
                                        .selected_text(paper.label())
                                        .show_ui(ui, |ui| {
                                            for p in jjcad_core::style::PaperSize::ALL {
                                                ui.selectable_value(&mut paper, p, p.label());
                                            }
                                        });
                                    if ui.checkbox(&mut landscape, jjcad_core::i18n::tr("gui.app.quer")).changed() {
                                        let _ = app.session_mut().execute(
                                            "sheet.update",
                                            &serde_json::json!({"sheet": id, "paper": paper.label(), "landscape": landscape, "scale": scale}),
                                        );
                                        app.mark_changed();
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label(theme::dim(jjcad_core::i18n::tr("gui.app.massstab")));
                                    if ui.add_sized([54.0, 24.0], crate::widgets::zahl(&mut scale, 1.0, 1.0..=1000.0)).changed() {
                                        let _ = app.session_mut().execute(
                                            "sheet.update",
                                            &serde_json::json!({"sheet": id, "scale": scale}),
                                        );
                                        app.mark_changed();
                                    }
                                });
                            });
                        } else {
                            ui.add_space(8.0);
                            ui.label(theme::dim(jjcad_core::i18n::tr("gui.app.kein_blatt_aktiv_oben_auf_einen_blatt_reiter_wechseln")));
                        }
                    });
                }
                RibbonTab::Ansicht => {
                    self.group(ui, "ribbon.group.zoom", |ui, app| {
                        if widgets::ribbon_button(ui, "zoomfit", rb("ribbon.zoom_all"), false).clicked() {
                            app.fit_view();
                        }
                        if widgets::ribbon_button(ui, "zoomin", rb("ribbon.zoom_in"), false).clicked() {
                            app.zoom_by(1.25);
                        }
                        if widgets::ribbon_button(ui, "zoomout", rb("ribbon.zoom_out"), false).clicked() {
                            app.zoom_by(0.8);
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.drehen", |ui, app| {
                        // Bleibt gedrückt, solange die Ansicht gedreht ist; ein
                        // zweiter Klick stellt die Ausgangsorientierung her.
                        let gedreht = app.current_view().ist_gedreht();
                        let laeuft = app.tool_id() == Some("view.rotate_tool");
                        if widgets::ribbon_button(ui, "rotate", rb("ribbon.rotate"), gedreht || laeuft).clicked() {
                            if gedreht {
                                let mut v = app.current_view();
                                v.zurueck_drehen();
                                app.set_view(v);
                                app.tool = None;
                                app.toast(jjcad_core::i18n::tr("gui.app.ansicht_ausgangslage"));
                            } else if laeuft {
                                app.tool = None;
                                app.toast(jjcad_core::i18n::tr("gui.app.ansicht_drehen_abgebrochen"));
                            } else {
                                app.start_tool("view.rotate_tool");
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.panels", |ui, app| {
                        if widgets::ribbon_button(ui, "layers", rb("ribbon.layers"), app.ui.layers_open).clicked() {
                            app.ui.layers_open = !app.ui.layers_open;
                        }
                        if widgets::ribbon_button(ui, "properties", rb("ribbon.properties"), app.ui.props_open).clicked() {
                            app.ui.props_open = !app.ui.props_open;
                        }
                        if widgets::ribbon_button(ui, "grid", rb("ribbon.grid"), app.ui.show_grid).clicked() {
                            app.ui.show_grid = !app.ui.show_grid;
                            let v = app.ui.show_grid;
                            let _ = app.session_mut().execute(
                                "doc.settings",
                                &serde_json::json!({"show_grid": v, "snap": {"grid": v}}),
                            );
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.fenster", |ui, app| {
                        let voll = app.is_fullscreen(ui.ctx());
                        if widgets::ribbon_button(ui, "fullscreen", rb("ribbon.fullscreen"), voll).clicked() {
                            app.run_action(ui.ctx(), "view.fullscreen");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "ribbon.group.fangen", |ui, app| {
                        ui.vertical(|ui| {
                            // Zwei Zeilen à vier Felder: die Gruppe bleibt damit
                            // genau so hoch wie eine Knopfreihe (54 px), sodass
                            // alle Reiter dieselbe Ribbon-Höhe haben (t_92b03440).
                            ui.spacing_mut().item_spacing.y = 6.0;
                            let mut snap = app.doc().settings.snap.clone();
                            let mut changed = false;
                            ui.horizontal(|ui| {
                                changed |= widgets::check_cell(ui, &mut snap.endpoint, jjcad_core::i18n::tr("gui.app.endpunkt")).changed();
                                changed |= widgets::check_cell(ui, &mut snap.intersection, jjcad_core::i18n::tr("gui.app.schnitt")).changed();
                                changed |= widgets::check_cell(ui, &mut snap.midpoint, jjcad_core::i18n::tr("gui.app.mitte")).changed();
                                changed |= widgets::check_cell(ui, &mut snap.center, jjcad_core::i18n::tr("gui.app.zentrum")).changed();
                            });
                            ui.horizontal(|ui| {
                                changed |= widgets::check_cell(ui, &mut snap.grid, jjcad_core::i18n::tr("gui.app.fangraster")).changed();
                                changed |= widgets::check_cell(ui, &mut snap.ortho, jjcad_core::i18n::tr("gui.app.ortho")).changed();
                                changed |= widgets::check_cell(ui, &mut snap.wall_edges, jjcad_core::i18n::tr("gui.app.wandkanten")).changed();
                            });
                            if changed {
                                let _ = app.session_mut().execute(
                                    "doc.settings",
                                    &serde_json::json!({"snap": {
                                        "endpoint": snap.endpoint, "midpoint": snap.midpoint,
                                        "intersection": snap.intersection, "center": snap.center,
                                        "grid": snap.grid, "ortho": snap.ortho, "polar": snap.polar,
                                        "wall_edges": snap.wall_edges
                                    }}),
                                );
                                app.prefs.snap_wall_edges = snap.wall_edges;
                                let _ = app.prefs.save();
                            }
                        });
                    });
                }
            }
        });
    }

    /// Eine Gruppe im Menüband. `key` ist ein Katalogschlüssel (T0103) — die
    /// Überschrift folgt damit der Sprache, ohne dass hier Text steht.
    fn group(&mut self, ui: &mut egui::Ui, key: &'static str, body: impl FnOnce(&mut egui::Ui, &mut App)) {
        let label = rb(key);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                body(ui, self);
                ui.add_space(2.0);
            });
            let used_w = ui.min_rect().width();
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(used_w, 11.0),
                egui::Sense::hover(),
            );
            if ui.is_rect_visible(rect) {
                let galley = ui.painter().layout_no_wrap(
                    label.to_uppercase(),
                    FontId::proportional(9.0),
                    Palette::text_dim(),
                );
                let w = galley.size().x;
                let h = galley.size().y;
                ui.painter().galley(
                    Pos2::new(rect.center().x - w * 0.5, rect.center().y - h * 0.5),
                    galley,
                    Palette::text_dim(),
                );
            }
        });
    }


    // ---- Vorschau vor dem Platzieren -------------------------------------

    /// Startet die Vorschau für einen Block aus der Bibliothek.
    pub fn start_ghost_block(&mut self, block: String) {
        // T0180: wie beim Einfügen aus der Zwischenablage — eine neue
        // Zeichenfunktion schließt das offene Kontextmenü.
        self.ui.menu = None;
        let pos = self
            .pointer_world
            .or_else(|| self.pointer_world_fallback())
            .unwrap_or_default();
        self.ghost = Some(crate::ghost::Ghost::new(
            Some(block),
            pos,
            self.prefs.paste_pivot,
        ));
        self.tool = None;
        self.ui.inventory_open = false;
        self.release_keyboard_focus();
    }

    /// Läuft gerade das Einfügen eines **Blocks** aus der Bibliothek? (T0116)
    ///
    /// Dann ist der nächste Klick auf einen Slot eine Verknüpfung und kein
    /// Start: der Slot bekommt den Block, genau wie ein laufendes Werkzeug
    /// einen leeren Slot belegt. Eine laufende **Zwischenablage**-Vorschau
    /// liefert hier noch keinen Block; für sie entsteht er erst beim Klick
    /// (`App::block_fuer_verknuepfung`, T0142).
    pub fn block_einfuegen(&self) -> Option<&str> {
        self.ghost.as_ref().and_then(|g| g.block.as_deref())
    }

    /// Name eines Blocks für Meldungen und Tooltips — die ID, wenn die
    /// Bibliothek ihn nicht (mehr) kennt.
    pub fn block_name(&self, id: &str) -> String {
        self.doc()
            .blocks
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// Verknüpft den laufenden Block mit einem Hotbar-Slot (T0116).
    ///
    /// Der eine Weg für Hotbar und Lager: Slot belegen, Einfügen beenden,
    /// Meldung im Chat-Schema (`<Block> auf Slot N gelegt`). Liefert `false`,
    /// wenn gerade kein Block eingefügt wird — dann bleibt der Klick beim
    /// bisherigen Verhalten.
    pub fn bind_slot_block(&mut self, slot: usize) -> bool {
        let Some(block) = self.block_fuer_verknuepfung() else {
            return false;
        };
        let name = self.block_name(&block);
        self.prefs.set_slot(slot, Some(format!("block:{block}")));
        self.speichere_prefs();
        self.ghost = None;
        self.toast(jjcad_core::i18n::t_args_owned(
            "gui.hotbar.x_auf_slot_x_gelegt",
            &[("wert1", name), ("wert2", format!("{}", slot + 1))],
        ));
        true
    }

    /// Legt den laufenden Block auf einen Lagerplatz (T0116) — dieselbe Geste
    /// wie in der Hotbar, nur ohne Tastenkürzel.
    pub fn bind_lager_block(&mut self, idx: usize) -> bool {
        let Some(block) = self.block_fuer_verknuepfung() else {
            return false;
        };
        if idx >= self.prefs.inventory.len() {
            return false;
        }
        let name = self.block_name(&block);
        self.prefs.inventory[idx] = Some(format!("block:{block}"));
        self.speichere_prefs();
        self.ghost = None;
        self.toast(jjcad_core::i18n::t_args_owned(
            "gui.inventory.x_ins_lager_gelegt",
            &[("wert1", name)],
        ));
        true
    }

    /// Verknüpft den laufenden Block mit einer Taste der Tastaturbelegung
    /// (T0116) — dieselbe Geste wie Hotbar-Slot und Lagerplatz.
    ///
    /// Gesperrte Tasten (Ziffern, `Esc`, `F1` …) lehnt `UserPrefs::bind` ab;
    /// das Tastenbild nimmt sie ohnehin nicht an.
    pub fn bind_key_block(&mut self, key: &str) -> bool {
        let Some(block) = self.block_fuer_verknuepfung() else {
            return false;
        };
        let name = self.block_name(&block);
        self.prefs.bind(&format!("block:{block}"), key);
        self.speichere_prefs();
        self.ghost = None;
        self.ui.keymap_selected = Some(key.to_lowercase());
        self.toast(jjcad_core::i18n::t_args_owned(
            "gui.dialogs.x_taste_x",
            &[("name", name), ("wert1", key.to_uppercase())],
        ));
        true
    }

    /// Block, der beim Klick auf einen Slot/Taste verknüpft wird (T0116/T0142).
    ///
    /// Läuft die Vorschau eines **Blocks**, ist das dieser Block. Läuft die
    /// Vorschau der **Zwischenablage**, entsteht daraus jetzt ein Block
    /// (T0142) — der Klick auf den Slot ist die Geste „daraus einen Block
    /// machen und ihn ablegen“. Ohne laufendes Einfügen gibt es nichts zu
    /// verknüpfen; dann bleibt der Klick beim bisherigen Verhalten.
    pub fn block_fuer_verknuepfung(&mut self) -> Option<String> {
        if let Some(block) = self.block_einfuegen() {
            return Some(block.to_string());
        }
        if self.ghost.is_some() {
            return self.block_aus_zwischenablage();
        }
        None
    }

    /// Speichert die Voreinstellungen — in Tests abschaltbar, damit ein Lauf
    /// nicht die `prefs.json` des Nutzers überschreibt.
    fn speichere_prefs(&self) {
        #[cfg(test)]
        if !self.test_prefs_speichern {
            return;
        }
        let _ = self.prefs.save();
    }

    /// Ordner, in dem ein direkt erzeugter Block als Datei landet (T0142).
    fn blockordner(&self) -> std::path::PathBuf {
        #[cfg(test)]
        if let Some(dir) = &self.test_blockordner {
            return dir.clone();
        }
        self.prefs.blocks_dir()
    }

    /// Erzeugt aus der Zwischenablage einen Block ohne Namensdialog (T0142).
    ///
    /// Der Name kommt aus `bloecke::freier_name` („Unbenannt 1“, „Unbenannt 2“, …)
    /// und ist damit kollisionsfrei und als „nicht benannt“ erkennbar. Der Block
    /// wird als `.jjblk` in der Benutzerbibliothek abgelegt und in `doc.blocks`
    /// aufgenommen; die Zwischenablage bleibt unverändert. Liefert die Kennung
    /// des neuen Blocks oder `None`, wenn nichts einzufügen war bzw. die Datei
    /// nicht geschrieben werden konnte.
    pub fn block_aus_zwischenablage(&mut self) -> Option<String> {
        let clip = self.ui.clipboard.clone();
        if clip.is_empty() {
            self.toast(jjcad_core::i18n::tr("gui.app.einfuegen_zwischenablage_leer"));
            return None;
        }
        // Basispunkt ist die untere linke Ecke der Auswahl — dieselbe Lage, die
        // ein normaler Einfügevorgang als Anker benutzt.
        let mut bb = BBox::EMPTY;
        for e in &clip.ents {
            bb.union(jjcad_core::render::entity_bbox(self.doc(), e));
        }
        let base = if bb.is_valid() { bb.min } else { V2::ZERO };
        let name = jjcad_core::bloecke::freier_name(
            self.doc().blocks.iter().map(|b| b.name.as_str()),
        );
        let block =
            jjcad_core::bloecke::definition_aus_elementen(&name, "Projekt", &clip.ents, base);
        let dir = self.blockordner();
        if let Err(e) = jjcad_core::bloecke::schreibe_block(&dir, &block) {
            self.toast(e);
            return None;
        }
        let id = block.id.clone();
        self.doc_mut().blocks.push(block);
        self.mark_changed();
        self.toast(jjcad_core::i18n::t_args_owned(
            "gui.dialogs.block_x_angelegt",
            &[("name", name)],
        ));
        Some(id)
    }

    /// „Einfügen als Block“ — Strg+Umschalt+V (T0142).
    ///
    /// Statt der losen Objekte der Zwischenablage entsteht ein Block, der
    /// sofort als Vorschau am Zeiger steht und mit einem Klick abgesetzt wird.
    /// Eine bereits laufende Vorschau behält Lage, Drehung und Spiegelung.
    pub fn einfuegen_als_block(&mut self) {
        if self.bild_einfuegen() {
            return;
        }
        if self.ui.clipboard.is_empty() {
            self.toast(jjcad_core::i18n::tr("gui.app.einfuegen_zwischenablage_leer"));
            return;
        }
        let Some(id) = self.block_aus_zwischenablage() else {
            return;
        };
        let (pos, rot, mirror, scale) = match &self.ghost {
            Some(g) => (g.pos, g.rot, g.mirror, g.scale),
            None => (
                self.pointer_world
                    .or_else(|| self.pointer_world_fallback())
                    .unwrap_or_default(),
                0.0,
                false,
                1.0,
            ),
        };
        self.ghost = Some(crate::ghost::Ghost {
            block: Some(id),
            pos,
            rot,
            mirror,
            scale,
            pivot: self.prefs.paste_pivot,
        });
        self.tool = None;
        self.session_mut().clear_selection();
        self.release_keyboard_focus();
    }

    /// Block zu einem Slot-Inhalt (`block:<id>`) — für die Miniaturaansicht
    /// im Slot, im Lager und im Tastenbild (T0116). `None` bei Werkzeugen und
    /// bei Blöcken, die die Bibliothek nicht mehr kennt.
    pub fn slot_block(&self, item: &str) -> Option<(&jjcad_core::model::Document, &jjcad_core::model::BlockDef)> {
        let id = item.strip_prefix("block:")?;
        let doc = self.doc();
        doc.blocks.iter().find(|b| b.id == id).map(|b| (doc, b))
    }

    /// Beschriftung eines Slot-Inhalts: Werkzeugname oder Blockname (T0116).
    ///
    /// Ein Slot, eine Taste und ein Lagerplatz tragen entweder eine Action-ID
    /// oder `block:<id>` — beides muss lesbar benannt sein, sonst stünde in
    /// Tooltip und Meldung nur die Kennung.
    pub fn slot_label(&self, item: &str) -> String {
        if let Some(id) = item.strip_prefix("block:") {
            return self.block_name(id);
        }
        jjcad_core::actions::action(item)
            .map(|d| d.label().to_string())
            .unwrap_or_else(|| jjcad_core::i18n::tr("gui.hotbar.werkzeug").to_string())
    }

    /// Startet die Vorschau für das Einfügen aus der Zwischenablage.
    ///
    /// Der Drehpunkt kommt aus den Einstellungen (T0070): „Basispunkt" (Vorgabe)
    /// hält den Basispunkt am Fadenkreuz, „Mittelpunkt" hält die Vorschau oben
    /// rechts davon.
    pub fn start_ghost_paste(&mut self) {
        if self.ui.clipboard.is_empty() {
            self.toast(jjcad_core::i18n::tr("gui.app.einfuegen_zwischenablage_leer"));
            return;
        }
        if self.ghost.is_some() {
            return;
        }
        // T0180: Das Einfügen ist ebenfalls eine Zeichenfunktion — das offene
        // Kontextmenü schließt dabei.
        self.ui.menu = None;
        let pos = self
            .pointer_world
            .or_else(|| self.pointer_world_fallback())
            .unwrap_or_default();
        self.ghost = Some(crate::ghost::Ghost::new(None, pos, self.prefs.paste_pivot));
        self.tool = None;
        self.session_mut().clear_selection();
        self.release_keyboard_focus();
    }

    /// Setzt den Drehpunkt der Einfüge-Vorschau (T0087).
    ///
    /// Die Einstellung wirkt **sofort**, auch auf eine laufende Vorschau: sonst
    /// bliebe ein im Einstellungsfenster umgestellter Modus unsichtbar, bis der
    /// Nutzer das Einfügen erneut startet — er hielte die Einstellung für wirkungslos.
    pub fn set_paste_pivot(&mut self, pivot: jjcad_core::persist::PastePivot) {
        self.prefs.paste_pivot = pivot;
        if let Some(g) = self.ghost.as_mut() {
            g.pivot = pivot;
        }
    }

    /// Schaltet „Maus verschiebt“ um (T0108) — derselbe Wert wie
    /// Einstellungen → Bedienung → „Maus verschiebt“ (`prefs.mouse_move`).
    ///
    /// Der Schalter in der unteren Leiste ist nur ein zweiter Zugang zur
    /// Einstellung; gespeichert wird sie wie dort über `prefs.save()`.
    pub fn set_mouse_move(&mut self, an: bool) {
        self.prefs.mouse_move = an;
    }

    /// Schaltet den Drehpunkt der Einfüge-Vorschau um (T0108):
    /// Basispunkt ⇄ Mittelpunkt. Wirkt sofort, auch auf eine laufende
    /// Vorschau — dafür sorgt [`App::set_paste_pivot`].
    pub fn umschalte_paste_pivot(&mut self) {
        let neu = match self.prefs.paste_pivot {
            jjcad_core::persist::PastePivot::BasePoint => jjcad_core::persist::PastePivot::Center,
            jjcad_core::persist::PastePivot::Center => jjcad_core::persist::PastePivot::BasePoint,
        };
        self.set_paste_pivot(neu);
    }

    /// Design umstellen (Einstellungen → Darstellung → „Design“, T0102).
    ///
    /// Die Wahl wird gespeichert und gilt ab dem nächsten Bild. Die
    /// Anzeigelisten der Reiter werden verworfen: sie tragen zwar die
    /// Objektfarben, aber Papier und Hintergrund werden neu bestimmt — ein
    /// Neuaufbau ist billiger als eine Sonderbehandlung.
    pub fn set_design(&mut self, design: jjcad_core::persist::Design, ctx: &egui::Context) {
        theme::set_design(design, ctx);
        self.prefs.design = design;
        let _ = self.prefs.save();
        for t in &mut self.tabs {
            t.cache = None;
        }
        ctx.request_repaint();
    }

    /// Farbakzent umstellen (Einstellungen → Darstellung → „Farbakzente“,
    /// T0111).
    ///
    /// Die Wahl wird gespeichert und gilt ab dem nächsten Bild. Die
    /// Anzeigelisten der Reiter werden verworfen: die Zeichnung selbst bleibt
    /// zwar gleich, aber Rahmen und Auswahlflächen tragen den Akzent.
    pub fn set_akzent(
        &mut self,
        akzent: jjcad_core::persist::Farbakzent,
        ctx: &egui::Context,
    ) {
        theme::set_akzent(akzent, ctx);
        self.prefs.farbakzent = akzent;
        let _ = self.prefs.save();
        for t in &mut self.tabs {
            t.cache = None;
        }
        ctx.request_repaint();
    }

    /// Eigene Zeichenfarben umstellen (Einstellungen → Darstellung →
    /// „Zeichenfarben“, T0181).
    ///
    /// Die Wahl wird gespeichert und gilt ab dem nächsten Bild. Die
    /// Anzeigelisten der Reiter bleiben gültig: die Farben der Zeichnung
    /// stecken in den Objekten, die Rollenfarben werden erst beim Malen
    /// aufgesetzt.
    pub fn set_zeichenfarben(
        &mut self,
        farben: &std::collections::BTreeMap<jjcad_core::persist::Zeichenfarbe, egui::Color32>,
        ctx: &egui::Context,
    ) {
        theme::set_zeichenfarben(farben, ctx);
        self.prefs.zeichenfarben = farben
            .iter()
            .map(|(rolle, c)| {
                (
                    *rolle,
                    jjcad_core::style::Color::rgb(c.r(), c.g(), c.b()),
                )
            })
            .collect();
        let _ = self.prefs.save();
        ctx.request_repaint();
    }

    fn pointer_world_fallback(&self) -> Option<V2> {
        Some(self.current_view().center)
    }

    /// Einen einzelnen Fangmodus setzen (Statusleiste, Menüs).
    pub fn set_snap(&mut self, key: &str, value: bool) {
        let mut sn = self.doc().settings.snap.clone();
        match key {
            "endpoint" => sn.endpoint = value,
            "midpoint" => sn.midpoint = value,
            "center" => sn.center = value,
            "intersection" => sn.intersection = value,
            "perpendicular" => sn.perpendicular = value,
            "tangent" => sn.tangent = value,
            "lot" => sn.lot = value,
            "nearest" => sn.nearest = value,
            "grid" => sn.grid = value,
            "ortho" => sn.ortho = value,
            "polar" => sn.polar = value,
            "wall_edges" => sn.wall_edges = value,
            _ => return,
        }
        let _ = self.session_mut().execute(
            "doc.settings",
            &serde_json::json!({"snap": {
                "endpoint": sn.endpoint, "midpoint": sn.midpoint, "center": sn.center,
                "intersection": sn.intersection, "perpendicular": sn.perpendicular,
                "tangent": sn.tangent, "lot": sn.lot,
                "nearest": sn.nearest, "grid": sn.grid, "ortho": sn.ortho, "polar": sn.polar,
                "wall_edges": sn.wall_edges
            }}),
        );
        // Der Wandkontur-Fang ist eine app-weite Vorliebe — auch für neue
        // Zeichnungen und über einen Neustart hinweg merken.
        if key == "wall_edges" {
            self.prefs.snap_wall_edges = sn.wall_edges;
            let _ = self.prefs.save();
        }
    }

    fn release_keyboard_focus(&mut self) {
        let ctx = self.ctx.clone();
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
    }

    /// Drehwinkel für einen Tastendruck auf `R` in der Vorschau.
    ///
    /// `R` dreht im Uhrzeigersinn, `Umschalt+R` gegen den Uhrzeigersinn
    /// (T0006). Im Modell zeigt +y nach oben, ein positiver Winkel dreht
    /// deshalb mathematisch positiv = gegen den Uhrzeigersinn.
    pub fn ghost_drehgrad(shift: bool) -> f64 {
        if shift {
            90.0
        } else {
            -90.0
        }
    }

    pub fn rotate_ghost(&mut self, deg: f64) {
        if let Some(g) = &mut self.ghost {
            g.rotate(deg);
        }
    }

    pub fn flip_ghost(&mut self) {
        if let Some(g) = &mut self.ghost {
            g.flip();
        }
    }

    pub fn cancel_ghost(&mut self) {
        if self.ghost.take().is_some() {
            self.toast(jjcad_core::i18n::tr("gui.app.platzieren_abgebrochen"));
        }
    }

    /// Setzt die Vorschau an der aktuellen Lage ein.
    pub fn commit_ghost(&mut self) -> bool {
        let Some(g) = self.ghost.clone() else {
            return false;
        };
        if let Some(bid) = g.block.clone() {
            let geo = crate::ghost::geometry(self.doc(), &g, &crate::ghost::Clipboard::default());
            let o = geo.origin;
            let u = self.unit();
            let params = serde_json::json!({
                "block": bid,
                "pos": [u.from_mm(o.x), u.from_mm(o.y)],
                "rot": g.rot.to_degrees(),
                "mirror": g.mirror,
                "scale": g.scale,
            });
            // Ebene wie bei jedem Werkzeug (T0202): die typgerechte
            // Standardebene „Blöcke“, außer eine eigene Ebene ist gewählt.
            let mut params = params;
            let ebene = self
                .ui
                .pending
                .layer
                .filter(|l| self.doc().layer(*l).is_some())
                .or_else(|| self.werkzeug_ebene("block.insert"));
            if let Some(ebene) = ebene {
                params["layer"] = serde_json::json!(ebene);
            }
            return match self.session_mut().execute("block.insert", &params) {
                Ok(_) => {
                    self.mark_changed();
                    self.ghost = None;
                    true
                }
                Err(e) => {
                    self.toast(e);
                    false
                }
            };
        }

        let clip = self.ui.clipboard.clone();
        let ents = crate::ghost::clipboard_entities(self.doc(), &g, &clip);
        if ents.is_empty() {
            self.ghost = None;
            return false;
        }
        let snap = self.doc().clone();
        self.session_mut().history.record(&snap);
        let mut ids = Vec::new();
        let mut zuordnung: Vec<(Id, Id)> = Vec::new();
        for mut e in ents {
            let alt = e.id;
            e.id = self.doc_mut().new_id();
            zuordnung.push((alt, e.id));
            ids.push(e.id);
            // Eingefügte Elemente ordnen sich nach ihrem Zeichenrang ein
            // (`Document::insert_ranked`): Flächen hinter Linien und Wänden.
            self.doc_mut().insert_ranked(e);
        }
        // Die mitkopierten Öffnungen hängen an der **eingefügten** Wand, nicht
        // am Original (T0207): die Verweise auf die alten Ids werden auf die
        // neuen umgesetzt. Ohne das saß die Tür weiter in der Ursprungswand.
        for (_, neu) in &zuordnung {
            let Some(em) = self.doc_mut().entity_mut(*neu) else {
                continue;
            };
            let host = match &mut em.kind {
                EntityKind::Door { host, .. } | EntityKind::Window { host, .. } => host,
                _ => continue,
            };
            if let Some((_, neu_host)) = zuordnung.iter().find(|(alt, _)| Some(*alt) == *host) {
                *host = Some(*neu_host);
            }
        }
        // T0191: eine eingefügte Tür hängt an ihrer Wand — die Kopie liegt
        // danach wieder auf der Wandkante, nicht frei in der Zeichnung.
        self.session_mut().oeffnungen_auf_wand();
        self.session_mut().select(ids);
        self.mark_changed();
        self.ghost = None;
        self.toast(jjcad_core::i18n::tr("gui.app.auswahl_eingefuegt"));
        true
    }

    /// Startet einen Slot-Inhalt: Werkzeug oder Block (`block:<id>`, T0116).
    ///
    /// Der eine Weg für Klick und Zifferntaste — die Block-Vorschau steckt in
    /// [`App::run_action`], damit beide Eingaben nicht auseinanderlaufen.
    pub fn activate(&mut self, ctx: &egui::Context, item: &str) {
        self.run_action(ctx, item);
    }

    pub fn tool_id(&self) -> Option<&str> {
        self.tool.as_ref().map(|t| t.action.as_str())
    }

    // ---- Werkzeugleiste (Optionen des aktiven Werkzeugs) ----


    // ---- Rahmen ----

    pub fn ui(&mut self, ctx: &egui::Context) {
        // Leistungsanzeige (T0068): Abschnittsgrenze vor dem Aufbau der
        // Oberfläche. Der Wechsel gehört zum **vorigen** Abschnitt, deshalb
        // steht er hier und nicht nach der Zeichenfläche.
        self.perf.abschnitt(crate::perf::Abschnitt::Oberflaeche);
        // T0147: Fensterfläche und Schatten — ohne Titelleiste zeichnet
        // JoJa-CAD den Schatten selbst, der Rand bleibt durchsichtig. Steht
        // vor allen Panels, weil es den Bildschirmausschnitt verkleinert.
        self.fensterflaeche(ctx);
        // T0140: Fokusverlust heißt, das Betriebssystem schickt kein
        // Tastenloslassen mehr nach — der Systemdialog („Strg+S", „Strg+O")
        // nimmt den Fokus, egui führt die gedrückte Taste aber in `keys_down`
        // fort. Der Dauerlauf des Schwenks liefe dann endlos weiter, obwohl
        // niemand mehr drückt. Nach einem Fokusverlust gilt deshalb jede Taste
        // als losgelassen, bis sie neu gedrückt wird.
        if !ctx.input(|i| i.focused) {
            ctx.input_mut(|i| i.keys_down.clear());
        }
        // Offener Systemdialog ist modal: die Anwendung nimmt keine Eingaben an,
        // bis der Nutzer im Dialog wählt oder abbricht. Ohne das ließen sich
        // Zeichnung und Werkzeuge hinter dem Dialog weiterbedienen.
        if self.ui.systemdialog.is_some() {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                // Notausstieg, falls das Portal gar nicht antwortet
                self.ui.systemdialog = None;
            } else {
                ctx.input_mut(|i| i.events.clear());
            }
        }

        // T0141: Fenstergröße im Normalzustand mitschreiben — beim Beenden im
        // Vollbild oder maximiert wird sonst die Bildschirmgröße gespeichert.
        self.fenster_merken();
        // Und den gespeicherten Fensterzustand anwenden (nur im ersten Bild).
        self.fenster_start_anwenden(ctx);

        // Drag an Drop (T0115): im Fenster abgelegte Dateien gehen denselben
        // Weg wie eine Auswahl im Dateidialog.
        self.dateien_ablegen(ctx);

        // Bearbeitungsraum an den aktiven Reiter binden: Modell oder Papierraum
        // der Druckvorschau. Alle Befehle arbeiten danach im selben Raum.
        self.sync_space();
        self.handle_shortcuts(ctx);
        // T0098: Ohne Titelleiste ändern die Fensterkanten die Größe.
        self.fensterkanten(ctx);

        egui::TopBottomPanel::top("header")
            .exact_height(auf_pixel(ctx, HEADER_H))
            .frame(Frame::none().fill(Palette::ribbon()))
            .show(ctx, |ui| self.header(ui));

        if !self.ui.ribbon_collapsed {
            egui::TopBottomPanel::top("ribbon")
                .exact_height(auf_pixel(ctx, RIBBON_H))
                .frame(
                    Frame::none()
                        .fill(Palette::panel())
                        .inner_margin(Margin::symmetric(0.0, RIBBON_PAD)),
                )
                .show(ctx, |ui| self.ribbon(ui));
        }

        egui::TopBottomPanel::top("doctabs")
            .exact_height(auf_pixel(ctx, DOCTABS_H))
            .frame(Frame::none().fill(Palette::ribbon()))
            .show(ctx, |ui| self.doc_tabs(ui));

        egui::TopBottomPanel::bottom("hotbar")
            .exact_height(auf_pixel(ctx, HOTBAR_H))
            .frame(Frame::none().fill(Palette::panel()))
            .show(ctx, |ui| hotbar::show(self, ui));

        // T0101: Kontinuierliches Verschieben der Ansicht. Nach der Leiste —
        // deren Slots melden einen gehaltenen Druck — und vor der
        // Zeichenfläche, damit die Bewegung im selben Bild zu sehen ist.
        self.schwenk_fortschritt(ctx);

        // T0203/T0218: Alle drei Seitenpanels dürfen auf beiden Seiten
        // liegen. Die Leisten tragen deshalb stets **dieselben** Icons
        // (Layer, Eigenschaften, Blätter); ein Klick legt das Panel auf diese
        // Seite und klappt es aus.
        self.seitenleisten(ctx);

        egui::CentralPanel::default()
            .frame(Frame::none().fill(Palette::bg()))
            .show(ctx, |ui| {
                self.ui.last_canvas = ui.available_rect_before_wrap();
                canvas::show(self, ui);
            });

        // Dialoge, Menüs, Inventar und Chat sind der letzte Abschnitt eines
        // Bildes; `App::update` schließt ihn ab (`bild_beenden`).
        self.perf.abschnitt(crate::perf::Abschnitt::Fenster);
        dialogs::show(self, ctx);
        // Geschlossene Tastaturbelegung lässt keinen Merker zurück (T0064):
        // sonst legte ein Klick auf eine leere Taste eine längst vergessene
        // Funktion ab.
        if !self.ui.keymap_open {
            self.ui.keymap_funktion = None;
        }
        inventory::show(self, ctx);
        hotbar::auswahl_fenster(self, ctx);
        panels::layer_color_popup(self, ctx);
        // Chat-Protokoll über der Zeichnung (unten links), mit T geöffnet
        crate::chat::fenster(self, ctx);
        // Schwebende Dateien nennen ihr Ziel, bevor sie abgelegt werden (T0115).
        dialogs::datei_hinweis(self, ctx);
        // Systemdialog offen: Zeichnung abdunkeln und Hinweis zeigen
        dialogs::systemdialog_schleier(self, ctx);
        // Leistungsanzeige zuletzt: sie zeigt die **abgeschlossenen** Bilder und
        // gehört damit über alles andere.
        crate::perf::overlay(self, ctx);
    }

    // ---- Seitenpanels auf beiden Seiten (T0203) ----

    /// Ist das Panel über den Ribbon-Schalter geöffnet?
    pub fn panel_offen(&self, p: SeitenPanel) -> bool {
        match p {
            SeitenPanel::Layer => self.ui.layers_open,
            SeitenPanel::Eigenschaften => self.ui.props_open,
            SeitenPanel::Blatt => self.ui.blatt_open,
        }
    }

    /// Ist das Panel auf seine Leiste eingeklappt?
    pub fn panel_eingeklappt(&self, p: SeitenPanel) -> bool {
        match p {
            SeitenPanel::Layer => self.ui.layers_collapsed,
            SeitenPanel::Eigenschaften => self.ui.props_collapsed,
            SeitenPanel::Blatt => self.ui.blatt_collapsed,
        }
    }

    /// Auf welcher Seite liegt das Panel?
    pub fn panel_seite(&self, p: SeitenPanel) -> PanelSeite {
        match p {
            SeitenPanel::Layer => self.ui.layers_side,
            SeitenPanel::Eigenschaften => self.ui.props_side,
            SeitenPanel::Blatt => self.ui.blatt_side,
        }
    }

    /// Setzt Öffnung, Einklappung und Seite eines Panels in einem Zug.
    pub fn panel_setze(&mut self, p: SeitenPanel, offen: bool, eingeklappt: bool, seite: PanelSeite) {
        match p {
            SeitenPanel::Layer => {
                self.ui.layers_open = offen;
                self.ui.layers_collapsed = eingeklappt;
                self.ui.layers_side = seite;
            }
            SeitenPanel::Eigenschaften => {
                self.ui.props_open = offen;
                self.ui.props_collapsed = eingeklappt;
                self.ui.props_side = seite;
            }
            SeitenPanel::Blatt => {
                self.ui.blatt_open = offen;
                self.ui.blatt_collapsed = eingeklappt;
                self.ui.blatt_side = seite;
            }
        }
        // T0222: ausgeklappte Panels stehen hinten in der Benutzungsreihenfolge.
        self.ui.panel_benutzt.retain(|q| *q != p);
        if offen && !eingeklappt {
            self.ui.panel_benutzt.push(p);
        }
    }

    /// T0222: Höchstens zwei ausgeklappte Panels je Seite.
    ///
    /// Der Aufruf steht vor dem Zeichnen der Seitenpanels, damit **jeder** Weg
    /// die Grenze einhält: die Leiste, der Ribbon-Schalter `view.layers` und die
    /// Objektmenü-Einträge („Eigenschaften im Panel“) setzen die Felder an
    /// verschiedenen Stellen. Beim dritten Panel gibt das am längsten nicht
    /// mehr benutzte auf — es bleibt offen und liegt danach als Symbol in der
    /// Leiste, ist also mit einem Klick zurück.
    ///
    /// T0233: Ohne „Seitenleiste immer anzeigen“ (Programmstandard) ist **je
    /// Seite ein** ausgeklapptes Panel die Grenze — die übrigen derselben Seite
    /// geben auf. Beide Seiten behalten ihr Panel.
    pub fn panels_begrenzen(&mut self) {
        // T0233: Ein direkt geöffnetes Panel (Ribbon, Objektmenü, Demo) gilt
        // als zuletzt benutzt — sonst verdrängte die Grenze unten gerade das
        // eben geöffnete.
        let offen = [
            self.panel_offen(SeitenPanel::Layer),
            self.panel_offen(SeitenPanel::Eigenschaften),
            self.panel_offen(SeitenPanel::Blatt),
        ];
        for (i, p) in SeitenPanel::ALLE.iter().copied().enumerate() {
            if offen[i] && !self.ui.panel_offen_vorher[i] {
                self.ui.panel_benutzt.retain(|q| *q != p);
                self.ui.panel_benutzt.push(p);
            }
        }
        self.ui.panel_offen_vorher = offen;
        let mut reihe: Vec<SeitenPanel> = self.ui.panel_benutzt.clone();
        reihe.retain(|p| self.panel_offen(*p) && !self.panel_eingeklappt(*p));
        for p in SeitenPanel::ALLE {
            if self.panel_offen(p) && !self.panel_eingeklappt(p) && !reihe.contains(&p) {
                reihe.push(p);
            }
        }
        // T0233: „Seitenleiste immer anzeigen“ an → zwei Panels je Seite (der
        // Stand seit T0222), aus (Programmstandard) → eines je Seite. Über die
        // Seite hinweg gibt es keine Grenze: links und rechts dürfen gleichzeitig
        // je ein Panel stehen.
        let grenze = if self.seitenleiste_immer() { 2 } else { 1 };
        for seite in PanelSeite::ALL {
            let mut hier: Vec<SeitenPanel> = reihe
                .iter()
                .copied()
                .filter(|p| self.panel_seite(*p) == seite)
                .collect();
            while hier.len() > grenze {
                let alt = hier.remove(0);
                reihe.retain(|p| *p != alt);
                self.panel_setze(alt, true, true, seite);
            }
        }
        self.ui.panel_benutzt = reihe;
    }

    /// T0233: „Seitenleiste immer anzeigen“ aus den Einstellungen (Vorgabe aus).
    pub fn seitenleiste_immer(&self) -> bool {
        self.prefs.seitenleiste_immer
    }

    /// Liegt auf dieser Seite ein **ausgeklapptes** Panel? Dann entfällt dort
    /// im Standardbetrieb die Leiste (T0233).
    pub fn panel_ausgeklappt_auf(&self, seite: PanelSeite) -> bool {
        SeitenPanel::ALLE.iter().any(|p| {
            self.panel_seite(*p) == seite && self.panel_offen(*p) && !self.panel_eingeklappt(*p)
        })
    }

    /// Klick auf ein Symbol der Seitenleiste.
    ///
    /// Liegt das Panel schon auf dieser Seite, klappt der Klick es ein bzw.
    /// wieder aus. Liegt es auf der anderen Seite, wandert es hierher und ist
    /// danach ausgeklappt — so ist jedes Panel von jeder Seite erreichbar.
    ///
    /// T0233: Im Standardbetrieb ist **je Seite ein** Panel offen; ein Klick
    /// schließt die übrigen Panels **derselben** Seite (die andere Seite behält
    /// ihres) und klappt das gewählte aus.
    pub fn leisten_klick(&mut self, p: SeitenPanel, seite: PanelSeite) {
        if self.seitenleiste_immer() {
            if self.panel_seite(p) == seite {
                let eingeklappt = self.panel_eingeklappt(p);
                self.panel_setze(p, true, !eingeklappt, seite);
            } else {
                self.panel_setze(p, true, false, seite);
            }
        } else {
            for q in SeitenPanel::ALLE {
                if q != p && self.panel_seite(q) == seite {
                    self.panel_setze(q, false, false, seite);
                }
            }
            self.panel_setze(p, true, false, seite);
        }
        // T0222: Das dritte ausgeklappte Panel dieser Seite gibt sofort auf,
        // damit das Bild gar nicht erst drei zusammengequetschte zeigt.
        self.panels_begrenzen();
    }

    /// Zeichnet Leisten und ausgeklappte Panels beider Seiten.
    ///
    /// Die Leisten stehen **immer** — auf beiden Seiten mit denselben drei
    /// Symbolen (T0222), damit jedes Panel von jeder Seite erreichbar bleibt.
    /// Ausgeklappt sind höchstens zwei Panels je Seite (`panels_begrenzen`);
    /// sie teilen sich die Höhe.
    fn seitenleisten(&mut self, ctx: &egui::Context) {
        self.ui.last_leisten_icons.clear();
        // T0222: Höchstens zwei Panels je Seite — vor dem Zeichnen aufräumen.
        self.panels_begrenzen();
        for seite in PanelSeite::ALL {
            // T0222: Die Leiste steht **immer**, auch wenn kein Panel
            // eingeklappt ist. Vorher verschwand sie, sobald drei Panels
            // ausgeklappt waren — und mit ihr der einzige Weg, ein Panel zu
            // wechseln.
            // T0233: Im Standardbetrieb („Seitenleiste immer anzeigen“ aus)
            // entfällt die Leiste auf einer Seite, sobald dort ein Panel
            // ausgeklappt ist — der Weg zum Wechseln ist dann das Einklappen.
            if self.seitenleiste_immer() || !self.panel_ausgeklappt_auf(seite) {
                let id = match seite {
                    PanelSeite::Links => "seitenleiste_links",
                    PanelSeite::Rechts => "seitenleiste_rechts",
                };
                let mut panel = match seite {
                    PanelSeite::Links => egui::SidePanel::left(id),
                    PanelSeite::Rechts => egui::SidePanel::right(id),
                };
                panel
                    .exact_width(auf_pixel(ctx, LEISTE_W))
                    .resizable(false)
                    .frame(Frame::none().fill(Palette::panel()))
                    .show(ctx, |ui| self.leiste(ui, seite));
            }
            // Ausgeklappte Panels dieser Seite, in der Reihenfolge der Leiste.
            let hier: Vec<SeitenPanel> = SeitenPanel::ALLE
                .iter()
                .copied()
                .filter(|p| {
                    self.panel_offen(*p)
                        && !self.panel_eingeklappt(*p)
                        && self.panel_seite(*p) == seite
                })
                .collect();
            if hier.is_empty() {
                continue;
            }
            let breite = hier.iter().map(|p| p.breite()).fold(0.0_f32, f32::max);
            let id = match seite {
                PanelSeite::Links => "seiten_panel_links",
                PanelSeite::Rechts => "seiten_panel_rechts",
            };
            let mut panel = match seite {
                PanelSeite::Links => egui::SidePanel::left(id),
                PanelSeite::Rechts => egui::SidePanel::right(id),
            };
            panel
                .exact_width(auf_pixel(ctx, breite + 20.0))
                .resizable(false)
                .frame(
                    Frame::none()
                        .fill(Palette::panel())
                        .inner_margin(Margin::symmetric(10.0, 8.0)),
                )
                .show(ctx, |ui| {
                    // Liegen beide Panels auf einer Seite, teilen sie sich die
                    // Höhe — sonst nähme das erste dem zweiten alles weg.
                    let n = hier.len();
                    for (i, p) in hier.iter().enumerate() {
                        let hoehe = if i + 1 == n {
                            ui.available_height()
                        } else {
                            (ui.available_height() / (n - i) as f32).max(120.0)
                        };
                        let _ = ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), hoehe),
                            Layout::top_down(Align::Min),
                            |ui| match p {
                                SeitenPanel::Layer => panels::layers(self, ui),
                                SeitenPanel::Eigenschaften => panels::properties(self, ui),
                                SeitenPanel::Blatt => panels::blaetter(self, ui),
                            },
                        );
                        if i + 1 < n {
                            ui.separator();
                        }
                    }
                });
        }
    }

    /// Zeichnet die Leiste einer Seite: **beide** Symbole, auf beiden Seiten
    /// dieselben (T0203).
    fn leiste(&mut self, ui: &mut egui::Ui, seite: PanelSeite) {
        let tr = jjcad_core::i18n::tr;
        ui.vertical_centered(|ui| {
            ui.add_space(6.0);
            for p in SeitenPanel::ALLE {
                let hier = self.panel_seite(p) == seite;
                let ausgeklappt = hier && self.panel_offen(p) && !self.panel_eingeklappt(p);
                let tip = if !hier {
                    tr(match p {
                        SeitenPanel::Layer => "gui.app.layers_hierher",
                        SeitenPanel::Eigenschaften => "gui.app.eigenschaften_hierher",
                        SeitenPanel::Blatt => "gui.app.blaetter_hierher",
                    })
                } else if ausgeklappt {
                    tr(match p {
                        SeitenPanel::Layer => "gui.app.layers_einklappen",
                        SeitenPanel::Eigenschaften => "gui.app.eigenschaften_einklappen",
                        SeitenPanel::Blatt => "gui.app.blaetter_einklappen",
                    })
                } else {
                    tr(match p {
                        SeitenPanel::Layer => "gui.app.layers_ausklappen",
                        SeitenPanel::Eigenschaften => "gui.app.eigenschaften_ausklappen",
                        SeitenPanel::Blatt => "gui.app.blaetter_ausklappen",
                    })
                };
                let resp = widgets::icon_button(ui, p.icon(), 22.0, ausgeklappt, tip);
                self.ui.last_leisten_icons.push((p, seite, resp.rect));
                if resp.clicked() {
                    self.leisten_klick(p, seite);
                }
                ui.add_space(6.0);
            }
        });
    }
}

/// Einheitliches Schema für die Chat-Meldungen:
///
/// * Aufforderung: `<Werkzeug> · <Schritt>` — was der Nutzer jetzt tun muss,
///   kurz und im Imperativ (`Wand · Ende klicken`).
/// * Ergebnis: `<Objekt> · <Zustand>` (`Wand · gesetzt`, `Auswahl · kopiert`).
/// * Abbruch/Fehler: `<Werkzeug> · <Grund>` (`Tür · nur auf einer Wand`).
///
/// Der Werkzeugname ist immer die Beschriftung aus dem Aktionsregister, damit
/// Ribbon, Hotbar und Chat dasselbe Wort benutzen.
pub fn hinweis_unbekannt() -> &'static str {
    jjcad_core::i18n::tr("gui.app.werkzeug_aktion_waehlen")
}

/// Aufforderung des laufenden Werkzeugs — ein Text pro Bedienschritt.
///
/// `opts` liefert den Einfügemodus: Rechteck und Ellipse fragen in der
/// Basislinien-Variante einen Punkt mehr ab als achsenparallel.
pub(crate) fn commands_hint(tool: &Tool, opts: &ToolOptions) -> &'static str {
    // Textwerkzeug: nach dem Klick wird getippt, nicht mehr geklickt.
    if tool.action == "draw.text" && tool.phase == ToolPhase::TextInput {
        return jjcad_core::i18n::tr("gui.app.text_text_eingeben_esc_uebernimmt");
    }
    let lebt = !tool.pts.is_empty();
    let modus = opts.einfuegemodus(&tool.action);
    match tool.action.as_str() {
        "draw.line" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.linie_endpunkt_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.linie_startpunkt_klicken")
            }
        }
        "draw.polyline" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.polylinie_naechster_punkt_enter_beendet")
            } else {
                jjcad_core::i18n::tr("gui.app.polylinie_startpunkt_klicken")
            }
        }
        "draw.spline" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.spline_naechster_punkt_enter_beendet")
            } else {
                jjcad_core::i18n::tr("gui.app.spline_startpunkt_klicken")
            }
        }
        // Pfad: der Schritt hängt am Modus und am halben Bogen (T0229).
        "draw.path" => {
            if !lebt {
                jjcad_core::i18n::tr("gui.app.pfad_startpunkt_klicken")
            } else if tool.schritte.last() == Some(&jjcad_core::pfad::PfadSchritt::ArcPoint) {
                jjcad_core::i18n::tr("gui.app.pfad_bogenende_klicken")
            } else if tool.bogen {
                jjcad_core::i18n::tr("gui.app.pfad_bogenpunkt_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.pfad_naechster_punkt_enter_beendet")
            }
        }
        "draw.rect" => match (modus, tool.pts.len()) {
            (Einfuegemodus::Basislinie, 0) => jjcad_core::i18n::tr("gui.app.rechteck_basislinie_anfang_klicken"),
            (Einfuegemodus::Basislinie, 1) => jjcad_core::i18n::tr("gui.app.rechteck_basislinie_ende_klicken"),
            (Einfuegemodus::Basislinie, _) => jjcad_core::i18n::tr("gui.app.rechteck_hoehe_klicken"),
            (_, 0) => jjcad_core::i18n::tr("gui.app.rechteck_erste_ecke_klicken"),
            _ => jjcad_core::i18n::tr("gui.app.rechteck_zweite_ecke_klicken"),
        },
        "draw.circle" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.kreis_radius_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.kreis_mittelpunkt_klicken")
            }
        }
        "draw.ellipse" => match (modus, tool.pts.len()) {
            (Einfuegemodus::Basislinie, 0) => jjcad_core::i18n::tr("gui.app.ellipse_mittelachse_anfang_klicken"),
            (Einfuegemodus::Basislinie, 1) => jjcad_core::i18n::tr("gui.app.ellipse_mittelachse_ende_klicken"),
            (Einfuegemodus::Basislinie, _) => jjcad_core::i18n::tr("gui.app.ellipse_breite_klicken"),
            (_, 0) => jjcad_core::i18n::tr("gui.app.ellipse_mittelpunkt_klicken"),
            _ => jjcad_core::i18n::tr("gui.app.ellipse_achsen_aufziehen"),
        },
        "draw.arc" => {
            if opts.arc_mode == Bogenmodus::Mittelpunkt {
                match tool.pts.len() {
                    0 => jjcad_core::i18n::tr("gui.app.bogen_mittelpunkt_klicken"),
                    1 => jjcad_core::i18n::tr("gui.app.bogen_startpunkt_klicken"),
                    _ => jjcad_core::i18n::tr("gui.app.bogen_endpunkt_klicken"),
                }
            } else {
                match tool.pts.len() {
                    0 => jjcad_core::i18n::tr("gui.app.bogen_startpunkt_klicken"),
                    1 => jjcad_core::i18n::tr("gui.app.bogen_endpunkt_klicken"),
                    _ => jjcad_core::i18n::tr("gui.app.bogen_punkt_auf_dem_bogen_klicken"),
                }
            }
        }
        "draw.image" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.bild_zweite_ecke_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.bild_erste_ecke_klicken")
            }
        }
        "draw.point" => jjcad_core::i18n::tr("gui.app.punkt_position_klicken"),
        "draw.text" => jjcad_core::i18n::tr("gui.app.text_position_klicken"),
        "draw.wall" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.wand_ende_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.wand_anfang_klicken")
            }
        }
        "draw.door" => jjcad_core::i18n::tr("gui.app.tuer_in_eine_wand_klicken"),
        "draw.window" => jjcad_core::i18n::tr("gui.app.fenster_in_eine_wand_klicken"),
        "draw.stair" => match tool.pts.len() {
            0 => jjcad_core::i18n::tr("gui.app.treppe_aussenkante_anfang_klicken"),
            1 => jjcad_core::i18n::tr("gui.app.treppe_aussenkante_ende_klicken"),
            _ => jjcad_core::i18n::tr("gui.app.treppe_breite_klicken"),
        },
        "draw.fill" => jjcad_core::i18n::tr("gui.app.fuellflaeche_kontur_anklicken"),
        "draw.fill.boundary" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.fuellflaeche_naechster_umrisspunkt_enter")
            } else {
                jjcad_core::i18n::tr("gui.app.fuellflaeche_umriss_abfahren")
            }
        }
        "draw.fill.points" => {
            if tool.pts.len() < 2 {
                jjcad_core::i18n::tr("gui.app.fuellflaeche_punkt_klicken_mind_3")
            } else {
                jjcad_core::i18n::tr("gui.app.fuellflaeche_enter_schliesst")
            }
        }
        "draw.fill.paint" => jjcad_core::i18n::tr("gui.app.farbeimer_flaeche_anklicken"),
        "draw.volume" => jjcad_core::i18n::tr("gui.app.volumen_kontur_anklicken"),
        "draw.volume.points" => {
            if tool.pts.len() < 2 {
                jjcad_core::i18n::tr("gui.app.volumen_nach_punktfolge_punkt_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.volumen_nach_punktfolge_enter_schliesst")
            }
        }
        "draw.volume.paint" => jjcad_core::i18n::tr("gui.app.volumen_fuellen_leere_flaeche_anklicken"),
        "draw.hatch" => jjcad_core::i18n::tr("gui.app.schraffur_kontur_anklicken"),
        "dim.linear" => match tool.pts.len() {
            0 => jjcad_core::i18n::tr("gui.app.mass_erster_punkt"),
            1 => jjcad_core::i18n::tr("gui.app.mass_zweiter_punkt"),
            _ => jjcad_core::i18n::tr("gui.app.mass_masslinie_platzieren"),
        },
        // Maßkette: der Schritt hängt am Eingabemodus (T0154) — erst die Lage
        // der Maßlinie, dann die Messpunkte. Den Modus sagt das Werkzeug.
        "dim.chain" | "dim.chain.h" | "dim.chain.v" | "dim.chain.d" => {
            let modus = DimModus::aus_werkzeug(&tool.action).unwrap_or(DimModus::Direkt);
            if crate::canvas::dim_legt_masslinie(tool, modus) {
                match modus {
                    DimModus::Direkt => {
                        jjcad_core::i18n::tr("gui.app.masskette_position_parallel_klicken")
                    }
                    _ => jjcad_core::i18n::tr("gui.app.masskette_position_klicken"),
                }
            } else if tool.pts.is_empty() {
                jjcad_core::i18n::tr("gui.app.masskette_erster_punkt")
            } else if crate::canvas::dim_messpunkte(tool, modus).len() < 2 {
                jjcad_core::i18n::tr("gui.app.masskette_zweiter_punkt")
            } else {
                jjcad_core::i18n::tr("gui.app.masskette_messpunkt_klicken_enter")
            }
        }
        // Messpunkte ändern: erst die Maßkette, dann der Punkt (T0154).
        // Höhenangabe (T0253): ein Klick auf den Abschnitt öffnet das Feld.
        "dim.height" => jjcad_core::i18n::tr("gui.app.hoehenangabe_masskette_klicken"),
        "dim.point.add" => {
            if tool.obj.is_some() {
                jjcad_core::i18n::tr("gui.app.punkt_hinzufuegen_messpunkt_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.punkt_hinzufuegen_masskette_klicken")
            }
        }
        "dim.point.remove" => {
            if tool.obj.is_some() {
                jjcad_core::i18n::tr("gui.app.punkt_entfernen_messpunkt_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.punkt_entfernen_masskette_klicken")
            }
        }
        "dim.radius" => jjcad_core::i18n::tr("gui.app.radius_kreis_oder_bogen"),
        "dim.elevation.plan" => jjcad_core::i18n::tr("gui.app.grundriss_kote_punkt_klicken"),
        "dim.elevation.section" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.schnitt_kote_naechster_punkt_enter_beendet")
            } else {
                jjcad_core::i18n::tr("gui.app.schnitt_kote_erster_punkt_0_00")
            }
        }
        "dim.room" => {
            if tool.pts.len() < 2 {
                "Raumstempel · Raumecken klicken (mind. 2)"
            } else {
                jjcad_core::i18n::tr("gui.app.raumstempel_enter_setzt_den_stempel")
            }
        }
        "modify.select" => jjcad_core::i18n::tr("gui.app.auswaehlen_objekte_oder_fenster"),
        "modify.move" => match (tool.selecting, lebt) {
            (true, _) => jjcad_core::i18n::tr("gui.app.verschieben_objekte_waehlen_enter"),
            (false, true) => jjcad_core::i18n::tr("gui.app.verschieben_zielpunkt_klicken"),
            (false, false) => jjcad_core::i18n::tr("gui.app.verschieben_basispunkt_klicken"),
        },
        // Kein Zielpunkt: der Klick setzt nur den Basispunkt der Zwischenablage.
        "modify.copy" => match tool.selecting {
            true => jjcad_core::i18n::tr("gui.app.kopieren_mit_basispunkt_objekte_waehlen"),
            false => jjcad_core::i18n::tr("gui.app.kopieren_mit_basispunkt_basispunkt_klicken"),
        },
        "modify.rotate" => match tool.pts.len() {
            0 => jjcad_core::i18n::tr("gui.app.drehen_zentrum_klicken"),
            1 => jjcad_core::i18n::tr("gui.app.drehen_bezugspunkt_klicken"),
            _ => jjcad_core::i18n::tr("gui.app.drehen_neuen_punkt_klicken"),
        },
        "modify.mirror" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.spiegeln_zweiten_achspunkt_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.spiegeln_punkt_der_achse_klicken")
            }
        }
        "modify.scale" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.skalieren_faktor_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.skalieren_bezugspunkt_klicken")
            }
        }
        // Parallele (T0200): „weiter" heißt hier **Vorlage gewählt** — die
        // Punktliste bleibt leer, das Werkzeug merkt sich die Linie in `obj`.
        "draw.parallel" => {
            if tool.obj.is_some() {
                jjcad_core::i18n::tr("gui.app.parallele_seite_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.parallele_quelle_waehlen")
            }
        }
        "modify.offset" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.versetzen_seite_klicken")
            } else {
                jjcad_core::i18n::tr("gui.app.versetzen_objekt_anklicken")
            }
        }
        "modify.trim" => jjcad_core::i18n::tr("gui.app.stutzen_segment_anklicken"),
        "modify.extend" => jjcad_core::i18n::tr("gui.app.verlaengern_ende_anklicken"),
        "modify.intersect" => {
            if tool.obj.is_some() {
                jjcad_core::i18n::tr("gui.app.verschneiden_zweites_objekt_anklicken")
            } else {
                jjcad_core::i18n::tr("gui.app.verschneiden_erstes_objekt_anklicken")
            }
        }
        // Abrunden und Phase (T0247): erst die zwei Linien, dann das Maß.
        "modify.fillet" => {
            // Auch der Knick einer Polylinie ist eine fertige Ecke (T0247).
            if tool.obj2.is_some() || tool.knoten.is_some() {
                jjcad_core::i18n::tr("gui.app.abrunden_radius_klicken")
            } else if tool.obj.is_some() {
                jjcad_core::i18n::tr("gui.app.abrunden_zweite_linie_anklicken")
            } else {
                jjcad_core::i18n::tr("gui.app.abrunden_erste_linie_anklicken")
            }
        }
        "modify.chamfer" => {
            if tool.obj2.is_some() || tool.knoten.is_some() {
                jjcad_core::i18n::tr("gui.app.phase_mass_klicken")
            } else if tool.obj.is_some() {
                jjcad_core::i18n::tr("gui.app.phase_zweite_linie_anklicken")
            } else {
                jjcad_core::i18n::tr("gui.app.phase_erste_linie_anklicken")
            }
        }
        "modify.explode" => jjcad_core::i18n::tr("gui.app.zerlegen_objekt_anklicken"),
        "modify.area.split" => {
            if lebt {
                jjcad_core::i18n::tr("gui.app.flaeche_schneiden_naechster_punkt_enter")
            } else {
                jjcad_core::i18n::tr("gui.app.flaeche_schneiden_randpunkt_klicken")
            }
        }
        "modify.quickselect" => jjcad_core::i18n::tr("gui.app.schnellauswahl_objekt_anklicken"),
        "modify.order" => jjcad_core::i18n::tr("gui.app.reihenfolge_objekt_anklicken"),
        "block.insert" => jjcad_core::i18n::tr("gui.app.block_position_klicken"),
        "sheet.viewport_tool" => jjcad_core::i18n::tr("gui.app.ansichtsfenster_rechteck_aufziehen"),
        "view.rotate_tool" => match tool.pts.len() {
            0 => jjcad_core::i18n::tr("gui.app.ansicht_drehen_zentrum_klicken"),
            1 => jjcad_core::i18n::tr("gui.app.ansicht_drehen_bezugspunkt_klicken"),
            _ => jjcad_core::i18n::tr("gui.app.ansicht_drehen_richtung_klicken"),
        },
        _ => hinweis_unbekannt(),
    }
}

/// Ergebnis-Meldung nach einem erfolgreichen Befehl: `<Objekt> · <Zustand>`.
/// `None` = der Befehl meldet sein Ergebnis selbst (Drehen, Stutzen,
/// Reihenfolge) oder braucht keine Rückmeldung.
pub(crate) fn abschluss_meldung(action: &str) -> Option<&'static str> {
    Some(match action {
        "draw.line" => jjcad_core::i18n::tr("gui.app.linie_gesetzt"),
        "draw.polyline" => jjcad_core::i18n::tr("gui.app.polylinie_abgeschlossen"),
        "draw.parallel" => jjcad_core::i18n::tr("gui.app.parallele_gesetzt"),
        "draw.spline" => jjcad_core::i18n::tr("gui.app.spline_abgeschlossen"),
        "draw.path" => jjcad_core::i18n::tr("gui.app.pfad_abgeschlossen"),
        "path.connect" => jjcad_core::i18n::tr("gui.app.objekte_verbunden"),
        "draw.rect" => jjcad_core::i18n::tr("gui.app.rechteck_gesetzt"),
        "draw.image" => jjcad_core::i18n::tr("gui.app.bild_gesetzt"),
        "draw.circle" => jjcad_core::i18n::tr("gui.app.kreis_gesetzt"),
        "draw.ellipse" => jjcad_core::i18n::tr("gui.app.ellipse_gesetzt"),
        "draw.arc" => jjcad_core::i18n::tr("gui.app.bogen_gesetzt"),
        "draw.point" => jjcad_core::i18n::tr("gui.app.punkt_gesetzt"),
        "draw.text" => jjcad_core::i18n::tr("gui.app.text_gesetzt"),
        "draw.wall" => jjcad_core::i18n::tr("gui.app.wand_gesetzt"),
        "draw.door" => jjcad_core::i18n::tr("gui.app.tuer_eingefuegt"),
        "draw.window" => jjcad_core::i18n::tr("gui.app.fenster_eingefuegt"),
        "draw.stair" => jjcad_core::i18n::tr("gui.app.treppe_gesetzt"),
        "draw.fill" | "draw.fill.boundary" | "draw.fill.points" | "draw.fill.paint" => {
            jjcad_core::i18n::tr("gui.app.fuellflaeche_gesetzt")
        }
        "draw.hatch" => jjcad_core::i18n::tr("gui.app.schraffur_gesetzt"),
        "dim.linear" => jjcad_core::i18n::tr("gui.app.mass_gesetzt"),
        "dim.chain" | "dim.chain.h" | "dim.chain.v" | "dim.chain.d" => {
            jjcad_core::i18n::tr("gui.app.masskette_gesetzt")
        }
        "dim.point.add" => jjcad_core::i18n::tr("gui.app.punkt_hinzufuegen_gesetzt"),
        "dim.point.remove" => jjcad_core::i18n::tr("gui.app.punkt_entfernen_entfernt"),
        "dim.radius" => jjcad_core::i18n::tr("gui.app.radius_gesetzt"),
        "dim.elevation.plan" => jjcad_core::i18n::tr("gui.app.grundriss_kote_gesetzt"),
        "dim.elevation.section" => jjcad_core::i18n::tr("gui.app.schnitt_kote_gesetzt"),
        "dim.room" => jjcad_core::i18n::tr("gui.app.raumstempel_gesetzt"),
        "modify.move" => jjcad_core::i18n::tr("gui.app.auswahl_verschoben"),
        "modify.copy" => jjcad_core::i18n::tr("gui.app.auswahl_kopiert"),
        "modify.mirror" => jjcad_core::i18n::tr("gui.app.auswahl_gespiegelt"),
        "modify.scale" => jjcad_core::i18n::tr("gui.app.auswahl_skaliert"),
        "modify.offset" => jjcad_core::i18n::tr("gui.app.auswahl_versetzt"),
        "modify.explode" => jjcad_core::i18n::tr("gui.app.objekt_zerlegt"),
        "modify.intersect" => jjcad_core::i18n::tr("gui.app.verschneiden_ecke_gesetzt"),
        "modify.fillet" => jjcad_core::i18n::tr("gui.app.ecke_abgerundet"),
        "modify.chamfer" => jjcad_core::i18n::tr("gui.app.ecke_phase_gesetzt"),
        "modify.area.union" => jjcad_core::i18n::tr("gui.app.fuellflaeche_vereinigt"),
        "modify.area.split" => jjcad_core::i18n::tr("gui.app.fuellflaeche_geteilt"),
        "block.insert" => jjcad_core::i18n::tr("gui.app.block_eingefuegt"),
        "sheet.viewport_tool" | "sheet.add_viewport" => {
            jjcad_core::i18n::tr("gui.app.ansichtsfenster_angelegt")
        }
        _ => return None,
    })
}

/// Weltkoordinaten aus Bildschirmposition.
///
/// Die Ansicht kann gedreht sein (`View::rot`): erst den Bildschirmversatz
/// zurückdrehen, dann auf die Bildmitte addieren. Diese eine Umrechnung trägt
/// die Drehung für Fangen, Trefferprüfung, Griffe und alle Zeichenwege.
pub fn screen_to_world(rect: Rect, view: View, pos: Pos2) -> V2 {
    let cx = rect.center().x as f64;
    let cy = rect.center().y as f64;
    let sx = (pos.x as f64 - cx) / view.zoom;
    let sy = -(pos.y as f64 - cy) / view.zoom;
    let d = rot_vec(V2::new(sx, sy), -view.rot);
    V2::new(view.center.x + d.x, view.center.y + d.y)
}

pub fn world_to_screen(rect: Rect, view: View, p: V2) -> Pos2 {
    let cx = rect.center().x as f64;
    let cy = rect.center().y as f64;
    let d = rot_vec(V2::new(p.x - view.center.x, p.y - view.center.y), view.rot);
    Pos2::new(
        (cx + d.x * view.zoom) as f32,
        (cy - d.y * view.zoom) as f32,
    )
}

/// Sichtbarer Weltbereich als (x0, y0, x1, y1) — die vier Bildschirmecken
/// zurückgerechnet. Bei gedrehter Ansicht ist das die achsenparallele Hülle des
/// gedrehten Rechtecks; das Raster wird über diese Hülle gespannt.
pub fn visible_world_bounds(rect: Rect, view: View) -> (f64, f64, f64, f64) {
    let ecken = [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ];
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for e in ecken {
        let w = screen_to_world(rect, view, e);
        x0 = x0.min(w.x);
        y0 = y0.min(w.y);
        x1 = x1.max(w.x);
        y1 = y1.max(w.y);
    }
    (x0, y0, x1, y1)
}

/// Welches Menü unter der Statusleiste offen ist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusMenu {
    Raster,
    Ortho,
    Fang,
    Staerke,
}

/// Worauf sich ein Kontextmenü bezieht.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind {
    /// Rechtsklick auf ein Zeichnungselement
    Object(Id),
    /// Rechtsklick in die leere Zeichenfläche
    Canvas,
    /// Rechtsklick auf einen Chip der Statusleiste
    Chip(StatusMenu),
}

/// Ein offenes Kontextmenü.
#[derive(Clone, Copy, Debug)]
pub struct MenuState {
    pub kind: MenuKind,
    pub pos: egui::Pos2,
    /// Im Öffnungsframe darf der auslösende Klick das Menü nicht gleich schließen.
    pub fresh: bool,
}

impl App {
    pub fn open_menu(&mut self, kind: MenuKind, pos: egui::Pos2) {
        self.ui.menu = Some(MenuState {
            kind,
            pos,
            fresh: true,
        });
    }

    pub fn menu_open(&self) -> bool {
        self.ui.menu.is_some()
    }

    /// T0189: Die Objekte, auf die ein Objektmenü wirkt.
    ///
    /// Liegt der angeklickte Gegenstand in einer **Mehrfachauswahl**, gilt das
    /// Menü für alle gewählten Objekte — der Rechtsklick verengt die Auswahl
    /// nicht mehr. Sonst für das eine Objekt.
    pub fn menue_ziel(&self, id: Id) -> Vec<Id> {
        let sel = self.selection();
        if sel.len() > 1 && sel.contains(&id) {
            sel.to_vec()
        } else {
            vec![id]
        }
    }

    /// Steht das Eigenschaften-Panel **offen** — also eingeschaltet *und*
    /// aufgeklappt? Eingeklappt ist es nur noch die schmale Leiste am rechten
    /// Rand, und ausgeschaltet ist es gar nicht da. Beide Fälle zählen als
    /// „nicht offen“ (T0179).
    pub fn eigenschaften_panel_offen(&self) -> bool {
        self.ui.props_open && !self.ui.props_collapsed
    }

    /// Zeichnet das offene Kontextmenü (Objekt, Zeichenfläche oder Chip).
    fn draw_menu(&mut self, ctx: &egui::Context) {
        let Some(menu) = self.ui.menu else {
            return;
        };
        let mut close = false;
        // Unten in der Zeichenfläche (z. B. auf den Schaltern der unteren
        // Leiste) klappt das Menü nach oben auf — sonst liegt die untere Hälfte
        // hinter dem Fensterrand und ist nicht erreichbar.
        let unten = menu.pos.y > ctx.screen_rect().bottom() - 300.0;
        let mut area = egui::Area::new(egui::Id::new("jjcad_menu"))
            .order(egui::Order::Tooltip)
            .interactable(true)
            .sense(egui::Sense::click())
            .constrain(true);
        if unten {
            area = area
                .pivot(egui::Align2::LEFT_BOTTOM)
                .fixed_pos(egui::Pos2::new(menu.pos.x, menu.pos.y - 6.0));
        } else {
            area = area.fixed_pos(menu.pos);
        }
        let shown = area.show(ctx, |ui| {
            egui::Frame::menu(&ctx.style()).show(ui, |ui| {
                ui.set_min_width(210.0);
                let v = &mut ui.style_mut().visuals.widgets;
                v.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                v.inactive.bg_fill = egui::Color32::TRANSPARENT;
                v.inactive.bg_stroke = egui::Stroke::NONE;
                v.hovered.weak_bg_fill = Palette::panel_alt();
                v.hovered.bg_stroke = egui::Stroke::NONE;
                v.active.weak_bg_fill = Palette::panel_alt();
                v.active.bg_stroke = egui::Stroke::NONE;

                match menu.kind {
                    MenuKind::Object(id) => {
                        // T0189: Das Menü gilt für die **ganze** Auswahl, wenn
                        // das angeklickte Objekt zu einer Mehrfachauswahl
                        // gehört (`App::menue_ziel`).
                        let ziel = self.menue_ziel(id);
                        let mehrfach = ziel.len() > 1;
                        // T0185: Eine Blockinstanz führt über das Menü direkt in
                        // den Blockeditor. Nur **eigene** Blöcke — Systemblöcke
                        // sind schreibgeschützt, genau wie im Rechtsklick der
                        // Blockbibliothek (`inventory::block_tile`).
                        let block_bearbeitbar = self
                            .doc()
                            .entity(id)
                            .and_then(|e| match &e.kind {
                                EntityKind::BlockRef { block, .. } => Some(block.clone()),
                                _ => None,
                            })
                            .filter(|b| {
                                self.doc()
                                    .block(b)
                                    .map(|blk| {
                                        blk.source != jjcad_core::model::BlockSource::System
                                    })
                                    .unwrap_or(false)
                            })
                            // T0189: Bei mehreren Objekten gibt es keinen
                            // einzelnen Block zu bearbeiten.
                            .filter(|_| !mehrfach);
                        let (typ, layer) = self
                            .doc()
                            .entity(id)
                            .map(|e| {
                                (
                                    e.kind.type_name().to_string(),
                                    self.doc()
                                        .layer(e.layer)
                                        .map(|l| l.name.clone())
                                        .unwrap_or_default(),
                                )
                            })
                            .unwrap_or(("Objekt".into(), String::new()));
                        if mehrfach {
                            // T0189: Anzahl statt einer Objektart — bei
                            // mehreren Objekten nennt eine einzelne Art die
                            // falsche. Die Ebene steht nur darunter, wenn alle
                            // Objekte auf derselben liegen.
                            ui.label(theme::bold(
                                jjcad_core::i18n::t_args_owned(
                                    "gui.dialogs.bestand_x_objekte",
                                    &[("anzahl", ziel.len().to_string())],
                                ),
                                12.0,
                            ));
                            let mut ebenen = ziel
                                .iter()
                                .filter_map(|i| self.doc().entity(*i).map(|e| e.layer));
                            if let Some(erste) = ebenen.next() {
                                if ebenen.all(|l| l == erste) {
                                    if let Some(l) = self.doc().layer(erste) {
                                        ui.label(theme::dim(l.name.clone()));
                                    }
                                }
                            }
                        } else {
                            ui.label(theme::bold(typ, 12.0));
                            ui.label(theme::dim(layer));
                        }
                        ui.separator();
                        for (label, action) in [
                            (jjcad_core::i18n::tr("gui.app.verschieben"), "modify.move"),
                            (jjcad_core::i18n::tr("gui.app.kopieren_mit_basispunkt"), "modify.copy"),
                            (jjcad_core::i18n::tr("gui.app.drehen"), "modify.rotate"),
                            (jjcad_core::i18n::tr("gui.app.spiegeln"), "modify.mirror"),
                        ] {
                            if ui.button(label).clicked() {
                                self.start_tool(action);
                                close = true;
                            }
                        }
                        ui.separator();
                        // T0185: bearbeitet die **Definition**, nicht die
                        // Instanz — steht deshalb vor der Zeichenreihenfolge.
                        if let Some(block) = block_bearbeitbar.as_deref() {
                            if ui
                                .button(jjcad_core::i18n::tr("gui.app.block_bearbeiten"))
                                .clicked()
                            {
                                self.oeffne_block_editor(block);
                                close = true;
                            }
                            ui.separator();
                        }
                        // Zeichenreihenfolge: Position in der Objektliste
                        ui.label(theme::faint(jjcad_core::i18n::tr("gui.app.reihenfolge")));
                        if ui.button(jjcad_core::i18n::tr("gui.app.in_den_vordergrund")).clicked() {
                            self.order_to(true, ziel.clone());
                            close = true;
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.app.in_den_hintergrund")).clicked() {
                            self.order_to(false, ziel.clone());
                            close = true;
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.app.vor_element")).clicked() {
                            self.session_mut().select(ziel.clone());
                            self.order_pick_start(true);
                            close = true;
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.app.hinter_element")).clicked() {
                            self.session_mut().select(ziel.clone());
                            self.order_pick_start(false);
                            close = true;
                        }
                        ui.separator();
                        // Verbinden (T0229): nur, wenn die ganze Auswahl aus
                        // Linien, Bögen, Ellipsenbögen oder Splines besteht.
                        // Ob sie zusammenhängen, prüft der Befehl selbst.
                        {
                            let faehig = ziel
                                .iter()
                                .filter(|id| {
                                    matches!(
                                        self.doc().entity(**id).map(|e| &e.kind),
                                        Some(EntityKind::Line { .. })
                                            | Some(EntityKind::Arc { .. })
                                            | Some(EntityKind::EllipseArc { .. })
                                            | Some(EntityKind::Spline { .. })
                                    )
                                })
                                .count();
                            if faehig >= 2 && faehig == ziel.len() {
                                if ui.button(jjcad_core::i18n::tr("gui.app.verbinden")).clicked() {
                                    match self
                                        .session_mut()
                                        .execute("path.connect", &serde_json::json!({ "ids": ziel.clone() }))
                                    {
                                        Ok(v) => {
                                            self.mark_changed();
                                            let neu = v
                                                .get("created")
                                                .and_then(|c| c.get(0))
                                                .and_then(|x| x.as_u64());
                                            self.session_mut().select(neu.into_iter().collect());
                                        }
                                        Err(e) => self.toast(e),
                                    }
                                    close = true;
                                }
                                ui.separator();
                            }
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.app.zerlegen")).clicked() {
                            let _ = self.session_mut().execute(
                                "modify.explode",
                                &serde_json::json!({"ids": ziel.clone()}),
                            );
                            self.session_mut().clear_selection();
                            self.mark_changed();
                            close = true;
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.app.loeschen")).clicked() {
                            let _ = self
                                .session_mut()
                                .execute("modify.erase", &serde_json::json!({"ids": ziel.clone()}));
                            self.session_mut().clear_selection();
                            self.mark_changed();
                            close = true;
                        }
                        // T0179: Der Eintrag öffnet das Eigenschaften-Panel.
                        // Steht es schon offen, gibt es nichts zu öffnen —
                        // dann wird er gar nicht erst angeboten (auch der
                        // Trenner entfällt, sonst stünde eine Leerzeile).
                        if !self.eigenschaften_panel_offen() {
                            ui.separator();
                            if ui.button(jjcad_core::i18n::tr("gui.app.eigenschaften_im_panel")).clicked() {
                                self.ui.props_open = true;
                                self.ui.props_collapsed = false;
                                self.session_mut().select(ziel.clone());
                                close = true;
                            }
                        }
                    }
                    MenuKind::Canvas => {
                        ui.label(theme::bold(jjcad_core::i18n::tr("gui.app.zeichenflaeche"), 12.0));
                        ui.separator();
                        let has_clip = !self.ui.clipboard.is_empty();
                        if ui
                            .add_enabled(has_clip, egui::Button::new(jjcad_core::i18n::tr("gui.app.einfuegen")))
                            .clicked()
                        {
                            self.start_ghost_paste();
                            close = true;
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.app.alles_auswaehlen")).clicked() {
                            let ids: Vec<Id> = self.doc().ents().iter().map(|e| e.id).collect();
                            self.session_mut().select(ids);
                            close = true;
                        }
                        if ui.button(jjcad_core::i18n::tr("gui.app.auswahl_aufheben")).clicked() {
                            self.session_mut().clear_selection();
                            close = true;
                        }
                        ui.separator();
                        if ui.button(jjcad_core::i18n::tr("gui.app.ansicht_einpassen")).clicked() {
                            self.fit_view();
                            close = true;
                        }
                    }
                    MenuKind::Chip(kind) => match kind {
                        StatusMenu::Raster => {
                            ui.label(theme::bold(jjcad_core::i18n::tr("gui.app.raster"), 12.0));
                            ui.add_space(2.0);
                            let mut show = self.ui.show_grid;
                            if ui.checkbox(&mut show, jjcad_core::i18n::tr("gui.app.raster_anzeigen")).changed() {
                                self.ui.show_grid = show;
                                let _ = self.session_mut().execute(
                                    "doc.settings",
                                    &serde_json::json!({"show_grid": show, "snap": {"grid": show}}),
                                );
                            }
                            let mut g = self.doc().settings.snap.grid;
                            if ui.checkbox(&mut g, jjcad_core::i18n::tr("gui.app.rasterfang")).changed() {
                                self.set_snap("grid", g);
                            }
                            let step = self.doc().settings.snap.grid_step;
                            ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.app.rasterweite_x_mm", &[("wert1", format!("{:.0}", step))])));
                            // T0144: Art und Modus stehen als Kurzinfo darunter.
                            let art = self.doc().settings.snap.grid_kind;
                            let modus = self.doc().settings.snap.grid_mode;
                            ui.label(theme::dim(format!(
                                "{} · {}",
                                crate::dialogs::rasterart_text(art),
                                crate::dialogs::rastermodus_text(modus)
                            )));
                            ui.separator();
                            if ui.button(jjcad_core::i18n::tr("gui.app.weitere_einstellungen")).clicked() {
                                self.ui.settings_open = true;
                                // Direkt in die Kategorie „Raster“ — dort stehen
                                // alle Rastereinstellungen (T0144).
                                self.ui.settings_kat = crate::dialogs::EINSTELLUNGSKATEGORIEN
                                    .iter()
                                    .position(|k| k.key == "settings.category_raster")
                                    .unwrap_or(0);
                                close = true;
                            }
                        }
                        StatusMenu::Ortho => {
                            ui.label(theme::bold(jjcad_core::i18n::tr("gui.app.ausrichtung"), 12.0));
                            ui.add_space(2.0);
                            let mut o = self.doc().settings.snap.ortho;
                            if ui.checkbox(&mut o, jjcad_core::i18n::tr("gui.app.ortho_0_90")).changed() {
                                self.set_snap("ortho", o);
                            }
                            let mut p = self.doc().settings.snap.polar;
                            if ui.checkbox(&mut p, jjcad_core::i18n::tr("gui.app.polar")).changed() {
                                self.set_snap("polar", p);
                            }
                            let st = self.doc().settings.snap.polar_step;
                            let tol = self.doc().settings.snap.polar_tolerance;
                            ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.app.schrittweite_x_fenster_x", &[("wert1", format!("{:.0}", st)), ("wert2", format!("{:.0}", tol))])));
                            ui.separator();
                            if ui.button(jjcad_core::i18n::tr("gui.app.polarwinkel_aendern")).clicked() {
                                self.ui.settings_open = true;
                                close = true;
                            }
                        }
                        StatusMenu::Fang => {
                            ui.label(theme::bold(jjcad_core::i18n::tr("gui.app.fangmodi"), 12.0));
                            ui.add_space(2.0);
                            let s0 = self.doc().settings.snap.clone();
                            for (label, key, val) in [
                                (jjcad_core::i18n::tr("gui.app.endpunkt"), "endpoint", s0.endpoint),
                                (jjcad_core::i18n::tr("gui.app.wandkanten"), "wall_edges", s0.wall_edges),
                                (jjcad_core::i18n::tr("gui.app.mittelpunkt"), "midpoint", s0.midpoint),
                                (jjcad_core::i18n::tr("gui.app.zentrum"), "center", s0.center),
                                (jjcad_core::i18n::tr("gui.app.schnittpunkt"), "intersection", s0.intersection),
                                (jjcad_core::i18n::tr("gui.app.lot"), "perpendicular", s0.perpendicular),
                                (jjcad_core::i18n::tr("gui.app.tangential"), "tangent", s0.tangent),
                                (jjcad_core::i18n::tr("gui.app.lotfang"), "lot", s0.lot),
                                (jjcad_core::i18n::tr("gui.app.naechster_punkt"), "nearest", s0.nearest),
                            ] {
                                let mut v = val;
                                if ui.checkbox(&mut v, label).changed() {
                                    self.set_snap(key, v);
                                }
                            }
                            ui.separator();
                            if ui.button(jjcad_core::i18n::tr("gui.app.alle_an")).clicked() {
                                for k in [
                                    "endpoint",
                                    "wall_edges",
                                    "midpoint",
                                    "center",
                                    "intersection",
                                    "perpendicular",
                                    "tangent",
                                    "lot",
                                    "nearest",
                                ] {
                                    self.set_snap(k, true);
                                }
                                close = true;
                            }
                            if ui.button(jjcad_core::i18n::tr("gui.app.alle_aus")).clicked() {
                                for k in [
                                    "endpoint",
                                    "wall_edges",
                                    "midpoint",
                                    "center",
                                    "intersection",
                                    "perpendicular",
                                    "tangent",
                                    "lot",
                                    "nearest",
                                ] {
                                    self.set_snap(k, false);
                                }
                                close = true;
                            }
                            // Ortho und Polar stehen seit T0043 hier, nicht mehr
                            // als eigener Chip in der unteren Leiste.
                            ui.separator();
                            ui.label(theme::bold(jjcad_core::i18n::tr("gui.app.ausrichtung"), 12.0));
                            let mut o = self.doc().settings.snap.ortho;
                            if ui.checkbox(&mut o, jjcad_core::i18n::tr("gui.app.ortho_0_90")).changed() {
                                self.set_snap("ortho", o);
                            }
                            let mut p = self.doc().settings.snap.polar;
                            if ui.checkbox(&mut p, jjcad_core::i18n::tr("gui.app.polar")).changed() {
                                self.set_snap("polar", p);
                            }
                            let st = self.doc().settings.snap.polar_step;
                            let tol = self.doc().settings.snap.polar_tolerance;
                            ui.label(theme::dim(jjcad_core::i18n::t_args_owned("gui.app.schrittweite_x_fenster_x_2", &[("st", format!("{:.0}", st)), ("tol", format!("{:.0}", tol))])));
                        }
                        StatusMenu::Staerke => {
                            ui.label(theme::bold(jjcad_core::i18n::tr("gui.app.linienstaerken"), 12.0));
                            ui.add_space(2.0);
                            let mut m = if self.ui.lineweights_scaled { 1 } else { 0 };
                            if ui
                                .radio_value(&mut m, 0, jjcad_core::i18n::tr("gui.app.linienstaerke_bildschirm"))
                                .clicked()
                            {
                                self.ui.lineweights = true;
                                self.ui.lineweights_scaled = false;
                                close = true;
                            }
                            if ui
                                .radio_value(&mut m, 1, jjcad_core::i18n::tr("gui.app.linienstaerke_papier"))
                                .clicked()
                            {
                                self.ui.lineweights = true;
                                self.ui.lineweights_scaled = true;
                                close = true;
                            }
                            ui.separator();
                            ui.label(theme::dim(
                                jjcad_core::i18n::tr("gui.app.in_beiden_modi_bleiben_die_linienstaerken_sichtbar"),
                            ));
                        }
                    },
                }
            });
        });

        let rect = shown.response.rect;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if ctx.input(|i| i.pointer.any_click()) {
            let inside = ctx
                .pointer_latest_pos()
                .map(|p| rect.contains(p))
                .unwrap_or(false);
            if !inside {
                if menu.fresh {
                    self.ui.menu = Some(MenuState {
                        fresh: false,
                        ..menu
                    });
                } else {
                    close = true;
                }
            }
        }
        if close {
            self.ui.menu = None;
        }
    }
}


impl eframe::App for App {
    /// Grundfarbe des Fensters.
    ///
    /// eframe gibt hier planmäßig ein dunkelgraues, halbtransparentes Grau vor
    /// (Alpha 180) — es ist nur als Unterlage für Schatten gedacht. Bleibt bei
    /// krummer Bildschirmskalierung zwischen zwei Flächen ein Pixel unbedeckt,
    /// schimmert dort dieser Grund durch und erscheint als helle Fuge. Deckend
    /// in Fensterfarbe ist die Fuge unsichtbar.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // T0147: Ohne Titelleiste liegt der Fensterschatten im durchsichtigen
        // Rand — dort darf kein deckender Grund stehen, sonst bliebe vom
        // Schatten nur ein dunkler Rahmen. Der Grund muss dann **schwarz** mit
        // Alpha 0 sein: der Puffer ist vormultipliziert, ein Grund in
        // Fensterfarbe mit Alpha 0 legte diese Farbe über den Hintergrund
        // (gemessen: der Rand erschien als dunkler Rahmen statt durchsichtig).
        // T0177: Maßgeblich ist der Rand **dieses** Bildes (`raw_input_hook`
        // setzt ihn) — ist er abgeschaltet oder liegt das Fenster angedockt am
        // Bildschirmrand, deckt die Oberfläche das Fenster und der Grund ist
        // wieder Fensterfarbe.
        if self.ui.fenster_aussen.is_some() && self.prefs.fensterrahmen.ohne_titelleiste() {
            return [0.0, 0.0, 0.0, 0.0];
        }
        let bg = Palette::bg();
        [
            bg.r() as f32 / 255.0,
            bg.g() as f32 / 255.0,
            bg.b() as f32 / 255.0,
            1.0,
        ]
    }

    /// T0147: Ohne Titelleiste liegt die Oberfläche im Schattenrand.
    ///
    /// Der kleinere Bildschirmausschnitt muss **vor** dem Bild gesetzt werden:
    /// `PassState::available_rect` — die Fläche, in der die Panels liegen —
    /// entsteht beim Beginn des Bildes aus `RawInput::screen_rect`. Ein späteres
    /// Setzen erreichte nur `InputState`; die Panels blieben dann auf der ganzen
    /// Fensterfläche und die Zeichnung liefe unter den Schatten.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.ui.fenster_aussen = None;
        // T0177: Der Schatten lässt sich abschalten — ohne ihn füllt die
        // Oberfläche das Fenster ganz, und das Fenstermanagement rechnet keinen
        // Streifen mit (Andocken an den Bildschirmrand ohne Abstand).
        if !self.schatten_im_rand
            || !self.prefs.fensterschatten
            || !self.fenster_ohne_titelleiste()
        {
            return;
        }
        let Some(aussen) = raw_input.screen_rect else {
            return;
        };
        let (vollbild, maximiert, monitor) = raw_input
            .viewports
            .get(&egui::ViewportId::ROOT)
            .map(|v| {
                (
                    v.fullscreen.unwrap_or(false),
                    v.maximized.unwrap_or(false),
                    v.monitor_size,
                )
            })
            .unwrap_or((false, false, None));
        // Vollbild und maximiert füllen den Bildschirm — dort gibt es keinen
        // Rand, in dem ein Schatten läge.
        if vollbild || maximiert || self.ui.fullscreen {
            return;
        }
        // T0177: Ein angedocktes Fenster liegt am Bildschirmrand. Ein Rand dort
        // wäre ein Abstand zur Kante — und die Oberfläche wäre um seine Breite
        // kleiner als die Fläche, die der Nutzer angedockt hat.
        if fenster_angedockt(aussen.size(), monitor) {
            return;
        }
        let innen = aussen.shrink(SCHATTEN_RAND);
        if innen.width() <= 0.0 || innen.height() <= 0.0 {
            return;
        }
        raw_input.screen_rect = Some(innen);
        self.ui.fenster_aussen = Some(aussen);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Leistungsanzeige (T0068): Bild beginnen — Abstand zum vorigen Bild und
        // die Abschnittsmarken messen. Das läuft immer mit; ein paar
        // Uhrablesungen je Bild kosten nichts.
        self.perf.bild_beginnen();
        // T0135: Aufträge einer zweiten Instanz abholen — Zeichnung als Reiter
        // öffnen oder das Fenster nach vorn holen.
        let auftraege = self
            .instanz
            .as_ref()
            .map(|d| d.abholen())
            .unwrap_or_default();
        let jetzt = ctx.input(|i| i.time);
        self.auftraege_bearbeiten(ctx, auftraege, jetzt);
        // T0224: Ein Auftrag von außen holt das Fenster nach vorn; der Wunsch
        // wird über einige Bilder nachgehalten, bis das Fenster den Fokus hat.
        if let Some(token) = self.nach_vorn_betreuen(ctx, jetzt) {
            // Wayland: Ohne `xdg-activation` bleibt `Focus` wirkungslos —
            // das Token des Aufrufers ist der einzige Weg nach vorn. Ein
            // Fehlschlag ist harmlos, der Fokuswunsch läuft daneben weiter.
            if let Err(e) = crate::aktivieren::aktiviere(frame, &token) {
                eprintln!("Aktivierung: {e}");
            }
        }
        // T0102: Bei der Wahl „System“ folgt das Design dem Betriebssystem auch
        // im Betrieb (Wechsel hell ↔ dunkel am Abend). `system_nachziehen`
        // sieht höchstens alle zwei Sekunden nach.
        if self.prefs.design == jjcad_core::persist::Design::System {
            theme::system_nachziehen(ctx);
        }
        self.ui(ctx);
        self.draw_menu(ctx);
        // T0129: Sicherung — Stände anlegen, wenn das Intervall abgelaufen ist.
        // Der Weckruf hält die Prüfung auch ohne Eingabe am Laufen.
        self.sicherung_tick(jetzt);
        let intervall = (self.prefs.sicherung_einstellungen().intervall_min.max(1) as f64) * 60.0;
        let rest = (intervall - (jetzt - self.sicherung_zuletzt)).max(1.0);
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(rest));
        if self.started_at < 3.0 {
            // Die ersten Frames: Layout einschwingen lassen, dann einpassen
            self.started_at += 1.0;
            if self.started_at >= 2.0 {
                self.fit_view();
            }
            ctx.request_repaint();
        }
        // Meldungen verblassen in Echtzeit: solange etwas sichtbar ist,
        // weiterzeichnen (der Chat fordert das selbst an).
        if self.chat.braucht_repaint(ctx.input(|i| i.time)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    
        // Auswahl hat sich geändert: ein weiteres Frame zeichnen, damit das
        // Eigenschaften-Panel (wird vor der Zeichenfläche gezeichnet) nachzieht.
        let mut stamp: u64 = 1469598103934665603;
        for id in self.selection() {
            stamp ^= *id as u64;
            stamp = stamp.wrapping_mul(1099511628211);
        }
        if stamp != self.ui.sel_stamp {
            self.ui.sel_stamp = stamp;
            ctx.request_repaint();
        }
        // Offene Leistungsanzeige: die Zahlen einmal je Sekunde nachziehen, auch
        // wenn sonst nichts passiert. Solche Pausenbilder verlassen den Verlauf
        // (`perf::PAUSE_MS`), die Perzentile bleiben also sauber.
        if self.perf.an {
            ctx.request_repaint_after(std::time::Duration::from_millis(1000));
        }
        // Selbstdiagnose: F12 (oder Start mit JJCAD_UI_DIAG=1) hält das Bild
        // samt Kennwerten fest, damit sich Anzeigefehler fremder Rechner
        // zuordnen lassen.
        self.diag.tasten(ctx);
        self.diag.von_selbst(ctx, ctx.input(|i| i.time));
        if let Some(ergebnis) = self.diag.einsammeln(ctx, frame.gl()) {
            self.status_msg = match ergebnis {
                Ok(pfad) => format!("Diagnose gespeichert: {}", pfad.display()),
                Err(e) => format!("Diagnose fehlgeschlagen: {e}"),
            };
        }
        self.perf.bild_beenden();
}

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.prefs.show_layers = self.ui.layers_open;
        self.prefs.show_properties = self.ui.props_open;
        self.prefs.layers_side = self.ui.layers_side;
        self.prefs.props_side = self.ui.props_side;
        self.prefs.show_blattmanager = self.ui.blatt_open;
        self.prefs.blatt_side = self.ui.blatt_side;
        self.prefs.show_lineweights = self.ui.lineweights;
        self.prefs.lineweights_scaled = self.ui.lineweights_scaled;
        self.prefs.lineweight_factor = self.ui.lineweight_factor;
        let _ = self.prefs.save();
        // T0130: Reiter, Ansichten und Fensterlage für den nächsten Start
        // merken. Beim Beenden wird **nicht** nach dem Speichern gefragt — die
        // Arbeit steckt im Sicherungspool und in der Sitzung. Ein Demo-Start
        // schreibt nichts: seine Reiter sind Prüfzustand, nicht Arbeit.
        if ist_demo_start() {
            return;
        }
        // T0129: ungespeicherte Änderungen zuerst als Stand ablegen — sonst
        // findet der nächste Start nur den Stand von vor dem letzten Intervall.
        self.sicherung_jetzt();
        if let Err(e) = self.sitzung_schreiben() {
            eprintln!("Sitzung nicht gespeichert: {e}");
        }
    }
}

/// Neuester Stand mit **ungespeicherter Arbeit**, wenn er vom Inhalt der
/// Zieldatei abweicht (T0129).
///
/// `None` heißt: die Datei gilt. Ein Stand, der der Datei entspricht
/// (`gesichert`, geschrieben nach dem Speichern oder nach „Verwerfen“), zählt
/// nie — sonst käme verworfene Arbeit zurück. Ebenso zählt ein als
/// `verworfen` markierter Stand nie (R3.5): er bleibt auffindbar, kehrt aber
/// nicht von selbst zurück.
fn ungespeicherter_stand(
    pool: &jjcad_core::sicherung::Sicherung,
    id: &str,
    pfad: &Path,
) -> Option<(jjcad_core::sicherung::Stand, jjcad_core::model::Document)> {
    let stand = pool.neuester(id)?.clone();
    if stand.gesichert || stand.verworfen {
        return None;
    }
    let datei_hash = std::fs::read(pfad)
        .ok()
        .map(|b| jjcad_core::sicherung::hash_hex(&b));
    if datei_hash.as_deref() == Some(stand.hash.as_str()) {
        return None;
    }
    let doc = pool.laden(&stand).ok()?;
    Some((stand, doc))
}

/// Zeitstempel eines Standes für die Anzeige: `2026-11-08T12:33:29` →
/// `08.11.2026 12:33`. Unbekannte Formen bleiben, wie sie sind.
pub fn kurz_zeit(zeit: &str) -> String {
    let (datum, uhr) = match zeit.split_once('T') {
        Some((d, u)) => (d, u),
        None => return zeit.to_string(),
    };
    let teile: Vec<&str> = datum.split('-').collect();
    if teile.len() != 3 {
        return zeit.to_string();
    }
    let hm: String = uhr.chars().take(5).collect();
    format!("{}.{}.{} {hm}", teile[2], teile[1], teile[0])
}

/// Dateiendungen für Dialoge.
pub fn ext_for(mode: FileMode) -> &'static str {
    // Eine Quelle: die Endung steht in `FileMode::ext` (T0076).
    mode.ext()
}

pub fn color_to_egui(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

pub fn egui_to_color(c: Color32) -> Color {
    Color::rgb(c.r(), c.g(), c.b())
}

pub fn layer_color_label(c: Color) -> String {
    c.to_hex()
}

pub fn entity_kind_icon(kind: &EntityKind) -> &'static str {
    match kind {
        EntityKind::Point { .. } => "point",
        EntityKind::Line { .. } => "line",
        EntityKind::Circle { .. } => "circle",
        EntityKind::Arc { .. } => "arc",
        EntityKind::Ellipse { .. } => "ellipse",
        EntityKind::EllipseArc { .. } => "arc",
        EntityKind::Polyline { .. } => "polyline",
        EntityKind::Spline { .. } => "spline",
        EntityKind::Path { .. } => "polyline",
        EntityKind::Fill { .. } => "fill",
        EntityKind::Volume { .. } => "volume",
        EntityKind::Text { .. } => "text",
        EntityKind::Dim { .. } => "dim",
        EntityKind::DimRadius { .. } => "radius",
        EntityKind::ElevationPlan { .. } => "elevation",
        EntityKind::ElevationSection { .. } => "elevation-section",
        EntityKind::Room { .. } => "room",
        EntityKind::Wall { .. } => "wall",
        EntityKind::Door { .. } => "door",
        EntityKind::Window { .. } => "window",
        EntityKind::Stair { .. } => "stair",
        EntityKind::BlockRef { .. } => "block",
        EntityKind::Image { .. } => "image",
        EntityKind::Viewport(_) => "viewport",
    }
}

#[allow(dead_code)]
fn fmt_len(mm: f64, unit: Unit) -> String {
    format!("{} {}", format_number(unit.from_mm(mm), unit.decimals().max(1)), unit.short())
}

#[allow(dead_code)]
fn unused(_: RasterOptions, _: persist::HotbarPreset) {}

impl App {
    /// Startet ein Füllwerkzeug mit passender Muster-Vorgabe.
    pub fn start_fill_input(&mut self, mode: FillInput, kind: FillKind) {
        self.options.fill_pattern = kind.pattern();
        self.start_tool(kind.action(mode));
        self.ui.fill_active = Some(kind);
    }

    /// Geteilte Ribbon-Schaltfläche für Füllflächen, Schraffuren und Volumen.
    /// Wählt den Einfügemodus eines Werkzeugs und startet es.
    ///
    /// Ein Weg für Ribbon-Menü und Tests: der Modus steht in `ToolOptions`
    /// (dieselbe Quelle, die auch das Eigenschaften-Panel zeigt).
    pub fn waehle_einfuegemodus(&mut self, tool_id: &str, modus: Einfuegemodus) {
        *self.options.einfuegemodus_mut(tool_id) = modus;
        self.start_tool(tool_id);
    }

    /// Wählt den Eingabemodus des Bogens und startet das Werkzeug.
    pub fn waehle_bogenmodus(&mut self, modus: Bogenmodus) {
        self.options.arc_mode = modus;
        self.start_tool("draw.arc");
    }

    /// Geteilte Schaltfläche mit einer Liste von Eingabemodi.
    ///
    /// Der Hauptknopf startet das Werkzeug im zuletzt gewählten Modus, der
    /// Pfeil öffnet die Modi. Ein Weg für Ribbon-Menü und Tests: der Modus
    /// steht in `ToolOptions` (dieselbe Quelle, die auch das
    /// Eigenschaften-Panel zeigt).
    fn modus_group_button(
        &mut self,
        ui: &mut egui::Ui,
        tool_id: &'static str,
        icon: &str,
        label: &str,
        cur: usize,
        items: &[&str],
        demo: &[&str],
    ) -> Option<usize> {
        let active = self.tool_id() == Some(tool_id);
        let (main, arrow) = widgets::ribbon_split_button(ui, icon, label, active);
        let popup_id = ui.make_persistent_id(("moduspop", tool_id));
        let now = ui.input(|i| i.time);
        if arrow.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
            ui.memory_mut(|m| m.data.insert_temp(popup_id.with("opened"), now));
        } else if main.clicked() {
            self.start_tool(tool_id);
        }
        // Prüfhilfe: Menü für einen Screenshot offen halten (nur mit Env-Variable)
        if let Ok(welches) = std::env::var("JJCAD_DEMO_MENU") {
            if demo.contains(&welches.as_str()) {
                ui.memory_mut(|m| m.open_popup(popup_id));
            }
        }
        // Der Klick, der das Menü öffnet, darf nicht sofort einen Eintrag wählen
        let geoeffnet = ui
            .memory(|m| m.data.get_temp::<f64>(popup_id.with("opened")).unwrap_or(0.0));
        let bereit = now - geoeffnet > 0.25;
        let mut gewaehlt: Option<usize> = None;
        egui::popup_below_widget(
            ui,
            popup_id,
            &arrow,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(230.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                for (i, name) in items.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width().max(226.0), 26.0),
                        egui::Sense::click(),
                    );
                    if resp.hovered() || i == cur {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(4.0),
                            Palette::panel_alt(),
                        );
                    }
                    let farbe = if i == cur { Palette::accent() } else { Palette::text() };
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 10.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        *name,
                        egui::FontId::proportional(12.5),
                        farbe,
                    );
                    if bereit && resp.clicked() {
                        gewaehlt = Some(i);
                    }
                }
            },
        );
        if gewaehlt.is_some() {
            ui.memory_mut(|mm| mm.close_popup());
        }
        gewaehlt
    }

    /// Geteilte Schaltfläche für Rechteck und Ellipse.
    ///
    /// Der Hauptknopf startet das Werkzeug im zuletzt gewählten Modus, der
    /// Pfeil öffnet die beiden Einfügemodi. Ohne dieses Menü war der diagonale
    /// Modus nur im Eigenschaften-Panel zu finden — und dort erst, wenn das
    /// Werkzeug schon lief.
    fn einfuege_group_button(&mut self, ui: &mut egui::Ui, tool_id: &'static str) {
        let ellipse = tool_id == "draw.ellipse";
        let (icon, label) = if ellipse {
            ("ellipse", jjcad_core::i18n::tr("gui.props.ellipse"))
        } else {
            ("rect", jjcad_core::i18n::tr("gui.props.rechteck"))
        };
        let cur = self.options.einfuegemodus(tool_id);
        let idx = Einfuegemodus::ALL
            .iter()
            .position(|m| *m == cur)
            .unwrap_or(0);
        let items: Vec<&str> = Einfuegemodus::ALL.iter().map(|m| m.label(tool_id)).collect();
        let demo: &[&str] = if ellipse {
            &["ellipse"]
        } else {
            &["rect", "rechteck"]
        };
        if let Some(i) = self.modus_group_button(ui, tool_id, icon, label, idx, &items, demo) {
            self.waehle_einfuegemodus(tool_id, Einfuegemodus::ALL[i]);
        }
    }

    /// Geteilte Schaltfläche für den Bogen: drei Punkte oder Mittelpunkt,
    /// Startpunkt und Endpunkt (T0085).
    fn bogen_group_button(&mut self, ui: &mut egui::Ui) {
        let cur = self.options.arc_mode;
        let idx = Bogenmodus::ALL.iter().position(|m| *m == cur).unwrap_or(0);
        let items: Vec<&str> = Bogenmodus::ALL.iter().map(|m| m.label()).collect();
        if let Some(i) =
            self.modus_group_button(ui, "draw.arc", "arc", jjcad_core::i18n::tr("gui.props.bogen"), idx, &items, &["bogen", "arc"])
        {
            self.waehle_bogenmodus(Bogenmodus::ALL[i]);
        }
    }

    /// Startet die Maßkette im gewünschten Modus — ein Weg für Ribbon und
    /// Demo-Zustände. Der Modus steckt im Werkzeug (`DimModus::werkzeug`),
    /// nicht in einer zweiten Einstellung daneben.
    pub(crate) fn waehle_dim_modus(&mut self, modus: DimModus) {
        self.options.dim_axis = Some(modus.axis());
        self.start_tool(modus.werkzeug());
    }

    fn fill_group_button(&mut self, ui: &mut egui::Ui, kind: FillKind) {
        let hatch = kind == FillKind::Hatch;
        let cur = match kind {
            FillKind::Hatch => self.ui.hatch_input,
            FillKind::Volume => self.ui.volume_input,
            FillKind::Area => self.ui.fill_input,
        };
        let active =
            self.tool_id() == Some(kind.action(cur)) && self.ui.fill_active == Some(kind);
        let label = kind.label();
        let (main, arrow) = widgets::ribbon_split_button(ui, kind.button_icon(cur), label, active);
        let popup_id = ui.make_persistent_id(("fillpop", label));
        let now = ui.input(|i| i.time);
        if arrow.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
            ui.memory_mut(|m| m.data.insert_temp(popup_id.with("opened"), now));
        } else if main.clicked() {
            self.start_fill_input(cur, kind);
        }
        // Prüfhilfe: Menü für einen Screenshot offen halten (nur mit Env-Variable)
        if let Ok(welches) = std::env::var("JJCAD_DEMO_MENU") {
            let passt = match welches.as_str() {
                "hatch" => kind == FillKind::Hatch,
                "volume" | "volumen" => kind == FillKind::Volume,
                "area" | "flaeche" | "fill" => kind == FillKind::Area,
                // Andere Werte gehören den übrigen Menüs (z. B. „rect“ für den
                // Einfügemodus) — sonst stünde beim Screenshot ein zweites
                // Menü offen (T0024, Turn 2).
                _ => false,
            };
            if passt {
                ui.memory_mut(|m| m.open_popup(popup_id));
            }
        }
        // Der Klick, der das Menü öffnet, darf nicht sofort einen Eintrag wählen
        let geoeffnet = ui.memory(|m| m.data.get_temp::<f64>(popup_id.with("opened")).unwrap_or(0.0));
        let bereit = now - geoeffnet > 0.25;
        egui::popup_below_widget(
            ui,
            popup_id,
            &arrow,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(200.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                for m in FillInput::ALL {
                    // Zeile mit Icon: gleiches Bild wie der Knopf, damit man den
                    // Modus auch im Menü am Symbol erkennt
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width().max(196.0), 26.0),
                        egui::Sense::click(),
                    );
                    if resp.hovered() || m == cur {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(4.0),
                            Palette::panel_alt(),
                        );
                    }
                    crate::icons::paint(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::Pos2::new(rect.left() + 16.0, rect.center().y),
                            egui::Vec2::splat(18.0),
                        ),
                        m.icon(hatch),
                        if m == cur { Palette::accent() } else { Palette::text() },
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        m.label(),
                        egui::FontId::proportional(12.5),
                        if m == cur { Palette::accent() } else { Palette::text() },
                    );
                    if bereit && resp.clicked() {
                        match kind {
                            FillKind::Hatch => {
                                self.ui.hatch_input = m;
                                self.prefs.hatch_input = m.key().to_string();
                            }
                            FillKind::Volume => {
                                self.ui.volume_input = m;
                                self.prefs.volume_input = m.key().to_string();
                            }
                            FillKind::Area => {
                                self.ui.fill_input = m;
                                self.prefs.fill_input = m.key().to_string();
                            }
                        }
                        let _ = self.prefs.save();
                        self.start_fill_input(m, kind);
                        ui.memory_mut(|mm| mm.close_popup());
                    }
                }
            },
        );
    }

    /// Startet die gewählte Kopierfunktion.
    ///
    /// „Kopieren" füllt nur die Zwischenablage (kein Werkzeug), „Kopieren mit
    /// Basispunkt" startet `modify.copy` — dieselbe Action wie Tastatur, MCP und
    /// Kontextmenü.
    pub fn start_copy_input(&mut self, mode: CopyInput) {
        match mode {
            CopyInput::Clipboard => {
                let ctx = self.ctx.clone();
                self.run_action(&ctx, "edit.copy");
            }
            CopyInput::BasePoint => self.start_tool("modify.copy"),
        }
    }

    /// Geteilte Schaltfläche „Kopieren" (zwei Kopierfunktionen).
    ///
    /// Gleiche Bedienung wie bei den Flächen: Hauptfläche startet den zuletzt
    /// benutzten Modus, der Pfeil öffnet die Auswahl. Das Icon zeigt den Modus —
    /// „Kopieren mit Basispunkt" trägt unten links einen Punkt.
    /// Polylinie und Pfad auf **einem** Platz (T0229): der Knopf startet den
    /// zuletzt benutzten, der Pfeil öffnet die Wahl. Zwei eigene Knöpfe
    /// überschritten die Mindestfensterbreite des Reiters „Zeichnen".
    fn polyline_group_button(&mut self, ui: &mut egui::Ui) {
        let cur = self.tool_id() == Some("draw.path");
        let aktiv = cur || self.tool_id() == Some("draw.polyline");
        let label = aktion_label(if cur { "draw.path" } else { "draw.polyline" });
        let (main, arrow) = widgets::ribbon_split_button(ui, "polyline", label, aktiv);
        let popup_id = ui.make_persistent_id("pfadpop");
        let now = ui.input(|i| i.time);
        if arrow.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
            ui.memory_mut(|m| m.data.insert_temp(popup_id.with("opened"), now));
        } else if main.clicked() {
            self.start_tool(if cur { "draw.path" } else { "draw.polyline" });
        }
        // Der Klick, der das Menü öffnet, darf nicht sofort einen Eintrag wählen
        let geoeffnet = ui.memory(|m| m.data.get_temp::<f64>(popup_id.with("opened")).unwrap_or(0.0));
        let bereit = now - geoeffnet > 0.25;
        egui::popup_below_widget(
            ui,
            popup_id,
            &arrow,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(260.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                for (id, icon) in [("draw.polyline", "polyline"), ("draw.path", "polyline")] {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width().max(256.0), 26.0),
                        egui::Sense::click(),
                    );
                    let gewaehlt = self.tool_id() == Some(id);
                    if resp.hovered() || gewaehlt {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(4.0),
                            Palette::panel_alt(),
                        );
                    }
                    crate::icons::paint(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::Pos2::new(rect.left() + 16.0, rect.center().y),
                            egui::Vec2::splat(18.0),
                        ),
                        icon,
                        if gewaehlt { Palette::accent() } else { Palette::text() },
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        aktion_label(id),
                        egui::FontId::proportional(12.5),
                        if gewaehlt { Palette::accent() } else { Palette::text() },
                    );
                    if resp.clicked() && bereit {
                        self.start_tool(id);
                        ui.memory_mut(|m| m.close_popup());
                    }
                }
            },
        );
    }

    fn copy_group_button(&mut self, ui: &mut egui::Ui) {
        let cur = self.ui.copy_input;
        let aktiv = self.tool_id() == Some(cur.action());
        let (main, arrow) =
            widgets::ribbon_split_button(ui, cur.icon(), rb("ui.copyinput.kopieren"), aktiv);
        let popup_id = ui.make_persistent_id("copypop");
        let now = ui.input(|i| i.time);
        if arrow.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
            ui.memory_mut(|m| m.data.insert_temp(popup_id.with("opened"), now));
        } else if main.clicked() {
            self.start_copy_input(cur);
        }
        // Prüfhilfe: Menü für einen Screenshot offen halten (nur mit Env-Variable)
        if let Ok(welches) = std::env::var("JJCAD_DEMO_MENU") {
            if welches == "copy" {
                ui.memory_mut(|m| m.open_popup(popup_id));
            }
        }
        // Der Klick, der das Menü öffnet, darf nicht sofort einen Eintrag wählen
        let geoeffnet = ui.memory(|m| m.data.get_temp::<f64>(popup_id.with("opened")).unwrap_or(0.0));
        let bereit = now - geoeffnet > 0.25;
        egui::popup_below_widget(
            ui,
            popup_id,
            &arrow,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(320.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                for m in CopyInput::ALL {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width().max(316.0), 26.0),
                        egui::Sense::click(),
                    );
                    if resp.hovered() || m == cur {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(4.0),
                            Palette::panel_alt(),
                        );
                    }
                    crate::icons::paint(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::Pos2::new(rect.left() + 16.0, rect.center().y),
                            egui::Vec2::splat(18.0),
                        ),
                        m.icon(),
                        if m == cur { Palette::accent() } else { Palette::text() },
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        m.label(),
                        egui::FontId::proportional(12.5),
                        if m == cur { Palette::accent() } else { Palette::text() },
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.right() - 8.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        m.key_label(),
                        egui::FontId::proportional(11.0),
                        Palette::text_faint(),
                    );
                    if bereit && resp.clicked() {
                        self.ui.copy_input = m;
                        self.prefs.copy_input = m.key().to_string();
                        let _ = self.prefs.save();
                        self.start_copy_input(m);
                        ui.memory_mut(|mm| mm.close_popup());
                    }
                }
            },
        );
    }

    /// Lässt sich dieses Objekt durch Ziehen mit der Maus verschieben?
    ///
    /// Entscheidet die Einstellung „Maus verschiebt“ (Einstellungen →
    /// Bedienung, `prefs.mouse_move`, Vorgabe an). Sie gilt für **alle**
    /// Objektarten gleich — auch Text und Treppe (T0063). Ausgeschaltet wandert
    /// ausschließlich das Werkzeug „Verschieben“; das ist der genaue Weg für
    /// professionelles Arbeiten.
    pub fn maus_verschiebt(&self, id: Id) -> bool {
        self.prefs.mouse_move && self.doc().entity(id).is_some()
    }

    /// Die **Ebene**, die gerade gehalten wird (T0215) — Umschalt, Strg, Alt
    /// und ihre Kombinationen. Sie schaltet die Hotbar um und wird als kleines
    /// Feld neben ihr angezeigt.
    ///
    /// Gelesen wird der Zustand der Tastatur in **diesem** Bild, nicht ein
    /// gemerkter Wert: die Hotbar folgt der Hand, und beim Loslassen steht
    /// wieder die Grundbelegung da. `Shift` ist dabei dieselbe Ebene wie im
    /// Tastenbild (`shift+c`) — die frühere „zweite Ebene" ist eine Variante
    /// unter acht.
    pub fn hotbar_kombination(&self, ctx: &egui::Context) -> jjcad_core::persist::Kombination {
        ctx.input(|i| {
            jjcad_core::persist::Kombination::aus_modifikatoren(
                i.modifiers.shift,
                i.modifiers.command,
                i.modifiers.alt,
            )
        })
    }

    /// Beginnt das Maus-Verschieben nur, wenn die Einstellung es zulässt.
    ///
    /// Liefert `false`, wenn der Zug nicht startet — dann läuft die übrige
    /// Zeigerauswertung weiter, statt die Auswahl zu verschieben.
    pub fn begin_move_drag_bei_klick(&mut self, hit: Id, at: V2, frame: f64) -> bool {
        if !self.maus_verschiebt(hit) {
            return false;
        }
        self.begin_move_drag(at, frame);
        self.move_drag.is_some()
    }

    /// Beginnt das Verschieben der Auswahl mit der Maus (Undo-Snapshot einmalig).
    pub fn begin_move_drag(&mut self, start: V2, frame: f64) {
        let ids = self.selection().to_vec();
        if ids.is_empty() {
            return;
        }
        let orig: Vec<jjcad_core::model::Entity> = ids
            .iter()
            .filter_map(|id| self.doc().entity(*id).cloned())
            .collect();
        self.move_drag = Some(MoveDrag {
            start,
            cur: start,
            frame,
            orig,
            recorded: false,
        });
        self.mitteilung(jjcad_core::i18n::tr("gui.app.verschieben_basispunkt_klicken"));
    }

    /// Verschiebt die Auswahl live mit (Ausgangsgeometrie bleibt gespeichert).
    pub fn update_move_drag(&mut self, p: V2) {
        let Some(md) = self.move_drag.as_ref() else {
            return;
        };
        let d = p - md.start;
        let orig = md.orig.clone();
        let schon = md.recorded;
        if !schon && d.len() < 0.5 {
            return;
        }
        if !schon {
            let snap = self.doc().clone();
            self.session_mut().history.record(&snap);
        }
        if let Some(md) = self.move_drag.as_mut() {
            md.cur = p;
            md.recorded = true;
        }
        for e in orig {
            let moved = jjcad_core::transform::transform_entity(
                &e,
                &jjcad_core::geom::Affine::translate(d),
            );
            if let Some(dst) = self.doc_mut().entity_mut(e.id) {
                *dst = moved;
            }
        }
        // Türen und Fenster bleiben auf ihrer Wand (T0191): der freie Zug
        // schreibt die Objekte unmittelbar, ohne die Action-Schicht — ohne
        // diesen Schritt wanderte eine Tür samt Basispunkt von der Wandkante
        // weg. Quer zur Wand rührt sich die Tür dadurch nicht mehr, längs
        // wandert sie wie bisher (an den Wandenden begrenzt).
        self.session_mut().oeffnungen_auf_wand();
        // Displayliste neu aufbauen: sonst zeigt die Zeichenfläche beim Ziehen
        // den alten Stand (die Vorschau klebt am Zeiger statt mitzuwandern).
        self.mark_changed();
    }

    /// Verwirft eine begonnene Maus-Verschiebung ohne Meldung.
    ///
    /// Der zweite Druck eines Doppelklicks auf einem Text startet seit T0063
    /// den Mauszug; der Klick gehört aber der Texteingabe. Chat-Meldung und
    /// „abgebrochen“ wären hier falsch, es war kein Zug des Nutzers.
    pub fn verwerfe_move_drag(&mut self) {
        let Some(md) = self.move_drag.take() else {
            return;
        };
        for e in &md.orig {
            if let Some(dst) = self.doc_mut().entity_mut(e.id) {
                *dst = e.clone();
            }
        }
        self.mark_changed();
    }

    pub fn end_move_drag(&mut self) {
        if self.move_drag.take().is_some() {
            // T0191: die abschließende Lage noch einmal auf die Wand setzen —
            // der letzte Zugframe kann einen Zwischenstand tragen.
            self.session_mut().oeffnungen_auf_wand();
            self.mark_changed();
            self.mitteilung(jjcad_core::i18n::tr("gui.app.auswahl_verschoben"));
        }
    }

    /// Bricht eine laufende Maus-Verschiebung ab und stellt die Ausgangslage her
    /// (Esc während des Ziehens).
    pub fn cancel_move_drag(&mut self) {
        let Some(md) = self.move_drag.take() else {
            return;
        };
        for e in &md.orig {
            if let Some(dst) = self.doc_mut().entity_mut(e.id) {
                *dst = e.clone();
            }
        }
        self.mark_changed();
        self.mitteilung(jjcad_core::i18n::tr("gui.app.verschieben_abgebrochen"));
    }
}

#[cfg(test)]
mod tests_treppe {
    //! Die Treppe verhält sich wie ein Rechteck (T0027, Turn 3): Griffe an den
    //! Kanten, aber kein Verschieben mit der gedrückten Maustaste.

    use super::*;

    fn app_mit_treppe() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        let r = app
            .session_mut()
            .execute(
                "draw.stair",
                &serde_json::json!({"a": [0.0, 0.0], "b": [0.0, 4000.0], "width": 1000.0}),
            )
            .expect("Treppe anlegen");
        let id = r["created"][0].as_u64().unwrap();
        (app, id)
    }

    /// Mit ausgeschaltetem „Maus verschiebt“ (Einstellungen → Bedienung) startet
    /// ein Klick in die Treppenfläche kein Verschieben — sonst wandert sie beim
    /// Auswählen versehentlich mit. Die Treppe ist damit keine Sonderregel mehr:
    /// sie folgt derselben Einstellung wie jedes andere Objekt (T0063).
    #[test]
    fn abgeschaltet_verschiebt_der_klick_die_treppe_nicht() {
        let (mut app, id) = app_mit_treppe();
        app.prefs.mouse_move = false;
        app.session_mut().select(vec![id]);
        assert!(!app.maus_verschiebt(id), "Treppe hing am Mauszug");
        assert!(!app.begin_move_drag_bei_klick(id, V2::ZERO, 0.0));
        assert!(app.move_drag.is_none(), "Verschieben wurde gestartet");
    }

    /// Eingeschaltet lässt sich auch die Treppe mit der Maus ziehen; die
    /// Kantengriffe bleiben der genauere Weg, sind aber keine Bedingung.
    #[test]
    fn eingeschaltet_verschiebt_der_klick_die_treppe() {
        let (mut app, id) = app_mit_treppe();
        // Der Test prüft die eingeschaltete Einstellung, nicht die Vorgabe des
        // Rechners — eine gespeicherte `prefs.json` darf ihn nicht verfälschen.
        app.prefs.mouse_move = true;
        assert!(app.prefs.mouse_move, "Vorgabe ist eingeschaltet");
        app.session_mut().select(vec![id]);
        assert!(app.maus_verschiebt(id));
        assert!(app.begin_move_drag_bei_klick(id, V2::ZERO, 0.0));
        assert!(app.move_drag.is_some(), "Verschieben lief nicht an");
    }

    /// Die vier Kantengriffe brauchen eine Kantenrichtung — ohne sie zeichnet
    /// `draw_grips` keinen Griff (die Treppe ließ sich ziehen, ohne dass man
    /// die Punkte sah).
    #[test]
    fn treppe_hat_vier_sichtbare_kantengriffe() {
        let (app, id) = app_mit_treppe();
        let e = app.doc().entity(id).unwrap();
        let griffe = crate::handles::grips_of(e);
        assert_eq!(griffe.len(), 4, "vier Kantenmitten");
        for (art, _) in griffe {
            let crate::handles::GripKind::Edge(i) = art else {
                panic!("Treppengriff ist keine Kante: {art:?}");
            };
            assert!(
                crate::handles::edge_dir(e, i).is_some(),
                "Kante {i} ohne Richtung — der Griff bliebe unsichtbar"
            );
        }
    }
}

#[cfg(test)]
mod tests_reihenfolge {
    //! Die Ribbon-Knöpfe der Zeichenreihenfolge rufen die vorhandenen
    //! Funktionen auf (keine zweite Logik). Geprüft wird das Ergebnis in der
    //! Objektliste — die Liste IST die Zeichenreihenfolge.

    use super::*;

    fn app_mit_drei_linien() -> (App, Vec<Id>) {
        let mut app = App::with_ctx(egui::Context::default());
        for k in 0..3 {
            app.session_mut()
                .execute(
                    "draw.line",
                    &serde_json::json!({"a": [k as f64, 0.0], "b": [k as f64, 1.0]}),
                )
                .expect("Linie anlegen");
        }
        let ids: Vec<Id> = app.doc().entities.iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), 3);
        (app, ids)
    }

    fn reihenfolge(app: &App) -> Vec<Id> {
        app.doc().entities.iter().map(|e| e.id).collect()
    }

    /// Knopf „Vordergrund“: die Auswahl wird zum letzten Element der Liste.
    #[test]
    fn vordergrund_schiebt_auswahl_nach_vorn() {
        let (mut app, ids) = app_mit_drei_linien();
        app.session_mut().select(vec![ids[0]]);
        app.order_selection_to(true);
        assert_eq!(reihenfolge(&app), vec![ids[1], ids[2], ids[0]]);
        assert_eq!(app.status_msg, "Ganz nach vorn");
    }

    /// Knopf „Hintergrund“: die Auswahl wird zum ersten Element der Liste.
    #[test]
    fn hintergrund_schiebt_auswahl_nach_hinten() {
        let (mut app, ids) = app_mit_drei_linien();
        app.session_mut().select(vec![ids[2]]);
        app.order_selection_to(false);
        assert_eq!(reihenfolge(&app), vec![ids[2], ids[0], ids[1]]);
        assert_eq!(app.status_msg, "Ganz nach hinten");
    }

    /// Mehrfachauswahl wandert geschlossen und in ihrer Reihenfolge.
    #[test]
    fn mehrfachauswahl_bleibt_zusammen() {
        let (mut app, ids) = app_mit_drei_linien();
        app.session_mut().select(vec![ids[0], ids[1]]);
        app.order_selection_to(true);
        assert_eq!(reihenfolge(&app), vec![ids[2], ids[0], ids[1]]);
    }

    /// Ohne Auswahl passiert nichts — und der Anwender erfährt warum.
    #[test]
    fn ohne_auswahl_keine_aenderung() {
        let (mut app, ids) = app_mit_drei_linien();
        app.order_selection_to(true);
        assert_eq!(reihenfolge(&app), ids, "Reihenfolge unverändert");
        assert_eq!(app.status_msg, "Auswahl · leer");

        app.order_pick_start(true);
        assert_eq!(app.ui.order_pick, None, "ohne Auswahl kein Zielmodus");
        assert_eq!(app.status_msg, "Reihenfolge · erst ein Element wählen");
    }

    /// Knopf „Vor/Hinter Element“: merkt sich die Richtung für den nächsten
    /// Klick in die Zeichenfläche (die Auswertung sitzt in `canvas`).
    #[test]
    fn vor_element_merkt_richtung() {
        let (mut app, ids) = app_mit_drei_linien();
        app.session_mut().select(vec![ids[0]]);

        app.order_pick_start(true);
        assert_eq!(app.ui.order_pick, Some(true));
        assert!(app.status_msg.contains("VOR"), "{}", app.status_msg);

        app.order_pick_start(false);
        assert_eq!(app.ui.order_pick, Some(false));
        assert!(app.status_msg.contains("HINTER"), "{}", app.status_msg);
    }
}

mod tests_ribbon_bearbeiten {
    //! Der Reiter „Ändern" bietet kein „Auswählen" mehr an (t_2b62367d):
    //! Klicken auf der Zeichenfläche wählt ohnehin aus. Die übrigen Werkzeuge
    //! der Gruppe bleiben unverändert, und der Befehl selbst bleibt für
    //! Tastatur, Inventar und MCP erreichbar.

    use super::RIBBON_BEARBEITEN;

    #[test]
    fn auswaehlen_ist_nicht_im_ribbon() {
        let ids: Vec<&str> = RIBBON_BEARBEITEN.iter().map(|(id, _)| *id).collect();
        assert!(
            !ids.contains(&"modify.select"),
            "Auswählen gehört nicht mehr in den Ribbon: {ids:?}"
        );
        assert_eq!(
            ids,
            [
                "modify.move",
                "modify.copy",
                "modify.rotate",
                "modify.mirror",
                "modify.scale",
            ],
            "übrige Bearbeiten-Werkzeuge unverändert"
        );
        for (id, icon) in RIBBON_BEARBEITEN {
            assert!(!icon.is_empty() && !super::aktion_label(id).is_empty());
        }
    }

    #[test]
    fn auswaehlen_bleibt_als_befehl_erreichbar() {
        // Nur der Ribbon-Eintrag entfällt — die Action bleibt registriert.
        assert!(
            jjcad_core::actions::action("modify.select").is_some(),
            "modify.select muss für Tastatur/Inventar/MCP registriert bleiben"
        );
    }
}

mod tests_ribbon_hoehe {
    //! Alle Ribbon-Reiter sind gleich hoch (t_92b03440).
    //!
    //! Vorher bestimmte der Inhalt die Panelhöhe: der Reiter „Ansicht" (vier
    //! Fangzeilen) und die Trennlinien (`vline` mit voller `RIBBON_H`) wuchsen
    //! über die Panelhöhe hinaus. Der Rahmen wurde dadurch höher, die
    //! Zeichenfläche rutschte beim Reiterwechsel nach unten und zwischen Ribbon
    //! und Zeichenfläche blieb ein ungezeichneter (schwarzer) Streifen.
    //!
    //! Der Test zeichnet die ganze Oberfläche headless (ohne Fenster) und misst
    //! die echte Inhaltshöhe und die Oberkante der Zeichenfläche.

    use super::*;

    /// Zeichnet drei Frames mit fester Fenstergröße und liefert
    /// (Ribbon-Inhaltshöhe, Oberkante der Zeichenfläche).
    fn messen(tab: RibbonTab, breite: f32, hoehe: f32) -> (f32, f32) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.ribbon = tab;
        let mut werte = (0.0, 0.0);
        for _ in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(breite, hoehe),
                )),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
            werte = (app.ui.last_ribbon_content_h, app.ui.last_canvas.top());
        }
        werte
    }

    #[test]
    fn alle_reiter_gleich_hoch_ohne_verschiebung() {
        let mut hoehen = Vec::new();
        let mut kanten = Vec::new();
        for tab in RibbonTab::ALL {
            let (inhalt, kante) = messen(tab, 1600.0, 1000.0);
            hoehen.push((tab.label(), inhalt));
            kanten.push((tab.label(), kante));
        }

        let _soll_kante = HEADER_H + RIBBON_H + DOCTABS_H;
        let erste_kante = kanten[0].1;
        assert!(
            (erste_kante - _soll_kante).abs() < 0.5,
            "Oberkante der Zeichenfläche {erste_kante} statt {_soll_kante} \
             (Kopfzeile {HEADER_H} + Ribbon {RIBBON_H} + Blattreiter {DOCTABS_H})"
        );
        for (name, kante) in &kanten {
            assert!(
                (kante - erste_kante).abs() < 0.5,
                "Reiter {name}: Oberkante der Zeichenfläche {kante} statt {erste_kante} \
                 — beim Reiterwechsel springt die Zeichnung (Höhen: {hoehen:?}, Kanten: {kanten:?})"
            );
        }

        for (name, inhalt) in &hoehen {
            assert!(
                (inhalt - RIBBON_GROUP_H).abs() < 0.5,
                "Reiter {name}: Ribbon-Inhalt {inhalt} px hoch, erwartet {RIBBON_GROUP_H} px \
                 (Höhen aller Reiter: {hoehen:?})"
            );
            assert!(
                *inhalt <= RIBBON_CONTENT_H + 0.01,
                "Reiter {name}: Ribbon-Inhalt {inhalt} px überschreitet den Rahmen \
                 ({RIBBON_CONTENT_H} px) — das erzeugt den schwarzen Balken"
            );
        }
    }

    #[test]
    fn hoehe_bleibt_bei_kleiner_und_grosser_flaeche_gleich() {
        for (breite, hoehe) in [(1024.0, 720.0), (1920.0, 1080.0)] {
            let mut kanten = Vec::new();
            for tab in RibbonTab::ALL {
                let (inhalt, kante) = messen(tab, breite, hoehe);
                assert!(
                    (inhalt - RIBBON_GROUP_H).abs() < 0.5,
                    "{}×{}: Reiter {} Inhalt {inhalt} px statt {RIBBON_GROUP_H} px",
                    breite,
                    hoehe,
                    tab.label()
                );
                kanten.push(kante);
            }
            for kante in &kanten {
                assert!(
                    (kante - kanten[0]).abs() < 0.5,
                    "{breite}×{hoehe}: Zeichenfläche verschiebt sich beim Reiterwechsel \
                     ({kanten:?})"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests_untere_leiste {
    //! T0043: Hotbar und Statuszeile sind **eine** Leiste.
    //!
    //! Vorher standen zwei Panels untereinander (64 px Hotbar + 26 px
    //! Statuszeile); die Statuszeile wiederholte Angaben, die schon im
    //! Eigenschaften-Panel, im Fenstertitel und in den Einstellungen stehen
    //! (Koordinaten, Objektzahl, Build-Kennung, Einheitenwahl). Gemessen wird
    //! die echte Höhe über die Unterkante der Zeichenfläche.

    use super::*;

    fn frame(app: &mut App, ctx: &egui::Context, breite: f32, hoehe: f32) {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(breite, hoehe))),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    #[test]
    fn untere_leiste_ist_ein_element_und_niedriger() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let mut kante = 0.0;
        for _ in 0..3 {
            frame(&mut app, &ctx, 1500.0, 950.0);
            kante = app.ui.last_canvas.bottom();
        }
        assert!(
            (kante - (950.0 - HOTBAR_H)).abs() < 0.5,
            "Zeichenfläche endet bei {kante} px, die untere Leiste ({HOTBAR_H} px) \
             beginnt also nicht bei {} — es gibt mehr als eine untere Leiste",
            950.0 - HOTBAR_H
        );
        assert!(
            HOTBAR_H >= crate::hotbar::SLOT + 8.0 && HOTBAR_H <= 64.0,
            "untere Leiste ist {HOTBAR_H} px hoch, muss die {} px großen Slots \
             fassen und darf die alte Hotbar (64 px) nicht übersteigen",
            crate::hotbar::SLOT
        );
    }

    #[test]
    fn hotbar_slots_stehen_mittig() {
        let erwartet_breite = 10.0 * crate::hotbar::SLOT + 9.0 * 4.0;
        // Mindestbreite der App (1080) und eine breite Fläche: in beiden Fällen
        // muss die Slot-Mitte die Mitte der Leiste sein.
        for breite in [1080.0, 1500.0, 1920.0] {
            let ctx = egui::Context::default();
            let mut app = App::with_ctx(ctx.clone());
            for _ in 0..3 {
                frame(&mut app, &ctx, breite, 950.0);
            }
            let slots = app.ui.last_slots.expect("Slots müssen gezeichnet werden");
            assert!(
                (slots.center().x - breite * 0.5).abs() < 1.0,
                "{breite} px breit: Hotbar-Slots stehen bei x = {} statt in der Mitte ({}) \
                 — links/rechts verschieben sie sich mit den Randgruppen",
                slots.center().x,
                breite * 0.5
            );
            assert!(
                (slots.width() - erwartet_breite).abs() < 1.0,
                "Slots sind {} px breit, erwartet {erwartet_breite} px (zehn Slots à {} px)",
                slots.width(),
                crate::hotbar::SLOT
            );
        }
    }

    /// Die äußeren Schaltflächen brauchen Luft zum Fensterrand (T0043, Turn 3):
    /// mit 8 px lagen sie praktisch am Bildschirmrand und wirkten abgeschnitten.
    #[test]
    fn randgruppen_haben_abstand_zum_fensterrand() {
        for breite in [1080.0, 1500.0, 1920.0] {
            let ctx = egui::Context::default();
            let mut app = App::with_ctx(ctx.clone());
            for _ in 0..3 {
                frame(&mut app, &ctx, breite, 950.0);
            }
            let l = app.ui.last_kopf_links.expect("linke Gruppe");
            let r = app.ui.last_kopf_rechts.expect("rechte Gruppe");
            assert!(
                l.left() >= 12.0,
                "{breite} px: linke Schaltergruppe beginnt bei x = {} — zu nah am Rand",
                l.left()
            );
            assert!(
                breite - r.right() >= 12.0,
                "{breite} px: rechte Gruppe endet bei x = {} (Randabstand {} px) \
                 — die äußere Schaltfläche liegt am Fensterrand",
                r.right(),
                breite - r.right()
            );
        }
    }

    /// Die entfernten Angaben dürfen nicht über einen zweiten Weg
    /// zurückkommen, und die Vorlagenauswahl ist ein Fenster, kein Aufklappfeld.
    #[test]
    fn keine_doppelten_angaben_und_eigenes_vorlagenfenster() {
        let app_quelle = include_str!("app.rs");
        // Der Quelltext dieses Tests nennt die gesuchten Wörter selbst —
        // für die Prüfung zählt nur der Code davor.
        let app_quelle = &app_quelle[..app_quelle
            .find("mod tests_untere_leiste")
            .unwrap_or(app_quelle.len())];
        let hotbar_quelle = include_str!("hotbar.rs");
        for verboten in ["fn status_bar", "STATUS_H", "summary_short()", "build_label())"] {
            assert!(
                !app_quelle.contains(verboten),
                "„{verboten}\" gehört nicht mehr in die Oberfläche — Koordinaten, \
                 Objektzahl, Build-Kennung und Einheitenwahl stehen anderswo"
            );
        }
        assert!(
            !hotbar_quelle.contains("ComboBox"),
            "Die Hotbar-Auswahl ist ein eigenes Fenster, kein Aufklappfeld"
        );
        assert!(
            hotbar_quelle.contains("auswahl_fenster"),
            "Fenster „Hotbar-Vorlagen\" fehlt"
        );
    }
}

#[cfg(test)]
mod tests_ghost_drehrichtung {
    //! T0006: `R` dreht die Vorschau im Uhrzeigersinn, `Umschalt+R` dagegen.
    //!
    //! Geprüft an der Geometrie der Vorschau, nicht am gespeicherten Winkel:
    //! eine waagerechte Linie nach rechts muss nach `R` nach unten zeigen
    //! (Uhrzeigersinn) und nach `Umschalt+R` nach oben.

    use super::*;

    fn app_mit_vorschau() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        // `R` gehört `modify.rotate` — ausdrücklich setzen, nicht auf die
        // `prefs.json` des Rechners verlassen: der Nutzer belegt seine Tasten
        // um, und der Test fiel dann grundlos (gesehen beim Ausliefern).
        app.prefs.bind("modify.rotate", "r");
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [2.0, 0.0]}),
            )
            .expect("Linie anlegen");
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        app.copy_selection(false);
        app.pointer_world = Some(V2::new(0.0, 0.0));
        app.start_ghost_paste();
        app
    }

    /// Ein Frame mit genau einer Taste, wie ihn das laufende Programm bekommt.
    fn taste(app: &mut App, key: egui::Key, modifiers: egui::Modifiers) {
        let ctx = app.ctx.clone();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            modifiers,
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    /// Richtung der Linie in der Vorschau (Endpunkt minus Startpunkt).
    fn vorschau_richtung(app: &App) -> V2 {
        let g = app.ghost.as_ref().expect("Vorschau muss offen sein");
        let ents = crate::ghost::clipboard_entities(app.doc(), g, &app.ui.clipboard);
        for e in &ents {
            if let EntityKind::Line { a, b } = &e.kind {
                return *b - *a;
            }
        }
        panic!("keine Linie in der Vorschau");
    }

    #[test]
    fn r_dreht_im_uhrzeigersinn() {
        let mut app = app_mit_vorschau();
        taste(&mut app, egui::Key::R, egui::Modifiers::NONE);
        let d = vorschau_richtung(&app);
        assert!(
            d.y < -1.9 && d.x.abs() < 0.01,
            "R muss im Uhrzeigersinn drehen (waagerecht nach rechts → nach unten), \
             Richtung war {d:?}"
        );
    }

    #[test]
    fn umschalt_r_dreht_gegen_den_uhrzeigersinn() {
        let mut app = app_mit_vorschau();
        taste(&mut app, egui::Key::R, egui::Modifiers::SHIFT);
        let d = vorschau_richtung(&app);
        assert!(
            d.y > 1.9 && d.x.abs() < 0.01,
            "Umschalt+R muss gegen den Uhrzeigersinn drehen (waagerecht nach rechts → nach oben), \
             Richtung war {d:?}"
        );
    }
}

#[cfg(test)]
mod tests_tastenbelegung {
    //! T0056: Die Tastatur in drei Schichten (`docs/benutzer/TASTEN.md`).
    //!
    //! 1. Ziffern `1` … `0` starten den Hotbar-Slot.
    //! 2. Buchstaben und Symbole kommen aus der Tastaturbelegung.
    //! 3. Steuerungstasten, Funktionstasten und Kombinationen sind fest.
    //!
    //! Geprüft wird das Verhalten, nicht nur die Belegung: der Schwenk folgt der
    //! Belegung (vorher hing er an `Key::W`/`A`/`S`/`D` im Code), und Drehen und
    //! Spiegeln der Einfüge-Vorschau hängen an den Funktionen „Drehen" und
    //! „Spiegeln".

    use super::*;

    fn frame_mit(app: &mut App, events: Vec<egui::Event>) {
        frame_mit_lage(app, events, egui::Modifiers::NONE);
    }

    /// Ein Bild mit ausdrücklicher Modifier-Lage: egui 0.29 liest
    /// `InputState::modifiers` aus `RawInput::modifiers`, nicht aus dem
    /// Tastenereignis (T0068) — Strg+S kommt sonst nie als Kombination an.
    fn frame_mit_lage(app: &mut App, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            modifiers,
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// Ein Bild mit ausdrücklicher Fokuslage und laufender Zeit (T0140):
    /// `focused = false` steht für den Systemdialog, während dem das
    /// Betriebssystem kein Tastenloslassen mehr nachliefert. Die laufende Zeit
    /// braucht der Dauerlauf — er rechnet mit der Bilddauer.
    fn frame_zeit_fokus(
        app: &mut App,
        t: &mut f64,
        focused: bool,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) {
        let ctx = app.ctx.clone();
        *t += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            time: Some(*t),
            modifiers,
            focused,
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    fn druck(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn loslassen(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Bild mit **laufender** Zeit (T0101): der kontinuierliche Schwenk rechnet
    /// mit der echten Bilddauer — ohne Zeitfortschritt bewegt er sich nicht.
    fn frame_zeit(app: &mut App, t: &mut f64, events: Vec<egui::Event>) {
        frame_zeit_lage(app, t, events, egui::Modifiers::NONE);
    }

    /// Wie [`frame_zeit`], mit ausdrücklicher Modifier-Lage (T0068: egui liest
    /// sie aus `RawInput::modifiers`, nicht aus dem Tastenereignis).
    fn frame_zeit_lage(
        app: &mut App,
        t: &mut f64,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) {
        let ctx = app.ctx.clone();
        *t += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            time: Some(*t),
            modifiers,
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// App mit einer Zeichnung in der Mitte — damit der Schwenk messbar ist.
    fn app_mit_zeichnung() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [2000.0, 0.0]}),
            )
            .expect("Linie anlegen");
        // Vorgabebelegung ausdrücklich setzen: `App::with_ctx` lädt die
        // Einstellungen des Nutzers, die hier nicht stören dürfen.
        app.prefs.reset_bindings();
        // T0233: Dasselbe für die Hotbar-Vorlagen. Seit T0215 prüfen mehrere
        // Tests die Slots der ersten Vorlage; die `prefs.json` des Rechners
        // hat der Nutzer aber womöglich umbelegt (`strg_schaltet_die_hotbar_…`
        // fiel genau daran, nicht am Code).
        let werk = jjcad_core::persist::UserPrefs::default();
        app.prefs.hotbar_presets = werk.hotbar_presets;
        app.prefs.active_preset = werk.active_preset;
        app
    }

    #[test]
    fn w_schwenkt_die_ansicht_nach_oben() {
        let mut app = app_mit_zeichnung();
        // Zwei Bilder ohne Taste: Ansicht steht, Grundlage für den Vergleich.
        frame_mit(&mut app, vec![]);
        let vorher = app.current_view().center;
        frame_mit(&mut app, vec![druck(egui::Key::W)]);
        frame_mit(&mut app, vec![]);
        let nachher = app.current_view().center;
        assert!(
            nachher.y > vorher.y + 1.0 && (nachher.x - vorher.x).abs() < 1.0,
            "W muss die Ansicht nach oben schieben (Welt-y wächst), vorher {vorher:?}, \
             nachher {nachher:?}"
        );
    }

    #[test]
    fn s_schwenkt_nach_unten_und_stutzt_nicht() {
        let mut app = app_mit_zeichnung();
        frame_mit(&mut app, vec![]);
        let vorher = app.current_view().center;
        frame_mit(&mut app, vec![druck(egui::Key::S)]);
        frame_mit(&mut app, vec![]);
        let nachher = app.current_view().center;
        assert!(
            nachher.y < vorher.y - 1.0,
            "S muss die Ansicht nach unten schieben, vorher {vorher:?}, nachher {nachher:?}"
        );
        assert!(
            app.tool.is_none(),
            "S darf kein Werkzeug starten — die Taste gehört dem Schwenk, nicht dem \
             Stutzen: {:?}",
            app.tool.as_ref().map(|t| t.action.clone())
        );
    }

    /// T0089/T0101: Im Modus **Sprunghaft** ist der Schwenk eine gewöhnliche
    /// Funktion — ein Druck rückt um genau einen Schritt (60 px × Faktor). Die
    /// Taste weiter gedrückt zu halten bewegt nichts mehr; den Dauerlauf gibt
    /// es nur im Modus „Kontinuierlich“.
    #[test]
    fn gehaltene_schwenktaste_rueckt_nur_einen_schritt() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_speed = 1.0;
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Sprunghaft;
        frame_mit(&mut app, vec![]);
        let vorher = app.current_view().center;
        frame_mit(&mut app, vec![druck(egui::Key::W)]);
        let nach_druck = app.current_view().center;
        assert!(
            nach_druck.y > vorher.y + 1.0,
            "ein Druck muss die Ansicht sichtbar schieben: {vorher:?} → {nach_druck:?}"
        );
        // Zwanzig Bilder ohne neues Tastenereignis: die Taste gilt weiter als
        // gedrückt (`keys_down` bleibt stehen), die Ansicht darf ruhen.
        for _ in 0..20 {
            frame_mit(&mut app, vec![]);
        }
        assert_eq!(
            app.current_view().center,
            nach_druck,
            "die gehaltene Taste darf nicht weiterschieben — der Schwenk ist ein Schritt, kein Dauerlauf"
        );
    }

    /// T0089: `Strg+S` speichert — es schwenkt nicht. Vorher wertete der
    /// Dauerlauf in `canvas::handle_navigation` nur `key_down` aus und sah die
    /// Strg-Taste nicht.
    #[test]
    fn strg_s_schwenkt_nicht() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_speed = 1.0;
        // Der eingebaute Dateidialog: `Strg+S` löst `file.save` aus und die
        // Zeichnung ist unbenannt. Mit dem Systemdialog ginge ein echtes
        // Speichern-Fenster der Sitzung auf (XDG-Portal) — im Test unzulässig.
        app.prefs.file_dialog = "intern".into();
        frame_mit(&mut app, vec![]);
        let vorher = app.current_view().center;
        let strg_s = egui::Event::Key {
            key: egui::Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        frame_mit_lage(&mut app, vec![strg_s], egui::Modifiers::COMMAND);
        frame_mit_lage(&mut app, vec![], egui::Modifiers::COMMAND);
        assert_eq!(
            app.current_view().center, vorher,
            "Strg+S darf die Ansicht nicht bewegen"
        );
        // Gegenprobe an einer frischen App: ohne Strg schiebt dieselbe Taste
        // (in der ersten App steht nach Strg+S der Speicherdialog offen — der
        // nimmt die Tastatur zu Recht für sich).
        let mut frisch = app_mit_zeichnung();
        frisch.prefs.pan_speed = 1.0;
        frame_mit(&mut frisch, vec![]);
        let start = frisch.current_view().center;
        frame_mit(&mut frisch, vec![druck(egui::Key::S)]);
        assert!(
            frisch.current_view().center.y < start.y - 1.0,
            "S allein muss nach unten schieben: {start:?} → {:?}",
            frisch.current_view().center
        );
    }

    /// T0140: Der Dauerlauf liest `keys_down`. Nimmt ein anderes Fenster den
    /// Fokus (Systemdialog bei „Strg+S"/„Strg+O"), liefert das Betriebssystem
    /// das Loslassen nicht mehr nach — die Taste bliebe „gedrückt" und die
    /// Ansicht liefe endlos weiter. Nach einem Fokusverlust gilt keine Taste
    /// mehr als gedrückt, bis sie neu kommt.
    #[test]
    fn fokusverlust_beendet_den_dauerlauf() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_speed = 1.0;
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        let mut t = 0.0;
        frame_zeit_fokus(&mut app, &mut t, true, vec![], egui::Modifiers::NONE);
        let start = app.current_view().center;
        frame_zeit_fokus(
            &mut app,
            &mut t,
            true,
            vec![druck(egui::Key::S)],
            egui::Modifiers::NONE,
        );
        assert!(
            app.current_view().center.y < start.y - 1.0,
            "S muss die Ansicht in Bewegung halten: {start:?} → {:?}",
            app.current_view().center
        );
        // Der Dialog nimmt den Fokus: „S" wird draußen losgelassen, das
        // Loslassen erreicht die Anwendung nie.
        frame_zeit_fokus(&mut app, &mut t, false, vec![], egui::Modifiers::NONE);
        assert!(
            app.ctx.input(|i| i.keys_down.is_empty()),
            "nach dem Fokusverlust darf keine Taste als gedrückt gelten"
        );
        let beim_verlust = app.current_view().center;
        for _ in 0..20 {
            frame_zeit_fokus(&mut app, &mut t, true, vec![], egui::Modifiers::NONE);
        }
        assert_eq!(
            app.current_view().center,
            beim_verlust,
            "die Taste darf nach dem Fokusverlust nicht weiter schieben"
        );
    }

    /// T0140, gemeldeter Fall: „Strg+S" speichert und öffnet den Dateidialog.
    /// Der nimmt den Fokus, das Loslassen von „S" bleibt aus — nach dem
    /// Schließen des Dialogs schob die Ansicht endlos weiter.
    #[test]
    fn nach_strg_s_schwenkt_die_ansicht_nicht_weiter() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_speed = 1.0;
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        // Eingebauter Dateidialog: im Test geht kein Fenster der Sitzung auf.
        app.prefs.file_dialog = "intern".into();
        let mut t = 0.0;
        frame_zeit_fokus(&mut app, &mut t, true, vec![], egui::Modifiers::NONE);
        let vorher = app.current_view().center;
        let strg_s = egui::Event::Key {
            key: egui::Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        frame_zeit_fokus(
            &mut app,
            &mut t,
            true,
            vec![strg_s],
            egui::Modifiers::COMMAND,
        );
        assert_eq!(
            app.current_view().center, vorher,
            "Strg+S speichert, es schwenkt nicht"
        );
        // Der Dialog nimmt den Fokus (Systemdialog); „S" und „Strg" werden
        // draußen losgelassen und erreichen die Anwendung nie.
        frame_zeit_fokus(&mut app, &mut t, false, vec![], egui::Modifiers::NONE);
        // Dialog zu, Fenster wieder vorn.
        app.ui.file_dialog = None;
        frame_zeit_fokus(&mut app, &mut t, true, vec![], egui::Modifiers::NONE);
        let nach_dialog = app.current_view().center;
        for _ in 0..20 {
            frame_zeit_fokus(&mut app, &mut t, true, vec![], egui::Modifiers::NONE);
        }
        assert_eq!(
            app.current_view().center,
            nach_dialog,
            "nach dem Speichern darf die Ansicht nicht von selbst weiterlaufen"
        );
    }

    /// T0089: Auch aus der **Hotbar** ist der Schwenk eine gewöhnliche
    /// Funktion — ein Druck auf den Slot rückt um einen Schritt, gehalten
    /// (Ziffer bleibt gedrückt) nicht weiter.
    #[test]
    fn schwenk_aus_der_hotbar_rueckt_einen_schritt() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_speed = 1.0;
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Sprunghaft;
        app.prefs.set_slot(0, Some("view.pan_right".into()));
        frame_mit(&mut app, vec![]);
        let vorher = app.current_view().center;
        frame_mit(&mut app, vec![druck(egui::Key::Num1)]);
        let nachher = app.current_view().center;
        assert!(
            nachher.x > vorher.x + 1.0 && (nachher.y - vorher.y).abs() < 1.0,
            "Slot 1 mit „Ansicht rechts“ muss nach rechts schieben: {vorher:?} → {nachher:?}"
        );
        assert!(app.tool.is_none(), "der Schwenk ist kein Werkzeug");
        for _ in 0..10 {
            frame_mit(&mut app, vec![]);
        }
        assert_eq!(
            app.current_view().center,
            nachher,
            "die gehaltene Ziffer darf nicht weiterschieben"
        );
    }

    /// Die Taste folgt der Belegung, nicht dem Buchstaben: liegt der Schwenk auf
    /// `Z`, schiebt `Z` und `W` bleibt wirkungslos.
    #[test]
    fn schwenk_folgt_der_belegung() {
        let mut app = app_mit_zeichnung();
        // T0064: `bind` legt die Funktion **zusätzlich** auf Z — W muss
        // deshalb ausdrücklich freigegeben werden, sonst schwenkte es weiter.
        app.prefs.bind("view.pan_up", "z");
        app.prefs.unbind_key("w");
        assert_eq!(
            app.prefs.keys_for("view.pan_up"),
            &["z".to_string()],
            "der Schwenk liegt jetzt allein auf Z"
        );
        frame_mit(&mut app, vec![]);
        let vorher = app.current_view().center;
        frame_mit(&mut app, vec![druck(egui::Key::W)]);
        frame_mit(&mut app, vec![]);
        assert_eq!(
            app.current_view().center,
            vorher,
            "W ist nicht mehr belegt und darf die Ansicht nicht bewegen"
        );
        frame_mit(&mut app, vec![druck(egui::Key::Z)]);
        frame_mit(&mut app, vec![]);
        assert!(
            app.current_view().center.y > vorher.y + 1.0,
            "Z schwenkt jetzt nach oben: {:?} → {:?}",
            vorher,
            app.current_view().center
        );
    }

    /// T0065: Die Taste `0` startet den **letzten** Hotbar-Slot. Die Auswertung
    /// rechnete früher `0 - 1` und landete damit auf dem ersten Element — `0` und
    /// `1` starteten beide den ersten Slot.
    #[test]
    fn taste_0_startet_den_letzten_slot() {
        for (key, erwartet, grund) in [
            (egui::Key::Num0, "draw.wall", "0 gehört dem letzten Slot (Index 9)"),
            (egui::Key::Num1, "draw.line", "1 gehört dem ersten Slot (Index 0)"),
        ] {
            let mut app = app_mit_zeichnung();
            app.prefs.set_slot(0, Some("draw.line".into()));
            app.prefs.set_slot(9, Some("draw.wall".into()));
            assert_eq!(app.prefs.slots().len(), 10, "Hotbar hat zehn Slots");
            frame_mit(&mut app, vec![druck(key)]);
            assert_eq!(app.tool_id(), Some(erwartet), "{grund}");
        }
    }

    /// T0067: Die Einstellung „Schwenk“ skaliert den Einzelschritt der
    /// Schwenk-Funktion — doppelter Faktor heißt doppelter Weg.
    #[test]
    fn schwenkgeschwindigkeit_skaliert_den_schritt() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Sprunghaft;
        let start = app.current_view().center;
        app.prefs.pan_speed = 1.0;
        app.run_action(&app.ctx.clone(), "view.pan_right");
        let weg_eins = (app.current_view().center.x - start.x).abs();

        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Sprunghaft;
        let start = app.current_view().center;
        app.prefs.pan_speed = 2.0;
        app.run_action(&app.ctx.clone(), "view.pan_right");
        let weg_zwei = (app.current_view().center.x - start.x).abs();

        assert!(weg_eins > 1.0, "ein Schritt muss sichtbar sein: {weg_eins}");
        assert!(
            (weg_zwei / weg_eins - 2.0).abs() < 1e-3,
            "doppelter Faktor muss doppelten Weg geben: {weg_eins} → {weg_zwei}"
        );
    }

    /// Ein einzelner Druck (Hotbar, Kürzel) rückt die Ansicht im Modus
    /// **Sprunghaft** einen Schritt weiter — die Funktion ist auch ohne
    /// gehaltene Taste benutzbar.
    #[test]
    fn schwenkfunktion_rueckt_einen_schritt() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Sprunghaft;
        let vorher = app.current_view().center;
        app.run_action(&app.ctx.clone(), "view.pan_right");
        let nachher = app.current_view().center;
        assert!(
            nachher.x > vorher.x + 1.0 && (nachher.y - vorher.y).abs() < 1.0,
            "„Ansicht rechts“ muss nach rechts schieben: {vorher:?} → {nachher:?}"
        );
    }

    /// T0101: Im Modus „Kontinuierlich“ setzt ein Aufruf der Funktion **keinen**
    /// Schritt neben dem Dauerlauf — sonst zählte der Druck doppelt und die
    /// Tastenwiederholung des Betriebssystems beschleunigte die Ansicht.
    #[test]
    fn dauerlauf_setzt_keinen_schritt_neben_dem_bild() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        let vorher = app.current_view().center;
        app.run_action(&app.ctx.clone(), "view.pan_right");
        assert_eq!(
            app.current_view().center,
            vorher,
            "der Aufruf allein darf im Dauerlauf nichts bewegen"
        );
        // Ein Bild mit gehaltener Taste bewegt dagegen — der Dauerlauf läuft.
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![druck(egui::Key::D)]);
        assert!(
            app.current_view().center.x > vorher.x + 1.0,
            "das gehaltene D muss den Dauerlauf starten: {vorher:?} → {:?}",
            app.current_view().center
        );
    }

    /// T0101: Im Modus „Kontinuierlich“ hält eine gedrückt gehaltene    /// T0101: Im Modus „Kontinuierlich“ hält eine gedrückt gehaltene
    /// Schwenktaste die Ansicht in Bewegung — Bild für Bild, nicht nur beim
    /// Druck. Gemessen wird der Weg über mehrere Bilder ohne neues Ereignis.
    #[test]
    fn kontinuierlicher_schwenk_haelt_die_ansicht_in_bewegung() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.prefs.pan_speed = 1.0;
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![]);
        let vorher = app.current_view().center;
        frame_zeit(&mut app, &mut t, vec![druck(egui::Key::W)]);
        let nach_druck = app.current_view().center;
        assert!(
            nach_druck.y > vorher.y + 1.0,
            "der Druck muss sofort schieben: {vorher:?} → {nach_druck:?}"
        );
        // Zehn Bilder ohne neues Ereignis: die Taste gilt weiter als gedrückt.
        for _ in 0..10 {
            frame_zeit(&mut app, &mut t, vec![]);
        }
        let nach_halten = app.current_view().center;
        let strecke = nach_halten.y - nach_druck.y;
        assert!(
            strecke > 200.0,
            "gehalten muss die Ansicht weiterlaufen (erwartet einige hundert mm, gemessen {strecke})"
        );
        assert!(
            (nach_halten.x - vorher.x).abs() < 1.0,
            "der Schwenk nach oben darf nicht seitlich wandern"
        );
    }

    /// T0101: Loslassen beendet den Dauerlauf — danach steht die Ansicht.
    #[test]
    fn loslassen_beendet_den_dauerlauf() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.prefs.pan_speed = 1.0;
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![]);
        frame_zeit(&mut app, &mut t, vec![druck(egui::Key::W)]);
        for _ in 0..3 {
            frame_zeit(&mut app, &mut t, vec![]);
        }
        frame_zeit(&mut app, &mut t, vec![loslassen(egui::Key::W)]);
        let nach_loslassen = app.current_view().center;
        for _ in 0..6 {
            frame_zeit(&mut app, &mut t, vec![]);
        }
        assert_eq!(
            app.current_view().center,
            nach_loslassen,
            "nach dem Loslassen darf die Ansicht nicht weiterwandern"
        );
    }

    /// T0101: Auch der Dauerlauf folgt der Einstellung „Schwenk“ — doppelter
    /// Faktor heißt doppelter Weg in derselben Zeit.
    #[test]
    fn dauerlauf_folgt_der_geschwindigkeit() {
        let weg = |faktor: f32| {
            let mut app = app_mit_zeichnung();
            app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
            app.prefs.pan_speed = faktor;
            let mut t = 0.0;
            frame_zeit(&mut app, &mut t, vec![]);
            let start = app.current_view().center;
            frame_zeit(&mut app, &mut t, vec![druck(egui::Key::W)]);
            for _ in 0..8 {
                frame_zeit(&mut app, &mut t, vec![]);
            }
            app.current_view().center.y - start.y
        };
        let einfach = weg(1.0);
        let doppelt = weg(2.0);
        assert!(einfach > 100.0, "der Dauerlauf muss messbar sein: {einfach}");
        assert!(
            (doppelt / einfach - 2.0).abs() < 0.05,
            "doppelter Faktor muss doppelten Weg geben: {einfach} → {doppelt}"
        );
    }

    /// T0146: Das **Grundtempo** des Schwenks ist um 50 % erhöht — der Faktor
    /// bleibt bei 1 × die Vorgabe, damit die Erhöhung auch für gespeicherte
    /// Einstellungen greift. Gemessen am Bildweg: ein Druck im Modus
    /// „Sprunghaft" schiebt 90 px (vorher 60), der Dauerlauf läuft mit
    /// 720 px/s (vorher 480).
    #[test]
    fn schwenk_grundtempo_ist_50_prozent_hoeher() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_speed = 1.0;
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Sprunghaft;
        frame_mit(&mut app, vec![]);
        let vorher = app.current_view().center;
        let zoom = app.current_view().zoom;
        frame_mit(&mut app, vec![druck(egui::Key::W)]);
        let nachher = app.current_view().center;
        let px = (nachher.y - vorher.y) * zoom;
        assert!(
            (px - 90.0).abs() < 1.0,
            "ein Schritt muss 90 px weit sein (60 px + 50 %), gemessen {px} px"
        );
        assert!(
            (SCHWENK_SCHRITT_PX - 90.0).abs() < 1e-6,
            "Schrittweite ist 90 px, nicht {SCHWENK_SCHRITT_PX}"
        );
        assert!(
            (SCHWENK_TEMPO_PX_PRO_S - 720.0).abs() < 1e-6,
            "Dauerlauf-Tempo ist 720 px/s, nicht {SCHWENK_TEMPO_PX_PRO_S}"
        );
    }

    /// T0101: Ein gedrückt gehaltener Schwenk-Slot der Hotbar bewegt die Ansicht
    /// — die Leiste meldet die Richtung, `schwenk_fortschritt` verrechnet sie
    /// und löscht den Merker im selben Bild.
    #[test]
    fn gedrueckter_hotbar_slot_bewegt_die_ansicht() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.prefs.pan_speed = 1.0;
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![]);
        let vorher = app.current_view().center;
        app.ui.schwenk_zeiger = Some((1.0, 0.0));
        app.schwenk_fortschritt(&app.ctx.clone());
        let nachher = app.current_view().center;
        assert!(
            nachher.x > vorher.x + 1.0 && (nachher.y - vorher.y).abs() < 1.0,
            "gehaltener Slot „Ansicht rechts“ muss nach rechts schieben: {vorher:?} → {nachher:?}"
        );
        assert_eq!(
            app.ui.schwenk_zeiger, None,
            "der gemerkte Druck gilt nur für ein Bild"
        );
    }

    /// T0101: Auch die gehaltene **Ziffer** eines Schwenk-Slots läuft weiter —
    /// der Weg über die Tastatur ist derselbe Dauerlauf.
    #[test]
    fn gehaltene_hotbar_ziffer_bewegt_weiter() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.prefs.pan_speed = 1.0;
        app.prefs.set_slot(0, Some("view.pan_right".into()));
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![]);
        let vorher = app.current_view().center;
        frame_zeit(&mut app, &mut t, vec![druck(egui::Key::Num1)]);
        let nach_druck = app.current_view().center;
        for _ in 0..8 {
            frame_zeit(&mut app, &mut t, vec![]);
        }
        let nach_halten = app.current_view().center;
        assert!(
            nach_halten.x - nach_druck.x > 200.0,
            "die gehaltene Ziffer muss weiterlaufen: {nach_druck:?} → {nach_halten:?}"
        );
        assert!(
            nach_druck.x > vorher.x,
            "der Druck selbst muss schon schieben"
        );
    }

    /// T0101/T0089: Mit Strg sind die Kombinationen gemeint — der Dauerlauf
    /// schweigt, sonst schöbe `Strg+W` die Ansicht.
    #[test]
    fn dauerlauf_schweigt_mit_strg() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.prefs.pan_speed = 1.0;
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![]);
        let vorher = app.current_view().center;
        frame_zeit_lage(
            &mut app,
            &mut t,
            vec![druck(egui::Key::W)],
            egui::Modifiers::COMMAND,
        );
        for _ in 0..5 {
            frame_zeit_lage(&mut app, &mut t, vec![], egui::Modifiers::COMMAND);
        }
        assert_eq!(
            app.current_view().center, vorher,
            "mit Strg darf der Dauerlauf die Ansicht nicht bewegen"
        );
    }

    /// T0101: Bei offenem Protokoll gehört die Tastatur dem Chat — die Ansicht
    /// bleibt stehen.
    #[test]
    fn dauerlauf_steht_bei_offenem_protokoll() {
        let mut app = app_mit_zeichnung();
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.prefs.pan_speed = 1.0;
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![]);
        app.chat.offen = true;
        let vorher = app.current_view().center;
        frame_zeit(&mut app, &mut t, vec![druck(egui::Key::W)]);
        for _ in 0..5 {
            frame_zeit(&mut app, &mut t, vec![]);
        }
        assert_eq!(
            app.current_view().center, vorher,
            "bei offenem Protokoll darf der Dauerlauf nicht schieben"
        );
    }

    /// Drehen und Spiegeln der Einfüge-Vorschau hängen an den Funktionen
    /// „Drehen" (`modify.rotate`) und „Spiegeln" (`modify.mirror`) — nicht an
    /// den festen Tasten R und F (T0056).
    #[test]
    fn vorschau_dreht_und_spiegelt_mit_den_funktionen() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [2.0, 0.0]}),
            )
            .expect("Linie anlegen");
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        app.copy_selection(false);
        app.pointer_world = Some(V2::new(0.0, 0.0));
        app.start_ghost_paste();
        app.prefs.reset_bindings();
        assert_eq!(app.prefs.key_for("modify.rotate").as_deref(), Some("r"));
        assert_eq!(app.prefs.key_for("modify.mirror").as_deref(), Some("f"));

        // Drehen auf Q legen (T0064: zusätzlich) und R freigeben: R darf nichts
        // mehr drehen, Q schon.
        app.prefs.bind("modify.rotate", "q");
        app.prefs.unbind_key("r");
        let vorher = app.ghost.as_ref().map(|g| g.rot).unwrap_or(0.0);
        frame_mit(&mut app, vec![druck(egui::Key::R)]);
        let nach_r = app.ghost.as_ref().map(|g| g.rot).unwrap_or(0.0);
        assert!(
            (nach_r - vorher).abs() < 1e-9,
            "R ist nicht mehr „Drehen“ und darf die Vorschau nicht drehen"
        );
        frame_mit(&mut app, vec![druck(egui::Key::Q)]);
        let nach_q = app.ghost.as_ref().map(|g| g.rot).unwrap_or(0.0);
        assert!(
            (nach_q - vorher).abs() > 0.1,
            "Q dreht jetzt die Vorschau: {vorher} → {nach_q}"
        );

        // Spiegeln auf E legen (T0064: zusätzlich) und F freigeben: F wirkt
        // nicht mehr, E spiegelt.
        app.prefs.bind("modify.mirror", "e");
        app.prefs.unbind_key("f");
        let gespiegelt_vorher = app.ghost.as_ref().map(|g| g.mirror).unwrap_or(false);
        frame_mit(&mut app, vec![druck(egui::Key::F)]);
        assert_eq!(
            app.ghost.as_ref().map(|g| g.mirror).unwrap_or(false),
            gespiegelt_vorher,
            "F ist nicht mehr „Spiegeln“ und darf die Vorschau nicht spiegeln"
        );
        frame_mit(&mut app, vec![druck(egui::Key::E)]);
        assert_ne!(
            app.ghost.as_ref().map(|g| g.mirror).unwrap_or(false),
            gespiegelt_vorher,
            "E spiegelt jetzt die Vorschau"
        );
    }

    /// T0215: Funktionstasten bleiben fest im Code; **Tastenkombinationen** sind
    /// eine eigene Ebene der Belegung und lassen sich umbelegen.
    #[test]
    fn funktionstasten_bleiben_fest() {
        let mut app = App::with_ctx(egui::Context::default());
        // Welche Funktion heute auf `k` liegt, hängt von der Belegung des
        // Rechners ab — gemerkt wird sie trotzdem, das ist der Prüfpunkt.
        let vorher = app.prefs.action_for_key("k");
        // F5 gehört „Alles zeigen" und lässt sich nicht auf einen Buchstaben legen.
        app.prefs.bind("view.zoom_extents", "k");
        assert_eq!(
            app.prefs.key_for("view.zoom_extents").as_deref(),
            Some("f5"),
            "die feste Funktionstaste bleibt"
        );
        assert_eq!(app.prefs.action_for_key("k"), vorher);
        assert!(!app.prefs.belegung_aenderbar("view.zoom_extents"));

        // Strg+S darf dagegen auf eine andere Kombination wandern.
        app.prefs.bind("file.save", "ctrl+k");
        assert_eq!(
            app.prefs.action_for_key("ctrl+k").as_deref(),
            Some("file.save")
        );
        assert!(app.prefs.belegung_aenderbar("file.save"));
    }

    /// T0159: `F5` zeigt die ganze Zeichnung.
    ///
    /// Geprüft wird das Ergebnis, nicht der Zoom-Wert: vorher steht die Ansicht
    /// weit neben der Linie und viel zu nah, nach `F5` liegen **beide** Enden in
    /// der Zeichenfläche. Die Taste ist fest verdrahtet (Schicht 3) und läuft
    /// über `view.zoom_extents` — denselben Weg wie der Knopf „Alles zeigen".
    #[test]
    fn f5_zeigt_die_ganze_zeichnung() {
        let mut app = app_mit_zeichnung();
        app.set_view(View {
            center: V2::new(-60_000.0, 45_000.0),
            zoom: 4.0,
            rot: 0.0,
        });
        let mut t = 0.0;
        // Ein Bild ohne Tastendruck: die Zeichenfläche steht danach fest.
        frame_zeit(&mut app, &mut t, vec![]);
        let flaeche = app.ui.last_canvas;
        let vorher = app.current_view();
        let ende = V2::new(2000.0, 0.0);
        assert!(
            !flaeche.contains(world_to_screen(flaeche, vorher, ende)),
            "Ausgangslage muss die Zeichnung außerhalb der Fläche zeigen"
        );

        frame_zeit(
            &mut app,
            &mut t,
            vec![druck(egui::Key::F5), loslassen(egui::Key::F5)],
        );

        let nachher = app.current_view();
        assert!(
            (nachher.zoom - vorher.zoom).abs() > 1e-6,
            "F5 muss den Zoom ändern: {} → {}",
            vorher.zoom,
            nachher.zoom
        );
        for p in [V2::new(0.0, 0.0), ende] {
            let s = world_to_screen(flaeche, nachher, p);
            assert!(
                flaeche.contains(s),
                "Punkt {p:?} liegt nach F5 außerhalb der Zeichenfläche: {s:?} in {flaeche:?}"
            );
        }
    }

    /// T0215: Die gehaltene Steuerungstaste schaltet die **Hotbar** um — mit
    /// Strg liegt dort eine eigene Belegung, und das kleine Feld neben der
    /// Hotbar zeigt die Ebene. Die Ziffer startet den Slot dieser Ebene.
    #[test]
    fn strg_schaltet_die_hotbar_um_und_zeigt_die_ebene() {
        let mut app = app_mit_zeichnung();
        // Die Ebenen liegen nebeneinander: Slot 1 trägt ohne Strg etwas
        // anderes als mit Strg.
        app.prefs.set_slot_fuer(
            jjcad_core::persist::Kombination::STRG,
            0,
            Some("draw.circle".into()),
        );
        assert_eq!(
            app.prefs.slots()[0].as_deref(),
            Some("draw.wall"),
            "die Strg-Variante darf die Grundbelegung nicht überschreiben"
        );

        // Ohne Steuerungstaste: kein Ebenenfeld.
        frame_mit(&mut app, vec![]);
        assert!(
            app.ui.last_kombi_icon.is_none(),
            "ohne gehaltene Steuerungstaste steht kein Feld neben der Hotbar"
        );
        assert_eq!(app.ui.hotbar_kombi, jjcad_core::persist::Kombination::KEINE);

        // Mit Strg: das Feld steht da, die Hotbar zeigt die Strg-Slots.
        frame_mit_lage(&mut app, vec![], egui::Modifiers::COMMAND);
        assert!(
            app.ui.last_kombi_icon.is_some(),
            "das kleine Feld neben der Hotbar fehlt"
        );
        assert_eq!(
            app.ui.hotbar_kombi,
            jjcad_core::persist::Kombination::STRG
        );

        // Ziffer 1 startet den Slot **dieser** Ebene.
        frame_mit_lage(
            &mut app,
            vec![druck(egui::Key::Num1)],
            egui::Modifiers::COMMAND,
        );
        assert_eq!(
            app.tool_id(),
            Some("draw.circle"),
            "die Ziffer startet nicht den Slot der Strg-Ebene"
        );
    }
}

#[cfg(test)]
mod tests_sonderzeichen {
    //! T0121: Umlaute und andere Sonderzeichen in der Tastaturbelegung.
    //!
    //! `egui::Key` ist die US-Lage und kennt kein `ä`, `ö`, `ü` — die Taste
    //! kommt als `Key::Quote`/`Key::Semicolon`/`Key::OpenBracket` an, das
    //! Zeichen nur als `Event::Text`. Die Belegung wird deshalb am **Zeichen**
    //! ausgewertet; damit gilt sie für jedes Sonderzeichen, nicht nur für die
    //! drei Umlaute.

    use super::*;

    fn frame(app: &mut App, events: Vec<egui::Event>) {
        frame_lage(app, events, egui::Modifiers::NONE);
    }

    fn frame_lage(app: &mut App, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            modifiers,
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// Bild mit laufender Zeit — der Dauerlauf des Schwenks rechnet mit der
    /// echten Bilddauer.
    fn frame_zeit(app: &mut App, t: &mut f64, events: Vec<egui::Event>) {
        let ctx = app.ctx.clone();
        *t += 1.0 / 60.0;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            time: Some(*t),
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// Ein Tastendruck, wie ihn egui-winit liefert: die physische Taste in
    /// US-Lage **und** das erzeugte Zeichen im selben Bild.
    fn druck_zeichen(key: egui::Key, zeichen: &str) -> Vec<egui::Event> {
        vec![
            egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Text(zeichen.to_string()),
        ]
    }

    fn neue_app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app
    }

    #[test]
    fn umlaut_startet_die_zugewiesene_funktion() {
        let mut app = neue_app();
        app.prefs.bind("draw.wall", "ä");
        assert_eq!(
            app.prefs.action_for_key("ä").as_deref(),
            Some("draw.wall"),
            "die Belegung nimmt den Umlaut an"
        );
        frame(&mut app, druck_zeichen(egui::Key::Quote, "ä"));
        assert_eq!(
            app.tool_id(),
            Some("draw.wall"),
            "die auf ä gelegte Funktion muss beim Druck starten"
        );
    }

    #[test]
    fn alle_drei_umlaute_starten_ihre_funktion() {
        for (key, zeichen, funktion) in [
            (egui::Key::Quote, "ä", "draw.wall"),
            (egui::Key::Semicolon, "ö", "draw.line"),
            (egui::Key::OpenBracket, "ü", "draw.circle"),
        ] {
            let mut app = neue_app();
            app.prefs.bind(funktion, zeichen);
            frame(&mut app, druck_zeichen(key, zeichen));
            assert_eq!(
                app.tool_id(),
                Some(funktion),
                "{zeichen} muss {funktion} starten"
            );
        }
    }

    /// Die Lösung hängt am Zeichen, nicht an einer Tabelle: ein Zeichen, das
    /// auf keiner deutschen Taste liegt, funktioniert genauso.
    #[test]
    fn anderes_sonderzeichen_startet_ebenfalls() {
        let mut app = neue_app();
        app.prefs.bind("draw.line", "ß");
        frame(&mut app, druck_zeichen(egui::Key::Minus, "ß"));
        assert_eq!(app.tool_id(), Some("draw.line"), "ß muss die Funktion starten");
    }

    /// Ein ausdrücklich belegtes Sonderzeichen schlägt die Taste, über die es
    /// getippt wird: AltGr+E liefert `€`, nicht „e".
    #[test]
    fn belegtes_sonderzeichen_schlaegt_die_grundtaste() {
        let mut app = neue_app();
        app.prefs.bind("draw.wall", "e");
        app.prefs.bind("draw.line", "€");
        frame(&mut app, druck_zeichen(egui::Key::E, "€"));
        assert_eq!(
            app.tool_id(),
            Some("draw.line"),
            "€ ist belegt und muss die Grundtaste e verdrängen"
        );
    }

    #[test]
    fn umlaut_ohne_belegung_startet_nichts() {
        let mut app = neue_app();
        assert_eq!(app.prefs.action_for_key("ä"), None);
        frame(&mut app, druck_zeichen(egui::Key::Quote, "ä"));
        assert_eq!(app.tool_id(), None, "ohne Belegung darf ä nichts starten");
    }

    /// Die Umschalt-Ebene gilt auch für Sonderzeichen: `Umschalt+ä` ist eine
    /// eigene Belegung, genau wie `Umschalt+a` neben `a`.
    #[test]
    fn umschalt_umlaut_ist_eine_eigene_ebene() {
        let mut app = neue_app();
        app.prefs.bind("draw.wall", "ä");
        frame_lage(
            &mut app,
            druck_zeichen(egui::Key::Quote, "Ä"),
            egui::Modifiers::SHIFT,
        );
        assert_eq!(
            app.tool_id(),
            None,
            "Umschalt+ä ist nicht die Belegung von ä"
        );

        let mut zweite = neue_app();
        zweite.prefs.bind("draw.line", "shift+ä");
        frame_lage(
            &mut zweite,
            druck_zeichen(egui::Key::Quote, "Ä"),
            egui::Modifiers::SHIFT,
        );
        assert_eq!(
            zweite.tool_id(),
            Some("draw.line"),
            "Umschalt+ä muss seine eigene Belegung starten"
        );
    }

    /// Drehen der Einfüge-Vorschau hängt an der Funktion, nicht an `R` — das
    /// gilt auch, wenn sie auf einem Umlaut liegt. Der Druck darf das Einfügen
    /// nicht abbrechen (T0094).
    #[test]
    fn umlaut_dreht_die_einfuege_vorschau() {
        let mut app = neue_app();
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [2.0, 0.0]}),
            )
            .expect("Linie anlegen");
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        app.copy_selection(false);
        app.pointer_world = Some(V2::new(0.0, 0.0));
        app.start_ghost_paste();
        app.prefs.bind("modify.rotate", "ä");
        let vorher = app.ghost.as_ref().map(|g| g.rot).unwrap_or(0.0);
        frame(&mut app, druck_zeichen(egui::Key::Quote, "ä"));
        let nachher = app.ghost.as_ref().map(|g| g.rot).unwrap_or(0.0);
        assert!(
            (nachher - vorher).abs() > 0.1,
            "ä muss die Vorschau drehen: {vorher} → {nachher}"
        );
        assert!(
            app.ghost.is_some(),
            "Drehen darf das Einfügen nicht abbrechen"
        );
    }

    /// Der Dauerlauf des Schwenks liest `keys_down` und braucht die physische
    /// Taste. Sie wird beim Tippen gelernt (T0121) — sonst bliebe eine auf ä
    /// gelegte Schwenkfunktion im Modus „Kontinuierlich" wirkungslos.
    #[test]
    fn gehaltener_umlaut_schwenkt_weiter() {
        let mut app = neue_app();
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [2000.0, 0.0]}),
            )
            .expect("Linie anlegen");
        app.prefs.pan_mode = jjcad_core::persist::PanMode::Kontinuierlich;
        app.prefs.pan_speed = 1.0;
        app.prefs.bind("view.pan_up", "ä");
        app.prefs.unbind_key("w");
        let mut t = 0.0;
        frame_zeit(&mut app, &mut t, vec![]);
        let vorher = app.current_view().center;
        frame_zeit(&mut app, &mut t, druck_zeichen(egui::Key::Quote, "ä"));
        let nach_druck = app.current_view().center;
        assert!(
            nach_druck.y > vorher.y + 1.0,
            "der Druck auf ä muss sofort schieben: {vorher:?} → {nach_druck:?}"
        );
        for _ in 0..10 {
            frame_zeit(&mut app, &mut t, vec![]);
        }
        let nach_halten = app.current_view().center;
        assert!(
            nach_halten.y - nach_druck.y > 200.0,
            "gehalten muss die Ansicht weiterlaufen (gemessen {})",
            nach_halten.y - nach_druck.y
        );
    }

    /// Gegenprobe zur Zeichenauswertung: ein gewöhnlicher Buchstabe läuft
    /// weiter über seine Taste und wird nicht doppelt ausgewertet.
    #[test]
    fn buchstabe_startet_weiterhin_genau_eine_funktion() {
        let mut app = neue_app();
        app.prefs.bind("draw.wall", "a");
        frame(&mut app, druck_zeichen(egui::Key::A, "a"));
        assert_eq!(app.tool_id(), Some("draw.wall"));
    }
}

#[cfg(test)]
mod tests_schnellauswahl_schalter {
    //! T0060: Die Schnellauswahl ist **ein** Schalter auf `q`.
    //!
    //! Läuft ein Werkzeug, beendet der Druck es (Abbruch); läuft keines, wird
    //! die Schnellauswahl aktiv. Beides über die Taste und über die Action —
    //! Ribbon, Hotbar und Tastatur gehen denselben Weg (`App::run_action`).

    use super::*;

    fn frame_mit(app: &mut App, events: Vec<egui::Event>) {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    fn druck(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app
    }

    #[test]
    fn taste_q_bricht_ab_und_schaltet_wieder_ein() {
        let mut app = app();
        frame_mit(&mut app, vec![]);
        app.run_action(&app.ctx.clone(), "draw.wall");
        assert_eq!(app.tool_id(), Some("draw.wall"), "Wandwerkzeug läuft");

        // Erster Druck: Abbruch — danach ist keine Funktion aktiv.
        frame_mit(&mut app, vec![druck(egui::Key::Q)]);
        assert!(
            app.tool.is_none(),
            "q muss das laufende Werkzeug beenden: {:?}",
            app.tool_id()
        );

        // Zweiter Druck: die Schnellauswahl wird aktiv.
        frame_mit(&mut app, vec![druck(egui::Key::Q)]);
        assert_eq!(app.tool_id(), Some("modify.quickselect"), "Schnellauswahl aktiv");

        // Dritter Druck: wieder aus — sie ist kein Dauerzustand.
        frame_mit(&mut app, vec![druck(egui::Key::Q)]);
        assert!(app.tool.is_none(), "q schaltet die Schnellauswahl auch aus");
    }

    /// Die Schnellauswahl hat auf `q` ihre Vorgabe — auch nach dem Laden einer
    /// Belegung, in der noch die alte Funktion „Eigenschaften aufnehmen" stand.
    #[test]
    fn q_gehoert_der_schnellauswahl() {
        let app = app();
        assert_eq!(
            app.prefs.key_for("modify.quickselect").as_deref(),
            Some("q")
        );
        assert_eq!(
            app.prefs.action_for_key("q").as_deref(),
            Some("modify.quickselect")
        );
        assert!(
            actions::action("modify.pickprops").is_none(),
            "die alte Funktion „Eigenschaften aufnehmen“ gibt es nicht mehr"
        );
    }
}

#[cfg(test)]
mod tests_einfuegen_abbrechen {
    //! T0094: Wird während des Einfügens eine andere Funktion aktiviert, bricht
    //! das Einfügen ab.
    //!
    //! Vorher blieb die Vorschau stehen, während das Werkzeug daneben lief: der
    //! nächste Klick setzte die Kopie ab, statt den Punkt des Werkzeugs zu
    //! nehmen. Die Schnellauswahl ist dabei ein reiner **Abbruch** — wie bei
    //! jedem laufenden Werkzeug wird sie nicht selbst aktiv.

    use super::*;

    /// App mit einer Linie in der Zwischenablage und laufender Einfüge-Vorschau.
    fn app_mit_einfuegen() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        // Vorgabe-Belegung selbst setzen — die echte `prefs.json` des Rechners
        // darf den Prüfpunkt nicht verschieben.
        app.prefs.reset_bindings();
        let id = app.doc_mut().new_id();
        let ebene = app.doc().current_layer;
        app.doc_mut().entities.push(Entity::new(
            id,
            ebene,
            EntityKind::Line {
                a: V2::new(0.0, 0.0),
                b: V2::new(1000.0, 0.0),
            },
        ));
        app.session_mut().select(vec![id]);
        let ids = app.selection().to_vec();
        app.merke_zwischenablage(&ids, None);
        app.pointer_world = Some(V2::new(3000.0, 2000.0));
        app.start_ghost_paste();
        assert!(app.ghost.is_some(), "die Einfüge-Vorschau muss laufen");
        app
    }

    /// Ein Werkzeug aus Ribbon, Hotbar oder Inventar beendet das Einfügen — es
    /// läuft allein weiter, die Vorschau ist weg.
    #[test]
    fn werkzeug_start_bricht_das_einfuegen_ab() {
        let mut app = app_mit_einfuegen();
        let ctx = app.ctx.clone();
        app.run_action(&ctx, "draw.wall");

        assert!(app.ghost.is_none(), "die Vorschau muss verschwunden sein");
        assert_eq!(
            app.tool_id(),
            Some("draw.wall"),
            "die gewählte Funktion läuft"
        );
        assert_eq!(
            app.doc().entities.len(),
            1,
            "abgebrochen heißt: nichts eingefügt"
        );
    }

    /// Die Schnellauswahl bricht das Einfügen ab und wird dabei nicht selbst
    /// aktiv — genau wie bei einem laufenden Werkzeug.
    #[test]
    fn schnellauswahl_bricht_das_einfuegen_ab() {
        let mut app = app_mit_einfuegen();
        let ctx = app.ctx.clone();
        app.run_action(&ctx, "modify.quickselect");

        assert!(app.ghost.is_none(), "die Vorschau muss verschwunden sein");
        assert!(
            app.tool.is_none(),
            "die Schnellauswahl wird beim Abbruch nicht aktiv: {:?}",
            app.tool_id()
        );
        assert_eq!(app.doc().entities.len(), 1, "nichts eingefügt");
    }

    /// Ohne Funktionswechsel bleibt die Vorschau stehen und setzt wie gewohnt
    /// ab — der Abbruch greift nur beim Wechsel.
    #[test]
    fn ohne_funktionswechsel_setzt_der_klick_ab() {
        let mut app = app_mit_einfuegen();
        assert!(app.ghost.is_some(), "die Vorschau läuft weiter");
        assert!(app.commit_ghost(), "der Klick muss die Kopie absetzen");
        assert_eq!(app.doc().entities.len(), 2, "eine Kopie eingefügt");
    }

    fn frame_mit(app: &mut App, events: Vec<egui::Event>) {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    fn druck(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Die Taste der Schnellauswahl (`q`) bricht das Einfügen ab — sie wird
    /// dabei nicht selbst aktiv, genau wie bei einem laufenden Werkzeug.
    #[test]
    fn taste_der_schnellauswahl_bricht_das_einfuegen_ab() {
        let mut app = app_mit_einfuegen();
        assert_eq!(
            app.prefs.action_for_key("q").as_deref(),
            Some("modify.quickselect"),
            "Vorgabe: q gehört der Schnellauswahl"
        );
        frame_mit(&mut app, vec![druck(egui::Key::Q)]);
        assert!(app.ghost.is_none(), "q bricht das Einfügen ab");
        assert!(
            app.tool.is_none(),
            "die Schnellauswahl wird beim Abbruch nicht aktiv: {:?}",
            app.tool_id()
        );
    }

    /// Die Drehtaste dreht weiterhin **nur** die Vorschau: sie startet kein
    /// Werkzeug und bricht das Einfügen deshalb nicht ab.
    #[test]
    fn drehtaste_dreht_die_vorschau_ohne_abzubrechen() {
        let mut app = app_mit_einfuegen();
        frame_mit(&mut app, vec![druck(egui::Key::R)]);
        let g = app
            .ghost
            .as_ref()
            .expect("die Vorschau läuft nach dem Drehen weiter");
        assert_eq!(g.rot_deg(), 270.0, "R dreht im Uhrzeigersinn um 90°");
        assert!(
            app.tool.is_none(),
            "Drehen der Vorschau startet kein Werkzeug: {:?}",
            app.tool_id()
        );
    }

    /// T0237: „Drehen“ gehört der **Funktion**, nicht der Taste. Derselbe Weg
    /// wie der Ribbon-Knopf (der `start_tool` ruft) und ein Hotbar-Slot
    /// (`run_action`) dreht die laufende Vorschau — statt das Einfügen
    /// abzubrechen und das Drehwerkzeug zu starten.
    #[test]
    fn drehen_ueber_ribbon_und_hotbar_dreht_die_vorschau() {
        let mut app = app_mit_einfuegen();
        let ctx = app.ctx.clone();
        // Hotbar: die Ziffer startet die Funktion über `run_action`.
        app.run_action(&ctx, "modify.rotate");
        // Ribbon: die Schaltfläche ruft `start_tool` unmittelbar.
        app.start_tool("modify.rotate");

        let g = app
            .ghost
            .as_ref()
            .expect("die Vorschau muss nach dem Drehen weiterlaufen");
        assert_eq!(
            g.rot_deg(),
            180.0,
            "zwei Druck auf „Drehen“ drehen zweimal um 90° im Uhrzeigersinn"
        );
        assert!(
            app.tool.is_none(),
            "die Funktion startet kein Werkzeug: {:?}",
            app.tool_id()
        );
        assert_eq!(
            app.doc().entities.len(),
            1,
            "abgebrochen heißt: nichts eingefügt"
        );
    }

    /// T0237: „Spiegeln“ ebenso — über Ribbon/Hotbar (`start_tool`), nicht nur
    /// über die Taste.
    #[test]
    fn spiegeln_ueber_ribbon_und_hotbar_spiegelt_die_vorschau() {
        let mut app = app_mit_einfuegen();
        let ctx = app.ctx.clone();
        app.run_action(&ctx, "modify.mirror");

        let g = app
            .ghost
            .as_ref()
            .expect("die Vorschau muss nach dem Spiegeln weiterlaufen");
        assert!(g.mirror, "die Vorschau ist gespiegelt");
        assert!(app.tool.is_none(), "kein Werkzeug gestartet: {:?}", app.tool_id());
    }
}

#[cfg(test)]
mod tests_tuer_einfuegen {
    //! T0191: Eine aus der Zwischenablage eingefügte Tür hängt an ihrer Wand.
    //!
    //! `App::commit_ghost` schreibt die Kopien unmittelbar in die Zeichnung —
    //! ohne die Nachführung `Session::oeffnungen_auf_wand` blieb die Kopie an
    //! der Stelle des Zeigers stehen und ihr Basispunkt lag neben der Wand.

    use super::*;

    #[test]
    fn eingefuegte_tuer_bleibt_auf_der_wandkante() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &serde_json::json!({"input_unit": "mm"}))
            .expect("Einheit setzen");
        let w = app
            .session_mut()
            .execute(
                "draw.wall",
                &serde_json::json!({"a": [0, 0], "b": [6000, 0], "thickness": 300}),
            )
            .expect("Wand anlegen")["created"][0]
            .as_u64()
            .expect("Wand-Id") as Id;
        let t = app
            .session_mut()
            .execute(
                "draw.door",
                &serde_json::json!({"center": [2000, 0], "width": 900, "host": [w]}),
            )
            .expect("Tür anlegen")["created"][0]
            .as_u64()
            .expect("Tür-Id") as Id;

        app.session_mut().select(vec![t]);
        let ids = app.selection().to_vec();
        app.merke_zwischenablage(&ids, None);
        // Zeiger weit neben der Wand: die Vorschau hängt dort.
        app.pointer_world = Some(V2::new(3000.0, 2500.0));
        app.start_ghost_paste();
        assert!(app.commit_ghost(), "der Klick setzt die Kopie ab");

        let kopie = app
            .doc()
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Door { .. }))
            .map(|e| e.id)
            .find(|id| *id != t)
            .expect("keine Kopie der Tür");
        let bp = jjcad_core::render::door_base_point(app.doc().entity(kopie).unwrap())
            .expect("Basispunkt");
        assert!(
            (bp.y - 150.0).abs() < 0.5,
            "Basispunkt der Kopie muss auf der Wandkante liegen: {bp:?}"
        );
        match &app.doc().entity(kopie).unwrap().kind {
            EntityKind::Door { at, center, .. } => {
                assert!(at.is_some(), "die Kopie hat eine Längslage in der Wand");
                assert!(
                    (center.y - 150.0).abs() < 0.5,
                    "die Kopie sitzt auf der Wandkante: center={center:?}"
                );
            }
            _ => panic!("keine Tür"),
        }
    }
}

#[cfg(test)]
mod tests_vollbild {
    //! T0008: Vollbild (rahmenloses Fenster) über F11 und über den Knopf im
    //! Reiter „Ansicht".
    //!
    //! Geprüft wird der Befehl, den eframe/winit ausführt
    //! (`ViewportCommand::Fullscreen`) — im Test gibt es kein echtes Fenster.

    use super::*;

    fn frame_mit(
        ctx: &egui::Context,
        app: &mut App,
        events: Vec<egui::Event>,
        fenster_vollbild: Option<bool>,
    ) -> Vec<egui::ViewportCommand> {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        if let Some(v) = fenster_vollbild {
            raw.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    fullscreen: Some(v),
                    ..Default::default()
                },
            );
        }
        let out = ctx.run(raw, |ctx| app.ui(ctx));
        out.viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.commands.clone())
            .unwrap_or_default()
    }

    fn f11() -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::F11,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    /// Nur die Vollbild-Befehle aus der Befehlsliste.
    fn vollbild(cmds: &[egui::ViewportCommand]) -> Vec<bool> {
        cmds.iter()
            .filter_map(|c| match c {
                egui::ViewportCommand::Fullscreen(v) => Some(*v),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn f11_schaltet_vollbild_an_und_wieder_aus() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());

        let cmds = frame_mit(&ctx, &mut app, f11(), None);
        assert_eq!(
            vollbild(&cmds),
            vec![true],
            "F11 muss Vollbild einschalten (Befehle: {cmds:?})"
        );
        assert!(app.ui.fullscreen);

        let cmds = frame_mit(&ctx, &mut app, f11(), None);
        assert_eq!(
            vollbild(&cmds),
            vec![false],
            "zweites F11 muss Vollbild ausschalten (Befehle: {cmds:?})"
        );
        assert!(!app.ui.fullscreen);
    }

    #[test]
    fn knopf_und_f11_nutzen_denselben_befehl() {
        // Der Knopf im Reiter „Ansicht" ruft genau diese Action auf.
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let cmds = frame_mit(&ctx, &mut app, Vec::new(), None);
        assert!(
            vollbild(&cmds).is_empty(),
            "ohne Klick/F11 darf kein Vollbildbefehl gehen (Befehle: {cmds:?})"
        );

        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
                ..Default::default()
            },
            |ctx| app.run_action(ctx, "view.fullscreen"),
        );
        let cmds = out
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.commands.clone())
            .unwrap_or_default();
        assert_eq!(
            vollbild(&cmds),
            vec![true],
            "Action view.fullscreen muss Vollbild einschalten (Befehle: {cmds:?})"
        );
        assert!(app.ui.fullscreen);
    }

    #[test]
    fn fensterzustand_ist_massgeblich() {
        // Wird das Vollbild von der Fensterverwaltung beendet, meldet das
        // Fenster „aus" — der nächste F11-Druck muss dann einschalten, nicht
        // ausschalten (sonst bliebe die Taste wirkungslos).
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.fullscreen = true;
        let cmds = frame_mit(&ctx, &mut app, f11(), Some(false));
        assert_eq!(
            vollbild(&cmds),
            vec![true],
            "nach Fenster-Rückmeldung ohne Vollbild muss F11 einschalten (Befehle: {cmds:?})"
        );

        // Umgekehrt: Fenster ist im Vollbild, der Knopf schaltet aus.
        let mut app = App::with_ctx(ctx.clone());
        let cmds = frame_mit(&ctx, &mut app, f11(), Some(true));
        assert_eq!(
            vollbild(&cmds),
            vec![false],
            "im Vollbild muss F11 ausschalten (Befehle: {cmds:?})"
        );
    }

    /// Der Reiter „Ansicht" ist durch die Gruppe „Fenster" breiter geworden.
    /// Er muss in die kleinste zulässige Fensterbreite passen (1080 Punkte,
    /// `with_min_inner_size`), sonst wird rechts „Zentrum" abgeschnitten.
    #[test]
    fn reiter_ansicht_passt_in_die_mindestbreite() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.ribbon = RibbonTab::Ansicht;
        let mut breite = 0.0;
        for _ in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1080.0, 700.0))),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
            breite = app.ui.last_ribbon_content_w;
        }
        assert!(
            breite <= 1080.0,
            "Reiter Ansicht braucht {breite} px und passt nicht in die Mindestbreite 1080 px"
        );
    }

    /// Der Reiter „Ändern" ist **breiter als die Mindestfensterbreite** — wie
    /// „Start" schon vor ihm. Gemessen: 1660 px mit dem Standardabstand und
    /// 1584 px mit 2 px (seit T0247 „Abrunden" und „Phase" in der Gruppe
    /// „Geometrie" stehen; davor 1513 px). Der engere Abstand (`item_spacing.x`
    /// im Ändern-Zweig) holt einen Teil zurück. Bei der Standardfensterbreite
    /// schneidet die letzte Gruppe („Rückgängig/Wiederholen") rechts ab — die
    /// beiden Knöpfe stehen zusätzlich in der Kopfzeile. Der Test hält den
    /// Ist-Stand als Riegel: ein weiterer Knopf fällt sofort auf.
    #[test]
    fn reiter_aendern_bleibt_in_seiner_breite() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.ribbon = RibbonTab::Aendern;
        let mut breite = 0.0;
        for _ in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1080.0, 700.0))),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
            breite = app.ui.last_ribbon_content_w;
        }
        assert!(
            breite <= 1584.0,
            "Reiter Ändern braucht {breite} px (Riegel 1584) — ein Knopf mehr schneidet rechts ab"
        );
    }

    /// Der Reiter „Start" ist **breiter als die Mindestfensterbreite** — das war
    /// er schon vor T0100 (1308 px bei 1080 px Fenster); „Bild einfügen" in der
    /// Gruppe „Ausgabe" kommt hinzu (gemessen 1383 px), seit T0127 „Datenordner"
    /// in der Gruppe „Projekt" (gemessen 1458 px), seit T0235 die geteilte
    /// PNG-Schaltfläche (74 px gegen 70 px, gemessen 1462 px). Der Test hält den
    /// Ist-Stand als Riegel: ein weiterer Knopf fällt sofort auf. Bei der
    /// Standard-Fensterbreite 1500 px ist alles sichtbar.
    #[test]
    fn reiter_start_bleibt_in_seiner_breite() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.ribbon = RibbonTab::Datei;
        let mut breite = 0.0;
        for _ in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1080.0, 700.0))),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
            breite = app.ui.last_ribbon_content_w;
        }
        assert!(
            breite <= 1462.0,
            "Reiter Start braucht {breite} px (Riegel 1462) — ein Knopf mehr schneidet rechts ab"
        );
    }

    /// Der Reiter „Zeichnen" trägt seit T0090 „Vereinigen" und „Schneiden" in
    /// der Gruppe „Füllen" und seit T0200 „Parallele" in „Grundelemente" — er
    /// muss weiter in die Mindestbreite passen (gemessen 1074 px mit dem
    /// engeren Abstand dieses Reiters).
    #[test]
    fn reiter_zeichnen_passt_in_die_mindestbreite() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.ui.ribbon = RibbonTab::Zeichnen;
        let mut breite = 0.0;
        for _ in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1080.0, 700.0))),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
            breite = app.ui.last_ribbon_content_w;
        }
        assert!(
            breite <= 1080.0,
            "Reiter Zeichnen braucht {breite} px und passt nicht in die Mindestbreite 1080 px"
        );
    }

    #[test]
    fn action_steht_im_reiter_ansicht_mit_f11() {
        let a = actions::action("view.fullscreen").expect("Action view.fullscreen fehlt");
        assert_eq!(a.label(), "Vollbild");
        assert_eq!(a.group, ActionGroup::Ansicht);
        assert_eq!(a.key, "f11");
        assert!(!a.tool, "Vollbild ist kein Zeichenwerkzeug");
        let im_reiter = actions::actions_in_group(ActionGroup::Ansicht)
            .iter()
            .any(|d| d.id == "view.fullscreen");
        assert!(
            im_reiter,
            "Action fehlt in der Gruppe Ansicht (Inventar, Hilfe, Ribbon)"
        );
    }
}

#[cfg(test)]
mod tests_text_ziehen {
    //! Text folgt seit T0063 derselben Einstellung wie jedes andere Objekt:
    //!
    //! Einstellungen → Bedienung → „Maus verschiebt“. Eingeschaltet wandert
    //! auch der Text mit der gedrückten Maustaste, ausgeschaltet **kein**
    //! Objekt — dann bleibt das Werkzeug „Verschieben“ der genaue Weg.
    //! Geprüft wird der Weg, den die Zeichenfläche beim Klick nimmt
    //! (`App::begin_move_drag_bei_klick`) — nicht nur ein Kennzeichen.

    use super::*;

    fn app_mit_text_und_linie() -> (App, Id, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute(
                "draw.text",
                &serde_json::json!({ "p": [0.0, 0.0], "text": "Hallo", "height": 250.0 }),
            )
            .expect("Text anlegen");
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({ "a": [0.0, 2000.0], "b": [1000.0, 2000.0] }),
            )
            .expect("Linie anlegen");
        let text = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Text { .. }))
            .expect("Text fehlt")
            .id;
        let linie = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Line { .. }))
            .expect("Linie fehlt")
            .id;
        (app, text, linie)
    }

    /// T0191: Der Mauszug schreibt die Objekte unmittelbar, an der
    /// Action-Schicht vorbei — ohne Nachführung wanderte der Basispunkt einer
    /// Tür beliebig von der Wandkante weg. Quer zur Wand rührt sie sich nicht
    /// mehr, längs wandert sie weiter (an den Wandenden begrenzt).
    #[test]
    fn mauszug_haelt_den_basispunkt_der_tuer_auf_der_wandkante() {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.mouse_move = true;
        app.session_mut()
            .execute("doc.settings", &serde_json::json!({"input_unit": "mm"}))
            .expect("Einheit setzen");
        let w = app
            .session_mut()
            .execute(
                "draw.wall",
                &serde_json::json!({"a": [0, 0], "b": [6000, 0], "thickness": 300}),
            )
            .expect("Wand anlegen")["created"][0]
            .as_u64()
            .expect("Wand-Id") as Id;
        let t = app
            .session_mut()
            .execute(
                "draw.door",
                &serde_json::json!({"center": [2000, 0], "width": 900, "host": [w]}),
            )
            .expect("Tür anlegen")["created"][0]
            .as_u64()
            .expect("Tür-Id") as Id;
        app.session_mut().select(vec![t]);

        // 2 m quer zur Wand ziehen: die Tür bleibt auf der Wandkante.
        assert!(app.begin_move_drag_bei_klick(t, V2::ZERO, 0.0), "Zug startet");
        app.update_move_drag(V2::new(0.0, 2000.0));
        let bp = jjcad_core::render::door_base_point(app.doc().entity(t).unwrap())
            .expect("Basispunkt");
        assert!(
            (bp.y - 150.0).abs() < 0.5,
            "Basispunkt muss auf der Wandkante bleiben: {bp:?}"
        );
        app.end_move_drag();

        // 1 m längs der Wand: die Tür wandert mit (Zentrum 2450 → 3450).
        assert!(app.begin_move_drag_bei_klick(t, V2::ZERO, 0.0), "Zug startet");
        app.update_move_drag(V2::new(1000.0, 0.0));
        app.end_move_drag();
        match &app.doc().entity(t).unwrap().kind {
            EntityKind::Door { at, center, .. } => {
                assert!(
                    (at.unwrap() - 3450.0).abs() < 0.5,
                    "längs der Wand wandert die Tür: at={at:?}"
                );
                assert!(
                    (center.y - 150.0).abs() < 0.5,
                    "und bleibt auf der Wandkante: center={center:?}"
                );
            }
            _ => panic!("keine Tür"),
        }
    }

    /// Abgeschaltet startet Ziehen auf dem ausgewählten Text kein Verschieben
    /// (T0063: dieselbe Einstellung wie für Linie, Wand, Treppe …).
    #[test]
    fn abgeschaltet_startet_text_kein_maus_verschieben() {
        let (mut app, text, _) = app_mit_text_und_linie();
        app.prefs.mouse_move = false;
        app.session_mut().select(vec![text]);

        let gestartet = app.begin_move_drag_bei_klick(text, V2::new(0.0, 0.0), 1.0);

        assert!(!gestartet, "Text darf bei abgeschalteter Einstellung nicht ziehen");
        assert!(app.move_drag.is_none(), "kein laufender Zug am Text");
        assert_eq!(app.selection(), &[text], "Text bleibt ausgewählt");
    }

    /// Eingeschaltet wandert der Text wie jedes andere Objekt mit der Maus.
    #[test]
    fn eingeschaltet_startet_text_maus_verschieben() {
        let (mut app, text, _) = app_mit_text_und_linie();
        app.prefs.mouse_move = true;
        assert!(app.prefs.mouse_move, "Vorgabe ist eingeschaltet");
        app.session_mut().select(vec![text]);

        let gestartet = app.begin_move_drag_bei_klick(text, V2::new(0.0, 0.0), 1.0);

        assert!(gestartet, "Text muss sich mit der Maus ziehen lassen");
        assert!(app.move_drag.is_some(), "Zug läuft");
    }

    /// Die Linie wandert weiterhin mit der Maus.
    #[test]
    fn linie_startet_weiterhin_maus_verschieben() {
        let (mut app, _, linie) = app_mit_text_und_linie();
        app.prefs.mouse_move = true;
        app.session_mut().select(vec![linie]);

        let gestartet = app.begin_move_drag_bei_klick(linie, V2::new(500.0, 2000.0), 1.0);

        assert!(gestartet, "Linie muss sich ziehen lassen");
        assert!(app.move_drag.is_some(), "Zug läuft");
    }

    /// Abgeschaltet gilt die Sperre für **alle** Objektarten gleich, nicht nur
    /// für Text: Linie, Wand, Treppe und Füllfläche starten dann keinen Zug.
    #[test]
    fn abgeschaltet_verschiebt_kein_objekt_mit_der_maus() {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.mouse_move = false;
        let mut ids = Vec::new();
        for (action, params) in [
            ("draw.line", serde_json::json!({"a": [0.0, 0.0], "b": [1000.0, 0.0]})),
            ("draw.wall", serde_json::json!({"a": [0.0, 0.0], "b": [3000.0, 0.0]})),
            ("draw.circle", serde_json::json!({"c": [0.0, 0.0], "r": 500.0})),
            ("draw.text", serde_json::json!({"p": [0.0, 0.0], "text": "Hallo", "height": 250.0})),
            ("draw.stair", serde_json::json!({"a": [0.0, 0.0], "b": [0.0, 4000.0], "width": 1000.0})),
        ] {
            let r = app
                .session_mut()
                .execute(action, &params)
                .unwrap_or_else(|e| panic!("{action}: {e}"));
            ids.push(r["created"][0].as_u64().expect("Id") as Id);
        }
        assert!(ids.len() >= 5);
        for id in ids {
            assert!(
                !app.maus_verschiebt(id),
                "Objekt {id} hängt noch am Mauszug"
            );
            app.session_mut().select(vec![id]);
            assert!(
                !app.begin_move_drag_bei_klick(id, V2::ZERO, 0.0),
                "Objekt {id} startet trotzdem einen Zug"
            );
            assert!(app.move_drag.is_none());
        }
    }
}

#[cfg(test)]
mod tests_chat_texte {
    //! Das Chat-Schema (T0009): jede Funktion bekommt eine Aufforderung und
    //! eine Ergebniszeile — kurz, mit demselben Werkzeugnamen wie im Register
    //! und in der Form `<Werkzeug> · <Schritt>`.

    use super::*;

    /// Name, der in der Aufforderung vor dem Trennpunkt steht.
    fn werkzeugname(id: &str) -> String {
        match id {
            "draw.fill" | "draw.fill.boundary" | "draw.fill.points" => "Füllfläche".into(),
            "block.insert" => "Block".into(),
            "sheet.viewport_tool" => "Ansichtsfenster".into(),
            // Die drei Maßketten-Modi sind **ein** Werkzeug mit drei
            // Schaltflächen — im Chat heißt es „Maßkette“ (T0154).
            "dim.chain" | "dim.chain.h" | "dim.chain.v" | "dim.chain.d" => "Maßkette".into(),
            other => actions::action(other).map(|d| d.label()).unwrap_or("?").to_string(),
        }
    }

    fn werkzeuge() -> Vec<Tool> {
        let mut out = Vec::new();
        for a in actions::ACTIONS.iter().filter(|a| a.tool) {
            for anzahl in [0usize, 1, 2, 3] {
                for selecting in [false, true] {
                    for phase in [ToolPhase::Collecting, ToolPhase::TextInput] {
                        out.push(Tool {
                            action: a.id.to_string(),
                            pts: vec![V2::ZERO; anzahl],
                            schritte: Vec::new(),
                            bogen: false,
                            obj: None,
                            obj2: None,
                            pick_a: None,
                            pick_b: None,
                            knoten: None,
                            selecting,
                            phase,
                        });
                    }
                }
            }
        }
        out
    }

    /// Jedes Werkzeug hat in jedem Schritt eine Aufforderung im Schema
    /// `<Werkzeug> · <Schritt>`, höchstens 48 Zeichen.
    #[test]
    fn jedes_werkzeug_hat_eine_kurze_aufforderung() {
        let opts = ToolOptions::from_doc(&Document::new());
        for t in werkzeuge() {
            let h = commands_hint(&t, &opts);
            assert_ne!(
                h, hinweis_unbekannt(),
                "{} ({} Punkte) hat keinen Hinweis",
                t.action,
                t.pts.len()
            );
            assert!(h.contains(" · "), "{}: kein Schema: {h}", t.action);
            assert!(
                h.chars().count() <= 48,
                "{}: {} Zeichen: {h}",
                t.action,
                h.chars().count()
            );
            let name = werkzeugname(&t.action);
            assert!(
                h.starts_with(&format!("{name} · ")),
                "{}: erwartet \"{name} · …\", war {h}",
                t.action
            );
        }
    }

    /// Jede platzierende Änderung meldet ein Ergebnis im Schema
    /// `<Objekt> · <Zustand>`, höchstens 30 Zeichen.
    #[test]
    fn ergebnisse_sind_kurz_und_einheitlich() {
        let mut geprueft = 0;
        for a in actions::ACTIONS.iter().filter(|a| a.tool) {
            let Some(m) = abschluss_meldung(a.id) else { continue };
            geprueft += 1;
            assert!(m.contains(" · "), "{}: kein Schema: {m}", a.id);
            assert!(
                m.chars().count() <= 30,
                "{}: {} Zeichen: {m}",
                a.id,
                m.chars().count()
            );
            let objekt = m.split(" · ").next().unwrap();
            // Erlaubt: das Ergebnisobjekt (meist der Werkzeugname), die
            // Auswahl oder ein einzelnes Objekt. Der Farbeimer erzeugt eine
            // Füllfläche, das Werkzeug heißt anders.
            let name = werkzeugname(a.id);
            let erlaubt = ["Auswahl", "Objekt", "Füllfläche", name.as_str()];
            assert!(
                erlaubt.contains(&objekt),
                "{}: Objekt \"{objekt}\" passt nicht zum Werkzeug",
                a.id
            );
        }
        assert!(geprueft >= 25, "zu wenige geprüfte Werkzeuge: {geprueft}");
    }

    /// Werkzeuge mit eigener, genauerer Meldung schweigen hier (sonst zwei
    /// Zeilen für einen Vorgang) — und umgekehrt.
    #[test]
    fn keine_doppelten_ergebniszeilen() {
        for id in [
            "modify.rotate",
            "modify.trim",
            "modify.extend",
            "modify.order",
            "modify.quickselect",
        ] {
            assert_eq!(abschluss_meldung(id), None, "{id} meldet doppelt");
        }
        for id in ["draw.wall", "draw.circle", "draw.polyline", "modify.move"] {
            assert!(abschluss_meldung(id).is_some(), "{id} meldet nichts");
        }
    }

    /// Aufforderungen sind Aufforderungen: kurz, Verb am Ende, keine
    /// Implementierungserklärungen in Klammern außer „Enter“.
    #[test]
    fn aufforderungen_sind_knapp() {
        let opts = ToolOptions::from_doc(&Document::new());
        for t in werkzeuge() {
            let h = commands_hint(&t, &opts);
            assert!(!h.contains('…'), "{}: Auslassung im Text: {h}", t.action);
            assert!(!h.contains("bitte"), "{}: Floskel: {h}", t.action);
        }
    }
}

#[cfg(test)]
mod tests_systemdialog_modal {
    //! Der Dateidialog des Systems hält die Anwendung an (T0015, Turn 2).
    //!
    //! Vorher lief der Dialog in einem eigenen Thread, die Zeichenfläche blieb
    //! aber bedienbar: man konnte hinter dem Dialog weiterzeichnen und Werkzeuge
    //! starten. Jetzt nimmt `App::ui` keine Eingaben mehr an, solange der
    //! Systemdialog offen ist — erst Wählen oder Abbrechen gibt die Anwendung
    //! wieder frei.

    use super::*;
    use crate::dialogs::FileMode;
    use crate::systemdialog::Systemdialog;
    use std::time::Duration;

    fn rahmen(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            events,
            ..Default::default()
        }
    }

    fn taste(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// App mit fest eingestellter Taste „l“ für das Linienwerkzeug.
    fn app() -> (egui::Context, App) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.bindings.insert("draw.line".into(), vec!["l".into()]);
        (ctx, app)
    }

    /// Ein Systemdialog, der nie zurückkommt (offen, aber ohne Portal).
    fn offener_dialog() -> (FileMode, Systemdialog) {
        (
            FileMode::Open,
            Systemdialog::mit_ausfuehrung(|| {
                std::thread::sleep(Duration::from_secs(30));
                None
            }),
        )
    }

    #[test]
    fn eingabe_ist_gesperrt_solange_der_systemdialog_offen_ist() {
        let (ctx, mut app) = app();
        app.ui.systemdialog = Some(offener_dialog());
        // Erst ein ruhiger Frame (Fenster einpassen), dann die Taste
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let _ = ctx.run(rahmen(vec![taste(egui::Key::L)]), |ctx| app.ui(ctx));
        assert!(
            app.tool.is_none(),
            "Taste „l“ hat hinter dem offenen Systemdialog ein Werkzeug gestartet ({:?})",
            app.tool.as_ref().map(|t| t.action.clone())
        );
        assert!(
            app.ui.systemdialog.is_some(),
            "der offene Systemdialog wurde ohne Nutzerentscheidung verworfen"
        );
    }

    #[test]
    fn ohne_systemdialog_wirkt_dieselbe_taste() {
        // Gegenprobe: derselbe Ablauf ohne offenen Dialog startet das Werkzeug.
        let (ctx, mut app) = app();
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let _ = ctx.run(rahmen(vec![taste(egui::Key::L)]), |ctx| app.ui(ctx));
        assert_eq!(
            app.tool.as_ref().map(|t| t.action.as_str()),
            Some("draw.line"),
            "Gegenprobe: ohne offenen Dialog muss „l“ das Linienwerkzeug starten"
        );
    }

    fn klick_ereignis(pos: egui::Pos2, gedrueckt: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: gedrueckt,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Zwei Klicks auf die Zeichenfläche mit dem Linienwerkzeug — über denselben
    /// Ereignisweg wie die echte Maus.
    fn zwei_klicks(ctx: &egui::Context, app: &mut App) -> usize {
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let mitte = app.ui.last_canvas.center();
        let ziel = [mitte, mitte + egui::vec2(120.0, 40.0)];
        for pos in ziel {
            let _ = ctx.run(
                rahmen(vec![egui::Event::PointerMoved(pos), klick_ereignis(pos, true)]),
                |ctx| app.ui(ctx),
            );
            let _ = ctx.run(rahmen(vec![klick_ereignis(pos, false)]), |ctx| app.ui(ctx));
        }
        app.doc().entities.len()
    }

    #[test]
    fn mausklicks_erreichen_die_zeichenflaeche_nicht() {
        let (ctx, mut app) = app();
        app.start_tool("draw.line");
        app.ui.systemdialog = Some(offener_dialog());
        assert_eq!(
            zwei_klicks(&ctx, &mut app),
            0,
            "hinter dem offenen Systemdialog wurde gezeichnet"
        );
    }

    #[test]
    fn gegenprobe_ohne_systemdialog_wird_gezeichnet() {
        let (ctx, mut app) = app();
        app.start_tool("draw.line");
        assert_eq!(
            zwei_klicks(&ctx, &mut app),
            1,
            "Gegenprobe: ohne offenen Dialog müssen zwei Klicks eine Linie ergeben"
        );
    }

    #[test]
    fn esc_gibt_einen_haengenden_systemdialog_frei() {
        let (ctx, mut app) = app();
        app.ui.systemdialog = Some(offener_dialog());
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let _ = ctx.run(rahmen(vec![taste(egui::Key::Escape)]), |ctx| app.ui(ctx));
        assert!(
            app.ui.systemdialog.is_none(),
            "ESC muss den hängenden Systemdialog freigeben, sonst bleibt die Anwendung gesperrt"
        );
        // … und danach ist die Anwendung wieder bedienbar
        let _ = ctx.run(rahmen(vec![taste(egui::Key::L)]), |ctx| app.ui(ctx));
        assert!(app.tool.is_some(), "nach ESC muss die Taste wieder wirken");
    }
}

mod tests_masskette_esc {
    //! T0154, Turn 5: **ESC bestätigt** die Maßketten-Eingabe.
    //!
    //! Gesetzt Punkte dürfen nicht verloren gehen, wenn man die Eingabe mit
    //! ESC abschließt — wie beim Text, wo ESC übernimmt. Geprüft am echten
    //! Tastendruck über `App::ui` (derselbe Weg wie im Betrieb) und direkt an
    //! `App::escape_pressed` für die Randfälle.

    use super::*;
    use jjcad_core::geom::v;

    fn rahmen(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            events,
            ..Default::default()
        }
    }

    fn taste(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn app_mit_kette() -> (egui::Context, App) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.start_tool("dim.chain.h");
        if let Some(t) = app.tool.as_mut() {
            t.pts.push(v(0.0, -900.0)); // Lage der Maßlinie
            t.pts.push(v(0.0, 0.0));
            t.pts.push(v(2000.0, 0.0));
        }
        (ctx, app)
    }

    /// ESC legt die Maßkette an, statt die Punkte wegzuwerfen.
    #[test]
    fn esc_uebernimmt_die_eingabe_der_masskette() {
        let (ctx, mut app) = app_mit_kette();
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let _ = ctx.run(rahmen(vec![taste(egui::Key::Escape)]), |ctx| app.ui(ctx));

        assert!(app.tool.is_none(), "ESC muss die Eingabe beenden");
        let kette = app
            .doc()
            .ents()
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Dim { .. }))
            .expect("ESC muss die Maßkette anlegen");
        match &kette.kind {
            EntityKind::Dim { pts, .. } => assert_eq!(pts.len(), 2, "{pts:?}"),
            _ => unreachable!(),
        }
    }

    /// Gegenprobe: ohne einen einzigen Klick bricht ESC weiter ab.
    #[test]
    fn esc_ohne_klick_bricht_ab() {
        let mut app = App::with_ctx(egui::Context::default());
        app.start_tool("dim.chain.h");
        app.escape_pressed();
        assert!(app.tool.is_none());
        assert!(
            !app.doc()
                .ents()
                .iter()
                .any(|e| matches!(e.kind, EntityKind::Dim { .. })),
            "ohne Punkte darf nichts entstehen"
        );
    }

    /// Nur die Lage der Maßlinie gesetzt: es gibt nichts zu bestätigen — das
    /// Werkzeug endet, ohne eine unbrauchbare Kette zu hinterlassen.
    #[test]
    fn esc_nach_der_masslinie_legt_nichts_an() {
        let mut app = App::with_ctx(egui::Context::default());
        app.start_tool("dim.chain.v");
        if let Some(t) = app.tool.as_mut() {
            t.pts.push(v(-800.0, 0.0));
        }
        app.escape_pressed();
        assert!(app.tool.is_none());
        assert!(!app
            .doc()
            .ents()
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Dim { .. })));
    }

    /// „Punkt hinzufügen“: ESC beendet die Reihe und lässt die gewählte
    /// Maßkette ausgewählt (kein „Abgebrochen“, keine leere Auswahl).
    #[test]
    fn esc_beendet_das_punktwerkzeug_ohne_auswahl_zu_loeschen() {
        let mut app = App::with_ctx(egui::Context::default());
        let antwort = app
            .session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0]],
                                    "line_pos": [1000.0, -500.0], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = antwort["created"][0].as_u64().unwrap();
        app.session_mut().select(vec![id]);
        app.start_tool("dim.point.add");
        if let Some(t) = app.tool.as_mut() {
            t.obj = Some(id);
        }

        app.escape_pressed();
        assert!(app.tool.is_none(), "ESC beendet das Werkzeug");
        assert_eq!(
            app.selection(),
            &[id],
            "die bearbeitete Maßkette bleibt ausgewählt"
        );
    }
}

mod tests_masskette_drehen {
    //! T0250: Während der Platzierung schaltet die Funktion „Drehen“
    //! (`modify.rotate`) zwischen waagerecht und senkrecht um, statt das
    //! Drehwerkzeug zu starten. Die gesetzten Punkte bleiben dabei stehen.

    use super::*;
    use jjcad_core::geom::v;

    fn app_mit_kette(action: &str) -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.start_tool(action);
        if let Some(t) = app.tool.as_mut() {
            t.pts.push(v(0.0, -900.0)); // Lage der Maßlinie
            t.pts.push(v(0.0, 0.0));
            t.pts.push(v(2000.0, 0.0));
        }
        app
    }

    /// Drehen schaltet den Modus um und lässt Maßlinie wie Messpunkte stehen;
    /// ein zweiter Druck schaltet zurück.
    #[test]
    fn drehen_schaltet_die_richtung_und_behaelt_die_punkte() {
        let mut app = app_mit_kette("dim.chain.h");
        let vorher = app.tool.as_ref().unwrap().pts.clone();

        app.start_tool("modify.rotate");
        let t = app.tool.as_ref().expect("die Maßkette muss weiterlaufen");
        assert_eq!(t.action, "dim.chain.v", "Drehen muss senkrecht machen");
        assert_eq!(t.pts, vorher, "gesetzte Punkte bleiben stehen");
        assert_eq!(
            app.options.dim_axis,
            Some(DimAxis::Y),
            "das Panel zeigt die neue Richtung"
        );

        app.start_tool("modify.rotate");
        assert_eq!(
            app.tool.as_ref().unwrap().action,
            "dim.chain.h",
            "ein zweiter Druck schaltet zurück"
        );
        assert_eq!(app.options.dim_axis, Some(DimAxis::X));
    }

    /// Der erste Klick setzt die Lage der Maßlinie; danach gedreht, wird die
    /// Kette in der neuen Richtung angelegt — gemessen am gespeicherten Winkel.
    #[test]
    fn die_gedrehte_kette_wird_senkrecht_angelegt() {
        let mut app = app_mit_kette("dim.chain.h");
        app.start_tool("modify.rotate");
        app.escape_pressed();

        let kette = app
            .doc()
            .ents()
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Dim { .. }))
            .expect("ESC muss die Maßkette anlegen");
        match &kette.kind {
            EntityKind::Dim { pts, angle, .. } => {
                assert_eq!(pts.len(), 2, "{pts:?}");
                assert!(
                    (*angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
                    "die gedrehte Kette steht senkrecht, Winkel {angle}"
                );
            }
            _ => unreachable!(),
        }
    }

    /// Gegenprobe: „Direkt“ hat keine feste Richtung — dort startet Drehen
    /// wie bisher das Drehwerkzeug (kein stiller Moduswechsel).
    #[test]
    fn direktes_mass_bleibt_unberuehrt() {
        let mut app = app_mit_kette("dim.chain.d");
        app.start_tool("modify.rotate");
        assert_eq!(app.tool.as_ref().unwrap().action, "modify.rotate");
    }

    /// Der Weg des Nutzers: `R` ist die Vorgabe der Funktion „Drehen“, der
    /// Druck läuft über `run_action` → `start_tool`. Geprüft am echten
    /// Tastendruck über `App::ui` (derselbe Weg wie im Betrieb).
    #[test]
    fn die_taste_r_dreht_die_laufende_masskette() {
        fn rahmen(events: Vec<egui::Event>) -> egui::RawInput {
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                events,
                ..Default::default()
            }
        }
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.start_tool("dim.chain.h");
        app.tool.as_mut().unwrap().pts = vec![v(0.0, -900.0), v(0.0, 0.0), v(2000.0, 0.0)];

        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let _ = ctx.run(
            rahmen(vec![egui::Event::Key {
                key: egui::Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]),
            |ctx| app.ui(ctx),
        );

        assert_eq!(
            app.tool.as_ref().map(|t| t.action.as_str()),
            Some("dim.chain.v"),
            "R muss die Maßkette drehen, nicht das Drehwerkzeug starten"
        );
    }
}

mod tests_raumstempel_esc {
    //! T0220: **ESC bestätigt** die Punktfolge des Raumstempels.
    //!
    //! Gesetzte Raumecken dürfen nicht verloren gehen, wenn man die Eingabe mit
    //! ESC abschließt — wie bei Polylinie und Maßkette. Ohne einen einzigen
    //! Klick bricht ESC weiterhin ab.

    use super::*;
    use jjcad_core::geom::v;

    fn rahmen(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            events,
            ..Default::default()
        }
    }

    fn taste(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Oberfläche mit installierter Schrift (der Stempel malt Text).
    fn app_mit_ecken() -> (egui::Context, App) {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let _ = ctx.run(rahmen(vec![]), |_| {});
        let mut app = App::with_ctx(ctx.clone());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.start_tool("dim.room");
        if let Some(t) = app.tool.as_mut() {
            t.pts.push(v(0.0, 0.0));
            t.pts.push(v(4000.0, 0.0));
            t.pts.push(v(4000.0, 3000.0));
            t.pts.push(v(0.0, 3000.0));
        }
        (ctx, app)
    }

    fn stempel(app: &App) -> Option<Vec<jjcad_core::geom::Vec2>> {
        app.doc().ents().iter().find_map(|e| match &e.kind {
            EntityKind::Room { boundary, .. } => Some(boundary.clone()),
            _ => None,
        })
    }

    /// ESC legt den Stempel an, statt die Ecken wegzuwerfen.
    #[test]
    fn esc_uebernimmt_die_ecken_des_raumstempels() {
        let (ctx, mut app) = app_mit_ecken();
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let _ = ctx.run(rahmen(vec![taste(egui::Key::Escape)]), |ctx| app.ui(ctx));

        assert!(app.tool.is_none(), "ESC muss die Eingabe beenden");
        let boundary = stempel(&app).expect("ESC muss den Stempel anlegen");
        assert_eq!(boundary.len(), 4, "vier Ecken: {boundary:?}");
    }

    /// Gegenprobe: ohne eine einzige Ecke bricht ESC weiter ab.
    #[test]
    fn esc_ohne_ecken_bricht_ab() {
        let mut app = App::with_ctx(egui::Context::default());
        app.start_tool("dim.room");
        app.escape_pressed();
        assert!(app.tool.is_none());
        assert!(stempel(&app).is_none(), "ohne Ecken darf nichts entstehen");
    }
}

mod tests_kopieren {
    //! T0012: zwei Kopierfunktionen an einer geteilten Schaltfläche.
    //!
    //! Strg+C kopiert in die Zwischenablage (kein Werkzeug), Strg+Umschalt+C
    //! startet „Kopieren mit Basispunkt". Beides kommt bei egui als
    //! `Event::Copy` an — die Umschalttaste entscheidet.

    use super::*;

    fn app_mit_rechteck(auswahl: bool) -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        // Millimeter, damit die Koordinaten im Test direkt mm sind
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        let r = app
            .session_mut()
            .execute("draw.rect", &serde_json::json!({"a": [0, 0], "b": [1000, 1000]}))
            .expect("Rechteck");
        if auswahl {
            let id = r["created"][0].as_u64().unwrap();
            app.session_mut().select(vec![id]);
        }
        app
    }

    /// Ein Frame mit Kopier-Ereignis und den angegebenen Zusatztasten.
    fn kopier_frame(app: &mut App, shift: bool) {
        let ctx = app.ctx.clone();
        let mut mods = egui::Modifiers::COMMAND;
        mods.shift = shift;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            modifiers: mods,
            events: vec![egui::Event::Copy],
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    #[test]
    fn strg_c_fuellt_die_zwischenablage_ohne_werkzeug() {
        let mut app = app_mit_rechteck(true);
        kopier_frame(&mut app, false);
        assert_eq!(
            app.ui.clipboard.len(),
            1,
            "Strg+C muss die Auswahl in die Zwischenablage legen"
        );
        assert_eq!(app.tool_id(), None, "reines Kopieren startet kein Werkzeug");
    }

    #[test]
    fn strg_umschalt_c_startet_kopieren_mit_basispunkt() {
        let mut app = app_mit_rechteck(true);
        kopier_frame(&mut app, true);
        assert_eq!(
            app.tool_id(),
            Some("modify.copy"),
            "Strg+Umschalt+C startet Kopieren mit Basispunkt"
        );
        assert!(
            app.ui.clipboard.is_empty(),
            "der Start allein füllt noch nichts — erst das Absetzen legt die Kopie in die Zwischenablage"
        );
    }

    /// Umschließendes Rechteck der Zwischenablage-Elemente (Weltkoordinaten).
    fn zwischenablage_bbox(app: &App) -> jjcad_core::geom::BBox {
        let doc = app.doc();
        let mut bb = jjcad_core::geom::BBox::EMPTY;
        for e in &app.ui.clipboard.ents {
            bb.union(jjcad_core::render::entity_bbox(doc, e));
        }
        bb
    }

    /// Normales Kopieren setzt den Basispunkt automatisch auf die untere linke
    /// Ecke der Auswahl — genau das, was das Einfügen bisher gemacht hat.
    #[test]
    fn strg_c_setzt_den_basispunkt_links_unten() {
        let mut app = app_mit_rechteck(true);
        kopier_frame(&mut app, false);
        let bb = zwischenablage_bbox(&app);
        assert_eq!(
            app.ui.clipboard.base,
            Some(bb.min),
            "Basispunkt ist die untere linke Ecke der Auswahl"
        );
        assert_eq!(bb.min, V2::new(0.0, 0.0), "Rechteck beginnt im Ursprung");
    }

    /// Einfügen nach „Kopieren": die linke untere Ecke steht am Zeiger.
    #[test]
    fn einfuegen_nach_kopieren_setzt_die_ecke_an_den_zeiger() {
        let mut app = app_mit_rechteck(true);
        kopier_frame(&mut app, false);
        app.pointer_world = Some(V2::new(4000.0, 5000.0));
        app.start_ghost_paste();
        if let Some(g) = app.ghost.as_mut() {
            g.pos = V2::new(4000.0, 5000.0);
        }
        assert!(app.commit_ghost(), "Einfügen muss etwas anlegen");
        let neu = app.doc().entities.last().cloned().expect("eingefügtes Objekt");
        let bb = jjcad_core::render::entity_bbox(app.doc(), &neu);
        assert_eq!(bb.min, V2::new(4000.0, 5000.0), "Ecke unten links am Zeiger");
    }

    /// Ein einzelnes Textfeld führt seinen **Einfügepunkt** als Basispunkt mit:
    /// ein „Zentriert/Mitte" gesetzter Text liegt beim Einfügen genau mittig
    /// auf dem Zielpunkt, nicht mit seiner linken unteren Rahmenecke (T0028).
    #[test]
    fn kopierter_text_landet_mittig_auf_dem_zielpunkt() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.prefs.paste_pivot = jjcad_core::persist::PastePivot::BasePoint;
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        let r = app
            .session_mut()
            .execute(
                "draw.text",
                &serde_json::json!({"p": [1000, 500], "text": "Wohnen", "height": 250.0,
                                    "align": "center", "valign": "middle"}),
            )
            .expect("Text anlegen");
        let id = r["created"][0].as_u64().unwrap();
        app.session_mut().select(vec![id]);
        let ids = app.selection().to_vec();
        app.merke_zwischenablage(&ids, None);
        assert_eq!(
            app.ui.clipboard.base,
            Some(V2::new(1000.0, 500.0)),
            "Basispunkt eines einzelnen Textes ist sein Einfügepunkt"
        );

        app.pointer_world = Some(V2::new(4000.0, 3000.0));
        app.start_ghost_paste();
        if let Some(g) = app.ghost.as_mut() {
            g.pos = V2::new(4000.0, 3000.0);
        }
        assert!(app.commit_ghost(), "Einfügen muss einen Text anlegen");
        let neu = app.doc().entities.last().cloned().expect("eingefügter Text");
        let bb = jjcad_core::render::entity_bbox(app.doc(), &neu);
        assert!(
            (bb.center().x - 4000.0).abs() < 0.5 && (bb.center().y - 3000.0).abs() < 0.5,
            "Textmitte muss auf dem Zielpunkt liegen: {:?}",
            bb.center()
        );
    }

    /// Einfügen nach „Kopieren mit Basispunkt": der gemerkte Basispunkt steht am
    /// Zeiger, nicht die Ecke der Auswahl.
    #[test]
    fn einfuegen_nach_basispunkt_setzt_den_basispunkt_an_den_zeiger() {
        let mut app = app_mit_rechteck(true);
        app.prefs.paste_pivot = jjcad_core::persist::PastePivot::BasePoint;
        let ids = app.selection().to_vec();
        app.merke_zwischenablage(&ids, Some(V2::new(250.0, 100.0)));
        app.pointer_world = Some(V2::new(4000.0, 5000.0));
        app.start_ghost_paste();
        if let Some(g) = app.ghost.as_mut() {
            g.pos = V2::new(4000.0, 5000.0);
        }
        assert!(app.commit_ghost(), "Einfügen muss etwas anlegen");
        let neu = app.doc().entities.last().cloned().expect("eingefügtes Objekt");
        let bb = jjcad_core::render::entity_bbox(app.doc(), &neu);
        assert_eq!(
            bb.min,
            V2::new(3750.0, 4900.0),
            "Basispunkt (250,100) sitzt am Zeiger, die Ecke wandert mit"
        );
    }

    /// Das Aufklappmenü bietet beide Funktionen, jede mit eigener Aktion.
    #[test]
    fn menue_bietet_beide_kopierfunktionen() {
        let aktionen: Vec<&str> = CopyInput::ALL.iter().map(|m| m.action()).collect();
        assert_eq!(aktionen, ["edit.copy", "modify.copy"]);
        for m in CopyInput::ALL {
            assert!(
                jjcad_core::actions::action(m.action()).is_some(),
                "{} fehlt im Aktionsregister",
                m.action()
            );
            assert!(!m.label().is_empty() && !m.key_label().is_empty());
        }
        assert_eq!(CopyInput::Clipboard.label(), "Kopieren");
        assert_eq!(CopyInput::BasePoint.label(), "Kopieren mit Basispunkt");
        assert_ne!(
            CopyInput::Clipboard.icon(),
            CopyInput::BasePoint.icon(),
            "der Basispunkt braucht ein eigenes Icon"
        );
    }

    /// Vorgabe und gespeicherte Wahl: der Basispunkt-Modus ist voreingestellt
    /// (wie bisher im Reiter „Ändern"), die Wahl überlebt den Neustart.
    #[test]
    fn modus_ist_gemerkt_und_voreingestellt() {
        assert_eq!(UiState::default().copy_input, CopyInput::BasePoint);
        for m in CopyInput::ALL {
            assert_eq!(CopyInput::from_key(m.key()), m, "Schlüssel {}", m.key());
        }
    }
}

#[cfg(test)]
mod tests_block_direkt {
    //! T0142: Blöcke entstehen direkt, ohne Benennung.
    //!
    //! Strg+Umschalt+V macht aus der Zwischenablage einen Block und fügt ihn
    //! ein; ein Klick auf einen Hotbar-Slot während des Einfügens legt den
    //! Block auf dem Slot ab. Beide Wege vergeben einen Namen „Unbenannt N“
    //! ohne Dialog, der nicht mit vorhandenen Namen kollidiert.

    use super::*;

    fn app_mit_zwischenablage() -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        // Das Dokument kommt mit der Benutzerbibliothek des Rechners — hat der
        // Nutzer dort eigene „Unbenannt N“-Blöcke, zählen die Tests sie mit.
        // Für die Prüfung zählt nur, was der Test selbst anlegt; die Vorlagen
        // („WC“, „Tisch“, …) bleiben.
        app.doc_mut().blocks.retain(|b| !b.name.starts_with("Unbenannt "));
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        let r = app
            .session_mut()
            .execute("draw.rect", &serde_json::json!({"a": [0, 0], "b": [1000, 1000]}))
            .expect("Rechteck");
        let id = r["created"][0].as_u64().unwrap();
        app.session_mut().select(vec![id]);
        let ids = app.selection().to_vec();
        app.merke_zwischenablage(&ids, None);
        app
    }

    /// Die Benutzerbibliothek landet im Wegwerfordner (`App::test_blockordner`),
    /// nie in der Datenwurzel des Nutzers.
    fn wegwerf_blockordner(app: &mut App, name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "jjcad-t0142-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Nicht über `prefs.blocks_pfad`: das Speichern der Slots schriebe den
        // Wegwerfpfad in die `prefs.json` des Nutzers.
        app.test_blockordner = Some(dir.clone());
        app.test_prefs_speichern = false;
        dir
    }

    /// Nur die ohne Namen entstandenen Blöcke — die Bibliothek der Vorlagen
    /// („WC“, „Tisch“, …) steht in jedem neuen Dokument.
    fn unbenannte(app: &App) -> Vec<String> {
        app.doc()
            .blocks
            .iter()
            .filter(|b| b.name.starts_with("Unbenannt "))
            .map(|b| b.name.clone())
            .collect()
    }

    fn block_mit_namen(app: &App, name: &str) -> jjcad_core::model::BlockDef {
        app.doc()
            .blocks
            .iter()
            .find(|b| b.name == name)
            .cloned()
            .unwrap_or_else(|| panic!("Block '{name}' fehlt"))
    }

    fn paste_frame(app: &mut App, shift: bool) {
        let ctx = app.ctx.clone();
        let mut mods = egui::Modifiers::COMMAND;
        mods.shift = shift;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            modifiers: mods,
            events: vec![egui::Event::Paste(String::new())],
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// Strg+Umschalt+V legt einen unbenannten Block an, schreibt die
    /// Blockdatei und startet die Vorschau mit genau diesem Block.
    #[test]
    fn strg_umschalt_v_legt_einen_block_an_und_fuegt_ihn_ein() {
        let mut app = app_mit_zwischenablage();
        let dir = wegwerf_blockordner(&mut app, "shiftv");
        app.pointer_world = Some(V2::new(4000.0, 5000.0));
        paste_frame(&mut app, true);

        assert_eq!(unbenannte(&app), vec!["Unbenannt 1"], "genau ein Block entstanden");
        let block = block_mit_namen(&app, "Unbenannt 1");
        assert_eq!(
            app.block_einfuegen(),
            Some(block.id.as_str()),
            "die Vorschau ist der neue Block"
        );
        let dateien = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(dateien, 1, "der Block liegt als .jjblk in der Bibliothek");
        let (geladen, meldungen) = jjcad_core::bloecke::lade_bloecke(&dir);
        assert!(meldungen.is_empty(), "{meldungen:?}");
        assert_eq!(geladen.len(), 1, "die Datei ist wieder lesbar");
        assert_eq!(geladen[0].name, "Unbenannt 1");
        assert_eq!(geladen[0].entities.len(), 1, "das Rechteck steckt im Block");

        assert!(app.commit_ghost(), "der Klick setzt die Blockinstanz");
        assert!(
            app.doc().entities.iter().any(|e| matches!(
                &e.kind,
                jjcad_core::model::EntityKind::BlockRef { block: b, .. } if *b == block.id
            )),
            "eingefügt wird eine Blockinstanz, keine losen Objekte"
        );
    }

    /// Ohne Umschalt bleibt Strg+V beim gewohnten Einfügen der losen Objekte —
    /// es entsteht kein Block.
    #[test]
    fn strg_v_allein_legt_keinen_block_an() {
        let mut app = app_mit_zwischenablage();
        let _dir = wegwerf_blockordner(&mut app, "strgv");
        paste_frame(&mut app, false);
        assert!(unbenannte(&app).is_empty(), "kein Block ohne Umschalt");
        assert!(app.ghost.is_some(), "die Vorschau der Zwischenablage läuft");
        assert_eq!(app.block_einfuegen(), None);
    }

    /// Klick auf einen Hotbar-Slot während des Einfügens: aus der
    /// Zwischenablage wird ein Block, der Slot trägt ihn.
    #[test]
    fn slot_klick_erzeugt_block_aus_der_zwischenablage() {
        let mut app = app_mit_zwischenablage();
        let _dir = wegwerf_blockordner(&mut app, "slot");
        app.start_ghost_paste();
        assert!(app.ghost.is_some());
        assert!(app.block_einfuegen().is_none(), "noch kein Block");

        assert!(app.bind_slot_block(2), "der Slot nimmt den neuen Block auf");
        assert_eq!(unbenannte(&app), vec!["Unbenannt 1"]);
        let block = block_mit_namen(&app, "Unbenannt 1");
        assert_eq!(
            app.prefs.slots()[2].as_deref(),
            Some(format!("block:{}", block.id).as_str()),
            "der Slot verweist auf den Block"
        );
        assert!(app.ghost.is_none(), "das Einfügen ist beendet");
    }

    /// Ohne laufendes Einfügen erzeugt ein Slotklick keinen Block.
    #[test]
    fn slot_klick_ohne_einfuegen_erzeugt_keinen_block() {
        let mut app = app_mit_zwischenablage();
        let _dir = wegwerf_blockordner(&mut app, "ohne");
        assert!(!app.bind_slot_block(0));
        assert!(unbenannte(&app).is_empty());
    }

    /// Zweimal erzeugen ergibt zwei Namen: „Unbenannt 1“ und „Unbenannt 2“.
    #[test]
    fn zweiter_block_bekommt_einen_freien_namen() {
        let mut app = app_mit_zwischenablage();
        let _dir = wegwerf_blockordner(&mut app, "zaehler");
        assert!(app.block_aus_zwischenablage().is_some());
        assert!(app.block_aus_zwischenablage().is_some());
        assert_eq!(unbenannte(&app), vec!["Unbenannt 1", "Unbenannt 2"]);
    }
}

mod tests_einfuegemodus {
    //! Einfügemodus von Rechteck und Ellipse (T0024, Turn 2).
    //!
    //! Der Modus war zuerst nur im Eigenschaften-Panel zu finden — und dort
    //! erst, wenn das Werkzeug schon lief. Jetzt bietet der Pfeil am
    //! Ribbon-Knopf beide Modi an; dieser Test prüft den gemeinsamen Weg
    //! (`App::waehle_einfuegemodus`), den Menü und Panel benutzen.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    #[test]
    fn ribbon_menue_setzt_modus_und_startet_werkzeug() {
        let mut app = app();
        app.waehle_einfuegemodus("draw.rect", Einfuegemodus::Basislinie);
        assert_eq!(app.options.rect_mode, Einfuegemodus::Basislinie);
        assert_eq!(app.tool_id(), Some("draw.rect"), "Werkzeug startet mit");
        assert_eq!(
            commands_hint(app.tool.as_ref().unwrap(), &app.options),
            "Rechteck · Basislinie Anfang klicken",
            "die Aufforderung nennt den gewählten Modus"
        );

        // Ellipse hat einen eigenen Modus: die Wahl beim Rechteck bleibt.
        app.waehle_einfuegemodus("draw.ellipse", Einfuegemodus::Basislinie);
        assert_eq!(app.options.ellipse_mode, Einfuegemodus::Basislinie);
        assert_eq!(app.options.rect_mode, Einfuegemodus::Basislinie);
        assert_eq!(app.tool_id(), Some("draw.ellipse"));
        assert_eq!(
            commands_hint(app.tool.as_ref().unwrap(), &app.options),
            "Ellipse · Mittelachse Anfang klicken"
        );

        // Zurück auf den achsenparallelen Weg: zwei Klicks genügen wieder.
        app.waehle_einfuegemodus("draw.rect", Einfuegemodus::Achsenparallel);
        assert_eq!(
            commands_hint(app.tool.as_ref().unwrap(), &app.options),
            "Rechteck · erste Ecke klicken"
        );
    }

    /// Beide Modi tragen eine verständliche Beschriftung für ihr Werkzeug.
    #[test]
    fn beschriftungen_nennen_die_linie() {
        assert_eq!(Einfuegemodus::Achsenparallel.label("draw.rect"), "Über Ecken");
        assert_eq!(Einfuegemodus::Basislinie.label("draw.rect"), "Basislinie + Höhe");
        assert_eq!(
            Einfuegemodus::Achsenparallel.label("draw.ellipse"),
            "Über Mittelpunkt"
        );
        assert_eq!(
            Einfuegemodus::Basislinie.label("draw.ellipse"),
            "Mittelachse + Breite"
        );
    }
}

mod tests_leistung {
    //! T0068: Leistungsanzeige (Umschalt+F3).
    //!
    //! Geprüft wird der Weg über die Tastatur (feste Kombination) und dass die
    //! Anzeige nur dann gezeichnet wird, wenn sie offen ist. Die Aufteilung der
    //! Rechenzeit selbst prüft `perf::tests`.

    use super::*;

    /// Ein Bild wie in `App::update`: Messung beginnen, zeichnen, abschließen.
    ///
    /// Die Umschaltlage kommt in egui 0.29 aus `RawInput::modifiers` — aus den
    /// `Event::Key`-Einträgen allein liest sie niemand (siehe `InputState`).
    fn frame_mit(
        ctx: &egui::Context,
        app: &mut App,
        events: Vec<egui::Event>,
        mods: egui::Modifiers,
    ) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            modifiers: mods,
            ..Default::default()
        };
        app.perf.bild_beginnen();
        let out = ctx.run(raw, |ctx| app.ui(ctx));
        app.perf.bild_beenden();
        out
    }

    fn frame(ctx: &egui::Context, app: &mut App) -> egui::FullOutput {
        frame_mit(ctx, app, Vec::new(), egui::Modifiers::NONE)
    }

    /// Umschalt+F3 — die Umschaltlage steht zusätzlich im Rohzustand.
    fn umschalt_f3(ctx: &egui::Context, app: &mut App) -> egui::FullOutput {
        frame_mit(ctx, app, f3(true), egui::Modifiers::SHIFT)
    }

    fn f3(umschalt: bool) -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::F3,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if umschalt {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            },
        }]
    }

    /// Alle gemalten Texte eines Bildes.
    fn texte(out: &egui::FullOutput) -> Vec<String> {
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    /// Steht die Zeile im Bild? Die Einrückung der Anzeige zählt dabei nicht.
    fn hat(out: &egui::FullOutput, text: &str) -> bool {
        texte(out).iter().any(|t| t.trim() == text)
    }

    #[test]
    fn umschalt_f3_schaltet_die_anzeige_ein_und_aus() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        frame(&ctx, &mut app);
        assert!(!app.perf.an, "die Anzeige darf nicht von selbst offen sein");
        let out = frame(&ctx, &mut app);
        assert!(!hat(&out, "Leistung"), "ohne Umschalt+F3 darf nichts stehen");

        let _ = umschalt_f3(&ctx, &mut app);
        assert!(app.perf.an, "Umschalt+F3 muss die Anzeige öffnen");
        // Ein frisches `egui::Area` kennt seine Größe erst nach einem Bild — im
        // Auslösebild bleibt es leer (gemessen), deshalb das nächste abwarten.
        let out = frame(&ctx, &mut app);
        assert!(hat(&out, "Leistung"), "die Anzeige muss im Bild stehen");
        // Je Zeile der Aufschlüsselung eine Sonde — „Zeichnen" allein wäre
        // nicht eindeutig (so heißt auch eine Registerkarte).
        for name in [
            "Abstand",
            "Rechnen",
            "außerhalb",
            "Displayliste",
            "Fangen",
            "Bedienung",
            "Fenster",
            "Füllungen",
            "Linien",
        ] {
            assert!(hat(&out, name), "Zeile „{name}\" fehlt in der Anzeige");
        }

        let _ = umschalt_f3(&ctx, &mut app);
        assert!(!app.perf.an, "zweites Umschalt+F3 muss die Anzeige schließen");
        let out = frame(&ctx, &mut app);
        assert!(!hat(&out, "Leistung"), "geschlossen darf nichts stehen bleiben");
    }

    #[test]
    fn f3_ohne_umschalt_schaltet_nichts() {
        // F3 allein ist keine Belegung: die Kombination ist die Funktion.
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        frame(&ctx, &mut app);
        let out = frame_mit(&ctx, &mut app, f3(false), egui::Modifiers::NONE);
        assert!(!app.perf.an, "F3 ohne Umschalt darf die Anzeige nicht öffnen");
        assert!(!hat(&out, "Leistung"));
    }

    #[test]
    fn anzeige_steht_ueber_der_zeichenflaeche_und_bleibt_im_bild() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.perf.an = true;
        for _ in 0..3 {
            frame(&ctx, &mut app);
        }
        // Die Anzeige liegt in der Zeichenfläche (nicht über Ribbon oder Panel)
        // und wird nicht abgeschnitten: ihre Textzeilen liegen im Fenster.
        let out = frame(&ctx, &mut app);
        let flaeche = app.ui.last_canvas;
        let fenster = Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0));
        let mut gefunden = false;
        for cs in &out.shapes {
            if let egui::Shape::Text(t) = &cs.shape {
                if t.galley.text() == "Leistung" {
                    gefunden = true;
                    let kasten = t.galley.rect.translate(t.pos.to_vec2());
                    assert!(
                        flaeche.contains_rect(kasten),
                        "die Anzeige steht bei {kasten:?} außerhalb der Zeichenfläche {flaeche:?}"
                    );
                    assert!(fenster.contains_rect(kasten));
                }
            }
        }
        assert!(gefunden, "die Anzeige fehlt im Bild");
    }

    #[test]
    fn spalten_springen_nicht_beim_wechsel_der_herkunft() {
        // T0068, Turn 2: Die Herkunft der Displayliste („neu aufgebaut“ /
        // „aus dem Zwischenspeicher“) stand als eigene Zeile **in** der Tabelle.
        // Der längere Text bestimmte die Breite der Namensspalte, und beim
        // Wechsel wanderte die ganze Tabelle — im Bild ein Flackern.
        //
        // Gemessen werden zwei Läufe: einer baut die Liste in **jedem** Bild neu
        // auf (Herkunft dauerhaft „neu aufgebaut“), der andere zeichnet aus dem
        // Zwischenspeicher. Die Spaltenköpfe müssen in beiden an derselben
        // Stelle stehen.
        let x = |out: &egui::FullOutput, text: &str| -> f32 {
            out.shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) if t.galley.text().trim() == text => Some(t.pos.x),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Spaltenkopf „{text}“ fehlt"))
        };

        // Lauf „neu aufgebaut“: jeder Frame verwirft den Zwischenspeicher.
        let ctx_neu = egui::Context::default();
        let mut app_neu = App::with_ctx(ctx_neu.clone());
        app_neu.perf.an = true;
        let mut neu = None;
        for _ in 0..4 {
            app_neu.tab_mut().cache = None;
            neu = Some(frame(&ctx_neu, &mut app_neu));
        }
        let neu = neu.expect("Bild gezeichnet");
        assert!(
            hat(&neu, "Displayliste: neu aufgebaut"),
            "der Neubau wird nicht angezeigt"
        );

        // Lauf „aus dem Zwischenspeicher“: nach dem ersten Bild bleibt der Cache.
        let ctx_kopie = egui::Context::default();
        let mut app_kopie = App::with_ctx(ctx_kopie.clone());
        app_kopie.perf.an = true;
        let mut kopie = None;
        for _ in 0..4 {
            kopie = Some(frame(&ctx_kopie, &mut app_kopie));
        }
        let kopie = kopie.expect("Bild gezeichnet");
        assert!(
            hat(&kopie, "Displayliste: aus dem Zwischenspeicher"),
            "die Herkunft der Liste fehlt"
        );

        for spalte in ["jetzt", "Anteil", "p50", "p95", "p99"] {
            assert_eq!(
                x(&kopie, spalte),
                x(&neu, spalte),
                "Spalte „{spalte}“ springt mit der Herkunft ({} gegen {} px)",
                x(&kopie, spalte),
                x(&neu, spalte)
            );
        }
        for name in ["Displayliste", "Oberfläche", "Fangen"] {
            assert_eq!(
                x(&kopie, name),
                x(&neu, name),
                "Zeile „{name}“ wandert mit der Herkunft"
            );
        }
    }

    #[test]
    fn anzeige_faengt_keine_klicks() {
        // Sie liegt über der Zeichnung: ein Klick darauf muss die Zeichenfläche
        // erreichen, sonst ist ein Stück Plan nicht mehr bedienbar.
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.perf.an = true;
        for _ in 0..3 {
            frame(&ctx, &mut app);
        }
        let _ = frame(&ctx, &mut app);
        let flaeche = app.ui.last_canvas;
        let in_der_anzeige = egui::pos2(flaeche.right() - 120.0, flaeche.top() + 90.0);
        let freie_flaeche = egui::pos2(flaeche.left() + 60.0, flaeche.bottom() - 60.0);
        assert_eq!(
            ctx.layer_id_at(in_der_anzeige).map(|l| l.order),
            ctx.layer_id_at(freie_flaeche).map(|l| l.order),
            "die Leistungsanzeige zieht eine eigene Klick-Ebene über die Zeichnung"
        );
    }

}


#[cfg(test)]
mod tests_drehpunkt_einfuegen {
    //! T0070: Zwei Drehpunkte für das Einfügen aus der Zwischenablage.
    //!
    //! Vorgabe bleibt der **Basispunkt** am Fadenkreuz (bisheriges Verhalten).
    //! Im Modus „Mittelpunkt" steht die untere linke Ecke der gedrehten Auswahl
    //! am Fadenkreuz: die Vorschau dreht um ihren eigenen Mittelpunkt, springt
    //! nicht mehr um das Fadenkreuz und bleibt immer oben rechts davon. Geprüft
    //! wird an der Vorschau **und** am eingefügten Ergebnis — beide müssen
    //! dieselbe Lage haben.

    use super::*;

    fn rechteck_app(pivot: jjcad_core::persist::PastePivot) -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.prefs.paste_pivot = pivot;
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        let r = app
            .session_mut()
            .execute(
                "draw.rect",
                &serde_json::json!({"a": [0, 0], "b": [2000, 800]}),
            )
            .expect("Rechteck");
        let id = r["created"][0].as_u64().unwrap();
        app.session_mut().select(vec![id]);
        app
    }

    fn bbox_aller(app: &App, ids: &[Id]) -> BBox {
        let mut bb = BBox::EMPTY;
        for id in ids {
            if let Some(e) = app.doc().entity(*id) {
                bb.union(jjcad_core::render::entity_bbox(app.doc(), &e));
            }
        }
        bb
    }

    /// Vorschau-Lage bei einer Vierteldrehung.
    fn vorschau_bbox(app: &App) -> BBox {
        let g = app.ghost.as_ref().expect("Vorschau offen");
        let mut bb = BBox::EMPTY;
        for e in crate::ghost::clipboard_entities(app.doc(), g, &app.ui.clipboard) {
            bb.union(jjcad_core::render::entity_bbox(app.doc(), &e));
        }
        bb
    }

    #[test]
    fn mittelpunkt_modus_haelt_die_vorschau_oben_rechts() {
        let mut app = rechteck_app(jjcad_core::persist::PastePivot::Center);
        app.copy_selection(false);
        app.start_ghost_paste();
        let zeiger = V2::new(6000.0, 4000.0);
        if let Some(g) = app.ghost.as_mut() {
            g.pos = zeiger;
        }
        app.rotate_ghost(App::ghost_drehgrad(false)); // R = im Uhrzeigersinn
        let bb = vorschau_bbox(&app);
        assert!(
            (bb.min.x - zeiger.x).abs() < 1e-6 && (bb.min.y - zeiger.y).abs() < 1e-6,
            "nach dem Drehen muss die untere linke Ecke am Fadenkreuz stehen, sie stand \
             bei {:?} (Fadenkreuz {zeiger:?})",
            bb.min
        );
        assert!(
            bb.max.x > zeiger.x + 1.0 && bb.max.y > zeiger.y + 1.0,
            "die Vorschau steht nicht oben rechts vom Fadenkreuz: {bb:?}"
        );

        // Eingefügt landet sie an derselben Stelle.
        assert!(app.commit_ghost(), "Einfügen muss etwas anlegen");
        let ids = app.selection().to_vec();
        let eingefuegt = bbox_aller(&app, &ids);
        assert!(
            (eingefuegt.min.x - zeiger.x).abs() < 1e-6
                && (eingefuegt.min.y - zeiger.y).abs() < 1e-6,
            "das eingefügte Ergebnis steht nicht dort, wo die Vorschau stand: {:?}",
            eingefuegt.min
        );
    }

    /// Gegenprobe: der Basispunkt-Modus (Vorgabe) dreht weiter um das
    /// Fadenkreuz — die Ecke springt beim Drehen weg.
    #[test]
    fn basispunkt_modus_dreht_weiter_um_das_fadenkreuz() {
        let mut app = rechteck_app(jjcad_core::persist::PastePivot::BasePoint);
        app.copy_selection(false);
        app.start_ghost_paste();
        let zeiger = V2::new(6000.0, 4000.0);
        if let Some(g) = app.ghost.as_mut() {
            g.pos = zeiger;
        }
        // Ohne Drehung steht die Ecke auch hier am Zeiger.
        let vorher = vorschau_bbox(&app);
        assert_eq!(vorher.min, zeiger, "ohne Drehung liegt die Ecke am Zeiger");
        app.rotate_ghost(App::ghost_drehgrad(false));
        let nachher = vorschau_bbox(&app);
        assert!(
            (nachher.min.x - zeiger.x).abs() > 100.0 || (nachher.min.y - zeiger.y).abs() > 100.0,
            "im Modus Basispunkt muss die Ecke vom Fadenkreuz wegdrehen, sie stand bei {:?}",
            nachher.min
        );
    }

    /// Die Vorschau übernimmt den eingestellten Drehpunkt — nicht nur das
    /// Einfügen, auch die Voranzeige arbeitet damit. T0087: das Umschalten wirkt
    /// **sofort**, auch auf eine laufende Vorschau — sonst hielte der Nutzer die
    /// Einstellung im Einstellungsfenster für wirkungslos.
    #[test]
    fn vorschau_uebernimmt_den_eingestellten_drehpunkt() {
        let mut app = rechteck_app(jjcad_core::persist::PastePivot::Center);
        app.copy_selection(false);
        app.start_ghost_paste();
        let g = app.ghost.as_ref().expect("Vorschau offen");
        assert_eq!(
            g.pivot,
            jjcad_core::persist::PastePivot::Center,
            "die Vorschau kennt den eingestellten Drehpunkt nicht"
        );

        // Umschalten bei laufender Vorschau nimmt die Vorschau sofort mit.
        app.set_paste_pivot(jjcad_core::persist::PastePivot::BasePoint);
        assert_eq!(
            app.ghost.as_ref().map(|g| g.pivot),
            Some(jjcad_core::persist::PastePivot::BasePoint),
            "die laufende Vorschau behält den alten Drehpunkt"
        );
        assert_eq!(
            app.prefs.paste_pivot,
            jjcad_core::persist::PastePivot::BasePoint,
            "die Einstellung wurde nicht übernommen"
        );
    }
}

#[cfg(test)]
mod tests_mehrere_dateien {
    //! T0073: „Neu“ ersetzt die Zeichnung nicht mehr, sondern hängt einen
    //! Datei-Reiter an. Geprüft werden der Zustand je Reiter und der
    //! Reiterstreifen im Bild.

    use super::*;

    fn frame(ctx: &egui::Context, app: &mut App) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            ..Default::default()
        };
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    /// Alle gemalten Textzeilen eines Bildes mit ihrer Lage.
    fn texte(out: &egui::FullOutput) -> Vec<(String, Pos2)> {
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some((t.galley.text().trim().to_string(), t.pos)),
                _ => None,
            })
            .collect()
    }

    fn linie(app: &mut App, x: f64) {
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [x, 0.0], "b": [x, 1000.0]}),
            )
            .expect("Linie anlegen");
    }

    /// Eine neue Datei bekommt einen Reiter; die alte Zeichnung bleibt offen.
    #[test]
    fn neue_datei_haengt_einen_reiter_an() {
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        app.new_document();

        assert_eq!(app.tabs.len(), 2, "der bisherige Reiter bleibt offen");
        assert_eq!(app.active, 1, "der neue Reiter ist vorn");
        assert_eq!(app.doc().entities.len(), 0, "der neue Reiter ist leer");
        assert_eq!(
            app.tabs[0].session.doc.entities.len(),
            1,
            "die erste Datei behält ihre Linie"
        );
        assert_eq!(app.doc().name, "Unbenannt 2", "eigener Name im Reiterstreifen");

        app.new_document();
        assert_eq!(app.doc().name, "Unbenannt 3", "Namen zählen weiter");
    }

    /// T0162: Mit einer Vorlage in den Einstellungen beginnt „Neu“ als Kopie
    /// dieser Datei — Inhalt und Ebenen kommen aus der Vorlage, der Reiter ist
    /// aber neu (kein Pfad, nicht geändert, eigener Name). Ohne Vorlage bleibt
    /// es beim leeren Blatt.
    #[test]
    fn neue_datei_ist_kopie_der_vorlage() {
        let mut app = App::with_ctx(egui::Context::default());
        let dir = std::env::temp_dir().join(format!("jjcad-t0162-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pfad = dir.join("Vorlage.jjcad");
        let mut doc = jjcad_core::model::Document::new();
        jjcad_core::actions::execute(
            &mut doc,
            "draw.line",
            &serde_json::json!({"a": [0.0, 0.0], "b": [1000.0, 0.0]}),
        )
        .expect("Linie anlegen");
        jjcad_core::persist::save_document(&mut doc, &pfad).expect("Vorlage schreiben");

        app.prefs.vorlage_pfad = pfad.to_string_lossy().to_string();
        app.new_document();

        assert_eq!(
            app.doc().entities.len(),
            1,
            "die Kopie hat den Inhalt der Vorlage nicht"
        );
        assert!(
            app.doc().path.is_none(),
            "die Kopie hängt an der Vorlagendatei"
        );
        assert!(!app.doc().modified, "die frische Kopie gilt als geändert");
        assert!(
            app.doc().name.starts_with("Unbenannt"),
            "die Kopie trägt den Dateinamen: {}",
            app.doc().name
        );
        assert_eq!(app.tabs.len(), 2, "die neue Datei bekam keinen Reiter");

        // Ohne Vorlage: wieder das leere Blatt.
        app.prefs.vorlage_pfad.clear();
        app.new_document();
        assert!(
            app.doc().entities.is_empty(),
            "ohne Vorlage muss die neue Datei leer sein"
        );
        assert!(app.doc().path.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T0162: Eine unlesbare Vorlage blockiert „Neu“ nicht — die Datei beginnt
    /// leer und der Grund steht in der Statuszeile (kein stiller Fehlschlag).
    #[test]
    fn unlesbare_vorlage_beginnt_leer_und_meldet_sich() {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.vorlage_pfad = "/gibt/es/nicht/Vorlage.jjcad".into();
        app.new_document();
        assert!(
            app.doc().entities.is_empty(),
            "die unlesbare Vorlage liefert trotzdem Inhalt"
        );
        assert!(app.doc().path.is_none());
        assert_eq!(
            app.status_msg,
            jjcad_core::i18n::tr("settings.vorlage_nicht_lesbar"),
            "der Grund der unlesbaren Vorlage fehlt in der Statuszeile"
        );
        assert_eq!(app.tabs.len(), 2, "der Reiter wurde trotzdem angelegt");
    }

    /// Jede Datei behält Inhalt, Auswahl und Ansicht für sich.
    #[test]
    fn reiterwechsel_zeigt_jede_datei_wieder() {
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        let erste_ansicht = View {
            center: V2::new(1234.0, 5678.0),
            zoom: 0.5,
            rot: 0.0,
        };
        app.set_view(erste_ansicht);
        app.new_document();
        linie(&mut app, 1000.0);
        linie(&mut app, 2000.0);
        app.set_view(View::default());

        app.switch_doc(0);
        assert_eq!(app.active, 0);
        assert_eq!(app.doc().entities.len(), 1, "die erste Datei hat eine Linie");
        assert!(
            (app.current_view().zoom - 0.5).abs() < 1e-9,
            "die Ansicht der Datei bleibt erhalten"
        );

        app.switch_doc(1);
        assert_eq!(app.doc().entities.len(), 2, "die zweite Datei hat zwei Linien");
        assert!(
            (app.current_view().zoom - View::default().zoom).abs() < 1e-9,
            "die zweite Datei hat ihre eigene Ansicht"
        );
        assert_eq!(app.tabs.len(), 2, "Umschalten legt keinen Reiter an");
    }

    /// Ein laufendes Werkzeug gehört zur Datei, in der es begonnen wurde.
    #[test]
    fn werkzeug_gehoert_zur_datei() {
        let mut app = App::with_ctx(egui::Context::default());
        app.start_tool("draw.line");
        assert!(app.tool.is_some(), "Werkzeug läuft");
        app.new_document();
        assert!(app.tool.is_none(), "der Reiterwechsel bricht das Werkzeug ab");

        app.start_tool("draw.line");
        app.switch_doc(0);
        assert!(app.tool.is_none(), "auch das Umschalten bricht es ab");
    }

    /// Schließen nimmt nur den einen Reiter weg.
    #[test]
    fn schliessen_entfernt_nur_diesen_reiter() {
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        app.new_document();
        linie(&mut app, 1000.0);

        app.switch_doc(0);
        app.close_tab(0);
        assert_eq!(app.tabs.len(), 1, "ein Reiter bleibt");
        assert_eq!(app.active, 0);
        assert_eq!(
            app.doc().entities.len(),
            1,
            "die verbliebene Datei behält ihren Inhalt"
        );
    }

    /// Der letzte Reiter schließt nicht die Anwendung, sondern wird leer neu angelegt.
    #[test]
    fn letzter_reiter_bleibt_als_leere_zeichnung() {
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        app.close_tab(0);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.doc().entities.len(), 0, "leere Zeichnung");
        assert_eq!(app.doc().name, "Unbenannt");
        assert_eq!(app.active, 0);
    }

    /// Ungespeichert: erst nachfragen, nicht stillschweigend verwerfen.
    #[test]
    fn ungespeicherte_datei_fragt_vor_dem_schliessen() {
        let mut app = App::with_ctx(egui::Context::default());
        app.new_document();
        linie(&mut app, 1000.0);
        assert!(app.doc().modified, "die Datei hat Änderungen");

        app.request_close_tab(1);
        assert_eq!(app.ui.confirm_close, Some(1), "der Dialog muss nachfragen");
        assert_eq!(app.tabs.len(), 2, "noch ist nichts geschlossen");

        // „Verwerfen“ im Dialog
        app.close_tab(1);
        app.ui.confirm_close = None;
        assert_eq!(app.tabs.len(), 1);
    }

    /// T0143: „Speichern und schließen“ mit Speicherort schreibt sofort dorthin
    /// und fragt keinen Dateidialog — der Weg ohne Speicherort dagegen schon.
    #[test]
    fn speichern_und_schliessen_mit_speicherort_fragt_nicht() {
        let dir = std::env::temp_dir().join(format!("jjcad-t0143-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pfad = dir.join("Plan.jjcad");
        let mut app = App::with_ctx(egui::Context::default());
        app.new_document();
        linie(&mut app, 0.0);
        app.session_mut().save(&pfad).expect("speichern");
        linie(&mut app, 2000.0);
        assert!(app.weicht_von_zieldatei(app.active), "die Datei hat Änderungen");

        let idx = app.active;
        app.close_tab_speichern_mit_ziel(idx);
        assert!(app.ui.file_dialog.is_none(), "mit Speicherort braucht es keinen Dateidialog");
        assert_eq!(app.ui.schliessen_nach_speichern, None);
        assert_eq!(app.tabs.len(), 1, "der Reiter ist zu");

        let gespeichert = jjcad_core::persist::load_document(&pfad).expect("laden");
        assert_eq!(gespeichert.entities.len(), 2, "der neue Stand steht nicht in der Datei");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Gespeicherte Datei schließt ohne Rückfrage.
    #[test]
    fn gespeicherte_datei_schliesst_ohne_rueckfrage() {
        let pfad = std::env::temp_dir().join("jjcad-t0073-gespeichert.jjcad");
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut().save(&pfad).expect("speichern");
        app.new_document();
        app.request_close_tab(0);
        assert_eq!(app.ui.confirm_close, None, "ohne Änderungen keine Rückfrage");
        assert_eq!(app.tabs.len(), 1, "der Reiter ist zu");
        let _ = std::fs::remove_file(&pfad);
    }

    /// Dieselbe Datei landet nicht zweimal im Reiterstreifen.
    #[test]
    fn offene_datei_wird_erkannt() {
        let ordner = std::env::temp_dir();
        let pfad = ordner.join("jjcad-t0073-pfad.jjcad");
        let mut app = App::with_ctx(egui::Context::default());
        assert_eq!(app.reiter_der_datei(&pfad), None, "noch nicht offen");

        app.doc_mut().path = Some(pfad.to_string_lossy().into());
        assert_eq!(app.reiter_der_datei(&pfad), Some(0), "die Datei liegt im ersten Reiter");
        assert_eq!(
            app.reiter_der_datei(&ordner.join("jjcad-t0073-anderer.jjcad")),
            None,
            "ein anderer Pfad ist ein anderer Reiter"
        );
    }

    /// Die zwei Bereiche bleiben im Bild geschieden: Dateien auf eigenem Grund,
    /// Blätter in einer abgesetzten Fläche — ohne Beschriftung, denn die Formen
    /// sagen es. Die Chips haben Luft zueinander, damit sich ihre Umrisse nicht
    /// überschneiden (T0073, Turn 3).
    #[test]
    fn reiterstreifen_hat_zwei_zonen() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.doc_mut().name = "Grundriss".into();
        app.new_document();
        app.doc_mut().name = "Schnitt".into();
        // Zwei weitere Blätter, damit mehrere Chips nebeneinander liegen.
        app.run_action(&ctx, "sheet.new");
        app.run_action(&ctx, "sheet.new");
        for _ in 0..2 {
            let _ = frame(&ctx, &mut app);
        }
        let out = frame(&ctx, &mut app);

        let zeilen = texte(&out);
        let lage = |name: &str| {
            zeilen
                .iter()
                .filter(|(t, p)| t == name && p.y > 100.0)
                .map(|(_, p)| *p)
                .next()
                .unwrap_or_else(|| panic!("„{name}“ fehlt im Streifen: {zeilen:?}"))
        };
        let datei = lage("Schnitt");
        let modell = lage("Modell");
        assert!(
            datei.x < modell.x,
            "Dateien stehen links, die Blätter rechts"
        );
        for wort in ["Dateien", "Blätter"] {
            assert!(
                !zeilen
                    .iter()
                    .any(|(t, p)| t == wort && p.y > 100.0 && p.y < 160.0),
                "die Beschriftung „{wort}“ gehört nicht in den Reiterstreifen"
            );
        }

        // Die Zone hat einen eigenen Grund (BG-Band) bzw. eine eigene Fläche
        // (PANEL-Kasten); ohne sie wären beide Reiterarten nur verschiedene Größen.
        let flach = |farbe: egui::Color32, punkt: Pos2| {
            out.shapes.iter().any(|cs| match &cs.shape {
                egui::Shape::Rect(r) => r.fill == farbe && r.rect.contains(punkt),
                _ => false,
            })
        };
        assert!(
            flach(Palette::bg(), datei),
            "die Dateizone hat keinen eigenen Grund"
        );
        assert!(
            flach(Palette::panel(), modell),
            "die Blattzone hat keine eigene Fläche"
        );

        // Kein Aneinanderstoßen der Chip-Umrisse: zwischen zwei Chips muss Luft
        // stehen (gemeldet war eine Überschneidung von 2–3 px).
        let rechtecke = app
            .ui
            .last_blattrechtecke
            .clone()
            .expect("die Blattreiter wurden nicht gezeichnet");
        assert!(
            rechtecke.len() >= 3,
            "Modell und zwei Blätter erwartet, gezeichnet: {}",
            rechtecke.len()
        );
        for paar in rechtecke.windows(2) {
            let luft = paar[1].left() - paar[0].right();
            assert!(
                luft >= 3.0,
                "Blattreiter stehen zu nah aneinander: {luft} px Luft"
            );
        }
    }

    /// Der Befehl „file.new“ (Ribbon „Neu“, Strg+N) legt einen Reiter an.
    #[test]
    fn befehl_neu_legt_einen_reiter_an() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        linie(&mut app, 0.0);
        app.run_action(&ctx, "file.new");

        assert_eq!(app.tabs.len(), 2, "die Datei bleibt offen, der Reiter kommt dazu");
        assert_eq!(app.active, 1, "der neue Reiter ist vorn");
        assert_eq!(app.tabs[0].session.doc.entities.len(), 1, "die Linie bleibt");
        assert_eq!(app.doc().entities.len(), 0);
    }

    /// Ein frischer Reiter gilt nicht als geändert: sonst wandert eine geöffnete
    /// Datei in einen zweiten Reiter und jeder leere Reiter zeigt „ungespeichert“.
    #[test]
    fn frischer_reiter_gilt_nicht_als_geaendert() {
        let app = App::with_ctx(egui::Context::default());
        assert!(!app.doc().modified, "ein leerer Reiter ist unverändert");
        assert!(app.doc().entities.is_empty());
        assert!(app.doc().path.is_none());
        assert!(
            !app.doc().sheets.is_empty(),
            "das Standardblatt gehört zum leeren Reiter"
        );
    }

    /// Getippter Text landet in der Datei, in der er begonnen wurde.
    #[test]
    fn texteingabe_bleibt_bei_ihrer_datei() {
        let mut app = App::with_ctx(egui::Context::default());
        crate::textedit::start(&mut app, V2::new(1000.0, 1000.0));
        app.ui.texteingabe.as_mut().expect("Eingabe läuft").text = "Notiz".into();
        app.new_document();

        assert!(app.ui.texteingabe.is_none(), "der Wechsel beendet die Eingabe");
        assert_eq!(app.doc().entities.len(), 0, "der neue Reiter bleibt leer");
        assert_eq!(
            app.tabs[0].session.doc.entities.len(),
            1,
            "der Text ist in der ersten Datei"
        );
        let text = serde_json::to_string(&app.tabs[0].session.doc.entities[0]).unwrap();
        assert!(text.contains("Notiz"), "Textinhalt: {text}");
    }

    /// Der Streifen zeigt jede Datei als eigenen Reiter, darüber die Blätter der
    /// aktiven Datei.
    #[test]
    fn reiterstreifen_zeigt_beide_dateien() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.doc_mut().name = "Grundriss.jjcad".into();
        app.new_document();
        app.doc_mut().name = "Schnitt.jjcad".into();
        for _ in 0..2 {
            let _ = frame(&ctx, &mut app);
        }
        let out = frame(&ctx, &mut app);
        let zeilen = texte(&out);
        // Der Dateiname steht auch in der Kopfzeile — gesucht ist der Reiter,
        // also die Zeile unter Ribbon (y > 100).
        let lage = |name: &str| {
            zeilen
                .iter()
                .filter(|(t, p)| t == name && p.y > 100.0)
                .map(|(_, p)| *p)
                .next()
                .unwrap_or_else(|| panic!("„{name}“ fehlt im Reiterstreifen: {zeilen:?}"))
        };
        let grundriss = lage("Grundriss.jjcad");
        let schnitt = lage("Schnitt.jjcad");
        assert!(
            (grundriss.y - schnitt.y).abs() < 0.5,
            "beide Dateien stehen in einer Reihe"
        );
        assert!(
            grundriss.x < schnitt.x,
            "die Reiter stehen in der Reihenfolge der Dateien"
        );
        assert!(
            schnitt.y < app.ui.last_canvas.top(),
            "der Streifen steht über der Zeichenfläche"
        );
        // Blattreiter gehören nur zur aktiven Datei.
        assert!(zeilen.iter().any(|(t, _)| t == "Modell"), "Blattreiter fehlt");
        assert!(zeilen.iter().any(|(t, _)| t == "Blatt 1"), "Blatt fehlt");
    }
}

#[cfg(test)]
mod tests_fensterrahmen {
    //! T0098 — Fenster ohne Titelleiste.
    //!
    //! Ohne Titelleiste fehlen die Knöpfe und der Griff des Fenstermanagers:
    //! JoJa-CAD zeichnet Minimieren, Vergrößern/Wiederherstellen und Schließen
    //! selbst oben rechts, die Kopfzeile verschiebt das Fenster
    //! (`ViewportCommand::StartDrag`) und die Fensterkanten ändern die Größe
    //! (`ViewportCommand::BeginResize`). Geprüft wird der Befehl, den
    //! eframe/winit ausführt — im Test gibt es kein echtes Fenster.

    use super::*;
    // T0147: `raw_input_hook` gehört zum Trait — die Tests rufen ihn wie eframe.
    use eframe::App as _;

    /// Die **sichtbare** Fläche im Testfenster (1500 × 950): ohne Titelleiste
    /// liegt sie um den Schattenrand eingerückt (T0147).
    fn flaeche() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0)).shrink(SCHATTEN_RAND)
    }

    fn bild(
        app: &mut App,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        maximiert: Option<bool>,
    ) -> egui::FullOutput {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        raw.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                maximized: maximiert,
                ..Default::default()
            },
        );
        // eframe ruft den Hook vor jedem Bild; ohne ihn bliebe der
        // Schattenrand in den Tests aus (T0147).
        app.raw_input_hook(ctx, &mut raw);
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    /// Ein Bild mit **fester Zeit** — nur so lässt sich die Verweildauer der
    /// Kantenmarkierung prüfen (T0098, Turn 7).
    fn bild_zeit(
        app: &mut App,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        zeit: f64,
    ) -> egui::FullOutput {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            time: Some(zeit),
            ..Default::default()
        };
        raw.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                maximized: None,
                ..Default::default()
            },
        );
        // eframe ruft den Hook vor jedem Bild; ohne ihn bliebe der
        // Schattenrand in den Tests aus (T0147).
        app.raw_input_hook(ctx, &mut raw);
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    /// Wie `bild_zeit`, aber mit veränderbarer Fenstergröße — damit lässt sich
    /// ein laufendes Größe-Ziehen nachstellen (T0098, Turn 11).
    fn bild_zeit_groesse(
        app: &mut App,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        zeit: f64,
        breite: f32,
        hoehe: f32,
    ) -> egui::FullOutput {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(breite, hoehe))),
            events,
            time: Some(zeit),
            ..Default::default()
        };
        raw.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                maximized: None,
                ..Default::default()
            },
        );
        // eframe ruft den Hook vor jedem Bild; ohne ihn bliebe der
        // Schattenrand in den Tests aus (T0147).
        app.raw_input_hook(ctx, &mut raw);
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    /// Wie `bild_zeit`, aber mit ausdrücklichem Maximiert-Zustand (T0119).
    fn bild_zeit_max(
        app: &mut App,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        zeit: f64,
        maximiert: Option<bool>,
    ) -> egui::FullOutput {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            time: Some(zeit),
            ..Default::default()
        };
        raw.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                maximized: maximiert,
                ..Default::default()
            },
        );
        // eframe ruft den Hook vor jedem Bild; ohne ihn bliebe der
        // Schattenrand in den Tests aus (T0147).
        app.raw_input_hook(ctx, &mut raw);
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    fn befehle(out: &egui::FullOutput) -> Vec<egui::ViewportCommand> {
        out.viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.commands.clone())
            .unwrap_or_default()
    }

    /// App auf **demselben** Kontext — nur dort sind die Schriftfamilien
    /// eingerichtet (`theme::install`); ein zweiter `Context::default()` kennt
    /// „bold“ nicht und paniert beim Zeichnen der Kopfzeile.
    fn ohne_titelleiste(ctx: egui::Context) -> App {
        let mut app = App::with_ctx(ctx);
        app.prefs.fensterrahmen = jjcad_core::persist::Fensterrahmen::OhneTitelleiste;
        // T0147: der eigene Schatten hängt am Fenstersystem — im Test
        // ausdrücklich an, sonst hinge das Ergebnis an der Umgebung.
        // `raw_input_hook` fragt **beides** ab (`schatten_im_rand` und
        // `prefs.fensterschatten`) — ohne den zweiten Wert hing der Test an der
        // `prefs.json` des Rechners: mit abgeschaltetem Schatten fielen alle
        // Kantenzonen-Tests.
        app.schatten_im_rand = true;
        app.prefs.fensterschatten = true;
        app
    }

    fn knopf(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn klick(
        app: &mut App,
        ctx: &egui::Context,
        pos: Pos2,
        maximiert: Option<bool>,
    ) -> Vec<egui::ViewportCommand> {
        let mut cmds = Vec::new();
        for events in [
            vec![egui::Event::PointerMoved(pos)],
            vec![knopf(pos, true)],
            vec![knopf(pos, false)],
            vec![],
        ] {
            let out = bild(app, ctx, events, maximiert);
            cmds.extend(befehle(&out));
        }
        cmds
    }

    #[test]
    fn mit_titelleiste_zeichnet_die_app_keine_fensterknoepfe() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        // Der Test setzt die Vorgabe selbst — die echte `prefs.json` des
        // Nutzers darf das Ergebnis nicht verschieben.
        app.prefs.fensterrahmen = jjcad_core::persist::Fensterrahmen::System;
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        assert!(
            app.ui.last_fenster_knoepfe.is_none(),
            "mit Titelleiste gehören die Fensterknöpfe dem Fenstermanager, \
             die Anwendung darf keine zweiten zeichnen"
        );
        assert!(
            app.ui.last_fensterkanten.is_none(),
            "ohne eigenen Rahmen sind auch keine Kantenzonen nötig"
        );
    }

    #[test]
    fn ohne_titelleiste_stehen_drei_knoepfe_oben_rechts() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let r = app
            .ui
            .last_fenster_knoepfe
            .expect("ohne Titelleiste müssen die drei Fensterknöpfe gezeichnet werden");
        for (i, rect) in r.iter().enumerate() {
            assert!(
                (rect.width() - FENSTERKNOPF_W).abs() < 0.5,
                "Knopf {i} ist {:.1} px breit, erwartet {FENSTERKNOPF_W}",
                rect.width()
            );
            assert!(
                rect.bottom() <= flaeche().top() + HEADER_H + 0.5
                    && rect.top() >= flaeche().top() - 0.5,
                "Knopf {i} liegt nicht in der Kopfzeile ({}…{}): {rect:?}",
                flaeche().top(),
                flaeche().top() + HEADER_H
            );
        }
        assert!(
            (r[2].right() - flaeche().right()).abs() < 0.5,
            "Schließen sitzt nicht am rechten Fensterrand: {r:?}"
        );
        assert!(
            r[0].right() <= r[1].left() + 0.5 && r[1].right() <= r[2].left() + 0.5,
            "die Knöpfe stehen nicht in der Reihenfolge Minimieren · Vergrößern · Schließen: {r:?}"
        );
    }

    #[test]
    fn schliessen_knopf_schickt_close() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let r = app.ui.last_fenster_knoepfe.expect("Knöpfe fehlen");
        let cmds = klick(&mut app, &ctx, r[2].center(), None);
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "Klick auf Schließen muss das Fenster schließen (Befehle: {cmds:?})"
        );
    }

    #[test]
    fn vergroessern_knopf_schaltet_das_fenster_um() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), Some(false));
        }
        let r = app.ui.last_fenster_knoepfe.expect("Knöpfe fehlen");
        let cmds = klick(&mut app, &ctx, r[1].center(), Some(false));
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::Maximized(true))),
            "Klick auf Vergrößern muss das Fenster vergrößern (Befehle: {cmds:?})"
        );

        // Ist das Fenster schon groß, stellt derselbe Knopf wieder her.
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), Some(true));
        }
        let r = app.ui.last_fenster_knoepfe.expect("Knöpfe fehlen");
        let cmds = klick(&mut app, &ctx, r[1].center(), Some(true));
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::Maximized(false))),
            "Klick auf Wiederherstellen muss das Fenster verkleinern (Befehle: {cmds:?})"
        );
    }

    #[test]
    fn minimieren_knopf_schickt_minimized() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let r = app.ui.last_fenster_knoepfe.expect("Knöpfe fehlen");
        let cmds = klick(&mut app, &ctx, r[0].center(), None);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::Minimized(true))),
            "Klick auf Minimieren muss das Fenster minimieren (Befehle: {cmds:?})"
        );
    }

    /// Ein Zug auf die Kopfzeile verschiebt das Fenster. Der Griff liegt
    /// **unter** den Reitern und Knöpfen; geprüft wird in der freien Mitte
    /// zwischen den Reitern und der rechten Knopfgruppe (am Fensterrand läge
    /// die Zone zum Ziehen der Fensterkante darüber).
    #[test]
    fn zug_auf_die_kopfzeile_verschiebt_das_fenster() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let start = Pos2::new(750.0, flaeche().top() + HEADER_H * 0.5);
        bild(&mut app, &ctx, vec![egui::Event::PointerMoved(start)], None);
        bild(&mut app, &ctx, vec![knopf(start, true)], None);
        let out = bild(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start + Vec2::new(30.0, 0.0))],
            None,
        );
        let cmds = befehle(&out);
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::StartDrag)),
            "Zug auf die Kopfzeile muss das Fenster verschieben (Befehle: {cmds:?})"
        );
    }

    /// T0098 (Turn 3): Nach einem Fensterzug bekommt die Anwendung das
    /// Loslassen **nicht** — der Fenstermanager hat den Zeiger übernommen
    /// (`_NET_WM_MOVERESIZE` unter X11, `xdg_toplevel.move` unter Wayland).
    /// egui blieb deshalb für immer „am Ziehen“, und jeder zweite Zug
    /// verschob das Fenster nicht.
    #[test]
    fn zwei_zuege_nacheinander_verschieben_das_fenster() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let start = Pos2::new(750.0, flaeche().top() + HEADER_H * 0.5);
        let mut zuege = 0;
        for runde in 0..2 {
            bild(&mut app, &ctx, vec![egui::Event::PointerMoved(start)], None);
            bild(&mut app, &ctx, vec![knopf(start, true)], None);
            let out = bild(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start + Vec2::new(30.0, 0.0))],
                None,
            );
            if befehle(&out)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::StartDrag))
            {
                zuege += 1;
            }
            // Bewusst **kein** Loslassen: der Fenstermanager hält den Zeiger.
            assert!(runde < 2);
        }
        assert_eq!(
            zuege, 2,
            "nur {zuege} von 2 Zügen auf die Kopfzeile haben das Fenster verschoben"
        );
    }

    #[test]
    fn fensterkanten_stehen_an_den_raendern() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let kanten = app
            .ui
            .last_fensterkanten
            .clone()
            .expect("ohne Titelleiste braucht das Fenster Kantenzonen");
        assert_eq!(
            kanten.len(),
            7,
            "vier Kanten und drei Ecken erwartet — die Ecke oben rechts gehört den Fensterknöpfen"
        );
        assert!(
            !kanten.iter().any(|(n, _)| *n == "no"),
            "die Ecke oben rechts darf keine Ziehzone tragen, dort sitzt „Schließen“"
        );
        let ost = kanten
            .iter()
            .find(|(n, _)| *n == "o")
            .expect("rechte Kante fehlt")
            .1;
        assert!(
            (ost.right() - flaeche().right()).abs() < 0.5 && (ost.width() - FENSTERKANTE_PX).abs() < 0.5,
            "die Ziehzone der rechten Kante liegt nicht am Fensterrand: {ost:?}"
        );

        // Druck auf die Kante fordert das Ziehen der Größe an.
        let pos = ost.center();
        bild(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)], None);
        let out = bild(&mut app, &ctx, vec![knopf(pos, true)], None);
        let cmds = befehle(&out);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, egui::ViewportCommand::BeginResize(egui::ResizeDirection::East))),
            "Druck auf die rechte Fensterkante muss das Größe-Ziehen beginnen (Befehle: {cmds:?})"
        );
    }

    /// T0119: Ein maximiertes Fenster füllt den Bildschirm — dort darf die
    /// Größe nicht gezogen werden. Das Betriebssystem meldet weiter
    /// „bildschirmfüllend“; ein Zug an der Kante verkleinerte die Zeichnung
    /// gegen einen Rahmen, den es nicht gibt.
    #[test]
    fn maximiert_gibt_es_keine_kantenzonen() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), Some(true));
        }
        assert!(
            app.ui.last_fensterkanten.is_none(),
            "im maximierten Fenster darf es keine Ziehzonen geben"
        );
        // Auch ein Druck genau auf den Rand beginnt kein Größe-Ziehen.
        let pos = Pos2::new(flaeche().right() - 3.0, 500.0);
        let cmds = klick(&mut app, &ctx, pos, Some(true));
        assert!(
            !cmds
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::BeginResize(_))),
            "im maximierten Fenster darf der Rand kein Größe-Ziehen beginnen (Befehle: {cmds:?})"
        );
        // Und der Zeiger zeigt dort keinen Größen-Pfeil.
        let out = bild(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(pos)],
            Some(true),
        );
        assert_ne!(
            out.platform_output.cursor_icon,
            egui::CursorIcon::ResizeHorizontal,
            "im maximierten Fenster darf am Rand kein Größen-Pfeil stehen"
        );
    }

    /// T0119: Wiederhergestellt ist das Ziehen wieder da — und eine Markierung
    /// aus dem Fenstermodus bleibt beim Maximieren nicht stehen.
    #[test]
    fn maximieren_loescht_die_kantenmarkierung() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), Some(false));
        }
        assert!(
            app.ui.last_fensterkanten.is_some(),
            "im wiederhergestellten Fenster gehören die Kantenzonen dazu"
        );
        let kante = Pos2::new(flaeche().right() - 3.0, 500.0);
        bild_zeit_max(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(kante)],
            50.0,
            Some(false),
        );
        bild_zeit_max(&mut app, &ctx, Vec::new(), 50.2, Some(false));
        assert_eq!(
            app.ui.kanten_leuchten,
            Some("o"),
            "Vorbedingung: die rechte Kante leuchtet nach dem Verweilen"
        );
        // Das Fenster wird maximiert — Markierung und Verweilzustand fallen weg.
        bild_zeit_max(&mut app, &ctx, Vec::new(), 50.3, Some(true));
        assert_eq!(
            app.ui.kanten_leuchten, None,
            "im maximierten Fenster darf keine Kante leuchten"
        );
        assert!(
            app.ui.kanten_verweil.is_none(),
            "im maximierten Fenster darf kein Verweilzustand stehen bleiben"
        );
        // Wiederhergestellt: die Zonen sind zurück.
        bild_zeit_max(&mut app, &ctx, Vec::new(), 50.4, Some(false));
        assert!(
            app.ui.last_fensterkanten.is_some(),
            "nach dem Wiederherstellen müssen die Kantenzonen zurück sein"
        );
    }

    /// T0098 (Turn 5): Jede Ziehzone beginnt das Größe-Ziehen mit der
    /// richtigen Richtung — vier Kanten und drei Ecken (die Ecke oben rechts
    /// gehört den Fensterknöpfen).
    #[test]
    fn jede_kante_beginnt_das_groesse_ziehen() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let kanten = app
            .ui
            .last_fensterkanten
            .clone()
            .expect("ohne Titelleiste braucht das Fenster Kantenzonen");
        let erwartet = [
            ("n", egui::ResizeDirection::North),
            ("s", egui::ResizeDirection::South),
            ("w", egui::ResizeDirection::West),
            ("o", egui::ResizeDirection::East),
            ("nw", egui::ResizeDirection::NorthWest),
            ("sw", egui::ResizeDirection::SouthWest),
            ("so", egui::ResizeDirection::SouthEast),
        ];
        for (name, richtung) in erwartet {
            let rect = kanten
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("Zone „{name}“ fehlt"))
                .1;
            let pos = rect.center();
            bild(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)], None);
            let out = bild(&mut app, &ctx, vec![knopf(pos, true)], None);
            let cmds = befehle(&out);
            assert!(
                cmds.iter().any(|c| matches!(
                    c,
                    egui::ViewportCommand::BeginResize(r) if *r == richtung
                )),
                "Druck in Zone „{name}“ (bei {pos:?}) muss {richtung:?} beginnen, Befehle: {cmds:?}"
            );
        }
    }

    /// T0098 (Turn 5): Über der Kante zeigt der Zeiger den Größen-Pfeil — die
    /// einzige sichtbare Andeutung, dass dort gezogen werden kann.
    #[test]
    fn fensterkante_zeigt_den_groesse_pfeil() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        for (pos, cursor) in [
            (Pos2::new(flaeche().right() - 3.0, 500.0), egui::CursorIcon::ResizeHorizontal),
            (
                Pos2::new(800.0, flaeche().bottom() - 2.0),
                egui::CursorIcon::ResizeVertical,
            ),
        ] {
            let out = bild(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)], None);
            assert_eq!(
                out.platform_output.cursor_icon, cursor,
                "bei {pos:?} muss der Größen-Pfeil stehen"
            );
        }
    }

    /// T0098 (Turn 7): Die Kante leuchtet erst, wenn der Zeiger dort stehen
    /// bleibt — nicht schon beim Darüberstreichen.
    #[test]
    fn kante_leuchtet_erst_nach_verweilen() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let kante = Pos2::new(flaeche().right() - 3.0, 500.0);
        bild_zeit(&mut app, &ctx, vec![egui::Event::PointerMoved(kante)], 10.0);
        assert_eq!(
            app.ui.kanten_leuchten, None,
            "die Kante darf im Ankunftbild nicht leuchten"
        );
        bild_zeit(&mut app, &ctx, Vec::new(), 10.04);
        assert_eq!(
            app.ui.kanten_leuchten, None,
            "nach 40 ms darf die Kante noch nicht leuchten"
        );
        bild_zeit(&mut app, &ctx, Vec::new(), 10.15);
        assert_eq!(
            app.ui.kanten_leuchten,
            Some("o"),
            "nach {KANTEN_VERWEILDAUER} s muss die rechte Kante leuchten"
        );
        bild_zeit(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(700.0, 500.0))],
            10.3,
        );
        assert_eq!(
            app.ui.kanten_leuchten, None,
            "verlässt der Zeiger die Kante, hört die Markierung auf"
        );
    }

    /// T0098 (Turn 7): Ein schneller Durchzug über den Rand bleibt unsichtbar.
    #[test]
    fn schneller_durchzug_leuchtet_nicht() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        bild_zeit(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(flaeche().right() - 3.0, 500.0))],
            20.0,
        );
        bild_zeit(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(1200.0, 500.0))],
            20.1,
        );
        assert_eq!(
            app.ui.kanten_leuchten, None,
            "ein Durchzug von 100 ms darf keine Markierung zeigen"
        );
        assert!(
            app.ui.kanten_verweil.is_none(),
            "nach dem Verlassen ist der Verweil-Zustand zurückgesetzt"
        );
    }

    /// T0098 (Turn 9/11): Während die Größe gezogen wird, bleibt die Markierung
    /// der Kante stehen — die Kante wandert mit dem Zeiger, der Abstand bleibt
    /// also klein.
    #[test]
    fn markierung_bleibt_waehrend_des_ziehens() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let kante = Pos2::new(flaeche().right() - 3.0, 500.0);
        bild_zeit(&mut app, &ctx, vec![egui::Event::PointerMoved(kante)], 30.0);
        let out = bild_zeit(&mut app, &ctx, vec![knopf(kante, true)], 30.05);
        assert!(
            befehle(&out)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::BeginResize(_))),
            "der Druck auf die Kante muss das Ziehen beginnen"
        );
        // Zeiger wandert nach innen, das Fenster wird im selben Maß schmaler.
        for (i, (x, breite)) in [(1300.0_f32, 1300.0_f32), (1100.0, 1100.0), (900.0, 900.0)]
            .iter()
            .enumerate()
        {
            bild_zeit_groesse(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(Pos2::new(*x, 500.0))],
                30.1 + i as f64 * 0.1,
                *breite,
                950.0,
            );
            assert_eq!(
                app.ui.kanten_leuchten,
                Some("o"),
                "während des Ziehens muss die rechte Kante markiert bleiben (x = {x})"
            );
        }
        // Loslassen beendet den Zug; der Zeiger steht noch am Rand, dort hält
        // ihn die Verweil-Markierung — erst das Weggehen löscht sie.
        bild_zeit_groesse(
            &mut app,
            &ctx,
            vec![knopf(Pos2::new(900.0, 500.0), false)],
            30.5,
            900.0,
            950.0,
        );
        assert!(
            app.ui.kanten_zug.is_none(),
            "nach dem Loslassen darf kein Zug mehr laufen"
        );
        bild_zeit_groesse(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(600.0, 500.0))],
            30.6,
            900.0,
            950.0,
        );
        assert_eq!(
            app.ui.kanten_leuchten, None,
            "nach dem Loslassen und Weggehen darf die Kante nicht weiter leuchten"
        );
    }

    /// T0098 (Turn 9/11): Kommt das Loslassen nicht an (der Fenstermanager hält
    /// den Zeiger), endet die Markierung trotzdem — sie hängt an der
    /// Größenänderung, nicht am Tastenereignis.
    #[test]
    fn markierung_faellt_nach_dem_not_aus_ab() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let kante = Pos2::new(flaeche().right() - 3.0, 500.0);
        bild_zeit(&mut app, &ctx, vec![egui::Event::PointerMoved(kante)], 40.0);
        bild_zeit(&mut app, &ctx, vec![knopf(kante, true)], 40.05);
        assert_eq!(app.ui.kanten_leuchten, Some("o"));
        // Kein Loslassen, keine Größenänderung, Zeiger bleibt am Rand.
        bild_zeit(&mut app, &ctx, Vec::new(), 40.2);
        assert_eq!(
            app.ui.kanten_leuchten,
            Some("o"),
            "kurz nach dem letzten Schritt bleibt die Markierung"
        );
        bild_zeit(&mut app, &ctx, Vec::new(), 40.05 + ZUG_NACHLAUF_S + 0.1);
        assert!(
            app.ui.kanten_zug.is_none(),
            "ohne Größenänderung muss der Zug nach dem Nachlauf enden"
        );
    }

    /// T0098 (Turn 11): Wandert der Zeiger nach dem Ziehen ins Fenster zurück,
    /// ist die Markierung sofort weg — sie hängt nicht am Loslassen.
    #[test]
    fn markierung_endet_wenn_der_zeiger_weggeht() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let kante = Pos2::new(flaeche().right() - 3.0, 500.0);
        bild_zeit(&mut app, &ctx, vec![egui::Event::PointerMoved(kante)], 60.0);
        bild_zeit(&mut app, &ctx, vec![knopf(kante, true)], 60.05);
        assert_eq!(app.ui.kanten_leuchten, Some("o"));
        bild_zeit(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(1200.0, 500.0))],
            60.2,
        );
        assert_eq!(
            app.ui.kanten_leuchten, None,
            "der Zeiger ist weit von der Kante weg — die Markierung muss enden"
        );
    }

    /// T0098 (Turn 5): In der Ecke unten rechts steht ein sichtbarer Griff —
    /// ohne ihn war die Fläche unsichtbar und wurde nicht gefunden.
    #[test]
    fn ecke_unten_rechts_hat_einen_griff() {
        let ctx = egui::Context::default();
        let mut app = ohne_titelleiste(ctx.clone());
        for _ in 0..3 {
            bild(&mut app, &ctx, Vec::new(), None);
        }
        let griff = app
            .ui
            .last_fenster_griff
            .expect("ohne Titelleiste fehlt der Größen-Griff in der Ecke");
        assert!(
            (griff.right() - flaeche().right()).abs() < 0.5
                && (griff.bottom() - flaeche().bottom()).abs() < 0.5
                && (griff.width() - FENSTERECKE_PX).abs() < 0.5,
            "der Griff sitzt nicht in der Ecke unten rechts: {griff:?}"
        );
    }
}

mod tests_fensterschatten {
    //! T0147 — Fensterschatten ohne Titelleiste.
    //!
    //! Der Fenstermanager zeichnet für ein Fenster ohne Dekoration keinen
    //! Schatten; das Fenster lag danach ohne Abhebung auf dem Hintergrund.
    //! JoJa-CAD zeichnet ihn selbst: ein durchsichtiger Streifen von
    //! [`SCHATTEN_RAND`] Punkten am Fensterrand trägt den weichen Schatten, die
    //! Oberfläche liegt darin. Geprüft wird, was gezeichnet wird — im Test gibt
    //! es kein echtes Fenster.

    use super::*;
    use eframe::App as _;

    const FENSTER: Vec2 = Vec2::new(1500.0, 950.0);

    fn bild(app: &mut App, ctx: &egui::Context, maximiert: Option<bool>) -> egui::FullOutput {
        bild_mit(app, ctx, FENSTER, None, maximiert)
    }

    /// Wie `bild`, aber mit Fenstergröße und Bildschirmgröße (T0177) — daran
    /// erkennt `raw_input_hook` ein angedocktes Fenster.
    fn bild_mit(
        app: &mut App,
        ctx: &egui::Context,
        fenster: Vec2,
        monitor: Option<Vec2>,
        maximiert: Option<bool>,
    ) -> egui::FullOutput {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, fenster)),
            ..Default::default()
        };
        raw.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                maximized: maximiert,
                monitor_size: monitor,
                ..Default::default()
            },
        );
        // eframe ruft den Hook vor jedem Bild — er setzt den Schattenrand.
        app.raw_input_hook(ctx, &mut raw);
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    fn app_mit(rahmen: jjcad_core::persist::Fensterrahmen) -> (egui::Context, App) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.fensterrahmen = rahmen;
        // Der eigene Schatten hängt am Fenstersystem — im Test ausdrücklich an.
        // `raw_input_hook` prüft **beides**; `prefs.fensterschatten` kommt sonst
        // aus der `prefs.json` des Rechners und die Tests hingen an seiner
        // Einstellung (abgeschalteter Schatten → 12 rote Fenstertests).
        app.schatten_im_rand = true;
        app.prefs.fensterschatten = true;
        (ctx, app)
    }

    /// Die weichen Schatten im Bild: Rechtecke mit Unschärfe, samt Breite.
    fn schatten(out: &egui::FullOutput) -> Vec<(Rect, f32)> {
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) if r.blur_width > 0.0 => Some((r.rect, r.blur_width)),
                _ => None,
            })
            .collect()
    }

    fn gleich(a: Rect, b: Rect) -> bool {
        a.min.distance(b.min) < 0.5 && a.max.distance(b.max) < 0.5
    }

    #[test]
    fn ohne_titelleiste_zeichnet_den_schatten_um_die_flaeche() {
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::OhneTitelleiste);
        let mut out = bild(&mut app, &ctx, None);
        for _ in 0..2 {
            out = bild(&mut app, &ctx, None);
        }
        let innen = Rect::from_min_size(Pos2::ZERO, FENSTER).shrink(SCHATTEN_RAND);
        assert!(
            gleich(ctx.screen_rect(), innen),
            "die Oberfläche muss im Schattenrand liegen: {:?} statt {innen:?}",
            ctx.screen_rect()
        );
        let schatten = schatten(&out);
        let lage = innen.translate(egui::vec2(0.0, SCHATTEN_VERSATZ));
        let (rect, blur) = schatten
            .iter()
            .find(|(r, _)| gleich(*r, lage))
            .copied()
            .unwrap_or_else(|| {
                panic!("kein Schatten um die sichtbare Fläche (gezeichnet: {schatten:?})")
            });
        assert!(
            blur >= SCHATTEN_RAND,
            "der Schatten ist zu hart ({blur} px Unschärfe) — er reicht nicht in den Rand"
        );
        assert!(
            gleich(rect, lage),
            "der Schatten liegt nicht an der sichtbaren Fläche: {rect:?}"
        );
    }

    #[test]
    fn mit_titelleiste_gibt_es_keinen_schatten() {
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::System);
        let mut out = bild(&mut app, &ctx, None);
        for _ in 0..2 {
            out = bild(&mut app, &ctx, None);
        }
        assert!(
            gleich(ctx.screen_rect(), Rect::from_min_size(Pos2::ZERO, FENSTER)),
            "mit Titelleiste füllt die Oberfläche das ganze Fenster"
        );
        assert!(
            schatten(&out).is_empty(),
            "mit Titelleiste zeichnet der Fenstermanager den Schatten: {:?}",
            schatten(&out)
        );
    }

    #[test]
    fn maximiert_zeichnet_keinen_schatten() {
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::OhneTitelleiste);
        let mut out = bild(&mut app, &ctx, Some(true));
        for _ in 0..2 {
            out = bild(&mut app, &ctx, Some(true));
        }
        assert!(
            gleich(ctx.screen_rect(), Rect::from_min_size(Pos2::ZERO, FENSTER)),
            "maximiert füllt das Fenster den Bildschirm — kein Rand für einen Schatten"
        );
        assert!(
            schatten(&out).is_empty(),
            "maximiert darf kein Schatten gezeichnet werden: {:?}",
            schatten(&out)
        );
    }

    /// Der Grund des Fensters muss im Rand durchsichtig sein, sonst bliebe vom
    /// Schatten nur ein dunkler Rahmen.
    #[test]
    fn grund_ist_ohne_titelleiste_durchsichtig() {
        // T0177: Der Grund folgt dem Rand **dieses** Bildes — erst ein Bild
        // laufen lassen, dann fragen.
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::OhneTitelleiste);
        bild(&mut app, &ctx, None);
        assert_eq!(
            app.clear_color(&egui::Visuals::dark())[3],
            0.0,
            "ohne Titelleiste braucht der Rand einen durchsichtigen Grund"
        );
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::System);
        bild(&mut app, &ctx, None);
        assert_eq!(
            app.clear_color(&egui::Visuals::dark())[3],
            1.0,
            "mit Titelleiste bleibt der Grund deckend"
        );
    }

    /// T0177: Ein angedocktes Fenster liegt am Bildschirmrand. Gemessen unter
    /// KWin 6.3 (`kwin_wayland --virtual`, 1600 × 1000): „Fenster nach links
    /// andocken“ ergibt 799 × 972 — halbe Breite, Höhe um die Bedienleiste
    /// gekürzt. Dort darf kein Schattenrand liegen: er wäre ein Abstand zur
    /// Kante, und die Oberfläche wäre um seine Breite kleiner.
    #[test]
    fn angedocktes_fenster_fuellt_die_oberflaeche_das_fenster() {
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::OhneTitelleiste);
        let fenster = Vec2::new(799.0, 972.0);
        let monitor = Some(Vec2::new(1600.0, 1000.0));
        let mut out = bild_mit(&mut app, &ctx, fenster, monitor, None);
        for _ in 0..2 {
            out = bild_mit(&mut app, &ctx, fenster, monitor, None);
        }
        assert!(
            gleich(ctx.screen_rect(), Rect::from_min_size(Pos2::ZERO, fenster)),
            "angeschlossen am Bildschirmrand muss die Oberfläche das ganze Fenster füllen: {:?}",
            ctx.screen_rect()
        );
        assert!(
            schatten(&out).is_empty(),
            "am Bildschirmrand gibt es keinen Platz für einen Schatten: {:?}",
            schatten(&out)
        );
        assert_eq!(
            app.clear_color(&egui::Visuals::dark())[3],
            1.0,
            "ohne Rand ist der Grund deckend"
        );
    }

    /// T0177: Die Einstellung schaltet den Schatten ab — dann füllt die
    /// Oberfläche das Fenster ganz, auch ohne Andocken.
    #[test]
    fn abgeschalteter_schatten_fuellt_die_oberflaeche_das_fenster() {
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::OhneTitelleiste);
        app.prefs.fensterschatten = false;
        let mut out = bild(&mut app, &ctx, None);
        for _ in 0..2 {
            out = bild(&mut app, &ctx, None);
        }
        assert!(
            gleich(ctx.screen_rect(), Rect::from_min_size(Pos2::ZERO, FENSTER)),
            "ohne Schatten füllt die Oberfläche das Fenster: {:?}",
            ctx.screen_rect()
        );
        assert!(
            schatten(&out).is_empty(),
            "ohne Schatten darf nichts gezeichnet werden: {:?}",
            schatten(&out)
        );
        assert_eq!(
            app.clear_color(&egui::Visuals::dark())[3],
            1.0,
            "ohne Schatten braucht das Fenster keinen durchsichtigen Grund"
        );
    }

    /// Gegenprobe zur Erkennung: ein frei stehendes Fenster behält den Rand —
    /// auch wenn es fast bildschirmgroß ist (die Vorgabe 1500 × 950 auf einem
    /// 1600 × 1000 großen Bildschirm).
    #[test]
    fn frei_stehendes_fenster_behaelt_den_schattenrand() {
        let (ctx, mut app) = app_mit(jjcad_core::persist::Fensterrahmen::OhneTitelleiste);
        let mut out = bild_mit(
            &mut app,
            &ctx,
            FENSTER,
            Some(Vec2::new(1600.0, 1000.0)),
            None,
        );
        for _ in 0..2 {
            out = bild_mit(
                &mut app,
                &ctx,
                FENSTER,
                Some(Vec2::new(1600.0, 1000.0)),
                None,
            );
        }
        assert!(
            gleich(ctx.screen_rect(), Rect::from_min_size(Pos2::ZERO, FENSTER).shrink(SCHATTEN_RAND)),
            "ein frei stehendes Fenster behält den Schattenrand: {:?}",
            ctx.screen_rect()
        );
        assert!(
            !schatten(&out).is_empty(),
            "der Schatten fehlt am frei stehenden Fenster"
        );
    }

    /// T0177: Die Erkennung selbst — halbe Breite oder Höhe ist angedockt,
    /// volle Bildschirmgröße ist „maximiert“ und wird dort gemeldet.
    #[test]
    fn angedockt_wird_an_der_fenstergroesse_erkannt() {
        let monitor = Some(Vec2::new(1600.0, 1000.0));
        // Angedockt links/rechts: gemessen 799 × 972, rechnerisch 800 × 1000.
        assert!(fenster_angedockt(Vec2::new(799.0, 972.0), monitor));
        assert!(fenster_angedockt(Vec2::new(800.0, 1000.0), monitor));
        // Oben/unten angedockt.
        assert!(fenster_angedockt(Vec2::new(1600.0, 500.0), monitor));
        // Viertel: beide Richtungen halb.
        assert!(fenster_angedockt(Vec2::new(800.0, 500.0), monitor));
        // Frei stehende Fenster — auch ein fast bildschirmgroßes.
        assert!(!fenster_angedockt(Vec2::new(1500.0, 950.0), monitor));
        assert!(!fenster_angedockt(Vec2::new(1080.0, 680.0), monitor));
        // Das Vorgabefenster auf einem 4K-Bildschirm ist genau halb so breit
        // (1532 von 3072), aber nur 57 % hoch — es steht frei und behält den
        // Schatten.
        let uhd = Some(Vec2::new(3072.0, 1728.0));
        assert!(!fenster_angedockt(Vec2::new(1532.0, 982.0), uhd));
        // Angedockt auf demselben Bildschirm: halbe Breite, Höhe bis zur
        // Arbeitsfläche (gemessen 972 von 1000 → hier 1656 von 1728).
        assert!(fenster_angedockt(Vec2::new(1536.0, 1656.0), uhd));
        // Ohne Bildschirmgröße (erste Bilder, Testumgebung) wird nichts erkannt.
        assert!(!fenster_angedockt(Vec2::new(799.0, 972.0), None));
    }
}

mod tests_bildablage {
    //! T0112: Bilder über die System-Zwischenablage austauschen.
    //!
    //! Die Verbindung zur Zwischenablage (`arboard`) braucht eine Anzeige und
    //! ist im Testlauf nicht vorhanden — geprüft wird deshalb alles davor und
    //! danach: **welche** Auswahl als Bild gilt, wie aus rohen Pixeln ein
    //! einbettbares Bild wird, und woran Strg+V mit einem reinen Bild erkannt
    //! wird. Der Weg über die Zwischenablage selbst ist ein dünner Aufruf.

    use super::*;

    /// 2×2: rot, grün / blau, weiß.
    const RGBA: [u8; 16] = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];

    fn app_mit_bild() -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        let (name, data) = jjcad_core::bild::aus_rgba(2, 2, &RGBA).expect("PNG aus Pixeln");
        let r = app
            .session_mut()
            .execute(
                "draw.image",
                &serde_json::json!({"a": [0, 0], "b": [1000, 0], "name": name, "data": data}),
            )
            .expect("Bild");
        let id = r["created"][0].as_u64().unwrap();
        app.session_mut().select(vec![id]);
        app
    }

    /// Genau ein ausgewähltes Bild ist der Fall, in dem Strg+C eine reine
    /// Bilddatei ablegt.
    #[test]
    fn ein_einzelnes_bild_ist_das_bild_der_auswahl() {
        let app = app_mit_bild();
        let bild = app.bild_der_auswahl().expect("Bild der Auswahl");
        assert_eq!((bild.breite, bild.hoehe), (2, 2));
        assert_eq!(bild.pixel(0, 0), [255, 0, 0, 255], "links oben rot");
    }

    /// Zwei Bilder sind keine „einzelne Bilddatei“ — dann bleibt es bei der
    /// eigenen Zwischenablage.
    #[test]
    fn zwei_bilder_sind_kein_einzelbild() {
        let mut app = app_mit_bild();
        let (name, data) = jjcad_core::bild::aus_rgba(2, 2, &RGBA).expect("PNG");
        let r = app
            .session_mut()
            .execute(
                "draw.image",
                &serde_json::json!({"a": [0, 2000], "b": [1000, 2000], "name": name, "data": data}),
            )
            .expect("zweites Bild");
        let zweite = r["created"][0].as_u64().unwrap();
        let erste = app.selection()[0];
        app.session_mut().select(vec![erste, zweite]);
        assert!(
            app.bild_der_auswahl().is_none(),
            "zwei Bilder dürfen nicht als eine Bilddatei in die Zwischenablage"
        );
    }

    /// Eine Wand ist kein Bild.
    #[test]
    fn eine_wand_ist_kein_bild() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        let r = app
            .session_mut()
            .execute("draw.wall", &serde_json::json!({"a": [0, 0], "b": [3000, 0]}))
            .expect("Wand");
        let id = r["created"][0].as_u64().unwrap();
        app.session_mut().select(vec![id]);
        assert!(app.bild_der_auswahl().is_none());
    }

    /// Ein Bild aus der Zwischenablage landet auf demselben Weg im Dokument
    /// wie ein Datei-Import: eingebettete PNG-Daten plus Werkzeug `draw.image`.
    #[test]
    fn bild_aus_pixeln_startet_das_bildwerkzeug() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx);
        app.bild_aus_pixeln(2, 2, &RGBA);
        assert_eq!(
            app.tool_id(),
            Some("draw.image"),
            "nach dem Einfügen wartet das Bildwerkzeug auf die beiden Ecken"
        );
        let quelle = app.options.bild.clone().expect("Bilddaten für das Werkzeug");
        assert_eq!(quelle.name, "Zwischenablage.png");
        let bild = jjcad_core::bild::dekodiere(&quelle.data).expect("eingebettete Daten lesbar");
        assert_eq!((bild.breite, bild.hoehe), (2, 2));
        assert_eq!(bild.pixel(1, 1), [255, 255, 255, 255], "rechts unten weiß");
    }

    /// egui-winit verschluckt den Druck von Strg+V bei einem reinen Bild — das
    /// Loslassen kommt durch und ist der Auslöser.
    #[test]
    fn strg_v_wird_am_loslassen_erkannt() {
        let ctx = egui::Context::default();
        let loslassen = egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![loslassen.clone()],
                ..Default::default()
            },
            |_| {},
        );
        assert!(strg_v_losgelassen(&ctx), "Loslassen von Strg+V muss zählen");

        // Der Druck selbst wird von egui-winit geschluckt — er darf nicht zählen.
        let druck = egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![druck],
                ..Default::default()
            },
            |_| {},
        );
        assert!(!strg_v_losgelassen(&ctx), "der Druck allein ist kein Einfügen");

        // Ein blankes „v“ (Werkzeug Verschieben) ist kein Einfügen.
        let blank = egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![blank],
                ..Default::default()
            },
            |_| {},
        );
        assert!(
            !strg_v_losgelassen(&ctx),
            "ohne Strg darf das Loslassen von V nichts einfügen"
        );
    }
}

#[cfg(test)]
mod tests_dragdrop {
    //! Drag an Drop (T0115): eine ins Fenster gezogene Datei wird auf ihr
    //! Format geprüft und über **denselben** Weg importiert wie eine Auswahl im
    //! Dateidialog (`dialogs::datei_gewaehlt`).
    //!
    //! Ein echter Zug lässt sich headless nicht erzeugen; geprüft wird der
    //! Eingang, den egui dafür bereitstellt (`RawInput::dropped_files` bzw.
    //! `hovered_files`) — genau die Stelle, an der `App::dateien_ablegen` liest.

    use super::*;
    use crate::dialogs::tests_tastatur::{WegwerfKonfig, KONFIG};
    use crate::dialogs::FileMode;

    fn probe(name: &str) -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    fn bild() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1500.0, 950.0),
            )),
            ..Default::default()
        }
    }

    fn datei(pfad: &std::path::Path) -> egui::DroppedFile {
        egui::DroppedFile {
            path: Some(pfad.to_path_buf()),
            name: pfad
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            mime: String::new(),
            last_modified: None,
            bytes: None,
        }
    }

    /// Ein Bild zeichnen, in dem die Dateien abgelegt werden.
    fn ablegen(app: &mut App, ctx: &egui::Context, pfade: &[&std::path::Path]) {
        let raw = egui::RawInput {
            dropped_files: pfade.iter().map(|p| datei(p)).collect(),
            ..bild()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// Die Endung entscheidet über den Import — die eine Tabelle.
    #[test]
    fn endung_entscheidet_ueber_den_import() {
        let f = |n: &str| FileMode::import_aus_datei(std::path::Path::new(n));
        assert_eq!(f("haus.jjcad"), Some(FileMode::Open));
        assert_eq!(f("HAUS.JJCAD"), Some(FileMode::Open), "Großschreibung");
        assert_eq!(f("plan.dxf"), Some(FileMode::ImportDxf));
        assert_eq!(f("plan.pdf"), Some(FileMode::ImportPdf));
        assert_eq!(f("bild.png"), Some(FileMode::ImportImage));
        assert_eq!(f("foto.JPG"), Some(FileMode::ImportImage));
        assert_eq!(f("foto.jpeg"), Some(FileMode::ImportImage));
        assert_eq!(f("notizen.txt"), None, "Textdatei ist nicht importierbar");
        assert_eq!(f("ohne_endung"), None);
    }

    /// Eine abgelegte Zeichnung landet als Reiter mit Inhalt.
    #[test]
    fn abgelegte_zeichnung_wird_geoeffnet() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("drop-jjcad");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let pfad = probe("Wohnhaus.jjcad");
        assert!(pfad.exists(), "Prüfdatei fehlt: {}", pfad.display());

        ablegen(&mut app, &ctx, &[&pfad]);

        assert_eq!(
            app.doc().path.as_deref(),
            Some(pfad.to_string_lossy().as_ref()),
            "die abgelegte Zeichnung ist nicht geöffnet"
        );
        assert!(
            !app.doc().entities.is_empty(),
            "die geöffnete Zeichnung ist leer"
        );
    }

    /// Eine abgelegte DXF-Datei wird in die laufende Zeichnung importiert.
    #[test]
    fn abgelegtes_dxf_wird_importiert() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let vorher = app.doc().entities.len();
        let pfad = probe("Grundriss.dxf");
        assert!(pfad.exists(), "Prüfdatei fehlt: {}", pfad.display());

        ablegen(&mut app, &ctx, &[&pfad]);

        assert!(
            app.doc().entities.len() > vorher,
            "DXF-Import hat nichts hinzugefügt ({} → {})",
            vorher,
            app.doc().entities.len()
        );
    }

    /// Ein abgelegtes PDF öffnet die Seitenwahl — nicht mehr und nicht weniger.
    #[test]
    fn abgelegtes_pdf_oeffnet_die_seitenwahl() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let pfad = probe("Grundriss.pdf");
        assert!(pfad.exists(), "Prüfdatei fehlt: {}", pfad.display());

        ablegen(&mut app, &ctx, &[&pfad]);

        let wahl = app.ui.pdf_import.as_ref().expect("Seitenwahl nicht geöffnet");
        assert_eq!(wahl.pfad, pfad);
        assert!(wahl.seiten >= 1, "das PDF hat keine Seite");
    }

    /// Eine zweite PDF-Datei überschreibt die offene Seitenwahl nicht still.
    #[test]
    fn zweites_pdf_ueberschreibt_die_seitenwahl_nicht() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let pfad = probe("Grundriss.pdf");

        ablegen(&mut app, &ctx, &[&pfad]);
        ablegen(&mut app, &ctx, &[&pfad]);

        assert!(
            app.status_msg.contains("Seitenwahl offen"),
            "die zweite Datei wurde ohne Meldung verworfen: {:?}",
            app.status_msg
        );
    }

    /// Ein abgelegtes Bild startet das Bildwerkzeug mit der Datei.
    #[test]
    fn abgelegtes_bild_startet_das_bildwerkzeug() {
        let _sperre = KONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let _konfig = WegwerfKonfig::neu("drop-bild");
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let pfad = probe("Grundriss.png");
        assert!(pfad.exists(), "Prüfdatei fehlt: {}", pfad.display());

        ablegen(&mut app, &ctx, &[&pfad]);

        assert!(
            app.options.bild.is_some(),
            "das Bild wurde nicht eingelesen"
        );
        assert_eq!(
            app.tool.as_ref().map(|t| t.action.as_str()),
            Some("draw.image"),
            "das Bildwerkzeug läuft nicht"
        );
    }

    /// Ein nicht importierbares Format wird gemeldet und ändert nichts.
    #[test]
    fn nicht_importierbares_format_wird_gemeldet() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let pfad = std::env::temp_dir().join("jjcad-t0115-notizen.txt");
        std::fs::write(&pfad, b"kein CAD").unwrap();

        ablegen(&mut app, &ctx, &[&pfad]);

        assert!(
            app.status_msg.contains("Format nicht unterstützt"),
            "keine Meldung für ein fremdes Format: {:?}",
            app.status_msg
        );
        assert!(app.doc().path.is_none(), "die Zeichnung wurde ersetzt");
        assert!(app.doc().entities.is_empty(), "es wurde etwas eingefügt");
        let _ = std::fs::remove_file(&pfad);
    }

    /// Solange eine Datei schwebt, nennt der Hinweis ihr Ziel — und beim
    /// nicht importierbaren Format den Grund.
    #[test]
    fn schwebende_datei_zeigt_ihr_ziel() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let pfad = probe("Wohnhaus.jjcad");

        let mut texte = Vec::new();
        for _ in 0..2 {
            let raw = egui::RawInput {
                hovered_files: vec![egui::HoveredFile {
                    path: Some(pfad.clone()),
                    ..Default::default()
                }],
                ..bild()
            };
            let out = ctx.run(raw, |ctx| app.ui(ctx));
            texte = out
                .shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect();
        }
        assert!(
            texte.iter().any(|t| t == "Datei loslassen"),
            "der Hinweis fehlt: {texte:?}"
        );
        assert!(
            texte.iter().any(|t| t.contains("Wohnhaus.jjcad") && t.contains("Zeichnung öffnen")),
            "der Hinweis nennt das Ziel nicht: {texte:?}"
        );

        // Fremdes Format: derselbe Hinweis, aber mit dem Grund.
        let mut app = App::with_ctx(ctx.clone());
        let mut texte = Vec::new();
        for _ in 0..2 {
            let raw = egui::RawInput {
                hovered_files: vec![egui::HoveredFile {
                    path: Some(PathBuf::from("Notizen.txt")),
                    ..Default::default()
                }],
                ..bild()
            };
            let out = ctx.run(raw, |ctx| app.ui(ctx));
            texte = out
                .shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect();
        }
        assert!(
            texte.iter().any(|t| t.contains("Format nicht unterstützt")),
            "der Hinweis verschweigt das fremde Format: {texte:?}"
        );
    }
}

#[cfg(test)]
mod tests_sicherung {
    //! T0129: Die Sicherung arbeitet getrennt vom Speichern. Geprüft wird der
    //! Weg über die App: Intervall, Stände, Zieldatei und Wiederherstellen.

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-sicherung-app-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// App mit eigenem Pool — der Datenordner des Nutzers bleibt unberührt.
    fn app_mit_pool(dir: &std::path::Path) -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(dir));
        app.prefs.sicherung_intervall = 1;
        app.prefs.sicherung_verlauf = true;
        // Ohne feste Zahl hinge das Ausdünnen im Test an den Vorgaben.
        app.prefs.sicherung_plaetze = 100;
        app
    }

    fn linie(app: &mut App, x: f64) {
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [x, 0.0], "b": [x, 1000.0]}),
            )
            .expect("Linie anlegen");
    }

    /// Zeichnen, warten: ein Stand entsteht, die Zieldatei bleibt unverändert,
    /// der neueste Stand entspricht dem Arbeitsstand.
    #[test]
    fn tick_sichert_und_laesst_die_zieldatei_unberuehrt() {
        let dir = test_dir("tick");
        let mut app = app_mit_pool(&dir);
        let ziel = dir.join("ziele/Plan.jjcad");
        app.session_mut().save(&ziel).expect("Zieldatei");
        let vorher = std::fs::read(&ziel).unwrap();

        linie(&mut app, 0.0);
        app.sicherung_tick(120.0);

        let pool = app.sicherung.as_ref().expect("Pool");
        assert_eq!(pool.anzahl(), 1, "kein Stand angelegt");
        assert_eq!(
            std::fs::read(&ziel).unwrap(),
            vorher,
            "die Sicherung hat die Zieldatei verändert"
        );
        let stand = pool.staende_neueste()[0].clone();
        let geladen = pool.laden(&stand).expect("Stand lesbar");
        assert_eq!(
            geladen.entities.len(),
            app.doc().entities.len(),
            "der neueste Stand ist nicht der Arbeitsstand"
        );
    }

    /// Ungespeicherte Arbeit überlebt das Beenden: `sicherung_jetzt` (der
    /// Beenden-Weg) legt sie als Stand ab, der nächste Start holt sie zurück —
    /// mit Pfad, aber als geändert. Die Zieldatei bleibt unberührt.
    #[test]
    fn ungespeicherte_arbeit_kommt_beim_start_zurueck() {
        let dir = test_dir("ungespeichert");
        let mut app = app_mit_pool(&dir);
        let ziel = dir.join("ziele/Plan.jjcad");
        linie(&mut app, 0.0);
        app.session_mut().save(&ziel).expect("Zieldatei");
        app.sicherung_dateistand(0);

        linie(&mut app, 500.0);
        app.sicherung_jetzt();

        let s = app.sitzung_sammeln();
        let mut neu = App::with_ctx(egui::Context::default());
        let meldungen = neu.sitzung_wiederherstellen(&s, &dir);
        assert!(meldungen.is_empty(), "keine Meldung erwartet: {meldungen:?}");
        assert_eq!(neu.doc().entities.len(), 2, "die ungespeicherte Linie fehlt");
        assert_eq!(
            neu.doc().path.as_deref(),
            Some(ziel.to_string_lossy().as_ref()),
            "der Pfad muss erhalten bleiben"
        );
        assert!(neu.doc().modified, "der Reiter muss als geändert gelten");
        assert!(neu.weicht_von_zieldatei(0));

        let datei = jjcad_core::persist::load_document(&ziel).expect("Zieldatei lesbar");
        assert_eq!(datei.entities.len(), 1, "die Zieldatei wurde angefasst");
    }

    /// „Verwerfen“ lässt die Änderungen nicht zurückkehren: der neueste Stand
    /// wird der Dateiinhalt, der Start lädt wieder die Datei.
    #[test]
    fn verworfene_arbeit_kommt_nicht_zurueck() {
        let dir = test_dir("verworfen");
        let mut app = app_mit_pool(&dir);
        let ziel = dir.join("ziele/Plan.jjcad");
        linie(&mut app, 0.0);
        app.session_mut().save(&ziel).expect("Zieldatei");

        linie(&mut app, 500.0);
        app.sicherung_jetzt();
        app.verwerfen_stand(0);

        let s = app.sitzung_sammeln();
        let mut neu = App::with_ctx(egui::Context::default());
        neu.sitzung_wiederherstellen(&s, &dir);
        assert_eq!(
            neu.doc().entities.len(),
            1,
            "verworfene Änderung ist zurückgekommen"
        );
        assert!(!neu.doc().modified, "die Datei gilt als gespeichert");
    }

    /// R3.5 / P4: Verworfene Arbeit liegt danach als markierter Stand vor.
    #[test]
    fn verworfene_arbeit_liegt_als_stand_vor() {
        let dir = test_dir("verworfen-stand");
        let mut app = app_mit_pool(&dir);
        let ziel = dir.join("ziele/Plan.jjcad");
        linie(&mut app, 0.0);
        app.session_mut().save(&ziel).expect("Zieldatei");

        linie(&mut app, 500.0);
        app.sicherung_jetzt();
        app.verwerfen_stand(0);

        let pool = app.sicherung.as_ref().expect("Pool");
        assert_eq!(pool.verworfene(), 1, "der verworfene Stand fehlt");
        let verworfen = pool
            .staende_neueste()
            .into_iter()
            .find(|s| s.verworfen)
            .cloned()
            .expect("verworfener Stand");
        let inhalt = pool.laden(&verworfen).expect("Stand lesbar");
        assert_eq!(
            inhalt.entities.len(),
            2,
            "der Stand trägt nicht die verworfene Arbeit"
        );
        // Der neueste Stand ist trotzdem der Dateiinhalt.
        assert_eq!(pool.neuester(&app.doc().id).unwrap().name, "Plan");
        assert!(!pool.neuester(&app.doc().id).unwrap().verworfen);
    }

    /// R2.3 / P3: Ändert eine fremde Anwendung die Zieldatei, entscheidet der
    /// Benutzer — bis dahin gilt die Datei, und es wird nichts überschrieben.
    #[test]
    fn extern_geaenderte_datei_fragt_nach() {
        let dir = test_dir("konflikt");
        let mut app = app_mit_pool(&dir);
        let ziel = dir.join("ziele/Plan.jjcad");
        linie(&mut app, 0.0);
        app.session_mut().save(&ziel).expect("Zieldatei");
        app.sicherung_dateistand(0);

        linie(&mut app, 500.0);
        app.sicherung_jetzt();
        let s = app.sitzung_sammeln();
        let vorher = std::fs::read(&ziel).unwrap();

        // Fremder Schreiber: eine dritte Linie direkt in der Datei.
        let mut fremd = jjcad_core::persist::load_document(&ziel).expect("Datei lesbar");
        let ebene = fremd.layers.first().expect("Ebene").id;
        fremd.entities.push(jjcad_core::model::Entity::line(
            9_999,
            ebene,
            jjcad_core::geom::Vec2::new(900.0, 0.0),
            jjcad_core::geom::Vec2::new(900.0, 1000.0),
        ));
        jjcad_core::persist::save_document(&mut fremd, &ziel).expect("fremdes Speichern");
        let fremd_inhalt = std::fs::read(&ziel).unwrap();
        assert_ne!(vorher, fremd_inhalt, "Vorbereitung: Datei geändert");

        let mut neu = App::with_ctx(egui::Context::default());
        let meldungen = neu.sitzung_wiederherstellen(&s, &dir);
        assert!(meldungen.is_empty(), "keine Meldung erwartet: {meldungen:?}");
        let konflikt = neu.ui.konflikt.clone().expect("kein Konflikt gemeldet");
        assert!(konflikt.neu, "die Knöpfe dürfen nicht im ersten Bild greifen");
        assert_eq!(
            neu.doc().entities.len(),
            2,
            "bis zur Entscheidung muss die Datei gelten"
        );
        assert_eq!(
            std::fs::read(&ziel).unwrap(),
            fremd_inhalt,
            "die Datei wurde beim Start angefasst"
        );
    }

    /// R2.3: Entscheidung für den Stand holt die ungespeicherte Arbeit zurück;
    /// die Datei bleibt, wie sie ist.
    #[test]
    fn entscheidung_fuer_den_stand_holt_die_arbeit_zurueck() {
        let dir = test_dir("konflikt-stand");
        let mut app = app_mit_pool(&dir);
        let ziel = dir.join("ziele/Plan.jjcad");
        linie(&mut app, 0.0);
        app.session_mut().save(&ziel).expect("Zieldatei");
        app.sicherung_dateistand(0);
        linie(&mut app, 500.0);
        app.sicherung_jetzt();
        let s = app.sitzung_sammeln();

        let mut fremd = jjcad_core::persist::load_document(&ziel).expect("Datei lesbar");
        let ebene = fremd.layers.first().expect("Ebene").id;
        fremd.entities.push(jjcad_core::model::Entity::line(
            9_999,
            ebene,
            jjcad_core::geom::Vec2::new(900.0, 0.0),
            jjcad_core::geom::Vec2::new(900.0, 1000.0),
        ));
        jjcad_core::persist::save_document(&mut fremd, &ziel).expect("fremdes Speichern");
        let fremd_inhalt = std::fs::read(&ziel).unwrap();

        let mut neu = App::with_ctx(egui::Context::default());
        neu.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(&dir));
        neu.sitzung_wiederherstellen(&s, &dir);
        assert!(neu.ui.konflikt.is_some(), "kein Konflikt gemeldet");
        neu.konflikt_entscheiden(true);

        assert_eq!(neu.doc().entities.len(), 2, "der Stand fehlt");
        assert!(neu.doc().modified, "der Stand gilt als ungespeichert");
        assert_eq!(
            std::fs::read(&ziel).unwrap(),
            fremd_inhalt,
            "die Entscheidung hat die Datei überschrieben"
        );
        assert!(neu.ui.konflikt.is_none(), "der Konflikt bleibt stehen");
    }

    /// R2.3: Entscheidung für die Datei — der nächste Start fragt nicht wieder.
    #[test]
    fn entscheidung_fuer_die_datei_fragt_nicht_wieder() {
        let dir = test_dir("konflikt-datei");
        let mut app = app_mit_pool(&dir);
        let ziel = dir.join("ziele/Plan.jjcad");
        linie(&mut app, 0.0);
        app.session_mut().save(&ziel).expect("Zieldatei");
        app.sicherung_dateistand(0);
        linie(&mut app, 500.0);
        app.sicherung_jetzt();
        let s = app.sitzung_sammeln();

        let mut fremd = jjcad_core::persist::load_document(&ziel).expect("Datei lesbar");
        let ebene = fremd.layers.first().expect("Ebene").id;
        fremd.entities.push(jjcad_core::model::Entity::line(
            9_999,
            ebene,
            jjcad_core::geom::Vec2::new(900.0, 0.0),
            jjcad_core::geom::Vec2::new(900.0, 1000.0),
        ));
        jjcad_core::persist::save_document(&mut fremd, &ziel).expect("fremdes Speichern");

        let mut neu = App::with_ctx(egui::Context::default());
        neu.sicherung = Some(jjcad_core::sicherung::Sicherung::oeffnen(&dir));
        neu.sitzung_wiederherstellen(&s, &dir);
        neu.konflikt_entscheiden(false);
        assert!(neu.ui.konflikt.is_none());
        assert_eq!(neu.doc().entities.len(), 2, "die Datei muss gelten");

        // Zweiter Start: kein Konflikt mehr, die Datei gilt.
        let mut dritter = App::with_ctx(egui::Context::default());
        dritter.sitzung_wiederherstellen(&s, &dir);
        assert!(
            dritter.ui.konflikt.is_none(),
            "der Konflikt kommt beim nächsten Start wieder"
        );
        assert_eq!(dritter.doc().entities.len(), 2);
    }

    /// T0188: Die Seite arbeitet auf einem Entwurf — ohne „Übernehmen“ ändert
    /// sich an den Einstellungen nichts.
    #[test]
    fn sicherungseinstellungen_gelten_erst_nach_uebernehmen() {
        let dir = test_dir("entwurf");
        let mut app = app_mit_pool(&dir);
        app.prefs.sicherung_plaetze = 100;
        let entwurf = jjcad_core::sicherung::Einstellungen {
            plaetze: 30,
            ..app.prefs.sicherung_einstellungen()
        };
        app.ui.sicherung_entwurf = Some(entwurf);
        assert_eq!(
            app.prefs.sicherung_plaetze, 100,
            "der Entwurf hat sofort gewirkt"
        );
        assert!(
            app.sicherung.as_ref().unwrap().staende().is_empty(),
            "der Entwurf hat schon ausgedünnt"
        );
    }

    /// T0188: Ein Übernehmen, das Stände kostet, fragt erst und löscht nichts
    /// ohne Bestätigung.
    #[test]
    fn uebernehmen_warnt_vor_dem_loeschen() {
        let dir = test_dir("warnung");
        let mut app = app_mit_pool(&dir);
        let einst = app.prefs.sicherung_einstellungen();
        assert_eq!(einst.plaetze, 100, "Vorbereitung: Vorgabe 100 Plätze");
        let mut doc = jjcad_core::model::Document::new();
        doc.name = "Plan".into();
        doc.id = "aaaaaaaa-0000-4000-8000-0000000000ff".into();
        for i in 0..40 {
            doc.name = format!("Plan {i}");
            app.sicherung
                .as_mut()
                .unwrap()
                .sichern(&mut doc, &einst)
                .unwrap();
        }
        assert_eq!(app.sicherung.as_ref().unwrap().anzahl(), 40);

        let entwurf = jjcad_core::sicherung::Einstellungen {
            plaetze: 30,
            ..einst
        };
        // Vorbedingung: die neuen Werte kosten wirklich Stände. Sonst prüft der
        // Test nichts und die Zahlen unten wären nicht erklärbar.
        let angesagt = app
            .sicherung
            .as_ref()
            .unwrap()
            .wuerde_verlieren(&entwurf);
        assert_eq!(angesagt, 10, "Vorbereitung: 40 Stände, 30 Plätze");
        app.ui.sicherung_entwurf = Some(entwurf);
        app.sicherung_uebernehmen();

        assert!(app.ui.sicherung_warnung.is_some(), "keine Warnung gestellt");
        assert_eq!(
            app.sicherung.as_ref().unwrap().anzahl(),
            40,
            "ohne Bestätigung wurde gelöscht"
        );
        assert_eq!(app.prefs.sicherung_plaetze, 100, "ohne Bestätigung übernommen");

        app.sicherung_anwenden(entwurf);
        assert_eq!(app.sicherung.as_ref().unwrap().anzahl(), 30);
        assert_eq!(app.prefs.sicherung_plaetze, 30);
        assert!(app.ui.sicherung_warnung.is_none());
    }

    /// T0188: Aufräumen löscht nur Stände mit vermerktem, aber verschwundenem
    /// Pfad — Altbestand ohne Pfad bleibt stehen.
    #[test]
    fn aufraeumen_trifft_nur_verschwundene_zeichnungen() {
        let dir = test_dir("aufraeumen");
        let mut app = app_mit_pool(&dir);
        let einst = app.prefs.sicherung_einstellungen();
        let mut weg = jjcad_core::model::Document::new();
        weg.path = Some(dir.join("gibtsnicht.jjcad").to_string_lossy().to_string());
        weg.id = "aaaaaaaa-0000-4000-8000-0000000000a1".into();
        app.sicherung.as_mut().unwrap().sichern(&mut weg, &einst).unwrap();

        let mut bleibt = jjcad_core::model::Document::new();
        bleibt.id = "aaaaaaaa-0000-4000-8000-0000000000a2".into();
        app.sicherung.as_mut().unwrap().sichern(&mut bleibt, &einst).unwrap();

        assert_eq!(app.sicherung.as_ref().unwrap().ohne_zeichnung().len(), 1);
        app.sicherung_aufraeumen_jetzt();
        let pool = app.sicherung.as_ref().unwrap();
        assert_eq!(pool.anzahl(), 1, "falsche Stände gelöscht");
        assert_eq!(pool.staende()[0].datei, bleibt.id);
    }

    /// Vor Ablauf des Intervalls wird nichts gesichert.
    #[test]
    fn tick_wartet_das_intervall_ab() {
        let dir = test_dir("intervall");
        let mut app = app_mit_pool(&dir);
        linie(&mut app, 0.0);
        app.sicherung_tick(10.0);
        assert!(
            app.sicherung.as_ref().unwrap().staende().is_empty(),
            "vor Ablauf des Intervalls darf nichts gesichert werden"
        );
        app.sicherung_tick(61.0);
        assert_eq!(app.sicherung.as_ref().unwrap().anzahl(), 1);
    }

    /// „Verlauf sichern“ aus → genau ein Stand je Dokument.
    #[test]
    fn verlauf_aus_haelt_einen_stand() {
        let dir = test_dir("verlauf-aus");
        let mut app = app_mit_pool(&dir);
        app.prefs.sicherung_verlauf = false;
        for i in 0..5 {
            linie(&mut app, i as f64 * 100.0);
            app.sicherung_tick(120.0 * (i + 1) as f64);
        }
        let pool = app.sicherung.as_ref().unwrap();
        assert_eq!(pool.anzahl(), 1, "Verlauf aus hält einen Stand");
    }
}

#[cfg(test)]
mod tests_fensterlage {
    //! T0141: Fensterlage für die Sitzung. Geprüft wird die Umrechnung aus den
    //! egui-Meldungen — mit Fensterrechtecken (X11/Windows) und ohne
    //! (Wayland, dort meldet das Fenstersystem keine Position).

    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
    }

    /// Wayland: keine Fensterrechtecke, aber die Größe aus der Bildschirmfläche.
    #[test]
    fn ohne_fensterrechtecke_bleibt_die_groesse() {
        let lage = lage_aus(None, None, rect(0.0, 0.0, 1500.0, 950.0)).expect("Größe genügt");
        assert_eq!(lage.breite, 1500.0);
        assert_eq!(lage.hoehe, 950.0);
        assert_eq!(lage.lage(), None, "ohne Meldung keine Position");
        assert!(!lage.mit_lage, "kein Merker ohne Meldung");
        assert!(lage.brauchbar());
    }

    /// X11/Windows: die Außenkante ist die Fensterposition, die Innenfläche die
    /// Größe (die Titelleiste gehört nicht dazu).
    #[test]
    fn mit_fensterrechtecken_kommt_die_lage() {
        let lage = lage_aus(
            Some(rect(100.0, 50.0, 1600.0, 1000.0)),
            Some(rect(90.0, 20.0, 1620.0, 1030.0)),
            rect(0.0, 0.0, 1600.0, 1000.0),
        )
        .expect("Lage");
        assert_eq!((lage.breite, lage.hoehe), (1600.0, 1000.0));
        assert_eq!(lage.lage(), Some((90.0, 20.0)));
    }

    /// Fehlt nur die Außenkante, ist die Innenkante der Rückfall — sie ist
    /// ebenfalls global.
    #[test]
    fn innenkante_ist_der_rueckfall() {
        let lage = lage_aus(
            Some(rect(100.0, 50.0, 1600.0, 1000.0)),
            None,
            rect(0.0, 0.0, 1600.0, 1000.0),
        )
        .expect("Lage");
        assert_eq!(lage.lage(), Some((100.0, 50.0)));
    }

    /// Ein minimiertes Fenster (Größe 0) ergibt keine Lage — dann bleibt die
    /// gespeicherte in der Datei stehen. Dasselbe gilt für den Platzhalter,
    /// den egui ohne Fensterangabe meldet (10 000 × 10 000).
    #[test]
    fn ohne_groesse_keine_lage() {
        assert!(lage_aus(None, None, rect(0.0, 0.0, 0.0, 0.0)).is_none());
        assert!(lage_aus(None, None, rect(0.0, 0.0, 10_000.0, 10_000.0)).is_none());
    }

    /// Ein Bild mit gesetzten Fensterangaben — danach stehen sie in
    /// `ctx.input()`, und `fenster_merken`/`fensterlage` lesen sie wie im Lauf.
    fn bild_mit(ctx: &egui::Context, viewport: egui::ViewportInfo, bild: egui::Rect) {
        let mut raw = egui::RawInput {
            screen_rect: Some(bild),
            ..Default::default()
        };
        raw.viewports.insert(egui::ViewportId::ROOT, viewport);
        let _ = ctx.run(raw, |_| {});
    }

    fn viewport(
        inner: Option<egui::Rect>,
        outer: Option<egui::Rect>,
        vollbild: Option<bool>,
        maximiert: Option<bool>,
    ) -> egui::ViewportInfo {
        egui::ViewportInfo {
            inner_rect: inner,
            outer_rect: outer,
            fullscreen: vollbild,
            maximized: maximiert,
            ..Default::default()
        }
    }

    /// T0141, Turn 2: Ein Vollbild meldet die Bildschirmgröße. Gespeichert wird
    /// trotzdem die zuletzt bekannte normale Größe — sonst stünde nach dem
    /// Verlassen des Vollbilds ein bildschirmgroßes Fenster da.
    #[test]
    fn vollbild_behaelt_die_normale_groesse() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.fenster_normal = Some(sitzung::Fensterlage {
            breite: 1200.0,
            hoehe: 800.0,
            x: 0.0,
            y: 0.0,
            mit_lage: false,
            maximiert: false,
            vollbild: false,
        });
        bild_mit(
            &ctx,
            viewport(
                Some(rect(0.0, 0.0, 1600.0, 1000.0)),
                None,
                Some(true),
                Some(true),
            ),
            rect(0.0, 0.0, 1600.0, 1000.0),
        );
        let lage = app.fensterlage().expect("Lage");
        assert!(lage.vollbild, "Vollbild wird gemerkt");
        assert!(
            !lage.maximiert,
            "ein Vollbild meldet unter KWin oft zusätzlich maximiert"
        );
        assert_eq!(
            (lage.breite, lage.hoehe),
            (1200.0, 800.0),
            "die Bildschirmgröße taugt nicht als Fenstergröße"
        );
    }

    /// Maximiert ohne Vollbild: Zustand wird gemerkt, die Größe bleibt die
    /// normale.
    #[test]
    fn maximiert_behaelt_die_normale_groesse() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.fenster_normal = Some(sitzung::Fensterlage {
            breite: 1200.0,
            hoehe: 800.0,
            x: 0.0,
            y: 0.0,
            mit_lage: false,
            maximiert: false,
            vollbild: false,
        });
        bild_mit(
            &ctx,
            viewport(
                Some(rect(0.0, 0.0, 1600.0, 1000.0)),
                None,
                Some(false),
                Some(true),
            ),
            rect(0.0, 0.0, 1600.0, 1000.0),
        );
        let lage = app.fensterlage().expect("Lage");
        assert!(lage.maximiert && !lage.vollbild);
        assert_eq!((lage.breite, lage.hoehe), (1200.0, 800.0));
    }

    /// `fenster_merken` schreibt nur den Normalzustand mit — im Vollbild und
    /// maximiert bleibt der gemerkte Stand stehen.
    #[test]
    fn fenster_merken_ignoriert_vollbild_und_maximiert() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        bild_mit(
            &ctx,
            viewport(
                Some(rect(0.0, 0.0, 1600.0, 1000.0)),
                None,
                Some(true),
                Some(false),
            ),
            rect(0.0, 0.0, 1600.0, 1000.0),
        );
        app.fenster_merken();
        assert!(app.fenster_normal.is_none(), "Vollbild wird nicht gemerkt");

        bild_mit(
            &ctx,
            viewport(
                Some(rect(0.0, 0.0, 1250.0, 820.0)),
                None,
                Some(false),
                Some(false),
            ),
            rect(0.0, 0.0, 1250.0, 820.0),
        );
        app.fenster_merken();
        let normal = app.fenster_normal.expect("Normalzustand gemerkt");
        assert_eq!((normal.breite, normal.hoehe), (1250.0, 820.0));
    }

    /// Die gespeicherte Lage gilt beim Start als normaler Stand — ohne die
    /// Zustandsmerker, sonst würde ein Vollbild-Start die Bildschirmgröße
    /// weiterreichen.
    #[test]
    fn gespeicherte_lage_ist_der_normale_stand() {
        let mut app = App::with_ctx(egui::Context::default());
        app.setze_fensterlage(Some(sitzung::Fensterlage {
            breite: 1200.0,
            hoehe: 800.0,
            x: 10.0,
            y: 20.0,
            mit_lage: true,
            maximiert: true,
            vollbild: true,
        }));
        let normal = app.fenster_normal.expect("übernommen");
        assert!(!normal.maximiert && !normal.vollbild);
        assert_eq!((normal.breite, normal.hoehe), (1200.0, 800.0));
        assert_eq!(normal.lage(), Some((10.0, 20.0)));
    }

    fn befehle(aus: &egui::FullOutput) -> Vec<egui::ViewportCommand> {
        aus.viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.commands.clone())
            .unwrap_or_default()
    }

    /// Ein Bild mit Fensterangaben und fester Zeit für den Startzustand.
    fn bild_start(
        vollbild: Option<bool>,
        maximiert: Option<bool>,
        zeit: f64,
    ) -> egui::RawInput {
        let mut raw = egui::RawInput {
            time: Some(zeit),
            ..Default::default()
        };
        raw.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                fullscreen: vollbild,
                maximized: maximiert,
                ..Default::default()
            },
        );
        raw
    }

    /// T0141, Turn 2: Der Wunsch wird wiederholt, bis der Fenstermanager ihn
    /// meldet — beim Erzeugen des Fensters verwirft KWin ihn (gemessen). Danach
    /// ist er verbraucht, sonst ließe sich das Fenster nicht verkleinern.
    #[test]
    fn startzustand_wird_angefordert_bis_er_steht() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.setze_fensterlage(Some(sitzung::Fensterlage {
            breite: 1200.0,
            hoehe: 800.0,
            x: 0.0,
            y: 0.0,
            mit_lage: false,
            maximiert: false,
            vollbild: true,
        }));
        // Bild 1: das Fenster meldet noch kein Vollbild -> Befehl.
        let aus = ctx.run(
            bild_start(None, None, 0.0),
            |ctx| app.fenster_start_anwenden(ctx),
        );
        assert!(
            befehle(&aus)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Fullscreen(true))),
            "Vollbild muss angefordert werden: {:?}",
            befehle(&aus)
        );
        // Bild 2: das Fenster meldet Vollbild -> Wunsch ist erledigt.
        let aus2 = ctx.run(
            bild_start(Some(true), None, 0.05),
            |ctx| app.fenster_start_anwenden(ctx),
        );
        assert!(befehle(&aus2).is_empty(), "{:?}", befehle(&aus2));
        // Bild 3: der Nutzer verlässt das Vollbild — kein erneutes Maximieren.
        let aus3 = ctx.run(
            bild_start(Some(false), None, 0.1),
            |ctx| app.fenster_start_anwenden(ctx),
        );
        assert!(
            befehle(&aus3).is_empty(),
            "der Wunsch ist verbraucht: {:?}",
            befehle(&aus3)
        );
    }

    /// Ohne Vollbild wird maximiert angefordert — und nach
    /// [`FENSTER_START_GEDULD`] aufgegeben, damit ein Fenstermanager ohne
    /// Maximieren nicht endlos belästigt wird.
    #[test]
    fn startzustand_maximiert_und_gibt_dann_auf() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.setze_fensterlage(Some(sitzung::Fensterlage {
            breite: 1200.0,
            hoehe: 800.0,
            x: 0.0,
            y: 0.0,
            mit_lage: false,
            maximiert: true,
            vollbild: false,
        }));
        let aus = ctx.run(
            bild_start(Some(false), Some(false), 0.0),
            |ctx| app.fenster_start_anwenden(ctx),
        );
        assert!(befehle(&aus)
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::Maximized(true))));
        let spaet = FENSTER_START_GEDULD + 0.5;
        let aus2 = ctx.run(
            bild_start(Some(false), Some(false), spaet),
            |ctx| app.fenster_start_anwenden(ctx),
        );
        assert!(
            befehle(&aus2).is_empty(),
            "nach der Geduld wird aufgegeben: {:?}",
            befehle(&aus2)
        );
        assert!(app.fenster_start.is_none());
    }

    /// Die Sitzung nimmt die Lage mit — und ohne Fenster (Tests, headless)
    /// bleibt `fenster` leer, statt eine Null-Lage zu schreiben.
    #[test]
    fn sitzung_schreibt_die_fensterlage() {
        let mut app = App::with_ctx(egui::Context::default());
        assert!(
            app.sitzung_sammeln().fenster.is_none(),
            "ohne Fenster keine Lage"
        );
        let lage = lage_aus(None, None, rect(0.0, 0.0, 1400.0, 900.0)).unwrap();
        assert!(lage.brauchbar());
        let s = sitzung::Sitzung {
            fenster: Some(lage),
            ..sitzung::Sitzung::default()
        };
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains("\"breite\":1400.0"), "{text}");
        let wieder: sitzung::Sitzung = serde_json::from_str(&text).unwrap();
        assert_eq!(wieder.fenster, Some(lage));
        app.new_document();
    }
}

#[cfg(test)]
mod tests_sitzung {
    //! T0130: Sitzung merken und wiederherstellen. Geprüft wird der Weg über
    //! die App: Reiter in Reihenfolge, Ansicht, aktives Blatt, ungespeicherte
    //! Zeichnung aus dem Sicherungspool und die Rückfrage beim Schließen.

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-sitzung-app-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn linie(app: &mut App, x: f64) {
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [x, 0.0], "b": [x, 1000.0]}),
            )
            .expect("Linie anlegen");
    }

    fn ansicht(app: &mut App, cx: f64, cy: f64, zoom: f64, rot: f64) {
        let mut v = app.current_view();
        v.center = V2::new(cx, cy);
        v.zoom = zoom;
        v.rot = rot;
        app.set_view(v);
    }

    /// Drei Reiter — zwei gespeichert, einer ungespeichert (mit Stand im Pool):
    /// Reihenfolge, Ansichten, aktiver Reiter und der ungespeicherte Reiter
    /// kommen unverändert zurück.
    #[test]
    fn sitzung_stellt_reiter_ansicht_und_poolstand_wieder_her() {
        let dir = test_dir("reiter");
        let datei_a = dir.join("A.jjcad");
        let datei_b = dir.join("B.jjcad");
        let pool_dir = dir.join("sicherungen");

        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().name = "A".into();
        linie(&mut app, 0.0);
        app.session_mut().save(&datei_a).expect("A speichern");
        app.new_document();
        linie(&mut app, 1000.0);
        app.session_mut().save(&datei_b).expect("B speichern");
        app.new_document();
        linie(&mut app, 2000.0);
        // Die dritte Zeichnung bleibt ungespeichert; ihr Stand liegt im Pool.
        let einst = app.prefs.sicherung_einstellungen();
        let mut pool = jjcad_core::sicherung::Sicherung::oeffnen(&pool_dir);
        pool.sichern(&mut app.doc_mut(), &einst).expect("Stand");
        drop(pool);

        // Je Reiter eine eigene Ansicht.
        ansicht(&mut app, 500.0, 600.0, 0.55, 0.3);
        app.switch_doc(1);
        ansicht(&mut app, 300.0, 400.0, 0.44, 0.0);
        app.switch_doc(0);
        ansicht(&mut app, 100.0, 200.0, 0.33, 0.0);
        app.switch_doc(2);

        let kennungen: Vec<String> = app.tabs.iter().map(|t| t.session.doc.id.clone()).collect();
        let s = app.sitzung_sammeln();
        assert_eq!(s.reiter.len(), 3);
        assert_eq!(s.aktiv, 2);
        assert!(
            s.fenster.is_none(),
            "ohne Fenster (Test) darf keine Fensterlage geschrieben werden"
        );

        let pfad = dir.join("sitzung.json");
        sitzung::speichern(&pfad, &s).expect("Sitzung schreiben");
        let gelesen = sitzung::laden(&pfad)
            .expect("Sitzung lesen")
            .expect("Sitzung vorhanden");

        let mut neu = App::with_ctx(egui::Context::default());
        let meldungen = neu.sitzung_wiederherstellen(&gelesen, &pool_dir);
        assert!(meldungen.is_empty(), "Meldungen: {meldungen:?}");
        assert_eq!(neu.tabs.len(), 3, "drei Reiter in Reihenfolge");
        assert_eq!(neu.active, 2, "der dritte Reiter ist aktiv");

        assert_eq!(neu.tabs[0].session.doc.name, "A");
        assert_eq!(neu.tabs[0].session.doc.path.as_deref(), datei_a.to_str());
        assert_eq!(neu.tabs[1].session.doc.name, "B");
        assert_eq!(neu.tabs[1].session.doc.path.as_deref(), datei_b.to_str());
        assert_eq!(neu.tabs[2].session.doc.name, "Unbenannt");
        assert!(neu.tabs[2].session.doc.path.is_none(), "ungespeichert");
        assert_eq!(neu.tabs[2].session.doc.entities.len(), 1, "Inhalt aus dem Pool");

        for (i, id) in kennungen.iter().enumerate() {
            assert_eq!(&neu.tabs[i].session.doc.id, id, "Kennung {i} bleibt");
        }

        let v = neu.tabs[0].view_model;
        assert_eq!(v.center, V2::new(100.0, 200.0));
        assert!((v.zoom - 0.33).abs() < 1e-12);
        let v = neu.tabs[1].view_model;
        assert_eq!(v.center, V2::new(300.0, 400.0));
        assert!((v.zoom - 0.44).abs() < 1e-12);
        let v = neu.tabs[2].view_model;
        assert_eq!(v.center, V2::new(500.0, 600.0));
        assert!((v.zoom - 0.55).abs() < 1e-12);
        assert!((v.rot - 0.3).abs() < 1e-12, "Drehung der Ansicht");
        assert_eq!(neu.started_at, 3.0, "die Ansicht wird nicht eingepasst");
    }

    /// Ein Reiter mit aktivem Blatt kommt mit Blatt und Blattansicht zurück.
    #[test]
    fn blatt_und_blattansicht_kommen_zurueck() {
        let dir = test_dir("blatt");
        let pool_dir = dir.join("sicherungen");
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        let blatt = app.doc().sheets[0].id;
        app.switch_tab(TabKind::Sheet(blatt));
        ansicht(&mut app, 10.0, 20.0, 3.0, 0.0);

        let s = app.sitzung_sammeln();
        assert_eq!(s.reiter[0].blatt, Some(blatt));
        let pfad = dir.join("sitzung.json");
        sitzung::speichern(&pfad, &s).unwrap();
        let gelesen = sitzung::laden(&pfad).unwrap().unwrap();

        // Ohne Poolstand wird der Reiter weggelassen; mit gespeicherter Datei
        // genügt die Datei.
        let datei = dir.join("Blatt.jjcad");
        app.session_mut().save(&datei).unwrap();
        let mut s = gelesen;
        s.reiter[0].datei = Some(datei.to_string_lossy().to_string());

        let mut neu = App::with_ctx(egui::Context::default());
        let meldungen = neu.sitzung_wiederherstellen(&s, &pool_dir);
        assert!(meldungen.is_empty(), "Meldungen: {meldungen:?}");
        assert_eq!(neu.tabs.len(), 1);
        assert_eq!(neu.tabs[0].tab, TabKind::Sheet(blatt), "Blatt bleibt aktiv");
        let v = neu.tabs[0].sheet_views.get(&blatt).copied().expect("Blattansicht");
        assert_eq!(v.center, V2::new(10.0, 20.0));
        assert!((v.zoom - 3.0).abs() < 1e-12);
    }

    /// Fehlt die Zieldatei **und** der Stand im Pool, wird der Reiter
    /// weggelassen und gemeldet.
    #[test]
    fn fehlende_datei_und_fehlender_stand_laesst_den_reiter_weg() {
        let dir = test_dir("weg");
        let mut s = sitzung::Sitzung::default();
        s.reiter.push(sitzung::Reiter {
            id: "aaaaaaaa-0000-4000-8000-00000000dead".into(),
            datei: Some(dir.join("gibtsnicht.jjcad").to_string_lossy().to_string()),
            name: "Weg".into(),
            ansicht: sitzung::Ansicht::default(),
            blatt: None,
        });
        let mut app = App::with_ctx(egui::Context::default());
        let meldungen = app.sitzung_wiederherstellen(&s, &dir.join("sicherungen"));
        assert_eq!(meldungen.len(), 1, "ein Reiter wird gemeldet");
        assert!(meldungen[0].contains("Weg"), "Meldung nennt den Reiter: {meldungen:?}");
        assert_eq!(app.tabs.len(), 1, "die leere Startzeichnung bleibt");
        assert_eq!(app.doc().entities.len(), 0);
        assert_eq!(app.doc().name, "Unbenannt");
    }

    /// Ohne `sitzung.json` (von Hand gelöscht) startet eine leere Zeichnung.
    #[test]
    fn ohne_sitzungsdatei_startet_eine_leere_zeichnung() {
        let dir = test_dir("leer");
        let pfad = dir.join("sitzung.json");
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        sitzung::speichern(&pfad, &app.sitzung_sammeln()).unwrap();
        assert!(sitzung::laden(&pfad).unwrap().is_some());

        sitzung::entfernen(&pfad).unwrap();
        let s = sitzung::laden(&pfad).unwrap();
        assert!(s.is_none(), "gelöschte Sitzung ist der leere Start");
        assert_eq!(app.tabs.len(), 1);
        // Der leere Start ist eine frische Zeichnung ohne Inhalt.
        assert_eq!(app.tabs[0].session.doc.entities.len(), 1);
    }

    /// Ein wiederhergestellter ungespeicherter Reiter fragt beim Schließen nach
    /// — obwohl ihn niemand angefasst hat (`modified` ist false).
    #[test]
    fn wiederhergestellter_reiter_fragt_beim_schliessen() {
        let dir = test_dir("frage");
        let pool_dir = dir.join("sicherungen");
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        let einst = app.prefs.sicherung_einstellungen();
        let mut pool = jjcad_core::sicherung::Sicherung::oeffnen(&pool_dir);
        pool.sichern(&mut app.doc_mut(), &einst).unwrap();
        drop(pool);

        let s = app.sitzung_sammeln();
        let mut neu = App::with_ctx(egui::Context::default());
        neu.sitzung_wiederherstellen(&s, &pool_dir);
        assert!(!neu.doc().modified, "der Stand gilt als unverändert");
        assert!(
            neu.weicht_von_zieldatei(0),
            "ohne Zieldatei weicht der Reiter mit Inhalt ab"
        );
        neu.request_close_tab(0);
        assert_eq!(neu.ui.confirm_close, Some(0), "die Rückfrage muss kommen");
        assert_eq!(neu.tabs.len(), 1, "noch ist nichts geschlossen");
    }

    /// Gespeicherte Datei schließt ohne Rückfrage — auch wenn der Merker
    /// `modified` (von Hand) gesetzt ist. Maßgeblich ist der Inhalt.
    #[test]
    fn gespeicherte_datei_fragt_auch_mit_merker_nicht() {
        let dir = test_dir("merker");
        let pfad = dir.join("Plan.jjcad");
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        app.session_mut().save(&pfad).unwrap();
        app.doc_mut().modified = true;
        app.request_close_tab(0);
        assert_eq!(app.ui.confirm_close, None, "kein Unterschied zur Datei");
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.doc().entities.len(), 0, "der letzte Reiter wird leer");
    }

    /// Umgekehrt: Inhalt weicht von der Datei ab, obwohl `modified` false ist —
    /// die Rückfrage kommt.
    #[test]
    fn geaenderte_datei_fragt_auch_ohne_merker() {
        let dir = test_dir("geaendert");
        let pfad = dir.join("Plan.jjcad");
        let mut app = App::with_ctx(egui::Context::default());
        linie(&mut app, 0.0);
        app.session_mut().save(&pfad).unwrap();
        linie(&mut app, 500.0);
        app.doc_mut().modified = false;
        app.request_close_tab(0);
        assert_eq!(app.ui.confirm_close, Some(0), "der Inhalt weicht ab");
    }

    /// Eine frische leere Zeichnung ohne Zieldatei schließt ohne Rückfrage.
    #[test]
    fn leere_neue_zeichnung_fragt_nicht() {
        let mut app = App::with_ctx(egui::Context::default());
        assert!(!app.weicht_von_zieldatei(0));
        app.request_close_tab(0);
        assert_eq!(app.ui.confirm_close, None);
    }
}

#[cfg(test)]
mod tests_blockeditor {
    //! T0137: Blockeditor — nur eigene Blöcke, Speichern schreibt die
    //! Benutzerdatei und schließt den Reiter, Schließen ohne Speichern stellt
    //! den Ausgangsstand wieder her.

    use super::*;
    use jjcad_core::model::BlockSource;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-blockeditor-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Zeichnung mit einem Benutzerblock (`usr:stuhl`) aus zwei Linien.
    fn app_mit_benutzerblock(dir: &std::path::Path) -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.blocks_pfad = dir.to_string_lossy().to_string();
        let layer = app.doc().current_layer;
        let mut ents = Vec::new();
        for (i, (a, b)) in [
            ((-100.0, 0.0), (100.0, 0.0)),
            ((0.0, -100.0), (0.0, 100.0)),
        ]
        .iter()
        .enumerate()
        {
            let mut e = Entity::new(
                i as u64 + 1,
                layer,
                EntityKind::Line {
                    a: jjcad_core::geom::v(a.0, a.1),
                    b: jjcad_core::geom::v(b.0, b.1),
                },
            );
            e.by_block = true;
            ents.push(e);
        }
        app.doc_mut().blocks.push(BlockDef {
            id: "usr:stuhl".into(),
            name: "Stuhl".into(),
            category: "Projekt".into(),
            source: BlockSource::User,
            description: "Prüfblock".into(),
            entities: ents,
            icon: None,
            base: V2::ZERO,
            groesse: BlockGroesse::Fest,
        });
        app
    }

    #[test]
    fn nur_eigene_bloecke_bekommen_einen_editor() {
        let dir = test_dir("nur-eigene");
        let mut app = app_mit_benutzerblock(&dir);

        assert!(
            !app.oeffne_block_editor("sys:wc"),
            "ein Systemblock ist schreibgeschützt"
        );
        assert_eq!(app.aktiver_block(), None);
        assert_eq!(app.tab().offene_bloecke.len(), 0);

        assert!(app.oeffne_block_editor("usr:stuhl"));
        assert_eq!(app.aktiver_block().as_deref(), Some("usr:stuhl"));
        assert_eq!(app.doc().block_space.as_deref(), Some("usr:stuhl"));
        assert_eq!(app.tab().offene_bloecke, vec!["usr:stuhl".to_string()]);
    }

    /// T0231: Ein Block ist klein — ein Punktsymbol misst eine Einheit. Mit
    /// der Grenze der Zeichnung (80 px/mm) wäre es im Editor nur wenige Pixel
    /// groß, deshalb gilt dort `BLOCK_ZOOM_MAX`.
    #[test]
    fn blockeditor_zoomt_tiefer_als_die_zeichnung() {
        let dir = test_dir("zoom");
        let mut app = app_mit_benutzerblock(&dir);
        // Ein neutrales Symbol der Einheitsgröße 1, wie die Punktsymbole.
        let layer = app.doc().current_layer;
        app.doc_mut().blocks.push(BlockDef {
            id: "usr:punkt".into(),
            name: "Punkt".into(),
            category: jjcad_core::model::PUNKTSYMBOL_KATEGORIE.into(),
            source: BlockSource::User,
            description: "eine Einheit".into(),
            entities: vec![
                Entity::new(
                    1,
                    layer,
                    EntityKind::Line {
                        a: V2::new(-0.5, 0.0),
                        b: V2::new(0.5, 0.0),
                    },
                ),
                Entity::new(
                    2,
                    layer,
                    EntityKind::Line {
                        a: V2::new(0.0, -0.5),
                        b: V2::new(0.0, 0.5),
                    },
                ),
            ],
            icon: None,
            base: V2::ZERO,
            groesse: BlockGroesse::Papier,
        });
        assert_eq!(app.zoom_max(), 80.0, "in der Zeichnung gilt 80 px/mm");

        assert!(app.oeffne_block_editor("usr:punkt"));
        assert_eq!(
            app.zoom_max(),
            BLOCK_ZOOM_MAX,
            "im Blockeditor gilt die tiefere Grenze"
        );
        // Einpassen auf das eine Einheit große Symbol — der Zoom liegt weit
        // über der Zeichengrenze, sonst wäre der Block ein Punkt im Bild.
        app.ui.last_canvas = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1500.0, 950.0),
        );
        app.fit_view();
        let z = app.current_view().zoom;
        assert!(
            z > 80.0,
            "der Editor zoomt nur auf {z:.1} px/mm — die Zeichnungsgrenze gilt noch"
        );
        // Zoomen bleibt innerhalb der tieferen Grenze.
        app.zoom_by(2.0);
        assert!(app.current_view().zoom <= BLOCK_ZOOM_MAX);
    }

    #[test]
    fn speichern_schreibt_die_benutzerdatei_und_schliesst_den_reiter() {
        let dir = test_dir("speichern");
        let mut app = app_mit_benutzerblock(&dir);
        assert!(app.oeffne_block_editor("usr:stuhl"));

        // Im Blockeditor zeichnen — die Änderung gehört zur Blockdefinition.
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [200.0, 0.0], "b": [300.0, 0.0]}),
            )
            .unwrap();
        assert_eq!(app.doc().block("usr:stuhl").unwrap().entities.len(), 3);

        assert!(app.block_editor_speichern(), "Speichern muss gelingen");

        // Datei liegt im Blockverzeichnis und trägt alle drei Bauteile.
        let dateien = jjcad_core::bloecke::dateien(&dir);
        assert_eq!(dateien.len(), 1, "genau eine Blockdatei");
        let datei = jjcad_core::bloecke::lies(&dateien[0]).expect("Blockdatei lesbar");
        assert_eq!(datei.block.id, "usr:stuhl");
        assert_eq!(datei.block.source, BlockSource::User);
        assert_eq!(
            datei.block.entities.len(),
            3,
            "die neue Linie ist in der Datei"
        );

        // Reiter zu, Blockeditor zu, Modell wieder der Raum.
        assert_eq!(app.aktiver_block(), None);
        assert!(app.tab().offene_bloecke.is_empty());
        assert!(!app.doc().in_block_space());
    }

    /// Ein Bild mit einem Tastendruck. egui liest die Modifier aus
    /// `RawInput::modifiers`, nicht aus dem Ereignis (T0068) — sonst kommt
    /// Strg+S nie als Kombination an.
    fn frame_mit_taste(app: &mut App, key: egui::Key, modifiers: egui::Modifiers) {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1500.0, 950.0),
            )),
            modifiers,
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// T0186: „Speichern“ im Blockeditor (Strg+S) speichert **den Block** und
    /// schließt den Editor. Die Zeichnung auf der Platte bleibt Byte für Byte
    /// stehen — vorher schrieb Strg+S die ganze Datei.
    #[test]
    fn strg_s_im_blockeditor_speichert_nur_den_block() {
        let dir = test_dir("strg-s");
        let mut app = app_mit_benutzerblock(&dir);
        // Eine Zeichnung mit Pfad: ohne den Fix schriebe Strg+S sie neu.
        let pfad = dir.join("Zeichnung.jjcad");
        app.doc_mut().path = Some(pfad.to_string_lossy().to_string());
        app.session_mut().save(&pfad).expect("Zeichnung schreiben");
        let vorher = std::fs::read(&pfad).unwrap();

        assert!(app.oeffne_block_editor("usr:stuhl"));
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [200.0, 0.0], "b": [300.0, 0.0]}),
            )
            .unwrap();

        frame_mit_taste(&mut app, egui::Key::S, egui::Modifiers::COMMAND);

        // Der Editor ist zu und der Block trägt die Änderung.
        assert_eq!(app.aktiver_block(), None, "der Blockeditor schließt sich");
        assert!(app.tab().offene_bloecke.is_empty());
        assert!(!app.doc().in_block_space());
        let dateien = jjcad_core::bloecke::dateien(&dir);
        assert_eq!(dateien.len(), 1, "genau eine Blockdatei");
        let datei = jjcad_core::bloecke::lies(&dateien[0]).expect("Blockdatei lesbar");
        assert_eq!(
            datei.block.entities.len(),
            3,
            "die neue Linie steht im gespeicherten Block"
        );

        // Die Zeichnung wurde nicht angefasst.
        assert_eq!(
            std::fs::read(&pfad).unwrap(),
            vorher,
            "die .jjcad-Datei bleibt unverändert"
        );
    }

    /// T0186: Auch „Speichern unter“ (Strg+Umschalt+S) fragt im Blockeditor
    /// keinen Speicherort für die Zeichnung ab — das Ziel ist die Blockdatei.
    #[test]
    fn speichern_unter_im_blockeditor_oeffnet_keinen_dateidialog() {
        let dir = test_dir("speichern-unter");
        let mut app = app_mit_benutzerblock(&dir);
        app.prefs.file_dialog = "intern".into();
        assert!(app.oeffne_block_editor("usr:stuhl"));

        let mut lage = egui::Modifiers::COMMAND;
        lage.shift = true;
        frame_mit_taste(&mut app, egui::Key::S, lage);

        assert!(app.ui.file_dialog.is_none(), "kein Dateidialog");
        assert!(app.ui.systemdialog.is_none(), "auch kein Systemdialog");
        assert_eq!(app.aktiver_block(), None, "der Blockeditor schließt sich");
        assert_eq!(
            jjcad_core::bloecke::dateien(&dir).len(),
            1,
            "die Blockdatei ist geschrieben"
        );
    }

    #[test]
    fn schliessen_ohne_speichern_stellt_den_stand_wieder_her() {
        let dir = test_dir("verwerfen");
        let mut app = app_mit_benutzerblock(&dir);
        assert!(app.oeffne_block_editor("usr:stuhl"));
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [200.0, 0.0], "b": [300.0, 0.0]}),
            )
            .unwrap();
        assert_eq!(app.doc().block("usr:stuhl").unwrap().entities.len(), 3);

        app.schliesse_block_editor("usr:stuhl", false);

        assert_eq!(
            app.doc().block("usr:stuhl").unwrap().entities.len(),
            2,
            "Schließen ohne Speichern verwirft die Änderung"
        );
        assert!(
            jjcad_core::bloecke::dateien(&dir).is_empty(),
            "es wurde keine Datei geschrieben"
        );
        assert_eq!(app.aktiver_block(), None);
    }

    #[test]
    fn oberflaeche_zeichnet_mit_offenem_blockeditor() {
        // Der Reiter des Blockeditors liegt in der Blattzone, der Benutzerblock
        // in einer eigenen Gruppe — beide müssen ohne Panik gezeichnet werden.
        let dir = test_dir("oberflaeche");
        let mut app = app_mit_benutzerblock(&dir);
        assert!(app.oeffne_block_editor("usr:stuhl"));
        // Denselben Kontext nehmen: nur er kennt die geladenen Schriften.
        let ctx = app.ctx.clone();
        for _ in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1500.0, 950.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| app.ui(ctx));
        }
        assert!(
            app.doc().in_block_space(),
            "der Blockeditor bleibt nach den Frames der aktive Raum"
        );
        // Die Anzeigeliste zeigt den Block (zwei Bauteile), nicht die Zeichnung.
        let prims = app
            .tab()
            .cache
            .as_ref()
            .map(|(_, dl)| dl.prims.len())
            .expect("Anzeigeliste gezeichnet");
        assert_eq!(prims, 2, "Block: Rahmen und Kreis, sonst nichts");
        let reiter = app
            .ui
            .last_blattrechtecke
            .clone()
            .expect("Reiter gezeichnet");
        let blattzahl = app.doc().sheets.len();
        assert_eq!(
            reiter.len(),
            blattzahl + 2,
            "Modell, Blätter und der Blockeditor stehen im Streifen"
        );
    }
}

#[cfg(test)]
mod tests_rechtsklick {
    //! T0138: Der Rechtsklick in eine Schraffur öffnet das **Objektmenü** —
    //! dort steht „Zerlegen“. Vorher traf die Trefferprüfung nur den Rand der
    //! Fläche; der Klick in die Fläche landete im Flächenmenü, das kein
    //! Zerlegen kennt.

    use super::*;

    fn rahmen(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1600.0, 1000.0),
            )),
            events,
            ..Default::default()
        }
    }

    fn sekundaer(pos: Pos2, gedrueckt: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: gedrueckt,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// App mit einer Schraffur 3 × 2 m, Ansicht darauf eingepasst.
    fn app_mit_schraffur() -> (egui::Context, App, Id) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        // T0186: Der Test prüft das Kontextmenü, nicht den Fensterrahmen — die
        // Vorgabe kommt aber aus der `prefs.json` des Rechners. Mit „ohne
        // Titelleiste“ legt `App::fensterkanten` eigene Flächen (Foreground) an,
        // und ein Rechtsklick dicht an der Ecke der Zeichenfläche kommt danach
        // nicht mehr an — das Ergebnis hinge dann an der Einstellung des
        // Benutzers. Also hier fest setzen (dieselbe Regel wie bei den anderen
        // Tests, die Vorgaben prüfen).
        app.prefs.fensterrahmen = jjcad_core::persist::Fensterrahmen::System;
        app.prefs.fensterschatten = false;
        let layer = app.doc().current_layer;
        let id = app.doc_mut().new_id();
        let e = Entity::new(
            id,
            layer,
            EntityKind::Fill {
                boundary: vec![
                    V2::new(0.0, 0.0),
                    V2::new(3000.0, 0.0),
                    V2::new(3000.0, 2000.0),
                    V2::new(0.0, 2000.0),
                ],
                holes: Vec::new(),
                pattern: FillPattern::Lines,
                pattern_angle: 0.0,
                pattern_scale: 200.0,
                border: true,
            },
        );
        app.doc_mut().add_entity(e);
        app.fit_view();
        (ctx, app, id)
    }

    /// Rechtsklick in die **Mitte** der Fläche (nicht auf den Rand): das
    /// Objektmenü muss aufgehen, nur dort steht „Zerlegen“.
    #[test]
    fn rechtsklick_in_die_schraffur_oeffnet_das_objektmenue() {
        let (ctx, mut app, id) = app_mit_schraffur();
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        let pos = world_to_screen(
            app.ui.last_canvas,
            app.current_view(),
            V2::new(1500.0, 1000.0),
        );
        let _ = ctx.run(
            rahmen(vec![egui::Event::PointerMoved(pos), sekundaer(pos, true)]),
            |ctx| app.ui(ctx),
        );
        let _ = ctx.run(rahmen(vec![sekundaer(pos, false)]), |ctx| app.ui(ctx));

        match app.ui.menu.map(|m| m.kind) {
            Some(MenuKind::Object(hit)) => assert_eq!(hit, id, "falsches Objekt im Menü"),
            Some(MenuKind::Canvas) => {
                panic!("Flächenmenü statt Objektmenü — dort fehlt „Zerlegen“")
            }
            None => panic!("kein Menü geöffnet"),
            other => panic!("unerwartetes Menü: {other:?}"),
        }
    }

    /// Gegenprobe: neben der Fläche bleibt es beim Flächenmenü.
    #[test]
    fn rechtsklick_neben_der_schraffur_bleibt_beim_flaechenmenue() {
        let (ctx, mut app, _id) = app_mit_schraffur();
        let _ = ctx.run(rahmen(vec![]), |ctx| app.ui(ctx));
        // Die Ansicht passt die Fläche mit Rand ein — die linke obere Ecke
        // der Zeichenfläche liegt außerhalb.
        let pos = app.ui.last_canvas.left_top() + egui::vec2(4.0, 4.0);
        let _ = ctx.run(
            rahmen(vec![egui::Event::PointerMoved(pos), sekundaer(pos, true)]),
            |ctx| app.ui(ctx),
        );
        let _ = ctx.run(rahmen(vec![sekundaer(pos, false)]), |ctx| app.ui(ctx));

        match app.ui.menu.map(|m| m.kind) {
            Some(MenuKind::Canvas) => {}
            other => panic!("erwartet Flächenmenü, bekommen: {other:?}"),
        }
    }

    /// Alle gemalten Texte eines Bildes samt ihrer Fläche — auch die in
    /// Gruppen (`Shape::Vec`), sonst findet der Test die Menüeinträge nicht.
    fn texte_mit_lage(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        fn sammeln(s: &egui::Shape, ziel: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::Shape::Text(t) => ziel.push((
                    t.galley.text().to_string(),
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                )),
                egui::Shape::Vec(v) => {
                    for s in v {
                        sammeln(s, ziel);
                    }
                }
                _ => {}
            }
        }
        let mut ziel = Vec::new();
        for cs in &out.shapes {
            sammeln(&cs.shape, &mut ziel);
        }
        ziel
    }

    fn primaer(pos: Pos2, gedrueckt: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: gedrueckt,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Rechtsklick an einer Weltstelle und den Rahmen des offenen Menüs
    /// zurückgeben.
    ///
    /// `App::ui` zeichnet die Fläche samt Panels, das Menü hängt in
    /// `App::draw_menu` (in der App ruft `update` beide nacheinander) — der
    /// Test muss den zweiten Schritt selbst gehen, sonst bleibt das Menü
    /// unsichtbar und der Test prüft nichts.
    fn objektmenue_oeffnen(ctx: &egui::Context, app: &mut App) -> egui::FullOutput {
        objektmenue_bei(ctx, app, V2::new(1500.0, 1000.0))
    }

    /// Dasselbe an einer beliebigen Weltstelle (T0185: Blockinstanz).
    fn objektmenue_bei(ctx: &egui::Context, app: &mut App, welt: V2) -> egui::FullOutput {
        let _ = ctx.run(rahmen(vec![]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });
        let pos = world_to_screen(app.ui.last_canvas, app.current_view(), welt);
        let _ = ctx.run(
            rahmen(vec![egui::Event::PointerMoved(pos), sekundaer(pos, true)]),
            |ctx| {
                app.ui(ctx);
                app.draw_menu(ctx);
            },
        );
        let _ = ctx.run(rahmen(vec![sekundaer(pos, false)]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });
        // Das Menü öffnet beim **Loslassen** — erst das nächste Bild trägt
        // seinen Inhalt, und nur dieses Bild ist als Prüfstand brauchbar.
        ctx.run(rahmen(vec![]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        })
    }

    fn eintrag(out: &egui::FullOutput) -> Option<egui::Rect> {
        eintrag_mit(out, "Eigenschaften im Panel")
    }

    /// Fläche eines Menüeintrags mit diesem Text (getrimmt verglichen).
    fn eintrag_mit(out: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
        texte_mit_lage(out)
            .into_iter()
            .find(|(t, _)| t.trim() == text)
            .map(|(_, r)| r)
    }

    /// T0179: Der Eintrag „Eigenschaften im Panel“ steht nur, solange das
    /// Panel nicht offen ist — und sein Klick klappt es auf. Vorher setzte er
    /// nur `props_open`, was bei eingeklapptem Panel schon `true` war: der
    /// Klick tat sichtbar nichts.
    #[test]
    fn eigenschaften_eintrag_oeffnet_das_eingeklappte_panel() {
        let (ctx, mut app, id) = app_mit_schraffur();
        // Ausgangslage: Panel eingeklappt (Leiste am rechten Rand).
        app.ui.props_open = true;
        app.ui.props_collapsed = true;
        let out = objektmenue_oeffnen(&ctx, &mut app);
        let kasten = eintrag(&out).expect("bei eingeklapptem Panel fehlt der Eintrag");

        let klick = kasten.center();
        let _ = ctx.run(
            rahmen(vec![egui::Event::PointerMoved(klick), primaer(klick, true)]),
            |ctx| {
                app.ui(ctx);
                app.draw_menu(ctx);
            },
        );
        let _ = ctx.run(rahmen(vec![primaer(klick, false)]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });
        let out = ctx.run(rahmen(vec![]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });

        assert!(
            app.eigenschaften_panel_offen(),
            "der Klick hat das Panel nicht aufgeklappt (open={}, collapsed={})",
            app.ui.props_open,
            app.ui.props_collapsed
        );
        assert!(app.selection().contains(&id), "das Objekt ist nicht ausgewählt");
        assert!(
            eintrag(&out).is_none(),
            "der Eintrag steht noch im Menü, obwohl das Panel offen ist"
        );
    }

    /// Gegenprobe zur Sichtbarkeit: ausgeschaltet (gar kein Panel) wird der
    /// Eintrag angeboten, offen stehend nicht.
    #[test]
    fn eigenschaften_eintrag_nur_bei_geschlossenem_panel() {
        let (ctx, mut app, _id) = app_mit_schraffur();
        app.ui.props_open = false;
        app.ui.props_collapsed = false;
        let out = objektmenue_oeffnen(&ctx, &mut app);
        assert!(
            eintrag(&out).is_some(),
            "ohne Panel muss der Eintrag angeboten werden"
        );

        app.ui.props_open = true;
        app.ui.props_collapsed = false;
        let out = ctx.run(rahmen(vec![]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });
        assert!(
            eintrag(&out).is_none(),
            "bei offenem Panel darf der Eintrag nicht erscheinen"
        );
    }

    /// T0180: Eine bewegte Ansicht nimmt das Kontextmenü mit. Es hängt an einer
    /// Bildschirmstelle — die Zeichnung darunter wandert beim Schwenken und
    /// Zoomen weg, der Eintrag zeigte dann auf ein anderes Objekt. Alle
    /// Bewegungen laufen durch `App::set_view`.
    ///
    /// Je Bewegung eine frische App: ein zweiter Rechtsklick auf dieselbe
    /// Stelle gilt in egui als Doppelklick und öffnet das Menü nicht erneut.
    #[test]
    fn schwenken_schliesst_das_kontextmenue() {
        let (ctx, mut app, _id) = app_mit_schraffur();
        let _ = objektmenue_oeffnen(&ctx, &mut app);
        assert!(app.menu_open(), "Vorbedingung: das Menü steht offen");

        // Schwenk um einen Schritt (60 px, wie `view.pan_*`).
        app.schwenk_schritt(60.0, 0.0);
        assert!(!app.menu_open(), "das Menü blieb beim Schwenken offen");
    }

    #[test]
    fn zoomen_schliesst_das_kontextmenue() {
        let (ctx, mut app, _id) = app_mit_schraffur();
        let _ = objektmenue_oeffnen(&ctx, &mut app);
        assert!(app.menu_open(), "Vorbedingung: das Menü steht offen");

        app.zoom_by(1.25);
        assert!(!app.menu_open(), "das Menü blieb beim Zoomen offen");
    }

    #[test]
    fn einpassen_schliesst_das_kontextmenue() {
        let (ctx, mut app, _id) = app_mit_schraffur();
        let _ = objektmenue_oeffnen(&ctx, &mut app);
        assert!(app.menu_open(), "Vorbedingung: das Menü steht offen");

        app.fit_view();
        assert!(!app.menu_open(), "das Menü blieb beim Einpassen offen");
    }

    /// T0180: Eine neue Zeichenfunktion schließt das Menü. Der Klick auf
    /// Ribbon/Hotbar täte das ohnehin (Klick außerhalb), ein Tastenkürzel oder
    /// eine Hotbar-Ziffer nicht — dort blieb das Menü über dem startenden
    /// Werkzeug stehen.
    #[test]
    fn werkzeugstart_schliesst_das_kontextmenue() {
        let (ctx, mut app, _id) = app_mit_schraffur();
        let _ = objektmenue_oeffnen(&ctx, &mut app);
        assert!(app.menu_open(), "Vorbedingung: das Menü steht offen");

        app.start_tool("draw.wall");
        assert!(
            !app.menu_open(),
            "das Menü blieb beim Start des Werkzeugs offen"
        );
        assert_eq!(app.tool_id(), Some("draw.wall"), "das Werkzeug läuft nicht");
    }

    /// T0180: Auch das Einfügen (Zwischenablage) ist eine Zeichenfunktion.
    #[test]
    fn einfuegen_schliesst_das_kontextmenue() {
        let (ctx, mut app, _id) = app_mit_schraffur();
        let _ = objektmenue_oeffnen(&ctx, &mut app);
        assert!(app.menu_open(), "Vorbedingung: das Menü steht offen");

        let id = app.doc().ents()[0].id;
        app.merke_zwischenablage(&[id], None);
        app.start_ghost_paste();
        assert!(!app.menu_open(), "das Menü blieb beim Einfügen offen");
        assert!(app.ghost.is_some(), "die Einfüge-Vorschau läuft nicht");
    }

    // ---------------------------------------------------------------------
    // T0185: Blockinstanz direkt aus dem Rechtsklickmenü bearbeiten
    // ---------------------------------------------------------------------

    /// Zeichnung mit einer eingesetzten Blockinstanz (`usr:bank`) aus einer
    /// waagerechten Linie. Die Quelle bestimmt, ob der Block bearbeitbar ist.
    fn app_mit_blockinstanz(quelle: BlockSource) -> (egui::Context, App) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let layer = app.doc().current_layer;
        let mut kind = Entity::new(
            10_000,
            layer,
            EntityKind::Line {
                a: V2::new(-1000.0, 0.0),
                b: V2::new(1000.0, 0.0),
            },
        );
        kind.by_block = true;
        app.doc_mut().blocks.push(BlockDef {
            id: "usr:bank".into(),
            name: "Bank".into(),
            category: "Projekt".into(),
            source: quelle,
            description: String::new(),
            entities: vec![kind],
            icon: None,
            base: V2::ZERO,
            groesse: BlockGroesse::Fest,
        });
        let id = app.doc_mut().new_id();
        let instanz = Entity::new(
            id,
            layer,
            EntityKind::BlockRef {
                block: "usr:bank".into(),
                pos: V2::ZERO,
                rot: 0.0,
                scale: V2::new(1.0, 1.0),
                mirror: false,
            },
        );
        app.doc_mut().add_entity(instanz);
        app.fit_view();
        (ctx, app)
    }

    /// Der Eintrag „Block bearbeiten“ öffnet den Blockeditor der Instanz —
    /// nicht den der zuletzt gewählten Blockbibliothek.
    #[test]
    fn blockinstanz_bietet_bearbeiten_und_oeffnet_den_editor() {
        let (ctx, mut app) = app_mit_blockinstanz(BlockSource::User);
        let out = objektmenue_bei(&ctx, &mut app, V2::new(0.0, 0.0));
        assert!(
            app.menu_open(),
            "der Rechtsklick auf die Blockinstanz öffnete kein Menü"
        );
        let kasten = eintrag_mit(&out, "Block bearbeiten")
            .expect("im Objektmenü fehlt der Eintrag „Block bearbeiten“");

        let klick = kasten.center();
        let _ = ctx.run(
            rahmen(vec![egui::Event::PointerMoved(klick), primaer(klick, true)]),
            |ctx| {
                app.ui(ctx);
                app.draw_menu(ctx);
            },
        );
        let _ = ctx.run(rahmen(vec![primaer(klick, false)]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });

        assert_eq!(
            app.aktiver_block().as_deref(),
            Some("usr:bank"),
            "der Klick hat den Blockeditor nicht geöffnet"
        );
        assert!(!app.menu_open(), "das Menü blieb nach dem Klick offen");
    }

    /// Gegenprobe: ein Systemblock ist schreibgeschützt — der Eintrag fehlt,
    /// genau wie im Rechtsklick der Blockbibliothek.
    #[test]
    fn systemblock_bietet_kein_bearbeiten_an() {
        let (ctx, mut app) = app_mit_blockinstanz(BlockSource::System);
        let out = objektmenue_bei(&ctx, &mut app, V2::new(0.0, 0.0));
        assert!(
            eintrag_mit(&out, "Block bearbeiten").is_none(),
            "ein Systemblock darf keinen Editor anbieten"
        );
    }

    // ---------------------------------------------------------------------
    // T0189: Rechtsklick in einer Mehrfachauswahl
    // ---------------------------------------------------------------------

    /// Zeichnung mit **zwei** Schraffuren nebeneinander (je 3 × 2 m), Ansicht
    /// darauf eingepasst. Liefert die beiden Kennungen.
    fn app_mit_zwei_flaechen() -> (egui::Context, App, Id, Id) {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.prefs.fensterrahmen = jjcad_core::persist::Fensterrahmen::System;
        app.prefs.fensterschatten = false;
        let layer = app.doc().current_layer;
        for x in [0.0, 4000.0] {
            let id = app.doc_mut().new_id();
            let e = Entity::new(
                id,
                layer,
                EntityKind::Fill {
                    boundary: vec![
                        V2::new(x, 0.0),
                        V2::new(x + 3000.0, 0.0),
                        V2::new(x + 3000.0, 2000.0),
                        V2::new(x, 2000.0),
                    ],
                    holes: Vec::new(),
                    pattern: FillPattern::Lines,
                    pattern_angle: 0.0,
                    pattern_scale: 200.0,
                    border: true,
                },
            );
            app.doc_mut().add_entity(e);
        }
        app.fit_view();
        let ids: Vec<Id> = app.doc().ents().iter().map(|e| e.id).collect();
        (ctx, app, ids[0], ids[1])
    }

    /// Klickt einen Menüeintrag an (Fläche aus dem Menübild).
    fn klicke_eintrag(ctx: &egui::Context, app: &mut App, kasten: egui::Rect) {
        let klick = kasten.center();
        let _ = ctx.run(
            rahmen(vec![egui::Event::PointerMoved(klick), primaer(klick, true)]),
            |ctx| {
                app.ui(ctx);
                app.draw_menu(ctx);
            },
        );
        let _ = ctx.run(rahmen(vec![primaer(klick, false)]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });
        let _ = ctx.run(rahmen(vec![]), |ctx| {
            app.ui(ctx);
            app.draw_menu(ctx);
        });
    }

    /// Der Rechtsklick auf ein Objekt, das zur Auswahl gehört, lässt die
    /// Auswahl stehen. Vorher blieb nur das angeklickte Objekt übrig, und
    /// „Löschen“, „Verschieben“ & Co. trafen nur noch dieses.
    #[test]
    fn rechtsklick_in_der_mehrfachauswahl_haelt_die_auswahl() {
        let (ctx, mut app, a, b) = app_mit_zwei_flaechen();
        app.session_mut().select(vec![a, b]);
        // Rechtsklick in die Mitte der ersten Fläche.
        let _ = objektmenue_oeffnen(&ctx, &mut app);

        assert!(
            matches!(app.ui.menu.map(|m| m.kind), Some(MenuKind::Object(hit)) if hit == a),
            "das Menü zeigt nicht auf das angeklickte Objekt"
        );
        let sel = app.selection().to_vec();
        assert_eq!(
            sel.len(),
            2,
            "der Rechtsklick hat die Auswahl auf {sel:?} verengt"
        );
        assert!(
            sel.contains(&a) && sel.contains(&b),
            "die Auswahl ist nicht mehr vollständig: {sel:?}"
        );
    }

    /// Gegenprobe: ein Rechtsklick **neben** der Auswahl wählt wie bisher das
    /// angeklickte Objekt allein.
    #[test]
    fn rechtsklick_ausserhalb_der_auswahl_waehlt_das_objekt() {
        let (ctx, mut app, a, b) = app_mit_zwei_flaechen();
        app.session_mut().select(vec![a]);
        // Mitte der zweiten Fläche (4000…7000).
        let _ = objektmenue_bei(&ctx, &mut app, V2::new(5500.0, 1000.0));

        assert_eq!(
            app.selection().to_vec(),
            vec![b],
            "der Rechtsklick neben der Auswahl wählt nicht das getroffene Objekt"
        );
        assert!(
            matches!(app.ui.menu.map(|m| m.kind), Some(MenuKind::Object(hit)) if hit == b),
            "das Menü zeigt nicht auf das getroffene Objekt"
        );
    }

    /// „Löschen“ im Menü trifft **alle** Objekte der Auswahl, nicht nur das
    /// angeklickte.
    #[test]
    fn loeschen_im_menue_trifft_die_ganze_auswahl() {
        let (ctx, mut app, a, b) = app_mit_zwei_flaechen();
        app.session_mut().select(vec![a, b]);
        let out = objektmenue_oeffnen(&ctx, &mut app);
        let kasten = eintrag_mit(&out, "Löschen").expect("im Objektmenü fehlt „Löschen“");
        klicke_eintrag(&ctx, &mut app, kasten);

        assert!(
            app.doc().entity(a).is_none(),
            "das angeklickte Objekt steht noch in der Zeichnung"
        );
        assert!(
            app.doc().entity(b).is_none(),
            "das zweite Objekt der Auswahl steht noch — „Löschen“ traf nur eines"
        );
        assert!(
            app.selection().is_empty(),
            "die Auswahl zeigt nach dem Löschen noch Objekte"
        );
    }

    /// „Verschieben“ startet für die ganze Auswahl: die Auswahl bleibt stehen,
    /// das Werkzeug arbeitet auf ihr (so macht es auch die Zeichenfläche).
    #[test]
    fn verschieben_im_menue_behaelt_die_ganze_auswahl() {
        let (ctx, mut app, a, b) = app_mit_zwei_flaechen();
        app.session_mut().select(vec![a, b]);
        let out = objektmenue_oeffnen(&ctx, &mut app);
        let kasten = eintrag_mit(&out, "Verschieben").expect("im Objektmenü fehlt „Verschieben“");
        klicke_eintrag(&ctx, &mut app, kasten);

        assert_eq!(app.tool_id(), Some("modify.move"), "das Werkzeug läuft nicht");
        assert_eq!(
            app.selection().to_vec().len(),
            2,
            "die Auswahl für das Werkzeug ist geschrumpft"
        );
    }

    /// Der Menükopf nennt bei mehreren Objekten die Anzahl statt einer
    /// Objektart.
    #[test]
    fn menuekopf_nennt_bei_mehreren_objekten_die_anzahl() {
        let (ctx, mut app, a, b) = app_mit_zwei_flaechen();
        app.session_mut().select(vec![a, b]);
        let out = objektmenue_oeffnen(&ctx, &mut app);
        assert!(
            eintrag_mit(&out, "2 Objekte").is_some(),
            "der Menükopf nennt nicht die Anzahl der gewählten Objekte"
        );
    }

    /// Bei einem einzelnen Objekt bleibt der Kopf beim Typ (und nennt keine
    /// Anzahl).
    #[test]
    fn menuekopf_zeigt_bei_einem_objekt_die_art() {
        let (ctx, mut app, a, _b) = app_mit_zwei_flaechen();
        app.session_mut().select(vec![a]);
        let out = objektmenue_oeffnen(&ctx, &mut app);
        assert!(
            eintrag_mit(&out, "Füllfläche").is_some(),
            "der Menükopf nennt nicht die Objektart"
        );
    }
}

/// Tangentialfang (T0157): der Schalter im Fangmenü und in den Einstellungen
/// muss in der Zeichnung ankommen. Der Weg ist `App::set_snap` — ein
/// Schlüssel, der dort fehlt, wirkt im Menü und bleibt im Fang stumm.
#[cfg(test)]
mod tests_tangentialfang {
    use super::*;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app
    }

    #[test]
    fn schalter_erreicht_die_zeichnung() {
        let mut app = app();
        assert!(app.doc().settings.snap.tangent, "Vorgabe ist an");
        app.set_snap("tangent", false);
        assert!(
            !app.doc().settings.snap.tangent,
            "Tangentialfang ließ sich nicht abschalten"
        );
        app.set_snap("tangent", true);
        assert!(
            app.doc().settings.snap.tangent,
            "Tangentialfang ließ sich nicht einschalten"
        );
    }
}

/// Lotfang (T0216): derselbe Weg wie beim Tangentialfang — der Schalter im
/// Fangmenü und in den Einstellungen muss in der Zeichnung ankommen. Ein
/// Schlüssel, der in `App::set_snap` fehlt, wirkt im Menü und bleibt im Fang
/// stumm.
#[cfg(test)]
mod tests_lotfang {
    use super::*;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app
    }

    #[test]
    fn schalter_erreicht_die_zeichnung() {
        let mut app = app();
        assert!(app.doc().settings.snap.lot, "Vorgabe ist an");
        app.set_snap("lot", false);
        assert!(
            !app.doc().settings.snap.lot,
            "Lotfang ließ sich nicht abschalten"
        );
        app.set_snap("lot", true);
        assert!(app.doc().settings.snap.lot, "Lotfang ließ sich nicht einschalten");
    }
}

/// Grafikeinstellungen (T0165): Kantenglättung, Kantenweichheit und
/// Leistungsanzeige gelten **ab dem ersten Bild**. Der Prüfweg schreibt keine
/// Voreinstellungen — er vergleicht die Anzeige mit dem, was `prefs.json`
/// tatsächlich vorgibt (so bleibt der Lauf auf jedem Rechner gültig).
#[cfg(test)]
mod tests_grafikeinstellungen {
    use super::*;

    #[test]
    fn kanten_und_leistungsanzeige_gelten_ab_dem_ersten_bild() {
        let ctx = egui::Context::default();
        let app = App::with_ctx(ctx.clone());
        let erwartet = app.prefs.kantenglaettung
            && std::env::var("JJCAD_UI_FEATHER").ok().as_deref() != Some("0");
        assert_eq!(
            ctx.tessellation_options(|o| o.feathering),
            erwartet,
            "die Kantenglättung aus prefs.json steht nicht im Zeichner"
        );
        let breite = app.prefs.kantenweichheit.clamp(
            jjcad_core::persist::KANTENWEICHHEIT_MIN,
            jjcad_core::persist::KANTENWEICHHEIT_MAX,
        );
        assert_eq!(
            ctx.tessellation_options(|o| o.feathering_size_in_pixels),
            breite,
            "die Kantenweichheit aus prefs.json steht nicht im Zeichner"
        );
        assert_eq!(
            app.perf.an, app.prefs.perf_anzeige,
            "die Leistungsanzeige folgt nicht der gespeicherten Wahl"
        );
    }
}

#[cfg(test)]
mod tests_ansicht_verankern {
    //! T0184: Öffnen und Schließen der Seiten-Panels verschiebt die Zeichnung
    //! nicht mehr.
    //!
    //! Die Ansicht hängt an der **Mitte** der Zeichenfläche. Ändert sich nur die
    //! Fläche (Panel auf oder zu), hält `App::ansicht_verankern` den Weltpunkt am
    //! linken oberen Fensterrand fest — die Fläche wächst dann nur in eine
    //! Richtung. Geprüft wird die Bildschirmlage eines Weltpunkts über dieselbe
    //! Umrechnung, die auch die Zeichenfläche benutzt (`world_to_screen`).

    use super::*;

    fn bild(app: &mut App, breite: f32, hoehe: f32) {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(breite, hoehe))),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    /// Bildschirmlage eines Weltpunkts.
    fn lage(app: &App, p: V2) -> Pos2 {
        world_to_screen(app.ui.last_canvas, app.current_view(), p)
    }

    /// App mit geschlossenen Seiten-Panels, zwei Bilder gelaufen — die
    /// Zeichenfläche steht danach fest.
    fn app_ohne_panels() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app.ui.props_open = false;
        app.ui.layers_open = false;
        app.ui.props_collapsed = false;
        app.ui.layers_collapsed = false;
        app.ui.ribbon_collapsed = false;
        bild(&mut app, 1500.0, 950.0);
        bild(&mut app, 1500.0, 950.0);
        app
    }

    /// Prüfpunkt: die Zeichnung steht nach dem Umschalten an derselben Stelle.
    fn bleibt_stehen(app: &mut App, umschalten: impl FnOnce(&mut App)) {
        let punkt = V2::new(3000.0, 2000.0);
        let vorher = lage(app, punkt);
        let flaeche_vorher = app.ui.last_canvas;
        umschalten(app);
        bild(app, 1500.0, 950.0);
        bild(app, 1500.0, 950.0);
        assert!(
            !crate::canvas::flaechen_gleich(app.ui.last_canvas, flaeche_vorher),
            "die Zeichenfläche muss sich geändert haben, sonst prüft der Test nichts"
        );
        let nachher = lage(app, punkt);
        assert!(
            (nachher - vorher).length() < 0.5,
            "die Zeichnung ist um {:.1} px gewandert: {vorher:?} → {nachher:?}",
            (nachher - vorher).length()
        );
    }

    #[test]
    fn eigenschaften_panel_verschiebt_die_zeichnung_nicht() {
        let mut app = app_ohne_panels();
        bleibt_stehen(&mut app, |a| a.ui.props_open = true);
    }

    #[test]
    fn layer_panel_verschiebt_die_zeichnung_nicht() {
        let mut app = app_ohne_panels();
        bleibt_stehen(&mut app, |a| a.ui.layers_open = true);
    }

    #[test]
    fn beide_panels_verschieben_die_zeichnung_nicht() {
        let mut app = app_ohne_panels();
        bleibt_stehen(&mut app, |a| {
            a.ui.props_open = true;
            a.ui.layers_open = true;
        });
    }

    /// 28 px statt 292 px: die Rechnung hängt an der Fläche, nicht an der
    /// Panelbreite — deshalb gilt sie auch für künftige Panels anderer Größe.
    #[test]
    fn eingeklapptes_panel_verschiebt_ebenfalls_nicht() {
        let mut app = app_ohne_panels();
        app.ui.props_open = true;
        bild(&mut app, 1500.0, 950.0);
        bild(&mut app, 1500.0, 950.0);
        bleibt_stehen(&mut app, |a| a.ui.props_collapsed = true);
    }

    /// Auch ein Panel oben (Ribbon einklappen) hält die Zeichnung fest.
    #[test]
    fn eingeklappter_ribbon_verschiebt_die_zeichnung_nicht() {
        let mut app = app_ohne_panels();
        bleibt_stehen(&mut app, |a| a.ui.ribbon_collapsed = true);
    }

    /// Schließen stellt die Lage wieder her — hin und zurück bleibt sie gleich.
    #[test]
    fn schliessen_stellt_die_lage_wieder_her() {
        let mut app = app_ohne_panels();
        let punkt = V2::new(3000.0, 2000.0);
        let vorher = lage(&app, punkt);
        app.ui.props_open = true;
        bild(&mut app, 1500.0, 950.0);
        bild(&mut app, 1500.0, 950.0);
        app.ui.props_open = false;
        bild(&mut app, 1500.0, 950.0);
        bild(&mut app, 1500.0, 950.0);
        let nachher = lage(&app, punkt);
        assert!(
            (nachher - vorher).length() < 0.5,
            "nach dem Schließen steht die Zeichnung nicht wieder am Platz: \
             {vorher:?} → {nachher:?}"
        );
    }

    /// Gegenprobe zum Verhalten beim **Fenster**-Vergrößern: dort bleibt es
    /// beim Einpassen um die Mitte (die Fläche ändert sich mit dem Fenster).
    #[test]
    fn fenster_vergroessern_haelt_die_mitte() {
        let mut app = app_ohne_panels();
        let mitte = app.current_view().center;
        bild(&mut app, 1800.0, 1100.0);
        bild(&mut app, 1800.0, 1100.0);
        assert!(
            (app.current_view().center - mitte).len() < 1.0,
            "beim Vergrößern des Fensters muss die Mitte stehen bleiben: \
             {mitte:?} → {:?}",
            app.current_view().center
        );
    }
}

#[cfg(test)]
mod tests_seitenleisten {
    //! T0203: Layer und Eigenschaften lassen sich auf **beiden** Seiten
    //! ablegen. Die Leisten beider Seiten tragen dieselben Symbole; ein Klick
    //! legt das Panel auf diese Seite und klappt es aus.
    //!
    //! Geprüft wird an den Flächen, die `App::leiste` je Symbol meldet
    //! (`UiState::last_leisten_icons`) — dieselbe Stelle, die auch der Klick
    //! trifft.

    use super::*;

    fn bild(app: &mut App, events: Vec<egui::Event>) {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.ui(ctx));
    }

    fn knopf(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn klick(app: &mut App, pos: Pos2) {
        for events in [
            vec![egui::Event::PointerMoved(pos)],
            vec![knopf(pos, true)],
            vec![knopf(pos, false)],
            vec![],
        ] {
            bild(app, events);
        }
    }

    /// Beide Panels eingeklappt: Layer links, Eigenschaften rechts.
    fn app_mit_leisten() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app.ui.layers_open = true;
        app.ui.props_open = true;
        app.ui.layers_collapsed = true;
        app.ui.props_collapsed = true;
        app.ui.layers_side = PanelSeite::Links;
        app.ui.props_side = PanelSeite::Rechts;
        app.ui.ribbon_collapsed = false;
        // T0233: Die Prüfungen dieser Gruppe gelten dem Stand mit „Seitenleiste
        // immer anzeigen“ — beide Leisten stehen, zwei Panels je Seite. Der
        // Programmstandard (Leiste weg, ein Panel) hat eigene Tests.
        app.prefs.seitenleiste_immer = true;
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());
        app
    }

    fn symbol(app: &App, p: SeitenPanel, seite: PanelSeite) -> Rect {
        app.ui
            .last_leisten_icons
            .iter()
            .find(|(pp, ss, _)| *pp == p && *ss == seite)
            .map(|(_, _, r)| *r)
            .unwrap_or_else(|| panic!("Symbol {p:?} fehlt in der Leiste {seite:?}"))
    }

    /// Die eine Forderung des Tickets: **beide** Leisten zeigen **dieselben**
    /// Symbole, in derselben Reihenfolge und auf derselben Höhe.
    #[test]
    fn beide_leisten_zeigen_dieselben_symbole() {
        let app = app_mit_leisten();
        let mut links: Vec<(SeitenPanel, f32)> = app
            .ui
            .last_leisten_icons
            .iter()
            .filter(|(_, s, _)| *s == PanelSeite::Links)
            .map(|(p, _, r)| (*p, r.center().y))
            .collect();
        let mut rechts: Vec<(SeitenPanel, f32)> = app
            .ui
            .last_leisten_icons
            .iter()
            .filter(|(_, s, _)| *s == PanelSeite::Rechts)
            .map(|(p, _, r)| (*p, r.center().y))
            .collect();
        links.sort_by(|a, b| a.1.total_cmp(&b.1));
        rechts.sort_by(|a, b| a.1.total_cmp(&b.1));
        assert_eq!(links.len(), 3, "die linke Leiste muss alle Panels tragen");
        assert_eq!(rechts.len(), 3, "die rechte Leiste muss alle Panels tragen");
        assert_eq!(
            links, rechts,
            "die Leisten müssen dieselben Symbole in derselben Reihenfolge zeigen"
        );
    }

    /// T0222: Die Leiste steht **immer** — auch wenn kein Panel eingeklappt
    /// ist. Vorher verschwand sie, sobald drei Panels ausgeklappt waren; mit
    /// ihr war der einzige Weg zum Wechseln weg.
    #[test]
    fn leiste_steht_auch_ohne_eingeklapptes_panel() {
        let mut app = app_mit_leisten();
        app.ui.layers_collapsed = false;
        app.ui.props_collapsed = false;
        bild(&mut app, Vec::new());
        let je_seite = |seite: PanelSeite| {
            app.ui
                .last_leisten_icons
                .iter()
                .filter(|(_, s, _)| *s == seite)
                .count()
        };
        assert_eq!(
            je_seite(PanelSeite::Links),
            3,
            "die linke Leiste muss auch ohne eingeklapptes Panel stehen"
        );
        assert_eq!(
            je_seite(PanelSeite::Rechts),
            3,
            "die rechte Leiste muss auch ohne eingeklapptes Panel stehen"
        );
    }

    /// T0222: Je Seite höchstens zwei ausgeklappte Panels — das dritte gibt
    /// auf und liegt als Symbol in der Leiste.
    #[test]
    fn hoechstens_zwei_panels_je_seite() {
        let mut app = app_mit_leisten();
        for p in SeitenPanel::ALLE {
            app.panel_setze(p, true, false, PanelSeite::Links);
        }
        bild(&mut app, Vec::new());
        let offen: Vec<SeitenPanel> = SeitenPanel::ALLE
            .iter()
            .copied()
            .filter(|p| {
                app.panel_seite(*p) == PanelSeite::Links
                    && app.panel_offen(*p)
                    && !app.panel_eingeklappt(*p)
            })
            .collect();
        assert_eq!(
            offen.len(),
            2,
            "je Seite dürfen höchstens zwei Panels ausgeklappt sein, offen: {offen:?}"
        );
        // Erreichbar bleibt jedes: die Leiste trägt alle drei Symbole.
        let links = app
            .ui
            .last_leisten_icons
            .iter()
            .filter(|(_, s, _)| *s == PanelSeite::Links)
            .count();
        assert_eq!(links, 3, "die Leiste muss alle drei Panels anbieten");
    }

    /// T0222: Ein Klick auf das dritte Panel gibt das am längsten unbenutzte
    /// derselben Seite auf (Reihenfolge in `UiState::panel_benutzt`).
    #[test]
    fn drittes_panel_verdraengt_das_laengste_unbenutzte() {
        let mut app = app_mit_leisten();
        app.ui.layers_side = PanelSeite::Links;
        app.ui.props_side = PanelSeite::Links;
        app.ui.blatt_side = PanelSeite::Links;
        app.ui.layers_collapsed = false;
        app.ui.props_collapsed = false;
        app.ui.blatt_open = true;
        app.ui.blatt_collapsed = true;
        app.ui.panel_benutzt = vec![SeitenPanel::Layer, SeitenPanel::Eigenschaften];
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());

        let r = symbol(&app, SeitenPanel::Blatt, PanelSeite::Links);
        klick(&mut app, r.center());

        assert!(
            !app.ui.blatt_collapsed,
            "das geklickte Blatt-Panel muss ausgeklappt sein"
        );
        assert!(
            app.ui.layers_collapsed,
            "das am längsten unbenutzte Panel (Layer) muss aufgeben"
        );
        assert!(
            !app.ui.props_collapsed,
            "das zuletzt benutzte Eigenschaften-Panel bleibt stehen"
        );
    }

    /// T0233: Programmstandard („Seitenleiste immer anzeigen“ aus) — auf einer
    /// Seite mit ausgeklapptem Panel entfällt die Leiste, die andere bleibt.
    #[test]
    fn standard_blendet_die_leiste_der_panel_seite_aus() {
        let mut app = app_mit_leisten();
        app.prefs.seitenleiste_immer = false;
        app.ui.layers_collapsed = false; // Layer links ausgeklappt
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());
        let je_seite = |seite: PanelSeite| {
            app.ui
                .last_leisten_icons
                .iter()
                .filter(|(_, s, _)| *s == seite)
                .count()
        };
        assert_eq!(
            je_seite(PanelSeite::Links),
            0,
            "links steht ein Panel — die Leiste gehört ausgeblendet"
        );
        assert_eq!(
            je_seite(PanelSeite::Rechts),
            3,
            "rechts ist kein Panel offen — die Leiste bleibt stehen"
        );
    }

    /// T0233: „Immer anzeigen“ an — beide Leisten stehen auch neben einem
    /// ausgeklappten Panel, und zwei Panels dürfen gleichzeitig offen sein.
    #[test]
    fn immer_an_laesst_beide_leisten_und_zwei_panels_stehen() {
        let mut app = app_mit_leisten(); // setzt seitenleiste_immer = true
        app.ui.props_side = PanelSeite::Links;
        app.ui.layers_collapsed = false;
        app.ui.props_collapsed = false;
        app.ui.panel_benutzt = vec![SeitenPanel::Eigenschaften, SeitenPanel::Layer];
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());
        let ausgeklappt = SeitenPanel::ALLE
            .iter()
            .filter(|p| app.panel_offen(**p) && !app.panel_eingeklappt(**p))
            .count();
        assert_eq!(
            ausgeklappt, 2,
            "mit „immer anzeigen“ bleiben zwei Panels offen"
        );
        for seite in PanelSeite::ALL {
            let n = app
                .ui
                .last_leisten_icons
                .iter()
                .filter(|(_, s, _)| *s == seite)
                .count();
            assert_eq!(n, 3, "die Leiste {seite:?} muss trotz offener Panels stehen");
        }
    }

    /// T0233: Standardbetrieb hält **je Seite ein** Panel offen — das am
    /// längsten unbenutzte derselben Seite gibt auf, die andere Seite behält
    /// ihres.
    #[test]
    fn standard_haelt_ein_panel_je_seite() {
        let mut app = app_mit_leisten();
        app.prefs.seitenleiste_immer = false;
        // Links zwei Panels, rechts eines: links gibt das ältere auf.
        app.ui.props_side = PanelSeite::Links;
        app.ui.blatt_side = PanelSeite::Rechts;
        app.ui.blatt_open = true;
        app.ui.layers_collapsed = false;
        app.ui.props_collapsed = false;
        app.ui.blatt_collapsed = false;
        app.ui.panel_benutzt = vec![
            SeitenPanel::Blatt,
            SeitenPanel::Eigenschaften,
            SeitenPanel::Layer,
        ];
        app.panels_begrenzen();
        let offen: Vec<SeitenPanel> = SeitenPanel::ALLE
            .iter()
            .copied()
            .filter(|p| app.panel_offen(*p) && !app.panel_eingeklappt(*p))
            .collect();
        assert_eq!(
            offen,
            vec![SeitenPanel::Layer, SeitenPanel::Blatt],
            "je Seite bleibt das zuletzt benutzte Panel offen"
        );
    }

    /// T0233: Im Standardbetrieb schließt ein Klick in die Leiste die übrigen
    /// Panels **derselben** Seite — das Panel der anderen Seite bleibt stehen.
    #[test]
    fn standard_klick_schliesst_nur_die_panels_derselben_seite() {
        let mut app = app_mit_leisten();
        app.prefs.seitenleiste_immer = false;
        // Rechts steht der Blattmanager; links liegt alles in der Leiste.
        app.ui.layers_side = PanelSeite::Links;
        app.ui.props_side = PanelSeite::Links;
        app.ui.blatt_side = PanelSeite::Rechts;
        app.ui.layers_collapsed = true;
        app.ui.props_collapsed = true;
        app.ui.blatt_open = true;
        app.ui.blatt_collapsed = false;
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());
        let r = symbol(&app, SeitenPanel::Layer, PanelSeite::Links);
        klick(&mut app, r.center());
        assert!(
            app.ui.layers_open && !app.ui.layers_collapsed,
            "das geklickte Layer-Panel muss ausgeklappt sein"
        );
        assert!(
            !app.ui.props_open,
            "das zweite Panel derselben Seite muss geschlossen sein"
        );
        assert!(
            app.ui.blatt_open && !app.ui.blatt_collapsed,
            "der Blattmanager der anderen Seite bleibt offen"
        );
    }

    /// T0233: Auch im Standardbetrieb dürfen **beide** Seiten je ein Panel
    /// zeigen — ein Panel auf der einen Seite schließt das der anderen nicht.
    #[test]
    fn standard_haelt_panels_beider_seiten_offen() {
        let mut app = app_mit_leisten();
        app.prefs.seitenleiste_immer = false;
        app.ui.layers_side = PanelSeite::Links;
        app.ui.blatt_side = PanelSeite::Rechts;
        app.ui.blatt_open = true;
        app.ui.layers_collapsed = false;
        app.ui.blatt_collapsed = false;
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());
        assert!(
            app.ui.layers_open && !app.ui.layers_collapsed,
            "das Layer-Panel links muss offen bleiben"
        );
        assert!(
            app.ui.blatt_open && !app.ui.blatt_collapsed,
            "der Blattmanager rechts muss offen bleiben"
        );
        // Beide Leisten weichen ihren Panels: nirgends ein Symbol zu sehen.
        assert_eq!(
            app.ui.last_leisten_icons.len(),
            0,
            "beide Seiten haben ein Panel — beide Leisten gehören ausgeblendet"
        );
    }

    /// Ein Mausradschritt um `punkte` (negativ = nach unten).
    fn rad(punkte: f32) -> egui::Event {
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, punkte),
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Rad über einer Stelle: erst den Zeiger dorthin, dann drehen.
    fn rad_fahrt(app: &mut App, pos: Pos2, punkte: f32) {
        bild(app, vec![egui::Event::PointerMoved(pos)]);
        for _ in 0..6 {
            bild(app, vec![rad(punkte)]);
        }
        bild(app, Vec::new());
    }

    /// T0222: Das Mausrad bewegt nur das Panel unter dem Zeiger.
    ///
    /// Beide Scrollflächen lagen vorher unter **derselben** Kennung
    /// (`Id::new("scroll_area")` auf der stabilen Ui-Kennung) — ein Radschritt
    /// über dem Layer-Panel zog den Blattmanager darunter mit. Geprüft wird an
    /// den Blattzeilen aus `UiState::last_blattzeilen`; sie wandern mit dem
    /// Scrollstand.
    #[test]
    fn scrollen_bewegt_nur_das_panel_unter_dem_zeiger() {
        let mut app = app_mit_leisten();
        app.ui.layers_side = PanelSeite::Links;
        app.ui.blatt_side = PanelSeite::Links;
        app.ui.layers_open = true;
        app.ui.blatt_open = true;
        app.ui.layers_collapsed = false;
        app.ui.blatt_collapsed = false;
        for i in 1..=30 {
            let _ = app.session_mut().execute(
                "sheet.new",
                &serde_json::json!({"name": format!("Blatt {i}")}),
            );
        }
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());

        let zeilen = |app: &App| -> Vec<f32> {
            app.ui
                .last_blattzeilen
                .iter()
                .map(|(_, r, _)| r.min.y)
                .collect()
        };
        let vorher = zeilen(&app);
        assert!(vorher.len() >= 30, "die Blattliste muss lang genug sein");

        // Untere Hälfte links = Blattmanager: das Rad scrollt ihn.
        rad_fahrt(&mut app, Pos2::new(60.0, 780.0), -300.0);
        let nach_blatt = zeilen(&app);
        assert_ne!(
            nach_blatt.first(),
            vorher.first(),
            "über dem Blattmanager muss das Rad die Liste bewegen"
        );

        // Obere Hälfte links = Layer-Panel: der Blattmanager bleibt stehen.
        rad_fahrt(&mut app, Pos2::new(60.0, 300.0), -300.0);
        assert_eq!(
            zeilen(&app),
            nach_blatt,
            "das Rad über dem Layer-Panel darf den Blattmanager nicht mitziehen"
        );
    }

    /// Das Layer-Panel wandert über das Symbol der **rechten** Leiste dorthin.
    #[test]
    fn klick_legt_das_layer_panel_auf_die_rechte_seite() {
        let mut app = app_mit_leisten();
        let r = symbol(&app, SeitenPanel::Layer, PanelSeite::Rechts);
        klick(&mut app, r.center());
        assert_eq!(app.ui.layers_side, PanelSeite::Rechts);
        assert!(!app.ui.layers_collapsed, "das Panel muss ausgeklappt sein");
        bild(&mut app, Vec::new());
        // Links steht nur noch die Leiste (28 px), das Panel sitzt rechts.
        assert!(
            app.ui.last_canvas.left() < 40.0,
            "links darf nur die Leiste stehen, Fläche beginnt bei {:.1}",
            app.ui.last_canvas.left()
        );
        assert!(
            app.ui.last_canvas.right() < 1500.0 - 200.0,
            "rechts muss das Panel stehen, Fläche endet bei {:.1}",
            app.ui.last_canvas.right()
        );
    }

    /// Und das Eigenschaften-Panel über das Symbol der **linken** Leiste.
    #[test]
    fn klick_legt_das_eigenschaften_panel_auf_die_linke_seite() {
        let mut app = app_mit_leisten();
        let r = symbol(&app, SeitenPanel::Eigenschaften, PanelSeite::Links);
        klick(&mut app, r.center());
        assert_eq!(app.ui.props_side, PanelSeite::Links);
        assert!(!app.ui.props_collapsed);
        bild(&mut app, Vec::new());
        assert!(
            app.ui.last_canvas.left() > 200.0,
            "links muss das Panel stehen, Fläche beginnt bei {:.1}",
            app.ui.last_canvas.left()
        );
    }

    /// Liegt das Panel schon auf dieser Seite und ist ausgeklappt, klappt der
    /// Klick es ein.
    #[test]
    fn klick_auf_das_eigene_symbol_klappt_das_panel_ein() {
        let mut app = app_mit_leisten();
        app.ui.layers_collapsed = false;
        bild(&mut app, Vec::new());
        let r = symbol(&app, SeitenPanel::Layer, PanelSeite::Links);
        klick(&mut app, r.center());
        assert!(app.ui.layers_collapsed, "der Klick muss das Panel einklappen");
    }

    /// Beide Panels auf einer Seite: sie teilen sich die Höhe, statt dass das
    /// erste dem zweiten alles wegnimmt.
    #[test]
    fn zwei_panels_auf_einer_seite_teilen_sich_die_hoehe() {
        let mut app = app_mit_leisten();
        app.ui.layers_collapsed = false;
        app.ui.props_collapsed = false;
        app.ui.props_side = PanelSeite::Links;
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());
        assert!(
            app.ui.last_canvas.left() > 200.0,
            "links müssen beide Panels stehen, Fläche beginnt bei {:.1}",
            app.ui.last_canvas.left()
        );
        // Rechts steht nur noch die Leiste (T0222: sie ist immer da, 28 px).
        assert!(
            app.ui.last_canvas.right() > 1500.0 - 40.0,
            "rechts darf nur die Leiste stehen, Fläche endet bei {:.1}",
            app.ui.last_canvas.right()
        );
    }

    /// T0218: Der Blattmanager klappt in die Leiste und kommt über sein
    /// Symbol zurück — wie Layer und Eigenschaften.
    #[test]
    fn blattmanager_klappt_in_die_leiste_und_zurueck() {
        let mut app = app_mit_leisten();
        app.ui.blatt_open = true;
        app.ui.blatt_collapsed = true;
        app.ui.blatt_side = PanelSeite::Rechts;
        bild(&mut app, Vec::new());
        bild(&mut app, Vec::new());
        assert!(
            app.ui
                .last_leisten_icons
                .iter()
                .any(|(p, _, _)| *p == SeitenPanel::Blatt),
            "das Blatt-Symbol fehlt in der Leiste"
        );
        assert!(
            app.ui.last_blattzeilen.is_empty(),
            "eingeklappt darf die Liste nicht gezeichnet werden"
        );

        let r = symbol(&app, SeitenPanel::Blatt, PanelSeite::Links);
        klick(&mut app, r.center());
        assert_eq!(app.ui.blatt_side, PanelSeite::Links);
        assert!(!app.ui.blatt_collapsed, "der Klick muss ausklappen");
        bild(&mut app, Vec::new());
        assert!(
            !app.ui.last_blattzeilen.is_empty(),
            "ausgeklappt muss die Blattliste stehen"
        );
    }
}

#[cfg(test)]
mod tests_block_kopieren {
    //! T0217: Das Rechtsklickmenü der Blockbibliothek kopiert einen Block in
    //! die Benutzerblöcke. Der Systemblock bleibt schreibgeschützt, die Kopie
    //! ist ein Benutzerblock und damit bearbeitbar.

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-block-kopie-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Zeichnung mit einem Systemblock (`sys:pfeil`) und Bibliothek im Testordner.
    fn app_mit_systemblock(dir: &std::path::Path) -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.blocks_pfad = dir.to_string_lossy().to_string();
        // `App::with_ctx` hängt die **echte** Benutzerbibliothek des Rechners
        // ein. Liegt dort schon eine „Pfeil (Kopie)“, vergäbe `block.copy`
        // „(Kopie 2)“ und der Test hinge an der Bibliothek des Nutzers
        // (gesehen: mitten im Merge-Lauf kopiert). Kopien dieses Namens
        // deshalb vorher aus der Zeichnung nehmen — der Test legt sie selbst an.
        app.doc_mut()
            .blocks
            .retain(|b| !b.name.starts_with("Pfeil (Kopie"));
        let mut e = Entity::new(
            1,
            app.doc().current_layer,
            EntityKind::Line {
                a: V2::new(0.0, 0.0),
                b: V2::new(3.0, 0.0),
            },
        );
        e.id = 1;
        app.doc_mut().blocks.push(BlockDef {
            id: "sys:pfeil".into(),
            name: "Pfeil".into(),
            category: "Punkt Symbole".into(),
            source: BlockSource::System,
            description: String::new(),
            entities: vec![e],
            icon: None,
            base: V2::ZERO,
            groesse: BlockGroesse::Papier,
        });
        app
    }

    #[test]
    fn kopie_eines_systemblocks_wird_benutzerblock() {
        let dir = test_dir("system");
        let mut app = app_mit_systemblock(&dir);
        let vorher: Vec<String> = app.doc().blocks.iter().map(|b| b.id.clone()).collect();
        assert!(app.block_kopieren("sys:pfeil"), "Kopie muss gelingen");

        // Die Zeichnung bringt eigene Blöcke mit (Vorlagen) — die Kopie ist der
        // Block, der vorher nicht da war.
        let kopie = app
            .doc()
            .blocks
            .iter()
            .find(|b| !vorher.contains(&b.id))
            .expect("Kopie fehlt in der Bibliothek")
            .clone();
        assert!(kopie.id.starts_with("usr:"));
        assert_eq!(kopie.name, "Pfeil (Kopie)");
        assert_eq!(kopie.entities.len(), 1, "Geometrie kommt mit");
        assert!(
            dir.join("pfeil-kopie.jjblk").is_file(),
            "die Kopie liegt als Blockdatei im Verzeichnis"
        );
        // Das Original bleibt unberührt.
        assert!(app
            .doc()
            .blocks
            .iter()
            .any(|b| b.id == "sys:pfeil" && b.source == BlockSource::System));

        // Der Reiter springt auf die Benutzerblöcke, damit die Kopie zu sehen ist.
        assert_eq!(app.ui.inventory_tab, 1);
        assert_eq!(app.ui.block_quelle, 1);

        // Die Kopie ist bearbeitbar, der Systemblock weiterhin nicht.
        assert!(app.oeffne_block_eigenschaften(&kopie.id));
        assert!(!app.oeffne_block_eigenschaften("sys:pfeil"));
    }

    /// Zweite Kopie überschreibt die erste weder im Namen noch in der Datei.
    #[test]
    fn zweite_kopie_bekommt_einen_eigenen_namen() {
        let dir = test_dir("zweite");
        let mut app = app_mit_systemblock(&dir);
        assert!(app.block_kopieren("sys:pfeil"));
        assert!(app.block_kopieren("sys:pfeil"));
        let namen: Vec<&str> = app
            .doc()
            .blocks
            .iter()
            .filter(|b| b.name.starts_with("Pfeil (Kopie"))
            .map(|b| b.name.as_str())
            .collect();
        assert_eq!(namen, vec!["Pfeil (Kopie)", "Pfeil (Kopie 2)"]);
        assert!(dir.join("pfeil-kopie.jjblk").is_file());
        assert!(dir.join("pfeil-kopie-2.jjblk").is_file());
    }
}

mod tests_block_eigenschaften {
    //! T0204: Name und Gruppe eines Blocks — Fenster aus der Blockbibliothek
    //! (Rechtsklick ▸ „Eigenschaften …“), geschrieben über `block.update`.

    use super::*;
    use jjcad_core::model::BlockSource;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-blockeigenschaften-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Zeichnung mit einem Benutzerblock (`usr:stuhl`), Bibliothek im Testordner.
    fn app_mit_block(dir: &std::path::Path) -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.blocks_pfad = dir.to_string_lossy().to_string();
        app.doc_mut().blocks.push(BlockDef {
            id: "usr:stuhl".into(),
            name: "Stuhl".into(),
            category: "Projekt".into(),
            source: BlockSource::User,
            description: String::new(),
            entities: Vec::new(),
            icon: None,
            base: V2::ZERO,
            groesse: BlockGroesse::Fest,
        });
        app
    }

    #[test]
    fn fenster_zeigt_name_und_gruppe_des_blocks() {
        let dir = test_dir("oeffnen");
        let mut app = app_mit_block(&dir);
        assert!(app.oeffne_block_eigenschaften("usr:stuhl"));
        let wahl = app
            .ui
            .block_eigenschaften
            .clone()
            .expect("Fenster „Block-Eigenschaften“ fehlt");
        assert_eq!(wahl.block, "usr:stuhl");
        assert_eq!(wahl.name, "Stuhl");
        assert_eq!(wahl.gruppe, "Projekt");
    }

    #[test]
    fn systemblock_bekommt_kein_fenster() {
        let dir = test_dir("system");
        let mut app = app_mit_block(&dir);
        app.doc_mut().blocks.push(BlockDef {
            id: "sys:tuer".into(),
            name: "Tür".into(),
            category: "Architektur".into(),
            source: BlockSource::System,
            description: String::new(),
            entities: Vec::new(),
            icon: None,
            base: V2::ZERO,
            groesse: BlockGroesse::Fest,
        });
        assert!(
            !app.oeffne_block_eigenschaften("sys:tuer"),
            "Systemblöcke sind schreibgeschützt"
        );
        assert!(app.ui.block_eigenschaften.is_none());
    }

    #[test]
    fn uebernehmen_schreibt_name_und_gruppe_in_datei_und_dokument() {
        let dir = test_dir("uebernehmen");
        let mut app = app_mit_block(&dir);
        // Ausgangsstand als Datei — so ist das Umbenennen an der Datei prüfbar.
        let start = app.doc().block("usr:stuhl").cloned().expect("Block fehlt");
        jjcad_core::bloecke::schreibe_block(&app.prefs.blocks_dir(), &start).unwrap();
        assert!(app.prefs.blocks_dir().join("stuhl.jjblk").is_file());

        assert!(app.oeffne_block_eigenschaften("usr:stuhl"));
        if let Some(wahl) = app.ui.block_eigenschaften.as_mut() {
            wahl.name = "Stuhl rund".into();
            wahl.gruppe = "Möbel".into();
        }
        assert!(app.block_eigenschaften_uebernehmen());

        let block = app.doc().block("usr:stuhl").expect("Block fehlt");
        assert_eq!(block.name, "Stuhl rund");
        assert_eq!(block.category, "Möbel");
        assert!(
            app.ui.block_eigenschaften.is_none(),
            "das Fenster bleibt nach dem Übernehmen offen"
        );

        let ordner = app.prefs.blocks_dir();
        assert!(
            !ordner.join("stuhl.jjblk").exists(),
            "die alte Blockdatei muss weg sein"
        );
        let datei = ordner.join("stuhl-rund.jjblk");
        assert!(datei.is_file(), "neue Blockdatei fehlt: {}", datei.display());
        let roh: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&datei).unwrap()).unwrap();
        assert_eq!(roh["block"]["name"], "Stuhl rund");
        assert_eq!(roh["block"]["category"], "Möbel");
        assert_eq!(roh["block"]["id"], "usr:stuhl", "Kennung bleibt stabil");
    }

    /// T0231: Die **Größe** (feste Größe / Papiergröße) gehört zu den
    /// Block-Eigenschaften — sie schreibt `block.update` und landet in der
    /// `.jjblk`.
    #[test]
    fn groesse_wird_uebernommen() {
        let dir = test_dir("groesse");
        let mut app = app_mit_block(&dir);
        assert_eq!(
            app.doc().block("usr:stuhl").unwrap().groesse,
            jjcad_core::model::BlockGroesse::Fest
        );

        assert!(app.oeffne_block_eigenschaften("usr:stuhl"));
        if let Some(wahl) = app.ui.block_eigenschaften.as_mut() {
            wahl.groesse = jjcad_core::model::BlockGroesse::Papier;
        }
        assert!(app.block_eigenschaften_uebernehmen());
        assert_eq!(
            app.doc().block("usr:stuhl").unwrap().groesse,
            jjcad_core::model::BlockGroesse::Papier,
            "die Größe steht nicht im Dokument"
        );
        // Benutzerblöcke schreiben ihre `.jjblk` mit — dort muss sie auch stehen.
        let datei = app.prefs.blocks_dir().join("stuhl.jjblk");
        let roh: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&datei).unwrap()).unwrap();
        assert_eq!(roh["block"]["groesse"], serde_json::json!("Papier"));

        // Und wieder zurück: die Größe ist keine Einbahnstraße.
        assert!(app.oeffne_block_eigenschaften("usr:stuhl"));
        if let Some(wahl) = app.ui.block_eigenschaften.as_mut() {
            wahl.groesse = jjcad_core::model::BlockGroesse::Fest;
        }
        assert!(app.block_eigenschaften_uebernehmen());
        assert_eq!(
            app.doc().block("usr:stuhl").unwrap().groesse,
            jjcad_core::model::BlockGroesse::Fest
        );
    }

    /// T0231: Im Blockeditor eines Blocks in **Papiergröße** rechnet die
    /// Oberfläche in Millimetern — in Metern stünde jede Länge auf 0,00.
    #[test]
    fn papiergroesse_rechnet_im_blockeditor_in_millimetern() {
        let dir = test_dir("papier-mm");
        let mut app = app_mit_block(&dir);
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::M;
        assert_eq!(app.unit(), jjcad_core::units::Unit::M, "Modell in Metern");

        app.doc_mut().block_mut("usr:stuhl").unwrap().groesse =
            jjcad_core::model::BlockGroesse::Papier;
        assert!(app.oeffne_block_editor("usr:stuhl"));
        assert_eq!(
            app.unit(),
            jjcad_core::units::Unit::Mm,
            "ein Block in Papiergröße wird in Papier-Millimetern beschriftet"
        );

        // Ein Block fester Größe bleibt bei der Zeichnungseinheit.
        app.doc_mut().block_mut("usr:stuhl").unwrap().groesse =
            jjcad_core::model::BlockGroesse::Fest;
        assert_eq!(app.unit(), jjcad_core::units::Unit::M);
    }

    /// T0228: Modell und Papierraum tragen **eigene** Eingabeeinheiten — das
    /// Modell in Metern, das Papier in Millimetern (Vorgabe). Eine Wahl im
    /// Papierraum ändert das Modell nicht.
    #[test]
    fn papierbereich_hat_eigene_eingabeeinheit() {
        let mut app = App::with_ctx(egui::Context::default());
        app.test_prefs_speichern = false;
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::M;
        assert_eq!(app.unit(), jjcad_core::units::Unit::M, "Modell in Metern");

        let sid = app.doc().sheets[0].id;
        app.blatt_oeffnen(sid);
        assert!(app.doc().in_paper_space());
        assert_eq!(
            app.unit(),
            jjcad_core::units::Unit::Mm,
            "Papier in Millimetern"
        );

        let _ = app
            .session_mut()
            .execute("doc.settings", &serde_json::json!({"paper_input_unit": "cm"}));
        assert_eq!(app.unit(), jjcad_core::units::Unit::Cm);
        assert_eq!(
            app.doc().settings.input_unit,
            jjcad_core::units::Unit::M,
            "das Modell behält Meter"
        );

        app.switch_tab(TabKind::Model);
        assert_eq!(
            app.unit(),
            jjcad_core::units::Unit::M,
            "zurück im Modell gilt wieder Meter"
        );
    }

    /// Eine neue Gruppe entsteht, indem der Block sie bekommt — das Feld
    /// „Neue Gruppe“ sticht die Auswahl.
    #[test]
    fn neue_gruppe_aus_dem_feld_sticht_die_auswahl() {
        let dir = test_dir("neue-gruppe");
        let mut app = app_mit_block(&dir);
        assert!(app.oeffne_block_eigenschaften("usr:stuhl"));
        if let Some(wahl) = app.ui.block_eigenschaften.as_mut() {
            wahl.gruppe = "Projekt".into();
            wahl.neue_gruppe = "Sitzmöbel".into();
        }
        assert!(app.block_eigenschaften_uebernehmen());
        assert_eq!(app.doc().block("usr:stuhl").unwrap().category, "Sitzmöbel");
    }

    #[test]
    fn leerer_name_aendert_nichts_und_laesst_das_fenster_stehen() {
        let dir = test_dir("leer");
        let mut app = app_mit_block(&dir);
        assert!(app.oeffne_block_eigenschaften("usr:stuhl"));
        if let Some(wahl) = app.ui.block_eigenschaften.as_mut() {
            wahl.name = "   ".into();
            wahl.gruppe = "Möbel".into();
        }
        assert!(!app.block_eigenschaften_uebernehmen());
        assert_eq!(app.doc().block("usr:stuhl").unwrap().name, "Stuhl");
        assert_eq!(app.doc().block("usr:stuhl").unwrap().category, "Projekt");
        assert!(
            app.ui.block_eigenschaften.is_some(),
            "das Fenster muss zum Nachbessern offen bleiben"
        );
    }
}

#[cfg(test)]
mod tests_zwischenablage_wand {
    //! T0207: Kopieren und Einfügen einer Wand nimmt ihre Türen und Fenster mit
    //! — und die Kopie der Öffnung hängt an der **kopierten** Wand.
    //!
    //! Beide Wege sind betroffen: `App::merke_zwischenablage` (was kommt in die
    //! Zwischenablage) und `App::commit_ghost` (welche Wand-Id steht danach im
    //! `host` der eingefügten Tür).

    use super::*;

    fn app_mit_wand_und_tuer() -> (App, Id, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.test_prefs_speichern = false;
        let wand = app
            .session_mut()
            .execute(
                "draw.wall",
                &serde_json::json!({
                    "a": [0.0, 0.0], "b": [5000.0, 0.0],
                    "thickness": 240.0, "align": "center"
                }),
            )
            .expect("Wand")["created"][0]
            .as_u64()
            .unwrap();
        let tuer = app
            .session_mut()
            .execute(
                "draw.door",
                &serde_json::json!({
                    "host": wand, "at": [2500.0, 0.0], "width": 1000.0, "swing_in": true
                }),
            )
            .expect("Tür")["created"][0]
            .as_u64()
            .unwrap();
        (app, wand, tuer)
    }

    /// Die Zwischenablage enthält die Tür, wenn die Wand kopiert wird.
    #[test]
    fn kopieren_der_wand_legt_die_tuer_mit_in_die_zwischenablage() {
        let (mut app, wand, tuer) = app_mit_wand_und_tuer();
        app.merke_zwischenablage(&[wand], None);
        let ids: Vec<Id> = app.ui.clipboard.ents.iter().map(|e| e.id).collect();
        assert!(ids.contains(&wand), "Wand fehlt: {ids:?}");
        assert!(ids.contains(&tuer), "Tür fehlt in der Zwischenablage: {ids:?}");
    }

    /// Eingefügt: neue Wand, neue Tür — und die Tür hängt an der neuen Wand.
    #[test]
    fn einfuegen_haengt_die_tuer_an_die_eingefuegte_wand() {
        let (mut app, wand, tuer) = app_mit_wand_und_tuer();
        app.merke_zwischenablage(&[wand], None);
        app.pointer_world = Some(V2::new(0.0, 6000.0));
        app.start_ghost_paste();
        assert!(app.commit_ghost(), "Einfügen muss etwas anlegen");

        let waende: Vec<Id> = app
            .doc()
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Wall { .. }))
            .map(|e| e.id)
            .collect();
        let tueren: Vec<Id> = app
            .doc()
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Door { .. }))
            .map(|e| e.id)
            .collect();
        assert_eq!(waende.len(), 2, "eine Wand und ihre Kopie: {waende:?}");
        assert_eq!(tueren.len(), 2, "eine Tür und ihre Kopie: {tueren:?}");

        let neue_wand = *waende.iter().find(|id| **id != wand).unwrap();
        let neue_tuer = *tueren.iter().find(|id| **id != tuer).unwrap();
        let EntityKind::Door { host, center, .. } = &app.doc().entity(neue_tuer).unwrap().kind
        else {
            panic!("Tür erwartet");
        };
        assert_eq!(
            *host,
            Some(neue_wand),
            "die eingefügte Tür muss an der eingefügten Wand hängen, nicht am Original"
        );
        // Die Tür sitzt auf der vorderen Kante **dieser** Wand (Dicke/2 über
        // der Achse) — sie ist mit ihr gewandert.
        let EntityKind::Wall { a, b, thickness, .. } = &app.doc().entity(neue_wand).unwrap().kind
        else {
            panic!("Wand erwartet");
        };
        assert!(
            (center.y - (a.y + thickness * 0.5)).abs() <= 0.5
                && (center.y - (b.y + thickness * 0.5)).abs() <= 0.5,
            "die Tür muss auf der eingefügten Wand sitzen: Tür {center:?}, Wand {a:?}→{b:?}"
        );
        // Das Original bleibt, wo es war.
        let EntityKind::Door { host: alt, center: alt_c, .. } =
            &app.doc().entity(tuer).unwrap().kind
        else {
            panic!("Tür erwartet");
        };
        assert_eq!(*alt, Some(wand));
        assert!((alt_c.y - 120.0).abs() <= 0.5, "Original-Tür bleibt: {alt_c:?}");
    }
}

mod tests_blattmanager {
    //! T0218: Der Blattmanager listet **alle** Blätter. Ein Reiterkreuz
    //! schließt nur den Reiter — das Blatt bleibt in der Zeichnung und kommt
    //! per Klick in der Liste zurück. Das Häkchen nimmt es in den Plansatz.

    use super::*;

    fn bild(app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    fn knopf(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn klick(app: &mut App, pos: Pos2) {
        for events in [
            vec![egui::Event::PointerMoved(pos)],
            vec![knopf(pos, true)],
            vec![knopf(pos, false)],
            vec![],
        ] {
            let _ = bild(app, events);
        }
    }

    fn texte(out: &egui::FullOutput) -> Vec<String> {
        fn sammeln(s: &egui::Shape, ziel: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => ziel.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => {
                    for s in v {
                        sammeln(s, ziel);
                    }
                }
                _ => {}
            }
        }
        let mut ziel = Vec::new();
        for cs in &out.shapes {
            sammeln(&cs.shape, &mut ziel);
        }
        ziel
    }

    /// Zwei Blätter, das zweite ist der aktive Reiter.
    fn app_mit_zwei_blaettern() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        let res = app
            .session_mut()
            .execute("sheet.new", &serde_json::json!({"name": "Blatt 2"}))
            .expect("sheet.new");
        let id = res["sheet"].as_u64().expect("Blattkennung");
        app.switch_tab(TabKind::Sheet(id));
        app.ui.blatt_open = true;
        app.ui.layers_open = false;
        app.ui.props_open = false;
        (app, id)
    }

    /// Das Kreuz schließt den Reiter, nicht das Blatt — und der Klick in der
    /// Liste holt den Reiter zurück.
    #[test]
    fn reiter_schliessen_loescht_kein_blatt() {
        let (mut app, id) = app_mit_zwei_blaettern();
        let vorher = app.doc().sheets.len();
        app.blatt_tab_schliessen(id);
        assert_eq!(
            app.doc().sheets.len(),
            vorher,
            "das Blatt muss in der Zeichnung bleiben"
        );
        assert!(!app.blatt_tab_offen(id), "der Reiter ist zu");
        assert_eq!(app.tab().tab, TabKind::Model, "der aktive Reiter wechselt");

        app.blatt_oeffnen(id);
        assert!(app.blatt_tab_offen(id), "der Reiter ist wieder da");
        assert_eq!(app.tab().tab, TabKind::Sheet(id));
    }

    /// Ein geschlossener Reiter steht weiter in der Liste — genau das ist der
    /// Sinn des Blattmanagers.
    #[test]
    fn geschlossener_reiter_bleibt_in_der_liste() {
        let (mut app, id) = app_mit_zwei_blaettern();
        app.blatt_tab_schliessen(id);
        let _ = bild(&mut app, Vec::new());
        let out = bild(&mut app, Vec::new());
        let texte = texte(&out);
        assert!(
            texte.iter().any(|t| t.contains("Blattmanager")),
            "die Panelüberschrift fehlt: {texte:?}"
        );
        assert!(
            texte.iter().filter(|t| *t == "Blatt 2").count() == 1,
            "das geschlossene Blatt fehlt in der Liste: {texte:?}"
        );
        assert!(
            app.ui.last_blattzeilen.iter().any(|(b, _, _)| *b == id),
            "für das geschlossene Blatt fehlt die Zeile"
        );
    }

    /// Klick auf die Zeile öffnet das Blatt und den Reiter.
    #[test]
    fn klick_in_der_liste_oeffnet_den_reiter() {
        let (mut app, id) = app_mit_zwei_blaettern();
        app.blatt_tab_schliessen(id);
        let _ = bild(&mut app, Vec::new());
        let _ = bild(&mut app, Vec::new());
        let zeile = app
            .ui
            .last_blattzeilen
            .iter()
            .find(|(b, _, _)| *b == id)
            .map(|(_, text, _)| *text)
            .expect("Blattzeile");
        klick(&mut app, zeile.center());
        assert_eq!(app.tab().tab, TabKind::Sheet(id));
        assert!(app.blatt_tab_offen(id));
    }

    /// Das Häkchen setzt die Zugehörigkeit zum Plansatz — über dieselbe
    /// Action, die auch der Befehlskanal kennt.
    #[test]
    fn haekchen_setzt_den_plansatz() {
        let (mut app, id) = app_mit_zwei_blaettern();
        let _ = bild(&mut app, Vec::new());
        let _ = bild(&mut app, Vec::new());
        let haken = app
            .ui
            .last_blattzeilen
            .iter()
            .find(|(b, _, _)| *b == id)
            .map(|(_, _, cb)| *cb)
            .expect("Häkchenfläche");
        klick(&mut app, haken.center());
        assert!(
            app.doc().sheet(id).expect("Blatt").plansatz,
            "das Häkchen hat den Plansatz nicht gesetzt"
        );
        assert_eq!(app.plansatz_ids(), vec![id]);
    }
}

/// T0224: Ein Auftrag von außen (Zeichnung geöffnet, Fenster zeigen) holt
/// JoJa-CAD in den Vordergrund.
///
/// Vorher schickte nur der Auftrag „zeigen“ einen Fokuswunsch; beim Doppelklick
/// im Dateibrowser wurde die Zeichnung zwar geöffnet, das Fenster blieb aber
/// hinter dem Browser. Geprüft wird der Weg **bis zum Fensterbefehl**: der
/// Fokuswunsch steht in der Ausgabe des Bildes, in dem der Auftrag ankommt.
#[cfg(test)]
mod tests_nach_vorn {
    use super::*;

    /// Führt ein Bild aus und liefert die Fensterbefehle des Hauptfensters.
    fn befehle(
        app: &mut App,
        zeit: f64,
        fokus: bool,
        mut tun: impl FnMut(&mut App, &egui::Context),
    ) -> Vec<egui::ViewportCommand> {
        let ctx = app.ctx.clone();
        let raw = egui::RawInput {
            time: Some(zeit),
            focused: fokus,
            ..Default::default()
        };
        let out = ctx.run(raw, |c| tun(app, c));
        out.viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.commands.clone())
            .unwrap_or_default()
    }

    fn fordert_fokus(befehle: &[egui::ViewportCommand]) -> bool {
        befehle
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::Focus))
    }

    fn meldet_aufmerksamkeit(befehle: &[egui::ViewportCommand]) -> bool {
        befehle.iter().any(|c| {
            matches!(
                c,
                egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Critical
                )
            )
        })
    }

    /// App mit eigenem Sicherungsordner — ein Testlauf fasst den Pool des
    /// Nutzers nicht an.
    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.reset_bindings();
        app.prefs.sicherungen_pfad = std::env::temp_dir()
            .join(format!("jjcad-t0224-{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        app
    }

    /// Eine gespeicherte Zeichnung im Temp-Verzeichnis.
    fn datei(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jjcad-t0224-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pfad = dir.join(name);
        let mut doc = jjcad_core::model::Document::new();
        jjcad_core::persist::save_document(&mut doc, &pfad).expect("speichern");
        pfad
    }

    #[test]
    fn ohne_auftrag_kein_fokuswunsch() {
        // Gegenprobe: das Bild allein fordert keinen Fokus an.
        let mut app = app();
        let befehle = befehle(&mut app, 1.0, true, |app, ctx| {
            app.auftraege_bearbeiten(ctx, Vec::new(), 1.0)
        });
        assert!(!fordert_fokus(&befehle), "ohne Auftrag kein Fokuswunsch");
    }

    #[test]
    fn oeffnen_holt_das_fenster_nach_vorn() {
        let pfad = datei("Aussen.jjcad");
        let mut app = app();
        let vorher = app.tabs.len();
        let befehle = befehle(&mut app, 2.0, false, |app, ctx| {
            app.auftraege_bearbeiten(
                ctx,
                vec![jjcad_core::instanz::Auftrag::Oeffnen {
                    pfad: pfad.clone(),
                    token: Some("kwin-1".into()),
                }],
                2.0,
            )
        });
        assert!(
            fordert_fokus(&befehle),
            "der Auftrag „öffnen“ muss den Fokus anfordern: {befehle:?}"
        );
        assert!(
            befehle
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Minimized(false))),
            "ein minimiertes Fenster muss zurückkommen: {befehle:?}"
        );
        // Der leere Startreiter wird ersetzt, ein zweiter Reiter entsteht nur
        // bei vorhandener Arbeit — hier bleibt es bei einem Reiter, der die
        // übergebene Datei trägt.
        assert_eq!(app.tabs.len(), vorher, "die Zeichnung öffnet im Reiter");
        assert_eq!(
            app.tabs[app.active].session.doc.path.as_deref(),
            Some(pfad.to_string_lossy().as_ref()),
            "der Auftrag hat die übergebene Zeichnung geöffnet"
        );
        assert!(app.nach_vorn.is_some(), "der Wunsch wird nachgehalten");
    }

    #[test]
    fn zeigen_holt_das_fenster_nach_vorn() {
        let mut app = app();
        let befehle = befehle(&mut app, 2.0, false, |app, ctx| {
            app.auftraege_bearbeiten(
                ctx,
                vec![jjcad_core::instanz::Auftrag::Zeigen { token: None }],
                2.0,
            )
        });
        assert!(fordert_fokus(&befehle), "Fokuswunsch fehlt: {befehle:?}");
        assert_eq!(app.tabs.len(), 1, "„zeigen“ öffnet keine Zeichnung");
    }

    #[test]
    fn wunsch_endet_mit_dem_fokus() {
        let mut app = app();
        app.nach_vorn = Some(NachVorn {
            seit: 1.0,
            gemeldet: false,
            token: None,
            nativ: true,
        });
        let befehle = befehle(&mut app, 1.1, true, |app, ctx| {
            let _ = app.nach_vorn_betreuen(ctx, 1.1);
        });
        assert!(app.nach_vorn.is_none(), "mit Fokus ist der Wunsch erledigt");
        assert!(!fordert_fokus(&befehle), "erledigt wird nicht mehr gefordert");
    }

    #[test]
    fn wunsch_wird_bis_zur_frist_nachgehalten() {
        let mut app = app();
        app.nach_vorn = Some(NachVorn {
            seit: 1.0,
            gemeldet: false,
            token: None,
            nativ: true,
        });
        // Noch kein Fokus, aber innerhalb der Frist: der Wunsch wird wiederholt.
        let befehle = befehle(&mut app, 1.2, false, |app, ctx| {
            let _ = app.nach_vorn_betreuen(ctx, 1.2);
        });
        assert!(fordert_fokus(&befehle), "der Wunsch wird wiederholt");
        assert!(
            app.nach_vorn.is_some(),
            "vor Ablauf der Frist bleibt der Wunsch offen"
        );
    }

    #[test]
    fn wunsch_gibt_nach_der_frist_auf() {
        let mut app = app();
        app.nach_vorn = Some(NachVorn {
            seit: 1.0,
            gemeldet: false,
            token: None,
            nativ: true,
        });
        let befehle = befehle(&mut app, 1.0 + NACH_VORN_DAUER + 0.1, false, |app, ctx| {
            let _ = app.nach_vorn_betreuen(ctx, 1.0 + NACH_VORN_DAUER + 0.1);
        });
        assert!(
            app.nach_vorn.is_none(),
            "nach der Frist bedrängt die Anwendung nicht weiter"
        );
        assert!(!fordert_fokus(&befehle), "kein Fokuswunsch mehr");
    }

    /// T0224: Das Aktivierungstoken des Aufrufers wird **einmal** vorgelegt —
    /// ein Token gilt beim Compositor nur für einen Vorgang.
    #[test]
    fn token_wird_einmal_vorgelegt() {
        let mut app = app();
        app.nach_vorn = Some(NachVorn {
            seit: 1.0,
            gemeldet: false,
            token: Some("kwin-9".into()),
            nativ: false,
        });
        let mut erstes = None;
        let _ = befehle(&mut app, 1.05, false, |app, ctx| {
            erstes = app.nach_vorn_betreuen(ctx, 1.05);
        });
        let mut zweites = None;
        let _ = befehle(&mut app, 1.10, false, |app, ctx| {
            zweites = app.nach_vorn_betreuen(ctx, 1.10);
        });
        assert_eq!(erstes.as_deref(), Some("kwin-9"), "das Token kommt einmal");
        assert_eq!(zweites, None, "ein Token darf nicht zweimal vorgelegt werden");
    }

    /// Der Weg für Wayland: dort ist `Focus` wirkungslos, `RequestUserAttention`
    /// legt dem Compositor ein `xdg_activation`-Anliegen vor — genau einmal.
    #[test]
    fn ohne_fokus_meldet_er_einmal_aufmerksamkeit() {
        let mut app = app();
        app.nach_vorn = Some(NachVorn {
            seit: 1.0,
            gemeldet: false,
            token: None,
            nativ: true,
        });
        let spaet = 1.0 + NACH_VORN_MELDEN + 0.05;
        let erstes = befehle(&mut app, spaet, false, |app, ctx| {
            let _ = app.nach_vorn_betreuen(ctx, spaet);
        });
        assert!(
            meldet_aufmerksamkeit(&erstes),
            "ohne Fokus muss Aufmerksamkeit gemeldet werden: {erstes:?}"
        );
        assert!(
            app.nach_vorn.as_ref().map(|n| n.gemeldet).unwrap_or(false),
            "die Meldung ist vermerkt"
        );
        // Zweites Bild: nicht noch einmal.
        let zweites = befehle(&mut app, spaet + 0.1, false, |app, ctx| {
            let _ = app.nach_vorn_betreuen(ctx, spaet + 0.1);
        });
        assert!(
            !meldet_aufmerksamkeit(&zweites),
            "die Meldung wird nicht wiederholt: {zweites:?}"
        );
    }
}
