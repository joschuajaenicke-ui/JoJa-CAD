//! Zerlegen von Schraffuren (T0138): jedes Muster löst sich in Einzelteile auf,
//! und das Ergebnis sieht aus wie vorher.

use jjcad_core::geom::Vec2;
use jjcad_core::model::{Document, Entity, EntityKind};
use jjcad_core::style::{Color, FillPattern};

/// Quadrat 3 × 2 m mit dem Muster, Randlinie an.
fn doc_mit_muster(pattern: FillPattern) -> Document {
    let mut d = Document::new();
    // Halbgraue, dünne Linien: so ist der Unterschied zweier Zeichenwege an
    // den Stößen messbar (mit Schwarz überstrahlt die Kantenglättung).
    let layer = d.ensure_layer("Zeichenprobe", Color::rgb(150, 155, 160));
    if let Some(l) = d.layer_mut(layer) {
        l.line_weight = 0.18;
    }
    let boundary = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(3000.0, 0.0),
        Vec2::new(3000.0, 2000.0),
        Vec2::new(0.0, 2000.0),
    ];
    let mut e = Entity::new(
        1,
        layer,
        EntityKind::Fill {
            boundary,
            holes: Vec::new(),
            pattern,
            pattern_angle: 0.0,
            pattern_scale: 200.0,
            border: true,
        },
    );
    e.id = 1;
    d.add_entity(e);
    d
}

fn bild(d: &Document) -> image::RgbaImage {
    let opts = jjcad_core::export::RasterOptions {
        width_px: 900,
        height_px: 600,
        bbox: Some([-150.0, -150.0, 3150.0, 2150.0]),
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let png = jjcad_core::raster::png_bytes(d, &opts).expect("PNG rendern");
    image::load_from_memory(&png).expect("PNG lesen").to_rgba8()
}

/// Anteil der Bildpunkte, die sich sichtbar (Summe der Kanäle > 60) unterscheiden.
fn abweichung(a: &image::RgbaImage, b: &image::RgbaImage) -> f64 {
    assert_eq!(a.dimensions(), b.dimensions());
    let mut n = 0usize;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        let d: i32 = (0..3)
            .map(|k| (pa[k] as i32 - pb[k] as i32).abs())
            .sum::<i32>();
        if d > 60 {
            n += 1;
        }
    }
    n as f64 / (a.width() * a.height()) as f64
}

#[test]
fn jedes_muster_zerlegt_sich_in_einzelteile() {
    for p in FillPattern::ALL {
        let mut d = doc_mit_muster(*p);
        let ids = jjcad_core::actions::explode_entity(&mut d, 1).expect("zerlegen");
        assert!(!ids.is_empty(), "{:?}: nichts entstanden", p);
        assert!(
            d.entity(1).is_none(),
            "{:?}: das Ausgangsobjekt steht noch",
            p
        );
        assert_eq!(d.ents().len(), ids.len(), "{:?}: Objektzahl", p);
        // Kein Einzelteil trägt das Muster weiter — es ist ja aufgelöst.
        for id in ids {
            if let Some(e) = d.entity(id) {
                if let EntityKind::Fill { pattern, .. } = &e.kind {
                    assert_eq!(*pattern, FillPattern::Solid, "{:?}: Muster blieb", p);
                }
            }
        }
    }
}

#[test]
fn zerlegte_schraffur_sieht_aus_wie_vorher() {
    for p in FillPattern::ALL {
        let mut d = doc_mit_muster(*p);
        let vorher = bild(&d);
        jjcad_core::actions::explode_entity(&mut d, 1).expect("zerlegen");
        let nachher = bild(&d);
        let ab = abweichung(&vorher, &nachher);
        // Dämmung ist ein dichter Polygonzug: die Füllfläche zeichnet ihn als
        // **einen** Zug (T0164), die zerlegte Fassung besteht aus einzelnen
        // Strecken. Die Geometrie ist dieselbe, nur die Kantenglättung an den
        // Stößen unterscheidet sich (gemessen 0,43 % — ohne den Zug wären es
        // 0,00 %, dann aber 144 000 Primitiva für 100 m²).
        let grenze = if *p == FillPattern::InsulationHatch {
            0.006
        } else {
            0.002
        };
        assert!(
            ab < grenze,
            "{:?}: {:.2} % der Bildpunkte unterscheiden sich",
            p,
            ab * 100.0
        );
    }
}

#[test]
fn musterpunkte_behalten_ihre_groesse() {
    let mut d = doc_mit_muster(FillPattern::Dots);
    let ids = jjcad_core::actions::explode_entity(&mut d, 1).expect("zerlegen");
    let mut durchmesser = Vec::new();
    for id in ids {
        if let Some(e) = d.entity(id) {
            // Nur die Musterpunkte — die Randlinie ist auch ein Einzelteil.
            if let EntityKind::Circle { r, .. } = &e.kind {
                durchmesser.push(2.0 * r);
                assert!(
                    e.style.fill.is_some(),
                    "der Musterpunkt muss gefüllt sein — sonst ist er nur ein Ring"
                );
            }
        }
    }
    assert!(!durchmesser.is_empty(), "keine Punkte entstanden");
    let max = durchmesser.iter().cloned().fold(0.0f64, f64::max);
    // Musterabstand 200 mm, Radius 0,035 · 200 = 7 mm → Durchmesser ≤ 14 mm.
    // Ein Punktobjekt zeichnete bei 1:100 mindestens 64 mm.
    assert!(max < 20.0, "größter Musterpunkt {max:.1} mm");
}
