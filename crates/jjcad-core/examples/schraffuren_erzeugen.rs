//! Erzeugt `samples/Schraffuren.jjcad`: alle Muster der DIN 1356-1:2026-09,
//! Tabelle 5, als beschriftete Felder in einem Raster.
//!
//! Die Datei ist der Prüfstand für die Mustertabelle: jedes Feld trägt seinen
//! Namen nach der Norm, die Felder stehen in der Reihenfolge der Tabelle.
//! Musterabstand 100 mm, Umrandung an.
use jjcad_core::export::RasterOptions;
use jjcad_core::raster::render_png;
use jjcad_core::style::FillPattern;
use jjcad_core::{persist, Session};
use serde_json::json;

/// Feldbreite/-höhe und Raster im Musterblatt (mm).
const FELD_B: f64 = 1500.0;
const FELD_H: f64 = 750.0;
const SPALTE: usize = 3;
/// Abstand der Felder (Platz für die Beschriftung).
const LUFT: f64 = 700.0;

fn main() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    // Ebene „Schraffur“ ist hell — für ein lesbares Prüfbild die dunkle
    // Ebene „Wände“ nehmen (wie in den Pixel-Tests).
    let muster: Vec<FillPattern> = FillPattern::ALL.to_vec();
    for (i, p) in muster.iter().enumerate() {
        let spalte = (i % SPALTE) as f64;
        let zeile = (i / SPALTE) as f64;
        let x = spalte * (FELD_B + LUFT) + 250.0;
        let y = -zeile * (FELD_H + 350.0);
        s.execute(
            "draw.text",
            &json!({
                "p": [x, y + 120.0],
                "text": p.label(),
                "height": 150.0,
                "layer": "Wände"
            }),
        )
        .unwrap();
        s.execute(
            "draw.fill.points",
            &json!({
                "boundary": [[x, y], [x + FELD_B, y], [x + FELD_B, y - FELD_H], [x, y - FELD_H]],
                "pattern": p.key(),
                "scale": 100.0,
                "border": true,
                "layer": "Wände"
            }),
        )
        .unwrap();
    }
    let text = persist::to_json(&s.doc).unwrap();
    let ziel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/Schraffuren.jjcad");
    if let Some(dir) = ziel.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&ziel, text).unwrap();
    println!("geschrieben: {}", ziel.display());

    let breite = SPALTE as f64 * (FELD_B + LUFT) + 250.0;
    let hoehe = ((muster.len() as f64 / SPALTE as f64).ceil() * (FELD_H + 350.0)) + 400.0;
    let opts = RasterOptions {
        width_px: 1800,
        height_px: 1500,
        bbox: Some([0.0, -hoehe, breite, 400.0]),
        paper: false,
        annot_scale: Some(1.0),
        sheet: None,
    };
    let png = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/Schraffuren.png");
    render_png(&s.doc, &opts, &png).unwrap();
    println!("Bild: {}", png.display());
}
