//! Erzeugt die Beispieldatei „Kreis-Stutzen" (T0084, Turn 2).
//!   cargo run -p jjcad-core --example demo_kreis_stutzen
//!
//! Links liegen zwei gleich große Kreise, die sich schneiden — ein Klick mit
//! „Stutzen" auf die rechte Kappe des linken Kreises teilt ihn dort. Rechts ist
//! dieselbe Übung an einem **Bogen** bereits ausgeführt: der untere Teil fällt
//! weg, der Rest bleibt als Bogen stehen.

use jjcad_core::model::Document;
use jjcad_core::persist;
use serde_json::json;
use std::path::Path;

fn main() -> Result<(), String> {
    let mut doc = Document::blank();

    // ---- Links: zwei sich schneidende Kreise, noch ungestutzt ----
    for c in [[0.0, 0.0], [2.0, 0.0]] {
        jjcad_core::actions::execute(
            &mut doc,
            "draw.circle",
            &json!({
                "c": c,
                "r": 2.0,
                "color": "#23262C",
                "lineweight": 0.5,
                "layer": "Wände"
            }),
        )?;
    }

    // ---- Rechts: Bogen, an einer Senkrechten gestutzt ----
    let antwort = jjcad_core::actions::execute(
        &mut doc,
        "draw.arc",
        &json!({
            "c": [9.0, 0.0],
            "r": 2.0,
            "a0": 0,
            "a1": 180,
            "color": "#23262C",
            "lineweight": 0.5,
            "layer": "Wände"
        }),
    )?;
    let id = antwort["created"][0].as_u64().ok_or("Bogen ohne Id")?;
    jjcad_core::actions::execute(
        &mut doc,
        "draw.line",
        &json!({
            "a": [9.0, -3.0],
            "b": [9.0, 3.0],
            "color": "#9AA3AD",
            "lineweight": 0.25,
            "linetype": "Dashed"
        }),
    )?;
    // Klick bei 45° auf dem Bogen: der Abschnitt 0°…90° fällt weg.
    let klick = 2000.0 * 0.5f64.sqrt();
    jjcad_core::actions::execute(
        &mut doc,
        "modify.trim",
        &json!({
            "ids": [id],
            "at": [9000.0 + klick, klick],
            "units": "mm"
        }),
    )?;

    let path = Path::new("samples/Kreis-Stutzen.jjcad");
    persist::save_document(&mut doc, path)?;
    println!("geschrieben: {}", path.display());
    Ok(())
}