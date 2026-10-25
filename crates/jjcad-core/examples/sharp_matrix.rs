//! Provisorische Messhilfe: gibt für eine Matrix aus Eckenkonfigurationen die
//! Displayliste als JSON aus (Punkte in mm). Die Soll-Geometrie wird außerhalb
//! (Python) unabhängig gerechnet und verglichen.
//!
//! Aufruf: `cargo run -p jjcad-core --example sharp_matrix`

use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::Session;
use serde_json::json;

fn esc(s: &str) -> String {
    s.to_string()
}

/// Zwei Wände, die sich im Ursprung unter `theta` Grad treffen.
fn corner_doc(theta: f64, ta: f64, tb: f64, aa: &str, ab: &str, len: f64) -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    let d_b = (theta.to_radians().cos(), theta.to_radians().sin());
    s.execute(
        "draw.wall",
        &json!({"a": [-len / 1000.0, 0.0], "b": [0.0, 0.0],
                "thickness": ta / 1000.0, "align": aa}),
    )
    .unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0], "b": [d_b.0 * len / 1000.0, d_b.1 * len / 1000.0],
                "thickness": tb / 1000.0, "align": ab}),
    )
    .unwrap();
    s
}

fn main() {
    let angles = [45.0, 20.0, 10.0, 5.0, 2.0];
    let combos = [
        (300.0, 300.0, "center", "center"),
        (300.0, 300.0, "left", "left"),
        (300.0, 300.0, "right", "right"),
        (300.0, 300.0, "left", "right"),
        (365.0, 115.0, "center", "center"),
        (115.0, 365.0, "center", "center"),
    ];
    for th in angles {
        for (ta, tb, aa, ab) in combos {
            let s = corner_doc(th, ta, tb, aa, ab, 2000.0);
            let ctx = RenderCtx::with_scale(&s.doc, 1.0);
            let dl = render_document(&ctx);
            let mut prims = Vec::new();
            for p in &dl.prims {
                match &p.shape {
                    Shape::Fill { outer: poly, .. } => {
                        let pts: Vec<[f64; 2]> = poly.iter().map(|q| [q.x, q.y]).collect();
                        prims.push(json!({"t": "fill", "pts": pts}));
                    }
                    Shape::Line(a, b) => {
                        prims.push(json!({"t": "line", "a": [a.x, a.y], "b": [b.x, b.y]}));
                    }
                    _ => {}
                }
            }
            println!(
                "{}",
                json!({"theta": th, "ta": ta, "tb": tb, "aa": esc(aa), "ab": esc(ab),
                        "len": 2000.0, "prims": prims})
            );
        }
    }
}
