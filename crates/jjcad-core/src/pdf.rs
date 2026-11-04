//! PDF-Export (eigener, abhängigkeitsfreier PDF-Schreiber).
//!
//! Erzeugt vektorbasierte PDFs mit **eingebetteter Schrift** — geeignet für
//! Bauanträge. Ein Blatt wird als Seite ausgegeben, Ansichtsfenster werden
//! beschnitten.
//!
//! Text wird als `Type0`/`Identity-H` mit `ToUnicode`-Zuordnung geschrieben:
//! jedes Zeichen überlebt den Weg zurück in den Import, auch `⌀`, `²` und
//! `≤`, und die Schrift steckt in der Datei (`/FontFile2`). Fehlt eine
//! TrueType-Schrift auf dem System, fällt der Schreiber auf Helvetica zurück —
//! dann ohne Einbettung, aber mit denselben Maßen (Liberation ist metrisch
//! gleich).

use crate::fonts::{self, FontStyle};
use crate::geom::*;
use crate::model::*;
use crate::render::{self, DisplayList, Prim, Shape, TextPrim};
use crate::style::{Color, LineType, PaperSize};
use crate::tessellate::CurveQuality;
use crate::text;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

const MM2PT: f64 = 72.0 / 25.4;

// ---------------------------------------------------------------------------
// Schriftenverbrauch je Seite
// ---------------------------------------------------------------------------

/// Ein benutzter Glyph: Breite für `/W` und Zeichen für `/ToUnicode`.
#[derive(Clone, Copy, Debug)]
struct GlyphUse {
    width: u16,
    ch: char,
}

/// Welche Schriften und Glyphen eine Seite benutzt hat.
///
/// Der Schreiber braucht das nach dem Inhalt: `/W` (Vorschübe) und
/// `/ToUnicode` entstehen daraus, und nur wirklich benutzte Schriften werden
/// eingebettet.
#[derive(Clone, Debug, Default)]
struct FontUse {
    /// Je Familie und Schnitt die benutzten Glyphen (Schlüssel: Familie·4+Schnitt).
    glyphs: BTreeMap<(TextFont, FontStyle), BTreeMap<u16, GlyphUse>>,
}

impl FontUse {
    fn merge(&mut self, other: &FontUse) {
        for (k, glyphs) in &other.glyphs {
            let ziel = self.glyphs.entry(*k).or_default();
            for (gid, g) in glyphs {
                ziel.entry(*gid).or_insert(*g);
            }
        }
    }

    fn note(&mut self, font: TextFont, style: FontStyle, c: char) {
        let gid = fonts::glyph_id(font, style, c);
        if gid == 0 {
            return;
        }
        let width = (fonts::advance_em(font, style, c) * 1000.0).round().max(1.0) as u16;
        self.glyphs
            .entry((font, style))
            .or_default()
            .entry(gid)
            .or_insert(GlyphUse { width, ch: c });
    }
}

// ---------------------------------------------------------------------------
// Inhaltsstrom
// ---------------------------------------------------------------------------

struct Content {
    buf: String,
    fonts: FontUse,
    /// Bilder der Seite, nach Kennung. Der Inhalt verweist auf sie als
    /// `/Im<Kennung>`; die Objekte entstehen erst in [`build_pdf`], weil dort
    /// die Objektnummern vergeben werden.
    bilder: BTreeMap<u64, std::sync::Arc<crate::bild::Bild>>,
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
            c if (c as u32) < 127 => out.push(c),
            c if (c as u32) < 256 => {
                out.push_str(&format!("\\{:03o}", c as u32));
            }
            _ => out.push('?'),
        }
    }
    out
}

/// Text als Hex-Satz für `Identity-H`: zwei Byte je Zeichen, Code = Glyphennummer.
fn hex_identity(c: &mut Content, font: TextFont, style: FontStyle, s: &str) -> String {
    let mut out = String::with_capacity(s.chars().count() * 4);
    for ch in s.chars() {
        let gid = fonts::glyph_id(font, style, ch);
        c.fonts.note(font, style, ch);
        let _ = write!(out, "{gid:04X}");
    }
    out
}

/// Schriftname im Inhaltsstrom: je Familie und Schnitt einer (`/F00`, `/F11` …).
fn font_res(font: TextFont, style: FontStyle) -> String {
    format!("/F{}{}", font.index(), style.index())
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
            fonts: FontUse::default(),
            bilder: BTreeMap::new(),
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

    /// Runde Linienenden und runde Übergänge (`1 J 1 j`).
    ///
    /// PDF streicht mit **stumpfen** Enden (`0 J`) und Gehrungsecken (`0 j`):
    /// eine Linie ist dann ein exaktes Rechteck, und zwei Linien in einer Ecke
    /// stoßen als zwei Rechtecke zusammen — die Verbindung wirkt kantig.
    /// Zeichenfläche (`canvas::capsule`) und Rasterbild (`raster.rs`,
    /// `LineCap::Round`/`LineJoin::Round`) zeichnen dagegen Kapseln. Dieselbe
    /// Darstellung hier: ein Halbkreis am Ende, runde Übergänge — die Linie
    /// bleibt dabei **ein** Pfad, es kommt kein zweites Element hinzu.
    ///
    /// Die Einstellung gehört zum Grafikzustand und gilt bis zum Seitenende;
    /// `q`/`Q` (Ansichtsfenster) tragen sie mit.
    fn runde_enden(&mut self) {
        self.raw("1 J 1 j");
    }
    fn fill(&mut self) {
        self.raw("f");
    }

    /// Füllen nach der Even-Odd-Regel (`f*`) — nötig für Flächen mit Löchern:
    /// alle Ringe sind Unterpfade, jeder zweite Umschlag wird ausgespart.
    fn fill_even_odd(&mut self) {
        self.raw("f*");
    }
    /// Schreibt einen Text mit seinen Läufen; `size_pt` ist die Schrifthöhe.
    fn text(&mut self, t: &TextPrim, size_pt: f64) {
        if size_pt < 0.3 || t.text.is_empty() {
            return;
        }
        let runs = t.effective_runs();
        let lines = text::layout(&t.text, &runs);
        if lines.is_empty() {
            return;
        }
        let lh = size_pt * 1.25;
        let n = lines.len() as f64;
        // Senkrechte Lage gilt für den **ganzen** Block: „Mitte" zentriert
        // alle Zeilen um den Anker, „Unten" stellt die letzte Zeile darauf.
        let dy0 = match t.valign {
            TextAlignV::Top => -size_pt * 0.8,
            TextAlignV::Middle => -size_pt * 0.36 + (n - 1.0) * lh * 0.5,
            TextAlignV::Bottom => (n - 1.0) * lh,
        };
        let (s, c) = t.angle.sin_cos();
        let angle = t.angle;
        // Weltpunkt (in Punkt) aus Satzversatz in Textrichtung.
        let platz = |dx: f64, dy: f64| -> (f64, f64) {
            let rx = dx * c - dy * s;
            let ry = dx * s + dy * c;
            (t.pos.x * MM2PT + rx, t.pos.y * MM2PT + ry)
        };
        let _ = angle;
        for (i, line) in lines.iter().enumerate() {
            let w = line.width_em * size_pt;
            let dx0 = match t.align {
                TextAlignH::Left => 0.0,
                TextAlignH::Center => -w / 2.0,
                TextAlignH::Right => -w,
            };
            let dy = dy0 - i as f64 * lh;
            let mut pen = dx0;
            for seg in &line.segs {
                if seg.text.is_empty() {
                    continue;
                }
                let seg_size = size_pt * seg.scale;
                let seg_w = fonts::advance_em_str(t.font, seg.style, &seg.text) * seg_size;
                let (x, y) = platz(pen, dy);
                if fonts::ttf_bytes(t.font, seg.style).is_some() {
                    let hex = hex_identity(self, t.font, seg.style, &seg.text);
                    let rise = seg.rise_em * size_pt;
                    let s_str = format!(
                        "BT {} {:.2} Tf {:.3} Ts {:.5} {:.5} {:.5} {:.5} {:.3} {:.3} Tm <{}> Tj ET",
                        font_res(t.font, seg.style),
                        seg_size,
                        rise,
                        c,
                        s,
                        -s,
                        c,
                        x,
                        y,
                        hex
                    );
                    self.raw(&s_str);
                } else {
                    // Ersatzweg ohne eingebettete Schrift: Helvetica/WinAnsi.
                    let fname = font_res(t.font, seg.style);
                    let s_str = format!(
                        "BT {fname} {:.2} Tf {:.3} Ts {:.5} {:.5} {:.5} {:.5} {:.3} {:.3} Tm ({}) Tj ET",
                        seg_size,
                        seg.rise_em * size_pt,
                        c,
                        s,
                        -s,
                        c,
                        x,
                        y,
                        esc(&seg.text)
                    );
                    self.raw(&s_str);
                }
                if seg.underline {
                    // Unterstreichung als gefülltes Viereck unter der Grundlinie.
                    let dick = (seg_size * 0.06).max(0.3);
                    let (xa, ya) = platz(pen, dy - seg_size * 0.12);
                    let (xb, yb) = platz(pen + seg_w, dy - seg_size * 0.12);
                    let (xc, yc) = platz(pen + seg_w, dy - seg_size * 0.12 + dick);
                    let (xd, yd) = platz(pen, dy - seg_size * 0.12 + dick);
                    self.raw("q");
                    self.raw(&format!(
                        "{xa:.3} {ya:.3} m {xb:.3} {yb:.3} l {xc:.3} {yc:.3} l {xd:.3} {yd:.3} l h f"
                    ));
                    self.raw("Q");
                }
                pen += seg_w;
            }
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
        Shape::EllipseArc {
            c: cc,
            rx,
            ry,
            rot,
            a0,
            a1,
        } => {
            c.stroke_color(prim.style.color);
            c.line_width(prim.style.width);
            c.dash(prim.style.line_type);
            let pts = crate::geom::Ellipse::new(*cc, *rx, *ry, *rot).arc_points(*a0, *a1, 0.1);
            push_polyline(c, &pts, false, xf);
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
        Shape::Image { a, b, bild } => {
            // Das Bild wird als XObject eingebettet (RGB, auf Weiß gelegt —
            // PDF-Bilder dieses Schreibers tragen keinen Alphakanal) und über
            // eine cm-Matrix in das Rechteck gespannt.
            let ecken = crate::bild::ecken(*a, *b);
            let q0 = xf.map(ecken[0]);
            let q1 = xf.map(ecken[2]);
            let (x0, x1) = (q0.x.min(q1.x), q0.x.max(q1.x));
            let (y0, y1) = (q0.y.min(q1.y), q0.y.max(q1.y));
            let (bw, bh) = ((x1 - x0) * MM2PT, (y1 - y0) * MM2PT);
            if bw <= 0.01 || bh <= 0.01 {
                return;
            }
            c.bilder.insert(bild.kennung, bild.clone());
            c.raw("q");
            c.raw(&format!(
                "{:.4} 0 0 {:.4} {:.4} {:.4} cm",
                bw,
                bh,
                x0 * MM2PT,
                y0 * MM2PT
            ));
            c.raw(&format!("/{} Do", bild_res(bild.kennung)));
            c.raw("Q");
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

/// Ressourcenname eines Bildes im Seiteninhalt.
fn bild_res(kennung: u64) -> String {
    format!("Im{kennung:016X}")
}

/// Bild als PDF-XObject: RGB, FlateDecode, Alphakanal auf Weiß gelegt.
fn bild_obj(w: &mut PdfWriter, bild: &crate::bild::Bild) -> usize {
    let mut rgb = Vec::with_capacity((bild.breite as usize * bild.hoehe as usize) * 3);
    for px in bild.rgba.chunks_exact(4) {
        let a = px[3] as u32;
        let rest = 255 - a;
        for kanal in &px[0..3] {
            rgb.push((((*kanal as u32) * a + 255 * rest) / 255) as u8);
        }
    }
    let daten = compress(&rgb);
    let mut body = format!(
        "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB \
         /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
        bild.breite,
        bild.hoehe,
        daten.len()
    )
    .into_bytes();
    body.extend_from_slice(&daten);
    body.extend_from_slice(b"\nendstream");
    w.add(body)
}

fn draw_display_list(c: &mut Content, dl: &DisplayList, xf: &Xf, paper_scale: f64) {
    for prim in &dl.prims {
        draw_prim(c, prim, xf, paper_scale);
    }
}

// ---------------------------------------------------------------------------
// Kompression (FlateDecode) — dieselbe Bibliothek, die lopdf ohnehin mitbringt
// ---------------------------------------------------------------------------

fn compress(data: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    if enc.write_all(data).is_err() {
        return data.to_vec();
    }
    match enc.finish() {
        Ok(v) => v,
        Err(_) => data.to_vec(),
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
        out.extend_from_slice(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n");
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

fn stream_obj(data: &[u8]) -> Vec<u8> {
    let packed = compress(data);
    let mut v = format!(
        "<< /Length {} /Filter /FlateDecode >>\nstream\n",
        packed.len()
    )
    .into_bytes();
    v.extend_from_slice(&packed);
    v.extend_from_slice(b"\nendstream");
    v
}

/// Fontdatei als Stream (vollständig eingebettet, nur gepackt).
fn font_file_obj(data: &[u8]) -> Vec<u8> {
    let packed = compress(data);
    let mut v = format!(
        "<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n",
        packed.len(),
        data.len()
    )
    .into_bytes();
    v.extend_from_slice(&packed);
    v.extend_from_slice(b"\nendstream");
    v
}

/// Text als UTF-16BE-Hexstring mit BOM — der Weg, auf dem Umlaute und
/// Gedankenstriche im Info-Wörterbuch ankommen (`Wohnhaus — Blatt 1`).
fn utf16_hex(s: &str) -> String {
    let mut out = String::from("FEFF");
    for unit in s.encode_utf16() {
        let _ = write!(out, "{unit:04X}");
    }
    out
}

/// `/ToUnicode`-CMap: Glyphennummer → Zeichen, damit der Import wieder Text
/// erhält statt nur Konturen.
fn to_unicode_cmap(glyphs: &[(u16, char)]) -> Vec<u8> {
    let mut s = String::new();
    s.push_str("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n");
    s.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    s.push_str("/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n");
    s.push_str("1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    for chunk in glyphs.chunks(100) {
        let _ = write!(s, "{} beginbfchar\n", chunk.len());
        for (gid, ch) in chunk {
            let mut buf = [0u16; 2];
            let enc = ch.encode_utf16(&mut buf);
            if enc.len() == 1 {
                let _ = write!(s, "<{gid:04X}> <{:04X}>\n", enc[0]);
            } else {
                let _ = write!(s, "<{gid:04X}> <{:04X}{:04X}>\n", enc[0], enc[1]);
            }
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s.into_bytes()
}

/// Alle Objekte für eine eingebettete Schrift; liefert die Nummer des
/// Type0-Fonts, der im Seiten-`/Resources` steht.
fn write_font(
    w: &mut PdfWriter,
    font: TextFont,
    style: FontStyle,
    uses: &FontUse,
) -> Option<usize> {
    let data = fonts::ttf_bytes(font, style)?;
    let info = fonts::ttf_info(font, style)?;
    let glyphs = uses.glyphs.get(&(font, style))?;
    let upem = info.units_per_em.max(1.0);
    // /W: je Glyphe ein Einzeleintrag — kurz genug und ohne Annahmen über
    // zusammenhängende Bereiche.
    let mut w_arr = String::from("[");
    let mut pairs: Vec<(u16, char)> = Vec::new();
    for (gid, g) in glyphs.iter() {
        let _ = write!(w_arr, " {gid} [{}]", g.width);
        pairs.push((*gid, g.ch));
    }
    w_arr.push_str(" ]");
    let bbox = format!(
        "[{:.0} {:.0} {:.0} {:.0}]",
        info.x_min / upem * 1000.0,
        info.y_min / upem * 1000.0,
        info.x_max / upem * 1000.0,
        info.y_max / upem * 1000.0
    );
    // Deskriptor-Kennzeichen: 32 = serifenlos (nicht symbolisch),
    // 64 = kursiv, 262144 = fett.
    let mut flags = 32;
    if matches!(style, FontStyle::Italic | FontStyle::BoldItalic) {
        flags |= 64;
    }
    if matches!(style, FontStyle::Bold | FontStyle::BoldItalic) {
        flags |= 262144;
    }
    let file_idx = w.add(font_file_obj(data));
    let desc_idx = w.add(
        format!(
            "<< /Type /FontDescriptor /FontName /{} /Flags {} /FontBBox {} \
             /ItalicAngle {} /Ascent {:.0} /Descent {:.0} /CapHeight {:.0} /StemV {} /FontFile2 {} 0 R >>",
            fonts::ps_name(font, style),
            flags,
            bbox,
            if flags & 64 != 0 { -12 } else { 0 },
            info.ascent / upem * 1000.0,
            info.descent / upem * 1000.0,
            info.cap_height / upem * 1000.0,
            if flags & 262144 != 0 { 140 } else { 88 },
            file_idx
        )
        .into_bytes(),
    );
    let cid_idx = w.add(
        format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor {} 0 R /DW 1000 /W {} /CIDToGIDMap /Identity >>",
            fonts::ps_name(font, style),
            desc_idx,
            w_arr
        )
        .into_bytes(),
    );
    let cmap_idx = w.add(stream_obj(&to_unicode_cmap(&pairs)));
    let type0 = w.add(
        format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H \
             /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>",
            fonts::ps_name(font, style),
            cid_idx,
            cmap_idx
        )
        .into_bytes(),
    );
    Some(type0)
}

fn page_object(
    w: f64,
    h: f64,
    contents: usize,
    fonts: &[(TextFont, FontStyle, usize)],
    bilder: &[(u64, usize)],
    parent: usize,
) -> Vec<u8> {
    let mut res = String::new();
    for (font, st, idx) in fonts {
        let _ = write!(res, "{} {} 0 R ", font_res(*font, *st), idx);
    }
    let mut xobj = String::new();
    for (kennung, idx) in bilder {
        let _ = write!(xobj, "/{} {} 0 R ", bild_res(*kennung), idx);
    }
    let xobj = if xobj.is_empty() {
        String::new()
    } else {
        format!("/XObject << {xobj}>> ")
    };
    format!(
        "<< /Type /Page /Parent {parent} 0 R /MediaBox [0 0 {:.3} {:.3}] \
         /Resources << /Font << {res}>> {xobj}>> /Contents {contents} 0 R >>",
        w * MM2PT,
        h * MM2PT
    )
    .into_bytes()
}

/// Baut die PDF-Datei aus fertigen Seiten.
fn build_pdf(pages: Vec<Page>, title: &str) -> Vec<u8> {
    let mut uses = FontUse::default();
    for p in &pages {
        uses.merge(&p.content.fonts);
    }
    let mut w = PdfWriter::new();
    let catalog_idx = w.add(vec![]);
    let pages_idx = w.add(vec![]);
    let mut font_objs: Vec<(TextFont, FontStyle, usize)> = Vec::new();
    for (font, st) in uses.glyphs.keys().copied().collect::<Vec<_>>() {
        if let Some(idx) = write_font(&mut w, font, st, &uses) {
            font_objs.push((font, st, idx));
        }
    }
    // Ersatzweg ohne TrueType-Schrift: Helvetica (nicht eingebettet).
    if font_objs.is_empty() {
        let reg = w.add(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );
        let bold = w.add(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );
        font_objs.push((TextFont::Sans, FontStyle::Regular, reg));
        font_objs.push((TextFont::Sans, FontStyle::Bold, bold));
    }
    // Bilder einmal je Kennung einbetten — dieselbe Datei kann auf mehreren
    // Seiten und in mehreren Ansichtsfenstern liegen.
    let mut bild_objs: BTreeMap<u64, usize> = BTreeMap::new();
    for p in &pages {
        for (kennung, bild) in &p.content.bilder {
            if !bild_objs.contains_key(kennung) {
                let idx = bild_obj(&mut w, bild);
                bild_objs.insert(*kennung, idx);
            }
        }
    }
    let mut kids = Vec::new();
    for p in pages {
        let content_idx = w.add(stream_obj(p.content.buf.as_bytes()));
        let seite_bilder: Vec<(u64, usize)> = p
            .content
            .bilder
            .keys()
            .filter_map(|k| bild_objs.get(k).map(|i| (*k, *i)))
            .collect();
        let page_idx = w.add(page_object(
            p.width_pt / MM2PT,
            p.height_pt / MM2PT,
            content_idx,
            &font_objs,
            &seite_bilder,
            pages_idx,
        ));
        kids.push(page_idx);
    }
    let kids_str: Vec<String> = kids.iter().map(|k| format!("{k} 0 R")).collect();
    w.set(
        pages_idx,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids_str.join(" "),
            kids.len()
        )
        .into_bytes(),
    );
    w.set(
        catalog_idx,
        format!("<< /Type /Catalog /Pages {pages_idx} 0 R >>").into_bytes(),
    );
    let info_idx = w.add(
        format!(
            "<< /Title <{}> /Producer <{}> /Creator <{}> >>",
            utf16_hex(title),
            utf16_hex(&format!("JoJa-CAD {}", crate::APP_VERSION)),
            utf16_hex("JoJa-CAD")
        )
        .into_bytes(),
    );
    w.finish(catalog_idx, Some(info_idx))
}

fn write_pdf(pages: Vec<Page>, path: &Path, title: &str) -> Result<(), String> {
    let bytes = build_pdf(pages, title);
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

/// Neuer Seiteninhalt: weißer Grund und **runde Linienenden** (T0086).
///
/// Beide Seitenwege (Blatt und Modellzeichnung) beginnen hier — sonst müsste
/// die Einstellung an zwei Stellen gepflegt werden und einer der Exporte
/// zeichnete wieder Rechtecke.
fn seite(pw: f64, ph: f64) -> Content {
    let mut c = Content::new();
    c.fill_color(Color::rgb(255, 255, 255));
    c.raw(&format!("0 0 {:.3} {:.3} re f", pw * MM2PT, ph * MM2PT));
    c.runde_enden();
    c
}

fn draw_sheet_page(doc: &Document, sheet: &Sheet) -> Page {
    let (pw, ph) = sheet.size();
    let mut c = seite(pw, ph);

    // Papierraum: Rahmen, Schriftfeld und eigene Blattelemente sind Objekte
    // des Blatts (T0004) — gedruckt wird genau das, was auf dem Blatt steht.
    let dl = render::render_sheet(doc, sheet);
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

pub(crate) fn nice_scale(need: f64) -> f64 {
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
    // Modellseite: immer die Zeichnung, auch wenn gerade ein Blatt offen ist.
    let ctx = render::RenderCtx::model_scaled(doc, scale);
    let dl = render::render_document(&ctx);
    let mut c = seite(pw, ph);
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
    let page = draw_sheet_page(doc, sheet);
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
        .map(|s| draw_sheet_page(doc, s))
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
    let page = draw_sheet_page(doc, sheet);
    Ok(build_pdf(vec![page], &doc.name))
}

/// Textbreite in mm — dieselbe Vermessung wie beim Setzen.
pub fn measure_text(text: &str, height_mm: f64) -> f64 {
    fonts::advance_em_str(TextFont::Sans, FontStyle::Regular, text) * height_mm
}

/// Schriftgröße in Punkt für eine Höhe in mm.
pub fn mm_to_pt(mm: f64) -> f64 {
    mm * MM2PT
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    /// Der Seiteninhalt setzt runde Enden und Übergänge (T0086).
    #[test]
    fn seite_setzt_runde_linienenden() {
        let c = seite(420.0, 297.0);
        assert!(
            c.buf.contains("1 J 1 j"),
            "Seiteninhalt ohne runde Linienenden:\n{}",
            c.buf
        );
    }

    /// Auch die Modellseite trägt die Einstellung. Geprüft wird im
    /// **entpackten** Inhaltsstrom der geschriebenen Datei — der Strom selbst
    /// ist gepackt und in der Datei nicht lesbar.
    #[test]
    fn modellseite_setzt_runde_linienenden() {
        let mut sess = crate::session::Session::new();
        sess.execute(
            "draw.line",
            &serde_json::json!({"a": [0, 0], "b": [100, 0]}),
        )
        .unwrap();
        let pfad = std::env::temp_dir().join("jjcad-t0086-modell.pdf");
        export_model_pdf(&sess.doc, &pfad, PaperSize::A3).unwrap();
        let bytes = std::fs::read(&pfad).unwrap();

        let start = bytes
            .windows(7)
            .position(|w| w == b"stream\n")
            .expect("kein Inhaltsstrom")
            + 7;
        let ende = start
            + bytes[start..]
                .windows(10)
                .position(|w| w == b"\nendstream")
                .expect("Stromende fehlt");
        let mut entpackt = String::new();
        flate2::read::ZlibDecoder::new(&bytes[start..ende])
            .read_to_string(&mut entpackt)
            .expect("Inhaltsstrom nicht entpackbar");
        assert!(
            entpackt.contains("1 J 1 j"),
            "Modellseite ohne runde Linienenden:\n{entpackt}"
        );
    }
}
