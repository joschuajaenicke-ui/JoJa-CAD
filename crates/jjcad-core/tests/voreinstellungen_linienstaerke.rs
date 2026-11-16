//! T0125 — Voreinstellungen der Linienstärke je Objektart.
//!
//! Die Zeilen der Einstellungen kommen aus `style::LAYER_DEFAULTS`; die Breite
//! selbst steht **nicht** mehr dort, sondern in `prefs.json`
//! (`UserPrefs::layer_weights`). Vorgabe für jede Objektart: 0,18 mm.
//! Bestehende Ebenen einer geöffneten Zeichnung bleiben unangetastet.

use jjcad_core::persist::UserPrefs;
use jjcad_core::style::{LAYER_DEFAULTS, LINIENSTAERKE_VORGABE};
use jjcad_core::Document;
use std::collections::BTreeMap;

fn breiten(p: &[(&str, f64)]) -> BTreeMap<String, f64> {
    p.iter().map(|(n, w)| (n.to_string(), *w)).collect()
}

/// Eine Zeile je Layer — seit T0202 die vier Layer eines Plans.
#[test]
fn die_liste_ist_layer_defaults_mit_vier_layern() {
    let namen: Vec<&str> = LAYER_DEFAULTS.iter().map(|(n, _, _)| *n).collect();
    assert_eq!(
        namen,
        vec!["Zeichnung", "Architektur", "Beschriftung", "Blöcke"]
    );
    assert_eq!(LINIENSTAERKE_VORGABE, 0.18, "Programm-Vorgabe für alle Layer");
}

/// Gesetzte Werte überleben Speichern und Laden; nicht gesetzte bleiben 0,18.
#[test]
fn voreinstellungen_ueberleben_speichern_und_laden() {
    let dir = std::env::temp_dir().join(format!("jjcad-t0125-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pfad = dir.join("prefs.json");

    let mut prefs = UserPrefs::default();
    prefs.set_linienstaerke("Architektur", 0.50);
    prefs.set_linienstaerke("Zeichnung", 0.13);
    prefs.save_to(&pfad).unwrap();

    let geladen = UserPrefs::load_from(&pfad);
    assert!((geladen.linienstaerke("Architektur") - 0.50).abs() < 1e-9);
    assert!((geladen.linienstaerke("Zeichnung") - 0.13).abs() < 1e-9);
    assert!((geladen.linienstaerke("Blöcke") - 0.18).abs() < 1e-9, "nicht gesetzt = 0,18");
    std::fs::remove_dir_all(&dir).ok();
}

/// Ältere `prefs.json` ohne das Feld bleibt gültig und ergibt 0,18.
#[test]
fn fehlendes_feld_ergibt_018() {
    let alt: UserPrefs = serde_json::from_str("{}").expect("alte Datei ohne layer_weights");
    for (name, _, _) in LAYER_DEFAULTS {
        assert!(
            (alt.linienstaerke(name) - 0.18).abs() < 1e-9,
            "{name} muss ohne Einstellung 0,18 tragen"
        );
    }
    // Auch ein leerer Eintrag im Feld selbst zählt als „nichts gewählt“.
    let leer: UserPrefs =
        serde_json::from_str("{\"layer_weights\": {}}").expect("leere Tabelle");
    assert!((leer.linienstaerke("Architektur") - 0.18).abs() < 1e-9);
}

/// Eine neue Zeichnung trägt ohne Einstellung auf **jeder** Standardebene 0,18.
#[test]
fn neue_zeichnung_hat_fuer_alle_ebenen_018() {
    let doc = Document::new();
    assert_eq!(doc.layers.len(), LAYER_DEFAULTS.len());
    for (name, _, _) in LAYER_DEFAULTS {
        let l = doc.layer_by_name(name).expect("Standardebene fehlt");
        assert!(
            (l.line_weight - 0.18).abs() < 1e-9,
            "{name}: {} statt 0,18",
            l.line_weight
        );
    }
}

/// Die eingestellten Breiten landen in den Ebenen einer neuen Zeichnung.
#[test]
fn neue_zeichnung_uebernimmt_die_einstellungen() {
    let mut doc = Document::new();
    doc.setze_standard_breiten(&breiten(&[("Architektur", 0.50), ("Beschriftung", 0.25)]));
    assert!((doc.layer_by_name("Architektur").unwrap().line_weight - 0.50).abs() < 1e-9);
    assert!((doc.layer_by_name("Beschriftung").unwrap().line_weight - 0.25).abs() < 1e-9);
    // Nicht gesetzte Layer bleiben bei der Vorgabe.
    assert!((doc.layer_by_name("Blöcke").unwrap().line_weight - 0.18).abs() < 1e-9);
}

/// Eine offene Zeichnung behält ihre Ebenenbreiten, auch wenn die
/// Voreinstellungen inzwischen andere Werte tragen.
#[test]
fn bestehende_zeichnung_behaelt_ihre_breiten() {
    let mut doc = Document::new();
    doc.setze_standard_breiten(&breiten(&[("Architektur", 0.70)]));
    let text = jjcad_core::persist::to_json(&doc).expect("JSON");

    let mut wieder = jjcad_core::persist::from_json(&text).expect("geladen");
    // Der Nutzer ändert danach die Voreinstellungen.
    wieder.merke_standard_breiten(&breiten(&[("Architektur", 0.13), ("Zeichnung", 0.13)]));
    assert!(
        (wieder.layer_by_name("Architektur").unwrap().line_weight - 0.70).abs() < 1e-9,
        "bestehende Ebene darf sich nicht ändern"
    );
    assert!((wieder.layer_by_name("Zeichnung").unwrap().line_weight - 0.18).abs() < 1e-9);
    // Und die gespeicherte Breite kommt beim nächsten Laden unverändert zurück.
    let text2 = jjcad_core::persist::to_json(&wieder).expect("JSON");
    let dritt = jjcad_core::persist::from_json(&text2).expect("geladen");
    assert!((dritt.layer_by_name("Architektur").unwrap().line_weight - 0.70).abs() < 1e-9);
}

/// Ein Layer, der erst später entsteht, bekommt die eingestellte
/// Voreinstellung — freie Ebenen nicht.
#[test]
fn neue_standardebene_bekommt_die_voreinstellung() {
    let mut doc = Document::new();
    doc.merke_standard_breiten(&breiten(&[("Zeichnung", 0.35)]));
    let id = doc.ensure_layer("Eigene", jjcad_core::style::Color::BLACK);
    let l = doc.layer(id).expect("Ebene");
    assert!((l.line_weight - 0.25).abs() < 1e-9, "frei: {}", l.line_weight);
    let id = doc.ensure_layer_in(
        "Zeichnung",
        jjcad_core::style::Color::BLACK,
        doc.plans[0].id,
    );
    let l = doc.layer(id).expect("Ebene");
    assert!((l.line_weight - 0.18).abs() < 1e-9, "Zeichnung: {}", l.line_weight);

    let frei = doc.ensure_layer("Eigene", jjcad_core::style::Color::BLACK);
    assert!(
        (doc.layer(frei).unwrap().line_weight - 0.25).abs() < 1e-9,
        "freie Ebene behält die Grundbreite"
    );
}

/// „Vorgaben zurücksetzen“ stellt jede Objektart wieder auf 0,18.
#[test]
fn zuruecksetzen_stellt_alle_zeilen_auf_018() {
    let mut prefs = UserPrefs::default();
    prefs.set_linienstaerke("Architektur", 0.50);
    prefs.set_linienstaerke("Zeichnung", 0.13);
    prefs.linienstaerken_zuruecksetzen();
    for (name, _, _) in LAYER_DEFAULTS {
        assert!((prefs.linienstaerke(name) - 0.18).abs() < 1e-9, "{name}");
    }
    assert!(prefs.layer_weights.is_empty(), "Vorgabewerte werden nicht gespeichert");
}
