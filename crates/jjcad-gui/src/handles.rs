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
}

#[derive(Clone, Debug)]
pub struct GripDrag {
    pub entity: Id,
    pub kind: GripKind,
    pub orig: Entity,
}

/// Alle Griffpunkte eines Elements mit ihrer Weltposition.
pub fn grips_of(e: &Entity) -> Vec<(GripKind, V2)> {
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
            vec![(GripKind::Vert(0), *a), (GripKind::Vert(1), *b)]
        }
        EntityKind::Polyline { verts, closed } => {
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
        | EntityKind::Arc { c, .. }
        | EntityKind::Ellipse { c, .. }
        | EntityKind::Point { p: c } => vec![(GripKind::Vert(0), *c)],
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
        (EntityKind::Ellipse { c, .. }, GripKind::Vert(_)) => *c = target,
        (EntityKind::Polyline { verts, closed }, GripKind::Vert(i)) => {
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
        (EntityKind::Polyline { verts, .. }, GripKind::Vert(i)) => {
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
        // Kreis, Bogen, Ellipse und Punkt bewegen ihren Mittelpunkt.
        (
            EntityKind::Circle { c, .. }
            | EntityKind::Arc { c, .. }
            | EntityKind::Ellipse { c, .. }
            | EntityKind::Point { p: c },
            GripKind::Vert(_),
        ) => vec![*c],
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
        self.status_msg = "Griff ziehen".into();
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

    pub fn end_grip_drag(&mut self) {
        if self.grip_drag.take().is_some() {
            self.status_msg = "Bearbeitet".into();
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
