//! Rasterweite (Fangraster): Grenzen und Einheiten.
//!
//! Die Rasterweite ist eine Zeichnungseinstellung in mm. Ohne Grenzen kamen
//! negative und beliebig große Werte ins Dokument (T0011): negativ schaltet den
//! Rasterfang still ab (beide Snap-Pfade prüfen `> 0`), riesig rastet ins Nichts.
//! Diese Datei prüft die Action `doc.settings` in beiden Formen — flach und im
//! `snap`-Objekt — und das Laden einer Datei mit Altlasten.

use jjcad_core::model::{Document, GRID_STEP_MAX_MM, GRID_STEP_MIN_MM};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    Session::new()
}

#[test]
fn negative_rasterweite_landet_an_der_untergrenze() {
    let mut s = s();
    s.execute("doc.settings", &json!({"grid_step": -50.0}))
        .expect("doc.settings");
    assert_eq!(s.doc.settings.snap.grid_step, GRID_STEP_MIN_MM);
}

#[test]
fn riesige_rasterweite_landet_an_der_obergrenze() {
    let mut s = s();
    s.execute("doc.settings", &json!({"grid_step": 1.0e9}))
        .expect("doc.settings");
    assert_eq!(s.doc.settings.snap.grid_step, GRID_STEP_MAX_MM);
}

#[test]
fn gueltige_rasterweite_bleibt_unveraendert() {
    let mut s = s();
    // Zeichnungen beginnen in Metern: 250 wäre 250 m und wird begrenzt.
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .expect("Einheit");
    s.execute("doc.settings", &json!({"grid_step": 250.0}))
        .expect("doc.settings");
    assert_eq!(s.doc.settings.snap.grid_step, 250.0);
}

#[test]
fn rasterweite_im_snap_objekt_wird_in_millimeter_umgerechnet() {
    // 0,25 m müssen 250 mm werden — `num` statt `len` legte 0,25 mm an, sobald
    // die Eingabeeinheit nicht Millimeter war.
    let mut s = s();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    s.execute("doc.settings", &json!({"snap": {"grid_step": 0.25}}))
        .expect("snap");
    assert_eq!(s.doc.settings.snap.grid_step, 250.0);
}

#[test]
fn rasterweite_im_snap_objekt_wird_begrenzt() {
    let mut s = s();
    s.execute("doc.settings", &json!({"snap": {"grid_step": -1.0}}))
        .expect("negativ");
    assert_eq!(s.doc.settings.snap.grid_step, GRID_STEP_MIN_MM);
    s.execute("doc.settings", &json!({"snap": {"grid_step": 250000.0}}))
        .expect("zu groß");
    assert_eq!(s.doc.settings.snap.grid_step, GRID_STEP_MAX_MM);
}

#[test]
fn alte_datei_mit_unsinniger_rasterweite_wird_beim_laden_begrenzt() {
    let mut doc = Document::new();
    doc.settings.snap.grid_step = -42.0;
    let json_text = jjcad_core::persist::to_json(&doc).expect("schreiben");
    // Die Datei selbst trägt den alten Wert …
    assert!(json_text.contains("-42"), "Testdatei ohne Altlast");
    // … beim Laden wird sie begrenzt.
    let doc2 = jjcad_core::persist::from_json(&json_text).expect("laden");
    assert_eq!(doc2.settings.snap.grid_step, GRID_STEP_MIN_MM);
}
