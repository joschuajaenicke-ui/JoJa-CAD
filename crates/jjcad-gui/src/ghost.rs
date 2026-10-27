//! Vorschau („Geist") vor dem Platzieren.
//!
//! Gilt für Blöcke aus dem Inventar und für das Einfügen aus der Zwischenablage.
//! Bei Blöcken ist der Ankerpunkt die **untere linke Ecke** der Vorschau — sie
//! steht damit oben rechts am Mauszeiger und verdeckt ihn nicht. Bei der
//! Zwischenablage ist der Anker der mitkopierte **Basispunkt** (siehe
//! [`Clipboard`]).

use jjcad_core::geom::{Affine, BBox, Vec2 as V2};
use jjcad_core::model::{Document, Entity};
use jjcad_core::render::{self, Prim, RenderCtx};

/// Zwischenablage: die kopierten Elemente **und** ihr Basispunkt.
///
/// Der Basispunkt gehört zur Zwischenablage: „Kopieren mit Basispunkt" merkt
/// sich den geklickten Punkt, „Kopieren" automatisch die untere linke Ecke der
/// Auswahl. Beim Einfügen landet dieser Punkt unter dem Mauszeiger — die
/// Elemente setzen sich also immer gleich zum Basispunkt ab.
#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub ents: Vec<Entity>,
    /// Basispunkt in Weltkoordinaten (mm)
    pub base: Option<V2>,
}

impl Clipboard {
    pub fn is_empty(&self) -> bool {
        self.ents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ents.len()
    }
}

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
///
/// `g.pos` ist der Platz des **Basispunkts**: bei „Kopieren" die untere linke
/// Ecke (wie ohne Basispunkt), bei „Kopieren mit Basispunkt" der geklickte Punkt.
pub fn clipboard_entities(doc: &Document, g: &Ghost, clip: &Clipboard) -> Vec<Entity> {
    if clip.is_empty() {
        return Vec::new();
    }
    let mut base = BBox::EMPTY;
    for e in &clip.ents {
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
        .ents
        .iter()
        .map(|e| jjcad_core::transform::transform_entity(e, &m))
        .collect();

    // auf den Ankerpunkt schieben: der Basispunkt wandert mit der Drehung und
    // landet unter dem Zeiger, nicht die Ecke der gedrehten Auswahl.
    let mut bb = BBox::EMPTY;
    for e in &moved {
        bb.union(render::entity_bbox(doc, e));
    }
    if !bb.is_valid() {
        return moved;
    }
    let anker = clip.base.map(|b| m.apply(b)).unwrap_or(bb.min);
    let shift = Affine::translate(g.pos - anker);
    moved
        .iter()
        .map(|e| jjcad_core::transform::transform_entity(e, &shift))
        .collect()
}

/// Vorschaugeometrie für die aktuelle Lage des Geistes.
pub fn geometry(doc: &Document, g: &Ghost, clip: &Clipboard) -> GhostGeo {
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

#[cfg(test)]
mod tests_basispunkt {
    //! T0012: Der Basispunkt gehört zur Zwischenablage und ist der Anker beim
    //! Einfügen. „Kopieren" setzt ihn auf die untere linke Ecke, „Kopieren mit
    //! Basispunkt" auf den geklickten Punkt.

    use super::*;
    use jjcad_core::model::EntityKind;

    fn doc_mit_linie() -> (Document, Vec<Entity>) {
        let mut doc = Document::default();
        let e = Entity::new(
            doc.new_id(),
            doc.current_layer,
            EntityKind::Line {
                a: V2::new(0.0, 0.0),
                b: V2::new(1000.0, 2000.0),
            },
        );
        (doc, vec![e])
    }

    fn verschoben(doc: &Document, clip: &Clipboard, pos: V2) -> BBox {
        let g = Ghost::new(None, pos);
        let ents = clipboard_entities(doc, &g, clip);
        assert_eq!(ents.len(), 1, "ein Element eingefügt");
        let mut bb = BBox::EMPTY;
        for e in &ents {
            bb.union(render::entity_bbox(doc, e));
        }
        bb
    }

    #[test]
    fn ohne_basispunkt_landet_die_untere_linke_ecke_am_zeiger() {
        let (doc, ents) = doc_mit_linie();
        let clip = Clipboard { ents, base: None };
        let bb = verschoben(&doc, &clip, V2::new(5000.0, 6000.0));
        assert_eq!(bb.min, V2::new(5000.0, 6000.0), "Ecke unten links am Zeiger");
    }

    #[test]
    fn basispunkt_landet_am_zeiger() {
        let (doc, ents) = doc_mit_linie();
        // Basispunkt mitten in der Auswahl (500,1000)
        let clip = Clipboard {
            ents,
            base: Some(V2::new(500.0, 1000.0)),
        };
        let bb = verschoben(&doc, &clip, V2::new(3000.0, 4000.0));
        assert_eq!(
            bb.min,
            V2::new(2500.0, 3000.0),
            "der Basispunkt sitzt am Zeiger, die Ecke wandert mit"
        );
    }

    /// Der Basispunkt wandert mit der Drehung — sonst spränge die Auswahl beim
    /// Drehen der Vorschau um die Ecke.
    #[test]
    fn basispunkt_dreht_mit_der_vorschau() {
        let (doc, ents) = doc_mit_linie();
        let clip = Clipboard {
            ents,
            base: Some(V2::new(0.0, 0.0)),
        };
        let mut g = Ghost::new(None, V2::new(3000.0, 4000.0));
        g.rotate(90.0);
        let ents = clipboard_entities(&doc, &g, &clip);
        // Die Linie zeigt nach der Drehung nach oben; ihr Startpunkt (Basispunkt)
        // liegt am Zeiger.
        match &ents[0].kind {
            EntityKind::Line { a, .. } => {
                assert!(
                    (a.x - 3000.0).abs() < 1e-6 && (a.y - 4000.0).abs() < 1e-6,
                    "Basispunkt nach der Drehung am Zeiger, war {a:?}"
                );
            }
            other => panic!("keine Linie: {other:?}"),
        }
    }
}
