//! T0227: Punktsymbol auf der Maßlinie einer Maßkette.
//!
//! Die Marke steht an **jedem Basispunkt der Maßlinie** (und ersetzt dort den
//! Begrenzungsstrich), wird im Eigenschaften-Panel aus der Blockbibliothek
//! gewählt und ist der **Schrägstrich**, solange niemand etwas anderes wählt.
//! Geprüft wird über die Action-Schicht und die Anzeige — der Weg, den
//! Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::Vec2;
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
    // **Gleiche radiale Länge** für alle drei Striche (Turn 5): Fadenkreuz und
    // Schrägstrich enden auf demselben Kreis um den Basispunkt — der schräge
    // Strich liegt bei ±ARM/√2, nicht bei ±ARM.
    let mut arme: Vec<f64> = Vec::new();
    for e in &block.entities {
        match &e.kind {
            EntityKind::Line { a, b } => {
                assert!(
                    (a.dist(Vec2::ZERO) - b.dist(Vec2::ZERO)).abs() < 1e-9,
                    "der Strich liegt nicht mittig um den Basispunkt: {a:?}–{b:?}"
                );
                arme.push(a.dist(Vec2::ZERO));
            }
            k => panic!("ein Symbol aus Linien erwartet: {k:?}"),
        }
    }
    arme.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        (arme[arme.len() - 1] - arme[0]).abs() < 1e-9,
        "die Striche sind verschieden lang (radial): {arme:?}"
    );
    assert!(
        (arme[0] - 1.1).abs() < 1e-9,
        "radiale Länge muss 1,1 mm sein: {arme:?}"
    );
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

/// Die Marke steht **auf der Maßlinie** und **ersetzt** den Begrenzungsstrich:
/// drei Basispunkte → drei Symbole zu je drei Linien statt drei Schrägstrichen.
/// „Kein Symbol“ gibt dem Stil seine Marke zurück.
#[test]
fn symbol_liegt_auf_der_masslinie_und_ersetzt_den_strich() {
    let mut d = doc_mm();
    let vorher = linien(&d);
    let id = kette(&mut d);
    let mit = linien(&d) - vorher;

    actions::execute(&mut d, "modify.set", &json!({"ids": [id], "symbol": ""}))
        .expect("Symbol wegnehmen");
    let ohne = linien(&d) - vorher;

    // Ersatz, nicht Zusatz: neun Symbol-Linien, sonst steht dort **gar nichts**
    // (der alte Begrenzungsstrich kommt bei „Kein Symbol“ nicht zurück).
    assert_eq!(
        mit - ohne,
        9,
        "drei Symbole zu je drei Linien, ohne Symbol nichts (mit {mit}, ohne {ohne})"
    );
    assert_eq!(symbol(&d, id), "", "leeres Feld heißt kein Symbol");

    // Und die Marke liegt auf der Maßlinie (y = −400), nicht an den
    // Messpunkten (y = 0): keine Linie reicht dorthin.
    let hoehen = linien_hoehen(&d, id);
    assert!(
        hoehen.iter().all(|(y0, y1)| *y0 < -280.0 && *y1 < -280.0),
        "eine Linie steht an den Messpunkten statt auf der Maßlinie: {hoehen:?}"
    );
}

/// Die Höhen (y) aller Linien einer Maßkette.
fn linien_hoehen(d: &Document, id: u64) -> Vec<(f64, f64)> {
    render_document(&RenderCtx::new(d))
        .prims
        .iter()
        .filter(|p| p.entity == id)
        .filter_map(|p| match &p.shape {
            Shape::Line(a, b) => Some((a.y, b.y)),
            _ => None,
        })
        .collect()
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
/// wie jedes Punktsymbol (T0231) — und sie liegt auf der Maßlinie (y = −400),
/// nicht an den Messpunkten (y = 0).
#[test]
fn symbol_waechst_mit_dem_massstab() {
    let mut d = doc_mm();
    d.plans[0].scale = 100.0;
    let id = kette(&mut d);
    let dl = render_document(&RenderCtx::new(&d));
    let bbox = dl.bbox;
    // Erster und letzter Basispunkt der Maßlinie liegen bei x = 0 bzw. 5000:
    // die Marke reicht 1,1 Papier-mm (also 110 Welt-mm) darüber hinaus — die
    // Maßlinie selbst endet genau dort (kein Überstand mehr, Turn 7).
    assert!(
        bbox.min.x <= -110.0 && bbox.min.x > -130.0,
        "die Marke wächst nicht mit dem Maßstab oder die Maßlinie steht über: {bbox:?} (Kette {id})"
    );
    assert!(
        bbox.max.x >= 5110.0,
        "die Marke am letzten Basispunkt fehlt: {bbox:?}"
    );
    // Die Maßlinie liegt bei y = −400 (die Maßzahl steht darüber), die Marke
    // reicht ±110 darum — nicht bis zu den Messpunkten bei y = 0.
    assert!(
        bbox.min.y <= -510.0,
        "die Marke liegt nicht auf der Maßlinie (y = −400): {bbox:?}"
    );
}

/// „Kein Symbol“ heißt wirklich **kein** Begrenzungszeichen (T0227, Turn 7):
/// die Maßkette zeigt dann nur Maßlinie, Maßzahl und (falls an) Hilfslinien.
#[test]
fn ohne_symbol_steht_kein_begrenzungszeichen() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(&mut d, "modify.set", &json!({"ids": [id], "symbol": ""}))
        .expect("Symbol wegnehmen");
    let laengen: Vec<f64> = render_document(&RenderCtx::new(&d))
        .prims
        .iter()
        .filter(|p| p.entity == id)
        .filter_map(|p| match &p.shape {
            Shape::Line(a, b) => Some(a.dist(*b)),
            _ => None,
        })
        .collect();
    // Maßlinie 5000 mm (kein Überstand mehr); ein Symbol wäre 220–311 mm kurz.
    assert!(
        laengen.iter().all(|l| *l > 500.0),
        "ohne Symbol darf nichts an den Enden stehen: {laengen:?}"
    );
    assert!(
        laengen.iter().any(|l| (*l - 5000.0).abs() < 3.0),
        "die Maßlinie fehlt: {laengen:?}"
    );
}
