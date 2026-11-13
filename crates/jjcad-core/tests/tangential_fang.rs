//! Tangentialfang (T0157).
//!
//! Zwei Lagen aus der Aufgabenstellung:
//!
//! * **Linie an Kreis/Bogen** — der Anfang der Linie liegt auf der Kreisbahn.
//!   Durch diesen Punkt gibt es genau eine Tangente (die Senkrechte zum
//!   Radius); der zweite Punkt rastet auf ihr ein.
//! * **Kreis/Bogen an Linie** — ein bewegter Kreis rastet mit seinem
//!   Mittelpunkt auf die Parallele der Linie im Abstand `r`.
//!
//! Geprüft wird die **Geometrie**, nicht das Bild: der Abstand vom Kreismittel-
//! punkt zur Geraden muss genau dem Radius entsprechen — das ist die Definition
//! der Tangente. Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{dist_point_line, Vec2};
use jjcad_core::model::Id;
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained, SnapKind, SnapResolution};
use serde_json::json;

/// Sitzung in Millimetern ohne Raster, Ortho, Polar und ohne die
/// kontinuierlichen Fänge, die den Tangentialpunkt überdecken könnten.
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
            "endpoint": false,
            "center": false,
            "tangent": true
        }}),
    )
    .unwrap();
    s
}

fn kreis(s: &mut Session, c: (f64, f64), r: f64) -> Id {
    let a = s
        .execute(
            "draw.circle",
            &json!({ "c": [c.0, c.1], "r": r, "layer": "Hilfslinien" }),
        )
        .unwrap();
    a["created"][0].as_u64().unwrap() as Id
}

fn linie(s: &mut Session, a: (f64, f64), b: (f64, f64)) -> Id {
    let r = s
        .execute(
            "draw.line",
            &json!({ "a": [a.0, a.1], "b": [b.0, b.1], "layer": "Hilfslinien" }),
        )
        .unwrap();
    r["created"][0].as_u64().unwrap() as Id
}

fn fang(
    s: &Session,
    cursor: (f64, f64),
    reference: Option<(f64, f64)>,
    ignore: &[Id],
) -> SnapResolution {
    snap_constrained(
        &s.doc,
        Vec2::new(cursor.0, cursor.1),
        20.0,
        reference.map(|p| Vec2::new(p.0, p.1)),
        ignore,
        false,
    )
}

/// Der Kernfall: Punkt auf der Kreisbahn, Zeiger neben der Tangente. Der
/// gefangene Punkt liegt auf der Senkrechten zum Radius — die Linie vom
/// Berührpunkt dorthin berührt den Kreis in genau einem Punkt.
#[test]
fn linie_verlaesst_den_kreis_tangential() {
    let mut s = s();
    let c = Vec2::new(0.0, 0.0);
    let r = 1000.0;
    kreis(&mut s, (0.0, 0.0), r);

    // Berührpunkt auf der Bahn (rechter Quadrant), Zeiger 5 mm neben der
    // senkrechten Tangente x = 1000.
    let beruehr = Vec2::new(1000.0, 0.0);
    let res = fang(&s, (1005.0, 800.0), Some((1000.0, 0.0)), &[]);
    assert_eq!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "kein Tangentialfang: {:?}",
        res
    );
    // Der Punkt liegt exakt auf der Tangente (x = 1000), nicht auf dem Zeiger.
    assert!(
        (res.point.x - 1000.0).abs() < 1e-6,
        "Punkt nicht auf der Tangente: {:?}",
        res.point
    );
    assert!(
        (res.point.y - 800.0).abs() < 1e-6,
        "Punkt nicht auf der Höhe des Zeigers: {:?}",
        res.point
    );
    // Definition der Tangente: Abstand Mittelpunkt → Gerade = Radius.
    let d = dist_point_line(c, beruehr, res.point);
    assert!(
        (d - r).abs() < 1e-6,
        "Linie ist nicht tangential: Abstand {d} statt {r}"
    );
}

/// Außerhalb des Fangradius bleibt der Zeiger frei — der Fang zieht nicht
/// überall auf die Tangente.
#[test]
fn tangentialpunkt_nur_in_reichweite() {
    let mut s = s();
    kreis(&mut s, (0.0, 0.0), 1000.0);
    let res = fang(&s, (1200.0, 800.0), Some((1000.0, 0.0)), &[]);
    assert_ne!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "Tangentialfang greift zu weit: {:?}",
        res
    );
}

/// Liegt der Bezugspunkt **nicht** auf der Bahn, gibt es keine Tangente durch
/// ihn — der Fang darf nicht raten.
#[test]
fn bezugspunkt_ausserhalb_der_bahn_faengt_nicht() {
    let mut s = s();
    kreis(&mut s, (0.0, 0.0), 1000.0);
    // 300 mm neben der Bahn: keine Tangente durch diesen Punkt.
    let res = fang(&s, (1005.0, 800.0), Some((1300.0, 0.0)), &[]);
    assert_ne!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "Tangentialfang an einem Punkt neben der Bahn: {:?}",
        res
    );
}

/// Ein bewegter Kreis rastet mit seinem Mittelpunkt auf die Parallele der
/// Linie im Abstand `r` — dort berührt er die Linie.
#[test]
fn bewegter_kreis_rastet_an_der_linie() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let k = kreis(&mut s, (2000.0, 1500.0), 500.0);

    // Zeiger 20 mm unter der gesuchten Mittellage y = 500.
    let res = fang(&s, (2000.0, 480.0), None, &[k]);
    assert_eq!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "bewegter Kreis rastet nicht: {:?}",
        res
    );
    assert!(
        (res.point.y - 500.0).abs() < 1e-6,
        "Mittelpunkt nicht im Abstand r: {:?}",
        res.point
    );
    assert!(
        (res.point.x - 2000.0).abs() < 1e-6,
        "Mittelpunkt seitlich verschoben: {:?}",
        res.point
    );
    // Der Kreis berührt die Linie: Abstand Mittelpunkt → Gerade = Radius.
    let d = dist_point_line(res.point, Vec2::ZERO, Vec2::new(5000.0, 0.0));
    assert!((d - 500.0).abs() < 1e-6, "Kreis berührt nicht: {d}");
}

/// Jenseits des Linienendes gibt es keine Berührung — der Fang darf nicht auf
/// die Verlängerung ziehen.
#[test]
fn bewegter_kreis_rastet_nicht_an_der_verlaengerung() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let k = kreis(&mut s, (2000.0, 1500.0), 500.0);
    let res = fang(&s, (6000.0, 480.0), None, &[k]);
    assert_ne!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "Tangentialfang auf der Verlängerung: {:?}",
        res
    );
}

/// Erzeugen eines Kreises: der Radiuspunkt rastet auf das Lot vom Mittelpunkt
/// auf die Linie — der entstehende Kreis liegt damit tangential. Das ist der
/// bestehende Lot-Fang; der Test hält fest, dass die Zusage aus T0157 auch für
/// das **Erzeugen** gilt.
#[test]
fn kreisradius_rastet_auf_das_lot_zur_linie() {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "perpendicular": true } }),
    )
    .unwrap();
    linie(&mut s, (-2000.0, 1000.0), (2000.0, 1000.0));

    // Mittelpunkt (0,0), Zeiger 5 mm über der Linie.
    let res = fang(&s, (0.0, 1005.0), Some((0.0, 0.0)), &[]);
    assert_eq!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Perpendicular),
        "Radiuspunkt rastet nicht auf das Lot: {:?}",
        res
    );
    assert!(
        (res.point.y - 1000.0).abs() < 1e-6,
        "Radiuspunkt nicht auf der Linie: {:?}",
        res.point
    );
    let r = res.point.dist(Vec2::ZERO);
    let d = dist_point_line(Vec2::ZERO, Vec2::new(-2000.0, 1000.0), Vec2::new(2000.0, 1000.0));
    assert!((r - d).abs() < 1e-6, "Radius {r} ≠ Abstand {d}");
}

/// Der Fang ist abschaltbar (Einstellungen → Fangen → Tangential).
#[test]
fn tangentialfang_ist_abschaltbar() {
    let mut s = s();
    kreis(&mut s, (0.0, 0.0), 1000.0);
    s.execute("doc.settings", &json!({ "snap": { "tangent": false } }))
        .unwrap();
    let res = fang(&s, (1005.0, 800.0), Some((1000.0, 0.0)), &[]);
    assert_ne!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "Tangentialfang trotz Abschaltung: {:?}",
        res
    );
}

/// Der Bogen zählt wie der Kreis — aber nur innerhalb seines Winkelbereichs.
#[test]
fn bogen_faengt_tangential_nur_im_winkelbereich() {
    let mut s = s();
    // Viertelbogen von 0° bis 90° um den Ursprung.
    s.execute(
        "draw.arc",
        &json!({ "c": [0.0, 0.0], "r": 1000.0, "a0": 0.0, "a1": 90.0, "layer": "Hilfslinien" }),
    )
    .unwrap();

    // Berührpunkt (1000, 0) liegt auf dem Bogen (0°).
    let res = fang(&s, (1005.0, 800.0), Some((1000.0, 0.0)), &[]);
    assert_eq!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "Bogen fängt nicht tangential: {:?}",
        res
    );

    // Berührpunkt (0, -1000) liegt auf dem Vollkreis, aber nicht auf dem Bogen.
    let res = fang(&s, (5.0, -1800.0), Some((0.0, -1000.0)), &[]);
    assert_ne!(
        res.source,
        jjcad_core::snap::SnapSource::Object(SnapKind::Tangent),
        "Bogen fängt außerhalb seines Winkelbereichs: {:?}",
        res
    );
}
