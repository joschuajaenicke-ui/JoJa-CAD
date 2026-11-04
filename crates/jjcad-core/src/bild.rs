//! Bilder (PNG und JPEG) — einlesen, prüfen und in Pixel zerlegen.
//!
//! Ein Bild ist im Modell **eingebettet**: die Datei liegt als Base64 im
//! `.jjcad`-Dokument (`EntityKind::Image`). Damit bleibt die Zeichnung gültig,
//! wenn die Bilddatei verschoben oder gelöscht wird — anders als bei einem
//! bloßen Pfad, der nach dem Umbenennen ins Leere zeigt.
//!
//! Das Zerlegen in Pixel ist teuer (ein Foto mit 12 Megapixeln sind 48 MB).
//! Deshalb läuft es über einen kleinen Zwischenspeicher: die Anzeige fragt bei
//! jedem Neuaufbau der Displayliste nach demselben Bild und bekommt dieselbe
//! bereits zerlegte Fassung zurück.

use crate::geom::Vec2;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Zerlegtes Bild: Breite, Höhe und RGBA-Pixel (4 Byte je Pixel, Zeile 0 oben).
///
/// `kennung` ist der Hash der Base64-Daten. Er dient als Schlüssel für
/// Texturen und PDF-Objekte — nicht zum Vergleichen des Inhalts.
#[derive(Clone, Debug)]
pub struct Bild {
    pub kennung: u64,
    pub breite: u32,
    pub hoehe: u32,
    pub rgba: Vec<u8>,
}

impl PartialEq for Bild {
    /// Gleich heißt: gleiche Quelle (Kennung) und gleiche Maße. Die Pixel
    /// selbst zu vergleichen kostet bei jedem Aufbau der Displayliste
    /// Megabyte — und zwei verschiedene Bilder mit derselben Kennung sind bei
    /// 64 Bit Hash praktisch ausgeschlossen.
    fn eq(&self, other: &Self) -> bool {
        self.kennung == other.kennung && self.breite == other.breite && self.hoehe == other.hoehe
    }
}

impl Bild {
    /// Pixel als `(r, g, b, a)` an der Stelle `(x, y)` — Zeile 0 ist oben.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y.min(self.hoehe.saturating_sub(1)) * self.breite
            + x.min(self.breite.saturating_sub(1))) * 4) as usize;
        [
            *self.rgba.get(i).unwrap_or(&0),
            *self.rgba.get(i + 1).unwrap_or(&0),
            *self.rgba.get(i + 2).unwrap_or(&0),
            *self.rgba.get(i + 3).unwrap_or(&255),
        ]
    }
}

// ---------------------------------------------------------------------------
// Base64 (RFC 4648, mit Auffüllung) — kein zusätzliches Paket nötig
// ---------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_kodiere(daten: &[u8]) -> String {
    let mut out = String::with_capacity(daten.len().div_ceil(3) * 4);
    for stueck in daten.chunks(3) {
        let b0 = stueck[0] as u32;
        let b1 = *stueck.get(1).unwrap_or(&0) as u32;
        let b2 = *stueck.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        if stueck.len() > 1 {
            out.push(B64[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if stueck.len() > 2 {
            out.push(B64[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn base64_dekodiere(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut puffer: u32 = 0;
    let mut bits = 0u32;
    for c in text.bytes() {
        let wert = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return Err(format!("ungültiges Zeichen in den Bilddaten: {}", c as char)),
        };
        puffer = (puffer << 6) | wert as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((puffer >> bits) as u8);
            puffer &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Datei lesen und Format prüfen
// ---------------------------------------------------------------------------

/// Bildformat an den ersten Bytes erkennen. `None` = kein PNG und kein JPEG.
pub fn format(daten: &[u8]) -> Option<&'static str> {
    if daten.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if daten.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else {
        None
    }
}

/// Liest eine Bilddatei und liefert `(Dateiname, Base64-Daten)`.
///
/// Nur PNG und JPEG werden angenommen — ein anderes Format wird mit einem
/// verständlichen Fehler abgelehnt, statt später als leeres Rechteck zu
/// erscheinen.
pub fn lade_datei(pfad: &Path) -> Result<(String, String), String> {
    let daten = std::fs::read(pfad)
        .map_err(|e| format!("Bild nicht lesbar ({}): {e}", pfad.display()))?;
    if daten.is_empty() {
        return Err("Die Bilddatei ist leer.".to_string());
    }
    if format(&daten).is_none() {
        return Err("Nur PNG und JPEG werden eingefügt.".to_string());
    }
    // Zerlegen prüft zugleich, ob die Datei wirklich ein lesbares Bild ist —
    // eine abgeschnittene Datei fällt so beim Einfügen auf, nicht im Bild.
    zerlege(&daten).map_err(|e| format!("Bild nicht lesbar: {e}"))?;
    let name = pfad
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Bild".to_string());
    Ok((name, base64_kodiere(&daten)))
}

/// Zerlegt PNG- oder JPEG-Daten in RGBA-Pixel.
pub fn zerlege(daten: &[u8]) -> Result<Bild, String> {
    let img = image::load_from_memory(daten).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (breite, hoehe) = (rgba.width(), rgba.height());
    if breite == 0 || hoehe == 0 {
        return Err("Das Bild hat keine Fläche.".to_string());
    }
    Ok(Bild {
        kennung: kennung(daten),
        breite,
        hoehe,
        rgba: rgba.into_raw(),
    })
}

/// Hash über die Rohdaten — Schlüssel für Zwischenspeicher, Texturen und
/// PDF-Objekte.
pub fn kennung(daten: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in daten {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn zwischenspeicher() -> &'static Mutex<HashMap<u64, Arc<Bild>>> {
    static SPERRE: OnceLock<Mutex<HashMap<u64, Arc<Bild>>>> = OnceLock::new();
    SPERRE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Zerlegtes Bild zu eingebetteten Base64-Daten — mit Zwischenspeicher.
///
/// Die Anzeige baut die Displayliste bei jedem Zoom neu auf; ohne diesen
/// Speicher würde jedes Foto dabei erneut zerlegt.
pub fn dekodiere(base64: &str) -> Option<Arc<Bild>> {
    let roh = base64_dekodiere(base64).ok()?;
    let k = kennung(&roh);
    if let Ok(map) = zwischenspeicher().lock() {
        if let Some(b) = map.get(&k) {
            return Some(b.clone());
        }
    }
    let bild = Arc::new(zerlege(&roh).ok()?);
    if let Ok(mut map) = zwischenspeicher().lock() {
        // Kleiner Deckel: die letzten acht Bilder reichen für eine Zeichnung.
        if map.len() >= 8 && !map.contains_key(&k) {
            map.clear();
        }
        map.insert(k, bild.clone());
    }
    Some(bild)
}

/// Ecken des Rechtecks aus zwei gegenüberliegenden Punkten, beginnend unten
/// links und gegen den Uhrzeigersinn. Eine Quelle für Anzeige, Griffe und
/// Trefferflächen — damit die Reihenfolge überall gleich ist.
pub fn ecken(a: Vec2, b: Vec2) -> [Vec2; 4] {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    [
        Vec2::new(x0, y0),
        Vec2::new(x1, y0),
        Vec2::new(x1, y1),
        Vec2::new(x0, y1),
    ]
}

/// Breite und Höhe des Bildrechtecks in mm.
pub fn masse(a: Vec2, b: Vec2) -> (f64, f64) {
    ((b.x - a.x).abs(), (b.y - a.y).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kleinstes gültiges PNG (2×2: rot, grün / blau, weiß) — fest eingetragen,
    /// damit der Test ohne Bilddatei läuft.
    const PNG_ROT: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
        0x44, 0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00,
        0x00, 0xFD, 0xD4, 0x9A, 0x73, 0x00, 0x00, 0x00, 0x12, 0x49, 0x44, 0x41, 0x54, 0x78,
        0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xC0, 0x00, 0xC2, 0x0C, 0xFF, 0x81, 0x00, 0x00, 0x1F,
        0xEE, 0x05, 0xFB, 0x0B, 0xD9, 0x68, 0x8B, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
        0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn base64_laeuft_rund() {
        for n in 0..40usize {
            let daten: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            let text = base64_kodiere(&daten);
            assert_eq!(base64_dekodiere(&text).unwrap(), daten, "n = {n}");
        }
    }

    #[test]
    fn base64_kennt_die_auffuellung() {
        assert_eq!(base64_kodiere(b"a"), "YQ==");
        assert_eq!(base64_kodiere(b"ab"), "YWI=");
        assert_eq!(base64_kodiere(b"abc"), "YWJj");
    }

    #[test]
    fn png_wird_erkannt_und_zerlegt() {
        assert_eq!(format(PNG_ROT), Some("png"));
        let bild = zerlege(PNG_ROT).expect("PNG zerlegen");
        assert_eq!((bild.breite, bild.hoehe), (2, 2));
        let p = bild.pixel(0, 0);
        assert!(p[0] > 200 && p[1] < 80 && p[2] < 80, "rot erwartet, war {p:?}");
        let g = bild.pixel(1, 0);
        assert!(g[1] > 200 && g[0] < 80, "grün erwartet, war {g:?}");
    }

    #[test]
    fn fremdes_format_wird_abgelehnt() {
        assert_eq!(format(b"GIF89a"), None);
        assert!(zerlege(b"kein bild").is_err());
    }

    #[test]
    fn eingebettete_daten_kommen_zerlegt_zurueck() {
        let text = base64_kodiere(PNG_ROT);
        let bild = dekodiere(&text).expect("Bild aus Base64");
        assert_eq!((bild.breite, bild.hoehe), (2, 2));
        // Zweiter Aufruf kommt aus dem Zwischenspeicher und ist dasselbe Bild.
        let nochmal = dekodiere(&text).expect("Bild aus dem Speicher");
        assert!(Arc::ptr_eq(&bild, &nochmal));
    }

    #[test]
    fn ecken_und_masse_sind_achsenparallel() {
        let e = ecken(Vec2::new(500.0, 300.0), Vec2::new(-100.0, 900.0));
        assert_eq!(e[0], Vec2::new(-100.0, 300.0));
        assert_eq!(e[2], Vec2::new(500.0, 900.0));
        assert_eq!(masse(Vec2::new(500.0, 300.0), Vec2::new(-100.0, 900.0)), (600.0, 600.0));
    }
}
