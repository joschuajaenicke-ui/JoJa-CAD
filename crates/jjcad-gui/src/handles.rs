//! Direktes Bearbeiten über Griffe: Endpunkte, Ecken und Parallelverschiebung.

use crate::app::App;
use jjcad_core::geom::{Ellipse, Vec2 as V2};
use jjcad_core::model::{Entity, EntityKind, Id};

/// Art eines Griffpunkts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GripKind {
    /// Eckpunkt / Endpunkt (Index in der Punktliste bzw. 0 = Start, 1 = Ende)
    Vert(usize),
    /// Mitte einer Kante eines geschlossenen Polygons (Parallelverschiebung)
    Edge(usize),
    /// Startpunkt eines Bogens: zieht den Startwinkel um den Mittelpunkt.
    ArcStart,
    /// Endpunkt eines Bogens: zieht den Endwinkel um den Mittelpunkt.
    ArcEnd,
    /// Textfeld eines Raumstempels: verschiebt **nur** die Beschriftung,
    /// die Umgrenzung bleibt stehen (T0097).
    Label,
}

#[derive(Clone, Debug)]
pub struct GripDrag {
    pub entity: Id,
    pub kind: GripKind,
    pub orig: Entity,
}

/// Kleinster Knick, ab dem ein Randpunkt der Füllfläche als **Ecke** gilt
/// (1°). Der Farbeimer liefert seine Umgrenzung als dichten Punktzug entlang
/// der Wandkanten (gemessen: 125 Punkte für einen Raum); ohne diese Prüfung
/// läge auf jedem Zwischenpunkt ein Griff und die Fläche wäre nicht mehr zu
/// fassen.
const FILL_ECKE_MIN_WINKEL: f64 = 0.017_453_3;

/// Abstand, unter dem zwei Ecken als **dieselbe Stelle** gelten (mm). Der Fang
/// setzt den gezogenen Punkt exakt auf den Zielpunkt, deshalb genügt ein
/// Bruchteil eines Millimeters.
const FANG_SPERRE_EPS: f64 = 1e-6;

/// Knickt der Rand an `i`? Zwischenpunkte auf einer geraden Kante bekommen
/// keinen Griff — nur die Ecken.
fn ist_ecke(ring: &[V2], i: usize) -> bool {
    let n = ring.len();
    if n < 3 {
        return true;
    }
    let p = ring[i];
    let davor = p - ring[(i + n - 1) % n];
    let danach = ring[(i + 1) % n] - p;
    let (l1, l2) = (davor.len(), danach.len());
    if l1 < 1e-9 || l2 < 1e-9 {
        return false;
    }
    // Kreuzprodukt = Sinus, Punktprodukt = Cosinus des Knickwinkels.
    let kreuz = davor.x * danach.y - davor.y * danach.x;
    let punkt = davor.dot(danach);
    kreuz.atan2(punkt).abs() > FILL_ECKE_MIN_WINKEL
}

/// Zwei **benachbarte** Randpunkte übereinandergelegt werden zu einem Punkt:
/// der gezogene fällt weg, die Zahl der Umgrenzungspunkte sinkt um eins.
///
/// Nur Nachbarn — bei nicht benachbarten Punkten verschwände ein ganzes Stück
/// Umgrenzung mit. Ein Ring behält mindestens drei Punkte, sonst bleibt keine
/// Fläche übrig. Liefert `true`, wenn verschmolzen wurde.
fn verschmelze_nachbar(ring: &mut Vec<V2>, i: usize, tol: f64) -> bool {
    let n = ring.len();
    if n <= 3 || i >= n {
        return false;
    }
    let p = ring[i];
    let davor = ring[(i + n - 1) % n];
    let danach = ring[(i + 1) % n];
    if p.dist(davor) <= tol || p.dist(danach) <= tol {
        ring.remove(i);
        return true;
    }
    false
}

/// Ring einer Füllfläche zum flachen Eckindex `i` (Umgrenzung zuerst, dann die
/// Inseln) samt lokalem Index im Ring.
fn fill_ring_mut(kind: &mut EntityKind, i: usize) -> Option<(&mut Vec<V2>, usize)> {
    let (boundary, holes) = match kind {
        EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. } => (boundary, holes),
        _ => return None,
    };
    let mut base = 0usize;
    for ring in std::iter::once(&mut *boundary).chain(holes.iter_mut()) {
        let n = ring.len();
        if i < base + n {
            return Some((ring, i - base));
        }
        base += n;
    }
    None
}

/// Ecken des Treppenrechtecks in Anzeigereihenfolge, dazu Richtung, Normale
/// und halbe Breite.
///
/// Die Treppe ist von der Kontur her ein **Rechteck**: `a`→`b` ist die
/// Lauflinie (Mitte), die Breite steht senkrecht darauf. Die vier Kanten sind
/// damit festgelegt — Kante 0 = linke Längskante, 1 = Endkante, 2 = rechte
/// Längskante, 3 = Startkante.
fn stair_rechteck(a: V2, b: V2, width: f64) -> ([V2; 4], V2, V2, f64) {
    let dir = if a.dist(b) > 1e-9 {
        (b - a).normalized()
    } else {
        V2::new(1.0, 0.0)
    };
    let nrm = dir.perp();
    let hw = width * 0.5;
    (
        [a + nrm * hw, b + nrm * hw, b - nrm * hw, a - nrm * hw],
        dir,
        nrm,
        hw,
    )
}

/// Griffe einer Bemaßung: ein Quadrat je Basispunkt, eine Raute an der Mitte
/// der Maßlinie.
///
/// Die Raute ([`GripKind::Edge(0)`]) zieht die Maßlinie **senkrecht** zu ihrer
/// Richtung — die Basispunkte bleiben stehen. Ohne sie ließe sich die Lage der
/// Bemaßung nur über das Eigenschaften-Panel ändern (T0055).
fn dim_griffe(e: &Entity) -> Vec<(GripKind, V2)> {
    let mut out: Vec<(GripKind, V2)> = match &e.kind {
        EntityKind::Dim { a, b, .. } => vec![(GripKind::Vert(0), *a), (GripKind::Vert(1), *b)],
        EntityKind::DimChain { pts, .. } => pts
            .iter()
            .enumerate()
            .map(|(i, p)| (GripKind::Vert(i), *p))
            .collect(),
        _ => return Vec::new(),
    };
    if let Some((s, t)) = jjcad_core::render::dim_line_span(&e.kind) {
        out.push((GripKind::Edge(0), (s + t) * 0.5));
    }
    out
}

/// Alle Griffpunkte eines Elements mit ihrer Weltposition.
pub fn grips_of(e: &Entity) -> Vec<(GripKind, V2)> {
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
            vec![(GripKind::Vert(0), *a), (GripKind::Vert(1), *b)]
        }
        EntityKind::Polyline { verts, closed } | EntityKind::Spline { verts, closed } => {
            let mut out: Vec<(GripKind, V2)> = verts
                .iter()
                .enumerate()
                .map(|(i, v)| (GripKind::Vert(i), v.p))
                .collect();
            if *closed && verts.len() >= 3 {
                for i in 0..verts.len() {
                    let p1 = verts[i].p;
                    let p2 = verts[(i + 1) % verts.len()].p;
                    out.push((GripKind::Edge(i), (p1 + p2) * 0.5));
                }
            }
            out
        }
        EntityKind::Circle { c, .. }
        | EntityKind::Ellipse { c, .. }
        | EntityKind::Point { p: c } => vec![(GripKind::Vert(0), *c)],
        // Grundriss-Kote: das Symbol sitzt am Kreismittelpunkt.
        EntityKind::ElevationPlan { p, .. } => vec![(GripKind::Vert(0), *p)],
        // Schnitt-Kote: jeder Punkt ist ein Griff; Ziehen ändert die Höhe
        // (Y-Differenz zum ersten Punkt).
        EntityKind::ElevationSection { pts } => pts
            .iter()
            .enumerate()
            .map(|(i, p)| (GripKind::Vert(i), *p))
            .collect(),
        // Bogen: Mittelpunkt (verschiebt den Bogen) und beide Enden. Ein
        // Endpunkt läuft auf dem Kreis mit, gezogen wird der Winkel; der
        // Radius bleibt dabei stehen (T0022).
        EntityKind::Arc { c, r, a0, a1 } => vec![
            (GripKind::Vert(0), *c),
            (GripKind::ArcStart, *c + V2::polar(*a0, *r)),
            (GripKind::ArcEnd, *c + V2::polar(*a1, *r)),
        ],
        // Ellipsenbogen: Mittelpunkt (verschiebt) und beide Enden. Ein Ende
        // läuft auf der Ellipse mit, gezogen wird der Parameterwinkel; Achsen
        // und Drehlage bleiben stehen (T0084).
        EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = Ellipse::new(*c, *rx, *ry, *rot);
            vec![
                (GripKind::Vert(0), *c),
                (GripKind::ArcStart, el.point_at(*a0)),
                (GripKind::ArcEnd, el.point_at(*a1)),
            ]
        }
        // Treppe: die vier Kantenmitten des Rechtecks. Ziehen verschiebt genau
        // diese Kante parallel — die gegenüberliegende bleibt stehen, Breite
        // bzw. Länge ändern sich (T0027).
        EntityKind::Stair { a, b, width, .. } => {
            let (ecken, _, _, _) = stair_rechteck(*a, *b, *width);
            (0..4)
                .map(|i| {
                    let p = (ecken[i] + ecken[(i + 1) % 4]) * 0.5;
                    (GripKind::Edge(i), p)
                })
                .collect()
        }
        // Bemaßung: jeder Basispunkt ist ein Griff, dazu die Mitte der
        // Maßlinie (Raute, `GripKind::Edge(0)`) — Ziehen verschiebt sie
        // senkrecht zur Messrichtung (T0055).
        EntityKind::Dim { .. } | EntityKind::DimChain { .. } => dim_griffe(e),
        // Raumstempel: die Ecken der (unsichtbaren) Umgrenzung erscheinen erst
        // mit der Auswahl des Textfeldes — genau dann ist das Objekt gewählt.
        // Der runde Griff am Textfeld verschiebt nur die Beschriftung.
        EntityKind::Room { boundary, label, .. } => {
            let mut out: Vec<(GripKind, V2)> = boundary
                .iter()
                .enumerate()
                .map(|(i, p)| (GripKind::Vert(i), *p))
                .collect();
            out.push((GripKind::Label, *label));
            out
        }
        // Füllfläche: die Ecken der Umgrenzung und der Inseln. Zwischenpunkte
        // einer geraden Kante fallen weg (`ist_ecke`) — der Farbeimer liefert
        // seinen Rand als dichten Punktzug.
        EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. } => {
            let mut out: Vec<(GripKind, V2)> = Vec::new();
            let mut base = 0usize;
            for ring in std::iter::once(boundary).chain(holes.iter()) {
                for (i, p) in ring.iter().enumerate() {
                    if ist_ecke(ring, i) {
                        out.push((GripKind::Vert(base + i), *p));
                    }
                }
                base += ring.len();
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Wendet eine Griffbewegung auf eine Kopie des Elements an.
pub fn apply_grip(orig: &Entity, kind: GripKind, target: V2) -> Option<Entity> {
    apply_grip_tol(orig, kind, target, jjcad_core::model::POINT_EPS_MM)
}

/// Wie [`apply_grip`], aber mit der Toleranz des laufenden Zugs (Bildschirm-px
/// in Welt-mm). Ein offener Polygonzug wird geschlossen, sobald ein Endpunkt
/// auf den anderen gezogen wird: der doppelte Stützpunkt fällt weg, `closed`
/// wird gesetzt — dieselbe Regel wie beim Setzen des letzten Punktes
/// (`model::polyline_points`).
pub fn apply_grip_tol(orig: &Entity, kind: GripKind, target: V2, tol: f64) -> Option<Entity> {
    let mut e = orig.clone();
    // Maßlinie einer Bemaßung: eigener Weg, weil die Bewegung nur senkrecht zur
    // Messrichtung läuft und die Rechnung im Kern steht
    // (`render::dim_line_move`) — so liegt der Griff auf der gezeichneten Linie.
    if kind == GripKind::Edge(0)
        && matches!(
            orig.kind,
            EntityKind::Dim { .. } | EntityKind::DimChain { .. }
        )
    {
        return if jjcad_core::render::dim_line_move(&mut e.kind, target) {
            Some(e)
        } else {
            None
        };
    }
    match (&mut e.kind, kind) {
        // Bemaßung: Basispunkte ziehen, die Maßlinie bleibt an ihrer Höhe.
        (EntityKind::Dim { a, .. }, GripKind::Vert(0)) => *a = target,
        (EntityKind::Dim { b, .. }, GripKind::Vert(1)) => *b = target,
        (EntityKind::DimChain { pts, .. }, GripKind::Vert(i)) => {
            *pts.get_mut(i)? = target;
        }
        (EntityKind::ElevationPlan { p, .. }, GripKind::Vert(_)) => *p = target,
        (EntityKind::ElevationSection { pts }, GripKind::Vert(i)) => {
            *pts.get_mut(i)? = target;
        }
        (EntityKind::Line { a, .. }, GripKind::Vert(0)) => *a = target,
        (EntityKind::Line { b, .. }, GripKind::Vert(1)) => *b = target,
        (EntityKind::Wall { a, .. }, GripKind::Vert(0)) => *a = target,
        (EntityKind::Wall { b, .. }, GripKind::Vert(1)) => *b = target,
        (EntityKind::Point { p }, GripKind::Vert(_)) => *p = target,
        (EntityKind::Circle { c, .. }, GripKind::Vert(_)) => *c = target,
        (EntityKind::Arc { c, .. }, GripKind::Vert(_)) => *c = target,
        // Endpunkt eines Bogens: der Punkt folgt dem Zeiger auf dem Kreis,
        // gesetzt wird der Winkel — der Radius bleibt unverändert.
        (EntityKind::Arc { c, a0, .. }, GripKind::ArcStart)
        | (EntityKind::Arc { c, a1: a0, .. }, GripKind::ArcEnd) => {
            let d = target - *c;
            if d.len() < 1e-6 {
                return None;
            }
            *a0 = d.angle();
        }
        // Ellipsenbogen-Ende: der Punkt folgt dem Zeiger, gesetzt wird der
        // Parameterwinkel (`Ellipse::param_of`); Achsen bleiben unverändert.
        (EntityKind::EllipseArc { c, rx, ry, rot, a0, .. }, GripKind::ArcStart)
        | (EntityKind::EllipseArc { c, rx, ry, rot, a1: a0, .. }, GripKind::ArcEnd) => {
            *a0 = Ellipse::new(*c, *rx, *ry, *rot).param_of(target);
        }
        (EntityKind::EllipseArc { c, .. }, GripKind::Vert(_)) => *c = target,
        (EntityKind::Ellipse { c, .. }, GripKind::Vert(_)) => *c = target,
        (EntityKind::Polyline { verts, closed } | EntityKind::Spline { verts, closed }, GripKind::Vert(i)) => {
            let n = verts.len();
            verts.get_mut(i)?.p = target;
            // Endpunkt auf den anderen gezogen: der Zug ist geschlossen. Erst ab
            // vier Stützpunkten bleibt nach dem Wegfall des doppelten Punktes
            // noch eine Fläche (drei Ecken) übrig.
            if !*closed && n >= 4 && (i == n - 1 || i == 0) {
                let start = verts[0].p;
                let ende = verts[n - 1].p;
                if start.dist(ende) <= tol {
                    if i == 0 {
                        verts[0].p = ende;
                    }
                    verts.pop();
                    *closed = true;
                }
            }
        }
        (EntityKind::Polyline { verts, closed }, GripKind::Edge(i)) if *closed => {
            let n = verts.len();
            if n < 3 || i >= n {
                return None;
            }
            let j = (i + 1) % n;
            let p1 = verts[i].p;
            let p2 = verts[j].p;
            let seg = p2 - p1;
            let len = seg.len();
            if len < 1e-6 {
                return None;
            }
            // Verschiebung senkrecht zur Kante: die Nachbarkanten folgen automatisch,
            // weil sie sich dieselben Eckpunkte teilen.
            let normal = V2::new(-seg.y, seg.x) / len;
            let mid = (p1 + p2) * 0.5;
            let d = (target - mid).dot(normal);
            verts[i].p = p1 + normal * d;
            verts[j].p = p2 + normal * d;
        }
        // Treppenkante parallel verschieben. Die gegenüberliegende Kante bleibt
        // stehen: Breite und Lage der Lauflinie folgen der halben Verschiebung.
        (EntityKind::Stair { a, b, width, .. }, GripKind::Edge(i)) => {
            let (_, dir, nrm, hw) = stair_rechteck(*a, *b, *width);
            let mitte = (*a + *b) * 0.5;
            const MIN: f64 = 100.0;
            match i {
                0 | 2 => {
                    let kante = if i == 0 {
                        mitte + nrm * hw
                    } else {
                        mitte - nrm * hw
                    };
                    let mut d = (target - kante).dot(nrm);
                    if i == 0 {
                        d = d.max(MIN - *width);
                        *width += d;
                    } else {
                        d = d.min(*width - MIN);
                        *width -= d;
                    }
                    *a = *a + nrm * (d * 0.5);
                    *b = *b + nrm * (d * 0.5);
                }
                1 => {
                    let t = (target - *b).dot(dir);
                    if (*b + dir * t).dist(*a) < MIN {
                        return None;
                    }
                    *b = *b + dir * t;
                }
                3 => {
                    let t = (target - *a).dot(dir);
                    if (*a + dir * t).dist(*b) < MIN {
                        return None;
                    }
                    *a = *a + dir * t;
                }
                _ => return None,
            }
        }
        // Raumstempel: eine Ecke ändert die Fläche (der Text rechnet sich neu),
        // der Textgriff verschiebt nur die Beschriftung — die Fläche bleibt.
        (EntityKind::Room { boundary, .. }, GripKind::Vert(i)) => {
            *boundary.get_mut(i)? = target;
        }
        (EntityKind::Room { label, .. }, GripKind::Label) => *label = target,
        // Ecke einer Füllfläche: der flache Index zählt erst die Umgrenzung,
        // dann die Inseln (dieselbe Reihenfolge wie die Anzeige). Verschmolzen
        // wird erst beim Loslassen (`App::end_grip_drag`) — während des Zugs
        // bleiben die Indizes dadurch gültig.
        (EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. }, GripKind::Vert(i)) => {
            let mut base = 0usize;
            let mut ziel: Option<&mut Vec<V2>> = None;
            for ring in std::iter::once(&mut *boundary).chain(holes.iter_mut()) {
                let n = ring.len();
                if i < base + n {
                    ziel = Some(ring);
                    break;
                }
                base += n;
            }
            let ring = ziel?;
            ring[i - base] = target;
        }
        _ => return None,
    }
    Some(e)
}

/// Punkte, die eine Griffbewegung **verschiebt** — in der aktuellen Lage des
/// Elements (während des Zugs ist das schon die bewegte Lage).
///
/// Der Punktfang sperrt genau diese Punkte, solange gezogen wird: der gezogene
/// Stützpunkt liegt dauerhaft im Fangradius des Zeigers und würde sich sonst
/// selbst fangen — der Griff bliebe stehen, statt der Maus zu folgen. Die
/// *ruhenden* Punkte desselben Elements (Nachbarecken) bleiben fangbar.
pub fn moving_points(e: &Entity, kind: GripKind) -> Vec<V2> {
    match (&e.kind, kind) {
        (EntityKind::Line { a, .. }, GripKind::Vert(0))
        | (EntityKind::Wall { a, .. }, GripKind::Vert(0)) => vec![*a],
        (EntityKind::Line { b, .. }, GripKind::Vert(1))
        | (EntityKind::Wall { b, .. }, GripKind::Vert(1)) => vec![*b],
        // Raumstempel: die gezogene Ecke bzw. das gezogene Textfeld wandert.
        (EntityKind::Room { boundary, .. }, GripKind::Vert(i)) => {
            boundary.get(i).map(|p| vec![*p]).unwrap_or_default()
        }
        (EntityKind::Room { label, .. }, GripKind::Label) => vec![*label],
        // Bemaßung: der gezogene Basispunkt wandert, die übrigen stehen.
        (EntityKind::Dim { a, .. }, GripKind::Vert(0)) => vec![*a],
        (EntityKind::Dim { b, .. }, GripKind::Vert(1)) => vec![*b],
        (EntityKind::DimChain { pts, .. }, GripKind::Vert(i)) => {
            pts.get(i).map(|p| vec![*p]).unwrap_or_default()
        }
        (EntityKind::ElevationPlan { p, .. }, GripKind::Vert(_)) => vec![*p],
        (EntityKind::ElevationSection { pts }, GripKind::Vert(i)) => {
            pts.get(i).map(|p| vec![*p]).unwrap_or_default()
        }
        // Maßlinie: beide Enden wandern mit (senkrecht zur Messrichtung).
        (EntityKind::Dim { .. } | EntityKind::DimChain { .. }, GripKind::Edge(0)) => {
            match jjcad_core::render::dim_line_span(&e.kind) {
                Some((s, t)) => vec![s, t],
                None => Vec::new(),
            }
        }
        (EntityKind::Polyline { verts, .. } | EntityKind::Spline { verts, .. }, GripKind::Vert(i)) => {
            verts.get(i).map(|v| vec![v.p]).unwrap_or_default()
        }
        (EntityKind::Polyline { verts, closed }, GripKind::Edge(i)) if *closed => {
            let n = verts.len();
            if n < 3 || i >= n {
                return Vec::new();
            }
            let j = (i + 1) % n;
            vec![verts[i].p, verts[j].p]
        }
        // Treppenkante: beide Enden der Kante wandern mit.
        (EntityKind::Stair { a, b, width, .. }, GripKind::Edge(i)) => {
            let (ecken, _, _, _) = stair_rechteck(*a, *b, *width);
            if i >= 4 {
                return Vec::new();
            }
            vec![ecken[i], ecken[(i + 1) % 4]]
        }
        // Füllfläche: flacher Eckindex, Umgrenzung zuerst.
        (EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. }, GripKind::Vert(i)) => {
            let mut base = 0usize;
            for ring in std::iter::once(boundary).chain(holes.iter()) {
                let n = ring.len();
                if i < base + n {
                    return ring.get(i - base).map(|p| vec![*p]).unwrap_or_default();
                }
                base += n;
            }
            Vec::new()
        }
        // Kreis, Bogen, Ellipse und Punkt bewegen ihren Mittelpunkt.
        (
            EntityKind::Circle { c, .. }
            | EntityKind::Arc { c, .. }
            | EntityKind::Ellipse { c, .. }
            | EntityKind::EllipseArc { c, .. }
            | EntityKind::Point { p: c },
            GripKind::Vert(_),
        ) => vec![*c],
        (EntityKind::EllipseArc { c, rx, ry, rot, a0, .. }, GripKind::ArcStart) => {
            vec![Ellipse::new(*c, *rx, *ry, *rot).point_at(*a0)]
        }
        (EntityKind::EllipseArc { c, rx, ry, rot, a1, .. }, GripKind::ArcEnd) => {
            vec![Ellipse::new(*c, *rx, *ry, *rot).point_at(*a1)]
        }
        // Bogenenden: der Punkt selbst wandert, der Mittelpunkt bleibt.
        (EntityKind::Arc { c, r, a0, .. }, GripKind::ArcStart) => vec![*c + V2::polar(*a0, *r)],
        (EntityKind::Arc { c, r, a1, .. }, GripKind::ArcEnd) => vec![*c + V2::polar(*a1, *r)],
        _ => Vec::new(),
    }
}

/// Fang-Sperre für den laufenden Griffzug.
///
/// Wie [`moving_points`], aber ein Punkt, auf dem eine **zweite Ecke desselben
/// Elements** liegt, wird nicht gesperrt: dort ist der Fang das Ziel, kein
/// Selbst-Fang. Ohne diese Ausnahme verschwindet der Fang genau in dem Moment,
/// in dem er den Punkt auf die Nachbarecke gezogen hat — die Sperre liegt dann
/// auf derselben Stelle wie das Ziel, der Zeiger springt zurück zum Mauszeiger,
/// im nächsten Frame ist die Ecke wieder frei und der Fang zieht erneut:
/// Vorschau und Fangmarke flackern (T0018, Turn 3).
pub fn fang_sperre(e: &Entity, kind: GripKind) -> Vec<V2> {
    let punkte = moving_points(e, kind);
    if punkte.is_empty() {
        return punkte;
    }
    let eigene = eigene_fangpunkte(e);
    punkte
        .into_iter()
        .filter(|p| eigene.iter().filter(|q| q.dist(*p) <= FANG_SPERRE_EPS).count() < 2)
        .collect()
}

/// **Erster** Bezugspunkt für Ortho und Polar beim Ziehen eines Griffs.
///
/// Beim Erstellen ist der Bezug der zuletzt gesetzte Punkt, deshalb rastet die
/// neue Strecke in Waagerechte, Senkrechte und 45° ein. Beim Ziehen an einem
/// Griff fehlte dieser Bezug ganz — der Griff folgte dem Zeiger ohne
/// Zwangsrichtung, und eine vorhandene Linie ließ sich nicht mehr klar
/// ausrichten (T0046). Bezug ist der **ruhende** Punkt, an dem die Richtung
/// gemessen wird; an einem Stützpunkt sind das **zwei** (voriger und nächster)
/// — siehe [`bezugspunkte`].
///
/// Kantenmitten (parallel verschieben), Mittelpunkte, Bogenenden und die
/// Treppenkanten haben keinen Bezug: ihre Bewegung ist bereits eindimensional
/// bzw. richtungslos.
pub fn bezugspunkt(e: &Entity, kind: GripKind) -> Option<V2> {
    bezugspunkte(e, kind).into_iter().next()
}

/// **Alle** Bezugspunkte eines gezogenen Griffs — in der Regel zwei.
///
/// Ein Stützpunkt einer Polylinie, eines Splines, eines Rechtecks oder einer
/// Fläche hängt an **zwei** Linien (zum vorigen und zum nächsten Punkt). Beim
/// Ausrichten müssen beide gleichzeitig zur Verfügung stehen, sonst lässt sich
/// immer nur eine von beiden in die Waagerechte/Senkrechte ziehen (T0046,
/// Turn 3). Reihenfolge: voriger Punkt zuerst, dann der nächste — der erste
/// ist der Bezug für den Polarfang und für Einzelbezüge.
///
/// Ein Endpunkt einer offenen Polylinie hat nur einen Nachbarn, Linie und Wand
/// genau einen (das andere Ende), Kantenmitten und Mittelpunkte keinen.
pub fn bezugspunkte(e: &Entity, kind: GripKind) -> Vec<V2> {
    match (&e.kind, kind) {
        (EntityKind::Line { b, .. } | EntityKind::Wall { b, .. }, GripKind::Vert(0)) => vec![*b],
        (EntityKind::Line { a, .. } | EntityKind::Wall { a, .. }, GripKind::Vert(1)) => vec![*a],
        // Bemaßung: der Bezug für Ortho/Polar ist der Nachbarpunkt der Kette.
        (EntityKind::Dim { b, .. }, GripKind::Vert(0)) => vec![*b],
        (EntityKind::Dim { a, .. }, GripKind::Vert(1)) => vec![*a],
        (EntityKind::DimChain { pts, .. }, GripKind::Vert(i)) => {
            let n = pts.len();
            if n < 2 || i >= n {
                return Vec::new();
            }
            let mut out = Vec::new();
            if i > 0 {
                out.push(pts[i - 1]);
            }
            if i + 1 < n {
                out.push(pts[i + 1]);
            }
            out
        }
        (
            EntityKind::Polyline { verts, closed } | EntityKind::Spline { verts, closed },
            GripKind::Vert(i),
        ) => {
            let n = verts.len();
            if n < 2 || i >= n {
                return Vec::new();
            }
            let mut out = Vec::new();
            if i > 0 {
                out.push(verts[i - 1].p);
            } else if *closed && n >= 3 {
                out.push(verts[n - 1].p);
            }
            if i + 1 < n {
                out.push(verts[i + 1].p);
            } else if *closed && n >= 2 {
                out.push(verts[0].p);
            }
            out
        }
        // Raumstempel-Ecke: Bezug sind die beiden Nachbarecken im Ring.
        (EntityKind::Room { boundary, .. }, GripKind::Vert(i)) => {
            let n = boundary.len();
            if n < 2 || i >= n {
                return Vec::new();
            }
            let mut out = Vec::new();
            out.push(boundary[(i + n - 1) % n]);
            if n >= 3 {
                out.push(boundary[(i + 1) % n]);
            }
            out
        }
        (
            EntityKind::Fill { boundary, holes, .. }
            | EntityKind::Volume { boundary, holes, .. },
            GripKind::Vert(i),
        ) => {
            let mut base = 0usize;
            for ring in std::iter::once(boundary).chain(holes.iter()) {
                let n = ring.len();
                if i < base + n {
                    if n < 2 {
                        return Vec::new();
                    }
                    let k = i - base;
                    let mut out = Vec::new();
                    if n >= 2 {
                        out.push(ring[(k + n - 1) % n]);
                    }
                    if n >= 3 {
                        out.push(ring[(k + 1) % n]);
                    }
                    return out;
                }
                base += n;
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Fangpunkte, die das Element selbst anbietet: Ecken und Kantenmitten.
///
/// Liegt der gezogene Punkt auf einem davon, ist er das *Ziel* des Fangs und
/// darf nicht gesperrt werden — sonst fiele der Fang im nächsten Frame aus und
/// flackerte (siehe [`fang_sperre`]).
fn eigene_fangpunkte(e: &Entity) -> Vec<V2> {
    let mut out = eckpunkte(e);
    out.extend(kantenmitten(e));
    out
}

/// Kantenmitten eines Elements — die Punkte, die der Fang als „Mitte" (MIT)
/// anbietet.
fn kantenmitten(e: &Entity) -> Vec<V2> {
    let mitte = |a: V2, b: V2| (a + b) * 0.5;
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => vec![mitte(*a, *b)],
        EntityKind::Polyline { verts, closed } => {
            let n = verts.len();
            let mut out: Vec<V2> = (0..n.saturating_sub(1))
                .map(|i| mitte(verts[i].p, verts[i + 1].p))
                .collect();
            if *closed && n >= 2 {
                out.push(mitte(verts[n - 1].p, verts[0].p));
            }
            out
        }
        EntityKind::Stair { a, b, width, .. } => {
            let (ecken, _, _, _) = stair_rechteck(*a, *b, *width);
            (0..4).map(|i| mitte(ecken[i], ecken[(i + 1) % 4])).collect()
        }
        // Mitte der Maßlinie: dort sitzt der Griff, also ist sie auch ein
        // Fangpunkt des eigenen Elements.
        EntityKind::Dim { .. } | EntityKind::DimChain { .. } => {
            jjcad_core::render::dim_line_span(&e.kind)
                .map(|(s, t)| vec![mitte(s, t)])
                .unwrap_or_default()
        }
        EntityKind::Fill { boundary, holes, .. } => {
            let mut out = Vec::new();
            for ring in std::iter::once(boundary).chain(holes.iter()) {
                let n = ring.len();
                if n >= 2 {
                    for i in 0..n {
                        out.push(mitte(ring[i], ring[(i + 1) % n]));
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Alle Eckpunkte eines Elements — **ungefiltert**: auch Zwischenpunkte einer
/// geraden Kante und zwei Punkte, die aufeinander liegen.
///
/// [`grips_of`] blendet genau diese aus (kein Griff auf einem Zwischenpunkt,
/// und bei zwei aufeinanderliegenden Ecken knickt der Rand an keiner von
/// beiden). Für die Fang-Sperre zählt aber die Frage „liegt hier noch ein
/// zweiter Punkt desselben Elements?" — dafür braucht es die rohe Liste.
fn eckpunkte(e: &Entity) -> Vec<V2> {
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => vec![*a, *b],
        EntityKind::Polyline { verts, .. } | EntityKind::Spline { verts, .. } => {
            verts.iter().map(|v| v.p).collect()
        }
        EntityKind::Circle { c, .. }
        | EntityKind::Ellipse { c, .. }
        | EntityKind::Point { p: c } => vec![*c],
        // Bogen: Mittelpunkt und beide Enden. Die Enden sind eigene Fangziele;
        // ohne sie fiele der Fang beim Ziehen eines Endpunkts aus (Flackern).
        EntityKind::Arc { c, r, a0, a1 } => {
            vec![*c, *c + V2::polar(*a0, *r), *c + V2::polar(*a1, *r)]
        }
        EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = Ellipse::new(*c, *rx, *ry, *rot);
            vec![*c, el.point_at(*a0), el.point_at(*a1)]
        }
        EntityKind::Stair { a, b, width, .. } => {
            stair_rechteck(*a, *b, *width).0.to_vec()
        }
        // Basispunkte der Bemaßung: der Fang darf sie beim Ziehen nicht sperren
        // und beim Fangen nicht doppelt anbieten (T0055).
        EntityKind::Dim { a, b, .. } => vec![*a, *b],
        EntityKind::DimChain { pts, .. } => pts.clone(),
        EntityKind::Fill { boundary, holes, .. } => boundary
            .iter()
            .chain(holes.iter().flatten())
            .copied()
            .collect(),
        EntityKind::Room { boundary, label, .. } => {
            let mut out: Vec<V2> = boundary.clone();
            out.push(*label);
            out
        }
        _ => Vec::new(),
    }
}

/// Richtung einer Kante (normiert): Startpunkt und Einheitsrichtung.
///
/// Gilt für die Kanten einer geschlossenen Polylinie **und** für die vier
/// Rechteckkanten einer Treppe — `canvas::draw_grips` zeichnet den Griff quer
/// zur Kante, ohne Richtung gäbe es keinen Griff zu sehen (T0027, Turn 3).
pub fn edge_dir(e: &Entity, i: usize) -> Option<(V2, V2)> {
    if let EntityKind::Stair { a, b, width, .. } = &e.kind {
        let (ecken, _, _, _) = stair_rechteck(*a, *b, *width);
        if i >= 4 {
            return None;
        }
        let p = ecken[i];
        let q = ecken[(i + 1) % 4];
        let d = q - p;
        let l = d.len();
        if l < 1e-9 {
            return None;
        }
        return Some((p, d / l));
    }
    // Maßlinie: Richtung der gezeichneten Linie — der Griff steht quer dazu
    // und wandert senkrecht (T0055).
    if matches!(e.kind, EntityKind::Dim { .. } | EntityKind::DimChain { .. }) {
        let (s, t) = jjcad_core::render::dim_line_span(&e.kind)?;
        let d = t - s;
        let l = d.len();
        if l < 1e-9 {
            return None;
        }
        return Some((s, d / l));
    }
    if let EntityKind::Polyline { verts, closed } = &e.kind {
        let n = verts.len();
        if n < 2 || i >= n {
            return None;
        }
        let j = if i + 1 < n { i + 1 } else if *closed { 0 } else { return None };
        let a = verts[i].p;
        let b = verts[j].p;
        let d = b - a;
        let l = d.len();
        if l < 1e-9 { return None }
        Some((a, d / l))
    } else {
        None
    }
}

impl App {
    /// Griff unter dem Cursor (nur in der Auswahl).
    pub fn grip_at(&self, p: V2, tol: f64) -> Option<(Id, GripKind)> {
        for id in self.selection() {
            if let Some(e) = self.doc().entity(*id) {
                for (kind, gp) in grips_of(e) {
                    if p.dist(gp) <= tol {
                        return Some((*id, kind));
                    }
                }
            }
        }
        None
    }

    /// Beginnt das Ziehen an einem Griff (mit Undo-Schnappschuss).
    pub fn begin_grip_drag(&mut self, entity: Id, kind: GripKind) {
        let Some(orig) = self.doc().entity(entity).cloned() else {
            return;
        };
        let snap = self.doc().clone();
        self.session_mut().history.record(&snap);
        self.grip_drag = Some(GripDrag {
            entity,
            kind,
            orig,
        });
        self.mitteilung("Griff · ziehen");
    }

    /// Verschiebt den Griff auf die Weltposition `target`.
    pub fn update_grip_drag(&mut self, target: V2) -> bool {
        let Some(d) = self.grip_drag.clone() else {
            return false;
        };
        let Some(new_e) = apply_grip_tol(
            &d.orig,
            d.kind,
            target,
            (12.0 / self.current_view().zoom.max(1e-6)).max(jjcad_core::model::POINT_EPS_MM),
        ) else {

            return false;
        };
        if let Some(e) = self.doc_mut().entity_mut(d.entity) {
            *e = new_e;
            self.mark_changed();
            true
        } else {
            false
        }
    }

    /// Beendet den Griffzug. Liegt die gezogene Ecke einer Füllfläche auf einer
    /// ihrer Nachbarecken, werden beide zu **einem** Punkt: die Umgrenzung hat
    /// danach einen Punkt weniger.
    pub fn end_grip_drag(&mut self) {
        let Some(d) = self.grip_drag.take() else {
            return;
        };
        let mut verbunden = false;
        if let GripKind::Vert(i) = d.kind {
            let tol = (12.0 / self.current_view().zoom.max(1e-6))
                .max(jjcad_core::model::POINT_EPS_MM);
            if let Some(e) = self.doc_mut().entity_mut(d.entity) {
                if matches!(
                    e.kind,
                    EntityKind::Fill { .. } | EntityKind::Volume { .. }
                ) {
                    if let Some((ring, idx)) = fill_ring_mut(&mut e.kind, i) {
                        verbunden = verschmelze_nachbar(ring, idx, tol);
                        if verbunden {
                            self.mark_changed();
                        }
                    }
                }
            }
        }
        if verbunden {
            self.mitteilung("Füllfläche · Punkt verbunden");
        } else {
            self.mitteilung("Griff · gesetzt");
        }
    }

    pub fn cancel_grip_drag(&mut self) {
        if let Some(d) = self.grip_drag.take() {
            if let Some(e) = self.doc_mut().entity_mut(d.entity) {
                *e = d.orig;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jjcad_core::geom::Vertex;

    fn offen(pts: &[V2]) -> Entity {
        Entity::new(
            1,
            1,
            EntityKind::Polyline {
                verts: pts.iter().map(|p| Vertex::new(*p)).collect(),
                closed: false,
            },
        )
    }

    fn stand(e: &Entity) -> (Vec<V2>, bool) {
        match &e.kind {
            EntityKind::Polyline { verts, closed } => {
                (verts.iter().map(|v| v.p).collect(), *closed)
            }
            _ => panic!("keine Polylinie"),
        }
    }

    /// Letzten Stützpunkt auf den Startpunkt ziehen schließt den Zug: der
    /// doppelte Punkt fällt weg (drei Ecken bleiben), `closed` steht.
    #[test]
    fn griff_auf_startpunkt_schliesst_die_polylinie() {
        let orig = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(500.0, 1500.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(3), V2::new(0.0, 0.0), 1.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(closed, "Closed-Flag fehlt nach dem Griffzug");
        assert_eq!(pts.len(), 3, "der doppelte Stützpunkt bleibt stehen");
        assert_eq!(pts[0], V2::new(0.0, 0.0));
    }

    /// Auch der erste Stützpunkt darf auf den letzten gezogen werden; die
    /// Geometrie bleibt dabei erhalten (der letzte Punkt wird der neue erste).
    #[test]
    fn griff_des_startpunkts_auf_das_ende_schliesst() {
        let orig = offen(&[
            V2::new(500.0, 1500.0),
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(0), V2::new(2000.0, 1000.0), 1.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(closed);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0], V2::new(2000.0, 1000.0), "Endpunkt wird der neue Start");
    }

    /// Weit weg vom Gegenpunkt bleibt der Zug offen; ein mittlerer Stützpunkt
    /// schließt nie.
    #[test]
    fn griff_abseits_schliesst_nicht() {
        let orig = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(500.0, 1500.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(3), V2::new(0.0, 100.0), 1.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(!closed);
        assert_eq!(pts.len(), 4);

        let mitte = apply_grip_tol(&orig, GripKind::Vert(1), V2::new(0.0, 0.0), 1.0).unwrap();
        let (pts2, closed2) = stand(&mitte);
        assert!(!closed2, "ein mittlerer Stützpunkt schließt nichts");
        assert_eq!(pts2.len(), 4);
    }

    // ---- Füllflächen (T0018) ----

    fn flaeche(rand: &[V2], inseln: &[&[V2]]) -> Entity {
        Entity::new(
            7,
            1,
            EntityKind::Fill {
                boundary: rand.to_vec(),
                holes: inseln.iter().map(|h| h.to_vec()).collect(),
                pattern: jjcad_core::style::FillPattern::Lines,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: true,
            },
        )
    }

    fn raender(e: &Entity) -> (Vec<V2>, Vec<Vec<V2>>) {
        match &e.kind {
            EntityKind::Fill {
                boundary, holes, ..
            } => (boundary.clone(), holes.clone()),
            _ => panic!("keine Füllfläche"),
        }
    }

    fn quadrat() -> Entity {
        flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[],
        )
    }

    /// Vier Ecken — und nur dort Griffe, genau auf der Umgrenzung.
    #[test]
    fn fuellflaeche_hat_griffe_an_den_ecken() {
        let e = quadrat();
        let g = grips_of(&e);
        let ecken: Vec<V2> = g
            .iter()
            .filter(|(k, _)| matches!(k, GripKind::Vert(_)))
            .map(|(_, p)| *p)
            .collect();
        assert_eq!(ecken.len(), 4, "vier Eckgriffe: {g:?}");
        assert_eq!(ecken[0], V2::new(0.0, 0.0));
        assert_eq!(ecken[1], V2::new(4000.0, 0.0));
        assert_eq!(ecken[2], V2::new(4000.0, 3000.0));
        assert_eq!(ecken[3], V2::new(0.0, 3000.0));
    }

    /// Zwischenpunkte einer geraden Kante bekommen keinen Griff: der Farbeimer
    /// liefert seinen Rand als dichten Punktzug (ein Raum = 125 Punkte), sonst
    /// wäre die Fläche unter lauter Griffen nicht mehr zu treffen.
    #[test]
    fn gerade_kante_bekommt_keinen_griff() {
        let e = flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(1000.0, 0.0),
                V2::new(2000.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[],
        );
        let ecken: Vec<usize> = grips_of(&e)
            .into_iter()
            .filter_map(|(k, _)| match k {
                GripKind::Vert(i) => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(ecken, vec![0, 3, 4, 5], "nur die vier echten Ecken");
    }

    /// Ecke ziehen: nur dieser Punkt wandert, Muster und Umrandung bleiben.
    #[test]
    fn fuellflaeche_ecke_ziehen_aendert_nur_diesen_punkt() {
        let neu = apply_grip_tol(&quadrat(), GripKind::Vert(2), V2::new(5000.0, 3500.0), 1.0)
            .expect("Griff muss greifen");
        let (rand, inseln) = raender(&neu);
        assert_eq!(rand[2], V2::new(5000.0, 3500.0));
        assert_eq!(rand[0], V2::new(0.0, 0.0));
        assert_eq!(rand[1], V2::new(4000.0, 0.0));
        assert_eq!(rand[3], V2::new(0.0, 3000.0));
        assert!(inseln.is_empty());
        match &neu.kind {
            EntityKind::Fill {
                pattern,
                pattern_scale,
                border,
                ..
            } => {
                assert_eq!(*pattern, jjcad_core::style::FillPattern::Lines);
                assert_eq!(*pattern_scale, 100.0);
                assert!(*border);
            }
            _ => panic!("keine Füllfläche"),
        }
    }

    /// Inseln zählen hinter der Umgrenzung: ihr Eckgriff verschiebt nur die
    /// Insel, die Umgrenzung bleibt stehen.
    #[test]
    fn insel_ecke_ziehen_laesst_die_umgrenzung_stehen() {
        let e = flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[&[
                V2::new(1000.0, 1000.0),
                V2::new(1500.0, 1000.0),
                V2::new(1500.0, 1500.0),
            ]],
        );
        let g = grips_of(&e);
        // 4 Ecken der Umgrenzung, danach 3 Ecken der Insel (flach gezählt).
        assert_eq!(g.len(), 7, "Griffe: {g:?}");
        assert!(g.contains(&(GripKind::Vert(4), V2::new(1000.0, 1000.0))));
        assert!(g.contains(&(GripKind::Vert(6), V2::new(1500.0, 1500.0))));

        let neu = apply_grip_tol(&e, GripKind::Vert(5), V2::new(2000.0, 900.0), 1.0).unwrap();
        let (rand, inseln) = raender(&neu);
        assert_eq!(rand, raender(&e).0, "Umgrenzung bleibt unverändert");
        assert_eq!(inseln[0][1], V2::new(2000.0, 900.0));
        assert_eq!(inseln[0][0], V2::new(1000.0, 1000.0));
        assert_eq!(inseln[0][2], V2::new(1500.0, 1500.0));
    }

    /// Der gezogene Punkt ist der einzige, der aus dem Fang fällt.
    #[test]
    fn fuellflaeche_moving_points() {
        let e = quadrat();
        assert_eq!(moving_points(&e, GripKind::Vert(1)), vec![V2::new(4000.0, 0.0)]);
    }

    // ---- Bogen: Endpunkte ziehen (T0022) ----

    /// Bogen um (1000;1000) mit 500 mm von 0° bis 90°.
    fn bogen() -> Entity {
        Entity::new(
            1,
            1,
            EntityKind::Arc {
                c: V2::new(1000.0, 1000.0),
                r: 500.0,
                a0: 0.0,
                a1: std::f64::consts::FRAC_PI_2,
            },
        )
    }

    fn bogen_werte(e: &Entity) -> (V2, f64, f64, f64) {
        match &e.kind {
            EntityKind::Arc { c, r, a0, a1 } => (*c, *r, *a0, *a1),
            other => panic!("kein Bogen: {other:?}"),
        }
    }

    /// Drei Griffe: Mittelpunkt in der Mitte, je einer auf den beiden Enden.
    #[test]
    fn bogen_hat_griffe_am_mittelpunkt_und_an_den_enden() {
        let e = bogen();
        let g = grips_of(&e);
        assert_eq!(g.len(), 3, "Griffe: {g:?}");
        assert!(g.contains(&(GripKind::Vert(0), V2::new(1000.0, 1000.0))));
        let start = g
            .iter()
            .find(|(k, _)| *k == GripKind::ArcStart)
            .expect("Startgriff fehlt")
            .1;
        let ende = g
            .iter()
            .find(|(k, _)| *k == GripKind::ArcEnd)
            .expect("Endgriff fehlt")
            .1;
        assert!(start.dist(V2::new(1500.0, 1000.0)) < 1e-9, "Start {start:?}");
        assert!(ende.dist(V2::new(1000.0, 1500.0)) < 1e-9, "Ende {ende:?}");
    }

    /// Endpunkt ziehen setzt den Endwinkel — Radius und Mittelpunkt bleiben.
    #[test]
    fn bogen_ende_ziehen_aendert_den_endwinkel() {
        let e = bogen();
        // Zeiger unten am Mittelpunkt: Richtung 270°
        let neu = apply_grip_tol(&e, GripKind::ArcEnd, V2::new(1000.0, 0.0), 1.0)
            .expect("Endgriff muss greifen");
        let (c, r, a0, a1) = bogen_werte(&neu);
        assert_eq!(c, V2::new(1000.0, 1000.0), "Mittelpunkt verschoben");
        assert_eq!(r, 500.0, "Radius geändert");
        assert_eq!(a0, 0.0, "Startwinkel geändert");
        assert!((a1 + std::f64::consts::FRAC_PI_2).abs() < 1e-9, "a1 {a1}");
    }

    /// Startpunkt ziehen setzt den Startwinkel; der Endpunkt läuft auf dem
    /// Kreis mit — auch wenn der Zeiger weiter außen liegt.
    #[test]
    fn bogen_start_ziehen_aendert_den_startwinkel() {
        let e = bogen();
        let ziel = V2::new(1000.0, 3000.0); // Richtung 90°, Abstand 2000 mm
        let neu = apply_grip_tol(&e, GripKind::ArcStart, ziel, 1.0).expect("Startgriff");
        let (c, r, a0, a1) = bogen_werte(&neu);
        assert_eq!(r, 500.0, "Radius folgt dem Zeiger statt zu bleiben");
        assert!((a0 - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "a0 {a0}");
        assert_eq!(a1, std::f64::consts::FRAC_PI_2, "Endwinkel geändert");
        // Der Punkt liegt auf dem Kreis, nicht beim Zeiger.
        let start = c + V2::polar(a0, r);
        assert!(start.dist(V2::new(1000.0, 1500.0)) < 1e-9, "Start {start:?}");
    }

    /// Zeiger genau auf den Mittelpunkt: kein Zug (Richtung unbestimmt).
    #[test]
    fn bogen_ende_auf_mittelpunkt_bewegt_nichts() {
        let e = bogen();
        assert!(apply_grip_tol(&e, GripKind::ArcEnd, V2::new(1000.0, 1000.0), 1.0).is_none());
        assert!(apply_grip_tol(&e, GripKind::ArcStart, V2::new(1000.0, 1000.0), 1.0).is_none());
    }

    /// Der gezogene Endpunkt ist der einzige, der aus dem Fang fällt.
    #[test]
    fn bogen_ende_moving_points_ist_der_endpunkt() {
        let e = bogen();
        assert_eq!(
            moving_points(&e, GripKind::ArcEnd),
            vec![V2::new(1000.0, 1500.0)]
        );
        assert_eq!(
            moving_points(&e, GripKind::ArcStart),
            vec![V2::new(1500.0, 1000.0)]
        );
        assert_eq!(
            moving_points(&e, GripKind::Vert(0)),
            vec![V2::new(1000.0, 1000.0)]
        );
    }

    // ---- Zwei Ecken aufeinandergelegt: ein Punkt (T0018, Turn 2) ----

    /// Benachbarte Ecken auf derselben Stelle: der gezogene fällt weg, die
    /// Umgrenzung hat einen Punkt weniger.
    #[test]
    fn benachbarte_ecken_verschmelzen_zu_einem_punkt() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(4000.0, 0.0),
            V2::new(4000.0, 3000.0),
            V2::new(0.0, 3000.0),
        ];
        ring[1] = ring[0];
        assert!(verschmelze_nachbar(&mut ring, 1, 1.0));
        assert_eq!(ring.len(), 3, "ein Umrandungspunkt weniger: {ring:?}");
        assert_eq!(
            ring,
            vec![
                V2::new(0.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0)
            ]
        );
    }

    /// Dasselbe über den Griffweg: der Punkt landet auf dem Nachbarn, die
    /// Fläche bleibt eine Fläche mit drei Ecken.
    #[test]
    fn griffzug_auf_die_nachbarecke_verschmilzt() {
        let neu = apply_grip_tol(&quadrat(), GripKind::Vert(1), V2::new(0.0, 0.0), 1.0).unwrap();
        let (rand, _) = raender(&neu);
        assert_eq!(rand.len(), 4, "der Zug selbst verändert die Zahl der Punkte nicht");
        let mut ring = rand;
        assert!(verschmelze_nachbar(&mut ring, 1, 1.0));
        assert_eq!(ring.len(), 3);
    }

    /// Nur echte Berührung verschmilzt — knapp daneben bleibt der Punkt stehen.
    #[test]
    fn knapp_daneben_verschmilzt_nicht() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(4000.0, 0.0),
            V2::new(4000.0, 3000.0),
            V2::new(0.0, 3000.0),
        ];
        ring[1] = V2::new(0.0, 5.0);
        assert!(!verschmelze_nachbar(&mut ring, 1, 1.0));
        assert_eq!(ring.len(), 4);
    }

    /// Ein Dreieck bleibt ein Dreieck: zwei Punkte wären keine Fläche mehr.
    #[test]
    fn dreieck_behaelt_drei_punkte() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(0.0, 1000.0),
        ];
        ring[0] = ring[1];
        assert!(!verschmelze_nachbar(&mut ring, 0, 10.0));
        assert_eq!(ring.len(), 3);
    }

    /// Nicht benachbarte Punkte verschmelzen nicht — sonst verschwände ein
    /// ganzes Stück Umgrenzung mit.
    #[test]
    fn nicht_benachbarte_punkte_verschmelzen_nicht() {
        let mut ring = vec![
            V2::new(2000.0, 0.0),
            V2::new(1000.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(0.0, 1000.0),
        ];
        assert!(!verschmelze_nachbar(&mut ring, 0, 1.0));
        assert_eq!(ring.len(), 5);
    }

    /// Der gezogene Punkt verschwindet auch am Ringende (Nachbar = erster
    /// Punkt): die Umgrenzung schließt sich ohne Doppelpunkt.
    #[test]
    fn verschmelzen_ueber_die_ringnaht() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(4000.0, 0.0),
            V2::new(4000.0, 3000.0),
            V2::new(0.0, 3000.0),
        ];
        ring[3] = ring[0];
        assert!(verschmelze_nachbar(&mut ring, 3, 1.0));
        assert_eq!(ring.len(), 3, "{ring:?}");
    }

    // ---- Fang-Sperre: nicht sperren, was selbst das Ziel ist (T0018, Turn 3) ----

    /// Solange die Ecke frei am Zeiger hängt, wird sie gesperrt (sonst fängt sie
    /// sich selbst).
    #[test]
    fn freie_ecke_bleibt_gesperrt() {
        let e = quadrat();
        assert_eq!(fang_sperre(&e, GripKind::Vert(1)), vec![V2::new(4000.0, 0.0)]);
    }

    /// Liegt die gezogene Ecke auf der Nachbarecke, darf die Sperre nicht
    /// greifen: sonst sperrt sie das Fangziel und der Fang fällt im nächsten
    /// Frame aus (Flackern).
    #[test]
    fn ecke_auf_der_nachbarecke_wird_nicht_gesperrt() {
        let mut e = quadrat();
        if let EntityKind::Fill { boundary, .. } = &mut e.kind {
            boundary[1] = boundary[0];
        }
        assert!(
            fang_sperre(&e, GripKind::Vert(1)).is_empty(),
            "zwei Punkte auf derselben Stelle: dort ist der Fang das Ziel"
        );
    }

    /// Auch eine Kantenmitte desselben Elements ist ein Fangziel: liegt die
    /// gezogene Ecke dort, darf die Sperre nicht greifen (sonst flackert der
    /// Fang auf „Mitte" genauso).
    #[test]
    fn ecke_auf_eigener_kantenmitte_wird_nicht_gesperrt() {
        let mut e = quadrat();
        if let EntityKind::Fill { boundary, .. } = &mut e.kind {
            // Mitte der linken Kante (0;0) → (0;3000)
            boundary[1] = V2::new(0.0, 1500.0);
        }
        assert!(fang_sperre(&e, GripKind::Vert(1)).is_empty());
        // Der Punkt bleibt trotzdem in der Punktliste (er wird ja bewegt).
        assert_eq!(moving_points(&e, GripKind::Vert(1)), vec![V2::new(0.0, 1500.0)]);
    }

    /// Dasselbe für den geschlossenen Zug einer Polylinie (letzter Punkt auf
    /// dem ersten).
    #[test]
    fn polylinie_auf_dem_anfangspunkt_wird_nicht_gesperrt() {
        let mut e = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1500.0),
            V2::new(0.0, 3000.0),
        ]);
        let i = 3;
        if let EntityKind::Polyline { verts, .. } = &mut e.kind {
            verts[i].p = verts[0].p;
        }
        assert!(fang_sperre(&e, GripKind::Vert(i)).is_empty());
        // Ein mittlerer Punkt bleibt gesperrt.
        assert_eq!(
            fang_sperre(&e, GripKind::Vert(1)),
            vec![V2::new(2000.0, 0.0)]
        );
    }

    /// Ein Griff, den es nicht gibt, verändert nichts (kein Panik, kein None-Fehler).
    #[test]
    fn fuellflaeche_ohne_passenden_griff_bleibt_unveraendert() {
        let e = quadrat();
        assert!(apply_grip_tol(&e, GripKind::Vert(9), V2::new(1.0, 1.0), 1.0).is_none());
        assert!(apply_grip_tol(&e, GripKind::Edge(9), V2::new(1.0, 1.0), 1.0).is_none());
    }

    /// Ein Dreieck (drei Stützpunkte) lässt sich nicht durch einen Griffzug
    /// schließen — nach dem Wegfall des doppelten Punktes bliebe keine Fläche.
    #[test]
    fn dreieck_wird_nicht_geschlossen() {
        let orig = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(0.0, 1000.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(2), V2::new(0.0, 0.0), 5.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(!closed);
        assert_eq!(pts.len(), 3);
    }

    // ---- Bezugspunkt für Ortho/Polar beim Griffzug (T0046) ----

    fn linie() -> Entity {
        Entity::new(
            1,
            1,
            EntityKind::Line {
                a: V2::new(0.0, 0.0),
                b: V2::new(2000.0, 1000.0),
            },
        )
    }

    /// Linie und Wand messen die Richtung am **anderen** Ende.
    #[test]
    fn bezug_der_linie_ist_das_andere_ende() {
        let e = linie();
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(2000.0, 1000.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(1)), Some(V2::new(0.0, 0.0)));

        let w = Entity::new(
            1,
            1,
            EntityKind::Wall {
                a: V2::new(0.0, 0.0),
                b: V2::new(2000.0, 1000.0),
                thickness: 240.0,
                height: 2750.0,
                material: String::new(),
                hatch: None,
                align: jjcad_core::style::WallAlign::Center,
            },
        );
        assert_eq!(bezugspunkt(&w, GripKind::Vert(0)), Some(V2::new(2000.0, 1000.0)));
        assert_eq!(bezugspunkt(&w, GripKind::Vert(1)), Some(V2::new(0.0, 0.0)));
    }

    /// Offene Polylinie: der vorige Stützpunkt, am Anfang der nächste.
    #[test]
    fn bezug_der_offenen_polylinie_ist_die_nachbarecke() {
        let e = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
        ]);
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(2000.0, 0.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(2)), Some(V2::new(2000.0, 0.0)));
    }

    /// Geschlossener Zug (Rechteck): auch der Anfang hat einen Vorgänger —
    /// die letzte Ecke des Rings.
    #[test]
    fn bezug_des_geschlossenen_zugs_ist_die_ringecke() {
        let mut e = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(0.0, 1000.0),
        ]);
        if let EntityKind::Polyline { closed, .. } = &mut e.kind {
            *closed = true;
        }
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(0.0, 1000.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(1)), Some(V2::new(0.0, 0.0)));
    }

    /// Füllfläche und Volumen: der vorige Punkt desselben Rings.
    #[test]
    fn bezug_der_flaeche_ist_der_vorige_ringpunkt() {
        let e = quadrat();
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(0.0, 3000.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(2)), Some(V2::new(4000.0, 0.0)));
    }

    /// Kantenmitten und Mittelpunkte bewegen eindimensional bzw. richtungslos —
    /// dort gibt es keinen Bezug (sonst zöge Ortho sie aus ihrer Bahn).
    #[test]
    fn ohne_bezug_bei_kantenmitte_und_mittelpunkt() {
        let mut e = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
        ]);
        if let EntityKind::Polyline { closed, .. } = &mut e.kind {
            *closed = true;
        }
        assert_eq!(bezugspunkt(&e, GripKind::Edge(0)), None);
        assert_eq!(bezugspunkt(&quadrat(), GripKind::Edge(0)), None);

        let kreis = Entity::new(
            1,
            1,
            EntityKind::Circle {
                c: V2::new(0.0, 0.0),
                r: 500.0,
            },
        );
        assert_eq!(bezugspunkt(&kreis, GripKind::Vert(0)), None);
    }
}

#[cfg(test)]
mod tests_raumstempel {
    use super::*;

    fn stempel() -> Entity {
        Entity::new(
            5,
            1,
            EntityKind::Room {
                boundary: vec![
                    V2::new(0.0, 0.0),
                    V2::new(4000.0, 0.0),
                    V2::new(4000.0, 3000.0),
                    V2::new(0.0, 3000.0),
                ],
                label: V2::new(2000.0, 1500.0),
                height: 250.0,
                heading: "Zimmer".into(),
            },
        )
    }

    fn teile(e: &Entity) -> (Vec<V2>, V2) {
        match &e.kind {
            EntityKind::Room { boundary, label, .. } => (boundary.clone(), *label),
            _ => panic!("kein Raumstempel"),
        }
    }

    /// Vier Eckgriffe plus der Textgriff — die Fläche selbst ist unsichtbar,
    /// ihre Ecken erscheinen mit der Auswahl.
    #[test]
    fn raumstempel_hat_eck_und_textgriffe() {
        let griffe = grips_of(&stempel());
        assert_eq!(griffe.len(), 5, "vier Ecken und ein Textgriff");
        assert_eq!(
            griffe.iter().filter(|(k, _)| *k == GripKind::Label).count(),
            1
        );
        assert!(griffe.contains(&(GripKind::Label, V2::new(2000.0, 1500.0))));
    }

    /// Der Textgriff verschiebt nur die Beschriftung — die Umgrenzung bleibt
    /// unverändert (die Fläche wandert nicht mit).
    #[test]
    fn textgriff_verschiebt_nur_die_beschriftung() {
        let e = stempel();
        let (vorher, _) = teile(&e);
        let neu = apply_grip(&e, GripKind::Label, V2::new(500.0, 2500.0)).expect("Griff");
        let (nachher, label) = teile(&neu);
        assert_eq!(vorher, nachher, "die Fläche ist mitgewandert");
        assert_eq!(label, V2::new(500.0, 2500.0));
    }

    /// Eine Ecke ziehen ändert die Fläche; nur dieser Punkt wandert, der Text
    /// bleibt an seiner Stelle (der Inhalt rechnet sich neu).
    #[test]
    fn ecke_ziehen_aendert_nur_diesen_punkt() {
        let e = stempel();
        let neu = apply_grip(&e, GripKind::Vert(2), V2::new(6000.0, 4000.0)).expect("Griff");
        let (rand, label) = teile(&neu);
        assert_eq!(rand[2], V2::new(6000.0, 4000.0));
        assert_eq!(rand[0], V2::new(0.0, 0.0));
        assert_eq!(rand[1], V2::new(4000.0, 0.0));
        assert_eq!(rand[3], V2::new(0.0, 3000.0));
        assert_eq!(label, V2::new(2000.0, 1500.0), "Textlage bleibt stehen");
    }

    /// Der gezogene Punkt wird beim Fangen gesperrt, die Nachbarecken bleiben
    /// Fangziel — sonst flackerte der Griff (T0018).
    #[test]
    fn ecke_ziehen_sperrt_nur_den_gezogenen_punkt() {
        let e = stempel();
        assert_eq!(
            moving_points(&e, GripKind::Vert(1)),
            vec![V2::new(4000.0, 0.0)]
        );
        assert_eq!(moving_points(&e, GripKind::Label), vec![V2::new(2000.0, 1500.0)]);
        let bezug = bezugspunkte(&e, GripKind::Vert(0));
        assert_eq!(bezug.len(), 2, "voriger und nächster Punkt");
    }
}

#[cfg(test)]
mod tests_treppe {
    use super::*;

    fn treppe() -> Entity {
        Entity::new(
            3,
            1,
            EntityKind::Stair {
                a: V2::new(0.0, 0.0),
                b: V2::new(0.0, 4000.0),
                width: 1000.0,
                steps: 16,
                height: 2750.0,
                auto_steps: true,
                cut: true,
                unten: None,
                oben: None,
            },
        )
    }

    fn masse(e: &Entity) -> (V2, V2, f64) {
        match &e.kind {
            EntityKind::Stair { a, b, width, .. } => (*a, *b, *width),
            _ => panic!("keine Treppe"),
        }
    }

    /// Vier Kantengriffe auf dem Rechteck (nicht auf der Lauflinie).
    #[test]
    fn treppe_hat_kantengriffe() {
        let griffe = grips_of(&treppe());
        assert_eq!(griffe.len(), 4, "vier Kantenmitten");
        for (art, p) in griffe {
            assert!(matches!(art, GripKind::Edge(_)));
            // Rechteck 1000 breit, Lauflinie bei x = 0: die Längskanten liegen
            // bei x = ±500, die Querkanten bei y = 0 und y = 4000.
            assert!(
                (p.x.abs() - 500.0).abs() < 0.01 || p.y.abs() < 0.01 || (p.y - 4000.0).abs() < 0.01,
                "Griff {p:?} liegt nicht auf der Kontur"
            );
        }
    }

    /// Eine Längskante ziehen verschiebt nur diese Kante: die gegenüberliegende
    /// bleibt stehen, Breite und Lauflinie folgen der halben Verschiebung.
    #[test]
    fn laengskante_verschiebt_parallel() {
        let e = treppe();
        // Kante 0 liegt auf x = -500 (Normale zeigt nach -x); 200 weiter nach
        // außen ziehen.
        let neu = apply_grip(&e, GripKind::Edge(0), V2::new(-700.0, 2000.0)).unwrap();
        let (a, b, w) = masse(&neu);
        assert!((w - 1200.0).abs() < 0.01, "Breite {w}");
        assert!((a.x + 100.0).abs() < 0.01, "Lauflinie wandert 100 nach links: {a:?}");
        assert!((b.x + 100.0).abs() < 0.01, "{b:?}");
        assert!((a.y).abs() < 0.01 && (b.y - 4000.0).abs() < 0.01, "Länge bleibt");
    }

    /// Die Endkante zieht das Laufende mit — die Länge ist das Rechteckmaß.
    #[test]
    fn endkante_veraendert_die_laenge() {
        let e = treppe();
        let neu = apply_grip(&e, GripKind::Edge(1), V2::new(0.0, 3000.0)).unwrap();
        let (a, b, w) = masse(&neu);
        assert!((a.y).abs() < 0.01, "Anfang bleibt stehen");
        assert!((b.y - 3000.0).abs() < 0.01, "Ende bei 3,00 m: {b:?}");
        assert!((w - 1000.0).abs() < 0.01, "Breite bleibt");
    }

    /// Keine Kante darf die Treppe auf null schrumpfen.
    #[test]
    fn kantenziehen_haelt_die_mindestbreite() {
        let e = treppe();
        let neu = apply_grip(&e, GripKind::Edge(0), V2::new(900.0, 2000.0)).unwrap();
        let (_, _, w) = masse(&neu);
        assert!(w >= 99.9, "Breite {w} fällt unter das Minimum");
    }
}

mod tests_volumen {
    use super::*;

    /// Volumen: dieselben Eckpunkte wie eine Füllfläche — sie müssen als
    /// Griffe erscheinen und sich ziehen lassen.
    fn volumen() -> Entity {
        Entity::new(
            7,
            1,
            EntityKind::Volume {
                boundary: vec![
                    V2::new(0.0, 0.0),
                    V2::new(1000.0, 0.0),
                    V2::new(1000.0, 800.0),
                    V2::new(0.0, 800.0),
                ],
                holes: Vec::new(),
                pattern: jjcad_core::style::FillPattern::Solid,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: true,
                height: 2750.0,
            },
        )
    }

    #[test]
    fn volumen_hat_eckpunkte_als_griffe() {
        let e = volumen();
        let g = grips_of(&e);
        assert_eq!(g.len(), 4, "vier Eckpunkte, war {g:?}");
        assert!(g.iter().any(|(_, p)| p.dist(V2::new(1000.0, 800.0)) < 0.5));
    }

    #[test]
    fn volumen_eckpunkt_laesst_sich_ziehen() {
        let e = volumen();
        let neu = apply_grip(&e, GripKind::Vert(2), V2::new(1400.0, 900.0)).expect("kein Zug");
        match &neu.kind {
            EntityKind::Volume { boundary, height, .. } => {
                assert!(boundary[2].dist(V2::new(1400.0, 900.0)) < 0.5, "Ecke nicht gezogen");
                assert_eq!(*height, 2750.0, "Höhe darf sich nicht ändern");
            }
            _ => panic!("kein Volumen mehr"),
        }
    }
}

#[cfg(test)]
mod tests_volumen_drag {
    use super::*;

    /// Der komplette Griffzug an einem Volumen: Griff treffen, ziehen,
    /// loslassen — die Ecke sitzt danach am Ziel, die Höhe bleibt.
    #[test]
    fn volumen_griffzug_von_anfang_bis_ende() {
        let mut app = crate::app::App::with_ctx(egui::Context::default());
        let id = app
            .session_mut()
            .execute(
                "draw.volume",
                &serde_json::json!({
                    "boundary": [[0.0, 0.0], [1.0, 0.0], [1.0, 0.8], [0.0, 0.8]],
                    "height": 2.75,
                }),
            )
            .unwrap()["created"][0]
            .as_u64()
            .unwrap();
        app.session_mut().select(vec![id]);

        // Griff an der Ecke (1000, 800) treffen und ziehen
        let (gid, kind) = app
            .grip_at(V2::new(1000.0, 800.0), 5.0)
            .expect("kein Griff an der Ecke");
        assert_eq!(gid, id);
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(1400.0, 900.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Volume { boundary, height, .. } => {
                assert!(
                    boundary.iter().any(|p| p.dist(V2::new(1400.0, 900.0)) < 0.5),
                    "Ecke nicht am Ziel: {boundary:?}"
                );
                assert_eq!(*height, 2750.0);
            }
            _ => panic!("kein Volumen mehr"),
        }
    }

}

#[cfg(test)]
mod tests_masskette {
    use super::*;
    use jjcad_core::model::DimAxis;

    /// Dreipunktige Maßkette auf der Waagerechten, Maßlinie 400 mm darunter.
    fn kette() -> Entity {
        Entity::new(
            7,
            1,
            EntityKind::DimChain {
                pts: vec![
                    V2::new(0.0, 0.0),
                    V2::new(2000.0, 0.0),
                    V2::new(5000.0, 0.0),
                ],
                line_pos: V2::new(1000.0, -400.0),
                axis: DimAxis::X,
            },
        )
    }

    /// Jeder Basispunkt **und** die Maßlinie sind Griffe — vorher hatte eine
    /// Maßkette gar keinen (T0055).
    #[test]
    fn jeder_basispunkt_und_die_masslinie_haben_einen_griff() {
        let e = kette();
        let griffe = grips_of(&e);
        assert_eq!(
            griffe,
            vec![
                (GripKind::Vert(0), V2::new(0.0, 0.0)),
                (GripKind::Vert(1), V2::new(2000.0, 0.0)),
                (GripKind::Vert(2), V2::new(5000.0, 0.0)),
                // Mitte der gezeichneten Maßlinie
                (GripKind::Edge(0), V2::new(2500.0, -400.0)),
            ]
        );
    }

    /// Ein Basispunkt folgt dem Zeiger, die übrigen Punkte und die Maßlinie
    /// bleiben stehen.
    #[test]
    fn basispunkt_zieht_nur_sich_selbst() {
        let neu = apply_grip(&kette(), GripKind::Vert(1), V2::new(2600.0, 300.0)).unwrap();
        match &neu.kind {
            EntityKind::DimChain { pts, line_pos, .. } => {
                assert_eq!(pts[1], V2::new(2600.0, 300.0));
                assert_eq!(pts[0], V2::new(0.0, 0.0));
                assert_eq!(pts[2], V2::new(5000.0, 0.0));
                assert_eq!(*line_pos, V2::new(1000.0, -400.0), "Maßlinie wandert mit");
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Die Raute zieht nur die Maßlinie — senkrecht zur Messrichtung, die
    /// Basispunkte bleiben.
    #[test]
    fn masslinie_wandert_senkrecht_ohne_basispunkte() {
        let neu = apply_grip(&kette(), GripKind::Edge(0), V2::new(3000.0, 700.0)).unwrap();
        match &neu.kind {
            EntityKind::DimChain { pts, line_pos, .. } => {
                assert_eq!(
                    pts,
                    &vec![
                        V2::new(0.0, 0.0),
                        V2::new(2000.0, 0.0),
                        V2::new(5000.0, 0.0)
                    ]
                );
                // nur die Höhe zählt, die waagerechte Lage bleibt
                assert!((line_pos.y - 700.0).abs() < 1e-9, "{line_pos:?}");
                assert!((line_pos.x - 1000.0).abs() < 1e-9, "{line_pos:?}");
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Der gezogene Basispunkt darf sich nicht selbst fangen (Flackern), die
    /// Maßlinie als Ganzes ebenso wenig.
    #[test]
    fn gezogener_punkt_sperrt_sich_selbst() {
        assert_eq!(
            moving_points(&kette(), GripKind::Vert(1)),
            vec![V2::new(2000.0, 0.0)]
        );
        assert_eq!(
            moving_points(&kette(), GripKind::Edge(0)),
            vec![V2::new(0.0, -400.0), V2::new(5000.0, -400.0)]
        );
    }

    /// Ortho und Polar richten sich am Nachbarpunkt der Kette aus.
    #[test]
    fn bezug_ist_der_nachbarpunkt() {
        assert_eq!(
            bezugspunkt(&kette(), GripKind::Vert(1)),
            Some(V2::new(0.0, 0.0))
        );
        assert_eq!(bezugspunkt(&kette(), GripKind::Vert(0)), Some(V2::new(2000.0, 0.0)));
        // Die Maßlinie bewegt sich eindimensional — kein Bezug.
        assert_eq!(bezugspunkt(&kette(), GripKind::Edge(0)), None);
    }

    /// Der Griff unter dem Zeiger wird gefunden, und der Zug landet im
    /// **Dokument** (gespeicherter Basispunkt) — der Weg der Zeichenfläche.
    #[test]
    fn mauszug_bewegt_den_basispunkt() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]],
                                    "line_pos": [1.0, -0.4], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        // Griffe gibt es nur in der Auswahl (`App::grip_at`).
        app.session_mut().selection = vec![id];

        let (gid, kind) = app
            .grip_at(V2::new(2000.0, 0.0), 5.0)
            .expect("kein Griff am Basispunkt");
        assert_eq!(gid, id);
        assert_eq!(kind, GripKind::Vert(1));
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(2600.0, 300.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::DimChain { pts, .. } => {
                assert_eq!(pts[1], V2::new(2600.0, 300.0), "{pts:?}");
                assert_eq!(pts[0], V2::new(0.0, 0.0));
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Dasselbe für die Raute an der Maßlinie: sie verschiebt nur die Linie.
    #[test]
    fn mauszug_bewegt_die_masslinie() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2.0, 0.0], [5.0, 0.0]],
                                    "line_pos": [1.0, -0.4], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        app.session_mut().selection = vec![id];

        // Die Mitte der Maßlinie liegt bei (2500, -400).
        let (gid, kind) = app
            .grip_at(V2::new(2500.0, -400.0), 5.0)
            .expect("kein Griff an der Maßlinie");
        assert_eq!((gid, kind), (id, GripKind::Edge(0)));
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(2500.0, 700.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::DimChain { pts, line_pos, .. } => {
                assert!((line_pos.y - 700.0).abs() < 1e-9, "{line_pos:?}");
                assert_eq!(pts[1], V2::new(2000.0, 0.0), "Basispunkt wandert mit");
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Ein Einzelmaß hat dieselben Griffe.
    #[test]
    fn einzelmass_hat_basispunkte_und_masslinie() {
        let e = Entity::new(
            8,
            1,
            EntityKind::Dim {
                a: V2::new(0.0, 0.0),
                b: V2::new(3000.0, 0.0),
                line_pos: V2::new(1500.0, -300.0),
                axis: DimAxis::X,
                override_text: None,
            },
        );
        let griffe = grips_of(&e);
        assert_eq!(
            griffe,
            vec![
                (GripKind::Vert(0), V2::new(0.0, 0.0)),
                (GripKind::Vert(1), V2::new(3000.0, 0.0)),
                (GripKind::Edge(0), V2::new(1500.0, -300.0)),
            ]
        );
        let neu = apply_grip(&e, GripKind::Vert(1), V2::new(3500.0, 0.0)).unwrap();
        match &neu.kind {
            EntityKind::Dim { b, .. } => assert_eq!(*b, V2::new(3500.0, 0.0)),
            k => panic!("kein Maß mehr: {k:?}"),
        }
    }
}
