//! Dämmung als Schlaufenband — die Rechnung zur Linienart
//! [`crate::style::LineType::Insulation`].
//!
//! Die Kurve ist die dokumentierte parametrische Schlaufenkurve
//! (`docs/DAEMMUNG-SCHLAUFEN.md`):
//!
//! ```text
//! x(t) = L/(2πn) · (t + K · sin 2t),   y(t) = H/2 · cos t,   t ∈ [0, 2πn]
//! n    = max(1/2, round(2L/(r·H)) / 2),   K = 2,3016694
//! ```
//!
//! `H` ist die Dämmstärke (Bandhöhe), `r` die Schlaufenform
//! (Periodenlänge / Höhe). Bei `t = 2πn` ist `sin 2t = 0`, also `x = L`
//! **exakt** — die Kurve endet auf der Länge der Linie, unabhängig von `H`
//! und `r`. Die Konstante `K` ist auf volle Stellen nötig: mit einem gröberen
//! Wert ragt die Kurve über 0 bzw. `L` hinaus.
//!
//! Auf eine **beliebige Linie** (auch Polylinie, Bogen, Kreis) übertragen wird
//! die Kurve über die Bogenlänge: `x` ist der Weg längs der Linie, `y` der
//! Versatz quer dazu. Damit sitzt das Band mittig auf der Linie und die Länge
//! stimmt auf jedem Weg.

use crate::geom::Vec2;

/// Konstante der Schlaufenkurve: `K = |s| / 2` mit `s = -4,6033388`.
pub const K: f64 = 2.3016694;

/// Vorgabe der Dämmstärke (H) in mm.
pub const BREITE_MM: f64 = 100.0;
/// Vorgabe der Schlaufenform (r = Periodenlänge / Höhe).
pub const FORM: f64 = 0.5;

/// Grenzen der Dämmstärke: 5 mm … 2 m. Darunter ist keine Schlaufe zu sehen,
/// darüber ist es keine Dämmung mehr.
pub const BREITE_MIN_MM: f64 = 5.0;
pub const BREITE_MAX_MM: f64 = 2000.0;

/// Grenzen der Schlaufenform. Die Dokumentation nennt 0,2…0,8 als sinnvoll
/// (optimal 0,5); der Bereich lässt etwas Luft für Sonderfälle.
pub const FORM_MIN: f64 = 0.1;
pub const FORM_MAX: f64 = 2.0;

/// Anzahl Perioden in Halbschritten: `n = max(1/2, round(2L/(r·H)) / 2)`.
///
/// `n` springt in Halbschritten, deshalb weicht `p/H` je nach Länge etwas von
/// `r` ab — bei wenigen Perioden um einige Prozent, bei vielen kaum noch.
pub fn perioden(len: f64, h: f64, r: f64) -> f64 {
    let h = if h.is_finite() && h > 0.0 { h } else { BREITE_MM };
    let r = if r.is_finite() && r > 0.0 { r } else { FORM };
    if !len.is_finite() || len <= 0.0 {
        return 0.5;
    }
    ((2.0 * len / (r * h)).round() / 2.0).max(0.5)
}

/// Schlaufenband längs eines Wegs.
///
/// `pfad` ist der Weg (mindestens zwei Punkte), `geschlossen` schließt ihn zum
/// Ring. `h` ist die Bandhöhe in mm (H), `r` die Schlaufenform. Zurück kommt
/// der Polygonzug der Kurve — **nicht** geschlossen, weil Anfang und Ende des
/// Bandes verschieden liegen (Start bei +H/2, Ende je nach `n` oben oder
/// unten).
pub fn band(pfad: &[Vec2], geschlossen: bool, h: f64, r: f64) -> Vec<Vec2> {
    let mut pts: Vec<Vec2> = Vec::with_capacity(pfad.len() + 1);
    for p in pfad {
        if !p.is_finite() {
            continue;
        }
        if pts.last().map(|q| q.dist(*p) > 1e-9).unwrap_or(true) {
            pts.push(*p);
        }
    }
    if pts.len() < 2 {
        return pts;
    }
    if geschlossen && pts[0].dist(*pts.last().unwrap()) > 1e-9 {
        pts.push(pts[0]);
    }

    // Bogenlänge an jedem Stützpunkt — die eine Zuordnung „x auf dem Weg“.
    let mut s: Vec<f64> = Vec::with_capacity(pts.len());
    let mut summe = 0.0;
    s.push(0.0);
    for w in pts.windows(2) {
        summe += w[0].dist(w[1]);
        s.push(summe);
    }
    let len = summe;

    let h = if h.is_finite() && h > 0.0 { h } else { BREITE_MM };
    let r = if r.is_finite() && r > 0.0 { r } else { FORM };
    if !(len > 1e-9) {
        return pts;
    }

    let n = perioden(len, h, r);
    let t_max = std::f64::consts::TAU * n;
    let a = len / t_max;

    // Abtastung: ein Punkt je rund 1/25 Periode (feiner bei schmalen
    // Schlaufen), mindestens 12 je Periode, höchstens 6000 Punkte gesamt —
    // das Band ist gezeichnete Geometrie und läuft durch Anzeige, PDF und DXF.
    let p = len / n;
    let je = (p / (h * 0.02).max(0.25)).round().clamp(12.0, 40.0);
    let total = ((n * je).ceil() as usize).clamp(24, 6000);

    let mut out = Vec::with_capacity(total + 1);
    for i in 0..=total {
        let t = t_max * (i as f64 / total as f64);
        let x = a * (t + K * (2.0 * t).sin());
        let y = 0.5 * h * t.cos();
        // x bleibt bei exaktem K im Bereich 0…L; das Klemmen fängt nur
        // Rundungsreste ab und verhindert, dass ein Punkt neben dem Weg liegt.
        let (punkt, richtung) = punkt_bei(&pts, &s, x.clamp(0.0, len));
        out.push(punkt + richtung.perp() * y);
    }
    out
}

/// Punkt und Richtung bei der Bogenlänge `x` (lineare Interpolation).
fn punkt_bei(pts: &[Vec2], s: &[f64], x: f64) -> (Vec2, Vec2) {
    let i = match s.binary_search_by(|v| v.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal)) {
        Ok(i) => i.min(pts.len() - 2),
        Err(i) => i.saturating_sub(1).min(pts.len() - 2),
    };
    let seg = pts[i + 1] - pts[i];
    let l = s[i + 1] - s[i];
    let t = if l > 1e-12 {
        ((x - s[i]) / l).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (pts[i] + seg * t, seg.normalized())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gerade(len: f64) -> Vec<Vec2> {
        vec![Vec2::new(0.0, 0.0), Vec2::new(len, 0.0)]
    }

    /// Der Kern der Aufgabe: die Länge passt exakt — Anfang bei x = 0, Ende
    /// bei x = L, unabhängig von H und r.
    #[test]
    fn laenge_ist_exakt() {
        for (h, r) in [(100.0, 0.5), (60.0, 0.3), (240.0, 0.8), (100.0, 0.21)] {
            for len in [1000.0, 2500.0, 7777.0, 123.4] {
                let b = band(&gerade(len), false, h, r);
                let first = b.first().unwrap();
                let last = b.last().unwrap();
                assert!(
                    first.x.abs() < 1e-9,
                    "Anfang liegt bei x = {}, nicht 0 (H {h}, r {r}, L {len})",
                    first.x
                );
                assert!(
                    (last.x - len).abs() < 1e-9,
                    "Ende liegt bei x = {}, nicht {len} (H {h}, r {r})",
                    last.x
                );
                // Und nichts ragt über die Grenzen hinaus.
                for p in &b {
                    assert!(
                        p.x >= -1e-9 && p.x <= len + 1e-9,
                        "Punkt bei x = {} liegt außerhalb 0…{len}",
                        p.x
                    );
                }
            }
        }
    }

    /// Die Bandhöhe ist H: die Kurve schwingt zwischen ±H/2.
    #[test]
    fn breite_ist_die_bandhoehe() {
        let b = band(&gerade(2000.0), false, 120.0, 0.5);
        let max = b.iter().fold(0.0f64, |m, p| m.max(p.y));
        let min = b.iter().fold(0.0f64, |m, p| m.min(p.y));
        assert!((max - 60.0).abs() < 0.5, "Höchster Punkt bei {max}, erwartet 60");
        assert!((min + 60.0).abs() < 0.5, "Tiefster Punkt bei {min}, erwartet -60");
    }

    /// Die Schlaufenform bestimmt die Periodenzahl.
    #[test]
    fn form_bestimmt_die_periodenzahl() {
        // n = round(2L/(r·H))/2 — L = 2000, H = 100: 2·2000/(r·100) = 40/r
        assert_eq!(perioden(2000.0, 100.0, 0.5), 40.0);
        assert_eq!(perioden(2000.0, 100.0, 0.25), 80.0);
        assert_eq!(perioden(2000.0, 100.0, 1.0), 20.0);
        // Sehr kurze Linien: mindestens eine halbe Periode.
        assert_eq!(perioden(10.0, 100.0, 0.5), 0.5);
    }

    /// Auf einer Polylinie stimmt die Länge ebenso — die Kurve läuft um die
    /// Ecke, ohne zu springen.
    #[test]
    fn polylinie_haelt_die_laenge() {
        let pfad = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1000.0, 0.0),
            Vec2::new(1000.0, 800.0),
        ];
        let b = band(&pfad, false, 80.0, 0.5);
        let first = *b.first().unwrap();
        let last = *b.last().unwrap();
        assert!(first.dist(Vec2::new(0.0, 40.0)) < 1e-9, "Anfang {first:?}");
        // Das Ende liegt **auf der Höhe** des Pfadendes (Länge stimmt exakt),
        // quer dazu um H/2 versetzt — n = 45 ist ganzzahlig, also +H/2.
        assert!(
            (last.y - 800.0).abs() < 1e-9 && (last.x - (1000.0 - 40.0)).abs() < 1e-9,
            "Ende {last:?} liegt nicht am Pfadende"
        );
        // Kein Sprung: benachbarte Punkte liegen dicht beieinander.
        for w in b.windows(2) {
            assert!(w[0].dist(w[1]) < 60.0, "Sprung von {:?} nach {:?}", w[0], w[1]);
        }
    }

    /// Der Ring schließt: auf einem geschlossenen Weg endet das Band am
    /// Anfang (plus Versatz).
    #[test]
    fn ring_schliesst_sich() {
        let pfad = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1000.0, 0.0),
            Vec2::new(1000.0, 1000.0),
            Vec2::new(0.0, 1000.0),
        ];
        let b = band(&pfad, true, 80.0, 0.5);
        let len = 4000.0;
        // n = round(2·4000/(0.5·80))/2 = 100
        assert_eq!(perioden(len, 80.0, 0.5), 100.0);
        // Am Anfang steht der Versatz +H/2 quer zur ersten Kante (0,1),
        // am Ende +H/2 quer zur letzten Kante (1,0) — der Ring ist damit
        // geschlossen, die Länge aufgegangen.
        let first = *b.first().unwrap();
        let last = *b.last().unwrap();
        assert!(
            first.dist(Vec2::new(0.0, 40.0)) < 1e-9,
            "Ring beginnt nicht bei (0, 40): {first:?}"
        );
        assert!(
            last.dist(Vec2::new(40.0, 0.0)) < 1e-9,
            "Ring endet nicht bei (40, 0): {last:?}"
        );
    }

    /// Degenerierte Eingaben bleiben stehen statt zu panicken.
    #[test]
    fn entartete_eingaben() {
        assert!(band(&[], false, 100.0, 0.5).is_empty());
        assert_eq!(band(&[Vec2::new(1.0, 2.0)], false, 100.0, 0.5).len(), 1);
        let doppelt = vec![Vec2::new(5.0, 5.0), Vec2::new(5.0, 5.0)];
        assert_eq!(band(&doppelt, false, 100.0, 0.5).len(), 1);
    }
}
