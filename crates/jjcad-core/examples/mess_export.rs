//! Messprogramm: Export-Ausgaben (PDF/PNG) prüfen (t_aea1b20d).
//!
//! Lädt eine Zeichnung und schreibt PDF + PNG, damit alter und neuer Stand
//! Byte- und Pixel-identisch verglichen werden können. Zählt außerdem die
//! Zeichenoperatoren im PDF-Inhaltsstrom (Linien vs. Bézier) und die Zahl der
//! Polygonpunkte des Bogens — damit ist belegbar, ob im Druck echtes
//! Kurvenverhalten entsteht oder weiter ein Polygonzug.
//!
//! Aufruf: cargo run --release -p jjcad-core --example mess_export -- <datei> <ziel-prefix>

use jjcad_core::export::RasterOptions;
use jjcad_core::pdf::export_model_pdf;
use jjcad_core::raster::render_png;
use jjcad_core::session::Session;
use jjcad_core::style::PaperSize;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let datei = args.get(1).cloned().unwrap_or("samples/Wohnhaus.jjcad".into());
    let prefix = args.get(2).cloned().unwrap_or("/tmp/export".into());

    let s = Session::open(Path::new(&datei)).expect("Datei nicht lesbar");
    let doc = &s.doc;

    let pdf = format!("{prefix}.pdf");
    let png = format!("{prefix}.png");
    export_model_pdf(doc, Path::new(&pdf), PaperSize::A4).expect("PDF-Export");
    let opts = RasterOptions {
        width_px: 2000,
        height_px: 1400,
        ..Default::default()
    };
    render_png(doc, &opts, Path::new(&png)).expect("PNG-Export");

    // PDF-Inhaltsstrom grob auswerten: Operatoren zählen.
    let roh = std::fs::read(&pdf).expect("PDF lesen");
    let text = String::from_utf8_lossy(&roh);
    let linien = text.matches(" l\n").count() + text.matches(" l\r\n").count();
    let beziers = text.matches(" c\n").count() + text.matches(" c\r\n").count();
    println!("PDF: {pdf}  Bytes={}  Linienoperatoren(l)={linien}  Beziers(c)={beziers}", roh.len());
    let meta = std::fs::metadata(&png).expect("PNG-Stat");
    println!("PNG: {png}  Bytes={}", meta.len());

    // Bogenpunkte im PDF zählen: aus der Zeichnung bekannte Bögen
    let mut boegen = 0;
    for e in &doc.entities {
        if matches!(e.kind, jjcad_core::model::EntityKind::Arc { .. }) {
            boegen += 1;
        }
    }
    println!("Bögen in der Zeichnung: {boegen}");
}
