//! Tastaturbelegung nach T0009: T gehört dem Chat-Protokoll, nicht mehr dem
//! Text-Werkzeug. Eine alte gespeicherte Belegung darf die Taste nicht
//! zurückholen.

use jjcad_core::actions;
use jjcad_core::persist::UserPrefs;

/// Keine Aktion liegt mehr auf `t` — die Taste ist für den Chat frei.
#[test]
fn keine_aktion_auf_der_taste_t() {
    for a in actions::ACTIONS {
        assert_ne!(a.key.to_lowercase(), "t", "Aktion {} belegt noch T", a.id);
    }
    let text = actions::action("draw.text").expect("Text-Werkzeug fehlt");
    assert!(text.key.is_empty(), "Text-Werkzeug hat noch die Taste {}", text.key);
}

/// Eine prefs.json aus der Zeit vor T0009 verliert die T-Belegung des
/// Text-Werkzeugs, alle anderen Kürzel bleiben erhalten.
#[test]
fn alte_t_belegung_wird_beim_laden_entfernt() {
    let dir = std::env::temp_dir().join(format!("jjcad-prefs-t0009-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pfad = dir.join("prefs.json");
    let mut prefs = UserPrefs::default();
    prefs.bindings.insert("draw.text".into(), vec!["t".into()]);
    prefs.save_to(&pfad).unwrap();

    let geladen = UserPrefs::load_from(&pfad);
    assert_eq!(geladen.action_for_key("t"), None, "T ist weiterhin belegt");
    assert_eq!(geladen.key_for("draw.polyline").as_deref(), Some("o"));
    assert_eq!(geladen.key_for("draw.circle").as_deref(), Some("k"));
    std::fs::remove_dir_all(&dir).ok();
}
