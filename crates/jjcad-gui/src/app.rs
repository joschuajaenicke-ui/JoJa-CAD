//! Anwendungszustand und Fensteraufbau von JoJa-CAD.

use crate::canvas;
use crate::dialogs::{self, FileDialog, FileMode, TextDialog};
use crate::hotbar;
use crate::inventory;
use crate::panels;
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
use jjcad_core::units::{format_number, format_scale, Unit};
use jjcad_core::{dxf, pdf, raster};
use std::collections::HashMap;
use std::path::PathBuf;

pub const TOOLBAR_H: f32 = 38.0;
pub const RIBBON_H: f32 = 78.0;
pub const HEADER_H: f32 = 38.0;
pub const DOCTABS_H: f32 = 34.0;
pub const STATUS_H: f32 = 26.0;
pub const HOTBAR_H: f32 = 64.0;

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
}

impl Default for View {
    fn default() -> Self {
        Self {
            center: V2::new(5000.0, 4000.0),
            zoom: 0.08,
        }
    }
}

pub struct DocTab {
    pub session: Session,
    pub tab: TabKind,
    pub view_model: View,
    pub sheet_views: HashMap<Id, View>,
    pub rev: u64,
    pub cache: Option<(u64, f64, f64, DisplayList)>,
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
    pub stair_width: f64,
    pub stair_steps: u32,
    pub stair_dir: StairDir,
    pub text_height: f64,
    pub text: String,
    pub dim_axis: Option<DimAxis>,
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
    /// Randlinie entlang des Umrisses
    pub fill_border: bool,
    pub block_id: Option<String>,
    pub block_rot: f64,
    pub block_scale: f64,
}

impl ToolOptions {
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
            stair_width: doc.settings.stair_width,
            stair_steps: 17,
            stair_dir: StairDir::Up,
            text_height: doc.settings.text_height,
            text: String::new(),
            dim_axis: None,
            offset_dist: doc.settings.wall_thickness,
            elevation_z: 0.0,
            fill_pattern: FillPattern::Lines,
            paint_pattern: FillPattern::Solid,
            fill_scale: 100.0,
            fill_angle: std::f64::consts::FRAC_PI_4,
            fill_border: true,
            block_id: None,
            block_rot: 0.0,
            block_scale: 1.0,
        }
    }
}

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
    pub help_open: bool,
    pub about_open: bool,
    pub sheet_dialog: bool,
    pub project_dialog: bool,
    pub text_dialog: Option<TextDialog>,
    pub block_name_dialog: Option<(String, bool)>,
    /// Hotbar-Vorlage: (None = neu, Some(index) = umbenennen), Name
    pub preset_dialog: Option<(Option<usize>, String)>,
    pub file_dialog: Option<FileDialog>,
    pub toast: Option<(String, f64)>,
    pub show_grid: bool,
    pub lineweights: bool,
    /// Linienstärken über den Maßstab skalieren (gedruckte Breite)
    pub lineweights_scaled: bool,
    /// Zieh-Modus für Objekte und Griffe
    pub drag_mode: DragMode,
    /// Zuletzt benutzter Eingabemodus je Button (Fläche / Schraffur)
    pub fill_input: FillInput,
    pub hatch_input: FillInput,
    /// Welcher der beiden Knöpfe das laufende Füllwerkzeug gestartet hat
    /// (None = kein Füllwerkzeug aktiv). Nötig, weil beide Knöpfe dieselbe
    /// Action starten können und sonst beide hervorgehoben wären.
    pub fill_active: Option<bool>,
    /// Offenes Kontextmenü (Objekt, Zeichenfläche oder Chip)
    pub menu: Option<MenuState>,
    /// Auswahl eines Referenzobjekts für "Vor/Hinter Element"
    /// (Some(true) = davor, Some(false) = dahinter)
    pub order_pick: Option<bool>,
    pub bind_slot: Option<usize>,
    pub slot_drag: Option<(usize, String)>,
    pub keymap_selected: Option<String>,
    pub search: String,
    pub new_layer_dialog: Option<String>,
    pub color_popup: Option<Id>,
    pub rename_layer: Option<(Id, String)>,
    pub clipboard: Vec<Entity>,
    pub clipboard_offset: f64,
    pub confirm_new: bool,
    pub confirm_quit: bool,
    pub last_canvas: Rect,
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
            help_open: false,
            about_open: false,
            sheet_dialog: false,
            project_dialog: false,
            text_dialog: None,
            block_name_dialog: None,
            preset_dialog: None,
            file_dialog: None,
            toast: None,
            show_grid: true,
            lineweights: true,
            lineweights_scaled: false,
            drag_mode: DragMode::HoldAndDrag,
            fill_input: FillInput::Points,
            hatch_input: FillInput::Contour,
            fill_active: None,
            menu: None,
            order_pick: None,
            bind_slot: None,
            slot_drag: None,
            keymap_selected: None,
            search: String::new(),
            new_layer_dialog: None,
            color_popup: None,
            rename_layer: None,
            clipboard: Vec::new(),
            clipboard_offset: 20.0,
            confirm_new: false,
            confirm_quit: false,
            last_canvas: Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)),
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

impl FillInput {
    pub const ALL: [FillInput; 3] = [Self::Contour, Self::Points, Self::Paint];

    pub fn action(self) -> &'static str {
        match self {
            Self::Contour => "draw.fill",
            Self::Points => "draw.fill.points",
            Self::Paint => "draw.fill.paint",
        }
    }

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

pub struct App {
    pub tabs: Vec<DocTab>,
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
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Klick-Toleranz: egui verwirft einen Klick, sobald sich der Zeiger zwischen
        // Drücken und Loslassen mehr als 6 px bewegt — beim schnellen Klicken passiert
        // das leicht. 10 px sind für Maus und Touchpad unkritisch.
        cc.egui_ctx
            .options_mut(|o| o.input_options.max_click_dist = 10.0);
        Self::with_ctx(cc.egui_ctx.clone())
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
            new_tab_menu: false,
            started_at: 0.0,
            ctx: ctx.clone(),
            ghost: None,
            grip_drag: None,
            hover_grip: None,
            move_drag: None,
            grip_start_frame: 0.0,
            paint: crate::canvas::PaintPreview::default(),
        };
        app.ui.show_grid = show_grid;
        app.ui.lineweights = lineweights;
        app.ui.lineweights_scaled = lineweights_scaled;
        app.ui.drag_mode = drag_mode;
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
    /// auswählen, zum Prüfen des direkten Ziehens).
    pub fn apply_demo_ui(&mut self) {
        let Ok(spec) = std::env::var("JJCAD_DEMO_UI") else { return };
        for part in spec.split(',') {
            match part.trim() {
                "arch" => self.ui.ribbon = RibbonTab::Architektur,
                "draw" => self.ui.ribbon = RibbonTab::Zeichnen,
                "dim" => self.ui.ribbon = RibbonTab::Bemassung,
                "bloecke" | "blocks" => self.ui.ribbon = RibbonTab::Bloecke,
                "blatt" | "sheet" => self.ui.ribbon = RibbonTab::Blatt,
                "ansicht" => self.ui.ribbon = RibbonTab::Ansicht,
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
                "settings" => self.ui.settings_open = true,
                "help" => self.ui.help_open = true,
                "wall" => self.start_tool("draw.wall"),
                "paint" | "farbeimer" => self.start_tool("draw.fill.paint"),
                "door" => self.start_tool("draw.door"),
                "text" => self.start_tool("draw.text"),
                // Prüfhilfe: Füllwerkzeug über den jeweiligen Knopf starten
                "fillarea" => {
                    let m = self.ui.fill_input;
                    self.start_fill_input(m, false);
                }
                "fillhatch" => {
                    let m = self.ui.hatch_input;
                    self.start_fill_input(m, true);
                }
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
                    // Füllfläche (Muster, Winkel, Abstand, Randlinie) zeigen
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
                "sheetview" => {
                    if let Some(s) = self.doc().sheets.first().map(|s| s.id) {
                        self.tab_mut().tab = TabKind::Sheet(s);
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
                    let ids: Vec<Id> = self.doc().entities.iter().map(|e| e.id).collect();
                    self.start_move_preview(ids);
                }
                // Nur auswählen (für die Prüfung des direkten Ziehens)
                "selall" => {
                    let ids: Vec<Id> = self.doc().entities.iter().map(|e| e.id).collect();
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

    pub fn unit(&self) -> Unit {
        self.doc().settings.input_unit
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.ui.toast = Some((msg.into(), 0.0));
        self.status_msg = self.ui.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default();
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
            self.toast("Keine Auswahl");
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
            self.toast("Zuerst ein Element auswählen");
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
            "file.export_png" => self.open_file_dialog(FileMode::ExportPng),
            "edit.undo" => {
                if self.session_mut().undo() {
                    self.mark_changed();
                    self.toast("Rückgängig");
                }
            }
            "edit.redo" => {
                if self.session_mut().redo() {
                    self.mark_changed();
                    self.toast("Wiederholt");
                }
            }
            "modify.erase" => {
                let ids = self.selection().to_vec();
                if ids.is_empty() {
                    self.start_tool("modify.select");
                    self.toast("Objekte auswählen, dann Entf");
                } else {
                    let _ = self.session_mut().execute("modify.erase", &serde_json::json!({ "ids": ids }));
                    self.mark_changed();
                }
            }
            "modify.explode" | "block.explode" => {
                let ids = self.selection().to_vec();
                if ids.is_empty() {
                    self.toast("Zuerst Blöcke/Bauteile auswählen");
                } else {
                    match self
                        .session_mut()
                        .execute("modify.explode", &serde_json::json!({ "ids": ids }))
                    {
                        Ok(_) => {
                            self.mark_changed();
                            self.toast("Zerlegt");
                        }
                        Err(e) => self.toast(e),
                    }
                }
            }
            "block.create" => {
                if self.selection().is_empty() {
                    self.toast("Zuerst Objekte auswählen");
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
                self.toast(format!("Hotbar: {name}"));
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
                                self.toast(format!("Wiederholt: {}", actions::action(&last).map(|d| d.label).unwrap_or("")));
                            }
                            Err(e) => self.toast(e),
                        }
                    }
                }
            }
            "view.zoom_extents" => self.fit_view(),
            "view.zoom_in" => self.zoom_by(1.25),
            "view.zoom_out" => self.zoom_by(0.8),
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
                            self.toast("Ansichtsfenster hinzugefügt");
                        }
                        Err(e) => self.toast(e),
                    }
                }
            }
            "dim.style" => self.ui.settings_open = true,
            "doc.project" => self.ui.project_dialog = true,
            "select.all" => {
                let ids: Vec<Id> = self.doc().entities.iter().map(|e| e.id).collect();
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
        let Some(def) = actions::action(id) else { return };
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
        self.status_msg = def.hint.to_string();
        if self.ui.bind_slot.is_some() {
            // Aktives Werkzeug mit Slot verknüpfen
            let slot = self.ui.bind_slot.take().unwrap();
            self.bind_slot_action(slot, Some(id.to_string()));
            self.tool = None;
            return;
        }
        // Auswahlmodus bei Aktionen, die eine Auswahl brauchen
        if let Some(t) = &mut self.tool {
            if t.selecting {
                self.status_msg = "Objekte wählen – Enter bestätigt, ESC bricht ab".into();
            }
        }
        if !matches!(
            id,
            "modify.move" | "modify.copy" | "modify.rotate" | "modify.mirror" | "modify.scale" | "modify.offset"
        ) {
            // andere Werkzeuge beenden eine bestehende Auswahl nicht
        }
    }

    /// ESC: Polylinie/Maßkette abschließen statt alles zu verwerfen —
    /// nur ein unfertiger erster Punkt wird verworfen.
    fn escape_pressed(&mut self) {
        let finisheable = self
            .tool
            .as_ref()
            .map(|t| matches!(t.action.as_str(), "draw.polyline" | "dim.chain") && t.pts.len() >= 2)
            .unwrap_or(false);
        if finisheable {
            crate::canvas::finish_tool(self, false);
            self.toast("Abgeschlossen");
            return;
        }
        self.cancel_tool();
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

    pub fn cancel_tool(&mut self) {
        if self.tool.is_some() {
            self.tool = None;
            self.status_msg = "Abgebrochen".into();
        }
        if self.ghost.take().is_some() {
            self.status_msg = "Platzieren abgebrochen".into();
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
        self.toast(format!("Slot {} belegt ({name})", slot + 1));
    }

    // ---- Dateien ----

    pub fn open_file_dialog(&mut self, mode: FileMode) {
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
        self.ui.file_dialog = Some(FileDialog::new(mode, start_dir, &base));
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
                self.toast(format!("Gespeichert: {}", path.display()));
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
                self.toast(format!("Geöffnet: {}", path.file_name().unwrap_or_default().to_string_lossy()));
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
        self.toast("Neue Zeichnung");
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
            Ok(_) => self.toast(format!("PDF exportiert: {}", path.display())),
            Err(e) => self.toast(e),
        }
    }

    pub fn export_dxf_to(&mut self, path: PathBuf) {
        match dxf::export_dxf(self.doc(), &path) {
            Ok(_) => self.toast(format!("DXF exportiert: {}", path.display())),
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
            Ok(_) => self.toast(format!("PNG exportiert: {}", path.display())),
            Err(e) => self.toast(e),
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
                let mut msg = format!("{n} Objekte importiert");
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
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.ui.order_pick.is_some() {
                self.ui.order_pick = None;
                self.toast("Abgebrochen: kein Referenzobjekt gewählt");
            } else if self.ui.bind_slot.is_some() {
                self.ui.bind_slot = None;
                self.toast("Verknüpfen abgebrochen");
            } else {
                self.escape_pressed();
            }
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
                self.copy_selection(false);
            }
            if ev_cut {
                self.copy_selection(true);
            }
            if ev_paste {
                self.start_ghost_paste();
            }
        }
        let mut run: Option<String> = None;

        // Vorschau: drehen, spiegeln, setzen
        if self.ghost.is_some() && !typing {
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.rotate_ghost(if shift { -90.0 } else { 90.0 });
            }
            // Spiegeln: sowohl S als auch F (wie bei Türen/Blöcken üblich)
            if ctx.input(|i| i.key_pressed(egui::Key::S))
                || ctx.input(|i| i.key_pressed(egui::Key::F))
            {
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
            // Einzeltasten über die Belegung
            let keys = [
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
                (egui::Key::V, "v"),
                (egui::Key::W, "w"),
                (egui::Key::X, "x"),
                (egui::Key::Y, "y"),
            ];
            for (key, label) in keys {
                // Einzeltasten gelten nur ohne Umschalt — sonst würde
                // Umschalt+I zusätzlich die Belegung von "i" auslösen.
                if !shift && ctx.input(|i| i.key_pressed(key)) {
                    if let Some(action) = self.prefs.action_for_key(label) {
                        run = Some(action);
                    }
                }
            }
            // Kürzel mit Umschalt, in der Belegung als "shift+i" gespeichert
            // (Umschalt+I: Füllfläche aus einer freien Punktfolge).
            if shift {
                for (key, label) in [(egui::Key::I, "shift+i")] {
                    if ctx.input(|i| i.key_pressed(key)) {
                        if let Some(action) = self.prefs.action_for_key(label) {
                            run = Some(action);
                        }
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
                    if let Some(Some(action)) = self.prefs.slots().get(
                        label.parse::<usize>().unwrap_or(0).saturating_sub(1) % 10,
                    ) {
                        run = Some(action.clone());
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
                let ids = self.selection().to_vec();
                if !ids.is_empty() {
                    let _ = self.session_mut().execute("modify.erase", &serde_json::json!({"ids": ids}));
                    self.session_mut().clear_selection();
                    self.mark_changed();
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

    fn tool_enter(&mut self) {
        if let Some(tool) = self.tool.clone() {
            match tool.action.as_str() {
                "draw.polyline" | "dim.chain" => {
                    if tool.pts.len() >= 2 {
                        canvas::finish_tool(self, true);
                    }
                }
                _ => {
                    if tool.selecting && !self.selection().is_empty() {
                        if let Some(t) = &mut self.tool {
                            t.selecting = false;
                        }
                        self.status_msg = "Basispunkt wählen".into();
                    }
                }
            }
        }
    }

    pub fn copy_selection(&mut self, cut: bool) {
        let ids = self.selection().to_vec();
        let ents: Vec<Entity> = self
            .doc()
            .entities
            .iter()
            .filter(|e| ids.contains(&e.id))
            .cloned()
            .collect();
        if ents.is_empty() {
            return;
        }
        self.ui.clipboard = ents;
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
        self.toast(if cut { "Ausgeschnitten" } else { "Kopiert" });
    }

    pub fn paste_clipboard(&mut self) {
        if self.ui.clipboard.is_empty() {
            return;
        }
        let d = V2::new(self.ui.clipboard_offset, -self.ui.clipboard_offset);
        let mut new_ids = Vec::new();
        let clip = self.ui.clipboard.clone();
        for e in clip {
            let mut ne = jjcad_core::transform::transform_entity(&e, &jjcad_core::geom::Affine::translate(d));
            ne.id = self.doc_mut().new_id();
            new_ids.push(ne.id);
            self.doc_mut().entities.push(ne);
        }
        self.mark_changed();
        self.session_mut().select(new_ids);
        self.ui.clipboard_offset += 20.0;
        self.toast("Eingefügt");
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
            if self.tab().tab != kind {
                self.tab_mut().tab = kind;
                self.tool = None;
                self.session_mut().clear_selection();
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
        }
        if let Some(id) = close_sheet {
            if self.doc().sheets.len() > 1 {
                let _ = self.session_mut().execute("sheet.delete", &serde_json::json!({"sheet": id}));
                if self.tab().tab == TabKind::Sheet(id) {
                    self.tab_mut().tab = TabKind::Model;
                }
                self.tab_mut().sheet_views.remove(&id);
                self.mark_changed();
            } else {
                self.toast("Das letzte Blatt kann nicht geschlossen werden");
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

    fn ribbon(&mut self, ui: &mut egui::Ui) {
        let tab = self.ui.ribbon;
        ui.horizontal(|ui| {
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
                    widgets::vline(ui, RIBBON_H);
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
                    });
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Zwischenablage", |ui, app| {
                        if widgets::ribbon_button(ui, "copy", "Kopieren", false).clicked() {
                            app.copy_selection(false);
                        }
                        if widgets::ribbon_button(ui, "import", "Einfügen", false).clicked() {
                            app.paste_clipboard();
                        }
                        if widgets::ribbon_button(ui, "erase", "Ausschneiden", false).clicked() {
                            app.copy_selection(true);
                        }
                    });
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Projekt", |ui, app| {
                        if widgets::ribbon_button(ui, "info", "Projektdaten", false).clicked() {
                            app.ui.project_dialog = true;
                        }
                        if widgets::ribbon_button(ui, "settings", "Einstellungen", false).clicked() {
                            app.ui.settings_open = true;
                        }
                    });
                    widgets::vline(ui, RIBBON_H);
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
                            ("draw.rect", "rect", "Rechteck"),
                            ("draw.circle", "circle", "Kreis"),
                            ("draw.arc", "arc", "Bogen"),
                        ] {
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Füllen", |ui, app| {
                        // Zwei geteilte Schaltflächen: Fläche und Schraffur.
                        // Der Pfeil öffnet die Eingabemodi, der Knopf startet
                        // den zuletzt benutzten — das Icon zeigt ihn an.
                        app.fill_group_button(ui, false);
                        app.fill_group_button(ui, true);
                    });
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Weitere", |ui, app| {
                        for (id, icon, label) in [
                            ("draw.ellipse", "ellipse", "Ellipse"),
                            ("draw.point", "point", "Punkt"),
                            ("draw.text", "text", "Text"),
                        ] {
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
                    });
                }
                RibbonTab::Aendern => {
                    self.group(ui, "Bearbeiten", |ui, app| {
                        for (id, icon, label) in [
                            ("modify.select", "select", "Auswählen"),
                            ("modify.move", "move", "Verschieben"),
                            ("modify.copy", "copy", "Kopieren"),
                            ("modify.rotate", "rotate", "Drehen"),
                            ("modify.mirror", "mirror", "Spiegeln"),
                            ("modify.scale", "scale", "Skalieren"),
                        ] {
                            if widgets::ribbon_button(ui, icon, label, app.tool_id() == Some(id)).clicked() {
                                app.start_tool(id);
                            }
                        }
                    });
                    widgets::vline(ui, RIBBON_H);
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
                    widgets::vline(ui, RIBBON_H);
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
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Sonstiges", |ui, app| {
                        for (id, icon, label) in [
                            ("modify.pickprops", "eyedropper", "Pipette"),
                            ("edit.undo", "undo", "Zurück"),
                            ("edit.redo", "redo", "Vor"),
                        ] {
                            if widgets::ribbon_button(ui, icon, label, false).clicked() {
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
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Stil", |ui, app| {
                        ui.vertical(|ui| {
                            ui.add_space(6.0);
                            let unit = app.doc().settings.dim_unit;
                            ui.horizontal(|ui| {
                                ui.label(theme::dim("Maßeinheit"));
                                let mut u = unit;
                                egui::ComboBox::from_id_salt("dimunit")
                                    .width(70.0)
                                    .selected_text(u.short())
                                    .show_ui(ui, |ui| {
                                        for cand in Unit::ALL {
                                            if ui.selectable_value(&mut u, cand, cand.short()).changed() {
                                                let _ = app.session_mut().execute(
                                                    "doc.settings",
                                                    &serde_json::json!({"dim_unit": cand.short()}),
                                                );
                                            }
                                        }
                                    });
                            });
                            ui.horizontal(|ui| {
                                ui.label(theme::dim("Textgröße"));
                                let mut h = app.doc().dim_style.text_height;
                                if ui
                                    .add_sized([64.0, 20.0], crate::widgets::dv(&mut h).speed(0.1).suffix(" mm"))
                                    .changed()
                                {
                                    let _ = app
                                        .session_mut()
                                        .execute("dim.style", &serde_json::json!({"text_height": h}));
                                    app.mark_changed();
                                }
                            });
                        });
                    });
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Maßstab", |ui, app| {
                        ui.vertical(|ui| {
                            ui.add_space(6.0);
                            let mut s = app.doc().settings.annotation_scale;
                            ui.horizontal(|ui| {
                                ui.label(theme::dim("Bezug"));
                                if ui
                                    .add_sized([54.0, 20.0], crate::widgets::dv(&mut s).speed(1.0).range(1.0..=1000.0))
                                    .changed()
                                {
                                    let _ = app.session_mut().execute(
                                        "doc.settings",
                                        &serde_json::json!({"annotation_scale": s}),
                                    );
                                    app.mark_changed();
                                }
                                ui.label(theme::dim(&format!("({})", format_scale(s))));
                            });
                            ui.horizontal(|ui| {
                                ui.label(theme::dim("Bogensymbole"));
                                let mut arrows = app.doc().dim_style.arrow;
                                egui::ComboBox::from_id_salt("arrowkind")
                                    .width(120.0)
                                    .selected_text(arrows.label())
                                    .show_ui(ui, |ui| {
                                        for cand in jjcad_core::style::ArrowKind::ALL {
                                            if ui.selectable_value(&mut arrows, cand, cand.label()).changed() {
                                                let _ = app.session_mut().execute(
                                                    "dim.style",
                                                    &serde_json::json!({"arrow": match cand {
                                                        jjcad_core::style::ArrowKind::Arrow => "arrow",
                                                        jjcad_core::style::ArrowKind::Dot => "dot",
                                                        jjcad_core::style::ArrowKind::None => "none",
                                                        _ => "slash",
                                                    }}),
                                                );
                                                app.mark_changed();
                                            }
                                        }
                                    });
                            });
                        });
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
                    widgets::vline(ui, RIBBON_H);
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
                                app.toast("Rechteck für das Ansichtsfenster aufziehen");
                            } else {
                                app.toast("Bitte zuerst ein Blatt öffnen");
                            }
                        }
                        if widgets::ribbon_button(ui, "pdf", "Blatt-PDF", false).clicked() {
                            app.run_action(ui.ctx(), "sheet.pdf");
                        }
                    });
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Blatteinstellungen", |ui, app| {
                        if let TabKind::Sheet(id) = app.tab().tab {
                            let mut paper = app.doc().sheet(id).map(|s| s.paper).unwrap_or_default();
                            let mut landscape = app.doc().sheet(id).map(|s| s.landscape).unwrap_or(true);
                            let mut scale = app.doc().sheet(id).map(|s| s.scale).unwrap_or(100.0);
                            ui.vertical(|ui| {
                                ui.add_space(6.0);
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
                                    if ui.add_sized([54.0, 20.0], crate::widgets::dv(&mut scale).speed(1.0)).changed() {
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
                    widgets::vline(ui, RIBBON_H);
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
                    widgets::vline(ui, RIBBON_H);
                    self.group(ui, "Fangen", |ui, app| {
                        ui.vertical(|ui| {
                            ui.add_space(6.0);
                            let mut snap = app.doc().settings.snap.clone();
                            let mut changed = false;
                            ui.horizontal(|ui| {
                                changed |= ui.checkbox(&mut snap.endpoint, "Endpunkt").changed();
                                changed |= ui.checkbox(&mut snap.midpoint, "Mitte").changed();
                            });
                            ui.horizontal(|ui| {
                                changed |= ui.checkbox(&mut snap.intersection, "Schnitt").changed();
                                changed |= ui.checkbox(&mut snap.center, "Zentrum").changed();
                            });
                            ui.horizontal(|ui| {
                                changed |= ui.checkbox(&mut snap.grid, "Fangraster").changed();
                                changed |= ui.checkbox(&mut snap.ortho, "Ortho").changed();
                            });
                            ui.horizontal(|ui| {
                                changed |= ui.checkbox(&mut snap.wall_edges, "Wandkanten").changed();
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
            self.toast("Zwischenablage ist leer");
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
            self.toast("Platzieren abgebrochen");
        }
    }

    /// Setzt die Vorschau an der aktuellen Lage ein.
    pub fn commit_ghost(&mut self) -> bool {
        let Some(g) = self.ghost.clone() else {
            return false;
        };
        if let Some(bid) = g.block.clone() {
            let geo = crate::ghost::geometry(self.doc(), &g, &[]);
            let o = geo.origin;
            let u = self.unit();
            let params = serde_json::json!({
                "block": bid,
                "pos": [u.from_mm(o.x), u.from_mm(o.y)],
                "rot": g.rot.to_degrees(),
                "mirror": g.mirror,
                "scale": g.scale,
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
            self.doc_mut().entities.push(e);
        }
        self.session_mut().select(ids);
        self.mark_changed();
        self.ghost = None;
        self.toast("Eingefügt");
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


    // ---- Statuszeile ----

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(10.0);
            let p = self.pointer_world.unwrap_or(V2::ZERO);
            let unit = self.unit();
            ui.label(theme::mono(format!(
                "X {:<9} Y {:<9}",
                unit.format_plain(p.x, unit.decimals()),
                unit.format_plain(p.y, unit.decimals())
            )));
            ui.add_space(6.0);
            if let Some(s) = &self.snap_label {
                ui.label(
                    RichText::new(format!("• {s}"))
                        .size(10.5)
                        .color(Palette::ACCENT),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                ui.label(theme::faint(build_label()));
                ui.add_space(10.0);
                // Maßstab
                ui.label(theme::mono(format_scale(self.doc().settings.annotation_scale)));
                ui.add_space(8.0);
                // Zoom
                ui.label(theme::mono(format!("{:.0} %", (self.current_view().zoom * 100.0).max(1.0))));
                ui.add_space(8.0);
                ui.label(theme::faint("|"));
                ui.add_space(4.0);
                // Einheit
                let mut u = self.unit();
                egui::ComboBox::from_id_salt("unit")
                    .width(46.0)
                    .selected_text(u.short())
                    .show_ui(ui, |ui| {
                        for cand in Unit::ALL {
                            if ui
                                .selectable_value(&mut u, cand, format!("{} ({})", cand.short(), cand.label()))
                                .clicked()
                            {
                                let _ = self.session_mut().execute(
                                    "doc.settings",
                                    &serde_json::json!({"input_unit": cand.short()}),
                                );
                                self.options.text_height = self.doc().settings.text_height;
                            }
                        }
                    });
                ui.add_space(4.0);
                ui.label(theme::faint("|"));
                ui.add_space(4.0);
                // Rasteranzeige: Linksklick an/aus, Rechtsklick Menü
                let resp = theme::chip_id(ui, "chip_raster", "Raster", self.ui.show_grid);
                if resp.clicked() {
                    self.ui.show_grid = !self.ui.show_grid;
                    let v = self.ui.show_grid;
                    let _ = self.session_mut().execute(
                        "doc.settings",
                        &serde_json::json!({"show_grid": v, "snap": {"grid": v}}),
                    );
                }
                if resp.secondary_clicked() {
                    let p = ui.ctx().pointer_latest_pos().unwrap_or(resp.rect.center());
                    self.open_menu(MenuKind::Chip(StatusMenu::Raster), p);
                }
                resp.on_hover_text("Raster ein-/ausschalten · Rechtsklick: Optionen");

                // Ortho
                let snap_now = self.doc().settings.snap.clone();
                let resp = theme::chip_id(ui, "chip_ortho", "Ortho", snap_now.ortho);
                if resp.clicked() {
                    self.set_snap("ortho", !snap_now.ortho);
                }
                if resp.secondary_clicked() {
                    let p = ui.ctx().pointer_latest_pos().unwrap_or(resp.rect.center());
                    self.open_menu(MenuKind::Chip(StatusMenu::Ortho), p);
                }
                resp.on_hover_text("Rechtwinklig zeichnen · Rechtsklick: Polar-Optionen");

                // Fang
                let resp = theme::chip_id(ui, "chip_fang", "Fang", snap_now.endpoint);
                if resp.clicked() {
                    self.set_snap("endpoint", !snap_now.endpoint);
                }
                if resp.secondary_clicked() {
                    let p = ui.ctx().pointer_latest_pos().unwrap_or(resp.rect.center());
                    self.open_menu(MenuKind::Chip(StatusMenu::Fang), p);
                }
                resp.on_hover_text("Fangen ein-/ausschalten · Rechtsklick: einzelne Fangmodi");

                // Linienstärken: px (bildschirmkonstant) oder DPI (wie im PDF)
                let scaled = self.ui.lineweights_scaled;
                let label = if scaled { "Stärke DPI" } else { "Stärke px" };
                let resp = theme::chip_id(ui, "chip_staerke", label, true);
                if resp.clicked() {
                    self.ui.lineweights = true;
                    self.ui.lineweights_scaled = !scaled;
                    self.prefs.lineweights_scaled = self.ui.lineweights_scaled;
                    let _ = self.prefs.save();
                }
                if resp.secondary_clicked() {
                    let p = ui.ctx().pointer_latest_pos().unwrap_or(resp.rect.center());
                    self.open_menu(MenuKind::Chip(StatusMenu::Staerke), p);
                }
                resp.on_hover_text(
                    "px: gleiche Breite auf dem Bildschirm · DPI: Breite wie im PDF (zoombhängig)",
                );
                ui.add_space(6.0);
                ui.label(theme::faint("|"));
                ui.add_space(4.0);
                ui.label(theme::mono(self.doc().summary_short()));
            });
        });
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        if let Some((msg, t0)) = self.ui.toast.clone() {
            let now = ctx.input(|i| i.time);
            let age = if t0 == 0.0 { 0.0 } else { now - t0 };
            if t0 == 0.0 {
                self.ui.toast = Some((msg.clone(), now));
            } else if age > 4.0 {
                self.ui.toast = None;
                return;
            }
            let fade = if age > 3.2 { ((4.0 - age) / 0.8).clamp(0.0, 1.0) } else { 1.0 };
            egui::Area::new(egui::Id::new("toast"))
                .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -104.0))
                .interactable(false)
                .show(ctx, |ui| {
                    let frame = Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(30, 34, 40, (240.0 * fade as f64) as u8))
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0, Palette::BORDER))
                        .inner_margin(Margin::symmetric(14.0, 8.0));
                    frame.show(ui, |ui| {
                        ui.label(RichText::new(msg).size(12.0).color(Palette::TEXT.gamma_multiply(fade as f32)));
                    });
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    // ---- Rahmen ----

    pub fn ui(&mut self, ctx: &egui::Context) {
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
                        .inner_margin(Margin::symmetric(0.0, 2.0)),
                )
                .show(ctx, |ui| self.ribbon(ui));
        }

        egui::TopBottomPanel::top("doctabs")
            .exact_height(DOCTABS_H)
            .frame(Frame::none().fill(Palette::RIBBON))
            .show(ctx, |ui| self.doc_tabs(ui));

        egui::TopBottomPanel::bottom("status")
            .exact_height(STATUS_H)
            .frame(Frame::none().fill(Palette::RIBBON))
            .show(ctx, |ui| self.status_bar(ui));

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

        dialogs::show(self, ctx);
        inventory::show(self, ctx);
        panels::layer_color_popup(self, ctx);
        self.toast_ui(ctx);
    }
}

fn commands_hint(tool: &Tool) -> &'static str {
    match tool.action.as_str() {
        "draw.line" => {
            if tool.pts.is_empty() {
                "Startpunkt klicken"
            } else {
                "Endpunkt klicken – ESC beendet"
            }
        }
        "draw.wall" => {
            if tool.pts.is_empty() {
                "Wandanfang klicken"
            } else {
                "Wandende klicken – ESC beendet"
            }
        }
        "draw.circle" => {
            if tool.pts.is_empty() {
                "Mittelpunkt klicken"
            } else {
                "Radius klicken"
            }
        }
        "draw.rect" => {
            if tool.pts.is_empty() {
                "Erste Ecke"
            } else {
                "Zweite Ecke"
            }
        }
        "draw.door" | "draw.window" => "In eine Wand klicken (öffnet und richtet sich aus)",
        "draw.stair" => {
            if tool.pts.is_empty() {
                "Treppenanfang"
            } else {
                "Treppenende"
            }
        }
        "dim.linear" => match tool.pts.len() {
            0 => "Erster Messpunkt",
            1 => "Zweiter Messpunkt",
            _ => "Maßlinie platzieren",
        },
        "dim.chain" => "Punkte klicken – Enter beendet die Kette",
        "dim.elevation" => "Punkt für Höhenkote klicken",
        "modify.move" | "modify.copy" => {
            if tool.selecting {
                "Objekte wählen"
            } else if tool.pts.is_empty() {
                "Basispunkt"
            } else {
                "Zielpunkt"
            }
        }
        "modify.rotate" => match tool.pts.len() {
            0 => "Drehzentrum",
            1 => "Von-Punkt (Bezugsrichtung)",
            _ => "Bis-Punkt (neue Richtung) – Winkel von → bis",
        },
        "modify.mirror" => {
            if tool.pts.is_empty() {
                "Erster Punkt der Spiegelachse"
            } else {
                "Zweiter Punkt"
            }
        }
        "modify.scale" => {
            if tool.pts.is_empty() {
                "Bezugspunkt"
            } else {
                "Faktor klicken"
            }
        }
        "modify.offset" => "Objekt anklicken, dann Seite",
        "modify.trim" => "Zu stutzendes Segment anklicken",
        "modify.extend" => "Ende anklicken, das verlängert wird",
        "modify.explode" => "Objekt anklicken",
        "block.insert" => "Position klicken",
        "draw.text" => "Position klicken",
        "draw.polyline" => "Punkte klicken – Enter beendet",
        "draw.fill.boundary" => "Umriss abfahren – Enter oder Doppelklick beendet",
        "draw.fill.points" => {
            if tool.pts.len() < 3 {
                "Punkte klicken – mindestens drei, Enter schließt"
            } else {
                "Enter oder Doppelklick schließt die Fläche, ESC verwirft"
            }
        }
        "draw.fill" | "draw.hatch" => "Geschlossene Kontur anklicken",
        "draw.fill.paint" => "In eine leere Fläche klicken — Vorschau zeigt die Fläche",
        "draw.arc" => match tool.pts.len() {
            0 => "Startpunkt",
            1 => "Endpunkt",
            _ => "Punkt auf dem Bogen",
        },
        "draw.point" => "Punkt klicken",
        "modify.select" => "Objekte anklicken oder Fenster aufziehen",
        _ => "Punkt klicken",
    }
}

/// Weltkoordinaten aus Bildschirmposition.
pub fn screen_to_world(rect: Rect, view: View, pos: Pos2) -> V2 {
    let cx = rect.center().x as f64;
    let cy = rect.center().y as f64;
    V2::new(
        view.center.x + (pos.x as f64 - cx) / view.zoom,
        view.center.y - (pos.y as f64 - cy) / view.zoom,
    )
}

pub fn world_to_screen(rect: Rect, view: View, p: V2) -> Pos2 {
    let cx = rect.center().x as f64;
    let cy = rect.center().y as f64;
    Pos2::new(
        (cx + (p.x - view.center.x) * view.zoom) as f32,
        (cy - (p.y - view.center.y) * view.zoom) as f32,
    )
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
        let area = egui::Area::new(egui::Id::new("jjcad_menu"))
            .order(egui::Order::Tooltip)
            .interactable(true)
            .sense(egui::Sense::click())
            .fixed_pos(menu.pos)
            .constrain(true);
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
                            ("Kopieren", "modify.copy"),
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
                            let ids: Vec<Id> = self.doc().entities.iter().map(|e| e.id).collect();
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
        if self.ui.toast.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
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
        EntityKind::Fill { .. } => "fill",
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
    pub fn start_fill_input(&mut self, mode: FillInput, hatch: bool) {
        self.options.fill_pattern = if hatch {
            jjcad_core::style::FillPattern::Lines
        } else {
            jjcad_core::style::FillPattern::Solid
        };
        self.start_tool(mode.action());
        self.ui.fill_active = Some(hatch);
    }

    /// Geteilte Ribbon-Schaltfläche für Füllflächen/Schraffuren.
    fn fill_group_button(&mut self, ui: &mut egui::Ui, hatch: bool) {
        let cur = if hatch {
            self.ui.hatch_input
        } else {
            self.ui.fill_input
        };
        let active = self.tool_id() == Some(cur.action()) && self.ui.fill_active == Some(hatch);
        let label = if hatch { "Schraffur" } else { "Fläche" };
        let (main, arrow) = widgets::ribbon_split_button(ui, cur.icon(hatch), label, active);
        let popup_id = ui.make_persistent_id(("fillpop", hatch));
        let now = ui.input(|i| i.time);
        if arrow.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
            ui.memory_mut(|m| m.data.insert_temp(popup_id.with("opened"), now));
        } else if main.clicked() {
            self.start_fill_input(cur, hatch);
        }
        // Prüfhilfe: Menü für einen Screenshot offen halten (nur mit Env-Variable)
        if let Ok(welches) = std::env::var("JJCAD_DEMO_MENU") {
            let passt = (welches == "hatch") == hatch;
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
                        if hatch {
                            self.ui.hatch_input = m;
                            self.prefs.hatch_input = m.key().to_string();
                        } else {
                            self.ui.fill_input = m;
                            self.prefs.fill_input = m.key().to_string();
                        }
                        let _ = self.prefs.save();
                        self.start_fill_input(m, hatch);
                        ui.memory_mut(|mm| mm.close_popup());
                    }
                }
            },
        );
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
        self.status_msg = "Auswahl verschieben".into();
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

    pub fn end_move_drag(&mut self) {
        if self.move_drag.take().is_some() {
            self.mark_changed();
            self.status_msg = "Verschoben".into();
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
        self.status_msg = "Verschieben abgebrochen".into();
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
        assert_eq!(app.status_msg, "Keine Auswahl");

        app.order_pick_start(true);
        assert_eq!(app.ui.order_pick, None, "ohne Auswahl kein Zielmodus");
        assert_eq!(app.status_msg, "Zuerst ein Element auswählen");
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
