//! Messprogramm: Bogen-Unterteilung vor/nach der Glättung (t_aea1b20d).
//!
//! Vergleicht den alten Weg (`Arc::flatten(0.6 px / zoom)`, wie ihn
//! `canvas.rs` vor der Änderung aufrief) mit der neuen Vorgabe
//! `CurveQuality::SCREEN`. Ausgabe: Segmentzahl, Pfeilhöhe in px und
//! Knickwinkel je Fuge — die drei Größen, an denen "kantig" messbar ist.
//!
//! Aufruf: cargo run --release -p jjcad-core --example mess_bogen
//! (Wegwerfprogramm, wird nach der Messung gelöscht.)

use jjcad_core::geom::{Arc, Vec2};
use jjcad_core::tessellate::CurveQuality;
use std::time::Instant;

/// Größte Pfeilhöhe über alle Sehnen, in Bildschirm-Pixeln.
fn pfeilhoehe_px(arc: &Arc, pts: &[Vec2], zoom: f64) -> f64 {
    pts.windows(2)
        .map(|w| {
            let mid = (w[0] + w[1]) * 0.5;
            (arc.r - (mid - arc.c).len()).abs() * zoom
        })
        .fold(0.0, f64::max)
}

fn knick_deg(arc: &Arc, n: usize) -> f64 {
    arc.sweep().to_degrees() / n as f64
}

fn main() {
    // ---------------------------------------------------------------
    // 1. Bildschirm: alter Weg gegen neuen Weg, Viertelbogen
    // ---------------------------------------------------------------
    println!("## Bildschirm (Viertelbogen), zoom = Abbildung Welt-mm -> px");
    println!("r_px\tzoom\tn_alt\tknick_alt\tsag_alt_px\tn_neu\tknick_neu\tsag_neu_px\tfaktor_n");
    for r_px in [20.0f64, 50.0, 100.0, 300.0, 900.0, 2400.0, 6000.0] {
        for zoom in [0.02f64, 0.06, 0.25, 1.0, 8.0] {
            let arc = Arc::new(Vec2::new(0.0, 0.0), r_px / zoom, 0.0, std::f64::consts::FRAC_PI_2);
            let alt = arc.flatten(0.6 / zoom);
            let neu = CurveQuality::SCREEN.flatten(&arc, zoom);
            let (na, nn) = (alt.len() - 1, neu.len() - 1);
            println!(
                "{:.0}\t{}\t{}\t{:.1}\t{:.3}\t{}\t{:.1}\t{:.3}\t{:.2}",
                r_px,
                zoom,
                na,
                knick_deg(&arc, na),
                pfeilhoehe_px(&arc, &alt, zoom),
                nn,
                knick_deg(&arc, nn),
                pfeilhoehe_px(&arc, &neu, zoom),
                nn as f64 / na as f64
            );
        }
    }

    // ---------------------------------------------------------------
    // 2. Druck (PDF/PNG): alt 0,08 Papier-mm gegen PRINT 0,05
    // ---------------------------------------------------------------
    println!("\n## Druck (Viertelbogen), paper_scale = Papier-mm je Welt-mm");
    println!("paper_scale\tn_alt\tsag_alt_papier_mm\tn_neu\tsag_neu_papier_mm");
    for ps in [0.01f64, 0.02, 0.05, 0.1] {
        let arc = Arc::new(Vec2::ZERO, 1500.0, 0.0, std::f64::consts::FRAC_PI_2);
        let alt = arc.flatten(0.08 / ps);
        let neu = CurveQuality::PRINT.flatten(&arc, ps);
        println!(
            "{}\t{}\t{:.4}\t{}\t{:.4}",
            ps,
            alt.len() - 1,
            pfeilhoehe_px(&arc, &alt, ps),
            neu.len() - 1,
            pfeilhoehe_px(&arc, &neu, ps)
        );
    }

    // ---------------------------------------------------------------
    // 3. Grenzfälle: darf nicht panisch werden und nicht ausufern
    // ---------------------------------------------------------------
    println!("\n## Grenzfälle");
    let faelle: [(&str, Arc); 6] = [
        ("Radius 0", Arc::new(Vec2::new(12.0, 34.0), 0.0, 0.0, 1.0)),
        ("Radius 0,25 mm", Arc::new(Vec2::ZERO, 0.25, 0.0, 1.0)),
        ("Vollkreis r=400", Arc::new(Vec2::ZERO, 400.0, 0.0, 0.0)),
        ("Riesenbogen r=200 m", Arc::new(Vec2::ZERO, 200_000.0, 0.0, 1.5)),
        ("winziger Winkel", Arc::new(Vec2::ZERO, 900.0, 1.0, 1.0 + 1e-7)),
        ("fast Vollkreis", Arc::new(Vec2::ZERO, 900.0, 0.0, 6.283)),
    ];
    for (name, arc) in faelle.iter() {
        let pts = CurveQuality::SCREEN.flatten(arc, 0.06);
        let end_ok = (pts[0] - arc.start()).len() < 1e-6;
        let alle_auf_kreis = pts
            .iter()
            .all(|p| ((*p - arc.c).len() - arc.r).abs() < 1e-6 || arc.r < 1e-9);
        println!(
            "{name}: n={} start_trifft={} punkte_auf_kreis={}",
            pts.len() - 1,
            end_ok,
            alle_auf_kreis
        );
    }

    // ---------------------------------------------------------------
    // 4. Renderzeit: Unterteilung eines Plans (23 Bögen) und Lastfall
    // ---------------------------------------------------------------
    println!("\n## Renderzeit der Unterteilung (Mittel über 2000 Durchläufe)");
    let mut bau = || {
        let mut arcs = Vec::new();
        for i in 0..23 {
            let r_mm = 785.0 + i as f64 * 40.0;
            arcs.push(Arc::new(Vec2::new(i as f64 * 100.0, 0.0), r_mm, 0.0, 1.4));
        }
        arcs
    };
    let plan = bau();
    let zoom = 0.06;
    let t0 = Instant::now();
    let mut sum = 0usize;
    for _ in 0..2000 {
        for a in &plan {
            sum += a.flatten(0.6 / zoom).len();
        }
    }
    let alt_us = t0.elapsed().as_secs_f64() * 1e6 / 2000.0;
    let t0 = Instant::now();
    for _ in 0..2000 {
        for a in &plan {
            sum += CurveQuality::SCREEN.flatten(a, zoom).len();
        }
    }
    let neu_us = t0.elapsed().as_secs_f64() * 1e6 / 2000.0;
    println!("Wohnhaus-Plan (23 Bögen, zoom 0,06): alt {alt_us:.2} µs  neu {neu_us:.2} µs  ({:+.1} %)", (neu_us / alt_us - 1.0) * 100.0);

    // Lastfall: 200 Bögen, große Radien, starker Zoom
    let mut last: Vec<Arc> = Vec::new();
    for i in 0..200 {
        let r_px = 50.0 + (i as f64) * 25.0;
        last.push(Arc::new(Vec2::new(i as f64, 0.0), r_px / 8.0, 0.0, 2.8));
    }
    let zoom = 8.0;
    let t0 = Instant::now();
    for _ in 0..500 {
        for a in &last {
            sum += a.flatten(0.6 / zoom).len();
        }
    }
    let alt_us = t0.elapsed().as_secs_f64() * 1e6 / 500.0;
    let t0 = Instant::now();
    for _ in 0..500 {
        for a in &last {
            sum += CurveQuality::SCREEN.flatten(a, zoom).len();
        }
    }
    let neu_us = t0.elapsed().as_secs_f64() * 1e6 / 500.0;
    println!("Lastfall (200 Bögen bis 5000 px, zoom 8): alt {alt_us:.2} µs  neu {neu_us:.2} µs  ({:+.1} %)", (neu_us / alt_us - 1.0) * 100.0);
    println!("(Kontrollsumme, damit nichts wegoptimiert wird: {sum})");
}
