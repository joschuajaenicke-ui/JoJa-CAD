//! T0091: Der Basispunkt einer Tür bestimmt, wo sie am Mauszeiger hängt.
//!
//! „Links" (Vorgabe) setzt das linke Türblattende an den Klickpunkt, „Mitte"
//! die Türmitte, „Rechts" das rechte Ende. Die Vorschau rechnet dieselbe Lage
//! wie die Action — sonst springt die Tür beim Klick.

use jjcad_core::geom::Vec2;
use jjcad_core::model::{DoorBase, EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;

const BREITE: f64 = 900.0;
/// Geklickter Punkt auf der Wandachse.
const KLICK: f64 = 2000.0;

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

fn wand(s: &mut Session) -> Id {
    neu(
        s,
        "draw.wall",
        json!({"a": [0, 0], "b": [6000, 0], "thickness": 300}),
    )
}

/// Tür setzen; `base` = None lässt den Vorgabewert der Action stehen.
fn tuer(s: &mut Session, host: Id, base: Option<DoorBase>) -> Id {
    let mut params = json!({"center": [KLICK, 0], "width": BREITE, "host": [host]});
    if let Some(b) = base {
        params["base"] = json!(b.key());
    }
    neu(s, "draw.door", params)
}

/// Längslage (`at`) und Mittelpunkt einer Tür.
fn lage(s: &Session, id: Id) -> (Option<f64>, Vec2) {
    match &s.doc.entity(id).expect("Tür fehlt").kind {
        EntityKind::Door { at, center, .. } => (*at, *center),
        _ => panic!("keine Tür"),
    }
}

/// Wandöffnung als Intervall in mm ab Wandanfang.
fn loch(s: &Session, wand_id: Id) -> (f64, f64) {
    let w = s.doc.entity(wand_id).expect("Wand fehlt");
    let iv = jjcad_core::render::wall_opening_intervals(&s.doc, w);
    assert_eq!(iv.len(), 1, "genau eine Öffnung erwartet: {iv:?}");
    iv[0]
}

/// Erwartete Längslage des Türzentrums je Basispunkt.
fn erwartet(base: DoorBase) -> f64 {
    KLICK - base.offset() * BREITE
}

#[test]
fn basispunkt_verschiebt_die_tuer_um_die_breite() {
    for base in DoorBase::ALL {
        let mut s = leer();
        let w = wand(&mut s);
        let t = tuer(&mut s, w, Some(base));
        let (at, center) = lage(&s, t);
        let soll = erwartet(base);
        assert!(
            (at.unwrap() - soll).abs() < 0.5,
            "{:?}: at={:?}, erwartet {soll}",
            base,
            at
        );
        assert!(
            (center.x - soll).abs() < 0.5,
            "{:?}: center={center:?}",
            base
        );
        // Das Loch liegt symmetrisch um das Zentrum.
        let (s0, s1) = loch(&s, w);
        assert!(
            (s0 - (soll - BREITE * 0.5)).abs() < 0.5 && (s1 - (soll + BREITE * 0.5)).abs() < 0.5,
            "{:?}: Loch {s0}…{s1}",
            base
        );
    }
}

#[test]
fn vorgabe_ist_links() {
    let mut s = leer();
    let w = wand(&mut s);
    let t = tuer(&mut s, w, None);
    let (at, _) = lage(&s, t);
    assert!(
        (at.unwrap() - erwartet(DoorBase::Left)).abs() < 0.5,
        "Vorgabe muss Links sein: at={:?}",
        at
    );
    // Der Klickpunkt ist das linke Ende der Öffnung.
    let (s0, _) = loch(&s, w);
    assert!((s0 - KLICK).abs() < 0.5, "linkes Ende bei {s0}");
}

#[test]
fn basispunkt_laesst_sich_am_objekt_aendern() {
    let mut s = leer();
    let w = wand(&mut s);
    let t = tuer(&mut s, w, Some(DoorBase::Left));
    s.execute("modify.set", &json!({"ids": [t], "base": "right"}))
        .unwrap();
    match &s.doc.entity(t).unwrap().kind {
        EntityKind::Door { base, .. } => assert_eq!(*base, DoorBase::Right),
        _ => panic!("keine Tür"),
    }
    // Die Marke wandert, die Tür selbst bleibt stehen.
    let (at, _) = lage(&s, t);
    assert!(
        (at.unwrap() - erwartet(DoorBase::Left)).abs() < 0.5,
        "Tür darf nicht wandern: at={:?}",
        at
    );
}

#[test]
fn vorschau_zeigt_dieselbe_lage_wie_der_klick() {
    for base in DoorBase::ALL {
        let mut s = leer();
        let w = wand(&mut s);
        let spec = jjcad_core::render::DoorSpec {
            host: Some(w),
            center: jjcad_core::geom::v(KLICK, 0.0),
            angle: 0.0,
            width: BREITE,
            kind: jjcad_core::model::DoorKind::Hinge,
            hinge: 1.0,
            swing_in: true,
            base,
        };
        let vor = jjcad_core::render::preview_door(&s.doc, &spec).expect("keine Vorschau");
        let flaeche = vor.loch.first().expect("Vorschau ohne Loch");
        let (v0, v1) = flaeche
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));

        let t = tuer(&mut s, w, Some(base));
        let (s0, s1) = loch(&s, w);
        assert!(
            (v0 - s0).abs() < 0.5 && (v1 - s1).abs() < 0.5,
            "{:?}: Vorschau {v0}…{v1}, platziert {s0}…{s1}",
            base
        );
        let _ = t;
    }
}

#[test]
fn basispunkt_marke_sitzt_am_richtigen_ende() {
    let mut s = leer();
    let w = wand(&mut s);
    for base in DoorBase::ALL {
        let t = tuer(&mut s, w, Some(base));
        let e = s.doc.entity(t).unwrap();
        let bp = jjcad_core::render::door_base_point(e).expect("kein Basispunkt");
        assert!(
            (bp.x - KLICK).abs() < 0.5,
            "{:?}: Basispunkt {bp:?} muss am Klickpunkt liegen",
            base
        );
    }
}
