//! Zeichnungsfläche: Anzeige, Fang, Werkzeugbedienung, Navigation.

use crate::app::{screen_to_world, world_to_screen, App, TabKind, Tool, ToolPhase, View};
use crate::theme::Palette;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::geom::*;
use jjcad_core::geom::Vec2 as V2;
use jjcad_core::style::HatchPattern;
use jjcad_core::model::*;
use jjcad_core::render::{self, DisplayList, Prim, Shape as PShape};
use jjcad_core::snap;
use jjcad_core::units::{format_number, format_scale};
use serde_json::json;

const PAPER_PX_PER_MM: f64 = 3.2;

// ---------------------------------------------------------------------------
// Anzeige
// ---------------------------------------------------------------------------

struct PaintCtx {
    selected: Vec<Id>,
    hover: Option<Id>,
    lineweights: bool,
    /// Linienstärke maßstabsgetreu (über den Maßstab skaliert) statt bildschirmkonstant
    lineweights_scaled: bool,
    /// Pixel pro Papier-mm (für Strichstärken und Linientypen)
    paper_px: f64,
}

fn color32(c: jjcad_core::style::Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Strichmuster in Bildschirmpixeln.
/// `scaled` = maßstabsgetreu (DPI-Modus): das Muster wächst mit dem Zoom wie im PDF.
/// Sonst bildschirmkonstant (px-Modus) mit demselben Faktor wie die Strichstärken.
/// Wandausrichtung als Aktionsname
fn align_name(a: jjcad_core::style::WallAlign) -> &'static str {
    match a {
        jjcad_core::style::WallAlign::Left => "left",
        jjcad_core::style::WallAlign::Center => "center",
        jjcad_core::style::WallAlign::Right => "right",
    }
}

/// Mittelpunkt einer Öffnung im Wandkörper (berücksichtigt die Wandausrichtung).
fn opening_center(app: &App, host: jjcad_core::model::Id, p: V2, ang: f64) -> V2 {
    if let Some(jjcad_core::model::EntityKind::Wall { thickness, align, .. }) =
        app.doc().entity(host).map(|e| &e.kind)
    {
        let n = V2::polar(ang + std::f64::consts::FRAC_PI_2, 1.0);
        return p + n * (thickness * 0.5 * align.offset());
    }
    p
}

/// Nächste Wand unter dem Punkt: liefert (Punkt auf der Achse, Wandwinkel, Wand-Id).
fn wall_under(app: &App, p: V2) -> Option<(V2, f64, jjcad_core::model::Id)> {
    let mut best: Option<(f64, V2, f64, jjcad_core::model::Id)> = None;
    for e in &app.doc().entities {
        if let jjcad_core::model::EntityKind::Wall { a, b, thickness, .. } = &e.kind {
            let d = jjcad_core::geom::dist_point_segment(p, *a, *b);
            if d > thickness * 0.5 + 600.0 {
                continue;
            }
            let dir = (*b - *a).normalized();
            let t = (p - *a).dot(dir).clamp(0.0, a.dist(*b));
            let on_axis = *a + dir * t;
            if best.as_ref().map(|(bd, _, _, _)| d < *bd).unwrap_or(true) {
                best = Some((d, on_axis, dir.angle(), e.id));
            }
        }
    }
    best.map(|(_, p, a, id)| (p, a, id))
}

pub(crate) fn dash_pattern(
    lt: jjcad_core::style::LineType,
    paper_px: f64,
    scaled: bool,
) -> Option<Vec<f32>> {
    let p = lt.pattern();
    if p.len() < 2 {
        return None;
    }
    let k = if scaled { paper_px } else { 3.2 };
    Some(p.iter().map(|v| (*v * k).max(0.8) as f32).collect())
}

pub(crate) fn dashed_segments(a: Pos2, b: Pos2, pattern: &[f32]) -> Vec<[Pos2; 2]> {
    let mut out = Vec::new();
    let total: f32 = pattern.iter().sum();
    if total <= 0.0 {
        return vec![[a, b]];
    }
    let dir = b - a;
    let len = dir.length();
    if len < 0.5 {
        return vec![[a, b]];
    }
    let u = dir / len;
    let mut t = 0.0f32;
    let mut idx = 0usize;
    while t < len {
        let seg = pattern[idx % pattern.len()];
        let t1 = (t + seg).min(len);
        if idx % 2 == 0 {
            out.push([a + u * t, a + u * t1]);
        }
        t = t1;
        idx += 1;
    }
    out
}

/// Kleines Mittelpunktkreuz für Kreise, Bögen und Ellipsen.
fn center_mark(p: &Painter, c: Pos2, col: Color32) {
    let r = 4.0;
    let s = Stroke::new(1.0, col.gamma_multiply(0.85));
    p.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], s);
    p.line_segment([Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)], s);
}

/// Strich mit runden Enden (Kapsel): Rechteck plus zwei Halbkreise.
/// Dünne Striche bleiben einfache Linien — das ist schneller und optisch gleich.
fn capsule(p: &Painter, a: Pos2, b: Pos2, w: f32, col: Color32) {
    if w < 2.0 {
        p.line_segment([a, b], Stroke::new(w.max(0.4), col));
        return;
    }
    let r = w * 0.5;
    let d = b - a;
    let len = d.length();
    if len < 0.01 {
        p.circle_filled(a, r, col);
        return;
    }
    let u = d / len;
    let n = egui::vec2(-u.y, u.x);
    // Halbkreise als Teil EINES Umrisses: dadurch bekommt die Kapsel genau eine
    // Kantenglättung. Drei getrennte Formen (Rechteck + zwei Kreise) erzeugen am
    // Übergang einen zusätzlichen halben Pixel.
    let seg = if r > 24.0 {
        18
    } else if r > 10.0 {
        12
    } else {
        8
    };
    let mut pts: Vec<Pos2> = Vec::with_capacity(2 * seg + 2);
    for i in 0..=seg {
        let t = std::f32::consts::PI * (i as f32) / (seg as f32);
        pts.push(b + (n * t.cos() + u * t.sin()) * r);
    }
    for i in 0..=seg {
        let t = std::f32::consts::PI * (i as f32) / (seg as f32);
        pts.push(a + (-n * t.cos() - u * t.sin()) * r);
    }
    p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
}

/// Polylinie aus Kapseln — dadurch sind auch die Übergänge rund.
fn stroke_pts(p: &Painter, pts: &[Pos2], w: f32, col: Color32) {
    if pts.len() == 1 {
        p.circle_filled(pts[0], (w * 0.5).max(0.4), col);
        return;
    }
    for pair in pts.windows(2) {
        capsule(p, pair[0], pair[1], w, col);
    }
}

fn paint_prim(p: &Painter, prim: &Prim, w2s: &dyn Fn(V2) -> Pos2, scale: f64, pc: &PaintCtx, alpha: f32) {
    let is_sel = pc.selected.contains(&prim.entity) && prim.entity != 0;
    let is_hover = pc.hover == Some(prim.entity) && prim.entity != 0 && !is_sel;
    let mut col = color32(prim.style.color);
    if is_sel {
        col = Palette::SELECT;
    } else if is_hover {
        col = Palette::HOVER;
    }
    let col = col.gamma_multiply(alpha);
    // Zwei Modi:
    //  "px"  = bildschirmkonstant (Breite in Pixeln, unabhängig vom Zoom)
    //  "DPI" = wie im PDF: Breite_mm * Pixel-pro-Papier-mm, ohne Obergrenze,
    //          damit Strichbreite und Abstand in jedem Zoom im selben Verhältnis stehen.
    let base_w = if pc.lineweights_scaled {
        (prim.style.width * pc.paper_px).max(0.05)
    } else {
        (prim.style.width * PAPER_PX_PER_MM).clamp(0.7, 9.0)
    };
    let _ = pc.lineweights;
    let w = if is_sel || is_hover { base_w.max(1.6) + 0.6 } else { base_w };
    let stroke = Stroke::new(w as f32, col);
    let dash = dash_pattern(prim.style.line_type, pc.paper_px, pc.lineweights_scaled);

    match &prim.shape {
        PShape::Line(a, b) => {
            let (pa, pb) = (w2s(*a), w2s(*b));
            match &dash {
                Some(pat) if !is_sel => {
                    for s in dashed_segments(pa, pb, pat) {
                        capsule(p, s[0], s[1], w as f32, col);
                    }
                }
                _ => {
                    capsule(p, pa, pb, w as f32, col);
                }
            }
        }
        PShape::Poly(pts, closed) => {
            if pts.len() < 2 {
                return;
            }
            let mut v: Vec<Pos2> = pts.iter().map(|q| w2s(*q)).collect();
            if *closed {
                v.push(v[0]);
            }
            if let Some(pat) = &dash {
                if !is_sel {
                    for pair in v.windows(2) {
                        for s in dashed_segments(pair[0], pair[1], pat) {
                            capsule(p, s[0], s[1], w as f32, col);
                        }
                    }
                    return;
                }
            }
            stroke_pts(p, &v, w as f32, col);
        }
        PShape::Fill(pts) => {
            if pts.len() < 3 {
                return;
            }
            let v: Vec<Pos2> = pts.iter().map(|q| w2s(*q)).collect();
            let fill = prim.style.fill.unwrap_or(prim.style.color);
            let c = if is_sel {
                Palette::SELECT.gamma_multiply(0.25)
            } else if is_hover {
                Palette::HOVER.gamma_multiply(0.2)
            } else {
                color32(fill)
            };
            for tri in triangulate(&v) {
                p.add(Shape::convex_polygon(
                    tri.to_vec(),
                    c.gamma_multiply(alpha),
                    Stroke::NONE,
                ));
            }
        }
        PShape::Circle { c, r } => {
            p.circle_stroke(w2s(*c), (*r * scale) as f32, stroke);
            center_mark(p, w2s(*c), col);
        }
        PShape::Arc { c, r, a0, a1 } => {
            let arc = Arc::new(*c, *r, *a0, *a1);
            let pts: Vec<Pos2> = arc
                .flatten(0.6 / scale.max(1e-6))
                .iter()
                .map(|q| w2s(*q))
                .collect();
            stroke_pts(p, &pts, w as f32, col);
            center_mark(p, w2s(*c), col.gamma_multiply(0.8));
        }
        PShape::Ellipse { c, rx, ry, rot } => {
            let pts: Vec<Pos2> = (0..=72)
                .map(|i| {
                    let t = i as f64 / 72.0 * std::f64::consts::TAU;
                    w2s(*c + V2::new(rx * t.cos(), ry * t.sin()).rotated(*rot))
                })
                .collect();
            stroke_pts(p, &pts, w as f32, col);
            center_mark(p, w2s(*c), col);
        }
        PShape::Dot { p: pos, r } => {
            p.circle_filled(w2s(*pos), ((*r * scale) as f32).max(1.2), col);
        }
        PShape::Text(t) => {
            let size = (t.height * scale * 1.32).max(3.0) as f32;
            if size < 3.0 {
                return;
            }
            let font = FontId::proportional(size);
            let galley = p.layout_no_wrap(t.text.clone(), font, col);
            let sz = galley.size();
            let (dx, dy) = match t.align {
                TextAlignH::Left => (0.0, 0.0),
                TextAlignH::Center => (-sz.x / 2.0, 0.0),
                TextAlignH::Right => (-sz.x, 0.0),
            };
            let (vx, vy) = match t.valign {
                TextAlignV::Top => (0.0, 0.0),
                TextAlignV::Middle => (0.0, -sz.y / 2.0),
                TextAlignV::Bottom => (0.0, -sz.y),
            };
            let base = w2s(t.pos);
            let pos = Pos2::new(base.x + dx + vx, base.y + dy + vy);
            if t.angle.abs() < 0.001 {
                p.galley(pos, galley, col);
            } else {
                let shape = egui::epaint::TextShape::new(pos, galley, col)
                    .with_angle(t.angle as f32);
                p.add(shape);
            }
        }
    }
}

/// Einfache Ohr-Triangulation für gefüllte Polygone (auch konkav).
fn triangulate(pts: &[Pos2]) -> Vec<Vec<Pos2>> {
    let n = pts.len();
    if n < 3 {
        return vec![];
    }
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::new();
    let area = |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let mut guard = 0;
    while idx.len() > 3 && guard < 1000 {
        guard += 1;
        let mut clipped = false;
        for k in 0..idx.len() {
            let i0 = idx[(k + idx.len() - 1) % idx.len()];
            let i1 = idx[k];
            let i2 = idx[(k + 1) % idx.len()];
            let (a, b, c) = (pts[i0], pts[i1], pts[i2]);
            if area(a, b, c).abs() < 0.01 {
                idx.remove(k);
                clipped = true;
                break;
            }
            if area(a, b, c) < 0.0 {
                continue;
            }
            let mut inside = false;
            for &j in &idx {
                if j == i0 || j == i1 || j == i2 {
                    continue;
                }
                let p = pts[j];
                let inside_tri = area(a, b, p) >= 0.0 && area(b, c, p) >= 0.0 && area(c, a, p) >= 0.0;
                if inside_tri {
                    inside = true;
                    break;
                }
            }
            if inside {
                continue;
            }
            out.push(vec![a, b, c]);
            idx.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            // Abbruch: Rest als Fächer
            for k in 1..idx.len() - 1 {
                out.push(vec![pts[idx[0]], pts[idx[k]], pts[idx[k + 1]]]);
            }
            return out;
        }
    }
    if idx.len() == 3 {
        out.push(vec![pts[idx[0]], pts[idx[1]], pts[idx[2]]]);
    }
    out
}

// ---------------------------------------------------------------------------
// Hauptfunktion
// ---------------------------------------------------------------------------

pub fn show(app: &mut App, ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    let (resp, painter) = ui.allocate_painter(rect.size(), Sense::click_and_drag());
    let rect = resp.rect;
    app.ui.last_canvas = rect;

    // Betriebssystem-Zeiger ausblenden: die Fläche zeichnet ihr eigenes Fadenkreuz.
    // Nicht, solange ein Menü offen ist — das Ausblenden löst ein Zeiger-Ereignis aus,
    // an dem egui das Kontextmenü schließt.
    if resp.hovered() || resp.dragged_by(egui::PointerButton::Primary) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    }
    let view = app.current_view();
    let is_sheet = matches!(app.tab().tab, TabKind::Sheet(_));

    // Hintergrund
    painter.rect_filled(rect, 0.0, Palette::BG);

    // Navigation
    handle_navigation(app, ui, &resp, rect, view);

    // Weltkoordinate unter dem Zeiger
    let pointer = ui.ctx().pointer_hover_pos().filter(|p| rect.contains(*p));
    let raw_world = pointer.map(|p| screen_to_world(rect, app.current_view(), p));

    // Fangen
    let mut snapped = raw_world;
    app.snap_label = None;
    if let Some(mut w) = raw_world {
        if !is_sheet {
            let s = &app.doc().settings.snap;
            let tol = (s.tolerance_px / app.current_view().zoom).max(0.5);
            let reference = app.tool.as_ref().and_then(|t| t.pts.last().copied());
            if s.ortho {
                if let Some(r) = reference {
                    w = w.ortho_from(r, false);
                }
            } else if s.polar {
                if let Some(r) = reference {
                    let d = w.dist(r);
                    if d > tol * 2.0 {
                        if let Some(p) = w.polar_snap_from_tol(
                            r,
                            s.polar_step.max(1.0),
                            s.polar_tolerance,
                        ) {
                            w = p;
                            app.snap_label = Some("POL".to_string());
                        }
                    }
                }
            }
            let ignore: Vec<Id> = app.selection().to_vec();
            if let Some(hit) = snap::find_snap(app.doc(), w, tol, reference, &ignore) {
                w = hit.point;
                app.snap_label = Some(hit.kind.label().to_string());
            }
        }
        snapped = Some(w);
    }
    app.pointer_world = snapped;
    if !is_sheet {
        if let (Some(g), Some(p)) = (app.ghost.as_mut(), snapped) {
            g.pos = p;
        }
    }

    // Zeichnung
    let view_now = app.current_view();
    let selected = app.selection().to_vec();
    let hover = app.tab().hover;
    let lineweights = app.ui.lineweights;
    let lineweights_scaled = app.ui.lineweights_scaled;
    let paper_px = view_now.zoom * app.doc().settings.annotation_scale;

    if is_sheet {
        draw_sheet(app, &painter, rect, view_now, snapped);
    } else {
        let dl = display_list(app);
        let pc = PaintCtx {
            selected: selected.clone(),
            hover,
            lineweights,
            lineweights_scaled,
            paper_px,
        };
        if app.ui.show_grid {
            draw_grid(app, &painter, rect, view_now);
        }
        // Papierfläche
        painter.rect_filled(rect, 0.0, Palette::PAPER);
        if app.ui.show_grid {
            draw_grid(app, &painter, rect, view_now);
        }
        let w2s = |p: V2| world_to_screen(rect, view_now, p);
        for prim in &dl.prims {
            paint_prim(&painter, prim, &w2s, view_now.zoom, &pc, 1.0);
        }
        // Fangmarker + Cursor
        if app.tool.is_some() {
            draw_tool_preview(app, &painter, rect, view_now, snapped);
        }
        draw_grips(app, &painter, rect, view_now, &selected);
        if app.ghost.is_some() {
            draw_ghost(app, &painter, rect, view_now);
        }
        if let Some((start, cur, _)) = app.select_drag {
            let r = Rect::from_two_pos(world_to_screen(rect, view_now, start), world_to_screen(rect, view_now, cur));
            let crossing = cur.x < start.x;
            let fill = if crossing {
                Color32::from_rgba_unmultiplied(0x5A, 0xB5, 0x82, 34)
            } else {
                Color32::from_rgba_unmultiplied(0x2F, 0x8C, 0xF0, 34)
            };
            painter.rect_filled(r, 0.0, fill);
            painter.rect_stroke(r, 0.0, Stroke::new(1.2, if crossing { Palette::OK } else { Palette::SELECT }));
        }
        draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world);
    }

    // Eingabe
    let pointer_pos = ui.ctx().pointer_hover_pos();
    if resp.hovered() {
        if let Some(pos) = pointer_pos {
            // Hover-Highlight
            if app.tool.is_none() && !is_sheet {
                if let Some(w) = snapped {
                    let tol = (8.0 / view_now.zoom).max(0.5);
                    let h = snap::hit_test(app.doc(), w, tol, None);
                    app.tab_mut().hover = h;
                }
            } else {
                app.tab_mut().hover = None;
            }
        }
    }

    handle_input(app, ui, &resp, rect, &painter, is_sheet, snapped, raw_world);

    // Overlay: Blattmaßstab / Zoom
    if is_sheet {
        if let Some(sid) = match app.tab().tab {
            TabKind::Sheet(id) => Some(id),
            _ => None,
        } {
            if let Some(sheet) = app.doc().sheet(sid) {
                painter.text(
                    Pos2::new(rect.left() + 14.0, rect.top() + 12.0),
                    Align2::LEFT_TOP,
                    format!("{} · {} · {}", sheet.name, sheet.paper.label(), format_scale(sheet.scale)),
                    FontId::proportional(11.0),
                    Palette::TEXT_FAINT,
                );
            }
        }
    }
}

fn display_list(app: &mut App) -> DisplayList {
    let annot = app.doc().settings.annotation_scale;
    let rev = app.tab().rev;
    let zoom = app.current_view().zoom;
    let doc = app.doc().clone();
    if let Some((r, z, a, dl)) = &app.tab().cache {
        if *r == rev && (*z - zoom).abs() < 1e-9 && (*a - annot).abs() < 1e-9 {
            return dl.clone();
        }
    }
    let ctx = render::RenderCtx::new(&doc);
    let dl = render::render_document(&ctx);
    app.tab_mut().cache = Some((rev, zoom, annot, dl.clone()));
    dl
}

// ---------------------------------------------------------------------------
// Gitter
// ---------------------------------------------------------------------------

fn draw_grid(app: &App, p: &Painter, rect: Rect, view: View) {
    let step = app.doc().settings.snap.grid_step.max(1.0);
    let mut minor = step;
    // Bei sehr kleinem Raster ausdünnen
    while minor * view.zoom < 7.0 {
        minor *= 10.0;
    }
    let major = minor * 10.0;
    let x0 = view.center.x - (rect.width() as f64 * 0.5) / view.zoom;
    let x1 = view.center.x + (rect.width() as f64 * 0.5) / view.zoom;
    let y0 = view.center.y - (rect.height() as f64 * 0.5) / view.zoom;
    let y1 = view.center.y + (rect.height() as f64 * 0.5) / view.zoom;

    let draw_set = |step: f64, color: Color32, w: f32, p: &Painter| {
        let start = (x0 / step).floor() as i64;
        let end = (x1 / step).ceil() as i64;
        if end - start > 3000 {
            return;
        }
        for i in start..=end {
            let x = i as f64 * step;
            let sx = world_to_screen(rect, view, V2::new(x, 0.0)).x;
            p.line_segment(
                [Pos2::new(sx, rect.top()), Pos2::new(sx, rect.bottom())],
                Stroke::new(w, color),
            );
        }
        let start = (y0 / step).floor() as i64;
        let end = (y1 / step).ceil() as i64;
        for i in start..=end {
            let y = i as f64 * step;
            let sy = world_to_screen(rect, view, V2::new(0.0, y)).y;
            p.line_segment(
                [Pos2::new(rect.left(), sy), Pos2::new(rect.right(), sy)],
                Stroke::new(w, color),
            );
        }
    };
    draw_set(minor, Palette::GRID_MINOR, 1.0, p);
    draw_set(major, Palette::GRID_MAJOR, 1.0, p);
    // Achsen
    let o = world_to_screen(rect, view, V2::ZERO);
    if rect.contains(o) {
        p.line_segment(
            [Pos2::new(rect.left(), o.y), Pos2::new(rect.right(), o.y)],
            Stroke::new(1.2, Color32::from_rgb(0xC9, 0xC2, 0xB0)),
        );
        p.line_segment(
            [Pos2::new(o.x, rect.top()), Pos2::new(o.x, rect.bottom())],
            Stroke::new(1.2, Color32::from_rgb(0xC9, 0xC2, 0xB0)),
        );
    }
}

// ---------------------------------------------------------------------------
// Blatt
// ---------------------------------------------------------------------------

fn draw_sheet(app: &mut App, p: &Painter, rect: Rect, view: View, cursor: Option<V2>) {
    let Some(sid) = (match app.tab().tab {
        TabKind::Sheet(id) => Some(id),
        _ => None,
    }) else {
        return;
    };
    let Some(sheet) = app.doc().sheet(sid).cloned() else { return };
    // Arbeitsfläche
    p.rect_filled(rect, 0.0, Color32::from_rgb(0x0E, 0x10, 0x13));
    let (pw, ph) = sheet.size();
    let tl = world_to_screen(rect, view, V2::new(0.0, ph));
    let br = world_to_screen(rect, view, V2::new(pw, 0.0));
    let paper = Rect::from_min_max(tl, br);
    // Schatten
    p.rect_filled(
        paper.translate(Vec2::new(6.0, 10.0)),
        2.0,
        Color32::from_black_alpha(90),
    );
    p.rect_filled(paper, 1.0, Palette::PAPER);

    let w2s = |q: V2| world_to_screen(rect, view, q);
    let pc = PaintCtx {
        selected: Vec::new(),
        hover: None,
        lineweights: true,
        lineweights_scaled: true,
        paper_px: view.zoom,
    };
    let dl = render::render_sheet(app.doc(), &sheet);
    for prim in &dl.prims {
        paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
    }

    // Ansichtsfenster
    let selected_vp = app.tab().selected_viewport;
    for vp in &sheet.viewports {
        let vp_rect = Rect::from_min_max(
            w2s(V2::new(vp.x, vp.y + vp.h)),
            w2s(V2::new(vp.x + vp.w, vp.y)),
        );
        let s = 1.0 / vp.scale.max(1e-9);
        let vp_w2s = |q: V2| -> Pos2 {
            let paper_p = V2::new(
                vp.x + vp.w * 0.5 + (q.x - vp.center.x) * s,
                vp.y + vp.h * 0.5 + (q.y - vp.center.y) * s,
            );
            w2s(paper_p)
        };
        let clip = p.with_clip_rect(vp_rect);
        let ctx = render::RenderCtx::with_scale(app.doc(), vp.scale);
        let vdl = render::render_document(&ctx);
        let vpc = PaintCtx {
            selected: Vec::new(),
            hover: None,
            lineweights: true,
            lineweights_scaled: true,
            paper_px: view.zoom * vp.scale,
        };
        for prim in &vdl.prims {
            paint_prim(&clip, prim, &vp_w2s, view.zoom * s, &vpc, 1.0);
        }
        // Rahmen des Fensters
        let sel = selected_vp == Some(vp.id);
        let stroke = if sel {
            Stroke::new(2.0, Palette::SELECT)
        } else if vp.show_frame {
            Stroke::new(1.0, Palette::TEXT_FAINT)
        } else {
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x99, 0x99, 0x99, 60))
        };
        p.rect_stroke(vp_rect, 0.0, stroke);
        // Griffmarkierung
        if sel {
            for c in [
                vp_rect.left_top(),
                vp_rect.right_top(),
                vp_rect.left_bottom(),
                vp_rect.right_bottom(),
            ] {
                p.rect_filled(Rect::from_center_size(c, Vec2::splat(7.0)), 1.0, Palette::SELECT);
            }
        }
        p.text(
            Pos2::new(vp_rect.left() + 4.0, vp_rect.top() + 2.0),
            Align2::LEFT_TOP,
            format!("{} · {}", vp.name, format_scale(vp.scale)),
            FontId::proportional(9.5),
            Color32::from_rgba_unmultiplied(0x88, 0x88, 0x88, if sel { 255 } else { 110 }),
        );
    }

    // Aufziehen eines neuen Ansichtsfensters
    if let Some(tool) = &app.tool {
        if tool.action == "sheet.viewport_tool" {
            if let (Some(start), Some(cur)) = (tool.pts.first().copied(), cursor) {
                let r = Rect::from_two_pos(w2s(start), w2s(cur));
                p.rect_filled(
                    r,
                    0.0,
                    Color32::from_rgba_unmultiplied(0x2F, 0x8C, 0xF0, 26),
                );
                p.rect_stroke(r, 0.0, Stroke::new(1.4, Palette::SELECT));
                p.text(
                    Pos2::new(r.left() + 4.0, r.top() + 3.0),
                    Align2::LEFT_TOP,
                    format!("{:.0} × {:.0} mm", r.width(), r.height()),
                    FontId::proportional(10.5),
                    Palette::SELECT,
                );
            }
        }
    }

    // Bedienhinweis
    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 34.0),
        Align2::LEFT_TOP,
        "Ansicht anklicken · verschieben mit V · neues Fenster über Blatt > Ansicht",
        FontId::proportional(10.5),
        Palette::TEXT_FAINT,
    );
}

// ---------------------------------------------------------------------------
// Auswahl, Cursor, Vorschau
// ---------------------------------------------------------------------------

fn draw_cursor(
    app: &mut App,
    ui: &mut Ui,
    p: &Painter,
    rect: Rect,
    view: View,
    snapped: Option<V2>,
    raw: Option<V2>,
) {
    let Some(pos) = ui.ctx().pointer_hover_pos().filter(|q| rect.contains(*q)) else {
        return;
    };
    if let Some(w) = snapped {
        let s = world_to_screen(rect, view, w);
        let r = 5.0;
        p.line_segment([s - Vec2::new(r, 0.0), s + Vec2::new(r, 0.0)], Stroke::new(1.2, Palette::ACCENT));
        p.line_segment([s - Vec2::new(0.0, r), s + Vec2::new(0.0, r)], Stroke::new(1.2, Palette::ACCENT));
        if raw.map(|q| q.dist(w)).unwrap_or(0.0) * view.zoom > 1.0 {
            p.circle_stroke(s, 7.0, Stroke::new(1.0, Palette::ACCENT.gamma_multiply(0.7)));
        }
    }
    // Fadenkreuz bis zu den Bildschirmrändern
    let c = Color32::from_rgba_unmultiplied(88, 98, 112, 160);
    p.line_segment(
        [Pos2::new(rect.left(), pos.y), Pos2::new(rect.right(), pos.y)],
        Stroke::new(1.0, c),
    );
    p.line_segment(
        [Pos2::new(pos.x, rect.top()), Pos2::new(pos.x, rect.bottom())],
        Stroke::new(1.0, c),
    );

    // Aktives Werkzeug am Mauszeiger
    if let Some(t) = &app.tool {
        if let Some(def) = actions::action(&t.action) {
            let ir = Rect::from_center_size(pos + Vec2::new(22.0, 22.0), Vec2::splat(18.0));
            p.circle_filled(ir.center(), 12.0, Color32::from_rgba_unmultiplied(20, 22, 26, 190));
            p.circle_stroke(ir.center(), 12.0, Stroke::new(1.0, Palette::ACCENT.gamma_multiply(0.6)));
            crate::icons::paint(p, ir.shrink(2.0), def.icon, Palette::ACCENT);
        }
    }
}

fn draw_tool_preview(
    app: &mut App,
    p: &Painter,
    rect: Rect,
    view: View,
    snapped: Option<V2>,
) {
    let Some(cursor) = snapped else { return };
    let Some(tool) = app.tool.clone() else { return };
    let col = Palette::ACCENT.gamma_multiply(0.9);
    let stroke = Stroke::new(1.4, col);
    let dashed = Stroke::new(1.0, col.gamma_multiply(0.55));
    let w2s = |q: V2| world_to_screen(rect, view, q);
    let unit = app.unit();
    let last = tool.pts.last().copied();
    let a = if let Some(l) = last { l } else { cursor };

    let mut label = String::new();
    let mut label_pos = w2s(cursor) + Vec2::new(14.0, -16.0);

    match tool.action.as_str() {
        "draw.line" | "draw.wall" => {
            if let Some(start) = last {
                if tool.action == "draw.line" {
                    p.line_segment([w2s(start), w2s(cursor)], stroke);
                }
                let d = start.dist(cursor);
                let ang = (cursor - start).angle().to_degrees();
                label = format!(
                    "{} {}   {:.1}°",
                    unit.format_plain(d, unit.decimals()),
                    unit.short(),
                    ang
                );
                if tool.action == "draw.wall" {
                    // Vorschau wie die fertige Wand: Körper mit Ausrichtung, Achse gestrichelt
                    let t = app.options.wall_thickness * 0.5;
                    let dirw = (cursor - start).normalized();
                    let nrm = dirw.perp();
                    let off = nrm * (t * app.options.wall_align.offset());
                    let a0 = start + off;
                    let b0 = cursor + off;
                    let quad = vec![
                        w2s(a0 + nrm * t),
                        w2s(b0 + nrm * t),
                        w2s(b0 - nrm * t),
                        w2s(a0 - nrm * t),
                    ];
                    p.add(egui::Shape::convex_polygon(
                        quad,
                        Palette::SELECT.gamma_multiply(0.18),
                        Stroke::NONE,
                    ));
                    p.line_segment([w2s(a0 + nrm * t), w2s(b0 + nrm * t)], stroke);
                    p.line_segment([w2s(a0 - nrm * t), w2s(b0 - nrm * t)], stroke);
                    p.line_segment([w2s(a0), w2s(b0)], dashed);
                }
            }
        }
        "draw.polyline" => {
            for pair in tool.pts.windows(2) {
                p.line_segment([w2s(pair[0]), w2s(pair[1])], stroke);
            }
            if let Some(start) = last {
                p.line_segment([w2s(start), w2s(cursor)], dashed);
                label = format!(
                    "{} {}",
                    unit.format_plain(start.dist(cursor), unit.decimals()),
                    unit.short()
                );
            }
        }
        "draw.rect" => {
            if let Some(start) = last {
                let r = Rect::from_two_pos(w2s(start), w2s(cursor));
                p.rect_stroke(r, 0.0, stroke);
                label = format!(
                    "{} × {}",
                    unit.format_plain((cursor.x - start.x).abs(), unit.decimals()),
                    unit.format_plain((cursor.y - start.y).abs(), unit.decimals())
                );
            }
        }
        "draw.circle" => {
            if let Some(c) = last {
                let r = c.dist(cursor);
                p.circle_stroke(w2s(c), (r * view.zoom) as f32, stroke);
                p.line_segment([w2s(c), w2s(cursor)], dashed);
                label = format!("⌀ {}", unit.format_plain(r * 2.0, unit.decimals()));
            }
        }
        "draw.ellipse" => {
            if let Some(c) = last {
                let rx = (cursor.x - c.x).abs().max(1.0);
                let ry = (cursor.y - c.y).abs().max(1.0);
                let pts: Vec<Pos2> = (0..=64)
                    .map(|i| {
                        let t = i as f64 / 64.0 * std::f64::consts::TAU;
                        w2s(c + V2::new(rx * t.cos(), ry * t.sin()))
                    })
                    .collect();
                p.add(Shape::line(pts, stroke));
                label = format!("{} × {}", unit.format_plain(rx, 1), unit.format_plain(ry, 1));
            }
        }
        "draw.arc" => {
            if tool.pts.len() == 1 {
                p.line_segment([w2s(tool.pts[0]), w2s(cursor)], dashed);
            } else if tool.pts.len() >= 2 {
                let p0 = tool.pts[0];
                let p1 = tool.pts[1];
                if let Some(arc) = Arc::from_3_points(p0, cursor, p1) {
                    let pts: Vec<Pos2> = arc
                        .flatten(0.5 / view.zoom.max(1e-6))
                        .iter()
                        .map(|q| w2s(*q))
                        .collect();
                    p.add(Shape::line(pts, stroke));
                } else {
                    p.line_segment([w2s(p0), w2s(p1)], dashed);
                }
            }
        }
        "draw.stair" => {
            if let Some(start) = last {
                let dir = (cursor - start).normalized();
                let n = dir.perp() * (app.options.stair_width * 0.5);
                let quad = vec![
                    w2s(start + n),
                    w2s(cursor + n),
                    w2s(cursor - n),
                    w2s(start - n),
                ];
                p.add(Shape::closed_line(quad, stroke));
                let steps = app.options.stair_steps.max(1);
                for i in 1..steps {
                    let t = i as f64 / steps as f64;
                    let q = start.lerp(cursor, t);
                    p.line_segment([w2s(q + n), w2s(q - n)], dashed);
                }
                label = format!(
                    "{} {} · {} Stufen",
                    unit.format_plain(start.dist(cursor), unit.decimals()),
                    unit.short(),
                    steps
                );
            }
        }
        "draw.door" => {
            let width = app.options.door_width;
            // Vorschau liegt immer auf der nächsten Wand
            let (c, ang) = match wall_under(app, cursor) {
                Some((cp, a, id)) => (opening_center(app, id, cp, a), a),
                None => (cursor, 0.0),
            };
            let dir = V2::polar(ang, 1.0);
            let half = dir * (width * 0.5);
            p.line_segment([w2s(c - half), w2s(c + half)], stroke);
            // Türblatt + Öffnungsbogen andeuten
            let nrm = dir.perp();
            let hinge_at = if app.options.hinge >= 0.0 { c + half } else { c - half };
            let leaf_dir = if app.options.swing_in { nrm } else { -nrm };
            let leaf = hinge_at + leaf_dir * width;
            p.line_segment([w2s(hinge_at), w2s(leaf)], dashed);
            p.circle_stroke(w2s(hinge_at), (width * view.zoom) as f32, dashed);
            label = format!("Tür {}", unit.format_plain(width, 2));
        }
        "draw.window" => {
            let width = app.options.window_width;
            let (c, ang) = match wall_under(app, cursor) {
                Some((cp, a, id)) => (opening_center(app, id, cp, a), a),
                None => (cursor, 0.0),
            };
            let dir = V2::polar(ang, 1.0);
            let nrm = dir.perp();
            let half = dir * (width * 0.5);
            p.line_segment([w2s(c - half), w2s(c + half)], stroke);
            for off in [-60.0, 60.0] {
                p.line_segment(
                    [w2s(c - half + nrm * off), w2s(c + half + nrm * off)],
                    dashed,
                );
            }
            label = format!("Fenster {}", unit.format_plain(width, 2));
        }
        "draw.point" => {
            p.circle_stroke(w2s(cursor), 6.0, stroke);
        }
        "draw.text" => {
            let h = app.options.text_height;
            let txt = if app.options.text.is_empty() {
                "Text".to_string()
            } else {
                app.options.text.clone()
            };
            p.text(
                w2s(cursor),
                Align2::LEFT_BOTTOM,
                &txt,
                FontId::proportional((h * view.zoom * 1.3).clamp(8.0, 60.0) as f32),
                col,
            );
        }
        "draw.hatch" => {
            p.circle_stroke(w2s(cursor), 9.0, dashed);
            label = "Kontur anklicken".into();
        }
        "dim.linear" => {
            if tool.pts.len() == 1 {
                p.line_segment([w2s(tool.pts[0]), w2s(cursor)], dashed);
                label = format!(
                    "{} {}",
                    unit.format_plain(tool.pts[0].dist(cursor), unit.decimals()),
                    unit.short()
                );
            } else if tool.pts.len() >= 2 {
                let a = tool.pts[0];
                let b = tool.pts[1];
                let axis = app.options.dim_axis.unwrap_or(if (b.x - a.x).abs() >= (b.y - a.y).abs() {
                    DimAxis::X
                } else {
                    DimAxis::Y
                });
                let (d1, d2) = match axis {
                    DimAxis::X => (V2::new(a.x, cursor.y), V2::new(b.x, cursor.y)),
                    DimAxis::Y => (V2::new(cursor.x, a.y), V2::new(cursor.x, b.y)),
                    DimAxis::Aligned => {
                        let dir = (b - a).normalized();
                        let n = dir.perp();
                        let off = (cursor - a).dot(n);
                        (a + n * off, b + n * off)
                    }
                };
                p.line_segment([w2s(d1), w2s(d2)], stroke);
                p.line_segment([w2s(a), w2s(d1)], dashed);
                p.line_segment([w2s(b), w2s(d2)], dashed);
                let m = match axis {
                    DimAxis::X => (b.x - a.x).abs(),
                    DimAxis::Y => (b.y - a.y).abs(),
                    DimAxis::Aligned => a.dist(b),
                };
                label = format!(
                    "{} {}",
                    format_number(app.doc().settings.dim_unit.from_mm(m), app.doc().dim_style.decimals),
                    app.doc().settings.dim_unit.short()
                );
                label_pos = w2s((d1 + d2) * 0.5) + Vec2::new(10.0, -20.0);
            }
        }
        "dim.chain" => {
            for pair in tool.pts.windows(2) {
                p.line_segment([w2s(pair[0]), w2s(pair[1])], dashed);
            }
            if let Some(start) = last {
                p.line_segment([w2s(start), w2s(cursor)], dashed);
                label = format!(
                    "{} {}",
                    unit.format_plain(start.dist(cursor), unit.decimals()),
                    unit.short()
                );
            }
        }
        "dim.elevation" => {
            p.circle_stroke(w2s(cursor), 8.0, dashed);
            label = format!("Kote {:.2} m", app.options.elevation_z / 1000.0);
        }
        "modify.move" | "modify.copy" => {
            if let Some(base) = last {
                p.line_segment([w2s(base), w2s(cursor)], stroke);
                let ids = app.selection().to_vec();
                for id in ids {
                    if let Some(e) = app.doc().entity(id) {
                        if let EntityKind::Line { a, b } = &e.kind {
                            let m = Affine::translate(cursor - base);
                            p.line_segment([w2s(m.apply(*a)), w2s(m.apply(*b))], dashed);
                        }
                    }
                }
                label = format!(
                    "{} {}",
                    unit.format_plain(base.dist(cursor), unit.decimals()),
                    unit.short()
                );
            }
        }
        "modify.rotate" => {
            if let Some(c) = last {
                let a0 = (cursor - c).angle();
                p.line_segment([w2s(c), w2s(cursor)], stroke);
                p.circle_stroke(w2s(c), (c.dist(cursor) * view.zoom) as f32, dashed);
                label = format!("{:.1}°", a0.to_degrees());
            }
        }
        "modify.mirror" => {
            if let Some(p0) = last {
                let dir = (cursor - p0).normalized();
                let n = dir.perp();
                let far = 100000.0;
                p.line_segment(
                    [w2s(p0 - dir * far), w2s(p0 + dir * far)],
                    Stroke::new(1.0, Palette::SELECT),
                );
                let drag = cursor - p0;
                let m = Affine::mirror(p0, cursor);
                let ids = app.selection().to_vec();
                for id in ids {
                    if let Some(e) = app.doc().entity(id) {
                        let a = render::entity_bbox(app.doc(), e);
                        if a.is_valid() {
                            let c2 = m.apply(a.center());
                            let off = c2 - a.center();
                            p.line_segment([w2s(a.center()), w2s(c2)], dashed);
                            let _ = off;
                        }
                    }
                }
                let _ = n;
                let _ = drag;
            }
        }
        "modify.scale" => {
            if let Some(c) = last {
                let f = (c.dist(cursor) / 1000.0).max(0.05);
                p.line_segment([w2s(c), w2s(cursor)], stroke);
                label = format!("Faktor {:.2}", f);
            }
        }
        "modify.offset" => {
            if let Some(a) = last {
                p.line_segment([w2s(a), w2s(cursor)], dashed);
                label = format!("Seite wählen ({} mm)", app.options.offset_dist);
            } else {
                p.circle_stroke(w2s(cursor), 8.0, dashed);
                label = "Objekt anklicken".into();
            }
        }
        "block.insert" => {
            let name = app
                .options
                .block_id
                .as_ref()
                .and_then(|id| app.doc().block(id))
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Block".into());
            // Vorschau der Blockgeometrie
            if let Some(id) = &app.options.block_id {
                if let Some(def) = app.doc().block(id).cloned() {
                    let mut m = Affine::translate(cursor);
                    m = m * Affine::rotate(app.options.block_rot.to_radians());
                    let s = app.options.block_scale;
                    m = m * Affine::scale(s, s);
                    let mut dl = DisplayList::default();
                    let ctx = render::RenderCtx::new(app.doc());
                    for e in &def.entities {
                        render::render_entity(&ctx, e, m, &mut dl, 0);
                    }
                    for prim in &dl.prims {
                        let stroke2 = Stroke::new(1.2, Palette::SELECT.gamma_multiply(0.8));
                        if let PShape::Line(a, b) = &prim.shape {
                            p.line_segment([w2s(*a), w2s(*b)], stroke2);
                        }
                        if let PShape::Poly(pts, closed) = &prim.shape {
                            let mut pts2: Vec<Pos2> = pts.iter().map(|q| w2s(*q)).collect();
                            if *closed {
                                pts2.push(pts2[0]);
                            }
                            p.add(Shape::line(pts2, stroke2));
                        }
                    }
                }
            }
            label = name;
        }
        _ => {}
    }

    if !label.is_empty() {
        let galley = p.layout_no_wrap(label, FontId::proportional(11.0), Palette::TEXT);
        let bg = Rect::from_min_size(
            label_pos - Vec2::new(6.0, 3.0),
            galley.size() + Vec2::new(12.0, 6.0),
        );
        p.rect_filled(bg, 4.0, Color32::from_rgba_unmultiplied(24, 27, 32, 235));
        p.rect_stroke(bg, 4.0, Stroke::new(1.0, Palette::BORDER));
        p.galley(label_pos, galley, Palette::TEXT);
    }

}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

fn handle_navigation(app: &mut App, ui: &mut Ui, resp: &Response, rect: Rect, view: View) {
    let ctx = ui.ctx();
    // Mausrad-Zoom
    if resp.hovered() {
        let scroll = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll.abs() > 0.1 {
            let factor = if scroll > 0.0 { 1.12 } else { 1.0 / 1.12 };
            let pointer = ctx.pointer_hover_pos().unwrap_or(rect.center());
            let before = screen_to_world(rect, view, pointer);
            let mut v = view;
            v.zoom = (v.zoom * factor).clamp(1e-5, 80.0);
            let after = screen_to_world(rect, v, pointer);
            v.center += before - after;
            app.set_view(v);
        }
    }
    // Pan per Mitteltaste / Leertaste+Ziehen
    let space = ctx.input(|i| i.key_down(egui::Key::Space));
    if resp.dragged_by(egui::PointerButton::Middle) || (resp.dragged_by(egui::PointerButton::Primary) && space) {
        let d = resp.drag_delta();
        let mut v = app.current_view();
        v.center.x -= d.x as f64 / v.zoom;
        v.center.y += d.y as f64 / v.zoom;
        app.set_view(v);
    }
    // WASD-Schwenk
    if !ctx.wants_keyboard_input() {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let mut dir = Vec2::ZERO;
        if ctx.input(|i| i.key_down(egui::Key::A)) {
            dir.x -= 1.0;
        }
        if ctx.input(|i| i.key_down(egui::Key::D)) {
            dir.x += 1.0;
        }
        if ctx.input(|i| i.key_down(egui::Key::W)) {
            dir.y -= 1.0;
        }
        if ctx.input(|i| i.key_down(egui::Key::S)) {
            dir.y += 1.0;
        }
        if dir != Vec2::ZERO {
            let mut v = app.current_view();
            let speed = 700.0 * dt as f64 / v.zoom;
            v.center.x += dir.x as f64 * speed;
            v.center.y -= dir.y as f64 * speed;
            app.set_view(v);
            ctx.request_repaint();
        }
    }
    // Zoom per Doppelklick auf Mitteltaste = alles zeigen
    if resp.double_clicked_by(egui::PointerButton::Middle) {
        app.fit_view();
    }
    let _ = rect;
}

// ---------------------------------------------------------------------------
// Eingabe
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
/// Fängt einen Weltpunkt für eine beliebige Bildschirmposition (ohne Label-Nebenwirkungen).
fn snap_at(app: &App, rect: Rect, view: View, pos: Pos2) -> V2 {
    let mut w = screen_to_world(rect, view, pos);
    let s = &app.doc().settings.snap;
    let tol = (s.tolerance_px / view.zoom).max(0.5);
    let reference = app.tool.as_ref().and_then(|t| t.pts.last().copied());
    if s.ortho {
        if let Some(r) = reference {
            w = w.ortho_from(r, false);
        }
    } else if s.polar {
        if let Some(r) = reference {
            if w.dist(r) > tol * 2.0 {
                if let Some(p) = w.polar_snap_from_tol(r, s.polar_step.max(1.0), s.polar_tolerance) {
                    w = p;
                }
            }
        }
    }
    let ignore: Vec<Id> = app.selection().to_vec();
    if let Some(hit) = snap::find_snap(app.doc(), w, tol, reference, &ignore) {
        w = hit.point;
    }
    w
}

fn handle_input(
    app: &mut App,
    ui: &mut Ui,
    resp: &Response,
    rect: Rect,
    painter: &Painter,
    is_sheet: bool,
    snapped: Option<V2>,
    raw: Option<V2>,
) {
    let ctx = ui.ctx();
    let view = app.current_view();

    // Solange ein Kontextmenü offen ist, reagiert die Fläche nicht auf Klicks
    if app.menu_open() {
        return;
    }

    // Enter beendet Polylinie und Maßkette (Tippeingabe hat Vorrang)
    if app.ui.typed.is_empty()
        && !ctx.wants_keyboard_input()
        && ctx.input(|i| i.key_pressed(egui::Key::Enter))
    {
        if let Some(t) = app.tool.clone() {
            if matches!(t.action.as_str(), "draw.polyline" | "dim.chain") && t.pts.len() >= 2 {
                finish_tool(app, true);
                return;
            }
        }
    }

    // Tippen von Längen/Koordinaten
    let tool_waiting = app.tool.as_ref().map(|t| !t.pts.is_empty()).unwrap_or(false);
    if tool_waiting && !ctx.wants_keyboard_input() {
        ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Text(t) = ev {
                    for ch in t.chars() {
                        if ch.is_ascii_digit() || ch == '.' || ch == ',' || ch == '-' || ch == '+' || ch == ';' {
                            if app.ui.typed.len() < 18 {
                                app.ui.typed.push(ch);
                            }
                        }
                    }
                }
            }
            if i.key_pressed(egui::Key::Backspace) {
                app.ui.typed.pop();
            }
        });
        if !app.ui.typed.is_empty() {
            let pos = ctx.pointer_hover_pos().unwrap_or(rect.center());
            let text = app.ui.typed.clone();
            let galley = painter.layout_no_wrap(
                if text.contains(';') {
                    format!("{}  [Enter] Punkt x;y", text)
                } else {
                    format!("{}  [Enter] Länge", text)
                },
                FontId::proportional(13.0),
                Palette::ACCENT,
            );
            let bg = Rect::from_min_size(
                pos + Vec2::new(16.0, 18.0),
                galley.size() + Vec2::new(12.0, 6.0),
            );
            painter.rect_filled(bg, 4.0, Color32::from_rgba_unmultiplied(24, 27, 32, 240));
            painter.rect_stroke(bg, 4.0, Stroke::new(1.0, Palette::ACCENT));
            painter.galley(pos + Vec2::new(22.0, 21.0), galley, Palette::ACCENT);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let (Some(w), Some(last)) = (snapped, app.tool.as_ref().and_then(|t| t.pts.last().copied())) {
                let typed = app.ui.typed.clone();
                // Koordinaten nur mit Semikolon trennen — das Komma bleibt
                // Dezimaltrennzeichen ("2,5" = 2,5 m, "2,5;3,0" = x/y).
                if typed.contains(';') {
                    let parts: Vec<&str> = typed.split(';').collect();
                    if parts.len() == 2 {
                        let unit = app.unit();
                        if let (Some(x), Some(y)) = (
                            jjcad_core::units::parse_length(parts[0], unit),
                            jjcad_core::units::parse_length(parts[1], unit),
                        ) {
                            tool_click(app, V2::new(x, y));
                        }
                    }
                } else if let Some(dist) = jjcad_core::units::parse_length(&typed, app.unit()) {
                    let dir = (w - last).normalized();
                    if dir.len() > 0.0 {
                        tool_click(app, last + dir * dist);
                    }
                }
                app.ui.typed.clear();
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.typed.clear();
        }
    } else {
        app.ui.typed.clear();
    }

    if is_sheet {
        sheet_input(app, resp, rect, view);
        return;
    }

    let Some(pos) = ctx.pointer_hover_pos() else { return };
    if !rect.contains(pos) {
        return;
    }
    let w = match snapped {
        Some(w) => w,
        None => return,
    };
    let shift = ctx.input(|i| i.modifiers.shift);

    // Vorschau platzieren
    if app.ghost.is_some() {
        if resp.clicked_by(egui::PointerButton::Primary) {
            app.commit_ghost();
        } else if resp.clicked_by(egui::PointerButton::Secondary) {
            app.cancel_ghost();
        }
        return;
    }

    // Rechtsklick: Kontextmenü für ein Objekt oder die Fläche
    if resp.clicked_by(egui::PointerButton::Secondary) {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(w) = raw {
                let tol = (app.doc().settings.snap.tolerance_px / view.zoom).max(1.0);
                match snap::hit_test(app.doc(), w, tol, None) {
                    Some(id) => {
                        app.session_mut().select(vec![id]);
                        app.open_menu(crate::app::MenuKind::Object(id), pos);
                    }
                    None => app.open_menu(crate::app::MenuKind::Canvas, pos),
                }
            }
        }
    }

    // Klick auf die Zeichenfläche gibt Textfeldern den Fokus zurück
    if resp.clicked() {
        if let Some(id) = ui.ctx().memory(|m| m.focused()) {
            ui.ctx().memory_mut(|m| m.surrender_focus(id));
        }
    }

    // egui fasst mehrere Klicks EINES Frames zu einem zusammen. Beim schnellen
    // Klicken geht dadurch jeder zweite Klick verloren. Für Werkzeuge zählen wir
    // deshalb alle Linksklick-Loslassen des Frames einzeln.
    if app.tool.is_some() {
        let releases: Vec<Pos2> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => Some(*pos),
                    _ => None,
                })
                .collect()
        });
        if releases.len() > 1 {
            for pos in releases {
                if rect.contains(pos) {
                    let pw = snap_at(app, rect, view, pos);
                    tool_click(app, pw);
                }
            }
            return;
        }
    }

    // Klick
    if resp.clicked_by(egui::PointerButton::Primary) {
        if app.tool.is_some() {
            tool_click(app, w);
        } else {
            let tol = (8.0 / view.zoom).max(0.5);
            let hit = snap::hit_test(app.doc(), w, tol, None);
            match hit {
                Some(id) => {
                    let mut sel = if shift {
                        app.selection().to_vec()
                    } else {
                        Vec::new()
                    };
                    if sel.contains(&id) {
                        sel.retain(|x| *x != id);
                    } else {
                        sel.push(id);
                    }
                    app.session_mut().select(sel);
                }
                None => {
                    if !shift {
                        app.session_mut().clear_selection();
                    }
                }
            }
        }
    }

    // Rechte Maustaste: beenden / wiederholen
    if resp.clicked_by(egui::PointerButton::Secondary) {
        if let Some(tool) = app.tool.clone() {
            match tool.action.as_str() {
                "draw.polyline" | "dim.chain" => {
                    finish_tool(app, true);
                }
                _ => {
                    app.tool = None;
                }
            }
        }
    }

    // Fensterauswahl aufziehen
    // Griffe: direktes Bearbeiten der Geometrie
    if app.tool.is_none() {
        let zwei_klicks = app.ui.drag_two_clicks;
        // Laufende Verschiebung der Auswahl
        if let Some(md) = app.move_drag.as_ref() {
            let cur = snapped.or(raw).unwrap_or(md.cur);
            let start_frame = md.frame;
            app.update_move_drag(cur);
            let frame = resp.ctx.input(|i| i.time);
            let fertig = fertig_erkennen(resp, zwei_klicks, start_frame, frame);
            if fertig {
                app.end_move_drag();
            }
            return;
        }
        if app.grip_drag.is_some() {
            let start_frame = app.grip_start_frame;
            if let Some(pt) = snapped.or(raw) {
                app.update_grip_drag(pt);
            }
            let frame = resp.ctx.input(|i| i.time);
            let fertig = fertig_erkennen(resp, zwei_klicks, start_frame, frame);
            if fertig {
                app.end_grip_drag();
            }
            return;
        }
        // Griff hat Vorrang: Treffer mit der ROHEN Zeigerposition (nicht dem
        // Fangpunkt) und 12 Bildschirm-Pixel Radius — sonst muss man pixelgenau
        // treffen, sobald der Fang aufs Raster zieht.
        let griff_tol = 12.0 / view.zoom.max(1e-6);
        let griff = raw.and_then(|r| app.grip_at(r, griff_tol));
        if resp.hovered() {
            app.hover_grip = griff;
        }
        if resp.hovered() && resp.ctx.input(|i| i.pointer.primary_pressed()) {
            if let Some((id, kind)) = griff {
                app.begin_grip_drag(id, kind);
                app.grip_start_frame = resp.ctx.input(|i| i.time);
                app.select_drag = None;
                return;
            }
            // Klick auf ein bereits ausgewaehltes Objekt: Auswahl verschieben
            if let Some(r) = raw {
                let tol2 = 10.0 / view.zoom.max(1e-6);
                if let Some(hit) = snap::hit_test(app.doc(), r, tol2, None) {
                    if app.selection().contains(&hit) {
                        app.begin_move_drag(r, resp.ctx.input(|i| i.time));
                        app.select_drag = None;
                        return;
                    }
                }
            }
        }
    }

    if resp.drag_started_by(egui::PointerButton::Primary) && app.tool.is_none() {
        app.select_drag = Some((w, w, shift));
    }

    if let Some((start, _, add)) = app.select_drag {
        if resp.dragged_by(egui::PointerButton::Primary) {
            app.select_drag = Some((start, w, add));
        }
    }
    if resp.drag_stopped_by(egui::PointerButton::Primary) {
        if let Some((start, end, add)) = app.select_drag.take() {
            if start.dist(end) * view.zoom > 4.0 {
                let crossing = end.x < start.x;
                let ids = snap::select_window(app.doc(), start, end, crossing);
                let mut sel = if add { app.selection().to_vec() } else { Vec::new() };
                for id in ids {
                    if !sel.contains(&id) {
                        sel.push(id);
                    }
                }
                app.session_mut().select(sel);
            }
        }
    }
}

/// Ende einer Mausbewegung: im Zwei-Klick-Modus der naechste Klick (nicht im
/// Startframe), im Gedrueckthalten-Modus das Loslassen.
fn fertig_erkennen(resp: &Response, zwei_klicks: bool, start_frame: f64, frame: f64) -> bool {
    if zwei_klicks {
        (start_frame - frame).abs() > 1e-9 && resp.ctx.input(|i| i.pointer.primary_pressed())
    } else {
        resp.ctx.input(|i| i.pointer.any_released())
    }
}

fn sheet_input(app: &mut App, resp: &Response, rect: Rect, view: View) {
    let Some(sid) = (match app.tab().tab {
        TabKind::Sheet(id) => Some(id),
        _ => None,
    }) else {
        return;
    };
    let Some(pos) = resp.ctx.pointer_hover_pos() else { return };
    if !rect.contains(pos) {
        return;
    }
    let w = screen_to_world(rect, view, pos);

    // Werkzeug "Ansicht aufziehen": Rechteck in Papierkoordinaten
    if let Some(tool) = app.tool.clone() {
        if tool.action == "sheet.viewport_tool" {
            if resp.clicked_by(egui::PointerButton::Primary) {
                let mut t = tool.clone();
                t.pts.push(w);
                if t.pts.len() >= 2 {
                    let (a, b) = (t.pts[0], t.pts[1]);
                    // auf das Blatt begrenzen und auf 0,5 mm runden
                    let (sw, sh) = app.doc().sheet(sid).map(|s| s.size()).unwrap_or((420.0, 297.0));
                    let mut x = a.x.min(b.x);
                    let mut y = a.y.min(b.y);
                    let mut ww = (a.x - b.x).abs();
                    let mut hh = (a.y - b.y).abs();
                    x = x.clamp(0.0, sw - 10.0);
                    y = y.clamp(0.0, sh - 10.0);
                    ww = ww.min(sw - x);
                    hh = hh.min(sh - y);
                    x = (x * 2.0).round() / 2.0;
                    y = (y * 2.0).round() / 2.0;
                    ww = (ww * 2.0).round() / 2.0;
                    hh = (hh * 2.0).round() / 2.0;
                    if ww > 8.0 && hh > 8.0 {
                        let center = app.doc().bbox().center();
                        let scale = app.doc().sheet(sid).map(|s| s.scale).unwrap_or(100.0);
                        let u = app.unit();
                        let cx = u.from_mm(center.x);
                        let cy = u.from_mm(center.y);
                        let _ = app.session_mut().execute(
                            "sheet.add_viewport",
                            &json!({"sheet": sid, "x": x, "y": y, "w": ww, "h": hh,
                                    "scale": scale, "show_frame": true,
                                    "center": [cx, cy]}),
                        );
                        app.mark_changed();
                        app.tab_mut().selected_viewport = app
                            .doc()
                            .sheet(sid)
                            .and_then(|s| s.viewports.last().map(|v| v.id));
                    }
                    app.tool = None;
                } else {
                    app.tool = Some(t);
                }
            }
            return;
        }
    }

    // Auswahl per Klick
    if resp.clicked_by(egui::PointerButton::Primary) {
        let vps: Vec<(Id, f64, f64, f64, f64)> = app
            .doc()
            .sheet(sid)
            .map(|s| {
                s.viewports
                    .iter()
                    .map(|v| (v.id, v.x, v.y, v.w, v.h))
                    .collect()
            })
            .unwrap_or_default();
        let mut hit = None;
        for (id, x, y, vw, vh) in vps {
            if w.x >= x && w.x <= x + vw && w.y >= y && w.y <= y + vh {
                hit = Some(id);
            }
        }
        app.tab_mut().selected_viewport = hit;
    }

    // Verschieben nur mit aktivem Verschieben-Werkzeug (V)
    let move_tool = app.tool_id() == Some("modify.move");
    if move_tool && resp.drag_started_by(egui::PointerButton::Primary) {
        if app.tab().selected_viewport.is_some() {
            let snap = app.doc().clone();
            app.session_mut().history.record(&snap);
        }
    }
    if move_tool && resp.dragged_by(egui::PointerButton::Primary) {
        if let Some(vid) = app.tab().selected_viewport {
            let d = resp.drag_delta();
            let dx = d.x as f64 / view.zoom;
            let dy = -d.y as f64 / view.zoom;
            if let Some(sheet) = app.doc_mut().sheet_mut(sid) {
                if let Some(vp) = sheet.viewports.iter_mut().find(|v| v.id == vid) {
                    vp.x = ((vp.x + dx) * 2.0).round() / 2.0;
                    vp.y = ((vp.y + dy) * 2.0).round() / 2.0;
                }
            }
            app.mark_changed();
        }
    }
}


// ---------------------------------------------------------------------------
// Vorschau vor dem Platzieren (Block oder Zwischenablage)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Griffe zum direkten Bearbeiten
// ---------------------------------------------------------------------------

fn draw_grips(app: &App, p: &Painter, rect: Rect, view: View, selected: &[Id]) {
    if selected.is_empty() || selected.len() > 60 {
        return;
    }
    for id in selected {
        let Some(e) = app.doc().entity(*id) else {
            continue;
        };
        for (kind, gw) in crate::handles::grips_of(e) {
            let c = world_to_screen(rect, view, gw);
            // Griff unter dem Zeiger: groesser und in Warnfarbe, damit man
            // sieht, dass die Trefferflaeche greift
            let hot = app.hover_grip == Some((*id, kind));
            let size = if hot { 11.0 } else { 8.0 };
            match kind {
                crate::handles::GripKind::Vert(_) => {
                    let r = Rect::from_center_size(c, Vec2::splat(size));
                    p.rect_filled(
                        r,
                        egui::Rounding::same(1.5),
                        if hot { Palette::TEXT } else { Palette::ACCENT },
                    );
                    p.rect_stroke(r, egui::Rounding::same(1.5), Stroke::new(1.2_f32, Palette::BG));
                }
                crate::handles::GripKind::Edge(i) => {
                    let Some((_, dir)) = crate::handles::edge_dir(e, i) else {
                        continue;
                    };
                    // Raute quer zur Kante: steht für Parallelverschiebung
                    let d = egui::vec2(dir.x as f32, -(dir.y as f32));
                    let n = egui::vec2(-d.y, d.x);
                    let pts = vec![
                        c + d * 7.0,
                        c + n * 5.0,
                        c - d * 7.0,
                        c - n * 5.0,
                    ];
                    p.add(Shape::convex_polygon(pts, Palette::ACCENT, Stroke::new(1.2_f32, Palette::BG)));
                    // zwei kurze Pfeile in Normalenrichtung
                    p.line_segment(
                        [c + n * 7.0, c + n * 11.0],
                        Stroke::new(1.4_f32, Palette::ACCENT),
                    );
                    p.line_segment(
                        [c - n * 7.0, c - n * 11.0],
                        Stroke::new(1.4_f32, Palette::ACCENT),
                    );
                }
            }
        }
    }
}

fn draw_ghost(app: &App, p: &Painter, rect: Rect, view: View) {
    let Some(g) = &app.ghost else { return };
    let geo = crate::ghost::geometry(app.doc(), g, &app.ui.clipboard);
    let w2s = |q: V2| world_to_screen(rect, view, q);
    let zoom = view.zoom;

    let stroke = Stroke::new(1.5, Palette::ACCENT);
    let mut dl = jjcad_core::render::DisplayList::default();
    dl.prims = geo.prims.clone();
    for prim in &dl.prims {
        ghost_shape(p, &prim.shape, &w2s, zoom, stroke);
    }

    // Umriss und Ankerpunkt
    if geo.bbox.is_valid() {
        let a = w2s(V2::new(geo.bbox.min.x, geo.bbox.max.y));
        let b = w2s(V2::new(geo.bbox.max.x, geo.bbox.min.y));
        let r = Rect::from_two_pos(a, b);
        p.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, Palette::SELECT.gamma_multiply(0.55)),
        );
    }
    let anchor = w2s(g.pos);
    let c = Palette::ACCENT;
    p.line_segment([anchor - Vec2::new(6.0, 0.0), anchor + Vec2::new(6.0, 0.0)], Stroke::new(1.4, c));
    p.line_segment([anchor - Vec2::new(0.0, 6.0), anchor + Vec2::new(0.0, 6.0)], Stroke::new(1.4, c));

    // Kurzanzeige am Zeiger (der ausführliche Hinweis steht in der Optionsleiste)
    let mut badge = String::new();
    if g.rot_deg().abs() > 0.1 {
        badge.push_str(&format!("{:.0}°  ", g.rot_deg()));
    }
    if g.mirror {
        badge.push_str("gespiegelt");
    }
    if !badge.is_empty() {
        p.text(
            anchor + Vec2::new(12.0, -10.0),
            Align2::LEFT_BOTTOM,
            badge,
            FontId::proportional(11.0),
            Palette::ACCENT,
        );
    }
}

fn ghost_shape(p: &Painter, sh: &PShape, w2s: &dyn Fn(V2) -> Pos2, zoom: f64, stroke: Stroke) {
    match sh {
        PShape::Line(a, b) => {
            p.line_segment([w2s(*a), w2s(*b)], stroke);
        }
        PShape::Poly(pts, closed) => {
            if pts.len() < 2 {
                return;
            }
            let mut v: Vec<Pos2> = pts.iter().map(|q| w2s(*q)).collect();
            if *closed {
                v.push(v[0]);
            }
            p.add(egui::Shape::line(v, stroke));
        }
        PShape::Circle { c, r } => {
            p.circle_stroke(w2s(*c), (*r * zoom) as f32, stroke);
        }
        PShape::Arc { c, r, a0, a1 } => {
            let arc = Arc::new(*c, *r, *a0, *a1);
            let pts: Vec<Pos2> = arc
                .flatten(0.6 / zoom.max(1e-6))
                .iter()
                .map(|q| w2s(*q))
                .collect();
            p.add(egui::Shape::line(pts, stroke));
        }
        PShape::Ellipse { c, rx, ry, rot } => {
            let pts: Vec<Pos2> = (0..=72)
                .map(|i| {
                    let t = i as f64 / 72.0 * std::f64::consts::TAU;
                    w2s(*c + V2::new(rx * t.cos(), ry * t.sin()).rotated(*rot))
                })
                .collect();
            p.add(egui::Shape::line(pts, stroke));
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Werkzeug-Logik
// ---------------------------------------------------------------------------

pub fn finish_tool(app: &mut App, from_enter: bool) {
    let Some(tool) = app.tool.clone() else { return };
    match tool.action.as_str() {
        "draw.polyline" => {
            if tool.pts.len() >= 2 {
                let layer = app.doc().current_layer;
                let _ = run(
                    app,
                    "draw.polyline",
                    json!({"pts": tool.pts.iter().map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)]).collect::<Vec<_>>(), "layer": layer}),
                );
            }
            app.tool = None;
        }
        "dim.chain" => {
            if tool.pts.len() >= 2 {
                let last = *tool.pts.last().unwrap();
                let b = BBox::from_points(&tool.pts);
                let line_pos = V2::new(b.min.x, b.min.y - app.doc().settings.annotation_scale * 8.0);
                let axis = if (last.x - tool.pts[0].x).abs() >= (last.y - tool.pts[0].y).abs() {
                    "x"
                } else {
                    "y"
                };
                let _ = run(
                    app,
                    "dim.chain",
                    json!({"pts": tool.pts.iter().map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)]).collect::<Vec<_>>(),
                           "line_pos": [app.unit().from_mm(line_pos.x), app.unit().from_mm(line_pos.y)], "axis": axis}),
                );
            }
            app.tool = None;
        }
        _ => {
            if from_enter {
                let has_sel = !app.selection().is_empty();
                if let Some(t) = &mut app.tool {
                    if t.selecting && has_sel {
                        t.selecting = false;
                    }
                }
            }
        }
    }
}

fn tool_click(app: &mut App, w: V2) {
    let Some(mut tool) = app.tool.clone() else { return };
    let unit = app.unit();
    let ax = |p: V2| [unit.from_mm(p.x), unit.from_mm(p.y)];

    // Auswahl sammeln
    if tool.selecting {
        let tol = (8.0 / app.current_view().zoom).max(0.5);
        if let Some(id) = snap::hit_test(app.doc(), w, tol, None) {
            let mut sel = app.selection().to_vec();
            if sel.contains(&id) {
                sel.retain(|x| *x != id);
            } else {
                sel.push(id);
            }
            app.session_mut().select(sel);
        }
        return;
    }

    match tool.action.as_str() {
        "draw.point" => {
            let _ = run(app, "draw.point", json!({"p": ax(w)}));
        }
        "draw.line" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let start = tool.pts[0];
                if start.dist(w) > 0.5 {
                    let _ = run(app, "draw.line", json!({"a": ax(start), "b": ax(w)}));
                    // Kette: weiter vom Endpunkt
                    app.tool = Some(Tool {
                        action: "draw.line".into(),
                        pts: vec![w],
                        selecting: false,
                        phase: ToolPhase::Collecting,
                    });
                }
            }
        }
        "draw.wall" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let start = tool.pts[0];
                if start.dist(w) > 0.5 {
                    let t = app.options.wall_thickness;
                    let h = app.options.wall_height;
                    let _ = run(
                        app,
                        "draw.wall",
                        json!({"a": ax(start), "b": ax(w), "thickness": unit.from_mm(t),
                                    "height": unit.from_mm(h), "align": align_name(app.options.wall_align)}),
                    );
                    app.tool = Some(Tool {
                        action: "draw.wall".into(),
                        pts: vec![w],
                        selecting: false,
                        phase: ToolPhase::Collecting,
                    });
                }
            }
        }
        "draw.polyline" => {
            tool.pts.push(w);
            app.tool = Some(tool);
        }
        "draw.rect" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let a = tool.pts[0];
                let b = w;
                if a.x != b.x && a.y != b.y {
                    let _ = run(
                        app,
                        "draw.rect",
                        json!({"a": ax(a), "b": ax(b)}),
                    );
                }
                app.tool = None;
            }
        }
        "draw.circle" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let c = tool.pts[0];
                let r = c.dist(w);
                if r > 0.5 {
                    let _ = run(
                        app,
                        "draw.circle",
                        json!({"c": ax(c), "r": unit.from_mm(r)}),
                    );
                }
                app.tool = None;
            }
        }
        "draw.ellipse" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let c = tool.pts[0];
                let rx = (w.x - c.x).abs().max(1.0);
                let ry = (w.y - c.y).abs().max(1.0);
                let _ = run(
                    app,
                    "draw.ellipse",
                    json!({"c": ax(c), "rx": unit.from_mm(rx), "ry": unit.from_mm(ry)}),
                );
                app.tool = None;
            }
        }
        "draw.arc" => {
            tool.pts.push(w);
            if tool.pts.len() == 3 {
                let (p0, p1, pm) = (tool.pts[0], tool.pts[1], tool.pts[2]);
                if let Some(arc) = Arc::from_3_points(p0, pm, p1) {
                    let _ = run(
                        app,
                        "draw.arc",
                        json!({"c": ax(arc.c), "r": unit.from_mm(arc.r), "a0": arc.a0.to_degrees(), "a1": arc.a1.to_degrees()}),
                    );
                }
                app.tool = None;
            } else {
                app.tool = Some(tool);
            }
        }
        "draw.text" => {
            app.ui.text_dialog = Some(crate::dialogs::TextDialog {
                pos: w,
                text: if app.options.text.is_empty() {
                    String::new()
                } else {
                    app.options.text.clone()
                },
                height: app.options.text_height,
                angle: 0.0,
                layer: app.doc().current_layer,
                editing_id: None,
            });
            app.tool = None;
        }
        "draw.hatch" => {
            let tol = (10.0 / app.current_view().zoom).max(0.5);
            let hit = snap::hit_test(app.doc(), w, tol, None);
            if let Some(id) = hit {
                if let Some(e) = app.doc().entity(id).cloned() {
                    let boundary = match &e.kind {
                        EntityKind::Polyline { verts, closed } if *closed => {
                            Some(verts.iter().map(|v| v.p).collect::<Vec<_>>())
                        }
                        EntityKind::Wall { a, b, thickness, .. } => {
                            let dir = (*b - *a).normalized();
                            let n = dir.perp() * (*thickness * 0.5);
                            Some(vec![*a + n, *b + n, *b - n, *a - n])
                        }
                        _ => None,
                    };
                    match boundary {
                        Some(pts) => {
                            let p = app.options.hatch_pattern;
                            let pat = match p {
                                HatchPattern::Solid => "solid",
                                HatchPattern::Lines => "lines",
                                HatchPattern::Cross => "cross",
                                HatchPattern::Dots => "dots",
                                HatchPattern::Insulation => "insulation",
                            };
                            let _ = run(
                                app,
                                "draw.hatch",
                                json!({"boundary": pts.iter().map(|q| ax(*q)).collect::<Vec<_>>(),
                                       "pattern": pat, "spacing": unit.from_mm(app.options.hatch_spacing)}),
                            );
                        }
                        None => app.toast("Bitte eine geschlossene Kontur anklicken"),
                    }
                }
            }
        }
        "draw.door" => {
            let d = app.options.door_kind;
            let kind = match d {
                DoorKind::Hinge => "hinge",
                DoorKind::Sliding => "sliding",
                DoorKind::Double => "double",
                DoorKind::Opening => "opening",
            };
            let (cp, wa, host) = match wall_under(app, w) {
                Some(v) => (v.0, v.1, Some(v.2)),
                None => (w, 0.0, None),
            };
            if host.is_none() {
                app.toast("Türen müssen auf einer Wand liegen");
                return;
            }
            let _ = run(
                app,
                "draw.door",
                json!({"center": ax(cp), "angle": wa, "host": [host.unwrap()],
                       "width": unit.from_mm(app.options.door_width),
                       "kind": kind, "hinge": app.options.hinge, "swing_in": app.options.swing_in}),
            );
        }
        "draw.window" => {
            let (cp, wa, host) = match wall_under(app, w) {
                Some(v) => (v.0, v.1, Some(v.2)),
                None => (w, 0.0, None),
            };
            if host.is_none() {
                app.toast("Fenster müssen auf einer Wand liegen");
                return;
            }
            let _ = run(
                app,
                "draw.window",
                json!({"center": ax(cp), "angle": wa, "host": [host.unwrap()],
                       "width": unit.from_mm(app.options.window_width), "sill": unit.from_mm(app.options.window_sill)}),
            );
        }
        "draw.stair" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let a = tool.pts[0];
                if a.dist(w) > 0.5 {
                    let dir = match app.options.stair_dir {
                        StairDir::Up => "up",
                        StairDir::Down => "down",
                        StairDir::Both => "both",
                    };
                    let _ = run(
                        app,
                        "draw.stair",
                        json!({"a": ax(a), "b": ax(w), "width": unit.from_mm(app.options.stair_width),
                               "steps": app.options.stair_steps, "direction": dir}),
                    );
                }
                app.tool = None;
            }
        }
        "dim.linear" => {
            tool.pts.push(w);
            if tool.pts.len() >= 3 {
                let (a, b, lp) = (tool.pts[0], tool.pts[1], tool.pts[2]);
                let axis = match app.options.dim_axis {
                    Some(DimAxis::X) => "x",
                    Some(DimAxis::Y) => "y",
                    Some(DimAxis::Aligned) => "aligned",
                    None => {
                        if (b.x - a.x).abs() >= (b.y - a.y).abs() {
                            "x"
                        } else {
                            "y"
                        }
                    }
                };
                let _ = run(
                    app,
                    "dim.linear",
                    json!({"a": ax(a), "b": ax(b), "line_pos": ax(lp), "axis": axis}),
                );
                app.tool = None;
            } else {
                app.tool = Some(tool);
            }
        }
        "dim.chain" => {
            tool.pts.push(w);
            app.tool = Some(tool);
        }
        "dim.elevation" => {
            let z = app.options.elevation_z;
            let _ = run(
                app,
                "dim.elevation",
                json!({"p": ax(w), "z": unit.from_mm(z)}),
            );
        }
        "modify.move" | "modify.copy" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let base = tool.pts[0];
                let ids = app.selection().to_vec();
                let action = tool.action.clone();
                let _ = run(
                    app,
                    &action,
                    json!({"ids": ids, "from": ax(base), "to": ax(w)}),
                );
                app.tool = None;
            }
        }
        "modify.rotate" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let c = tool.pts[0];
                let a = (w - c).angle().to_degrees();
                let ids = app.selection().to_vec();
                let _ = run(
                    app,
                    "modify.rotate",
                    json!({"ids": ids, "center": ax(c), "angle": a}),
                );
                app.tool = None;
            }
        }
        "modify.mirror" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let p0 = tool.pts[0];
                let ids = app.selection().to_vec();
                let _ = run(
                    app,
                    "modify.mirror",
                    json!({"ids": ids, "p1": ax(p0), "p2": ax(w), "keep_original": true}),
                );
                app.tool = None;
            }
        }
        "modify.scale" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let c = tool.pts[0];
                let f = (c.dist(w) / 1000.0).max(0.05);
                let ids = app.selection().to_vec();
                let _ = run(
                    app,
                    "modify.scale",
                    json!({"ids": ids, "center": ax(c), "factor": f}),
                );
                app.tool = None;
            }
        }
        "modify.offset" => {
            if tool.pts.is_empty() {
                let tol = (8.0 / app.current_view().zoom).max(0.5);
                if let Some(id) = snap::hit_test(app.doc(), w, tol, None) {
                    tool.pts.push(w);
                    app.tool = Some(tool);
                    app.session_mut().select(vec![id]);
                } else {
                    app.toast("Objekt anklicken");
                }
            } else {
                let ids = app.selection().to_vec();
                let d = app.options.offset_dist;
                let _ = run(
                    app,
                    "modify.offset",
                    json!({"ids": ids, "distance": unit.from_mm(d), "side": ax(w)}),
                );
                app.tool = None;
            }
        }
        "modify.trim" | "modify.extend" => {
            let tol = (8.0 / app.current_view().zoom).max(0.5);
            if let Some(id) = snap::hit_test(app.doc(), w, tol, None) {
                let action = tool.action.clone();
                let _ = run(app, &action, json!({"ids": [id], "at": ax(w)}));
            }
            // Werkzeug bleibt aktiv für weitere Schnitte
        }
        "modify.explode" => {
            let tol = (8.0 / app.current_view().zoom).max(0.5);
            if let Some(id) = snap::hit_test(app.doc(), w, tol, None) {
                let _ = run(app, "modify.explode", json!({"ids": [id]}));
            }
        }
        "block.insert" => {
            let Some(bid) = app.options.block_id.clone() else {
                app.toast("Kein Block gewählt – Inventar (E) öffnen");
                return;
            };
            let _ = run(
                app,
                "block.insert",
                json!({"block": bid, "pos": ax(w), "rot": app.options.block_rot, "scale": app.options.block_scale}),
            );
        }
        "modify.select" => {}
        _ => {}
    }
}

fn run(app: &mut App, action: &str, params: serde_json::Value) -> bool {
    // Aktuelle Zeichen-Eigenschaften (Farbe/Stärke/Typ) an neue Elemente weitergeben
    let mut params = params;
    if action.starts_with("draw.") {
        if let Some(o) = params.as_object_mut() {
            let pend = app.ui.pending.clone();
            if !o.contains_key("color") {
                if let Some(c) = pend.color {
                    o.insert("color".into(), serde_json::json!(c.to_hex()));
                }
            }
            if !o.contains_key("fill") {
                if let Some(f) = pend.fill {
                    o.insert("fill".into(), serde_json::json!(f.to_hex()));
                }
            }
            if !o.contains_key("lineweight") {
                if let Some(w) = pend.lineweight {
                    o.insert("lineweight".into(), serde_json::json!(w));
                }
            }
            if !o.contains_key("linetype") {
                if let Some(t) = pend.line_type {
                    let name = match t {
                        jjcad_core::style::LineType::Solid => "solid",
                        jjcad_core::style::LineType::Dashed => "dashed",
                        jjcad_core::style::LineType::Dotted => "dotted",
                        jjcad_core::style::LineType::DashDot => "dashdot",
                        jjcad_core::style::LineType::Hidden => "hidden",
                    };
                    o.insert("linetype".into(), serde_json::json!(name));
                }
            }
        }
    }
    match app.session_mut().execute(action, &params) {
        Ok(_) => {
            app.mark_changed();
            true
        }
        Err(e) => {
            app.toast(e);
            false
        }
    }
}

