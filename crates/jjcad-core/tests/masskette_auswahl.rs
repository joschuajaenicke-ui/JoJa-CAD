//! Auswahl von Maßketten: die Trefferfläche ist der sichtbare Bereich.
//!
//! Eine Maßkette speichert ihre Basispunkte am Bauteil; gezeichnet wird die
//! Maßlinie versetzt daneben. Vorher prüften Klick (`snap::hit_test`) und
//! Fensterauswahl (`snap::select_window`) nur die Grundlinie zwischen den
//! Basispunkten: der sichtbare Maßtext und die Maßlinie trafen nicht, ein
//! Klick auf der unsichtbaren Grundlinie dagegen schon (T0052).

use jjcad_core::geom::{BBox, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::render;
use jjcad_core::session::Session;
use jjcad_core::snap;
use serde_json::json;

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

/// Maßkette über drei Punkte (0 / 2 / 4 m), Maßlinie 1,5 m unter der Wand.
/// Achse x, Textgröße aus der Zeichnung (Standard 2,5 mm × Maßstab 100).
fn kette() -> (Session, u64) {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let antwort = s
        .execute(
            "dim.chain",
            &json!({
                "pts": [[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]],
                "line_pos": [0.0, -1.5],
                "axis": "x"
            }),
        )
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    (s, id)
}

fn mitte(kasten: &[Vec2; 4]) -> Vec2 {
    let s = kasten.iter().fold(v(0.0, 0.0), |acc, p| acc + *p);
    s * 0.25
}

#[test]
fn klick_trifft_die_gezeichnete_masslinie() {
    let (s, id) = kette();
    // Mitte der Maßlinie: die Basispunkte liegen bei y = 0, gezeichnet wird
    // die Maßlinie bei y = -1500.
    let treffer = snap::hit_test(&s.doc, v(2000.0, -1500.0), 20.0, None);
    assert_eq!(
        treffer,
        Some(id),
        "Klick auf die sichtbare Maßlinie muss die Kette wählen"
    );
}

#[test]
fn klick_zwischen_den_basispunkten_trifft_nicht() {
    let (s, id) = kette();
    // Auf der Verbindung zwischen den Basispunkten ist nichts gezeichnet.
    let treffer = snap::hit_test(&s.doc, v(1000.0, 0.0), 20.0, None);
    assert_ne!(
        treffer,
        Some(id),
        "die unsichtbare Grundlinie zwischen den Basispunkten ist keine Trefferfläche"
    );
}

#[test]
fn klick_auf_den_masstext_waehlt_die_kette() {
    let (s, id) = kette();
    let e = s.doc.entity(id).unwrap();
    let tb = render::trefferbild(&s.doc, e);
    assert_eq!(tb.text_kaesten.len(), 2, "zwei Maßzahlen in der Kette");
    let treffer = snap::hit_test(&s.doc, mitte(&tb.text_kaesten[0]), 20.0, None);
    assert_eq!(treffer, Some(id), "Klick auf die Maßzahl muss die Kette wählen");
}

#[test]
fn querfenster_ueber_der_masszahl_waehlt_die_kette() {
    let (s, id) = kette();
    let e = s.doc.entity(id).unwrap();
    let tb = render::trefferbild(&s.doc, e);
    // Nur um die erste Maßzahl — Maßlinie und Hilfslinien ragen darüber hinaus
    // (genau der Fall aus dem Screenshot).
    let kb = BBox::from_points(&tb.text_kaesten[0]).expanded(-0.01);
    let ids = snap::select_window(&s.doc, v(kb.min.x, kb.min.y), v(kb.max.x, kb.max.y), true);
    assert!(
        ids.contains(&id),
        "Querfenster über der Maßzahl muss die Kette wählen: {ids:?}"
    );
}

#[test]
fn fenster_zwischen_den_basispunkten_waehlt_nicht() {
    let (s, id) = kette();
    let ids = snap::select_window(&s.doc, v(1000.0, -100.0), v(3000.0, 100.0), false);
    assert!(
        !ids.contains(&id),
        "dort ist nichts gezeichnet: {ids:?}"
    );
}

#[test]
fn fenster_um_die_ganze_kette_waehlt() {
    let (s, id) = kette();
    let bb = render::entity_bbox(&s.doc, s.doc.entity(id).unwrap());
    let ids = snap::select_window(
        &s.doc,
        v(bb.min.x - 100.0, bb.min.y - 100.0),
        v(bb.max.x + 100.0, bb.max.y + 100.0),
        false,
    );
    assert!(ids.contains(&id), "Fenster um die ganze Kette: {ids:?}");
}

#[test]
fn klick_trifft_die_einzelmasslinie() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let antwort = s
        .execute(
            "dim.linear",
            &json!({ "a": [0.0, 0.0], "b": [3.0, 0.0], "pos": [0.0, -1.5], "axis": "x" }),
        )
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let e = s.doc.entity(id).unwrap();
    assert!(matches!(e.kind, EntityKind::Dim { .. }));
    // Sichtbare Maßlinie bei y = -1500, Basispunkte bei y = 0.
    assert_eq!(
        snap::hit_test(&s.doc, v(1500.0, -1500.0), 20.0, None),
        Some(id),
        "Klick auf die sichtbare Maßlinie"
    );
    assert_ne!(
        snap::hit_test(&s.doc, v(1500.0, 0.0), 20.0, None),
        Some(id),
        "Klick auf der unsichtbaren Grundlinie trifft nicht"
    );
    let tb = render::trefferbild(&s.doc, e);
    assert_eq!(tb.text_kaesten.len(), 1);
    assert_eq!(
        snap::hit_test(&s.doc, mitte(&tb.text_kaesten[0]), 20.0, None),
        Some(id),
        "Klick auf die Maßzahl"
    );
}
