//! Dateien: Projektformat `.jjcad` (JSON), Benutzereinstellungen.

use crate::actions;
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Dokument
// ---------------------------------------------------------------------------

pub fn to_json(doc: &Document) -> Result<String, String> {
    serde_json::to_string_pretty(doc).map_err(|e| format!("Serialisierung fehlgeschlagen: {e}"))
}

pub fn from_json(s: &str) -> Result<Document, String> {
    let mut raw: serde_json::Value = serde_json::from_str(s)
        .map_err(|e| format!("Datei ist keine gültige JoJa-CAD-Zeichnung: {e}"))?;
    migrate_legacy_fills(&mut raw);
    let mut doc: Document = serde_json::from_value(raw)
        .map_err(|e| format!("Datei ist keine gültige JoJa-CAD-Zeichnung: {e}"))?;
    if doc.format != FORMAT_NAME {
        return Err(format!(
            "Unbekanntes Format '{}' (erwartet '{}')",
            doc.format, FORMAT_NAME
        ));
    }
    if doc.version > FORMAT_VERSION {
        return Err(format!(
            "Die Datei wurde mit einer neueren Version erstellt (Format {} > {})",
            doc.version, FORMAT_VERSION
        ));
    }
    // Rasterweite aus älteren Dateien in den brauchbaren Bereich holen
    doc.settings.snap.grid_step = crate::model::clamp_grid_step(doc.settings.snap.grid_step);
    // Fehlende Layer reparieren
    if doc.layers.is_empty() {
        doc.layers.push(Layer::new(1, "Waende", crate::style::Color::BLACK));
        doc.current_layer = 1;
    }
    if doc.layer(doc.current_layer).is_none() {
        doc.current_layer = doc.layers[0].id;
    }
    let max_id = doc
        .entities
        .iter()
        .map(|e| e.id)
        .chain(doc.layers.iter().map(|l| l.id))
        .chain(doc.sheets.iter().map(|s| s.id))
        .max()
        .unwrap_or(0);
    doc.next_id = doc.next_id.max(max_id + 1);
    // Türen auf die vordere Wandkante setzen (auch bei älteren Dateien)
    actions::normalize_openings(&mut doc);
    // Blattrahmen älterer Dateien zu normalen Linien machen
    normalize_sheet_frames(&mut doc);
    doc.modified = false;
    Ok(doc)
}

/// Blattrahmen zu normalen Linien machen (T0004).
///
/// Bis dahin zeichnete der Renderer den Rahmen aus `Sheet::frame` — er war
/// damit weder auswählbar noch bearbeitbar. Beim Laden bekommt jedes Blatt mit
/// gesetztem Rahmen vier Linienobjekte (vor dem Schriftfeld, also dahinter) und
/// die Flags werden zurückgesetzt. Neue Blätter legen sie sofort an.
pub fn normalize_sheet_frames(doc: &mut Document) {
    let mut nachzuholen: Vec<(usize, Vec<Entity>)> = Vec::new();
    for (i, sheet) in doc.sheets.iter().enumerate() {
        if !sheet.frame {
            continue;
        }
        let (w, h) = sheet.size();
        nachzuholen.push((i, crate::templates::frame_entities(w, h)));
    }
    for (i, mut rahmen) in nachzuholen {
        for e in rahmen.iter_mut() {
            e.id = doc.new_id();
        }
        let sheet = &mut doc.sheets[i];
        rahmen.extend(std::mem::take(&mut sheet.entities));
        sheet.entities = rahmen;
        sheet.frame = false;
        sheet.show_title_block = false;
    }
}

/// Einzige Migrationsstelle für den alten Objekttyp `Hatch` (Schraffur).
///
/// Bis v0.2 gab es neben der Füllfläche eine eigene Schraffur. Beide sind jetzt
/// `EntityKind::Fill`; alte Dateien werden hier beim Laden umgeschrieben:
///
/// * `type: "hatch"`  → `type: "fill"`
/// * `angle`          → `pattern_angle` (Bogenmaß, unverändert)
/// * `spacing`        → `pattern_scale` (mm, unverändert)
/// * `filled: true`   → `style.fill` (die alten Renderer zeichneten die Fläche
///   dann in der aufgehellten Linienfarbe; ohne Angabe bleibt sie leer)
///
/// Serde-Alias geht nicht, weil sich neben dem Typnamen auch Feldnamen ändern.
fn migrate_legacy_fills(v: &mut serde_json::Value) {
    let layer_colors: std::collections::BTreeMap<u64, crate::style::Color> = v
        .get("layers")
        .and_then(|l| l.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|l| {
                    let id = l.get("id")?.as_u64()?;
                    let col = l.get("color").and_then(|c| c.as_str())?;
                    Some((id, crate::style::Color::from_hex(col)?))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut fix_list = |list: &mut Vec<serde_json::Value>| {
        for e in list.iter_mut() {
            let Some(kind) = e.get_mut("kind").and_then(|k| k.as_object_mut()) else {
                continue;
            };
            if kind.get("type").and_then(|t| t.as_str()) != Some("hatch") {
                continue;
            }
            kind.insert("type".into(), serde_json::json!("fill"));
            if let Some(ang) = kind.remove("angle") {
                kind.insert("pattern_angle".into(), ang);
            }
            if let Some(sp) = kind.remove("spacing") {
                kind.insert("pattern_scale".into(), sp);
            }
            let filled = kind
                .remove("filled")
                .and_then(|f| f.as_bool())
                .unwrap_or(false);
            if filled {
                // Der alte Renderer nahm die Objektfarbe, sonst die Layerfarbe.
                let layer = e.get("layer").and_then(|l| l.as_u64()).unwrap_or(0);
                let base = e
                    .get("style")
                    .and_then(|s| s.get("color"))
                    .and_then(|c| c.as_str())
                    .and_then(crate::style::Color::from_hex)
                    .or_else(|| layer_colors.get(&layer).copied())
                    .unwrap_or(crate::style::Color::BLACK);
                if !e.get("style").map(|s| s.is_object()).unwrap_or(false) {
                    e["style"] = serde_json::json!({});
                }
                if let Some(st) = e.get_mut("style").and_then(|s| s.as_object_mut()) {
                    st.entry("fill")
                        .or_insert_with(|| serde_json::json!(base.mix(crate::style::Color::WHITE, 0.72).to_hex()));
                }
            }
        }
    };

    if let Some(arr) = v.get_mut("entities").and_then(|e| e.as_array_mut()) {
        fix_list(arr);
    }
    if let Some(blocks) = v.get_mut("blocks").and_then(|b| b.as_array_mut()) {
        for b in blocks.iter_mut() {
            if let Some(arr) = b.get_mut("entities").and_then(|e| e.as_array_mut()) {
                fix_list(arr);
            }
        }
    }
}

pub fn save_document(doc: &mut Document, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Verzeichnis konnte nicht angelegt werden: {e}"))?;
        }
    }
    let json = to_json(doc)?;
    std::fs::write(path, json).map_err(|e| format!("Schreiben fehlgeschlagen: {e}"))?;
    doc.path = Some(path.to_string_lossy().to_string());
    doc.name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| doc.name.clone());
    doc.modified = false;
    Ok(())
}

pub fn load_document(path: &Path) -> Result<Document, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("Lesen fehlgeschlagen: {e}"))?;
    let mut doc = from_json(&s)?;
    doc.path = Some(path.to_string_lossy().to_string());
    doc.name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Zeichnung".into());
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Benutzereinstellungen (Hotbar, Tastenbelegung)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HotbarPreset {
    pub name: String,
    /// 10 Slots mit Action-IDs (None = leer)
    pub slots: Vec<Option<String>>,
}

impl HotbarPreset {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            slots: vec![None; 10],
        }
    }

    pub fn filled(name: &str, items: &[&str]) -> Self {
        let mut slots: Vec<Option<String>> = items.iter().map(|s| Some(s.to_string())).collect();
        slots.resize(10, None);
        Self {
            name: name.to_string(),
            slots,
        }
    }
}

/// Art, wie Objekte und Griffe gezogen werden.
///
/// `HoldAndDrag` ist das bisherige Verhalten und bleibt Standard, damit sich
/// für Bestandsnutzer nichts ändert.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DragMode {
    /// Taste drücken, ziehen, loslassen
    HoldAndDrag,
    /// Erster Klick setzt den Basispunkt, zweiter Klick bestätigt
    TwoClicks,
}

impl Default for DragMode {
    fn default() -> Self {
        DragMode::HoldAndDrag
    }
}

impl DragMode {
    /// Beide Modi in Anzeigereihenfolge (Einstellungen, Hilfetexte).
    pub const ALL: [DragMode; 2] = [DragMode::HoldAndDrag, DragMode::TwoClicks];

    /// Deutsche Beschriftung für die Oberfläche.
    pub fn label(self) -> &'static str {
        match self {
            DragMode::HoldAndDrag => "Gedrückthalten",
            DragMode::TwoClicks => "Zwei Klicks",
        }
    }

    /// Kurze Erläuterung (Tooltip bzw. Hilfetext im Dialog).
    pub fn hint(self) -> &'static str {
        match self {
            DragMode::HoldAndDrag => "Taste drücken, ziehen, loslassen",
            DragMode::TwoClicks => {
                "Klick setzt den Basispunkt, Bewegung zeigt die Vorschau, zweiter Klick bestätigt"
            }
        }
    }

    /// true, wenn mit zwei Klicks gezogen wird.
    pub fn two_clicks(self) -> bool {
        matches!(self, DragMode::TwoClicks)
    }
}

/// Liest den neuen Modus und übernimmt zusätzlich das alte Wahrheitsfeld
/// `drag_two_clicks` (Build b7), damit eine bestehende `prefs.json` ihre
/// Einstellung behält, statt still auf den Standard zurückzufallen.
fn de_drag_mode_mit_altfeld<'de, D>(d: D) -> Result<DragMode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Roh {
        Modus(DragMode),
        Alt(bool),
    }
    Ok(match Roh::deserialize(d)? {
        Roh::Modus(m) => m,
        Roh::Alt(true) => DragMode::TwoClicks,
        Roh::Alt(false) => DragMode::HoldAndDrag,
    })
}

/// Liest die Tastenbelegung in beiden Formen: `"draw.line": "l"` (bis T0063)
/// und `"draw.line": ["l", "t"]` (seit T0064, mehrere Tasten je Funktion).
fn de_bindungen<'de, D>(d: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Roh {
        Einzeln(String),
        Liste(Vec<String>),
    }
    let roh: BTreeMap<String, Roh> = BTreeMap::deserialize(d)?;
    Ok(roh
        .into_iter()
        .map(|(aktion, wert)| {
            let tasten = match wert {
                Roh::Einzeln(k) => vec![k],
                Roh::Liste(l) => l,
            };
            (aktion, tasten)
        })
        .collect())
}

fn default_volume_input() -> String {
    "points".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPrefs {
    #[serde(default)]
    pub hotbar_presets: Vec<HotbarPreset>,
    #[serde(default = "default_preset_idx")]
    pub active_preset: usize,
    /// Action-ID → Tasten, die diese Funktion auslösen (z. B. `["l"]`,
    /// `["ctrl+s"]`, `["g", "t"]`). Eine **Taste** trägt immer genau eine
    /// Funktion; eine **Funktion** darf auf mehreren Tasten liegen (T0064).
    /// Ältere Dateien tragen je Funktion eine einzelne Zeichenkette — der
    /// Leser nimmt beides (`de_bindungen`).
    #[serde(default, deserialize_with = "de_bindungen")]
    pub bindings: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub recent_files: Vec<String>,
    #[serde(default)]
    pub last_dir: String,
    #[serde(default = "default_true")]
    pub show_lineweights: bool,
    /// Linienstärken maßstabsgetreu statt bildschirmkonstant
    #[serde(default)]
    pub lineweights_scaled: bool,
    /// Zieh-Modus für Objekte und Griffe
    #[serde(
        default,
        alias = "drag_two_clicks",
        deserialize_with = "de_drag_mode_mit_altfeld"
    )]
    pub drag_mode: DragMode,
    /// Objekte lassen sich mit der gedrückten Maustaste verschieben (T0063).
    /// Ausgeschaltet wandert ausschließlich das Werkzeug „Verschieben“ — die
    /// Einstellung gilt für **alle** Objektarten, auch Text und Treppe.
    #[serde(default = "default_true")]
    pub mouse_move: bool,
    /// Schwenkgeschwindigkeit der Ansicht mit den Schwenk-Funktionen
    /// (`view.pan_left` … `view.pan_down`, Vorgabe W/A/S/D) als **Faktor**
    /// auf das Grundtempo (T0067): 1,0 = Vorgabe, 2,0 = doppelt so schnell.
    /// Gilt beim Halten (Bildlauf) und beim Einzelschritt der Funktion.
    #[serde(default = "default_pan_speed")]
    pub pan_speed: f32,
    /// Zuletzt benutzter Eingabemodus der Fuellflaechen-Schaltflaeche
    /// ("contour" | "points" | "paint")
    #[serde(default)]
    pub fill_input: String,
    /// Dasselbe fuer die Schraffur-Schaltflaeche
    #[serde(default)]
    pub hatch_input: String,
    /// Zuletzt benutzter Einfügemodus des Volumens ("contour" | "points" | "paint")
    #[serde(default = "default_volume_input")]
    pub volume_input: String,
    /// Zuletzt benutzte Kopierfunktion der Schaltflaeche „Kopieren"
    /// ("copy" = in die Zwischenablage | "copy_base" = mit Basispunkt)
    #[serde(default)]
    pub copy_input: String,
    #[serde(default = "default_true")]
    pub snap_marker: bool,
    /// Wandkontur-Punktfang (snap.wall_edges) — app-weit gemerkt, damit die
    /// Einstellung auch in einer neuen/ungespeicherten Zeichnung erhalten bleibt.
    #[serde(default = "default_true")]
    pub snap_wall_edges: bool,
    #[serde(default)]
    pub show_properties: bool,
    #[serde(default = "default_true")]
    pub show_layers: bool,
    /// Inventar-Raster (frei belegbar, beliebig erweiterbar)
    #[serde(default)]
    pub inventory: Vec<Option<String>>,
    /// Dateidialog: "system" = Dialog des Betriebssystems (Vorgabe),
    /// "intern" = eigener Dialog der Anwendung
    #[serde(default = "default_dateidialog")]
    pub file_dialog: String,
}

fn default_dateidialog() -> String {
    "system".to_string()
}

fn default_preset_idx() -> usize {
    0
}
fn default_true() -> bool {
    true
}

/// Vorgabe der Schwenkgeschwindigkeit: unverändert das bisherige Tempo (T0067).
fn default_pan_speed() -> f32 {
    1.0
}

/// Grenzen der Schwenkgeschwindigkeit (T0067): höchstens ein Zehntel bis
/// fünffach — darunter kriecht die Ansicht, darüber ist kein Ziel mehr zu
/// treffen. Eigene Grenzen, weil der Wert kein Papier-Maß ist.
pub const SCHWENK_FAKTOR_MIN: f32 = 0.1;
/// Gegenstück zu [`SCHWENK_FAKTOR_MIN`].
pub const SCHWENK_FAKTOR_MAX: f32 = 5.0;

/// Die drei Schichten der Tastatur (T0056, `docs/TASTEN.md`):
///
/// 1. **Ziffern `1` … `0`** starten den Hotbar-Slot — gesteuert über die
///    Hotbar, nie über die Tastaturbelegung.
/// 2. **Buchstaben und Symbole** sind frei zuweisbar (`UserPrefs::bindings`,
///    Dialog „Tastaturbelegung").
/// 3. **Funktionstasten, Steuerungstasten und Tastenkombinationen** sind
///    hart verdrahtet: ihre Funktion steht im Code und lässt sich nicht
///    umbelegen.
///
/// `fest_belegt` sagt, in welche Schicht eine Taste gehört; `None` heißt frei
/// zuweisbar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FestBelegt {
    /// Ziffer 1 … 0 — startet den Hotbar-Slot (`App::handle_shortcuts`).
    Hotbar,
    /// Esc, Tab, Caps, Umschalt, Strg, Alt, Enter, Rücktaste, Leertaste,
    /// Entf — fester Ablauf im Code.
    Steuerung,
    /// F1 … F12 — fester Ablauf im Code.
    Funktion,
    /// Tastenkombination mit `+` (Strg+C, Strg+S …) — fest im Code.
    Kombination,
    /// Zeichen der zweiten Ebene einer deutschen Tastatur (`#`, `<`, `´`):
    /// Umschalt- bzw. AltGr-Zeichen eines anderen Keys. Sie kommen nie als
    /// eigene Taste an und lassen sich deshalb nicht belegen.
    ZweiteEbene,
}

impl FestBelegt {
    /// Kurzhinweis für den Tooltip der Taste.
    pub fn grund(self) -> &'static str {
        match self {
            FestBelegt::Hotbar => "Ziffer 1 … 0 startet die Hotbar — nicht belegbar",
            FestBelegt::Steuerung => "Steuerungstaste — fest belegt, nicht zuweisbar",
            FestBelegt::Funktion => "Funktionstaste — fest belegt, nicht zuweisbar",
            FestBelegt::Kombination => "Tastenkombination — fest belegt, nicht zuweisbar",
            FestBelegt::ZweiteEbene => {
                "Zeichen der zweiten Ebene (Umschalt/AltGr) — nicht einzeln belegbar"
            }
        }
    }

    /// Darf diese Taste überhaupt einer Funktion gehören? Ziffern und
    /// Steuerungstasten lösen eigene Abläufe aus (Hotbar-Slot, Abbrechen,
    /// Bestätigen) und sind in keiner Belegung zu finden. Funktionstasten und
    /// Kombinationen stehen dagegen als feste Belegung ihrer Funktion in
    /// `bindings` — nur ändern lässt sich daran nichts.
    pub fn ist_feste_belegung(self) -> bool {
        matches!(self, FestBelegt::Funktion | FestBelegt::Kombination)
    }
}

/// Warum eine Taste nicht belegbar ist? `None` = frei zuweisbar.
pub fn fest_belegt(key: &str) -> Option<FestBelegt> {
    let k = key.to_lowercase();
    // Kombinationen zuerst: "ctrl+s" ist keine Steuerungstaste, sondern eine
    // Kombination aus Strg und einem Buchstaben.
    // Tastenkombinationen mit Strg oder Alt sind fest verdrahtet. Umschalt
    // gehört dagegen zur Taste: `Umschalt+I` ist die zweite Ebene des
    // Buchstabens und bleibt wie dieser frei belegbar. Die Taste `+` selbst ist
    // ein Zeichen, keine Kombination.
    if k.starts_with("ctrl+") || k.starts_with("alt+") {
        return Some(FestBelegt::Kombination);
    }
    match k.as_str() {
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "0" => Some(FestBelegt::Hotbar),
        "esc" | "tab" | "caps" | "shift" | "shift_r" | "ctrl" | "ctrl_r" | "alt" | "altgr"
        | "enter" | "backspace" | "space" | " " | "entf" | "delete" => {
            Some(FestBelegt::Steuerung)
        }
        "f1" | "f2" | "f3" | "f4" | "f5" | "f6" | "f7" | "f8" | "f9" | "f10" | "f11" | "f12" => {
            Some(FestBelegt::Funktion)
        }
        // Zweite Ebene der deutschen Tastatur: diese Zeichen teilen sich die
        // Taste mit einem anderen (z. B. `#` mit der Taste rechts von Ä) und
        // kommen nie als eigener Tastendruck an.
        "´" | "#" | "<" => Some(FestBelegt::ZweiteEbene),
        _ => None,
    }
}

impl Default for UserPrefs {
    fn default() -> Self {
        let mut bindings = BTreeMap::new();
        for a in actions::ACTIONS {
            if a.key.is_empty() {
                continue;
            }
            // Ziffern und Steuerungstasten gehören keiner Funktion; Funktions-
            // tasten und Kombinationen sind dagegen genau ihre feste Belegung
            // (T0056, `docs/TASTEN.md`).
            if matches!(fest_belegt(a.key), Some(f) if !f.ist_feste_belegung()) {
                continue;
            }
            bindings.insert(a.id.to_string(), vec![a.key.to_string()]);
        }
        Self {
            hotbar_presets: vec![
                HotbarPreset::filled(
                    "Zeichnen",
                    &[
                        "draw.wall",
                        "draw.door",
                        "draw.window",
                        "draw.line",
                        "draw.polyline",
                        "draw.circle",
                        "draw.text",
                        "dim.linear",
                        "modify.move",
                        "modify.erase",
                    ],
                ),
                HotbarPreset::filled(
                    "Architektur",
                    &[
                        "draw.wall",
                        "draw.door",
                        "draw.window",
                        "draw.stair",
                        "dim.chain",
                        "dim.elevation",
                        "draw.hatch",
                        "block.insert",
                        "modify.trim",
                        "modify.offset",
                    ],
                ),
                HotbarPreset::filled(
                    "Bearbeiten",
                    &[
                        "modify.select",
                        "modify.move",
                        "modify.copy",
                        "modify.rotate",
                        "modify.mirror",
                        "modify.scale",
                        "modify.trim",
                        "modify.extend",
                        "modify.offset",
                        "modify.explode",
                    ],
                ),
            ],
            active_preset: 0,
            bindings,
            recent_files: Vec::new(),
            last_dir: String::new(),
            show_lineweights: true,
            lineweights_scaled: false,
            drag_mode: DragMode::HoldAndDrag,
            mouse_move: true,
            pan_speed: default_pan_speed(),
            fill_input: "points".into(),
            hatch_input: "contour".into(),
            volume_input: "points".into(),
            // Vorgabe wie bisher im Reiter „Ändern": Kopieren setzt mit
            // Basispunkt ab. Reines Kopieren in die Zwischenablage wählt man im
            // Aufklappmenü oder mit Strg+C.
            copy_input: "copy_base".into(),
            snap_marker: true,
            snap_wall_edges: true,
            show_properties: true,
            show_layers: true,
            inventory: vec![None; 40],
            file_dialog: default_dateidialog(),
        }
    }
}

impl UserPrefs {
    /// Soll der Dateidialog des Betriebssystems benutzt werden? Vorgabe ist
    /// „system“; nur ein ausdrückliches "intern" wählt den eigenen Dialog.
    pub fn dateidialog_system(&self) -> bool {
        self.file_dialog != "intern"
    }

    pub fn path() -> PathBuf {
        config_dir().join("prefs.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    /// Lädt Einstellungen aus einer beliebigen Datei (Tests, portable Profile).
    pub fn load_from(path: &Path) -> Self {
        if let Ok(s) = std::fs::read_to_string(path) {
            if let Ok(mut prefs) = serde_json::from_str::<UserPrefs>(&s) {
                prefs.ensure_defaults();
                return prefs;
            }
        }
        let mut p = UserPrefs::default();
        p.ensure_defaults();
        p
    }

    fn ensure_defaults(&mut self) {
        // T0067: Ein alter oder fremder Wert darf die Ansicht weder einfrieren
        // noch ins Nichts schießen — deshalb beim Laden auf die Grenzen klemmen.
        self.pan_speed = if self.pan_speed.is_finite() {
            self.pan_speed.clamp(SCHWENK_FAKTOR_MIN, SCHWENK_FAKTOR_MAX)
        } else {
            default_pan_speed()
        };
        if self.hotbar_presets.is_empty() {
            self.hotbar_presets = UserPrefs::default().hotbar_presets;
        }
        for p in &mut self.hotbar_presets {
            p.slots.resize(10, None);
            p.slots.truncate(10);
        }
        if self.inventory.len() < 40 {
            self.inventory.resize(40, None);
        }
        if self.bindings.is_empty() {
            self.bindings = UserPrefs::default().bindings;
        }
        // T0045: Ziffern und Steuerungstasten gehören dem Programm. Ältere
        // Belegungen (früher stand `modify.select` auf ESC) würden die Taste
        // sonst stillschweigend belegen und im Tastenbild falsch aussehen.
        // Funktionstasten und Kombinationen bleiben dagegen stehen: sie sind die
        // feste Belegung ihrer Funktion (T0056).
        self.bindings.retain(|_, keys| {
            keys.retain(|k| !matches!(fest_belegt(k), Some(f) if !f.ist_feste_belegung()));
            !keys.is_empty()
        });
        // T0056: `S` schwenkt die Ansicht nach unten (`view.pan_down`). Das
        // Stutzen lag früher auf derselben Taste — eine gespeicherte Belegung
        // würde dem Schwenk die Taste wegnehmen.
        if let Some(keys) = self.bindings.get_mut("modify.trim") {
            keys.retain(|k| !k.eq_ignore_ascii_case("s"));
        }
        self.bindings.retain(|_, keys| !keys.is_empty());
        // T0056: `V` gehört dem Verschieben (so steht es in der Bedienung). Das
        // Volumen lag früher auf derselben Taste und gewann dort die Auswertung —
        // jetzt auf Umschalt+V.
        if let Some(keys) = self.bindings.get_mut("draw.volume") {
            for k in keys.iter_mut() {
                if k.eq_ignore_ascii_case("v") {
                    *k = "shift+v".into();
                }
            }
        }
        // T0056: Tastenkombinationen und Funktionstasten sind hart verdrahtet
        // (`App::handle_shortcuts`) — eine gespeicherte Belegung darf sie nicht
        // verschieben, sonst täte Strg+S etwas anderes als dokumentiert.
        for a in actions::ACTIONS {
            if matches!(fest_belegt(a.key), Some(f) if f.ist_feste_belegung()) {
                self.bindings
                    .insert(a.id.to_string(), vec![a.key.to_string()]);
            }
        }
        // T gehört seit T0009 dem Chat-Protokoll. Das Text-Werkzeug lag früher
        // auf dieser Taste; eine gespeicherte Belegung würde sie stillschweigend
        // weiter belegen und in der Tastaturbelegung falsch anzeigen.
        if let Some(keys) = self.bindings.get_mut("draw.text") {
            keys.retain(|k| !k.eq_ignore_ascii_case("t"));
        }
        self.bindings.retain(|_, keys| !keys.is_empty());
        // T0012: „Kopieren" (Zwischenablage) und „Kopieren mit Basispunkt" sind
        // getrennte Funktionen. Ältere Belegungen tragen Strg+C noch an der
        // Basispunkt-Kopie — die Tasten kommen jetzt aus dem Register.
        let alt = self
            .bindings
            .get("modify.copy")
            .map(|keys| keys.iter().any(|k| k.eq_ignore_ascii_case("ctrl+c")))
            .unwrap_or(false);
        if alt {
            self.bindings
                .insert("modify.copy".into(), vec!["ctrl+shift+c".into()]);
            self.bindings
                .insert("edit.copy".into(), vec!["ctrl+c".into()]);
        }
        // T0060: „Eigenschaften aufnehmen" (`modify.pickprops`) ist ersetzt durch
        // die **Schnellauswahl**. Die alte Belegung würde `q` sonst für eine
        // Funktion belegen, die es nicht mehr gibt — und der neuen die Taste
        // wegnehmen. Entfernen lässt sie der Nachzieh-Schritt unten wieder
        // vergeben.
        self.bindings.remove("modify.pickprops");
        // `bindings` wird als Ganzes gespeichert. Eine später ergänzte Aktion
        // bekäme ihre Werktaste sonst nie: fehlende Einträge nachziehen, solange
        // die Taste nicht schon einer anderen Aktion gehört.
        for a in actions::ACTIONS {
            if a.key.is_empty() || self.bindings.contains_key(a.id) {
                continue;
            }
            if matches!(fest_belegt(a.key), Some(f) if !f.ist_feste_belegung()) {
                continue;
            }
            let frei = !self
                .bindings
                .values()
                .flatten()
                .any(|k| k.eq_ignore_ascii_case(a.key));
            if frei {
                self.bindings
                    .insert(a.id.to_string(), vec![a.key.to_string()]);
            }
        }
        if self.active_preset >= self.hotbar_presets.len() {
            self.active_preset = 0;
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    /// Schreibt die Einstellungen in eine beliebige Datei (Tests, portable Profile).
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, s)
            .map_err(|e| format!("Einstellungen konnten nicht gespeichert werden: {e}"))
    }

    /// Aktuelle Hotbar-Slots.
    pub fn slots(&self) -> Vec<Option<String>> {
        self.hotbar_presets
            .get(self.active_preset)
            .map(|p| p.slots.clone())
            .unwrap_or_else(|| vec![None; 10])
    }

    pub fn set_slot(&mut self, idx: usize, action: Option<String>) {
        if let Some(p) = self.hotbar_presets.get_mut(self.active_preset) {
            if idx < p.slots.len() {
                p.slots[idx] = action;
            }
        }
    }

    pub fn next_preset(&mut self) {
        if !self.hotbar_presets.is_empty() {
            self.active_preset = (self.active_preset + 1) % self.hotbar_presets.len();
        }
    }

    /// Die Tasten einer Funktion in Belegungsreihenfolge (leer, wenn keine).
    pub fn keys_for(&self, action: &str) -> &[String] {
        self.bindings
            .get(action)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Erste Taste einer Funktion — Anzeige in der Funktionsliste, Tests.
    pub fn key_for(&self, action: &str) -> Option<String> {
        self.keys_for(action).first().cloned()
    }

    /// Aktion für eine Taste (ohne Modifier). Jede Taste gehört genau einer
    /// Funktion — die erste Fundstelle gewinnt, sollte eine alte Datei doch
    /// zwei tragen.
    pub fn action_for_key(&self, key: &str) -> Option<String> {
        let k = key.to_lowercase();
        self.bindings
            .iter()
            .find(|(_, keys)| keys.iter().any(|v| v.to_lowercase() == k))
            .map(|(a, _)| a.clone())
    }

    /// Ist die Belegung dieser Funktion änderbar? Nein, wenn eine ihrer Tasten
    /// fest verdrahtet ist (Funktionstaste, Tastenkombination) — die stehen im
    /// Code (T0056, `docs/TASTEN.md`).
    pub fn belegung_aenderbar(&self, action: &str) -> bool {
        match self.bindings.get(action) {
            Some(keys) => !keys
                .iter()
                .any(|k| matches!(fest_belegt(k), Some(f) if f.ist_feste_belegung())),
            None => true,
        }
    }

    /// Legt `action` **zusätzlich** auf `key` (T0064: eine Funktion darf auf
    /// mehreren Tasten liegen). Gesperrte Tasten (`fest_belegt`) bleiben
    /// unberührt — sie gehören dem Programm, nicht der Belegung. Funktionen mit
    /// fest verdrahteter Taste lassen sich nicht umbelegen.
    ///
    /// Die Taste wird dabei einer anderen Funktion entzogen: eine Taste trägt
    /// immer genau eine Funktion.
    pub fn bind(&mut self, action: &str, key: &str) {
        let key = key.to_lowercase();
        if fest_belegt(&key).is_some() || !self.belegung_aenderbar(action) {
            return;
        }
        for keys in self.bindings.values_mut() {
            keys.retain(|k| k.to_lowercase() != key);
        }
        self.bindings.retain(|_, keys| !keys.is_empty());
        let liste = self.bindings.entry(action.to_string()).or_default();
        if !liste.iter().any(|k| k.to_lowercase() == key) {
            liste.push(key);
        }
    }

    /// Nimmt eine Taste aus der Belegung (mittlere Maustaste im Tastenbild) und
    /// liefert die Funktion, die sie getragen hat.
    pub fn unbind_key(&mut self, key: &str) -> Option<String> {
        let k = key.to_lowercase();
        let aktion = self.action_for_key(&k);
        for keys in self.bindings.values_mut() {
            keys.retain(|s| s.to_lowercase() != k);
        }
        self.bindings.retain(|_, keys| !keys.is_empty());
        aktion
    }

    /// Nimmt der Funktion **alle** Tasten.
    pub fn unbind_action(&mut self, action: &str) {
        self.bindings.remove(action);
    }

    pub fn reset_bindings(&mut self) {
        self.bindings = UserPrefs::default().bindings;
    }

    pub fn add_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(12);
    }
}

/// Konfigurationsverzeichnis (plattformabhängig).
pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("JoJa-CAD");
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("jjcad");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("jjcad")
}

/// Benutzerblock-Bibliothek (profilweit).
pub fn user_blocks_dir() -> PathBuf {
    config_dir().join("blocks")
}

pub fn autosave_path() -> PathBuf {
    config_dir().join("autosave.jjcad")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eigenes Testverzeichnis, damit die echten Einstellungen des Nutzers
    /// (`~/.config/jjcad/prefs.json`) unberührt bleiben.
    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jjcad-prefs-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn drag_mode_standard_ist_gedrueckthalten() {
        assert_eq!(DragMode::default(), DragMode::HoldAndDrag);
        assert_eq!(UserPrefs::default().drag_mode, DragMode::HoldAndDrag);
        assert!(!DragMode::HoldAndDrag.two_clicks());
        assert!(DragMode::TwoClicks.two_clicks());
    }

    /// T0063: „Maus verschiebt“ ist in der Vorgabe an (bisheriges Verhalten)
    /// und überlebt Speichern und Laden; fehlt der Schlüssel, gilt ebenfalls an.
    #[test]
    fn maus_verschiebt_vorgabe_an_und_wird_gespeichert() {
        assert!(UserPrefs::default().mouse_move);
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert!(alt.mouse_move, "Altbestand ohne Schlüssel muss eingeschaltet sein");

        let mut prefs = UserPrefs::default();
        prefs.mouse_move = false;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(roh.contains("\"mouse_move\":false"), "Schlüssel fehlt: {roh}");
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert!(!wieder.mouse_move, "Einstellung ging beim Laden verloren");
    }

    /// T0067: Die Schwenkgeschwindigkeit ist ein Faktor mit Vorgabe 1,0, wird
    /// gespeichert und beim Laden auf ihre Grenzen geklemmt.
    #[test]
    fn schwenkgeschwindigkeit_vorgabe_gespeichert_und_begrenzt() {
        assert!((UserPrefs::default().pan_speed - 1.0).abs() < 1e-9);
        // Altbestand ohne Schlüssel: unverändertes Tempo.
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert!((alt.pan_speed - 1.0).abs() < 1e-9);

        let mut prefs = UserPrefs::default();
        prefs.pan_speed = 2.5;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(roh.contains("\"pan_speed\":2.5"), "Schlüssel fehlt: {roh}");
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert!(
            (wieder.pan_speed - 2.5).abs() < 1e-9,
            "Einstellung ging beim Laden verloren"
        );

        // Unsinnige Werte werden auf die Grenzen gezogen.
        for (eingabe, erwartet) in [
            (0.0_f32, SCHWENK_FAKTOR_MIN),
            (-3.0, SCHWENK_FAKTOR_MIN),
            (99.0, SCHWENK_FAKTOR_MAX),
            (f32::NAN, 1.0),
        ] {
            let mut p = UserPrefs::default();
            p.pan_speed = eingabe;
            p.ensure_defaults();
            assert!(
                (p.pan_speed - erwartet).abs() < 1e-9,
                "{eingabe} ergibt {}, erwartet {erwartet}",
                p.pan_speed
            );
        }
    }

    #[test]
    fn drag_mode_schreibt_snake_case() {
        let s = serde_json::to_string(&DragMode::TwoClicks).unwrap();
        assert_eq!(s, "\"two_clicks\"");
        let s = serde_json::to_string(&DragMode::HoldAndDrag).unwrap();
        assert_eq!(s, "\"hold_and_drag\"");
    }

    #[test]
    fn alte_prefs_ohne_drag_mode_ergeben_gedrueckthalten() {
        // Alte Datei: vollständige Einstellungen, aber ohne den neuen Schlüssel.
        let dir = test_dir("alt");
        let p = dir.join("prefs.json");
        std::fs::write(
            &p,
            r#"{"active_preset":0,"last_dir":"/tmp","lineweights_scaled":true,"show_grid":true}"#,
        )
        .unwrap();
        let prefs = UserPrefs::load_from(&p);
        assert_eq!(prefs.drag_mode, DragMode::HoldAndDrag);
        assert!(prefs.lineweights_scaled, "übrige Werte bleiben erhalten");
        assert_eq!(prefs.last_dir, "/tmp");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn zwei_klicks_ueberleben_speichern_und_laden() {
        let dir = test_dir("zwei");
        let p = dir.join("prefs.json");

        let mut prefs = UserPrefs::default();
        prefs.drag_mode = DragMode::TwoClicks;
        prefs.save_to(&p).unwrap();

        let roh = std::fs::read_to_string(&p).unwrap();
        assert!(
            roh.contains("\"drag_mode\": \"two_clicks\""),
            "prefs.json muss den Wert lesbar enthalten:\n{roh}"
        );
        assert!(
            !roh.contains("drag_two_clicks"),
            "Altfeld darf nicht mehr geschrieben werden"
        );

        // Nächster Start
        let wieder = UserPrefs::load_from(&p);
        assert_eq!(wieder.drag_mode, DragMode::TwoClicks);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dateidialog_steht_standardmaessig_auf_system() {
        // Vorgabe: Dialog des Betriebssystems
        assert_eq!(UserPrefs::default().file_dialog, "system");
        assert!(UserPrefs::default().dateidialog_system());
        // Alte prefs.json ohne den Schlüssel → ebenfalls Systemdialog
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert!(alt.dateidialog_system(), "Altbestand nutzt den Systemdialog");
        // Ausdrücklich eingebaut
        let intern: UserPrefs = serde_json::from_str(r#"{"file_dialog": "intern"}"#).unwrap();
        assert!(!intern.dateidialog_system());
    }

    #[test]
    fn dateidialog_ueberlebt_speichern_und_laden() {
        let dir = test_dir("dateidialog");
        let p = dir.join("prefs.json");
        let mut prefs = UserPrefs::default();
        prefs.file_dialog = "intern".into();
        prefs.save_to(&p).unwrap();
        let roh = std::fs::read_to_string(&p).unwrap();
        assert!(
            roh.contains("\"file_dialog\": \"intern\""),
            "prefs.json muss die Wahl enthalten:\n{roh}"
        );
        assert!(!UserPrefs::load_from(&p).dateidialog_system());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// T0045: Ziffern gehören der Hotbar, die Steuerungstasten dem Ablauf.
    /// Beide lassen sich nicht belegen — auch nicht über eine alte prefs.json,
    /// die sie noch trägt (früher stand `modify.select` auf ESC).
    #[test]
    fn ziffern_und_steuerungstasten_sind_nicht_belegbar() {
        assert_eq!(fest_belegt("7"), Some(FestBelegt::Hotbar));
        assert_eq!(fest_belegt("ESC"), Some(FestBelegt::Steuerung));
        assert_eq!(fest_belegt("shift_r"), Some(FestBelegt::Steuerung));
        assert_eq!(fest_belegt("ctrl_r"), Some(FestBelegt::Steuerung));
        assert_eq!(fest_belegt("q"), None);

        let mut prefs = UserPrefs::default();
        for key in ["1", "0", "tab", "caps", "shift", "shift_r", "ctrl", "ctrl_r", "enter", "backspace"] {
            prefs.bind("draw.line", key);
            assert!(
                !prefs
                    .bindings
                    .values()
                    .flatten()
                    .any(|v| v.eq_ignore_ascii_case(key)),
                "Taste {key} ist gesperrt, wurde aber belegt"
            );
        }
        // Der Werkssatz belegt keine Ziffern- oder Steuerungstaste. Funktions-
        // tasten und Kombinationen (Strg+S, F1) sind dagegen die feste Belegung
        // ihrer Funktion (T0056) und stehen im Werkssatz.
        for (aktion, keys) in &UserPrefs::default().bindings {
            for key in keys {
                assert!(
                    !matches!(fest_belegt(key), Some(f) if !f.ist_feste_belegung()),
                    "{aktion} liegt auf der gesperrten Taste {key}"
                );
            }
        }
        // Alte Einstellungen: gesperrte Belegungen fallen beim Laden weg.
        let mut alt = UserPrefs::default();
        alt.bindings.insert("draw.line".into(), vec!["tab".into()]);
        alt.bindings.insert("draw.circle".into(), vec!["k".into()]);
        alt.ensure_defaults();
        assert!(
            !alt
                .bindings
                .values()
                .flatten()
                .any(|v| v.eq_ignore_ascii_case("tab")),
            "alte Belegung auf Tab überlebt das Laden"
        );
        assert_eq!(alt.action_for_key("k").as_deref(), Some("draw.circle"));
    }

    /// T0056: Drei Schichten der Tastatur (`docs/TASTEN.md`). Ziffern gehören
    /// der Hotbar, Buchstaben und Symbole der Tastaturbelegung, Steuerungstasten
    /// und Kombinationen sind fest verdrahtet.
    #[test]
    fn drei_schichten_der_tastatur() {
        assert_eq!(fest_belegt("3"), Some(FestBelegt::Hotbar));
        assert_eq!(fest_belegt("esc"), Some(FestBelegt::Steuerung));
        assert_eq!(fest_belegt(" "), Some(FestBelegt::Steuerung), "Leertaste");
        assert_eq!(fest_belegt("entf"), Some(FestBelegt::Steuerung));
        assert_eq!(fest_belegt("f1"), Some(FestBelegt::Funktion));
        assert_eq!(fest_belegt("f11"), Some(FestBelegt::Funktion));
        assert_eq!(fest_belegt("ctrl+c"), Some(FestBelegt::Kombination));
        assert_eq!(fest_belegt("ctrl+shift+s"), Some(FestBelegt::Kombination));
        // Buchstaben und Symbole bleiben frei — auch die Umlaute des Tastenbilds.
        for frei in ["a", "w", "s", "d", "q", "ü", "ö", "ä", ",", ".", "-", "+"] {
            assert_eq!(fest_belegt(frei), None, "Taste {frei} muss frei belegbar sein");
        }
        // `#`, `<` und `´` sind auf einer deutschen Tastatur Umschalt- bzw.
        // AltGr-Zeichen einer anderen Taste — sie kommen nie einzeln an.
        for zweite in ["#", "<", "´"] {
            assert_eq!(
                fest_belegt(zweite),
                Some(FestBelegt::ZweiteEbene),
                "Zeichen {zweite} gehört zur zweiten Ebene"
            );
        }
        // Nur Ziffern und Steuerungstasten gehören keiner Funktion.
        assert!(FestBelegt::Funktion.ist_feste_belegung());
        assert!(FestBelegt::Kombination.ist_feste_belegung());
        assert!(!FestBelegt::Hotbar.ist_feste_belegung());
        assert!(!FestBelegt::Steuerung.ist_feste_belegung());
    }

    /// T0056: Das Umsetzen der Ansicht sind **Funktionen** auf W/A/S/D — nicht
    /// fest verdrahtete Tasten. Vorher schwenkte `canvas` auf Key::WASD, egal was
    /// in der Belegung stand; die Taste ließ sich also nicht umbelegen und das
    /// Tastenbild zeigte die Funktion nicht.
    #[test]
    fn ansicht_schwenk_ist_belegbar() {
        let prefs = UserPrefs::default();
        for (aktion, taste) in [
            ("view.pan_up", "w"),
            ("view.pan_left", "a"),
            ("view.pan_down", "s"),
            ("view.pan_right", "d"),
        ] {
            assert_eq!(
                prefs.key_for(aktion).as_deref(),
                Some(taste),
                "{aktion} muss im Werkssatz auf {taste} liegen"
            );
            assert_eq!(
                prefs.action_for_key(taste).as_deref(),
                Some(aktion),
                "Taste {taste} gehört dem Schwenk"
            );
            let def = actions::action(aktion).expect("Action fehlt");
            assert!(!def.icon.is_empty(), "{aktion} braucht ein Icon");
            assert!(!def.tool, "{aktion} ist kein Zeichenwerkzeug");
        }
        // Das Stutzen lag früher ebenfalls auf S — eine Taste trägt genau eine
        // Funktion, sonst schwenkt und stutzt ein Druck gleichzeitig.
        assert_eq!(prefs.key_for("modify.trim"), None, "S gehört dem Schwenk");
    }

    /// T0056: Eine Taste trägt genau eine Funktion. Zwei Aktionen auf demselben
    /// Buchstaben machten die Belegung unvorhersehbar (`action_for_key` nimmt
    /// die erste Fundstelle der sortierten Liste).
    #[test]
    fn jede_taste_traegt_genau_eine_funktion() {
        let prefs = UserPrefs::default();
        let mut gesehen: BTreeMap<String, String> = BTreeMap::new();
        for (aktion, keys) in &prefs.bindings {
            for key in keys {
                let k = key.to_lowercase();
                // Kombinationen teilen sich den Buchstaben mit der Einzeltaste
                // (Strg+S und S) — sie sind eine eigene Schicht.
                if k.contains('+') {
                    continue;
                }
                if let Some(alt) = gesehen.insert(k.clone(), aktion.clone()) {
                    panic!("Taste {k} doppelt belegt: {alt} und {aktion}");
                }
            }
        }
        assert!(gesehen.len() >= 20, "zu wenige Einzeltasten: {}", gesehen.len());
    }

    /// T0064: Eine Funktion darf auf mehreren Tasten liegen; eine Taste trägt
    /// weiterhin genau eine Funktion. Die Taste wird der anderen Funktion
    /// entzogen, die alte Belegung der Funktion bleibt erhalten.
    #[test]
    fn funktion_laesst_sich_mehrfach_belegen() {
        let mut prefs = UserPrefs::default();
        assert_eq!(prefs.key_for("draw.wall").as_deref(), Some("g"));
        // Q gehörte der Schnellauswahl — die Taste wandert zur Wand.
        assert_eq!(prefs.action_for_key("q").as_deref(), Some("modify.quickselect"));
        prefs.bind("draw.wall", "q");
        assert_eq!(
            prefs.keys_for("draw.wall"),
            &["g".to_string(), "q".to_string()],
            "die Wand muss auf beiden Tasten liegen"
        );
        assert_eq!(prefs.key_for("draw.wall").as_deref(), Some("g"));
        assert_eq!(prefs.action_for_key("q").as_deref(), Some("draw.wall"));
        assert!(
            prefs.keys_for("modify.quickselect").is_empty(),
            "die Taste darf nicht bei zwei Funktionen liegen"
        );
        assert_eq!(
            prefs.key_for("modify.move").as_deref(),
            Some("v"),
            "fremde Belegungen bleiben unberührt"
        );
        // Dieselbe Taste noch einmal: keine Dublette.
        prefs.bind("draw.wall", "q");
        assert_eq!(prefs.keys_for("draw.wall").len(), 2);
    }

    /// T0064: Die mittlere Maustaste nimmt genau eine Taste aus der Belegung —
    /// die übrigen Tasten derselben Funktion bleiben, die Funktion selbst auch.
    #[test]
    fn unbind_key_nimmt_nur_diese_taste_weg() {
        let mut prefs = UserPrefs::default();
        prefs.bind("draw.wall", "q");
        assert_eq!(prefs.unbind_key("q").as_deref(), Some("draw.wall"));
        assert_eq!(prefs.keys_for("draw.wall"), &["g".to_string()]);
        assert_eq!(prefs.action_for_key("q"), None);
        // Unbelegte Taste: nichts zu entfernen.
        assert_eq!(prefs.unbind_key("q"), None);
        // Alle Tasten einer Funktion entfernen.
        prefs.unbind_action("draw.wall");
        assert!(prefs.keys_for("draw.wall").is_empty());
        assert_eq!(prefs.action_for_key("g"), None);
        assert!(
            prefs.belegung_aenderbar("draw.wall"),
            "die Funktion bleibt umbelegbar"
        );
    }

    /// T0064: Ältere `prefs.json` tragen je Funktion eine einzelne Zeichenkette.
    /// Sie wird als einelementige Liste gelesen; gespeichert wird die Liste.
    #[test]
    fn alte_einzelne_belegung_wird_als_liste_gelesen() {
        let alt: UserPrefs = serde_json::from_str(
            r#"{"bindings": {"draw.line": "l", "draw.wall": ["g", "t"]}}"#,
        )
        .unwrap();
        assert_eq!(alt.keys_for("draw.line"), &["l".to_string()]);
        assert_eq!(alt.keys_for("draw.wall"), &["g".to_string(), "t".to_string()]);
        let mut frisch = UserPrefs::default();
        frisch.bind("draw.wall", "t");
        let roh = serde_json::to_string(&frisch).unwrap();
        assert!(
            roh.contains(r#""draw.wall":["g","t"]"#),
            "gespeichert wird eine Liste:\n{roh}"
        );
    }

    /// T0056: Funktionen mit hart verdrahteter Taste lassen sich nicht umbelegen
    /// — auch nicht über eine alte `prefs.json`.
    #[test]
    fn feste_kombinationen_lassen_sich_nicht_umbelegen() {
        let mut prefs = UserPrefs::default();
        assert!(!prefs.belegung_aenderbar("file.save"));
        assert!(prefs.belegung_aenderbar("draw.line"));

        prefs.bind("file.save", "k");
        assert_eq!(
            prefs.key_for("file.save").as_deref(),
            Some("ctrl+s"),
            "Strg+S muss auf Speichern bleiben"
        );
        assert_eq!(
            prefs.action_for_key("k").as_deref(),
            Some("draw.circle"),
            "die Taste K darf dabei nicht umgehängt werden"
        );

        // Alte Datei: Speichern lag auf K, Stutzen auf S.
        let mut alt = UserPrefs::default();
        alt.bindings.insert("file.save".into(), vec!["k".into()]);
        alt.bindings.insert("modify.trim".into(), vec!["s".into()]);
        alt.ensure_defaults();
        assert_eq!(alt.key_for("file.save").as_deref(), Some("ctrl+s"));
        assert_eq!(alt.key_for("modify.trim"), None, "S gehört jetzt dem Schwenk");
        assert_eq!(alt.action_for_key("s").as_deref(), Some("view.pan_down"));
    }

    #[test]
    fn altes_bool_feld_wird_uebernommen() {
        let an: UserPrefs = serde_json::from_str(r#"{"drag_two_clicks": true}"#).unwrap();
        assert_eq!(an.drag_mode, DragMode::TwoClicks);
        let aus: UserPrefs = serde_json::from_str(r#"{"drag_two_clicks": false}"#).unwrap();
        assert_eq!(aus.drag_mode, DragMode::HoldAndDrag);
        // Neuer Schlüssel hat Vorrang bzw. wird normal gelesen.
        let neu: UserPrefs = serde_json::from_str(r#"{"drag_mode": "two_clicks"}"#).unwrap();
        assert_eq!(neu.drag_mode, DragMode::TwoClicks);
    }
}
