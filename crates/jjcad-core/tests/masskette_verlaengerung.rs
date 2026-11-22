//! Verlängerung zu den Messpunkten (T0226): Maßhilfslinien von jedem
//! Messpunkt zur Maßlinie sind abschaltbar, Vorgabe **aus**.

use jjcad_core::actions;
use jjcad_core::geom::Vec2;
use jjcad_core::model::{Document, EntityKind};
use jjcad_core::persist;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut d = Document::new();
    d.settings.input_unit = Unit::Mm;
    d
}

fn kette(d: &mut Document) -> u64 {
    let r = actions::execute(
        d,
        "dim.chain",
        &json!({
            "pts": [[0.0, 0.0], [3000.0, 0.0]],
            "line_pos": [0.0, -400.0],
            "angle": 0.0,
            // Das Punktsymbol der Messpunkte (T0227) ist hier aus: der Test
            // zählt die Linien der Bemaßung selbst.
            "symbol": ""
        }),
    )
    .expect("Maßkette anlegen");
    r["created"][0].as_u64().expect("Kennung")
}

/// Zählt die gezeichneten Linien einer Maßkette.
fn linien(d: &Document) -> usize {
    render_document(&RenderCtx::new(d))
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Line(..)))
        .count()
}

fn ext_lines(d: &Document, id: u64) -> bool {
    match &d.entity(id).expect("Maßkette").kind {
        EntityKind::Dim { ext_lines, .. } => *ext_lines,
        k => panic!("keine Maßkette: {k:?}"),
    }
}

/// Vorgabe: **aus** — die Maßkette zeigt keine Maßhilfslinien, und das
/// gespeicherte Feld fehlt bzw. ist `false`.
#[test]
fn vorgabe_ist_aus() {
    let mut d = doc_mm();
    let ohne = linien(&d);
    let id = kette(&mut d);
    assert!(!ext_lines(&d, id), "Vorgabe muss aus sein");
    let mit = linien(&d);

    // Ohne Verlängerung entsteht nur die Maßlinie: die Marken der Enden
    // stellt das Punktsymbol, hier abgeschaltet (T0227, Turn 7).
    assert_eq!(
        mit - ohne,
        1,
        "Maßlinie, keine Maßhilfslinien (gezeichnet: {})",
        mit - ohne
    );

    let json = persist::to_json(&d).expect("Datei schreiben");
    assert!(
        !json.contains("\"ext_lines\": true"),
        "gespeicherte Datei darf die Verlängerung nicht einschalten"
    );
    let wieder = persist::from_json(&json).expect("Datei lesen");
    assert!(!ext_lines(&wieder, id), "nach dem Laden weiterhin aus");
}

/// Eingeschaltet zeichnet die Kette eine Maßhilfslinie je Messpunkt, und der
/// Schalter überlebt Speichern und Laden.
#[test]
fn eingeschaltet_zeichnet_hilfslinien() {
    let mut d = doc_mm();
    let ohne = linien(&d);
    let id = kette(&mut d);
    let aus = linien(&d);

    actions::execute(&mut d, "modify.set", &json!({"ids": [id], "ext_lines": true}))
        .expect("Verlängerung einschalten");
    assert!(ext_lines(&d, id), "Feld muss true sein");
    let an = linien(&d);
    assert_eq!(
        an - aus,
        2,
        "eine Maßhilfslinie je Messpunkt (gezeichnet: {})",
        an - aus
    );

    // Eine Maßhilfslinie je Messpunkt: senkrecht, vom Messpunkt über die
    // Maßlinie (y = -400) hinaus. Der Anfang liegt um den DIN-Abstand am
    // Maßpunkt versetzt unter dem Messpunkt.
    let hilfslinien: Vec<(Vec2, Vec2)> = render_document(&RenderCtx::new(&d))
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Line(a, b) => Some((*a, *b)),
            _ => None,
        })
        .filter(|(a, b)| (a.x - b.x).abs() < 1e-6 && (a.x == 0.0 || a.x == 3000.0))
        .collect();
    assert_eq!(hilfslinien.len(), 2, "zwei Maßhilfslinien erwartet");
    for (a, b) in &hilfslinien {
        assert!(a.y < 0.0 && a.y > -400.0, "Beginn unter dem Messpunkt: {a:?}");
        assert!(b.y < -400.0, "Ende über der Maßlinie: {b:?}");
    }
    let x: Vec<f64> = hilfslinien.iter().map(|(a, _)| a.x).collect();
    assert!(x.contains(&0.0) && x.contains(&3000.0), "Lage: {x:?}");

    let json = persist::to_json(&d).expect("Datei schreiben");
    assert!(json.contains("\"ext_lines\": true"), "Feld fehlt in der Datei");
    let wieder = persist::from_json(&json).expect("Datei lesen");
    assert!(ext_lines(&wieder, id), "nach dem Laden weiterhin an");
}

/// Die Action `dim.chain` nimmt den Schalter beim Anlegen mit — Vorgabe aus,
/// ausdrücklich `true` schaltet ihn ein.
#[test]
fn action_nimmt_den_schalter_mit() {
    let mut d = doc_mm();
    let r = actions::execute(
        &mut d,
        "dim.chain",
        &json!({
            "pts": [[0.0, 0.0], [3000.0, 0.0]],
            "line_pos": [0.0, -400.0],
            "angle": 0.0,
            "ext_lines": true
        }),
    )
    .expect("Maßkette anlegen");
    let id = r["created"][0].as_u64().expect("Kennung");
    assert!(ext_lines(&d, id), "ext_lines: true muss ankommen");

    let r = actions::execute(
        &mut d,
        "dim.linear",
        &json!({"a": [0.0, 0.0], "b": [1000.0, 0.0], "line_pos": [0.0, -200.0]}),
    )
    .expect("Einzelmaß anlegen");
    let id = r["created"][0].as_u64().expect("Kennung");
    assert!(!ext_lines(&d, id), "Vorgabe des Einzelmaßes ist aus");
}
