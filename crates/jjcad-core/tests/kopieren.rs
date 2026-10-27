//! Zwei Kopierfunktionen (T0012).
//!
//! „Kopieren" legt die Auswahl in die Zwischenablage (Strg+C), „Kopieren mit
//! Basispunkt" setzt eine Kopie versetzt ab (Strg+Umschalt+C). Beide sind
//! getrennte Aktionen — ohne eigenen Eintrag hätte der Ribbon keinen Modus zum
//! Umschalten, und Strg+C hätte wieder zwei Bedeutungen.

use jjcad_core::actions::{action, ACTIONS};
use jjcad_core::session::Session;
use serde_json::json;

#[test]
fn kopieren_und_kopieren_mit_basispunkt_sind_getrennt() {
    let kopieren = action("edit.copy").expect("Action edit.copy fehlt");
    let mit_basispunkt = action("modify.copy").expect("Action modify.copy fehlt");

    assert_eq!(kopieren.label, "Kopieren");
    assert_eq!(kopieren.key, "ctrl+c", "Strg+C ist reines Kopieren");
    assert!(!kopieren.tool, "Kopieren in die Zwischenablage braucht keine Punkte");
    assert_eq!(kopieren.icon, "copy");

    assert_eq!(mit_basispunkt.label, "Kopieren mit Basispunkt");
    assert_eq!(
        mit_basispunkt.key, "ctrl+shift+c",
        "Kopieren mit Basispunkt liegt auf Strg+Umschalt+C"
    );
    assert!(mit_basispunkt.tool, "Kopieren mit Basispunkt sammelt Punkte");
    assert_eq!(
        mit_basispunkt.icon, "copy-base",
        "Icon mit Basispunkt (kleiner Punkt unten links)"
    );
}

/// Kein zweiter Eintrag darf Strg+C oder Strg+Umschalt+C belegen — sonst
/// entscheidet die Reihenfolge im Register über die Taste.
#[test]
fn tastenkuerzel_sind_eindeutig() {
    for (key, erlaubt) in [("ctrl+c", "edit.copy"), ("ctrl+shift+c", "modify.copy")] {
        let belegt: Vec<&str> = ACTIONS
            .iter()
            .filter(|a| a.key.eq_ignore_ascii_case(key))
            .map(|a| a.id)
            .collect();
        assert_eq!(belegt, vec![erlaubt], "Taste {key} mehrfach belegt: {belegt:?}");
    }
}

/// Die Action-Schicht kennt beide Wege: verschieben (`modify.move`) und
/// kopieren (`modify.copy`) — die Kopie lässt das Original stehen.
#[test]
fn action_schicht_legt_eine_kopie_an() {
    let mut s = Session::new();
    let r = s
        .execute("draw.rect", &json!({"a": [0, 0], "b": [100, 100]}))
        .expect("Rechteck");
    let id = r["created"][0].as_u64().unwrap();
    let vorher = s.doc.ents().len();

    let out = s
        .execute("modify.copy", &json!({"ids": [id], "from": [0, 0], "to": [500, 0]}))
        .expect("Kopieren mit Basispunkt");

    assert_eq!(s.doc.ents().len(), vorher + 1, "die Kopie kommt dazu");
    let neu = out["created"][0].as_u64().unwrap();
    assert_ne!(neu, id);
}
