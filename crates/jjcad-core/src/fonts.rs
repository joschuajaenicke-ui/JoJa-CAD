//! Schriften: die eine Stelle, an der Schriftschnitte gefunden und vermessen
//! werden.
//!
//! Anzeige (`raster`), PDF-Schreiber und PDF-Import müssen dieselben Vorschübe
//! benutzen, sonst sitzt Text in der einen Ausgabe anders als in der anderen.
//! Deshalb liegt die Auswahl der Dateien und die Vermessung hier zentral.
//!
//! Bevorzugt wird die Familie **Liberation Sans**: sie ist metrisch identisch
//! mit Helvetica und Arial, also passen alte Ausgaben (Helvetica) und neue
//! (eingebettete Schrift) zusammen.

use ab_glyph::{Font, FontRef, GlyphId, ScaleFont};
use std::sync::OnceLock;

/// Schriftschnitt eines Textlaufs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

    /// PostScript-Name, wie er im PDF als `/BaseFont` steht.
    pub fn ps_name(self) -> &'static str {
        match self {
            FontStyle::Regular => "LiberationSans",
            FontStyle::Bold => "LiberationSans-Bold",
            FontStyle::Italic => "LiberationSans-Italic",
            FontStyle::BoldItalic => "LiberationSans-BoldItalic",
        }
    }

    /// Suchpfade, in Reihenfolge. Die erste vorhandene Datei gewinnt.
    fn candidates(self) -> &'static [&'static str] {
        match self {
            FontStyle::Regular => &[
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                "C:\\Windows\\Fonts\\arial.ttf",
                "/Library/Fonts/Arial.ttf",
            ],
            FontStyle::Bold => &[
                "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
                "C:\\Windows\\Fonts\\arialbd.ttf",
                "/Library/Fonts/Arial Bold.ttf",
            ],
            FontStyle::Italic => &[
                "/usr/share/fonts/truetype/liberation/LiberationSans-Italic.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-Italic.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Oblique.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Italic.ttf",
                "C:\\Windows\\Fonts\\ariali.ttf",
                "/Library/Fonts/Arial Italic.ttf",
            ],
            FontStyle::BoldItalic => &[
                "/usr/share/fonts/truetype/liberation/LiberationSans-BoldItalic.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-BoldItalic.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-BoldOblique.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-BoldOblique.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-BoldItalic.ttf",
                "C:\\Windows\\Fonts\\arialbi.ttf",
                "/Library/Fonts/Arial Bold Italic.ttf",
            ],
        }
    }
}

/// Gelesene Schriftdatei — einmal je Schnitt, danach aus dem Zwischenspeicher.
struct Loaded {
    data: Vec<u8>,
    info: TtfInfo,
}

static CACHE: OnceLock<[Option<Loaded>; 4]> = OnceLock::new();

fn cache() -> &'static [Option<Loaded>; 4] {
    CACHE.get_or_init(|| {
        let lade = |st: FontStyle| -> Option<Loaded> {
            for p in st.candidates() {
                if let Ok(data) = std::fs::read(p) {
                    if let Ok(info) = TtfInfo::parse(&data) {
                        return Some(Loaded { data, info });
                    }
                }
            }
            // Notfall: irgendeine Sans-Schrift im Standardsystem.
            for dir in [
                "/usr/share/fonts/truetype/liberation",
                "/usr/share/fonts/truetype/dejavu",
            ] {
                let Ok(entries) = std::fs::read_dir(dir) else {
                    continue;
                };
                for e in entries.flatten() {
                    let fp = e.path();
                    let Some(name) = fp.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    let passt = match st {
                        FontStyle::Regular => name.contains("Regular") || name == "DejaVuSans.ttf",
                        FontStyle::Bold => name.contains("Bold") && !name.contains("Italic"),
                        FontStyle::Italic => name.contains("Italic") || name.contains("Oblique"),
                        FontStyle::BoldItalic => {
                            name.contains("Bold") && (name.contains("Italic") || name.contains("Oblique"))
                        }
                    };
                    if passt && name.ends_with(".ttf") {
                        if let Ok(data) = std::fs::read(&fp) {
                            if let Ok(info) = TtfInfo::parse(&data) {
                                return Some(Loaded { data, info });
                            }
                        }
                    }
                }
            }
            None
        };
        [
            lade(FontStyle::Regular),
            lade(FontStyle::Bold),
            lade(FontStyle::Italic),
            lade(FontStyle::BoldItalic),
        ]
    })
}

/// Rohdaten der Schriftdatei (TrueType) für den gewählten Schnitt.
pub fn ttf_bytes(style: FontStyle) -> Option<&'static [u8]> {
    cache()[style.index()].as_ref().map(|l| l.data.as_slice())
}

/// Kennwerte der Schriftdatei (Einheiten, Rahmen, Auf-/Abstieg).
pub fn ttf_info(style: FontStyle) -> Option<&'static TtfInfo> {
    cache()[style.index()].as_ref().map(|l| &l.info)
}

/// Arbeitet mit der geladenen Schrift (Vermessung, Glyphennummern).
pub fn with_font<R>(style: FontStyle, f: impl FnOnce(&FontRef<'_>) -> R) -> Option<R> {
    let data = ttf_bytes(style)?;
    let font = FontRef::try_from_slice(data).ok()?;
    Some(f(&font))
}

/// Vorschub eines Zeichens in Em (1.0 = Schrifthöhe).
pub fn advance_em(style: FontStyle, c: char) -> f64 {
    with_font(style, |f| {
        let upem = f.units_per_em().unwrap_or(1000.0) as f64;
        let adv = f.as_scaled(upem as f32).h_advance(f.glyph_id(c)) as f64;
        adv / upem
    })
    .unwrap_or_else(|| crate::text::fallback_advance_em(c))
}

/// Summe der Vorschübe eines Textes in Em.
pub fn advance_em_str(style: FontStyle, s: &str) -> f64 {
    with_font(style, |f| {
        let upem = f.units_per_em().unwrap_or(1000.0) as f64;
        let scaled = f.as_scaled(upem as f32);
        s.chars()
            .map(|c| scaled.h_advance(f.glyph_id(c)) as f64 / upem)
            .sum()
    })
    .unwrap_or_else(|| s.chars().map(crate::text::fallback_advance_em).sum())
}

/// Glyphennummer im Font (für Identity-H-Sätze im PDF).
pub fn glyph_id(style: FontStyle, c: char) -> u16 {
    with_font(style, |f| f.glyph_id(c).0).unwrap_or(0)
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
            let table_off = u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]) as usize;
            match tag {
                b"head" => head = Some(table_off),
                b"OS/2" => os2 = Some(table_off),
                b"hhea" => hhea = Some(table_off),
                _ => {}
            }
        }
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
