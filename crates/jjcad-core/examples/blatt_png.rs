//! Ein Blatt als PNG rendern (ohne GUI): `blatt_png <datei.jjcad> [ziel.png] [blatt-nr]`
//!
//! Anders als `zeichnung_png` (Modell, ohne Papier) zeigt dieses Beispiel das
//! Blatt mit Rahmen, Schriftfeld und Ansichtsfenstern — so, wie es gedruckt
//! aussieht. Ohne Blattnummer wird das erste Blatt genommen.

use jjcad_core::export::RasterOptions;
use jjcad_core::{persist, raster};

fn main() {
    let mut args = std::env::args().skip(1);
    let datei = args.next().expect("datei.jjcad fehlt");
    let ziel = args
        .next()
        .unwrap_or_else(|| std::path::Path::new(&datei).with_extension("blatt.png").to_string_lossy().to_string());
    let blatt: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let doc = persist::load_document(std::path::Path::new(&datei)).expect("Datei laden");
    let opts = RasterOptions {
        width_px: 2400,
        height_px: 1700,
        bbox: None,
        paper: true,
        annot_scale: None,
        sheet: Some(blatt.saturating_sub(1)),
    };
    raster::render_png(&doc, &opts, std::path::Path::new(&ziel)).expect("PNG schreiben");
    println!("{ziel}");
}
