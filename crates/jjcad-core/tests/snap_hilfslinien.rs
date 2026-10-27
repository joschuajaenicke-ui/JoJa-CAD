//! Erweiterter Punktfang (Ticket T0014): Hilfslinien eines gehaltenen Punktes.
//!
//! Der Zeiger stand lange genug über einem Fangpunkt (ohne Klick) — dann geht
//! von ihm ein Paar Hilfslinien aus, auf dem der nächste Punkt gefangen wird:
//!
//! * Punkt auf einer Linie: **Verlängerung** der Linie und **Normale** dazu.
//! * Einzelner Punkt (Zentrum, Blattecke): **Waagerechte** und **Senkrechte**.
//!
//! Geprüft wird über `snap::hilfslinien_snap` (die Hilfslinien selbst) und über
//! `snap::snap_constrained_mit_halte` — denselben Weg, den die Zeichenfläche für
//! Zeigeranzeige und Klickpunkt nimmt. Alle Koordinaten in Millimetern.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::session::Session;
use jjcad_core::snap::{
    hilfslinien, hilfslinien_snap, snap_constrained_mit_halte, HaltPunkt, SnapHit, SnapKind,
    SnapSource,
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

fn assert_punkt(ist: Vec2, soll: Vec2, was: &str) {
    assert!(ist.dist(soll) < 1e-9, "{was}: erwartet {soll:?}, ist {ist:?}");
}

/// Halt aus einem echten Fangtreffer der Engine (wie ihn die GUI bildet).
fn halt(s: &Session, p: Vec2, tol: f64) -> HaltPunkt {
    let hit: SnapHit = jjcad_core::snap::find_snap(&s.doc, p, tol, None, &[], &[])
        .unwrap_or_else(|| panic!("kein Fangpunkt bei {p:?}"));
    HaltPunkt::neu(&s.doc, &hit)
}

/// Fang mit Haltepunkten — die Lage während der Eingabe.
fn fang(
    s: &Session,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    halte: &[HaltPunkt],
) -> jjcad_core::snap::SnapResolution {
    snap_constrained_mit_halte(
        &s.doc, cursor, tol, reference, &[], &[], &[], &[], halte, false,
    )
}

// ---------------------------------------------------------------------------
// Welche Hilfslinien entstehen
// ---------------------------------------------------------------------------

/// Ein Punkt auf einer schrägen Linie bekommt deren Verlängerung und die
/// Normale — nicht das Achsenkreuz.
#[test]
fn punkt_auf_linie_liefert_verlaengerung_und_normale() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [3000.0, 3000.0] }))
        .unwrap();
    let h = halt(&s, v(2998.0, 3001.0), 30.0);
    assert_punkt(h.point, v(3000.0, 3000.0), "Linienende");
    let d = h.richtung.expect("Punkt gehört zur Linie");
    assert!((d.angle().to_degrees() - 45.0).abs() < 1e-6, "{d:?}");

    let linien = hilfslinien(std::slice::from_ref(&h));
    assert_eq!(linien.len(), 2);
    // Beide durch den Haltepunkt, eine in Linienrichtung, eine senkrecht dazu.
    for l in &linien {
        assert_punkt(l.origin, h.point, "Ursprung");
        assert!(
            l.dir.angle().to_degrees().rem_euclid(180.0).round() == 45.0
                || l.dir.angle().to_degrees().rem_euclid(180.0).round() == 135.0,
            "Winkel {:?}",
            l.dir.angle().to_degrees()
        );
    }
}

/// Ein einzelner Punkt (hier der Kreismittelpunkt) bekommt Waagerechte und
/// Senkrechte — er gehört zu keiner Linie.
#[test]
fn einzelner_punkt_bekommt_achsenkreuz() {
    let mut s = s();
    s.execute(
        "draw.circle",
        &json!({ "c": [1000.0, 1000.0], "r": 500.0 }),
    )
    .unwrap();
    let h = halt(&s, v(1002.0, 1003.0), 30.0);
    assert_eq!(h.kind, SnapKind::Center);
    assert!(h.richtung.is_none(), "Zentrum liegt auf keiner Linie: {h:?}");
    let linien = hilfslinien(std::slice::from_ref(&h));
    let winkel: Vec<f64> = linien
        .iter()
        .map(|l| l.dir.angle().to_degrees().rem_euclid(180.0).round())
        .collect();
    assert_eq!(winkel, vec![0.0, 90.0], "{winkel:?}");
}

// ---------------------------------------------------------------------------
// Der Fang auf den Hilfslinien
// ---------------------------------------------------------------------------

/// Der Kern: neben der Senkrechten des Haltepunkts rastet der Zeiger auf deren
/// x-Wert ein — der y-Wert bleibt frei.
#[test]
fn senkrechte_hilfslinie_zieht_den_zeiger_auf_die_x_achse() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [3000.0, 0.0] }))
        .unwrap();
    let h = halt(&s, v(2999.0, 2.0), 30.0);
    let hit = hilfslinien_snap(std::slice::from_ref(&h), v(3004.0, 800.0), 30.0)
        .expect("Hilfslinie muss fangen");
    assert_eq!(hit.kind, SnapKind::Tracking);
    assert_punkt(hit.point, v(3000.0, 800.0), "auf der Senkrechten");
    assert_eq!(hit.kind.label(), "Hilfslinie");
}

/// Außerhalb des Fangradius zieht die Hilfslinie nicht.
#[test]
fn hilfslinie_faengt_nur_im_radius() {
    let h = HaltPunkt {
        point: v(3000.0, 0.0),
        kind: SnapKind::Endpoint,
        entity: None,
        richtung: None,
    };
    assert!(hilfslinien_snap(std::slice::from_ref(&h), v(3100.0, 800.0), 30.0).is_none());
}

/// Zwei Haltepunkte: x vom einen, y vom anderen — der Schnittpunkt beider
/// Hilfslinien hat Vorrang vor der näheren Einzelachse.
#[test]
fn zwei_haltepunkte_kombinieren_x_und_y() {
    let mut s = s();
    s.execute(
        "draw.circle",
        &json!({ "c": [0.0, 0.0], "r": 300.0 }),
    )
    .unwrap();
    s.execute(
        "draw.circle",
        &json!({ "c": [3000.0, 4000.0], "r": 300.0 }),
    )
    .unwrap();
    let a = halt(&s, v(1.0, 1.0), 30.0);
    let b = halt(&s, v(2999.0, 4001.0), 30.0);
    assert_punkt(a.point, v(0.0, 0.0), "erster Halt");
    assert_punkt(b.point, v(3000.0, 4000.0), "zweiter Halt");

    // 4 mm neben der Kreuzung (0; 4000) — die Einzelachsen lägen bei (0; 3996)
    // bzw. (4; 4000), also näher am Zeiger.
    let hit = hilfslinien_snap(&[a, b], v(4.0, 3996.0), 30.0).expect("Kreuzung muss fangen");
    assert_eq!(hit.kind, SnapKind::Tracking);
    assert_punkt(hit.point, v(0.0, 4000.0), "Kreuzung x von A, y von B");
}

// ---------------------------------------------------------------------------
// Einordnung in die Fang-Kette
// ---------------------------------------------------------------------------

/// Die Hilfslinie schlägt Ortho: die Zwangsachse vom Bezugspunkt würde den
/// Zeiger nach (3004; 0) ziehen, der gehaltene Punkt verlangt (3000; 800).
#[test]
fn hilfslinie_schlaegt_ortho() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [3000.0, 0.0] }))
        .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "ortho": true, "polar": false } }),
    )
    .unwrap();
    let h = halt(&s, v(2999.0, 2.0), 30.0);
    let r = fang(&s, v(3004.0, 800.0), 30.0, Some(v(0.0, 0.0)), &[h.clone()]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert_punkt(r.point, v(3000.0, 800.0), "Hilfslinie");
    assert_eq!(r.label(), Some("Hilfslinie"));

    // Gegenprobe: ohne Haltepunkt zieht Ortho den Zeiger auf die Achse.
    let ohne = fang(&s, v(3004.0, 800.0), 30.0, Some(v(0.0, 0.0)), &[]);
    assert_eq!(ohne.source, SnapSource::Ortho, "{ohne:?}");
}

/// Ein echter Endpunkt schlägt die Hilfslinie — der Punkt auf der Geometrie ist
/// immer die genauere Wahl.
#[test]
fn endpunkt_schlaegt_die_hilfslinie() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [3000.0, 0.0] }))
        .unwrap();
    // Zweite Linie, deren Ende genau auf der Senkrechten von (3000; 0) liegt.
    s.execute(
        "draw.line",
        &json!({ "a": [3000.0, 2000.0], "b": [5000.0, 2000.0] }),
    )
    .unwrap();
    let h = halt(&s, v(2999.0, 2.0), 30.0);
    assert_punkt(h.point, v(3000.0, 0.0), "gehaltenes Linienende");
    let r = fang(&s, v(3003.0, 2004.0), 30.0, None, &[h]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(3000.0, 2000.0), "Endpunkt der zweiten Linie");
}

/// Ohne Haltepunkte ändert sich nichts: der Zeiger bleibt frei.
#[test]
fn ohne_haltepunkt_bleibt_der_zeiger_frei() {
    let s = s();
    let cursor = v(6000.0, 800.0);
    let r = fang(&s, cursor, 30.0, None, &[]);
    assert_eq!(r.source, SnapSource::Free, "{r:?}");
    assert_punkt(r.point, cursor, "roher Zeiger");
}
