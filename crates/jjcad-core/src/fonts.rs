//! Schriften: die eine Stelle, an der Schriftfamilien und -schnitte gefunden
//! und vermessen werden.
//!
//! Anzeige (`raster`), PDF-Schreiber und PDF-Import müssen dieselben Vorschübe
//! benutzen, sonst sitzt Text in der einen Ausgabe anders als in der anderen.
//! Deshalb liegt die Auswahl der Dateien und die Vermessung hier zentral.
//!
//! **Keine Schriftdateien im Programm:** alle Familien werden zur Laufzeit aus
//! dem System geladen. Das hält die Programmdatei klein; in ein PDF kommt nur
//! die Schrift, die der Plan tatsächlich benutzt.
//!
//! Bevorzugt wird die Familie **Liberation**, weil sie metrisch identisch mit
//! Helvetica/Arial/Times/Courier ist: alte Ausgaben (Helvetica) und neue
//! (eingebettete Schrift) passen zusammen.

use crate::model::TextFont;
use ab_glyph::{Font, FontRef, GlyphId, ScaleFont};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Schriftschnitt eines Textlaufs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontStyle {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FontStyle {
    pub const ALL: [FontStyle; 4] = [
        FontStyle::Regular,
        FontStyle::Bold,
        FontStyle::Italic,
        FontStyle::BoldItalic,
    ];

    pub fn index(self) -> usize {
        match self {
            FontStyle::Regular => 0,
            FontStyle::Bold => 1,
            FontStyle::Italic => 2,
            FontStyle::BoldItalic => 3,
        }
    }

    pub fn from_flags(bold: bool, italic: bool) -> FontStyle {
        match (bold, italic) {
            (true, true) => FontStyle::BoldItalic,
            (true, false) => FontStyle::Bold,
            (false, true) => FontStyle::Italic,
            (false, false) => FontStyle::Regular,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            FontStyle::Regular => "",
            FontStyle::Bold => "-Bold",
            FontStyle::Italic => "-Italic",
            FontStyle::BoldItalic => "-BoldItalic",
        }
    }
}

/// Dateinamen (ohne Endung) einer Familie je Schnitt, in Vorzugsreihenfolge.
fn bases(font: TextFont) -> &'static [(&'static str, [&'static str; 4])] {
    match font {
        TextFont::Sans => &[
            ("LiberationSans", ["-Regular", "-Bold", "-Italic", "-BoldItalic"]),
            ("DejaVuSans", ["", "-Bold", "-Oblique", "-BoldOblique"]),
            ("NotoSans", ["-Regular", "-Bold", "-Italic", "-BoldItalic"]),
            ("FreeSans", ["", "-Bold", "-Oblique", "-BoldOblique"]),
        ],
        TextFont::Serif => &[
            ("LiberationSerif", ["-Regular", "-Bold", "-Italic", "-BoldItalic"]),
            ("DejaVuSerif", ["", "-Bold", "-Italic", "-BoldItalic"]),
            ("NimbusRoman", ["", "-Bold", "-Italic", "-BoldItalic"]),
            ("FreeSerif", ["", "-Bold", "-Italic", "-BoldItalic"]),
        ],
        TextFont::Mono => &[
            ("LiberationMono", ["-Regular", "-Bold", "-Italic", "-BoldItalic"]),
            ("DejaVuSansMono", ["", "-Bold", "-Oblique", "-BoldOblique"]),
            ("NimbusMonoPS", ["", "-Bold", "-Italic", "-BoldItalic"]),
            ("FreeMono", ["", "-Bold", "-Oblique", "-BoldOblique"]),
        ],
        TextFont::Narrow => &[
            ("LiberationSansNarrow", ["-Regular", "-Bold", "-Italic", "-BoldItalic"]),
            ("NimbusSansNarrow", ["", "-Bold", "-Italic", "-BoldItalic"]),
            ("LiberationSans", ["-Regular", "-Bold", "-Italic", "-BoldItalic"]),
        ],
        TextFont::Rund => &[
            // Quicksand hat nur Regular/Bold — kursiv gibt es nicht; dann wird
            // der Grundschnitt genommen (Lage bleibt, die Schräge fehlt).
            ("Quicksand", ["-Regular", "-Bold", "-Regular", "-Bold"]),
        ],
    }
}

/// Suchpfade für eine Familie und einen Schnitt — mit Rückfall auf den
/// Grundschnitt, falls es den Schnitt nicht gibt.
fn candidates(font: TextFont, style: FontStyle) -> Vec<String> {
    const DIRS: [&str; 9] = [
        "/usr/share/fonts/truetype/liberation/",
        "/usr/share/fonts/liberation-sans/",
        "/usr/share/fonts/truetype/quicksand/",
        "/usr/share/fonts/truetype/dejavu/",
        "/usr/share/fonts/TTF/",
        "/usr/share/fonts/truetype/noto/",
        "/usr/share/fonts/freefont/",
        "C:\\Windows\\Fonts\\",
        "/Library/Fonts/",
    ];
    let b = bases(font);
    let mut namen: Vec<String> = b
        .iter()
        .map(|(base, suf)| format!("{base}{}", suf[style.index()]))
        .collect();
    // Manche Systeme nennen den Grundschnitt ohne Zusatz (`arial.ttf`).
    if style == FontStyle::Regular {
        for (base, suf) in b {
            let mit = format!("{base}{}", suf[0]);
            if mit.ends_with("-Regular") {
                namen.push(mit.replace("-Regular", ""));
            }
        }
    }
    // Grundschnitt derselben Familie, dann Sans als letzter Rückfall.
    for (base, suf) in b {
        namen.push(format!("{base}{}", suf[0]));
    }
    if font != TextFont::Sans {
        namen.push("LiberationSans".into());
        namen.push("DejaVuSans".into());
    }
    let mut pfade = Vec::with_capacity(namen.len() * DIRS.len());
    for n in namen {
        for d in DIRS {
            pfade.push(format!("{d}{n}.ttf"));
        }
    }
    pfade
}

/// Gelesene Schriftdatei — einmal je Familie und Schnitt.
struct Loaded {
    data: Vec<u8>,
    info: TtfInfo,
    /// Dateiname ohne Endung — steht im PDF als `/BaseFont`.
    ps_name: String,
}

static CACHE: OnceLock<Mutex<HashMap<usize, Option<&'static Loaded>>>> = OnceLock::new();

fn schluessel(font: TextFont, style: FontStyle) -> usize {
    font.index() * 4 + style.index()
}

/// Lädt (einmalig) und behält die Schrift im Speicher.
fn lade(font: TextFont, style: FontStyle) -> Option<&'static Loaded> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = schluessel(font, style);
    if let Ok(m) = cache.lock() {
        if let Some(v) = m.get(&key) {
            return *v;
        }
    }
    let geladen = candidates(font, style)
        .into_iter()
        .find_map(|pfad| {
            let data = std::fs::read(&pfad).ok()?;
            let info = TtfInfo::parse(&data).ok()?;
            let ps_name = std::path::Path::new(&pfad)
                .file_stem()
                .map(|s| s.to_string_lossy().replace(' ', ""))
                .unwrap_or_else(|| "Schrift".into());
            Some(Loaded {
                data,
                info,
                ps_name,
            })
        })
        .map(|l| &*Box::leak(Box::new(l)));
    if let Ok(mut m) = cache.lock() {
        m.insert(key, geladen);
    }
    geladen
}

/// Rohdaten der Schriftdatei für Familie und Schnitt.
pub fn ttf_bytes(font: TextFont, style: FontStyle) -> Option<&'static [u8]> {
    lade(font, style).map(|l| l.data.as_slice())
}

/// Kennwerte der Schriftdatei (Einheiten, Rahmen, Auf-/Abstieg).
pub fn ttf_info(font: TextFont, style: FontStyle) -> Option<&'static TtfInfo> {
    lade(font, style).map(|l| &l.info)
}

/// PostScript-Name der geladenen Datei (für `/BaseFont` im PDF).
pub fn ps_name(font: TextFont, style: FontStyle) -> String {
    lade(font, style)
        .map(|l| l.ps_name.clone())
        .unwrap_or_else(|| format!("{}{}", font.key(), style.suffix()))
}

/// Ist diese Familie auf diesem Rechner vorhanden?
pub fn verfuegbar(font: TextFont) -> bool {
    ttf_bytes(font, FontStyle::Regular).is_some()
}

/// Arbeitet mit der geladenen Schrift (Vermessung, Glyphennummern).
pub fn with_font<R>(
    font: TextFont,
    style: FontStyle,
    f: impl FnOnce(&FontRef<'_>) -> R,
) -> Option<R> {
    let data = ttf_bytes(font, style)?;
    let fref = FontRef::try_from_slice(data).ok()?;
    Some(f(&fref))
}

/// Vorschub eines Zeichens in Em (1.0 = Schrifthöhe).
pub fn advance_em(font: TextFont, style: FontStyle, c: char) -> f64 {
    with_font(font, style, |f| {
        let upem = f.units_per_em().unwrap_or(1000.0) as f64;
        let adv = f.as_scaled(upem as f32).h_advance(f.glyph_id(c)) as f64;
        adv / upem
    })
    .unwrap_or_else(|| crate::text::fallback_advance_em(c))
}

/// Summe der Vorschübe eines Textes in Em.
pub fn advance_em_str(font: TextFont, style: FontStyle, s: &str) -> f64 {
    with_font(font, style, |f| {
        let upem = f.units_per_em().unwrap_or(1000.0) as f64;
        let scaled = f.as_scaled(upem as f32);
        s.chars()
            .map(|c| scaled.h_advance(f.glyph_id(c)) as f64 / upem)
            .sum()
    })
    .unwrap_or_else(|| s.chars().map(crate::text::fallback_advance_em).sum())
}

/// Glyphennummer im Font (für Identity-H-Sätze im PDF).
pub fn glyph_id(font: TextFont, style: FontStyle, c: char) -> u16 {
    with_font(font, style, |f| f.glyph_id(c).0).unwrap_or(0)
}

/// Kennwerte aus dem TrueType-Kopf — ohne zusätzliche Abhängigkeit gelesen.
#[derive(Clone, Copy, Debug, Default)]
pub struct TtfInfo {
    pub units_per_em: f64,
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
    pub ascent: f64,
    pub descent: f64,
    pub cap_height: f64,
    pub italic_angle: f64,
    pub weight_class: u16,
    /// 0 = installierbar, 2 = nur einbetten, 4 = nicht einbetten
    pub fs_type: u16,
}

impl TtfInfo {
    /// Liest `head` und `OS/2`. Fehlt eine Tabelle, bleiben Näherungswerte.
    pub fn parse(data: &[u8]) -> Result<TtfInfo, String> {
        if data.len() < 12 {
            return Err("Schriftdatei zu kurz".into());
        }
        let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
        let mut head = None;
        let mut os2 = None;
        let mut hhea = None;
        for i in 0..num_tables {
            let off = 12 + i * 16;
            if off + 16 > data.len() {
                break;
            }
            let tag = &data[off..off + 4];
            let table_off = u32::from_be_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
            ]) as usize;
            match tag {
                b"head" => head = Some(table_off),
                b"OS/2" => os2 = Some(table_off),
                b"hhea" => hhea = Some(table_off),
                _ => {}
            }
        }
        // Ohne `head` ist es keine TrueType-Datei (z. B. CFF/OTF) — nicht nehmen.
        let Some(h) = head else {
            return Err("Schrift ohne 'head'-Tabelle".into());
        };
        let rd_i16 = |o: usize| -> f64 {
            if o + 2 <= data.len() {
                i16::from_be_bytes([data[o], data[o + 1]]) as f64
            } else {
                0.0
            }
        };
        let rd_u16 = |o: usize| -> u16 {
            if o + 2 <= data.len() {
                u16::from_be_bytes([data[o], data[o + 1]])
            } else {
                0
            }
        };
        let mut info = TtfInfo {
            units_per_em: rd_u16(h + 18).max(1) as f64,
            x_min: rd_i16(h + 36),
            y_min: rd_i16(h + 38),
            x_max: rd_i16(h + 40),
            y_max: rd_i16(h + 42),
            ..Default::default()
        };
        // Aufstieg/Abstieg aus hhea (übliche Quelle für PDF-Deskriptoren).
        if let Some(hh) = hhea {
            info.ascent = rd_i16(hh + 4);
            info.descent = rd_i16(hh + 6);
        }
        if info.ascent == 0.0 {
            info.ascent = info.units_per_em * 0.8;
        }
        if info.descent == 0.0 {
            info.descent = -info.units_per_em * 0.2;
        }
        info.cap_height = info.units_per_em * 0.7;
        if let Some(o) = os2 {
            let version = rd_u16(o);
            info.weight_class = rd_u16(o + 4);
            info.fs_type = rd_u16(o + 8);
            // sCapHeight steht ab Version 2 an Offset 88.
            if version >= 2 {
                let ch = rd_i16(o + 88);
                if ch > 0.0 {
                    info.cap_height = ch;
                }
            }
        }
        Ok(info)
    }
}

/// Schrift einer geladenen Datei (für den Import fremder PDFs).
pub fn font_from_bytes(data: &[u8]) -> Option<FontRef<'_>> {
    FontRef::try_from_slice(data).ok()
}

/// Glyphennummer zu einem Zeichen in einer beliebigen geladenen Schrift.
pub fn glyph_id_in(font: &FontRef<'_>, c: char) -> GlyphId {
    font.glyph_id(c)
}

/// Familie aus einem PDF-Schriftnamen raten (`LiberationSerif`,
/// `Quicksand-Bold`, `Courier` …). Der Import kennt die Schrift nicht anders.
pub fn font_from_ps_name(name: &str) -> TextFont {
    let n = name.to_lowercase();
    if n.contains("quicksand") {
        TextFont::Rund
    } else if n.contains("narrow") {
        TextFont::Narrow
    } else if n.contains("mono") || n.contains("courier") {
        TextFont::Mono
    } else if n.contains("serif") || n.contains("times") || n.contains("roman") {
        TextFont::Serif
    } else {
        TextFont::Sans
    }
}

