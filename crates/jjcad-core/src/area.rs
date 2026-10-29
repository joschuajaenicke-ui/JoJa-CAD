//! Füllflächen mit Löchern (Inseln).
//!
//! Eine Füllfläche besteht aus einem Außenring und beliebig vielen Löchern.
//! Bildschirm, PDF und PNG müssen dieselbe Fläche zeigen — deshalb liegen
//! Punktprobe, Musterbeschnitt und die Ringverbindung für Triangulierer hier
//! an einer Stelle und nicht in den einzelnen Renderern.
//!
//! Zwei Darstellungen sind zulässig und liefern dasselbe Bild:
//! * **Even-Odd** — alle Ringe als Unterpfade, abwechselnd gefüllt (Raster, PDF),
//! * **ein Ring** — Löcher über Stege an den Außenring angeschlossen und als ein
//!   Polygon trianguliert (egui kennt keine Löcher; siehe [`bridge_holes`]).

use crate::geom::*;
use crate::model::{Entity, EntityKind};

/// Liegt der Punkt in der gefüllten Fläche? Löcher zählen als außen.
pub fn point_in_area(p: Vec2, outer: &[Vec2], holes: &[Vec<Vec2>]) -> bool {
    point_in_polygon(p, outer) && !holes.iter().any(|h| point_in_polygon(p, h))
}

/// Fläche eines Objekts in mm² — `None`, wenn das Objekt nichts umschließt.
///
/// Eine Quelle für Anzeige (Eigenschaften-Panel) **und** Abfrage
/// (`Session::describe_entity`, Aktion `select.area`): sonst rechnen Panel und
/// MCP die Fläche getrennt und laufen auseinander. Offene Linienzüge, Texte,
/// Maße und Bögen umschließen nichts und liefern `None` — nicht 0.
pub fn entity_area(e: &Entity) -> Option<f64> {
    let mm2 = match &e.kind {
        EntityKind::Circle { r, .. } => std::f64::consts::PI * r * r,
        EntityKind::Ellipse { rx, ry, .. } => std::f64::consts::PI * rx * ry,
        EntityKind::Polyline { verts, closed } if *closed && verts.len() > 2 => {
            let pl = crate::geom::Polyline {
                verts: verts.clone(),
                closed: true,
            };
            pl.area().abs()
        }
        EntityKind::Spline { verts, closed } if *closed && verts.len() > 2 => {
            let sp = crate::geom::Spline {
                verts: verts.clone(),
                closed: true,
            };
            sp.area().abs()
        }
        EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. } => area_of(boundary, holes),
        // Wandkörper: Länge × Dicke (Öffnungen zählen nicht ab, die Wand ist
        // der Körper selbst).
        EntityKind::Wall {
            a, b, thickness, ..
        } => a.dist(*b) * thickness.abs(),
        _ => 0.0,
    };
    // 1 µm² — alles darunter ist entartet und keine Fläche.
    if mm2.is_finite() && mm2 > 1e-6 {
        Some(mm2)
    } else {
        None
    }
}

/// Fläche in mm² (Außenring minus Löcher).
pub fn area_of(outer: &[Vec2], holes: &[Vec<Vec2>]) -> f64 {
    let aussen = polygon_area(outer).abs();
    let loecher: f64 = holes.iter().map(|h| polygon_area(h).abs()).sum();
    (aussen - loecher).max(0.0)
}

/// Strecke auf die gefüllte Fläche beschneiden — der Teil, der im Loch liegt,
/// entfällt. Grundlage für Musterlinien und Dämmung: ein Muster darf nicht
/// durch eine Insel laufen.
pub fn clip_line_area(a: Vec2, b: Vec2, outer: &[Vec2], holes: &[Vec<Vec2>]) -> Vec<(Vec2, Vec2)> {
    let d = b - a;
    let len2 = d.len2();
    if len2 < 1e-18 {
        return vec![];
    }
    let mut ts: Vec<f64> = vec![0.0, 1.0];
    for ring in std::iter::once(outer).chain(holes.iter().map(|h| h.as_slice())) {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        for i in 0..n {
            let p1 = ring[i];
            let p2 = ring[(i + 1) % n];
            if let Some(x) = line_line(a, b, p1, p2, false, false) {
                let t = (x - a).dot(d) / len2;
                if (-1e-9..=1.0 + 1e-9).contains(&t) {
                    ts.push(t.clamp(0.0, 1.0));
                }
            }
        }
    }
    ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let mut out = Vec::new();
    for w in ts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if t1 - t0 < 1e-9 {
            continue;
        }
        let m = a + d * ((t0 + t1) * 0.5);
        if point_in_area(m, outer, holes) {
            out.push((a + d * t0, a + d * t1));
        }
    }
    out
}

/// Ringe so drehen, dass der Außenring mathematisch positiv (links) und jedes
/// Loch negativ läuft. Nur so addieren sich die Flächen beim Verbinden.
fn orientiere(outer: &[Vec2], holes: &[Vec<Vec2>]) -> (Vec<Vec2>, Vec<Vec<Vec2>>) {
    let mut aussen = outer.to_vec();
    if polygon_area(&aussen) < 0.0 {
        aussen.reverse();
    }
    let mut loecher: Vec<Vec<Vec2>> = Vec::with_capacity(holes.len());
    for h in holes {
        if h.len() < 3 {
            continue;
        }
        let mut r = h.clone();
        if polygon_area(&r) > 0.0 {
            r.reverse();
        }
        loecher.push(r);
    }
    // Von rechts nach links anschließen: das rechteste Loch zuerst, damit die
    // späteren Stege keinen schon verbundenen Ring durchqueren müssen.
    loecher.sort_by(|a, b| {
        max_x(b)
            .partial_cmp(&max_x(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (aussen, loecher)
}

fn max_x(ring: &[Vec2]) -> f64 {
    ring.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max)
}

/// Verbinden sich zwei Strecken im Inneren (nicht nur an den Enden)?
fn kreuzt(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let Some(x) = line_line(a, b, c, d, true, true) else {
        return false;
    };
    // Ein Treffer genau auf einem Endpunkt ist erlaubt (Steg endet dort).
    let eps = 1e-6;
    let innen_ab = x.dist(a) > eps && x.dist(b) > eps;
    let innen_cd = x.dist(c) > eps && x.dist(d) > eps;
    innen_ab && innen_cd
}

/// Ist der Steg von `a` nach `b` ganz in der Fläche sichtbar?
///
/// Nicht sichtbar, wenn er eine Kante des Außenrings oder eines Lochs im
/// Inneren kreuzt oder mit seiner Mitte außerhalb der Fläche liegt.
fn sichtbar(a: Vec2, b: Vec2, outer: &[Vec2], holes: &[Vec<Vec2>]) -> bool {
    for ring in std::iter::once(outer).chain(holes.iter().map(|h| h.as_slice())) {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        for i in 0..n {
            if kreuzt(a, b, ring[i], ring[(i + 1) % n]) {
                return false;
            }
        }
    }
    point_in_area(a + (b - a) * 0.5, outer, holes)
}

/// Außenring und Löcher zu **einem** Ring verbinden (Stege doppelt durchlaufen).
///
/// egui füllt nur konvexe Polygone und kennt keine Aussparungen; ein
/// Triangulierer ohne Lochunterstützung bekommt so trotzdem die richtige
/// Fläche. Die Stege sind null breit und verschwinden beim Triangulieren
/// (entartete Dreiecke fallen heraus).
pub fn bridge_holes(outer: &[Vec2], holes: &[Vec<Vec2>]) -> Vec<Vec2> {
    let (aussen, loecher) = orientiere(outer, holes);
    let mut ring = aussen.clone();
    for loch in &loecher {
        // Rechtestes Eck des Lochs — von dort zeigt der kürzeste Weg nach außen.
        let (hi, hr) = loch
            .iter()
            .enumerate()
            .max_by(|a, b| {
                a.1.x
                    .partial_cmp(&b.1.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(
                        a.1.y
                            .partial_cmp(&b.1.y)
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
            })
            .map(|(i, p)| (i, *p))
            .unwrap_or((0, Vec2::ZERO));

        // Zielpunkt am bisherigen Ring: der nächste sichtbare Punkt rechts vom
        // Loch. Nur Punkte rechts von `hr` kommen in Frage — ein Steg nach links
        // müsste die eigene Insel umlaufen.
        let mut kandidaten: Vec<(usize, f64)> = ring
            .iter()
            .enumerate()
            .filter(|(_, p)| p.x >= hr.x - 1e-9)
            .map(|(i, p)| (i, p.dist(hr)))
            .collect();
        kandidaten.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut ziel: Option<usize> = None;
        // Genug Kandidaten für jede realistische Insel; die Schranke hält die
        // Sichtprüfung bei sehr feinen Konturen (bis 250 Ecken) bezahlbar.
        for (i, _) in kandidaten.iter().take(60) {
            if sichtbar(hr, ring[*i], &aussen, holes) {
                ziel = Some(*i);
                break;
            }
        }
        let ri = ziel.unwrap_or_else(|| {
            // Kein sichtbarer Punkt gefunden (entartete Geometrie): den nächsten
            // nehmen, statt das Loch zu verlieren.
            kandidaten.first().map(|(i, _)| *i).unwrap_or(0)
        });

        // Steg: Ring bis zum Zielpunkt, Loch ab dem rechtesten Eck einmal
        // herum, zurück über das Eck zum Zielpunkt, dann der Rest des Rings.
        let mut neu = Vec::with_capacity(ring.len() + loch.len() + 2);
        neu.extend_from_slice(&ring[..=ri]);
        for k in 0..loch.len() {
            neu.push(loch[(hi + k) % loch.len()]);
        }
        neu.push(hr);
        neu.push(ring[ri]);
        neu.extend_from_slice(&ring[ri + 1..]);
        ring = neu;
    }
    ring
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    fn quad(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)]
    }

    fn aussen() -> Vec<Vec2> {
        quad(0.0, 0.0, 4000.0, 3000.0)
    }

    #[test]
    fn punktprobe_kennt_das_loch() {
        let loch = quad(1000.0, 1000.0, 2000.0, 2000.0);
        assert!(point_in_area(v(500.0, 500.0), &aussen(), &[loch.clone()]));
        assert!(!point_in_area(v(1500.0, 1500.0), &aussen(), &[loch.clone()]));
        assert!(!point_in_area(v(5000.0, 500.0), &aussen(), &[loch]));
    }

    #[test]
    fn flaeche_zieht_die_loecher_ab() {
        let loch = quad(1000.0, 1000.0, 2000.0, 2000.0);
        let a = area_of(&aussen(), &[loch]);
        assert!((a - (12.0e6 - 1.0e6)).abs() < 1e-6, "Fläche {a}");
    }

    /// Eine Musterlinie durch das Loch wird in zwei Stücke geteilt.
    #[test]
    fn musterlinie_ueberspringt_das_loch() {
        let loch = quad(1000.0, 1000.0, 2000.0, 2000.0);
        let stuecke = clip_line_area(v(-100.0, 1500.0), v(4100.0, 1500.0), &aussen(), &[loch]);
        assert_eq!(stuecke.len(), 2, "zwei Stücke: {stuecke:?}");
        // Lücke genau über dem Loch: 1000 → 2000
        assert!((stuecke[0].1.x - 1000.0).abs() < 1e-6, "{:?}", stuecke[0]);
        assert!((stuecke[1].0.x - 2000.0).abs() < 1e-6, "{:?}", stuecke[1]);
    }

    /// Eine Strecke, die nur im Loch liegt, entfällt ganz.
    #[test]
    fn musterlinie_im_loch_entfaellt() {
        let loch = quad(1000.0, 1000.0, 2000.0, 2000.0);
        assert!(clip_line_area(v(1200.0, 1100.0), v(1800.0, 1900.0), &aussen(), &[loch]).is_empty());
    }

    /// Senkrecht durch das ganze Feld: zwei Stücke bleiben, keines im Loch.
    #[test]
    fn senkrechte_musterlinie_teilt_sich_am_loch() {
        let loch = quad(1000.0, 1000.0, 2000.0, 2000.0);
        let stuecke = clip_line_area(v(1200.0, -100.0), v(1200.0, 3100.0), &aussen(), &[loch]);
        assert_eq!(stuecke.len(), 2, "{stuecke:?}");
        for (a, b) in &stuecke {
            let m = *a + (*b - *a) * 0.5;
            assert!(m.y < 1000.0 || m.y > 2000.0, "Stück {m:?} liegt im Loch");
        }
        assert!((stuecke[0].1.y - 1000.0).abs() < 1e-6, "{:?}", stuecke[0]);
        assert!((stuecke[1].0.y - 2000.0).abs() < 1e-6, "{:?}", stuecke[1]);
    }

    /// Nach dem Verbinden füllt der eine Ring genau die Fläche mit Loch:
    /// Punktprobe stimmt an einem Raster von Proben mit der Fläche überein.
    #[test]
    fn verbundener_ring_hat_dieselbe_flaeche() {
        let loch = quad(1000.0, 1000.0, 2000.0, 2000.0);
        let mut ring = bridge_holes(&aussen(), &[loch.clone()]);
        // Ecken des Stegs sind doppelt vorhanden — für die Punktprobe egal.
        assert!(ring.len() > aussen().len());
        let mut abweichungen = 0;
        for i in 1..40 {
            for j in 1..30 {
                let p = v(i as f64 * 100.0 + 13.0, j as f64 * 100.0 + 7.0);
                let soll = point_in_area(p, &aussen(), &[loch.clone()]);
                let ist = point_in_polygon(p, &ring);
                if soll != ist {
                    abweichungen += 1;
                }
            }
        }
        assert_eq!(abweichungen, 0, "Ring weicht von der Fläche ab");
        let diff = (polygon_area(&ring).abs() - area_of(&aussen(), &[loch])).abs();
        assert!(diff < 1e-3, "Ringfläche weicht um {diff} mm² ab");
    }

    /// Auch zwei Löcher und ein Loch mit vielen Ecken (Insel mit 12 Ecken)
    /// füllen richtig; die Ringe müssen nicht vorsortiert sein.
    #[test]
    fn zwei_loecher_und_viele_ecken() {
        let a = quad(1000.0, 300.0, 1500.0, 800.0);
        let b = quad(2500.0, 2000.0, 3200.0, 2600.0);
        let mut ring = bridge_holes(&aussen(), &[b.clone(), a.clone()]);
        ring.dedup_by(|x, y| x.dist(*y) < 1e-9);
        for i in 1..40 {
            for j in 1..30 {
                let p = v(i as f64 * 100.0 + 11.0, j as f64 * 100.0 + 3.0);
                let soll = point_in_area(p, &aussen(), &[a.clone(), b.clone()]);
                assert_eq!(soll, point_in_polygon(p, &ring), "bei {p:?}");
            }
        }
    }

    /// Umgekehrter Umlaufsinn darf nichts ändern: die Orientierung wird
    /// normalisiert, sonst zieht der Steg die Fläche ab statt sie zu füllen.
    #[test]
    fn umlaufsinn_ist_egal() {
        let loch = quad(1000.0, 1000.0, 2000.0, 2000.0);
        let mut aussen_gedreht = aussen();
        aussen_gedreht.reverse();
        let mut loch_gedreht = loch.clone();
        loch_gedreht.reverse();
        let ring = bridge_holes(&aussen_gedreht, &[loch_gedreht]);
        let p = v(1700.0, 1700.0); // im Loch
        assert!(!point_in_polygon(p, &ring));
        assert!(point_in_polygon(v(500.0, 500.0), &ring));
        let diff =
            (polygon_area(&ring).abs() - area_of(&aussen(), &[loch])).abs();
        assert!(diff < 1e-3, "Ringfläche weicht um {diff} mm² ab");
    }
}
