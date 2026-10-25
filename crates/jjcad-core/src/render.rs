//! Gemeinsamer Renderer: erzeugt aus dem Dokument eine Displayliste.
//!
//! GUI (egui), PDF-Export und PNG-Rendering arbeiten alle auf dieser Liste —
//! dadurch sehen Bildschirm, PDF und Vorschau identisch aus.

use crate::geom::*;
use crate::model::*;
use crate::style::*;
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
    Fill(Vec<Vec2>),
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
            Shape::Fill(pts) => BBox::from_points(pts),
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
                if matches!(prim.shape, Shape::Fill(_)) {
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
        EntityKind::Hatch {
            boundary,
            pattern,
            angle,
            spacing,
            filled,
        } => {
            let pts: Vec<Vec2> = boundary.iter().map(|p| xf.apply(*p)).collect();
            let fill = e.style.fill.unwrap_or(st.color);
            let fill = fill.mix(Color::WHITE, 0.72);
            if *filled || *pattern == HatchPattern::Solid {
                dl.push(Prim {
                    shape: Shape::Fill(pts.clone()),
                    style: PrimStyle {
                        color: fill,
                        fill: Some(fill),
                        ..st
                    },
                    entity: e.id,
                    layer: e.layer,
                });
            }
            match pattern {
                HatchPattern::Solid => {
                    if !*filled {
                        dl.push(Prim {
                            shape: Shape::Fill(pts),
                            style: PrimStyle {
                                color: fill,
                                fill: Some(fill),
                                ..st
                            },
                            entity: e.id,
                            layer: e.layer,
                        });
                    }
                }
                p => {
                    let mut angles: Vec<f64> = vec![*angle];
                    if *p == HatchPattern::Cross {
                        angles.push(*angle + std::f64::consts::FRAC_PI_2);
                    }
                    let sp = spacing.max(1.0) * xf.scale_factor();
                    for ang in angles {
                        let n = Vec2::polar(ang + std::f64::consts::FRAC_PI_2, 1.0);
                        let d = Vec2::polar(ang, 1.0);
                        let origin = BBox::from_points(&pts).center();
                        for i in -200..=200 {
                            let off = n * (i as f64 * sp);
                            let p0 = origin + off - d * 500000.0;
                            let p1 = origin + off + d * 500000.0;
                            for (a0, a1) in clip_line_polygon(p0, p1, &pts) {
                                dl.push(Prim {
                                    shape: Shape::Line(a0, a1),
                                    style: PrimStyle {
                                        color: st.color,
                                        width: st.width * 0.6,
                                        ..st
                                    },
                                    entity: e.id,
                                    layer: e.layer,
                                });
                            }
                        }
                    }
                }
            }
        }
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
            align,
            ..
        } => {
            render_wall(ctx, e, &st, *p0, *p1, *thickness, *hatch, *align, xf, dl);
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

fn wall_quads(doc: &Document) -> Vec<(Id, Vec<Vec2>)> {
    let mut out = Vec::new();
    for e in &doc.entities {
        if let EntityKind::Wall {
            a,
            b,
            thickness,
            align,
            ..
        } = &e.kind
        {
            if *thickness <= 0.5 {
                continue;
            }
            let dir = (*b - *a).normalized();
            let n = dir.perp();
            let h = thickness * 0.5;
            // Ausrichtung wie beim Zeichnen
            let (a, b) = (*a + n * (h * align.offset()), *b + n * (h * align.offset()));
            let (ea, eb, _, _) = wall_join(doc, e, a, b, *thickness);
            let start = a - dir * ea;
            let end = b + dir * eb;
            let (s_o, s_i) = wall_end_corners(doc, e, a, -dir, n, h)
                .map(|(o, i, _)| (o, i))
                .unwrap_or((start + n * h, start - n * h));
            let (e_o, e_i) = wall_end_corners(doc, e, b, dir, n, h)
                .map(|(o, i, _)| (o, i))
                .unwrap_or((end + n * h, end - n * h));
            out.push((e.id, vec![s_o, e_o, e_i, s_i]));
        }
    }
    out
}

/// Verlängerung der Wandenden an Anschlüssen (halbierte Dicke der Nachbarwand).

/// Ecken eines Wandendes. Bei einem Eckanschluss (zwei Wände treffen sich in
/// einem Punkt) wird die Stirnkante entlang der **Winkelhalbierenden** beider
/// Achsen geführt — so schließen die Außenkanten ohne Spalt und ohne Stummel.
fn wall_end_corners(
    doc: &Document,
    e: &Entity,
    at: Vec2,
    outward: Vec2,
    n: Vec2,
    h: f64,
) -> Option<(Vec2, Vec2, Id)> {
    let mut best: Option<(f64, Vec2, Id, Vec2, Vec2)> = None;
    for other in &doc.entities {
        if other.id == e.id {
            continue;
        }
        let EntityKind::Wall {
            a: oa,
            b: ob,
            thickness: ot,
            align: oalign,
            ..
        } = &other.kind
        else {
            continue;
        };
        if *ot <= 0.5 {
            continue;
        }
        let oh = ot * 0.5;
        let d_end = (oa.dist(at)).min(ob.dist(at));
        if d_end > h + oh + 2.0 + oh.abs() {
            continue;
        }
        // Achse des Partners, aber um dessen Ausrichtung verschoben
        let odir = (*ob - *oa).normalized();
        let onrm = odir.perp();
        let oshift = onrm * (oh * oalign.offset());
        let oa2 = *oa + oshift;
        let ob2 = *ob + oshift;
        let d_other = (*ob - *oa).normalized();
        let away = if oa.dist(at) <= ob.dist(at) {
            d_other
        } else {
            -d_other
        };
        if best.map(|(d, ..)| d_end < d).unwrap_or(true) {
            best = Some((d_end, away, other.id, oa2, ob2));
        }
    }
    let (_, away, partner, oa2, ob2) = best?;
    // gemeinsamer Knotenpunkt: Schnitt der beiden (verschobenen) Achsen
    let joint = line_line(at, at + outward, oa2, ob2, true, true)?;
    let mut m = away - outward;
    if m.len() < 1e-6 {
        return None;
    }
    m = m.normalized();
    let l1 = line_line(joint, joint + m, at + n * h, at + n * h + outward, true, true)?;
    let l2 = line_line(joint, joint + m, at - n * h, at - n * h + outward, true, true)?;
    Some((l1, l2, partner))
}

/// Anschluss einer Wand an ihre Nachbarn: Verlängerung bis zum Schnittpunkt der
/// Achsen und ob das Ende angeschlossen ist (dann keine Stirnlinie zeichnen).
fn wall_join(
    doc: &Document,
    e: &Entity,
    a: Vec2,
    b: Vec2,
    thickness: f64,
) -> (f64, f64, bool, bool) {
    let dir = (b - a).normalized();
    let h = thickness * 0.5;
    let (mut ea, mut eb) = (0.0f64, 0.0f64);
    let (mut ca, mut cb) = (false, false);
    for other in &doc.entities {
        if other.id == e.id {
            continue;
        }
        let EntityKind::Wall {
            a: oa,
            b: ob,
            thickness: ot,
            align: oalign,
            ..
        } = &other.kind
        else {
            continue;
        };
        if *ot <= 0.5 {
            continue;
        }
        let oh = ot * 0.5;
        // Achse des Partners in dessen eigener Ausrichtung
        let oshift = (*ob - *oa).normalized().perp() * (oh * oalign.offset());
        let oa2 = *oa + oshift;
        let ob2 = *ob + oshift;
        for (end, ext, conn) in [(a, &mut ea, &mut ca), (b, &mut eb, &mut cb)] {
            let touching = end.dist(oa2) <= h + oh + 1.0
                || end.dist(ob2) <= h + oh + 1.0
                || dist_point_segment(end, oa2, ob2) <= h + oh + 1.0;
            if !touching {
                continue;
            }
            *conn = true;
            let limit = thickness * 12.0 + 50.0;
            // Eckanschluss: keine Verlaengerung, die Stirnkante liegt auf der
            // Winkelhalbierenden (siehe wall_end_corners).
            if (oa2.dist(end)).min(ob2.dist(end)) <= h + oh + 2.0 {
                continue;
            }
            if let Some(x) = line_line(a, b, oa2, ob2, false, false) {
                let t = (x - end).dot(dir);
                if t < -0.5 && -t < limit {
                    *ext = ext.max(-t);
                }
            }
            let d_other = (ob2 - oa2).normalized();
            let away = if (oa2 - end).len() <= (ob2 - end).len() {
                d_other
            } else {
                -d_other
            };
            let u = if end == a { -dir } else { dir };
            let n_u = u.perp();
            let cross = u.x * away.y - u.y * away.x;
            if cross.abs() > 1e-6 {
                let mut best_t: f64 = 0.0;
                for s1 in [1.0f64, -1.0] {
                    for s2 in [1.0f64, -1.0] {
                        let t = (-s2 * oh - s1 * h * (n_u.x * away.y - n_u.y * away.x)) / cross;
                        if t > best_t && t < limit {
                            best_t = t;
                        }
                    }
                }
                if best_t > 0.0 {
                    *ext = ext.max(best_t);
                }
            }
        }
    }
    (ea, eb, ca, cb)
}

fn push_face_line(
    dl: &mut DisplayList,
    e: &Entity,
    st: &PrimStyle,
    p0: Vec2,
    p1: Vec2,
    others: &[Vec<Vec2>],
    xf: Affine,
) {
    let d = p1 - p0;
    let l2 = d.len2();
    if l2 < 1e-12 {
        return;
    }
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
    for w in ts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if t1 - t0 < 1e-9 {
            continue;
        }
        let mid = p0 + d * ((t0 + t1) * 0.5);
        // Nur ausblenden, wenn der Punkt deutlich INNERHALB einer anderen Wand liegt.
        // Liegt er nur knapp an deren Rand (Gehrungsspitze), bleibt die Linie stehen —
        // sonst entsteht an spitzen Ecken ein Spalt in der Umrandung.
        let inside = others.iter().any(|poly| {
            if !point_in_polygon(mid, poly) {
                return false;
            }
            let mut clearance = f64::INFINITY;
            for i in 0..poly.len() {
                let q0 = poly[i];
                let q1 = poly[(i + 1) % poly.len()];
                clearance = clearance.min(dist_point_segment(mid, q0, q1));
            }
            clearance > 0.75
        });
        if inside {
            continue;
        }
        dl.push(Prim {
            shape: Shape::Line(xf.apply(p0 + d * t0), xf.apply(p0 + d * t1)),
            style: *st,
            entity: e.id,
            layer: e.layer,
        });
    }
}

fn render_wall(
    ctx: &RenderCtx,
    e: &Entity,
    st: &PrimStyle,
    a: Vec2,
    b: Vec2,
    thickness: f64,
    hatch: Option<HatchPattern>,
    align: crate::style::WallAlign,
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
    let dir = (b - a).normalized();
    let n = dir.perp();
    let len = a.dist(b);
    let h = thickness * 0.5;
    // Ausrichtung: Wandkörper relativ zur Achse verschieben (links/mittig/rechts)
    let shift = n * (h * align.offset());
    let a = a + shift;
    let b = b + shift;

    // Anschlüsse: Wandenden verlängern, damit Ecken geschlossen sind
    let (ext_a, ext_b, conn_a, conn_b) = wall_join(ctx.doc, e, a, b, thickness);
    let start = a - dir * ext_a;
    let end = b + dir * ext_b;
    let total = len + ext_a + ext_b;
    // Gehrungsecken an den Enden (Winkelhalbierende) bzw. senkrechte Stirnkanten
    let sc = wall_end_corners(ctx.doc, e, a, -dir, n, h);
    let ec = wall_end_corners(ctx.doc, e, b, dir, n, h);
    let (s_out, s_in) = sc
        .map(|(o, i, _)| (o, i))
        .unwrap_or((start + n * h, start - n * h));
    let (e_out, e_in) = ec
        .map(|(o, i, _)| (o, i))
        .unwrap_or((end + n * h, end - n * h));
    // Die Gehrungspartner dürfen die eigenen Kanten nicht wegschneiden:
    // die Kante endet genau auf deren Rand.
    let partners: Vec<Id> = [sc.map(|(_, _, p)| p), ec.map(|(_, _, p)| p)]
        .into_iter()
        .flatten()
        .collect();

    // Öffnungen (relativ zum ursprünglichen Wandanfang)
    let openings: Vec<(f64, f64)> = wall_opening_intervals(ctx.doc, e)
        .into_iter()
        .map(|(s0, s1)| (s0 + ext_a, s1 + ext_a))
        .collect();

    let fill = e
        .style
        .fill
        .unwrap_or_else(|| st.color.mix(Color::WHITE, 0.74));


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

    // Andere Wände für das Beschneiden der Wandflächen
    let quads = wall_quads(ctx.doc);
    let others: Vec<Vec<Vec2>> = quads
        .iter()
        .filter(|(id, _)| *id != e.id && !partners.contains(id))
        .map(|(_, q)| q.clone())
        .collect();

    for (s0, s1) in &segments {
        let p0 = start + dir * *s0;
        let p1 = start + dir * *s1;
        let (c0o, c0i) = if *s0 < 0.5 {
            (s_out, s_in)
        } else {
            (p0 + n * h, p0 - n * h)
        };
        let (c1o, c1i) = if (*s1 - total).abs() < 0.5 {
            (e_out, e_in)
        } else {
            (p1 + n * h, p1 - n * h)
        };
        // Füllung an angeschlossenen Enden minimal überlappen lassen, damit an
        // Gehrungsspitzen kein Haarspalt stehen bleibt (Linien bleiben exakt).
        let ov0 = if *s0 < 0.5 && conn_a { -dir * 2.0 } else { Vec2::ZERO };
        let ov1 = if (*s1 - total).abs() < 0.5 && conn_b {
            dir * 2.0
        } else {
            Vec2::ZERO
        };
        let quad = vec![
            xf.apply(c0o + ov0),
            xf.apply(c1o + ov1),
            xf.apply(c1i + ov1),
            xf.apply(c0i + ov0),
        ];
        dl.push(Prim {
            shape: Shape::Fill(quad),
            style: PrimStyle {
                color: fill,
                fill: Some(fill),
                width: 0.0,
                ..*st
            },
            entity: e.id,
            layer: e.layer,
        });
        // Schraffur der Wandfläche
        if let Some(hp) = hatch {
            if hp != HatchPattern::Solid {
                let thin = PrimStyle {
                    width: st.width * 0.55,
                    ..*st
                };
                let seg_dir = (p1 - p0).normalized();
                match hp {
                    HatchPattern::Insulation => {
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
                        let mut angles = vec![std::f64::consts::FRAC_PI_4];
                        if hp == HatchPattern::Cross {
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
                                for (q0, q1) in clip_line_polygon(p_start, p_end, &quad_w) {
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

        // Längskanten (in Anschlussbereichen beschnitten)
        push_face_line(dl, e, st, c0o, c1o, &others, xf);
        push_face_line(dl, e, st, c0i, c1i, &others, xf);
        // Stirnseiten nur an freien Wandenden
        if *s0 < 0.5 && !conn_a {
            dl.push(Prim {
                shape: Shape::Line(xf.apply(s_out), xf.apply(s_in)),
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
        }
        if (*s1 - total).abs() < 0.5 && !conn_b {
            dl.push(Prim {
                shape: Shape::Line(xf.apply(e_out), xf.apply(e_in)),
                style: *st,
                entity: e.id,
                layer: e.layer,
            });
        }
    }

    // Laibungen an Öffnungen
    for (s0, s1) in &openings {
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
        EntityKind::Hatch { boundary, .. } => {
            for i in 0..boundary.len() {
                out.push(Seg::Line(boundary[i], boundary[(i + 1) % boundary.len()]));
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
        EntityKind::BlockRef { block, .. } => {
            if let Some(def) = doc.block(block) {
                for ce in &def.entities {
                    out.extend(entity_segments(doc, ce));
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
