//! DXF-Export (ASCII, AC1009/R12 — von AutoCAD, BricsCAD, LibreCAD lesbar).

use crate::geom::*;
use crate::model::*;
use crate::render;
use crate::style::{Color, LineType};
use std::fmt::Write as _;
use std::path::Path;

fn aci_from_color(c: Color) -> u8 {
    // Grobe Zuordnung zu den AutoCAD-Standardfarben
    const TABLE: &[(Color, u8)] = &[
        (Color::rgb(255, 0, 0), 1),
        (Color::rgb(255, 255, 0), 2),
        (Color::rgb(0, 255, 0), 3),
        (Color::rgb(0, 255, 255), 4),
        (Color::rgb(0, 0, 255), 5),
        (Color::rgb(255, 0, 255), 6),
        (Color::rgb(255, 255, 255), 7),
        (Color::rgb(128, 128, 128), 8),
        (Color::rgb(192, 192, 192), 9),
        (Color::rgb(200, 60, 55), 1),
        (Color::rgb(45, 105, 190), 5),
        (Color::rgb(60, 140, 95), 3),
        (Color::rgb(214, 138, 45), 30),
        (Color::rgb(130, 90, 170), 200),
        (Color::rgb(50, 150, 160), 4),
    ];
    let mut best = 7u8;
    let mut bestd = f64::INFINITY;
    let lum = c.luminance();
    for (col, v) in TABLE {
        let d = ((col.r as f64 - c.r as f64).powi(2)
            + (col.g as f64 - c.g as f64).powi(2)
            + (col.b as f64 - c.b as f64).powi(2))
        .sqrt();
        if d < bestd {
            bestd = d;
            best = *v;
        }
    }
    if lum > 0.9 {
        7
    } else {
        best
    }
}

fn ltype_name(lt: LineType) -> &'static str {
    match lt {
        LineType::Solid => "CONTINUOUS",
        LineType::Dashed => "DASHED",
        LineType::Dotted => "DOT",
        LineType::DashDot => "DASHDOT",
        LineType::Hidden => "HIDDEN",
        // Die Dämmung ist kein Strichmuster, sondern gezeichnete Geometrie
        // (Schlaufenband). Sie geht als POLYLINE mit durchgezogener Linie
        // hinaus — siehe `write_entity`.
        LineType::Insulation => "CONTINUOUS",
    }
}

struct W {
    s: String,
}

impl W {
    fn new() -> Self {
        Self {
            s: String::with_capacity(64 * 1024),
        }
    }
    fn g(&mut self, code: i32, val: impl std::fmt::Display) {
        let _ = writeln!(self.s, "{code}");
        let _ = writeln!(self.s, "{val}");
    }
    fn f(&mut self, code: i32, val: f64) {
        let v = if val.abs() < 1e-9 { 0.0 } else { val };
        self.g(code, format!("{v:.9}"));
    }
    fn p(&mut self, base: i32, p: Vec2, z: f64) {
        self.f(base, p.x);
        self.f(base + 10, p.y);
        self.f(base + 20, z);
    }
}

fn layer_name(doc: &Document, id: Id) -> String {
    doc.layer(id)
        .map(|l| l.name.clone())
        .unwrap_or_else(|| "0".into())
}

fn write_polyline(out: &mut W, layer: &str, verts: &[Vertex], closed: bool, color: Option<u8>) {
    out.g(0, "POLYLINE");
    out.g(8, layer);
    if let Some(c) = color {
        out.g(62, c);
    }
    out.g(66, 1);
    out.g(70, if closed { 1 } else { 0 });
    out.p(10, Vec2::ZERO, 0.0);
    for v in verts {
        out.g(0, "VERTEX");
        out.g(8, layer);
        out.p(10, v.p, 0.0);
        if v.bulge.abs() > 1e-12 {
            out.f(42, v.bulge);
        }
    }
    out.g(0, "SEQEND");
    out.g(8, layer);
}

/// Exportiert die Modellzeichnung als DXF.
pub fn export_dxf(doc: &Document, path: &Path) -> Result<(), String> {
    let mut out = W::new();

    // ---------------- HEADER ----------------
    out.g(0, "SECTION");
    out.g(2, "HEADER");
    out.g(9, "$ACADVER");
    out.g(1, "AC1009");
    out.g(9, "$INSUNITS");
    out.g(70, 4); // Millimeter
    out.g(9, "$EXTMIN");
    out.p(10, doc.bbox().min, 0.0);
    out.g(9, "$EXTMAX");
    out.p(10, doc.bbox().max, 0.0);
    out.g(0, "ENDSEC");

    // ---------------- TABLES ----------------
    out.g(0, "SECTION");
    out.g(2, "TABLES");

    // Layer
    out.g(0, "TABLE");
    out.g(2, "LAYER");
    out.g(70, doc.layers.len());
    for l in &doc.layers {
        let aci = aci_from_color(l.color) as i16;
        out.g(0, "LAYER");
        out.g(2, &l.name);
        out.g(70, if l.locked { 4 } else { 0 });
        out.g(62, if l.visible { aci } else { -aci });
        out.g(6, ltype_name(l.line_type));
    }
    out.g(0, "ENDTAB");

    // Linientypen
    out.g(0, "TABLE");
    out.g(2, "LTYPE");
    out.g(70, 5);
    for (name, desc, pattern) in [
        ("CONTINUOUS", "Solid line", vec![]),
        ("DASHED", "Dashed __ __", vec![4.0, 2.0]),
        ("DOT", "Dot . . .", vec![0.4, 1.4]),
        ("DASHDOT", "Dash dot __ . __", vec![6.0, 1.6, 0.4, 1.6]),
        ("HIDDEN", "Hidden __ __ __", vec![2.0, 1.2]),
    ] {
        out.g(0, "LTYPE");
        out.g(2, &name);
        out.g(70, 0);
        out.g(3, desc);
        out.g(72, 65);
        out.g(73, pattern.len());
        out.f(40, pattern.iter().sum::<f64>());
        for p in pattern {
            out.f(49, p);
        }
    }
    out.g(0, "ENDTAB");
    out.g(0, "ENDSEC");

    // ---------------- BLOCKS ----------------
    out.g(0, "SECTION");
    out.g(2, "BLOCKS");
    for b in &doc.blocks {
        let name = sanitize_block_name(&b.name);
        out.g(0, "BLOCK");
        out.g(8, "0");
        out.g(2, &name);
        out.g(70, 0);
        out.p(10, b.base, 0.0);
        out.g(3, &name);
        out.g(1, "");
        for e in &b.entities {
            write_entity(&mut out, doc, e, Some(name.as_str()));
        }
        out.g(0, "ENDBLK");
        out.g(8, "0");
    }
    out.g(0, "ENDSEC");

    // ---------------- ENTITIES ----------------
    out.g(0, "SECTION");
    out.g(2, "ENTITIES");
    for e in &doc.entities {
        write_entity(&mut out, doc, e, None);
    }
    out.g(0, "ENDSEC");
    out.g(0, "EOF");

    std::fs::write(path, out.s).map_err(|e| format!("DXF schreiben fehlgeschlagen: {e}"))
}

pub fn sanitize_block_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "BLOCK".into()
    } else {
        s
    }
}

fn write_entity(out: &mut W, doc: &Document, e: &Entity, block_ctx: Option<&str>) {
    let layer = if block_ctx.is_some() {
        layer_name(doc, e.layer)
    } else {
        layer_name(doc, e.layer)
    };
    let color = e.style.color.map(aci_from_color);
    let lt = doc.effective_line_type(e);
    // Dämmung: das Schlaufenband ist gezeichnete Geometrie, kein Strichmuster.
    // Solche Objekte gehen über die Anzeige (Band als POLYLINE) statt über das
    // rohe DXF-Element — sonst käme in jedem anderen CAD nur eine Gerade an.
    if lt == LineType::Insulation {
        write_via_display_list(out, doc, e, &layer);
        return;
    }
    match &e.kind {
        EntityKind::Line { a, b } => {
            out.g(0, "LINE");
            out.g(8, &layer);
            if let Some(c) = color {
                out.g(62, c);
            }
            out.g(6, ltype_name(lt));
            out.p(10, *a, 0.0);
            out.p(11, *b, 0.0);
        }
        EntityKind::Circle { c, r } => {
            out.g(0, "CIRCLE");
            out.g(8, &layer);
            if let Some(cc) = color {
                out.g(62, cc);
            }
            out.g(6, ltype_name(lt));
            out.p(10, *c, 0.0);
            out.f(40, *r);
        }
        EntityKind::Arc { c, r, a0, a1 } => {
            out.g(0, "ARC");
            out.g(8, &layer);
            if let Some(cc) = color {
                out.g(62, cc);
            }
            out.g(6, ltype_name(lt));
            out.p(10, *c, 0.0);
            out.f(40, *r);
            out.f(50, to_deg(norm_angle(*a0)));
            out.f(51, to_deg(norm_angle(*a1)));
        }
        EntityKind::Polyline { verts, closed } => {
            write_polyline(out, &layer, verts, *closed, color);
        }
        // Splines gehen als abgeflachte POLYLINE hinaus: DXF-SPLINE verlangt
        // Kontrollpunkte und Knotenvektoren, die eine Catmull-Rom-Kurve nicht
        // hat. Die Kurve bleibt damit in jedem CAD gleich (Abweichung
        // `SPLINE_TOL_MM` = 0,05 mm), nur die Bearbeitung als Spline entfällt.
        EntityKind::Spline { verts, closed } => {
            let sp = crate::geom::Spline {
                verts: verts.clone(),
                closed: *closed,
            };
            let flach: Vec<Vertex> = sp
                .flatten(crate::geom::SPLINE_TOL_MM)
                .into_iter()
                .map(Vertex::new)
                .collect();
            write_polyline(out, &layer, &flach, *closed, color);
        }
        EntityKind::Point { p } => {
            out.g(0, "POINT");
            out.g(8, &layer);
            out.p(10, *p, 0.0);
        }
        EntityKind::Text {
            p,
            text,
            height,
            angle,
            align,
            ..
        } => {
            out.g(0, "TEXT");
            out.g(8, &layer);
            if let Some(c) = color {
                out.g(62, c);
            }
            out.p(10, *p, 0.0);
            out.f(40, *height);
            out.g(1, text.replace('\n', " "));
            out.f(50, to_deg(*angle));
            out.g(7, "STANDARD");
            let h = match align {
                TextAlignH::Left => 0,
                TextAlignH::Center => 1,
                TextAlignH::Right => 2,
            };
            out.g(72, h);
            if h != 0 {
                out.p(11, *p, 0.0);
            }
        }
        EntityKind::BlockRef {
            block,
            pos,
            rot,
            scale,
            mirror,
        } => {
            let Some(def) = doc.block(block) else { return };
            out.g(0, "INSERT");
            out.g(8, &layer);
            out.g(2, sanitize_block_name(&def.name));
            out.p(10, *pos, 0.0);
            out.f(41, if *mirror { -scale.x } else { scale.x });
            out.f(42, scale.y);
            out.f(50, to_deg(*rot));
        }
        // Architektur, Bemaßung, Schraffur: über die Anzeigegeometrie exportieren
        _ => write_via_display_list(out, doc, e, &layer),
    }
}

/// Objekt über die Anzeigeliste exportieren (DXF beschreibt die Zeichnung,
/// nie den Papierraum).
fn write_via_display_list(out: &mut W, doc: &Document, e: &Entity, layer: &str) {
    let ctx = render::RenderCtx::model(doc);
    let mut dl = render::DisplayList::default();
    render::render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    for prim in dl.prims {
        write_shape(out, layer, &prim);
    }
}

fn write_shape(out: &mut W, layer: &str, prim: &render::Prim) {
    let color = Some(aci_from_color(prim.style.color));
    match &prim.shape {
        render::Shape::Line(a, b) => {
            out.g(0, "LINE");
            out.g(8, layer);
            if let Some(c) = color {
                out.g(62, c);
            }
            out.p(10, *a, 0.0);
            out.p(11, *b, 0.0);
        }
        render::Shape::Poly(pts, closed) => {
            let verts: Vec<Vertex> = pts.iter().map(|p| Vertex::new(*p)).collect();
            write_polyline(out, layer, &verts, *closed, color);
        }
        render::Shape::Circle { c, r } => {
            out.g(0, "CIRCLE");
            out.g(8, layer);
            out.p(10, *c, 0.0);
            out.f(40, *r);
        }
        render::Shape::Arc { c, r, a0, a1 } => {
            out.g(0, "ARC");
            out.g(8, layer);
            out.p(10, *c, 0.0);
            out.f(40, *r);
            out.f(50, to_deg(norm_angle(*a0)));
            out.f(51, to_deg(norm_angle(*a1)));
        }
        render::Shape::Ellipse { c, rx, ry, rot } => {
            out.g(0, "ELLIPSE");
            out.g(8, layer);
            out.p(10, *c, 0.0);
            let major = Vec2::polar(*rot, *rx);
            out.p(11, maybe_major(major), 0.0);
            out.f(40, if *rx > 1e-9 { rx / ry } else { 1.0 });
            out.f(41, 0.0);
            out.f(42, std::f64::consts::TAU);
        }
        // Ellipsenbogen: DXF-ELLIPSE mit Start-/Endparameter (40 = Verhältnis
        // Haupt-/Nebenachse, 41/42 = Parameter in Grad).
        render::Shape::EllipseArc {
            c,
            rx,
            ry,
            rot,
            a0,
            a1,
        } => {
            out.g(0, "ELLIPSE");
            out.g(8, layer);
            out.p(10, *c, 0.0);
            let major = Vec2::polar(*rot, *rx);
            out.p(11, maybe_major(major), 0.0);
            out.f(40, if *rx > 1e-9 { rx / ry } else { 1.0 });
            out.f(41, to_deg(norm_angle(*a0)));
            out.f(42, to_deg(norm_angle(*a1)));
        }
        // DXF kennt kein SOLID mit Aussparungen; hier steht deshalb nur der
        // Außenring. Bildschirm, PDF und PNG zeigen die Löcher (Even-Odd).
        render::Shape::Fill { outer, .. } => {
            let pts = outer;
            if pts.len() >= 3 {
                out.g(0, "SOLID");
                out.g(8, layer);
                if let Some(c) = color {
                    out.g(62, c);
                }
                out.p(10, pts[0], 0.0);
                out.p(11, pts[1], 0.0);
                out.p(12, pts[2], 0.0);
                if pts.len() >= 4 {
                    out.p(13, pts[3], 0.0);
                } else {
                    out.p(13, pts[2], 0.0);
                }
            }
        }
        // Ein Rasterbild kennt DXF hier nicht; exportiert wird der Rahmen, damit
        // die belegte Fläche in der Fremdzeichnung sichtbar bleibt.
        render::Shape::Image { a, b, hoehe, .. } => {
            let ecken = crate::bild::ecken(*a, *b, *hoehe);
            let verts: Vec<Vertex> = ecken.iter().map(|p| Vertex::new(*p)).collect();
            write_polyline(out, layer, &verts, true, color);
        }
        render::Shape::Dot { p, .. } => {
            out.g(0, "POINT");
            out.g(8, layer);
            out.p(10, *p, 0.0);
        }
        render::Shape::Text(t) => {
            out.g(0, "TEXT");
            out.g(8, layer);
            out.p(10, t.pos, 0.0);
            out.f(40, t.height);
            out.g(1, t.text.replace('\n', " "));
            out.f(50, to_deg(t.angle));
            out.g(7, "STANDARD");
            let h = match t.align {
                TextAlignH::Left => 0,
                TextAlignH::Center => 1,
                TextAlignH::Right => 2,
            };
            out.g(72, h);
            if h != 0 {
                out.p(11, t.pos, 0.0);
            }
        }
    }
}

fn maybe_major(m: Vec2) -> Vec2 {
    m
}
