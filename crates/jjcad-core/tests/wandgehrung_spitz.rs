//! Wandgehrung bei extrem spitzen Ecken und unterschiedlich dicken Wänden.
//!
//! Geprüft wird die gezeichnete Geometrie (Displayliste), nicht eine Formel:
//! 1. Der Wandkörper ist vollständig gefüllt — auch das Stück, das die
//!    Gehrungslinie bei ungleich dicken Wänden abschneidet.
//! 2. Jede Füllkante ist umrandet (oder liegt in einer anderen Füllung).
//! 3. Keine Linie verläuft im Inneren einer Wandfläche.

use jjcad_core::geom::{dist_point_segment, point_in_polygon, Vec2};
use jjcad_core::style::WallAlign;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::Session;
use serde_json::json;

struct Soll {
    name: &'static str,
    a: [f64; 2],
    b: [f64; 2],
    thickness: f64,
    align: WallAlign,
}

/// Ecke: Wand A läuft von (-2000,0) zum Knoten, Wand B vom Knoten unter `deg`.
fn ecke(
    deg: f64,
    ta: f64,
    tb: f64,
    aa: WallAlign,
    ab: WallAlign,
) -> (Session, Vec<Soll>) {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    let e = [2000.0 * deg.to_radians().cos(), 2000.0 * deg.to_radians().sin()];
    for (a, b, t, al) in [
        ([-2000.0, 0.0], [0.0, 0.0], ta, aa),
        ([0.0, 0.0], e, tb, ab),
    ] {
        s.execute(
            "draw.wall",
            &json!({"a": [a[0] / 1000.0, a[1] / 1000.0],
                    "b": [b[0] / 1000.0, b[1] / 1000.0],
                    "thickness": t / 1000.0,
                    "align": match al {
                        WallAlign::Left => "left",
                        WallAlign::Right => "right",
                        WallAlign::Center => "center",
                    }}),
        )
        .unwrap();
    }
    let walls = vec![
        Soll { name: "A", a: [-2000.0, 0.0], b: [0.0, 0.0], thickness: ta, align: aa },
        Soll { name: "B", a: [0.0, 0.0], b: e, thickness: tb, align: ab },
    ];
    (s, walls)
}

/// Band einer Wand (Achse + Ausrichtungsversatz, ± halbe Dicke).
fn band(w: &Soll) -> Vec<Vec2> {
    let a = Vec2::new(w.a[0], w.a[1]);
    let b = Vec2::new(w.b[0], w.b[1]);
    let dir = (b - a).normalized();
    let n = dir.perp();
    let h = w.thickness * 0.5;
    let off = n * (h * w.align.offset());
    let (a, b) = (a + off, b + off);
    vec![a + n * h, b + n * h, b - n * h, a - n * h]
}

struct Zeichnung {
    fills: Vec<Vec<Vec2>>,
    lines: Vec<(Vec2, Vec2)>,
}

fn zeichnung(s: &Session) -> Zeichnung {
    let ctx = RenderCtx::with_scale(&s.doc, 1.0);
    let dl = render_document(&ctx);
    let mut fills = Vec::new();
    let mut lines = Vec::new();
    for p in &dl.prims {
        match &p.shape {
            Shape::Fill { outer, .. } => fills.push(outer.clone()),
            Shape::Line(a, b) => lines.push((*a, *b)),
            _ => {}
        }
    }
    Zeichnung { fills, lines }
}

impl Zeichnung {
    /// Tiefe im Inneren der Füllflächen: positiv innen, negativ außerhalb (mm).
    fn fuellungstiefe(&self, p: Vec2) -> f64 {
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

    /// Gilt als gefüllt, wenn der Punkt innen liegt oder höchstens `tol` neben
    /// einer Füllkante (Naht zwischen zwei Füllungen, Toleranz gegen Rundung).
    fn gefuellt(&self, p: Vec2, tol: f64) -> bool {
        self.fuellungstiefe(p) > -tol
    }

    fn linie_nahe(&self, p: Vec2, tol: f64) -> bool {
        self.lines
            .iter()
            .any(|(a, b)| dist_point_segment(p, *a, *b) <= tol)
    }
}

/// 1. Jeder Punkt des Wandkörpers bis zur eigenen Stirnebene muss gefüllt sein.
fn pruefe_gefuellt(name: &str, z: &Zeichnung, walls: &[Soll]) {
    let mut fehlt = Vec::new();
    for w in walls {
        let a = Vec2::new(w.a[0], w.a[1]);
        let b = Vec2::new(w.b[0], w.b[1]);
        let dir = (b - a).normalized();
        let n = dir.perp();
        let h = w.thickness * 0.5;
        let off = n * (h * w.align.offset());
        let (a, b) = (a + off, b + off);
        let len = a.dist(b);
        // bis 0,5 mm an die Stirnebenen heran; außen liegende Zipfel sind
        // Sache des Nachbarn
        let mut t = 0.5;
        while t < len - 0.5 {
            for k in -10..=10 {
                let s = h * (k as f64 / 10.0) * 0.94;
                let p = a + dir * t + n * s;
                if !z.gefuellt(p, 0.5) {
                    fehlt.push((w.name, p));
                }
            }
            t += 5.0;
        }
    }
    assert!(
        fehlt.is_empty(),
        "{}: {} Punkte im Wandkörper ohne Füllung, z. B. {:?}",
        name,
        fehlt.len(),
        &fehlt[..fehlt.len().min(4)]
    );
}

/// 2. Jede Füllkante ist umrandet oder liegt in einer anderen Füllung.
fn pruefe_umrandet(name: &str, z: &Zeichnung) {
    let mut offen = Vec::new();
    for (i, f) in z.fills.iter().enumerate() {
        for k in 0..f.len() {
            let (p0, p1) = (f[k], f[(k + 1) % f.len()]);
            let n = ((p1 - p0).len() / 2.0).ceil().max(1.0) as usize;
            for j in 0..=n {
                let p = p0 + (p1 - p0) * (j as f64 / n as f64);
                let innen = z
                    .fills
                    .iter()
                    .enumerate()
                    .any(|(o, g)| o != i && point_in_polygon(p, g));
                if !innen && !z.linie_nahe(p, 0.5) {
                    offen.push(p);
                }
            }
        }
    }
    assert!(
        offen.is_empty(),
        "{}: {} Punkte auf Füllkanten ohne Linie, z. B. {:?}",
        name,
        offen.len(),
        &offen[..offen.len().min(4)]
    );
}

/// 3. Keine Linie im Inneren einer Füllung (2 mm Abstand zum Rand).
fn pruefe_keine_innenlinien(name: &str, z: &Zeichnung) {
    let mut innen = Vec::new();
    for (a, b) in &z.lines {
        let n = ((*b - *a).len() / 2.0).ceil().max(1.0) as usize;
        for j in 0..=n {
            let p = *a + (*b - *a) * (j as f64 / n as f64);
            if z.gefuellt(p, 0.0) && z.fuellungstiefe(p) > 2.0 {
                innen.push(p);
            }
        }
    }
    assert!(
        innen.is_empty(),
        "{}: {} Linienpunkte im Wandinneren, z. B. {:?}",
        name,
        innen.len(),
        &innen[..innen.len().min(4)]
    );
}

fn pruefe(name: &str, s: &Session, walls: &[Soll]) {
    let z = zeichnung(s);
    assert!(!z.fills.is_empty(), "{name}: keine Füllungen gezeichnet");
    pruefe_gefuellt(name, &z, walls);
    pruefe_umrandet(name, &z);
    pruefe_keine_innenlinien(name, &z);
}

#[test]
fn rechte_ecke_unveraendert() {
    let (s, w) = ecke(90.0, 300.0, 300.0, WallAlign::Center, WallAlign::Center);
    pruefe("90 Grad", &s, &w);
}

#[test]
fn spitze_ecke_gleich_dick() {
    for deg in [30.0, 15.0, 10.0, 5.0, 2.0] {
        let (s, w) = ecke(deg, 300.0, 300.0, WallAlign::Center, WallAlign::Center);
        pruefe(&format!("{deg} Grad gleich dick"), &s, &w);
    }
}

#[test]
fn spitze_ecke_ungleich_dick() {
    // Vor dem Fix endete die dicke Wand vor ihrer Stirnebene: die Füllung hörte
    // mitten in der Ecke auf und die Kante blieb ohne Linie.
    for (deg, ta, tb) in [(20.0, 365.0, 115.0), (15.0, 365.0, 115.0), (10.0, 365.0, 115.0),
                          (5.0, 365.0, 115.0), (15.0, 115.0, 365.0)] {
        let (s, w) = ecke(deg, ta, tb, WallAlign::Center, WallAlign::Center);
        pruefe(&format!("{deg} Grad {ta}/{tb}"), &s, &w);
    }
}

#[test]
fn spitze_ecke_versetzte_achsen() {
    for (deg, aa, ab) in [
        (15.0, WallAlign::Left, WallAlign::Left),
        (15.0, WallAlign::Left, WallAlign::Right),
        (5.0, WallAlign::Left, WallAlign::Left),
        (5.0, WallAlign::Right, WallAlign::Left),
    ] {
        let (s, w) = ecke(deg, 300.0, 300.0, aa, ab);
        pruefe(&format!("{deg} Grad {aa:?}/{ab:?}"), &s, &w);
    }
}

#[test]
fn freies_ende_bleibt_gerade() {
    let (s, w) = ecke(90.0, 300.0, 300.0, WallAlign::Center, WallAlign::Center);
    let z = zeichnung(&s);
    // Stirnkanten der freien Enden liegen bei x = -2000 bzw. y = 2000
    let mut stirn_a = false;
    for (a, b) in &z.lines {
        if (a.x + 2000.0).abs() < 1e-6 && (b.x + 2000.0).abs() < 1e-6 {
            stirn_a = true;
        }
    }
    assert!(stirn_a, "Stirnkante am freien Wandende fehlt");
    pruefe("freie Enden", &s, &w);
}

/// Anschluss einer dünnen Wand an eine durchlaufende Wand (Stich) muss
/// unverändert geschlossen bleiben.
#[test]
fn stichanschluss_geschlossen() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [-2.0, 0.0], "b": [2.0, 0.0], "thickness": 0.3}),
    )
    .unwrap();
    let e = [2.0 * 20.0_f64.to_radians().cos(), 2.0 * 20.0_f64.to_radians().sin()];
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0], "b": [e[0], e[1]], "thickness": 0.115}),
    )
    .unwrap();
    let walls = vec![
        Soll { name: "quer", a: [-2000.0, 0.0], b: [2000.0, 0.0], thickness: 300.0, align: WallAlign::Center },
        Soll { name: "stich", a: [0.0, 0.0], b: e, thickness: 115.0, align: WallAlign::Center },
    ];
    pruefe("Stich 20 Grad", &s, &walls);
}

/// Die Prüfungen selbst müssen anschlagen, wenn die Füllung fehlt — sonst
/// beweisen sie nichts.
#[test]
fn pruefung_erkennt_fehlende_fuellung() {
    let (s, _w) = ecke(15.0, 365.0, 115.0, WallAlign::Center, WallAlign::Center);
    let z = zeichnung(&s);
    // Punkt im Körper der dicken Wand kurz vor ihrer Stirnebene, oberhalb der
    // dünnen Wand: muss gefüllt sein.
    let p = Vec2::new(-1.0, 170.0);
    assert!(z.gefuellt(p, 0.2), "Füllung fehlt bei ({}, {})", p.x, p.y);
    let leer = Zeichnung { fills: vec![], lines: z.lines.clone() };
    assert!(!leer.gefuellt(p, 0.2), "Prüffunktion meldet ohne Füllung trotzdem Treffer");
}
