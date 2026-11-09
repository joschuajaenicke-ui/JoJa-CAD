//! Headless-Rendering nach PNG (ohne GUI).
//!
//! Nutzt tiny-skia für Vektoren und ab_glyph für Text. Damit kann der MCP-Server
//! Bilder einer Zeichnung liefern, ohne dass ein Fenster geöffnet wird.

use crate::geom::*;
use crate::model::*;
use crate::render::{DisplayList, Prim, Shape};
use crate::style::LineType;
use crate::tessellate::CurveQuality;
use crate::export::RasterOptions;
use ab_glyph::{Font, FontRef};
use std::path::Path;
use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Paint, PathBuilder, Pattern, Pixmap, PixmapPaint,
    SpreadMode, Stroke, StrokeDash, Transform,
};

struct Map {
    /// Pixel pro Welt-Millimeter
    k: f64,
    ox: f64,
    oy: f64,
}

impl Map {
    fn p(&self, p: Vec2) -> (f32, f32) {
        (
            (self.ox + p.x * self.k) as f32,
            (self.oy - p.y * self.k) as f32,
        )
    }
}

fn color_of(c: crate::style::Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, 255)
}

fn build_path(pts: &[Vec2], closed: bool, map: &Map) -> Option<tiny_skia::Path> {
    if pts.len() < 2 {
        return None;
    }
    let mut pb = PathBuilder::new();
    let (fx, fy) = map.p(pts[0]);
    pb.move_to(fx, fy);
    for p in &pts[1..] {
        let (qx, qy) = map.p(*p);
        pb.line_to(qx, qy);
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

/// Pfad aus mehreren geschlossenen Ringen (Außenkontur plus Löcher).
fn build_rings_path(
    outer: &[Vec2],
    holes: &[Vec<Vec2>],
    map: &Map,
) -> Option<tiny_skia::Path> {
    if outer.len() < 3 {
        return None;
    }
    let mut pb = PathBuilder::new();
    for ring in std::iter::once(outer).chain(holes.iter().map(|h| h.as_slice())) {
        if ring.len() < 3 {
            continue;
        }
        let (fx, fy) = map.p(ring[0]);
        pb.move_to(fx, fy);
        for p in &ring[1..] {
            let (qx, qy) = map.p(*p);
            pb.line_to(qx, qy);
        }
        pb.close();
    }
    pb.finish()
}

fn dash_for(lt: LineType, paper_px: f64) -> Option<StrokeDash> {
    let pattern: Vec<f32> = lt
        .pattern()
        .iter()
        .map(|v| (*v * paper_px) as f32)
        .filter(|v| *v > 0.05)
        .collect();
    if pattern.len() < 2 {
        None
    } else {
        StrokeDash::new(pattern, 0.0)
    }
}

fn draw_text(pm: &mut Pixmap, t: &crate::render::TextPrim, prim: &Prim, map: &Map) {
    let size_px = (t.height * map.k) as f32;
    if size_px < 1.5 {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(color_of(prim.style.color));
    paint.anti_alias = true;

    // Satz mit Läufen: dieselbe Zerlegung wie im PDF, damit beide Ausgaben
    // dieselben Vorschübe benutzen.
    let runs = t.effective_runs();
    let lines = crate::text::layout_mit_schrift(&t.text, &runs, t.font);
    // Zeilenhöhe der Familie (früher fest 1,25): je Schrift verschieden, wie im
    // Rahmen. Ohne das überlappen Zeilen einer Familie mit großem Durchschuss.
    let zeilenhoehe_em = t.line_height_em() as f32;
    let line_h = size_px * zeilenhoehe_em * t.line_spacing.clamp(0.1, 10.0) as f32;
    let (cos, sin) = (t.angle.cos() as f32, t.angle.sin() as f32);
    let bx = map.ox + t.pos.x * map.k;
    let by = map.oy - t.pos.y * map.k;

    // Senkrechte Lage gilt für den ganzen Block (wie im PDF und in der Anzeige).
    let n = lines.len() as f32;
    let dy_erste = match t.valign {
        TextAlignV::Top => -size_px * 0.8,
        TextAlignV::Middle => -size_px * 0.36 + (n - 1.0) * line_h * 0.5,
        TextAlignV::Bottom => (n - 1.0) * line_h,
    };
    for (li, line) in lines.iter().enumerate() {
        let w = line.width_em as f32 * size_px;
        let dx0 = match t.align {
            TextAlignH::Left => 0.0,
            TextAlignH::Center => -w * 0.5,
            TextAlignH::Right => -w,
        };
        let dy_line = dy_erste - li as f32 * line_h;
        let mut pen = dx0;
        for seg in &line.segs {
            if seg.text.is_empty() {
                continue;
            }
            let Some(data) = crate::fonts::ttf_bytes(t.font, seg.style) else {
                continue;
            };
            let Ok(font) = FontRef::try_from_slice(data) else {
                continue;
            };
            let seg_size = size_px * seg.scale as f32;
            let upem = font.units_per_em().unwrap_or(1000.0);
            let unit = seg_size / upem;
            let dy0 = dy_line + seg.rise_em as f32 * size_px;
            let pen_start = pen;
            for ch in seg.text.chars() {
                let gid = font.glyph_id(ch);
                if ch != ' ' {
                    if let Some(outline) = font.outline(gid) {
                        let mut pb = PathBuilder::new();
                        let mut prev_end: Option<ab_glyph::Point> = None;
                        for curve in &outline.curves {
                            let tf = |p: ab_glyph::Point| -> (f32, f32) {
                                let lx = pen + p.x * unit;
                                let ly = dy0 + p.y * unit;
                                let rx = lx * cos - ly * sin;
                                let ry = lx * sin + ly * cos;
                                ((bx + rx as f64) as f32, (by - ry as f64) as f32)
                            };
                            let (startp, endp) = match curve {
                                ab_glyph::OutlineCurve::Line(p, b) => (*p, *b),
                                ab_glyph::OutlineCurve::Quad(p, _, b) => (*p, *b),
                                ab_glyph::OutlineCurve::Cubic(p, _, _, b) => (*p, *b),
                            };
                            // Neuer Teilpfad, wenn der vorherige nicht hier endet
                            let new_contour = match prev_end {
                                None => true,
                                Some(e) => {
                                    (e.x - startp.x).abs() > 0.01 || (e.y - startp.y).abs() > 0.01
                                }
                            };
                            let (sx, sy) = tf(startp);
                            if new_contour {
                                pb.move_to(sx, sy);
                            }
                            prev_end = Some(endp);
                            match curve {
                                ab_glyph::OutlineCurve::Line(_, b) => {
                                    let (ex, ey) = tf(*b);
                                    pb.line_to(ex, ey);
                                }
                                ab_glyph::OutlineCurve::Quad(_, c, b) => {
                                    let (cx, cy) = tf(*c);
                                    let (ex, ey) = tf(*b);
                                    pb.quad_to(cx, cy, ex, ey);
                                }
                                ab_glyph::OutlineCurve::Cubic(_, c1, c2, b) => {
                                    let (x1, y1) = tf(*c1);
                                    let (x2, y2) = tf(*c2);
                                    let (ex, ey) = tf(*b);
                                    pb.cubic_to(x1, y1, x2, y2, ex, ey);
                                }
                            }
                        }
                        if let Some(path) = pb.finish() {
                            pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
                        }
                    }
                }
                // Vorschub in Em-Einheiten der Datei (dieselbe Umrechnung wie
                // die Umrisse: `unit = seg_size / units_per_em`). `as_scaled`
                // bezöge ihn auf Aufstieg − Abstieg, dann stünden die Zeichen
                // 12 % enger als ihre eigenen Umrisse (T0110).
                pen += font.h_advance_unscaled(gid) as f32 * unit;
            }
            if seg.underline {
                // Unterstreichung: kurzer Strich unter der Grundlinie.
                let dick = (seg_size * 0.06).max(0.4);
                let yy = dy_line - seg_size * 0.12;
                let tf = |lx: f32, ly: f32| -> (f32, f32) {
                    let rx = lx * cos - ly * sin;
                    let ry = lx * sin + ly * cos;
                    ((bx + rx as f64) as f32, (by - ry as f64) as f32)
                };
                let a = tf(pen_start, yy);
                let b = tf(pen, yy);
                let mut pb = PathBuilder::new();
                pb.move_to(a.0, a.1);
                pb.line_to(b.0, b.1);
                if let Some(path) = pb.finish() {
                    let mut st = Stroke::default();
                    st.width = dick;
                    st.line_cap = LineCap::Butt;
                    pm.stroke_path(&path, &paint, &st, Transform::identity(), None);
                }
            }
        }
    }
}

fn draw_prim(pm: &mut Pixmap, prim: &Prim, map: &Map, paper_px: f64) {
    let width_px = (prim.style.width * paper_px).max(0.6) as f32;
    let mut stroke = Stroke::default();
    stroke.width = width_px;
    stroke.line_cap = LineCap::Round;
    stroke.line_join = LineJoin::Round;
    stroke.dash = dash_for(prim.style.line_type, paper_px);
    let mut paint = Paint::default();
    paint.set_color(color_of(prim.style.color));
    paint.anti_alias = true;

    match &prim.shape {
        Shape::Line(a, b) => {
            if let Some(path) = build_path(&[*a, *b], false, map) {
                pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        Shape::Poly(pts, closed) => {
            if let Some(path) = build_path(pts, *closed, map) {
                pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        Shape::Fill { outer, holes } | Shape::FillOrdered { outer, holes } => {
            // Alle Ringe als Unterpfade einer Fläche, gefüllt nach Even-Odd:
            // die Löcher sind dann echte Aussparungen (der Untergrund bleibt
            // sichtbar) und nicht mit Hintergrundfarbe übermalt.
            if let Some(path) = build_rings_path(outer, holes, map) {
                let mut fp = Paint::default();
                fp.set_color(color_of(prim.style.fill.unwrap_or(prim.style.color)));
                fp.anti_alias = true;
                pm.fill_path(&path, &fp, FillRule::EvenOdd, Transform::identity(), None);
            }
        }
        Shape::Circle { c, r } => {
            let rp = (*r * map.k) as f32;
            if rp > 0.4 {
                let (cxp, cyp) = map.p(*c);
                if let Some(path) = PathBuilder::from_circle(cxp, cyp, rp) {
                    // Ein gefüllter Kreis (Musterpunkt beim Zerlegen, T0138)
                    // wird als Scheibe geschrieben, sonst als Ring.
                    if let Some(fill) = prim.style.fill {
                        let mut fp = Paint::default();
                        fp.set_color(color_of(fill));
                        fp.anti_alias = true;
                        pm.fill_path(&path, &fp, FillRule::Winding, Transform::identity(), None);
                    } else {
                        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
            }
        }
        Shape::Arc { c, r, a0, a1 } => {
            let arc = Arc::new(*c, *r, *a0, *a1);
            let pts = CurveQuality::PRINT.flatten(&arc, map.k);
            if let Some(path) = build_path(&pts, false, map) {
                pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        Shape::Ellipse { c, rx, ry, rot } => {
            let n = 72;
            let pts: Vec<Vec2> = (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64 * std::f64::consts::TAU;
                    *c + Vec2::new(rx * t.cos(), ry * t.sin()).rotated(*rot)
                })
                .collect();
            if let Some(path) = build_path(&pts, true, map) {
                pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        Shape::EllipseArc {
            c,
            rx,
            ry,
            rot,
            a0,
            a1,
        } => {
            let pts = crate::geom::Ellipse::new(*c, *rx, *ry, *rot).arc_points(*a0, *a1, 0.1);
            if let Some(path) = build_path(&pts, false, map) {
                pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        Shape::Dot { p, r } => {
            let rp = (*r * map.k) as f32;
            if rp > 0.3 {
                let (dpx, dpy) = map.p(*p);
                if let Some(path) = PathBuilder::from_circle(dpx, dpy, rp) {
                    pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
                }
            }
        }
        Shape::Text(t) => draw_text(pm, t, prim, map),
        Shape::Image { a, b, hoehe, bild } => draw_bild(pm, *a, *b, *hoehe, bild, map),
    }
}

/// Bild über seiner unteren Kante `a`→`b` zeichnen.
///
/// tiny-skia erwartet **vormultipliziertes** RGBA; die zerlegten Bilddaten
/// tragen geradlinige Deckung. Gezeichnet wird über ein Muster (Pattern), das
/// die Bildpixel über eine Matrix auf das gedrehte Viereck abbildet — dieselbe
/// Lage, die Anzeige und PDF benutzen.
fn draw_bild(
    pm: &mut Pixmap,
    a: Vec2,
    b: Vec2,
    hoehe: f64,
    bild: &crate::bild::Bild,
    map: &Map,
) {
    if hoehe <= 0.0 || bild.breite == 0 || bild.hoehe == 0 {
        return;
    }
    let ecken = crate::bild::ecken(a, b, hoehe);
    // Pixelkoordinaten: map.p dreht die Y-Achse. ecken[0] ist unten links,
    // [1] unten rechts, [3] oben links.
    let (ux, uy) = map.p(ecken[0]);
    let (rx, ry) = map.p(ecken[1]);
    let (ox, oy) = map.p(ecken[3]);
    let (trx, try_) = map.p(ecken[2]);
    let breite_px = (rx - ux).hypot(ry - uy);
    let hoehe_px = (ox - ux).hypot(oy - uy);
    if !breite_px.is_finite() || !hoehe_px.is_finite() || breite_px <= 0.5 || hoehe_px <= 0.5 {
        return;
    }
    let Some(mut quelle) = Pixmap::new(bild.breite, bild.hoehe) else {
        return;
    };
    for (dst, src) in quelle.data_mut().chunks_exact_mut(4).zip(bild.rgba.chunks_exact(4)) {
        let al = src[3] as u32;
        dst[0] = ((src[0] as u32 * al) / 255) as u8;
        dst[1] = ((src[1] as u32 * al) / 255) as u8;
        dst[2] = ((src[2] as u32 * al) / 255) as u8;
        dst[3] = src[3];
    }
    // Bildpixel (0,0) ist oben links: die Matrix legt die x-Achse auf die
    // untere Kante und die y-Achse von oben nach unten.
    let sx = ((rx - ux) / bild.breite as f32, (ry - uy) / bild.breite as f32);
    let sy = ((ux - ox) / bild.hoehe as f32, (uy - oy) / bild.hoehe as f32);
    let matrix = Transform::from_row(sx.0, sx.1, sy.0, sy.1, ox, oy);
    let mut paint = Paint::default();
    paint.shader = Pattern::new(
        quelle.as_ref(),
        SpreadMode::Pad,
        FilterQuality::Bilinear,
        1.0,
        matrix,
    );
    paint.anti_alias = true;
    let mut pb = PathBuilder::new();
    pb.move_to(ux, uy);
    pb.line_to(rx, ry);
    pb.line_to(trx, try_);
    pb.line_to(ox, oy);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_dl(pm: &mut Pixmap, dl: &DisplayList, map: &Map, paper_px: f64) {
    for prim in &dl.prims {
        draw_prim(pm, prim, map, paper_px);
    }
}

/// Rendert das Modell in ein Pixmap.
pub fn render_model_pixmap(doc: &Document, opts: &RasterOptions) -> Result<Pixmap, String> {
    let mut bbox = match opts.bbox {
        Some([x0, y0, x1, y1]) => BBox::new(v(x0, y0), v(x1, y1)),
        None => doc.bbox(),
    };
    if !bbox.is_valid() || bbox.width() < 1e-6 || bbox.height() < 1e-6 {
        bbox = BBox::new(v(-5000.0, -5000.0), v(5000.0, 5000.0));
    }
    bbox = bbox.expanded(bbox.width().max(bbox.height()) * 0.03);
    let (w, h) = (opts.width_px.max(16), opts.height_px.max(16));
    let mut pm = Pixmap::new(w, h).ok_or_else(|| "Bild konnte nicht angelegt werden".to_string())?;
    pm.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));

    let k = (w as f64 / bbox.width()).min(h as f64 / bbox.height());
    let map = Map {
        k,
        ox: -bbox.min.x * k + (w as f64 - bbox.width() * k) * 0.5,
        oy: bbox.max.y * k + (h as f64 - bbox.height() * k) * 0.5,
    };
    let annot = opts.annot_scale.unwrap_or(doc.settings.annotation_scale);
    let ctx = crate::render::RenderCtx::model_scaled(doc, annot);
    let dl = crate::render::render_document(&ctx);
    // Papier-mm pro Pixel
    let paper_px = k * annot;
    draw_dl(&mut pm, &dl, &map, paper_px);
    Ok(pm)
}

/// Rendert ein Blatt (Papier + Ansichtsfenster) in ein Pixmap.
pub fn render_sheet_pixmap(doc: &Document, sheet: &Sheet, opts: &RasterOptions) -> Result<Pixmap, String> {
    let (pw, ph) = sheet.size();
    let (w, h) = (opts.width_px.max(16), opts.height_px.max(16));
    let mut pm = Pixmap::new(w, h).ok_or_else(|| "Bild konnte nicht angelegt werden".to_string())?;
    pm.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
    let k = (w as f64 / pw).min(h as f64 / ph);
    let map = Map {
        k,
        ox: (w as f64 - pw * k) * 0.5,
        oy: (h as f64 - ph * k) * 0.5 + ph * k,
    };
    let dl = crate::render::render_sheet(doc, sheet);
    draw_dl(&mut pm, &dl, &map, k);

    for (_, vp) in sheet.viewports() {
        let sub_w = (vp.w * k).round().max(1.0) as u32;
        let sub_h = (vp.h * k).round().max(1.0) as u32;
        let Some(mut sub) = Pixmap::new(sub_w, sub_h) else {
            continue;
        };
        let s = 1.0 / vp.scale.max(1e-9);
        let km = sub_w as f64 / vp.w; // px pro Papier-mm im Fenster
        let sub_map = Map {
            k: km * s,
            ox: (vp.w * 0.5 - vp.center.x * s) * km,
            oy: (vp.h * 0.5 + vp.center.y * s) * km,
        };
        let vl = crate::render::render_viewport(doc, vp);
        draw_dl(&mut sub, &vl, &sub_map, km);
        let px = (vp.x * k + map.ox) as i32;
        let py = ((ph - vp.y - vp.h) * k + (map.oy - ph * k)) as i32;
        pm.draw_pixmap(
            px,
            py,
            sub.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    Ok(pm)
}

/// PNG-Datei erzeugen.
pub fn render_png(doc: &Document, opts: &RasterOptions, path: &Path) -> Result<(), String> {
    let pm = match opts.sheet {
        Some(idx) => {
            let sheet = doc
                .sheets
                .get(idx)
                .ok_or_else(|| format!("Blatt {} existiert nicht", idx + 1))?;
            render_sheet_pixmap(doc, sheet, opts)?
        }
        None => render_model_pixmap(doc, opts)?,
    };
    let png = pm
        .encode_png()
        .map_err(|e| format!("PNG-Kodierung fehlgeschlagen: {e}"))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    std::fs::write(path, png).map_err(|e| format!("PNG schreiben fehlgeschlagen: {e}"))
}

/// PNG-Bytes (für MCP-Antworten).
pub fn png_bytes(doc: &Document, opts: &RasterOptions) -> Result<Vec<u8>, String> {
    let pm = match opts.sheet {
        Some(idx) => {
            let sheet = doc
                .sheets
                .get(idx)
                .ok_or_else(|| format!("Blatt {} existiert nicht", idx + 1))?;
            render_sheet_pixmap(doc, sheet, opts)?
        }
        None => render_model_pixmap(doc, opts)?,
    };
    pm.encode_png()
        .map_err(|e| format!("PNG-Kodierung fehlgeschlagen: {e}"))
}

/// PNG-Bytes aus einem fertigen RGBA-Bild (Bildschirmfotos der Oberfläche).
///
/// Der Aufrufer liefert die Pixel in der Reihenfolge R,G,B,A je Punkt, wie sie
/// aus dem Framebuffer kommen; hier landen sie unverändert in einem PNG.
pub fn rgba_png(breite: u32, hoehe: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut pm = Pixmap::new(breite, hoehe)
        .ok_or_else(|| "Bild konnte nicht angelegt werden".to_string())?;
    let ziel = pm.data_mut();
    if rgba.len() != ziel.len() {
        return Err(format!(
            "Bildgröße passt nicht: {} Bytes für {}x{}",
            rgba.len(),
            breite,
            hoehe
        ));
    }
    ziel.copy_from_slice(rgba);
    pm.encode_png()
        .map_err(|e| format!("PNG-Kodierung fehlgeschlagen: {e}"))
}

/// Blockvorschau als PNG (für Blockmanager, sofern kein Live-Rendering genutzt wird).
pub fn block_thumbnail(doc: &Document, block: &BlockDef, px: u32) -> Result<Vec<u8>, String> {
    let bbox = block.bbox();
    if !bbox.is_valid() {
        return Err("Block ist leer".into());
    }
    let b = bbox.expanded(bbox.width().max(bbox.height()) * 0.08);
    let k = (px as f64 / b.width()).min(px as f64 / b.height());
    let mut pm = Pixmap::new(px, px).ok_or_else(|| "Bild konnte nicht angelegt werden".to_string())?;
    pm.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0));
    let map = Map {
        k,
        ox: -b.min.x * k + (px as f64 - b.width() * k) * 0.5,
        oy: b.max.y * k + (px as f64 - b.height() * k) * 0.5,
    };
    let mut dl = DisplayList::default();
    let ctx = crate::render::RenderCtx::new(doc);
    for e in &block.entities {
        crate::render::render_entity(&ctx, e, Affine::IDENTITY, &mut dl, 0);
    }
    draw_dl(&mut pm, &dl, &map, k);
    pm.encode_png()
        .map_err(|e| format!("PNG-Kodierung fehlgeschlagen: {e}"))
}

/// Prüft, ob Text rendern möglich ist (Schrift gefunden).
pub fn has_font() -> bool {
    crate::fonts::ttf_bytes(crate::model::TextFont::Sans, crate::fonts::FontStyle::Regular).is_some()
}
