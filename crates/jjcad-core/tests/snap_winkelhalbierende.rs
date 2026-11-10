//! Winkelhalbierender Fang (T0150).
//!
//! Der Fang bietet die **Winkelhalbierende** an, wenn der feste Punkt einer
//! neuen Linie auf dem **Schnittpunkt zweier Linien** liegt — also im Scheitel
//! einer Ecke, Kreuzung oder eines Stoßes. Nur dort gibt es einen Winkel zu
//! halbieren.
//!
//! Der erste Stand war falsch: er bot die Halbierende überall an, wo zwei
//! Linien in der Nähe waren („Punkt zwischen zwei Linien"). Diese Tests halten
//! beide Richtungen fest — sie greift am Scheitel und sie schweigt daneben.
//!
//! Geprüft wird über `snap::winkelhalbierende` (die Geometrie), über
//! `snap::snap_constrained_achsen` (den Weg der Zeichenfläche) und über
//! `snap::hilfslinien_gesamt` (die gezeichneten Führungslinien). Alle
//! Koordinaten in Millimetern.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::session::Session;
use jjcad_core::snap::{
    hilfslinien_gesamt, snap_constrained_achsen, HaltPunkt, SnapKind, SnapSource,
};
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "grid": false, "ortho": false, "polar": false } }),
    )
    .unwrap();
    s
}

fn linie(s: &mut Session, a: [f64; 2], b: [f64; 2]) {
    s.execute("draw.line", &json!({ "a": a, "b": b })).unwrap();
}

/// Fang wie in der Zeichenfläche: Bezugspunkte, Haltepunkte, kein Raster.
fn fang(
    s: &Session,
    cursor: Vec2,
    tol: f64,
    achsen: &[Vec2],
    halte: &[HaltPunkt],
) -> jjcad_core::snap::SnapResolution {
    snap_constrained_achsen(
        &s.doc, cursor, tol, achsen, &[], &[], &[], &[], halte, false,
    )
}

fn assert_punkt(ist: Vec2, soll: Vec2, was: &str) {
    assert!(ist.dist(soll) < 1e-6, "{was}: erwartet {soll:?}, ist {ist:?}");
}

/// Rechter Winkel im Ursprung: zwei Schenkel, die sich in (0;0) treffen.
fn ecke() -> Session {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [0.0, 0.0], [0.0, 4000.0]);
    s
}

fn halt(p: Vec2) -> HaltPunkt {
    HaltPunkt {
        point: p,
        kind: SnapKind::Endpoint,
        entity: None,
        richtung: None,
    }
}

// ---------------------------------------------------------------------------
// Die Bedingung: nur im Scheitel zweier Linien
// ---------------------------------------------------------------------------

/// Im Scheitel einer Ecke liegt die Halbierende unter 45° — und sie geht vom
/// Scheitel aus, nicht von irgendeinem Punkt im Winkel.
#[test]
fn halbierende_in_der_ecke_zeigt_45_grad() {
    let s = ecke();
    let linien = jjcad_core::snap::winkelhalbierende(&s.doc, v(0.0, 0.0), &[]);
    assert_eq!(linien.len(), 1, "{linien:?}");
    assert_eq!(linien[0].kind, SnapKind::Bisector);
    assert_punkt(linien[0].origin, v(0.0, 0.0), "Ursprung ist der Scheitel");
    let grad = linien[0].dir.angle().to_degrees();
    assert!((grad - 45.0).abs() < 1e-6, "Winkel {grad}");
}

/// **Der Fehler des ersten Stands**: ein Punkt *zwischen* zwei Linien ist kein
/// Scheitel — dort darf keine Halbierende entstehen.
#[test]
fn punkt_zwischen_zwei_linien_liefert_keine_halbierende() {
    let s = ecke();
    // Mitten im Winkel, 1 m von beiden Schenkeln entfernt.
    assert!(
        jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 1000.0), &[]).is_empty(),
        "zwischen den Linien gibt es keinen Winkel zu halbieren"
    );
    // Gegenprobe über die Fang-Kette: der Zeiger bleibt frei.
    let r = fang(&s, v(1200.0, 1190.0), 30.0, &[v(1000.0, 1000.0)], &[]);
    assert_ne!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_punkt(r.point, v(1200.0, 1190.0), "roher Zeiger");
}

/// Nur **eine** Linie am Punkt: kein Scheitel, keine Halbierende.
#[test]
fn eine_einzelne_linie_am_punkt_liefert_keine_halbierende() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(0.0, 0.0), &[]).is_empty());
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(2000.0, 0.0), &[]).is_empty());
}

/// Neben dem Scheitel (20 mm daneben) greift der Fang nicht — der Punkt muss
/// **auf** dem Schnittpunkt liegen.
#[test]
fn neben_dem_scheitel_keine_halbierende() {
    let s = ecke();
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(20.0, 0.0), &[]).is_empty());
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(0.0, 20.0), &[]).is_empty());
}

/// Zwei parallele Linien schneiden sich nirgends — zwischen ihnen gibt es
/// keine Halbierende (das war die alte, zu weite Regel).
#[test]
fn parallele_linien_liefern_keine_halbierende() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [0.0, 2000.0], [4000.0, 2000.0]);
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 1000.0), &[]).is_empty());
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 0.0), &[]).is_empty());
}

/// Zwei fluchtende Strecken (eine gerade Linie aus zwei Teilen) bilden keinen
/// Winkel — gestreckt, also keine Halbierende.
#[test]
fn fluchtende_strecken_liefern_keine_halbierende() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [2000.0, 0.0]);
    linie(&mut s, [2000.0, 0.0], [4000.0, 0.0]);
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(2000.0, 0.0), &[]).is_empty());
}

// ---------------------------------------------------------------------------
// Welche Halbierenden am Scheitel entstehen
// ---------------------------------------------------------------------------

/// Kreuzung mitten in beiden Linien: es gibt zwei Winkelpaare, also **zwei**
/// Halbierende (senkrecht zueinander).
#[test]
fn kreuzung_liefert_zwei_halbierende() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [2000.0, -2000.0], [2000.0, 2000.0]);
    let linien = jjcad_core::snap::winkelhalbierende(&s.doc, v(2000.0, 0.0), &[]);
    assert_eq!(linien.len(), 2, "{linien:?}");
    let winkel: Vec<f64> = linien
        .iter()
        .map(|l| l.dir.angle().to_degrees().rem_euclid(180.0).round())
        .collect();
    assert!(
        winkel.contains(&45.0) && winkel.contains(&135.0),
        "{winkel:?}"
    );
}

/// Stoß: eine Linie endet auf einer anderen — auch dort zwei Winkelpaare.
#[test]
fn stoss_liefert_zwei_halbierende() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [2000.0, 0.0], [2000.0, 2000.0]);
    let linien = jjcad_core::snap::winkelhalbierende(&s.doc, v(2000.0, 0.0), &[]);
    assert_eq!(linien.len(), 2, "{linien:?}");
}

/// Drei Linien in einem Punkt: dieselbe Halbierende wird nur einmal geliefert.
#[test]
fn drei_linien_am_punkt_ohne_doppelte() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [0.0, 0.0], [0.0, 4000.0]);
    linie(&mut s, [0.0, 0.0], [3000.0, 3000.0]);
    let linien = jjcad_core::snap::winkelhalbierende(&s.doc, v(0.0, 0.0), &[]);
    for (i, a) in linien.iter().enumerate() {
        for b in linien.iter().skip(i + 1) {
            assert!(
                a.dir.cross(b.dir).abs() > 1e-6,
                "doppelte Halbierende: {linien:?}"
            );
        }
    }
}

/// Wände zählen mit ihrer **Achse**: zwei Wände in einer Ecke liefern die
/// Halbierende des Innenwinkels.
#[test]
fn waende_zaehlen_mit_ihrer_achse() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({ "a": [0.0, 0.0], "b": [4000.0, 0.0], "thickness": 240.0 }),
    )
    .unwrap();
    s.execute(
        "draw.wall",
        &json!({ "a": [0.0, 0.0], "b": [0.0, 4000.0], "thickness": 240.0 }),
    )
    .unwrap();
    let linien = jjcad_core::snap::winkelhalbierende(&s.doc, v(0.0, 0.0), &[]);
    assert_eq!(linien.len(), 1, "{linien:?}");
    let grad = linien[0].dir.angle().to_degrees();
    assert!((grad - 45.0).abs() < 1e-6, "Winkel {grad}");
}

/// „Zwei Linien" heißt zwei Linien-Objekte: Rechteck und Polylinie sind keine
/// Linien und liefern keine Halbierende.
#[test]
fn rechteck_und_polylinie_liefern_keine_halbierende() {
    let mut s = s();
    s.execute(
        "draw.rect",
        &json!({ "a": [0.0, 0.0], "b": [2000.0, 2000.0] }),
    )
    .unwrap();
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(0.0, 0.0), &[]).is_empty());

    let mut poly = self::s();
    poly.execute(
        "draw.polyline",
        &json!({ "pts": [[0.0, 0.0], [4000.0, 0.0], [0.0, 4000.0]] }),
    )
    .unwrap();
    assert!(jjcad_core::snap::winkelhalbierende(&poly.doc, v(0.0, 0.0), &[]).is_empty());
}

/// Ein bewegtes Element (im Fang ausgeschlossen) bildet keinen Scheitel.
#[test]
fn ausgeschlossenes_element_liefert_keine_halbierende() {
    let s = ecke();
    let id = s.doc.entities[0].id;
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(0.0, 0.0), &[id]).is_empty());
}

// ---------------------------------------------------------------------------
// Der Fang selbst
// ---------------------------------------------------------------------------

/// Der Kern: die neue Linie beginnt im Scheitel — der zweite Punkt rastet auf
/// der Halbierenden ein, die Linie läuft also genau zwischen beiden hindurch.
#[test]
fn linie_beginnt_im_scheitel_und_folgt_der_halbierenden() {
    let s = ecke();
    let start = v(0.0, 0.0);
    // Zeiger 10 mm neben der 45°-Halbierenden (x − y = 10).
    let r = fang(&s, v(1200.0, 1190.0), 30.0, &[start], &[]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_eq!(r.text(), Some("Winkelhalbierende"));
    let d = (r.point - start).normalized();
    assert!(
        (d.angle().to_degrees() - 45.0).abs() < 1e-6,
        "Richtung der neuen Linie: {:?}",
        d.angle().to_degrees()
    );
}

/// Abseits der Halbierenden bleibt der Zeiger frei.
#[test]
fn ausserhalb_des_fangradius_kein_fang() {
    let s = ecke();
    // 280 mm neben der Halbierenden (x − y = 280).
    let r = fang(&s, v(1400.0, 1120.0), 30.0, &[v(0.0, 0.0)], &[]);
    assert_ne!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_punkt(r.point, v(1400.0, 1120.0), "roher Zeiger");
}

/// Ein echter Endpunkt schlägt die Halbierende: der Punkt auf der Geometrie
/// ist die genauere Wahl.
#[test]
fn endpunkt_schlaegt_die_halbierende() {
    let mut s = ecke();
    // Linie, deren Ende genau auf der Halbierenden liegt: (2000; 2000).
    linie(&mut s, [2000.0, 2000.0], [2600.0, 2400.0]);
    let r = fang(&s, v(2002.0, 1998.0), 30.0, &[v(0.0, 0.0)], &[]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(2000.0, 2000.0), "Endpunkt");
}

/// Zweite Lage: der Punkt im Scheitel ist für den **erweiterten Punktfang**
/// gehalten — von ihm geht die Halbierende aus, auch ohne Bezugspunkt.
#[test]
fn gehaltener_scheitel_liefert_die_halbierende() {
    let s = ecke();
    let r = fang(&s, v(1190.0, 1200.0), 30.0, &[], &[halt(v(0.0, 0.0))]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_eq!(r.label(), Some("Winkelhalbierende"));
}

/// Die Führungslinien für die Anzeige enthalten die Halbierende — am
/// Haltepunkt und am Bezugspunkt, und nur einmal.
#[test]
fn fuehrungslinien_enthalten_die_halbierende() {
    let s = ecke();
    let p = v(0.0, 0.0);
    let linien = hilfslinien_gesamt(&s.doc, &[halt(p)], None, &[]);
    let halbe: Vec<_> = linien
        .iter()
        .filter(|l| l.kind == SnapKind::Bisector)
        .collect();
    assert_eq!(halbe.len(), 1, "{linien:?}");
    assert!((halbe[0].dir.angle().to_degrees() - 45.0).abs() < 1e-6);

    // Am Bezugspunkt dasselbe, ohne Haltepunkt.
    let linien = hilfslinien_gesamt(&s.doc, &[], Some(p), &[]);
    assert!(
        linien.iter().any(|l| l.kind == SnapKind::Bisector),
        "{linien:?}"
    );

    // Derselbe Punkt als Bezug **und** als Haltepunkt: die Linie steht einmal.
    let linien = hilfslinien_gesamt(&s.doc, &[halt(p)], Some(p), &[]);
    assert_eq!(
        linien
            .iter()
            .filter(|l| l.kind == SnapKind::Bisector)
            .count(),
        1,
        "{linien:?}"
    );

    // Ein Punkt zwischen den Linien bringt keine Führungslinie mit.
    let linien = hilfslinien_gesamt(&s.doc, &[], Some(v(1000.0, 1000.0)), &[]);
    assert!(linien.is_empty(), "{linien:?}");
}
