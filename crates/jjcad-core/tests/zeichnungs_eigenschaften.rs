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

#[test]
fn panel_zeigt_keine_texthoehe_und_keine_wandstaerke() {
    let panel = doc_props();
    for verboten in ["Texthöhe", "Wandstärke", "text_height", "wall_thickness"] {
        assert!(
            !panel.contains(verboten),
            "Eigenschaften-Panel der Zeichnung enthält weiterhin {verboten}"
        );
    }
    // Einheit, Dezimalen und Rasterweite bleiben (zeichnungsweite Anzeigewerte).
    for erlaubt in ["Einheit", "Dezimalen", "Raster"] {
        assert!(panel.contains(erlaubt), "Zeile {erlaubt} fehlt im Panel");
    }
}

#[test]
fn rasterfeld_im_panel_hat_grenzen() {
    let panel = doc_props();
    assert!(
        panel.contains("length_input_between"),
        "Rasterweite ohne Grenzen (length_input statt length_input_between)"
    );
    assert!(
        panel.contains("GRID_STEP_MIN_MM") && panel.contains("GRID_STEP_MAX_MM"),
        "Grenzen der Rasterweite fehlen im Panel"
    );
    assert!(
        !panel.contains("length_input(ui, &mut grid"),
        "Rasterweite nutzt weiterhin das unbegrenzte Feld"
    );
}

#[test]
fn einstellungen_rasterweite_hat_dieselben_grenzen() {
    let dialogs = std::fs::read_to_string(gui_dir().join("dialogs.rs")).expect("dialogs.rs");
    let start = dialogs.find("fn settings_body").expect("settings_body");
    let rest = &dialogs[start..];
    let body = ohne_kommentare(&rest[..rest.find("\nfn ").unwrap_or(rest.len())]);
    assert!(
        body.contains("length_input_between"),
        "Rasterweite im Einstellungen-Dialog ohne Grenzen"
    );
    assert!(
        body.contains("GRID_STEP_MIN_MM") && body.contains("GRID_STEP_MAX_MM"),
        "Grenzen der Rasterweite fehlen im Einstellungen-Dialog"
    );
}
