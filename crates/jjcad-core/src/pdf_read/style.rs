//! PDF-Grafikzustand → JoJa-Werte: Farbe, Strichstärke, Strichart.

use crate::style::{Color, LineType};

/// Farbe aus 0…1-Werten (Graustufen, RGB).
pub fn rgb(r: f64, g: f64, b: f64) -> Color {
    let k = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgb(k(r), k(g), k(b))
}

pub fn gray(v: f64) -> Color {
    rgb(v, v, v)
}

/// CMYK nach der üblichen Näherung — genau genug für Strichfarben.
pub fn cmyk(c: f64, m: f64, y: f64, k: f64) -> Color {
    let f = |v: f64| (1.0 - v.clamp(0.0, 1.0)) * (1.0 - k.clamp(0.0, 1.0)) * 255.0;
    Color::rgb(f(c).round() as u8, f(m).round() as u8, f(y).round() as u8)
}

/// Strichmuster aus `d` → vorhandene Strichart.
///
/// Verglichen wird der **ganze** Rhythmus (Summe und Verhältnis), nicht die
/// einzelnen Zahlen: PDF schreibt Muster gern in Punkt, unsere Stricharten in
/// Millimetern.
pub fn line_type_from_dash(pattern: &[f64]) -> Option<LineType> {
    let vals: Vec<f64> = pattern.iter().copied().filter(|v| *v > 1e-6).collect();
    if vals.is_empty() {
        return Some(LineType::Solid);
    }
    let summe: f64 = vals.iter().sum();
    if summe < 1e-6 {
        return Some(LineType::Solid);
    }
    let normiert: Vec<f64> = vals.iter().map(|v| v / summe).collect();
    let mut best: Option<(f64, LineType)> = None;
    for lt in LineType::ALL {
        let p = lt.pattern();
        if p.is_empty() {
            continue;
        }
        let ps: f64 = p.iter().sum();
        if (ps - 1.0).abs() < 0.0 {
            // Platzhalter, damit kein Muster mit sich selbst verglichen wird
        }
        let pn: Vec<f64> = p.iter().map(|v| v / ps).collect();
        // Gleiche Anzahl Abschnitte? Sonst höchstens grob ähnlich.
        let score = if pn.len() == normiert.len() {
            pn.iter()
                .zip(normiert.iter())
                .map(|(a, b)| (a - b).abs())
                .sum()
        } else {
            // Nur die Grundform unterscheiden: Strich/Punkt-Verhältnis
            let kurz = normiert.iter().filter(|v| **v < 0.1).count();
            let punkte = pn.iter().filter(|v| **v < 0.1).count();
            if kurz == punkte {
                0.35
            } else {
                2.0
            }
        };
        if best.as_ref().map(|(b, _)| score < *b).unwrap_or(true) {
            best = Some((score, lt));
        }
    }
    match best {
        // Alles über 0,6 gilt als „kein Treffer".
        Some((score, lt)) if score <= 0.6 => Some(lt),
        _ => None,
    }
}
