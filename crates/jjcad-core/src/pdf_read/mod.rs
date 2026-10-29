//! PDF-Import: ein PDF als **ein Element** (Block) einlesen.
//!
//! Die Fassade dieses Moduls liest Seitenmaß, Lage und Drehung, ruft den
//! Interpreter und gibt Entities in Welt-Millimetern zurück. Was PDF nicht
//! hergibt (Bilder, Transparenz, Schattierungen, Muster, Beschnitt), wird
//! gezählt und gemeldet — nicht stillschweigend verschluckt.

mod font;
mod interp;
mod path;
mod style;
mod token;

use crate::geom::{Affine, Vec2};
use crate::model::EntityKind;
use crate::style::{Color, LineType};
use lopdf::{Dictionary, Document, Object};
use std::collections::BTreeMap;
use std::path::Path;

/// Punkt → Millimeter.
pub const MM_PER_PT: f64 = 25.4 / 72.0;

/// Obergrenze je Seite: ein importiertes A3-Blatt kann zehntausende Objekte
/// haben. Ohne Grenze hängt die Anzeige, deshalb bricht der Leser ab und sagt
/// es (Plan Fallstrick 11).
pub const MAX_ENTITIES_PER_PAGE: usize = 50_000;

/// Ein übernommenes Objekt samt Aussehen.
#[derive(Clone, Debug)]
pub struct ImportedEntity {
    pub kind: EntityKind,
    pub color: Color,
    /// Linienstärke in mm (Papier).
    pub width: f64,
    pub line_type: LineType,
    /// Füllfarbe, wenn das Objekt gefüllt wird.
    pub fill: Option<Color>,
}

/// Was beim Import nicht übernommen wurde.
#[derive(Clone, Debug, Default)]
pub struct Warnings {
    pub counts: BTreeMap<String, usize>,
}

impl Warnings {
    pub fn add(&mut self, art: &str) {
        *self.counts.entry(art.to_string()).or_insert(0) += 1;
    }

    pub fn merge(&mut self, other: &Warnings) {
        for (k, v) in &other.counts {
            *self.counts.entry(k.clone()).or_insert(0) += v;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Lesbare Zeile für Konsole und Meldung.
    pub fn summary(&self) -> String {
        if self.counts.is_empty() {
            return String::new();
        }
        self.counts
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Eine gelesene Seite.
pub struct PageImport {
    /// 1-basiert, wie im PDF.
    pub number: usize,
    pub width_mm: f64,
    pub height_mm: f64,
    pub entities: Vec<ImportedEntity>,
    pub warnings: Warnings,
}

/// Ergebnis eines PDF-Imports.
pub struct PdfImport {
    pub title: Option<String>,
    pub pages: Vec<PageImport>,
}

impl PdfImport {
    pub fn page(&self, number: usize) -> Option<&PageImport> {
        self.pages.iter().find(|p| p.number == number)
    }
}

/// Liest ein PDF vollständig (alle Seiten).
pub fn read_pdf(path: &Path) -> Result<PdfImport, String> {
    let doc = Document::load(path).map_err(|e| format!("PDF nicht lesbar: {e}"))?;
    if doc.is_encrypted() {
        return Err("Das PDF ist verschlüsselt — bitte ohne Schutz exportieren".into());
    }
    let title = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|o| match o {
            Object::Reference(id) => doc.get_object(*id).ok(),
            other => Some(other),
        })
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"Title").ok())
        .and_then(|t| match t {
            Object::String(s, _) => Some(decode_pdf_string(s)),
            _ => None,
        });

    let mut pages = Vec::new();
    for (nr, page_id) in doc.get_pages() {
        pages.push(read_page(&doc, nr as usize, page_id));
    }
    if pages.is_empty() {
        return Err("Das PDF enthält keine Seiten".into());
    }
    Ok(PdfImport { title, pages })
}

/// `/Title` kann UTF-16BE mit BOM oder PDFDocEncoding sein.
fn decode_pdf_string(s: &[u8]) -> String {
    if s.len() >= 2 && s[0] == 0xFE && s[1] == 0xFF {
        let units: Vec<u16> = s[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(s).into_owned()
}

fn read_page(doc: &Document, number: usize, page_id: lopdf::ObjectId) -> PageImport {
    let dict = doc.get_dictionary(page_id).cloned().unwrap_or_default();
    let (x0, y0, x1, y1) = page_box(&dict);
    let breite_pt = (x1 - x0).abs().max(1.0);
    let hoehe_pt = (y1 - y0).abs().max(1.0);
    let rot = dict
        .get(b"Rotate")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0)
        .rem_euclid(360);
    let (rot, bw, bh) = if rot == 90 || rot == 270 {
        (rot, hoehe_pt, breite_pt)
    } else {
        (rot, breite_pt, hoehe_pt)
    };

    // Erst untere linke Ecke auf den Ursprung, dann Drehung, dann in mm.
    let mut base = Affine::translate(Vec2::new(-x0, -y0));
    let dreh = match rot {
        90 => Affine {
            a: 0.0,
            b: -1.0,
            c: 1.0,
            d: 0.0,
            tx: 0.0,
            ty: breite_pt,
        },
        180 => Affine {
            a: -1.0,
            b: 0.0,
            c: 0.0,
            d: -1.0,
            tx: breite_pt,
            ty: hoehe_pt,
        },
        270 => Affine {
            a: 0.0,
            b: 1.0,
            c: -1.0,
            d: 0.0,
            tx: hoehe_pt,
            ty: 0.0,
        },
        _ => Affine::IDENTITY,
    };
    base = compose(dreh, base);
    base = compose(Affine::scale(MM_PER_PT, MM_PER_PT), base);

    let mut warnings = Warnings::default();
    let resources = page_resources(doc, &dict);
    let content = doc.get_page_content(page_id);
    let mut entities = interp::run(doc, &content, &resources, base, &mut warnings);
    unterstreichungen_zuordnen(&mut entities);

    PageImport {
        number,
        width_mm: bw * MM_PER_PT,
        height_mm: bh * MM_PER_PT,
        entities,
        warnings,
    }
}


/// PDF kennt keine Unterstreichung — sie ist ein dünnes Viereck unter dem Text.
///
/// Beim eigenen Export ist das genau ein Rechteck in Textfarbe direkt unter
/// der Grundlinie. Der Leser erkennt es wieder und macht daraus die
/// Auszeichnung „unterstrichen" (statt eines eigenen gefüllten Objekts). So
/// bleibt der Text nach dem Rundlauf als Text erhalten.
fn unterstreichungen_zuordnen(entities: &mut Vec<ImportedEntity>) {
    use crate::geom::BBox;
    // Kandidaten sammeln: flache, geschlossene Vierecke.
    let mut quads: Vec<(usize, BBox, Color)> = Vec::new();
    for (i, e) in entities.iter().enumerate() {
        if let EntityKind::Fill { boundary, .. } = &e.kind {
            if boundary.len() == 4 {
                let mut bb = BBox::EMPTY;
                for p in boundary {
                    bb.union(BBox::new(*p, *p));
                }
                if bb.is_valid() && bb.width() > 0.2 && bb.height() > 0.0 {
                    // Strichdicke ist ein Bruchteil der Textgröße (6 %), also
                    // höchstens ein Fünftel der Länge.
                    if bb.height() <= bb.width() * 0.2 {
                        quads.push((i, bb, e.color));
                    }
                }
            }
        }
    }
    let mut weg: Vec<usize> = Vec::new();
    for (idx, bb, color) in quads {
        let mut getroffen = false;
        for e in entities.iter_mut() {
            let EntityKind::Text {
                p, height, runs, ..
            } = &mut e.kind
            else {
                continue;
            };
            let h = *height;
            if h <= 0.0 || e.color != color {
                continue;
            }
            // Gleiche Grundlinie, direkt darunter (Strich liegt 12 % unter der
            // Grundlinie, Dicke 6 % der Größe)
            let band_ok = bb.max.y <= p.y + h * 0.05 && bb.min.y >= p.y - h * 0.35;
            if !band_ok {
                continue;
            }
            // Nur die Läufe unterstreichen, die über dem Viereck liegen —
            // sonst wäre plötzlich der ganze Text unterstrichen.
            let mut x = p.x;
            let mut einer = false;
            for r in runs.iter_mut() {
                let scale = if r.superscript {
                    crate::text::SCRIPT_SCALE
                } else {
                    1.0
                };
                let style = crate::fonts::FontStyle::from_flags(r.bold, r.italic);
                let w = crate::fonts::advance_em_str(style, &r.text) * h * scale;
                let (x0, x1) = (x, x + w);
                let ueberlappung = bb.max.x.min(x1) - bb.min.x.max(x0);
                if ueberlappung > w * 0.4 && ueberlappung > 0.0 {
                    r.underline = true;
                    einer = true;
                }
                x = x1;
            }
            if einer {
                getroffen = true;
                break;
            }
        }
        if getroffen {
            weg.push(idx);
        }
    }
    for i in weg.into_iter().rev() {
        entities.remove(i);
    }
}

/// `[x0 y0 x1 y1]` aus MediaBox/CropBox, notfalls A4.
fn page_box(dict: &Dictionary) -> (f64, f64, f64, f64) {
    for key in [b"MediaBox".as_slice(), b"CropBox"] {
        if let Ok(Object::Array(a)) = dict.get(key) {
            let v: Vec<f64> = a.iter().filter_map(|o| o.as_float().ok().map(|f| f as f64)).collect();
            if v.len() == 4 {
                let (mut x0, mut y0, mut x1, mut y1) = (v[0], v[1], v[2], v[3]);
                if x1 < x0 {
                    std::mem::swap(&mut x0, &mut x1);
                }
                if y1 < y0 {
                    std::mem::swap(&mut y0, &mut y1);
                }
                return (x0, y0, x1, y1);
            }
        }
    }
    (0.0, 0.0, 595.0, 842.0)
}

/// Ressourcen-Wörterbuch der Seite (mit Vererbung von den Seitenknoten).
pub fn page_resources(doc: &Document, dict: &Dictionary) -> Dictionary {
    let mut cur = dict.clone();
    for _ in 0..16 {
        if let Ok(o) = cur.get(b"Resources") {
            let resolved = match o {
                Object::Reference(id) => doc.get_object(*id).ok(),
                other => Some(other),
            };
            if let Some(Object::Dictionary(d)) = resolved {
                return d.clone();
            }
        }
        match cur.get(b"Parent").ok() {
            Some(Object::Reference(id)) => match doc.get_dictionary(*id) {
                Ok(d) => cur = d.clone(),
                Err(_) => break,
            },
            _ => break,
        }
    }
    Dictionary::new()
}

/// Verkettung zweier Abbildungen: erst `n`, dann `m`.
pub fn compose(m: Affine, n: Affine) -> Affine {
    Affine {
        a: m.a * n.a + m.c * n.b,
        b: m.b * n.a + m.d * n.b,
        c: m.a * n.c + m.c * n.d,
        d: m.b * n.c + m.d * n.d,
        tx: m.a * n.tx + m.c * n.ty + m.tx,
        ty: m.b * n.tx + m.d * n.ty + m.ty,
    }
}
