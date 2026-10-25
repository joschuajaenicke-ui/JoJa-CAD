//! Gemeinsamer Renderer: erzeugt aus dem Dokument eine Displayliste.
//!
//! GUI (egui), PDF-Export und PNG-Rendering arbeiten alle auf dieser Liste —
//! dadurch sehen Bildschirm, PDF und Vorschau identisch aus.

use crate::geom::*;
use crate::model::*;
use crate::style::*;
use crate::transform;
use crate::units::{dim_text, elevation_text, format_number, format_scale};

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
            Shape::Fill { outer, .. } => BBox::from_points(outer),
            Shape::Dot { p, r } => BBox::new(*p - v(*r, *r), *p + v(*r, *r)),
            Shape::Text(t) => {
                let w = estimate_text_width(&t.text, t.height);
                let h = t.height;
                let (ax, ay) = match t.align {
                    TextAlignH::Left => (0.0, 0.0),
                    TextAlignH::Center => (-w * 0.5, 0.0),
                    TextAlignH::Right => (-w, 0.0),
                };
                let (vx, vy) = match t.valign {
                    TextAlignV::Top => (0.0, -h),
                    TextAlignV::Middle => (0.0, -h * 0.5),
                    TextAlignV::Bottom => (0.0, 0.0),
                };
                let base = t.pos + Vec2::polar(t.angle, 0.0) + Vec2 { x: ax + vx, y: ay + vy };
                let corners = [
                    base,
                    base + Vec2::new(w, 0.0),
                    base + Vec2::new(w, h),
                    base + Vec2::new(0.0, h),
                ];
                let rot = Affine::rotate_about(t.pos, t.angle);
                BBox::from_points(&corners.iter().map(|p| rot.apply(*p)).collect::<Vec<_>>())
            }
        }
    }
}

/// Grobe Textbreite (Faktor 0,58 der Höhe je Zeichen).
pub fn estimate_text_width(text: &str, height: f64) -> f64 {
    text.chars().count() as f64 * height * 0.58
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
        }
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
    // Wände zuerst als reine Füllflächen, danach alle Linien. So kann keine
    // Wandfüllung die Umrandung einer anderen Wand überdecken.
    let mut fills = DisplayList::default();
    let mut rest = DisplayList::default();
    for e in &ctx.doc.entities {
        if matches!(e.kind, EntityKind::Wall { .. }) {
            let mut tmp = DisplayList::default();
            render_entity(ctx, e, Affine::IDENTITY, &mut tmp, 0);
            for prim in tmp.prims {
                if matches!(prim.shape, Shape::Fill { .. }) {
                    fills.push(prim);
                } else {
                    rest.push(prim);
                }
            }
        } else {
            render_entity(ctx, e, Affine::IDENTITY, &mut rest, 0);
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
        if matches!(e.kind, EntityKind::Wall { .. }) {
            for prim in tmp.prims {
                if matches!(prim.shape, Shape::Fill { .. }) {
                    fills.push(prim);
                } else {
                    rest.push(prim);
                }
            }
        } else {
            for prim in tmp.prims {
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
    let ctx = RenderCtx::new(doc);
    let mut dl = DisplayList::default();
    render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    dl.bbox
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
        EntityKind::Polyline { verts, closed } => render_polyline(ctx, e, &st, verts, *closed, xf, dl),
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
                    bold: *bold || st.width > 0.4,
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
            direction,
        } => {
            render_stair(ctx, e, &st, *p0, *p1, *width, *steps, *direction, xf, dl);
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
pub fn render_fill(
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
        (FillPattern::Solid, None) => Some(st.color.mix(Color::WHITE, 0.72)),
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
        match pattern {
            FillPattern::Lines => {
                push_pattern_lines(dl, e, &thin, &pts, &loecher, &[angle], spacing)
            }
            FillPattern::Cross => {
                push_pattern_lines(
                    dl,
                    e,
                    &thin,
                    &pts,
                    &loecher,
                    &[angle, angle + std::f64::consts::FRAC_PI_2],
                    spacing,
                )
            }
            FillPattern::Dots => push_pattern_dots(dl, e, &thin, &pts, &loecher, angle, spacing),
            FillPattern::Insulation => {
                push_pattern_meander(dl, e, &thin, &pts, &loecher, angle, spacing)
            }
            FillPattern::Solid => {}
        }
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

/// Parallele Musterlinien in mehreren Richtungen über eine Kontur legen und
/// am Polygon beschneiden — Löcher bleiben frei.
fn push_pattern_lines(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angles: &[f64],
    spacing: f64,
) {
    let origin = BBox::from_points(pts).center();
    let reach = fill_diag(pts) * 0.5 + spacing;
    // Genug Linien für die ganze Kontur, aber begrenzt — ein sehr kleiner
    // Abstand darf das Zeichnen nicht lahmlegen.
    let n = ((reach / spacing).ceil() as i64 + 1).clamp(1, 4000);
    for &ang in angles {
        let nrm = Vec2::polar(ang + std::f64::consts::FRAC_PI_2, 1.0);
        let dir = Vec2::polar(ang, 1.0);
        for i in -n..=n {
            let off = nrm * (i as f64 * spacing);
            let a = origin + off - dir * reach;
            let b = origin + off + dir * reach;
            for (q0, q1) in crate::area::clip_line_area(a, b, pts, holes) {
                dl.push(Prim {
                    shape: Shape::Line(q0, q1),
                    style: *st,
                    entity: e.id,
                    layer: e.layer,
                });
            }
        }
    }
}

/// Punktraster im gedrehten Musterraster, nur innerhalb der Kontur.
fn push_pattern_dots(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
) {
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    // Rasterweite notfalls vergröbern, damit eine große Fläche bei kleinem
    // Abstand nicht zehntausende Punkte erzeugt.
    let mut spacing = spacing;
    let half = fill_diag(pts) * 0.5 + spacing;
    while (2.0 * half / spacing).powi(2) > 30000.0 {
        spacing *= 1.5;
    }
    let r = (spacing * 0.055).max(st.width * 0.9).max(0.05);
    let n = ((half / spacing).ceil() as i64 + 1).clamp(1, 4000);
    for i in -n..=n {
        for j in -n..=n {
            let p = origin + dir * (i as f64 * spacing) + nrm * (j as f64 * spacing);
            if !crate::area::point_in_area(p, pts, holes) {
                continue;
            }
            dl.push(Prim {
                shape: Shape::Dot { p, r },
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
        }
    }
}

/// Dämmung: Mäanderlinien (Mineralwolle) entlang der Musterrichtung, am
/// Polygon beschnitten — Löcher bleiben frei.
fn push_pattern_meander(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    pts: &[Vec2],
    holes: &[Vec<Vec2>],
    angle: f64,
    spacing: f64,
) {
    let origin = BBox::from_points(pts).center();
    let dir = Vec2::polar(angle, 1.0);
    let nrm = dir.perp();
    // Rasterweite notfalls vergröbern, damit die Zahl der Wellensegmente
    // begrenzt bleibt (die Displayliste wird je Stand einmal gebaut).
    let mut spacing = spacing;
    let mut half = fill_diag(pts) * 0.5 + spacing;
    while (2.0 * half / spacing) * (2.0 * half / (spacing * 2.0 / 10.0)) > 40000.0 {
        spacing *= 1.5;
        half = fill_diag(pts) * 0.5 + spacing;
    }
    let amp = spacing * 0.45;
    let period = spacing * 2.0;
    // Auflösung so wählen, dass die Welle glatt bleibt und die Zahl der
    // Segmente trotzdem begrenzt ist.
    let step = (period / 10.0).max(half / 400.0).max(0.2);
    let steps = ((half * 2.0) / step).ceil() as i64;
    let n = ((half / spacing).ceil() as i64 + 1).clamp(1, 400);
    for i in -n..=n {
        let base = origin + nrm * (i as f64 * spacing);
        let mut prev: Option<Vec2> = None;
        for k in 0..=steps {
            let s = -half + k as f64 * step;
            let off = ((s / period) * std::f64::consts::TAU).sin() * amp;
            let p = base + dir * s + nrm * off;
            if let Some(q) = prev {
                for (q0, q1) in crate::area::clip_line_area(q, p, pts, holes) {
                    dl.push(Prim {
                        shape: Shape::Line(q0, q1),
                        style: *st,
                        entity: e.id,
                        layer: e.layer,
                    });
                }
            }
            prev = Some(p);
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

/// Grenze der Gehrung als Vielfaches der halben Wanddicke. Wird die
/// Gehrungslinie länger, wird das Wandende gerade abgeschnitten (Stumpfstoß) —
/// sonst schießt sie bei extrem spitzen Ecken meterweit aus und der Wandkörper
/// wird zu einem Splitter.
const MITER_LIMIT: f64 = 6.0;

/// Halbe Breite des Füllbands auf der Anschlusskante (mm). Deckt die
/// Antialiasing-Naht zwischen zwei Wandfüllungen ab; bleibt unter der
/// Außenlinie (Strichbreite ≥ 0,5 mm) verborgen.
const FILL_OVERLAP: f64 = 0.2;

/// Toleranz für „liegt im Nachbarkörper" beim Beschneiden der Kanten.
const INSIDE_EPS: f64 = 0.05;

/// Achsrahmen einer Wand: verschobene Achse, Richtung, Normale, halbe Dicke.
struct WallFrame {
    a: Vec2,
    b: Vec2,
    dir: Vec2,
    n: Vec2,
    h: f64,
}

impl WallFrame {
    fn of(e: &Entity) -> Option<Self> {
        let EntityKind::Wall {
            a,
            b,
            thickness,
            align,
            ..
        } = &e.kind
        else {
            return None;
        };
        if *thickness <= 0.5 {
            return None;
        }
        let dir = (*b - *a).normalized();
        let n = dir.perp();
        let h = thickness * 0.5;
        let shift = n * (h * align.offset());
        Some(Self {
            a: *a + shift,
            b: *b + shift,
            dir,
            n,
            h,
        })
    }

    /// Ungetrimmtes Wandband (Achse ± halbe Dicke) als Viereck.
    fn band(&self) -> Vec<Vec2> {
        vec![
            self.a + self.n * self.h,
            self.b + self.n * self.h,
            self.b - self.n * self.h,
            self.a - self.n * self.h,
        ]
    }
}

/// Zustand eines Wandendes nach dem Anschluss an die Nachbarwände.
struct WallEnd {
    /// Verlängerung des Endes über die Achse hinaus (mm)
    ext: f64,
    /// Ende steckt in einem Nachbarkörper (dann keine Stirnlinie)
    conn: bool,
    /// Stirnecken (außen, innen)
    o: Vec2,
    i: Vec2,
    /// Richtung von der Stirnkante in den Nachbarkörper hinein
    into: Vec2,
    /// Nachbarwand, in der das Ende steckt (für die Füllungs-Überlappung)
    partner: Option<Id>,
    /// Wandstück, das die Gehrungslinie abschneidet, ohne dass der Nachbar es
    /// abdeckt (siehe `miter_patch`); leer, wenn nichts fehlt.
    patch: Vec<Vec2>,
    /// Ende liegt auf einer Gehrungslinie (o→i ist die Naht zum Nachbarn)
    mitered: bool,
}

/// Sucht die Nachbarwand, deren Ende am nächsten an `at` liegt (Eckanschluss).
fn end_partner(doc: &Document, w: &Entity, at: Vec2, h: f64) -> Option<Id> {
    let mut best: Option<(f64, Id)> = None;
    for other in &doc.entities {
        if other.id == w.id {
            continue;
        }
        let Some(f) = WallFrame::of(other) else {
            continue;
        };
        let d = f.a.dist(at).min(f.b.dist(at));
        if d > h + f.h + 2.0 {
            continue;
        }
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, other.id));
        }
    }
    best.map(|(_, id)| id)
}

/// Gehrung an einem Wandende: Knotenpunkt = Schnitt der beiden verschobenen
/// Achsen, Gehrungslinie = Winkelhalbierende. Die Stirnecken sind deren
/// Schnitt mit den beiden Längskanten. `None`, wenn es keine brauchbare
/// Gehrung gibt (kein Eckanschluss oder Länge über der Grenze).
fn miter_corners(
    doc: &Document,
    at: Vec2,
    outward: Vec2,
    n: Vec2,
    h: f64,
    partner: Id,
) -> Option<(Vec2, Vec2, Vec2)> {
    let other = doc.entity(partner)?;
    let f = WallFrame::of(other)?;
    let joint = line_line(at, at + outward, f.a, f.b, true, true)?;
    // Richtung des Partners vom Knoten weg
    let away = if f.a.dist(joint) <= f.b.dist(joint) {
        f.dir
    } else {
        -f.dir
    };
    let m = away - outward;
    if m.len() < 1e-6 {
        return None;
    }
    let m = m.normalized();
    let o = line_line(joint, joint + m, at + n * h, at + n * h + outward, true, true)?;
    let i = line_line(joint, joint + m, at - n * h, at - n * h + outward, true, true)?;
    // Extrem spitze Ecken: Gehrung meterlang -> lieber stumpf abschneiden
    if o.dist(at).max(i.dist(at)) > MITER_LIMIT * h {
        return None;
    }
    // Quer zur Gehrungslinie in den Nachbarkörper hinein (für die Füllung)
    let into = away - m * away.dot(m);
    let into = if into.len() > 1e-9 {
        into.normalized()
    } else {
        outward
    };
    Some((o, i, into))
}

/// Behält den Teil eines konvexen Polygons auf der Seite `(p - p0)·nrm >= 0`
/// (Sutherland-Hodgman, eine Halbebene).
pub(crate) fn clip_keep_side(poly: &[Vec2], p0: Vec2, nrm: Vec2) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(poly.len() + 2);
    for k in 0..poly.len() {
        let a = poly[k];
        let b = poly[(k + 1) % poly.len()];
        let (da, db) = ((a - p0).dot(nrm), (b - p0).dot(nrm));
        if da >= 0.0 {
            out.push(a);
        }
        if (da > 0.0 && db < 0.0) || (da < 0.0 && db > 0.0) {
            let t = da / (da - db);
            out.push(a + (b - a) * t);
        }
    }
    // Doppelte Punkte (Schnitt genau auf einer Ecke) entfernen
    out.dedup_by(|a, b| a.dist(*b) < 1e-6);
    if out.len() > 1 && out[0].dist(*out.last().unwrap()) < 1e-6 {
        out.pop();
    }
    out
}

/// Fläche, die die Gehrungslinie vom eigenen Wandkörper abschneidet, obwohl der
/// Nachbar sie nicht abdeckt.
///
/// Bei gleich dicken Wänden deckt der Nachbarkörper alles ab, was hinter der
/// Gehrungslinie liegt. Bei unterschiedlich dicken Wänden (oder versetzten
/// Achsen) kippt die Gehrungslinie so weit, dass sie in die eigene Wand
/// hineinschneidet: die Füllung endet dann vor der eigenen Stirnebene, ein
/// Stück Wandkörper fehlt und dessen Kante bleibt ohne Linie (sichtbar als
/// ungerandete Stufe an der Ecke). Dieses Stück wird hier nachgerechnet —
/// eigener Körper bis zur Stirnebene, auf der dem Körper abgewandten Seite
/// der Gehrungslinie.
fn miter_patch(
    at: Vec2,
    outward: Vec2,
    n: Vec2,
    h: f64,
    len: f64,
    o: Vec2,
    i: Vec2,
) -> Vec<Vec2> {
    let d = i - o;
    if d.len() < 1e-6 || len < 1e-6 {
        return vec![];
    }
    let back = at - outward * len;
    let band = [at + n * h, at - n * h, back - n * h, back + n * h];
    // nur hinter der eigenen Stirnebene
    let rest = clip_keep_side(&band, at, -outward);
    if rest.len() < 3 {
        return vec![];
    }
    // Seite der Gehrungslinie bestimmen: die dem Körper abgewandte behalten
    let inside = at - outward * (h.min(len) * 0.5);
    let side = d.cross(inside - o);
    let nrm = if side > 0.0 { -d.perp() } else { d.perp() };
    let patch = clip_keep_side(&rest, o, nrm);
    if patch.len() < 3 || polygon_area(&patch).abs() < 1.0 {
        return vec![];
    }
    patch
}

/// Schnitt der Strecke a→b mit einem konvexen Polygon als Parameterintervall.
fn clip_t_interval(a: Vec2, b: Vec2, poly: &[Vec2]) -> Option<(f64, f64)> {
    let d = b - a;
    let l2 = d.len2();
    if l2 < 1e-12 {
        return None;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for (p, q) in clip_line_polygon(a, b, poly) {
        let t0 = (p - a).dot(d) / l2;
        let t1 = (q - a).dot(d) / l2;
        lo = lo.min(t0.min(t1));
        hi = hi.max(t0.max(t1));
    }
    if lo <= hi {
        Some((lo, hi))
    } else {
        None
    }
}

/// Kleinste Verlängerung t ≥ 0, sodass beide Stirnecken im Nachbarkörper
/// liegen. 0, wenn sie schon darin liegen (dann wird nicht verlängert) —
/// so steckt eine Wand an einem spitzen Stichanschluss nicht über den Nachbarn
/// hinaus. `None`, wenn kein Nachbar den Anschluss aufnimmt.
fn wall_reach(
    doc: &Document,
    w: &Entity,
    at: Vec2,
    outward: Vec2,
    n: Vec2,
    h: f64,
) -> Option<(f64, Id)> {
    let limit = MITER_LIMIT * h + 200.0;
    let mut best: Option<(f64, Id)> = None;
    for other in &doc.entities {
        if other.id == w.id {
            continue;
        }
        let Some(f) = WallFrame::of(other) else {
            continue;
        };
        // Anschluss nur, wenn das Ende den Nachbarn überhaupt berührt
        let touching = [at + n * h, at - n * h]
            .iter()
            .any(|c| dist_point_segment(*c, f.a, f.b) <= h + f.h + 1.0);
        if !touching {
            continue;
        }
        let band = f.band();
        let mut lo = 0.0f64;
        let mut hi = limit;
        let mut ok = true;
        for c in [at + n * h, at - n * h] {
            match clip_t_interval(c, c + outward * limit, &band) {
                Some((t0, t1)) => {
                    lo = lo.max(t0);
                    hi = hi.min(t1);
                }
                None => ok = false,
            }
        }
        if !ok || hi + 1e-9 < lo {
            continue;
        }
        let t = lo.max(0.0);
        if best.map(|(bt, _)| t < bt).unwrap_or(true) {
            best = Some((t, other.id));
        }
    }
    best
}

/// Anschluss eines Wandendes: Gehrung zum Ecknachbarn, sonst so weit
/// verlängern, dass das Ende im Nachbarkörper steckt, sonst freies Ende.
fn wall_end(
    doc: &Document,
    w: &Entity,
    at: Vec2,
    outward: Vec2,
    n: Vec2,
    h: f64,
    len: f64,
) -> WallEnd {
    if let Some(p) = end_partner(doc, w, at, h) {
        if let Some((o, i, into)) = miter_corners(doc, at, outward, n, h, p) {
            return WallEnd {
                ext: 0.0,
                conn: true,
                o,
                i,
                into,
                partner: Some(p),
                patch: miter_patch(at, outward, n, h, len, o, i),
                mitered: true,
            };
        }
    }
    if let Some((t, id)) = wall_reach(doc, w, at, outward, n, h) {
        let p = at + outward * t;
        return WallEnd {
            ext: t,
            conn: true,
            o: p + n * h,
            i: p - n * h,
            into: outward,
            partner: Some(id),
            patch: vec![],
            mitered: false,
        };
    }
    WallEnd {
        ext: 0.0,
        conn: false,
        o: at + n * h,
        i: at - n * h,
        into: outward,
        partner: None,
        patch: vec![],
        mitered: false,
    }
}

/// Liegt die Strecke p–q auf der Geraden durch a–b?
fn on_line(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> bool {
    let d = b - a;
    if d.len() < 1e-9 {
        return false;
    }
    let n = d.normalized().perp();
    (p - a).dot(n).abs() < 1e-6 && (q - a).dot(n).abs() < 1e-6
}

/// Teile einer Strecke, die **außerhalb** eines Polygons liegen (mindestens
/// `eps` vom Rand entfernt). Gegenstück zu `clipped_line_pieces`, das die
/// innen liegenden Teile behält.
fn line_outside_pieces(a: Vec2, b: Vec2, poly: &[Vec2], eps: f64) -> Vec<(Vec2, Vec2)> {
    let d = b - a;
    let l2 = d.len2();
    if poly.len() < 3 || l2 < 1e-12 {
        return vec![];
    }
    let mut ts: Vec<f64> = vec![0.0, 1.0];
    for i in 0..poly.len() {
        if let Some(x) = line_line(a, b, poly[i], poly[(i + 1) % poly.len()], false, false) {
            let t = (x - a).dot(d) / l2;
            if t > 1e-6 && t < 1.0 - 1e-6 {
                ts.push(t);
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
        let p = a + d * (0.5 * (t0 + t1));
        if point_in_polygon(p, poly) {
            continue;
        }
        let near = poly
            .iter()
            .enumerate()
            .map(|(i, q)| dist_point_segment(p, *q, poly[(i + 1) % poly.len()]))
            .fold(f64::INFINITY, f64::min);
        if near <= eps {
            continue;
        }
        out.push((a + d * t0, a + d * t1));
    }
    out
}

/// Körperfläche des Nachbarn am Stoß (gehrungbeschnitten, also ohne den
/// Bereich, den die Gehrungslinie abschneidet). Dient dazu, die Gehrungsnaht
/// nur dort zu zeichnen, wo der Nachbar sie nicht abdeckt.
fn partner_body(doc: &Document, id: Id, at: Vec2) -> Option<Vec<Vec2>> {
    let other = doc.entity(id)?;
    let l = WallLayout::of(doc, other)?;
    let mut best: Option<(f64, Vec<Vec2>)> = None;
    for (s0, s1) in &l.segments {
        let (c0o, c0i, c1o, c1i) = l.corners(*s0, *s1);
        let quad = vec![c0o, c1o, c1i, c0i];
        let d = quad
            .iter()
            .map(|p| p.dist(at))
            .fold(f64::INFINITY, f64::min);
        if best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, quad));
        }
    }
    best.map(|(_, q)| q)
}

/// Wandbänder aller Wände (zum Beschneiden von Kanten und Füllungen).
fn wall_bands(doc: &Document) -> Vec<(Id, Vec<Vec2>)> {
    let mut out = Vec::new();
    for e in &doc.entities {
        if let Some(f) = WallFrame::of(e) {
            out.push((e.id, f.band()));
        }
    }
    out
}

/// Füllband auf der Anschlusskante. Zwei Wandfüllungen stoßen an der
/// Gehrungslinie stumpf aneinander; die Antialiasing-Ränder beider Flächen
/// ergänzen sich dort nur auf ~75 %, was als heller Haarstrich sichtbar wird.
/// Ein Band in Füllfarbe, je `FILL_OVERLAP` links und rechts der Kante, deckt
/// ihn ab. Es liegt vollständig in den beiden Wandkörpern (bzw. unter der
/// Außenlinie), ragt also nicht über die Sichtkante hinaus.
fn push_joint_band(
    dl: &mut DisplayList,
    e: &Entity,
    style: PrimStyle,
    o: Vec2,
    i: Vec2,
    into: Vec2,
    xf: Affine,
) {
    let d = into * FILL_OVERLAP;
    dl.push(Prim {
        shape: Shape::Fill {
            outer: vec![
                xf.apply(o + d),
                xf.apply(i + d),
                xf.apply(i - d),
                xf.apply(o - d),
            ],
            holes: Vec::new(),
        },
        style,
        entity: e.id,
        layer: e.layer,
    });
}

/// Zeichnet eine Wandkante und blendet die Abschnitte aus, die **im Inneren**
/// einer anderen Wand liegen. Die Kante endet damit exakt auf deren Rand —
/// kein Haarspalt, kein Stummel und keine doppelte Linie im Nachbarkörper.
fn push_clipped_line(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    p0: Vec2,
    p1: Vec2,
    others: &[Vec<Vec2>],
    xf: Affine,
) {
    for (a, b) in clipped_line_pieces(p0, p1, others) {
        dl.push(Prim {
            shape: Shape::Line(xf.apply(a), xf.apply(b)),
            style: *st,
            entity: e.id,
            layer: e.layer,
        });
    }
}

/// Zerlegt eine Wandkante in die sichtbaren Abschnitte (außerhalb fremder
/// Wandkörper). Gemeinsame Grundlage für das Zeichnen und den Punktfang —
/// so können Anzeige und Fang nicht auseinanderdriften.
fn clipped_line_pieces(p0: Vec2, p1: Vec2, others: &[Vec<Vec2>]) -> Vec<(Vec2, Vec2)> {
    let d = p1 - p0;
    let l2 = d.len2();
    if l2 < 1e-12 {
        return Vec::new();
    }
    // Schnittpunkte mit allen Nachbarkanten als Unterteilungspunkte
    let mut ts: Vec<f64> = vec![0.0, 1.0];
    for poly in others {
        for i in 0..poly.len() {
            let q0 = poly[i];
            let q1 = poly[(i + 1) % poly.len()];
            if let Some(x) = line_line(p0, p1, q0, q1, false, false) {
                let t: f64 = (x - p0).dot(d) / l2;
                if t > 1e-6 && t < 1.0 - 1e-6 {
                    ts.push(t);
                }
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
        // Nur echt innen liegende Abschnitte ausblenden. Ein Punkt auf dem Rand
        // des Nachbarn (Gehrungsecke) zählt dabei als außen, sonst entsteht dort
        // ein Spalt in der Umrandung.
        let inside = others.iter().any(|poly| {
            let ts = [t0 + (t1 - t0) * 0.25, t0 + (t1 - t0) * 0.5, t0 + (t1 - t0) * 0.75];
            ts.iter().all(|t| {
                let p = p0 + d * *t;
                if !point_in_polygon(p, poly) {
                    return false;
                }
                poly.iter()
                    .enumerate()
                    .map(|(i, q)| dist_point_segment(p, *q, poly[(i + 1) % poly.len()]))
                    .fold(f64::INFINITY, f64::min)
                    > INSIDE_EPS
            })
        });
        if inside {
            continue;
        }
        out.push((p0 + d * t0, p0 + d * t1));
    }
    out
}

/// Aufbau einer Wand: verschobene Achse, Anschlüsse an beiden Enden,
/// Öffnungsintervalle und die festen Abschnitte dazwischen.
///
/// EINE Quelle für Darstellung **und** Fang — würde die Kontur an zwei Stellen
/// gerechnet, weichen gezeichnete Kante und Fangpunkt bei Gehrung/Öffnung
/// still voneinander ab.
struct WallLayout {
    dir: Vec2,
    n: Vec2,
    /// halbe Wanddicke
    h: f64,
    se: WallEnd,
    ee: WallEnd,
    /// Beginn der (verlängerten) Achse
    start: Vec2,
    /// Gesamtlänge inklusive Verlängerungen
    total: f64,
    /// Öffnungsintervalle, um die Verlängerung am Anfang verschoben
    openings: Vec<(f64, f64)>,
    /// feste Abschnitte zwischen den Öffnungen
    segments: Vec<(f64, f64)>,
}

impl WallLayout {
    fn of(doc: &Document, e: &Entity) -> Option<Self> {
        let EntityKind::Wall {
            a,
            b,
            thickness,
            align,
            ..
        } = &e.kind
        else {
            return None;
        };
        if *thickness <= 0.5 {
            return None;
        }
        let len = a.dist(*b);
        if len < 1e-6 {
            return None;
        }
        let dir = (*b - *a).normalized();
        let n = dir.perp();
        let h = thickness * 0.5;
        // Ausrichtung: Wandkörper relativ zur Achse verschieben (links/mittig/rechts)
        let shift = n * (h * align.offset());
        let a = *a + shift;
        let b = *b + shift;

        // Anschlüsse: Gehrung zum Ecknachbarn oder Ende so weit in den Nachbarn
        // schieben, dass beide Stirnecken in dessen Körper liegen.
        let se = wall_end(doc, e, a, -dir, n, h, len);
        let ee = wall_end(doc, e, b, dir, n, h, len);
        let start = a - dir * se.ext;
        let total = len + se.ext + ee.ext;

        // Öffnungen (relativ zum ursprünglichen Wandanfang)
        let openings: Vec<(f64, f64)> = wall_opening_intervals(doc, e)
            .into_iter()
            .map(|(s0, s1)| (s0 + se.ext, s1 + se.ext))
            .collect();

        // Feste Segmente zwischen den Öffnungen
        let mut segments: Vec<(f64, f64)> = Vec::new();
        let mut cursor = 0.0;
        for (s0, s1) in &openings {
            if *s0 > cursor + 0.5 {
                segments.push((cursor, *s0));
            }
            cursor = (*s1).max(cursor);
        }
        if cursor < total - 0.5 {
            segments.push((cursor, total));
        }

        Some(Self {
            dir,
            n,
            h,
            se,
            ee,
            start,
            total,
            openings,
            segments,
        })
    }

    /// Punkt auf der (verlängerten) Achse, `s` ab Wandbeginn gemessen.
    fn p(&self, s: f64) -> Vec2 {
        self.start + self.dir * s
    }

    /// Die vier Ecken eines festen Abschnitts: (außen Anfang, innen Anfang,
    /// außen Ende, innen Ende). An den Wandenden sind es die Stirn-/Gehrungsecken.
    fn corners(&self, s0: f64, s1: f64) -> (Vec2, Vec2, Vec2, Vec2) {
        let p0 = self.p(s0);
        let p1 = self.p(s1);
        let (c0o, c0i) = if s0 < 0.5 {
            (self.se.o, self.se.i)
        } else {
            (p0 + self.n * self.h, p0 - self.n * self.h)
        };
        let (c1o, c1i) = if (s1 - self.total).abs() < 0.5 {
            (self.ee.o, self.ee.i)
        } else {
            (p1 + self.n * self.h, p1 - self.n * self.h)
        };
        (c0o, c0i, c1o, c1i)
    }
}

/// Umgrenzende Linien einer Wand in Weltkoordinaten: Längskanten je Abschnitt,
/// Stirnkanten an freien Enden und Laibungskanten an Öffnungen.
///
/// Das ist die Kontur, die der Betrachter sieht — bewusst **ohne** den Beschnitt
/// an Nachbarwänden: gefangen wird die konstruierte Ecke (z. B. der Gehrungspunkt),
/// nicht der sichtbare Rest der Linie. Leere Liste, wenn die Wand zu dünn ist
/// und als einfache Linie gezeichnet wird.
pub fn wall_contour_segments(doc: &Document, e: &Entity) -> Vec<Seg> {
    let Some(l) = WallLayout::of(doc, e) else {
        return vec![];
    };
    let mut out = Vec::new();
    for (s0, s1) in &l.segments {
        let (c0o, c0i, c1o, c1i) = l.corners(*s0, *s1);
        // Längskanten
        out.push(Seg::Line(c0o, c1o));
        out.push(Seg::Line(c0i, c1i));
        // Stirnseiten nur an freien Wandenden (bei Anschluss deckt der Nachbar)
        if *s0 < 0.5 && !l.se.conn {
            out.push(Seg::Line(l.se.o, l.se.i));
        }
        if (*s1 - l.total).abs() < 0.5 && !l.ee.conn {
            out.push(Seg::Line(l.ee.o, l.ee.i));
        }
    }
    // Laibungen an Öffnungen
    for (s0, s1) in &l.openings {
        for s in [*s0, *s1] {
            let p = l.p(s);
            out.push(Seg::Line(p + l.n * l.h, p - l.n * l.h));
        }
    }
    out
}

/// Umgrenzende Eckpunkte einer Wand (Endpunkte der Konturlinien, entdoppelt).
pub fn wall_contour_points(doc: &Document, e: &Entity) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    for sg in wall_contour_segments(doc, e) {
        for p in [sg.start(), sg.end()] {
            if !out.iter().any(|q| q.dist(p) <= CONTOUR_DEDUP) {
                out.push(p);
            }
        }
    }
    out
}

/// Toleranz, unterhalb derer zwei Konturpunkte als derselbe Punkt gelten (mm).
/// Gehrungsecken werden je Wand über dieselben Schnitte gerechnet; die
/// Rundungsreste liegen weit unter dieser Grenze.
const CONTOUR_DEDUP: f64 = 0.1;

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
    let Some(layout) = WallLayout::of(ctx.doc, e) else {
        return;
    };
    let dir = layout.dir;
    let n = layout.n;
    let h = layout.h;
    let (se, ee) = (&layout.se, &layout.ee);
    let (conn_a, conn_b) = (se.conn, ee.conn);
    let (s_out, s_in) = (se.o, se.i);
    let (e_out, e_in) = (ee.o, ee.i);
    let start = layout.start;
    let total = layout.total;
    let openings = &layout.openings;
    let segments = &layout.segments;

    let fill = e
        .style
        .fill
        .unwrap_or_else(|| st.color.mix(Color::WHITE, 0.74));

    // Andere Wände (Bänder) für das Beschneiden von Kanten und Füllungen
    let bands = wall_bands(ctx.doc);
    let others: Vec<Vec<Vec2>> = bands
        .iter()
        .filter(|(id, _)| *id != e.id)
        .map(|(_, q)| q.clone())
        .collect();

    for (s0, s1) in segments {
        let p0 = start + dir * *s0;
        let p1 = start + dir * *s1;
        let (c0o, c0i, c1o, c1i) = layout.corners(*s0, *s1);
        let fill_style = PrimStyle {
            color: fill,
            fill: Some(fill),
            width: 0.0,
            ..*st
        };
        dl.push(Prim {
            shape: Shape::Fill {
                outer: vec![
                    xf.apply(c0o),
                    xf.apply(c1o),
                    xf.apply(c1i),
                    xf.apply(c0i),
                ],
                holes: Vec::new(),
            },
            style: fill_style,
            entity: e.id,
            layer: e.layer,
        });
        // Haarnnaht am Anschluss: Füllung minimal in den Nachbarkörper
        // hineinlaufen lassen, aber exakt auf dessen Körper beschnitten.
        if *s0 < 0.5 && conn_a {
            push_joint_band(dl, e, fill_style, s_out, s_in, se.into, xf);
        }
        if (*s1 - total).abs() < 0.5 && conn_b {
            push_joint_band(dl, e, fill_style, e_out, e_in, ee.into, xf);
        }
        // Von der Gehrungslinie abgeschnittenes, vom Nachbarn nicht abgedecktes
        // Wandstück: als Füllung nachreichen, sonst endet der Wandkörper an
        // einer ungerandeten Stufe.
        for (end, near_start) in [(se, true), (ee, false)] {
            let active = if near_start { *s0 < 0.5 } else { (*s1 - total).abs() < 0.5 };
            if !active || end.patch.len() < 3 {
                continue;
            }
            // Liegt das Stück ohnehin im Körper einer anderen Wand, ist es
            // dort schon gefüllt — dann nicht doppelt zeichnen.
            let c = end
                .patch
                .iter()
                .fold(Vec2::ZERO, |s, q| s + *q)
                / end.patch.len() as f64;
            if others.iter().any(|q| point_in_polygon(c, q)) {
                continue;
            }
            dl.push(Prim {
                shape: Shape::Fill {
                    outer: end.patch.iter().map(|q| xf.apply(*q)).collect(),
                    holes: Vec::new(),
                },
                style: fill_style,
                entity: e.id,
                layer: e.layer,
            });
        }
        // Schraffur der Wandfläche
        if let Some(hp) = hatch {
            if hp != FillPattern::Solid {
                let thin = PrimStyle {
                    width: st.width * 0.55,
                    ..*st
                };
                let seg_dir = (p1 - p0).normalized();
                match hp {
                    FillPattern::Insulation => {
                        // Mäanderlinie (Mineralwolle/ Dämmung)
                        let seg_len = p0.dist(p1);
                        let period = (4.0 * h).max(80.0);
                        let steps = ((seg_len / period) * 14.0).ceil().max(4.0) as usize;
                        let mut pts = Vec::with_capacity(steps + 1);
                        for i in 0..=steps {
                            let t = i as f64 / steps as f64;
                            let s_along = t * seg_len;
                            let ang = s_along / period * std::f64::consts::TAU;
                            let off = ang.sin() * (h * 0.9);
                            pts.push(xf.apply(p0 + seg_dir * s_along + n * off));
                        }
                        dl.push(Prim {
                            shape: Shape::Poly(pts, false),
                            style: thin,
                            entity: e.id,
                            layer: e.layer,
                        });
                    }
                    _ => {
                        let quad_w = [xf.apply(c0o), xf.apply(c1o), xf.apply(c1i), xf.apply(c0i)];
                        // Schraffurfläche = Wandkörper **plus** das nachgereichte
                        // Gehrungsstück; sonst bleibt an spitzen Ecken ein
                        // unschraffierter Keil neben der Gehrung stehen.
                        let mut flechen: Vec<Vec<Vec2>> = vec![quad_w.to_vec()];
                        for (end, near_start) in [(se, true), (ee, false)] {
                            let active =
                                if near_start { *s0 < 0.5 } else { (*s1 - total).abs() < 0.5 };
                            if active && end.patch.len() >= 3 {
                                flechen.push(end.patch.iter().map(|q| xf.apply(*q)).collect());
                            }
                        }
                        let mut angles = vec![std::f64::consts::FRAC_PI_4];
                        if hp == FillPattern::Cross {
                            angles.push(std::f64::consts::FRAC_PI_4 + std::f64::consts::FRAC_PI_2);
                        }
                        let sp = (thickness * 0.25).max(12.0);
                        let origin = BBox::from_points(&quad_w).center();
                        for ang in angles {
                            let nrm = Vec2::polar(ang + std::f64::consts::FRAC_PI_2, 1.0);
                            let dirh = Vec2::polar(ang, 1.0);
                            for i in -80..=80 {
                                let off = nrm * (i as f64 * sp);
                                let p_start = origin + off - dirh * 4000.0;
                                let p_end = origin + off + dirh * 4000.0;
                                for fl in &flechen {
                                    for (q0, q1) in clip_line_polygon(p_start, p_end, fl) {
                                        dl.push(Prim {
                                            shape: Shape::Line(q0, q1),
                                            style: thin,
                                            entity: e.id,
                                            layer: e.layer,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Längskanten (im Inneren der Nachbarwände beschnitten)
        push_clipped_line(dl, e, st, c0o, c1o, &others, xf);
        push_clipped_line(dl, e, st, c0i, c1i, &others, xf);
        // Stirnseiten nur an freien Wandenden
        if *s0 < 0.5 && !conn_a {
            push_clipped_line(dl, e, st, s_out, s_in, &others, xf);
        }
        if (*s1 - total).abs() < 0.5 && !conn_b {
            push_clipped_line(dl, e, st, e_out, e_in, &others, xf);
        }
        // Kanten des nachgereichten Gehrungsstücks (nur außerhalb der
        // Nachbarkörper; im Nachbarn liegende Teile blendet der Beschnitt aus)
        for (end, near_start) in [(se, true), (ee, false)] {
            let active = if near_start { *s0 < 0.5 } else { (*s1 - total).abs() < 0.5 };
            if !active || end.patch.len() < 3 {
                continue;
            }
            let c = end.patch.iter().fold(Vec2::ZERO, |s, q| s + *q) / end.patch.len() as f64;
            if others.iter().any(|q| point_in_polygon(c, q)) {
                continue;
            }
            for k in 0..end.patch.len() {
                let q0 = end.patch[k];
                let q1 = end.patch[(k + 1) % end.patch.len()];
                // Die Gehrungsnaht selbst zeichnet der Block darunter
                if on_line(q0, q1, end.o, end.i) {
                    continue;
                }
                push_clipped_line(dl, e, st, q0, q1, &others, xf);
            }
        }
        // Gehrungsnaht: nur die Stücke, die der Nachbar nicht abdeckt. Bei
        // gleich dicken Wänden deckt er die ganze Naht ab (nichts zu zeichnen);
        // bei unterschiedlich dicken Wänden bleiben Zipfel übrig, die sonst als
        // ungerandete Füllkante enden.
        for (end, near_start) in [(se, true), (ee, false)] {
            let active = if near_start { *s0 < 0.5 } else { (*s1 - total).abs() < 0.5 };
            let Some(pid) = end.partner else { continue };
            if !active || !end.mitered {
                continue;
            }
            let Some(body) = partner_body(ctx.doc, pid, end.o) else {
                continue;
            };
            // Stücke außerhalb des Nachbarkörpers; die an das nachgereichte
            // Gehrungsstück angrenzenden Teile liegen danach wieder im
            // Wandinneren und entfallen.
            let mut pieces = line_outside_pieces(end.o, end.i, &body, 0.1);
            if end.patch.len() >= 3 {
                pieces = pieces
                    .into_iter()
                    .flat_map(|(a, b)| line_outside_pieces(a, b, &end.patch, 0.1))
                    .collect();
            }
            for (a, b) in pieces {
                push_clipped_line(dl, e, st, a, b, &others, xf);
            }
        }
    }

    // Laibungen an Öffnungen
    for (s0, s1) in openings {
        for s in [*s0, *s1] {
            let p = start + dir * s;
            dl.push(Prim {
                shape: Shape::Line(xf.apply(p + n * h), xf.apply(p - n * h)),
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
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

#[allow(clippy::too_many_arguments)]
fn render_stair(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    a: Vec2,
    b: Vec2,
    width: f64,
    steps: u32,
    direction: StairDir,
    xf: Affine,
    dl: &mut DisplayList,
) {
    let dir = (b - a).normalized();
    let nrm = dir.perp();
    let len = a.dist(b);
    let hw = width * 0.5;
    let quad = vec![
        xf.apply(a + nrm * hw),
        xf.apply(b + nrm * hw),
        xf.apply(b - nrm * hw),
        xf.apply(a - nrm * hw),
    ];
    dl.push(Prim {
        shape: Shape::Poly(quad, true),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
    let n = steps.max(1);
    let run = len / n as f64;
    for i in 1..n {
        let t = i as f64 * run;
        let p = a + dir * t;
        dl.push(Prim {
            shape: Shape::Line(xf.apply(p + nrm * hw), xf.apply(p - nrm * hw)),
            style: PrimStyle {
                width: st.width * 0.75,
                ..*st
            },
            entity: e.id,
            layer: e.layer,
        });
    }
    // Laufrichtung + Steigungslinie
    let mid = (a + b) * 0.5;
    let arrow_len = (len * 0.5).min(1600.0);
    let arrow_start = mid - dir * (arrow_len * 0.5);
    let arrow_end = mid + dir * (arrow_len * 0.5);
    dl.push(Prim {
        shape: Shape::Line(xf.apply(arrow_start), xf.apply(arrow_end)),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
    let ah = ctx.a(1.2);
    let head = |sign: f64| {
        let tip = if sign > 0.0 { arrow_end } else { arrow_start };
        let d = dir * sign;
        vec![
            xf.apply(tip),
            xf.apply(tip - d * ah + nrm * (ah * 0.45)),
            xf.apply(tip - d * ah - nrm * (ah * 0.45)),
        ]
    };
    let label = match direction {
        StairDir::Up => "AUF",
        StairDir::Down => "AB",
        StairDir::Both => "AUF/AB",
    };
    let tip_dir = match direction {
        StairDir::Down => -1.0,
        _ => 1.0,
    };
    let tri = head(tip_dir);
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
    if direction == StairDir::Both {
        dl.push(Prim {
            shape: Shape::Poly(head(-1.0), true),
            style: PrimStyle {
                color: st.color,
                fill: Some(st.color),
                ..*st
            },
            entity: e.id,
            layer: e.layer,
        });
    }
    let text_pos = mid + nrm * (hw + ctx.a(1.4));
    dl.push(Prim {
        shape: Shape::Text(TextPrim {
            pos: xf.apply(text_pos),
            text: label.to_string(),
            height: ctx.a(2.0) * xf.scale_factor(),
            angle: dir.angle() + xf.rotation(),
            align: TextAlignH::Center,
            valign: TextAlignV::Middle,
            bold: false,
        }),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
}

// ---------------------------------------------------------------------------
// Bemaßung (DIN 406 / DIN 1356)
// ---------------------------------------------------------------------------

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

fn dim_geometry(
    ctx: &RenderCtx,
    p0: Vec2,
    p1: Vec2,
    line_pos: Vec2,
    axis: DimAxis,
    override_text: Option<&str>,
) -> DimGeom {
    let doc = ctx.doc;
    let (wa, wb, da, db, measure, angle) = match axis {
        DimAxis::X => {
            let y = line_pos.y;
            let m = (p1.x - p0.x).abs();
            (p0, p1, v(p0.x, y), v(p1.x, y), m, 0.0)
        }
        DimAxis::Y => {
            let x = line_pos.x;
            let m = (p1.y - p0.y).abs();
            (p0, p1, v(x, p0.y), v(x, p1.y), m, std::f64::consts::FRAC_PI_2)
        }
        DimAxis::Aligned => {
            let dir = (p1 - p0).normalized();
            let n = dir.perp();
            let off = (line_pos - p0).dot(n);
            let da = p0 + n * off;
            let db = p1 + n * off;
            (p0, p1, da, db, p0.dist(p1), dir.angle())
        }
    };
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
    let text_gap = ctx.a(style.text_gap);

    // Hilfslinien
    for (w, d) in [(g.wa, g.da), (g.wb, g.db)] {
        let dir = (d - w).normalized();
        let s = w + dir * ext_gap.min(w.dist(d) * 0.5);
        let t = d + dir * over;
        dl.push(Prim {
            shape: Shape::Line(xf.apply(s), xf.apply(t)),
            style: PrimStyle {
                width: st.width * 0.8,
                ..*st
            },
            entity: e.id,
            layer: e.layer,
        });
    }
    // Maßlinie
    dl.push(Prim {
        shape: Shape::Line(xf.apply(g.da), xf.apply(g.db)),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
    // Markierungen
    let dir = Vec2::polar(g.angle, 1.0);
    match style.arrow {
        ArrowKind::Slash => {
            let d = Vec2::polar(g.angle + std::f64::consts::FRAC_PI_4, arrow * 0.7);
            for p in [g.da, g.db] {
                dl.push(Prim {
                    shape: Shape::Line(xf.apply(p - d), xf.apply(p + d)),
                    style: PrimStyle {
                        width: st.width * 1.2,
                        ..*st
                    },
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
        (mid + n * (text_gap + text_h * 0.15), TextAlignH::Center)
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
    let text_gap = ctx.a(style.text_gap);
    let angle = match axis {
        DimAxis::X => 0.0,
        DimAxis::Y => std::f64::consts::FRAC_PI_2,
        DimAxis::Aligned => (pts[1] - pts[0]).angle(),
    };
    let dir = Vec2::polar(angle, 1.0);
    let n = dir.perp();

    // Maßlinie
    let projected: Vec<Vec2> = pts
        .iter()
        .map(|p| match axis {
            DimAxis::X => v(p.x, line_pos.y),
            DimAxis::Y => v(line_pos.x, p.y),
            DimAxis::Aligned => {
                let off = (line_pos - pts[0]).dot(n);
                *p + n * off
            }
        })
        .collect();
    dl.push(Prim {
        shape: Shape::Line(xf.apply(projected[0]), xf.apply(*projected.last().unwrap())),
        style: *st,
        entity: e.id,
        layer: e.layer,
    });
    // Hilfslinien an allen Punkten
    for (w, d) in pts.iter().zip(projected.iter()) {
        let dd = (*d - *w).normalized();
        let s = *w + dd * ext_gap.min(w.dist(*d) * 0.5);
        let t = *d + dd * over;
        dl.push(Prim {
            shape: Shape::Line(xf.apply(s), xf.apply(t)),
            style: PrimStyle {
                width: st.width * 0.8,
                ..*st
            },
            entity: e.id,
            layer: e.layer,
        });
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
        // Markierungen
        match style.arrow {
            ArrowKind::Slash => {
                let d = Vec2::polar(angle + std::f64::consts::FRAC_PI_4, arrow * 0.7);
                for p in [p0, p1] {
                    dl.push(Prim {
                        shape: Shape::Line(xf.apply(p - d), xf.apply(p + d)),
                        style: PrimStyle {
                            width: st.width * 1.2,
                            ..*st
                        },
                        entity: e.id,
                        layer: e.layer,
                    });
                }
            }
            ArrowKind::Dot => {
                for p in [p0, p1] {
                    dl.push(Prim {
                        shape: Shape::Dot {
                            p: xf.apply(p),
                            r: arrow * 0.28,
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
            ArrowKind::None => {}
        }
        let mid = (p0 + p1) * 0.5;
        let text_w = estimate_text_width(&txt, text_h);
        let span = p0.dist(p1);
        let mut ta = angle;
        if ta > std::f64::consts::FRAC_PI_2 + 0.01 || ta < -std::f64::consts::FRAC_PI_2 - 0.01 {
            ta += std::f64::consts::PI;
        }
        let pos = if text_w + a > span {
            p1 + dir * (text_w * 0.5 + a * 1.5)
        } else {
            mid + n * (text_gap + text_h * 0.15)
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
    let (w, h) = sheet.size();
    let ctx = RenderCtx {
        doc,
        annot_scale: 1.0,
        layer_filter: None,
        resolve_text: true,
        selection: None,
        sheet_scale: Some(sheet.scale),
    };
    // Papierraum-Objekte (Schriftfeld etc.)
    for e in &sheet.entities {
        render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    }
    // Rahmen
    if sheet.frame {
        let m = 10.0;
        let left = m + 5.0; // Heftrand
        dl.push(Prim {
            shape: Shape::Poly(
                vec![
                    v(left, m),
                    v(w - m, m),
                    v(w - m, h - m),
                    v(left, h - m),
                ],
                true,
            ),
            style: PrimStyle {
                color: Color::BLACK,
                line_type: LineType::Solid,
                width: 0.35,
                fill: None,
            },
            entity: 0,
            layer: 0,
        });
    }
    dl
}

/// Inhalt eines Viewports (Weltkoordinaten), beschnitten auf das Fenster.
pub fn render_viewport(doc: &Document, vp: &Viewport) -> DisplayList {
    let ctx = RenderCtx {
        doc,
        annot_scale: vp.scale.max(0.01),
        layer_filter: vp.layers.clone(),
        resolve_text: true,
        selection: None,
        sheet_scale: None,
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
        EntityKind::Polyline { verts, closed } => {
            let pl = Polyline {
                verts: verts.clone(),
                closed: *closed,
            };
            out.extend(pl.segments());
        }
        EntityKind::Fill {
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
        EntityKind::Dim { a, b, .. } => out.push(Seg::Line(*a, *b)),
        EntityKind::DimChain { pts, .. } => {
            for i in 0..pts.len().saturating_sub(1) {
                out.push(Seg::Line(pts[i], pts[i + 1]));
            }
        }
        EntityKind::Stair { a, b, .. } => out.push(Seg::Line(*a, *b)),
        EntityKind::Door { center, angle, width, .. } => {
            let d = Vec2::polar(*angle, *width * 0.5);
            out.push(Seg::Line(*center - d, *center + d));
        }
        EntityKind::Window { center, angle, width, .. } => {
            let d = Vec2::polar(*angle, *width * 0.5);
            out.push(Seg::Line(*center - d, *center + d));
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
