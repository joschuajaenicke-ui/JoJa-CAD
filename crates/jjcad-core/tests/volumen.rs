//! Abnahme des Elements **Volumen**: ein Körper mit Höhe, der sich optisch wie
//! eine Füllfläche verhält, aber **an die Wände anschließt**.
//!
//! Geprüft wird die gezeichnete Geometrie (Displayliste + Pixelbild):
//!
//! 1. **Anschluss ohne Trennlinie** — die Wandfläche und die Volumenfläche
//!    gehen ineinander über: keine Linie an der Naht, in keiner Richtung.
//!    Weder die Stirnkante der Wand im Volumen noch die Volumenkante in der
//!    Wand darf stehen bleiben.
//! 2. **Gleicher Ton** — das Volumen wird im Wandton gefüllt, sonst bliebe ein
//!    Farbwechsel als Kante sichtbar.
//! 3. **Körper** — Höhe und Volumen (m³) sind Metadaten; die Wand rechnet ihr
//!    Volumen aus Länge × Dicke × Höhe.

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{dist_point_segment, point_in_polygon, Vec2};
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::wall::FILL_BLEED;
use jjcad_core::Session;
use serde_json::json;

// ---------------------------------------------------------------------------
// Hilfen
// ---------------------------------------------------------------------------

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

struct Z {
    fills: Vec<Vec<Vec2>>,
    lines: Vec<(Vec2, Vec2)>,
    pixels: Option<(tiny_skia::Pixmap, [f64; 4])>,
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
    fn grau(&self, p: Vec2) -> Option<u8> {
        let (pm, bb) = self.pixels.as_ref()?;
        let (w, h) = (pm.width() as f64, pm.height() as f64);
        // `render_model_pixmap` erweitert den Ausschnitt um 3 % der größeren
        // Kante — dieselbe Abbildung muss hier gelten.
        let pad = (bb[2] - bb[0]).max(bb[3] - bb[1]) * 0.03;
        let bb = [bb[0] - pad, bb[1] - pad, bb[2] + pad, bb[3] + pad];
        let bw = bb[2] - bb[0];
        let bh = bb[3] - bb[1];
        let k = (w / bw).min(h / bh);
        let ox = -bb[0] * k + (w - bw * k) * 0.5;
        let oy = bb[3] * k + (h - bh * k) * 0.5;
        let x = (ox + p.x * k).round() as i64;
        let y = (oy - p.y * k).round() as i64;
        if x < 0 || y < 0 || x >= pm.width() as i64 || y >= pm.height() as i64 {
            return None;
        }
        let i = (y as usize * pm.width() as usize + x as usize) * 4;
        let d = pm.data();
        Some(((d[i] as u32 + d[i + 1] as u32 + d[i + 2] as u32) / 3) as u8)
    }
}

fn messen(s: &Session, bb: [f64; 4]) -> Z {
    let ctx = RenderCtx::model(&s.doc);
    let dl = render::render_document(&ctx);
    let mut fills = Vec::new();
    let mut lines = Vec::new();
    for p in &dl.prims {
        match &p.shape {
            Shape::Fill { outer, .. } => fills.push(outer.clone()),
            Shape::Line(a, b) => lines.push((*a, *b)),
            _ => {}
        }
    }
    let opts = RasterOptions {
        width_px: 900,
        height_px: 900,
        bbox: Some(bb),
        paper: false,
        annot_scale: Some(1.0),
        sheet: None,
    };
    let pixels = render_model_pixmap(&s.doc, &opts).ok().map(|pm| (pm, bb));
    Z {
        fills,
        lines,
        pixels,
    }
}

/// Wand waagerecht von (0,0) nach (2000,0), 240 mm dick, mittig.
fn wand(s: &mut Session) {
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0], "b": [2.0, 0.0], "thickness": 0.24}),
    )
    .unwrap();
}

/// Volumen, das 200 mm über das Wandende greift (Rechteck 2000 ± 1000 breit).
fn volumen_ueberlappend(s: &mut Session) -> u64 {
    let v = s
        .execute(
            "draw.volume",
            &json!({
                "boundary": [[1.8, -0.3], [3.0, -0.3], [3.0, 0.3], [1.8, 0.3]],
                "height": 2.75,
                "border": true,
                "pattern": "solid",
            }),
        )
        .unwrap();
    v["created"][0].as_u64().unwrap()
}

// ---------------------------------------------------------------------------
// 1. Anschluss ohne Trennlinie
// ---------------------------------------------------------------------------

#[test]
fn anschlusspunkte_ohne_linie() {
    let mut s = session();
    wand(&mut s);
    volumen_ueberlappend(&mut s);
    let z = messen(&s, [-400.0, -900.0, 3400.0, 900.0]);

    // Naht: das Wandende liegt bei x = 2000 mitten im Volumen.
    // Beide Kanten, die dort verlaufen, dürfen keine Linie sein:
    //  - die Stirnkante der Wand (x = 2000, y = ±120) → liegt im Volumen
    //  - die linke Kante des Volumens (x = 1800, y = ±300) → liegt in der Wand
    for (p, name) in [
        (Vec2::new(2000.0, 0.0), "Wandstirn im Volumen"),
        (Vec2::new(1800.0, 0.0), "Volumenkante in der Wand"),
    ] {
        assert!(
            !z.linie_nahe(p, 2.0 + FILL_BLEED),
            "{name}: Linie bei {p:?} — der Anschluss ist sichtbar"
        );
    }

    // Die sichtbaren Kanten des Übergangs (Wandlängskanten außerhalb, die
    // Volumenkanten außerhalb der Wand) müssen dagegen da sein.
    for (p, name) in [
        (Vec2::new(1500.0, 120.0), "Wandoberkante"),
        (Vec2::new(1500.0, -120.0), "Wandunterkante"),
        (Vec2::new(2600.0, 300.0), "Volumenoberkante"),
    ] {
        assert!(z.linie_nahe(p, 2.0), "{name}: Linie fehlt bei {p:?}");
    }

    // Kein heller Spalt im Übergang: links und rechts der Naht dieselbe
    // Füllung (kein weißer Streifen, kein Tonwechsel).
    let links = z.grau(Vec2::new(1950.0, 0.0)).expect("Pixel links");
    let rechts = z.grau(Vec2::new(2050.0, 0.0)).expect("Pixel rechts");
    assert!(
        (links as i32 - rechts as i32).abs() <= 2,
        "Tonwechsel an der Naht: {links} links, {rechts} rechts"
    );
    assert!(links < 250, "Naht ist weiß ({links}) — Lücke");
}

#[test]
fn volumen_hat_wandton() {
    let mut s = session();
    wand(&mut s);
    volumen_ueberlappend(&mut s);
    let z = messen(&s, [-400.0, -900.0, 3400.0, 900.0]);
    let w = z.grau(Vec2::new(500.0, 0.0)).expect("Wand");
    let v = z.grau(Vec2::new(2600.0, 0.0)).expect("Volumen");
    assert!(
        (w as i32 - v as i32).abs() <= 2,
        "Volumen {v} und Wand {w} haben verschiedene Töne"
    );
}

/// Eine Wand, die *in* einem Volumen endet (Wand kürzer als das Volumen):
/// ihre Stirnkante und ihre Längskanten im Volumen fallen aus.
#[test]
fn wandende_im_volumen() {
    let mut s = session();
    // Volumen zuerst, damit es hinter der Wand liegt (Zeichenreihenfolge).
    s.execute(
        "draw.volume",
        &json!({
            "boundary": [[1.2, -0.8], [3.2, -0.8], [3.2, 0.8], [1.2, 0.8]],
            "height": 2.75,
            "border": true,
            "pattern": "solid",
        }),
    )
    .unwrap();
    // Wand ragt 500 mm in das Volumen
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0], "b": [2.2, 0.0], "thickness": 0.24}),
    )
    .unwrap();
    let z = messen(&s, [-400.0, -1400.0, 3800.0, 1400.0]);
    for (p, name) in [
        (Vec2::new(2200.0, 0.0), "Wandstirn im Volumen"),
        (Vec2::new(1800.0, 120.0), "Wandoberkante im Volumen"),
    ] {
        assert!(
            !z.linie_nahe(p, 2.0 + FILL_BLEED),
            "{name}: Linie bei {p:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Körper: Höhe und Volumen
// ---------------------------------------------------------------------------

#[test]
fn volumen_ist_ein_koerper() {
    let mut s = session();
    let id = s
        .execute(
            "draw.volume",
            &json!({
                "boundary": [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]],
                "height": 2.75,
            }),
        )
        .unwrap()["created"][0]
        .as_u64()
        .unwrap();

    let e = s.doc.entity(id).unwrap();
    assert_eq!(e.kind.type_name(), "Volumen");
    assert_eq!(e.body_height(), Some(2750.0));
    // 2,0 m² × 2,75 m = 5,5 m³
    let v = e.volume_m3().expect("Volumen fehlt");
    assert!((v - 5.5).abs() < 1e-6, "Volumen ist {v} m³");

    // Höhe ändern → Volumen folgt
    s.execute("modify.set", &json!({"ids": [id], "height": 3.0}))
        .unwrap();
    let v = s.doc.entity(id).unwrap().volume_m3().unwrap();
    assert!((v - 6.0).abs() < 1e-6, "nach Höhenänderung {v} m³");
}

#[test]
fn wand_volumen_ist_laenge_mal_dicke_mal_hoehe() {
    let mut s = session();
    wand(&mut s); // 2,0 m lang, 0,24 m dick, 2,75 m hoch
    let v = s.doc.entities[0].volume_m3().expect("Wand ohne Volumen");
    assert!((v - 2.0 * 0.24 * 2.75).abs() < 1e-6, "Wandvolumen {v} m³");
}

// ---------------------------------------------------------------------------
// 3. Bleibt beim Speichern erhalten
// ---------------------------------------------------------------------------

#[test]
fn volumen_ueberlebt_speichern() {
    let mut s = session();
    let id = volumen_ueberlappend(&mut s);
    let json_text = jjcad_core::persist::to_json(&s.doc).expect("Speichern");
    let doc2 = jjcad_core::persist::from_json(&json_text).expect("Laden");
    let e = doc2.entity(id).expect("Volumen fehlt nach dem Laden");
    assert_eq!(e.kind.type_name(), "Volumen");
    let v = e.volume_m3().unwrap();
    assert!((v - 1.2 * 0.6 * 2.75).abs() < 1e-6, "Volumen nach Laden {v} m³");
}
