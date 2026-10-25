//! Provisorische Messhilfe: rendert Wandgehrungen unter verschiedenen Winkeln
//! als PNG (Zoom auf den Knotenpunkt).
//!
//! Aufruf: `cargo run -p jjcad-core --example sharp_probe -- /tmp/sharp [winkel...]`

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{v, Vec2};
use jjcad_core::{raster, Session};
use serde_json::json;
use std::path::PathBuf;

fn corner_doc(theta_deg: f64, thickness: f64, len: f64) -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    let j = Vec2::ZERO;
    let d_a = Vec2::new(-1.0, 0.0);
    let d_b = Vec2::polar(theta_deg.to_radians(), 1.0);
    s.execute(
        "draw.wall",
        &json!({"a": [j.x + d_a.x * len / 1000.0, j.y + d_a.y * len / 1000.0],
                "b": [j.x / 1000.0, j.y / 1000.0],
                "thickness": thickness / 1000.0}),
    )
    .unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [j.x / 1000.0, j.y / 1000.0],
                "b": [j.x / 1000.0 + d_b.x * len / 1000.0, j.y / 1000.0 + d_b.y * len / 1000.0],
                "thickness": thickness / 1000.0}),
    )
    .unwrap();
    s
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "/tmp/sharp".into()));
    let angles: Vec<f64> = args
        .map(|a| a.parse().unwrap())
        .collect::<Vec<_>>()
        .into_iter()
        .chain(std::iter::empty())
        .collect();
    let angles = if angles.is_empty() {
        vec![90.0, 45.0, 20.0, 10.0, 5.0, 3.0]
    } else {
        angles
    };
    std::fs::create_dir_all(&out).unwrap();
    for th in angles {
        let thickness = 300.0;
        let s = corner_doc(th, thickness, 2000.0);
        // Zoom: Knotenpunkt im Ursprung, Fenster je nach Winkel
        let half = if th >= 30.0 { 500.0 } else { 400.0 };
        let opts = RasterOptions {
            width_px: 900,
            height_px: 900,
            bbox: Some([-half, -half, half, half]),
            paper: false,
            annot_scale: Some(1.0),
            sheet: None,
        };
        let path = out.join(format!("winkel_{:05.1}.png", th));
        raster::render_png(&s.doc, &opts, &path).unwrap();
        println!("{}", path.display());
    }
}
