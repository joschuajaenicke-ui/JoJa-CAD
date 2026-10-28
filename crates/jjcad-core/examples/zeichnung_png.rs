//! Zeichnung als PNG rendern (ohne GUI): `zeichnung_png <datei.jjcad> [ziel.png]`

use jjcad_core::export::RasterOptions;
use jjcad_core::persist;
use jjcad_core::raster::render_png;
use jjcad_core::render::visible_bbox;

fn main() {
    let datei = std::env::args().nth(1).expect("datei.jjcad fehlt");
    let ziel = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/home/joschua/jjcad-live/zeichnung.png".to_string());
    let doc = persist::load_document(std::path::Path::new(&datei)).expect("Datei laden");
    let bb = visible_bbox(&doc);
    // Optionaler Ausschnitt: x0 y0 x1 y1 (Welt-mm)
    let a: Vec<f64> = std::env::args().skip(3).filter_map(|v| v.parse().ok()).collect();
    let bbox = if a.len() == 4 {
        [a[0], a[1], a[2], a[3]]
    } else {
        [bb.min.x, bb.min.y, bb.max.x, bb.max.y]
    };
    let opts = RasterOptions {
        width_px: 1600,
        height_px: 1200,
        bbox: Some(bbox),
        paper: false,
        annot_scale: Some(1.0),
        sheet: None,
    };
    render_png(&doc, &opts, std::path::Path::new(&ziel)).expect("PNG schreiben");
    println!("{ziel}");
}
