//! Vorschau („Geist") vor dem Platzieren.
//!
//! Gilt für Blöcke aus dem Inventar und für das Einfügen aus der Zwischenablage.
//! Der Ankerpunkt ist die **untere linke Ecke** der Vorschau — sie steht damit
//! oben rechts am Mauszeiger und verdeckt ihn nicht.

use jjcad_core::geom::{Affine, BBox, Vec2 as V2};
use jjcad_core::model::{Document, Entity};
use jjcad_core::render::{self, Prim, RenderCtx};

/// Was gerade platziert wird.
#[derive(Clone, Debug)]
pub struct Ghost {
    /// Block-ID aus der Bibliothek — `None` bedeutet Zwischenablage.
    pub block: Option<String>,
    /// Ankerpunkt in Weltkoordinaten (untere linke Ecke).
    pub pos: V2,
    /// Drehung im Bogenmaß (mathematisch positiv).
    pub rot: f64,
    /// Gespiegelt an der senkrechten Achse durch den Anker.
    pub mirror: bool,
    pub scale: f64,
}

impl Ghost {
    pub fn new(block: Option<String>, pos: V2) -> Self {
        Self {
            block,
            pos,
            rot: 0.0,
            mirror: false,
            scale: 1.0,
        }
    }

    /// Dreht die Vorschau schrittweise (Standard 90°).
    pub fn rotate(&mut self, deg: f64) {
        use std::f64::consts::TAU;
        self.rot = (self.rot + deg.to_radians()).rem_euclid(TAU);
    }

    pub fn flip(&mut self) {
        self.mirror = !self.mirror;
    }

    pub fn rot_deg(&self) -> f64 {
        self.rot.to_degrees().round()
    }
}

/// Geometrie der Vorschau, bereits an `g.pos` ausgerichtet.
pub struct GhostGeo {
    pub prims: Vec<Prim>,
    pub bbox: BBox,
    /// Einfügepunkt (Blockursprung) in Weltkoordinaten.
    pub origin: V2,
}

/// Transformierte Kopien der Zwischenablage-Elemente (für Vorschau und Einfügen).
pub fn clipboard_entities(doc: &Document, g: &Ghost, clip: &[Entity]) -> Vec<Entity> {
    if clip.is_empty() {
        return Vec::new();
    }
    let mut base = BBox::EMPTY;
    for e in clip {
        base.union(render::entity_bbox(doc, e));
    }
    let c = base.center();
    // erst spiegeln, dann um den eigenen Mittelpunkt drehen
    let mut m = Affine::mirror(c + V2::new(0.0, -1.0), c + V2::new(0.0, 1.0));
    if !g.mirror {
        m = Affine::IDENTITY;
    }
    m = Affine::rotate_about(c, g.rot) * m;
    if (g.scale - 1.0).abs() > 1e-9 {
        m = Affine::scale_about(c, g.scale, g.scale) * m;
    }

    let moved: Vec<Entity> = clip
        .iter()
        .map(|e| jjcad_core::transform::transform_entity(e, &m))
        .collect();

    // auf den Ankerpunkt schieben
    let mut bb = BBox::EMPTY;
    for e in &moved {
        bb.union(render::entity_bbox(doc, e));
    }
    if !bb.is_valid() {
        return moved;
    }
    let shift = Affine::translate(g.pos - bb.min);
    moved
        .iter()
        .map(|e| jjcad_core::transform::transform_entity(e, &shift))
        .collect()
}

/// Vorschaugeometrie für die aktuelle Lage des Geistes.
pub fn geometry(doc: &Document, g: &Ghost, clip: &[Entity]) -> GhostGeo {
    let ctx = RenderCtx::new(doc);
    let mut local = Affine::scale(g.scale, g.scale);
    if g.mirror {
        local = Affine::mirror(V2::new(0.0, -1.0), V2::new(0.0, 1.0)) * local;
    }
    local = Affine::rotate(g.rot) * local;

    if let Some(bid) = &g.block {
        if let Some(def) = doc.block(bid) {
            // Lage ohne Verschiebung -> Bezugsrahmen für den Anker
            let mut base = render::DisplayList::default();
            for e in &def.entities {
                render::render_entity(&ctx, e, local, &mut base, 0);
            }
            let shift = if base.bbox.is_valid() {
                Affine::translate(g.pos - base.bbox.min)
            } else {
                Affine::translate(g.pos)
            };
            let m = shift * local;
            let mut dl = render::DisplayList::default();
            for e in &def.entities {
                render::render_entity(&ctx, e, m, &mut dl, 0);
            }
            return GhostGeo {
                prims: dl.prims,
                bbox: dl.bbox,
                origin: g.pos - if base.bbox.is_valid() { base.bbox.min } else { V2::ZERO },
            };
        }
    }

    // Zwischenablage
    let ents = clipboard_entities(doc, g, clip);
    let mut dl = render::DisplayList::default();
    for e in &ents {
        render::render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    }
    GhostGeo {
        prims: dl.prims,
        bbox: dl.bbox,
        origin: g.pos,
    }
}
