//! T0191: Der Basispunkt einer Tür bleibt auf der Wandkante.
//!
//! Das Verschieben mit der Maus und das Einfügen aus der Zwischenablage
//! schreiben die Objekte unmittelbar, ohne die Action-Schicht — die Längslage
//! `at` blieb dabei alt und der Querabstand zur Wand beliebig. Eine Tür stand
//! danach **neben** der Wand, ihr Basispunkt war frei verschiebbar.
//! `Session::oeffnungen_auf_wand` zieht beides nach: die Tür sitzt wieder auf
//! der vorderen Wandkante (Blattseite), längs begrenzt auf die freie
//! Wandstrecke.

use jjcad_core::geom::{v, Affine, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;

const BREITE: f64 = 900.0;
const DICKE: f64 = 300.0;
/// Klickpunkt = Basispunkt (Vorgabe „Links“); das Türzentrum liegt um die
/// halbe Breite daneben.
const KLICK: f64 = 2000.0;
const MITTE: f64 = KLICK + BREITE * 0.5;

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
        json!({"a": [0, 0], "b": [6000, 0], "thickness": DICKE}),
    )
}

fn tuer(s: &mut Session, host: Id, swing_in: bool) -> Id {
    neu(
        s,
        "draw.door",
        json!({
            "center": [KLICK, 0],
            "width": BREITE,
            "host": [host],
            "swing_in": swing_in,
        }),
    )
}

/// Genau der Weg der Maus: die Geometrie wird unmittelbar um `d` verschoben,
/// ohne Action und ohne Nachführung.
fn frei_verschieben(s: &mut Session, id: Id, d: Vec2) {
    let e = s.doc.entity(id).expect("Tür fehlt").clone();
    let moved = jjcad_core::transform::transform_entity(&e, &Affine::translate(d));
    *s.doc.entity_mut(id).expect("Tür fehlt") = moved;
}

fn lage(s: &Session, id: Id) -> (Option<f64>, Vec2) {
    match &s.doc.entity(id).expect("Tür fehlt").kind {
        EntityKind::Door { at, center, .. } => (*at, *center),
        _ => panic!("keine Tür"),
    }
}

/// Basispunkt der Tür (Marke der Zeichenfläche).
fn basispunkt(s: &Session, id: Id) -> Vec2 {
    let e = s.doc.entity(id).expect("Tür fehlt");
    jjcad_core::render::door_base_point(e).expect("kein Basispunkt")
}

#[test]
fn freies_verschieben_quer_zur_wand_setzt_die_tuer_zurueck() {
    let mut s = leer();
    let w = wand(&mut s);
    let t = tuer(&mut s, w, true);

    // 2,5 m quer zur Wand wegziehen: die Tür stünde weit neben der Wand.
    frei_verschieben(&mut s, t, v(0.0, 2500.0));
    let (_, weg) = lage(&s, t);
    assert!(
        (weg.y - (DICKE * 0.5 + 2500.0)).abs() < 0.5,
        "Vorbereitung: die Tür muss erst neben der Wand stehen, center={weg:?}"
    );
    let bp = basispunkt(&s, t);
    assert!(
        (bp.y - (DICKE * 0.5 + 2500.0)).abs() < 0.5,
        "Vorbereitung: Basispunkt muss die Wandkante verlassen haben: {bp:?}"
    );

    s.oeffnungen_auf_wand();

    let (at, center) = lage(&s, t);
    assert!(
        (center.y - DICKE * 0.5).abs() < 0.5,
        "Tür muss wieder auf der vorderen Wandkante sitzen: center={center:?}"
    );
    assert!(
        (at.unwrap() - MITTE).abs() < 0.5,
        "Längslage bleibt: at={:?}",
        at
    );
    let bp = basispunkt(&s, t);
    assert!(
        (bp.y - DICKE * 0.5).abs() < 0.5,
        "Basispunkt muss auf der Wandkante liegen: {bp:?}"
    );
    assert!(
        (bp.x - KLICK).abs() < 0.5,
        "Basispunkt bleibt an der Klickstelle: {bp:?}"
    );
}

#[test]
fn freies_verschieben_laengs_der_wand_wandert_weiterhin() {
    let mut s = leer();
    let w = wand(&mut s);
    let t = tuer(&mut s, w, true);

    frei_verschieben(&mut s, t, v(1200.0, 0.0));
    s.oeffnungen_auf_wand();

    let (at, center) = lage(&s, t);
    assert!(
        (at.unwrap() - (MITTE + 1200.0)).abs() < 0.5,
        "die Tür folgt dem Zug längs der Wand: at={:?}",
        at
    );
    assert!(
        (center.y - DICKE * 0.5).abs() < 0.5,
        "und bleibt dabei auf der Wandkante: center={center:?}"
    );
}

#[test]
fn freies_verschieben_ueber_das_wandende_kappt_die_tuer() {
    let mut s = leer();
    let w = wand(&mut s);
    let t = tuer(&mut s, w, true);

    frei_verschieben(&mut s, t, v(20000.0, -3000.0));
    s.oeffnungen_auf_wand();

    let (at, center) = lage(&s, t);
    assert!(
        (at.unwrap() - (6000.0 - BREITE * 0.5)).abs() < 0.5,
        "die Tür endet am Wandende: at={:?}",
        at
    );
    assert!(
        (center.y - DICKE * 0.5).abs() < 0.5,
        "und sitzt dabei auf der Wandkante: center={center:?}"
    );
    let bp = basispunkt(&s, t);
    assert!(
        (bp.y - DICKE * 0.5).abs() < 0.5,
        "Basispunkt auf der Wandkante: {bp:?}"
    );
}

#[test]
fn blattseite_b_legt_den_basispunkt_auf_die_andere_wanckante() {
    let mut s = leer();
    let w = wand(&mut s);
    let t = tuer(&mut s, w, false);

    frei_verschieben(&mut s, t, v(0.0, 4000.0));
    s.oeffnungen_auf_wand();

    let (_, center) = lage(&s, t);
    assert!(
        (center.y + DICKE * 0.5).abs() < 0.5,
        "Blattseite B liegt auf der anderen Wandkante: center={center:?}"
    );
    let bp = basispunkt(&s, t);
    assert!(
        (bp.y + DICKE * 0.5).abs() < 0.5,
        "Basispunkt folgt der Blattseite: {bp:?}"
    );
}

#[test]
fn fenster_kehrt_in_den_wandkoerper_zurueck() {
    let mut s = leer();
    let w = wand(&mut s);
    let f = neu(
        &mut s,
        "draw.window",
        json!({"center": [KLICK, 0], "width": 1400, "host": [w]}),
    );

    frei_verschieben(&mut s, f, v(0.0, 2500.0));
    s.oeffnungen_auf_wand();

    let center = match &s.doc.entity(f).expect("Fenster fehlt").kind {
        EntityKind::Window { center, .. } => *center,
        _ => panic!("kein Fenster"),
    };
    assert!(
        center.y.abs() < 0.5,
        "Fenster sitzt wieder mittig im Wandkörper: center={center:?}"
    );
}

#[test]
fn ohne_wand_bleibt_die_lage_stehen() {
    let mut s = leer();
    let w = wand(&mut s);
    let t = tuer(&mut s, w, true);
    let (at_vorher, center_vorher) = lage(&s, t);

    // Wand weg: die Tür hat keinen Bezug mehr — die Nachführung darf weder
    // panisch werden noch die Tür verschieben.
    s.execute("modify.erase", &json!({"ids": [w]})).unwrap();
    s.oeffnungen_auf_wand();

    let (at, center) = lage(&s, t);
    assert_eq!(at, at_vorher, "ohne Wand bleibt die Längslage stehen");
    assert!(
        (center.x - center_vorher.x).abs() < 0.5
            && (center.y - center_vorher.y).abs() < 0.5,
        "ohne Wand bleibt die Tür stehen: {center:?} statt {center_vorher:?}"
    );
}
