//! Punktfang an einem Spline.
//!
//! Die gezeichnete Kurve ist eine Zerlegung in viele kurze Teilstrecken. Deren
//! Enden sind **keine** Fangpunkte — sie sind keine gesetzten Punkte, sondern
//! Rechenergebnis; an jedem Zerlegungspunkt hinge sonst ein „Endpunkt", den
//! niemand gesetzt hat. Fangbar sind allein die Stützpunkte.
//!
//! Während der Spline **bearbeitet** wird, fängt er überhaupt nicht: er steht
//! dann im Ausschluss der Fang-Engine (siehe `snap_edit_griff.rs`).
//!
//! Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{v, Spline, Vec2, SPLINE_TOL_MM};
use jjcad_core::model::Id;
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained, SnapKind, SnapResolution, SnapSource};
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

/// Fang ohne Bearbeitung: der Spline steht im Dokument, nichts ist
/// ausgeschlossen.
fn fang(s: &Session, cursor: Vec2) -> SnapResolution {
    snap_constrained(&s.doc, cursor, 20.0, None, &[], false)
}

/// Fang während eines Griffzugs am Spline: er steht im Ausschluss.
fn fang_bearbeitet(s: &Session, cursor: Vec2, id: Id) -> SnapResolution {
    snap_constrained(&s.doc, cursor, 20.0, None, &[id], false)
}

fn stuetzpunkte() -> Vec<Vec2> {
    STUETZ.iter().map(|(x, y)| v(*x, *y)).collect()
}

/// Ein Punkt der Kurvenzerlegung, der **kein** Stützpunkt ist und außerhalb
/// des Fangradius (20 mm) aller Stützpunkte liegt.
///
/// Seit die Zerlegung der Pfeilhöhe folgt (T0175), liegen die Zwischenpunkte
/// dichter an den Stützpunkten; ein Punkt im Fangradius fängt zu Recht auf dem
/// Stützpunkt. Für die Prüfung „der Kurvenpunkt selbst fängt nicht" wird
/// deshalb der Punkt mit dem größten Abstand zu allen Stützpunkten genommen.
fn zwischenpunkt() -> Vec2 {
    let kurve = Spline::new(stuetzpunkte(), false).flatten(SPLINE_TOL_MM);
    let abstand = |p: &Vec2| {
        stuetzpunkte()
            .iter()
            .map(|q| p.dist(*q))
            .fold(f64::INFINITY, f64::min)
    };
    kurve
        .iter()
        .copied()
        .filter(|p| abstand(p) > 40.0)
        .max_by(|a, b| abstand(a).partial_cmp(&abstand(b)).unwrap())
        .expect("die Zerlegung hat Zwischenpunkte")
}

/// Ein Zwischenpunkt der Kurve fängt nicht: er ist keine gesetzte Stelle.
#[test]
fn teilstrecken_der_kurve_fangen_nicht() {
    let (s, _) = szenario();
    let zwischen = zwischenpunkt();
    let res = fang(&s, zwischen);
    assert!(
        !res.is_snap(),
        "Teilstrecken-Ende darf nicht fangen: {res:?}"
    );
    assert!(
        res.point.dist(zwischen) < 1e-9,
        "ohne Fang bleibt der Zeigerpunkt stehen: {res:?}"
    );
}

/// Kein einziger Zwischenpunkt der Zerlegung zieht den Zeiger an sich.
///
/// Fängt ein Kurvenpunkt doch, muss das Ziel ein **Stützpunkt** sein — der darf
/// fangen. Mit feinerer Zerlegung (T0175) liegen Zwischenpunkte dichter am
/// Stützpunkt und geraten in dessen Fangradius; das ist richtig, solange der
/// Stützpunkt das Ziel ist und nicht das Teilstrecken-Ende.
#[test]
fn kein_kurvenpunkt_zieht_den_zeiger() {
    let (s, _) = szenario();
    let kurve = Spline::new(stuetzpunkte(), false).flatten(SPLINE_TOL_MM);
    for p in &kurve {
        if stuetzpunkte().iter().any(|q| p.dist(*q) <= 1.0) {
            continue;
        }
        let res = fang(&s, *p);
        if res.is_snap() {
            assert!(
                stuetzpunkte().iter().any(|q| res.point.dist(*q) < 1e-9),
                "Kurvenpunkt {p:?} fängt auf {:?} — nur Stützpunkte dürfen fangen",
                res.point
            );
        }
    }
}

/// Die Stützpunkte sind die festen Fangziele der Kurve.
#[test]
fn stuetzpunkt_faengt() {
    let (s, _) = szenario();
    let ziel = v(2000.0, 0.0);
    let res = fang(&s, ziel + v(6.0, 4.0));
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint));
    assert!(
        res.point.dist(ziel) < 1e-9,
        "der Stützpunkt muss das Fangziel sein: {res:?}"
    );
}

/// Während der Spline bearbeitet wird, fängt er gar nicht — auch nicht mit
/// seinen Stützpunkten. Der gezogene Punkt bleibt am Zeiger, der Griff folgt
/// der Maus.
#[test]
fn bearbeiteter_spline_faengt_nicht() {
    let (s, id) = szenario();
    // Der gezogene Stützpunkt selbst …
    let res = fang_bearbeitet(&s, v(3.0, 3.0), id);
    assert!(!res.is_snap(), "der gezogene Punkt darf nicht fangen: {res:?}");
    // … und ein ruhender Stützpunkt desselben Splines.
    let ruhend = v(2000.0, 0.0);
    let res = fang_bearbeitet(&s, ruhend + v(6.0, 4.0), id);
    assert!(
        !res.is_snap(),
        "der bearbeitete Spline darf gar nicht fangen: {res:?}"
    );
    assert!(res.point.dist(ruhend + v(6.0, 4.0)) < 1e-9, "{res:?}");
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
    let res = fang_bearbeitet(&s, zwischen + v(4.0, 3.0), id);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint));
    assert!(res.point.dist(zwischen) < 1e-9, "{res:?}");
    assert_ne!(res.point, v(0.0, 0.0));
}

/// „Nächster Punkt" auf der Kurve bleibt erhalten: ein **nicht** bearbeiteter
/// Spline lässt sich weiterhin an seiner Kurve fangen.
#[test]
fn naechster_punkt_auf_der_kurve_bleibt() {
    let mut s = s();
    s.execute("doc.settings", &json!({ "snap": { "nearest": true } }))
        .unwrap();
    s.execute("draw.spline", &json!({ "pts": STUETZ }))
        .unwrap();
    let zwischen = zwischenpunkt();
    let res = snap_constrained(&s.doc, zwischen, 20.0, None, &[], false);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Nearest));
    assert!(res.point.dist(zwischen) < 1e-6, "{res:?}");
}
