//! T0200: Parallele zu Linie und Polylinie.
//!
//! Die Funktion zeichnet eine zweite Linie im Abstand daneben; die Seite
//! bestimmt der Mauszeiger, nicht der Betrag. Geprüft wird die Action
//! (`draw.parallel`) — derselbe Weg, den Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{Document, Entity, EntityKind};
use serde_json::json;

/// Zeichnung mit einer waagerechten Linie (0,0)→(2000,0) in mm.
fn doc_mit_linie() -> (Document, u64) {
    let mut doc = Document::new();
    doc.settings.input_unit = jjcad_core::units::Unit::Mm;
    let id = doc.new_id();
    doc.add_entity(Entity::new(id, doc.current_layer, EntityKind::Line {
        a: v(0.0, 0.0),
        b: v(2000.0, 0.0),
    }));
    (doc, id)
}

fn enden(doc: &Document, id: u64) -> (Vec2, Vec2) {
    match &doc.entity(id).expect("Objekt").kind {
        EntityKind::Line { a, b } => (*a, *b),
        other => panic!("keine Linie: {other:?}"),
    }
}

fn letzte_linie(doc: &Document) -> (Vec2, Vec2) {
    let e = doc
        .ents()
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .next_back()
        .expect("Linie");
    match &e.kind {
        EntityKind::Line { a, b } => (*a, *b),
        _ => unreachable!(),
    }
}

/// Der Abstand ist der senkrechte Abstand, die Seite kommt vom Punkt `side`.
#[test]
fn parallele_liegt_im_abstand_auf_der_zeigerseite() {
    let (mut doc, id) = doc_mit_linie();

    let oben = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 500.0, "side": [1000.0, 900.0]}),
    )
    .expect("Parallele oben");
    let neu = oben["created"][0].as_u64().expect("neue Kennung");
    let (a, b) = enden(&doc, neu);
    assert!((a.y - 500.0).abs() < 1e-6 && (b.y - 500.0).abs() < 1e-6, "oben bei y=500: {a:?} {b:?}");
    assert!((a.x - 0.0).abs() < 1e-6 && (b.x - 2000.0).abs() < 1e-6, "Länge bleibt: {a:?} {b:?}");

    let unten = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 500.0, "side": [1000.0, -900.0]}),
    )
    .expect("Parallele unten");
    let neu = unten["created"][0].as_u64().expect("neue Kennung");
    let (a, b) = enden(&doc, neu);
    assert!((a.y + 500.0).abs() < 1e-6 && (b.y + 500.0).abs() < 1e-6, "unten bei y=-500: {a:?} {b:?}");
}

/// Der Betrag des Seitenpunktes zählt nicht — nur seine Seite.
#[test]
fn der_seitenpunkt_bestimmt_nur_die_seite() {
    let (mut doc, id) = doc_mit_linie();
    let nah = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 300.0, "side": [400.0, 10.0]}),
    )
    .expect("nah");
    let fern = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 300.0, "side": [400.0, 40000.0]}),
    )
    .expect("fern");
    let (a1, _) = enden(&doc, nah["created"][0].as_u64().unwrap());
    let (a2, _) = enden(&doc, fern["created"][0].as_u64().unwrap());
    assert!((a1.y - 300.0).abs() < 1e-6, "nah: {a1:?}");
    assert!((a2.y - 300.0).abs() < 1e-6, "fern: {a2:?}");
}

/// Ebene und Stil kommen von der Vorlage — eine Parallele ist eine Kopie ihrer
/// Quelle, keine neue Zeichnung mit den Vorgaben des Panels.
#[test]
fn parallele_erbt_ebene_und_stil() {
    let (mut doc, id) = doc_mit_linie();
    let farbe = jjcad_core::style::Color::BLACK;
    if let Some(e) = doc.entity_mut(id) {
        e.style.color = Some(farbe);
        e.layer = 3;
    }
    let antwort = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 200.0, "side": [0.0, 100.0]}),
    )
    .expect("Parallele");
    let neu = doc.entity(antwort["created"][0].as_u64().unwrap()).expect("neu");
    assert_eq!(neu.layer, 3, "Ebene der Vorlage");
    assert_eq!(neu.style.color, Some(farbe), "Farbe der Vorlage");
}

/// Eine Polylinie wird als Ganzes versetzt; der Knick bekommt die Gehrung.
#[test]
fn parallele_polylinie_folgt_dem_zug() {
    let mut doc = Document::new();
    doc.settings.input_unit = jjcad_core::units::Unit::Mm;
    let id = doc.new_id();
    doc.add_entity(Entity::new(
        id,
        doc.current_layer,
        EntityKind::Polyline {
            verts: vec![
                jjcad_core::geom::Vertex::new(v(0.0, 0.0)),
                jjcad_core::geom::Vertex::new(v(2000.0, 0.0)),
                jjcad_core::geom::Vertex::new(v(2000.0, 1000.0)),
            ],
            closed: false,
        },
    ));

    let antwort = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 500.0, "side": [1000.0, 400.0]}),
    )
    .expect("Parallele zur Polylinie");
    let neu = doc.entity(antwort["created"][0].as_u64().unwrap()).expect("neu");
    let EntityKind::Polyline { verts, closed } = &neu.kind else {
        panic!("keine Polylinie: {:?}", neu.kind);
    };
    assert_eq!(verts.len(), 3, "drei Stützpunkte");
    assert!(!closed, "offen bleibt offen");
    let pts: Vec<Vec2> = verts.iter().map(|q| q.p).collect();
    assert!((pts[0].y - 500.0).abs() < 1e-6, "erster Punkt versetzt: {:?}", pts[0]);
    // Gehrung am Knick: 500 mm nach innen in beiden Richtungen.
    assert!(
        (pts[1].x - 1500.0).abs() < 1e-6 && (pts[1].y - 500.0).abs() < 1e-6,
        "Knick auf der Gehrung: {:?}",
        pts[1]
    );
    assert!(
        (pts[2].x - 1500.0).abs() < 1e-6 && (pts[2].y - 1000.0).abs() < 1e-6,
        "Ende versetzt: {:?}",
        pts[2]
    );
}

/// Alles außer Linie und Polylinie ist ein Fehler, kein stiller Erfolg (T0038).
#[test]
fn andere_objektarten_werden_abgewiesen() {
    let mut doc = Document::new();
    doc.settings.input_unit = jjcad_core::units::Unit::Mm;
    let id = doc.new_id();
    doc.add_entity(Entity::new(
        id,
        doc.current_layer,
        EntityKind::Wall {
            a: v(0.0, 0.0),
            b: v(2000.0, 0.0),
            thickness: 240.0,
            height: 2500.0,
            material: String::new(),
            hatch: None,
            align: jjcad_core::style::WallAlign::Center,
        },
    ));
    let ergebnis = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 300.0, "side": [1000.0, 500.0]}),
    );
    assert!(ergebnis.is_err(), "eine Wand hat keine Parallele: {ergebnis:?}");
    assert_eq!(doc.ents().len(), 1, "nichts angelegt");
}

/// Ohne Vorlage gibt es nichts zu versetzen.
#[test]
fn ohne_vorlage_ist_es_ein_fehler() {
    let mut doc = Document::new();
    doc.settings.input_unit = jjcad_core::units::Unit::Mm;
    let fehlt = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"distance": 300.0, "side": [0.0, 500.0]}),
    );
    assert!(fehlt.is_err(), "ohne ids kein Erfolg: {fehlt:?}");
    let unbrauchbar = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [99], "distance": 300.0, "side": [0.0, 500.0]}),
    );
    assert!(unbrauchbar.is_err(), "unbekannte Kennung: {unbrauchbar:?}");
}

/// Abstand null ist keine Parallele — sonst entstünde eine deckungsgleiche
/// Kopie, die im Bild wie ein Fehler aussieht.
#[test]
fn abstand_null_wird_abgewiesen() {
    let (mut doc, id) = doc_mit_linie();
    let ergebnis = actions::execute(
        &mut doc,
        "draw.parallel",
        &json!({"ids": [id], "distance": 0.0, "side": [1000.0, 100.0]}),
    );
    assert!(ergebnis.is_err(), "Abstand 0: {ergebnis:?}");
    assert_eq!(doc.ents().len(), 1, "nichts angelegt");
}

/// Der gemessene Abstand des Zeigers ist der senkrechte — auch wenn der Zeiger
/// seitlich weit über das Linienende hinaus steht.
#[test]
fn abstand_wird_senkrecht_gemessen() {
    let (doc, id) = doc_mit_linie();
    let e = doc.entity(id).expect("Linie");
    let d = actions::parallel_abstand(e, v(9000.0, 750.0)).expect("Abstand");
    assert!((d - 750.0).abs() < 1e-6, "senkrecht gemessen: {d}");
    assert_eq!(actions::parallel_abstand(e, v(0.0, 0.0)), Some(0.0));
}
