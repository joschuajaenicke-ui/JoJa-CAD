//! Provisorisch: rendert typische spitze Anschlusssituationen als PNG.
//!
//! Aufruf: `cargo run -p jjcad-core --example sharp_cases -- /tmp/sharp/cases`

use jjcad_core::export::RasterOptions;
use jjcad_core::{raster, Session};
use serde_json::json;
use std::path::PathBuf;

fn wall(s: &mut Session, a: [f64; 2], b: [f64; 2], t: f64, align: &str) {
    s.execute(
        "draw.wall",
        &json!({"a": [a[0] / 1000.0, a[1] / 1000.0], "b": [b[0] / 1000.0, b[1] / 1000.0],
                "thickness": t / 1000.0, "align": align}),
    )
    .unwrap();
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or("/tmp/sharp/cases".into()));
    std::fs::create_dir_all(&out).unwrap();
    let mut cases: Vec<(&str, Session, [f64; 4])> = Vec::new();

    // 1) spitze Ecke 15°, beide 300
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    let th: f64 = 15.0_f64.to_radians();
    wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], 300.0, "center");
    wall(&mut s, [0.0, 0.0], [2000.0 * th.cos(), 2000.0 * th.sin()], 300.0, "center");
    cases.push(("15deg_300_300", s, [-300.0, -300.0, 400.0, 400.0]));

    // 2) spitze Ecke 15°, 365 / 115
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], 365.0, "center");
    wall(&mut s, [0.0, 0.0], [2000.0 * th.cos(), 2000.0 * th.sin()], 115.0, "center");
    cases.push(("15deg_365_115", s, [-300.0, -300.0, 400.0, 400.0]));

    // 3) stumpfe Ecke 150°
    let th2: f64 = 150.0_f64.to_radians();
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], 300.0, "center");
    wall(&mut s, [0.0, 0.0], [2000.0 * th2.cos(), 2000.0 * th2.sin()], 300.0, "center");
    cases.push(("150deg_300_300", s, [-300.0, -300.0, 400.0, 400.0]));

    // 4) spitzer Stichanschluss (Stichwand 20° in eine durchlaufende Wand)
    let th3: f64 = 20.0_f64.to_radians();
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    wall(&mut s, [-2000.0, 0.0], [2000.0, 0.0], 300.0, "center");
    wall(&mut s, [0.0, 0.0], [2000.0 * th3.cos(), 2000.0 * th3.sin()], 115.0, "center");
    cases.push(("stich_20deg", s, [-300.0, -300.0, 400.0, 400.0]));

    // 5) drei Wände in einem Punkt (15° zwischen zwei Achsen)
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], 300.0, "center");
    wall(&mut s, [0.0, 0.0], [2000.0 * th.cos(), 2000.0 * th.sin()], 300.0, "center");
    wall(&mut s, [0.0, 0.0], [-2000.0, -300.0], 300.0, "center");
    cases.push(("drei_waende_15deg", s, [-300.0, -350.0, 450.0, 450.0]));

    // 6) spitze Ecke 15°, Ausrichtung links/links
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], 300.0, "left");
    wall(&mut s, [0.0, 0.0], [2000.0 * th.cos(), 2000.0 * th.sin()], 300.0, "left");
    cases.push(("15deg_left_left", s, [-300.0, -300.0, 400.0, 400.0]));

    let dump = std::env::var("DUMP").ok();
    for (name, s, bb) in &cases {
        if let Some(d) = &dump {
            if d == "*" || d == name {
                let ctx = jjcad_core::render::RenderCtx::with_scale(&s.doc, 1.0);
                let dl = jjcad_core::render::render_document(&ctx);
                println!("===== {name} =====");
                for p in &dl.prims {
                    match &p.shape {
                        jjcad_core::render::Shape::Fill(poly) => {
                            let pts: Vec<String> = poly
                                .iter()
                                .map(|q| format!("({:8.2},{:8.2})", q.x, q.y))
                                .collect();
                            println!("  FILL {:3} {}", p.entity, pts.join(" "));
                        }
                        jjcad_core::render::Shape::Line(a, b) => {
                            println!(
                                "  LINE {:3} ({:8.2},{:8.2}) -> ({:8.2},{:8.2})",
                                p.entity, a.x, a.y, b.x, b.y
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    for (name, s, bb) in cases {
        let opts = RasterOptions {
            width_px: 900,
            height_px: 900,
            bbox: Some(bb),
            paper: false,
            annot_scale: Some(1.0),
            sheet: None,
        };
        let path = out.join(format!("{name}.png"));
        raster::render_png(&s.doc, &opts, &path).unwrap();
        println!("{}", path.display());
    }
}
