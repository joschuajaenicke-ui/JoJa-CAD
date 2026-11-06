//! Ellipse stutzen (T0084).
//!
//! Aus einer gestutzten Ellipse entsteht ein **Ellipsenbogen** — das neue
//! Objekt, das einen Abschnitt einer Ellipse darstellt (Gegenstück zum Bogen
//! beim Stutzen eines Kreises). Geprüft wird: die Schnittpunkte liegen auf der
//! Kurve, der angeklickte Abschnitt fällt weg, der Rest bleibt als ein oder
//! mehrere Bögen stehen, ein Bogen lässt sich erneut stutzen, die Vorschau zeigt
//! genau den wegfälligen Abschnitt und die Datei trägt den neuen Objekttyp.

use jjcad_core::geom::{v, Ellipse, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

/// Ellipse mit Mittelpunkt im Ursprung, `rx = 2000 mm`, `ry = 1000 mm`.
fn ellipse(s: &mut Session) -> Id {
    let antwort = s
        .execute("draw.ellipse", &json!({"c": [0, 0], "rx": 2, "ry": 1}))
        .expect("Ellipse zeichnen");
    antwort["created"][0].as_u64().expect("Id der Ellipse")
}

fn bogen_parameter(s: &Session, id: Id) -> Option<(f64, f64)> {
    match &s.doc.entity(id)?.kind {
        EntityKind::EllipseArc { a0, a1, .. } => Some((*a0, *a1)),
        _ => None,
    }
}

/// Punkt der Ellipse am Parameter `u` (rx = 2000, ry = 1000, keine Drehung).
fn punkt(u: f64) -> Vec2 {
    Ellipse::new(Vec2::ZERO, 2000.0, 1000.0, 0.0).point_at(u)
}

#[test]
fn schnittpunkte_liegen_auf_der_ellipse() {
    let el = Ellipse::new(Vec2::ZERO, 2000.0, 1000.0, 0.0);
    // Senkrechte Gerade durch den Mittelpunkt: Schnitt bei 90° und 270°.
    let treffer = el.line_hits(v(0.0, -5000.0), v(0.0, 5000.0));
    assert_eq!(treffer.len(), 2, "{treffer:?}");
    // `line_hits` liefert den Parameter als atan2-Wert (−π…π).
    let mut winkel: Vec<f64> =
        treffer.iter().map(|(u, _)| jjcad_core::geom::norm_angle(*u)).collect();
    winkel.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((winkel[0] - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "{winkel:?}");
    assert!((winkel[1] - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9, "{winkel:?}");
    // Die Punkte liegen exakt auf der Kurve.
    for (_, p) in &treffer {
        assert!((p.x * p.x / (2000.0 * 2000.0) + p.y * p.y / (1000.0 * 1000.0) - 1.0).abs() < 1e-9);
    }
    // Der Parameter eines Kurvenpunkts ist der Wert, mit dem er entstand.
    let q = el.point_at(1.234);
    assert!((el.param_of(q) - 1.234).abs() < 1e-9);
}

#[test]
fn ellipse_wird_zu_einem_ellipsenbogen_gestutzt() {
    let mut s = s();
    let id = ellipse(&mut s);
    // Eine Schnittkante durch den Mittelpunkt teilt die Ellipse in zwei Hälften.
    s.execute("draw.line", &json!({"a": [0, -5], "b": [0, 5]}))
        .unwrap();

    // Klick auf den oberen linken Viertelpunkt (Parameter 135°).
    let klick = punkt(3.0 * std::f64::consts::FRAC_PI_4);
    s.execute(
        "modify.trim",
        &json!({"ids": [id], "at": [klick.x / 1000.0, klick.y / 1000.0]}),
    )
    .expect("Ellipse stutzen");

    let (a0, a1) = bogen_parameter(&s, id).expect("aus der Ellipse wird ein Ellipsenbogen");
    // Übrig bleibt die rechte Hälfte: 270° → 90° gegen den Uhrzeigersinn.
    assert!((a0 - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9, "a0 = {a0}");
    assert!((a1 - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "a1 = {a1}");
    // Die Mitte des Bogens ist der rechte Scheitel.
    let mitte = punkt(a0 + Ellipse::new(Vec2::ZERO, 2000.0, 1000.0, 0.0).sweep(a0, a1) * 0.5);
    assert!((mitte.x - 2000.0).abs() < 1e-6 && mitte.y.abs() < 1e-6, "{mitte:?}");
    // Genau ein Objekt — die Ellipse wurde umgewandelt, nicht dupliziert.
    assert_eq!(s.doc.entities.len(), 2, "Ellipse + Schnittkante");
}

/// Vier Schnittpunkte, ein Klick: nur die angeklickte Stelle fällt weg, der
/// Rest bleibt **ein** Ellipsenbogen. Vorher zerfiel die Ellipse an jeder
/// Schnittstelle in drei Einzelbögen (T0118).
#[test]
fn zwei_schnittkanten_zerlegen_die_ellipse_nicht() {
    let mut s = s();
    let id = ellipse(&mut s);
    // Zwei senkrechte Schnittkanten bei x = ±1000 mm.
    for x in [-1.0, 1.0] {
        s.execute("draw.line", &json!({"a": [x, -5], "b": [x, 5]}))
            .unwrap();
    }
    // Klick in den oberen Abschnitt zwischen den beiden Kanten (Parameter 90°).
    let klick = punkt(std::f64::consts::FRAC_PI_2);
    s.execute(
        "modify.trim",
        &json!({"ids": [id], "at": [klick.x / 1000.0, klick.y / 1000.0]}),
    )
    .expect("Ellipse stutzen");

    let boegen: Vec<(Id, f64, f64)> = s
        .doc
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::EllipseArc { .. }))
        .filter_map(|e| match &e.kind {
            EntityKind::EllipseArc { a0, a1, .. } => Some((e.id, *a0, *a1)),
            _ => None,
        })
        .collect();
    assert_eq!(boegen.len(), 1, "ein Ellipsenbogen bleibt: {boegen:?}");
    // Der Bogen liegt auf derselben Ellipse (Mittelpunkt, Achsen unverändert).
    for e in s.doc.entities.iter().filter(|e| matches!(e.kind, EntityKind::EllipseArc { .. })) {
        if let EntityKind::EllipseArc { c, rx, ry, .. } = &e.kind {
            assert!(c.x.abs() < 1e-9 && c.y.abs() < 1e-9);
            assert!((*rx - 2000.0).abs() < 1e-9 && (*ry - 1000.0).abs() < 1e-9);
        }
    }
    let (_, a0, a1) = boegen[0];
    assert!((a0 - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-9, "a0 = {a0}");
    assert!((a1 - std::f64::consts::FRAC_PI_3).abs() < 1e-9, "a1 = {a1}");
    // Der obere Abschnitt fehlt: der Bogen deckt den Parameter 90° nicht.
    assert!(
        !jjcad_core::geom::angle_in_arc(std::f64::consts::FRAC_PI_2, a0, a1),
        "der angeklickte Abschnitt ist weg: {boegen:?}"
    );
    // Restsumme: die Ellipse ohne den oberen Sechstel-Abschnitt (60°…120°).
    let el = Ellipse::new(Vec2::ZERO, 2000.0, 1000.0, 0.0);
    let rest = el.sweep(a0, a1);
    let erwartet = std::f64::consts::TAU - std::f64::consts::TAU / 6.0;
    assert!((rest - erwartet).abs() < 1e-6, "Rest {rest}, erwartet {erwartet}");
}

#[test]
fn ellipsenbogen_laesst_sich_erneut_stutzen() {
    let mut s = s();
    let id = ellipse(&mut s);
    s.execute("draw.line", &json!({"a": [0, -5], "b": [0, 5]}))
        .unwrap();
    let klick = punkt(3.0 * std::f64::consts::FRAC_PI_4);
    s.execute(
        "modify.trim",
        &json!({"ids": [id], "at": [klick.x / 1000.0, klick.y / 1000.0]}),
    )
    .expect("Ellipse stutzen");

    // Waagerechte Schnittkante bei y = 500 mm: Parameter 30° und 150°.
    s.execute("draw.line", &json!({"a": [-5, 0.5], "b": [5, 0.5]}))
        .unwrap();
    let klick2 = punkt(std::f64::consts::FRAC_PI_3); // 60° — oberhalb der Kante
    s.execute(
        "modify.trim",
        &json!({"ids": [id], "at": [klick2.x / 1000.0, klick2.y / 1000.0]}),
    )
    .expect("Ellipsenbogen stutzen");

    let (a0, a1) = bogen_parameter(&s, id).expect("Bogen bleibt Bogen");
    assert!(
        (a0 - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "Anfang bleibt am unteren Scheitel: {a0}"
    );
    assert!(
        (a1 - std::f64::consts::FRAC_PI_6).abs() < 1e-9,
        "Ende liegt am Schnitt (30°): {a1}"
    );
}

#[test]
fn vorschau_zeigt_den_abschnitt_der_wegfaellt() {
    let mut s = s();
    let id = ellipse(&mut s);
    s.execute("draw.line", &json!({"a": [0, -5], "b": [0, 5]}))
        .unwrap();
    let anzahl = s.doc.entities.len();

    let klick = punkt(3.0 * std::f64::consts::FRAC_PI_4);
    let segs = jjcad_core::actions::trim_vorschau(&s.doc, id, klick, &[]);
    assert!(!segs.is_empty(), "Vorschau des Abschnitts");
    assert_eq!(s.doc.entities.len(), anzahl, "Vorschau legt nichts an");

    // Jede Sehne liegt auf der Ellipse und im **linken** Abschnitt.
    let el = Ellipse::new(Vec2::ZERO, 2000.0, 1000.0, 0.0);
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    for sg in &segs {
        for q in sg.flatten(0.0) {
            let norm = q.x * q.x / (2000.0 * 2000.0) + q.y * q.y / (1000.0 * 1000.0);
            assert!((norm - 1.0).abs() < 1e-3, "Punkt neben der Kurve: {q:?}");
            x_min = x_min.min(q.x);
            x_max = x_max.max(q.x);
        }
    }
    assert!(x_max < 1e-6 && (x_min + 2000.0).abs() < 30.0, "{x_min} … {x_max}");
    let _ = el;
}

#[test]
fn ellipsenbogen_ueberlebt_die_datei() {
    let mut s = s();
    let id = ellipse(&mut s);
    s.execute("draw.line", &json!({"a": [0, -5], "b": [0, 5]}))
        .unwrap();
    let klick = punkt(3.0 * std::f64::consts::FRAC_PI_4);
    s.execute(
        "modify.trim",
        &json!({"ids": [id], "at": [klick.x / 1000.0, klick.y / 1000.0]}),
    )
    .expect("Ellipse stutzen");

    let text = jjcad_core::persist::to_json(&s.doc).expect("Zeichnung schreiben");
    assert!(text.contains("ellipse_arc"), "Objekttyp im Format: {text}");
    let wieder = jjcad_core::persist::from_json(&text).expect("Zeichnung laden");
    let (a0, a1) = match &wieder.entity(id).expect("Objekt nach dem Laden").kind {
        EntityKind::EllipseArc { a0, a1, .. } => (*a0, *a1),
        k => panic!("Ellipsenbogen erwartet, war {k:?}"),
    };
    assert!((a0 - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert!((a1 - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn trefferflaeche_des_bogens_liegt_auf_der_kurve() {
    let mut s = s();
    let id = ellipse(&mut s);
    s.execute("draw.line", &json!({"a": [0, -5], "b": [0, 5]}))
        .unwrap();
    let klick = punkt(3.0 * std::f64::consts::FRAC_PI_4);
    s.execute(
        "modify.trim",
        &json!({"ids": [id], "at": [klick.x / 1000.0, klick.y / 1000.0]}),
    )
    .unwrap();

    let e = s.doc.entity(id).expect("Bogen");
    let segs = jjcad_core::render::entity_segments(&s.doc, e);
    assert!(!segs.is_empty());
    for sg in &segs {
        for q in sg.flatten(0.0) {
            let norm = q.x * q.x / (2000.0 * 2000.0) + q.y * q.y / (1000.0 * 1000.0);
            assert!((norm - 1.0).abs() < 1e-3, "Trefferfläche neben der Kurve: {q:?}");
            assert!(q.x >= -1e-6, "nur die rechte Hälfte: {q:?}");
        }
    }
    // Der Klick auf den Bogen trifft ihn auch.
    let treffer = jjcad_core::snap::hit_test(&s.doc, punkt(0.2), 20.0, None);
    assert_eq!(treffer, Some(id), "Bogen ist anklickbar");
}