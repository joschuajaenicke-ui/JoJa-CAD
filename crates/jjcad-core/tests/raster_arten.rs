//! Rasterarten, Rastermodus und erzwungener Rasterfang (T0144).
//!
//! Geprüft wird über die Action `doc.settings` und die öffentliche Fang-Engine
//! `snap::snap_constrained` — denselben Weg, den die Zeichenfläche
//! für Zeigeranzeige und Klickpunkt nimmt. Die Rasterart muss **auch fangen**;
//! eine Anzeige, die anders liegt als der Fang, wäre ein zweites Rastermodell.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::gitter::{Rasterart, Rastermodus};
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained, SnapKind, SnapSource};
use serde_json::json;

/// Sitzung in Millimetern mit Rasterfang an und 100 mm Weite.
fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "grid": true, "grid_step": 100.0 } }),
    )
    .unwrap();
    s
}

fn fangen(s: &Session, p: Vec2, tol: f64) -> Vec2 {
    snap_constrained(&s.doc, p, tol, None, &[], true).point
}

#[test]
fn rasterart_und_modus_kommen_ueber_die_action_an() {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "grid_kind": "isometrisch", "grid_mode": "fest", "grid_force": true } }),
    )
    .unwrap();
    assert_eq!(s.doc.settings.snap.grid_kind, Rasterart::Isometrisch);
    assert_eq!(s.doc.settings.snap.grid_mode, Rastermodus::Fest);
    assert!(s.doc.settings.snap.grid_force);

    // Auch flach (ohne `snap`-Objekt) — die Oberfläche schickt beides.
    s.execute(
        "doc.settings",
        &json!({ "grid_kind": "trimetrie", "grid_mode": "skaliert", "grid_force": false }),
    )
    .unwrap();
    assert_eq!(s.doc.settings.snap.grid_kind, Rasterart::Trimetrie);
    assert_eq!(s.doc.settings.snap.grid_mode, Rastermodus::Skaliert);
    assert!(!s.doc.settings.snap.grid_force);
}

#[test]
fn unbekannte_rasterart_aendert_nichts() {
    let mut s = s();
    s.execute("doc.settings", &json!({ "grid_kind": "quatsch" }))
        .unwrap();
    assert_eq!(s.doc.settings.snap.grid_kind, Rasterart::Rechteckig);
}

#[test]
fn isometrisches_raster_faengt_auf_seinen_achsen() {
    let mut s = s();
    s.execute("doc.settings", &json!({ "grid_kind": "isometrisch" }))
        .unwrap();
    // Gitterpunkt der Isometrie: 100 mm auf 30° = (86,60; 50,00).
    let ziel = v(100.0 * 30f64.to_radians().cos(), 100.0 * 30f64.to_radians().sin());
    let p = fangen(&s, ziel + v(3.0, -2.0), 12.0);
    assert!(
        (p - ziel).len() < 1e-6,
        "erwartet {:?}, gefangen {:?}",
        ziel,
        p
    );
    // Ein Punkt der Senkrechten (dritte Familie) liegt ebenfalls im Raster.
    let q = fangen(&s, v(0.0, 100.0) + v(2.0, 1.0), 12.0);
    assert!((q - v(0.0, 100.0)).len() < 1e-6, "gefangen {:?}", q);
}

#[test]
fn ohne_zwang_rastet_das_raster_nur_im_fangradius() {
    let s = s();
    // 40 mm neben dem Gitterpunkt, Fangradius 12 mm: kein Rasterfang.
    let p = fangen(&s, v(140.0, 40.0), 12.0);
    assert!((p - v(140.0, 40.0)).len() < 1e-9, "unerwartet gefangen: {:?}", p);
}

#[test]
fn erzwungener_rasterfang_rastet_auch_weit_abseits() {
    let mut s = s();
    s.execute("doc.settings", &json!({ "snap": { "grid_force": true } }))
        .unwrap();
    let p = fangen(&s, v(140.0, 40.0), 12.0);
    assert!((p - v(100.0, 0.0)).len() < 1e-9, "gefangen {:?}", p);
    // Auch bei winzigem Fangradius bleibt es im Raster.
    let q = fangen(&s, v(149.0, 51.0), 0.5);
    assert!((q - v(100.0, 100.0)).len() < 1e-9, "gefangen {:?}", q);
}

#[test]
fn erzwungener_rasterfang_laesst_objektpunkte_exakt() {
    let mut s = s();
    s.execute("doc.settings", &json!({ "snap": { "grid_force": true } }))
        .unwrap();
    // Eine Linie mit einem Ende abseits des Rasters (137, 41).
    s.execute(
        "draw.line",
        &json!({ "a": [137.0, 41.0], "b": [900.0, 41.0] }),
    )
    .unwrap();
    let res = snap_constrained(&s.doc, v(139.0, 42.0), 12.0, None, &[], true);
    assert_eq!(res.source, SnapSource::Object(SnapKind::Endpoint));
    assert!((res.point - v(137.0, 41.0)).len() < 1e-6, "Punkt {:?}", res.point);
}

#[test]
fn erzwungener_rasterfang_bleibt_auf_der_zwangsachse() {
    let mut s = s();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "grid_force": true, "ortho": true, "polar": false } }),
    )
    .unwrap();
    // Bezug (0,0), Zeiger bei (137, 41): Ortho zieht auf die X-Achse (137, 0),
    // das Raster rundet danach auf (100, 0) — beides gilt zugleich.
    let res = snap_constrained(
        &s.doc,
        v(137.0, 41.0),
        12.0,
        Some(v(0.0, 0.0)),
        &[],
        true,
    );
    assert!((res.point - v(100.0, 0.0)).len() < 1e-9, "Punkt {:?}", res.point);
}
