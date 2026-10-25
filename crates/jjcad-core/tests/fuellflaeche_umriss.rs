//! Füllfläche nach Umriss (`draw.fill.boundary`).
//!
//! Prüft die Abnahme der Karte F2: drei Punkte ergeben genau eine Füllfläche
//! mit drei Ecken und der richtigen Fläche, Undo nimmt sie wieder weg, und die
//! Action ist über MCP/Dispatch unter ihrem eigenen Namen erreichbar.

use jjcad_core::actions;
use jjcad_core::geom::polygon_area;
use jjcad_core::model::{Document, EntityKind};
use jjcad_core::persist::UserPrefs;
use jjcad_core::session::Session;
use jjcad_core::style::FillPattern;
use serde_json::json;

/// Dreieck 2 m × 1 m (Eingabeeinheit der neuen Zeichnung ist Meter):
/// halbe Fläche = 1 m² = 1 000 000 mm².
fn dreieck() -> serde_json::Value {
    json!({"boundary": [[0, 0], [2, 0], [0, 1]]})
}

fn fills(doc: &Document) -> Vec<(Vec<jjcad_core::geom::Vec2>, FillPattern, f64, f64, bool)> {
    doc.entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Fill {
                boundary,
                pattern,
                pattern_angle,
                pattern_scale,
                border,
                ..
            } => Some((
                boundary.clone(),
                *pattern,
                *pattern_angle,
                *pattern_scale,
                *border,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn drei_punkte_ergeben_genau_eine_flaeche_mit_drei_ecken() {
    let mut s = Session::new();
    s.execute("draw.fill.boundary", &dreieck()).expect("Umriss");

    assert_eq!(s.doc.entities.len(), 1, "genau ein Objekt");
    let alle = fills(&s.doc);
    assert_eq!(alle.len(), 1, "genau eine Füllfläche (keine Polylinie daneben)");

    let (ecken, _, _, _, _) = alle[0].clone();
    assert_eq!(ecken.len(), 3, "drei Eckpunkte");
    let flaeche = polygon_area(&ecken).abs();
    assert!(
        (flaeche - 1_000_000.0).abs() < 1e-6,
        "Fläche muss 1 m² sein, ist {flaeche} mm²"
    );
}

#[test]
fn muster_winkel_abstand_und_randlinie_kommen_aus_den_optionen() {
    let mut s = Session::new();
    s.execute(
        "draw.fill.boundary",
        &json!({"boundary": [[0, 0], [2000, 0], [2000, 1000], [0, 1000]],
                "pattern": "kreuz",
                "angle": 30.0,
                "scale": 0.25,
                "border": false}),
    )
    .expect("Umriss");

    let (_, muster, winkel, abstand, rand) = fills(&s.doc)[0].clone();
    assert_eq!(muster, FillPattern::Cross, "Muster aus dem Panel");
    assert!((winkel - 30f64.to_radians()).abs() < 1e-9, "Winkel 30°");
    assert!((abstand - 250.0).abs() < 1e-9, "0,25 m = 250 mm: {abstand}");
    assert!(!rand, "Randlinie aus den Optionen");
}

#[test]
fn zu_wenige_punkte_werden_abgewiesen() {
    let mut s = Session::new();
    let res = s.execute("draw.fill.boundary", &json!({"boundary": [[0, 0], [2000, 0]]}));
    assert!(res.is_err(), "zwei Punkte ergeben keine Fläche");
    assert!(s.doc.entities.is_empty(), "nichts angelegt");
}

#[test]
fn ohne_punktliste_bleibt_das_dokument_leer() {
    let mut s = Session::new();
    assert!(s.execute("draw.fill.boundary", &json!({})).is_err());
    assert!(s.doc.entities.is_empty());
}

#[test]
fn undo_nimmt_die_flaeche_wieder_weg() {
    let mut s = Session::new();
    s.execute("draw.fill.boundary", &dreieck()).expect("Umriss");
    assert_eq!(s.doc.entities.len(), 1);

    assert!(s.undo(), "ein Schritt in der Historie");
    assert!(s.doc.entities.is_empty(), "Undo entfernt die Füllfläche");

    assert!(s.redo(), "Wiederholen möglich");
    assert_eq!(s.doc.entities.len(), 1, "Wiederholen bringt sie zurück");
}

/// Die Action steht unter ihrem eigenen Namen im Katalog (Ribbon, Tastatur,
/// Inventar und MCP lesen dieselbe Liste) und trägt die Taste I.
#[test]
fn eigener_name_und_taste_i_im_katalog() {
    let def = actions::action("draw.fill.boundary").expect("Action fehlt");
    assert_eq!(def.key, "i", "Tastenkürzel I");
    assert!(def.tool, "Werkzeug mit Klickpfad");
    assert!(actions::is_mutating("draw.fill.boundary"));
    // eigener Eintrag, nicht der Name der anderen beiden Modi
    let ids: Vec<&str> = actions::actions_in_group(actions::ActionGroup::Zeichnen)
        .iter()
        .map(|a| a.id)
        .collect();
    assert!(ids.contains(&"draw.fill.boundary"));
    assert!(ids.contains(&"draw.fill"));
}

/// Gespeicherte Einstellungen tragen die Tastaturbelegung als Ganzes. Eine
/// später ergänzte Aktion muss ihre Werktaste trotzdem bekommen — sonst wäre I
/// bei jedem bestehenden Nutzer wirkungslos.
#[test]
fn neue_aktion_bekommt_ihre_taste_bei_alten_einstellungen() {
    let alt = json!({
        "bindings": {
            "draw.polyline": "o",
            "draw.wall": "g",
            "view.inventory": "e"
        }
    });
    let datei = std::env::temp_dir().join("jjcad_prefs_umriss_test.json");
    std::fs::write(&datei, alt.to_string()).unwrap();
    let prefs = UserPrefs::load_from(&datei);
    let _ = std::fs::remove_file(&datei);

    assert_eq!(
        prefs.action_for_key("i").as_deref(),
        Some("draw.fill.boundary"),
        "Taste I gehört der neuen Aktion"
    );
    // vorhandene Belegungen bleiben unangetastet
    assert_eq!(prefs.key_for("draw.polyline").as_deref(), Some("o"));
}
