//! Punktfang beim Bearbeiten einer ausgewählten Polylinie.
//!
//! **Regel:** Ein Element, das gerade bearbeitet wird, ist kein Fangziel. Es
//! steht in `ignore` und liefert überhaupt keine Fangpunkte — auch nicht seine
//! ruhenden Nachbarecken. Damit kann es sich nicht selbst fangen, und es gibt
//! keine Liste „welche Punkte wandern mit?", die je Elementart gepflegt werden
//! müsste.
//!
//! Fremde Elemente fangen unverändert, auch wenn sie genau auf einer Ecke des
//! bearbeiteten Elements liegen.
//!
//! Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained, SnapKind, SnapResolution, SnapSource};
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    // Raster und Zwangsrichtungen aus: gemessen wird der Objektfang.
    s.execute(
        "doc.settings",
        &json!({ "snap": { "grid": false, "ortho": false, "polar": false } }),
    )
    .unwrap();
    s
}

/// Polylinie (0;0) → (2000;0) → (2000;1500) und eine freie Linie
/// (0;3000) → (2000;3000). Liefert (Sitzung, Polylinie, Linie).
fn szenario() -> (Session, Id, Id) {
    let mut s = s();
    s.execute(
        "draw.polyline",
        &json!({ "pts": [[0.0, 0.0], [2000.0, 0.0], [2000.0, 1500.0]] }),
    )
    .unwrap();
    s.execute(
        "draw.line",
        &json!({ "a": [0.0, 3000.0], "b": [2000.0, 3000.0] }),
    )
    .unwrap();
    let poly = s.doc.entities[0].id;
    let linie = s.doc.entities[1].id;
    (s, poly, linie)
}

/// Fang während eines Griffzugs an der Polylinie: sie steht im Ausschluss.
fn fang(s: &Session, cursor: Vec2, tol: f64, poly: Id) -> SnapResolution {
    snap_constrained(&s.doc, cursor, tol, None, &[poly], false)
}

fn punkte(s: &Session, id: Id) -> Vec<Vec2> {
    match &s.doc.entity(id).expect("Element fehlt").kind {
        EntityKind::Polyline { verts, .. } => verts.iter().map(|w| w.p).collect(),
        other => panic!("keine Polylinie: {other:?}"),
    }
}

fn assert_punkt(ist: Vec2, soll: Vec2) {
    assert!(ist.dist(soll) < 1e-9, "erwartet {soll:?}, ist {ist:?}");
}

// ---------------------------------------------------------------------------
// Das bearbeitete Element fängt nicht auf sich selbst
// ---------------------------------------------------------------------------

/// Der gezogene Stützpunkt liegt dauerhaft im Fangradius des Zeigers. Ohne
/// Ausschluss gewinnt der eigene Endpunkt (Priorität END) und der Griff bliebe
/// stehen, statt der Maus zu folgen.
#[test]
fn gezogener_stuetzpunkt_faengt_sich_nicht_selbst() {
    let (s, poly, _) = szenario();
    let cursor = v(5.0, 5.0);
    let res = fang(&s, cursor, 40.0, poly);
    assert_eq!(res.source, SnapSource::Free, "{res:?}");
    assert_punkt(res.point, cursor);
}

/// Auch die **ruhenden** Ecken desselben Elements fangen nicht: sie sind
/// Fangziele desselben, gerade bewegten Objekts. Ein geschlossener Zug entsteht
/// über die Zieh-Toleranz von `apply_grip_tol`, nicht über den Fang.
#[test]
fn eigene_ruhende_ecke_faengt_nicht() {
    let (s, poly, _) = szenario();
    assert_eq!(punkte(&s, poly).len(), 3, "drei Stützpunkte");
    // Zeiger 8 mm neben der Nachbarecke (2000;0) derselben Polylinie.
    let cursor = v(1994.0, 8.0);
    let res = fang(&s, cursor, 40.0, poly);
    assert_eq!(res.source, SnapSource::Free, "{res:?}");
    assert_punkt(res.point, cursor);
}

/// Dasselbe am Anfangspunkt: auch er ist kein Fangziel, solange die Polylinie
/// bearbeitet wird.
#[test]
fn eigener_anfangspunkt_faengt_nicht() {
    let (s, poly, _) = szenario();
    let cursor = v(6.0, -4.0);
    let res = fang(&s, cursor, 40.0, poly);
    assert_eq!(res.source, SnapSource::Free, "{res:?}");
    assert_punkt(res.point, cursor);
}

// ---------------------------------------------------------------------------
// Fremde Elemente fangen unverändert
// ---------------------------------------------------------------------------

/// Ein fremdes Element auf derselben Stelle fängt weiter: der Ausschluss hängt
/// am Element, nicht an der Koordinate.
#[test]
fn fremder_endpunkt_faengt_weiter() {
    let (s, poly, linie) = szenario();
    assert_ne!(linie, poly);
    let ziel = v(2000.0, 3000.0);
    let res = fang(&s, v(1990.0, 3008.0), 120.0, poly);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint), "{res:?}");
    assert_punkt(res.point, ziel);
}

/// Ohne Bearbeitung fängt die Polylinie ganz normal — der Ausschluss gilt nur,
/// solange sie das bearbeitete Element ist.
#[test]
fn ohne_bearbeitung_faengt_die_polylinie_normal() {
    let (s, poly, _) = szenario();
    let res = snap_constrained(&s.doc, v(1994.0, 8.0), 40.0, None, &[], false);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint), "{res:?}");
    assert_punkt(res.point, v(2000.0, 0.0));
}
