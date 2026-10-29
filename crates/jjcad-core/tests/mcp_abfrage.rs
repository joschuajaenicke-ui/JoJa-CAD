//! MCP-Abfrage `jjcad_query`: `type`, `layer` und `bbox` filtern wirklich.
//!
//! `Session::query` wertete früher nur `ids` aus — die anderen Filter standen
//! im Schema, wurden aber still ignoriert, und die Abfrage lieferte die ganze
//! Objektliste. Beide Wege (Action `select.query`, `Session::query`) laufen
//! jetzt über `actions::query_ids`; der Test prüft Trefferzahl **und** Typen.
//!
//! Achtung: Eine neue Zeichnung rechnet in **Metern** (`Document::new`), die
//! Koordinaten hier sind also Meter; `units: "mm"` im Bereich schaltet um.

use jjcad_core::actions;
use jjcad_core::session::Session;
use serde_json::{json, Value};

/// Wand (Ebene Wände) + Text (Ebene Text) + Maß (Ebene Bemaßung).
fn probedokument() -> (Session, u64, u64, u64) {
    let mut s = Session::new();
    let wall = s
        .execute(
            "draw.wall",
            &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.24}),
        )
        .expect("Wand")["created"][0]
        .as_u64()
        .unwrap();
    let text = s
        .execute(
            "draw.text",
            &json!({"p": [0, 3], "text": "Wohnzimmer", "layer": "Text"}),
        )
        .expect("Text")["created"][0]
        .as_u64()
        .unwrap();
    let mass = s
        .execute(
            "dim.linear",
            &json!({"a": [0, 3], "b": [3, 3], "line_pos": [1.5, 3.2]}),
        )
        .expect("Maß")["created"][0]
        .as_u64()
        .unwrap();
    (s, wall, text, mass)
}

fn typen(res: &Value) -> Vec<String> {
    res["entities"]
        .as_array()
        .expect("entities fehlt")
        .iter()
        .map(|e| e["type"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn ohne_filter_alle_objekte() {
    let (s, _, _, _) = probedokument();
    let res = s.query(&json!({}));
    assert_eq!(res["count"], 3, "leere Abfrage liefert alles: {res}");
}

#[test]
fn typ_filter_trifft_nur_den_typ() {
    let (s, _, _, _) = probedokument();
    let res = s.query(&json!({"type": "Wand"}));
    assert_eq!(res["count"], 1, "nur die Wand: {res}");
    assert_eq!(typen(&res), vec!["wall"]);

    let res = s.query(&json!({"type": "Text"}));
    assert_eq!(typen(&res), vec!["text"]);

    let res = s.query(&json!({"type": "Maß"}));
    assert_eq!(typen(&res), vec!["dim"]);

    // Die Schreibweise aus dem Abfrageergebnis filtert ebenfalls.
    let res = s.query(&json!({"type": "dim"}));
    assert_eq!(typen(&res), vec!["dim"]);
}

#[test]
fn unbekannter_typ_liefert_null_treffer() {
    let (s, _, _, _) = probedokument();
    let res = s.query(&json!({"type": "Quatsch"}));
    assert_eq!(
        res["count"], 0,
        "unbekannter Typ darf nichts treffen: {res}"
    );
    assert!(res["entities"].as_array().unwrap().is_empty());
}

#[test]
fn ebene_filter_trifft_nur_diese_ebene() {
    let (s, _, _, _) = probedokument();
    let res = s.query(&json!({"layer": "Bemaßung"}));
    assert_eq!(res["count"], 1, "nur das Maß: {res}");
    assert_eq!(typen(&res), vec!["dim"]);

    let res = s.query(&json!({"layer": "Text"}));
    assert_eq!(typen(&res), vec!["text"]);

    let res = s.query(&json!({"layer": "Gibt es nicht"}));
    assert_eq!(res["count"], 0, "unbekannte Ebene trifft nichts: {res}");
}

#[test]
fn bereich_filter_schneidet_geometrie() {
    let (s, wall, _, _) = probedokument();
    // Fenster nur um die Wand (y −500…500 mm) — Text und Maß liegen bei 3 m.
    let res = s.query(&json!({"a": [-500, -500], "b": [5500, 500], "units": "mm"}));
    assert_eq!(res["count"], 1, "nur die Wand im Bereich: {res}");
    assert_eq!(res["entities"][0]["id"], wall);

    // Bereich um Text und Maß.
    let res = s.query(&json!({"a": [-500, 2500], "b": [3500, 3500], "units": "mm"}));
    assert_eq!(res["count"], 2, "Text und Maß: {res}");
}

#[test]
fn bereich_und_typ_wirken_zusammen() {
    let (s, _, _, _) = probedokument();
    let res = s.query(&json!({
        "a": [-500, 2500], "b": [3500, 3500], "units": "mm", "type": "Maß"
    }));
    assert_eq!(typen(&res), vec!["dim"]);
}

#[test]
fn ids_filter_bleibt_erhalten() {
    let (s, _, text, _) = probedokument();
    let res = s.query(&json!({"ids": [text]}));
    assert_eq!(res["count"], 1);
    assert_eq!(res["entities"][0]["id"], text);
}

/// Die Action `select.query` und die Sitzungsabfrage dürfen nicht
/// auseinanderlaufen — eine Quelle (`actions::query_ids`).
#[test]
fn action_und_sitzung_filtern_gleich() {
    let (s, _, _, _) = probedokument();
    for params in [
        json!({"type": "Wand"}),
        json!({"type": "Quatsch"}),
        json!({"layer": "Bemaßung"}),
        json!({"a": [0, -500], "b": [5000, 500], "units": "mm"}),
    ] {
        let action =
            actions::execute(&mut s.doc.clone(), "select.query", &params).expect("Abfrage");
        let session = s.query(&params);
        assert_eq!(
            action["count"], session["count"],
            "Filter {params} liefert in Action und Session verschiedene Zahlen"
        );
    }
}
