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

/// Ab dieser Abbildung (Bildschirm-px je Periode) wird nach der Pfeilhöhe
/// abgetastet; darunter gelten feste Stufen (T0176).
pub const DYNAMISCH_AB_PX: f64 = 12.0;
/// Stufe mit 10 Punkten je Periode: 5…12 px.
pub const ZEHN_AB_PX: f64 = 5.0;
/// Stufe mit 6 Punkten je Periode: 2,5…5 px.
pub const SECHS_AB_PX: f64 = 2.5;
/// Stufe mit 2 Punkten je Periode: 1,5…2,5 px.
pub const ZWEI_AB_PX: f64 = 1.5;
/// Faktor der Pfeilhöhen-Regel `N = gerade(ceil(sqrt(52 · ppx)))`: er hält die
/// Sehnenabweichung bei rund 0,15 px (gemessen 0,14…0,15 px).
pub const PFEILHOEHE_FAKTOR: f64 = 52.0;
/// Obergrenze der Punktzahl je Periode — auch bei stärkster Vergrößerung.
pub const MAX_PUNKTE_JE_PERIODE: usize = 512;

/// Darstellung eines Schlaufenbands in der Anzeige, gemessen in
/// Bildschirm-px je Periode (Periodenlänge in mm × Abbildung px/mm).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stufe {
    /// `N` Punkte je Periode — immer **gerade**.
    Punkte(usize),
    /// Unter 1,5 px je Periode: Gerade in Bandmitte, Bandhöhe als Strichbreite.
    Gerade,
}

/// Stufe zu einer Abbildung von `px_je_periode`.
///
/// Die Punktzahl ist immer gerade: der untere Scheitel der Kurve liegt bei
/// `t = π`. Bei ungerader Zahl trifft ihn kein Knoten, und das Band wird unten
/// zu flach (N = 5: 80,9 %, N = 7: 90,1 %, N = 9: 94,0 % der Bandhöhe) und die
/// Periode asymmetrisch.
pub fn stufe(px_je_periode: f64) -> Stufe {
    let ppx = if px_je_periode.is_finite() {
        px_je_periode.max(0.0)
    } else {
        DYNAMISCH_AB_PX
    };
    if ppx >= DYNAMISCH_AB_PX {
        let roh = (PFEILHOEHE_FAKTOR * ppx).sqrt().ceil() as usize;
        return Stufe::Punkte(gerade_zahl(roh).min(MAX_PUNKTE_JE_PERIODE));
    }
    if ppx >= ZEHN_AB_PX {
        return Stufe::Punkte(10);
    }
    if ppx >= SECHS_AB_PX {
        return Stufe::Punkte(6);
    }
    if ppx >= ZWEI_AB_PX {
        return Stufe::Punkte(2);
    }
    Stufe::Gerade
}

/// Nächstgrößere gerade Zahl, mindestens 2.
fn gerade_zahl(n: usize) -> usize {
    let n = n.max(2);
    if n % 2 == 0 {
        n
    } else {
        n + 1
    }
}

/// Wirksame Bandhöhe: 0 oder unsinnig = Vorgabe [`BREITE_MM`].
pub fn hoehe(h: f64) -> f64 {
    if h.is_finite() && h > 0.0 {
        h
    } else {
        BREITE_MM
    }
}

/// Was aus einem Schlaufenband in der Anzeige wird.
#[derive(Clone, Debug, PartialEq)]
pub enum Verlauf {
    /// Polygonzug der Schlaufenkurve.
    Zug(Vec<Vec2>),
    /// Gerade in Bandmitte — gezeichnet mit der Bandhöhe ([`hoehe`]) als
    /// Strichbreite. Unter 1,5 px je Periode ist von der Schlaufe kein Umriss
    /// mehr zu sehen; ein Punktzug würde nur ein Zittern erzeugen.
    Gerade(Vec<Vec2>),
}

impl Verlauf {
    pub fn punkte(&self) -> &[Vec2] {
        match self {
            Verlauf::Zug(p) | Verlauf::Gerade(p) => p,
        }
    }

    pub fn ist_gerade(&self) -> bool {
        matches!(self, Verlauf::Gerade(_))
    }

    pub fn in_punkte(self) -> Vec<Vec2> {
        match self {
            Verlauf::Zug(p) | Verlauf::Gerade(p) => p,
        }
    }
}

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
/// Ring. `h` ist die Bandhöhe in mm (H), `r` die Schlaufenform. `abbildung` ist
/// die Anzeige in Bildschirm-px je Welt-mm; `None` (Druck, PNG, DXF) tastet
/// weltfest ab, `Some(px)` folgt der Stufe aus [`stufe`].
///
/// Zurück kommt der Polygonzug der Kurve — **nicht** geschlossen, weil Anfang
/// und Ende des Bandes verschieden liegen (Start bei +H/2, Ende je nach `n`
/// oben oder unten). Unter 1,5 px je Periode kommt [`Verlauf::Gerade`]: die
/// Bandmitte selbst, die der Aufrufer mit der Bandhöhe als Strichbreite malt.
pub fn band(pfad: &[Vec2], geschlossen: bool, h: f64, r: f64, abbildung: Option<f64>) -> Verlauf {
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
        return Verlauf::Zug(pts);
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

    let h = hoehe(h);
    let r = if r.is_finite() && r > 0.0 { r } else { FORM };
    if !(len > 1e-9) {
        return Verlauf::Zug(pts);
    }

    let n = perioden(len, h, r);
    let t_max = std::f64::consts::TAU * n;
    let a = len / t_max;
    // Periodenlänge in mm — daraus wird die Stufe der Anzeige bestimmt.
    let p = len / n;
    let st = abbildung
        .filter(|px| px.is_finite() && *px > 0.0)
        .map(|px| stufe(p * px));
    if st == Some(Stufe::Gerade) {
        // Bandmitte: der Weg, ein Knoten je Periodengrenze; der Aufrufer
        // zeichnet ihn mit `h` als Strichbreite. Ein Knoten je Periode hält
        // die Zählung der Stufen einheitlich (5-m-Wand: 100 Punkte).
        let total = (n.ceil() as usize).max(1);
        let mut out = Vec::with_capacity(total + 1);
        for i in 0..=total {
            let x = len * (i as f64 / total as f64);
            out.push(punkt_bei(&pts, &s, x).0);
        }
        return Verlauf::Gerade(out);
    }

    let mut out: Vec<Vec2> = Vec::new();
    match st {
        // Knoten exakt auf den Periodengrenzen (Bruchteile 0, 1/N … der
        // Periode), der letzte Punkt einer Periode ist der erste der nächsten.
        Some(Stufe::Punkte(k)) => {
            let total = ((n * k as f64).round() as usize).max(2);
            out.reserve(total + 1);
            for i in 0..=total {
                let t = std::f64::consts::TAU * (i as f64 / k as f64);
                out.push(band_punkt(&pts, &s, len, a, h, t));
            }
        }
        // Ohne Abbildung (Druck, PNG, DXF): weltfeste Dichte wie bisher — ein
        // Ausdruck darf sich nicht mit dem Zoom ändern.
        _ => {
            let je = (p / (h * 0.02).max(0.25)).round().clamp(12.0, 40.0);
            let total = ((n * je).ceil() as usize).clamp(24, 6000);
            out.reserve(total + 1);
            for i in 0..=total {
                let t = t_max * (i as f64 / total as f64);
                out.push(band_punkt(&pts, &s, len, a, h, t));
            }
        }
    }
    Verlauf::Zug(out)
}

/// Kurvenpunkt zum Parameter `t` auf den Weg abgebildet.
fn band_punkt(pts: &[Vec2], s: &[f64], len: f64, a: f64, h: f64, t: f64) -> Vec2 {
    let x = a * (t + K * (2.0 * t).sin());
    let y = 0.5 * h * t.cos();
    // x bleibt bei exaktem K im Bereich 0…L; das Klemmen fängt nur
    // Rundungsreste ab und verhindert, dass ein Punkt neben dem Weg liegt.
    let (punkt, richtung) = punkt_bei(pts, s, x.clamp(0.0, len));
    punkt + richtung.perp() * y
}

/// Periodenlänge der Schlaufe bei **fester** Schlaufengröße (mm).
///
/// Die Linienart rechnet die Periode aus der Linienlänge
/// ([`perioden`]): jede Linie bekommt eine runde Anzahl Schlaufen, die Form
/// wird dafür leicht gestaucht oder gestreckt. In einer Schraffur liegen
/// viele Bänder nebeneinander, und jedes würde für sich gerechnet — die
/// Schlaufen stünden dann an jeder Kante anders (T0136). Für das Muster gilt
/// deshalb `p = r · H`, unabhängig von der Länge des beschnittenen Stücks.
pub fn periode(h: f64, r: f64) -> f64 {
    let h = if h.is_finite() && h > 0.0 { h } else { BREITE_MM };
    let r = if r.is_finite() && r > 0.0 { r } else { FORM };
    (r * h).max(1.0)
}

/// Schlaufenband mit **fester** Periodenlänge — die Schraffur (T0136).
///
/// Wie [`band`], aber die Phase hängt am **Weltnullpunkt** statt an der Länge
/// des Wegs: die Kurve beginnt bei `t = 0` mit `x = 0` und hat ihre Perioden
/// bei `x = k · p` ([`periode`]). Damit zeigen alle Bänder einer Fläche
/// dieselben Schlaufenlagen, und zwei Flächen mit verschiedenem Rand passen
/// an der gemeinsamen Kante zusammen — dieselbe Regel wie beim Musterraster.
///
/// Zurück kommt ein Punktzug in **Rasterkoordinaten der Kurve**: `x` ist der
/// Weg längs (`u`, absolut, nicht relativ zu `u0`), `y` der Versatz quer dazu
/// gegen die Bandmitte (±H/2). Der Aufrufer setzt die Bandmitte auf die
/// gewünschte Reihe. Gezeichnet wird ein Polygonzug; beschnitten wird er an
/// der Kontur, nicht gestaucht.
///
/// `u0`…`u1` begrenzen den Bereich, den der Punktzug sicher abdeckt. Gesampelt
/// wird immer über **ganze** Perioden — innerhalb einer Periode läuft `x`
/// zurück (die Schlaufe), ein Abbruch mitten darin ließe ein Stück aus.
///
/// `abbildung` gilt wie bei [`band`]; unter 1,5 px je Periode kommt
/// [`Verlauf::Gerade`] (eine Strecke in Bandmitte).
pub fn band_fest(h: f64, r: f64, u0: f64, u1: f64, abbildung: Option<f64>) -> Verlauf {
    let h = hoehe(h);
    if !u0.is_finite() || !u1.is_finite() {
        return Verlauf::Zug(Vec::new());
    }
    let (u0, u1) = if u0 <= u1 { (u0, u1) } else { (u1, u0) };
    let p = periode(h, r);
    let a = p / std::f64::consts::TAU;

    // Eine Periode vor und zwei dahinter: so liegt der Bereich sicher im
    // erzeugten Zug, und der Beschnitt an der Kontur bleibt die einzige Kürzung.
    let k0 = (u0 / p).floor() - 1.0;
    let k1 = (u1 / p).floor() + 2.0;
    let t0 = k0 * std::f64::consts::TAU;
    let t1 = k1 * std::f64::consts::TAU;

    let st = abbildung
        .filter(|px| px.is_finite() && *px > 0.0)
        .map(|px| stufe(p * px));
    if st == Some(Stufe::Gerade) {
        // Bandmitte als Gerade; der Aufrufer zeichnet sie mit `h` als
        // Strichbreite.
        return Verlauf::Gerade(vec![Vec2::new(u0 - p, 0.0), Vec2::new(u1 + p, 0.0)]);
    }

    let mut out: Vec<Vec2> = Vec::new();
    match st {
        // Knoten auf den Periodengrenzen, von der Phase am Weltnullpunkt aus
        // gezählt: `t = k · 2π/N`, damit benachbarte Bänder nicht driften.
        Some(Stufe::Punkte(k)) => {
            let total = (((k1 - k0) * k as f64).round() as usize).max(2);
            out.reserve(total + 1);
            for i in 0..=total {
                let t = std::f64::consts::TAU * (k0 + i as f64 / k as f64);
                out.push(Vec2::new(a * (t + K * (2.0 * t).sin()), 0.5 * h * t.cos()));
            }
        }
        // Ohne Abbildung: weltfest wie bisher (32 je Schlaufe).
        _ => {
            let per = 32.0;
            let n = (((k1 - k0) * per).ceil() as usize).clamp(24, 4000);
            out.reserve(n + 1);
            for i in 0..=n {
                let t = t0 + (t1 - t0) * (i as f64 / n as f64);
                out.push(Vec2::new(a * (t + K * (2.0 * t).sin()), 0.5 * h * t.cos()));
            }
        }
    }
    Verlauf::Zug(out)
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
                let b = band(&gerade(len), false, h, r, None).in_punkte();
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
        let b = band(&gerade(2000.0), false, 120.0, 0.5, None).in_punkte();
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
        let b = band(&pfad, false, 80.0, 0.5, None).in_punkte();
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
        let b = band(&pfad, true, 80.0, 0.5, None).in_punkte();
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

    /// Die Schraffur hat ein **festes** Schlaufenmaß: die Scheitel liegen
    /// genau auf `k · p`, unabhängig davon, wie lang der abgefragte Bereich
    /// ist (T0136). Die Linienart bekäme für 830 mm eine Periode von 50,3 mm.
    #[test]
    fn schraffur_haelt_das_schlaufenmass() {
        let (h, r) = (100.0, FORM);
        let p = periode(h, r);
        assert!((p - 50.0).abs() < 1e-12, "Periode {p}");
        for (u0, u1) in [(-800.0, 800.0), (-800.0, 5000.0), (0.0, 830.0)] {
            let b = band_fest(h, r, u0, u1, None).in_punkte();
            let mut scheitel: Vec<f64> = b
                .iter()
                .filter(|q| (q.y - 0.5 * h).abs() < 1e-9)
                .map(|q| q.x)
                .collect();
            scheitel.sort_by(|a, b| a.partial_cmp(b).unwrap());
            scheitel.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            assert!(scheitel.len() >= 3, "Bereich {u0}…{u1}: {}", scheitel.len());
            for w in scheitel.windows(2) {
                assert!(
                    ((w[1] - w[0]) - p).abs() < 1e-9,
                    "Schlaufenmaß {} statt {p} (Bereich {u0}…{u1})",
                    w[1] - w[0]
                );
            }
            for x in &scheitel {
                assert!(
                    (x / p - (x / p).round()).abs() < 1e-9,
                    "Scheitel bei {x} liegt nicht auf k·p"
                );
            }
        }
    }

    /// Die Phase hängt am Weltnullpunkt, nicht am abgefragten Bereich: ein
    /// längerer Bereich verlängert den Punktzug nur, er verschiebt ihn nicht.
    #[test]
    fn schraffur_phase_haengt_am_nullpunkt() {
        let kurz = band_fest(100.0, FORM, -500.0, 900.0, None).in_punkte();
        let lang = band_fest(100.0, FORM, -500.0, 4000.0, None).in_punkte();
        assert!(lang.len() > kurz.len());
        for (a, b) in kurz.iter().zip(lang.iter()) {
            assert!(
                a.dist(*b) < 1e-9,
                "gleicher Anfang, andere Punkte: {a:?} gegen {b:?}"
            );
        }
    }

    /// Das Band bleibt in seiner Höhe und springt nicht zwischen zwei
    /// Stützpunkten (ein Sprung wäre eine Lücke im Muster).
    #[test]
    fn schraffur_band_bleibt_zusammenhaengend() {
        let (h, r) = (100.0, 0.5);
        let b = band_fest(h, r, -500.0, 2000.0, None).in_punkte();
        let p = periode(h, r);
        for q in &b {
            assert!(q.y.abs() <= 0.5 * h + 1e-9, "Punkt außerhalb der Bandhöhe");
        }
        for w in b.windows(2) {
            assert!(
                w[0].dist(w[1]) < p / 4.0,
                "Sprung von {:?} nach {:?}",
                w[0],
                w[1]
            );
        }
    }

    /// Degenerierte Eingaben bleiben stehen statt zu panicken.
    #[test]
    fn entartete_eingaben() {
        assert!(band(&[], false, 100.0, 0.5, None).in_punkte().is_empty());
        assert_eq!(band(&[Vec2::new(1.0, 2.0)], false, 100.0, 0.5, None).in_punkte().len(), 1);
        let doppelt = vec![Vec2::new(5.0, 5.0), Vec2::new(5.0, 5.0)];
        assert_eq!(band(&doppelt, false, 100.0, 0.5, None).in_punkte().len(), 1);
    }
}
