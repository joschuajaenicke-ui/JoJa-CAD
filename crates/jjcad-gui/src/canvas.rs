//! Zeichnungsfläche: Anzeige, Fang, Werkzeugbedienung, Navigation.

use crate::app::{
    screen_to_world, visible_world_bounds, world_to_screen, App, Bogenmodus, DrawCacheKey,
    Einfuegemodus, TabKind, Tool, ToolPhase, View,
};
use crate::perf::Abschnitt;
use crate::theme::Palette;
use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};
use jjcad_core::actions;
use jjcad_core::boundary::{detect_boundary, BoundaryError, BoundaryOpts, FillBoundary};
use jjcad_core::geom::*;
use jjcad_core::geom::Vec2 as V2;
use jjcad_core::model::*;
use jjcad_core::render::{self, DisplayList, Prim, Shape as PShape};
use jjcad_core::snap;
use jjcad_core::units::Unit;
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
    /// Höchste Rastergröße der Schrift in Pixeln (T0114, Einstellungen →
    /// Darstellung → „Textauflösung“). Höher = schärfer im hohen Zoom, aber
    /// mehr Schriftatlas.
    text_deckel: f32,
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

/// Nächste Wand unter dem Punkt: liefert (Punkt auf der Achse, Wandwinkel, Wand-Id).
///
/// Der Punkt wird auf die **freie Wandstrecke** gezogen (T0078). An
/// verbundenen Ecken beginnt sie erst an der Anschlussnaht — die
/// Gehrungspunkte der Umrandung schneiden den Wandkörper dort ab, eine Tür
/// stünde sonst mit einer Ecke neben der Wand.
fn wall_under(app: &App, p: V2) -> Option<(V2, f64, jjcad_core::model::Id)> {
    let doc = app.doc();
    // Ein Index für alle Kandidaten — er muss die Nachbarwände am Knoten
    // mitfassen (Naht), daher der weitere Bereich.
    let idx = jjcad_core::wall::WallIndex::build_near(
        doc,
        &doc.entities,
        &jjcad_core::geom::BBox::from_point(p).expanded(3000.0),
    );
    let mut best: Option<(f64, V2, f64, jjcad_core::model::Id)> = None;
    for e in &doc.entities {
        if let jjcad_core::model::EntityKind::Wall { a, b, thickness, .. } = &e.kind {
            let d = jjcad_core::geom::dist_point_segment(p, *a, *b);
            if d > thickness * 0.5 + 600.0 {
                continue;
            }
            let dir = (*b - *a).normalized();
            let (lo, hi) = idx.span(e.id).unwrap_or((0.0, a.dist(*b)));
            let t = (p - *a).dot(dir).clamp(lo, hi);
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
    center_mark_r(p, c, col, 4.0);
}

/// Mittelpunktkreuz mit eigener Halbbreite.
///
/// Beim Fangen wird es größer gezeichnet als die allgemeine Fangmarke, damit
/// der sonst unsichtbare Mittelpunkt erkennbar ist (T0117).
fn center_mark_r(p: &Painter, c: Pos2, col: Color32, r: f32) {
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
/// Kreis als **geschlossenen Zug** mit mittiger Kontur zeichnen.
///
/// `Painter::circle_stroke` legt die Strichbreite bei egui/epaint nach **außen**
/// (`PathStroke::outside` in `tessellate_circle`): die Kontur steht dann von `r`
/// bis `r + w`. Der Kreis wirkt dadurch um die halbe Strichbreite größer als sein
/// Durchmesser, der Punktfang (auf `r`) sitzt am Innenrand der Linie, und Bild
/// und PDF/PNG laufen auseinander — die zeichnen mittig. Die Punkte liegen
/// deshalb auf dem Radius und die Kontur wird wie bei Bogen (`Shape::line`) und
/// Ellipse (`stroke_pts`) zentriert gestrichen (T0080).
pub(crate) fn kreis_zug(center: Pos2, radius: f32, stroke: Stroke) -> egui::Shape {
    if radius <= 0.0 {
        return egui::Shape::Noop;
    }
    // Segmentzahl nach derselben Pfeilhöhen-Regel wie alle Bögen.
    let arc = Arc::new(V2::new(0.0, 0.0), radius as f64, 0.0, std::f64::consts::TAU);
    let n = CurveQuality::SCREEN.segments(&arc, 1.0).max(8);
    let pts: Vec<Pos2> = (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            center + Vec2::new(radius * a.cos() as f32, radius * a.sin() as f32)
        })
        .collect();
    egui::Shape::closed_line(pts, stroke)
}

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

/// Strichbreite eines Objekts auf dem Bildschirm — die **eine** Umrechnung von
/// Papier-Millimetern in Pixel, für die Anzeige (`paint_prim`) und für Vorschauen
/// wie die Stutzen-Hervorhebung (T0075).
///
/// Zwei Modi: `px` = bildschirmkonstant (Breite in Pixeln, unabhängig vom Zoom),
/// `DPI` = wie im PDF (Breite_mm × Pixel je Papier-mm, ohne Obergrenze, damit
/// Strichbreite und Abstand in jedem Zoom im selben Verhältnis stehen).
fn linienbreite_px(width_mm: f64, paper_px: f64, scaled: bool) -> f64 {
    if scaled {
        (width_mm * paper_px).max(0.05)
    } else {
        (width_mm * PAPER_PX_PER_MM).clamp(0.7, 9.0)
    }
}

/// Texturen der Bilder, je egui-Kontext.
///
/// Der Zwischenspeicher liegt in `Context::data` — damit gehört er zur
/// laufenden Anwendung und nicht zu einem prozessweiten Zustand (Tests bauen
/// je Fall einen eigenen Kontext). Der Schlüssel ist die Kennung der
/// Bilddaten; gleiche Bilder teilen sich eine Textur.
type BildTexturen = std::collections::HashMap<u64, egui::TextureHandle>;

fn bild_textur(p: &Painter, bild: &jjcad_core::bild::Bild) -> egui::TextureHandle {
    let ctx = p.ctx().clone();
    let id = egui::Id::new("jjcad_bild_texturen");
    if let Some(h) = ctx.data(|d| {
        d.get_temp::<BildTexturen>(id)
            .and_then(|m| m.get(&bild.kennung).cloned())
    }) {
        return h;
    }
    let farben = egui::ColorImage::from_rgba_unmultiplied(
        [bild.breite as usize, bild.hoehe as usize],
        &bild.rgba,
    );
    let h = ctx.load_texture(
        format!("bild-{:016X}", bild.kennung),
        farben,
        egui::TextureOptions::LINEAR,
    );
    let merken = h.clone();
    ctx.data_mut(|d| {
        let mut map = d.get_temp::<BildTexturen>(id).unwrap_or_default();
        map.insert(bild.kennung, merken);
        d.insert_temp(id, map);
    });
    h
}

/// Einheitliche UV-Deckung einer Textur (0…1 in beiden Richtungen).
fn uv_voll() -> Rect {
    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
}

/// Bild als Textur über vier Ecken — auch gedreht.
///
/// egui zeichnet Texturen nur achsenparallel; ein Bild über einer schrägen
/// unteren Kante braucht deshalb ein Mesh aus zwei Dreiecken. Ecke 0 = unten
/// links, 1 = unten rechts, 2 = oben rechts, 3 = oben links; die Textur hat
/// ihren Ursprung oben links, deshalb liegen die UV-Koordinaten spiegelbildlich.
fn bild_mesh(
    tex: &egui::TextureHandle,
    ecken: &[V2; 4],
    w2s: &dyn Fn(V2) -> Pos2,
    tint: Color32,
) -> Shape {
    let uv = [
        Pos2::new(0.0, 1.0),
        Pos2::new(1.0, 1.0),
        Pos2::new(1.0, 0.0),
        Pos2::new(0.0, 0.0),
    ];
    let mut mesh = egui::Mesh::with_texture(tex.id());
    for i in 0..4 {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: w2s(ecken[i]),
            uv: uv[i],
            color: tint,
        });
    }
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    Shape::mesh(mesh)
}

fn paint_prim(p: &Painter, prim: &Prim, w2s: &dyn Fn(V2) -> Pos2, scale: f64, pc: &PaintCtx, alpha: f32) {
    let is_sel = !pc.preview && pc.selected.contains(&prim.entity) && prim.entity != 0;
    let is_hover = !pc.preview && pc.hover == Some(prim.entity) && prim.entity != 0 && !is_sel;
    let mut col = color32(prim.style.color);
    if pc.preview {
        // Wie die Einfüge-Vorschau (ghost): durchgezogene Akzentkontur
        col = Palette::accent();
    } else if is_sel {
        col = Palette::select();
    } else if is_hover {
        col = Palette::hover();
    }
    let col = col.gamma_multiply(alpha);
    // Zwei Modi:
    //  "px"  = bildschirmkonstant (Breite in Pixeln, unabhängig vom Zoom)
    //  "DPI" = wie im PDF: Breite_mm * Pixel-pro-Papier-mm, ohne Obergrenze,
    //          damit Strichbreite und Abstand in jedem Zoom im selben Verhältnis stehen.
    let base_w = linienbreite_px(prim.style.width, pc.paper_px, pc.lineweights_scaled);
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
        PShape::Image { a, b, hoehe, bild } => {
            let ecken = jjcad_core::bild::ecken(*a, *b, *hoehe);
            let pts: Vec<Pos2> = ecken.iter().map(|q| w2s(*q)).collect();
            if pts.iter().any(|q| !q.x.is_finite() || !q.y.is_finite()) {
                return;
            }
            let tex = bild_textur(p, bild);
            let tint = Color32::from_white_alpha((255.0 * alpha).round().clamp(0.0, 255.0) as u8);
            p.add(bild_mesh(&tex, &ecken, w2s, tint));
            if is_sel || is_hover {
                // Auswahl färbt nur blau ein — bei einem Bild als durchsichtige
                // Fläche darüber (ein Rahmen wäre ein zweites Auswahlzeichen).
                p.add(Shape::convex_polygon(
                    pts,
                    Palette::select().gamma_multiply(0.25 * alpha),
                    Stroke::NONE,
                ));
            }
        }
        PShape::Fill { outer, holes } | PShape::FillOrdered { outer, holes } => {
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
                Palette::accent().gamma_multiply(0.12)
            } else if is_sel {
                Palette::select().gamma_multiply(0.25)
            } else if is_hover {
                Palette::hover().gamma_multiply(0.2)
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
            let rr = (*r * scale) as f32;
            if rr < 0.4 {
                // Auf dem Schirm punktförmiger Kreis
                p.circle_filled(w2s(*c), (w as f32 * 0.5).max(0.5), col);
            } else {
                p.add(kreis_zug(w2s(*c), rr, stroke));
            }
            // Mittelpunktkreuz nur bei Auswahl (T0117): im Plan ist der
            // Kreismittelpunkt kein Zeichnungselement. Fangen darf er
            // trotzdem — dann zeigt ihn die Fangmarke (`draw_cursor`).
            if is_sel {
                center_mark(p, w2s(*c), col);
            }
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
            if is_sel {
                center_mark(p, w2s(*c), col.gamma_multiply(0.8));
            }
        }
        PShape::Ellipse { c, rx, ry, rot } => {
            let pts: Vec<Pos2> = (0..=72)
                .map(|i| {
                    let t = i as f64 / 72.0 * std::f64::consts::TAU;
                    w2s(*c + V2::new(rx * t.cos(), ry * t.sin()).rotated(*rot))
                })
                .collect();
            stroke_pts(p, &pts, w as f32, col);
            if is_sel {
                center_mark(p, w2s(*c), col);
            }
        }
        // Ellipsenbogen: dieselbe Pfeilhöhen-Regel wie Bogen und Kreis
        // (Gerätemaß 0,15 px), damit die Kurve in jedem Zoom glatt bleibt.
        PShape::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = jjcad_core::geom::Ellipse::new(*c, *rx, *ry, *rot);
            let pts: Vec<Pos2> = el
                .arc_points(*a0, *a1, 0.15 / scale.max(1e-9))
                .iter()
                .map(|q| w2s(*q))
                .collect();
            stroke_pts(p, &pts, w as f32, col);
            if is_sel {
                center_mark(p, w2s(*c), col);
            }
        }
        PShape::Dot { p: pos, r } => {
            p.circle_filled(w2s(*pos), ((*r * scale) as f32).max(1.2), col);
        }
        PShape::Text(t) => paint_text(p, t, w2s(t.pos), scale, col, pc.rot, pc.text_deckel),
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
    deckel: f32,
) {
    let galley = text_galley(p, t, scale, col, deckel);
    let (pos, winkel) = text_ort(t, &galley, base, rot);
    if winkel.abs() < 0.001 {
        p.galley(pos, galley, col);
    } else {
        let shape = egui::epaint::TextShape::new(pos, galley, col).with_angle(winkel);
        p.add(shape);
    }
}

/// Ort der Schrift und Drehwinkel für `paint_text` — die **eine** Stelle, die
/// den Anker auf den Text anwendet.
///
/// Waagerecht wird nichts verschoben: die Zeilen hat der Satz schon um den
/// Ursprung ausgerichtet (`halign`, siehe [`text_galley`]). Senkrecht hängt der
/// Kasten am Anker (`text_senkrecht`), bei gedrehtem Text wandert dieser
/// Versatz mit der Drehung mit.
pub(crate) fn text_ort(
    t: &render::TextPrim,
    galley: &egui::Galley,
    base: Pos2,
    rot: f32,
) -> (Pos2, f32) {
    let vy = text_senkrecht(t.valign, galley.size().y);
    let winkel = text_winkel(t.angle, rot);
    if winkel.abs() < 0.001 {
        (Pos2::new(base.x, base.y + vy), winkel)
    } else {
        let (sin, cos) = winkel.sin_cos();
        (base + Vec2::new(-vy * sin, vy * cos), winkel)
    }
}

/// Waagerechte Ausrichtung für den Satz — `halign` legt die Zeilen **um den
/// Ursprung** der Galley (links ab 0, zentriert um 0, rechts endend bei 0).
pub(crate) fn halign(a: TextAlignH) -> egui::Align {
    match a {
        TextAlignH::Left => egui::Align::LEFT,
        TextAlignH::Center => egui::Align::Center,
        TextAlignH::Right => egui::Align::RIGHT,
    }
}

/// Satz eines Textes: eine Galley, deren Anker im Ursprung liegt.
///
/// Mehrzeilig trennt `\n` die Zeilen; die waagerechte Ausrichtung gilt je
/// Zeile über `halign`. Weil die Zeilen dort schon um den Ursprung ausgerichtet
/// sind, verschiebt `paint_text` waagerecht **nichts** mehr — der frühere
/// zweite Versatz (`-Breite/2`, `-Breite`) zog einen mittigen Text um die halbe
/// Breite zu weit nach links (T0028).
pub(crate) fn text_galley(
    p: &Painter,
    t: &render::TextPrim,
    scale: f64,
    col: Color32,
    deckel: f32,
) -> std::sync::Arc<egui::Galley> {
    let gewuenscht = (t.height * scale * render::TEXT_GROESSE_FAKTOR).max(3.0) as f32;
    // Rastergröße gedeckelt und auf die Größenleiter gelegt, den Rest streckt
    // die Galley (T0114) — sonst rastet egui jede Zoomstufe in den gemeinsamen
    // Schriftatlas und der Text friert im hohen Zoom ein. Der Deckel ist
    // einstellbar (`Textaufloesung`, T0114 Turn 3).
    let (raster, faktor) = crate::textgroesse::raster_und_skalierung(gewuenscht, deckel);
    let galley = satz(p.ctx(), t, raster, col);
    crate::textgroesse::skaliere(galley, faktor)
}

/// Satz eines Textprims in einer Schriftgröße (px) — ohne Maler.
///
/// Dieselbe Galley bekommen Zeichenfläche, Eigenschaften-Panel, Editor und
/// die Prüfung des Rahmens: Schriftgröße aus dem Kern, Zeilenhöhe der
/// Familie mal Zeilenabstand, Ausrichtung über `halign` (T0110).
pub(crate) fn satz(
    ctx: &egui::Context,
    t: &render::TextPrim,
    size: f32,
    col: Color32,
) -> std::sync::Arc<egui::Galley> {
    let runs = t.effective_runs();
    let mut job = auszeichnungs_job(ctx, &runs, size, col, t.font);
    job.halign = halign(t.align);
    zeilenhoehe_setzen(&mut job, size, t.line_height_em() as f32, t.line_spacing);
    ctx.fonts(|f| f.layout_job(job))
}

/// Zeilenhöhe des Satzes festlegen — eine Zeile ist `size · lh_em · abstand` hoch,
/// in **jedem** Lauf (auch der Hochstellung).
///
/// Ohne diese Vorgabe setzt egui jede Zeile mit der Zeilenhöhe ihrer Schrift;
/// bei gemischter Auszeichnung (Hochstellung in kleinerer Schrift) wäre die
/// Zeile dann niedriger als die übrigen. Der Wert ist derselbe, den
/// `render::TextPrim::line_height` für den Rahmen rechnet.
pub(crate) fn zeilenhoehe_setzen(
    job: &mut egui::text::LayoutJob,
    size: f32,
    line_height_em: f32,
    line_spacing: f64,
) {
    let lh = (size * line_height_em * line_spacing.clamp(0.1, 10.0) as f32).max(1.0);
    for abschnitt in &mut job.sections {
        abschnitt.format.line_height = Some(lh);
    }
}

/// Satzauftrag mit den Auszeichnungen der Läufe — die eine Stelle, an der
/// Fett/Kursiv/Unterstrichen/Hochgestellt zu egui-Formaten werden.
///
/// Wird von der Zeichenfläche, dem Textfeld im Eigenschaften-Panel und dem
/// Editor in der Zeichnung benutzt: überall sieht der Text damit gleich aus.
pub(crate) fn auszeichnungs_job(
    ctx: &egui::Context,
    runs: &[jjcad_core::model::TextRun],
    size: f32,
    col: Color32,
    font: jjcad_core::model::TextFont,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for r in runs {
        // Hochstellung: kleinere Schrift am Zeilenkopf statt auf der Grundlinie.
        let r_size = if r.superscript { size * 0.7 } else { size };
        let family = schrift_familie(ctx, font, r.bold);
        let fmt = egui::TextFormat {
            font_id: FontId::new(r_size.max(2.0), family),
            color: col,
            italics: r.italic,
            underline: if r.underline {
                Stroke::new(1.0, col)
            } else {
                Stroke::NONE
            },
            valign: if r.superscript {
                egui::Align::Min
            } else {
                egui::Align::Center
            },
            ..Default::default()
        };
        job.append(&r.text, 0.0, fmt);
    }
    job
}

/// egui-Familie zu Schriftart und Schnitt.
///
/// Die Familien werden in `theme` aus denselben Systemschriften angelegt, aus
/// denen auch Raster und PDF zeichnen — sonst zeigte die Zeichenfläche etwas
/// anderes als der Ausdruck.
pub(crate) fn schrift_familie(
    ctx: &egui::Context,
    font: jjcad_core::model::TextFont,
    bold: bool,
) -> FontFamily {
    let name = if bold {
        format!("schrift_{}_fett", font.key())
    } else {
        format!("schrift_{}", font.key())
    };
    let familie = FontFamily::Name(name.into());
    // Vor `theme::install` (und in Tests mit nacktem Kontext) ist die Familie
    // nicht angemeldet — dann die Standardschrift, statt in egui abzustürzen.
    let bekannt = ctx.fonts(|f| f.families().contains(&familie));
    if bekannt {
        familie
    } else if bold {
        FontFamily::Name("bold".into())
    } else {
        FontFamily::Proportional
    }
}

/// Senkrechter Versatz des Textkastens gegenüber dem Anker (Bildschirm-px,
/// y nach unten): Oben 0, Mitte −Höhe/2, Unten −Höhe.
///
/// Dieselbe Lage, die `render::text_corners` für Rahmen und Fang rechnet: bei
/// „Oben" hängt der Text unter dem Einfügepunkt, bei „Unten" steht er darüber.
pub(crate) fn text_senkrecht(v: TextAlignV, hoehe: f32) -> f32 {
    match v {
        TextAlignV::Top => 0.0,
        TextAlignV::Middle => -hoehe * 0.5,
        TextAlignV::Bottom => -hoehe,
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

/// Beispielwort der Textvorschau. Ein neues Textfeld beginnt **immer leer**
/// (der Inhalt wird erst nach dem Klick getippt, `textedit::start`) — die
/// Vorschau zeigt deshalb immer dieses Wort und nie den zuletzt gesetzten
/// Text (T0066).
pub(crate) const TEXT_VORSCHAU: &str = "Text";

/// Text, den das Textwerkzeug beim Klick anlegen wird — genau die Vorgaben der
/// Action `draw.text` (Ausrichtung aus dem Eigenschaften-Panel, ohne Drehung).
/// Die Vorschau muss diesen Text zeichnen, damit sie dieselbe Lage zeigt wie
/// das Ergebnis: der Klickpunkt ist der Einfügepunkt der Ausrichtung, bei
/// „Zentriert/Mitte" also die Mitte des Textes.
pub(crate) fn text_preview_prim(
    text: String,
    height: f64,
    align: TextAlignH,
    valign: TextAlignV,
) -> render::TextPrim {
    text_preview_prim_mit(
        text,
        height,
        align,
        valign,
        jjcad_core::text::RunStyle::default(),
        jjcad_core::model::TextFont::default(),
        1.0,
        0.0,
    )
}

/// Dasselbe mit Auszeichnung und Drehung — die Vorschau zeigt, was der Klick
/// erzeugt, auch wenn der Text im Panel gedreht eingestellt ist.
#[allow(clippy::too_many_arguments)]
pub(crate) fn text_preview_prim_mit(
    text: String,
    height: f64,
    align: TextAlignH,
    valign: TextAlignV,
    stil: jjcad_core::text::RunStyle,
    font: jjcad_core::model::TextFont,
    line_spacing: f64,
    angle: f64,
) -> render::TextPrim {
    let runs = if stil.plain() {
        Vec::new()
    } else {
        vec![jjcad_core::model::TextRun {
            text: text.clone(),
            bold: stil.bold,
            italic: stil.italic,
            underline: stil.underline,
            superscript: stil.superscript,
        }]
    };
    render::TextPrim {
        runs,
        pos: V2::ZERO,
        text,
        height,
        angle,
        align,
        valign,
        bold: false,
        italic: false,
        underline: false,
        superscript: false,
        font,
        line_spacing,
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
    // Leistungsanzeige (T0068): mit der Zeichenfläche beginnt der Abschnitt
    // „Fangen“ — Navigation, Zeigerlage und Fangpunkte werden hier ermittelt.
    app.perf.abschnitt(Abschnitt::Fangen);
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
    let is_sheet = matches!(&app.tab().tab, TabKind::Sheet(_));
    let is_block = matches!(&app.tab().tab, TabKind::Block(_));

    // Hintergrund
    painter.rect_filled(rect, 0.0, Palette::bg());

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
    // Gilt auch beim **Manipulieren** (Griffzug, Maus-Verschiebung): dort wird
    // ebenso ein Punkt gesetzt, und ohne Haltepunkte ließ sich nichts mehr
    // ausrichten (T0046, Turn 2).
    if fang_aktiv(app) {
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
    let mut snap_kind = None;
    app.snap_label = None;
    if let Some(w) = raw_world {
        let (p, label, kind) =
            fang_zeiger_art(app, w, fang_radius_px(app, app.current_view().zoom));
        snapped = Some(p);
        app.snap_label = label.map(str::to_string);
        snap_kind = kind;
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
        // Leistungsanzeige: der Papierraum baut seine Ansichtsfenster selbst auf
        // und zeichnet sie sofort — Aufbau und Zeichnen sind hier ein Abschnitt.
        app.perf.abschnitt(Abschnitt::Zeichnen);
        // Druckvorschau: Blatt, Ansichtsfenster und Papierraum-Objekte.
        draw_sheet(app, &painter, rect, view_now, snapped);
        app.perf.abschnitt(Abschnitt::Bedienung);
        // Danach dieselbe Bedien-Darstellung wie im Zeichenmodus — Werkzeug,
        // Griffe, Einfüge-Vorschau, Auswahlfenster und Fadenkreuz-Zeiger.
        if app.tool.is_some() {
            draw_tool_preview(app, &painter, rect, view_now, snapped, raw_world);
        }
        // Maßangabe beim Ziehen an einer Treppenkante (T0027, Turn 4)
        draw_grip_mass(app, ui, &painter, rect, view_now);
        draw_hilfslinien(app, &painter, rect, view_now);
        draw_text_rahmen(app, &painter, rect, view_now, &selected);
        draw_wand_basislinien(app, &painter, rect, view_now, &selected);
        draw_tuer_basispunkte(app, &painter, rect, view_now, &selected);
        draw_grips(app, &painter, rect, view_now, &selected);
        if app.ghost.is_some() {
            draw_ghost(app, &painter, rect, view_now);
        }
        if let Some((start, cur, _)) = app.select_drag {
            draw_auswahlfenster(&painter, rect, view_now, start, cur);
        }
        draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world, snap_kind);
    } else {
        app.perf.abschnitt(Abschnitt::Displayliste);
        let dl = display_list(app);
        app.perf.abschnitt(Abschnitt::Zeichnen);
        let pc = PaintCtx {
            selected: selected.clone(),
            hover,
            lineweights,
            lineweights_scaled,
            paper_px,
            preview: false,
            rot: view_now.rot as f32,
            text_deckel: app.prefs.textaufloesung.px(),
        };
        if app.ui.show_grid {
            draw_grid(app, &painter, rect, view_now);
        }
        // Papierfläche — der Blockeditor hat einen eigenen Ton (T0137).
        painter.rect_filled(
            rect,
            0.0,
            if is_block {
                Palette::block_paper()
            } else {
                Palette::paper()
            },
        );
        if app.ui.show_grid {
            draw_grid(app, &painter, rect, view_now);
        }
        let w2s = |p: V2| world_to_screen(rect, view_now, p);
        if app.perf.an {
            // Offene Leistungsanzeige: je Grafikelement die Zeit messen, damit
            // sich „Zeichnen“ nach Füllung, Linie und Text aufteilen lässt. Die
            // Uhr je Element kostet selbst Zeit — deshalb nur bei offener
            // Anzeige.
            for prim in &dl.prims {
                let t = std::time::Instant::now();
                paint_prim(&painter, prim, &w2s, view_now.zoom, &pc, 1.0);
                app.perf.grafikelement(prim, t.elapsed());
            }
        } else {
            for prim in &dl.prims {
                paint_prim(&painter, prim, &w2s, view_now.zoom, &pc, 1.0);
            }
        }
        // Blockeditor: Basispunkt und Hinweis, wo man zeichnet (T0137).
        if is_block {
            draw_block_hinweise(app, &painter, rect, view_now);
        }
        // Fangmarker + Cursor
        app.perf.abschnitt(Abschnitt::Bedienung);
        if app.tool.is_some() {
            draw_tool_preview(app, &painter, rect, view_now, snapped, raw_world);
        }
        // Maßangabe beim Ziehen an einer Treppenkante (T0027, Turn 4)
        draw_grip_mass(app, ui, &painter, rect, view_now);
        // Hilfslinien des erweiterten Punktfangs unter allem, was der Zeiger
        // selbst zeichnet (Vorschau, Griffe, Fadenkreuz).
        draw_hilfslinien(app, &painter, rect, view_now);
        draw_text_rahmen(app, &painter, rect, view_now, &selected);
        draw_wand_basislinien(app, &painter, rect, view_now, &selected);
        draw_tuer_basispunkte(app, &painter, rect, view_now, &selected);
        draw_grips(app, &painter, rect, view_now, &selected);
        if app.ghost.is_some() {
            draw_ghost(app, &painter, rect, view_now);
        }
        if let Some((start, cur, _)) = app.select_drag {
            draw_auswahlfenster(&painter, rect, view_now, start, cur);
        }
        draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world, snap_kind);
    }

    // Werkzeug-Icon am Zeiger — auch außerhalb der Zeichenfläche (T0057),
    // deshalb auf eigener Ebene statt im beschnittenen Maler der Fläche.
    draw_werkzeug_zeiger(app, ui.ctx());

    // Eingabe
    let pointer_pos = ui.ctx().pointer_hover_pos();
    if resp.hovered() && !chat_offen {
        if let Some(pos) = pointer_pos {
            // Hover-Highlight (auch im Papierraum — dort auf Schriftfeld/Texten).
            // Die Schnellauswahl arbeitet **mit** einem Werkzeug: das Objekt,
            // das der nächste Klick trifft, muss schon vorher hervortreten.
            if app.tool.is_none() || app.schnellauswahl_aktiv() {
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
        handle_input(app, ui, &resp, rect, &painter, snapped, raw_world);
        // Aufforderung des laufenden Werkzeugs (bzw. der Einfüge-Vorschau) in
        // den Chat. Nur bei geändertem Text, sonst stünde sie 60-mal pro
        // Sekunde im Protokoll.
        if app.ghost.is_some() {
            app.mitteilung_wenn_neu(jjcad_core::i18n::tr("gui.canvas.einfuegen_klick_setzt_ab"));
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
        if let Some(sid) = match &app.tab().tab {
            TabKind::Sheet(id) => Some(*id),
            _ => None,
        } {
            if let Some(sheet) = app.doc().sheet(sid) {
                painter.text(
                    Pos2::new(rect.left() + 14.0, rect.top() + 12.0),
                    Align2::LEFT_TOP,
                    format!("{} · {} · {}", sheet.name, sheet.paper.label(), format_scale(sheet.scale)),
                    FontId::proportional(11.0),
                    Palette::text_faint(),
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
        block: app.doc().in_block_space(),
    };
    let (mut dl, neu) = match &app.tab().cache {
        Some((k, dl)) if k.matches(&key) => (dl.clone(), false),
        _ => {
            let doc = app.doc().clone();
            let ctx = render::RenderCtx::new(&doc);
            let dl = render::render_document(&ctx);
            app.tab_mut().cache = Some((key, dl.clone()));
            (dl, true)
        }
    };
    // Leistungsanzeige (T0068): Herkunft der Liste und Größe der Zeichnung.
    app.perf.anzeigeliste(neu, app.doc().ents().len());
    ohne_texteingabe(app, &mut dl);
    dl
}

// ---------------------------------------------------------------------------
// Blockeditor
// ---------------------------------------------------------------------------

/// Hinweise im Blockeditor (T0137): der Koordinatenursprung ist der
/// Einfüge-Basispunkt des Blocks und wird als Kreuz mit Kreis gezeigt; oben in
/// der Zeichenfläche steht, dass hier ein Block und nicht die Zeichnung
/// bearbeitet wird.
///
/// Beides ist reine Anzeige — es wird nicht gefangen und nicht gedruckt.
fn draw_block_hinweise(app: &App, p: &Painter, rect: Rect, view: View) {
    let Some(block) = app.doc().block_edit() else {
        return;
    };
    let akzent = Palette::accent();
    // Basispunkt (Weltursprung des Blocks)
    let o = world_to_screen(rect, view, V2::ZERO);
    p.circle_stroke(o, 6.0, Stroke::new(1.6, akzent));
    p.line_segment(
        [Pos2::new(o.x - 11.0, o.y), Pos2::new(o.x + 11.0, o.y)],
        Stroke::new(1.6, akzent),
    );
    p.line_segment(
        [Pos2::new(o.x, o.y - 11.0), Pos2::new(o.x, o.y + 11.0)],
        Stroke::new(1.6, akzent),
    );
    p.text(
        Pos2::new(o.x + 14.0, o.y + 4.0),
        Align2::LEFT_TOP,
        jjcad_core::i18n::tr("gui.canvas.basispunkt"),
        FontId::proportional(10.0),
        akzent,
    );

    // Hinweis oben in der Mitte: eigener Raum, nicht die Zeichnung.
    let text = format!(
        "{} · {}",
        jjcad_core::i18n::tr("gui.canvas.blockeditor"),
        block.name
    );
    let galley = p.layout_no_wrap(text, FontId::proportional(12.5), Palette::text());
    let size = galley.size() + Vec2::new(24.0, 12.0);
    let box_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 22.0),
        size,
    );
    p.rect_filled(box_rect, egui::Rounding::same(8.0), Palette::hinweis_flaeche());
    p.rect_stroke(
        box_rect,
        egui::Rounding::same(8.0),
        Stroke::new(1.0_f32, akzent.gamma_multiply(0.7)),
    );
    p.galley(
        Pos2::new(
            box_rect.center().x - galley.size().x / 2.0,
            box_rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        Palette::text(),
    );
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
    draw_set(minor, Palette::grid_minor(), 1.0, p);
    draw_set(major, Palette::grid_major(), 1.0, p);
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
    let Some(sid) = (match &app.tab().tab {
        TabKind::Sheet(id) => Some(*id),
        _ => None,
    }) else {
        return;
    };
    let Some(sheet) = app.doc().sheet(sid).cloned() else {
        return;
    };
    // Arbeitsfläche um das Blatt — folgt dem Design (T0102).
    p.rect_filled(rect, 0.0, Palette::bg());
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
    p.add(Shape::convex_polygon(ecken.to_vec(), Palette::paper(), Stroke::NONE));
    let w2s = |q: V2| world_to_screen(rect, view, q);

    // 1. Inhalt der Ansichtsfenster (liegt unter dem Papierraum)
    for (_, vp) in sheet.viewports() {
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
            text_deckel: app.prefs.textaufloesung.px(),
        };
        for prim in &vdl.prims {
            paint_prim(&clip, prim, &vp_w2s, view.zoom * s, &vpc, 1.0);
        }
    }

    // 2. Beschriftung der Ansichtsfenster. Rahmen, Auswahlfarbe und Griffe
    //    kommen aus der normalen Objektbehandlung (`render_sheet` +
    //    `draw_grips`) — ein Ansichtsfenster ist seit T0109 ein Blattobjekt.
    for (e, vp) in sheet.viewports() {
        let vp_rect = Rect::from_min_max(
            w2s(V2::new(vp.x, vp.y + vp.h)),
            w2s(V2::new(vp.x + vp.w, vp.y)),
        );
        let sel = app.selection().contains(&e.id);
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
        text_deckel: app.prefs.textaufloesung.px(),
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
                p.rect_stroke(r, 0.0, Stroke::new(1.4, Palette::select()));
                p.text(
                    Pos2::new(r.left() + 4.0, r.top() + 3.0),
                    Align2::LEFT_TOP,
                    jjcad_core::i18n::t_args_owned("gui.canvas.x_x_mm", &[("wert1", format!("{:.0}", r.width())), ("wert2", format!("{:.0}", r.height()))]),
                    FontId::proportional(10.5),
                    Palette::select(),
                );
            }
        }
    }

    // Bedienhinweis
    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 34.0),
        Align2::LEFT_TOP,
        jjcad_core::i18n::tr("gui.canvas.papierraum_zeichnen_und_aendern_wie_im_modell_ansicht_an"),
        FontId::proportional(10.5),
        Palette::text_faint(),
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

/// Beschriftung am Zeiger (dunkle Box, heller Text).
///
/// Eine Stelle für die Werkzeug-Vorschau — die Maßangabe beim Ziehen
/// (`draw_grip_mass`) sieht genauso aus.
fn paint_label(p: &Painter, rect: Rect, pos: Pos2, text: &str) {
    let galley = p.layout_no_wrap(text.to_string(), FontId::proportional(11.0), Palette::text());
    // Box im Zeichenbereich halten: am unteren/äußeren Rand würde sie sonst
    // abgeschnitten (Fehlermeldungen stehen genau dort, wo geklickt wird).
    let groesse = galley.size() + Vec2::new(12.0, 6.0);
    let min = Pos2::new(
        (pos.x - 6.0)
            .min(rect.right() - groesse.x - 4.0)
            .max(rect.left() + 4.0),
        (pos.y - 3.0)
            .min(rect.bottom() - groesse.y - 4.0)
            .max(rect.top() + 4.0),
    );
    let bg = Rect::from_min_size(min, groesse);
    p.rect_filled(bg, 4.0, Palette::hinweis_flaeche());
    p.rect_stroke(bg, 4.0, Stroke::new(1.0, Palette::border()));
    p.galley(bg.min + Vec2::new(6.0, 3.0), galley, Palette::text());
}

/// Maßangabe einer Treppe für die Beschriftung am Zeiger:
/// „3,00 × 1,00 · 13 Stg" (Länge × Breite · Stufenzahl).
pub(crate) fn stair_mass_text(unit: Unit, laenge: f64, breite: f64, stufen: u32) -> String {
    format!(
        "{} × {} · {} Stg",
        unit.format_plain(laenge, unit.decimals()),
        unit.format_plain(breite, unit.decimals()),
        stufen
    )
}

/// Maßangabe während des Griffzugs: beim Ziehen an einer Treppenkante laufen
/// Länge, Breite und Stufenzahl mit — dieselbe Beschriftung wie beim Erstellen
/// (T0027, Turn 4).
fn draw_grip_mass(app: &App, ui: &Ui, p: &Painter, rect: Rect, view: View) {
    let Some(drag) = app.grip_drag.as_ref() else {
        return;
    };
    let Some(e) = app.doc().entity(drag.entity) else {
        return;
    };
    let EntityKind::Stair {
        a,
        b,
        width,
        height,
        steps,
        auto_steps,
        ..
    } = &e.kind
    else {
        return;
    };
    // Ohne Zeiger (Screenshot-Zustand) steht die Angabe am gezogenen Griff.
    let griff = crate::handles::grips_of(e)
        .into_iter()
        .find(|(art, _)| *art == drag.kind)
        .map(|(_, g)| world_to_screen(rect, view, g));
    let Some(pos) = ui
        .ctx()
        .pointer_hover_pos()
        .filter(|q| rect.contains(*q))
        .or(griff)
    else {
        return;
    };
    let mass = jjcad_core::stair::treppenmass(a.dist(*b), *height, *steps, *auto_steps);
    let text = stair_mass_text(app.unit(), a.dist(*b), *width, mass.stufen);
    paint_label(p, rect, pos + Vec2::new(14.0, -16.0), &text);
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
            blatt.contains("draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world, snap_kind)"),
            "Blatt-Zweig zeichnet den Zeiger nicht über draw_cursor"
        );
        let j = src.find("let dl = display_list(app);").unwrap();
        let rest = &src[j..];
        let modell = &rest[..rest.find("// Eingabe").expect("Zeichenmodus nicht gefunden")];
        assert!(
            modell.contains("draw_cursor(app, ui, &painter, rect, view_now, snapped, raw_world, snap_kind)"),
            "Zeichenmodus zeichnet den Zeiger nicht über draw_cursor"
        );
        assert!(
            src.contains("draw_fadenkreuz(p, rect, Some(pos));"),
            "draw_cursor zeichnet das Fadenkreuz nicht mehr"
        );
    }

}

fn draw_cursor(
    _app: &mut App,
    ui: &mut Ui,
    p: &Painter,
    rect: Rect,
    view: View,
    snapped: Option<V2>,
    raw: Option<V2>,
    snap_kind: Option<snap::SnapKind>,
) {
    let Some(pos) = ui.ctx().pointer_hover_pos().filter(|q| rect.contains(*q)) else {
        return;
    };
    if let Some(w) = snapped {
        let s = world_to_screen(rect, view, w);
        // Der Mittelpunkt von Kreis, Bogen und Ellipse ist im Plan unsichtbar
        // (T0117) — wird er gefangen, zeigt ihn das Kreuz hier an. Sonst wäre
        // der Fangpunkt nur an der allgemeinen Fangmarke zu erahnen.
        if snap_kind == Some(snap::SnapKind::Center) {
            center_mark_r(p, s, Palette::accent(), 7.0);
        }
        let r = 5.0;
        p.line_segment([s - Vec2::new(r, 0.0), s + Vec2::new(r, 0.0)], Stroke::new(1.2, Palette::accent()));
        p.line_segment([s - Vec2::new(0.0, r), s + Vec2::new(0.0, r)], Stroke::new(1.2, Palette::accent()));
        if raw.map(|q| q.dist(w)).unwrap_or(0.0) * view.zoom > 1.0 {
            p.circle_stroke(s, 7.0, Stroke::new(1.0, Palette::accent().gamma_multiply(0.7)));
        }
    }
    // Fadenkreuz bis zu den Bildschirmrändern
    draw_fadenkreuz(p, rect, Some(pos));
}

/// Lage des Werkzeug-Icons am Mauszeiger (Bildschirmpunkte).
///
/// Eine Quelle für Zeichnen und Test: das Icon sitzt unten rechts neben dem
/// Zeiger, egal in welchem Teil des Fensters er steht.
pub fn werkzeug_zeiger_mitte(zeiger: Pos2) -> Pos2 {
    zeiger + Vec2::new(22.0, 22.0)
}

/// Zeichnet das Icon des laufenden Werkzeugs am Mauszeiger — im **ganzen**
/// Fenster, nicht nur über der Zeichenfläche (T0057).
///
/// Der Maler der Zeichenfläche ist auf ihr Rechteck beschnitten; über Ribbon,
/// Eigenschaften-Panel oder Statusleiste wäre das Icon dort abgeschnitten.
/// Deshalb eine eigene Ebene in `Order::Middle`: sie liegt über Zeichenfläche,
/// Ribbon und Panels (alle `Order::Background`), aber unter Dialogen, Menüs,
/// Protokoll und Tooltips (`Foreground`/`Tooltip`) — ein offenes Fenster
/// verdeckt das Icon, statt dass es darüber schwebt. Bei offenem
/// Chat-Protokoll ruht das Zeichnen, dort bleibt auch das Icon weg.
pub fn draw_werkzeug_zeiger(app: &App, ctx: &egui::Context) {
    if app.chat.offen {
        return;
    }
    let Some(t) = &app.tool else { return };
    let Some(def) = actions::action(&t.action) else {
        return;
    };
    let Some(pos) = ctx.pointer_hover_pos() else {
        return;
    };
    // Nur im Fenster: außerhalb gibt es keinen Zeiger, an dem es hängen könnte.
    if !ctx.screen_rect().contains(pos) {
        return;
    }
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("werkzeug_zeiger"),
    ));
    let mitte = werkzeug_zeiger_mitte(pos);
    let ir = Rect::from_center_size(mitte, Vec2::splat(18.0));
    p.circle_filled(mitte, 12.0, Color32::from_rgba_unmultiplied(20, 22, 26, 190));
    p.circle_stroke(mitte, 12.0, Stroke::new(1.0, Palette::accent().gamma_multiply(0.6)));
    crate::icons::paint(&p, ir.shrink(2.0), def.icon, Palette::accent());
}

/// Zeichnet die Vorschau einer Öffnung (Tür oder Fenster).
///
/// Drei Schritte, weil die Vorschau **über** der vorhandenen Zeichnung liegt:
/// erst das Loch in Papierfarbe freilegen (die Wand darunter läuft sonst
/// durch), dann die Wandkanten in Normalfarbe (Füllflächen auslassen, sie
/// deckten Bemaßung und Schraffur zu), zuletzt das Symbol als Akzentkontur.
fn draw_opening_preview(
    app: &App,
    p: &Painter,
    view: View,
    v: &render::OpeningPreview,
    w2s: &dyn Fn(V2) -> Pos2,
) {
    for quad in &v.loch {
        let pts: Vec<Pos2> = quad.iter().map(|q| w2s(*q)).collect();
        p.add(egui::Shape::convex_polygon(pts, Palette::paper(), Stroke::NONE));
    }
    let paper_px = view.zoom * app.doc().settings.annotation_scale;
    let wand_ctx = PaintCtx {
        selected: app.selection().to_vec(),
        hover: app.tab().hover,
        lineweights: app.ui.lineweights,
        lineweights_scaled: app.ui.lineweights_scaled,
        paper_px,
        preview: false,
        rot: view.rot as f32,
        text_deckel: app.prefs.textaufloesung.px(),
    };
    let symbol_ctx = PaintCtx {
        preview: true,
        ..wand_ctx.clone()
    };
    for prim in &v.wall.prims {
        if matches!(prim.shape, PShape::Fill { .. }) {
            continue;
        }
        paint_prim(p, prim, w2s, view.zoom, &wand_ctx, 1.0);
    }
    for prim in &v.symbol.prims {
        paint_prim(p, prim, w2s, view.zoom, &symbol_ctx, 1.0);
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
    let col = Palette::accent().gamma_multiply(0.9);
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
                        Palette::select().gamma_multiply(0.18),
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
                    label = jjcad_core::i18n::tr("gui.canvas.schliessen").to_string();
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
                label = jjcad_core::i18n::tr("gui.canvas.schliessen").to_string();
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
                p.circle_stroke(w2s(*q), 2.0, Stroke::new(1.0, Palette::accent()));
            }
            if label.is_empty() {
                label = jjcad_core::i18n::t_args_owned("gui.canvas.x_stuetzpunkte", &[("wert1", format!("{}", tool.pts.len() + 1))]);
            }
        }
        // Raumstempel: Umriss bis zum Zeiger, keine Füllung — die Fläche des
        // Stempels bleibt unsichtbar. Am Zeiger steht die Fläche, die der
        // Stempel später beschriftet.
        "dim.room" => {
            for pair in tool.pts.windows(2) {
                p.line_segment([w2s(pair[0]), w2s(pair[1])], stroke);
            }
            if let Some(start) = last {
                p.line_segment([w2s(start), w2s(cursor)], stroke);
                if let Some(first) = tool.pts.first() {
                    p.line_segment([w2s(cursor), w2s(*first)], dashed);
                }
            }
            let mut umriss = tool.pts.clone();
            umriss.push(cursor);
            if let Some(pts) = fill_boundary_points(&umriss) {
                label = jjcad_core::i18n::t_args_owned("gui.canvas.x_punkte_x", &[("wert1", format!("{}", pts.len())), ("wert2", format!("{}", unit.format_area(polygon_area(&pts).abs(), 2)))]);
            } else {
                label = jjcad_core::i18n::t_args_owned("gui.canvas.x_punkte", &[("wert1", format!("{}", tool.pts.len() + 1))]);
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
            if let Some(pts) = fill_boundary_points_rechteck(&umriss) {
                label = jjcad_core::i18n::t_args_owned("gui.canvas.x_punkte_x", &[("wert1", format!("{}", pts.len())), ("wert2", format!("{}", unit.format_area(polygon_area(&pts).abs(), 2)))]);
            } else {
                label = jjcad_core::i18n::t_args_owned("gui.canvas.x_punkte", &[("wert1", format!("{}", tool.pts.len() + 1))]);
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
                    label = jjcad_core::i18n::t_args_owned("gui.canvas.flaeche_x", &[("wert1", format!("{}", unit.format_area(g.area(), 2)))]);
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
                    label = jjcad_core::i18n::tr("gui.canvas.flaeche_ermitteln").into();
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
        // Bild: das gewählte Bild halbtransparent im aufgezogenen Rechteck,
        // damit der Nutzer vor dem zweiten Klick sieht, was dort landet.
        // Bild: die untere Kante folgt dem Zeiger, die Höhe kommt aus dem
        // Seitenverhältnis der Datei. Gezeigt wird das fertige Bild.
        "draw.image" => {
            if let Some(start) = last {
                let breite = start.dist(cursor);
                if let Some(q) = &app.options.bild {
                    if let Some(bild) = jjcad_core::bild::dekodiere(&q.data) {
                        let hoehe = jjcad_core::bild::hoehe_aus_breite(&bild, breite);
                        let ecken = jjcad_core::bild::ecken(start, cursor, hoehe);
                        let tex = bild_textur(p, &bild);
                        p.add(bild_mesh(&tex, &ecken, &w2s, Color32::from_white_alpha(170)));
                        let pts: Vec<Pos2> = ecken.iter().map(|q| w2s(*q)).collect();
                        p.add(Shape::closed_line(pts, stroke));
                        label = format!(
                            "{} × {}",
                            unit.format_plain(breite, unit.decimals()),
                            unit.format_plain(hoehe, unit.decimals())
                        );
                    }
                }
                // Die untere Kante selbst hervorheben: sie ist die Eingabe.
                p.line_segment([w2s(start), w2s(cursor)], stroke);
            }
        }
        "draw.circle" => {
            if let Some(c) = last {
                let r = c.dist(cursor);
                p.add(kreis_zug(w2s(c), (r * view.zoom) as f32, stroke));
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
            if app.options.arc_mode == Bogenmodus::Mittelpunkt {
                // Mittelpunkt, Startpunkt (Radius), Endpunkt: der Startpunkt
                // legt den Radius fest, der Bogen läuft von dort im
                // mathematisch positiven Drehsinn zum Zeiger.
                match tool.pts.len() {
                    0 => {}
                    1 => {
                        let c = tool.pts[0];
                        let r = c.dist(cursor);
                        p.line_segment([w2s(c), w2s(cursor)], dashed);
                        if r > 0.5 {
                            p.add(kreis_zug(w2s(c), (r * view.zoom) as f32, stroke));
                            label = jjcad_core::i18n::t_args_owned("gui.canvas.r_x", &[("wert1", format!("{}", unit.format_plain(r, unit.decimals())))]);
                        }
                    }
                    _ => {
                        let c = tool.pts[0];
                        let s0 = tool.pts[1];
                        let r = c.dist(s0);
                        p.line_segment([w2s(c), w2s(cursor)], dashed);
                        if r > 0.5 {
                            let a0 = (s0 - c).angle();
                            let a1 = (cursor - c).angle();
                            let arc = Arc::new(c, r, a0, a1);
                            let pts: Vec<Pos2> = CurveQuality::SCREEN
                                .flatten(&arc, view.zoom)
                                .iter()
                                .map(|q| w2s(*q))
                                .collect();
                            p.add(Shape::line(pts, stroke));
                            label = jjcad_core::i18n::t_args_owned("gui.canvas.r_x", &[("wert1", format!("{}", unit.format_plain(r, unit.decimals())))]);
                        }
                    }
                }
            } else if tool.pts.len() == 1 {
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
            // Drei Klicks: Lauflinie (Anfang, Ende), dann die Breite. Die
            // Vorschau zeigt dieselbe Gestalt wie das fertige Objekt — Umriss,
            // Stufenlinien und Lauflinie mit Richtungspfeil.
            match tool.pts.len() {
                0 => {}
                1 => {
                    let a0 = tool.pts[0];
                    p.line_segment([w2s(a0), w2s(cursor)], dashed);
                    label = format!(
                        "{} {}",
                        unit.format_plain(a0.dist(cursor), unit.decimals()),
                        unit.short()
                    );
                }
                _ => {
                    // Die ersten beiden Klicks liegen auf der **Außenkante**,
                    // der dritte bestimmt die Tiefe dorthin, wo der Zeiger
                    // steht. Die Anzeige zeigt die Treppe so, wie sie entsteht.
                    let a0 = tool.pts[0];
                    let b0 = tool.pts[1];
                    let laenge = a0.dist(b0);
                    let tiefe = basis_hoehe(a0, b0, cursor);
                    let breite = tiefe.abs();
                    if laenge > 1.0 && breite > 1.0 {
                        let dir = (b0 - a0).normalized();
                        let nrm = dir.perp();
                        let hw = breite * 0.5;
                        // Lauflinie = Mitte zwischen Außenkante und Zeiger
                        let ma = a0 + nrm * (tiefe * 0.5);
                        let mb = b0 + nrm * (tiefe * 0.5);
                        let quad = vec![
                            w2s(ma + nrm * hw),
                            w2s(mb + nrm * hw),
                            w2s(mb - nrm * hw),
                            w2s(ma - nrm * hw),
                        ];
                        p.add(Shape::closed_line(quad, stroke));
                        let stufen = if app.options.stair_auto {
                            jjcad_core::stair::stufenzahl(laenge, app.options.stair_height)
                        } else {
                            app.options.stair_steps.max(1)
                        };
                        for i in 1..stufen {
                            let q = ma.lerp(mb, i as f64 / stufen as f64);
                            p.line_segment([w2s(q + nrm * hw), w2s(q - nrm * hw)], dashed);
                        }
                        p.line_segment([w2s(ma), w2s(mb)], stroke);
                        let ah = (laenge * 0.05).max(40.0);
                        for seite in [1.0, -1.0] {
                            p.line_segment(
                                [
                                    w2s(mb),
                                    w2s(mb - dir * (ah * 1.5) + nrm * (ah * 0.6 * seite)),
                                ],
                                stroke,
                            );
                        }
                        label = stair_mass_text(unit, laenge, breite, stufen);
                    } else {
                        label = jjcad_core::i18n::tr("gui.canvas.treppe_breite_klicken").to_string();
                    }
                }
            }
        }
        "draw.door" => {
            let width = app.options.door_width;
            // Vorschau ist die Tür selbst: dasselbe Symbol wie nach dem Klick
            // (Blatt, Öffnungsbogen, Anschlagseite) auf der vorderen
            // Wandkante, dazu die Wand mit ihrem Ausschnitt. Ein eigener
            // Platzhalter hatte mit dem Ergebnis nichts zu tun (T0026).
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
                        base: app.options.door_base,
                    };
                    if let Some(v) = render::preview_door(app.doc(), &spec) {
                        draw_opening_preview(app, p, view, &v, &w2s);
                        // Basispunkt sichtbar machen (T0091): hier hängt die Tür am Zeiger.
                        if let Some(bp) = v.base {
                            draw_basispunkt_marke(p, w2s(bp));
                        }
                        label = jjcad_core::i18n::t_args_owned("gui.canvas.tuer_x", &[("wert1", format!("{}", unit.format_plain(width, 2)))]);
                    } else {
                        // Die Tür passt nicht in die freie Wandstrecke — der
                        // Klick ergäbe einen Fehler, also keine Vorschau (T0078).
                        p.circle_stroke(w2s(cursor), 9.0, dashed);
                        label = jjcad_core::i18n::tr("gui.canvas.tuer_wand_zu_kurz").into();
                    }
                }
                None => {
                    // Eine Tür braucht eine Wand — ohne sie nur der Hinweis
                    p.circle_stroke(w2s(cursor), 9.0, dashed);
                    label = jjcad_core::i18n::tr("gui.canvas.tuer_nur_auf_einer_wand").into();
                }
            }
        }
        "draw.window" => {
            let width = app.options.window_width;
            // Dasselbe wie bei der Tür: das fertige Fenster mit demselben
            // Ausschnitt in der Wand (T0026, Turn 2).
            match wall_under(app, cursor) {
                Some((cp, ang, host)) => {
                    let spec = render::WindowSpec {
                        host: Some(host),
                        center: cp,
                        angle: ang,
                        width,
                        align: app.options.window_align,
                    };
                    if let Some(v) = render::preview_window(app.doc(), &spec) {
                        draw_opening_preview(app, p, view, &v, &w2s);
                        label = jjcad_core::i18n::t_args_owned("gui.canvas.fenster_x", &[("wert1", format!("{}", unit.format_plain(width, 2)))]);
                    } else {
                        p.circle_stroke(w2s(cursor), 9.0, dashed);
                        label = jjcad_core::i18n::tr("gui.canvas.fenster_wand_zu_kurz").into();
                    }
                }
                None => {
                    p.circle_stroke(w2s(cursor), 9.0, dashed);
                    label = jjcad_core::i18n::tr("gui.canvas.fenster_nur_auf_einer_wand").into();
                }
            }
        }
        "draw.point" => {
            p.circle_stroke(w2s(cursor), 6.0, stroke);
        }
        "draw.text" => {
            let h = app.options.text_height;
            // Immer das Beispielwort: ein neues Textfeld beginnt leer (der
            // Inhalt wird erst nach dem Klick getippt, `textedit::start`) —
            // der zuletzt gesetzte Text gehört nicht zur Vorschau (T0066).
            let txt = TEXT_VORSCHAU.to_string();
            // Dieselbe Platzierung wie der Text, der beim Klick entsteht
            // (Ausrichtung aus dem Panel) — nicht mit der Grundlinie am Zeiger.
            let stil = jjcad_core::text::RunStyle {
                bold: app.options.text_bold,
                italic: app.options.text_italic,
                underline: app.options.text_underline,
                superscript: app.options.text_superscript,
            };
            // Vorschau in der gewählten Farbe — sonst zeigte sie etwas anderes
            // als der Klick erzeugt.
            let vorschau_col = app
                .options
                .text_color
                .map(color32)
                .unwrap_or(col);
            // Dieselbe Form wie das fertige Objekt: Hintergrundfläche (immer
            // das Rechteck) und Umrandung, eckig oder rund (T0126). Die
            // Vorschau liegt über der Zeichnung, deshalb mit leichter
            // Deckkraft — sie darf den Plan darunter nicht verdecken.
            let tprim = text_preview_prim_mit(
                txt,
                h,
                app.options.text_align,
                app.options.text_valign,
                stil,
                app.options.text_font,
                app.options.text_line_spacing,
                app.options.text_angle.to_radians(),
            );
            if let Some(bg) = app.options.text_background {
                let pts: Vec<Pos2> = render::text_frame_corners(&tprim)
                    .iter()
                    .map(|q| w2s(*q + cursor))
                    .collect();
                p.add(Shape::convex_polygon(
                    pts,
                    color32(bg).gamma_multiply(0.6),
                    Stroke::NONE,
                ));
            }
            paint_text(
                p,
                &tprim,
                w2s(cursor),
                view.zoom,
                vorschau_col,
                view.rot as f32,
                app.prefs.textaufloesung.px(),
            );
            if app.options.text_frame {
                // Derselbe Radius wie im fertigen Objekt: halbe Zeilenhöhe
                // (einzeilig ein Oval, mehrzeilig Viertelkreise; T0126, Turn 7).
                let radius = if app.options.text_frame_round {
                    tprim.line_height() * 0.5
                } else {
                    0.0
                };
                let pts: Vec<Pos2> = render::text_frame_path(&tprim, radius)
                    .iter()
                    .map(|q| w2s(*q + cursor))
                    .collect();
                p.add(Shape::closed_line(
                    pts,
                    Stroke::new(1.0, vorschau_col.gamma_multiply(0.7)),
                ));
            }
        }
        "draw.fill" | "draw.hatch" | "draw.volume" => {
            p.circle_stroke(w2s(cursor), 9.0, dashed);
            label = jjcad_core::i18n::tr("gui.canvas.kontur_anklicken").into();
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
        // Grundriss-Kote: Kreis mit Fadenkreuz, Größe wie das eingefügte
        // Symbol (Textgröße der Bemaßung).
        "dim.elevation.plan" => {
            let r_welt = app.doc().dim_style.text_height * 0.55;
            let r = (r_welt * view.zoom) as f32;
            p.circle_stroke(w2s(cursor), r, dashed);
            let arm = r * 1.4;
            let c = w2s(cursor);
            p.line_segment(
                [Pos2::new(c.x - arm, c.y), Pos2::new(c.x + arm, c.y)],
                dashed,
            );
            p.line_segment(
                [Pos2::new(c.x, c.y - arm), Pos2::new(c.x, c.y + arm)],
                dashed,
            );
            label = jjcad_core::i18n::t_args_owned("gui.canvas.kote_x", &[("wert1", format!("{}", jjcad_core::units::elevation_text(app.options.elevation_z)))]);
        }
        // Schnitt-Kote: Vorschau ist das **fertige** Objekt — Dreiecke der
        // gesetzten Punkte plus der Punkt am Zeiger, mit Höhenangabe unter
        // jedem Dreieck (T0122). Gezeichnet wird über dieselbe Anzeige wie die
        // eingefügte Kote, damit Form und Werte vor dem Klick feststehen.
        "dim.elevation.section" => {
            let mut pts = tool.pts.clone();
            pts.push(cursor);
            let dl = render::preview_elevation_section(
                app.doc(),
                &pts,
                app.doc().current_layer,
                app.ui.pending.color,
            );
            let pc = PaintCtx {
                selected: Vec::new(),
                hover: None,
                lineweights: app.ui.lineweights,
                lineweights_scaled: app.ui.lineweights_scaled,
                paper_px: view.zoom * app.doc().settings.annotation_scale,
                preview: true,
                rot: view.rot as f32,
                text_deckel: app.prefs.textaufloesung.px(),
            };
            for prim in &dl.prims {
                paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
            }
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
                center_mark(p, w2s(c), Palette::accent());
                match tool.pts.get(1).copied() {
                    None => {
                        // Von-Richtung (Bezugsrichtung) wird gesetzt
                        if c.dist(cursor) > 1e-6 {
                            p.line_segment([w2s(c), w2s(cursor)], dashed);
                        }
                        p.circle_stroke(w2s(cursor), 5.0, stroke);
                        label = jjcad_core::i18n::t_args_owned("gui.canvas.von_x", &[("wert1", format!("{}", format_number((cursor - c).angle().to_degrees(), 1)))]);
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
                                let pre = Stroke::new(1.1, Palette::select().gamma_multiply(0.85));
                                for prim in &dl.prims {
                                    ghost_shape(
                                        p,
                                        &prim.shape,
                                        &w2s,
                                        view.zoom,
                                        pre,
                                        view.rot as f32,
                                        app.prefs.textaufloesung.px(),
                                    );
                                }
                                label = jjcad_core::i18n::t_args_owned("gui.canvas.drehen_x", &[("wert1", format!("{}", format_number(angle, 1)))]);
                            }
                            None => {
                                // Bis-Punkt liegt auf dem Zentrum: keine Richtung
                                label = jjcad_core::i18n::tr("gui.canvas.bis_abstand_zum_drehzentrum_noetig").into();
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
                center_mark(p, w2s(c), Palette::accent());
                match tool.pts.get(1).copied() {
                    None => {
                        if c.dist(cursor) > 1e-6 {
                            p.line_segment([w2s(c), w2s(cursor)], dashed);
                        }
                        p.circle_stroke(w2s(cursor), 5.0, stroke);
                        label = jjcad_core::i18n::t_args_owned("gui.canvas.bezug_x", &[("wert1", format!("{}", format_number((cursor - c).angle().to_degrees(), 1)))]);
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
                                label = jjcad_core::i18n::t_args_owned("gui.canvas.ansicht_x", &[("wert1", format!("{}", format_number(angle, 1)))]);
                            }
                            None => label = jjcad_core::i18n::tr("gui.canvas.abstand_zum_drehzentrum_noetig").into(),
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
                    Stroke::new(1.0, Palette::select()),
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
                label = jjcad_core::i18n::t_args_owned("gui.canvas.faktor_x", &[("wert1", format!("{:.2}", f))]);
            }
        }
        "modify.offset" => {
            if let Some(a) = last {
                p.line_segment([w2s(a), w2s(cursor)], dashed);
                label = jjcad_core::i18n::t_args_owned("gui.canvas.seite_waehlen_x_mm", &[("wert1", format!("{}", app.options.offset_dist))]);
            } else {
                p.circle_stroke(w2s(cursor), 8.0, dashed);
                label = jjcad_core::i18n::tr("gui.canvas.objekt_anklicken").into();
            }
        }
        "modify.trim" => {
            // Der Abschnitt, den ein Klick hier entfernen würde (T0075).
            draw_trim_preview(app, p, rect, view, cursor);
        }
        "modify.extend" => {
            // Die Strecke, die ein Klick hier ansetzen würde (T0081).
            draw_extend_preview(app, p, rect, view, cursor);
        }
        "modify.intersect" => {
            // Nach dem ersten Klick: beide Strecken bis zum Schnittpunkt (T0082).
            draw_verschneiden_preview(app, p, rect, view, cursor);
        }
        "modify.explode" => {
            // Das Objekt unter dem Zeiger, das ein Klick auflösen würde (T0093).
            draw_explode_preview(app, p, rect, view, cursor);
        }
        "modify.area.split" => {
            // Die Schnittlinie als Polygonzug; ab dem ersten Punkt zeigt die
            // gestrichelte Strecke, wie sie weiterläuft.
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
                        let stroke2 = Stroke::new(1.2, Palette::select().gamma_multiply(0.8));
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
        paint_label(p, rect, label_pos, &label);
    }

}

/// Hervorhebung des Abschnitts, den „Stutzen“ an dieser Stelle entfernen würde
/// (T0075): steht der Zeiger auf einer Linie, ist der Bereich zwischen den
/// beiden Schnittkanten in der Warnfarbe zu sehen — vor dem Klick, ohne die
/// Zeichnung zu ändern.
///
/// Der Abschnitt kommt aus [`actions::trim_vorschau`] — derselben Rechnung,
/// die der Klick in `modify.trim` schneidet.
fn draw_trim_preview(app: &App, p: &Painter, rect: Rect, view: View, at: V2) {
    let Some((id, segs)) = vorschau_objekt(app, view, at, actions::trim_vorschau) else {
        return;
    };
    draw_abschnitt_preview(app, p, rect, view, id, &segs, Palette::danger());
}

/// Hervorhebung der Strecke, die „Verlängern“ an dieser Stelle ansetzen würde
/// (T0081): steht der Zeiger auf einem Ende, ist der Weg bis zur nächsten
/// Schnittkante vorher in der Erfolgsfarbe zu sehen — vor dem Klick, ohne die
/// Zeichnung zu ändern.
///
/// Die Strecke kommt aus [`actions::extend_vorschau`] — derselben Rechnung,
/// die der Klick in `modify.extend` ausführt.
fn draw_extend_preview(app: &App, p: &Painter, rect: Rect, view: View, at: V2) {
    let Some((id, segs)) = vorschau_objekt(app, view, at, actions::extend_vorschau) else {
        return;
    };
    draw_abschnitt_preview(app, p, rect, view, id, &segs, Palette::ok());
}

/// Hervorhebung beim Verschneiden (T0082): ist das erste Objekt gemerkt und
/// steht der Zeiger auf dem zweiten, sind beide Strecken bis zum gemeinsamen
/// Schnittpunkt in der Erfolgsfarbe zu sehen — vor dem Klick.
///
/// Die Strecken kommen aus [`actions::verschneiden_vorschau`] — derselben
/// Rechnung, die der Klick in `modify.intersect` ausführt.
fn draw_verschneiden_preview(app: &App, p: &Painter, rect: Rect, view: View, at: V2) {
    let Some(erstes) = app.tool.as_ref().and_then(|t| t.obj) else {
        return;
    };
    let tol = (8.0 / view.zoom).max(0.5);
    let Some(zweites) = snap::hit_test(app.doc(), at, tol, None) else {
        return;
    };
    if zweites == erstes {
        return;
    }
    for (id, segs) in actions::verschneiden_vorschau(app.doc(), erstes, zweites) {
        draw_abschnitt_preview(app, p, rect, view, id, &segs, Palette::ok());
    }
}

/// Hebt das Objekt hervor, das ein Klick mit „Zerlegen“ auflösen würde (T0093).
///
/// Ohne Auswahl ist Zerlegen ein Klick-Werkzeug: jedes angeklickte Objekt wird
/// in seine Einzelteile zerlegt. Vor dem Klick zeigt die Warnfarbe der
/// Stutzen-Vorschau ([`Palette::danger()`]), welches Objekt der Zeiger trifft.
fn draw_explode_preview(app: &App, p: &Painter, rect: Rect, view: View, at: V2) {
    let tol = (8.0 / view.zoom).max(0.5);
    let Some(id) = snap::hit_test(app.doc(), at, tol, None) else {
        return;
    };
    let Some(e) = app.doc().entity(id) else {
        return;
    };
    // Was sich nicht zerlegen lässt, wird nicht angekündigt: runde Zierschrift und
    // ein Block ohne Definition melden ihren Grund erst beim Klick.
    match &e.kind {
        EntityKind::Text { font, .. } if !font.explodierbar() => return,
        EntityKind::BlockRef { block, .. } if app.doc().block(block).is_none() => return,
        _ => {}
    }
    draw_objekt_hervorhebung(app, p, rect, view, e, Palette::danger());
}

/// Zeichnet ein Objekt vollständig in einer Farbe über die Zeichnung.
///
/// Grundlage ist die Displayliste des Objekts — dieselbe Geometrie, die auch
/// angezeigt und beim Zerlegen in Einzelteile aufgelöst wird. Der Strich ist
/// dicker als der des Objekts, damit die Hervorhebung auch auf dünnen Linien
/// auffällt (dieselbe Regel wie bei der Stutzen-Vorschau).
fn draw_objekt_hervorhebung(
    app: &App,
    p: &Painter,
    rect: Rect,
    view: View,
    e: &Entity,
    farbe: Color32,
) {
    let ctx = render::RenderCtx::for_entity(app.doc(), e);
    let mut dl = render::DisplayList::default();
    render::render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    let w2s = |q: V2| world_to_screen(rect, view, q);
    let breite = linienbreite_px(
        app.doc().effective_line_weight(e),
        view.zoom * app.doc().settings.annotation_scale,
        app.ui.lineweights_scaled,
    );
    let strich = Stroke::new((breite + 1.0).max(3.0) as f32, farbe);
    for prim in &dl.prims {
        match &prim.shape {
            PShape::Line(a, b) => {
                p.line_segment([w2s(*a), w2s(*b)], strich);
            }
            PShape::Poly(pts, closed) => {
                if pts.len() < 2 {
                    continue;
                }
                let mut v: Vec<Pos2> = pts.iter().map(|q| w2s(*q)).collect();
                if *closed {
                    v.push(v[0]);
                }
                stroke_pts(p, &v, strich.width, farbe);
            }
            // Flächen: nur der Umriss (Umgrenzung und Inseln), damit die Zeichnung
            // darunter sichtbar bleibt.
            PShape::Fill { outer, holes } | PShape::FillOrdered { outer, holes } => {
                for ring in std::iter::once(outer).chain(holes.iter()) {
                    if ring.len() < 2 {
                        continue;
                    }
                    let mut v: Vec<Pos2> = ring.iter().map(|q| w2s(*q)).collect();
                    v.push(v[0]);
                    stroke_pts(p, &v, strich.width, farbe);
                }
            }
            PShape::Circle { c, r } => {
                p.add(kreis_zug(w2s(*c), (*r * view.zoom) as f32, strich));
            }
            PShape::Arc { c, r, a0, a1 } => {
                let arc = Arc::new(*c, *r, *a0, *a1);
                let pts: Vec<Pos2> = CurveQuality::SCREEN
                    .flatten(&arc, view.zoom)
                    .iter()
                    .map(|q| w2s(*q))
                    .collect();
                if pts.len() >= 2 {
                    p.add(Shape::line(pts, strich));
                }
            }
            PShape::Ellipse { c, rx, ry, rot } => {
                let pts: Vec<Pos2> = (0..=72)
                    .map(|i| {
                        let t = i as f64 / 72.0 * std::f64::consts::TAU;
                        w2s(*c + V2::new(rx * t.cos(), ry * t.sin()).rotated(*rot))
                    })
                    .collect();
                stroke_pts(p, &pts, strich.width, farbe);
            }
            PShape::EllipseArc { c, rx, ry, rot, a0, a1 } => {
                let el = Ellipse::new(*c, *rx, *ry, *rot);
                let pts: Vec<Pos2> = el
                    .arc_points(*a0, *a1, 0.15 / view.zoom.max(1e-9))
                    .iter()
                    .map(|q| w2s(*q))
                    .collect();
                stroke_pts(p, &pts, strich.width, farbe);
            }
            PShape::Dot { p: pos, .. } => {
                p.circle_filled(w2s(*pos), 3.0, farbe);
            }
            // Bild: hervorgehoben wird der Rahmen (die Textur bliebe sonst
            // unverändert und wäre als Hervorhebung nicht zu erkennen).
            PShape::Image { a, b, hoehe, .. } => {
                let ecken = jjcad_core::bild::ecken(*a, *b, *hoehe);
                let pts: Vec<Pos2> = ecken.iter().map(|q| w2s(*q)).collect();
                p.add(Shape::closed_line(pts, strich));
            }
            // Text: der Kasten, den auch Auswahlrahmen und Einfüge-Vorschau benutzen
            // (`render::text_corners`).
            PShape::Text(t) => {
                let kasten = render::text_corners(t);
                let pts: Vec<Pos2> = kasten.iter().map(|q| w2s(*q)).collect();
                p.add(Shape::closed_line(pts, strich));
            }
        }
    }
}

/// Das Objekt unter dem Zeiger und die Strecken, die seine Vorschau markiert.
///
/// Stutzen und Verlängern teilen den Weg: Treffer am Zeiger, dann die
/// Vorschau-Rechnung der jeweiligen Aktion. Ohne Treffer oder ohne Strecke
/// gibt es nichts zu zeigen.
fn vorschau_objekt(
    app: &App,
    view: View,
    at: V2,
    rechnung: fn(&Document, Id, V2, &[Id]) -> Vec<Seg>,
) -> Option<(Id, Vec<Seg>)> {
    let tol = (8.0 / view.zoom).max(0.5);
    let id = snap::hit_test(app.doc(), at, tol, None)?;
    if app.doc().entity(id).is_none() {
        return None;
    }
    let segs = rechnung(app.doc(), id, at, &[]);
    if segs.is_empty() {
        None
    } else {
        Some((id, segs))
    }
}

/// Zeichnet die Strecken einer Vorschau in einer Farbe: bei einer Wand als
/// Körper, damit auch die Dicke stimmt, sonst als Linie.
///
/// Der Strich folgt der Strichstärke des Objekts (T0075): bei fetten Linien
/// deckt er die Linie ab, im DPI-Modus wächst er mit dem Zoom. Ein Mindestmaß
/// hält ihn auch auf dünnen Linien sichtbar, der Zuschlag lässt die Linie
/// links und rechts darunter hervorschauen.
fn draw_abschnitt_preview(
    app: &App,
    p: &Painter,
    rect: Rect,
    view: View,
    id: Id,
    segs: &[Seg],
    farbe: Color32,
) {
    let w2s = |q: V2| world_to_screen(rect, view, q);
    if let Some(EntityKind::Wall {
        thickness, align, ..
    }) = app.doc().entity(id).map(|e| &e.kind)
    {
        let t = thickness * 0.5;
        for s in segs {
            if let Seg::Line(a, b) = s {
                let n = (*b - *a).normalized().perp();
                let off = n * (t * align.offset());
                let quad = vec![
                    w2s(*a + off + n * t),
                    w2s(*b + off + n * t),
                    w2s(*b + off - n * t),
                    w2s(*a + off - n * t),
                ];
                p.add(Shape::convex_polygon(
                    quad,
                    farbe.gamma_multiply(0.30),
                    Stroke::NONE,
                ));
            }
        }
    }
    let Some(e) = app.doc().entity(id) else {
        return;
    };
    let breite = linienbreite_px(
        app.doc().effective_line_weight(e),
        view.zoom * app.doc().settings.annotation_scale,
        app.ui.lineweights_scaled,
    );
    let strich = Stroke::new((breite + 1.0).max(3.0) as f32, farbe);
    for s in segs {
        match s {
            Seg::Line(a, b) => {
                p.line_segment([w2s(*a), w2s(*b)], strich);
            }
            Seg::Arc(arc) => {
                let pts: Vec<Pos2> = arc.flatten(1.0).iter().map(|q| w2s(*q)).collect();
                if pts.len() >= 2 {
                    p.add(Shape::line(pts, strich));
                }
            }
        }
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
        text_deckel: app.prefs.textaufloesung.px(),
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
    // Der Schwenk mit der Tastatur läuft **nicht** hier: die vier Richtungen
    // sind gewöhnliche Funktionen der Tastaturbelegung (`view.pan_*`, Vorgabe
    // W/A/S/D) und werden wie jede andere Funktion in `App::handle_shortcuts`
    // ausgelöst (T0089). Ein Dauerlauf auf `key_down` bewegte die Ansicht
    // zusätzlich — auch bei Strg+S und unabhängig von der Hotbar.
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

/// Bezugspunkte für Ortho und Polar (Winkelfang) in der aktuellen Lage.
///
/// Beim **Erstellen** ist das der zuletzt gesetzte Punkt. Beim **Manipulieren**
/// fehlte er bisher ganz: der Griffzug hatte damit keine Zwangsrichtung und
/// ließ sich nicht waagerecht, senkrecht oder unter 45° ausrichten (T0046).
/// Deshalb liefert der laufende Griffzug die ruhenden Punkte des Elements
/// (`handles::bezugspunkte`) — an einem Stützpunkt sind das **zwei** (voriger
/// und nächster), damit beide angrenzenden Linien gleichzeitig fangen (Turn 3);
/// die Maus-Verschiebung liefert ihren Griffpunkt, wie das Werkzeug
/// „Verschieben" über seinen Basispunkt.
fn bezug_achsen(app: &App) -> Vec<V2> {
    if let Some(t) = app.tool.as_ref() {
        return t.pts.last().copied().into_iter().collect();
    }
    if let Some(drag) = app.grip_drag.as_ref() {
        return crate::handles::bezugspunkte(&drag.orig, drag.kind);
    }
    app.move_drag
        .as_ref()
        .map(|md| vec![md.start])
        .unwrap_or_default()
}

/// Fang-Kette für den Zeiger in der aktuellen Werkzeuglage.
///
/// Liefert den zu übernehmenden Weltpunkt und das Label für die Statuszeile.
/// **Einziger Weg** für Zeigeranzeige (`show`) und Klickpunkt (`snap_at`) —
/// laufen sie getrennt, weicht der gespeicherte Punkt vom sichtbaren
/// Fangpunkt ab. Reihenfolge der Fänge: `snap::snap_constrained`.
fn fang_zeiger(app: &App, w: V2, tol: f64) -> (V2, Option<&'static str>) {
    let res = fang_aufloesung(app, w, tol);
    (res.point, res.text())
}

/// Wie [`fang_zeiger`], gibt aber zusätzlich die **Art** des Fangs zurück.
///
/// Die Zeichenfläche braucht sie, um einen unsichtbaren Mittelpunkt beim
/// Fangen zu zeigen (T0117): der Punkt ist im Plan nicht gezeichnet, die
/// allgemeine Fangmarke allein ließe nicht erkennen, dass es der Kreismittel-
/// punkt ist.
fn fang_zeiger_art(
    app: &App,
    w: V2,
    tol: f64,
) -> (V2, Option<&'static str>, Option<snap::SnapKind>) {
    let res = fang_aufloesung(app, w, tol);
    let kind = match res.source {
        snap::SnapSource::Object(k) => Some(k),
        _ => None,
    };
    (res.point, res.text(), kind)
}

/// Die eine Fangrechnung für Zeigeranzeige und Klickpunkt.
fn fang_aufloesung(app: &App, w: V2, tol: f64) -> snap::SnapResolution {
    let achsen = bezug_achsen(app);
    let regeln = fang_regeln(app);
    let vorschau = vorschau_punkte(app);
    // `snap_constrained_achsen`: die Haltepunkte des erweiterten Punktfangs
    // liefern ihre Hilfslinien als zusätzliche Fangziele mit; mehrere
    // Bezugspunkte (beide Nachbarn eines Griffs) rasten getrennt je Koordinate.
    snap::snap_constrained_achsen(
        app.doc(),
        w,
        tol,
        &achsen,
        &regeln.ignore,
        &regeln.keep,
        &regeln.gesperrt,
        &vorschau,
        &app.halt,
        app.ui.show_grid,
    )
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
/// * **Wand im Griffzug:** die Wand fällt ganz aus dem Fang. Ihre Stirn- und
///   Gehrungsecken hängen am gezogenen Ende und wandern mit; der Fang zöge den
///   Punkt auf eine Ecke, die dadurch weiterwandert — der Fang oszilliert
///   (T0051).
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
        // Wand (T0051): Ihre Stirn- und Gehrungsecken hängen am gezogenen
        // Endpunkt und wandern mit. Liegt der Griff im Fangradius einer
        // solchen Ecke, zieht der Fang die Wand auf die Ecke, die Ecke wandert
        // aus dem Radius, im Bild darauf greift sie wieder — der Punkt
        // oszilliert. Eine Wand fängt beim Manipulieren deshalb **gar nicht**
        // auf sich selbst, auch nicht mit ihren diskreten Ecken.
        let eigene_wand = app
            .doc()
            .entity(drag.entity)
            .map(|e| matches!(e.kind, EntityKind::Wall { .. }))
            .unwrap_or(false);
        if eigene_wand {
            return FangRegeln {
                ignore: vec![drag.entity],
                ..FangRegeln::default()
            };
        }
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
            if matches!(
                t.action.as_str(),
                "draw.polyline" | "draw.spline" | "dim.chain" | "dim.room"
            ) || punktfolge
                || t.action == "modify.area.split"
            {
                if t.pts.len() >= 2 {
                    finish_tool(app, true);
                    return;
                }
            }
            // Schnitt-Kote: schon ein Punkt ist ein vollständiges Objekt
            // (erster Punkt = ±0,00).
            if t.action == "dim.elevation.section" && !t.pts.is_empty() {
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
                    jjcad_core::i18n::t_args_owned("gui.canvas.x_enter_laenge", &[("eingabe", format!("{}", text))])
                },
                FontId::proportional(13.0),
                Palette::accent(),
            );
            let bg = Rect::from_min_size(
                pos + Vec2::new(16.0, 18.0),
                galley.size() + Vec2::new(12.0, 6.0),
            );
            painter.rect_filled(bg, 4.0, Palette::hinweis_flaeche());
            painter.rect_stroke(bg, 4.0, Stroke::new(1.0, Palette::accent()));
            painter.galley(pos + Vec2::new(22.0, 21.0), galley, Palette::accent());
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

    // Doppelklick auf einen Text: der zweite Druck hat einen Mauszug gestartet
    // (T0063) — der Klick gehört der Texteingabe, der Zug wird still verworfen
    // (T0019). Muss vor `laufender_drag` stehen, das den Zug sonst abschließt.
    if app.tool.is_none()
        && app.move_drag.is_some()
        && resp.double_clicked_by(egui::PointerButton::Primary)
    {
        if let Some(p) = raw.or(snapped) {
            if let Some(id) = crate::textedit::text_bei(app, p) {
                app.verwerfe_move_drag();
                crate::textedit::start_entitaet(app, id);
                return;
            }
        }
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
                            if above { jjcad_core::i18n::tr("gui.canvas.vor_element_gesetzt") } else { jjcad_core::i18n::tr("gui.canvas.hinter_element_gesetzt") }.to_string(),
                        );
                    }
                    None => app.toast(jjcad_core::i18n::tr("gui.canvas.reihenfolge_kein_element_getroffen").to_string()),
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
                "draw.polyline" | "draw.spline" | "dim.chain" | "dim.room" => {
                    finish_tool(app, true);
                }
                "dim.elevation.section" => {
                    if tool.pts.is_empty() {
                        app.tool = None;
                    } else {
                        finish_tool(app, true);
                    }
                }
                // Füllflächen aus Punkten: Rechtsklick schließt wie Enter. Bei
                // zu wenigen Punkten gibt es nichts zu schließen — dann bricht
                // er das Werkzeug ab (für beide Füll-Werkzeuge gleich).
                action if ist_fuell_punkte(action) => {
                    if tool.pts.len() >= 3 {
                        finish_tool(app, true);
                    } else {
                        app.tool = None;
                        app.mitteilung(jjcad_core::i18n::tr("gui.canvas.abgebrochen"));
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
            // Klick auf ein bereits ausgewaehltes Objekt: Auswahl verschieben —
            // je nach Einstellung „Maus verschiebt“ (T0063). Der zweite Druck
            // eines Doppelklicks auf einem Text startet keinen Zug: er oeffnet
            // die Texteingabe (T0019), der Zug wuerde sie verschlucken.
            if let Some(r) = raw {
                let tol2 = 10.0 / view.zoom.max(1e-6);
                let (frame, doppelklick_druck) = resp.ctx.input(|i| {
                    (
                        i.time,
                        i.pointer.button_double_clicked(egui::PointerButton::Primary),
                    )
                });
                if let Some(hit) = snap::hit_test(app.doc(), r, tol2, None) {
                    let text = matches!(
                        app.doc().entity(hit).map(|e| &e.kind),
                        Some(EntityKind::Text { .. })
                    );
                    if app.selection().contains(&hit)
                        && !(text && doppelklick_druck)
                        && app.begin_move_drag_bei_klick(hit, r, frame)
                    {
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
        Stroke::new(1.2, if crossing { Palette::ok() } else { Palette::select() }),
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
            Stroke::new(1.0, Palette::select().gamma_multiply(0.55)),
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
    let stroke = Stroke::new(1.2, Palette::accent());
    for id in selected {
        let Some(e) = doc.entity(*id) else { continue };
        let EntityKind::Wall { a, b, .. } = &e.kind else {
            continue;
        };
        let pa = world_to_screen(rect, view, *a);
        let pb = world_to_screen(rect, view, *b);
        p.extend(Shape::dashed_line(&[pa, pb], stroke, 9.0, 5.0));
        for q in [pa, pb] {
            p.circle_filled(q, 2.2, Palette::accent());
        }
    }
}

/// Marke für einen Basispunkt (T0091): kleiner Kreis mit Fadenkreuz.
fn draw_basispunkt_marke(p: &Painter, c: Pos2) {
    let stroke = Stroke::new(1.4, Palette::accent());
    p.circle_stroke(c, 5.0, stroke);
    p.line_segment([c - Vec2::new(9.0, 0.0), c + Vec2::new(9.0, 0.0)], stroke);
    p.line_segment([c - Vec2::new(0.0, 9.0), c + Vec2::new(0.0, 9.0)], stroke);
}

/// Basispunkt ausgewählter Türen (T0091) — der Punkt, an dem eine neue Tür
/// am Mauszeiger hängt.
fn draw_tuer_basispunkte(app: &App, p: &Painter, rect: Rect, view: View, selected: &[Id]) {
    if selected.is_empty() || selected.len() > 60 {
        return;
    }
    let doc = app.doc();
    for id in selected {
        let Some(e) = doc.entity(*id) else { continue };
        let Some(bp) = render::door_base_point(e) else {
            continue;
        };
        draw_basispunkt_marke(p, world_to_screen(rect, view, bp));
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
                        if hot { Palette::text() } else { Palette::accent() },
                    );
                    p.rect_stroke(r, egui::Rounding::same(1.5), Stroke::new(1.2_f32, Palette::bg()));
                }
                // Bogenenden: runder Griff — der Punkt läuft auf dem Kreis mit
                crate::handles::GripKind::ArcStart | crate::handles::GripKind::ArcEnd => {
                    p.circle_filled(
                        c,
                        size * 0.55,
                        if hot { Palette::text() } else { Palette::accent() },
                    );
                    p.circle_stroke(c, size * 0.55, Stroke::new(1.2_f32, Palette::bg()));
                }
                // Raumstempel: runder Griff mit Kreuz — verschiebt nur die
                // Beschriftung, die Fläche bleibt stehen (T0097).
                crate::handles::GripKind::Label => {
                    p.circle_filled(
                        c,
                        size * 0.7,
                        if hot { Palette::text() } else { Palette::accent() },
                    );
                    p.circle_stroke(c, size * 0.7, Stroke::new(1.2_f32, Palette::bg()));
                    let r = size * 0.7;
                    p.line_segment(
                        [c - Vec2::new(r * 0.6, 0.0), c + Vec2::new(r * 0.6, 0.0)],
                        Stroke::new(1.2_f32, Palette::bg()),
                    );
                    p.line_segment(
                        [c - Vec2::new(0.0, r * 0.6), c + Vec2::new(0.0, r * 0.6)],
                        Stroke::new(1.2_f32, Palette::bg()),
                    );
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
                    p.add(Shape::convex_polygon(pts, Palette::accent(), Stroke::new(1.2_f32, Palette::bg())));
                    // zwei kurze Pfeile in Normalenrichtung
                    p.line_segment(
                        [c + n * 7.0, c + n * 11.0],
                        Stroke::new(1.4_f32, Palette::accent()),
                    );
                    p.line_segment(
                        [c - n * 7.0, c - n * 11.0],
                        Stroke::new(1.4_f32, Palette::accent()),
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

    let stroke = Stroke::new(1.5, Palette::accent());
    let mut dl = jjcad_core::render::DisplayList::default();
    dl.prims = geo.prims.clone();
    for prim in &dl.prims {
        ghost_shape(
            p,
            &prim.shape,
            &w2s,
            zoom,
            stroke,
            view.rot as f32,
            app.prefs.textaufloesung.px(),
        );
    }

    // Umriss und Ankerpunkt
    if geo.bbox.is_valid() {
        let a = w2s(V2::new(geo.bbox.min.x, geo.bbox.max.y));
        let b = w2s(V2::new(geo.bbox.max.x, geo.bbox.min.y));
        let r = Rect::from_two_pos(a, b);
        p.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, Palette::select().gamma_multiply(0.55)),
        );
    }
    let anchor = w2s(g.pos);
    let c = Palette::accent();
    p.line_segment([anchor - Vec2::new(6.0, 0.0), anchor + Vec2::new(6.0, 0.0)], Stroke::new(1.4, c));
    p.line_segment([anchor - Vec2::new(0.0, 6.0), anchor + Vec2::new(0.0, 6.0)], Stroke::new(1.4, c));

    // Kurzanzeige am Zeiger (der ausführliche Hinweis steht in der Optionsleiste)
    let mut badge = String::new();
    if g.rot_deg().abs() > 0.1 {
        badge.push_str(&format!("{:.0}°  ", g.rot_deg()));
    }
    if g.mirror {
        badge.push_str(jjcad_core::i18n::tr("gui.canvas.gespiegelt"));
    }
    if !badge.is_empty() {
        p.text(
            anchor + Vec2::new(12.0, -10.0),
            Align2::LEFT_BOTTOM,
            badge,
            FontId::proportional(11.0),
            Palette::accent(),
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
    deckel: f32,
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
            p.add(kreis_zug(w2s(*c), (*r * zoom) as f32, stroke));
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
        PShape::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = jjcad_core::geom::Ellipse::new(*c, *rx, *ry, *rot);
            let pts: Vec<Pos2> = el
                .arc_points(*a0, *a1, 0.15 / zoom.max(1e-9))
                .iter()
                .map(|q| w2s(*q))
                .collect();
            p.add(egui::Shape::line(pts, stroke));
        }
        // Text war hier nicht gezeichnet: die Einfüge-Vorschau zeigte bei
        // kopierten Textfeldern nur den Anker, keinen Text (T0025). Gezeichnet
        // über `paint_text` — dieselbe Lage, Schrift und Drehung wie danach.
        PShape::Text(t) => paint_text(p, t, w2s(t.pos), zoom, Palette::accent(), rot, deckel),
        PShape::Dot { p: pos, r } => {
            p.circle_filled(w2s(*pos), ((*r * zoom) as f32).max(1.2), stroke.color);
        }
        // Füllungen haben in der Vorschau keine Kontur — die Umrandung
        // zeichnet `draw_ghost` (äußerer Rahmen) bzw. das Objekt selbst.
        PShape::Fill { .. } | PShape::FillOrdered { .. } => {}
        // Bild: in der Einfüge-Vorschau nur der Rahmen (die Textur gehört zur
        // Zeichenfläche, nicht zur Vorschau-Zeichnung).
        PShape::Image { a, b, hoehe, .. } => {
            let ecken = jjcad_core::bild::ecken(*a, *b, *hoehe);
            let pts: Vec<Pos2> = ecken.iter().map(|q| w2s(*q)).collect();
            p.add(egui::Shape::closed_line(pts, stroke));
        }
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

/// Entsteht gerade ein Punkt? Werkzeug, Griffzug oder Maus-Verschiebung —
/// nur dann gibt es einen „nächsten Punkt", für den der erweiterte Punktfang
/// (Verweilen, Hilfslinien) sinnvoll ist.
///
/// Der Griffzug war hier früher ausgenommen (nur `app.tool`): beim
/// Manipulieren wurden die Haltepunkte jedes Bild geleert, der erweiterte
/// Punktfang war damit tot (T0046, Turn 2).
fn fang_aktiv(app: &App) -> bool {
    app.tool.is_some() || app.grip_drag.is_some() || app.move_drag.is_some()
}

/// Fangpunkt unter dem Zeiger, über dem der erweiterte Punktfang auslösen darf.
///
/// Nur **diskrete** Punkte (Endpunkt, Blattecke, Schnittpunkt, Mittelpunkt,
/// Zentrum, Quadrant): „Nächster Punkt" wandert mit dem Zeiger und das Raster
/// liegt überall — beide stünden nie still und würden bei jedem Verweilen
/// Hilfslinien erzeugen.
fn verweil_kandidat(app: &App, w: V2, tol: f64) -> Option<snap::HaltPunkt> {
    // Derselbe Bezug wie beim Fangen (`bezug_achsen`): beim Griffzug der
    // nächstgelegene ruhende Punkt des Elements, sonst der zuletzt gesetzte.
    let reference = bezug_achsen(app).into_iter().next();
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
    let col = Palette::track();
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
/// Ecken ergeben keine Fläche — den Sonderfall zweier Punkte erlaubt
/// [`fill_boundary_points_rechteck`].
pub fn fill_boundary_points(pts: &[V2]) -> Option<Vec<V2>> {
    fill_boundary_points_mit_diagonale(pts, false)
}

/// Wie [`fill_boundary_points`], erlaubt aber den Sonderfall **zweier**
/// Punkte: sie sind die Diagonale einer achsenparallelen Fläche (T0131).
///
/// Das gilt für Füllfläche, Schraffur und Volumen — überall dort, wo der
/// Nutzer den Umriss selbst klickt. Liegen beide Punkte auf einer
/// Waagerechten oder Senkrechten, entsteht keine Fläche und es bleibt bei
/// `None` (die Meldung nennt dann die Diagonale).
pub fn fill_boundary_points_rechteck(pts: &[V2]) -> Option<Vec<V2>> {
    fill_boundary_points_mit_diagonale(pts, true)
}

fn fill_boundary_points_mit_diagonale(pts: &[V2], diagonale: bool) -> Option<Vec<V2>> {
    let mut out: Vec<V2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().map(|q: &V2| q.dist(*p) > 0.5).unwrap_or(true) {
            out.push(*p);
        }
    }
    if out.len() > 1 && out[0].dist(*out.last().unwrap()) <= 0.5 {
        out.pop();
    }
    if out.len() == 2 && diagonale {
        // Zwei Punkte als Diagonale: die vier Ecken liegen auf den Achsen.
        if let Some(ecken) = jjcad_core::geom::rechteck_aus_diagonale(out[0], out[1]) {
            return Some(ecken.to_vec());
        }
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
    let Some(boundary) = fill_boundary_points_rechteck(umriss) else {
        return;
    };
    let w2s = |q: V2| world_to_screen(rect, view, q);

    // Fläche andeuten — der Plan darunter bleibt lesbar
    let screen: Vec<Pos2> = boundary.iter().map(|q| w2s(*q)).collect();
    if !fill_too_thin(&screen) {
        let c = Palette::accent().gamma_multiply(0.14);
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
        text_deckel: app.prefs.textaufloesung.px(),
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
        text_deckel: app.prefs.textaufloesung.px(),
    };
    for prim in &dl.prims {
        paint_prim(p, prim, &w2s, view.zoom, &pc, 1.0);
    }
    // Ohne Umrandung am Objekt die Grenze gestrichelt nachziehen, damit die
    // Fläche nicht ohne Kante steht. Mit Umrandung zeichnet der Renderer sie
    // bereits — eine zweite Linie würde sie nur dicker machen.
    if !app.options.fill_border_paint {
        let stroke = Stroke::new(1.2, Palette::accent().gamma_multiply(0.85));
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
    let Some(boundary) = fill_boundary_points_rechteck(&tool.pts) else {
        // Zwei Punkte auf einer Waagerechten oder Senkrechten sind keine
        // Diagonale — die Meldung sagt, was fehlt (T0131). Sonst bleibt es bei
        // der Zahl der nötigen Ecken.
        let zwei = tool.pts.len() == 2 && tool.pts[0].dist(tool.pts[1]) > 0.5;
        app.toast(jjcad_core::i18n::tr(if zwei {
            "gui.canvas.fuellflaeche_diagonale_auf_einer_achse"
        } else {
            "gui.canvas.fuellflaeche_mindestens_3_punkte"
        }));
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
        // Volumen: alle Werte kommen aus dem Platzieren-Panel (dieselben
        // Zeilen wie am vorhandenen Objekt). Standard ist die volle Fläche im
        // Wandton — ein Körper ist Mauerwerk.
        params["height"] = json!(app.unit().from_mm(app.options.volume_height));
        params["pattern"] = json!(app.options.volume_pattern.key());
        params["angle"] = json!(app.options.volume_angle.to_degrees());
        params["scale"] = json!(app.unit().from_mm(app.options.volume_scale));
        params["border"] = json!(app.options.fill_border_for(action));
    }
    let ok = run_und_waehlen(app, action, params);
    if ok {
        app.tool = None;
    }
    ok
}

/// Schließt einen Raumstempel aus der geklickten Punktfolge.
///
/// Die Eckpunkte entstehen mit [`fill_boundary_points`] (doppelte Punkte,
/// Schluss auf dem Start), die Beschriftung setzt die Action selbst in den
/// „Pole of Inaccessibility". Höhe und Überschrift kommen aus dem
/// Eigenschaften-Panel — dieselben Zeilen wie am vorhandenen Stempel.
fn finish_room(app: &mut App) -> bool {
    let Some(tool) = app.tool.clone() else {
        return false;
    };
    let Some(boundary) = fill_boundary_points(&tool.pts) else {
        app.toast(jjcad_core::i18n::tr("gui.canvas.raumstempel_mindestens_3_punkte"));
        return false;
    };
    let pts: Vec<[f64; 2]> = boundary
        .iter()
        .map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)])
        .collect();
    let params = json!({
        "pts": pts,
        "height": app.unit().from_mm(app.options.room_height),
        "heading": app.options.room_heading,
    });
    let ok = run_und_waehlen(app, "dim.room", params);
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
        "dim.room" => {
            finish_room(app);
        }
        "modify.area.split" => {
            // Die Punktfolge ist die Schnittlinie. Ausgewählte Füllflächen
            // bestimmen das Ziel; ohne Auswahl findet die Action die Fläche
            // unter der Linie selbst.
            if tool.pts.len() >= 2 {
                let ids: Vec<Id> = app
                    .selection()
                    .iter()
                    .copied()
                    .filter(|id| {
                        matches!(
                            app.doc().entity(*id).map(|e| &e.kind),
                            Some(EntityKind::Fill { .. })
                        )
                    })
                    .collect();
                let _ = run(
                    app,
                    "modify.area.split",
                    json!({"pts": tool.pts.iter().map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)]).collect::<Vec<_>>(), "ids": ids}),
                );
            }
            app.tool = None;
        }
        "dim.elevation.section" => {
            if !tool.pts.is_empty() {
                let _ = run(
                    app,
                    "dim.elevation.section",
                    json!({"pts": tool.pts.iter().map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)]).collect::<Vec<_>>() }),
                );
            }
            app.tool = None;
        }
        "dim.chain" => {
            if tool.pts.len() >= 2 {
                let b = BBox::from_points(&tool.pts);
                let line_pos = V2::new(b.min.x, b.min.y - app.doc().settings.annotation_scale * 8.0);
                // Achse bestimmt die Action selbst (Dominanz der Punktdifferenz) —
                // eine hier geratene Achse ergäbe bei senkrechten Ketten „0,00" (T0038).
                let _ = run(
                    app,
                    "dim.chain",
                    json!({"pts": tool.pts.iter().map(|p| [app.unit().from_mm(p.x), app.unit().from_mm(p.y)]).collect::<Vec<_>>(),
                           "line_pos": [app.unit().from_mm(line_pos.x), app.unit().from_mm(line_pos.y)]}),
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
                return RotateStep::Reject(jjcad_core::i18n::tr("gui.canvas.drehen_abstand_zum_zentrum"));
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
        None => RotateStep::Reject(jjcad_core::i18n::tr("gui.canvas.drehen_abstand_zum_zentrum")),
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
        obj: None,
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
                        obj: None,
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
                        obj: None,
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
        // Raumstempel: dieselbe Punktfolge wie die Fläche, nur entsteht beim
        // Abschluss eine unsichtbare Fläche mit Textfeld.
        "dim.room" => {
            let doppelt = tool.pts.last().map(|q| q.dist(w) <= 0.5).unwrap_or(false);
            let am_start = tool.pts.first().map(|q| q.dist(w) <= 0.5).unwrap_or(false);
            if tool.pts.len() >= 3 && (doppelt || am_start) {
                finish_tool(app, true);
            } else {
                tool.pts.push(w);
                app.tool = Some(tool);
            }
        }
        "draw.fill.boundary" | "draw.fill.points" | "draw.volume.points" => {
            // Klick auf den letzten Punkt (Doppelklick) oder auf den Startpunkt
            // schließt den Umriss — wie Enter, nur mit der Maus. Ab **zwei**
            // Punkten, weil zwei Punkte als Diagonale eine rechteckige Fläche
            // ergeben (T0131).
            let doppelt = tool.pts.last().map(|q| q.dist(w) <= 0.5).unwrap_or(false);
            let am_start = tool.pts.first().map(|q| q.dist(w) <= 0.5).unwrap_or(false);
            if tool.pts.len() >= 2 && (doppelt || am_start) {
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
        // Bild: zwei gegenüberliegende Ecken legen Größe und Lage fest. Wie
        // beim Rechteck bleibt das Werkzeug danach aktiv (nächste Ecke).
        "draw.image" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let a = tool.pts[0];
                let b = w;
                if a.dist(b) > 0.5 {
                    match app.options.bild.clone() {
                        Some(q) => {
                            let ok = run(
                                app,
                                "draw.image",
                                json!({"a": ax(a), "b": ax(b), "name": q.name, "data": q.data,
                                       "lock_ratio": app.options.bild_lock}),
                            );
                            if ok {
                                restart_tool(app, "draw.image");
                            }
                        }
                        // Ohne gewählte Datei zuerst den Dialog öffnen — sonst
                        // startet das Werkzeug aus dem Inventar und tut nichts.
                        None => app.open_file_dialog(crate::dialogs::FileMode::ImportImage),
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
            if app.options.arc_mode == Bogenmodus::Mittelpunkt {
                // Mittelpunkt, Startpunkt (Radius), Endpunkt. Ein Klick auf
                // die zuletzt gesetzte Stelle zählt nicht doppelt.
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
                    let c = tool.pts[0];
                    let s0 = tool.pts[1];
                    let r = c.dist(s0);
                    // Endpunkt auf dem Startpunkt: kein Bogen (der nächste
                    // Klick zählt als Endpunkt).
                    if r > 0.5 && s0.dist(w) > 0.5 {
                        let a0 = (s0 - c).angle();
                        let a1 = (w - c).angle();
                        let _ = run(
                            app,
                            "draw.arc",
                            json!({"c": ax(c), "r": unit.from_mm(r), "a0": a0.to_degrees(), "a1": a1.to_degrees()}),
                        );
                        app.tool = None;
                    }
                }
                return;
            }
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
                            jjcad_core::i18n::tr("gui.canvas.volumen_geschlossene_kontur_anklicken")
                        } else {
                            jjcad_core::i18n::tr("gui.canvas.fuellflaeche_geschlossene_kontur_anklicken")
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
                app.toast(jjcad_core::i18n::tr("gui.canvas.tuer_nur_auf_einer_wand"));
                return;
            }
            let _ = run(
                app,
                "draw.door",
                json!({"center": ax(cp), "angle": wa, "host": [host.unwrap()],
                       "width": unit.from_mm(app.options.door_width),
                       "kind": kind, "hinge": app.options.hinge, "swing_in": app.options.swing_in,
                       "base": app.options.door_base.key()}),
            );
        }
        "draw.window" => {
            let (cp, wa, host) = match wall_under(app, w) {
                Some(v) => (v.0, v.1, Some(v.2)),
                None => (w, 0.0, None),
            };
            if host.is_none() {
                app.toast(jjcad_core::i18n::tr("gui.canvas.fenster_nur_auf_einer_wand"));
                return;
            }
            let _ = run(
                app,
                "draw.window",
                json!({"center": ax(cp), "angle": wa, "host": [host.unwrap()],
                       "width": unit.from_mm(app.options.window_width),
                       "sill": unit.from_mm(app.options.window_sill),
                       "height": unit.from_mm(app.options.window_height),
                       "align": match app.options.window_align {
                           jjcad_core::style::WallAlign::Left => "left",
                           jjcad_core::style::WallAlign::Center => "center",
                           jjcad_core::style::WallAlign::Right => "right",
                       }}),
            );
        }
        "draw.stair" => {
            // Drei Punkte: Lauflinie (Anfang, Ende), Breite (halber Abstand
            // zur Lauflinie, also bis zur Außenkante).
            match tool.pts.len() {
                0 => {
                    tool.pts.push(w);
                    app.tool = Some(tool);
                }
                1 => {
                    let a = tool.pts[0];
                    if a.dist(w) > 0.5 {
                        tool.pts.push(w);
                        app.tool = Some(tool);
                    }
                }
                _ => {
                    // Erster und zweiter Klick: Außenkante. Der dritte Klick
                    // setzt die Tiefe; die Lauflinie liegt in der Mitte.
                    let a0 = tool.pts[0];
                    let b0 = tool.pts[1];
                    let tiefe = basis_hoehe(a0, b0, w);
                    let breite = tiefe.abs();
                    if breite > 1.0 && a0.dist(b0) > 0.5 {
                        let nrm = (b0 - a0).normalized().perp();
                        let a = a0 + nrm * (tiefe * 0.5);
                        let b = b0 + nrm * (tiefe * 0.5);
                        let _ = run(
                            app,
                            "draw.stair",
                            json!({"a": ax(a), "b": ax(b), "width": unit.from_mm(breite),
                                   "steps": app.options.stair_steps,
                                   "height": unit.from_mm(app.options.stair_height),
                                   "auto": app.options.stair_auto,
                                   "cut": app.options.stair_cut}),
                        );
                    } else {
                        app.toast(jjcad_core::i18n::tr("gui.canvas.treppe_breite_zu_klein"));
                    }
                    app.tool = None;
                }
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
                let mut params =
                    json!({"a": ax(a), "b": ax(b), "line_pos": ax(lp), "axis": axis});
                // Eigener Maßtext aus dem Panel (leer = automatisch berechnet).
                if !app.options.dim_text.trim().is_empty() {
                    params["text"] = json!(app.options.dim_text.clone());
                }
                let _ = run(app, "dim.linear", params);
                app.tool = None;
            } else {
                app.tool = Some(tool);
            }
        }
        "dim.chain" => {
            tool.pts.push(w);
            app.tool = Some(tool);
        }
        "dim.elevation.plan" => {
            let z = app.options.elevation_z;
            let _ = run(
                app,
                "dim.elevation.plan",
                json!({"p": ax(w), "z": unit.from_mm(z)}),
            );
        }
        "dim.elevation.section" => {
            tool.pts.push(w);
            app.tool = Some(tool);
        }
        // Ansichtsfenster aufziehen: zwei Klicks spannen das Rechteck auf.
        // Es entsteht als normales Blattobjekt (`sheet.add_viewport`) und ist
        // danach ausgewählt — Griffe, Verschieben und Löschen wie bei jedem
        // anderen Objekt.
        "sheet.viewport_tool" => {
            if tool.pts.is_empty() {
                tool.pts.push(w);
                app.tool = Some(tool);
            } else {
                let a = tool.pts[0];
                let b = w;
                let sid = match &app.tab().tab {
                    TabKind::Sheet(id) => *id,
                    _ => {
                        app.tool = None;
                        return;
                    }
                };
                let (sw, sh) = app
                    .doc()
                    .sheet(sid)
                    .map(|s| s.size())
                    .unwrap_or((420.0, 297.0));
                let x = a.x.min(b.x).clamp(0.0, sw - 10.0);
                let y = a.y.min(b.y).clamp(0.0, sh - 10.0);
                let ww = (a.x - b.x).abs().min(sw - x);
                let hh = (a.y - b.y).abs().min(sh - y);
                // Auf 0,5 mm runden — Papier-Millimeter, keine Zeichnungsmaße.
                let runde = |v: f64| (v * 2.0).round() / 2.0;
                let (x, y, ww, hh) = (runde(x), runde(y), runde(ww), runde(hh));
                if ww > 8.0 && hh > 8.0 {
                    let center = app.doc().bbox().center();
                    let scale = app.doc().sheet(sid).map(|s| s.scale).unwrap_or(100.0);
                    let u = app.unit();
                    let cx = u.from_mm(center.x);
                    let cy = u.from_mm(center.y);
                    // Das neue Fenster ist danach ausgewählt — wie ein frisch
                    // gezeichnetes Objekt.
                    let _ = run_und_waehlen(
                        app,
                        "sheet.add_viewport",
                        json!({"sheet": sid, "x": x, "y": y, "w": ww, "h": hh,
                               "scale": scale, "show_frame": true,
                               "center": [cx, cy]}),
                    );
                }
                app.tool = None;
            }
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
                app.toast(jjcad_core::i18n::tr("gui.canvas.kopieren_mit_basispunkt_erst_objekte_waehlen"));
                return;
            }
            app.merke_zwischenablage(&ids, Some(w));
            app.tool = None;
            app.toast(jjcad_core::i18n::tr("gui.canvas.auswahl_kopiert"));
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
                    app.toast(&jjcad_core::i18n::t_args_owned("gui.canvas.drehen_x_2", &[("wert1", format!("{}", format_number(angle, 1)))]));
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
                    app.toast(&jjcad_core::i18n::t_args_owned("gui.canvas.ansicht_gedreht_x", &[("wert1", format!("{}", format_number(angle, 1)))]));
                }
                RotateStep::Reject(_) => {
                    app.toast(jjcad_core::i18n::tr("gui.canvas.ansicht_drehen_abstand_zum_zentrum_noetig"))
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
                    app.toast(jjcad_core::i18n::tr("gui.canvas.versetzen_objekt_anklicken"));
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
        // Verschneiden (T0082): erstes Objekt merken, zweites verschnitten.
        // Der Punktpuffer taugt dafür nicht — die Action braucht die Kennung.
        "modify.intersect" => {
            let tol = (8.0 / app.current_view().zoom).max(0.5);
            let Some(id) = snap::hit_test(app.doc(), w, tol, None) else {
                app.toast(jjcad_core::i18n::tr("gui.canvas.verschneiden_objekt_anklicken"));
                return;
            };
            match tool.obj {
                None => {
                    tool.obj = Some(id);
                    app.tool = Some(tool);
                    app.session_mut().select(vec![id]);
                }
                Some(erstes) if erstes == id => {
                    app.toast(jjcad_core::i18n::tr("gui.canvas.verschneiden_zweites_objekt_anklicken"));
                }
                Some(erstes) => {
                    if run(app, "modify.intersect", json!({"ids": [erstes, id]})) {
                        app.session_mut().select(vec![erstes, id]);
                    }
                    // Werkzeug bleibt für das nächste Paar aktiv.
                    if let Some(t) = app.tool.as_mut() {
                        t.obj = None;
                    }
                }
            }
        }
        // Zerlegen ohne Auswahl (T0093): jeder Klick löst das Objekt unter dem
        // Zeiger auf; das Werkzeug bleibt für das nächste bereit. Welches
        // Objekt es trifft, zeigt die Vorschau vorher in der Warnfarbe.
        "modify.explode" => {
            let tol = (8.0 / app.current_view().zoom).max(0.5);
            if let Some(id) = snap::hit_test(app.doc(), w, tol, None) {
                let _ = run(app, "modify.explode", json!({"ids": [id]}));
            }
        }
        // Fläche schneiden (T0090): Punkte sammeln wie bei einer Polylinie,
        // Enter (oder ESC) führt den Schnitt aus.
        "modify.area.split" => {
            if tool
                .pts
                .last()
                .map(|q| q.dist(w) < 0.5)
                .unwrap_or(false)
            {
                return;
            }
            tool.pts.push(w);
            app.tool = Some(tool);
        }
        "block.insert" => {
            let Some(bid) = app.options.block_id.clone() else {
                app.toast(jjcad_core::i18n::tr("gui.canvas.block_im_inventar_waehlen_e"));
                return;
            };
            let _ = run(
                app,
                "block.insert",
                json!({"block": bid, "pos": ax(w), "rot": app.options.block_rot, "scale": app.options.block_scale}),
            );
        }
        "modify.quickselect" => {
            // T0060: das angeklickte Objekt bestimmt das Werkzeug, das ein
            // weiteres Objekt seiner Art erzeugt (`actions::erzeuger_fuer`).
            // Ein Klick ins Leere lässt die Schnellauswahl stehen.
            let tol = (8.0 / app.current_view().zoom).max(0.5);
            let Some(id) = snap::hit_test(app.doc(), w, tol, None) else {
                app.toast(jjcad_core::i18n::tr("gui.canvas.schnellauswahl_objekt_anklicken"));
                return;
            };
            let Some(kind) = app
                .doc()
                .ents()
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.kind.clone())
            else {
                return;
            };
            // Ein Block wird aus seiner eigenen Definition gesetzt — sonst
            // landete der im Inventar zuletzt gewählte Block am Zeiger.
            if let EntityKind::BlockRef { block, .. } = &kind {
                app.options.block_id = Some(block.clone());
            }
            app.start_tool(actions::erzeuger_fuer(&kind));
        }
        "modify.select" => {}
        _ => {}
    }
}

/// Wie [`run`], wählt aber das neu entstandene Objekt aus — damit sich seine
/// Eckpunkte sofort greifen lassen (Füllfläche, Volumen).
fn run_und_waehlen(app: &mut App, action: &str, params: serde_json::Value) -> bool {
    // Im **aktiven Raum** suchen: im Papierraum entsteht das Objekt in
    // `sheet.entities`, nicht in der Modellliste (T0109).
    let vorher: Vec<Id> = app.doc().ents().iter().map(|e| e.id).collect();
    let ok = run(app, action, params);
    if ok {
        if let Some(id) = app
            .doc()
            .ents()
            .iter()
            .find(|e| !vorher.contains(&e.id))
            .map(|e| e.id)
        {
            app.session_mut().select(vec![id]);
        }
    }
    ok
}

/// Legt diese Action ein **neues** Objekt an?
///
/// Nur für Erzeuger gelten die Vorgaben aus dem Eigenschaften-Panel (Ebene,
/// Farbe, Stärke, Linientyp) — bei Änderungsaktionen würden sie den Stil
/// fremder Objekte überschreiben. Die Bemaßung war hier lange vergessen: ihre
/// Erzeuger heißen `dim.*` und nicht `draw.*`, deshalb fiel jede neue Maßkette
/// auf die Standardebene „Bemaßung“ (rot) zurück, obwohl das Panel die
/// gewählte Ebene und Farbe anzeigte (T0062). `dim.style` ist **kein**
/// Erzeuger — das sind Maßketten-Einstellungen.
fn ist_erzeuger(action: &str) -> bool {
    action.starts_with("draw.")
        || matches!(
            action,
            "dim.linear"
                | "dim.chain"
                | "dim.elevation.plan"
                | "dim.elevation.section"
                | "dim.room"
        )
}

fn run(app: &mut App, action: &str, params: serde_json::Value) -> bool {
    // Aktuelle Zeichen-Eigenschaften (Farbe/Stärke/Typ) an neue Elemente weitergeben
    let mut params = params;
    if ist_erzeuger(action) {
        // Die im Layer-Panel gewählte Ebene gilt für Werkzeuge (der Nutzer
        // wählt sie selbst). Ohne diese Angabe legt die Befehls-Schicht das
        // Objekt auf die typgerechte Standardebene — richtig für MCP-Aufrufe
        // ohne `layer`, für ein Werkzeug aber eine stille Umstellung (T0039).
        if let Some(o) = params.as_object_mut() {
            if !o.contains_key("layer") {
                o.insert("layer".into(), serde_json::json!(app.doc().current_layer));
            }
        }
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
                    o.insert("linetype".into(), serde_json::json!(t.key()));
                }
            }
            // Dämmung: Breite (H) und Schlaufenform (r) gehören zur Linienart.
            if !o.contains_key("loop_width") {
                if let Some(b) = pend.loop_width {
                    // Länge in der Eingabeeinheit der Zeichnung — die
                    // Befehlsschicht rechnet über `Params::len` nach mm um.
                    o.insert(
                        "loop_width".into(),
                        serde_json::json!(app.unit().from_mm(b)),
                    );
                }
            }
            if !o.contains_key("loop_shape") {
                if let Some(f) = pend.loop_shape {
                    o.insert("loop_shape".into(), serde_json::json!(f));
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

    /// T0125: Ein neues Objekt erbt die Stärke seiner Ebene. Der frühere
    /// Vorgabewert 0,35 in `PendingStyle` wurde von `run` an **jede**
    /// Erzeuger-Action weitergereicht und überschrieb damit die
    /// Voreinstellungen vollständig.
    #[test]
    fn neues_objekt_erbt_die_ebenenstaerke() {
        let mut app = app();
        app.prefs.set_linienstaerke("Wände", 0.50);
        app.new_document();
        assert!(run(
            &mut app,
            "draw.wall",
            json!({"a": [0.0, 0.0], "b": [2.0, 0.0]})
        ));
        let id = *app.session().last_created.first().expect("Wand angelegt");
        let e = app.doc().entity(id).expect("Wand");
        assert_eq!(e.style.line_weight, None, "keine feste Stärke am Objekt");
        assert!(
            (app.doc().effective_line_weight(e) - 0.50).abs() < 1e-9,
            "Wand trägt {}, erwartet 0,50 aus der Ebene",
            app.doc().effective_line_weight(e)
        );
    }

    /// T0125: Ohne eigene Wahl trägt eine neue Zeichnung 0,18 mm — auch am
    /// erzeugten Objekt, nicht nur in der Ebene.
    #[test]
    fn neues_objekt_traegt_ohne_einstellung_018() {
        let mut app = app();
        app.prefs.linienstaerken_zuruecksetzen();
        app.new_document();
        assert!(run(
            &mut app,
            "draw.wall",
            json!({"a": [0.0, 0.0], "b": [2.0, 0.0]})
        ));
        let id = *app.session().last_created.first().expect("Wand angelegt");
        let e = app.doc().entity(id).expect("Wand");
        assert!(
            (app.doc().effective_line_weight(e) - 0.18).abs() < 1e-9,
            "Wand trägt {}",
            app.doc().effective_line_weight(e)
        );
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

    /// Zwei Punkte auf einer Waagerechten sind keine Diagonale — dann bleibt
    /// es beim Abbruch, der gesammelte Umriss bleibt stehen (T0131).
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

    /// Zwei Klicks als **Diagonale** legen eine rechteckige Fläche an (T0131):
    /// vier achsenparallele Ecken und die Fläche der Diagonale.
    #[test]
    fn zwei_klicks_als_diagonale_legen_ein_rechteck_an() {
        let mut app = app();
        app.start_tool("draw.fill.points");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(4000.0, 3000.0));
        finish_tool(&mut app, true);

        assert_eq!(app.doc().entities.len(), 1, "genau eine Fläche");
        let rand = ecken(&app);
        assert_eq!(rand.len(), 4, "vier Ecken: {rand:?}");
        for (got, want) in rand.iter().zip([
            V2::new(0.0, 0.0),
            V2::new(4000.0, 0.0),
            V2::new(4000.0, 3000.0),
            V2::new(0.0, 3000.0),
        ]) {
            assert!((*got - want).len() < 1e-6, "Ecke {got:?} statt {want:?}");
        }
        assert!(app.tool.is_none(), "Werkzeug nach dem Abschluss beendet");
    }

    /// Dieselbe Diagonale im Volumen-Werkzeug (T0131).
    #[test]
    fn zwei_klicks_als_diagonale_legen_ein_volumen_an() {
        let mut app = app();
        app.start_tool("draw.volume.points");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 1000.0));
        finish_tool(&mut app, true);

        let volumen = app
            .doc()
            .entities
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Volume { boundary, .. } => Some(boundary.clone()),
                _ => None,
            })
            .expect("Volumen angelegt");
        assert_eq!(volumen.len(), 4, "vier Ecken: {volumen:?}");
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
        app.options.fill_pattern = FillPattern::Lines;
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
        assert_eq!(pattern, FillPattern::Lines, "Muster aus dem Panel");
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

    /// T0100: Das Bildwerkzeug setzt nach zwei Klicks ein Bild mit genau
    /// diesen gegenüberliegenden Ecken — und bleibt für das nächste bereit.
    #[test]
    fn zwei_klicks_setzen_ein_bild() {
        let mut app = neue_app();
        // 2×2-PNG (rot, grün / blau, weiß), fest eingetragen.
        app.options.bild = Some(crate::app::BildQuelle {
            name: "Probe.png".into(),
            data: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4z8DAAMIM/4EAAB/uBfsL2WiLAAAAAElFTkSuQmCC".into(),
        });
        app.start_tool("draw.image");
        tool_click(&mut app, V2::new(1000.0, 500.0));
        assert!(
            app.doc().entities.is_empty(),
            "der erste Klick setzt noch nichts"
        );
        // Zweiter Klick = Ende der unteren Kante; das Bild ist quadratisch,
        // also 2000 mm hoch.
        tool_click(&mut app, V2::new(3000.0, 500.0));
        assert_eq!(app.doc().entities.len(), 1, "ein Bild angelegt");
        let e = app.doc().entities.last().unwrap();
        match &e.kind {
            EntityKind::Image {
                a,
                b,
                lock_ratio,
                name,
                ..
            } => {
                assert_eq!(*a, V2::new(1000.0, 500.0));
                assert_eq!(*b, V2::new(3000.0, 500.0));
                assert!(*lock_ratio, "Seitenverhältnis ist voreingestellt gesperrt");
                assert_eq!(name, "Probe.png");
            }
            k => panic!("kein Bild: {k:?}"),
        }
        let bb = jjcad_core::render::entity_bbox(app.doc(), e);
        assert_eq!(bb.max, V2::new(3000.0, 2500.0), "Höhe aus dem Verhältnis");
        assert_eq!(app.tool_id(), Some("draw.image"), "Werkzeug bleibt aktiv");
    }

    /// T0100: Ohne Sperre gilt die Höhe, die das Panel vorgibt — das Werkzeug
    /// reicht die Einstellung an die Action durch.
    #[test]
    fn bild_ohne_sperre_nimmt_die_hoehe_der_vorgaben() {
        let mut app = neue_app();
        app.options.bild = Some(crate::app::BildQuelle {
            name: "Probe.png".into(),
            data: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4z8DAAMIM/4EAAB/uBfsL2WiLAAAAAElFTkSuQmCC".into(),
        });
        app.options.bild_lock = false;
        app.start_tool("draw.image");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 0.0));
        let e = app.doc().entities.last().unwrap();
        match &e.kind {
            EntityKind::Image { lock_ratio, .. } => {
                assert!(!*lock_ratio, "Sperre aus der Vorgabe übernommen");
            }
            k => panic!("kein Bild: {k:?}"),
        }
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
mod tests_bogen_mittelpunkt {
    //! T0085: Bogen über Mittelpunkt, Startpunkt und Endpunkt.
    //!
    //! Der Startpunkt bestimmt den Radius, der Bogen läuft von dort im
    //! mathematisch positiven Drehsinn zum Endpunkt. Geprüft wird über den
    //! Klickpfad `tool_click` — denselben Weg, den die Maus geht — und am
    //! gespeicherten Objekt, nicht an der Vorschau.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    /// Bögen als (Mittelpunkt, Radius, Startwinkel, Endwinkel) in Grad.
    fn boegen(app: &App) -> Vec<(V2, f64, f64, f64)> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Arc { c, r, a0, a1 } => {
                    Some((*c, *r, a0.to_degrees(), a1.to_degrees()))
                }
                _ => None,
            })
            .collect()
    }

    /// Mittelpunkt, Startpunkt (Radius) und Endpunkt ergeben den Bogen; der
    /// Radius kommt aus dem Startpunkt, nicht aus dem Endpunkt.
    #[test]
    fn mittelpunkt_start_und_ende_ergeben_den_bogen() {
        let mut app = app();
        app.options.arc_mode = Bogenmodus::Mittelpunkt;
        app.start_tool("draw.arc");
        tool_click(&mut app, V2::new(0.0, 0.0)); // Mittelpunkt
        tool_click(&mut app, V2::new(1000.0, 0.0)); // Startpunkt = Radius
        assert!(boegen(&app).is_empty(), "ohne Endpunkt entsteht nichts");
        // Der Endpunkt liegt doppelt so weit weg — der Radius bleibt 1000.
        tool_click(&mut app, V2::new(0.0, 2000.0));

        let alle = boegen(&app);
        assert_eq!(alle.len(), 1, "Bogen angelegt");
        let (c, r, a0, a1) = alle[0];
        assert!(c.dist(V2::new(0.0, 0.0)) < 1e-6, "Mittelpunkt: {c:?}");
        assert!((r - 1000.0).abs() < 1e-6, "Radius aus dem Startpunkt: {r}");
        assert!(a0.abs() < 1e-6, "Startwinkel: {a0}");
        assert!((a1 - 90.0).abs() < 1e-6, "Endwinkel: {a1}");
        assert!(app.tool.is_none(), "Werkzeug ist nach dem Bogen beendet");
    }

    /// Ein zweiter Klick auf den Mittelpunkt legt keinen Radius fest — der
    /// nächste Klick zählt als Startpunkt.
    #[test]
    fn mittelpunkt_ohne_radius_behaelt_den_mittelpunkt() {
        let mut app = app();
        app.options.arc_mode = Bogenmodus::Mittelpunkt;
        app.start_tool("draw.arc");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 0.0));
        assert!(boegen(&app).is_empty(), "Radius null legt nichts an");
        assert_eq!(app.tool.as_ref().unwrap().pts.len(), 1, "Mittelpunkt bleibt");

        tool_click(&mut app, V2::new(500.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 500.0));
        assert_eq!(boegen(&app).len(), 1, "der nächste Klick setzt den Radius");
    }

    /// Ein Endpunkt auf dem Startpunkt ergibt keinen Bogen; die beiden
    /// gesetzten Punkte bleiben stehen.
    #[test]
    fn endpunkt_auf_dem_startpunkt_behaelt_die_punkte() {
        let mut app = app();
        app.options.arc_mode = Bogenmodus::Mittelpunkt;
        app.start_tool("draw.arc");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 0.0));
        assert!(boegen(&app).is_empty(), "gleicher Punkt legt nichts an");
        assert_eq!(app.tool.as_ref().unwrap().pts.len(), 2, "Punkte bleiben");

        tool_click(&mut app, V2::new(0.0, 1000.0));
        assert_eq!(boegen(&app).len(), 1, "der nächste Klick setzt das Ende");
    }

    /// Ohne Umschalten bleibt der bisherige Weg (Start, Ende, Bogenpunkt).
    #[test]
    fn drei_punkte_bleiben_die_vorgabe() {
        let mut app = app();
        assert_eq!(app.options.arc_mode, Bogenmodus::DreiPunkte);
        app.start_tool("draw.arc");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(1000.0, 1000.0));
        let alle = boegen(&app);
        assert_eq!(alle.len(), 1, "Bogen aus drei Punkten");
        let (c, r, _, _) = alle[0];
        assert!(c.dist(V2::new(1000.0, 0.0)) < 1e-6, "Mittelpunkt: {c:?}");
        assert!((r - 1000.0).abs() < 1e-6, "Radius: {r}");
    }

    /// Die Aufforderung nennt im neuen Modus die drei Schritte.
    #[test]
    fn aufforderung_nennt_mittelpunkt_start_und_ende() {
        let mut app = app();
        app.options.arc_mode = Bogenmodus::Mittelpunkt;
        app.start_tool("draw.arc");
        let hint = |app: &App| {
            let t = app.tool.as_ref().unwrap();
            crate::app::commands_hint(t, &app.options)
        };
        assert_eq!(hint(&app), "Bogen · Mittelpunkt klicken");
        tool_click(&mut app, V2::new(0.0, 0.0));
        assert_eq!(hint(&app), "Bogen · Startpunkt klicken");
        tool_click(&mut app, V2::new(1000.0, 0.0));
        assert_eq!(hint(&app), "Bogen · Endpunkt klicken");
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

    // ---- Wand beim Manipulieren (T0051) ----

    /// Wand (0;0)→(4000;0), Dicke 240 mm, ausgewählt; dazu eine freie Linie
    /// (0;3000)→(4000;3000) als fremdes Fangziel.
    fn wand_szenario() -> (App, Id) {
        let mut app = app();
        let s = app.session_mut();
        s.execute(
            "draw.wall",
            &serde_json::json!({"a": [0.0, 0.0], "b": [4000.0, 0.0], "thickness": 240.0}),
        )
        .unwrap();
        s.execute(
            "draw.line",
            &serde_json::json!({"a": [0.0, 3000.0], "b": [4000.0, 3000.0]}),
        )
        .unwrap();
        let wand = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Wall { .. }))
            .expect("Wand fehlt")
            .id;
        app.session_mut().select(vec![wand]);
        (app, wand)
    }

    /// Ecken des Wandkörpers (Stirn- und Längskanten) aus derselben Quelle wie
    /// die Anzeige.
    fn wand_ecken(app: &App, id: Id) -> Vec<V2> {
        let idx = render::WallIndex::build(app.doc(), app.doc().ents());
        let e = app.doc().entity(id).expect("Wand fehlt");
        render::wall_contour_segments_idx(&idx, e)
            .into_iter()
            .flat_map(|s| [s.start(), s.end()])
            .collect()
    }

    /// Ende `b` der Wand.
    fn wand_b(app: &App, id: Id) -> V2 {
        match &app.doc().entity(id).expect("Wand fehlt").kind {
            EntityKind::Wall { b, .. } => *b,
            other => panic!("keine Wand: {other:?}"),
        }
    }

    /// Die gezogene Wand fängt **nicht auf sich selbst**: keine ihrer Kanten
    /// (Stirnecken, Längskanten) zieht den Griff an. Vorher galt die Wand als
    /// „nur mit diskreten Punkten fangbar", und ihre mitwandernden Ecken
    /// rissen den Punkt an sich (T0051).
    #[test]
    fn wand_greift_beim_griffzug_nicht_auf_sich_selbst() {
        let (mut app, wand) = wand_szenario();
        let ecken = wand_ecken(&app, wand);
        assert_eq!(ecken.len(), 8, "vier Wandkanten mit je zwei Enden");
        app.begin_grip_drag(wand, crate::handles::GripKind::Vert(1));

        for ecke in ecken {
            let roh = ecke + V2::new(4.0, 4.0);
            let (p, label) = fang_zeiger(&app, roh, 40.0);
            assert_eq!(label, None, "eigene Wandecke {ecke:?} fängt: {p:?}");
            assert_punkt(p, roh);
        }
    }

    /// Der Fall aus dem Ticket: die Stirnecke liegt 120 mm (= halbe Wanddicke)
    /// neben dem Griff, die Toleranz ist etwas größer. Vorher zog der Fang den
    /// Punkt auf die Ecke, die dadurch aus dem Radius wanderte, worauf der
    /// Zeigerpunkt zurückkam — der Griff sprang in jedem Bild hin und her.
    /// Jetzt bleibt der Endpunkt bei jedem Bild am Zeiger.
    #[test]
    fn wand_griffzug_oszilliert_nicht_an_der_eigenen_ecke() {
        let (mut app, wand) = wand_szenario();
        app.begin_grip_drag(wand, crate::handles::GripKind::Vert(1));

        let roh = V2::new(4000.0, 100.0);
        let mut lagen = Vec::new();
        for _ in 0..6 {
            let (p, _) = fang_zeiger(&app, roh, 130.0);
            app.update_grip_drag(p);
            lagen.push(wand_b(&app, wand));
        }
        for (i, b) in lagen.iter().enumerate() {
            assert!(
                b.dist(roh) < 1e-9,
                "Bild {i}: Endpunkt {b:?} springt vom Zeiger {roh:?} weg — Lagen {lagen:?}"
            );
        }
    }

    /// Voller Mauszug durch die Oberfläche: das Wandende folgt dem Zeiger und
    /// landet nicht auf der eigenen Stirnecke; der gespeicherte Stand trägt die
    /// Zeigerlage.
    #[test]
    fn mauszug_bewegt_das_wandende_ans_zeigerende() {
        let (mut app, wand) = wand_szenario();
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();

        let griff = world_to_screen(rect, view, V2::new(4000.0, 0.0));
        let ziel = world_to_screen(rect, view, V2::new(4000.0, 100.0));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");

        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        assert!(
            wand_b(&app, wand).dist(V2::new(4000.0, 100.0)) < 2.0,
            "Endpunkt {:?} folgt dem Zeiger",
            wand_b(&app, wand)
        );
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        match &wieder.entity(wand).expect("Wand in der Datei").kind {
            EntityKind::Wall { b, .. } => assert!(
                b.dist(V2::new(4000.0, 100.0)) < 2.0,
                "gespeichertes Ende {b:?}"
            ),
            other => panic!("keine Wand: {other:?}"),
        }
    }

    /// Ansichtsfenster im Papierraum (T0109): Klick wählt es, Ziehen
    /// verschiebt es bei eingeschaltetem „Maus verschiebt" — wie jedes andere
    /// Objekt. Vorher ging das nur mit dem Werkzeug „Verschieben".
    #[test]
    fn ansichtsfenster_waehlt_und_zieht_wie_ein_objekt() {
        let mut app = app();
        app.prefs.mouse_move = true;
        let sid = app.doc().sheets[0].id;
        app.switch_tab(TabKind::Sheet(sid));
        frame(&mut app, vec![]);
        let vid = app
            .doc()
            .sheets[0]
            .viewports()
            .next()
            .map(|(e, _)| e.id)
            .expect("Ansichtsfenster");
        let vp = app.doc().sheets[0].viewport(vid).expect("Fenster").clone();
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let mitte = world_to_screen(rect, view, vp.rect().center());

        // Klick in die Fläche wählt das Fenster.
        frame(&mut app, vec![egui::Event::PointerMoved(mitte)]);
        frame(&mut app, vec![druck(mitte)]);
        frame(&mut app, vec![loslassen(mitte)]);
        assert_eq!(app.selection(), &[vid], "Klick muss das Fenster wählen");

        // Ziehen verschiebt es (Maus verschiebt = an).
        let ziel = mitte + egui::vec2(40.0, 20.0);
        frame(&mut app, vec![egui::Event::PointerMoved(mitte)]);
        frame(&mut app, vec![druck(mitte)]);
        assert!(app.move_drag.is_some(), "Zug muss beginnen");
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        let nachher = app.doc().sheets[0].viewport(vid).expect("Fenster").clone();
        // Bildschirm +y zeigt nach unten, die Zeichnung rechnet nach oben.
        assert!(
            nachher.x > vp.x + 1.0 && nachher.y < vp.y - 1.0,
            "Fenster muss dem Zeiger folgen: vorher {:?}, nachher {:?}",
            (vp.x, vp.y),
            (nachher.x, nachher.y)
        );
        // Größe bleibt beim Verschieben.
        assert!((nachher.w - vp.w).abs() < 1e-6 && (nachher.h - vp.h).abs() < 1e-6);
    }

    /// Werkzeug „Ansichtsfenster": zwei Klicks spannen das Rechteck auf, das
    /// neue Fenster ist danach ausgewählt (T0109).
    #[test]
    fn ansichtsfenster_aufziehen_legt_ein_ausgewaehltes_objekt_an() {
        let mut app = app();
        let sid = app.doc().sheets[0].id;
        app.switch_tab(TabKind::Sheet(sid));
        frame(&mut app, vec![]);
        let vorher = app.doc().sheets[0].viewports().count();
        app.start_tool("sheet.viewport_tool");
        tool_click_at(&mut app, V2::new(30.0, 100.0), V2::new(30.0, 100.0));
        tool_click_at(&mut app, V2::new(200.0, 200.0), V2::new(200.0, 200.0));
        assert!(app.tool.is_none(), "zweiter Klick beendet das Werkzeug");
        let fenster: Vec<_> = app.doc().sheets[0].viewports().collect();
        assert_eq!(fenster.len(), vorher + 1, "ein Fenster mehr");
        // Das neue Fenster liegt als Hintergrund vorn in der Liste.
        let (e, vp) = fenster
            .iter()
            .find(|(_, v)| (v.x, v.y, v.w, v.h) == (30.0, 100.0, 170.0, 100.0))
            .expect("neues Fenster");
        assert_eq!(app.selection(), &[e.id], "neues Fenster ist ausgewählt");
    }

    /// Gegenprobe: fremde Punkte fangen beim Wand-Griffzug weiterhin.
    #[test]
    fn wand_griffzug_faengt_weiterhin_fremde_punkte() {
        let (mut app, wand) = wand_szenario();
        app.begin_grip_drag(wand, crate::handles::GripKind::Vert(1));

        let (p, label) = fang_zeiger(&app, V2::new(3990.0, 3008.0), 60.0);
        assert_eq!(label, Some("Endpunkt"));
        assert_punkt(p, V2::new(4000.0, 3000.0));
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

    // ---- Spline im Griffzug (T0134) ----

    /// Spline (0;0) → (1000;600) → (2000;0) → (3000;600), ausgewählt.
    fn szenario_spline() -> (App, Id) {
        let mut app = app();
        app.session_mut()
            .execute(
                "draw.spline",
                &serde_json::json!({"pts": [[0.0, 0.0], [1000.0, 600.0],
                                            [2000.0, 0.0], [3000.0, 600.0]]}),
            )
            .expect("Spline");
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        (app, id)
    }

    fn spline_stuetzpunkte() -> Vec<V2> {
        vec![
            V2::new(0.0, 0.0),
            V2::new(1000.0, 600.0),
            V2::new(2000.0, 0.0),
            V2::new(3000.0, 600.0),
        ]
    }

    /// Ein Punkt der Kurvenzerlegung, der **kein** Stützpunkt ist.
    fn spline_zwischenpunkt() -> V2 {
        let stuetz = spline_stuetzpunkte();
        let kurve = Spline::new(stuetz.clone(), false).flatten(SPLINE_TOL_MM);
        kurve
            .iter()
            .copied()
            .find(|p| stuetz.iter().all(|q| p.dist(*q) > 1.0))
            .expect("die Zerlegung hat Zwischenpunkte")
    }

    /// Die Zerlegung des Splines in kurze Teilstrecken darf beim Ziehen an
    /// einem Stützpunkt nicht fangen: der Punkt wandert mit der Kurve, der Fang
    /// zöge den Zeiger auf eine Stelle, die im nächsten Bild woanders liegt —
    /// die Linie flackert und zittert (T0134). Fangbar bleiben die Stützpunkte.
    #[test]
    fn spline_griffzug_faengt_nicht_an_der_eigenen_kurve() {
        let (mut app, id) = szenario_spline();
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(0));
        let zoom = app.current_view().zoom.max(1e-6);
        let tol = 12.0 / zoom;

        let zwischen = spline_zwischenpunkt();
        let (p, label) = fang_zeiger(&app, zwischen, tol);
        assert_eq!(label, None, "Kurvenpunkt darf nicht fangen: {p:?}");
        assert_punkt(p, zwischen);

        // Auch nach mehreren Bildern bleibt es dabei (kein Aufschaukeln).
        app.update_grip_drag(p);
        for _ in 0..3 {
            let (q, label) = fang_zeiger(&app, zwischen, tol);
            assert_eq!(label, None, "der Fang kehrt im nächsten Bild zurück: {q:?}");
            assert_punkt(q, zwischen);
            app.update_grip_drag(q);
        }

        // Gegenprobe: der ruhende Stützpunkt (1000;600) fängt weiterhin.
        let (q, label) = fang_zeiger(&app, V2::new(1006.0, 603.0), tol);
        assert_eq!(label, Some("Endpunkt"), "Stützpunkt muss fangen");
        assert_punkt(q, V2::new(1000.0, 600.0));
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

    // ---- Winkelfang (Ortho/Polar) beim Griffzug (T0046) ----

    /// App mit eingeschalteter Zwangsrichtung; Ziehen als Gedrückthalten und
    /// ohne Objektfang, damit nur Ortho/Polar den Punkt bestimmt.
    fn app_mit_zwang(ortho: bool, polar: bool) -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.ui.drag_mode = DragMode::HoldAndDrag;
        app.session_mut()
            .execute(
                "doc.settings",
                &serde_json::json!({"input_unit": "mm",
                                   "snap": {"ortho": ortho, "polar": polar,
                                            "grid": false, "endpoint": false,
                                            "midpoint": false, "center": false,
                                            "intersection": false,
                                            "perpendicular": false,
                                            "nearest": false, "wall_edges": false}}),
            )
            .unwrap();
        app
    }

    /// Linie (0;0) → (2000;0), ausgewählt.
    fn szenario_linie(ortho: bool, polar: bool) -> (App, Id) {
        let mut app = app_mit_zwang(ortho, polar);
        app.session_mut()
            .execute(
                "draw.line",
                &serde_json::json!({"a": [0.0, 0.0], "b": [2000.0, 0.0]}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        (app, id)
    }

    /// Ortho hält das gezogene Linienende auf der Achse des anderen Endes.
    /// Beim Erstellen galt das schon, beim Manipulieren fehlte der Bezug (T0046).
    #[test]
    fn griffzug_haelt_die_linie_in_der_zwangsachse() {
        let (mut app, id) = szenario_linie(true, false);
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(1));

        let (p, _) = fang_zeiger(&app, V2::new(2500.0, 300.0), 40.0);
        assert_punkt(p, V2::new(2500.0, 0.0));
    }

    /// Polar rastet den Griff auf 45° ein, ohne die Länge zu verändern.
    #[test]
    fn griffzug_rastet_auf_dem_polarwinkel_ein() {
        let (mut app, id) = szenario_linie(false, true);
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(1));

        let roh = V2::new(2500.0, 2400.0);
        let (p, label) = fang_zeiger(&app, roh, 40.0);
        assert_eq!(label, Some("POL"), "Statuszeile zeigt den Polarfang");
        assert!((p.x - p.y).abs() < 1e-6, "nicht auf 45°: {p:?}");
        assert!(
            (p.dist(V2::ZERO) - roh.dist(V2::ZERO)).abs() < 1e-6,
            "Länge bleibt: {p:?}"
        );
    }

    /// Polylinien-Ecke: die Kante zum vorigen Stützpunkt bleibt senkrecht.
    #[test]
    fn griffzug_der_polylinie_haelt_die_kante() {
        let mut app = app_mit_zwang(true, false);
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [2000.0, 1500.0]]}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(2));

        let (p, _) = fang_zeiger(&app, V2::new(1900.0, 1400.0), 40.0);
        assert_punkt(p, V2::new(2000.0, 1400.0));
    }

    /// Auch die Maus-Verschiebung eines Objekts läuft über die Zwangsachse:
    /// „Manipulieren" schließt das Verschieben mit ein (T0046).
    #[test]
    fn verschieben_mit_der_maus_haelt_die_zwangsachse() {
        let (mut app, _) = szenario_linie(true, false);
        app.begin_move_drag(V2::ZERO, 0.0);
        assert!(app.move_drag.is_some(), "Verschiebung muss laufen");

        let (p, _) = fang_zeiger(&app, V2::new(300.0, 1900.0), 40.0);
        assert_punkt(p, V2::new(0.0, 1900.0));
    }

    /// Polylinien-Ecke mit **zwei** Linien: beide Nachbarn stehen als Achsen
    /// zur Verfügung, die Ecke rastet auf ihren Schnittpunkt — beide Linien
    /// sind danach ausgerichtet (T0046, Turn 3).
    #[test]
    fn griffzug_richtet_beide_polylinienlinien_aus() {
        let mut app = app_mit_zwang(true, false);
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 1500.0], [2000.0, 4000.0]]}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(1));

        // 20 mm neben der Senkrechten des nächsten, 50 mm über der
        // Waagerechten des vorigen Stützpunkts.
        let (p, _) = fang_zeiger(&app, V2::new(1980.0, 50.0), 100.0);
        assert_punkt(p, V2::new(2000.0, 0.0));
    }

    /// Polar nutzt **beide** Winkelkränze des Griffs (T0054): der Zeiger steht
    /// näher am nächsten Stützpunkt, dort liegt aber keine Richtung im Fenster
    /// — die Waagerechte des weiter entfernten Bezugs muss trotzdem fangen.
    /// Vorher rastete nur der Kranz des näheren Nachbarn.
    #[test]
    fn griffzug_rastet_auf_den_polarwinkel_des_ferneren_nachbarn() {
        let mut app = app_mit_zwang(false, true);
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 1500.0], [2500.0, 0.0]]}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(1));

        // 200 mm über dem nächsten Stützpunkt (2500;0), 2600 mm vom vorigen:
        // vom nächsten aus 63° (kein Fenster), vom vorigen aus 4,4°.
        let roh = V2::new(2600.0, 200.0);
        let (p, label) = fang_zeiger(&app, roh, 40.0);
        assert_eq!(label, Some("POL"), "Statuszeile zeigt den Polarfang");
        assert!(p.y.abs() < 1e-6, "nicht auf der Waagerechten: {p:?}");
        assert!(
            (p.dist(V2::ZERO) - roh.dist(V2::ZERO)).abs() < 1e-6,
            "Länge bleibt: {p:?}"
        );
    }

    /// Die Kreuzung der beiden Winkelkränze ist ein Punktziel (T0054): der
    /// Griff landet genau im Schnittpunkt, und so steht er in der Datei.
    #[test]
    fn mauszug_faengt_die_kreuzung_der_polarwinkel_und_speichert() {
        let mut app = app_mit_zwang(false, true);
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [1500.0, 1500.0], [4000.0, 0.0]]}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);

        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let tol = fang_radius_px(&app, view.zoom);
        let griff = world_to_screen(rect, view, V2::new(1500.0, 1500.0));
        // 45° des vorigen (0;0) und 135° des nächsten (4000;0) kreuzen sich in
        // (2000;2000); der Zeiger steht knapp daneben.
        let kreuzung = V2::new(2000.0, 2000.0);
        let ziel = world_to_screen(
            rect,
            view,
            V2::new(kreuzung.x + tol * 0.6, kreuzung.y + tol * 0.6),
        );

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        let geladen = wieder
            .entity(id)
            .and_then(|e| match &e.kind {
                EntityKind::Polyline { verts, .. } => Some(verts[1].p),
                _ => None,
            })
            .expect("Polylinie in der Datei");
        assert_punkt(geladen, kreuzung);
    }

    /// Derselbe Fall durch die Oberfläche: die ausgerichtete Ecke steht so in
    /// der gespeicherten Datei.
    #[test]
    fn mauszug_richtet_beide_linien_aus_und_speichert() {
        let mut app = app_mit_zwang(true, false);
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 1500.0], [2000.0, 4000.0]]}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);

        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let tol = fang_radius_px(&app, view.zoom);
        let griff = world_to_screen(rect, view, V2::new(2000.0, 1500.0));
        let ziel = world_to_screen(rect, view, V2::new(2000.0 - tol * 0.3, tol * 0.3));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let pts = match &app.doc().entity(id).expect("Polylinie fehlt").kind {
            EntityKind::Polyline { verts, .. } => verts.iter().map(|v| v.p).collect::<Vec<_>>(),
            other => panic!("keine Polylinie: {other:?}"),
        };
        assert_punkt(pts[1], V2::new(2000.0, 0.0));

        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        let geladen = wieder
            .entity(id)
            .and_then(|e| match &e.kind {
                EntityKind::Polyline { verts, .. } => Some(verts[1].p),
                _ => None,
            })
            .expect("Polylinie in der Datei");
        assert_punkt(geladen, V2::new(2000.0, 0.0));
    }

    /// Eine Ecke zwischen zwei Linien: der **Schnittpunkt** beider Achsen ist
    /// auch dann zu fangen, wenn keine der Koordinaten allein im Fangradius
    /// liegt — beide angrenzenden Linien stehen danach gerade (T0046, Turn 4).
    #[test]
    fn griffzug_faengt_den_schnittpunkt_beider_nachbarn() {
        let mut app = app_mit_zwang(true, false);
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0],
                                              [1500.0, 4500.0], [0.0, 3000.0]], "closed": true}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);
        // Ecke (0;3000), Nachbarn (1500;4500) und (0;0) — Schnittpunkt (1500;0).
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(4));

        // 110 mm neben dem Schnittpunkt in beiden Koordinaten (Radius 81).
        let (p, _) = fang_zeiger(&app, V2::new(1610.0, 110.0), 81.0);
        assert_punkt(p, V2::new(1500.0, 0.0));
    }

    /// Dasselbe durch die Oberfläche: die Ecke landet im Schnittpunkt der
    /// beiden Nachbarachsen, und so steht sie in der gespeicherten Datei.
    #[test]
    fn mauszug_faengt_den_schnittpunkt_und_speichert_ihn() {
        let mut app = app_mit_zwang(true, false);
        app.session_mut()
            .execute(
                "draw.polyline",
                &serde_json::json!({"pts": [[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0],
                                              [1500.0, 4500.0], [0.0, 3000.0]], "closed": true}),
            )
            .unwrap();
        let id = app.doc().entities[0].id;
        app.session_mut().select(vec![id]);

        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let tol = fang_radius_px(&app, view.zoom);
        let griff = world_to_screen(rect, view, V2::new(0.0, 3000.0));
        // 1,35 × Radius neben dem Schnittpunkt in beiden Koordinaten: keine
        // Einzelachse greift, der Schnittpunkt (Abstand 1,9 × Radius) schon.
        let ziel = world_to_screen(
            rect,
            view,
            V2::new(1500.0 + tol * 1.35, 0.0 + tol * 1.35),
        );

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        let pts = match &app.doc().entity(id).expect("Polylinie fehlt").kind {
            EntityKind::Polyline { verts, .. } => verts.iter().map(|v| v.p).collect::<Vec<_>>(),
            other => panic!("keine Polylinie: {other:?}"),
        };
        assert_punkt(pts[4], V2::new(1500.0, 0.0));
        // Beide angrenzenden Linien stehen gerade: (1500;0)→(1500;4500)
        // senkrecht, (1500;0)→(0;0) waagerecht.
        assert!((pts[3].x - pts[4].x).abs() < 1e-9, "Kante zum Vorgänger senkrecht");
        assert!((pts[0].y - pts[4].y).abs() < 1e-9, "Kante zum Nachfolger waagerecht");

        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        let geladen = wieder
            .entity(id)
            .and_then(|e| match &e.kind {
                EntityKind::Polyline { verts, .. } => Some(verts[4].p),
                _ => None,
            })
            .expect("Polylinie in der Datei");
        assert_punkt(geladen, V2::new(1500.0, 0.0));
    }

    /// Gegenprobe: ohne Ortho und Polar folgt der Griff weiter dem Zeiger —
    /// der Winkelfang ist eine Führung, kein Zwang (T0046).
    #[test]
    fn ohne_zwang_folgt_der_griff_dem_zeiger() {
        let (mut app, id) = szenario_linie(false, false);
        app.begin_grip_drag(id, crate::handles::GripKind::Vert(1));

        let (p, label) = fang_zeiger(&app, V2::new(2500.0, 300.0), 40.0);
        assert_eq!(label, None);
        assert_punkt(p, V2::new(2500.0, 300.0));
    }

    /// Voller Mauszug durch die Oberfläche: der Endpunkt landet auf der
    /// Zwangsachse, und genau diese Lage steht in der gespeicherten Datei.
    #[test]
    fn mauszug_speichert_die_ausgerichtete_lage() {
        let (mut app, id) = szenario_linie(true, false);
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();

        let griff = world_to_screen(rect, view, V2::new(0.0, 0.0));
        let ziel = world_to_screen(rect, view, V2::new(300.0, 1900.0));

        frame(&mut app, vec![egui::Event::PointerMoved(griff)]);
        frame(&mut app, vec![druck(griff)]);
        assert!(app.grip_drag.is_some(), "Griffzug muss beginnen");

        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        frame(&mut app, vec![loslassen(ziel)]);
        assert!(app.grip_drag.is_none(), "Loslassen beendet den Zug");

        // Senkrecht unter dem festen Ende: gleiche x-Lage, y = 1900.
        let a = match &app.doc().entity(id).expect("Linie fehlt").kind {
            EntityKind::Line { a, .. } => *a,
            other => panic!("keine Linie: {other:?}"),
        };
        assert_punkt(a, V2::new(2000.0, 1900.0));

        let json = jjcad_core::persist::to_json(app.doc()).expect("speichern");
        let wieder = jjcad_core::persist::from_json(&json).expect("laden");
        let geladen = wieder
            .entity(id)
            .and_then(|e| match &e.kind {
                EntityKind::Line { a, .. } => Some(*a),
                _ => None,
            })
            .expect("Linie in der Datei");
        assert_punkt(geladen, V2::new(2000.0, 1900.0));
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
            block: false,
        };
        let blatt = DrawCacheKey {
            rev: 7,
            zoom: 0.08,
            annot: 100.0,
            space: Some(3),
            block: false,
        };
        // T0137: Der Blockeditor hat keinen `space` — ohne eigenes Kennzeichen
        // gälte seine Liste als die des Modells.
        let block = DrawCacheKey {
            rev: 7,
            zoom: 0.08,
            annot: 100.0,
            space: None,
            block: true,
        };
        assert!(!modell.matches(&blatt), "Papierraum gilt sonst als Modell");
        assert!(!blatt.matches(&modell));
        assert!(!modell.matches(&block), "Blockeditor gilt sonst als Modell");
        assert!(!block.matches(&modell));
        assert!(modell.matches(&modell));
        assert!(block.matches(&block));
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
        let vorschau = text_preview_prim(
            "Hallo".to_string(),
            250.0,
            TextAlignH::Left,
            TextAlignV::Middle,
        );

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
        let t = text_preview_prim(
            "Hallo".to_string(),
            250.0,
            TextAlignH::Left,
            TextAlignV::Middle,
        );
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
            let prim = text_preview_prim(
                "Hallo".to_string(),
                250.0,
                TextAlignH::Left,
                TextAlignV::Middle,
            );
            let w2s = |q: V2| Pos2::new(q.x as f32, q.y as f32);
            ghost_shape(
                &p,
                &PShape::Text(prim),
                &w2s,
                1.0,
                Stroke::new(1.5, Palette::accent()),
                0.0,
                crate::textgroesse::RASTER_DECKEL_PX,
            );
        });
        assert!(
            !out.shapes.is_empty(),
            "der Text fehlt in der Einfüge-Vorschau"
        );
    }

    /// Alle gezeichneten Texte einer Vorschau (galley-Text, nicht die
    /// Zeichnung) — so lässt sich prüfen, **was** die Vorschau schreibt.
    fn vorschau_texte(app: &mut App) -> Vec<String> {
        fn sammle(s: &egui::Shape, texte: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => texte.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| sammle(s, texte)),
                _ => {}
            }
        }
        let ctx = app.ctx.clone();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("vorschau"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
            let view = View {
                center: V2::new(1000.0, 500.0),
                zoom: 0.2,
                rot: 0.0,
            };
            let cursor = V2::new(1200.0, 600.0);
            draw_tool_preview(app, &p, rect, view, Some(cursor), Some(cursor));
        });
        let mut texte = Vec::new();
        for cs in &out.shapes {
            sammle(&cs.shape, &mut texte);
        }
        texte
    }

    /// Die Vorschau des Textwerkzeugs zeigt **immer** das Beispielwort „Text".
    ///
    /// Vorher merkte sich `ToolOptions` den zuletzt gesetzten Inhalt und die
    /// Vorschau zeichnete ihn — jedes neue Textfeld kündigte sich mit dem alten
    /// Text an, obwohl es leer beginnt (T0066).
    #[test]
    fn vorschau_zeigt_nach_einem_text_immer_das_beispielwort() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "mm" }))
            .expect("Einheit setzen");

        // Wie im Betrieb: Textwerkzeug, tippen, Esc übernimmt.
        crate::textedit::start(&mut app, V2::new(1000.0, 500.0));
        app.ui
            .texteingabe
            .as_mut()
            .expect("Eingabe läuft")
            .text = "Wohnen".to_string();
        crate::textedit::uebernehmen_fuer_test(&mut app);
        assert!(
            app.doc().ents().iter().any(|e| matches!(
                &e.kind,
                EntityKind::Text { text, .. } if text == "Wohnen"
            )),
            "der Text muss erst gesetzt sein"
        );

        app.start_tool("draw.text");
        let texte = vorschau_texte(&mut app);
        assert!(
            texte.iter().any(|t| t == TEXT_VORSCHAU),
            "Vorschau zeigt nicht „{TEXT_VORSCHAU}\": {texte:?}"
        );
        assert!(
            !texte.iter().any(|t| t.contains("Wohnen")),
            "der zuletzt gesetzte Inhalt darf nicht in der Vorschau stehen: {texte:?}"
        );
    }
}

#[cfg(test)]
mod tests_textausrichtung {
    //! Ausrichtung des **gemalten** Textes (T0028).
    //!
    //! `paint_text` richtete doppelt aus: `LayoutJob::halign` legt die Zeilen
    //! schon um den Ursprung (links ab 0, zentriert um 0, rechts endend bei 0),
    //! danach zog der Code noch einmal `-Breite/2` bzw. `-Breite` ab. Ein
    //! mittiger Text stand dadurch eine halbe Textbreite zu weit links, ein
    //! rechtsbündiger um eine ganze — der Auswahlrahmen (`render::text_corners`)
    //! saß richtig, nur die Schrift nicht.
    use super::*;

    const BASIS: Pos2 = Pos2::new(300.0, 200.0);

    /// Bildschirmkasten des gemalten Textes: Galley an ihrem Ort plus den
    /// senkrechten Versatz, den `paint_text` anwendet.
    fn gemalt(align: TextAlignH, valign: TextAlignV) -> Rect {
        let ctx = egui::Context::default();
        let mut ergebnis = Rect::NOTHING;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("text"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            let t = render::TextPrim {
                pos: V2::ZERO,
                text: "Hallo\nWelt".to_string(),
                height: 250.0,
                angle: 0.0,
                align,
                valign,
                bold: false,
                italic: false,
                underline: false,
                superscript: false,
                runs: Vec::new(),
                font: jjcad_core::model::TextFont::default(),
                line_spacing: 1.0,
            };
            let g = text_galley(&p, &t, 1.0, Color32::WHITE, crate::textgroesse::RASTER_DECKEL_PX);
            let (pos, _) = text_ort(&t, &g, BASIS, 0.0);
            ergebnis = g.rect.translate(pos.to_vec2());
        });
        ergebnis
    }

    fn nahe(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    /// Alle neun Fälle: die Kante, die die Ausrichtung benennt, liegt auf dem
    /// Anker — die Mitte bei „Zentriert/Mitte", die Kante bei den übrigen.
    #[test]
    fn gemalter_text_haengt_an_seinem_anker() {
        for h in TextAlignH::ALL {
            for v in TextAlignV::ALL {
                let r = gemalt(h, v);
                let x = match h {
                    TextAlignH::Left => r.left(),
                    TextAlignH::Center => r.center().x,
                    TextAlignH::Right => r.right(),
                };
                assert!(
                    nahe(x, BASIS.x),
                    "{}/{}: waagerecht bei {x}, erwartet {}",
                    h.label(),
                    v.label(),
                    BASIS.x
                );
                match v {
                    TextAlignV::Top => assert!(nahe(r.top(), BASIS.y), "Oben an der oberen Kante"),
                    TextAlignV::Middle => {
                        assert!(nahe(r.center().y, BASIS.y), "Mitte in der Mitte")
                    }
                    TextAlignV::Bottom => {
                        assert!(nahe(r.bottom(), BASIS.y), "Unten an der unteren Kante")
                    }
                }
            }
        }
    }

    /// Der Kasten des gemalten Textes bleibt so hoch wie der Rahmen — die
    /// senkrechte Ausrichtung verschiebt nur, sie verändert die Höhe nicht.
    #[test]
    fn senkrechte_ausrichtung_verschiebt_nur() {
        let oben = gemalt(TextAlignH::Left, TextAlignV::Top);
        let mitte = gemalt(TextAlignH::Left, TextAlignV::Middle);
        let unten = gemalt(TextAlignH::Left, TextAlignV::Bottom);
        assert!(nahe(oben.height(), mitte.height()) && nahe(mitte.height(), unten.height()));
        // Unten steht die Schrift über dem Anker, Oben darunter.
        assert!(unten.top() < oben.top() - 1.0);
        assert!(nahe(unten.top(), oben.top() - oben.height()));
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

    /// Werkzeug läuft, Ortho an, erster Punkt gesetzt (T0042): der Zeiger neben
    /// der Senkrechten des Haltepunkts landet auf dem Schnittpunkt aus
    /// Senkrechter und Zwangsachse — beide Fänge greifen zugleich. Ohne
    /// Haltepunkt zieht allein Ortho den Zeiger auf die Achse (Gegenprobe).
    #[test]
    fn ortho_und_hilfslinie_fangen_in_der_app() {
        let mut app = app();
        let id = linie(&mut app);
        app.session_mut()
            .execute(
                "doc.settings",
                &serde_json::json!({"snap": {"ortho": true, "polar": false}}),
            )
            .unwrap();
        app.start_tool("draw.line");
        app.tool
            .as_mut()
            .unwrap()
            .pts
            .push(V2::new(0.0, -2000.0));
        app.halt.push(halt_am_ende(&app, id));
        // 4 mm neben der Senkrechten x = 2000, 1000 mm über der Achse y = -2000.
        let (p, label) = fang_zeiger(&app, V2::new(2004.0, -1000.0), 30.0);
        assert_punkt(p, V2::new(2000.0, -2000.0), "Schnittpunkt");
        assert_eq!(label, Some("Hilfslinie"));

        app.halt.clear();
        let (p2, _) = fang_zeiger(&app, V2::new(2004.0, -1000.0), 30.0);
        assert_punkt(p2, V2::new(2004.0, -2000.0), "nur Zwangsachse");
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

    /// Zwei Linien: a (0;0) → (2000;0) wird gezogen, b endet bei (2000;1500).
    fn szenario_zwei_linien() -> (App, Id) {
        let mut app = app();
        let s = app.session_mut();
        s.execute(
            "draw.line",
            &serde_json::json!({"a": [0.0, 0.0], "b": [2000.0, 0.0]}),
        )
        .unwrap();
        s.execute(
            "draw.line",
            &serde_json::json!({"a": [0.0, 1500.0], "b": [2000.0, 1500.0]}),
        )
        .unwrap();
        let a = app.doc().entities[0].id;
        app.session_mut().select(vec![a]);
        (app, a)
    }

    // ---- Erweiterter Punktfang beim Manipulieren (T0046, Turn 2) ----

    /// Beim Ziehen an einem Griff muss der erweiterte Punktfang wieder greifen:
    /// vorher räumte die Zeichenfläche die Haltepunkte in jedem Bild ab, weil
    /// sie nur ein laufendes Werkzeug kannte.
    #[test]
    fn griffzug_haelt_einen_fremden_punkt() {
        let (mut app, a) = szenario_zwei_linien();
        frame(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let ende_b = V2::new(2000.0, 1500.0);
        let ziel = world_to_screen(rect, view, ende_b);
        assert!(rect.contains(ziel), "Zielfläche {rect:?}, Punkt {ziel:?}");

        app.begin_grip_drag(a, crate::handles::GripKind::Vert(1));
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        assert!(app.halt.is_empty(), "nach einem Frame noch nicht gehalten");

        std::thread::sleep(VERWEILDAUER + std::time::Duration::from_millis(120));
        frame(&mut app, vec![egui::Event::PointerMoved(ziel)]);
        assert_eq!(
            app.halt.len(),
            1,
            "Verweilen hält das fremde Ende: {:?}",
            app.halt
        );
        assert_punkt(app.halt[0].point, ende_b, "Haltepunkt");

        // Zeiger auf die Senkrechte dieses Punktes: der gezogene Griff fängt.
        let tol = fang_radius_px(&app, view.zoom);
        let daneben = world_to_screen(rect, view, V2::new(2000.0 + tol * 0.3, 900.0));
        frame(&mut app, vec![egui::Event::PointerMoved(daneben)]);
        assert_eq!(app.snap_label.as_deref(), Some("Hilfslinie"));
    }

    /// Auch das Verschieben mit der Maus ist „Manipulieren": die Hilfslinien
    /// bleiben stehen.
    #[test]
    fn verschieben_behaelt_die_hilfslinien() {
        let (mut app, a) = szenario_zwei_linien();
        app.halt.push(halt_am_ende(&app, a));
        app.begin_move_drag(V2::ZERO, 0.0);
        assert!(app.move_drag.is_some(), "Verschiebung muss laufen");
        frame(&mut app, vec![]);
        assert_eq!(
            app.halt.len(),
            1,
            "Haltepunkte bleiben beim Verschieben stehen"
        );
    }

    /// Gegenprobe: ohne Werkzeug und ohne laufenden Zug gibt es keine
    /// Hilfslinien — der Fang gehört zum Punktesetzen, nicht zum Ruhezustand.
    #[test]
    fn ohne_bearbeitung_keine_hilfslinien() {
        let (mut app, a) = szenario_zwei_linien();
        app.halt.push(halt_am_ende(&app, a));
        frame(&mut app, vec![]);
        assert!(app.halt.is_empty(), "ohne Werkzeug und Zug keine Hilfslinien");
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

#[cfg(test)]
mod tests_treppenmass {
    //! Die Maßangabe am Zeiger ist beim Ziehen dieselbe wie beim Erstellen
    //! (T0027, Turn 4) — eine Quelle für beide (`stair_mass_text`).

    use super::*;

    #[test]
    fn massangabe_in_meter() {
        assert_eq!(
            stair_mass_text(Unit::M, 3000.0, 1000.0, 13),
            "3,00 × 1,00 · 13 Stg"
        );
    }

    #[test]
    fn massangabe_in_millimeter() {
        assert_eq!(
            stair_mass_text(Unit::Mm, 3000.0, 1000.0, 13),
            "3.000 × 1.000 · 13 Stg"
        );
    }
}

#[cfg(test)]
mod tests_werkzeug_zeiger {
    //! Werkzeug-Icon am Mauszeiger (T0057).
    //!
    //! Vorher zeichnete `draw_cursor` das Icon im beschnittenen Maler der
    //! Zeichenfläche und stieg bei einem Zeiger außerhalb der Fläche sofort
    //! aus — über Ribbon, Eigenschaften-Panel oder Statusleiste war es nicht
    //! zu sehen. Jetzt zeichnet `draw_werkzeug_zeiger` es auf einer eigenen
    //! Ebene im ganzen Fenster. Geprüft wird am gerenderten Bild (Kreise mit
    //! dem Icon-Radius) und an ihrem Beschnitt: der Beschnitt der Zeichenfläche
    //! wäre der Beweis für den alten Fehler.

    use super::*;
    use crate::app::App;

    const FENSTER: Vec2 = Vec2::new(1500.0, 950.0);

    fn rahmen(ctx: &egui::Context, app: &mut App, zeiger: Option<Pos2>) -> egui::FullOutput {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, FENSTER)),
            ..Default::default()
        };
        if let Some(p) = zeiger {
            raw.events.push(egui::Event::PointerMoved(p));
        }
        ctx.run(raw, |ctx| app.ui(ctx))
    }

    /// Die Kreise des Icons (Füllung und Rand) an `mitte`.
    fn icon_kreise(out: &egui::FullOutput, mitte: Pos2) -> Vec<egui::epaint::ClippedShape> {
        out.shapes
            .iter()
            .filter(|cs| match &cs.shape {
                egui::Shape::Circle(c) => {
                    (c.radius - 12.0).abs() < 0.01 && c.center.distance(mitte) < 0.01
                }
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn app_mit_wandwerkzeug(ctx: &egui::Context) -> App {
        let mut app = App::with_ctx(ctx.clone());
        app.start_tool("draw.wall");
        app
    }

    /// Zeiger über dem Ribbon: dort ist das Icon am Mauszeiger zu sehen, und
    /// zwar ohne den Beschnitt der Zeichenfläche (sonst wäre es abgeschnitten).
    #[test]
    fn icon_haengt_auch_ueber_dem_ribbon_am_zeiger() {
        let ctx = egui::Context::default();
        let mut app = app_mit_wandwerkzeug(&ctx);
        rahmen(&ctx, &mut app, None);
        let flaeche = app.ui.last_canvas;
        let zeiger = Pos2::new(700.0, flaeche.top() - 30.0);
        assert!(
            !flaeche.contains(zeiger),
            "Prüfpunkt muss außerhalb der Zeichenfläche liegen ({flaeche:?})"
        );

        let out = rahmen(&ctx, &mut app, Some(zeiger));
        let kreise = icon_kreise(&out, werkzeug_zeiger_mitte(zeiger));
        assert_eq!(
            kreise.len(),
            2,
            "Füllung und Rand des Werkzeug-Icons fehlen über dem Ribbon"
        );
        let fenster = Rect::from_min_size(Pos2::ZERO, FENSTER);
        for c in &kreise {
            assert!(
                c.clip_rect.contains_rect(Rect::from_center_size(
                    werkzeug_zeiger_mitte(zeiger),
                    Vec2::splat(24.0)
                )),
                "Icon ist beschnitten ({:?}) — es muss im ganzen Fenster sichtbar sein",
                c.clip_rect
            );
            assert!(
                fenster.contains_rect(c.clip_rect),
                "Beschnitt liegt außerhalb des Fensters ({:?})",
                c.clip_rect
            );
        }
    }

    /// Über der Zeichenfläche bleibt es bei **einem** Icon — der Umzug auf
    /// eine eigene Ebene darf es nicht doppelt zeichnen.
    #[test]
    fn icon_ueber_der_zeichenflaeche_genau_einmal() {
        let ctx = egui::Context::default();
        let mut app = app_mit_wandwerkzeug(&ctx);
        rahmen(&ctx, &mut app, None);
        let flaeche = app.ui.last_canvas;
        let zeiger = flaeche.center();

        let out = rahmen(&ctx, &mut app, Some(zeiger));
        assert_eq!(
            icon_kreise(&out, werkzeug_zeiger_mitte(zeiger)).len(),
            2,
            "genau ein Icon (Füllung + Rand), nicht zwei"
        );
    }

    /// Ohne laufendes Werkzeug gibt es kein Icon — auch nicht über dem Ribbon.
    #[test]
    fn ohne_werkzeug_kein_icon() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        rahmen(&ctx, &mut app, None);
        let zeiger = Pos2::new(700.0, app.ui.last_canvas.top() - 30.0);

        let out = rahmen(&ctx, &mut app, Some(zeiger));
        assert!(
            icon_kreise(&out, werkzeug_zeiger_mitte(zeiger)).is_empty(),
            "ohne Werkzeug darf kein Icon am Zeiger hängen"
        );
    }

    /// Das Icon sitzt immer unten rechts neben dem Zeiger, unabhängig davon,
    /// wo im Fenster er steht.
    #[test]
    fn lage_ist_immer_derselbe_versatz() {
        let a = werkzeug_zeiger_mitte(Pos2::new(100.0, 200.0));
        let b = werkzeug_zeiger_mitte(Pos2::new(900.0, 40.0));
        assert_eq!(a - Pos2::new(100.0, 200.0), b - Pos2::new(900.0, 40.0));
        assert!(a.x > 100.0 && a.y > 200.0, "unten rechts vom Zeiger");
    }
}

#[cfg(test)]
mod schrifttests {
    use super::*;

    /// Gleiche Texthöhe über alle Schriften (T0059): ein Text mit `height` mm
    /// steht in jeder Familie gleich hoch. Die mitgelieferten technischen
    /// Schriften haben andere Kennwerte (DIN 1451: `units_per_em` 1000,
    /// Osifont: Auf-/Abstieg 1,66 Em) — ohne diese Prüfung fiele eine zu
    /// kleine oder große Zeile erst im Bild auf.
    #[test]
    fn textzeile_ist_in_jeder_schrift_gleich_hoch() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let _ = ctx.run(Default::default(), |_| {});
        let hoehe = |f: jjcad_core::model::TextFont| -> f32 {
            let familie = schrift_familie(&ctx, f, false);
            let job = egui::text::LayoutJob::simple(
                "H".to_owned(),
                FontId::new(100.0, familie),
                Color32::WHITE,
                f32::INFINITY,
            );
            ctx.fonts(|fo| fo.layout_job(job)).size().y
        };
        let soll = hoehe(jjcad_core::model::TextFont::Sans);
        assert!(soll > 50.0, "Sans-Zeile zu klein gemessen: {soll}");
        for f in jjcad_core::model::TextFont::ALL {
            let h = hoehe(f);
            assert!(
                (h - soll).abs() <= soll * 0.15,
                "{} setzt die Zeile mit {h} statt {soll}",
                f.label()
            );
        }
    }
}

#[cfg(test)]
mod tests_schnellauswahl {
    //! T0060: In der Schnellauswahl bestimmt der **Klick** das Werkzeug.
    //!
    //! Geprüft über denselben Weg wie die Maus (`tool_click`) und am
    //! laufenden Werkzeug — nicht an der Vorschau. Die Objekte entstehen über
    //! die Befehlsschicht, damit die gemessene Art die gespeicherte ist.

    use super::*;
    use jjcad_core::units::Unit;

    fn app_mit_objekten() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = Unit::Mm;
        let wand = app
            .session_mut()
            .execute(
                "draw.wall",
                &json!({"a": [0.0, 0.0], "b": [4000.0, 0.0], "thickness": 240.0, "height": 2500.0}),
            )
            .expect("Wand anlegen");
        assert!(wand["ok"].as_bool().unwrap_or(false));
        app.session_mut()
            .execute(
                "dim.chain",
                &json!({"pts": [[0.0, -500.0], [2000.0, -500.0], [4000.0, -500.0]],
                        "line_pos": [0.0, -800.0]}),
            )
            .expect("Maßkette anlegen");
        app.start_tool("modify.quickselect");
        app
    }

    /// Klick auf die Wandachse startet das Wandwerkzeug — ein weiteres Objekt
    /// dieser Art entsteht mit den Standardeinstellungen.
    #[test]
    fn klick_auf_wand_startet_das_wandwerkzeug() {
        let mut app = app_mit_objekten();
        tool_click(&mut app, V2::new(2000.0, 0.0));
        assert_eq!(
            app.tool_id(),
            Some("draw.wall"),
            "die Schnellauswahl muss das Wandwerkzeug starten"
        );
        assert!(
            app.tool.as_ref().map(|t| t.pts.is_empty()).unwrap_or(false),
            "das gestartete Werkzeug beginnt ohne gesetzten Punkt"
        );
    }

    /// Klick auf die Maßlinie einer Maßkette startet die Maßkette (nicht das
    /// Einzelmaß) — die Zuordnung kommt aus `actions::erzeuger_fuer`.
    #[test]
    fn klick_auf_masskette_startet_die_masskette() {
        let mut app = app_mit_objekten();
        tool_click(&mut app, V2::new(2000.0, -800.0));
        assert_eq!(
            app.tool_id(),
            Some("dim.chain"),
            "die Schnellauswahl muss die Maßkette starten"
        );
    }

    /// Ein Klick ins Leere beendet die Schnellauswahl nicht — sie wartet auf
    /// das nächste Objekt.
    #[test]
    fn klick_ins_leere_bleibt_bei_der_schnellauswahl() {
        let mut app = app_mit_objekten();
        tool_click(&mut app, V2::new(9000.0, 9000.0));
        assert_eq!(app.tool_id(), Some("modify.quickselect"));
    }
}

mod tests_massketten_farbe {
    //! T0062: Neue Maßketten übernehmen Ebene und Farbe aus dem
    //! Eigenschaften-Panel und sind von Haus aus **schwarz**.
    //!
    //! Der Fehler steckte in `canvas::run`: die Vorgaben (Ebene, Farbe, Stärke,
    //! Linientyp) wurden nur für `draw.*` weitergegeben. Die Erzeuger der
    //! Bemaßung heißen `dim.*`; ihre neuen Objekte fielen deshalb in der
    //! Befehls-Schicht auf die Standardebene „Bemaßung“ zurück und erbten
    //! deren Rot — egal was im Panel stand.

    use super::*;
    use jjcad_core::style::Color;
    use jjcad_core::units::Unit;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = Unit::Mm;
        app
    }

    /// Höhenkote/Maß/… über die Befehls-Schicht (derselbe Weg wie die Aktionen).
    fn neu(app: &mut App, action: &str, params: serde_json::Value) -> Entity {
        assert!(run(app, action, params), "{action} ist fehlgeschlagen");
        app.doc()
            .entities
            .iter()
            .rev()
            .find(|e| {
                matches!(
                    e.kind,
                    EntityKind::Dim { .. }
                        | EntityKind::DimChain { .. }
                        | EntityKind::ElevationPlan { .. }
                        | EntityKind::ElevationSection { .. }
                )
            })
            .cloned()
            .expect("keine Bemaßung angelegt")
    }

    fn ebene_name(app: &App, e: &Entity) -> String {
        app.doc()
            .layer(e.layer)
            .map(|l| l.name.clone())
            .unwrap_or_default()
    }

    /// Ohne Farbwahl im Panel ist eine neue Maßkette schwarz — auch wenn die
    /// Ebene „Bemaßung“ selbst rot ist.
    #[test]
    fn neue_masskette_ist_schwarz() {
        let mut app = app();
        let e = neu(
            &mut app,
            "dim.chain",
            json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [4000.0, 0.0]],
                   "line_pos": [0.0, -800.0]}),
        );
        assert_eq!(
            app.doc().effective_color(&e),
            Color::BLACK,
            "neue Maßkette muss schwarz sein, ist {}",
            app.doc().effective_color(&e).to_hex()
        );
    }

    /// Die im Panel gewählte Ebene gilt auch für die Bemaßung — nicht die
    /// Standardebene „Bemaßung“.
    #[test]
    fn masskette_landet_auf_der_gewaehlten_ebene() {
        let mut app = app();
        let ziel = app
            .doc()
            .layer_id_by_name("Text")
            .expect("Ebene „Text“ fehlt");
        app.doc_mut().current_layer = ziel;
        let e = neu(
            &mut app,
            "dim.chain",
            json!({"pts": [[0.0, 0.0], [2000.0, 0.0]], "line_pos": [0.0, -800.0]}),
        );
        assert_eq!(e.layer, ziel, "Ebene aus dem Panel");
        assert_eq!(ebene_name(&app, &e), "Text");
    }

    /// Auch eine Farbe aus dem Panel bleibt stehen (vorher überschrieb die
    /// Standardebene sie mit ihrem Rot).
    #[test]
    fn masskette_haelt_die_farbe_aus_dem_panel() {
        let mut app = app();
        app.ui.pending.color = Some(Color::BLUE);
        let e = neu(
            &mut app,
            "dim.chain",
            json!({"pts": [[0.0, 0.0], [2000.0, 0.0]], "line_pos": [0.0, -800.0]}),
        );
        assert_eq!(e.style.color, Some(Color::BLUE), "Farbe aus dem Panel");
        assert_eq!(app.doc().effective_color(&e), Color::BLUE);
    }

    /// Dasselbe für das Einzelmaß und die Höhenkote — alle drei Erzeuger
    /// laufen über denselben Parameterweg.
    #[test]
    fn einzelmass_und_hoehenkote_nehmen_die_vorgaben() {
        let mut app = app();
        let ziel = app.doc().layer_id_by_name("Schraffur").expect("Ebene fehlt");
        app.doc_mut().current_layer = ziel;
        app.ui.pending.color = Some(Color::GREEN);
        let mass = neu(
            &mut app,
            "dim.linear",
            json!({"a": [0.0, 0.0], "b": [3000.0, 0.0], "line_pos": [0.0, -500.0]}),
        );
        assert_eq!(mass.layer, ziel);
        assert_eq!(mass.style.color, Some(Color::GREEN));
        let kote = neu(
            &mut app,
            "dim.elevation.plan",
            json!({"p": [1000.0, 0.0], "z": 2500.0}),
        );
        assert_eq!(kote.layer, ziel);
        assert_eq!(kote.style.color, Some(Color::GREEN));
    }

    /// Klickweg wie mit der Maus: drei Punkte, Enter beendet die Kette. Das
    /// Ergebnis trägt die Vorgaben aus den Werkzeugoptionen (T0062).
    #[test]
    fn klickweg_der_masskette_nimmt_die_vorgaben() {
        let mut app = app();
        let ziel = app.doc().layer_id_by_name("Achsen").expect("Ebene fehlt");
        app.doc_mut().current_layer = ziel;
        app.ui.pending.color = Some(Color::PURPLE);
        app.start_tool("dim.chain");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(2000.0, 0.0));
        tool_click(&mut app, V2::new(4000.0, 0.0));
        finish_tool(&mut app, true);
        assert!(app.tool.is_none(), "die Kette ist beendet");
        let e = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::DimChain { .. }))
            .cloned()
            .expect("keine Maßkette angelegt");
        assert_eq!(e.layer, ziel, "Ebene der Werkzeugoptionen");
        assert_eq!(e.style.color, Some(Color::PURPLE), "Farbe der Werkzeugoptionen");
    }
}

#[cfg(test)]
mod tests_hoehenkoten {
    //! T0096: Klickweg der beiden Koten.
    //!
    //! Die Grundriss-Kote setzt ein Objekt pro Klick, die Schnitt-Kote sammelt
    //! beliebig viele Punkte und schließt mit Enter. Geprüft wird über denselben
    //! Weg wie die Maus (`tool_click` + `finish_tool`).

    use super::*;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app
    }

    /// Ein Klick setzt eine Grundriss-Kote; das Werkzeug bleibt für weitere
    /// Marken stehen.
    #[test]
    fn grundriss_kote_setzt_ein_symbol_pro_klick() {
        let mut app = app();
        app.options.elevation_z = 1200.0;
        app.start_tool("dim.elevation.plan");
        tool_click(&mut app, V2::new(1000.0, 500.0));
        tool_click(&mut app, V2::new(3000.0, 500.0));
        let koten: Vec<_> = app
            .doc()
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::ElevationPlan { .. }))
            .collect();
        assert_eq!(koten.len(), 2, "zwei Grundriss-Koten erwartet");
        assert!(
            matches!(koten[0].kind, EntityKind::ElevationPlan { z, .. } if (z - 1200.0).abs() < 1e-9),
            "Höhe aus den Werkzeugoptionen"
        );
        assert_eq!(
            app.tool_id(),
            Some("dim.elevation.plan"),
            "das Werkzeug bleibt für die nächste Kote stehen"
        );
    }

    /// Alle Texte, die die Werkzeug-Vorschau malt (T0122).
    fn vorschau_texte(app: &mut App, cursor: V2) -> Vec<String> {
        fn sammle(s: &egui::Shape, texte: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => texte.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| sammle(s, texte)),
                _ => {}
            }
        }
        let ctx = app.ctx.clone();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("vorschau"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
            let view = View {
                center: V2::new(0.0, 1000.0),
                zoom: 0.2,
                rot: 0.0,
            };
            draw_tool_preview(app, &p, rect, view, Some(cursor), Some(cursor));
        });
        let mut texte = Vec::new();
        for cs in &out.shapes {
            sammle(&cs.shape, &mut texte);
        }
        texte
    }

    /// Die Vorschau der Schnitt-Kote ist das fertige Objekt: sie schreibt die
    /// Höhe unter jedes Dreieck — auch unter den Punkt am Zeiger. Vorher zeigte
    /// sie nur Umrisse, der Nutzer setzte die Kote blind (T0122).
    #[test]
    fn vorschau_der_schnitt_kote_zeigt_die_hoehen() {
        let mut app = app();
        app.start_tool("dim.elevation.section");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 1200.0));
        let texte = vorschau_texte(&mut app, V2::new(0.0, 2000.0));
        assert_eq!(
            texte,
            vec![
                "±0,00".to_string(),
                "+1,200".to_string(),
                "+2,000".to_string()
            ],
            "die Vorschau zeigt nicht die Höhen aller Punkte"
        );
    }

    /// Die Schnitt-Kote sammelt Punkte; Enter schließt die Folge ab.
    #[test]
    fn schnitt_kote_sammelt_punkte_und_schliesst_mit_enter() {
        let mut app = app();
        app.start_tool("dim.elevation.section");
        tool_click(&mut app, V2::new(0.0, 0.0));
        tool_click(&mut app, V2::new(0.0, 1200.0));
        tool_click(&mut app, V2::new(1000.0, -350.0));
        assert_eq!(
            app.tool.as_ref().map(|t| t.pts.len()),
            Some(3),
            "drei Punkte im Werkzeug"
        );
        finish_tool(&mut app, true);
        assert!(app.tool.is_none(), "Enter beendet die Schnitt-Kote");
        let e = app
            .doc()
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::ElevationSection { .. }))
            .cloned()
            .expect("keine Schnitt-Kote angelegt");
        match e.kind {
            EntityKind::ElevationSection { pts } => {
                assert_eq!(pts.len(), 3);
                assert!((pts[1].y - 1200.0).abs() < 1e-9, "Punktlage gespeichert");
            }
            k => panic!("falsche Objektart: {k:?}"),
        }
    }
}

#[cfg(test)]
mod tests_stutzvorschau {
    //! Vorschau des Stutzens (T0075): steht der Zeiger auf einer Linie, ist der
    //! Abschnitt, den ein Klick entfernt, vorher in der Warnfarbe zu sehen.

    use super::*;

    const RECT: Rect = Rect {
        min: Pos2::new(0.0, 0.0),
        max: Pos2::new(1200.0, 800.0),
    };

    /// Zwei Schnittkanten bei x = 2000/4000 mm und eine Linie von 0 bis 6000 mm.
    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit setzen");
        for (a, b) in [
            ([2, -1], [2, 1]),
            ([4, -1], [4, 1]),
            ([0, 0], [6, 0]),
        ] {
            app.session_mut()
                .execute("draw.line", &json!({ "a": a, "b": b }))
                .expect("Linie anlegen");
        }
        app.start_tool("modify.trim");
        app
    }

    /// Die Strecken der Warnfarbe, die `draw_trim_preview` malt, mit Strichbreite.
    fn stutz_strecken(app: &mut App, zeiger: Option<V2>) -> Vec<(Pos2, Pos2, f32)> {
        let view = app.current_view();
        let ctx = app.ctx.clone();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("stutz"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            draw_tool_preview(app, &p, RECT, view, zeiger, zeiger);
        });
        let mut v = Vec::new();
        for cs in &out.shapes {
            if let Shape::LineSegment { points, stroke } = &cs.shape {
                if stroke.color == egui::epaint::ColorMode::Solid(Palette::danger()) {
                    v.push((points[0], points[1], stroke.width));
                }
            }
        }
        v
    }

    /// Alle Formen der Warnfarbe (auch Pfade — ein Bogen wird als Linienzug
    /// gezeichnet, nicht als einzelne Strecke).
    fn stutz_formen(app: &mut App, zeiger: Option<V2>) -> Vec<usize> {
        let view = app.current_view();
        let ctx = app.ctx.clone();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("stutz"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            draw_tool_preview(app, &p, RECT, view, zeiger, zeiger);
        });
        let mut v = Vec::new();
        for cs in &out.shapes {
            let farbe = match &cs.shape {
                Shape::LineSegment { stroke, .. } => Some(stroke.color.clone()),
                Shape::Path(p) => Some(p.stroke.color.clone()),
                _ => None,
            };
            if farbe == Some(egui::epaint::ColorMode::Solid(Palette::danger())) {
                v.push(match &cs.shape {
                    Shape::Path(p) => p.points.len(),
                    _ => 2,
                });
            }
        }
        v
    }

    /// Strichstärke des Zielobjekts setzen (Papier-Millimeter).
    fn staerke(app: &mut App, id: Id, mm: f64) {
        let e = app.doc_mut().entities.iter_mut().find(|e| e.id == id).unwrap();
        e.style.line_weight = Some(mm);
    }

    fn ziel_id(app: &App) -> Id {
        app.doc().entities.last().unwrap().id
    }

    /// Kreis, der von einem zweiten Kreis geschnitten wird (T0084, Turn 2):
    /// die Vorschau zeigt die Kappe zwischen den beiden Schnittpunkten.
    #[test]
    fn kreisvorschau_zeigt_die_kappe_zwischen_zwei_kreisen() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit setzen");
        for c in [[0, 0], [2, 0]] {
            app.session_mut()
                .execute("draw.circle", &json!({ "c": c, "r": 2 }))
                .expect("Kreis anlegen");
        }
        app.start_tool("modify.trim");
        // Punkt auf dem rechten Scheitel des ersten Kreises (45°).
        let formen = stutz_formen(&mut app, Some(V2::new(1414.0, 1414.0)));
        assert!(!formen.is_empty(), "Vorschau fehlt: {formen:?}");
        assert!(
            formen.iter().any(|n| *n > 4),
            "Bogen als Linienzug erwartet: {formen:?}"
        );
    }

    #[test]
    fn abschnitt_zwischen_den_kanten_ist_hervorgehoben() {
        let mut app = app();
        let view = app.current_view();
        let strecken = stutz_strecken(&mut app, Some(V2::new(3000.0, 0.0)));
        assert_eq!(strecken.len(), 1, "genau ein Abschnitt: {strecken:?}");
        let (erwartet_a, erwartet_b) = (
            world_to_screen(RECT, view, V2::new(2000.0, 0.0)),
            world_to_screen(RECT, view, V2::new(4000.0, 0.0)),
        );
        let (a, b, _breite) = strecken[0];
        assert!(
            (a.x - erwartet_a.x).abs() < 0.6 && (a.y - erwartet_a.y).abs() < 0.6,
            "Anfang an der ersten Kante: {a:?} statt {erwartet_a:?}"
        );
        assert!(
            (b.x - erwartet_b.x).abs() < 0.6 && (b.y - erwartet_b.y).abs() < 0.6,
            "Ende an der zweiten Kante: {b:?} statt {erwartet_b:?}"
        );
    }

    /// Neben der Linie (mehr als der Trefferradius) gibt es nichts hervorzuheben.
    #[test]
    fn neben_der_linie_keine_hervorhebung() {
        let mut app = app();
        let strecken = stutz_strecken(&mut app, Some(V2::new(3000.0, 900.0)));
        assert!(strecken.is_empty(), "kein Abschnitt ohne Linie: {strecken:?}");
    }

    /// Die Hervorhebung folgt der Linienstärke (Turn 2): eine fette Linie bekommt
    /// einen breiteren roten Strich als eine dünne, sonst verschwindet sie darunter.
    #[test]
    fn hervorhebung_folgt_der_linienstaerke() {
        let mut app = app();
        let id = ziel_id(&app);
        // Modus ausdrücklich setzen: er kommt sonst aus der echten `prefs.json`
        // des Nutzers (hier steht DPI), und der Test hinge an seiner Einstellung.
        app.ui.lineweights_scaled = false;
        staerke(&mut app, id, 0.25);
        let duenn = stutz_strecken(&mut app, Some(V2::new(3000.0, 0.0)))[0].2;
        staerke(&mut app, id, 2.0);
        let fett = stutz_strecken(&mut app, Some(V2::new(3000.0, 0.0)))[0].2;
        assert!(
            fett > duenn + 1.0,
            "fette Linie braucht einen breiteren Strich: dünn {duenn}, fett {fett}"
        );
        // px-Modus: 2,0 mm × 3,2 px/mm + 1 px Zuschlag
        assert!((fett - 7.4).abs() < 0.01, "Breite im px-Modus: {fett}");
    }

    /// Im DPI-Modus wächst die Hervorhebung mit dem Zoom — vorher blieb sie bei
    /// 3 px stehen, während die Linie selbst breiter wurde.
    #[test]
    fn hervorhebung_skaliert_im_dpi_modus() {
        let mut app = app();
        let id = ziel_id(&app);
        staerke(&mut app, id, 0.5);
        app.ui.lineweights_scaled = true;
        let mut view = app.current_view();
        view.zoom = 0.08; // Papier-mm → 8 px
        app.set_view(view);
        let klein = stutz_strecken(&mut app, Some(V2::new(3000.0, 0.0)))[0].2;
        view.zoom = 0.40; // Papier-mm → 40 px
        app.set_view(view);
        let gross = stutz_strecken(&mut app, Some(V2::new(4000.0 - 100.0, 0.0)))[0].2;
        assert!(
            (klein - 5.0).abs() < 0.01,
            "0,5 mm bei 8 px/mm → 4 px + 1: {klein}"
        );
        assert!(
            (gross - 21.0).abs() < 0.01,
            "0,5 mm bei 40 px/mm → 20 px + 1: {gross}"
        );
    }
}

#[cfg(test)]
mod tests_verlaengernvorschau {
    //! Vorschau des Verlängerns (T0081): steht der Zeiger auf einem Ende, ist
    //! die Strecke bis zur nächsten Schnittkante vorher in der Erfolgsfarbe zu
    //! sehen.

    use super::*;

    const RECT: Rect = Rect {
        min: Pos2::new(0.0, 0.0),
        max: Pos2::new(1200.0, 800.0),
    };

    /// Schnittkanten bei x = 2000/4000 mm und eine Linie von 0 bis 1000 mm.
    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit setzen");
        for (a, b) in [([2, -1], [2, 1]), ([4, -1], [4, 1]), ([0, 0], [1, 0])] {
            app.session_mut()
                .execute("draw.line", &json!({ "a": a, "b": b }))
                .expect("Linie anlegen");
        }
        app.start_tool("modify.extend");
        app
    }

    /// Die Strecken der Erfolgsfarbe, die `draw_extend_preview` malt.
    fn vorschau_strecken(app: &mut App, zeiger: Option<V2>) -> Vec<(Pos2, Pos2, f32)> {
        let view = app.current_view();
        let ctx = app.ctx.clone();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("verlaengern"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            draw_tool_preview(app, &p, RECT, view, zeiger, zeiger);
        });
        let mut v = Vec::new();
        for cs in &out.shapes {
            if let Shape::LineSegment { points, stroke } = &cs.shape {
                if stroke.color == egui::epaint::ColorMode::Solid(Palette::ok()) {
                    v.push((points[0], points[1], stroke.width));
                }
            }
        }
        v
    }

    fn ziel_id(app: &App) -> Id {
        app.doc().entities.last().unwrap().id
    }

    #[test]
    fn strecke_bis_zur_naechsten_kante_ist_hervorgehoben() {
        let mut app = app();
        let view = app.current_view();
        let strecken = vorschau_strecken(&mut app, Some(V2::new(1000.0, 0.0)));
        assert_eq!(strecken.len(), 1, "genau eine Strecke: {strecken:?}");
        let (erwartet_a, erwartet_b) = (
            world_to_screen(RECT, view, V2::new(1000.0, 0.0)),
            world_to_screen(RECT, view, V2::new(2000.0, 0.0)),
        );
        let (a, b, _breite) = strecken[0];
        assert!(
            (a.x - erwartet_a.x).abs() < 0.6 && (a.y - erwartet_a.y).abs() < 0.6,
            "Anfang am wandernden Ende: {a:?} statt {erwartet_a:?}"
        );
        assert!(
            (b.x - erwartet_b.x).abs() < 0.6 && (b.y - erwartet_b.y).abs() < 0.6,
            "Ende an der nächsten Kante: {b:?} statt {erwartet_b:?}"
        );
    }

    /// Neben der Linie (mehr als der Trefferradius) gibt es nichts zu zeigen.
    #[test]
    fn neben_der_linie_keine_hervorhebung() {
        let mut app = app();
        let strecken = vorschau_strecken(&mut app, Some(V2::new(1000.0, 900.0)));
        assert!(strecken.is_empty(), "keine Strecke ohne Linie: {strecken:?}");
    }

    /// Die Vorschau zeigt genau die Strecke, die der Klick danach ansetzt.
    #[test]
    fn vorschau_und_ergebnis_sind_dieselbe_strecke() {
        let mut app = app();
        let id = ziel_id(&app);
        let vorschau = vorschau_strecken(&mut app, Some(V2::new(1000.0, 0.0)));
        assert_eq!(vorschau.len(), 1, "eine Strecke: {vorschau:?}");

        tool_click(&mut app, V2::new(1000.0, 0.0));

        let e = app.doc().entity(id).expect("Linie fehlt").clone();
        let EntityKind::Line { a, b } = e.kind else {
            panic!("Linie erwartet");
        };
        assert!(
            a.x.abs() < 1e-6 && (b.x - 2000.0).abs() < 1e-6,
            "der Klick setzt bis zur Kante an: {a:?} nach {b:?}"
        );
        let view = app.current_view();
        let erwartet = world_to_screen(RECT, view, b);
        let (_, vorschau_ende, _) = vorschau[0];
        assert!(
            (vorschau_ende.x - erwartet.x).abs() < 0.6
                && (vorschau_ende.y - erwartet.y).abs() < 0.6,
            "die Vorschau endet am gesetzten Punkt: {vorschau_ende:?} statt {erwartet:?}"
        );
    }
}

#[cfg(test)]
mod tests_tuer_position {
    //! Tür und Fenster liegen in der Wand (T0078): der Zeiger auf der
    //! verlängerten Achse wird auf die freie Wandstrecke gezogen, der Klick
    //! setzt die Öffnung dort — nicht hinter dem Wandende.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    fn wand(app: &mut App, a: (f64, f64), b: (f64, f64)) -> u64 {
        let res = app
            .session_mut()
            .execute(
                "draw.wall",
                &serde_json::json!({"a": [a.0, a.1], "b": [b.0, b.1], "thickness": 0.3}),
            )
            .expect("Wand");
        res["created"][0].as_u64().expect("keine Id")
    }

    fn tueren(app: &App) -> Vec<(Option<f64>, V2, f64)> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Door {
                    at,
                    center,
                    width,
                    ..
                } => Some((*at, *center, *width)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn zeiger_hinter_dem_wandende_bleibt_auf_der_wand() {
        let mut app = app();
        wand(&mut app, (0.0, 0.0), (6.0, 0.0));
        let (p, _, _) = wall_under(&app, V2::new(6200.0, 0.0)).expect("keine Wand");
        assert!(
            (p.x - 6000.0).abs() < 0.5,
            "der Punkt muss am Wandende liegen: {p:?}"
        );
    }

    #[test]
    fn klick_hinter_dem_ende_setzt_die_tuer_in_die_wand() {
        let mut app = app();
        let w = wand(&mut app, (0.0, 0.0), (6.0, 0.0));
        app.start_tool("draw.door");
        // 200 mm hinter dem Wandende geklickt.
        tool_click(&mut app, V2::new(6200.0, 0.0));
        let t = tueren(&app);
        assert_eq!(t.len(), 1, "genau eine Tür");
        let (at, center, breite) = t[0];
        assert!(
            (at.unwrap() - (6000.0 - breite * 0.5)).abs() < 0.5,
            "at muss am Wandende enden: {at:?}"
        );
        assert!((center.x - (6000.0 - breite * 0.5)).abs() < 0.5);
        let wand_e = app.doc().entity(w).unwrap();
        let iv = jjcad_core::render::wall_opening_intervals(app.doc(), wand_e);
        assert!(
            (iv[0].1 - 6000.0).abs() < 0.5,
            "Öffnung endet an der Wand: {iv:?}"
        );
    }
}

#[cfg(test)]
mod tests_kreis_kontur {
    //! Die Kreiskontur liegt **mittig** auf dem Radius (T0080).
    //!
    //! `Painter::circle_stroke` legt die Strichbreite bei egui nach außen
    //! (`PathStroke::outside` in `tessellate_circle`): der Kreis erschien um die
    //! halbe Strichbreite zu groß, und der Punktfang (er rechnet mit `r`) saß am
    //! Innenrand der Linie. Gemessen wird am gezeichneten Rand (Mesh-Eckpunkte),
    //! nicht am Quelltext: die Strichmitte muss auf `r` liegen und die Kontur
    //! nach innen und außen gleich weit reichen.

    use super::*;

    const MITTE: Pos2 = Pos2::new(200.0, 200.0);
    /// Kreisradius in Welt-mm (2,5 cm Radius = 5 cm Durchmesser).
    const RADIUS_MM: f64 = 25.0;

    /// Radiale Ausdehnung der gezeichneten Kontur (min, max) in Bildpunkten.
    ///
    /// Punkte dichter als 20 px an der Mitte gehören dem Mittelkreuz, nicht der
    /// Kontur.
    fn ausdehnung(shapes: Vec<egui::epaint::ClippedShape>) -> (f32, f32) {
        let ctx = egui::Context::default();
        // Schrift laden: `tessellate` setzt einen gelaufenen Durchgang voraus.
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0))),
                ..Default::default()
            },
            |_| {},
        );
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for cp in ctx.tessellate(shapes, 1.0) {
            if let egui::epaint::Primitive::Mesh(m) = &cp.primitive {
                for v in &m.vertices {
                    let d = v.pos.distance(MITTE);
                    if d > 20.0 {
                        min = min.min(d);
                        max = max.max(d);
                    }
                }
            }
        }
        (min, max)
    }

    /// Der Kreis über den Weg der Zeichenfläche zeichnen (`paint_prim`) und die
    /// Ausdehnung seiner Kontur messen.
    fn band_von_paint_prim(scale: f64, breite_mm: f64, skalierte_breiten: bool) -> (f32, f32) {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0))),
            ..Default::default()
        };
        let pc = PaintCtx {
            selected: Vec::new(),
            hover: None,
            lineweights: skalierte_breiten,
            lineweights_scaled: skalierte_breiten,
            paper_px: scale,
            preview: false,
            rot: 0.0,
            text_deckel: crate::textgroesse::RASTER_DECKEL_PX,
        };
        let prim = Prim {
            shape: PShape::Circle {
                c: V2::new(0.0, 0.0),
                r: RADIUS_MM,
            },
            style: jjcad_core::render::PrimStyle {
                color: jjcad_core::style::Color::BLACK,
                line_type: jjcad_core::style::LineType::Solid,
                width: breite_mm,
                fill: None,
                ..Default::default()
            },
            entity: 0,
            layer: 0,
        };
        let w2s = |q: V2| {
            Pos2::new(
                MITTE.x + q.x as f32 * scale as f32,
                MITTE.y - q.y as f32 * scale as f32,
            )
        };
        let out = ctx.run(raw, |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            paint_prim(&painter, &prim, &w2s, scale, &pc, 1.0);
        });
        ausdehnung(out.shapes)
    }

    /// Strichmitte auf dem Radius, Kontur beidseitig gleich breit.
    fn pruefe_mittig(scale: f64, breite_mm: f64, skalierte_breiten: bool) {
        let (min, max) = band_von_paint_prim(scale, breite_mm, skalierte_breiten);
        let r = (RADIUS_MM * scale) as f32;
        let mitte = (min + max) * 0.5;
        assert!(
            (mitte - r).abs() < 0.6,
            "Strichmitte {mitte:.2} px statt Radius {r:.2} px (Kontur {min:.2}…{max:.2}) — \
             der Kreis steht um die halbe Strichbreite neben dem Durchmesser"
        );
        let aussen = max - r;
        let innen = r - min;
        assert!(
            (aussen - innen).abs() < 1.0,
            "Kontur reicht nach außen {aussen:.2} px, nach innen {innen:.2} px — sie muss mittig liegen"
        );
        assert!(aussen > 2.0, "gemessene Kontur ist zu schmal ({min:.2}…{max:.2})");
    }

    #[test]
    fn kontur_liegt_mittig_dpi_modus() {
        // 4 px je mm, 2 mm Strich → 8 px; Fang und Anzeige teilen den Radius.
        pruefe_mittig(4.0, 2.0, true);
    }

    #[test]
    fn kontur_liegt_mittig_px_modus() {
        pruefe_mittig(4.0, 2.0, false);
    }
}

#[cfg(test)]
mod tests_verschneiden {
    //! Verschneiden (T0082): erstes Objekt anklicken, zweites anklicken — beide
    //! treffen sich im Schnittpunkt. Die Vorschau zeigt die Strecken vorher.

    use super::*;

    const RECT: Rect = Rect {
        min: Pos2::new(0.0, 0.0),
        max: Pos2::new(1200.0, 800.0),
    };

    /// Waagerechte Linie 0…1000 mm und senkrechte bei x = 2000 mm, y 1000…2000.
    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit setzen");
        for (a, b) in [([0, 0], [1, 0]), ([2, 1], [2, 2])] {
            app.session_mut()
                .execute("draw.line", &json!({ "a": a, "b": b }))
                .expect("Linie anlegen");
        }
        app.start_tool("modify.intersect");
        app
    }

    fn ids(app: &App) -> (Id, Id) {
        let mut it = app.doc().entities.iter().map(|e| e.id);
        (it.next().unwrap(), it.next().unwrap())
    }

    fn enden(app: &App, id: Id) -> (V2, V2) {
        match &app.doc().entity(id).expect("Objekt fehlt").kind {
            EntityKind::Line { a, b } => (*a, *b),
            other => panic!("Linie erwartet: {other:?}"),
        }
    }

    /// Die Strecken der Erfolgsfarbe, die `draw_verschneiden_preview` malt.
    fn vorschau_strecken(app: &mut App, zeiger: Option<V2>) -> Vec<(Pos2, Pos2)> {
        let view = app.current_view();
        let ctx = app.ctx.clone();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("verschneiden"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            draw_tool_preview(app, &p, RECT, view, zeiger, zeiger);
        });
        let mut v = Vec::new();
        for cs in &out.shapes {
            if let Shape::LineSegment { points, stroke } = &cs.shape {
                if stroke.color == egui::epaint::ColorMode::Solid(Palette::ok()) {
                    v.push((points[0], points[1]));
                }
            }
        }
        v
    }

    #[test]
    fn zwei_klicks_verschneiden_die_linien() {
        let mut app = app();
        let (links, rechts) = ids(&app);

        tool_click(&mut app, V2::new(500.0, 0.0));
        assert_eq!(
            app.tool.as_ref().and_then(|t| t.obj),
            Some(links),
            "der erste Klick merkt sich das Objekt"
        );
        assert_eq!(app.selection(), &[links], "das gemerkte Objekt ist markiert");

        tool_click(&mut app, V2::new(2000.0, 1500.0));

        let (a, b) = enden(&app, links);
        assert!(a.dist(V2::new(0.0, 0.0)) < 1e-6, "Anfang bleibt: {a:?}");
        assert!(
            b.dist(V2::new(2000.0, 0.0)) < 1e-6,
            "Ende am Schnittpunkt: {b:?}"
        );
        let (a, b) = enden(&app, rechts);
        assert!(
            a.dist(V2::new(2000.0, 0.0)) < 1e-6,
            "Anfang am Schnittpunkt: {a:?}"
        );
        assert!(b.dist(V2::new(2000.0, 2000.0)) < 1e-6, "Ende bleibt: {b:?}");

        assert!(
            app.tool.as_ref().and_then(|t| t.obj).is_none(),
            "der Partner ist verbraucht"
        );
        assert_eq!(
            app.tool_id(),
            Some("modify.intersect"),
            "das Werkzeug bleibt für das nächste Paar aktiv"
        );
        assert_eq!(app.selection(), &[links, rechts], "beide bleiben markiert");
    }

    #[test]
    fn vorschau_zeigt_beide_strecken_zum_schnittpunkt() {
        let mut app = app();
        tool_click(&mut app, V2::new(500.0, 0.0));
        let strecken = vorschau_strecken(&mut app, Some(V2::new(2000.0, 1500.0)));
        assert_eq!(strecken.len(), 2, "beide Objekte zeigen ihre Strecke");
        let ziel = world_to_screen(RECT, app.current_view(), V2::new(2000.0, 0.0));
        for (a, b) in strecken {
            assert!(
                (a.x - ziel.x).abs() < 0.6 && (a.y - ziel.y).abs() < 0.6
                    || (b.x - ziel.x).abs() < 0.6 && (b.y - ziel.y).abs() < 0.6,
                "eine Seite endet im Schnittpunkt: {a:?}–{b:?} statt {ziel:?}"
            );
        }
    }

    #[test]
    fn ohne_erstes_objekt_gibt_es_keine_vorschau() {
        let mut app = app();
        let strecken = vorschau_strecken(&mut app, Some(V2::new(2000.0, 1500.0)));
        assert!(
            strecken.is_empty(),
            "vor dem ersten Klick nichts zu zeigen: {strecken:?}"
        );
    }
}

#[cfg(test)]
mod tests_flaechen_bearbeiten {
    //! Flächen vereinigen und schneiden über den Werkzeugweg (T0090).
    //!
    //! Die Geometrie selbst prüft `jjcad_core::flaeche`; hier zählt der Weg
    //! vom Klick bzw. von der Auswahl bis zum Objekt in der Zeichnung.

    use super::*;

    fn app() -> App {
        App::with_ctx(egui::Context::default())
    }

    fn fills(app: &App) -> Vec<(Vec<V2>, f64)> {
        app.doc()
            .entities
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Fill { boundary, .. } => {
                    Some((boundary.clone(), polygon_area(boundary).abs()))
                }
                _ => None,
            })
            .collect()
    }

    fn flaeche_anlegen(app: &mut App, pts: [[f64; 2]; 4]) -> u64 {
        let antwort = app
            .session_mut()
            .execute("draw.fill.points", &json!({ "pts": pts }))
            .expect("Füllfläche");
        antwort["created"][0].as_u64().expect("Id")
    }

    /// Zwei Klicks quer über die Fläche und Enter: die Fläche ist geteilt.
    #[test]
    fn zwei_klicks_und_enter_teilen_die_flaeche() {
        let mut app = app();
        flaeche_anlegen(&mut app, [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]);
        app.start_tool("modify.area.split");
        tool_click(&mut app, V2::new(2000.0, -500.0));
        tool_click(&mut app, V2::new(2000.0, 3500.0));
        finish_tool(&mut app, true);

        let teile = fills(&app);
        assert_eq!(teile.len(), 2, "zwei Teilflächen");
        for (_, a) in &teile {
            assert!(
                (a - 6_000_000.0).abs() < 1.0,
                "Teilfläche {} mm² statt 6 m²",
                a
            );
        }
        assert!(app.tool.is_none(), "Werkzeug beendet");
    }

    /// Ein einzelner Punkt reicht nicht — es bleibt bei der einen Fläche.
    #[test]
    fn ein_punkt_schneidet_nichts() {
        let mut app = app();
        flaeche_anlegen(&mut app, [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]);
        app.start_tool("modify.area.split");
        tool_click(&mut app, V2::new(2000.0, 1500.0));
        finish_tool(&mut app, true);
        assert_eq!(fills(&app).len(), 1, "nichts geteilt");
    }

    /// Vereinigen wirkt auf die Auswahl: aus zwei angrenzenden Flächen wird
    /// eine, und sie ist danach ausgewählt.
    #[test]
    fn vereinigen_aus_der_auswahl_ergibt_eine_flaeche() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        let a = flaeche_anlegen(&mut app, [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]);
        let b = flaeche_anlegen(&mut app, [[4.0, 0.0], [8.0, 0.0], [8.0, 3.0], [4.0, 3.0]]);
        app.session_mut().select(vec![a, b]);
        app.run_action(&ctx, "modify.area.union");

        let flaechen = fills(&app);
        assert_eq!(flaechen.len(), 1, "eine Fläche");
        assert!((flaechen[0].1 - 24_000_000.0).abs() < 1.0, "24 m²");
        assert_eq!(app.selection().len(), 1, "Ergebnis ist ausgewählt");
        assert_eq!(
            app.doc().entity(app.selection()[0]).map(|e| &e.kind),
            app.doc()
                .entities
                .iter()
                .find(|e| matches!(e.kind, EntityKind::Fill { .. }))
                .map(|e| &e.kind),
            "die Auswahl ist die neue Fläche"
        );
    }

    /// Ohne Auswahl passiert nichts — die Fläche bleibt, wie sie ist.
    #[test]
    fn vereinigen_ohne_auswahl_aendert_nichts() {
        let ctx = egui::Context::default();
        let mut app = App::with_ctx(ctx.clone());
        flaeche_anlegen(&mut app, [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]);
        app.session_mut().select(Vec::new());
        app.run_action(&ctx, "modify.area.union");
        assert_eq!(fills(&app).len(), 1);
    }
}

#[cfg(test)]
mod tests_zerlegen {
    //! Zerlegen (T0093): steht etwas in der Auswahl, wird es beim Start sofort
    //! zerlegt. Ohne Auswahl löst jeder Klick das Objekt unter dem Zeiger auf —
    //! vorher hebt die Vorschau es in der Warnfarbe der Stutzen-Funktion hervor.

    use super::*;

    const RECT: Rect = Rect {
        min: Pos2::new(0.0, 0.0),
        max: Pos2::new(1200.0, 800.0),
    };

    /// Eine Wand von (0;0) nach (4;0) in Metern, 240 mm dick.
    fn app_mit_wand() -> (App, Id) {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &json!({ "input_unit": "m" }))
            .expect("Einheit setzen");
        let r = app
            .session_mut()
            .execute(
                "draw.wall",
                &json!({ "a": [0, 0], "b": [4, 0], "thickness": 0.24 }),
            )
            .expect("Wand anlegen");
        let id = r["created"][0].as_u64().expect("Wand-Kennung");
        (app, id)
    }

    fn wand_steht(app: &App) -> bool {
        app.doc()
            .ents()
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Wall { .. }))
    }

    /// Alle Formen der Warnfarbe aus der Werkzeug-Vorschau (Strecken und Pfade).
    fn warnformen(app: &mut App, zeiger: Option<V2>) -> usize {
        let view = app.current_view();
        let ctx = app.ctx.clone();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("zerlegen"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            draw_tool_preview(app, &p, RECT, view, zeiger, zeiger);
        });
        out.shapes
            .iter()
            .filter(|cs| match &cs.shape {
                Shape::LineSegment { stroke, .. } => {
                    stroke.color == egui::epaint::ColorMode::Solid(Palette::danger())
                }
                Shape::Path(p) => p.stroke.color == egui::epaint::ColorMode::Solid(Palette::danger()),
                _ => false,
            })
            .count()
    }

    /// Der Start mit Auswahl ist eine Ausführung: die Wand ist danach zerlegt,
    /// kein Klick-Werkzeug wartet auf Eingaben.
    #[test]
    fn start_zerlegt_die_auswahl_sofort() {
        let (mut app, id) = app_mit_wand();
        app.session_mut().select(vec![id]);
        app.start_tool("modify.explode");

        assert!(!wand_steht(&app), "die Wand in der Auswahl muss zerlegt sein");
        assert!(
            app.doc().ents().len() > 1,
            "es müssen Einzelteile entstanden sein: {}",
            app.doc().ents().len()
        );
        assert!(app.tool.is_none(), "mit Auswahl startet kein Klick-Werkzeug");
        assert!(
            app.selection().is_empty(),
            "die Kennungen der zerlegten Objekte sind weg"
        );
    }

    /// Ohne Auswahl wird das Werkzeug aktiv, und der Klick zerlegt — er sammelt
    /// nicht in der Auswahl (das war der gemeldete Fehler).
    #[test]
    fn ohne_auswahl_zerlegt_der_klick_das_objekt() {
        let (mut app, _id) = app_mit_wand();
        assert!(app.selection().is_empty());
        app.start_tool("modify.explode");
        assert_eq!(app.tool_id(), Some("modify.explode"), "Werkzeug läuft");
        assert!(
            !app.tool.as_ref().map(|t| t.selecting).unwrap_or(true),
            "der Klick darf nicht nur auswählen"
        );

        tool_click(&mut app, V2::new(2000.0, 0.0));

        assert!(!wand_steht(&app), "der Klick muss die Wand zerlegen");
        assert!(
            app.selection().is_empty(),
            "der Klick sammelt nicht in der Auswahl"
        );
        assert_eq!(
            app.tool_id(),
            Some("modify.explode"),
            "bleibt für das nächste Objekt aktiv"
        );
    }

    /// Die Vorschau hebt das Objekt unter dem Zeiger in derselben Warnfarbe wie
    /// die Stutzen-Vorschau hervor — und nur dort.
    #[test]
    fn vorschau_hebt_das_objekt_unter_dem_zeiger_hervor() {
        let (mut app, _id) = app_mit_wand();
        app.start_tool("modify.explode");

        let auf_der_wand = warnformen(&mut app, Some(V2::new(2000.0, 0.0)));
        assert!(
            auf_der_wand > 0,
            "die Wand unter dem Zeiger muss hervorgehoben sein"
        );

        let neben_der_wand = warnformen(&mut app, Some(V2::new(2000.0, 5000.0)));
        assert_eq!(neben_der_wand, 0, "ohne Objekt keine Hervorhebung");
    }
}

#[cfg(test)]
mod tests_raumstempel {
    //! T0097: Werkzeugweg des Raumstempels — Punkte sammeln, Abschluss legt
    //! die unsichtbare Fläche samt Textfeld in ihrer Mitte an.

    use super::*;
    use jjcad_core::units::Unit;

    fn app() -> App {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = Unit::Mm;
        app
    }

    fn mit_punkten(mut app: App, pts: Vec<V2>) -> App {
        app.start_tool("dim.room");
        let mut t = app.tool.clone().expect("Werkzeug startet nicht");
        t.pts = pts;
        app.tool = Some(t);
        app
    }

    /// Vier Klicks + Abschluss: ein Stempel, Text in der Flächenmitte, Objekt
    /// ausgewählt, Werkzeug beendet.
    #[test]
    fn werkzeug_legt_stempel_in_der_flaechenmitte_an() {
        let mut app = mit_punkten(
            app(),
            vec![
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
        );
        finish_tool(&mut app, true);

        assert!(app.tool.is_none(), "Werkzeug blieb stehen");
        assert_eq!(app.doc().entities.len(), 1, "genau ein Objekt");
        match &app.doc().entities[0].kind {
            EntityKind::Room { boundary, label, .. } => {
                assert_eq!(boundary.len(), 4, "vier Ecken");
                assert!(label.dist(V2::new(2000.0, 1500.0)) < 1.0, "Textlage {label:?}");
            }
            k => panic!("kein Raumstempel: {}", k.type_name()),
        }
        assert_eq!(app.selection().len(), 1, "der neue Stempel wird ausgewählt");
    }

    /// Unter drei Ecken entsteht kein Stempel — das Werkzeug bleibt stehen,
    /// damit der gesetzte Punkt nicht verloren ist (ESC verwirft).
    #[test]
    fn zu_wenige_punkte_legen_nichts_an() {
        let mut app = mit_punkten(app(), vec![V2::new(0.0, 0.0), V2::new(1000.0, 0.0)]);
        finish_tool(&mut app, true);
        assert!(app.doc().entities.is_empty(), "es wurde trotzdem etwas angelegt");
    }
}

#[cfg(test)]
mod tests_textrahmen {
    //! T0110: Der Rahmen um ein Textfeld ist die **echte** Größe der Schrift.
    //!
    //! Gemessen wird die gesetzte Galley (dieselbe Schrift, dieselbe
    //! Zeilenhöhe, derselbe Weg wie in der Zeichnung: `canvas::satz`) gegen den
    //! Kasten aus `render::TextPrim::box_size` — der Rahmen, den Auswahl,
    //! Fang und Vorschau zeigen. Vorher schätzte `render::text_box` die Breite
    //! je Zeichen und nahm fest 1,4 Zeilenhöhen; das traf nur serifenlos.

    use super::*;
    use jjcad_core::model::TextFont;

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let _ = ctx.run(Default::default(), |_| {});
        ctx
    }

    fn prim(text: &str, font: TextFont, abstand: f64) -> render::TextPrim {
        text_preview_prim_mit(
            text.to_string(),
            // groß genug, dass egui's Rundung auf ganze Bildpunkte (je Zeichen
            // bis zu einem halben) neben den Millimetern verschwindet
            1000.0,
            TextAlignH::Left,
            TextAlignV::Middle,
            jjcad_core::text::RunStyle::default(),
            font,
            abstand,
            0.0,
        )
    }

    /// Für jede Schriftfamilie: Kasten und gesetzte Galley sind gleich groß.
    ///
    /// Die Höhe muss zeilengenau stimmen (je Zeile höchstens ein Pixel
    /// Rundung), die Breite darf die Galley nicht schneiden und höchstens 2 %
    /// Luft haben — Kerning setzt die Zeichen enger als die Summe der
    /// Vorschübe.
    #[test]
    fn rahmen_ist_so_gross_wie_die_gesetzte_schrift() {
        let ctx = ctx();
        let mut geprueft = 0;
        for font in TextFont::ALL {
            for text in ["Hallo", "Wohnen 12,4", "A\nB\nC", "Hallo\nWohnen"] {
                for abstand in [1.0, 1.5] {
                    let t = prim(text, font, abstand);
                    let size = (t.height * render::TEXT_GROESSE_FAKTOR) as f32;
                    let g = satz(&ctx, &t, size, Color32::WHITE);
                    let (w, h) = t.box_size();
                    let (gw, gh) = (g.size().x as f64, g.size().y as f64);
                    let zeilen = t.lines() as f64;
                    assert!(
                        (gh - h).abs() <= zeilen * 1.5 + 0.5,
                        "{} / {:?} / Abstand {abstand}: Rahmen {h:.1} mm hoch, \
                         Schrift {gh:.1} mm (Zeilen {zeilen})",
                        font.key(),
                        text
                    );
                    // egui rundet jede Zeichenposition auf ganze Bildpunkte;
                    // bei langen Zeilen summiert sich das auf Bruchteile eines
                    // Prozents.
                    assert!(
                        gw <= w + w * 0.002 + 1.5,
                        "{} / {:?}: Schrift {gw:.1} mm breit, Rahmen nur {w:.1} mm",
                        font.key(),
                        text
                    );
                    // egui kernt die Zeichen (Paarweise enger); Kern und PDF
                    // setzen die reinen Vorschübe. Der Rahmen nimmt die
                    // Vorschübe und ist damit nie zu schmal — höchstens
                    // ein paar Prozent Luft.
                    assert!(
                        w - gw <= w * 0.03 + 1.5,
                        "{} / {:?}: Rahmen {w:.1} mm leer, Schrift {gw:.1} mm breit",
                        font.key(),
                        text
                    );
                    geprueft += 1;
                }
            }
        }
        assert_eq!(geprueft, TextFont::ALL.len() * 4 * 2);
    }

    /// Gegenprobe zum alten festen Zeilenmaß (1,4 für den Rahmen): Serifenlos
    /// setzt 1,150 Em Zeilenhöhe, DIN 1451 1,086 Em — der Rahmen darf für
    /// beide nicht derselbe sein, und beide müssen zur gesetzten Schrift passen.
    #[test]
    fn zeilenhoehe_folgt_der_schriftart() {
        let ctx = ctx();
        let hoehe = |font: TextFont| {
            let t = prim("A\nB", font, 1.0);
            let size = (t.height * render::TEXT_GROESSE_FAKTOR) as f32;
            let g = satz(&ctx, &t, size, Color32::WHITE);
            (t.box_size().1, g.size().y as f64)
        };
        let (rahmen_sans, schrift_sans) = hoehe(TextFont::Sans);
        let (rahmen_din, schrift_din) = hoehe(TextFont::DinMittel);
        for (name, rahmen, schrift) in [
            ("Serifenlos", rahmen_sans, schrift_sans),
            ("DIN 1451 Mittel", rahmen_din, schrift_din),
        ] {
            assert!(
                (rahmen - schrift).abs() <= 5.0,
                "{name}: Rahmen {rahmen:.1} gegen Schrift {schrift:.1}"
            );
        }
        // Die feste Annahme 1,4 war für beide falsch; die Schriften sind
        // untereinander verschieden hoch.
        assert!(
            (rahmen_sans - rahmen_din).abs() > 5.0,
            "Serifenlos {rahmen_sans:.1} und DIN 1451 {rahmen_din:.1} sind gleich hoch"
        );
    }
}

#[cfg(test)]
mod tests_textzoom {
    //! T0114: Schrift im hohen Zoom — Rasterdeckel, Größenleiter und Atlas.
    //!
    //! Gemessen wird über `canvas::text_galley`, also genau den Weg, den die
    //! Zeichenfläche geht (Satz + Streckung). Die Gegenprobe ist
    //! `textgroesse::raster_und_skalierung` auf die alte Rasterung
    //! zurückzusetzen: dann fällt `zoomlauf_haelt_den_schriftatlas_klein` und
    //! `text_waechst_ueber_620_prozent_hinaus` (Atlas läuft über, Galley bleibt
    //! bei 2048 px stehen).

    use super::*;

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let _ = ctx.run(Default::default(), |_| {});
        ctx
    }

    fn prim(text: &str, height: f64) -> render::TextPrim {
        render::TextPrim {
            pos: V2::ZERO,
            text: text.to_string(),
            height,
            angle: 0.0,
            align: TextAlignH::Left,
            valign: TextAlignV::Middle,
            bold: false,
            italic: false,
            underline: false,
            superscript: false,
            runs: Vec::new(),
            font: jjcad_core::model::TextFont::default(),
            line_spacing: 1.0,
        }
    }

    fn maler(ctx: &egui::Context) -> egui::Painter {
        egui::Painter::new(
            ctx.clone(),
            egui::LayerId::new(egui::Order::Background, egui::Id::new("textzoom")),
            Rect::EVERYTHING,
        )
    }

    /// Die gezeichnete Schrift wächst über den 620-%-Deckel hinaus.
    ///
    /// Vorher deckelte epaint die Rastergröße hart auf 2048 px: bei 250 mm Text
    /// war das 620 % Zoom, darüber blieb die Galley stehen. Jetzt wächst die
    /// Streckung weiter.
    #[test]
    fn text_waechst_ueber_620_prozent_hinaus() {
        let ctx = ctx();
        let p = maler(&ctx);
        let t = prim("Wohnen 12,4", 250.0);
        let hoehe = |zoom: f64| text_galley(&p, &t, zoom, Color32::WHITE, crate::textgroesse::RASTER_DECKEL_PX).size().y;
        let (h6, h10, h20) = (hoehe(6.0), hoehe(10.0), hoehe(20.0));
        assert!(h6 > 1000.0, "600 %: nur {h6} px hoch");
        assert!(
            h10 > h6 * 1.5,
            "1000 %: {h10} px gegen {h6} px bei 600 % — die Schrift wächst nicht"
        );
        assert!(
            h20 > h10 * 1.5,
            "2000 %: {h20} px gegen {h10} px bei 1000 % — die Schrift wächst nicht"
        );
    }

    /// Der Schriftatlas bleibt beim Zoomen klein.
    ///
    /// Ein Zoomlauf über den ganzen Bereich mit vielen Texten darf ihn höchstens
    /// zur Hälfte füllen; vorher lief er über (fill_ratio 1,0) und egui baute
    /// ihn mit einer 268-MB-Kopie neu.
    #[test]
    fn zoomlauf_haelt_den_schriftatlas_klein() {
        let ctx = ctx();
        let p = maler(&ctx);
        let mut fuellstand = 0.0_f32;
        for zoom in [
            0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0, 15.0, 20.0,
        ] {
            for i in 0..64 {
                let t = prim(&format!("Wohnen {i} 12,4 m²"), 250.0);
                let _ = text_galley(&p, &t, zoom, Color32::WHITE, crate::textgroesse::RASTER_DECKEL_PX);
            }
            fuellstand = fuellstand.max(ctx.fonts(|f| f.font_atlas_fill_ratio()));
        }
        assert!(
            fuellstand < 0.5,
            "Schriftatlas zu {:.0} % voll",
            fuellstand * 100.0
        );
    }

    /// Die gestreckte Galley ist so groß wie der Rahmen — der Deckel verkleinert
    /// die Darstellung nicht, er verschiebt nur die Rasterung.
    #[test]
    fn gestreckte_schrift_bleibt_so_gross_wie_der_rahmen() {
        let ctx = ctx();
        let p = maler(&ctx);
        // Kleine Höhen (2,5 mm) sind bei 1 px/mm nur 3 px groß — dort rundet
        // egui je Zeichen auf ganze Bildpunkte, und der Vergleich wäre eine
        // Aussage über die Rundung, nicht über den Deckel.
        for height in [40.0, 250.0, 1000.0] {
            let t = prim("Wohnen 12,4", height);
            let g = text_galley(&p, &t, 1.0, Color32::WHITE, crate::textgroesse::RASTER_DECKEL_PX);
            let (w, h) = t.box_size();
            let (gw, gh) = (g.size().x as f64, g.size().y as f64);
            assert!(
                (gh - h).abs() <= h * 0.02 + 1.0,
                "{height} mm: Schrift {gh:.1} hoch, Rahmen {h:.1}"
            );
            assert!(
                (gw - w).abs() <= w * 0.03 + 1.0,
                "{height} mm: Schrift {gw:.1} breit, Rahmen {w:.1}"
            );
        }
    }
}

#[cfg(test)]
mod tests_mittelpunkt {
    //! T0117: Der Mittelpunkt von Kreis, Bogen und Ellipse ist im Plan **nicht**
    //! gezeichnet — er erscheint nur bei Auswahl. Gefangen wird er trotzdem;
    //! dann zeigt ihn ein Kreuz an der Fangstelle.

    use super::*;

    /// Zählt waagerechte/senkrechte Striche der Länge `laenge`, deren Mitte auf
    /// `c` liegt. Das Mittelpunktkreuz besteht aus genau zwei solchen Strichen.
    fn zaehle(shape: &egui::Shape, c: Pos2, laenge: f32, n: &mut usize) {
        match shape {
            egui::Shape::LineSegment { points, .. } => {
                let (a, b) = (points[0], points[1]);
                let mitte = a + (b - a) * 0.5;
                if mitte.distance(c) < 0.5 && ((b - a).length() - laenge).abs() < 0.01 {
                    *n += 1;
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| zaehle(s, c, laenge, n)),
            _ => {}
        }
    }

    fn striche_am_mittelpunkt(shape: PShape, ausgewaehlt: bool) -> usize {
        let ctx = egui::Context::default();
        let c = V2::new(500.0, 500.0);
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("t0117"));
            let p = egui::Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            let prim = Prim {
                shape: shape.clone(),
                style: render::PrimStyle::default(),
                entity: 7,
                layer: 1,
            };
            let pc = PaintCtx {
                selected: if ausgewaehlt { vec![7] } else { Vec::new() },
                hover: None,
                lineweights: false,
                lineweights_scaled: false,
                paper_px: 1.0,
                preview: false,
                rot: 0.0,
                text_deckel: 64.0,
            };
            let w2s = |q: V2| Pos2::new(q.x as f32, q.y as f32);
            paint_prim(&p, &prim, &w2s, 1.0, &pc, 1.0);
        });
        let mut n = 0;
        for cs in &out.shapes {
            zaehle(&cs.shape, Pos2::new(500.0, 500.0), 8.0, &mut n);
        }
        n
    }

    /// Ohne Auswahl zeichnet keine der vier Kurven ein Mittelpunktkreuz; mit
    /// Auswahl zeichnen alle vier es (zwei Striche).
    #[test]
    fn mittelpunktkreuz_nur_bei_auswahl() {
        let c = V2::new(500.0, 500.0);
        let formen = [
            ("Kreis", PShape::Circle { c, r: 200.0 }),
            (
                "Bogen",
                PShape::Arc {
                    c,
                    r: 200.0,
                    a0: 0.0,
                    a1: 1.5,
                },
            ),
            (
                "Ellipse",
                PShape::Ellipse {
                    c,
                    rx: 200.0,
                    ry: 100.0,
                    rot: 0.0,
                },
            ),
            (
                "Ellipsenbogen",
                PShape::EllipseArc {
                    c,
                    rx: 200.0,
                    ry: 100.0,
                    rot: 0.0,
                    a0: 0.0,
                    a1: 1.5,
                },
            ),
        ];
        for (name, form) in formen {
            assert_eq!(
                striche_am_mittelpunkt(form.clone(), false),
                0,
                "{name}: Mittelpunkt darf ohne Auswahl nicht gezeichnet werden"
            );
            assert_eq!(
                striche_am_mittelpunkt(form, true),
                2,
                "{name}: bei Auswahl fehlt das Mittelpunktkreuz"
            );
        }
    }

    fn frame_out(app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = app.ctx.clone();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1500.0, 950.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| app.ui(ctx))
    }

    /// Der unsichtbare Mittelpunkt bleibt Fangziel: steht der Zeiger auf ihm,
    /// meldet die Statuszeile „Zentrum" und die Zeichenfläche zeigt das Kreuz
    /// (Halbbreite 7 px, also 14 px lange Striche).
    #[test]
    fn gefangener_mittelpunkt_wird_gezeigt() {
        let mut app = App::with_ctx(egui::Context::default());
        app.session_mut()
            .execute("doc.settings", &serde_json::json!({"input_unit": "mm"}))
            .expect("Einheit");
        frame_out(&mut app, vec![]);
        let rect = app.ui.last_canvas;
        let view = app.current_view();
        let mitte = Pos2::new(rect.center().x, rect.center().y);
        let w = screen_to_world(rect, view, mitte);
        app.session_mut()
            .execute(
                "draw.circle",
                &serde_json::json!({"c": [w.x, w.y], "r": 1000.0}),
            )
            .expect("Kreis");

        // Zeiger genau auf den Mittelpunkt: der Fang greift.
        let out = frame_out(&mut app, vec![egui::Event::PointerMoved(mitte)]);
        assert_eq!(
            app.snap_label.as_deref(),
            Some("Zentrum"),
            "der Mittelpunkt muss weiter fangen"
        );
        let mut n = 0;
        for cs in &out.shapes {
            zaehle(&cs.shape, mitte, 14.0, &mut n);
        }
        assert_eq!(n, 2, "das Mittelpunktkreuz fehlt am gefangenen Zentrum");

        // Zeiger weit weg: kein Fang, kein Kreuz.
        let weg = Pos2::new(rect.left() + 20.0, rect.top() + 20.0);
        let out = frame_out(&mut app, vec![egui::Event::PointerMoved(weg)]);
        let mut n = 0;
        for cs in &out.shapes {
            zaehle(&cs.shape, mitte, 14.0, &mut n);
        }
        assert_eq!(n, 0, "ohne Fang darf kein Mittelpunktkreuz stehen");
    }
}
