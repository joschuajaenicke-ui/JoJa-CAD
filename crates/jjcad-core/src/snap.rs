//! Fang-Engine: Endpunkt, Mittelpunkt, Zentrum, Schnittpunkt, Lot, Nächster Punkt, Raster.

use crate::geom::*;
use crate::model::*;
use crate::render;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Endpoint,
    Intersection,
    Midpoint,
    Center,
    Quadrant,
    Perpendicular,
    Nearest,
    Grid,
}

impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "Endpunkt",
            SnapKind::Intersection => "Schnittpunkt",
            SnapKind::Midpoint => "Mittelpunkt",
            SnapKind::Center => "Zentrum",
            SnapKind::Quadrant => "Quadrant",
            SnapKind::Perpendicular => "Lot",
            SnapKind::Nearest => "Nächster Punkt",
            SnapKind::Grid => "Raster",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "END",
            SnapKind::Intersection => "SCHN",
            SnapKind::Midpoint => "MIT",
            SnapKind::Center => "ZEN",
            SnapKind::Quadrant => "QUA",
            SnapKind::Perpendicular => "LOT",
            SnapKind::Nearest => "NAE",
            SnapKind::Grid => "RAS",
        }
    }

    fn priority(self) -> u8 {
        match self {
            SnapKind::Endpoint => 0,
            SnapKind::Intersection => 1,
            SnapKind::Midpoint => 2,
            SnapKind::Center => 3,
            SnapKind::Quadrant => 4,
            SnapKind::Perpendicular => 5,
            SnapKind::Nearest => 6,
            SnapKind::Grid => 7,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SnapHit {
    pub point: Vec2,
    pub kind: SnapKind,
    pub entity: Option<Id>,
}

/// Sucht den besten Fangpunkt in der Nähe des Cursors.
///
/// * `tol` – Fangradius in Weltkoordinaten
/// * `reference` – Bezugspunkt (für Lotfuß / Ortho)
pub fn find_snap(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
) -> Option<SnapHit> {
    find_snap_gated(doc, cursor, tol, reference, ignore, true)
}

/// Wie [`find_snap`], aber der Rasterfang lässt sich abschalten —
/// er soll nur greifen, wenn das Raster auch angezeigt wird.
pub fn find_snap_gated(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    allow_grid: bool,
) -> Option<SnapHit> {
    let mut s = doc.settings.snap.clone();
    if !allow_grid {
        s.grid = false;
    }
    let s = &s;
    let tol = tol.max(1e-6);
    let mut best: Option<SnapHit> = None;
    let mut best_key = (u8::MAX, f64::INFINITY);

    let mut consider = |p: Vec2, kind: SnapKind, entity: Option<Id>, dist: f64| {
        if dist > tol {
            return;
        }
        let key = (kind.priority(), dist);
        if key.0 < best_key.0 || (key.0 == best_key.0 && key.1 < best_key.1) {
            best_key = key;
            best = Some(SnapHit {
                point: p,
                kind,
                entity,
            });
        }
    };

    // Kandidaten sammeln (nur sichtbare Objekte in der Nähe)
    let area = BBox::new(cursor - v(tol, tol), cursor + v(tol, tol));
    let mut segs: Vec<(Seg, Id)> = Vec::new();
    for e in &doc.entities {
        if ignore.contains(&e.id) {
            continue;
        }
        if let Some(l) = doc.layer(e.layer) {
            if !l.visible || l.locked {
                continue;
            }
        }
        let b = render::entity_bbox(doc, e);
        if !b.is_valid() || !b.expanded(tol * 2.0).intersects(&area) {
            continue;
        }
        for sg in render::entity_segments(doc, e) {
            segs.push((sg, e.id));
        }
        if s.center {
            for c in render::entity_centers(doc, e) {
                let d = c.dist(cursor);
                if d <= tol {
                    consider(c, SnapKind::Center, Some(e.id), d);
                }
            }
        }
    }

    // Endpunkte / Mittelpunkte / Quadranten
    for (sg, id) in &segs {
        let a = sg.start();
        let b = sg.end();
        let da = a.dist(cursor);
        let db = b.dist(cursor);
        if s.endpoint {
            consider(a, SnapKind::Endpoint, Some(*id), da);
            consider(b, SnapKind::Endpoint, Some(*id), db);
        }
        if s.midpoint {
            let m = a.lerp(b, 0.5);
            consider(m, SnapKind::Midpoint, Some(*id), m.dist(cursor));
        }
        if let Seg::Arc(arc) = sg {
            if s.center {
                consider(arc.c, SnapKind::Center, Some(*id), arc.c.dist(cursor));
            }
            if s.endpoint {
                for k in 0..4 {
                    let ang = k as f64 * std::f64::consts::FRAC_PI_2;
                    let q = arc.c + Vec2::polar(ang, arc.r);
                    consider(q, SnapKind::Quadrant, Some(*id), q.dist(cursor));
                }
            }
        }
        if s.perpendicular {
            if let Some(r) = reference {
                let foot = closest_point_on_segment(r, a, b);
                // Lot nur, wenn der Fuß nahe am Cursor liegt
                consider(foot, SnapKind::Perpendicular, Some(*id), foot.dist(cursor));
            }
        }
        if s.nearest {
            let (q, d) = sg.closest(cursor);
            consider(q, SnapKind::Nearest, Some(*id), d);
        }
    }

    // Schnittpunkte
    if s.intersection {
        let n = segs.len().min(400);
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, ia) = &segs[i];
                let (b, ib) = &segs[j];
                // Nur wenn beide Segmente nahe am Cursor liegen
                let ba = a.bbox();
                let bb = b.bbox();
                if !ba.expanded(tol).intersects(&area) || !bb.expanded(tol).intersects(&area) {
                    continue;
                }
                for p in seg_seg(a, b) {
                    // Schnittpunkt muss zum Objekt gehören (nicht nur verlängert)
                    consider(p, SnapKind::Intersection, Some(*ia), p.dist(cursor));
                    let _ = ib;
                }
            }
        }
    }

    // Raster
    if s.grid {
        let g = doc.settings.snap.grid_step;
        if g > 0.0 {
            let gp = cursor.snap_grid(g, Vec2::ZERO);
            consider(gp, SnapKind::Grid, None, gp.dist(cursor));
        }
    }

    let _ = best_key;
    best
}

/// Trefferprüfung (Auswahl per Klick).
pub fn hit_test(doc: &Document, p: Vec2, tol: f64, layer_filter: Option<&[Id]>) -> Option<Id> {
    let mut best: Option<(Id, f64, u8)> = None;
    for e in doc.entities.iter().rev() {
        if let Some(l) = doc.layer(e.layer) {
            if !l.visible || l.locked {
                continue;
            }
            if let Some(f) = layer_filter {
                if !f.contains(&e.layer) {
                    continue;
                }
            }
        }
        let b = render::entity_bbox(doc, e);
        if !b.is_valid() {
            continue;
        }
        // Text/Punkt: über BBox
        let is_text = matches!(
            e.kind,
            EntityKind::Text { .. } | EntityKind::Point { .. } | EntityKind::Elevation { .. }
        );
        if is_text {
            if b.expanded(tol).contains(p) {
                let d = b.center().dist(p);
                let prio = 2;
                if best.map(|(_, bd, bp)| prio < bp || (prio == bp && d < bd)).unwrap_or(true) {
                    best = Some((e.id, d, prio));
                }
            }
            continue;
        }
        // Wand/Schraffur: auch Innenfläche
        let filled = matches!(
            e.kind,
            EntityKind::Wall { .. } | EntityKind::Hatch { .. } | EntityKind::Stair { .. }
        );
        if filled {
            if let EntityKind::Wall { a, b: bb, thickness, .. } = &e.kind {
                if dist_point_segment(p, *a, *bb) <= thickness * 0.5 + tol {
                    let d = dist_point_segment(p, *a, *bb);
                    if best.map(|(_, bd, _)| d < bd).unwrap_or(true) {
                        best = Some((e.id, d, 0));
                    }
                    continue;
                }
            }
        }
        let mut dmin = f64::INFINITY;
        for sg in render::entity_segments(doc, e) {
            let (_, d) = sg.closest(p);
            dmin = dmin.min(d);
        }
        if dmin <= tol {
            let prio = 1;
            if best
                .map(|(_, bd, bp)| prio < bp || (dmin < bd && prio <= bp))
                .unwrap_or(true)
            {
                best = Some((e.id, dmin, prio));
            }
        }
    }
    best.map(|(id, _, _)| id)
}

/// Punkte, die ein Segment beschreiben (Bögen werden abgeflacht).
fn seg_points(sg: &Seg) -> Vec<Vec2> {
    match sg {
        Seg::Line(a, b) => vec![*a, *b],
        Seg::Arc(arc) => arc.flatten(1.0),
    }
}

/// Berührt die Strecke das Rechteck? (Endpunkt innen oder echte Kreuzung)
fn seg_meets_rect(a: Vec2, b: Vec2, r: &BBox) -> bool {
    if r.contains(a) || r.contains(b) {
        return true;
    }
    let ecken = [
        (r.min, Vec2::new(r.max.x, r.min.y)),
        (Vec2::new(r.max.x, r.min.y), r.max),
        (r.max, Vec2::new(r.min.x, r.max.y)),
        (Vec2::new(r.min.x, r.max.y), r.min),
    ];
    ecken
        .iter()
        .any(|(p, q)| line_line(a, b, *p, *q, false, false).is_some())
}

/// Objekte in einem Fenster (links→rechts) oder Querfenster (rechts→links).
/// `crossing` = berührt (sonst: vollständig enthalten).
/// Geprüft wird die echte Geometrie, nicht die achsenparallele Bounding-Box —
/// sonst würde eine Diagonale im Eckbereich ihres Rechtecks mitgewählt.
pub fn select_window(doc: &Document, a: Vec2, b: Vec2, crossing: bool) -> Vec<Id> {
    let rect = BBox::from_corners(a, b);
    let mut out = Vec::new();
    for e in &doc.entities {
        if let Some(l) = doc.layer(e.layer) {
            if !l.visible || l.locked {
                continue;
            }
        }
        let bb = render::entity_bbox(doc, e);
        if !bb.is_valid() {
            continue;
        }
        // Flächige Elemente: Fläche zählt, BBox ist hier korrekt
        let flaechig = matches!(
            e.kind,
            EntityKind::Hatch { .. }
                | EntityKind::Stair { .. }
                | EntityKind::Text { .. }
                | EntityKind::Point { .. }
                | EntityKind::Elevation { .. }
        );
        if flaechig {
            if (crossing && bb.intersects(&rect)) || (!crossing && rect.contains_bbox(&bb)) {
                out.push(e.id);
            }
            continue;
        }
        // Wand: Körper zählt als Fläche
        if let EntityKind::Wall {
            a: wa,
            b: wb,
            thickness,
            ..
        } = &e.kind
        {
            let halb = thickness * 0.5;
            let mitte_innen = dist_point_segment(rect.center(), *wa, *wb) <= halb;
            let im_koerper = |p: Vec2| dist_point_segment(p, *wa, *wb) <= halb;
            let ecken_innen = [rect.min, Vec2::new(rect.max.x, rect.min.y), rect.max, Vec2::new(rect.min.x, rect.max.y)]
                .iter()
                .all(|p| im_koerper(*p));
            if crossing {
                if mitte_innen || ecken_innen || {
                    let pts: Vec<Vec2> = render::entity_segments(doc, e)
                        .iter()
                        .flat_map(seg_points)
                        .collect();
                    pts.windows(2).any(|w| seg_meets_rect(w[0], w[1], &rect))
                } {
                    out.push(e.id);
                }
            } else if ecken_innen {
                out.push(e.id);
            }
            continue;
        }
        let pts: Vec<Vec2> = render::entity_segments(doc, e)
            .iter()
            .flat_map(seg_points)
            .collect();
        if pts.is_empty() {
            if rect.contains(bb.center()) {
                out.push(e.id);
            }
            continue;
        }
        if crossing {
            if pts.windows(2).any(|w| seg_meets_rect(w[0], w[1], &rect)) {
                out.push(e.id);
            }
        } else if pts.iter().all(|p| rect.contains(*p)) {
            out.push(e.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Document;

    #[test]
    fn polarfang_rastet_nur_in_der_naehe() {
        let base = Vec2::ZERO;
        // 40° liegt nahe an 45° -> rastet ein
        let nah = Vec2::polar(40f64.to_radians(), 1000.0);
        let hit = nah.polar_snap_from_tol(base, 45.0, 6.0).expect("muss rasten");
        assert!((hit.angle().to_degrees() - 45.0).abs() < 1e-6);
        // 25° ist 20° entfernt -> kein Rasten
        let fern = Vec2::polar(25f64.to_radians(), 1000.0);
        assert!(fern.polar_snap_from_tol(base, 45.0, 6.0).is_none());
    }

    #[test]
    fn polarwinkel_standard_ist_45_grad() {
        let doc = Document::blank();
        assert!((doc.settings.snap.polar_step - 45.0).abs() < 1e-9);
    }

    #[test]
    fn rasterfang_folgt_der_rasteranzeige() {
        let doc = Document::blank();
        let p = Vec2::new(104.0, 97.0);
        // Raster eingeschaltet -> fängt auf 100/100
        let on = find_snap_gated(&doc, p, 20.0, None, &[], true).expect("Rasterfang");
        assert!((on.point.x - 100.0).abs() < 1e-9);
        // Raster ausgeschaltet -> kein Rasterfang mehr
        assert!(find_snap_gated(&doc, p, 20.0, None, &[], false).is_none());
    }
}
