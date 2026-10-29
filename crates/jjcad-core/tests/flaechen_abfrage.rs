//! Flächen über die Schnittstelle: `area_m2` in der Objektbeschreibung und die
//! Aktion `select.area` (Einzelflächen + Summe).
//!
//! Vorher kannte nur das Eigenschaften-Panel eine Fläche; ein Agent konnte
//! nicht prüfen, ob ein Raum die geforderte Größe hat. Der Test hält beide
//! Wege zusammen: `Session::describe_entity` (MCP `jjcad_query`) und
//! `select.area` rechnen über `area::entity_area` — dieselbe Quelle wie das
//! Panel.
//!
//! Eine neue Zeichnung rechnet in **Metern** (`Document::new`).

use jjcad_core::actions;
use jjcad_core::session::Session;
use serde_json::{json, Value};

fn id_von(res: &Value) -> u64 {
    res["created"][0].as_u64().expect("kein created")
}

/// 4 × 3 m über zwei gegenüberliegende Ecken (Standard-Einheit Meter).
fn rechteck(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> u64 {
    id_von(
        &s.execute("draw.rect", &json!({"a": a, "b": b}))
            .expect("Rechteck"),
    )
}

fn beschreibung(s: &Session, id: u64) -> Value {
    let res = s.query(&json!({"ids": [id]}));
    assert_eq!(res["count"], 1, "Objekt nicht gefunden: {res}");
    res["entities"][0].clone()
}

#[test]
fn rechteck_hat_flaeche_als_deutsche_zahl() {
    let mut s = Session::new();
    let id = rechteck(&mut s, [0.0, 0.0], [4.0, 3.0]);
    let e = beschreibung(&s, id);
    assert_eq!(
        e["area_m2"], "12,00",
        "4 × 3 m müssen 12,00 m² ergeben: {e}"
    );
    assert_eq!(e["area_mm2"], 12_000_000.0, "{e}");
}

#[test]
fn offener_linienzug_hat_keine_flaeche() {
    let mut s = Session::new();
    let id = id_von(
        &s.execute(
            "draw.polyline",
            &json!({"pts": [[0, 0], [4, 0], [4, 3]], "closed": false}),
        )
        .expect("Polylinie"),
    );
    let e = beschreibung(&s, id);
    assert!(
        e.get("area_m2").is_none(),
        "offener Zug darf kein Flächenfeld tragen: {e}"
    );
    assert!(e.get("area_mm2").is_none(), "{e}");
}

#[test]
fn geschlossener_linienzug_hat_flaeche() {
    let mut s = Session::new();
    let id = id_von(
        &s.execute(
            "draw.polyline",
            &json!({"pts": [[0, 0], [2, 0], [2, 2], [0, 2]], "closed": true}),
        )
        .expect("Polylinie"),
    );
    assert_eq!(beschreibung(&s, id)["area_m2"], "4,00");
}

#[test]
fn kreis_und_ellipse_haben_flaeche() {
    let mut s = Session::new();
    let kreis = id_von(
        &s.execute("draw.circle", &json!({"c": [0, 0], "r": 1}))
            .expect("Kreis"),
    );
    // π · 1 m² = 3,1416 → 3,14
    assert_eq!(beschreibung(&s, kreis)["area_m2"], "3,14");

    // Halbachsen 2 m und 1 m → π · 2 m² = 6,2832 → 6,28
    let ellipse = id_von(
        &s.execute("draw.ellipse", &json!({"c": [0, 0], "rx": 2, "ry": 1}))
            .expect("Ellipse"),
    );
    assert_eq!(beschreibung(&s, ellipse)["area_m2"], "6,28");
}

#[test]
fn geschlossene_fuellflaeche_und_wandkoerper_haben_flaeche() {
    let mut s = Session::new();
    let fill = id_von(
        &s.execute(
            "draw.fill",
            &json!({"boundary": [[0, 0], [5, 0], [5, 2], [0, 2]]}),
        )
        .expect("Füllfläche"),
    );
    assert_eq!(beschreibung(&s, fill)["area_m2"], "10,00");

    // Wand 5 m lang, 0,24 m dick → 1,20 m²
    let wall = id_von(
        &s.execute(
            "draw.wall",
            &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.24}),
        )
        .expect("Wand"),
    );
    assert_eq!(beschreibung(&s, wall)["area_m2"], "1,20");
}

#[test]
fn text_und_mass_haben_keine_flaeche() {
    let mut s = Session::new();
    let text = id_von(
        &s.execute(
            "draw.text",
            &json!({"p": [1, 1], "text": "Wohnzimmer"}),
        )
        .expect("Text"),
    );
    assert!(beschreibung(&s, text).get("area_m2").is_none());
}

/// Zwei Räume summieren sich; der offene Zug bleibt ohne Fläche und ohne 0.
#[test]
fn select_area_summiert_und_laesst_offene_objekte_weg() {
    let mut s = Session::new();
    let r1 = rechteck(&mut s, [0.0, 0.0], [4.0, 3.0]); // 12,00 m²
    let r2 = rechteck(&mut s, [4.0, 0.0], [6.0, 3.0]); //  6,00 m²
    let offen = id_von(
        &s.execute(
            "draw.polyline",
            &json!({"pts": [[0, 5], [3, 5], [3, 6]], "closed": false}),
        )
        .expect("Polylinie"),
    );

    let res = actions::execute(
        &mut s.doc.clone(),
        "select.area",
        &json!({"ids": [r1, r2, offen]}),
    )
    .expect("select.area");

    assert_eq!(res["count"], 2, "nur die zwei Räume: {res}");
    assert_eq!(res["sum_m2"], "18,00", "{res}");
    assert_eq!(res["sum_mm2"], 18_000_000.0, "{res}");
    assert_eq!(res["without_area"], json!([offen]), "{res}");

    let areas = res["areas"].as_array().expect("areas fehlt");
    assert_eq!(areas[0]["id"], r1);
    assert_eq!(areas[0]["area_m2"], "12,00");
    assert_eq!(areas[1]["area_m2"], "6,00");
}

/// Ohne `ids` wertet die Aktion dieselben Filter wie `select.query` aus —
/// sonst gäbe es zwei Wege, die auseinanderlaufen.
#[test]
fn select_area_nimmt_dieselben_filter_wie_select_query() {
    let mut s = Session::new();
    let _r = rechteck(&mut s, [0.0, 0.0], [4.0, 3.0]);
    let _offen = s
        .execute("draw.line", &json!({"a": [0, 6], "b": [2, 6]}))
        .expect("Linie");

    let alle = actions::execute(&mut s.doc.clone(), "select.area", &json!({}))
        .expect("select.area");
    assert_eq!(alle["count"], 1, "nur das Rechteck: {alle}");
    assert_eq!(alle["sum_m2"], "12,00");

    let nichts = actions::execute(
        &mut s.doc.clone(),
        "select.area",
        &json!({"type": "gibt es nicht"}),
    )
    .expect("select.area");
    assert_eq!(nichts["count"], 0, "{nichts}");
    assert_eq!(nichts["sum_m2"], "0,00");
}
