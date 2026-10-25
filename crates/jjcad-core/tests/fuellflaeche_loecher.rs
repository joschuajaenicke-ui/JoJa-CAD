//! Füllfläche mit Löchern (Inseln) — Karte F1b.
//!
//! Geprüft wird die ganze Kette: Umgrenzung aus dem Farbeimer-Kern
//! (`boundary::detect_boundary`) → Füllfläche über die Befehls-Schicht →
//! Speichern/Laden → Anzeige in Displayliste, PDF und PNG. Der Nachweis ist
//! ein Pixelmaß: im Loch muss der Untergrund zu sehen sein, nicht das Muster.

use jjcad_core::area;
use jjcad_core::boundary::{detect_boundary, BoundaryOpts};
use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{polygon_area, Vec2};
use jjcad_core::model::*;
use jjcad_core::persist;
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::{Color, FillPattern};
use serde_json::json;

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

/// Neue Zeichnung mit Millimetern als Eingabeeinheit — die Umgrenzung aus
/// `detect_boundary` ist in mm, und die Zahlen unten sind mm.
fn leinwand() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"})).unwrap();
    s
}

fn wohnhaus() -> Document {
    let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/Wohnhaus.jjcad");
    persist::load_document(&pfad).expect("Wohnhaus.jjcad")
}

fn flaechen(doc: &Document) -> Vec<(Vec<Vec2>, Vec<Vec<Vec2>>)> {
    doc.entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Fill {
                boundary, holes, ..
            } => Some((boundary.clone(), holes.clone())),
            _ => None,
        })
        .collect()
}

fn ring_mm(r: &[Vec2]) -> Vec<[f64; 2]> {
    r.iter().map(|p| [p.x, p.y]).collect()
}

/// Füllfläche mit einem rechteckigen Loch (1000/1000 bis 2000/2000).
fn flaeche_mit_loch(s: &mut Session, pattern: &str, winkel: f64, farbe: &str) -> u64 {
    let r = s
        .execute(
            "draw.fill.boundary",
            &json!({
                "boundary": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]],
                "holes": [[[1000, 1000], [2000, 1000], [2000, 2000], [1000, 2000]]],
                "pattern": pattern,
                "angle": winkel,
                "scale": 250.0,
                "border": false,
                "fill": farbe,
            }),
        )
        .expect("Füllfläche mit Loch");
    r["created"][0].as_u64().expect("neue ID")
}

// --- 1. Aus der Umgrenzung des Farbeimers entsteht eine Fläche mit Inseln ---

/// Der Raum unten links im Wohnhaus hat Inseln (Schornstein, Treppe). Sie
/// müssen im Objekt landen, sonst übermalt der Farbeimer sie.
#[test]
fn wohnhaus_raum_behaelt_die_inseln() {
    let doc = wohnhaus();
    let klick = v(1500.0, 2600.0);
    let grenze = detect_boundary(&doc, klick, &BoundaryOpts::default()).expect("Raum");
    assert!(
        !grenze.holes.is_empty(),
        "Testannahme verletzt: der Raum hat keine Inseln"
    );

    let mut s = leinwand();
    s.doc = doc;
    s.execute("doc.settings", &json!({"input_unit": "mm"})).unwrap();
    let holes: Vec<Vec<[f64; 2]>> = grenze.holes.iter().map(|h| ring_mm(h)).collect();
    let r = s
        .execute(
            "draw.fill.boundary",
            &json!({
                "boundary": ring_mm(&grenze.outer),
                "holes": holes,
                "pattern": "solid",
            }),
        )
        .expect("Farbeimer-Fläche");
    let id = r["created"][0].as_u64().unwrap();

    let (boundary, ist) = match &s.doc.entity(id).expect("Füllfläche").kind {
        EntityKind::Fill {
            boundary, holes, ..
        } => (boundary.clone(), holes.clone()),
        _ => panic!("kein Fill"),
    };
    assert_eq!(boundary.len(), grenze.outer.len());
    assert_eq!(ist.len(), grenze.holes.len(), "Inseln verloren");
    for (a, b) in ist.iter().zip(grenze.holes.iter()) {
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(b.iter()) {
            assert!(p.dist(*q) < 1e-9, "Ringpunkt verschoben: {p:?} statt {q:?}");
        }
    }
    // Die Inselfläche fehlt in der Gesamtfläche.
    let summe = area::area_of(&boundary, &ist);
    assert!(
        (summe - grenze.area()).abs() < 1.0,
        "Fläche {summe} statt {}",
        grenze.area()
    );
}

/// Speichern und Laden bringt Außenring und Inseln unverändert zurück.
#[test]
fn inseln_ueberleben_speichern_und_laden() {
    let mut s = leinwand();
    let id = flaeche_mit_loch(&mut s, "lines", 45.0, "#c83232");
    let vorher = flaechen(&s.doc);

    let json = persist::to_json(&s.doc).expect("JSON");
    assert!(json.contains("holes"), "Löcher fehlen in der Datei");
    let geladen = persist::from_json(&json).expect("laden");
    let nachher = flaechen(&geladen);
    assert_eq!(nachher.len(), 1);
    assert_eq!(nachher[0].1.len(), 1, "Loch verloren");
    assert_eq!(nachher[0].0, vorher[0].0);
    assert_eq!(nachher[0].1, vorher[0].1);
    assert!(geladen.entity(id).is_some());
}

/// Alte Datei ohne `holes`-Schlüssel lädt weiter und zeichnet eine Fläche.
#[test]
fn alte_datei_ohne_holes_laedt() {
    let alt = r##"{
      "format": "jjcad",
      "version": 2,
      "name": "Alt",
      "entities": [
        {
          "id": 1,
          "kind": {
            "type": "fill",
            "boundary": [[0, 0], [2000, 0], [2000, 1000], [0, 1000]],
            "pattern": "Solid",
            "border": true
          },
          "layer": 1,
          "style": { "color": "#202020", "fill": "#88bbdd" }
        }
      ]
    }"##;
    let doc = persist::from_json(alt).expect("alte Datei lädt");
    let f = flaechen(&doc);
    assert_eq!(f.len(), 1);
    assert!(f[0].1.is_empty(), "Löcher aus dem Nichts");

    let dl = render::render_document(&RenderCtx::new(&doc));
    let fills: Vec<_> = dl
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Fill { .. }))
        .collect();
    assert_eq!(fills.len(), 1, "Fläche wird nicht gezeichnet");
    if let Shape::Fill { holes, .. } = &fills[0].shape {
        assert!(holes.is_empty());
    }
}

/// Auch die vorhandene Beispieldatei lädt und hat dort, wo nichts ausgespart
/// ist, keine Löcher.
#[test]
fn beispieldatei_laedt_mit_leeren_lochlisten() {
    let doc = wohnhaus();
    for (_, holes) in flaechen(&doc) {
        assert!(holes.is_empty());
    }
}

// --- 2. Anzeige: das Loch bleibt frei -------------------------------------

/// Vergleichsbild: ein blaues Feld liegt unter der roten Fläche mit Loch.
fn pixmap(s: &Session) -> tiny_skia::Pixmap {
    let opts = RasterOptions {
        width_px: 400,
        height_px: 300,
        bbox: Some([0.0, 0.0, 4000.0, 3000.0]),
        paper: false,
        ..Default::default()
    };
    render_model_pixmap(&s.doc, &opts).expect("PNG rendern")
}

/// Pixel an Weltkoordinate (mm) — 400×300 px auf 4000×3000 mm, 10 mm je Pixel.
fn px(pm: &tiny_skia::Pixmap, x_mm: f64, y_mm: f64) -> (u8, u8, u8, u8) {
    // Bild-Y läuft von oben nach unten, Welt-Y von unten nach oben.
    let ix = (x_mm / 10.0) as u32;
    let iy = 299 - (y_mm / 10.0) as u32;
    let c = pm.pixel(ix, iy).expect("Pixel").demultiply();
    (c.red(), c.green(), c.blue(), c.alpha())
}

fn farbe(x: &str) -> Color {
    Color::from_hex(x).expect("Farbe")
}

/// Vollflächen-Füllung: im Loch ist der Untergrund zu sehen (hier das blaue
/// Feld darunter), in der Fläche die Füllfarbe.
#[test]
fn vollflaeche_laesst_das_loch_frei() {
    let mut s = leinwand();
    // Untergrund: blaues Feld über die ganze Zeichnung.
    s.execute(
        "draw.fill.boundary",
        &json!({
            "boundary": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]],
            "pattern": "solid",
            "border": false,
            "fill": "#2255cc",
        }),
    )
    .expect("Untergrund");
    flaeche_mit_loch(&mut s, "solid", 0.0, "#dd2222");

    let pm = pixmap(&s);
    let blau = farbe("#2255cc");
    let rot = farbe("#dd2222");

    let aussen = px(&pm, 500.0, 500.0);
    let innen = px(&pm, 1500.0, 1500.0);
    assert_eq!(
        (aussen.0, aussen.1, aussen.2),
        (rot.r, rot.g, rot.b),
        "Fläche nicht in der Füllfarbe: {aussen:?}"
    );
    assert_eq!(
        (innen.0, innen.1, innen.2),
        (blau.r, blau.g, blau.b),
        "Im Loch steht nicht der Untergrund: {innen:?}"
    );

    // Gezählt: kein einziges rotes Pixel im Loch (Rand ausgespart).
    let mut rot_im_loch = 0;
    for x in 1100..1900 {
        for y in 1100..1900 {
            let p = px(&pm, x as f64, y as f64);
            if p.0 > 200 && p.1 < 90 && p.2 < 90 {
                rot_im_loch += 1;
            }
        }
    }
    assert_eq!(rot_im_loch, 0, "Füllfarbe im Loch");
}

/// Schraffur: das Muster läuft nicht durch die Insel.
#[test]
fn muster_laeuft_nicht_durchs_loch() {
    let mut s = leinwand();
    flaeche_mit_loch(&mut s, "lines", 45.0, "#202020");
    let pm = pixmap(&s);

    let dunkel = |p: (u8, u8, u8, u8)| p.3 > 0 && p.0 < 160 && p.1 < 160 && p.2 < 160;
    let mut muster_im_loch = 0;
    for x in 1100..1900 {
        for y in 1100..1900 {
            if dunkel(px(&pm, x as f64, y as f64)) {
                muster_im_loch += 1;
            }
        }
    }
    assert_eq!(muster_im_loch, 0, "Muster im Loch");

    // Gegenprobe: neben dem Loch ist das Muster da.
    let mut muster_aussen = 0;
    for x in 500..900 {
        for y in 500..900 {
            if dunkel(px(&pm, x as f64, y as f64)) {
                muster_aussen += 1;
            }
        }
    }
    assert!(
        muster_aussen > 200,
        "kein Muster in der Fläche ({muster_aussen} Pixel)"
    );
}

/// Ohne Untergrund zeigt das Loch den Zeichnungshintergrund — die Fläche hat
/// dort nichts gemalt (keine weiße Ersatzfläche, kein Füllmuster).
#[test]
fn loch_zeigt_den_hintergrund_ohne_untergrund() {
    let mut s = leinwand();
    flaeche_mit_loch(&mut s, "solid", 0.0, "#dd2222");
    let pm = pixmap(&s);
    let innen = px(&pm, 1500.0, 1500.0);
    assert_eq!(
        (innen.0, innen.1, innen.2),
        (255, 255, 255),
        "Loch übermalt: {innen:?}"
    );
}

/// PDF: Flächen mit Löchern werden nach Even-Odd gefüllt (`f*`), nicht mit
/// einfachem `f` — sonst hinge die Aussparung am Umlaufsinn der Ringe.
#[test]
fn pdf_fuellt_mit_even_odd() {
    let mut s = leinwand();
    flaeche_mit_loch(&mut s, "solid", 0.0, "#dd2222");
    let bytes = jjcad_core::pdf::pdf_bytes(&s.doc, None).expect("PDF");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("f*"), "kein Even-Odd-Fülloperator im PDF");
}

// --- 3. Ändern trägt die Inseln mit ---------------------------------------

/// Verschieben wirkt auf Außenring und Insel gleich — der Abstand bleibt.
#[test]
fn verschieben_traegt_die_inseln_mit() {
    let mut s = leinwand();
    let id = flaeche_mit_loch(&mut s, "solid", 0.0, "#dd2222");
    s.execute(
        "modify.move",
        &json!({"ids": [id], "from": [0, 0], "to": [1000, 500]}),
    )
    .expect("verschieben");
    let (_, holes) = &flaechen(&s.doc)[0];
    assert_eq!(holes.len(), 1);
    let ecke = holes[0]
        .iter()
        .fold((f64::INFINITY, f64::INFINITY), |a, p| {
            (a.0.min(p.x), a.1.min(p.y))
        });
    assert!((ecke.0 - 2000.0).abs() < 1e-6, "x {ecke:?}");
    assert!((ecke.1 - 1500.0).abs() < 1e-6, "y {ecke:?}");
}

/// Drehen um 90° dreht die Insel mit und lässt ihre Fläche unverändert.
#[test]
fn drehen_traegt_die_inseln_mit() {
    let mut s = leinwand();
    let id = flaeche_mit_loch(&mut s, "solid", 0.0, "#dd2222");
    let vorher = polygon_area(&flaechen(&s.doc)[0].1[0]).abs();
    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0, 0], "angle": 90}),
    )
    .expect("drehen");
    let (boundary, holes) = &flaechen(&s.doc)[0];
    assert_eq!(holes.len(), 1);
    let nachher = polygon_area(&holes[0]).abs();
    assert!(
        (nachher - vorher).abs() < 1.0,
        "Inselfläche {nachher} statt {vorher}"
    );
    // Aus (1000…2000)² wird (-2000…-1000)×(1000…2000).
    let xs = holes[0].iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let ys = holes[0].iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    assert!((xs + 2000.0).abs() < 1e-6, "x min {xs}");
    assert!((ys - 2000.0).abs() < 1e-6, "y max {ys}");
    assert_eq!(boundary.len(), 4);
}

/// Ein Loch in einer anderen Einheit darf nicht in Millimetern landen:
/// `input_unit: m` gilt für Außenring und Insel gleich.
#[test]
fn inseln_folgen_der_eingabeeinheit() {
    let mut s = leinwand();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s.execute(
        "draw.fill.boundary",
        &json!({
            "boundary": [[0, 0], [4, 0], [4, 3], [0, 3]],
            "holes": [[[1, 1], [2, 1], [2, 2], [1, 2]]],
        }),
    )
    .expect("Fläche in Metern");
    let (boundary, holes) = &flaechen(&s.doc)[0];
    assert!((polygon_area(boundary).abs() - 12e6).abs() < 1.0);
    assert!((polygon_area(&holes[0]).abs() - 1e6).abs() < 1.0);
}

/// Der Randlinien-Schalter umrandet auch die Insel — sonst wirkt sie wie ein
/// Loch im Papier.
#[test]
fn randlinie_umrandet_die_insel() {
    let mut s = leinwand();
    s.execute(
        "draw.fill.boundary",
        &json!({
            "boundary": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]],
            "holes": [[[1000, 1000], [2000, 1000], [2000, 2000], [1000, 2000]]],
            "pattern": "solid",
            "border": true,
        }),
    )
    .expect("Fläche");
    let dl = render::render_document(&RenderCtx::new(&s.doc));
    let raender: Vec<&Vec<Vec2>> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Poly(pts, true) => Some(pts),
            _ => None,
        })
        .collect();
    assert_eq!(raender.len(), 2, "Außenring und Inselkante erwartet");
    let hat_insel = raender.iter().any(|r| {
        r.len() == 4
            && r.iter().any(|p| p.dist(v(1000.0, 1000.0)) < 1e-6)
            && r.iter().any(|p| p.dist(v(2000.0, 2000.0)) < 1e-6)
    });
    assert!(hat_insel, "Inselkante nicht umrandet");
}

/// Displayliste: ein Flächenprim mit Löchern, damit Bildschirm, PDF und PNG
/// aus derselben Quelle zeichnen.
#[test]
fn displayliste_traegt_die_ringe() {
    let mut s = leinwand();
    flaeche_mit_loch(&mut s, "solid", 0.0, "#dd2222");
    let dl = render::render_document(&RenderCtx::new(&s.doc));
    let fills: Vec<_> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Fill { outer, holes } => Some((outer.len(), holes.len())),
            _ => None,
        })
        .collect();
    assert_eq!(fills, vec![(4, 1)]);
}

/// Das Muster-Merkmal: `FillPattern` bleibt unabhängig von den Löchern.
#[test]
fn muster_bleibt_erhalten() {
    let mut s = leinwand();
    let id = flaeche_mit_loch(&mut s, "cross", 30.0, "#202020");
    let e = s.doc.entity(id).unwrap();
    match &e.kind {
        EntityKind::Fill {
            pattern,
            pattern_angle,
            holes,
            ..
        } => {
            assert_eq!(*pattern, FillPattern::Cross);
            assert!((pattern_angle.to_degrees() - 30.0).abs() < 1e-9);
            assert_eq!(holes.len(), 1);
        }
        _ => panic!("kein Fill"),
    }
}
