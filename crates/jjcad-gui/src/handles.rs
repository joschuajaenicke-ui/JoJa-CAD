//! Direktes Bearbeiten über Griffe: Endpunkte, Ecken und Parallelverschiebung.

use crate::app::App;
use jjcad_core::geom::Vec2 as V2;
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
        // Bogen: Mittelpunkt (verschiebt den Bogen) und beide Enden. Ein
        // Endpunkt läuft auf dem Kreis mit, gezogen wird der Winkel; der
        // Radius bleibt dabei stehen (T0022).
        EntityKind::Arc { c, r, a0, a1 } => vec![
            (GripKind::Vert(0), *c),
            (GripKind::ArcStart, *c + V2::polar(*a0, *r)),
            (GripKind::ArcEnd, *c + V2::polar(*a1, *r)),
        ],
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
    match (&mut e.kind, kind) {
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
            | EntityKind::Point { p: c },
            GripKind::Vert(_),
        ) => vec![*c],
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
        EntityKind::Fill { boundary, holes, .. } => boundary
            .iter()
            .chain(holes.iter().flatten())
            .copied()
            .collect(),
        _ => Vec::new(),
    }
}

/// Richtung einer Polygonkante (normiert).
pub fn edge_dir(e: &Entity, i: usize) -> Option<(V2, V2)> {
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
}

#[cfg(test)]
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
