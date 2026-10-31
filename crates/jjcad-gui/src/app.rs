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
use jjcad_core::persist::{self, DragMode, UserPrefs};
use jjcad_core::render::DisplayList;
use jjcad_core::session::Session;
use jjcad_core::style::{Color, FillPattern};
use jjcad_core::units::{format_number, Unit};
use jjcad_core::{dxf, pdf, raster};
use std::collections::HashMap;
use std::path::PathBuf;

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
/// Höhe der einen unteren Leiste: Slot-Kante (50 px) + 2 × 5 px Rand
pub const HOTBAR_H: f32 = 60.0;

/// Weg eines einzelnen Schwenk-Schritts in Bildpunkten (T0056): ein Tastendruck
/// rückt die Ansicht sichtbar weiter, ohne sie springen zu lassen.
pub const SCHWENK_SCHRITT_PX: f32 = 60.0;
/// Grundtempo des Schwenks mit gehaltener Taste in Bildpunkten je Sekunde
/// (Vorgabe, T0067). Die Einstellung „Schwenk“ skaliert es über
/// `UserPrefs::pan_speed`.
pub const SCHWENK_PX_JE_SEKUNDE: f32 = 700.0;

/// Die Tasten, deren Funktion aus der **Tastaturbelegung** kommt — Buchstaben
/// und Symbole (T0056, `docs/TASTEN.md`). Ziffern gehören der Hotbar,
/// Steuerungstasten und Kombinationen dem festen Ablauf im Code; keine dieser
/// Tasten steht deshalb in dieser Liste.
///
/// **Eine** Quelle für Kürzel-Auswertung (`handle_shortcuts`) und
/// Ansichtsschwenk (`canvas::handle_navigation`) — sonst legt man eine Taste in
/// der Belegung um und die eine Hälfte des Programms hört nicht darauf.
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabKind {
    Model,
    Sheet(Id),
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
}

impl DrawCacheKey {
    /// Gleiche Zeichnung, gleiche Ansicht, gleicher Raum?
    pub fn matches(&self, other: &Self) -> bool {
        self.rev == other.rev
            && (self.zoom - other.zoom).abs() < 1e-9
            && (self.annot - other.annot).abs() < 1e-9
            && self.space == other.space
    }
}

pub struct DocTab {
    pub session: Session,
    pub tab: TabKind,
    pub view_model: View,
    pub sheet_views: HashMap<Id, View>,
    pub rev: u64,
    pub cache: Option<(DrawCacheKey, DisplayList)>,
    pub hover: Option<Id>,
    pub selected_viewport: Option<Id>,
}

#[derive(Clone)]
pub struct Tool {
    pub action: String,
    pub pts: Vec<V2>,
    pub selecting: bool,
    pub phase: ToolPhase,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolPhase {
    Collecting,
    TextInput,
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
            (Einfuegemodus::Achsenparallel, "draw.ellipse") => "Über Mittelpunkt",
            (Einfuegemodus::Achsenparallel, _) => "Über Ecken",
            (Einfuegemodus::Basislinie, "draw.ellipse") => "Mittelachse + Breite",
            (Einfuegemodus::Basislinie, _) => "Basislinie + Höhe",
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
    pub dim_axis: Option<DimAxis>,
    /// Eigener Maßtext des nächsten Maßes (leer = automatisch berechnet).
    pub dim_text: String,
    pub offset_dist: f64,
    pub elevation_z: f64,
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
            dim_axis: None,
            dim_text: String::new(),
            offset_dist: doc.settings.wall_thickness,
            elevation_z: 0.0,
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
pub const RIBBON_BEARBEITEN: [(&str, &str, &str); 5] = [
    ("modify.move", "move", "Verschieben"),
    // Kopieren ist eine geteilte Schaltfläche (siehe `copy_group_button`)
    ("modify.copy", "copy-base", "Kopieren mit Basispunkt"),
    ("modify.rotate", "rotate", "Drehen"),
    ("modify.mirror", "mirror", "Spiegeln"),
    ("modify.scale", "scale", "Skalieren"),
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
    pub fn label(self) -> &'static str {
        match self {
            RibbonTab::Datei => "Start",
            RibbonTab::Zeichnen => "Zeichnen",
            RibbonTab::Architektur => "Architektur",
            RibbonTab::Aendern => "Ändern",
            RibbonTab::Bemassung => "Bemaßung",
            RibbonTab::Bloecke => "Blöcke",
            RibbonTab::Blatt => "Blatt",
            RibbonTab::Ansicht => "Ansicht",
        }
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
    pub lineweight: Option<f64>,
    pub line_type: Option<jjcad_core::style::LineType>,
}

impl Default for PendingStyle {
    fn default() -> Self {
        // Mittel (0,35 mm) ist die Vorgabe für neue Elemente
        Self {
            color: None,
            fill: None,
            lineweight: Some(0.35),
            line_type: None,
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

pub struct UiState {
    pub pending: PendingStyle,
    pub ribbon: RibbonTab,
    pub inventory_open: bool,
    pub inventory_tab: usize,
    pub layers_open: bool,
    pub props_open: bool,
    /// Panels und Ribbon eingeklappt
    pub layers_collapsed: bool,
    pub props_collapsed: bool,
    pub ribbon_collapsed: bool,
    pub keymap_open: bool,
    pub settings_open: bool,
    /// Gewählte Kategorie im Einstellungsfenster (Index in
    /// `dialogs::EINSTELLUNGSKATEGORIEN`)
    pub settings_kat: usize,
    /// Suchtext über der Kategorienliste im Einstellungsfenster
    pub settings_suche: String,
    /// Lage von Suchfeld, Kategorienliste und Einstellungsseite im zuletzt
    /// gezeichneten Einstellungsfenster (Regressionstest T0061)
    pub last_settings_suche: Option<Rect>,
    pub last_settings_nav: Option<Rect>,
    pub last_settings_pane: Option<Rect>,
    pub help_open: bool,
    pub about_open: bool,
    pub sheet_dialog: bool,
    pub project_dialog: bool,
    /// Laufende Texteingabe in der Zeichnung (T0019)
    pub texteingabe: Option<crate::textedit::Texteingabe>,
    pub block_name_dialog: Option<(String, bool)>,
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
    /// Zieh-Modus für Objekte und Griffe
    pub drag_mode: DragMode,
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
    pub slot_drag: Option<(usize, String)>,
    pub keymap_selected: Option<String>,
    /// Zuletzt gewählte Funktion für die Tastaturbelegung (T0064): Solange das
    /// Fenster offen ist, merkt es sich, was im Ribbon (oder über Hotbar und
    /// Taste) gestartet wurde — ein Klick auf eine leere Taste legt sie dort
    /// ab, genau wie die Hotbar es macht. Beim Schließen wird der Merker
    /// geleert, damit keine alte Funktion unbemerkt auf einer Taste landet.
    pub keymap_funktion: Option<String>,
    pub search: String,
    pub new_layer_dialog: Option<String>,
    pub color_popup: Option<Id>,
    pub rename_layer: Option<(Id, String)>,
    /// Zwischenablage: Elemente **und** Basispunkt (siehe `ghost::Clipboard`)
    pub clipboard: crate::ghost::Clipboard,
    pub clipboard_offset: f64,
    pub confirm_new: bool,
    pub confirm_quit: bool,
    pub last_canvas: Rect,
    /// Höhe des zuletzt gezeichneten Ribbon-Inhalts (Regressionstest t_92b03440)
    pub last_ribbon_content_h: f32,
    /// Lage der Hotbar-Slots in der unteren Leiste (Regressionstest T0043:
    /// die Slots müssen mittig in der Leiste stehen)
    pub last_slots: Option<Rect>,
    /// Lage der linken und rechten Schaltergruppe der unteren Leiste — geprüft
    /// wird der Abstand zum Fensterrand (`tests_untere_leiste`).
    pub last_kopf_links: Option<Rect>,
    pub last_kopf_rechts: Option<Rect>,
    /// Tasten des Tastenbilds der Tastaturbelegung in Fensterkoordinaten
    /// (Regressionstest T0045: gesperrte Tasten dürfen sich nicht belegen lassen)
    pub last_tasten: Option<Vec<(&'static str, &'static str, Rect)>>,
    /// Breite des zuletzt gezeichneten Ribbon-Inhalts
    pub last_ribbon_content_w: f32,
    pub zoom_edit: Option<String>,
    pub typed: String,
    /// Signatur der Auswahl für die letzte Zeichnung (Panel-Aktualisierung)
    pub sel_stamp: u64,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            pending: PendingStyle::default(),
            ribbon: RibbonTab::Start_default(),
            inventory_open: false,
            inventory_tab: 0,
            layers_open: true,
            props_open: true,
            layers_collapsed: false,
            props_collapsed: false,
            ribbon_collapsed: false,
            keymap_open: false,
            settings_open: false,
            settings_kat: 0,
            settings_suche: String::new(),
            last_settings_suche: None,
            last_settings_nav: None,
            last_settings_pane: None,
            help_open: false,
            about_open: false,
            sheet_dialog: false,
            project_dialog: false,
            texteingabe: None,
            block_name_dialog: None,
            preset_dialog: None,
            hotbar_picker_open: false,
            file_dialog: None,
            systemdialog: None,
            pdf_import: None,
            text_format_next: None,
            show_grid: true,
            lineweights: true,
            fullscreen: false,
            lineweights_scaled: false,
            drag_mode: DragMode::HoldAndDrag,
            fill_input: FillInput::Points,
            hatch_input: FillInput::Contour,
            // Programmstandard ist das Einfügen per Punkten (wie bei der Fläche)
            volume_input: FillInput::Points,
            copy_input: CopyInput::BasePoint,
            fill_active: None,
            menu: None,
            order_pick: None,
            bind_slot: None,
            slot_drag: None,
            keymap_selected: None,
            keymap_funktion: None,
            search: String::new(),
            new_layer_dialog: None,
            color_popup: None,
            rename_layer: None,
            clipboard: crate::ghost::Clipboard::default(),
            clipboard_offset: 20.0,
            confirm_new: false,
            confirm_quit: false,
            last_canvas: Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)),
            last_ribbon_content_h: 0.0,
            last_slots: None,
            last_kopf_links: None,
            last_kopf_rechts: None,
            last_tasten: None,
            last_ribbon_content_w: 0.0,
            zoom_edit: None,
            typed: String::new(),
            sel_stamp: 0,
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
            FillKind::Area => "Fläche",
            FillKind::Hatch => "Schraffur",
            FillKind::Volume => "Volumen",
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
            Self::Contour => "Kontur anklicken",
            Self::Points => "Punkte setzen",
            Self::Paint => "Farbeimer",
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
            Self::Clipboard => "Kopieren",
            Self::BasePoint => "Kopieren mit Basispunkt",
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

pub struct App {
    pub tabs: Vec<DocTab>,
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
    /// Leistungsanzeige (Umschalt+F3): Messwerte je Bild und ihr Verlauf
    pub perf: crate::perf::Perf,
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
        let prefs = UserPrefs::load();
        let mut s = Session::new();
        s.doc.name = "Unbenannt".into();
        // App-weite Vorliebe für den Wandkontur-Fang übernehmen, damit die
        // Einstellung auch ohne gespeicherte Zeichnung einen Neustart überlebt.
        s.doc.settings.snap.wall_edges = prefs.snap_wall_edges;
        let tab = DocTab {
            session: s,
            tab: TabKind::Model,
            view_model: View::default(),
            sheet_views: HashMap::new(),
            rev: 0,
            cache: None,
            hover: None,
            selected_viewport: None,
        };
        let options = ToolOptions::from_doc(&tab.session.doc);
        let show_grid = tab.session.doc.settings.show_grid;
        let lineweights = prefs.show_lineweights;
        let lineweights_scaled = prefs.lineweights_scaled;
        let drag_mode = prefs.drag_mode;
        let layers_open = prefs.show_layers;
        let props_open = prefs.show_properties;
        let mut app = Self {
            tabs: vec![tab],
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
            perf: crate::perf::Perf::default(),
        };
        app.ui.show_grid = show_grid;
        app.ui.lineweights = lineweights;
        app.ui.lineweights_scaled = lineweights_scaled;
        app.ui.drag_mode = drag_mode;
        app.ui.copy_input = CopyInput::from_key(&app.prefs.copy_input);
        app.ui.layers_open = layers_open;
        app.ui.props_open = props_open;
        app.fit_view();
        app.started_at = 0.0;
        app
    }

    /// Erlaubt das Setzen eines Startzustands für Screenshots/automatisierte UI-Tests:
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
                "arch" => self.ui.ribbon = RibbonTab::Architektur,
                "draw" => self.ui.ribbon = RibbonTab::Zeichnen,
                "dim" => self.ui.ribbon = RibbonTab::Bemassung,
                "bloecke" | "blocks" => self.ui.ribbon = RibbonTab::Bloecke,
                "blatt" | "sheet" => self.ui.ribbon = RibbonTab::Blatt,
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
                "vollbild" | "fullscreen" => self.ui.fullscreen = true,
                "aendern" | "change" => self.ui.ribbon = RibbonTab::Aendern,
                "inv" | "inventar" => self.ui.inventory_open = true,
                "blocks-tab" => {
                    self.ui.inventory_open = true;
                    self.ui.inventory_tab = 1;
                }
                "functions" => {
                    self.ui.inventory_open = true;
                    self.ui.inventory_tab = 2;
                }
                "keymap" => self.ui.keymap_open = true,
                // Leistungsanzeige offen (Screenshot-Prüfung T0068)
                "perf" | "leistung" => self.perf.an = true,
                // Chat-Protokoll mit Beispielmeldungen (Screenshot-Prüfung)
                "chat" => {
                    for t in [
                        "Polylinie: Startpunkt klicken",
                        "Nächster Punkt klicken – Enter beendet",
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
                        self.chat.sagen(0.0, format!("Meldung {i} – Polylinie: nächster Punkt klicken"));
                    }
                    self.chat.offen = true;
                }
                // Hinweis im geschlossenen Chat (verblassende Meldung)
                "chathinweis" => {
                    self.start_tool("draw.polyline");
                    self.chat.sagen(0.0, "Polylinie: Startpunkt klicken");
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
                            text: "Küche".into(),
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
                        },
                    );
                    e.style.color = Some(jjcad_core::style::Color::BLACK);
                    self.doc_mut().add_entity(e);
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
                                text: format!("{} · Wohnen 12,4 m²", font.label()),
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
                "settings" => self.ui.settings_open = true,
                // Einstellungen mit gewählter Kategorie (`settings:2`) oder mit
                // Suchbegriff (`settings:Fenster`) — Prüfbild für T0061.
                s if s.starts_with("settings:") => {
                    self.ui.settings_open = true;
                    let wahl = &s["settings:".len()..];
                    match wahl.parse::<usize>() {
                        Ok(n) => {
                            self.ui.settings_kat =
                                n.min(crate::dialogs::EINSTELLUNGSKATEGORIEN.len() - 1)
                        }
                        Err(_) => self.ui.settings_suche = wahl.to_string(),
                    }
                }
                "help" => self.ui.help_open = true,
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
                            &[],
                        ) {
                            self.halt.push(jjcad_core::snap::HaltPunkt::neu(self.doc(), &hit));
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
                "wall" => self.start_tool("draw.wall"),
                "paint" | "farbeimer" => self.start_tool("draw.fill.paint"),
                "door" => self.start_tool("draw.door"),
                "window" => self.start_tool("draw.window"),
                "dimtool" => self.start_tool("dim.linear"),
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
                // Prüffall T0004: Papierraum der Druckvorschau mit ausgewähltem
                // Textfeld des Plankopfes (Eigenschaften-Panel zeigt „Text").
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
                                    .find(|e| {
                                        matches!(&e.kind, EntityKind::Text { text, .. } if text == "{PLANTITEL}")
                                    })
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
        self.tab_mut().tab = kind;
        self.tool = None;
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
        }
    }

    /// Bearbeitungsraum diesem Reiter nachziehen (Modell ↔ Papierraum).
    /// Wird einmal je Frame aufgerufen; die Action-Schicht liest ihn aus dem
    /// Dokument, damit GUI, Tastatur und MCP denselben Weg gehen.
    pub fn sync_space(&mut self) {
        let raum = match self.tab().tab {
            TabKind::Model => None,
            TabKind::Sheet(id) => Some(id),
        };
        if self.session().space() != raum {
            self.session_mut().set_space(raum);
            self.tool = None;
            self.ghost = None;
            self.tab_mut().selected_viewport = None;
            self.tab_mut().hover = None;
            // Die Anzeigeliste des alten Raums gilt hier nicht mehr.
            self.tab_mut().cache = None;
            // Der Papierraum ist in Blatt-Millimetern gezeichnet: dort ist eine
            // Texthöhe von 2,5 mm richtig, nicht die 250 mm des Modells.
            let modell_hoehe = self.doc().settings.text_height;
            if self.doc().in_paper_space() {
                self.options.text_height = 2.5;
            } else if (self.options.text_height - 2.5).abs() < 1e-9 {
                self.options.text_height = modell_hoehe;
            }
        }
    }

    /// Läuft die Bearbeitung im Papierraum (Druckvorschau)?
    pub fn im_papierraum(&self) -> bool {
        self.doc().in_paper_space()
    }

    pub fn unit(&self) -> Unit {
        self.doc().settings.input_unit
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
            self.toast("Auswahl · leer");
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
        self.toast(if front { "Ganz nach vorn" } else { "Ganz nach hinten" });
    }

    /// „Vor/Hinter Element“: der nächste Klick in die Zeichenfläche bestimmt das
    /// Referenzobjekt (ausgewertet in `canvas`). Braucht eine Auswahl.
    pub fn order_pick_start(&mut self, above: bool) {
        if self.selection().is_empty() {
            self.toast("Reihenfolge · erst ein Element wählen");
            return;
        }
        self.ui.order_pick = Some(above);
        self.status_msg = if above {
            "Element anklicken, VOR das die Auswahl soll — ESC bricht ab"
        } else {
            "Element anklicken, HINTER das die Auswahl soll — ESC bricht ab"
        }
        .into();
    }

    // ---- Ansicht ----

    pub fn current_view(&self) -> View {
        let t = self.tab();
        match t.tab {
            TabKind::Model => t.view_model,
            TabKind::Sheet(id) => *t.sheet_views.get(&id).unwrap_or(&View {
                center: V2::new(0.0, 0.0),
                zoom: 2.2,
                rot: 0.0,
            }),
        }
    }

    pub fn set_view(&mut self, v: View) {
        let active = self.active;
        let tabkind = self.tabs[active].tab;
        let t = &mut self.tabs[active];
        match tabkind {
            TabKind::Model => t.view_model = v,
            TabKind::Sheet(id) => {
                t.sheet_views.insert(id, v);
            }
        }
        t.cache = None;
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
        let rect = self.ui.last_canvas;
        let (w, h) = (rect.width().max(50.0) as f64, rect.height().max(50.0) as f64);
        let tab = self.tab();
        let (bbox, is_sheet): (BBox, bool) = match tab.tab {
            TabKind::Model => {
                let mut b = jjcad_core::render::visible_bbox(&tab.session.doc);
                if !b.is_valid() || b.width() < 1.0 {
                    b = BBox::new(V2::new(0.0, 0.0), V2::new(12000.0, 9000.0));
                }
                (b, false)
            }
            TabKind::Sheet(id) => match tab.session.doc.sheet(id) {
                Some(s) => (s.bbox(), true),
                None => (BBox::new(V2::new(0.0, 0.0), V2::new(420.0, 297.0)), true),
            },
        };
        let bw = bbox.width().max(1.0);
        let bh = bbox.height().max(1.0);
        let margin = if is_sheet { 46.0 } else { 70.0 };
        let zoom = ((w - margin).max(60.0) / bw).min((h - margin).max(60.0) / bh);
        let v = View {
            center: bbox.center(),
            zoom: zoom.clamp(1e-4, 60.0),
            ..self.current_view()
        };
        self.set_view(v);
    }

    pub fn zoom_by(&mut self, factor: f64) {
        let mut v = self.current_view();
        v.zoom = (v.zoom * factor).clamp(1e-5, 80.0);
        self.set_view(v);
    }

    /// Aktionen aus Ribbon, Hotbar, Tastatur oder Menü.
    pub fn run_action(&mut self, ctx: &egui::Context, id: &str) {
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
            "file.new" => self.ui.confirm_new = true,
            "file.open" => self.open_file_dialog(FileMode::Open),
            "file.save" => self.save_current(false),
            "file.save_as" => self.open_file_dialog(FileMode::SaveJjcad),
            "file.export_pdf" | "sheet.pdf" => self.open_file_dialog(FileMode::ExportPdf),
            "file.export_dxf" => self.open_file_dialog(FileMode::ExportDxf),
            "file.import_dxf" => self.open_file_dialog(FileMode::ImportDxf),
            "file.import.pdf" => self.open_file_dialog(FileMode::ImportPdf),
            "file.export_png" => self.open_file_dialog(FileMode::ExportPng),
            "edit.undo" => {
                if self.session_mut().undo() {
                    self.mark_changed();
                    self.toast("Verlauf · zurück");
                }
            }
            "edit.redo" => {
                if self.session_mut().redo() {
                    self.mark_changed();
                    self.toast("Verlauf · wiederholt");
                }
            }
            "edit.copy" => self.copy_selection(false),
            "modify.erase" => {
                let ids = self.selection().to_vec();
                if ids.is_empty() {
                    self.start_tool("modify.select");
                    self.toast("Löschen · erst Objekte wählen");
                } else {
                    let _ = self.session_mut().execute("modify.erase", &serde_json::json!({ "ids": ids }));
                    self.mark_changed();
                }
            }
            "modify.explode" | "block.explode" => {
                let ids = self.selection().to_vec();
                if ids.is_empty() {
                    self.toast("Blöcke · erst auswählen");
                } else {
                    match self
                        .session_mut()
                        .execute("modify.explode", &serde_json::json!({ "ids": ids }))
                    {
                        Ok(_) => {
                            self.mark_changed();
                            self.toast("Objekt · zerlegt");
                        }
                        Err(e) => self.toast(e),
                    }
                }
            }
            "block.create" => {
                if self.selection().is_empty() {
                    self.toast("Auswahl · leer");
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
            "view.layers" => self.ui.layers_open = !self.ui.layers_open,
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
                self.toast(format!("Hotbar · {name}"));
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
                                self.toast(format!("Verlauf · wiederholt: {}", actions::action(&last).map(|d| d.label).unwrap_or("")));
                            }
                            Err(e) => self.toast(e),
                        }
                    }
                }
            }
            "view.zoom_extents" => self.fit_view(),
            "view.zoom_in" => self.zoom_by(1.25),
            "view.zoom_out" => self.zoom_by(0.8),
            // Umsetzen der Ansicht: ein einzelner Aufruf (Hotbar, Kürzel aus
            // einer anderen Quelle) rückt um einen Schritt weiter; gehalten
            // läuft der Schwenk in `canvas::handle_navigation` (T0056).
            "view.pan_left" => self.schwenk_schritt(-1.0, 0.0),
            "view.pan_right" => self.schwenk_schritt(1.0, 0.0),
            "view.pan_up" => self.schwenk_schritt(0.0, -1.0),
            "view.pan_down" => self.schwenk_schritt(0.0, 1.0),
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
                if let TabKind::Sheet(sid) = self.tab().tab {
                    let r = self.session_mut().execute(
                        "sheet.add_viewport",
                        &serde_json::json!({"sheet": sid, "show_frame": true}),
                    );
                    match r {
                        Ok(_) => {
                            self.mark_changed();
                            self.toast("Ansichtsfenster · angelegt");
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

    pub fn start_tool(&mut self, id: &str) {
        if actions::action(id).is_none() {
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
            selecting: self.session().selection.is_empty()
                && matches!(
                    id,
                    "modify.move"
                        | "modify.copy"
                        | "modify.rotate"
                        | "modify.mirror"
                        | "modify.scale"
                        | "modify.offset"
                        | "modify.explode"
                ),
            phase: ToolPhase::Collecting,
        });
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
        let finisheable = self
            .tool
            .as_ref()
            .map(|t| {
                let punktfolge = matches!(
                    t.action.as_str(),
                    "draw.polyline" | "draw.spline" | "dim.chain"
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
        self.start_tool("modify.quickselect");
    }

    /// Ist die Schnellauswahl das laufende Werkzeug? Dann hebt die
    /// Zeichenfläche das Objekt unter dem Zeiger hervor, obwohl ein Werkzeug
    /// aktiv ist.
    pub fn schnellauswahl_aktiv(&self) -> bool {
        self.tool_id() == Some("modify.quickselect")
    }

    pub fn cancel_tool(&mut self) {
        if self.tool.is_some() {
            self.tool = None;
            self.mitteilung("Abgebrochen");
        }
        if self.ghost.take().is_some() {
            self.mitteilung("Platzieren · abgebrochen");
        }
        // Laufendes Ziehen der Auswahl zurücknehmen (keine halb verschobene
        // Geometrie zurücklassen)
        self.cancel_move_drag();
        self.cancel_grip_drag();
        self.session_mut().clear_selection();
        self.select_drag = None;
    }

    pub fn bind_slot_action(&mut self, slot: usize, action: Option<String>) {
        self.prefs.set_slot(slot, action);
        let name = self.prefs.hotbar_presets[self.prefs.active_preset].name.clone();
        let _ = self.prefs.save();
        self.toast(format!("Hotbar · Slot {} belegt ({name})", slot + 1));
    }

    // ---- Dateien ----

    pub fn open_file_dialog(&mut self, mode: FileMode) {
        // Ein bereits offener Systemdialog bleibt; nicht doppelt öffnen.
        if self.ui.systemdialog.is_some() {
            return;
        }
        if self.prefs.dateidialog_system() && crate::systemdialog::verfuegbar() {
            let (dir, base) = self.starthilfe();
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
        let (start_dir, base) = self.starthilfe();
        self.ui.file_dialog = Some(FileDialog::new(mode, start_dir, &base));
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
                self.toast(format!("Datei · gespeichert: {}", path.display()));
            }
            Err(e) => self.toast(e),
        }
    }

    pub fn load_document(&mut self, path: &PathBuf) {
        match Session::open(path) {
            Ok(s) => {
                let tab = DocTab {
                    session: s,
                    tab: TabKind::Model,
                    view_model: View::default(),
                    sheet_views: HashMap::new(),
                    rev: 0,
                    cache: None,
                    hover: None,
                    selected_viewport: None,
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
                self.options = ToolOptions::from_doc(self.doc());
                self.ui.show_grid = self.doc().settings.show_grid;
                self.prefs.add_recent(&path.to_string_lossy());
                if let Some(p) = path.parent() {
                    self.prefs.last_dir = p.to_string_lossy().to_string();
                }
                let _ = self.prefs.save();
                self.fit_view();
                self.toast(format!("Datei · geöffnet: {}", path.file_name().unwrap_or_default().to_string_lossy()));
            }
            Err(e) => self.toast(e),
        }
    }

    pub fn new_document(&mut self) {
        let mut s = Session::new();
        s.doc.settings.snap.wall_edges = self.prefs.snap_wall_edges;
        let tab = DocTab {
            session: s,
            tab: TabKind::Model,
            view_model: View::default(),
            sheet_views: HashMap::new(),
            rev: 0,
            cache: None,
            hover: None,
            selected_viewport: None,
        };
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.options = ToolOptions::from_doc(self.doc());
        self.fit_view();
        self.toast("Datei · neu");
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.new_document();
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        self.tool = None;
    }

    pub fn export_pdf_to(&mut self, path: PathBuf) {
        let idx = match self.tab().tab {
            TabKind::Sheet(id) => self.doc().sheets.iter().position(|s| s.id == id),
            TabKind::Model => None,
        };
        let res = if let Some(i) = idx {
            pdf::export_pdf(self.doc(), &path, Some(i))
        } else {
            pdf::export_all_sheets_pdf(self.doc(), &path)
        };
        match res {
            Ok(_) => self.toast(format!("PDF · exportiert: {}", path.display())),
            Err(e) => self.toast(e),
        }
    }

    pub fn export_dxf_to(&mut self, path: PathBuf) {
        match dxf::export_dxf(self.doc(), &path) {
            Ok(_) => self.toast(format!("DXF · exportiert: {}", path.display())),
            Err(e) => self.toast(e),
        }
    }

    pub fn export_png_to(&mut self, path: PathBuf) {
        let sheet = match self.tab().tab {
            TabKind::Sheet(id) => self.doc().sheets.iter().position(|s| s.id == id),
            TabKind::Model => None,
        };
        let opts = RasterOptions {
            width_px: 2400,
            height_px: if sheet.is_some() { 1697 } else { 1600 },
            sheet,
            ..Default::default()
        };
        match raster::render_png(self.doc(), &opts, &path) {
            Ok(_) => self.toast(format!("PNG · exportiert: {}", path.display())),
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
            Err(e) => self.toast(format!("PDF · {e}")),
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
                let mut text = format!(
                    "PDF · {} Objekte übernommen ({} Seite {})",
                    objekte,
                    wahl
                        .pfad
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    wahl.seite
                );
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
                    text.push_str(" · nicht übernommen: ");
                    text.push_str(&warn.join(", "));
                }
                self.toast(text);
            }
            Err(e) => self.toast(format!("PDF · {e}")),
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
                let mut msg = format!("DXF · {n} Objekte importiert");
                if warn > 0 {
                    msg.push_str(&format!(", {warn} Hinweise"));
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
            self.toast(if self.perf.an { "Leistung · an" } else { "Leistung · aus" });
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
                self.toast("Reihenfolge · kein Objekt gewählt");
            } else if self.ui.bind_slot.is_some() {
                self.ui.bind_slot = None;
                self.toast("Hotbar · verknüpfen abgebrochen");
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
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        // Strg+C/X/V kommen von egui als Ereignisse, nicht als Tastendruck
        let (ev_copy, ev_cut, ev_paste) = ctx.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            )
        });
        if !typing {
            if ev_copy {
                // Strg+Umschalt+C kommt bei egui ebenfalls als Kopier-Ereignis
                // an (egui-winit prüft nur Strg) — Umschalt entscheidet über den
                // Basispunkt, damit Strg+C weiterhin nur kopiert.
                if shift {
                    self.start_tool("modify.copy");
                } else {
                    self.run_action(ctx, "edit.copy");
                }
            }
            if ev_cut {
                self.copy_selection(true);
            }
            if ev_paste {
                self.start_ghost_paste();
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
            }
            return;
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
                    self.toast(format!("Wandausrichtung: {}", self.options.wall_align.label()));
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
                            "Öffnet zur Wandinnenseite"
                        } else {
                            "Öffnet zur Wandaußenseite"
                        });
                        return;
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                        self.options.hinge = -self.options.hinge;
                        self.toast(if self.options.hinge >= 0.0 {
                            "Anschlag rechts"
                        } else {
                            "Anschlag links"
                        });
                        return;
                    }
                }
            }
        }

        if ctrl {
            if ctx.input(|i| i.key_pressed(egui::Key::Z)) {
                run = Some(if shift { "edit.redo" } else { "edit.undo" }.into());
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
                run = Some("edit.redo".into());
            }
            if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                run = Some(if shift { "file.save_as" } else { "file.save" }.into());
            }
            if ctx.input(|i| i.key_pressed(egui::Key::O)) {
                run = Some("file.open".into());
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                run = Some("file.new".into());
            }
            if ctx.input(|i| i.key_pressed(egui::Key::P)) {
                run = Some("file.export_pdf".into());
            }
            if ctx.input(|i| i.key_pressed(egui::Key::A)) {
                run = Some("select.all".into());
            }
        } else if !typing {
            // Einzeltasten über die Belegung (Buchstaben und Symbole, T0056)
            for (key, label) in EINZELTASTEN {
                // Einzeltasten gelten nur ohne Umschalt — sonst würde
                // Umschalt+I zusätzlich die Belegung von "i" auslösen.
                if !shift && ctx.input(|i| i.key_pressed(*key)) {
                    if let Some(action) = self.prefs.action_for_key(label) {
                        // Der Ansichtsschwenk ist keine Aktion, die man
                        // „auslöst": er läuft, solange die Taste gehalten wird
                        // (`canvas::handle_navigation`). Ein Druck würde die
                        // Ansicht sonst zweimal bewegen.
                        if !action.starts_with("view.pan_") {
                            run = Some(action);
                        }
                    }
                }
            }
            // Umschalt-Ebene: jede Taste kann ein zweites Kürzel tragen, in der
            // Belegung als "shift+<Zeichen>" gespeichert (Umschalt+I =
            // Füllfläche nach Punktfolge, Umschalt+V = Volumen). Gilt für alle
            // Einzeltasten, nicht nur für eine fest eingetragene (T0056).
            if shift {
                for (key, label) in EINZELTASTEN {
                    if !ctx.input(|i| i.key_pressed(*key)) {
                        continue;
                    }
                    let kombi = format!("shift+{label}");
                    if let Some(action) = self.prefs.action_for_key(&kombi) {
                        run = Some(action);
                    }
                }
            }
            let tool_waiting = self.tool.as_ref().map(|t| !t.pts.is_empty()).unwrap_or(false);
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
                    // Ziffer → Slot über **eine** Quelle (`hotbar::taste_slot`):
                    // „0" ist der letzte Slot, nicht der erste (T0065).
                    if let Some(slot) = crate::hotbar::taste_slot(label) {
                        if let Some(Some(action)) = self.prefs.slots().get(slot) {
                            run = Some(action.clone());
                        }
                    }
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                if self.tool.is_none() {
                    run = Some("edit.repeat".into());
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !tool_waiting {
                self.tool_enter();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                // Füllfläche/Schraffur im Modus „Punkte setzen": Entf und
                // Backspace brechen die Eingabe ab — ESC bestätigt dort.
                // Nur ohne Tippeingabe, sonst löscht Backspace die getippte
                // Zahl/Länge statt das Werkzeug zu beenden.
                if self.ui.typed.is_empty() && self.fill_points_running() {
                    self.cancel_tool();
                    self.toast("Füllfläche · verworfen");
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
        }
        if let Some(id) = run {
            self.run_action(ctx, &id);
        }
    }

    /// Ist eine Taste gedrückt, die in der Belegung auf `action` steht? Damit
    /// hängen Sonderfunktionen an einer **Funktion** statt an einer festen
    /// Taste (T0056): drehen/spiegeln der Einfüge-Vorschau folgt „Drehen" und
    /// „Spiegeln" aus der Tastaturbelegung.
    fn taste_der_funktion(&self, ctx: &egui::Context, action: &str) -> bool {
        EINZELTASTEN.iter().any(|(key, label)| {
            self.prefs.action_for_key(label).as_deref() == Some(action)
                && ctx.input(|i| i.key_pressed(*key))
        })
    }

    /// Schieberichtung der gehaltenen Tasten in Bildschirmrichtung
    /// (x nach rechts, y nach unten). Welche Taste welche Richtung schiebt,
    /// steht in der **Tastaturbelegung** (`view.pan_*`) — nicht im Code (T0056).
    pub fn schwenk_richtung(&self, ctx: &egui::Context) -> Vec2 {
        let mut dir = Vec2::ZERO;
        for (key, label) in EINZELTASTEN {
            let Some(action) = self.prefs.action_for_key(label) else {
                continue;
            };
            if !ctx.input(|i| i.key_down(*key)) {
                continue;
            }
            match action.as_str() {
                "view.pan_left" => dir.x -= 1.0,
                "view.pan_right" => dir.x += 1.0,
                "view.pan_up" => dir.y -= 1.0,
                "view.pan_down" => dir.y += 1.0,
                _ => {}
            }
        }
        dir
    }

    /// Schiebt die Ansicht um einen Schritt (`SCHWENK_SCHRITT_PX` Bildpunkte) in
    /// Bildschirmrichtung — die Umrechnung läuft über die gedrehte Ansicht,
    /// damit der Schub auch bei gedrehter Zeichnung der Taste folgt.
    pub fn schwenk_schritt(&mut self, dx: f32, dy: f32) {
        let mut v = self.current_view();
        // T0067: Der Einzelschritt folgt demselben Faktor wie das Halten.
        let px = SCHWENK_SCHRITT_PX * self.prefs.pan_speed;
        let w = v.screen_delta_to_world(Vec2::new(dx * px, dy * px));
        v.center += w;
        self.set_view(v);
    }

    fn tool_enter(&mut self) {
        if let Some(tool) = self.tool.clone() {
            match tool.action.as_str() {
                "draw.polyline" | "draw.spline" | "dim.chain" => {
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
        let msg = format!(
            "JoJa-CAD · Zwischenablage ({} Objekte)",
            self.ui.clipboard.len()
        );
        self.ctx.copy_text(msg);
        if cut {
            let _ = self.session_mut().execute("modify.erase", &serde_json::json!({"ids": ids}));
            self.session_mut().clear_selection();
            self.mark_changed();
        }
        self.toast(if cut { "Auswahl · ausgeschnitten" } else { "Auswahl · kopiert" });
    }

    pub fn paste_clipboard(&mut self) {
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
        self.toast("Auswahl · eingefügt");
    }

    // ---- Zeichnungstabs ----

    fn doc_tabs(&mut self, ui: &mut egui::Ui) {
        let mut switch_to: Option<TabKind> = None;
        let mut close_sheet: Option<Id> = None;
        let mut new_sheet = false;
        let tabs: Vec<(TabKind, String, bool)> = {
            let t = self.tab();
            let mut v = vec![(TabKind::Model, t.session.doc.name.clone(), false)];
            for s in &t.session.doc.sheets {
                v.push((TabKind::Sheet(s.id), s.name.clone(), true));
            }
            v
        };
        let current = self.tab().tab;
        let doc_modified = self.doc().modified;
        ui.horizontal_centered(|ui| {
            ui.add_space(6.0);
            for (kind, name, closable) in &tabs {
                let active = current == *kind;
                let galley = ui.painter().layout_no_wrap(
                    name.clone(),
                    FontId::proportional(12.0),
                    if active { Palette::TEXT } else { Palette::TEXT_DIM },
                );
                let w = galley.size().x + if *closable { 52.0 } else { 36.0 };
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
                if active {
                    ui.painter().rect_filled(rect, Rounding::same(7.0), Palette::PANEL_ALT);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, Rounding::same(7.0), Palette::PANEL);
                }
                crate::icons::paint(
                    ui.painter(),
                    Rect::from_min_size(rect.min + Vec2::new(7.0, 6.0), Vec2::splat(14.0)),
                    match kind {
                        TabKind::Model => "layers",
                        TabKind::Sheet(_) => "sheet",
                    },
                    if active { Palette::ACCENT } else { Palette::TEXT_FAINT },
                );
                ui.painter().galley(
                    Pos2::new(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0),
                    galley,
                    if active { Palette::TEXT } else { Palette::TEXT_DIM },
                );
                if active && doc_modified && matches!(kind, TabKind::Model) {
                    ui.painter().circle_filled(
                        Pos2::new(rect.left() + 4.0, rect.center().y),
                        2.2,
                        Palette::ACCENT,
                    );
                }
                if *closable {
                    let xr = Rect::from_center_size(
                        Pos2::new(rect.right() - 13.0, rect.center().y),
                        Vec2::splat(16.0),
                    );
                    let xresp = ui.interact(xr, ui.id().with(("close", name)), Sense::click());
                    if xresp.clicked() {
                        if let TabKind::Sheet(id) = kind {
                            close_sheet = Some(*id);
                        }
                    }
                    let c = if xresp.hovered() { Palette::DANGER } else { Palette::TEXT_FAINT };
                    ui.painter().line_segment(
                        [xr.center() - Vec2::splat(3.5), xr.center() + Vec2::splat(3.5)],
                        Stroke::new(1.4, c),
                    );
                    ui.painter().line_segment(
                        [xr.center() + Vec2::new(3.5, -3.5), xr.center() + Vec2::new(-3.5, 3.5)],
                        Stroke::new(1.4, c),
                    );
                }
                if resp.clicked() {
                    switch_to = Some(*kind);
                }
            }
            ui.add_space(4.0);
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, Rounding::same(6.0), Palette::PANEL_ALT);
            }
            crate::icons::paint(
                ui.painter(),
                r.shrink(5.0),
                "plus",
                if resp.hovered() { Palette::TEXT } else { Palette::TEXT_FAINT },
            );
            if resp.clicked() {
                new_sheet = true;
            }
            resp.on_hover_text("Neues Blatt");
        });
        if let Some(kind) = switch_to {
            self.switch_tab(kind);
        }
        if let Some(id) = close_sheet {
            if self.doc().sheets.len() > 1 {
                let _ = self.session_mut().execute("sheet.delete", &serde_json::json!({"sheet": id}));
                if self.tab().tab == TabKind::Sheet(id) {
                    self.switch_tab(TabKind::Model);
                }
                self.tab_mut().sheet_views.remove(&id);
                self.mark_changed();
            } else {
                self.toast("Blatt · das letzte bleibt offen");
            }
        }
        if new_sheet {
            self.run_action(ui.ctx(), "sheet.new");
        }
    }

    // ---- Kopfzeile + Ribbon ----

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(10.0);
            // Logo
            let (r, _) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::hover());
            ui.painter().rect_filled(r, Rounding::same(6.0), Palette::ACCENT);
            ui.painter().rect_stroke(r, Rounding::same(6.0), Stroke::new(1.0, Palette::ACCENT));
            ui.painter().line_segment(
                [r.center() + Vec2::new(-6.0, -5.0), r.center() + Vec2::new(6.0, -5.0)],
                Stroke::new(2.0, Palette::ACCENT_TEXT),
            );
            ui.painter().line_segment(
                [r.center() + Vec2::new(-6.0, 0.0), r.center() + Vec2::new(3.0, 0.0)],
                Stroke::new(2.0, Palette::ACCENT_TEXT),
            );
            ui.painter().line_segment(
                [r.center() + Vec2::new(-6.0, 5.0), r.center() + Vec2::new(6.0, 5.0)],
                Stroke::new(2.0, Palette::ACCENT_TEXT),
            );
            ui.add_space(4.0);
            ui.label(theme::bold("JoJa-CAD", 14.0));
            ui.label(
                RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .size(9.5)
                    .color(Palette::TEXT_FAINT),
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
                    if active { Palette::TEXT } else { Palette::TEXT_DIM },
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
                        Palette::ACCENT,
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, Rounding::same(6.0), Palette::PANEL);
                }
                ui.painter().galley(
                    Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0 - 1.0),
                    galley,
                    if active { Palette::TEXT } else { Palette::TEXT_DIM },
                );
                if resp.clicked() {
                    self.ui.ribbon = rt;
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                let (ric, rtip) = if self.ui.ribbon_collapsed {
                    ("chevron_down", "Ribbon ausklappen")
                } else {
                    ("chevron_up", "Ribbon einklappen")
                };
                if widgets::icon_button(ui, ric, 26.0, self.ui.ribbon_collapsed, rtip).clicked() {
                    self.ui.ribbon_collapsed = !self.ui.ribbon_collapsed;
                }
                if widgets::icon_button(ui, "help", 26.0, false, "Hilfe (F1)").clicked() {
                    self.ui.help_open = true;
                }
                if widgets::icon_button(ui, "keyboard", 26.0, self.ui.keymap_open, "Tastaturbelegung").clicked() {
                    self.ui.keymap_open = !self.ui.keymap_open;
                }
                if widgets::icon_button(ui, "settings", 26.0, self.ui.settings_open, "Einstellungen").clicked() {
                    self.ui.settings_open = !self.ui.settings_open;
                }
                ui.add_space(6.0);
                widgets::vline(ui, 20.0);
                let can_redo = self.session().history.can_redo();
                let can_undo = self.session().history.can_undo();
                if widgets::icon_button(ui, "redo", 26.0, false, "Wiederholen (Strg+Y)").clicked() && can_redo {
                    self.run_action(ui.ctx(), "edit.redo");
                }
                if widgets::icon_button(ui, "undo", 26.0, false, "Rückgängig (Strg+Z)").clicked() && can_undo {
                    self.run_action(ui.ctx(), "edit.undo");
                }
                ui.add_space(6.0);
                let name = self.doc().name.clone();
                let modified = self.doc().modified;
                ui.label(
                    RichText::new(format!("{}{}", name, if modified { " •" } else { "" }))
                        .size(11.5)
                        .color(Palette::TEXT_DIM),
                );
            });
        });
    }

/// Schmale Leiste eines eingeklappten Seitenpanels.
fn rail(ui: &mut egui::Ui, icon: &str, tooltip: &str, chevron: &str) -> bool {
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        ui.add_space(6.0);
        if widgets::icon_button(ui, chevron, 22.0, false, tooltip).clicked() {
            clicked = true;
        }
        ui.add_space(10.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
        crate::icons::paint(ui.painter(), r, icon, Palette::TEXT_DIM);
        if resp.clicked() {
            clicked = true;
        }
        resp.on_hover_text(tooltip);
    });
    clicked
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
                    self.group(ui, "Zeichnung", |ui, app| {
                        if widgets::ribbon_button(ui, "new", "Neu", false).clicked() {
                            app.run_action(ui.ctx(), "file.new");
                        }
                        if widgets::ribbon_button(ui, "open", "Öffnen", false).clicked() {
                            app.run_action(ui.ctx(), "file.open");
                        }
                        if widgets::ribbon_button(ui, "save", "Speichern", false).clicked() {
                            app.run_action(ui.ctx(), "file.save");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Ausgabe", |ui, app| {
                        if widgets::ribbon_button(ui, "pdf", "PDF", false).clicked() {
                            app.run_action(ui.ctx(), "file.export_pdf");
                        }
                        if widgets::ribbon_button(ui, "dxf", "DXF", false).clicked() {
                            app.run_action(ui.ctx(), "file.export_dxf");
                        }
                        if widgets::ribbon_button(ui, "image", "PNG", false).clicked() {
                            app.run_action(ui.ctx(), "file.export_png");
                        }
                        if widgets::ribbon_button(ui, "import", "Import", false).clicked() {
                            app.run_action(ui.ctx(), "file.import_dxf");
                        }
                        if widgets::ribbon_button(ui, "pdf", "PDF einfügen", false).clicked() {
                            app.run_action(ui.ctx(), "file.import.pdf");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Zwischenablage", |ui, app| {
                        // Kopieren ist eine geteilte Schaltfläche: Hauptfläche
                        // führt den zuletzt gewählten Modus aus, der Pfeil
                        // öffnet die Auswahl (Zwischenablage / Basispunkt).
                        app.copy_group_button(ui);
                        if widgets::ribbon_button(ui, "import", "Einfügen", false).clicked() {
                            app.paste_clipboard();
                        }
                        if widgets::ribbon_button(ui, "erase", "Ausschneiden", false).clicked() {
                            app.copy_selection(true);
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Projekt", |ui, app| {
                        if widgets::ribbon_button(ui, "info", "Projektdaten", false).clicked() {
                            app.ui.project_dialog = true;
                        }
                        if widgets::ribbon_button(ui, "settings", "Einstellungen", false).clicked() {
                            app.ui.settings_open = true;
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Ansicht", |ui, app| {
                        if widgets::ribbon_button(ui, "inventory", "Inventar", app.ui.inventory_open).clicked() {
                            app.run_action(ui.ctx(), "view.inventory");
                        }
                        if widgets::ribbon_button(ui, "keyboard", "Tasten", app.ui.keymap_open).clicked() {
                            app.ui.keymap_open = !app.ui.keymap_open;
                        }
                        if widgets::ribbon_button(ui, "hotbar", "Hotbar", false).clicked() {
                            app.run_action(ui.ctx(), "view.hotbar_next");
                        }
                    });
                }
                RibbonTab::Zeichnen => {
                    self.group(ui, "Grundelemente", |ui, app| {
                        for (id, icon, label) in [
                            ("draw.line", "line", "Linie"),
                            ("draw.polyline", "polyline", "Polylinie"),
                            ("draw.spline", "spline", "Spline"),
                            ("draw.rect", "rect", "Rechteck"),
                            ("draw.circle", "circle", "Kreis"),
                            ("draw.arc", "arc", "Bogen"),
                        ] {
                            // Rechteck trägt zwei Einfügemodi — der Pfeil am
                            // Knopf öffnet sie (Über Ecken / Basislinie + Höhe).
                            if id == "draw.rect" {
                                app.einfuege_group_button(ui, "draw.rect");
                                continue;
                            }
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Füllen", |ui, app| {
                        // Zwei geteilte Schaltflächen: Fläche und Schraffur.
                        // Der Pfeil öffnet die Eingabemodi, der Knopf startet
                        // den zuletzt benutzten — das Icon zeigt ihn an.
                        app.fill_group_button(ui, FillKind::Area);
                        app.fill_group_button(ui, FillKind::Hatch);
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Weitere", |ui, app| {
                        for (id, icon, label) in [
                            ("draw.ellipse", "ellipse", "Ellipse"),
                            ("draw.point", "point", "Punkt"),
                            ("draw.text", "text", "Text"),
                        ] {
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
                    self.group(ui, "Bauteile", |ui, app| {
                        for (id, icon, label) in [
                            ("draw.wall", "wall", "Wand"),
                            ("draw.door", "door", "Tür"),
                            ("draw.window", "window", "Fenster"),
                            ("draw.stair", "stair", "Treppe"),
                        ] {
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                        // Volumen mit denselben drei Einfüge-Modi wie die Fläche
                        app.fill_group_button(ui, FillKind::Volume);
                    });
                }
                RibbonTab::Aendern => {
                    self.group(ui, "Bearbeiten", |ui, app| {
                        for (id, icon, label) in RIBBON_BEARBEITEN {
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
                    self.group(ui, "Geometrie", |ui, app| {
                        for (id, icon, label) in [
                            ("modify.offset", "offset", "Versetzen"),
                            ("modify.trim", "trim", "Stutzen"),
                            ("modify.extend", "extend", "Verlängern"),
                            ("modify.explode", "explode", "Zerlegen"),
                            ("modify.erase", "erase", "Löschen"),
                        ] {
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    // Zeichenreihenfolge: dieselben vier Funktionen wie im
                    // Objekt-Kontextmenü, hier für die aktuelle Auswahl.
                    self.group(ui, "Reihenfolge", |ui, app| {
                        for (icon, label, tip, front) in [
                            (
                                "order-front",
                                "Vordergrund",
                                "Auswahl ganz nach vorn — zuletzt gezeichnet",
                                true,
                            ),
                            (
                                "order-back",
                                "Hintergrund",
                                "Auswahl ganz nach hinten — zuerst gezeichnet",
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
                                "Vor Element",
                                "Auswahl vor ein Element setzen — danach das Element anklicken",
                                true,
                            ),
                            (
                                "order-below",
                                "Hinter Element",
                                "Auswahl hinter ein Element setzen — danach das Element anklicken",
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
                    self.group(ui, "Sonstiges", |ui, app| {
                        for (id, icon, label) in [
                            ("modify.quickselect", "quickselect", "Schnellauswahl"),
                            ("edit.undo", "undo", "Zurück"),
                            ("edit.redo", "redo", "Vor"),
                        ] {
                            let aktiv = app.tool_id() == Some(id);
                            if widgets::ribbon_button(ui, icon, label, aktiv).clicked() {
                                app.run_action(ui.ctx(), id);
                            }
                        }
                    });
                }
                RibbonTab::Bemassung => {
                    self.group(ui, "Maße", |ui, app| {
                        for (id, icon, label) in [
                            ("dim.linear", "dim", "Maß"),
                            ("dim.chain", "dimchain", "Maßkette"),
                            ("dim.elevation", "elevation", "Höhenkote"),
                        ] {
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Maßketten", |ui, app| {
                        // Maßeinheit, Textgröße, Maßstab und Bogensymbole stehen
                        // ausschließlich im Einstellungen-Dialog (Abschnitt
                        // „Bemaßung") — eine Stelle für jeden Wert.
                        if widgets::ribbon_button(ui, "settings", "Einstellungen", false).clicked() {
                            app.ui.settings_open = true;
                        }
                    });
                }
                RibbonTab::Bloecke => {
                    self.group(ui, "Bibliothek", |ui, app| {
                        if widgets::ribbon_button(ui, "library", "Block-Manager", app.ui.inventory_open && app.ui.inventory_tab == 1).clicked() {
                            app.ui.inventory_open = true;
                            app.ui.inventory_tab = 1;
                        }
                        if widgets::ribbon_button(ui, "block", "Einfügen", app.tool_id() == Some("block.insert")).clicked() {
                            app.start_tool("block.insert");
                        }
                        if widgets::ribbon_button(ui, "blocknew", "Aus Auswahl", false).clicked() {
                            app.run_action(ui.ctx(), "block.create");
                        }
                        if widgets::ribbon_button(ui, "explode", "Zerlegen", false).clicked() {
                            app.run_action(ui.ctx(), "modify.explode");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Schnellzugriff", |ui, app| {
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
                    self.group(ui, "Blätter", |ui, app| {
                        if widgets::ribbon_button(ui, "sheet", "Neues Blatt", false).clicked() {
                            app.run_action(ui.ctx(), "sheet.new");
                        }
                        if widgets::ribbon_button(ui, "viewport", "Ansicht", app.tool_id() == Some("sheet.viewport_tool")).clicked() {
                            if matches!(app.tab().tab, TabKind::Sheet(_)) {
                                app.start_tool("sheet.viewport_tool");
                                app.toast("Ansichtsfenster · Rechteck aufziehen");
                            } else {
                                app.toast("Blatt · zuerst ein Blatt öffnen");
                            }
                        }
                        if widgets::ribbon_button(ui, "pdf", "Blatt-PDF", false).clicked() {
                            app.run_action(ui.ctx(), "sheet.pdf");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    // Papierraum: dieselben Zeichenwerkzeuge wie im Modell.
                    // Der Plankopf besteht aus normalen Linien und Textfeldern.
                    self.group(ui, "Papierraum", |ui, app| {
                        if matches!(app.tab().tab, TabKind::Sheet(_)) {
                            for (icon, label, action) in [
                                ("line", "Linie", "draw.line"),
                                ("rect", "Rechteck", "draw.rect"),
                                ("text", "Text", "draw.text"),
                            ] {
                                if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(action)).clicked() {
                                    app.start_tool(action);
                                }
                            }
                        } else {
                            ui.label(theme::dim("Blatt öffnen, dann zeichnen"));
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Blatteinstellungen", |ui, app| {
                        if let TabKind::Sheet(id) = app.tab().tab {
                            let mut paper = app.doc().sheet(id).map(|s| s.paper).unwrap_or_default();
                            let mut landscape = app.doc().sheet(id).map(|s| s.landscape).unwrap_or(true);
                            let mut scale = app.doc().sheet(id).map(|s| s.scale).unwrap_or(100.0);
                            ui.vertical(|ui| {
                                // Zwei Zeilen à 24 px (Abstand 6) = 54 px — wie eine
                                // Knopfreihe, damit der Reiter dieselbe Höhe hat.
                                ui.horizontal(|ui| {
                                    ui.label(theme::dim("Format"));
                                    egui::ComboBox::from_id_salt("paper")
                                        .width(64.0)
                                        .selected_text(paper.label())
                                        .show_ui(ui, |ui| {
                                            for p in jjcad_core::style::PaperSize::ALL {
                                                ui.selectable_value(&mut paper, p, p.label());
                                            }
                                        });
                                    if ui.checkbox(&mut landscape, "quer").changed() {
                                        let _ = app.session_mut().execute(
                                            "sheet.update",
                                            &serde_json::json!({"sheet": id, "paper": paper.label(), "landscape": landscape, "scale": scale}),
                                        );
                                        app.mark_changed();
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label(theme::dim("Maßstab"));
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
                            ui.label(theme::dim("Kein Blatt aktiv – oben auf einen Blatt-Reiter wechseln."));
                        }
                    });
                }
                RibbonTab::Ansicht => {
                    self.group(ui, "Zoom", |ui, app| {
                        if widgets::ribbon_button(ui, "zoomfit", "Alles", false).clicked() {
                            app.fit_view();
                        }
                        if widgets::ribbon_button(ui, "zoomin", "Größer", false).clicked() {
                            app.zoom_by(1.25);
                        }
                        if widgets::ribbon_button(ui, "zoomout", "Kleiner", false).clicked() {
                            app.zoom_by(0.8);
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Drehen", |ui, app| {
                        // Bleibt gedrückt, solange die Ansicht gedreht ist; ein
                        // zweiter Klick stellt die Ausgangsorientierung her.
                        let gedreht = app.current_view().ist_gedreht();
                        let laeuft = app.tool_id() == Some("view.rotate_tool");
                        if widgets::ribbon_button(ui, "rotate", "Drehen", gedreht || laeuft).clicked() {
                            if gedreht {
                                let mut v = app.current_view();
                                v.zurueck_drehen();
                                app.set_view(v);
                                app.tool = None;
                                app.toast("Ansicht · Ausgangslage");
                            } else if laeuft {
                                app.tool = None;
                                app.toast("Ansicht drehen · abgebrochen");
                            } else {
                                app.start_tool("view.rotate_tool");
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Panels", |ui, app| {
                        if widgets::ribbon_button(ui, "layers", "Layer", app.ui.layers_open).clicked() {
                            app.ui.layers_open = !app.ui.layers_open;
                        }
                        if widgets::ribbon_button(ui, "properties", "Eigenschaften", app.ui.props_open).clicked() {
                            app.ui.props_open = !app.ui.props_open;
                        }
                        if widgets::ribbon_button(ui, "grid", "Raster", app.ui.show_grid).clicked() {
                            app.ui.show_grid = !app.ui.show_grid;
                            let v = app.ui.show_grid;
                            let _ = app.session_mut().execute(
                                "doc.settings",
                                &serde_json::json!({"show_grid": v, "snap": {"grid": v}}),
                            );
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Fenster", |ui, app| {
                        let voll = app.is_fullscreen(ui.ctx());
                        if widgets::ribbon_button(ui, "fullscreen", "Vollbild", voll).clicked() {
                            app.run_action(ui.ctx(), "view.fullscreen");
                        }
                    });
                    widgets::vline(ui, RIBBON_GROUP_H);
                    self.group(ui, "Fangen", |ui, app| {
                        ui.vertical(|ui| {
                            // Zwei Zeilen à vier Felder: die Gruppe bleibt damit
                            // genau so hoch wie eine Knopfreihe (54 px), sodass
                            // alle Reiter dieselbe Ribbon-Höhe haben (t_92b03440).
                            ui.spacing_mut().item_spacing.y = 6.0;
                            let mut snap = app.doc().settings.snap.clone();
                            let mut changed = false;
                            ui.horizontal(|ui| {
                                changed |= widgets::check_cell(ui, &mut snap.endpoint, "Endpunkt").changed();
                                changed |= widgets::check_cell(ui, &mut snap.intersection, "Schnitt").changed();
                                changed |= widgets::check_cell(ui, &mut snap.midpoint, "Mitte").changed();
                                changed |= widgets::check_cell(ui, &mut snap.center, "Zentrum").changed();
                            });
                            ui.horizontal(|ui| {
                                changed |= widgets::check_cell(ui, &mut snap.grid, "Fangraster").changed();
                                changed |= widgets::check_cell(ui, &mut snap.ortho, "Ortho").changed();
                                changed |= widgets::check_cell(ui, &mut snap.wall_edges, "Wandkanten").changed();
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

    fn group(&mut self, ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui, &mut App)) {
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
                    Palette::TEXT_DIM,
                );
                let w = galley.size().x;
                let h = galley.size().y;
                ui.painter().galley(
                    Pos2::new(rect.center().x - w * 0.5, rect.center().y - h * 0.5),
                    galley,
                    Palette::TEXT_DIM,
                );
            }
        });
    }


    // ---- Vorschau vor dem Platzieren -------------------------------------

    /// Startet die Vorschau für einen Block aus der Bibliothek.
    pub fn start_ghost_block(&mut self, block: String) {
        let pos = self
            .pointer_world
            .or_else(|| self.pointer_world_fallback())
            .unwrap_or_default();
        self.ghost = Some(crate::ghost::Ghost::new(Some(block), pos));
        self.tool = None;
        self.ui.inventory_open = false;
        self.release_keyboard_focus();
    }

    /// Startet die Vorschau für das Einfügen aus der Zwischenablage.
    pub fn start_ghost_paste(&mut self) {
        if self.ui.clipboard.is_empty() {
            self.toast("Einfügen · Zwischenablage leer");
            return;
        }
        if self.ghost.is_some() {
            return;
        }
        let pos = self
            .pointer_world
            .or_else(|| self.pointer_world_fallback())
            .unwrap_or_default();
        self.ghost = Some(crate::ghost::Ghost::new(None, pos));
        self.tool = None;
        self.session_mut().clear_selection();
        self.release_keyboard_focus();
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
            self.toast("Platzieren · abgebrochen");
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
                // Ebene des Layer-Panels (nicht die Standardebene „Möbel“).
                "layer": self.doc().current_layer,
            });
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
        for mut e in ents {
            e.id = self.doc_mut().new_id();
            ids.push(e.id);
            // Eingefügte Elemente ordnen sich nach ihrem Zeichenrang ein
            // (`Document::insert_ranked`): Flächen hinter Linien und Wänden.
            self.doc_mut().insert_ranked(e);
        }
        self.session_mut().select(ids);
        self.mark_changed();
        self.ghost = None;
        self.toast("Auswahl · eingefügt");
        true
    }

    pub fn activate(&mut self, ctx: &egui::Context, item: &str) {
        if let Some(block_id) = item.strip_prefix("block:") {
            self.start_ghost_block(block_id.to_string());
        } else {
            self.run_action(ctx, item);
        }
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

        // Bearbeitungsraum an den aktiven Reiter binden: Modell oder Papierraum
        // der Druckvorschau. Alle Befehle arbeiten danach im selben Raum.
        self.sync_space();
        self.handle_shortcuts(ctx);

        egui::TopBottomPanel::top("header")
            .exact_height(HEADER_H)
            .frame(Frame::none().fill(Palette::RIBBON))
            .show(ctx, |ui| self.header(ui));

        if !self.ui.ribbon_collapsed {
            egui::TopBottomPanel::top("ribbon")
                .exact_height(RIBBON_H)
                .frame(
                    Frame::none()
                        .fill(Palette::PANEL)
                        .inner_margin(Margin::symmetric(0.0, RIBBON_PAD)),
                )
                .show(ctx, |ui| self.ribbon(ui));
        }

        egui::TopBottomPanel::top("doctabs")
            .exact_height(DOCTABS_H)
            .frame(Frame::none().fill(Palette::RIBBON))
            .show(ctx, |ui| self.doc_tabs(ui));

        egui::TopBottomPanel::bottom("hotbar")
            .exact_height(HOTBAR_H)
            .frame(Frame::none().fill(Palette::PANEL))
            .show(ctx, |ui| hotbar::show(self, ui));

        if self.ui.layers_open {
            let collapsed = self.ui.layers_collapsed;
            let (w, margin) = if collapsed { (28.0, 0.0) } else { (232.0, 10.0) };
            egui::SidePanel::left("layers")
                .exact_width(w)
                .resizable(false)
                .frame(
                    Frame::none()
                        .fill(Palette::PANEL)
                        .inner_margin(Margin::symmetric(margin, if collapsed { 0.0 } else { 8.0 })),
                )
                .show(ctx, |ui| {
                    if collapsed {
                        if Self::rail(ui, "layers", "Layers ausklappen", "chevron_right") {
                            self.ui.layers_collapsed = false;
                        }
                    } else {
                        panels::layers(self, ui);
                    }
                });
        }
        if self.ui.props_open {
            let collapsed = self.ui.props_collapsed;
            let (w, margin) = if collapsed { (28.0, 0.0) } else { (292.0, 10.0) };
            egui::SidePanel::right("props")
                .exact_width(w)
                .resizable(false)
                .frame(
                    Frame::none()
                        .fill(Palette::PANEL)
                        .inner_margin(Margin::symmetric(margin, if collapsed { 0.0 } else { 8.0 })),
                )
                .show(ctx, |ui| {
                    if collapsed {
                        if Self::rail(ui, "properties", "Eigenschaften ausklappen", "chevron_left") {
                            self.ui.props_collapsed = false;
                        }
                    } else {
                        panels::properties(self, ui);
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(Frame::none().fill(Palette::BG))
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
        // Systemdialog offen: Zeichnung abdunkeln und Hinweis zeigen
        dialogs::systemdialog_schleier(self, ctx);
        // Leistungsanzeige zuletzt: sie zeigt die **abgeschlossenen** Bilder und
        // gehört damit über alles andere.
        crate::perf::overlay(self, ctx);
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
pub const HINWEIS_UNBEKANNT: &str = "Werkzeug · Aktion wählen";

/// Aufforderung des laufenden Werkzeugs — ein Text pro Bedienschritt.
///
/// `opts` liefert den Einfügemodus: Rechteck und Ellipse fragen in der
/// Basislinien-Variante einen Punkt mehr ab als achsenparallel.
pub(crate) fn commands_hint(tool: &Tool, opts: &ToolOptions) -> &'static str {
    // Textwerkzeug: nach dem Klick wird getippt, nicht mehr geklickt.
    if tool.action == "draw.text" && tool.phase == ToolPhase::TextInput {
        return "Text · Text eingeben (Esc übernimmt)";
    }
    let lebt = !tool.pts.is_empty();
    let modus = opts.einfuegemodus(&tool.action);
    match tool.action.as_str() {
        "draw.line" => {
            if lebt {
                "Linie · Endpunkt klicken"
            } else {
                "Linie · Startpunkt klicken"
            }
        }
        "draw.polyline" => {
            if lebt {
                "Polylinie · nächster Punkt (Enter beendet)"
            } else {
                "Polylinie · Startpunkt klicken"
            }
        }
        "draw.spline" => {
            if lebt {
                "Spline · nächster Punkt (Enter beendet)"
            } else {
                "Spline · Startpunkt klicken"
            }
        }
        "draw.rect" => match (modus, tool.pts.len()) {
            (Einfuegemodus::Basislinie, 0) => "Rechteck · Basislinie Anfang klicken",
            (Einfuegemodus::Basislinie, 1) => "Rechteck · Basislinie Ende klicken",
            (Einfuegemodus::Basislinie, _) => "Rechteck · Höhe klicken",
            (_, 0) => "Rechteck · erste Ecke klicken",
            _ => "Rechteck · zweite Ecke klicken",
        },
        "draw.circle" => {
            if lebt {
                "Kreis · Radius klicken"
            } else {
                "Kreis · Mittelpunkt klicken"
            }
        }
        "draw.ellipse" => match (modus, tool.pts.len()) {
            (Einfuegemodus::Basislinie, 0) => "Ellipse · Mittelachse Anfang klicken",
            (Einfuegemodus::Basislinie, 1) => "Ellipse · Mittelachse Ende klicken",
            (Einfuegemodus::Basislinie, _) => "Ellipse · Breite klicken",
            (_, 0) => "Ellipse · Mittelpunkt klicken",
            _ => "Ellipse · Achsen aufziehen",
        },
        "draw.arc" => match tool.pts.len() {
            0 => "Bogen · Startpunkt klicken",
            1 => "Bogen · Endpunkt klicken",
            _ => "Bogen · Punkt auf dem Bogen klicken",
        },
        "draw.point" => "Punkt · Position klicken",
        "draw.text" => "Text · Position klicken",
        "draw.wall" => {
            if lebt {
                "Wand · Ende klicken"
            } else {
                "Wand · Anfang klicken"
            }
        }
        "draw.door" => "Tür · in eine Wand klicken",
        "draw.window" => "Fenster · in eine Wand klicken",
        "draw.stair" => match tool.pts.len() {
            0 => "Treppe · Außenkante Anfang klicken",
            1 => "Treppe · Außenkante Ende klicken",
            _ => "Treppe · Breite klicken",
        },
        "draw.fill" => "Füllfläche · Kontur anklicken",
        "draw.fill.boundary" => {
            if lebt {
                "Füllfläche · nächster Umrisspunkt (Enter)"
            } else {
                "Füllfläche · Umriss abfahren"
            }
        }
        "draw.fill.points" => {
            if tool.pts.len() < 3 {
                "Füllfläche · Punkt klicken (mind. 3)"
            } else {
                "Füllfläche · Enter schließt"
            }
        }
        "draw.fill.paint" => "Farbeimer · Fläche anklicken",
        "draw.volume" => "Volumen · Kontur anklicken",
        "draw.volume.points" => {
            if tool.pts.len() < 3 {
                "Volumen nach Punktfolge · Punkt klicken"
            } else {
                "Volumen nach Punktfolge · Enter schließt"
            }
        }
        "draw.volume.paint" => "Volumen füllen · leere Fläche anklicken",
        "draw.hatch" => "Schraffur · Kontur anklicken",
        "dim.linear" => match tool.pts.len() {
            0 => "Maß · erster Punkt",
            1 => "Maß · zweiter Punkt",
            _ => "Maß · Maßlinie platzieren",
        },
        "dim.chain" => {
            if lebt {
                "Maßkette · nächster Punkt (Enter beendet)"
            } else {
                "Maßkette · erster Punkt"
            }
        }
        "dim.elevation" => "Höhenkote · Punkt klicken",
        "modify.select" => "Auswählen · Objekte oder Fenster",
        "modify.move" => match (tool.selecting, lebt) {
            (true, _) => "Verschieben · Objekte wählen (Enter)",
            (false, true) => "Verschieben · Zielpunkt klicken",
            (false, false) => "Verschieben · Basispunkt klicken",
        },
        // Kein Zielpunkt: der Klick setzt nur den Basispunkt der Zwischenablage.
        "modify.copy" => match tool.selecting {
            true => "Kopieren mit Basispunkt · Objekte wählen",
            false => "Kopieren mit Basispunkt · Basispunkt klicken",
        },
        "modify.rotate" => match tool.pts.len() {
            0 => "Drehen · Zentrum klicken",
            1 => "Drehen · Bezugspunkt klicken",
            _ => "Drehen · neuen Punkt klicken",
        },
        "modify.mirror" => {
            if lebt {
                "Spiegeln · zweiten Achspunkt klicken"
            } else {
                "Spiegeln · Punkt der Achse klicken"
            }
        }
        "modify.scale" => {
            if lebt {
                "Skalieren · Faktor klicken"
            } else {
                "Skalieren · Bezugspunkt klicken"
            }
        }
        "modify.offset" => {
            if lebt {
                "Versetzen · Seite klicken"
            } else {
                "Versetzen · Objekt anklicken"
            }
        }
        "modify.trim" => "Stutzen · Segment anklicken",
        "modify.extend" => "Verlängern · Ende anklicken",
        "modify.explode" => "Zerlegen · Objekt anklicken",
        "modify.quickselect" => "Schnellauswahl · Objekt anklicken",
        "modify.order" => "Reihenfolge · Objekt anklicken",
        "block.insert" => "Block · Position klicken",
        "sheet.viewport_tool" => "Ansichtsfenster · Rechteck aufziehen",
        "view.rotate_tool" => match tool.pts.len() {
            0 => "Ansicht drehen · Zentrum klicken",
            1 => "Ansicht drehen · Bezugspunkt klicken",
            _ => "Ansicht drehen · Richtung klicken",
        },
        _ => HINWEIS_UNBEKANNT,
    }
}

/// Ergebnis-Meldung nach einem erfolgreichen Befehl: `<Objekt> · <Zustand>`.
/// `None` = der Befehl meldet sein Ergebnis selbst (Drehen, Stutzen,
/// Reihenfolge) oder braucht keine Rückmeldung.
pub(crate) fn abschluss_meldung(action: &str) -> Option<&'static str> {
    Some(match action {
        "draw.line" => "Linie · gesetzt",
        "draw.polyline" => "Polylinie · abgeschlossen",
        "draw.spline" => "Spline · abgeschlossen",
        "draw.rect" => "Rechteck · gesetzt",
        "draw.circle" => "Kreis · gesetzt",
        "draw.ellipse" => "Ellipse · gesetzt",
        "draw.arc" => "Bogen · gesetzt",
        "draw.point" => "Punkt · gesetzt",
        "draw.text" => "Text · gesetzt",
        "draw.wall" => "Wand · gesetzt",
        "draw.door" => "Tür · eingefügt",
        "draw.window" => "Fenster · eingefügt",
        "draw.stair" => "Treppe · gesetzt",
        "draw.fill" | "draw.fill.boundary" | "draw.fill.points" | "draw.fill.paint" => {
            "Füllfläche · gesetzt"
        }
        "draw.hatch" => "Schraffur · gesetzt",
        "dim.linear" => "Maß · gesetzt",
        "dim.chain" => "Maßkette · gesetzt",
        "dim.elevation" => "Höhenkote · gesetzt",
        "modify.move" => "Auswahl · verschoben",
        "modify.copy" => "Auswahl · kopiert",
        "modify.mirror" => "Auswahl · gespiegelt",
        "modify.scale" => "Auswahl · skaliert",
        "modify.offset" => "Auswahl · versetzt",
        "modify.explode" => "Objekt · zerlegt",
        "block.insert" => "Block · eingefügt",
        "sheet.viewport_tool" => "Ansichtsfenster · angelegt",
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
                v.hovered.weak_bg_fill = Palette::PANEL_ALT;
                v.hovered.bg_stroke = egui::Stroke::NONE;
                v.active.weak_bg_fill = Palette::PANEL_ALT;
                v.active.bg_stroke = egui::Stroke::NONE;

                match menu.kind {
                    MenuKind::Object(id) => {
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
                        ui.label(theme::bold(typ, 12.0));
                        ui.label(theme::dim(layer));
                        ui.separator();
                        for (label, action) in [
                            ("Verschieben", "modify.move"),
                            ("Kopieren mit Basispunkt", "modify.copy"),
                            ("Drehen", "modify.rotate"),
                            ("Spiegeln", "modify.mirror"),
                        ] {
                            if ui.button(label).clicked() {
                                self.start_tool(action);
                                close = true;
                            }
                        }
                        ui.separator();
                        // Zeichenreihenfolge: Position in der Objektliste
                        ui.label(theme::faint("Reihenfolge"));
                        if ui.button("In den Vordergrund").clicked() {
                            self.order_to(true, vec![id]);
                            close = true;
                        }
                        if ui.button("In den Hintergrund").clicked() {
                            self.order_to(false, vec![id]);
                            close = true;
                        }
                        if ui.button("Vor Element …").clicked() {
                            self.session_mut().select(vec![id]);
                            self.order_pick_start(true);
                            close = true;
                        }
                        if ui.button("Hinter Element …").clicked() {
                            self.session_mut().select(vec![id]);
                            self.order_pick_start(false);
                            close = true;
                        }
                        ui.separator();
                        if ui.button("Zerlegen").clicked() {
                            let _ = self.session_mut().execute(
                                "modify.explode",
                                &serde_json::json!({"ids": [id]}),
                            );
                            self.mark_changed();
                            close = true;
                        }
                        if ui.button("Löschen").clicked() {
                            let _ = self
                                .session_mut()
                                .execute("modify.erase", &serde_json::json!({"ids": [id]}));
                            self.session_mut().clear_selection();
                            self.mark_changed();
                            close = true;
                        }
                        ui.separator();
                        if ui.button("Eigenschaften im Panel").clicked() {
                            self.ui.props_open = true;
                            self.session_mut().select(vec![id]);
                            close = true;
                        }
                    }
                    MenuKind::Canvas => {
                        ui.label(theme::bold("Zeichenfläche", 12.0));
                        ui.separator();
                        let has_clip = !self.ui.clipboard.is_empty();
                        if ui
                            .add_enabled(has_clip, egui::Button::new("Einfügen"))
                            .clicked()
                        {
                            self.start_ghost_paste();
                            close = true;
                        }
                        if ui.button("Alles auswählen").clicked() {
                            let ids: Vec<Id> = self.doc().ents().iter().map(|e| e.id).collect();
                            self.session_mut().select(ids);
                            close = true;
                        }
                        if ui.button("Auswahl aufheben").clicked() {
                            self.session_mut().clear_selection();
                            close = true;
                        }
                        ui.separator();
                        if ui.button("Ansicht einpassen").clicked() {
                            self.fit_view();
                            close = true;
                        }
                    }
                    MenuKind::Chip(kind) => match kind {
                        StatusMenu::Raster => {
                            ui.label(theme::bold("Raster", 12.0));
                            ui.add_space(2.0);
                            let mut show = self.ui.show_grid;
                            if ui.checkbox(&mut show, "Raster anzeigen").changed() {
                                self.ui.show_grid = show;
                                let _ = self.session_mut().execute(
                                    "doc.settings",
                                    &serde_json::json!({"show_grid": show, "snap": {"grid": show}}),
                                );
                            }
                            let mut g = self.doc().settings.snap.grid;
                            if ui.checkbox(&mut g, "Rasterfang").changed() {
                                self.set_snap("grid", g);
                            }
                            let step = self.doc().settings.snap.grid_step;
                            ui.label(theme::dim(format!("Rasterweite: {:.0} mm", step)));
                            ui.separator();
                            if ui.button("Weitere Einstellungen …").clicked() {
                                self.ui.settings_open = true;
                                close = true;
                            }
                        }
                        StatusMenu::Ortho => {
                            ui.label(theme::bold("Ausrichtung", 12.0));
                            ui.add_space(2.0);
                            let mut o = self.doc().settings.snap.ortho;
                            if ui.checkbox(&mut o, "Ortho (0°/90°)").changed() {
                                self.set_snap("ortho", o);
                            }
                            let mut p = self.doc().settings.snap.polar;
                            if ui.checkbox(&mut p, "Polar").changed() {
                                self.set_snap("polar", p);
                            }
                            let st = self.doc().settings.snap.polar_step;
                            let tol = self.doc().settings.snap.polar_tolerance;
                            ui.label(theme::dim(format!(
                                "Schrittweite {:.0}°, Fenster ±{:.0}°",
                                st, tol
                            )));
                            ui.separator();
                            if ui.button("Polarwinkel ändern …").clicked() {
                                self.ui.settings_open = true;
                                close = true;
                            }
                        }
                        StatusMenu::Fang => {
                            ui.label(theme::bold("Fangmodi", 12.0));
                            ui.add_space(2.0);
                            let s0 = self.doc().settings.snap.clone();
                            for (label, key, val) in [
                                ("Endpunkt", "endpoint", s0.endpoint),
                                ("Wandkanten", "wall_edges", s0.wall_edges),
                                ("Mittelpunkt", "midpoint", s0.midpoint),
                                ("Zentrum", "center", s0.center),
                                ("Schnittpunkt", "intersection", s0.intersection),
                                ("Lot", "perpendicular", s0.perpendicular),
                                ("Nächster Punkt", "nearest", s0.nearest),
                            ] {
                                let mut v = val;
                                if ui.checkbox(&mut v, label).changed() {
                                    self.set_snap(key, v);
                                }
                            }
                            ui.separator();
                            if ui.button("Alle an").clicked() {
                                for k in [
                                    "endpoint",
                                    "wall_edges",
                                    "midpoint",
                                    "center",
                                    "intersection",
                                    "perpendicular",
                                    "nearest",
                                ] {
                                    self.set_snap(k, true);
                                }
                                close = true;
                            }
                            if ui.button("Alle aus").clicked() {
                                for k in [
                                    "endpoint",
                                    "wall_edges",
                                    "midpoint",
                                    "center",
                                    "intersection",
                                    "perpendicular",
                                    "nearest",
                                ] {
                                    self.set_snap(k, false);
                                }
                                close = true;
                            }
                            // Ortho und Polar stehen seit T0043 hier, nicht mehr
                            // als eigener Chip in der unteren Leiste.
                            ui.separator();
                            ui.label(theme::bold("Ausrichtung", 12.0));
                            let mut o = self.doc().settings.snap.ortho;
                            if ui.checkbox(&mut o, "Ortho (0°/90°)").changed() {
                                self.set_snap("ortho", o);
                            }
                            let mut p = self.doc().settings.snap.polar;
                            if ui.checkbox(&mut p, "Polar").changed() {
                                self.set_snap("polar", p);
                            }
                            let st = self.doc().settings.snap.polar_step;
                            let tol = self.doc().settings.snap.polar_tolerance;
                            ui.label(theme::dim(format!(
                                "Schrittweite {st:.0}°, Fenster ±{tol:.0}°"
                            )));
                        }
                        StatusMenu::Staerke => {
                            ui.label(theme::bold("Linienstärken", 12.0));
                            ui.add_space(2.0);
                            let mut m = if self.ui.lineweights_scaled { 1 } else { 0 };
                            if ui
                                .radio_value(&mut m, 0, "px — bildschirmkonstant")
                                .clicked()
                            {
                                self.ui.lineweights = true;
                                self.ui.lineweights_scaled = false;
                                close = true;
                            }
                            if ui
                                .radio_value(&mut m, 1, "DPI — wie im PDF (zoombhängig)")
                                .clicked()
                            {
                                self.ui.lineweights = true;
                                self.ui.lineweights_scaled = true;
                                close = true;
                            }
                            ui.separator();
                            ui.label(theme::dim(
                                "In beiden Modi bleiben die Linienstärken sichtbar.",
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Leistungsanzeige (T0068): Bild beginnen — Abstand zum vorigen Bild und
        // die Abschnittsmarken messen. Das läuft immer mit; ein paar
        // Uhrablesungen je Bild kosten nichts.
        self.perf.bild_beginnen();
        self.ui(ctx);
        self.draw_menu(ctx);
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
        self.perf.bild_beenden();
}

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.prefs.show_layers = self.ui.layers_open;
        self.prefs.show_properties = self.ui.props_open;
        self.prefs.show_lineweights = self.ui.lineweights;
        self.prefs.lineweights_scaled = self.ui.lineweights_scaled;
        let _ = self.prefs.save();
    }
}

/// Dateiendungen für Dialoge.
pub fn ext_for(mode: FileMode) -> &'static str {
    match mode {
        FileMode::Open | FileMode::SaveJjcad => "jjcad",
        FileMode::ImportDxf => "dxf",
        FileMode::ImportPdf => "pdf",
        FileMode::ExportPdf => "pdf",
        FileMode::ExportDxf => "dxf",
        FileMode::ExportPng => "png",
    }
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
        EntityKind::Polyline { .. } => "polyline",
        EntityKind::Spline { .. } => "spline",
        EntityKind::Fill { .. } => "fill",
        EntityKind::Volume { .. } => "volume",
        EntityKind::Text { .. } => "text",
        EntityKind::Dim { .. } | EntityKind::DimChain { .. } => "dim",
        EntityKind::Elevation { .. } => "elevation",
        EntityKind::Wall { .. } => "wall",
        EntityKind::Door { .. } => "door",
        EntityKind::Window { .. } => "window",
        EntityKind::Stair { .. } => "stair",
        EntityKind::BlockRef { .. } => "block",
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

    /// Geteilte Schaltfläche für Rechteck und Ellipse.
    ///
    /// Der Hauptknopf startet das Werkzeug im zuletzt gewählten Modus, der
    /// Pfeil öffnet die beiden Einfügemodi. Ohne dieses Menü war der diagonale
    /// Modus nur im Eigenschaften-Panel zu finden — und dort erst, wenn das
    /// Werkzeug schon lief.
    fn einfuege_group_button(&mut self, ui: &mut egui::Ui, tool_id: &'static str) {
        let ellipse = tool_id == "draw.ellipse";
        let (icon, label) = if ellipse {
            ("ellipse", "Ellipse")
        } else {
            ("rect", "Rechteck")
        };
        let cur = self.options.einfuegemodus(tool_id);
        let active = self.tool_id() == Some(tool_id);
        let (main, arrow) = widgets::ribbon_split_button(ui, icon, label, active);
        let popup_id = ui.make_persistent_id(("einfuegepop", tool_id));
        let now = ui.input(|i| i.time);
        if arrow.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
            ui.memory_mut(|m| m.data.insert_temp(popup_id.with("opened"), now));
        } else if main.clicked() {
            self.start_tool(tool_id);
        }
        // Prüfhilfe: Menü für einen Screenshot offen halten (nur mit Env-Variable)
        if let Ok(welches) = std::env::var("JJCAD_DEMO_MENU") {
            let passt = match welches.as_str() {
                "ellipse" => ellipse,
                "rect" | "rechteck" => !ellipse,
                _ => false,
            };
            if passt {
                ui.memory_mut(|m| m.open_popup(popup_id));
            }
        }
        // Der Klick, der das Menü öffnet, darf nicht sofort einen Eintrag wählen
        let geoeffnet = ui
            .memory(|m| m.data.get_temp::<f64>(popup_id.with("opened")).unwrap_or(0.0));
        let bereit = now - geoeffnet > 0.25;
        let mut gewaehlt: Option<Einfuegemodus> = None;
        egui::popup_below_widget(
            ui,
            popup_id,
            &arrow,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(230.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                for m in Einfuegemodus::ALL {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width().max(226.0), 26.0),
                        egui::Sense::click(),
                    );
                    if resp.hovered() || m == cur {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(4.0),
                            Palette::PANEL_ALT,
                        );
                    }
                    let farbe = if m == cur { Palette::ACCENT } else { Palette::TEXT };
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 10.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        m.label(tool_id),
                        egui::FontId::proportional(12.5),
                        farbe,
                    );
                    if bereit && resp.clicked() {
                        gewaehlt = Some(m);
                    }
                }
            },
        );
        if let Some(m) = gewaehlt {
            self.waehle_einfuegemodus(tool_id, m);
            ui.memory_mut(|mm| mm.close_popup());
        }
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
                            Palette::PANEL_ALT,
                        );
                    }
                    crate::icons::paint(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::Pos2::new(rect.left() + 16.0, rect.center().y),
                            egui::Vec2::splat(18.0),
                        ),
                        m.icon(hatch),
                        if m == cur { Palette::ACCENT } else { Palette::TEXT },
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        m.label(),
                        egui::FontId::proportional(12.5),
                        if m == cur { Palette::ACCENT } else { Palette::TEXT },
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
    fn copy_group_button(&mut self, ui: &mut egui::Ui) {
        let cur = self.ui.copy_input;
        let aktiv = self.tool_id() == Some(cur.action());
        let (main, arrow) = widgets::ribbon_split_button(ui, cur.icon(), "Kopieren", aktiv);
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
                            Palette::PANEL_ALT,
                        );
                    }
                    crate::icons::paint(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::Pos2::new(rect.left() + 16.0, rect.center().y),
                            egui::Vec2::splat(18.0),
                        ),
                        m.icon(),
                        if m == cur { Palette::ACCENT } else { Palette::TEXT },
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        m.label(),
                        egui::FontId::proportional(12.5),
                        if m == cur { Palette::ACCENT } else { Palette::TEXT },
                    );
                    ui.painter().text(
                        egui::Pos2::new(rect.right() - 8.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        m.key_label(),
                        egui::FontId::proportional(11.0),
                        Palette::TEXT_FAINT,
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
        self.mitteilung("Verschieben · Basispunkt klicken");
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
            self.mark_changed();
            self.mitteilung("Auswahl · verschoben");
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
        self.mitteilung("Verschieben · abgebrochen");
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
        let ids: Vec<&str> = RIBBON_BEARBEITEN.iter().map(|(id, _, _)| *id).collect();
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
        for (_, icon, label) in RIBBON_BEARBEITEN {
            assert!(!icon.is_empty() && !label.is_empty());
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
    //! T0056: Die Tastatur in drei Schichten (`docs/TASTEN.md`).
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
        let start = app.current_view().center;
        app.prefs.pan_speed = 1.0;
        app.run_action(&app.ctx.clone(), "view.pan_right");
        let weg_eins = (app.current_view().center.x - start.x).abs();

        let mut app = app_mit_zeichnung();
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

    /// Ein einzelner Druck (Hotbar, Kürzel) rückt die Ansicht einen Schritt
    /// weiter — die Funktion ist auch ohne gehaltene Taste benutzbar.
    #[test]
    fn schwenkfunktion_rueckt_einen_schritt() {
        let mut app = app_mit_zeichnung();
        let vorher = app.current_view().center;
        app.run_action(&app.ctx.clone(), "view.pan_right");
        let nachher = app.current_view().center;
        assert!(
            nachher.x > vorher.x + 1.0 && (nachher.y - vorher.y).abs() < 1.0,
            "„Ansicht rechts“ muss nach rechts schieben: {vorher:?} → {nachher:?}"
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

    /// Ein Druck auf eine Taste mit fester Kombination (Strg+S) bleibt beim
    /// Speichern — die Funktion lässt sich nicht auf einen Buchstaben umlegen.
    #[test]
    fn kombinationen_bleiben_fest() {
        let mut app = App::with_ctx(egui::Context::default());
        app.prefs.bind("file.save", "k");
        assert_eq!(app.prefs.key_for("file.save").as_deref(), Some("ctrl+s"));
        assert_eq!(app.prefs.action_for_key("k").as_deref(), Some("draw.circle"));
        assert!(!app.prefs.belegung_aenderbar("file.save"));
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

    #[test]
    fn action_steht_im_reiter_ansicht_mit_f11() {
        let a = actions::action("view.fullscreen").expect("Action view.fullscreen fehlt");
        assert_eq!(a.label, "Vollbild");
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
            other => actions::action(other).map(|d| d.label).unwrap_or("?").to_string(),
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
                h, HINWEIS_UNBEKANNT,
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

