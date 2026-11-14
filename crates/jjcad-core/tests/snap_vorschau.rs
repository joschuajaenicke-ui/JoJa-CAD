//! Punktfang auf die eigene Vorschau bei der Polylinien-Eingabe (Ticket T0002).
//!
//! Die Polylinie steht erst mit dem Abschluss im Modell. Bis dahin muss die
//! Fang-Engine die bereits gesetzten Stützpunkte als Vorschau bekommen — sonst
//! ist weder der Anfang (Schließen) noch ein früherer Zwischenpunkt anvisierbar,
//! und eine geschlossene Polylinie entsteht nur per Zufall.
//!
//! Geprüft wird über `snap::snap_constrained_mit_vorschau` — denselben Weg, den
//! die Zeichenfläche für Zeigeranzeige und Klickpunkt nimmt. Alle Koordinaten
//! in Millimetern (`input_unit: mm`).

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::session::Session;
use jjcad_core::snap::{
    snap_constrained, snap_constrained_mit_vorschau, SnapKind, SnapResolution, SnapSource,
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

/// Ortho-Zwang an (der Fall, in dem die Zwangsachse den Anfangspunkt verdeckt).
fn mit_ortho() -> Session {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "ortho": true, "polar": false } }),
    )
    .unwrap();
    s
}

/// Fang mit Vorschau-Punkten — die Lage während der Eingabe.
fn fang(s: &Session, cursor: Vec2, tol: f64, reference: Option<Vec2>, vorschau: &[Vec2]) -> SnapResolution {
    snap_constrained_mit_vorschau(&s.doc, cursor, tol, reference, &[], vorschau, false)
}

fn assert_punkt(ist: Vec2, soll: Vec2, was: &str) {
    assert!(ist.dist(soll) < 1e-9, "{was}: erwartet {soll:?}, ist {ist:?}");
}

// ---------------------------------------------------------------------------
// Anfang und Zwischenpunkte sind anvisierbar
// ---------------------------------------------------------------------------

/// Der Kern: der Anfangspunkt der Vorschau fängt — auch mit Ortho, dessen Achse
/// vom letzten Stützpunkt aus weit am Anfang vorbeiläuft.
#[test]
fn anfangspunkt_der_vorschau_faengt_trotz_ortho() {
    let s = mit_ortho();
    let start = v(0.0, 0.0);
    let letzter = v(2000.0, 1500.0);
    let vorschau = [start, v(2000.0, 0.0)];

    // 10 mm neben dem Anfang; Ortho vom letzten Punkt aus ergäbe (10; 1500).
    let r = fang(&s, v(10.0, -6.0), 40.0, Some(letzter), &vorschau);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, start, "Anfangspunkt");
    assert_eq!(r.label(), Some("Endpunkt"));
}

/// Ein Zwischenpunkt der Vorschau ist ebenso Fangziel — nicht nur der Anfang.
#[test]
fn zwischenpunkt_der_vorschau_faengt() {
    let s = s();
    let ecke = v(2000.0, 0.0);
    let vorschau = [v(0.0, 0.0), ecke];
    let r = fang(&s, v(1994.0, 8.0), 40.0, Some(v(2000.0, 1500.0)), &vorschau);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, ecke, "Zwischenpunkt");
}

/// Gegenprobe: ohne Vorschau bleibt der Zeiger frei — der Fehler war, dass die
/// Vorschau-Punkte der Fang-Engine überhaupt nicht bekannt waren.
#[test]
fn ohne_vorschau_kein_fang() {
    let s = s();
    let cursor = v(1994.0, 8.0);
    let r = snap_constrained(
        &s.doc,
        cursor,
        40.0,
        Some(v(2000.0, 1500.0)),
        &[],
        false,
    );
    assert_eq!(r.source, SnapSource::Free, "{r:?}");
    assert_punkt(r.point, cursor, "roher Zeiger");
}

/// Außerhalb des Fangradius zieht die Vorschau nicht — der Radius gilt wie für
/// jedes andere Objekt.
#[test]
fn vorschau_faengt_nur_im_radius() {
    let s = s();
    let cursor = v(1460.0, 0.0);
    let r = fang(&s, cursor, 40.0, Some(v(0.0, 0.0)), &[v(2000.0, 0.0)]);
    assert_eq!(r.source, SnapSource::Free, "{r:?}");
    assert_punkt(r.point, cursor, "roher Zeiger");
}

// ---------------------------------------------------------------------------
// Übernommener Punkt == Fangpunkt, Schluss auf dem Anfang
// ---------------------------------------------------------------------------

/// Der Fangpunkt geht unverändert in die Punktliste und schließt den Zug exakt:
/// drei Stützpunkte, `closed` gesetzt, kein doppelter Endpunkt.
#[test]
fn gefangener_anfang_schliesst_den_zug() {
    let mut s = s();
    let start = v(0.0, 0.0);
    let zweiter = v(2000.0, 0.0);
    let dritter = v(2000.0, 1500.0);

    // Vierter Klick: Zeiger 7 mm neben dem Anfang, Vorschau = erste drei Punkte
    // ohne den letzten (der ist Bezugspunkt).
    let r = fang(&s, v(7.0, -5.0), 40.0, Some(dritter), &[start, zweiter]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, start, "Anfangspunkt");

    s.execute(
        "draw.polyline",
        &json!({ "pts": [[start.x, start.y], [zweiter.x, zweiter.y],
                         [dritter.x, dritter.y], [r.point.x, r.point.y]] }),
    )
    .unwrap();

    match &s.doc.entities.last().unwrap().kind {
        EntityKind::Polyline { verts, closed } => {
            assert!(*closed, "letzter Punkt ist der erste → geschlossen");
            assert_eq!(verts.len(), 3, "der doppelte Endpunkt wird nicht gespeichert");
            assert_punkt(verts[0].p, start, "erster Stützpunkt");
        }
        other => panic!("keine Polylinie angelegt: {other:?}"),
    }
}

/// Ein vorhandenes Objekt an derselben Stelle bleibt fangbar: die Vorschau
/// verdrängt den Objektfang nicht, sie ergänzt ihn.
#[test]
fn objektfang_neben_der_vorschau_bleibt_erhalten() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 3000.0], "b": [2000.0, 3000.0] }))
        .unwrap();
    let r = fang(&s, v(1990.0, 3007.0), 40.0, Some(v(0.0, 0.0)), &[v(0.0, 0.0), v(0.0, 500.0)]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(2000.0, 3000.0), "Linienende");
}
