//! Provisorisch: V-förmige Ecken (beide Wände laufen unter spitzem Innenwinkel
//! vom Knoten weg). Aufruf: `cargo run -p jjcad-core --example sharp_v -- /tmp/sharp/v`

use jjcad_core::export::RasterOptions;
use jjcad_core::{raster, Session};
use serde_json::json;
use std::path::PathBuf;

fn wall(s: &mut Session, a: [f64; 2], b: [f64; 2], t: f64) {
    s.execute(
        "draw.wall",
        &json!({"a": [a[0] / 1000.0, a[1] / 1000.0], "b": [b[0] / 1000.0, b[1] / 1000.0],
                "thickness": t / 1000.0}),
    )
    .unwrap();
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or("/tmp/sharp/v".into()));
    std::fs::create_dir_all(&out).unwrap();
    let angles: Vec<f64> = std::env::args()
        .skip(2)
        .map(|a| a.parse().unwrap())
        .collect();
    let angles = if angles.is_empty() {
        vec![90.0, 60.0, 45.0, 30.0, 20.0, 10.0, 5.0]
    } else {
        angles
    };
    for th in angles {
        let mut s = Session::new();
        s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
        // Wand A: Körper in Richtung 180° vom Knoten weg
        wall(&mut s, [2000.0, 0.0], [0.0, 0.0], 300.0);
        // Wand B: Körper unter 180° - th
        let a = (180.0 - th).to_radians();
        wall(&mut s, [0.0, 0.0], [2000.0 * a.cos(), 2000.0 * a.sin()], 300.0);
        let opts = RasterOptions {
            width_px: 900,
            height_px: 900,
            bbox: Some([-700.0, -200.0, 500.0, 1000.0]),
            paper: false,
            annot_scale: Some(1.0),
            sheet: None,
        };
        let p = out.join(format!("v_{:05.1}.png", th));
        raster::render_png(&s.doc, &opts, &p).unwrap();
        println!("{}", p.display());
    }
}
