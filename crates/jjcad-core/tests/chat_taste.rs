//! Tastaturbelegung: `T` gehört dem Text-Werkzeug (T0079). Zwischenzeitlich
//! lag T auf dem Chat-Protokoll; das sitzt seit T0009 auf `^` (Taste unter
//! ESC), eine alte Belegung darf T also nicht mehr verdrängen.

use jjcad_core::actions;
use jjcad_core::persist::UserPrefs;

/// Das Text-Werkzeug trägt `t` — so steht es in der Aufgabenstellung T0079.
#[test]
fn text_werkzeug_liegt_auf_der_taste_t() {
    let text = actions::action("draw.text").expect("Text-Werkzeug fehlt");
    assert_eq!(text.key, "t", "Text-Werkzeug muss auf T liegen");
    assert_eq!(
        UserPrefs::default().action_for_key("t").as_deref(),
        Some("draw.text"),
        "T muss in der Belegung den Text starten"
    );
    // Genau eine Aktion liegt auf T.
    let auf_t: Vec<&str> = actions::ACTIONS
        .iter()
        .filter(|a| a.key.eq_ignore_ascii_case("t"))
        .map(|a| a.id)
        .collect();
    assert_eq!(auf_t, vec!["draw.text"], "T muss genau einer Funktion gehören");
}

/// Eine prefs.json aus der Zeit vor T0079 (Text ohne Taste) bekommt T beim
/// Laden nach; alle anderen Kürzel bleiben erhalten.
#[test]
fn alte_belegung_ohne_t_bekommt_sie_beim_laden() {
    let dir = std::env::temp_dir().join(format!("jjcad-prefs-t0079-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pfad = dir.join("prefs.json");
    let mut prefs = UserPrefs::default();
    prefs.bindings.remove("draw.text");
    prefs.save_to(&pfad).unwrap();

    let geladen = UserPrefs::load_from(&pfad);
    assert_eq!(
        geladen.action_for_key("t").as_deref(),
        Some("draw.text"),
        "T ist nach dem Laden unbelegt"
    );
    assert_eq!(geladen.key_for("draw.polyline").as_deref(), Some("o"));
    assert_eq!(geladen.key_for("draw.circle").as_deref(), Some("k"));
    std::fs::remove_dir_all(&dir).ok();
}