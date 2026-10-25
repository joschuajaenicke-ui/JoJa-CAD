//! 2D-Geometrie-Kern von JoJa-CAD.
//!
//! Alle Koordinaten sind in Millimetern (Dokumenteinheit), Winkel in Bogenmaß,
//! Nullrichtung ist +X, Drehsinn mathematisch positiv (gegen den Uhrzeigersinn).

use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

pub const EPS: f64 = 1e-9;
pub const TOL: f64 = 1e-6;

// ---------------------------------------------------------------------------
// Vektor
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[inline]
pub fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn from_xy(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn polar(angle: f64, len: f64) -> Self {
        Self {
            x: angle.cos() * len,
            y: angle.sin() * len,
        }
    }

    #[inline]
    pub fn dot(self, o: Vec2) -> f64 {
        self.x * o.x + self.y * o.y
    }

    /// 2D-Kreuzprodukt (z-Komponente).
    #[inline]
    pub fn cross(self, o: Vec2) -> f64 {
        self.x * o.y - self.y * o.x
    }

    #[inline]
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }

    #[inline]
    pub fn len2(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    #[inline]
    pub fn normalized(self) -> Vec2 {
        let l = self.len();
        if l < EPS {
            Vec2::ZERO
        } else {
            Vec2 {
                x: self.x / l,
                y: self.y / l,
            }
        }
    }

    /// Normale (90° linksdrehend).
    #[inline]
    pub fn perp(self) -> Vec2 {
        Vec2 {
            x: -self.y,
            y: self.x,
        }
    }

    #[inline]
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Winkel eines Segments a->b.
    #[inline]
    pub fn angle_to(self, o: Vec2) -> f64 {
        (o - self).angle()
    }

    /// Vorzeichenbehafteter Winkel von `self` nach `o`, Bereich (-π, π].
    /// Positiv ist der mathematisch positive Drehsinn (gegen den Uhrzeigersinn).
    /// Anders als `o.angle() - self.angle()` läuft das Ergebnis nicht über ±π hinaus.
    #[inline]
    pub fn signed_angle_to(self, o: Vec2) -> f64 {
        let cross = self.x * o.y - self.y * o.x;
        let dot = self.x * o.x + self.y * o.y;
        cross.atan2(dot)
    }

    #[inline]
    pub fn rotated(self, angle: f64) -> Vec2 {
        let (s, c) = angle.sin_cos();
        Vec2 {
            x: self.x * c - self.y * s,
            y: self.x * s + self.y * c,
        }
    }

    #[inline]
    pub fn rotated_about(self, center: Vec2, angle: f64) -> Vec2 {
        center + (self - center).rotated(angle)
    }

    #[inline]
    pub fn dist(self, o: Vec2) -> f64 {
        (o - self).len()
    }

    #[inline]
    pub fn dist2(self, o: Vec2) -> f64 {
        (o - self).len2()
    }

    #[inline]
    pub fn lerp(self, o: Vec2, t: f64) -> Vec2 {
        self + (o - self) * t
    }

    #[inline]
    pub fn abs(self) -> Vec2 {
        Vec2 {
            x: self.x.abs(),
            y: self.y.abs(),
        }
    }

    #[inline]
    pub fn min(self, o: Vec2) -> Vec2 {
        Vec2 {
            x: self.x.min(o.x),
            y: self.y.min(o.y),
        }
    }

    #[inline]
    pub fn max(self, o: Vec2) -> Vec2 {
        Vec2 {
            x: self.x.max(o.x),
            y: self.y.max(o.y),
        }
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    #[inline]
    pub fn round_to(self, step: f64) -> Vec2 {
        if step <= 0.0 {
            self
        } else {
            Vec2 {
                x: (self.x / step).round() * step,
                y: (self.y / step).round() * step,
            }
        }
    }

    /// Auf Raster mit Ursprung `origin` einrasten.
    pub fn snap_grid(self, step: f64, origin: Vec2) -> Vec2 {
        if step <= 0.0 {
            return self;
        }
        let d = self - origin;
        origin + v((d.x / step).round() * step, (d.y / step).round() * step)
    }

    /// In ein Ortho-System zwingen (0/90/180/270°) relativ zum Basispunkt.
    pub fn ortho_from(self, base: Vec2, vertical_first: bool) -> Vec2 {
        let d = self - base;
        if vertical_first {
            if d.y.abs() >= d.x.abs() {
                v(base.x, self.y)
            } else {
                v(self.x, base.y)
            }
        } else if d.x.abs() >= d.y.abs() {
            v(self.x, base.y)
        } else {
            v(base.x, self.y)
        }
    }

    /// Auf 15°-Schritte einrasten (polar tracking).
    pub fn polar_snap_from(self, base: Vec2, step_deg: f64) -> Vec2 {
        let d = self - base;
        let l = d.len();
        if l < EPS {
            return self;
        }
        let step = step_deg.to_radians();
        let a = (d.angle() / step).round() * step;
        base + Vec2::polar(a, l)
    }

    /// Polarfang mit Winkel-Fenster: rastet nur ein, wenn die Zeigerrichtung
    /// höchstens `tol_deg` von einem Vielfachen der Schrittweite abweicht.
    pub fn polar_snap_from_tol(
        self,
        base: Vec2,
        step_deg: f64,
        tol_deg: f64,
    ) -> Option<Vec2> {
        let d = self - base;
        let l = d.len();
        if l < EPS {
            return None;
        }
        let step = step_deg.to_radians().max(1e-6);
        let a = (d.angle() / step).round() * step;
        let diff = (d.angle() - a).abs();
        // Winkelabweichung auf ±π normieren
        let diff = if diff > std::f64::consts::PI {
            std::f64::consts::TAU - diff
        } else {
            diff
        };
        if diff.to_degrees() > tol_deg.max(0.0) {
            return None;
        }
        Some(base + Vec2::polar(a, l))
    }

    pub fn to_array(self) -> [f64; 2] {
        [self.x, self.y]
    }

    pub fn from_array(a: [f64; 2]) -> Self {
        Self { x: a[0], y: a[1] }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        v(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        v(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f64) -> Vec2 {
        v(self.x * s, self.y * s)
    }
}
impl Mul<Vec2> for f64 {
    type Output = Vec2;
    fn mul(self, o: Vec2) -> Vec2 {
        o * self
    }
}
impl Div<f64> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f64) -> Vec2 {
        v(self.x / s, self.y / s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        v(-self.x, -self.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}

// ---------------------------------------------------------------------------
// Bounding-Box
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct BBox {
    pub min: Vec2,
    pub max: Vec2,
}

impl Default for BBox {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl BBox {
    pub const EMPTY: BBox = BBox {
        min: Vec2 {
            x: f64::INFINITY,
            y: f64::INFINITY,
        },
        max: Vec2 {
            x: f64::NEG_INFINITY,
            y: f64::NEG_INFINITY,
        },
    };

    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self {
            min: min.min(max),
            max: min.max(max),
        }
    }

    pub fn from_point(p: Vec2) -> Self {
        Self { min: p, max: p }
    }

    pub fn from_points(pts: &[Vec2]) -> Self {
        let mut b = Self::EMPTY;
        for p in pts {
            b.add_point(*p);
        }
        b
    }

    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self::new(a, b)
    }

    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y
    }

    pub fn add_point(&mut self, p: Vec2) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn union(&mut self, o: BBox) {
        if !o.is_valid() {
            return;
        }
        self.add_point(o.min);
        self.add_point(o.max);
    }

    pub fn expanded(&self, d: f64) -> Self {
        if !self.is_valid() {
            return *self;
        }
        Self {
            min: self.min - v(d, d),
            max: self.max + v(d, d),
        }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn contains_bbox(&self, o: &BBox) -> bool {
        o.is_valid() && self.contains(o.min) && self.contains(o.max)
    }

    pub fn intersects(&self, o: &BBox) -> bool {
        self.is_valid()
            && o.is_valid()
            && self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
    }

    pub fn overlaps_rect(&self, a: Vec2, b: Vec2) -> bool {
        self.intersects(&BBox::from_corners(a, b))
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec2 {
        if self.is_valid() {
            self.max - self.min
        } else {
            Vec2::ZERO
        }
    }

    pub fn width(&self) -> f64 {
        self.size().x
    }

    pub fn height(&self) -> f64 {
        self.size().y
    }
}

// ---------------------------------------------------------------------------
// Affine Transformation
// ---------------------------------------------------------------------------

/// Affine Abbildung: x' = a*x + c*y + tx ; y' = b*x + d*y + ty
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub fn translate(t: Vec2) -> Self {
        Self {
            tx: t.x,
            ty: t.y,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            d: sy,
            ..Self::IDENTITY
        }
    }

    pub fn rotate(angle: f64) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            a: c,
            b: s,
            c: -s,
            d: c,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn rotate_about(center: Vec2, angle: f64) -> Self {
        Affine::translate(center) * Affine::rotate(angle) * Affine::translate(-center)
    }

    pub fn scale_about(center: Vec2, sx: f64, sy: f64) -> Self {
        Affine::translate(center) * Affine::scale(sx, sy) * Affine::translate(-center)
    }

    /// Spiegelung an der Geraden p1-p2.
    pub fn mirror(p1: Vec2, p2: Vec2) -> Self {
        let d = (p2 - p1).normalized();
        // sin_cos() liefert (sin, cos) — daher diese Reihenfolge!
        let (sin2, cos2) = (2.0 * d.angle()).sin_cos();
        let (a, b, c) = (cos2, sin2, sin2);
        // Spiegelmatrix: [cos2 sin2; sin2 -cos2]
        let m = Affine {
            a,
            b,
            c,
            d: -a,
            tx: 0.0,
            ty: 0.0,
        };
        Affine::translate(p1) * m * Affine::translate(-p1)
    }

    pub fn apply(&self, p: Vec2) -> Vec2 {
        v(
            self.a * p.x + self.c * p.y + self.tx,
            self.b * p.x + self.d * p.y + self.ty,
        )
    }

    pub fn apply_vec(&self, p: Vec2) -> Vec2 {
        v(self.a * p.x + self.c * p.y, self.b * p.x + self.d * p.y)
    }

    pub fn determinant(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    pub fn inverse(&self) -> Affine {
        let det = self.determinant();
        if det.abs() < 1e-15 {
            return Affine::IDENTITY;
        }
        let ia = self.d / det;
        let ib = -self.b / det;
        let ic = -self.c / det;
        let id = self.a / det;
        let tx = -(ia * self.tx + ic * self.ty);
        let ty = -(ib * self.tx + id * self.ty);
        Affine {
            a: ia,
            b: ib,
            c: ic,
            d: id,
            tx,
            ty,
        }
    }

    /// Skalierung in x/y-Richtung (Betrag).
    pub fn scale_factor(&self) -> f64 {
        (self.a * self.a + self.b * self.b).sqrt().max(1e-12)
    }

    pub fn is_mirrored(&self) -> bool {
        self.determinant() < 0.0
    }

    /// Rotation der Abbildung.
    pub fn rotation(&self) -> f64 {
        self.b.atan2(self.a)
    }
}

impl Mul for Affine {
    type Output = Affine;
    fn mul(self, o: Affine) -> Affine {
        // (self ∘ o)
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }
}

// ---------------------------------------------------------------------------
// Winkel-Helfer
// ---------------------------------------------------------------------------

#[inline]
pub fn norm_angle(a: f64) -> f64 {
    let mut a = a % TAU;
    if a < 0.0 {
        a += TAU;
    }
    a
}

/// Liegt Winkel `a` im Bogen [a0, a1] (CCW)?
pub fn angle_in_arc(a: f64, a0: f64, a1: f64) -> bool {
    let span = norm_angle(a1 - a0);
    let rel = norm_angle(a - a0);
    if span < TOL {
        // Vollkreis
        true
    } else {
        rel <= span + 1e-9
    }
}

pub fn deg(d: f64) -> f64 {
    d.to_radians()
}

pub fn to_deg(r: f64) -> f64 {
    r.to_degrees()
}

// ---------------------------------------------------------------------------
// Bogen
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Arc {
    pub c: Vec2,
    pub r: f64,
    pub a0: f64,
    pub a1: f64,
}

impl Arc {
    pub fn new(c: Vec2, r: f64, a0: f64, a1: f64) -> Self {
        Self { c, r, a0, a1 }
    }

    /// Bogen aus drei Punkten (Start, Punkt auf Bogen, Ende).
    pub fn from_3_points(p0: Vec2, pm: Vec2, p1: Vec2) -> Option<Self> {
        let d = 2.0 * (p0.x * (pm.y - p1.y) + pm.x * (p1.y - p0.y) + p1.x * (p0.y - pm.y));
        if d.abs() < 1e-9 {
            return None;
        }
        let p0l = p0.len2();
        let pml = pm.len2();
        let p1l = p1.len2();
        let c = v(
            (p0l * (pm.y - p1.y) + pml * (p1.y - p0.y) + p1l * (p0.y - pm.y)) / d,
            (p0l * (p1.x - pm.x) + pml * (p0.x - p1.x) + p1l * (pm.x - p0.x)) / d,
        );
        let r = c.dist(p0);
        let a0 = (p0 - c).angle();
        let am = (pm - c).angle();
        let a1 = (p1 - c).angle();
        let arc = if angle_in_arc(am, a0, a1) {
            Arc::new(c, r, a0, a1)
        } else {
            Arc::new(c, r, a1, a0)
        };
        Some(arc)
    }

    pub fn sweep(&self) -> f64 {
        let s = norm_angle(self.a1 - self.a0);
        if s < 1e-12 {
            TAU
        } else {
            s
        }
    }

    pub fn point_at(&self, t: f64) -> Vec2 {
        let a = self.a0 + self.sweep() * t;
        self.c + Vec2::polar(a, self.r)
    }

    pub fn start(&self) -> Vec2 {
        self.c + Vec2::polar(self.a0, self.r)
    }

    pub fn end(&self) -> Vec2 {
        self.c + Vec2::polar(self.a1, self.r)
    }

    pub fn mid(&self) -> Vec2 {
        self.point_at(0.5)
    }

    pub fn len(&self) -> f64 {
        self.r * self.sweep()
    }

    pub fn flatten(&self, tol: f64) -> Vec<Vec2> {
        let sweep = self.sweep();
        let max_step = if self.r > 1e-6 {
            (2.0 * (1.0 - (tol / self.r).clamp(-1.0, 1.0)).acos()).max(0.02)
        } else {
            0.3
        };
        let n = ((sweep / max_step).ceil() as usize).clamp(2, 512);
        (0..=n)
            .map(|i| {
                let a = self.a0 + sweep * (i as f64 / n as f64);
                self.c + Vec2::polar(a, self.r)
            })
            .collect()
    }

    pub fn bbox(&self) -> BBox {
        let mut b = BBox::from_point(self.start());
        b.add_point(self.end());
        for k in 0..4 {
            let a = k as f64 * PI / 2.0;
            if angle_in_arc(a, self.a0, self.a1) {
                b.add_point(self.c + Vec2::polar(a, self.r));
            }
        }
        b
    }

    pub fn contains_point(&self, p: Vec2, tol: f64) -> bool {
        let d = (p - self.c).len();
        d <= self.r + tol && d >= (self.r - tol).max(0.0) && angle_in_arc((p - self.c).angle(), self.a0, self.a1)
    }

    pub fn reversed(&self) -> Arc {
        Arc::new(self.c, self.r, self.a1, self.a0)
    }
}

// ---------------------------------------------------------------------------
// Segment (Linie oder Bogen)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Seg {
    Line(Vec2, Vec2),
    Arc(Arc),
}

impl Seg {
    pub fn start(&self) -> Vec2 {
        match self {
            Seg::Line(a, _) => *a,
            Seg::Arc(a) => a.start(),
        }
    }
    pub fn end(&self) -> Vec2 {
        match self {
            Seg::Line(_, b) => *b,
            Seg::Arc(a) => a.end(),
        }
    }
    pub fn len(&self) -> f64 {
        match self {
            Seg::Line(a, b) => a.dist(*b),
            Seg::Arc(a) => a.len(),
        }
    }
    pub fn point_at(&self, t: f64) -> Vec2 {
        match self {
            Seg::Line(a, b) => a.lerp(*b, t),
            Seg::Arc(a) => a.point_at(t),
        }
    }
    pub fn flatten(&self, tol: f64) -> Vec<Vec2> {
        match self {
            Seg::Line(a, b) => vec![*a, *b],
            Seg::Arc(a) => a.flatten(tol),
        }
    }
    pub fn bbox(&self) -> BBox {
        match self {
            Seg::Line(a, b) => BBox::new(*a, *b),
            Seg::Arc(a) => a.bbox(),
        }
    }
    pub fn reversed(&self) -> Seg {
        match self {
            Seg::Line(a, b) => Seg::Line(*b, *a),
            Seg::Arc(a) => Seg::Arc(a.reversed()),
        }
    }
    /// Kürzester Abstand Punkt→Segment, mit Punkt.
    pub fn closest(&self, p: Vec2) -> (Vec2, f64) {
        match self {
            Seg::Line(a, b) => {
                let q = closest_point_on_segment(p, *a, *b);
                (q, q.dist(p))
            }
            Seg::Arc(a) => {
                let ang = (p - a.c).angle();
                if a.r > 1e-9 && angle_in_arc(ang, a.a0, a.a1) {
                    let q = a.c + Vec2::polar(ang, a.r);
                    (q, q.dist(p))
                } else {
                    let s = a.start();
                    let e = a.end();
                    if s.dist(p) <= e.dist(p) {
                        (s, s.dist(p))
                    } else {
                        (e, e.dist(p))
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Polylinie (mit Bögen über bulge)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Vertex {
    pub p: Vec2,
    /// Bogenmaß der Krümmung: tan(theta/4). 0 = Gerade.
    pub bulge: f64,
}

impl Vertex {
    pub fn new(p: Vec2) -> Self {
        Self { p, bulge: 0.0 }
    }
    pub fn arc(p: Vec2, bulge: f64) -> Self {
        Self { p, bulge }
    }
}

pub fn bulge_from_arc(arc: &Arc) -> f64 {
    (arc.sweep() / 4.0).tan()
}

/// Kreisbogen aus Startpunkt, Endpunkt und Bulge.
pub fn arc_from_bulge(p0: Vec2, p1: Vec2, bulge: f64) -> Option<Arc> {
    if bulge.abs() < 1e-12 {
        return None;
    }
    let chord = p1 - p0;
    let d = chord.len();
    if d < 1e-12 {
        return None;
    }
    let theta = 4.0 * bulge.atan();
    let r = (d / 2.0) / (theta / 2.0).sin().abs();
    let mid = (p0 + p1) * 0.5;
    // Abstand Mittelpunkt zur Sehnenmitte
    let h = r * (theta / 2.0).cos();
    let n = chord.perp().normalized();
    let side = if bulge > 0.0 { 1.0 } else { -1.0 };
    let c = mid + n * (h * side * if theta.abs() > PI { 1.0 } else { 1.0 });
    let a0 = (p0 - c).angle();
    let a1 = (p1 - c).angle();
    let arc = if bulge > 0.0 {
        Arc::new(c, r, a0, a1)
    } else {
        Arc::new(c, r, a1, a0)
    };
    Some(arc)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Polyline {
    pub verts: Vec<Vertex>,
    pub closed: bool,
}

impl Polyline {
    pub fn new(pts: Vec<Vec2>, closed: bool) -> Self {
        Self {
            verts: pts.into_iter().map(Vertex::new).collect(),
            closed,
        }
    }

    pub fn segments(&self) -> Vec<Seg> {
        let mut out = Vec::new();
        if self.verts.len() < 2 {
            return out;
        }
        let n = self.verts.len();
        let last = if self.closed { n } else { n - 1 };
        for i in 0..last {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            if a.bulge.abs() > 1e-12 {
                if let Some(arc) = arc_from_bulge(a.p, b.p, a.bulge) {
                    out.push(Seg::Arc(arc));
                    continue;
                }
            }
            out.push(Seg::Line(a.p, b.p));
        }
        out
    }

    /// Geschlossener Polygonzug (Bögen fein aufgelöst).
    pub fn flatten(&self, tol: f64) -> Vec<Vec2> {
        let mut pts: Vec<Vec2> = Vec::new();
        let segs = self.segments();
        if segs.is_empty() {
            return self.verts.iter().map(|v| v.p).collect();
        }
        for (i, s) in segs.iter().enumerate() {
            let mut f = s.flatten(tol);
            if i > 0 && !f.is_empty() {
                f.remove(0);
            }
            pts.extend(f);
        }
        pts
    }

    pub fn bbox(&self) -> BBox {
        let mut b = BBox::EMPTY;
        for s in self.segments() {
            b.union(s.bbox());
        }
        if !b.is_valid() {
            b = BBox::from_points(&self.verts.iter().map(|v| v.p).collect::<Vec<_>>());
        }
        b
    }

    pub fn area(&self) -> f64 {
        if !self.closed {
            return 0.0;
        }
        polygon_area(&self.flatten(0.5))
    }

    pub fn len(&self) -> f64 {
        self.segments().iter().map(|s| s.len()).sum()
    }

    pub fn points(&self) -> Vec<Vec2> {
        self.verts.iter().map(|v| v.p).collect()
    }
}

// ---------------------------------------------------------------------------
// Polygon-Helfer
// ---------------------------------------------------------------------------

pub fn polygon_area(poly: &[Vec2]) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        a += p.cross(q);
    }
    a * 0.5
}

/// Kleinste Breite eines Polygons: 2·Fläche / Umfang.
///
/// Für einen Streifen der Länge L und Breite b ergibt das genau b — anders als
/// die Fläche allein ist das Maß unabhängig von der Länge. Damit lassen sich
/// Füllungen erkennen, die auf dem Bildschirm schmaler als ein Pixel sind
/// (z. B. die 0,4-mm-Haarnnaht an Wandstößen bei kleiner Anzeige): die sind
/// unsichtbar und dürfen gar nicht erst tesselliert werden.
pub fn polygon_min_width(poly: &[Vec2]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut per = 0.0;
    for i in 0..n {
        per += poly[i].dist(poly[(i + 1) % n]);
    }
    let a = polygon_area(poly).abs();
    if per < 1e-12 || !a.is_finite() || !per.is_finite() {
        return 0.0;
    }
    2.0 * a / per
}

pub fn polygon_centroid(poly: &[Vec2]) -> Vec2 {
    let a = polygon_area(poly);
    if a.abs() < 1e-9 {
        if poly.is_empty() {
            return Vec2::ZERO;
        }
        let mut s = Vec2::ZERO;
        for p in poly {
            s += *p;
        }
        return s / poly.len() as f64;
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        let f = p.cross(q);
        cx += (p.x + q.x) * f;
        cy += (p.y + q.y) * f;
    }
    v(cx / (6.0 * a), cy / (6.0 * a))
}

/// Punkt-in-Polygon (Even-Odd).
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = poly[i];
        let pj = poly[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y + 1e-18) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Schnittpunkte einer Strecke mit einem Polygon; liefert Segmente innerhalb.
pub fn clip_line_polygon(a: Vec2, b: Vec2, poly: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    if poly.len() < 3 {
        return vec![];
    }
    let d = b - a;
    let len2 = d.len2();
    if len2 < 1e-18 {
        return vec![];
    }
    let mut ts: Vec<f64> = vec![0.0, 1.0];
    let n = poly.len();
    for i in 0..n {
        let p1 = poly[i];
        let p2 = poly[(i + 1) % n];
        if let Some(x) = line_line(a, b, p1, p2, false, false) {
            let t = (x - a).dot(d) / len2;
            if (-1e-9..=1.0 + 1e-9).contains(&t) {
                ts.push(t.clamp(0.0, 1.0));
            }
        }
    }
    ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let mut out = Vec::new();
    for w in ts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if t1 - t0 < 1e-9 {
            continue;
        }
        let m = a + d * ((t0 + t1) * 0.5);
        if point_in_polygon(m, poly) {
            out.push((a + d * t0, a + d * t1));
        }
    }
    out
}

/// Einfacher Polygon-Offset (Miter) für geschlossene und offene Züge.
pub fn offset_polyline(pts: &[Vec2], closed: bool, dist: f64) -> Vec<Vec2> {
    let n = pts.len();
    if n < 2 {
        return pts.to_vec();
    }
    let count = if closed { n } else { n };
    let mut out = Vec::with_capacity(n);
    for i in 0..count {
        let p = pts[i];
        let prev = if i == 0 {
            if closed {
                Some(pts[n - 1])
            } else {
                None
            }
        } else {
            Some(pts[i - 1])
        };
        let next = if i == n - 1 {
            if closed {
                Some(pts[0])
            } else {
                None
            }
        } else {
            Some(pts[i + 1])
        };
        let mut nsum = Vec2::ZERO;
        if let Some(pr) = prev {
            let d = (p - pr).normalized();
            nsum += d.perp();
        }
        if let Some(nx) = next {
            let d = (nx - p).normalized();
            nsum += d.perp();
        }
        let nl = nsum.len();
        if nl < 1e-9 {
            out.push(p);
            continue;
        }
        let normal = nsum / nl;
        // Miter-Länge korrigieren
        let miter = 1.0 / (normal.dot(
            (if let Some(nx) = next {
                (nx - p).normalized().perp()
            } else if let Some(pr) = prev {
                (p - pr).normalized().perp()
            } else {
                Vec2::Y
            }),
        ))
        .clamp(0.3, 8.0);
        out.push(p + normal * (dist * miter));
    }
    out
}

// ---------------------------------------------------------------------------
// Schnittpunkte
// ---------------------------------------------------------------------------

pub fn closest_point_on_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let l2 = ab.len2();
    if l2 < 1e-18 {
        return a;
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    a + ab * t
}

pub fn dist_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    closest_point_on_segment(p, a, b).dist(p)
}

pub fn dist_point_line(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l = d.len();
    if l < 1e-15 {
        return p.dist(a);
    }
    ((p - a).cross(d) / l).abs()
}

pub fn line_line(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2, inf_a: bool, inf_b: bool) -> Option<Vec2> {
    let r = a2 - a1;
    let s = b2 - b1;
    let denom = r.cross(s);
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = (b1 - a1).cross(s) / denom;
    let u = (b1 - a1).cross(r) / denom;
    if !inf_a && !(-1e-9..=1.0 + 1e-9).contains(&t) {
        return None;
    }
    if !inf_b && !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    Some(a1 + r * t)
}

pub fn line_circle(a: Vec2, b: Vec2, c: Vec2, r: f64) -> Vec<Vec2> {
    let d = b - a;
    let f = a - c;
    let aa = d.dot(d);
    if aa < 1e-18 {
        return vec![];
    }
    let bb = 2.0 * f.dot(d);
    let cc = f.dot(f) - r * r;
    let disc = bb * bb - 4.0 * aa * cc;
    if disc < 0.0 {
        return vec![];
    }
    let sq = disc.sqrt();
    let mut out = Vec::new();
    for t in [(-bb - sq) / (2.0 * aa), (-bb + sq) / (2.0 * aa)] {
        if (-1e-9..=1.0 + 1e-9).contains(&t) {
            let p = a + d * t;
            if !out.iter().any(|q: &Vec2| q.dist(p) < 1e-7) {
                out.push(p);
            }
        }
    }
    out
}

pub fn circle_circle(c1: Vec2, r1: f64, c2: Vec2, r2: f64) -> Vec<Vec2> {
    let d = c1.dist(c2);
    if d < 1e-12 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 {
        return vec![];
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h2 = r1 * r1 - a * a;
    if h2 < 0.0 {
        return vec![];
    }
    let h = h2.sqrt();
    let base = c1 + (c2 - c1) * (a / d);
    let n = (c2 - c1).perp().normalized();
    if h < 1e-9 {
        vec![base]
    } else {
        vec![base + n * h, base - n * h]
    }
}

pub fn seg_seg(a: &Seg, b: &Seg) -> Vec<Vec2> {
    match (a, b) {
        (Seg::Line(a1, a2), Seg::Line(b1, b2)) => line_line(*a1, *a2, *b1, *b2, false, false)
            .into_iter()
            .collect(),
        (Seg::Line(a1, a2), Seg::Arc(arc)) | (Seg::Arc(arc), Seg::Line(a1, a2)) => {
            line_circle(*a1, *a2, arc.c, arc.r)
                .into_iter()
                .filter(|p| angle_in_arc((*p - arc.c).angle(), arc.a0, arc.a1))
                .collect()
        }
        (Seg::Arc(x), Seg::Arc(y)) => circle_circle(x.c, x.r, y.c, y.r)
            .into_iter()
            .filter(|p| {
                angle_in_arc((*p - x.c).angle(), x.a0, x.a1)
                    && angle_in_arc((*p - y.c).angle(), y.a0, y.a1)
            })
            .collect(),
    }
}

/// Alle Schnittpunkte zweier Polylinien.
pub fn polyline_polyline(a: &Polyline, b: &Polyline, tol: f64) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    let sa = a.segments();
    let sb = b.segments();
    for x in &sa {
        for y in &sb {
            for p in seg_seg(x, y) {
                if !out.iter().any(|q| q.dist(p) < tol) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Abstand Punkt → Polylinie.
pub fn polyline_distance(p: Vec2, pl: &Polyline) -> f64 {
    pl.segments()
        .iter()
        .map(|s| s.closest(p).1)
        .fold(f64::INFINITY, f64::min)
}

/// Fläche zwischen zwei Punkten auf einer Linie (für Wandöffnungen).
pub struct Opening {
    pub center: Vec2,
    pub width: f64,
}

// ---------------------------------------------------------------------------
// Text-Ausrichtung
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum VAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_line() {
        let p = line_line(v(0., 0.), v(10., 0.), v(5., -5.), v(5., 5.), false, false).unwrap();
        assert!((p.x - 5.0).abs() < 1e-9 && p.y.abs() < 1e-9);
        assert!(line_line(v(0., 0.), v(1., 0.), v(2., -5.), v(2., 5.), false, false).is_none());
    }

    #[test]
    fn test_arc_from_bulge() {
        let a = arc_from_bulge(v(0., 0.), v(10., 0.), 1.0).unwrap();
        assert!((a.r - 5.0).abs() < 1e-6);
        assert!((a.c.x - 5.0).abs() < 1e-6);
        assert!(a.c.y.abs() < 1e-6);
        assert!((a.sweep() - PI).abs() < 1e-6);
    }

    #[test]
    fn test_offset_square() {
        // Konvention: positiver Offset liegt links der Laufrichtung
        // (bei gegen den Uhrzeigersinn orientierten Polygonen also innen).
        let pts = vec![v(0., 0.), v(10., 0.), v(10., 10.), v(0., 10.)];
        let off = offset_polyline(&pts, true, 1.0);
        assert!((off[0].x - 1.0).abs() < 1e-6 && (off[0].y - 1.0).abs() < 1e-6, "{:?}", off[0]);
        let out = offset_polyline(&pts, true, -1.0);
        assert!((out[0].x + 1.0).abs() < 1e-6, "{:?}", out[0]);
    }

    #[test]
    fn test_clip_polygon() {
        let poly = vec![v(0., 0.), v(10., 0.), v(10., 10.), v(0., 10.)];
        let segs = clip_line_polygon(v(-5., 5.), v(15., 5.), &poly);
        assert_eq!(segs.len(), 1);
        assert!((segs[0].0.x - 0.0).abs() < 1e-9);
        assert!((segs[0].1.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_mirror() {
        // Spiegelung an der X-Achse
        let m = Affine::mirror(v(0., 0.), v(1., 0.));
        let p = m.apply(v(2., 3.));
        assert!((p.x - 2.0).abs() < 1e-9 && (p.y + 3.0).abs() < 1e-9, "{p:?}");
        // Spiegelung an einer 45°-Linie (y = x)
        let m = Affine::mirror(v(0., 0.), v(1., 1.));
        let p = m.apply(v(2., 3.));
        assert!((p.x - 3.0).abs() < 1e-9 && (p.y - 2.0).abs() < 1e-9, "{p:?}");
        // Spiegelung an einer senkrechten Achse
        let m = Affine::mirror(v(1., 0.), v(1., 5.));
        let p = m.apply(v(3., 2.));
        assert!((p.x + 1.0).abs() < 1e-9 && (p.y - 2.0).abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn test_affine_inverse() {
        let m = Affine::translate(v(10., 3.)) * Affine::rotate(0.7) * Affine::scale(2.0, 3.0);
        let p = v(4.0, -1.0);
        let q = m.apply(p);
        let back = m.inverse().apply(q);
        assert!(back.dist(p) < 1e-9);
    }
}
