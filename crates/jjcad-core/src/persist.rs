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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPrefs {
    #[serde(default)]
    pub hotbar_presets: Vec<HotbarPreset>,
    #[serde(default = "default_preset_idx")]
    pub active_preset: usize,
    /// Action-ID → Tastenkürzel (z. B. "l", "ctrl+s", "1")
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
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
    /// Zuletzt benutzter Eingabemodus der Fuellflaechen-Schaltflaeche
    /// ("contour" | "points" | "paint")
    #[serde(default)]
    pub fill_input: String,
    /// Dasselbe fuer die Schraffur-Schaltflaeche
    #[serde(default)]
    pub hatch_input: String,
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
}

fn default_preset_idx() -> usize {
    0
}
fn default_true() -> bool {
    true
}

impl Default for UserPrefs {
    fn default() -> Self {
        let mut bindings = BTreeMap::new();
        for a in actions::ACTIONS {
            if !a.key.is_empty() {
                bindings.insert(a.id.to_string(), a.key.to_string());
            }
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
            fill_input: "points".into(),
            hatch_input: "contour".into(),
            // Vorgabe wie bisher im Reiter „Ändern": Kopieren setzt mit
            // Basispunkt ab. Reines Kopieren in die Zwischenablage wählt man im
            // Aufklappmenü oder mit Strg+C.
            copy_input: "copy_base".into(),
            snap_marker: true,
            snap_wall_edges: true,
            show_properties: true,
            show_layers: true,
            inventory: vec![None; 40],
        }
    }
}

impl UserPrefs {
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
        // T gehört seit T0009 dem Chat-Protokoll. Das Text-Werkzeug lag früher
        // auf dieser Taste; eine gespeicherte Belegung würde sie stillschweigend
        // weiter belegen und in der Tastaturbelegung falsch anzeigen.
        if self
            .bindings
            .get("draw.text")
            .map(|k| k.eq_ignore_ascii_case("t"))
            .unwrap_or(false)
        {
            self.bindings.remove("draw.text");
        }
        // T0012: „Kopieren" (Zwischenablage) und „Kopieren mit Basispunkt" sind
        // getrennte Funktionen. Ältere Belegungen tragen Strg+C noch an der
        // Basispunkt-Kopie — die Tasten kommen jetzt aus dem Register.
        let alt = self
            .bindings
            .get("modify.copy")
            .map(|k| k.eq_ignore_ascii_case("ctrl+c"))
            .unwrap_or(false);
        if alt {
            self.bindings
                .insert("modify.copy".into(), "ctrl+shift+c".into());
            self.bindings.insert("edit.copy".into(), "ctrl+c".into());
        }
        // `bindings` wird als Ganzes gespeichert. Eine später ergänzte Aktion
        // bekäme ihre Werktaste sonst nie: fehlende Einträge nachziehen, solange
        // die Taste nicht schon einer anderen Aktion gehört.
        for a in actions::ACTIONS {
            if a.key.is_empty() || self.bindings.contains_key(a.id) {
                continue;
            }
            let frei = !self
                .bindings
                .values()
                .any(|k| k.eq_ignore_ascii_case(a.key));
            if frei {
                self.bindings.insert(a.id.to_string(), a.key.to_string());
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

    pub fn key_for(&self, action: &str) -> Option<String> {
        self.bindings.get(action).cloned()
    }

    /// Aktion für eine Taste (ohne Modifier) — zuletzt definierte gewinnt.
    pub fn action_for_key(&self, key: &str) -> Option<String> {
        let k = key.to_lowercase();
        self.bindings
            .iter()
            .find(|(_, v)| v.to_lowercase() == k)
            .map(|(a, _)| a.clone())
    }

    pub fn bind(&mut self, action: &str, key: &str) {
        let key = key.to_lowercase();
        // Konflikt: andere Aktion auf derselben Taste entfernen
        let conflict: Vec<String> = self
            .bindings
            .iter()
            .filter(|(a, v)| v.to_lowercase() == key && a.as_str() != action)
            .map(|(a, _)| a.clone())
            .collect();
        for c in conflict {
            self.bindings.remove(&c);
        }
        self.bindings.insert(action.to_string(), key);
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
