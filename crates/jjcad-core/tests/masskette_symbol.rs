//! T0227: Punktsymbol an den Messpunkten einer Maßkette.
//!
//! Die Marke steht an **jedem** Messpunkt, wird im Eigenschaften-Panel aus der
//! Blockbibliothek gewählt und ist der **Schrägstrich**, solange niemand etwas
//! anderes wählt. Geprüft wird über die Action-Schicht und die Anzeige — der
//! Weg, den Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::model::{
    BlockGroesse, Document, EntityKind, PUNKTSYMBOL_KATEGORIE, DIM_SYMBOL_VORGABE,
};
use jjcad_core::persist;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut d = Document::new();
    d.settings.input_unit = Unit::Mm;
    d
}

fn kette(d: &mut Document) -> u64 {
    let r = actions::execute(
        d,
        "dim.chain",
        &json!({
            "pts": [[0.0, 0.0], [3000.0, 0.0], [5000.0, 0.0]],
            "line_pos": [0.0, -400.0],
            "angle": 0.0
        }),
    )
    .expect("Maßkette anlegen");
    r["created"][0].as_u64().expect("Kennung")
}

fn symbol(d: &Document, id: u64) -> String {
    match &d.entity(id).expect("Maßkette").kind {
        EntityKind::Dim { symbol, .. } => symbol.clone(),
        k => panic!("keine Maßkette: {k:?}"),
    }
}

/// Zählt die gezeichneten Linien des Dokuments.
fn linien(d: &Document) -> usize {
    render_document(&RenderCtx::new(d))
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Line(..)))
        .count()
}

/// Der Schrägstrich steht in der Blockbibliothek: Fadenkreuz plus 45°-Strich,
/// in Papiergröße wie alle Punktsymbole.
#[test]
fn der_schraegstrich_steht_in_den_punktsymbolen() {
    let d = Document::new();
    let block = d
        .block(DIM_SYMBOL_VORGABE)
        .unwrap_or_else(|| panic!("Block {DIM_SYMBOL_VORGABE} fehlt"));
    assert_eq!(block.name, "Schrägstrich");
    assert_eq!(block.category, PUNKTSYMBOL_KATEGORIE);
    assert_eq!(block.groesse, BlockGroesse::Papier, "Papiergröße");
    // Senkrechte, Waagerechte und der Strich unter 45°.
    assert_eq!(block.entities.len(), 3, "Fadenkreuz plus Schrägstrich");
    for e in &block.entities {
        assert!(
            matches!(e.kind, EntityKind::Line { .. }),
            "ein Symbol aus Linien erwartet: {:?}",
            e.kind
        );
    }
    assert!(
        d.punktsymbol_bloecke()
            .iter()
            .any(|b| b.id == DIM_SYMBOL_VORGABE),
        "der Schrägstrich fehlt in der Auswahlliste der Punktsymbole"
    );
}

/// Ohne eigene Wahl trägt jeder Messpunkt den Schrägstrich — für die Kette wie
/// für das Einzelmaß.
#[test]
fn vorgabe_ist_der_schraegstrich() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    assert_eq!(symbol(&d, id), DIM_SYMBOL_VORGABE, "Vorgabe der Kette");

    let r = actions::execute(
        &mut d,
        "dim.linear",
        &json!({"a": [0.0, 2000.0], "b": [3000.0, 2000.0], "line_pos": [0.0, 1500.0]}),
    )
    .expect("Einzelmaß");
    let id2 = r["created"][0].as_u64().expect("Kennung");
    assert_eq!(symbol(&d, id2), DIM_SYMBOL_VORGABE, "Vorgabe des Einzelmaßes");
}

/// Die Marke steht **an jedem** Messpunkt: drei Punkte → drei Symbole zu je
/// drei Linien. „Kein Symbol“ nimmt genau diese neun Linien wieder weg.
#[test]
fn symbol_steht_an_jedem_messpunkt() {
    let mut d = doc_mm();
    let vorher = linien(&d);
    let id = kette(&mut d);
    let mit = linien(&d) - vorher;

    actions::execute(&mut d, "modify.set", &json!({"ids": [id], "symbol": ""}))
        .expect("Symbol wegnehmen");
    let ohne = linien(&d) - vorher;

    assert_eq!(
        mit - ohne,
        9,
        "drei Messpunkte mal drei Linien (Vorgabe {mit}, ohne Symbol {ohne})"
    );
    assert_eq!(symbol(&d, id), "", "leeres Feld heißt kein Symbol");
}

/// Das Panel schickt `symbol` über `modify.set`; ein unbekannter Block ist ein
/// Fehler (kein stiller Erfolg).
#[test]
fn modify_set_setzt_und_nimmt_das_symbol() {
    let mut d = doc_mm();
    let id = kette(&mut d);

    actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [id], "symbol": "sys:pkt-raute"}),
    )
    .expect("Symbol setzen");
    assert_eq!(symbol(&d, id), "sys:pkt-raute");

    let fehler = actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [id], "symbol": "gibts:nicht"}),
    )
    .expect_err("unbekannter Block muss abgelehnt werden");
    assert!(fehler.contains("gibts:nicht"), "Meldung nennt den Block: {fehler}");
    assert_eq!(symbol(&d, id), "sys:pkt-raute", "der Stand bleibt unberührt");
}

/// Das Symbol steht in der Datei und kommt beim Laden zurück; eine Datei
/// **ohne** das Feld (ältere Zeichnung) bekommt die Vorgabe.
#[test]
fn symbol_ueberlebt_das_speichern() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [id], "symbol": "sys:pkt-kreuz"}),
    )
    .expect("Symbol setzen");

    let text = persist::to_json(&d).expect("speichern");
    assert!(
        text.contains("\"symbol\": \"sys:pkt-kreuz\""),
        "Feld fehlt in der Datei"
    );
    let wieder = persist::from_json(&text).expect("laden");
    assert_eq!(symbol(&wieder, id), "sys:pkt-kreuz", "nach dem Laden");

    // Alte Zeichnung: Feld entfernen — die Vorgabe greift.
    let mut roh: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let mut entfernt = false;
    for e in roh["entities"].as_array_mut().expect("entities") {
        if e["id"].as_u64() == Some(id) {
            entfernt = e["kind"]
                .as_object_mut()
                .expect("kind")
                .remove("symbol")
                .is_some();
        }
    }
    assert!(entfernt, "das Feld muss entfernt worden sein");
    let alt = serde_json::to_string(&roh).expect("JSON schreiben");
    let alt = persist::from_json(&alt).expect("alte Datei laden");
    assert_eq!(
        symbol(&alt, id),
        DIM_SYMBOL_VORGABE,
        "eine ältere Zeichnung bekommt den Schrägstrich"
    );
}

/// Die Marke ist eine Papiergröße: bei 1:100 wächst sie auf 2,2 m Kantenlänge,
/// wie jedes Punktsymbol (T0231).
#[test]
fn symbol_waechst_mit_dem_massstab() {
    let mut d = doc_mm();
    d.settings.annotation_scale = 100.0;
    let id = kette(&mut d);
    let dl = render_document(&RenderCtx::new(&d));
    let bbox = dl.bbox;
    // Der erste Messpunkt liegt auf (0,0): die Marke reicht 1,1 Papier-mm
    // (also 110 Welt-mm) darüber hinaus, ebenso am letzten Punkt bei 5000 mm.
    assert!(
        bbox.min.x <= -110.0 && bbox.min.y <= -110.0,
        "die Marke wächst nicht mit dem Maßstab: {bbox:?} (Kette {id})"
    );
    assert!(
        bbox.max.x >= 5110.0,
        "die Marke am letzten Messpunkt fehlt: {bbox:?}"
    );
}
