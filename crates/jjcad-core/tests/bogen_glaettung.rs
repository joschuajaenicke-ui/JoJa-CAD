//! Nachweis für die Bogenglättung (Karte t_cde0fdde).
//!
//! Ausgangslage der Meldung („Bögen wirken kantig"): ein 1,5-m-Bogen bei 6 %
//! Zoom wurde mit 7 Sehnen gezeichnet — 13° je Knick, 0,6 px Abweichung
//! zwischen Sehne und wahrer Kurve, sichtbar als Polygonzug.
//!
//! Geprüft wird über die öffentliche Befehlsschicht (wie GUI und MCP):
//! 1. Im Basisszenario liegt die Pfeilhöhe unter 0,15 px und der Knickwinkel
//!    unter 8°.
//! 2. Grenzfälle — Radius 0, sehr kleiner und sehr großer Radius, Vollkreis —
//!    rendern und exportieren ohne Panik, mit endlichen Koordinaten.

use jjcad_core::geom::{Arc, Vec2};
use jjcad_core::model::{Entity, EntityKind};
use jjcad_core::pdf;
use jjcad_core::raster;
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::tessellate::CurveQuality;
use jjcad_core::export::RasterOptions;
use jjcad_core::Document;
use serde_json::json;

/// Zoom des Basisszenarios: 6 % — so wird ein 1,5-m-Bogen in der Passansicht
/// auf rund 95 px Radius abgebildet (Ausgangslage der Meldung).
const ZOOM_PX_PRO_MM: f64 = 0.063;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

/// Größte Pfeilhöhe (Abstand Sehnenmitte ↔ Kreis) in Welt-Millimetern.
fn max_sagitta(arc: &Arc, pts: &[Vec2]) -> f64 {
    pts.windows(2)
        .map(|w| {
            let mid = (w[0] + w[1]) * 0.5;
            (arc.r - (mid - arc.c).len()).abs()
        })
        .fold(0.0, f64::max)
}

fn bogen_aus_displayliste(doc: &Document) -> Arc {
    let dl = render::render_document(&RenderCtx::new(doc));
    dl.prims
        .iter()
        .find_map(|p| match &p.shape {
            Shape::Arc { c, r, a0, a1 } => Some(Arc::new(*c, *r, *a0, *a1)),
            _ => None,
        })
        .expect("kein Bogen in der Displayliste")
}

#[test]
fn basisszenario_bogen_ist_glatt() {
    let mut s = session();
    // 1,5 m Radius, Viertelkreis — wie ein Türblattbogen im Wohnhaus.
    s.execute(
        "draw.arc",
        // Winkel in Grad (Befehlsschicht), Radius in Metern (input_unit)
        &json!({"c": [0.0, 0.0], "r": 1.5, "a0": 0.0, "a1": 90.0}),
    )
    .unwrap();

    let arc = bogen_aus_displayliste(&s.doc);
    assert!((arc.r - 1500.0).abs() < 1e-9, "Radius {}", arc.r);

    let n = CurveQuality::SCREEN.segments(&arc, ZOOM_PX_PRO_MM);
    let knick_grad = (arc.sweep() / n as f64).to_degrees();
    assert!(
        n >= 13,
        "zu wenige Segmente für 6 % Zoom: {n} (vorher 7) — Knick {knick_grad:.1}°"
    );
    assert!(
        knick_grad <= 8.0,
        "Knickwinkel {knick_grad:.1}° über der Wahrnehmungsschwelle"
    );

    let pts = CurveQuality::SCREEN.flatten(&arc, ZOOM_PX_PRO_MM);
    let tol_world = CurveQuality::SCREEN.max_sagitta / ZOOM_PX_PRO_MM;
    let h = max_sagitta(&arc, &pts);
    assert!(
        h <= tol_world * 1.001,
        "Pfeilhöhe {h:.3} mm > {tol_world:.3} mm (0,15 px)"
    );
}

/// Baut ein Dokument mit den Grenzfällen direkt im Modell — so, wie es eine
/// Datei enthalten kann (die Aktion `draw.arc` prüft keine Radien).
fn grenzfall_dokument() -> Document {
    let mut doc = Document::new();
    let layer = doc.layers[0].id;
    let faelle = [
        ("Radius 0", 0.0, 0.0, std::f64::consts::FRAC_PI_2),
        ("Radius 0,25 mm", 0.25, 0.0, std::f64::consts::PI),
        ("Radius 200 m", 200_000.0, 3.10, 3.19),
        ("Vollkreis (a0 == a1)", 400.0, 0.0, 0.0),
    ];
    for (i, (name, r, a0, a1)) in faelle.iter().enumerate() {
        let id = doc.new_id();
        let mut e = Entity::new(
            id,
            layer,
            EntityKind::Arc {
                c: Vec2::new(i as f64 * 1000.0, 0.0),
                r: *r,
                a0: *a0,
                a1: *a1,
            },
        );
        e.style.line_weight = Some(0.5);
        doc.add_entity(e);
        let _ = name;
    }
    doc
}

#[test]
fn grenzfaelle_rendern_ohne_panik() {
    let doc = grenzfall_dokument();
    let dl = render::render_document(&RenderCtx::new(&doc));
    let bogen: Vec<Arc> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Arc { c, r, a0, a1 } => Some(Arc::new(*c, *r, *a0, *a1)),
            _ => None,
        })
        .collect();
    assert_eq!(bogen.len(), 4, "nicht alle Bögen in der Displayliste");

    for zoom in [0.001, 0.063, 1.0, 20.0] {
        for arc in &bogen {
            let pts = CurveQuality::SCREEN.flatten(arc, zoom);
            assert!(pts.len() >= 2, "zu wenige Punkte bei zoom {zoom}");
            assert!(
                pts.iter().all(|p| p.x.is_finite() && p.y.is_finite()),
                "nicht-endliche Koordinaten bei zoom {zoom}, r = {}",
                arc.r
            );
        }
    }
}

#[test]
fn grenzfaelle_lassen_sich_exportieren() {
    let doc = grenzfall_dokument();

    let opt = RasterOptions {
        width_px: 400,
        height_px: 300,
        bbox: None,
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let png = raster::png_bytes(&doc, &opt).expect("PNG-Export fehlgeschlagen");
    assert!(png.len() > 1000, "PNG zu klein: {} Byte", png.len());

    let pdf = pdf::pdf_bytes(&doc, None).expect("PDF-Export fehlgeschlagen");
    assert!(pdf.starts_with(b"%PDF"), "kein PDF-Kopf");
    assert!(pdf.len() > 500, "PDF zu klein: {} Byte", pdf.len());
}

#[test]
fn vollkreis_bogen_wird_im_png_gezeichnet() {
    // Gegenprobe, dass der Bogen im Raster-Export wirklich Tinte erzeugt
    // (nicht nur fehlerfrei durchläuft).
    let mut doc = Document::new();
    let layer = doc.layers[0].id;
    let id = doc.new_id();
    let mut e = Entity::new(
        id,
        layer,
        EntityKind::Arc {
            c: Vec2::ZERO,
            r: 400.0,
            a0: 0.0,
            a1: 0.0,
        },
    );
    e.style.line_weight = Some(0.5);
    doc.add_entity(e);

    let opt = RasterOptions {
        width_px: 300,
        height_px: 300,
        bbox: None,
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let png = raster::png_bytes(&doc, &opt).unwrap();
    assert!(!png.is_empty());
    // PNG dekodieren wäre ein zusätzlicher Aufwand; die Bytes des IDAT-Chunks
    // sind komprimiert. Stattdessen: das Pixmap direkt rendern und Tinte zählen.
    let pm = raster::render_model_pixmap(&doc, &opt).unwrap();
    let dunkel = pm
        .pixels()
        .iter()
        .filter(|p| p.alpha() > 128 && p.red() < 100 && p.green() < 100 && p.blue() < 100)
        .count();
    assert!(dunkel > 200, "kaum Tinte im Bild: {dunkel} Pixel");
}
