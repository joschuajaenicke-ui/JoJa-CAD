//! T0229: Pfad — Kette aus Linien, Bögen, Ellipsenbögen und Splines.
//!
//! Geprüft wird die Befehls-Schicht (`draw.path`, `path.connect`) — derselbe
//! Weg, den Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::v;
use jjcad_core::model::{Document, Entity, EntityKind};
use jjcad_core::pfad::{pfad_flaeche, PathSeg};
use serde_json::json;

fn doc_mm() -> Document {
    let mut doc = Document::new();
    doc.settings.input_unit = jjcad_core::units::Unit::Mm;
    doc
}

fn pfad(doc: &Document, id: u64) -> (Vec<PathSeg>, bool) {
    match &doc.entity(id).expect("Objekt").kind {
        EntityKind::Path { segments, closed } => (segments.clone(), *closed),
        other => panic!("kein Pfad: {other:?}"),
    }
}

/// Drei Punkte, in Linien: zwei aneinanderhängende Strecken.
#[test]
fn linienkette_entsteht_aus_drei_klicks() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.path",
        &json!({"pts": [[0, 0], [1000, 0], [1000, 800]],
                "schritte": ["start", "line", "line"]}),
    )
    .expect("Pfad");
    let id = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, id);
    assert_eq!(segs.len(), 2, "zwei Stücke: {segs:?}");
    assert!(!closed);
    assert!((segs[0].start().x - 0.0).abs() < 1e-9);
    assert!((segs[0].end().x - 1000.0).abs() < 1e-9);
    // Das zweite Stück beginnt am Ende des ersten — ohne eigenes `a`.
    assert!(segs[1].start().dist(v(1000.0, 0.0)) < 1e-9);
}

/// Mit `arc_point`/`arc_end` entsteht ein Bogen durch drei Punkte, danach geht
/// die Kette am Bogenende weiter.
#[test]
fn bogenstueck_haengt_am_vorigen_ende() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.path",
        &json!({"pts": [[0, 0], [500, 500], [1000, 0], [1000, -500]],
                "schritte": ["start", "arc_point", "arc_end", "line"]}),
    )
    .expect("Pfad mit Bogen");
    let id = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, _) = pfad(&doc, id);
    assert_eq!(segs.len(), 2);
    match &segs[0] {
        PathSeg::Arc { r, .. } => assert!((r - 500.0).abs() < 1e-6, "Radius 500: {r}"),
        other => panic!("kein Bogen: {other:?}"),
    }
    assert!(segs[1].start().dist(v(1000.0, 0.0)) < 1e-6);
}

/// Klick auf den Startpunkt schließt den Pfad; er umschließt dann eine Fläche.
#[test]
fn geschlossener_pfad_hat_eine_flaeche() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.path",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 1000], [0, 0]],
                "schritte": ["start", "line", "line", "line", "line"]}),
    )
    .expect("Pfad");
    let id = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, id);
    assert_eq!(segs.len(), 4);
    assert!(closed, "Anfang und Ende liegen aufeinander");
    let flaeche = pfad_flaeche(&segs).expect("Fläche");
    assert!((flaeche - 2_000_000.0).abs() < 1.0, "2 m²: {flaeche}");
}

/// Vier Linien, kreuz und quer ausgewählt und teils falsch herum: verbinden
/// ordnet sie zur Kette und schließt das Rechteck.
#[test]
fn verbinden_ordnet_und_schliesst_das_rechteck() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let ecken = [
        v(0.0, 0.0),
        v(2000.0, 0.0),
        v(2000.0, 1000.0),
        v(0.0, 1000.0),
    ];
    let mut ids = Vec::new();
    for i in 0..4 {
        let a = ecken[i];
        let b = ecken[(i + 1) % 4];
        // Jede zweite Linie umgedreht einfügen.
        let (a, b) = if i % 2 == 0 { (a, b) } else { (b, a) };
        let id = doc.new_id();
        doc.add_entity(Entity::new(id, layer, EntityKind::Line { a, b }));
        ids.push(id);
    }
    // Auswahl in falscher Reihenfolge an die Action geben.
    let reihenfolge = [ids[2], ids[0], ids[3], ids[1]];
    let antwort = actions::execute(
        &mut doc,
        "path.connect",
        &json!({ "ids": reihenfolge }),
    )
    .expect("verbinden");
    let neu = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, neu);
    assert_eq!(segs.len(), 4);
    assert!(closed, "das Rechteck ist geschlossen");
    for i in 0..4 {
        let ende = segs[i].end();
        let anfang = segs[(i + 1) % 4].start();
        assert!(ende.dist(anfang) < 1e-6, "Stück {i} schließt nicht an");
    }
    // Die vier Linien sind verbraucht.
    assert_eq!(doc.ents().len(), 1, "nur der Pfad bleibt");
    assert!((pfad_flaeche(&segs).unwrap() - 2_000_000.0).abs() < 1.0);
}

/// Was nicht zusammenhängt, wird abgelehnt — und die Ausgangsobjekte bleiben.
#[test]
fn verbinden_lehnt_getrennte_linien_ab() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let a = doc.new_id();
    doc.add_entity(Entity::new(a, layer, EntityKind::Line {
        a: v(0.0, 0.0),
        b: v(1000.0, 0.0),
    }));
    let b = doc.new_id();
    doc.add_entity(Entity::new(b, layer, EntityKind::Line {
        a: v(5000.0, 0.0),
        b: v(6000.0, 0.0),
    }));
    let err = actions::execute(&mut doc, "path.connect", &json!({"ids": [a, b]}))
        .expect_err("getrennte Linien");
    assert!(err.to_lowercase().contains("hängen"), "Meldung: {err}");
    assert_eq!(doc.ents().len(), 2, "nichts wurde entfernt");
}

/// Ein Pfad mit Spline ergibt nie eine geschlossene Kontur (T0229).
#[test]
fn spline_pfad_ist_nie_geschlossen() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let linie = doc.new_id();
    doc.add_entity(Entity::new(linie, layer, EntityKind::Line {
        a: v(0.0, 0.0),
        b: v(1000.0, 0.0),
    }));
    let spline = doc.new_id();
    doc.add_entity(Entity::new(
        spline,
        layer,
        EntityKind::Spline {
            verts: vec![
                jjcad_core::Vertex::new(v(1000.0, 0.0)),
                jjcad_core::Vertex::new(v(1500.0, 500.0)),
                jjcad_core::Vertex::new(v(0.0, 0.0)),
            ],
            closed: false,
        },
    ));
    let antwort = actions::execute(&mut doc, "path.connect", &json!({"ids": [linie, spline]}))
        .expect("verbinden");
    let neu = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, neu);
    assert_eq!(segs.len(), 2);
    assert!(!closed, "Spline schließt nie");
    assert!(pfad_flaeche(&segs).is_none(), "keine Fläche mit Spline");
}

/// Ein einzelnes Objekt ist kein Pfad.
#[test]
fn verbinden_braucht_zwei_objekte() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let a = doc.new_id();
    doc.add_entity(Entity::new(a, layer, EntityKind::Line {
        a: v(0.0, 0.0),
        b: v(1000.0, 0.0),
    }));
    assert!(actions::execute(&mut doc, "path.connect", &json!({"ids": [a]})).is_err());
}
