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

use crate::geom::{BBox, Vec2};
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

/// PNG-Daten aus rohen RGBA-Pixeln (4 Byte je Punkt, Zeile 0 oben).
///
/// Für Bilder, die nicht als Datei vorliegen — etwa ein Screenshot aus der
/// Zwischenablage (T0112). Das Ergebnis hat dieselbe Form wie bei
/// [`lade_datei`]: Anzeigename und Base64-Daten für `EntityKind::Image`.
pub fn aus_rgba(breite: u32, hoehe: u32, rgba: &[u8]) -> Result<(String, String), String> {
    if breite == 0 || hoehe == 0 {
        return Err("Das Bild hat keine Fläche.".to_string());
    }
    let png = crate::raster::rgba_png(breite, hoehe, rgba)?;
    Ok(("Zwischenablage.png".to_string(), base64_kodiere(&png)))
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
/// Kennung über die Rohdaten — Schlüssel für Zwischenspeicher, Texturen und
/// PDF-Objekte.
///
/// Bewusst acht Byte auf einmal: die Funktion läuft bei jedem Neuaufbau der
/// Displayliste über die Base64-Daten eines Bildes (bei einem Foto mehrere
/// Megabyte). Ein Byte je Schritt wäre dort im Zoom sichtbar.
pub fn kennung(daten: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut rest = daten;
    while rest.len() >= 8 {
        let mut block = [0u8; 8];
        block.copy_from_slice(&rest[..8]);
        h ^= u64::from_le_bytes(block);
        h = h.wrapping_mul(0x0000_0100_0000_01b3).rotate_left(27);
        rest = &rest[8..];
    }
    for b in rest {
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
    // Schlüssel ist der Hash des **Textes**: die Anzeige fragt bei jedem
    // Neuaufbau der Displayliste nach demselben Bild und darf dafür nicht
    // jedes Mal die Base64-Daten zurückrechnen.
    let k = kennung(base64.as_bytes());
    if let Ok(map) = zwischenspeicher().lock() {
        if let Some(b) = map.get(&k) {
            return Some(b.clone());
        }
    }
    let roh = base64_dekodiere(base64).ok()?;
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

/// Ecken des Bildes aus der **unteren Kante** `a`→`b` und der Höhe, beginnend
/// unten links und gegen den Uhrzeigersinn. Eine Quelle für Anzeige, Griffe
/// und Trefferflächen — damit die Reihenfolge überall gleich ist.
///
/// Die Höhe steht senkrecht auf der Kante, nach links (mathematisch positiv):
/// bei waagerechter Kante `a` links und `b` rechts liegt das Bild darüber.
pub fn ecken(a: Vec2, b: Vec2, hoehe: f64) -> [Vec2; 4] {
    let d = b - a;
    let n = d.perp().normalized() * hoehe;
    [a, b, b + n, a + n]
}

/// Breite (Länge der unteren Kante) und Höhe in mm.
pub fn masse(a: Vec2, b: Vec2, hoehe: f64) -> (f64, f64) {
    (a.dist(b), hoehe)
}

/// Höhe, die aus Breite und Seitenverhältnis der Datei folgt.
pub fn hoehe_aus_breite(bild: &Bild, breite: f64) -> f64 {
    if bild.breite == 0 {
        return breite;
    }
    breite * bild.hoehe as f64 / bild.breite as f64
}

/// Wirksame Höhe eines Bildes: bei gesperrtem Seitenverhältnis aus Breite und
/// Dateimaßen, sonst der gespeicherte Wert (0 = noch nicht gesetzt → Verhältnis).
pub fn wirksame_hoehe(bild: &Bild, breite: f64, gespeichert: f64, gesperrt: bool) -> f64 {
    if gesperrt || gespeichert <= 0.0 {
        hoehe_aus_breite(bild, breite)
    } else {
        gespeichert
    }
}

/// Drehwinkel der unteren Kante (Bogenmaß) — dieselbe Richtung, in der das
/// Bild gespannt ist.
pub fn winkel(a: Vec2, b: Vec2) -> f64 {
    (b - a).angle()
}

/// Liegt `p` im Bild? Gerechnet wird in Kantenkoordinaten (längs der unteren
/// Kante und senkrecht dazu) — die Fläche ist ein gedrehtes Rechteck.
pub fn punkt_im_bild(a: Vec2, b: Vec2, hoehe: f64, p: Vec2) -> bool {
    let d = b - a;
    let len = d.len();
    if len < 1e-9 || hoehe <= 0.0 {
        return false;
    }
    let u = (p - a).dot(d) / len;
    let v = (p - a).dot(d.perp() / len);
    (-1e-6..=len + 1e-6).contains(&u) && (-1e-6..=hoehe + 1e-6).contains(&v)
}

/// Abstand von `p` zum **Rand** des Bildes (0 auf einer Kante, im Inneren
/// positiv). Nur der Rand ist anklickbar (T0201): über einem Bild lässt sich
/// zeichnen, ohne dass es dabei hervortritt.
pub fn kantenabstand(a: Vec2, b: Vec2, hoehe: f64, p: Vec2) -> f64 {
    let e = ecken(a, b, hoehe);
    (0..4)
        .map(|i| crate::geom::dist_point_segment(p, e[i], e[(i + 1) % 4]))
        .fold(f64::INFINITY, f64::min)
}

/// Berührt das Auswahlfenster `rect` eine der vier Bildkanten?
///
/// Das ist die **eine** Regel für die Fensterauswahl von Bildern (T0201): ein
/// Fenster, das ganz **innerhalb** des Bildes liegt, hat keine Kante berührt
/// und wählt es nicht mit — sonst ließe sich über einem großen Bild nichts
/// anderes auswählen. Ein Fenster, das das Bild umschließt, deckt alle vier
/// Kanten ab und wählt es. Geprüft wird das **gedrehte** Rechteck, nicht seine
/// achsenparallele Hülle.
pub fn kanten_beruehrt(a: Vec2, b: Vec2, hoehe: f64, rect: &BBox) -> bool {
    let e = ecken(a, b, hoehe);
    let fenster = [
        rect.min,
        Vec2::new(rect.max.x, rect.min.y),
        rect.max,
        Vec2::new(rect.min.x, rect.max.y),
    ];
    (0..4).any(|i| {
        let (p, q) = (e[i], e[(i + 1) % 4]);
        // Kante liegt ganz im Fenster (Fenster umschließt das Bild) …
        (rect.contains(p) && rect.contains(q))
            // … oder kreuzt eine Fensterkante.
            || (0..4).any(|k| {
                crate::geom::line_line(p, q, fenster[k], fenster[(k + 1) % 4], false, false)
                    .is_some()
            })
    })
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

    /// T0112: ein Bild aus der Zwischenablage liegt nur als Pixel vor — daraus
    /// muss derselbe einbettbare Datensatz werden wie aus einer Datei.
    #[test]
    fn aus_pixeln_wird_ein_einbettbares_png() {
        // 2×2: rot, grün / blau, weiß
        let rgba = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let (name, data) = aus_rgba(2, 2, &rgba).expect("PNG aus Pixeln");
        assert_eq!(name, "Zwischenablage.png");
        let bild = dekodiere(&data).expect("zurückgelesen");
        assert_eq!((bild.breite, bild.hoehe), (2, 2));
        assert_eq!(bild.pixel(0, 0), [255, 0, 0, 255], "links oben rot");
        assert_eq!(bild.pixel(1, 1), [255, 255, 255, 255], "rechts unten weiß");
    }

    #[test]
    fn unvollstaendige_pixel_werden_abgelehnt() {
        assert!(aus_rgba(0, 0, &[]).is_err(), "ohne Fläche kein Bild");
        assert!(
            aus_rgba(2, 2, &[0; 4]).is_err(),
            "zu wenige Bytes dürfen kein halbes Bild ergeben"
        );
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
    fn ecken_liegen_ueber_der_unteren_kante() {
        // Waagerechte Kante 600 mm, Höhe 300 mm: das Bild steht darüber.
        let e = ecken(Vec2::new(0.0, 0.0), Vec2::new(600.0, 0.0), 300.0);
        assert_eq!(e[0], Vec2::new(0.0, 0.0));
        assert_eq!(e[1], Vec2::new(600.0, 0.0));
        assert_eq!(e[2], Vec2::new(600.0, 300.0));
        assert_eq!(e[3], Vec2::new(0.0, 300.0));
        assert_eq!(masse(Vec2::new(0.0, 0.0), Vec2::new(600.0, 0.0), 300.0), (600.0, 300.0));
    }

    #[test]
    fn gedrehte_kante_dreht_das_bild_mit() {
        // Kante um 90°: a unten, b oben — das Bild steht links davon.
        let e = ecken(Vec2::new(0.0, 0.0), Vec2::new(0.0, 600.0), 300.0);
        assert_eq!(e[0], Vec2::new(0.0, 0.0));
        assert_eq!(e[1], Vec2::new(0.0, 600.0));
        assert_eq!(e[2], Vec2::new(-300.0, 600.0));
        assert_eq!(e[3], Vec2::new(-300.0, 0.0));
        // Die Fläche ist ein gedrehtes Rechteck, keine achsenparallele Box.
        assert!(punkt_im_bild(
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 600.0),
            300.0,
            Vec2::new(-150.0, 300.0)
        ));
        assert!(!punkt_im_bild(
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 600.0),
            300.0,
            Vec2::new(150.0, 300.0)
        ));
    }

    #[test]
    fn hoehe_folgt_dem_seitenverhaeltnis() {
        let bild = zerlege(PNG_ROT).expect("PNG");
        // 2 × 2 Pixel: quadratisch, also Höhe = Breite.
        assert!((hoehe_aus_breite(&bild, 400.0) - 400.0).abs() < 1e-9);
    }
}
