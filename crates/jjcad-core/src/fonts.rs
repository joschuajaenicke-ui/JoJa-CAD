//! Schriften: die eine Stelle, an der Schriftfamilien und -schnitte gefunden
//! und vermessen werden.
//!
//! Anzeige (`raster`), PDF-Schreiber und PDF-Import müssen dieselben Vorschübe
//! benutzen, sonst sitzt Text in der einen Ausgabe anders als in der anderen.
//! Deshalb liegt die Auswahl der Dateien und die Vermessung hier zentral.
//!
//! **Gebrauchsschriften aus dem System:** Serifenlos, Serifen, Nichtproportional,
//! Schmal und Rund werden zur Laufzeit geladen. Das hält die Programmdatei
//! klein; in ein PDF kommt nur die Schrift, die der Plan tatsächlich benutzt.
//!
//! **Technische Schriften liegen mit im Programm** (`crates/jjcad-core/fonts/`):
//! DIN 1451 (Eng-, Mittel-, Breitschrift) und EN ISO 3098 (Typ A, Typ B) gibt
//! es auf keinem üblichen Rechner, und aus einer Systemschrift lassen sie sich
//! nicht ableiten. Sie kommen über `include_bytes!` mit — dieselbe Schrift steht
//! damit in Tests, Rasterausgabe, PDF und GUI zur Verfügung, ohne dass jemand
//! Dateien nachinstalliert. Herkunft und Lizenzen: `fonts/LIESMICH.md`.
//!
//! Bevorzugt wird bei den Systemschriften die Familie **Liberation**, weil sie
//! metrisch identisch mit Helvetica/Arial/Times/Courier ist: alte Ausgaben
//! (Helvetica) und neue (eingebettete Schrift) passen zusammen.

use crate::model::TextFont;
use ab_glyph::{Font, FontRef, GlyphId};
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
        // Die technischen Schriften kommen aus `fonts/` (siehe [`gebuendelt`]).
        // Fehlt die Datei, bleibt die serifenlose Schrift als Rückfall — die
        // Namen stehen hier bewusst nicht, damit keine Systemschrift
        // versehentlich als DIN 1451 durchgeht.
        TextFont::DinEng
        | TextFont::DinMittel
        | TextFont::DinBreit
        | TextFont::IsoA
        | TextFont::IsoB => &[],
    }
}

/// Eine mitgelieferte Schriftdatei.
struct Gebuendelt {
    daten: &'static [u8],
    /// Waagerechte Streckung der Formen (1.0 = wie in der Datei).
    ///
    /// DIN 1451 ist **eine** Schrift in drei Breiten — Eng-, Mittel- und
    /// Breitschrift sind dieselben Formen mit anderem Breitenverhältnis
    /// (rund 0,82 : 1,0 : 1,25). Die mitgelieferten Dateien geben nur Eng- und
    /// Mittelschnitt her; die Breitschrift entsteht durch Strecken, sonst wäre
    /// sie von der Mittelschrift nicht zu unterscheiden (gemessen: 2 % breiter).
    streckung: f32,
    /// Name für `/BaseFont` im PDF. Je Familie verschieden, auch wenn zwei
    /// Familien dieselbe Datei benutzen: zwei PDF-Schriften mit gleichem Namen,
    /// aber eigenen Glyphentabellen verwirren den Betrachter.
    ps: &'static str,
}

/// Mitgelieferte technische Schrift (DIN 1451, EN ISO 3098) — sonst `None`.
///
/// `include_bytes!` statt Dateisuche: die Schriften sind Teil des Programms.
/// Zuordnung: Engschrift = D-DIN Condensed, Mittelschrift = D-DIN,
/// Breitschrift = D-DIN Expanded; EN ISO 3098 Typ A = osifont (dünn, d = h/14),
/// Typ B = D-DIN (d = h/10, formgleich mit der Mittelschrift).
fn gebuendelt(font: TextFont) -> Option<Gebuendelt> {
    Some(match font {
        TextFont::DinEng => Gebuendelt {
            daten: include_bytes!("../fonts/D-DINCondensed.ttf"),
            streckung: 1.0,
            ps: "D-DINCondensed",
        },
        TextFont::DinMittel => Gebuendelt {
            daten: include_bytes!("../fonts/D-DIN.ttf"),
            streckung: 1.0,
            ps: "D-DIN",
        },
        TextFont::DinBreit => Gebuendelt {
            daten: include_bytes!("../fonts/D-DIN.ttf"),
            streckung: BREITSCHRIFT_STRECKUNG,
            // Eigener Name: die OFL verbietet den reservierten Namen für
            // Abwandlungen, und gestreckt ist eine Abwandlung.
            ps: "JjcadDIN1451Breit",
        },
        TextFont::IsoA => Gebuendelt {
            daten: include_bytes!("../fonts/osifont-lgpl3fe.ttf"),
            streckung: 1.0,
            ps: "osifont",
        },
        TextFont::IsoB => Gebuendelt {
            daten: include_bytes!("../fonts/D-DIN.ttf"),
            streckung: 1.0,
            ps: "D-DIN-ISO3098-B",
        },
        _ => return None,
    })
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
    // Neben dem Programm mitgelieferte Schriften haben Vorrang — nur so ist
    // die Windows-Fassung ohne Systeminstallation von Liberation/Quicksand zu
    // gebrauchen.
    let mut dirs: Vec<String> = Vec::with_capacity(DIRS.len() + 1);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(eltern) = exe.parent() {
            let mut d = eltern.join("fonts").to_string_lossy().to_string();
            d.push(std::path::MAIN_SEPARATOR);
            dirs.push(d);
        }
    }
    dirs.extend(DIRS.iter().map(|d| (*d).to_string()));

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
    let mut pfade = Vec::with_capacity(namen.len() * dirs.len());
    for n in namen {
        for d in &dirs {
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
    // Mitgelieferte Schrift: fett/kursiv gibt es in der technischen Schrift
    // nicht, es bleibt beim Grundschnitt (gleiche Datei, gleiche Vorschübe).
    if let Some(g) = gebuendelt(font) {
        let daten = if (g.streckung - 1.0).abs() < 1e-3 {
            g.daten.to_vec()
        } else {
            strecke(g.daten, g.streckung).unwrap_or_else(|| g.daten.to_vec())
        };
        let geladen = TtfInfo::parse(&daten).ok().map(|info| Loaded {
            data: daten,
            info,
            ps_name: g.ps.to_string(),
        });
        let geladen = geladen.map(|l| &*Box::leak(Box::new(l)));
        if let Ok(mut m) = cache.lock() {
            m.insert(key, geladen);
        }
        return geladen;
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

/// Vorschub eines Zeichens in Em (1,0 = Em-Quadrat, also `units_per_em`).
///
/// Gelesen wird der **unskalierte** Vorschub der Datei: genau so setzen die
/// Zeichenfläche (`egui`) und der PDF-Leser die Zeichen. `as_scaled(x)`
/// bezieht Vorschübe dagegen auf Aufstieg − Abstieg statt auf das Em-Quadrat —
/// bei Liberation Sans wären alle Breiten damit 12 % zu schmal (T0110).
pub fn advance_em(font: TextFont, style: FontStyle, c: char) -> f64 {
    with_font(font, style, |f| {
        let upem = f.units_per_em().unwrap_or(1000.0) as f64;
        f.h_advance_unscaled(f.glyph_id(c)) as f64 / upem
    })
    .unwrap_or_else(|| crate::text::fallback_advance_em(c))
}

/// Summe der Vorschübe eines Textes in Em — dieselben Werte wie [`advance_em`].
pub fn advance_em_str(font: TextFont, style: FontStyle, s: &str) -> f64 {
    with_font(font, style, |f| {
        let upem = f.units_per_em().unwrap_or(1000.0) as f64;
        s.chars()
            .map(|c| f.h_advance_unscaled(f.glyph_id(c)) as f64 / upem)
            .sum()
    })
    .unwrap_or_else(|| s.chars().map(crate::text::fallback_advance_em).sum())
}

/// Zeilenhöhe einer Familie in Vielfachen der Schriftgröße (Em).
///
/// Aufstieg, Abstieg und Durchschuss der **unskalierten** Datei, geteilt durch
/// `units_per_em` — genau das Maß, mit dem die Zeichenfläche eine Zeile setzt.
/// Es hängt an der Schriftdatei (Serifenlos 1,150 / DIN 1451 1,086 / EN ISO 3098
/// Typ A 1,091 / Monospace 1,133) und ist deshalb je Familie verschieden: eine
/// feste Annahme (früher 1,4 für den Rahmen, 1,25 für Druck) trifft nur eine
/// davon (T0110).
pub fn line_height_em(font: TextFont, style: FontStyle) -> f64 {
    let wert = with_font(font, style, |f| {
        let upem = f.units_per_em().unwrap_or(1000.0) as f64;
        let (a, d, g) = (
            f.ascent_unscaled() as f64,
            f.descent_unscaled() as f64,
            f.line_gap_unscaled() as f64,
        );
        if upem <= 0.0 {
            return 1.2;
        }
        (a - d + g) / upem
    })
    .unwrap_or(1.2);
    wert.clamp(0.5, 4.0)
}

/// Versalhöhe einer Familie in Vielfachen der Schriftgröße (Em).
///
/// Die Höhe der Großbuchstaben („H") aus dem Kopf der Schriftdatei — das Maß,
/// um das der Zeilenkasten über und unter dem sichtbaren Text Luft hat. Der
/// Rahmen eines Textfelds bekommt links und rechts denselben Abstand
/// (T0126, Turn 5). Fehlt der Wert, gilt die übliche Annahme 0,7 Em.
pub fn cap_height_em(font: TextFont, style: FontStyle) -> f64 {
    ttf_info(font, style)
        .map(|i| {
            if i.units_per_em > 0.0 {
                (i.cap_height / i.units_per_em).clamp(0.3, 0.95)
            } else {
                0.7
            }
        })
        .unwrap_or(0.7)
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
    /// Durchschuss aus `hhea` (Zeilenvorschub zwischen zwei Zeilen).
    pub line_gap: f64,
    pub cap_height: f64,
    pub italic_angle: f64,
    pub weight_class: u16,
    /// 0 = installierbar, 2 = nur einbetten, 4 = nicht einbetten
    pub fs_type: u16,
}

impl TtfInfo {
    /// Zeilenhöhe der Schrift in Em: Aufstieg, Abstieg und Durchschuss.
    ///
    /// Dasselbe Maß, mit dem die Anzeige (egui über ab_glyph) eine Zeile setzt:
    /// `ascent − descent + line_gap`. Es hängt an der Datei und ist damit je
    /// Familie verschieden (EN ISO 3098 Typ A liegt bei 1,66 Em, DIN 1451 bei
    /// 1,09 Em) — der Textrahmen muss es deshalb je Schrift rechnen.
    pub fn line_height(&self) -> f64 {
        if self.units_per_em <= 0.0 {
            return 1.2;
        }
        (self.ascent - self.descent + self.line_gap) / self.units_per_em
    }

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
            info.line_gap = rd_i16(hh + 8);
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
    } else if n.contains("osifont") {
        TextFont::IsoA
    } else if n.contains("iso3098") {
        TextFont::IsoB
    } else if n.contains("condensed") {
        TextFont::DinEng
    } else if n.contains("breit") || n.contains("expanded") || n.contains("d-dinexp") {
        TextFont::DinBreit
    } else if n.contains("d-din") || n.contains("ddin") {
        TextFont::DinMittel
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

// ---------------------------------------------------------------------------
// Waagerechte Streckung einer TrueType-Datei
// ---------------------------------------------------------------------------
//
// DIN 1451 ist **eine** Schrift in drei Breiten: Eng-, Mittel- und Breitschrift
// sind dieselben Formen mit anderem Breitenverhältnis. Damit die Breitschrift
// nicht nur 2 % breiter ist als die Mittelschrift (gemessen an der Vorlage),
// wird die Mittelschrift beim Laden waagerecht gestreckt. Geschieht das an der
// Schriftdatei und nicht am Zeichner, wirkt es überall gleich: Zeichenfläche,
// Rasterausgabe, PDF und „in Linien zerlegen".

/// Breite der DIN 1451 Breitschrift gegenüber der Mittelschrift.
pub const BREITSCHRIFT_STRECKUNG: f32 = 1.25;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    Some(i16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

fn set_u16(d: &mut [u8], o: usize, v: u16) {
    d[o..o + 2].copy_from_slice(&v.to_be_bytes());
}

fn set_i16(d: &mut [u8], o: usize, v: i16) {
    d[o..o + 2].copy_from_slice(&v.to_be_bytes());
}

/// Summe der 32-Bit-Wörter (Prüfsumme einer sfnt-Tabelle).
fn pruefsumme(d: &[u8]) -> u32 {
    let mut s: u32 = 0;
    let mut i = 0;
    while i < d.len() {
        let mut wort = [0u8; 4];
        for (k, b) in wort.iter_mut().enumerate() {
            *b = d.get(i + k).copied().unwrap_or(0);
        }
        s = s.wrapping_add(u32::from_be_bytes(wort));
        i += 4;
    }
    s
}

/// Setzt die waagerechten Koordinaten einer Glyphe um `f` auseinander.
///
/// Die x-Werte werden als vorzeichenbehaftete 16-Bit-Deltas neu geschrieben
/// (die kurze 8-Bit-Form kann nach dem Strecken überlaufen), die
/// TrueType-Instruktionen fallen weg — nach einer Streckung wären sie falsch,
/// und keine der Ausgaben hintet.
fn strecke_glyphe(g: &[u8], f: f64) -> Option<Vec<u8>> {
    let nc = i16_at(g, 0)?;
    let mut out = Vec::with_capacity(g.len() + 64);
    out.extend_from_slice(g.get(..10)?);
    let runden = |v: f64| -> i16 { v.round().clamp(-32768.0, 32767.0) as i16 };

    if nc >= 0 {
        let n = nc as usize;
        let mut endp: Vec<u16> = Vec::with_capacity(n);
        let mut np = 0usize;
        for k in 0..n {
            let v = u16_at(g, 10 + 2 * k)?;
            endp.push(v);
            np = np.max(v as usize + 1);
        }
        let mut p = 10 + 2 * n;
        let il = u16_at(g, p)? as usize;
        p += 2 + il; // Instruktionen werden nicht übernommen
        let mut flags: Vec<u8> = Vec::with_capacity(np);
        while flags.len() < np {
            let fl = *g.get(p)?;
            p += 1;
            flags.push(fl);
            if fl & 0x08 != 0 {
                let r = *g.get(p)? as usize;
                p += 1;
                for _ in 0..r {
                    flags.push(fl);
                }
            }
        }
        flags.truncate(np);
        // x-Deltas (gestreckt) und y-Deltas (unverändert) auslesen
        let mut xs: Vec<f64> = Vec::with_capacity(np);
        let mut x = 0.0f64;
        for fl in &flags {
            let d = if fl & 0x02 != 0 {
                let v = *g.get(p)? as f64;
                p += 1;
                if fl & 0x10 != 0 {
                    v
                } else {
                    -v
                }
            } else if fl & 0x10 != 0 {
                0.0
            } else {
                let v = i16_at(g, p)? as f64;
                p += 2;
                v
            };
            x += d;
            xs.push(x);
        }
        let mut ydel: Vec<i16> = Vec::with_capacity(np);
        for fl in &flags {
            let d = if fl & 0x04 != 0 {
                let v = *g.get(p)? as i32;
                p += 1;
                if fl & 0x20 != 0 {
                    v
                } else {
                    -v
                }
            } else if fl & 0x20 != 0 {
                0
            } else {
                let v = i16_at(g, p)? as i32;
                p += 2;
                v
            };
            ydel.push(d.clamp(-32768, 32767) as i16);
        }
        // neue Glyphe zusammensetzen
        for e in &endp {
            out.extend_from_slice(&e.to_be_bytes());
        }
        out.extend_from_slice(&0u16.to_be_bytes()); // keine Instruktionen
        for fl in &flags {
            // Nur „auf der Kurve" und „Überlappung" behalten: kurze Vektoren
            // gibt es nach dem Neuschreiben nicht mehr.
            out.push(fl & 0x41);
        }
        let mut prev = 0i32;
        for x in &xs {
            let v = runden(x * f) as i32;
            out.extend_from_slice(&((v - prev).clamp(-32768, 32767) as i16).to_be_bytes());
            prev = v;
        }
        for d in &ydel {
            out.extend_from_slice(&d.to_be_bytes());
        }
        // Rahmen der Glyphe nachziehen
        let xmin = runden(i16_at(g, 2)? as f64 * f);
        let xmax = runden(i16_at(g, 6)? as f64 * f);
        set_i16(&mut out, 2, xmin);
        set_i16(&mut out, 6, xmax);
        return Some(out);
    }

    // Zusammengesetzte Glyphe: Versätze strecken, Bausteine unverändert.
    let mut p = 10usize;
    loop {
        let fl = u16_at(g, p)?;
        let gi = u16_at(g, p + 2)?;
        p += 4;
        let words = fl & 0x0001 != 0;
        let (a1, a2) = if words {
            let a = i16_at(g, p)? as i32;
            let b = i16_at(g, p + 2)? as i32;
            p += 4;
            (a, b)
        } else {
            let a = *g.get(p)? as i8 as i32;
            let b = *g.get(p + 1)? as i8 as i32;
            p += 2;
            (a, b)
        };
        let mut breite = 0usize;
        if fl & 0x0008 != 0 {
            breite = 2;
        } else if fl & 0x0040 != 0 {
            breite = 4;
        } else if fl & 0x0080 != 0 {
            breite = 8;
        }
        let matrix = g.get(p..p + breite)?.to_vec();
        p += breite;
        let dx = if fl & 0x0002 != 0 {
            (a1 as f64 * f).round() as i32
        } else {
            a1
        };
        out.extend_from_slice(&(fl | 0x0001).to_be_bytes());
        out.extend_from_slice(&gi.to_be_bytes());
        out.extend_from_slice(&(dx.clamp(-32768, 32767) as i16).to_be_bytes());
        out.extend_from_slice(&(a2.clamp(-32768, 32767) as i16).to_be_bytes());
        out.extend_from_slice(&matrix);
        if fl & 0x0020 == 0 {
            break;
        }
    }
    let xmin = runden(i16_at(g, 2)? as f64 * f);
    let xmax = runden(i16_at(g, 6)? as f64 * f);
    set_i16(&mut out, 2, xmin);
    set_i16(&mut out, 6, xmax);
    Some(out)
}

/// Setzt eine TrueType-Datei waagerecht um `faktor` auseinander.
fn strecke(data: &[u8], faktor: f32) -> Option<Vec<u8>> {
    let num = u16_at(data, 4)? as usize;
    let mut tabellen: Vec<([u8; 4], Vec<u8>)> = Vec::with_capacity(num);
    for i in 0..num {
        let o = 12 + i * 16;
        let tag = [data.get(o)?, data.get(o + 1)?, data.get(o + 2)?, data.get(o + 3)?].map(|b| *b);
        let off = u32_at(data, o + 8)? as usize;
        let len = u32_at(data, o + 12)? as usize;
        tabellen.push((tag, data.get(off..off + len)?.to_vec()));
    }
    let hole = |name: &[u8; 4]| -> Option<Vec<u8>> {
        tabellen
            .iter()
            .find(|(t, _)| t == name)
            .map(|(_, d)| d.clone())
    };
    let mut head = hole(b"head")?;
    let mut hhea = hole(b"hhea")?;
    let mut hmtx = hole(b"hmtx")?;
    let maxp = hole(b"maxp")?;
    let loca = hole(b"loca")?;
    let glyf = hole(b"glyf")?;

    let num_glyphs = u16_at(&maxp, 4)? as usize;
    let num_h = u16_at(&hhea, 34)? as usize;
    let lang = i16_at(&head, 50)? != 0;
    let mut alt_loca: Vec<usize> = Vec::with_capacity(num_glyphs + 1);
    for i in 0..=num_glyphs {
        let v = if lang {
            u32_at(&loca, 4 * i)? as usize
        } else {
            u16_at(&loca, 2 * i)? as usize * 2
        };
        if v > glyf.len() {
            return None;
        }
        alt_loca.push(v);
    }

    let f = faktor as f64;
    let mut neu_glyf: Vec<u8> = Vec::with_capacity(glyf.len() + 4096);
    let mut neu_loca: Vec<u32> = Vec::with_capacity(num_glyphs + 1);
    for i in 0..num_glyphs {
        let (a, b) = (alt_loca[i], alt_loca[i + 1]);
        neu_loca.push(neu_glyf.len() as u32);
        if b > a {
            let neu = strecke_glyphe(&glyf[a..b], f)?;
            neu_glyf.extend_from_slice(&neu);
            while neu_glyf.len() % 4 != 0 {
                neu_glyf.push(0);
            }
        }
    }
    neu_loca.push(neu_glyf.len() as u32);
    let mut neu_loca_bytes = Vec::with_capacity(4 * neu_loca.len());
    for v in &neu_loca {
        neu_loca_bytes.extend_from_slice(&v.to_be_bytes());
    }

    for i in 0..num_h.min(num_glyphs) {
        let o = 4 * i;
        let adv = u16_at(&hmtx, o)? as f64 * f;
        let lsb = i16_at(&hmtx, o + 2)? as f64 * f;
        set_u16(&mut hmtx, o, adv.round().clamp(0.0, 65535.0) as u16);
        set_i16(&mut hmtx, o + 2, lsb.round().clamp(-32768.0, 32767.0) as i16);
    }
    // Kopf- und Maßtabellen nachziehen
    set_u16(&mut head, 50, 1); // loca in langer Form
    let xmin = i16_at(&head, 36)? as f64 * f;
    let xmax = i16_at(&head, 40)? as f64 * f;
    set_i16(&mut head, 36, xmin.round() as i16);
    set_i16(&mut head, 40, xmax.round() as i16);
    let awmax = (u16_at(&hhea, 10)? as f64 * f).round().clamp(0.0, 65535.0) as u16;
    set_u16(&mut hhea, 10, awmax);
    for o in [12usize, 14, 16] {
        let v = i16_at(&hhea, o)? as f64 * f;
        set_i16(&mut hhea, o, v.round().clamp(-32768.0, 32767.0) as i16);
    }

    for (tag, d) in tabellen.iter_mut() {
        let t: &[u8; 4] = tag;
        match t {
            b"head" => *d = head.clone(),
            b"hhea" => *d = hhea.clone(),
            b"hmtx" => *d = hmtx.clone(),
            b"loca" => *d = neu_loca_bytes.clone(),
            b"glyf" => *d = neu_glyf.clone(),
            _ => {}
        }
    }
    Some(baue_sfnt(data.get(..4)?, tabellen))
}

/// Schreibt die Tabellen als sfnt-Datei (Verzeichnis, Prüfsummen, Ausgleich).
fn baue_sfnt(version: &[u8], mut tabellen: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    tabellen.sort_by(|a, b| a.0.cmp(&b.0));
    let n = tabellen.len() as u16;
    let mut sr = 1u16;
    while sr * 2 <= n {
        sr *= 2;
    }
    let mut out = Vec::with_capacity(1024 + tabellen.iter().map(|(_, d)| d.len()).sum::<usize>());
    out.extend_from_slice(version);
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&(sr * 16).to_be_bytes());
    out.extend_from_slice(&((sr as f32).log2() as u16).to_be_bytes());
    out.extend_from_slice(&((n - sr) * 16).to_be_bytes());
    let mut offset = 12 + 16 * n as usize;
    let mut verzeichnis = Vec::with_capacity(16 * n as usize);
    let mut daten: Vec<u8> = Vec::new();
    let mut head_offset: Option<usize> = None;
    for (tag, d) in &tabellen {
        verzeichnis.extend_from_slice(tag);
        verzeichnis.extend_from_slice(&pruefsumme(d).to_be_bytes());
        verzeichnis.extend_from_slice(&(offset as u32).to_be_bytes());
        verzeichnis.extend_from_slice(&(d.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_offset = Some(offset);
        }
        daten.extend_from_slice(d);
        while daten.len() % 4 != 0 {
            daten.push(0);
        }
        offset += d.len() + (4 - d.len() % 4) % 4;
    }
    out.extend_from_slice(&verzeichnis);
    out.extend_from_slice(&daten);
    if let Some(ho) = head_offset {
        if ho + 12 <= out.len() {
            set_u16(&mut out, ho + 8, 0);
            let summe = pruefsumme(&out);
            let ausgleich = 0xB1B0_AFBAu32.wrapping_sub(summe);
            out[ho + 8..ho + 12].copy_from_slice(&ausgleich.to_be_bytes());
        }
    }
    out
}
