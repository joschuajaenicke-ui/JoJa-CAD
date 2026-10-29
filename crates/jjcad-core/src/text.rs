//! Textsatz mit Läufen: die eine Stelle, an der ein Text mit Fett/Kursiv/
//! Unterstrichen/Hochstellung in Zeilen und Stücke zerlegt wird.
//!
//! Anzeige, Rasterausgabe und PDF-Schreiber setzen danach dieselben Maße um,
//! deshalb sitzt ein Text überall an derselben Stelle.
//!
//! Maßeinheit ist **Em** (1,0 = Schrifthöhe). Erst der Aufrufer multipliziert
//! mit der tatsächlichen Größe — so bleibt der Satz von der Ausgabegröße
//! unabhängig.

use crate::fonts::{self, FontStyle};
use crate::model::TextRun;

/// Aufschlag einer Hochstellung in Em über der Grundlinie.
pub const SCRIPT_RISE_EM: f64 = 0.30;
/// Verkleinerung hochgestellter Zeichen.
pub const SCRIPT_SCALE: f64 = 0.70;

/// Ein Stück einer Zeile mit einheitlicher Formatierung.
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub text: String,
    pub style: FontStyle,
    pub underline: bool,
    /// Versatz der Grundlinie in Em (Hochstellung positiv).
    pub rise_em: f64,
    /// Größenfaktor (Hochstellung 0,7).
    pub scale: f64,
}

impl Seg {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Eine gesetzte Zeile: Stücke mit ihren Vorschüben in Em.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub segs: Vec<Seg>,
    /// Gesamtbreite in Em.
    pub width_em: f64,
}

impl Line {
    /// Breite eines Stücks in Em (ohne Berücksichtigung des Größenfaktors).
    pub fn seg_width_em(seg: &Seg) -> f64 {
        fonts::advance_em_str(seg.style, &seg.text) * seg.scale
    }
}

/// Zerlegt einen Text samt Läufen in Zeilen.
///
/// `runs` darf leer sein — dann gilt der ganze `text` als ein Lauf; `bold`
/// kommt in dem Fall vom Objekt (Altbestand ohne Läufe).
pub fn layout(text: &str, runs: &[TextRun]) -> Vec<Line> {
    let mut lines: Vec<Line> = vec![Line::default()];
    let mut any = false;
    let mut push_run = |run: &TextRun, lines: &mut Vec<Line>| {
        let style = FontStyle::from_flags(run.bold, run.italic);
        let (rise, scale) = if run.superscript {
            (SCRIPT_RISE_EM, SCRIPT_SCALE)
        } else {
            (0.0, 1.0)
        };
        for (i, teil) in run.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Line::default());
            }
            if teil.is_empty() {
                continue;
            }
            let seg = Seg {
                text: teil.to_string(),
                style,
                underline: run.underline,
                rise_em: rise,
                scale,
            };
            let w = Line::seg_width_em(&seg);
            if let Some(line) = lines.last_mut() {
                line.width_em += w;
                // Gleichartige Nachbarn zusammenfassen (weniger Stücke im PDF).
                if let Some(last) = line.segs.last_mut() {
                    if last.style == seg.style
                        && last.underline == seg.underline
                        && (last.rise_em - seg.rise_em).abs() < 1e-9
                    {
                        last.text.push_str(&seg.text);
                        any = true;
                        continue;
                    }
                }
                line.segs.push(seg);
            }
        }
        any = true;
    };

    if runs.is_empty() {
        let r = TextRun {
            text: text.to_string(),
            ..Default::default()
        };
        push_run(&r, &mut lines);
    } else {
        for r in runs {
            push_run(r, &mut lines);
        }
    }
    let _ = any;
    lines
}

/// Gesamtbreite eines Textes in Em (längste Zeile).
pub fn width_em(text: &str, runs: &[TextRun]) -> f64 {
    layout(text, runs)
        .iter()
        .map(|l| l.width_em)
        .fold(0.0, f64::max)
}

/// Näherungsbreite ohne geladene Schrift — Helvetica-Maße (1/1000 Em).
///
/// Wird nur benutzt, wenn keine TrueType-Schrift gefunden wurde; dann schreibt
/// der PDF-Export auch mit Helvetica, sodass beide Wege zusammenpassen.
pub fn fallback_advance_em(c: char) -> f64 {
    const HELV_W: [u16; 95] = [
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556,
        556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722,
        722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722,
        667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556,
        556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500,
        500, 334, 260, 334, 584,
    ];
    let code = c as u32;
    if (32..127).contains(&code) {
        HELV_W[(code - 32) as usize] as f64 / 1000.0
    } else {
        match code {
            0xF6 | 0xFC | 0xE4 | 0xDF => 0.556,
            0x20AC => 0.556,
            _ => 0.6,
        }
    }
}

// ---------------------------------------------------------------------------
// Zeichenweise Auszeichnung (für die Textbearbeitung im Panel)
// ---------------------------------------------------------------------------

/// Auszeichnung eines Zeichens — ohne den Text selbst.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub superscript: bool,
}

impl RunStyle {
    pub fn of(r: &TextRun) -> Self {
        Self {
            bold: r.bold,
            italic: r.italic,
            underline: r.underline,
            superscript: r.superscript,
        }
    }

    pub fn plain(self) -> bool {
        !self.bold && !self.italic && !self.underline && !self.superscript
    }
}

/// Auszeichnung je Zeichen des Textes (Länge = Zeichenzahl).
///
/// Läufe, die den Text nicht genau abdecken, werden aufgefüllt: fehlende Zeichen
/// erben die Auszeichnung des vorigen Zeichens, überzählige Läufe fallen weg.
pub fn char_styles(text: &str, runs: &[TextRun], fallback: RunStyle) -> Vec<RunStyle> {
    let n = text.chars().count();
    let mut out = Vec::with_capacity(n);
    if runs.is_empty() {
        return vec![fallback; n];
    }
    let mut letzte = fallback;
    for r in runs {
        let st = RunStyle::of(r);
        for _ in r.text.chars() {
            if out.len() >= n {
                break;
            }
            out.push(st);
            letzte = st;
        }
        if out.len() >= n {
            break;
        }
    }
    while out.len() < n {
        out.push(letzte);
    }
    out
}

/// Läufe aus zeichenweiser Auszeichnung — gleiche Nachbarn werden zusammengefasst.
pub fn runs_from_styles(text: &str, styles: &[RunStyle]) -> Vec<TextRun> {
    let mut out: Vec<TextRun> = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let st = styles.get(i).copied().unwrap_or_default();
        match out.last_mut() {
            Some(r) if RunStyle::of(r) == st => r.text.push(c),
            _ => out.push(TextRun {
                text: c.to_string(),
                bold: st.bold,
                italic: st.italic,
                underline: st.underline,
                superscript: st.superscript,
            }),
        }
    }
    out
}

/// Läufe auf einen geänderten Text übertragen.
///
/// Die Formatierung bleibt an den Zeichen hängen: gemeinsamer Anfang und
/// gemeinsames Ende werden übernommen, eingefügte Zeichen erben die
/// Auszeichnung des Zeichens davor (sonst `fallback`).
pub fn retarget_runs(
    text: &str,
    runs: &[TextRun],
    neu: &str,
    fallback: RunStyle,
) -> Vec<TextRun> {
    if text == neu {
        return runs.to_vec();
    }
    let alt: Vec<char> = text.chars().collect();
    let neu_c: Vec<char> = neu.chars().collect();
    let alt_st = char_styles(text, runs, fallback);

    let max = alt.len().min(neu_c.len());
    let mut prefix = 0;
    while prefix < max && alt[prefix] == neu_c[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < max - prefix
        && alt[alt.len() - 1 - suffix] == neu_c[neu_c.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let mut styles: Vec<RunStyle> = Vec::with_capacity(neu_c.len());
    styles.extend_from_slice(&alt_st[..prefix]);
    let erb = if prefix > 0 {
        alt_st[prefix - 1]
    } else {
        alt_st.first().copied().unwrap_or(fallback)
    };
    for _ in 0..(neu_c.len().saturating_sub(prefix + suffix)) {
        styles.push(erb);
    }
    if suffix > 0 {
        styles.extend_from_slice(&alt_st[alt.len() - suffix..]);
    }
    styles.truncate(neu_c.len());
    runs_from_styles(neu, &styles)
}

/// Auszeichnung auf den Zeichenbereich `from..to` anwenden (Zeichenindizes).
pub fn apply_style(
    text: &str,
    runs: &[TextRun],
    from: usize,
    to: usize,
    st: RunStyle,
) -> Vec<TextRun> {
    let mut styles = char_styles(text, runs, RunStyle::default());
    if styles.is_empty() {
        return Vec::new();
    }
    let a = from.min(styles.len());
    let b = to.max(a).min(styles.len());
    if a == b {
        // Leerer Bereich: nichts zu tun (der Aufrufer entscheidet, ob er den
        // ganzen Text meint).
        return runs_from_styles(text, &styles);
    }
    for s in styles.iter_mut().take(b).skip(a) {
        *s = st;
    }
    runs_from_styles(text, &styles)
}

/// Läufe so zurechtschneiden, dass sie den Text genau abdecken.
pub fn normalize_runs(text: &str, runs: &[TextRun], fallback: RunStyle) -> Vec<TextRun> {
    let st = char_styles(text, runs, fallback);
    runs_from_styles(text, &st)
}
