//! Bemaßung ist schwarz (T0062).
//!
//! Die Ebene „Bemaßung“ trägt als Vorgabe ein Rot, und jede neue Maßkette
//! erbte diese Farbe über „vom Layer“ — alle Maße waren rot. Jetzt setzt jeder
//! Bemaßungs-Erzeuger ausdrücklich Schwarz; eine mitgegebene Farbe (auch
//! `bylayer`) sticht weiterhin. Geprüft über die Befehls-Schicht — denselben
//! Weg, den GUI, MCP und Tastatur nehmen.

use jjcad_core::actions;
use jjcad_core::model::{Document, Entity, EntityKind};
use jjcad_core::style::Color;
use serde_json::{json, Value};

/// Führt eine Aktion aus und liefert das erzeugte Objekt.
fn neu(doc: &mut Document, action: &str, params: Value) -> Entity {
    let r = actions::execute(doc, action, &params)
        .unwrap_or_else(|e| panic!("{action} fehlgeschlagen: {e}"));
    let id = r["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{action} hat kein Objekt erzeugt"));
    doc.entity(id)
        .cloned()
        .unwrap_or_else(|| panic!("Objekt {id} fehlt"))
}

/// Ebene des Objekts.
fn ebene<'a>(doc: &'a Document, e: &Entity) -> &'a str {
    doc.layer(e.layer).map(|l| l.name.as_str()).unwrap_or("")
}

/// Alle drei Bemaßungs-Erzeuger ohne Farbangabe.
fn ohne_farbe(doc: &mut Document) -> Vec<Entity> {
    vec![
        neu(
            doc,
            "dim.linear",
            json!({"a": [0, 0], "b": [5, 0], "line_pos": [0, -0.5]}),
        ),
        neu(
            doc,
            "dim.chain",
            json!({"pts": [[0, 0], [2, 0], [5, 0]], "line_pos": [0, -0.8]}),
        ),
        neu(doc, "dim.elevation", json!({"p": [0, 0], "z": 1.0})),
    ]
}

/// Ein Neudokument zeichnet auf „Wände“; die Bemaßungsebene selbst ist rot.
/// Trotzdem ist jedes neue Maß **schwarz**.
#[test]
fn neue_bemassung_ist_schwarz() {
    let mut doc = Document::new();
    let rot = doc
        .layer_by_name("Bemaßung")
        .map(|l| l.color)
        .expect("Ebene „Bemaßung“ fehlt");
    assert_ne!(rot, Color::BLACK, "Ausgangslage: die Ebene trägt Rot");

    let erzeugt = ohne_farbe(&mut doc);
    assert_eq!(erzeugt.len(), 3);
    for e in &erzeugt {
        assert_eq!(
            doc.effective_color(e),
            Color::BLACK,
            "{} auf „{}“ ist {} statt schwarz",
            match e.kind {
                EntityKind::Dim { .. } => "Einzelmaß",
                EntityKind::DimChain { .. } => "Maßkette",
                _ => "Höhenkote",
            },
            ebene(&doc, e),
            doc.effective_color(e).to_hex()
        );
    }
}

/// Eine ausdrückliche Farbe gewinnt — die Oberfläche reicht hier die Wahl aus
/// dem Eigenschaften-Panel durch.
#[test]
fn ausdrueckliche_farbe_gewinnt() {
    let mut doc = Document::new();
    let e = neu(
        &mut doc,
        "dim.chain",
        json!({"pts": [[0, 0], [2, 0]], "line_pos": [0, -0.8],
               "color": Color::BLUE.to_hex()}),
    );
    assert_eq!(e.style.color, Some(Color::BLUE));
    assert_eq!(doc.effective_color(&e), Color::BLUE);
}

/// „vom Layer“ bleibt ein Wunsch, kein Zufall: wer `bylayer` ausdrücklich
/// verlangt, bekommt die Ebenenfarbe — die Schwarz-Vorgabe greift dann nicht.
#[test]
fn ausdrueckliches_bylayer_bleibt_bylayer() {
    let mut doc = Document::new();
    let e = neu(
        &mut doc,
        "dim.linear",
        json!({"a": [0, 0], "b": [5, 0], "line_pos": [0, -0.5], "color": "bylayer",
               "layer": "Bemaßung"}),
    );
    assert_eq!(e.style.color, None, "keine eigene Farbe gespeichert");
    assert_eq!(ebene(&doc, &e), "Bemaßung");
    assert_eq!(
        doc.effective_color(&e),
        doc.layer_by_name("Bemaßung").map(|l| l.color).unwrap(),
        "Farbe kommt aus der Ebene"
    );
}

/// Der Wert steht so in der gespeicherten Datei — nicht nur im Speicher.
#[test]
fn schwarz_steht_in_der_gespeicherten_datei() {
    let mut doc = Document::new();
    let e = neu(
        &mut doc,
        "dim.chain",
        json!({"pts": [[0, 0], [2, 0], [5, 0]], "line_pos": [0, -0.8]}),
    );
    let text = jjcad_core::persist::to_json(&doc).expect("Datei nicht schreibbar");
    let json: Value = serde_json::from_str(&text).expect("JSON unlesbar");
    let gespeichert = json["entities"]
        .as_array()
        .expect("entities fehlt")
        .iter()
        .find(|x| x["id"].as_u64() == Some(e.id))
        .expect("Maßkette fehlt in der Datei");
    assert_eq!(
        gespeichert["style"]["color"].as_str(),
        Some("#1C1F24"),
        "gespeicherte Farbe der Maßkette"
    );
}
