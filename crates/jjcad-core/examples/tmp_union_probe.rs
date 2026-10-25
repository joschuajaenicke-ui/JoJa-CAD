//! Provisorisch (wird wieder gelöscht): druckt Wände, Füllungen und Linien
//! eines Prüffalls als JSON, damit ein Python-Skript die Vereinigung der
//! Wandkörper mit der gezeichneten Geometrie vergleichen kann.

use jjcad_core::model::EntityKind;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::Session;
use serde_json::json;
use std::fmt::Write as _;

fn wall(s: &mut Session, a: [f64; 2], b: [f64; 2], t: f64, align: &str) {
    s.execute(
        "draw.wall",
        &json!({"a": [a[0] / 1000.0, a[1] / 1000.0], "b": [b[0] / 1000.0, b[1] / 1000.0],
                "thickness": t / 1000.0, "align": align}),
    )
    .unwrap();
}

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

fn polar(r: f64, deg: f64) -> [f64; 2] {
    let a = deg.to_radians();
    [r * a.cos(), r * a.sin()]
}

/// Ecke: Wand A von (-2000,0) nach (0,0), Wand B vom Knoten unter `deg`.
fn corner(deg: f64, ta: f64, tb: f64, aa: &str, ab: &str) -> Session {
    let mut s = session();
    wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], ta, aa);
    let e = polar(2000.0, deg);
    wall(&mut s, [0.0, 0.0], e, tb, ab);
    s
}

/// Stich: durchlaufende Wand in X, Stichwand vom Knoten unter `deg`.
fn stub(deg: f64, tb: f64, off: f64) -> Session {
    let mut s = session();
    wall(&mut s, [-2000.0, 0.0], [2000.0, 0.0], 300.0, "center");
    let e = polar(2000.0, deg);
    wall(&mut s, [off, 0.0], [off + e[0], e[1]], tb, "center");
    s
}

fn cross(deg: f64) -> Session {
    let mut s = session();
    wall(&mut s, [-2000.0, 0.0], [2000.0, 0.0], 300.0, "center");
    let a = polar(2000.0, deg);
    wall(&mut s, [-a[0], -a[1]], a, 300.0, "center");
    s
}

fn push_fill(out: &mut String, poly: &[jjcad_core::geom::Vec2]) {
    out.push_str("\n    [");
    for (i, p) in poly.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "[{:.4},{:.4}]", p.x, p.y);
    }
    out.push(']');
}

fn main() {
    let mut cases: Vec<(String, Session)> = Vec::new();
    for deg in [90.0, 45.0, 30.0, 15.0, 10.0, 5.0, 3.0, 2.0, 1.0] {
        cases.push((format!("v{deg:.0}"), corner(deg, 300.0, 300.0, "center", "center")));
    }
    for deg in [20.0, 15.0, 10.0, 5.0] {
        cases.push((format!("u{deg:.0}"), corner(deg, 365.0, 115.0, "center", "center")));
    }
    for deg in [15.0, 5.0] {
        cases.push((format!("ll{deg:.0}"), corner(deg, 300.0, 300.0, "left", "left")));
        cases.push((format!("lr{deg:.0}"), corner(deg, 300.0, 300.0, "left", "right")));
    }
    for deg in [30.0, 20.0, 10.0, 5.0] {
        cases.push((format!("s{deg:.0}"), stub(deg, 115.0, 0.0)));
        cases.push((format!("o{deg:.0}"), stub(deg, 115.0, 400.0)));
    }
    for deg in [90.0, 30.0, 10.0] {
        cases.push((format!("x{deg:.0}"), cross(deg)));
    }
    {
        let mut s = session();
        wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], 300.0, "center");
        wall(&mut s, [0.0, 0.0], polar(2000.0, 15.0), 300.0, "center");
        wall(&mut s, [0.0, 0.0], polar(2000.0, 200.0), 300.0, "center");
        cases.push(("drei15".into(), s));
    }
    {
        let mut s = session();
        wall(&mut s, [-2000.0, 0.0], [0.0, 0.0], 300.0, "center");
        cases.push(("frei".into(), s));
    }

    let mut out = String::from("[");
    for (i, (name, s)) in cases.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "\n{{\"name\":\"{name}\",\"walls\":[");
        let mut first = true;
        for e in &s.doc.entities {
            if let EntityKind::Wall {
                a,
                b,
                thickness,
                align,
                ..
            } = &e.kind
            {
                if !first {
                    out.push(',');
                }
                first = false;
                let _ = write!(
                    out,
                    "{{\"a\":[{:.4},{:.4}],\"b\":[{:.4},{:.4}],\"t\":{:.4},\"align\":\"{:?}\"}}",
                    a.x, a.y, b.x, b.y, thickness, align
                );
            }
        }
        out.push_str("],\n  \"fills\":[");
        let ctx = RenderCtx::with_scale(&s.doc, 1.0);
        let dl = render_document(&ctx);
        let mut ff = false;
        let mut lf = false;
        let mut lines = String::new();
        for p in &dl.prims {
            match &p.shape {
                Shape::Fill(poly) => {
                    if ff {
                        out.push(',');
                    }
                    ff = true;
                    push_fill(&mut out, poly);
                }
                Shape::Line(a, b) => {
                    if lf {
                        lines.push(',');
                    }
                    lf = true;
                    let _ = write!(
                        lines,
                        "\n    [[{:.4},{:.4}],[{:.4},{:.4}]]",
                        a.x, a.y, b.x, b.y
                    );
                }
                _ => {}
            }
        }
        let _ = write!(out, "],\n  \"lines\":[{lines}]}}");
    }
    out.push_str("\n]");
    println!("{out}");

    // Ausschnitte um den Knoten (2 px/mm) für die Sichtprüfung
    use jjcad_core::export::RasterOptions;
    let dir = std::path::PathBuf::from("/tmp/sharp/corner");
    std::fs::create_dir_all(&dir).unwrap();
    let zoom: f64 = std::env::var("ZOOM").ok().and_then(|z| z.parse().ok()).unwrap_or(400.0);
    for (name, s) in &cases {
        let opts = RasterOptions {
            width_px: 1600,
            height_px: 1600,
            bbox: Some([-zoom, -zoom, zoom, zoom]),
            paper: false,
            annot_scale: Some(1.0),
            sheet: None,
        };
        let path = dir.join(format!("{name}.png"));
        jjcad_core::raster::render_png(&s.doc, &opts, &path).unwrap();
    }
    println!("PNG: {}", dir.display());
}
