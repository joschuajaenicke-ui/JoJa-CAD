//! Winkelhalbierender Fang (T0150).
//!
//! Zwischen zwei Linien wird die **Winkelhalbierende** als Führungslinie
//! angeboten — der Punkt, den der Zeiger fängt, liegt auf ihr. Sie wird aktiv
//!
//! * wenn eine Linie von einem Punkt zwischen zwei Linien beginnt (der
//!   Bezugspunkt des Zuges liegt zwischen den Linien), und
//! * wenn dieser Punkt für den erweiterten Punktfang gehalten ist.
//!
//! Geprüft wird über `snap::winkelhalbierende` (die Geometrie selbst), über
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

/// Rechter Winkel im Ursprung (Schenkel nach +X und +Y), Punkt im Winkel.
fn rechter_winkel() -> Session {
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
// Die Geometrie
// ---------------------------------------------------------------------------

/// Zwischen den Schenkeln eines rechten Winkels liegt die Halbierende unter
/// 45° — und zwar durch den Punkt, von dem die neue Linie ausgeht.
#[test]
fn halbierende_rechter_winkel_zeigt_45_grad() {
    let s = rechter_winkel();
    let p = v(1000.0, 1000.0);
    let l = jjcad_core::snap::winkelhalbierende(&s.doc, p, 30.0, &[])
        .expect("zwischen zwei Linien muss es eine Halbierende geben");
    assert_eq!(l.kind, SnapKind::Bisector);
    assert_punkt(l.origin, p, "Führungslinie durch den Punkt");
    let grad = l.dir.angle().to_degrees();
    assert!((grad - 45.0).abs() < 1e-6, "Winkel {grad}");
}

/// Ein Punkt auf einer der beiden Linien ist kein Punkt *zwischen* ihnen —
/// dort gibt es keine Halbierende.
#[test]
fn punkt_auf_einer_linie_hat_keine_halbierende() {
    let s = rechter_winkel();
    // Auf dem Schenkel y = 0, 500 mm vom Scheitel entfernt.
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(500.0, 0.0), 30.0, &[]).is_none());
}

/// Parallele Linien: die Halbierende ist die Mittellinie — der Punkt liegt in
/// der Mitte zwischen beiden.
#[test]
fn parallele_linien_liefern_die_mittellinie() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [0.0, 2000.0], [4000.0, 2000.0]);
    let l = jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 1000.0), 30.0, &[])
        .expect("Mittellinie zwischen zwei parallelen Linien");
    assert!((l.dir.angle().to_degrees()).abs() < 1e-6, "{:?}", l.dir);
    // Die Führungslinie liegt auf y = 1000 (Mitte zwischen 0 und 2000).
    assert_punkt(l.origin, v(1000.0, 1000.0), "Mitte");
}

/// Außerhalb der Reichweite gibt es keine Halbierende: die Linien müssen in
/// der Nähe des Punktes **verlaufen**, nicht nur als Gerade dorthin zeigen.
#[test]
fn weit_entfernte_linien_liefern_keine_halbierende() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [0.0, 4000.0], [4000.0, 4000.0]);
    // 4 m über der oberen Linie, Fangradius 30 mm → Weite 1200 mm.
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 8000.0), 30.0, &[]).is_none());
}

/// „Zwei Linien" heißt zwei Linien-Objekte: Rechteck und Polylinie sind keine
/// Linien und liefern keine Halbierende (sonst entstünde an jeder Ecke eines
/// Zuges eine).
#[test]
fn rechteck_und_polylinie_liefern_keine_halbierende() {
    let mut s = s();
    s.execute(
        "draw.rect",
        &json!({ "a": [0.0, 0.0], "b": [2000.0, 2000.0] }),
    )
    .unwrap();
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 1000.0), 30.0, &[]).is_none());

    let mut poly = self::s();
    poly.execute(
        "draw.polyline",
        &json!({ "pts": [[0.0, 0.0], [4000.0, 0.0], [0.0, 4000.0]] }),
    )
    .unwrap();
    assert!(jjcad_core::snap::winkelhalbierende(&poly.doc, v(800.0, 800.0), 30.0, &[]).is_none());
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
    let l = jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 1000.0), 30.0, &[])
        .expect("Halbierende zwischen zwei Wänden");
    let grad = l.dir.angle().to_degrees();
    assert!((grad - 45.0).abs() < 1e-6, "Winkel {grad}");
}

/// Liegen drei Linien um den Punkt, gewinnt das **engste** Paar: die beiden,
/// zwischen denen er tatsächlich sitzt.
#[test]
fn das_engste_paar_gewinnt() {
    let mut s = s();
    // Zwei Schenkel des rechten Winkels …
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [0.0, 0.0], [0.0, 4000.0]);
    // … und eine dritte Linie weit draußen.
    linie(&mut s, [-3000.0, 4000.0], [4000.0, 4000.0]);
    let l = jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 1000.0), 30.0, &[])
        .expect("Halbierende");
    let grad = l.dir.angle().to_degrees();
    assert!((grad - 45.0).abs() < 1e-6, "Winkel {grad}");
}

// ---------------------------------------------------------------------------
// Der Fang
// ---------------------------------------------------------------------------

/// Der Kern des Tickets: die Linie beginnt zwischen zwei Linien — der zweite
/// Punkt rastet auf der Winkelhalbierenden ein, die neue Linie läuft also
/// genau zwischen beiden hindurch.
#[test]
fn linie_beginnt_zwischen_zwei_linien_und_folgt_der_halbierenden() {
    let s = rechter_winkel();
    let start = v(1000.0, 1000.0);
    // Zeiger 10 mm neben der 45°-Halbierenden (x − y = 10).
    let r = fang(&s, v(1200.0, 1190.0), 30.0, &[start], &[]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_eq!(r.text(), Some("Winkelhalbierende"));
    // Der gefangene Punkt liegt auf der Halbierenden durch den Startpunkt …
    let d = (r.point - start).normalized();
    assert!(
        (d.angle().to_degrees() - 45.0).abs() < 1e-6,
        "Richtung der neuen Linie: {:?}",
        d.angle().to_degrees()
    );
    // … und die Strecke vom Startpunkt läuft schräg (nicht waagerecht/senkrecht).
    assert!(r.point.dist(start) > 100.0, "{:?}", r.point);
}

/// Gegenprobe: ohne Punkt zwischen den Linien (kein Bezug, kein Haltepunkt)
/// zieht die Halbierende nicht — sie ist an die beiden Lagen gebunden.
#[test]
fn ohne_punkt_zwischen_den_linien_kein_fang() {
    let s = rechter_winkel();
    let r = fang(&s, v(1200.0, 1190.0), 30.0, &[], &[]);
    assert_ne!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
}

/// Abseits der Halbierenden bleibt der Zeiger frei.
#[test]
fn ausserhalb_des_fangradius_kein_fang() {
    let s = rechter_winkel();
    let start = v(1000.0, 1000.0);
    // 280 mm neben der Halbierenden (x − y = 280).
    let r = fang(&s, v(1400.0, 1120.0), 30.0, &[start], &[]);
    assert_ne!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_punkt(r.point, v(1400.0, 1120.0), "roher Zeiger");
}

/// Ein echter Endpunkt schlägt die Halbierende: der Punkt auf der Geometrie
/// ist die genauere Wahl.
#[test]
fn endpunkt_schlaegt_die_halbierende() {
    let mut s = rechter_winkel();
    // Linie, deren Ende genau auf der Halbierenden liegt: (2000; 2000).
    linie(&mut s, [2000.0, 2000.0], [2600.0, 2400.0]);
    let start = v(1000.0, 1000.0);
    let r = fang(&s, v(2002.0, 1998.0), 30.0, &[start], &[]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(2000.0, 2000.0), "Endpunkt");
}

/// Zweite Lage aus der Aufgabenstellung: der Punkt zwischen zwei Linien ist
/// für den **erweiterten Punktfang** gehalten — von ihm geht die Halbierende
/// als Führungslinie aus, auch ohne Bezugspunkt des Zuges.
#[test]
fn gehaltener_punkt_zwischen_linien_liefert_die_halbierende() {
    let s = rechter_winkel();
    let p = v(1000.0, 1000.0);
    let r = fang(&s, v(1190.0, 1200.0), 30.0, &[], &[halt(p)]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_eq!(r.label(), Some("Winkelhalbierende"));
    let d = (r.point - p).normalized();
    assert!((d.angle().to_degrees() - 45.0).abs() < 1e-6, "{:?}", d);
}

/// Die Führungslinien für die Anzeige enthalten die Halbierende — an einem
/// Haltepunkt und am Bezugspunkt des Zuges.
#[test]
fn fuehrungslinien_enthalten_die_halbierende() {
    let s = rechter_winkel();
    let p = v(1000.0, 1000.0);
    let linien = hilfslinien_gesamt(&s.doc, &[halt(p)], None, 30.0, &[]);
    let halbe: Vec<_> = linien
        .iter()
        .filter(|l| l.kind == SnapKind::Bisector)
        .collect();
    assert_eq!(halbe.len(), 1, "{linien:?}");
    assert!((halbe[0].dir.angle().to_degrees() - 45.0).abs() < 1e-6);

    // Am Bezugspunkt dasselbe, ohne Haltepunkt.
    let linien = hilfslinien_gesamt(&s.doc, &[], Some(p), 30.0, &[]);
    assert!(
        linien.iter().any(|l| l.kind == SnapKind::Bisector),
        "{linien:?}"
    );

    // Derselbe Punkt als Bezug **und** als Haltepunkt: die Linie steht einmal.
    let linien = hilfslinien_gesamt(&s.doc, &[halt(p)], Some(p), 30.0, &[]);
    assert_eq!(
        linien.iter().filter(|l| l.kind == SnapKind::Bisector).count(),
        1,
        "{linien:?}"
    );
}

/// Parallele Wände: die neue Linie läuft auf der Mittellinie zwischen ihnen.
#[test]
fn zwischen_parallelen_linien_laeuft_die_linie_auf_der_mitte() {
    let mut s = s();
    linie(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    linie(&mut s, [0.0, 2000.0], [4000.0, 2000.0]);
    let r = fang(&s, v(1500.0, 1004.0), 30.0, &[v(1000.0, 1000.0)], &[]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert_punkt(r.point, v(1500.0, 1000.0), "Mittellinie");
}

/// Ein bewegtes Element (im Fang ausgeschlossen) liefert keine Halbierende —
/// sonst wanderte die Führungslinie mit dem Griff.
#[test]
fn ausgeschlossenes_element_liefert_keine_halbierende() {
    let s = rechter_winkel();
    let id = s.doc.entities[0].id;
    assert!(jjcad_core::snap::winkelhalbierende(&s.doc, v(1000.0, 1000.0), 30.0, &[id]).is_none());
}
