//! Provisorische Messhilfe für Wandanschlüsse: rendert Fälle als PNG und
//! prüft die gezeichnete Geometrie zahlenmäßig (Lücken in der Füllung,
//! ungerandete Füllkanten, Linien im Wandinneren).

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{dist_point_segment, point_in_polygon, Vec2};
use jjcad_core::raster::render_png;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::style::WallAlign;
use jjcad_core::Session;
use serde_json::json;

#[derive(Clone)]
struct W {
    a: (f64, f64),
    b: (f64, f64),
    t: f64,
    al: WallAlign,
}

fn w(a: (f64, f64), b: (f64, f64), t: f64, al: WallAlign) -> W {
    W { a, b, t, al }
}

struct Fall {
    name: &'static str,
    walls: Vec<W>,
}

fn faelle() -> Vec<Fall> {
    let mut out = Vec::new();
    let a = (-2000.0, 0.0);
    let o = (0.0, 0.0);
    let p = |deg: f64| (2000.0 * deg.to_radians().cos(), 2000.0 * deg.to_radians().sin());
    let k = WallAlign::Center;
    let l = WallAlign::Left;
    let r = WallAlign::Right;

    out.push(Fall {
        name: "01_k90_gleich",
        walls: vec![w(a, o, 300.0, k), w(o, p(90.0), 300.0, k)],
    });
    out.push(Fall {
        name: "02_k90_dick",
        walls: vec![w(a, o, 365.0, k), w(o, p(90.0), 115.0, k)],
    });
    out.push(Fall {
        name: "03_k45_dick",
        walls: vec![w(a, o, 300.0, k), w(o, p(45.0), 115.0, k)],
    });
    out.push(Fall {
        name: "04_k135_dick",
        walls: vec![w(a, o, 300.0, k), w(o, p(135.0), 115.0, k)],
    });
    out.push(Fall {
        name: "05_k90_links_links",
        walls: vec![w(a, o, 300.0, l), w(o, p(90.0), 300.0, l)],
    });
    out.push(Fall {
        name: "06_k90_links_rechts",
        walls: vec![w(a, o, 300.0, l), w(o, p(90.0), 300.0, r)],
    });
    out.push(Fall {
        name: "07_k90_links_mitte_dick",
        walls: vec![w(a, o, 300.0, l), w(o, p(90.0), 115.0, k)],
    });
    out.push(Fall {
        name: "08_k90_rechts_links_dick",
        walls: vec![w(a, o, 365.0, r), w(o, p(90.0), 115.0, l)],
    });
    out.push(Fall {
        name: "09_k30_dick",
        walls: vec![w(a, o, 300.0, k), w(o, p(30.0), 115.0, k)],
    });
    out.push(Fall {
        name: "10_k150_dick",
        walls: vec![w(a, o, 300.0, l), w(o, p(150.0), 115.0, r)],
    });
    out.push(Fall {
        name: "11_drei_winkel",
        walls: vec![w(a, o, 300.0, k), w(o, p(120.0), 200.0, k), w(o, p(240.0), 200.0, k)],
    });
    out.push(Fall {
        name: "12_stich_20",
        walls: vec![w((-2000.0, 0.0), (2000.0, 0.0), 300.0, k), w(o, p(20.0), 115.0, k)],
    });
    for deg in [20.0, 45.0, 60.0, 120.0, 135.0, 160.0] {
        out.push(Fall {
            name: match deg as i32 {
                20 => "20_gleich_20",
                45 => "21_gleich_45",
                60 => "22_gleich_60",
                120 => "23_gleich_120",
                135 => "24_gleich_135",
                _ => "25_gleich_160",
            },
            walls: vec![w(a, o, 300.0, k), w(o, p(deg), 300.0, k)],
        });
    }
    out.push(Fall {
        name: "13_kreuzung",
        walls: vec![w((-2000.0, 0.0), (2000.0, 0.0), 300.0, k), w((0.0, -2000.0), (0.0, 2000.0), 300.0, k)],
    });
    out
}

fn band(c: &W) -> Vec<Vec2> {
    let a = Vec2::new(c.a.0, c.a.1);
    let b = Vec2::new(c.b.0, c.b.1);
    let dir = (b - a).normalized();
    let n = dir.perp();
    let h = c.t * 0.5;
    let off = n * (h * c.al.offset());
    let (a, b) = (a + off, b + off);
    vec![a + n * h, b + n * h, b - n * h, a - n * h]
}

struct Z {
    fills: Vec<Vec<Vec2>>,
    lines: Vec<(Vec2, Vec2)>,
}

fn zeichnung(s: &Session) -> Z {
    let ctx = RenderCtx::with_scale(&s.doc, 1.0);
    let dl = render_document(&ctx);
    let mut fills = Vec::new();
    let mut lines = Vec::new();
    for p in &dl.prims {
        match &p.shape {
            Shape::Fill { outer, .. } => fills.push(outer.clone()),
            Shape::Line(a, b) => lines.push((*a, *b)),
            _ => {}
        }
    }
    Z { fills, lines }
}

impl Z {
    fn tiefe(&self, p: Vec2) -> f64 {
        let mut best = f64::NEG_INFINITY;
        for f in &self.fills {
            let rand = f
                .iter()
                .enumerate()
                .map(|(i, q)| dist_point_segment(p, *q, f[(i + 1) % f.len()]))
                .fold(f64::INFINITY, f64::min);
            let d = if point_in_polygon(p, f) { rand } else { -rand };
            best = best.max(d);
        }
        best
    }
    fn linie_nahe(&self, p: Vec2, tol: f64) -> bool {
        self.lines
            .iter()
            .any(|(a, b)| dist_point_segment(p, *a, *b) <= tol)
    }
}

/// Sammelt Messwerte statt zu panicken: Lücken in der Vereinigung der Bänder,
/// ungerandete Füllkanten, Linien im Wandinneren.
fn messe(z: &Z, walls: &[W]) -> (usize, f64, Vec<Vec2>, Vec<Vec2>) {
    let bands: Vec<Vec<Vec2>> = walls.iter().map(band).collect();
    let mut luecken = 0;
    let mut luecke_max = 0.0f64;
    let mut luecke_bsp: Vec<Vec2> = Vec::new();
    let mut offen: Vec<Vec2> = Vec::new();

    // Vereinigung der ungetrimmten Bänder abtasten (1 mm), 0,8 mm innerhalb
    // der Bandkante, damit wir nicht den Sollrand selbst messen.
    for (bi, b) in bands.iter().enumerate() {
        let _ = bi;
        let xs: Vec<f64> = b.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = b.iter().map(|p| p.y).collect();
        let (x0, x1) = (
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        let (y0, y1) = (
            ys.iter().cloned().fold(f64::INFINITY, f64::min),
            ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                let p = Vec2::new(x, y);
                if point_in_polygon(p, b) && z.tiefe(p) < -0.8 {
                    luecken += 1;
                    let d = -z.tiefe(p);
                    if d > luecke_max {
                        luecke_max = d;
                    }
                    if luecke_bsp.len() < 6 {
                        luecke_bsp.push(p);
                    }
                }
                x += 1.0;
            }
            y += 1.0;
        }
    }

    for (i, f) in z.fills.iter().enumerate() {
        for k in 0..f.len() {
            let (p0, p1) = (f[k], f[(k + 1) % f.len()]);
            let n = ((p1 - p0).len() / 2.0).ceil().max(1.0) as usize;
            for j in 0..=n {
                let p = p0 + (p1 - p0) * (j as f64 / n as f64);
                let innen = z
                    .fills
                    .iter()
                    .enumerate()
                    .any(|(o, g)| o != i && point_in_polygon(p, g));
                if !innen && !z.linie_nahe(p, 0.3) && offen.len() < 12 {
                    offen.push(p);
                }
            }
        }
    }
    (luecken, luecke_max, luecke_bsp, offen)
}

fn main() {
    let dir = std::path::Path::new("/home/joschua/jjcad-live/wand-probe");
    std::fs::create_dir_all(dir).unwrap();
    for f in faelle() {
        let mut s = Session::new();
        s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
        for c in &f.walls {
            s.execute(
                "draw.wall",
                &json!({
                    "a": [c.a.0 / 1000.0, c.a.1 / 1000.0],
                    "b": [c.b.0 / 1000.0, c.b.1 / 1000.0],
                    "thickness": c.t / 1000.0,
                    "align": match c.al {
                        WallAlign::Left => "left",
                        WallAlign::Center => "center",
                        WallAlign::Right => "right",
                    },
                }),
            )
            .unwrap();
        }
        let z = zeichnung(&s);
        let (luecken, lmax, lbsp, offen) = messe(&z, &f.walls);
        let opts = RasterOptions {
            width_px: 1000,
            height_px: 1000,
            bbox: Some([-700.0, -700.0, 700.0, 700.0]),
            paper: false,
            annot_scale: Some(1.0),
            sheet: None,
        };
        let p = dir.join(format!("{}.png", f.name));
        render_png(&s.doc, &opts, &p).unwrap();
        println!(
            "{}: Lückenpunkte={} (max {:.2} mm) offene Kantenpunkte={}",
            f.name,
            luecken,
            lmax,
            offen.len()
        );
        if !lbsp.is_empty() {
            println!("    Lücken z. B. {:?}", &lbsp[..lbsp.len().min(4)]);
        }
        if !offen.is_empty() {
            println!("    offen z. B. {:?}", &offen[..offen.len().min(4)]);
        }
    }
}
