//! Fang-Einstellungen (T0160) — die Kategorie „Fangen“ im Einstellungsfenster.
//!
//! Geprüft wird, was die Einstellungen **wirken** lassen: Quadranten und
//! Winkelhalbierende sind einzeln abschaltbar, die Hilfslinien des erweiterten
//! Punktfangs lassen sich abschalten, Verweildauer und Zahl der Haltepunkte
//! kommen aus der Zeichnung. Dazu die Grenzen in der Action `doc.settings` und
//! die Vorgaben für alte Dateien ohne diese Felder.
//!
//! Alle Koordinaten in Millimetern.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{Document, SnapSettings};
use jjcad_core::session::Session;
use jjcad_core::snap::{
    find_snap, snap_candidates, snap_constrained_achsen, HaltPunkt, SnapKind, SnapSource,
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

/// Kreis um den Ursprung mit 1 m Radius — sein Quadrant bei (1000; 0) ist
/// gleichzeitig das Ende des Bogenstücks.
fn kreis(s: &mut Session) {
    s.execute(
        "draw.circle",
        &json!({ "c": [0.0, 0.0], "r": 1000.0 }),
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Quadrant
// ---------------------------------------------------------------------------

/// Der Quadrant ist ein eigenes Fangziel: mit abgeschaltetem Endpunkt-Fang
/// bleibt er erreichbar, solange `quadrant` an ist.
#[test]
fn quadrant_faengt_auch_ohne_endpunkt_fang() {
    let mut s = s();
    kreis(&mut s);
    s.doc.settings.snap.endpoint = false;
    // Mitte und „Nächster Punkt" aus: sonst liegen sie auf demselben Punkt wie
    // der Quadrant und gewinnen über die Fangpriorität.
    s.doc.settings.snap.midpoint = false;
    s.doc.settings.snap.nearest = false;
    s.doc.settings.snap.quadrant = true;
    let hit = find_snap(&s.doc, v(0.0, 1004.0), 30.0, None, &[], &[])
        .expect("Quadrant muss fangen");
    assert_eq!(hit.kind, SnapKind::Quadrant);
    assert!(hit.point.dist(v(0.0, 1000.0)) < 1e-6, "{:?}", hit.point);
}

/// Und er lässt sich einzeln abschalten, ohne den Endpunkt-Fang zu verlieren.
#[test]
fn quadrant_ist_einzeln_abschaltbar() {
    let mut s = s();
    kreis(&mut s);
    s.doc.settings.snap.quadrant = false;
    s.doc.settings.snap.endpoint = true;
    s.doc.settings.snap.midpoint = false;
    s.doc.settings.snap.nearest = false;
    let cands = snap_candidates(&s.doc, v(0.0, 1000.0), 30.0, None, &[], &[]);
    assert!(
        !cands.iter().any(|h| h.kind == SnapKind::Quadrant),
        "Quadrant trotz Abschaltung: {cands:?}"
    );
    // Der Endpunkt des Bogenstücks (1000; 0) fängt weiter.
    let hit = find_snap(&s.doc, v(1000.0, 0.0), 30.0, None, &[], &[]).expect("Endpunkt");
    assert_eq!(hit.kind, SnapKind::Endpoint);
}

// ---------------------------------------------------------------------------
// Winkelhalbierende
// ---------------------------------------------------------------------------

fn ecke(s: &mut Session) {
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [4000.0, 0.0] }))
        .unwrap();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [0.0, 4000.0] }))
        .unwrap();
}

fn fang(s: &Session, cursor: Vec2, achsen: &[Vec2]) -> jjcad_core::snap::SnapResolution {
    snap_constrained_achsen(&s.doc, cursor, 30.0, achsen, &[], &[], &[], &[], &[], false)
}

/// Die Winkelhalbierende im Scheitel fängt, solange `bisector` an ist.
#[test]
fn winkelhalbierende_faengt_im_scheitel() {
    let mut s = s();
    ecke(&mut s);
    let r = fang(&s, v(1000.0, 990.0), &[v(0.0, 0.0)]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert!(r.point.dist(v(995.0, 995.0)) < 1.0, "{:?}", r.point);
}

/// Abgeschaltet bleibt der Zeiger auf der Halbierenden frei.
#[test]
fn winkelhalbierende_ist_abschaltbar() {
    let mut s = s();
    ecke(&mut s);
    s.doc.settings.snap.bisector = false;
    let r = fang(&s, v(1000.0, 990.0), &[v(0.0, 0.0)]);
    assert_ne!(r.source, SnapSource::Object(SnapKind::Bisector), "{r:?}");
    assert!(r.point.dist(v(1000.0, 990.0)) < 1e-6, "{:?}", r.point);
}

// ---------------------------------------------------------------------------
// Erweiterter Punktfang (Hilfslinien)
// ---------------------------------------------------------------------------

fn halt(p: Vec2) -> HaltPunkt {
    HaltPunkt {
        point: p,
        kind: SnapKind::Endpoint,
        entity: None,
        richtung: None,
    }
}

fn fang_mit_halte(s: &Session, cursor: Vec2, halte: &[HaltPunkt]) -> jjcad_core::snap::SnapResolution {
    snap_constrained_achsen(&s.doc, cursor, 30.0, &[], &[], &[], &[], &[], halte, false)
}

/// Die Senkrechte eines gehaltenen Punktes fängt den nächsten Punkt.
#[test]
fn hilfslinie_faengt_den_naechsten_punkt() {
    let s = s();
    let r = fang_mit_halte(&s, v(6.0, 1000.0), &[halt(v(0.0, 0.0))]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert!(r.point.dist(v(0.0, 1000.0)) < 1e-6, "{:?}", r.point);
}

/// Mit `hilfslinien = false` fängt dort nichts mehr.
#[test]
fn hilfslinien_sind_abschaltbar() {
    let mut s = s();
    s.doc.settings.snap.hilfslinien = false;
    let r = fang_mit_halte(&s, v(6.0, 1000.0), &[halt(v(0.0, 0.0))]);
    assert_ne!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert!(r.point.dist(v(6.0, 1000.0)) < 1e-6, "{:?}", r.point);
}

// ---------------------------------------------------------------------------
// Action `doc.settings`: Werte, Grenzen, alte Dateien
// ---------------------------------------------------------------------------

/// Die neuen Fang-Einstellungen kommen über `doc.settings` in die Zeichnung —
/// mit denselben Grenzen wie die Felder im Einstellungsfenster.
#[test]
fn doc_settings_setzt_und_begrenzt_die_fang_einstellungen() {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": {
            "quadrant": false,
            "bisector": false,
            "hilfslinien": false,
            "verweil_ms": 900.0,
            "halte_max": 3
        } }),
    )
    .unwrap();
    assert!(!s.doc.settings.snap.quadrant);
    assert!(!s.doc.settings.snap.bisector);
    assert!(!s.doc.settings.snap.hilfslinien);
    assert!((s.doc.settings.snap.verweil_ms - 900.0).abs() < 1e-9);
    assert_eq!(s.doc.settings.snap.halte_max, 3);

    // Grenzen: zu kurz, zu lang, zu viele Haltepunkte.
    s.execute(
        "doc.settings",
        &json!({ "snap": { "verweil_ms": 1.0, "halte_max": 99 } }),
    )
    .unwrap();
    assert!(
        (s.doc.settings.snap.verweil_ms - jjcad_core::model::VERWEIL_MIN_MS).abs() < 1e-9,
        "{}",
        s.doc.settings.snap.verweil_ms
    );
    assert_eq!(
        s.doc.settings.snap.halte_max,
        jjcad_core::model::HALTE_MAX
    );
}

/// Alte Dateien kennen die neuen Felder nicht — sie bekommen die Vorgaben,
/// ohne dass etwas am Dokument geändert werden muss.
#[test]
fn alte_datei_ohne_fang_felder_bekommt_die_vorgaben() {
    let alt: SnapSettings = serde_json::from_str("{}").expect("leeres snap-Objekt");
    assert!(alt.quadrant);
    assert!(alt.bisector);
    assert!(alt.hilfslinien);
    assert!((alt.verweil_ms - 400.0).abs() < 1e-9);
    assert_eq!(alt.halte_max, 2);
}

/// Die Vorgabe einer frischen Zeichnung bleibt die des erweiterten
/// Punktfangs: 400 ms und zwei Haltepunkte.
#[test]
fn neue_zeichnung_hat_die_bisherigen_vorgaben() {
    let d = Document::new();
    assert!((d.settings.snap.verweil_ms - 400.0).abs() < 1e-9);
    assert_eq!(d.settings.snap.halte_max, 2);
    assert!(d.settings.snap.hilfslinien);
}

/// Die Einstellungen stehen in der gespeicherten `.jjcad`-Datei und laden
/// unverändert wieder — der Prüfweg am Feld, nicht am Bild.
#[test]
fn fang_einstellungen_stehen_in_der_datei() {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": {
            "quadrant": false,
            "hilfslinien": false,
            "verweil_ms": 700.0,
            "halte_max": 3
        } }),
    )
    .unwrap();
    let text = jjcad_core::persist::to_json(&s.doc).expect("schreiben");
    assert!(text.contains("\"verweil_ms\": 700"), "verweil_ms fehlt: {text}");
    assert!(text.contains("\"halte_max\": 3"), "halte_max fehlt");
    assert!(text.contains("\"quadrant\": false"), "quadrant fehlt");
    assert!(text.contains("\"hilfslinien\": false"), "hilfslinien fehlt");

    let wieder = jjcad_core::persist::from_json(&text).expect("lesen");
    assert!(!wieder.settings.snap.quadrant);
    assert!(!wieder.settings.snap.hilfslinien);
    assert!((wieder.settings.snap.verweil_ms - 700.0).abs() < 1e-9);
    assert_eq!(wieder.settings.snap.halte_max, 3);
}
