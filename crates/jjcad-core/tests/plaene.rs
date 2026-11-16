//! Pläne und Layer (Ticket T0202).
//!
//! Der Layer-Manager hat **eine** Stufe: Layer gehören zu einem Plan. Pläne
//! sortieren Geschosse, Schnitte, Ansichten und Lagepläne; jeder Plan lässt
//! sich als Ganzes ausblenden und sperren. Eine neue Zeichnung besitzt genau
//! einen Plan („Plan 1“) mit den vier Standardlayern Zeichnung, Architektur,
//! Beschriftung und Blöcke.
//!
//! Geprüft wird über die Befehls-Schicht (`actions::execute`) und die
//! Anzeigeliste bzw. den Fang — dieselben Wege wie GUI, MCP und Druck.

use jjcad_core::actions;
use jjcad_core::model::Document;
use jjcad_core::render::{self, RenderCtx};
use jjcad_core::style::LAYER_DEFAULTS;
use serde_json::{json, Value};

fn neu(doc: &mut Document, action: &str, params: Value) -> u64 {
    let r = actions::execute(doc, action, &params)
        .unwrap_or_else(|e| panic!("{action} fehlgeschlagen: {e}"));
    r["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{action} hat kein Objekt erzeugt"))
}

/// Ein Dokument mit **zwei** Plänen: Plan 1 trägt eine Wand („Architektur“),
/// Plan 2 eine Linie („Zeichnung“). Beide Pläne haben ihre eigenen Layer.
fn dokument() -> (Document, u64, u64, u64) {
    let mut d = Document::new();
    let wand = neu(
        &mut d,
        "draw.wall",
        json!({"a": [0, 0], "b": [4, 0], "thickness": 0.2}),
    );
    let r = actions::execute(&mut d, "plan.create", &json!({"name": "Obergeschoss"})).unwrap();
    let plan2 = r["id"].as_u64().unwrap();
    // Im zweiten Plan zeichnen: seine „Zeichnung“ ist die aktuelle Ebene.
    let zeichnung2 = d
        .layers_of_plan(plan2)
        .iter()
        .find(|l| l.name == "Zeichnung")
        .map(|l| l.id)
        .expect("Zeichnung im zweiten Plan");
    d.current_layer = zeichnung2;
    let linie = neu(&mut d, "draw.line", json!({"a": [0, 2], "b": [4, 2]}));
    (d, wand, linie, plan2)
}

fn anzahl_primitive(doc: &Document) -> usize {
    render::render_document(&RenderCtx::new(doc)).prims.len()
}

#[test]
fn neue_zeichnung_hat_einen_plan_mit_vier_layern() {
    let d = Document::new();
    assert_eq!(d.plans.len(), 1, "eine neue Datei hat genau einen Plan");
    assert_eq!(d.plans[0].name, "Plan 1");
    assert!(d.plans[0].visible && !d.plans[0].locked);

    let namen: Vec<&str> = d.layers.iter().map(|l| l.name.as_str()).collect();
    let soll: Vec<&str> = LAYER_DEFAULTS.iter().map(|(n, _, _)| *n).collect();
    assert_eq!(namen, soll, "die vier Standardlayer in fester Reihenfolge");
    assert_eq!(soll, vec!["Zeichnung", "Architektur", "Beschriftung", "Blöcke"]);
    for l in &d.layers {
        assert_eq!(l.plan, d.plans[0].id, "jeder Layer gehört zum Plan");
    }
    assert_eq!(
        d.layer(d.current_layer).map(|l| l.name.as_str()),
        Some("Zeichnung"),
        "gezeichnet wird auf „Zeichnung“"
    );
}

#[test]
fn neuer_plan_bringt_die_vier_layer_mit() {
    let mut d = Document::new();
    let r = actions::execute(&mut d, "plan.create", &json!({"name": "Erdgeschoss"})).unwrap();
    let pid = r["id"].as_u64().unwrap();
    assert_eq!(d.plans.len(), 2);
    assert_eq!(d.plan(pid).map(|p| p.name.as_str()), Some("Erdgeschoss"));

    let namen: Vec<String> = d
        .layers_of_plan(pid)
        .iter()
        .map(|l| l.name.clone())
        .collect();
    assert_eq!(
        namen,
        vec!["Zeichnung", "Architektur", "Beschriftung", "Blöcke"],
        "jeder Plan besteht aus denselben vier Layern"
    );
    assert_eq!(d.layers.len(), 8, "die Layer des ersten Plans bleiben");

    // Ein Name gilt je Plan, nicht global: „Zeichnung“ gibt es zweimal.
    let z = d
        .layers
        .iter()
        .filter(|l| l.name == "Zeichnung")
        .count();
    assert_eq!(z, 2);
}

#[test]
fn layer_entsteht_im_angegebenen_plan() {
    let mut d = Document::new();
    let r = actions::execute(&mut d, "plan.create", &json!({"name": "Schnitt"})).unwrap();
    let pid = r["id"].as_u64().unwrap();
    let l = actions::execute(
        &mut d,
        "layer.create",
        &json!({"name": "Schatten", "plan": pid}),
    )
    .unwrap();
    let lid = l["id"].as_u64().unwrap();
    assert_eq!(d.layer(lid).map(|x| x.plan), Some(pid));
    // Der neue Layer steht bei seinem Plan, nicht am Ende der ganzen Liste.
    let letzter_von_plan = d.layers_of_plan(pid).last().map(|x| x.id);
    assert_eq!(letzter_von_plan, Some(lid));
}

#[test]
fn ausgeblendeter_plan_blendet_seine_layer_aus() {
    let (mut d, wand, linie, _plan2) = dokument();
    let architektur = d.layer(wand_ebene(&d)).unwrap().plan;
    let vorher = anzahl_primitive(&d);
    assert!(vorher > 0);

    actions::execute(
        &mut d,
        "plan.update",
        &json!({"plan": architektur, "visible": false}),
    )
    .unwrap();

    // Der Layer selbst bleibt sichtbar — nur der Plan ist zu.
    let lid = d.entity(wand).unwrap().layer;
    assert!(d.layer(lid).unwrap().visible, "der Layer bleibt eingeschaltet");
    assert!(!d.layer_sichtbar(lid), "wirksam unsichtbar");
    assert!(d.layer_gesperrt(lid), "unsichtbar zählt wie gesperrt");
    assert!(!d.visible_layers().contains(&lid));

    // Die Anzeigeliste enthält kein Primitiv der Wand mehr, die Linie bleibt.
    let dl = render::render_document(&RenderCtx::new(&d));
    assert!(
        !dl.prims.iter().any(|p| p.entity == wand),
        "ausgeblendeter Plan darf nicht gezeichnet werden"
    );
    assert!(
        dl.prims.iter().any(|p| p.entity == linie),
        "die Zeichnung des anderen Plans bleibt"
    );

    // Auswahl über „alles“ überspringt die Layer des Plans.
    let r = actions::execute(&mut d, "select.all", &json!({})).unwrap();
    let ids: Vec<u64> = r["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(ids.contains(&linie));
    assert!(!ids.contains(&wand));

    // Zurückschalten stellt den vorherigen Zustand wieder her.
    actions::execute(
        &mut d,
        "plan.update",
        &json!({"plan": architektur, "visible": true}),
    )
    .unwrap();
    assert!(d.layer_sichtbar(lid));
    assert_eq!(anzahl_primitive(&d), vorher);
}

#[test]
fn gesperrter_plan_sperrt_seine_layer() {
    let (mut d, wand, linie, _plan2) = dokument();
    let pid = d.layer(wand_ebene(&d)).unwrap().plan;
    actions::execute(
        &mut d,
        "plan.update",
        &json!({"plan": pid, "locked": true}),
    )
    .unwrap();
    let lid = d.entity(wand).unwrap().layer;
    assert!(d.layer(lid).unwrap().visible, "Sperren blendet nicht aus");
    assert!(d.layer_gesperrt(lid), "der Layer ist gesperrt");
    assert!(d.layer_locked(lid), "dieselbe Auskunft über layer_locked");
    // Gezeichnet wird weiter, aber nicht mehr gewählt.
    let dl = render::render_document(&RenderCtx::new(&d));
    assert!(dl.prims.iter().any(|p| p.entity == wand));
    let r = actions::execute(&mut d, "select.all", &json!({})).unwrap();
    let ids: Vec<u64> = r["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(!ids.contains(&wand));
    assert!(ids.contains(&linie));
}

/// Ebene der Wand („Architektur“ des ersten Plans).
fn wand_ebene(d: &Document) -> u64 {
    d.layers
        .iter()
        .find(|l| l.name == "Architektur")
        .expect("Ebene fehlt")
        .id
}

#[test]
fn plaene_ueberleben_speichern_und_laden() {
    let mut d = Document::new();
    let r = actions::execute(&mut d, "plan.create", &json!({"name": "Ansicht"}))
        .unwrap();
    let pid = r["id"].as_u64().unwrap();
    actions::execute(
        &mut d,
        "plan.update",
        &json!({"plan": pid, "visible": false, "collapsed": true}),
    )
    .unwrap();

    let text = jjcad_core::persist::to_json(&d).unwrap();
    let geladen = jjcad_core::persist::from_json(&text).unwrap();
    assert_eq!(geladen.plans.len(), 2);
    let p = geladen.plan(pid).expect("Plan fehlt");
    assert_eq!(p.name, "Ansicht");
    assert!(!p.visible && p.collapsed);
    assert_eq!(geladen.layers_of_plan(pid).len(), 4);
    assert_eq!(geladen.layers.len(), 8);
}

#[test]
fn alte_datei_ohne_plaene_bekommt_einen_plan() {
    // Datei vor T0202: Layer ohne `plan`-Feld, keine Pläne.
    let alt = json!({
        "format": "jjcad",
        "version": 2,
        "name": "Alt",
        "layers": [
            {"id": 1, "name": "Wände", "color": "#000000"},
            {"id": 2, "name": "Text", "color": "#000000"}
        ],
        "current_layer": 1,
        "entities": []
    });
    let d = jjcad_core::persist::from_json(&alt.to_string()).expect("alte Datei");
    assert_eq!(d.plans.len(), 1, "ein Plan wird ergänzt");
    assert_eq!(d.plans[0].name, "Plan 1");
    for l in &d.layers {
        assert_eq!(l.plan, d.plans[0].id, "alle Layer gehören dem Plan");
    }
    assert_eq!(d.layers.len(), 2, "die Layer selbst bleiben");
}

#[test]
fn letzter_layer_eines_plans_bleibt_stehen() {
    let mut d = Document::new();
    let plan = d.plans[0].id;
    // Drei Layer löschen geht, der vierte nicht.
    for name in ["Blöcke", "Beschriftung", "Architektur"] {
        let id = d.layer_id_by_name(name).expect("Ebene");
        actions::execute(&mut d, "layer.delete", &json!({"layer": id}))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    assert_eq!(d.layers_of_plan(plan).len(), 1);
    let letzter = d.layer_id_by_name("Zeichnung").expect("Ebene");
    let r = actions::execute(&mut d, "layer.delete", &json!({"layer": letzter}));
    assert!(r.is_err(), "der letzte Layer eines Plans darf nicht weg");
    assert_eq!(d.layers.len(), 1);
}
