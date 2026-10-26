//! Baseline-Messung der Bogenauflösung.
//!
//! Kein Abnahmetest: dieses Programm misst den Polygonzug, den
//! `geom::Arc::flatten` liefert, und schreibt die Werte auf stdout. Damit
//! lässt sich die Kantigkeit von Bögen vor und nach einer Änderung
//! objektiv vergleichen (Sollwerte: `docs/BOGEN_BASELINE.md`).
//!
//! Aufruf:
//!
//! ```text
//! cargo test -p jjcad-core --test bogen_baseline -- --nocapture
//! ```
//!
//! Die Toleranzen stammen aus den Ausgabewegen (nicht neu erfunden):
//!
//! | Weg | Toleranz | Quelle |
//! |---|---|---|
//! | Zeichenfläche | 0,6 px | `crates/jjcad-gui/src/canvas.rs`, `flatten(0.6 / scale)` |
//! | PNG/Raster | 0,1 px | `crates/jjcad-core/src/raster.rs`, `flatten(0.1 / map.k)` |
//! | PDF | 0,08 mm Papier | `crates/jjcad-core/src/pdf.rs`, `flatten(0.08 / paper_scale)` |
//!
//! Gemessen wird in Bildschirmpixeln (px) bzw. Grad; das ist die Einheit, in
//! der man Kantigkeit sieht, und sie hängt nicht von der Weltgröße ab.

use jjcad_core::geom::{Arc, Vec2};
use jjcad_core::persist;
use jjcad_core::render::render_document;
use jjcad_core::render::{RenderCtx, Shape};

/// Toleranz der Zeichenfläche in Pixeln (siehe `canvas.rs`).
const TOL_SCREEN_PX: f64 = 0.6;
/// Toleranz des Raster/PNG-Wegs in Pixeln (siehe `raster.rs`).
const TOL_RASTER_PX: f64 = 0.1;

/// Ein Messfall: Bogen mit Radius und Öffnungswinkel, dargestellt bei einem Zoom.
struct Fall {
    name: &'static str,
    r_mm: f64,
    sweep_deg: f64,
    /// Pixeln je Millimeter (100 % in der Statusleiste = 1,0).
    zoom: f64,
}

impl Fall {
    fn arc(&self) -> Arc {
        Arc::new(
            Vec2::ZERO,
            self.r_mm,
            0.0,
            self.sweep_deg.to_radians(),
        )
    }

    fn r_px(&self) -> f64 {
        self.r_mm * self.zoom
    }
}

/// Kennzahlen eines Polygonzugs gegenüber dem idealen Bogen.
struct Messung {
    segmente: usize,
    /// Größter Abstand zwischen idealem Bogen und Polygonzug, in Pixeln.
    sagitta_px: f64,
    /// Größter Knick zwischen zwei benachbarten Segmenten, in Grad.
    knick_deg: f64,
    /// Wurde die harte Obergrenze von 512 Segmenten erreicht?
    geklemmt: bool,
}

fn messen(arc: &Arc, tol_px: f64, zoom: f64) -> Messung {
    // Toleranz von Pixeln in Weltmillimeter umrechnen — genau wie die
    // Aufrufer es tun.
    let tol_world = tol_px / zoom.max(1e-9);
    let pts = arc.flatten(tol_world);
    let n = pts.len().saturating_sub(1);
    let sweep = arc.sweep();

    let mut sagitta_px: f64 = 0.0;
    for i in 0..n {
        let a = arc.a0 + sweep * ((i as f64) + 0.5) / (n as f64);
        let ideal = arc.c + Vec2::polar(a, arc.r);
        sagitta_px = sagitta_px.max(abstand_punkt_strecke(ideal, pts[i], pts[i + 1]) * zoom);
    }

    let mut knick_deg: f64 = 0.0;
    for i in 0..n.saturating_sub(1) {
        let d0 = pts[i + 1] - pts[i];
        let d1 = pts[i + 2] - pts[i + 1];
        if d0.len() > 1e-9 && d1.len() > 1e-9 {
            let w = (d0.x * d1.y - d0.y * d1.x).atan2(d0.x * d1.x + d0.y * d1.y);
            knick_deg = knick_deg.max(w.abs().to_degrees());
        }
    }

    // 512 ist die Obergrenze in `Arc::flatten`; erreicht sie, wächst der
    // Fehler über die Toleranz hinaus.
    let max_step = if arc.r > 1e-6 {
        (2.0 * (1.0 - (tol_world / arc.r).clamp(-1.0, 1.0)).acos()).max(0.02)
    } else {
        0.3
    };
    let roh = (sweep / max_step).ceil();

    Messung {
        segmente: n,
        sagitta_px,
        knick_deg,
        geklemmt: roh > 512.0,
    }
}

fn abstand_punkt_strecke(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.len2();
    if l2 < 1e-12 {
        return p.dist(a);
    }
    let t = (((p - a).x * ab.x + (p - a).y * ab.y) / l2).clamp(0.0, 1.0);
    p.dist(a + ab * t)
}

fn faelle() -> Vec<Fall> {
    // Radien, Öffnungswinkel und Zoom entsprechen den Szenen der visuellen
    // Baseline (`samples/bogen_baseline/`, Ansicht aus `docs/BOGEN_BASELINE.md`).
    vec![
        // S1 — kleine Radien, Ansicht bei 100 % (1,0 px/mm)
        Fall { name: "S1   60 mm /  90° / 95 %", r_mm: 60.0, sweep_deg: 90.0, zoom: 0.953 },
        Fall { name: "S1  150 mm / 180° / 95 %", r_mm: 150.0, sweep_deg: 180.0, zoom: 0.953 },
        Fall { name: "S1  400 mm /  90° / 95 %", r_mm: 400.0, sweep_deg: 90.0, zoom: 0.953 },
        // S2 — Türbogen und Wandbogen, Ansicht bei 15 % (0,15 px/mm)
        Fall { name: "S2  885 mm /  90° / 14 %", r_mm: 885.0, sweep_deg: 90.0, zoom: 0.141 },
        Fall { name: "S2 2000 mm /  90° / 14 %", r_mm: 2000.0, sweep_deg: 90.0, zoom: 0.141 },
        Fall { name: "S2 3000 mm / 180° / 14 %", r_mm: 3000.0, sweep_deg: 180.0, zoom: 0.141 },
        // S3 — große Radien, Planansicht bei 0,8 % (0,008 px/mm)
        Fall { name: "S3  8000 mm /  90° / 0,75 %", r_mm: 8000.0, sweep_deg: 90.0, zoom: 0.0075 },
        Fall { name: "S3 25000 mm /  90° / 0,75 %", r_mm: 25000.0, sweep_deg: 90.0, zoom: 0.0075 },
        Fall { name: "S3 60000 mm /  20° / 0,75 %", r_mm: 60000.0, sweep_deg: 20.0, zoom: 0.0075 },
        // S4 — dicke Striche bei 38 % sowie Grenzfälle
        Fall { name: "S4  400 mm /  90° / 38 %", r_mm: 400.0, sweep_deg: 90.0, zoom: 0.3795 },
        Fall { name: "S4  885 mm /  90° / 38 %", r_mm: 885.0, sweep_deg: 90.0, zoom: 0.3795 },
        Fall { name: "G  Vollkreis 1000 mm / 10 %", r_mm: 1000.0, sweep_deg: 360.0, zoom: 0.1 },
        Fall { name: "G  1500 mm /  90° / 500 %", r_mm: 1500.0, sweep_deg: 90.0, zoom: 5.0 },
        Fall { name: "G  50 mm /  90° / 400 %", r_mm: 50.0, sweep_deg: 90.0, zoom: 4.0 },
    ]
}

#[test]
fn bogen_baseline_messwerte() {
    println!("\n### Bogenauflösung, Zeichenfläche (Toleranz {TOL_SCREEN_PX} px)\n");
    println!("| Fall | Bildschirmradius | Segmente | max. Bogenfehler | max. Knick | 512er-Grenze |");
    println!("|---|---:|---:|---:|---:|---|");
    for f in faelle() {
        let m = messen(&f.arc(), TOL_SCREEN_PX, f.zoom);
        println!(
            "| {} | {:.1} px | {} | {:.3} px | {:.2}° | {} |",
            f.name,
            f.r_px(),
            m.segmente,
            m.sagitta_px,
            m.knick_deg,
            if m.geklemmt { "erreicht" } else { "frei" }
        );
    }

    println!("\n### Derselbe Polygonzug, Raster/PNG (Toleranz {TOL_RASTER_PX} px)\n");
    println!("| Fall | Segmente | max. Bogenfehler | max. Knick |");
    println!("|---|---:|---:|---:|");
    for f in faelle() {
        let m = messen(&f.arc(), TOL_RASTER_PX, f.zoom);
        println!(
            "| {} | {} | {:.3} px | {:.2}° |",
            f.name, m.segmente, m.sagitta_px, m.knick_deg
        );
    }

    // Aufwand im echten Plan: Wie viele Bogenprimitive und Segmente entstehen
    // für das Wohnhaus bei Planansicht, und wie lange dauert das?
    println!("\n### Wohnhaus.jjcad bei 5 % (0,05 px/mm)\n");
    let doc = persist::load_document(std::path::Path::new("../../samples/Wohnhaus.jjcad"))
        .or_else(|_| persist::load_document(std::path::Path::new("samples/Wohnhaus.jjcad")))
        .expect("Wohnhaus.jjcad nicht gefunden");
    let zoom = 0.05;
    let ctx = RenderCtx::new(&doc);
    let t0 = std::time::Instant::now();
    let dl = render_document(&ctx);
    let t_build = t0.elapsed();
    let mut bogen = 0usize;
    let mut segmente = 0usize;
    let t1 = std::time::Instant::now();
    for p in &dl.prims {
        if let Shape::Arc { c, r, a0, a1 } = &p.shape {
            bogen += 1;
            segmente += Arc::new(*c, *r, *a0, *a1)
                .flatten(TOL_SCREEN_PX / zoom)
                .len()
                .saturating_sub(1);
        }
    }
    let t_flatten = t1.elapsed();
    println!("| Kennzahl | Wert |");
    println!("|---|---|");
    println!("| Primitive gesamt | {} |", dl.prims.len());
    println!("| davon Bögen | {} |", bogen);
    println!("| Segmente aller Bögen | {} |", segmente);
    println!("| Displayliste bauen | {:.3} ms |", t_build.as_secs_f64() * 1e3);
    println!("| Bögen tessellieren | {:.3} ms |", t_flatten.as_secs_f64() * 1e3);

    // Grundkontrolle: ohne Bogenfläche liefert die Messung nichts.
    assert!(bogen > 0, "keine Bögen im Plan gefunden");
    // Grundkontrolle: die Toleranz muss eingehalten werden, solange die
    // 512er-Klemme nicht greift (das ist die Definition von `flatten`).
    for f in faelle() {
        let m = messen(&f.arc(), TOL_SCREEN_PX, f.zoom);
        if !m.geklemmt {
            assert!(
                m.sagitta_px <= TOL_SCREEN_PX + ZUSCHLAG,
                "{}: Bogenfehler {:.3} px über Toleranz",
                f.name,
                m.sagitta_px
            );
        }
    }
}

/// Rechenungenauigkeit in Pixeln, die die Messung zulässt.
const ZUSCHLAG: f64 = 1e-6;

// ---------------------------------------------------------------------------
// Baseline-Zeichnungen erzeugen
// ---------------------------------------------------------------------------

/// Ein Bogen der Baseline-Zeichnung: Mitte, Radius, Öffnungswinkel, Stärke.
struct Bogen {
    cx: f64,
    cy: f64,
    r: f64,
    a0: f64,
    a1: f64,
    /// Linienstärke in mm (wie im Eigenschaften-Panel).
    staerke: f64,
}

const fn b(cx: f64, cy: f64, r: f64, a0: f64, a1: f64, staerke: f64) -> Bogen {
    Bogen { cx, cy, r, a0, a1, staerke }
}

/// Szenen der visuellen Baseline: Name, Maßstab der Zeichnung (steuert die
/// Strichbreite im Modus „DPI"), Bögen.
fn szenen() -> Vec<(&'static str, f64, Vec<Bogen>)> {
    vec![
        // S1 — kleine Radien (Detail), 1:10
        (
            "S1_klein",
            10.0,
            vec![
                b(-450.0, -150.0, 60.0, 0.0, 90.0, 0.35),
                b(-250.0, -150.0, 150.0, 0.0, 180.0, 0.50),
                b(100.0, -150.0, 400.0, 0.0, 90.0, 0.70),
            ],
        ),
        // S2 — Türbogen-Maß (885 mm) und Wandbögen, 1:50
        (
            "S2_mittel",
            50.0,
            vec![
                b(-1200.0, -600.0, 885.0, 0.0, 90.0, 0.35),
                b(0.0, -600.0, 2000.0, 0.0, 90.0, 0.50),
                b(2200.0, -600.0, 3000.0, 0.0, 180.0, 0.70),
            ],
        ),
        // S3 — große Radien (Planansicht), 1:100
        (
            "S3_gross",
            100.0,
            vec![
                b(-30000.0, 0.0, 8000.0, 0.0, 90.0, 0.50),
                b(0.0, 0.0, 25000.0, 0.0, 90.0, 0.70),
                b(30000.0, 0.0, 60000.0, 0.0, 20.0, 1.00),
            ],
        ),
        // S4 — dicke Striche (0,13 / 0,50 / 1,00 / 2,00 mm), 1:20
        (
            "S4_starken",
            20.0,
            vec![
                b(-600.0, -450.0, 400.0, 0.0, 90.0, 0.13),
                b(-100.0, -450.0, 400.0, 0.0, 90.0, 0.50),
                b(400.0, -450.0, 400.0, 0.0, 90.0, 1.00),
                b(900.0, -450.0, 885.0, 0.0, 90.0, 2.00),
            ],
        ),
    ]
}

/// Baut eine Zeichnung, die nur aus den angegebenen Bögen besteht.
fn szene_doc(bogen: &[Bogen], massstab: f64) -> jjcad_core::model::Document {
    use jjcad_core::model::{Document, Entity, EntityKind, EntityStyle};
    let mut doc = Document::new();
    doc.settings.annotation_scale = massstab;
    let layer = doc.current_layer;
    for s in bogen {
        let mut e = Entity::new(
            doc.new_id(),
            layer,
            EntityKind::Arc {
                c: Vec2::new(s.cx, s.cy),
                r: s.r,
                a0: s.a0.to_radians(),
                a1: s.a1.to_radians(),
            },
        );
        e.style = EntityStyle {
            line_weight: Some(s.staerke),
            ..Default::default()
        };
        doc.entities.push(e);
    }
    doc
}

/// Schreibt die Baseline-Zeichnungen nach `samples/bogen_baseline/` und gibt
/// je Szene die Ansicht aus, mit der die Szene die Zeichenfläche füllt
/// (Zoom in px/mm, Blickpunkt in mm).
///
/// ```text
/// cargo test -p jjcad-core --test bogen_baseline -- --ignored --nocapture
/// ```
#[test]
#[ignore = "schreibt samples/bogen_baseline/*.jjcad (Baseline-Artefakte)"]
fn baseline_szenen_schreiben() {
    // Zeichenfläche im Fenster 1500x950: rund 975x745 pt; `fit_view` zieht
    // 70 pt Rand ab. Dieselbe Formel, damit die Doku den echten Zoom nennt.
    const FLAECHE_PX: (f64, f64) = (905.0, 675.0);
    for (name, massstab, bogen) in szenen() {
        let mut doc = szene_doc(&bogen, massstab);
        let rel = format!("../../samples/bogen_baseline/{name}.jjcad");
        let pfad = std::path::Path::new(&rel);
        persist::save_document(&mut doc, pfad).expect("Baseline-Zeichnung schreiben");
        let bb = jjcad_core::render::visible_bbox(&doc);
        let zoom = (FLAECHE_PX.0 / bb.width()).min(FLAECHE_PX.1 / bb.height());
        println!(
            "{}: {}x{} mm (1:{}), Mitte ({:.0}, {:.0}), Ansicht {:.0} % ({:.3} px/mm)",
            name,
            bb.width().round(),
            bb.height().round(),
            massstab as i64,
            bb.center().x,
            bb.center().y,
            zoom * 100.0,
            zoom
        );
        // Maschinenlesbar für scripts/bogen-baseline.sh: Name, Zoom, Blickpunkt.
        println!(
            "POSE {} {:.4} {:.1} {:.1}",
            name,
            zoom,
            bb.center().x,
            bb.center().y
        );
    }
}
