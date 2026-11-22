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
        // Ansichtsfenster: das Rechteck folgt der Abbildung (Verschieben,
        // Drehen, Spiegeln) und bleibt achsenparallel — die Bounding-Box der
        // abgebildeten Ecken. Maßstab und Modellmitte bleiben stehen.
        EntityKind::Viewport(vp) => {
            let a = m.apply(v(vp.x, vp.y));
            let b = m.apply(v(vp.x + vp.w, vp.y + vp.h));
            let mut neu = vp.clone();
            neu.x = a.x.min(b.x);
            neu.y = a.y.min(b.y);
            neu.w = (a.x - b.x).abs().max(1.0);
            neu.h = (a.y - b.y).abs().max(1.0);
            EntityKind::Viewport(neu)
        }
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
        // Ellipsenbogen wie der Bogen: Parameter laufen im ungedrehten
        // Ellipsenraum, gespiegelt kehren sie sich um (u → −u).
        EntityKind::EllipseArc {
            c,
            rx,
            ry,
            rot,
            a0,
            a1,
        } => {
            let (aa, ab, r) = if m.is_mirrored() {
                (-*a1, -*a0, m.rotation() - *rot)
            } else {
                (*a0, *a1, *rot + m.rotation())
            };
            EntityKind::EllipseArc {
                c: m.apply(*c),
                rx: rx * sx(m),
                ry: ry * sy(m),
                rot: r,
                a0: aa,
                a1: ab,
            }
        }
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
        EntityKind::Spline { verts, closed } => EntityKind::Spline {
            verts: verts
                .iter()
                .map(|v| Vertex {
                    p: m.apply(v.p),
                    bulge: 0.0,
                })
                .collect(),
            closed: *closed,
        },
        EntityKind::Fill {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
        } => EntityKind::Fill {
            boundary: boundary.iter().map(|p| m.apply(*p)).collect(),
            // Inseln wandern mit: verschieben, drehen und spiegeln wirkt auf
            // Außenring und Locheringe gleich.
            holes: holes
                .iter()
                .map(|h| h.iter().map(|p| m.apply(*p)).collect())
                .collect(),
            pattern: *pattern,
            pattern_angle: pattern_angle + m.rotation(),
            pattern_scale: pattern_scale * f,
            border: *border,
        },
        // Volumen wandert wie eine Füllfläche mit; die Höhe ist ein Maß in mm
        // und bleibt beim Verschieben/Drehen unverändert.
        EntityKind::Volume {
            boundary,
            holes,
            pattern,
            pattern_angle,
            pattern_scale,
            border,
            height,
        } => EntityKind::Volume {
            boundary: boundary.iter().map(|p| m.apply(*p)).collect(),
            holes: holes
                .iter()
                .map(|h| h.iter().map(|p| m.apply(*p)).collect())
                .collect(),
            pattern: *pattern,
            pattern_angle: *pattern_angle,
            pattern_scale: *pattern_scale,
            border: *border,
            height: *height,
        },
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
            line_spacing,
            frame,
            frame_round,
            background,
        } => EntityKind::Text {
            p: m.apply(*p),
            text: text.clone(),
            height: height * f,
            angle: angle + m.rotation(),
            align: *align,
            valign: *valign,
            bold: *bold,
            italic: *italic,
            underline: *underline,
            superscript: *superscript,
            runs: runs.clone(),
            font: *font,
            line_spacing: *line_spacing,
            frame: *frame,
            frame_round: *frame_round,
            background: *background,
        },
        // Raumstempel: Umgrenzung und Textfeld wandern mit, der Text bleibt
        // abgeleitet (Fläche und Beschriftung stimmen danach von selbst).
        EntityKind::Room {
            boundary,
            label,
            height,
            heading,
        } => EntityKind::Room {
            boundary: boundary.iter().map(|p| m.apply(*p)).collect(),
            label: m.apply(*label),
            height: height * f,
            heading: heading.clone(),
        },
        // Bemaßung: Messpunkte und Maßlinie wandern mit, und die
        // **Orientierung** dreht mit (T0166). Ohne das blieben beim Drehen die
        // Maßlinie waagerecht und nur die Messpunkte wanderten — die Kette sah
        // danach aus wie vorher.
        EntityKind::Dim {
            pts,
            line_pos,
            angle,
            override_text,
            ext_lines,
        } => {
            // Messpunkte in Messreihenfolge halten (T0195): Ziehen, Drehen,
            // Spiegeln und Einfügen verschieben Punkte über ihre Nachbarn
            // hinweg — die Maßzahlen hängen an der gespeicherten Reihenfolge.
            let mut kind = EntityKind::Dim {
                pts: pts.iter().map(|p| m.apply(*p)).collect(),
                line_pos: m.apply(*line_pos),
                // Gespiegelt kehrt sich die Richtung: bei einer Spiegelung an der
                // Geraden mit Winkel θ ist `rotation()` = 2θ, die gespiegelte
                // Richtung also 2θ − φ (T0166).
                angle: if m.is_mirrored() {
                    m.rotation() - *angle
                } else {
                    *angle + m.rotation()
                },
                override_text: override_text.clone(),
                ext_lines: *ext_lines,
            };
            crate::render::dim_reihenfolge(&mut kind);
            kind
        }
        EntityKind::ElevationPlan { p, z, text_pos } => EntityKind::ElevationPlan {
            p: m.apply(*p),
            z: *z,
            text_pos: *text_pos,
        },
        // Die Höhen der Schnitt-Kote hängen an der Y-Differenz zum ersten
        // Punkt. Nach Drehen/Spiegeln rechnen sie sich deshalb neu — das ist
        // gewollt: die Kote beschreibt die Lage in der Zeichnung, nicht einen
        // gespeicherten Höhenwert.
        EntityKind::ElevationSection { pts } => EntityKind::ElevationSection {
            pts: pts.iter().map(|p| m.apply(*p)).collect(),
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
            base,
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
            base: *base,
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
            height,
            auto_steps,
            cut,
            unten,
            oben,
        } => EntityKind::Stair {
            a: m.apply(*a),
            b: m.apply(*b),
            width: width * f,
            steps: *steps,
            // Die Höhe ist eine Eigenschaft der Treppe, kein Maß des Grundrisses:
            // sie bleibt beim Skalieren/Verschieben unverändert.
            height: *height,
            auto_steps: *auto_steps,
            cut: *cut,
            unten: unten.clone(),
            oben: oben.clone(),
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
        // Bild: die untere Kante wandert mit (Verschieben, Drehen, Spiegeln,
        // Skalieren); die Höhe skaliert mit dem Faktor.
        EntityKind::Image {
            a,
            b,
            height,
            lock_ratio,
            name,
            data,
        } => EntityKind::Image {
            a: m.apply(*a),
            b: m.apply(*b),
            height: height * f,
            lock_ratio: *lock_ratio,
            name: name.clone(),
            data: data.clone(),
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

/// Mindestabstand eines Winkelpunktes zum Drehzentrum (mm). Liegt ein Punkt
/// darauf, ist seine Richtung nicht bestimmt.
pub const ANGLE_MIN_RADIUS: f64 = 0.5;

/// Drehwinkel in Grad (mathematisch positiv) zwischen den vom Drehzentrum aus
/// gesehenen Richtungen `from` und `to` — die Rechnung der Zwei-Klick-Winkelwahl.
///
/// `None`, wenn einer der Punkte auf dem Zentrum liegt; der Winkel ist dann
/// nicht bestimmt und es darf nicht gedreht werden.
pub fn angle_between_directions(center: Vec2, from: Vec2, to: Vec2) -> Option<f64> {
    let a = from - center;
    let b = to - center;
    if a.len() < ANGLE_MIN_RADIUS || b.len() < ANGLE_MIN_RADIUS {
        return None;
    }
    Some(a.signed_angle_to(b).to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winkel_zwischen_richtungen_mit_vorzeichen() {
        let c = v(0., 0.);
        // von +X nach +Y: +90°
        let a = angle_between_directions(c, v(1., 0.), v(0., 1.)).unwrap();
        assert!((a - 90.0).abs() < 1e-9, "{a}");
        // von +X nach -Y: -90°
        let a = angle_between_directions(c, v(1., 0.), v(0., -1.)).unwrap();
        assert!((a + 90.0).abs() < 1e-9, "{a}");
        // von +Y nach +X: -90° (Gegenprobe zur ersten Zeile)
        let a = angle_between_directions(c, v(0., 1.), v(1., 0.)).unwrap();
        assert!((a + 90.0).abs() < 1e-9, "{a}");
        // 45°-Richtung nach +Y: +45°
        let a = angle_between_directions(c, v(1., 1.), v(0., 1.)).unwrap();
        assert!((a - 45.0).abs() < 1e-9, "{a}");
        // Richtungsumkehr: 180°, nicht 0°
        let a = angle_between_directions(c, v(1., 0.), v(-1., 0.)).unwrap();
        assert!((a.abs() - 180.0).abs() < 1e-9, "{a}");
    }

    #[test]
    fn winkel_um_ein_versetztes_zentrum() {
        // Richtungen zählen ab dem Zentrum, nicht ab dem Ursprung
        let c = v(1000., 1000.);
        let a = angle_between_directions(c, v(2000., 1000.), v(1000., 2000.)).unwrap();
        assert!((a - 90.0).abs() < 1e-9, "{a}");
    }

    #[test]
    fn winkel_auf_dem_zentrum_ist_unbestimmt() {
        let c = v(500., 500.);
        assert!(angle_between_directions(c, c, v(0., 0.)).is_none());
        assert!(angle_between_directions(c, v(0., 0.), c).is_none());
        // knapp daneben (0,4 mm) gilt ebenfalls als unbestimmt
        assert!(angle_between_directions(c, v(500.4, 500.), v(600., 500.)).is_none());
    }
}

pub fn mirror_at(p1: Vec2, p2: Vec2) -> Affine {
    Affine::mirror(p1, p2)
}

pub fn scale_about(c: Vec2, s: f64) -> Affine {
    Affine::scale_about(c, s, s)
}
