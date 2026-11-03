//! Flächen vereinigen und schneiden (T0090).
//!
//! Geprüft wird der Weg, den auch MCP und die Zeichenfläche nehmen:
//! `Session::execute` mit `modify.area.union` bzw. `modify.area.split`.
//! Die Geometrie selbst liegt in `flaeche` (Einheitstests dort); hier zählt,
//! dass die Aktionen die Objekte richtig ersetzen, Muster und Rand mitnehmen
//! und Fehlfälle als Fehler melden.

use jjcad_core::geom::{polygon_area, Vec2};
use jjcad_core::model::{Entity, EntityKind};
use jjcad_core::session::Session;
use jjcad_core::style::FillPattern;
use serde_json::json;

fn ring(e: &Entity) -> Vec<Vec2> {
    match &e.kind {
        EntityKind::Fill { boundary, .. } => boundary.clone(),
        other => panic!("keine Füllfläche, sondern {}", other.type_name()),
    }
}

fn flaeche_m2(e: &Entity) -> f64 {
    polygon_area(&ring(e)).abs() / 1_000_000.0
}

fn fills(s: &Session) -> Vec<&Entity> {
    s.doc
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Fill { .. }))
        .collect()
}

/// Zwei aneinanderliegende Flächen (4×3 m und 4×3 m) ergeben **eine** Fläche
/// mit 24 m² — die gemeinsame Kante verschwindet, die Ecken sind exakt.
#[test]
fn zwei_angrenzende_flaechen_werden_eine() {
    let mut s = Session::new();
    let a = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]], "pattern": "lines"}),
        )
        .expect("erste Fläche");
    let b = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[4.0, 0.0], [8.0, 0.0], [8.0, 3.0], [4.0, 3.0]], "pattern": "lines"}),
        )
        .expect("zweite Fläche");
    let ids: Vec<u64> = [a, b]
        .iter()
        .map(|v| v["created"][0].as_u64().unwrap())
        .collect();
    assert_eq!(fills(&s).len(), 2);

    let r = s
        .execute("modify.area.union", &json!({"ids": ids}))
        .expect("Vereinigen");
    assert_eq!(fills(&s).len(), 1, "genau eine Fläche");
    let e = fills(&s)[0];
    assert!((flaeche_m2(e) - 24.0).abs() < 1e-6, "{} m²", flaeche_m2(e));
    assert_eq!(ring(e).len(), 4, "vier Ecken: {:?}", ring(e));
    assert_eq!(r["deleted"].as_u64(), Some(2));
    // Das Muster der ersten Fläche bleibt.
    match &e.kind {
        EntityKind::Fill { pattern, .. } => assert_eq!(*pattern, FillPattern::Lines),
        _ => unreachable!(),
    }
}

/// Vereinigen braucht mindestens zwei Flächen — eine einzelne ist ein Fehler,
/// kein stilles „ok“.
#[test]
fn vereinigen_mit_einer_flaeche_ist_ein_fehler() {
    let mut s = Session::new();
    let a = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]}),
        )
        .unwrap();
    let id = a["created"][0].as_u64().unwrap();
    let err = s
        .execute("modify.area.union", &json!({"ids": [id]}))
        .unwrap_err();
    assert!(err.contains("zwei"), "{err}");
}

/// Getrennte Flächen lassen sich nicht vereinigen.
#[test]
fn getrennte_flaechen_werden_abgewiesen() {
    let mut s = Session::new();
    let a = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]}),
        )
        .unwrap();
    let b = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[10.0, 0.0], [12.0, 0.0], [12.0, 2.0], [10.0, 2.0]]}),
        )
        .unwrap();
    let ids = vec![
        a["created"][0].as_u64().unwrap(),
        b["created"][0].as_u64().unwrap(),
    ];
    let err = s
        .execute("modify.area.union", &json!({"ids": ids}))
        .unwrap_err();
    assert!(err.contains("angrenzend"), "{err}");
    assert_eq!(fills(&s).len(), 2, "nichts verändert");
}

/// Schneiden ersetzt die Fläche durch zwei Teile: je 6 m², Muster und
/// Randlinie bleiben, das Original ist weg.
#[test]
fn schneiden_teilt_eine_flaeche_in_zwei() {
    let mut s = Session::new();
    let a = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
                    "pattern": "cross", "border": true}),
        )
        .unwrap();
    let id = a["created"][0].as_u64().unwrap();

    let r = s
        .execute(
            "modify.area.split",
            &json!({"ids": [id], "pts": [[2.0, -1.0], [2.0, 4.0]]}),
        )
        .expect("Schneiden");
    assert_eq!(r["deleted"].as_u64(), Some(1));
    let teile = fills(&s);
    assert_eq!(teile.len(), 2, "zwei Teilflächen");
    for e in &teile {
        assert!(
            (flaeche_m2(e) - 6.0).abs() < 1e-6,
            "Teilfläche {} m²",
            flaeche_m2(e)
        );
        match &e.kind {
            EntityKind::Fill {
                pattern, border, ..
            } => {
                assert_eq!(*pattern, FillPattern::Cross, "Muster verloren");
                assert!(*border, "Randlinie verloren");
            }
            _ => unreachable!(),
        }
        assert!(
            ring(e).iter().all(|p| (p.x - 2000.0).abs() < 1e-6 || p.x >= 0.0),
            "Teilfläche liegt nicht im Original"
        );
    }
    // Beide Teile tragen denselben Stil — sonst meldet das Eigenschaften-Panel
    // „*variiert*“ für Werte, die gleich sein müssen.
    assert_eq!(
        format!("{:?}", teile[0].style),
        format!("{:?}", teile[1].style),
        "Stil der Teile weicht ab"
    );
    // Summe der Teile = Original.
    let summe: f64 = teile.iter().map(|e| flaeche_m2(e)).sum();
    assert!((summe - 12.0).abs() < 1e-6, "Summe {summe} m²");
}

/// Ohne Punktfolge kein Schnitt — ein fehlender Pflichtparameter ist ein
/// Fehler (kein stiller Erfolg).
#[test]
fn schneiden_ohne_punktfolge_ist_ein_fehler() {
    let mut s = Session::new();
    let a = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]}),
        )
        .unwrap();
    let id = a["created"][0].as_u64().unwrap();
    let err = s
        .execute("modify.area.split", &json!({"ids": [id]}))
        .unwrap_err();
    assert!(err.contains("pts"), "{err}");
}

/// Die gespeicherte Datei trägt nach dem Schnitt **zwei** Füllflächen — der
/// Nachweis liegt damit in der Datei, nicht nur im Bild.
#[test]
fn gespeicherte_datei_traegt_zwei_teilflaechen() {
    let mut s = Session::new();
    let a = s
        .execute(
            "draw.fill.points",
            &json!({"pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]}),
        )
        .unwrap();
    let id = a["created"][0].as_u64().unwrap();
    s.execute(
        "modify.area.split",
        &json!({"ids": [id], "pts": [[2.0, -1.0], [2.0, 4.0]]}),
    )
    .expect("Schneiden");

    let text = jjcad_core::persist::to_json(&s.doc).expect("schreiben");
    let wieder = jjcad_core::persist::from_json(&text).expect("lesen");
    let anzahl = wieder
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Fill { .. }))
        .count();
    assert_eq!(anzahl, 2, "zwei Füllflächen in der Datei");
    assert!(text.contains("\"fill\""), "Fill-Typ fehlt in der Datei");
}
