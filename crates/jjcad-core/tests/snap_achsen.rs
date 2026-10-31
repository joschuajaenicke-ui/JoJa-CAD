//! Ortho/Polar mit **mehreren** Bezugspunkten (`snap::snap_constrained_achsen`).
//!
//! Ein gezogener Stützpunkt einer Polylinie hängt an zwei Linien. Mit nur einem
//! Bezug ließ sich immer nur eine davon ausrichten; hier wird geprüft, dass
//! beide Achsen gleichzeitig zur Verfügung stehen — jede Koordinate einzeln,
//! ohne die harte (toleranzlose) Ortho-Regel des Erstellens anzutasten.

use jjcad_core::geom::Vec2;
use jjcad_core::model::Document;
use jjcad_core::snap::{snap_constrained_achsen, SnapSource};

fn doc() -> Document {
    let mut d = Document::new();
    d.settings.snap.ortho = true;
    d.settings.snap.polar = false;
    d.settings.snap.grid = false;
    d
}

fn fang(d: &Document, cursor: Vec2, tol: f64, achsen: &[Vec2]) -> (Vec2, SnapSource) {
    let r = snap_constrained_achsen(
        d, cursor, tol, achsen, &[], &[], &[], &[], &[], false,
    );
    (r.point, r.source)
}

/// Liegen beide Koordinaten an je einer Achse (hier des vorigen und des
/// nächsten Stützpunkts), rastet der Punkt auf den Schnittpunkt — beide
/// angrenzenden Linien stehen dann gleichzeitig im Zwang.
#[test]
fn beide_achsen_rasten_gleichzeitig() {
    let d = doc();
    // Nachbarn: voriger (0;0), nächster (2000;4000)
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(2000.0, 4000.0)];
    let (p, quelle) = fang(&d, Vec2::new(1980.0, 50.0), 100.0, &achsen);
    assert!(p.dist(Vec2::new(2000.0, 0.0)) < 1e-9, "Punkt {p:?}");
    assert_eq!(quelle, SnapSource::Ortho);
}

/// Nur eine Koordinate liegt an einer Achse: die andere bleibt am Zeiger.
#[test]
fn eine_achse_reicht() {
    let d = doc();
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(2000.0, 4000.0)];
    let (p, quelle) = fang(&d, Vec2::new(1980.0, 1500.0), 100.0, &achsen);
    assert!(p.dist(Vec2::new(2000.0, 1500.0)) < 1e-9, "Punkt {p:?}");
    assert_eq!(quelle, SnapSource::Ortho);
}

/// Weit von jeder Achse entfernt bleibt der Zeiger frei — der Fang führt,
/// er zwingt nicht.
#[test]
fn ausserhalb_des_radius_frei() {
    let d = doc();
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(2000.0, 4000.0)];
    let (p, quelle) = fang(&d, Vec2::new(1200.0, 1500.0), 100.0, &achsen);
    assert!(p.dist(Vec2::new(1200.0, 1500.0)) < 1e-9, "Punkt {p:?}");
    assert_eq!(quelle, SnapSource::Free);
}

/// Ein einziger Bezug verhält sich wie beim Erstellen: hartes Ortho, ohne
/// Fangradius — sonst ließe sich eine Linie nicht mehr frei ziehen.
#[test]
fn ein_bezug_bleibt_hartes_ortho() {
    let d = doc();
    let (p, quelle) = fang(&d, Vec2::new(2500.0, 300.0), 40.0, &[Vec2::new(0.0, 0.0)]);
    assert!(p.dist(Vec2::new(2500.0, 0.0)) < 1e-9, "Punkt {p:?}");
    assert_eq!(quelle, SnapSource::Ortho);
}

/// Ohne Bezug bleibt alles, wie es ist.
#[test]
fn ohne_bezug_frei() {
    let d = doc();
    let (p, quelle) = fang(&d, Vec2::new(2500.0, 300.0), 40.0, &[]);
    assert!(p.dist(Vec2::new(2500.0, 300.0)) < 1e-9);
    assert_eq!(quelle, SnapSource::Free);
}

/// Polarfang mit zwei Bezügen: die Richtung kommt vom nächsten Bezug, die
/// Länge bleibt — wie beim Erstellen.
#[test]
fn polar_nimmt_den_naechsten_bezug() {
    let mut d = doc();
    d.settings.snap.ortho = false;
    d.settings.snap.polar = true;
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(5000.0, 0.0)];
    // Zeiger 3° unter der 45°-Richtung des ersten Bezugs, weit vom zweiten.
    let roh = Vec2::new(-200.0, 180.0);
    let (p, quelle) = fang(&d, roh, 40.0, &achsen);
    assert_eq!(quelle, SnapSource::Polar);
    assert!((p.x.abs() - p.y.abs()).abs() < 1e-6, "nicht auf 45°: {p:?}");
    assert!((p.dist(Vec2::new(0.0, 0.0)) - roh.dist(Vec2::new(0.0, 0.0))).abs() < 1e-6);
}
