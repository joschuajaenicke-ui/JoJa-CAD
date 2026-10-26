//! PDF-Export (eigener, abhängigkeitsfreier PDF-Schreiber).
//!
//! Erzeugt vektorbasierte PDFs mit Helvetica-Text — geeignet für Bauanträge.
//! Ein Blatt wird als Seite ausgegeben, Ansichtsfenster werden beschnitten.

use crate::geom::*;
use crate::model::*;
use crate::render::{self, DisplayList, Prim, Shape, TextPrim};
use crate::style::{Color, LineType, PaperSize};
use crate::tessellate::CurveQuality;
use std::fmt::Write as _;
use std::path::Path;

const MM2PT: f64 = 72.0 / 25.4;

// ---------------------------------------------------------------------------
// Inhaltsstrom
// ---------------------------------------------------------------------------

struct Content {
    buf: String,
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 32 => {}
            // WinAnsi: Zeichen > 127 als Oktal-Escape (1 Byte im PDF-String)
            c if (c as u32) < 256 => {
                let b = c as u32;
                if b < 127 {
                    out.push(c);
                } else {
                    out.push_str(&format!("\\{b:03o}"));
                }
            }
            _ => out.push('?'),
        }
    }
    out
}

/// Zeichenbreiten Helvetica (1/1000 em) für ASCII 32..126.
const HELV_W: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

fn char_width(c: char) -> f64 {
    let code = c as u32;
    if (32..127).contains(&code) {
        HELV_W[(code - 32) as usize] as f64 / 1000.0
    } else if code == 0xF6 || code == 0xFC || code == 0xE4 || code == 0xDF {
        0.556
    } else {
        0.6
    }
}

pub fn text_width(text: &str, size_pt: f64) -> f64 {
    text.chars().map(char_width).sum::<f64>() * size_pt
}

struct Page {
    width_pt: f64,
    height_pt: f64,
    content: Content,
}

impl Content {
    fn new() -> Self {
        Self {
            buf: String::with_capacity(8192),
        }
    }

    fn raw(&mut self, s: &str) {
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    fn save(&mut self) {
        self.buf.push_str("q\n");
    }
    fn restore(&mut self) {
        self.buf.push_str("Q\n");
    }
    fn stroke_color(&mut self, c: Color) {
        let s = format!("{:.4} {:.4} {:.4} RG", c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
        self.raw(&s);
    }
    fn fill_color(&mut self, c: Color) {
        let s = format!("{:.4} {:.4} {:.4} rg", c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
        self.raw(&s);
    }
    fn line_width(&mut self, mm: f64) {
        let s = format!("{:.3} w", (mm * MM2PT).max(0.05));
        self.raw(&s);
    }
    fn dash(&mut self, lt: LineType) {
        let p = lt.pattern();
        if p.is_empty() {
            self.raw("[] 0 d");
        } else {
            let vals: Vec<String> = p
                .iter()
                .map(|v| format!("{:.2}", v * MM2PT))
                .collect();
            let s = format!("[{}] 0 d", vals.join(" "));
            self.raw(&s);
        }
    }

    fn move_to(&mut self, p: Vec2) {
        let s = format!("{:.3} {:.3} m", p.x * MM2PT, p.y * MM2PT);
        self.raw(&s);
    }
    fn line_to(&mut self, p: Vec2) {
        let s = format!("{:.3} {:.3} l", p.x * MM2PT, p.y * MM2PT);
        self.raw(&s);
    }
    fn close(&mut self) {
        self.raw("h");
    }
    fn stroke(&mut self) {
        self.raw("S");
    }
    fn fill(&mut self) {
        self.raw("f");
    }

    /// Füllen nach der Even-Odd-Regel (`f*`) — nötig für Flächen mit Löchern:
    /// alle Ringe sind Unterpfade, jeder zweite Umschlag wird ausgespart.
    fn fill_even_odd(&mut self) {
        self.raw("f*");
    }
    fn fill_stroke(&mut self) {
        self.raw("B");
    }

    fn text(&mut self, t: &TextPrim, size_pt: f64) {
        if size_pt < 0.3 || t.text.is_empty() {
            return;
        }
        let lines: Vec<&str> = t.text.split('\n').collect();
        let lh = size_pt * 1.25;
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let w = text_width(line, size_pt);
            let dx = match t.align {
                TextAlignH::Left => 0.0,
                TextAlignH::Center => -w / 2.0,
                TextAlignH::Right => -w,
            };
            let dy = match t.valign {
                TextAlignV::Top => -size_pt * 0.8,
                TextAlignV::Middle => -size_pt * 0.36,
                TextAlignV::Bottom => 0.0,
            } - i as f64 * lh;
            let (s, c) = t.angle.sin_cos();
            let x = t.pos.x * MM2PT + dx * c - dy * s;
            let y = t.pos.y * MM2PT + dx * s + dy * c;
            let font = if t.bold { "/F2" } else { "/F1" };
            let s_str = format!(
                "BT {font} {:.2} Tf {:.5} {:.5} {:.5} {:.5} {:.3} {:.3} Tm ({}) Tj ET",
                size_pt, c, s, -s, c, x, y, esc(line)
            );
            self.raw(&s_str);
        }
    }

    fn clip_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let s = format!(
            "{:.3} {:.3} {:.3} {:.3} re W n",
            x * MM2PT,
            y * MM2PT,
            w * MM2PT,
            h * MM2PT
        );
        self.raw(&s);
    }
}

// ---------------------------------------------------------------------------
// Primitive → PDF
// ---------------------------------------------------------------------------

struct Xf {
    scale: f64,
    offset: Vec2,
}

impl Xf {
    fn map(&self, p: Vec2) -> Vec2 {
        self.offset + p * self.scale
    }
}

fn push_polyline(c: &mut Content, pts: &[Vec2], closed: bool, xf: &Xf) {
    if pts.len() < 2 {
        return;
    }
    c.move_to(xf.map(pts[0]));
    for p in &pts[1..] {
        c.line_to(xf.map(*p));
    }
    if closed {
        c.close();
    }
}

fn draw_prim(c: &mut Content, prim: &Prim, xf: &Xf, paper_scale: f64) {
    match &prim.shape {
        Shape::Line(a, b) => {
            c.stroke_color(prim.style.color);
            c.line_width(prim.style.width);
            c.dash(prim.style.line_type);
            c.move_to(xf.map(*a));
            c.line_to(xf.map(*b));
            c.stroke();
        }
        Shape::Poly(pts, closed) => {
            c.stroke_color(prim.style.color);
            c.line_width(prim.style.width);
            c.dash(prim.style.line_type);
            push_polyline(c, pts, *closed, xf);
            c.stroke();
        }
        Shape::Fill { outer, holes } => {
            if outer.len() < 3 {
                return;
            }
            c.fill_color(prim.style.fill.unwrap_or(prim.style.color));
            push_polyline(c, outer, true, xf);
            for h in holes {
                if h.len() >= 3 {
                    push_polyline(c, h, true, xf);
                }
            }
            // Even-Odd: die Löcher bleiben frei. Ein einfaches `f` würde die
            // Innenringe je nach Umlaufsinn mitfüllen.
            c.fill_even_odd();
        }
        Shape::Circle { c: cc, r } => {
            c.stroke_color(prim.style.color);
            c.line_width(prim.style.width);
            c.dash(prim.style.line_type);
            let k = 0.5522847498 * r;
            let p = |dx: f64, dy: f64| xf.map(*cc + v(dx, dy));
            let p0 = p(*r, 0.0);
            c.move_to(p0);
            let pts = [
                (p(*r, k), p(k, *r), p(0.0, *r)),
                (p(-k, *r), p(-*r, k), p(-*r, 0.0)),
                (p(-*r, -k), p(-k, -*r), p(0.0, -*r)),
                (p(k, -*r), p(*r, -k), p(*r, 0.0)),
            ];
            for (c1, c2, e) in pts {
                let s = format!(
                    "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c",
                    c1.x * MM2PT,
                    c1.y * MM2PT,
                    c2.x * MM2PT,
                    c2.y * MM2PT,
                    e.x * MM2PT,
                    e.y * MM2PT
                );
                c.raw(&s);
            }
            c.stroke();
        }
        Shape::Arc { c: cc, r, a0, a1 } => {
            c.stroke_color(prim.style.color);
            c.line_width(prim.style.width);
            c.dash(prim.style.line_type);
            let arc = Arc::new(*cc, *r, *a0, *a1);
            let pts = CurveQuality::PRINT.flatten(&arc, paper_scale);
            push_polyline(c, &pts, false, xf);
            c.stroke();
        }
        Shape::Ellipse { c: cc, rx, ry, rot } => {
            c.stroke_color(prim.style.color);
            c.line_width(prim.style.width);
            c.dash(prim.style.line_type);
            let n = 72;
            let pts: Vec<Vec2> = (0..=n)
                .map(|i| {
                    let t = i as f64 / n as f64 * std::f64::consts::TAU;
                    *cc + Vec2::new(rx * t.cos(), ry * t.sin()).rotated(*rot)
                })
                .collect();
            push_polyline(c, &pts, true, xf);
            c.stroke();
        }
        Shape::Dot { p, r } => {
            c.fill_color(prim.style.color);
            let rr = r * xf.scale;
            let k = 0.5522847498 * rr;
            let q = |dx: f64, dy: f64| xf.map(*p + v(dx, dy));
            c.move_to(q(rr, 0.0));
            for (c1, c2, e) in [
                (q(rr, k), q(k, rr), q(0.0, rr)),
                (q(-k, rr), q(-rr, k), q(-rr, 0.0)),
                (q(-rr, -k), q(-k, -rr), q(0.0, -rr)),
                (q(k, -rr), q(rr, -k), q(rr, 0.0)),
            ] {
                let s = format!(
                    "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c",
                    c1.x * MM2PT,
                    c1.y * MM2PT,
                    c2.x * MM2PT,
                    c2.y * MM2PT,
                    e.x * MM2PT,
                    e.y * MM2PT
                );
                c.raw(&s);
            }
            c.fill();
        }
        Shape::Text(t) => {
            c.fill_color(prim.style.color);
            let size_pt = t.height * xf.scale * MM2PT;
            let mut tt = t.clone();
            tt.pos = xf.map(t.pos);
            c.text(&tt, size_pt);
        }
    }
}

fn draw_display_list(c: &mut Content, dl: &DisplayList, xf: &Xf, paper_scale: f64) {
    for prim in &dl.prims {
        draw_prim(c, prim, xf, paper_scale);
    }
}

// ---------------------------------------------------------------------------
// PDF-Container
// ---------------------------------------------------------------------------

struct PdfWriter {
    objects: Vec<Vec<u8>>,
}

impl PdfWriter {
    fn new() -> Self {
        Self { objects: Vec::new() }
    }

    fn add(&mut self, body: Vec<u8>) -> usize {
        self.objects.push(body);
        self.objects.len()
    }

    fn set(&mut self, idx: usize, body: Vec<u8>) {
        self.objects[idx - 1] = body;
    }

    fn finish(self, root: usize, info: Option<usize>) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
        let mut offsets = Vec::new();
        for (i, obj) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(obj);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_pos = out.len();
        let n = self.objects.len() + 1;
        out.extend_from_slice(format!("xref\n0 {n}\n").as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        let mut trailer = format!("trailer\n<< /Size {n} /Root {root} 0 R");
        if let Some(i) = info {
            trailer.push_str(&format!(" /Info {i} 0 R"));
        }
        trailer.push_str(" >>\nstartxref\n");
        trailer.push_str(&format!("{xref_pos}\n%%EOF\n"));
        out.extend_from_slice(trailer.as_bytes());
        out
    }
}

fn page_object(w: f64, h: f64, contents: usize, font_reg: usize, font_bold: usize, parent: usize) -> Vec<u8> {
    format!(
        "<< /Type /Page /Parent {parent} 0 R /MediaBox [0 0 {:.3} {:.3}] \
         /Resources << /Font << /F1 {font_reg} 0 R /F2 {font_bold} 0 R >> >> \
         /Contents {contents} 0 R >>",
        w * MM2PT,
        h * MM2PT
    )
    .into_bytes()
}

fn stream_obj(data: &str) -> Vec<u8> {
    let mut v = format!("<< /Length {} >>\nstream\n", data.len()).into_bytes();
    v.extend_from_slice(data.as_bytes());
    v.extend_from_slice(b"\nendstream");
    v
}

fn write_pdf(pages: Vec<Page>, path: &Path, title: &str) -> Result<(), String> {
    let mut w = PdfWriter::new();
    let catalog_idx = w.add(vec![]);
    let pages_idx = w.add(vec![]);
    let font_reg = w.add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
    );
    let font_bold = w.add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    let mut kids = Vec::new();
    for p in pages {
        let content_idx = w.add(stream_obj(&p.content.buf));
        let page_idx = w.add(page_object(
            p.width_pt / MM2PT,
            p.height_pt / MM2PT,
            content_idx,
            font_reg,
            font_bold,
            pages_idx,
        ));
        kids.push(page_idx);
    }
    let kids_str: Vec<String> = kids.iter().map(|k| format!("{k} 0 R")).collect();
    w.set(
        pages_idx,
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids_str.join(" "), kids.len()).into_bytes(),
    );
    w.set(
        catalog_idx,
        format!("<< /Type /Catalog /Pages {pages_idx} 0 R >>").into_bytes(),
    );
    let info_idx = w.add(
        format!(
            "<< /Title ({}) /Producer (JoJa-CAD {}) /Creator (JoJa-CAD) >>",
            esc(title),
            crate::APP_VERSION
        )
        .into_bytes(),
    );
    let bytes = w.finish(catalog_idx, Some(info_idx));
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    std::fs::write(path, bytes).map_err(|e| format!("PDF schreiben fehlgeschlagen: {e}"))
}

// ---------------------------------------------------------------------------
// Blatt zeichnen
// ---------------------------------------------------------------------------

fn draw_sheet_page(doc: &Document, sheet: &Sheet, show_frame: bool) -> Page {
    let (pw, ph) = sheet.size();
    let mut c = Content::new();
    // Weißer Hintergrund
    c.fill_color(Color::rgb(255, 255, 255));
    c.raw(&format!("0 0 {:.3} {:.3} re f", pw * MM2PT, ph * MM2PT));

    // Papierraum (Schriftfeld, Rahmen)
    let mut dl = DisplayList::default();
    if sheet.show_title_block {
        dl.prims.extend(render::render_sheet(doc, sheet).prims);
    } else {
        // nur Rahmen
        let mut s2 = sheet.clone();
        s2.frame = show_frame;
        // render_sheet zeichnet Papierraum-Objekte immer; für Rahmen ohne Schriftfeld:
        let only_frame = render::render_sheet(doc, &Sheet { entities: vec![], ..s2 });
        dl.prims.extend(only_frame.prims);
    }
    let page_xf = Xf {
        scale: 1.0,
        offset: Vec2::ZERO,
    };
    draw_display_list(&mut c, &dl, &page_xf, 1.0);

    // Ansichtsfenster
    for vp in &sheet.viewports {
        let vl = render::render_viewport(doc, vp);
        c.save();
        c.clip_rect(vp.x, vp.y, vp.w, vp.h);
        let scale = 1.0 / vp.scale.max(1e-9);
        let offset = v(vp.x + vp.w * 0.5, vp.y + vp.h * 0.5) - vp.center * scale;
        let xf = Xf { scale, offset };
        draw_display_list(&mut c, &vl, &xf, scale);
        c.restore();
    }
    Page {
        width_pt: pw * MM2PT,
        height_pt: ph * MM2PT,
        content: c,
    }
}

fn nice_scale(need: f64) -> f64 {
    let list = [
        1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 75.0, 100.0, 125.0, 150.0, 200.0, 250.0, 500.0,
        1000.0,
    ];
    list.iter().copied().find(|s| *s >= need).unwrap_or(1000.0)
}

/// Modellzeichnung als eigene Seite (A3 quer oder passend).
pub fn export_model_pdf(doc: &Document, path: &Path, paper: PaperSize) -> Result<(), String> {
    let (pw, ph) = paper.dims(true);
    let bbox = doc.bbox();
    let margin = 15.0;
    let avail_w = pw - 2.0 * margin;
    let avail_h = ph - 2.0 * margin;
    let (scale, center) = if bbox.is_valid() && bbox.width() > 1.0 {
        let need = (bbox.width() / avail_w).max(bbox.height() / avail_h);
        (nice_scale(need * 1.05), bbox.center())
    } else {
        (100.0, Vec2::ZERO)
    };
    let ctx = render::RenderCtx::with_scale(doc, scale);
    let dl = render::render_document(&ctx);
    let mut c = Content::new();
    c.fill_color(Color::rgb(255, 255, 255));
    c.raw(&format!("0 0 {:.3} {:.3} re f", pw * MM2PT, ph * MM2PT));
    let s = 1.0 / scale;
    let offset = v(pw * 0.5, ph * 0.5) - center * s;
    let xf = Xf { scale: s, offset };
    draw_display_list(&mut c, &dl, &xf, s);
    write_pdf(
        vec![Page {
            width_pt: pw * MM2PT,
            height_pt: ph * MM2PT,
            content: c,
        }],
        path,
        &doc.name,
    )
}

/// Ein Blatt als PDF exportieren.
pub fn export_pdf(doc: &Document, path: &Path, sheet_index: Option<usize>) -> Result<(), String> {
    let idx = sheet_index.unwrap_or(0);
    let sheet = doc
        .sheets
        .get(idx)
        .ok_or_else(|| format!("Blatt {} existiert nicht", idx + 1))?;
    let page = draw_sheet_page(doc, sheet, sheet.frame);
    let title = format!("{} — {}", doc.name, sheet.name);
    write_pdf(vec![page], path, &title)
}

/// Alle Blätter in eine PDF.
pub fn export_all_sheets_pdf(doc: &Document, path: &Path) -> Result<(), String> {
    if doc.sheets.is_empty() {
        return export_model_pdf(doc, path, PaperSize::A3);
    }
    let pages: Vec<Page> = doc
        .sheets
        .iter()
        .map(|s| draw_sheet_page(doc, s, s.frame))
        .collect();
    write_pdf(pages, path, &doc.name)
}

/// Vorschau-Bilddaten für GUI (PDF-Seite als Bytes).
pub fn pdf_bytes(doc: &Document, sheet_index: Option<usize>) -> Result<Vec<u8>, String> {
    let idx = sheet_index.unwrap_or(0);
    let sheet = doc
        .sheets
        .get(idx)
        .ok_or_else(|| "Blatt nicht gefunden".to_string())?;
    let page = draw_sheet_page(doc, sheet, sheet.frame);
    let mut w = PdfWriter::new();
    let catalog = w.add(vec![]);
    let pages_idx = w.add(vec![]);
    let f1 = w.add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
    let f2 = w.add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec(),
    );
    let ci = w.add(stream_obj(&page.content.buf));
    let pi = w.add(page_object(
        page.width_pt / MM2PT,
        page.height_pt / MM2PT,
        ci,
        f1,
        f2,
        pages_idx,
    ));
    w.set(pages_idx, format!("<< /Type /Pages /Kids [{pi} 0 R] /Count 1 >>").into_bytes());
    w.set(catalog, format!("<< /Type /Catalog /Pages {pages_idx} 0 R >>").into_bytes());
    Ok(w.finish(catalog, None))
}

/// Textbreiten-Helfer für GUI-Vorschau.
pub fn measure_text(text: &str, height_mm: f64) -> f64 {
    text_width(text, height_mm * MM2PT) / MM2PT
}

/// Schriftgröße in Punkt für eine Höhe in mm.
pub fn mm_to_pt(mm: f64) -> f64 {
    mm * MM2PT
}

#[allow(dead_code)]
fn unused(_: &mut std::fmt::Formatter) -> std::fmt::Result {
    let mut s = String::new();
    let _ = write!(s, "");
    Ok(())
}
