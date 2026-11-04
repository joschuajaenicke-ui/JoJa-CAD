//! Interpreter für PDF-Inhaltsströme.
//!
//! Führt den Grafikzustand (`q`/`Q`, `cm`, Farben, Linien, Textzustand) und
//! schreibt dabei Entities in Welt-Millimetern. Die Umrechnung Punkt → mm
//! steckt in `base`, das die Fassade übergibt (Seitenkasten, Drehung, Maßstab).

use super::font::{self, FontInfo};
use super::path::{self, GeoShape, PdfPath};
use super::{compose, ImportedEntity, Warnings, MAX_ENTITIES_PER_PAGE};
use super::style;
use crate::geom::{v, Affine, Vec2, Vertex};
use crate::model::*;
use crate::style::{Color, FillPattern, LineType};
use lopdf::{Dictionary, Document, Object, Stream};
use std::collections::HashMap;

/// Grafikzustand nach PDF-Spezifikation (Abschnitt 4.3).
#[derive(Clone)]
struct Gs {
    ctm: Affine,
    line_width: f64,
    /// Erkannte Strichart; `None` = Muster ohne Entsprechung (gezählt).
    dash: Option<LineType>,
    stroke: Color,
    fill: Color,
    font: Option<String>,
    font_size: f64,
    char_space: f64,
    word_space: f64,
    h_scale: f64,
    leading: f64,
    rise: f64,
}

impl Default for Gs {
    fn default() -> Self {
        Self {
            ctm: Affine::IDENTITY,
            // PDF-Vorgabe: 1 Punkt
            line_width: 1.0,
            dash: Some(LineType::Solid),
            stroke: Color::rgb(0, 0, 0),
            fill: Color::rgb(0, 0, 0),
            font: None,
            font_size: 1.0,
            char_space: 0.0,
            word_space: 0.0,
            h_scale: 1.0,
            leading: 0.0,
            rise: 0.0,
        }
    }
}

/// Gesammelter Textlauf: mehrere Anzeigeoperationen auf einer Grundlinie.
struct TextAccum {
    runs: Vec<TextRun>,
    start: Vec2,
    /// Ende des zuletzt gesetzten Stücks (mm).
    end: Vec2,
    height_mm: f64,
    angle: f64,
    /// Text wird mit der **Füllfarbe** gesetzt, nicht mit der Strichfarbe.
    color: Color,
    /// Schriftart, aus dem PDF-Schriftnamen geraten.
    font: crate::model::TextFont,
}

pub fn run(
    doc: &Document,
    content: &[u8],
    resources: &Dictionary,
    base: Affine,
    warn: &mut Warnings,
) -> Vec<ImportedEntity> {
    let mut it = Interp {
        doc,
        base,
        gs: Gs::default(),
        stack: Vec::new(),
        path: PdfPath::default(),
        fonts: HashMap::new(),
        out: Vec::new(),
        text: None,
        warn,
        depth: 0,
        tm: Affine::IDENTITY,
        tlm: Affine::IDENTITY,
    };
    it.exec(content, resources);
    it.flush_text();
    it.out
}

struct Interp<'a> {
    doc: &'a Document,
    base: Affine,
    gs: Gs,
    stack: Vec<Gs>,
    path: PdfPath,
    fonts: HashMap<String, Option<FontInfo>>,
    out: Vec<ImportedEntity>,
    text: Option<TextAccum>,
    warn: &'a mut Warnings,
    depth: u32,
    /// Textmatrix und Zeilenmatrix (`BT` setzt beide zurück, `q`/`Q` nicht).
    tm: Affine,
    tlm: Affine,
}

impl<'a> Interp<'a> {
    /// Gesamtabbildung PDF-Raum → Welt-mm.
    fn total(&self) -> Affine {
        compose(self.base, self.gs.ctm)
    }

    fn scale(&self) -> f64 {
        self.total().scale_factor()
    }

    fn warn_once_key(&mut self, art: &str, key: &str, einmal: &mut Vec<String>) {
        if !einmal.iter().any(|k| k == key) {
            einmal.push(key.to_string());
            self.warn.add(art);
        }
    }

    fn exec(&mut self, content: &[u8], resources: &Dictionary) {
        let decoded = super::token::decode(content);
        if decoded.truncated {
            self.warn.add("Abgebrochener Inhaltsstrom");
        }
        let mut einmal: Vec<String> = Vec::new();
        for op in decoded.ops {
            if self.out.len() >= MAX_ENTITIES_PER_PAGE {
                self.warn.add("Obergrenze je Seite erreicht");
                break;
            }
            self.step(&op, resources, &mut einmal);
        }
        // Ein Pfad ohne Maloperation verfällt (wie im PDF).
        self.path.clear();
    }

    fn step(&mut self, op: &super::token::Op, res: &Dictionary, einmal: &mut Vec<String>) {
        use super::token::Operand as O;
        let a = &op.args;
        let num = |i: usize| a.get(i).and_then(|o| o.num()).unwrap_or(0.0);
        match op.op.as_str() {
            // ---------------- Grafikzustand ----------------
            "q" => {
                self.stack.push(self.gs.clone());
                if self.stack.len() > 256 {
                    self.stack.remove(0);
                }
            }
            "Q" => {
                if let Some(g) = self.stack.pop() {
                    self.gs = g;
                }
            }
            "cm" => {
                if a.len() >= 6 {
                    let m = Affine {
                        a: num(0),
                        b: num(1),
                        c: num(2),
                        d: num(3),
                        tx: num(4),
                        ty: num(5),
                    };
                    self.gs.ctm = compose(m, self.gs.ctm);
                }
            }
            "w" => self.gs.line_width = num(0).abs().max(0.0),
            "d" => {
                // [Muster] Versatz
                let muster: Vec<f64> = match a.first() {
                    Some(O::Array(arr)) => arr.iter().filter_map(|o| o.num()).collect(),
                    _ => Vec::new(),
                };
                match style::line_type_from_dash(&muster) {
                    Some(lt) => self.gs.dash = Some(lt),
                    None => {
                        self.gs.dash = Some(LineType::Solid);
                        self.warn.add("Strichmuster ohne Entsprechung");
                    }
                }
            }
            "J" | "j" | "M" | "ri" | "i" => {}
            "gs" => {
                if let Some(O::Name(n)) = a.first() {
                    if let Some(eg) = lookup_res(self.doc, res, b"ExtGState", n.as_bytes()) {
                        for key in [b"ca".as_slice(), b"CA"] {
                            if let Ok(v) = eg.get(key).and_then(|o| o.as_float()) {
                                if (v as f64 - 1.0).abs() > 0.01 {
                                    self.warn_once_key("Transparenz ignoriert", "Transparenz", einmal);
                                }
                            }
                        }
                        if let Ok(lw) = eg.get(b"LW").and_then(|o| o.as_float()) {
                            self.gs.line_width = lw as f64;
                        }
                    }
                }
            }
            // ---------------- Farben ----------------
            "rg" => self.gs.fill = style::rgb(num(0), num(1), num(2)),
            "RG" => self.gs.stroke = style::rgb(num(0), num(1), num(2)),
            "g" => self.gs.fill = style::gray(num(0)),
            "G" => self.gs.stroke = style::gray(num(0)),
            "k" => self.gs.fill = style::cmyk(num(0), num(1), num(2), num(3)),
            "K" => self.gs.stroke = style::cmyk(num(0), num(1), num(2), num(3)),
            "sc" | "scn" => {
                if a.len() == 1 {
                    self.gs.fill = style::gray(num(0));
                } else if a.len() == 3 {
                    self.gs.fill = style::rgb(num(0), num(1), num(2));
                } else if a.len() == 4 {
                    self.gs.fill = style::cmyk(num(0), num(1), num(2), num(3));
                } else if a.iter().any(|o| matches!(o, O::Name(_))) {
                    self.warn_once_key("Muster ignoriert", "Muster", einmal);
                }
            }
            "SC" | "SCN" => {
                if a.len() == 1 {
                    self.gs.stroke = style::gray(num(0));
                } else if a.len() == 3 {
                    self.gs.stroke = style::rgb(num(0), num(1), num(2));
                } else if a.len() == 4 {
                    self.gs.stroke = style::cmyk(num(0), num(1), num(2), num(3));
                } else if a.iter().any(|o| matches!(o, O::Name(_))) {
                    self.warn_once_key("Muster ignoriert", "Muster", einmal);
                }
            }
            // ---------------- Pfad ----------------
            "m" => {
                let p = self.pt(v(num(0), num(1)));
                self.path.move_to(p);
            }
            "l" => {
                let p = self.pt(v(num(0), num(1)));
                self.path.line_to(p);
            }
            "c" => {
                let c1 = self.pt(v(num(0), num(1)));
                let c2 = self.pt(v(num(2), num(3)));
                let e = self.pt(v(num(4), num(5)));
                self.path.curve_to(c1, c2, e);
            }
            "v" => {
                // erste Kontrollstrecke = aktueller Punkt
                let cur = self.path_current().unwrap_or_else(|| self.pt(v(num(0), num(1))));
                let c2 = self.pt(v(num(0), num(1)));
                let e = self.pt(v(num(2), num(3)));
                self.path.curve_to(cur, c2, e);
            }
            "y" => {
                let c1 = self.pt(v(num(0), num(1)));
                let e = self.pt(v(num(2), num(3)));
                self.path.curve_to(c1, e, e);
            }
            "h" => self.path.close(),
            "re" => {
                let (x, y, w, h) = (num(0), num(1), num(2), num(3));
                let p0 = self.pt(v(x, y));
                let p1 = self.pt(v(x + w, y));
                let p2 = self.pt(v(x + w, y + h));
                let p3 = self.pt(v(x, y + h));
                self.path.rect(p0, p1, p2, p3);
            }
            "S" => self.stroke(1.0),
            "s" => {
                self.path.close();
                self.stroke(1.0);
            }
            "f" | "F" => self.fill_only(),
            "f*" => self.fill_only(),
            "B" => {
                self.fill_and_stroke();
            }
            "B*" => self.fill_and_stroke(),
            "b" => {
                self.path.close();
                self.fill_and_stroke();
            }
            "b*" => {
                self.path.close();
                self.fill_and_stroke();
            }
            "n" => self.path.clear(),
            "W" | "W*" => self.warn_once_key("Beschnitt ignoriert", "Beschnitt", einmal),
            // ---------------- Text ----------------
            // BT/ET beginnen und beenden nur ein Textobjekt. Der Export
            // schreibt jeden Lauf als eigenes Objekt mit eigener Matrix —
            // deshalb wird hier **nicht** abgeschlossen: zusammengehörende
            // Läufe (Fett, Hochstellung) werden über die Lage erkannt.
            "BT" => {
                self.tm = Affine::IDENTITY;
                self.tlm = Affine::IDENTITY;
            }
            "ET" => {}
            "Tf" => {
                if let Some(O::Name(n)) = a.first() {
                    self.gs.font = Some(n.clone());
                }
                self.gs.font_size = num(1).abs();
            }
            "Td" => self.td(num(0), num(1)),
            "TD" => {
                self.gs.leading = -num(1);
                self.td(num(0), num(1));
            }
            "Tm" => {
                let m = Affine {
                    a: num(0),
                    b: num(1),
                    c: num(2),
                    d: num(3),
                    tx: num(4),
                    ty: num(5),
                };
                self.tm = m;
                self.tlm = m;
            }
            "T*" => self.td(0.0, -self.gs.leading),
            "TL" => self.gs.leading = num(0),
            "Tc" => self.gs.char_space = num(0),
            "Tw" => self.gs.word_space = num(0),
            "Tz" => {
                self.gs.h_scale = num(0) / 100.0;
                if (self.gs.h_scale - 1.0).abs() > 0.001 {
                    self.warn_once_key("Textbreite (Tz) normalisiert", "Tz", einmal);
                }
            }
            "Ts" => self.gs.rise = num(0),
            "Tj" => {
                if let Some(O::Str(s)) = a.first() {
                    let s = s.clone();
                    self.show(&s, res);
                }
            }
            "TJ" => {
                if let Some(O::Array(items)) = a.first() {
                    let items = items.clone();
                    for it in items {
                        match it {
                            O::Str(s) => self.show(&s, res),
                            O::Num(n) => {
                                // Kerning in Tausendstel Em
                                let tx = -n / 1000.0 * self.gs.font_size * self.gs.h_scale;
                                self.tm = compose(Affine::translate(v(tx, 0.0)), self.tm);
                            }
                            _ => {}
                        }
                    }
                }
            }
            "'" => {
                self.td(0.0, -self.gs.leading);
                if let Some(O::Str(s)) = a.first() {
                    let s = s.clone();
                    self.show(&s, res);
                }
            }
            "\"" => {
                self.gs.word_space = num(0);
                self.gs.char_space = num(1);
                self.td(0.0, -self.gs.leading);
                if let Some(O::Str(s)) = a.get(2) {
                    let s = s.clone();
                    self.show(&s, res);
                }
            }
            // ---------------- Fremdobjekte ----------------
            "Do" => {
                if let Some(O::Name(n)) = a.first() {
                    self.do_xobject(n, res);
                }
            }
            "sh" => self.warn_once_key("Schattierung ignoriert", "Schattierung", einmal),
            "BI" => self.warn_once_key("Bild nicht übernommen", "Bild", einmal),
            _ => {}
        }
    }

    fn path_current(&self) -> Option<Vec2> {
        self.path.current()
    }

    fn pt(&self, p: Vec2) -> Vec2 {
        self.total().apply(p)
    }

    // ---------------- Textzustand ----------------

    fn td(&mut self, tx: f64, ty: f64) {
        self.tlm = compose(Affine::translate(v(tx, ty)), self.tlm);
        self.tm = self.tlm;
    }

    /// Schrift laden (mit Zwischenspeicher) und die Angaben zum Satz
    /// herausziehen — als Kopie, damit danach der Zustand wieder frei ist.
    fn text_snapshot(
        &mut self,
        res: &Dictionary,
        bytes: &[u8],
    ) -> Option<(String, usize, f64, crate::fonts::FontStyle, crate::model::TextFont)> {
        let name = self.gs.font.clone()?;
        if !self.fonts.contains_key(&name) {
            let info =
                lookup_res(self.doc, res, b"Font", name.as_bytes()).map(|d| font::load(self.doc, &d));
            self.fonts.insert(name.clone(), info);
        }
        let fi = self.fonts.get(&name)?.as_ref()?;
        let (text, fehlend) = fi.decode(bytes);
        let mut w_em = 0.0;
        for code in fi.codes(bytes) {
            w_em += fi.advance(code);
        }
        // Schriftart aus dem Namen raten — so kommt der Text mit derselben
        // Familie zurück, mit der er geschrieben wurde.
        let familie = crate::fonts::font_from_ps_name(&fi.base);
        Some((text, fehlend, w_em, fi.style, familie))
    }

    fn show(&mut self, bytes: &[u8], res: &Dictionary) {
        let Some((s, fehlend, w_em, style_flags, schrift)) = self.text_snapshot(res, bytes) else {
            self.warn.add("Schrift nicht lesbar");
            return;
        };
        let total = self.total();
        if fehlend > 0 {
            self.warn.add("Text ohne Zeichenzuordnung");
        }
        let fs = self.gs.font_size;
        let rise = self.gs.rise;
        let start_pdf = self.tm.apply(v(0.0, rise));
        let end_pdf = self.tm.apply(v(w_em * fs * self.gs.h_scale, rise));
        let start = total.apply(start_pdf);
        let end = total.apply(end_pdf);
        let height_mm = fs.abs() * total.scale_factor();
        let angle = total.rotation();
        let sup = rise.abs() > 0.15 * fs.abs() && fs != 0.0;
        // Hochgestellte Zeichen sitzen über der Grundlinie. Für die Frage
        // „gehört das noch zum selben Text?" zählt die **Grundlinie**, nicht
        // die Lage des Zeichens.
        let rise_mm = rise * total.scale_factor();
        let (sa, ca) = angle.sin_cos();
        let up = v(-sa, ca);
        let base_start = start - up * rise_mm;
        let base_end = end - up * rise_mm;
        let base_height = if sup {
            height_mm / crate::text::SCRIPT_SCALE
        } else {
            height_mm
        };

        if s.is_empty() {
            self.tm = compose(
                Affine::translate(v(w_em * fs * self.gs.h_scale, 0.0)),
                self.tm,
            );
            return;
        }

        let run = TextRun {
            text: s.clone(),
            bold: style_flags == crate::fonts::FontStyle::Bold
                || style_flags == crate::fonts::FontStyle::BoldItalic,
            italic: style_flags == crate::fonts::FontStyle::Italic
                || style_flags == crate::fonts::FontStyle::BoldItalic,
            underline: false,
            superscript: sup,
        };

        // Anschließen an den laufenden Satz?
        let mut angeschlossen = false;
        if let Some(acc) = &mut self.text {
            let (sin, cos) = acc.angle.sin_cos();
            let d = base_start - acc.end;
            let along = d.x * cos + d.y * sin;
            let quer = -d.x * sin + d.y * cos;
            let gleich_gross = (acc.height_mm - base_height).abs() <= acc.height_mm.abs() * 0.02 + 1e-6;
            let gleich_gedreht = ((acc.angle - angle).abs() % std::f64::consts::TAU) < 0.01;
            if gleich_gross
                && gleich_gedreht
                && quer.abs() < acc.height_mm.abs() * 0.2 + 1e-6
                && along.abs() < acc.height_mm.abs() * 0.6 + 1e-6
            {
                acc.runs.push(run.clone());
                acc.end = base_end;
                angeschlossen = true;
            }
        }
        if !angeschlossen {
            self.flush_text();
            self.text = Some(TextAccum {
                runs: vec![run],
                start: base_start,
                end: base_end,
                height_mm: base_height,
                angle,
                color: self.gs.fill,
                font: schrift,
            });
        }

        // Textmatrix fortschreiben
        let tx = w_em * fs * self.gs.h_scale
            + self.gs.char_space * bytes.len() as f64
            + self.gs.word_space * bytes.iter().filter(|b| **b == b' ').count() as f64;
        self.tm = compose(Affine::translate(v(tx, 0.0)), self.tm);
        let _ = fs;
    }

    /// Schließt den laufenden Textlauf als eine Text-Entity ab.
    fn flush_text(&mut self) {
        let Some(acc) = self.text.take() else {
            return;
        };
        let mut text = String::new();
        for r in &acc.runs {
            text.push_str(&r.text);
        }
        if text.trim().is_empty() {
            return;
        }
        let erster = acc.runs.first().cloned().unwrap_or_default();
        let farbe = acc.color;
        let kind = EntityKind::Text {
            p: acc.start,
            text,
            height: acc.height_mm.max(0.1),
            angle: acc.angle,
            align: TextAlignH::Left,
            valign: TextAlignV::Bottom,
            bold: erster.bold,
            italic: erster.italic,
            underline: erster.underline,
            superscript: erster.superscript,
            runs: acc.runs,
            font: acc.font,
            line_spacing: 1.0,
        };
        self.push_color(kind, None, farbe);
    }

    // ---------------- Malen ----------------

    /// Objekt mit eigener Farbe (Text nutzt die Füllfarbe).
    fn push_color(&mut self, kind: EntityKind, fill: Option<Color>, color: Color) {
        if self.out.len() >= MAX_ENTITIES_PER_PAGE {
            return;
        }
        let breite = (self.gs.line_width * self.scale()).max(0.05);
        self.out.push(ImportedEntity {
            kind,
            color,
            width: breite,
            line_type: self.gs.dash.unwrap_or(LineType::Solid),
            fill,
        });
    }

    fn stroke(&mut self, _breite: f64) {
        let subs = self.path.take();
        let color = self.gs.stroke;
        let breite = (self.gs.line_width * self.scale()).max(0.05);
        let lt = self.gs.dash.unwrap_or(LineType::Solid);
        for sub in subs {
            if sub.pts.len() < 2 {
                continue;
            }
            let kind = match path::classify(&sub) {
                GeoShape::Line(a, b) => EntityKind::Line { a, b },
                GeoShape::Circle(c, r) => EntityKind::Circle { c, r },
                GeoShape::Arc(c, r, a0, a1) => EntityKind::Arc { c, r, a0, a1 },
                GeoShape::Poly(pts, closed) => EntityKind::Polyline {
                    verts: pts.iter().map(|p| Vertex::new(*p)).collect(),
                    closed,
                },
            };
            if self.out.len() >= MAX_ENTITIES_PER_PAGE {
                self.warn.add("Obergrenze je Seite erreicht");
                return;
            }
            self.out.push(ImportedEntity {
                kind,
                color,
                width: breite,
                line_type: lt,
                fill: None,
            });
        }
    }

    fn fill_only(&mut self) {
        let subs = self.path.take();
        self.emit_fills(subs);
    }

    fn fill_and_stroke(&mut self) {
        let subs = self.path.take();
        let farbe = self.gs.fill;
        self.emit_fills(subs.clone());
        // Umriss zusätzlich als Strich
        let color = self.gs.stroke;
        let breite = (self.gs.line_width * self.scale()).max(0.05);
        let lt = self.gs.dash.unwrap_or(LineType::Solid);
        for sub in subs {
            if sub.pts.len() < 2 {
                continue;
            }
            let kind = match path::classify(&sub) {
                GeoShape::Line(a, b) => EntityKind::Line { a, b },
                GeoShape::Circle(c, r) => EntityKind::Circle { c, r },
                GeoShape::Arc(c, r, a0, a1) => EntityKind::Arc { c, r, a0, a1 },
                GeoShape::Poly(pts, closed) => EntityKind::Polyline {
                    verts: pts.iter().map(|p| Vertex::new(*p)).collect(),
                    closed,
                },
            };
            self.out.push(ImportedEntity {
                kind,
                color,
                width: breite,
                line_type: lt,
                fill: None,
            });
        }
        let _ = farbe;
    }

    fn emit_fills(&mut self, subs: Vec<path::SubPath>) {
        let farbe = self.gs.fill;
        let ringe: Vec<Vec<Vec2>> = subs
            .into_iter()
            .filter(|s| s.pts.len() >= 3)
            .map(|s| s.pts)
            .collect();
        for (outer, holes) in path::fill_rings(ringe) {
            if self.out.len() >= MAX_ENTITIES_PER_PAGE {
                self.warn.add("Obergrenze je Seite erreicht");
                return;
            }
            self.out.push(ImportedEntity {
                kind: EntityKind::Fill {
                    boundary: outer,
                    holes,
                    pattern: FillPattern::Solid,
                    pattern_angle: 0.0,
                    pattern_scale: 100.0,
                    border: false,
                },
                color: farbe,
                width: (self.gs.line_width * self.scale()).max(0.05),
                line_type: self.gs.dash.unwrap_or(LineType::Solid),
                fill: Some(farbe),
            });
        }
    }

    // ---------------- Form-XObjects ----------------

    fn do_xobject(&mut self, name: &str, res: &Dictionary) {
        let Some(dict) = lookup_res(self.doc, res, b"XObject", name.as_bytes()) else {
            return;
        };
        let subtype = dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default();
        if subtype == "Image" {
            self.warn.add("Bild nicht übernommen");
            return;
        }
        if subtype != "Form" {
            return;
        }
        if self.depth >= 8 {
            self.warn.add("Formular zu tief verschachtelt");
            return;
        }
        // Matrix und Inhalt aus dem Stream
        let Some(stream) = lookup_stream(self.doc, res, b"XObject", name.as_bytes()) else {
            return;
        };
        let Some(bytes) = stream.decompressed_content().ok() else {
            return;
        };
        let form_dict = stream.dict.clone();
        let matrix = match form_dict.get(b"Matrix") {
            Ok(Object::Array(a)) if a.len() >= 6 => {
                let g = |i: usize| a[i].as_float().ok().map(|f| f as f64).unwrap_or(0.0);
                Affine {
                    a: g(0),
                    b: g(1),
                    c: g(2),
                    d: g(3),
                    tx: g(4),
                    ty: g(5),
                }
            }
            _ => Affine::IDENTITY,
        };
        let gespeichert = self.gs.clone();
        self.gs.ctm = compose(matrix, self.gs.ctm);
        let inner = match form_dict.get(b"Resources") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(id)) => self.doc.get_dictionary(*id).cloned().unwrap_or(res.clone()),
            _ => res.clone(),
        };
        self.fonts.clear();
        self.depth += 1;
        self.exec(&bytes, &inner);
        self.flush_text();
        self.depth -= 1;
        self.fonts.clear();
        self.gs = gespeichert;
    }
}

/// Wörterbuch aus `/Resources /<bereich> /<name>`.
fn lookup_res(doc: &Document, res: &Dictionary, bereich: &[u8], name: &[u8]) -> Option<Dictionary> {
    let bereich_obj = res.get(bereich).ok()?;
    let bereich_obj = match bereich_obj {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        o => o,
    };
    let d = bereich_obj.as_dict().ok()?;
    let ziel = d.get(name).ok()?;
    let ziel = match ziel {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        o => o,
    };
    ziel.as_dict().ok().cloned()
}

/// Stream aus `/Resources /<bereich> /<name>`.
fn lookup_stream(
    doc: &Document,
    res: &Dictionary,
    bereich: &[u8],
    name: &[u8],
) -> Option<Stream> {
    let bereich_obj = res.get(bereich).ok()?;
    let bereich_obj = match bereich_obj {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        o => o,
    };
    let d = bereich_obj.as_dict().ok()?;
    let ziel = d.get(name).ok()?;
    let ziel = match ziel {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        o => o,
    };
    match ziel {
        Object::Stream(s) => Some(s.clone()),
        _ => None,
    }
}
