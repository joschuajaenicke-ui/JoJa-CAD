//! Befehls-/Action-Schicht.
//!
//! GUI und MCP-Server sind nur zwei Frontends auf diese Schicht. Jede Funktion
//! hat eine stabile Action-ID und ist über JSON-Parameter steuerbar.

use crate::blocks_lib;
use ab_glyph::{Font, ScaleFont};
use crate::geom::*;
use crate::model::*;
use crate::render;
use crate::render::Shape;
use crate::style::*;
use crate::templates;
use crate::transform;
use crate::units::*;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Aktionsregister
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionGroup {
    Datei,
    Zeichnen,
    Architektur,
    Aendern,
    Bemassung,
    Bloecke,
    Blatt,
    Ansicht,
    Hilfe,
}

impl ActionGroup {
    pub fn label(self) -> &'static str {
        match self {
            ActionGroup::Datei => "Datei",
            ActionGroup::Zeichnen => "Zeichnen",
            ActionGroup::Architektur => "Architektur",
            ActionGroup::Aendern => "Ändern",
            ActionGroup::Bemassung => "Bemaßung",
            ActionGroup::Bloecke => "Blöcke",
            ActionGroup::Blatt => "Blatt",
            ActionGroup::Ansicht => "Ansicht",
            ActionGroup::Hilfe => "Hilfe",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ActionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub group: ActionGroup,
    /// Icon-Name für den GUI-Zeichner
    pub icon: &'static str,
    /// Standard-Tastenkürzel ("" = keins)
    pub key: &'static str,
    /// Kurzhinweis / Bedienung
    pub hint: &'static str,
    /// Interaktives Werkzeug (braucht Punkt-Eingaben)?
    pub tool: bool,
}

macro_rules! act {
    ($id:expr, $label:expr, $group:ident, $icon:expr, $key:expr, $hint:expr, $tool:expr) => {
        ActionDef {
            id: $id,
            label: $label,
            group: ActionGroup::$group,
            icon: $icon,
            key: $key,
            hint: $hint,
            tool: $tool,
        }
    };
}

pub const ACTIONS: &[ActionDef] = &[
    // Datei
    act!("file.new", "Neu", Datei, "new", "ctrl+n", "Neue Zeichnung anlegen", false),
    act!("file.open", "Öffnen", Datei, "open", "ctrl+o", "Zeichnung (.jjcad) öffnen", false),
    act!("file.save", "Speichern", Datei, "save", "ctrl+s", "Zeichnung speichern", false),
    act!("file.save_as", "Speichern unter", Datei, "save", "ctrl+shift+s", "Speichern unter …", false),
    act!("file.export_pdf", "PDF exportieren", Datei, "pdf", "ctrl+p", "Blatt als PDF exportieren", false),
    act!("file.export_dxf", "DXF exportieren", Datei, "dxf", "", "Zeichnung als DXF exportieren", false),
    act!("file.import_dxf", "DXF importieren", Datei, "import", "", "DXF-Datei importieren", false),
    act!(
        "file.import.pdf",
        "PDF einfügen",
        Datei,
        "pdf",
        "",
        "PDF-Seite als ein Element einfügen (Block, zerlegbar)",
        false
    ),
    act!("file.export_png", "PNG exportieren", Datei, "image", "", "Ansicht als PNG rendern", false),
    act!("edit.undo", "Rückgängig", Aendern, "undo", "ctrl+z", "Letzten Schritt rückgängig", false),
    act!("edit.redo", "Wiederholen", Aendern, "redo", "ctrl+y", "Rückgängig machen rückgängig", false),
    // Kopieren ohne Basispunkt: legt die Auswahl nur in die Zwischenablage.
    // Eingefügt wird danach mit der Vorschau (Strg+V, Knopf „Einfügen").
    act!("edit.copy", "Kopieren", Aendern, "copy", "ctrl+c", "Auswahl in die Zwischenablage", false),

    // Zeichnen
    act!("draw.line", "Linie", Zeichnen, "line", "l", "Start- und Endpunkt klicken", true),
    act!("draw.polyline", "Polylinie", Zeichnen, "polyline", "o", "Punkte klicken, Klick auf den Startpunkt schließt, Enter beendet", true),
    act!("draw.spline", "Spline", Zeichnen, "spline", "u", "Punkte klicken, Klick auf den Startpunkt schließt, Enter beendet", true),
    act!("draw.rect", "Rechteck", Zeichnen, "rect", "", "Zwei Ecken klicken", true),
    act!("draw.circle", "Kreis", Zeichnen, "circle", "k", "Mittelpunkt und Radius klicken", true),
    act!("draw.arc", "Bogen", Zeichnen, "arc", "", "Start, Ende, Bogenpunkt klicken", true),
    act!("draw.point", "Punkt", Zeichnen, "point", "p", "Punkt setzen", true),
    act!("draw.fill", "Füllfläche", Zeichnen, "fill", "", "Geschlossene Kontur klicken", true),
    act!("draw.fill.boundary", "Fläche nach Umriss", Zeichnen, "fill", "i", "Umriss abfahren: Punkte klicken, Enter beendet", true),
    act!("draw.fill.points", "Fläche nach Punktfolge", Zeichnen, "fill", "shift+i", "Punkte klicken, Enter schließt die Fläche", true),
    act!("draw.fill.paint", "Farbeimer", Zeichnen, "fill", "c", "In eine leere Fläche klicken — die Umgrenzung wird ermittelt", true),
    act!("draw.text", "Text", Zeichnen, "text", "", "Position klicken, dann Text eingeben", true),
    act!("draw.ellipse", "Ellipse", Zeichnen, "ellipse", "", "Mittelpunkt und Achsen klicken", true),

    // Architektur
    act!("draw.wall", "Wand", Architektur, "wall", "g", "Wandachse: Start- und Endpunkt", true),
    act!(
        "draw.volume",
        "Volumen",
        Architektur,
        "volume",
        "v",
        "Geschlossene Kontur klicken — der Körper erhält eine Höhe",
        true
    ),
    act!(
        "draw.volume.points",
        "Volumen nach Punktfolge",
        Architektur,
        "volume",
        "shift+v",
        "Punkte klicken, Enter schließt den Körper",
        true
    ),
    act!(
        "draw.volume.paint",
        "Volumen füllen",
        Architektur,
        "volume",
        "",
        "In eine leere Fläche klicken — die Umgrenzung wird ermittelt",
        true
    ),
    act!("draw.door", "Tür", Architektur, "door", "h", "In eine Wand klicken", true),
    act!("draw.window", "Fenster", Architektur, "window", "j", "In eine Wand klicken", true),
    act!(
        "draw.stair",
        "Treppe",
        Architektur,
        "stair",
        "b",
        "Außenkante aufziehen, dann die Breite klicken",
        true
    ),
    act!(
        "draw.stair.label",
        "Treppenbeschriftung",
        Architektur,
        "text",
        "",
        "Stufenzahl und Schrittmaß als Textfeld neben die Treppe setzen",
        false
    ),

    // Ändern
    act!("modify.select", "Auswählen", Aendern, "select", "esc", "Objekte anklicken oder Fenster aufziehen", false),
    act!("modify.move", "Verschieben", Aendern, "move", "v", "Auswahl, Basispunkt, Zielpunkt", true),
    act!("modify.order", "Reihenfolge", Aendern, "layers", "", "to = Platz in der Zeichenreihenfolge (1 = ganz hinten)", true),
    // Kopie versetzt absetzen: erst Basispunkt, dann Zielpunkt. Getrennt vom
    // reinen Kopieren in die Zwischenablage (`edit.copy`, Strg+C).
    act!("modify.copy", "Kopieren mit Basispunkt", Aendern, "copy-base", "ctrl+shift+c", "Auswahl und Basispunkt — Kopie in die Zwischenablage", true),
    act!("modify.rotate", "Drehen", Aendern, "rotate", "r", "Auswahl, Zentrum, Winkel", true),
    act!("modify.mirror", "Spiegeln", Aendern, "mirror", "f", "Auswahl, Spiegelachse", true),
    act!("modify.scale", "Skalieren", Aendern, "scale", "", "Auswahl, Zentrum, Faktor", true),
    act!("modify.offset", "Versetzen", Aendern, "offset", "", "Abstand und Seite klicken", true),
    act!("modify.trim", "Stutzen", Aendern, "trim", "s", "Zu stutzendes Segment anklicken", true),
    act!("modify.extend", "Verlängern", Aendern, "extend", "x", "Ende anklicken, das verlängert wird", true),
    act!("modify.erase", "Löschen", Aendern, "erase", "entf", "Auswahl löschen", false),
    act!("modify.explode", "Zerlegen", Aendern, "explode", "", "Blöcke/Bauteile in Einzelelemente, Text in Linien zerlegen", false),
    act!(
        "text.format",
        "Text auszeichnen",
        Aendern,
        "text",
        "",
        "Zeichenbereich eines Textes fett/kursiv/unterstrichen/hochgestellt setzen",
        false
    ),
    act!("modify.pickprops", "Eigenschaften aufnehmen", Aendern, "eyedropper", "q", "Eigenschaften von Objekt übernehmen", true),

    // Bemaßung
    act!("dim.linear", "Maß", Bemassung, "dim", "m", "Zwei Punkte und Maßlinie setzen", true),
    act!("dim.chain", "Maßkette", Bemassung, "dimchain", "", "Punkte klicken, Enter beendet", true),
    act!("dim.elevation", "Höhenkote", Bemassung, "elevation", "n", "Punkt klicken, Höhe eingeben", true),
    act!("dim.style", "Bemaßungsstil", Bemassung, "style", "", "Bemaßungsstil bearbeiten", false),

    // Blöcke
    act!("block.insert", "Block einfügen", Bloecke, "block", "", "Block aus Inventar platzieren", true),
    act!("block.create", "Block aus Auswahl", Bloecke, "blocknew", "", "Auswahl zu Projektblock machen", false),
    act!("block.explode", "Block zerlegen", Bloecke, "explode", "", "Blockinstanzen auflösen", false),
    act!("block.manager", "Block-Manager", Bloecke, "library", "", "Blockbibliothek öffnen", false),

    // Blatt
    act!("sheet.new", "Neues Blatt", Blatt, "sheet", "", "Blatt anlegen", false),
    act!("sheet.pdf", "Blatt → PDF", Blatt, "pdf", "", "Aktuelles Blatt als PDF", false),
    act!("sheet.add_viewport", "Ansichtsfenster", Blatt, "viewport", "", "Ansichtsfenster hinzufügen", false),
    act!("sheet.viewport_tool", "Ansicht aufziehen", Blatt, "viewport", "", "Rechteck für ein Ansichtsfenster aufziehen", true),

    // Ansicht
    act!("view.zoom_extents", "Alles zeigen", Ansicht, "zoomfit", "", "Zeichnung einpassen", false),
    act!("view.zoom_in", "Vergrößern", Ansicht, "zoomin", "", "Hineinzoomen", false),
    act!("view.zoom_out", "Verkleinern", Ansicht, "zoomout", "", "Herauszoomen", false),
    act!("view.inventory", "Inventar", Ansicht, "inventory", "e", "Inventar ein-/ausblenden (E)", false),
    act!("view.layers", "Layer", Ansicht, "layers", "", "Layerverwaltung", false),
    act!("view.keymap", "Tastaturbelegung", Ansicht, "keyboard", "", "Tastenbelegung bearbeiten", false),
    act!("view.settings", "Einstellungen", Ansicht, "settings", "", "Einheiten und Raster", false),
    act!("view.hotbar_next", "Hotbar-Vorlage", Ansicht, "hotbar", "y", "Nächste Hotbar-Vorlage", false),
    act!("view.help", "Hilfe", Hilfe, "help", "f1", "Tastenbelegung und Kurzhilfe", false),
    act!("view.fullscreen", "Vollbild", Ansicht, "fullscreen", "f11", "Vollbild ein-/ausschalten (F11)", false),
    act!("view.rotate_tool", "Ansicht drehen", Ansicht, "rotate", "", "Ansicht um ein Drehzentrum drehen (drei Klicks)", true),
    act!("edit.repeat", "Wiederholen", Aendern, "repeat", " ", "Letzten Befehl wiederholen", false),

    // Interne/fortgeschrittene Aktionen (MCP + Eigenschaften-Panel)
    act!("modify.set", "Eigenschaften ändern", Aendern, "properties", "", "Layer, Farbe, Maße setzen", false),
    act!("layer.create", "Layer anlegen", Ansicht, "layers", "", "Neuen Layer anlegen", false),
    act!("layer.update", "Layer ändern", Ansicht, "layers", "", "Layer umbenennen/ändern", false),
    act!("layer.delete", "Layer löschen", Ansicht, "layers", "", "Leeren Layer löschen", false),
    act!("layer.list", "Layer auflisten", Ansicht, "layers", "", "Alle Layer mit Eigenschaften", false),
    act!("layer.set_current", "Aktueller Layer", Ansicht, "layers", "", "Zeichenlayer wechseln", false),
    act!("select.all", "Alles auswählen", Aendern, "select", "ctrl+a", "Gesamte Zeichnung auswählen", false),
    act!("select.query", "Objekte abfragen", Aendern, "select", "", "Nach Layer/Typ/Bereich filtern", false),
    act!("block.update", "Block aktualisieren", Bloecke, "blocknew", "", "Blockdefinition aus Auswahl ersetzen", false),
    act!("block.delete", "Block löschen", Bloecke, "library", "", "Blockdefinition entfernen", false),
    act!("block.list", "Blöcke auflisten", Bloecke, "library", "", "Alle Blöcke mit Anzahl", false),
    act!("sheet.update", "Blatt bearbeiten", Blatt, "sheet", "", "Blattgröße, Maßstab, Rahmen", false),
    act!("sheet.delete", "Blatt löschen", Blatt, "sheet", "", "Blatt entfernen", false),
    act!("sheet.update_viewport", "Ansicht ändern", Blatt, "viewport", "", "Ansichtsfenster anpassen", false),
    act!("sheet.remove_viewport", "Ansicht löschen", Blatt, "viewport", "", "Ansichtsfenster entfernen", false),
    act!("doc.settings", "Zeichnungseinstellungen", Ansicht, "settings", "", "Einheiten, Raster, Vorgaben", false),
    act!("doc.project", "Projektdaten", Datei, "info", "", "Bauherr, Projekt, Plan-Nr.", false),
];

pub fn action(id: &str) -> Option<&'static ActionDef> {
    ACTIONS.iter().find(|a| a.id == id)
}

pub fn actions_in_group(g: ActionGroup) -> Vec<&'static ActionDef> {
    ACTIONS.iter().filter(|a| a.group == g).collect()
}

/// Aktionen, die das Dokument verändern.
pub fn is_mutating(id: &str) -> bool {
    matches!(
        id,
        "draw.line"
            | "draw.polyline"
            | "draw.spline"
            | "draw.rect"
            | "draw.circle"
            | "draw.arc"
            | "draw.point"
            | "draw.fill"
            | "draw.fill.boundary"
            | "draw.fill.points"
            | "draw.fill.paint"
            | "draw.hatch"
            | "draw.text"
            | "draw.ellipse"
            | "draw.wall"
            | "draw.volume"
            | "draw.volume.points"
            | "draw.volume.paint"
            | "draw.door"
            | "draw.window"
            | "draw.stair"
            | "dim.linear"
            | "dim.chain"
            | "dim.elevation"
            | "dim.style"
            | "modify.move"
            | "modify.order"
            | "modify.copy"
            | "modify.rotate"
            | "modify.mirror"
            | "modify.scale"
            | "modify.offset"
            | "modify.trim"
            | "modify.extend"
            | "modify.erase"
            | "modify.explode"
            | "text.format"
            | "modify.set"
            | "layer.create"
            | "layer.update"
            | "layer.delete"
            | "block.create"
            | "block.insert"
            | "block.update"
            | "block.delete"
            | "file.import.pdf"
            | "sheet.new"
            | "sheet.update"
            | "sheet.delete"
            | "sheet.add_viewport"
            | "sheet.update_viewport"
            | "sheet.remove_viewport"
            | "doc.settings"
            | "doc.project"
            | "doc.layers_restore"
    )
}

// ---------------------------------------------------------------------------
// Parameter-Zugriff
// ---------------------------------------------------------------------------

pub struct Params<'a> {
    v: &'a Value,
    /// Einheit für Längenangaben
    pub unit: Unit,
}

impl<'a> Params<'a> {
    pub fn new(v: &'a Value, default_unit: Unit) -> Self {
        let unit = v
            .get("units")
            .and_then(|u| u.as_str())
            .and_then(Unit::from_str_loose)
            .unwrap_or(default_unit);
        Self { v, unit }
    }

    fn raw(&self, k: &str) -> Option<&Value> {
        self.v.get(k)
    }

    pub fn has(&self, k: &str) -> bool {
        self.v.get(k).map(|x| !x.is_null()).unwrap_or(false)
    }

    pub fn num(&self, k: &str) -> Option<f64> {
        match self.raw(k)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.replace(',', ".").parse().ok(),
            _ => None,
        }
    }

    pub fn num_or(&self, k: &str, d: f64) -> f64 {
        self.num(k).unwrap_or(d)
    }

    /// Länge in mm (Eingabeeinheit wird umgerechnet)
    pub fn len(&self, k: &str) -> Option<f64> {
        self.num(k).map(|n| self.unit.to_mm(n))
    }

    pub fn len_or(&self, k: &str, d_mm: f64) -> f64 {
        self.len(k).unwrap_or(d_mm)
    }

    /// Winkel in Grad → Bogenmaß
    pub fn angle(&self, k: &str) -> Option<f64> {
        self.num(k).map(|d| d.to_radians())
    }

    pub fn angle_or(&self, k: &str, d: f64) -> f64 {
        self.angle(k).unwrap_or(d)
    }

    pub fn boolean(&self, k: &str) -> Option<bool> {
        match self.raw(k)? {
            Value::Bool(b) => Some(*b),
            Value::String(s) => Some(matches!(s.to_lowercase().as_str(), "1" | "true" | "ja" | "yes")),
            Value::Number(n) => Some(n.as_f64().unwrap_or(0.0) != 0.0),
            _ => None,
        }
    }

    pub fn boolean_or(&self, k: &str, d: bool) -> bool {
        self.boolean(k).unwrap_or(d)
    }

    pub fn text(&self, k: &str) -> Option<String> {
        match self.raw(k)? {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    pub fn text_or(&self, k: &str, d: &str) -> String {
        self.text(k).unwrap_or_else(|| d.to_string())
    }

    /// Punkt in mm: [x,y] oder {"x":..,"y":..}
    pub fn pt(&self, k: &str) -> Option<Vec2> {
        let val = self.raw(k)?;
        if let Some(arr) = val.as_array() {
            if arr.len() >= 2 {
                let x = arr[0].as_f64()?;
                let y = arr[1].as_f64()?;
                return Some(v(x * self.unit.mm(), y * self.unit.mm()));
            }
        }
        if let Value::Object(o) = val {
            let x = o.get("x")?.as_f64()?;
            let y = o.get("y")?.as_f64()?;
            return Some(v(x * self.unit.mm(), y * self.unit.mm()));
        }
        if let Some(s) = val.as_str() {
            let parts: Vec<&str> = s.split([',', ';']).collect();
            if parts.len() == 2 {
                let x = parse_length(parts[0], self.unit)?;
                let y = parse_length(parts[1], self.unit)?;
                return Some(v(x, y));
            }
        }
        None
    }

    pub fn pt_or(&self, k: &str, d: Vec2) -> Vec2 {
        self.pt(k).unwrap_or(d)
    }

    pub fn pts(&self, k: &str) -> Option<Vec<Vec2>> {
        let arr = self.raw(k)?.as_array()?;
        self.pts_from(arr)
    }

    /// Ringe (Löcher einer Fläche): `[[[x,y], …], [[x,y], …]]`.
    ///
    /// Dieselbe Einheitenrechnung wie [`Params::pts`] — ein Loch darf nicht in
    /// einer anderen Einheit ankommen als sein Außenring.
    pub fn rings(&self, k: &str) -> Option<Vec<Vec<Vec2>>> {
        let arr = self.raw(k)?.as_array()?;
        let mut out = Vec::new();
        for ring in arr {
            let pts = self.pts_from(ring.as_array()?)?;
            out.push(pts);
        }
        Some(out)
    }

    fn pts_from(&self, arr: &[Value]) -> Option<Vec<Vec2>> {
        let mut out = Vec::new();
        for item in arr {
            let p = if let Some(a) = item.as_array() {
                if a.len() >= 2 {
                    Some(v(a[0].as_f64()? * self.unit.mm(), a[1].as_f64()? * self.unit.mm()))
                } else {
                    None
                }
            } else if let Some(o) = item.as_object() {
                Some(v(
                    o.get("x")?.as_f64()? * self.unit.mm(),
                    o.get("y")?.as_f64()? * self.unit.mm(),
                ))
            } else {
                None
            };
            out.push(p?);
        }
        Some(out)
    }

    /// Einzel-ID (Zahl oder String-Zahl)
    pub fn id(&self, k: &str) -> Option<Id> {
        match self.raw(k)? {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.trim().parse().ok(),
            Value::Array(a) => a.first().and_then(|x| x.as_u64()),
            _ => None,
        }
    }

    pub fn ids(&self, k: &str) -> Option<Vec<Id>> {
        let arr = self.raw(k)?.as_array()?;
        Some(
            arr.iter()
                .filter_map(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
                .collect(),
        )
    }

    pub fn color(&self, k: &str) -> Option<Color> {
        match self.raw(k)? {
            Value::String(s) => Color::from_hex(s),
            _ => None,
        }
    }

    pub fn linetype(&self, k: &str) -> Option<LineType> {
        let s = self.text(k)?.to_lowercase();
        Some(match s.as_str() {
            "solid" | "durchgezogen" => LineType::Solid,
            "dashed" | "gestrichelt" => LineType::Dashed,
            "dotted" | "gepunktet" => LineType::Dotted,
            "dashdot" | "strichpunkt" => LineType::DashDot,
            "hidden" | "verdeckt" => LineType::Hidden,
            _ => return None,
        })
    }

    pub fn fill_pattern(&self, k: &str) -> Option<FillPattern> {
        FillPattern::from_key(&self.text(k)?)
    }

    pub fn dim_axis(&self, k: &str) -> Option<DimAxis> {
        match self.text(k)?.to_lowercase().as_str() {
            "x" | "horizontal" | "waagerecht" => Some(DimAxis::X),
            "y" | "vertical" | "senkrecht" => Some(DimAxis::Y),
            "aligned" | "parallel" => Some(DimAxis::Aligned),
            _ => None,
        }
    }

    pub fn door_kind(&self, k: &str) -> Option<DoorKind> {
        match self.text(k)?.to_lowercase().as_str() {
            "hinge" | "drehtuer" | "drehtür" | "swing" => Some(DoorKind::Hinge),
            "sliding" | "schiebetuer" | "schiebetür" => Some(DoorKind::Sliding),
            "double" | "zweifluegelig" | "zweiflügelig" => Some(DoorKind::Double),
            "opening" | "oeffnung" | "öffnung" => Some(DoorKind::Opening),
            _ => None,
        }
    }

    pub fn text_align(&self, k: &str) -> Option<TextAlignH> {
        match self.text(k)?.to_lowercase().as_str() {
            "left" | "links" => Some(TextAlignH::Left),
            "center" | "mitte" => Some(TextAlignH::Center),
            "right" | "rechts" => Some(TextAlignH::Right),
            _ => None,
        }
    }

    /// Senkrechte Ausrichtung von Textfeldern (`valign`).
    pub fn text_valign(&self, k: &str) -> Option<TextAlignV> {
        match self.text(k)?.to_lowercase().as_str() {
            "top" | "oben" => Some(TextAlignV::Top),
            "middle" | "mitte" | "zentriert" => Some(TextAlignV::Middle),
            "bottom" | "unten" => Some(TextAlignV::Bottom),
            _ => None,
        }
    }
}

fn require_pt(p: &Params, k: &str) -> Result<Vec2, String> {
    p.pt(k)
        .ok_or_else(|| format!("Parameter '{k}' (Punkt [x,y]) fehlt"))
}

/// Layer aus Parameter `layer` (Name oder ID), sonst aktueller Layer.
fn resolve_layer(doc: &mut Document, p: &Params) -> Id {
    if let Some(val) = p.v.get("layer") {
        if let Some(id) = val.as_u64() {
            if doc.layer(id).is_some() {
                return id;
            }
        }
        if let Some(name) = val.as_str() {
            if let Some(id) = doc.layer_id_by_name(name) {
                return id;
            }
            // unbekannter Layer wird angelegt
            if !name.is_empty() {
                return doc.ensure_layer(name, Color::BLACK);
            }
        }
    }
    doc.current_layer
}


/// Verschiebt den Mittelpunkt einer Öffnung quer zur Wand (Ausrichtung in der Wanddicke).
pub fn offset_opening(
    doc: &Document,
    host: Option<Id>,
    center: Vec2,
    angle: f64,
    align: crate::style::WallAlign,
) -> Vec2 {
    let Some(hid) = host else {
        return center;
    };
    let Some(EntityKind::Wall {
        thickness,
        align: wall_align,
        ..
    }) = doc.entity(hid).map(|w| &w.kind)
    else {
        return center;
    };
    // Öffnung sitzt im Wandkörper: Versatz der Wand selbst + Lage innerhalb der Dicke
    let h = thickness * 0.5;
    let shift = wall_align.offset() + 0.5 * align.offset();
    if shift.abs() < 1e-9 {
        return center;
    }
    let n = Vec2::polar(angle + std::f64::consts::FRAC_PI_2, 1.0);
    center + n * (h * shift)
}

/// Türen sitzen immer auf der vorderen Wandkante — also auf der Fläche, zu der
/// das Blatt aufschlägt. Lage längs der Wand bleibt dabei erhalten.
pub fn door_face_center(
    doc: &Document,
    host: Option<Id>,
    center: Vec2,
    angle: f64,
    swing_in: bool,
) -> Vec2 {
    let Some(hid) = host else {
        return center;
    };
    let Some(EntityKind::Wall {
        a,
        b,
        thickness,
        align: wall_align,
        ..
    }) = doc.entity(hid).map(|w| &w.kind)
    else {
        return center;
    };
    let dir = (*b - *a).normalized();
    let axis = *a + dir * (center - *a).dot(dir);
    let n = Vec2::polar(angle + std::f64::consts::FRAC_PI_2, 1.0);
    let side = if swing_in { 1.0 } else { -1.0 };
    axis + n * (thickness * 0.5 * (wall_align.offset() + side))
}

/// Länge längs der Wandachse ab Wandanfang (mm).
fn wall_at(doc: &Document, host: Option<Id>, p: Vec2) -> Option<f64> {
    let hid = host?;
    let EntityKind::Wall { a, b, .. } = &doc.entity(hid)?.kind else {
        return None;
    };
    let dir = (*b - *a).normalized();
    Some((p - *a).dot(dir))
}

/// Lage einer Öffnung aus der Wandgeometrie: Position längs (`at`) und Richtung.
/// `side` = Some(Blattseite) für Türen (vordere Kante), None für Fenster.
fn opening_from_wall(
    doc: &Document,
    host: Option<Id>,
    at: f64,
    side: Option<bool>,
    align: crate::style::WallAlign,
) -> Option<(Vec2, f64)> {
    let hid = host?;
    let EntityKind::Wall {
        a,
        b,
        thickness,
        align: wall_align,
        ..
    } = &doc.entity(hid)?.kind
    else {
        return None;
    };
    let dir = (*b - *a).normalized();
    let angle = dir.angle();
    let n = Vec2::polar(angle + std::f64::consts::FRAC_PI_2, 1.0);
    let offset = match side {
        // Tür: vordere Kante auf der Blattseite
        Some(sw) => thickness * 0.5 * (wall_align.offset() + if sw { 1.0 } else { -1.0 }),
        // Fenster: Lage innerhalb der Wanddicke
        None => thickness * 0.5 * (wall_align.offset() + 0.5 * align.offset()),
    };
    Some((*a + dir * at + n * offset, angle))
}

/// Längslage einer Öffnung aus ihrer aktuellen Position neu bestimmen.
pub fn resync_openings(doc: &mut Document) {
    let mut fix: Vec<(Id, f64)> = Vec::new();
    for e in &doc.entities {
        if let EntityKind::Door { host, center, .. } | EntityKind::Window { host, center, .. } =
            &e.kind
        {
            if host.is_some() {
                if let Some(t) = wall_at(&*doc, *host, *center) {
                    fix.push((e.id, t));
                }
            }
        }
    }
    for (id, t) in fix {
        if let Some(e) = doc.entity_mut(id) {
            match &mut e.kind {
                EntityKind::Door { at, .. } | EntityKind::Window { at, .. } => *at = Some(t),
                _ => {}
            }
        }
    }
}

/// Türen/Fenster, die in einer der übergebenen Wände sitzen (wandern mit).
pub fn carried_openings(doc: &Document, walls: &[Id]) -> Vec<Id> {
    doc.entities
        .iter()
        .filter(|e| match &e.kind {
            EntityKind::Door { host, .. } | EntityKind::Window { host, .. } => {
                host.map(|h| walls.contains(&h)).unwrap_or(false)
            }
            _ => false,
        })
        .map(|e| e.id)
        .collect()
}

/// Öffnungen folgen ihrer Wand: wird nach jeder mutierenden Aktion aufgerufen,
/// damit Türen und Fenster beim Verschieben oder Verdicken der Wand mitlaufen.
pub fn reflow_openings(doc: &mut Document) {
    let mut updates: Vec<(Id, Vec2, f64)> = Vec::new();
    for e in &doc.entities {
        let found = match &e.kind {
            EntityKind::Door {
                host,
                at,
                swing_in,
                ..
            } => at.and_then(|t| {
                opening_from_wall(
                    &*doc,
                    *host,
                    t,
                    Some(*swing_in),
                    crate::style::WallAlign::Center,
                )
            }),
            EntityKind::Window { host, at, align, .. } => at.and_then(|t| {
                opening_from_wall(&*doc, *host, t, None, *align)
            }),
            _ => None,
        };
        if let Some((c, ang)) = found {
            updates.push((e.id, c, ang));
        }
    }
    for (id, c, ang) in updates {
        if let Some(e) = doc.entity_mut(id) {
            match &mut e.kind {
                EntityKind::Door { center, angle, .. }
                | EntityKind::Window { center, angle, .. } => {
                    *center = c;
                    *angle = ang;
                }
                _ => {}
            }
        }
    }
}

/// Datei-Migration: Längslage fehlender Öffnungen ergänzen und alle Öffnungen
/// anschließend aus ihrer Wand neu setzen.
pub fn normalize_openings(doc: &mut Document) {
    let mut fix: Vec<(Id, f64)> = Vec::new();
    for e in &doc.entities {
        let mig = match &e.kind {
            EntityKind::Door { host, at, center, .. }
            | EntityKind::Window { host, at, center, .. } => {
                if at.is_none() {
                    wall_at(&*doc, *host, *center).map(|t| (e.id, t))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(m) = mig {
            fix.push(m);
        }
    }
    for (id, t) in fix {
        if let Some(e) = doc.entity_mut(id) {
            match &mut e.kind {
                EntityKind::Door { at, .. } | EntityKind::Window { at, .. } => *at = Some(t),
                _ => {}
            }
        }
    }
    reflow_openings(doc);
}

/// Setzt eine Öffnung erneut in den Wandkörper: Lage entlang der Wand bleibt,
/// der Abstand quer zur Achse folgt der Ausrichtung (links/mittig/rechts).
fn realign_opening(
    doc: &Document,
    host: Option<Id>,
    center: Vec2,
    angle: f64,
    align: crate::style::WallAlign,
) -> Vec2 {
    let Some(hid) = host else {
        return center;
    };
    let Some(EntityKind::Wall {
        a,
        b,
        thickness,
        align: wall_align,
        ..
    }) = doc.entity(hid).map(|w| &w.kind)
    else {
        return center;
    };
    let dir = (*b - *a).normalized();
    // Projektion auf die Wandachse hält die Position längs der Wand
    let axis = *a + dir * (center - *a).dot(dir);
    let n = Vec2::polar(angle + std::f64::consts::FRAC_PI_2, 1.0);
    let shift = wall_align.offset() + 0.5 * align.offset();
    axis + n * (thickness * 0.5 * shift)
}

/// Eigenen Linienstil eines Treppenteils aus den Parametern setzen.
///
/// `praefix` ist `unten` oder `oben`; gelesen werden `color_unten`,
/// `weight_unten` und `type_unten` (bzw. `_oben`). Ohne eigene Werte bleibt der
/// Teil beim Objektstil — `None` heißt genau das.
fn stair_teil_setzen(ziel: &mut Option<EntityStyle>, basis: &EntityStyle, p: &Params, praefix: &str) {
    let farbe = p.v.get(format!("color_{praefix}"));
    let staerke = p.num(&format!("weight_{praefix}"));
    let typ = p.linetype(&format!("type_{praefix}"));
    if farbe.is_none() && staerke.is_none() && typ.is_none() {
        return;
    }
    let mut st = ziel.clone().unwrap_or_else(|| basis.clone());
    if let Some(v) = farbe {
        st.color = match v.as_str() {
            Some(s) if s.eq_ignore_ascii_case("bylayer") => None,
            Some(s) => Color::from_hex(s).or(st.color),
            None => st.color,
        };
    }
    if let Some(w) = staerke {
        st.line_weight = Some(w);
    }
    if let Some(t) = typ {
        st.line_type = Some(t);
    }
    *ziel = Some(st);
}

fn apply_common(doc: &mut Document, e: &mut Entity, p: &Params, layer: Id) {
    e.layer = layer;
    if let Some(c) = p.color("color") {
        e.style.color = Some(c);
    } else if let Some(s) = p.text("color") {
        if s.eq_ignore_ascii_case("bylayer") {
            e.style.color = None;
        }
    }
    if let Some(lt) = p.linetype("linetype") {
        e.style.line_type = Some(lt);
    }
    if let Some(lw) = p.num("lineweight") {
        e.style.line_weight = Some(lw);
    }
    if let Some(c) = p.color("fill") {
        e.style.fill = Some(c);
    }
    let _ = doc;
}

fn ids_from(p: &Params, key: &str) -> Vec<Id> {
    p.ids(key).unwrap_or_default()
}

/// Läufe aus einem JSON-Array lesen: `[{"text": "…", "bold": true}, …]`.
fn parse_runs(v: &Value) -> Result<Vec<TextRun>, String> {
    let Some(arr) = v.as_array() else {
        return Err("Liste erwartet".into());
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let text = item
            .get("text")
            .and_then(|t| t.as_str())
            .ok_or_else(|| "jeder Lauf braucht 'text'".to_string())?;
        let flag = |k: &str| {
            item.get(k)
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
        };
        out.push(TextRun {
            text: text.to_string(),
            bold: flag("bold"),
            italic: flag("italic"),
            underline: flag("underline"),
            superscript: flag("superscript"),
        });
    }
    Ok(out)
}

/// Gemeinsamer Abschluss aller Füll-Werkzeuge: aus einer geschlossenen Kontur
/// wird genau eine Füllfläche.
///
/// Jede Eingabe — Kontur anklicken (`draw.fill`), Umriss abfahren
/// (`draw.fill.boundary`), freie Punktfolge (`draw.fill.points`), der Farbeimer
/// (`draw.fill.paint`) und der alte Name `draw.hatch` — läuft hier durch. Damit
/// setzen alle Werkzeuge Muster, Winkel, Abstand, Umrandung und Stil an
/// derselben Stelle und erzeugen zwangsläufig dasselbe Objekt; ein zweiter
/// Konstruktionsweg würde früher oder später ein Feld anders setzen.
///
/// `rand_vorgabe` ist die Umrandung des jeweiligen Erstellmodus: Punkte setzen
/// zeichnet sie (die Fläche hat dort keine vorhandene Kante), Kontur anklicken
/// und Farbeimer nicht (dort liegt die Umgrenzung bereits auf einer Linie oder
/// Wand — eine zweite Kante würde sie optisch dicker machen). Ein explizites
/// `border` im Aufruf sticht die Vorgabe.
fn add_fill(
    doc: &mut Document,
    boundary: Vec<Vec2>,
    p: &Params,
    rand_vorgabe: bool,
) -> Result<Value, String> {
    // Inseln kommen als Ringe mit: der Farbeimer (`detect_boundary`) liefert
    // sie aus derselben Flutfüllung. Ohne sie würde er Schornstein, Treppe und
    // Pfeiler übermalen. Werkzeuge ohne Löcher lassen den Schlüssel weg.
    let holes: Vec<Vec<Vec2>> = p
        .rings("holes")
        .unwrap_or_default()
        .into_iter()
        .filter(|h| h.len() >= 3)
        .collect();
    // Kein Randobjekt bekannt (Kontur aus Punkten): die Fläche wird angehängt.
    add_fill_rings(doc, boundary, holes, p, rand_vorgabe, &[])
}

/// Wie [`add_fill`], aber mit bereits ermittelten Ringen und einstellbarer
/// Vorgabe für die Umrandung.
///
/// Der Farbeimer rechnet seine Ringe selbst (`detect_boundary`) und setzt
/// `rand_vorgabe = false`: seine Umgrenzung liegt exakt auf der Wand oder auf
/// der Linie, die die Fläche schließt. Eine Umrandung würde dort eine zweite
/// Kante auf die vorhandene legen und die Wand optisch dicker machen. Die
/// Umgrenzung wird nie als eigene Linien im Dokument abgelegt — nur die
/// Flächengeometrie.
fn add_fill_rings(
    doc: &mut Document,
    boundary: Vec<Vec2>,
    holes: Vec<Vec<Vec2>>,
    p: &Params,
    rand_vorgabe: bool,
    unter: &[Id],
) -> Result<Value, String> {
    add_filled(doc, boundary, holes, p, rand_vorgabe, unter, false)
}

/// Gemeinsamer Abschluss von Füllfläche und Volumen.
///
/// Beide haben dieselbe Geometrie (Außenring + Inseln, Muster, Randlinie);
/// das Volumen trägt zusätzlich eine Höhe. Ein zweiter Konstruktionsweg würde
/// früher oder später ein Feld anders setzen.
fn add_filled(
    doc: &mut Document,
    boundary: Vec<Vec2>,
    holes: Vec<Vec<Vec2>>,
    p: &Params,
    rand_vorgabe: bool,
    unter: &[Id],
    volumen: bool,
) -> Result<Value, String> {
    if boundary.len() < 3 {
        return Err("Füllfläche braucht mindestens drei Punkte".into());
    }
    let layer = resolve_layer(doc, p);
    let idn = doc.new_id();
    let pattern = p.fill_pattern("pattern").unwrap_or(if volumen {
        FillPattern::Solid
    } else {
        FillPattern::Lines
    });
    let pattern_angle = p.angle_or("angle", std::f64::consts::FRAC_PI_4);
    let pattern_scale = p
        .len("scale")
        .or_else(|| p.len("spacing"))
        .unwrap_or(100.0);
    let border = p.boolean_or("border", rand_vorgabe);
    let kind = if volumen {
        EntityKind::Volume {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
            height: p.len_or("height", 2750.0).max(1.0),
        }
    } else {
        EntityKind::Fill {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
        }
    };
    let mut e = Entity::new(idn, layer, kind);
    apply_common(doc, &mut e, p, layer);
    // Zeichenreihenfolge: die Liste `entities` IST die Reihenfolge, Index 0
    // liegt ganz hinten. Die Füllfläche gehört unter die Objekte, die sie
    // begrenzen — sonst überdeckt ihre Fläche deren Linien und der Rand des
    // Raums verschwindet. Sie kommt deshalb direkt hinter das vorderste
    // Randobjekt, also an dessen Platz; die übrigen Objekte rücken nur nach
    // hinten und behalten ihre relative Lage. Ohne Randobjekte (Kontur aus
    // freien Punkten) bleibt es beim Anhängen wie bisher.
    let ziel = unter
        .iter()
        .filter_map(|id| doc.ents().iter().position(|o| o.id == *id))
        .min()
        .unwrap_or(doc.ents().len());
    doc.ents_mut().insert(ziel, e);
    doc.modified = true;
    Ok(json!({"ok": true, "created": [idn], "position": ziel + 1}))
}

// ---------------------------------------------------------------------------
// Ausführung
// ---------------------------------------------------------------------------

/// Führt eine Aktion aus. Rückgabe: JSON mit Ergebnisdetails.
pub fn execute(doc: &mut Document, id: &str, params: &Value) -> Result<Value, String> {
    let p = Params::new(params, doc.settings.input_unit);

    match id {
        // ---------------- Zeichnen ----------------
        "draw.point" => {
            let pt = require_pt(&p, "p").or_else(|_| require_pt(&p, "at"))?;
            let layer = resolve_layer(doc, &p);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Point { p: pt });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [id]}))
        }
        "draw.line" => {
            let a = require_pt(&p, "a")?;
            let b = require_pt(&p, "b")?;
            let layer = resolve_layer(doc, &p);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Line { a, b });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [id], "length_mm": a.dist(b)}))
        }
        "draw.circle" => {
            let c = p.pt("c").or_else(|| p.pt("center")).unwrap_or(Vec2::ZERO);
            let r = if let Some(r) = p.len("r").or_else(|| p.len("radius")) {
                r
            } else if let Some(through) = p.pt("through") {
                c.dist(through)
            } else {
                return Err("Parameter 'r' (Radius) oder 'through' fehlt".into());
            };
            let layer = resolve_layer(doc, &p);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Circle { c, r });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [id], "radius_mm": r}))
        }
        "draw.arc" => {
            let c = require_pt(&p, "c")?;
            let r = p
                .len("r")
                .ok_or_else(|| "Parameter 'r' fehlt".to_string())?;
            let a0 = p.angle_or("a0", 0.0);
            let a1 = p.angle_or("a1", std::f64::consts::PI);
            let layer = resolve_layer(doc, &p);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Arc { c, r, a0, a1 });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [id]}))
        }
        "draw.ellipse" => {
            let c = p.pt("c").or_else(|| p.pt("center")).unwrap_or(Vec2::ZERO);
            let rx = p.len("rx").unwrap_or(500.0);
            let ry = p.len("ry").unwrap_or(300.0);
            let rot = p.angle_or("rot", 0.0);
            let layer = resolve_layer(doc, &p);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Ellipse { c, rx, ry, rot });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [id]}))
        }
        "draw.polyline" | "draw.rect" => {
            let pts = if id == "draw.rect" {
                let a = require_pt(&p, "a")?;
                let b = require_pt(&p, "b")?;
                // Mit `height` ist `a`→`b` eine **Basislinie** beliebiger Lage: das
                // Rechteck steht senkrecht darauf, die Höhe ist der Abstand zur
                // Basislinie (Vorzeichen wählt die Seite — positiv = links der
                // Richtung a→b). Ohne `height` bleibt es achsenparallel über zwei
                // gegenüberliegende Ecken.
                match p.len("height") {
                    Some(h) => {
                        let d = b - a;
                        if d.len() <= POINT_EPS_MM {
                            return Err("Basislinie hat keine Länge".into());
                        }
                        if h.abs() <= POINT_EPS_MM {
                            return Err("Höhe darf nicht null sein".into());
                        }
                        let n = d.perp().normalized() * h;
                        vec![a, b, b + n, a + n]
                    }
                    None => vec![a, v(b.x, a.y), b, v(a.x, b.y)],
                }
            } else {
                p.pts("pts")
                    .or_else(|| p.pts("points"))
                    .ok_or_else(|| "Parameter 'pts' fehlt".to_string())?
            };
            if pts.len() < 2 {
                return Err("Mindestens zwei Punkte nötig".into());
            }
            // Geschlossener Zug: ein Schlusspunkt auf dem Startpunkt wird nicht
            // als eigener Stützpunkt gespeichert (siehe `polyline_points`).
            // Das gilt für den ausdrücklichen Parameter `closed` genauso wie für
            // „der letzte Punkt ist der erste" — beide ergeben dieselbe Geometrie.
            let closed_req = if id == "draw.rect" {
                true
            } else {
                p.boolean_or("closed", false)
            };
            let (pts, closed) = polyline_points(&pts, closed_req, POINT_EPS_MM);
            if pts.len() < 2 {
                return Err("Mindestens zwei verschiedene Punkte nötig".into());
            }
            let layer = resolve_layer(doc, &p);
            let idn = doc.new_id();
            let verts = pts.iter().map(|q| Vertex::new(*q)).collect();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Polyline {
                    verts,
                    closed,
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Spline: dieselbe Eingabe wie die Polylinie (Punktfolge, Klick auf den
        // Startpunkt schließt), aber als Catmull-Rom-Kurve gespeichert. Die
        // Stützpunkte sind die geklickten Punkte — Tangenten gibt es nicht.
        "draw.spline" => {
            let pts = p
                .pts("pts")
                .or_else(|| p.pts("points"))
                .ok_or_else(|| "Parameter 'pts' fehlt".to_string())?;
            if pts.len() < 2 {
                return Err("Mindestens zwei Punkte nötig".into());
            }
            let closed_req = p.boolean_or("closed", false);
            let (pts, closed) = polyline_points(&pts, closed_req, POINT_EPS_MM);
            if pts.len() < 2 {
                return Err("Mindestens zwei verschiedene Punkte nötig".into());
            }
            let layer = resolve_layer(doc, &p);
            let idn = doc.new_id();
            let verts = pts.iter().map(|q| Vertex::new(*q)).collect();
            let mut e = Entity::new(idn, layer, EntityKind::Spline { verts, closed });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Eine Füllfläche aus einer Punktliste. Alle vier Namen laufen durch
        // denselben Abschluss (`add_fill`): `draw.fill` (Kontur anklicken),
        // `draw.fill.boundary` (Umriss abfahren), `draw.fill.points` (freie
        // Punktfolge) und der alte Name `draw.hatch` (Hotbar-Vorgaben,
        // MCP-Aufrufe von vor der Vereinheitlichung).
        //
        // Die Umrandung folgt dem Erstellmodus: beim Punkte setzen an (die
        // Fläche hat keine vorhandene Kante), beim Kontur anklicken aus (die
        // Umgrenzung liegt schon auf einer Linie oder Wand).
        "draw.fill.points" | "draw.fill.boundary" => {
            let boundary = p
                .pts("boundary")
                .or_else(|| p.pts("pts"))
                .ok_or_else(|| "Parameter 'boundary' fehlt".to_string())?;
            add_fill(doc, boundary, &p, true)
        }
        "draw.fill" | "draw.hatch" => {
            let boundary = p
                .pts("boundary")
                .or_else(|| p.pts("pts"))
                .ok_or_else(|| "Parameter 'boundary' fehlt".to_string())?;
            add_fill(doc, boundary, &p, false)
        }
        // Volumen: dieselben drei Eingaben wie bei der Füllfläche — Kontur
        // anklicken (`draw.volume`), Punktfolge (`draw.volume.points`) und
        // Farbeimer (`draw.volume.paint`) —, nur entsteht ein Körper mit Höhe.
        // `params.height` in mm; ohne Angabe die Standard-Wandhöhe.
        "draw.volume" => {
            let boundary = p
                .pts("boundary")
                .or_else(|| p.pts("pts"))
                .ok_or_else(|| "Parameter 'boundary' fehlt".to_string())?;
            let holes: Vec<Vec<Vec2>> = p
                .rings("holes")
                .unwrap_or_default()
                .into_iter()
                .filter(|h| h.len() >= 3)
                .collect();
            // Umrandung an: die Kontur, die hier übernommen wird (z. B. der
            // Wandkörper), verschwindet im Volumen — dessen eigene Kante wird
            // damit zur sichtbaren Kante.
            add_filled(doc, boundary, holes, &p, true, &[], true)
        }
        "draw.volume.points" => {
            let boundary = p
                .pts("boundary")
                .or_else(|| p.pts("pts"))
                .ok_or_else(|| "Parameter 'boundary' fehlt".to_string())?;
            let holes: Vec<Vec<Vec2>> = p
                .rings("holes")
                .unwrap_or_default()
                .into_iter()
                .filter(|h| h.len() >= 3)
                .collect();
            add_filled(doc, boundary, holes, &p, true, &[], true)
        }
        "draw.volume.paint" => {
            let pt = require_pt(&p, "p")?;
            let g = crate::boundary::detect_boundary(
                doc,
                pt,
                &crate::boundary::BoundaryOpts::default(),
            )
            .map_err(|e| e.message().to_string())?;
            add_filled(doc, g.outer, g.holes, &p, false, &g.border_entities, true)
        }
        // Farbeimer: ein Klick in die leere Fläche, die Umgrenzung ermittelt der
        // Kern (`boundary::detect_boundary`) aus derselben Displayliste, die
        // Bildschirm, PDF und PNG zeichnen. Der Klickpunkt ist ein Punkt in der
        // Fläche — kein Fangpunkt auf einer Kante, sonst meldet die Erkennung
        // „auf einer Linie geklickt“.
        "draw.fill.paint" => {
            let pt = require_pt(&p, "p")?;
            let g = crate::boundary::detect_boundary(
                doc,
                pt,
                &crate::boundary::BoundaryOpts::default(),
            )
            .map_err(|e| e.message().to_string())?;
            // Die ermittelte Umgrenzung nennt ihre Objekte: unter denen liegt
            // die Fläche in der Zeichenreihenfolge (`border_entities`).
            add_fill_rings(doc, g.outer, g.holes, &p, false, &g.border_entities)
        }
        "draw.text" => {
            let pt = require_pt(&p, "p")?;
            let text = p.text_or("text", "");
            if text.is_empty() {
                return Err("Parameter 'text' fehlt".into());
            }
            let layer = resolve_layer(doc, &p);
            let idn = doc.new_id();
            let h = p.len_or("height", doc.settings.text_height);
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Text {
                    p: pt,
                    text,
                    height: h,
                    angle: p.angle_or("angle", 0.0),
                    align: p.text_align("align").unwrap_or(TextAlignH::Left),
                    valign: p.text_valign("valign").unwrap_or(TextAlignV::Middle),
                    bold: p.boolean_or("bold", false),
                    italic: p.boolean_or("italic", false),
                    underline: p.boolean_or("underline", false),
                    superscript: p.boolean_or("superscript", false),
                    runs: Vec::new(),
                    font: p
                        .text("font")
                        .and_then(|s| TextFont::from_key(&s))
                        .unwrap_or_default(),
                },
            );
            // Gemischte Auszeichnung gleich beim Anlegen (Editor, MCP).
            if let Some(v) = p.raw("runs") {
                let liste = parse_runs(v).map_err(|e| format!("runs: {e}"))?;
                if let EntityKind::Text { text, runs, .. } = &mut e.kind {
                    *runs = crate::text::normalize_runs(
                        text,
                        &liste,
                        crate::text::RunStyle::default(),
                    );
                }
            }
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }

        // ---------------- Architektur ----------------
        "draw.wall" => {
            let a = require_pt(&p, "a")?;
            let b = require_pt(&p, "b")?;
            let thickness = p.len_or("thickness", doc.settings.wall_thickness);
            let layer = resolve_layer(doc, &p);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Wall {
                    a,
                    b,
                    thickness,
                    height: p.len_or("height", 2750.0),
                    material: p.text_or("material", "Mauerwerk"),
                    hatch: match p.text("hatch").unwrap_or_default().to_lowercase().as_str() {
                        "lines" => Some(crate::style::FillPattern::Lines),
                        "cross" => Some(crate::style::FillPattern::Cross),
                        "dots" => Some(crate::style::FillPattern::Dots),
                        "insulation" => Some(crate::style::FillPattern::Insulation),
                        _ => None,
                    },
                    align: match p.text("align").unwrap_or_default().to_lowercase().as_str() {
                        "left" | "links" => crate::style::WallAlign::Left,
                        "right" | "rechts" => crate::style::WallAlign::Right,
                        _ => crate::style::WallAlign::Center,
                    },
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn], "length_mm": a.dist(b)}))
        }
        "draw.door" => {
            let center = p
                .pt("center")
                .or_else(|| p.pt("at"))
                .or_else(|| p.pt("p"))
                .ok_or_else(|| "Parameter 'center' fehlt".to_string())?;
            let mut angle = p.angle("angle");
            let mut host = p.ids("host").and_then(|v| v.first().copied());
            if host.is_none() {
                // nächstgelegene Wand suchen
                let mut best: Option<(Id, f64, f64)> = None;
                for e in &doc.entities {
                    if let EntityKind::Wall { a, b, thickness, .. } = &e.kind {
                        let d = dist_point_segment(center, *a, *b);
                        if d <= thickness * 0.5 + 400.0 {
                            let cand = (e.id, d, (*b - *a).angle());
                            if best.map(|(_, bd, _)| d < bd).unwrap_or(true) {
                                best = Some(cand);
                            }
                        }
                    }
                }
                if let Some((wid, _, wall_angle)) = best {
                    host = Some(wid);
                    if angle.is_none() {
                        angle = Some(wall_angle);
                    }
                }
            }
            if angle.is_none() {
                if let Some(h) = host {
                    if let Some(EntityKind::Wall { a, b, .. }) = doc.entity(h).map(|e| &e.kind) {
                        angle = Some((*b - *a).angle());
                    }
                }
            }
            let layer = resolve_layer(doc, &p);
            let swing_in = p.boolean_or("swing_in", true);
            let at = wall_at(doc, host, center);
            // Tür immer auf die vordere Wandkante (Blattseite) setzen
            let center = door_face_center(doc, host, center, angle.unwrap_or(0.0), swing_in);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Door {
                    align: crate::style::WallAlign::Center,
                    center,
                    angle: angle.unwrap_or(0.0),
                    width: p.len_or("width", doc.settings.door_width),
                    kind: p.door_kind("kind").unwrap_or(DoorKind::Hinge),
                    hinge: p.num_or("hinge", 1.0),
                    swing_in,
                    host,
                    at,
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn], "host": host}))
        }
        "draw.window" => {
            let center = p
                .pt("center")
                .or_else(|| p.pt("at"))
                .or_else(|| p.pt("p"))
                .ok_or_else(|| "Parameter 'center' fehlt".to_string())?;
            let mut angle = p.angle("angle");
            let mut host = p.ids("host").and_then(|v| v.first().copied());
            if host.is_none() {
                let mut best: Option<(Id, f64, f64)> = None;
                for e in &doc.entities {
                    if let EntityKind::Wall { a, b, thickness, .. } = &e.kind {
                        let d = dist_point_segment(center, *a, *b);
                        if d <= thickness * 0.5 + 400.0 {
                            if best.map(|(_, bd, _)| d < bd).unwrap_or(true) {
                                best = Some((e.id, d, (*b - *a).angle()));
                            }
                        }
                    }
                }
                if let Some((wid, _, wa)) = best {
                    host = Some(wid);
                    if angle.is_none() {
                        angle = Some(wa);
                    }
                }
            }
            if angle.is_none() {
                if let Some(h) = host {
                    if let Some(EntityKind::Wall { a, b, .. }) = doc.entity(h).map(|e| &e.kind) {
                        angle = Some((*b - *a).angle());
                    }
                }
            }
            let layer = resolve_layer(doc, &p);
            let align = match p.text("align").unwrap_or_default().to_lowercase().as_str() {
                "left" | "links" => crate::style::WallAlign::Left,
                "right" | "rechts" => crate::style::WallAlign::Right,
                _ => crate::style::WallAlign::Center,
            };
            let at = wall_at(doc, host, center);
            let center = offset_opening(doc, host, center, angle.unwrap_or(0.0), align);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Window {
                    align,
                    center,
                    angle: angle.unwrap_or(0.0),
                    width: p.len_or("width", doc.settings.window_width),
                    sill: p.len_or("sill", 900.0),
                    height: p.len_or("height", 1250.0),
                    host,
                    at,
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn], "host": host}))
        }
        "draw.stair" => {
            let a = require_pt(&p, "a")?;
            let b = require_pt(&p, "b")?;
            let layer = resolve_layer(doc, &p);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Stair {
                    a,
                    b,
                    width: p.len_or("width", doc.settings.stair_width),
                    steps: p.num_or("steps", 16.0).max(1.0) as u32,
                    height: p.len_or("height", crate::stair::HOEHE_VORGABE_MM),
                    // Ohne ausdrückliche Stufenzahl folgt die Treppe der
                    // Schrittmaßregel; ein `steps`-Wert gilt als Vorgabe.
                    auto_steps: p
                        .boolean("auto")
                        .unwrap_or_else(|| p.raw("steps").is_none()),
                    cut: p.boolean_or("cut", true),
                    unten: None,
                    oben: None,
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Beschriftung einer Treppe: Stufenzahl und Schrittmaß als eigenes
        // Textfeld. Bewusst eine Action — der Knopf im Eigenschaften-Panel
        // erzeugt damit dasselbe Objekt wie ein MCP-Aufruf.
        "draw.stair.label" => {
            let id = p
                .id("id")
                .or_else(|| p.ids("ids").and_then(|v| v.first().copied()))
                .ok_or("Parameter 'id' (Treppe) fehlt")?;
            let (a, b, width, steps, height, auto_steps) = match doc.entity(id).map(|e| &e.kind) {
                Some(EntityKind::Stair {
                    a,
                    b,
                    width,
                    steps,
                    height,
                    auto_steps,
                    ..
                }) => (*a, *b, *width, *steps, *height, *auto_steps),
                Some(_) => return Err(format!("Objekt {id} ist keine Treppe")),
                None => return Err(format!("Objekt {id} nicht gefunden")),
            };
            let mass = crate::stair::treppenmass(a.dist(b), height, steps, auto_steps);
            let dir = (b - a).normalized();
            // `perp` dreht 90° nach links: die Beschriftung steht links vom Lauf,
            // wie im Vorbild (Stufenzahl über Schrittmaß).
            let nrm = dir.perp();
            let mid = (a + b) * 0.5;
            let h = p.len_or("height", doc.settings.text_height);
            let text = mass.beschriftung();
            // Die Textbreite kommt aus derselben Schätzung wie der Auswahlrahmen
            // (`render::text_box`), damit der Kasten genau neben der Treppe endet.
            let (tw, _) = crate::render::text_box(&text, h);
            let abstand = p.len_or("gap", h * 0.8);
            // Anker ist die linke Kante des Textkastens (linksbündig) — so liegt
            // die rechte Kante genau `abstand` neben der Treppenkante.
            let anker = mid + nrm * (width * 0.5 + abstand + tw);
            let layer = resolve_layer(doc, &p);
            let idn = doc.new_id();
            let mut t = Entity::new(
                idn,
                layer,
                EntityKind::Text {
                    p: anker,
                    text: text.clone(),
                    height: h,
                    angle: 0.0,
                    align: TextAlignH::Left,
                    valign: TextAlignV::Middle,
                    bold: false,
                    italic: false,
                    underline: false,
                    superscript: false,
                    runs: Vec::new(),
                    font: TextFont::Sans,
                },
            );
            apply_common(doc, &mut t, &p, layer);
            doc.add_entity(t);
            Ok(json!({"ok": true, "created": [idn], "text": mass.beschriftung()}))
        }

        // ---------------- Bemaßung ----------------
        "dim.linear" => {
            let a = require_pt(&p, "a")?;
            let b = require_pt(&p, "b")?;
            let line_pos = p.pt("line_pos").or_else(|| p.pt("pos")).unwrap_or_else(|| {
                let m = (a + b) * 0.5;
                v(m.x, m.y + doc.settings.annotation_scale * 8.0)
            });
            let axis = p.dim_axis("axis").unwrap_or_else(|| {
                if (b.x - a.x).abs() >= (b.y - a.y).abs() {
                    DimAxis::X
                } else {
                    DimAxis::Y
                }
            });
            let layer = p
                .id("layer")
                .or_else(|| doc.layer_id_by_name("Bemaßung"))
                .unwrap_or(doc.current_layer);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Dim {
                    a,
                    b,
                    line_pos,
                    axis,
                    override_text: p.text("text"),
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        "dim.chain" => {
            let pts = p
                .pts("pts")
                .ok_or_else(|| "Parameter 'pts' fehlt".to_string())?;
            if pts.len() < 2 {
                return Err("Maßkette braucht mindestens zwei Punkte".into());
            }
            let line_pos = p
                .pt("line_pos")
                .or_else(|| p.pt("pos"))
                .unwrap_or_else(|| {
                    let b = BBox::from_points(&pts);
                    v(b.min.x, b.min.y - doc.settings.annotation_scale * 8.0)
                });
            let layer = doc
                .layer_id_by_name("Bemaßung")
                .unwrap_or(doc.current_layer);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::DimChain {
                    pts,
                    line_pos,
                    axis: p.dim_axis("axis").unwrap_or(DimAxis::X),
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        "dim.elevation" => {
            let pt = require_pt(&p, "p")?;
            let layer = doc
                .layer_id_by_name("Bemaßung")
                .unwrap_or(doc.current_layer);
            let idn = doc.new_id();
            let z = p.len_or("z", 0.0);
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Elevation {
                    p: pt,
                    z,
                    angle: p.angle_or("angle", 0.0),
                    override_text: p.text("text"),
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn], "z_mm": z}))
        }

        // ---------------- Ändern ----------------
        // Zeichenreihenfolge: die Liste der Objekte IST die Reihenfolge, der
        // Index ist der Platz. Ein Element wird entnommen und an der neuen
        // Stelle wieder eingesetzt — dadurch kann es keine doppelte Position
        // geben, und die Plaetze bleiben lueckenlos.
        "modify.order" => {
            let ids = ids_from(&p, "ids");
            if ids.is_empty() {
                return Err("Parameter 'ids' fehlt".into());
            }
            // Referenzobjekt für "vor/hinter Element"
            let referenz = p.id("ref");
            let wo = p.text("where").unwrap_or_default().to_lowercase();

            let mut gewaehlt: Vec<Entity> = Vec::new();
            doc.ents_mut().retain(|e| {
                if ids.contains(&e.id) {
                    gewaehlt.push(e.clone());
                    false
                } else {
                    true
                }
            });
            let n = doc.ents().len();

            // Ziel erst nach dem Entnehmen bestimmen, damit sich die Plätze
            // nicht durch das eigene Element verschieben
            let ziel: usize = if let Some(r) = referenz {
                match doc.ents().iter().position(|e| e.id == r) {
                    Some(i) if wo == "above" => i + 1,
                    Some(i) => i,
                    None => n,
                }
            } else if let Some(t) = p.num("to") {
                ((t.round() as i64 - 1).max(0) as usize).min(n)
            } else if wo == "front" || wo == "vorn" || wo == "oben" {
                n
            } else {
                0
            };
            let at = ziel.min(n);
            for (k, e) in gewaehlt.into_iter().enumerate() {
                doc.ents_mut().insert(at + k, e);
            }
            doc.modified = true;
            Ok(json!({
                "ok": true,
                "position": at + 1,
                "count": doc.ents().len(),
            }))
        }
        "modify.move" | "modify.copy" => {
            let ids = ids_from(&p, "ids");
            if ids.is_empty() {
                return Err("Parameter 'ids' fehlt".into());
            }
            let base = p.pt("from").or_else(|| p.pt("base")).unwrap_or(Vec2::ZERO);
            let to = if let Some(t) = p.pt("to") {
                t
            } else {
                let dx = p.len("dx").unwrap_or(0.0);
                let dy = p.len("dy").unwrap_or(0.0);
                base + v(dx, dy)
            };
            let m = Affine::translate(to - base);
            let copy = id == "modify.copy";
            let mut new_ids = Vec::new();
            // Öffnungen wandern mit ihrer Wand
            let mut ids = ids;
            if !copy {
                for extra in carried_openings(doc, &ids) {
                    if !ids.contains(&extra) {
                        ids.push(extra);
                    }
                }
            }
            for eid in &ids {
                let Some(e) = doc.entity(*eid).cloned() else { continue };
                if copy {
                    let mut ne = transform::transform_entity(&e, &m);
                    ne.id = doc.new_id();
                    new_ids.push(ne.id);
                    doc.ents_mut().push(ne);
                } else if let Some(em) = doc.entity_mut(*eid) {
                    *em = transform::transform_entity(em, &m);
                }
            }
            doc.modified = true;
            Ok(json!({"ok": true, "created": new_ids, "delta_mm": (to - base).to_array()}))
        }
        "modify.rotate" => {
            let ids = ids_from(&p, "ids");
            let center = require_pt(&p, "center")?;
            let angle = p
                .angle("angle")
                .ok_or_else(|| "Parameter 'angle' (Grad) fehlt".to_string())?;
            let copy = p.boolean_or("copy", false);
            let m = Affine::rotate_about(center, angle);
            let mut new_ids = Vec::new();
            for eid in &ids {
                let Some(e) = doc.entity(*eid).cloned() else { continue };
                if copy {
                    let mut ne = transform::transform_entity(&e, &m);
                    ne.id = doc.new_id();
                    new_ids.push(ne.id);
                    doc.ents_mut().push(ne);
                } else if let Some(em) = doc.entity_mut(*eid) {
                    *em = transform::transform_entity(em, &m);
                }
            }
            doc.modified = true;
            Ok(json!({"ok": true, "created": new_ids}))
        }
        "modify.mirror" => {
            let ids = ids_from(&p, "ids");
            let p1 = require_pt(&p, "p1")?;
            let p2 = require_pt(&p, "p2")?;
            let keep = p.boolean_or("keep_original", true);
            let m = Affine::mirror(p1, p2);
            let mut new_ids = Vec::new();
            for eid in &ids {
                let Some(e) = doc.entity(*eid).cloned() else { continue };
                if keep {
                    let mut ne = transform::transform_entity(&e, &m);
                    ne.id = doc.new_id();
                    new_ids.push(ne.id);
                    doc.ents_mut().push(ne);
                } else if let Some(em) = doc.entity_mut(*eid) {
                    *em = transform::transform_entity(em, &m);
                }
            }
            doc.modified = true;
            Ok(json!({"ok": true, "created": new_ids}))
        }
        "modify.scale" => {
            let ids = ids_from(&p, "ids");
            let center = require_pt(&p, "center")?;
            let factor = p
                .num("factor")
                .or_else(|| p.num("scale"))
                .ok_or_else(|| "Parameter 'factor' fehlt".to_string())?;
            if factor.abs() < 1e-9 {
                return Err("Faktor darf nicht 0 sein".into());
            }
            let m = Affine::scale_about(center, factor, factor);
            for eid in &ids {
                if let Some(em) = doc.entity_mut(*eid) {
                    *em = transform::transform_entity(em, &m);
                }
            }
            doc.modified = true;
            Ok(json!({"ok": true}))
        }
        "modify.offset" => {
            let ids = ids_from(&p, "ids");
            let dist = p
                .len("distance")
                .or_else(|| p.len("dist"))
                .ok_or_else(|| "Parameter 'distance' fehlt".to_string())?;
            let side_pt = p.pt("side").or_else(|| p.pt("at"));
            let mut new_ids = Vec::new();
            for eid in &ids {
                let Some(e) = doc.entity(*eid).cloned() else { continue };
                let Some(kind) = offset_kind(&e, dist, side_pt) else { continue };
                let mut ne = Entity::new(doc.new_id(), e.layer, kind);
                ne.style = e.style.clone();
                new_ids.push(ne.id);
                doc.ents_mut().push(ne);
            }
            doc.modified = true;
            Ok(json!({"ok": true, "created": new_ids}))
        }
        "modify.trim" => {
            let ids = ids_from(&p, "ids");
            let at = require_pt(&p, "at")?;
            let cutters = p.ids("cutters").unwrap_or_default();
            let mut created = Vec::new();
            for eid in &ids {
                match trim_entity(doc, *eid, at, &cutters) {
                    Ok(mut v) => created.append(&mut v),
                    Err(e) => return Err(e),
                }
            }
            doc.modified = true;
            Ok(json!({"ok": true, "created": created}))
        }
        "modify.extend" => {
            let ids = ids_from(&p, "ids");
            let at = require_pt(&p, "at")?;
            let cutters = p.ids("cutters").unwrap_or_default();
            for eid in &ids {
                extend_entity(doc, *eid, at, &cutters)?;
            }
            doc.modified = true;
            Ok(json!({"ok": true}))
        }
        "modify.erase" => {
            let ids = ids_from(&p, "ids");
            let mut n = 0;
            for eid in &ids {
                if doc.remove_entity(*eid).is_some() {
                    n += 1;
                }
            }
            Ok(json!({"ok": true, "deleted": n}))
        }
        "modify.explode" => {
            let ids = ids_from(&p, "ids");
            let mut created = Vec::new();
            for eid in &ids {
                created.extend(explode_entity(doc, *eid)?);
            }
            doc.modified = true;
            Ok(json!({"ok": true, "created": created}))
        }
        // ---------------- Text auszeichnen ----------------
        // Setzt die Auszeichnung für einen Zeichenbereich (Indizes in Zeichen,
        // 0-basiert, `to` ausschließend). Ohne brauchbaren Bereich gilt der
        // ganze Text — so verhält sich auch das Panel ohne Markierung.
        "text.format" => {
            // Eine Kennung oder eine Liste — beides ist erlaubt.
            let id = match p.raw("id").or_else(|| p.raw("ids")) {
                Some(Value::Number(n)) => n.as_u64(),
                Some(Value::Array(a)) => a.first().and_then(|x| x.as_u64()),
                _ => None,
            }
            .ok_or_else(|| "Parameter 'id' fehlt".to_string())?;
            let Some(e) = doc.entity_mut(id) else {
                return Err(format!("Objekt {id} nicht gefunden"));
            };
            let EntityKind::Text {
                text,
                runs,
                bold,
                italic,
                underline,
                superscript,
                ..
            } = &mut e.kind
            else {
                return Err("Nur Text lässt sich auszeichnen".into());
            };
            let laenge = text.chars().count();
            let mut von = p.num("from").unwrap_or(0.0).max(0.0) as usize;
            let mut bis = p.num("to").unwrap_or(laenge as f64).max(0.0) as usize;
            von = von.min(laenge);
            bis = bis.min(laenge);
            if bis <= von {
                von = 0;
                bis = laenge;
            }
            // Ausgangszustand: vorhandene Läufe, sonst die Felder des Objekts.
            if runs.is_empty() {
                *runs = vec![TextRun {
                    text: text.clone(),
                    bold: *bold,
                    italic: *italic,
                    underline: *underline,
                    superscript: *superscript,
                }];
            }
            let mut st = crate::text::char_styles(text, runs, crate::text::RunStyle::default())
                .get(von)
                .copied()
                .unwrap_or_default();
            if let Some(v) = p.boolean("bold") {
                st.bold = v;
            }
            if let Some(v) = p.boolean("italic") {
                st.italic = v;
            }
            if let Some(v) = p.boolean("underline") {
                st.underline = v;
            }
            if let Some(v) = p.boolean("superscript") {
                st.superscript = v;
            }
            *runs = crate::text::apply_style(text, runs, von, bis, st);
            let anzahl = runs.len();
            doc.modified = true;
            Ok(json!({"ok": true, "from": von, "to": bis, "runs": anzahl}))
        }
        "modify.set" => {
            let ids = ids_from(&p, "ids");
            let mut changed = 0;
            // Layerziel vor der Schleife bestimmen (Borromaschine: kein doppelter Borrow)
            let new_layer: Option<Id> = p.v.get("layer").and_then(|l| {
                l.as_u64()
                    .filter(|i| doc.layers.iter().any(|x| x.id == *i))
                    .or_else(|| l.as_str().and_then(|s| doc.layer_id_by_name(s)))
            });
            // Ausrichtung vorab lesen (gilt für Wand und Öffnungen)
            let align_new: Option<crate::style::WallAlign> =
                p.text("align").map(|al| match al.to_lowercase().as_str() {
                    "left" | "links" => crate::style::WallAlign::Left,
                    "right" | "rechts" => crate::style::WallAlign::Right,
                    _ => crate::style::WallAlign::Center,
                });
            let swing_new = p.boolean("swing_in");
            for eid in &ids {
                // Öffnungen: neue Lage berechnen, solange das Dokument nur
                // lesend geliehen ist
                let realign: Option<Vec2> = match doc.entity(*eid).map(|e| &e.kind) {
                    // Tür sitzt immer auf der vorderen Wandkante (Blattseite)
                    Some(EntityKind::Door { host, center, angle, swing_in, .. }) => {
                        if align_new.is_some() || swing_new.is_some() {
                            Some(door_face_center(
                                doc,
                                *host,
                                *center,
                                *angle,
                                swing_new.unwrap_or(*swing_in),
                            ))
                        } else {
                            None
                        }
                    }
                    Some(EntityKind::Window { host, center, angle, .. }) => {
                        align_new.map(|al| realign_opening(doc, *host, *center, *angle, al))
                    }
                    _ => None,
                };
                if let Some(e) = doc.entity_mut(*eid) {
                    if let Some(lid) = new_layer {
                        e.layer = lid;
                    }
                    if let Some(c) = p.color("color") {
                        e.style.color = Some(c);
                    } else if p.text("color").map(|s| s.eq_ignore_ascii_case("bylayer")).unwrap_or(false) {
                        e.style.color = None;
                    }
                    if let Some(lt) = p.linetype("linetype") {
                        e.style.line_type = Some(lt);
                    }
                    if let Some(lw) = p.num("lineweight") {
                        e.style.line_weight = Some(lw);
                    }
                    if let Some(c) = p.color("fill") {
                        e.style.fill = Some(c);
                    } else if p
                        .text("fill")
                        .map(|t| t.eq_ignore_ascii_case("none") || t.eq_ignore_ascii_case("bylayer"))
                        .unwrap_or(false)
                    {
                        e.style.fill = None;
                    }
                    // Objektstil als Rückfall für die Treppenteile, bevor
                    // `kind` verliehen wird.
                    let basis_stil = e.style.clone();
                    match &mut e.kind {
                        EntityKind::Wall { align, .. } => {
                            if let Some(wa) = align_new {
                                *align = wa;
                            }
                        }
                        EntityKind::Door { center, .. } => {
                            if let Some(p2) = realign {
                                *center = p2;
                            }
                        }
                        EntityKind::Window { align, center, .. } => {
                            if let Some(wa) = align_new {
                                *align = wa;
                            }
                            if let Some(p2) = realign {
                                *center = p2;
                            }
                        }
                        _ => {}
                    }
                    if let Some(h) = p.text("hatch") {
                        let hp = match h.to_lowercase().as_str() {
                            "none" | "solid" | "" => None,
                            "lines" => Some(crate::style::FillPattern::Lines),
                            "cross" => Some(crate::style::FillPattern::Cross),
                            "dots" => Some(crate::style::FillPattern::Dots),
                            "insulation" | "daemmung" | "dämmung" => {
                                Some(crate::style::FillPattern::Insulation)
                            }
                            _ => None,
                        };
                        if let EntityKind::Wall { hatch, .. } = &mut e.kind {
                            *hatch = hp;
                        }
                    }
                    // Typparameter
                    let thickness = p.len("thickness");
                    let width = p.len("width");
                    let steps = p.num("steps").map(|s| s.max(1.0) as u32);
                    let height_new = p.len("height");
                    let auto_new = p.boolean("auto_steps");
                    let height = p.len("height");
                    let text = p.text("text");
                    match &mut e.kind {
                        EntityKind::Wall {
                            thickness: t,
                            height: h,
                            material,
                            ..
                        } => {
                            if let Some(v) = thickness {
                                *t = v;
                            }
                            if let Some(v) = height {
                                *h = v;
                            }
                            if let Some(m) = p.text("material") {
                                *material = m;
                            }
                        }
                        EntityKind::Door {
                            width: w, kind, hinge, swing_in, ..
                        } => {
                            if let Some(v) = width {
                                *w = v;
                            }
                            if let Some(k) = p.door_kind("kind") {
                                *kind = k;
                            }
                            if let Some(h) = p.num("hinge") {
                                *hinge = h;
                            }
                            if let Some(b) = p.boolean("swing_in") {
                                *swing_in = b;
                            }
                        }
                        EntityKind::Window { width: w, sill, height: h, .. } => {
                            if let Some(v) = width {
                                *w = v;
                            }
                            if let Some(v) = p.len("sill") {
                                *sill = v;
                            }
                            if let Some(v) = height {
                                *h = v;
                            }
                        }
                        EntityKind::Volume {
                            pattern,
                            pattern_angle,
                            pattern_scale,
                            border,
                            height: h,
                            ..
                        } => {
                            if let Some(v) = p.fill_pattern("pattern") {
                                *pattern = v;
                            }
                            if let Some(v) = p.angle("pattern_angle") {
                                *pattern_angle = v;
                            }
                            if let Some(v) = p.len("pattern_scale") {
                                *pattern_scale = v.max(0.5);
                            }
                            if let Some(v) = p.boolean("border") {
                                *border = v;
                            }
                            if let Some(v) = height {
                                *h = v.max(1.0);
                            }
                        }
                        EntityKind::Fill {
                            pattern,
                            pattern_angle,
                            pattern_scale,
                            border,
                            ..
                        } => {
                            if let Some(v) = p.fill_pattern("pattern") {
                                *pattern = v;
                            }
                            if let Some(v) = p.angle("pattern_angle") {
                                *pattern_angle = v;
                            }
                            if let Some(v) = p.len("pattern_scale") {
                                *pattern_scale = v.max(0.5);
                            }
                            if let Some(v) = p.boolean("border") {
                                *border = v;
                            }
                        }
                        EntityKind::Stair {
                            a: sa,
                            b: sb,
                            width: w,
                            steps: s,
                            height,
                            auto_steps,
                            cut,
                            unten: teil_unten,
                            oben: teil_oben,
                        } => {
                            if let Some(v) = width {
                                *w = v.max(1.0);
                            }
                            if let Some(v) = height_new {
                                *height = v.max(1.0);
                            }
                            if let Some(v) = steps {
                                // Eine ausdrücklich gesetzte Stufenzahl ist
                                // eine Überschreibung der Schrittmaßregel.
                                *s = v;
                                *auto_steps = false;
                            }
                            if let Some(v) = auto_new {
                                *auto_steps = v;
                            }
                            if let Some(v) = p.boolean("cut") {
                                *cut = v;
                            }
                            // Länge des Laufs: der Endpunkt wandert auf der
                            // Lauflinie, der Anfang bleibt stehen.
                            if let Some(v) = p.len("len") {
                                let d = *sb - *sa;
                                if d.len() > 1e-6 {
                                    *sb = *sa + d.normalized() * v.max(1.0);
                                }
                            }
                            // Linien der beiden Treppenteile (ober-/unterhalb
                            // des Schnitts), getrennt einstellbar.
                            stair_teil_setzen(teil_unten, &basis_stil, &p, "unten");
                            stair_teil_setzen(teil_oben, &basis_stil, &p, "oben");
                        }
                        EntityKind::Text {
                            text: t,
                            height: h,
                            angle,
                            align,
                            valign,
                            bold,
                            italic,
                            underline,
                            superscript,
                            runs,
                            font,
                            ..
                        } => {
                            if let Some(v) = text {
                                *t = v;
                                // Geänderter Text macht gemischte Läufe
                                // ungültig — sonst stünde im PDF etwas
                                // anderes als im Objekt.
                                runs.clear();
                            }
                            if let Some(v) = height {
                                *h = v;
                            }
                            if let Some(v) = p.angle("angle") {
                                *angle = v;
                            }
                            // Ausrichtung des Textfelds: waagerecht `align`
                            // (Linksbündig/Zentriert/Rechtsbündig), senkrecht
                            // `valign` (Oben/Mitte/Unten). `align` gilt sonst
                            // für Wände und Öffnungen — getrennt wird am
                            // Objekttyp, nicht am Schlüssel.
                            if let Some(v) = p.text_align("align") {
                                *align = v;
                            }
                            if let Some(v) = p.text_valign("valign") {
                                *valign = v;
                            }
                            if let Some(b) = p.boolean("bold") {
                                *bold = b;
                            }
                            if let Some(b) = p.boolean("italic") {
                                *italic = b;
                            }
                            if let Some(b) = p.boolean("underline") {
                                *underline = b;
                            }
                            if let Some(b) = p.boolean("superscript") {
                                *superscript = b;
                            }
                            if let Some(f) = p.text("font").and_then(|s| TextFont::from_key(&s)) {
                                *font = f;
                            }
                            // Gemischte Auszeichnung: eine Liste von Läufen.
                            // Sie muss den Text genau abdecken; fehlende
                            // Zeichen erben die vorige Auszeichnung.
                            if let Some(v) = p.raw("runs") {
                                match parse_runs(v) {
                                    Ok(liste) => {
                                        let vorgabe = crate::text::RunStyle {
                                            bold: *bold,
                                            italic: *italic,
                                            underline: *underline,
                                            superscript: *superscript,
                                        };
                                        *runs = crate::text::normalize_runs(t, &liste, vorgabe);
                                    }
                                    Err(e) => return Err(format!("runs: {e}")),
                                }
                            }
                        }
                        EntityKind::Circle { r, .. } => {
                            if let Some(v) = p.len("r").or(p.len("radius")) {
                                *r = v.max(0.001);
                            }
                        }
                        // Bogen: Radius und beide Endwinkel (Eingabe in Grad,
                        // wie überall in der Befehlsschicht) — T0022.
                        EntityKind::Arc { r, a0, a1, .. } => {
                            if let Some(v) = p.len("r").or(p.len("radius")) {
                                *r = v.max(0.001);
                            }
                            if let Some(v) = p.angle("a0") {
                                *a0 = v;
                            }
                            if let Some(v) = p.angle("a1") {
                                *a1 = v;
                            }
                        }
                        EntityKind::Dim { override_text, axis, .. } => {
                            if let Some(v) = text {
                                *override_text = Some(v);
                            }
                            if let Some(ax) = p.dim_axis("axis") {
                                *axis = ax;
                            }
                        }
                        EntityKind::Elevation { z, override_text, .. } => {
                            if let Some(v) = p.len("z") {
                                *z = v;
                            }
                            if let Some(v) = text {
                                *override_text = Some(v);
                            }
                        }
                        _ => {}
                    }
                    changed += 1;
                }
            }
            doc.modified = true;
            Ok(json!({"ok": true, "changed": changed}))
        }

        // ---------------- Layer ----------------
        "layer.create" => {
            let name = p.text("name").ok_or_else(|| "Parameter 'name' fehlt".to_string())?;
            if doc.layer_by_name(&name).is_some() {
                return Err(format!("Layer '{name}' existiert bereits"));
            }
            let id = doc.new_id();
            let mut l = Layer::new(id, &name, p.color("color").unwrap_or(Color::BLACK));
            l.line_weight = p.num_or("lineweight", 0.25);
            l.line_type = p.linetype("linetype").unwrap_or(LineType::Solid);
            l.visible = p.boolean_or("visible", true);
            l.locked = p.boolean_or("locked", false);
            doc.layers.push(l);
            Ok(json!({"ok": true, "id": id}))
        }
        "layer.update" => {
            let target = p
                .v
                .get("layer")
                .or_else(|| p.v.get("id"))
                .cloned()
                .ok_or_else(|| "Parameter 'layer' fehlt".to_string())?;
            let lid = target
                .as_u64()
                .or_else(|| target.as_str().and_then(|s| doc.layer_id_by_name(s)));
            let lid = lid.ok_or_else(|| "Layer nicht gefunden".to_string())?;
            if let Some(l) = doc.layer_mut(lid) {
                if let Some(n) = p.text("name") {
                    l.name = n;
                }
                if let Some(c) = p.color("color") {
                    l.color = c;
                }
                if let Some(lw) = p.num("lineweight") {
                    l.line_weight = lw;
                }
                if let Some(lt) = p.linetype("linetype") {
                    l.line_type = lt;
                }
                if let Some(v) = p.boolean("visible") {
                    l.visible = v;
                }
                if let Some(v) = p.boolean("locked") {
                    l.locked = v;
                }
                if let Some(v) = p.boolean("printable") {
                    l.printable = v;
                }
            }
            Ok(json!({"ok": true}))
        }
        "layer.delete" => {
            let target = p.v.get("layer").cloned().unwrap_or(Value::Null);
            let lid = target
                .as_u64()
                .or_else(|| target.as_str().and_then(|s| doc.layer_id_by_name(s)))
                .ok_or_else(|| "Parameter 'layer' fehlt".to_string())?;
            if doc.layers.len() <= 1 {
                return Err("Der letzte Layer kann nicht gelöscht werden".into());
            }
            // Papierraum zählt mit: ein Layer, der nur im Schriftfeld benutzt
            // wird, ist nicht leer.
            let benutzt = doc.entities.iter().chain(
                doc.sheets
                    .iter()
                    .flat_map(|s| s.entities.iter()),
            );
            if benutzt.clone().any(|e| e.layer == lid) {
                return Err("Layer enthält noch Objekte".into());
            }
            doc.layers.retain(|l| l.id != lid);
            if doc.current_layer == lid {
                doc.current_layer = doc.layers[0].id;
            }
            Ok(json!({"ok": true}))
        }
        "layer.set_current" => {
            let target = p.v.get("layer").cloned().unwrap_or(Value::Null);
            let lid = target
                .as_u64()
                .or_else(|| target.as_str().and_then(|s| doc.layer_id_by_name(s)))
                .ok_or_else(|| "Parameter 'layer' fehlt".to_string())?;
            if doc.layer(lid).is_some() {
                doc.current_layer = lid;
                Ok(json!({"ok": true}))
            } else {
                Err("Layer nicht gefunden".into())
            }
        }
        "layer.list" => {
            let layers: Vec<Value> = doc
                .layers
                .iter()
                .map(|l| {
                    json!({
                        "id": l.id, "name": l.name, "color": l.color.to_hex(),
                        "visible": l.visible, "locked": l.locked,
                        "lineweight": l.line_weight, "linetype": l.line_type
                    })
                })
                .collect();
            Ok(json!({"ok": true, "layers": layers, "current": doc.current_layer}))
        }

        // ---------------- Blöcke ----------------
        "block.insert" => {
            let name = p
                .text("block")
                .or_else(|| p.text("name"))
                .ok_or_else(|| "Parameter 'block' fehlt".to_string())?;
            let def = doc
                .blocks
                .iter()
                .find(|b| b.id == name || b.name.eq_ignore_ascii_case(&name))
                .ok_or_else(|| format!("Block '{name}' nicht gefunden"))?;
            let block_id = def.id.clone();
            let pos = p.pt("pos").or_else(|| p.pt("at")).unwrap_or(Vec2::ZERO);
            let layer = resolve_layer(doc, &p);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::BlockRef {
                    block: block_id,
                    pos,
                    rot: p.angle_or("rot", 0.0),
                    scale: v(
                        p.num_or("scale", 1.0),
                        p.num_or("scale_y", p.num_or("scale", 1.0)),
                    ),
                    mirror: p.boolean_or("mirror", false),
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        "block.create" => {
            let name = p.text("name").ok_or_else(|| "Parameter 'name' fehlt".to_string())?;
            let ids = ids_from(&p, "ids");
            if ids.is_empty() {
                return Err("Parameter 'ids' fehlt".into());
            }
            let mut bbox = BBox::EMPTY;
            for id in &ids {
                if let Some(e) = doc.entity(*id) {
                    bbox.union(render::entity_bbox(doc, e));
                }
            }
            let base = p.pt("base").unwrap_or_else(|| {
                if bbox.is_valid() {
                    bbox.center()
                } else {
                    Vec2::ZERO
                }
            });
            let mut entities = Vec::new();
            for id in &ids {
                if let Some(e) = doc.entity(*id).cloned() {
                    let m = Affine::translate(-base);
                    let mut ne = transform::transform_entity(&e, &m);
                    ne.id = entities.len() as u64 + 1;
                    entities.push(ne);
                }
            }
            let bid = format!("prj:{}", crate::util::short_id());
            doc.blocks.push(BlockDef {
                id: bid.clone(),
                name: name.clone(),
                category: p.text_or("category", "Projekt"),
                source: BlockSource::Project,
                description: p.text_or("description", ""),
                entities,
                icon: None,
                base,
            });
            // Originale entfernen, Instanz einfügen
            for id in &ids {
                doc.remove_entity(*id);
            }
            let layer = resolve_layer(doc, &p);
            let inst = doc.new_id();
            let mut e = Entity::new(
                inst,
                layer,
                EntityKind::BlockRef {
                    block: bid,
                    pos: base,
                    rot: 0.0,
                    scale: v(1.0, 1.0),
                    mirror: false,
                },
            );
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [inst], "block": e_block_id(doc.ents(), inst)}))
        }
        "block.update" => {
            let name = p.text("block").ok_or_else(|| "Parameter 'block' fehlt".to_string())?;
            let ids = ids_from(&p, "ids");
            let (base, is_system) = {
                let block = doc
                    .blocks
                    .iter()
                    .find(|b| b.id == name || b.name.eq_ignore_ascii_case(&name))
                    .ok_or_else(|| format!("Block '{name}' nicht gefunden"))?;
                (block.base, block.source == BlockSource::System)
            };
            if is_system {
                return Err("Systemblöcke sind schreibgeschützt".into());
            }
            let mut ents = Vec::new();
            for (i, id) in ids.iter().enumerate() {
                if let Some(e) = doc.entity(*id).cloned() {
                    let m = Affine::translate(-base);
                    let mut ne = transform::transform_entity(&e, &m);
                    ne.id = i as u64 + 1;
                    ents.push(ne);
                }
            }
            let new_name = p.text("name");
            if let Some(block) = doc
                .blocks
                .iter_mut()
                .find(|b| b.id == name || b.name.eq_ignore_ascii_case(&name))
            {
                if !ents.is_empty() {
                    block.entities = ents;
                }
                if let Some(n) = new_name {
                    block.name = n;
                }
            }
            Ok(json!({"ok": true}))
        }
        "block.delete" => {
            let name = p.text("block").ok_or_else(|| "Parameter 'block' fehlt".to_string())?;
            let in_benutzung = doc
                .entities
                .iter()
                .chain(doc.sheets.iter().flat_map(|s| s.entities.iter()))
                .any(|e| matches!(&e.kind, EntityKind::BlockRef{block,..} if *block == name));
            if in_benutzung {
                return Err("Block wird noch verwendet".into());
            }
            doc.blocks.retain(|b| b.id != name && !b.name.eq_ignore_ascii_case(&name));
            Ok(json!({"ok": true}))
        }
        // ---------------- PDF einfügen ----------------
        "file.import.pdf" => import_pdf_action(doc, &p),
        "block.list" => {
            let blocks: Vec<Value> = doc
                .blocks
                .iter()
                .map(|b| {
                    json!({
                        "id": b.id, "name": b.name, "category": b.category,
                        "source": b.source, "entities": b.entities.len(),
                        "instances": doc.block_instances(&b.id).len()
                    })
                })
                .collect();
            Ok(json!({"ok": true, "blocks": blocks}))
        }

        // ---------------- Blatt ----------------
        "sheet.new" => {
            let paper = match p.text_or("paper", "A3").to_uppercase().as_str() {
                "A4" => PaperSize::A4,
                "A2" => PaperSize::A2,
                "A1" => PaperSize::A1,
                "A0" => PaperSize::A0,
                _ => PaperSize::A3,
            };
            let landscape = p.boolean_or("landscape", true);
            let name = p.text_or("name", &format!("Blatt {}", doc.sheets.len() + 1));
            let sheet = templates::new_sheet(doc, paper, landscape, &name);
            let sid = sheet.id;
            doc.sheets.push(sheet);
            Ok(json!({"ok": true, "sheet": sid}))
        }
        "sheet.update" => {
            let sid = p.id("sheet");
            let sid = sid.ok_or_else(|| "Parameter 'sheet' fehlt".to_string())?;
            let sheet = doc.sheet_mut(sid).ok_or_else(|| "Blatt nicht gefunden".to_string())?;
            if let Some(n) = p.text("name") {
                sheet.name = n;
            }
            if let Some(s) = p.num("scale") {
                sheet.scale = s;
            }
            if let Some(v) = p.boolean("frame") {
                sheet.frame = v;
            }
            if let Some(v) = p.boolean("show_title_block") {
                sheet.show_title_block = v;
            }
            if let Some(t) = p.text("paper") {
                sheet.paper = match t.to_uppercase().as_str() {
                    "A4" => PaperSize::A4,
                    "A2" => PaperSize::A2,
                    "A1" => PaperSize::A1,
                    "A0" => PaperSize::A0,
                    _ => PaperSize::A3,
                };
            }
            if let Some(v) = p.boolean("landscape") {
                sheet.landscape = v;
            }
            Ok(json!({"ok": true}))
        }
        "sheet.delete" => {
            let sid = p.id("sheet").unwrap_or(0);
            if doc.sheets.len() <= 1 {
                return Err("Das letzte Blatt kann nicht gelöscht werden".into());
            }
            doc.sheets.retain(|s| s.id != sid);
            Ok(json!({"ok": true}))
        }
        "sheet.add_viewport" => {
            let sid = p.id("sheet").unwrap_or(0);
            let (sw, sh) = {
                let s = doc.sheet(sid).ok_or_else(|| "Blatt nicht gefunden".to_string())?;
                s.size()
            };
            let mut vp = templates::default_viewport(doc, sw, sh);
            if let Some(x) = p.num("x") {
                vp.x = x;
            }
            if let Some(y) = p.num("y") {
                vp.y = y;
            }
            if let Some(w) = p.num("w") {
                vp.w = w;
            }
            if let Some(h) = p.num("h") {
                vp.h = h;
            }
            if let Some(c) = p.pt("center") {
                vp.center = c;
            }
            if let Some(s) = p.num("scale") {
                vp.scale = s;
            }
            vp.show_frame = p.boolean_or("show_frame", true);
            let vid = vp.id;
            if let Some(sh) = doc.sheet_mut(sid) {
                sh.viewports.push(vp);
            }
            Ok(json!({"ok": true, "viewport": vid}))
        }
        "sheet.update_viewport" => {
            let sid = p.id("sheet").unwrap_or(0);
            let vid = p.id("viewport").unwrap_or(0);
            let sheet = doc.sheet_mut(sid).ok_or_else(|| "Blatt nicht gefunden".to_string())?;
            let vp = sheet
                .viewports
                .iter_mut()
                .find(|v| v.id == vid)
                .ok_or_else(|| "Ansichtsfenster nicht gefunden".to_string())?;
            if let Some(x) = p.num("x") {
                vp.x = x;
            }
            if let Some(y) = p.num("y") {
                vp.y = y;
            }
            if let Some(w) = p.num("w") {
                vp.w = w;
            }
            if let Some(h) = p.num("h") {
                vp.h = h;
            }
            if let Some(s) = p.num("scale") {
                vp.scale = s;
            }
            if let Some(c) = p.pt("center") {
                vp.center = c;
            }
            if let Some(f) = p.boolean("show_frame") {
                vp.show_frame = f;
            }
            Ok(json!({"ok": true}))
        }
        "sheet.remove_viewport" => {
            let sid = p.id("sheet").unwrap_or(0);
            let vid = p.id("viewport").unwrap_or(0);
            if let Some(sh) = doc.sheet_mut(sid) {
                sh.viewports.retain(|v| v.id != vid);
            }
            Ok(json!({"ok": true}))
        }

        // ---------------- Dokument ----------------
        "doc.settings" => {
            if let Some(u) = p.text("input_unit") {
                if let Some(u) = Unit::from_str_loose(&u) {
                    doc.settings.input_unit = u;
                }
            }
            if let Some(u) = p.text("dim_unit") {
                if let Some(u) = Unit::from_str_loose(&u) {
                    doc.settings.dim_unit = u;
                }
            }
            if let Some(d) = p.num("decimals") {
                doc.settings.decimals = d.clamp(0.0, 4.0) as usize;
            }
            if let Some(s) = p.num("annotation_scale") {
                doc.settings.annotation_scale = s.max(1.0);
            }
            if let Some(g) = p.len("grid_step") {
                doc.settings.snap.grid_step = crate::model::clamp_grid_step(g);
            }
            if let Some(v) = p.boolean("show_grid") {
                doc.settings.show_grid = v;
            }
            if let Some(v) = p.len("text_height") {
                doc.settings.text_height = v;
            }
            if let Some(v) = p.len("wall_thickness") {
                doc.settings.wall_thickness = v;
            }
            if let Some(v) = p.len("door_width") {
                doc.settings.door_width = v;
            }
            if let Some(v) = p.len("window_width") {
                doc.settings.window_width = v;
            }
            if let Some(v) = p.len("stair_width") {
                doc.settings.stair_width = v;
            }
            if let Some(v) = p.boolean("show_rulers") {
                doc.settings.show_rulers = v;
            }
            if let Some(o) = p.v.get("snap") {
                let sp = Params::new(o, doc.settings.input_unit);
                for (k, target) in [
                    ("endpoint", &mut doc.settings.snap.endpoint),
                    ("midpoint", &mut doc.settings.snap.midpoint),
                    ("center", &mut doc.settings.snap.center),
                    ("intersection", &mut doc.settings.snap.intersection),
                    ("perpendicular", &mut doc.settings.snap.perpendicular),
                    ("nearest", &mut doc.settings.snap.nearest),
                    ("grid", &mut doc.settings.snap.grid),
                    ("ortho", &mut doc.settings.snap.ortho),
                    ("polar", &mut doc.settings.snap.polar),
                    ("wall_edges", &mut doc.settings.snap.wall_edges),
                ] {
                    if let Some(v) = sp.boolean(k) {
                        *target = v;
                    }
                }
                // `len` statt `num`: die Rasterweite kommt in der Eingabeeinheit
                // der Zeichnung (0,25 m), gespeichert wird in mm (250) — `num`
                // legte 0,25 mm an, sobald die Einheit nicht mm war.
                if let Some(g) = sp.len("grid_step") {
                    doc.settings.snap.grid_step = crate::model::clamp_grid_step(g);
                }
                if let Some(a) = sp.num("polar_step") {
                    doc.settings.snap.polar_step = a.clamp(1.0, 180.0);
                }
                if let Some(a) = sp.num("polar_tolerance") {
                    doc.settings.snap.polar_tolerance = a.clamp(0.0, 45.0);
                }
                if let Some(t) = sp.num("tolerance_px") {
                    doc.settings.snap.tolerance_px = t.clamp(1.0, 60.0);
                }
            }
            Ok(json!({"ok": true}))
        }
        "doc.project" => {
            macro_rules! setstr {
                ($($k:expr),* $(,)?) => {
                    $( if let Some(v) = p.text($k) {
                        match $k {
                            "project" => doc.project.project = v,
                            "client" => doc.project.client = v,
                            "planner" => doc.project.planner = v,
                            "plan_no" => doc.project.plan_no = v,
                            "plan_title" => doc.project.plan_title = v,
                            "address" => doc.project.address = v,
                            "note" => doc.project.note = v,
                            "date" => doc.project.date = v,
                            "revision" => doc.project.revision = v,
                            _ => {}
                        }
                    } )*
                };
            }
            setstr!(
                "project", "client", "planner", "plan_no", "plan_title", "address", "note", "date",
                "revision"
            );
            Ok(json!({"ok": true}))
        }
        "dim.style" => {
            let s = &mut doc.dim_style;
            if let Some(v) = p.num("text_height") {
                s.text_height = v;
            }
            if let Some(v) = p.num("arrow_size") {
                s.arrow_size = v;
            }
            if let Some(v) = p.text("arrow") {
                s.arrow = match v.to_lowercase().as_str() {
                    "arrow" | "pfeil" => ArrowKind::Arrow,
                    "dot" | "punkt" => ArrowKind::Dot,
                    "none" | "ohne" => ArrowKind::None,
                    _ => ArrowKind::Slash,
                };
            }
            if let Some(v) = p.num("text_gap") {
                s.text_gap = v;
            }
            if let Some(v) = p.num("ext_gap") {
                s.ext_gap = v;
            }
            if let Some(v) = p.num("ext_overshoot") {
                s.ext_overshoot = v;
            }
            if let Some(v) = p.num("chain_gap") {
                s.chain_gap = v;
            }
            if let Some(v) = p.num("decimals") {
                s.decimals = v.clamp(0.0, 4.0) as usize;
            }
            Ok(json!({"ok": true}))
        }

        // ---------------- Auswahl / Abfragen ----------------
        "select.all" => {
            let ids: Vec<Id> = doc
                .ents()
                .iter()
                .filter(|e| doc.layer(e.layer).map(|l| l.visible && !l.locked).unwrap_or(true))
                .map(|e| e.id)
                .collect();
            Ok(json!({"ok": true, "ids": ids}))
        }
        "select.query" => {
            let ids = query_ids(doc, params);
            Ok(json!({"ok": true, "ids": ids, "count": ids.len()}))
        }

        other => Err(format!("Aktion '{other}' ist in dieser Version nicht implementiert")),
    }
}

/// Objektfilter einer Abfrage: `ids`, `layer`, `type` und Bereich (`a`/`b`).
///
/// Eine Quelle für die Action `select.query` und `Session::query` (MCP-Werkzeug
/// `jjcad_query`) — sonst wirbt die Abfrage mit Filtern, die eine der beiden
/// Stellen still ignoriert. Unbekannte Werte (Typ, Ebene) treffen nichts, sie
/// liefern 0 Treffer statt der ganzen Liste.
pub fn query_ids(doc: &Document, params: &Value) -> Vec<Id> {
    let p = Params::new(params, doc.settings.input_unit);
    let ids = p.ids("ids");
    let layer = p.text("layer").filter(|l| !l.trim().is_empty());
    let typ = p.text("type").filter(|t| !t.trim().is_empty());
    let area = match (p.pt("a"), p.pt("b")) {
        (Some(a), Some(b)) => Some(BBox::from_corners(a, b)),
        _ => None,
    };
    doc.ents()
        .iter()
        .filter(|e| {
            if let Some(list) = &ids {
                if !list.contains(&e.id) {
                    return false;
                }
            }
            if let Some(l) = &layer {
                let name = doc.layer(e.layer).map(|x| x.name.clone()).unwrap_or_default();
                if !name.eq_ignore_ascii_case(l) && l != &e.layer.to_string() {
                    return false;
                }
            }
            if let Some(t) = &typ {
                let gesucht = t.to_lowercase();
                // Der Typ wird auf Deutsch angezeigt (`type_name`), die
                // Objektfelder tragen aber den Serde-Namen (`wall`, `dim`).
                // Beide Schreibweisen sollen filtern — ein Agent liest den
                // Namen aus dem Abfrageergebnis und fragt damit zurück.
                let tag = serde_json::to_value(&e.kind)
                    .ok()
                    .and_then(|v| v.get("type").and_then(|x| x.as_str()).map(|s| s.to_lowercase()));
                let passt = e.kind.type_name().to_lowercase().contains(&gesucht)
                    || tag.map(|s| s.contains(&gesucht)).unwrap_or(false);
                if !passt {
                    return false;
                }
            }
            if let Some(b) = &area {
                if !render::entity_bbox(doc, e).intersects(b) {
                    return false;
                }
            }
            true
        })
        .map(|e| e.id)
        .collect()
}

fn e_block_id(ents: &[Entity], id: Id) -> String {
    ents.iter()
        .find(|e| e.id == id)
        .and_then(|e| match &e.kind {
            EntityKind::BlockRef { block, .. } => Some(block.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Versetzen
// ---------------------------------------------------------------------------

fn offset_kind(e: &Entity, dist: f64, side: Option<Vec2>) -> Option<EntityKind> {
    match &e.kind {
        EntityKind::Line { a, b } => {
            let dir = (*b - *a).normalized();
            let n = dir.perp();
            let sign = match side {
                Some(s) => {
                    if (s - *a).dot(n) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                None => 1.0,
            };
            Some(EntityKind::Line {
                a: *a + n * (dist * sign),
                b: *b + n * (dist * sign),
            })
        }
        EntityKind::Wall {
            a,
            b,
            thickness,
            height,
            material,
            hatch,
            align,
        } => {
            let dir = (*b - *a).normalized();
            let n = dir.perp();
            let sign = match side {
                Some(s) => {
                    if (s - *a).dot(n) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                None => 1.0,
            };
            Some(EntityKind::Wall {
                hatch: *hatch,
                align: *align,
                a: *a + n * (dist * sign),
                b: *b + n * (dist * sign),
                thickness: *thickness,
                height: *height,
                material: material.clone(),
            })
        }
        EntityKind::Circle { c, r } => {
            let sign = match side {
                Some(s) => {
                    if s.dist(*c) >= *r {
                        1.0
                    } else {
                        -1.0
                    }
                }
                None => 1.0,
            };
            let nr = r + dist * sign;
            if nr <= 1.0 {
                None
            } else {
                Some(EntityKind::Circle { c: *c, r: nr })
            }
        }
        EntityKind::Polyline { verts, closed } => {
            let pts: Vec<Vec2> = verts.iter().map(|q| q.p).collect();
            let sign = if let Some(s) = side {
                if point_in_polygon(s, &pts) {
                    -1.0
                } else {
                    1.0
                }
            } else {
                1.0
            };
            let off = offset_polyline(&pts, *closed, dist * sign);
            Some(EntityKind::Polyline {
                verts: off.into_iter().map(Vertex::new).collect(),
                closed: *closed,
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Stutzen / Verlängern
// ---------------------------------------------------------------------------

fn cutter_segments(doc: &Document, exclude: &[Id]) -> Vec<(Seg, Id)> {
    let mut out = Vec::new();
    for e in doc.ents() {
        if exclude.contains(&e.id) {
            continue;
        }
        if let Some(l) = doc.layer(e.layer) {
            if !l.visible {
                continue;
            }
        }
        for s in render::entity_segments(doc, e) {
            out.push((s, e.id));
        }
    }
    out
}

fn trim_entity(doc: &mut Document, id: Id, at: Vec2, cutters: &[Id]) -> Result<Vec<Id>, String> {
    let Some(e) = doc.entity(id).cloned() else {
        return Err(format!("Objekt {id} nicht gefunden"));
    };
    let exclude: Vec<Id> = if cutters.is_empty() {
        vec![id]
    } else {
        let mut v = vec![id];
        v.extend_from_slice(cutters);
        v
    };
    let segs = cutter_segments(doc, &exclude);
    // Für Stutzen: alle sichtbaren Objekte außer dem Ziel sind Schnittkanten
    let exclude2 = vec![id];
    let segs = if cutters.is_empty() {
        cutter_segments(doc, &exclude2)
    } else {
        segs
    };

    match &e.kind {
        EntityKind::Line { a, b } => {
            let len = a.dist(*b);
            if len < 1e-6 {
                return Ok(vec![]);
            }
            let dir = (*b - *a).normalized();
            let mut ts = vec![0.0, 1.0];
            for (sg, _) in &segs {
                if let Seg::Line(c, d) = sg {
                    if let Some(x) = line_line(*a, *b, *c, *d, false, false) {
                        let t = (x - *a).dot(dir) / len;
                        if t > 1e-6 && t < 1.0 - 1e-6 {
                            ts.push(t);
                        }
                    }
                } else if let Seg::Arc(arc) = sg {
                    for x in line_circle(*a, *b, arc.c, arc.r) {
                        if angle_in_arc((x - arc.c).angle(), arc.a0, arc.a1) {
                            let t = (x - *a).dot(dir) / len;
                            if t > 1e-6 && t < 1.0 - 1e-6 {
                                ts.push(t);
                            }
                        }
                    }
                }
            }
            ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
            ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            let tclick = (at - *a).dot(dir) / len;
            let mut span: Option<(f64, f64)> = None;
            for w in ts.windows(2) {
                if tclick >= w[0] - 1e-9 && tclick <= w[1] + 1e-9 {
                    span = Some((w[0], w[1]));
                    break;
                }
            }
            let Some((t0, t1)) = span else {
                return Ok(vec![]);
            };
            if t0 <= 1e-9 && t1 >= 1.0 - 1e-9 {
                doc.remove_entity(id);
                return Ok(vec![]);
            }
            let mut created = Vec::new();
            if t0 <= 1e-9 {
                if let Some(em) = doc.entity_mut(id) {
                    if let EntityKind::Line { a, .. } = &mut em.kind {
                        *a = a.lerp(*b, t1);
                    }
                }
            } else if t1 >= 1.0 - 1e-9 {
                if let Some(em) = doc.entity_mut(id) {
                    if let EntityKind::Line { b, .. } = &mut em.kind {
                        *b = a.lerp(*b, t0);
                    }
                }
            } else {
                // Zweiteilig: erstes Stück behalten, zweites neu
                let mid = a.lerp(*b, t1);
                let end = *b;
                let a0 = a.lerp(*b, t0);
                if let Some(em) = doc.entity_mut(id) {
                    if let EntityKind::Line { b, .. } = &mut em.kind {
                        *b = a0;
                    }
                }
                let nid = doc.new_id();
                let mut ne = Entity::new(nid, e.layer, EntityKind::Line { a: mid, b: end });
                ne.style = e.style.clone();
                doc.ents_mut().push(ne);
                created.push(nid);
            }
            Ok(created)
        }
        EntityKind::Wall {
            a,
            b,
            thickness,
            height,
            material,
            hatch,
            align,
        } => {
            let len = a.dist(*b);
            if len < 1e-6 {
                return Ok(vec![]);
            }
            let dir = (*b - *a).normalized();
            let mut ts = vec![0.0, 1.0];
            for (sg, _) in &segs {
                if let Seg::Line(c, d) = sg {
                    if let Some(x) = line_line(*a, *b, *c, *d, false, false) {
                        let t = (x - *a).dot(dir) / len;
                        if t > 1e-6 && t < 1.0 - 1e-6 {
                            ts.push(t);
                        }
                    }
                }
            }
            ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
            ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            let tclick = (at - *a).dot(dir) / len;
            let mut span: Option<(f64, f64)> = None;
            for w in ts.windows(2) {
                if tclick >= w[0] - 1e-9 && tclick <= w[1] + 1e-9 {
                    span = Some((w[0], w[1]));
                    break;
                }
            }
            let Some((t0, t1)) = span else {
                return Ok(vec![]);
            };
            if t0 <= 1e-9 && t1 >= 1.0 - 1e-9 {
                doc.remove_entity(id);
                return Ok(vec![]);
            }
            let make = |aa: Vec2, bb: Vec2| EntityKind::Wall {
                hatch: *hatch,
                align: *align,
                a: aa,
                b: bb,
                thickness: *thickness,
                height: *height,
                material: material.clone(),
            };
            let mut created = Vec::new();
            if t0 <= 1e-9 {
                if let Some(em) = doc.entity_mut(id) {
                    em.kind = make(a.lerp(*b, t1), *b);
                }
            } else if t1 >= 1.0 - 1e-9 {
                if let Some(em) = doc.entity_mut(id) {
                    em.kind = make(*a, a.lerp(*b, t0));
                }
            } else {
                if let Some(em) = doc.entity_mut(id) {
                    em.kind = make(*a, a.lerp(*b, t0));
                }
                let nid = doc.new_id();
                let mut ne = Entity::new(nid, e.layer, make(a.lerp(*b, t1), *b));
                ne.style = e.style.clone();
                doc.ents_mut().push(ne);
                created.push(nid);
            }
            Ok(created)
        }
        EntityKind::Circle { c, r } => {
            let mut hits = Vec::new();
            for (sg, _) in &segs {
                for x in seg_seg(&Seg::Line(*c, *c + v(1.0, 0.0)), sg) {
                    let _ = x;
                }
                match sg {
                    Seg::Line(a, b) => {
                        for x in line_circle(*a, *b, *c, *r) {
                            hits.push((x - *c).angle());
                        }
                    }
                    Seg::Arc(_) => {}
                }
            }
            hits.sort_by(|x, y| x.partial_cmp(y).unwrap());
            hits.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            if hits.len() < 2 {
                return Err("Kreis braucht zwei Schnittpunkte zum Stutzen".into());
            }
            // Bogen wählen, der den Klickwinkel nicht enthält
            let aclick = (at - *c).angle();
            let mut best: Option<(f64, f64)> = None;
            for i in 0..hits.len() {
                let a0 = hits[i];
                let a1 = hits[(i + 1) % hits.len()];
                let contains = angle_in_arc(aclick, a0, a1);
                if !contains {
                    best = Some((a0, a1));
                    break;
                }
            }
            if let Some((a0, a1)) = best {
                if let Some(em) = doc.entity_mut(id) {
                    em.kind = EntityKind::Arc {
                        c: *c,
                        r: *r,
                        a0,
                        a1,
                    };
                }
            }
            Ok(vec![])
        }
        _ => Err("Stutzen wird für diesen Objekttyp nicht unterstützt".into()),
    }
}

fn extend_entity(doc: &mut Document, id: Id, at: Vec2, cutters: &[Id]) -> Result<(), String> {
    let Some(e) = doc.entity(id).cloned() else {
        return Err(format!("Objekt {id} nicht gefunden"));
    };
    let exclude = vec![id];
    let segs = if cutters.is_empty() {
        cutter_segments(doc, &exclude)
    } else {
        cutter_segments(doc, &exclude)
            .into_iter()
            .filter(|(_, sid)| cutters.contains(sid))
            .collect()
    };
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
            let near_b = at.dist(*b) <= at.dist(*a);
            let (fixed, moving) = if near_b { (*a, *b) } else { (*b, *a) };
            let dir = (moving - fixed).normalized();
            let ray_end = fixed + dir * (fixed.dist(moving) * 1000.0 + 1e9);
            let mut best: Option<(f64, Vec2)> = None;
            for (sg, _) in &segs {
                let hits = match sg {
                    Seg::Line(c, d) => line_line(fixed, ray_end, *c, *d, true, false)
                        .into_iter()
                        .collect::<Vec<_>>(),
                    Seg::Arc(arc) => line_circle(fixed, ray_end, arc.c, arc.r)
                        .into_iter()
                        .filter(|p| angle_in_arc((*p - arc.c).angle(), arc.a0, arc.a1))
                        .collect(),
                };
                for h in hits {
                    let t = (h - fixed).dot(dir);
                    let t0 = (moving - fixed).dot(dir);
                    if t > t0 + 1e-6 {
                        if best.map(|(bt, _)| t < bt).unwrap_or(true) {
                            best = Some((t, h));
                        }
                    }
                }
            }
            let Some((_, hp)) = best else {
                return Err("Keine Schnittkante zum Verlängern gefunden".into());
            };
            if let Some(em) = doc.entity_mut(id) {
                match &mut em.kind {
                    EntityKind::Line { a, b } => {
                        if near_b {
                            *b = hp;
                        } else {
                            *a = hp;
                        }
                    }
                    EntityKind::Wall { a, b, .. } => {
                        if near_b {
                            *b = hp;
                        } else {
                            *a = hp;
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        }
        _ => Err("Verlängern wird für diesen Objekttyp nicht unterstützt".into()),
    }
}

// ---------------------------------------------------------------------------
// Zerlegen
// ---------------------------------------------------------------------------

/// Zerlegt ein Objekt in Grundelemente (auch Blöcke und Bemaßungen).
pub fn explode_entity(doc: &mut Document, id: Id) -> Result<Vec<Id>, String> {
    let Some(e) = doc.entity(id).cloned() else {
        return Err(format!("Objekt {id} nicht gefunden"));
    };
    if matches!(e.kind, EntityKind::BlockRef { .. }) && doc.block(match &e.kind {
        EntityKind::BlockRef { block, .. } => block,
        _ => "",
    }).is_none() {
        return Err("Blockdefinition fehlt".into());
    }
    // Ein Text wird nicht über die Displayliste zerlegt, sondern über die
    // Umrisse seiner Zeichen — und nur mit Schriften, die dafür vorgesehen sind.
    if matches!(e.kind, EntityKind::Text { .. }) {
        return explode_text(doc, &e);
    }
    let ctx = render::RenderCtx::new(doc);
    let mut dl = render::DisplayList::default();
    render::render_entity(&ctx, &e, Affine::IDENTITY, &mut dl, 0);
    let mut created = Vec::new();
    let mut new_ents = Vec::new();
    for prim in dl.prims {
        let kind = match &prim.shape {
            Shape::Line(a, b) => {
                if a.dist(*b) < 1e-9 {
                    None
                } else {
                    Some(EntityKind::Line { a: *a, b: *b })
                }
            }
            Shape::Poly(pts, closed) => {
                if pts.len() < 2 {
                    None
                } else {
                    Some(EntityKind::Polyline {
                        verts: pts.iter().map(|p| Vertex::new(*p)).collect(),
                        closed: *closed,
                    })
                }
            }
            Shape::Circle { c, r } => Some(EntityKind::Circle { c: *c, r: *r }),
            Shape::Arc { c, r, a0, a1 } => Some(EntityKind::Arc {
                c: *c,
                r: *r,
                a0: *a0,
                a1: *a1,
            }),
            Shape::Ellipse { c, rx, ry, rot } => Some(EntityKind::Ellipse {
                c: *c,
                rx: *rx,
                ry: *ry,
                rot: *rot,
            }),
            Shape::Fill { outer, holes } => {
                if outer.len() >= 3 {
                    Some(EntityKind::Fill {
                        boundary: outer.clone(),
                        holes: holes.clone(),
                        pattern: FillPattern::Solid,
                        pattern_angle: 0.0,
                        pattern_scale: 100.0,
                        border: false,
                    })
                } else {
                    None
                }
            }
            Shape::Dot { p, r } => Some(EntityKind::Point { p: *p }).map(|k| {
                let _ = r;
                k
            }),
            Shape::Text(t) => Some(EntityKind::Text {
                p: t.pos,
                text: t.text.clone(),
                height: t.height,
                angle: t.angle,
                align: t.align,
                valign: t.valign,
                bold: t.bold,
                italic: t.italic,
                underline: t.underline,
                superscript: t.superscript,
                runs: t.runs.clone(),
                font: t.font,
            }),
        };
        if let Some(kind) = kind {
            let nid = doc.new_id();
            let mut ne = Entity::new(nid, e.layer, kind);
            ne.style.color = Some(prim.style.color);
            ne.style.line_type = Some(prim.style.line_type);
            ne.style.line_weight = Some(prim.style.width.max(0.05));
            ne.style.fill = prim.style.fill;
            new_ents.push(ne);
            created.push(nid);
        }
    }
    doc.remove_entity(id);
    for ne in new_ents {
        doc.ents_mut().push(ne);
    }
    doc.modified = true;
    Ok(created)
}

// ---------------------------------------------------------------------------
// PDF einfügen
// ---------------------------------------------------------------------------

/// Skaliert die Geometrie eines importierten Objekts (Papier → Zeichnung).
fn scale_kind(kind: &EntityKind, f: f64) -> EntityKind {
    if (f - 1.0).abs() < 1e-12 {
        return kind.clone();
    }
    let s = |p: &Vec2| Vec2::new(p.x * f, p.y * f);
    match kind {
        EntityKind::Line { a, b } => EntityKind::Line { a: s(a), b: s(b) },
        EntityKind::Polyline { verts, closed } => EntityKind::Polyline {
            verts: verts
                .iter()
                .map(|v| Vertex::new(Vec2::new(v.p.x * f, v.p.y * f)))
                .collect(),
            closed: *closed,
        },
        EntityKind::Circle { c, r } => EntityKind::Circle { c: s(c), r: r * f },
        EntityKind::Arc { c, r, a0, a1 } => EntityKind::Arc {
            c: s(c),
            r: r * f,
            a0: *a0,
            a1: *a1,
        },
        EntityKind::Ellipse { c, rx, ry, rot } => EntityKind::Ellipse {
            c: s(c),
            rx: rx * f,
            ry: ry * f,
            rot: *rot,
        },
        EntityKind::Fill {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
        } => EntityKind::Fill {
            boundary: boundary.iter().map(s).collect(),
            holes: holes.iter().map(|h| h.iter().map(s).collect()).collect(),
            pattern: *pattern,
            pattern_angle: *pattern_angle,
            pattern_scale: *pattern_scale,
            border: *border,
        },
        EntityKind::Text {
            p,
            text,
            height,
            angle,
            align,
            valign,
            bold,
            italic,
            underline,
            superscript,
            runs,
            font,
        } => EntityKind::Text {
            p: s(p),
            text: text.clone(),
            height: height * f,
            angle: *angle,
            align: *align,
            valign: *valign,
            bold: *bold,
            italic: *italic,
            underline: *underline,
            superscript: *superscript,
            runs: runs.clone(),
            font: TextFont::Sans,
        },
        other => other.clone(),
    }
}

/// `file.import.pdf` — eine PDF-Seite als **ein** Element einfügen.
///
/// Parameter: `path`, `page` (1-basiert, Standard 1), `scale` (Maßstab der
/// PDF-Seite, Standard = Zeichnungsmaßstab; `1` = maßstäblich in Papier-mm),
/// `layer`, `as_block` (Standard `true` = BlockRef, sonst Einzelobjekte).
fn import_pdf_action(doc: &mut Document, p: &Params) -> Result<Value, String> {
    let path = p
        .text("path")
        .ok_or_else(|| "Parameter 'path' fehlt".to_string())?;
    let page_nr = p.num_or("page", 1.0).max(1.0) as usize;
    // Maßstab, in dem die PDF-Seite gezeichnet ist. Vorgabe ist der
    // Zeichnungsmaßstab (1:100): dann liegt das eingefügte Blatt auf Papier
    // genau so groß wie das Original, und Strichstärken stimmen — Linien sind
    // in JoJa-CAD Papier-Millimeter und werden mit demselben Maßstab gerechnet.
    let massstab = p
        .num_or("scale", doc.settings.annotation_scale)
        .abs()
        .max(1e-9);
    let as_block = p.boolean_or("as_block", true);
    let liest = crate::pdf_read::read_pdf(std::path::Path::new(&path))?;
    let seite = liest.page(page_nr).ok_or_else(|| {
        format!(
            "Seite {page_nr} gibt es nicht — das PDF hat {} Seite(n)",
            liest.pages.len()
        )
    })?;
    let layer = if p.v.get("layer").is_some() {
        resolve_layer(doc, p)
    } else {
        // Eigene Ebene, damit sich der Import ausblenden und sperren lässt.
        doc.ensure_layer("PDF-Import", Color::rgb(45, 105, 190))
    };
    let dateiname = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "PDF".to_string());

    let mut ents: Vec<Entity> = Vec::with_capacity(seite.entities.len());
    for (i, ie) in seite.entities.iter().enumerate() {
        let kind = scale_kind(&ie.kind, massstab);
        let mut e = Entity::new(i as Id + 1, layer, kind);
        e.style.color = Some(ie.color);
        e.style.line_weight = Some(ie.width.max(0.05));
        e.style.line_type = Some(ie.line_type);
        e.style.fill = ie.fill;
        ents.push(e);
    }
    let objekte = ents.len();

    let mut warn: Vec<Value> = seite
        .warnings
        .counts
        .iter()
        .map(|(art, anzahl)| json!({"art": art, "anzahl": anzahl}))
        .collect();
    warn.sort_by(|a, b| a["art"].as_str().cmp(&b["art"].as_str()));
    // Andere Seiten interessieren nur, wenn sie etwas zu melden haben.
    let mut weitere = 0;
    for s in &liest.pages {
        if s.number != page_nr && !s.warnings.is_empty() {
            weitere += 1;
        }
    }

    if !as_block {
        let mut ids = Vec::with_capacity(ents.len());
        for mut e in ents {
            let id = doc.new_id();
            e.id = id;
            ids.push(id);
            doc.add_entity(e);
        }
        doc.modified = true;
        return Ok(json!({
            "ok": true,
            "created": ids,
            "page": page_nr,
            "size_mm": [seite.width_mm, seite.height_mm],
            "warnings": warn,
        }));
    }

    let block_id = format!("pdf:{}", crate::util::short_id());
    let block_name = format!("PDF {} S.{page_nr}", dateiname);
    doc.blocks.push(BlockDef {
        id: block_id.clone(),
        name: block_name.clone(),
        category: "PDF-Import".to_string(),
        source: BlockSource::Project,
        description: format!(
            "Aus {} eingefügt, Seite {page_nr}, {objekte} Objekte",
            dateiname
        ),
        entities: ents,
        icon: None,
        base: Vec2::ZERO,
    });
    let idn = doc.new_id();
    let mut inst = Entity::new(
        idn,
        layer,
        EntityKind::BlockRef {
            block: block_id.clone(),
            pos: Vec2::ZERO,
            rot: 0.0,
            scale: v(1.0, 1.0),
            mirror: false,
        },
    );
    inst.style.color = None;
    doc.add_entity(inst);
    doc.modified = true;
    Ok(json!({
        "ok": true,
        "created": [idn],
        "block": block_name,
        "page": page_nr,
        "pages": liest.pages.len(),
        "size_mm": [seite.width_mm, seite.height_mm],
        "entities": objekte,
        "warnings": warn,
        "warnings_other_pages": weitere,
    }))
}


// ---------------------------------------------------------------------------
// Text in Linien zerlegen
// ---------------------------------------------------------------------------

/// Zerlegt einen Text in die Umrisslinien seiner Zeichen.
///
/// Nur die dafür vorgesehenen Schriften lassen sich zerlegen: konturierte
/// Gebrauchsschriften ergeben ein paar Polygonzüge je Zeichen, runde
/// Zierschriften sehr viele kleine Striche — die werden mit klarer Meldung
/// abgelehnt (siehe [`crate::model::TextFont::explodierbar`]).
fn explode_text(doc: &mut Document, e: &Entity) -> Result<Vec<Id>, String> {
    let EntityKind::Text {
        font,
        height,
        p,
        angle,
        align,
        valign,
        text,
        runs,
        bold,
        italic,
        underline,
        superscript,
    } = &e.kind
    else {
        return Err("Kein Text".into());
    };
    if !font.explodierbar() {
        return Err(format!(
            "Schrift „{}“ lässt sich nicht in Linien zerlegen — {}",
            font.label(),
            font.explode_hinweis()
        ));
    }
    // Läufe wie im Satz: dieselben Stücke, dieselben Vorschübe.
    let runs_eff: Vec<TextRun> = if runs.is_empty() {
        vec![TextRun {
            text: text.clone(),
            bold: *bold,
            italic: *italic,
            underline: *underline,
            superscript: *superscript,
        }]
    } else {
        runs.clone()
    };
    let zeilen = crate::text::layout_mit_schrift(text, &runs_eff, *font);
    if zeilen.is_empty() {
        return Err("Der Text ist leer".into());
    }
    let size = *height;
    let lh = size * 1.25;
    let n = zeilen.len() as f64;
    let dy0 = match valign {
        TextAlignV::Top => -size * 0.8,
        TextAlignV::Middle => -size * 0.36 + (n - 1.0) * lh * 0.5,
        TextAlignV::Bottom => (n - 1.0) * lh,
    };
    let (sin, cos) = angle.sin_cos();
    let platz = |dx: f64, dy: f64| -> Vec2 {
        v(
            p.x + dx * cos - dy * sin,
            p.y + dx * sin + dy * cos,
        )
    };

    let mut konturen: Vec<(Vec<Vec2>, bool)> = Vec::new();
    for (i, zeile) in zeilen.iter().enumerate() {
        let breite = zeile.width_em * size;
        let dx0 = match align {
            TextAlignH::Left => 0.0,
            TextAlignH::Center => -breite / 2.0,
            TextAlignH::Right => -breite,
        };
        let mut pen = dx0;
        let dy_zeile = dy0 - i as f64 * lh;
        for seg in &zeile.segs {
            let Some(data) = crate::fonts::ttf_bytes(*font, seg.style) else {
                continue;
            };
            let Some(font_ref) = crate::fonts::font_from_bytes(data) else {
                continue;
            };
            let seg_size = size * seg.scale;
            let upem = font_ref.units_per_em().unwrap_or(1000.0) as f64;
            let unit = seg_size / upem;
            let scaled = font_ref.as_scaled(seg_size as f32);
            let dy_seg = dy_zeile + seg.rise_em * size;
            for ch in seg.text.chars() {
                let gid = font_ref.glyph_id(ch);
                if ch != ' ' {
                    if let Some(outline) = font_ref.outline(gid) {
                        let mut aktuell: Vec<Vec2> = Vec::new();
                        let mut start: Option<Vec2> = None;
                        let mut ende: Option<Vec2> = None;
                        for curve in &outline.curves {
                            // Alle Kurven in Welt-mm umrechnen (Drehung inklusive).
                            let tf = |q: ab_glyph::Point| -> Vec2 {
                                platz(pen + q.x as f64 * unit, dy_seg + q.y as f64 * unit)
                            };
                            let (a, b) = match curve {
                                ab_glyph::OutlineCurve::Line(a, b) => (tf(*a), tf(*b)),
                                ab_glyph::OutlineCurve::Quad(a, _, b) => (tf(*a), tf(*b)),
                                ab_glyph::OutlineCurve::Cubic(a, _, _, b) => (tf(*a), tf(*b)),
                            };
                            // Neuer Teilpfad, wenn der vorige hier nicht endet.
                            let neu = match ende {
                                None => true,
                                Some(x) => x.dist(a) > 1e-6,
                            };
                            if neu {
                                if aktuell.len() >= 2 {
                                    konturen.push((std::mem::take(&mut aktuell), false));
                                }
                                aktuell.clear();
                                aktuell.push(a);
                                start = Some(a);
                            }
                            match curve {
                                ab_glyph::OutlineCurve::Line(_, b) => aktuell.push(tf(*b)),
                                ab_glyph::OutlineCurve::Quad(_, c, b) => {
                                    abflachen_quad(a, tf(*c), tf(*b), &mut aktuell)
                                }
                                ab_glyph::OutlineCurve::Cubic(_, c1, c2, b) => {
                                    abflachen_kubisch(a, tf(*c1), tf(*c2), tf(*b), 0, &mut aktuell)
                                }
                            }
                            ende = Some(b);
                        }
                        if let (Some(first), Some(last)) = (start, ende) {
                            let geschlossen = first.dist(last) < seg_size * 0.02;
                            if aktuell.len() >= 2 {
                                konturen.push((aktuell, geschlossen));
                            }
                        }
                    }
                }
                pen += scaled.h_advance(gid) as f64;
            }
        }
    }
    if konturen.is_empty() {
        return Err("Der Text enthält keine zeichenbaren Umrisse".into());
    }

    let mut neue = Vec::with_capacity(konturen.len());
    let mut created = Vec::with_capacity(konturen.len());
    for (pts, geschlossen) in konturen {
        if pts.len() < 2 {
            continue;
        }
        let nid = doc.new_id();
        let mut ne = Entity::new(
            nid,
            e.layer,
            EntityKind::Polyline {
                verts: pts.iter().map(|q| Vertex::new(*q)).collect(),
                closed: geschlossen,
            },
        );
        ne.style = e.style.clone();
        // Umrisse statt gefüllter Zeichen: mit dem Layer-Stift (0,35 mm) wirkt
        // ein kleiner Text wie ein schwarzer Block. Ein dünner Stift lässt die
        // Linien als Linien erkennen; eine eigene Stärke am Objekt bleibt.
        if ne.style.line_weight.is_none() {
            ne.style.line_weight = Some(0.18);
        }
        neue.push(ne);
        created.push(nid);
    }
    doc.remove_entity(e.id);
    for ne in neue {
        doc.ents_mut().push(ne);
    }
    doc.modified = true;
    Ok(created)
}

/// Quadratische Kurve in Welt-mm abflachen (Toleranz 0,02 mm).
fn abflachen_quad(a: Vec2, c: Vec2, b: Vec2, out: &mut Vec<Vec2>) {
    let d = abstand(a, c, b);
    if d <= 0.02 {
        out.push(b);
        return;
    }
    let ab = (a + c) * 0.5;
    let bc = (c + b) * 0.5;
    let m = (ab + bc) * 0.5;
    abflachen_quad(a, ab, m, out);
    abflachen_quad(m, bc, b, out);
}

/// Kubische Kurve in Welt-mm abflachen.
fn abflachen_kubisch(a: Vec2, c1: Vec2, c2: Vec2, b: Vec2, tiefe: u32, out: &mut Vec<Vec2>) {
    let d = abstand(a, c1, b).max(abstand(a, c2, b));
    if d <= 0.02 || tiefe >= 10 {
        out.push(b);
        return;
    }
    let p01 = (a + c1) * 0.5;
    let p12 = (c1 + c2) * 0.5;
    let p23 = (c2 + b) * 0.5;
    let p012 = (p01 + p12) * 0.5;
    let p123 = (p12 + p23) * 0.5;
    let m = (p012 + p123) * 0.5;
    abflachen_kubisch(a, p01, p012, m, tiefe + 1, out);
    abflachen_kubisch(m, p123, p23, b, tiefe + 1, out);
}

/// Abstand eines Kontrollpunkts von der Sehne a–b.
fn abstand(a: Vec2, c: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l = d.len();
    if l < 1e-9 {
        return c.dist(a);
    }
    ((d.x * (a.y - c.y) - (a.x - c.x) * d.y) / l).abs()
}

// ---------------------------------------------------------------------------
// Hilfen für GUI/MCP
// ---------------------------------------------------------------------------

/// Aktionen für die Funktionsliste im Inventar.
pub fn all_actions() -> &'static [ActionDef] {
    ACTIONS
}

/// Kurzbeschreibung des Aktionskatalogs als Markdown (für Hilfe/MCP).
pub fn catalog_markdown() -> String {
    let mut s = String::new();
    for g in [
        ActionGroup::Datei,
        ActionGroup::Zeichnen,
        ActionGroup::Architektur,
        ActionGroup::Aendern,
        ActionGroup::Bemassung,
        ActionGroup::Bloecke,
        ActionGroup::Blatt,
        ActionGroup::Ansicht,
    ] {
        s.push_str(&format!("### {}\n\n", g.label()));
        for a in actions_in_group(g) {
            s.push_str(&format!(
                "- `{}` — {} — {}\n",
                a.id, a.label, a.hint
            ));
        }
        s.push('\n');
    }
    s
}

/// Systemblock-Kategorien.
pub fn block_categories() -> Vec<&'static str> {
    blocks_lib::categories()
}
