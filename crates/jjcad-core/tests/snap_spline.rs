//! Punktfang an einem Spline (T0134).
//!
//! Die gezeichnete Kurve ist eine Zerlegung in viele kurze Teilstrecken. Deren
//! Enden dürfen **keine** Fangpunkte sein: beim Ziehen an einem Stützpunkt
//! wandert die Kurve mit, der Fang zöge den Zeiger auf einen Punkt, der im
//! nächsten Bild woanders liegt — die Linie flackert und zittert. Fangbar sind
//! allein die Stützpunkte (die vom Nutzer gesetzten Punkte).
//!
//! Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{v, Spline, Vec2, SPLINE_TOL_MM};
use jjcad_core::model::Id;
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained_ohne_punkte, SnapKind, SnapResolution, SnapSource};
use serde_json::json;

/// Stützpunkte des Prüfsplines — die Kurve wölbt sich deutlich zwischen ihnen.
const STUETZ: [(f64, f64); 4] = [
    (0.0, 0.0),
    (1000.0, 600.0),
    (2000.0, 0.0),
    (3000.0, 600.0),
];

/// Sitzung ohne Raster, Ortho, Polar und ohne kontinuierliche Fänge: gemessen
/// wird allein, welche **Punkte** der Spline anbietet.
fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": {
            "grid": false,
            "ortho": false,
            "polar": false,
            "nearest": false,
            "midpoint": false,
            "perpendicular": false,
            "intersection": false,
            "center": false
        } }),
    )
    .unwrap();
    s
}

fn szenario() -> (Session, Id) {
    let mut s = s();
    s.execute("draw.spline", &json!({ "pts": STUETZ }))
        .unwrap();
    let id = s.doc.entities[0].id;
    (s, id)
}

/// Fang wie während eines Griffzugs: der Spline ist ausgewählt (`selected`),
/// der gezogene Stützpunkt ist gesperrt.
fn fang(s: &Session, cursor: Vec2, id: Id, gesperrt: &[(Id, Vec2)]) -> SnapResolution {
    snap_constrained_ohne_punkte(&s.doc, cursor, 20.0, None, &[], &[id], gesperrt, false)
}

fn stuetzpunkte() -> Vec<Vec2> {
    STUETZ.iter().map(|(x, y)| v(*x, *y)).collect()
}

/// Ein Punkt der Kurvenzerlegung, der **kein** Stützpunkt ist.
fn zwischenpunkt() -> Vec2 {
    let kurve = Spline::new(stuetzpunkte(), false).flatten(SPLINE_TOL_MM);
    kurve
        .iter()
        .copied()
        .find(|p| stuetzpunkte().iter().all(|q| p.dist(*q) > 1.0))
        .expect("die Zerlegung hat Zwischenpunkte")
}

/// Der Kern des Tickets: ein Zwischenpunkt der Kurve fängt nicht.
#[test]
fn teilstrecken_der_kurve_fangen_nicht() {
    let (s, id) = szenario();
    let zwischen = zwischenpunkt();
    let res = fang(&s, zwischen, id, &[(id, v(0.0, 0.0))]);
    assert!(
        !res.is_snap(),
        "Teilstrecken-Ende darf nicht fangen, sonst zittert die Linie: {res:?}"
    );
    assert!(
        res.point.dist(zwischen) < 1e-9,
        "ohne Fang bleibt der Zeigerpunkt stehen: {res:?}"
    );
}

/// Kein einziger Zwischenpunkt der Zerlegung zieht den Zeiger an sich.
#[test]
fn kein_kurvenpunkt_zieht_den_zeiger() {
    let (s, id) = szenario();
    let kurve = Spline::new(stuetzpunkte(), false).flatten(SPLINE_TOL_MM);
    let gesperrt = [(id, v(0.0, 0.0))];
    for p in &kurve {
        if stuetzpunkte().iter().any(|q| p.dist(*q) <= 1.0) {
            continue;
        }
        let res = fang(&s, *p, id, &gesperrt);
        assert!(
            !res.is_snap(),
            "Kurvenpunkt {p:?} fängt und verschiebt damit die Linie: {res:?}"
        );
    }
}

/// Die Stützpunkte bleiben fangbar — auch während ein anderer gezogen wird.
#[test]
fn stuetzpunkt_faengt_beim_griffzug() {
    let (s, id) = szenario();
    let ziel = v(2000.0, 0.0);
    let res = fang(&s, ziel + v(6.0, 4.0), id, &[(id, v(0.0, 0.0))]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint));
    assert!(
        res.point.dist(ziel) < 1e-9,
        "der Stützpunkt muss das Fangziel sein: {res:?}"
    );
}

/// Der gezogene Stützpunkt fängt sich nicht selbst (sonst bleibt der Griff
/// stehen) — die Sperre greift weiterhin.
#[test]
fn gezogener_stuetzpunkt_faengt_sich_nicht_selbst() {
    let (s, id) = szenario();
    let res = fang(&s, v(3.0, 3.0), id, &[(id, v(0.0, 0.0))]);
    assert!(!res.is_snap(), "der gezogene Punkt darf nicht fangen: {res:?}");
}

/// Ein fremdes Objekt auf derselben Stelle fängt unverändert — der Fang darf
/// nicht allgemein abgeschaltet worden sein.
#[test]
fn fremdes_objekt_faengt_weiterhin() {
    let (mut s, id) = szenario();
    let zwischen = zwischenpunkt();
    s.execute(
        "draw.line",
        &json!({ "a": [zwischen.x, zwischen.y], "b": [zwischen.x + 500.0, zwischen.y + 500.0] }),
    )
    .unwrap();
    let res = fang(&s, zwischen + v(4.0, 3.0), id, &[(id, v(0.0, 0.0))]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint));
    assert!(res.point.dist(zwischen) < 1e-9, "{res:?}");
    assert_ne!(res.point, v(0.0, 0.0));
}

/// „Nächster Punkt" auf der Kurve bleibt erhalten: ein **nicht** ausgewählter
/// Spline lässt sich weiterhin an seiner Kurve fangen.
#[test]
fn naechster_punkt_auf_der_kurve_bleibt() {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "nearest": true } }),
    )
    .unwrap();
    s.execute("draw.spline", &json!({ "pts": STUETZ }))
        .unwrap();
    let zwischen = zwischenpunkt();
    let res = snap_constrained_ohne_punkte(&s.doc, zwischen, 20.0, None, &[], &[], &[], false);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Nearest));
    assert!(res.point.dist(zwischen) < 1e-6, "{res:?}");
}
