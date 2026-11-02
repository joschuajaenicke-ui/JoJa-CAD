//! Verschneiden (T0082): zwei Linien oder Wände werden auf ihren gemeinsamen
//! Schnittpunkt gezogen — verlängert, wenn der Schnittpunkt hinter dem Ende
//! liegt, sonst auf die Ecke gekürzt.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::style::WallAlign;
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

fn last_id(s: &Session) -> u64 {
    s.doc.entities.last().unwrap().id
}

fn linie(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> u64 {
    s.execute("draw.line", &json!({"a": a, "b": b}))
        .expect("Linie anlegen");
    last_id(s)
}

fn wand(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> u64 {
    s.execute(
        "draw.wall",
        &json!({"a": a, "b": b, "thickness": 0.3, "height": 2.5}),
    )
    .expect("Wand anlegen");
    last_id(s)
}

/// Endpunkte einer Linie bzw. Wandachse (in mm).
fn enden(s: &Session, id: u64) -> (Vec2, Vec2) {
    match &s.doc.entity(id).expect("Objekt fehlt").kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => (*a, *b),
        other => panic!("keine Gerade: {other:?}"),
    }
}

fn assert_punkt(ist: Vec2, soll: Vec2, was: &str) {
    assert!(
        ist.dist(soll) < 1e-6,
        "{was}: erwartet {soll:?}, ist {ist:?}"
    );
}

/// Beide Linien enden vor dem Schnittpunkt — beide werden verlängert.
#[test]
fn zwei_linien_treffen_sich_im_schnittpunkt() {
    let mut s = s();
    let waagerecht = linie(&mut s, [0.0, 1.0], [2.0, 1.0]);
    let senkrecht = linie(&mut s, [4.0, 2.0], [4.0, 3.0]);

    s.execute("modify.intersect", &json!({"ids": [waagerecht, senkrecht]}))
        .expect("Verschneiden");

    let (a, b) = enden(&s, waagerecht);
    assert_punkt(a, v(0.0, 1000.0), "Anfang der waagerechten Linie bleibt");
    assert_punkt(b, v(4000.0, 1000.0), "Ende wandert auf den Schnittpunkt");

    let (a, b) = enden(&s, senkrecht);
    assert_punkt(a, v(4000.0, 1000.0), "Anfang der senkrechten Linie wandert");
    assert_punkt(b, v(4000.0, 3000.0), "Ende der senkrechten Linie bleibt");
}

/// Wände werden genauso verschnitten; Dicke, Höhe und Ausrichtung bleiben.
#[test]
fn waende_werden_genauso_verschnitten() {
    let mut s = s();
    let waagerecht = wand(&mut s, [0.0, 1.0], [2.0, 1.0]);
    let senkrecht = wand(&mut s, [4.0, 2.0], [4.0, 3.0]);

    s.execute("modify.intersect", &json!({"ids": [waagerecht, senkrecht]}))
        .expect("Verschneiden");

    let (_, b) = enden(&s, waagerecht);
    assert_punkt(b, v(4000.0, 1000.0), "Wandende am Schnittpunkt");
    let (a, _) = enden(&s, senkrecht);
    assert_punkt(a, v(4000.0, 1000.0), "Wandanfang am Schnittpunkt");

    for id in [waagerecht, senkrecht] {
        match &s.doc.entity(id).unwrap().kind {
            EntityKind::Wall {
                thickness,
                height,
                align,
                ..
            } => {
                assert!((thickness - 300.0).abs() < 1e-6, "Dicke {thickness}");
                assert!((height - 2500.0).abs() < 1e-6, "Höhe {height}");
                assert_eq!(*align, WallAlign::Center, "Ausrichtung bleibt");
            }
            other => panic!("Wand erwartet: {other:?}"),
        }
    }
}

/// Liegt der Schnittpunkt innerhalb beider Strecken, entsteht eine Ecke —
/// das jeweils nähere Ende wandert, das andere bleibt.
#[test]
fn kreuzende_linien_bilden_eine_ecke() {
    let mut s = s();
    let waagerecht = linie(&mut s, [0.0, 0.0], [6.0, 0.0]);
    let senkrecht = linie(&mut s, [4.0, -1.0], [4.0, 3.0]);

    s.execute("modify.intersect", &json!({"ids": [waagerecht, senkrecht]}))
        .expect("Verschneiden");

    let (a, b) = enden(&s, waagerecht);
    assert_punkt(a, v(0.0, 0.0), "fernes Ende bleibt");
    assert_punkt(b, v(4000.0, 0.0), "nahes Ende auf die Ecke gekürzt");

    let (a, b) = enden(&s, senkrecht);
    assert_punkt(a, v(4000.0, 0.0), "nahes Ende auf die Ecke gekürzt");
    assert_punkt(b, v(4000.0, 3000.0), "fernes Ende bleibt");
}

/// Parallele Objekte haben keinen Schnittpunkt — das ist ein Fehler, kein
/// stiller Erfolg (T0038).
#[test]
fn parallele_linien_sind_ein_fehler() {
    let mut s = s();
    let eins = linie(&mut s, [0.0, 0.0], [2.0, 0.0]);
    let zwei = linie(&mut s, [0.0, 1.0], [2.0, 1.0]);
    let vorher = (enden(&s, eins), enden(&s, zwei));

    let fehler = s
        .execute("modify.intersect", &json!({"ids": [eins, zwei]}))
        .expect_err("parallel muss ein Fehler sein");
    assert!(fehler.contains("parallel"), "Meldung: {fehler}");
    assert_eq!((enden(&s, eins), enden(&s, zwei)), vorher, "nichts verändert");
}

/// Die Vorschau rechnet dieselben Strecken, die der Befehl danach setzt.
#[test]
fn vorschau_zeigt_dieselben_strecken() {
    let mut s = s();
    let waagerecht = linie(&mut s, [0.0, 1.0], [2.0, 1.0]);
    let senkrecht = linie(&mut s, [4.0, 2.0], [4.0, 3.0]);

    let vorschau = jjcad_core::actions::verschneiden_vorschau(&s.doc, waagerecht, senkrecht);
    assert_eq!(vorschau.len(), 2, "beide Objekte zeigen ihre Strecke");
    for (_id, segs) in &vorschau {
        assert_eq!(segs.len(), 1);
    }

    s.execute("modify.intersect", &json!({"ids": [waagerecht, senkrecht]}))
        .expect("Verschneiden");

    for (id, segs) in vorschau {
        let (a, b) = enden(&s, id);
        let jjcad_core::geom::Seg::Line(va, vb) = segs[0] else {
            panic!("Strecke erwartet");
        };
        // Die Vorschau läuft vom stehenden Ende zum Schnittpunkt — einer der
        // beiden neuen Endpunkte muss der Schnittpunkt sein.
        let ziel = v(4000.0, 1000.0);
        assert!(
            (va.dist(ziel) < 1e-6 && (vb.dist(a) < 1e-6 || vb.dist(b) < 1e-6))
                || (vb.dist(ziel) < 1e-6 && (va.dist(a) < 1e-6 || va.dist(b) < 1e-6)),
            "Vorschau {va:?}–{vb:?} passt nicht zum Ergebnis {a:?}–{b:?}"
        );
    }
}
