//! Eigenschaften von Kreis und Bogen (T0022).
//!
//! Der Radius beider Typen und die beiden Endwinkel eines Bogens lassen sich
//! über `modify.set` ändern — das ist der Weg, den das Eigenschaften-Panel
//! nimmt (es schickt die Werte dort hinein, nicht über einen zweiten Pfad).
//! Alles in Millimetern, Winkel in der Befehlsschicht in **Grad**.

use jjcad_core::model::EntityKind;
use jjcad_core::persist;
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s
}

/// Kreis (1000;1000) mit 500 mm und Bogen um (0;0) mit 400 mm von 0° bis 90°.
fn szenario() -> (Session, u64, u64) {
    let mut s = s();
    s.execute(
        "draw.circle",
        &json!({ "c": [1000.0, 1000.0], "r": 500.0 }),
    )
    .unwrap();
    s.execute(
        "draw.arc",
        &json!({ "c": [0.0, 0.0], "r": 400.0, "a0": 0.0, "a1": 90.0 }),
    )
    .unwrap();
    let kreis = s.doc.entities[0].id;
    let bogen = s.doc.entities[1].id;
    (s, kreis, bogen)
}

fn r_und_winkel(s: &Session, id: u64) -> (f64, f64, f64) {
    match s.doc.entity(id).expect("Element fehlt").kind {
        EntityKind::Arc { r, a0, a1, .. } => (r, a0.to_degrees(), a1.to_degrees()),
        ref other => panic!("kein Bogen: {other:?}"),
    }
}

/// Der Radius eines Kreises lässt sich setzen; Mittelpunkt bleibt.
#[test]
fn kreis_radius_ist_editierbar() {
    let (mut s, kreis, _) = szenario();
    s.execute("modify.set", &json!({ "ids": [kreis], "r": 1250.0 }))
        .unwrap();
    match s.doc.entity(kreis).unwrap().kind {
        EntityKind::Circle { c, r } => {
            assert_eq!(r, 1250.0, "Radius nicht gesetzt");
            assert_eq!((c.x, c.y), (1000.0, 1000.0), "Mittelpunkt verschoben");
        }
        ref other => panic!("kein Kreis: {other:?}"),
    }
}

/// Negative oder null Radius wird nicht übernommen (Untergrenze).
#[test]
fn kreis_radius_bleibt_positiv() {
    let (mut s, kreis, _) = szenario();
    s.execute("modify.set", &json!({ "ids": [kreis], "r": -300.0 }))
        .unwrap();
    match s.doc.entity(kreis).unwrap().kind {
        EntityKind::Circle { r, .. } => assert!(r > 0.0, "Radius {r} ist nicht positiv"),
        ref other => panic!("kein Kreis: {other:?}"),
    }
}

/// Radius eines Bogens ist editierbar — die Winkel bleiben stehen.
#[test]
fn bogen_radius_ist_editierbar() {
    let (mut s, _, bogen) = szenario();
    s.execute("modify.set", &json!({ "ids": [bogen], "r": 750.0 }))
        .unwrap();
    let (r, w0, w1) = r_und_winkel(&s, bogen);
    assert_eq!(r, 750.0);
    assert!((w0 - 0.0).abs() < 1e-9, "Startwinkel {w0}");
    assert!((w1 - 90.0).abs() < 1e-9, "Endwinkel {w1}");
}

/// Endwinkel eines Bogens sind editierbar (Grad in der Befehlsschicht).
#[test]
fn bogen_endwinkel_sind_editierbar() {
    let (mut s, _, bogen) = szenario();
    s.execute(
        "modify.set",
        &json!({ "ids": [bogen], "a0": 30.0, "a1": 200.0 }),
    )
    .unwrap();
    let (r, w0, w1) = r_und_winkel(&s, bogen);
    assert_eq!(r, 400.0, "Radius darf sich nicht ändern");
    assert!((w0 - 30.0).abs() < 1e-9, "Startwinkel {w0}");
    assert!((w1 - 200.0).abs() < 1e-9, "Endwinkel {w1}");
}

/// Gespeichert wird der neue Stand: die Datei trägt Radius und beide Winkel.
#[test]
fn geaenderte_werte_stehen_in_der_gespeicherten_datei() {
    let (mut s, _, bogen) = szenario();
    s.execute(
        "modify.set",
        &json!({ "ids": [bogen], "r": 620.0, "a0": 15.0, "a1": 120.0 }),
    )
    .unwrap();
    let text = persist::to_json(&s.doc).expect("JSON");
    let geladen = persist::from_json(&text).expect("laden");
    match &geladen.entity(bogen).expect("Bogen fehlt").kind {
        EntityKind::Arc { r, a0, a1, .. } => {
            assert!((*r - 620.0).abs() < 1e-9, "r = {r}");
            assert!((a0.to_degrees() - 15.0).abs() < 1e-6, "a0 = {a0}");
            assert!((a1.to_degrees() - 120.0).abs() < 1e-6, "a1 = {a1}");
        }
        other => panic!("kein Bogen: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Eigenschaften-Panel der GUI
// ---------------------------------------------------------------------------
//
// Der Panel-Abschnitt ist die zweite Hälfte der Aufgabe: ohne Feld gibt es
// nichts zu editieren. Geprüft wird der Quelltext des jeweiligen Typs in
// `single_props` — die Werte gehen über `changed_params` in `modify.set`,
// genau den Weg, den die Tests oben abdecken.

use std::path::PathBuf;

fn gui_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../jjcad-gui/src")
}

/// Nur die Codezeilen — Kommentare erklären dieselben Wörter und dürfen den
/// Test nicht auslösen.
fn ohne_kommentare(src: &str) -> String {
    src.lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Quelle des Eigenschaften-Panels. Seit dem Umbau liegt sie in `props.rs`:
/// **eine** Funktion je Objektart, die Erzeugen und Bearbeiten zeichnet.
fn panel_quelle() -> String {
    let src = std::fs::read_to_string(gui_dir().join("props.rs")).expect("props.rs");
    ohne_kommentare(&src)
}

#[test]
fn panel_bietet_radius_fuer_kreis_und_bogen() {
    let panel = panel_quelle();

    // Kreis: eigenes Feld „Radius“, das in modify.set landet.
    assert!(
        panel.contains("zeile(\"radius\")"),
        "Kreis-Panel ohne Feld \"Radius\""
    );
    assert!(
        panel.contains("tr(\"gui.props.radius\")"),
        "Kreis-Panel ohne Beschriftung \"Radius\" (gui.props.radius)"
    );
    assert!(
        panel.contains("length_field"),
        "Kreis-Radius ist kein Eingabefeld"
    );
    assert!(
        panel.contains("\"r\".into()"),
        "Kreis-Radius landet nicht in modify.set"
    );

    // Bogen: Radius und beide Endwinkel.
    // T0106: die Beschriftungen stehen im Katalog — der Code nennt den Schlüssel.
    for key in ["gui.props.radius", "gui.props.startwinkel", "gui.props.endwinkel"] {
        let muster = format!("tr(\"{key}\")");
        assert!(panel.contains(&muster), "Bogen-Panel ohne Feld {key}");
    }
    assert!(
        panel.contains("(\"startwinkel\", jjcad_core::i18n::tr(\"gui.props.startwinkel\")")
            && panel.contains("(\"endwinkel\", jjcad_core::i18n::tr(\"gui.props.endwinkel\")"),
        "Bogen-Panel meldet die Winkelzeilen nicht an"
    );
    assert!(
        panel.contains("\"a0\".into()") && panel.contains("\"a1\".into()"),
        "Bogen-Winkel landen nicht in modify.set"
    );
}

/// Der Radius steht genau einmal im Panel: die Anzeigezeile in „Geometrie“
/// ist entfallen, seit das Feld editierbar ist.
#[test]
fn radius_steht_nicht_doppelt_im_panel() {
    let panels = std::fs::read_to_string(gui_dir().join("panels.rs")).expect("panels.rs");
    let start = panels.find("fn geo_rows").expect("geo_rows");
    let rest = &panels[start..];
    let ende = rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len());
    let geo = ohne_kommentare(&rest[..ende]);
    assert!(
        !geo.contains("\"Radius\"") && !geo.contains("gui.props.radius"),
        "geo_rows zeigt Radius weiterhin an — doppelte Zeile"
    );
}
