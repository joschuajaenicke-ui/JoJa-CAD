//! Punktfang beim Bearbeiten einer ausgewählten Polylinie (Kanban t_eb950a21).
//!
//! Beim Ziehen eines Stützpunkts wandert genau *ein* Punkt mit der Maus. Die
//! Fang-Engine bekommt deshalb drei Angaben getrennt:
//!
//! * `selected` — das Element bleibt mit seinen diskreten Punkten fangbar
//!   (die Polylinie soll sich an ihre eigenen Nachbarecken anschließen können),
//! * `gesperrt` — nur der gezogene Punkt selbst fängt nicht (sonst zieht er
//!   sich selbst fest und der Griff bleibt stehen),
//! * ein fremdes Element auf derselben Stelle fängt weiter (sonst bricht der
//!   Fang in dem Moment ab, in dem der Griff auf dem Ziel ankommt).
//!
//! Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained_ohne_punkte, SnapKind, SnapResolution, SnapSource};
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

/// Fang mit der Polylinie als *ausgewähltem* Element und gesperrten
/// Stützpunkten — genau die Lage während eines Griffzugs.
fn fang(s: &Session, cursor: Vec2, tol: f64, poly: Id, gesperrt: &[(Id, Vec2)]) -> SnapResolution {
    snap_constrained_ohne_punkte(&s.doc, cursor, tol, None, &[], &[poly], gesperrt, false)
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
// Die ausgewählte Polylinie darf sich selbst fangen
// ---------------------------------------------------------------------------

/// Kern der Karte: die blau hinterlegte Polylinie liefert ihre eigene
/// Nachbarecke als Fangpunkt. Vorher schob die Zeichenfläche die *ganze*
/// Auswahl in den Ausschluss — die Polylinie konnte sich nicht an sich selbst
/// anschließen, geschlossene Züge waren unmöglich.
#[test]
fn ausgewaehlte_polylinie_faengt_eigene_ecke() {
    let (s, poly, _) = szenario();
    assert_eq!(punkte(&s, poly).len(), 3, "drei Stützpunkte");
    // Griff 0 (auf (0;0)) wird gezogen und ist gesperrt; der Zeiger steht 8 mm
    // neben der Ecke (2000;0) derselben Polylinie.
    let res = fang(&s, v(1994.0, 8.0), 40.0, poly, &[(poly, v(0.0, 0.0))]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint), "{res:?}");
    assert_punkt(res.point, v(2000.0, 0.0));
}

/// Der Anfangspunkt derselben Polylinie ist ebenfalls fangbar — so entsteht der
/// geschlossene Zug (letzter Stützpunkt auf den ersten).
#[test]
fn anfangspunkt_der_eigenen_polylinie_faengt() {
    let (s, poly, _) = szenario();
    // Griff 2 (auf (2000;1500)) wird gezogen, Ziel ist der Anfang (0;0).
    let res = fang(&s, v(6.0, -4.0), 40.0, poly, &[(poly, v(2000.0, 1500.0))]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint), "{res:?}");
    assert_punkt(res.point, v(0.0, 0.0));
}

// ---------------------------------------------------------------------------
// Der gezogene Punkt fängt sich nicht selbst
// ---------------------------------------------------------------------------

/// Ohne Sperre gewinnt der eigene Endpunkt (Priorität END) und der Griff bliebe
/// stehen, statt der Maus zu folgen.
#[test]
fn gezogener_stuetzpunkt_faengt_sich_nicht_selbst() {
    let (s, poly, _) = szenario();
    let cursor = v(5.0, 5.0);
    let res = fang(&s, cursor, 40.0, poly, &[(poly, v(0.0, 0.0))]);
    assert_eq!(res.source, SnapSource::Free, "{res:?}");
    assert_punkt(res.point, cursor);
}

/// Sperre nur für *diesen* Punkt: die ruhende Nachbarecke fängt weiter.
#[test]
fn ruhende_nachbar_ecke_bleibt_trotz_sperre_fangbar() {
    let (s, poly, _) = szenario();
    let res = fang(&s, v(1994.0, 1506.0), 40.0, poly, &[(poly, v(2000.0, 0.0))]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint), "{res:?}");
    assert_punkt(res.point, v(2000.0, 1500.0));
}

/// Ein fremdes Element auf derselben Stelle fängt weiter (die Sperre hängt am
/// Element, nicht an der Koordinate). Sonst fiele der Fang in dem Moment auf
/// „Nächster Punkt“ derselben Linie zurück, in dem der Griff ankommt — der
/// Punkt wanderte bei jeder weiteren Bewegung um Bruchteile.
#[test]
fn fremder_endpunkt_auf_gesperrter_stelle_faengt_weiter() {
    let (s, poly, linie) = szenario();
    assert_ne!(linie, poly);
    let ziel = v(2000.0, 3000.0);
    let res = fang(&s, v(1990.0, 3008.0), 120.0, poly, &[(poly, ziel)]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint), "{res:?}");
    assert_punkt(res.point, ziel);
}
