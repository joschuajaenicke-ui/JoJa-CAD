//! Lotfang (T0216) — das Gegenstück zum Tangentialfang für Linien.
//!
//! Liegt der Bezugspunkt auf einer vorhandenen Linie, gibt es durch ihn genau
//! **eine** Senkrechte auf dieser Linie. Der Zeiger rastet auf ihr ein, die
//! neue (bzw. gezogene) Linie steht damit rechtwinklig auf der vorhandenen.
//!
//! Geprüft wird die **Geometrie**, nicht das Bild: das Skalarprodukt der beiden
//! Richtungen muss 0 sein (90°) und der Fangpunkt auf der Senkrechten durch den
//! Bezugspunkt liegen. Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{dist_point_segment, v, Vec2};
use jjcad_core::model::Id;
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_candidates, snap_constrained, SnapKind, SnapResolution, SnapSource};
use serde_json::json;

/// Sitzung in Millimetern ohne Raster, Ortho und Polar; die übrigen Fänge aus,
/// damit nur der Lotfang über den Punkt entscheiden kann.
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
            "tangent": false,
            "lot": true
        }}),
    )
    .unwrap();
    s
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

/// Der Kernfall: Bezugspunkt auf der Linie, Zeiger neben der Senkrechten. Der
/// gefangene Punkt liegt exakt auf der Senkrechten — die neue Linie steht damit
/// rechtwinklig auf der vorhandenen.
#[test]
fn linie_steht_lotrecht_auf_der_vorhandenen() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));

    // Bezugspunkt (2000; 0) liegt auf der Linie, Zeiger 5 mm neben der
    // Senkrechten x = 2000.
    let res = fang(&s, (2005.0, 800.0), Some((2000.0, 0.0)), &[]);
    assert_eq!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "kein Lotfang: {res:?}"
    );
    assert!(
        (res.point.x - 2000.0).abs() < 1e-6,
        "Fangpunkt nicht auf der Senkrechten: {:?}",
        res.point
    );
    assert!(
        (res.point.y - 800.0).abs() < 1e-6,
        "Fangpunkt nicht auf der Höhe des Zeigers: {:?}",
        res.point
    );
    // Rechtwinklig: die Richtung Bezugspunkt → Fangpunkt steht senkrecht auf
    // der Richtung der vorhandenen Linie.
    let basis = Vec2::new(5000.0, 0.0) - Vec2::new(0.0, 0.0);
    let neu = res.point - Vec2::new(2000.0, 0.0);
    assert!(
        basis.dot(neu).abs() < 1e-6,
        "Linie steht nicht rechtwinklig: {}",
        basis.dot(neu)
    );
}

/// Auch auf einer schrägen Linie ist der Winkel 90° — gerechnet aus ihrer
/// Richtung, nicht aus der Ansicht.
#[test]
fn schraege_linie_steht_ebenso_lotrecht() {
    let mut s = s();
    // 45°-Linie durch den Ursprung.
    linie(&mut s, (0.0, 0.0), (3000.0, 3000.0));
    let r0 = Vec2::new(1000.0, 1000.0);
    // Senkrechte auf 45° verläuft unter −45°: Punkt (1500; 500).
    let res = fang(&s, (1505.0, 495.0), Some((1000.0, 1000.0)), &[]);
    assert_eq!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "kein Lotfang auf der schrägen Linie: {res:?}"
    );
    let basis = Vec2::new(3000.0, 3000.0);
    let neu = res.point - r0;
    assert!(
        basis.dot(neu).abs() < 1e-6,
        "schräge Linie steht nicht rechtwinklig: {}",
        basis.dot(neu)
    );
}

/// Außerhalb des Fangradius bleibt der Zeiger frei.
#[test]
fn lotpunkt_nur_in_reichweite() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let res = fang(&s, (2200.0, 800.0), Some((2000.0, 0.0)), &[]);
    assert_ne!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Lotfang greift zu weit: {res:?}"
    );
}

/// Liegt der Bezugspunkt **nicht** auf der Linie, gibt es dort keinen rechten
/// Winkel — der Fang darf nicht raten.
#[test]
fn bezugspunkt_neben_der_linie_faengt_nicht() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let res = fang(&s, (2005.0, 800.0), Some((2000.0, 300.0)), &[]);
    assert_ne!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Lotfang an einem Punkt neben der Linie: {res:?}"
    );
}

/// Jenseits des Linienendes steht nichts mehr senkrecht — die Verlängerung
/// zählt nicht.
#[test]
fn bezugspunkt_auf_der_verlaengerung_faengt_nicht() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let res = fang(&s, (6005.0, 800.0), Some((6000.0, 0.0)), &[]);
    assert_ne!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Lotfang auf der Verlängerung: {res:?}"
    );
}

/// Beim **Manipulieren** ist der Bezugspunkt das stehengebliebene Ende der
/// gezogenen Linie; die Linie selbst steht in `ignore` und fängt nicht auf
/// sich selbst.
#[test]
fn beim_griffzug_bleibt_die_linie_lotrecht() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    // Gezogene Linie: festes Ende (1000; 0) auf der Basislinie, bewegtes Ende
    // irgendwo.
    let gezogen = linie(&mut s, (1000.0, 0.0), (1000.0, 900.0));

    let res = fang(&s, (1005.0, 1500.0), Some((1000.0, 0.0)), &[gezogen]);
    assert_eq!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "kein Lotfang beim Griffzug: {res:?}"
    );
    assert!(
        (res.point.x - 1000.0).abs() < 1e-6,
        "gezogenes Ende nicht auf der Senkrechten: {:?}",
        res.point
    );
}

/// Die Markenstelle ist der **Schnittpunkt** beider Linien (der Bezugspunkt) —
/// der Fangpunkt wandert mit dem Zeiger, das Zeichen sitzt aber am rechten
/// Winkel.
#[test]
fn marke_steht_im_schnittpunkt_beider_linien() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let res = fang(&s, (2005.0, 800.0), Some((2000.0, 0.0)), &[]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Lot), "{res:?}");
    let marke = res.marke.expect("Lotfang ohne Markenstelle");
    assert!(
        marke.dist(Vec2::new(2000.0, 0.0)) < 1e-6,
        "Marke steht nicht im Schnittpunkt: {marke:?}"
    );
    assert!(
        marke.dist(res.point) > 700.0,
        "Marke und Fangpunkt fallen zusammen: {:?}",
        res.point
    );
}

/// Der Lotfang ist abschaltbar (Einstellungen → Fangen → Lotfang).
#[test]
fn lotfang_ist_abschaltbar() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    s.execute("doc.settings", &json!({ "snap": { "lot": false } }))
        .unwrap();
    let res = fang(&s, (2005.0, 800.0), Some((2000.0, 0.0)), &[]);
    assert_ne!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Lotfang trotz Abschaltung: {res:?}"
    );
}

/// Eine Wand ist von der Achse her eine Linie: die neue Linie steht
/// rechtwinklig auf ihr.
#[test]
fn wandachse_zaehlt_wie_eine_linie() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({ "a": [0.0, 0.0], "b": [6000.0, 0.0], "thickness": 240.0 }),
    )
    .unwrap();
    let res = fang(&s, (3005.0, 1000.0), Some((3000.0, 0.0)), &[]);
    assert_eq!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Wandachse fängt nicht lotrecht: {res:?}"
    );
    assert!(
        (res.point.x - 3000.0).abs() < 1e-6,
        "Fangpunkt nicht auf der Senkrechten: {:?}",
        res.point
    );
}

/// Die **Umgrenzung** einer Wand zählt wie eine Linie (T0216, Turn 3): der
/// Bezugspunkt liegt auf der Längskante, 120 mm neben der Achse. Die neue Linie
/// steht rechtwinklig auf der Wand.
#[test]
fn wandkante_faengt_lotrecht() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({ "a": [0.0, 0.0], "b": [6000.0, 0.0], "thickness": 240.0 }),
    )
    .unwrap();
    // Längskante bei y = +120 (Bezugspunkt liegt **nicht** auf der Achse).
    let res = fang(&s, (3005.0, 900.0), Some((3000.0, 120.0)), &[]);
    assert_eq!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Wandkante fängt nicht lotrecht: {res:?}"
    );
    assert!(
        (res.point.x - 3000.0).abs() < 1e-6,
        "Fangpunkt nicht auf der Senkrechten zur Kante: {:?}",
        res.point
    );
    let neu = res.point - Vec2::new(3000.0, 120.0);
    assert!(
        Vec2::new(6000.0, 0.0).dot(neu).abs() < 1e-6,
        "Linie steht nicht rechtwinklig zur Wandkante"
    );
}

/// An der **Stirnseite** steht die neue Linie parallel zur Wand — ebenfalls
/// rechtwinklig auf der umgrenzenden Kante.
#[test]
fn wandstirnseite_steht_parallel_zur_wand() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({ "a": [0.0, 0.0], "b": [6000.0, 0.0], "thickness": 240.0 }),
    )
    .unwrap();
    // Stirnkante bei x = 0, Bezugspunkt bei (0; 60).
    let res = fang(&s, (800.0, 65.0), Some((0.0, 60.0)), &[]);
    assert_eq!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Stirnseite fängt nicht lotrecht: {res:?}"
    );
    assert!(
        (res.point.y - 60.0).abs() < 1e-6,
        "Fangpunkt nicht auf der Waagerechten zur Stirnkante: {:?}",
        res.point
    );
    let neu = res.point - Vec2::new(0.0, 60.0);
    assert!(
        neu.y.abs() < 1e-6 && neu.x > 1.0,
        "die neue Linie läuft nicht parallel zur Wand: {neu:?}"
    );
}

/// Ohne den Schalter „Wandkanten“ bleibt es bei der Achse — ein Punkt auf der
/// Umgrenzung ist dann kein Ausgangspunkt für den Lotfang.
#[test]
fn wandkante_faengt_nur_mit_wandkanten() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({ "a": [0.0, 0.0], "b": [6000.0, 0.0], "thickness": 240.0 }),
    )
    .unwrap();
    s.execute("doc.settings", &json!({ "snap": { "wall_edges": false } }))
        .unwrap();
    let res = fang(&s, (3005.0, 900.0), Some((3000.0, 120.0)), &[]);
    assert_ne!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Lotfang auf der Wandkante trotz abgeschalteter Wandkanten: {res:?}"
    );
}

/// Ein Kreis ist keine Linie: seine Zerlegung besteht aus kurzen Sehnen, deren
/// Senkrechte nichts mit dem Zeiger zu tun hat. Liegt der Bezugspunkt auf der
/// Bahn, darf daraus kein Lotfang werden.
#[test]
fn kreisbahn_ist_keine_linie() {
    let mut s = s();
    s.execute(
        "draw.circle",
        &json!({ "c": [0.0, 0.0], "r": 1000.0, "layer": "Hilfslinien" }),
    )
    .unwrap();
    let res = fang(&s, (1005.0, 900.0), Some((1000.0, 0.0)), &[]);
    assert_ne!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "Lotfang auf der Kreisbahn: {res:?}"
    );
}

/// Der Lotfang steht als Kandidat in derselben Liste wie jeder andere Fang —
/// die Zeichenfläche bekommt ihn über denselben Weg.
#[test]
fn lotpunkt_steht_in_der_kandidatenliste() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let cands = snap_candidates(
        &s.doc,
        Vec2::new(2005.0, 800.0),
        20.0,
        Some(Vec2::new(2000.0, 0.0)),
        &[],
    );
    let lot = cands
        .iter()
        .find(|h| h.kind == SnapKind::Lot)
        .unwrap_or_else(|| panic!("Lotfang fehlt in der Kandidatenliste: {cands:?}"));
    // Der Bezugspunkt liegt auf der Basislinie — die Voraussetzung des Fangs.
    assert!(
        dist_point_segment(
            Vec2::new(2000.0, 0.0),
            v(0.0, 0.0),
            v(5000.0, 0.0)
        ) < 1e-6
    );
    assert!((lot.point.x - 2000.0).abs() < 1e-6, "{:?}", lot.point);
}

// ---------------------------------------------------------------------------
// Kombination Lotrecht × „Nächster Punkt" (T0243)
// ---------------------------------------------------------------------------

/// Sitzung wie [`s`], aber mit eingeschaltetem „Nächster Punkt" — nur dann darf
/// die Senkrechte eine andere Linie kreuzen und dort fangen.
fn s_mit_naechstem() -> Session {
    let mut s = s();
    s.execute("doc.settings", &json!({ "snap": { "nearest": true } }))
        .unwrap();
    s
}

/// Der Fall aus dem Ticket: die neue Linie steht lotrecht auf der bestehenden
/// **und** endet auf der anderen Linie. Beide Fänge einzeln liefern je nur eine
/// der beiden Bedingungen — gefangen werden muss der Schnittpunkt.
#[test]
fn lot_und_naechster_punkt_fangen_den_schnittpunkt() {
    let mut s = s_mit_naechstem();
    // Quelle des Lots: waagerechte Linie durch den Ursprung.
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    // Die andere Linie, auf der die neue enden soll: waagerecht bei y = 1500.
    linie(&mut s, (1000.0, 1500.0), (4000.0, 1500.0));

    // Bezugspunkt (2000; 0) auf der Quellinie; Zeiger 5 mm über der
    // Kreuzung (2000; 1500) und 4 mm neben der Senkrechten.
    let res = fang(&s, (2004.0, 1495.0), Some((2000.0, 0.0)), &[]);
    assert_eq!(
        res.source,
        SnapSource::Object(SnapKind::Lot),
        "kein Lotfang an der Kreuzung: {res:?}"
    );
    assert!(
        res.point.dist(Vec2::new(2000.0, 1500.0)) < 1e-6,
        "Fangpunkt ist nicht die Kreuzung: {:?}",
        res.point
    );
    // Beide Bedingungen einzeln nachgerechnet.
    let neu = res.point - Vec2::new(2000.0, 0.0);
    assert!(
        Vec2::new(5000.0, 0.0).dot(neu).abs() < 1e-6,
        "Linie steht nicht rechtwinklig: {:?}",
        res.point
    );
    assert!(
        (res.point.y - 1500.0).abs() < 1e-6,
        "Fangpunkt liegt nicht auf der anderen Linie: {:?}",
        res.point
    );
}

/// Die Marke bleibt der rechte Winkel am Bezugspunkt — die Kreuzung ist der
/// Fangpunkt, nicht die Stelle des Zeichens.
#[test]
fn kreuzung_meldet_die_marke_am_bezugspunkt() {
    let mut s = s_mit_naechstem();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    linie(&mut s, (1000.0, 1500.0), (4000.0, 1500.0));
    let res = fang(&s, (2004.0, 1495.0), Some((2000.0, 0.0)), &[]);
    let marke = res.marke.expect("Lotfang ohne Markenstelle");
    assert!(
        marke.dist(Vec2::new(2000.0, 0.0)) < 1e-6,
        "Marke steht nicht im Bezugspunkt: {marke:?}"
    );
}

/// Auch auf einer **schrägen** anderen Linie wird exakt geschnitten — der
/// Punkt wird gerechnet, nicht auf die nähere Achse gerundet.
#[test]
fn schraege_andere_linie_wird_exakt_geschnitten() {
    let mut s = s_mit_naechstem();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    // 45°-Linie durch (1000; 1000).
    linie(&mut s, (1000.0, 1000.0), (4000.0, 4000.0));
    // Senkrechte durch (2000; 0) ist x = 2000; sie trifft die 45°-Linie bei
    // (2000; 2000).
    let res = fang(&s, (2006.0, 1993.0), Some((2000.0, 0.0)), &[]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Lot), "{res:?}");
    assert!(
        res.point.dist(Vec2::new(2000.0, 2000.0)) < 1e-6,
        "Kreuzung mit der schrägen Linie nicht getroffen: {:?}",
        res.point
    );
}

/// Ohne „Nächster Punkt" bleibt es beim Fußpunkt des Zeigers auf der
/// Senkrechten — die Kombination hängt am eingeschalteten Fang.
#[test]
fn ohne_naechsten_punkt_bleibt_der_fusspunkt() {
    let mut s = s();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    linie(&mut s, (1000.0, 1500.0), (4000.0, 1500.0));
    let res = fang(&s, (2004.0, 1495.0), Some((2000.0, 0.0)), &[]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Lot), "{res:?}");
    assert!(
        (res.point.x - 2000.0).abs() < 1e-6 && (res.point.y - 1495.0).abs() < 1e-6,
        "Fangpunkt ist nicht der Fußpunkt: {:?}",
        res.point
    );
}

/// Liegt die Kreuzung außerhalb des Fangradius, gilt weiter der Fußpunkt —
/// die andere Linie darf den Punkt nicht über den Radius hinaus ziehen.
#[test]
fn kreuzung_ausserhalb_der_reichweite_bleibt_beim_fusspunkt() {
    let mut s = s_mit_naechstem();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    linie(&mut s, (1000.0, 1500.0), (4000.0, 1500.0));
    // Zeiger 3 mm neben der Senkrechten, aber 30 mm unter der anderen Linie —
    // die Kreuzung liegt damit außerhalb von 20 mm.
    let res = fang(&s, (2003.0, 1595.0), Some((2000.0, 0.0)), &[]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Lot), "{res:?}");
    assert!(
        (res.point.x - 2000.0).abs() < 1e-6 && (res.point.y - 1595.0).abs() < 1e-6,
        "Kreuzung außerhalb des Radius wurde trotzdem gefangen: {:?}",
        res.point
    );
}

/// Die Quellinie selbst ist keine „andere Linie": ihr Schnitt mit der
/// Senkrechten ist der Bezugspunkt und damit kein Fangpunkt mit Länge.
#[test]
fn eigene_quelle_ist_keine_kreuzung() {
    let mut s = s_mit_naechstem();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    let res = fang(&s, (2005.0, 800.0), Some((2000.0, 0.0)), &[]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Lot), "{res:?}");
    assert!(
        (res.point.x - 2000.0).abs() < 1e-6 && (res.point.y - 800.0).abs() < 1e-6,
        "eigene Linie wurde als Kreuzung gefangen: {:?}",
        res.point
    );
}

/// Der Lotfang auf einer **Wandachse** kreuzt ebenso eine Linie: der Punkt
/// liegt auf der Senkrechten zur Wand und auf der Linie.
#[test]
fn wandachse_kreuzt_eine_linie() {
    let mut s = s_mit_naechstem();
    s.execute(
        "draw.wall",
        &json!({ "a": [0.0, 0.0], "b": [6000.0, 0.0], "thickness": 240.0 }),
    )
    .unwrap();
    linie(&mut s, (1000.0, 2500.0), (4000.0, 2500.0));
    let res = fang(&s, (3004.0, 2496.0), Some((3000.0, 0.0)), &[]);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Lot), "{res:?}");
    assert!(
        res.point.dist(Vec2::new(3000.0, 2500.0)) < 1e-6,
        "Kreuzung mit der Wandachse nicht getroffen: {:?}",
        res.point
    );
}

/// Die Kreuzung steht als Kandidat in derselben Liste wie jeder andere Fang —
/// die Zeichenfläche bekommt sie über denselben Weg.
#[test]
fn kreuzung_steht_in_der_kandidatenliste() {
    let mut s = s_mit_naechstem();
    linie(&mut s, (0.0, 0.0), (5000.0, 0.0));
    linie(&mut s, (1000.0, 1500.0), (4000.0, 1500.0));
    let cands = snap_candidates(
        &s.doc,
        Vec2::new(2004.0, 1495.0),
        20.0,
        Some(Vec2::new(2000.0, 0.0)),
        &[],
    );
    assert!(
        cands
            .iter()
            .any(|h| h.kind == SnapKind::Lot && h.point.dist(Vec2::new(2000.0, 1500.0)) < 1e-6),
        "Kreuzung fehlt in der Kandidatenliste: {cands:?}"
    );
}
