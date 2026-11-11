//! Messprogramm T0174: Vollellipse mit Pfeilhöhen-Regel.
//!
//! Zwei Messungen, die das Ticket verlangt:
//!
//! 1. **Pfeilhöhe und Segmentzahl** der vollen Ellipse bei 0,02 / 1 / 30 px je
//!    mm — alt (fest 72 Segmente, der Stand vor T0174) gegen neu
//!    (`Ellipse::arc_points` mit der Pfeilhöhe aus `CurveQuality`).
//! 2. **PNG mit 300 dpi** (11,811 px je mm) derselben Ellipse für den
//!    Pixelvergleich vor/nach („der Ausdruck soll gleich aussehen“).
//!
//! Aufruf: `cargo run --release -p jjcad-core --example mess_vollellipse [ziel.png]`
//!
//! Die Pfeilhöhe wird nicht genähert: für jede Sehne werden 64 wahre
//! Ellipsenpunkte auf ihren Abstand zur Sehne geprüft (größter Wert).

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{dist_point_segment, Ellipse, Vec2};
use jjcad_core::model::{Document, Entity, EntityKind, EntityStyle};
use jjcad_core::raster::render_png;
use jjcad_core::tessellate::CurveQuality;
use std::f64::consts::TAU;

/// Segmentzahl des Stands vor T0174 (fest in `canvas.rs`, `raster.rs`, `pdf.rs`).
const ALT_SEGMENTE: usize = 72;

/// Punktfolge wie vor T0174: feste 72 Segmente, gleich im Parameter.
fn alte_punkte(el: &Ellipse) -> Vec<Vec2> {
    (0..=ALT_SEGMENTE)
        .map(|i| el.point_at(i as f64 / ALT_SEGMENTE as f64 * TAU))
        .collect()
}

/// Größte Pfeilhöhe in px: Abstand wahrer Ellipsenpunkte zur Sehne.
///
/// Beide Punktfolgen liegen auf gleichen Parameterabständen (0…TAU), deshalb
/// ist der Parameter eines Zwischenpunkts `(i + t) / n · TAU`.
fn pfeilhoehe_px(el: &Ellipse, pts: &[Vec2], zoom: f64) -> f64 {
    let n = pts.len() - 1;
    let mut max = 0.0f64;
    for i in 0..n {
        let (a, b) = (pts[i], pts[i + 1]);
        for k in 0..=64 {
            let u = (i as f64 + k as f64 / 64.0) / n as f64 * TAU;
            max = max.max(dist_point_segment(el.point_at(u), a, b));
        }
    }
    max * zoom
}

/// Zeichnung mit **einer** Ellipse (rx 100, ry 60, gedreht) für das PNG.
fn probe_dokument() -> Document {
    let mut doc = Document::new();
    let layer = doc.layers.first().map(|l| l.id).unwrap_or(0);
    doc.add_entity(Entity {
        id: 0,
        kind: EntityKind::Ellipse {
            c: Vec2::new(0.0, 0.0),
            rx: 100.0,
            ry: 60.0,
            rot: 0.3,
        },
        layer,
        style: EntityStyle::default(),
        by_block: false,
    });
    doc
}

fn main() {
    println!("## Vollellipse: Pfeilhöhe und Segmentzahl");
    println!("zoom_px_mm\tn_alt\tsag_alt_px\tn_neu\tsag_neu_px");
    let el = Ellipse::new(Vec2::ZERO, 100.0, 60.0, 0.3);
    for zoom in [0.02f64, 1.0, 30.0] {
        let alt = alte_punkte(&el);
        // Dieselbe Rechnung wie die Zeichenfläche: Pfeilhöhe in px, aus der
        // Abbildung in Welt-Millimeter umgerechnet.
        let tol = CurveQuality::SCREEN.max_sagitta / zoom;
        let neu = el.arc_points(0.0, 0.0, tol);
        println!(
            "{}\t{}\t{:.3}\t{}\t{:.3}",
            zoom,
            alt.len() - 1,
            pfeilhoehe_px(&el, &alt, zoom),
            neu.len() - 1,
            pfeilhoehe_px(&el, &neu, zoom)
        );
    }

    // Druck: dieselbe Rechnung mit der Drucktoleranz (0,05 Papier-mm). Der PDF-
    // Schreiber rechnet mit Papier-mm je Welt-mm, das PNG mit px je Welt-mm —
    // beide Male gilt die Pfeilhöhe in Ausgabe-Einheiten.
    println!("\n## Druck");
    println!("fall\tskala\tn_alt\tsag_alt\tn_neu\tsag_neu");
    for (name, skala) in [("PDF 1:100", 0.01f64), ("PDF 1:50", 0.02), ("PDF 1:20", 0.05)] {
        let alt = alte_punkte(&el);
        let tol = CurveQuality::PRINT.max_sagitta / skala;
        let neu = el.arc_points(0.0, 0.0, tol);
        println!(
            "{}\t{}\t{}\t{:.4} mm\t{}\t{:.4} mm",
            name,
            skala,
            alt.len() - 1,
            pfeilhoehe_px(&el, &alt, skala),
            neu.len() - 1,
            pfeilhoehe_px(&el, &neu, skala)
        );
    }
    // PNG 300 dpi: 11,811 px je mm, Toleranz 0,05 px (wie `raster.rs`).
    let png_skala = 300.0f64 / 25.4;
    let alt = alte_punkte(&el);
    let neu = el.arc_points(0.0, 0.0, CurveQuality::PRINT.max_sagitta / png_skala);
    println!(
        "PNG 300 dpi\t{:.4}\t{}\t{:.4} px\t{}\t{:.4} px",
        png_skala,
        alt.len() - 1,
        pfeilhoehe_px(&el, &alt, png_skala),
        neu.len() - 1,
        pfeilhoehe_px(&el, &neu, png_skala)
    );

    let ziel = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "vollellipse-300dpi.png".to_string());
    let doc = probe_dokument();
    // Ausschnitt 300 × 220 mm um die Ellipse; `render_png` erweitert um 3 %
    // der längeren Seite (9 mm), damit die Linienbreite nicht am Rand klebt.
    let (breite_mm, hoehe_mm) = (300.0f64 + 9.0, 220.0f64 + 9.0);
    let k = 300.0f64 / 25.4;
    let opts = RasterOptions {
        width_px: (breite_mm * k).round() as u32,
        height_px: (hoehe_mm * k).round() as u32,
        bbox: Some([-150.0, -110.0, 150.0, 110.0]),
        paper: false,
        annot_scale: Some(1.0),
        sheet: None,
    };
    let echt_k = (opts.width_px as f64 / breite_mm).min(opts.height_px as f64 / hoehe_mm);
    render_png(&doc, &opts, std::path::Path::new(&ziel)).expect("PNG schreiben");
    println!(
        "\nPNG: {ziel} — {} × {} px, {:.3} px/mm ({:.1} dpi)",
        opts.width_px,
        opts.height_px,
        echt_k,
        echt_k * 25.4
    );
}
