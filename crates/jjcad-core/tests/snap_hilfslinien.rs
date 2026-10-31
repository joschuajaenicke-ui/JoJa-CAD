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

/// Ortho und Hilfslinie greifen **gemeinsam** (T0042): die Zwangsachse bleibt
/// Bedingung, die Hilfslinie wählt die Lage auf ihr. Bisher gewann je nach
/// Abstand zur Hilfslinie das eine oder das andere — die Hilfslinie zog den
/// Punkt auf (3000; 800), Ortho auf (3004; 0), und die gezeichnete Linie kippte
/// aus der Zwangsrichtung. Jetzt ist der Schnittpunkt beider der gemeinte
/// Punkt: (3000; 0).
#[test]
fn hilfslinie_und_ortho_bilden_den_schnittpunkt() {
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
    assert_punkt(r.point, v(3000.0, 0.0), "Schnittpunkt Hilfslinie × Ortho");
    assert_eq!(r.label(), Some("Hilfslinie"));

    // Gegenprobe 1: ohne Haltepunkt zieht Ortho den Zeiger auf die Achse.
    let ohne = fang(&s, v(3004.0, 800.0), 30.0, Some(v(0.0, 0.0)), &[]);
    assert_eq!(ohne.source, SnapSource::Ortho, "{ohne:?}");
    assert_punkt(ohne.point, v(3004.0, 0.0), "nur Ortho");
}

/// Ohne Ortho bleibt die Hilfslinie das, was sie war: der Fußpunkt des Zeigers
/// auf ihr (Gegenprobe zum Schnittpunkt oben — der Unterschied kommt allein aus
/// der Zwangsachse).
#[test]
fn ohne_ortho_bleibt_der_fusspunkt_der_hilfslinie() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [3000.0, 0.0] }))
        .unwrap();
    let h = halt(&s, v(2999.0, 2.0), 30.0);
    let r = fang(&s, v(3004.0, 800.0), 30.0, Some(v(0.0, 0.0)), &[h]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert_punkt(r.point, v(3000.0, 800.0), "Fußpunkt auf der Senkrechten");
}

/// Der Fall aus dem Ticket: eine neue Linie wird gezeichnet (Bezugspunkt unten
/// rechts), ein Punkt ist gehalten — der Zeiger steht wenige Millimeter neben
/// dessen Senkrechter und läuft waagerecht: gefangen wird die Kreuzung aus
/// Senkrechter und Zwangsachse, also **beides** zugleich. Der Zeiger liegt
/// dabei weit über der Achse; die Achse gilt trotzdem (Ortho ist hart).
#[test]
fn ortho_und_hilfslinie_fangen_die_kreuzung_auch_weit_abseits_der_achse() {
    let mut s = s();
    s.execute(
        "draw.line",
        &json!({ "a": [3000.0, 0.0], "b": [3000.0, 2000.0] }),
    )
    .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "ortho": true, "polar": false } }),
    )
    .unwrap();
    // Gehalten wird das untere Ende der Senkrechten: Verlängerung senkrecht,
    // Normale waagerecht.
    let h = halt(&s, v(3001.0, 1.0), 30.0);
    assert_punkt(h.point, v(3000.0, 0.0), "gehaltenes Linienende");
    // Bezugspunkt der neuen Linie (0; -1000), Zeiger 5 mm neben x = 3000 und
    // 900 mm über der waagerechten Zwangsachse y = -1000.
    let r = fang(&s, v(2995.0, -100.0), 30.0, Some(v(0.0, -1000.0)), &[h]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert_punkt(r.point, v(3000.0, -1000.0), "Kreuzung Senkrechte × Ortho");
}

/// Polarfang genauso: rastet die Richtung ein, ist der Schnittpunkt der
/// Polargeraden mit der Hilfslinie der Punkt — nicht der Fußpunkt daneben.
#[test]
fn polar_und_hilfslinie_bilden_den_schnittpunkt() {
    let mut s = s();
    s.execute("draw.line", &json!({ "a": [0.0, 0.0], "b": [3000.0, 0.0] }))
        .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "ortho": false, "polar": true } }),
    )
    .unwrap();
    // Haltepunkt ist die Linienmitte (1500; 0): Verlängerung waagerecht,
    // Normale senkrecht bei x = 1500.
    let h = halt(&s, v(1501.0, 1.0), 30.0);
    assert_punkt(h.point, v(1500.0, 0.0), "gehaltene Linienmitte");
    // Bezugspunkt (0; -3000): die 45°-Gerade läuft durch (1500; -1500), der
    // Zeiger steht 4 mm daneben auf der Senkrechten.
    let r = fang(&s, v(1504.0, -1496.0), 30.0, Some(v(0.0, -3000.0)), &[h]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert_punkt(r.point, v(1500.0, -1500.0), "Kreuzung Senkrechte × 45°");
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

// ---------------------------------------------------------------------------
// Schnittpunkt Hilfslinie × bestehende Linie (Turn 2)
// ---------------------------------------------------------------------------

/// Ein einzelner Haltepunkt im Ursprung: Waagerechte y = 0, Senkrechte x = 0.
fn halt_im_ursprung() -> HaltPunkt {
    HaltPunkt {
        point: v(0.0, 0.0),
        kind: SnapKind::Center,
        entity: None,
        richtung: None,
    }
}

/// Wo eine Hilfslinie eine bestehende Linie kreuzt, rastet der Zeiger **exakt**
/// auf der Kreuzung ein — nicht auf dem Fußpunkt daneben.
#[test]
fn schnittpunkt_hilfslinie_mit_linie_faengt_exakt() {
    let mut s = s();
    s.execute(
        "draw.line",
        &json!({ "a": [-2000.0, 1000.0], "b": [5000.0, 1000.0] }),
    )
    .unwrap();
    let r = fang(&s, v(4.0, 996.0), 30.0, None, &[halt_im_ursprung()]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Intersection), "{r:?}");
    assert_punkt(r.point, v(0.0, 1000.0), "Kreuzung Senkrechte × Linie");
    assert_eq!(r.label(), Some("Schnittpunkt"));
}

/// Abseits der Kreuzung bleibt der gleitende Fußpunkt auf der Hilfslinie —
/// die Kreuzung wird nicht Globalfänger.
#[test]
fn fusspunkt_bleibt_abseits_der_kreuzung() {
    let mut s = s();
    s.execute(
        "draw.line",
        &json!({ "a": [-2000.0, 1000.0], "b": [5000.0, 1000.0] }),
    )
    .unwrap();
    let r = fang(&s, v(4.0, 3000.0), 30.0, None, &[halt_im_ursprung()]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert_punkt(r.point, v(0.0, 3000.0), "Fußpunkt auf der Senkrechten");
}

/// Nur die **Strecke** zählt: die Verlängerung einer bestehenden Linie ist kein
/// Schnittpunkt (gleiche Regel wie beim Objekt-Schnittpunkt).
#[test]
fn verlaengerung_der_linie_ist_kein_schnittpunkt() {
    let mut s = s();
    s.execute(
        "draw.line",
        &json!({ "a": [2000.0, 1000.0], "b": [5000.0, 1000.0] }),
    )
    .unwrap();
    // Der Fußpunkt (0; 1000) liegt auf der verlängerten Linie, nicht auf ihr.
    let r = fang(&s, v(4.0, 1000.0), 30.0, None, &[halt_im_ursprung()]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
    assert_punkt(r.point, v(0.0, 1000.0), "Fußpunkt");
}

/// Ein ausgewähltes Objekt liefert keinen Hilfslinien-Schnittpunkt — es bewegt
/// sich mit, der Punkt wanderte mit (wie beim Objekt-Schnittpunkt).
#[test]
fn ausgewaehlte_linie_bildet_keinen_hilfslinien_schnittpunkt() {
    let mut s = s();
    s.execute(
        "draw.line",
        &json!({ "a": [-2000.0, 1000.0], "b": [5000.0, 1000.0] }),
    )
    .unwrap();
    let id = s.doc.entities[0].id;
    let r = snap_constrained_mit_halte(
        &s.doc,
        v(4.0, 996.0),
        30.0,
        None,
        &[],
        &[id],
        &[],
        &[],
        &[halt_im_ursprung()],
        false,
    );
    assert_eq!(r.source, SnapSource::Object(SnapKind::Tracking), "{r:?}");
}

/// Ohne Haltepunkt gibt es keinen Hilfslinien-Schnittpunkt — die Gegenprobe
/// zeigt, dass wirklich die Hilfslinie den Punkt liefert.
#[test]
fn ohne_haltepunkt_kein_hilfslinien_schnittpunkt() {
    let s2 = {
        let mut s = s();
        s.execute(
            "draw.line",
            &json!({ "a": [-2000.0, 1000.0], "b": [5000.0, 1000.0] }),
        )
        .unwrap();
        s
    };
    let r = fang(&s2, v(4.0, 996.0), 30.0, None, &[]);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Nearest), "{r:?}");
}
