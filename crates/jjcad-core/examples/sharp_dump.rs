//! Provisorisch: gibt die Gehrungsgeometrie (Füllpolygone + Linien) zahlenmäßig aus.

use jjcad_core::geom::Vec2;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::Session;
use serde_json::json;

fn corner_doc(theta_deg: f64, thickness: f64, len: f64) -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    let d_a = Vec2::new(-1.0, 0.0);
    let d_b = Vec2::polar(theta_deg.to_radians(), 1.0);
    s.execute(
        "draw.wall",
        &json!({"a": [d_a.x * len / 1000.0, d_a.y * len / 1000.0], "b": [0.0, 0.0],
                "thickness": thickness / 1000.0}),
    )
    .unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0],
                "b": [d_b.x * len / 1000.0, d_b.y * len / 1000.0],
                "thickness": thickness / 1000.0}),
    )
    .unwrap();
    s
}

fn fmt(p: Vec2) -> String {
    format!("({:9.3},{:9.3})", p.x, p.y)
}

fn main() {
    let angles: Vec<f64> = std::env::args()
        .skip(1)
        .map(|a| a.parse().unwrap())
        .collect::<Vec<_>>();
    let angles = if angles.is_empty() {
        vec![90.0, 45.0, 20.0, 10.0, 5.0, 3.0, 1.0]
    } else {
        angles
    };
    for th in angles {
        let s = corner_doc(th, 300.0, 2000.0);
        let ctx = RenderCtx::with_scale(&s.doc, 1.0);
        let dl = render_document(&ctx);
        println!("===== Winkel {th}° =====");
        for p in &dl.prims {
            match &p.shape {
                Shape::Fill(poly) => {
                    let pts: Vec<String> = poly.iter().map(|q| fmt(*q)).collect();
                    println!("  FILL [{}]", pts.join(" "));
                }
                Shape::Line(a, b) => {
                    println!("  LINE {} -> {}", fmt(*a), fmt(*b));
                }
                _ => {}
            }
        }
    }
}
