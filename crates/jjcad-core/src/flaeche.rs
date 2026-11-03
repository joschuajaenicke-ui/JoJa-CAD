//! Flächen schneiden und vereinigen.
//!
//! Zwei Aktionen teilen sich diese Geometrie:
//!
//! * **Vereinigen** (`modify.area.union`) verschmilzt zwei oder mehr
//!   **angrenzende** Füllflächen zu einer. Die Umgrenzungen werden als ebenes
//!   Netz behandelt: jede Kante wird an allen Kreuzungen und Stößen geteilt,
//!   gemeinsame (doppelt vorhandene) Kanten fallen weg, Kanten im Inneren
//!   einer anderen Fläche ebenso. Aus den übrigen Kanten werden die Ringe neu
//!   zusammengesetzt — Außenring gegen den Uhrzeigersinn, Inseln im
//!   Uhrzeigersinn.
//! * **Schneiden** (`modify.area.split`) zerlegt eine Füllfläche entlang einer
//!   Punktfolge in zwei Teilflächen. Die Punktfolge darf außerhalb der Fläche
//!   beginnen und enden; maßgeblich ist die Strecke zwischen dem ersten und
//!   dem letzten Randübertritt. Kreuzt sie den Rand mehrfach, wird das
//!   abgewiesen statt geraten.
//!
//! Beide Rechnungen arbeiten auf reinen Ringen (kein Modellbezug), damit sie
//! ohne Dokument prüfbar sind.

use crate::geom::*;
use std::collections::HashMap;

/// Eine Füllfläche als reine Geometrie: Außenring plus Inseln.
#[derive(Clone, Debug, PartialEq)]
pub struct Flaeche {
    pub outer: Vec<Vec2>,
    pub holes: Vec<Vec<Vec2>>,
}

/// Zwei Punkte gelten als derselbe, wenn sie näher als 1 µm liegen.
const PUNKT_EPS: f64 = 1e-3;

/// Rasterweite der Punkt-Schlüssel im Kantennetz (0,001 mm).
const SCHLUESSEL_MM: f64 = 1e-3;

/// Ein Punkt-Schlüssel für das Kantennetz — gerundet, damit gerechnete
/// Schnittpunkte aus zwei Kanten denselben Knoten treffen.
fn schluessel(p: Vec2) -> (i64, i64) {
    (
        (p.x / SCHLUESSEL_MM).round() as i64,
        (p.y / SCHLUESSEL_MM).round() as i64,
    )
}

/// Schlüssel einer Kante, richtungsunabhängig — so findet die Vereinigungs-
/// rechnung zwei Mal dieselbe (gemeinsame) Kante wieder.
fn kanten_schluessel(a: Vec2, b: Vec2) -> ((i64, i64), (i64, i64)) {
    let (ka, kb) = (schluessel(a), schluessel(b));
    if ka <= kb {
        (ka, kb)
    } else {
        (kb, ka)
    }
}

/// Liegt der Punkt in der gefüllten Fläche? Inseln zählen als außen.
fn in_flaeche(p: Vec2, f: &Flaeche) -> bool {
    point_in_polygon(p, &f.outer) && !f.holes.iter().any(|h| point_in_polygon(p, h))
}

/// Ring mathematisch positiv (gegen den Uhrzeigersinn) drehen.
fn gegen_uhrzeigersinn(ring: &[Vec2]) -> Vec<Vec2> {
    let mut r = ring.to_vec();
    if polygon_area(&r) < 0.0 {
        r.reverse();
    }
    r
}

/// Ring mathematisch negativ (im Uhrzeigersinn) drehen — für Inseln.
fn im_uhrzeigersinn(ring: &[Vec2]) -> Vec<Vec2> {
    let mut r = ring.to_vec();
    if polygon_area(&r) > 0.0 {
        r.reverse();
    }
    r
}

/// Doppelte und geradlinig zwischenliegende Punkte entfernen.
///
/// Die Vereinigungsrechnung teilt Kanten an jeder Kreuzung; dadurch bleiben
/// Zwischenpunkte auf geraden Kanten stehen. Sie sind geometrisch harmlos,
/// blähen aber Umgrenzung und Griffe auf.
fn entdopple(ring: &[Vec2]) -> Vec<Vec2> {
    let mut r: Vec<Vec2> = Vec::new();
    for p in ring {
        if r.last().map(|q: &Vec2| q.dist(*p) < PUNKT_EPS).unwrap_or(false) {
            continue;
        }
        r.push(*p);
    }
    while r.len() > 1 && r[0].dist(*r.last().unwrap()) < PUNKT_EPS {
        r.pop();
    }
    let n = r.len();
    if n < 3 {
        return r;
    }
    let mut out: Vec<Vec2> = Vec::new();
    for i in 0..n {
        let prev = r[(i + n - 1) % n];
        let cur = r[i];
        let next = r[(i + 1) % n];
        let d1 = cur - prev;
        let d2 = next - cur;
        if d1.len() < PUNKT_EPS || d2.len() < PUNKT_EPS {
            continue;
        }
        // Kreuzprodukt verschwindet und die Richtung bleibt: Zwischenpunkt
        // einer geraden Kante. 1e-6 rad Toleranz — enger als jede Zeichnung,
        // weiter als das Rechenrauschen der Schnittpunkte.
        if d1.cross(d2).abs() <= 1e-6 * d1.len() * d2.len() && d1.dot(d2) > 0.0 {
            continue;
        }
        out.push(cur);
    }
    if out.len() >= 3 {
        out
    } else {
        r
    }
}

/// Eine Kante des ebenen Netzes samt Herkunftsfläche.
#[derive(Clone, Copy, Debug)]
struct Kante {
    a: Vec2,
    b: Vec2,
    /// Index der Fläche, aus der die Kante stammt.
    poly: usize,
}

/// Verschmilzt Flächen zu **einer**.
///
/// Voraussetzung ist, dass die Flächen aneinanderstoßen (gemeinsame Kante oder
/// Überdeckung); sonst bleibt mehr als ein Außenring übrig und die Rechnung
/// meldet einen Fehler — still zwei Flächen zu erzeugen wäre kein Vereinigen.
pub fn vereinigen(flaechen: &[Flaeche]) -> Result<Flaeche, String> {
    let mut sauber: Vec<Flaeche> = Vec::new();
    for f in flaechen {
        if f.outer.len() < 3 {
            continue;
        }
        sauber.push(Flaeche {
            outer: gegen_uhrzeigersinn(&f.outer),
            holes: f
                .holes
                .iter()
                .filter(|h| h.len() >= 3)
                .map(|h| im_uhrzeigersinn(h))
                .collect(),
        });
    }
    if sauber.is_empty() {
        return Err("Keine Füllfläche ausgewählt".into());
    }
    if sauber.len() == 1 {
        return Ok(sauber[0].clone());
    }

    // 1. Kanten aller Ringe einsammeln.
    let mut kanten: Vec<Kante> = Vec::new();
    for (pi, f) in sauber.iter().enumerate() {
        for ring in std::iter::once(&f.outer).chain(f.holes.iter()) {
            for i in 0..ring.len() {
                let a = ring[i];
                let b = ring[(i + 1) % ring.len()];
                if a.dist(b) > PUNKT_EPS {
                    kanten.push(Kante { a, b, poly: pi });
                }
            }
        }
    }

    // 2. Jede Kante an Kreuzungen und Stößen teilen.
    let mut stuecke: Vec<Kante> = Vec::new();
    for (i, k) in kanten.iter().enumerate() {
        let d = k.b - k.a;
        let l2 = d.len2();
        if l2 < 1e-18 {
            continue;
        }
        let mut ts: Vec<f64> = Vec::new();
        for (j, o) in kanten.iter().enumerate() {
            if i == j {
                continue;
            }
            if let Some(x) = line_line(k.a, k.b, o.a, o.b, false, false) {
                let t = (x - k.a).dot(d) / l2;
                if t > 1e-9 && t < 1.0 - 1e-9 {
                    ts.push(t);
                }
            }
            // T-Stoß: ein Endpunkt der anderen Kante liegt auf dieser.
            for v in [o.a, o.b] {
                let t = (v - k.a).dot(d) / l2;
                if t > 1e-9 && t < 1.0 - 1e-9 && v.dist(k.a + d * t) < PUNKT_EPS {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        let mut von = k.a;
        for t in ts {
            let bis = k.a + d * t;
            if von.dist(bis) > PUNKT_EPS {
                stuecke.push(Kante {
                    a: von,
                    b: bis,
                    poly: k.poly,
                });
            }
            von = bis;
        }
        if von.dist(k.b) > PUNKT_EPS {
            stuecke.push(Kante {
                a: von,
                b: k.b,
                poly: k.poly,
            });
        }
    }

    // 3. Gemeinsame Kanten (doppelt vorhanden) und Kanten im Inneren einer
    //    anderen Fläche sind nicht Teil der neuen Umgrenzung.
    let mut zaehler: HashMap<((i64, i64), (i64, i64)), usize> = HashMap::new();
    for s in &stuecke {
        *zaehler.entry(kanten_schluessel(s.a, s.b)).or_insert(0) += 1;
    }
    let mut behalten: Vec<Kante> = Vec::new();
    for s in stuecke {
        if zaehler[&kanten_schluessel(s.a, s.b)] > 1 {
            continue;
        }
        let m = (s.a + s.b) * 0.5;
        let innen = sauber
            .iter()
            .enumerate()
            .any(|(pi, f)| pi != s.poly && in_flaeche(m, f));
        if innen {
            continue;
        }
        behalten.push(s);
    }
    if behalten.is_empty() {
        return Err("Flächen überdecken sich vollständig — nichts zu vereinigen".into());
    }

    // 4. Ringe aus den übrigen Kanten zusammensetzen.
    let mut ausgehend: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in behalten.iter().enumerate() {
        ausgehend.entry(schluessel(s.a)).or_default().push(i);
    }
    let mut benutzt = vec![false; behalten.len()];
    let mut ringe: Vec<Vec<Vec2>> = Vec::new();
    for start in 0..behalten.len() {
        if benutzt[start] {
            continue;
        }
        let start_key = schluessel(behalten[start].a);
        let mut ring: Vec<Vec2> = Vec::new();
        let mut idx = start;
        let mut richtung = behalten[start].b - behalten[start].a;
        loop {
            benutzt[idx] = true;
            let k = behalten[idx];
            ring.push(k.a);
            let end_key = schluessel(k.b);
            if end_key == start_key {
                break;
            }
            let Some(kandidaten) = ausgehend.get(&end_key) else {
                return Err("Fläche konnte nicht zusammengesetzt werden".into());
            };
            // Am Knoten die am stärksten rechts abbiegende Kante nehmen: so
            // bleibt das Innere links und die äußere Umfahrung entsteht.
            let mut naechste: Option<usize> = None;
            let mut kleinster = f64::INFINITY;
            for &c in kandidaten {
                if benutzt[c] {
                    continue;
                }
                let d = behalten[c].b - behalten[c].a;
                let winkel = richtung.signed_angle_to(d);
                if winkel < kleinster {
                    kleinster = winkel;
                    naechste = Some(c);
                }
            }
            let Some(n) = naechste else {
                return Err("Fläche konnte nicht zusammengesetzt werden".into());
            };
            richtung = behalten[n].b - behalten[n].a;
            idx = n;
        }
        if ring.len() >= 3 {
            ringe.push(ring);
        }
    }

    // 5. Ringe klassifizieren: positiv = außen, negativ = Insel.
    let mut aussen: Vec<Vec<Vec2>> = Vec::new();
    let mut inseln: Vec<Vec<Vec2>> = Vec::new();
    for r in ringe {
        let a = polygon_area(&r);
        if a > 1e-6 {
            aussen.push(r);
        } else if a < -1e-6 {
            inseln.push(r);
        }
    }
    if aussen.is_empty() {
        return Err("Fläche konnte nicht zusammengesetzt werden".into());
    }
    if aussen.len() > 1 {
        return Err("Flächen berühren sich nicht — Vereinigen braucht angrenzende Flächen".into());
    }
    let outer = entdopple(&aussen.remove(0));
    let mut holes: Vec<Vec<Vec2>> = Vec::new();
    for h in inseln {
        if point_in_polygon(h[0], &outer) {
            holes.push(entdopple(&h));
        }
    }
    Ok(Flaeche { outer, holes })
}

/// Kreuzt die Punktfolge den Ring? Grundlage der Zielsuche, wenn keine
/// Fläche ausgewählt ist: die Schnittlinie bestimmt dann die Fläche selbst.
pub fn kreuzt_ring(ring: &[Vec2], weg: &[Vec2]) -> bool {
    if ring.len() < 3 || weg.len() < 2 {
        return false;
    }
    for w in weg.windows(2) {
        for i in 0..ring.len() {
            let c = ring[i];
            let d = ring[(i + 1) % ring.len()];
            if line_line(w[0], w[1], c, d, false, false).is_some() {
                return true;
            }
        }
    }
    false
}

/// Ein Randübertritt der Schnittlinie.
#[derive(Clone, Copy, Debug)]
struct Treffer {
    /// Lage auf der Schnittlinie: Abschnittsindex + Anteil.
    pos: f64,
    p: Vec2,
    /// Ringkante, auf der der Übertritt liegt.
    kante: usize,
    /// Anteil auf dieser Ringkante.
    t: f64,
}

/// Zerlegt eine Fläche entlang einer Punktfolge in **zwei** Teilflächen.
///
/// Die Punktfolge ist die Schnittlinie. Sie muss den Außenring an genau zwei
/// Stellen übertreten; der erste und der letzte Abschnitt dürfen dafür über
/// den Rand hinaus verlängert werden (Klick innerhalb der Fläche genügt).
pub fn teilen(
    outer: &[Vec2],
    holes: &[Vec<Vec2>],
    schnitt: &[Vec2],
) -> Result<(Flaeche, Flaeche), String> {
    if outer.len() < 3 {
        return Err("Fläche hat keine Umgrenzung".into());
    }
    let ring = gegen_uhrzeigersinn(outer);
    let mut weg: Vec<Vec2> = Vec::new();
    for p in schnitt {
        if weg
            .last()
            .map(|q: &Vec2| q.dist(*p) < PUNKT_EPS)
            .unwrap_or(false)
        {
            continue;
        }
        weg.push(*p);
    }
    if weg.len() < 2 {
        return Err("Schnittlinie braucht zwei verschiedene Punkte".into());
    }

    // Kreuzungen der Schnittlinie mit dem Außenring. Der erste und der letzte
    // Abschnitt werden verlängert gesucht, damit ein Klick im Inneren der
    // Fläche als Schnittlinie taugt.
    let letzter = weg.len() - 2;
    let mut treffer: Vec<Treffer> = Vec::new();
    for i in 0..weg.len() - 1 {
        let a = weg[i];
        let b = weg[i + 1];
        let dir = b - a;
        let l2 = dir.len2();
        if l2 < 1e-18 {
            continue;
        }
        let verlaengert = i == 0 || i == letzter;
        for k in 0..ring.len() {
            let c = ring[k];
            let d = ring[(k + 1) % ring.len()];
            let Some(x) = line_line(a, b, c, d, verlaengert, false) else {
                continue;
            };
            let t = (x - a).dot(dir) / l2;
            let im_abschnitt = if i == 0 && i == letzter {
                // Einziger Abschnitt: nach beiden Seiten verlängert.
                true
            } else if i == 0 {
                t <= 1.0 + 1e-9
            } else if i == letzter {
                t >= -1e-9
            } else {
                (-1e-9..=1.0 + 1e-9).contains(&t)
            };
            if !im_abschnitt {
                continue;
            }
            let u = (x - c).dot(d - c) / (d - c).len2();
            if !(-1e-9..=1.0 + 1e-9).contains(&u) {
                continue;
            }
            treffer.push(Treffer {
                pos: i as f64 + t,
                p: x,
                kante: k,
                t: u.clamp(0.0, 1.0),
            });
        }
    }
    treffer.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    // Ein Übertritt auf einer Ringecke wird von beiden Ringkanten gefunden.
    let mut punkte: Vec<Treffer> = Vec::new();
    for t in treffer {
        if punkte
            .last()
            .map(|u: &Treffer| u.p.dist(t.p) < PUNKT_EPS)
            .unwrap_or(false)
        {
            continue;
        }
        punkte.push(t);
    }
    if punkte.len() != 2 {
        return Err(if punkte.len() < 2 {
            "Schnittlinie muss die Fläche von Rand zu Rand durchqueren".into()
        } else {
            "Schnittlinie kreuzt den Rand mehrfach".into()
        });
    }
    let (eintritt, austritt) = (punkte[0], punkte[1]);

    // Schnittweg zwischen den beiden Randpunkten.
    let mut pfad: Vec<Vec2> = vec![eintritt.p];
    for (k, p) in weg.iter().enumerate() {
        let pos = k as f64;
        if pos > eintritt.pos + 1e-9 && pos < austritt.pos - 1e-9 {
            pfad.push(*p);
        }
    }
    pfad.push(austritt.p);
    let flaeche = Flaeche {
        outer: ring.clone(),
        holes: holes.to_vec(),
    };
    for p in &pfad[1..pfad.len() - 1] {
        if !in_flaeche(*p, &flaeche) {
            return Err("Schnittlinie verlässt die Fläche".into());
        }
    }
    // Eine Insel darf nicht zerschnitten werden — dann wären es mehr als zwei
    // Teilflächen oder ein Loch mit Schnittkante.
    for h in holes {
        if h.len() < 3 {
            continue;
        }
        for i in 0..h.len() {
            let c = h[i];
            let d = h[(i + 1) % h.len()];
            for w in pfad.windows(2) {
                if line_line(w[0], w[1], c, d, false, false).is_some() {
                    return Err("Schnittlinie kreuzt eine Insel".into());
                }
            }
        }
    }

    // Randpunkte in den Ring einsetzen.
    let mut pro_kante: HashMap<usize, Vec<(f64, Vec2)>> = HashMap::new();
    for t in [eintritt, austritt] {
        pro_kante.entry(t.kante).or_default().push((t.t, t.p));
    }
    for liste in pro_kante.values_mut() {
        liste.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }
    let mut neu: Vec<Vec2> = Vec::new();
    for k in 0..ring.len() {
        neu.push(ring[k]);
        if let Some(liste) = pro_kante.get(&k) {
            for (_, p) in liste {
                if neu
                    .last()
                    .map(|q: &Vec2| q.dist(*p) < PUNKT_EPS)
                    .unwrap_or(false)
                {
                    continue;
                }
                neu.push(*p);
            }
        }
    }
    let mut sauber: Vec<Vec2> = Vec::new();
    for p in neu {
        if sauber
            .last()
            .map(|q: &Vec2| q.dist(p) < PUNKT_EPS)
            .unwrap_or(false)
        {
            continue;
        }
        sauber.push(p);
    }
    while sauber.len() > 1 && sauber[0].dist(*sauber.last().unwrap()) < PUNKT_EPS {
        sauber.pop();
    }
    let ia = sauber
        .iter()
        .position(|p| p.dist(eintritt.p) < PUNKT_EPS)
        .ok_or("Randpunkt nicht gefunden")?;
    let ib = sauber
        .iter()
        .position(|p| p.dist(austritt.p) < PUNKT_EPS)
        .ok_or("Randpunkt nicht gefunden")?;
    if ia == ib {
        return Err("Schnittlinie trifft den Rand nur in einem Punkt".into());
    }

    // Die beiden Ringbögen zwischen den Randpunkten.
    let n = sauber.len();
    let mut teil_a: Vec<Vec2> = Vec::new();
    let mut i = ia;
    loop {
        teil_a.push(sauber[i]);
        if i == ib {
            break;
        }
        i = (i + 1) % n;
    }
    let mut teil_b: Vec<Vec2> = Vec::new();
    let mut i = ib;
    loop {
        teil_b.push(sauber[i]);
        if i == ia {
            break;
        }
        i = (i + 1) % n;
    }
    // Schnittweg ohne die beiden Randpunkte (die stehen schon am Bogenende).
    let innen: Vec<Vec2> = pfad[1..pfad.len() - 1].to_vec();
    for p in innen.iter().rev() {
        teil_a.push(*p);
    }
    for p in innen.iter() {
        teil_b.push(*p);
    }

    // Inseln gehören zu der Teilfläche, in der sie liegen.
    let mut holes_a: Vec<Vec<Vec2>> = Vec::new();
    let mut holes_b: Vec<Vec<Vec2>> = Vec::new();
    for h in holes {
        if h.len() < 3 {
            continue;
        }
        if point_in_polygon(h[0], &teil_a) {
            holes_a.push(im_uhrzeigersinn(h));
        } else if point_in_polygon(h[0], &teil_b) {
            holes_b.push(im_uhrzeigersinn(h));
        } else {
            return Err("Insel konnte keiner Teilfläche zugeordnet werden".into());
        }
    }

    let a = entdopple(&teil_a);
    let b = entdopple(&teil_b);
    if a.len() < 3 || b.len() < 3 {
        return Err("Schnittlinie teilt die Fläche nicht".into());
    }
    let fl_a = Flaeche {
        outer: a,
        holes: holes_a,
    };
    let fl_b = Flaeche {
        outer: b,
        holes: holes_b,
    };
    if polygon_area(&fl_a.outer).abs() < 1.0 || polygon_area(&fl_b.outer).abs() < 1.0 {
        return Err("Schnittlinie teilt die Fläche nicht".into());
    }
    Ok((fl_a, fl_b))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rechteck(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)]
    }

    fn flaeche(outer: Vec<Vec2>) -> Flaeche {
        Flaeche {
            outer,
            holes: Vec::new(),
        }
    }

    /// Zwei aneinanderliegende Rechtecke ergeben ein Rechteck — mit den
    /// **exakten** Ecken, nicht mit Rastertoleranz.
    #[test]
    fn zwei_angrenzende_rechtecke_werden_ein_rechteck() {
        let a = flaeche(rechteck(0.0, 0.0, 100.0, 100.0));
        let b = flaeche(rechteck(100.0, 0.0, 200.0, 100.0));
        let u = vereinigen(&[a, b]).expect("Vereinigen");
        assert_eq!(u.outer.len(), 4, "Ecken: {:?}", u.outer);
        let mut ecken: Vec<(i64, i64)> = u
            .outer
            .iter()
            .map(|p| (p.x.round() as i64, p.y.round() as i64))
            .collect();
        ecken.sort();
        assert_eq!(ecken, vec![(0, 0), (0, 100), (200, 0), (200, 100)]);
        assert!((polygon_area(&u.outer) - 20000.0).abs() < 1e-6);
    }

    /// Gestapelt (L-Form): die gemeinsame Kante verschwindet, die Innenkante
    /// bleibt als Knick stehen.
    #[test]
    fn l_form_aus_zwei_rechtecken() {
        let a = flaeche(rechteck(0.0, 0.0, 100.0, 100.0));
        let b = flaeche(rechteck(100.0, 0.0, 200.0, 50.0));
        let u = vereinigen(&[a, b]).expect("Vereinigen");
        assert!((polygon_area(&u.outer).abs() - 15000.0).abs() < 1e-6);
        // Die gemeinsame Kante x=100 von y=0…50 ist weg, der Knick bleibt.
        assert!(
            !u.outer
                .windows(2)
                .any(|w| (w[0].x - 100.0).abs() < 1e-6
                    && (w[1].x - 100.0).abs() < 1e-6
                    && (w[0].y - 50.0).abs() < 1e-6
                    && (w[1].y - 0.0).abs() < 1e-6),
            "gemeinsame Kante steht noch: {:?}",
            u.outer
        );
    }

    /// Nicht aneinanderstoßende Flächen lassen sich nicht vereinigen — die
    /// Meldung ist ein Fehler, keine stille Zweitfläche.
    #[test]
    fn getrennte_flaechen_melden_einen_fehler() {
        let a = flaeche(rechteck(0.0, 0.0, 100.0, 100.0));
        let b = flaeche(rechteck(500.0, 0.0, 600.0, 100.0));
        let err = vereinigen(&[a, b]).unwrap_err();
        assert!(err.contains("angrenzende"), "{err}");
    }

    /// Eine Fläche innerhalb einer anderen verschwindet, die äußere bleibt.
    #[test]
    fn innenliegende_flaeche_verschwindet() {
        let a = flaeche(rechteck(0.0, 0.0, 1000.0, 1000.0));
        let b = flaeche(rechteck(100.0, 100.0, 200.0, 200.0));
        let u = vereinigen(&[a, b]).expect("Vereinigen");
        assert_eq!(u.outer.len(), 4);
        assert!((polygon_area(&u.outer).abs() - 1_000_000.0).abs() < 1e-6);
        assert!(u.holes.is_empty());
    }

    /// Inseln der Teile bleiben Inseln des Ergebnisses.
    #[test]
    fn inseln_bleiben_erhalten() {
        let mut a = flaeche(rechteck(0.0, 0.0, 100.0, 100.0));
        a.holes = vec![rechteck(20.0, 20.0, 40.0, 40.0)];
        let b = flaeche(rechteck(100.0, 0.0, 200.0, 100.0));
        let u = vereinigen(&[a, b]).expect("Vereinigen");
        assert_eq!(u.holes.len(), 1, "Insel verloren: {:?}", u.holes);
        assert!((polygon_area(&u.outer).abs() - 20000.0).abs() < 1e-6);
    }

    /// Drei Flächen in einer Reihe — die Rechnung ist nicht auf ein Paar
    /// beschränkt.
    #[test]
    fn drei_flaechen_werden_eine() {
        let a = flaeche(rechteck(0.0, 0.0, 100.0, 100.0));
        let b = flaeche(rechteck(100.0, 0.0, 200.0, 100.0));
        let c = flaeche(rechteck(200.0, 0.0, 300.0, 100.0));
        let u = vereinigen(&[a, b, c]).expect("Vereinigen");
        assert!((polygon_area(&u.outer).abs() - 30000.0).abs() < 1e-6);
        assert_eq!(u.outer.len(), 4, "{:?}", u.outer);
    }

    /// Die Zielsuche erkennt eine Fläche auch an der gekreuzten Kante — der
    /// Nutzer darf die Schnittlinie außerhalb beginnen lassen.
    #[test]
    fn kreuzt_ring_findet_die_geschnittene_flaeche() {
        let ring = rechteck(0.0, 0.0, 1000.0, 500.0);
        assert!(kreuzt_ring(&ring, &[v(500.0, -100.0), v(500.0, 600.0)]));
        assert!(!kreuzt_ring(&ring, &[v(500.0, 600.0), v(500.0, 900.0)]));
    }

    /// Ein Schnitt quer durch ein Rechteck ergibt zwei Rechtecke mit exakten
    /// Ecken und der halben Fläche.
    #[test]
    fn schnitt_teilt_ein_rechteck_in_zwei() {
        let ring = rechteck(0.0, 0.0, 1000.0, 500.0);
        let (a, b) = teilen(&ring, &[], &[v(500.0, -100.0), v(500.0, 600.0)])
            .expect("Teilen");
        assert!((polygon_area(&a.outer).abs() - 250_000.0).abs() < 1e-6);
        assert!((polygon_area(&b.outer).abs() - 250_000.0).abs() < 1e-6);
        let summe = polygon_area(&a.outer).abs() + polygon_area(&b.outer).abs();
        assert!((summe - 500_000.0).abs() < 1e-6);
    }

    /// Der Schnitt darf innerhalb der Fläche beginnen und enden — die
    /// Endabschnitte werden bis zum Rand verlängert.
    #[test]
    fn schnitt_innerhalb_der_flaeche_wird_verlaengert() {
        let ring = rechteck(0.0, 0.0, 1000.0, 500.0);
        let (a, b) = teilen(&ring, &[], &[v(500.0, 100.0), v(500.0, 400.0)])
            .expect("Teilen");
        assert!((polygon_area(&a.outer).abs() - 250_000.0).abs() < 1e-6);
        assert!((polygon_area(&b.outer).abs() - 250_000.0).abs() < 1e-6);
    }

    /// Ein geknickter Schnitt (Polyline) teilt ebenfalls — die Teilflächen
    /// tragen den Knick.
    #[test]
    fn geknickter_schnitt_teilt_die_flaeche() {
        let ring = rechteck(0.0, 0.0, 1000.0, 500.0);
        let (a, b) = teilen(
            &ring,
            &[],
            &[v(500.0, -100.0), v(500.0, 250.0), v(1000.0, 250.0)],
        )
        .expect("Teilen");
        let summe = polygon_area(&a.outer).abs() + polygon_area(&b.outer).abs();
        assert!((summe - 500_000.0).abs() < 1e-6);
        // Beide Teile haben den Knickpunkt (500, 250).
        for f in [&a, &b] {
            assert!(
                f.outer.iter().any(|p| p.dist(v(500.0, 250.0)) < 1e-6),
                "Knick fehlt: {:?}",
                f.outer
            );
        }
    }

    /// Eine Schnittlinie, die die Fläche verlässt und wieder betritt, kreuzt den
    /// Rand mehrfach — das wird abgewiesen statt in beliebig viele Teile zu
    /// zerlegen.
    #[test]
    fn schnitt_mit_mehrfachem_randkontakt_ist_ein_fehler() {
        let ring = rechteck(0.0, 0.0, 1000.0, 500.0);
        let err = teilen(
            &ring,
            &[],
            &[
                v(500.0, 100.0),
                v(1200.0, 100.0),
                v(1200.0, 400.0),
                v(500.0, 400.0),
            ],
        )
        .unwrap_err();
        assert!(err.contains("mehrfach"), "{err}");
    }

    /// Ein Schnitt, der eine Insel trifft, wird abgewiesen statt halb
    /// ausgeführt.
    #[test]
    fn schnitt_durch_eine_insel_ist_ein_fehler() {
        let ring = rechteck(0.0, 0.0, 1000.0, 500.0);
        let insel = rechteck(450.0, 200.0, 550.0, 300.0);
        let err = teilen(&ring, &[insel], &[v(500.0, -100.0), v(500.0, 600.0)])
            .unwrap_err();
        assert!(err.contains("Insel"), "{err}");
    }

    /// Schneiden und danach wieder vereinigen ergibt die Ausgangsfläche.
    #[test]
    fn schnitt_und_vereinigung_ergeben_die_ausgangsflaeche() {
        let ring = rechteck(0.0, 0.0, 1000.0, 500.0);
        let (a, b) = teilen(&ring, &[], &[v(500.0, -100.0), v(500.0, 600.0)])
            .expect("Teilen");
        let u = vereinigen(&[a, b]).expect("Vereinigen");
        assert!((polygon_area(&u.outer).abs() - 500_000.0).abs() < 1e-6);
        assert_eq!(u.outer.len(), 4, "{:?}", u.outer);
    }
}
