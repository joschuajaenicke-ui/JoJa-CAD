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
/// * `ignore` – Objekte, die überhaupt nicht fangen (z. B. das gerade live
///   gezogene Objekt)
/// * `selected` – ausgewählte Objekte: sie bleiben mit ihren *diskreten*
///   Punkten (Ende, Mitte, Zentrum, Quadrant) fangbar, liefern aber keine
///   kontinuierlichen Fänge (Nächster Punkt, Lot, Schnittpunkt). Sonst klebte
///   ein bewegtes Objekt an sich selbst.
pub fn find_snap(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    selected: &[Id],
) -> Option<SnapHit> {
    find_snap_gated(doc, cursor, tol, reference, ignore, selected, true)
}

/// Wie [`find_snap`], aber der Rasterfang lässt sich abschalten —
/// er soll nur greifen, wenn das Raster auch angezeigt wird.
pub fn find_snap_gated(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    selected: &[Id],
    allow_grid: bool,
) -> Option<SnapHit> {
    snap_candidates_gated(doc, cursor, tol, reference, ignore, selected, allow_grid)
        .into_iter()
        .next()
}

/// Toleranz, unterhalb derer zwei Fangpunkte als derselbe Punkt gelten (mm).
/// Nötig, weil eine Gehrungsecke von beiden beteiligten Wänden geliefert wird
/// und die Wandachse eigene Endpunkte beisteuert.
const SNAP_DEDUP: f64 = 0.1;

/// Alle Fang-Kandidaten in Reichweite, entdoppelt und nach Priorität/Abstand
/// sortiert (bester zuerst). Für Anzeige und Prüfung mehrerer Vorschläge;
/// [`find_snap`] nimmt nur den ersten Treffer.
pub fn snap_candidates(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    selected: &[Id],
) -> Vec<SnapHit> {
    snap_candidates_gated(doc, cursor, tol, reference, ignore, selected, true)
}

#[allow(clippy::too_many_arguments)]
pub fn snap_candidates_gated(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    selected: &[Id],
    allow_grid: bool,
) -> Vec<SnapHit> {
    snap_candidates_ohne_punkte(doc, cursor, tol, reference, ignore, selected, &[], allow_grid)
}

/// Wie [`snap_candidates_gated`], aber gesperrte Punkte fallen schon beim
/// Sammeln heraus — **vor** dem Entdoppeln. Nachträglich zu filtern wäre falsch:
/// sitzt ein gesperrter Punkt genau auf dem Endpunkt eines fremden Elements,
/// hätte das Entdoppeln den fremden Kandidaten bereits verworfen und der Fang
/// fiele auf einen kontinuierlichen Treffer zurück.
#[allow(clippy::too_many_arguments)]
pub fn snap_candidates_ohne_punkte(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    selected: &[Id],
    gesperrt: &[GesperrterPunkt],
    allow_grid: bool,
) -> Vec<SnapHit> {
    let mut s = doc.settings.snap.clone();
    if !allow_grid {
        s.grid = false;
    }
    let s = &s;
    let tol = tol.max(1e-6);
    let mut cands: Vec<(f64, SnapHit)> = Vec::new();

    let mut consider = |p: Vec2, kind: SnapKind, entity: Option<Id>, dist: f64| {
        if dist > tol {
            return;
        }
        let hit = SnapHit {
            point: p,
            kind,
            entity,
        };
        if ist_gesperrt(&hit, gesperrt) {
            return;
        }
        cands.push((dist, hit));
    };

    // Kandidaten sammeln (nur sichtbare Objekte in der Nähe).
    // Dritter Wert je Segment: gehört das Segment zu einem ausgewählten
    // Objekt? Dann nur diskrete Punkte anbieten, keine kontinuierlichen Fänge.
    let area = BBox::new(cursor - v(tol, tol), cursor + v(tol, tol));
    let mut segs: Vec<(Seg, Id, bool)> = Vec::new();
    for e in &doc.entities {
        if ignore.contains(&e.id) {
            continue;
        }
        let ausgewaehlt = selected.contains(&e.id);
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
            segs.push((sg, e.id, ausgewaehlt));
        }
        // Wandkörper: die umgrenzenden Linien (Längskanten, Gehrungs-/Stirn-
        // ecken, Laibungen) liefern ebenfalls Fangpunkte. Ohne diesen Zweig
        // kennt der Fang nur die Wandachse. Getrennt schaltbar über
        // `snap.wall_edges` — die Achse bleibt in jedem Fall erhalten.
        if s.wall_edges && matches!(e.kind, EntityKind::Wall { .. }) {
            for sg in render::wall_contour_segments(doc, e) {
                segs.push((sg, e.id, ausgewaehlt));
            }
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
    for (sg, id, ausgewaehlt) in &segs {
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
        // Kontinuierliche Fänge nur an nicht ausgewählten Objekten: an einem
        // ausgewählten Objekt würde „Nächster Punkt" den Zeiger auf das
        // Objekt selbst zurückziehen, sobald es mitgezogen wird.
        if s.perpendicular && !*ausgewaehlt {
            if let Some(r) = reference {
                let foot = closest_point_on_segment(r, a, b);
                // Lot nur, wenn der Fuß nahe am Cursor liegt
                consider(foot, SnapKind::Perpendicular, Some(*id), foot.dist(cursor));
            }
        }
        if s.nearest && !*ausgewaehlt {
            let (q, d) = sg.closest(cursor);
            consider(q, SnapKind::Nearest, Some(*id), d);
        }
    }

    // Schnittpunkte — nicht mit ausgewählten Objekten: der Schnittpunkt
    // wandert mit, sobald das Objekt bewegt wird (gleiche Begründung wie oben).
    if s.intersection {
        let n = segs.len().min(400);
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, ia, sa) = &segs[i];
                let (b, ib, sb) = &segs[j];
                if *sa || *sb {
                    continue;
                }
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

    // Beste zuerst: Priorität der Fangart, dann Abstand zum Zeiger.
    cands.sort_by(|a, b| {
        a.1.kind
            .priority()
            .cmp(&b.1.kind.priority())
            .then(a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Entdoppeln: dieselbe Koordinate (innerhalb SNAP_DEDUP) wird nur einmal
    // angeboten — der beste Kandidat an dieser Stelle gewinnt.
    let mut out: Vec<SnapHit> = Vec::with_capacity(cands.len());
    for (_, hit) in cands {
        if out.iter().any(|h| h.point.dist(hit.point) <= SNAP_DEDUP) {
            continue;
        }
        out.push(hit);
    }
    out
}

/// Ein gesperrter Fangpunkt: gehört zu diesem Element *und* liegt auf dieser
/// Stelle. Nur die Kombination ist gesperrt — ein fremdes Element, das zufällig
/// auf demselben Punkt sitzt, darf weiterhin fangen (sonst bricht der Fang ab,
/// sobald der gezogene Griff auf dem Ziel angekommen ist).
pub type GesperrterPunkt = (Id, Vec2);

/// Ist dieser Kandidat gesperrt? Verglichen wird mit derselben Toleranz, unter
/// der zwei Fangpunkte als derselbe Punkt gelten.
fn ist_gesperrt(hit: &SnapHit, gesperrt: &[GesperrterPunkt]) -> bool {
    gesperrt
        .iter()
        .any(|(id, g)| hit.entity == Some(*id) && g.dist(hit.point) <= SNAP_DEDUP)
}

/// Woher der endgültige Zeigerpunkt stammt — die Stufen der Fang-Kette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapSource {
    /// Objektfang der Fang-Engine (Endpunkt, Schnittpunkt, …, Raster).
    Object(SnapKind),
    /// Polarfang: auf ein Vielfaches der Polarschrittweite gedreht.
    Polar,
    /// Ortho-Zwang: waagerecht/senkrecht zum Bezugspunkt.
    Ortho,
    /// Kein Fang — der Zeigerpunkt gilt unverändert.
    Free,
}

impl SnapSource {
    /// Label für die Statuszeile; Ortho und „frei" bleiben ohne Label, weil
    /// sie den Zeiger nur führen und keinen Sonderpunkt liefern.
    pub fn label(self) -> Option<&'static str> {
        match self {
            SnapSource::Object(k) => Some(k.label()),
            SnapSource::Polar => Some("POL"),
            SnapSource::Ortho | SnapSource::Free => None,
        }
    }
}

/// Ergebnis der Fang-Kette für einen Zeigerpunkt.
#[derive(Clone, Debug)]
pub struct SnapResolution {
    pub point: Vec2,
    pub source: SnapSource,
}

impl SnapResolution {
    /// Der übernommene Punkt ist ein echter Fangpunkt (nicht nur der Zeiger).
    pub fn is_snap(&self) -> bool {
        matches!(self.source, SnapSource::Object(_) | SnapSource::Polar)
    }

    pub fn label(&self) -> Option<&'static str> {
        self.source.label()
    }
}

/// Fang-Kette in der richtigen Reihenfolge: **Objektfang vor Ortho/Polar**.
///
/// Aus Schaden gelernt (Karte t_809d529d): wird der Zeiger zuerst ortho
/// projiziert und der Objektfang erst danach um den projizierten Punkt
/// gesucht, liegt ein vorhandenes Linienende je nach Winkel weit neben der
/// Zwangsachse und ist damit unerreichbar — die Polylinie kann nicht an
/// bestehende Linien anschließen. Deshalb:
///
/// 1. Objektfang im Fangradius um den **rohen** Zeiger (Endpunkt hat Vorrang
///    vor Kante, siehe [`SnapKind::priority`]).
/// 2. Nur wenn nichts fängt: Ortho bzw. Polar als Führung.
/// 3. Zuletzt das Raster — es darf eine Zwangsrichtung nicht über den
///    Fangradius hinaus verbiegen.
///
/// Der zurückgegebene Punkt ist der Fangpunkt selbst (keine Rundung); die
/// Zeichenfläche übernimmt ihn unverändert, damit gespeicherter Punkt und
/// Fangpunkt identisch sind. `tol` ist der Fangradius in Weltkoordinaten
/// (Bildschirm-Pixel ÷ Zoom), damit das Verhalten zoomunabhängig bleibt.
pub fn snap_constrained(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    selected: &[Id],
    allow_grid: bool,
) -> SnapResolution {
    snap_constrained_ohne_punkte(doc, cursor, tol, reference, ignore, selected, &[], allow_grid)
}

/// Wie [`snap_constrained`], aber die Punkte in `gesperrt` sind als Fangziel
/// ausgeschlossen.
///
/// Gebraucht beim Ziehen eines Griffs: der bewegte Stützpunkt liegt dauerhaft
/// im Fangradius des Zeigers. Ohne Sperre fängt er sich selbst (Endpunkt hat
/// die höchste Priorität) und der Griff bleibt stehen, statt der Maus zu
/// folgen. Gesperrt wird nur der *eine* Punkt an seinem Element — die ruhenden
/// Nachbarecken derselben Polylinie fangen weiter (Polylinie an sich selbst
/// anschließen), und ein fremdes Element auf derselben Stelle ebenfalls.
#[allow(clippy::too_many_arguments)]
pub fn snap_constrained_ohne_punkte(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    selected: &[Id],
    gesperrt: &[GesperrterPunkt],
    allow_grid: bool,
) -> SnapResolution {
    let s = doc.settings.snap.clone();
    let tol = tol.max(1e-6);

    // 1. Objektfang ohne Raster: er gewinnt gegen jede Zwangsrichtung.
    //    Das Raster ist eine Führung wie Ortho und kommt erst in Schritt 3.
    if let Some(hit) =
        snap_candidates_ohne_punkte(doc, cursor, tol, reference, ignore, selected, gesperrt, false)
            .into_iter()
            .next()
    {
        return SnapResolution {
            point: hit.point,
            source: SnapSource::Object(hit.kind),
        };
    }

    // 2. Kein Objekt in Reichweite: Ortho bzw. Polar.
    let mut point = cursor;
    let mut source = SnapSource::Free;
    if let Some(r) = reference {
        if s.ortho {
            point = cursor.ortho_from(r, false);
            source = SnapSource::Ortho;
        } else if s.polar && cursor.dist(r) > tol * 2.0 {
            if let Some(p) = cursor.polar_snap_from_tol(r, s.polar_step.max(1.0), s.polar_tolerance)
            {
                point = p;
                source = SnapSource::Polar;
            }
        }
    }

    // 3. Raster zuletzt und nur im Fangradius.
    if allow_grid && s.grid {
        let step = s.grid_step;
        if step > 0.0 {
            let gp = point.snap_grid(step, Vec2::ZERO);
            if gp.dist(point) <= tol {
                return SnapResolution {
                    point: gp,
                    source: SnapSource::Object(SnapKind::Grid),
                };
            }
        }
    }
    SnapResolution { point, source }
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
            EntityKind::Wall { .. } | EntityKind::Fill { .. } | EntityKind::Stair { .. }
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
            EntityKind::Fill { .. }
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

    /// Eine Wand über die Befehlsschicht anlegen (Achse 0,0 → 5000,0).
    fn wand(doc: &mut Document, a: Vec2, b: Vec2) -> Id {
        let e = Entity {
            id: doc.new_id(),
            layer: 0,
            kind: EntityKind::Wall {
                a,
                b,
                thickness: 300.0,
                height: 2500.0,
                material: String::new(),
                hatch: None,
                align: crate::style::WallAlign::Center,
            },
            style: Default::default(),
            by_block: false,
        };
        let id = e.id;
        doc.entities.push(e);
        id
    }

    #[test]
    fn wandkontur_liefert_eckpunkte() {
        let mut doc = Document::blank();
        doc.settings.snap.wall_edges = true;
        let id = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        // Mitte der Wandachse liegt bei (2500,0); die Konturecken bei 0/5000 ± 150.
        let pts = render::wall_contour_points(&doc, doc.entity(id).unwrap());
        assert!(
            pts.iter().any(|p| p.dist(Vec2::new(0.0, 150.0)) < 1e-6),
            "{pts:?}"
        );
        assert!(
            pts.iter().any(|p| p.dist(Vec2::new(5000.0, -150.0)) < 1e-6),
            "{pts:?}"
        );
        // Der Fang greift auf der Konturecke, nicht auf der Achse.
        let hit = find_snap_gated(&doc, Vec2::new(4.0, 146.0), 30.0, None, &[], &[], false)
            .expect("Konturecke muss fangbar sein");
        assert!(hit.point.dist(Vec2::new(0.0, 150.0)) < 1e-6, "{hit:?}");
        assert_eq!(hit.kind, SnapKind::Endpoint);
        assert_eq!(hit.entity, Some(id));
    }

    #[test]
    fn oeffnung_laibungsecken_sind_fangbar() {
        let mut doc = Document::blank();
        let id = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        // Tür 1000 mm breit, Mitte bei 2000 mm auf der Wand
        let door = Entity {
            id: doc.new_id(),
            layer: 0,
            kind: EntityKind::Door {
                center: Vec2::new(2000.0, 0.0),
                angle: 0.0,
                width: 1000.0,
                kind: DoorKind::Opening,
                hinge: 1.0,
                swing_in: true,
                align: crate::style::WallAlign::Center,
                host: Some(id),
                at: Some(2000.0),
            },
            style: Default::default(),
            by_block: false,
        };
        doc.entities.push(door);
        let pts = render::wall_contour_points(&doc, doc.entity(id).unwrap());
        // Laibungsecken bei 1500/2500 mm, je ±150 mm neben der Achse
        for soll in [
            Vec2::new(1500.0, 150.0),
            Vec2::new(1500.0, -150.0),
            Vec2::new(2500.0, 150.0),
            Vec2::new(2500.0, -150.0),
        ] {
            assert!(pts.iter().any(|p| p.dist(soll) < 1e-6), "fehlt {soll:?}: {pts:?}");
        }
        let hit = find_snap_gated(&doc, Vec2::new(1502.0, 147.0), 30.0, None, &[], &[], false)
            .expect("Laibungsecke muss fangbar sein");
        assert!(hit.point.dist(Vec2::new(1500.0, 150.0)) < 1e-6, "{hit:?}");
    }

    #[test]
    fn wandkontur_fang_ist_abschaltbar() {
        let mut doc = Document::blank();
        let mut doc_aus = Document::blank();
        doc_aus.settings.snap.wall_edges = false;
        for d in [&mut doc, &mut doc_aus] {
            wand(d, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        }
        let cursor = Vec2::new(4.0, 146.0);
        let an = find_snap_gated(&doc, cursor, 30.0, None, &[], &[], false);
        assert!(an.is_some());
        // Aus: keine Konturpunkte mehr — und die Achse liefert hier nichts.
        let aus = find_snap_gated(&doc_aus, cursor, 30.0, None, &[], &[], false);
        assert!(aus.is_none(), "{aus:?}");
    }

    #[test]
    fn gehrungsecke_wird_nur_einmal_angeboten() {
        let mut doc = Document::blank();
        // Zwei Wände im rechten Winkel am gemeinsamen Punkt (0,0).
        wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        wand(&mut doc, Vec2::ZERO, Vec2::new(0.0, 4000.0));
        // Beide Wände rechnen die Gehrungsecke (-150,-150) — sie darf nur einmal
        // im Vorschlag stehen.
        let cands = snap_candidates_gated(&doc, Vec2::new(-150.0, -150.0), 200.0, None, &[], &[], false);
        for (i, a) in cands.iter().enumerate() {
            for b in cands.iter().skip(i + 1) {
                assert!(
                    a.point.dist(b.point) > SNAP_DEDUP,
                    "doppelt: {:?}",
                    (a, b)
                );
            }
        }
        let ecke = Vec2::new(-150.0, -150.0);
        let treffer = cands.iter().filter(|h| h.point.dist(ecke) < 1.0).count();
        assert_eq!(treffer, 1, "{cands:?}");
    }

    #[test]
    fn ausgewaehlte_wand_liefert_weiter_eckpunkte() {
        let mut doc = Document::blank();
        let id = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        let cursor = Vec2::new(4.0, 146.0);
        // Bisher wurde die Auswahl als `ignore` übergeben — dann kein Fang …
        assert!(
            find_snap_gated(&doc, cursor, 30.0, None, &[id], &[], false).is_none(),
            "ignoriertes Objekt darf nichts liefern"
        );
        // … als ausgewähltes Objekt liefert sie ihre Konturecke.
        let hit = find_snap_gated(&doc, cursor, 30.0, None, &[], &[id], false)
            .expect("ausgewählte Wandecke muss fangen");
        assert!(hit.point.dist(Vec2::new(0.0, 150.0)) < 1e-9, "{hit:?}");
        assert_eq!(hit.kind, SnapKind::Endpoint);
        assert_eq!(hit.entity, Some(id));
    }

    #[test]
    fn ausgewaehltes_objekt_liefert_keinen_naechsten_punkt() {
        let mut doc = Document::blank();
        let id = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        // 1000 mm die Wand entlang, 5 mm daneben: nichts diskretes in Reichweite.
        let cursor = Vec2::new(1000.0, 5.0);
        let ohne = find_snap_gated(&doc, cursor, 30.0, None, &[], &[], false);
        assert_eq!(
            ohne.map(|h| h.kind),
            Some(SnapKind::Nearest),
            "nicht ausgewählt: Nächster Punkt wie bisher"
        );
        // Ausgewählt: kein Zurückziehen auf das eigene Objekt (sonst klebt das
        // Verschieben am Startpunkt).
        assert!(
            find_snap_gated(&doc, cursor, 30.0, None, &[], &[id], false).is_none(),
            "ausgewähltes Objekt darf keinen Nächster-Punkt-Fang liefern"
        );
    }

    #[test]
    fn ausgewaehltes_objekt_bildet_keinen_schnittpunkt() {
        let mut doc = Document::blank();
        let a = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        wand(&mut doc, Vec2::new(2500.0, -2000.0), Vec2::new(2500.0, 2000.0));
        doc.settings.snap.nearest = false;
        doc.settings.snap.midpoint = false;
        doc.settings.snap.perpendicular = false;
        doc.settings.snap.grid = false;
        let cursor = Vec2::new(2500.0, 0.0);
        let ohne = snap_candidates_gated(&doc, cursor, 30.0, None, &[], &[], false);
        assert!(
            ohne
                .iter()
                .any(|h| h.kind == SnapKind::Intersection && h.point.dist(cursor) < 1.0),
            "{ohne:?}"
        );
        let mit = snap_candidates_gated(&doc, cursor, 30.0, None, &[], &[a], false);
        assert!(
            mit.iter().all(|h| h.kind != SnapKind::Intersection),
            "{mit:?}"
        );
    }

    #[test]
    fn nicht_ausgewaehlte_objekte_fangen_unveraendert() {
        let mut doc = Document::blank();
        let a = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        let b = wand(&mut doc, Vec2::new(0.0, -4000.0), Vec2::new(0.0, -1000.0));
        // Cursor an der Konturecke der *nicht* ausgewählten Wand b.
        let cursor = Vec2::new(147.0, -1004.0);
        let ohne = find_snap_gated(&doc, cursor, 30.0, None, &[], &[], false).expect("Fang an b");
        let mit =
            find_snap_gated(&doc, cursor, 30.0, None, &[], &[a], false).expect("Fang an b mit Auswahl");
        assert_eq!(ohne.point, mit.point);
        assert_eq!(ohne.kind, mit.kind);
        assert_eq!(mit.entity, Some(b));
    }

    #[test]
    fn verschieben_rastet_auf_eigener_wandecke() {
        use crate::session::Session;
        let mut s = Session::new();
        // Eingaben in Millimetern: Wand 0…5000 mm lang, 300 mm dick.
        s.execute("doc.settings", &serde_json::json!({"input_unit": "mm"}))
            .unwrap();
        s.execute(
            "draw.wall",
            &serde_json::json!({"a": [0.0, 0.0], "b": [5000.0, 0.0], "thickness": 300.0}),
        )
        .unwrap();
        let id = s.doc.entities[0].id;
        // Klick 6 mm neben der Konturecke (0,150) — der Fang zieht exakt dorthin.
        let basis = find_snap_gated(&s.doc, Vec2::new(6.0, 144.0), 30.0, None, &[], &[id], false)
            .expect("Wandecke als Basispunkt")
            .point;
        assert!(basis.dist(Vec2::new(0.0, 150.0)) < 1e-9, "{basis:?}");
        // Verschiebung um exakt 1000 mm: Achse liegt danach bei 1000…6000.
        let ziel = basis + v(1000.0, 0.0);
        s.execute(
            "modify.move",
            &serde_json::json!({"ids": [id], "from": [basis.x, basis.y], "to": [ziel.x, ziel.y]}),
        )
        .unwrap();
        match &s.doc.entities[0].kind {
            EntityKind::Wall { a, b, .. } => {
                assert!(a.dist(Vec2::new(1000.0, 0.0)) < 1e-9, "{a:?}");
                assert!(b.dist(Vec2::new(6000.0, 0.0)) < 1e-9, "{b:?}");
            }
            other => panic!("keine Wand: {other:?}"),
        }
    }

    #[test]
    fn rasterfang_folgt_der_rasteranzeige() {
        let doc = Document::blank();
        let p = Vec2::new(104.0, 97.0);
        // Raster eingeschaltet -> fängt auf 100/100
        let on = find_snap_gated(&doc, p, 20.0, None, &[], &[], true).expect("Rasterfang");
        assert!((on.point.x - 100.0).abs() < 1e-9);
        // Raster ausgeschaltet -> kein Rasterfang mehr
        assert!(find_snap_gated(&doc, p, 20.0, None, &[], &[], false).is_none());
    }
}