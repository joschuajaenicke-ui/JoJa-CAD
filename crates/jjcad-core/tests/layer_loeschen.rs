//! T0213 — Layer mit Objekten löschen.
//!
//! Ohne Ziel bleibt es beim Fehler; mit `move_to` wandern die Objekte auf den
//! Ziellayer (Modell- und Papierraum) und der Layer geht weg.

use jjcad_core::actions;
use jjcad_core::model::{Document, Entity, EntityKind};
use serde_json::json;

fn dokument_mit_linie() -> (Document, u64, u64) {
    let mut doc = Document::new();
    let quelle = doc.layer_id_by_name("Zeichnung").expect("Zeichnung");
    let ziel = doc.layer_id_by_name("Architektur").expect("Architektur");
    actions::execute(
        &mut doc,
        "draw.line",
        &json!({"a": [0.0, 0.0], "b": [2000.0, 0.0], "layer": quelle}),
    )
    .expect("Linie");
    (doc, quelle, ziel)
}

#[test]
fn ohne_ziel_bleibt_der_layer_stehen() {
    let (mut doc, quelle, _ziel) = dokument_mit_linie();
    assert_eq!(doc.layer_objekte(quelle), 1);
    let r = actions::execute(&mut doc, "layer.delete", &json!({"layer": quelle}));
    let fehler = r.expect_err("Layer mit Objekten darf nicht still verschwinden");
    assert!(fehler.contains("Layer enthält noch Objekte"), "{fehler}");
    assert!(doc.layer(quelle).is_some(), "der Layer bleibt");
    assert_eq!(doc.layer_objekte(quelle), 1, "das Objekt bleibt");
}

#[test]
fn mit_ziel_wandern_die_objekte_und_der_layer_geht() {
    let (mut doc, quelle, ziel) = dokument_mit_linie();
    actions::execute(
        &mut doc,
        "layer.delete",
        &json!({"layer": quelle, "move_to": ziel}),
    )
    .expect("verschieben und löschen");
    assert!(doc.layer(quelle).is_none(), "der Layer ist weg");
    assert_eq!(doc.layer_objekte(quelle), 0);
    assert_eq!(doc.layer_objekte(ziel), 1, "das Objekt liegt auf dem Ziel");
    let e = doc
        .entities
        .iter()
        .find(|e| e.kind.type_name() == "Linie")
        .expect("Linie");
    assert_eq!(e.layer, ziel);
}

#[test]
fn ziellayer_darf_auch_ein_name_sein() {
    let (mut doc, quelle, ziel) = dokument_mit_linie();
    actions::execute(
        &mut doc,
        "layer.delete",
        &json!({"layer": quelle, "move_to": "Architektur"}),
    )
    .expect("Name als Ziel");
    assert!(doc.layer(quelle).is_none());
    assert_eq!(doc.layer_objekte(ziel), 1);
}

#[test]
fn papierraum_zaehlt_mit_und_wandert_mit() {
    let (mut doc, quelle, ziel) = dokument_mit_linie();
    // Ein Objekt nur im Schriftfeld: der Layer ist damit nicht leer.
    let id = doc.new_id();
    let mut e = Entity::new(
        id,
        quelle,
        EntityKind::Line {
            a: jjcad_core::geom::v(0.0, 0.0),
            b: jjcad_core::geom::v(50.0, 0.0),
        },
    );
    doc.sheets[0].entities.push(e);
    assert_eq!(doc.layer_objekte(quelle), 2);

    let r = actions::execute(&mut doc, "layer.delete", &json!({"layer": quelle}));
    assert!(r.is_err(), "auch der Papierraum hält den Layer");

    actions::execute(
        &mut doc,
        "layer.delete",
        &json!({"layer": quelle, "move_to": ziel}),
    )
    .expect("mit Ziel geht es");
    assert_eq!(doc.layer_objekte(ziel), 2);
}

#[test]
fn ziel_gleich_quelle_ist_ein_fehler() {
    let (mut doc, quelle, _ziel) = dokument_mit_linie();
    let r = actions::execute(
        &mut doc,
        "layer.delete",
        &json!({"layer": quelle, "move_to": quelle}),
    );
    assert!(r.is_err(), "auf sich selbst verschieben ist kein Ziel");
    assert!(doc.layer(quelle).is_some());
    assert_eq!(doc.layer_objekte(quelle), 1);
}

#[test]
fn unbekanntes_ziel_ist_ein_fehler() {
    let (mut doc, quelle, _ziel) = dokument_mit_linie();
    let r = actions::execute(
        &mut doc,
        "layer.delete",
        &json!({"layer": quelle, "move_to": 999_999}),
    );
    let fehler = r.expect_err("unbekanntes Ziel");
    assert!(fehler.contains("Ziellayer nicht gefunden"), "{fehler}");
    assert_eq!(doc.layer_objekte(quelle), 1);
}
