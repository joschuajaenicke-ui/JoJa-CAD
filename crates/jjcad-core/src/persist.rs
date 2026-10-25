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
    let mut doc: Document =
        serde_json::from_str(s).map_err(|e| format!("Datei ist keine gültige JoJa-CAD-Zeichnung: {e}"))?;
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
    doc.modified = false;
    Ok(doc)
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
    #[serde(default = "default_true")]
    pub snap_marker: bool,
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
            snap_marker: true,
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
        let p = Self::path();
        if let Ok(s) = std::fs::read_to_string(&p) {
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
        if self.active_preset >= self.hotbar_presets.len() {
            self.active_preset = 0;
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&p, s).map_err(|e| format!("Einstellungen konnten nicht gespeichert werden: {e}"))
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
