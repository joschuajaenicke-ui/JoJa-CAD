//! Layer und Plan getrennt wählen (Ticket T0245).
//!
//! Im Eigenschaften-Panel stehen Layer und Plan in **einem** Dropdown, aber in
//! zwei Abschnitten. Die Befehlsschicht bekommt dafür zwei getrennte Aufträge:
//!
//! - `layer_name` — dieser Layername im Plan **jedes** Objekts,
//! - `plan` — dieser Zielplan, die Objekte behalten ihren **Layernamen**.
//!
//! Damit lassen sich mehrere Objekte von verschiedenen Layern gemeinsam auf
//! einen anderen Plan verschieben, ohne ihre Layerstruktur zu verlieren.

use jjcad_core::actions;
use jjcad_core::model::Document;
use serde_json::{json, Value};

fn neu(doc: &mut Document, action: &str, params: Value) -> u64 {
    let r = actions::execute(doc, action, &params)
        .unwrap_or_else(|e| panic!("{action} fehlgeschlagen: {e}"));
    r["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{action} hat kein Objekt erzeugt"))
}

/// Linie („Zeichnung“) und Text („Beschriftung“) im Plan 1, dazu ein zweiter
/// Plan mit seinen vier Standardlayern. Liefert (doc, linie, text, plan2).
fn zeichnung() -> (Document, u64, u64, u64) {
    let mut d = Document::new();
    let linie = neu(&mut d, "draw.line", json!({"a": [0, 0], "b": [4, 0]}));
    let text = neu(
        &mut d,
        "draw.text",
        json!({"p": [1, 1], "text": "Hallo", "height": 0.35}),
    );
    let plan2 = actions::execute(&mut d, "plan.create", &json!({"name": "Plan 2"}))
        .expect("Plan 2")["id"]
        .as_u64()
        .expect("Plan-Id");
    (d, linie, text, plan2)
}

fn layername(doc: &Document, id: u64) -> String {
    doc.layer(doc.entity(id).expect("Objekt").layer)
        .expect("Layer")
        .name
        .clone()
}

fn plan_von(doc: &Document, id: u64) -> u64 {
    doc.plan_of_layer(doc.entity(id).expect("Objekt").layer)
        .expect("Plan")
        .id
}

#[test]
fn planwechsel_behaelt_den_layernamen() {
    let (mut d, linie, text, plan2) = zeichnung();
    assert_eq!(layername(&d, linie), "Zeichnung");
    assert_eq!(layername(&d, text), "Beschriftung");
    let plan1 = plan_von(&d, linie);
    assert_eq!(plan_von(&d, text), plan1, "beide liegen in Plan 1");

    actions::execute(&mut d, "modify.set", &json!({"ids": [linie, text], "plan": plan2}))
        .expect("Planwechsel");

    // Beide im Zielplan, jeder auf seinem Layer — die Struktur bleibt.
    assert_eq!(plan_von(&d, linie), plan2, "Linie in Plan 2");
    assert_eq!(plan_von(&d, text), plan2, "Text in Plan 2");
    assert_eq!(layername(&d, linie), "Zeichnung");
    assert_eq!(layername(&d, text), "Beschriftung");
    // Und zwar auf den Ebenen des **Zielplans**, nicht auf den alten.
    let ziel_zeichnung = d
        .layers_of_plan(plan2)
        .iter()
        .find(|l| l.name == "Zeichnung")
        .map(|l| l.id)
        .expect("Zeichnung in Plan 2");
    assert_eq!(d.entity(linie).expect("Linie").layer, ziel_zeichnung);
    assert_ne!(d.entity(linie).expect("Linie").layer, 0);
}

#[test]
fn planwechsel_auf_mehreren_layern_gleicher_name_eine_ebene() {
    // Zwei Linien auf demselben Layer: der Planwechsel legt **eine** Ebene im
    // Zielplan an, nicht je Objekt eine (sonst wüchse der Layer-Manager).
    let (mut d, linie, _text, plan2) = zeichnung();
    let linie2 = neu(&mut d, "draw.line", json!({"a": [0, 1], "b": [4, 1]}));
    let vorher = d.layers.len();
    actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [linie, linie2], "plan": plan2}),
    )
    .expect("Planwechsel");
    assert_eq!(d.entity(linie).unwrap().layer, d.entity(linie2).unwrap().layer);
    assert_eq!(
        d.layers_of_plan(plan2)
            .iter()
            .filter(|l| l.name == "Zeichnung")
            .count(),
        1,
        "genau eine Zeichnung-Ebene im Zielplan"
    );
    assert!(d.layers.len() <= vorher + 1, "keine Ebenenflut");
}

#[test]
fn layername_gilt_im_plan_jedes_objekts() {
    let (mut d, linie, text, plan2) = zeichnung();
    // Den Text vorher in Plan 2 schieben — dann liegen die Objekte in
    // verschiedenen Plänen, und derselbe Layername muss in jedem Plan gelten.
    actions::execute(&mut d, "modify.set", &json!({"ids": [text], "plan": plan2}))
        .expect("Text nach Plan 2");
    let arch1 = d
        .layers_of_plan(plan_von(&d, linie))
        .iter()
        .find(|l| l.name == "Architektur")
        .map(|l| l.id)
        .expect("Architektur in Plan 1");

    actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [linie, text], "layer_name": "Architektur"}),
    )
    .expect("Layername");

    assert_eq!(layername(&d, linie), "Architektur");
    assert_eq!(layername(&d, text), "Architektur");
    assert_ne!(plan_von(&d, linie), plan_von(&d, text), "Pläne bleiben getrennt");
    assert_eq!(d.entity(linie).unwrap().layer, arch1, "Plan 1 behält seine Ebene");
}

#[test]
fn unbekannter_plan_ist_ein_fehler() {
    let (mut d, linie, _text, _plan2) = zeichnung();
    let fehler = actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [linie], "plan": 999_999}),
    );
    assert!(fehler.is_err(), "unbekannter Plan ist kein stiller Erfolg");
}

#[test]
fn plan_als_name_wird_angenommen() {
    let (mut d, linie, text, _plan2) = zeichnung();
    actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [linie, text], "plan": "Plan 2"}),
    )
    .expect("Plan über den Namen");
    assert_eq!(d.plans[1].name, "Plan 2");
    assert_eq!(plan_von(&d, linie), d.plans[1].id);
    assert_eq!(plan_von(&d, text), d.plans[1].id);
}
