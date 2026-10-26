//! Dokumentmodell: Zeichnung, Layer, Entitäten, Blöcke, Blätter.

use crate::geom::*;
use crate::style::*;
use crate::units::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Id = u64;

pub const FORMAT_NAME: &str = "jjcad";
pub const FORMAT_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Layer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    pub color: Color,
    /// Linienstärke in mm (Plot).
    #[serde(default = "default_lw")]
    pub line_weight: f64,
    #[serde(default)]
    pub line_type: LineType,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "default_true")]
    pub printable: bool,
}

fn default_lw() -> f64 {
    0.25
}
fn default_true() -> bool {
    true
}

impl Layer {
    pub fn new(id: Id, name: &str, color: Color) -> Self {
        Self {
            id,
            name: name.to_string(),
            color,
            line_weight: 0.25,
            line_type: LineType::Solid,
            visible: true,
            locked: false,
            printable: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Entitäten
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DimAxis {
    #[default]
    /// Waagerecht gemessen
    X,
    /// Senkrecht gemessen
    Y,
    /// Parallel zur Messstrecke
    Aligned,
}

impl DimAxis {
    pub const ALL: [DimAxis; 3] = [DimAxis::X, DimAxis::Y, DimAxis::Aligned];
    pub fn label(self) -> &'static str {
        match self {
            DimAxis::X => "Waagerecht",
            DimAxis::Y => "Senkrecht",
            DimAxis::Aligned => "Parallel",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DoorKind {
    #[default]
    Hinge,
    Sliding,
    Double,
    Opening,
}

impl DoorKind {
    pub const ALL: [DoorKind; 4] = [
        DoorKind::Hinge,
        DoorKind::Sliding,
        DoorKind::Double,
        DoorKind::Opening,
    ];
    pub fn label(self) -> &'static str {
        match self {
            DoorKind::Hinge => "Drehtür",
            DoorKind::Sliding => "Schiebetür",
            DoorKind::Double => "Zweiflügelig",
            DoorKind::Opening => "Öffnung",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StairDir {
    #[default]
    Up,
    Down,
    Both,
}

impl StairDir {
    pub const ALL: [StairDir; 3] = [StairDir::Up, StairDir::Down, StairDir::Both];
    pub fn label(self) -> &'static str {
        match self {
            StairDir::Up => "Aufwärts",
            StairDir::Down => "Abwärts",
            StairDir::Both => "Beide",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignH {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignV {
    Top,
    #[default]
    Middle,
    Bottom,
}

/// Alle Maße in Millimetern, Winkel im Bogenmaß.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityKind {
    Point {
        p: Vec2,
    },
    Line {
        a: Vec2,
        b: Vec2,
    },
    Circle {
        c: Vec2,
        r: f64,
    },
    Arc {
        c: Vec2,
        r: f64,
        a0: f64,
        a1: f64,
    },
    Ellipse {
        c: Vec2,
        rx: f64,
        ry: f64,
        rot: f64,
    },
    Polyline {
        verts: Vec<Vertex>,
        closed: bool,
    },
    /// Füllfläche: Muster + Flächenfarbe + optionale Randlinie.
    ///
    /// Der einzige gefüllte Objekttyp — die frühere Schraffur (Typ
    /// `"hatch"`) wird beim Laden migriert (siehe `persist::from_json`).
    Fill {
        /// Geschlossene Kontur (mm, wie eingegeben).
        boundary: Vec<Vec2>,
        /// Inseln in der Fläche (z. B. Schornstein, Treppe, Pfeiler) — werden
        /// ausgespart, nicht übermalt. Leer bei Flächen ohne Insel und in
        /// Dateien von vor diesem Feld (`serde(default)`).
        #[serde(default)]
        holes: Vec<Vec<Vec2>>,
        #[serde(default)]
        pattern: FillPattern,
        /// Drehung des Musters (Bogenmaß, 0 = waagerecht)
        #[serde(default)]
        pattern_angle: f64,
        /// Musterabstand bzw. Rasterweite in mm
        #[serde(default = "default_pattern_scale")]
        pattern_scale: f64,
        /// Umrandung entlang der Umgrenzung zeichnen (Farbe aus `style.color`).
        /// Vorgabe je Erstellmodus: Punkte setzen an, Kontur anklicken und
        /// Farbeimer aus; im Eigenschaften-Panel umschaltbar.
        #[serde(default)]
        border: bool,
    },
    Text {
        p: Vec2,
        text: String,
        height: f64,
        #[serde(default)]
        angle: f64,
        #[serde(default)]
        align: TextAlignH,
        #[serde(default)]
        valign: TextAlignV,
        #[serde(default)]
        bold: bool,
    },
    /// Einzelmaß / Maßkette (linear).
    Dim {
        a: Vec2,
        b: Vec2,
        /// Punkt auf der Maßlinie (bestimmt Lage der Maßlinie).
        line_pos: Vec2,
        #[serde(default)]
        axis: DimAxis,
        /// Maßtext überschreiben (z. B. "2,50").
        #[serde(default)]
        override_text: Option<String>,
    },
    /// Maßkette über mehrere Punkte mit gemeinsamer Maßlinie.
    DimChain {
        pts: Vec<Vec2>,
        /// Punkt auf der Maßlinie.
        line_pos: Vec2,
        #[serde(default)]
        axis: DimAxis,
    },
    Elevation {
        p: Vec2,
        /// Höhe in mm über ±0,00.
        z: f64,
        #[serde(default)]
        angle: f64,
        #[serde(default)]
        override_text: Option<String>,
    },
    Wall {
        a: Vec2,
        b: Vec2,
        thickness: f64,
        #[serde(default = "default_wall_height")]
        height: f64,
        #[serde(default)]
        material: String,
        /// Schraffur der Wandfläche (None = Vollfläche in der Füllfarbe)
        #[serde(default)]
        hatch: Option<FillPattern>,
        /// Ausrichtung des Wandkörpers zur Achse
        #[serde(default)]
        align: crate::style::WallAlign,
    },
    Door {
        center: Vec2,
        /// Richtung der Wand (Öffnungsrichtung der Wand).
        angle: f64,
        width: f64,
        #[serde(default)]
        kind: DoorKind,
        /// +1 = Anschlag rechts, -1 = links
        #[serde(default = "default_one")]
        hinge: f64,
        /// Öffnet zur positiven Normalen der Wand
        #[serde(default = "default_true")]
        swing_in: bool,
        /// Lage in der Wanddicke (links/mittig/rechts)
        #[serde(default)]
        align: crate::style::WallAlign,
        /// Wand, in der die Tür sitzt (für Öffnungen).
        #[serde(default)]
        host: Option<Id>,
        /// Lage längs der Wand in mm ab Wandanfang — die Tür folgt der Wand.
        #[serde(default)]
        at: Option<f64>,
    },
    Window {
        center: Vec2,
        angle: f64,
        width: f64,
        /// Lage in der Wanddicke (links/mittig/rechts)
        #[serde(default)]
        align: crate::style::WallAlign,
        #[serde(default = "default_sill")]
        sill: f64,
        #[serde(default = "default_window_height")]
        height: f64,
        #[serde(default)]
        host: Option<Id>,
        /// Lage längs der Wand in mm ab Wandanfang — das Fenster folgt der Wand.
        #[serde(default)]
        at: Option<f64>,
    },
    Stair {
        a: Vec2,
        b: Vec2,
        width: f64,
        #[serde(default = "default_steps")]
        steps: u32,
        #[serde(default)]
        direction: StairDir,
    },
    BlockRef {
        block: String,
        pos: Vec2,
        #[serde(default)]
        rot: f64,
        #[serde(default = "default_scale")]
        scale: Vec2,
        #[serde(default)]
        mirror: bool,
    },
}

fn default_pattern_scale() -> f64 {
    100.0
}
fn default_wall_height() -> f64 {
    2750.0
}
fn default_one() -> f64 {
    1.0
}
fn default_sill() -> f64 {
    900.0
}
fn default_window_height() -> f64 {
    1250.0
}
fn default_steps() -> u32 {
    16
}
fn default_scale() -> Vec2 {
    v(1.0, 1.0)
}

impl EntityKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            EntityKind::Point { .. } => "Punkt",
            EntityKind::Line { .. } => "Linie",
            EntityKind::Circle { .. } => "Kreis",
            EntityKind::Arc { .. } => "Bogen",
            EntityKind::Ellipse { .. } => "Ellipse",
            EntityKind::Polyline { .. } => "Polylinie",
            EntityKind::Fill { .. } => "Füllfläche",
            EntityKind::Text { .. } => "Text",
            EntityKind::Dim { .. } => "Maß",
            EntityKind::DimChain { .. } => "Maßkette",
            EntityKind::Elevation { .. } => "Höhenkote",
            EntityKind::Wall { .. } => "Wand",
            EntityKind::Door { .. } => "Tür",
            EntityKind::Window { .. } => "Fenster",
            EntityKind::Stair { .. } => "Treppe",
            EntityKind::BlockRef { .. } => "Block",
        }
    }

    pub fn is_architecture(&self) -> bool {
        matches!(
            self,
            EntityKind::Wall { .. }
                | EntityKind::Door { .. }
                | EntityKind::Window { .. }
                | EntityKind::Stair { .. }
        )
    }

    pub fn is_dimension(&self) -> bool {
        matches!(
            self,
            EntityKind::Dim { .. } | EntityKind::DimChain { .. } | EntityKind::Elevation { .. }
        )
    }
}

/// Weltkoordinaten-Epsilon für „derselbe Punkt" (Millimeter).
///
/// Ein Schlusspunkt innerhalb dieser Strecke vom Startpunkt gilt als Startpunkt:
/// die Polylinie ist dann geschlossen. Der Wert ist bewusst klein — er fängt
/// Rundungsreste aus der Tippeingabe ab, nicht eine bewusst gesetzte Kurzstrecke.
pub const POINT_EPS_MM: f64 = 0.5;

/// Fertige Punktfolge für einen Polygonzug.
///
/// Doppelte Stützpunkte fallen weg (zwei Klicks auf dieselbe Stelle erzeugen
/// sonst eine Strecke der Länge null). Liegt der Schlusspunkt innerhalb von
/// `tol` auf dem Startpunkt, wird er nicht gespeichert: der Zug gilt als
/// geschlossen und `closed` wird gesetzt — der doppelte Endpunkt existiert im
/// Modell nicht. Offene Züge bleiben unverändert offen; unter drei Punkten
/// umschließt nichts eine Fläche, dann bleibt der Zug offen.
pub fn polyline_points(pts: &[Vec2], closed: bool, tol: f64) -> (Vec<Vec2>, bool) {
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().map(|q| q.dist(*p) <= tol).unwrap_or(false) {
            continue;
        }
        out.push(*p);
    }
    let mut geschlossen = closed;
    if out.len() >= 3 {
        let first = out[0];
        if out.last().map(|q| q.dist(first) <= tol).unwrap_or(false) {
            out.pop();
            geschlossen = true;
        }
    }
    if out.len() < 3 {
        geschlossen = false;
    }
    (out, geschlossen)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EntityStyle {
    /// None = ByLayer
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub line_type: Option<LineType>,
    #[serde(default)]
    pub line_weight: Option<f64>,
    /// Füllung (z. B. Wand-Poché); None = ByLayer bzw. kein Fill
    #[serde(default)]
    pub fill: Option<Color>,
}

impl Default for EntityStyle {
    fn default() -> Self {
        Self {
            color: None,
            line_type: None,
            line_weight: None,
            fill: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub id: Id,
    pub kind: EntityKind,
    #[serde(default)]
    pub layer: Id,
    #[serde(default)]
    pub style: EntityStyle,
    /// In Blöcken: "ByBlock" für Farbe/Linie/Stärke
    #[serde(default)]
    pub by_block: bool,
}

impl Entity {
    pub fn new(id: Id, layer: Id, kind: EntityKind) -> Self {
        Self {
            id,
            kind,
            layer,
            style: EntityStyle::default(),
            by_block: false,
        }
    }

    pub fn line(id: Id, layer: Id, a: Vec2, b: Vec2) -> Self {
        Self::new(id, layer, EntityKind::Line { a, b })
    }
}

// ---------------------------------------------------------------------------
// Blöcke
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlockSource {
    /// Mit dem Programm geliefert, schreibgeschützt
    #[default]
    System,
    /// In der Zeichnung definiert
    Project,
    /// Benutzerbibliothek (profilweit)
    User,
}

impl BlockSource {
    pub fn label(self) -> &'static str {
        match self {
            BlockSource::System => "System",
            BlockSource::Project => "Projekt",
            BlockSource::User => "Benutzer",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockDef {
    /// Stabile, namensraum-präfixierte ID, z. B. "sys:wc" oder "prj:3f2a".
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub source: BlockSource,
    #[serde(default)]
    pub description: String,
    pub entities: Vec<Entity>,
    /// Optionales eigenes Icon (PNG-Pfad), überschreibt die automatische Vorschau.
    #[serde(default)]
    pub icon: Option<String>,
    /// Einfügebasispunkt (meist 0,0)
    #[serde(default)]
    pub base: Vec2,
}

impl BlockDef {
    pub fn bbox(&self) -> BBox {
        let scratch = Document::blank();
        let mut b = BBox::EMPTY;
        for e in &self.entities {
            b.union(crate::render::entity_bbox(&scratch, e));
        }
        b
    }
}

// ---------------------------------------------------------------------------
// Blätter / Layout
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Viewport {
    pub id: Id,
    pub name: String,
    /// Papierkoordinaten in mm, Ursprung unten links.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Weltkoordinate in der Mitte des Fensters.
    pub center: Vec2,
    /// Maßstab des Fensters (1:scale).
    pub scale: f64,
    #[serde(default)]
    pub show_frame: bool,
    #[serde(default)]
    pub locked: bool,
    /// Nur diese Layer zeigen (None = alle sichtbaren)
    #[serde(default)]
    pub layers: Option<Vec<Id>>,
}

impl Viewport {
    pub fn rect(&self) -> BBox {
        BBox::new(v(self.x, self.y), v(self.x + self.w, self.y + self.h))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sheet {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub paper: PaperSize,
    #[serde(default = "default_true")]
    pub landscape: bool,
    /// Blattmaßstab (Anzeige im Schriftfeld)
    #[serde(default = "default_hundred")]
    pub scale: f64,
    #[serde(default = "default_true")]
    pub frame: bool,
    #[serde(default)]
    pub show_title_block: bool,
    #[serde(default)]
    pub viewports: Vec<Viewport>,
    /// Zeichnungselemente im Papierraum (Schriftfeld etc.)
    #[serde(default)]
    pub entities: Vec<Entity>,
}

fn default_hundred() -> f64 {
    100.0
}

impl Sheet {
    pub fn size(&self) -> (f64, f64) {
        self.paper.dims(self.landscape)
    }
    pub fn bbox(&self) -> BBox {
        let (w, h) = self.size();
        BBox::new(v(0.0, 0.0), v(w, h))
    }
    pub fn viewport(&self, id: Id) -> Option<&Viewport> {
        self.viewports.iter().find(|v| v.id == id)
    }
}

// ---------------------------------------------------------------------------
// Einstellungen
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SnapSettings {
    #[serde(default = "default_true")]
    pub endpoint: bool,
    #[serde(default = "default_true")]
    pub midpoint: bool,
    #[serde(default = "default_true")]
    pub center: bool,
    #[serde(default = "default_true")]
    pub intersection: bool,
    #[serde(default = "default_true")]
    pub perpendicular: bool,
    #[serde(default = "default_true")]
    pub nearest: bool,
    #[serde(default = "default_true")]
    pub grid: bool,
    #[serde(default = "default_true")]
    pub ortho: bool,
    #[serde(default = "default_true")]
    pub polar: bool,
    /// Punktfang auf den umgrenzenden Linien der Wandkörper (Kanten, Ecken,
    /// Laibungen) — unabhängig von den übrigen Fangarten schaltbar.
    #[serde(default = "default_true")]
    pub wall_edges: bool,
    /// Rasterweite in mm
    #[serde(default = "default_grid")]
    pub grid_step: f64,
    /// Fangradius in Pixeln
    #[serde(default = "default_snap_px")]
    pub tolerance_px: f64,
    /// Schrittweite des Polarfangs in Grad (Architektur: 45°)
    #[serde(default = "default_polar_step")]
    pub polar_step: f64,
    /// Winkel-Fenster des Polarfangs in Grad: nur innerhalb dieser Abweichung rastet er ein
    #[serde(default = "default_polar_tol")]
    pub polar_tolerance: f64,
}

fn default_polar_tol() -> f64 {
    6.0
}

fn default_polar_step() -> f64 {
    45.0
}

fn default_grid() -> f64 {
    100.0
}
fn default_snap_px() -> f64 {
    12.0
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            endpoint: true,
            midpoint: true,
            center: true,
            intersection: true,
            perpendicular: true,
            nearest: true,
            grid: true,
            ortho: false,
            polar: true,
            wall_edges: true,
            grid_step: 100.0,
            tolerance_px: 12.0,
            polar_step: 45.0,
            polar_tolerance: 6.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub input_unit: Unit,
    #[serde(default = "default_dim_unit")]
    pub dim_unit: Unit,
    #[serde(default = "default_two")]
    pub decimals: usize,
    /// Bezugsmaßstab für Papiergrößen von Texten/Bemaßung (1:annotation_scale)
    #[serde(default = "default_hundred")]
    pub annotation_scale: f64,
    #[serde(default)]
    pub snap: SnapSettings,
    #[serde(default = "default_text_height")]
    pub text_height: f64,
    #[serde(default = "default_wall_thickness")]
    pub wall_thickness: f64,
    #[serde(default = "default_door_width")]
    pub door_width: f64,
    #[serde(default = "default_window_width")]
    pub window_width: f64,
    #[serde(default = "default_stair_width")]
    pub stair_width: f64,
    #[serde(default = "default_true")]
    pub show_grid: bool,
    #[serde(default = "default_true")]
    pub show_rulers: bool,
}

fn default_dim_unit() -> Unit {
    Unit::M
}
fn default_two() -> usize {
    2
}
fn default_text_height() -> f64 {
    250.0
}
fn default_wall_thickness() -> f64 {
    240.0
}
fn default_door_width() -> f64 {
    885.0
}
fn default_window_width() -> f64 {
    1200.0
}
fn default_stair_width() -> f64 {
    1000.0
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            input_unit: Unit::M,
            dim_unit: Unit::M,
            decimals: 2,
            annotation_scale: 100.0,
            snap: SnapSettings::default(),
            text_height: 250.0,
            wall_thickness: 240.0,
            door_width: 885.0,
            window_width: 1200.0,
            stair_width: 1000.0,
            show_grid: true,
            show_rulers: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectMeta {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub planner: String,
    #[serde(default)]
    pub plan_no: String,
    #[serde(default)]
    pub plan_title: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub note: String,
    /// Datum ISO (leer = aktuelles Datum)
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub revision: String,
}

// ---------------------------------------------------------------------------
// Dokument
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub project: ProjectMeta,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub current_layer: Id,
    #[serde(default)]
    pub blocks: Vec<BlockDef>,
    #[serde(default)]
    pub entities: Vec<Entity>,
    /// Bearbeitungsraum der Befehlsschicht: `None` = Modell, `Some(blatt)` =
    /// Papierraum (Druckvorschau) dieses Blatts. Reiner Laufzeitzustand — wird
    /// nicht gespeichert, nicht verglichen und nicht mitgeschrieben.
    #[serde(skip)]
    pub space: Option<Id>,
    #[serde(default)]
    pub sheets: Vec<Sheet>,
    #[serde(default)]
    pub dim_style: DimStyle,
    #[serde(default = "default_one_id")]
    pub next_id: Id,
    #[serde(skip)]
    pub modified: bool,
    #[serde(skip)]
    pub path: Option<String>,
}

fn default_one_id() -> Id {
    1
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// Leere Architekturzeichnung mit Standard-Layern, Systemblöcken und A3-Blatt.
    pub fn new() -> Self {
        let mut doc = Self {
            format: FORMAT_NAME.to_string(),
            version: FORMAT_VERSION,
            name: "Unbenannt".to_string(),
            project: ProjectMeta::default(),
            settings: Settings::default(),
            layers: Vec::new(),
            current_layer: 0,
            blocks: Vec::new(),
            entities: Vec::new(),
            space: None,
            sheets: Vec::new(),
            dim_style: DimStyle::default(),
            next_id: 1,
            modified: false,
            path: None,
        };
        for (i, (name, color, lw, lt)) in LAYER_DEFAULTS.iter().enumerate() {
            let mut l = Layer::new(doc.next_id, name, *color);
            l.line_weight = *lw;
            l.line_type = *lt;
            if i == 0 {
                doc.current_layer = l.id;
            }
            doc.next_id += 1;
            doc.layers.push(l);
        }
        doc.blocks = crate::blocks_lib::system_blocks(&mut doc.next_id);
        let sheet = crate::templates::new_sheet(&mut doc, PaperSize::A3, true, "Blatt 1");
        doc.sheets.push(sheet);
        doc
    }

    /// Minimales Dokument ohne Layer/Blöcke (für Geometrie-Hilfsrechnungen).
    pub fn blank() -> Self {
        Self {
            format: FORMAT_NAME.to_string(),
            version: FORMAT_VERSION,
            name: String::new(),
            project: ProjectMeta::default(),
            settings: Settings::default(),
            layers: Vec::new(),
            current_layer: 0,
            blocks: Vec::new(),
            entities: Vec::new(),
            space: None,
            sheets: Vec::new(),
            dim_style: DimStyle::default(),
            next_id: 1,
            modified: false,
            path: None,
        }
    }

    pub fn new_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        self.modified = true;
        id
    }

    pub fn layer(&self, id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn layer_by_name(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name.eq_ignore_ascii_case(name))
    }

    pub fn layer_id_by_name(&self, name: &str) -> Option<Id> {
        self.layer_by_name(name).map(|l| l.id)
    }

    /// Layer für einen Namen holen oder neu anlegen.
    pub fn ensure_layer(&mut self, name: &str, color: Color) -> Id {
        if let Some(l) = self.layer_by_name(name) {
            return l.id;
        }
        let id = self.new_id();
        let mut l = Layer::new(id, name, color);
        l.line_weight = 0.25;
        self.layers.push(l);
        id
    }

    pub fn block(&self, id: &str) -> Option<&BlockDef> {
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn block_mut(&mut self, id: &str) -> Option<&mut BlockDef> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    pub fn sheet(&self, id: Id) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    pub fn sheet_mut(&mut self, id: Id) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }

    // ---- Bearbeitungsraum (Modell / Papierraum) ---------------------------
    // Die Befehlsschicht arbeitet immer auf "den Objekten des aktiven Raums".
    // Im Modell ist das `entities`, im Papierraum `sheets[..].entities`.
    // Dadurch gilt jeder Befehl (Zeichnen, Ändern, Reihenfolge, Auswahl) in
    // beiden Räumen gleich — ohne zweiten Weg an der Action-Schicht vorbei.

    /// Objekte des aktiven Bearbeitungsraums.
    pub fn ents(&self) -> &[Entity] {
        match self.space.and_then(|sid| self.sheet(sid)) {
            Some(s) => &s.entities,
            None => &self.entities,
        }
    }

    /// Objekte des aktiven Bearbeitungsraums, veränderbar.
    pub fn ents_mut(&mut self) -> &mut Vec<Entity> {
        let Some(sid) = self.space.filter(|sid| self.sheets.iter().any(|s| s.id == *sid)) else {
            return &mut self.entities;
        };
        &mut self
            .sheets
            .iter_mut()
            .find(|s| s.id == sid)
            .expect("Blatt eben geprüft")
            .entities
    }

    /// Blatt des aktiven Papierraums (None = Modell).
    pub fn space_sheet(&self) -> Option<&Sheet> {
        self.space.and_then(|sid| self.sheet(sid))
    }

    /// Bearbeitungsraum wechseln. Ein unbekanntes Blatt fällt auf das Modell
    /// zurück — nie auf ein Blatt, das es nicht gibt.
    pub fn set_space(&mut self, space: Option<Id>) {
        self.space = space.filter(|sid| self.sheets.iter().any(|s| s.id == *sid));
    }

    /// Ist der Papierraum (Druckvorschau) der aktive Raum?
    pub fn in_paper_space(&self) -> bool {
        self.space_sheet().is_some()
    }

    /// Rasterweite des aktiven Raums in mm. Der Papierraum ist in Blatt-
    /// Millimetern gezeichnet, dort ist 10 mm die brauchbare Rasterweite.
    pub fn grid_step(&self) -> f64 {
        if self.in_paper_space() {
            10.0
        } else {
            self.settings.snap.grid_step
        }
    }

    pub fn add_entity(&mut self, mut e: Entity) -> Id {
        if e.id == 0 {
            e.id = self.new_id();
        }
        if e.layer == 0 {
            e.layer = self.current_layer;
        }
        let id = e.id;
        self.ents_mut().push(e);
        self.modified = true;
        id
    }

    pub fn remove_entity(&mut self, id: Id) -> Option<Entity> {
        let ents = self.ents_mut();
        let entfernt = ents.iter().position(|e| e.id == id).map(|pos| ents.remove(pos));
        if entfernt.is_some() {
            self.modified = true;
        }
        entfernt
    }

    pub fn entity(&self, id: Id) -> Option<&Entity> {
        self.ents().iter().find(|e| e.id == id)
    }

    pub fn entity_mut(&mut self, id: Id) -> Option<&mut Entity> {
        self.ents_mut().iter_mut().find(|e| e.id == id)
    }

    /// Alle sichtbaren Layer-IDs
    pub fn visible_layers(&self) -> Vec<Id> {
        self.layers
            .iter()
            .filter(|l| l.visible)
            .map(|l| l.id)
            .collect()
    }

    pub fn effective_color(&self, e: &Entity) -> Color {
        if let Some(c) = e.style.color {
            return c;
        }
        self.layer(e.layer).map(|l| l.color).unwrap_or(Color::BLACK)
    }

    pub fn effective_line_type(&self, e: &Entity) -> LineType {
        if let Some(lt) = e.style.line_type {
            return lt;
        }
        self.layer(e.layer)
            .map(|l| l.line_type)
            .unwrap_or(LineType::Solid)
    }

    pub fn effective_line_weight(&self, e: &Entity) -> f64 {
        if let Some(lw) = e.style.line_weight {
            return lw;
        }
        self.layer(e.layer).map(|l| l.line_weight).unwrap_or(0.25)
    }

    /// Gesamt-Bounding-Box aller Modellobjekte.
    pub fn bbox(&self) -> BBox {
        let mut b = BBox::EMPTY;
        for e in &self.entities {
            b.union(crate::render::entity_bbox(self, e));
        }
        b
    }

    /// Wand, in der eine Tür/ein Fenster sitzt.
    pub fn host_wall(&self, id: Option<Id>) -> Option<&Entity> {
        let id = id?;
        self.entities
            .iter()
            .find(|e| e.id == id && matches!(e.kind, EntityKind::Wall { .. }))
    }

    /// Öffnungen (Türen/Fenster) für eine Wand.
    pub fn wall_openings(&self, wall_id: Id) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for e in &self.entities {
            match &e.kind {
                EntityKind::Door {
                    center,
                    angle,
                    width,
                    host: Some(h),
                    ..
                }
                | EntityKind::Window {
                    center,
                    angle,
                    width,
                    host: Some(h),
                    ..
                } if *h == wall_id => {
                    out.push((*angle, *width));
                    let _ = center;
                }
                _ => {}
            }
        }
        out.into_iter().map(|(_, w)| (0.0, w)).collect()
    }

    /// Text mit dynamischen Feldern auflösen (Maßstab aus den Einstellungen).
    pub fn resolve_text(&self, text: &str) -> String {
        self.resolve_text_scaled(text, None)
    }

    /// Text mit dynamischen Feldern auflösen; `scale` überschreibt {MASSTAB}
    /// (im Papierraum: der Blattmaßstab).
    pub fn resolve_text_scaled(&self, text: &str, scale: Option<f64>) -> String {
        let mut s = text.to_string();
        let today = crate::util::today_iso();
        let today_de = crate::util::iso_to_de(&today);
        s = s.replace("{DATUM}", &today_de);
        s = s.replace("{DATE}", &today_de);
        s = s.replace("{ISO}", &today);
        s = s.replace("{PROJEKT}", &self.project.project);
        s = s.replace("{BAUHERR}", &self.project.client);
        s = s.replace("{PLANER}", &self.project.planner);
        s = s.replace("{PLAN}", &self.project.plan_no);
        s = s.replace("{PLANTITEL}", &self.project.plan_title);
        s = s.replace("{ADRESSE}", &self.project.address);
        s = s.replace("{REVISION}", &self.project.revision);
        s = s.replace("{DATEI}", &self.name);
        let sc = scale.unwrap_or(self.settings.annotation_scale);
        s = s.replace("{MASSTAB}", &crate::units::format_scale(sc));
        s = s.replace("{MASSSTAB}", &crate::units::format_scale(sc));
        s
    }

    /// Alle Objekte, die eine Blockdefinition referenzieren.
    pub fn block_instances(&self, block_id: &str) -> Vec<Id> {
        self.entities
            .iter()
            .filter(|e| matches!(&e.kind, EntityKind::BlockRef { block, .. } if block == block_id))
            .map(|e| e.id)
            .collect()
    }

    /// Prüft, ob Layer `id` gesperrt ist.
    pub fn layer_locked(&self, id: Id) -> bool {
        self.layer(id).map(|l| l.locked || !l.visible).unwrap_or(false)
    }

    /// Kurzfassung für die Statuszeile.
    pub fn summary_short(&self) -> String {
        format!("{} Objekte", self.entities.len())
    }

    pub fn summary(&self) -> String {
        let walls = self
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Wall { .. }))
            .count();
        let doors = self
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Door { .. }))
            .count();
        let windows = self
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Window { .. }))
            .count();
        format!(
            "{} Objekte · {} Wände · {} Türen · {} Fenster · {} Blöcke",
            self.entities.len(),
            walls,
            doors,
            windows,
            self.blocks.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Attribut-Map für Blöcke (zukünftige Nutzung)
// ---------------------------------------------------------------------------

pub type Attributes = BTreeMap<String, String>;
