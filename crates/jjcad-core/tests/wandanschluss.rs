//! Abnahme der Wandanschlüsse: **verbundene Basislinien-Enden schließen
//! sichtbar aneinander an.**
//!
//! Geprüft wird die gezeichnete Geometrie — Füllflächen und Linien aus der
//! Displayliste plus ein Pixelbild —, nicht eine Formel. Für jede Ecke gilt:
//!
//! 1. **Keine Lücke**: jeder Punkt im Wandkörper (Vereinigung der Bänder),
//!    der mindestens 1,5 mm von deren Rand entfernt liegt, ist gefüllt — auch
//!    im Pixelbild (dort sähe eine Lücke weiß aus).
//! 2. **Keine Naht im Körper**: keine Linie tiefer als 1,5 mm im Wandinneren.
//! 3. **Geschlossene Umrandung**: jede Füllkante ist umrandet oder liegt in
//!    einer anderen Füllung.
//! 4. **Gemeinsame Gehrung**: bei gleich dicken Wänden enden beide Füllungen
//!    auf **derselben** Geraden (der Winkelhalbierenden) — die Ecken beider
//!    Wände sind dieselben Punkte.

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{dist_point_segment, point_in_polygon, Vec2};
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::style::WallAlign;
use jjcad_core::wall::{Seam, WallIndex};
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

fn wand(s: &mut Session, a: (f64, f64), b: (f64, f64), t: f64, al: WallAlign) {
    s.execute(
        "draw.wall",
        &json!({
            "a": [a.0 / 1000.0, a.1 / 1000.0],
            "b": [b.0 / 1000.0, b.1 / 1000.0],
            "thickness": t / 1000.0,
            "align": match al {
                WallAlign::Left => "left",
                WallAlign::Center => "center",
                WallAlign::Right => "right",
            },
        }),
    )
    .unwrap();
}

/// Band einer Wand (Basislinie ± halbe Dicke, um die Ausrichtung verschoben).
fn band(a: (f64, f64), b: (f64, f64), t: f64, al: WallAlign) -> Vec<Vec2> {
    let a = Vec2::new(a.0, a.1);
    let b = Vec2::new(b.0, b.1);
    let dir = (b - a).normalized();
    let n = dir.perp();
    let h = t * 0.5;
    let off = n * (h * al.offset());
    let (a, b) = (a + off, b + off);
    vec![
        a + n * h,
        b + n * h,
        b - n * h,
        a - n * h,
    ]
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
    /// Grauwert im Pixelbild (0 = schwarz, 255 = weiß).
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
        let px = pm.pixel(x as u32, y as u32)?;
        Some(((px.red() as u32 + px.green() as u32 + px.blue() as u32) / 3) as u8)
    }
}

/// Zeichnung eines Ausschnitts messen (Displayliste + Pixelbild).
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

/// Liegt `p` in der Vereinigung der Bänder, und zwar mit Abstand `r` zum Rand —
/// und nicht jenseits einer Anschlussnaht (dort ist der Körper absichtlich
/// abgeschnitten)?
fn im_korper(p: Vec2, waende: &[(Vec<Vec2>, Vec<Seam>)], r: f64) -> bool {
    waende.iter().any(|(b, naehte)| {
        point_in_polygon(p, b)
            && b.iter()
                .enumerate()
                .map(|(i, e)| dist_point_segment(p, *e, b[(i + 1) % b.len()]))
                .fold(f64::INFINITY, f64::min)
                >= r
            && !hinter_naht(p, naehte)
    })
}

/// Prüfungen 1–3 für einen Ausschnitt.
fn pruefe(name: &str, z: &Z, waende: &[(Vec<Vec2>, Vec<Seam>)], bbox: [f64; 4]) {
    // 1. Keine Lücke — im Vektor und im Pixelbild.
    let mut luecken: Vec<Vec2> = Vec::new();
    let mut weiss: Vec<Vec2> = Vec::new();
    let mut y = bbox[1];
    while y <= bbox[3] {
        let mut x = bbox[0];
        while x <= bbox[2] {
            let p = Vec2::new(x, y);
            if im_korper(p, waende, 1.5) {
                if z.tiefe(p) < -0.5 {
                    luecken.push(p);
                }
                if let Some(g) = z.grau(p) {
                    // Füllung ist hellgrau (~195); weiß wäre eine Lücke.
                    if g > 235 {
                        weiss.push(p);
                    }
                }
            }
            x += 2.0;
        }
        y += 2.0;
    }
    assert!(
        luecken.is_empty(),
        "{name}: {} Punkte im Wandkörper ohne Füllung, z. B. {:?}",
        luecken.len(),
        &luecken[..luecken.len().min(4)]
    );
    assert!(
        weiss.is_empty(),
        "{name}: {} weiße Pixel im Wandkörper (Lücke), z. B. {:?}",
        weiss.len(),
        &weiss[..weiss.len().min(4)]
    );

    // 2. Keine Linie im Wandinneren (tiefer als 1,5 mm).
    let mut innen: Vec<Vec2> = Vec::new();
    for (a, b) in &z.lines {
        let n = ((*b - *a).len() / 2.0).ceil().max(1.0) as usize;
        for j in 0..=n {
            let p = *a + (*b - *a) * (j as f64 / n as f64);
            if z.tiefe(p) > 1.5 {
                innen.push(p);
            }
        }
    }
    assert!(
        innen.is_empty(),
        "{name}: {} Linienpunkte im Wandinneren, z. B. {:?}",
        innen.len(),
        &innen[..innen.len().min(4)]
    );

    // 3. Jede Füllkante ist umrandet oder liegt in einer anderen Füllung.
    let mut offen: Vec<Vec2> = Vec::new();
    for (i, f) in z.fills.iter().enumerate() {
        for k in 0..f.len() {
            let (p0, p1) = (f[k], f[(k + 1) % f.len()]);
            let n = ((p1 - p0).len() / 2.0).ceil().max(1.0) as usize;
            for j in 0..=n {
                let p = p0 + (p1 - p0) * (j as f64 / n as f64);
                let drin = z
                    .fills
                    .iter()
                    .enumerate()
                    .any(|(o, g)| o != i && point_in_polygon(p, g));
                if !drin && !z.linie_nahe(p, 0.3) {
                    offen.push(p);
                }
            }
        }
    }
    assert!(
        offen.is_empty(),
        "{name}: {} Punkte auf Füllkanten ohne Linie, z. B. {:?}",
        offen.len(),
        &offen[..offen.len().min(4)]
    );
}

// ---------------------------------------------------------------------------
// Fälle
// ---------------------------------------------------------------------------

/// Liegt `p` jenseits einer Anschlussnaht dieser Wand (abgeschnitten)?
fn hinter_naht(p: Vec2, naehte: &[Seam]) -> bool {
    naehte.iter().any(|s| (p - s.p).dot(s.nrm) < -0.5)
}

/// Ecke: Wand A von (-2000,0) zum Knoten, Wand B vom Knoten unter `deg`.
/// Rückgabe: Sitzung, Bänder und Nähte je Wand.
#[allow(clippy::type_complexity)]
fn ecke(
    deg: f64,
    ta: f64,
    tb: f64,
    aa: WallAlign,
    ab: WallAlign,
) -> (Session, Vec<(Vec<Vec2>, Vec<Seam>)>) {
    let mut s = session();
    let p = (
        2000.0 * deg.to_radians().cos(),
        2000.0 * deg.to_radians().sin(),
    );
    let a = ((-2000.0, 0.0), (0.0, 0.0), ta, aa);
    let b = ((0.0, 0.0), p, tb, ab);
    wand(&mut s, a.0, a.1, a.2, a.3);
    wand(&mut s, b.0, b.1, b.2, b.3);
    let idx = WallIndex::build(&s.doc, s.doc.ents());
    let naehte = |i: usize| idx.seams_of(s.doc.entities[i].id);
    let baender = vec![
        (band(a.0, a.1, a.2, a.3), naehte(0)),
        (band(b.0, b.1, b.2, b.3), naehte(1)),
    ];
    (s, baender)
}

#[test]
fn ecken_ohne_luecke_und_ohne_naht() {
    let mut faelle = Vec::new();
    for deg in [20.0, 30.0, 45.0, 60.0, 90.0, 120.0, 135.0, 150.0] {
        for (ta, tb) in [(300.0, 300.0), (365.0, 115.0), (115.0, 365.0), (175.0, 300.0)] {
            for (aa, ab) in [
                (WallAlign::Center, WallAlign::Center),
                (WallAlign::Left, WallAlign::Left),
                (WallAlign::Left, WallAlign::Right),
                (WallAlign::Right, WallAlign::Left),
                (WallAlign::Center, WallAlign::Left),
            ] {
                faelle.push((deg, ta, tb, aa, ab));
            }
        }
    }
    for (deg, ta, tb, aa, ab) in faelle {
        let name = format!("{deg}° {ta}/{tb} {aa:?}/{ab:?}");
        let (s, waende) = ecke(deg, ta, tb, aa, ab);
        let z = messen(&s, [-600.0, -600.0, 600.0, 600.0]);
        assert!(!z.fills.is_empty(), "{name}: keine Füllung gezeichnet");
        pruefe(&name, &z, &waende, [-600.0, -600.0, 600.0, 600.0]);
    }
}

/// Gleich dicke Wände: die Gehrung liegt auf **einer** Geraden, beide Füllungen
/// enden dort. Geprüft wird, dass beide Wände dieselben beiden Gehrungspunkte
/// als Füllungsecken haben.
#[test]
fn gleich_dick_gemeinsame_gehrung() {
    for deg in [45.0f64, 60.0, 90.0, 120.0, 135.0] {
        let (s, _) = ecke(deg, 300.0, 300.0, WallAlign::Center, WallAlign::Center);
        let z = messen(&s, [-600.0, -600.0, 600.0, 600.0]);
        // Gehrungspunkte: Ecken, die in beiden Wandkörpern vorkommen.
        // Die größten beiden Füllungen sind die Wandkörper (die Füllbänder an
        // der Naht sind schmaler).
        let mut nach_flaeche: Vec<&Vec<Vec2>> = z.fills.iter().collect();
        nach_flaeche.sort_by(|p, q| {
            jjcad_core::geom::polygon_area(q)
                .abs()
                .partial_cmp(&jjcad_core::geom::polygon_area(p).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let a = nach_flaeche[0];
        let b = nach_flaeche[1];
        let gemeinsam: Vec<Vec2> = a
            .iter()
            .filter(|p| b.iter().any(|q| q.dist(**p) <= 0.01))
            .cloned()
            .collect();
        assert!(
            gemeinsam.len() >= 2,
            "{deg}°: keine gemeinsame Gehrung, A={a:?} B={b:?}"
        );
        // Die Naht ist eine Gerade durch beide Punkte und steht senkrecht auf
        // der Winkelhalbierenden — hier genügt: beide Punkte liegen auf einer
        // Geraden mit der Nahtlänge ≥ Wanddicke.
        assert!(
            gemeinsam[0].dist(gemeinsam[1]) >= 299.0,
            "{deg}°: Gehrungslinie zu kurz: {gemeinsam:?}"
        );
    }
}

/// Drei Wände in einem Knoten: keine Lücke, keine Naht im Körper, kein
/// Splitter (kein Konturpunkt weiter als eine Wanddicke vom Knoten weg).
#[test]
fn drei_waende_in_einem_punkt() {
    let mut s = session();
    let a = ((-2000.0, 0.0), (0.0, 0.0), 300.0, WallAlign::Center);
    let b = ((0.0, 0.0), (2000.0 * 120.0_f64.to_radians().cos(), 2000.0 * 120.0_f64.to_radians().sin()), 200.0, WallAlign::Center);
    let c = ((0.0, 0.0), (2000.0 * 240.0_f64.to_radians().cos(), 2000.0 * 240.0_f64.to_radians().sin()), 200.0, WallAlign::Center);
    for w in [a, b, c] {
        wand(&mut s, w.0, w.1, w.2, w.3);
    }
    let idx = WallIndex::build(&s.doc, s.doc.ents());
    let naehte = |i: usize| idx.seams_of(s.doc.entities[i].id);
    let baender = vec![
        (band(a.0, a.1, a.2, a.3), naehte(0)),
        (band(b.0, b.1, b.2, b.3), naehte(1)),
        (band(c.0, c.1, c.2, c.3), naehte(2)),
    ];
    let z = messen(&s, [-600.0, -600.0, 600.0, 600.0]);
    pruefe("drei Wände", &z, &baender, [-600.0, -600.0, 600.0, 600.0]);
    // Kein Splitter: jeder Konturpunkt liegt im Wandkörper oder höchstens
    // eine Wanddicke daneben (Gehrungsecke hinter dem Wandende).
    for e in &s.doc.entities {
        for p in render::wall_contour_points(&s.doc, e) {
            let d = baender
                .iter()
                .map(|(b, _)| {
                    if point_in_polygon(p, b) {
                        0.0
                    } else {
                        b.iter()
                            .enumerate()
                            .map(|(i, q)| dist_point_segment(p, *q, b[(i + 1) % b.len()]))
                            .fold(f64::INFINITY, f64::min)
                    }
                })
                .fold(f64::INFINITY, f64::min);
            assert!(
                d <= 200.0,
                "Konturpunkt ({}, {}) liegt {d:.1} mm neben dem Wandkörper (Splitter)",
                p.x,
                p.y
            );
        }
    }
}

/// Stich und Kreuzung bleiben geschlossen (durchlaufende Wand, kein Beschnitt
/// der durchlaufenden Kante quer durch den Körper).
#[test]
fn stich_und_kreuzung_geschlossen() {
    // Stich: dünne Wand endet auf der Basislinie einer durchlaufenden Wand.
    let mut s = session();
    let a = ((-2000.0, 0.0), (2000.0, 0.0), 300.0, WallAlign::Center);
    let b = ((0.0, 0.0), (2000.0 * 20.0_f64.to_radians().cos(), 2000.0 * 20.0_f64.to_radians().sin()), 115.0, WallAlign::Center);
    wand(&mut s, a.0, a.1, a.2, a.3);
    wand(&mut s, b.0, b.1, b.2, b.3);
    let idx = WallIndex::build(&s.doc, s.doc.ents());
    let naehte = |i: usize| idx.seams_of(s.doc.entities[i].id);
    let baender = vec![
        (band(a.0, a.1, a.2, a.3), naehte(0)),
        (band(b.0, b.1, b.2, b.3), naehte(1)),
    ];
    let z = messen(&s, [-600.0, -600.0, 600.0, 600.0]);
    pruefe("Stich 20°", &z, &baender, [-600.0, -600.0, 600.0, 600.0]);

    // Kreuzung: zwei Wände, die sich in der Mitte überkreuzen.
    let mut s = session();
    let a = ((0.0, 0.0), (6000.0, 0.0), 300.0, WallAlign::Center);
    let b = ((3000.0, -3000.0), (3000.0, 3000.0), 200.0, WallAlign::Center);
    wand(&mut s, a.0, a.1, a.2, a.3);
    wand(&mut s, b.0, b.1, b.2, b.3);
    let idx = WallIndex::build(&s.doc, s.doc.ents());
    let naehte = |i: usize| idx.seams_of(s.doc.entities[i].id);
    let baender = vec![
        (band(a.0, a.1, a.2, a.3), naehte(0)),
        (band(b.0, b.1, b.2, b.3), naehte(1)),
    ];
    let z = messen(&s, [2400.0, -600.0, 3600.0, 600.0]);
    pruefe("Kreuzung", &z, &baender, [2400.0, -600.0, 3600.0, 600.0]);
}

/// Die Reihenfolge der Wände im Dokument darf das Bild nicht ändern.
#[test]
fn reihenfolge_aendert_das_bild_nicht() {
    let bauen = |verdreht: bool| {
        let mut s = session();
        let mut ws = vec![
            ((-2000.0, 0.0), (0.0, 0.0), 365.0, WallAlign::Center),
            ((0.0, 0.0), (2000.0 * 135.0_f64.to_radians().cos(), 2000.0 * 135.0_f64.to_radians().sin()), 115.0, WallAlign::Left),
            ((0.0, 0.0), (2000.0 * 45.0_f64.to_radians().cos(), 2000.0 * 45.0_f64.to_radians().sin()), 200.0, WallAlign::Right),
        ];
        if verdreht {
            ws.reverse();
        }
        for w in ws {
            wand(&mut s, w.0, w.1, w.2, w.3);
        }
        s
    };
    let a = messen(&bauen(false), [-600.0, -600.0, 600.0, 600.0]);
    let b = messen(&bauen(true), [-600.0, -600.0, 600.0, 600.0]);
    assert_eq!(a.fills.len(), b.fills.len(), "Füllflächenzahl hängt an der Reihenfolge");
    assert_eq!(a.lines.len(), b.lines.len(), "Linienzahl hängt an der Reihenfolge");
}

/// Die Naht gilt **beidseitig oder gar nicht** — unabhängig von Wandlänge,
/// Dicke und Ausrichtung. Sonst schneidet die eine Wand und die andere nicht,
/// und die Körper überlappen sichtbar (Kreuz aus beiden Umrandungen).
#[test]
fn naht_gilt_beidseitig() {
    for deg in [20.0f64, 35.0, 45.0, 90.0, 135.0, 170.0] {
        for (la, lb) in [(3000.0, 800.0), (800.0, 3000.0), (500.0, 500.0)] {
            for (aa, ab) in [
                (WallAlign::Center, WallAlign::Center),
                (WallAlign::Left, WallAlign::Right),
                (WallAlign::Left, WallAlign::Left),
                (WallAlign::Right, WallAlign::Center),
            ] {
                let mut s = session();
                let r = deg.to_radians();
                wand(&mut s, (-la, 0.0), (0.0, 0.0), 240.0, aa);
                wand(
                    &mut s,
                    (0.0, 0.0),
                    (lb * r.cos(), lb * r.sin()),
                    240.0,
                    ab,
                );
                let idx = WallIndex::build(&s.doc, s.doc.ents());
                let a = idx.seams_of(s.doc.entities[0].id);
                let b = idx.seams_of(s.doc.entities[1].id);
                assert_eq!(
                    a.len(),
                    b.len(),
                    "{deg}° {la}/{lb} {aa:?}/{ab:?}: Naht einseitig (A {} · B {})",
                    a.len(),
                    b.len()
                );
            }
        }
    }
}

/// Sehr spitzer Winkel (unter `MIN_JOINT_ANGLE`): die Wände stoßen stumpf,
/// **ohne Finne** — die Wand darf nicht als meterlange Spitze über den Knoten
/// hinausschießen. Geprüft am Füllbild: zwischen 1,2 Dicken und 1,5 m Abstand
/// vom Knoten darf kein Füllpunkt liegen.
#[test]
fn spitze_ecke_ohne_finne() {
    for deg in [150.0f64, 155.0, 160.0, 170.0] {
        for (aa, ab) in [
            (WallAlign::Center, WallAlign::Center),
            (WallAlign::Center, WallAlign::Left),
            (WallAlign::Left, WallAlign::Right),
        ] {
            let (s, _) = ecke(deg, 300.0, 300.0, aa, ab);
            let idx = WallIndex::build(&s.doc, s.doc.ents());
            for e in s.doc.entities.iter() {
                assert!(
                    idx.seams_of(e.id).is_empty(),
                    "{deg}° {aa:?}/{ab:?}: Naht trotz spitzem Winkel"
                );
            }
            let z = messen(&s, [-2400.0, -2400.0, 2400.0, 2400.0]);
            for f in &z.fills {
                for p in f {
                    let r = (p.x * p.x + p.y * p.y).sqrt();
                    assert!(
                        !(360.0..1500.0).contains(&r),
                        "{deg}° {aa:?}/{ab:?}: Füllpunkt {p:?} bei {r:.0} mm — Finne"
                    );
                }
            }
        }
    }
}
