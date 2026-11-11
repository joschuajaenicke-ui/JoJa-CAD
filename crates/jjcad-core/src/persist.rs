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
    migrate_legacy_images(&mut raw);
    migrate_legacy_dims(&mut raw);
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
    // T0129: Ältere Dateien tragen keine Kennung. Vergeben wird sie in
    // `load_document` — dort ist der Pfad bekannt, und daraus lässt sich eine
    // **stabile** Kennung ableiten. Hier (ohne Pfad) bleibt das Feld leer.
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
    // Ansichtsfenster älterer Dateien zu normalen Blattobjekten machen (T0109)
    normalize_sheet_viewports(&mut doc);
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

/// Ansichtsfenster zu normalen Blattobjekten machen (T0109).
///
/// Bis dahin standen sie in `Sheet::viewports` — mit eigenem Auswahlfeld,
/// eigenem Verschieben und ohne Griffe. Jetzt ist jedes Fenster ein
/// `EntityKind::Viewport` in `sheet.entities`; die alte Liste wird beim Laden
/// geleert und nie wieder geschrieben.
pub fn normalize_sheet_viewports(doc: &mut Document) {
    for sheet in doc.sheets.iter_mut() {
        let alt = std::mem::take(&mut sheet.viewports_alt);
        for vp in alt {
            let id = vp.id;
            let mut e = crate::model::Entity::new(id, 0, crate::model::EntityKind::Viewport(vp));
            e.style.color = Some(crate::style::Color::BLACK);
            // Hinter allem, was schon auf dem Blatt steht.
            sheet.entities.insert(0, e);
        }
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

/// Bilder älterer Dateien auf die **untere Kante** umstellen.
///
/// Bis T0100 (Turn 3) war ein Bild über zwei gegenüberliegende Ecken des
/// Rechtecks gespeichert. Jetzt beschreibt `a`→`b` die untere Kante, und die
/// Höhe steht in `height` (wirksam, solange `lock_ratio` aus ist). Alte Dateien
/// bekommen beim Laden dieselbe Geometrie: untere Kante aus der Box, Höhe als
/// gespeicherter Wert, Seitenverhältnis **nicht** gesperrt — so sieht die
/// Zeichnung aus wie vorher, statt sich beim Öffnen zu verändern.
fn migrate_legacy_images(v: &mut serde_json::Value) {
    let mut fix_list = |list: &mut Vec<serde_json::Value>| {
        for e in list.iter_mut() {
            let Some(kind) = e.get_mut("kind").and_then(|k| k.as_object_mut()) else {
                continue;
            };
            if kind.get("type").and_then(|t| t.as_str()) != Some("image") {
                continue;
            }
            // Neue Dateien tragen `height`; fehlt es, ist es ein altes Rechteck.
            if kind.contains_key("height") {
                continue;
            }
            let punkt = |k: &str| -> Option<(f64, f64)> {
                let p = kind.get(k)?;
                Some((p.get("x")?.as_f64()?, p.get("y")?.as_f64()?))
            };
            let (Some((ax, ay)), Some((bx, by))) = (punkt("a"), punkt("b")) else {
                continue;
            };
            let (x0, x1) = (ax.min(bx), ax.max(bx));
            let (y0, y1) = (ay.min(by), ay.max(by));
            kind.insert("a".into(), serde_json::json!({"x": x0, "y": y0}));
            kind.insert("b".into(), serde_json::json!({"x": x1, "y": y0}));
            kind.insert("height".into(), serde_json::json!((y1 - y0).max(1.0)));
            kind.insert("lock_ratio".into(), serde_json::json!(false));
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

/// Bemaßung älterer Dateien auf **ein** Element umstellen (T0154).
///
/// Bis dahin gab es zwei Objektarten: das Einzelmaß (`a`, `b`) und die
/// Maßkette (`dim_chain` mit `pts`). Jetzt ist beides `dim` mit `pts`; das
/// Einzelmaß ist die Kette aus genau zwei Punkten.
///
/// * `type: "dim_chain"` → `type: "dim"` (die Felder passen schon)
/// * `type: "dim"` mit `a`/`b` → `pts: [a, b]`, `a`/`b` entfallen
///
/// Serde-Alias geht nicht, weil sich neben dem Typnamen auch die Feldnamen
/// ändern — dieselbe Bauart wie [`migrate_legacy_fills`].
pub fn migrate_legacy_dims(v: &mut serde_json::Value) {
    let mut fix_list = |list: &mut Vec<serde_json::Value>| {
        for e in list.iter_mut() {
            let Some(kind) = e.get_mut("kind").and_then(|k| k.as_object_mut()) else {
                continue;
            };
            match kind.get("type").and_then(|t| t.as_str()) {
                Some("dim_chain") => {
                    kind.insert("type".into(), serde_json::json!("dim"));
                }
                Some("dim") => {
                    if !kind.contains_key("pts") {
                        if let Some(a) = kind.remove("a") {
                            if let Some(b) = kind.remove("b") {
                                kind.insert("pts".into(), serde_json::json!([a, b]));
                            } else {
                                kind.insert("a".into(), a);
                            }
                        }
                    }
                    // T0166: Die Achse (`x`/`y`/`aligned`) ist kein Feld des
                    // Objekts mehr — gespeichert wird die **Orientierung** als
                    // Winkel in Bogenmaß. Alte Dateien bekommen ihn hier.
                    if kind.contains_key("angle") {
                        kind.remove("axis");
                        continue;
                    }
                    // In der Datei steht der Serde-Name der Achse (`X`, `Y`,
                    // `Aligned`), in der Befehlsschicht der kleine Schlüssel
                    // (`x`, `y`, `aligned`) — beide gelten hier.
                    let achse = kind
                        .remove("axis")
                        .and_then(|a| a.as_str().map(|s| s.to_lowercase()))
                        .unwrap_or_else(|| "x".to_string());
                    let winkel = match achse.as_str() {
                        "y" | "vertical" | "senkrecht" => std::f64::consts::FRAC_PI_2,
                        "aligned" | "parallel" => {
                            // Richtung der ersten Messstrecke — dieselbe
                            // Rechnung wie `DimAxis::Aligned`.
                            let p = kind
                                .get("pts")
                                .and_then(|p| p.as_array())
                                .map(|a| a.as_slice())
                                .unwrap_or(&[]);
                            let punkt = |i: usize| -> Option<(f64, f64)> {
                                let q = p.get(i)?.as_array()?;
                                Some((q.first()?.as_f64()?, q.get(1)?.as_f64()?))
                            };
                            match (punkt(0), punkt(1)) {
                                (Some((ax, ay)), Some((bx, by))) => {
                                    let (dx, dy) = (bx - ax, by - ay);
                                    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
                                        0.0
                                    } else {
                                        dy.atan2(dx)
                                    }
                                }
                                _ => 0.0,
                            }
                        }
                        _ => 0.0,
                    };
                    kind.insert("angle".into(), serde_json::json!(winkel));
                }
                _ => {}
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
    // Blockdatei (`<name>.jjblk`): die Definition steckt unter `block`.
    if let Some(arr) = v
        .get_mut("block")
        .and_then(|b| b.get_mut("entities"))
        .and_then(|e| e.as_array_mut())
    {
        fix_list(arr);
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
    // T0129: Eine Zeichnung ohne Kennung im Kopf bekommt eine aus dem Pfad
    // abgeleitete — dieselbe Datei behält sie über Sitzungen hinweg. Eine
    // zufällige Kennung je Öffnen würde im Sicherungspool bei jedem Start einen
    // neuen Pin anlegen und den Verlauf der Zeichnung aufteilen.
    if doc.id.trim().is_empty() {
        doc.id = crate::daten::kennung_fuer_pfad(path);
    }
    doc.path = Some(path.to_string_lossy().to_string());
    doc.name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Zeichnung".into());
    Ok(doc)
}

/// Kopie einer Vorlagendatei als **neue** Zeichnung (T0162).
///
/// Der Inhalt kommt vollständig aus der Datei — Ebenen, Blätter, Blöcke,
/// Einstellungen und Objekte. Die Zeichnung selbst ist aber neu: kein Pfad,
/// nicht „geändert“ und mit eigener Kennung. Mit der Kennung der Vorlage
/// teilte die Kopie im Sicherungspool den Verlauf der Vorlagendatei.
pub fn lade_vorlage(pfad: &Path) -> Result<Document, String> {
    let mut doc = load_document(pfad)?;
    doc.path = None;
    doc.name = "Unbenannt".to_string();
    doc.id = crate::sicherung::neue_uuid();
    doc.modified = false;
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
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            DragMode::HoldAndDrag => crate::i18n::tr("ui.dragmode.hold"),
            DragMode::TwoClicks => crate::i18n::tr("ui.dragmode.two_clicks"),
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
    /// Tooltip in der gewählten Sprache (T0106).
    pub fn tip(self) -> &'static str {
        match self {
            DragMode::HoldAndDrag => crate::i18n::tr("ui.dragmode.hint_hold"),
            DragMode::TwoClicks => crate::i18n::tr("ui.dragmode.hint_two_clicks"),
        }
    }


    /// true, wenn mit zwei Klicks gezogen wird.
    pub fn two_clicks(self) -> bool {
        matches!(self, DragMode::TwoClicks)
    }
}

/// Ankerpunkt der Einfüge-Vorschau (T0070) — Einstellungen → Bedienung →
/// „Drehpunkt“.
///
/// Die Auswahl dreht in **beiden** Modi um ihren eigenen Mittelpunkt; der Modus
/// entscheidet, welcher Punkt danach am Fadenkreuz steht und damit, wo die
/// Vorschau liegt:
///
/// - [`PastePivot::Center`] (Vorgabe seit T0087): die untere linke Ecke der
///   **gedrehten** Auswahl — die Vorschau bleibt immer oben rechts am
///   Fadenkreuz und deckt es nicht zu.
/// - [`PastePivot::BasePoint`]: der mitkopierte Basispunkt. Er wandert mit der
///   Drehung, die Vorschau springt deshalb von Viertel zu Viertel um das
///   Fadenkreuz.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PastePivot {
    /// Basispunkt der Zwischenablage steht am Fadenkreuz
    BasePoint,
    /// Untere linke Ecke der gedrehten Auswahl steht am Fadenkreuz
    Center,
}

impl Default for PastePivot {
    fn default() -> Self {
        PastePivot::Center
    }
}

impl PastePivot {
    /// Beide Modi in Anzeigereihenfolge (Einstellungen).
    pub const ALL: [PastePivot; 2] = [PastePivot::BasePoint, PastePivot::Center];

    /// Deutsche Beschriftung für die Oberfläche.
    pub fn label(self) -> &'static str {
        match self {
            PastePivot::BasePoint => "Basispunkt",
            PastePivot::Center => "Mittelpunkt",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            PastePivot::BasePoint => crate::i18n::tr("ui.pastepivot.base"),
            PastePivot::Center => crate::i18n::tr("ui.pastepivot.center"),
        }
    }


    /// Kurze Erläuterung (Tooltip im Einstellungsfenster).
    pub fn hint(self) -> &'static str {
        match self {
            PastePivot::BasePoint => {
                "Der Basispunkt steht am Fadenkreuz — die Vorschau springt beim Drehen um das \
                 Fadenkreuz (bisheriges Verhalten)"
            }
            PastePivot::Center => {
                "Die Vorschau dreht um ihren Mittelpunkt und bleibt oben rechts am Fadenkreuz"
            }
        }
    }
    /// Tooltip in der gewählten Sprache (T0106).
    pub fn tip(self) -> &'static str {
        match self {
            PastePivot::BasePoint => crate::i18n::tr("ui.pastepivot.hint_base"),
            PastePivot::Center => crate::i18n::tr("ui.pastepivot.hint_center"),
        }
    }

}

/// Bewegungsart der Ansicht für die Schwenk-Funktionen (`view.pan_left` …
/// `view.pan_down`) — Einstellungen → Bedienung → „Ansicht verschieben“ (T0101).
///
/// - [`PanMode::Kontinuierlich`] (Programmstandard): solange eine
///   Schwenkfunktion gedrückt gehalten wird, wandert die Ansicht mit
///   gleichmäßigem Tempo weiter (Taste, Hotbar-Slot oder Ziffer).
/// - [`PanMode::Sprunghaft`]: ein Druck rückt genau **einen** Schritt weiter
///   (bisheriges Verhalten vor T0101).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanMode {
    /// Gedrückt halten bewegt die Ansicht gleichmäßig weiter (Vorgabe)
    #[default]
    Kontinuierlich,
    /// Ein Druck rückt um einen Schritt weiter
    Sprunghaft,
}

impl PanMode {
    /// Beide Modi in Anzeigereihenfolge (Einstellungen).
    pub const ALL: [PanMode; 2] = [PanMode::Kontinuierlich, PanMode::Sprunghaft];

    /// Deutscher Name — für Dateien, Action-Parameter und Tests.
    pub fn label(self) -> &'static str {
        match self {
            PanMode::Kontinuierlich => "Kontinuierlich",
            PanMode::Sprunghaft => "Sprunghaft",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            PanMode::Kontinuierlich => crate::i18n::tr("ui.panmode.kontinuierlich"),
            PanMode::Sprunghaft => crate::i18n::tr("ui.panmode.sprunghaft"),
        }
    }

    /// Kurze Erläuterung (Tooltip im Einstellungsfenster).
    pub fn hint(self) -> &'static str {
        match self {
            PanMode::Kontinuierlich => {
                "Solange die Funktion gedrückt bleibt, wandert die Ansicht gleichmäßig weiter"
            }
            PanMode::Sprunghaft => "Ein Druck rückt die Ansicht um einen Schritt weiter",
        }
    }
    /// Tooltip in der gewählten Sprache (T0106).
    pub fn tip(self) -> &'static str {
        match self {
            PanMode::Kontinuierlich => crate::i18n::tr("ui.panmode.hint_kontinuierlich"),
            PanMode::Sprunghaft => crate::i18n::tr("ui.panmode.hint_sprunghaft"),
        }
    }

    /// true, wenn gehaltenes Drücken die Ansicht weiterbewegt.
    pub fn kontinuierlich(self) -> bool {
        matches!(self, PanMode::Kontinuierlich)
    }
}

/// Farbdesign der Oberfläche (T0102) — Einstellungen → Darstellung → „Design“.
///
/// `System` ist der Programmstandard: beim Start und bei jeder Änderung des
/// Betriebssystems nimmt JoJa-CAD dessen helles oder dunkles Farbschema. Hell
/// und Dunkel sind feste Wahlen und überstimmen das Betriebssystem; sie
/// bleiben in `prefs.json` gespeichert.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Design {
    /// Farbschema des Betriebssystems (Vorgabe)
    #[default]
    #[serde(rename = "system")]
    System,
    /// Immer hell — helle Flächen, dunkle Schrift
    #[serde(rename = "hell")]
    Hell,
    /// Immer dunkel — dunkle Flächen, helle Schrift (bisheriges Aussehen)
    #[serde(rename = "dunkel")]
    Dunkel,
}

impl Design {
    /// Alle Wahlen in Anzeigereihenfolge (Einstellungen).
    pub const ALL: [Design; 3] = [Design::System, Design::Hell, Design::Dunkel];

    /// Deutscher Name — für Dateien, Action-Parameter und Tests.
    pub fn label(self) -> &'static str {
        match self {
            Design::System => "System",
            Design::Hell => "Hell",
            Design::Dunkel => "Dunkel",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106).
    pub fn text(self) -> &'static str {
        match self {
            Design::System => crate::i18n::tr("ui.design.system"),
            Design::Hell => crate::i18n::tr("ui.design.hell"),
            Design::Dunkel => crate::i18n::tr("ui.design.dunkel"),
        }
    }

    /// Kurze Erläuterung (Tooltip im Einstellungsfenster).
    pub fn hint(self) -> &'static str {
        match self {
            Design::System => "Folgt dem hellen oder dunklen Farbschema des Betriebssystems",
            Design::Hell => "Helle Flächen, dunkle Schrift — unabhängig vom Betriebssystem",
            Design::Dunkel => "Dunkle Flächen, helle Schrift — bisheriges Aussehen",
        }
    }
    /// Tooltip in der gewählten Sprache (T0106).
    pub fn tip(self) -> &'static str {
        match self {
            Design::System => crate::i18n::tr("ui.design.hint_system"),
            Design::Hell => crate::i18n::tr("ui.design.hint_hell"),
            Design::Dunkel => crate::i18n::tr("ui.design.hint_dunkel"),
        }
    }
}

/// Farbakzent der Oberfläche (T0111) — Einstellungen → Darstellung →
/// „Farbakzente“.
///
/// Der Akzent färbt **alle** hervorgehobenen Stellen der Bedienoberfläche:
/// aktive Reiter und Chips, Rahmen überfahrener Felder, Auswahl in Menüs und
/// Schalter. Bernstein-Orange ist der Programmstandard (bisheriges Aussehen);
/// die übrigen Wahlen sind nach Edelsteinen benannt — Saphir (blau), Smaragd
/// (grün), Rubin (rot), Amethyst (violett). Je Design (hell/dunkel) hat jeder
/// Akzent einen eigenen Ton, damit Schrift und Fläche lesbar bleiben; die Töne
/// stehen in `jjcad_gui::theme`.
///
/// Die Dateischlüssel tragen die alten Farbwörter als **Alias**: eine
/// `prefs.json` aus der ersten Fassung (`"blau"`, `"gruen"`, `"rot"`,
/// `"violett"`) lädt unverändert, ebenso die inzwischen entfallene Wahl
/// `"tuerkis"` (sie wird zu Saphir).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Farbakzent {
    /// Bernstein-Orange (Vorgabe, bisheriges Aussehen)
    #[default]
    #[serde(rename = "bernstein")]
    Bernstein,
    /// Blau
    #[serde(rename = "saphir", alias = "blau", alias = "tuerkis")]
    Saphir,
    /// Grün
    #[serde(rename = "smaragd", alias = "gruen")]
    Smaragd,
    /// Rot
    #[serde(rename = "rubin", alias = "rot")]
    Rubin,
    /// Violett
    #[serde(rename = "amethyst", alias = "violett")]
    Amethyst,
}

impl Farbakzent {
    /// Alle Wahlen in Anzeigereihenfolge (Einstellungen).
    pub const ALL: [Farbakzent; 5] = [
        Farbakzent::Bernstein,
        Farbakzent::Saphir,
        Farbakzent::Smaragd,
        Farbakzent::Rubin,
        Farbakzent::Amethyst,
    ];

    /// Deutscher Name — für Dateien, Action-Parameter und Tests.
    pub fn label(self) -> &'static str {
        match self {
            Farbakzent::Bernstein => "Bernstein",
            Farbakzent::Saphir => "Saphir",
            Farbakzent::Smaragd => "Smaragd",
            Farbakzent::Rubin => "Rubin",
            Farbakzent::Amethyst => "Amethyst",
        }
    }

    /// Beschriftung in der gewählten Sprache (T0106).
    pub fn text(self) -> &'static str {
        match self {
            Farbakzent::Bernstein => crate::i18n::tr("ui.farbakzent.bernstein"),
            Farbakzent::Saphir => crate::i18n::tr("ui.farbakzent.saphir"),
            Farbakzent::Smaragd => crate::i18n::tr("ui.farbakzent.smaragd"),
            Farbakzent::Rubin => crate::i18n::tr("ui.farbakzent.rubin"),
            Farbakzent::Amethyst => crate::i18n::tr("ui.farbakzent.amethyst"),
        }
    }

    /// Kurze Erläuterung (Tooltip im Einstellungsfenster).
    pub fn hint(self) -> &'static str {
        match self {
            Farbakzent::Bernstein => {
                "Bernstein-Orange — das bisherige Aussehen (Programmstandard)"
            }
            Farbakzent::Saphir => "Saphir — blauer Akzent für Reiter, Chips und Rahmen",
            Farbakzent::Smaragd => "Smaragd — grüner Akzent für Reiter, Chips und Rahmen",
            Farbakzent::Rubin => "Rubin — roter Akzent für Reiter, Chips und Rahmen",
            Farbakzent::Amethyst => "Amethyst — violetter Akzent für Reiter, Chips und Rahmen",
        }
    }

    /// Tooltip in der gewählten Sprache (T0106).
    pub fn tip(self) -> &'static str {
        match self {
            Farbakzent::Bernstein => crate::i18n::tr("ui.farbakzent.hint_bernstein"),
            Farbakzent::Saphir => crate::i18n::tr("ui.farbakzent.hint_saphir"),
            Farbakzent::Smaragd => crate::i18n::tr("ui.farbakzent.hint_smaragd"),
            Farbakzent::Rubin => crate::i18n::tr("ui.farbakzent.hint_rubin"),
            Farbakzent::Amethyst => crate::i18n::tr("ui.farbakzent.hint_amethyst"),
        }
    }
}

/// Fensterrahmen (T0098) — experimentell, Einstellungen → Bedienung →
/// „Fensterrahmen“.
///
/// `System` ist das bisherige Verhalten: der Fenstermanager (KWin, Windows,
/// …) zeichnet Titelleiste, Rahmen und die drei Fensterknöpfe.
///
/// `OhneTitelleiste` meldet dem Fenstermanager „keine Dekoration“
/// (`ViewportBuilder::with_decorations(false)`):
///
/// * **X11 (KDE/X11, jedes andere X11-Desktop):** winit setzt
///   `_MOTIF_WM_HINTS` — KWin entfernt Rahmen und Titel.
/// * **Wayland (KDE/Wayland):** winit fordert *Client-seitige* Dekoration an
///   (`xdg-decoration`, `RequestClient`); KWin zeichnet dann nichts, und
///   winit zeichnet ebenfalls nichts — das Fenster ist rahmenlos.
/// * **Windows:** rahmenloses Fenster (`WS_POPUP`).
///
/// Ohne Titelleiste fehlen die Knöpfe und der Griff: JoJa-CAD zeichnet
/// Minimieren, Vergrößern/Wiederherstellen und Schließen selbst oben rechts,
/// die Kopfzeile verschiebt das Fenster (`ViewportCommand::StartDrag`) und die
/// Fensterkanten ändern die Größe (`ViewportCommand::BeginResize`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Fensterrahmen {
    /// Titelleiste und Rahmen vom Fenstermanager (bisheriges Verhalten)
    #[default]
    #[serde(rename = "system")]
    System,
    /// Eigenes Fenster ohne Titelleiste — Knöpfe und Ziehen zeichnet JoJa-CAD
    #[serde(rename = "ohne_titelleiste")]
    OhneTitelleiste,
}

impl Fensterrahmen {
    /// Beide Modi in Anzeigereihenfolge (Einstellungen).
    pub const ALL: [Fensterrahmen; 2] = [Fensterrahmen::System, Fensterrahmen::OhneTitelleiste];

    /// Deutsche Beschriftung für die Oberfläche.
    pub fn label(self) -> &'static str {
        match self {
            Fensterrahmen::System => "System",
            Fensterrahmen::OhneTitelleiste => "Ohne Titelleiste",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            Fensterrahmen::System => crate::i18n::tr("ui.fensterrahmen.system"),
            Fensterrahmen::OhneTitelleiste => crate::i18n::tr("ui.fensterrahmen.ohne_titelleiste"),
        }
    }


    /// Kurze Erläuterung (Tooltip im Einstellungsfenster).
    pub fn hint(self) -> &'static str {
        match self {
            Fensterrahmen::System => "Titelleiste und Rahmen zeichnet der Fenstermanager",
            Fensterrahmen::OhneTitelleiste => {
                "Experimentell: Knöpfe für Minimieren, Vergrößern und Schließen oben rechts, \
                 die Kopfzeile verschiebt das Fenster, die Kanten ändern die Größe \
                 (geprüft unter KDE)"
            }
        }
    }
    /// Tooltip in der gewählten Sprache (T0106).
    pub fn tip(self) -> &'static str {
        match self {
            Fensterrahmen::System => crate::i18n::tr("ui.fensterrahmen.hint_system"),
            Fensterrahmen::OhneTitelleiste => crate::i18n::tr("ui.fensterrahmen.hint_ohne_titelleiste"),
        }
    }


    /// `true`, wenn JoJa-CAD die Fensterknöpfe und das Ziehen selbst übernimmt.
    pub fn ohne_titelleiste(self) -> bool {
        matches!(self, Fensterrahmen::OhneTitelleiste)
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

/// Die eingebauten Tastaturvorlagen (T0079), in Anzeigereihenfolge. Sie stehen
/// immer als **erste** Einträge in [`UserPrefs::keymap_presets`], lassen sich
/// weder überschreiben noch löschen — der Rückweg auf eine bekannte Belegung.
/// Die erste Vorlage ist zugleich die Werkseinstellung.
pub const VORLAGEN: [&str; 4] = [
    "Vorlage JoJa-CAD",
    "Vorlage Allplan",
    "Vorlage ArchiCAD",
    "Vorlage AutoCAD",
];

/// Name des eingebauten Tastaturlayouts (T0076/T0079). Es lässt sich nicht
/// überschreiben und ist der Rückweg auf die Vorgabebelegung.
pub const WERKSEINSTELLUNG: &str = VORLAGEN[0];

/// Trägt der Name eine eingebaute Vorlage (oder den früheren Namen der ersten,
/// „Werkseinstellung" bis T0079)? Solche Namen gehören nicht dem Nutzer: sie
/// lassen sich nicht speichern, nicht löschen und werden beim Laden aus den
/// Vorlagen neu aufgebaut.
pub fn ist_vorlage(name: &str) -> bool {
    VORLAGEN.iter().any(|v| v.eq_ignore_ascii_case(name))
        || name.eq_ignore_ascii_case("Werkseinstellung")
}

/// Ein gespeichertes Tastaturlayout (T0076): Name und die vollständige
/// Belegung (Aktion → Tasten). Ein Layout lässt sich benennen, wechseln und als
/// Datei aus- und wieder einlesen; die „Werkseinstellung" ist das eingebaute
/// erste Layout und ersetzt den früheren Knopf gleichen Namens.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeymapPreset {
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "de_bindungen")]
    pub bindings: BTreeMap<String, Vec<String>>,
}

impl KeymapPreset {
    /// Dateiinhalt eines Layouts — dieselbe Form, die der Import wieder liest.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Liest ein Layout aus einer Datei. Zwei Formen werden angenommen: das
    /// vollständige Layout (`{"name": …, "bindings": {…}}`) und eine nackte
    /// Belegung (`{"draw.line": ["l"]}`) — eine von Hand geschriebene Datei
    /// soll nicht am fehlenden Rahmen scheitern.
    pub fn from_json(text: &str, fallback_name: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Roh {
            Layout(KeymapPreset),
            NurBelegung(BTreeMap<String, Vec<String>>),
        }
        match serde_json::from_str::<Roh>(text) {
            Ok(Roh::Layout(mut p)) => {
                if p.name.trim().is_empty() {
                    p.name = fallback_name.to_string();
                }
                Ok(p)
            }
            Ok(Roh::NurBelegung(bindings)) => Ok(KeymapPreset {
                name: fallback_name.to_string(),
                bindings,
            }),
            Err(e) => Err(format!("Kein Tastaturlayout: {e}")),
        }
    }
}

/// Maximale Rastergröße der Schrift in der Zeichenfläche (T0114).
///
/// egui rastert jede Glyphe in genau der angefragten Pixelgröße in einen
/// gemeinsamen Schriftatlas; je größer die Rasterung, desto schärfer die
/// Schrift im hohen Zoom — und desto mehr Atlas-Pixel. Die Zeichenfläche
/// rastert deshalb höchstens in dieser Größe und streckt den Rest
/// (`jjcad_gui::textgroesse`). 256 px ist die Vorgabe: alle Stufen bis dahin
/// brauchen zusammen rund 7 % des Atlasses, darüber wird die Schrift weich.
/// 128 px ist die sparsamste Wahl, 1024 px die schärfste.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Textaufloesung {
    /// 128 px — sparsam, weich bei großen Schriften
    #[serde(rename = "px128")]
    Px128,
    /// 256 px — Vorgabe
    #[default]
    #[serde(rename = "px256")]
    Px256,
    /// 512 px — schärfer, mehr Atlas
    #[serde(rename = "px512")]
    Px512,
    /// 1024 px — schärfste Rasterung, größter Atlas
    #[serde(rename = "px1024")]
    Px1024,
}

impl Textaufloesung {
    /// Alle Wahlen in Anzeigereihenfolge (Einstellungen).
    pub const ALL: [Textaufloesung; 4] = [
        Textaufloesung::Px128,
        Textaufloesung::Px256,
        Textaufloesung::Px512,
        Textaufloesung::Px1024,
    ];

    /// Rastergröße in Pixeln — der Wert, der an `textgroesse` geht.
    pub fn px(self) -> f32 {
        match self {
            Textaufloesung::Px128 => 128.0,
            Textaufloesung::Px256 => 256.0,
            Textaufloesung::Px512 => 512.0,
            Textaufloesung::Px1024 => 1024.0,
        }
    }

    /// Deutscher Name — für Dateien und Tests.
    pub fn label(self) -> &'static str {
        match self {
            Textaufloesung::Px128 => "128 px",
            Textaufloesung::Px256 => "256 px",
            Textaufloesung::Px512 => "512 px",
            Textaufloesung::Px1024 => "1024 px",
        }
    }

    /// Beschriftung in der gewählten Sprache (T0106).
    pub fn text(self) -> &'static str {
        match self {
            Textaufloesung::Px128 => crate::i18n::tr("ui.textaufloesung.px128"),
            Textaufloesung::Px256 => crate::i18n::tr("ui.textaufloesung.px256"),
            Textaufloesung::Px512 => crate::i18n::tr("ui.textaufloesung.px512"),
            Textaufloesung::Px1024 => crate::i18n::tr("ui.textaufloesung.px1024"),
        }
    }

    /// Kurze Erläuterung (Tooltip im Einstellungsfenster).
    pub fn tip(self) -> &'static str {
        match self {
            Textaufloesung::Px128 => crate::i18n::tr("ui.textaufloesung.tip128"),
            Textaufloesung::Px256 => crate::i18n::tr("ui.textaufloesung.tip256"),
            Textaufloesung::Px512 => crate::i18n::tr("ui.textaufloesung.tip512"),
            Textaufloesung::Px1024 => crate::i18n::tr("ui.textaufloesung.tip1024"),
        }
    }
}

/// Die drei in den Einstellungen änderbaren Speicherorte (T0127).
/// `prefs.json` und `sitzung.json` liegen immer in der Wurzel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpeicherortFeld {
    /// Benutzerblock-Bibliothek, Vorgabe `blocks/`.
    Bloecke,
    /// Sicherungen, Vorgabe `sicherungen/`.
    Sicherungen,
    /// Protokolle, Vorgabe `log/`.
    Protokolle,
}

impl SpeicherortFeld {
    /// Alle Felder in der Reihenfolge der Einstellungsseite.
    pub const ALL: [SpeicherortFeld; 3] = [
        SpeicherortFeld::Bloecke,
        SpeicherortFeld::Sicherungen,
        SpeicherortFeld::Protokolle,
    ];

    /// Name des Unterordners in der Wurzel (Vorgabe).
    pub fn vorgabe(self) -> &'static str {
        match self {
            SpeicherortFeld::Bloecke => "blocks",
            SpeicherortFeld::Sicherungen => "sicherungen",
            SpeicherortFeld::Protokolle => "log",
        }
    }
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
    /// Gespeicherte Tastaturlayouts (T0076/T0079). Die ersten vier Einträge
    /// sind die **eingebauten Vorlagen** ([`VORLAGEN`]) — JoJa-CAD, Allplan,
    /// ArchiCAD, AutoCAD; sie lassen sich nicht überschreiben und nicht
    /// löschen. Eigene Layouts des Nutzers stehen dahinter.
    #[serde(default)]
    pub keymap_presets: Vec<KeymapPreset>,
    /// Welches Layout gerade gilt (Index in `keymap_presets`). `None` =
    /// „Eigene Belegung": die Belegung wurde von Hand geändert oder das Layout
    /// gelöscht. Ausdrücklich gespeichert, weil zwei Layouts dieselbe Belegung
    /// tragen dürfen — die Auswahl muss trotzdem den Namen zeigen, den der
    /// Nutzer zuletzt gespeichert oder gewählt hat (T0076).
    #[serde(default)]
    pub keymap_aktiv: Option<usize>,
    #[serde(default)]
    pub recent_files: Vec<String>,
    #[serde(default)]
    pub last_dir: String,
    #[serde(default = "default_true")]
    pub show_lineweights: bool,
    /// Linienstärken maßstabsgetreu statt bildschirmkonstant
    #[serde(default)]
    pub lineweights_scaled: bool,
    /// Faktor der Linienstärke im Modus „Bildschirm (konstant)“ (T0139):
    /// 1,0 = Vorgabe, 2,0 = doppelt so dicke Striche. Im Modus „Papier
    /// (maßstabsgetreu)“ gilt er nicht — dort bestimmt der Maßstab der
    /// Zeichnung die Breite.
    #[serde(default = "default_lineweight_factor")]
    pub lineweight_factor: f32,
    /// Zieh-Modus für Objekte und Griffe
    #[serde(
        default,
        alias = "drag_two_clicks",
        deserialize_with = "de_drag_mode_mit_altfeld"
    )]
    pub drag_mode: DragMode,
    /// Zieh-Modus des **Auswahlfensters** (T0145) — eigene Einstellung neben
    /// [`UserPrefs::drag_mode`]: „Gedrückthalten“ (Vorgabe) zieht das Rechteck
    /// mit gedrückter Taste, „Zwei Klicks“ setzt die erste Ecke mit einem Klick
    /// und schließt mit dem zweiten. Griffe und Objekte bleiben unberührt.
    #[serde(default)]
    pub select_mode: DragMode,
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
    /// Bewegungsart der Ansicht für die Schwenk-Funktionen (T0101):
    /// `kontinuierlich` (Vorgabe) hält die Ansicht in Bewegung, solange die
    /// Funktion gedrückt bleibt; `sprunghaft` rückt je Druck um einen Schritt.
    #[serde(default)]
    pub pan_mode: PanMode,
    /// Drehpunkt und Anker der Einfüge-Vorschau aus der Zwischenablage (T0070):
    /// `center` (Vorgabe seit T0087) = die Vorschau dreht um ihren Mittelpunkt
    /// und bleibt oben rechts am Fadenkreuz, `base_point` = der Basispunkt steht
    /// am Fadenkreuz.
    #[serde(default)]
    pub paste_pivot: PastePivot,
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
    /// Sprache der Oberfläche (T0103), gespeichert als Sprachcode (`de`, `en`).
    /// Deutsch ist Quellsprache und Rückfall; die Umschaltung wirkt sofort,
    /// ohne Neustart (`jjcad_core::i18n::set_language`).
    #[serde(default)]
    pub language: crate::i18n::Lang,
    /// Fensterrahmen (T0098) — experimentell: Fenster **ohne Titelleiste**.
    /// Dann zeichnet JoJa-CAD Minimieren, Vergrößern und Schließen selbst oben
    /// rechts und die Kopfzeile verschiebt das Fenster (`prefs.json`).
    #[serde(default)]
    pub fensterrahmen: Fensterrahmen,
    /// Farbdesign der Oberfläche (T0102): `system` folgt dem Betriebssystem
    /// (Programmstandard), `hell` und `dunkel` sind feste Wahlen. Die Wahl
    /// wirkt sofort, ohne Neustart (`crate::theme::set_design`).
    #[serde(default)]
    pub design: Design,
    /// Farbakzent der Oberfläche (T0111): Bernstein (Programmstandard), Saphir,
    /// Smaragd, Rubin oder Amethyst. Er färbt alle Akzentstellen der
    /// Bedienung (Reiter, Chips, Rahmen, Schalter); die Wahl wirkt sofort
    /// (`crate::theme::set_akzent`).
    #[serde(default)]
    pub farbakzent: Farbakzent,
    /// Maximale Rastergröße der Schrift in der Zeichenfläche (T0114):
    /// 128, 256 (Vorgabe), 512 oder 1024 px. Höher = schärfer im hohen Zoom,
    /// aber mehr Schriftatlas. Gilt sofort, ohne Neustart. Steht seit T0165 in
    /// der Kategorie „Grafik“ — sie ist eine Leistungseinstellung (Atlasgröße),
    /// keine Darstellungsvorliebe.
    #[serde(default)]
    pub textaufloesung: Textaufloesung,
    /// Kantenglättung der gezeichneten Flächen und Linien (T0165): an
    /// (Vorgabe) zeichnet egui mit weicher Kante, aus endet jede Fläche hart
    /// auf dem Pixel — auf Rechnern ohne brauchbare Kantenglättung sichtbar
    /// sauberer und schneller. Gilt sofort.
    #[serde(default = "default_true")]
    pub kantenglaettung: bool,
    /// Weichheit der Kante in Bildpunkten (T0165): 1,0 ist die Vorgabe von
    /// egui, 0,5 eine schmale, 2,0 eine breite Kante. Gilt nur bei
    /// eingeschalteter [`UserPrefs::kantenglaettung`].
    #[serde(default = "default_kantenweichheit")]
    pub kantenweichheit: f32,
    /// Pfeilhöhe, mit der Rundungen (Kreis, Bogen, Ellipsenbogen) auf dem
    /// Bildschirm in Sehnen zerlegt werden (T0172), in Bildpunkten. Vorgabe
    /// 0,15 px — der Wert von [`crate::tessellate::CurveQuality::SCREEN`].
    /// Klein = glatter, groß = schneller; gilt sofort, ohne Neustart. Der
    /// Druck liest die Einstellung **nicht** (dort gilt
    /// [`crate::tessellate::CurveQuality::PRINT`]).
    #[serde(default = "default_kurven_toleranz")]
    pub kurven_toleranz_px: f64,
    /// Anzeigeliste je Zeichnungsstand zwischenspeichern (T0165, Vorgabe an):
    /// die aus dem Modell erzeugte Displayliste wird wiederverwendet, solange
    /// sich Zeichnung, Zoom und Bearbeitungsraum nicht ändern. Aus = jedes Bild
    /// baut sie neu; das ist langsamer und nur zum Messen sinnvoll.
    #[serde(default = "default_true")]
    pub anzeigeliste_cache: bool,
    /// Leistungsanzeige (Umschalt+F3) beim Start offen (T0165, Vorgabe aus).
    /// Die Taste schaltet sie weiterhin jederzeit um; die Wahl hier gilt beim
    /// nächsten Start.
    #[serde(default)]
    pub perf_anzeige: bool,
    /// Voreinstellungen der Linienstärke je Objektart (T0125):
    /// Name der Standardebene ([`crate::style::LAYER_DEFAULTS`]) → mm.
    /// Fehlt ein Eintrag, gilt 0,18 mm ([`crate::style::LINIENSTAERKE_VORGABE`]).
    /// Ältere `prefs.json` bleiben damit gültig — das Feld ist optional.
    #[serde(default)]
    pub layer_weights: BTreeMap<String, f64>,
    /// Speicherort der Benutzerblock-Bibliothek (T0127), leer = `blocks/` in
    /// der Datenwurzel. Relativ zur Wurzel oder absolut.
    #[serde(default)]
    pub blocks_pfad: String,
    /// Speicherort der Sicherungen (T0127), leer = `sicherungen/`.
    #[serde(default)]
    pub sicherungen_pfad: String,
    /// Speicherort der Protokolle (T0127), leer = `log/`.
    #[serde(default)]
    pub log_pfad: String,
    /// Eigene Vorlage für neue Dateien (T0162): Pfad einer `.jjcad`-Datei,
    /// leer = neue Zeichnungen beginnen leer. Relativ zur Datenwurzel oder
    /// absolut — wie die drei Speicherorte darüber.
    #[serde(default)]
    pub vorlage_pfad: String,
    /// Sicherung (T0129): „Verlauf sichern“. Aus = nur der neueste Stand je
    /// Dokument; Pins bleiben in beiden Fällen.
    #[serde(default = "default_true")]
    pub sicherung_verlauf: bool,
    /// Plätze im Pool (30 … 2000, Vorgabe 100) — für alle Dateien zusammen.
    #[serde(default = "default_sicherung_plaetze")]
    pub sicherung_plaetze: u32,
    /// „Reicht zurück“ in Arbeitsstunden (Vorgabe 480 = Quartal).
    #[serde(default = "default_sicherung_stunden")]
    pub sicherung_stunden: u32,
    /// Intervall zwischen zwei Sicherungen in Minuten (Vorgabe 1).
    #[serde(default = "default_sicherung_intervall")]
    pub sicherung_intervall: u32,
}

fn default_dateidialog() -> String {
    "system".to_string()
}

fn default_preset_idx() -> usize {
    0
}

fn default_sicherung_plaetze() -> u32 {
    crate::sicherung::PLAETZE_VORGABE
}

fn default_sicherung_stunden() -> u32 {
    crate::sicherung::STUNDEN_VORGABE
}

fn default_sicherung_intervall() -> u32 {
    crate::sicherung::INTERVALL_VORGABE
}
fn default_true() -> bool {
    true
}

/// Vorgabe der Schwenkgeschwindigkeit: unverändert das bisherige Tempo (T0067).
fn default_pan_speed() -> f32 {
    1.0
}

/// Vorgabe der Kantenweichheit in Bildpunkten (T0165) — der Wert, mit dem egui
/// seine Kantenglättung rechnet.
fn default_kantenweichheit() -> f32 {
    1.0
}

/// Vorgabe der Kurventoleranz in Bildpunkten (T0172) — die Pfeilhöhe, mit der
/// Rundungen in Sehnen zerlegt werden. Der Wert kommt aus der einen Quelle
/// [`crate::tessellate::CurveQuality::SCREEN`], damit Vorgabe und
/// Zeichenfläche nicht auseinanderlaufen.
fn default_kurven_toleranz() -> f64 {
    crate::tessellate::CurveQuality::SCREEN.max_sagitta
}

/// Grenzen der Kantenweichheit (T0165): darunter verschwindet die Glättung
/// (harte Kante, dann ist der Schalter sinnvoller), darüber wird jede Kante ein
/// weicher Saum. Eigene Grenzen, weil der Wert kein Papier-Maß ist.
pub const KANTENWEICHHEIT_MIN: f32 = 0.5;
/// Gegenstück zu [`KANTENWEICHHEIT_MIN`].
pub const KANTENWEICHHEIT_MAX: f32 = 2.0;

/// Grenzen der Kurventoleranz (T0172) in Bildpunkten: darunter kostet jede
/// Rundung Segmente ohne sichtbaren Gewinn (die Kantenglättung liegt bei 1 px),
/// darüber wird der Bogen sichtbar eckig. Eigene Grenzen, weil der Wert kein
/// Papier-Maß ist.
pub const KURVEN_TOLERANZ_MIN: f64 = 0.05;
/// Gegenstück zu [`KURVEN_TOLERANZ_MIN`].
pub const KURVEN_TOLERANZ_MAX: f64 = 2.0;

/// Grenzen der Schwenkgeschwindigkeit (T0067): höchstens ein Zehntel bis
/// fünffach — darunter kriecht die Ansicht, darüber ist kein Ziel mehr zu
/// treffen. Eigene Grenzen, weil der Wert kein Papier-Maß ist.
pub const SCHWENK_FAKTOR_MIN: f32 = 0.1;
/// Gegenstück zu [`SCHWENK_FAKTOR_MIN`].
pub const SCHWENK_FAKTOR_MAX: f32 = 5.0;

/// Grenzen des Linienstärke-Faktors im Modus „Bildschirm (konstant)“ (T0139):
/// halbe bis dreifache Strichdicke. Darunter verschwinden dünne Linien, darüber
/// deckt ein 1-mm-Strich die halbe Zeichnung zu.
pub const LINIENSTAERKE_FAKTOR_MIN: f32 = 0.5;
/// Gegenstück zu [`LINIENSTAERKE_FAKTOR_MIN`].
pub const LINIENSTAERKE_FAKTOR_MAX: f32 = 3.0;

/// Vorgabe des Linienstärke-Faktors: unverändert die bisherige Strichdicke.
fn default_lineweight_factor() -> f32 {
    1.0
}

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

/// Die Belegung der Vorlage JoJa-CAD: alle Kürzel aus dem Action-Register,
/// soweit sie in die freie Schicht gehören (T0056).
fn werkseinstellung_bindungen() -> BTreeMap<String, Vec<String>> {
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
    bindings
}

/// Baut eine Vorlage aus der JoJa-CAD-Belegung und den Abweichungen eines
/// fremden Programms (`Aktion → Taste`). Eine Taste trägt genau eine Funktion:
/// die Zuweisung nimmt sie deshalb jeder anderen Funktion weg. Eine leere Taste
/// nimmt der Aktion ihr Kürzel (die Funktion bleibt über Ribbon und Inventar
/// erreichbar).
fn vorlage(name: &str, abweichungen: &[(&str, &str)]) -> KeymapPreset {
    let mut p = KeymapPreset {
        name: name.to_string(),
        bindings: werkseinstellung_bindungen(),
    };
    for (aktion, taste) in abweichungen {
        p.bindings.remove(*aktion);
        if taste.is_empty() {
            continue;
        }
        let taste = taste.to_lowercase();
        for keys in p.bindings.values_mut() {
            keys.retain(|k| k.to_lowercase() != taste);
        }
        p.bindings
            .entry((*aktion).to_string())
            .or_default()
            .push(taste);
    }
    p.bindings.retain(|_, keys| !keys.is_empty());
    p
}

/// Abweichungen der Vorlage Allplan. Quelle: Allplan-Kurzanleitung „Shortcuts"
/// (deutsch): Linie `Strg+L`, Maßlinie `Strg+M`, Wand `Strg+W`, Decke `Strg+D`,
/// Tür `Strg+T`, Fenster `Strg+F`. Tastenkombinationen sind in JoJa-CAD fest
/// verdrahtet (`docs/TASTEN.md`) — die Vorlage nimmt den Buchstaben dahinter.
/// Text liegt deshalb auf `Umschalt+T`, Spiegeln auf `Umschalt+F`, und der
/// Ansichtsschwenk wandert auf die Umschalt-Ebene.
const VORLAGE_ALLPLAN: &[(&str, &str)] = &[
    ("draw.line", "l"),
    ("draw.wall", "w"),
    ("draw.door", "t"),
    ("draw.window", "f"),
    ("draw.text", "shift+t"),
    ("draw.fill", "d"),
    ("dim.linear", "m"),
    ("modify.mirror", "shift+f"),
    ("modify.trim", "c"),
    ("modify.explode", "z"),
    ("view.pan_up", "shift+w"),
    ("view.pan_left", "shift+a"),
    ("view.pan_down", "shift+s"),
    ("view.pan_right", "shift+d"),
];

/// Abweichungen der Vorlage ArchiCAD. Quelle: „ARCHICAD Default Keyboard
/// Shortcuts" (Graphisoft): Wand `W`, Fenster `,`, Tür `M`, Linie
/// `Umschalt+L`, Polylinie `L`, Text `K`, Maß `J`, Höhenkote `Umschalt+J`,
/// Versetzen `O`, Verschieben (Drag) `Strg+D`, Kopieren `Strg+Umschalt+D`,
/// Drehen `Strg+E`, Spiegeln `Strg+M`, Zoom `+`/`-`. `;` (Bogen/Kreis) und
/// `’` (Füllung) sind in JoJa-CAD nicht belegbar (zweite Ebene) — der Kreis
/// liegt deshalb auf `C`, die Füllung auf `H`.
const VORLAGE_ARCHICAD: &[(&str, &str)] = &[
    ("draw.wall", "w"),
    ("draw.window", ","),
    ("draw.door", "m"),
    ("draw.stair", "."),
    ("draw.polyline", "l"),
    ("draw.line", "shift+l"),
    ("draw.text", "k"),
    ("draw.circle", "c"),
    ("draw.fill.paint", "h"),
    ("dim.linear", "j"),
    ("dim.elevation.plan", "shift+j"),
    ("modify.offset", "o"),
    ("modify.move", "d"),
    ("modify.copy", "shift+c"),
    ("modify.rotate", "shift+e"),
    ("modify.mirror", "shift+m"),
    ("modify.explode", "z"),
    ("view.zoom_in", "+"),
    ("view.zoom_out", "-"),
    ("view.pan_up", "shift+w"),
    ("view.pan_left", "shift+a"),
    ("view.pan_down", "shift+s"),
    ("view.pan_right", "shift+d"),
];

/// Abweichungen der Vorlage AutoCAD. Quelle: AutoCAD-Befehlskürzel (`acad.pgp`
/// und Shortcuts-Guide): `LINE`=L, `PLINE`=P, `ARC`=A, `CIRCLE`=C, `TEXT`/
/// `MTEXT`=T, `MOVE`=M, `ROTATE`=R, `OFFSET`=O, `SCALE`=S, `EXPLODE`=X,
/// `HATCH`=H, `DIM`=D, `ZOOM`=Z, `INSERT`=I, `BLOCK`=B. Mehrbuchstabige
/// Kürzel (`TR`, `CO`, `MI`, `EX`) sind nicht tippbar; die Vorlage nimmt die
/// Umschalt-Ebene (`Umschalt+T` Stutzen, `Umschalt+C` Kopieren, `Umschalt+M`
/// Spiegeln) bzw. den ersten Buchstaben (`E` Verlängern).
const VORLAGE_AUTOCAD: &[(&str, &str)] = &[
    ("draw.line", "l"),
    ("draw.polyline", "p"),
    ("draw.arc", "a"),
    ("draw.circle", "c"),
    ("draw.text", "t"),
    ("draw.fill.paint", "h"),
    ("draw.door", "shift+h"),
    ("draw.point", ""),
    ("block.insert", "i"),
    ("block.create", "b"),
    ("modify.move", "m"),
    ("modify.copy", "shift+c"),
    ("modify.rotate", "r"),
    ("modify.mirror", "shift+m"),
    ("modify.offset", "o"),
    ("modify.scale", "s"),
    ("modify.extend", "shift+e"),
    ("modify.explode", "x"),
    ("modify.trim", "shift+t"),
    ("dim.linear", "d"),
    ("view.zoom_extents", "z"),
    ("view.pan_up", "shift+w"),
    ("view.pan_left", "shift+a"),
    ("view.pan_down", "shift+s"),
    ("view.pan_right", "shift+d"),
];

/// Die vier eingebauten Vorlagen (T0079): erst die eigene Belegung, dann die
/// angenäherten Vorgaben von Allplan, ArchiCAD und AutoCAD.
pub fn vorlagen() -> Vec<KeymapPreset> {
    vec![
        KeymapPreset {
            name: VORLAGEN[0].to_string(),
            bindings: werkseinstellung_bindungen(),
        },
        vorlage(VORLAGEN[1], VORLAGE_ALLPLAN),
        vorlage(VORLAGEN[2], VORLAGE_ARCHICAD),
        vorlage(VORLAGEN[3], VORLAGE_AUTOCAD),
    ]
}

impl Default for UserPrefs {
    fn default() -> Self {
        let bindings = werkseinstellung_bindungen();
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
                        "dim.elevation.section",
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
            // Die vier eingebauten Vorlagen (T0079); die erste ist die
            // Vorgabebelegung, auf die jederzeit zurückgeschaltet werden kann.
            keymap_presets: vorlagen(),
            keymap_aktiv: Some(0),
            recent_files: Vec::new(),
            last_dir: String::new(),
            show_lineweights: true,
            lineweights_scaled: false,
            lineweight_factor: default_lineweight_factor(),
            drag_mode: DragMode::HoldAndDrag,
            select_mode: DragMode::HoldAndDrag,
            mouse_move: true,
            pan_speed: default_pan_speed(),
            // T0101: Die Ansicht folgt der gedrückten Schwenkfunktion, statt
            // bei jedem Druck um einen Schritt zu springen.
            pan_mode: PanMode::Kontinuierlich,
            // Vorgabe (T0087): die Vorschau bleibt oben rechts am Fadenkreuz.
            paste_pivot: PastePivot::Center,
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
            language: crate::i18n::Lang::Deutsch,
            fensterrahmen: Fensterrahmen::System,
            // Programmstandard (T0102): das Farbschema des Betriebssystems.
            design: Design::System,
            // Programmstandard (T0111): der Bernstein-Akzent (bisheriges
            // Aussehen).
            farbakzent: Farbakzent::Bernstein,
            // Programmstandard (T0114): 256 px Rastergröße — der Atlas bleibt
            // klein, kleine Schriften sind bei jedem Zoom scharf.
            textaufloesung: Textaufloesung::Px256,
            // T0165: die Zeichnung wird weich gezeichnet (bisheriges Verhalten)
            // und die Anzeigeliste wiederverwendet; die Leistungsanzeige ist aus.
            kantenglaettung: true,
            kantenweichheit: default_kantenweichheit(),
            kurven_toleranz_px: default_kurven_toleranz(),
            anzeigeliste_cache: true,
            perf_anzeige: false,
            // T0125: ohne eigene Wahl trägt jede Objektart 0,18 mm.
            layer_weights: BTreeMap::new(),
            // T0127: ohne eigene Angabe liegen alle Unterordner in der Wurzel.
            blocks_pfad: String::new(),
            sicherungen_pfad: String::new(),
            sicherung_verlauf: true,
            sicherung_plaetze: crate::sicherung::PLAETZE_VORGABE,
            sicherung_stunden: crate::sicherung::STUNDEN_VORGABE,
            sicherung_intervall: crate::sicherung::INTERVALL_VORGABE,
            log_pfad: String::new(),
            // T0162: ohne eigene Wahl beginnt eine neue Zeichnung leer.
            vorlage_pfad: String::new(),
        }
    }
}

impl UserPrefs {
    /// Voreingestellte Linienstärke einer Objektart (T0125) — der gesetzte
    /// Wert oder 0,18 mm, wenn der Nutzer nichts gewählt hat.
    pub fn linienstaerke(&self, ebene: &str) -> f64 {
        self.layer_weights
            .get(ebene)
            .copied()
            .filter(|w| w.is_finite() && *w > 0.0)
            .unwrap_or(crate::style::LINIENSTAERKE_VORGABE)
    }

    /// Setzt die Voreinstellung einer Objektart; der Vorgabewert 0,18 mm wird
    /// **nicht** gespeichert, damit die Datei schlank bleibt und spätere
    /// Änderungen an [`crate::style::LINIENSTAERKE_VORGABE`] greifen.
    pub fn set_linienstaerke(&mut self, ebene: &str, wert: f64) {
        if (wert - crate::style::LINIENSTAERKE_VORGABE).abs() < 1e-9 {
            self.layer_weights.remove(ebene);
        } else {
            self.layer_weights.insert(ebene.to_string(), wert);
        }
    }

    /// „Vorgaben zurücksetzen“: alle Objektarten wieder auf 0,18 mm.
    pub fn linienstaerken_zuruecksetzen(&mut self) {
        self.layer_weights.clear();
    }

    /// Benutzerblock-Bibliothek (T0127): eigener Pfad oder `blocks/` in der
    /// Datenwurzel.
    pub fn blocks_dir(&self) -> PathBuf {
        crate::daten::pfad_in_wurzel(&self.blocks_pfad, "blocks")
    }

    /// Sicherungen (T0127): eigener Pfad oder `sicherungen/` in der Wurzel.
    pub fn sicherungen_dir(&self) -> PathBuf {
        crate::daten::pfad_in_wurzel(&self.sicherungen_pfad, "sicherungen")
    }

    /// Einstellungen der Sicherung (T0129) in der Form, die der Pool erwartet.
    /// Werte außerhalb der Grenzen werden geklemmt — eine von Hand bearbeitete
    /// `prefs.json` soll die Ausdünnung nicht lahmlegen.
    pub fn sicherung_einstellungen(&self) -> crate::sicherung::Einstellungen {
        use crate::sicherung::{INTERVALL_MAX, INTERVALL_MIN, PLAETZE_MAX, PLAETZE_MIN};
        crate::sicherung::Einstellungen {
            verlauf: self.sicherung_verlauf,
            plaetze: self.sicherung_plaetze.clamp(PLAETZE_MIN, PLAETZE_MAX),
            stunden: self.sicherung_stunden.max(1),
            intervall_min: self.sicherung_intervall.clamp(INTERVALL_MIN, INTERVALL_MAX),
        }
    }

    /// Protokolle (T0127): eigener Pfad oder `log/` in der Wurzel.
    pub fn log_dir(&self) -> PathBuf {
        crate::daten::pfad_in_wurzel(&self.log_pfad, "log")
    }

    /// Ausdrücklicher Speicherort einer der drei ändbaren Angaben
    /// (`"blocks"`, `"sicherungen"`, `"log"`) — leer, wenn die Vorgabe gilt.
    pub fn speicherort(&self, feld: SpeicherortFeld) -> &str {
        match feld {
            SpeicherortFeld::Bloecke => &self.blocks_pfad,
            SpeicherortFeld::Sicherungen => &self.sicherungen_pfad,
            SpeicherortFeld::Protokolle => &self.log_pfad,
        }
    }

    /// Setzt einen Speicherort; ein leerer Text stellt die Vorgabe her
    /// („Zurücksetzen“).
    pub fn setze_speicherort(&mut self, feld: SpeicherortFeld, wert: &str) {
        let ziel = match feld {
            SpeicherortFeld::Bloecke => &mut self.blocks_pfad,
            SpeicherortFeld::Sicherungen => &mut self.sicherungen_pfad,
            SpeicherortFeld::Protokolle => &mut self.log_pfad,
        };
        *ziel = wert.trim().to_string();
    }

    /// Aufgelöster Speicherort eines der drei Felder.
    pub fn speicherort_pfad(&self, feld: SpeicherortFeld) -> PathBuf {
        match feld {
            SpeicherortFeld::Bloecke => self.blocks_dir(),
            SpeicherortFeld::Sicherungen => self.sicherungen_dir(),
            SpeicherortFeld::Protokolle => self.log_dir(),
        }
    }

    /// Vorlagedatei für neue Zeichnungen (T0162) — `None`, wenn keine gewählt
    /// ist. Ein relativer Pfad hängt an der Datenwurzel, wie die Speicherorte.
    pub fn vorlage_datei(&self) -> Option<PathBuf> {
        let text = self.vorlage_pfad.trim();
        if text.is_empty() {
            return None;
        }
        Some(crate::daten::pfad_in_wurzel(text, ""))
    }

    /// Soll der Dateidialog des Betriebssystems benutzt werden? Vorgabe ist
    /// „system“; nur ein ausdrückliches "intern" wählt den eigenen Dialog.
    pub fn dateidialog_system(&self) -> bool {
        self.file_dialog != "intern"
    }

    /// `prefs.json` liegt immer in der Datenwurzel (T0127) — nicht änderbar.
    pub fn path() -> PathBuf {
        crate::daten::prefs_datei()
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
        // T0139: derselbe Schutz für den Linienstärke-Faktor — ein alter oder
        // fremder Wert darf die Zeichnung weder unsichtbar noch zudecken.
        self.lineweight_factor = if self.lineweight_factor.is_finite() {
            self.lineweight_factor
                .clamp(LINIENSTAERKE_FAKTOR_MIN, LINIENSTAERKE_FAKTOR_MAX)
        } else {
            default_lineweight_factor()
        };
        // T0172: dasselbe für die Kurventoleranz — ein alter oder fremder Wert
        // darf die Rundungen weder in Segmentlawinen noch in Ecken verwandeln.
        self.kurven_toleranz_px = if self.kurven_toleranz_px.is_finite() {
            self.kurven_toleranz_px
                .clamp(KURVEN_TOLERANZ_MIN, KURVEN_TOLERANZ_MAX)
        } else {
            default_kurven_toleranz()
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
        // T0079: `C` ist das Stutzen, `T` der Text, `Z` das Zerlegen (so steht es
        // in der Aufgabenstellung). Der Farbeimer lag früher auf `C` und wandert
        // deshalb auf Umschalt+C — sonst nähme seine gespeicherte Belegung dem
        // Stutzen die Taste weg. `T` gehörte bis T0009 dem Chat-Protokoll; das
        // liegt inzwischen auf `^`, eine alte Belegung darf den Text also nicht
        // mehr von der Taste verdrängen.
        if let Some(keys) = self.bindings.get_mut("draw.fill.paint") {
            for k in keys.iter_mut() {
                if k.eq_ignore_ascii_case("c") {
                    *k = "shift+c".into();
                }
            }
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
        // T0079: Die vier eingebauten Vorlagen stehen immer vorn und tragen
        // immer die Vorgabebelegung — der Rückweg, den früher der Knopf
        // „Werkseinstellungen" war. Eine alte `prefs.json` kennt nur
        // „Werkseinstellung"; eigene Layouts des Nutzers bleiben **hinter** den
        // Vorlagen stehen und ihre Reihenfolge bleibt erhalten.
        let aktiv_vorher = self
            .keymap_aktiv
            .and_then(|i| self.keymap_presets.get(i))
            .map(|p| p.name.clone());
        let mut eigene: Vec<KeymapPreset> = self
            .keymap_presets
            .drain(..)
            .filter(|p| !ist_vorlage(&p.name))
            .collect();
        let mut presets = vorlagen();
        presets.append(&mut eigene);
        self.keymap_presets = presets;
        // Der gemerkte Index verschiebt sich durch die Vorlagen — über den
        // Namen wiederfinden, nicht über die alte Zahl.
        self.keymap_aktiv = aktiv_vorher
            .and_then(|name| self.keymap_presets.iter().position(|p| p.name == name));
        // Der gemerkte Index muss auf ein vorhandenes Layout zeigen. Eine alte
        // `prefs.json` kennt das Feld nicht — dann gilt das Layout, dessen
        // Belegung der aktuellen entspricht (Vorgabe: die Werkseinstellung).
        self.keymap_aktiv = match self.keymap_aktiv {
            Some(i) if i < self.keymap_presets.len() => Some(i),
            _ => self
                .keymap_presets
                .iter()
                .position(|p| p.bindings == self.bindings),
        };
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
        // Von Hand geändert: das gemerkte Layout gilt nicht mehr (T0076).
        self.keymap_aktiv = None;
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
        if aktion.is_some() {
            self.keymap_aktiv = None;
        }
        for keys in self.bindings.values_mut() {
            keys.retain(|s| s.to_lowercase() != k);
        }
        self.bindings.retain(|_, keys| !keys.is_empty());
        aktion
    }

    /// Nimmt der Funktion **alle** Tasten.
    pub fn unbind_action(&mut self, action: &str) {
        if self.bindings.remove(action).is_some() {
            self.keymap_aktiv = None;
        }
    }

    pub fn reset_bindings(&mut self) {
        self.bindings = UserPrefs::default().bindings;
        self.keymap_aktiv = Some(0);
    }

    // ---- Tastaturlayouts (T0076) -------------------------------------------

    /// Das gerade gültige Layout — `None` = „Eigene Belegung" (von Hand
    /// geändert). Immer der **ausdrücklich** gemerkte Eintrag, nicht der erste
    /// mit gleicher Belegung: zwei Layouts dürfen dieselben Tasten tragen, die
    /// Auswahl muss trotzdem den zuletzt gespeicherten Namen zeigen (T0076).
    pub fn keymap_aktiv(&self) -> Option<usize> {
        self.keymap_aktiv
            .filter(|i| *i < self.keymap_presets.len())
    }

    /// Trägt die aktuelle Belegung ein Layout, das noch nicht gespeichert ist?
    /// Dann fragt der Wechsel auf ein anderes Layout nach (Abbrechen, Verwerfen,
    /// Speichern — T0076). Eine gelöschte Belegung, die zufällig einem anderen
    /// Layout entspricht, gilt als gespeichert.
    pub fn keymap_ungespeichert(&self) -> bool {
        self.keymap_aktiv().is_none()
            && !self
                .keymap_presets
                .iter()
                .any(|p| p.bindings == self.bindings)
    }

    /// Schaltet auf ein gespeichertes Layout um und liefert seinen Namen.
    /// Unbekannter Index oder ein fehlendes Layout ändern nichts.
    pub fn keymap_anwenden(&mut self, idx: usize) -> Option<String> {
        let p = self.keymap_presets.get(idx)?;
        let name = p.name.clone();
        self.bindings = p.bindings.clone();
        self.keymap_aktiv = Some(idx);
        Some(name)
    }

    /// Speichert die **aktuelle** Belegung unter `name`. Die Namen der
    /// eingebauten Vorlagen sind belegt (T0079); ein anderer vorhandener Name
    /// wird überschrieben (kein zweiter Eintrag).
    pub fn keymap_speichern(&mut self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Bitte einen Namen angeben".into());
        }
        if ist_vorlage(name) {
            return Err(format!("„{name}“ ist eine eingebaute Vorlage"));
        }
        let idx = match self
            .keymap_presets
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
        {
            Some(i) => {
                self.keymap_presets[i].bindings = self.bindings.clone();
                i
            }
            None => {
                self.keymap_presets.push(KeymapPreset {
                    name: name.to_string(),
                    bindings: self.bindings.clone(),
                });
                self.keymap_presets.len() - 1
            }
        };
        // Ab jetzt gilt genau dieses Layout — auch wenn seine Belegung mit
        // einem anderen Eintrag übereinstimmt.
        self.keymap_aktiv = Some(idx);
        Ok(name.to_string())
    }

    /// Löscht ein gespeichertes Layout. Die eingebauten Vorlagen bleiben
    /// stehen (T0079). Wird das gerade benutzte Layout gelöscht, bleiben dessen
    /// Tasten gültig, aber es gilt danach „Eigene Belegung".
    pub fn keymap_loeschen(&mut self, idx: usize) -> bool {
        if idx < VORLAGEN.len() || idx >= self.keymap_presets.len() {
            return false;
        }
        self.keymap_presets.remove(idx);
        self.keymap_aktiv = match self.keymap_aktiv {
            Some(a) if a == idx => None,
            Some(a) if a > idx => Some(a - 1),
            other => other,
        };
        true
    }

    /// Schreibt ein Layout als Datei — dieselbe Form, die
    /// [`UserPrefs::keymap_importieren`] wieder liest.
    pub fn keymap_exportieren(&self, idx: usize, path: &Path) -> Result<String, String> {
        let p = self
            .keymap_presets
            .get(idx)
            .ok_or_else(|| "Layout nicht gefunden".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, p.to_json()?)
            .map_err(|e| format!("Layout konnte nicht geschrieben werden: {e}"))?;
        Ok(p.name.clone())
    }

    /// Schreibt die **aktuelle** Belegung als Datei — auch dann, wenn sie zu
    /// keinem gespeicherten Layout gehört („Eigene Belegung"). Liefert den
    /// Namen, unter dem sie in der Datei steht.
    pub fn keymap_exportieren_aktuell(&self, path: &Path) -> Result<String, String> {
        let name = self
            .keymap_aktiv()
            .map(|i| self.keymap_presets[i].name.clone())
            .unwrap_or_else(|| "Eigene Belegung".to_string());
        let p = KeymapPreset {
            name: name.clone(),
            bindings: self.bindings.clone(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, p.to_json()?)
            .map_err(|e| format!("Layout konnte nicht geschrieben werden: {e}"))?;
        Ok(name)
    }

    /// Liest ein Layout aus einer Datei und **wendet es sofort an**. Ein bereits
    /// vorhandener Name wird überschrieben; die Werkseinstellung bleibt
    /// unangetastet. Liefert den Namen des geladenen Layouts.
    pub fn keymap_importieren(&mut self, path: &Path) -> Result<String, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Layout konnte nicht gelesen werden: {e}"))?;
        let stamm = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut p = KeymapPreset::from_json(&text, &stamm)?;
        if ist_vorlage(&p.name) {
            p.name = format!("{} (importiert)", p.name);
        }
        let bindings = p.bindings.clone();
        let idx = match self
            .keymap_presets
            .iter()
            .position(|q| q.name.eq_ignore_ascii_case(&p.name))
        {
            Some(i) => {
                self.keymap_presets[i] = p.clone();
                i
            }
            None => {
                self.keymap_presets.push(p.clone());
                self.keymap_presets.len() - 1
            }
        };
        self.bindings = bindings;
        self.keymap_aktiv = Some(idx);
        Ok(p.name)
    }

    pub fn add_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(12);
    }
}

/// Konfigurationsverzeichnis des Betriebssystems — **eine** Auflösung für
/// alles, was dort liegt (T0127): das alte JoJa-CAD-Verzeichnis und die
/// Systemdateien, die `theme.rs` liest (`kdeglobals`, `settings.ini`).
pub fn xdg_config_home() -> PathBuf {
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata);
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        return PathBuf::from(home).join("AppData").join("Roaming");
    }
    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        return PathBuf::from(home).join("Library").join("Application Support");
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config")
}

/// **Alter** Ort der Einstellungen (`~/.config/jjcad` bzw.
/// `%APPDATA%\JoJa-CAD`). Seit T0127 liegen die Nutzdaten in
/// [`crate::daten::wurzel`]; hier wird nur noch beim ersten Start eine
/// vorhandene `prefs.json` herüberkopiert (`daten::prefs_uebernehmen`).
pub fn config_dir() -> PathBuf {
    xdg_config_home().join("jjcad")
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

    /// T0127: Die drei Speicherorte überleben Speichern und Laden; ein leeres
    /// Feld bedeutet die Vorgabe in der Datenwurzel („Zurücksetzen“).
    #[test]
    fn speicherorte_ueberleben_und_lassen_sich_zuruecksetzen() {
        use crate::persist::SpeicherortFeld;
        let mut prefs = UserPrefs::default();
        assert_eq!(prefs.blocks_pfad, "");
        assert!(prefs.blocks_dir().ends_with("blocks"));

        prefs.setze_speicherort(SpeicherortFeld::Bloecke, "eigene/bloecke");
        prefs.setze_speicherort(SpeicherortFeld::Protokolle, "/var/log/jjcad");
        let dir = test_dir("speicherorte");
        let p = dir.join("prefs.json");
        prefs.save_to(&p).unwrap();

        let wieder = UserPrefs::load_from(&p);
        assert_eq!(wieder.blocks_pfad, "eigene/bloecke");
        assert_eq!(wieder.log_pfad, "/var/log/jjcad");
        assert!(
            wieder.blocks_dir().ends_with("eigene/bloecke"),
            "relativer Pfad hängt an der Wurzel: {:?}",
            wieder.blocks_dir()
        );
        assert_eq!(wieder.log_dir(), PathBuf::from("/var/log/jjcad"));

        // Altbestand ohne die Felder bleibt gültig.
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.blocks_pfad, "");
        assert!(alt.blocks_dir().ends_with("blocks"));

        // „Zurücksetzen“ stellt die Vorgabe her.
        let mut zurueck = wieder;
        zurueck.setze_speicherort(SpeicherortFeld::Bloecke, "");
        assert_eq!(zurueck.blocks_pfad, "");
        assert!(zurueck.blocks_dir().ends_with("blocks"));
    }

    /// T0162: Eine Vorlagendatei wird als **neue** Zeichnung geladen — der
    /// Inhalt bleibt, Pfad und Kennung sind neu, „geändert“ ist aus. Eine
    /// fehlende Vorlage meldet den Grund (kein stiller Fehlschlag).
    #[test]
    fn vorlage_wird_als_neue_zeichnung_geladen() {
        let dir = test_dir("vorlage");
        let pfad = dir.join("Vorlage.jjcad");
        let mut doc = Document::new();
        crate::actions::execute(
            &mut doc,
            "draw.line",
            &serde_json::json!({"a": [0.0, 0.0], "b": [1000.0, 0.0]}),
        )
        .expect("Linie anlegen");
        let vorlage_id = doc.id.clone();
        save_document(&mut doc, &pfad).expect("Vorlage schreiben");
        assert_eq!(doc.entities.len(), 1);

        let kopie = lade_vorlage(&pfad).expect("Vorlage laden");
        assert_eq!(kopie.entities.len(), 1, "der Inhalt der Vorlage fehlt");
        assert!(kopie.path.is_none(), "die Kopie hängt an der Vorlagendatei");
        assert_eq!(kopie.name, "Unbenannt", "die Kopie trägt den Dateinamen");
        assert!(!kopie.modified, "eine frische Kopie gilt als geändert");
        assert_ne!(
            kopie.id, vorlage_id,
            "die Kopie teilt die Kennung der Vorlage"
        );
        assert!(!kopie.layers.is_empty(), "die Ebenen der Vorlage fehlen");

        // Die Vorlagedatei selbst bleibt unberührt.
        let wieder = load_document(&pfad).expect("Vorlage erneut lesen");
        assert_eq!(wieder.id, vorlage_id);

        let fehlt = dir.join("gibtesnicht.jjcad");
        assert!(
            lade_vorlage(&fehlt).is_err(),
            "eine fehlende Vorlage wird stillschweigend geladen"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T0162: Die Vorlage überlebt Speichern und Laden; ohne Angabe gibt es
    /// keine (`None`), ein relativer Pfad hängt an der Datenwurzel.
    #[test]
    fn vorlagenpfad_wird_gespeichert_und_aufgeloest() {
        let mut prefs = UserPrefs::default();
        assert!(prefs.vorlage_datei().is_none(), "die Vorgabe ist leer");

        prefs.vorlage_pfad = "/tmp/Vorlage.jjcad".into();
        let dir = test_dir("vorlagenpfad");
        let p = dir.join("prefs.json");
        prefs.save_to(&p).unwrap();
        let wieder = UserPrefs::load_from(&p);
        assert_eq!(wieder.vorlage_pfad, "/tmp/Vorlage.jjcad");
        assert_eq!(
            wieder.vorlage_datei(),
            Some(PathBuf::from("/tmp/Vorlage.jjcad"))
        );

        // Altbestand ohne den Schlüssel bleibt gültig.
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.vorlage_pfad, "");
        assert!(alt.vorlage_datei().is_none());

        // Relativ heißt: in der Datenwurzel.
        let mut relativ = UserPrefs::default();
        relativ.vorlage_pfad = "vorlagen/Haus.jjcad".into();
        let aufgeloest = relativ.vorlage_datei().expect("Vorlage");
        assert!(
            aufgeloest.ends_with("vorlagen/Haus.jjcad"),
            "relativer Pfad: {aufgeloest:?}"
        );
        assert!(aufgeloest.is_absolute(), "relativer Pfad bleibt relativ");
        let _ = std::fs::remove_dir_all(&dir);
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

    /// T0165: Die Grafikeinstellungen der Kategorie „Grafik“ stehen in den
    /// Voreinstellungen. Kantenglättung und Anzeigeliste sind in der Vorgabe
    /// **an** (bisheriges Verhalten), die Leistungsanzeige **aus**; ein
    /// Altbestand ohne die Schlüssel bekommt dieselben Vorgaben, und alle vier
    /// Werte überleben Speichern und Laden.
    #[test]
    fn grafikeinstellungen_vorgabe_und_gespeichert() {
        let standard = UserPrefs::default();
        assert!(standard.kantenglaettung, "Kantenglättung muss an sein");
        assert_eq!(standard.kantenweichheit, 1.0, "Kantenweichheit");
        // T0172: die Kurventoleranz ist die Pfeilhöhe der Zeichenfläche.
        assert_eq!(standard.kurven_toleranz_px, 0.15, "Kurventoleranz");
        assert!(standard.anzeigeliste_cache, "Anzeigeliste muss gespeichert werden");
        assert!(!standard.perf_anzeige, "Leistungsanzeige ist in der Vorgabe aus");

        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert!(alt.kantenglaettung && alt.anzeigeliste_cache);
        assert!(!alt.perf_anzeige);
        assert_eq!(alt.kantenweichheit, 1.0);
        assert_eq!(alt.kurven_toleranz_px, 0.15, "Altbestand ohne Schlüssel");

        let mut prefs = UserPrefs::default();
        prefs.kantenglaettung = false;
        prefs.kantenweichheit = 1.75;
        prefs.kurven_toleranz_px = 0.5;
        prefs.anzeigeliste_cache = false;
        prefs.perf_anzeige = true;
        let roh = serde_json::to_string(&prefs).unwrap();
        for schluessel in [
            "\"kantenglaettung\":false",
            "\"kantenweichheit\":1.75",
            "\"kurven_toleranz_px\":0.5",
            "\"anzeigeliste_cache\":false",
            "\"perf_anzeige\":true",
        ] {
            assert!(roh.contains(schluessel), "{schluessel} fehlt: {roh}");
        }
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert!(!wieder.kantenglaettung);
        assert_eq!(wieder.kantenweichheit, 1.75);
        assert_eq!(wieder.kurven_toleranz_px, 0.5);
        assert!(!wieder.anzeigeliste_cache);
        assert!(wieder.perf_anzeige);
    }

    /// T0172: Ein fremder Wert in `prefs.json` wird beim Laden auf die Grenzen
    /// geklemmt — 0,05 … 2,0 px. Darunter wäre jede Rundung eine Segmentlawine,
    /// darüber sichtbar eckig; ein unbrauchbarer Wert fällt auf die Vorgabe.
    #[test]
    fn kurventoleranz_wird_beim_laden_geklemmt() {
        for (eingabe, erwartet) in [
            (0.001, KURVEN_TOLERANZ_MIN),
            (0.05, 0.05),
            (0.5, 0.5),
            (2.0, 2.0),
            (99.0, KURVEN_TOLERANZ_MAX),
        ] {
            let mut p: UserPrefs =
                serde_json::from_str(&format!(r#"{{"kurven_toleranz_px": {eingabe}}}"#)).unwrap();
            p.ensure_defaults();
            assert!(
                (p.kurven_toleranz_px - erwartet).abs() < 1e-9,
                "{eingabe} ergibt {}, erwartet {erwartet}",
                p.kurven_toleranz_px
            );
        }
        // Ein Text statt einer Zahl lässt den ganzen Eintrag scheitern; dann
        // gilt die Vorgabe.
        let p: UserPrefs = serde_json::from_str(r#"{"kurven_toleranz_px": "fein"}"#)
            .unwrap_or_default();
        assert_eq!(p.kurven_toleranz_px, 0.15);
    }

    /// T0098: Der Fensterrahmen steht in den Voreinstellungen. Vorgabe ist die
    /// Titelleiste des Fenstermanagers; ein Altbestand ohne den Schlüssel
    /// bleibt dabei, der Modus „ohne Titelleiste“ überlebt Speichern und Laden.
    #[test]
    fn fensterrahmen_vorgabe_und_gespeichert() {
        assert_eq!(Fensterrahmen::default(), Fensterrahmen::System);
        assert!(!UserPrefs::default().fensterrahmen.ohne_titelleiste());
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(
            alt.fensterrahmen,
            Fensterrahmen::System,
            "Altbestand ohne Schlüssel behält die Titelleiste"
        );

        let mut prefs = UserPrefs::default();
        prefs.fensterrahmen = Fensterrahmen::OhneTitelleiste;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(
            roh.contains("\"fensterrahmen\":\"ohne_titelleiste\""),
            "Schlüssel fehlt oder ist unlesbar: {roh}"
        );
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert_eq!(
            wieder.fensterrahmen,
            Fensterrahmen::OhneTitelleiste,
            "der Fensterrahmen ging beim Laden verloren"
        );
        assert!(Fensterrahmen::OhneTitelleiste.ohne_titelleiste());
    }

    /// T0103: Die Sprache der Oberfläche steht in den Voreinstellungen, wird
    /// als Sprachcode gespeichert und überlebt Laden und Speichern. Ein
    /// Altbestand ohne Schlüssel gilt als Deutsch.
    #[test]
    fn sprache_wird_gespeichert_und_gelesen() {
        assert_eq!(UserPrefs::default().language, crate::i18n::Lang::Deutsch);
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.language, crate::i18n::Lang::Deutsch);

        let mut prefs = UserPrefs::default();
        prefs.language = crate::i18n::Lang::Englisch;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(roh.contains("\"language\":\"en\""), "Schlüssel fehlt: {roh}");
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert_eq!(
            wieder.language,
            crate::i18n::Lang::Englisch,
            "Sprachwahl ging beim Laden verloren"
        );
    }

    /// T0102: Das Farbdesign steht in den Voreinstellungen. Vorgabe ist
    /// „System“ (dem Betriebssystem folgen) — auch für Altbestände ohne
    /// Schlüssel; „hell“ und „dunkel“ werden gespeichert und wieder gelesen.
    #[test]
    fn design_wird_gespeichert_und_gelesen() {
        assert_eq!(
            UserPrefs::default().design,
            Design::System,
            "Vorgabe muss das Betriebssystem sein"
        );
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.design, Design::System);

        let mut prefs = UserPrefs::default();
        prefs.design = Design::Hell;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(
            roh.contains("\"design\":\"hell\""),
            "Schlüssel fehlt: {roh}"
        );
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert_eq!(wieder.design, Design::Hell, "Designwahl ging verloren");

        let dunkel: UserPrefs = serde_json::from_str(r#"{"design": "dunkel"}"#).unwrap();
        assert_eq!(dunkel.design, Design::Dunkel);
        // Drei Wahlen, deutsche Namen; „System“ ist die erste.
        assert_eq!(Design::ALL.len(), 3);
        assert_eq!(Design::ALL[0], Design::System);
        assert_eq!(Design::Hell.label(), "Hell");
        assert_eq!(Design::Dunkel.label(), "Dunkel");
    }

    /// T0111: Der Farbakzent steht in den Voreinstellungen. Vorgabe ist
    /// „Bernstein“ (das bisherige Aussehen) — auch für Altbestände ohne
    /// Schlüssel; jede andere Wahl wird gespeichert und wieder gelesen.
    #[test]
    fn farbakzent_wird_gespeichert_und_gelesen() {
        assert_eq!(
            UserPrefs::default().farbakzent,
            Farbakzent::Bernstein,
            "Vorgabe muss der Bernstein-Akzent sein"
        );
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.farbakzent, Farbakzent::Bernstein);

        let mut prefs = UserPrefs::default();
        prefs.farbakzent = Farbakzent::Smaragd;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(
            roh.contains("\"farbakzent\":\"smaragd\""),
            "Schlüssel fehlt: {roh}"
        );
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert_eq!(
            wieder.farbakzent,
            Farbakzent::Smaragd,
            "Akzentwahl ging verloren"
        );

        // T0111, Turn 2: Die Wahlen heißen nach Edelsteinen, die früheren
        // Farbschlüssel bleiben gültig (Alias) — alte `prefs.json` laden ohne
        // Verlust. Die entfallene Wahl „türkis“ wird zu Saphir.
        for (alt, soll) in [
            ("blau", Farbakzent::Saphir),
            ("tuerkis", Farbakzent::Saphir),
            ("gruen", Farbakzent::Smaragd),
            ("rot", Farbakzent::Rubin),
            ("violett", Farbakzent::Amethyst),
        ] {
            let p: UserPrefs =
                serde_json::from_str(&format!(r#"{{"farbakzent": "{alt}"}}"#)).unwrap();
            assert_eq!(p.farbakzent, soll, "alter Schlüssel {alt} ging verloren");
        }

        // Fünf Wahlen in Anzeigereihenfolge, deutsche Namen; Bernstein zuerst.
        assert_eq!(Farbakzent::ALL.len(), 5);
        assert_eq!(Farbakzent::ALL[0], Farbakzent::Bernstein);
        assert_eq!(
            Farbakzent::ALL.map(|a| a.label()),
            ["Bernstein", "Saphir", "Smaragd", "Rubin", "Amethyst"]
        );
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

    /// T0139: Der Linienstärke-Faktor im Modus „Bildschirm (konstant)“ hat die
    /// Vorgabe 1,0, wird gespeichert und beim Laden auf seine Grenzen geklemmt.
    #[test]
    fn linienstaerke_faktor_vorgabe_gespeichert_und_begrenzt() {
        assert!((UserPrefs::default().lineweight_factor - 1.0).abs() < 1e-9);
        // Altbestand ohne Schlüssel: unveränderte Strichdicke.
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert!((alt.lineweight_factor - 1.0).abs() < 1e-9);

        let mut prefs = UserPrefs::default();
        prefs.lineweight_factor = 2.5;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(
            roh.contains("\"lineweight_factor\":2.5"),
            "Schlüssel fehlt: {roh}"
        );
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert!(
            (wieder.lineweight_factor - 2.5).abs() < 1e-9,
            "Einstellung ging beim Laden verloren"
        );

        for (eingabe, erwartet) in [
            (0.0_f32, LINIENSTAERKE_FAKTOR_MIN),
            (-3.0, LINIENSTAERKE_FAKTOR_MIN),
            (99.0, LINIENSTAERKE_FAKTOR_MAX),
            (f32::NAN, 1.0),
        ] {
            let mut p = UserPrefs::default();
            p.lineweight_factor = eingabe;
            p.ensure_defaults();
            assert!(
                (p.lineweight_factor - erwartet).abs() < 1e-9,
                "{eingabe} ergibt {}, erwartet {erwartet}",
                p.lineweight_factor
            );
        }
    }

    /// T0101: Die Bewegungsart der Ansicht (kontinuierlich/sprunghaft) steht in
    /// `prefs.json`; ohne Schlüssel gilt der neue Programmstandard
    /// „kontinuierlich“.
    #[test]
    fn bewegungsart_der_ansicht_wird_gespeichert() {
        assert_eq!(PanMode::default(), PanMode::Kontinuierlich);
        assert_eq!(
            serde_json::to_string(&PanMode::Kontinuierlich).unwrap(),
            "\"kontinuierlich\""
        );
        assert_eq!(
            serde_json::to_string(&PanMode::Sprunghaft).unwrap(),
            "\"sprunghaft\""
        );
        // Altbestand ohne Schlüssel → kontinuierlich.
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.pan_mode, PanMode::Kontinuierlich);

        let mut prefs = UserPrefs::default();
        prefs.pan_mode = PanMode::Sprunghaft;
        let roh = serde_json::to_string(&prefs).unwrap();
        assert!(
            roh.contains("\"pan_mode\":\"sprunghaft\""),
            "Schlüssel fehlt: {roh}"
        );
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert_eq!(wieder.pan_mode, PanMode::Sprunghaft);

        // Zwei Modi, deutsche Namen; „kontinuierlich“ ist die erste Wahl.
        assert_eq!(PanMode::ALL.len(), 2);
        assert_eq!(PanMode::ALL[0], PanMode::Kontinuierlich);
        assert_eq!(PanMode::Kontinuierlich.label(), "Kontinuierlich");
        assert_eq!(PanMode::Sprunghaft.label(), "Sprunghaft");
        assert!(PanMode::Kontinuierlich.kontinuierlich());
        assert!(!PanMode::Sprunghaft.kontinuierlich());
    }

    /// T0070/T0087: Der Drehpunkt der Einfüge-Vorschau steht in `prefs.json`;
    /// ohne Schlüssel gilt der Mittelpunkt (Vorgabe).
    #[test]
    fn drehpunkt_der_einfuege_vorschau_wird_gespeichert() {
        assert_eq!(PastePivot::default(), PastePivot::Center);
        assert_eq!(
            serde_json::to_string(&PastePivot::Center).unwrap(),
            "\"center\""
        );
        assert_eq!(
            serde_json::to_string(&PastePivot::BasePoint).unwrap(),
            "\"base_point\""
        );
        // Datei ohne den Schlüssel → Mittelpunkt (Vorgabe).
        let alt: UserPrefs = serde_json::from_str(r#"{"last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.paste_pivot, PastePivot::Center);

        let dir = test_dir("drehpunkt");
        let p = dir.join("prefs.json");
        let mut prefs = UserPrefs::default();
        prefs.paste_pivot = PastePivot::Center;
        prefs.save_to(&p).unwrap();
        let roh = std::fs::read_to_string(&p).unwrap();
        assert!(
            roh.contains("\"paste_pivot\": \"center\""),
            "prefs.json muss den Drehpunkt enthalten:\n{roh}"
        );
        let wieder = UserPrefs::load_from(&p);
        assert_eq!(
            wieder.paste_pivot,
            PastePivot::Center,
            "Einstellung ging beim Laden verloren"
        );
        std::fs::remove_dir_all(&dir).ok();
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
    fn auswahlfenster_modus_ueberlebt_speichern_und_laden() {
        // T0145: eigene Einstellung für das Auswahlfenster, unabhängig vom
        // Zieh-Modus der Griffe.
        let dir = test_dir("auswahlmodus");
        let p = dir.join("prefs.json");

        let mut prefs = UserPrefs::default();
        assert_eq!(prefs.select_mode, DragMode::HoldAndDrag, "Vorgabe");
        prefs.drag_mode = DragMode::TwoClicks;
        prefs.select_mode = DragMode::TwoClicks;
        prefs.save_to(&p).unwrap();

        let roh = std::fs::read_to_string(&p).unwrap();
        assert!(
            roh.contains("\"select_mode\": \"two_clicks\""),
            "prefs.json muss den Auswahl-Modus lesbar enthalten:\n{roh}"
        );

        let wieder = UserPrefs::load_from(&p);
        assert_eq!(wieder.select_mode, DragMode::TwoClicks);
        assert_eq!(wieder.drag_mode, DragMode::TwoClicks);

        // Beide Schalter sind getrennt: Griffe Gedrückthalten, Auswahl Zwei Klicks.
        let mut getrennt = UserPrefs::default();
        getrennt.select_mode = DragMode::TwoClicks;
        assert_eq!(getrennt.drag_mode, DragMode::HoldAndDrag);

        // Alte Datei ohne den Schlüssel fällt auf die Vorgabe zurück.
        let alt: UserPrefs =
            serde_json::from_str(r#"{"drag_mode": "two_clicks", "last_dir": "/tmp"}"#).unwrap();
        assert_eq!(alt.select_mode, DragMode::HoldAndDrag);
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
        // Funktion, sonst schwenkt und stutzt ein Druck gleichzeitig. Seit
        // T0079 liegt es auf C („c: Stutzen“ aus der Aufgabenstellung).
        assert_eq!(
            prefs.key_for("modify.trim").as_deref(),
            Some("c"),
            "das Stutzen gehört auf C"
        );
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

        // T0159: „Alles zeigen" liegt fest auf F5 — im Register, in der
        // Vorgabebelegung und nicht umbelegbar.
        assert_eq!(
            actions::action("view.zoom_extents").map(|a| a.key),
            Some("f5"),
            "Alles zeigen muss die feste Taste F5 tragen"
        );
        assert_eq!(
            prefs.key_for("view.zoom_extents").as_deref(),
            Some("f5"),
            "F5 muss in der Vorgabebelegung bei Alles zeigen stehen"
        );
        assert_eq!(
            prefs.action_for_key("f5").as_deref(),
            Some("view.zoom_extents")
        );
        assert!(!prefs.belegung_aenderbar("view.zoom_extents"));

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
        assert_eq!(
            alt.key_for("modify.trim").as_deref(),
            Some("c"),
            "S fällt weg, C kommt aus der Vorgabe nach"
        );
        assert_eq!(alt.action_for_key("s").as_deref(), Some("view.pan_down"));
    }

    // ---- Tastaturlayouts (T0076) -------------------------------------------

    fn wegwerf_datei(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jjcad-t0076-{}-{}", std::process::id(), name));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("layout.json")
    }

    /// Das eingebaute Layout steht an erster Stelle und trägt die
    /// Vorgabebelegung; eine Änderung von Hand fällt aus ihm heraus.
    #[test]
    fn werkseinstellung_ist_das_erste_layout() {
        let prefs = UserPrefs::default();
        assert_eq!(prefs.keymap_presets[0].name, WERKSEINSTELLUNG);
        assert_eq!(prefs.keymap_aktiv(), Some(0));

        let mut geaendert = prefs.clone();
        geaendert.bind("draw.wall", "ä");
        assert_eq!(
            geaendert.keymap_aktiv(),
            None,
            "eine geänderte Belegung gilt als eigene Belegung"
        );
    }

    /// Speichern, umschalten, zurück: das Layout trägt die Belegung, unter der
    /// es gespeichert wurde — nicht die spätere.
    #[test]
    fn layout_wird_gespeichert_und_wieder_angewendet() {
        let mut prefs = UserPrefs::default();
        prefs.bind("draw.wall", "ä");
        let name = prefs.keymap_speichern("Meins").expect("Layout speichern");
        assert_eq!(name, "Meins");
        // Hinter den vier eingebauten Vorlagen (T0079).
        assert_eq!(prefs.keymap_aktiv(), Some(4));

        // Weitere Änderung von Hand: kein Layout gilt mehr.
        prefs.bind("draw.wall", "ü");
        assert_eq!(prefs.keymap_aktiv(), None);

        // Zurück auf das gespeicherte Layout.
        assert_eq!(prefs.keymap_anwenden(4).as_deref(), Some("Meins"));
        assert_eq!(prefs.action_for_key("ä").as_deref(), Some("draw.wall"));
        assert!(prefs.action_for_key("ü").is_none(), "die spätere Taste blieb belegt");

        // Und zurück auf die Vorlage JoJa-CAD (früher „Werkseinstellungen").
        assert_eq!(prefs.keymap_anwenden(0).as_deref(), Some(WERKSEINSTELLUNG));
        assert_eq!(prefs.key_for("draw.wall").as_deref(), Some("g"));
        assert!(prefs.action_for_key("ä").is_none());
    }

    #[test]
    fn werkseinstellung_laesst_sich_nicht_ueberschreiben() {
        let mut prefs = UserPrefs::default();
        for name in VORLAGEN {
            assert!(
                prefs.keymap_speichern(name).is_err(),
                "die Vorlage {name} darf nicht überschreibbar sein"
            );
        }
        // Der frühere Name der ersten Vorlage bleibt ebenfalls gesperrt.
        assert!(prefs.keymap_speichern("Werkseinstellung").is_err());
        assert!(prefs.keymap_speichern("  ").is_err());
        assert_eq!(prefs.keymap_presets.len(), VORLAGEN.len());
    }

    /// Export und Import sind zueinander passend: die Datei trägt Name und
    /// Belegung, der Import setzt sie sofort wieder.
    #[test]
    fn layout_wird_exportiert_und_importiert() {
        let path = wegwerf_datei("export");
        let mut prefs = UserPrefs::default();
        prefs.bind("draw.wall", "ä");
        prefs.keymap_speichern("Geteilt").unwrap();
        let idx = prefs.keymap_aktiv().unwrap();
        prefs.keymap_exportieren(idx, &path).expect("Export");

        let mut andere = UserPrefs::default();
        let name = andere.keymap_importieren(&path).expect("Import");
        assert_eq!(name, "Geteilt");
        assert_eq!(andere.action_for_key("ä").as_deref(), Some("draw.wall"));
        assert_eq!(andere.keymap_aktiv().map(|i| andere.keymap_presets[i].name.clone()).as_deref(), Some("Geteilt"));
        // Die Werkseinstellung bleibt dabei erhalten und an erster Stelle.
        assert_eq!(andere.keymap_presets[0].name, WERKSEINSTELLUNG);
        let _ = std::fs::remove_file(&path);
    }

    /// Eine von Hand geschriebene Datei ist nur die nackte Belegung — der Name
    /// kommt dann vom Dateinamen.
    #[test]
    fn import_nimmt_auch_eine_nackte_belegung() {
        let path = wegwerf_datei("nackt");
        std::fs::write(&path, r#"{"draw.wall": ["t"], "draw.line": ["k"]}"#).unwrap();
        let mut prefs = UserPrefs::default();
        let name = prefs.keymap_importieren(&path).expect("Import");
        assert_eq!(name, "layout");
        assert_eq!(prefs.action_for_key("t").as_deref(), Some("draw.wall"));
        assert_eq!(prefs.action_for_key("k").as_deref(), Some("draw.line"));
        let _ = std::fs::remove_file(&path);

        // Unsinn wird nicht stillschweigend geschluckt.
        let kaputt = wegwerf_datei("kaputt");
        std::fs::write(&kaputt, "kein JSON").unwrap();
        assert!(UserPrefs::default().keymap_importieren(&kaputt).is_err());
        let _ = std::fs::remove_file(&kaputt);
    }

    #[test]
    fn layout_laesst_sich_loeschen_aber_nicht_das_benutzte() {
        let mut prefs = UserPrefs::default();
        prefs.bind("draw.wall", "ä");
        prefs.keymap_speichern("Eins").unwrap();
        prefs.bind("draw.wall", "ü");
        prefs.keymap_speichern("Zwei").unwrap();
        assert_eq!(prefs.keymap_presets.len(), 6);
        // „Zwei" ist das benutzte Layout; es lässt sich löschen, danach gilt
        // „Eigene Belegung" — die Tasten bleiben unverändert gültig.
        assert_eq!(prefs.keymap_aktiv(), Some(5));
        for i in 0..VORLAGEN.len() {
            assert!(
                !prefs.keymap_loeschen(i),
                "die eingebaute Vorlage {} bleibt stehen",
                VORLAGEN[i]
            );
        }
        assert!(prefs.keymap_loeschen(5), "das benutzte Layout muss weggehen");
        assert_eq!(prefs.keymap_aktiv(), None);
        assert_eq!(prefs.action_for_key("ü").as_deref(), Some("draw.wall"));
        assert_eq!(prefs.keymap_presets.len(), 5);
    }

    /// T0076: „Speichern" nimmt das neue Layout auch dann in die Auswahl auf,
    /// wenn seine Belegung mit einem vorhandenen Eintrag übereinstimmt — der
    /// Name, den der Nutzer gespeichert hat, muss dort stehen.
    #[test]
    fn gespeichertes_layout_steht_auch_bei_gleicher_belegung_in_der_auswahl() {
        let mut prefs = UserPrefs::default();
        // Unveränderte Belegung = Werkseinstellung, gespeichert als „Mein".
        prefs.keymap_speichern("Mein").unwrap();
        assert_eq!(prefs.keymap_aktiv(), Some(4));
        assert_eq!(
            prefs
                .keymap_aktiv()
                .map(|i| prefs.keymap_presets[i].name.clone())
                .as_deref(),
            Some("Mein"),
            "die Auswahl zeigt die erste Vorlage statt des gespeicherten Namens"
        );

        // Ein zweiter Name mit derselben Belegung wird ebenso übernommen.
        prefs.keymap_speichern("Mein 2").unwrap();
        assert_eq!(prefs.keymap_aktiv(), Some(5));

        // Und der Weg durch die Datei behält ihn.
        let roh = serde_json::to_string(&prefs).unwrap();
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert_eq!(
            wieder
                .keymap_aktiv()
                .map(|i| wieder.keymap_presets[i].name.clone())
                .as_deref(),
            Some("Mein 2")
        );
    }

    /// T0076: Ein von Hand geänderter Belegungseintrag fällt auf „Eigene
    /// Belegung" zurück; wird er wieder gespeichert, steht er wieder in der Auswahl.
    #[test]
    fn handaenderung_gilt_als_eigene_belegung() {
        let mut prefs = UserPrefs::default();
        prefs.keymap_speichern("Mein").unwrap();
        prefs.bind("draw.line", "ä");
        assert_eq!(prefs.keymap_aktiv(), None);
        prefs.keymap_speichern("Mein").unwrap();
        assert_eq!(prefs.keymap_aktiv(), Some(4));
    }

    /// T0076: Nur eine Belegung, die in keinem Layout steht, gilt als
    /// ungespeichert — sie löst die Rückfrage vor dem Wechsel aus.
    #[test]
    fn ungespeicherte_belegung_wird_erkannt() {
        let mut prefs = UserPrefs::default();
        assert!(!prefs.keymap_ungespeichert(), "frisch = Vorlage JoJa-CAD");
        prefs.bind("draw.wall", "ä");
        assert!(prefs.keymap_ungespeichert());
        prefs.keymap_speichern("Meins").unwrap();
        assert!(!prefs.keymap_ungespeichert());
        // Ein Layout löschen, dessen Tasten kein anderes trägt: eigene Belegung.
        prefs.bind("draw.wall", "ü");
        prefs.keymap_speichern("Zwei").unwrap();
        assert!(!prefs.keymap_ungespeichert());
        assert!(prefs.keymap_loeschen(5));
        assert!(
            prefs.keymap_ungespeichert(),
            "nach dem Löschen ist die Belegung in keinem Layout"
        );
    }

    /// Die Layouts überleben den Weg durch die `prefs.json`.
    #[test]
    fn layouts_werden_mit_gespeichert() {
        let mut prefs = UserPrefs::default();
        prefs.bind("draw.wall", "ä");
        prefs.keymap_speichern("Mit").unwrap();
        let roh = serde_json::to_string(&prefs).unwrap();
        let wieder: UserPrefs = serde_json::from_str(&roh).unwrap();
        assert_eq!(wieder.keymap_presets.len(), VORLAGEN.len() + 1);
        assert!(wieder.keymap_presets.iter().any(|p| p.name == "Mit"));
        assert_eq!(wieder.keymap_presets[0].name, WERKSEINSTELLUNG);
    }

    /// T0079: Die vier Vorlagen stehen in fester Reihenfolge vorn — JoJa-CAD,
    /// Allplan, ArchiCAD, AutoCAD.
    #[test]
    fn vier_vorlagen_stehen_vorn() {
        let prefs = UserPrefs::default();
        let namen: Vec<&str> = prefs
            .keymap_presets
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(namen, VORLAGEN.to_vec());
        // Jede Vorlage ist eine eigene Belegung — sonst wäre sie überflüssig.
        for i in 0..VORLAGEN.len() {
            for j in (i + 1)..VORLAGEN.len() {
                assert_ne!(
                    prefs.keymap_presets[i].bindings, prefs.keymap_presets[j].bindings,
                    "Vorlage {} und {} sind identisch",
                    VORLAGEN[i], VORLAGEN[j]
                );
            }
        }
    }

    /// T0079: Die Werkseinstellung belegt `c` mit Stutzen, `t` mit Text und `z`
    /// mit Zerlegen (Aufgabenstellung). Der Farbeimer lag auf `C` und weicht
    /// deshalb auf Umschalt+C aus.
    #[test]
    fn vorlage_jjcad_belegt_c_t_z() {
        let prefs = UserPrefs::default();
        for (taste, aktion) in [
            ("c", "modify.trim"),
            ("t", "draw.text"),
            ("z", "modify.explode"),
            ("shift+c", "draw.fill.paint"),
        ] {
            assert_eq!(
                prefs.action_for_key(taste).as_deref(),
                Some(aktion),
                "Taste {taste} muss {aktion} tragen"
            );
            assert_eq!(
                prefs.key_for(aktion).as_deref(),
                Some(taste),
                "{aktion} muss auf {taste} liegen"
            );
        }
    }

    /// T0079: Jede Vorlage gehorcht der Regel „eine Taste, eine Funktion" —
    /// der Zusammenbau nimmt der vorherigen Funktion die Taste weg. Geprüft
    /// wird der **ganze** Schlüssel, auch `shift+…` und Kombinationen.
    #[test]
    fn jede_vorlage_belegt_jede_taste_genau_einmal() {
        for p in vorlagen() {
            let mut gesehen: BTreeMap<String, String> = BTreeMap::new();
            for (aktion, keys) in &p.bindings {
                for key in keys {
                    let k = key.to_lowercase();
                    if let Some(alt) = gesehen.insert(k.clone(), aktion.clone()) {
                        panic!("Vorlage {}: Taste {k} doppelt ({alt}, {aktion})", p.name);
                    }
                }
            }
            assert!(
                gesehen.len() >= 15,
                "Vorlage {} belegt nur {} Tasten",
                p.name,
                gesehen.len()
            );
        }
    }

    /// T0079: Die Vorlagen belegen nur Tasten, die auch ankommen — Einzelzeichen,
    /// Umschalt+Zeichen (Schicht 2) oder die feste Belegung (Schicht 3).
    /// `;` oder `=` (ArchiCAD, AutoCAD) sind auf einer deutschen Tastatur
    /// Umschaltzeichen einer anderen Taste und werden nie ausgewertet.
    #[test]
    fn vorlagen_belegen_nur_erreichbare_tasten() {
        for p in vorlagen() {
            for (aktion, keys) in &p.bindings {
                for key in keys {
                    let k = key.to_lowercase();
                    let erreichbar = match k.strip_prefix("shift+") {
                        Some(rest) => rest.chars().count() == 1,
                        None => k.chars().count() == 1 || fest_belegt(&k).is_some(),
                    };
                    assert!(
                        erreichbar,
                        "Vorlage {}: {aktion} trägt die nicht erreichbare Taste {key}",
                        p.name
                    );
                }
            }
        }
    }

    /// T0079: Stichproben der fremden Vorgaben. Quellen stehen in den
    /// Kommentaren über den Abweichungstabellen (`VORLAGE_ALLPLAN` usw.).
    #[test]
    fn vorlagen_folgen_den_vorgaben_der_fremden_programme() {
        let v = vorlagen();
        let traeger = |idx: usize, key: &str| -> Option<String> {
            v[idx]
                .bindings
                .iter()
                .find(|(_, keys)| keys.iter().any(|k| k.eq_ignore_ascii_case(key)))
                .map(|(aktion, _)| aktion.clone())
        };
        // Allplan: Strg+W Wand, Strg+T Tür, Strg+F Fenster, Strg+L Linie,
        // Strg+M Maßlinie.
        for (key, aktion) in [
            ("w", "draw.wall"),
            ("t", "draw.door"),
            ("f", "draw.window"),
            ("l", "draw.line"),
            ("m", "dim.linear"),
            ("shift+t", "draw.text"),
        ] {
            assert_eq!(traeger(1, key).as_deref(), Some(aktion), "Allplan · {key}");
        }
        // ArchiCAD: W Wand, , Fenster, M Tür, L Polylinie, Umschalt+L Linie,
        // K Text, J Maß, O Versetzen, D Verschieben, Umschalt+C Kopieren.
        for (key, aktion) in [
            ("w", "draw.wall"),
            (",", "draw.window"),
            ("m", "draw.door"),
            ("l", "draw.polyline"),
            ("shift+l", "draw.line"),
            ("k", "draw.text"),
            ("j", "dim.linear"),
            ("o", "modify.offset"),
            ("d", "modify.move"),
            ("shift+c", "modify.copy"),
        ] {
            assert_eq!(traeger(2, key).as_deref(), Some(aktion), "ArchiCAD · {key}");
        }
        // AutoCAD: L Linie, P Polylinie, A Bogen, C Kreis, T Text, M Move,
        // O Offset, S Scale, X Explode, H Hatch, I Insert, D Bemaßung, Z Zoom.
        for (key, aktion) in [
            ("l", "draw.line"),
            ("p", "draw.polyline"),
            ("a", "draw.arc"),
            ("c", "draw.circle"),
            ("t", "draw.text"),
            ("m", "modify.move"),
            ("r", "modify.rotate"),
            ("o", "modify.offset"),
            ("s", "modify.scale"),
            ("x", "modify.explode"),
            ("h", "draw.fill.paint"),
            ("i", "block.insert"),
            ("d", "dim.linear"),
            ("z", "view.zoom_extents"),
        ] {
            assert_eq!(traeger(3, key).as_deref(), Some(aktion), "AutoCAD · {key}");
        }
    }

    /// T0079: Eine alte `prefs.json` (Stutzen ohne Taste, Farbeimer auf C, Text
    /// ohne T, Zerlegen ohne Taste) bekommt beim Laden die neuen Werkstasten.
    #[test]
    fn alte_belegung_uebernimmt_die_neuen_werkstasten() {
        let mut alt = UserPrefs::default();
        alt.bindings.remove("modify.trim");
        alt.bindings.remove("modify.explode");
        alt.bindings.remove("draw.text");
        alt.bindings.insert("draw.fill.paint".into(), vec!["c".into()]);
        alt.ensure_defaults();
        assert_eq!(
            alt.action_for_key("c").as_deref(),
            Some("modify.trim"),
            "C muss das Stutzen tragen"
        );
        assert_eq!(alt.key_for("draw.fill.paint").as_deref(), Some("shift+c"));
        assert_eq!(alt.action_for_key("t").as_deref(), Some("draw.text"));
        assert_eq!(alt.action_for_key("z").as_deref(), Some("modify.explode"));
    }

    /// T0079: Eigene Layouts bleiben **hinter** den Vorlagen, und die Auswahl
    /// wandert mit — sonst zeigte sie nach dem Laden auf eine fremde Vorlage.
    #[test]
    fn eigene_layouts_bleiben_hinter_den_vorlagen() {
        let mut prefs = UserPrefs::default();
        prefs.bind("draw.wall", "ä");
        prefs.keymap_speichern("Büro").unwrap();
        // Datei aus der Zeit vor T0079: „Werkseinstellung" und „Büro".
        let mut roh = serde_json::to_value(&prefs).unwrap();
        roh["keymap_presets"] = serde_json::json!([
            {"name": "Werkseinstellung", "bindings": prefs.bindings},
            {"name": "Büro", "bindings": prefs.bindings},
        ]);
        roh["keymap_aktiv"] = serde_json::json!(1);
        let mut wieder: UserPrefs = serde_json::from_value(roh).unwrap();
        wieder.ensure_defaults();
        assert_eq!(wieder.keymap_presets.len(), VORLAGEN.len() + 1);
        assert_eq!(wieder.keymap_presets[0].name, WERKSEINSTELLUNG);
        assert_eq!(wieder.keymap_presets[4].name, "Büro");
        assert_eq!(
            wieder.keymap_aktiv(),
            Some(4),
            "die Auswahl zeigt auf eine Vorlage statt auf das eigene Layout"
        );
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

    /// Eine Altdatei ohne `id` im Kopf bekommt beim Laden eine Kennung aus dem
    /// Pfad — zweimal geöffnet muss dieselbe herauskommen (T0129).
    #[test]
    fn altdatei_ohne_kennung_bekommt_eine_stabile() {
        let d = test_dir("alt-ohne-id");
        let p = d.join("alt.jjcad");
        let doc = crate::model::Document::new();
        let mut v: serde_json::Value = serde_json::from_str(&to_json(&doc).unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("id");
        std::fs::write(&p, serde_json::to_string(&v).unwrap()).unwrap();

        let a = load_document(&p).expect("Altdatei muss laden");
        let b = load_document(&p).expect("Altdatei muss laden");
        assert!(!a.id.trim().is_empty(), "Kennung fehlt");
        assert_eq!(a.id, b.id, "zweimal geöffnet muss dieselbe Kennung ergeben");
    }

    /// Steht die Kennung in der Datei, gewinnt sie gegen den Pfad — sonst
    /// verlöre eine umbenannte Zeichnung ihren Verlauf.
    #[test]
    fn kennung_aus_der_datei_gewinnt_gegen_den_pfad() {
        let d = test_dir("id-aus-datei");
        let p = d.join("mit-id.jjcad");
        let mut doc = crate::model::Document::new();
        doc.id = "11111111-2222-4333-8444-555555555555".into();
        std::fs::write(&p, to_json(&doc).unwrap()).unwrap();

        assert_eq!(
            load_document(&p).expect("laden").id,
            "11111111-2222-4333-8444-555555555555"
        );
    }
}
