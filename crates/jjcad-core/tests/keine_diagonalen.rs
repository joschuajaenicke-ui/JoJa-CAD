//! Keine Diagonalen in der Displayliste.
//!
//! Symptom (b): Im Grundriss zogen sich lange diagonale Striche quer durch den
//! Plan — ein Gitternetz aus Winkelhalbierenden, in der GUI sichtbar, in PDF/PNG
//! nicht. Ursache war **nicht** eine gezeichnete Konstruktionslinie: Die
//! Wandfüllung enthält ein 0,4 mm breites Füllband auf der Gehrungsnaht
//! (Haarnnaht gegen Antialiasing-Streifen). Bei kleiner Anzeige ist dieses Band
//! schmaler als ein Pixel; egui tesselliert ein so entartetes Polygon zu
//! Dreiecken mit unbrauchbaren Ecken und zieht daraus Striche über die ganze
//! Fläche.
//!
//! Diese Tests halten beide Seiten fest:
//! 1. Die Displayliste selbst enthält keine unerwartete Diagonale (Kanten einer
//!    Wand-Ecke sind achsparallel und nicht länger als die Wand).
//! 2. Für den Beispielplan gibt es keine Linie, die länger als das Haus ist.
//! 3. Das Haarnnaht-Band ist wirklich unterpixelbreit — also muss die GUI es
//!    ausblenden (die Grenze dafür prüft `fill_too_thin` in `canvas.rs`).

use jjcad_core::geom::{polygon_min_width, Vec2};
use jjcad_core::persist;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::Session;
use serde_json::json;
use std::path::PathBuf;

/// Anzeige des Beispielplans auf dem Bildschirm: ganze Zeichnung im Fenster
/// ergibt etwa 53 px je Meter (gemessen im Fenster bei 1:100).
const PX_PER_MM: f64 = 0.053;

fn ecke_90() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [-2.0, 0.0], "b": [0.0, 0.0], "thickness": 0.3}),
    )
    .unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0], "b": [0.0, 2.0], "thickness": 0.3}),
    )
    .unwrap();
    s
}

fn linien(s: &Session) -> Vec<(Vec2, Vec2)> {
    let ctx = RenderCtx::with_scale(&s.doc, 1.0);
    let dl = render_document(&ctx);
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Line(a, b) => Some((*a, *b)),
            _ => None,
        })
        .collect()
}

fn fuellungen(s: &Session) -> Vec<Vec<Vec2>> {
    let ctx = RenderCtx::with_scale(&s.doc, 1.0);
    let dl = render_document(&ctx);
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Fill { outer, .. } => Some(outer.clone()),
            _ => None,
        })
        .collect()
}

/// Kanten einer rechten Wand-Ecke: nur Längs- und Stirnkanten, keine Diagonale.
#[test]
fn ecke_zeigt_nur_erwartete_kanten() {
    let s = ecke_90();
    let ls = linien(&s);
    assert!(!ls.is_empty(), "keine Linien gezeichnet");

    let mut diagonale = Vec::new();
    let mut zu_lang = Vec::new();
    for (a, b) in &ls {
        let d = *b - *a;
        // Achsparallel: eine Kante der Ecke ist waagerecht oder senkrecht.
        if d.x.abs() > 1e-6 && d.y.abs() > 1e-6 {
            diagonale.push((*a, *b));
        }
        // Längste erwartete Kante: Wand 2000 mm + Gehrungszugabe 150 mm.
        if a.dist(*b) > 2400.0 {
            zu_lang.push((*a, *b));
        }
    }
    assert!(
        diagonale.is_empty(),
        "Diagonale in der Displayliste: {:?}",
        &diagonale[..diagonale.len().min(3)]
    );
    assert!(
        zu_lang.is_empty(),
        "Linie länger als die Wand: {:?}",
        &zu_lang[..zu_lang.len().min(3)]
    );
}

/// Der Beispielplan: keine Linie, die länger als das Haus ist (10,5 m).
#[test]
fn beispielplan_ohne_ueberlange_linie() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/Wohnhaus.jjcad");
    let doc = match persist::load_document(&path) {
        Ok(d) => d,
        Err(e) => panic!("Beispielplan {} nicht lesbar: {e}", path.display()),
    };
    let ctx = RenderCtx::with_scale(&doc, 1.0);
    let dl = render_document(&ctx);
    let mut lang = Vec::new();
    let mut quer = Vec::new();
    for p in &dl.prims {
        match &p.shape {
            Shape::Line(a, b) => {
                let len = a.dist(*b);
                if len > 12000.0 {
                    lang.push((a.x, a.y, b.x, b.y, len));
                }
                // Schräge Linien über 3 m gibt es im Plan nicht (nur Maßpfeile,
                // die als Poly gezeichnet werden).
                let d = *b - *a;
                if len > 3000.0 && d.x.abs() > 1e-6 && d.y.abs() > 1e-6 {
                    quer.push((a.x, a.y, b.x, b.y, len));
                }
            }
            Shape::Poly(pts, _) => {
                for w in pts.windows(2) {
                    if w[0].dist(w[1]) > 12000.0 {
                        lang.push((w[0].x, w[0].y, w[1].x, w[1].y, w[0].dist(w[1])));
                    }
                }
            }
            _ => {}
        }
    }
    assert!(lang.is_empty(), "Linie länger als 12 m: {:?}", &lang[..lang.len().min(3)]);
    assert!(quer.is_empty(), "schräge Linie über 3 m: {:?}", &quer[..quer.len().min(3)]);
}

/// Das Haarnnaht-Band ist bei kleiner Anzeige schmaler als ein Pixel.
///
/// Damit ist belegt, warum die GUI Füllungen unter 0,75 px Breite verwerfen
/// muss — und dass die Wandfläche selbst (300 mm) weit darüber liegt.
#[test]
fn haarnnaht_unterpixelbreit() {
    let s = ecke_90();
    let fills = fuellungen(&s);
    assert!(!fills.is_empty(), "keine Wandfüllungen");

    let breiten: Vec<f64> = fills
        .iter()
        .map(|f| polygon_min_width(f) * PX_PER_MM)
        .collect();
    let schmal = breiten.iter().cloned().fold(f64::INFINITY, f64::min);
    let breit = breiten.iter().cloned().fold(0.0f64, f64::max);
    // Haarnnaht: 2 × 0,2 mm = 0,4 mm -> ~0,02 px
    assert!(
        schmal < 0.75,
        "schmalste Füllung ist {schmal:.3} px breit — Haarnnaht fehlt?"
    );
    // Wandkörper: 300 mm -> ~16 px
    assert!(
        breit > 5.0,
        "breiteste Füllung ist nur {breit:.3} px — die Wandfläche wird verworfen"
    );
}
