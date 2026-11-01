//! Erzeugt die Beispieldatei „Polylinie-Stutzen" (T0077).
//!   cargo run -p jjcad-core --example demo_polylinie_stutzen
//!
//! In der Datei liegt ein Polygonzug, den drei Linien queren: zwei auf dem
//! waagerechten Schenkel (x = 2,5 und x = 4,5), eine auf dem senkrechten
//! (y = 3,5). Mit dem Werkzeug „Stutzen" lässt sich der Zug an jeder dieser
//! Stellen kürzen. Bleibt dabei ein Stück mit nur zwei Punkten übrig, wird
//! daraus eine Linie.
//!
//! Rechts liegt zusätzlich ein Polygonzug, der sich **selbst kreuzt** — beide
//! Schenkel sind füreinander Schnittkante.

use jjcad_core::model::Document;
use jjcad_core::persist;
use serde_json::json;
use std::path::Path;

fn main() -> Result<(), String> {
    let mut doc = Document::blank();
    // Der Polygonzug: waagerecht von (0,5|2,0) nach (6,5|2,0), dann senkrecht
    // hinauf nach (6,5|5,0).
    jjcad_core::actions::execute(
        &mut doc,
        "draw.polyline",
        &json!({
            "pts": [[0.5, 2.0], [6.5, 2.0], [6.5, 5.0]],
            "color": "#23262C",
            "lineweight": 0.5,
            "layer": "Wände"
        }),
    )?;
    // Schnittkanten: zwei auf dem waagerechten Schenkel …
    for x in [2.5, 4.5] {
        jjcad_core::actions::execute(
            &mut doc,
            "draw.line",
            &json!({
                "a": [x, 1.0],
                "b": [x, 3.0],
                "color": "#9AA3AD",
                "lineweight": 0.25,
                "linetype": "Dashed"
            }),
        )?;
    }
    // … und eine auf dem senkrechten.
    jjcad_core::actions::execute(
        &mut doc,
        "draw.line",
        &json!({
            "a": [5.5, 3.5],
            "b": [7.5, 3.5],
            "color": "#9AA3AD",
            "lineweight": 0.25,
            "linetype": "Dashed"
        }),
    )?;
    // Zweiter Fall (T0077, Turn 2): ein Zug, der sich selbst kreuzt. Der
    // Schenkel (9|2)→(12|5) wird vom Schenkel (9|5)→(12|2) bei (10,5|3,5)
    // gekreuzt — Stutzen schneidet dort, ohne den ganzen Schenkel zu entfernen.
    jjcad_core::actions::execute(
        &mut doc,
        "draw.polyline",
        &json!({
            "pts": [[9.0, 2.0], [12.0, 5.0], [9.0, 5.0], [12.0, 2.0]],
            "color": "#23262C",
            "lineweight": 0.5,
            "layer": "Wände"
        }),
    )?;
    let path = Path::new("samples/Polylinie-Stutzen.jjcad");
    persist::save_document(&mut doc, path)?;
    println!("geschrieben: {}", path.display());
    Ok(())
}
