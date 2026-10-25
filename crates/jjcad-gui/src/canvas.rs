//! Zeichnungsfläche: Anzeige, Fang, Werkzeugbedienung, Navigation.

use crate::app::{screen_to_world, world_to_screen, App, TabKind, Tool, ToolPhase, View};
use crate::theme::Palette;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::geom::*;
use jjcad_core::geom::Vec2 as V2;
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
    /// Vorschau-Modus: einheitliche Akzentkontur (wie die Einfüge-Vorschau),
    /// ohne Auswahl-/Hover-Hervorhebung und ohne Originalfarbe/-strichart.
    preview: bool,
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

/// Grenze, unter der eine Füllung auf dem Bildschirm verschwindet (Pixel).
/// Unter einem Pixel Breite ist nichts zu sehen — solche Streifen dürfen nicht
/// tesselliert werden (siehe `paint_prim`, Fill-Zweig).
const FILL_MIN_WIDTH_PX: f64 = 0.75;

/// Ist die Füllung auf dem Bildschirm schmaler als ein Pixel?
///
/// Maß ist die kleinste Breite des Polygons (2·Fläche/Umfang) in Pixeln; die
/// Länge spielt dabei keine Rolle. Abläufe, die nur die Fläche betrachten,
/// verpassen lange, hauchdünne Streifen.
fn fill_too_thin(v: &[Pos2]) -> bool {
    if v.len() < 3 {
        return true;
    }
    let pts: Vec<V2> = v.iter().map(|q| V2::new(q.x as f64, q.y as f64)).collect();
    jjcad_core::geom::polygon_min_width(&pts) < FILL_MIN_WIDTH_PX
}

fn paint_prim(p: &Painter, prim: &Prim, w2s: &dyn Fn(V2) -> Pos2, scale: f64, pc: &PaintCtx, alpha: f32) {
    let is_sel = !pc.preview && pc.selected.contains(&prim.entity) && prim.entity != 0;
    let is_hover = !pc.preview && pc.hover == Some(prim.entity) && prim.entity != 0 && !is_sel;
    let mut col = color32(prim.style.color);
    if pc.preview {
        // Wie die Einfüge-Vorschau (ghost): durchgezogene Akzentkontur
        col = Palette::ACCENT;
    } else if is_sel {
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
    let w = if pc.preview {
        1.2
    } else if is_sel || is_hover {
        base_w.max(1.6) + 0.6
    } else {
        base_w
    };
    let stroke = Stroke::new(w as f32, col);
    let dash = if pc.preview {
        None
    } else {
        dash_pattern(prim.style.line_type, pc.paper_px, pc.lineweights_scaled)
    };

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
        PShape::Fill { outer, holes } => {
            if outer.len() < 3 {
                return;
            }
            let v: Vec<Pos2> = outer.iter().map(|q| w2s(*q)).collect();
            if v.iter().any(|q| !q.x.is_finite() || !q.y.is_finite()) {
                return;
            }
            // Unterpixelbreite Füllungen nicht zeichnen. Wandfüllungen enthalten
            // das 0,4-mm-Füllband auf der Gehrungsnaht (Haarnnaht gegen
            // Antialiasing-Streifen); bei kleiner Ansicht ist der Streifen
            // schmaler als ein Pixel. egui tesselliert solche entarteten
            // Polygone zu Dreiecken mit praktisch unendlichen Ecken und zieht
            // dadurch lange diagonale Striche quer über die Zeichenfläche
            // (Gitternetz aus Winkelhalbierenden). Sichtbar ist der Streifen
            // ohnehin nicht — also vorher ausblenden.
            if fill_too_thin(&v) {
                return;
            }
            let fill = prim.style.fill.unwrap_or(prim.style.color);
            let c = if pc.preview {
                // Nur andeuten: die Vorschau darf den Plan darunter nicht verdecken
                Palette::ACCENT.gamma_multiply(0.12)
            } else if is_sel {
                Palette::SELECT.gamma_multiply(0.25)
            } else if is_hover {
                Palette::HOVER.gamma_multiply(0.2)
            } else {
                color32(fill)
            };
            // egui füllt nur konvexe Polygone und kennt keine Aussparungen.
            // Löcher deshalb über null breite Stege zu **einem** Ring verbinden
            // (`area::bridge_holes`) und den triangulieren — dieselbe Fläche
            // wie im PDF/PNG, nur anders zerlegt.
            let mut screen: Vec<Pos2> = if holes.is_empty() {
                v
            } else {
                jjcad_core::area::bridge_holes(outer, holes)
                    .iter()
                    .map(|q| w2s(*q))
                    .collect()
            };
            // Der Triangulierer erwartet positiven Umlaufsinn in
            // Bildschirmkoordinaten (y nach unten); sonst findet er keine Ohren.
            if flaeche_signiert(&screen) < 0.0 {
                screen.reverse();
            }
            for tri in triangulate(&screen) {
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

#[cfg(test)]
mod tests_loecher {
    use super::*;

    fn rechteck(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<V2> {
        vec![
            V2::new(x0 as f64, y0 as f64),
            V2::new(x1 as f64, y0 as f64),
            V2::new(x1 as f64, y1 as f64),
            V2::new(x0 as f64, y1 as f64),
        ]
    }

    /// egui füllt nur konvexe Polygone: die Insel wird über einen Steg zu einem
    /// Ring verbunden und trianguliert. Die Dreiecke müssen zusammen genau die
    /// Fläche ohne Loch ergeben — und keines darf im Loch liegen (sonst wäre die
    /// Insel auf dem Bildschirm übermalt, im PDF aber frei).
    #[test]
    fn dreiecke_fuellen_die_flaeche_ohne_das_loch() {
        let k: f32 = 0.1; // 0,1 px je mm
        let aussen = rechteck(0.0, 0.0, 400.0, 300.0);
        let loch = rechteck(100.0, 100.0, 200.0, 200.0);
        let ring = jjcad_core::area::bridge_holes(&aussen, &[loch]);
        let mut screen: Vec<Pos2> = ring
            .iter()
            .map(|q| Pos2::new((q.x * k as f64) as f32, (q.y * k as f64) as f32))
            .collect();
        if flaeche_signiert(&screen) < 0.0 {
            screen.reverse();
        }
        let tris = triangulate(&screen);
        assert!(!tris.is_empty(), "keine Dreiecke");
        let flaeche: f32 = tris
            .iter()
            .map(|t| {
                ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x)).abs()
                    * 0.5
            })
            .sum();
        let soll = (400.0 * 300.0 - 100.0 * 100.0) * k * k;
        assert!(
            (flaeche - soll).abs() < 1.5,
            "Fläche {flaeche} px² statt {soll} px²"
        );
        for t in &tris {
            let c = Pos2::new(
                (t[0].x + t[1].x + t[2].x) / 3.0,
                (t[0].y + t[1].y + t[2].y) / 3.0,
            );
            let im_loch = c.x > 100.0 * k + 0.01
                && c.x < 200.0 * k - 0.01
                && c.y > 100.0 * k + 0.01
                && c.y < 200.0 * k - 0.01;
            assert!(!im_loch, "Dreieck im Loch bei {c:?}");
        }
    }
}

/// Vorzeichen der Polygonfläche (Shoelace) in Bildschirmkoordinaten.
fn flaeche_signiert(v: &[Pos2]) -> f32 {
    let mut a = 0.0;
    for i in 0..v.len() {
        let p = v[i];
        let q = v[(i + 1) % v.len()];
        a += p.x * q.y - p.y * q.x;
    }
    a * 0.5
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
                // Nur **echt** innen liegende Punkte blockieren das Ohr. Ein
                // Punkt auf der Kante gehört zum Ohr selbst: bei verbundenen
                // Löchern ist der Steg doppelt vorhanden, und die zweite Kopie
                // der Stegecke liegt exakt auf der Kante — mit `>= 0` würde
                // jedes Ohr am Steg verworfen und der Rest als Fächer mit
                // überlappenden Dreiecken ausgegeben (Fläche zu groß).
                const EPS: f32 = 1e-4;
                let inside_tri =
                    area(a, b, p) > EPS && area(b, c, p) > EPS && area(c, a, p) > EPS;
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
    // Entartete Dreiecke (Fläche ~0) nicht zurückgeben: egui zeichnet sie nicht,
    // tesselliert sie aber zu Formen mit unbrauchbaren Normalen — im
    // ungünstigen Fall als lange Striche quer über die Fläche.
    out.retain(|t| area(t[0], t[1], t[2]).abs() >= 0.01);
    out
}

#[cfg(test)]
mod tests_haarnnaht {
    use super::*;

    /// Anzeige bei „ganze Zeichnung im Fenster“: rund 53 px je Meter.
    const K: f32 = 0.053;

    fn rechteck(w_mm: f32, h_mm: f32) -> Vec<Pos2> {
        vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(w_mm * K, 0.0),
            Pos2::new(w_mm * K, h_mm * K),
            Pos2::new(0.0, h_mm * K),
        ]
    }

    /// Die Haarnnaht an der Gehrungsnaht (0,4 mm breit) verschwindet unter einem
    /// Pixel und darf nicht tesselliert werden — sonst zieht egui lange
    /// Diagonalen über die Zeichenfläche.
    #[test]
    fn haarnnaht_wird_verworfen() {
        assert!(fill_too_thin(&rechteck(300.0, 0.4)));
        assert!(fill_too_thin(&rechteck(10500.0, 0.4)));
    }

    /// Die Wandfläche selbst bleibt sichtbar (300 mm ≈ 16 px).
    #[test]
    fn wandflaeche_wird_gezeichnet() {
        assert!(!fill_too_thin(&rechteck(300.0, 300.0)));
        assert!(!fill_too_thin(&rechteck(2100.0, 300.0)));
        assert!(!fill_too_thin(&rechteck(300.0, 115.0)));
    }

    /// Entartete Polygone (doppelte Punkte, weniger als drei Ecken) fallen raus.
    #[test]
    fn entartete_fuellung_wird_verworfen() {
        assert!(fill_too_thin(&[]));
        assert!(fill_too_thin(&[Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)]));
        assert!(fill_too_thin(&[
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 10.0),
            Pos2::new(20.0, 20.0),
        ]));
    }
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
            // Ausgewählte Objekte bleiben mit ihren Ecken/Endpunkten fangbar.
            // Nur während sie live gezogen werden (Maus-Verschieben, Griff),
            // fallen sie ganz aus dem Fang — sonst zieht der Fang das Objekt
            // auf sich selbst zurück und die Bewegung klebt am Startpunkt.
            let (ignore, keep) = snap_ignore_keep(app);
            if let Some(hit) = snap::find_snap(app.doc(), w, tol, reference, ignore, keep) {
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
            preview: false,
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
        preview: false,
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
            preview: false,
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
        "draw.fill.boundary" | "draw.fill.points" => {
            // Polygonzug bis zum Zeiger — der Umriss, der beim Klick entsteht.
            for pair in tool.pts.windows(2) {
                p.line_segment([w2s(pair[0]), w2s(pair[1])], stroke);
            }
            if let Some(start) = last {
                p.line_segment([w2s(start), w2s(cursor)], stroke);
                // Schlusskante zum Anfang: zeigt, wo sich das Polygon schließt
                if let Some(first) = tool.pts.first() {
                    p.line_segment([w2s(cursor), w2s(*first)], dashed);
                }
            }
            let mut umriss = tool.pts.clone();
            umriss.push(cursor);
            draw_fill_ghost(app, p, rect, view, &umriss);
            if let Some(pts) = fill_boundary_points(&umriss) {
                label = format!(
                    "{} Punkte · {}",
                    pts.len(),
                    unit.format_area(polygon_area(&pts).abs(), 2)
                );
            } else {
                label = format!("{} Punkte", tool.pts.len() + 1);
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
        "draw.fill" | "draw.hatch" => {
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
                draw_selection_preview(app, p, rect, view, &ids, cursor - base);
                label = format!(
                    "{} {}",
                    unit.format_plain(base.dist(cursor), unit.decimals()),
                    unit.short()
                );
            }
        }
        "modify.rotate" => {
            // Winkelwahl über „von"/„bis": der Winkel ist die Differenz der
            // beiden vom Drehzentrum aus gesehenen Richtungen.
            if let Some(&c) = tool.pts.first() {
                center_mark(p, w2s(c), Palette::ACCENT);
                match tool.pts.get(1).copied() {
                    None => {
                        // Von-Richtung (Bezugsrichtung) wird gesetzt
                        if c.dist(cursor) > 1e-6 {
                            p.line_segment([w2s(c), w2s(cursor)], dashed);
                        }
                        p.circle_stroke(w2s(cursor), 5.0, stroke);
                        label = format!(
                            "von {}°",
                            format_number((cursor - c).angle().to_degrees(), 1)
                        );
                    }
                    Some(from) => {
                        let v0 = from - c;
                        let v1 = cursor - c;
                        p.line_segment([w2s(c), w2s(from)], stroke);
                        match jjcad_core::transform::angle_between_directions(c, from, cursor) {
                            Some(angle) => {
                                p.line_segment([w2s(c), w2s(cursor)], dashed);
                                // Winkelbogen in Drehrichtung (mit Vorzeichen, <= 180°).
                                // Nicht über geom::Arc: das läuft immer gegen den
                                // Uhrzeigersinn und zeichnet bei negativem Winkel
                                // den langen Weg (315° statt 45°).
                                let r = v0.len().min(v1.len()) * 0.45;
                                if r > 1.0 {
                                    let a0 = v0.angle();
                                    let sweep = angle.to_radians();
                                    let pts: Vec<Pos2> = (0..=48)
                                        .map(|i| {
                                            let a = a0 + sweep * (i as f64 / 48.0);
                                            w2s(c + V2::new(a.cos(), a.sin()) * r)
                                        })
                                        .collect();
                                    p.add(Shape::line(pts, stroke));
                                }
                                // Vorschau der gedrehten Auswahl
                                let m = Affine::rotate_about(c, angle.to_radians());
                                let rctx = render::RenderCtx::new(app.doc());
                                let mut dl = DisplayList::default();
                                for id in app.selection().to_vec() {
                                    if let Some(e) = app.doc().entity(id) {
                                        render::render_entity(&rctx, e, m, &mut dl, 0);
                                    }
                                }
                                let pre = Stroke::new(1.1, Palette::SELECT.gamma_multiply(0.85));
                                for prim in &dl.prims {
                                    ghost_shape(p, &prim.shape, &w2s, view.zoom, pre);
                                }
                                label = format!("Drehen {}°", format_number(angle, 1));
                            }
                            None => {
                                // Bis-Punkt liegt auf dem Zentrum: keine Richtung
                                label = "bis: Abstand zum Drehzentrum nötig".into();
                            }
                        }
                    }
                }
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
// Vorschau beim Verschieben/Kopieren
// ---------------------------------------------------------------------------

/// Zeichnet die Auswahl als Akzentkontur an der Zielposition.
///
/// Grundlage ist dieselbe Displayliste wie beim Platzieren
/// ([`render::preview_selection`]) — dadurch bekommt **jeder** Elementtyp eine
/// Vorschau (Linie, Wand, Tür, Fenster, Text, Maß, Block …), nicht nur Linien.
fn draw_selection_preview(
    app: &mut App,
    p: &Painter,
    rect: Rect,
    view: View,
    ids: &[Id],
    delta: V2,
) {
    if ids.is_empty() || delta.len() < 1e-9 {
        return;
    }
    let dl = render::preview_selection(app.doc(), ids, delta);
    if dl.prims.is_empty() {
        return;
    }
    let paper_px = view.zoom * app.doc().settings.annotation_scale;
    let pc = PaintCtx {
        selected: Vec::new(),
        hover: None,
        lineweights: true,
        lineweights_scaled: app.ui.lineweights_scaled,
        paper_px,
        preview: true,
    };
    let w2s = |q: V2| world_to_screen(rect, view, q);
    for prim in &dl.prims {
        paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
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
    let (ignore, keep) = snap_ignore_keep(app);
    if let Some(hit) = snap::find_snap(app.doc(), w, tol, reference, ignore, keep) {
        w = hit.point;
    }
    w
}

/// Fang-Ausschluss und Fang-Auswahl für den aktuellen Zustand.
///
/// Normalerweise ist die Auswahl ausdrücklich *erlaubt* (Ecken/Endpunkte), und
/// nichts ist ausgeschlossen. Während die Auswahl selbst bewegt wird (Ziehen
/// mit der Maus, Griff, Verschieben/Kopieren ab dem Basispunkt), fällt sie
/// dagegen ganz aus dem Fang, weil ihre Ecken in diesem Moment mit dem Zeiger
/// wandern — sonst klebt die Bewegung am Startpunkt.
fn snap_ignore_keep(app: &App) -> (&[Id], &[Id]) {
    let auswahl_wandert = app.move_drag.is_some()
        || app.grip_drag.is_some()
        || app
            .tool
            .as_ref()
            .map(|t| {
                matches!(t.action.as_str(), "modify.move" | "modify.copy") && !t.pts.is_empty()
            })
            .unwrap_or(false);
    if auswahl_wandert {
        (app.selection(), &[])
    } else {
        (&[], app.selection())
    }
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

    // Enter beendet Polylinie, Umriss-Fläche und Maßkette (Tippeingabe hat Vorrang)
    if app.ui.typed.is_empty()
        && !ctx.wants_keyboard_input()
        && ctx.input(|i| i.key_pressed(egui::Key::Enter))
    {
        if let Some(t) = app.tool.clone() {
            let punktfolge = ist_fuell_punkte(&t.action);
            if matches!(t.action.as_str(), "draw.polyline" | "dim.chain") || punktfolge {
                if t.pts.len() >= 2 {
                    finish_tool(app, true);
                    return;
                }
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

    // Laufende Drags zuerst: sie müssen auch dann abschließen, wenn der Zeiger
    // die Zeichenfläche verlässt (Loslassen außerhalb).
    if laufender_drag(app, resp, snapped, raw) {
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

    // Rechtsklick: Kontextmenü für ein Objekt oder die Fläche.
    // Während ein Werkzeug läuft, beendet der Rechtsklick das Werkzeug
    // (Polylinie, Umriss-Fläche, Maßkette) — dann kein Menü darüberlegen.
    if app.tool.is_none() && resp.clicked_by(egui::PointerButton::Secondary) {
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
                // Füllflächen aus Punkten: Rechtsklick schließt wie Enter. Bei
                // zu wenigen Punkten gibt es nichts zu schließen — dann bricht
                // er das Werkzeug ab (für beide Füll-Werkzeuge gleich).
                action if ist_fuell_punkte(action) => {
                    if tool.pts.len() >= 3 {
                        finish_tool(app, true);
                    } else {
                        app.tool = None;
                        app.status_msg = "Abgebrochen".into();
                    }
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

/// Setzt eine laufende Maus-Verschiebung bzw. einen Griff-Drag fort und beendet
/// sie beim Loslassen. Liefert `true`, wenn ein Drag läuft — die übrige
/// Zeigerauswertung wird dann übersprungen.
fn laufender_drag(app: &mut App, resp: &Response, snapped: Option<V2>, raw: Option<V2>) -> bool {
    if app.tool.is_some() {
        return false;
    }
    let zwei_klicks = app.ui.drag_mode.two_clicks();
    if let Some(md) = app.move_drag.as_ref() {
        // Ohne Zeigerposition über der Fläche bleibt die letzte Lage stehen
        // (Zeiger außerhalb): der Drag läuft weiter, bis die Taste loskommt.
        let cur = snapped.or(raw).unwrap_or(md.cur);
        let start_frame = md.frame;
        app.update_move_drag(cur);
        let frame = resp.ctx.input(|i| i.time);
        if fertig_erkennen(resp, zwei_klicks, start_frame, frame) {
            app.end_move_drag();
        }
        return true;
    }
    if app.grip_drag.is_some() {
        let start_frame = app.grip_start_frame;
        if let Some(pt) = snapped.or(raw) {
            app.update_grip_drag(pt);
        }
        let frame = resp.ctx.input(|i| i.time);
        if fertig_erkennen(resp, zwei_klicks, start_frame, frame) {
            app.end_grip_drag();
        }
        return true;
    }
    false
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

/// Füll-Werkzeuge, deren Umgrenzung als Punktfolge geklickt wird.
///
/// `draw.fill.boundary` (Umriss abfahren) und `draw.fill.points` (freie
/// Punktfolge) unterscheiden sich nur im Einstieg — Klicks, Vorschau, Abschluss,
/// Undo und Platzieren-Panel teilen sie sich. Ein zweiter Zweig pro Werkzeug
/// würde die beiden Werkzeuge früher oder später auseinanderlaufen lassen.
pub fn ist_fuell_punkte(action: &str) -> bool {
    matches!(action, "draw.fill.boundary" | "draw.fill.points")
}

/// Eckpunkte einer Füllfläche aus den gesammelten Klicks.
///
/// Doppelte Punkte fallen weg (zweimal auf dieselbe Stelle geklickt,
/// Doppelklick am letzten Punkt), und ein Schlusspunkt auf dem Startpunkt
/// schließt den Umriss, statt eine vierte Ecke zu erzeugen. Weniger als drei
/// Ecken ergeben keine Fläche.
pub fn fill_boundary_points(pts: &[V2]) -> Option<Vec<V2>> {
    let mut out: Vec<V2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().map(|q: &V2| q.dist(*p) > 0.5).unwrap_or(true) {
            out.push(*p);
        }
    }
    if out.len() > 1 && out[0].dist(*out.last().unwrap()) <= 0.5 {
        out.pop();
    }
    if out.len() >= 3 {
        Some(out)
    } else {
        None
    }
}

/// Vorschau der aufgezogenen Füllfläche: halbtransparente Fläche unter dem
/// Polygonzug plus das echte Muster aus den Werkzeugoptionen.
///
/// Das Muster kommt aus [`render::render_fill`] — derselben Funktion, die das
/// fertige Objekt zeichnet; die Fläche selbst wird trianguliert eingefärbt
/// (egui füllt nur konvexe Polygone selbst, ein Umriss darf konkav sein).
fn draw_fill_ghost(app: &App, p: &Painter, rect: Rect, view: View, umriss: &[V2]) {
    let Some(boundary) = fill_boundary_points(umriss) else {
        return;
    };
    let w2s = |q: V2| world_to_screen(rect, view, q);

    // Fläche andeuten — der Plan darunter bleibt lesbar
    let screen: Vec<Pos2> = boundary.iter().map(|q| w2s(*q)).collect();
    if !fill_too_thin(&screen) {
        let c = Palette::ACCENT.gamma_multiply(0.14);
        for tri in triangulate(&screen) {
            p.add(Shape::convex_polygon(tri.to_vec(), c, Stroke::NONE));
        }
    }

    // Muster wie beim fertigen Objekt
    let mut e = Entity::new(
        0,
        app.doc().current_layer,
        EntityKind::Fill {
            boundary,
            holes: Vec::new(),
            pattern: app.options.fill_pattern,
            pattern_angle: app.options.fill_angle,
            pattern_scale: app.options.fill_scale,
            border: false,
        },
    );
    e.style.fill = app.ui.pending.fill;
    let mut dl = DisplayList::default();
    render::render_entity(
        &render::RenderCtx::new(app.doc()),
        &e,
        Affine::IDENTITY,
        &mut dl,
        0,
    );
    let pc = PaintCtx {
        selected: Vec::new(),
        hover: None,
        lineweights: true,
        lineweights_scaled: app.ui.lineweights_scaled,
        paper_px: view.zoom * app.doc().settings.annotation_scale,
        preview: true,
    };
    for prim in &dl.prims {
        paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
    }
}

/// Schließt eine Füllfläche aus der geklickten Punktfolge.
///
/// Gemeinsamer Abschluss der Füll-Werkzeuge: baut die Eckpunkte mit
/// [`fill_boundary_points`] (doppelte Punkte, Schluss auf dem Start), nimmt
/// Muster, Winkel, Abstand und Randlinie aus dem Platzieren-Panel und ruft die
/// Action `action` — `draw.fill.boundary` bzw. `draw.fill.points`. Beide landen
/// im Kern in `actions::add_fill`, also entstehen dasselbe Objekt, dasselbe
/// Undo und derselbe Panel-Zustand.
///
/// `true`, wenn eine Fläche angelegt wurde. Bei zu wenigen Ecken bleibt das
/// Werkzeug stehen (der gesetzte Punkt ist noch nicht verloren, ESC verwirft).
fn finish_fill(app: &mut App, action: &str) -> bool {
    let Some(tool) = app.tool.clone() else {
        return false;
    };
    let Some(boundary) = fill_boundary_points(&tool.pts) else {
        app.toast("Mindestens drei Punkte für eine Fläche");
        return false;
    };
    let pts: Vec<[f64; 2]> = boundary
        .iter()
        .map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)])
        .collect();
    let ok = run(
        app,
        action,
        json!({"boundary": pts,
               "pattern": app.options.fill_pattern.key(),
               "angle": app.options.fill_angle.to_degrees(),
               "scale": app.unit().from_mm(app.options.fill_scale),
               "border": app.options.fill_border}),
    );
    if ok {
        app.tool = None;
    }
    ok
}

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
        "draw.fill.boundary" | "draw.fill.points" => {
            // Ein Abschluss für beide Eingaben; `action` bestimmt nur, welche
            // Action den Kern aufruft (MCP und Klickpfad landen im selben Code).
            finish_fill(app, &tool.action);
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

/// Ergebnis eines Klicks im Dreh-Werkzeug (Drehzentrum → „von" → „bis").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RotateStep {
    /// Punkt übernehmen (Drehzentrum bzw. „von"-Punkt) und weiter sammeln.
    Collect { point: V2 },
    /// Winkel steht: die Auswahl um `angle` Grad um `center` drehen.
    Apply {
        center: V2,
        from: V2,
        angle: f64,
    },
    /// Klick ist unbrauchbar (zu nah am Drehzentrum, Richtung unbestimmt).
    Reject(&'static str),
}

/// Wertet einen Klick der Zwei-Klick-Winkelwahl aus — ohne Seiteneffekte,
/// damit die Reihenfolge Zentrum → „von" → „bis" prüfbar ist. Erst der dritte
/// Klick liefert einen Winkel: die Differenz der beiden vom Zentrum aus
/// gesehenen Richtungen (mathematisch positiv), nicht die Richtung des
/// Klicks gegenüber einer festen Achse.
pub fn rotate_step(pts: &[V2], w: V2) -> RotateStep {
    if pts.len() < 2 {
        if let Some(c) = pts.first().copied() {
            if c.dist(w) < jjcad_core::transform::ANGLE_MIN_RADIUS {
                return RotateStep::Reject("Etwas Abstand zum Drehzentrum nötig");
            }
        }
        return RotateStep::Collect { point: w };
    }
    match jjcad_core::transform::angle_between_directions(pts[0], pts[1], w) {
        Some(angle) => RotateStep::Apply {
            center: pts[0],
            from: pts[1],
            angle,
        },
        None => RotateStep::Reject("Bis-Punkt braucht Abstand zum Drehzentrum"),
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
        // Umriss abfahren: derselbe Klickpfad wie die Polylinie — gesammelt
        // werden Punkte, nur das Ende erzeugt eine Füllfläche statt einer Linie.
        // Gilt für beide Füll-Werkzeuge (`draw.fill.boundary` und
        // `draw.fill.points`), damit sich die Eingabe nicht unterscheidet.
        "draw.fill.boundary" | "draw.fill.points" => {
            // Klick auf den letzten Punkt (Doppelklick) oder auf den Startpunkt
            // schließt den Umriss — wie Enter, nur mit der Maus.
            let doppelt = tool.pts.last().map(|q| q.dist(w) <= 0.5).unwrap_or(false);
            let am_start = tool.pts.first().map(|q| q.dist(w) <= 0.5).unwrap_or(false);
            if tool.pts.len() >= 3 && (doppelt || am_start) {
                finish_tool(app, true);
            } else {
                tool.pts.push(w);
                app.tool = Some(tool);
            }
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
        "draw.fill" | "draw.hatch" => {
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
                            let _ = run(
                                app,
                                "draw.fill",
                                json!({"boundary": pts.iter().map(|q| ax(*q)).collect::<Vec<_>>(),
                                       "pattern": app.options.fill_pattern.key(),
                                       "angle": app.options.fill_angle.to_degrees(),
                                       "scale": unit.from_mm(app.options.fill_scale),
                                       "border": app.options.fill_border}),
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
            // Drei Klicks: Drehzentrum, „von"-Punkt, „bis"-Punkt. Der Winkel ist
            // die Differenz der beiden vom Zentrum aus gesehenen Richtungen —
            // nicht mehr die Richtung eines Klicks zur X-Achse.
            match rotate_step(&tool.pts, w) {
                RotateStep::Collect { point } => {
                    tool.pts.push(point);
                    app.tool = Some(tool);
                }
                RotateStep::Apply {
                    center,
                    from: _,
                    angle,
                } => {
                    let ids = app.selection().to_vec();
                    let _ = run(
                        app,
                        "modify.rotate",
                        json!({"ids": ids, "center": ax(center), "angle": angle}),
                    );
                    app.toast(&format!("Gedreht um {}°", format_number(angle, 1)));
                    app.tool = None;
                }
                RotateStep::Reject(msg) => app.toast(msg),
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


#[cfg(test)]
mod tests {
    use super::*;
    use jjcad_core::style::FillPattern;

    // ---- Umriss-Füllfläche (Karte F2) ----

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    fn ecken(app: &App) -> Vec<V2> {
        app.doc()
            .entities
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Fill { boundary, .. } => Some(boundary.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn fill_kind(app: &App) -> Option<EntityKind> {
        app.doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Fill { .. }))
            .map(|e| e.kind.clone())
    }

    /// Drei Klicks und Enter ergeben genau eine Füllfläche mit drei Ecken und
    /// der richtigen Fläche — über denselben Klickpfad wie die Polylinie.
    #[test]
    fn drei_klicks_und_enter_erzeugen_eine_flaeche() {
        let mut app = app();
        app.options.fill_pattern = FillPattern::Cross;
        app.options.fill_angle = 0.0;
        app.options.fill_scale = 200.0;
        app.options.fill_border = true;
        app.start_tool("draw.fill.boundary");
        assert_eq!(app.tool_id(), Some("draw.fill.boundary"));

        // 2 m × 1 m Dreieck in mm
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 1000.0));
        assert_eq!(app.doc().entities.len(), 0, "während des Klickens nichts anlegen");

        finish_tool(&mut app, true);
        assert_eq!(app.doc().entities.len(), 1, "genau ein Objekt");
        let ecken = ecken(&app);
        assert_eq!(ecken.len(), 3, "drei Eckpunkte");
        let flaeche = jjcad_core::geom::polygon_area(&ecken).abs();
        assert!(
            (flaeche - 1_000_000.0).abs() < 1e-6,
            "Fläche 1 m², ist {flaeche} mm²"
        );

        let Some(EntityKind::Fill {
            pattern,
            pattern_scale,
            border,
            ..
        }) = fill_kind(&app)
        else {
            panic!("kein Fill angelegt");
        };
        assert_eq!(pattern, FillPattern::Cross, "Muster aus den Werkzeugoptionen");
        assert!((pattern_scale - 200.0).abs() < 1e-9, "Abstand aus den Optionen");
        assert!(border);
        assert!(app.tool.is_none(), "Werkzeug beendet");
    }

    /// Klick auf den Startpunkt schließt den Umriss (Maus statt Enter), ohne
    /// eine vierte Ecke zu erzeugen; ein Doppelklick auf den letzten Punkt
    /// ebenso.
    #[test]
    fn klick_auf_start_schliesst_den_umriss() {
        let mut app = app();
        app.start_tool("draw.fill.boundary");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));
        tool_click(&mut app, V2::new(0.0, 0.0)); // zurück auf den Start
        assert_eq!(app.doc().entities.len(), 1);
        assert_eq!(ecken(&app).len(), 3);
        assert!(app.tool.is_none());
    }

    #[test]
    fn doppelklick_auf_den_letzten_punkt_schliesst() {
        let mut app = app();
        app.start_tool("draw.fill.boundary");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 1000.0));
        tool_click(&mut app, V2::new(0.0, 1000.0)); // derselbe Punkt nochmal
        assert_eq!(app.doc().entities.len(), 1);
        assert_eq!(ecken(&app).len(), 3, "Doppelklick zählt keine Ecke");
    }

    /// ESC verwirft: kein Objekt, Werkzeug beendet.
    #[test]
    fn esc_verwirft_den_umriss() {
        let mut app = app();
        app.start_tool("draw.fill.boundary");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 1000.0));
        app.cancel_tool();
        assert!(app.tool.is_none(), "Werkzeug beendet");
        assert!(app.doc().entities.is_empty(), "nichts angelegt");
    }

    /// Zu wenige Punkte: Enter legt nichts an, der gesammelte Umriss bleibt
    /// stehen (der Nutzer kann weiterklicken).
    #[test]
    fn zwei_punkte_und_enter_legen_nichts_an() {
        let mut app = app();
        app.start_tool("draw.fill.boundary");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        finish_tool(&mut app, true);
        assert!(app.doc().entities.is_empty(), "keine Fläche aus zwei Punkten");
        assert_eq!(
            app.tool.as_ref().map(|t| t.pts.len()),
            Some(2),
            "Punkte bleiben erhalten"
        );
    }

    /// Punktliste: Doppelte fallen weg, Schlusspunkt auf dem Start schließt.
    #[test]
    fn punktliste_entdoppelt_und_schliesst() {
        let a = V2::new(0.0, 0.0);
        let b = V2::new(2000.0, 0.0);
        let c = V2::new(0.0, 1000.0);
        assert!(fill_boundary_points(&[a, b]).is_none(), "zwei Ecken reichen nicht");
        assert_eq!(fill_boundary_points(&[a, b, c]).unwrap().len(), 3);
        assert_eq!(
            fill_boundary_points(&[a, b, c, a]).unwrap().len(),
            3,
            "Schlusspunkt auf dem Start ist keine vierte Ecke"
        );
        assert_eq!(
            fill_boundary_points(&[a, b, c, c]).unwrap().len(),
            3,
            "Doppelklick-Punkt fällt weg"
        );
        assert!(
            fill_boundary_points(&[a, b, b, c]).unwrap().len() == 3,
            "doppelter Zwischenpunkt fällt weg"
        );
    }

    // ---- Füllfläche aus freier Punktfolge (Karte F3) ----

    /// Vier Punkte + Enter: ein Fill mit vier Ecken und 12 m² — Werte aus dem
    /// Platzieren-Panel, Undo nimmt genau dieses Objekt zurück.
    #[test]
    fn vier_punkte_und_enter_erzeugen_vier_ecken() {
        let mut app = app();
        app.options.fill_pattern = FillPattern::Insulation;
        app.options.fill_angle = 30.0_f64.to_radians();
        app.options.fill_scale = 120.0;
        app.options.fill_border = true;
        app.start_tool("draw.fill.points");
        assert_eq!(app.tool_id(), Some("draw.fill.points"));

        // 4 m × 3 m Rechteck in mm
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(4000.0, 0.0));
        tool_click(&mut app, V2::new(4000.0, 3000.0));
        tool_click(&mut app, V2::new(0.0, 3000.0));
        assert_eq!(app.doc().entities.len(), 0, "während des Klickens nichts anlegen");

        finish_tool(&mut app, true);
        assert_eq!(app.doc().entities.len(), 1, "genau ein Objekt");
        let ecken = ecken(&app);
        assert_eq!(ecken.len(), 4, "vier Eckpunkte");
        let flaeche = jjcad_core::geom::polygon_area(&ecken).abs();
        assert!(
            (flaeche - 12_000_000.0).abs() < 1e-6,
            "Fläche 12 m², ist {flaeche} mm²"
        );

        let Some(EntityKind::Fill {
            pattern,
            pattern_angle,
            pattern_scale,
            border,
            ..
        }) = fill_kind(&app)
        else {
            panic!("kein Fill angelegt");
        };
        assert_eq!(pattern, FillPattern::Insulation, "Muster aus dem Panel");
        assert!((pattern_angle - 30.0_f64.to_radians()).abs() < 1e-9);
        assert!((pattern_scale - 120.0).abs() < 1e-9, "Abstand aus dem Panel");
        assert!(border, "Randlinie aus dem Panel");
        assert!(app.tool.is_none(), "Werkzeug beendet");

        // Undo: genau ein Schritt nimmt die Fläche zurück
        assert!(app.session_mut().undo(), "Rückgängig muss greifen");
        assert!(app.doc().entities.is_empty(), "Fläche wieder weg");
    }

    /// Dieselben Klicks über beide Füll-Werkzeuge ergeben dasselbe Objekt —
    /// der gemeinsame Abschluss ist der Grund dafür.
    #[test]
    fn punktfolge_und_umriss_ergeben_dasselbe_objekt() {
        let klicks = [
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1500.0),
            V2::new(700.0, 2200.0),
        ];

        let mut a = app();
        a.options.fill_pattern = FillPattern::Dots;
        a.options.fill_scale = 80.0;
        a.start_tool("draw.fill.boundary");
        for w in klicks {
            tool_click(&mut a, w);
        }
        finish_tool(&mut a, true);

        let mut b = app();
        b.options.fill_pattern = FillPattern::Dots;
        b.options.fill_scale = 80.0;
        b.start_tool("draw.fill.points");
        for w in klicks {
            tool_click(&mut b, w);
        }
        finish_tool(&mut b, true);

        assert_eq!(a.doc().entities.len(), 1);
        assert_eq!(b.doc().entities.len(), 1);
        assert_eq!(
            a.doc().entities[0].kind,
            b.doc().entities[0].kind,
            "gleiche Ecken und gleiches Muster"
        );
        assert_eq!(a.doc().entities[0].style, b.doc().entities[0].style);
    }

    /// Doppelklick auf den letzten Punkt schließt die Punktfolge, ohne eine
    /// fünfte Ecke zu erzeugen.
    #[test]
    fn doppelklick_schliesst_die_punktfolge() {
        let mut app = app();
        app.start_tool("draw.fill.points");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0)); // derselbe Punkt nochmal
        assert_eq!(app.doc().entities.len(), 1);
        assert_eq!(ecken(&app).len(), 3, "Doppelklick zählt keine Ecke");
        assert!(app.tool.is_none());
    }

    /// ESC verwirft die Punktfolge: kein Objekt, Werkzeug beendet.
    #[test]
    fn esc_verwirft_die_punktfolge() {
        let mut app = app();
        app.start_tool("draw.fill.points");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));
        app.cancel_tool();
        assert!(app.tool.is_none());
        assert!(app.doc().entities.is_empty(), "nichts angelegt");
    }

    /// Der gemeinsame Abschluss greift für beide Füll-Werkzeuge — nicht nur
    /// für eines (Schutz gegen einen zweiten, abweichenden Zweig).
    #[test]
    fn beide_fuell_werkzeuge_teilen_den_abschluss() {
        assert!(ist_fuell_punkte("draw.fill.boundary"));
        assert!(ist_fuell_punkte("draw.fill.points"));
        assert!(!ist_fuell_punkte("draw.fill"));
        assert!(!ist_fuell_punkte("draw.polyline"));
    }

    /// Zwei-Klick-Winkelwahl: Zentrum → „von" → „bis", Winkel zwischen den
    /// beiden Richtungen (nicht die Richtung zur X-Achse).
    #[test]
    fn drehwerkzeug_fragt_zentrum_dann_von_dann_bis() {
        let c = V2::new(0.0, 0.0);
        let von = V2::new(1000.0, 1000.0); // 45°-Richtung
        let bis = V2::new(0.0, 1000.0); // 90°-Richtung

        // 1. Klick: Drehzentrum wird nur gesammelt
        assert!(matches!(
            rotate_step(&[], c),
            RotateStep::Collect { .. }
        ));
        // 2. Klick: „von" wird gesammelt, noch kein Winkel
        assert!(matches!(
            rotate_step(&[c], von),
            RotateStep::Collect { .. }
        ));
        // 3. Klick: Winkel = 90° - 45° = 45°, nicht 90°
        match rotate_step(&[c, von], bis) {
            RotateStep::Apply {
                center,
                angle,
                ..
            } => {
                assert_eq!(center, c);
                assert!((angle - 45.0).abs() < 1e-9, "{angle}");
            }
            other => panic!("Winkel erwartet, war {other:?}"),
        }
        // Gegenrichtung: 45° → 0° ergibt -45°
        match rotate_step(&[c, von], V2::new(1000.0, 0.0)) {
            RotateStep::Apply { angle, .. } => assert!((angle + 45.0).abs() < 1e-9, "{angle}"),
            other => panic!("Winkel erwartet, war {other:?}"),
        }
        // Vollkreis-Gegenprobe: 0° → 180° sind 180°, nicht 0°
        match rotate_step(&[c, V2::new(1000.0, 0.0)], V2::new(-1000.0, 0.0)) {
            RotateStep::Apply { angle, .. } => assert!((angle.abs() - 180.0).abs() < 1e-9, "{angle}"),
            other => panic!("Winkel erwartet, war {other:?}"),
        }
        // Klick auf das Zentrum ist keine gültige Richtung
        assert!(matches!(rotate_step(&[c], c), RotateStep::Reject(_)));
        assert!(matches!(
            rotate_step(&[c, von], c),
            RotateStep::Reject(_)
        ));
    }
}
