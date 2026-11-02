//! Kreis und Bogen stutzen (T0084, Turn 2).
//!
//! Der Kreis nahm beim Stutzen nur **Geraden** als Schnittkanten wahr: ein
//! Kreis, der von einem anderen Kreis oder einem Bogen geschnitten wurde, ließ
//! sich nicht teilen, obwohl die Schnittpunkte sichtbar auf der Kurve lagen.
//! Geprüft wird: Kreis × Kreis, Kreis × Bogen, mehrere Kanten ergeben mehrere
//! Bögen, ein Bogen lässt sich selbst stutzen, die Vorschau zeigt genau den
//! wegfälligen Abschnitt und ohne Schnitt gibt es einen Fehler statt eines
//! stillen Erfolgs.

use jjcad_core::geom::{v, Seg, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

fn kreis(s: &mut Session, c: [f64; 2], r: f64) -> Id {
    let antwort = s
        .execute("draw.circle", &json!({"c": c, "r": r}))
        .expect("Kreis zeichnen");
    antwort["created"][0].as_u64().expect("Id des Kreises")
}

fn bogen_parameter(s: &Session, id: Id) -> Option<(f64, f64)> {
    match &s.doc.entity(id)?.kind {
        EntityKind::Arc { a0, a1, .. } => Some((*a0, *a1)),
        _ => None,
    }
}

fn nahe(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Zwei gleich große Kreise (r = 2000 mm) mit Mittelpunkten 2000 mm auseinander
/// schneiden sich bei x = 1000 mm, y = ±1732 mm — Winkel 60° und 300°.
#[test]
fn kreis_wird_von_einem_kreis_geschnitten() {
    let mut s = s();
    let a = kreis(&mut s, [0.0, 0.0], 2.0);
    kreis(&mut s, [2.0, 0.0], 2.0);

    // Klick auf den rechten Scheitel: die kleine Kappe rechts fällt weg.
    s.execute("modify.trim", &json!({"ids": [a], "at": [2.0, 0.0]}))
        .expect("Kreis stutzen");

    let (a0, a1) = bogen_parameter(&s, a).expect("aus dem Kreis wird ein Bogen");
    assert!(nahe(a0, std::f64::consts::FRAC_PI_3), "a0 = {a0}");
    assert!(nahe(a1, 5.0 * std::f64::consts::FRAC_PI_3), "a1 = {a1}");
    assert_eq!(s.doc.entities.len(), 2, "Kreis + Schnittkreis");
}

#[test]
fn kreis_wird_von_einem_bogen_geschnitten() {
    let mut s = s();
    let a = kreis(&mut s, [0.0, 0.0], 2.0);
    // Linke Hälfte eines zweiten Kreises — ein Bogen, kein Vollkreis.
    s.execute(
        "draw.arc",
        &json!({"c": [2.0, 0.0], "r": 2.0, "a0": 90, "a1": 270}),
    )
    .expect("Bogen zeichnen");

    s.execute("modify.trim", &json!({"ids": [a], "at": [2.0, 0.0]}))
        .expect("Kreis am Bogen stutzen");

    let (a0, a1) = bogen_parameter(&s, a).expect("Bogen bleibt Bogen");
    assert!(nahe(a0, std::f64::consts::FRAC_PI_3), "a0 = {a0}");
    assert!(nahe(a1, 5.0 * std::f64::consts::FRAC_PI_3), "a1 = {a1}");
}

#[test]
fn zwei_kanten_ergeben_drei_boegen() {
    let mut s = s();
    let a = kreis(&mut s, [0.0, 0.0], 2.0);
    for x in [-1.0, 1.0] {
        s.execute("draw.line", &json!({"a": [x, -5], "b": [x, 5]}))
            .unwrap();
    }
    // Klick oben (90°) — der Abschnitt zwischen den beiden Kanten fällt weg.
    s.execute("modify.trim", &json!({"ids": [a], "at": [0.0, 2.0]}))
        .expect("Kreis stutzen");

    let mut boegen: Vec<(f64, f64)> = s
        .doc
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Arc { a0, a1, .. } => Some((*a0, *a1)),
            _ => None,
        })
        .collect();
    assert_eq!(boegen.len(), 3, "vier Abschnitte, einer weg: {boegen:?}");
    // Der obere Abschnitt (60°…120°) fehlt.
    assert!(
        boegen
            .iter()
            .all(|(a0, a1)| !jjcad_core::geom::angle_in_arc(std::f64::consts::FRAC_PI_2, *a0, *a1)),
        "angeklickter Abschnitt ist weg: {boegen:?}"
    );
    // Restsumme: der Kreis ohne den 60°-Abschnitt.
    let rest: f64 = boegen
        .iter()
        .map(|(a0, a1)| jjcad_core::geom::Arc::new(Vec2::ZERO, 2000.0, *a0, *a1).sweep())
        .sum();
    let erwartet = std::f64::consts::TAU - std::f64::consts::FRAC_PI_3;
    assert!((rest - erwartet).abs() < 1e-6, "Rest {rest}, erwartet {erwartet}");
    boegen.clear();
}

#[test]
fn bogen_laesst_sich_selbst_stutzen() {
    let mut s = s();
    let antwort = s
        .execute(
            "draw.arc",
            &json!({"c": [0.0, 0.0], "r": 2.0, "a0": 0, "a1": 180}),
        )
        .expect("Bogen zeichnen");
    let id = antwort["created"][0].as_u64().expect("Id");
    // Senkrechte durch den Mittelpunkt: Schnitt bei 90° (270° liegt nicht auf
    // dem Bogen und zählt nicht).
    s.execute("draw.line", &json!({"a": [0, -5], "b": [0, 5]}))
        .unwrap();

    // Klick bei 45° — der untere Teil des Bogens (0°…90°) fällt weg.
    let klick = v(2000.0 * 0.5f64.sqrt(), 2000.0 * 0.5f64.sqrt());
    s.execute(
        "modify.trim",
        &json!({"ids": [id], "at": [klick.x / 1000.0, klick.y / 1000.0]}),
    )
    .expect("Bogen stutzen");

    let (a0, a1) = bogen_parameter(&s, id).expect("Bogen bleibt Bogen");
    assert!(nahe(a0, std::f64::consts::FRAC_PI_2), "a0 = {a0}");
    assert!(nahe(a1, std::f64::consts::PI), "a1 = {a1}");
}

#[test]
fn vorschau_zeigt_den_wegfallenden_bogen() {
    let mut s = s();
    let a = kreis(&mut s, [0.0, 0.0], 2.0);
    kreis(&mut s, [2.0, 0.0], 2.0);
    let anzahl = s.doc.entities.len();

    let segs = jjcad_core::actions::trim_vorschau(&s.doc, a, v(2000.0, 0.0), &[]);
    assert_eq!(segs.len(), 1, "ein Abschnitt: {segs:?}");
    let Seg::Arc(arc) = &segs[0] else {
        panic!("Bogen erwartet: {:?}", segs[0]);
    };
    assert!(nahe(arc.a0, 5.0 * std::f64::consts::FRAC_PI_3), "a0 = {}", arc.a0);
    assert!(nahe(arc.a1, std::f64::consts::FRAC_PI_3), "a1 = {}", arc.a1);
    assert_eq!(s.doc.entities.len(), anzahl, "Vorschau legt nichts an");
}

#[test]
fn ohne_schnitt_kein_stiller_erfolg() {
    let mut s = s();
    let a = kreis(&mut s, [0.0, 0.0], 2.0);
    // Weit entfernt: kein Schnittpunkt.
    kreis(&mut s, [10.0, 0.0], 1.0);
    let fehler = s.execute("modify.trim", &json!({"ids": [a], "at": [2.0, 0.0]}));
    assert!(fehler.is_err(), "ohne Schnittpunkte muss Stutzen melden: {fehler:?}");
    // Der Kreis bleibt unverändert.
    assert!(matches!(
        s.doc.entity(a).map(|e| &e.kind),
        Some(EntityKind::Circle { .. })
    ));
}