//! Schriften aus einem PDF: Zeichenzuordnung, Vorschübe, eingebettete Datei.
//!
//! Drei Fälle, die der Leser auseinanderhalten muss (Plan 3.6):
//!
//! 1. **Eigenes PDF** — `Type0`/`Identity-H` mit `ToUnicode` und eingebetteter
//!    TrueType-Datei: Code = Glyphennummer, Zeichen aus der CMap, Breite aus
//!    `/W`.
//! 2. **Fremdes PDF, Base-14** — einfache Schrift, 1 Byte je Zeichen,
//!    `WinAnsiEncoding`: Zeichen direkt, Breite aus `/Widths`.
//! 3. **Fremdes PDF, Type0 ohne `ToUnicode`** — Glyphennummern ohne
//!    Zeichenzuordnung: aus der eingebetteten Datei rückwärts zuordnen; hilft
//!    das nicht, bleibt der Lauf als „ohne Zeichenzuordnung" gezählt.

use crate::fonts::FontStyle;
use crate::text::fallback_advance_em;
use ab_glyph::{Font, FontRef, GlyphId, ScaleFont};
use lopdf::{Dictionary, Document, Object};
use std::collections::BTreeMap;

/// Eine im PDF benutzte Schrift, aufbereitet für den Leser.
pub struct FontInfo {
    /// Schriftname aus dem PDF (`/BaseFont`).
    #[allow(dead_code)]
    pub base: String,
    pub style: FontStyle,
    pub two_byte: bool,
    /// Breiten je Code in Em (aus `/Widths` bzw. `/W`).
    widths: BTreeMap<u16, f64>,
    /// Code → Zeichen (aus `/ToUnicode`).
    to_unicode: BTreeMap<u16, char>,
    /// Eingebettete TrueType-Datei, falls vorhanden.
    pub ttf: Option<Vec<u8>>,
    /// Ersatzbreite in Em, wenn weder `/Widths` noch Schrift vorliegen.
    default_width: f64,
    /// Zeichen aus `/Encoding` (nur 1-Byte-Schriften).
    encoding: BTreeMap<u8, char>,
}

impl FontInfo {
    /// Codewerte eines PDF-Strings — zwei Byte je Zeichen bei `Type0`.
    pub fn codes(&self, bytes: &[u8]) -> Vec<u16> {
        if self.two_byte {
            bytes
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect()
        } else {
            bytes.iter().map(|b| *b as u16).collect()
        }
    }

    /// Zeichen eines Codes; `None`, wenn das PDF keine Zuordnung hergibt.
    pub fn ch(&self, code: u16) -> Option<char> {
        if let Some(c) = self.to_unicode.get(&code) {
            return Some(*c);
        }
        if !self.two_byte {
            if let Some(c) = self.encoding.get(&(code as u8)) {
                return Some(*c);
            }
        }
        // Rückwärts aus der eingebetteten Schrift (Glyphennummer → Zeichen).
        if self.two_byte {
            if let Some(ttf) = &self.ttf {
                return reverse_lookup(ttf, code);
            }
        }
        None
    }

    pub fn decode(&self, bytes: &[u8]) -> (String, usize) {
        let mut s = String::new();
        let mut fehlend = 0;
        for code in self.codes(bytes) {
            match self.ch(code) {
                Some(c) => s.push(c),
                None => fehlend += 1,
            }
        }
        (s, fehlend)
    }

    /// Vorschub eines Codes in Em.
    pub fn advance(&self, code: u16) -> f64 {
        if let Some(w) = self.widths.get(&code) {
            return *w;
        }
        if let Some(ttf) = &self.ttf {
            if let Ok(font) = FontRef::try_from_slice(ttf) {
                let upem = font.units_per_em().unwrap_or(1000.0) as f64;
                let adv = font.as_scaled(upem as f32).h_advance(GlyphId(code)) as f64;
                if adv > 0.0 {
                    return adv / upem;
                }
            }
        }
        match self.ch(code) {
            Some(c) => fallback_advance_em(c),
            None => self.default_width,
        }
    }
}

/// Schrift eines Font-Wörterbuchs aufbereiten.
pub fn load(doc: &Document, dict: &Dictionary) -> FontInfo {
    let base = dict
        .get(b"BaseFont")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .unwrap_or_else(|| "?".into());
    let subtype = dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .unwrap_or_default();
    let two_byte = subtype == "Type0";
    let mut info = FontInfo {
        base: base.clone(),
        style: style_from_name(&base),
        two_byte,
        widths: BTreeMap::new(),
        to_unicode: BTreeMap::new(),
        ttf: None,
        default_width: 0.5,
        encoding: if two_byte {
            BTreeMap::new()
        } else {
            winansi_table()
        },
    };

    // --- eingebettete Datei und Kennzeichen (FontDescriptor) --------------
    let desc = if two_byte {
        descendant(doc, dict).and_then(|cid| get_dict(doc, &cid, b"FontDescriptor"))
    } else {
        get_dict(doc, dict, b"FontDescriptor")
    };
    if let Some(d) = &desc {
        if let Ok(flags) = d.get(b"Flags").and_then(|o| o.as_i64()) {
            let kursiv = flags & 64 != 0;
            let fett = flags & 262144 != 0;
            if kursiv || fett {
                info.style = FontStyle::from_flags(fett, kursiv);
            }
        }
        if let Ok(mw) = d.get(b"MissingWidth").and_then(|o| o.as_float()) {
            if mw > 0.0 {
                info.default_width = mw as f64 / 1000.0;
            }
        }
        if let Some(bytes) = embedded_font(doc, d) {
            info.ttf = Some(bytes);
        }
    }

    // --- Breiten ---------------------------------------------------------
    if two_byte {
        if let Some(cid) = descendant(doc, dict) {
            read_w_array(doc, cid.get(b"W").ok(), &mut info.widths);
            if let Ok(dw) = cid.get(b"DW").and_then(|o| o.as_i64()) {
                info.default_width = dw as f64 / 1000.0;
            }
        }
    } else {
        // /Widths + /FirstChar
        let first = dict
            .get(b"FirstChar")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0) as u16;
        if let Ok(Object::Array(arr)) = dict.get(b"Widths") {
            for (i, w) in arr.iter().enumerate() {
                if let Ok(v) = w.as_float() {
                    info.widths.insert(first + i as u16, v as f64 / 1000.0);
                }
            }
        }
        // /Encoding: Name oder Wörterbuch mit /Differences
        if let Ok(enc) = dict.get(b"Encoding") {
            match enc {
                Object::Name(n) => {
                    if n == b"MacRomanEncoding" {
                        info.encoding = macroman_table();
                    }
                }
                Object::Dictionary(d) => {
                    read_differences(doc, d, &mut info.encoding);
                }
                _ => {}
            }
        }
    }

    // --- Zeichenzuordnung -------------------------------------------------
    if let Some(bytes) = stream_of(doc, dict, b"ToUnicode") {
        info.to_unicode = parse_cmap(&bytes);
    }
    info
}

fn style_from_name(name: &str) -> FontStyle {
    let n = name.to_lowercase();
    let fett = n.contains("bold") || n.contains("black") || n.contains("heavy");
    let kursiv = n.contains("italic") || n.contains("oblique");
    FontStyle::from_flags(fett, kursiv)
}

fn get_dict(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<Dictionary> {
    let o = dict.get(key).ok()?;
    let o = resolve(doc, o)?;
    o.as_dict().ok().cloned()
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Object> {
    match o {
        Object::Reference(id) => doc.get_object(*id).ok(),
        other => Some(other),
    }
}

fn descendant(doc: &Document, dict: &Dictionary) -> Option<Dictionary> {
    let arr = dict.get(b"DescendantFonts").ok()?;
    let Object::Array(items) = resolve(doc, arr)? else {
        return None;
    };
    let first = items.first()?;
    resolve(doc, first)?.as_dict().ok().cloned()
}

/// Rohbytes der eingebetteten Schriftdatei (`/FontFile2` oder `/FontFile3`).
fn embedded_font(doc: &Document, desc: &Dictionary) -> Option<Vec<u8>> {
    for key in [b"FontFile2".as_slice(), b"FontFile3", b"FontFile"] {
        if let Ok(o) = desc.get(key) {
            if let Some(Object::Stream(s)) = resolve(doc, o) {
                if let Ok(bytes) = s.decompressed_content() {
                    return Some(bytes);
                }
            }
        }
        // Direkt als Stream im Wörterbuch (kommt bei fremden PDFs vor)
        if let Some(Object::Stream(s)) = desc.get(key).ok() {
            if let Ok(bytes) = s.decompressed_content() {
                return Some(bytes);
            }
        }
    }
    None
}

fn stream_of(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<Vec<u8>> {
    let o = dict.get(key).ok()?;
    match resolve(doc, o)? {
        Object::Stream(s) => s.decompressed_content().ok(),
        _ => None,
    }
}

/// `/W`-Array lesen: `[ 10 [w w w] 20 30 [w] ]`.
fn read_w_array(doc: &Document, obj: Option<&Object>, out: &mut BTreeMap<u16, f64>) {
    let Some(obj) = obj else { return };
    let Some(Object::Array(items)) = resolve(doc, obj) else {
        return;
    };
    let mut i = 0;
    while i < items.len() {
        let Some(start) = items[i].as_i64().ok() else {
            i += 1;
            continue;
        };
        match items.get(i + 1) {
            Some(Object::Array(ws)) => {
                for (k, w) in ws.iter().enumerate() {
                    if let Ok(v) = w.as_float() {
                        out.insert(start as u16 + k as u16, v as f64 / 1000.0);
                    }
                }
                i += 2;
            }
            Some(end) => {
                // Bereich von…bis mit einer Breite
                let Some(ende) = end.as_i64().ok() else {
                    i += 1;
                    continue;
                };
                let w = items
                    .get(i + 2)
                    .and_then(|o| o.as_float().ok())
                    .unwrap_or(500.0) as f64
                    / 1000.0;
                for c in start..=ende {
                    out.insert(c as u16, w);
                }
                i += 3;
            }
            None => break,
        }
    }
}

fn read_differences(doc: &Document, enc: &Dictionary, out: &mut BTreeMap<u8, char>) {
    let Ok(Object::Array(diff)) = enc.get(b"Differences") else {
        return;
    };
    let mut code: u8 = 0;
    for item in diff {
        match resolve(doc, item) {
            Some(Object::Integer(i)) => code = *i as u8,
            Some(Object::Name(name)) => {
                if let Some(c) = glyph_name_to_char(&String::from_utf8_lossy(name)) {
                    out.insert(code, c);
                }
                code = code.wrapping_add(1);
            }
            _ => {}
        }
    }
}

/// `/ToUnicode`-CMap lesen: `bfchar` und `bfrange`.
pub fn parse_cmap(data: &[u8]) -> BTreeMap<u16, char> {
    let text = String::from_utf8_lossy(data);
    let mut map = BTreeMap::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "beginbfchar" => {
                i += 1;
                while i + 1 < tokens.len() && tokens[i] != "endbfchar" {
                    let code = hex_u32(tokens[i]);
                    let ziel = hex_string(tokens[i + 1]);
                    if let (Some(code), Some(s)) = (code, ziel) {
                        if let Some(c) = s.chars().next() {
                            map.insert(code as u16, c);
                        }
                    }
                    i += 2;
                }
            }
            "beginbfrange" => {
                i += 1;
                while i + 2 < tokens.len() && tokens[i] != "endbfrange" {
                    let von = hex_u32(tokens[i]);
                    let bis = hex_u32(tokens[i + 1]);
                    let ziel = hex_string(tokens[i + 2]);
                    if let (Some(von), Some(bis), Some(s)) = (von, bis, ziel) {
                        if let Some(start) = s.chars().next() {
                            for off in 0..=(bis.saturating_sub(von)) {
                                let c = char::from_u32(start as u32 + off);
                                if let Some(c) = c {
                                    map.insert((von + off) as u16, c);
                                }
                            }
                        }
                    }
                    i += 3;
                }
            }
            _ => i += 1,
        }
    }
    map
}

fn hex_u32(t: &str) -> Option<u32> {
    let t = t.trim_matches(|c| c == '<' || c == '>');
    u32::from_str_radix(t, 16).ok()
}

fn hex_string(t: &str) -> Option<String> {
    let t = t.trim_matches(|c| c == '<' || c == '>');
    if t.len() % 4 != 0 {
        return None;
    }
    let mut units = Vec::new();
    let mut i = 0;
    while i + 4 <= t.len() {
        units.push(u16::from_str_radix(&t[i..i + 4], 16).ok()?);
        i += 4;
    }
    Some(String::from_utf16_lossy(&units))
}

/// Glyphennummer → Zeichen aus der eingebetteten Datei (Rückweg ohne CMap).
///
/// Wird nur gebraucht, wenn ein fremdes PDF keine `ToUnicode`-Zuordnung hat.
fn reverse_lookup(ttf: &[u8], gid: u16) -> Option<char> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<u64, BTreeMap<u16, char>>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let key = ttf.len() as u64 * 1_000_003 + ttf.first().copied().unwrap_or(0) as u64;
    let mut guard = cache.lock().ok()?;
    let map = guard.entry(key).or_insert_with(|| {
        let mut m = BTreeMap::new();
        if let Ok(font) = FontRef::try_from_slice(ttf) {
            for cp in 0x20u32..=0x2FFF {
                if let Some(c) = char::from_u32(cp) {
                    let g = font.glyph_id(c).0;
                    if g != 0 {
                        m.entry(g).or_insert(c);
                    }
                }
            }
            for cp in [0x2013u32, 0x2014, 0x201E, 0x201C, 0x201D, 0x20AC, 0x2300, 0x2264, 0x2265] {
                if let Some(c) = char::from_u32(cp) {
                    let g = font.glyph_id(c).0;
                    if g != 0 {
                        m.entry(g).or_insert(c);
                    }
                }
            }
        }
        m
    });
    map.get(&gid).copied()
}

/// WinAnsi (CP1252) — die Kodierung, die Base-14-Schriften meistens nutzen.
fn winansi_table() -> BTreeMap<u8, char> {
    let mut m = BTreeMap::new();
    for b in 0x20u8..=0x7Eu8 {
        m.insert(b, b as char);
    }
    // 0x80…0x9F: CP1252-Sonderzeichen
    let hoch = [
        0x20AC, 0x81, 0x201A, 0x192, 0x201E, 0x2026, 0x2020, 0x2021, 0x2C6, 0x2030, 0x160, 0x2039,
        0x152, 0x8D, 0x17D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
        0x2DC, 0x2122, 0x161, 0x203A, 0x153, 0x9D, 0x17E, 0x178,
    ];
    for (i, cp) in hoch.iter().enumerate() {
        if let Some(c) = char::from_u32(*cp) {
            m.insert(0x80 + i as u8, c);
        }
    }
    // 0xA0…0xFF: ISO-8859-1 (WinAnsi weicht nur beim weichen Trennstrich ab)
    for b in 0xA0u8..=0xFFu8 {
        if let Some(c) = char::from_u32(b as u32) {
            m.insert(b, c);
        }
    }
    m
}

/// MacRoman ist nur grob genähert — fremde PDFs mit dieser Kodierung sind
/// selten; Umlaute und Grundzeichen stimmen.
fn macroman_table() -> BTreeMap<u8, char> {
    let mut m = BTreeMap::new();
    for b in 0x20u8..=0x7Eu8 {
        m.insert(b, b as char);
    }
    let hoch = [
        0xC4, 0xC5, 0xC7, 0xC9, 0xD1, 0xD6, 0xDC, 0xE1, 0xE0, 0xE2, 0xE4, 0xE3, 0xE5, 0xE7, 0xE9,
        0xE8, 0xEA, 0xEB, 0xED, 0xEC, 0xEE, 0xEF, 0xF1, 0xF3, 0xF2, 0xF4, 0xF6, 0xF5, 0xFA, 0xF9,
        0xFB, 0xFC,
    ];
    for (i, cp) in hoch.iter().enumerate() {
        if let Some(c) = char::from_u32(*cp) {
            m.insert(0x80 + i as u8, c);
        }
    }
    m
}

/// Glyphenname → Zeichen (nur die häufigen Namen aus `/Differences`).
fn glyph_name_to_char(name: &str) -> Option<char> {
    let einfach = [
        ("space", ' '),
        ("period", '.'),
        ("comma", ','),
        ("hyphen", '-'),
        ("slash", '/'),
        ("colon", ':'),
        ("semicolon", ';'),
        ("parenleft", '('),
        ("parenright", ')'),
        ("bracketleft", '['),
        ("bracketright", ']'),
        ("germandbls", 'ß'),
        ("adieresis", 'ä'),
        ("odieresis", 'ö'),
        ("udieresis", 'ü'),
        ("Adieresis", 'Ä'),
        ("Odieresis", 'Ö'),
        ("Udieresis", 'Ü'),
        ("degree", '°'),
        ("plusminus", '±'),
        ("multiply", '×'),
        ("mu", 'µ'),
        ("Euro", '€'),
        ("endash", '–'),
        ("emdash", '—'),
        ("quotedblleft", '„'),
        ("quotedblright", '“'),
        ("quotesingle", '\''),
        ("quotedbl", '"'),
        ("zero", '0'),
        ("one", '1'),
        ("two", '2'),
        ("three", '3'),
        ("four", '4'),
        ("five", '5'),
        ("six", '6'),
        ("seven", '7'),
        ("eight", '8'),
        ("nine", '9'),
    ];
    for (n, c) in einfach {
        if n == name {
            return Some(c);
        }
    }
    if name.len() == 1 {
        return name.chars().next();
    }
    if let Some(rest) = name.strip_prefix("uni") {
        if let Ok(cp) = u32::from_str_radix(rest, 16) {
            return char::from_u32(cp);
        }
    }
    if let Some(rest) = name.strip_prefix('u') {
        if let Ok(cp) = u32::from_str_radix(rest, 16) {
            return char::from_u32(cp);
        }
    }
    None
}
