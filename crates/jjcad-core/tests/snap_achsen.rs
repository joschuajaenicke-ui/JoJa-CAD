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
        d, cursor, tol, achsen, false, &[], &[], &[], false,
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

/// Der **Schnittpunkt** zweier Achsen (eine je Nachbar) ist auch dann zu
/// fangen, wenn keine der beiden Koordinaten allein im Radius liegt — er ist
/// das Ziel, das man anvisiert (T0046, Turn 4).
#[test]
fn schnittpunkt_auch_ohne_einzelachse() {
    let d = doc();
    // Ecke zwischen Nachbarn (0;0) und (1500;4500): Schnittpunkte (1500;0)
    // und (0;4500).
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(1500.0, 4500.0)];
    // 110 mm neben dem Schnittpunkt in beiden Koordinaten: keine Einzelachse
    // im Radius (81), der Punkt aber im Schnittpunktfenster (162).
    let (p, quelle) = fang(&d, Vec2::new(1610.0, 110.0), 81.0, &achsen);
    assert!(p.dist(Vec2::new(1500.0, 0.0)) < 1e-9, "Punkt {p:?}");
    assert_eq!(quelle, SnapSource::Ortho);
}

/// Weit außerhalb des Schnittpunktfensters bleibt der Zeiger frei.
#[test]
fn ausserhalb_des_schnittpunktfensters_frei() {
    let d = doc();
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(1500.0, 4500.0)];
    let (p, quelle) = fang(&d, Vec2::new(1800.0, 300.0), 81.0, &achsen);
    assert!(p.dist(Vec2::new(1800.0, 300.0)) < 1e-9, "Punkt {p:?}");
    assert_eq!(quelle, SnapSource::Free);
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

fn doc_polar() -> Document {
    let mut d = Document::new();
    d.settings.snap.ortho = false;
    d.settings.snap.polar = true;
    d.settings.snap.grid = false;
    d
}

/// **Alle** Bezüge liefern ihren Winkelkranz, nicht nur der nächstgelegene
/// (T0054): der Zeiger steht zwar näher am zweiten Bezug, dort liegt aber
/// keine Richtung im Winkel-Fenster — die Waagerechte des ersten Bezugs muss
/// trotzdem fangen.
#[test]
fn polar_rastet_auf_den_ferneren_bezug() {
    let d = doc_polar();
    // Voriger (0;0), nächster (2500;0). Zeiger (2600;30): Abstand 104 zum
    // nächsten, 2600 zum vorigen. Vom nächsten aus 17° (kein Fenster), vom
    // vorigen aus 0,7° an der Waagerechten — seitlich 30 mm, im Band von 40 mm.
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(2500.0, 0.0)];
    let roh = Vec2::new(2600.0, 30.0);
    let (p, quelle) = fang(&d, roh, 40.0, &achsen);
    assert_eq!(quelle, SnapSource::Polar, "Fang fehlt: {p:?}");
    assert!(p.y.abs() < 1e-6, "nicht auf der Waagerechten: {p:?}");
    assert!((p.dist(Vec2::ZERO) - roh.dist(Vec2::ZERO)).abs() < 1e-6);
}

/// Kreuzung zweier Polarrichtungen **verschiedener** Bezüge ist ein eigenes
/// Punktziel (T0054): der Zeiger wird auf den Schnittpunkt gezogen, nicht nur
/// auf die nähere Einzelgerade.
#[test]
fn polar_schnittpunkt_zweier_bezuege_rastet_ein() {
    let d = doc_polar();
    // Voriger (0;0), nächster (4000;0): 45° des ersten (y = x) schneidet 135°
    // des zweiten (y = 4000 − x) in (2000;2000).
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(4000.0, 0.0)];
    // 36 mm neben der Kreuzung; die Einzelgerade läge 7 mm daneben am Zeiger.
    let roh = Vec2::new(2030.0, 2020.0);
    let (p, quelle) = fang(&d, roh, 40.0, &achsen);
    assert_eq!(quelle, SnapSource::Polar);
    assert!(p.dist(Vec2::new(2000.0, 2000.0)) < 1e-6, "nicht die Kreuzung: {p:?}");
}

/// Weit von jeder Kreuzung bleibt es beim Fang auf die Einzelgerade — der
/// Schnittpunkt ist ein Punktziel, kein Flächenfang.
#[test]
fn ferne_kreuzung_faengt_nicht() {
    let d = doc_polar();
    let achsen = [Vec2::new(0.0, 0.0), Vec2::new(4000.0, 0.0)];
    // 45°-Gerade des ersten Bezugs, 1200 mm von der Kreuzung entfernt.
    let roh = Vec2::new(3200.0, 3200.0);
    let (p, _) = fang(&d, roh, 40.0, &achsen);
    assert!(p.dist(Vec2::new(2000.0, 2000.0)) > 1000.0, "{p:?}");
}

/// Der Polarfang greift nur in einem **Band** begrenzter Breite, nicht in
/// einem Kegel (T0148): bei 6° Winkel-Fenster wuchs der Fangbereich mit dem
/// Abstand (auf 20 m über 2 m seitlich), weit entfernte Punkte rasteten auf
/// eine Richtung, die der Zeiger nicht meint. Die halbe Bandbreite ist der
/// Fangradius (`tol`), also bildschirmkonstant.
#[test]
fn polar_fangt_nicht_weit_neben_dem_strahl() {
    let d = doc_polar();
    let bezug = [Vec2::new(0.0, 0.0)];
    // 1° neben der Waagerechten, 20 m entfernt: seitlich 349 mm.
    let roh = Vec2::polar(1f64.to_radians(), 20_000.0);
    let (p, quelle) = fang(&d, roh, 12.0, &bezug);
    assert_eq!(quelle, SnapSource::Free, "Kegel statt Band: {p:?}");
    assert!(p.dist(roh) < 1e-9, "Punkt bleibt am Zeiger: {p:?}");
    // Derselbe Winkel nah am Bezug (seitlich 1,7 mm) rastet ein.
    let nah = Vec2::polar(1f64.to_radians(), 100.0);
    let (p, quelle) = fang(&d, nah, 12.0, &bezug);
    assert_eq!(quelle, SnapSource::Polar, "Band muss greifen: {p:?}");
    assert!(p.y.abs() < 1e-9, "nicht auf der Waagerechten: {p:?}");
}
