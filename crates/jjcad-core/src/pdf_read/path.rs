//! PDF-Pfad aufbauen, Kurven abflachen, Form erkennen.
//!
//! PDF schreibt alles als Pfad (`m`, `l`, `c`, `v`, `y`, `re`, `h`). Hier
//! entstehen daraus die Formen, die JoJa-CAD kennt. Kurven werden **im
//! Zielmaß** abgeflacht (Toleranz in mm), sonst wirkt ein bei 1:100
//! importierter Kreis sichtbar eckig.

use crate::geom::*;

/// Toleranz beim Abflachen in mm (Weltmaß, nach Umrechnung der Seite).
pub const TOL_MM: f64 = 0.03;

/// Ein Unterpfad (Folge von Punkten zwischen zwei `m`).
#[derive(Clone, Debug)]
pub struct SubPath {
    pub pts: Vec<Vec2>,
    pub closed: bool,
}

/// Sammelbecken für die Pfadoperatoren eines Abschnitts.
#[derive(Default)]
pub struct PdfPath {
    subs: Vec<SubPath>,
    cur: Vec<Vec2>,
    start: Vec2,
    started: bool,
}

impl PdfPath {
    pub fn clear(&mut self) {
        self.subs.clear();
        self.cur.clear();
        self.started = false;
    }

    fn flush(&mut self, closed: bool) {
        if self.cur.len() >= 2 {
            self.subs.push(SubPath {
                pts: std::mem::take(&mut self.cur),
                closed,
            });
        } else {
            self.cur.clear();
        }
    }

    pub fn move_to(&mut self, p: Vec2) {
        self.flush(false);
        self.cur.push(p);
        self.start = p;
        self.started = true;
    }

    pub fn line_to(&mut self, p: Vec2) {
        if !self.started {
            self.move_to(p);
            return;
        }
        // Doppelte Punkte weglassen (PDF schreibt sie häufig).
        if let Some(last) = self.cur.last() {
            if last.dist(p) < 1e-6 {
                return;
            }
        }
        self.cur.push(p);
    }

    /// Kubische Kurve abflachen (schon in Zielkoordinaten).
    pub fn curve_to(&mut self, c1: Vec2, c2: Vec2, end: Vec2) {
        let Some(&p0) = self.cur.last() else {
            self.move_to(end);
            return;
        };
        let mut pts = Vec::new();
        flatten_cubic(p0, c1, c2, end, TOL_MM, 0, &mut pts);
        for p in pts {
            self.line_to(p);
        }
    }

    /// Aktueller Pfadpunkt (für `v`, das den letzten Punkt als Kontrolle nutzt).
    pub fn current(&self) -> Option<Vec2> {
        self.cur.last().copied()
    }

    pub fn close(&mut self) {
        if self.cur.len() >= 2 {
            let pts = std::mem::take(&mut self.cur);
            self.subs.push(SubPath { pts, closed: true });
        } else {
            self.cur.clear();
        }
        if self.started {
            self.cur.push(self.start);
        }
    }

    /// Rechteck als geschlossener Unterpfad.
    pub fn rect(&mut self, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) {
        self.flush(false);
        self.subs.push(SubPath {
            pts: vec![p0, p1, p2, p3],
            closed: true,
        });
        self.started = false;
        self.cur.clear();
    }

    /// Alle fertigen Unterpfade (offene werden abgeschlossen übernommen).
    pub fn take(&mut self) -> Vec<SubPath> {
        self.flush(false);
        std::mem::take(&mut self.subs)
    }
}

/// Kubische Kurve rekursiv abflachen, bis sie unter der Toleranz gerade ist.
fn flatten_cubic(p0: Vec2, c1: Vec2, c2: Vec2, p3: Vec2, tol: f64, tiefe: u32, out: &mut Vec<Vec2>) {
    if tiefe >= 12 {
        out.push(p3);
        return;
    }
    // Abstand der Kontrollpunkte zur Sehne — einfaches, robustes Maß.
    let d1 = abstand_zur_geraden(c1, p0, p3);
    let d2 = abstand_zur_geraden(c2, p0, p3);
    if (d1 + d2) * 0.5 <= tol {
        out.push(p3);
        return;
    }
    // De Casteljau
    let p01 = (p0 + c1) * 0.5;
    let p12 = (c1 + c2) * 0.5;
    let p23 = (c2 + p3) * 0.5;
    let p012 = (p01 + p12) * 0.5;
    let p123 = (p12 + p23) * 0.5;
    let pm = (p012 + p123) * 0.5;
    flatten_cubic(p0, p01, p012, pm, tol, tiefe + 1, out);
    flatten_cubic(pm, p123, p23, p3, tol, tiefe + 1, out);
}

fn abstand_zur_geraden(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l = d.len();
    if l < 1e-9 {
        return p.dist(a);
    }
    ((d.x * (a.y - p.y) - (a.x - p.x) * d.y) / l).abs()
}

/// Erkannte Form eines Unterpfads.
#[derive(Clone, Debug)]
pub enum GeoShape {
    Line(Vec2, Vec2),
    Poly(Vec<Vec2>, bool),
    Circle(Vec2, f64),
    Arc(Vec2, f64, f64, f64),
}

/// Form aus einem Unterpfad ableiten: Gerade, Kreis, Bogen oder Streckenzug.
pub fn classify(sub: &SubPath) -> GeoShape {
    let mut pts: Vec<Vec2> = Vec::with_capacity(sub.pts.len());
    for p in &sub.pts {
        if pts.last().map(|l: &Vec2| l.dist(*p) > 1e-6).unwrap_or(true) {
            pts.push(*p);
        }
    }
    if sub.closed && pts.len() > 2 {
        // Geschlossene Pfade enden oft am Startpunkt — der doppelte Punkt stört
        // die Kreiserkennung, ist aber für den Umriss egal.
        if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
            if first.dist(*last) < 1e-6 {
                pts.pop();
            }
        }
    }
    if pts.len() == 2 {
        return GeoShape::Line(pts[0], pts[1]);
    }
    if pts.len() >= 6 {
        if let Some((c, r)) = kreis_aus_punkten(&pts) {
            if sub.closed {
                return GeoShape::Circle(c, r);
            }
            if let Some((a0, a1)) = bogen_winkel(&pts, c) {
                return GeoShape::Arc(c, r, a0, a1);
            }
        }
    }
    GeoShape::Poly(pts, sub.closed)
}

/// Mittelpunkt und Radius, wenn alle Punkte auf einem Kreis liegen (1 %).
fn kreis_aus_punkten(pts: &[Vec2]) -> Option<(Vec2, f64)> {
    let n = pts.len();
    // Drei gut verteilte Punkte für den Umkreis, danach prüfen.
    let (pa, pb, pc) = (pts[0], pts[n / 3], pts[2 * n / 3]);
    let c = umkreis(pa, pb, pc)?;
    let r = c.dist(pa);
    if r < 1e-6 {
        return None;
    }
    for p in pts {
        if (c.dist(*p) - r).abs() > r * 0.01 {
            return None;
        }
    }
    Some((c, r))
}

/// Schnittpunkt der Mittelsenkrechten dreier Punkte.
fn umkreis(a: Vec2, b: Vec2, c: Vec2) -> Option<Vec2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let a2 = a.x * a.x + a.y * a.y;
    let b2 = b.x * b.x + b.y * b.y;
    let c2 = c.x * c.x + c.y * c.y;
    Some(v(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    ))
}

/// Start- und Endwinkel eines Bogens, wenn der Umlaufsinn eindeutig ist.
fn bogen_winkel(pts: &[Vec2], c: Vec2) -> Option<(f64, f64)> {
    let a0 = (pts[0] - c).angle();
    let mut sweep = 0.0;
    let mut prev = a0;
    for p in &pts[1..] {
        let a = (*p - c).angle();
        let mut d = a - prev;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        sweep += d;
        prev = a;
    }
    if sweep.abs() < 1e-3 || sweep.abs() > std::f64::consts::TAU - 1e-3 {
        return None;
    }
    Some((a0, a0 + sweep))
}

/// Fläche eines Rings (Vorzeichen = Umlaufsinn).
pub fn ring_area(pts: &[Vec2]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        s += a.x * b.y - b.x * a.y;
    }
    s * 0.5
}

/// Liegt `p` innerhalb des Rings? (Strahlverfahren)
pub fn point_in_ring(p: Vec2, ring: &[Vec2]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut innen = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if ((a.y > p.y) != (b.y > p.y))
            && (p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x)
        {
            innen = !innen;
        }
        j = i;
    }
    innen
}

/// Ringe zu Außenring + Löchern zusammenfassen (Flächenfüllung).
///
/// Sortiert nach Fläche, dann Enthaltensein prüfen: der kleinste umschließende
/// Ring nimmt den Ring als Loch auf. Das entspricht dem Bild, das PDF mit
/// Even-Odd bzw. Nonzero gezeichnet hat, sobald die Ringe geschachtelt sind.
pub fn fill_rings(rings: Vec<Vec<Vec2>>) -> Vec<(Vec<Vec2>, Vec<Vec<Vec2>>)> {
    let mut sorted: Vec<(f64, Vec<Vec2>)> = rings
        .into_iter()
        .filter(|r| r.len() >= 3 && ring_area(r).abs() > 1e-9)
        .map(|r| (ring_area(&r).abs(), r))
        .collect();
    sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(Vec<Vec2>, Vec<Vec<Vec2>>)> = Vec::new();
    for (_, ring) in sorted {
        let probe = ring[0];
        // Kleinstes vorhandenes Außenobjekt suchen, das diesen Ring enthält.
        let mut ziel: Option<usize> = None;
        let mut ziel_flaeche = f64::INFINITY;
        for (i, (outer, _)) in out.iter().enumerate() {
            let flaeche = ring_area(outer).abs();
            if flaeche < ziel_flaeche && point_in_ring(probe, outer) {
                ziel = Some(i);
                ziel_flaeche = flaeche;
            }
        }
        match ziel {
            Some(i) => out[i].1.push(ring),
            None => out.push((ring, Vec::new())),
        }
    }
    out
}
