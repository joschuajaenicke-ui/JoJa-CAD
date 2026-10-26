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
        (EntityKind::Polyline { verts, .. }, GripKind::Vert(i)) => {
            verts.get_mut(i)?.p = target;
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
        let Some(new_e) = apply_grip(&d.orig, d.kind, target) else {
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
