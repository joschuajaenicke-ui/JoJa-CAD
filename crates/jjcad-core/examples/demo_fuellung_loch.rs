//! Erzeugt eine Beispieldatei mit einer Füllfläche, die ein Loch hat.
//!   cargo run -p jjcad-core --example demo_fuellung_loch

use jjcad_core::model::Document;
use jjcad_core::persist;
use std::path::Path;

fn main() -> Result<(), String> {
    let mut doc = Document::blank();
    // Aussenkante 6 x 4 m
    let outer = [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]];
    // Insel 1,5 x 1 m
    let hole = [[1.5, 1.5], [3.0, 1.5], [3.0, 2.5], [1.5, 2.5]];
    let params = serde_json::json!({
        "boundary": outer,
        "holes": [hole],
        "pattern": "lines",
        "fill": "#FFD24A",
        "color": "#000000",
    });
    jjcad_core::actions::execute(&mut doc, "draw.fill", &params)?;
    let path = Path::new("samples/Fuellung_Loch.jjcad");
    persist::save_document(&mut doc, path)?;
    println!("geschrieben: {}", path.display());
    Ok(())
}
