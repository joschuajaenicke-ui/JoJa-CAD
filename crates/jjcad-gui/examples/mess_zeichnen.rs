//! Messprogramm: Zeichenaufwand Bogen vor/nach (t_aea1b20d).
//!
//! Misst, was Epaint tatsächlich tun muss: alte Darstellung = eine Kapsel
//! (bzw. ein `line_segment` bei dünnem Strich) **je Sehne**, neue Darstellung
//! = **ein** Pfad plus zwei Kreise für die runden Enden. Ausgegeben werden
//! Scheitelpunktzahl und Tessellierungszeit.
//!
//! Aufruf: cargo run --release -p jjcad-gui --example mess_zeichnen

use egui::epaint::{ClippedShape, TessellationOptions, Tessellator};
use egui::{Color32, Pos2, Rect, Shape, Stroke};

const CLIP: Rect = Rect {
    min: Pos2::new(0.0, 0.0),
    max: Pos2::new(4000.0, 4000.0),
};

/// Kapsel wie `canvas::capsule` (Rechteck + zwei Halbkreise in einem Umriss).
fn kapsel(a: Pos2, b: Pos2, w: f32, col: Color32) -> Shape {
    if w < 2.0 {
        return Shape::line_segment([a, b], Stroke::new(w.max(0.4), col));
    }
    let r = w * 0.5;
    let d = b - a;
    let len = d.length();
    if len < 0.01 {
        return Shape::circle_filled(a, r, col);
    }
    let u = d / len;
    let n = egui::vec2(-u.y, u.x);
    let seg = if r > 24.0 {
        18
    } else if r > 10.0 {
        12
    } else {
        8
    };
    let mut pts: Vec<Pos2> = Vec::with_capacity(2 * seg + 2);
    for i in 0..=seg {
        let t = std::f32::consts::PI * (i as f32) / (seg as f32);
        pts.push(b + (n * t.cos() + u * t.sin()) * r);
    }
    for i in 0..=seg {
        let t = std::f32::consts::PI * (i as f32) / (seg as f32);
        pts.push(a + (-n * t.cos() - u * t.sin()) * r);
    }
    Shape::convex_polygon(pts, col, Stroke::NONE)
}

fn formen(alt: bool, n: usize, w: f32) -> Vec<ClippedShape> {
    let col = Color32::from_rgb(20, 20, 20);
    let mut aus = Vec::new();
    if alt {
        // Eine Form je Sehne (alter Weg: stroke_pts über die abgeflachten Punkte).
        let mut pts = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let a = std::f32::consts::FRAC_PI_2 * (i as f32 / n as f32);
            pts.push(Pos2::new(200.0 + 1000.0 * a.cos(), 200.0 + 1000.0 * a.sin()));
        }
        for paar in pts.windows(2) {
            aus.push(ClippedShape {
                clip_rect: CLIP,
                shape: kapsel(paar[0], paar[1], w, col),
            });
        }
    } else {
        // Neuer Weg: ein Pfad mit runden Enden als zwei Kreise.
        let mut pts = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let a = std::f32::consts::FRAC_PI_2 * (i as f32 / n as f32);
            pts.push(Pos2::new(200.0 + 1000.0 * a.cos(), 200.0 + 1000.0 * a.sin()));
        }
        let (anfang, ende) = (pts[0], *pts.last().unwrap());
        aus.push(ClippedShape {
            clip_rect: CLIP,
            shape: Shape::line(pts, Stroke::new(w, col)),
        });
        let rr = (w * 0.5).max(0.4);
        aus.push(ClippedShape {
            clip_rect: CLIP,
            shape: Shape::circle_filled(anfang, rr, col),
        });
        aus.push(ClippedShape {
            clip_rect: CLIP,
            shape: Shape::circle_filled(ende, rr, col),
        });
    }
    aus
}

fn messen(name: &str, alt: bool, n: usize, w: f32, durchlaeufe: u32) {
    let shapes = formen(alt, n, w);
    let mut tess = Tessellator::new(
        1.0,
        TessellationOptions::default(),
        [2048, 64],
        Vec::new(),
    );
    // Einlauf
    let _ = tess.tessellate_shapes(shapes.clone());
    let t0 = std::time::Instant::now();
    let mut scheitel = 0usize;
    for _ in 0..durchlaeufe {
        let mut tess = Tessellator::new(
            1.0,
            TessellationOptions::default(),
            [2048, 64],
            Vec::new(),
        );
        let prims = tess.tessellate_shapes(shapes.clone());
        scheitel = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                _ => 0,
            })
            .sum();
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / durchlaeufe as f64;
    println!(
        "{name}: n={n} breite={w}px  formen={}  scheitel={scheitel}  {us:.2} µs",
        shapes.len()
    );
}

fn main() {
    println!("## Zeichenaufwand je Bogen (Viertelbogen, 1000 px Radius)");
    // Segmentzahlen aus der Messung von mess_bogen: r=1000px -> alt 22, neu 44
    for (w, name) in [(1.0f32, "dünn 1 px"), (3.0, "breit 3 px")] {
        messen(&format!("alt  {name}"), true, 22, w, 2000);
        messen(&format!("neu  {name}"), false, 44, w, 2000);
    }
    println!("\n## Zeichenaufwand je Bogen (Viertelbogen, 2400 px Radius)");
    for (w, name) in [(1.0f32, "dünn 1 px"), (3.0, "breit 3 px")] {
        messen(&format!("alt  {name}"), true, 36, w, 2000);
        messen(&format!("neu  {name}"), false, 71, w, 2000);
    }
    println!("\n## Wohnhaus: 23 Türbögen, r≈785 mm bei 6 % (je 13 Sehnen alt / 24 neu)");
    let alt: Vec<ClippedShape> = (0..23).flat_map(|_| formen(true, 13, 1.0)).collect();
    let neu: Vec<ClippedShape> = (0..23).flat_map(|_| formen(false, 24, 1.0)).collect();
    for (name, shapes) in [("alt", alt), ("neu", neu)] {
        let t0 = std::time::Instant::now();
        let mut scheitel = 0usize;
        for _ in 0..2000 {
            let mut tess =
                Tessellator::new(1.0, TessellationOptions::default(), [2048, 64], Vec::new());
            scheitel = tess
                .tessellate_shapes(shapes.clone())
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                    _ => 0,
                })
                .sum();
        }
        println!(
            "{name}: formen={} scheitel={scheitel} {:.2} µs je Bild",
            shapes.len(),
            t0.elapsed().as_secs_f64() * 1e6 / 2000.0
        );
    }
}
