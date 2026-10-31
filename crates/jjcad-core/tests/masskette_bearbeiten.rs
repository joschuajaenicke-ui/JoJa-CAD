//! Maßketten bearbeiten: Basispunkte verschieben, Lage der Maßlinie setzen.
//!
//! Die Griffe der Zeichenfläche laufen über `handles` (GUI); hier steht der
//! Weg der Befehlsschicht (`modify.set`) — er trägt dieselben Werte und ist der
//! Prüfweg ohne Oberfläche (MCP, Panel).

use jjcad_core::geom::v;
use jjcad_core::model::{DimAxis, EntityKind};
use jjcad_core::render::{dim_line_span, dim_offset};
use serde_json::json;

fn kette(s: &mut jjcad_core::session::Session) -> u64 {
    let r = s
        .execute(
            "dim.chain",
            &json!({"pts": [[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]],
                    "line_pos": [1.0, -0.4], "axis": "x"}),
        )
        .expect("Maßkette");
    r["created"][0].as_u64().expect("id")
}

/// Ein Basispunkt lässt sich verschieben — die übrigen Punkte bleiben, die
/// Maßlinie behält ihre Lage (gespeichertes Feld).
#[test]
fn basispunkt_verschieben() {
    let mut s = jjcad_core::session::Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let id = kette(&mut s);
    s.execute("modify.set", &json!({"ids": [id], "pts": [[0.0, 0.0], [2.6, 0.3], [5.0, 0.0]]}))
        .expect("Basispunkt setzen");

    let e = s.doc.entity(id).expect("Maßkette");
    match &e.kind {
        EntityKind::DimChain { pts, line_pos, .. } => {
            assert_eq!(pts[1], v(2600.0, 300.0), "Basispunkt nicht gesetzt: {pts:?}");
            assert_eq!(pts[0], v(0.0, 0.0));
            assert_eq!(pts[2], v(5000.0, 0.0));
            assert_eq!(*line_pos, v(1000.0, -400.0), "Maßlinie wandert mit");
        }
        k => panic!("keine Maßkette mehr: {k:?}"),
    }

    // Eine Kette braucht mindestens zwei Punkte — sonst stiller Unsinn.
    let fehler = s.execute("modify.set", &json!({"ids": [id], "pts": [[0.0, 0.0]]}));
    assert!(fehler.is_err(), "einpunktige Kette muss ein Fehler sein");
}

/// Der Abstand der Maßlinie ist ein Feld: gesetzt wird der Versatz, nicht die
/// absolute Lage. Er trägt vorzeichenbehaftet (positiv = links der Messrichtung).
#[test]
fn abstand_der_masslinie_setzen() {
    let mut s = jjcad_core::session::Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let id = kette(&mut s);

    let e = s.doc.entity(id).expect("Maßkette");
    assert!(
        (dim_offset(&e.kind).expect("Abstand") + 400.0).abs() < 1e-6,
        "Ausgangsabstand: {:?}",
        dim_offset(&e.kind)
    );

    s.execute("modify.set", &json!({"ids": [id], "offset": 0.25}))
        .expect("Abstand setzen");
    let e = s.doc.entity(id).expect("Maßkette");
    assert!((dim_offset(&e.kind).unwrap() - 250.0).abs() < 1e-6);

    // Die gezeichnete Maßlinie folgt derselben Rechnung.
    let (p0, p1) = dim_line_span(&e.kind).expect("Maßlinie");
    assert!((p0.y - 250.0).abs() < 1e-6 && (p1.y - 250.0).abs() < 1e-6, "{p0:?} {p1:?}");
    assert!((p0.x - 0.0).abs() < 1e-6 && (p1.x - 5000.0).abs() < 1e-6, "{p0:?} {p1:?}");
}

/// Nach dem Verschieben eines Basispunkts liegt der Abstand weiter auf der
/// (alten) Höhe der Maßlinie — der Abstand misst gegen den **neuen** Punkt.
#[test]
fn abstand_rechnet_gegen_den_neuen_basispunkt() {
    let mut s = jjcad_core::session::Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let id = kette(&mut s);
    s.execute("modify.set", &json!({"ids": [id], "pts": [[0.0, 0.1], [2.0, 0.0], [5.0, 0.0]]}))
        .expect("Basispunkt");

    let e = s.doc.entity(id).expect("Maßkette");
    let off = dim_offset(&e.kind).expect("Abstand");
    assert!((off + 500.0).abs() < 1e-6, "Abstand gegen neuen Punkt: {off}");
}

/// Ein Einzelmaß trägt dieselben Felder: a, b, Abstand.
#[test]
fn einzelmass_basispunkte_und_abstand() {
    let mut s = jjcad_core::session::Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let r = s
        .execute(
            "dim.linear",
            &json!({"a": [0.0, 0.0], "b": [3.0, 0.0], "line_pos": [1.5, -0.3]}),
        )
        .expect("Maß");
    let id = r["created"][0].as_u64().expect("id");

    s.execute("modify.set", &json!({"ids": [id], "a": [-0.5, 0.0], "b": [3.5, 0.0]}))
        .expect("Basispunkte");
    s.execute("modify.set", &json!({"ids": [id], "offset": 0.6}))
        .expect("Abstand");

    let e = s.doc.entity(id).expect("Maß");
    match &e.kind {
        EntityKind::Dim { a, b, line_pos, axis, .. } => {
            assert_eq!(*a, v(-500.0, 0.0));
            assert_eq!(*b, v(3500.0, 0.0));
            assert_eq!(*axis, DimAxis::X);
            // Nur die Höhe zählt, die Lage entlang der Maßlinie bleibt.
            assert_eq!(*line_pos, v(1500.0, 600.0), "Maßlinie auf dem Abstand");
        }
        k => panic!("kein Maß mehr: {k:?}"),
    }
}

/// Bericht aus Turn 1: „die Maßlinien rotieren beim Ändern". Die Maßlinie
/// eines waagerechten Maßes liegt auf **einer** Höhe — wandert ein Basispunkt
/// seitlich, darf sie weder kippen noch die Höhe wechseln.
#[test]
fn waagerechtes_mass_kippt_nicht() {
    let mut s = jjcad_core::session::Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let r = s
        .execute(
            "dim.chain",
            &json!({"pts": [[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]],
                    "line_pos": [1.0, -0.4], "axis": "x"}),
        )
        .expect("Maßkette");
    let id = r["created"][0].as_u64().expect("id");

    // Ersten Basispunkt um 0,3 m nach oben ziehen.
    s.execute(
        "modify.set",
        &json!({"ids": [id], "pts": [[0.0, 0.3], [2.0, 0.0], [5.0, 0.0]]}),
    )
    .expect("Basispunkt");

    let e = s.doc.entity(id).expect("Maßkette");
    let (p0, p1) = dim_line_span(&e.kind).expect("Maßlinie");
    assert!((p0.y + 400.0).abs() < 1e-6 && (p1.y + 400.0).abs() < 1e-6, "Höhe: {p0:?} {p1:?}");
    assert!((p0.x).abs() < 1e-6 && (p1.x - 5000.0).abs() < 1e-6, "{p0:?} {p1:?}");
}

/// Dasselbe für das Einzelmaß — die obere Bemaßung aus `samples/Wohnhaus.jjcad`
/// kippte nach dem Ziehen eines Basispunkts (gemessen: (0|9400)→(10500|8300)).
#[test]
fn einzelmass_kippt_nicht() {
    let mut s = jjcad_core::session::Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let r = s
        .execute(
            "dim.linear",
            &json!({"a": [0.0, 8.0], "b": [10.5, 8.0], "line_pos": [5.0, 9.4]}),
        )
        .expect("Maß");
    let id = r["created"][0].as_u64().expect("id");
    s.execute("modify.set", &json!({"ids": [id], "a": [0.0, 9.1]}))
        .expect("Basispunkt");

    let e = s.doc.entity(id).expect("Maß");
    let (p0, p1) = dim_line_span(&e.kind).expect("Maßlinie");
    assert!((p0.y - 9400.0).abs() < 1e-6 && (p1.y - 9400.0).abs() < 1e-6, "{p0:?} {p1:?}");
    assert!((p0.x - 0.0).abs() < 1e-6 && (p1.x - 10500.0).abs() < 1e-6, "{p0:?} {p1:?}");
    let winkel = (p1 - p0).angle().abs();
    assert!(winkel < 1e-9, "Maßlinie steht schief: {winkel}");
}

/// Senkrechtes Maß: der Abstand zählt nach rechts, die Maßlinie bleibt auf
/// ihrer Breite stehen.
#[test]
fn senkrechtes_mass_bleibt_auf_der_breite() {
    let mut s = jjcad_core::session::Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let r = s
        .execute(
            "dim.chain",
            &json!({"pts": [[0.0, 0.0], [0.0, 2.0], [0.0, 5.0]],
                    "line_pos": [-0.4, 0.0], "axis": "y"}),
        )
        .expect("Maßkette");
    let id = r["created"][0].as_u64().expect("id");
    let e = s.doc.entity(id).expect("Maßkette");
    assert!(
        (dim_offset(&e.kind).unwrap() + 400.0).abs() < 1e-6,
        "Abstand: {:?}",
        dim_offset(&e.kind)
    );

    s.execute(
        "modify.set",
        &json!({"ids": [id], "pts": [[0.3, 0.0], [0.0, 2.0], [0.0, 5.0]]}),
    )
    .expect("Basispunkt");
    let e = s.doc.entity(id).expect("Maßkette");
    let (p0, p1) = dim_line_span(&e.kind).expect("Maßlinie");
    assert!((p0.x + 400.0).abs() < 1e-6 && (p1.x + 400.0).abs() < 1e-6, "{p0:?} {p1:?}");

    // Abstand zählt vom ersten Basispunkt: 0,6 m rechts von x = 0,3 m.
    s.execute("modify.set", &json!({"ids": [id], "offset": 0.6}))
        .expect("Abstand");
    let e = s.doc.entity(id).expect("Maßkette");
    let (p0, _) = dim_line_span(&e.kind).expect("Maßlinie");
    assert!((p0.x - 900.0).abs() < 1e-6, "{p0:?}");
}
