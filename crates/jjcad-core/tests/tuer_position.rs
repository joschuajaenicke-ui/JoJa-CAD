//! T0078: Türen und Fenster liegen **in** der Wand.
//!
//! Vorher nahm die Action jede Längslage an: die Öffnung ließ sich über das
//! Wandende hinaus schieben und stand dort in der Luft. Die Grenze ist die
//! freie Wandstrecke — an verbundenen Ecken endet sie an der Anschlussnaht
//! (den Gehrungspunkten der Umrandung), sonst an der Stirn. Sie gilt beim
//! Platzieren, beim Ändern der Breite **und** beim Verschieben.

use jjcad_core::geom::Vec2;
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;

fn leer() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .unwrap();
    s
}

fn neu(s: &mut Session, action: &str, params: serde_json::Value) -> Id {
    let res = s
        .execute(action, &params)
        .unwrap_or_else(|e| panic!("{action} fehlgeschlagen: {e}"));
    res["created"][0].as_u64().expect("keine Id erzeugt")
}

fn wand(s: &mut Session, a: (f64, f64), b: (f64, f64), dicke: f64) -> Id {
    neu(
        s,
        "draw.wall",
        json!({"a": [a.0, a.1], "b": [b.0, b.1], "thickness": dicke}),
    )
}

/// Längslage (`at`), Mittelpunkt und Breite einer Öffnung.
fn lage(s: &Session, id: Id) -> (Option<f64>, Vec2, f64) {
    let e = s.doc.entity(id).expect("Öffnung fehlt");
    match &e.kind {
        EntityKind::Door {
            at,
            center,
            width,
            ..
        }
        | EntityKind::Window {
            at,
            center,
            width,
            ..
        } => (*at, *center, *width),
        _ => panic!("keine Öffnung"),
    }
}

/// Wandöffnung als Intervall in mm ab Wandanfang.
fn loch(s: &Session, wand_id: Id) -> (f64, f64) {
    let w = s.doc.entity(wand_id).expect("Wand fehlt");
    let iv = jjcad_core::render::wall_opening_intervals(&s.doc, w);
    assert_eq!(iv.len(), 1, "genau eine Öffnung erwartet: {iv:?}");
    iv[0]
}

/// Freie Wandstrecke aus demselben Baustein, den die Actions benutzen.
fn strecke(s: &Session, wand_id: Id) -> (f64, f64) {
    jjcad_core::wall::wall_span(&s.doc, wand_id).expect("keine Wandstrecke")
}

const BREITE: f64 = 900.0;

#[test]
fn tuer_ueber_das_wandende_wird_gekappt() {
    let mut s = leer();
    let w = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    // Klick hinter dem Wandende: die Tür bekommt die letzte mögliche Lage,
    // ihre rechte Kante liegt genau auf der Stirn.
    let t = neu(
        &mut s,
        "draw.door",
        json!({"center": [5900, 0], "width": BREITE}),
    );
    let (at, center, breite) = lage(&s, t);
    assert_eq!(breite, BREITE);
    assert!(
        (at.unwrap() - (6000.0 - BREITE * 0.5)).abs() < 0.5,
        "at muss auf 5550 liegen: {at:?}"
    );
    assert!((center.x - 5550.0).abs() < 0.5, "center: {center:?}");
    let (s0, s1) = loch(&s, w);
    assert!(
        (s0 - 5100.0).abs() < 0.5 && (s1 - 6000.0).abs() < 0.5,
        "Öffnung muss im Wandende enden: {s0}…{s1}"
    );
}

#[test]
fn fenster_vor_dem_wandanfang_wird_gekappt() {
    let mut s = leer();
    let w = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    let f = neu(
        &mut s,
        "draw.window",
        json!({"center": [-400, 0], "width": 1400}),
    );
    let (at, center, _) = lage(&s, f);
    assert!(
        (at.unwrap() - 700.0).abs() < 0.5,
        "at muss auf 700 liegen: {at:?}"
    );
    assert!((center.x - 700.0).abs() < 0.5, "center: {center:?}");
    let (s0, _) = loch(&s, w);
    assert!(s0.abs() < 0.5, "Öffnung beginnt am Wandanfang: {s0}");
}

#[test]
fn verbundene_wand_begrenzt_die_lage() {
    let mut s = leer();
    // Rechtwinklige Ecke: dort endet die Strecke der liegenden Wand bei 0.
    let a = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    let _b = wand(&mut s, (0.0, 0.0), (0.0, 3000.0), 300.0);
    let (lo, hi) = strecke(&s, a);
    // Die Naht schneidet die Innenkante bei +h — erst dahinter ist die volle
    // Wanddicke im Körper (h = 150 bei 300 mm Wand).
    assert!(
        (lo - 150.0).abs() < 0.5,
        "Ecke beginnt eine halbe Wanddicke hinter dem Knoten: {lo}"
    );
    assert!((hi - 6000.0).abs() < 0.5);

    let t = neu(
        &mut s,
        "draw.door",
        json!({"center": [-300, 0], "width": BREITE}),
    );
    let (at, _, _) = lage(&s, t);
    assert!(
        (at.unwrap() - (lo + BREITE * 0.5)).abs() < 0.5,
        "an der Ecke beginnt die Tür an der Naht: at = {at:?}, Naht bei {lo}"
    );
}

#[test]
fn schraege_naht_schiebt_die_oeffnung_weiter_hinein() {
    let mut s = leer();
    // 45°-Anschluss am Anfang: die Naht schneidet die liegende Wand **vor**
    // ihrem Basispunkt — die freie Strecke beginnt erst hinter der Naht.
    let a = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    let _b = wand(&mut s, (0.0, 0.0), (2500.0, 2500.0), 300.0);
    let (lo, hi) = strecke(&s, a);
    assert!(
        lo > 100.0,
        "die Naht muss die Strecke vorn beschneiden, lo = {lo}"
    );
    assert!((hi - 6000.0).abs() < 0.5, "hinten frei: {hi}");

    let t = neu(
        &mut s,
        "draw.door",
        json!({"center": [-300, 0], "width": BREITE}),
    );
    let (at, _, _) = lage(&s, t);
    assert!(
        (at.unwrap() - (lo + BREITE * 0.5)).abs() < 0.5,
        "Tür beginnt an der Naht: at = {at:?}, Naht bei {lo}"
    );
    let (s0, _) = loch(&s, a);
    assert!(
        s0 >= lo - 0.5,
        "Öffnung darf nicht vor der Naht beginnen: {s0} < {lo}"
    );
}

#[test]
fn verschieben_endet_am_wandende() {
    let mut s = leer();
    let w = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    let t = neu(
        &mut s,
        "draw.door",
        json!({"center": [3000, 0], "width": BREITE}),
    );
    // Weit über das Wandende hinaus schieben: die Tür bleibt in der Wand.
    s.execute(
        "modify.move",
        &json!({"ids": [t], "from": [0, 0], "to": [6000, 0]}),
    )
    .unwrap();
    let (at, center, _) = lage(&s, t);
    assert!(
        (at.unwrap() - (6000.0 - BREITE * 0.5)).abs() < 0.5,
        "at muss am Wandende stehen: {at:?}"
    );
    assert!((center.x - 5550.0).abs() < 0.5, "center: {center:?}");
    let (_, s1) = loch(&s, w);
    assert!((s1 - 6000.0).abs() < 0.5, "Öffnung endet an der Wand: {s1}");
}

#[test]
fn verschieben_endet_an_der_anschlussnaht() {
    let mut s = leer();
    let a = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    let _b = wand(&mut s, (0.0, 0.0), (0.0, 3000.0), 300.0);
    let t = neu(
        &mut s,
        "draw.door",
        json!({"center": [2000, 0], "width": BREITE}),
    );
    s.execute(
        "modify.move",
        &json!({"ids": [t], "from": [0, 0], "to": [-3000, 0]}),
    )
    .unwrap();
    let (at, _, _) = lage(&s, t);
    let (lo, _) = strecke(&s, a);
    assert!(
        (at.unwrap() - (lo + BREITE * 0.5)).abs() < 0.5,
        "Tür steht an der Naht: at = {at:?}, Naht bei {lo}"
    );
}

#[test]
fn breitere_oeffnung_wird_zurueckgezogen() {
    let mut s = leer();
    let w = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    let f = neu(
        &mut s,
        "draw.window",
        json!({"center": [5600, 0], "width": 600}),
    );
    // Aus 600 werden 1600: die alte Lage ragte dann über das Ende hinaus.
    s.execute("modify.set", &json!({"ids": [f], "width": 1600}))
        .unwrap();
    let (at, _, breite) = lage(&s, f);
    assert!((breite - 1600.0).abs() < 0.5);
    assert!(
        (at.unwrap() - (6000.0 - 800.0)).abs() < 0.5,
        "at muss auf 5200 liegen: {at:?}"
    );
    let (s0, s1) = loch(&s, w);
    assert!(
        s0 > -0.5 && (s1 - 6000.0).abs() < 0.5,
        "Öffnung bleibt in der Wand: {s0}…{s1}"
    );
}

#[test]
fn zu_kurze_wand_ist_ein_fehler() {
    let mut s = leer();
    let w = wand(&mut s, (0.0, 0.0), (500.0, 0.0), 200.0);
    let err = s
        .execute("draw.door", &json!({"center": [250, 0], "width": BREITE}))
        .expect_err("Tür breiter als die Wand muss ein Fehler sein");
    assert!(err.contains("passt nicht"), "Meldung: {err}");
    assert!(
        !s.doc
            .entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Door { .. })),
        "nichts gesetzt"
    );
    assert!(jjcad_core::render::wall_opening_intervals(
        &s.doc,
        s.doc.entity(w).unwrap()
    )
    .is_empty());
}

#[test]
fn vorschau_zeigt_dieselbe_lage_wie_der_klick() {
    let mut s = leer();
    let w = wand(&mut s, (0.0, 0.0), (6000.0, 0.0), 300.0);
    let spec = jjcad_core::render::DoorSpec {
        host: Some(w),
        center: jjcad_core::geom::v(5900.0, 0.0),
        angle: 0.0,
        width: BREITE,
        kind: jjcad_core::model::DoorKind::Hinge,
        hinge: 1.0,
        swing_in: true,
    };
    let vor = jjcad_core::render::preview_door(&s.doc, &spec).expect("keine Vorschau");
    // Vorschau-Loch auf die Wandachse projizieren.
    let flaeche = vor.loch.first().expect("Vorschau ohne Loch");
    let (v0, v1) = flaeche.iter().fold((f64::MAX, f64::MIN), |(a, b), p| {
        (a.min(p.x), b.max(p.x))
    });

    neu(
        &mut s,
        "draw.door",
        json!({"center": [5900, 0], "width": BREITE}),
    );
    let (s0, s1) = loch(&s, w);
    assert!(
        (v0 - s0).abs() < 0.5 && (v1 - s1).abs() < 0.5,
        "Vorschau {v0}…{v1} gegen Tür {s0}…{s1}"
    );
}
