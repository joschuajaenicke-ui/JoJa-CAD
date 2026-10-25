//! Transformation von Entitäten (Verschieben, Drehen, Spiegeln, Skalieren).

use crate::geom::*;
use crate::model::*;

/// Wendet eine affine Abbildung auf eine Entität an (id/Layer bleiben).
pub fn transform_entity(e: &Entity, m: &Affine) -> Entity {
    let mut out = e.clone();
    out.kind = transform_kind(&e.kind, m);
    out
}

fn sx(m: &Affine) -> f64 {
    Vec2::new(m.a, m.b).len()
}
fn sy(m: &Affine) -> f64 {
    Vec2::new(m.c, m.d).len()
}

pub fn transform_kind(k: &EntityKind, m: &Affine) -> EntityKind {
    let f = m.scale_factor();
    let nonuniform = ((sx(m) - sy(m)).abs() > 1e-9).then_some((sx(m), sy(m)));
    match k {
        EntityKind::Point { p } => EntityKind::Point { p: m.apply(*p) },
        EntityKind::Line { a, b } => EntityKind::Line {
            a: m.apply(*a),
            b: m.apply(*b),
        },
        EntityKind::Circle { c, r } => match nonuniform {
            None => EntityKind::Circle {
                c: m.apply(*c),
                r: r * f,
            },
            Some((a, b)) => EntityKind::Ellipse {
                c: m.apply(*c),
                rx: r * a,
                ry: r * b,
                rot: m.rotation(),
            },
        },
        EntityKind::Arc { c, r, a0, a1 } => {
            let rot = m.rotation();
            if m.is_mirrored() {
                EntityKind::Arc {
                    c: m.apply(*c),
                    r: r * f,
                    a0: -a1 + rot,
                    a1: -a0 + rot,
                }
            } else {
                EntityKind::Arc {
                    c: m.apply(*c),
                    r: r * f,
                    a0: a0 + rot,
                    a1: a1 + rot,
                }
            }
        }
        EntityKind::Ellipse { c, rx, ry, rot } => EntityKind::Ellipse {
            c: m.apply(*c),
            rx: rx * sx(m),
            ry: ry * sy(m),
            rot: rot + m.rotation(),
        },
        EntityKind::Polyline { verts, closed } => {
            let mirrored = m.is_mirrored();
            let verts = verts
                .iter()
                .map(|v| Vertex {
                    p: m.apply(v.p),
                    bulge: if mirrored && nonuniform.is_none() { -v.bulge } else { v.bulge },
                })
                .collect();
            EntityKind::Polyline {
                verts,
                closed: *closed,
            }
        }
        EntityKind::Hatch {
            boundary,
            pattern,
            angle,
            spacing,
            filled,
        } => EntityKind::Hatch {
            boundary: boundary.iter().map(|p| m.apply(*p)).collect(),
            pattern: *pattern,
            angle: angle + m.rotation(),
            spacing: spacing * f,
            filled: *filled,
        },
        EntityKind::Text {
            p,
            text,
            height,
            angle,
            align,
            valign,
            bold,
        } => EntityKind::Text {
            p: m.apply(*p),
            text: text.clone(),
            height: height * f,
            angle: angle + m.rotation(),
            align: *align,
            valign: *valign,
            bold: *bold,
        },
        EntityKind::Dim {
            a,
            b,
            line_pos,
            axis,
            override_text,
        } => EntityKind::Dim {
            a: m.apply(*a),
            b: m.apply(*b),
            line_pos: m.apply(*line_pos),
            axis: *axis,
            override_text: override_text.clone(),
        },
        EntityKind::DimChain {
            pts,
            line_pos,
            axis,
        } => EntityKind::DimChain {
            pts: pts.iter().map(|p| m.apply(*p)).collect(),
            line_pos: m.apply(*line_pos),
            axis: *axis,
        },
        EntityKind::Elevation {
            p,
            z,
            angle,
            override_text,
        } => EntityKind::Elevation {
            p: m.apply(*p),
            z: *z,
            angle: angle + m.rotation(),
            override_text: override_text.clone(),
        },
        EntityKind::Wall {
            a,
            b,
            thickness,
            height,
            material,
            hatch,
            align,
        } => EntityKind::Wall {
            a: m.apply(*a),
            b: m.apply(*b),
            thickness: thickness * f,
            height: *height,
            material: material.clone(),
            hatch: *hatch,
            align: *align,
        },
        EntityKind::Door {
            center,
            angle,
            align,
            width,
            kind,
            hinge,
            swing_in,
            host,
            at,
        } => EntityKind::Door {
            align: *align,
            center: m.apply(*center),
            angle: angle + m.rotation(),
            width: width * f,
            kind: *kind,
            hinge: *hinge,
            swing_in: *swing_in,
            host: *host,
            at: *at,
        },
        EntityKind::Window {
            center,
            angle,
            align,
            width,
            sill,
            height,
            host,
            at,
        } => EntityKind::Window {
            align: *align,
            center: m.apply(*center),
            angle: angle + m.rotation(),
            width: width * f,
            sill: *sill,
            height: *height,
            host: *host,
            at: *at,
        },
        EntityKind::Stair {
            a,
            b,
            width,
            steps,
            direction,
        } => EntityKind::Stair {
            a: m.apply(*a),
            b: m.apply(*b),
            width: width * f,
            steps: *steps,
            direction: *direction,
        },
        EntityKind::BlockRef {
            block,
            pos,
            rot,
            scale,
            mirror,
        } => EntityKind::BlockRef {
            block: block.clone(),
            pos: m.apply(*pos),
            rot: rot + m.rotation(),
            scale: v(scale.x * f, scale.y * f),
            mirror: *mirror ^ m.is_mirrored(),
        },
    }
}

/// Hilfsabbildungen
pub fn move_by(d: Vec2) -> Affine {
    Affine::translate(d)
}

pub fn rotate_about(c: Vec2, angle: f64) -> Affine {
    Affine::rotate_about(c, angle)
}

pub fn mirror_at(p1: Vec2, p2: Vec2) -> Affine {
    Affine::mirror(p1, p2)
}

pub fn scale_about(c: Vec2, s: f64) -> Affine {
    Affine::scale_about(c, s, s)
}
