//! Vollellipse mit Pfeilhöhen-Regel (T0174).
//!
//! Vorher zeichneten Zeichenfläche, PNG und PDF die **volle** Ellipse mit
//! festen 72 Segmenten, während der Ellipsenbogen daneben die Pfeilhöhen-Regel
//! benutzte: bei 30 px/mm 2,85 px Abweichung (sichtbare Ecken), bei 0,02 px/mm
//! 72 Segmente, wo 9 genügen.
//!
//! Geprüft wird die eine Quelle (`CurveQuality::ellipse_punkte`, die alle drei
//! Renderer aufrufen) und das Rasterbild: bei 30 px/mm muss die wahre Kurve
//! überall Tinte haben — mit 72 Segmenten fehlt sie in der Sehnenmitte.

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{dist_point_segment, BBox, Ellipse, Vec2};
use jjcad_core::model::{Document, Entity, EntityKind, EntityStyle};
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::style::{Color, LineType};
use jjcad_core::tessellate::CurveQuality;
use std::f64::consts::TAU;

/// Pfeilhöhe je Segment bei der festen Zahl aus der Zeit vor T0174.
const ALT_SEGMENTE: usize = 72;

/// Punktfolge wie vor T0174: feste 72 Segmente, gleich im Parameter.
fn alte_punkte(el: &Ellipse) -> Vec<Vec2> {
    (0..=ALT_SEGMENTE)
        .map(|i| el.point_at(i as f64 / ALT_SEGMENTE as f64 * TAU))
        .collect()
}

/// Größte Pfeilhöhe in Ausgabe-Einheiten: Abstand wahrer Ellipsenpunkte zur
/// Sehne. Beide Punktfolgen liegen auf gleichen Parameterabständen (0 … TAU),
/// deshalb ist der Parameter eines Zwischenpunkts `(i + t) / n · TAU`.
fn pfeilhoehe(el: &Ellipse, pts: &[Vec2], out_per_unit: f64) -> f64 {
    let n = pts.len() - 1;
    let mut max = 0.0f64;
    for i in 0..n {
        let (a, b) = (pts[i], pts[i + 1]);
        for k in 0..=64 {
            let u = (i as f64 + k as f64 / 64.0) / n as f64 * TAU;
            max = max.max(dist_point_segment(el.point_at(u), a, b));
        }
    }
    max * out_per_unit
}

/// Die Abnahme aus dem Ticket: bei 0,02 / 1 / 30 px je mm höchstens 0,2 px.
#[test]
fn pfeilhoehe_bleibt_unter_0_2_px() {
    let el = Ellipse::new(Vec2::ZERO, 100.0, 60.0, 0.3);
    for zoom in [0.02f64, 1.0, 30.0] {
        let pts = CurveQuality::SCREEN.ellipse_punkte(&el, zoom);
        let h = pfeilhoehe(&el, &pts, zoom);
        assert!(
            h <= 0.2,
            "bei {zoom} px/mm ist die Pfeilhöhe {h:.3} px, erlaubt sind 0,2 px"
        );
    }
}

/// Wenige Segmente, wo wenig zu sehen ist: bei 0,02 px/mm genügen für eine
/// 100-mm-Ellipse neun — vorher waren es 72.
#[test]
fn bei_kleinem_zoom_bleibt_die_segmentzahl_klein() {
    let el = Ellipse::new(Vec2::ZERO, 100.0, 60.0, 0.0);
    let pts = CurveQuality::SCREEN.ellipse_punkte(&el, 0.02);
    let n = pts.len() - 1;
    assert!(
        n <= 12,
        "bei 0,02 px/mm sind es {n} Segmente, höchstens 12 sind erlaubt"
    );
    assert!(n >= 4, "eine Ellipse braucht mindestens vier Segmente ({n})");
}

/// Mehr Zoom = mehr Segmente; der Druck ist feiner als der Schirm (0,05 gegen
/// 0,15 Ausgabe-Einheiten) — dieselbe Ordnung wie bei Bogen und Kreis.
#[test]
fn segmentzahl_folgt_zoom_und_drucktoleranz() {
    let el = Ellipse::new(Vec2::ZERO, 100.0, 60.0, 0.0);
    let grob = CurveQuality::SCREEN.ellipse_punkte(&el, 0.02).len();
    let mittel = CurveQuality::SCREEN.ellipse_punkte(&el, 1.0).len();
    let fein = CurveQuality::SCREEN.ellipse_punkte(&el, 30.0).len();
    assert!(grob < mittel && mittel < fein, "{grob} / {mittel} / {fein}");

    let schirm = CurveQuality::SCREEN.ellipse_punkte(&el, 1.0).len();
    let druck = CurveQuality::PRINT.ellipse_punkte(&el, 1.0).len();
    assert!(
        druck > schirm,
        "Druck (0,05) muss feiner sein als der Schirm (0,15): {druck} gegen {schirm}"
    );
}

/// Die Punktfolge ist geschlossen (Anfangspunkt am Ende noch einmal) — der
/// Zeichenweg macht daraus einen `closed_line`-Zug ohne doppelten Punkt.
#[test]
fn punktfolge_ist_geschlossen() {
    let el = Ellipse::new(Vec2::new(120.0, -30.0), 100.0, 60.0, 0.7);
    let pts = CurveQuality::SCREEN.ellipse_punkte(&el, 1.0);
    assert!(pts.len() > 8);
    assert!(
        (pts[0] - pts[pts.len() - 1]).len() < 1e-6,
        "Anfang und Ende müssen zusammenfallen"
    );
}

/// Gegenprobe für das Maß selbst: mit den festen 72 Segmenten wäre die
/// Pfeilhöhe bei 30 px/mm 2,85 px — der Test oben beißt also.
#[test]
fn die_alte_feste_segmentzahl_war_bei_30_px_mm_zu_grob() {
    let el = Ellipse::new(Vec2::ZERO, 100.0, 60.0, 0.3);
    let h = pfeilhoehe(&el, &alte_punkte(&el), 30.0);
    assert!(
        h > 2.5,
        "die alte Regel müsste 2,85 px Abweichung zeigen, gemessen {h:.3} px"
    );
}

/// Rasterbild bei 30 px/mm: jeder Punkt der wahren Ellipse muss Tinte in der
/// Nähe haben. Mit 72 Segmenten liegt die Kurve in der Sehnenmitte 2,85 px
/// neben dem Strich — dort wäre keine Tinte.
#[test]
fn png_zeichnet_die_volle_ellipse_glatt() {
    let el = Ellipse::new(Vec2::ZERO, 100.0, 60.0, 0.0);
    let mut doc = Document::new();
    let layer = doc.layers.first().map(|l| l.id).unwrap_or(0);
    doc.add_entity(Entity {
        id: 0,
        kind: EntityKind::Ellipse {
            c: el.c,
            rx: el.rx,
            ry: el.ry,
            rot: el.rot,
        },
        layer,
        style: EntityStyle {
            color: Some(Color::BLACK),
            line_type: Some(LineType::Solid),
            // Dünn, damit eine Sehne von 2,85 px als Lücke auffällt.
            line_weight: Some(0.05),
            ..Default::default()
        },
        by_block: false,
    });

    // Ausschnitt um den Scheitel (100, 0) — dort liegt die Sehnenmitte, an der
    // die Abweichung am größten ist.
    let (w, h) = (318u32, 318u32);
    let opts = RasterOptions {
        width_px: w,
        height_px: h,
        bbox: Some([95.0, -5.0, 105.0, 5.0]),
        paper: false,
        annot_scale: Some(1.0),
        sheet: None,
    };
    let pm = render_model_pixmap(&doc, &opts).expect("Rasterbild");

    // Abbildung wie `render_model_pixmap`: 3 % Rand um den Ausschnitt, dann das
    // kleinere der beiden Maße.
    let b = BBox::new(Vec2::new(95.0, -5.0), Vec2::new(105.0, 5.0));
    let b = b.expanded(b.width().max(b.height()) * 0.03);
    let k = (w as f64 / b.width()).min(h as f64 / b.height());
    let ox = -b.min.x * k + (w as f64 - b.width() * k) * 0.5;
    let oy = b.max.y * k + (h as f64 - b.height() * k) * 0.5;
    assert!(
        (k - 30.0).abs() < 0.01,
        "die Messung rechnet mit 30 px/mm, die Abbildung liefert {k:.3}"
    );

    let tinte = |x: isize, y: isize| -> bool {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            return false;
        }
        match pm.pixel(x as u32, y as u32) {
            Some(p) => {
                let summe = p.red() as u32 + p.green() as u32 + p.blue() as u32;
                summe / 3 < 128
            }
            None => false,
        }
    };

    let mut schlimmste = 0.0f64;
    // Der Scheitel läuft von y = −4,2 bis +4,2 (innerhalb des Ausschnitts).
    let u_max = (4.2f64 / el.ry).asin();
    for i in 0..=400 {
        let u = -u_max + 2.0 * u_max * i as f64 / 400.0;
        let q = el.point_at(u);
        let (x, y) = ((ox + q.x * k).round() as isize, (oy - q.y * k).round() as isize);
        let mut best = f64::MAX;
        for dy in -4isize..=4 {
            for dx in -4isize..=4 {
                if tinte(x + dx, y + dy) {
                    best = best.min(((dx * dx + dy * dy) as f64).sqrt());
                }
            }
        }
        assert!(
            best < 3.0,
            "keine Tinte an der wahren Kurve bei u = {u:.4} ({q:?})"
        );
        schlimmste = schlimmste.max(best);
    }
    assert!(
        schlimmste <= 1.2,
        "die Kurve liegt bis zu {schlimmste:.2} px neben dem Strich — mit 72 \
         Segmenten wären es rund 2 px"
    );
}
