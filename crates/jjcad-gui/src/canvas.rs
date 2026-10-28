//! Zeichnungsfläche: Anzeige, Fang, Werkzeugbedienung, Navigation.

use crate::app::{
    screen_to_world, visible_world_bounds, world_to_screen, App, DrawCacheKey, Einfuegemodus,
    TabKind, Tool, ToolPhase, View,
};
use crate::theme::Palette;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::boundary::{detect_boundary, BoundaryError, BoundaryOpts, FillBoundary};
use jjcad_core::geom::*;
use jjcad_core::geom::Vec2 as V2;
use jjcad_core::model::*;
use jjcad_core::render::{self, DisplayList, Prim, Shape as PShape};
use jjcad_core::snap;
use jjcad_core::tessellate::CurveQuality;
use jjcad_core::units::{format_number, format_scale};
use serde_json::json;

const PAPER_PX_PER_MM: f64 = 3.2;

// ---------------------------------------------------------------------------
// Anzeige
// ---------------------------------------------------------------------------

#[derive(Clone)]
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
    /// Drehung der Ansicht (Bogenmaß, mathematisch positiv). Texte drehen mit
    /// der Zeichnung; Linien und Flächen stecken schon in `w2s`.
    rot: f32,
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
            // Die Dreiecke als **ein Mesh** zeichnen, nicht als einzelne
            // `convex_polygon`-Formen: egui glättet jede Form mit rund 1 px
            // Kantenbreite. Bei den sehr schmalen Dreiecken, die eine
            // getreppte Rasterkontur mit Löchern zwangsläufig ergibt, wird die
            // Gehrung der beiden Längskanten ein Vielfaches länger als das
            // Dreieck — sichtbar als lange diagonale Streifen und als Fläche
            // weit außerhalb des Umrisses. Ein Mesh wird ohne Glättung
            // gezeichnet: keine Spikes und keine doppelt geblendeten
            // Innenkanten.
            let col = c.gamma_multiply(alpha);
            let mut mesh = egui::Mesh::default();
            for tri in triangulate(&screen) {
                let base = mesh.vertices.len() as u32;
                for q in tri {
                    mesh.colored_vertex(q, col);
                }
                mesh.add_triangle(base, base + 1, base + 2);
            }
            if !mesh.is_empty() {
                p.add(Shape::mesh(mesh));
            }
        }
        PShape::Circle { c, r } => {
            p.circle_stroke(w2s(*c), (*r * scale) as f32, stroke);
            center_mark(p, w2s(*c), col);
        }
        PShape::Arc { c, r, a0, a1 } => {
            let arc = Arc::new(*c, *r, *a0, *a1);
            if *r * scale < 0.4 {
                // Degenerierter bzw. auf dem Schirm punktförmiger Bogen.
                p.circle_filled(w2s(*c), (w as f32 * 0.5).max(0.5), col);
                return;
            }
            let pts: Vec<Pos2> = CurveQuality::SCREEN
                .flatten(&arc, scale)
                .iter()
                .map(|q| w2s(*q))
                .collect();
            // Ein **durchgehender** Pfad statt einer Kapsel je Sehne: bei einer
            // Kapsel je Segment überlagern sich an jedem Stoß zwei
            // Kantenglättungen, sichtbar als dunkle Knicke. Der Pfad bekommt
            // genau eine Glättung; die runden Enden liefern zwei Kreise.
            if pts.len() >= 2 {
                let (anfang, ende) = (pts.first().copied(), pts.last().copied());
                p.add(Shape::line(pts, stroke));
                let rr = (w as f32 * 0.5).max(0.4);
                if let (Some(a), Some(b)) = (anfang, ende) {
                    p.circle_filled(a, rr, col);
                    p.circle_filled(b, rr, col);
                }
            }
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
        PShape::Text(t) => paint_text(p, t, w2s(t.pos), scale, col, pc.rot),
    }
}

/// Text an einem Bildschirmpunkt zeichnen — die **eine** Stelle, an der Text
/// platziert wird.
///
/// Gezeichnet wird die Vorschau des Textwerkzeugs über denselben Weg; sonst
/// stünde sie an einer anderen Stelle als der Text, der beim Klick entsteht
/// (Vorschau mit Grundlinie am Zeiger, Ergebnis mittig — der Text sprang beim
/// Einfügen nach unten).
/// Text in der Zeichenfläche. `rot` ist die Drehung der Ansicht: der Text
/// dreht mit der Zeichnung, seine Ausrichtung (links/mittig/rechts, oben/mittig/
/// unten) wird dabei mitgedreht — sonst rutschte der Anker beim Drehen weg.
fn paint_text(
    p: &Painter,
    t: &render::TextPrim,
    base: Pos2,
    scale: f64,
    col: Color32,
    rot: f32,
) {
    let size = (t.height * scale * 1.32).max(3.0) as f32;
    let font = FontId::proportional(size);
    // Mehrzeilig: `\n` trennt die Zeilen, und die waagerechte Ausrichtung gilt
    // je Zeile — sonst stünde ein mittig ausgerichteter Text linksbündig im
    // Rahmen (`layout_no_wrap` setzt immer links).
    let mut job = egui::text::LayoutJob::simple(t.text.clone(), font, col, f32::INFINITY);
    job.halign = match t.align {
        TextAlignH::Left => egui::Align::LEFT,
        TextAlignH::Center => egui::Align::Center,
        TextAlignH::Right => egui::Align::RIGHT,
    };
    let galley = p.layout_job(job);
    let sz = galley.size();
    let (dx, dy) = match t.align {
        TextAlignH::Left => (0.0, 0.0),
        TextAlignH::Center => (-sz.x / 2.0, 0.0),
        TextAlignH::Right => (-sz.x, 0.0),
    };
    let vy = match t.valign {
        TextAlignV::Top => 0.0,
        TextAlignV::Middle => -sz.y / 2.0,
        TextAlignV::Bottom => -sz.y,
    };
    let winkel = text_winkel(t.angle, rot);
    if winkel.abs() < 0.001 {
        let pos = Pos2::new(base.x + dx, base.y + dy + vy);
        p.galley(pos, galley, col);
    } else {
        let (sin, cos) = winkel.sin_cos();
        let u = dy + vy;
        let off = Vec2::new(dx * cos - u * sin, dx * sin + u * cos);
        let pos = base + off;
        let shape = egui::epaint::TextShape::new(pos, galley, col).with_angle(winkel);
        p.add(shape);
    }
}

/// Winkel, mit dem egui den Text dreht (Radiant, **im Uhrzeigersinn**).
///
/// Die Zeichnung dreht mathematisch positiv (gegen den Uhrzeigersinn) — der
/// Textwinkel selbst **und** die Ansichtsdrehung. egui zählt andersherum,
/// deshalb das negative Vorzeichen für beide: mit `t.angle - rot` kippte ein
/// gedrehter Text in die Gegenrichtung (T0025, „Rotieren ist falschherum“).
pub(crate) fn text_winkel(angle: f64, rot: f32) -> f32 {
    -(angle as f32 + rot)
}

/// Text, den das Textwerkzeug beim Klick anlegen wird — genau die Vorgaben der
/// Action `draw.text` (links, mittig, ohne Drehung). Die Vorschau muss diesen
/// Text zeichnen, damit sie dieselbe Lage zeigt wie das Ergebnis.
pub(crate) fn text_preview_prim(text: String, height: f64) -> render::TextPrim {
    render::TextPrim {
        pos: V2::ZERO,
        text,
        height,
        angle: 0.0,
        align: TextAlignH::Left,
        valign: TextAlignV::Middle,
        bold: false,
    }
}

#[cfg(test)]
mod tests_ansichtsdrehung {
    //! Drehung der Zeichnungsansicht (T0017): gerechnet wird in
    //! `world_to_screen`/`screen_to_world`; das Drehzentrum bleibt beim Drehen
    //! an seiner Bildschirmposition, und das Werkzeug „Ansicht drehen" nimmt
    //! Zentrum, Bezugspunkt und Zielrichtung in drei Klicks.

    use super::*;
    use egui::{Pos2, Rect, Vec2};

    fn flaeche() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(1000.0, 800.0))
    }

    fn ansicht() -> View {
        View {
            center: V2::new(2000.0, 1500.0),
            zoom: 0.1,
            rot: 0.0,
        }
    }

    /// Das angeklickte Drehzentrum bleibt nach dem Drehen an derselben
    /// Bildschirmstelle — sonst springt die Zeichnung unter dem Zeiger weg.
    #[test]
    fn drehzentrum_bleibt_auf_dem_schirm() {
        let rect = flaeche();
        let mut view = ansicht();
        let zentrum = V2::new(1500.0, 1200.0);
        let vorher = world_to_screen(rect, view, zentrum);
        view.rotate_about(zentrum, 37f64.to_radians());
        let nachher = world_to_screen(rect, view, zentrum);
        assert!(
            (vorher - nachher).length() < 0.01,
            "Zentrum wandert um {:?} px",
            vorher - nachher
        );
        assert!((view.rot - 37f64.to_radians()).abs() < 1e-12);
    }

    /// Positive Drehung dreht die Zeichnung gegen den Uhrzeigersinn: +X zeigt
    /// danach nach oben, +Y nach links.
    #[test]
    fn positive_drehung_geht_gegen_den_uhrzeigersinn() {
        let rect = flaeche();
        let mut view = ansicht();
        view.rot = 90f64.to_radians();
        let mitte = world_to_screen(rect, view, view.center);
        let x_ende = world_to_screen(rect, view, view.center + V2::new(1000.0, 0.0));
        let y_ende = world_to_screen(rect, view, view.center + V2::new(0.0, 1000.0));
        assert!((x_ende.x - mitte.x).abs() < 0.01 && x_ende.y < mitte.y - 10.0,
            "+X muss bei 90° nach oben zeigen, ist {x_ende:?}");
        assert!((y_ende.y - mitte.y).abs() < 0.01 && y_ende.x < mitte.x - 10.0,
            "+Y muss bei 90° nach links zeigen, ist {y_ende:?}");
    }

    /// Hin- und Rückrechnung sind auch bei gedrehter Ansicht exakt.
    #[test]
    fn hin_und_rueckrechnung_stimmen() {
        let rect = flaeche();
        let mut view = ansicht();
        view.rot = 0.7;
        for q in [
            V2::new(0.0, 0.0),
            V2::new(1234.0, -567.0),
            V2::new(5000.0, 4000.0),
        ] {
            let s = world_to_screen(rect, view, q);
            let w = screen_to_world(rect, view, s);
            // Bildschirmkoordinaten sind f32 — auf 0,01 mm genau reicht.
            assert!((w - q).len() < 0.01, "{q:?} → {s:?} → {w:?}");
        }
    }

    /// Die Rasterhülle umfasst bei 90° die gedrehten Bildschirmecken.
    #[test]
    fn rasterhuelle_umfasst_die_gedrehte_flaeche() {
        let rect = flaeche();
        let mut view = ansicht();
        let (x0, y0, x1, y1) = visible_world_bounds(rect, view);
        assert!((x1 - x0 - 10_000.0).abs() < 1.0, "Breite ist {}", x1 - x0);
        assert!((y1 - y0 - 8_000.0).abs() < 1.0, "Höhe ist {}", y1 - y0);

        view.rot = 90f64.to_radians();
        let (x0, y0, x1, y1) = visible_world_bounds(rect, view);
        assert!((x1 - x0 - 8_000.0).abs() < 1.0, "bei 90° Breite {}", x1 - x0);
        assert!((y1 - y0 - 10_000.0).abs() < 1.0, "bei 90° Höhe {}", y1 - y0);
    }

    /// Drei Klicks (Zentrum, Bezugspunkt, Zielrichtung) drehen die Ansicht um
    /// den Winkel zwischen den beiden Richtungen — und beenden das Werkzeug.
    #[test]
    fn werkzeug_dreht_die_ansicht_um_den_winkel() {
        let mut app = App::with_ctx(egui::Context::default());
        app.start_tool("view.rotate_tool");
        assert_eq!(app.tool_id(), Some("view.rotate_tool"));

        let rect = flaeche();
        let zentrum = V2::new(2000.0, 1500.0);
        let vorher = world_to_screen(rect, app.current_view(), zentrum);

        tool_click(&mut app, zentrum);
        tool_click(&mut app, zentrum + V2::new(2000.0, 0.0)); // Bezug: 0°
        assert_eq!(app.current_view().rot, 0.0, "noch nicht gedreht");
        tool_click(&mut app, zentrum + V2::new(0.0, 2000.0)); // Ziel: 90°

        let view = app.current_view();
        assert!(
            (view.rot.to_degrees() - 90.0).abs() < 1e-6,
            "Winkel ist {}°",
            view.rot.to_degrees()
        );
        assert!(app.tool.is_none(), "Werkzeug endet nach dem dritten Klick");
        let nachher = world_to_screen(rect, view, zentrum);
        assert!(
            (vorher - nachher).length() < 0.01,
            "Drehzentrum wandert um {:?} px",
            vorher - nachher
        );

        // Zurück auf die Ausgangsorientierung
        let mut v = app.current_view();
        v.zurueck_drehen();
        app.set_view(v);
        assert_eq!(app.current_view().rot, 0.0);
    }

    /// Ein Klick direkt auf das Zentrum liefert keinen Winkel — das Werkzeug
    /// lehnt ab und sammelt nicht weiter.
    #[test]
    fn klick_auf_das_zentrum_wird_abgelehnt() {
        let mut app = App::with_ctx(egui::Context::default());
        app.start_tool("view.rotate_tool");
        let zentrum = V2::new(2000.0, 1500.0);
        tool_click(&mut app, zentrum);
        tool_click(&mut app, zentrum);
        assert_eq!(
            app.tool.as_ref().map(|t| t.pts.len()),
            Some(1),
            "der Klick auf das Zentrum darf keinen Bezugspunkt setzen"
        );
        assert_eq!(app.current_view().rot, 0.0);
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
    // Bei offenem Protokoll bleibt der Mauszeiger stehen: die Zeichenfläche
    // nimmt keine Eingabe an, und in der Eingabezeile wird er zum Textcursor.
    if !app.chat.offen && (resp.hovered() || resp.dragged_by(egui::PointerButton::Primary)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    }
    let view = app.current_view();
    let is_sheet = matches!(app.tab().tab, TabKind::Sheet(_));

    // Hintergrund
    painter.rect_filled(rect, 0.0, Palette::BG);

    // Chat-Protokoll offen: die Zeichenfläche ist gesperrt (Navigation,
    // Werkzeug und Zeichnen ruhen), damit man im Protokoll lesen und
    // scrollen kann. Die Zeichnung wird dafür blass.
    let chat_offen = app.chat.offen;

    // Navigation
    if !chat_offen {
        handle_navigation(app, ui, &resp, rect, view);
    }

    // Weltkoordinate unter dem Zeiger
    let pointer = ui.ctx().pointer_hover_pos().filter(|p| rect.contains(*p));
    let raw_world = pointer.map(|p| screen_to_world(rect, app.current_view(), p));

    // Erweiterter Punktfang: steht der Zeiger lange genug auf einem Fangpunkt,
    // wird dieser gehalten — von ihm gehen dann Hilfslinien aus, auf denen der
    // nächste Punkt fängt. Vor dem Fangen, weil der Fang die Haltepunkte nutzt.
    if app.tool.is_some() {
        let tol = fang_radius_px(app, app.current_view().zoom);
        let kandidat = raw_world.and_then(|w| verweil_kandidat(app, w, tol));
        let jetzt = std::time::Instant::now();
        if let Some(h) = app.verweil.update(kandidat, jetzt) {
            halt_merken(app, h);
        }
        if app.verweil.laeuft() {
            // Ohne Zeigerbewegung kommt kein Frame — der Zeitgeber muss selbst
            // einen anfordern, sonst löst das Verweilen nie aus.
            ui.ctx().request_repaint_after(VERWEILDAUER);
        }
    } else {
        // Kein Werkzeug: kein Haltepunkt, keine Hilfslinien.
        app.halt.clear();
        app.verweil.zuruecksetzen();
    }

    // Fangen — im Papierraum genauso wie im Modell (dort auf Schriftfeld,
    // Texten und eigenen Blattelementen).
    let mut snapped = raw_world;
    app.snap_label = None;
    if let Some(w) = raw_world {
        let (p, label) = fang_zeiger(app, w, fang_radius_px(app, app.current_view().zoom));
        snapped = Some(p);
        app.snap_label = label.map(str::to_string);
    }
    app.pointer_world = snapped;
    if let Some(g) = app.ghost.as_mut() {
        if let Some(p) = snapped {
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
        // Druckvorschau: Blatt, Ansichtsfenster und Papierraum-Objekte.
        draw_sheet(app, &painter, rect, view_now, snapped);
        // Danach dieselbe Bedien-Darstellung wie im Zeichenmodus — Werkzeug,
        // Griffe, Einfüge-Vorschau, Auswahlfenster und Fadenkreuz-Zeiger.
        if app.tool.is_some() {
            draw_tool_preview(app, &painter, rect, view_now, snapped, raw_world);
        }
        draw_hilfslinien(app, &painter, rect, view_now);
        draw_text_rahmen(app, &painter, rect, view_now, &selected);
        draw_wand_basislinien(app, &painter, rect, view_now, &selected);
        draw_grips(app, &painter, rect, view_now, &selected);
        if app.ghost.is_some() {
            draw_ghost(app, &painter, rect, view_now);
        }
        if let Some((start, cur, _)) = app.select_drag {
            draw_auswahlfenster(&painter, rect, view_now, start, cur);
        }
        draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world);
    } else {
        let dl = display_list(app);
        let pc = PaintCtx {
            selected: selected.clone(),
            hover,
            lineweights,
            lineweights_scaled,
            paper_px,
            preview: false,
            rot: view_now.rot as f32,
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
            draw_tool_preview(app, &painter, rect, view_now, snapped, raw_world);
        }
        // Hilfslinien des erweiterten Punktfangs unter allem, was der Zeiger
        // selbst zeichnet (Vorschau, Griffe, Fadenkreuz).
        draw_hilfslinien(app, &painter, rect, view_now);
        draw_text_rahmen(app, &painter, rect, view_now, &selected);
        draw_wand_basislinien(app, &painter, rect, view_now, &selected);
        draw_grips(app, &painter, rect, view_now, &selected);
        if app.ghost.is_some() {
            draw_ghost(app, &painter, rect, view_now);
        }
        if let Some((start, cur, _)) = app.select_drag {
            draw_auswahlfenster(&painter, rect, view_now, start, cur);
        }
        draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world);
    }

    // Eingabe
    let pointer_pos = ui.ctx().pointer_hover_pos();
    if resp.hovered() && !chat_offen {
        if let Some(pos) = pointer_pos {
            // Hover-Highlight (auch im Papierraum — dort auf Schriftfeld/Texten)
            if app.tool.is_none() {
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

    if chat_offen {
        // Zeichnung blass hinter dem Protokoll (Lesbarkeit), darüber dieselben
        // Meldungen wie bei der Ausgabe — nur weiter nach oben und ohne
        // Verblassen. Die Eingabezeile zeichnet `chat::fenster`.
        let (flaeche, farbe) = crate::chat::schleier(rect);
        painter.rect_filled(flaeche, 0.0, farbe);
        crate::chat::zeichnen_offen(&app.chat, &painter, rect);
    } else {
        handle_input(app, ui, &resp, rect, &painter, is_sheet, snapped, raw_world);
        // Aufforderung des laufenden Werkzeugs (bzw. der Einfüge-Vorschau) in
        // den Chat. Nur bei geändertem Text, sonst stünde sie 60-mal pro
        // Sekunde im Protokoll.
        if app.ghost.is_some() {
            app.mitteilung_wenn_neu("Einfügen · Klick setzt ab (R dreht, S spiegelt)");
        } else if let Some(t) = app.tool.clone() {
            let hinweis = crate::app::commands_hint(&t, &app.options);
            app.mitteilung_wenn_neu(hinweis);
        }
        // Transparente Meldungen unten links, die nach einigen Sekunden
        // verblassen; T öffnet das vollständige Protokoll.
        let jetzt = ui.ctx().input(|i| i.time);
        crate::chat::zeichnen_geschlossen(&app.chat, &painter, rect, jetzt);
    }

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

    // Texteingabe steht über der Zeichnung, damit der Cursor sichtbar ist und
    // die Klicks das Feld treffen.
    crate::textedit::show(app, ui, rect, view_now);
}

/// Der Text, der gerade geschrieben wird, steht in der Eingabe
/// (`textedit::show`) — er darf nicht zusätzlich aus der Anzeige kommen.
///
/// Der Filter läuft **nach** dem Zwischenspeicher: die Liste hängt an
/// Revision/Zoom, der Bearbeitungszustand nicht. Stand er im Aufbau, blieb der
/// alte Text beim Doppelklick auf eine geladene Zeichnung im Hintergrund
/// stehen (T0019, Turn 3).
fn ohne_texteingabe(app: &App, dl: &mut DisplayList) {
    if let Some(id) = app.ui.texteingabe.as_ref().and_then(|t| t.id) {
        dl.prims.retain(|p| p.entity != id);
    }
}

fn display_list(app: &mut App) -> DisplayList {
    // Der Schlüssel enthält den Bearbeitungsraum: im Modell und im Papierraum
    // entstehen bei gleicher Revision völlig verschiedene Listen. Fehlte er,
    // wurde ein Papierraum-Stand als Modell gezeichnet (T0007).
    let key = DrawCacheKey {
        rev: app.tab().rev,
        zoom: app.current_view().zoom,
        annot: app.doc().settings.annotation_scale,
        space: app.session().space(),
    };
    let mut dl = match &app.tab().cache {
        Some((k, dl)) if k.matches(&key) => dl.clone(),
        _ => {
            let doc = app.doc().clone();
            let ctx = render::RenderCtx::new(&doc);
            let dl = render::render_document(&ctx);
            app.tab_mut().cache = Some((key, dl.clone()));
            dl
        }
    };
    ohne_texteingabe(app, &mut dl);
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
    // Sichtbarer Weltbereich: bei gedrehter Ansicht ist das die Hülle der
    // vier Bildschirmecken. Die Rasterlinien laufen als Weltstrecken durch
    // diese Hülle und werden mitgedreht gezeichnet — sonst stünden sie quer
    // zur gedrehten Zeichnung.
    let (x0, y0, x1, y1) = visible_world_bounds(rect, view);

    let draw_set = |step: f64, color: Color32, w: f32, p: &Painter| {
        let start = (x0 / step).floor() as i64;
        let end = (x1 / step).ceil() as i64;
        if end - start > 3000 {
            return;
        }
        let stroke = Stroke::new(w, color);
        for i in start..=end {
            let x = i as f64 * step;
            p.line_segment(
                [
                    world_to_screen(rect, view, V2::new(x, y0)),
                    world_to_screen(rect, view, V2::new(x, y1)),
                ],
                stroke,
            );
        }
        let start = (y0 / step).floor() as i64;
        let end = (y1 / step).ceil() as i64;
        for i in start..=end {
            let y = i as f64 * step;
            p.line_segment(
                [
                    world_to_screen(rect, view, V2::new(x0, y)),
                    world_to_screen(rect, view, V2::new(x1, y)),
                ],
                stroke,
            );
        }
    };
    draw_set(minor, Palette::GRID_MINOR, 1.0, p);
    draw_set(major, Palette::GRID_MAJOR, 1.0, p);
    // Achsen
    let achse = Stroke::new(1.2, Color32::from_rgb(0xC9, 0xC2, 0xB0));
    if x0 <= 0.0 && 0.0 <= x1 {
        p.line_segment(
            [
                world_to_screen(rect, view, V2::new(0.0, y0)),
                world_to_screen(rect, view, V2::new(0.0, y1)),
            ],
            achse,
        );
    }
    if y0 <= 0.0 && 0.0 <= y1 {
        p.line_segment(
            [
                world_to_screen(rect, view, V2::new(x0, 0.0)),
                world_to_screen(rect, view, V2::new(x1, 0.0)),
            ],
            achse,
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
    let Some(sheet) = app.doc().sheet(sid).cloned() else {
        return;
    };
    // Arbeitsfläche
    p.rect_filled(rect, 0.0, Color32::from_rgb(0x0E, 0x10, 0x13));
    let (pw, ph) = sheet.size();
    // Das Blatt ist ein Rechteck in Papierkoordinaten — bei gedrehter Ansicht
    // sind seine vier Ecken ein gedrehtes Viereck, keine achsenparallele Box.
    let ecken = [
        world_to_screen(rect, view, V2::new(0.0, 0.0)),
        world_to_screen(rect, view, V2::new(pw, 0.0)),
        world_to_screen(rect, view, V2::new(pw, ph)),
        world_to_screen(rect, view, V2::new(0.0, ph)),
    ];
    let schatten: Vec<Pos2> = ecken.iter().map(|q| *q + Vec2::new(6.0, 10.0)).collect();
    p.add(Shape::convex_polygon(
        schatten,
        Color32::from_black_alpha(90),
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(ecken.to_vec(), Palette::PAPER, Stroke::NONE));
    let w2s = |q: V2| world_to_screen(rect, view, q);

    // 1. Inhalt der Ansichtsfenster (liegt unter dem Papierraum)
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
        let vdl = render::render_model_scaled(app.doc(), vp.scale, vp.layers.clone());
        // Strichstärken sind Papier-Millimeter und hängen nicht am Maßstab
        // des Fensters — sonst wird aus einer 0,5-mm-Wand eine 60-fache Kante
        // (so wie im PDF, das die Breite unverändert übernimmt).
        let vpc = PaintCtx {
            selected: Vec::new(),
            hover: None,
            lineweights: true,
            lineweights_scaled: true,
            paper_px: view.zoom,
            preview: false,
            rot: 0.0,
        };
        for prim in &vdl.prims {
            paint_prim(&clip, prim, &vp_w2s, view.zoom * s, &vpc, 1.0);
        }
    }

    // 2. Rahmen der Ansichtsfenster
    let selected_vp = app.tab().selected_viewport;
    for vp in &sheet.viewports {
        let vp_rect = Rect::from_min_max(
            w2s(V2::new(vp.x, vp.y + vp.h)),
            w2s(V2::new(vp.x + vp.w, vp.y)),
        );
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

    // 3. Papierraum: Schriftfeld, Texte, eigene Blattelemente — mit Auswahl
    //    und Hover wie im Zeichenmodus.
    let pc = PaintCtx {
        selected: app.selection().to_vec(),
        hover: app.tab().hover,
        lineweights: true,
        lineweights_scaled: true,
        paper_px: view.zoom,
        preview: false,
        rot: view.rot as f32,
    };
    let mut dl = render::render_sheet(app.doc(), &sheet);
    ohne_texteingabe(app, &mut dl);
    for prim in &dl.prims {
        paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
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
        "Papierraum: zeichnen und ändern wie im Modell · Ansicht anklicken, verschieben mit V",
        FontId::proportional(10.5),
        Palette::TEXT_FAINT,
    );
}

// ---------------------------------------------------------------------------
// Auswahl, Cursor, Vorschau
// ---------------------------------------------------------------------------

/// Die beiden Linien des Fadenkreuzes (waagerecht, senkrecht) über die ganze
/// Fläche. `None`, wenn der Zeiger außerhalb liegt — dann bleibt der
/// Systemzeiger sichtbar.
fn fadenkreuz_linien(rect: Rect, pos: Option<Pos2>) -> Option<([Pos2; 2], [Pos2; 2])> {
    let pos = pos.filter(|q| rect.contains(*q))?;
    Some((
        [Pos2::new(rect.left(), pos.y), Pos2::new(rect.right(), pos.y)],
        [Pos2::new(pos.x, rect.top()), Pos2::new(pos.x, rect.bottom())],
    ))
}

/// Durchgehendes Fadenkreuz bis zu den Bildschirmrändern — eine Darstellung
/// für Zeichenmodus und Druckvorschau (Blatt).
fn draw_fadenkreuz(p: &Painter, rect: Rect, pos: Option<Pos2>) {
    let Some((waagerecht, senkrecht)) = fadenkreuz_linien(rect, pos) else {
        return;
    };
    let c = Color32::from_rgba_unmultiplied(88, 98, 112, 160);
    p.line_segment(waagerecht, Stroke::new(1.0, c));
    p.line_segment(senkrecht, Stroke::new(1.0, c));
}

#[cfg(test)]
mod tests_fadenkreuz {
    use super::*;

    fn flaeche() -> Rect {
        Rect::from_min_max(Pos2::new(100.0, 200.0), Pos2::new(900.0, 700.0))
    }

    /// Das Fadenkreuz reicht bis an alle vier Ränder, nicht nur um den Zeiger.
    #[test]
    fn fadenkreuz_geht_bis_zum_rand() {
        let (w, s) = fadenkreuz_linien(flaeche(), Some(Pos2::new(400.0, 300.0))).unwrap();
        assert_eq!(w[0], Pos2::new(100.0, 300.0));
        assert_eq!(w[1], Pos2::new(900.0, 300.0));
        assert_eq!(s[0], Pos2::new(400.0, 200.0));
        assert_eq!(s[1], Pos2::new(400.0, 700.0));
    }

    /// Außerhalb der Vorschaufläche wird kein Fadenkreuz gezeichnet, damit
    /// Ribbon, Panels und Dialoge ihren normalen Zeiger behalten.
    #[test]
    fn ausserhalb_kein_fadenkreuz() {
        assert!(fadenkreuz_linien(flaeche(), None).is_none());
        assert!(fadenkreuz_linien(flaeche(), Some(Pos2::new(400.0, 100.0))).is_none());
        assert!(fadenkreuz_linien(flaeche(), Some(Pos2::new(950.0, 300.0))).is_none());
    }

    /// Druckvorschau und Zeichenmodus benutzen denselben Zeiger: beide Zweige
    /// enden in `draw_cursor`, und das zeichnet das Fadenkreuz bis zum Rand.
    /// Sonst verschwindet der Zeiger in der Vorschau wieder (t_a2e21b73).
    #[test]
    fn druckvorschau_und_zeichenmodus_teilen_das_fadenkreuz() {
        let src = include_str!("canvas.rs");
        let i = src
            .find("draw_sheet(app, &painter, rect, view_now, snapped);")
            .unwrap();
        let rest = &src[i..];
        let blatt = &rest[..rest.find("} else {").expect("Blatt-Zweig nicht gefunden")];
        assert!(
            blatt.contains("draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world)"),
            "Blatt-Zweig zeichnet den Zeiger nicht über draw_cursor"
        );
        let j = src.find("let dl = display_list(app);").unwrap();
        let rest = &src[j..];
        let modell = &rest[..rest.find("// Eingabe").expect("Zeichenmodus nicht gefunden")];
        assert!(
            modell.contains("draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world)"),
            "Zeichenmodus zeichnet den Zeiger nicht über draw_cursor"
        );
        assert!(
            src.contains("draw_fadenkreuz(p, rect, Some(pos));"),
            "draw_cursor zeichnet das Fadenkreuz nicht mehr"
        );
    }

}

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
    draw_fadenkreuz(p, rect, Some(pos));

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
    raw: Option<V2>,
) {
    // Farbeimer-Vorschau nur für sein eigenes Werkzeug: beim Wechsel (oder wenn
    // der Zeiger die Fläche verlässt) bleibt sonst eine Fläche stehen, die zu
    // keinem Werkzeug mehr gehört.
    if !matches!(
        app.tool.as_ref().map(|t| t.action.as_str()),
        Some("draw.fill.paint") | Some("draw.volume.paint")
    ) {
        app.paint.zuruecksetzen();
    }
    let Some(cursor) = snapped else {
        app.paint.zuruecksetzen();
        return;
    };
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
                // Klick auf den Startpunkt schließt den Zug — der Zeiger zeigt
                // vorher, wo die Schlusskante liegen wird.
                let tol = fang_radius_px(app, view.zoom);
                let schliesst = tool.pts.len() >= 2
                    && tool
                        .pts
                        .first()
                        .map(|q| q.dist(cursor) <= tol)
                        .unwrap_or(false);
                if schliesst {
                    p.line_segment([w2s(start), w2s(cursor)], stroke);
                    label = "schließen".to_string();
                } else {
                    p.line_segment([w2s(start), w2s(cursor)], dashed);
                    if tool.pts.len() >= 2 {
                        if let Some(first) = tool.pts.first() {
                            p.line_segment([w2s(cursor), w2s(*first)], dashed);
                        }
                    }
                    label = format!(
                        "{} {}",
                        unit.format_plain(start.dist(cursor), unit.decimals()),
                        unit.short()
                    );
                }
            }
        }
        "draw.spline" => {
            // Vorschau ist die Kurve selbst — dieselbe Catmull-Rom-Zerlegung
            // wie beim Anlegen. Der Polygonzug als Vorschau würde ein anderes
            // Ergebnis zeigen als der Abschluss erzeugt.
            let tol = fang_radius_px(app, view.zoom);
            let am_start = tool.pts.len() >= 3
                && tool
                    .pts
                    .first()
                    .map(|q| q.dist(cursor) <= tol)
                    .unwrap_or(false);
            let mut pts = tool.pts.clone();
            if am_start {
                label = "schließen".to_string();
            } else {
                pts.push(cursor);
            }
            let kurve = catmull_rom_points(&pts, am_start, SPLINE_TOL_MM);
            if kurve.len() >= 2 {
                let screen: Vec<Pos2> = kurve.iter().map(|q| w2s(*q)).collect();
                if am_start {
                    p.add(Shape::line(screen, stroke));
                } else {
                    p.add(Shape::line(screen, dashed));
                }
            }
            // Die schon gesetzten Punkte als durchgezogene Kurve darüber.
            if tool.pts.len() >= 2 {
                let fest = catmull_rom_points(&tool.pts, false, SPLINE_TOL_MM);
                if fest.len() >= 2 {
                    let screen: Vec<Pos2> = fest.iter().map(|q| w2s(*q)).collect();
                    p.add(Shape::line(screen, stroke));
                }
            }
            for q in &tool.pts {
                p.circle_stroke(w2s(*q), 2.0, Stroke::new(1.0, Palette::ACCENT));
            }
            if label.is_empty() {
                label = format!("{} Stützpunkte", tool.pts.len() + 1);
            }
        }
        "draw.fill.boundary" | "draw.fill.points" | "draw.volume.points" => {
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
        "draw.fill.paint" | "draw.volume.paint" => {
            // Gerechnet wird auf dem ROHEN Zeigerpunkt: der Fang zieht sonst
            // genau auf die Wandkante, und dort wäre „auf einer Linie geklickt“
            // die richtige, aber unerwünschte Antwort.
            let zeiger = raw.unwrap_or(cursor);
            let jetzt = std::time::Instant::now();
            if app.paint.update(zeiger, jetzt) {
                let g = detect_boundary(app.doc(), zeiger, &BoundaryOpts::default());
                app.paint.merken(zeiger, g);
            }
            match app.paint.ergebnis(zeiger) {
                Some(Ok(g)) => {
                    draw_paint_ghost(app, p, rect, view, &g.outer, &g.holes);
                    label = format!("Fläche {}", unit.format_area(g.area(), 2));
                }
                Some(Err(e)) => {
                    // Kein Dialog: der Grund steht am Zeiger und beim Klick
                    // zusätzlich als Toast.
                    label = e.message().to_string();
                }
                None => {
                    // Auf die Ruhezeit warten — ohne neuen Eingabe-Impuls zeichnet
                    // egui nicht neu, die Vorschau käme sonst nie.
                    p.ctx().request_repaint_after(PAINT_RUHE);
                    label = "Fläche ermitteln …".into();
                }
            }
        }
        "draw.rect" => {
            if app.options.rect_mode == Einfuegemodus::Basislinie {
                match tool.pts.len() {
                    // Basislinie steht noch nicht: nichts zu zeigen.
                    0 => {}
                    // Erste Kante: Basislinie zum Zeiger, mit Länge und Winkel.
                    1 => {
                        let a0 = tool.pts[0];
                        p.line_segment([w2s(a0), w2s(cursor)], dashed);
                        label = format!(
                            "{} {}   {:.1}°",
                            unit.format_plain(a0.dist(cursor), unit.decimals()),
                            unit.short(),
                            (cursor - a0).angle().to_degrees()
                        );
                    }
                    // Rechteck über der festen Basislinie, Höhe am Zeiger.
                    _ => {
                        let a0 = tool.pts[0];
                        let b0 = tool.pts[1];
                        let h = basis_hoehe(a0, b0, cursor);
                        let n = (b0 - a0).normalized().perp() * h;
                        let quad: Vec<Pos2> =
                            [a0, b0, b0 + n, a0 + n].iter().map(|q| w2s(*q)).collect();
                        p.add(egui::Shape::closed_line(quad, stroke));
                        p.line_segment([w2s(a0), w2s(b0)], dashed);
                        label = format!(
                            "{} × {}",
                            unit.format_plain(a0.dist(b0), unit.decimals()),
                            unit.format_plain(h.abs(), unit.decimals())
                        );
                    }
                }
            } else if let Some(start) = last {
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
            if app.options.ellipse_mode == Einfuegemodus::Basislinie {
                match tool.pts.len() {
                    0 => {}
                    // Mittelachse ziehen: Lage und Länge (2 × große Halbachse).
                    1 => {
                        let a0 = tool.pts[0];
                        p.line_segment([w2s(a0), w2s(cursor)], dashed);
                        label = format!(
                            "{} {}   {:.1}°",
                            unit.format_plain(a0.dist(cursor), unit.decimals()),
                            unit.short(),
                            (cursor - a0).angle().to_degrees()
                        );
                    }
                    // Ellipse um die feste Mittelachse, Breite am Zeiger.
                    _ => {
                        let a0 = tool.pts[0];
                        let b0 = tool.pts[1];
                        let laenge = a0.dist(b0);
                        let rx = laenge * 0.5;
                        let ry = basis_hoehe(a0, b0, cursor).abs();
                        let c = v((a0.x + b0.x) * 0.5, (a0.y + b0.y) * 0.5);
                        let rot = (b0 - a0).angle();
                        // Dieselbe Punktfolge wie die Anzeige: gedrehte Ellipse.
                        let pts: Vec<Pos2> = (0..=64)
                            .map(|i| {
                                let t = i as f64 / 64.0 * std::f64::consts::TAU;
                                let lx = rx * t.cos();
                                let ly = ry * t.sin();
                                w2s(c + V2::new(
                                    lx * rot.cos() - ly * rot.sin(),
                                    lx * rot.sin() + ly * rot.cos(),
                                ))
                            })
                            .collect();
                        p.add(Shape::line(pts, stroke));
                        p.line_segment([w2s(a0), w2s(b0)], dashed);
                        label = format!(
                            "{} × {}",
                            unit.format_plain(laenge, unit.decimals()),
                            unit.format_plain(ry * 2.0, unit.decimals())
                        );
                    }
                }
            } else if let Some(c) = last {
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
                    let pts: Vec<Pos2> = CurveQuality::SCREEN
                        .flatten(&arc, view.zoom)
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
            // Vorschau ist die Tür selbst: dasselbe Symbol wie nach dem Klick
            // (Blatt, Öffnungsbogen, Anschlagseite) auf der vorderen
            // Wandkante, dazu die Wand mit ihrem Loch. Ein eigener Platzhalter
            // hatte mit dem Ergebnis nichts zu tun (T0026).
            match wall_under(app, cursor) {
                Some((cp, ang, host)) => {
                    let spec = render::DoorSpec {
                        host: Some(host),
                        center: cp,
                        angle: ang,
                        width,
                        kind: app.options.door_kind,
                        hinge: app.options.hinge,
                        swing_in: app.options.swing_in,
                    };
                    if let Some(v) = render::preview_door(app.doc(), &spec) {
                        let paper_px = view.zoom * app.doc().settings.annotation_scale;
                        let wand_ctx = PaintCtx {
                            selected: app.selection().to_vec(),
                            hover: app.tab().hover,
                            lineweights: app.ui.lineweights,
                            lineweights_scaled: app.ui.lineweights_scaled,
                            paper_px,
                            preview: false,
                            rot: view.rot as f32,
                        };
                        let tuer_ctx = PaintCtx {
                            preview: true,
                            ..wand_ctx.clone()
                        };
                        // Loch in der Wand freilegen (die Wand darunter läuft
                        // sonst durch), dann Wand, dann Tür als Akzent
                        for quad in &v.loch {
                            let pts: Vec<Pos2> = quad.iter().map(|q| w2s(*q)).collect();
                            p.add(egui::Shape::convex_polygon(
                                pts,
                                Palette::PAPER,
                                Stroke::NONE,
                            ));
                        }
                        for prim in &v.wall.prims {
                            // Füllflächen auslassen: sie lägen über allem, was
                            // die Wand überdeckt (Bemaßung, Schraffur) —
                            // sichtbar ist nur die Unterbrechung der Kanten.
                            if matches!(prim.shape, PShape::Fill { .. }) {
                                continue;
                            }
                            paint_prim(p, prim, &w2s, view.zoom, &wand_ctx, 1.0);
                        }
                        for prim in &v.door.prims {
                            paint_prim(p, prim, &w2s, view.zoom, &tuer_ctx, 1.0);
                        }
                    }
                    label = format!("Tür {}", unit.format_plain(width, 2));
                }
                None => {
                    // Eine Tür braucht eine Wand — ohne sie nur der Hinweis
                    p.circle_stroke(w2s(cursor), 9.0, dashed);
                    label = "Tür · nur auf einer Wand".into();
                }
            }
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
            // Dieselbe Platzierung wie der Text, der beim Klick entsteht
            // (links, mittig) — nicht mit der Grundlinie am Zeiger.
            paint_text(
                p,
                &text_preview_prim(txt, h),
                w2s(cursor),
                view.zoom,
                col,
                view.rot as f32,
            );
        }
        "draw.fill" | "draw.hatch" | "draw.volume" => {
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
        "modify.move" => {
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
                                    ghost_shape(
                                        p,
                                        &prim.shape,
                                        &w2s,
                                        view.zoom,
                                        pre,
                                        view.rot as f32,
                                    );
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
        "view.rotate_tool" => {
            // Gleiche Winkelwahl wie beim Drehen (Zentrum, Bezugspunkt, neue
            // Richtung) — gedreht wird aber die Ansicht, nicht die Auswahl.
            if let Some(&c) = tool.pts.first() {
                center_mark(p, w2s(c), Palette::ACCENT);
                match tool.pts.get(1).copied() {
                    None => {
                        if c.dist(cursor) > 1e-6 {
                            p.line_segment([w2s(c), w2s(cursor)], dashed);
                        }
                        p.circle_stroke(w2s(cursor), 5.0, stroke);
                        label = format!(
                            "Bezug {}°",
                            format_number((cursor - c).angle().to_degrees(), 1)
                        );
                    }
                    Some(from) => {
                        let v0 = from - c;
                        p.line_segment([w2s(c), w2s(from)], stroke);
                        match jjcad_core::transform::angle_between_directions(c, from, cursor) {
                            Some(angle) => {
                                p.line_segment([w2s(c), w2s(cursor)], dashed);
                                // Winkelbogen in Drehrichtung (Vorzeichen, <= 180°)
                                let r = v0.len().min((cursor - c).len()) * 0.45;
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
                                // Das Drehzentrum bleibt beim Drehen stehen
                                p.circle_stroke(w2s(c), 9.0, dashed);
                                label = format!("Ansicht {}°", format_number(angle, 1));
                            }
                            None => label = "Abstand zum Drehzentrum nötig".into(),
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
        // Box im Zeichenbereich halten: am unteren/äußeren Rand würde sie sonst
        // abgeschnitten (Fehlermeldungen stehen genau dort, wo geklickt wird).
        let groesse = galley.size() + Vec2::new(12.0, 6.0);
        let min = Pos2::new(
            (label_pos.x - 6.0)
                .min(rect.right() - groesse.x - 4.0)
                .max(rect.left() + 4.0),
            (label_pos.y - 3.0)
                .min(rect.bottom() - groesse.y - 4.0)
                .max(rect.top() + 4.0),
        );
        let bg = Rect::from_min_size(min, groesse);
        p.rect_filled(bg, 4.0, Color32::from_rgba_unmultiplied(24, 27, 32, 235));
        p.rect_stroke(bg, 4.0, Stroke::new(1.0, Palette::BORDER));
        p.galley(bg.min + Vec2::new(6.0, 3.0), galley, Palette::TEXT);
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
        rot: view.rot as f32,
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
        // Verschiebeversatz über die gedrehte Ansicht zurückrechnen, damit die
        // Zeichnung dem Zeiger folgt (auch bei gedrehter Ansicht).
        let w = v.screen_delta_to_world(d);
        v.center -= w;
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
            let px = 700.0 * dt as f32;
            let w = v.screen_delta_to_world(Vec2::new(dir.x * px, dir.y * px));
            v.center += w;
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

/// Fangradius in Weltkoordinaten für eine Zoomstufe: Bildschirm-Pixel ÷ Zoom.
///
/// Damit ist der Fang unabhängig von der Zoomstufe immer gleich groß auf dem
/// Schirm; die Untergrenze fängt entartete Zoomwerte ab.
fn fang_radius_px(app: &App, zoom: f64) -> f64 {
    (app.doc().settings.snap.tolerance_px / zoom.max(1e-6)).max(0.5)
}

/// Fang-Kette für den Zeiger in der aktuellen Werkzeuglage.
///
/// Liefert den zu übernehmenden Weltpunkt und das Label für die Statuszeile.
/// **Einziger Weg** für Zeigeranzeige (`show`) und Klickpunkt (`snap_at`) —
/// laufen sie getrennt, weicht der gespeicherte Punkt vom sichtbaren
/// Fangpunkt ab. Reihenfolge der Fänge: `snap::snap_constrained`.
fn fang_zeiger(app: &App, w: V2, tol: f64) -> (V2, Option<&'static str>) {
    let reference = app.tool.as_ref().and_then(|t| t.pts.last().copied());
    let regeln = fang_regeln(app);
    let vorschau = vorschau_punkte(app);
    // `snap_constrained_mit_halte`: die Haltepunkte des erweiterten Punktfangs
    // liefern ihre Hilfslinien als zusätzliche Fangziele mit.
    let res = snap::snap_constrained_mit_halte(
        app.doc(),
        w,
        tol,
        reference,
        &regeln.ignore,
        &regeln.keep,
        &regeln.gesperrt,
        &vorschau,
        &app.halt,
        app.ui.show_grid,
    );
    (res.point, res.label())
}

/// Stützpunkte des gerade gezeichneten Zuges als zusätzliche Fangziele.
///
/// Die Polylinie entsteht erst mit dem Abschluss im Modell; bis dahin kennt die
/// Fang-Engine ihre Punkte nicht und der Anfang (Schließen) oder ein früherer
/// Stützpunkt ist nicht anvisierbar. Sie fangen deshalb als Endpunkte.
///
/// Der **letzte** Punkt ist Bezugspunkt der nächsten Strecke (Lot, Ortho,
/// Polar) und bleibt außen vor — sonst zöge er den Zeiger auf sich selbst und
/// die Strecke käme nicht vom Fleck. Weniger als zwei Punkte haben keine
/// Vorschau: ein einzelner Punkt ist immer der Bezugspunkt.
fn vorschau_punkte(app: &App) -> Vec<V2> {
    let Some(t) = app.tool.as_ref() else {
        return Vec::new();
    };
    if !matches!(t.action.as_str(), "draw.polyline" | "draw.spline") || t.pts.len() < 2 {
        return Vec::new();
    }
    t.pts[..t.pts.len() - 1].to_vec()
}

/// Fängt einen Weltpunkt für eine beliebige Bildschirmposition (ohne Label-Nebenwirkungen).
fn snap_at(app: &App, rect: Rect, view: View, pos: Pos2) -> V2 {
    let w = screen_to_world(rect, view, pos);
    fang_zeiger(app, w, fang_radius_px(app, view.zoom)).0
}

/// Fang-Regeln für den aktuellen Zustand: was ist gesperrt, was ausdrücklich
/// erlaubt, welche Punkte sind als Ziel verboten.
#[derive(Default)]
struct FangRegeln {
    /// Elemente, die überhaupt nicht fangen.
    ignore: Vec<Id>,
    /// Elemente, die nur mit ihren *diskreten* Punkten fangen (Ecke, Mitte,
    /// Zentrum, Quadrant) — keine kontinuierlichen Fänge. Sie bewegen sich
    /// gerade mit, an ihnen würde „Nächster Punkt" den Zeiger zurückziehen.
    keep: Vec<Id>,
    /// Einzelne Punkte, die nicht fangen, obwohl ihr Element es darf: die
    /// gerade gezogenen Stützpunkte (Element + Stelle).
    gesperrt: Vec<snap::GesperrterPunkt>,
}

/// Fang-Ausschluss, Fang-Auswahl und gesperrte Punkte für den aktuellen Zustand.
///
/// * **Ruhezustand / Werkzeug:** die Auswahl ist ausdrücklich *erlaubt*
///   (Ecken/Endpunkte), nichts ist ausgeschlossen.
/// * **Auswahl wandert ganz** (Ziehen am Objekt, Verschieben/Kopieren ab dem
///   Basispunkt): die ganze Auswahl fällt aus dem Fang, weil alle ihre Ecken
///   mit dem Zeiger wandern — sonst klebt die Bewegung am Startpunkt.
/// * **Griff-Drag:** nur *ein* Stützpunkt wandert. Das Element bleibt mit
///   seinen übrigen Ecken fangbar (Polylinie an ihre eigene Nachbarecke bzw.
///   an den Anfangspunkt anschließen — Voraussetzung für geschlossene Züge);
///   allein der gezogene Punkt ist gesperrt, sonst fängt er sich selbst und
///   der Griff bleibt stehen (Karte t_eb950a21).
fn fang_regeln(app: &App) -> FangRegeln {
    let auswahl_wandert = app.move_drag.is_some()
        || app
            .tool
            .as_ref()
            .map(|t| {
                t.action == "modify.move" && !t.pts.is_empty()
            })
            .unwrap_or(false);
    if auswahl_wandert {
        return FangRegeln {
            ignore: app.selection().to_vec(),
            ..FangRegeln::default()
        };
    }
    if let Some(drag) = app.grip_drag.as_ref() {
        let gesperrt = app
            .doc()
            .entity(drag.entity)
            .map(|e| crate::handles::fang_sperre(e, drag.kind))
            .unwrap_or_default()
            .into_iter()
            .map(|p| (drag.entity, p))
            .collect();
        return FangRegeln {
            ignore: Vec::new(),
            keep: vec![drag.entity],
            gesperrt,
        };
    }
    FangRegeln {
        keep: app.selection().to_vec(),
        ..FangRegeln::default()
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

    // Offene Texteingabe: die Zeichenfläche nimmt keine Klicks an. Übernommen
    // bzw. abgebrochen wird in `textedit::show` (Enter, Esc, Klick daneben).
    if app.ui.texteingabe.is_some() {
        return;
    }

    // Enter beendet Polylinie, Umriss-Fläche und Maßkette (Tippeingabe hat Vorrang)
    if app.ui.typed.is_empty()
        && !ctx.wants_keyboard_input()
        && ctx.input(|i| i.key_pressed(egui::Key::Enter))
    {
        if let Some(t) = app.tool.clone() {
            let punktfolge = ist_fuell_punkte(&t.action);
            if matches!(t.action.as_str(), "draw.polyline" | "draw.spline" | "dim.chain")
                || punktfolge
            {
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

    // Laufende Fensterauswahl zuerst abschließen: das muss auch dann passieren,
    // wenn der Zeiger die Zeichenfläche oder das Fenster verlassen hat — unten
    // wird wegen der Zeigerposition abgebrochen, das Auswahlfenster blieb sonst
    // dauerhaft stehen (T0023).
    if auswahl_abschliessen(app, ctx, resp, rect, view) {
        return;
    }

    // Papierraum: Ansichtsfenster (auswählen, aufziehen, mit V verschieben)
    // haben Vorrang vor der Objektbearbeitung — alles andere läuft wie im
    // Modell über dieselben Werkzeuge.
    if is_sheet && sheet_viewport_input(app, resp, rect, view) {
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

    // "Vor/Hinter Element": nächster Klick wählt das Referenzobjekt
    if let Some(above) = app.ui.order_pick {
        if resp.clicked_by(egui::PointerButton::Primary) {
            if let Some(pos) = resp.interact_pointer_pos() {
                let w = screen_to_world(rect, view, pos);
                let tol = 10.0 / view.zoom.max(1e-6);
                match snap::hit_test(app.doc(), w, tol, None) {
                    Some(hit) => {
                        let ids = app.selection().to_vec();
                        let _ = app.session_mut().execute(
                            "modify.order",
                            &serde_json::json!({
                                "ids": ids,
                                "ref": hit,
                                "where": if above { "above" } else { "below" },
                            }),
                        );
                        app.mark_changed();
                        app.toast(
                            if above { "Vor Element gesetzt" } else { "Hinter Element gesetzt" }.to_string(),
                        );
                    }
                    None => app.toast("Reihenfolge · kein Element getroffen".to_string()),
                }
                app.ui.order_pick = None;
            }
            return;
        }
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
                    tool_click_at(app, pw, screen_to_world(rect, view, pos));
                }
            }
            // Jeder gesetzte Punkt löst die Hilfslinien; der nächste holt sich
            // seinen Haltepunkt neu (Verweilen) oder zeichnet frei.
            app.halt.clear();
            return;
        }
    }

    // Doppelklick auf einen Text: Inhalt direkt in der Zeichnung ändern.
    if app.tool.is_none() && resp.double_clicked_by(egui::PointerButton::Primary) {
        if let Some(id) = crate::textedit::text_bei(app, raw.unwrap_or(w)) {
            crate::textedit::start_entitaet(app, id);
            return;
        }
    }

    // Klick
    if resp.clicked_by(egui::PointerButton::Primary) {
        if app.tool.is_some() {
            tool_click_at(app, w, raw.unwrap_or(w));
            app.halt.clear();
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
                "draw.polyline" | "draw.spline" | "dim.chain" => {
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
                        app.mitteilung("Abgebrochen");
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
            // Klick auf ein bereits ausgewaehltes Objekt: Auswahl verschieben.
            // Text nicht — der wandert nur ueber das Werkzeug "Verschieben".
            if let Some(r) = raw {
                let tol2 = 10.0 / view.zoom.max(1e-6);
                let frame = resp.ctx.input(|i| i.time);
                if let Some(hit) = snap::hit_test(app.doc(), r, tol2, None) {
                    if app.selection().contains(&hit) && app.begin_move_drag_bei_klick(hit, r, frame) {
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
    // Beendet wird eine Auswahl in `auswahl_abschliessen` (oben) — dort auch
    // dann, wenn der Zeiger die Zeichenfläche bzw. das Fenster verlassen hat.
}

/// Schließt eine laufende Fensterauswahl ab (T0023).
///
/// `handle_input` bricht weiter unten ab, sobald der Zeiger nicht über der
/// Zeichenfläche steht. Wer die Maus gedrückt hält und über das Ribbon, die
/// Panels oder aus dem Fenster hinausfährt, hat das Auswahlfenster deshalb nie
/// abgeräumt bekommen — es blieb samt Einfärbung stehen. Hier wird der Zug
/// stattdessen beendet:
///
/// * solange die Taste liegt und der Zeiger beim Fenster ist, läuft er weiter;
///   außerhalb der Zeichenfläche folgt der Endpunkt dem Rand (sonst freeze);
/// * sonst wird die Auswahl übernommen — das Loslassen außerhalb, ein
///   abgelöster Zeiger (X11 ohne Mausgriff: das Loslassen erreicht das Fenster
///   gar nicht) und der Wechsel in ein anderes Fenster beenden ihn hier.
///
/// Liefert `true`, wenn der Zug beendet wurde (die übrige Auswertung entfällt).
fn auswahl_abschliessen(
    app: &mut App,
    ctx: &egui::Context,
    resp: &Response,
    rect: Rect,
    view: View,
) -> bool {
    let Some((start, letzter, add)) = app.select_drag else {
        return false;
    };
    let (taste_liegt, zeiger_da) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.has_pointer()));
    if taste_liegt && zeiger_da && !resp.drag_stopped_by(egui::PointerButton::Primary) {
        if let Some(p) = ctx.input(|i| i.pointer.latest_pos()) {
            if !rect.contains(p) {
                let rand = screen_to_world(rect, view, rect.clamp(p));
                app.select_drag = Some((start, rand, add));
            }
        }
        return false;
    }

    let end = match ctx.input(|i| i.pointer.latest_pos()) {
        Some(p) => screen_to_world(rect, view, rect.clamp(p)),
        None => letzter,
    };
    app.select_drag = None;
    auswahl_uebernehmen(app, start, end, add, view);
    true
}

/// Übernimmt das aufgezogene Auswahlfenster (blau = Fenster, grün = Querfenster
/// bei Zug nach links).
fn auswahl_uebernehmen(app: &mut App, start: V2, end: V2, add: bool, view: View) {
    if start.dist(end) * view.zoom <= 4.0 {
        return;
    }
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
        // Loslassen — oder der Zeiger hat das Fenster verlassen: unter X11 ohne
        // Mausgriff erreicht das Loslassen das Fenster dann nicht mehr, der Zug
        // liefe sonst endlos weiter (T0023).
        resp.ctx
            .input(|i| i.pointer.any_released() || !i.pointer.has_pointer())
    }
}

/// Ansichtsfenster im Papierraum: aufziehen, anklicken, mit `V` verschieben.
///
/// Liefert `true`, wenn der Klick/Zug hier verbraucht wurde. Papierraum-Objekte
/// (Schriftfeld, Texte, eigene Linien) werden nicht angefasst — die laufen
/// durch dieselbe Werkzeug- und Auswahlbehandlung wie im Modell.
fn sheet_viewport_input(app: &mut App, resp: &Response, rect: Rect, view: View) -> bool {
    let Some(sid) = (match app.tab().tab {
        TabKind::Sheet(id) => Some(id),
        _ => None,
    }) else {
        return false;
    };
    let Some(pos) = resp.ctx.pointer_hover_pos() else {
        return false;
    };
    if !rect.contains(pos) {
        return false;
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
                    let (sw, sh) = app
                        .doc()
                        .sheet(sid)
                        .map(|s| s.size())
                        .unwrap_or((420.0, 297.0));
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
            return true;
        }
    }

    // Klick: Papierraum-Objekt hat Vorrang, sonst wird das Fenster gewählt.
    // Während ein Zeichenwerkzeug läuft, wird nichts ausgewählt — sonst
    // markiert jeder Klick ins Fenster zusätzlich das Ansichtsfenster.
    if resp.clicked_by(egui::PointerButton::Primary) && app.tool.is_none() {
        let tol = (8.0 / view.zoom).max(0.5);
        let objekt = snap::hit_test(app.doc(), w, tol, None);
        if objekt.is_none() {
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
            if hit.is_some() {
                app.tab_mut().selected_viewport = hit;
                app.session_mut().clear_selection();
            } else {
                // Klick auf freies Papier: Fensterauswahl aufheben, damit
                // Werkzeuge und Objektauswahl wieder greifen.
                app.tab_mut().selected_viewport = None;
            }
        } else {
            app.tab_mut().selected_viewport = None;
        }
    }

    // Verschieben nur mit aktivem Verschieben-Werkzeug (V) und ohne Objektauswahl
    let move_tool = app.tool_id() == Some("modify.move");
    let fenster_gewaehlt = app.tab().selected_viewport.is_some();
    if move_tool && fenster_gewaehlt && app.selection().is_empty() {
        if resp.drag_started_by(egui::PointerButton::Primary) {
            let snapshot = app.doc().clone();
            app.session_mut().history.record(&snapshot);
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(vid) = app.tab().selected_viewport {
                let d = resp.drag_delta();
                let w = view.screen_delta_to_world(d);
                let dx = w.x;
                let dy = w.y;
                if let Some(sheet) = app.doc_mut().sheet_mut(sid) {
                    if let Some(vp) = sheet.viewports.iter_mut().find(|v| v.id == vid) {
                        vp.x = ((vp.x + dx) * 2.0).round() / 2.0;
                        vp.y = ((vp.y + dy) * 2.0).round() / 2.0;
                    }
                }
                app.mark_changed();
            }
        }
        // Klicks nicht an die Werkzeuge weitergeben, solange das Fenster steht
        if resp.clicked_by(egui::PointerButton::Primary) || resp.dragged_by(egui::PointerButton::Primary) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Vorschau vor dem Platzieren (Block oder Zwischenablage)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Griffe zum direkten Bearbeiten
// ---------------------------------------------------------------------------

/// Auswahlfenster (blau = Fenster, grün = Querfenster). Beide Räume benutzen
/// dieselbe Darstellung.
fn draw_auswahlfenster(p: &Painter, rect: Rect, view: View, start: V2, cur: V2) {
    let r = Rect::from_two_pos(world_to_screen(rect, view, start), world_to_screen(rect, view, cur));
    let crossing = cur.x < start.x;
    let fill = if crossing {
        Color32::from_rgba_unmultiplied(0x5A, 0xB5, 0x82, 34)
    } else {
        Color32::from_rgba_unmultiplied(0x2F, 0x8C, 0xF0, 34)
    };
    p.rect_filled(r, 0.0, fill);
    p.rect_stroke(
        r,
        0.0,
        Stroke::new(1.2, if crossing { Palette::OK } else { Palette::SELECT }),
    );
}

/// Rahmen ausgewählter Texte: derselbe Kasten, den die Einfüge-Vorschau zeigt.
///
/// Er ist sonst unsichtbar — er dient dem Punktfang (Ecken und Kantenmitten)
/// und wird bei Auswahl gezeichnet, damit man sieht, wo der Text hängt. Kein
/// allgemeiner Auswahlrahmen: nur Text hat keinen Griff, an dem man ihn fasst.
fn draw_text_rahmen(app: &App, p: &Painter, rect: Rect, view: View, selected: &[Id]) {
    if selected.is_empty() {
        return;
    }
    let doc = app.doc();
    for id in selected {
        let Some(e) = doc.entity(*id) else { continue };
        let Some(ecken) = render::text_frame(doc, e) else { continue };
        // Viereck aus den vier Ecken — bei gedrehtem Text steht der Kasten
        // schief wie die Schrift, nicht als achsenparalleles Rechteck (T0025).
        let pts: Vec<Pos2> = ecken.iter().map(|q| world_to_screen(rect, view, *q)).collect();
        p.add(Shape::closed_line(
            pts,
            Stroke::new(1.0, Palette::SELECT.gamma_multiply(0.55)),
        ));
    }
}

/// Basislinie der ausgewählten Wände: die Strecke zwischen den beiden
/// Endpunkten (`a`→`b`), auf der der Wandkörper aufsitzt.
///
/// Sie ist das Maß für den Eckanschluss: verbunden wird nur, wenn zwei
/// Basislinien-Endpunkte **aufeinander** liegen (siehe `render::JOIN_EPS`).
/// Ohne diese Linie ist bei versetzten Enden nicht zu sehen, warum zwei Wände
/// keine Gehrung bekommen — gestrichelt und in der Akzentfarbe, damit sie sich
/// vom blau eingefärbten Wandkörper abhebt.
fn draw_wand_basislinien(app: &App, p: &Painter, rect: Rect, view: View, selected: &[Id]) {
    if selected.is_empty() || selected.len() > 60 {
        return;
    }
    let doc = app.doc();
    let stroke = Stroke::new(1.2, Palette::ACCENT);
    for id in selected {
        let Some(e) = doc.entity(*id) else { continue };
        let EntityKind::Wall { a, b, .. } = &e.kind else {
            continue;
        };
        let pa = world_to_screen(rect, view, *a);
        let pb = world_to_screen(rect, view, *b);
        p.extend(Shape::dashed_line(&[pa, pb], stroke, 9.0, 5.0));
        for q in [pa, pb] {
            p.circle_filled(q, 2.2, Palette::ACCENT);
        }
    }
}

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
                // Bogenenden: runder Griff — der Punkt läuft auf dem Kreis mit
                crate::handles::GripKind::ArcStart | crate::handles::GripKind::ArcEnd => {
                    p.circle_filled(
                        c,
                        size * 0.55,
                        if hot { Palette::TEXT } else { Palette::ACCENT },
                    );
                    p.circle_stroke(c, size * 0.55, Stroke::new(1.2_f32, Palette::BG));
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
        ghost_shape(p, &prim.shape, &w2s, zoom, stroke, view.rot as f32);
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

fn ghost_shape(
    p: &Painter,
    sh: &PShape,
    w2s: &dyn Fn(V2) -> Pos2,
    zoom: f64,
    stroke: Stroke,
    rot: f32,
) {
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
            let pts: Vec<Pos2> = CurveQuality::SCREEN
                .flatten(&arc, zoom)
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
        // Text war hier nicht gezeichnet: die Einfüge-Vorschau zeigte bei
        // kopierten Textfeldern nur den Anker, keinen Text (T0025). Gezeichnet
        // über `paint_text` — dieselbe Lage, Schrift und Drehung wie danach.
        PShape::Text(t) => paint_text(p, t, w2s(t.pos), zoom, Palette::ACCENT, rot),
        PShape::Dot { p: pos, r } => {
            p.circle_filled(w2s(*pos), ((*r * zoom) as f32).max(1.2), stroke.color);
        }
        // Füllungen haben in der Vorschau keine Kontur — die Umrandung
        // zeichnet `draw_ghost` (äußerer Rahmen) bzw. das Objekt selbst.
        PShape::Fill { .. } => {}
    }
}

// ---------------------------------------------------------------------------
// Werkzeug-Logik
// ---------------------------------------------------------------------------

/// Wartezeit, bevor der Farbeimer die Umgrenzung rechnet.
///
/// Die Erkennung ist eine Flutfüllung über die halbe Zeichnung (gemessen
/// 10–80 ms im Wohnhaus-Plan). Bei jeder Zeigerbewegung gerechnet, würde die
/// Vorschau dem Zeiger hinterherhinken; nach kurzem Stillstand ist sie
/// schnell genug. 100 ms ist die Grenze, ab der ein stehender Zeiger als
/// „will jetzt füllen“ gilt.
pub const PAINT_RUHE: std::time::Duration = std::time::Duration::from_millis(100);

/// Vorschau-Zustand des Farbeimers.
///
/// Merkt sich, wo der Zeiger steht, seit wann er dort steht und welches
/// Ergebnis für genau diese Position gehört. Ohne den Zwischenspeicher würde
/// jeder Frame neu geflutet; ohne die Ruhezeit würde schon eine winzige
/// Bewegung eine neue Rechnung auslösen.
#[derive(Clone, Default)]
pub struct PaintPreview {
    /// Position, für die gerechnet wurde bzw. auf die gewartet wird.
    pos: Option<V2>,
    /// Zeitpunkt, seit dem der Zeiger auf `pos` steht.
    seit: Option<std::time::Instant>,
    /// Ergebnis für genau `pos` (`Err` = kein Füllen möglich).
    ergebnis: Option<Result<FillBoundary, BoundaryError>>,
}

impl PaintPreview {
    /// Zeigerposition melden. `true`, wenn jetzt gerechnet werden soll.
    pub fn update(&mut self, pos: V2, jetzt: std::time::Instant) -> bool {
        let gewandert = self.pos.map(|p| p.dist(pos) > 0.01).unwrap_or(true);
        if gewandert {
            self.pos = Some(pos);
            self.seit = Some(jetzt);
            self.ergebnis = None;
            return false;
        }
        if self.ergebnis.is_some() {
            return false;
        }
        jetzt.duration_since(self.seit.unwrap_or(jetzt)) >= PAINT_RUHE
    }

    /// Ergebnis für die zuletzt gemeldete Position ablegen.
    pub fn merken(&mut self, pos: V2, r: Result<FillBoundary, BoundaryError>) {
        if self.pos.map(|p| p.dist(pos) <= 0.01).unwrap_or(false) {
            self.ergebnis = Some(r);
        }
    }

    /// Ergebnis zur Position — `None`, solange dafür noch nichts gerechnet ist.
    pub fn ergebnis(&self, pos: V2) -> Option<&Result<FillBoundary, BoundaryError>> {
        if self.pos.map(|p| p.dist(pos) <= 0.01).unwrap_or(false) {
            self.ergebnis.as_ref()
        } else {
            None
        }
    }

    /// Vorschau verwerfen (Zeiger verlässt die Zeichenfläche, Werkzeug wechselt,
    /// Fläche wurde gefüllt).
    pub fn zuruecksetzen(&mut self) {
        self.pos = None;
        self.seit = None;
        self.ergebnis = None;
    }
}

// ---------------------------------------------------------------------------
// Erweiterter Punktfang
// ---------------------------------------------------------------------------

/// Wie lange der Zeiger auf einem Fangpunkt stehen muss, bis er gehalten wird.
/// Kurz genug, um nebenbei zu passieren, lang genug, um nicht beim Durchfahren
/// eines Endpunkts versehentlich Hilfslinien aufzuziehen.
pub const VERWEILDAUER: std::time::Duration = std::time::Duration::from_millis(400);

/// Höchstzahl gehaltener Punkte. Zwei genügen: x vom einen, y vom anderen —
/// mehr Linien machen den Fang nur unübersichtlich.
pub const HALTE_MAX: usize = 2;

/// Verweil-Erkennung des erweiterten Punktfangs.
///
/// Merkt sich, welcher Fangpunkt unter dem Zeiger liegt und seit wann. Bleibt
/// derselbe Punkt lange genug stehen (der Fangpunkt, nicht der Zeiger — ein
/// paar Pixel Zittern sind unschädlich), gilt er als gehalten und liefert
/// Hilfslinien. Wandert der Kandidat, beginnt die Zeit von vorn.
#[derive(Clone, Default)]
pub struct VerweilFang {
    /// Fangpunkt unter dem Zeiger (Punkt + Fangart), ohne Richtung.
    kandidat: Option<(V2, snap::SnapKind)>,
    /// Zeitpunkt, seit dem dieser Kandidat steht.
    seit: Option<std::time::Instant>,
}

impl VerweilFang {
    /// Kandidat melden; `Some`, sobald er lange genug steht.
    ///
    /// Der Kandidat wird nur **einmal** geliefert: danach bleibt `seit` auf dem
    /// Stand „schon gehalten", sonst entstünde bei jedem Frame ein neuer Halt.
    pub fn update(
        &mut self,
        kandidat: Option<snap::HaltPunkt>,
        jetzt: std::time::Instant,
    ) -> Option<snap::HaltPunkt> {
        let kennung = kandidat.as_ref().map(|h| (h.point, h.kind));
        let gewechselt = match (&self.kandidat, &kennung) {
            (Some((p, k)), Some((q, l))) => p.dist(*q) > 0.1 || k != l,
            (None, None) => false,
            _ => true,
        };
        if gewechselt {
            self.kandidat = kennung;
            self.seit = kandidat.as_ref().map(|_| jetzt);
        }
        let kandidat = kandidat?;
        if jetzt.duration_since(self.seit.unwrap_or(jetzt)) >= VERWEILDAUER {
            // Gehalten: Zeitgeber auf NULL, den Kandidaten aber behalten. Sonst
            // gälte derselbe Punkt im nächsten Frame wieder als neu, der
            // Zeitgeber liefe endlos und die Zeichenfläche müsste dauernd
            // neu zeichnen. Erst ein anderer Kandidat löst wieder aus.
            self.seit = None;
            return Some(kandidat);
        }
        None
    }

    /// Läuft gerade eine Verweilzeit? Dann muss die Zeichenfläche einen Frame
    /// anfordern — ohne Zeigerbewegung kommt sonst keiner.
    pub fn laeuft(&self) -> bool {
        self.seit.is_some()
    }

    /// Verweil-Erkennung verwerfen (Werkzeugwechsel, Zeiger verlässt die Fläche).
    pub fn zuruecksetzen(&mut self) {
        self.kandidat = None;
        self.seit = None;
    }
}

/// Haltepunkt übernehmen: höchstens [`HALTE_MAX`], derselbe Punkt nicht doppelt.
/// Der neueste steht vorn — er ist der zuletzt gewählte.
fn halt_merken(app: &mut App, h: snap::HaltPunkt) {
    if app.halt.iter().any(|x| x.point.dist(h.point) <= 0.1) {
        return;
    }
    app.halt.insert(0, h);
    app.halt.truncate(HALTE_MAX);
}

/// Fangpunkt unter dem Zeiger, über dem der erweiterte Punktfang auslösen darf.
///
/// Nur **diskrete** Punkte (Endpunkt, Blattecke, Schnittpunkt, Mittelpunkt,
/// Zentrum, Quadrant): „Nächster Punkt" wandert mit dem Zeiger und das Raster
/// liegt überall — beide stünden nie still und würden bei jedem Verweilen
/// Hilfslinien erzeugen.
fn verweil_kandidat(app: &App, w: V2, tol: f64) -> Option<snap::HaltPunkt> {
    let reference = app.tool.as_ref().and_then(|t| t.pts.last().copied());
    let regeln = fang_regeln(app);
    let vorschau = vorschau_punkte(app);
    snap::snap_candidates_mit_vorschau(
        app.doc(),
        w,
        tol,
        reference,
        &regeln.ignore,
        &regeln.keep,
        &regeln.gesperrt,
        &vorschau,
        false,
    )
    .into_iter()
    .find(|h| h.kind.ist_diskret())
    .map(|h| snap::HaltPunkt::neu(app.doc(), &h))
}

/// Hilfslinien der gehaltenen Punkte zeichnen: gestrichelt quer durch die
/// Zeichenfläche, die Haltepunkte selbst als kleines Kreuz.
fn draw_hilfslinien(app: &App, p: &Painter, rect: Rect, view: View) {
    if app.halt.is_empty() {
        return;
    }
    let col = Palette::TRACK;
    let muster = [6.0, 4.0];
    let lang = rect.width().max(rect.height()) * 1.2;
    let mut shapes: Vec<Shape> = Vec::new();

    for l in snap::hilfslinien(&app.halt) {
        let o = world_to_screen(rect, view, l.origin);
        // Richtung auf dem Schirm: der y-Anteil dreht sich mit der Ansicht.
        let d = world_to_screen(rect, view, l.origin + l.dir) - o;
        if d.length() < 1e-3 {
            continue;
        }
        let u = d.normalized();
        for [a, b] in dashed_segments(o - u * lang, o + u * lang, &muster) {
            shapes.push(Shape::line_segment(
                [a, b],
                Stroke::new(1.0, col.gamma_multiply(0.8)),
            ));
        }
    }

    for h in &app.halt {
        let s = world_to_screen(rect, view, h.point);
        let r = 6.0;
        shapes.push(Shape::line_segment(
            [s - Vec2::new(r, 0.0), s + Vec2::new(r, 0.0)],
            Stroke::new(1.4, col),
        ));
        shapes.push(Shape::line_segment(
            [s - Vec2::new(0.0, r), s + Vec2::new(0.0, r)],
            Stroke::new(1.4, col),
        ));
        shapes.push(Shape::circle_stroke(
            s,
            9.0,
            Stroke::new(1.0, col.gamma_multiply(0.6)),
        ));
    }
    p.extend(shapes);
}

/// Füll-Werkzeuge, deren Umgrenzung als Punktfolge geklickt wird.
///
/// `draw.fill.boundary` (Umriss abfahren) und `draw.fill.points` (freie
/// Punktfolge) unterscheiden sich nur im Einstieg — Klicks, Vorschau, Abschluss,
/// Undo und Platzieren-Panel teilen sie sich. Ein zweiter Zweig pro Werkzeug
/// würde die beiden Werkzeuge früher oder später auseinanderlaufen lassen.
pub fn ist_fuell_punkte(action: &str) -> bool {
    matches!(
        action,
        "draw.fill.boundary" | "draw.fill.points" | "draw.volume.points"
    )
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
            // Vorschau zeigt die Umrandung, die das Objekt bekommt
            border: app.options.fill_border_points,
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
        rot: view.rot as f32,
    };
    for prim in &dl.prims {
        paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
    }
}

/// Vorschau des Farbeimers: die ermittelte Fläche halbtransparent mit dem
/// Muster aus dem Panel, Inseln (Schornstein, Treppe) ausgespart.
///
/// Gezeichnet wird über denselben Renderer wie das fertige Objekt
/// ([`render::render_entity`]) — Vorschau und Ergebnis können damit nicht
/// auseinanderlaufen, auch nicht bei Mustern mit Löchern.
fn draw_paint_ghost(
    app: &App,
    p: &Painter,
    rect: Rect,
    view: View,
    boundary: &[V2],
    holes: &[Vec<V2>],
) {
    if boundary.len() < 3 {
        return;
    }
    let w2s = |q: V2| world_to_screen(rect, view, q);
    let mut e = Entity::new(
        0,
        app.doc().current_layer,
        EntityKind::Fill {
            boundary: boundary.to_vec(),
            holes: holes.to_vec(),
            pattern: app.options.paint_pattern,
            pattern_angle: app.options.fill_angle,
            pattern_scale: app.options.fill_scale,
            // Vorgabe aus: die Grenze des Eimers ist die Wand. Die Checkbox
            // schaltet sie zu, dann zeichnet die Vorschau sie wie das Objekt.
            border: app.options.fill_border_paint,
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
        rot: view.rot as f32,
    };
    for prim in &dl.prims {
        paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
    }
    // Ohne Umrandung am Objekt die Grenze gestrichelt nachziehen, damit die
    // Fläche nicht ohne Kante steht. Mit Umrandung zeichnet der Renderer sie
    // bereits — eine zweite Linie würde sie nur dicker machen.
    if !app.options.fill_border_paint {
        let stroke = Stroke::new(1.2, Palette::ACCENT.gamma_multiply(0.85));
        for ring in std::iter::once(boundary).chain(holes.iter().map(|h| h.as_slice())) {
            if ring.len() < 2 {
                continue;
            }
            let pts: Vec<Pos2> = ring.iter().map(|q| w2s(*q)).collect();
            for pair in pts.windows(2) {
                p.line_segment([pair[0], pair[1]], stroke);
            }
            p.line_segment([*pts.last().unwrap(), pts[0]], stroke);
        }
    }
}

/// Schließt eine Füllfläche aus der geklickten Punktfolge.
///
/// Gemeinsamer Abschluss der Füll-Werkzeuge: baut die Eckpunkte mit
/// [`fill_boundary_points`] (doppelte Punkte, Schluss auf dem Start), nimmt
/// Muster, Winkel, Abstand und Umrandung aus dem Platzieren-Panel und ruft die
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
        app.toast("Füllfläche · mindestens 3 Punkte");
        return false;
    };
    let pts: Vec<[f64; 2]> = boundary
        .iter()
        .map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)])
        .collect();
    let mut params = json!({"boundary": pts,
           "pattern": app.options.fill_pattern.key(),
           "angle": app.options.fill_angle.to_degrees(),
           "scale": app.unit().from_mm(app.options.fill_scale),
           "border": app.options.fill_border_points});
    if action.starts_with("draw.volume") {
        // Volumen: Höhe kommt aus dem Platzieren-Panel (Standard = Wandhöhe).
        params["height"] = json!(app.unit().from_mm(app.options.wall_height));
        // Ein Körper ist Mauerwerk: standardmäßig die volle Fläche in
        // Wandfarbe, kein Muster.
        params["pattern"] = json!("solid");
    }
    let ok = run_und_waehlen(app, action, params);
    if ok {
        app.tool = None;
    }
    ok
}

pub fn finish_tool(app: &mut App, from_enter: bool) {
    let Some(tool) = app.tool.clone() else { return };
    match tool.action.as_str() {
        "draw.polyline" | "draw.spline" => {
            let action = if tool.action == "draw.spline" {
                "draw.spline"
            } else {
                "draw.polyline"
            };
            if tool.pts.len() >= 2 {
                let layer = app.doc().current_layer;
                // Der Klick auf den Startpunkt legt den ersten Punkt erneut ab;
                // erkannt wird das hier (letzter Punkt = erster Punkt) und geht
                // als `closed` an die Befehls-Schicht. Die entfernt den doppelten
                // Endpunkt, damit er nicht im Modell landet.
                let tol = fang_radius_px(app, app.current_view().zoom);
                let geschlossen = tool.pts.len() >= 3
                    && tool
                        .pts
                        .first()
                        .zip(tool.pts.last())
                        .map(|(a, b)| a.dist(*b) <= tol)
                        .unwrap_or(false);
                let _ = run(
                    app,
                    action,
                    json!({"pts": tool.pts.iter().map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)]).collect::<Vec<_>>(), "closed": geschlossen, "layer": layer}),
                );
            }
            app.tool = None;
        }
        "draw.fill.boundary" | "draw.fill.points" | "draw.volume.points" => {
            // Ein Abschluss für alle drei Eingaben; `action` bestimmt nur,
            // welche Action den Kern aufruft (MCP und Klickpfad landen im
            // selben Code). `draw.volume` erzeugt dasselbe Objekt wie eine
            // Füllfläche, nur als Körper mit Höhe.
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
                return RotateStep::Reject("Drehen · Abstand zum Zentrum nötig");
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
        None => RotateStep::Reject("Drehen · Abstand zum Zentrum nötig"),
    }
}

/// Werkzeug nach einem fertigen Objekt für das nächste bereit machen.
///
/// Zwei-Klick-Werkzeuge (Rechteck, Kreis, Ellipse) bleiben nach dem Anlegen
/// aktiv: der Punktpuffer wird geleert, die Action bleibt dieselbe. Damit
/// entsteht das nächste Objekt ohne erneutes Auswählen des Werkzeugs, und es
/// bleibt kein Restzustand (Startpunkt, Vorschau) vom vorherigen Objekt zurück
/// — die Vorschau hängt an `pts` und zeichnet bei leerem Puffer nichts.
/// Der Hinweis kommt aus dem Action-Register, damit Statusleiste und Start
/// über Ribbon/Hotbar/Taste dasselbe sagen. Beendet wird weiterhin mit ESC,
/// Werkzeugwechsel oder Rechtsklick (`cancel_tool`).
fn restart_tool(app: &mut App, action: &str) {
    app.tool = Some(Tool {
        action: action.to_string(),
        pts: Vec::new(),
        selecting: false,
        phase: ToolPhase::Collecting,
    });
    // Die Aufforderung des ersten Schritts kommt in jedem Frame aus
    // `commands_hint` — hier nichts zusätzlich melden (sonst zwei Zeilen).
    let _ = action;
    // Tippepuffer (Länge/Koordinaten) gehört zum vorigen Objekt.
    app.ui.typed.clear();
}

/// Vorzeichenbehafteter Abstand eines Punkts von der Geraden durch `a` in
/// Richtung `b` (positiv links der Richtung a→b).
///
/// Grundlage der diagonalen Einfügemodi: beim Rechteck ist der Abstand die
/// Höhe über der Basislinie, bei der Ellipse die halbe Breite neben der
/// Mittelachse. Der Anteil in Achsrichtung zählt nicht — nur der senkrechte
/// Abstand, damit die Maus entlang der Achse nicht die Höhe verändert.
fn basis_hoehe(a: V2, b: V2, p: V2) -> f64 {
    let d = b - a;
    let l = d.len();
    if l <= 1e-9 {
        return 0.0;
    }
    let n = d.perp() / l;
    let r = p - a;
    r.x * n.x + r.y * n.y
}

fn tool_click(app: &mut App, w: V2) {
    tool_click_at(app, w, w);
}

/// Klick im Werkzeug. `w` ist der Fangpunkt, `raw` der rohe Zeigerpunkt.
///
/// Fast alle Werkzeuge arbeiten auf dem Fangpunkt. Der Farbeimer braucht den
/// rohen Punkt: ein Fang auf die Wandkante bedeutet dort „auf einer Linie
/// geklickt“ und würde einen Klick dicht neben der Wand abweisen.
fn tool_click_at(app: &mut App, w: V2, raw: V2) {
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
        "draw.polyline" | "draw.spline" => {
            // Klick auf den Startpunkt schließt den Zug: der Punkt wird exakt
            // übernommen (kein doppelter Stützpunkt), `finish_tool` übergibt
            // `closed`. Unter drei Punkten entsteht keine Fläche — dann beendet
            // der Klick die Punktfolge offen, wie ein Klick auf „Fertig".
            let tol = fang_radius_px(app, app.current_view().zoom);
            let am_start = tool
                .pts
                .first()
                .map(|q| q.dist(w) <= tol)
                .unwrap_or(false);
            if am_start {
                if tool.pts.len() >= 3 {
                    let start = tool.pts[0];
                    tool.pts.push(start);
                }
                app.tool = Some(tool);
                finish_tool(app, false);
                return;
            }
            // Kein doppelter Stützpunkt, wenn zweimal dieselbe Stelle getroffen
            // wird (schneller Doppelklick).
            if tool.pts.last().map(|q| q.dist(w) <= tol).unwrap_or(false) {
                return;
            }
            tool.pts.push(w);
            app.tool = Some(tool);
        }
        // Umriss abfahren: derselbe Klickpfad wie die Polylinie — gesammelt
        // werden Punkte, nur das Ende erzeugt eine Füllfläche statt einer Linie.
        // Gilt für beide Füll-Werkzeuge (`draw.fill.boundary` und
        // `draw.fill.points`), damit sich die Eingabe nicht unterscheidet.
        "draw.fill.boundary" | "draw.fill.points" | "draw.volume.points" => {
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
        // Rechteck, Kreis und Ellipse brauchen zwei Klicks. Nach dem Anlegen
        // bleibt das Werkzeug aktiv (nächster Startpunkt, leerer Puffer),
        // damit mehrere Objekte nacheinander entstehen. Ein unbrauchbarer
        // zweiter Klick (gleiche x- oder y-Lage, Radius null) legt nichts an
        // und behält den Startpunkt — der nächste Klick zählt als zweiter.
        "draw.rect" => {
            if app.options.rect_mode == Einfuegemodus::Basislinie {
                // Basislinie + Höhe: Anfang, Ende, dann die Höhe (3 Klicks).
                if tool.pts.len() < 2 {
                    let doppelt = tool
                        .pts
                        .last()
                        .map(|q| q.dist(w) <= 0.5)
                        .unwrap_or(false);
                    if !doppelt {
                        tool.pts.push(w);
                        app.tool = Some(tool);
                    }
                } else {
                    let a = tool.pts[0];
                    let b = tool.pts[1];
                    let h = basis_hoehe(a, b, w);
                    if h.abs() > 0.5 {
                        let ok = run(
                            app,
                            "draw.rect",
                            json!({"a": ax(a), "b": ax(b), "height": unit.from_mm(h)}),
                        );
                        if ok {
                            restart_tool(app, "draw.rect");
                        }
                    }
                }
            } else if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let a = tool.pts[0];
                let b = w;
                if a.x != b.x && a.y != b.y {
                    let ok = run(app, "draw.rect", json!({"a": ax(a), "b": ax(b)}));
                    if ok {
                        restart_tool(app, "draw.rect");
                    }
                }
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
                    let ok = run(app, "draw.circle", json!({"c": ax(c), "r": unit.from_mm(r)}));
                    if ok {
                        restart_tool(app, "draw.circle");
                    }
                }
            }
        }
        "draw.ellipse" => {
            if app.options.ellipse_mode == Einfuegemodus::Basislinie {
                // Mittelachse + Breite: Anfang, Ende der Mittelachse, dann die
                // Breite (3 Klicks). Die Mittelachse liegt durch den Mittelpunkt,
                // ihre Länge ist die doppelte große Halbachse; der Abstand des
                // dritten Klicks zur Achse ist die halbe Breite — der Zeiger
                // fährt dabei am Ellipsenrand entlang.
                if tool.pts.len() < 2 {
                    let doppelt = tool
                        .pts
                        .last()
                        .map(|q| q.dist(w) <= 0.5)
                        .unwrap_or(false);
                    if !doppelt {
                        tool.pts.push(w);
                        app.tool = Some(tool);
                    }
                } else {
                    let a = tool.pts[0];
                    let b = tool.pts[1];
                    let laenge = a.dist(b);
                    let halbe_breite = basis_hoehe(a, b, w).abs();
                    if laenge > 0.5 && halbe_breite > 0.5 {
                        let c = v((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                        let rot = (b - a).angle().to_degrees();
                        let ok = run(
                            app,
                            "draw.ellipse",
                            json!({"c": ax(c), "rx": unit.from_mm(laenge * 0.5),
                                   "ry": unit.from_mm(halbe_breite), "rot": rot}),
                        );
                        if ok {
                            restart_tool(app, "draw.ellipse");
                        }
                    }
                }
            } else if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let c = tool.pts[0];
                let rx = (w.x - c.x).abs().max(1.0);
                let ry = (w.y - c.y).abs().max(1.0);
                let ok = run(
                    app,
                    "draw.ellipse",
                    json!({"c": ax(c), "rx": unit.from_mm(rx), "ry": unit.from_mm(ry)}),
                );
                if ok {
                    restart_tool(app, "draw.ellipse");
                }
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
        // Text direkt in der Zeichnung: ein Eingabefeld an der Klickstelle,
        // der Cursor steht sofort darin (kein Dialog, T0019).
        "draw.text" => {
            crate::textedit::start(app, w);
        }
        // Farbeimer: ein Klick, kein Ziehen. Der Kern ermittelt die Umgrenzung
        // (`draw.fill.paint` → `boundary::detect_boundary`); Fehler kommen als
        // Toast aus der Action zurück („Fläche ist nicht geschlossen“, „auf
        // einer Linie geklickt“) — kein Dialog, kein Objekt.
        "draw.fill.paint" | "draw.volume.paint" => {
            let act = tool.action.clone();
            let ok = run(
                app,
                &act,
                json!({"p": ax(raw),
                       "pattern": app.options.paint_pattern.key(),
                       "angle": app.options.fill_angle.to_degrees(),
                       "scale": unit.from_mm(app.options.fill_scale),
                       "border": app.options.fill_border_paint}),
            );
            // Nach dem Füllen (und auch nach einem Fehlschlag) neu rechnen:
            // die Vorschau gehört zur Fläche unter dem Zeiger, nicht zum Klick.
            app.paint.zuruecksetzen();
            let _ = ok;
        }
        "draw.fill" | "draw.hatch" | "draw.volume" => {
            let vol = tool.action == "draw.volume";
            let tol = (10.0 / app.current_view().zoom).max(0.5);
            let hit = snap::hit_test(app.doc(), w, tol, None);
            if let Some(id) = hit {
                if let Some(e) = app.doc().entity(id).cloned() {
                    let boundary = match &e.kind {
                        EntityKind::Polyline { verts, closed } if *closed => {
                            Some(verts.iter().map(|v| v.p).collect::<Vec<_>>())
                        }
                        // Geschlossener Spline: die Füllfläche folgt der Kurve,
                        // nicht den Stützpunkten — sonst lägen Muster und
                        // Umrandung neben der sichtbaren Kontur.
                        EntityKind::Spline { verts, closed } if *closed => {
                            let sp = Spline {
                                verts: verts.clone(),
                                closed: true,
                            };
                            Some(sp.flatten(SPLINE_TOL_MM))
                        }
                        EntityKind::Wall { a, b, thickness, .. } => {
                            let dir = (*b - *a).normalized();
                            let n = dir.perp() * (*thickness * 0.5);
                            Some(vec![*a + n, *b + n, *b - n, *a - n])
                        }
                        // Volumen und Füllfläche: ihre Kontur lässt sich
                        // übernehmen (z. B. Volumen in eine Fläche umwandeln).
                        EntityKind::Volume { boundary, .. }
                        | EntityKind::Fill { boundary, .. } => Some(boundary.clone()),
                        _ => None,
                    };
                    match boundary {
                        Some(pts) => {
                            // Volumen: dieselbe Kontur, aber als Körper mit
                            // Höhe (Standard = Wandhöhe), Fläche im Wandton.
                            let mut params = json!({
                                "boundary": pts.iter().map(|q| ax(*q)).collect::<Vec<_>>(),
                                "pattern": if vol {
                                    "solid".to_string()
                                } else {
                                    app.options.fill_pattern.key().to_string()
                                },
                                "angle": app.options.fill_angle.to_degrees(),
                                "scale": unit.from_mm(app.options.fill_scale),
                                "border": app.options.fill_border_outline,
                            });
                            if vol {
                                params["height"] = json!(unit.from_mm(app.options.wall_height));
                            }
                            let _ = run_und_waehlen(
                                app,
                                if vol { "draw.volume" } else { "draw.fill" },
                                params,
                            );
                        }
                        None => app.toast(if vol {
                            "Volumen · geschlossene Kontur anklicken"
                        } else {
                            "Füllfläche · geschlossene Kontur anklicken"
                        }),
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
                app.toast("Tür · nur auf einer Wand");
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
                app.toast("Fenster · nur auf einer Wand");
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
        "modify.move" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let base = tool.pts[0];
                let ids = app.selection().to_vec();
                let _ = run(
                    app,
                    "modify.move",
                    json!({"ids": ids, "from": ax(base), "to": ax(w)}),
                );
                app.tool = None;
            }
        }
        // Kopieren mit Basispunkt setzt **keine** Kopie ab, sondern legt die
        // Auswahl in die Zwischenablage; der Klick bestimmt den Basispunkt.
        // Abgesetzt wird danach mit der Einfüge-Vorschau (Strg+V) — so oft man
        // will und an der Stelle, die man gerade braucht.
        "modify.copy" => {
            let ids = app.selection().to_vec();
            if ids.is_empty() {
                app.toast("Kopieren mit Basispunkt · erst Objekte wählen");
                return;
            }
            app.merke_zwischenablage(&ids, Some(w));
            app.tool = None;
            app.toast("Auswahl · kopiert");
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
                    app.toast(&format!("Drehen · {}°", format_number(angle, 1)));
                    app.tool = None;
                }
                RotateStep::Reject(msg) => app.toast(msg),
            }
        }
        "view.rotate_tool" => {
            // Drei Klicks: Drehzentrum, Bezugspunkt, Zielrichtung. Der Winkel
            // ist die Differenz der beiden vom Zentrum aus gesehenen
            // Richtungen; die Ansicht dreht um das angeklickte Zentrum.
            match rotate_step(&tool.pts, w) {
                RotateStep::Collect { point } => {
                    tool.pts.push(point);
                    app.tool = Some(tool);
                }
                RotateStep::Apply { center, angle, .. } => {
                    let mut v = app.current_view();
                    v.rotate_about(center, angle.to_radians());
                    app.set_view(v);
                    app.tool = None;
                    app.toast(&format!("Ansicht · gedreht {}°", format_number(angle, 1)));
                }
                RotateStep::Reject(_) => {
                    app.toast("Ansicht drehen · Abstand zum Zentrum nötig")
                }
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
                    app.toast("Versetzen · Objekt anklicken");
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
                app.toast("Block · im Inventar wählen (E)");
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

/// Wie [`run`], wählt aber das neu entstandene Objekt aus — damit sich seine
/// Eckpunkte sofort greifen lassen (Füllfläche, Volumen).
fn run_und_waehlen(app: &mut App, action: &str, params: serde_json::Value) -> bool {
    let vorher: Vec<Id> = app.doc().entities.iter().map(|e| e.id).collect();
    let ok = run(app, action, params);
    if ok {
        if let Some(id) = app
            .doc()
            .entities
            .iter()
            .find(|e| !vorher.contains(&e.id))
            .map(|e| e.id)
        {
            app.session_mut().select(vec![id]);
        }
    }
    ok
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
            // Einheitliche Rückmeldung im Chat: `<Objekt> · <Zustand>`
            if let Some(m) = crate::app::abschluss_meldung(action) {
                app.mitteilung(m);
            }
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
        app.options.fill_border_points = true;
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

    /// Abbrechen verwirft: kein Objekt, Werkzeug beendet. (Der harte Abbruch
    /// liegt im Modus „Punkte setzen" auf Entf/Backspace — ESC bestätigt, siehe
    /// `tests_tasten_punktfolge`.)
    #[test]
    fn abbrechen_verwirft_den_umriss() {
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
        app.options.fill_border_points = true;
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

    // ---- Farbeimer (Karte F5, Action `draw.fill.paint`) ----

    #[test]
    fn farbeimer_vorschau_rechnet_erst_nach_ruhezeit() {
        use std::time::{Duration, Instant};
        let mut p = PaintPreview::default();
        let t0 = Instant::now();
        let klick = V2::new(1000.0, 1000.0);

        // Erste Meldung: Position merken, noch nicht rechnen.
        assert!(!p.update(klick, t0), "erster Frame darf nicht rechnen");
        assert!(
            !p.update(klick, t0 + Duration::from_millis(50)),
            "50 ms Stillstand sind zu wenig"
        );
        assert!(
            p.update(klick, t0 + Duration::from_millis(120)),
            "nach 100 ms Stillstand muss gerechnet werden"
        );

        // Ergebnis merken: eine Rechnung pro Position reicht.
        p.merken(klick, Err(BoundaryError::Open));
        assert!(
            !p.update(klick, t0 + Duration::from_secs(2)),
            "gemerkte Position darf nicht erneut rechnen"
        );
        assert_eq!(p.ergebnis(klick), Some(&Err(BoundaryError::Open)));

        // Bewegung verwirft Position und Ergebnis (Vorschau verschwindet).
        let weiter = V2::new(1500.0, 1000.0);
        assert!(p.ergebnis(weiter).is_none(), "kein Ergebnis für neue Position");
        assert!(!p.update(weiter, t0), "neue Position beginnt zu warten");
        assert!(p.ergebnis(klick).is_none(), "altes Ergebnis ist verworfen");

        // Verlassen der Fläche leert alles.
        p.merken(weiter, Err(BoundaryError::OnLine));
        p.zuruecksetzen();
        assert!(p.ergebnis(weiter).is_none());
    }

    /// Vier Linien als geschlossener Raum, Klick in die Mitte: genau eine
    /// Füllfläche, keine zusätzlichen Linien (die Umgrenzung ist keine eigene
    /// Geometrie im Dokument).
    #[test]
    fn farbeimer_klick_im_raum_legt_genau_eine_flaeche_an() {
        let mut app = app();
        let u = app.unit();
        let ecken = [
            (0.0, 0.0),
            (4000.0, 0.0),
            (4000.0, 3000.0),
            (0.0, 3000.0),
        ];
        for i in 0..4 {
            let (a, b) = (ecken[i], ecken[(i + 1) % 4]);
            let um = |p: (f64, f64)| [u.from_mm(p.0), u.from_mm(p.1)];
            assert!(run(
                &mut app,
                "draw.line",
                json!({"a": um(a), "b": um(b)})
            ));
        }
        assert_eq!(app.doc().entities.len(), 4);

        app.start_tool("draw.fill.paint");
        let klick = V2::new(2000.0, 1500.0);
        tool_click_at(&mut app, klick, klick);

        assert_eq!(
            app.doc().entities.len(),
            5,
            "vier Linien + eine Fläche, keine Umgrenzungslinien"
        );
        let (boundary, holes, pattern, border) = app
            .doc()
            .entities
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Fill {
                    boundary,
                    holes,
                    pattern,
                    border,
                    ..
                } => Some((boundary.clone(), holes.clone(), *pattern, *border)),
                _ => None,
            })
            .expect("Füllfläche");
        assert!(holes.is_empty());
        assert_eq!(pattern, app.options.paint_pattern, "Muster aus dem Panel");
        assert_eq!(pattern, FillPattern::Solid, "Vorgabe des Eimers");
        assert!(!border, "keine Randlinie auf der Wand");
        for (soll_x, soll_y) in [(0.0, 0.0), (4000.0, 3000.0)] {
            let nah = boundary
                .iter()
                .map(|q| q.dist(V2::new(soll_x, soll_y)))
                .fold(f64::INFINITY, f64::min);
            assert!(nah <= 0.5, "Ecke ({soll_x},{soll_y}) weicht {nah} mm ab");
        }
    }

    /// Klick auf eine Linie: keine Fläche, Toast mit dem Grund.
    #[test]
    fn farbeimer_klick_auf_linie_meldet_toast() {
        let mut app = app();
        let u = app.unit();
        for (a, b) in [
            ((-1000.0, 0.0), (1000.0, 0.0)),
            ((1000.0, 0.0), (1000.0, 1000.0)),
            ((1000.0, 1000.0), (-1000.0, 1000.0)),
            ((-1000.0, 1000.0), (-1000.0, 0.0)),
        ] {
            let um = |p: (f64, f64)| [u.from_mm(p.0), u.from_mm(p.1)];
            assert!(run(&mut app, "draw.line", json!({"a": um(a), "b": um(b)})));
        }
        app.start_tool("draw.fill.paint");
        let auf_der_linie = V2::new(0.0, 0.0);
        tool_click_at(&mut app, auf_der_linie, auf_der_linie);

        assert_eq!(app.doc().entities.len(), 4, "kein Objekt bei Fehler");
        assert_eq!(
            app.status_msg, "auf einer Linie geklickt",
            "Meldung als Toast/Status, kein Dialog"
        );
    }
    /// Die echte Farbeimer-Fläche aus dem Wohnhaus (125 Ecken, 3 Inseln) muss
    /// vollständig trianguliert werden: Summe der Dreiecke = Außenring minus
    /// Inseln. Bricht das, entstehen Lücken oder — schlimmer — Dreiecke
    /// außerhalb des Umrisses (die als diagonale Streifen über die Zeichenfläche
    /// laufen).
    #[test]
    fn farbeimer_flaeche_trianguliert_vollstaendig() {
        let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/Wohnhaus.jjcad");
        let doc = jjcad_core::persist::load_document(&pfad).expect("Wohnhaus");
        let g = detect_boundary(&doc, V2::new(8000.0, 4300.0), &BoundaryOpts::default())
            .expect("Fläche");

        let k = 0.05284f32; // Bildschirm-px je mm bei „ganze Zeichnung im Fenster“
        let ring = jjcad_core::area::bridge_holes(&g.outer, &g.holes);
        let mut screen: Vec<Pos2> = ring
            .iter()
            .map(|q| Pos2::new((q.x * k as f64) as f32, (q.y * k as f64) as f32))
            .collect();
        if flaeche_signiert(&screen) < 0.0 {
            screen.reverse();
        }
        let tris = triangulate(&screen);
        let summe: f32 = tris
            .iter()
            .map(|t| {
                let (a, b, c) = (t[0], t[1], t[2]);
                0.5 * ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs()
            })
            .sum();
        let soll = g.area() as f32 * k * k;
        assert!(tris.len() > 50, "nur {} Dreiecke", tris.len());
        assert!(
            (summe / soll - 1.0).abs() < 0.02,
            "Dreieckssumme {summe:.0} px² statt {soll:.0} px²"
        );
        // Kein Dreieck darf größer sein als die ganze Fläche.
        assert!(
            tris.iter().all(|t| {
                let (a, b, c) = (t[0], t[1], t[2]);
                0.5 * ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() <= soll
            }),
            "Dreieck größer als die Fläche"
        );
    }
}

/// Umrandung je Erstellmodus (Karte t_21567295): die Checkbox im
/// Eigenschaften-Panel bindet an genau ein Feld pro Modus.
#[cfg(test)]
mod tests_umrandung {
    use super::*;

    fn neue_app() -> App {
        App::with_ctx(egui::Context::default())
    }

    /// Liest die Umrandung des einzigen Fills im Dokument.
    fn umrandung(app: &App) -> bool {
        let e = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Fill { .. }))
            .expect("keine Füllfläche angelegt");
        match &e.kind {
            EntityKind::Fill { border, .. } => *border,
            _ => unreachable!(),
        }
    }

    fn anzahl_fills(app: &App) -> usize {
        app.doc()
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Fill { .. }))
            .count()
    }

    /// Rahmen aus vier Linien — die Umgrenzung, die der Eimer findet.
    fn rahmen(app: &mut App) {
        for (a, b) in [
            ([0.0, 0.0], [4.0, 0.0]),
            ([4.0, 0.0], [4.0, 3.0]),
            ([4.0, 3.0], [0.0, 3.0]),
            ([0.0, 3.0], [0.0, 0.0]),
        ] {
            run(app, "draw.line", json!({ "a": a, "b": b }));
        }
    }

    /// Die Vorgaben stehen im Modell des Panels: Punkte setzen an, Kontur
    /// anklicken und Farbeimer aus.
    #[test]
    fn vorgabe_je_erstellmodus() {
        let o = crate::app::ToolOptions::from_doc(&jjcad_core::model::Document::default());
        assert!(o.fill_border_for("draw.fill.points"), "Punkte setzen: an");
        assert!(o.fill_border_for("draw.fill.boundary"), "Umriss abfahren: an");
        assert!(!o.fill_border_for("draw.fill"), "Kontur anklicken: aus");
        assert!(!o.fill_border_for("draw.hatch"), "alter Name: aus");
        assert!(!o.fill_border_for("draw.fill.paint"), "Farbeimer: aus");
    }

    /// Das Panel schreibt in genau ein Feld je Modus — die Modi stören sich
    /// nicht gegenseitig.
    #[test]
    fn checkbox_schreibt_nur_ihren_modus() {
        let mut o = crate::app::ToolOptions::from_doc(&jjcad_core::model::Document::default());
        *o.fill_border_for_mut("draw.fill") = true;
        assert!(o.fill_border_for("draw.fill"));
        assert!(o.fill_border_for("draw.fill.points"), "unverändert an");
        assert!(!o.fill_border_for("draw.fill.paint"), "unverändert aus");

        *o.fill_border_for_mut("draw.fill.paint") = true;
        assert!(o.fill_border_for("draw.fill.paint"));
        assert!(o.fill_border_for("draw.fill.points"), "Punktmodus bleibt an");
    }

    /// Kontur anklicken: Klick auf eine geschlossene Polylinie ergibt eine
    /// Fläche ohne Umrandung (Vorgabe des Modus).
    #[test]
    fn kontur_anklicken_legt_flaeche_ohne_umrandung_an() {
        let mut app = neue_app();
        run(
            &mut app,
            "draw.polyline",
            json!({"pts": [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]], "closed": true}),
        );
        app.start_tool("draw.fill");
        // Klick auf die untere Kante (0,0)-(2,0)
        tool_click(&mut app, V2::new(1000.0, 0.0));
        assert_eq!(anzahl_fills(&app), 1, "eine Füllfläche angelegt");
        assert!(!umrandung(&app), "Kontur anklicken: keine Umrandung");

        // Checkbox an: derselbe Weg legt eine umrandete Fläche an. Frisches
        // Dokument, damit der Klick wieder die Polylinie trifft und nicht die
        // gerade entstandene Fläche.
        let mut mit_rand = neue_app();
        run(
            &mut mit_rand,
            "draw.polyline",
            json!({"pts": [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]], "closed": true}),
        );
        mit_rand.options.fill_border_outline = true;
        mit_rand.start_tool("draw.fill");
        tool_click(&mut mit_rand, V2::new(1000.0, 0.0));
        assert_eq!(anzahl_fills(&mit_rand), 1, "eine Füllfläche angelegt");
        assert!(umrandung(&mit_rand), "Checkbox an: Umrandung gesetzt");
    }

    /// Farbeimer: Klick in die leere Fläche ergibt eine Fläche ohne
    /// Umrandung; mit gesetzter Checkbox mit.
    #[test]
    fn farbeimer_folgt_der_checkbox() {
        let mut app = neue_app();
        rahmen(&mut app);
        app.start_tool("draw.fill.paint");
        tool_click(&mut app, V2::new(2000.0, 1500.0));
        assert_eq!(anzahl_fills(&app), 1, "der Eimer füllt die Fläche");
        assert!(!umrandung(&app), "Farbeimer: keine Umrandung");

        app.options.fill_border_paint = true;
        app.start_tool("draw.fill.paint");
        tool_click(&mut app, V2::new(1000.0, 1000.0));
        assert_eq!(anzahl_fills(&app), 2, "zweite Fläche gefüllt");
        let letzte = app
            .doc()
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Fill { .. }))
            .next_back()
            .unwrap();
        let EntityKind::Fill { border, .. } = &letzte.kind else {
            unreachable!()
        };
        assert!(*border, "Farbeimer mit Checkbox: Umrandung gesetzt");
    }
}

#[cfg(test)]
mod tests_werkzeug_bleibt_aktiv {
    //! Zeichenwerkzeug bleibt nach dem Objekt aktiv (t_ee4e8d3b).
    //!
    //! Rechteck, Kreis und Ellipse brauchen zwei Klicks. Nach dem Anlegen muss
    //! dasselbe Werkzeug sofort wieder bereitstehen (leerer Punktpuffer), damit
    //! mehrere Objekte nacheinander ohne erneutes Auswählen entstehen. Geprüft
    //! wird über den Klickpfad `tool_click` — denselben Weg, den die Maus geht —
    //! und an den erzeugten Objekten im Dokument, nicht an der Vorschau.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    /// Geschlossene Polylinien (Rechtecke) mit ihren Ecken.
    fn polylinien(app: &App) -> Vec<Vec<V2>> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Polyline { verts, .. } => Some(verts.iter().map(|v| v.p).collect()),
                _ => None,
            })
            .collect()
    }

    /// Kreise als (Mittelpunkt, Radius).
    fn kreise(app: &App) -> Vec<(V2, f64)> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Circle { c, r } => Some((*c, *r)),
                _ => None,
            })
            .collect()
    }

    /// Ellipsen als (Mittelpunkt, rx, ry).
    fn ellipsen(app: &App) -> Vec<(V2, f64, f64)> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Ellipse { c, rx, ry, .. } => Some((*c, *rx, *ry)),
                _ => None,
            })
            .collect()
    }

    /// Zwei Rechtecke nacheinander — das zweite ohne erneutes `start_tool`.
    #[test]
    fn zwei_rechtecke_ohne_erneutes_auswaehlen() {
        let mut app = app();
        app.start_tool("draw.rect");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));

        assert_eq!(polylinien(&app).len(), 1, "erstes Rechteck angelegt");
        assert_eq!(app.tool_id(), Some("draw.rect"), "Werkzeug bleibt aktiv");
        assert!(
            app.tool.as_ref().unwrap().pts.is_empty(),
            "kein Restzustand: der Startpunkt des ersten Rechtecks ist verworfen"
        );

        // zweites Rechteck, ohne das Werkzeug neu zu wählen
        tool_click(&mut app, V2::new(3000.0, 0.0));
        tool_click(&mut app, V2::new(4000.0, 500.0));

        let alle = polylinien(&app);
        assert_eq!(alle.len(), 2, "zweites Rechteck angelegt");
        assert_eq!(
            alle[1],
            vec![
                V2::new(3000.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 500.0),
                V2::new(3000.0, 500.0),
            ],
            "Ecken des zweiten Rechtecks stammen aus den neuen Klicks"
        );
        assert_eq!(app.tool_id(), Some("draw.rect"), "nach dem zweiten weiter aktiv");
        assert!(app.tool.as_ref().unwrap().pts.is_empty());
    }

    /// Zwei Kreise nacheinander, jeweils mit eigenem Mittelpunkt und Radius.
    #[test]
    fn zwei_kreise_ohne_erneutes_auswaehlen() {
        let mut app = app();
        app.start_tool("draw.circle");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 0.0));

        assert_eq!(kreise(&app).len(), 1, "erster Kreis angelegt");
        assert_eq!(app.tool_id(), Some("draw.circle"), "Werkzeug bleibt aktiv");
        assert!(app.tool.as_ref().unwrap().pts.is_empty());

        tool_click(&mut app, V2::new(5000.0, 0.0));
        tool_click(&mut app, V2::new(5500.0, 0.0));

        let alle = kreise(&app);
        assert_eq!(alle.len(), 2, "zweiter Kreis angelegt");
        assert!((alle[0].1 - 1000.0).abs() < 1e-6, "Radius 1: {}", alle[0].1);
        assert_eq!(alle[1].0, V2::new(5000.0, 0.0), "neuer Mittelpunkt");
        assert!((alle[1].1 - 500.0).abs() < 1e-6, "Radius 2: {}", alle[1].1);
        assert_eq!(app.tool_id(), Some("draw.circle"));
    }

    /// Ellipse verhält sich gleich (gleiche Zwei-Klick-Klasse).
    #[test]
    fn zwei_ellipsen_ohne_erneutes_auswaehlen() {
        let mut app = app();
        app.start_tool("draw.ellipse");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 500.0));
        tool_click(&mut app, V2::new(4000.0, 1000.0));
        tool_click(&mut app, V2::new(4600.0, 1300.0));

        let alle = ellipsen(&app);
        assert_eq!(alle.len(), 2, "zwei Ellipsen");
        assert!((alle[1].1 - 600.0).abs() < 1e-6, "rx: {}", alle[1].1);
        assert!((alle[1].2 - 300.0).abs() < 1e-6, "ry: {}", alle[1].2);
        assert_eq!(app.tool_id(), Some("draw.ellipse"));
    }

    /// Ein unbrauchbarer zweiter Klick legt kein Objekt an, behält aber den
    /// Startpunkt — der nächste Klick zählt als zweiter.
    #[test]
    fn entarteter_zweiter_klick_behaelt_den_startpunkt() {
        let mut app = app();
        app.start_tool("draw.rect");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 1000.0)); // gleiche x-Lage → kein Rechteck

        assert!(polylinien(&app).is_empty(), "kein entartetes Rechteck angelegt");
        assert_eq!(app.tool_id(), Some("draw.rect"));
        let pts = &app.tool.as_ref().unwrap().pts;
        assert_eq!(pts.len(), 1, "Startpunkt bleibt stehen");
        assert_eq!(pts[0], V2::new(0.0, 0.0));

        tool_click(&mut app, V2::new(2000.0, 1000.0));
        assert_eq!(polylinien(&app).len(), 1, "der nächste Klick ergänzt das Rechteck");
    }

    /// Beenden geht weiterhin — danach entsteht durch Klicks kein Objekt mehr.
    #[test]
    fn beenden_wirkt_weiterhin() {
        let mut app = app();
        app.start_tool("draw.rect");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 1000.0));
        assert_eq!(app.tool_id(), Some("draw.rect"));

        app.cancel_tool();
        assert!(app.tool.is_none(), "ESC/Abbrechen beendet das Werkzeug");

        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(3000.0, 1000.0));
        assert_eq!(polylinien(&app).len(), 1, "ohne Werkzeug entsteht nichts");
    }

    /// Einfügemodus „Basislinie + Höhe": drei Klicks (Anfang, Ende, Höhe)
    /// ergeben ein Rechteck in beliebiger Lage. Der Abstand in Achsrichtung
    /// zählt nicht — nur der senkrechte, sonst kippt die Höhe beim Ziehen.
    #[test]
    fn rechteck_ueber_basislinie_und_hoehe() {
        let mut app = app();
        app.options.rect_mode = Einfuegemodus::Basislinie;
        app.start_tool("draw.rect");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(3000.0, 3000.0));
        assert!(polylinien(&app).is_empty(), "ohne Höhe entsteht nichts");

        // 1000 mm links der 45°-Basislinie, zusätzlich 500 mm in Achsrichtung:
        // der Versatz entlang der Linie darf die Höhe nicht verändern.
        let n = V2::new(3000.0, 3000.0)
            .normalized()
            .perp()
            * 1000.0;
        let klick = V2::new(3000.0, 3000.0) + n + V2::new(353.553, 353.553);
        tool_click(&mut app, klick);

        let alle = polylinien(&app);
        assert_eq!(alle.len(), 1, "Rechteck angelegt");
        let p = &alle[0];
        assert_eq!(p.len(), 4, "vier Ecken");
        assert!(p[0].dist(V2::new(0.0, 0.0)) < 1e-6, "{:?}", p[0]);
        assert!(p[1].dist(V2::new(3000.0, 3000.0)) < 1e-6, "{:?}", p[1]);
        assert!(
            (p[1].dist(p[2]) - 1000.0).abs() < 1e-6,
            "Höhe 1000 mm: {}",
            p[1].dist(p[2])
        );
        let basis = p[1] - p[0];
        let hoehe = p[2] - p[1];
        assert!(
            (basis.x * hoehe.x + basis.y * hoehe.y).abs() < 1.0,
            "Höhe steht senkrecht auf der Basislinie"
        );
        assert_eq!(app.tool_id(), Some("draw.rect"), "Werkzeug bleibt aktiv");
        assert!(app.tool.as_ref().unwrap().pts.is_empty());
    }

    /// Ein dritter Klick auf der Basislinie legt nichts an und behält die
    /// Basislinie — der nächste Klick zählt als Höhe.
    #[test]
    fn basislinie_ohne_hoehe_behaelt_die_basis() {
        let mut app = app();
        app.options.rect_mode = Einfuegemodus::Basislinie;
        app.start_tool("draw.rect");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(3000.0, 0.0));
        tool_click(&mut app, V2::new(1500.0, 0.0)); // auf der Basislinie
        assert!(polylinien(&app).is_empty(), "Höhe null legt nichts an");
        assert_eq!(app.tool.as_ref().unwrap().pts.len(), 2, "Basislinie bleibt");

        tool_click(&mut app, V2::new(1500.0, 800.0));
        assert_eq!(polylinien(&app).len(), 1, "der nächste Klick setzt die Höhe");
    }

    /// Einfügemodus „Mittelachse + Breite": die Mittelachse liegt durch den
    /// Mittelpunkt, ihre Länge ist 2 × rx, der Abstand des dritten Klicks die
    /// halbe Breite. Die Ellipse trägt die Drehlage der Achse.
    #[test]
    fn ellipse_ueber_mittelachse_und_breite() {
        let mut app = app();
        app.options.ellipse_mode = Einfuegemodus::Basislinie;
        app.start_tool("draw.ellipse");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 2000.0));
        assert!(ellipsen(&app).is_empty(), "ohne Breite entsteht nichts");

        let n = V2::new(2000.0, 2000.0).normalized().perp() * 500.0;
        tool_click(&mut app, V2::new(2000.0, 2000.0) + n);

        let alle = ellipsen(&app);
        assert_eq!(alle.len(), 1, "Ellipse angelegt");
        let (c, rx, ry) = alle[0];
        assert!(c.dist(V2::new(1000.0, 1000.0)) < 1e-6, "Mittelpunkt: {c:?}");
        assert!((rx - 1414.2135623730951).abs() < 0.01, "rx: {rx}");
        assert!((ry - 500.0).abs() < 1e-6, "halbe Breite: {ry}");
        let rot = app
            .doc()
            .entities
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Ellipse { rot, .. } => Some(*rot),
                _ => None,
            })
            .unwrap();
        assert!((rot - std::f64::consts::FRAC_PI_4).abs() < 1e-9, "Drehlage: {rot}");
        assert_eq!(app.tool_id(), Some("draw.ellipse"));
        assert!(app.tool.as_ref().unwrap().pts.is_empty());
    }

    /// Ohne Umschalten bleibt der bisherige Weg (zwei Klicks) unverändert.
    #[test]
    fn einfuegemodus_ist_vorgabe_achsenparallel() {
        let mut app = app();
        assert_eq!(app.options.rect_mode, Einfuegemodus::Achsenparallel);
        assert_eq!(app.options.ellipse_mode, Einfuegemodus::Achsenparallel);
        app.start_tool("draw.rect");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));
        assert_eq!(polylinien(&app).len(), 1, "zwei Klicks genügen weiterhin");
    }
}

#[cfg(test)]
mod tests_tasten_punktfolge {
    //! Tasten im Modus „Punkte setzen" (Karte t_d72aabf3).
    //!
    //! Füllflächen und Schraffuren laufen über dieselben Action-IDs
    //! (`draw.fill.points`, `draw.fill.boundary`, alter Name `draw.hatch`); der
    //! Unterschied ist nur das Muster in den Werkzeugoptionen. Enter und ESC
    //! bestätigen die Punktfolge, Entf und Backspace brechen ab.
    //!
    //! Geprüft wird mit echten Tastendrücken durch die volle Oberfläche
    //! (`app.ui`) — ein direkter Aufruf von `finish_tool` würde die
    //! Tastenverdrahtung nicht belegen — und an den Objekten im Dokument, nie
    //! an der Vorschau.

    use super::*;
    use jjcad_core::style::FillPattern;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    /// Ein Frame mit genau einem Tastendruck, wie ihn das laufende Programm
    /// bekommt (ohne Zeiger, ohne Fenster).
    fn taste(app: &mut App, key: egui::Key) {
        let ctx = app.ctx.clone();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    /// Drei Klicks: Dreieck 2 m × 1 m (1 m²) — Werkzeug läuft danach noch.
    fn dreieck(app: &mut App) {
        tool_click(app, V2::new(0.0, 0.0));
        tool_click(app, V2::new(2000.0, 0.0));
        tool_click(app, V2::new(0.0, 1000.0));
    }

    /// Muster der angelegten Füllflächen.
    fn muster(app: &App) -> Vec<FillPattern> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Fill { pattern, .. } => Some(*pattern),
                _ => None,
            })
            .collect()
    }

    /// Ein Durchlauf für ein Muster und eine Taste: (Objekte, Werkzeug läuft).
    fn ergebnis(pattern: FillPattern, key: egui::Key) -> (usize, bool) {
        let mut app = app();
        app.options.fill_pattern = pattern;
        app.start_tool("draw.fill.points");
        dreieck(&mut app);
        taste(&mut app, key);
        (app.doc().entities.len(), app.tool.is_some())
    }

    #[test]
    fn enter_bestaetigt_fuellflaeche() {
        let mut app = app();
        app.options.fill_pattern = FillPattern::Solid;
        app.start_tool("draw.fill.points");
        dreieck(&mut app);
        taste(&mut app, egui::Key::Enter);
        assert_eq!(app.doc().entities.len(), 1, "Enter legt die Fläche an");
        assert!(app.tool.is_none(), "Enter beendet das Werkzeug");
        assert_eq!(muster(&app), vec![FillPattern::Solid]);
    }

    /// Kern der Karte: ESC bestätigt, statt zu verwerfen.
    #[test]
    fn esc_bestaetigt_fuellflaeche() {
        let mut app = app();
        app.options.fill_pattern = FillPattern::Solid;
        app.start_tool("draw.fill.points");
        dreieck(&mut app);
        taste(&mut app, egui::Key::Escape);

        assert_eq!(app.doc().entities.len(), 1, "ESC legt die Fläche an");
        assert!(app.tool.is_none(), "ESC beendet das Werkzeug");
        let ecken = match &app.doc().entities[0].kind {
            EntityKind::Fill { boundary, .. } => boundary.len(),
            _ => panic!("kein Fill angelegt"),
        };
        assert_eq!(ecken, 3, "drei Eckpunkte, keine vierte Ecke");
    }

    /// Schraffur ist dasselbe Werkzeug mit anderem Muster — ESC bestätigt auch
    /// dort und das Muster kommt aus den Werkzeugoptionen.
    #[test]
    fn esc_bestaetigt_schraffur() {
        let mut app = app();
        app.options.fill_pattern = FillPattern::Lines;
        app.start_tool("draw.fill.points");
        dreieck(&mut app);
        taste(&mut app, egui::Key::Escape);

        assert_eq!(app.doc().entities.len(), 1, "ESC legt die Schraffur an");
        assert!(app.tool.is_none());
        assert_eq!(muster(&app), vec![FillPattern::Lines], "Schraffur 45°");
    }

    /// Umriss-Modus (`draw.fill.boundary`) ist dieselbe Punktfolge — ESC
    /// bestätigt dort genauso.
    #[test]
    fn esc_bestaetigt_den_umriss_modus() {
        let mut app = app();
        app.start_tool("draw.fill.boundary");
        dreieck(&mut app);
        taste(&mut app, egui::Key::Escape);
        assert_eq!(app.doc().entities.len(), 1);
        assert!(app.tool.is_none());
    }

    /// Entf und Backspace brechen ab — ohne Objekt, Werkzeug beendet.
    #[test]
    fn entf_und_backspace_brechen_ab() {
        for (key, name) in [
            (egui::Key::Delete, "Entf"),
            (egui::Key::Backspace, "Backspace"),
        ] {
            let mut app = app();
            app.start_tool("draw.fill.points");
            dreieck(&mut app);
            taste(&mut app, key);
            assert!(app.doc().entities.is_empty(), "{name}: nichts angelegt");
            assert!(app.tool.is_none(), "{name}: Werkzeug beendet");
        }
    }

    /// Schraffur und Füllfläche verhalten sich in allen vier Tasten gleich:
    /// Enter/ESC legen genau eine Fläche an, Entf/Backspace keine.
    #[test]
    fn schraffur_verhaelt_sich_wie_fuellflaeche() {
        let faelle = [
            (egui::Key::Enter, "Enter", (1usize, false)),
            (egui::Key::Escape, "ESC", (1, false)),
            (egui::Key::Delete, "Entf", (0, false)),
            (egui::Key::Backspace, "Backspace", (0, false)),
        ];
        for (key, name, soll) in faelle {
            let flaeche = ergebnis(FillPattern::Solid, key);
            let schraffur = ergebnis(FillPattern::Lines, key);
            assert_eq!(flaeche, schraffur, "{name}: Schraffur wie Füllfläche");
            assert_eq!(flaeche, soll, "{name}: erwartetes Ergebnis");
        }
    }

    /// Eine unfertige Punktfolge (ein Punkt) wird von ESC weiterhin verworfen —
    /// ohne mindestens zwei Punkte gibt es nichts zu bestätigen.
    #[test]
    fn esc_verwirft_einen_einzelnen_punkt() {
        let mut app = app();
        app.start_tool("draw.fill.points");
        tool_click(&mut app, V2::new(0.0, 0.0));
        taste(&mut app, egui::Key::Escape);
        assert!(app.tool.is_none(), "ESC beendet das Werkzeug");
        assert!(app.doc().entities.is_empty());
    }

    /// Zwei Punkte reichen für keine Fläche: ESC versucht den Abschluss (wie
    /// Enter), die Punkte bleiben für weitere Klicks stehen.
    #[test]
    fn esc_bei_zwei_punkten_behaelt_die_punkte() {
        let mut app = app();
        app.start_tool("draw.fill.points");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        taste(&mut app, egui::Key::Escape);
        assert!(app.doc().entities.is_empty(), "keine Fläche aus zwei Punkten");
        assert_eq!(
            app.tool.as_ref().map(|t| t.pts.len()),
            Some(2),
            "die gesetzten Punkte bleiben erhalten"
        );
    }

    /// Backspace gehört der Tippeingabe, solange eine Zahl im Puffer steht —
    /// dann bricht es das Werkzeug nicht ab.
    #[test]
    fn backspace_loescht_getippte_zahl_statt_abzubrechen() {
        let mut app = app();
        app.start_tool("draw.fill.points");
        dreieck(&mut app);
        app.ui.typed = "12".into();
        taste(&mut app, egui::Key::Backspace);
        assert!(app.tool.is_some(), "Werkzeug läuft weiter");
        assert!(app.doc().entities.is_empty(), "nichts angelegt");
        assert_eq!(app.ui.typed, "1", "Backspace löscht die getippte Ziffer");
    }

    /// Polylinie bleibt unverändert: ESC schließt ab (wie bisher).
    #[test]
    fn polylinie_bestaetigt_mit_esc_wie_bisher() {
        let mut app = app();
        app.start_tool("draw.polyline");
        dreieck(&mut app);
        taste(&mut app, egui::Key::Escape);
        assert_eq!(app.doc().entities.len(), 1, "ESC schließt die Polylinie ab");
        match &app.doc().entities[0].kind {
            EntityKind::Polyline { verts, .. } => assert_eq!(verts.len(), 3),
            _ => panic!("keine Polylinie angelegt"),
        }
        assert!(app.tool.is_none());
    }

    /// Wand bleibt unverändert: ESC bricht ab, es entsteht keine Wand.
    #[test]
    fn wand_bricht_mit_esc_ab_wie_bisher() {
        let mut app = app();
        app.start_tool("draw.wall");
        tool_click(&mut app, V2::new(0.0, 0.0));
        taste(&mut app, egui::Key::Escape);
        assert!(app.doc().entities.is_empty(), "keine Wand angelegt");
        assert!(app.tool.is_none(), "ESC beendet das Wandwerkzeug");
    }

    // ---- Geschlossene Polylinie (Karte t_8b6feebf) ----

    fn polylinie_stand(app: &App) -> (Vec<V2>, bool) {
        app.doc()
            .entities
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Polyline { verts, closed } => {
                    Some((verts.iter().map(|w| w.p).collect(), *closed))
                }
                _ => None,
            })
            .expect("keine Polylinie angelegt")
    }

    /// Klick auf den Startpunkt schließt den Zug: drei Stützpunkte, Closed-Flag,
    /// Werkzeug beendet — kein vierter (doppelter) Punkt.
    #[test]
    fn klick_auf_startpunkt_schliesst_die_polylinie() {
        let mut app = app();
        app.start_tool("draw.polyline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));
        tool_click(&mut app, V2::new(0.0, 0.0));
        assert!(app.tool.is_none(), "der Startklick beendet die Eingabe");
        let (pts, closed) = polylinie_stand(&app);
        assert!(closed, "Closed-Flag fehlt");
        assert_eq!(pts.len(), 3, "der doppelte Endpunkt wird nicht gespeichert");
        assert!(
            pts.last().unwrap().dist(V2::new(0.0, 0.0)) > 1.0,
            "der Startpunkt steht nicht doppelt in der Punktliste"
        );
    }

    /// Ohne Startklick bleibt die Polylinie offen (ESC wie bisher).
    #[test]
    fn polylinie_ohne_startklick_bleibt_offen() {
        let mut app = app();
        app.start_tool("draw.polyline");
        dreieck(&mut app);
        taste(&mut app, egui::Key::Escape);
        let (pts, closed) = polylinie_stand(&app);
        assert!(!closed, "offene Punktfolge wird nicht geschlossen");
        assert_eq!(pts.len(), 3);
    }

    /// Zweimal auf dieselbe Stelle geklickt: der Punkt wird nicht doppelt
    /// gespeichert (keine Strecke der Länge null).
    #[test]
    fn doppelklick_erzeugt_keinen_doppelten_punkt() {
        let mut app = app();
        app.start_tool("draw.polyline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        finish_tool(&mut app, true);
        let (pts, closed) = polylinie_stand(&app);
        assert_eq!(pts.len(), 2, "der wiederholte Punkt entfällt");
        assert!(!closed);
    }

    /// Geschlossener Zug lässt sich mit der Befehls-Schicht speichern und laden;
    /// nach dem Laden steht das Closed-Flag wieder (Datei, nicht nur Modell).
    #[test]
    fn geschlossene_polylinie_ueberlebt_speichern_und_laden() {
        let mut app = app();
        app.start_tool("draw.polyline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));
        tool_click(&mut app, V2::new(0.0, 0.0));
        let text = jjcad_core::persist::to_json(app.doc()).unwrap();
        let geladen = jjcad_core::persist::from_json(&text).unwrap();
        match &geladen.entities[0].kind {
            EntityKind::Polyline { verts, closed } => {
                assert!(*closed, "Closed-Flag geht beim Speichern verloren");
                assert_eq!(verts.len(), 3);
            }
            _ => panic!("keine Polylinie geladen"),
        }
    }
}

mod tests_fang_polylinie {
    //! Punktfang bei der Polylinien-Eingabe (Karte t_809d529d).
    //!
    //! Geprüft wird die Verdrahtung der Zeichenfläche, nicht die Fang-Engine
    //! selbst (die deckt `crates/jjcad-core/tests/snap_polylinie.rs` ab): der
    //! Zeiger, den `show` anzeigt, und der Punkt, den der Klick übernimmt,
    //! laufen beide durch `fang_zeiger`. Mit Ortho-Zwang (der Fehlerfall) liegt
    //! das vorhandene Linienende neben der Zwangsachse — es muss trotzdem
    //! fangen und unverändert in der Polylinie landen.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    /// Ortho an, Polar aus, Raster aus: die Zwangsrichtung, unter der der
    /// Objektfang vorher unerreichbar war.
    fn ortho_app() -> App {
        let mut app = app();
        let _ = app.session_mut().execute(
            "doc.settings",
            &serde_json::json!({"input_unit": "mm",
                               "snap": {"ortho": true, "polar": false, "grid": false}}),
        );
        let _ = app
            .session_mut()
            .execute("draw.line", &serde_json::json!({"a": [0.0, 0.0], "b": [2000.0, 0.0]}));
        app
    }

    fn polylinien_punkte(app: &App) -> Vec<V2> {
        app.doc()
            .entities
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Polyline { verts, .. } => Some(verts.iter().map(|w| w.p).collect()),
                _ => None,
            })
            .expect("keine Polylinie angelegt")
    }

    /// Zeiger 8 mm neben dem Linienende, vom letzten Polylinien-Punkt (0; 2000)
    /// aus gesehen neben der Ortho-Achse: der Fang muss auf (2000; 0) springen
    /// und den Endpunkt melden.
    #[test]
    fn zeiger_faengt_linienende_trotz_ortho() {
        let mut app = ortho_app();
        app.start_tool("draw.polyline");
        tool_click(&mut app, V2::new(0.0, 2000.0));

        let (p, label) = fang_zeiger(&app, V2::new(1992.0, 8.0), 40.0);
        assert_eq!(label, Some("Endpunkt"), "Snap-Indikator in der Statuszeile");
        assert!(
            p.dist(V2::new(2000.0, 0.0)) < 1e-9,
            "Fangpunkt muss das Linienende sein, ist {p:?}"
        );
    }

    /// Der übernommene Punkt ist der Fangpunkt: nach dem Klick steht in der
    /// gespeicherten Polylinie exakt (2000; 0) — keine Rundungsabweichung.
    #[test]
    fn uebernommener_punkt_ist_der_fangpunkt() {
        let mut app = ortho_app();
        app.start_tool("draw.polyline");
        tool_click(&mut app, V2::new(0.0, 2000.0));

        let (p, _) = fang_zeiger(&app, V2::new(1992.0, 8.0), 40.0);
        tool_click(&mut app, p);
        finish_tool(&mut app, true);

        let pts = polylinien_punkte(&app);
        assert_eq!(pts.len(), 2, "zwei Stützpunkte");
        assert!(pts[1].dist(p) < 1e-9, "gespeichert {:?}, gefangen {p:?}", pts[1]);
        assert!(pts[1].dist(V2::new(2000.0, 0.0)) < 1e-9, "{:?}", pts[1]);
    }

    /// Fangradius in Bildschirm-Pixeln: bei doppeltem Zoom halbiert sich der
    /// Radius in Weltkoordinaten — derselbe Zeigerabstand in Pixeln fängt
    /// weiterhin.
    #[test]
    fn fangradius_ist_bildschirmkonstant() {
        let app = ortho_app();
        let k = app.doc().settings.snap.tolerance_px;
        assert!((fang_radius_px(&app, 1.0) - k).abs() < 1e-9);
        assert!((fang_radius_px(&app, 2.0) - k / 2.0).abs() < 1e-9);
        assert!((fang_radius_px(&app, 0.5) - k * 2.0).abs() < 1e-9);
    }
}


mod tests_auswahl_bereich {
    //! Fensterauswahl, die außerhalb der Zeichenfläche bzw. außerhalb des
    //! Fensters endet (T0023).
    //!
    //! `handle_input` bricht ab, sobald der Zeiger nicht über der Zeichenfläche
    //! steht. Wer die Maus gedrückt hielt und über das Ribbon, die Panels oder
    //! aus dem Fenster hinausfuhr, ließ das Auswahlfenster deshalb stehen — es
    //! blieb samt Einfärbung dauerhaft in der Zeichnung.

    use super::*;
    use jjcad_core::persist::DragMode;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        // Ziehen als Gedrückthalten: unabhängig von den Einstellungen des Rechners.
        app.ui.drag_mode = DragMode::HoldAndDrag;
        app.session_mut()
            .execute(
                "doc.settings",
                &serde_json::json!({"input_unit": "mm",
                                   "snap": {"ortho": false, "polar": false, "grid": false}}),
            )
            .unwrap();
        app
    }

    fn frame(app: &mut App, events: Vec<egui::Event>) {
        let ctx = app.ctx.clone();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    fn druck(pos: Pos2) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn loslassen(pos: Pos2) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Linie quer über der Zeichenfläche. Liefert (App, Linien-Id,
    /// Startpunkt des Auswahlfensters, entfernten Eckpunkt) in Bildschirm-Pixeln.
    fn szenario() -> (App, Id, Pos2, Pos2) {
        let mut app = app();
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let a = Pos2::new(rect.left() + 200.0, rect.top() + 150.0);
        let b = Pos2::new(rect.left() + 600.0, rect.top() + 400.0);
        let (wa, wb) = (
            screen_to_world(rect, view, a),
            screen_to_world(rect, view, b),
        );
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [wa.x, wa.y], "b": [wb.x, wb.y]}),
            )
            .expect("Linie");
        let id = app.doc().entities[0].id;
        let start = Pos2::new(rect.left() + 60.0, rect.top() + 60.0);
        let ende = Pos2::new(b.x + 60.0, b.y + 60.0);
        assert!(
            rect.contains(start) && rect.contains(ende),
            "Auswahlfenster muss in der Zeichenfläche liegen"
        );
        (app, id, start, ende)
    }

    /// Auswahlfenster aufziehen und die Taste gedrückt lassen.
    ///
    /// Nach dem Druck ein kurzer Weg: egui beginnt den Zug erst, wenn der Zeiger
    /// sich bewegt hat, und der Zugbeginn ist die Stelle dieses Bildes.
    fn zug_bis(app: &mut App, start: Pos2, ende: Pos2) {
        frame(app, vec![egui::Event::PointerMoved(start)]);
        frame(app, vec![druck(start)]);
        frame(app, vec![egui::Event::PointerMoved(start + Vec2::new(8.0, 8.0))]);
        frame(app, vec![egui::Event::PointerMoved(ende)]);
        assert!(app.select_drag.is_some(), "Auswahlfenster muss laufen");
    }

    /// Loslassen über dem Ribbon (im Fenster, außerhalb der Zeichenfläche): das
    /// Auswahlfenster darf nicht stehen bleiben.
    #[test]
    fn loslassen_ueber_dem_ribbon_raeumt_das_auswahlfenster_ab() {
        let (mut app, _id, start, ende) = szenario();
        zug_bis(&mut app, start, ende);

        // Zeiger über das Ribbon schieben (Taste liegt noch) — kein Abbruch.
        let ribbon = Pos2::new(ende.x, 8.0);
        frame(&mut app, vec![egui::Event::PointerMoved(ribbon)]);
        assert!(
            app.select_drag.is_some(),
            "solange die Taste liegt, läuft der Zug weiter"
        );

        frame(&mut app, vec![loslassen(ribbon)]);
        assert!(
            app.select_drag.is_none(),
            "Loslassen außerhalb der Zeichenfläche lässt das Auswahlfenster stehen"
        );
    }

    /// Loslassen rechts neben der Zeichenfläche: der Endpunkt folgt dem Rand,
    /// die Auswahl wird übernommen — nichts bleibt hängen.
    #[test]
    fn loslassen_neben_der_flaeche_waehlt_bis_zum_rand() {
        let (mut app, id, start, ende) = szenario();
        zug_bis(&mut app, start, ende);

        let daneben = Pos2::new(app.ui.last_canvas.right() + 30.0, ende.y);
        frame(&mut app, vec![egui::Event::PointerMoved(daneben)]);
        frame(&mut app, vec![loslassen(daneben)]);

        assert!(app.select_drag.is_none(), "Auswahlfenster abgeräumt");
        assert_eq!(app.selection(), &[id], "Linie im Zug bis zum Rand");
    }

    /// Verliert das Fenster den Zeiger (X11 ohne Mausgriff: das Loslassen kommt
    /// nie an), endet der Zug ebenfalls — die Auswahl bleibt nicht stehen.
    #[test]
    fn zeiger_verlaesst_das_fenster_beendet_die_auswahl() {
        let (mut app, id, start, ende) = szenario();
        zug_bis(&mut app, start, ende);

        frame(&mut app, vec![egui::Event::PointerGone]);
        assert!(
            app.select_drag.is_none(),
            "abgelöster Zeiger lässt das Auswahlfenster stehen"
        );
        assert_eq!(app.selection(), &[id], "bis zum letzten Zeigerstand ausgewählt");
    }

    /// Gegenprobe: das Loslassen in der Fläche wählt weiterhin wie bisher.
    #[test]
    fn loslassen_in_der_flaeche_waehlt_die_linie() {
        let (mut app, id, start, ende) = szenario();
        zug_bis(&mut app, start, ende);

        frame(&mut app, vec![loslassen(ende)]);
        assert!(app.select_drag.is_none());
        assert_eq!(app.selection(), &[id]);
    }

    /// Derselbe Fehler beim Griffzug: verliert das Fenster den Zeiger, endet der
    /// Zug, statt am letzten Stand hängen zu bleiben.
    #[test]
    fn zeiger_verlaesst_das_fenster_beendet_den_griffzug() {
        let mut app = app();
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [2000.0, 1500.0]]}),
            )
            .expect("Polylinie");
        let poly = app.doc().entities[0].id;
        app.session_mut().select(vec![poly]);
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let griff = world_to_screen(rect, view, V2::new(0.0, 0.0));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");

        frame(&mut app, vec![egui::Event::PointerGone]);
        assert!(
            app.grip_drag.is_none(),
            "abgelöster Zeiger lässt den Griffzug stehen"
        );
    }
}


mod tests_fang_vorschau {
    //! Punktfang auf die eigene Vorschau bei der Polylinien-Eingabe (Ticket T0002).
    //!
    //! Während des Zeichnens steht die Polylinie noch nicht im Modell; ihre
    //! Stützpunkte kommen über `vorschau_punkte` in die Fang-Engine. Geprüft wird
    //! der Weg der Zeichenfläche (`fang_zeiger` für Anzeige *und* Klickpunkt) —
    //! mit Ortho, weil dessen Achse den Anfangspunkt sonst verdeckt.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    /// Ortho an, Polar und Raster aus.
    fn ortho_app() -> App {
        let mut app = app();
        let _ = app.session_mut().execute(
            "doc.settings",
            &serde_json::json!({"input_unit": "mm",
                               "snap": {"ortho": true, "polar": false, "grid": false}}),
        );
        app
    }

    /// Offenes Dreieck der laufenden Eingabe: (0;0) → (2000;0) → (2000;1500).
    fn dreieck_im_werkzeug(app: &mut App) {
        app.start_tool("draw.polyline");
        tool_click(app, V2::new(0.0, 0.0));
        tool_click(app, V2::new(2000.0, 0.0));
        tool_click(app, V2::new(2000.0, 1500.0));
    }

    fn polylinie_stand(app: &App) -> (Vec<V2>, bool) {
        app.doc()
            .entities
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Polyline { verts, closed } => {
                    Some((verts.iter().map(|w| w.p).collect(), *closed))
                }
                _ => None,
            })
            .expect("keine Polylinie angelegt")
    }

    /// Der Zeiger fängt auf dem eigenen Anfang — trotz Ortho, dessen Achse vom
    /// letzten Stützpunkt (2000;1500) aus weit am Anfang vorbeiläuft.
    #[test]
    fn zeiger_faengt_eigenen_anfang_trotz_ortho() {
        let mut app = ortho_app();
        dreieck_im_werkzeug(&mut app);

        let (p, label) = fang_zeiger(&app, V2::new(10.0, -6.0), 40.0);
        assert_eq!(label, Some("Endpunkt"), "Snap-Indikator in der Statuszeile");
        assert!(
            p.dist(V2::new(0.0, 0.0)) < 1e-9,
            "Fangpunkt muss der Anfang der Vorschau sein, ist {p:?}"
        );
    }

    /// Klick auf den gefangenen Anfang schließt den Zug: Closed-Flag, drei
    /// Stützpunkte, Werkzeug beendet — auch mit Ortho.
    #[test]
    fn klick_auf_gefangenen_anfang_schliesst_trotz_ortho() {
        let mut app = ortho_app();
        dreieck_im_werkzeug(&mut app);

        let (p, _) = fang_zeiger(&app, V2::new(10.0, -6.0), 40.0);
        tool_click(&mut app, p);
        assert!(app.tool.is_none(), "der Startklick beendet die Eingabe");

        let (pts, closed) = polylinie_stand(&app);
        assert!(closed, "Closed-Flag fehlt");
        assert_eq!(pts.len(), 3, "kein doppelter Endpunkt");
        assert!(pts[0].dist(V2::new(0.0, 0.0)) < 1e-9, "Anfang {:?}", pts[0]);
    }

    /// Ein Zwischenpunkt der Vorschau ist Fangziel: der Klick landet exakt auf
    /// der früheren Ecke.
    #[test]
    fn zwischenpunkt_der_vorschau_faengt() {
        let mut app = app();
        let _ = app.session_mut().execute(
            "doc.settings",
            &serde_json::json!({"input_unit": "mm",
                               "snap": {"ortho": false, "polar": false, "grid": false}}),
        );
        dreieck_im_werkzeug(&mut app);

        let (p, label) = fang_zeiger(&app, V2::new(1994.0, 8.0), 40.0);
        assert_eq!(label, Some("Endpunkt"));
        assert!(p.dist(V2::new(2000.0, 0.0)) < 1e-9, "ist {p:?}");
    }

    /// Der zuletzt gesetzte Punkt bleibt Bezugspunkt und zieht den Zeiger nicht
    /// auf sich selbst — sonst käme die nächste Strecke nicht vom Fleck.
    #[test]
    fn letzter_punkt_zieht_den_zeiger_nicht() {
        let mut app = app();
        let _ = app.session_mut().execute(
            "doc.settings",
            &serde_json::json!({"input_unit": "mm",
                               "snap": {"ortho": false, "polar": false, "grid": false}}),
        );
        app.start_tool("draw.polyline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));

        let cursor = V2::new(1992.0, 8.0);
        let (p, label) = fang_zeiger(&app, cursor, 40.0);
        assert_eq!(label, None, "kein Fang am eigenen Bezugspunkt");
        assert!(p.dist(cursor) < 1e-9, "Zeiger bleibt frei, ist {p:?}");
    }

    /// Weniger als zwei Punkte haben keine Vorschau — der einzige Punkt ist
    /// immer der Bezugspunkt der nächsten Strecke.
    #[test]
    fn ein_punkt_hat_keine_vorschau() {
        let mut app = app();
        assert!(vorschau_punkte(&app).is_empty(), "ohne Werkzeug leer");
        app.start_tool("draw.polyline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        assert!(vorschau_punkte(&app).is_empty(), "ein Punkt ist Bezugspunkt");
        tool_click(&mut app, V2::new(2000.0, 0.0));
        assert_eq!(
            vorschau_punkte(&app),
            vec![V2::new(0.0, 0.0)],
            "ab zwei Punkten fängt der Anfang"
        );
    }
}

mod tests_fang_griff {
    //! Punktfang beim Bearbeiten einer ausgewählten Polylinie (Karte t_eb950a21).
    //!
    //! Geprüft wird der Weg der Zeichenfläche selbst: `fang_zeiger` (Zeigeranzeige
    //! *und* Klickpunkt laufen beide darüber) mit laufendem Griffzug sowie ein
    //! vollständiger Mauszug durch `show` mit synthetischen Zeigerereignissen.
    //! Der Fehler war, dass die Zeichenfläche beim Griffzug die *ganze* Auswahl
    //! aus dem Fang nahm — die ausgewählte Polylinie konnte sich weder an fremde
    //! Linien noch an ihre eigenen Ecken anschließen.

    use super::*;
    use jjcad_core::persist::DragMode;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        // Ziehen als Gedrückthalten: unabhängig von den Einstellungen des Rechners.
        app.ui.drag_mode = DragMode::HoldAndDrag;
        app.session_mut()
            .execute(
                "doc.settings",
                &serde_json::json!({"input_unit": "mm",
                                   "snap": {"ortho": false, "polar": false, "grid": false}}),
            )
            .unwrap();
        app
    }

    /// Polylinie (0;0) → (2000;0) → (2000;1500) und freie Linie
    /// (0;3000) → (2000;3000); die Polylinie ist ausgewählt.
    fn szenario() -> (App, Id) {
        let mut app = app();
        let s = app.session_mut();
        s.execute(
            "draw.polyline",
            &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [2000.0, 1500.0]]}),
        )
        .unwrap();
        s.execute(
            "draw.line",
            &serde_json::json!({"a": [0.0, 3000.0], "b": [2000.0, 3000.0]}),
        )
        .unwrap();
        let poly = app.doc().entities[0].id;
        app.session_mut().select(vec![poly]);
        (app, poly)
    }

    fn punkte(app: &App, id: Id) -> Vec<V2> {
        match &app.doc().entity(id).expect("Element fehlt").kind {
            EntityKind::Polyline { verts, .. } => verts.iter().map(|w| w.p).collect(),
            other => panic!("keine Polylinie: {other:?}"),
        }
    }

    fn assert_punkt(ist: V2, soll: V2) {
        assert!(ist.dist(soll) < 1e-9, "erwartet {soll:?}, ist {ist:?}");
    }

    /// Ein Frame mit festem Fenster und den gegebenen Zeigerereignissen.
    fn frame(app: &mut App, events: Vec<egui::Event>) {
        let ctx = app.ctx.clone();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    fn druck(pos: Pos2) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn loslassen(pos: Pos2) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    // ---- Verdrahtung: fang_zeiger während eines Griffzugs ----

    /// Der Griff der ausgewählten Polylinie fängt auf das fremde Linienende —
    /// und zwar exakt, ohne Rundung.
    #[test]
    fn griffzug_faengt_fremdes_linienende() {
        let (mut app, poly) = szenario();
        app.begin_grip_drag(poly, crate::handles::GripKind::Vert(0));

        let (p, label) = fang_zeiger(&app, V2::new(1990.0, 3008.0), 60.0);
        assert_eq!(label, Some("Endpunkt"), "Snap-Indikator in der Statuszeile");
        assert_punkt(p, V2::new(2000.0, 3000.0));
    }

    /// Die ausgewählte Polylinie fängt auf ihre *eigene* Nachbarecke — die
    /// Grundlage für den geschlossenen Zug.
    #[test]
    fn griffzug_faengt_eigene_ecke() {
        let (mut app, poly) = szenario();
        app.begin_grip_drag(poly, crate::handles::GripKind::Vert(0));

        let (p, label) = fang_zeiger(&app, V2::new(1994.0, 8.0), 40.0);
        assert_eq!(label, Some("Endpunkt"), "{p:?}");
        assert_punkt(p, V2::new(2000.0, 0.0));
    }

    /// Kein Selbst-Fang: der gezogene Stützpunkt liegt im Fangradius des
    /// Zeigers, darf ihn aber nicht festziehen.
    #[test]
    fn gezogener_punkt_faengt_sich_nicht() {
        let (mut app, poly) = szenario();
        app.begin_grip_drag(poly, crate::handles::GripKind::Vert(0));

        let roh = V2::new(5.0, 5.0);
        let (p, label) = fang_zeiger(&app, roh, 40.0);
        assert_eq!(label, None, "kein Fang in der Nähe außer dem eigenen Punkt");
        assert_punkt(p, roh);
    }

    // ---- Voller Mauszug durch die Oberfläche ----

    /// Klick auf einen Griff, ziehen auf ein fremdes Linienende, loslassen:
    /// der Stützpunkt landet exakt auf dem Fangpunkt, der Zug endet, und der
    /// Punkt steht so auch in der gespeicherten Datei.
    #[test]
    fn mauszug_uebernimmt_den_fangpunkt_und_speichert_ihn() {
        let (mut app, poly) = szenario();
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();

        let griff = world_to_screen(rect, view, V2::new(0.0, 0.0));
        let ziel = world_to_screen(rect, view, V2::new(1990.0, 3008.0));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss begonnen haben");

        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        assert_eq!(app.snap_label.as_deref(), Some("Endpunkt"), "sichtbarer Fang");
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let pts = punkte(&app, poly);
        assert_punkt(pts[0], V2::new(2000.0, 3000.0));
        // Die übrigen Stützpunkte bleiben unangetastet.
        assert_punkt(pts[1], V2::new(2000.0, 0.0));
        assert_punkt(pts[2], V2::new(2000.0, 1500.0));

        // Gespeichert: die Datei trägt die gefangene Koordinate.
        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        let geladen = wieder
            .entity(poly)
            .and_then(|e| match &e.kind {
                EntityKind::Polyline { verts, .. } => Some(verts[0].p),
                _ => None,
            })
            .expect("Polylinie in der Datei");
        assert_punkt(geladen, V2::new(2000.0, 3000.0));
    }

    // ---- Füllflächen: Griffe an den Ecken (T0018) ----

    /// Füllfläche 4000 × 3000 mm, ausgewählt; liefert (App, Füllflächen-Id).
    fn szenario_flaeche() -> (App, Id) {
        let mut app = app();
        app.session_mut()
            .execute(
                "draw.fill.points",
                &serde_json::json!({
                    "pts": [[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0], [0.0, 3000.0]],
                    "pattern": "solid",
                }),
            )
            .expect("Füllfläche");
        let id = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Fill { .. }))
            .map(|e| e.id)
            .expect("Füllfläche fehlt");
        app.session_mut().select(vec![id]);
        (app, id)
    }

    fn rand(app: &App, id: Id) -> Vec<V2> {
        match &app.doc().entity(id).expect("Element fehlt").kind {
            EntityKind::Fill { boundary, .. } => boundary.clone(),
            other => panic!("keine Füllfläche: {other:?}"),
        }
    }

    /// Jede Ecke der ausgewählten Füllfläche ist als Griff erreichbar — ohne
    /// das ließe sich die Fläche nicht formen.
    #[test]
    fn fuellflaeche_hat_griffe_zum_anfassen() {
        let (app, id) = szenario_flaeche();
        for p in rand(&app, id) {
            let g = app.grip_at(p, 5.0);
            assert!(
                matches!(g, Some((gid, crate::handles::GripKind::Vert(_))) if gid == id),
                "Ecke {p:?}: {g:?}"
            );
        }
    }

    /// Mauszug auf einen Eckgriff: nur diese Ecke wandert, die Fläche bleibt
    /// eine Füllfläche, und die gespeicherte Datei trägt die neue Koordinate.
    #[test]
    fn mauszug_formt_die_fuellflaeche_und_speichert_sie() {
        let (mut app, id) = szenario_flaeche();
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();

        let griff = world_to_screen(rect, view, V2::new(4000.0, 0.0));
        let ziel = world_to_screen(rect, view, V2::new(5000.0, 500.0));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");

        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let pts = rand(&app, id);
        assert_punkt(pts[1], V2::new(5000.0, 500.0));
        assert_punkt(pts[0], V2::new(0.0, 0.0));
        assert_punkt(pts[3], V2::new(0.0, 3000.0));

        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        let geladen = wieder
            .entity(id)
            .and_then(|e| match &e.kind {
                EntityKind::Fill { boundary, .. } => Some(boundary[1]),
                _ => None,
            })
            .expect("Füllfläche in der Datei");
        assert_punkt(geladen, V2::new(5000.0, 500.0));
    }

    /// Bogen auswählen und seinen Endpunkt mit der Maus ziehen: der Winkel
    /// folgt dem Zeiger, der Radius bleibt stehen, und die gespeicherte Datei
    /// trägt den neuen Endwinkel (T0022).
    #[test]
    fn mauszug_dreht_das_bogenende() {
        let mut app = app();
        app.session_mut()
            .execute(
                "draw.arc",
                &serde_json::json!({ "c": [1000.0, 1000.0], "r": 500.0, "a0": 0.0, "a1": 90.0 }),
            )
            .expect("Bogen");
        let id = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Arc { .. }))
            .map(|e| e.id)
            .expect("Bogen fehlt");
        app.session_mut().select(vec![id]);
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        // Endpunkt liegt bei 90° auf (1000;1500); gezogen wird nach 180° (500;1000).
        let griff = world_to_screen(rect, view, V2::new(1000.0, 1500.0));
        let ziel = world_to_screen(rect, view, V2::new(500.0, 1000.0));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");

        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let kind = app.doc().entity(id).expect("Bogen fehlt").kind.clone();
        match kind {
            EntityKind::Arc { c, r, a0, a1 } => {
                assert_punkt(c, V2::new(1000.0, 1000.0));
                assert_eq!(r, 500.0, "Radius darf sich nicht ändern");
                assert_eq!(a0, 0.0, "Startwinkel darf sich nicht ändern");
                assert!(
                    (a1.to_degrees() - 180.0).abs() < 1e-6,
                    "Endwinkel ist {:.3}°",
                    a1.to_degrees()
                );
            }
            other => panic!("kein Bogen: {other:?}"),
        }

        // Gespeichert wird der gezogene Stand.
        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        match &wieder.entity(id).expect("Bogen in der Datei").kind {
            EntityKind::Arc { a1, r, .. } => {
                assert!((a1.to_degrees() - 180.0).abs() < 1e-6, "Datei: a1");
                assert_eq!(*r, 500.0, "Datei: r");
            }
            other => panic!("kein Bogen in der Datei: {other:?}"),
        }
    }

    /// Fängt der Fang auf der Nachbarecke, während die gezogene Ecke schon dort
    /// liegt, darf er nicht wieder abfallen — sonst springt die Vorschau
    /// zwischen Fangpunkt und Zeiger hin und her (T0018, Turn 3).
    #[test]
    fn griffzug_auf_die_nachbarecke_flackert_nicht() {
        let (mut app, id) = szenario_flaeche();
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(1));
        let zoom = app.current_view().zoom.max(1e-6);
        let tol = 12.0 / zoom;
        // ~11 px neben der Nachbarecke (0;0), innerhalb des Fangradius.
        let roh = V2::new(10.0 / zoom, 5.0 / zoom);

        let mut lagen = Vec::new();
        for _ in 0..4 {
            let (p, label) = fang_zeiger(&app, roh, tol);
            app.update_grip_drag(p);
            lagen.push((rand(&app, id)[1], label));
        }
        for (ecke, label) in &lagen {
            assert_eq!(
                *label,
                Some("Endpunkt"),
                "der Fang fällt im nächsten Frame wieder aus: {lagen:?}"
            );
            assert!(
                ecke.dist(V2::new(0.0, 0.0)) < 1e-9,
                "die Ecke springt zwischen Fangpunkt und Zeiger: {lagen:?}"
            );
        }

        // Dasselbe beim **Bewegen**: solange der Zeiger im Fangradius um die
        // Ecke bleibt, sitzt der Punkt dort — er springt nicht zwischen den
        // Bildern hin und her (so sieht der Nutzer das Flackern).
        for px in [11.0, 9.0, 7.0, 5.0, 8.0, 10.0] {
            let roh = V2::new(px / zoom, 2.0 / zoom);
            let (p, label) = fang_zeiger(&app, roh, tol);
            app.update_grip_drag(p);
            assert_eq!(label, Some("Endpunkt"), "bei {px} px: Fang fällt aus");
            assert!(
                rand(&app, id)[1].dist(V2::new(0.0, 0.0)) < 1e-9,
                "bei {px} px: der Punkt springt vom Ziel weg"
            );
        }

        // Weit weg vom Punkt löst sich der Fang — keine Haftung.
        let weit = V2::new(60.0 / zoom, 40.0 / zoom);
        let (p, label) = fang_zeiger(&app, weit, tol);
        assert_eq!(label, None, "kein Fang weit weg von der Ecke");
        assert!(p.dist(weit) < 1e-9);
    }

    /// Zwei Ecken aufeinandergelegt (Zug mit Fang auf die Nachbarecke): aus
    /// vier Umgrenzungspunkten werden drei, die Fläche bleibt eine Fläche —
    /// und die gespeicherte Datei trägt drei Punkte (T0018, Turn 2).
    #[test]
    fn mauszug_verschmilzt_zwei_ecken_zu_einem_punkt() {
        let (mut app, id) = szenario_flaeche();
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();

        let griff = world_to_screen(rect, view, V2::new(4000.0, 0.0));
        let ziel = world_to_screen(rect, view, V2::new(0.0, 0.0));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");

        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let pts = rand(&app, id);
        assert_eq!(pts.len(), 3, "ein Umgrenzungspunkt weniger: {pts:?}");
        assert_eq!(app.status_msg, "Füllfläche · Punkt verbunden");

        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        let geladen = wieder
            .entity(id)
            .and_then(|e| match &e.kind {
                EntityKind::Fill { boundary, .. } => Some(boundary.len()),
                _ => None,
            })
            .expect("Füllfläche in der Datei");
        assert_eq!(geladen, 3, "Punktzahl steht so in der Datei");
    }
}

/// Papierraum der Druckvorschau (Ticket T0004): Zeichnen und Ändern laufen im
/// Blatt über dieselben Werkzeuge wie im Modell.
#[cfg(test)]
mod tests_papierraum {
    use super::*;

    fn app_im_papierraum() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        let sid = app.doc().sheets[0].id;
        app.tab_mut().tab = TabKind::Sheet(sid);
        app.sync_space();
        app
    }

    /// Gerade gezeichnete Linie gehört dem Blatt, nicht der Zeichnung.
    #[test]
    fn linie_im_blatt_landet_im_papierraum() {
        let mut app = app_im_papierraum();
        assert!(app.im_papierraum(), "Reiter Blatt muss Papierraum sein");
        let vorher = app.doc().space_sheet().unwrap().entities.len();

        app.start_tool("draw.line");
        tool_click(&mut app, V2::new(20.0, 20.0));
        tool_click(&mut app, V2::new(120.0, 20.0));

        assert!(
            app.doc().entities.is_empty(),
            "Modell darf durch Blattzeichnen leer bleiben"
        );
        let sheet = app.doc().space_sheet().unwrap();
        assert_eq!(sheet.entities.len(), vorher + 1, "eine Linie im Blatt");
        let neu = sheet
            .entities
            .iter()
            .rev()
            .find(|e| matches!(e.kind, EntityKind::Line { .. }))
            .unwrap();
        match &neu.kind {
            EntityKind::Line { a, b } => {
                assert!((a.x - 20.0).abs() < 1e-6, "{a:?}");
                assert!((b.x - 120.0).abs() < 1e-6, "{b:?}");
            }
            other => panic!("keine Linie: {other:?}"),
        }
    }

    /// Texthöhe im Blatt in Blatt-Millimetern, nicht die Modell-Vorgabe.
    #[test]
    fn texthoehe_im_blatt_ist_in_millimetern() {
        let app = app_im_papierraum();
        assert!((app.options.text_height - 2.5).abs() < 1e-9, "{}", app.options.text_height);
    }

    /// Der Wechsel zurück ins Modell räumt Werkzeug und Auswahl ab.
    #[test]
    fn wechsel_ins_modell_leert_auswahl() {
        let mut app = app_im_papierraum();
        let feld = app.doc().sheets[0]
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Text { .. }))
            .map(|e| e.id)
            .unwrap();
        app.session_mut().selection = vec![feld];
        app.start_tool("draw.line");

        app.tab_mut().tab = TabKind::Model;
        app.sync_space();

        assert!(!app.im_papierraum());
        assert!(app.selection().is_empty(), "Auswahl gehört zum Raum");
        assert!(app.tool.is_none(), "Werkzeug wird beim Raumwechsel beendet");
        assert!(
            (app.options.text_height - app.doc().settings.text_height).abs() < 1e-9,
            "Modell-Texthöhe muss zurückkommen"
        );
    }
}

/// Anzeige beim Reiterwechsel Modell ↔ Papierraum (Ticket T0007): Nach dem
/// Zoom in der Druckvorschau war die Zeichnung unsichtbar und im Ursprung stand
/// ein winziger Blattrahmen — bis die Ansicht bewegt wurde.
#[cfg(test)]
mod tests_raumwechsel_anzeige {
    use super::*;

    /// Eine Modelllinie 3000 mm breit; das Blatt ist 420 mm breit. Die
    /// Ausdehnung der Anzeigeliste sagt damit eindeutig, welcher Raum
    /// gezeichnet wurde. (Aktionskoordinaten sind Meter: 3.0 = 3000 mm.)
    fn app_mit_linie_und_blatt() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [3.0, 0.0]}),
            )
            .expect("Linie im Modell");
        let sid = app.doc().sheets[0].id;
        (app, sid)
    }

    /// Zoom im Papierraum ändern — genau das verwirft den Anzeige-Cache, und
    /// nur dann trat der Fehler auf.
    fn blattzoom_aendern(app: &mut App) {
        let mut v = app.current_view();
        v.zoom *= 3.0;
        app.set_view(v);
    }

    fn ist_modelllinie(dl: &DisplayList, was: &str) {
        assert!(dl.bbox.is_valid(), "{was}: Anzeigeliste ist leer");
        assert!(
            (dl.bbox.width() - 3000.0).abs() < 1.0,
            "{was}: Modelllinie (3000 mm) erwartet, gezeichnet wurde {} mm breit",
            dl.bbox.width()
        );
    }

    /// `switch_tab` hält Reiter und Bearbeitungsraum zusammen — nicht erst der
    /// nächste Frame. Sonst zeichnet die Fläche im Wechselframe den alten Raum
    /// und legt dessen Liste unter dem neuen Schlüssel ab (T0007).
    #[test]
    fn reiterwechsel_zieht_den_bearbeitungsraum_mit() {
        let (mut app, sid) = app_mit_linie_und_blatt();
        app.switch_tab(TabKind::Sheet(sid));
        assert!(app.im_papierraum(), "Blattreiter muss den Papierraum setzen");
        assert_eq!(app.session().space(), Some(sid));

        app.switch_tab(TabKind::Model);
        assert!(!app.im_papierraum(), "Modellreiter muss den Papierraum verlassen");
        assert_eq!(app.session().space(), None);
    }

    /// Der gemeldete Ablauf: im Blatt zoomen, zurück ins Modell — die Zeichnung
    /// muss sofort wieder da sein, ohne die Ansicht zu bewegen.
    #[test]
    fn nach_blattzoom_zeigt_das_modell_wieder_die_zeichnung() {
        let (mut app, sid) = app_mit_linie_und_blatt();
        app.switch_tab(TabKind::Sheet(sid));
        blattzoom_aendern(&mut app);

        app.switch_tab(TabKind::Model);

        assert_eq!(app.session().space(), None, "Modell ist der Bearbeitungsraum");
        ist_modelllinie(&display_list(&mut app), "nach Reiterwechsel");
    }

    /// Wechselframe einzeln nachgestellt: Reiter steht schon auf „Modell“, der
    /// Bearbeitungsraum hängt noch nach. Die dort entstehende Liste darf den
    /// nächsten Frame nicht überleben.
    #[test]
    fn zwischengespeicherte_papierraumliste_wird_nicht_als_modell_gezeichnet() {
        let (mut app, sid) = app_mit_linie_und_blatt();
        app.switch_tab(TabKind::Sheet(sid));
        blattzoom_aendern(&mut app);

        app.tab_mut().tab = TabKind::Model; // Raum absichtlich noch nicht nachgezogen
        let _ = display_list(&mut app);

        app.sync_space();
        ist_modelllinie(&display_list(&mut app), "Frame nach dem Raumwechsel");
    }

    /// Wechselt nur der Bearbeitungsraum (gleiche Revision, gleiche Ansicht),
    /// ist die alte Liste ungültig — im Modell stand die Zeichnung, im
    /// Papierraum steht der Blattrahmen.
    #[test]
    fn raumwechsel_allein_macht_die_anzeigeliste_ungueltig() {
        let (mut app, sid) = app_mit_linie_und_blatt();
        ist_modelllinie(&display_list(&mut app), "Modell");

        app.session_mut().set_space(Some(sid)); // Cache bleibt bestehen

        let blatt = display_list(&mut app);
        assert!(blatt.bbox.is_valid(), "Blatt-Anzeigeliste ist leer");
        // Papierbereich (Blattrahmen samt Rand, deutlich unter 1000 mm) —
        // nicht die 3000-mm-Modelllinie aus dem Zwischenspeicher.
        assert!(
            blatt.bbox.width() < 1000.0 && blatt.bbox.width() > 200.0,
            "Blattrahmen erwartet, gezeichnet wurde {} mm breit",
            blatt.bbox.width()
        );
    }

    /// Der Anzeige-Schlüssel trennt die Räume — gleiche Revision und gleiche
    /// Ansicht ergeben im Modell und im Papierraum verschiedene Listen.
    #[test]
    fn anzeige_schluessel_trennt_modell_und_papierraum() {
        let modell = DrawCacheKey {
            rev: 7,
            zoom: 0.08,
            annot: 100.0,
            space: None,
        };
        let blatt = DrawCacheKey {
            rev: 7,
            zoom: 0.08,
            annot: 100.0,
            space: Some(3),
        };
        assert!(!modell.matches(&blatt), "Papierraum gilt sonst als Modell");
        assert!(!blatt.matches(&modell));
        assert!(modell.matches(&modell));
        assert!(!modell.matches(&DrawCacheKey { rev: 8, ..modell }));
        assert!(!modell.matches(&DrawCacheKey {
            zoom: 0.16,
            ..modell
        }));
        assert!(!modell.matches(&DrawCacheKey {
            annot: 50.0,
            ..modell
        }));
    }
}

#[cfg(test)]
mod tests_spline_werkzeug {
    //! Spline-Werkzeug (T0005): Eingabe wie bei der Polylinie — Punkte klicken,
    //! Klick auf den Startpunkt schließt, ESC/Enter beendet. Geprüft wird über
    //! den Klickpfad und die Objekte im Dokument, nicht über die Vorschau.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    fn taste(app: &mut App, key: egui::Key) {
        let ctx = app.ctx.clone();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    fn splines(app: &App) -> Vec<(Vec<V2>, bool)> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Spline { verts, closed } => {
                    Some((verts.iter().map(|v| v.p).collect(), *closed))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn klicks_erzeugen_einen_spline_mit_drei_stuetzpunkten() {
        let mut app = app();
        app.start_tool("draw.spline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1500.0));
        assert!(app.doc().entities.is_empty(), "noch nicht abgeschlossen");
        taste(&mut app, egui::Key::Escape);
        let s = splines(&app);
        assert_eq!(s.len(), 1, "ESC schließt den Spline ab");
        assert_eq!(s[0].0.len(), 3);
        assert!(!s[0].1, "drei Punkte bleiben offen");
        assert!(app.tool.is_none(), "Werkzeug beendet");
    }

    #[test]
    fn klick_auf_den_startpunkt_schliesst() {
        let mut app = app();
        app.start_tool("draw.spline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1500.0));
        tool_click(&mut app, V2::new(0.0, 0.0));
        let s = splines(&app);
        assert_eq!(s.len(), 1, "Klick auf den Startpunkt schließt und legt an");
        assert!(s[0].1, "geschlossen");
        assert_eq!(s[0].0.len(), 3, "kein doppelter Stützpunkt");
        assert!(app.tool.is_none());
    }

    #[test]
    fn ein_einzelner_punkt_legt_nichts_an() {
        let mut app = app();
        app.start_tool("draw.spline");
        tool_click(&mut app, V2::new(0.0, 0.0));
        taste(&mut app, egui::Key::Escape);
        assert!(app.doc().entities.is_empty(), "ein Punkt ist keine Kurve");
    }
}

#[cfg(test)]
mod tests_textvorschau {
    //! Vorschau des Textwerkzeugs und eingefügter Text sind derselbe Text
    //! (Ticket T0010, Turn 2).
    //!
    //! Die Vorschau stand mit ihrer Grundlinie am Zeiger, der eingefügte Text
    //! wurde mittig am Klickpunkt gesetzt — beim Klick sprang der Text eine
    //! halbe Zeile nach unten und der Rahmen saß darunter. Beide gehen jetzt
    //! über `paint_text` mit den Vorgaben aus `text_preview_prim`.

    use super::*;

    #[test]
    fn vorschau_legt_den_text_wie_die_action_an() {
        let vorschau = text_preview_prim("Hallo".to_string(), 250.0);

        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [0.0, 0.0], "text": "Hallo", "height": 250.0}),
            )
            .expect("Text anlegen");
        let e = app.doc().entities.last().expect("Text fehlt");
        let EntityKind::Text {
            height,
            angle,
            align,
            valign,
            bold,
            ..
        } = &e.kind
        else {
            panic!("kein Text angelegt");
        };

        assert_eq!(*align, vorschau.align, "waagerechte Ausrichtung");
        assert_eq!(*valign, vorschau.valign, "Lage in der Höhe");
        assert_eq!(*bold, vorschau.bold, "Schriftschnitt");
        assert!((height - vorschau.height).abs() < 1e-9, "Höhe");
        assert!((angle - vorschau.angle).abs() < 1e-9, "Drehung");
    }

    /// Mittig: der Text steht um den Klickpunkt, nicht mit der Grundlinie
    /// darauf — sonst zeigt die Vorschau eine andere Lage als das Ergebnis.
    #[test]
    fn vorschau_steht_mittig_auf_dem_zeiger() {
        let t = text_preview_prim("Hallo".to_string(), 250.0);
        assert_eq!(t.valign, TextAlignV::Middle);
    }

    /// Die Drehung des Textes folgt der Zeichnung (mathematisch positiv) —
    /// nicht der Bildschirmrichtung. Vorher kippte ein um +90° gedrehter Text
    /// nach unten statt nach oben (T0025: „Rotieren ist falschherum“).
    #[test]
    fn textwinkel_dreht_wie_die_zeichnung() {
        use std::f64::consts::FRAC_PI_2;
        assert_eq!(text_winkel(0.0, 0.0), 0.0);
        assert!(
            (text_winkel(FRAC_PI_2, 0.0) + FRAC_PI_2 as f32).abs() < 1e-6,
            "90° in der Zeichnung sind −90° für egui"
        );
        // Die Ansichtsdrehung zieht in dieselbe Richtung mit.
        assert!(
            (text_winkel(0.0, FRAC_PI_2 as f32) + FRAC_PI_2 as f32).abs() < 1e-6,
            "Ansichtsdrehung um 90°"
        );
        // Textwinkel und Ansichtsdrehung addieren sich.
        assert!((text_winkel(0.3, 0.4) + 0.7).abs() < 1e-6);
    }

    /// Einfüge-Vorschau (Zwischenablage): ein kopierter Text wird gezeichnet —
    /// vorher fiel der Text stillschweigend durch (`_ => {}` in `ghost_shape`),
    /// sichtbar war nur der Anker (T0025: „Prewiev beim Einfügen fehlt“).
    #[test]
    fn einfuege_vorschau_zeichnet_text() {
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("vorschau"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            let prim = text_preview_prim("Hallo".to_string(), 250.0);
            let w2s = |q: V2| Pos2::new(q.x as f32, q.y as f32);
            ghost_shape(
                &p,
                &PShape::Text(prim),
                &w2s,
                1.0,
                Stroke::new(1.5, Palette::ACCENT),
                0.0,
                );
        });
        assert!(
            !out.shapes.is_empty(),
            "der Text fehlt in der Einfüge-Vorschau"
        );
    }
}

#[cfg(test)]
mod tests_hilfslinien {
    //! Erweiterter Punktfang (Ticket T0014): Verweilen über einem Fangpunkt
    //! erzeugt Hilfslinien, auf denen der nächste Punkt fängt.
    //!
    //! Zwei Ebenen: die Verweil-Erkennung selbst (Zeitgeber, ohne Fenster) und
    //! die Verdrahtung in der Zeichenfläche (`fang_zeiger`, Klick).

    use super::*;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute(
                "doc.settings",
                &serde_json::json!({"input_unit": "mm",
                                    "snap": {"grid": false, "ortho": false, "polar": false}}),
            )
            .unwrap();
        app
    }

    fn linie(app: &mut App) -> Id {
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [2000.0, 0.0]}),
            )
            .unwrap();
        app.doc().entities[0].id
    }

    fn halt_am_ende(app: &App, id: Id) -> snap::HaltPunkt {
        let hit = snap::find_snap(app.doc(), V2::new(1999.0, 2.0), 30.0, None, &[], &[])
            .expect("Linienende muss fangen");
        snap::HaltPunkt::neu(app.doc(), &hit)
    }

    fn assert_punkt(ist: V2, soll: V2, was: &str) {
        assert!(ist.dist(soll) < 1e-9, "{was}: erwartet {soll:?}, ist {ist:?}");
    }

    fn frame(app: &mut App, events: Vec<egui::Event>) {
        let ctx = app.ctx.clone();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    // ---- Verweil-Erkennung ------------------------------------------------

    /// Erst nach der Verweildauer gilt der Punkt als gehalten — und nur einmal.
    #[test]
    fn verweilen_haelt_den_punkt_erst_nach_der_dauer() {
        let t0 = std::time::Instant::now();
        let mut v = VerweilFang::default();
        let h = snap::HaltPunkt {
            point: V2::new(1000.0, 500.0),
            kind: snap::SnapKind::Endpoint,
            entity: None,
            richtung: None,
        };
        assert!(v.update(Some(h.clone()), t0).is_none(), "sofort gehalten?");
        assert!(v.laeuft(), "Zeitgeber läuft");
        let halb = t0 + VERWEILDAUER / 2;
        assert!(v.update(Some(h.clone()), halb).is_none(), "halb ist zu früh");
        let spaeter = t0 + VERWEILDAUER + std::time::Duration::from_millis(20);
        assert!(v.update(Some(h.clone()), spaeter).is_some(), "jetzt halten");
        assert!(!v.laeuft(), "danach läuft kein Zeitgeber mehr");
        // Derselbe Punkt löst nicht erneut aus.
        assert!(
            v.update(Some(h), spaeter + std::time::Duration::from_millis(500)).is_none(),
            "derselbe Punkt darf nicht doppelt halten"
        );
    }

    /// Wandert der Fangpunkt, beginnt die Zeit von vorn.
    #[test]
    fn neuer_punkt_beginnt_von_vorn() {
        let t0 = std::time::Instant::now();
        let mut v = VerweilFang::default();
        let a = snap::HaltPunkt {
            point: V2::new(1000.0, 0.0),
            kind: snap::SnapKind::Endpoint,
            entity: None,
            richtung: None,
        };
        let b = snap::HaltPunkt {
            point: V2::new(3000.0, 0.0),
            kind: snap::SnapKind::Endpoint,
            entity: None,
            richtung: None,
        };
        v.update(Some(a), t0);
        let spaet = t0 + VERWEILDAUER + std::time::Duration::from_millis(50);
        assert!(v.update(Some(b.clone()), spaet).is_none(), "neuer Punkt, neue Zeit");
        assert!(v.laeuft());
        assert!(
            v.update(Some(b), spaet + VERWEILDAUER).is_some(),
            "nach der Dauer halten"
        );
    }

    // ---- Verdrahtung in der Zeichenfläche ---------------------------------

    /// Die Senkrechte des gehaltenen Punktes zieht den Fangpunkt auf ihr x.
    #[test]
    fn hilfslinie_zieht_den_fangpunkt_in_der_app() {
        let mut app = app();
        let id = linie(&mut app);
        let h = halt_am_ende(&app, id);
        assert_punkt(h.point, V2::new(2000.0, 0.0), "Haltepunkt");
        app.halt.push(h);
        let (p, label) = fang_zeiger(&app, V2::new(2004.0, 800.0), 30.0);
        assert_punkt(p, V2::new(2000.0, 800.0), "auf der Senkrechten");
        assert_eq!(label, Some("Hilfslinie"));
    }

    /// Wo die Hilfslinie eine bestehende Linie kreuzt, rastet der Fang exakt
    /// auf der Kreuzung ein und die Statuszeile meldet „Schnittpunkt" (Turn 2).
    #[test]
    fn schnittpunkt_von_hilfslinie_und_linie_faengt_in_der_app() {
        let mut app = app();
        let id = linie(&mut app);
        app.session_mut()
            .execute(
                "draw.line",
                // Mitte der Linie liegt bei x = 2250, nicht auf der Kreuzung —
                // sonst gewinnt der (richtige) Mittelpunkt-Fang davor.
                &serde_json::json!({"a": [1000.0, 1500.0], "b": [3500.0, 1500.0]}),
            )
            .unwrap();
        // Wandende (2000; 0) halten: Senkrechte x = 2000, Waagerechte y = 0.
        app.halt.push(halt_am_ende(&app, id));
        let (p, label) = fang_zeiger(&app, V2::new(2004.0, 1504.0), 30.0);
        assert_punkt(p, V2::new(2000.0, 1500.0), "Kreuzung Senkrechte × Linie");
        assert_eq!(label, Some("Schnittpunkt"));
    }

    /// Ohne Haltepunkt bleibt derselbe Zeiger frei — die Gegenprobe zeigt, dass
    /// wirklich die Hilfslinie zieht und nicht etwa das Raster.
    #[test]
    fn ohne_halt_bleibt_der_zeiger_frei() {
        let mut app = app();
        linie(&mut app);
        let (p, label) = fang_zeiger(&app, V2::new(2004.0, 800.0), 30.0);
        assert_punkt(p, V2::new(2004.0, 800.0), "roher Zeiger");
        assert_eq!(label, None);
    }

    /// Verweilen über dem Linienende legt den Haltepunkt an (echte Frames) und
    /// die Statuszeile meldet „Hilfslinie", sobald der Zeiger auf der
    /// Senkrechten steht.
    #[test]
    fn verweilen_ueber_dem_ende_erzeugt_hilfslinien() {
        let mut app = app();
        linie(&mut app);
        app.start_tool("draw.line");
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let ende = world_to_screen(rect, view, V2::new(2000.0, 0.0));

        frame(&mut app, vec![egui::Event::PointerMoved(ende)]);
        assert!(app.halt.is_empty(), "nach einem Frame noch nicht gehalten");

        std::thread::sleep(VERWEILDAUER + std::time::Duration::from_millis(120));
        frame(&mut app, vec![egui::Event::PointerMoved(ende)]);
        assert_eq!(app.halt.len(), 1, "Verweilen hält den Endpunkt: {:?}", app.halt);
        assert_punkt(app.halt[0].point, V2::new(2000.0, 0.0), "Haltepunkt");

        // Zeiger auf die Senkrechte des Haltepunkts: der Fang meldet sie.
        let tol = fang_radius_px(&app, view.zoom);
        let auf_der_linie = world_to_screen(rect, view, V2::new(2000.0 + tol * 0.3, 700.0));
        frame(&mut app, vec![egui::Event::PointerMoved(auf_der_linie)]);
        assert_eq!(app.snap_label.as_deref(), Some("Hilfslinie"));
    }

    /// Ein gesetzter Punkt löst die Hilfslinien — sonst blieben sie für den
    /// nächsten Punkt ohne Anlass stehen.
    #[test]
    fn klick_loest_die_hilfslinien() {
        let mut app = app();
        let id = linie(&mut app);
        app.start_tool("draw.line");
        app.halt.push(halt_am_ende(&app, id));
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let center = app.current_view().center;
        let ziel = world_to_screen(rect, view, center);
        assert!(rect.contains(ziel), "Zielfläche: {rect:?}, Punkt {ziel:?}");

        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(ziel),
                egui::Event::PointerButton {
                    pos: ziel,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: ziel,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(app.halt.is_empty(), "Klick löst die Hilfslinien");
        assert_eq!(app.tool.as_ref().map(|t| t.pts.len()), Some(1), "erster Punkt gesetzt");
    }
}

#[cfg(test)]
mod tests_kopieren_basispunkt {
    //! T0012: „Kopieren mit Basispunkt" legt die Auswahl **nur** in die
    //! Zwischenablage — der Klick bestimmt den Basispunkt, es entsteht dabei
    //! keine Kopie in der Zeichnung. Abgesetzt wird mit der Einfüge-Vorschau.

    use super::*;

    fn leer_app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app
    }

    fn app_mit_rechteck() -> App {
        let mut app = leer_app();
        app.start_tool("draw.rect");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 500.0));
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        app
    }

    #[test]
    fn kopieren_mit_basispunkt_fuellt_nur_die_zwischenablage() {
        let mut app = app_mit_rechteck();

        app.start_tool("modify.copy");
        tool_click(&mut app, V2::new(250.0, 100.0)); // Basispunkt, ein Klick genügt

        assert_eq!(
            app.doc().entities.len(),
            1,
            "Kopieren mit Basispunkt setzt keine Kopie ab — nur die Zwischenablage füllt sich"
        );
        assert!(app.tool.is_none(), "nach dem Basispunkt ist das Werkzeug fertig");
        assert_eq!(app.ui.clipboard.len(), 1, "die Auswahl liegt in der Zwischenablage");
        assert_eq!(
            app.ui.clipboard.base,
            Some(V2::new(250.0, 100.0)),
            "der geklickte Basispunkt gehört zur Zwischenablage"
        );
    }

    /// Ein weiterer Klick ins Leere legt nichts an (kein zweiter Schritt).
    #[test]
    fn nach_dem_basispunkt_entsteht_bei_weiterem_klick_nichts() {
        let mut app = app_mit_rechteck();
        app.start_tool("modify.copy");
        tool_click(&mut app, V2::new(250.0, 100.0));
        tool_click(&mut app, V2::new(2250.0, 1100.0));
        assert_eq!(app.doc().entities.len(), 1, "kein zweiter Klick, keine Kopie");
        assert_eq!(
            app.ui.clipboard.base,
            Some(V2::new(250.0, 100.0)),
            "der Basispunkt bleibt der zuerst geklickte"
        );
    }
}

#[cfg(test)]
mod tests_texteingabe_anzeige {
    //! Während ein Text bearbeitet wird, darf er nicht zusätzlich aus der
    //! Anzeige kommen (T0019, Turn 3). Der Filter lief zunächst nur beim
    //! Aufbau der Displayliste — bei einer geladenen Zeichnung lag die Liste
    //! schon im Zwischenspeicher, und der alte Text blieb im Hintergrund stehen.

    use super::*;

    fn app_mit_text() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");
        app.session_mut()
            .execute(
                "draw.text",
                &json!({"p": [1000.0, 500.0], "text": "WOHNEN", "height": 250.0}),
            )
            .expect("Text anlegen");
        app.mark_changed();
        let id = app.doc().ents()[0].id;
        (app, id)
    }

    fn sichtbar(app: &mut App, id: Id) -> bool {
        display_list(app).prims.iter().any(|p| p.entity == id)
    }

    #[test]
    fn bearbeiteter_text_verschwindet_aus_der_anzeige() {
        let (mut app, id) = app_mit_text();

        // Erster Durchgang füllt den Zwischenspeicher (wie beim Öffnen einer Datei).
        assert!(sichtbar(&mut app, id), "Text steht in der Anzeige");

        assert!(crate::textedit::start_entitaet(&mut app, id), "Bearbeiten startet");
        assert!(
            !sichtbar(&mut app, id),
            "der bearbeitete Text darf nicht zusätzlich gezeichnet werden (Cache-Treffer)"
        );

        // Eingabe weg: der Text ist wieder da.
        app.ui.texteingabe = None;
        assert!(sichtbar(&mut app, id), "nach dem Bearbeiten wieder sichtbar");
    }
}
