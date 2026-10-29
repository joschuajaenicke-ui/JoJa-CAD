//! Sitzung: Dokument + Undo-Historie + Auswahl, ohne GUI.
//!
//! Der MCP-Server arbeitet direkt auf dieser Struktur, die GUI hält ebenfalls
//! eine Session pro Zeichnung.

use crate::actions;
use crate::history::{same_content, History};
use crate::model::*;
use crate::persist;
use serde_json::{json, Value};
use std::path::Path;

pub struct Session {
    pub doc: Document,
    pub history: History,
    pub selection: Vec<Id>,
    pub last_action: Option<String>,
    pub last_params: Option<Value>,
    /// Zuletzt erzeugte Objekte (für GUI-Feedback)
    pub last_created: Vec<Id>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            history: History::default(),
            selection: Vec::new(),
            last_action: None,
            last_params: None,
            last_created: Vec::new(),
        }
    }

    pub fn with_doc(doc: Document) -> Self {
        Self {
            doc,
            history: History::default(),
            selection: Vec::new(),
            last_action: None,
            last_params: None,
            last_created: Vec::new(),
        }
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let doc = persist::load_document(path)?;
        Ok(Self::with_doc(doc))
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        persist::save_document(&mut self.doc, path)
    }

    /// Führt eine Aktion aus und pflegt die Undo-Historie.
    pub fn execute(&mut self, action: &str, params: &Value) -> Result<Value, String> {
        let before = self.doc.clone();
        let result = actions::execute(&mut self.doc, action, params)?;
        // Türen/Fenster folgen ihrer Wand (Verschieben, Verdicken, Drehen).
        // Das gilt nur im Modell — im Papierraum gibt es keine Wände.
        if actions::is_mutating(action) {
            if matches!(
                action,
                "modify.move" | "modify.copy" | "modify.rotate" | "modify.scale" | "modify.mirror"
            ) {
                let raum = self.doc.space;
                self.doc.space = None;
                actions::resync_openings(&mut self.doc);
                self.doc.space = raum;
            }
            let raum = self.doc.space;
            self.doc.space = None;
            actions::reflow_openings(&mut self.doc);
            self.doc.space = raum;
        }
        if actions::is_mutating(action) && !same_content(&before, &self.doc) {
            self.history.record(&before);
            self.doc.modified = true;
        }
        if let Some(created) = result.get("created").and_then(|c| c.as_array()) {
            self.last_created = created.iter().filter_map(|v| v.as_u64()).collect();
        }
        if actions::is_mutating(action) {
            self.last_action = Some(action.to_string());
            self.last_params = Some(params.clone());
        }
        // Auswahl aufräumen (nur der aktive Raum zählt)
        let ids: std::collections::HashSet<Id> = self.doc.ents().iter().map(|e| e.id).collect();
        self.selection.retain(|id| ids.contains(id));
        Ok(result)
    }

    /// Bearbeitungsraum: `None` = Modell, `Some(blatt)` = Papierraum.
    pub fn space(&self) -> Option<Id> {
        self.doc.space
    }

    /// Bearbeitungsraum wechseln. Die Auswahl gehört zum Raum und wird beim
    /// Wechsel geleert — sonst stünde im Papierraum die Modellauswahl.
    pub fn set_space(&mut self, space: Option<Id>) {
        let vorher = self.doc.space;
        self.doc.set_space(space);
        if self.doc.space != vorher {
            self.selection.clear();
        }
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.history.undo(&self.doc) {
            self.doc = prev;
            self.doc.modified = true;
            self.selection.clear();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.history.redo(&self.doc) {
            self.doc = next;
            self.doc.modified = true;
            self.selection.clear();
            true
        } else {
            false
        }
    }

    pub fn select(&mut self, ids: Vec<Id>) {
        self.selection = ids;
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn info(&self) -> Value {
        json!({
            "name": self.doc.name,
            "path": self.doc.path,
            "format": self.doc.format,
            "version": self.doc.version,
            "entities": self.doc.entities.len(),
            "layers": self.doc.layers.len(),
            "blocks": self.doc.blocks.len(),
            "sheets": self.doc.sheets.len(),
            "modified": self.doc.modified,
            "units": self.doc.settings.input_unit.short(),
            "dim_unit": self.doc.settings.dim_unit.short(),
            "annotation_scale": self.doc.settings.annotation_scale,
            "can_undo": self.history.can_undo(),
            "can_redo": self.history.can_redo(),
            "selection": self.selection,
            "bbox": {
                "min": self.doc.bbox().min.to_array(),
                "max": self.doc.bbox().max.to_array()
            },
            "summary": self.doc.summary(),
        })
    }

    /// Objektbeschreibung für Abfragen.
    pub fn describe_entity(&self, e: &Entity) -> Value {
        let layer = self.doc.layer(e.layer).map(|l| l.name.clone()).unwrap_or_default();
        let bbox = crate::render::entity_bbox(&self.doc, e);
        let mut base = json!({
            "id": e.id,
            "type": e.kind.type_name(),
            "layer": layer,
            "layer_id": e.layer,
            "color": self.doc.effective_color(e).to_hex(),
            "lineweight": self.doc.effective_line_weight(e),
            "bbox": {"min": bbox.min.to_array(), "max": bbox.max.to_array()},
        });
        if let Value::Object(map) = &mut base {
            if let Ok(Value::Object(k)) = serde_json::to_value(&e.kind) {
                for (key, val) in k {
                    map.insert(key, val);
                }
            }
            // Fläche nur bei Objekten, die etwas umschließen — ein offener
            // Linienzug bekommt kein Feld (und keinen 0-Wert).
            if let Some(mm2) = crate::area::entity_area(e) {
                map.insert(
                    "area_m2".to_string(),
                    json!(crate::units::format_area_m2(mm2)),
                );
                map.insert("area_mm2".to_string(), json!(mm2));
            }
        }
        base
    }

    /// Objekte abfragen — dieselben Filter wie die Action `select.query`:
    /// `ids`, `layer`, `type` und Bereich (`a`/`b`, vom MCP aus `bbox`).
    pub fn query(&self, params: &Value) -> Value {
        let ids = actions::query_ids(&self.doc, params);
        let ents: Vec<Value> = ids
            .iter()
            .filter_map(|id| self.doc.ents().iter().find(|e| e.id == *id))
            .map(|e| self.describe_entity(e))
            .collect();
        json!({"ok": true, "count": ents.len(), "entities": ents})
    }
}
