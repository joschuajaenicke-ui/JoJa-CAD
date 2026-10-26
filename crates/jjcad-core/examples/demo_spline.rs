//! Erzeugt die Beispieldatei der Splines (T0005).
//!   cargo run -p jjcad-core --example demo_spline
//!
//! In der Datei liegen drei Objekte: eine Polylinie mit denselben Stützpunkten
//! (zum Vergleich), ein offener Spline und ein geschlossener Spline. Damit ist
//! der Unterschied „eckig / glatt" in einem Bild zu sehen.

use jjcad_core::model::Document;
use jjcad_core::persist;
use std::path::Path;

fn main() -> Result<(), String> {
    let mut doc = Document::blank();
    // Gleiche Stützpunkte für alle drei: Polylinie daneben, Spline darüber.
    let punkte = [[0.5, 5.0], [2.5, 7.5], [5.0, 4.0], [7.5, 6.5], [9.5, 4.0]];
    jjcad_core::actions::execute(
        &mut doc,
        "draw.polyline",
        &serde_json::json!({"pts": punkte, "color": "#9AA3AD"}),
    )?;
    jjcad_core::actions::execute(
        &mut doc,
        "draw.spline",
        &serde_json::json!({"pts": punkte, "color": "#23262C", "lineweight": 0.5}),
    )?;
    // Geschlossener Spline: ein weiches Vieleck.
    let ring = [
        [2.0, 1.0],
        [4.0, 0.6],
        [5.5, 2.0],
        [4.5, 3.2],
        [2.5, 3.0],
        [1.5, 2.0],
    ];
    jjcad_core::actions::execute(
        &mut doc,
        "draw.spline",
        &serde_json::json!({"pts": ring, "closed": true, "color": "#2D69BE"}),
    )?;
    let path = Path::new("samples/Spline.jjcad");
    persist::save_document(&mut doc, path)?;
    println!("geschrieben: {}", path.display());
    Ok(())
}
