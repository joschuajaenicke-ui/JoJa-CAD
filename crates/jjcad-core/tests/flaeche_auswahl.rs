//! Fensterauswahl von Flächen (T0099).
//!
//! Der grüne Querkasten (Zug von rechts nach links) darf eine Fläche nur
//! wählen, wenn er sie **wirklich berührt**. Vorher entschied die
//! achsenparallele Bounding-Box: bei einer L-förmigen (konkaven) Fläche deckt
//! sie auch die Aussparung ab, und ein Kasten in der Aussparung wählte die
//! Fläche mit, obwohl dort nichts gezeichnet ist.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::snap::select_window;
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

/// L-förmige Fläche in Metern: 6 × 4 m, ausgespart bleibt rechts unten
/// (x 3…6, y 0…2). Die Bounding-Box deckt die Aussparung mit ab.
fn l_flaeche(s: &mut Session) -> Id {
    let r = s
        .execute(
            "draw.fill",
            &json!({
                "boundary": [
                    [0.0, 0.0], [3.0, 0.0], [3.0, 2.0],
                    [6.0, 2.0], [6.0, 4.0], [0.0, 4.0]
                ]
            }),
        )
        .unwrap();
    r["created"][0].as_u64().expect("Fläche angelegt") as Id
}

#[test]
fn querkasten_in_der_aussparung_waehlt_die_konkave_flaeche_nicht() {
    let mut s = s();
    let id = l_flaeche(&mut s);
    // Kasten ganz in der Aussparung — berührt keine Kante, keine Ecke.
    let ids = select_window(&s.doc, v(4000.0, 300.0), v(5500.0, 1500.0), true);
    assert!(
        !ids.contains(&id),
        "Kasten in der Aussparung darf die Fläche nicht wählen: {ids:?}"
    );
}

#[test]
fn querkasten_ueber_der_konkaven_flaeche_waehlt_sie() {
    let mut s = s();
    let id = l_flaeche(&mut s);
    // Kasten über der Ecke des L (ragt in die Fläche).
    let ids = select_window(&s.doc, v(2500.0, 1500.0), v(3500.0, 2500.0), true);
    assert_eq!(ids, vec![id], "Kasten über der Fläche muss sie wählen");
    // Kasten ganz innen (Fläche deckt ihn ab).
    let ids = select_window(&s.doc, v(500.0, 2500.0), v(2500.0, 3500.0), true);
    assert_eq!(ids, vec![id], "Kasten in der Fläche muss sie wählen");
    // Kasten schneidet nur eine Kante, keine Ecke liegt innen.
    let ids = select_window(&s.doc, v(2900.0, 900.0), v(3100.0, 1100.0), true);
    assert_eq!(ids, vec![id], "Kante geschnitten: Fläche ist berührt");
}

#[test]
fn fenster_ueber_der_aussparung_waehlt_die_konkave_flaeche_nicht() {
    let mut s = s();
    let id = l_flaeche(&mut s);
    // Blaues Fenster (links→rechts) verlangt volle Aufnahme; die Fläche ragt
    // heraus. Auch hier darf die Box nicht genügen.
    let ids = select_window(&s.doc, v(3500.0, 500.0), v(5500.0, 1500.0), false);
    assert!(!ids.contains(&id), "Fenster in der Aussparung: {ids:?}");
    // Fenster um die ganze Fläche nimmt sie auf.
    let ids = select_window(&s.doc, v(-500.0, -500.0), v(6500.0, 4500.0), false);
    assert_eq!(ids, vec![id], "ganze Fläche im Fenster");
}

#[test]
fn querkasten_in_der_insel_waehlt_die_flaeche_nicht() {
    let mut s = s();
    let r = s
        .execute(
            "draw.fill",
            &json!({
                "boundary": [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]],
                "holes": [[[1.5, 1.5], [3.0, 1.5], [3.0, 2.5], [1.5, 2.5]]]
            }),
        )
        .unwrap();
    let id = r["created"][0].as_u64().unwrap() as Id;
    // Kasten ganz in der Insel: dort ist nichts gefüllt.
    let ids = select_window(&s.doc, v(1800.0, 1800.0), v(2700.0, 2200.0), true);
    assert!(!ids.contains(&id), "Kasten in der Insel: {ids:?}");
    // Kasten über die Inselkante hinaus: berührt die Fläche.
    let ids = select_window(&s.doc, v(2900.0, 2000.0), v(3500.0, 2300.0), true);
    assert_eq!(ids, vec![id], "Inselkante geschnitten");
}

#[test]
fn querkasten_neben_einer_geraden_flaeche_waehlt_sie_nicht() {
    let mut s = s();
    let r = s
        .execute(
            "draw.fill",
            &json!({"boundary": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]}),
        )
        .unwrap();
    let id = r["created"][0].as_u64().unwrap() as Id;
    // Innerhalb der Bounding-Box, aber neben der Fläche gibt es keine.
    let ids = select_window(&s.doc, v(-1000.0, 1000.0), v(-200.0, 2000.0), true);
    assert!(ids.is_empty(), "neben der Fläche: {ids:?}");
    let ids = select_window(&s.doc, v(1000.0, 1000.0), v(2000.0, 2000.0), true);
    assert_eq!(vec![id], ids, "in der Fläche");
}

#[test]
fn konkaves_volumen_wird_wie_eine_flaeche_geprueft() {
    let mut s = s();
    let r = s
        .execute(
            "draw.volume",
            &json!({
                "boundary": [
                    [0.0, 0.0], [3.0, 0.0], [3.0, 2.0],
                    [6.0, 2.0], [6.0, 4.0], [0.0, 4.0]
                ],
                "height": 2500.0
            }),
        )
        .unwrap();
    let id = r["created"][0].as_u64().unwrap() as Id;
    assert!(matches!(
        &s.doc.entity(id).map(|e| &e.kind),
        Some(EntityKind::Volume { .. })
    ));
    let ids = select_window(&s.doc, v(4000.0, 300.0), v(5500.0, 1500.0), true);
    assert!(!ids.contains(&id), "Volumen in der Aussparung: {ids:?}");
    let ids = select_window(&s.doc, v(500.0, 2500.0), v(2500.0, 3500.0), true);
    assert_eq!(ids, vec![id], "Volumen berührt");
}

#[test]
fn punktprobe_der_flaeche_bleibt_unveraendert() {
    // Gegenprobe zur Geometrie: die Aussparung ist außerhalb, der Schenkel innen.
    let ecken: Vec<Vec2> = vec![
        v(0.0, 0.0),
        v(3000.0, 0.0),
        v(3000.0, 2000.0),
        v(6000.0, 2000.0),
        v(6000.0, 4000.0),
        v(0.0, 4000.0),
    ];
    assert!(jjcad_core::area::point_in_area(v(1000.0, 1000.0), &ecken, &[]));
    assert!(!jjcad_core::area::point_in_area(
        v(4500.0, 1000.0),
        &ecken,
        &[]
    ));
}
