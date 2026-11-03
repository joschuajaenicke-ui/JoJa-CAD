//! Eigenschaften-Panel ohne Auswahl (T0011): der Abschnitt „Zeichnung" zeigt
//! Informationen über die Zeichnungsdatei und nur Einstellungen, die dort
//! hingehören (Einheit, Dezimalen, Rasterweite). Texthöhe und Wandstärke stehen
//! im Einstellungen-Dialog unter „Architektur", das Rasterfeld hat Grenzen.
//!
//! Der Test liest die Quellen der GUI, weil es um das Vorhandensein von
//! Bedienelementen geht — pro Panel eine Stelle, kein zweiter Weg.

use std::path::PathBuf;

fn gui_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../jjcad-gui/src")
}

/// Nur die Codezeilen — Kommentare nennen die Werte, um zu erklären, wo sie
/// stehen, und dürfen den Test nicht auslösen.
fn ohne_kommentare(src: &str) -> String {
    src.lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ausschnitt der Funktion `doc_props` (Abschnitt „Zeichnung" im Modell-Tab).
fn doc_props() -> String {
    let panels = std::fs::read_to_string(gui_dir().join("panels.rs")).expect("panels.rs");
    let start = panels.find("fn doc_props").expect("doc_props");
    let rest = &panels[start..];
    let ende = rest.find("/// Maßketten:").expect("dim_fields");
    ohne_kommentare(&rest[..ende])
}

/// Kommt das Wort in diesem Quelltext-Ausschnitt vor — wörtlich oder im Text
/// eines dort benutzten Katalogschlüssels? Seit T0104 steht der sichtbare Text
/// in `lang/de.toml`, im Quelltext nur der Schlüssel (`jjcad_core::i18n::tr`).
fn zeigt_wort(quelle: &str, wort: &str) -> bool {
    if quelle.contains(wort) {
        return true;
    }
    let katalog = jjcad_core::i18n::katalog(jjcad_core::i18n::Lang::Deutsch);
    jjcad_core::i18n::schluessel()
        .into_iter()
        .filter(|k| quelle.contains(&format!("\"{k}\"")))
        .filter_map(|k| katalog.get(k).copied())
        .any(|text| text.contains(wort))
}

#[test]
fn panel_zeigt_keine_texthoehe_und_keine_wandstaerke() {
    let panel = doc_props();
    for verboten in ["Texthöhe", "Wandstärke"] {
        assert!(
            !zeigt_wort(&panel, verboten),
            "Eigenschaften-Panel der Zeichnung enthält weiterhin {verboten}"
        );
    }
    for verboten in ["text_height", "wall_thickness"] {
        assert!(
            !panel.contains(verboten),
            "Eigenschaften-Panel der Zeichnung enthält weiterhin {verboten}"
        );
    }
    // Einheit, Dezimalen und Rasterweite bleiben (zeichnungsweite Anzeigewerte).
    for erlaubt in ["Einheit", "Dezimalen", "Raster"] {
        assert!(zeigt_wort(&panel, erlaubt), "Zeile {erlaubt} fehlt im Panel");
    }
}

#[test]
fn rasterfeld_im_panel_hat_grenzen() {
    let panel = doc_props();
    // Seit T0029 gibt es kein unbegrenztes Längenfeld mehr: `length_field`
    // verlangt Unter- und Obergrenze als Argument.
    assert!(
        panel.contains("length_field"),
        "Rasterweite ohne Eingabefeld"
    );
    assert!(
        panel.contains("GRID_STEP_MIN_MM") && panel.contains("GRID_STEP_MAX_MM"),
        "Grenzen der Rasterweite fehlen im Panel"
    );
}

#[test]
fn einstellungen_rasterweite_hat_dieselben_grenzen() {
    let dialogs = std::fs::read_to_string(gui_dir().join("dialogs.rs")).expect("dialogs.rs");
    let start = dialogs.find("fn settings_body").expect("settings_body");
    let rest = &dialogs[start..];
    let body = ohne_kommentare(&rest[..rest.find("\nfn ").unwrap_or(rest.len())]);
    assert!(
        body.contains("length_field"),
        "Rasterweite im Einstellungen-Dialog ohne Eingabefeld"
    );
    assert!(
        body.contains("GRID_STEP_MIN_MM") && body.contains("GRID_STEP_MAX_MM"),
        "Grenzen der Rasterweite fehlen im Einstellungen-Dialog"
    );
}
