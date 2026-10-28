//! Vorschau („Geist") vor dem Platzieren.
//!
//! Gilt für Blöcke aus dem Inventar und für das Einfügen aus der Zwischenablage.
//! Bei Blöcken ist der Ankerpunkt die **untere linke Ecke** der Vorschau — sie
//! steht damit oben rechts am Mauszeiger und verdeckt ihn nicht. Bei der
//! Zwischenablage ist der Anker der mitkopierte **Basispunkt** (siehe
//! [`Clipboard`]).

use jjcad_core::geom::{Affine, BBox, Vec2 as V2};
use jjcad_core::model::{Document, Entity, EntityKind};
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
    let (liste, ab) = vorschau_durchgang(doc, &ents);
    let ctx = RenderCtx::over_entities(doc, &liste);
    let mut dl = render::DisplayList::default();
    for e in &liste[ab..] {
        render::render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    }
    GhostGeo {
        prims: dl.prims,
        bbox: dl.bbox,
        origin: g.pos,
    }
}

/// Objekte des Dokuments im Umfeld der Vorschau — Wände und Volumen.
fn umfeld_bbox(e: &Entity) -> Option<BBox> {
    match &e.kind {
        EntityKind::Wall { .. } => {
            jjcad_core::wall::Frame::of(e).map(|f| BBox::from_points(&f.band()))
        }
        EntityKind::Volume { boundary, holes, .. } => {
            let mut b = BBox::from_points(boundary);
            for h in holes {
                b.union(BBox::from_points(h));
            }
            Some(b)
        }
        _ => None,
    }
}

/// Objektliste für einen Vorschau-Durchgang und der Beginn darin, an dem die
/// Vorschauobjekte stehen.
///
/// Die Vorschauobjekte bekommen **eigene** Ids: sie tragen sonst die Id ihres
/// Originals, und der Wandindex träfe die Originalwand — die Vorschau stünde am
/// alten Platz, unter dem Zeiger wäre nichts zu sehen (T0021). Dazu kommen die
/// Wände und Volumen des Dokuments im Umfeld, damit Anschlüsse und Durchdringungen
/// so aussehen wie nach dem Einfügen.
fn vorschau_durchgang(doc: &Document, ents: &[Entity]) -> (Vec<Entity>, usize) {
    if ents.is_empty() {
        return (Vec::new(), 0);
    }
    let mut bb = BBox::EMPTY;
    let mut pad = 400.0_f64;
    for e in ents {
        bb.union(render::entity_bbox(doc, e));
        if let Some(f) = jjcad_core::wall::Frame::of(e) {
            pad = pad.max(jjcad_core::wall::MITER_LIMIT * f.h * 1.5 + 50.0);
        }
    }
    if !bb.is_valid() {
        return (Vec::new(), 0);
    }
    let umfeld = bb.expanded(pad);

    let mut liste: Vec<Entity> = doc
        .ents()
        .iter()
        .filter(|e| matches!(umfeld_bbox(e), Some(b) if b.intersects(&umfeld)))
        .cloned()
        .collect();
    let ab = liste.len();
    let mut next = liste.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    for e in ents {
        let mut e = e.clone();
        e.id = next;
        next += 1;
        liste.push(e);
    }
    (liste, ab)
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

#[cfg(test)]
mod tests_wandvorschau {
    //! T0021: Kopierte Wände brauchen eine Vorschau am Zeiger.
    //!
    //! Die kopierten Objekte tragen die **Id ihres Originals**. Der Wandindex
    //! fand darüber die Originalwand und lieferte deren Körper — die Vorschau
    //! stand also am alten Platz; unter dem Zeiger war nichts zu sehen.

    use super::*;
    use jjcad_core::model::EntityKind;

    fn wand(doc: &mut Document, a: V2, b: V2, t: f64) -> Entity {
        Entity::new(
            doc.new_id(),
            doc.current_layer,
            EntityKind::Wall {
                a,
                b,
                thickness: t,
                height: 2500.0,
                material: String::new(),
                hatch: None,
                align: Default::default(),
            },
        )
    }

    /// Eine einzelne kopierte Wand steht an der Zeigerstelle, nicht beim Original.
    #[test]
    fn kopierte_wand_steht_am_zeiger() {
        let mut doc = Document::default();
        let w = wand(&mut doc, V2::new(0.0, 0.0), V2::new(3000.0, 0.0), 100.0);
        doc.entities.push(w.clone());
        let clip = Clipboard {
            ents: vec![w],
            base: Some(V2::new(0.0, 0.0)),
        };
        let g = Ghost::new(None, V2::new(8000.0, 5000.0));
        let geo = geometry(&doc, &g, &clip);
        assert!(!geo.prims.is_empty(), "Vorschau ohne Geometrie");
        assert!(
            (geo.bbox.min.x - 8000.0).abs() < 1.0 && (geo.bbox.min.y - 4950.0).abs() < 1.0,
            "Körper der kopierten Wand sitzt am Zeiger, war {:?}",
            geo.bbox
        );
    }

    /// Zwei verbundene Wände: die Vorschau zeichnet **dieselben** Linien wie
    /// das eingefügte Ergebnis — gleiche Gehrung, keine Stirnkanten im Körper.
    #[test]
    fn vorschau_zeigt_die_gehrung_des_ergebnisses() {
        let mut doc = Document::default();
        let w1 = wand(&mut doc, V2::new(0.0, 0.0), V2::new(3000.0, 0.0), 100.0);
        doc.entities.push(w1);
        let w2 = wand(&mut doc, V2::new(0.0, 0.0), V2::new(0.0, 2000.0), 100.0);
        doc.entities.push(w2);
        let clip = Clipboard {
            ents: doc.entities.clone(),
            base: Some(V2::new(0.0, 0.0)),
        };
        let ziel = V2::new(10_000.0, 10_000.0);
        let vorschau = linien(&geometry(&doc, &Ghost::new(None, ziel), &clip).prims);

        // Ergebnis: dieselben Wände im Dokument an der Zeigerstelle
        let mut ergebnis_doc = Document::default();
        for e in &clip.ents {
            let mut e = jjcad_core::transform::transform_entity(e, &Affine::translate(ziel));
            e.id = ergebnis_doc.new_id();
            ergebnis_doc.entities.push(e);
        }
        let ctx = RenderCtx::new(&ergebnis_doc);
        let ergebnis = linien(&render::render_document(&ctx).prims);

        assert_eq!(
            vorschau.len(),
            ergebnis.len(),
            "gleiche Linienzahl in Vorschau und Ergebnis\nVorschau: {vorschau:?}\nErgebnis: {ergebnis:?}"
        );
        for (a, b) in vorschau.iter().zip(ergebnis.iter()) {
            assert!(
                a.0.dist(b.0) < 0.5 && a.1.dist(b.1) < 0.5,
                "Vorschau weicht vom Ergebnis ab: {a:?} statt {b:?}"
            );
        }
    }

    /// Linien einer Displayliste, Enden geordnet und gerundet (verglichen wird
    /// als Menge, nicht in Zeichenreihenfolge).
    fn linien(prims: &[jjcad_core::render::Prim]) -> Vec<(V2, V2)> {
        let mut out: Vec<(V2, V2)> = prims
            .iter()
            .filter_map(|p| match &p.shape {
                jjcad_core::render::Shape::Line(a, b) => Some((rund(*a), rund(*b))),
                _ => None,
            })
            .map(|(a, b)| if (a.x, a.y) <= (b.x, b.y) { (a, b) } else { (b, a) })
            .collect();
        out.sort_by(|x, y| {
            (x.0.x, x.0.y, x.1.x, x.1.y)
                .partial_cmp(&(y.0.x, y.0.y, y.1.x, y.1.y))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    fn rund(p: V2) -> V2 {
        V2::new((p.x * 100.0).round() / 100.0, (p.y * 100.0).round() / 100.0)
    }
}
