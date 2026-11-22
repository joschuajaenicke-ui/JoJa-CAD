//! T0228 — Modell und Papierraum haben **eigene** Eingabeeinheiten.
//!
//! Das Modell beginnt in Metern, das Papier in Millimetern (Blattecken
//! (0,0)…(420,297), Texthöhe 2,5 mm). Die Befehle lesen ihre Vorgabe aus
//! `Document::input_unit` — dieselbe Abfrage, mit der die Oberfläche ihre
//! Felder beschriftet. Vorher las core immer `settings.input_unit`, dadurch
//! wurde aus einer 100 mm langen Linie im Blatt eine von 100 m.

use jjcad_core::model::{Entity, EntityKind};
use jjcad_core::persist::{load_document, save_document};
use jjcad_core::session::Session;
use jjcad_core::units::Unit;
use serde_json::json;

/// Länge einer Linie in mm.
fn laenge(e: &Entity) -> f64 {
    match &e.kind {
        EntityKind::Line { a, b } => a.dist(*b),
        _ => panic!("keine Linie"),
    }
}

fn session_mit_blatt() -> (Session, u64) {
    let mut s = Session::new();
    // Modell ausdrücklich in Metern, Papier bei seiner Vorgabe (mm).
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    let sid = s.doc.sheets[0].id;
    (s, sid)
}

/// Ohne `units`-Parameter liest der Befehl die Einheit des **aktiven** Raums.
#[test]
fn linie_im_papierraum_wird_in_millimetern_gelesen() {
    let (mut s, sid) = session_mit_blatt();
    assert_eq!(s.doc.settings.input_unit, Unit::M, "Modell in Metern");
    assert_eq!(
        s.doc.settings.paper_input_unit,
        Unit::Mm,
        "Papier ist von Haus aus in Millimetern"
    );

    // Modell: 100 heißt 100 m.
    let antwort = s
        .execute("draw.line", &json!({"a": [0, 0], "b": [100, 0]}))
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let laenge_modell = laenge(&s.doc.entities.iter().find(|e| e.id == id).cloned().unwrap());
    assert_eq!(laenge_modell, 100_000.0, "100 m im Modell");

    // Papierraum: dieselbe Zahl heißt 100 mm.
    s.set_space(Some(sid));
    assert!(s.doc.in_paper_space());
    let antwort = s
        .execute("draw.line", &json!({"a": [50, 50], "b": [150, 50]}))
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let sheet = s.doc.sheet(sid).unwrap();
    let e = sheet.entities.iter().find(|e| e.id == id).cloned().unwrap();
    let laenge_blatt = laenge(&e);
    assert_eq!(laenge_blatt, 100.0, "100 mm im Papierraum");
}

/// Die beiden Einheiten sind getrennt einstellbar — eine Wahl im Papierraum
/// ändert das Modell nicht.
#[test]
fn einheiten_sind_getrennt_einstellbar() {
    let (mut s, sid) = session_mit_blatt();
    s.set_space(Some(sid));
    s.execute("doc.settings", &json!({"paper_input_unit": "cm"}))
        .unwrap();
    assert_eq!(s.doc.settings.paper_input_unit, Unit::Cm);
    assert_eq!(
        s.doc.settings.input_unit,
        Unit::M,
        "das Modell behält seine Einheit"
    );

    // 12 cm im Papierraum sind 120 mm.
    let antwort = s
        .execute("draw.line", &json!({"a": [0, 0], "b": [12, 0]}))
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let e = s
        .doc
        .sheet(sid)
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == id)
        .cloned()
        .unwrap();
    assert_eq!(laenge(&e), 120.0);

    // Zurück ins Modell: dort gilt weiter Meter.
    s.set_space(None);
    assert_eq!(s.doc.input_unit(), Unit::M);
    let antwort = s
        .execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]}))
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let e = s.doc.entities.iter().find(|e| e.id == id).cloned().unwrap();
    assert_eq!(laenge(&e), 2000.0, "2 m im Modell");
}

/// `units` am Befehl sticht die Raum-Einheit (MCP und Skripte rechnen dann
/// ausdrücklich in einer anderen).
#[test]
fn units_parameter_sticht_die_raum_einheit() {
    let (mut s, sid) = session_mit_blatt();
    s.set_space(Some(sid));
    assert_eq!(s.doc.input_unit(), Unit::Mm);
    let antwort = s
        .execute(
            "draw.line",
            &json!({"a": [0, 0], "b": [1, 0], "units": "m"}),
        )
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let e = s
        .doc
        .sheet(sid)
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == id)
        .cloned()
        .unwrap();
    assert_eq!(laenge(&e), 1000.0, "1 m = 1000 mm");
}

/// Die Wahl steht in der Datei: nach Neuladen ist das Papier weiter in
/// Zentimetern, das Modell in Metern.
#[test]
fn papier_einheit_ueberlebt_das_speichern() {
    let (mut s, sid) = session_mit_blatt();
    s.set_space(Some(sid));
    s.execute("doc.settings", &json!({"paper_input_unit": "mm"}))
        .unwrap();
    let datei = std::env::temp_dir().join(format!("t0228-{}.jjcad", std::process::id()));
    save_document(&mut s.doc, &datei).unwrap();
    let roh: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&datei).unwrap()).unwrap();
    assert_eq!(
        roh["settings"]["paper_input_unit"],
        json!("Mm"),
        "Papier-Einheit steht in der Datei"
    );
    let wieder = load_document(&datei).unwrap();
    assert_eq!(wieder.settings.paper_input_unit, Unit::Mm);
    assert_eq!(wieder.settings.input_unit, Unit::M);
    std::fs::remove_file(&datei).ok();
}
