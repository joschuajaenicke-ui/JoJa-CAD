//! Kontaktbogen: Wandanschlüsse als Einzelbilder (für die Abnahme).
use jjcad_core::export::RasterOptions;
use jjcad_core::raster::render_png;
use jjcad_core::wall::WallIndex;
use jjcad_core::Session;
use serde_json::json;

fn bau(name: &str, al_a: &str, al_b: &str, psi: f64, ta: f64, tb: f64, lb: f64) {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    // Wände so legen, dass der Öffnungswinkel psi entsteht: B läuft unter
    // 180°-psi von der Nahtstelle weg.
    let r = (180.0 - psi).to_radians();
    s.execute("draw.wall", &json!({"a": [-2.0, 0.0], "b": [0.0, 0.0], "thickness": ta / 1000.0, "align": al_a})).unwrap();
    s.execute("draw.wall", &json!({"a": [0.0, 0.0], "b": [lb / 1000.0 * r.cos(), lb / 1000.0 * r.sin()], "thickness": tb / 1000.0, "align": al_b})).unwrap();
    let idx = WallIndex::build(&s.doc, s.doc.ents());
    let n: Vec<usize> = s.doc.entities.iter().map(|e| idx.seams_of(e.id).len()).collect();
    let l = s.doc.entities.iter().filter_map(|e| idx.seams_of(e.id).first().map(|s| s.inner.dist(s.outer))).next().unwrap_or(0.0);
    println!("{name:<26} Naht {n:?} Länge {l:6.0} mm");
    let opts = RasterOptions { width_px: 760, height_px: 560, bbox: None, paper: false, annot_scale: Some(1.0), sheet: None };
    render_png(&s.doc, &opts, std::path::Path::new(&format!("/home/joschua/jjcad-live/wand-probe/bogen_{name}.png"))).unwrap();
}

fn main() {
    bau("1_spitz20", "center", "center", 20.0, 240.0, 240.0, 1500.0);
    bau("2_spitz30", "center", "center", 30.0, 240.0, 240.0, 1500.0);
    bau("3_45", "center", "center", 45.0, 240.0, 240.0, 1500.0);
    bau("4_90", "center", "center", 90.0, 240.0, 240.0, 1500.0);
    bau("5_135", "center", "center", 135.0, 240.0, 240.0, 1500.0);
    bau("6_90_aussen", "center", "left", 90.0, 240.0, 240.0, 1500.0);
    bau("7_90_dick", "center", "center", 90.0, 365.0, 115.0, 1500.0);
    bau("8_60_kurz", "center", "left", 60.0, 240.0, 240.0, 800.0);
}
