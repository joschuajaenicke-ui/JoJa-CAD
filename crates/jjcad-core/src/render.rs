//! Gemeinsamer Renderer: erzeugt aus dem Dokument eine Displayliste.
//!
//! GUI (egui), PDF-Export und PNG-Rendering arbeiten alle auf dieser Liste —
//! dadurch sehen Bildschirm, PDF und Vorschau identisch aus.

use crate::geom::*;
use crate::model::*;
use crate::style::*;
use crate::transform;
use crate::units::{dim_text, elevation_text, format_number, format_scale};
use crate::wall;
use std::cell::OnceCell;

pub use crate::wall::{
    wall_contour_points, wall_contour_segments, wall_contour_segments_idx, WallIndex, JOIN_EPS,
};

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimStyle {
    pub color: Color,
    pub line_type: LineType,
    /// Linienstärke in Papier-mm
    pub width: f64,
    /// Füllfarbe (nur bei Fill-Primitiven relevant)
    pub fill: Option<Color>,
}

impl Default for PrimStyle {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            line_type: LineType::Solid,
            width: 0.25,
            fill: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextPrim {
    pub pos: Vec2,
    pub text: String,
    /// Höhe in Weltkoordinaten (mm)
    pub height: f64,
    pub angle: f64,
    pub align: TextAlignH,
    pub valign: TextAlignV,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub superscript: bool,
    /// Gemischte Auszeichnung; leer = die Felder oben gelten für den ganzen Text.
    pub runs: Vec<TextRun>,
    /// Schriftart des Textes (Familie für Satz und Linien).
    pub font: TextFont,
}

impl TextPrim {
    /// Die Läufe, mit denen gesetzt wird: entweder die eigenen oder — ohne
    /// gemischte Auszeichnung — ein einziger Lauf aus den Feldern des Objekts.
    pub fn effective_runs(&self) -> Vec<TextRun> {
        if !self.runs.is_empty() {
            return self.runs.clone();
        }
        vec![TextRun {
            text: self.text.clone(),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            superscript: self.superscript,
        }]
    }

    /// Breite der breitesten Zeile in mm — dieselbe Rechnung wie beim Satz.
    pub fn width(&self) -> f64 {
        crate::text::width_em_mit_schrift(&self.text, &self.effective_runs(), self.font) * self.height
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line(Vec2, Vec2),
    Poly(Vec<Vec2>, bool),
    Circle {
        c: Vec2,
        r: f64,
    },
    Arc {
        c: Vec2,
        r: f64,
        a0: f64,
        a1: f64,
    },
    Ellipse {
        c: Vec2,
        rx: f64,
        ry: f64,
        rot: f64,
    },
    /// Abschnitt einer Ellipse (Ellipsenbogen): `a0`→`a1` gegen den
    /// Uhrzeigersinn, Winkel sind Parameterwinkel im ungedrehten Ellipsenraum.
    EllipseArc {
        c: Vec2,
        rx: f64,
        ry: f64,
        rot: f64,
        a0: f64,
        a1: f64,
    },
    /// Gefüllte Fläche: Außenring plus Löcher (Inseln).
    ///
    /// Gefüllt wird nach der Even-Odd-Regel — die Löcher sind Aussparungen,
    /// nicht Flächen in einer anderen Farbe. Raster und PDF schreiben dafür
    /// alle Ringe als Unterpfade, die GUI verbindet sie über Stege zu einem
    /// Polygon ([`crate::area::bridge_holes`]).
    Fill {
        outer: Vec<Vec2>,
        holes: Vec<Vec<Vec2>>,
    },
    Text(TextPrim),
    Dot {
        p: Vec2,
        r: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prim {
    pub shape: Shape,
    pub style: PrimStyle,
    pub entity: Id,
    pub layer: Id,
}

impl Prim {
    pub fn bbox(&self) -> BBox {
        match &self.shape {
            Shape::Line(a, b) => BBox::new(*a, *b),
            Shape::Poly(pts, _) => BBox::from_points(pts),
            Shape::Circle { c, r } => BBox::new(*c - v(*r, *r), *c + v(*r, *r)),
            Shape::Arc { c, r, a0, a1 } => Arc::new(*c, *r, *a0, *a1).bbox(),
            Shape::Ellipse { c, rx, ry, rot } => {
                let ex = Vec2::polar(*rot, *rx);
                let ey = Vec2::polar(*rot + std::f64::consts::FRAC_PI_2, *ry);
                let dx = (ex.x * ex.x + ey.x * ey.x).sqrt();
                let dy = (ex.y * ex.y + ey.y * ey.y).sqrt();
                BBox::new(*c - v(dx, dy), *c + v(dx, dy))
            }
            Shape::EllipseArc {
                c,
                rx,
                ry,
                rot,
                a0,
                a1,
            } => crate::geom::Ellipse::new(*c, *rx, *ry, *rot).arc_bbox(*a0, *a1),
            Shape::Fill { outer, .. } => BBox::from_points(outer),
            Shape::Dot { p, r } => BBox::new(*p - v(*r, *r), *p + v(*r, *r)),
            Shape::Text(t) => BBox::from_points(&text_corners(
                t.pos, &t.text, t.height, t.angle, t.align, t.valign,
            )),
        }
    }
}

/// Grobe Textbreite (Faktor 0,58 der Höhe je Zeichen).
///
/// Für Bemaßungs- und Kotenbeschriftungen, die mittig an ihrer Bezugslinie
/// sitzen: dort zählt nur ein gleichmäßiger Mittelwert.
pub fn estimate_text_width(text: &str, height: f64) -> f64 {
    text.chars().count() as f64 * height * 0.58
}

/// Höhe der Textzeile (Kasten um den Text) in Welt-mm.
///
/// Die Anzeige setzt die Schrift auf `Höhe × 1,32` und legt die Zeile mit
/// ihrem Zeilenabstand (× 1,2) um den Anker. Der Kasten ist deshalb höher als
/// die reine Texthöhe: sonst lägen Unterlängen (g, p, j) außerhalb des Rahmens
/// und der Rahmen schnitte die Schrift an.
pub fn text_line_height(height: f64) -> f64 {
    height * 1.4
}

/// Breite des Textkastens: enger am gezeichneten Text als
/// [`estimate_text_width`], weil der Rahmen die Schrift wirklich umschließen
/// muss.
///
/// Ein fester Faktor je Zeichen schneidet breite Buchstaben ab — „WOHNEN“ war
/// im Rahmen sichtbar zu schmal, das letzte N lag außerhalb. Deshalb grobe
/// Breitenklassen in Vielfachen der Texthöhe; lieber eine Spur zu breit als
/// durch die Buchstaben geschnitten.
pub fn text_box_width(text: &str, height: f64) -> f64 {
    // Kleiner Rand je Seite (5 % der Höhe): die Schätzung liegt bei einzelnen
    // Wörtern wenige Prozent daneben, und ein Rahmen darf die Schrift nicht
    // schneiden.
    let rand = height * 0.1;
    text.chars().map(|c| char_width_factor(c) * height).sum::<f64>() + rand
}

/// Breite eines Zeichens in Vielfachen der Texthöhe (Noto Sans, grob).
fn char_width_factor(c: char) -> f64 {
    match c {
        ' ' => 0.50,
        'i' | 'l' | 'j' | 't' | 'f' | 'r' | 'I' | '1' | '.' | ',' | ';' | ':' | '\'' | '|' | '!' => {
            0.40
        }
        'm' | 'w' => 1.05,
        'M' | 'W' => 1.15,
        c if c.is_ascii_digit() || c.is_uppercase() => 0.90,
        c if c.is_lowercase() => 0.72,
        _ => 0.60,
    }
}

/// Zahl der Zeilen eines Textes — `\n` trennt, auch leere Zeilen zählen mit.
pub fn text_lines(text: &str) -> usize {
    text.split('\n').count().max(1)
}

/// Breite und Höhe des Textkastens in Welt-mm.
///
/// Mehrzeilig bestimmt die **breiteste Zeile** die Breite (nicht die Summe
/// aller Zeichen), und die Zahl der Zeilen die Höhe. Vorher lief beides über
/// den ganzen Text: bei zwei Zeilen war der Rahmen zu breit und nur eine
/// Zeile hoch (T0025).
pub fn text_box(text: &str, height: f64) -> (f64, f64) {
    let breite = text
        .split('\n')
        .map(|zeile| text_box_width(zeile, height))
        .fold(0.0_f64, f64::max);
    (breite, text_line_height(height) * text_lines(text) as f64)
}

/// Die vier Ecken des Textkastens in Weltkoordinaten:
/// links unten, rechts unten, rechts oben, links oben.
///
/// Der Kasten hängt am Anker `pos` (waagerecht nach `align`, senkrecht nach
/// `valign`) und wird um genau diesen Anker gedreht — dieselbe Lage, die die
/// Anzeige zeichnet. Eine Quelle für Bounding-Box, Rahmen, Fang und Vorschau;
/// deshalb ist der Rahmen eines gedrehten Textes ein **gedrehtes Viereck** und
/// keine achsenparallele Box.
pub fn text_corners(
    pos: Vec2,
    text: &str,
    height: f64,
    angle: f64,
    align: TextAlignH,
    valign: TextAlignV,
) -> [Vec2; 4] {
    let (w, h) = text_box(text, height);
    let dx = match align {
        TextAlignH::Left => 0.0,
        TextAlignH::Center => -w * 0.5,
        TextAlignH::Right => -w,
    };
    let dy = match valign {
        TextAlignV::Top => -h,
        TextAlignV::Middle => -h * 0.5,
        TextAlignV::Bottom => 0.0,
    };
    let base = pos + Vec2::new(dx, dy);
    let rot = Affine::rotate_about(pos, angle);
    [
        rot.apply(base),
        rot.apply(base + Vec2::new(w, 0.0)),
        rot.apply(base + Vec2::new(w, h)),
        rot.apply(base + Vec2::new(0.0, h)),
    ]
}

#[derive(Clone, Debug, Default)]
pub struct DisplayList {
    pub prims: Vec<Prim>,
    pub bbox: BBox,
}

impl DisplayList {
    pub fn push(&mut self, p: Prim) {
        self.bbox.union(p.bbox());
        self.prims.push(p);
    }

    pub fn extend(&mut self, other: DisplayList) {
        for p in other.prims {
            self.push(p);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.prims.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Kontext
// ---------------------------------------------------------------------------

pub struct RenderCtx<'a> {
    pub doc: &'a Document,
    /// Papier-mm → Welt-mm (Zeichnungsmaßstab für Beschriftungshöhen)
    pub annot_scale: f64,
    /// Falls gesetzt, nur diese Layer rendern
    pub layer_filter: Option<Vec<Id>>,
    /// Dynamische Textfelder auflösen
    pub resolve_text: bool,
    /// Auswahl (wird für Farbgebung genutzt, hier nur durchgereicht)
    pub selection: Option<&'a [Id]>,
    /// Papierraum: Maßstab des Blattes für {MASSTAB}
    pub sheet_scale: Option<f64>,
    /// Objektliste dieses Durchgangs. `None` = aktiver Bearbeitungsraum.
    /// Ansichtsfenster und Ausgaben setzen hier ausdrücklich das Modell, damit
    /// ein gerade offener Papierraum nicht in die Zeichnung durchschlägt.
    pub entities: Option<&'a [Entity]>,
    /// Wände des Durchgangs, einmal aufgebaut (Nachbarsuche, Bänder).
    walls: OnceCell<WallIndex<'a>>,
    /// Nur dieses Gebiet braucht der Durchgang (ein Objekt). Dann wird der
    /// Wandindex nur im Umfeld gebaut statt über die ganze Zeichnung.
    wall_scope: Option<BBox>,
    /// Grundflächen aller Volumen des Durchgangs (Außenring, Inseln) — für die
    /// Anschlussregel zwischen Volumen und Wänden.
    volumes: OnceCell<Vec<(Vec<Vec2>, Vec<Vec<Vec2>>)>>,
    /// Kanten aller Wandkörper, einmal gesammelt: Fremdkanten (Volumen) werden
    /// daran unterteilt, damit nur das Stück ausfällt, das wirklich in der Wand
    /// liegt.
    wall_edges: OnceCell<Vec<(Vec2, Vec2)>>,
}

impl<'a> RenderCtx<'a> {
    pub fn new(doc: &'a Document) -> Self {
        Self {
            doc,
            annot_scale: doc.settings.annotation_scale.max(0.01),
            layer_filter: None,
            resolve_text: true,
            selection: None,
            sheet_scale: None,
            entities: None,
            walls: OnceCell::new(),
            wall_scope: None,
            volumes: OnceCell::new(),
            wall_edges: OnceCell::new(),
        }
    }

    pub fn with_scale(doc: &'a Document, scale: f64) -> Self {
        Self {
            doc,
            annot_scale: scale.max(0.01),
            layer_filter: None,
            resolve_text: true,
            selection: None,
            sheet_scale: None,
            entities: None,
            walls: OnceCell::new(),
            wall_scope: None,
            volumes: OnceCell::new(),
            wall_edges: OnceCell::new(),
        }
    }

    /// Kontext für die Zeichnung selbst — nie den Papierraum. Für Ausgaben,
    /// Exporte und Ansichtsfenster, die unabhängig vom offenen Blatt gelten.
    pub fn model(doc: &'a Document) -> Self {
        let mut c = Self::new(doc);
        c.entities = Some(&doc.entities);
        c
    }

    /// Wie `model`, mit Zeichnungsmaßstab für Beschriftungshöhen.
    pub fn model_scaled(doc: &'a Document, scale: f64) -> Self {
        let mut c = Self::with_scale(doc, scale);
        c.entities = Some(&doc.entities);
        c
    }

    /// Kontext über eine **ausdrückliche** Objektliste statt über das Dokument.
    ///
    /// Für Vorschauen bewegter oder kopierter Objekte: Wandanschlüsse,
    /// Volumen und Kanten werden dann unter genau diesen Objekten gerechnet —
    /// wie nach dem Einfügen —, nicht über die Objekte des Dokuments.
    pub fn over_entities(doc: &'a Document, ents: &'a [Entity]) -> Self {
        let mut c = Self::new(doc);
        c.entities = Some(ents);
        c
    }

    /// Objektliste des Durchgangs: ausdrücklich gesetzt, sonst der aktive Raum.
    pub fn ents(&self) -> &[Entity] {
        self.entities.unwrap_or_else(|| self.doc.ents())
    }

    /// Wandindex dieses Durchgangs — einmal gebaut, danach nur Nachbarschaft.
    /// Grundflächen aller Volumen dieses Durchgangs (mit Inseln).
    pub fn volumes(&self) -> &[(Vec<Vec2>, Vec<Vec<Vec2>>)] {
        self.volumes.get_or_init(|| {
            let ents: &'a [Entity] = self.entities.unwrap_or_else(|| self.doc.ents());
            ents.iter()
                .filter_map(|e| match &e.kind {
                    EntityKind::Volume { boundary, holes, .. } => {
                        Some((boundary.clone(), holes.clone()))
                    }
                    _ => None,
                })
                .collect()
        })
    }

    /// Liegt `p` in einem Volumen? Wandkanten fallen dort aus — so schließt
    /// das Volumen an die Wand an, ohne Trennlinie.
    pub fn in_volume(&self, p: Vec2) -> bool {
        self.volumes()
            .iter()
            .any(|(outer, holes)| crate::area::point_in_area(p, outer, holes))
    }

    /// Kanten aller Wandkörper (unsplittet) — Unterteilung für Fremdkanten.
    pub fn wall_body_edges(&self) -> &[(Vec2, Vec2)] {
        self.wall_edges.get_or_init(|| {
            let ent: &[Entity] = self.entities.unwrap_or_else(|| self.doc.ents());
            let mut out = Vec::new();
            for e in ent {
                if !matches!(e.kind, EntityKind::Wall { .. }) {
                    continue;
                }
                if let Some(b) = self.wall_index().body(e.id) {
                    for poly in &b.pieces {
                        let n = poly.len();
                        for k in 0..n {
                            out.push((poly[k], poly[(k + 1) % n]));
                        }
                    }
                }
            }
            out
        })
    }

    /// Kanten aller Volumenflächen (Außenringe und Inseln).
    pub fn volume_edges(&self) -> Vec<(Vec2, Vec2)> {
        let mut out = Vec::new();
        for (outer, holes) in self.volumes() {
            for ring in std::iter::once(outer).chain(holes.iter()) {
                let n = ring.len();
                for k in 0..n {
                    out.push((ring[k], ring[(k + 1) % n]));
                }
            }
        }
        out
    }

    /// Liegt `p` im Körper einer Wand? Umrandung eines Volumens fällt dort aus.
    pub fn in_wall(&self, p: Vec2) -> bool {
        self.wall_index().covered_any(p)
    }

    pub fn wall_index(&self) -> &WallIndex<'a> {
        self.walls.get_or_init(|| {
            let ents: &'a [Entity] = self.entities.unwrap_or_else(|| self.doc.ents());
            match self.wall_scope {
                Some(area) => WallIndex::build_near(self.doc, ents, &area),
                None => WallIndex::build(self.doc, ents),
            }
        })
    }

    /// Kontext für **ein** Objekt: der Wandindex deckt nur dessen Umfeld ab.
    /// Ohne das würde für die Bounding-Box einer einzelnen Wand der Index über
    /// die ganze Zeichnung gebaut — und `entity_bbox` läuft in Schleifen.
    pub fn for_entity(doc: &'a Document, e: &Entity) -> Self {
        let mut c = Self::new(doc);
        if let Some(f) = wall::Frame::of(e) {
            let pad = wall::MITER_LIMIT * f.h + 400.0;
            c.wall_scope = Some(crate::geom::BBox::from_points(&f.band()).expanded(pad));
        }
        c
    }

    fn layer_ok(&self, layer: Id) -> bool {
        if let Some(l) = self.doc.layer(layer) {
            if !l.visible {
                return false;
            }
        }
        match &self.layer_filter {
            Some(list) => list.contains(&layer),
            None => true,
        }
    }

    /// Papier-mm in Welt-mm
    pub fn a(&self, paper_mm: f64) -> f64 {
        paper_mm * self.annot_scale
    }
}

// ---------------------------------------------------------------------------
// Modell rendern
// ---------------------------------------------------------------------------

pub fn render_document(ctx: &RenderCtx) -> DisplayList {
    // Die Objektliste ist die Zeichenreihenfolge (Index 0 = hinten). Innerhalb
    // dieser Reihenfolge stehen **alle** Füllungen vor allen Linien: sonst
    // deckt die Wandfüllung die Umrandung einer anderen Wand zu. Untereinander
    // behalten die Füllungen ihre Stellung im Dokument — der Zeichenrang
    // entscheidet also, welche Fläche oben liegt (Wand über Füllfläche, T0050).
    // Gerendert wird der aktive Bearbeitungsraum: Modell oder Papierraum.
    let mut fills = DisplayList::default();
    let mut rest = DisplayList::default();
    for e in ctx.ents() {
        let mut tmp = DisplayList::default();
        render_entity(ctx, e, Affine::IDENTITY, &mut tmp, 0);
        for prim in tmp.prims {
            if matches!(prim.shape, Shape::Fill { .. }) {
                fills.push(prim);
            } else {
                rest.push(prim);
            }
        }
    }
    for prim in rest.prims {
        fills.push(prim);
    }
    fills
}

pub fn render_entities(ctx: &RenderCtx, ents: &[Entity]) -> DisplayList {
    let mut dl = DisplayList::default();
    for e in ents {
        render_entity(ctx, e, Affine::IDENTITY, &mut dl, 0);
    }
    dl
}

/// Vorschau einer Auswahl, um `delta` verschoben.
///
/// Grundlage für die Vorschau beim Verschieben/Kopieren: dieselbe Geometrie wie
/// die fertige Platzierung (alle Elementtypen inklusive Blockauflösung), nur um
/// `delta` versetzt. Wandfüllungen stehen wie in [`render_document`] vor allen
/// Linien, damit keine Umrandung überdeckt wird.
pub fn preview_selection(doc: &Document, ids: &[Id], delta: Vec2) -> DisplayList {
    let ctx = RenderCtx::new(doc);
    let xf = Affine::translate(delta);
    let mut fills = DisplayList::default();
    let mut rest = DisplayList::default();
    for id in ids {
        let Some(e) = doc.entity(*id) else { continue };
        let mut tmp = DisplayList::default();
        render_entity(&ctx, e, xf, &mut tmp, 0);
        // Dieselbe Ordnung wie `render_document`: alle Füllungen vor allen
        // Linien, untereinander in der Reihenfolge der Auswahl.
        for prim in tmp.prims {
            if matches!(prim.shape, Shape::Fill { .. }) {
                fills.push(prim);
            } else {
                rest.push(prim);
            }
        }
    }
    for prim in rest.prims {
        fills.push(prim);
    }
    fills
}

pub fn entity_bbox(doc: &Document, e: &Entity) -> BBox {
    let ctx = RenderCtx::for_entity(doc, e);
    let mut dl = DisplayList::default();
    render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    dl.bbox
}

/// Unsichtbarer Rahmen eines Textes: die vier Ecken der Zeilenbox
/// (links unten, rechts unten, rechts oben, links oben).
///
/// Derselbe Kasten erscheint in der Einfüge-Vorschau; bei Auswahl wird er
/// gezeichnet, und seine Ecken und Kantenmitten sind Fangpunkte. Eine Quelle
/// für Anzeige und Fang, damit Rahmen und Fangpunkt nie auseinanderlaufen.
///
/// Der Kasten ist **gedreht wie der Text** (Viereck, keine achsenparallele
/// Box) und bei mehrzeiligen Texten so hoch wie alle Zeilen zusammen.
pub fn text_frame(doc: &Document, e: &Entity) -> Option<[Vec2; 4]> {
    if !matches!(e.kind, EntityKind::Text { .. }) {
        return None;
    }
    // Über die Anzeige gehen: dort steht der aufgelöste Text samt tatsächlicher
    // Schriftgröße (Block-/Blattmaßstab) — der Rahmen kann so nicht abweichen.
    let ctx = RenderCtx::for_entity(doc, e);
    let mut dl = DisplayList::default();
    render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    for prim in &dl.prims {
        if let Shape::Text(t) = &prim.shape {
            return Some(text_corners(
                t.pos, &t.text, t.height, t.angle, t.align, t.valign,
            ));
        }
    }
    None
}

/// Kanten des Textrahmens als Segmente (für den Punktfang).
pub fn text_frame_segments(doc: &Document, e: &Entity) -> Vec<Seg> {
    let Some(k) = text_frame(doc, e) else {
        return Vec::new();
    };
    (0..4).map(|i| Seg::Line(k[i], k[(i + 1) % 4])).collect()
}

/// Bounding-Box inklusive Layer-Sichtbarkeit (für Zoom Extents).
pub fn visible_bbox(doc: &Document) -> BBox {
    let ctx = RenderCtx::new(doc);
    let mut b = BBox::EMPTY;
    for e in &doc.entities {
        if let Some(l) = doc.layer(e.layer) {
            if !l.visible {
                continue;
            }
        }
        let mut dl = DisplayList::default();
        render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
        b.union(dl.bbox);
    }
    b
}

fn base_style(doc: &Document, e: &Entity, override_color: Option<Color>) -> PrimStyle {
    PrimStyle {
        color: override_color.unwrap_or_else(|| doc.effective_color(e)),
        line_type: doc.effective_line_type(e),
        width: doc.effective_line_weight(e),
        fill: e.style.fill,
    }
}

/// Ein Objekt in die Displayliste rendern (inkl. Blockauflösung).
pub fn render_entity(
    ctx: &RenderCtx,
    e: &Entity,
    xf: Affine,
    dl: &mut DisplayList,
    depth: usize,
) {
    if depth > 12 {
        return;
    }
    if !ctx.layer_ok(e.layer) && depth == 0 {
        return;
    }
    let doc = ctx.doc;
    let st = base_style(doc, e, None);
    let a = ctx.a(1.0); // Welt-mm pro Papier-mm (Annotation)

    let push_line = |dl: &mut DisplayList, a0: Vec2, b0: Vec2| {
        dl.push(Prim {
            shape: Shape::Line(xf.apply(a0), xf.apply(b0)),
            style: st,
            entity: e.id,
            layer: e.layer,
        });
    };

    match &e.kind {
        EntityKind::Point { p } => {
            dl.push(Prim {
                shape: Shape::Dot {
                    p: xf.apply(*p),
                    r: st.width.max(0.2) * a * 1.6,
                },
                style: st,
                entity: e.id,
                layer: e.layer,
            });
        }
        EntityKind::Line { a: p0, b: p1 } => push_line(dl, *p0, *p1),
        EntityKind::Circle { c, r } => {
            dl.push(Prim {
                shape: Shape::Circle {
                    c: xf.apply(*c),
                    r: *r * xf.scale_factor(),
                },
                style: st,
                entity: e.id,
                layer: e.layer,
            });
        }
        EntityKind::Arc { c, r, a0, a1 } => {
            let rot = xf.rotation();
            let mirror = xf.is_mirrored();
            let (aa, ab) = if mirror { (-*a1, -*a0) } else { (*a0, *a1) };
            dl.push(Prim {
                shape: Shape::Arc {
                    c: xf.apply(*c),
                    r: *r * xf.scale_factor(),
                    a0: aa + rot,
                    a1: ab + rot,
                },
                style: st,
                entity: e.id,
                layer: e.layer,
            });
        }
        EntityKind::Ellipse { c, rx, ry, rot } => {
            let f = xf.scale_factor();
            dl.push(Prim {
                shape: Shape::Ellipse {
                    c: xf.apply(*c),
                    rx: *rx * f,
                    ry: *ry * f,
                    rot: *rot + xf.rotation(),
                },
                style: st,
                entity: e.id,
                layer: e.layer,
            });
        }
        // Ellipsenbogen: Parameterwinkel liegen im **ungedrehten** Ellipsenraum,
        // deshalb dreht nur `rot` mit; gespiegelt laufen die Parameter
        // andersherum (`-u`) und die Drehlage kehrt sich um.
        EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let f = xf.scale_factor();
            let uhr = xf.rotation();
            let (aa, ab, r) = if xf.is_mirrored() {
                (-*a1, -*a0, uhr - *rot)
            } else {
                (*a0, *a1, *rot + uhr)
            };
            dl.push(Prim {
                shape: Shape::EllipseArc {
                    c: xf.apply(*c),
                    rx: *rx * f,
                    ry: *ry * f,
                    rot: r,
                    a0: aa,
                    a1: ab,
                },
                style: st,
                entity: e.id,
                layer: e.layer,
            });
        }
        EntityKind::Polyline { verts, closed } => render_polyline(ctx, e, &st, verts, *closed, xf, dl),
        EntityKind::Spline { verts, closed } => render_spline(e, &st, verts, *closed, xf, dl),
        EntityKind::Fill {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
        } => render_fill(
            e,
            &st,
            boundary,
            holes,
            *pattern,
            *pattern_angle,
            *pattern_scale,
            *border,
            None,
            xf,
            dl,
        ),
        EntityKind::Volume {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
            height: _,
        } => render_volume(
            ctx,
            e,
            &st,
            boundary,
            holes,
            *pattern,
            *pattern_angle,
            *pattern_scale,
            *border,
            xf,
            dl,
        ),
        EntityKind::Text {
            p,
            text,
            height,
            angle,
            align,
            valign,
            bold,
            italic,
            underline,
            superscript,
            runs,
            font,
        } => {
            let txt = if ctx.resolve_text {
                doc.resolve_text_scaled(text, ctx.sheet_scale)
            } else {
                text.clone()
            };
            if txt.trim().is_empty() {
                return;
            }
            dl.push(Prim {
                shape: Shape::Text(TextPrim {
                    pos: xf.apply(*p),
                    text: txt,
                    height: *height * xf.scale_factor(),
                    angle: *angle + xf.rotation(),
                    align: *align,
                    valign: *valign,
                    bold: *bold || (st.width > 0.4 && runs.is_empty()),
                    italic: *italic,
                    underline: *underline,
                    superscript: *superscript,
                    runs: runs.clone(),
                    font: *font,
                }),
                style: st,
                entity: e.id,
                layer: e.layer,
            });
        }
        EntityKind::Dim {
            a: p0,
            b: p1,
            line_pos,
            axis,
            override_text,
        } => {
            render_dim(ctx, e, &st, *p0, *p1, *line_pos, *axis, override_text.as_deref(), xf, dl);
        }
        EntityKind::DimChain { pts, line_pos, axis } => {
            render_dim_chain(ctx, e, &st, pts, *line_pos, *axis, xf, dl);
        }
        EntityKind::Elevation {
            p,
            z,
            angle,
            override_text,
        } => {
            let h = ctx.a(doc.dim_style.text_height);
            let s = ctx.a(doc.dim_style.arrow_size * 1.3);
            let p = xf.apply(*p);
            let dir = Vec2::polar(*angle, 1.0);
            let n = dir.perp();
            // Bezugslinie
            dl.push(Prim {
                shape: Shape::Line(p, p + dir * (s * 2.2)),
                style: st,
                entity: e.id,
                layer: e.layer,
            });
            // Dreieck (DIN-Höhenkote)
            dl.push(Prim {
                shape: Shape::Poly(
                    vec![
                        p,
                        p - dir * s + n * (s * 0.5),
                        p - dir * s - n * (s * 0.5),
                    ],
                    true,
                ),
                style: st,
                entity: e.id,
                layer: e.layer,
            });
            let txt = override_text
                .clone()
                .unwrap_or_else(|| elevation_text(*z));
            let txt = if ctx.resolve_text {
                doc.resolve_text_scaled(&txt, ctx.sheet_scale)
            } else {
                txt
            };
            dl.push(Prim {
                shape: Shape::Text(TextPrim {
                    pos: p + dir * (s * 2.2),
                    text: txt,
                    height: h,
                    angle: 0.0,
                    align: TextAlignH::Left,
                    valign: TextAlignV::Bottom,
                    bold: false,
                    italic: false,
                    underline: false,
                    superscript: false,
                    runs: Vec::new(),
                    font: TextFont::Sans,
                }),
                style: st,
                entity: e.id,
                layer: e.layer,
            });
        }
        EntityKind::Wall {
            a: p0,
            b: p1,
            thickness,
            hatch,
            ..
        } => {
            // Ausrichtung und Anschlüsse rechnet `WallLayout::of` aus dem Objekt.
            render_wall(ctx, e, &st, *p0, *p1, *thickness, *hatch, xf, dl);
        }
        EntityKind::Door {
            center,
            angle,
            width,
            kind,
            hinge,
            swing_in,
            ..
        } => {
            render_door(ctx, e, &st, *center, *angle, *width, *kind, *hinge, *swing_in, xf, dl);
        }
        EntityKind::Window {
            center,
            angle,
            width,
            ..
        } => {
            render_window(ctx, e, &st, *center, *angle, *width, xf, dl);
        }
        EntityKind::Stair {
            a: p0,
            b: p1,
            width,
            steps,
            height,
            auto_steps,
            cut,
            unten,
            oben,
        } => {
            render_stair(
                ctx,
                e,
                &st,
                *p0,
                *p1,
                *width,
                *steps,
                *height,
                *auto_steps,
                *cut,
                unten.as_ref(),
                oben.as_ref(),
                xf,
                dl,
            );
        }
        EntityKind::BlockRef {
            block,
            pos,
            rot,
            scale,
            mirror,
        } => {
            let Some(def) = doc.block(block) else {
                // Verwaiste Referenz: Platzhalter
                let c = xf.apply(*pos);
                dl.push(Prim {
                    shape: Shape::Poly(
                        vec![
                            xf.apply(c_off(*pos, *rot, *scale, -200.0, -200.0)),
                            xf.apply(c_off(*pos, *rot, *scale, 200.0, -200.0)),
                            xf.apply(c_off(*pos, *rot, *scale, 200.0, 200.0)),
                            xf.apply(c_off(*pos, *rot, *scale, -200.0, 200.0)),
                        ],
                        true,
                    ),
                    style: PrimStyle {
                        color: Color::RED,
                        line_type: LineType::Dashed,
                        ..st
                    },
                    entity: e.id,
                    layer: e.layer,
                });
                let _ = c;
                return;
            };
            let mut m = Affine::translate(*pos);
            m = m * Affine::rotate(*rot);
            if *mirror {
                m = m * Affine::scale(-1.0, 1.0);
            }
            m = m * Affine::scale(scale.x, scale.y);
            let m = xf * m;
            let block_color = doc.effective_color(e);
            for child in &def.entities {
                let mut child_style = child.clone();
                // Die gezeichneten Grundlinien gehören zur *Instanz* — nur so
                // wird eine Auswahl (blau) und die Hervorhebung sichtbar.
                child_style.id = e.id;
                if child_style.by_block {
                    // ByBlock: Eigenschaften der Instanz erben
                    if child_style.style.color.is_none() {
                        child_style.style.color = Some(block_color);
                    }
                    if child_style.style.line_type.is_none() {
                        child_style.style.line_type = Some(st.line_type);
                    }
                    if child_style.style.line_weight.is_none() {
                        child_style.style.line_weight = Some(st.width);
                    }
                    child_style.layer = e.layer;
                }
                render_entity(ctx, &child_style, m, dl, depth + 1);
            }
        }
    }
}

fn c_off(pos: Vec2, rot: f64, scale: Vec2, x: f64, y: f64) -> Vec2 {
    pos + Vec2::new(x * scale.x, y * scale.y).rotated(rot)
}

// ---------------------------------------------------------------------------
// Füllfläche
// ---------------------------------------------------------------------------

/// Zeichnet eine Füllfläche: Fläche, Muster und optionale Randlinie.
///
/// Das ist der einzige Weg, ein gefülltes Feld zu erzeugen — Bildschirm, PDF
/// und PNG lesen anschließend dieselbe Displayliste. Randstriche werden als
/// geschlossene Polygonzüge ausgegeben, damit das Muster die Fläche nicht
/// überragt. Löcher (`holes`) sind Aussparungen: Fläche, Muster *und*
/// Randlinie lassen sie frei, damit im Loch der Untergrund zu sehen ist.
/// Aufhellung des Wandonne für die Wandfläche (Anteil Richtung Weiß).
pub const WALL_FILL_MIX: f64 = 0.74;

/// Tastabstand für „liegt jenseits dieser Kante schon Material?" (mm).
const WALL_PROBE: f64 = 0.08;

/// Strecke an `trenn`-Kanten unterteilen und Stücke auslassen, deren Mitte
/// (um `out · WALL_PROBE` nach außen) als verdeckt gilt.
///
/// Eine Kante, die nur **teilweise** in einer anderen Fläche liegt, muss
/// unterteilt werden — sonst fällt entweder zu viel weg (Lücke in der
/// Umrandung) oder zu wenig (Trennlinie bleibt stehen).
fn sichtbare_stuecke(
    a: Vec2,
    b: Vec2,
    out: Vec2,
    trenn: &[(Vec2, Vec2)],
    verdeckt: &dyn Fn(Vec2) -> bool,
) -> Vec<(Vec2, Vec2)> {
    let d = b - a;
    if d.len() < 1e-9 {
        return Vec::new();
    }
    let mut ts: Vec<f64> = vec![0.0, 1.0];
    for (r, q) in trenn {
        if let Some(x) = line_line(a, b, *r, *q, false, false) {
            let t = (x - a).dot(d) / d.len2();
            if t > 1e-6 && t < 1.0 - 1e-6 {
                ts.push(t);
            }
        }
    }
    ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let mut out_pieces = Vec::new();
    for w in ts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if t1 - t0 < 1e-9 {
            continue;
        }
        let p = a + d * t0;
        let q = a + d * t1;
        if verdeckt((p + q) * 0.5 + out * WALL_PROBE) {
            continue;
        }
        out_pieces.push((p, q));
    }
    out_pieces
}

/// Volumen zeichnen: Fläche und Muster wie eine Füllfläche, aber im Wandton
/// und mit Anschluss an die Wände.
///
/// Die Umrandung fällt aus, wo sie in einer Wand (oder in einem anderen
/// Volumen) liegt; umgekehrt blendet `render_wall` seine Kanten aus, wo sie im
/// Volumen liegen. Beide Seiten halten dieselbe Regel ein, deshalb bleibt an
/// der Naht keine Linie stehen — ohne dass Geometrie verschmolzen wird.
fn render_volume(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    boundary: &[Vec2],
    holes: &[Vec<Vec2>],
    pattern: FillPattern,
    pattern_angle: f64,
    pattern_scale: f64,
    border: bool,
    xf: Affine,
    dl: &mut DisplayList,
) {
    render_fill(
        e,
        st,
        boundary,
        holes,
        pattern,
        pattern_angle,
        pattern_scale,
        false,
        Some(st.color.mix(Color::WHITE, WALL_FILL_MIX)),
        xf,
        dl,
    );
    if !border {
        return;
    }
    let mut ringe: Vec<&[Vec2]> = vec![boundary];
    ringe.extend(holes.iter().map(|h| h.as_slice()));
    let trenn = ctx.wall_body_edges().to_vec();
    for ring in ringe {
        let n = ring.len();
        if n < 2 {
            continue;
        }
        for k in 0..n {
            let p = ring[k];
            let q = ring[(k + 1) % n];
            if p.dist(q) < 0.01 {
                continue;
            }
            let mut out = (q - p).normalized().perp();
            let mid = (p + q) * 0.5;
            // Außenseite des Rings bestimmen (Inseln laufen andersherum)
            if point_in_polygon(mid + out * 0.5, ring) {
                out = -out;
            }
            for (a, b) in sichtbare_stuecke(p, q, out, &trenn, &|pt| {
                ctx.in_wall(pt) || ctx.in_volume(pt)
            }) {
                dl.push(Prim {
                    shape: Shape::Line(xf.apply(a), xf.apply(b)),
                    style: *st,
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
    }
}

pub fn render_fill(
    e: &Entity,
    st: &PrimStyle,
    boundary: &[Vec2],
    holes: &[Vec<Vec2>],
    pattern: FillPattern,
    pattern_angle: f64,
    pattern_scale: f64,
    border: bool,
    // Standardton für die Vollfläche, wenn keine Füllfarbe gesetzt ist
    // (`None` = aufgehellte Linienfarbe einer Fläche). Ein Volumen gibt hier
    // den **Wandton** an, damit beide Flächen gleich aussehen.
    shade: Option<Color>,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let pts: Vec<Vec2> = boundary.iter().map(|p| xf.apply(*p)).collect();
    if pts.len() < 3 {
        return;
    }
    // Ringe, die keine Fläche sind (entartet), fallen weg.
    let loecher: Vec<Vec<Vec2>> = holes
        .iter()
        .map(|h| h.iter().map(|p| xf.apply(*p)).collect::<Vec<Vec2>>())
        .filter(|h| h.len() >= 3)
        .collect();
    let scale = xf.scale_factor().abs().max(1e-6);

    // Fläche: eine gesetzte Füllfarbe gewinnt immer. Ohne sie wird nur bei
    // Vollfläche aufgehellt — ein Muster mit Flächenfarbe bleibt sonst ein
    // grauer Kasten unter den Linien.
    let area = match (pattern, e.style.fill) {
        (_, Some(c)) => Some(c),
        (FillPattern::Solid, None) => {
            Some(shade.unwrap_or_else(|| st.color.mix(Color::WHITE, 0.72)))
        }
        _ => None,
    };
    if let Some(c) = area {
        dl.push(Prim {
            shape: Shape::Fill {
                outer: pts.clone(),
                holes: loecher.clone(),
            },
            style: PrimStyle {
                color: c,
                fill: Some(c),
                ..*st
            },
            entity: e.id,
            layer: e.layer,
        });
    }

    if pattern != FillPattern::Solid {
        let thin = PrimStyle {
            color: st.color,
            fill: None,
            width: st.width * 0.6,
            ..*st
        };
        let spacing = (pattern_scale * scale).max(0.5);
        let angle = pattern_angle + xf.rotation();
        push_muster(dl, e, &thin, &pts, &loecher, pattern, angle, spacing);
    }

    if border {
        // Randlinie: Außenring und jede Lochkante — die Insel muss sichtbar
        // umrandet sein, sonst wirkt sie wie ein Loch im Papier.
        dl.push(Prim {
            shape: Shape::Poly(pts.clone(), true),
            style: PrimStyle { fill: None, ..*st },
            entity: e.id,
            layer: e.layer,
        });
        for h in &loecher {
            dl.push(Prim {
                shape: Shape::Poly(h.clone(), true),
                style: PrimStyle { fill: None, ..*st },
                entity: e.id,
                layer: e.layer,
            });
        }
    }
}

/// Bereich einer Kontur als Bounding-Box-Maß (Diagonale).
fn fill_diag(pts: &[Vec2]) -> f64 {
    if pts.is_empty() {
        return 0.0;
    }
    let bb = BBox::from_points(pts);
    (bb.max.x - bb.min.x).hypot(bb.max.y - bb.min.y)
}

// ---------------------------------------------------------------------------
// Muster (DIN 1356-1:2026-09, Tabelle 5)
// ---------------------------------------------------------------------------
//
// Alle Muster entstehen hier — Füllfläche, Volumen und Wandschraffur rufen
// `push_muster` auf. Die Figuren (Striche, Punkte, Wellen, Schleifen, Felder)
// werden an der Kontur beschnitten; Löcher bleiben frei.

/// Musterstück als Linie in die Displayliste legen.
fn muster_linie(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    a: Vec2,
    b: Vec2,
) {
    if a.dist(b) < 0.05 {
        return;
    }
    dl.push(Prim {
        shape: Shape::Line(a, b),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
}

/// Eine Strecke an der Kontur beschneiden und als Musterlinie(n) ausgeben.
fn muster_abschnitt(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    a: Vec2,
    b: Vec2,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
) {
    for (q0, q1) in crate::area::clip_line_area(a, b, pts, holes) {
        muster_linie(dl, e, st, q0, q1);
    }
}

/// Feste Streuung 0…1 aus dem Rasterindex. Unregelmäßige Muster (Sand, Putz,
/// Kies) sehen damit gestreut aus, bleiben aber bei jedem Aufbau gleich —
/// ein Zufallsgenerator würde das Bild bei jedem Zoom neu würfeln.
fn streuung(i: i64, j: i64, salz: u64) -> f64 {
    let mut h = (i as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .wrapping_add(salz.wrapping_mul(0x1656_67B1_9E37_79F9));
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 29;
    (h % 10_000) as f64 / 10_000.0
}

/// Muster über eine Kontur legen — **die eine** Stelle für alle Muster.
///
/// `spacing` ist der Musterabstand (Objektwert, in mm); jede Figur rechnet
/// ihre Abmessungen daraus ab. `angle` ist die Musterrichtung inklusive
/// Objektwinkel und Ansichtsdrehung.
fn push_muster(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    pattern: FillPattern,
    angle: f64,
    spacing: f64,
) {
    let s = spacing.max(0.5);
    match pattern {
        FillPattern::Solid => {}
        // Nr. 9 Mauerwerk: eine Linienschar im Musterwinkel
        FillPattern::Lines => muster_linien(dl, e, st, pts, holes, &[angle], s, None, 0.0, 0.0),
        // Nr. 17 Kunststoffe: zwei Linienscharen im rechten Winkel
        FillPattern::Cross => muster_linien(
            dl,
            e,
            st,
            pts,
            holes,
            &[angle, angle + std::f64::consts::FRAC_PI_2],
            s,
            None,
            0.0,
            0.0,
        ),
        // Nr. 4 Sand: feine, gestreute Punkte
        FillPattern::Dots => muster_punkte(dl, e, st, pts, holes, angle, s, 1.0, 1.0, 0.035, 0.55),
        // Nr. 13 Putz: gröbere, gestreute Punkte
        FillPattern::Plaster => {
            muster_punkte(dl, e, st, pts, holes, angle, s * 1.9, 1.0, 1.9, 0.07, 0.6)
        }
        // Nr. 14 Dämmstoffe, Zickzackform: Mäanderlinie
        FillPattern::Insulation => muster_wellen(dl, e, st, pts, holes, angle, s, 1.0, 0.45, 2.0),
        // Nr. 1 Boden unverändert: Zickzackschraffur
        FillPattern::GroundPlain => muster_zickzack(dl, e, st, pts, holes, angle, s),
        // Nr. 2 Boden verändert: Längsstriche mit Querstrichen
        FillPattern::GroundChanged => muster_boden_veraendert(dl, e, st, pts, holes, angle, s),
        // Nr. 3 Kies: Kieselsteine
        FillPattern::Gravel => muster_kiesel(dl, e, st, pts, holes, angle, s),
        // Nr. 5 Beton unbewehrt: gestrichelte Linien, Reihen versetzt
        FillPattern::ConcretePlain => muster_linien(
            dl,
            e,
            st,
            pts,
            holes,
            &[angle],
            s,
            Some(&[(0.85, 0.55)]),
            0.0,
            0.5,
        ),
        // Nr. 6 Beton bewehrt: durchgehende Linien, dazwischen kurze Striche
        FillPattern::ConcreteReinforced => {
            muster_linien(dl, e, st, pts, holes, &[angle], s * 2.0, None, 0.0, 0.0);
            muster_linien(
                dl,
                e,
                st,
                pts,
                holes,
                &[angle],
                s * 2.0,
                Some(&[(0.9, 0.6)]),
                0.5,
                0.0,
            );
        }
        // Nr. 7 WU-Beton: durchgehende Linien, dazwischen lange Striche
        FillPattern::ConcreteWatertight => {
            muster_linien(dl, e, st, pts, holes, &[angle], s * 2.0, None, 0.0, 0.0);
            muster_linien(
                dl,
                e,
                st,
                pts,
                holes,
                &[angle],
                s * 2.0,
                Some(&[(1.55, 0.45)]),
                0.5,
                0.0,
            );
        }
        // Nr. 8 Textilbeton: Linien mit Pfeilen dazwischen
        FillPattern::TextileConcrete => {
            muster_linien(dl, e, st, pts, holes, &[angle], s * 2.0, None, 0.0, 0.0);
            muster_pfeile(dl, e, st, pts, holes, angle, s);
        }
        // Nr. 10 Vollholz quer: gewellte Linien quer zum Musterwinkel
        FillPattern::WoodAcross => {
            muster_wellen(dl, e, st, pts, holes, angle, s, 1.0, 0.10, 2.6)
        }
        // Nr. 11 Vollholz längs: gewellte Linien waagerecht (Vorgabe 45° − 45°)
        FillPattern::WoodAlong => muster_wellen(
            dl,
            e,
            st,
            pts,
            holes,
            angle - std::f64::consts::FRAC_PI_4,
            s,
            1.0,
            0.10,
            2.6,
        ),
        // Nr. 12 Metall: weite Linien, dazwischen kurze Striche
        FillPattern::Metal => {
            muster_linien(dl, e, st, pts, holes, &[angle], s * 2.0, None, 0.0, 0.0);
            muster_linien(
                dl,
                e,
                st,
                pts,
                holes,
                &[angle],
                s * 2.0,
                Some(&[(1.25, 1.55)]),
                0.5,
                0.0,
            );
        }
        // Nr. 14 Dämmstoffe, Schlaufenform: Reihen geschlossener Schleifen
        FillPattern::InsulationLoops => muster_schlaufen(dl, e, st, pts, holes, angle, s),
        // Nr. 15 Abdichtungen: einlagig / mehrlagig / mit Aufkantung
        FillPattern::SealSingle => muster_abdichtung(
            dl,
            e,
            st,
            pts,
            holes,
            angle - std::f64::consts::FRAC_PI_4,
            s,
            1,
            false,
        ),
        FillPattern::SealMulti => muster_abdichtung(
            dl,
            e,
            st,
            pts,
            holes,
            angle - std::f64::consts::FRAC_PI_4,
            s,
            2,
            false,
        ),
        FillPattern::SealUpstand => muster_abdichtung(
            dl,
            e,
            st,
            pts,
            holes,
            angle - std::f64::consts::FRAC_PI_4,
            s,
            2,
            true,
        ),
        // Nr. 16 Dichtstoffe: enges Kreuzraster
        FillPattern::Sealant => muster_linien(
            dl,
            e,
            st,
            pts,
            holes,
            &[angle, angle + std::f64::consts::FRAC_PI_2],
            s * 0.45,
            None,
            0.0,
            0.0,
        ),
        // Nr. 18 Glas: Gruppen aus drei kurzen Strichen
        FillPattern::Glass => muster_linien(
            dl,
            e,
            st,
            pts,
            holes,
            &[angle],
            s,
            Some(&[(0.45, 0.12), (0.45, 0.12), (0.45, 1.05)]),
            0.0,
            0.5,
        ),
    }
}

/// Parallele Musterlinien über eine Kontur, optional gestrichelt.
///
/// `versatz` ist die Lage der ersten Linie in Einheiten von `spacing`
/// (0,5 legt sie zwischen zwei Linien), `staffel` versetzt das Strichmuster
/// jeder zweiten Reihe — so entstehen die gestaffelten Reihen der Norm.
#[allow(clippy::too_many_arguments)]
fn muster_linien(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angles: &[f64],
    spacing: f64,
    striche: Option<&[(f64, f64)]>,
    versatz: f64,
    staffel: f64,
) {
    let spacing = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let reach = fill_diag(pts) * 0.5 + spacing;
    // Genug Linien für die ganze Kontur, aber begrenzt — ein sehr kleiner
    // Abstand darf das Zeichnen nicht lahmlegen.
    let n = ((reach / spacing).ceil() as i64 + 2).clamp(1, 4000);
    for &ang in angles {
        let nrm = Vec2::polar(ang + std::f64::consts::FRAC_PI_2, 1.0);
        let dir = Vec2::polar(ang, 1.0);
        for i in -n..=n {
            let off = nrm * ((i as f64 + versatz) * spacing);
            let a = origin + off - dir * reach;
            let b = origin + off + dir * reach;
            let phase = if i.rem_euclid(2) == 1 { staffel } else { 0.0 };
            for (q0, q1) in crate::area::clip_line_area(a, b, pts, holes) {
                match striche {
                    Some(muster) => muster_striche(dl, e, st, q0, q1, spacing, muster, phase),
                    None => muster_linie(dl, e, st, q0, q1),
                }
            }
        }
    }
}

/// Strichmuster auf einer Strecke: `muster` ist eine Folge aus (Strich, Lücke)
/// in Einheiten von `spacing`. Mehr als ein Paar ergibt Gruppen (Glas: drei
/// Striche, dann eine lange Lücke).
fn muster_striche(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    a: Vec2,
    b: Vec2,
    spacing: f64,
    muster: &[(f64, f64)],
    phase: f64,
) {
    let len = a.dist(b);
    if len < 0.05 || muster.is_empty() {
        return;
    }
    let zyklus: f64 = muster.iter().map(|(an, aus)| an + aus).sum::<f64>() * spacing;
    if zyklus < 1e-6 {
        return;
    }
    let dir = (b - a).normalized();
    let mut t = -(phase * zyklus);
    let mut idx = 0usize;
    let mut schutz = 0;
    while t < len && schutz < 20_000 {
        schutz += 1;
        let (an, aus) = muster[idx % muster.len()];
        let t0 = t.max(0.0);
        let t1 = (t + an * spacing).min(len);
        if t1 > t0 {
            muster_linie(dl, e, st, a + dir * t0, a + dir * t1);
        }
        t += (an + aus) * spacing;
        idx += 1;
    }
}

/// Punktraster im gedrehten Musterraster, nur innerhalb der Kontur.
///
/// `reihen` ist der Reihenabstand in Einheiten von `spacing`, `groesse` der
/// Punktradius in Einheiten von `spacing` (vor der Streuung), `streu` der
/// Anteil der Rasterweite, um den ein Punkt wandert.
#[allow(clippy::too_many_arguments)]
fn muster_punkte(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
    reihen: f64,
    groesse: f64,
    radius: f64,
    streu: f64,
) {
    let spacing = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    let reihe = (spacing * reihen).max(0.5);
    let half = fill_diag(pts) * 0.5 + spacing;
    // Rasterweite notfalls vergröbern, damit eine große Fläche bei kleinem
    // Abstand nicht zehntausende Punkte erzeugt.
    let mut sp = spacing;
    while (2.0 * half / sp).powi(2) > 30_000.0 {
        sp *= 1.4;
    }
    let r = ((sp * radius) * groesse.max(0.1)).max(st.width * 0.9).max(0.05);
    let n = ((half / sp).ceil() as i64 + 3).clamp(1, 4000);
    let m = ((half / reihe).ceil() as i64 + 3).clamp(1, 4000);
    for i in -n..=n {
        for j in -m..=m {
            let jx = if streu > 0.0 {
                (streuung(i, j, 1) - 0.5) * streu * sp
            } else {
                0.0
            };
            let jy = if streu > 0.0 {
                (streuung(i, j, 2) - 0.5) * streu * reihe
            } else {
                0.0
            };
            let p = origin + dir * (i as f64 * sp + jx) + nrm * (j as f64 * reihe + jy);
            if !crate::area::point_in_area(p, pts, holes) {
                continue;
            }
            let rr = if streu > 0.0 {
                r * (0.7 + 0.6 * streuung(i, j, 3))
            } else {
                r
            };
            dl.push(Prim {
                shape: Shape::Dot { p, r: rr },
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
        }
    }
}

/// Wellenlinien entlang der Musterrichtung, am Polygon beschnitten —
/// Dämmstoffe (Zickzackform) und Vollholz. `amp` ist die Amplitude, `periode`
/// die Wellenlänge, beide in Einheiten von `spacing`.
fn muster_wellen(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
    reihen: f64,
    amp: f64,
    periode: f64,
) {
    let spacing = (spacing * reihen).max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    let mut spacing = spacing;
    let mut half = fill_diag(pts) * 0.5 + spacing;
    // Die Zahl der Wellensegmente begrenzen (die Displayliste wird je Stand
    // einmal gebaut).
    while (2.0 * half / spacing) * (2.0 * half / (spacing * periode / 10.0)) > 40_000.0 {
        spacing *= 1.5;
        half = fill_diag(pts) * 0.5 + spacing;
    }
    let amplitude = spacing * amp;
    let period = spacing * periode.max(0.5);
    let step = (period / 10.0).max(half / 400.0).max(0.2);
    let steps = ((half * 2.0) / step).ceil() as i64;
    let n = ((half / spacing).ceil() as i64 + 1).clamp(1, 400);
    for i in -n..=n {
        let base = origin + nrm * (i as f64 * spacing);
        let mut prev: Option<Vec2> = None;
        for k in 0..=steps {
            let s = -half + k as f64 * step;
            let off = ((s / period) * std::f64::consts::TAU).sin() * amplitude;
            let p = base + dir * s + nrm * off;
            if let Some(q) = prev {
                muster_abschnitt(dl, e, st, q, p, pts, holes);
            }
            prev = Some(p);
        }
    }
}

/// Nr. 1 Boden/Gelände unverändert: Zickzackschraffur (Dreiecke) unter der
/// Geländekante.
fn muster_zickzack(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
) {
    let s = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    let reihe = s * 1.3;
    let half = fill_diag(pts) * 0.5 + reihe;
    let zahn = s;
    let amp = s * 0.75;
    let m = ((half / reihe).ceil() as i64 + 1).clamp(1, 400);
    let n = ((half / (zahn * 0.5)).ceil() as i64 + 2).clamp(1, 4000);
    for j in -m..=m {
        let base = origin + nrm * (j as f64 * reihe);
        let mut prev: Option<Vec2> = None;
        for k in -n..=n {
            let x = k as f64 * zahn * 0.5;
            let y = if k.rem_euclid(2) == 0 { -amp } else { amp };
            let p = base + dir * x + nrm * y;
            if let Some(q) = prev {
                muster_abschnitt(dl, e, st, q, p, pts, holes);
            }
            prev = Some(p);
        }
    }
}

/// Nr. 2 Boden/Gelände verändert: Längsstriche zwischen Querstrichen.
fn muster_boden_veraendert(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
) {
    let s = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    let reihe = s * 2.0;
    let half = fill_diag(pts) * 0.5 + reihe;
    let m = ((half / reihe).ceil() as i64 + 1).clamp(1, 400);
    let n = ((half / s).ceil() as i64 + 2).clamp(1, 4000);
    for j in -m..=m {
        let base = origin + nrm * (j as f64 * reihe);
        for k in -n..=n {
            let p = base + dir * (k as f64 * s);
            // Querstrich über die halbe Reihenhöhe
            muster_abschnitt(dl, e, st, p - nrm * (s * 0.55), p + nrm * (s * 0.55), pts, holes);
            // Zwei Längsstriche in der folgenden Zelle
            for v in [-0.28f64, 0.28] {
                let a = p + dir * (s * 0.22) + nrm * (s * v);
                let b = p + dir * (s * 0.78) + nrm * (s * v);
                muster_abschnitt(dl, e, st, a, b, pts, holes);
            }
        }
    }
}

/// Nr. 3 Kies: Kieselsteine als geschlossene Kreise wechselnder Größe.
fn muster_kiesel(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
) {
    let s = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    let reihe = s * 1.5;
    let sp = s * 1.5;
    let half = fill_diag(pts) * 0.5 + sp;
    let n = ((half / sp).ceil() as i64 + 2).clamp(1, 400);
    let m = ((half / reihe).ceil() as i64 + 2).clamp(1, 2000);
    for i in -n..=n {
        for j in -m..=m {
            let jx = (streuung(i, j, 4) - 0.5) * sp * 0.5;
            let jy = (streuung(i, j, 5) - 0.5) * reihe * 0.5;
            let p = origin + dir * (i as f64 * sp + jx) + nrm * (j as f64 * reihe + jy);
            if !crate::area::point_in_area(p, pts, holes) {
                continue;
            }
            let r = s * (0.34 + 0.34 * streuung(i, j, 6));
            // Nur Steine zeichnen, die ganz in der Fläche liegen — sonst ragen
            // sie über die Umgrenzung hinaus.
            let passt = (0..8).all(|k| {
                let a = k as f64 / 8.0 * std::f64::consts::TAU;
                crate::area::point_in_area(p + Vec2::polar(a, r), pts, holes)
            });
            if !passt {
                continue;
            }
            dl.push(Prim {
                shape: Shape::Circle { c: p, r },
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
        }
    }
}

/// Nr. 8 Textilbeton: Pfeile zwischen den Musterlinien, in Musterrichtung.
fn muster_pfeile(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
) {
    let s = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    let reihe = s * 2.0;
    let half = fill_diag(pts) * 0.5 + reihe;
    let m = ((half / reihe).ceil() as i64 + 1).clamp(1, 400);
    let n = ((half / reihe).ceil() as i64 + 2).clamp(1, 400);
    for j in -m..=m {
        for i in -n..=n {
            let p = origin
                + nrm * ((j as f64 + 0.5) * reihe)
                + dir * ((i as f64 + 0.5) * reihe);
            if !crate::area::point_in_area(p, pts, holes) {
                continue;
            }
            let lang = s * 0.55;
            let spitz = p + dir * (lang * 0.5);
            let fuss = p - dir * (lang * 0.5);
            muster_abschnitt(dl, e, st, fuss, spitz, pts, holes);
            for seite in [-1.0f64, 1.0] {
                let ende = spitz - Vec2::polar(angle + seite * 0.7, lang * 0.42);
                muster_abschnitt(dl, e, st, spitz, ende, pts, holes);
            }
        }
    }
}

/// Nr. 14 Dämmstoffe, Schlaufenform: Reihen geschlossener Schlaufen.
fn muster_schlaufen(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
) {
    let s = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    // Eine Reihe ist ein Band aus aneinanderstoßenden Schlaufen: Schlaufe
    // 0,9 s breit, 1,9 s hoch, Reihen im Abstand einer Schlaufe.
    let rx = s * 0.46;
    let ry = s * 0.95;
    let schritt = rx * 2.0;
    let reihe = ry * 2.0;
    let half = fill_diag(pts) * 0.5 + reihe;
    let m = ((half / reihe).ceil() as i64 + 1).clamp(1, 400);
    let n = ((half / schritt).ceil() as i64 + 2).clamp(1, 2000);
    for j in -m..=m {
        for i in -n..=n {
            let p = origin + nrm * (j as f64 * reihe) + dir * (i as f64 * schritt);
            // Schlaufen nur ganz zeichnen — eine angeschnittene Schleife sähe
            // an der Umgrenzung wie ein Fehler aus.
            let passt = (0..8).all(|k| {
                let a = k as f64 / 8.0 * std::f64::consts::TAU;
                let q = p + dir * (rx * a.cos()) + nrm * (ry * a.sin());
                crate::area::point_in_area(q, pts, holes)
            });
            if !passt {
                continue;
            }
            dl.push(Prim {
                shape: Shape::Ellipse {
                    c: p,
                    rx,
                    ry,
                    rot: angle,
                },
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
        }
    }
}

/// Nr. 15 Abdichtungen: Bänder aus Feldern, die abwechselnd voll sind.
///
/// Die Norm zeichnet ein Band entlang der abgedichteten Fläche (einlagig eine
/// Lage, mehrlagig zwei versetzte Lagen, Aufkantung mit betonten Kanten). In
/// einer Füllfläche wiederholt sich das Band im Musterabstand; die Bandlängs-
/// richtung ist bei Vorgabe **waagerecht** (Musterwinkel − 45°), denn eine
/// Abdichtung liegt auf der Bauteilfläche, nicht schräg dazu.
fn muster_abdichtung(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
    lagen: usize,
    kanten: bool,
) {
    let s = spacing.max(0.5);
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    // Lagenhöhe, Abstand zweier Lagen und Zellenlänge in Musterrichtung.
    let lage_h = s * 0.42;
    let lage_abstand = lage_h + s * 0.05;
    let zelle = s * 1.3;
    let band = lage_h * lagen as f64 + lage_abstand * (lagen.saturating_sub(1)) as f64;
    let abstand = band + s * 0.9;
    let half = fill_diag(pts) * 0.5 + abstand;
    let m = ((half / abstand).ceil() as i64 + 1).clamp(1, 400);
    let n = ((half / zelle).ceil() as i64 + 2).clamp(1, 2000);
    let mut gefuellt = *st;
    gefuellt.fill = Some(st.color);
    for j in -m..=m {
        let band_anfang = j as f64 * abstand;
        for lage in 0..lagen {
            let y0 = band_anfang + lage as f64 * lage_abstand;
            // Die zweite Lage liegt um eine halbe Zelle versetzt.
            let versatz = if lage % 2 == 1 { 0.5 } else { 0.0 };
            for i in -n - 1..=n {
                let x0 = (i as f64 + versatz) * zelle;
                let ecken = [
                    origin + dir * x0 + nrm * y0,
                    origin + dir * (x0 + zelle) + nrm * y0,
                    origin + dir * (x0 + zelle) + nrm * (y0 + lage_h),
                    origin + dir * x0 + nrm * (y0 + lage_h),
                ];
                let drin = ecken
                    .iter()
                    .all(|q| crate::area::point_in_area(*q, pts, holes));
                if !drin {
                    continue;
                }
                // Die weißen Felder der Norm sind umrandet, jedes zweite Feld
                // ist voll.
                for k in 0..4 {
                    muster_linie(dl, e, st, ecken[k], ecken[(k + 1) % 4]);
                }
                if i.rem_euclid(2) == 0 {
                    dl.push(Prim {
                        shape: Shape::Fill {
                            outer: ecken.to_vec(),
                            holes: Vec::new(),
                        },
                        style: gefuellt,
                        entity: e.id,
                        layer: e.layer,
                    });
                }
            }
        }
        if kanten {
            // Aufkantung: die Bandkanten laufen als durchgehende Linie mit.
            for y in [band_anfang, band_anfang + band] {
                let a = origin - dir * half + nrm * y;
                let b = origin + dir * half + nrm * y;
                muster_abschnitt(dl, e, st, a, b, pts, holes);
            }
        }
    }
}

fn render_polyline(
    _ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    verts: &[Vertex],
    closed: bool,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let pl = Polyline {
        verts: verts.to_vec(),
        closed,
    };
    for s in pl.segments() {
        match s {
            Seg::Line(a, b) => dl.push(Prim {
                shape: Shape::Line(xf.apply(a), xf.apply(b)),
                style: *st,
                entity: e.id,
                layer: e.layer,
            }),
            Seg::Arc(arc) => {
                let mirror = xf.is_mirrored();
                let (a0, a1) = if mirror { (-arc.a1, -arc.a0) } else { (arc.a0, arc.a1) };
                dl.push(Prim {
                    shape: Shape::Arc {
                        c: xf.apply(arc.c),
                        r: arc.r * xf.scale_factor(),
                        a0: a0 + xf.rotation(),
                        a1: a1 + xf.rotation(),
                    },
                    style: *st,
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
    }
}

/// Spline zeichnen: die Catmull-Rom-Kurve wird in eine Punktfolge zerlegt und
/// als **ein** Polygonzug ausgegeben (eine Glättung, keine Stöße zwischen
/// Teilstrecken). Stützpunkte selbst sind keine Displaylist-Primitive — sie
/// erscheinen nur als Griffe in der Auswahl.
fn render_spline(
    e: &Entity,
    st: &PrimStyle,
    verts: &[Vertex],
    closed: bool,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let sp = Spline {
        verts: verts.to_vec(),
        closed,
    };
    let pts: Vec<Vec2> = sp.flatten(SPLINE_TOL_MM).into_iter().map(|p| xf.apply(p)).collect();
    if pts.len() < 2 {
        return;
    }
    dl.push(Prim {
        shape: Shape::Poly(pts, closed),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
}

// ---------------------------------------------------------------------------
// Wand
// ---------------------------------------------------------------------------

/// Öffnungsintervalle (Start/Ende entlang der Wandachse) für eine Wand.
pub fn wall_opening_intervals(doc: &Document, wall: &Entity) -> Vec<(f64, f64)> {
    let EntityKind::Wall { a, b, .. } = &wall.kind else {
        return vec![];
    };
    let dir = (*b - *a).normalized();
    let len = a.dist(*b);
    let mut out = Vec::new();
    for e in &doc.entities {
        let (center, angle, width, host) = match &e.kind {
            EntityKind::Door {
                center,
                angle,
                width,
                host,
                ..
            } => (*center, *angle, *width, *host),
            EntityKind::Window {
                center,
                angle,
                width,
                host,
                ..
            } => (*center, *angle, *width, *host),
            _ => continue,
        };
        if host != Some(wall.id) {
            continue;
        }
        // Nur wenn die Öffnung tatsächlich auf der Achse liegt
        if dist_point_line(center, *a, *b) > 2000.0 {
            continue;
        }
        let _ = angle;
        let t = (center - *a).dot(dir);
        let hw = width * 0.5;
        let s0 = (t - hw).clamp(0.0, len);
        let s1 = (t + hw).clamp(0.0, len);
        if s1 > s0 + 1.0 {
            out.push((s0, s1));
        }
    }
    out.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}



/// Wandkörper zeichnen: Füllflächen, Schraffur und die sichtbare Umrandung.
///
/// Die Geometrie kommt vollständig aus [`crate::wall`] — Körper (an den
/// Anschlussnähten beschnitten) und Randstücke (nur dort, wo außen keine
/// andere Wand liegt). Hier gibt es deshalb keine Sonderfälle mehr für
/// Gehrung, Stumpfstoß, Stich oder Kreuz.
fn render_wall(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    a: Vec2,
    b: Vec2,
    thickness: f64,
    hatch: Option<FillPattern>,
    xf: Affine,
    dl: &mut DisplayList,
) {
    if thickness <= 0.5 {
        dl.push(Prim {
            shape: Shape::Line(xf.apply(a), xf.apply(b)),
            style: *st,
            entity: e.id,
            layer: e.layer,
        });
        return;
    }
    let idx = ctx.wall_index();
    // Wand nicht im Index oder nicht **diese** Fassung (Vorschau, Blockinhalt):
    // als Einzelwand ohne Nachbarn zeichnen. `body_of` statt `body`, weil eine
    // kopierte oder verschobene Wand noch die Id ihres Originals trägt — sonst
    // zeichnete die Vorschau den Körper der Originalwand (T0021).
    let iso;
    let body = match idx.body_of(e) {
        Some(b) => b,
        None => match wall::Body::isolated(ctx.doc, e) {
            Some(b) => {
                iso = b;
                &iso
            }
            None => return,
        },
    };
    let edges = if idx.body_of(e).is_some() {
        idx.edges(e.id, true)
    } else {
        body.edges()
    };

    let fill = e
        .style
        .fill
        .unwrap_or_else(|| st.color.mix(Color::WHITE, WALL_FILL_MIX));
    let fill_style = PrimStyle {
        color: fill,
        fill: Some(fill),
        width: 0.0,
        ..*st
    };

    // 1. Füllflächen — der Wandkörper, an den Nähten in die Nachbarwand
    //    ausgebucht (überlappt dort, kein Antialiasing-Haarstrich).
    for poly in &body.fills {
        dl.push(Prim {
            shape: Shape::Fill {
                outer: poly.iter().map(|q| xf.apply(*q)).collect(),
                holes: Vec::new(),
            },
            style: fill_style,
            entity: e.id,
            layer: e.layer,
        });
    }
    // 2. Kein eigenes Band an der Naht: die Füllflächen selbst sind dort um
    //    `FILL_BLEED` in die Nachbarwand ausgebucht (`wall::Body::fills`) und
    //    überlappen. Eine zusätzliche Auflage war entweder als eigene Kante zu
    //    sehen (breitenabhängig getastet) oder unterpixelbreit und wurde von
    //    der GUI verworfen — beides erzeugte den Haarstrich.
    // 3. Schraffur der Wandfläche
    if let Some(hp) = hatch {
        push_wall_hatch(dl, e, st, hp, body, thickness, xf);
    }
    // 4. Umrandung: nur die Randstücke, deren Außenseite frei ist. Ein Stück,
    //    das in einem **Volumen** liegt, fällt ebenfalls aus — so schließen
    //    Volumen und Wand ohne Trennlinie aneinander an. Die Kante wird dafür
    //    an den Volumenrändern unterteilt: nur das Stück im Volumen entfällt.
    let vkanten = ctx.volume_edges();
    for ed in edges.iter().filter(|p| !p.covered) {
        for (p, q) in sichtbare_stuecke(ed.a, ed.b, ed.out, &vkanten, &|pt| ctx.in_volume(pt)) {
            dl.push(Prim {
                shape: Shape::Line(xf.apply(p), xf.apply(q)),
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
        }
    }
}

/// Schraffur (Muster) der Wandfläche, beschnitten auf die Wandkörper.
fn push_wall_hatch(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    hp: FillPattern,
    body: &wall::Body,
    thickness: f64,
    xf: Affine,
) {
    if hp == FillPattern::Solid {
        return;
    }
    let thin = PrimStyle {
        width: st.width * 0.55,
        ..*st
    };
    let (h, n, dir, start) = (body.h, body.n, body.dir, body.start);
    let flechen: Vec<Vec<Vec2>> = body
        .pieces
        .iter()
        .map(|p| p.iter().map(|q| xf.apply(*q)).collect())
        .collect();
    match hp {
        FillPattern::Insulation => {
            // Mäanderlinie (Mineralwolle/Dämmung) je Wandstück
            for poly in &body.fills {
                let ss: Vec<f64> = poly.iter().map(|q| (*q - start).dot(dir)).collect();
                let smin = ss.iter().cloned().fold(f64::INFINITY, f64::min);
                let smax = ss.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let seg_len = smax - smin;
                if seg_len < 1.0 {
                    continue;
                }
                let period = (4.0 * h).max(80.0);
                let steps = ((seg_len / period) * 14.0).ceil().max(4.0) as usize;
                let mut pts = Vec::with_capacity(steps + 1);
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let s_along = t * seg_len;
                    let ang = s_along / period * std::f64::consts::TAU;
                    let off = ang.sin() * (h * 0.9);
                    pts.push(xf.apply(start + dir * (smin + s_along) + n * off));
                }
                dl.push(Prim {
                    shape: Shape::Poly(pts, false),
                    style: thin,
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
        _ => {
            // Alle übrigen Muster (DIN 1356-1:2026-09, Tabelle 5) entstehen im
            // selben Mustergenerator wie die Füllfläche — je Wandstück
            // beschnitten, damit an Gehrung und Öffnung nichts übersteht.
            let sp = (thickness * 0.25).max(12.0);
            for fl in &flechen {
                push_muster(dl, e, &thin, fl, &[], hp, std::f64::consts::FRAC_PI_4, sp);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tür / Fenster / Treppe
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_door(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    center: Vec2,
    angle: f64,
    width: f64,
    kind: DoorKind,
    hinge: f64,
    swing_in: bool,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp() * if swing_in { 1.0 } else { -1.0 };
    let w = width.max(1.0);
    let hw = w * 0.5;

    let mut line = |dl: &mut DisplayList, p0: Vec2, p1: Vec2| {
        dl.push(Prim {
            shape: Shape::Line(xf.apply(p0), xf.apply(p1)),
            style: *st,
            entity: e.id,
            layer: e.layer,
        });
    };

    match kind {
        DoorKind::Opening => {}
        DoorKind::Sliding => {
            // Schiebetür: zwei versetzte Flügel
            let o = 35.0;
            line(
                dl,
                center - dir * hw,
                center + dir * hw + nrm * o,
            );
            line(
                dl,
                center + dir * hw + nrm * o,
                center + dir * (hw * 0.15) + nrm * o,
            );
            line(
                dl,
                center - dir * (hw * 0.15) - nrm * o,
                center + dir * hw - nrm * o,
            );
        }
        DoorKind::Double => {
            // Zwei Flügel, je halbe Breite, Scharniere an den Laibungen
            for side in [-1.0f64, 1.0] {
                let hinge_p = center + dir * (side * hw);
                let leaf = hw * 0.95;
                let leaf_end = hinge_p + nrm * leaf;
                line(dl, hinge_p, leaf_end);
                let other = center;
                let a0 = (other - hinge_p).angle();
                let a1 = (leaf_end - hinge_p).angle();
                let (a0, a1) = order_arc(a0, a1);
                push_arc_prim(dl, e, st, hinge_p, leaf, a0, a1, xf);
            }
        }
        DoorKind::Hinge => {
            let hinge_p = center - dir * (hinge * hw);
            let other = center + dir * (hinge * hw);
            let leaf_end = hinge_p + nrm * w;
            line(dl, hinge_p, leaf_end);
            let a0 = (other - hinge_p).angle();
            let a1 = (leaf_end - hinge_p).angle();
            // kürzerer Bogen
            let (a0, a1) = order_arc(a0, a1);
            push_arc_prim(dl, e, st, hinge_p, w, a0, a1, xf);
        }
    }
}

/// Lage und Form einer Tür, die gerade platziert wird (Werkzeug-Vorschau).
#[derive(Clone, Copy, Debug)]
pub struct DoorSpec {
    /// Wand, in der die Tür sitzt — ohne Wand gibt es keine Vorschau.
    pub host: Option<Id>,
    /// Geklickter Punkt auf der Wandachse (mm).
    pub center: Vec2,
    /// Richtung der Wand (Bogenmaß).
    pub angle: f64,
    pub width: f64,
    pub kind: DoorKind,
    /// Anschlagseite: -1 oder +1
    pub hinge: f64,
    /// Öffnungsseite; bestimmt zusammen mit der Anschlagseite die vordere
    /// Wandkante, auf der das Blatt sitzt.
    pub swing_in: bool,
    /// Lage des Basispunktes längs der Türbreite (T0091) — der Klickpunkt
    /// hängt an diesem Punkt der Tür.
    pub base: DoorBase,
}

/// Lage und Form eines Fensters, das gerade platziert wird.
#[derive(Clone, Copy, Debug)]
pub struct WindowSpec {
    /// Wand, in der das Fenster sitzt — ohne Wand gibt es keine Vorschau.
    pub host: Option<Id>,
    /// Geklickter Punkt auf der Wandachse (mm).
    pub center: Vec2,
    /// Richtung der Wand (Bogenmaß).
    pub angle: f64,
    pub width: f64,
    /// Lage in der Wanddicke (Links/Mittig/Rechts).
    pub align: WallAlign,
}

/// Vorschau einer Öffnung (Tür oder Fenster): Symbol, Wandstücke des Hosts und
/// die im Wandkörper freigelegte Fläche.
///
/// Alles kommt aus demselben Renderer wie das fertige Objekt — Türblatt samt
/// Öffnungsbogen bzw. Fensterrahmen aus [`render_entity`], das Loch in der Wand
/// aus [`wall_opening_intervals`]; die Öffnung wird dafür in eine Kopie des
/// Dokuments gelegt. Vorher zeichnete die GUI eigene Platzhalter: bei der Tür
/// ein voller Kreis in der Wandmitte mit gespiegeltem Anschlag, beim Fenster
/// eine Linie mit zwei Hilfslinien — beide ohne Ausschnitt in der Wand (T0026).
pub struct OpeningPreview {
    /// Symbol der Öffnung — als Akzentkontur gezeichnet
    pub symbol: DisplayList,
    /// Wandstücke des Hosts mit dem Loch dieser Öffnung — in Normalfarbe,
    /// damit die Wand genauso aussieht wie nach dem Klick
    pub wall: DisplayList,
    /// Flächen, die die Öffnung im Wandkörper freimacht (Weltkoordinaten). Die
    /// Vorschau wird **über** die vorhandene Wand gezeichnet; ohne Freilegen
    /// bliebe das Loch unsichtbar, weil die Wand darunter weiterläuft.
    pub loch: Vec<Vec<Vec2>>,
    /// Basispunkt der Tür (T0091) — die Zeichenfläche markiert ihn in der
    /// Vorschau, damit sichtbar ist, wo die Tür am Zeiger hängt.
    pub base: Option<Vec2>,
}

/// Vorschau der Tür, die beim Klick entsteht.
pub fn preview_door(doc: &Document, spec: &DoorSpec) -> Option<OpeningPreview> {
    let host = spec.host?;
    let host_e = host_wall_entity(doc, host)?;
    // Die Tür liegt nur in der freien Wandstrecke (T0078): dieselbe Begrenzung
    // wie in der Action `draw.door`, sonst zeigt die Vorschau eine Lage, die
    // beim Klick nicht entsteht. Passt sie nicht in die Wand, gibt es keine
    // Vorschau.
    crate::actions::opening_span(doc, spec.host, spec.width)?;
    // Der Klickpunkt ist der Basispunkt der Tür (Links/Mitte/Rechts) — dieselbe
    // Rechnung wie in der Action `draw.door` (T0091).
    let anschlusspunkt = crate::actions::door_center_from_base(
        doc,
        spec.host,
        spec.center,
        spec.width,
        spec.base,
    )
    .unwrap_or(spec.center);
    // Tür immer auf die vordere Wandkante — dieselbe Rechnung wie in der
    // Action `draw.door`, sonst springt die Tür beim Klick um die halbe
    // Wanddicke zur Seite.
    let center = crate::actions::door_face_center(
        doc,
        spec.host,
        anschlusspunkt,
        spec.angle,
        spec.swing_in,
    );
    let kind = EntityKind::Door {
        align: WallAlign::Center,
        center,
        angle: spec.angle,
        width: spec.width,
        kind: spec.kind,
        hinge: spec.hinge,
        swing_in: spec.swing_in,
        host: spec.host,
        at: None,
        base: spec.base,
    };
    let mut v = opening_preview(doc, host, &host_e, kind, center, spec.width)?;
    // Basispunkt der Vorschau: derselbe Punkt, den die Tür nach dem Klick trägt.
    v.base = Some(center + Vec2::polar(spec.angle, 1.0) * (spec.base.offset() * spec.width));
    Some(v)
}

/// Basispunkt einer Tür in Weltkoordinaten (T0091).
///
/// Das ist der Punkt, an dem eine neue Tür am Mauszeiger hängt und den die
/// Zeichenfläche als Marke zeichnet — Links/Mitte/Rechts längs der Türbreite.
pub fn door_base_point(e: &Entity) -> Option<Vec2> {
    let EntityKind::Door {
        center,
        angle,
        width,
        base,
        ..
    } = &e.kind
    else {
        return None;
    };
    let dir = Vec2::polar(*angle, 1.0);
    Some(*center + dir * (base.offset() * *width))
}

/// Vorschau des Fensters, die beim Klick entsteht.
pub fn preview_window(doc: &Document, spec: &WindowSpec) -> Option<OpeningPreview> {
    let host = spec.host?;
    let host_e = host_wall_entity(doc, host)?;
    // Nur in der freien Wandstrecke, wie die Action `draw.window` (T0078).
    crate::actions::opening_span(doc, spec.host, spec.width)?;
    let anschlusspunkt =
        crate::actions::clamp_opening_center(doc, spec.host, spec.center, spec.width);
    // Lage in der Wanddicke wie in der Action `draw.window`.
    let center =
        crate::actions::offset_opening(doc, spec.host, anschlusspunkt, spec.angle, spec.align);
    let kind = EntityKind::Window {
        align: spec.align,
        center,
        angle: spec.angle,
        width: spec.width,
        // Bauhöhen ändern am Grundriss-Symbol nichts.
        sill: 0.0,
        height: 0.0,
        host: spec.host,
        at: None,
    };
    opening_preview(doc, host, &host_e, kind, center, spec.width)
}

/// Host-Wand aus dem Modell — dieselbe Stelle, an der `canvas::wall_under` und
/// die Actions den Host suchen.
fn host_wall_entity(doc: &Document, host: Id) -> Option<Entity> {
    let e = doc.entities.iter().find(|e| e.id == host)?.clone();
    matches!(e.kind, EntityKind::Wall { .. }).then_some(e)
}

/// Gemeinsamer Weg für Tür und Fenster: Symbol und Wand mit Loch rendern.
///
/// Tür und Fenster bekommen damit denselben Ausschnitt in der Wand — gleiche
/// Breite, gleiche Laibungen (T0026, Turn 2).
fn opening_preview(
    doc: &Document,
    host: Id,
    host_e: &Entity,
    kind: EntityKind,
    center: Vec2,
    width: f64,
) -> Option<OpeningPreview> {
    let mut vor = doc.clone();
    let id = vor.new_id();
    let symbol = Entity::new(id, vor.current_layer, kind);
    vor.entities.push(symbol.clone());
    let ctx = RenderCtx::new(&vor);
    let mut symbol_dl = DisplayList::default();
    render_entity(&ctx, &symbol, Affine::IDENTITY, &mut symbol_dl, 0);
    let mut wall_dl = DisplayList::default();
    if let Some(w) = vor.entities.iter().find(|e| e.id == host) {
        render_entity(&ctx, w, Affine::IDENTITY, &mut wall_dl, 0);
    }
    // Fläche der Öffnung: dasselbe Intervall, das die Wand ausschneidet.
    let mut loch = Vec::new();
    if let Some(f) = wall::Frame::of(host_e) {
        let t = (center - f.a0).dot(f.dir);
        let total = f.len();
        let s0 = (t - width * 0.5).clamp(0.0, total);
        let s1 = (t + width * 0.5).clamp(0.0, total);
        if s1 > s0 + 1.0 {
            let p0 = f.a + f.dir * s0;
            let p1 = f.a + f.dir * s1;
            loch.push(vec![
                p0 + f.n * f.h,
                p1 + f.n * f.h,
                p1 - f.n * f.h,
                p0 - f.n * f.h,
            ]);
        }
    }
    Some(OpeningPreview {
        symbol: symbol_dl,
        wall: wall_dl,
        loch,
        base: None,
    })
}

fn order_arc(a0: f64, a1: f64) -> (f64, f64) {
    // kleineren Sweep wählen
    let ccw = norm_angle(a1 - a0);
    if ccw <= std::f64::consts::PI {
        (a0, a1)
    } else {
        (a1, a0)
    }
}

#[allow(clippy::too_many_arguments)]
fn push_arc_prim(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    c: Vec2,
    r: f64,
    a0: f64,
    a1: f64,
    xf: Affine,
) {
    let mirror = xf.is_mirrored();
    let (b0, b1) = if mirror { (-a1, -a0) } else { (a0, a1) };
    dl.push(Prim {
        shape: Shape::Arc {
            c: xf.apply(c),
            r: r * xf.scale_factor(),
            a0: b0 + xf.rotation(),
            a1: b1 + xf.rotation(),
        },
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
}

fn render_window(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    center: Vec2,
    angle: f64,
    width: f64,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    let hw = width * 0.5;
    // Wanddicke aus Host, sonst Standard
    let t = ctx
        .doc
        .host_wall(
            match &e.kind {
                EntityKind::Window { host, .. } => *host,
                _ => None,
            },
        )
        .and_then(|w| match &w.kind {
            EntityKind::Wall { thickness, .. } => Some(*thickness),
            _ => None,
        })
        .unwrap_or(240.0);
    let h = t * 0.5;
    let frame = vec![
        xf.apply(center - dir * hw + nrm * h),
        xf.apply(center + dir * hw + nrm * h),
        xf.apply(center + dir * hw - nrm * h),
        xf.apply(center - dir * hw - nrm * h),
    ];
    dl.push(Prim {
        shape: Shape::Poly(frame.clone(), true),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
    // Glaslinien
    for o in [-h * 0.35, 0.0, h * 0.35] {
        dl.push(Prim {
            shape: Shape::Line(
                xf.apply(center - dir * hw + nrm * o),
                xf.apply(center + dir * hw + nrm * o),
            ),
            style: PrimStyle {
                width: st.width * 0.8,
                ..*st
            },
            entity: e.id,
            layer: e.layer,
        });
    }
}

/// Linien eines Treppenteils: eigene Werte überschreiben den Objektstil.
fn halb_stil(st: &PrimStyle, teil: Option<&EntityStyle>) -> PrimStyle {
    let Some(t) = teil else { return *st };
    PrimStyle {
        color: t.color.unwrap_or(st.color),
        line_type: t.line_type.unwrap_or(st.line_type),
        width: t.line_weight.unwrap_or(st.width),
        fill: t.fill.or(st.fill),
    }
}

/// Eine Linie der Treppe in die Displayliste legen.
fn stair_linie(dl: &mut DisplayList, e: &Entity, xf: Affine, stil: PrimStyle, p: Vec2, q: Vec2) {
    dl.push(Prim {
        shape: Shape::Line(xf.apply(p), xf.apply(q)),
        style: stil,
        entity: e.id,
        layer: e.layer,
    });
}

/// Treppenschnitt: die beiden Diagonalen und die **Grenze** zwischen unterem
/// und oberem Teil. Die Grenze ist die Parallele in der Mitte zwischen den
/// beiden Diagonalen — an ihr wechseln die Linieneigenschaften, nicht an einer
/// geraden Querlinie (T0027, Turn 3).
struct Schnitt {
    diagonalen: Vec<(Vec2, Vec2)>,
    grenze: Option<(Vec2, Vec2)>,
    /// Normale der Grenze, die zum oberen Teil (Laufende) zeigt.
    oben: Vec2,
}

impl Schnitt {
    fn ist_oben(&self, p: Vec2) -> bool {
        match self.grenze {
            Some((g, _)) => (p - g).dot(self.oben) >= 0.0,
            None => false,
        }
    }
}

/// Treppenschnitt aufbauen. Ist er aus, bleibt alles ein Teil (keine
/// Diagonalen, keine Grenze). Bei kurzen Treppen gibt es nichts zu verkürzen —
/// dann liegt die Grenze gerade in der Laufmitte.
fn schnitt_geometrie(
    a: Vec2,
    dir: Vec2,
    nrm: Vec2,
    hw: f64,
    len: f64,
    width: f64,
    cut: bool,
) -> Schnitt {
    let mut s = Schnitt {
        diagonalen: Vec::new(),
        grenze: None,
        oben: nrm,
    };
    if !cut {
        return s;
    }
    let d = width * 0.8;
    let abstand = width * 0.16;
    if len >= d * 1.4 {
        let t0 = (len - (d + abstand)) * 0.5;
        for k in 0..2 {
            let t = t0 + abstand * k as f64;
            s.diagonalen
                .push((a + dir * t - nrm * hw, a + dir * (t + d) + nrm * hw));
        }
        let (p, q) = s.diagonalen[0];
        s.grenze = Some((p + dir * (abstand * 0.5), q + dir * (abstand * 0.5)));
    } else {
        s.grenze = Some((
            a + dir * (len * 0.5) - nrm * hw,
            a + dir * (len * 0.5) + nrm * hw,
        ));
    }
    if let Some((g0, g1)) = s.grenze {
        let mut n = (g1 - g0).normalized().perp();
        if n.dot(dir) < 0.0 {
            n = -n;
        }
        s.oben = n;
    }
    s
}

/// Eine Strecke der Treppe in Stücke zerlegen und jedem Stück seinen Linienstil
/// geben.
///
/// `luecke` = `Some(halbe Lücke)`: an den beiden Diagonalen wird die Strecke
/// unterbrochen (Stufenlinien). `None`: die Strecke läuft durch und wird nur an
/// der Schnittgrenze geteilt (Umrisskanten, Lauflinie).
fn stair_teile(
    segs: &mut Vec<(Vec2, Vec2, PrimStyle)>,
    schnitt: &Schnitt,
    cut: bool,
    st_unten: PrimStyle,
    st_oben: PrimStyle,
    p: Vec2,
    q: Vec2,
    luecke: Option<f64>,
) {
    let stil = |m: Vec2| {
        if cut && schnitt.ist_oben(m) {
            st_oben
        } else {
            st_unten
        }
    };
    let teile: Vec<(Vec2, Vec2)> = match luecke {
        Some(h) if !schnitt.diagonalen.is_empty() => {
            let mut treffer: Vec<f64> = Vec::new();
            for d in &schnitt.diagonalen {
                if let Some(u) = kreuz_anteil(p, q, *d) {
                    treffer.push(u);
                }
            }
            stuecke(p, q, &treffer, h)
        }
        _ => {
            let mut out = vec![(p, q)];
            if let Some(g) = schnitt.grenze {
                if let Some(u) = kreuz_anteil(p, q, g) {
                    if u > 0.001 && u < 0.999 {
                        let m = anteil_punkt(p, q, u);
                        out = vec![(p, m), (m, q)];
                    }
                }
            }
            out
        }
    };
    for (s0, s1) in teile {
        let m = (s0 + s1) * 0.5;
        segs.push((s0, s1, stil(m)));
    }
}

#[allow(clippy::too_many_arguments)]
fn render_stair(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    a: Vec2,
    b: Vec2,
    width: f64,
    steps: u32,
    height: f64,
    auto_steps: bool,
    cut: bool,
    unten: Option<&EntityStyle>,
    oben: Option<&EntityStyle>,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let len = a.dist(b);
    let hw = width * 0.5;
    if len < 1.0 || hw < 1.0 {
        return;
    }
    let dir = (b - a).normalized();
    let nrm = dir.perp();
    let mass = crate::stair::treppenmass(len, height, steps, auto_steps);
    let n = mass.stufen.max(1) as usize;
    let run = len / n as f64;
    let duenn = |s: PrimStyle| PrimStyle {
        width: s.width * 0.75,
        ..s
    };

    // Oberer und unterer Teil: eigene Linien nur mit Treppenschnitt, sonst
    // zeichnet der Objektstil die ganze Treppe.
    let (st_unten, st_oben) = if cut {
        (halb_stil(st, unten), halb_stil(st, oben))
    } else {
        (*st, *st)
    };
    let schnitt = schnitt_geometrie(a, dir, nrm, hw, len, width, cut);
    let stil_bei = |p: Vec2| {
        if cut && schnitt.ist_oben(p) {
            st_oben
        } else {
            st_unten
        }
    };

    let mut segs: Vec<(Vec2, Vec2, PrimStyle)> = Vec::new();
    // Umriss: Startkante (bei a), Endkante (bei b) und die beiden Längskanten,
    // die an der Schnittgrenze ihre Linienart wechseln.
    let ecken = [a + nrm * hw, b + nrm * hw, b - nrm * hw, a - nrm * hw];
    stair_teile(&mut segs, &schnitt, cut, st_unten, st_oben, ecken[3], ecken[0], None);
    stair_teile(&mut segs, &schnitt, cut, st_unten, st_oben, ecken[1], ecken[2], None);
    stair_teile(&mut segs, &schnitt, cut, st_unten, st_oben, ecken[0], ecken[1], None);
    stair_teile(&mut segs, &schnitt, cut, st_unten, st_oben, ecken[3], ecken[2], None);

    // Stufenlinien; am Treppenschnitt unterbrechen die Diagonalen sie.
    for i in 1..n {
        let p = a + dir * (run * i as f64);
        stair_teile(
            &mut segs,
            &schnitt,
            cut,
            duenn(st_unten),
            duenn(st_oben),
            p + nrm * hw,
            p - nrm * hw,
            Some(0.06),
        );
    }

    // Lauflinie (Mitte) — sie wechselt an der Schnittgrenze die Linienart.
    stair_teile(&mut segs, &schnitt, cut, st_unten, st_oben, a, b, None);

    for (p, q, stil) in segs {
        stair_linie(dl, e, xf, stil, p, q);
    }

    // Die beiden Diagonalen des Treppenschnitts
    for (p0, p1) in &schnitt.diagonalen {
        stair_linie(dl, e, xf, *st, *p0, *p1);
    }

    // Richtungspfeil am Laufende (die Laufrichtung ist a → b)
    let ah = ctx.a(1.2).max(run * 0.35);
    let stil_pfeil = stil_bei(b);
    for seite in [1.0, -1.0] {
        let back = b - dir * (ah * 1.5) + nrm * (ah * 0.6 * seite);
        stair_linie(dl, e, xf, stil_pfeil, b, back);
    }
    // Ring am Anfang des Laufs
    dl.push(Prim {
        shape: Shape::Circle {
            c: xf.apply(a),
            r: (width * 0.06).max(ctx.a(0.5)) * xf.scale_factor(),
        },
        style: stil_bei(a),
        entity: e.id,
        layer: e.layer,
    });
}

/// Anteil (0…1) auf der Strecke `s`→`t`, an dem die Strecke `l` sie schneidet.
fn kreuz_anteil(s: Vec2, t: Vec2, l: (Vec2, Vec2)) -> Option<f64> {
    let r = t - s;
    let q = l.1 - l.0;
    let denom = r.cross(q);
    if denom.abs() < 1e-12 {
        return None;
    }
    let u = (l.0 - s).cross(q) / denom;
    let v = (l.0 - s).cross(r) / denom;
    if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
        Some(u)
    } else {
        None
    }
}

/// Punkt auf der Strecke `s`→`t` beim Anteil `u`.
fn anteil_punkt(s: Vec2, t: Vec2, u: f64) -> Vec2 {
    s + (t - s) * u
}

/// Zerlegt eine Strecke an den Schnittanteilen und lässt an jedem Schnitt eine
/// Lücke der halben Breite `halb` (Anteil der Strecke).
fn stuecke(s: Vec2, t: Vec2, schnitte: &[f64], halb: f64) -> Vec<(Vec2, Vec2)> {
    let mut cuts: Vec<f64> = schnitte
        .iter()
        .copied()
        .filter(|u| *u > 0.01 && *u < 0.99)
        .collect();
    cuts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    let mut cur = 0.0;
    for u in cuts {
        let u0 = (u - halb).max(cur);
        let u1 = (u + halb).min(1.0);
        if u0 > cur + 0.001 {
            out.push((anteil_punkt(s, t, cur), anteil_punkt(s, t, u0)));
        }
        cur = cur.max(u1);
    }
    if cur < 0.999 {
        out.push((anteil_punkt(s, t, cur), t));
    }
    out
}

// ---------------------------------------------------------------------------
// Bemaßung (DIN 406 / DIN 1356)
// ---------------------------------------------------------------------------

/// Darstellung der Bemaßung nach DIN 1356-1:2026-09, Bild 3 — Länge des
/// Begrenzungsstrichs (Schrägstrich) und Überstand der Maßlinie über die
/// äußere Maßhilfslinie.
///
/// Bild 3 zeichnet bei 2,5 mm Texthöhe rund 1,5 mm Strich und 1,5 mm
/// Überstand, also etwa 0,6 h. Anteile der Texthöhe statt fester
/// Millimeter: sonst stimmt die Darstellung nur für eine Schriftgröße.
pub const DIM_MARKE_H: f64 = 0.6;
pub const DIM_UEBERSTAND_H: f64 = 0.6;
/// Abstand des Textankers über der Maßlinie. Der Anker ist die Unterkante der
/// Zeilenbox (Unterlängen), deshalb 0,6 h: die Ziffern stehen damit rund 0,7 h
/// über der Linie — so wie in Bild 3.
pub const DIM_TEXT_ABSTAND_H: f64 = 0.6;

struct DimGeom {
    /// zu bemaßende Punkte am Bauteil
    wa: Vec2,
    wb: Vec2,
    /// Punkte auf der Maßlinie
    da: Vec2,
    db: Vec2,
    text: String,
    angle: f64,
}

/// Richtung einer Maßlinie und die Einheitsrichtung ihres **Abstands**.
///
/// Eine Quelle für Anzeige (`dim_geometry`, `render_dim_chain`), Griffe
/// (`handles`) und die Befehlsschicht (`modify.set` mit `offset`): sonst
/// rechneten drei Stellen dieselbe Achse und der Griff liefe neben der Linie.
///
/// Der Abstand zählt von den Basispunkten **zur** Maßlinie: bei `X` nach oben
/// (+Y), bei `Y` nach rechts (+X), bei `Aligned` links der Messrichtung. Das
/// Vorzeichen ist damit anschaulich — positiv heißt „über“ bzw. „rechts“ der
/// Messstrecke.
pub fn dim_achse(pts: &[Vec2], axis: DimAxis) -> (Vec2, Vec2) {
    match axis {
        DimAxis::X => (v(1.0, 0.0), v(0.0, 1.0)),
        DimAxis::Y => (v(0.0, 1.0), v(1.0, 0.0)),
        DimAxis::Aligned => {
            let d = pts.get(1).map(|b| *b - pts[0]).unwrap_or(v(1.0, 0.0));
            // Entartete Messstrecke: die Achse ist nicht bestimmt, dann bleibt
            // es bei waagerecht (die Anzeige zeigt ohnehin keinen Maßtext).
            let dir = if d.len() < 1e-9 {
                v(1.0, 0.0)
            } else {
                d.normalized()
            };
            (dir, dir.perp())
        }
    }
}

/// Basispunkte auf die Maßlinie gelotet.
///
/// Für `X` ist die Maßlinie waagerecht auf **einer** Höhe (`line_pos.y`), für
/// `Y` senkrecht auf einer Breite, für `Aligned` parallel zur Messstrecke im
/// Abstand von `line_pos`. Anzeige und Griff benutzen dieselbe Rechnung.
///
/// Achtung: bei `X`/`Y` wird die **Koordinate gesetzt**, nicht ein Versatz
/// addiert. Ein Versatz je Basispunkt kippte die Maßlinie, sobald ein
/// Basispunkt seitlich wanderte (T0055, Turn 1: Ziehen am Punkt drehte die
/// Linie mit).
pub fn dim_projiziert(pts: &[Vec2], line_pos: Vec2, axis: DimAxis) -> Vec<Vec2> {
    match axis {
        DimAxis::X => pts.iter().map(|p| v(p.x, line_pos.y)).collect(),
        DimAxis::Y => pts.iter().map(|p| v(line_pos.x, p.y)).collect(),
        DimAxis::Aligned => {
            if pts.is_empty() {
                return Vec::new();
            }
            let (_, vers) = dim_achse(pts, axis);
            let off = (line_pos - pts[0]).dot(vers);
            pts.iter().map(|p| *p + vers * off).collect()
        }
    }
}

/// Basispunkte eines Maßes/einer Maßkette mit Maßlinie und Achse — die eine
/// Stelle, an der die Hülle eines Bemaßungsobjekts aufgemacht wird.
fn dim_teile(kind: &EntityKind) -> Option<(Vec<Vec2>, Vec2, DimAxis)> {
    match kind {
        EntityKind::Dim {
            a,
            b,
            line_pos,
            axis,
            ..
        } => Some((vec![*a, *b], *line_pos, *axis)),
        EntityKind::DimChain { pts, line_pos, axis } if pts.len() >= 2 => {
            Some((pts.clone(), *line_pos, *axis))
        }
        _ => None,
    }
}

/// Maßlinie eines Maßes oder einer Maßkette: Anfang und Ende der gezeichneten
/// Linie in Weltkoordinaten.
pub fn dim_line_span(kind: &EntityKind) -> Option<(Vec2, Vec2)> {
    let (pts, line_pos, axis) = dim_teile(kind)?;
    let proj = dim_projiziert(&pts, line_pos, axis);
    Some((*proj.first()?, *proj.last()?))
}

/// Abstand der Maßlinie vom **ersten** Basispunkt in mm, vorzeichenbehaftet
/// (positiv in Richtung [`dim_achse`]-Versatz).
pub fn dim_offset(kind: &EntityKind) -> Option<f64> {
    let (pts, line_pos, axis) = dim_teile(kind)?;
    let (_, vers) = dim_achse(&pts, axis);
    Some((line_pos - pts[0]).dot(vers))
}

/// Setzt den Abstand der Maßlinie. Liefert `false`, wenn das Objekt keine
/// Bemaßung ist — ein stiller Vorgabewert wäre hier ein Fehler (T0038).
pub fn dim_offset_set(kind: &mut EntityKind, off: f64) -> bool {
    let Some((pts, _, axis)) = dim_teile(kind) else {
        return false;
    };
    let (_, vers) = dim_achse(&pts, axis);
    let bezug = pts[0];
    match kind {
        EntityKind::Dim { line_pos, .. } | EntityKind::DimChain { line_pos, .. } => {
            // Nur längs des Versatzes schieben: die Lage entlang der Maßlinie
            // bleibt erhalten, am Objekt ändert sich nur das Angefasste.
            let alt = (*line_pos - bezug).dot(vers);
            *line_pos = *line_pos + vers * (off - alt);
            true
        }
        _ => false,
    }
}

/// Zieht die Maßlinie senkrecht zu ihrer Richtung auf die Höhe von `ziel`.
/// Liefert `false`, wenn das Objekt keine Bemaßung ist.
pub fn dim_line_move(kind: &mut EntityKind, ziel: Vec2) -> bool {
    let Some((pts, _, axis)) = dim_teile(kind) else {
        return false;
    };
    let (_, vers) = dim_achse(&pts, axis);
    dim_offset_set(kind, (ziel - pts[0]).dot(vers))
}

fn dim_geometry(
    ctx: &RenderCtx,
    p0: Vec2,
    p1: Vec2,
    line_pos: Vec2,
    axis: DimAxis,
    override_text: Option<&str>,
) -> DimGeom {
    let doc = ctx.doc;
    let (wa, wb) = (p0, p1);
    let (dir, _) = dim_achse(&[p0, p1], axis);
    let proj = dim_projiziert(&[p0, p1], line_pos, axis);
    let (da, db) = (proj[0], proj[1]);
    let measure = match axis {
        DimAxis::X => (p1.x - p0.x).abs(),
        DimAxis::Y => (p1.y - p0.y).abs(),
        DimAxis::Aligned => p0.dist(p1),
    };
    let angle = dir.angle();
    let text = override_text
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            dim_text(
                measure,
                doc.settings.dim_unit,
                doc.dim_style.decimals,
            )
        });
    let _ = format_number;
    DimGeom {
        wa,
        wb,
        da,
        db,
        text: if ctx.resolve_text {
            doc.resolve_text(&text)
        } else {
            text
        },
        angle,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_dim(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    p0: Vec2,
    p1: Vec2,
    line_pos: Vec2,
    axis: DimAxis,
    override_text: Option<&str>,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let g = dim_geometry(ctx, p0, p1, line_pos, axis, override_text);
    let a = ctx.a(1.0);
    let style = &ctx.doc.dim_style;
    let ext_gap = ctx.a(style.ext_gap);
    let over = ctx.a(style.ext_overshoot);
    let arrow = ctx.a(style.arrow_size);
    let text_h = ctx.a(style.text_height);
    // Bild 3: Begrenzungsstrich und Überstand hängen an der Texthöhe, nicht an
    // der Pfeilgröße; der Text steht deutlich über der Linie.
    let marke = ctx.a(style.text_height * DIM_MARKE_H);
    let ueberstand = ctx.a(style.text_height * DIM_UEBERSTAND_H);
    let text_gap = ctx.a(style.text_gap)
        .max(ctx.a(style.text_height * DIM_TEXT_ABSTAND_H));
    let dir = Vec2::polar(g.angle, 1.0);

    // Hilfslinien — Volllinie schmal wie die Maßlinie (Tabelle 2, 01.1.8)
    for (w, d) in [(g.wa, g.da), (g.wb, g.db)] {
        let dd = (d - w).normalized();
        let s = w + dd * ext_gap.min(w.dist(d) * 0.5);
        let t = d + dd * over;
        dl.push(Prim {
            shape: Shape::Line(xf.apply(s), xf.apply(t)),
            style: *st,
            entity: e.id,
            layer: e.layer,
        });
    }
    // Maßlinie mit Überstand über die Maßhilfslinien (§9.1: „Auf den Überstand
    // der Maßlinie über die Maßhilfslinie hinaus darf verzichtet werden“)
    dl.push(Prim {
        shape: Shape::Line(
            xf.apply(g.da - dir * ueberstand),
            xf.apply(g.db + dir * ueberstand),
        ),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
    // Markierungen
    match style.arrow {
        ArrowKind::Slash => {
            let d = Vec2::polar(g.angle + std::f64::consts::FRAC_PI_4, marke * 0.5);
            for p in [g.da, g.db] {
                dl.push(Prim {
                    shape: Shape::Line(xf.apply(p - d), xf.apply(p + d)),
                    style: *st,
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
        ArrowKind::Arrow => {
            for (p, s) in [(g.da, 1.0), (g.db, -1.0)] {
                let d = dir * s;
                let tri = vec![
                    xf.apply(p),
                    xf.apply(p + d * arrow - dir.perp() * (arrow * 0.22)),
                    xf.apply(p + d * arrow + dir.perp() * (arrow * 0.22)),
                ];
                dl.push(Prim {
                    shape: Shape::Poly(tri, true),
                    style: PrimStyle {
                        color: st.color,
                        fill: Some(st.color),
                        ..*st
                    },
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
        ArrowKind::Dot => {
            for p in [g.da, g.db] {
                dl.push(Prim {
                    shape: Shape::Dot {
                        p: xf.apply(p),
                        r: arrow * 0.3,
                    },
                    style: PrimStyle {
                        color: st.color,
                        fill: Some(st.color),
                        ..*st
                    },
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
        ArrowKind::None => {}
    }
    // Maßtext
    let mid = (g.da + g.db) * 0.5;
    let n = Vec2::polar(g.angle + std::f64::consts::FRAC_PI_2, 1.0);
    let text_w = estimate_text_width(&g.text, text_h);
    let span = g.da.dist(g.db);
    let mut angle = g.angle;
    // Lesbarkeit: nie kopfstehen
    if angle > std::f64::consts::FRAC_PI_2 + 0.01 || angle < -std::f64::consts::FRAC_PI_2 - 0.01 {
        angle += std::f64::consts::PI;
    }
    let (pos, align) = if text_w + ctx.a(1.0) > span && span > 0.0 {
        // Text passt nicht: außerhalb platzieren
        (g.db + dir * (text_w * 0.5 + ctx.a(1.5)), TextAlignH::Center)
    } else {
        (mid + n * text_gap, TextAlignH::Center)
    };
    dl.push(Prim {
        shape: Shape::Text(TextPrim {
            pos: xf.apply(pos),
            text: g.text,
            height: text_h * xf.scale_factor(),
            angle: angle + xf.rotation(),
            align,
            valign: TextAlignV::Bottom,
            bold: false,
            italic: false,
            underline: false,
            superscript: false,
            runs: Vec::new(),
            font: TextFont::Sans,
        }),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
}

fn render_dim_chain(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    line_pos: Vec2,
    axis: DimAxis,
    xf: Affine,
    dl: &mut DisplayList,
) {
    if pts.len() < 2 {
        return;
    }
    let style = &ctx.doc.dim_style;
    let a = ctx.a(1.0);
    let ext_gap = ctx.a(style.ext_gap);
    let over = ctx.a(style.ext_overshoot);
    let arrow = ctx.a(style.arrow_size);
    let text_h = ctx.a(style.text_height);
    // Bild 3: Begrenzungsstrich, Überstand der Maßlinie und Textabstand sind
    // Anteile der Texthöhe (die Pfeilgröße gilt nur für Pfeil und Punkt).
    let marke = ctx.a(style.text_height * DIM_MARKE_H);
    let ueberstand = ctx.a(style.text_height * DIM_UEBERSTAND_H);
    let text_gap = ctx.a(style.text_gap)
        .max(ctx.a(style.text_height * DIM_TEXT_ABSTAND_H));
    let (dir, _) = dim_achse(pts, axis);
    // Lage des Maßtextes: links der Messrichtung (nicht der Abstandsrichtung).
    let n = dir.perp();
    let angle = dir.angle();

    // Maßlinie — dieselbe Lotrechnung wie der Griff an der Maßlinie
    // (`render::dim_projiziert`), mit Überstand über die äußeren
    // Maßhilfslinien (§9.1, Bild 3).
    let projected: Vec<Vec2> = dim_projiziert(pts, line_pos, axis);
    dl.push(Prim {
        shape: Shape::Line(
            xf.apply(projected[0] - dir * ueberstand),
            xf.apply(*projected.last().unwrap() + dir * ueberstand),
        ),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
    // Hilfslinien an allen Punkten — schmal wie die Maßlinie (Tabelle 2, 01.1.8)
    for (w, d) in pts.iter().zip(projected.iter()) {
        let dd = (*d - *w).normalized();
        let s = *w + dd * ext_gap.min(w.dist(*d) * 0.5);
        let t = *d + dd * over;
        dl.push(Prim {
            shape: Shape::Line(xf.apply(s), xf.apply(t)),
            style: *st,
            entity: e.id,
            layer: e.layer,
        });
    }
    // Begrenzungszeichen **einmal je Basispunkt**: liegen zwei Segmente
    // aneinander, standen bisher zwei Striche (bei Pfeil und Punkt sichtbar
    // als Doppelmarke) auf demselben Punkt.
    match style.arrow {
        ArrowKind::Slash => {
            let d = Vec2::polar(angle + std::f64::consts::FRAC_PI_4, marke * 0.5);
            for p in &projected {
                dl.push(Prim {
                    shape: Shape::Line(xf.apply(*p - d), xf.apply(*p + d)),
                    style: *st,
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
        ArrowKind::Dot => {
            for p in &projected {
                dl.push(Prim {
                    shape: Shape::Dot {
                        p: xf.apply(*p),
                        r: arrow * 0.3,
                    },
                    style: PrimStyle {
                        color: st.color,
                        fill: Some(st.color),
                        ..*st
                    },
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
        ArrowKind::Arrow | ArrowKind::None => {}
    }
    // Segmente + Text
    for i in 0..projected.len() - 1 {
        let p0 = projected[i];
        let p1 = projected[i + 1];
        let measure = match axis {
            DimAxis::X => (p1.x - p0.x).abs(),
            DimAxis::Y => (p1.y - p0.y).abs(),
            DimAxis::Aligned => p0.dist(p1),
        };
        let txt = dim_text(measure, ctx.doc.settings.dim_unit, style.decimals);
        let txt = if ctx.resolve_text {
            ctx.doc.resolve_text(&txt)
        } else {
            txt
        };
        // Pfeile nur an den Segmentenden (Schrägstrich und Punkt stehen
        // bereits einmal je Basispunkt oben)
        match style.arrow {
            ArrowKind::Arrow => {
                for (p, s) in [(p0, -1.0), (p1, 1.0)] {
                    let d = dir * s;
                    let tri = vec![
                        xf.apply(p),
                        xf.apply(p + d * arrow - n * (arrow * 0.22)),
                        xf.apply(p + d * arrow + n * (arrow * 0.22)),
                    ];
                    dl.push(Prim {
                        shape: Shape::Poly(tri, true),
                        style: PrimStyle {
                            color: st.color,
                            fill: Some(st.color),
                            ..*st
                        },
                        entity: e.id,
                        layer: e.layer,
                    });
                }
            }
            // Schrägstrich und Punkt stehen schon oben, einmal je Basispunkt.
            ArrowKind::Slash | ArrowKind::Dot | ArrowKind::None => {}
        }
        let mid = (p0 + p1) * 0.5;
        let text_w = estimate_text_width(&txt, text_h);
        let span = p0.dist(p1);
        let mut ta = angle;
        if ta > std::f64::consts::FRAC_PI_2 + 0.01 || ta < -std::f64::consts::FRAC_PI_2 - 0.01 {
            ta += std::f64::consts::PI;
        }
        let pos = if text_w + a > span {
            // Zu schmal für die Maßzahl: außerhalb des Segments, damit sie
            // nicht auf dem Begrenzungsstrich des Nachbarsegments liegt.
            p1 + dir * (text_w * 0.5 + marke * 0.5 + a)
        } else {
            mid + n * text_gap
        };
        dl.push(Prim {
            shape: Shape::Text(TextPrim {
                pos: xf.apply(pos),
                text: txt,
                height: text_h * xf.scale_factor(),
                angle: ta + xf.rotation(),
                align: TextAlignH::Center,
                valign: TextAlignV::Bottom,
                bold: false,
                italic: false,
                underline: false,
                superscript: false,
                runs: Vec::new(),
                font: TextFont::Sans,
            }),
            style: *st,
            entity: e.id,
            layer: e.layer,
        });
    }
}

// ---------------------------------------------------------------------------
// Blatt (Papierraum)
// ---------------------------------------------------------------------------

/// Blattrahmen, Schriftfeld, Viewports und Papierraum-Objekte.
pub fn render_sheet(doc: &Document, sheet: &Sheet) -> DisplayList {
    let mut dl = DisplayList::default();
    let ctx = RenderCtx {
        doc,
        annot_scale: 1.0,
        layer_filter: None,
        resolve_text: true,
        selection: None,
        sheet_scale: Some(sheet.scale),
        entities: Some(&sheet.entities),
        walls: OnceCell::new(),
        wall_scope: None,
        volumes: OnceCell::new(),
        wall_edges: OnceCell::new(),
    };
    // Papierraum-Objekte: Blattrahmen, Schriftfeld und alles, was der Nutzer
    // selbst aufs Blatt zeichnet. Alles davon sind normale Objekte — der
    // Renderer hat hier keine Sonderdarstellung mehr.
    for e in &sheet.entities {
        render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    }
    dl
}

/// Modellgeometrie in einem Ansichtsfenster-Maßstab. Immer die Zeichnung —
/// auch dann, wenn der Papierraum gerade der aktive Bearbeitungsraum ist.
pub fn render_model_scaled(doc: &Document, scale: f64, layers: Option<Vec<Id>>) -> DisplayList {
    let ctx = RenderCtx {
        doc,
        annot_scale: scale.max(0.01),
        layer_filter: layers,
        resolve_text: true,
        selection: None,
        sheet_scale: None,
        entities: Some(&doc.entities),
        walls: OnceCell::new(),
        wall_scope: None,
        volumes: OnceCell::new(),
        wall_edges: OnceCell::new(),
    };
    render_document(&ctx)
}

/// Inhalt eines Viewports (Weltkoordinaten), beschnitten auf das Fenster.
pub fn render_viewport(doc: &Document, vp: &Viewport) -> DisplayList {
    // Ein Ansichtsfenster zeigt immer die Zeichnung, nie den Papierraum.
    let ctx = RenderCtx {
        doc,
        annot_scale: vp.scale.max(0.01),
        layer_filter: vp.layers.clone(),
        resolve_text: true,
        selection: None,
        sheet_scale: None,
        entities: Some(&doc.entities),
        walls: OnceCell::new(),
        wall_scope: None,
        volumes: OnceCell::new(),
        wall_edges: OnceCell::new(),
    };
    let mut dl = render_document(&ctx);
    // Auf Fensterfläche beschneiden: Primitive werden bei Bedarf gekürzt.
    if vp.show_frame {
        dl.push(Prim {
            shape: Shape::Poly(
                vec![
                    v(vp.x, vp.y),
                    v(vp.x + vp.w, vp.y),
                    v(vp.x + vp.w, vp.y + vp.h),
                    v(vp.x, vp.y + vp.h),
                ],
                true,
            ),
            style: PrimStyle {
                color: Color::GREY,
                line_type: LineType::Solid,
                width: 0.18,
                fill: None,
            },
            entity: 0,
            layer: 0,
        });
    }
    dl
}

// ---------------------------------------------------------------------------
// Hilfen für Auswahl / Fang
// ---------------------------------------------------------------------------

/// Trefferbild eines Objekts aus der Anzeige: die gezeichneten Strecken und
/// die Zeilenboxen seiner Texte.
///
/// Eine Quelle für Anzeige und Treffer — was gezeichnet wird, lässt sich
/// anklicken. Nötig für die Bemaßung: sie speichert ihre Basispunkte am
/// Bauteil und zeichnet die Maßlinie versetzt daneben. Vorher prüften Klick
/// und Fensterauswahl nur die Grundlinie zwischen den Basispunkten; die
/// sichtbare Maßlinie und der Maßtext trafen nicht, ein Klick auf der
/// unsichtbaren Grundlinie dagegen schon (T0052).
pub struct Trefferbild {
    pub segmente: Vec<Seg>,
    pub text_kaesten: Vec<[Vec2; 4]>,
}

/// Baut das Trefferbild über den Renderer: Linien und Polygonkanten als
/// Strecken, Texte als Zeilenbox ([`text_corners`] — derselbe Kasten wie
/// Rahmen und Fang).
pub fn trefferbild(doc: &Document, e: &Entity) -> Trefferbild {
    let ctx = RenderCtx::for_entity(doc, e);
    let mut dl = DisplayList::default();
    render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    let mut segmente = Vec::new();
    let mut text_kaesten = Vec::new();
    for prim in &dl.prims {
        match &prim.shape {
            Shape::Line(a, b) => segmente.push(Seg::Line(*a, *b)),
            Shape::Poly(pts, closed) => {
                if pts.len() < 2 {
                    continue;
                }
                let kanten = if *closed { pts.len() } else { pts.len() - 1 };
                for i in 0..kanten {
                    segmente.push(Seg::Line(pts[i], pts[(i + 1) % pts.len()]));
                }
            }
            Shape::Arc { c, r, a0, a1 } => segmente.push(Seg::Arc(Arc::new(*c, *r, *a0, *a1))),
            Shape::Text(t) => text_kaesten.push(text_corners(
                t.pos, &t.text, t.height, t.angle, t.align, t.valign,
            )),
            _ => {}
        }
    }
    Trefferbild {
        segmente,
        text_kaesten,
    }
}

/// Zerlegt ein Objekt in Segmente (für Fang, Schnitt und Trefferprüfung).
pub fn entity_segments(doc: &Document, e: &Entity) -> Vec<Seg> {
    entity_segments_tief(doc, e, 0)
}

/// Wie `entity_segments`, mit Tiefenzähler gegen verschachtelte Blöcke.
fn entity_segments_tief(doc: &Document, e: &Entity, depth: usize) -> Vec<Seg> {
    let mut out = Vec::new();
    match &e.kind {
        EntityKind::Line { a, b } => out.push(Seg::Line(*a, *b)),
        EntityKind::Wall { a, b, .. } => out.push(Seg::Line(*a, *b)),
        EntityKind::Circle { c, r } => out.push(Seg::Arc(Arc::new(*c, *r, 0.0, 0.0))),
        EntityKind::Arc { c, r, a0, a1 } => out.push(Seg::Arc(Arc::new(*c, *r, *a0, *a1))),
        EntityKind::Ellipse { c, rx, ry, rot } => {
            let n = 72;
            let pts: Vec<Vec2> = (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64 * std::f64::consts::TAU;
                    *c + Vec2::new(rx * t.cos(), ry * t.sin()).rotated(*rot)
                })
                .collect();
            for i in 0..n {
                out.push(Seg::Line(pts[i], pts[(i + 1) % n]));
            }
        }
        // Ellipsenbogen: dieselbe Zerlegung wie die Anzeige (kurze Sehnen),
        // damit Fang und Treffer auf der sichtbaren Kurve liegen. Die
        // Toleranz ist fest — Segmente sind Weltgeometrie.
        EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = crate::geom::Ellipse::new(*c, *rx, *ry, *rot);
            let pts = el.arc_points(*a0, *a1, 0.2);
            for w in pts.windows(2) {
                out.push(Seg::Line(w[0], w[1]));
            }
        }
        EntityKind::Polyline { verts, closed } => {
            let pl = Polyline {
                verts: verts.clone(),
                closed: *closed,
            };
            out.extend(pl.segments());
        }
        EntityKind::Spline { verts, closed } => {
            let sp = Spline {
                verts: verts.clone(),
                closed: *closed,
            };
            // Dieselbe Zerlegung wie beim Zeichnen (`render_spline`), sonst
            // läge der Fangpunkt neben der sichtbaren Kurve.
            out.extend(sp.segments());
        }
        EntityKind::Fill {
            boundary, holes, ..
        }
        | EntityKind::Volume {
            boundary, holes, ..
        } => {
            for ring in std::iter::once(boundary).chain(holes.iter()) {
                if ring.len() < 2 {
                    continue;
                }
                for i in 0..ring.len() {
                    out.push(Seg::Line(ring[i], ring[(i + 1) % ring.len()]));
                }
            }
        }
        EntityKind::Dim { .. } | EntityKind::DimChain { .. } => {
            // Gezeichnete Bemaßung — Maßlinie, Hilfslinien und Markierungen
            // versetzt neben den Basispunkten (T0052). Die Grundlinie zwischen
            // den Basispunkten ist unsichtbar und deshalb keine Trefferfläche.
            out.extend(trefferbild(doc, e).segmente);
        }
        EntityKind::Stair {
            a,
            b,
            width,
            steps,
            height,
            auto_steps,
            ..
        } => {
            // Dieselbe Zerlegung wie die Anzeige: Umriss plus Stufenlinien.
            // Vorher war nur die Achse a→b als Trefferfläche hinterlegt — die
            // Treppe ließ sich deshalb nur in ihrer Mitte anfassen und jeder
            // Fangpunkt auf der Kante fehlte.
            let len = a.dist(*b);
            if len < 1.0 || *width < 1.0 {
                out.push(Seg::Line(*a, *b));
            } else {
                let dir = (*b - *a).normalized();
                let nrm = dir.perp();
                let hw = width * 0.5;
                let ecken = [*a + nrm * hw, *b + nrm * hw, *b - nrm * hw, *a - nrm * hw];
                for i in 0..4 {
                    out.push(Seg::Line(ecken[i], ecken[(i + 1) % 4]));
                }
                let mass = crate::stair::treppenmass(len, *height, *steps, *auto_steps);
                let n = mass.stufen.max(1) as usize;
                let run = len / n as f64;
                for i in 1..n {
                    let p = *a + dir * (run * i as f64);
                    out.push(Seg::Line(p + nrm * hw, p - nrm * hw));
                }
            }
        }
        EntityKind::Door { .. } | EntityKind::Window { .. } => {
            // Tür und Fenster zeichnen **versetzt** zu den gespeicherten Werten:
            // die Tür sitzt auf der vorderen Wandkante (Flügel senkrecht zur
            // Wand, Schwungbogen daneben), das Fenster füllt die Wanddicke. Die
            // frühere Strecke `center ± Breite/2` liegt dagegen unsichtbar auf
            // der Wandachse — der Türflügel war damit nicht anklickbar (T0071).
            // Trefferflächen deshalb aus der Anzeige, wie bei der Bemaßung.
            out.extend(trefferbild(doc, e).segmente);
        }
        EntityKind::Text { p, .. } | EntityKind::Point { p } => {
            // Punktgeometrie über BBox
            out.push(Seg::Line(*p, *p));
        }
        EntityKind::Elevation { p, .. } => out.push(Seg::Line(*p, *p)),
        EntityKind::BlockRef {
            block,
            pos,
            rot,
            scale,
            mirror,
        } => {
            // Die Kindgeometrie liegt blocklokal — erst die Abbildung der
            // Instanz anwenden, sonst würde an der Blockdefinition im Ursprung
            // getroffen statt an der eingefügten Stelle.
            if depth > 12 {
                return out;
            }
            if let Some(def) = doc.block(block) {
                let mut m = Affine::translate(*pos);
                m = m * Affine::rotate(*rot);
                if *mirror {
                    m = m * Affine::scale(-1.0, 1.0);
                }
                m = m * Affine::scale(scale.x, scale.y);
                for ce in &def.entities {
                    let kind = transform::transform_kind(&ce.kind, &m);
                    let mut te = ce.clone();
                    te.kind = kind;
                    out.extend(entity_segments_tief(doc, &te, depth + 1));
                }
            }
        }
    }
    out
}

/// Mittelpunkt / Zentren für Fang.
pub fn entity_centers(doc: &Document, e: &Entity) -> Vec<Vec2> {
    match &e.kind {
        EntityKind::Circle { c, .. } => vec![*c],
        EntityKind::Arc { c, .. } => vec![*c],
        EntityKind::Ellipse { c, .. } => vec![*c],
        EntityKind::EllipseArc { c, .. } => vec![*c],
        EntityKind::Polyline { verts, closed } => {
            let pl = Polyline {
                verts: verts.clone(),
                closed: *closed,
            };
            pl.segments()
                .iter()
                .filter_map(|s| match s {
                    Seg::Arc(a) => Some(a.c),
                    _ => None,
                })
                .collect()
        }
        EntityKind::Dim { a, b, .. } => vec![(*a + *b) * 0.5],
        _ => vec![],
    }
}

/// Anzeigename des Maßstabs für ein Blatt.
pub fn sheet_scale_label(doc: &Document, sheet: &Sheet) -> String {
    let _ = doc;
    format_scale(sheet.scale)
}
