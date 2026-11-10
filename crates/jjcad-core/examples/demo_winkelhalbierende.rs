//! Erzeugt die Beispieldatei zum winkelhalbierenden Fang (T0150).
//!   cargo run -p jjcad-core --example demo_winkelhalbierende
//!
//! In der Datei liegen zwei Anordnungen:
//!
//! * eine **Ecke** aus zwei Linien (Schenkel nach +X und +Y, Scheitel bei
//!   1,00 / 1,00) — dort liegt die 45°-Halbierende,
//! * eine **Kreuzung** bei (4,00 / 9,00) — dort gibt es zwei Winkelpaare und
//!   damit zwei Halbierende.
//!
//! Prüfweg: Werkzeug „Linie" starten, den ersten Punkt **in den Scheitel**
//! setzen (er rastet dort als Endpunkt ein) und den zweiten neben die
//! Halbierende führen — der Zeiger rastet auf ihr ein, die Statuszeile meldet
//! „Winkelhalbierende". Zwischen den Linien (ohne Scheitel) schweigt der Fang.

use jjcad_core::model::Document;
use jjcad_core::persist;
use std::path::Path;

fn linie(doc: &mut Document, a: [f64; 2], b: [f64; 2]) -> Result<(), String> {
    jjcad_core::actions::execute(
        doc,
        "draw.line",
        &serde_json::json!({ "a": a, "b": b, "layer": "Wände" }),
    )?;
    Ok(())
}

fn main() -> Result<(), String> {
    let mut doc = Document::blank();
    // Ecke: Schenkel nach +X und +Y, Scheitel bei (1,00 / 1,00).
    linie(&mut doc, [1.0, 1.0], [7.0, 1.0])?;
    linie(&mut doc, [1.0, 1.0], [1.0, 6.0])?;
    // Kreuzung bei (4,00 / 9,00): zwei Winkelpaare, zwei Halbierende.
    linie(&mut doc, [1.0, 9.0], [7.0, 9.0])?;
    linie(&mut doc, [4.0, 7.0], [4.0, 11.0])?;
    let path = Path::new("samples/Winkelhalbierende.jjcad");
    persist::save_document(&mut doc, path)?;
    println!("geschrieben: {}", path.display());
    Ok(())
}
