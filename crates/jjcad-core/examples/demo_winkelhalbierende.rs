//! Erzeugt die Beispieldatei zum winkelhalbierenden Fang (T0150).
//!   cargo run -p jjcad-core --example demo_winkelhalbierende
//!
//! In der Datei liegen zwei Anordnungen:
//!
//! * ein **rechter Winkel** aus zwei Linien (Schenkel nach +X und +Y, Ecke bei
//!   1,00 / 1,00) — ein Punkt zwischen den Schenkeln liegt auf der
//!   45°-Halbierenden,
//! * zwei **parallele** Linien (y = 8,00 und y = 11,00) — zwischen ihnen ist die
//!   Mittellinie die Halbierende.
//!
//! Prüfweg: Werkzeug „Linie" starten, den ersten Punkt zwischen die Linien
//! setzen und den zweiten neben die Halbierende führen — der Zeiger rastet auf
//! ihr ein, die Statuszeile meldet „Winkelhalbierende".

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
    // Rechter Winkel: Schenkel nach +X und +Y.
    linie(&mut doc, [1.0, 1.0], [7.0, 1.0])?;
    linie(&mut doc, [1.0, 1.0], [1.0, 6.0])?;
    // Paralleles Paar.
    linie(&mut doc, [1.0, 8.0], [7.0, 8.0])?;
    linie(&mut doc, [1.0, 11.0], [7.0, 11.0])?;
    let path = Path::new("samples/Winkelhalbierende.jjcad");
    persist::save_document(&mut doc, path)?;
    println!("geschrieben: {}", path.display());
    Ok(())
}
