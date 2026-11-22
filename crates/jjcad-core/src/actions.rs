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
    /// Katalogschlüssel der Gruppenbeschriftung (T0103).
    pub fn key(self) -> &'static str {
        match self {
            ActionGroup::Datei => "group.datei",
            ActionGroup::Zeichnen => "group.zeichnen",
            ActionGroup::Architektur => "group.architektur",
            ActionGroup::Aendern => "group.aendern",
            ActionGroup::Bemassung => "group.bemassung",
            ActionGroup::Bloecke => "group.bloecke",
            ActionGroup::Blatt => "group.blatt",
            ActionGroup::Ansicht => "group.ansicht",
            ActionGroup::Hilfe => "group.hilfe",
        }
    }

    /// Beschriftung in der eingestellten Sprache.
    pub fn label(self) -> &'static str {
        crate::i18n::tr(self.key())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ActionDef {
    pub id: &'static str,
    /// Katalogschlüssel der Beschriftung (T0103) — der Text wird beim Zeichnen
    /// über [`crate::i18n`] nachgeschlagen, damit die Sprache zur Laufzeit
    /// wechseln kann. Eine `const`-Tabelle kann keinen `String` tragen.
    pub label_key: &'static str,
    pub group: ActionGroup,
    /// Icon-Name für den GUI-Zeichner
    pub icon: &'static str,
    /// Standard-Tastenkürzel ("" = keins)
    pub key: &'static str,
    /// Katalogschlüssel des Kurzhinweises (T0103)
    pub hint_key: &'static str,
    /// Interaktives Werkzeug (braucht Punkt-Eingaben)?
    pub tool: bool,
}

impl ActionDef {
    /// Beschriftung in der eingestellten Sprache (T0103). Der Text wird hier
    /// nachgeschlagen, nicht in der Tabelle gespeichert — nur so wirkt eine
    /// Sprachumschaltung ohne Neustart.
    pub fn label(&self) -> &'static str {
        crate::i18n::tr(self.label_key)
    }

    /// Kurzhinweis in der eingestellten Sprache.
    pub fn hint(&self) -> &'static str {
        crate::i18n::tr(self.hint_key)
    }
}

macro_rules! act {
    ($id:expr, $label:expr, $group:ident, $icon:expr, $key:expr, $hint:expr, $tool:expr) => {
        ActionDef {
            id: $id,
            label_key: $label,
            group: ActionGroup::$group,
            icon: $icon,
            key: $key,
            hint_key: $hint,
            tool: $tool,
        }
    };
}

pub const ACTIONS: &[ActionDef] = &[
    // Datei
    act!("file.new", "action.file.new.label", Datei, "new", "ctrl+n", "action.file.new.hint", false),
    act!("file.open", "action.file.open.label", Datei, "open", "ctrl+o", "action.file.open.hint", false),
    act!("file.save", "action.file.save.label", Datei, "save", "ctrl+s", "action.file.save.hint", false),
    // T0129: Stände der Sicherung (unabhängig vom Speichern).
    act!("file.restore", "action.file.restore.label", Datei, "datei", "", "action.file.restore.hint", false),
    act!("file.save_as", "action.file.save_as.label", Datei, "save", "ctrl+shift+s", "action.file.save_as.hint", false),
    act!("file.export_pdf", "action.file.export_pdf.label", Datei, "pdf", "ctrl+p", "action.file.export_pdf.hint", false),
    act!("file.export_dxf", "action.file.export_dxf.label", Datei, "dxf", "", "action.file.export_dxf.hint", false),
    act!("file.import_dxf", "action.file.import_dxf.label", Datei, "import", "", "action.file.import_dxf.hint", false),
    act!(
        "file.import.pdf",
        "action.file.import.pdf.label",
        Datei,
        "pdf",
        "",
        "action.file.import.pdf.hint",
        false
    ),
    act!("file.export_png", "action.file.export_png.label", Datei, "image", "", "action.file.export_png.hint", false),
    // Bild einfügen: der Dialog wählt die Datei, das Werkzeug `draw.image`
    // fragt danach die beiden Ecken des Rechtecks ab.
    act!(
        "file.import_image",
        "action.file.import_image.label",
        Datei,
        "image",
        "",
        "action.file.import_image.hint",
        false
    ),
    act!("edit.undo", "action.edit.undo.label", Aendern, "undo", "ctrl+z", "action.edit.undo.hint", false),
    act!("edit.redo", "action.edit.redo.label", Aendern, "redo", "ctrl+y", "action.edit.redo.hint", false),
    // Kopieren ohne Basispunkt: legt die Auswahl nur in die Zwischenablage.
    // Eingefügt wird danach mit der Vorschau (Strg+V, Knopf „Einfügen").
    act!("edit.copy", "action.edit.copy.label", Aendern, "copy", "ctrl+c", "action.edit.copy.hint", false),

    // Zeichnen
    act!("draw.line", "action.draw.line.label", Zeichnen, "line", "l", "action.draw.line.hint", true),
    act!("draw.polyline", "action.draw.polyline.label", Zeichnen, "polyline", "o", "action.draw.polyline.hint", true),
    // Pfad (T0229): Linien und Bögen am Stück — jedes Stück beginnt am Ende
    // des vorigen, Umschalt wechselt zwischen Linie und Bogen.
    act!("draw.path", "action.draw.path.label", Zeichnen, "polyline", "", "action.draw.path.hint", true),
    // Parallele (T0200): eigene Linie/Polylinie als Vorlage, Abstand am
    // Mauszeiger. Liegt im Reiter „Zeichnen", weil sie neues Zeichnen erzeugt.
    act!("draw.parallel", "action.draw.parallel.label", Zeichnen, "parallel", "", "action.draw.parallel.hint", true),
    act!("draw.spline", "action.draw.spline.label", Zeichnen, "spline", "u", "action.draw.spline.hint", true),
    act!("draw.rect", "action.draw.rect.label", Zeichnen, "rect", "", "action.draw.rect.hint", true),
    act!("draw.circle", "action.draw.circle.label", Zeichnen, "circle", "k", "action.draw.circle.hint", true),
    act!("draw.arc", "action.draw.arc.label", Zeichnen, "arc", "", "action.draw.arc.hint", true),
    act!("draw.point", "action.draw.point.label", Zeichnen, "point", "p", "action.draw.point.hint", true),
    act!("draw.fill", "action.draw.fill.label", Zeichnen, "fill", "", "action.draw.fill.hint", true),
    act!("draw.fill.boundary", "action.draw.fill.boundary.label", Zeichnen, "fill", "i", "action.draw.fill.boundary.hint", true),
    act!("draw.fill.points", "action.draw.fill.points.label", Zeichnen, "fill", "shift+i", "action.draw.fill.points.hint", true),
    act!("draw.fill.paint", "action.draw.fill.paint.label", Zeichnen, "fill", "shift+c", "action.draw.fill.paint.hint", true),
    act!("draw.text", "action.draw.text.label", Zeichnen, "text", "t", "action.draw.text.hint", true),
    act!("draw.ellipse", "action.draw.ellipse.label", Zeichnen, "ellipse", "", "action.draw.ellipse.hint", true),
    // Bild platzieren: zwei gegenüberliegende Ecken bestimmen Größe und Lage.
    // Ohne eingebettete Daten (`data`) liest die Action die Datei aus `file` —
    // so ist sie auch über MCP benutzbar.
    act!("draw.image", "action.draw.image.label", Zeichnen, "image", "", "action.draw.image.hint", true),

    // Architektur
    act!("draw.wall", "action.draw.wall.label", Architektur, "wall", "g", "action.draw.wall.hint", true),
    // V gehört dem Werkzeug „Verschieben" (modify.move, siehe BEDIENUNG);
    // Volumen liegt deshalb auf Umschalt+V (T0056).
    act!(
        "draw.volume",
        "action.draw.volume.label",
        Architektur,
        "volume",
        "shift+v",
        "action.draw.volume.hint",
        true
    ),
    // Kein eigenes Kürzel: `shift+v` gehört `draw.volume` (T0079). Eine Taste
    // darf nur eine Funktion tragen — vorher stand die Punktfolge auf derselben
    // Taste und war über sie nie erreichbar.
    act!(
        "draw.volume.points",
        "action.draw.volume.points.label",
        Architektur,
        "volume",
        "",
        "action.draw.volume.points.hint",
        true
    ),
    act!(
        "draw.volume.paint",
        "action.draw.volume.paint.label",
        Architektur,
        "volume",
        "",
        "action.draw.volume.paint.hint",
        true
    ),
    act!("draw.door", "action.draw.door.label", Architektur, "door", "h", "action.draw.door.hint", true),
    act!("draw.window", "action.draw.window.label", Architektur, "window", "j", "action.draw.window.hint", true),
    act!(
        "draw.stair",
        "action.draw.stair.label",
        Architektur,
        "stair",
        "b",
        "action.draw.stair.hint",
        true
    ),
    act!(
        "draw.stair.label",
        "action.draw.stair.label.label",
        Architektur,
        "text",
        "",
        "action.draw.stair.label.hint",
        false
    ),

    // Ändern
    // Ohne Kürzel: ESC ist eine Steuerungstaste (bricht ab bzw. schließt eine
    // Punktfolge ab, `App::escape_pressed`) und wird nicht vergeben (T0045).
    act!("modify.select", "action.modify.select.label", Aendern, "select", "", "action.modify.select.hint", false),
    act!("modify.move", "action.modify.move.label", Aendern, "move", "v", "action.modify.move.hint", true),
    act!("modify.order", "action.modify.order.label", Aendern, "layers", "", "action.modify.order.hint", true),
    // Kopie versetzt absetzen: erst Basispunkt, dann Zielpunkt. Getrennt vom
    // reinen Kopieren in die Zwischenablage (`edit.copy`, Strg+C).
    act!("modify.copy", "action.modify.copy.label", Aendern, "copy-base", "ctrl+shift+c", "action.modify.copy.hint", true),
    act!("modify.rotate", "action.modify.rotate.label", Aendern, "rotate", "r", "action.modify.rotate.hint", true),
    act!("modify.mirror", "action.modify.mirror.label", Aendern, "mirror", "f", "action.modify.mirror.hint", true),
    act!("modify.scale", "action.modify.scale.label", Aendern, "scale", "", "action.modify.scale.hint", true),
    act!("modify.offset", "action.modify.offset.label", Aendern, "offset", "", "action.modify.offset.hint", true),
    act!("modify.trim", "action.modify.trim.label", Aendern, "trim", "c", "action.modify.trim.hint", true),
    act!("modify.extend", "action.modify.extend.label", Aendern, "extend", "x", "action.modify.extend.hint", true),
    act!(
        "modify.intersect",
        "action.modify.intersect.label",
        Aendern,
        "intersect",
        "",
        "action.modify.intersect.hint",
        true
    ),
    act!(
        "modify.area.union",
        "action.modify.area.union.label",
        Aendern,
        "union",
        "",
        "action.modify.area.union.hint",
        false
    ),
    act!(
        "modify.area.split",
        "action.modify.area.split.label",
        Aendern,
        "split",
        "",
        "action.modify.area.split.hint",
        true
    ),
    act!("modify.erase", "action.modify.erase.label", Aendern, "erase", "entf", "action.modify.erase.hint", false),
    act!("modify.explode", "action.modify.explode.label", Aendern, "explode", "z", "action.modify.explode.hint", false),
    // Verbinden (T0229): ausgewählte Linien/Bögen/Splines zu einem Pfad.
    act!("path.connect", "action.path.connect.label", Aendern, "polyline", "", "action.path.connect.hint", false),
    act!(
        "text.format",
        "action.text.format.label",
        Aendern,
        "text",
        "",
        "action.text.format.hint",
        false
    ),
    // Schnellauswahl (T0060): ein Objekt anklicken und das Werkzeug starten,
    // das ein weiteres Objekt derselben Art erzeugt. Ersetzt „Eigenschaften
    // aufnehmen" vollständig — ein Schalter, kein zweiter Weg (`App::run_action`).
    act!("modify.quickselect", "action.modify.quickselect.label", Aendern, "quickselect", "q", "action.modify.quickselect.hint", true),

    // Bemaßung
    // Ein Element für Einzelmaß und Maßkette (T0154). `dim.linear` bleibt als
    // Befehl für MCP und ältere Skripte bestehen, ist aber kein eigenes
    // Werkzeug mehr — das Ribbon zeigt nur die Maßkette mit ihren Modi.
    // `dim.linear` und `dim.chain` sind Befehle (MCP, ältere Skripte) — die
    // Werkzeuge sind die drei Eingabemodi der Maßkette (T0154, Turn 3).
    act!("dim.linear", "action.dim.linear.label", Bemassung, "dim", "", "action.dim.linear.hint", false),
    act!("dim.chain", "action.dim.chain.label", Bemassung, "dimchain", "", "action.dim.chain.hint", false),
    act!("dim.chain.h", "action.dim.chain.h.label", Bemassung, "dimchain", "m", "action.dim.chain.h.hint", true),
    act!("dim.chain.v", "action.dim.chain.v.label", Bemassung, "dim-v", "", "action.dim.chain.v.hint", true),
    act!("dim.chain.d", "action.dim.chain.d.label", Bemassung, "dim-d", "", "action.dim.chain.d.hint", true),
    // Messpunkt an eine bestehende Maßkette anhängen bzw. herausnehmen
    // (T0154) — beides im Reiter „Bemaßung".
    act!("dim.point.add", "action.dim.point.add.label", Bemassung, "point", "", "action.dim.point.add.hint", true),
    act!("dim.point.remove", "action.dim.point.remove.label", Bemassung, "erase", "", "action.dim.point.remove.hint", true),
    act!("dim.elevation.plan", "action.dim.elevation.plan.label", Bemassung, "elevation", "n", "action.dim.elevation.plan.hint", true),
    act!("dim.elevation.section", "action.dim.elevation.section.label", Bemassung, "elevation-section", "", "action.dim.elevation.section.hint", true),
    act!(
        "dim.room",
        "action.dim.room.label",
        Bemassung,
        "room",
        "",
        "action.dim.room.hint",
        true
    ),
    act!("dim.style", "action.dim.style.label", Bemassung, "style", "", "action.dim.style.hint", false),

    // Blöcke
    act!("block.insert", "action.block.insert.label", Bloecke, "block", "", "action.block.insert.hint", true),
    act!("block.create", "action.block.create.label", Bloecke, "blocknew", "", "action.block.create.hint", false),
    act!("block.explode", "action.block.explode.label", Bloecke, "explode", "", "action.block.explode.hint", false),
    act!("block.manager", "action.block.manager.label", Bloecke, "library", "", "action.block.manager.hint", false),

    // Blatt
    act!("sheet.new", "action.sheet.new.label", Blatt, "sheet", "", "action.sheet.new.hint", false),
    act!("sheet.pdf", "action.sheet.pdf.label", Blatt, "pdf", "", "action.sheet.pdf.hint", false),
    act!("sheet.add_viewport", "action.sheet.add_viewport.label", Blatt, "viewport", "", "action.sheet.add_viewport.hint", false),
    act!("sheet.viewport_tool", "action.sheet.viewport_tool.label", Blatt, "viewport", "", "action.sheet.viewport_tool.hint", true),

    // Ansicht
    // T0159: „Alles zeigen“ liegt auf der festen Taste F5 — ausgewertet in
    // `App::handle_shortcuts`, der Eintrag hier macht sie in Tastaturbelegung
    // und Inventar als feste Belegung sichtbar (wie F1 und F11).
    act!("view.zoom_extents", "action.view.zoom_extents.label", Ansicht, "zoomfit", "f5", "action.view.zoom_extents.hint", false),
    act!("view.zoom_in", "action.view.zoom_in.label", Ansicht, "zoomin", "", "action.view.zoom_in.hint", false),
    act!("view.zoom_out", "action.view.zoom_out.label", Ansicht, "zoomout", "", "action.view.zoom_out.hint", false),
    // Umsetzen der Ansicht mit der Tastatur (T0056). Die vier Richtungen sind
    // eigene Funktionen — nicht fest im Code — damit sie in der
    // Tastaturbelegung stehen, ein Icon tragen und umbelegt werden können.
    // Wie jede andere Funktion: ein Druck rückt um einen Schritt weiter, aus
    // Taste, Hotbar und Ribbon gleich (T0089).
    act!("view.pan_left", "action.view.pan_left.label", Ansicht, "pan-left", "a", "action.view.pan_left.hint", false),
    act!("view.pan_right", "action.view.pan_right.label", Ansicht, "pan-right", "d", "action.view.pan_right.hint", false),
    act!("view.pan_up", "action.view.pan_up.label", Ansicht, "pan-up", "w", "action.view.pan_up.hint", false),
    act!("view.pan_down", "action.view.pan_down.label", Ansicht, "pan-down", "s", "action.view.pan_down.hint", false),
    act!("view.inventory", "action.view.inventory.label", Ansicht, "inventory", "e", "action.view.inventory.hint", false),
    act!("view.layers", "action.view.layers.label", Ansicht, "layers", "", "action.view.layers.hint", false),
    act!("view.keymap", "action.view.keymap.label", Ansicht, "keyboard", "", "action.view.keymap.hint", false),
    act!("view.settings", "action.view.settings.label", Ansicht, "settings", "", "action.view.settings.hint", false),
    act!("view.hotbar_next", "action.view.hotbar_next.label", Ansicht, "hotbar", "y", "action.view.hotbar_next.hint", false),
    act!("view.help", "action.view.help.label", Hilfe, "help", "f1", "action.view.help.hint", false),
    act!("view.fullscreen", "action.view.fullscreen.label", Ansicht, "fullscreen", "f11", "action.view.fullscreen.hint", false),
    act!("view.rotate_tool", "action.view.rotate_tool.label", Ansicht, "rotate", "", "action.view.rotate_tool.hint", true),
    act!("edit.repeat", "action.edit.repeat.label", Aendern, "repeat", " ", "action.edit.repeat.hint", false),

    // Interne/fortgeschrittene Aktionen (MCP + Eigenschaften-Panel)
    act!("modify.set", "action.modify.set.label", Aendern, "properties", "", "action.modify.set.hint", false),
    // Pläne (T0202): die oberste Gliederungsebene des Layer-Managers.
    act!("plan.create", "action.plan.create.label", Ansicht, "layers", "", "action.plan.create.hint", false),
    act!("plan.update", "action.plan.update.label", Ansicht, "layers", "", "action.plan.update.hint", false),
    act!("plan.list", "action.plan.list.label", Ansicht, "layers", "", "action.plan.list.hint", false),
    act!("layer.create", "action.layer.create.label", Ansicht, "layers", "", "action.layer.create.hint", false),
    act!("layer.update", "action.layer.update.label", Ansicht, "layers", "", "action.layer.update.hint", false),
    act!("layer.delete", "action.layer.delete.label", Ansicht, "layers", "", "action.layer.delete.hint", false),
    act!("layer.list", "action.layer.list.label", Ansicht, "layers", "", "action.layer.list.hint", false),
    act!("layer.set_current", "action.layer.set_current.label", Ansicht, "layers", "", "action.layer.set_current.hint", false),
    act!("select.all", "action.select.all.label", Aendern, "select", "ctrl+a", "action.select.all.hint", false),
    act!("select.query", "action.select.query.label", Aendern, "select", "", "action.select.query.hint", false),
    act!("select.area", "action.select.area.label", Aendern, "select", "", "action.select.area.hint", false),
    act!("block.update", "action.block.update.label", Bloecke, "blocknew", "", "action.block.update.hint", false),
    act!("block.delete", "action.block.delete.label", Bloecke, "library", "", "action.block.delete.hint", false),
    // T0217: Kopie eines Blocks — auch eines schreibgeschützten Systemblocks.
    act!("block.copy", "action.block.copy.label", Bloecke, "library", "", "action.block.copy.hint", false),
    act!("block.list", "action.block.list.label", Bloecke, "library", "", "action.block.list.hint", false),
    act!("sheet.update", "action.sheet.update.label", Blatt, "sheet", "", "action.sheet.update.hint", false),
    act!("sheet.delete", "action.sheet.delete.label", Blatt, "sheet", "", "action.sheet.delete.hint", false),
    act!("sheet.update_viewport", "action.sheet.update_viewport.label", Blatt, "viewport", "", "action.sheet.update_viewport.hint", false),
    act!("sheet.remove_viewport", "action.sheet.remove_viewport.label", Blatt, "viewport", "", "action.sheet.remove_viewport.hint", false),
    act!("sheet.fit_viewport", "action.sheet.fit_viewport.label", Blatt, "viewport", "", "action.sheet.fit_viewport.hint", false),
    act!("doc.settings", "action.doc.settings.label", Ansicht, "settings", "", "action.doc.settings.hint", false),
    act!("doc.project", "action.doc.project.label", Datei, "info", "", "action.doc.project.hint", false),
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
            | "draw.parallel"
            | "draw.spline"
            | "draw.path"
            | "path.connect"
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
            | "draw.image"
            | "draw.wall"
            | "draw.volume"
            | "draw.volume.points"
            | "draw.volume.paint"
            | "draw.door"
            | "draw.window"
            | "draw.stair"
            | "dim.linear"
            | "dim.chain"
            | "dim.chain.h"
            | "dim.chain.v"
            | "dim.chain.d"
            | "dim.elevation.plan"
            | "dim.elevation.section"
            | "dim.room"
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
            | "modify.area.union"
            | "modify.area.split"
            | "text.format"
            | "modify.set"
            | "plan.create"
            | "plan.update"
            | "layer.create"
            | "layer.update"
            | "layer.delete"
            | "block.create"
            | "block.insert"
            | "block.update"
            | "block.delete"
            | "block.copy"
            | "file.import.pdf"
            | "sheet.new"
            | "sheet.update"
            | "sheet.delete"
            | "sheet.add_viewport"
            | "sheet.update_viewport"
            | "sheet.fit_viewport"
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

    /// Punkt in mm: [x,y] oder {"x":..,"y":..} — in der Einheit `units`
    /// (Vorgabe: Dokumenteinheit).
    pub fn pt(&self, k: &str) -> Option<Vec2> {
        self.pt_in(k, self.unit)
    }

    /// Punkt in **Papier-Millimetern**, unabhängig von `units`.
    ///
    /// Für Werte, die schon Millimeter sind (Rechteck und Mittelpunkt eines
    /// Ansichtsfensters): sonst würde `center: [5618, 7076]` in einem Dokument
    /// mit `units: "m"` als 5618 m gedeutet und das Blatt bliebe leer (T0037).
    pub fn pt_mm(&self, k: &str) -> Option<Vec2> {
        self.pt_in(k, Unit::Mm)
    }

    fn pt_in(&self, k: &str, unit: Unit) -> Option<Vec2> {
        let val = self.raw(k)?;
        if let Some(arr) = val.as_array() {
            if arr.len() >= 2 {
                let x = arr[0].as_f64()?;
                let y = arr[1].as_f64()?;
                return Some(v(x * unit.mm(), y * unit.mm()));
            }
        }
        if let Value::Object(o) = val {
            let x = o.get("x")?.as_f64()?;
            let y = o.get("y")?.as_f64()?;
            return Some(v(x * unit.mm(), y * unit.mm()));
        }
        if let Some(s) = val.as_str() {
            let parts: Vec<&str> = s.split([',', ';']).collect();
            if parts.len() == 2 {
                let x = parse_length(parts[0], unit)?;
                let y = parse_length(parts[1], unit)?;
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
        LineType::from_key(&self.text(k)?)
    }

    /// Dämmung: Bandhöhe H (Länge, wird in mm umgerechnet) und Schlaufenform r
    /// (Verhältnis, keine Länge). Beide bleiben bei `None` unverändert.
    pub fn loop_params(&self) -> (Option<f64>, Option<f64>) {
        let breite = self.len("loop_width").filter(|v| *v > 0.0);
        let form = self.num("loop_shape").filter(|v| *v > 0.0);
        (breite, form)
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

/// Typgerechte Standardebene einer Zeichenaktion (T0039).
///
/// Ohne `layer` legt jede Zeichenaktion ihr Objekt auf die Ebene ihres Typs —
/// ein Plan aus einem MCP-Batch bekommt damit seine Ebenenstruktur, ohne dass
/// jeder einzelne Schritt `layer` mitschicken muss. Ein ausdrücklich
/// übergebenes `layer` sticht die Vorgabe immer (siehe [`resolve_layer_in`]).
///
/// Seit T0202 gibt es nur noch die vier Layer eines Plans
/// ([`crate::style::LAYER_DEFAULTS`]):
///
/// - **Zeichnung** — freie Geometrie und Flächen (Linien, Kreise, Bögen,
///   Rechtecke, Splines, Füllflächen, Schraffuren, Bilder),
/// - **Architektur** — Wände, Türen, Fenster, Treppen, Volumen,
/// - **Beschriftung** — Texte, Bemaßung, Höhenkoten, Raumstempel,
/// - **Blöcke** — eingesetzte Blockinstanzen.
pub fn standard_ebene(action: &str) -> Option<&'static str> {
    Some(match action {
        "draw.wall" | "draw.door" | "draw.window" | "draw.stair" | "draw.stair.label" => "Architektur",
        // Ein Volumen ist ein Körper im Wandton (Füllung aus der Ebene) — es
        // gehört deshalb zur Architektur, nicht zur Zeichnung.
        "draw.volume" | "draw.volume.points" | "draw.volume.paint" => "Architektur",
        "dim.linear"
        | "dim.chain"
        | "dim.chain.h"
        | "dim.chain.v"
        | "dim.chain.d"
        | "dim.elevation.plan"
        | "dim.elevation.section"
        | "dim.room"
        | "draw.text" => "Beschriftung",
        "block.insert" => "Blöcke",
        // Alles, was Fläche oder Linie ist, gehört in die Zeichnung.
        "draw.image"
        | "draw.fill"
        | "draw.fill.points"
        | "draw.fill.boundary"
        | "draw.fill.paint"
        | "draw.hatch"
        | "draw.point"
        | "draw.line"
        | "draw.polyline"
        | "draw.spline"
        | "draw.path"
        | "draw.rect"
        | "draw.circle"
        | "draw.arc"
        | "draw.ellipse" => "Zeichnung",
        _ => return None,
    })
}

/// Das Erzeugerwerkzeug zu einer Objektart — Grundlage der **Schnellauswahl**
/// (T0060).
///
/// Die Schnellauswahl klickt ein vorhandenes Objekt an und startet das
/// Werkzeug, das ein weiteres Objekt derselben Art erzeugt. Die Zuordnung steht
/// hier bei den Aktionen und nicht in der Oberfläche: sie gehört zur
/// Befehlsschicht und ist ohne GUI prüfbar. `match` ist erschöpfend — eine neue
/// Objektart ohne Erzeuger fällt sofort beim Bauen auf, statt still nichts zu
/// tun.
pub fn erzeuger_fuer(kind: &EntityKind) -> &'static str {
    match kind {
        EntityKind::Point { .. } => "draw.point",
        EntityKind::Line { .. } => "draw.line",
        EntityKind::Circle { .. } => "draw.circle",
        EntityKind::Arc { .. } => "draw.arc",
        EntityKind::Ellipse { .. } => "draw.ellipse",
        EntityKind::EllipseArc { .. } => "draw.ellipse",
        EntityKind::Polyline { .. } => "draw.polyline",
        EntityKind::Spline { .. } => "draw.spline",
        EntityKind::Path { .. } => "draw.path",
        EntityKind::Fill { .. } => "draw.fill",
        EntityKind::Volume { .. } => "draw.volume",
        EntityKind::Text { .. } => "draw.text",
        // Ein Element für Einzelmaß und Maßkette (T0154). Die Schnellauswahl
        // nimmt den Modus, der zur Achse des angeklickten Maßes passt
        // (`canvas` wertet die Achse aus) — hier steht der Rückfall.
        EntityKind::Dim { .. } => "dim.chain.d",
        EntityKind::ElevationPlan { .. } => "dim.elevation.plan",
        EntityKind::ElevationSection { .. } => "dim.elevation.section",
        EntityKind::Room { .. } => "dim.room",
        EntityKind::Wall { .. } => "draw.wall",
        EntityKind::Door { .. } => "draw.door",
        EntityKind::Window { .. } => "draw.window",
        EntityKind::Stair { .. } => "draw.stair",
        EntityKind::BlockRef { .. } => "block.insert",
        EntityKind::Image { .. } => "draw.image",
        EntityKind::Viewport(_) => "sheet.viewport_tool",
    }
}

/// Layer aus Parameter `layer` (Name oder ID), sonst die Standardebene der
/// Aktion (`standard`), sonst der aktuelle Layer.
///
/// Ein ausdrücklich übergebenes `layer` gewinnt immer — auch als Name, der
/// noch nicht existiert (dann wird die Ebene angelegt).
fn resolve_layer_in(doc: &mut Document, p: &Params, standard: Option<&str>) -> Id {
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
            if !name.trim().is_empty() {
                return doc.ensure_layer(name, Color::BLACK);
            }
        }
    }
    if let Some(name) = standard {
        // T0202: Jeder Plan hat seine eigenen vier Layer — die Standardebene
        // wird deshalb im Plan des aktuellen Layers gesucht (und dort bei
        // Bedarf angelegt), nicht irgendwo in der Zeichnung.
        return doc.ensure_layer(name, Color::BLACK);
    }
    doc.current_layer
}

/// Layer ohne typgerechte Vorgabe (Änderungs- und Import-Aktionen).
fn resolve_layer(doc: &mut Document, p: &Params) -> Id {
    resolve_layer_in(doc, p, None)
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

/// Meldung, wenn eine Tür keinen Wandbezug hat — sie braucht die vordere
/// Wandkante als Aufstandsfläche (T0038).
pub const TUER_OHNE_WAND: &str =
    "Tür · keine Wand an dieser Stelle — an eine Wand setzen oder 'host' angeben";

/// Meldung, wenn ein Fenster keinen Wandbezug hat.
pub const FENSTER_OHNE_WAND: &str =
    "Fenster · keine Wand an dieser Stelle — an eine Wand setzen oder 'host' angeben";

/// Meldung, wenn eine Öffnung breiter ist als die freie Wandstrecke.
pub const TUER_ZU_BREIT: &str = "Tür · passt nicht in die Wand";
pub const FENSTER_ZU_BREIT: &str = "Fenster · passt nicht in die Wand";

/// Zulässiger Längsbereich einer Öffnung in ihrer Wand (mm ab Wandanfang).
///
/// Die Wandstrecke kommt aus [`crate::wall::wall_span`] — an verbundenen Ecken
/// begrenzen die Gehrungspunkte der Umrandung die Wand, sonst die Stirn. Um
/// die halbe Öffnungsbreite eingerückt, damit die Öffnung **ganz** in der Wand
/// liegt. `None` = keine Wand oder die Öffnung ist breiter als die Strecke.
pub fn opening_span(doc: &Document, host: Option<Id>, width: f64) -> Option<(f64, f64)> {
    let (t0, t1) = crate::wall::wall_span(doc, host?)?;
    let hw = (width * 0.5).max(0.0);
    if t1 - t0 + 1e-6 < width {
        return None;
    }
    Some((t0 + hw, t1 - hw))
}

/// Längslage einer Öffnung in die freie Wandstrecke ziehen.
///
/// Passt die Öffnung nicht (Wand zu kurz), bleibt die Lage stehen — dann
/// entscheidet der Aufrufer, ob das ein Fehler ist (Platzieren) oder die
/// vorhandene Geometrie gilt.
pub fn clamp_opening_at(doc: &Document, host: Option<Id>, width: f64, at: f64) -> f64 {
    match opening_span(doc, host, width) {
        Some((lo, hi)) => at.clamp(lo, hi),
        None => at,
    }
}

/// Punkt auf der Wandachse in Längslage `at` (mm ab Wandanfang).
fn wall_axis_point(doc: &Document, host: Id, at: f64) -> Option<Vec2> {
    let EntityKind::Wall { a, b, .. } = &doc.entity(host)?.kind else {
        return None;
    };
    let dir = (*b - *a).normalized();
    Some(*a + dir * at)
}

/// Auf die freie Wandstrecke gezogener Anschlusspunkt einer Öffnung.
///
/// Dieselbe Begrenzung wie beim Platzieren: der Punkt wird auf die Wandachse
/// gelegt und längs in den Bereich zwischen den Anschlussnähten gezogen
/// (T0078). Vorschau und Action teilen sie, damit der Klick genau das setzt,
/// was zu sehen war.
pub fn clamp_opening_center(
    doc: &Document,
    host: Option<Id>,
    center: Vec2,
    width: f64,
) -> Vec2 {
    let Some(hid) = host else {
        return center;
    };
    let Some(t) = wall_at(doc, host, center) else {
        return center;
    };
    let t = clamp_opening_at(doc, host, width, t);
    wall_axis_point(doc, hid, t).unwrap_or(center)
}

/// Längslage des Türzentrums aus dem Basispunkt (T0091).
///
/// Der Klickpunkt ist der Basispunkt der Tür — Links/Mitte/Rechts längs der
/// Wand. Das Zentrum liegt um den Versatz daneben und wird auf die freie
/// Wandstrecke begrenzt. Action `draw.door` und Vorschau teilen die Rechnung,
/// sonst springt die Tür beim Klick.
pub fn door_at_from_base(
    doc: &Document,
    host: Option<Id>,
    base_pt: Vec2,
    width: f64,
    base: crate::model::DoorBase,
) -> Option<f64> {
    let t = wall_at(doc, host, base_pt)? - base.offset() * width;
    Some(clamp_opening_at(doc, host, width, t))
}

/// Türzentrum aus dem Basispunkt (T0091) — dieselbe Rechnung wie
/// [`door_at_from_base`], nur als Punkt auf der Wandachse.
pub fn door_center_from_base(
    doc: &Document,
    host: Option<Id>,
    base_pt: Vec2,
    width: f64,
    base: crate::model::DoorBase,
) -> Option<Vec2> {
    let hid = host?;
    let t = door_at_from_base(doc, host, base_pt, width, base)?;
    wall_axis_point(doc, hid, t)
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
        if let EntityKind::Door {
            host,
            center,
            width,
            ..
        }
        | EntityKind::Window {
            host,
            center,
            width,
            ..
        } = &e.kind
        {
            if host.is_some() {
                if let Some(t) = wall_at(&*doc, *host, *center) {
                    // Verschieben über das Wandende hinaus endet an der Naht —
                    // die Öffnung liegt immer in Wand (T0078).
                    fix.push((e.id, clamp_opening_at(&*doc, *host, *width, t)));
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

/// Kopiert die Objekte unter der Abbildung `m` — **mit** den Türen und Fenstern
/// ihrer Wände (T0207).
///
/// Zwei Fehler waren hier drin: die Öffnungen wurden beim Kopieren gar nicht
/// mitgenommen (die Kopie war eine nackte Wand), und ihre `host`-Kennung zeigte
/// danach auf das **Original**. Diese eine Stelle legt die Kopien an und setzt
/// die Verweise auf die neuen Wand-Ids um — für Verschieben-Kopieren, Drehen
/// und Spiegeln gleich.
///
/// Zurück kommen die neuen Ids (in der Reihenfolge der Eingabe) und die
/// Zuordnung alt → neu.
pub(crate) fn kopiere_mit_oeffnungen(
    doc: &mut Document,
    ids: &[Id],
    m: &Affine,
) -> (Vec<Id>, Vec<(Id, Id)>) {
    let mut zuordnung: Vec<(Id, Id)> = Vec::new();
    let mut created = Vec::new();
    for eid in ids {
        let Some(e) = doc.entity(*eid).cloned() else {
            continue;
        };
        let mut ne = transform::transform_entity(&e, m);
        ne.id = doc.new_id();
        zuordnung.push((*eid, ne.id));
        created.push(ne.id);
        // Kopien sind neue Elemente und ordnen sich nach ihrem Zeichenrang ein
        // (T0048).
        doc.insert_ranked(ne);
    }
    for (_, neu) in &zuordnung {
        let Some(em) = doc.entity_mut(*neu) else {
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
    (created, zuordnung)
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

/// Türen und Fenster nach einem **freien** Manipulieren wieder auf ihre Wand
/// setzen (T0191).
///
/// Wird ein Objekt unmittelbar geschrieben statt über die Action-Schicht —
/// Verschieben mit der Maus, Einfügen aus der Zwischenablage —, ändert sich
/// nur der Mittelpunkt: die Längslage `at` bleibt alt und der Querabstand zur
/// Wand ist beliebig. Eine Tür stand danach neben der Wand, und ihr Basispunkt
/// wanderte von der Wandkante weg. Diese eine Funktion zieht beides nach: erst
/// die Längslage aus dem Mittelpunkt neu bestimmen und auf die freie
/// Wandstrecke begrenzen ([`resync_openings`]), dann folgt der Mittelpunkt aus
/// der Wand ([`reflow_openings`]) — Türen sitzen wieder auf der vorderen
/// Wandkante (Blattseite), Fenster im Wandkörper.
pub fn oeffnungen_auf_wand(doc: &mut Document) {
    resync_openings(doc);
    reflow_openings(doc);
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
    let (breite, form) = p.loop_params();
    if let Some(b) = breite {
        e.style.loop_width = Some(b);
    }
    if let Some(f) = form {
        e.style.loop_shape = Some(f);
    }
    if let Some(c) = p.color("fill") {
        e.style.fill = Some(c);
    }
    let _ = doc;
}

/// Neue Bemaßung ist **schwarz** (T0062).
///
/// Die Ebene „Bemaßung“ trägt als Vorgabe ein Rot, jede neue Maßkette erbte
/// diese Farbe über „vom Layer“ — gewünscht ist Schwarz. Ein ausdrücklich
/// mitgegebenes `color` sticht weiterhin (die Oberfläche reicht die Wahl aus
/// dem Eigenschaften-Panel durch). Bis das Ebenen-System überarbeitet wird,
/// ist das die einfache Regel; danach darf sie wieder verschwinden.
fn bemasung_schwarz(p: &Params, e: &mut Entity) {
    // Ein ausdrückliches `color` (auch "bylayer") bleibt unangetastet.
    if p.v.get("color").is_none() && e.style.color.is_none() {
        e.style.color = Some(Color::BLACK);
    }
}

fn ids_from(p: &Params, key: &str) -> Vec<Id> {
    p.ids(key).unwrap_or_default()
}

/// Stelle, an der ein neuer Messpunkt die Kette **geordnet** hält (T0154).
///
/// Gezählt wird entlang der Messrichtung: der Punkt gehört vor den ersten
/// Punkt, der weiter vorn liegt. So bleibt die Kette monoton und ihre
/// Maßzahlen positiv — egal, in welcher Reihenfolge der Nutzer klickt.
fn einfuege_index(pts: &[Vec2], angle: f64, q: Vec2) -> usize {
    if pts.is_empty() {
        return 0;
    }
    let (dir, _) = crate::render::dim_achse(angle);
    let t = (q - pts[0]).dot(dir);
    pts.iter()
        .position(|p| (*p - pts[0]).dot(dir) > t)
        .unwrap_or(pts.len())
}

/// Benutzerbibliothek aus dem Aufruf (`dir`, die Oberfläche übergibt
/// `prefs.blocks_dir`). Ohne Angabe bleibt das Verzeichnis unberührt — Tests
/// und Aufrufe ohne Datenwurzel schreiben nichts in das Benutzerverzeichnis.
/// Frühere Angaben zur Blockgröße in die heutige übertragen (T0231).
///
/// Turn 2 nannte die Eigenschaft `einheit` (`neutral`/`mm`/`cm`/`m`), davor gab
/// es das Häkchen `annotative`. „Neutral“ (und `annotative: true`) ist heute die
/// **Papiergröße**, jede echte Einheit die **feste Größe**.
fn alt_einheit_zu_groesse(s: &str) -> BlockGroesse {
    match s.trim().to_ascii_lowercase().as_str() {
        "neutral" | "papier" | "papiergroesse" => BlockGroesse::Papier,
        _ => BlockGroesse::Fest,
    }
}

fn bloecke_verzeichnis(p: &Params) -> Option<std::path::PathBuf> {
    let angabe = p.text("dir")?;
    let angabe = angabe.trim();
    if angabe.is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(angabe))
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
    ebene: Option<&str>,
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
    add_fill_rings(doc, boundary, holes, p, rand_vorgabe, ebene)
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
    ebene: Option<&str>,
) -> Result<Value, String> {
    add_filled(doc, boundary, holes, p, rand_vorgabe, false, ebene)
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
    volumen: bool,
    ebene: Option<&str>,
) -> Result<Value, String> {
    if boundary.len() < 3 {
        // Sonderfall „Diagonale" (T0131): zwei Punkte ergeben ein
        // achsenparalleles Rechteck. Liegen beide auf einer Waagerechten oder
        // Senkrechten, ist das keine Fläche — dann bleibt es beim Fehler, kein
        // stiller Erfolg (T0038).
        if boundary.len() == 2 {
            if let Some(ecken) = crate::geom::rechteck_aus_diagonale(boundary[0], boundary[1]) {
                return add_filled(
                    doc,
                    ecken.to_vec(),
                    holes,
                    p,
                    rand_vorgabe,
                    volumen,
                    ebene,
                );
            }
            return Err("Zwei Punkte auf einer Achse ergeben keine Fläche".into());
        }
        return Err("Füllfläche braucht mindestens drei Punkte".into());
    }
    let layer = resolve_layer_in(doc, p, ebene);
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
    // Zeichenreihenfolge: der Platz kommt aus dem Zeichenrang (T0048). Eine
    // Füllfläche steht ganz hinten — ihre Umgrenzung (Wände, Linien) bleibt
    // dadurch sichtbar, auch wenn sie über einer schon vorhandenen Fläche
    // liegt. `idn` bleibt die vorab vergebene Id.
    let (_, ziel) = doc.insert_ranked(e);
    Ok(json!({"ok": true, "created": [idn], "position": ziel + 1}))
}

// ---------------------------------------------------------------------------
// Ansichtsfenster: eine Quelle für Einheiten und Darstellung
// ---------------------------------------------------------------------------
//
// Ein Ansichtsfenster mischt zwei Räume, deshalb steht die Einheit **im Namen**
// des Feldes: `x`/`y`/`w`/`h` bzw. `rect_mm` sind Papier-Millimeter, `center`
// ist eine Modellkoordinate in der Einheit `units` (Vorgabe: Dokumenteinheit)
// und `center_mm` dieselbe Lage in Millimetern. Gelesen wird genauso
// (`viewport_info`), damit Hin- und Rückweg ohne Kopfrechnen zusammenpassen.

/// Ansichtsfenster-Objekt eines Blatts (T0109).
///
/// Seit T0109 ist ein Ansichtsfenster ein normales Blattobjekt
/// (`EntityKind::Viewport`) — die Aktionen suchen es deshalb in
/// `sheet.entities`, nicht mehr in einer eigenen Liste.
fn viewport_mut(sheet: &mut Sheet, id: Id) -> Option<&mut Viewport> {
    sheet.entities.iter_mut().find_map(|e| {
        if e.id != id {
            return None;
        }
        match &mut e.kind {
            EntityKind::Viewport(vp) => Some(vp),
            _ => None,
        }
    })
}

/// Erstes Ansichtsfenster eines Blatts (für Aktionen ohne `viewport`-Angabe).
fn erster_viewport_mut(sheet: &mut Sheet) -> Option<&mut Viewport> {
    sheet.entities.iter_mut().find_map(|e| match &mut e.kind {
        EntityKind::Viewport(vp) => Some(vp),
        _ => None,
    })
}

/// Rechteck eines Ansichtsfensters übernehmen (`rect_mm` oder `x`/`y`/`w`/`h`).
fn viewport_rect(p: &Params, vp: &mut Viewport) -> Result<(), String> {
    if p.has("rect_mm") {
        let arr = p
            .raw("rect_mm")
            .and_then(|v| v.as_array().cloned())
            .ok_or_else(|| "rect_mm muss [x, y, b, h] sein".to_string())?;
        if arr.len() < 4 {
            return Err("rect_mm braucht vier Papier-Millimeter: [x, y, b, h]".into());
        }
        let n: Vec<f64> = arr[..4].iter().filter_map(|v| v.as_f64()).collect();
        if n.len() < 4 {
            return Err("rect_mm braucht vier Zahlen: [x, y, b, h]".into());
        }
        vp.x = n[0];
        vp.y = n[1];
        vp.w = n[2];
        vp.h = n[3];
    }
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
    Ok(())
}

/// Mittelpunkt eines Ansichtsfensters übernehmen.
///
/// Beide Schreibweisen gleichzeitig ist ein Fehler: sonst müsste man raten,
/// welche der beiden Zahlen gilt.
fn viewport_center(p: &Params, vp: &mut Viewport) -> Result<(), String> {
    let in_einheit = p.pt("center");
    let in_mm = p.pt_mm("center_mm");
    match (in_einheit, in_mm) {
        (Some(_), Some(_)) => Err("center und center_mm nicht gleichzeitig angeben".into()),
        (_, Some(mm)) => {
            vp.center = mm;
            Ok(())
        }
        (Some(c), None) => {
            vp.center = c;
            Ok(())
        }
        (None, None) => Ok(()),
    }
}

/// Ansichtsfenster als JSON — eine Quelle für `sheet list` (MCP) und für die
/// Antwort der Ansichts-Aktionen.
///
/// `rect_mm` sind Papier-Millimeter, `center` steht in der Dokumenteinheit
/// (`units`), `center_mm` zusätzlich in Millimetern. Beide `center`-Felder
/// lassen sich unverändert zurückschreiben.
pub fn viewport_info(einheit: Unit, vp: &Viewport) -> Value {
    let u = einheit;
    json!({
        "id": vp.id,
        "name": vp.name,
        "rect_mm": [vp.x, vp.y, vp.w, vp.h],
        "scale": vp.scale,
        "center": [u.from_mm(vp.center.x), u.from_mm(vp.center.y)],
        "center_mm": vp.center.to_array(),
        "units": u.short(),
        "show_frame": vp.show_frame,
    })
}

/// Maßstab und Mitte so wählen, dass die Zeichnung vollständig ins
/// Ansichtsfenster passt.
///
/// Rückgabe `(scale, center, warnung)`. Der Maßstab kommt aus der Liste
/// gängiger Maßstäbe (`pdf::nice_scale`), damit 1:50 herauskommt und nicht
/// 1:47,3; `center` ist der Mittelpunkt der Zeichnungs-BBox.
fn fit_werte(bbox: &BBox, w: f64, h: f64, margin: f64) -> (f64, Vec2, Option<String>) {
    let avail_w = (w - 2.0 * margin).max(1.0);
    let avail_h = (h - 2.0 * margin).max(1.0);
    let need = (bbox.width() / avail_w).max(bbox.height() / avail_h);
    let scale = crate::pdf::nice_scale(need * 1.05);
    let warnung = if need * 1.05 > scale + 1e-9 {
        Some("Zeichnung ist zu groß für dieses Blatt — Maßstab 1:1000 reicht nicht".to_string())
    } else {
        None
    };
    (scale, bbox.center(), warnung)
}

// ---------------------------------------------------------------------------
// Ausführung
// ---------------------------------------------------------------------------

/// Führt eine Aktion aus. Rückgabe: JSON mit Ergebnisdetails.
/// Legt eine Maßkette an — ein Weg für den Befehl und die drei Werkzeuge.
///
/// `feste_achse` kommt vom Werkzeug (waagerecht, senkrecht, direkt); ohne
/// Angabe bestimmt der Befehl die Achse aus der Dominanz der Punktdifferenz.
///
/// Die Messpunkte werden **sortiert** abgelegt (`render::dim_sortieren`): sie
/// dürfen in beliebiger Reihenfolge geklickt werden, die Kette zeigt sie immer
/// in der Reihenfolge der Maßlinie. Sonst entstünde ein Zickzack, sobald ein
/// Punkt in der falschen Reihenfolge gesetzt wurde (T0154, Turn 3).
fn dim_kette(
    doc: &mut Document,
    p: &Params,
    ebene: Option<&'static str>,
    feste_achse: Option<DimAxis>,
) -> Result<Value, String> {
    let pts = p
        .pts("pts")
        .ok_or_else(|| "Parameter 'pts' fehlt".to_string())?;
    if pts.len() < 2 {
        return Err("Maßkette braucht mindestens zwei Punkte".into());
    }
    // Orientierung: ein ausdrücklicher Winkel (`angle`, Grad) sticht; sonst
    // die Eingabeweise (`axis` bzw. das Werkzeug), sonst die Dominanz der
    // Punktdifferenz. Zwei verschiedene Punkte dürfen nie den Maßtext „0,0“
    // ergeben; eine gemischte Kette ohne brauchbare Richtung ist ein Fehler
    // (T0038).
    let angle = match p.angle("angle") {
        Some(a) => a,
        None => {
            let axis = match feste_achse.or_else(|| p.dim_axis("axis")) {
                Some(a) => a,
                None => DimAxis::aus_kette(&pts).ok_or_else(|| {
                    "Maßkette · Achse nicht bestimmbar — `axis` angeben".to_string()
                })?,
            };
            axis.winkel(&pts).ok_or_else(|| {
                "Maßkette · Achse nicht bestimmbar — `axis` angeben".to_string()
            })?
        }
    };
    let pts = crate::render::dim_sortieren(&pts, angle);
    let line_pos = p
        .pt("line_pos")
        .or_else(|| p.pt("pos"))
        .unwrap_or_else(|| {
            let b = BBox::from_points(&pts);
            v(b.min.x, b.min.y - doc.settings.annotation_scale * 8.0)
        });
    let layer = resolve_layer_in(doc, p, ebene);
    let idn = doc.new_id();
    let mut e = Entity::new(
        idn,
        layer,
        EntityKind::Dim {
            pts,
            line_pos,
            angle,
            override_text: p.text("text"),
            ext_lines: p.boolean_or("ext_lines", false),
            symbol: dim_symbol(doc, p)?,
        },
    );
    apply_common(doc, &mut e, p, layer);
    bemasung_schwarz(p, &mut e);
    doc.add_entity(e);
    Ok(json!({"ok": true, "created": [idn]}))
}

/// Punktsymbol einer neuen Maßkette (T0227): der Wunsch des Aufrufers, sonst
/// die Vorgabe „Schrägstrich“. Ein leerer Text heißt „kein Symbol“; ein
/// genannter Block muss existieren, sonst entstünde eine Marke, die niemand
/// zeichnen kann.
fn dim_symbol(doc: &Document, p: &Params) -> Result<String, String> {
    let sym = p
        .text("symbol")
        .unwrap_or_else(|| crate::model::DIM_SYMBOL_VORGABE.to_string());
    let sym = sym.trim().to_string();
    if !sym.is_empty() {
        pruefe_punktsymbol(doc, &sym)?;
    }
    Ok(sym)
}

/// Prüft eine Punktsymbol-Angabe (T0210): der genannte Block muss existieren.
///
/// Die Kategorie wird **nicht** verlangt. Die Auswahlliste im Eigenschaften-Panel
/// bietet nur „Punkt Symbole“ an; ein Aufruf über MCP darf aber jeden Block als
/// Marke setzen. Ein unbekannter Block ist dagegen ein Fehler — sonst stünde ein
/// Objekt mit einem Symbol in der Datei, das niemand zeichnen kann (T0038).
pub fn pruefe_punktsymbol(doc: &Document, id: &str) -> Result<(), String> {
    if doc.block(id.trim()).is_some() {
        Ok(())
    } else {
        Err(format!("Punktsymbol '{id}' ist kein Block dieser Zeichnung"))
    }
}

pub fn execute(doc: &mut Document, id: &str, params: &Value) -> Result<Value, String> {
    let p = Params::new(params, doc.input_unit());
    // Typgerechte Standardebene dieser Aktion (None = keine Vorgabe).
    let ebene = standard_ebene(id);

    match id {
        // ---------------- Zeichnen ----------------
        "draw.point" => {
            let pt = require_pt(&p, "p").or_else(|_| require_pt(&p, "at"))?;
            let layer = resolve_layer_in(doc, &p, ebene);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Point { p: pt });
            apply_common(doc, &mut e, &p, layer);
            // Punktsymbol (T0210): Block aus der Kategorie „Punkt Symbole“.
            if let Some(sym) = p.text("symbol").filter(|s| !s.trim().is_empty()) {
                pruefe_punktsymbol(doc, &sym)?;
                e.start_symbol = Some(sym);
            }
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [id]}))
        }
        "draw.line" => {
            let a = require_pt(&p, "a")?;
            let b = require_pt(&p, "b")?;
            let layer = resolve_layer_in(doc, &p, ebene);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Line { a, b });
            apply_common(doc, &mut e, &p, layer);
            // Punktsymbole an Anfang und Ende (T0210).
            if let Some(sym) = p.text("start_symbol").filter(|s| !s.trim().is_empty()) {
                pruefe_punktsymbol(doc, &sym)?;
                e.start_symbol = Some(sym);
            }
            if let Some(sym) = p.text("end_symbol").filter(|s| !s.trim().is_empty()) {
                pruefe_punktsymbol(doc, &sym)?;
                e.end_symbol = Some(sym);
            }
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
            let layer = resolve_layer_in(doc, &p, ebene);
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
            let layer = resolve_layer_in(doc, &p, ebene);
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
            let layer = resolve_layer_in(doc, &p, ebene);
            let id = doc.new_id();
            let mut e = Entity::new(id, layer, EntityKind::Ellipse { c, rx, ry, rot });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [id]}))
        }
        "draw.image" => {
            // `a`→`b` ist die **untere Kante**: Anfang, Richtung (Drehung) und
            // Breite. Die Höhe folgt aus dem Seitenverhältnis der Datei —
            // solange die Sperre an ist (Vorgabe). Ohne Sperre gilt `height`.
            let a = require_pt(&p, "a")?;
            let b = require_pt(&p, "b")?;
            let breite = a.dist(b);
            if breite <= POINT_EPS_MM {
                return Err("Bild: Die untere Kante braucht eine Länge".into());
            }
            // Eingebettete Daten (Oberfläche) haben Vorrang; sonst wird die
            // Datei gelesen. Beides landet als Base64 im Dokument.
            let (name, data) = match p.text("data") {
                Some(d) if !d.trim().is_empty() => (p.text_or("name", "Bild"), d),
                _ => {
                    let pfad = p
                        .text("file")
                        .ok_or_else(|| "Parameter 'file' oder 'data' fehlt".to_string())?;
                    crate::bild::lade_datei(std::path::Path::new(&pfad))?
                }
            };
            let Some(bild) = crate::bild::dekodiere(&data) else {
                return Err("Bilddaten sind nicht lesbar (PNG oder JPEG erwartet)".into());
            };
            let lock_ratio = p.boolean_or("lock_ratio", true);
            let height = p
                .len("height")
                .filter(|h| *h > POINT_EPS_MM)
                .unwrap_or_else(|| crate::bild::hoehe_aus_breite(&bild, breite));
            let layer = resolve_layer_in(doc, &p, ebene);
            let id = doc.new_id();
            let mut e = Entity::new(
                id,
                layer,
                EntityKind::Image {
                    a,
                    b,
                    height,
                    lock_ratio,
                    name,
                    data,
                },
            );
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
            let layer = resolve_layer_in(doc, &p, ebene);
            let idn = doc.new_id();
            let verts: Vec<Vertex> = pts.iter().map(|q| Vertex::new(*q)).collect();
            // Nur ein Element (zwei Punkte) ist keine Polylinie, sondern eine
            // Linie (T0149) — dieselbe Regel wie beim Stutzen.
            let Some(kind) = polyline_kind(&verts, closed) else {
                return Err("Mindestens zwei verschiedene Punkte nötig".into());
            };
            let mut e = Entity::new(idn, layer, kind);
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Parallele (T0200): aus einer Linie oder Polylinie eine neue im
        // Abstand daneben. Die Seite bestimmt der Punkt `side` (der Mauszeiger) —
        // derselbe Wert, den die Vorschau zeigt. Ebene und Stil erbt das neue
        // Objekt von der Vorlage; ein „color“/„lineweight“ aus dem Panel gilt hier
        // nicht, eine Parallele ist immer eine Kopie ihrer Quelle.
        "draw.parallel" => {
            let ids = ids_from(&p, "ids");
            let Some(src) = ids.first().and_then(|id| doc.entity(*id).cloned()) else {
                return Err("Parameter 'ids' fehlt: Parallele braucht eine Linie oder Polylinie".into());
            };
            let dist = p
                .len("distance")
                .or_else(|| p.len("dist"))
                .ok_or_else(|| "Parameter 'distance' fehlt".to_string())?;
            if dist.abs() <= POINT_EPS_MM {
                return Err("Parallele: Der Abstand darf nicht null sein".into());
            }
            let side = p
                .pt("side")
                .or_else(|| p.pt("at"))
                .ok_or_else(|| "Parameter 'side' (Seite) fehlt".to_string())?;
            let Some(kind) = parallel_kind(&src, dist.abs(), side) else {
                return Err("Parallele geht nur mit einer Linie oder Polylinie".into());
            };
            let idn = doc.new_id();
            let mut ne = Entity::new(idn, src.layer, kind);
            ne.style = src.style.clone();
            ne.by_block = src.by_block;
            doc.insert_ranked(ne);
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
            let layer = resolve_layer_in(doc, &p, ebene);
            let idn = doc.new_id();
            let verts = pts.iter().map(|q| Vertex::new(*q)).collect();
            let mut e = Entity::new(idn, layer, EntityKind::Spline { verts, closed });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Pfad (T0229): eine Kette aus Linien und Bögen. `pts` sind die
        // Klickpunkte, `schritte` sagt je Klick, was er beginnt (`start`,
        // `line`, `arc_point`, `arc_end`). Fehlt `schritte`, ist jedes Stück
        // eine Linie — das ist der einfachste Aufruf über MCP.
        "draw.path" => {
            let pts = p
                .pts("pts")
                .or_else(|| p.pts("points"))
                .ok_or_else(|| "Parameter 'pts' fehlt".to_string())?;
            if pts.len() < 2 {
                return Err("Mindestens zwei Punkte nötig".into());
            }
            let mut schritte: Vec<crate::pfad::PfadSchritt> = vec![crate::pfad::PfadSchritt::Start];
            if let Some(arr) = p.raw("schritte").and_then(|v| v.as_array()) {
                for (i, v) in arr.iter().enumerate() {
                    if i == 0 {
                        continue;
                    }
                    let key = v.as_str().unwrap_or("line");
                    let s = crate::pfad::PfadSchritt::from_key(key)
                        .ok_or_else(|| format!("Unbekannter Schritt '{key}'"))?;
                    schritte.push(s);
                }
            }
            while schritte.len() < pts.len() {
                schritte.push(crate::pfad::PfadSchritt::Line);
            }
            let mut segments = crate::pfad::pfad_aus_punkten(&pts, &schritte);
            if segments.is_empty() {
                return Err("Der Pfad braucht mindestens ein Stück".into());
            }
            let spline = segments.iter().any(|s| s.hat_spline());
            let closed = !spline
                && p.boolean_or("closed", crate::pfad::pfad_geschlossen(&segments));
            let layer = resolve_layer_in(doc, &p, ebene);
            let idn = doc.new_id();
            let mut e = Entity::new(idn, layer, EntityKind::Path { segments, closed });
            apply_common(doc, &mut e, &p, layer);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Verbinden (T0229): beliebig viele ausgewählte Linien, Bögen,
        // Ellipsenbögen und Splines werden geprüft und zu **einem** Pfad.
        // Hängen sie nicht zusammen, bleibt alles stehen und es gibt einen
        // Fehler — kein stiller Teilerfolg.
        "path.connect" => {
            let ids = ids_from(&p, "ids");
            if ids.len() < 2 {
                return Err("Mindestens zwei Objekte wählen".into());
            }
            let mut teile: Vec<EntityKind> = Vec::new();
            for id in &ids {
                let Some(e) = doc.entity(*id) else {
                    return Err(format!("Objekt {id} gibt es nicht"));
                };
                teile.push(e.kind.clone());
            }
            let segments = crate::pfad::pfad_verbinden(&teile)?;
            let spr = segments.iter().any(|s| s.hat_spline());
            let closed = !spr && crate::pfad::pfad_geschlossen(&segments);
            let Some(vorlage) = ids.first().and_then(|id| doc.entity(*id)).cloned() else {
                return Err("Mindestens zwei Objekte wählen".into());
            };
            let idn = doc.new_id();
            let mut e = Entity::new(idn, vorlage.layer, EntityKind::Path { segments, closed });
            e.style = vorlage.style.clone();
            e.by_block = vorlage.by_block;
            doc.insert_ranked(e);
            for id in &ids {
                doc.remove_entity(*id);
            }
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
            add_fill(doc, boundary, &p, true, ebene)
        }
        "draw.fill" | "draw.hatch" => {
            let boundary = p
                .pts("boundary")
                .or_else(|| p.pts("pts"))
                .ok_or_else(|| "Parameter 'boundary' fehlt".to_string())?;
            add_fill(doc, boundary, &p, false, ebene)
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
            add_filled(doc, boundary, holes, &p, true, true, ebene)
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
            add_filled(doc, boundary, holes, &p, true, true, ebene)
        }
        "draw.volume.paint" => {
            let pt = require_pt(&p, "p")?;
            let g = crate::boundary::detect_boundary(
                doc,
                pt,
                &crate::boundary::BoundaryOpts::default(),
            )
            .map_err(|e| e.message().to_string())?;
            add_filled(doc, g.outer, g.holes, &p, false, true, ebene)
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
            // Die Fläche ordnet sich über den Zeichenrang ein (T0048): sie
            // liegt hinter der ermittelten Umgrenzung (`border_entities`),
            // deren Platz dafür nicht mehr gebraucht wird.
            add_fill_rings(doc, g.outer, g.holes, &p, false, ebene)
        }
        "draw.text" => {
            let pt = require_pt(&p, "p")?;
            let text = p.text_or("text", "");
            if text.is_empty() {
                return Err("Parameter 'text' fehlt".into());
            }
            let layer = resolve_layer_in(doc, &p, ebene);
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
                    line_spacing: p.num("line_spacing").unwrap_or(1.0).clamp(0.5, 3.0),
                    frame: p.boolean_or("frame", false),
                    frame_round: p.boolean_or("frame_round", false),
                    background: p.color("background"),
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
            let layer = resolve_layer_in(doc, &p, ebene);
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
                    // Schraffur der Wand: derselbe Parameterschlüssel wie bei
                    // der Füllfläche (`FillPattern::from_key`).
                    hatch: p
                        .fill_pattern("hatch")
                        .filter(|hp| *hp != crate::style::FillPattern::Solid),
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
            // Ohne Wand gibt es keine vordere Wandkante — dann entstünde eine
            // freistehende Tür, die kein Bauteil ist. Das ist ein Fehler, kein
            // stiller Erfolg (T0038).
            let host = host.filter(|h| {
                matches!(doc.entity(*h).map(|e| &e.kind), Some(EntityKind::Wall { .. }))
            });
            if host.is_none() {
                return Err(TUER_OHNE_WAND.into());
            }
            if angle.is_none() {
                if let Some(h) = host {
                    if let Some(EntityKind::Wall { a, b, .. }) = doc.entity(h).map(|e| &e.kind) {
                        angle = Some((*b - *a).angle());
                    }
                }
            }
            let layer = resolve_layer_in(doc, &p, ebene);
            let swing_in = p.boolean_or("swing_in", true);
            let width = p.len_or("width", doc.settings.door_width);
            // Basispunkt der Tür (T0091): der Klickpunkt ist Links/Mitte/Rechts
            // der Türbreite, das Zentrum liegt um den Versatz daneben.
            let base = p
                .text("base")
                .and_then(|t| crate::model::DoorBase::from_text(&t))
                .unwrap_or_default();
            // Nur die freie Wandstrecke zählt: an verbundenen Ecken endet die
            // Wand an der Anschlussnaht (Gehrungspunkte der Umrandung), nicht
            // an der Stirnebene (T0078).
            let at = match (host, opening_span(doc, host, width)) {
                (_, Some((lo, _hi))) => {
                    Some(door_at_from_base(doc, host, center, width, base).unwrap_or(lo))
                }
                (Some(_), None) => return Err(TUER_ZU_BREIT.into()),
                (None, None) => None,
            };
            // Auf der begrenzten Längslage sitzen: sonst stünde die Tür
            // außerhalb der Wand, obwohl ihre Lage gekappt ist.
            let center = match (at, host) {
                (Some(t), Some(h)) => wall_axis_point(doc, h, t).unwrap_or(center),
                _ => center,
            };
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
                    width,
                    kind: p.door_kind("kind").unwrap_or(DoorKind::Hinge),
                    hinge: p.num_or("hinge", 1.0),
                    swing_in,
                    host,
                    at,
                    base,
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
            // Ohne Wand gibt es keine Wanddicke, in der das Fenster läge —
            // ein freistehendes Fenster ist kein Bauteil (T0038).
            let host = host.filter(|h| {
                matches!(doc.entity(*h).map(|e| &e.kind), Some(EntityKind::Wall { .. }))
            });
            if host.is_none() {
                return Err(FENSTER_OHNE_WAND.into());
            }
            if angle.is_none() {
                if let Some(h) = host {
                    if let Some(EntityKind::Wall { a, b, .. }) = doc.entity(h).map(|e| &e.kind) {
                        angle = Some((*b - *a).angle());
                    }
                }
            }
            let layer = resolve_layer_in(doc, &p, ebene);
            let align = match p.text("align").unwrap_or_default().to_lowercase().as_str() {
                "left" | "links" => crate::style::WallAlign::Left,
                "right" | "rechts" => crate::style::WallAlign::Right,
                _ => crate::style::WallAlign::Center,
            };
            let width = p.len_or("width", doc.settings.window_width);
            // Wie bei der Tür: die freie Wandstrecke zwischen den
            // Anschlussnähten ist die Grenze (T0078).
            let at = match (host, opening_span(doc, host, width)) {
                (_, Some((lo, hi))) => {
                    Some(wall_at(doc, host, center).unwrap_or(lo).clamp(lo, hi))
                }
                (Some(_), None) => return Err(FENSTER_ZU_BREIT.into()),
                (None, None) => None,
            };
            let center = match (at, host) {
                (Some(t), Some(h)) => wall_axis_point(doc, h, t).unwrap_or(center),
                _ => center,
            };
            let center = offset_opening(doc, host, center, angle.unwrap_or(0.0), align);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Window {
                    align,
                    center,
                    angle: angle.unwrap_or(0.0),
                    width,
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
            let layer = resolve_layer_in(doc, &p, ebene);
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
            let layer = resolve_layer_in(doc, &p, ebene);
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
                    line_spacing: 1.0,
                    frame: false,
                    frame_round: false,
                    background: None,
                },
            );
            apply_common(doc, &mut t, &p, layer);
            doc.add_entity(t);
            Ok(json!({"ok": true, "created": [idn], "text": mass.beschriftung()}))
        }

        // ---------------- Bemaßung ----------------
        // Einzelmaß: zwei Messpunkte — dasselbe Element wie die Maßkette,
        // nur mit genau zwei Punkten (T0154). `a`/`b` bleiben als Parameter,
        // damit MCP-Aufrufe und ältere Skripte unverändert laufen.
        "dim.linear" => {
            let a = require_pt(&p, "a")?;
            let b = require_pt(&p, "b")?;
            let line_pos = p.pt("line_pos").or_else(|| p.pt("pos")).unwrap_or_else(|| {
                let m = (a + b) * 0.5;
                v(m.x, m.y + doc.settings.annotation_scale * 8.0)
            });
            let angle = match p.angle("angle") {
                Some(w) => w,
                None => {
                    let axis = p.dim_axis("axis").unwrap_or_else(|| {
                        if (b.x - a.x).abs() >= (b.y - a.y).abs() {
                            DimAxis::X
                        } else {
                            DimAxis::Y
                        }
                    });
                    axis.winkel(&[a, b]).unwrap_or(0.0)
                }
            };
            let layer = resolve_layer_in(doc, &p, ebene);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Dim {
                    pts: vec![a, b],
                    line_pos,
                    angle,
                    override_text: p.text("text"),
                    ext_lines: p.boolean_or("ext_lines", false),
                    symbol: dim_symbol(doc, &p)?,
                },
            );
            apply_common(doc, &mut e, &p, layer);
            bemasung_schwarz(&p, &mut e);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Maßkette. Die drei Werkzeuge (`.h`, `.v`, `.d`) sind derselbe
        // Befehl mit **fester** Achse — der Modus steckt im Werkzeug, nicht in
        // einem Parameter, den die Oberfläche zusätzlich führen müsste (T0154,
        // Turn 3).
        "dim.chain" | "dim.chain.h" | "dim.chain.v" | "dim.chain.d" => {
            let feste_achse = match id {
                "dim.chain.h" => Some(DimAxis::X),
                "dim.chain.v" => Some(DimAxis::Y),
                "dim.chain.d" => Some(DimAxis::Aligned),
                _ => None,
            };
            dim_kette(doc, &p, ebene, feste_achse)
        }
        // Einen Messpunkt an eine bestehende Maßkette anhängen (T0154).
        //
        // Der Punkt wird an der Stelle eingefügt, die die Kette entlang ihrer
        // Messrichtung geordnet hält — sonst entstünde ein Zickzack mit
        // negativen Maßzahlen. `index` überschreibt das ausdrücklich.
        "dim.point.add" => {
            let ids = ids_from(&p, "ids");
            if ids.len() != 1 {
                return Err("dim.point.add · genau eine Maßkette angeben".to_string());
            }
            let id = ids[0];
            let q = require_pt(&p, "p").or_else(|_| require_pt(&p, "pos"))?;
            let (idx, punkte) = {
                let e = doc
                    .entity_mut(id)
                    .ok_or_else(|| format!("Objekt {id} nicht gefunden"))?;
                let EntityKind::Dim { pts, angle, .. } = &mut e.kind else {
                    return Err("dim.point.add · Objekt ist keine Maßkette".to_string());
                };
                let idx = match p.num("index") {
                    Some(i) => (i.round().max(0.0) as usize).min(pts.len()),
                    None => einfuege_index(pts, *angle, q),
                };
                pts.insert(idx, q);
                (idx, pts.len())
            };
            doc.modified = true;
            Ok(json!({"ok": true, "changed": [id], "index": idx, "punkte": punkte}))
        }
        // Einen Messpunkt entfernen. Unter zwei Punkten ist eine Maßkette
        // keine Maßkette mehr — das ist ein Fehler, kein stiller Erfolg.
        "dim.point.remove" => {
            let ids = ids_from(&p, "ids");
            if ids.len() != 1 {
                return Err("dim.point.remove · genau eine Maßkette angeben".to_string());
            }
            let id = ids[0];
            let (idx, punkte) = {
                let e = doc
                    .entity_mut(id)
                    .ok_or_else(|| format!("Objekt {id} nicht gefunden"))?;
                let EntityKind::Dim { pts, .. } = &mut e.kind else {
                    return Err("dim.point.remove · Objekt ist keine Maßkette".to_string());
                };
                if pts.len() <= 2 {
                    return Err("Maßkette braucht mindestens zwei Messpunkte".to_string());
                }
                let idx = match p.num("index") {
                    Some(i) => (i.round().max(0.0) as usize).min(pts.len() - 1),
                    None => {
                        let q = require_pt(&p, "p").or_else(|_| require_pt(&p, "pos"))?;
                        pts.iter()
                            .enumerate()
                            .min_by(|(_, a), (_, b)| {
                                a.dist(q)
                                    .partial_cmp(&b.dist(q))
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(i, _)| i)
                            .unwrap_or(0)
                    }
                };
                pts.remove(idx);
                (idx, pts.len())
            };
            doc.modified = true;
            Ok(json!({"ok": true, "changed": [id], "index": idx, "punkte": punkte}))
        }
        // Grundriss-Kote: Kreis-Symbol, Höhe kommt aus dem Panel. Die Lage des
        // Höhenwertes ist `text_pos` (Vorgabe „oben", T0196) — ein unbekannter
        // Wert ist ein Fehler, kein stiller Rückfall auf die Vorgabe.
        "dim.elevation.plan" => {
            let pt = require_pt(&p, "p")?;
            let layer = resolve_layer_in(doc, &p, ebene);
            let idn = doc.new_id();
            let z = p.len_or("z", 0.0);
            let text_pos = match p.text("text_pos") {
                Some(t) => crate::model::ElevationTextPos::from_text(&t).ok_or_else(|| {
                    format!("Unbekannte Textposition '{t}' (oben, unten, links, rechts)")
                })?,
                None => crate::model::ElevationTextPos::default(),
            };
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::ElevationPlan {
                    p: pt,
                    z,
                    text_pos,
                },
            );
            apply_common(doc, &mut e, &p, layer);
            bemasung_schwarz(&p, &mut e);
            doc.add_entity(e);
            Ok(json!({
                "ok": true,
                "created": [idn],
                "z_mm": z,
                "text_pos": text_pos.key(),
            }))
        }
        // Schnitt-Kote: eine Punktfolge. Der erste Punkt ist ±0,00, die
        // übrigen Höhen rechnet der Renderer aus der Y-Differenz — hier wird
        // nur die Punktfolge geprüft und abgelegt.
        "dim.elevation.section" => {
            let pts = p
                .pts("pts")
                .ok_or_else(|| "Parameter 'pts' fehlt".to_string())?;
            if pts.is_empty() {
                return Err("Schnitt-Kote braucht mindestens einen Punkt".to_string());
            }
            let layer = resolve_layer_in(doc, &p, ebene);
            let idn = doc.new_id();
            let mut e = Entity::new(idn, layer, EntityKind::ElevationSection { pts });
            apply_common(doc, &mut e, &p, layer);
            bemasung_schwarz(&p, &mut e);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
        }
        // Raumstempel: die Umgrenzung kommt als Punktfolge (wie die Fläche
        // nach Punktfolge). Die Beschriftung setzt die Action selbst in den
        // „Pole of Inaccessibility" — den Punkt der Fläche mit dem größten
        // Abstand zu allen Kanten. Eine Fläche ohne Ausdehnung ist ein Fehler
        // (T0038), kein Stempel im Ursprung.
        //
        // **Zwei Punkte** sind die gegenüberliegenden Ecken eines
        // achsenparallelen Rechtecks (T0220, Turn 3): die bequeme Eingabe für
        // einen rechteckigen Raum — zwei Klicks statt vier. Die vier Ecken
        // entstehen in `geom::rechteck_aus_diagonale`; liegen beide Punkte auf
        // einer Achse, umschließt nichts eine Fläche und das ist ein Fehler.
        "dim.room" => {
            let pts = p
                .pts("pts")
                .or_else(|| p.pts("boundary"))
                .ok_or_else(|| "Parameter 'pts' fehlt".to_string())?;
            let (mut boundary, _) = crate::model::polyline_points(
                &pts,
                true,
                crate::model::POINT_EPS_MM,
            );
            if boundary.len() == 2 {
                boundary = crate::geom::rechteck_aus_diagonale(boundary[0], boundary[1])
                    .ok_or_else(|| {
                        "Die zwei Punkte brauchen einen Abstand in beiden Richtungen".to_string()
                    })?
                    .to_vec();
            }
            if boundary.len() < 3 {
                return Err("Raumstempel braucht mindestens zwei Punkte".into());
            }
            let flaeche = crate::area::area_of(&boundary, &[]);
            if !(flaeche > 1.0) {
                return Err("Die Umgrenzung umschließt keine Fläche".into());
            }
            let label = p
                .pt("label")
                .unwrap_or_else(|| crate::area::pole_of_inaccessibility(&boundary, &[], 0.5));
            let layer = resolve_layer_in(doc, &p, ebene);
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                layer,
                EntityKind::Room {
                    boundary,
                    label,
                    height: p.len_or("height", doc.settings.text_height),
                    heading: p.text_or("heading", "Zimmer"),
                },
            );
            apply_common(doc, &mut e, &p, layer);
            bemasung_schwarz(&p, &mut e);
            doc.add_entity(e);
            Ok(json!({"ok": true, "created": [idn]}))
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
            // Öffnungen wandern mit ihrer Wand — auch beim **Kopieren** (T0207):
            // vorher blieben Türen und Fenster am Original, die Kopie war eine
            // nackte Wand.
            let mut ids = ids;
            for extra in carried_openings(doc, &ids) {
                if !ids.contains(&extra) {
                    ids.push(extra);
                }
            }
            if copy {
                let (created, _) = kopiere_mit_oeffnungen(doc, &ids, &m);
                new_ids = created;
            } else {
                for eid in &ids {
                    if let Some(em) = doc.entity_mut(*eid) {
                        *em = transform::transform_entity(em, &m);
                    }
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
            // Ein Bild dreht mit seiner unteren Kante mit — die Kante wandert
            // durch dieselbe Abbildung wie jede andere Geometrie.
            let m = Affine::rotate_about(center, angle);
            // Öffnungen wandern mit ihrer Wand — auch beim Kopieren (T0207).
            let mut ids = ids;
            for extra in carried_openings(doc, &ids) {
                if !ids.contains(&extra) {
                    ids.push(extra);
                }
            }
            let new_ids = if copy {
                let (created, _) = kopiere_mit_oeffnungen(doc, &ids, &m);
                created
            } else {
                for eid in &ids {
                    if let Some(em) = doc.entity_mut(*eid) {
                        *em = transform::transform_entity(em, &m);
                    }
                }
                Vec::new()
            };
            doc.modified = true;
            Ok(json!({"ok": true, "created": new_ids}))
        }
        "modify.mirror" => {
            let ids = ids_from(&p, "ids");
            let p1 = require_pt(&p, "p1")?;
            let p2 = require_pt(&p, "p2")?;
            let keep = p.boolean_or("keep_original", true);
            let m = Affine::mirror(p1, p2);
            // Öffnungen wandern mit ihrer Wand — auch beim Spiegeln (T0207).
            let mut ids = ids;
            for extra in carried_openings(doc, &ids) {
                if !ids.contains(&extra) {
                    ids.push(extra);
                }
            }
            let new_ids = if keep {
                let (created, _) = kopiere_mit_oeffnungen(doc, &ids, &m);
                created
            } else {
                for eid in &ids {
                    if let Some(em) = doc.entity_mut(*eid) {
                        *em = transform::transform_entity(em, &m);
                    }
                }
                Vec::new()
            };
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
        "modify.intersect" => {
            // Zwei gerade Objekte — Reihenfolge egal, es zählt nur das Paar.
            let ids = ids_from(&p, "ids");
            if ids.len() < 2 {
                return Err("Verschneiden braucht zwei Objekte".into());
            }
            let punkt = verschneiden(doc, ids[0], ids[1])?;
            Ok(json!({
                "ok": true,
                "point": [punkt.x / 1000.0, punkt.y / 1000.0]
            }))
        }
        // Flächen vereinigen: zwei oder mehr angrenzende Füllflächen werden
        // eine. Die Geometrie rechnet `flaeche::vereinigen`; Muster, Rand, Stil
        // und Ebene kommen von der ersten Fläche, die übrigen verschwinden.
        "modify.area.union" => {
            let ids = ids_from(&p, "ids");
            if ids.len() < 2 {
                return Err("Vereinigen braucht mindestens zwei Flächen".into());
            }
            let mut geometrien = Vec::new();
            let mut vorlage: Option<Entity> = None;
            for id in &ids {
                let Some(e) = doc.entity(*id).cloned() else {
                    return Err(format!("Objekt {id} nicht gefunden"));
                };
                match &e.kind {
                    EntityKind::Fill { boundary, holes, .. } => {
                        geometrien.push(crate::flaeche::Flaeche {
                            outer: boundary.clone(),
                            holes: holes.clone(),
                        });
                        if vorlage.is_none() {
                            vorlage = Some(e);
                        }
                    }
                    _ => return Err("Vereinigen geht nur mit Füllflächen".into()),
                }
            }
            let Some(vorlage) = vorlage else {
                return Err("Keine Füllfläche ausgewählt".into());
            };
            let ziel = crate::flaeche::vereinigen(&geometrien)?;
            let EntityKind::Fill {
                pattern,
                pattern_angle,
                pattern_scale,
                border,
                ..
            } = vorlage.kind.clone()
            else {
                return Err("Keine Füllfläche ausgewählt".into());
            };
            let idn = doc.new_id();
            let mut e = Entity::new(
                idn,
                vorlage.layer,
                EntityKind::Fill {
                    boundary: ziel.outer,
                    holes: ziel.holes,
                    pattern,
                    pattern_angle,
                    pattern_scale,
                    border,
                },
            );
            e.style = vorlage.style.clone();
            e.by_block = vorlage.by_block;
            for id in &ids {
                doc.remove_entity(*id);
            }
            doc.insert_ranked(e);
            doc.modified = true;
            Ok(json!({"ok": true, "created": [idn], "deleted": ids.len()}))
        }
        // Fläche schneiden: die Punktfolge ist die Schnittlinie. Sie zerlegt
        // die Füllfläche in zwei; das Original wird durch die beiden Teile
        // ersetzt, Muster, Rand, Stil und Ebene bleiben.
        "modify.area.split" => {
            let pts = p
                .pts("pts")
                .ok_or_else(|| "Parameter 'pts' (Punktfolge) fehlt".to_string())?;
            let ids = ids_from(&p, "ids");
            // Zielfläche: die ausdrücklich genannte, sonst die Fläche, in der
            // die Schnittlinie liegt.
            let ziel_id = match ids.first().copied() {
                Some(id) => id,
                None => doc
                    .ents()
                    .iter()
                    .find(|e| match &e.kind {
                        EntityKind::Fill { boundary, holes, .. } => pts
                            .iter()
                            .any(|q| crate::area::point_in_area(*q, boundary, holes)),
                        _ => false,
                    })
                    // Sonst die Fläche, deren Umgrenzung die Linie kreuzt —
                    // die Schnittlinie darf außerhalb beginnen und enden.
                    .or_else(|| {
                        doc.ents().iter().find(|e| match &e.kind {
                            EntityKind::Fill { boundary, .. } => {
                                crate::flaeche::kreuzt_ring(boundary, &pts)
                            }
                            _ => false,
                        })
                    })
                    .map(|e| e.id)
                    .ok_or_else(|| "Keine Fläche unter der Schnittlinie".to_string())?,
            };
            let Some(vorlage) = doc.entity(ziel_id).cloned() else {
                return Err(format!("Objekt {ziel_id} nicht gefunden"));
            };
            let EntityKind::Fill {
                boundary,
                holes,
                pattern,
                pattern_angle,
                pattern_scale,
                border,
            } = vorlage.kind.clone()
            else {
                return Err("Schneiden geht nur mit Füllflächen".into());
            };
            let (a, b) = crate::flaeche::teilen(&boundary, &holes, &pts)?;
            let mut created = Vec::new();
            for teil in [a, b] {
                let idn = doc.new_id();
                let mut e = Entity::new(
                    idn,
                    vorlage.layer,
                    EntityKind::Fill {
                        boundary: teil.outer,
                        holes: teil.holes,
                        pattern,
                        pattern_angle,
                        pattern_scale,
                        border,
                    },
                );
                e.style = vorlage.style.clone();
                e.by_block = vorlage.by_block;
                doc.insert_ranked(e);
                created.push(idn);
            }
            doc.remove_entity(ziel_id);
            doc.modified = true;
            Ok(json!({"ok": true, "created": created, "deleted": 1}))
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
            // Alle Objekte zusammen: die Umrandungen entstehen aus dem
            // unveränderten Stand (T0190, siehe `explode_entities`).
            let created = explode_entities(doc, &ids)?;
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
            // Punktsymbole (T0210) vorab lesen: ein unbekannter Block ist ein
            // Fehler, kein stiller Erfolg (T0038) — und geprüft wird, solange
            // das Dokument nur lesend geliehen ist. `symbol` ist die kurze Form
            // für den Anfang (ein Punkt hat nur diesen).
            let symbol_feld = |key: &str| -> Option<Option<String>> {
                let v = p.v.get(key)?;
                if v.is_null() {
                    return Some(None);
                }
                Some(
                    p.text(key)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                )
            };
            let symbol_start_neu = symbol_feld("symbol").or_else(|| symbol_feld("start_symbol"));
            let symbol_end_neu = symbol_feld("end_symbol");
            for sym in [&symbol_start_neu, &symbol_end_neu]
                .into_iter()
                .flatten()
                .flatten()
            {
                pruefe_punktsymbol(doc, sym)?;
            }
            // Punktsymbol an den Messpunkten einer Maßkette (T0227): derselbe
            // Schlüssel „symbol“. `Some(None)` = Symbol wegnehmen, `Some(Some)`
            // = setzen, `None` = unberührt — geprüft wird vorab, solange das
            // Dokument nur lesend geliehen ist (ein `pruefe` im Ausleihbereich
            // der Objekte wäre ein doppelter Borrow).
            let dim_symbol_neu = if p.v.get("symbol").is_some() {
                symbol_feld("symbol")
            } else {
                None
            };
            // Basispunkt der Tür (T0091) — verschiebt nur die Marke, nicht die Tür.
            let base_new = p
                .text("base")
                .and_then(|t| crate::model::DoorBase::from_text(&t));
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
                // Wird die Öffnung breiter, darf sie nicht über die Naht bzw.
                // das Wandende hinausragen — Längslage nachziehen (T0078).
                let at_neu: Option<f64> = match doc.entity(*eid).map(|e| &e.kind) {
                    Some(EntityKind::Door { host, at, .. })
                    | Some(EntityKind::Window { host, at, .. }) => p
                        .len("width")
                        .and_then(|w| at.map(|t| clamp_opening_at(doc, *host, w, t))),
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
                    let (breite, form) = p.loop_params();
                    if let Some(b) = breite {
                        e.style.loop_width = Some(b);
                    }
                    if let Some(f) = form {
                        e.style.loop_shape = Some(f);
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
                    // Punktsymbole an Anfang und Ende (T0210) — der Punkt
                    // trägt sein Symbol im Anfangsfeld. Nur Punkt und Linie
                    // zeichnen es; eine Maßkette führt ihr Symbol an den
                    // Messpunkten in `EntityKind::Dim::symbol` (T0227) und
                    // bekäme hier sonst ein zweites, ungenutztes Feld.
                    if let Some(s) = &symbol_start_neu {
                        if e.kann_punktsymbol() {
                            e.start_symbol = s.clone();
                        }
                    }
                    if let Some(s) = &symbol_end_neu {
                        e.end_symbol = s.clone();
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
                        // Leerer Wert, „none“/„aus“ schalten die Schraffur ab,
                        // „solid“ ebenso (Vollfläche an der Wand = keine
                        // Schraffur). Alles andere liest `FillPattern::from_key`.
                        let roh = h.trim().to_lowercase();
                        let hp = if roh.is_empty()
                            || matches!(roh.as_str(), "none" | "aus" | "keine" | "solid")
                        {
                            None
                        } else {
                            match crate::style::FillPattern::from_key(&roh) {
                                Some(crate::style::FillPattern::Solid) | None => None,
                                Some(x) => Some(x),
                            }
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
                        // Bild: Breite über die untere Kante (der Anfangspunkt
                        // bleibt stehen), Höhe gespeichert, Sperre und Drehung.
                        EntityKind::Image {
                            a: ia,
                            b: ib,
                            height: ih,
                            lock_ratio,
                            data,
                            ..
                        } => {
                            if let Some(v) = width {
                                let d = *ib - *ia;
                                if d.len() > 1e-6 {
                                    *ib = *ia + d.normalized() * v.max(1.0);
                                }
                            }
                            if let Some(v) = height {
                                *ih = v.max(1.0);
                                // Bei gesperrtem Verhältnis zieht die Breite
                                // mit, sonst stimmte die eingegebene Höhe nicht.
                                if *lock_ratio {
                                    if let Some(bild) = crate::bild::dekodiere(data) {
                                        if bild.hoehe > 0 {
                                            let breite =
                                                v * bild.breite as f64 / bild.hoehe as f64;
                                            let d = *ib - *ia;
                                            if d.len() > 1e-6 {
                                                *ib = *ia + d.normalized() * breite.max(1.0);
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(v) = p.boolean("lock_ratio") {
                                if v != *lock_ratio {
                                    // Beim Umschalten darf nichts springen:
                                    // gespeichert wird die aktuelle Höhe.
                                    if let Some(bild) = crate::bild::dekodiere(data) {
                                        *ih = crate::bild::hoehe_aus_breite(
                                            &bild,
                                            ia.dist(*ib),
                                        );
                                    }
                                }
                                *lock_ratio = v;
                            }
                            if let Some(w) = p.angle("angle") {
                                let d = *ib - *ia;
                                if d.len() > 1e-6 {
                                    *ib = *ia + Vec2::polar(w, d.len());
                                }
                            }
                        }
                        EntityKind::Door {
                            width: w,
                            at,
                            kind,
                            hinge,
                            swing_in,
                            base,
                            ..
                        } => {
                            if let Some(v) = width {
                                *w = v;
                            }
                            if let Some(t) = at_neu {
                                *at = Some(t);
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
                            if let Some(b) = base_new {
                                *base = b;
                            }
                        }
                        EntityKind::Window {
                            width: w,
                            at,
                            sill,
                            height: h,
                            ..
                        } => {
                            if let Some(v) = width {
                                *w = v;
                            }
                            if let Some(t) = at_neu {
                                *at = Some(t);
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
                            line_spacing,
                            frame,
                            frame_round,
                            background,
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
                            // Zeilenabstand: Vielfaches der Zeilenhöhe der Schrift.
                            if let Some(v) = p.num("line_spacing") {
                                *line_spacing = v.clamp(0.5, 3.0);
                            }
                            // Umrandung dauerhaft anzeigen, auf Wunsch rund (T0126).
                            if let Some(b) = p.boolean("frame") {
                                *frame = b;
                            }
                            if let Some(b) = p.boolean("frame_round") {
                                *frame_round = b;
                            }
                            // Hintergrund: Farbwert setzt die Fläche, "none"
                            // nimmt sie wieder weg — ohne diesen Weg ließe sich
                            // ein einmal gesetzter Hintergrund nie löschen.
                            match p.raw("background") {
                                Some(serde_json::Value::String(t)) if t == "none" => {
                                    *background = None;
                                }
                                Some(_) => {
                                    if let Some(c) = p.color("background") {
                                        *background = Some(c);
                                    }
                                }
                                None => {}
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
                        // Raumstempel: Textlage getrennt von der Fläche
                        // (`label` verschiebt nur das Textfeld), Überschrift,
                        // Textgröße und die Umgrenzung selbst.
                        EntityKind::Room {
                            boundary,
                            label,
                            height: h,
                            heading,
                        } => {
                            if let Some(v) = text {
                                *heading = v;
                            }
                            if let Some(v) = height_new {
                                *h = v.max(1.0);
                            }
                            if let Some(v) = p.pt("label") {
                                *label = v;
                            }
                            if let Some(pts) = p.pts("boundary") {
                                if pts.len() >= 3 {
                                    *boundary = pts;
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
                        // Ellipsenbogen: beide Halbachsen, Drehlage und die
                        // Parameterwinkel des Abschnitts (T0084).
                        EntityKind::EllipseArc { rx, ry, rot, a0, a1, .. } => {
                            if let Some(v) = p.len("rx") {
                                *rx = v.max(0.001);
                            }
                            if let Some(v) = p.len("ry") {
                                *ry = v.max(0.001);
                            }
                            if let Some(v) = p.angle("rot") {
                                *rot = v;
                            }
                            if let Some(v) = p.angle("a0") {
                                *a0 = v;
                            }
                            if let Some(v) = p.angle("a1") {
                                *a1 = v;
                            }
                        }
                        // Bemaßung: Basispunkte, Lage der Maßlinie und ihr
                        // Abstand sind änderbar — wie bei jedem anderen Objekt
                        // (T0055). Der Abstand kommt zuletzt, er rechnet aus
                        // dem (neuen) ersten Basispunkt.
                        // Bemaßung: Messpunkte, Lage der Maßlinie und ihr
                        // Abstand sind änderbar — wie bei jedem anderen Objekt
                        // (T0055). Der Abstand kommt zuletzt, er rechnet aus
                        // dem (neuen) ersten Messpunkt. `a`/`b` bleiben als
                        // Altlast gültig: ein Einzelmaß ist eine Kette aus zwei
                        // Punkten (T0154).
                        EntityKind::Dim {
                            pts,
                            line_pos,
                            angle,
                            override_text,
                            ext_lines,
                            symbol,
                        } => {
                            if let Some(v) = text {
                                *override_text = Some(v);
                            }
                            // Punktsymbol an den Messpunkten (T0227): leer =
                            // kein Symbol, sonst steht der Block geprüft bereit.
                            if let Some(neu) = &dim_symbol_neu {
                                *symbol = neu.clone().unwrap_or_default();
                            }
                            // Verlängerung zu den Messpunkten (T0226): der
                            // Schalter im Eigenschaften-Panel schickt hier
                            // `ext_lines`.
                            if let Some(v) = p.boolean("ext_lines") {
                                *ext_lines = v;
                            }
                            // Orientierung: `angle` (Grad) setzt sie frei,
                            // `axis` ist die bequeme Eingabeweise (T0166).
                            if let Some(w) = p.angle("angle") {
                                *angle = w;
                            } else if let Some(ax) = p.dim_axis("axis") {
                                if let Some(w) = ax.winkel(pts) {
                                    *angle = w;
                                }
                            }
                            let mut neue = p.pts("pts");
                            if neue.is_none() {
                                if let (Some(a), Some(b)) = (p.pt("a"), p.pt("b")) {
                                    neue = Some(vec![a, b]);
                                }
                            }
                            if let Some(q) = neue {
                                if q.len() < 2 {
                                    return Err(
                                        "Maßkette braucht mindestens zwei Messpunkte".to_string()
                                    );
                                }
                                *pts = q;
                            }
                            if let Some(q) = p.pt("line_pos") {
                                *line_pos = q;
                            }
                            if let Some(off) = p.len("offset") {
                                let (_, n) = crate::render::dim_achse(*angle);
                                let alt = (*line_pos - pts[0]).dot(n);
                                *line_pos = *line_pos + n * (off - alt);
                            }
                        }
                        EntityKind::ElevationPlan { z, text_pos, .. } => {
                            if let Some(v) = p.len("z") {
                                *z = v;
                            }
                            // Lage des Höhenwertes (T0196); ein unbekannter
                            // Wert ist ein Fehler, kein stiller Rückfall.
                            if let Some(t) = p.text("text_pos") {
                                *text_pos =
                                    crate::model::ElevationTextPos::from_text(&t).ok_or_else(
                                        || {
                                            format!(
                                                "Unbekannte Textposition '{t}' (oben, unten, links, rechts)"
                                            )
                                        },
                                    )?;
                            }
                        }
                        // Punkte der Schnitt-Kote lassen sich verschieben/ersetzen;
                        // die Höhen folgen daraus neu.
                        EntityKind::ElevationSection { pts } => {
                            if let Some(q) = p.pts("pts") {
                                if q.is_empty() {
                                    return Err(
                                        "Schnitt-Kote braucht mindestens einen Punkt"
                                            .to_string(),
                                    );
                                }
                                *pts = q;
                            }
                        }
                        // Ansichtsfenster (T0109): Papierlage und -größe in
                        // Millimetern (wie `sheet.update_viewport`), Maßstab,
                        // Modellmitte und Rahmen.
                        EntityKind::Viewport(vp) => {
                            if let Some(v) = p.num("x") {
                                vp.x = v;
                            }
                            if let Some(v) = p.num("y") {
                                vp.y = v;
                            }
                            if let Some(v) = p.num("w") {
                                vp.w = v.max(1.0);
                            }
                            if let Some(v) = p.num("h") {
                                vp.h = v.max(1.0);
                            }
                            if let Some(v) = p.num("scale").filter(|s| *s > 0.0) {
                                vp.scale = v;
                            }
                            if let Some(q) = p.pt("center") {
                                vp.center = q;
                            }
                            if let Some(b) = p.boolean("show_frame") {
                                vp.show_frame = b;
                            }
                            if let Some(n) = p.text("name") {
                                vp.name = n;
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

        // ---------------- Pläne (T0202) ----------------
        "plan.create" => {
            // Ein neuer Plan ist eine leere Zeichenfläche mit ihren vier
            // Standardlayern — genau wie „Plan 1“ in einer neuen Datei.
            let name = p
                .text("name")
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| format!("Plan {}", doc.plans.len() + 1));
            if doc.plans.iter().any(|x| x.name.eq_ignore_ascii_case(&name)) {
                return Err(format!("Plan '{name}' existiert bereits"));
            }
            let id = doc.new_id();
            doc.plans.push(Plan::new(id, &name));
            doc.add_standard_layers(id);
            Ok(json!({"ok": true, "id": id}))
        }
        "plan.update" => {
            let target = p
                .v
                .get("plan")
                .or_else(|| p.v.get("id"))
                .cloned()
                .ok_or_else(|| "Parameter 'plan' fehlt".to_string())?;
            let pid = target
                .as_u64()
                .or_else(|| {
                    target
                        .as_str()
                        .and_then(|s| doc.plans.iter().find(|x| x.name.eq_ignore_ascii_case(s)).map(|x| x.id))
                })
                .ok_or_else(|| "Plan nicht gefunden".to_string())?;
            let Some(plan) = doc.plan_mut(pid) else {
                return Err("Plan nicht gefunden".into());
            };
            if let Some(n) = p.text("name") {
                if !n.trim().is_empty() {
                    plan.name = n;
                }
            }
            if let Some(v) = p.boolean("visible") {
                plan.visible = v;
            }
            if let Some(v) = p.boolean("locked") {
                plan.locked = v;
            }
            if let Some(v) = p.boolean("collapsed") {
                plan.collapsed = v;
            }
            doc.modified = true;
            Ok(json!({"ok": true}))
        }
        "plan.list" => {
            let plans: Vec<Value> = doc
                .plans
                .iter()
                .map(|pl| {
                    let layer: Vec<Value> = doc
                        .layers_of_plan(pl.id)
                        .iter()
                        .map(|l| {
                            json!({
                                "id": l.id, "name": l.name, "visible": l.visible,
                                "locked": l.locked
                            })
                        })
                        .collect();
                    json!({
                        "id": pl.id, "name": pl.name, "visible": pl.visible,
                        "locked": pl.locked, "collapsed": pl.collapsed, "layers": layer
                    })
                })
                .collect();
            Ok(json!({"ok": true, "plans": plans, "current": doc.aktueller_plan()}))
        }

        // ---------------- Layer ----------------
        "layer.create" => {
            let name = p.text("name").ok_or_else(|| "Parameter 'name' fehlt".to_string())?;
            let plan = p
                .id("plan")
                .or_else(|| doc.aktueller_plan())
                .unwrap_or(0);
            if doc.plan(plan).is_none() {
                return Err("Plan nicht gefunden".into());
            }
            // Ein Layername ist je Plan eindeutig (T0202), nicht global.
            if doc
                .layers
                .iter()
                .any(|l| l.plan == plan && l.name.eq_ignore_ascii_case(&name))
            {
                return Err(format!("Layer '{name}' existiert bereits"));
            }
            let id = doc.new_id();
            let mut l = Layer::new(id, &name, p.color("color").unwrap_or(Color::BLACK));
            l.plan = plan;
            l.line_weight = p.num_or("lineweight", 0.25);
            l.line_type = p.linetype("linetype").unwrap_or(LineType::Solid);
            l.visible = p.boolean_or("visible", true);
            l.locked = p.boolean_or("locked", false);
            // Hinter den letzten Layer des Plans einsortieren, damit die
            // Anzeige gruppiert bleibt.
            let pos = doc
                .layers
                .iter()
                .rposition(|x| x.plan == plan)
                .map(|i| i + 1)
                .unwrap_or(doc.layers.len());
            doc.layers.insert(pos, l);
            Ok(json!({"ok": true, "id": id, "plan": plan}))
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
            // Ein Layer kann in einen anderen Plan wandern (T0202).
            if let Some(pid) = p.id("plan") {
                if doc.plan(pid).is_some() {
                    if let Some(l) = doc.layer_mut(lid) {
                        l.plan = pid;
                    }
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
            // T0202: Auch der letzte Layer **eines Plans** bleibt stehen —
            // ein Plan ohne Layer wäre keine Zeichenfläche mehr.
            if let Some(plan) = doc.layer(lid).map(|l| l.plan) {
                if doc.layers.iter().filter(|l| l.plan == plan).count() <= 1 {
                    return Err("Der letzte Layer eines Plans kann nicht gelöscht werden".into());
                }
            }
            // Papierraum zählt mit: ein Layer, der nur im Schriftfeld benutzt
            // wird, ist nicht leer.
            if doc.layer_objekte(lid) > 0 {
                // T0213: Die Objekte können mit `move_to` auf einen anderen
                // Layer wandern; ohne Ziel bleibt es beim Fehler (MCP, ältere
                // Aufrufer). Die Oberfläche fragt vorher nach.
                let ziel = p.v.get("move_to").cloned().unwrap_or(Value::Null);
                let zid = ziel
                    .as_u64()
                    .or_else(|| ziel.as_str().and_then(|s| doc.layer_id_by_name(s)));
                match zid {
                    Some(z) if z == lid => {
                        return Err(
                            "Ein Layer kann nicht auf sich selbst verschoben werden".into()
                        );
                    }
                    Some(z) if doc.layer(z).is_some() => {
                        doc.layer_objekte_verschieben(lid, z);
                    }
                    Some(_) => return Err("Ziellayer nicht gefunden".into()),
                    None => return Err("Layer enthält noch Objekte".into()),
                }
            }
            let plan = doc.layer(lid).map(|l| l.plan);
            doc.layers.retain(|l| l.id != lid);
            if doc.current_layer == lid {
                // Im selben Plan bleiben, sonst im ersten Layer überhaupt.
                doc.current_layer = plan
                    .and_then(|p| doc.layers.iter().find(|l| l.plan == p).map(|l| l.id))
                    .or_else(|| doc.layers.first().map(|l| l.id))
                    .unwrap_or(doc.current_layer);
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
                        "sichtbar": doc.layer_sichtbar(l.id), "gesperrt": doc.layer_gesperrt(l.id),
                        "plan": l.plan,
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
            let layer = resolve_layer_in(doc, &p, ebene);
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
            let ents: Vec<Entity> = ids.iter().filter_map(|id| doc.entity(*id).cloned()).collect();
            if ents.is_empty() {
                // Kein leerer Block als stiller Erfolg (T0038).
                return Err("ids: keine der Objektkennungen gefunden".into());
            }
            let mut bbox = BBox::EMPTY;
            for e in &ents {
                bbox.union(render::entity_bbox(doc, e));
            }
            let base = p.pt("base").unwrap_or_else(|| {
                if bbox.is_valid() {
                    bbox.center()
                } else {
                    Vec2::ZERO
                }
            });
            // Eine Quelle für die Blockdefinition — derselbe Weg entsteht aus
            // der Zwischenablage (T0142, `bloecke::definition_aus_elementen`).
            let mut block = crate::bloecke::definition_aus_elementen(
                &name,
                &p.text_or("category", "Projekt"),
                &ents,
                base,
            );
            block.description = p.text_or("description", "");
            // Größe der Blockgeometrie (T0231): feste Größe oder Papiergröße.
            // Die alten Argumente `annotative`/`einheit` gelten weiter.
            if p.boolean_or("annotative", false) {
                block.groesse = BlockGroesse::Papier;
            }
            if let Some(e) = p.text("einheit") {
                block.groesse = alt_einheit_zu_groesse(&e);
            }
            if let Some(g) = p.text("groesse").and_then(|g| BlockGroesse::aus_key(&g)) {
                block.groesse = g;
            }
            // Eigene Blöcke des Benutzers (T0128): die Kennung `usr:…` steht im
            // Inhalt, der Dateiname folgt dem Namen. Geschrieben wird, bevor das
            // Dokument geändert wird — scheitert die Datei, bleibt die Auswahl
            // unangetastet.
            let bid = block.id.clone();
            let mut datei: Option<String> = None;
            if let Some(dir) = bloecke_verzeichnis(&p) {
                let pfad = crate::bloecke::schreibe_block(&dir, &block)?;
                datei = Some(pfad.display().to_string());
            }
            doc.blocks.push(block);
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
            Ok(json!({
                "ok": true,
                "created": [inst],
                "block": e_block_id(doc.ents(), inst),
                "datei": datei
            }))
        }
        "block.update" => {
            let name = p.text("block").ok_or_else(|| "Parameter 'block' fehlt".to_string())?;
            let ids = ids_from(&p, "ids");
            let idx = doc
                .blocks
                .iter()
                .position(|b| b.id == name || b.name.eq_ignore_ascii_case(&name))
                .ok_or_else(|| format!("Block '{name}' nicht gefunden"))?;
            if doc.blocks[idx].source == BlockSource::System {
                return Err("Systemblöcke sind schreibgeschützt".into());
            }
            let base = doc.blocks[idx].base;
            let mut ents = Vec::new();
            for (i, id) in ids.iter().enumerate() {
                if let Some(e) = doc.entity(*id).cloned() {
                    let m = Affine::translate(-base);
                    let mut ne = transform::transform_entity(&e, &m);
                    ne.id = i as u64 + 1;
                    ents.push(ne);
                }
            }
            let mut neu = doc.blocks[idx].clone();
            if !ents.is_empty() {
                neu.entities = ents;
            }
            if let Some(n) = p.text("name") {
                neu.name = n.to_string();
            }
            // T0204: Name **und** Gruppe sind die Eigenschaften eines Blocks —
            // die Gruppe ist die Überschrift in der Blockbibliothek.
            if let Some(c) = p.text("category") {
                neu.category = c.to_string();
            }
            if let Some(d) = p.text("description") {
                neu.description = d.to_string();
            }
            // Größe der Blockgeometrie (T0231). `annotative`/`einheit` sind die
            // früheren Argumente derselben Angabe und bleiben gültig.
            if let Some(a) = p.boolean("annotative") {
                neu.groesse = if a {
                    BlockGroesse::Papier
                } else {
                    BlockGroesse::Fest
                };
            }
            if let Some(e) = p.text("einheit") {
                neu.groesse = alt_einheit_zu_groesse(&e);
            }
            if let Some(g) = p.text("groesse") {
                neu.groesse = BlockGroesse::aus_key(&g).ok_or_else(|| {
                    format!("Unbekannte Größe '{g}' (fest, papier)")
                })?;
            }
            // Benutzerblock (T0128): Datei nachziehen — Umbenennen ist
            // `rename` plus Feld, die Kennung bleibt gleich.
            if neu.source == BlockSource::User {
                if let Some(dir) = bloecke_verzeichnis(&p) {
                    crate::bloecke::schreibe_block(&dir, &neu)?;
                }
            }
            doc.blocks[idx] = neu;
            Ok(json!({"ok": true}))
        }
        "block.delete" => {
            let name = p.text("block").ok_or_else(|| "Parameter 'block' fehlt".to_string())?;
            // T0219: Mit `explode` werden vorher **alle** Verwendungen des
            // Blocks zerlegt — der Weg der Rückfrage in der Blockbibliothek.
            let zerlegen = p.boolean_or("explode", false);
            let ziel = doc
                .blocks
                .iter()
                .find(|b| b.id == name || b.name.eq_ignore_ascii_case(&name))
                .cloned()
                .ok_or_else(|| format!("Block '{name}' nicht gefunden"))?;
            let verwendet = doc.block_verwendungen(&ziel.id);
            if !verwendet.is_empty() {
                if !zerlegen {
                    return Err(format!(
                        "Block wird noch {}-mal verwendet",
                        verwendet.len()
                    ));
                }
                zerlege_block_instanzen(doc, &ziel.id)?;
                let rest = doc.block_verwendungen(&ziel.id);
                if !rest.is_empty() {
                    return Err(format!("Block wird noch {}-mal verwendet", rest.len()));
                }
            }
            // Benutzerblock (T0128): die Datei ist die Bibliothek — Löschen im
            // Blockmanager entfernt sie mit.
            if ziel.source == BlockSource::User {
                if let Some(dir) = bloecke_verzeichnis(&p) {
                    crate::bloecke::loesche_block(&dir, &ziel.id)?;
                }
            }
            doc.blocks.retain(|b| b.id != ziel.id);
            Ok(json!({"ok": true}))
        }
        // T0217: Kopie eines Blocks. Systemblöcke sind schreibgeschützt — ihre
        // Kopie entsteht als **Benutzerblock** im Blocksverzeichnis und ist
        // danach bearbeitbar. Kennung und Name sind neu, die Geometrie (samt
        // Basispunkt, Gruppe und Maßstabsregel) ist die der Vorlage.
        "block.copy" => {
            let name = p
                .text("block")
                .ok_or_else(|| "Parameter 'block' fehlt".to_string())?;
            let vorlage = doc
                .blocks
                .iter()
                .find(|b| b.id == name || b.name.eq_ignore_ascii_case(&name))
                .cloned()
                .ok_or_else(|| format!("Block '{name}' nicht gefunden"))?;
            let neu_name = p
                .text("name")
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| {
                    crate::bloecke::kopie_name(
                        &vorlage.name,
                        doc.blocks.iter().map(|b| b.name.as_str()),
                    )
                });
            let mut kopie = vorlage;
            kopie.id = format!("usr:{}", crate::util::short_id());
            kopie.name = neu_name;
            kopie.source = BlockSource::User;
            // Erst die Datei, dann das Dokument — scheitert die Datei, bleibt
            // die Bibliothek unverändert (wie bei `block.create`).
            let mut datei: Option<String> = None;
            if let Some(dir) = bloecke_verzeichnis(&p) {
                let pfad = crate::bloecke::schreibe_block(&dir, &kopie)?;
                datei = Some(pfad.display().to_string());
            }
            let bid = kopie.id.clone();
            let bname = kopie.name.clone();
            doc.blocks.push(kopie);
            Ok(json!({
                "ok": true,
                "block": bid,
                "name": bname,
                "datei": datei
            }))
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
                        "instances": doc.block_instances(&b.id).len(),
                        "groesse": b.groesse.key()
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
            // T0218: Häkchen im Blattmanager — Zugehörigkeit zum Plansatz.
            if let Some(v) = p.boolean("plansatz") {
                sheet.plansatz = v;
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
            viewport_rect(&p, &mut vp)?;
            viewport_center(&p, &mut vp)?;
            if let Some(s) = p.num("scale").filter(|s| *s > 0.0) {
                vp.scale = s;
            }
            vp.show_frame = p.boolean_or("show_frame", true);
            let vid = vp.id;
            let einheit = doc.input_unit();
            let info = viewport_info(einheit, &vp);
            let mut e = Entity::new(vid, 0, EntityKind::Viewport(vp));
            e.style.color = Some(Color::BLACK);
            if let Some(sh) = doc.sheet_mut(sid) {
                // Hinter allem, was schon auf dem Blatt steht.
                sh.entities.insert(0, e);
            }
            Ok(json!({"ok": true, "viewport": vid, "viewport_info": info}))
        }
        "sheet.update_viewport" => {
            let sid = p.id("sheet").unwrap_or(0);
            let vid = p.id("viewport").unwrap_or(0);
            let einheit = doc.input_unit();
            let sheet = doc.sheet_mut(sid).ok_or_else(|| "Blatt nicht gefunden".to_string())?;
            let vp = viewport_mut(sheet, vid)
                .ok_or_else(|| "Ansichtsfenster nicht gefunden".to_string())?;
            viewport_rect(&p, vp)?;
            if let Some(s) = p.num("scale").filter(|s| *s > 0.0) {
                vp.scale = s;
            }
            viewport_center(&p, vp)?;
            if let Some(f) = p.boolean("show_frame") {
                vp.show_frame = f;
            }
            let info = viewport_info(einheit, vp);
            Ok(json!({"ok": true, "viewport": vid, "viewport_info": info}))
        }
        // Ansichtsfenster auf die Zeichnung einpassen: Mittelpunkt aus der
        // Zeichnungs-BBox, Maßstab aus der Fenstergröße (T0037). Ohne diese
        // Aktion stand das Fenster auf `center: [0,0]` und schnitt eine
        // Zeichnung bei (0…13 m) am Blattrand ab.
        "sheet.fit_viewport" => {
            let sid = p.id("sheet").unwrap_or(0);
            let vid = p.id("viewport");
            let bbox = doc.bbox();
            if !bbox.is_valid() || bbox.width() <= 0.0 || bbox.height() <= 0.0 {
                return Err(
                    "Die Zeichnung ist leer — eine Ansicht lässt sich nicht einpassen".into(),
                );
            }
            // Rand in Papier-Millimetern; nicht in `params.unit`, weil er zum
            // Blatt gehört und nicht zur Zeichnung.
            let margin = p.num("margin_mm").unwrap_or(10.0).clamp(0.0, 100.0);
            let scale_vorgabe = p.num("scale").filter(|s| *s > 0.0);
            let einheit = doc.input_unit();
            let sheet = doc.sheet_mut(sid).ok_or_else(|| "Blatt nicht gefunden".to_string())?;
            let vp = match vid {
                Some(vid) => viewport_mut(sheet, vid)
                    .ok_or_else(|| "Ansichtsfenster nicht gefunden".to_string())?,
                None => erster_viewport_mut(sheet).ok_or_else(|| {
                    "Blatt hat kein Ansichtsfenster — erst 'sheet.add_viewport'".to_string()
                })?,
            };
            if vp.w - 2.0 * margin <= 1.0 || vp.h - 2.0 * margin <= 1.0 {
                return Err(format!(
                    "Ansichtsfenster {} × {} mm ist zu klein für einen Rand von {margin} mm",
                    vp.w, vp.h
                ));
            }
            let (scale, center, warnung) = fit_werte(&bbox, vp.w, vp.h, margin);
            vp.scale = scale_vorgabe.unwrap_or(scale);
            vp.center = center;
            let info = viewport_info(einheit, vp);
            // Der gezeichnete Bereich in Papier-Millimetern — damit der Aufrufer
            // ohne Nachrechnen sieht, ob die Zeichnung ins Fenster passt.
            let mut out = json!({
                "ok": true,
                "viewport": vp.id,
                "scale": vp.scale,
                "bbox_mm": [bbox.min.to_array(), bbox.max.to_array()],
                "gezeichnet_mm": [bbox.width() / vp.scale, bbox.height() / vp.scale],
                "viewport_info": info,
            });
            if let Some(w) = warnung {
                out["warnung"] = json!(w);
            }
            Ok(out)
        }
        "sheet.remove_viewport" => {
            let sid = p.id("sheet").unwrap_or(0);
            let vid = p.id("viewport").unwrap_or(0);
            if let Some(sh) = doc.sheet_mut(sid) {
                sh.entities.retain(|e| e.id != vid);
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
            // T0228: eigener Papierraum-Eingabeeinheit, unabhängig vom Modell.
            if let Some(u) = p.text("paper_input_unit") {
                if let Some(u) = Unit::from_str_loose(&u) {
                    doc.settings.paper_input_unit = u;
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
            // T0144: Rasterart, Modus und erzwungener Rasterfang. Unbekannte
            // Schlüssel werden abgewiesen (kein stiller Rückfall auf die Vorgabe).
            if let Some(k) = p.text("grid_kind") {
                if let Some(art) = crate::gitter::Rasterart::from_key(&k) {
                    doc.settings.snap.grid_kind = art;
                }
            }
            if let Some(m) = p.text("grid_mode") {
                if let Some(modus) = crate::gitter::Rastermodus::from_key(&m) {
                    doc.settings.snap.grid_mode = modus;
                }
            }
            if let Some(v) = p.boolean("grid_force") {
                doc.settings.snap.grid_force = v;
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
                    ("tangent", &mut doc.settings.snap.tangent),
                    ("lot", &mut doc.settings.snap.lot),
                    ("nearest", &mut doc.settings.snap.nearest),
                    ("grid", &mut doc.settings.snap.grid),
                    ("ortho", &mut doc.settings.snap.ortho),
                    ("polar", &mut doc.settings.snap.polar),
                    ("wall_edges", &mut doc.settings.snap.wall_edges),
                    ("quadrant", &mut doc.settings.snap.quadrant),
                    ("bisector", &mut doc.settings.snap.bisector),
                    ("hilfslinien", &mut doc.settings.snap.hilfslinien),
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
                if let Some(k) = sp.text("grid_kind") {
                    if let Some(art) = crate::gitter::Rasterart::from_key(&k) {
                        doc.settings.snap.grid_kind = art;
                    }
                }
                if let Some(m) = sp.text("grid_mode") {
                    if let Some(modus) = crate::gitter::Rastermodus::from_key(&m) {
                        doc.settings.snap.grid_mode = modus;
                    }
                }
                if let Some(v) = sp.boolean("grid_force") {
                    doc.settings.snap.grid_force = v;
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
                // Erweiterter Punktfang (T0160): Verweildauer und Zahl der
                // Haltepunkte. Grenzen stehen im Modell, damit Feld, Action und
                // Datei dieselben Werte kennen.
                if let Some(v) = sp.num("verweil_ms") {
                    doc.settings.snap.verweil_ms =
                        v.clamp(crate::model::VERWEIL_MIN_MS, crate::model::VERWEIL_MAX_MS);
                }
                if let Some(h) = sp.num("halte_max") {
                    doc.settings.snap.halte_max =
                        (h.round().max(0.0) as usize).min(crate::model::HALTE_MAX);
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
                .filter(|e| !doc.layer_gesperrt(e.layer))
                .map(|e| e.id)
                .collect();
            Ok(json!({"ok": true, "ids": ids}))
        }
        "select.query" => {
            let ids = query_ids(doc, params);
            Ok(json!({"ok": true, "ids": ids, "count": ids.len()}))
        }
        // Flächen einer Auswahl: je Objekt und als Summe (Raumgruppen).
        // Dieselben Filter wie `select.query` — ein Weg (`query_ids`). Objekte
        // ohne Fläche stehen in `without_area`, sie bekommen keine 0.
        "select.area" => {
            let ids = query_ids(doc, params);
            let mut sum = 0.0f64;
            let mut areas = Vec::new();
            let mut without = Vec::new();
            for id in ids {
                let Some(e) = doc.ents().iter().find(|e| e.id == id) else {
                    continue;
                };
                match crate::area::entity_area(e) {
                    Some(mm2) => {
                        sum += mm2;
                        areas.push(json!({
                            "id": id,
                            "area_m2": crate::units::format_area_m2(mm2),
                            "area_mm2": mm2
                        }));
                    }
                    None => without.push(id),
                }
            }
            Ok(json!({
                "ok": true,
                "count": areas.len(),
                "sum_m2": crate::units::format_area_m2(sum),
                "sum_mm2": sum,
                "areas": areas,
                "without_area": without
            }))
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
    let p = Params::new(params, doc.input_unit());
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

/// Parallele zu einer Linie oder Polylinie im Abstand `dist` auf der Seite von
/// `side` (T0200).
///
/// Die eine Rechnung für Werkzeug-Vorschau und Action: die Vorschau zeichnet
/// genau das Objekt, das der Klick danach anlegt. `side` ist ein Punkt — der
/// Mauszeiger; er bestimmt nur die **Seite**, nicht den Abstand.
///
/// Ein anderes Objekt ergibt `None` (kein stiller Erfolg, T0038): eine Parallele
/// gibt es nur zu Linie und Polylinie.
pub fn parallel_kind(e: &Entity, dist: f64, side: Vec2) -> Option<EntityKind> {
    match &e.kind {
        EntityKind::Line { a, b } => {
            let d = *b - *a;
            if d.len() <= POINT_EPS_MM {
                return None;
            }
            let n = d.normalized().perp();
            let sign = if (side - *a).dot(n) >= 0.0 { 1.0 } else { -1.0 };
            let off = n * (dist * sign);
            Some(EntityKind::Line {
                a: *a + off,
                b: *b + off,
            })
        }
        EntityKind::Polyline { verts, closed } => {
            let pts: Vec<Vec2> = verts.iter().map(|q| q.p).collect();
            if pts.len() < 2 {
                return None;
            }
            let sign = polyline_side(&pts, side);
            let off = offset_polyline(&pts, *closed, dist * sign);
            Some(EntityKind::Polyline {
                verts: off.into_iter().map(Vertex::new).collect(),
                closed: *closed,
            })
        }
        _ => None,
    }
}

/// Auf welcher Seite einer Polylinie liegt `side`?
///
/// Maßgeblich ist die **nächste** Strecke; ihre Normale (links der
/// Laufrichtung) gibt das Vorzeichen. Ein Punkt in einem geschlossenen Zug
/// liegt damit nicht pauschal „innen", sondern auf der Seite, auf der er
/// tatsächlich steht — bei einer L-Form wäre „innen" für den einen Schenkel
/// falsch.
fn polyline_side(pts: &[Vec2], side: Vec2) -> f64 {
    let mut best: Option<(f64, f64)> = None;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = b - a;
        if d.len() <= POINT_EPS_MM {
            continue;
        }
        let n = d.normalized().perp();
        let abstand = dist_point_segment(side, a, b);
        if best.map(|(bd, _)| abstand < bd).unwrap_or(true) {
            let sign = if (side - a).dot(n) >= 0.0 { 1.0 } else { -1.0 };
            best = Some((abstand, sign));
        }
    }
    best.map(|(_, s)| s).unwrap_or(1.0)
}

/// Abstand des Punktes `p` zur Linie bzw. Polylinie `e` — der Wert, den das
/// Werkzeug „Parallele" beim ersten Klick übernimmt.
///
/// Gemessen wird **senkrecht** zur nächsten Strecke (nicht der Abstand zum
/// nächsten Punkt): der Abstand einer Parallelen ist überall derselbe, sonst
/// hinge er davon ab, wo entlang der Linie der Zeiger gerade steht.
pub fn parallel_abstand(e: &Entity, p: Vec2) -> Option<f64> {
    match &e.kind {
        EntityKind::Line { a, b } => Some(dist_point_line(p, *a, *b)),
        EntityKind::Polyline { verts, .. } => {
            let pts: Vec<Vec2> = verts.iter().map(|q| q.p).collect();
            let mut best: Option<(f64, f64)> = None;
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                if a.dist(b) <= POINT_EPS_MM {
                    continue;
                }
                let abstand = dist_point_segment(p, a, b);
                if best.map(|(bd, _)| abstand < bd).unwrap_or(true) {
                    best = Some((abstand, dist_point_line(p, a, b)));
                }
            }
            best.map(|(_, d)| d)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Stutzen / Verlängern
// ---------------------------------------------------------------------------

fn cutter_segments(doc: &Document, exclude: &[Id]) -> Vec<(Seg, Id)> {
    cutter_segments_mit(doc, exclude, false)
}

/// Schnittkanten fürs **Verlängern**: Wände mit ihrer **Umrandung**, nicht mit
/// der Basislinie (T0207). Eine bis zur Achse verlängerte Linie steckte sonst
/// im Wandkörper, obwohl die sichtbare Kante davor liegt.
///
/// Fürs **Stutzen** bleibt es bei der Achse: dort endet die Linie *im* Wandton
/// (verdeckt) und der Anschluss liegt auf der Bezugslinie — so steht es in
/// `snap_wandfang::stutzen_nutzt_weiter_die_wandachse`.
fn cutter_segments_umrandung(doc: &Document, exclude: &[Id]) -> Vec<(Seg, Id)> {
    cutter_segments_mit(doc, exclude, true)
}

fn cutter_segments_mit(doc: &Document, exclude: &[Id], wandkante: bool) -> Vec<(Seg, Id)> {
    let mut out = Vec::new();
    // Der Index wird einmal gebaut — die Kontur braucht die Nachbarschaft
    // (Gehrung, Anschlüsse).
    let idx = if wandkante {
        Some(render::WallIndex::build(doc, doc.ents()))
    } else {
        None
    };
    for e in doc.ents() {
        if exclude.contains(&e.id) {
            continue;
        }
        if !doc.layer_sichtbar(e.layer) { continue; }
        if let (Some(idx), true) = (&idx, matches!(e.kind, EntityKind::Wall { .. })) {
            for s in render::wall_contour_segments_idx(idx, e) {
                out.push((s, e.id));
            }
            continue;
        }
        for s in render::entity_segments(doc, e) {
            out.push((s, e.id));
        }
    }
    out
}

/// Der Abschnitt der Strecke `a`→`b`, den ein Klick bei `at` entfernt: der
/// Bereich zwischen den beiden Schnittkanten, in dem `at` liegt — als
/// Parameter `(t0, t1)` mit 0 = `a` und 1 = `b`.
///
/// Eine Quelle für [`trim_entity`] (schneidet) und [`trim_vorschau`] (zeigt
/// vorher, was wegfällt) — sonst liefen Anzeige und Schnitt auseinander.
fn trim_span(a: Vec2, b: Vec2, at: Vec2, segs: &[(Seg, Id)]) -> Option<(f64, f64)> {
    let len = a.dist(b);
    if len < 1e-6 {
        return None;
    }
    let dir = (b - a).normalized();
    let mut ts = vec![0.0, 1.0];
    for (sg, _) in segs {
        match sg {
            Seg::Line(c, d) => {
                if let Some(x) = line_line(a, b, *c, *d, false, false) {
                    let t = (x - a).dot(dir) / len;
                    if t > 1e-6 && t < 1.0 - 1e-6 {
                        ts.push(t);
                    }
                }
            }
            Seg::Arc(arc) => {
                for x in line_circle(a, b, arc.c, arc.r) {
                    if angle_in_arc((x - arc.c).angle(), arc.a0, arc.a1) {
                        let t = (x - a).dot(dir) / len;
                        if t > 1e-6 && t < 1.0 - 1e-6 {
                            ts.push(t);
                        }
                    }
                }
            }
        }
    }
    ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    let tclick = (at - a).dot(dir) / len;
    ts.windows(2)
        .find(|w| tclick >= w[0] - 1e-9 && tclick <= w[1] + 1e-9)
        .map(|w| (w[0], w[1]))
}

/// Winkel aller Schnittpunkte eines Kreises mit den Schnittkanten.
///
/// Gerade Kanten schneiden über [`line_circle`], **Bögen und Kreise** über
/// [`circle_circle`] — mit Prüfung, ob der Punkt auf dem schneidenden Bogen
/// liegt. Vorher wurden nur Geraden betrachtet: ein Kreis ließ sich deshalb
/// nicht von einem anderen Kreis oder Bogen schneiden, obwohl die Schnittpunkte
/// sichtbar auf der Kurve lagen (T0084, Turn 2).
fn kreis_schnittwinkel(c: Vec2, r: f64, segs: &[(Seg, Id)]) -> Vec<f64> {
    let mut hits: Vec<f64> = Vec::new();
    for (sg, _) in segs {
        match sg {
            Seg::Line(a, b) => {
                for x in line_circle(*a, *b, c, r) {
                    hits.push((x - c).angle());
                }
            }
            Seg::Arc(arc) => {
                for x in circle_circle(c, r, arc.c, arc.r) {
                    if angle_in_arc((x - arc.c).angle(), arc.a0, arc.a1) {
                        hits.push((x - c).angle());
                    }
                }
            }
        }
    }
    for a in hits.iter_mut() {
        *a = norm_angle(*a);
    }
    hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    hits.dedup_by(|a, b| winkel_abstand(*a, *b) < 1e-7);
    // 0 und 2π sind derselbe Punkt — der kleinere bleibt.
    if hits.len() > 1 {
        let first = hits[0];
        let last = hits[hits.len() - 1];
        if winkel_abstand(first, last) < 1e-7 {
            hits.pop();
        }
    }
    hits
}

/// Die Abschnitte eines **Kreises** beim Stutzen an der Stelle `at`: was stehen
/// bleibt (a0, a1 je gegen den Uhrzeigersinn) und der Abschnitt, der wegfällt.
///
/// `None`, wenn weniger als zwei Schnittkanten den Kreis teilen.
///
/// Stehen bleibt **ein** Bogen: der Gegenbogen zum angeklickten Abschnitt. Die
/// übrigen Schnittstellen zerlegen ihn nicht (T0118) — gestutzt wird nur die
/// Stelle unter dem Zeiger. Vorher fiel der Kreis an *jeder* Schnittstelle
/// auseinander: ein Kreis mit vier Schnittpunkten ergab nach einem Klick drei
/// Einzelbögen, obwohl nur ein Abschnitt wegfiel.
fn kreis_trim_stuecke(
    c: Vec2,
    r: f64,
    at: Vec2,
    segs: &[(Seg, Id)],
) -> Option<(Vec<(f64, f64)>, (f64, f64))> {
    let hits = kreis_schnittwinkel(c, r, segs);
    if hits.len() < 2 {
        return None;
    }
    let n = hits.len();
    let a = norm_angle((at - c).angle());
    let stuecke: Vec<(f64, f64)> = (0..n).map(|i| (hits[i], hits[(i + 1) % n])).collect();
    let idx = stuecke
        .iter()
        .position(|(a0, a1)| angle_in_arc(a, *a0, *a1))?;
    let weg = stuecke[idx];
    Some((vec![(weg.1, weg.0)], weg))
}

/// Die Abschnitte eines **Bogens** beim Stutzen an der Stelle `at`.
///
/// Die Schnitte werden in Laufrichtung des Bogens (0 = Anfang) einsortiert;
/// der angeklickte Abschnitt fällt weg. Stehen bleiben nur die Stücke **vor**
/// und **nach** dem Schnitt — je ein Bogen, nicht an den übrigen Schnittstellen
/// zerlegt (T0118, wie beim Kreis).
fn bogen_trim_stuecke(
    c: Vec2,
    r: f64,
    a0: f64,
    a1: f64,
    at: Vec2,
    segs: &[(Seg, Id)],
) -> Option<(Vec<(f64, f64)>, (f64, f64))> {
    let sweep = Arc::new(c, r, a0, a1).sweep();
    if sweep >= std::f64::consts::TAU - 1e-9 {
        return kreis_trim_stuecke(c, r, at, segs);
    }
    let mut innen: Vec<f64> = kreis_schnittwinkel(c, r, segs)
        .into_iter()
        .map(|u| norm_angle(u - a0))
        .filter(|t| *t > 1e-7 && *t < sweep - 1e-7)
        .collect();
    innen.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    innen.dedup_by(|x, y| (*x - *y).abs() < 1e-7);
    if innen.is_empty() {
        return None;
    }
    let mut grenzen = vec![0.0];
    grenzen.extend(innen);
    grenzen.push(sweep);
    let t = norm_angle((at - c).angle() - a0);
    let mut stuecke: Vec<(f64, f64)> = grenzen.windows(2).map(|w| (w[0], w[1])).collect();
    let idx = stuecke
        .iter()
        .position(|(s, e)| t >= *s - 1e-9 && t <= *e + 1e-9)?;
    let weg = stuecke.remove(idx);
    // Nur die Stücke **vor** und **nach** dem Schnitt bleiben stehen — je ein
    // Bogen. Die übrigen Schnittstellen zerlegen sie nicht (T0118).
    let mut bleibt: Vec<(f64, f64)> = Vec::new();
    if weg.0 > 1e-7 {
        bleibt.push((norm_angle(a0), norm_angle(a0 + weg.0)));
    }
    if weg.1 < sweep - 1e-7 {
        bleibt.push((norm_angle(a0 + weg.1), norm_angle(a0 + sweep)));
    }
    Some((bleibt, (norm_angle(a0 + weg.0), norm_angle(a0 + weg.1))))
}

/// Parameterwinkel aller Schnittpunkte einer Ellipse mit den Schnittkanten.
///
/// Gerade Kanten schneiden exakt (quadratische Gleichung im normierten
/// Ellipsenraum, [`Ellipse::line_hits`]). Ein Bogen ist keine Gerade: er wird in
/// kurze Sehnen zerlegt, sonst gäbe es keine geschlossene Lösung.
fn ellipse_schnittwinkel(el: &Ellipse, segs: &[(Seg, Id)]) -> Vec<f64> {
    let mut hits: Vec<f64> = Vec::new();
    for (sg, _) in segs {
        let punkte: Vec<f64> = match sg {
            Seg::Line(a, b) => el.line_hits(*a, *b).into_iter().map(|(u, _)| u).collect(),
            Seg::Arc(arc) => {
                let tol = (arc.r * 1e-4).max(0.01);
                let pts = arc.flatten(tol);
                pts.windows(2)
                    .flat_map(|w| el.line_hits(w[0], w[1]))
                    .map(|(u, _)| u)
                    .collect()
            }
        };
        hits.extend(punkte);
    }
    for u in hits.iter_mut() {
        *u = norm_angle(*u);
    }
    hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    hits.dedup_by(|a, b| winkel_abstand(*a, *b) < 1e-7);
    // 0 und 2π sind derselbe Punkt — der kleinere bleibt.
    if hits.len() > 1 {
        let first = hits[0];
        let last = hits[hits.len() - 1];
        if winkel_abstand(first, last) < 1e-7 {
            hits.pop();
        }
    }
    hits
}

/// Abstand zweier Parameterwinkel auf dem Kreis (0…π), unabhängig vom Umlauf.
fn winkel_abstand(a: f64, b: f64) -> f64 {
    let d = norm_angle(a - b);
    d.min(std::f64::consts::TAU - d)
}

/// Die Abschnitte einer **geschlossenen** Ellipse beim Stutzen an der Stelle
/// `at`: was stehen bleibt (a0, a1 je gegen den Uhrzeigersinn) und der
/// Abschnitt, der wegfällt.
///
/// `None`, wenn weniger als zwei Schnittkanten die Ellipse teilen — dann gibt es
/// keinen Bereich, den ein Klick bestimmt.
///
/// Stehen bleibt **ein** Ellipsenbogen: der Gegenbogen zum angeklickten
/// Abschnitt. Die übrigen Schnittstellen zerlegen ihn nicht (T0118, wie beim
/// Kreis).
fn ellipse_trim_stuecke(
    el: &Ellipse,
    at: Vec2,
    segs: &[(Seg, Id)],
) -> Option<(Vec<(f64, f64)>, (f64, f64))> {
    let hits = ellipse_schnittwinkel(el, segs);
    if hits.len() < 2 {
        return None;
    }
    let n = hits.len();
    let u = norm_angle(el.param_of(at));
    let stuecke: Vec<(f64, f64)> = (0..n).map(|i| (hits[i], hits[(i + 1) % n])).collect();
    let idx = stuecke
        .iter()
        .position(|(a0, a1)| angle_in_arc(u, *a0, *a1))?;
    let weg = stuecke[idx];
    Some((vec![(weg.1, weg.0)], weg))
}

/// Die Abschnitte eines **Ellipsenbogens** beim Stutzen an der Stelle `at`.
///
/// Die Schnitte werden in Laufrichtung des Bogens (0 = Anfang) einsortiert;
/// der angeklickte Abschnitt fällt weg, die übrigen bleiben als eigene
/// Ellipsenbögen stehen (T0084).
fn ellipsearc_trim_stuecke(
    el: &Ellipse,
    a0: f64,
    a1: f64,
    at: Vec2,
    segs: &[(Seg, Id)],
) -> Option<(Vec<(f64, f64)>, (f64, f64))> {
    let sweep = el.sweep(a0, a1);
    if sweep >= std::f64::consts::TAU - 1e-9 {
        return ellipse_trim_stuecke(el, at, segs);
    }
    let mut innen: Vec<f64> = ellipse_schnittwinkel(el, segs)
        .into_iter()
        .map(|u| norm_angle(u - a0))
        .filter(|t| *t > 1e-7 && *t < sweep - 1e-7)
        .collect();
    innen.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    innen.dedup_by(|x, y| (*x - *y).abs() < 1e-7);
    if innen.is_empty() {
        return None;
    }
    let mut grenzen = vec![0.0];
    grenzen.extend(innen);
    grenzen.push(sweep);
    let t = norm_angle(el.param_of(at) - a0);
    let mut stuecke: Vec<(f64, f64)> = grenzen.windows(2).map(|w| (w[0], w[1])).collect();
    let idx = stuecke
        .iter()
        .position(|(s, e)| t >= *s - 1e-9 && t <= *e + 1e-9)?;
    let weg = stuecke.remove(idx);
    // Nur die Stücke **vor** und **nach** dem Schnitt bleiben stehen — je ein
    // Bogen. Die übrigen Schnittstellen zerlegen sie nicht (T0118).
    let mut bleibt: Vec<(f64, f64)> = Vec::new();
    if weg.0 > 1e-7 {
        bleibt.push((norm_angle(a0), norm_angle(a0 + weg.0)));
    }
    if weg.1 < sweep - 1e-7 {
        bleibt.push((norm_angle(a0 + weg.1), norm_angle(a0 + sweep)));
    }
    Some((bleibt, (norm_angle(a0 + weg.0), norm_angle(a0 + weg.1))))
}

/// Die Strecken, die von einem Ellipsen-Abschnitt wegfällt — für die Vorschau
/// des Stutzens (kurze Sehnen, dieselbe Kurve wie die Anzeige).
fn ellipse_sehnen(el: &Ellipse, a0: f64, a1: f64) -> Vec<Seg> {
    let pts = el.arc_points(a0, a1, 0.1);
    pts.windows(2).map(|w| Seg::Line(w[0], w[1])).collect()
}

/// Punkt auf einem Polylinien-Segment beim Parameter `t` (0 = Segmentanfang
/// `p0`, 1 = Segmentende `p1`) in **Laufrichtung** der Polylinie.
fn polyline_punkt(p0: Vec2, p1: Vec2, bulge: f64, t: f64) -> Vec2 {
    if bulge.abs() < 1e-12 {
        return p0.lerp(p1, t);
    }
    let Some(arc) = arc_from_bulge(p0, p1, bulge) else {
        return p0.lerp(p1, t);
    };
    // Bei negativem `bulge` läuft der gespeicherte Bogen andersherum als die
    // Polylinie; die Lage des Anfangspunkts entscheidet die Richtung.
    let vorwaerts = arc.start().dist(p0) <= arc.end().dist(p0);
    arc.point_at(if vorwaerts { t } else { 1.0 - t })
}

/// Krümmung des Teilstücks zwischen zwei Parametern desselben Segments: der
/// eingeschlossene Winkel wächst linear mit dem Parameter, also
/// `tan(Δt · atan(bulge))` — bei `Δt = 1` kommt wieder der volle `bulge` heraus.
fn polyline_teil_bulge(bulge: f64, von: f64, bis: f64) -> f64 {
    if bulge.abs() < 1e-12 {
        return 0.0;
    }
    ((bis - von) * bulge.atan()).tan()
}

/// Parameter (0…1 in Laufrichtung) eines Punktes, der auf dem Bogen liegt.
fn polyline_param(p0: Vec2, arc: &Arc, p: Vec2) -> f64 {
    let t = norm_angle((p - arc.c).angle() - arc.a0) / arc.sweep();
    let vorwaerts = arc.start().dist(p0) <= arc.end().dist(p0);
    if vorwaerts {
        t
    } else {
        1.0 - t
    }
}

/// Der Abschnitt eines Polylinien-Segments, den ein Klick bei `at` entfernt —
/// Parameter `(t0, t1)` in Laufrichtung (0 = Segmentanfang, 1 = Segmentende).
///
/// Gerade Segmente laufen über [`trim_span`]; bei einem Bogen zählen die
/// Schnittpunkte mit dem **Bogen**, nicht mit der Sehne.
fn polyline_seg_span(
    p0: Vec2,
    p1: Vec2,
    bulge: f64,
    at: Vec2,
    cutters: &[(Seg, Id)],
) -> Option<(f64, f64)> {
    if bulge.abs() < 1e-12 {
        return trim_span(p0, p1, at, cutters);
    }
    let arc = arc_from_bulge(p0, p1, bulge)?;
    if arc.r < 1e-9 || arc.sweep() < 1e-9 {
        return trim_span(p0, p1, at, cutters);
    }
    let seg = Seg::Arc(arc);
    let mut ts = vec![0.0, 1.0];
    for (sg, _) in cutters {
        for x in seg_seg(&seg, sg) {
            let t = polyline_param(p0, &arc, x);
            if t > 1e-6 && t < 1.0 - 1e-6 {
                ts.push(t);
            }
        }
    }
    ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    let tc = polyline_param(p0, &arc, at);
    ts.windows(2)
        .find(|w| tc >= w[0] - 1e-9 && tc <= w[1] + 1e-9)
        .map(|w| (w[0], w[1]))
}

/// Welches Segment einer Polylinie der Klick trifft und welchen Abschnitt
/// (Parameter 0…1 in Laufrichtung) er darin entfernt.
///
/// Schneidet sich die Polylinie **selbst**, sind ihre übrigen Segmente
/// ebenfalls Schnittkanten — sonst hätte ein Klick auf dem kreuzenden Schenkel
/// den ganzen Schenkel entfernt statt bis zum Kreuzungspunkt zu stutzen
/// (T0077, Turn 2).
fn polyline_trim(
    verts: &[Vertex],
    closed: bool,
    id: Id,
    at: Vec2,
    cutters: &[(Seg, Id)],
) -> Option<(usize, f64, f64)> {
    if verts.len() < 2 {
        return None;
    }
    let pl = Polyline {
        verts: verts.to_vec(),
        closed,
    };
    let segs = pl.segments();
    let mut best: Option<(usize, f64)> = None;
    for (i, s) in segs.iter().enumerate() {
        let d = s.closest(at).1;
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((i, d));
        }
    }
    let (i, _) = best?;
    // Die eigenen Segmente als Schnittkanten — außer dem angeklickten selbst
    // (es liegt unter dem Zeiger und schnitte sich nicht sinnvoll).
    let mut schnitte: Vec<(Seg, Id)> = cutters.to_vec();
    for (j, sg) in segs.iter().enumerate() {
        if j != i {
            schnitte.push((sg.clone(), id));
        }
    }
    let p0 = verts[i].p;
    let p1 = verts[(i + 1) % verts.len()].p;
    let (t0, t1) = polyline_seg_span(p0, p1, verts[i].bulge, at, &schnitte)?;
    Some((i, t0, t1))
}

/// Die Stücke, die von einer Polylinie übrig bleiben, wenn im Segment `i` der
/// Abschnitt `(t0, t1)` wegfällt. Alle Stücke sind **offen** — ein
/// geschlossener Zug wird durch den Schnitt aufgetrennt.
fn polyline_rest(verts: &[Vertex], closed: bool, i: usize, t0: f64, t1: f64) -> Vec<Vec<Vertex>> {
    let n = verts.len();
    if n < 2 || i >= n {
        return Vec::new();
    }
    let b = verts[i].bulge;
    let p0 = verts[i].p;
    let p1 = verts[(i + 1) % n].p;
    let mut stuecke: Vec<Vec<Vertex>> = Vec::new();
    if closed {
        // Ein Zug vom Schnittende einmal herum bis zum Schnittanfang.
        let mut v: Vec<Vertex> = Vec::new();
        if t1 < 1.0 - 1e-9 {
            let mut start = Vertex::new(polyline_punkt(p0, p1, b, t1));
            start.bulge = polyline_teil_bulge(b, t1, 1.0);
            v.push(start);
        }
        for k in 0..n {
            v.push(verts[(i + 1 + k) % n]);
        }
        if t0 > 1e-9 {
            let mut last = v.pop().expect("Zug hat mindestens einen Punkt");
            last.bulge = polyline_teil_bulge(b, 0.0, t0);
            v.push(last);
            v.push(Vertex::new(polyline_punkt(p0, p1, b, t0)));
        }
        stuecke.push(v);
    } else {
        let mut links: Vec<Vertex> = verts[..=i].to_vec();
        if t0 > 1e-9 {
            let mut last = links.pop().expect("Segment hat einen Anfangspunkt");
            last.bulge = polyline_teil_bulge(b, 0.0, t0);
            links.push(last);
            links.push(Vertex::new(polyline_punkt(p0, p1, b, t0)));
        }
        stuecke.push(links);
        let mut rechts: Vec<Vertex> = Vec::new();
        if t1 < 1.0 - 1e-9 {
            let mut first = Vertex::new(polyline_punkt(p0, p1, b, t1));
            first.bulge = polyline_teil_bulge(b, t1, 1.0);
            rechts.push(first);
        }
        rechts.extend_from_slice(&verts[i + 1..]);
        stuecke.push(rechts);
    }
    stuecke.retain(|v| v.len() >= 2);
    stuecke
}

/// Objektart für eine Punktfolge: eine Polylinie aus **nur einem Element**
/// (zwei Punkte, ein Segment) ist keine Polylinie, sondern eine Linie (bzw.
/// ein Bogen, wenn das Segment einen Stützbogen trägt). Ab drei Punkten bleibt
/// es ein Polygonzug.
///
/// Dieselbe Regel gilt beim Zeichnen (`draw.polyline`, T0149) und beim Stutzen
/// (`modify.trim`) — dort sind die übrig bleibenden Stücke immer **offen**.
fn polyline_kind(verts: &[Vertex], closed: bool) -> Option<EntityKind> {
    if verts.len() < 2 {
        return None;
    }
    if verts.len() == 2 {
        let a = verts[0];
        let b = verts[1];
        if a.p.dist(b.p) < 1e-9 {
            return None;
        }
        if a.bulge.abs() < 1e-12 {
            return Some(EntityKind::Line { a: a.p, b: b.p });
        }
        if let Some(arc) = arc_from_bulge(a.p, b.p, a.bulge) {
            return Some(EntityKind::Arc {
                c: arc.c,
                r: arc.r,
                a0: arc.a0,
                a1: arc.a1,
            });
        }
    }
    Some(EntityKind::Polyline {
        verts: verts.to_vec(),
        closed,
    })
}

/// Was `modify.trim` an der Stelle `at` von `id` entfernen würde — **ohne**
/// die Zeichnung zu ändern. Grundlage der Hervorhebung in der Zeichenfläche
/// (T0075), damit der Nutzer vor dem Klick sieht, welcher Abschnitt wegfällt.
///
/// Leer, wenn das Objekt nicht gestutzt werden kann oder keine Schnittkante
/// den Klickbereich begrenzt.
pub fn trim_vorschau(doc: &Document, id: Id, at: Vec2, cutters: &[Id]) -> Vec<Seg> {
    let Some(e) = doc.entity(id) else {
        return Vec::new();
    };
    // Dieselbe Auswahl der Schnittkanten wie in `trim_entity`: ohne `cutters`
    // ist jedes andere sichtbare Objekt eine Schnittkante.
    let exclude: Vec<Id> = if cutters.is_empty() {
        vec![id]
    } else {
        let mut v = vec![id];
        v.extend_from_slice(cutters);
        v
    };
    let segs = cutter_segments(doc, &exclude);
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
            let Some((t0, t1)) = trim_span(*a, *b, at, &segs) else {
                return Vec::new();
            };
            vec![Seg::Line(a.lerp(*b, t0), a.lerp(*b, t1))]
        }
        EntityKind::Circle { c, r } => {
            let Some((_, weg)) = kreis_trim_stuecke(*c, *r, at, &segs) else {
                return Vec::new();
            };
            vec![Seg::Arc(Arc::new(*c, *r, weg.0, weg.1))]
        }
        // Bogen: derselbe Weg wie beim Kreis, nur auf den vorhandenen Abschnitt
        // begrenzt (T0084, Turn 2).
        EntityKind::Arc { c, r, a0, a1 } => {
            let Some((_, weg)) = bogen_trim_stuecke(*c, *r, *a0, *a1, at, &segs) else {
                return Vec::new();
            };
            vec![Seg::Arc(Arc::new(*c, *r, weg.0, weg.1))]
        }
        // Ellipse: entfernt wird der angeklickte Abschnitt (T0084).
        EntityKind::Ellipse { c, rx, ry, rot } => {
            let el = Ellipse::new(*c, *rx, *ry, *rot);
            let Some((_, weg)) = ellipse_trim_stuecke(&el, at, &segs) else {
                return Vec::new();
            };
            ellipse_sehnen(&el, weg.0, weg.1)
        }
        EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = Ellipse::new(*c, *rx, *ry, *rot);
            let Some((_, weg)) = ellipsearc_trim_stuecke(&el, *a0, *a1, at, &segs) else {
                return Vec::new();
            };
            ellipse_sehnen(&el, weg.0, weg.1)
        }
        EntityKind::Polyline { verts, closed } => {
            let Some((i, t0, t1)) = polyline_trim(verts, *closed, id, at, &segs) else {
                return Vec::new();
            };
            let b = verts[i].bulge;
            let p0 = verts[i].p;
            let p1 = verts[(i + 1) % verts.len()].p;
            let von = polyline_punkt(p0, p1, b, t0);
            let bis = polyline_punkt(p0, p1, b, t1);
            let teil = polyline_teil_bulge(b, t0, t1);
            match arc_from_bulge(von, bis, teil) {
                Some(arc) => vec![Seg::Arc(arc)],
                None => vec![Seg::Line(von, bis)],
            }
        }
        _ => Vec::new(),
    }
}

/// Die Strecke, die `modify.extend` an der Stelle `at` an das Objekt ansetzen
/// würde — **ohne** die Zeichnung zu ändern. Grundlage der Hervorhebung in der
/// Zeichenfläche (T0081), damit der Nutzer vor dem Klick sieht, wohin
/// verlängert wird.
///
/// Leer, wenn das Objekt nicht verlängert werden kann oder keine Schnittkante
/// hinter dem Ende liegt.
pub fn extend_vorschau(doc: &Document, id: Id, at: Vec2, cutters: &[Id]) -> Vec<Seg> {
    match verlaengerung(doc, id, at, cutters) {
        // Das angesetzte Stück: eine Strecke oder — bei einem Bogen-Endsegment
        // einer Polylinie — der weiterlaufende Bogen (T0155).
        Ok(v) => vec![v.weg],
        Err(_) => Vec::new(),
    }
}

/// Übernimmt die beim Stutzen eines Kreises oder Bogens bleibenden Abschnitte:
/// der erste bleibt am Objekt (Id, Ebene, Stil), weitere werden neue Objekte
/// und ordnen sich nach ihrem Zeichenrang ein. Ohne Stück verschwindet das
/// Objekt.
fn kreis_stuecke_uebernehmen(
    doc: &mut Document,
    id: Id,
    e: &Entity,
    c: Vec2,
    r: f64,
    stuecke: Vec<(f64, f64)>,
) -> Vec<Id> {
    if stuecke.is_empty() {
        doc.remove_entity(id);
        return Vec::new();
    }
    let mut it = stuecke.into_iter();
    if let Some((a0, a1)) = it.next() {
        if let Some(em) = doc.entity_mut(id) {
            em.kind = EntityKind::Arc { c, r, a0, a1 };
        }
    }
    let mut created = Vec::new();
    for (a0, a1) in it {
        let nid = doc.new_id();
        let mut ne = Entity::new(nid, e.layer, EntityKind::Arc { c, r, a0, a1 });
        ne.style = e.style.clone();
        doc.insert_ranked(ne);
        created.push(nid);
    }
    created
}

/// Übernimmt die beim Stutzen einer Ellipse bleibenden Abschnitte: der erste
/// bleibt am Objekt (Id, Ebene, Stil), weitere werden neue Objekte und ordnen
/// sich nach ihrem Zeichenrang ein. Ohne Stück verschwindet das Objekt.
fn bogen_stuecke_uebernehmen(
    doc: &mut Document,
    id: Id,
    e: &Entity,
    c: Vec2,
    rx: f64,
    ry: f64,
    rot: f64,
    stuecke: Vec<(f64, f64)>,
) -> Vec<Id> {
    if stuecke.is_empty() {
        doc.remove_entity(id);
        return Vec::new();
    }
    let mut it = stuecke.into_iter();
    if let Some((a0, a1)) = it.next() {
        if let Some(em) = doc.entity_mut(id) {
            em.kind = EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 };
        }
    }
    let mut created = Vec::new();
    for (a0, a1) in it {
        let nid = doc.new_id();
        let mut ne = Entity::new(nid, e.layer, EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 });
        ne.style = e.style.clone();
        doc.insert_ranked(ne);
        created.push(nid);
    }
    created
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
            if a.dist(*b) < 1e-6 {
                return Ok(vec![]);
            }
            let Some((t0, t1)) = trim_span(*a, *b, at, &segs) else {
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
            if a.dist(*b) < 1e-6 {
                return Ok(vec![]);
            }
            let Some((t0, t1)) = trim_span(*a, *b, at, &segs) else {
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
        // Kreis: der Klick schneidet einen Abschnitt heraus, der Rest bleibt als
        // **Bogen** stehen. Mehrere Schnittkanten ergeben mehrere Bögen — vorher
        // blieb nur einer übrig und Bogen/Kreis als Schnittkante fehlten ganz
        // (T0084, Turn 2).
        EntityKind::Circle { c, r } => {
            let Some((bleibt, _)) = kreis_trim_stuecke(*c, *r, at, &segs) else {
                return Err("Kreis braucht zwei Schnittpunkte zum Stutzen".into());
            };
            Ok(kreis_stuecke_uebernehmen(doc, id, &e, *c, *r, bleibt))
        }
        EntityKind::Arc { c, r, a0, a1 } => {
            let Some((bleibt, _)) = bogen_trim_stuecke(*c, *r, *a0, *a1, at, &segs) else {
                return Ok(vec![]);
            };
            Ok(kreis_stuecke_uebernehmen(doc, id, &e, *c, *r, bleibt))
        }
        // Ellipse: der Klick schneidet einen Abschnitt heraus, der Rest bleibt
        // als **Ellipsenbogen** stehen — wie beim Kreis (dort wird daraus ein
        // Bogen). Mehrere Schnittkanten ergeben mehrere Teile (T0084).
        EntityKind::Ellipse { c, rx, ry, rot } => {
            let el = Ellipse::new(*c, *rx, *ry, *rot);
            let Some((bleibt, _)) = ellipse_trim_stuecke(&el, at, &segs) else {
                return Err("Ellipse braucht zwei Schnittpunkte zum Stutzen".into());
            };
            Ok(bogen_stuecke_uebernehmen(doc, id, &e, *c, *rx, *ry, *rot, bleibt))
        }
        EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = Ellipse::new(*c, *rx, *ry, *rot);
            let Some((bleibt, _)) = ellipsearc_trim_stuecke(&el, *a0, *a1, at, &segs) else {
                return Ok(vec![]);
            };
            Ok(bogen_stuecke_uebernehmen(doc, id, &e, *c, *rx, *ry, *rot, bleibt))
        }
        EntityKind::Polyline { verts, closed } => {
            let Some((i, t0, t1)) = polyline_trim(verts, *closed, id, at, &segs) else {
                return Ok(vec![]);
            };
            let stuecke: Vec<EntityKind> = polyline_rest(verts, *closed, i, t0, t1)
                .iter()
                .filter_map(|v| polyline_kind(v, false))
                .collect();
            if stuecke.is_empty() {
                doc.remove_entity(id);
                return Ok(vec![]);
            }
            // Das erste Stück bleibt am Objekt (Id, Ebene, Stil); weitere
            // Stücke sind neue Elemente und ordnen sich nach ihrem Zeichenrang ein.
            let mut it = stuecke.into_iter();
            if let Some(erstes) = it.next() {
                if let Some(em) = doc.entity_mut(id) {
                    em.kind = erstes;
                }
            }
            let mut created = Vec::new();
            for kind in it {
                let nid = doc.new_id();
                let mut ne = Entity::new(nid, e.layer, kind);
                ne.style = e.style.clone();
                doc.insert_ranked(ne);
                created.push(nid);
            }
            Ok(created)
        }
        _ => Err("Stutzen wird für diesen Objekttyp nicht unterstützt".into()),
    }
}

/// Ziel einer Verlängerung: das Stück, das angesetzt wird, und das neue Ende.
struct Verlaengerung {
    /// Das Stück zwischen dem bisherigen Ende und dem Ziel — Grundlage der
    /// Vorschau in der Zeichenfläche.
    weg: Seg,
    /// Der Punkt, bis zu dem verlängert wird.
    ziel: Vec2,
    /// Das wandernde Ende ist das `b`-Ende (bei einer Polylinie: der letzte Punkt).
    ende_b: bool,
    /// Neue Krümmung des Endsegments einer Polylinie; `None` = das Segment
    /// bleibt gerade.
    bulge: Option<f64>,
}

/// Wie weit [`extend_entity`] das Objekt an der Stelle `at` verlängern würde —
/// ohne die Zeichnung zu ändern. Eine Quelle für die Action (verlängert) und
/// [`extend_vorschau`] (zeigt vorher, was hinzukommt); sonst liefen Anzeige
/// und Ergebnis auseinander (T0081).
fn verlaengerung(doc: &Document, id: Id, at: Vec2, cutters: &[Id]) -> Result<Verlaengerung, String> {
    let Some(e) = doc.entity(id) else {
        return Err(format!("Objekt {id} nicht gefunden"));
    };
    let exclude = vec![id];
    let segs = if cutters.is_empty() {
        cutter_segments_umrandung(doc, &exclude)
    } else {
        cutter_segments_umrandung(doc, &exclude)
            .into_iter()
            .filter(|(_, sid)| cutters.contains(sid))
            .collect()
    };
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
            let ende_b = at.dist(*b) <= at.dist(*a);
            let (fest, wandernd) = if ende_b { (*a, *b) } else { (*b, *a) };
            let ziel = naechste_kante(fest, wandernd, &segs)?;
            Ok(Verlaengerung {
                weg: Seg::Line(wandernd, ziel),
                ziel,
                ende_b,
                bulge: None,
            })
        }
        // Polylinie (T0155): das Ende näher am Klick wandert — wie bei einer
        // Linie. Ein gerades Endsegment läuft in seiner Richtung weiter, ein
        // Endsegment mit Stützbogen auf seinem eigenen Kreis.
        EntityKind::Polyline { verts, closed } => {
            if *closed {
                return Err("Geschlossene Polylinie hat kein Ende zum Verlängern".into());
            }
            let n = verts.len();
            if n < 2 {
                return Err("Polylinie ohne Strecke".into());
            }
            let ende_b = at.dist(verts[n - 1].p) <= at.dist(verts[0].p);
            // Krümmung und Nachbarpunkt gehören zum Endsegment: bei `ende_b`
            // ist es das letzte Segment (Krümmung am vorletzten Punkt), sonst
            // das erste (Krümmung am ersten Punkt).
            let (prev, ende, bulge) = if ende_b {
                (verts[n - 2].p, verts[n - 1].p, verts[n - 2].bulge)
            } else {
                (verts[1].p, verts[0].p, verts[0].bulge)
            };
            if prev.dist(ende) < 1e-9 {
                return Err("Polylinie ohne Strecke am Ende".into());
            }
            // Gerades Endsegment (auch: Krümmung ohne Bogen) läuft in seiner
            // Richtung weiter; ein Bogen auf seinem eigenen Kreis.
            let arc = if bulge.abs() < 1e-12 {
                None
            } else {
                arc_from_bulge(prev, ende, bulge)
            };
            match arc {
                Some(arc) => {
                    let (ziel, weg, neu) = bogen_verlaengern(&arc, prev, ende, bulge, &segs)?;
                    Ok(Verlaengerung {
                        weg,
                        ziel,
                        ende_b,
                        bulge: Some(neu),
                    })
                }
                None => {
                    let ziel = naechste_kante(prev, ende, &segs)?;
                    Ok(Verlaengerung {
                        weg: Seg::Line(ende, ziel),
                        ziel,
                        ende_b,
                        bulge: None,
                    })
                }
            }
        }
        _ => Err("Verlängern wird für diesen Objekttyp nicht unterstützt".into()),
    }
}

/// Der erste Schnittpunkt auf dem Strahl über das wandernde Ende hinaus.
///
/// Dieselbe Rechnung für Linie, Wand und das gerade Endsegment einer Polylinie;
/// Schnittkanten sind Strecken **und** Bögen/Kreise.
fn naechste_kante(fest: Vec2, wandernd: Vec2, segs: &[(Seg, Id)]) -> Result<Vec2, String> {
    let len = fest.dist(wandernd);
    if len < 1e-9 {
        return Err("Objekt ohne Länge".into());
    }
    let dir = (wandernd - fest).normalized();
    let ray_end = fest + dir * (len * 1000.0 + 1e9);
    let mut best: Option<(f64, Vec2)> = None;
    for (sg, _) in segs {
        let hits = match sg {
            Seg::Line(c, d) => line_line(fest, ray_end, *c, *d, true, false)
                .into_iter()
                .collect::<Vec<_>>(),
            Seg::Arc(arc) => line_circle(fest, ray_end, arc.c, arc.r)
                .into_iter()
                .filter(|p| angle_in_arc((*p - arc.c).angle(), arc.a0, arc.a1))
                .collect(),
        };
        for h in hits {
            let t = (h - fest).dot(dir);
            if t > len + 1e-6 && best.map(|(bt, _)| t < bt).unwrap_or(true) {
                best = Some((t, h));
            }
        }
    }
    best.map(|(_, p)| p)
        .ok_or_else(|| "Keine Schnittkante zum Verlängern gefunden".to_string())
}

/// Liegt `p` auf der Strecke `a`→`b` — nicht nur auf ihrer Geraden?
///
/// [`line_circle`] schneidet die **unendliche** Gerade; ohne diese Prüfung
/// könnte ein Bogen an einer Schnittkante enden, die gar nicht bis dorthin
/// reicht.
fn auf_der_strecke(a: Vec2, b: Vec2, p: Vec2) -> bool {
    let d = a.dist(b);
    d > 1e-9 && (a.dist(p) + p.dist(b) - d).abs() <= 1e-6 * (1.0 + d)
}

/// Verlängert ein Bogen-Endsegment einer Polylinie auf seinem eigenen Kreis
/// (T0155): der Bogen läuft in seiner Laufrichtung weiter bis zur nächsten
/// Schnittkante.
///
/// Zurück kommen das neue Ende, das angesetzte Bogenstück und die neue
/// Krümmung des Segments — Vorzeichen wie bisher, Betrag aus dem größeren
/// Öffnungswinkel (`tan(θ/4)`), damit derselbe Kreis herauskommt.
fn bogen_verlaengern(
    arc: &Arc,
    prev: Vec2,
    ende: Vec2,
    bulge: f64,
    segs: &[(Seg, Id)],
) -> Result<(Vec2, Seg, f64), String> {
    if arc.r < 1e-9 {
        return Err("Bogen ohne Radius".into());
    }
    // `arc` ist immer positiv orientiert; ob die Polylinie in dieser Richtung
    // läuft, entscheidet der Anfangspunkt des Segments.
    let vorwaerts = arc.start().dist(prev) <= arc.end().dist(prev);
    let lauf = if vorwaerts { 1.0 } else { -1.0 };
    let winkel_ende = (ende - arc.c).angle();
    let sweep = arc.sweep();
    let mut best: Option<(f64, Vec2)> = None;
    for (sg, _) in segs {
        let punkte: Vec<Vec2> = match sg {
            Seg::Line(c, d) => line_circle(*c, *d, arc.c, arc.r)
                .into_iter()
                .filter(|p| auf_der_strecke(*c, *d, *p))
                .collect(),
            Seg::Arc(other) => circle_circle(arc.c, arc.r, other.c, other.r)
                .into_iter()
                .filter(|p| angle_in_arc((*p - other.c).angle(), other.a0, other.a1))
                .collect(),
        };
        for p in punkte {
            // Winkelweg vom bisherigen Ende bis zum Treffer in Laufrichtung.
            let t = norm_angle(lauf * ((p - arc.c).angle() - winkel_ende));
            if t > 1e-6 && t < std::f64::consts::TAU - 1e-6
                && best.map(|(bt, _)| t < bt).unwrap_or(true)
            {
                best = Some((t, p));
            }
        }
    }
    let Some((t, _)) = best else {
        return Err("Keine Schnittkante zum Verlängern gefunden".into());
    };
    let neuer_sweep = sweep + t;
    if neuer_sweep >= std::f64::consts::TAU - 1e-6 {
        return Err("Verlängern würde den Bogen schließen".into());
    }
    let ziel = arc.c + Vec2::polar(winkel_ende + lauf * t, arc.r);
    let weg = if lauf > 0.0 {
        Seg::Arc(Arc::new(arc.c, arc.r, winkel_ende, winkel_ende + t))
    } else {
        Seg::Arc(Arc::new(arc.c, arc.r, winkel_ende - t, winkel_ende))
    };
    let neu = (neuer_sweep / 4.0).tan() * if bulge < 0.0 { -1.0 } else { 1.0 };
    Ok((ziel, weg, neu))
}

fn extend_entity(doc: &mut Document, id: Id, at: Vec2, cutters: &[Id]) -> Result<(), String> {
    let v = verlaengerung(doc, id, at, cutters)?;
    if let Some(em) = doc.entity_mut(id) {
        match &mut em.kind {
            EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
                if v.ende_b {
                    *b = v.ziel;
                } else {
                    *a = v.ziel;
                }
            }
            // Polylinie (T0155): der Endpunkt wandert, die Krümmung des
            // Endsegments sitzt an dessen **Anfang**.
            EntityKind::Polyline { verts, .. } => {
                let n = verts.len();
                if v.ende_b {
                    if let Some(letztes) = verts.last_mut() {
                        letztes.p = v.ziel;
                    }
                    if let Some(b) = v.bulge {
                        if n >= 2 {
                            verts[n - 2].bulge = b;
                        }
                    }
                } else if let Some(erstes) = verts.first_mut() {
                    erstes.p = v.ziel;
                    if let Some(b) = v.bulge {
                        erstes.bulge = b;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Verschneiden
// ---------------------------------------------------------------------------

/// Die Achse eines geraden Objekts — Linie oder Wand.
///
/// Alles andere (Bogen, Kreis, Polylinie, Text …) hat keine Gerade, auf der
/// sich ein Schnittpunkt bestimmen ließe.
fn gerade(doc: &Document, id: Id) -> Result<(Vec2, Vec2), String> {
    let Some(e) = doc.entity(id) else {
        return Err(format!("Objekt {id} nicht gefunden"));
    };
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => Ok((*a, *b)),
        _ => Err("Verschneiden geht nur mit Linien und Wänden".into()),
    }
}

/// Der gemeinsame Schnittpunkt der unendlich verlängerten Achsen zweier
/// gerader Objekte.
fn treffpunkt(doc: &Document, a: Id, b: Id) -> Result<Vec2, String> {
    if a == b {
        return Err("Verschneiden braucht zwei verschiedene Objekte".into());
    }
    let (a1, a2) = gerade(doc, a)?;
    let (b1, b2) = gerade(doc, b)?;
    if a1.dist(a2) < 1e-9 || b1.dist(b2) < 1e-9 {
        return Err("Objekt ohne Länge".into());
    }
    // Beide Achsen als unendliche Geraden: der Schnittpunkt darf außerhalb
    // beider Strecken liegen, sonst gäbe es nichts zu verlängern.
    line_line(a1, a2, b1, b2, true, true).ok_or_else(|| "Linien sind parallel".to_string())
}

/// Das Ende, das zum Schnittpunkt wandert, und das andere, das stehen bleibt.
///
/// Gewandert wird das **nähere** Ende: liegt der Schnittpunkt dahinter, ist das
/// eine Verlängerung, liegt er davor, eine Kürzung auf die Ecke.
fn verschnitt_enden(pa: Vec2, pb: Vec2, ziel: Vec2) -> (Vec2, Vec2) {
    if pa.dist(ziel) > pb.dist(ziel) {
        (pb, pa)
    } else {
        (pa, pb)
    }
}

/// Zieht zwei gerade Objekte auf ihren gemeinsamen Schnittpunkt — die Action
/// `modify.intersect`.
///
/// Je Objekt wandert das Ende, das dem Schnittpunkt **näher** liegt, auf den
/// Schnittpunkt. Liegt er hinter diesem Ende, wird verlängert; liegt er
/// zwischen den Enden, wird auf die Ecke gekürzt. Beides ergibt denselben
/// sauberen Anschluss, deshalb braucht der Nutzer dafür keinen zweiten Befehl.
/// Zurück kommt der Schnittpunkt.
pub fn verschneiden(doc: &mut Document, a: Id, b: Id) -> Result<Vec2, String> {
    let ziel = treffpunkt(doc, a, b)?;
    for id in [a, b] {
        let Some(em) = doc.entity_mut(id) else {
            continue;
        };
        match &mut em.kind {
            EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
                let (wandernd, _) = verschnitt_enden(*a, *b, ziel);
                if wandernd.dist(*a) < 1e-9 {
                    *a = ziel;
                } else {
                    *b = ziel;
                }
            }
            _ => {}
        }
    }
    doc.modified = true;
    Ok(ziel)
}

/// Die beiden Strecken, die [`verschneiden`] setzen würde — je Objekt vom
/// stehenden Ende zum Schnittpunkt, **ohne** die Zeichnung zu ändern.
///
/// Grundlage der Hervorhebung in der Zeichenfläche: der Nutzer sieht nach dem
/// ersten Klick, wohin das zweite Objekt gezogen wird.
pub fn verschneiden_vorschau(doc: &Document, a: Id, b: Id) -> Vec<(Id, Vec<Seg>)> {
    let Ok(ziel) = treffpunkt(doc, a, b) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in [a, b] {
        let Ok((pa, pb)) = gerade(doc, id) else {
            continue;
        };
        let (_, fest) = verschnitt_enden(pa, pb, ziel);
        out.push((id, vec![Seg::Line(fest, ziel)]));
    }
    out
}

// ---------------------------------------------------------------------------
// Zerlegen
// ---------------------------------------------------------------------------

/// Zerlegt mehrere Objekte in **einem** Durchgang (T0190).
///
/// Die Anzeigelisten aller Objekte entstehen aus **demselben, unveränderten**
/// Stand der Zeichnung. Wird Objekt für Objekt zerlegt, fehlt dem zweiten
/// Objekt beim Rendern der bereits entfernte Nachbar: die Wand bekommt an der
/// Ecke eine geschlossene Umrandung statt der Gehrung, und die an einem
/// Volumen freigelegte Stelle entfällt — das Bild nach dem Zerlegen wiche vom
/// Bild davor ab.
pub fn explode_entities(doc: &mut Document, ids: &[Id]) -> Result<Vec<Id>, String> {
    // 1. Prüfen — ein Fehler darf die Auswahl noch nicht verändert haben.
    let mut ziele: Vec<Entity> = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(e) = doc.entity(*id).cloned() else {
            return Err(format!("Objekt {id} nicht gefunden"));
        };
        pruefe_zerlegbar(doc, &e)?;
        ziele.push(e);
    }
    if ziele.is_empty() {
        return Ok(Vec::new());
    }
    // 2. Anzeigelisten aller Objekte aus demselben Stand bauen — noch ist keine
    //    Wand entfernt, also rechnet der Renderer Gehrung und Anschlüsse wie im
    //    Bild vor dem Zerlegen.
    let mut listen: Vec<Vec<render::Prim>> = (0..ziele.len()).map(|_| Vec::new()).collect();
    {
        let ctx = render::RenderCtx::new(doc);
        for (i, e) in ziele.iter().enumerate() {
            if matches!(e.kind, EntityKind::Text { .. }) {
                continue;
            }
            let mut dl = render::DisplayList::default();
            render::render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
            listen[i] = dl.prims;
        }
    }
    // 3. Erst alle Objekte entfernen, dann die Grundelemente einhängen.
    for e in &ziele {
        doc.remove_entity(e.id);
    }
    let mut created = Vec::new();
    for (i, e) in ziele.iter().enumerate() {
        if matches!(e.kind, EntityKind::Text { .. }) {
            // Zeichenumrisse über die Schrift — `explode_text` legt sie selbst ab.
            created.extend(explode_text(doc, e)?);
            continue;
        }
        for prim in &listen[i] {
            let Some(kind) = zerlege_prim(&prim.shape) else {
                continue;
            };
            let nid = doc.new_id();
            let mut ne = Entity::new(nid, e.layer, kind);
            ne.style.color = Some(prim.style.color);
            ne.style.line_type = Some(prim.style.line_type);
            ne.style.line_weight = Some(prim.style.width.max(0.05));
            ne.style.fill = prim.style.fill;
            // Ein Musterpunkt wird zum **gefüllten Kreis in der Musterfarbe**:
            // ohne ausdrückliche Füllfarbe bliebe er ein leerer Ring und die
            // Schraffur verblasste beim Zerlegen (T0138).
            if matches!(&prim.shape, Shape::Dot { .. }) {
                ne.style.fill = Some(prim.style.color);
            }
            doc.ents_mut().push(ne);
            created.push(nid);
        }
    }
    doc.modified = true;
    Ok(created)
}

/// Zerlegt ein Objekt in Grundelemente (auch Blöcke und Bemaßungen).
pub fn explode_entity(doc: &mut Document, id: Id) -> Result<Vec<Id>, String> {
    explode_entities(doc, &[id])
}

/// Zerlegt **alle** Verwendungen eines Blocks — im Modell, auf jedem Blatt und
/// in anderen Blockdefinitionen (T0219). Liefert die Zahl der zerlegten
/// Instanzen.
///
/// Grundlage ist [`Document::block_verwendungen`]; zerlegt wird je
/// Bearbeitungsraum, weil `explode_entities` auf `doc.ents_mut()` arbeitet.
/// Der Raum, in dem der Aufrufer stand, wird danach wiederhergestellt.
pub fn zerlege_block_instanzen(doc: &mut Document, block_id: &str) -> Result<usize, String> {
    // Erst sammeln, was je Raum zu zerlegen ist — die Kennungen sind
    // zeichnungsweit eindeutig, die Zuordnung geht beim Umschalten nicht verloren.
    let treffer = |liste: &[Entity]| -> Vec<Id> {
        liste
            .iter()
            .filter(|e| matches!(&e.kind, EntityKind::BlockRef { block, .. } if block == block_id))
            .map(|e| e.id)
            .collect()
    };
    let mut raeume: Vec<(Option<Id>, Option<String>, Vec<Id>)> = Vec::new();
    let modell = treffer(&doc.entities);
    if !modell.is_empty() {
        raeume.push((None, None, modell));
    }
    for s in &doc.sheets {
        let ids = treffer(&s.entities);
        if !ids.is_empty() {
            raeume.push((Some(s.id), None, ids));
        }
    }
    for b in &doc.blocks {
        if b.id == block_id {
            continue;
        }
        let ids = treffer(&b.entities);
        if !ids.is_empty() {
            raeume.push((None, Some(b.id.clone()), ids));
        }
    }
    let (alt_space, alt_block) = (doc.space, doc.block_space.clone());
    let mut anzahl = 0usize;
    let mut fehler: Option<String> = None;
    for (space, block, ids) in raeume {
        doc.space = space;
        doc.block_space = block;
        for id in ids {
            match explode_entities(doc, &[id]) {
                Ok(_) => anzahl += 1,
                Err(e) => {
                    fehler = Some(e);
                    break;
                }
            }
        }
        if fehler.is_some() {
            break;
        }
    }
    doc.space = alt_space;
    doc.block_space = alt_block;
    match fehler {
        Some(e) => Err(e),
        None => Ok(anzahl),
    }
}

/// Was sich grundsätzlich nicht zerlegen lässt, wird **vor** jeder Änderung
/// abgewiesen — sonst bliebe bei einer Liste die halbe Auswahl zerlegt zurück.
fn pruefe_zerlegbar(doc: &Document, e: &Entity) -> Result<(), String> {
    if matches!(e.kind, EntityKind::BlockRef { .. })
        && doc
            .block(match &e.kind {
                EntityKind::BlockRef { block, .. } => block,
                _ => "",
            })
            .is_none()
    {
        return Err("Blockdefinition fehlt".into());
    }
    // Ein Bild besteht aus Pixeln, nicht aus Linien — Zerlegen würde es
    // ersatzlos löschen. Deshalb ein klarer Fehler statt eines stillen Verlusts.
    if matches!(e.kind, EntityKind::Image { .. }) {
        return Err("Ein Bild lässt sich nicht zerlegen".into());
    }
    // Ein Text wird nicht über die Displayliste zerlegt, sondern über die
    // Umrisse seiner Zeichen — und nur mit Schriften, die dafür vorgesehen sind.
    if let EntityKind::Text { font, text, .. } = &e.kind {
        if !font.explodierbar() {
            return Err(format!(
                "Schrift „{}“ lässt sich nicht in Linien zerlegen — {}",
                font.label(),
                font.explode_hinweis()
            ));
        }
        if text.trim().is_empty() {
            return Err("Der Text ist leer".into());
        }
    }
    Ok(())
}

/// Wandelt eine Anzeigeform in die passende Objektart (Grundelement).
fn zerlege_prim(shape: &Shape) -> Option<EntityKind> {
    Some(match shape {
        Shape::Line(a, b) => {
            if a.dist(*b) < 1e-9 {
                return None;
            }
            EntityKind::Line { a: *a, b: *b }
        }
        Shape::Poly(pts, closed) => {
            if pts.len() < 2 {
                return None;
            }
            EntityKind::Polyline {
                verts: pts.iter().map(|p| Vertex::new(*p)).collect(),
                closed: *closed,
            }
        }
        Shape::Circle { c, r } => EntityKind::Circle { c: *c, r: *r },
        Shape::Arc { c, r, a0, a1 } => EntityKind::Arc {
            c: *c,
            r: *r,
            a0: *a0,
            a1: *a1,
        },
        Shape::Ellipse { c, rx, ry, rot } => EntityKind::Ellipse {
            c: *c,
            rx: *rx,
            ry: *ry,
            rot: *rot,
        },
        Shape::EllipseArc {
            c,
            rx,
            ry,
            rot,
            a0,
            a1,
        } => EntityKind::EllipseArc {
            c: *c,
            rx: *rx,
            ry: *ry,
            rot: *rot,
            a0: *a0,
            a1: *a1,
        },
        Shape::Fill { outer, holes } | Shape::FillOrdered { outer, holes } => {
            if outer.len() < 3 {
                return None;
            }
            EntityKind::Fill {
                boundary: outer.clone(),
                holes: holes.clone(),
                pattern: FillPattern::Solid,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: false,
            }
        }
        // Ein Punkt des Musters (Sand, Putz) ist ein **gefüllter Kreis** vom
        // Radius `r` — ein Punktobjekt zeichnete dagegen einen Anzeigepunkt
        // (Radius aus der Linienstärke × Beschriftungsmaßstab, mindestens
        // 32 mm bei 1:100) und die Sandkörner wären neunmal zu groß (T0138).
        Shape::Dot { p, r } => EntityKind::Circle { c: *p, r: *r },
        // Bilder werden vor der Displayliste abgewiesen (siehe oben).
        Shape::Image { .. } => return None,
        Shape::Text(t) => EntityKind::Text {
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
            line_spacing: t.line_spacing,
            frame: false,
            frame_round: false,
            background: None,
        },
    })
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
        EntityKind::EllipseArc {
            c,
            rx,
            ry,
            rot,
            a0,
            a1,
        } => EntityKind::EllipseArc {
            c: s(c),
            rx: rx * f,
            ry: ry * f,
            rot: *rot,
            a0: *a0,
            a1: *a1,
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
            line_spacing,
            frame,
            frame_round,
            background,
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
            // Die Schrift des Textes bleibt beim Skalieren (vorher fiel sie
            // still auf die Vorgabeschrift zurück).
            font: *font,
            line_spacing: *line_spacing,
            frame: *frame,
            frame_round: *frame_round,
            background: *background,
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
        groesse: BlockGroesse::Fest,
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
        line_spacing,
        ..
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
    // Zeilenhöhe der Familie wie im Rahmen, hier in Em-Größe `height`
    // (Zerlegen rechnet ohne den Anzeigefaktor 1,32 wie Rasterausgabe und PDF).
    let lh = size
        * crate::fonts::line_height_em(*font, crate::fonts::FontStyle::Regular)
        * line_spacing.clamp(0.1, 10.0);
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
                a.id, a.label(), a.hint()
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
