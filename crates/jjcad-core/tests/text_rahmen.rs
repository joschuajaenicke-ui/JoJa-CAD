//! Unsichtbarer Textrahmen und sein Punktfang (Ticket T0010).
//!
//! Text hat keinen Griff und ist am Bildschirm nur die Schrift selbst. Sein
//! Rahmen — derselbe Kasten, den die Einfüge-Vorschau zeigt — ist der Bezug:
//! er wird bei Auswahl gezeichnet, und seine Ecken (END), Kantenmitten (MIT)
//! und Kanten (NÄC) fangen.
//!
//! Geprüft wird über `render::text_frame` (eine Quelle für Anzeige und Fang)
//! und `snap::snap_constrained_ohne_punkte` — denselben Weg, den die
//! Zeichenfläche für Zeigeranzeige und Klickpunkt nimmt. Alle Koordinaten in
//! Millimetern (`input_unit: mm`).

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::render;
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained_ohne_punkte, SnapKind, SnapResolution, SnapSource};
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

/// Text „Hallo“ mit Höhe 250 mm am Ursprung: Rahmen 810 × 350 mm,
/// (Zeichenbreiten 785 mm + 25 mm Rand),
/// senkrecht mittig (Vorgabe `valign: Middle`), also y von −175 bis +175.
/// Die Kastenhöhe ist die Zeilenhöhe der Schrift (250 × 1,4), damit auch
/// Unterlängen (g, p, j) im Rahmen liegen.
fn session_mit_text() -> Session {
    let mut s = s();
    s.execute(
        "draw.text",
        &json!({ "p": [0.0, 0.0], "text": "Hallo", "height": 250.0 }),
    )
    .unwrap();
    s
}

fn fang(s: &Session, cursor: Vec2, tol: f64) -> SnapResolution {
    snap_constrained_ohne_punkte(&s.doc, cursor, tol, None, &[], &[], &[], false)
}

fn assert_punkt(ist: Vec2, soll: Vec2, was: &str) {
    assert!(ist.dist(soll) < 1e-6, "{was}: erwartet {soll:?}, ist {ist:?}");
}

// ---------------------------------------------------------------------------
// Der Rahmen ist der Kasten der Bounding-Box
// ---------------------------------------------------------------------------

#[test]
fn rahmen_umschliesst_den_text() {
    let s = session_mit_text();
    let e = s.doc.entities.last().unwrap();
    let ecken = render::text_frame(&s.doc, e).expect("Text braucht einen Rahmen");

    assert!(matches!(e.kind, EntityKind::Text { .. }));
    assert_punkt(ecken[0], v(0.0, -175.0), "links unten");
    assert_punkt(ecken[1], v(810.0, -175.0), "rechts unten");
    assert_punkt(ecken[2], v(810.0, 175.0), "rechts oben");
    assert_punkt(ecken[3], v(0.0, 175.0), "links oben");

    // Rahmen und Bounding-Box sind derselbe Kasten (Breite/Höhe in mm).
    let b = render::entity_bbox(&s.doc, e);
    assert!((b.max.x - b.min.x - 810.0).abs() < 1e-6, "Breite {:?}", b);
    assert!((b.max.y - b.min.y - 350.0).abs() < 1e-6, "Höhe {:?}", b);
}

/// Großbuchstaben sind breiter als der alte Mittelwert von 0,58 je Zeichen:
/// der Rahmen muss sie umschließen, nicht durchschneiden („WOHNEN“ stand im
/// Rahmen über den Rand hinaus).
#[test]
fn rahmen_umschliesst_auch_grossbuchstaben() {
    let mut s = s();
    s.execute(
        "draw.text",
        &json!({ "p": [0.0, 0.0], "text": "WOHNEN", "height": 250.0 }),
    )
    .unwrap();
    let e = s.doc.entities.last().unwrap();
    let ecken = render::text_frame(&s.doc, e).expect("Rahmen");

    let breite = ecken[1].x - ecken[0].x;
    let alter_mittelwert = 6.0 * 250.0 * 0.58; // 870 mm
    assert!(
        breite > alter_mittelwert,
        "Rahmen {breite} mm ist nicht breiter als der alte Mittelwert {alter_mittelwert} mm"
    );
    // W ist das breiteste Zeichen (1,15 × Höhe), O/H/N/E/N je 0,90, dazu
    // 10 % Höhe Rand (je Seite 5 %).
    assert!((breite - ((1.15 + 5.0 * 0.90) * 250.0 + 25.0)).abs() < 1e-6, "{breite}");
}

/// Gegenprobe: andere Objekte haben keinen Textrahmen.
#[test]
fn andere_objekte_haben_keinen_textrahmen() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [1000.0, 0.0] }))
        .unwrap();
    let e = s.doc.entities.last().unwrap();
    assert!(render::text_frame(&s.doc, e).is_none());
    assert!(render::text_frame_segments(&s.doc, e).is_empty());
}

/// Vier Kanten, die den Kasten schließen — die Zerlegung, die der Fang nutzt.
#[test]
fn rahmen_besteht_aus_vier_kanten() {
    let s = session_mit_text();
    let e = s.doc.entities.last().unwrap();
    let segs = render::text_frame_segments(&s.doc, e);
    assert_eq!(segs.len(), 4, "vier Kanten: {segs:?}");
}

// ---------------------------------------------------------------------------
// Punktfang am Rahmen
// ---------------------------------------------------------------------------

/// Die Ecke des Rahmens fängt als Endpunkt — nicht der Ankerpunkt des Textes.
#[test]
fn ecke_des_rahmens_faengt_als_endpunkt() {
    let s = session_mit_text();
    let r = fang(&s, v(817.0, 180.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(810.0, 175.0), "Ecke rechts oben");
}

/// Die Mitte einer Rahmenkante fängt als Mittelpunkt.
#[test]
fn kantenmitte_faengt_als_mittelpunkt() {
    let s = session_mit_text();
    let r = fang(&s, v(407.0, 180.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Midpoint), "{r:?}");
    assert_punkt(r.point, v(405.0, 175.0), "Mitte der Oberkante");
}

/// Auf der Rahmenkante fängt der nächste Punkt — der Text hat also nicht nur
/// ein Fangpunkt am Anker, sondern eine fangbare Kante.
#[test]
fn kante_faengt_als_naechster_punkt() {
    let s = session_mit_text();
    let r = fang(&s, v(4.0, 60.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Nearest), "{r:?}");
    assert_punkt(r.point, v(0.0, 60.0), "Punkt auf der linken Kante");
}

/// Der Ankerpunkt des Textes bleibt fangbar (Rückfall auf das alte Verhalten).
#[test]
fn ankerpunkt_faengt_weiter() {
    let s = session_mit_text();
    let r = fang(&s, v(5.0, 4.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(0.0, 0.0), "Ankerpunkt");
}

/// Außerhalb des Fangradius zieht der Rahmen nicht.
#[test]
fn rahmen_faengt_nur_im_radius() {
    let s = session_mit_text();
    let cursor = v(810.0, 500.0);
    let r = fang(&s, cursor, 40.0);
    assert_eq!(r.source, SnapSource::Free, "{r:?}");
    assert_punkt(r.point, cursor, "roher Zeiger");
}

// ---------------------------------------------------------------------------
// Mehrzeilig und gedreht (T0025)
// ---------------------------------------------------------------------------

/// Mehrzeilig: die **breiteste Zeile** bestimmt die Breite (nicht die Summe
/// aller Zeichen), und die Zahl der Zeilen die Höhe. Vorher war der Rahmen
/// doppelt so breit wie der Text und nur eine Zeile hoch.
#[test]
fn mehrzeiliger_rahmen_umschliesst_alle_zeilen() {
    let mut s = s();
    s.execute(
        "draw.text",
        &json!({ "p": [0.0, 0.0], "text": "WOHNEN\nOG", "height": 250.0 }),
    )
    .unwrap();
    let e = s.doc.entities.last().unwrap();
    let ecken = render::text_frame(&s.doc, e).expect("Rahmen");

    // Breite = „WOHNEN“ (W 1,15 + 5 × 0,90 je Höhe, dazu 10 % Rand)
    let breite = ecken[1].x - ecken[0].x;
    let soll = (1.15 + 5.0 * 0.90) * 250.0 + 25.0;
    assert!((breite - soll).abs() < 1e-6, "Breite {breite} statt {soll}");

    // Höhe = zwei Zeilen à 250 × 1,4, senkrecht mittig um den Anker
    let hoehe = ecken[2].y - ecken[1].y;
    assert!((hoehe - 2.0 * 350.0).abs() < 1e-6, "Höhe {hoehe}");
    assert_punkt(ecken[0], v(0.0, -350.0), "links unten");
    assert_punkt(ecken[1], v(soll, -350.0), "rechts unten");
    assert_punkt(ecken[2], v(soll, 350.0), "rechts oben");
    assert_punkt(ecken[3], v(0.0, 350.0), "links oben");
}

/// Dreizeilig: die Höhe folgt der Zeilenzahl (3 × 1,4 × Höhe).
#[test]
fn drei_zeilen_ergeben_dreifache_hoehe() {
    let mut s = s();
    s.execute(
        "draw.text",
        &json!({ "p": [0.0, 0.0], "text": "A\nB\nC", "height": 100.0 }),
    )
    .unwrap();
    let e = s.doc.entities.last().unwrap();
    let ecken = render::text_frame(&s.doc, e).expect("Rahmen");
    let hoehe = ecken[2].y - ecken[1].y;
    assert!((hoehe - 3.0 * 140.0).abs() < 1e-6, "Höhe {hoehe}");
}

/// Ein gedrehter Text hat einen **gedrehten** Rahmen — das Viereck dreht um
/// den Anker mit, statt als achsenparallele Box zu stehen (T0025: „die
/// Umrandung ist falsch“).
#[test]
fn gedrehter_rahmen_dreht_mit_dem_text() {
    let mut s = s();
    s.execute(
        "draw.text",
        &json!({ "p": [0.0, 0.0], "text": "Hallo", "height": 250.0, "angle": 90.0 }),
    )
    .unwrap();
    let e = s.doc.entities.last().unwrap();
    let ecken = render::text_frame(&s.doc, e).expect("Rahmen");

    // 90° gegen den Uhrzeigersinn: „rechts“ zeigt nach oben, „oben“ nach links
    assert_punkt(ecken[0], v(175.0, 0.0), "links unten gedreht");
    assert_punkt(ecken[1], v(175.0, 810.0), "rechts unten gedreht");
    assert_punkt(ecken[2], v(-175.0, 810.0), "rechts oben gedreht");
    assert_punkt(ecken[3], v(-175.0, 0.0), "links oben gedreht");

    // Die Bounding-Box bleibt die Hülle des gedrehten Kastens.
    let b = render::entity_bbox(&s.doc, e);
    assert!((b.max.x - b.min.x - 350.0).abs() < 1e-6, "Breite {b:?}");
    assert!((b.max.y - b.min.y - 810.0).abs() < 1e-6, "Höhe {b:?}");
}
