//! Direktes Bearbeiten über Griffe: Endpunkte, Ecken und Parallelverschiebung.

use crate::app::App;
use jjcad_core::geom::{Ellipse, Vec2 as V2, Vertex};
use jjcad_core::model::{Entity, EntityKind, Id};

/// Art eines Griffpunkts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GripKind {
    /// Eckpunkt / Endpunkt (Index in der Punktliste bzw. 0 = Start, 1 = Ende)
    Vert(usize),
    /// Mitte einer Kante eines geschlossenen Polygons (Parallelverschiebung)
    Edge(usize),
    /// Startpunkt eines Bogens: zieht den Startwinkel um den Mittelpunkt.
    ArcStart,
    /// Endpunkt eines Bogens: zieht den Endwinkel um den Mittelpunkt.
    ArcEnd,
    /// Textfeld eines Raumstempels: verschiebt **nur** die Beschriftung,
    /// die Umgrenzung bleibt stehen (T0097).
    Label,
}

#[derive(Clone, Debug)]
pub struct GripDrag {
    pub entity: Id,
    pub kind: GripKind,
    pub orig: Entity,
    /// Zuletzt angesteuerte Weltposition des Griffs (T0198). Beim Loslassen
    /// entscheidet sie, ob ein Messpunkt auf einem anderen liegt — der
    /// Griffindex taugt dafür nicht, weil die Messpunkte nach jeder Bewegung
    /// neu sortiert werden.
    pub ziel: Option<V2>,
}

/// Kleinster Knick, ab dem ein Randpunkt der Füllfläche als **Ecke** gilt
/// (1°). Der Farbeimer liefert seine Umgrenzung als dichten Punktzug entlang
/// der Wandkanten (gemessen: 125 Punkte für einen Raum); ohne diese Prüfung
/// läge auf jedem Zwischenpunkt ein Griff und die Fläche wäre nicht mehr zu
/// fassen.
const FILL_ECKE_MIN_WINKEL: f64 = 0.017_453_3;

/// Knickt der Rand an `i`? Zwischenpunkte auf einer geraden Kante bekommen
/// keinen Griff — nur die Ecken.
fn ist_ecke(ring: &[V2], i: usize) -> bool {
    let n = ring.len();
    if n < 3 {
        return true;
    }
    let p = ring[i];
    let davor = p - ring[(i + n - 1) % n];
    let danach = ring[(i + 1) % n] - p;
    let (l1, l2) = (davor.len(), danach.len());
    if l1 < 1e-9 || l2 < 1e-9 {
        return false;
    }
    // Kreuzprodukt = Sinus, Punktprodukt = Cosinus des Knickwinkels.
    let kreuz = davor.x * danach.y - davor.y * danach.x;
    let punkt = davor.dot(danach);
    kreuz.atan2(punkt).abs() > FILL_ECKE_MIN_WINKEL
}

/// Zwei **benachbarte** Randpunkte übereinandergelegt werden zu einem Punkt:
/// der gezogene fällt weg, die Zahl der Umgrenzungspunkte sinkt um eins.
///
/// Nur Nachbarn — bei nicht benachbarten Punkten verschwände ein ganzes Stück
/// Umgrenzung mit. Ein Ring behält mindestens drei Punkte, sonst bleibt keine
/// Fläche übrig. Liefert `true`, wenn verschmolzen wurde.
fn verschmelze_nachbar(ring: &mut Vec<V2>, i: usize, tol: f64) -> bool {
    let n = ring.len();
    if n <= 3 || i >= n {
        return false;
    }
    let p = ring[i];
    let davor = ring[(i + n - 1) % n];
    let danach = ring[(i + 1) % n];
    if p.dist(davor) <= tol || p.dist(danach) <= tol {
        ring.remove(i);
        return true;
    }
    false
}

/// Zwei **Messpunkte** einer Maßkette übereinandergelegt werden zu einem: der
/// andere fällt weg, der gezogene bleibt auf seiner Stelle (T0198).
///
/// Geprüft wird die **echte** Deckung, nicht die Lage auf der Maßlinie: die
/// Messpunkte dürfen quer zur Messrichtung verschieden stehen und liegen
/// projiziert trotzdem auf derselben Stelle — dort ist nichts doppelt. Unter
/// zwei Punkten bleibt die Kette stehen, sonst gäbe es kein Maß mehr.
///
/// Gesucht wird über die **Zielstelle** des Zugs, nicht über den Griffindex:
/// `render::dim_reihenfolge` sortiert die Punkte nach jeder Bewegung, der
/// gezogene steht danach an einer anderen Stelle der Liste. Liefert `true`,
/// wenn ein Punkt entfernt wurde.
fn verschmelze_messpunkt(pts: &mut Vec<V2>, ziel: V2, tol: f64) -> bool {
    if pts.len() <= 2 {
        return false;
    }
    // Der gezogene Punkt ist der nächste an der Zielstelle — er bleibt stehen.
    let Some(i) = pts
        .iter()
        .enumerate()
        .min_by(|a, b| {
            a.1.dist(ziel)
                .partial_cmp(&b.1.dist(ziel))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
    else {
        return false;
    };
    let p = pts[i];
    if let Some(k) = pts.iter().enumerate().position(|(k, q)| k != i && p.dist(*q) <= tol) {
        pts.remove(k);
        return true;
    }
    false
}

/// Ring einer Füllfläche zum flachen Eckindex `i` (Umgrenzung zuerst, dann die
/// Inseln) samt lokalem Index im Ring.
fn fill_ring_mut(kind: &mut EntityKind, i: usize) -> Option<(&mut Vec<V2>, usize)> {
    let (boundary, holes) = match kind {
        EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. } => (boundary, holes),
        _ => return None,
    };
    let mut base = 0usize;
    for ring in std::iter::once(&mut *boundary).chain(holes.iter_mut()) {
        let n = ring.len();
        if i < base + n {
            return Some((ring, i - base));
        }
        base += n;
    }
    None
}

/// Punkte eines Rings, die auf der Rechteckkante `i` liegen — die beiden Ecken
/// und alle Zwischenpunkte dazwischen.
///
/// Der Farbeimer liefert seinen Rand als dichten Punktzug; beim Verschieben
/// einer Kante müssen diese Zwischenpunkte mitwandern, sonst bliebe der Rand
/// stehen und die Fläche bekäme einen Knick. Gesucht wird über den Abstand zur
/// Kante (0,5 mm), nicht über Indizes — so ist die Reihenfolge der Punkte im
/// Ring gleichgültig.
fn rechteck_kante_punkte(ring: &[V2], ecken: &[V2; 4], i: usize) -> Vec<V2> {
    let p1 = ecken[i % 4];
    let p2 = ecken[(i + 1) % 4];
    ring.iter()
        .copied()
        .filter(|p| jjcad_core::geom::dist_point_segment(*p, p1, p2) <= 0.5)
        .collect()
}

/// Verschiebt die Kante `i` eines rechteckigen Rings **parallel**: alle Punkte
/// auf der Kante wandern um denselben Betrag in Normalenrichtung, die
/// gegenüberliegende Kante bleibt stehen. Damit wird das Rechteck breiter bzw.
/// schmaler, ohne zu kippen — dieselbe Bewegung wie an einer geschlossenen
/// Polylinie (`GripKind::Edge`), nur mit Zwischenpunkten (T0131).
///
/// Die Breite bleibt mindestens 1 mm; über die gegenüberliegende Kante hinaus
/// lässt sich die Kante nicht ziehen (sonst kehrte sich die Fläche um).
fn rechteck_kante_schieben(ring: &mut [V2], ecken: &[V2; 4], i: usize, target: V2) -> Option<()> {
    const MIN_BREITE_MM: f64 = 1.0;
    let p1 = ecken[i % 4];
    let p2 = ecken[(i + 1) % 4];
    let seg = p2 - p1;
    let len = seg.len();
    if len < 1e-6 {
        return None;
    }
    // Linke Normale der Kante; die Gegenkante liegt auf der anderen Seite.
    let nrm = V2::new(-seg.y, seg.x) / len;
    let mitte = (p1 + p2) * 0.5;
    let mut d = (target - mitte).dot(nrm);
    let gegen = (ecken[(i + 2) % 4] + ecken[(i + 3) % 4]) * 0.5;
    let breite = (mitte - gegen).dot(nrm);
    if breite.abs() < 1e-6 {
        return None;
    }
    if breite > 0.0 {
        // Kante liegt auf der +n-Seite der Gegenkante: der Abstand darf nicht
        // unter das Minimum fallen, weiter weg ist immer erlaubt.
        d = d.max(MIN_BREITE_MM - breite);
    } else {
        d = d.min(-MIN_BREITE_MM - breite);
    }
    if rechteck_kante_punkte(ring, ecken, i).is_empty() {
        return None;
    }
    for p in ring.iter_mut() {
        if jjcad_core::geom::dist_point_segment(*p, p1, p2) <= 0.5 {
            *p = *p + nrm * d;
        }
    }
    Some(())
}

/// Ecken des Rechtecks, das eine **geschlossene** Polylinie beschreibt (T0133).
///
/// Das Rechteck-Werkzeug legt eine geschlossene Polylinie mit vier Ecken an;
/// Zwischenpunkte auf geraden Kanten (kopiert, verschmolzen) zählen nicht. Wird
/// eine Ecke schräg gezogen, ist der Zug kein Rechteck mehr — dann gibt es
/// keine Kantengriffe und keine Parallelverschiebung.
fn rechteck_der_polylinie(verts: &[Vertex], closed: bool) -> Option<[V2; 4]> {
    if !closed || verts.len() < 4 {
        return None;
    }
    let ring: Vec<V2> = verts.iter().map(|v| v.p).collect();
    jjcad_core::geom::rechteck_ecken(&ring)
}

/// Ecken des Treppenrechtecks in Anzeigereihenfolge, dazu Richtung, Normale
/// und halbe Breite.
///
/// Die Treppe ist von der Kontur her ein **Rechteck**: `a`→`b` ist die
/// Lauflinie (Mitte), die Breite steht senkrecht darauf. Die vier Kanten sind
/// damit festgelegt — Kante 0 = linke Längskante, 1 = Endkante, 2 = rechte
/// Längskante, 3 = Startkante.
fn stair_rechteck(a: V2, b: V2, width: f64) -> ([V2; 4], V2, V2, f64) {
    let dir = if a.dist(b) > 1e-9 {
        (b - a).normalized()
    } else {
        V2::new(1.0, 0.0)
    };
    let nrm = dir.perp();
    let hw = width * 0.5;
    (
        [a + nrm * hw, b + nrm * hw, b - nrm * hw, a - nrm * hw],
        dir,
        nrm,
        hw,
    )
}

/// Griffe einer Bemaßung: ein Quadrat je Basispunkt, eine Raute an der Mitte
/// der Maßlinie.
///
/// Die Raute ([`GripKind::Edge(0)`]) zieht die Maßlinie **senkrecht** zu ihrer
/// Richtung — die Basispunkte bleiben stehen. Ohne sie ließe sich die Lage der
/// Bemaßung nur über das Eigenschaften-Panel ändern (T0055).
fn dim_griffe(e: &Entity) -> Vec<(GripKind, V2)> {
    let mut out: Vec<(GripKind, V2)> = match &e.kind {
        // Ein Quadrat je Messpunkt — beim Einzelmaß sind das zwei (T0154).
        EntityKind::Dim { pts, .. } => pts
            .iter()
            .enumerate()
            .map(|(i, p)| (GripKind::Vert(i), *p))
            .collect(),
        _ => return Vec::new(),
    };
    if let Some((s, t)) = jjcad_core::render::dim_line_span(&e.kind) {
        out.push((GripKind::Edge(0), (s + t) * 0.5));
    }
    out
}

/// Alle Griffpunkte eines Elements mit ihrer Weltposition.
pub fn grips_of(e: &Entity) -> Vec<(GripKind, V2)> {
    match &e.kind {
        EntityKind::Line { a, b } | EntityKind::Wall { a, b, .. } => {
            vec![(GripKind::Vert(0), *a), (GripKind::Vert(1), *b)]
        }
        // Polylinie: jeder Stützpunkt ist ein Griff. Ein **geschlossener** Zug
        // bekommt die vier Kantenmitten nur, solange er ein echtes Rechteck
        // ist (T0133) — ziehen verschiebt die Kante parallel, wie beim
        // Rechteck. Ist die Form kein Rechteck mehr, verschwinden sie.
        EntityKind::Polyline { verts, closed } => {
            let mut out: Vec<(GripKind, V2)> = verts
                .iter()
                .enumerate()
                .map(|(i, v)| (GripKind::Vert(i), v.p))
                .collect();
            if let Some(ecken) = rechteck_der_polylinie(verts, *closed) {
                for i in 0..4 {
                    out.push((
                        GripKind::Edge(i),
                        (ecken[i] + ecken[(i + 1) % 4]) * 0.5,
                    ));
                }
            }
            out
        }
        // Spline: nur die Stützpunkte — eine Kurve ist nie ein Rechteck, also
        // gibt es auch keine Kantengriffe (T0133).
        EntityKind::Spline { verts, .. } => verts
            .iter()
            .enumerate()
            .map(|(i, v)| (GripKind::Vert(i), v.p))
            .collect(),
        EntityKind::Circle { c, .. }
        | EntityKind::Ellipse { c, .. }
        | EntityKind::Point { p: c } => vec![(GripKind::Vert(0), *c)],
        // Grundriss-Kote: das Symbol sitzt am Kreismittelpunkt.
        EntityKind::ElevationPlan { p, .. } => vec![(GripKind::Vert(0), *p)],
        // Schnitt-Kote: jeder Punkt ist ein Griff; Ziehen ändert die Höhe
        // (Y-Differenz zum ersten Punkt).
        EntityKind::ElevationSection { pts } => pts
            .iter()
            .enumerate()
            .map(|(i, p)| (GripKind::Vert(i), *p))
            .collect(),
        // Bogen: Mittelpunkt (verschiebt den Bogen) und beide Enden. Ein
        // Endpunkt läuft auf dem Kreis mit, gezogen wird der Winkel; der
        // Radius bleibt dabei stehen (T0022).
        EntityKind::Arc { c, r, a0, a1 } => vec![
            (GripKind::Vert(0), *c),
            (GripKind::ArcStart, *c + V2::polar(*a0, *r)),
            (GripKind::ArcEnd, *c + V2::polar(*a1, *r)),
        ],
        // Ellipsenbogen: Mittelpunkt (verschiebt) und beide Enden. Ein Ende
        // läuft auf der Ellipse mit, gezogen wird der Parameterwinkel; Achsen
        // und Drehlage bleiben stehen (T0084).
        EntityKind::EllipseArc { c, rx, ry, rot, a0, a1 } => {
            let el = Ellipse::new(*c, *rx, *ry, *rot);
            vec![
                (GripKind::Vert(0), *c),
                (GripKind::ArcStart, el.point_at(*a0)),
                (GripKind::ArcEnd, el.point_at(*a1)),
            ]
        }
        // Treppe: die vier Kantenmitten des Rechtecks. Ziehen verschiebt genau
        // diese Kante parallel — die gegenüberliegende bleibt stehen, Breite
        // bzw. Länge ändern sich (T0027).
        EntityKind::Stair { a, b, width, .. } => {
            let (ecken, _, _, _) = stair_rechteck(*a, *b, *width);
            (0..4)
                .map(|i| {
                    let p = (ecken[i] + ecken[(i + 1) % 4]) * 0.5;
                    (GripKind::Edge(i), p)
                })
                .collect()
        }
        // Bemaßung: jeder Basispunkt ist ein Griff, dazu die Mitte der
        // Maßlinie (Raute, `GripKind::Edge(0)`) — Ziehen verschiebt sie
        // senkrecht zur Messrichtung (T0055).
        EntityKind::Dim { .. } => dim_griffe(e),
        // Raumstempel: die Ecken der (unsichtbaren) Umgrenzung erscheinen erst
        // mit der Auswahl des Textfeldes — genau dann ist das Objekt gewählt.
        // Der runde Griff am Textfeld verschiebt nur die Beschriftung.
        EntityKind::Room { boundary, label, .. } => {
            let mut out: Vec<(GripKind, V2)> = boundary
                .iter()
                .enumerate()
                .map(|(i, p)| (GripKind::Vert(i), *p))
                .collect();
            out.push((GripKind::Label, *label));
            out
        }
        // Ansichtsfenster: die vier Ecken des Rechtecks. Ziehen spannt das
        // Fenster neu auf, die gegenüberliegende Ecke bleibt stehen — wie bei
        // einem Rechteck (T0109).
        EntityKind::Viewport(vp) => viewport_ecken(vp)
            .iter()
            .enumerate()
            .map(|(i, p)| (GripKind::Vert(i), *p))
            .collect(),
        // Bild: die vier Ecken des Rahmens. Ziehen spannt das Bild neu auf,
        // die gegenüberliegende Ecke bleibt stehen (wie beim Rechteck).
        EntityKind::Image { .. } => bild_ecken(e)
            .iter()
            .enumerate()
            .map(|(i, p)| (GripKind::Vert(i), *p))
            .collect(),
        // Füllfläche: die Ecken der Umgrenzung und der Inseln. Zwischenpunkte
        // einer geraden Kante fallen weg (`ist_ecke`) — der Farbeimer liefert
        // seinen Rand als dichten Punktzug. Ist die Umgrenzung ein **echtes
        // Rechteck**, kommen die vier Kantenmitten dazu: Ziehen verschiebt die
        // Kante parallel, wie beim Rechteck (T0131). Inseln schließen die
        // Kantengriffe aus — eine Kante mit Loch lässt sich nicht als Ganzes
        // verschieben.
        EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. } => {
            let mut out: Vec<(GripKind, V2)> = Vec::new();
            let mut base = 0usize;
            for ring in std::iter::once(boundary).chain(holes.iter()) {
                for (i, p) in ring.iter().enumerate() {
                    if ist_ecke(ring, i) {
                        out.push((GripKind::Vert(base + i), *p));
                    }
                }
                base += ring.len();
            }
            if holes.is_empty() {
                if let Some(ecken) = jjcad_core::geom::rechteck_ecken(boundary) {
                    for i in 0..4 {
                        out.push((
                            GripKind::Edge(i),
                            (ecken[i] + ecken[(i + 1) % 4]) * 0.5,
                        ));
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Die vier Ecken eines Ansichtsfensters (links unten, rechts unten, rechts
/// oben, links oben) — dieselbe Reihenfolge wie `bild::ecken`.
fn viewport_ecken(vp: &jjcad_core::model::Viewport) -> [V2; 4] {
    [
        V2::new(vp.x, vp.y),
        V2::new(vp.x + vp.w, vp.y),
        V2::new(vp.x + vp.w, vp.y + vp.h),
        V2::new(vp.x, vp.y + vp.h),
    ]
}

/// Ecken eines Bildes in Weltkoordinaten (unten links, unten rechts, oben
/// rechts, oben links). Die Höhe folgt dem Seitenverhältnis bzw. dem
/// gespeicherten Wert — dieselbe Rechnung wie in der Anzeige.
fn bild_ecken(e: &Entity) -> [V2; 4] {
    if let EntityKind::Image {
        a,
        b,
        height,
        lock_ratio,
        data,
        ..
    } = &e.kind
    {
        let hoehe = jjcad_core::bild::dekodiere(data)
            .map(|bild| jjcad_core::bild::wirksame_hoehe(&bild, a.dist(*b), *height, *lock_ratio))
            .unwrap_or(1.0);
        jjcad_core::bild::ecken(*a, *b, hoehe)
    } else {
        [V2::ZERO; 4]
    }
}

/// Wendet eine Griffbewegung auf eine Kopie des Elements an.
pub fn apply_grip(orig: &Entity, kind: GripKind, target: V2) -> Option<Entity> {
    apply_grip_tol(orig, kind, target, jjcad_core::model::POINT_EPS_MM)
}

/// Wie [`apply_grip`], aber mit der Toleranz des laufenden Zugs (Bildschirm-px
/// in Welt-mm). Ein offener Polygonzug wird geschlossen, sobald ein Endpunkt
/// auf den anderen gezogen wird: der doppelte Stützpunkt fällt weg, `closed`
/// wird gesetzt — dieselbe Regel wie beim Setzen des letzten Punktes
/// (`model::polyline_points`).
pub fn apply_grip_tol(orig: &Entity, kind: GripKind, target: V2, tol: f64) -> Option<Entity> {
    let mut e = orig.clone();
    // Maßlinie einer Bemaßung: eigener Weg, weil die Bewegung nur senkrecht zur
    // Messrichtung läuft und die Rechnung im Kern steht
    // (`render::dim_line_move`) — so liegt der Griff auf der gezeichneten Linie.
    if kind == GripKind::Edge(0)
        && matches!(orig.kind, EntityKind::Dim { .. })
    {
        return if jjcad_core::render::dim_line_move(&mut e.kind, target) {
            Some(e)
        } else {
            None
        };
    }
    match (&mut e.kind, kind) {
        // Bemaßung: Basispunkte ziehen, die Maßlinie bleibt an ihrer Höhe.
        // Wandert der Punkt zwischen seine Nachbarn, wird die Kette danach neu
        // sortiert (`render::dim_reihenfolge`) — sonst zeigten die Maßzahlen
        // Strecken, die es so nicht mehr gibt (T0195).
        (EntityKind::Dim { pts, .. }, GripKind::Vert(i)) => {
            *pts.get_mut(i)? = target;
        }
        (EntityKind::ElevationPlan { p, .. }, GripKind::Vert(_)) => *p = target,
        (EntityKind::ElevationSection { pts }, GripKind::Vert(i)) => {
            *pts.get_mut(i)? = target;
        }
        (EntityKind::Line { a, .. }, GripKind::Vert(0)) => *a = target,
        (EntityKind::Line { b, .. }, GripKind::Vert(1)) => *b = target,
        (EntityKind::Wall { a, .. }, GripKind::Vert(0)) => *a = target,
        (EntityKind::Wall { b, .. }, GripKind::Vert(1)) => *b = target,
        (EntityKind::Point { p }, GripKind::Vert(_)) => *p = target,
        (EntityKind::Circle { c, .. }, GripKind::Vert(_)) => *c = target,
        (EntityKind::Arc { c, .. }, GripKind::Vert(_)) => *c = target,
        // Endpunkt eines Bogens: der Punkt folgt dem Zeiger auf dem Kreis,
        // gesetzt wird der Winkel — der Radius bleibt unverändert.
        (EntityKind::Arc { c, a0, .. }, GripKind::ArcStart)
        | (EntityKind::Arc { c, a1: a0, .. }, GripKind::ArcEnd) => {
            let d = target - *c;
            if d.len() < 1e-6 {
                return None;
            }
            *a0 = d.angle();
        }
        // Ellipsenbogen-Ende: der Punkt folgt dem Zeiger, gesetzt wird der
        // Parameterwinkel (`Ellipse::param_of`); Achsen bleiben unverändert.
        (EntityKind::EllipseArc { c, rx, ry, rot, a0, .. }, GripKind::ArcStart)
        | (EntityKind::EllipseArc { c, rx, ry, rot, a1: a0, .. }, GripKind::ArcEnd) => {
            *a0 = Ellipse::new(*c, *rx, *ry, *rot).param_of(target);
        }
        (EntityKind::EllipseArc { c, .. }, GripKind::Vert(_)) => *c = target,
        (EntityKind::Ellipse { c, .. }, GripKind::Vert(_)) => *c = target,
        (EntityKind::Polyline { verts, closed } | EntityKind::Spline { verts, closed }, GripKind::Vert(i)) => {
            let n = verts.len();
            verts.get_mut(i)?.p = target;
            // Endpunkt auf den anderen gezogen: der Zug ist geschlossen. Erst ab
            // vier Stützpunkten bleibt nach dem Wegfall des doppelten Punktes
            // noch eine Fläche (drei Ecken) übrig.
            if !*closed && n >= 4 && (i == n - 1 || i == 0) {
                let start = verts[0].p;
                let ende = verts[n - 1].p;
                if start.dist(ende) <= tol {
                    if i == 0 {
                        verts[0].p = ende;
                    }
                    verts.pop();
                    *closed = true;
                }
            }
        }
        // Rechteckige Polylinie: eine Kante parallel verschieben — die
        // gegenüberliegende bleibt stehen, Zwischenpunkte auf der Kante wandern
        // mit. Nur echte Rechtecke haben diesen Griff (T0133), die Rechnung
        // teilt sie mit der rechteckigen Fläche (T0131).
        (EntityKind::Polyline { verts, closed }, GripKind::Edge(i)) => {
            if !*closed || i >= 4 {
                return None;
            }
            let ring: Vec<V2> = verts.iter().map(|v| v.p).collect();
            let ecken = jjcad_core::geom::rechteck_ecken(&ring)?;
            let mut neu = ring;
            rechteck_kante_schieben(&mut neu, &ecken, i, target)?;
            for (v, p) in verts.iter_mut().zip(neu) {
                v.p = p;
            }
        }
        // Treppenkante parallel verschieben. Die gegenüberliegende Kante bleibt
        // stehen: Breite und Lage der Lauflinie folgen der halben Verschiebung.
        (EntityKind::Stair { a, b, width, .. }, GripKind::Edge(i)) => {
            let (_, dir, nrm, hw) = stair_rechteck(*a, *b, *width);
            let mitte = (*a + *b) * 0.5;
            const MIN: f64 = 100.0;
            match i {
                0 | 2 => {
                    let kante = if i == 0 {
                        mitte + nrm * hw
                    } else {
                        mitte - nrm * hw
                    };
                    let mut d = (target - kante).dot(nrm);
                    if i == 0 {
                        d = d.max(MIN - *width);
                        *width += d;
                    } else {
                        d = d.min(*width - MIN);
                        *width -= d;
                    }
                    *a = *a + nrm * (d * 0.5);
                    *b = *b + nrm * (d * 0.5);
                }
                1 => {
                    let t = (target - *b).dot(dir);
                    if (*b + dir * t).dist(*a) < MIN {
                        return None;
                    }
                    *b = *b + dir * t;
                }
                3 => {
                    let t = (target - *a).dot(dir);
                    if (*a + dir * t).dist(*b) < MIN {
                        return None;
                    }
                    *a = *a + dir * t;
                }
                _ => return None,
            }
        }
        // Ansichtsfenster: eine Ecke verschiebt genau diese Rechteckecke, die
        // gegenüberliegende bleibt stehen. Eine entartete Kante (unter 10 mm)
        // wird nicht übernommen — ein Fenster ohne Fläche wäre nicht mehr zu
        // fassen.
        (EntityKind::Viewport(vp), GripKind::Vert(i)) => {
            // Die gegenüberliegende Ecke bleibt stehen — das neue Rechteck
            // spannt sich zwischen ihr und dem Zeiger auf.
            let ecken = viewport_ecken(vp);
            let fest = *ecken.get((i + 2) % 4)?;
            let x0 = target.x.min(fest.x);
            let x1 = target.x.max(fest.x);
            let y0 = target.y.min(fest.y);
            let y1 = target.y.max(fest.y);
            if x1 - x0 < 10.0 || y1 - y0 < 10.0 {
                return None;
            }
            vp.x = x0;
            vp.y = y0;
            vp.w = x1 - x0;
            vp.h = y1 - y0;
        }
        // Bild: unten links/rechts verschieben die untere Kante (Lage, Drehung,
        // Breite). Die oberen Ecken ziehen die Höhe; bei gesperrtem Verhältnis
        // wächst die Breite mit, sonst bliebe das Bild verzerrt.
        (
            EntityKind::Image {
                a: ia,
                b: ib,
                height: ih,
                lock_ratio,
                data,
                ..
            },
            GripKind::Vert(i),
        ) => match i {
            0 => *ia = target,
            1 => *ib = target,
            2 | 3 => {
                let d = *ib - *ia;
                if d.len() < 1e-6 {
                    return None;
                }
                let n = d.perp().normalized();
                let h = (target - *ia).dot(n);
                if h < 1.0 {
                    return None;
                }
                *ih = h;
                if *lock_ratio {
                    if let Some(bild) = jjcad_core::bild::dekodiere(data) {
                        if bild.hoehe > 0 {
                            let breite = h * bild.breite as f64 / bild.hoehe as f64;
                            *ib = *ia + d.normalized() * breite.max(1.0);
                        }
                    }
                }
            }
            _ => return None,
        },
        // Raumstempel: eine Ecke ändert die Fläche (der Text rechnet sich neu),
        // der Textgriff verschiebt nur die Beschriftung — die Fläche bleibt.
        (EntityKind::Room { boundary, .. }, GripKind::Vert(i)) => {
            *boundary.get_mut(i)? = target;
        }
        (EntityKind::Room { label, .. }, GripKind::Label) => *label = target,
        // Ecke einer Füllfläche: der flache Index zählt erst die Umgrenzung,
        // dann die Inseln (dieselbe Reihenfolge wie die Anzeige). Verschmolzen
        // wird erst beim Loslassen (`App::end_grip_drag`) — während des Zugs
        // bleiben die Indizes dadurch gültig.
        (EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. }, GripKind::Vert(i)) => {
            let mut base = 0usize;
            let mut ziel: Option<&mut Vec<V2>> = None;
            for ring in std::iter::once(&mut *boundary).chain(holes.iter_mut()) {
                let n = ring.len();
                if i < base + n {
                    ziel = Some(ring);
                    break;
                }
                base += n;
            }
            let ring = ziel?;
            ring[i - base] = target;
        }
        // Rechteckige Füllfläche/Volumen: eine Kante parallel verschieben —
        // die gegenüberliegende bleibt stehen, die Nachbarkanten werden länger
        // bzw. kürzer. Dieselbe Bewegung wie an einer geschlossenen Polylinie
        // (`GripKind::Edge`), zusätzlich wandern die Zwischenpunkte auf der
        // Kante mit (T0131).
        (
            EntityKind::Fill {
                boundary, holes, ..
            }
            | EntityKind::Volume {
                boundary, holes, ..
            },
            GripKind::Edge(i),
        ) => {
            if !holes.is_empty() || i >= 4 {
                return None;
            }
            let ecken = jjcad_core::geom::rechteck_ecken(boundary)?;
            rechteck_kante_schieben(boundary, &ecken, i, target)?;
        }
        _ => return None,
    }
    // Messpunkte einer Bemaßung stehen nach jeder Bewegung wieder in
    // Messreihenfolge (T0195): ein gezogener Punkt kann zwischen seine Nachbarn
    // geraten, und die Maßzahlen hängen an der gespeicherten Reihenfolge.
    // Eine Stelle für alle Griffarten — die Maßlinien-Raute kehrt oben zurück
    // und rührt die Punkte nicht an.
    jjcad_core::render::dim_reihenfolge(&mut e.kind);
    Some(e)
}

/// **Erster** Bezugspunkt für Ortho und Polar beim Ziehen eines Griffs.
///
/// Beim Erstellen ist der Bezug der zuletzt gesetzte Punkt, deshalb rastet die
/// neue Strecke in Waagerechte, Senkrechte und 45° ein. Beim Ziehen an einem
/// Griff fehlte dieser Bezug ganz — der Griff folgte dem Zeiger ohne
/// Zwangsrichtung, und eine vorhandene Linie ließ sich nicht mehr klar
/// ausrichten (T0046). Bezug ist der **ruhende** Punkt, an dem die Richtung
/// gemessen wird; an einem Stützpunkt sind das **zwei** (voriger und nächster)
/// — siehe [`bezugspunkte`].
///
/// Kantenmitten (parallel verschieben), Mittelpunkte, Bogenenden und die
/// Treppenkanten haben keinen Bezug: ihre Bewegung ist bereits eindimensional
/// bzw. richtungslos.
pub fn bezugspunkt(e: &Entity, kind: GripKind) -> Option<V2> {
    bezugspunkte(e, kind).into_iter().next()
}

/// **Alle** Bezugspunkte eines gezogenen Griffs — in der Regel zwei.
///
/// Ein Stützpunkt einer Polylinie, eines Splines, eines Rechtecks oder einer
/// Fläche hängt an **zwei** Linien (zum vorigen und zum nächsten Punkt). Beim
/// Ausrichten müssen beide gleichzeitig zur Verfügung stehen, sonst lässt sich
/// immer nur eine von beiden in die Waagerechte/Senkrechte ziehen (T0046,
/// Turn 3). Reihenfolge: voriger Punkt zuerst, dann der nächste — der erste
/// ist der Bezug für den Polarfang und für Einzelbezüge.
///
/// Ein Endpunkt einer offenen Polylinie hat nur einen Nachbarn, Linie und Wand
/// genau einen (das andere Ende), Kantenmitten und Mittelpunkte keinen.
pub fn bezugspunkte(e: &Entity, kind: GripKind) -> Vec<V2> {
    match (&e.kind, kind) {
        (EntityKind::Line { b, .. } | EntityKind::Wall { b, .. }, GripKind::Vert(0)) => vec![*b],
        (EntityKind::Line { a, .. } | EntityKind::Wall { a, .. }, GripKind::Vert(1)) => vec![*a],
        // Bemaßung: der Bezug für Ortho/Polar ist der Nachbarpunkt der Kette.
        (EntityKind::Dim { pts, .. }, GripKind::Vert(i)) => {
            let n = pts.len();
            if n < 2 || i >= n {
                return Vec::new();
            }
            let mut out = Vec::new();
            if i > 0 {
                out.push(pts[i - 1]);
            }
            if i + 1 < n {
                out.push(pts[i + 1]);
            }
            out
        }
        (
            EntityKind::Polyline { verts, closed } | EntityKind::Spline { verts, closed },
            GripKind::Vert(i),
        ) => {
            let n = verts.len();
            if n < 2 || i >= n {
                return Vec::new();
            }
            let mut out = Vec::new();
            if i > 0 {
                out.push(verts[i - 1].p);
            } else if *closed && n >= 3 {
                out.push(verts[n - 1].p);
            }
            if i + 1 < n {
                out.push(verts[i + 1].p);
            } else if *closed && n >= 2 {
                out.push(verts[0].p);
            }
            out
        }
        // Raumstempel-Ecke: Bezug sind die beiden Nachbarecken im Ring.
        (EntityKind::Room { boundary, .. }, GripKind::Vert(i)) => {
            let n = boundary.len();
            if n < 2 || i >= n {
                return Vec::new();
            }
            let mut out = Vec::new();
            out.push(boundary[(i + n - 1) % n]);
            if n >= 3 {
                out.push(boundary[(i + 1) % n]);
            }
            out
        }
        (
            EntityKind::Fill { boundary, holes, .. }
            | EntityKind::Volume { boundary, holes, .. },
            GripKind::Vert(i),
        ) => {
            let mut base = 0usize;
            for ring in std::iter::once(boundary).chain(holes.iter()) {
                let n = ring.len();
                if i < base + n {
                    if n < 2 {
                        return Vec::new();
                    }
                    let k = i - base;
                    let mut out = Vec::new();
                    if n >= 2 {
                        out.push(ring[(k + n - 1) % n]);
                    }
                    if n >= 3 {
                        out.push(ring[(k + 1) % n]);
                    }
                    return out;
                }
                base += n;
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Richtung einer Kante (normiert): Startpunkt und Einheitsrichtung.
///
/// Gilt für die Kanten einer geschlossenen Polylinie **und** für die vier
/// Rechteckkanten einer Treppe — `canvas::draw_grips` zeichnet den Griff quer
/// zur Kante, ohne Richtung gäbe es keinen Griff zu sehen (T0027, Turn 3).
pub fn edge_dir(e: &Entity, i: usize) -> Option<(V2, V2)> {
    if let EntityKind::Stair { a, b, width, .. } = &e.kind {
        let (ecken, _, _, _) = stair_rechteck(*a, *b, *width);
        if i >= 4 {
            return None;
        }
        let p = ecken[i];
        let q = ecken[(i + 1) % 4];
        let d = q - p;
        let l = d.len();
        if l < 1e-9 {
            return None;
        }
        return Some((p, d / l));
    }
    // Rechteckige Füllfläche/Volumen: Richtung der erkannten Rechteckkante —
    // der Kantengriff steht quer dazu und verschiebt die Kante parallel
    // (T0131).
    if let EntityKind::Fill {
        boundary, holes, ..
    }
    | EntityKind::Volume {
        boundary, holes, ..
    } = &e.kind
    {
        if !holes.is_empty() || i >= 4 {
            return None;
        }
        let ecken = jjcad_core::geom::rechteck_ecken(boundary)?;
        let d = ecken[(i + 1) % 4] - ecken[i];
        let l = d.len();
        if l < 1e-9 {
            return None;
        }
        return Some((ecken[i], d / l));
    }
    // Maßlinie: Richtung der gezeichneten Linie — der Griff steht quer dazu
    // und wandert senkrecht (T0055).
    if matches!(e.kind, EntityKind::Dim { .. }) {
        let (s, t) = jjcad_core::render::dim_line_span(&e.kind)?;
        let d = t - s;
        let l = d.len();
        if l < 1e-9 {
            return None;
        }
        return Some((s, d / l));
    }
    // Rechteckige Polylinie: Richtung der erkannten Rechteckkante — der
    // Kantengriff steht quer dazu und verschiebt die Kante parallel (T0133).
    // Ohne Rechteck gibt es keinen Kantengriff, also auch keine Richtung.
    if let EntityKind::Polyline { verts, closed } = &e.kind {
        if i >= 4 {
            return None;
        }
        let ecken = rechteck_der_polylinie(verts, *closed)?;
        let d = ecken[(i + 1) % 4] - ecken[i];
        let l = d.len();
        if l < 1e-9 {
            return None;
        }
        Some((ecken[i], d / l))
    } else {
        None
    }
}

impl App {
    /// Griff unter dem Cursor (nur in der Auswahl).
    pub fn grip_at(&self, p: V2, tol: f64) -> Option<(Id, GripKind)> {
        for id in self.selection() {
            if let Some(e) = self.doc().entity(*id) {
                for (kind, gp) in grips_of(e) {
                    if p.dist(gp) <= tol {
                        return Some((*id, kind));
                    }
                }
            }
        }
        None
    }

    /// Beginnt das Ziehen an einem Griff (mit Undo-Schnappschuss).
    pub fn begin_grip_drag(&mut self, entity: Id, kind: GripKind) {
        let Some(orig) = self.doc().entity(entity).cloned() else {
            return;
        };
        let snap = self.doc().clone();
        self.session_mut().history.record(&snap);
        self.grip_drag = Some(GripDrag {
            entity,
            kind,
            orig,
            ziel: None,
        });
        self.mitteilung(jjcad_core::i18n::tr("gui.handles.griff_ziehen"));
    }

    /// Verschiebt den Griff auf die Weltposition `target`.
    pub fn update_grip_drag(&mut self, target: V2) -> bool {
        let Some(d) = self.grip_drag.clone() else {
            return false;
        };
        let Some(new_e) = apply_grip_tol(
            &d.orig,
            d.kind,
            target,
            (12.0 / self.current_view().zoom.max(1e-6)).max(jjcad_core::model::POINT_EPS_MM),
        ) else {

            return false;
        };
        if let Some(e) = self.doc_mut().entity_mut(d.entity) {
            *e = new_e;
            self.mark_changed();
            if let Some(drag) = self.grip_drag.as_mut() {
                drag.ziel = Some(target);
            }
            true
        } else {
            false
        }
    }

    /// Beendet den Griffzug. Liegt die gezogene Ecke einer Füllfläche auf einer
    /// ihrer Nachbarecken, werden beide zu **einem** Punkt: die Umgrenzung hat
    /// danach einen Punkt weniger. Dasselbe gilt für einen **Messpunkt** einer
    /// Maßkette, der auf einem anderen Messpunkt derselben Kette liegt (T0198).
    pub fn end_grip_drag(&mut self) {
        let Some(d) = self.grip_drag.take() else {
            return;
        };
        let mut verbunden = false;
        let mut masskette = false;
        if let GripKind::Vert(i) = d.kind {
            let tol = (12.0 / self.current_view().zoom.max(1e-6))
                .max(jjcad_core::model::POINT_EPS_MM);
            if let Some(e) = self.doc_mut().entity_mut(d.entity) {
                if matches!(
                    e.kind,
                    EntityKind::Fill { .. } | EntityKind::Volume { .. }
                ) {
                    if let Some((ring, idx)) = fill_ring_mut(&mut e.kind, i) {
                        verbunden = verschmelze_nachbar(ring, idx, tol);
                    }
                } else if let EntityKind::Dim { pts, .. } = &mut e.kind {
                    if let Some(ziel) = d.ziel {
                        verbunden = verschmelze_messpunkt(pts, ziel, tol);
                    }
                    if verbunden {
                        // Nach dem Wegfall steht die Kette wieder in
                        // Messreihenfolge (T0195) — die Maßzahlen hängen an der
                        // gespeicherten Reihenfolge.
                        jjcad_core::render::dim_reihenfolge(&mut e.kind);
                        masskette = true;
                    }
                }
                if verbunden {
                    self.mark_changed();
                }
            }
        }
        if masskette {
            self.mitteilung(jjcad_core::i18n::tr("gui.handles.masskette_punkt_entfernt"));
        } else if verbunden {
            self.mitteilung(jjcad_core::i18n::tr("gui.handles.fuellflaeche_punkt_verbunden"));
        } else {
            self.mitteilung(jjcad_core::i18n::tr("gui.handles.griff_gesetzt"));
        }
    }

    pub fn cancel_grip_drag(&mut self) {
        if let Some(d) = self.grip_drag.take() {
            if let Some(e) = self.doc_mut().entity_mut(d.entity) {
                *e = d.orig;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jjcad_core::geom::Vertex;

    fn offen(pts: &[V2]) -> Entity {
        Entity::new(
            1,
            1,
            EntityKind::Polyline {
                verts: pts.iter().map(|p| Vertex::new(*p)).collect(),
                closed: false,
            },
        )
    }

    fn stand(e: &Entity) -> (Vec<V2>, bool) {
        match &e.kind {
            EntityKind::Polyline { verts, closed } => {
                (verts.iter().map(|v| v.p).collect(), *closed)
            }
            _ => panic!("keine Polylinie"),
        }
    }

    /// Letzten Stützpunkt auf den Startpunkt ziehen schließt den Zug: der
    /// doppelte Punkt fällt weg (drei Ecken bleiben), `closed` steht.
    #[test]
    fn griff_auf_startpunkt_schliesst_die_polylinie() {
        let orig = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(500.0, 1500.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(3), V2::new(0.0, 0.0), 1.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(closed, "Closed-Flag fehlt nach dem Griffzug");
        assert_eq!(pts.len(), 3, "der doppelte Stützpunkt bleibt stehen");
        assert_eq!(pts[0], V2::new(0.0, 0.0));
    }

    /// Auch der erste Stützpunkt darf auf den letzten gezogen werden; die
    /// Geometrie bleibt dabei erhalten (der letzte Punkt wird der neue erste).
    #[test]
    fn griff_des_startpunkts_auf_das_ende_schliesst() {
        let orig = offen(&[
            V2::new(500.0, 1500.0),
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(0), V2::new(2000.0, 1000.0), 1.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(closed);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0], V2::new(2000.0, 1000.0), "Endpunkt wird der neue Start");
    }

    /// Weit weg vom Gegenpunkt bleibt der Zug offen; ein mittlerer Stützpunkt
    /// schließt nie.
    #[test]
    fn griff_abseits_schliesst_nicht() {
        let orig = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(500.0, 1500.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(3), V2::new(0.0, 100.0), 1.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(!closed);
        assert_eq!(pts.len(), 4);

        let mitte = apply_grip_tol(&orig, GripKind::Vert(1), V2::new(0.0, 0.0), 1.0).unwrap();
        let (pts2, closed2) = stand(&mitte);
        assert!(!closed2, "ein mittlerer Stützpunkt schließt nichts");
        assert_eq!(pts2.len(), 4);
    }

    // ---- Füllflächen (T0018) ----

    fn flaeche(rand: &[V2], inseln: &[&[V2]]) -> Entity {
        Entity::new(
            7,
            1,
            EntityKind::Fill {
                boundary: rand.to_vec(),
                holes: inseln.iter().map(|h| h.to_vec()).collect(),
                pattern: jjcad_core::style::FillPattern::Lines,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: true,
            },
        )
    }

    fn raender(e: &Entity) -> (Vec<V2>, Vec<Vec<V2>>) {
        match &e.kind {
            EntityKind::Fill {
                boundary, holes, ..
            } => (boundary.clone(), holes.clone()),
            _ => panic!("keine Füllfläche"),
        }
    }

    fn quadrat() -> Entity {
        flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[],
        )
    }

    /// Vier Ecken — und nur dort Griffe, genau auf der Umgrenzung.
    #[test]
    fn fuellflaeche_hat_griffe_an_den_ecken() {
        let e = quadrat();
        let g = grips_of(&e);
        let ecken: Vec<V2> = g
            .iter()
            .filter(|(k, _)| matches!(k, GripKind::Vert(_)))
            .map(|(_, p)| *p)
            .collect();
        assert_eq!(ecken.len(), 4, "vier Eckgriffe: {g:?}");
        assert_eq!(ecken[0], V2::new(0.0, 0.0));
        assert_eq!(ecken[1], V2::new(4000.0, 0.0));
        assert_eq!(ecken[2], V2::new(4000.0, 3000.0));
        assert_eq!(ecken[3], V2::new(0.0, 3000.0));
    }

    /// Zwischenpunkte einer geraden Kante bekommen keinen Griff: der Farbeimer
    /// liefert seinen Rand als dichten Punktzug (ein Raum = 125 Punkte), sonst
    /// wäre die Fläche unter lauter Griffen nicht mehr zu treffen.
    #[test]
    fn gerade_kante_bekommt_keinen_griff() {
        let e = flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(1000.0, 0.0),
                V2::new(2000.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[],
        );
        let ecken: Vec<usize> = grips_of(&e)
            .into_iter()
            .filter_map(|(k, _)| match k {
                GripKind::Vert(i) => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(ecken, vec![0, 3, 4, 5], "nur die vier echten Ecken");
    }

    // ---- Rechteckige Fläche: Kantengriffe (T0131) ----

    /// Ein echtes Rechteck bekommt zusätzlich die vier Kantenmitten: Ziehen
    /// verschiebt die Kante parallel, wie beim Rechteck.
    #[test]
    fn rechteckige_flaeche_hat_kantengriffe() {
        let e = quadrat();
        let g = grips_of(&e);
        let kanten: Vec<(usize, V2)> = g
            .iter()
            .filter_map(|(k, p)| match k {
                GripKind::Edge(i) => Some((*i, *p)),
                _ => None,
            })
            .collect();
        assert_eq!(kanten.len(), 4, "vier Kantengriffe: {g:?}");
        let mitten: Vec<V2> = kanten.iter().map(|(_, p)| *p).collect();
        assert!(mitten.contains(&V2::new(2000.0, 0.0)), "unten: {mitten:?}");
        assert!(mitten.contains(&V2::new(4000.0, 1500.0)), "rechts: {mitten:?}");
        assert!(mitten.contains(&V2::new(2000.0, 3000.0)), "oben: {mitten:?}");
        assert!(mitten.contains(&V2::new(0.0, 1500.0)), "links: {mitten:?}");
        // Jeder Kantengriff braucht eine Richtung — ohne sie zeichnet
        // `draw_grips` keinen Griff (er bliebe unsichtbar aber klickbar).
        for (i, _) in kanten {
            assert!(
                edge_dir(&e, i).is_some(),
                "Kante {i} ohne Richtung — der Griff bliebe unsichtbar"
            );
        }
    }

    /// Eine Kante ziehen verschiebt **nur** diese Kante: die gegenüberliegende
    /// bleibt stehen, die Fläche wird breiter (T0131).
    #[test]
    fn kante_parallel_schieben_laesst_die_gegenkante_stehen() {
        let neu = apply_grip_tol(&quadrat(), GripKind::Edge(0), V2::new(2000.0, -500.0), 1.0)
            .expect("Kantengriff muss greifen");
        let (rand, _) = raender(&neu);
        assert_eq!(rand[0], V2::new(0.0, -500.0), "linke Ecke wandert mit");
        assert_eq!(rand[1], V2::new(4000.0, -500.0), "rechte Ecke wandert mit");
        assert_eq!(rand[2], V2::new(4000.0, 3000.0), "Gegenkante bleibt");
        assert_eq!(rand[3], V2::new(0.0, 3000.0), "Gegenkante bleibt");
        let flaeche = jjcad_core::geom::polygon_area(&rand).abs();
        assert!((flaeche - 4000.0 * 3500.0).abs() < 1e-6, "Fläche {flaeche}");
    }

    /// Auch die Zwischenpunkte einer geraden Kante wandern mit — sonst bekäme
    /// der Rand des Farbeimers einen Knick (T0131).
    #[test]
    fn kante_schiebt_zwischenpunkte_mit() {
        let e = flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(1000.0, 0.0),
                V2::new(2000.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[],
        );
        let neu = apply_grip_tol(&e, GripKind::Edge(0), V2::new(2000.0, 250.0), 1.0)
            .expect("Kantengriff muss greifen");
        let (rand, _) = raender(&neu);
        for (i, p) in rand.iter().enumerate().take(4) {
            assert!(
                (p.y - 250.0).abs() < 1e-6,
                "Punkt {i} auf der Kante nicht mitgewandert: {p:?}"
            );
        }
        assert_eq!(rand[4], V2::new(4000.0, 3000.0));
        assert_eq!(rand[5], V2::new(0.0, 3000.0));
    }

    /// Über die gegenüberliegende Kante hinaus lässt sich die Kante nicht
    /// ziehen — sonst kehrte sich die Fläche um (T0131).
    #[test]
    fn kante_kippt_nicht_ueber_die_gegenkante() {
        let neu = apply_grip_tol(&quadrat(), GripKind::Edge(0), V2::new(2000.0, 9000.0), 1.0)
            .expect("Kantengriff muss greifen");
        let (rand, _) = raender(&neu);
        let breite = (rand[2].y - rand[0].y).abs();
        assert!(
            breite >= 0.999,
            "Breite {breite} mm — Fläche wäre umgekehrt"
        );
        assert!(
            jjcad_core::geom::polygon_area(&rand) > 0.0,
            "Fläche muss positiv bleiben"
        );
    }

    /// Ein schiefes Viereck und ein Rechteck mit Insel sind keine „echten"
    /// Rechtecke im Sinn der Kantengriffe (T0131).
    #[test]
    fn nur_das_echte_rechteck_bekommt_kantengriffe() {
        let schief = flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(2000.0, 0.0),
                V2::new(2500.0, 1000.0),
                V2::new(500.0, 1000.0),
            ],
            &[],
        );
        assert!(
            !grips_of(&schief)
                .iter()
                .any(|(k, _)| matches!(k, GripKind::Edge(_))),
            "Parallelogramm darf keine Kantengriffe haben"
        );

        let mit_insel = flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[&[
                V2::new(1000.0, 1000.0),
                V2::new(2000.0, 1000.0),
                V2::new(2000.0, 2000.0),
            ]],
        );
        assert!(
            !grips_of(&mit_insel)
                .iter()
                .any(|(k, _)| matches!(k, GripKind::Edge(_))),
            "Rechteck mit Insel darf keine Kantengriffe haben"
        );
    }


    /// Ecke ziehen: nur dieser Punkt wandert, Muster und Umrandung bleiben.
    #[test]
    fn fuellflaeche_ecke_ziehen_aendert_nur_diesen_punkt() {
        let neu = apply_grip_tol(&quadrat(), GripKind::Vert(2), V2::new(5000.0, 3500.0), 1.0)
            .expect("Griff muss greifen");
        let (rand, inseln) = raender(&neu);
        assert_eq!(rand[2], V2::new(5000.0, 3500.0));
        assert_eq!(rand[0], V2::new(0.0, 0.0));
        assert_eq!(rand[1], V2::new(4000.0, 0.0));
        assert_eq!(rand[3], V2::new(0.0, 3000.0));
        assert!(inseln.is_empty());
        match &neu.kind {
            EntityKind::Fill {
                pattern,
                pattern_scale,
                border,
                ..
            } => {
                assert_eq!(*pattern, jjcad_core::style::FillPattern::Lines);
                assert_eq!(*pattern_scale, 100.0);
                assert!(*border);
            }
            _ => panic!("keine Füllfläche"),
        }
    }

    /// Inseln zählen hinter der Umgrenzung: ihr Eckgriff verschiebt nur die
    /// Insel, die Umgrenzung bleibt stehen.
    #[test]
    fn insel_ecke_ziehen_laesst_die_umgrenzung_stehen() {
        let e = flaeche(
            &[
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            &[&[
                V2::new(1000.0, 1000.0),
                V2::new(1500.0, 1000.0),
                V2::new(1500.0, 1500.0),
            ]],
        );
        let g = grips_of(&e);
        // 4 Ecken der Umgrenzung, danach 3 Ecken der Insel (flach gezählt).
        assert_eq!(g.len(), 7, "Griffe: {g:?}");
        assert!(g.contains(&(GripKind::Vert(4), V2::new(1000.0, 1000.0))));
        assert!(g.contains(&(GripKind::Vert(6), V2::new(1500.0, 1500.0))));

        let neu = apply_grip_tol(&e, GripKind::Vert(5), V2::new(2000.0, 900.0), 1.0).unwrap();
        let (rand, inseln) = raender(&neu);
        assert_eq!(rand, raender(&e).0, "Umgrenzung bleibt unverändert");
        assert_eq!(inseln[0][1], V2::new(2000.0, 900.0));
        assert_eq!(inseln[0][0], V2::new(1000.0, 1000.0));
        assert_eq!(inseln[0][2], V2::new(1500.0, 1500.0));
    }


    // ---- Bogen: Endpunkte ziehen (T0022) ----

    /// Bogen um (1000;1000) mit 500 mm von 0° bis 90°.
    fn bogen() -> Entity {
        Entity::new(
            1,
            1,
            EntityKind::Arc {
                c: V2::new(1000.0, 1000.0),
                r: 500.0,
                a0: 0.0,
                a1: std::f64::consts::FRAC_PI_2,
            },
        )
    }

    fn bogen_werte(e: &Entity) -> (V2, f64, f64, f64) {
        match &e.kind {
            EntityKind::Arc { c, r, a0, a1 } => (*c, *r, *a0, *a1),
            other => panic!("kein Bogen: {other:?}"),
        }
    }

    /// Drei Griffe: Mittelpunkt in der Mitte, je einer auf den beiden Enden.
    #[test]
    fn bogen_hat_griffe_am_mittelpunkt_und_an_den_enden() {
        let e = bogen();
        let g = grips_of(&e);
        assert_eq!(g.len(), 3, "Griffe: {g:?}");
        assert!(g.contains(&(GripKind::Vert(0), V2::new(1000.0, 1000.0))));
        let start = g
            .iter()
            .find(|(k, _)| *k == GripKind::ArcStart)
            .expect("Startgriff fehlt")
            .1;
        let ende = g
            .iter()
            .find(|(k, _)| *k == GripKind::ArcEnd)
            .expect("Endgriff fehlt")
            .1;
        assert!(start.dist(V2::new(1500.0, 1000.0)) < 1e-9, "Start {start:?}");
        assert!(ende.dist(V2::new(1000.0, 1500.0)) < 1e-9, "Ende {ende:?}");
    }

    /// Endpunkt ziehen setzt den Endwinkel — Radius und Mittelpunkt bleiben.
    #[test]
    fn bogen_ende_ziehen_aendert_den_endwinkel() {
        let e = bogen();
        // Zeiger unten am Mittelpunkt: Richtung 270°
        let neu = apply_grip_tol(&e, GripKind::ArcEnd, V2::new(1000.0, 0.0), 1.0)
            .expect("Endgriff muss greifen");
        let (c, r, a0, a1) = bogen_werte(&neu);
        assert_eq!(c, V2::new(1000.0, 1000.0), "Mittelpunkt verschoben");
        assert_eq!(r, 500.0, "Radius geändert");
        assert_eq!(a0, 0.0, "Startwinkel geändert");
        assert!((a1 + std::f64::consts::FRAC_PI_2).abs() < 1e-9, "a1 {a1}");
    }

    /// Startpunkt ziehen setzt den Startwinkel; der Endpunkt läuft auf dem
    /// Kreis mit — auch wenn der Zeiger weiter außen liegt.
    #[test]
    fn bogen_start_ziehen_aendert_den_startwinkel() {
        let e = bogen();
        let ziel = V2::new(1000.0, 3000.0); // Richtung 90°, Abstand 2000 mm
        let neu = apply_grip_tol(&e, GripKind::ArcStart, ziel, 1.0).expect("Startgriff");
        let (c, r, a0, a1) = bogen_werte(&neu);
        assert_eq!(r, 500.0, "Radius folgt dem Zeiger statt zu bleiben");
        assert!((a0 - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "a0 {a0}");
        assert_eq!(a1, std::f64::consts::FRAC_PI_2, "Endwinkel geändert");
        // Der Punkt liegt auf dem Kreis, nicht beim Zeiger.
        let start = c + V2::polar(a0, r);
        assert!(start.dist(V2::new(1000.0, 1500.0)) < 1e-9, "Start {start:?}");
    }

    /// Zeiger genau auf den Mittelpunkt: kein Zug (Richtung unbestimmt).
    #[test]
    fn bogen_ende_auf_mittelpunkt_bewegt_nichts() {
        let e = bogen();
        assert!(apply_grip_tol(&e, GripKind::ArcEnd, V2::new(1000.0, 1000.0), 1.0).is_none());
        assert!(apply_grip_tol(&e, GripKind::ArcStart, V2::new(1000.0, 1000.0), 1.0).is_none());
    }


    // ---- Zwei Ecken aufeinandergelegt: ein Punkt (T0018, Turn 2) ----

    /// Benachbarte Ecken auf derselben Stelle: der gezogene fällt weg, die
    /// Umgrenzung hat einen Punkt weniger.
    #[test]
    fn benachbarte_ecken_verschmelzen_zu_einem_punkt() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(4000.0, 0.0),
            V2::new(4000.0, 3000.0),
            V2::new(0.0, 3000.0),
        ];
        ring[1] = ring[0];
        assert!(verschmelze_nachbar(&mut ring, 1, 1.0));
        assert_eq!(ring.len(), 3, "ein Umrandungspunkt weniger: {ring:?}");
        assert_eq!(
            ring,
            vec![
                V2::new(0.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0)
            ]
        );
    }

    /// Dasselbe über den Griffweg: der Punkt landet auf dem Nachbarn, die
    /// Fläche bleibt eine Fläche mit drei Ecken.
    #[test]
    fn griffzug_auf_die_nachbarecke_verschmilzt() {
        let neu = apply_grip_tol(&quadrat(), GripKind::Vert(1), V2::new(0.0, 0.0), 1.0).unwrap();
        let (rand, _) = raender(&neu);
        assert_eq!(rand.len(), 4, "der Zug selbst verändert die Zahl der Punkte nicht");
        let mut ring = rand;
        assert!(verschmelze_nachbar(&mut ring, 1, 1.0));
        assert_eq!(ring.len(), 3);
    }

    /// Nur echte Berührung verschmilzt — knapp daneben bleibt der Punkt stehen.
    #[test]
    fn knapp_daneben_verschmilzt_nicht() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(4000.0, 0.0),
            V2::new(4000.0, 3000.0),
            V2::new(0.0, 3000.0),
        ];
        ring[1] = V2::new(0.0, 5.0);
        assert!(!verschmelze_nachbar(&mut ring, 1, 1.0));
        assert_eq!(ring.len(), 4);
    }

    /// Ein Dreieck bleibt ein Dreieck: zwei Punkte wären keine Fläche mehr.
    #[test]
    fn dreieck_behaelt_drei_punkte() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(0.0, 1000.0),
        ];
        ring[0] = ring[1];
        assert!(!verschmelze_nachbar(&mut ring, 0, 10.0));
        assert_eq!(ring.len(), 3);
    }

    /// Nicht benachbarte Punkte verschmelzen nicht — sonst verschwände ein
    /// ganzes Stück Umgrenzung mit.
    #[test]
    fn nicht_benachbarte_punkte_verschmelzen_nicht() {
        let mut ring = vec![
            V2::new(2000.0, 0.0),
            V2::new(1000.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(0.0, 1000.0),
        ];
        assert!(!verschmelze_nachbar(&mut ring, 0, 1.0));
        assert_eq!(ring.len(), 5);
    }

    /// Der gezogene Punkt verschwindet auch am Ringende (Nachbar = erster
    /// Punkt): die Umgrenzung schließt sich ohne Doppelpunkt.
    #[test]
    fn verschmelzen_ueber_die_ringnaht() {
        let mut ring = vec![
            V2::new(0.0, 0.0),
            V2::new(4000.0, 0.0),
            V2::new(4000.0, 3000.0),
            V2::new(0.0, 3000.0),
        ];
        ring[3] = ring[0];
        assert!(verschmelze_nachbar(&mut ring, 3, 1.0));
        assert_eq!(ring.len(), 3, "{ring:?}");
    }

    // ---- Fang-Sperre: nicht sperren, was selbst das Ziel ist (T0018, Turn 3) ----


    /// Ein Griff, den es nicht gibt, verändert nichts (kein Panik, kein None-Fehler).
    #[test]
    fn fuellflaeche_ohne_passenden_griff_bleibt_unveraendert() {
        let e = quadrat();
        assert!(apply_grip_tol(&e, GripKind::Vert(9), V2::new(1.0, 1.0), 1.0).is_none());
        assert!(apply_grip_tol(&e, GripKind::Edge(9), V2::new(1.0, 1.0), 1.0).is_none());
    }

    /// Ein Dreieck (drei Stützpunkte) lässt sich nicht durch einen Griffzug
    /// schließen — nach dem Wegfall des doppelten Punktes bliebe keine Fläche.
    #[test]
    fn dreieck_wird_nicht_geschlossen() {
        let orig = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(0.0, 1000.0),
        ]);
        let neu = apply_grip_tol(&orig, GripKind::Vert(2), V2::new(0.0, 0.0), 5.0).unwrap();
        let (pts, closed) = stand(&neu);
        assert!(!closed);
        assert_eq!(pts.len(), 3);
    }

    // ---- Bezugspunkt für Ortho/Polar beim Griffzug (T0046) ----

    fn linie() -> Entity {
        Entity::new(
            1,
            1,
            EntityKind::Line {
                a: V2::new(0.0, 0.0),
                b: V2::new(2000.0, 1000.0),
            },
        )
    }

    /// Linie und Wand messen die Richtung am **anderen** Ende.
    #[test]
    fn bezug_der_linie_ist_das_andere_ende() {
        let e = linie();
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(2000.0, 1000.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(1)), Some(V2::new(0.0, 0.0)));

        let w = Entity::new(
            1,
            1,
            EntityKind::Wall {
                a: V2::new(0.0, 0.0),
                b: V2::new(2000.0, 1000.0),
                thickness: 240.0,
                height: 2750.0,
                material: String::new(),
                hatch: None,
                align: jjcad_core::style::WallAlign::Center,
            },
        );
        assert_eq!(bezugspunkt(&w, GripKind::Vert(0)), Some(V2::new(2000.0, 1000.0)));
        assert_eq!(bezugspunkt(&w, GripKind::Vert(1)), Some(V2::new(0.0, 0.0)));
    }

    /// Offene Polylinie: der vorige Stützpunkt, am Anfang der nächste.
    #[test]
    fn bezug_der_offenen_polylinie_ist_die_nachbarecke() {
        let e = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
        ]);
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(2000.0, 0.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(2)), Some(V2::new(2000.0, 0.0)));
    }

    /// Geschlossener Zug (Rechteck): auch der Anfang hat einen Vorgänger —
    /// die letzte Ecke des Rings.
    #[test]
    fn bezug_des_geschlossenen_zugs_ist_die_ringecke() {
        let mut e = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
            V2::new(0.0, 1000.0),
        ]);
        if let EntityKind::Polyline { closed, .. } = &mut e.kind {
            *closed = true;
        }
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(0.0, 1000.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(1)), Some(V2::new(0.0, 0.0)));
    }

    /// Füllfläche und Volumen: der vorige Punkt desselben Rings.
    #[test]
    fn bezug_der_flaeche_ist_der_vorige_ringpunkt() {
        let e = quadrat();
        assert_eq!(bezugspunkt(&e, GripKind::Vert(0)), Some(V2::new(0.0, 3000.0)));
        assert_eq!(bezugspunkt(&e, GripKind::Vert(2)), Some(V2::new(4000.0, 0.0)));
    }

    /// Kantenmitten und Mittelpunkte bewegen eindimensional bzw. richtungslos —
    /// dort gibt es keinen Bezug (sonst zöge Ortho sie aus ihrer Bahn).
    #[test]
    fn ohne_bezug_bei_kantenmitte_und_mittelpunkt() {
        let mut e = offen(&[
            V2::new(0.0, 0.0),
            V2::new(2000.0, 0.0),
            V2::new(2000.0, 1000.0),
        ]);
        if let EntityKind::Polyline { closed, .. } = &mut e.kind {
            *closed = true;
        }
        assert_eq!(bezugspunkt(&e, GripKind::Edge(0)), None);
        assert_eq!(bezugspunkt(&quadrat(), GripKind::Edge(0)), None);

        let kreis = Entity::new(
            1,
            1,
            EntityKind::Circle {
                c: V2::new(0.0, 0.0),
                r: 500.0,
            },
        );
        assert_eq!(bezugspunkt(&kreis, GripKind::Vert(0)), None);
    }
}

#[cfg(test)]
mod tests_raumstempel {
    use super::*;

    fn stempel() -> Entity {
        Entity::new(
            5,
            1,
            EntityKind::Room {
                boundary: vec![
                    V2::new(0.0, 0.0),
                    V2::new(4000.0, 0.0),
                    V2::new(4000.0, 3000.0),
                    V2::new(0.0, 3000.0),
                ],
                label: V2::new(2000.0, 1500.0),
                height: 250.0,
                heading: "Zimmer".into(),
            },
        )
    }

    fn teile(e: &Entity) -> (Vec<V2>, V2) {
        match &e.kind {
            EntityKind::Room { boundary, label, .. } => (boundary.clone(), *label),
            _ => panic!("kein Raumstempel"),
        }
    }

    /// Vier Eckgriffe plus der Textgriff — die Fläche selbst ist unsichtbar,
    /// ihre Ecken erscheinen mit der Auswahl.
    #[test]
    fn raumstempel_hat_eck_und_textgriffe() {
        let griffe = grips_of(&stempel());
        assert_eq!(griffe.len(), 5, "vier Ecken und ein Textgriff");
        assert_eq!(
            griffe.iter().filter(|(k, _)| *k == GripKind::Label).count(),
            1
        );
        assert!(griffe.contains(&(GripKind::Label, V2::new(2000.0, 1500.0))));
    }

    /// Der Textgriff verschiebt nur die Beschriftung — die Umgrenzung bleibt
    /// unverändert (die Fläche wandert nicht mit).
    #[test]
    fn textgriff_verschiebt_nur_die_beschriftung() {
        let e = stempel();
        let (vorher, _) = teile(&e);
        let neu = apply_grip(&e, GripKind::Label, V2::new(500.0, 2500.0)).expect("Griff");
        let (nachher, label) = teile(&neu);
        assert_eq!(vorher, nachher, "die Fläche ist mitgewandert");
        assert_eq!(label, V2::new(500.0, 2500.0));
    }

    /// Eine Ecke ziehen ändert die Fläche; nur dieser Punkt wandert, der Text
    /// bleibt an seiner Stelle (der Inhalt rechnet sich neu).
    #[test]
    fn ecke_ziehen_aendert_nur_diesen_punkt() {
        let e = stempel();
        let neu = apply_grip(&e, GripKind::Vert(2), V2::new(6000.0, 4000.0)).expect("Griff");
        let (rand, label) = teile(&neu);
        assert_eq!(rand[2], V2::new(6000.0, 4000.0));
        assert_eq!(rand[0], V2::new(0.0, 0.0));
        assert_eq!(rand[1], V2::new(4000.0, 0.0));
        assert_eq!(rand[3], V2::new(0.0, 3000.0));
        assert_eq!(label, V2::new(2000.0, 1500.0), "Textlage bleibt stehen");
    }

}

#[cfg(test)]
mod tests_treppe {
    use super::*;

    fn treppe() -> Entity {
        Entity::new(
            3,
            1,
            EntityKind::Stair {
                a: V2::new(0.0, 0.0),
                b: V2::new(0.0, 4000.0),
                width: 1000.0,
                steps: 16,
                height: 2750.0,
                auto_steps: true,
                cut: true,
                unten: None,
                oben: None,
            },
        )
    }

    fn masse(e: &Entity) -> (V2, V2, f64) {
        match &e.kind {
            EntityKind::Stair { a, b, width, .. } => (*a, *b, *width),
            _ => panic!("keine Treppe"),
        }
    }

    /// Vier Kantengriffe auf dem Rechteck (nicht auf der Lauflinie).
    #[test]
    fn treppe_hat_kantengriffe() {
        let griffe = grips_of(&treppe());
        assert_eq!(griffe.len(), 4, "vier Kantenmitten");
        for (art, p) in griffe {
            assert!(matches!(art, GripKind::Edge(_)));
            // Rechteck 1000 breit, Lauflinie bei x = 0: die Längskanten liegen
            // bei x = ±500, die Querkanten bei y = 0 und y = 4000.
            assert!(
                (p.x.abs() - 500.0).abs() < 0.01 || p.y.abs() < 0.01 || (p.y - 4000.0).abs() < 0.01,
                "Griff {p:?} liegt nicht auf der Kontur"
            );
        }
    }

    /// Eine Längskante ziehen verschiebt nur diese Kante: die gegenüberliegende
    /// bleibt stehen, Breite und Lauflinie folgen der halben Verschiebung.
    #[test]
    fn laengskante_verschiebt_parallel() {
        let e = treppe();
        // Kante 0 liegt auf x = -500 (Normale zeigt nach -x); 200 weiter nach
        // außen ziehen.
        let neu = apply_grip(&e, GripKind::Edge(0), V2::new(-700.0, 2000.0)).unwrap();
        let (a, b, w) = masse(&neu);
        assert!((w - 1200.0).abs() < 0.01, "Breite {w}");
        assert!((a.x + 100.0).abs() < 0.01, "Lauflinie wandert 100 nach links: {a:?}");
        assert!((b.x + 100.0).abs() < 0.01, "{b:?}");
        assert!((a.y).abs() < 0.01 && (b.y - 4000.0).abs() < 0.01, "Länge bleibt");
    }

    /// Die Endkante zieht das Laufende mit — die Länge ist das Rechteckmaß.
    #[test]
    fn endkante_veraendert_die_laenge() {
        let e = treppe();
        let neu = apply_grip(&e, GripKind::Edge(1), V2::new(0.0, 3000.0)).unwrap();
        let (a, b, w) = masse(&neu);
        assert!((a.y).abs() < 0.01, "Anfang bleibt stehen");
        assert!((b.y - 3000.0).abs() < 0.01, "Ende bei 3,00 m: {b:?}");
        assert!((w - 1000.0).abs() < 0.01, "Breite bleibt");
    }

    /// Keine Kante darf die Treppe auf null schrumpfen.
    #[test]
    fn kantenziehen_haelt_die_mindestbreite() {
        let e = treppe();
        let neu = apply_grip(&e, GripKind::Edge(0), V2::new(900.0, 2000.0)).unwrap();
        let (_, _, w) = masse(&neu);
        assert!(w >= 99.9, "Breite {w} fällt unter das Minimum");
    }
}

mod tests_volumen {
    use super::*;

    /// Volumen: dieselben Eckpunkte wie eine Füllfläche — sie müssen als
    /// Griffe erscheinen und sich ziehen lassen.
    fn volumen() -> Entity {
        Entity::new(
            7,
            1,
            EntityKind::Volume {
                boundary: vec![
                    V2::new(0.0, 0.0),
                    V2::new(1000.0, 0.0),
                    V2::new(1000.0, 800.0),
                    V2::new(0.0, 800.0),
                ],
                holes: Vec::new(),
                pattern: jjcad_core::style::FillPattern::Solid,
                pattern_angle: 0.0,
                pattern_scale: 100.0,
                border: true,
                height: 2750.0,
            },
        )
    }

    #[test]
    fn volumen_hat_eckpunkte_als_griffe() {
        let e = volumen();
        let g = grips_of(&e);
        // Vier Ecken plus vier Kantenmitten — das Volumen ist ein echtes
        // Rechteck (T0131).
        assert_eq!(g.len(), 8, "vier Ecken und vier Kantenmitten, war {g:?}");
        assert!(g.iter().any(|(_, p)| p.dist(V2::new(1000.0, 800.0)) < 0.5));
        assert!(g.iter().any(|(k, p)| *k == GripKind::Edge(0)
            && p.dist(V2::new(500.0, 0.0)) < 0.5));
    }

    #[test]
    fn volumen_eckpunkt_laesst_sich_ziehen() {
        let e = volumen();
        let neu = apply_grip(&e, GripKind::Vert(2), V2::new(1400.0, 900.0)).expect("kein Zug");
        match &neu.kind {
            EntityKind::Volume { boundary, height, .. } => {
                assert!(boundary[2].dist(V2::new(1400.0, 900.0)) < 0.5, "Ecke nicht gezogen");
                assert_eq!(*height, 2750.0, "Höhe darf sich nicht ändern");
            }
            _ => panic!("kein Volumen mehr"),
        }
    }
}

#[cfg(test)]
mod tests_volumen_drag {
    use super::*;

    /// Der komplette Griffzug an einem Volumen: Griff treffen, ziehen,
    /// loslassen — die Ecke sitzt danach am Ziel, die Höhe bleibt.
    #[test]
    fn volumen_griffzug_von_anfang_bis_ende() {
        let mut app = crate::app::App::with_ctx(egui::Context::default());
        let id = app
            .session_mut()
            .execute(
                "draw.volume",
                &serde_json::json!({
                    "boundary": [[0.0, 0.0], [1.0, 0.0], [1.0, 0.8], [0.0, 0.8]],
                    "height": 2.75,
                }),
            )
            .unwrap()["created"][0]
            .as_u64()
            .unwrap();
        app.session_mut().select(vec![id]);

        // Griff an der Ecke (1000, 800) treffen und ziehen
        let (gid, kind) = app
            .grip_at(V2::new(1000.0, 800.0), 5.0)
            .expect("kein Griff an der Ecke");
        assert_eq!(gid, id);
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(1400.0, 900.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Volume { boundary, height, .. } => {
                assert!(
                    boundary.iter().any(|p| p.dist(V2::new(1400.0, 900.0)) < 0.5),
                    "Ecke nicht am Ziel: {boundary:?}"
                );
                assert_eq!(*height, 2750.0);
            }
            _ => panic!("kein Volumen mehr"),
        }
    }

}

#[cfg(test)]
mod tests_masskette {
    use super::*;
    use jjcad_core::model::DimAxis;

    /// Dreipunktige Maßkette auf der Waagerechten, Maßlinie 400 mm darunter.
    fn kette() -> Entity {
        Entity::new(
            7,
            1,
            EntityKind::Dim {
                pts: vec![
                    V2::new(0.0, 0.0),
                    V2::new(2000.0, 0.0),
                    V2::new(5000.0, 0.0),
                ],
                line_pos: V2::new(1000.0, -400.0),
                angle: 0.0,
                override_text: None,
                ext_lines: false,
                symbol: jjcad_core::model::DIM_SYMBOL_VORGABE.to_string(),
            },
        )
    }

    /// Jeder Basispunkt **und** die Maßlinie sind Griffe — vorher hatte eine
    /// Maßkette gar keinen (T0055).
    #[test]
    fn jeder_basispunkt_und_die_masslinie_haben_einen_griff() {
        let e = kette();
        let griffe = grips_of(&e);
        assert_eq!(
            griffe,
            vec![
                (GripKind::Vert(0), V2::new(0.0, 0.0)),
                (GripKind::Vert(1), V2::new(2000.0, 0.0)),
                (GripKind::Vert(2), V2::new(5000.0, 0.0)),
                // Mitte der gezeichneten Maßlinie
                (GripKind::Edge(0), V2::new(2500.0, -400.0)),
            ]
        );
    }

    /// Ein Basispunkt folgt dem Zeiger, die übrigen Punkte und die Maßlinie
    /// bleiben stehen.
    #[test]
    fn basispunkt_zieht_nur_sich_selbst() {
        let neu = apply_grip(&kette(), GripKind::Vert(1), V2::new(2600.0, 300.0)).unwrap();
        match &neu.kind {
            EntityKind::Dim { pts, line_pos, .. } => {
                assert_eq!(pts[1], V2::new(2600.0, 300.0));
                assert_eq!(pts[0], V2::new(0.0, 0.0));
                assert_eq!(pts[2], V2::new(5000.0, 0.0));
                assert_eq!(*line_pos, V2::new(1000.0, -400.0), "Maßlinie wandert mit");
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Ein Messpunkt, der zwischen seine Nachbarn gezogen wird, wandert in der
    /// Kette an die richtige Stelle (T0195): die Maßzahlen gehören danach zu
    /// den neuen Nachbarn, nicht zu den alten.
    #[test]
    fn gezogener_punkt_wird_einsortiert() {
        // Kette 0 / 2,00 m / 5,00 m; der letzte Punkt auf 0,70 m gezogen.
        let neu = apply_grip(&kette(), GripKind::Vert(2), V2::new(700.0, 0.0)).unwrap();
        match &neu.kind {
            EntityKind::Dim { pts, .. } => assert_eq!(
                pts,
                &vec![
                    V2::new(0.0, 0.0),
                    V2::new(700.0, 0.0),
                    V2::new(2000.0, 0.0)
                ],
                "Messpunkte nicht in Messreihenfolge"
            ),
            k => panic!("keine Maßkette mehr: {k:?}"),
        }

        // Die Anzeige schreibt die Maße der neuen Nachbarstrecken: 0,70 m und
        // 1,30 m — nicht 2,00 m und 1,30 m gegen den alten Nachbarn.
        let mut doc = jjcad_core::model::Document::new();
        doc.add_entity(neu.clone());
        let texte: Vec<String> = jjcad_core::render::render_document(&jjcad_core::render::RenderCtx::new(&doc))
            .prims
            .iter()
            .filter_map(|p| match &p.shape {
                jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texte, vec!["70".to_string(), "1,30".to_string()]);
    }

    /// Die Raute zieht nur die Maßlinie — senkrecht zur Messrichtung, die
    /// Basispunkte bleiben.
    #[test]
    fn masslinie_wandert_senkrecht_ohne_basispunkte() {
        let neu = apply_grip(&kette(), GripKind::Edge(0), V2::new(3000.0, 700.0)).unwrap();
        match &neu.kind {
            EntityKind::Dim { pts, line_pos, .. } => {
                assert_eq!(
                    pts,
                    &vec![
                        V2::new(0.0, 0.0),
                        V2::new(2000.0, 0.0),
                        V2::new(5000.0, 0.0)
                    ]
                );
                // nur die Höhe zählt, die waagerechte Lage bleibt
                assert!((line_pos.y - 700.0).abs() < 1e-9, "{line_pos:?}");
                assert!((line_pos.x - 1000.0).abs() < 1e-9, "{line_pos:?}");
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }


    /// Ortho und Polar richten sich am Nachbarpunkt der Kette aus.
    #[test]
    fn bezug_ist_der_nachbarpunkt() {
        assert_eq!(
            bezugspunkt(&kette(), GripKind::Vert(1)),
            Some(V2::new(0.0, 0.0))
        );
        assert_eq!(bezugspunkt(&kette(), GripKind::Vert(0)), Some(V2::new(2000.0, 0.0)));
        // Die Maßlinie bewegt sich eindimensional — kein Bezug.
        assert_eq!(bezugspunkt(&kette(), GripKind::Edge(0)), None);
    }

    /// Der Griff unter dem Zeiger wird gefunden, und der Zug landet im
    /// **Dokument** (gespeicherter Basispunkt) — der Weg der Zeichenfläche.
    #[test]
    fn mauszug_bewegt_den_basispunkt() {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [5000.0, 0.0]],
                                    "line_pos": [1000.0, -400.0], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        // Griffe gibt es nur in der Auswahl (`App::grip_at`).
        app.session_mut().selection = vec![id];

        let (gid, kind) = app
            .grip_at(V2::new(2000.0, 0.0), 5.0)
            .expect("kein Griff am Basispunkt");
        assert_eq!(gid, id);
        assert_eq!(kind, GripKind::Vert(1));
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(2600.0, 300.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Dim { pts, .. } => {
                assert_eq!(pts[1], V2::new(2600.0, 300.0), "{pts:?}");
                assert_eq!(pts[0], V2::new(0.0, 0.0));
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Zwei Messpunkte übereinandergelegt: einer fällt weg (T0198). Die Kette
    /// misst danach die Strecke zwischen den verbliebenen Punkten.
    #[test]
    fn messpunkt_auf_dem_nachbarn_wird_entfernt() {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [5000.0, 0.0]],
                                    "line_pos": [1000.0, -400.0], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        app.session_mut().selection = vec![id];

        let (gid, kind) = app
            .grip_at(V2::new(2000.0, 0.0), 5.0)
            .expect("kein Griff am Messpunkt");
        assert_eq!(kind, GripKind::Vert(1));
        app.begin_grip_drag(gid, kind);
        // Auf den dritten Messpunkt gezogen — der Fang liefert ihn exakt.
        assert!(app.update_grip_drag(V2::new(5000.0, 0.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Dim { pts, .. } => assert_eq!(
                pts,
                &vec![V2::new(0.0, 0.0), V2::new(5000.0, 0.0)],
                "doppelter Messpunkt steht noch in der Kette: {pts:?}"
            ),
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Der gezogene Punkt darf auch **über** seine Nachbarn hinweg auf einen
    /// weiter entfernten Punkt gelegt werden: die Kette wird beim Ziehen neu
    /// sortiert, der Griffindex zeigt danach auf einen anderen Punkt (T0198).
    #[test]
    fn messpunkt_auf_weiter_entferntem_punkt_wird_entfernt() {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [5000.0, 0.0]],
                                    "line_pos": [1000.0, -400.0], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        app.session_mut().selection = vec![id];

        let (gid, kind) = app
            .grip_at(V2::new(5000.0, 0.0), 5.0)
            .expect("kein Griff am Messpunkt");
        assert_eq!(kind, GripKind::Vert(2));
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(0.0, 0.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Dim { pts, .. } => assert_eq!(
                pts,
                &vec![V2::new(0.0, 0.0), V2::new(2000.0, 0.0)],
                "doppelter Messpunkt steht noch in der Kette: {pts:?}"
            ),
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Ohne Deckung bleibt die Kette unverändert — ein Zug in die freie Fläche
    /// entfernt keinen Messpunkt.
    #[test]
    fn messpunkt_ohne_deckung_bleibt_stehen() {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [5000.0, 0.0]],
                                    "line_pos": [1000.0, -400.0], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        app.session_mut().selection = vec![id];

        let (gid, kind) = app
            .grip_at(V2::new(2000.0, 0.0), 5.0)
            .expect("kein Griff am Messpunkt");
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(3000.0, 800.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Dim { pts, .. } => assert_eq!(pts.len(), 3, "{pts:?}"),
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Ein Einzelmaß (zwei Messpunkte) behält beide: ohne Messpunkte gäbe es
    /// kein Maß mehr (T0198).
    #[test]
    fn einzelmass_behaelt_seine_zwei_punkte() {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0]],
                                    "line_pos": [1000.0, -400.0], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        app.session_mut().selection = vec![id];

        let (gid, kind) = app
            .grip_at(V2::new(2000.0, 0.0), 5.0)
            .expect("kein Griff am Messpunkt");
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(0.0, 0.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Dim { pts, .. } => assert_eq!(pts.len(), 2, "{pts:?}"),
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Dasselbe für die Raute an der Maßlinie: sie verschiebt nur die Linie.
    #[test]
    fn mauszug_bewegt_die_masslinie() {
        let mut app = App::with_ctx(egui::Context::default());
        app.doc_mut().settings.input_unit = jjcad_core::units::Unit::Mm;
        app.session_mut()
            .execute(
                "dim.chain",
                &serde_json::json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [5000.0, 0.0]],
                                    "line_pos": [1000.0, -400.0], "axis": "x"}),
            )
            .expect("Maßkette");
        let id = app.doc().ents().last().map(|e| e.id).expect("Maßkette");
        app.session_mut().selection = vec![id];

        // Die Mitte der Maßlinie liegt bei (2500, -400).
        let (gid, kind) = app
            .grip_at(V2::new(2500.0, -400.0), 5.0)
            .expect("kein Griff an der Maßlinie");
        assert_eq!((gid, kind), (id, GripKind::Edge(0)));
        app.begin_grip_drag(gid, kind);
        assert!(app.update_grip_drag(V2::new(2500.0, 700.0)), "Zug kam nicht an");
        app.end_grip_drag();

        match &app.doc().entity(id).unwrap().kind {
            EntityKind::Dim { pts, line_pos, .. } => {
                assert!((line_pos.y - 700.0).abs() < 1e-9, "{line_pos:?}");
                assert_eq!(pts[1], V2::new(2000.0, 0.0), "Basispunkt wandert mit");
            }
            k => panic!("keine Maßkette mehr: {k:?}"),
        }
    }

    /// Ein Einzelmaß hat dieselben Griffe.
    #[test]
    fn einzelmass_hat_basispunkte_und_masslinie() {
        let e = Entity::new(
            8,
            1,
            EntityKind::Dim {
                pts: vec![V2::new(0.0, 0.0), V2::new(3000.0, 0.0)],
                line_pos: V2::new(1500.0, -300.0),
                angle: 0.0,
                override_text: None,
                ext_lines: false,
                symbol: jjcad_core::model::DIM_SYMBOL_VORGABE.to_string(),
            },
        );
        let griffe = grips_of(&e);
        assert_eq!(
            griffe,
            vec![
                (GripKind::Vert(0), V2::new(0.0, 0.0)),
                (GripKind::Vert(1), V2::new(3000.0, 0.0)),
                (GripKind::Edge(0), V2::new(1500.0, -300.0)),
            ]
        );
        let neu = apply_grip(&e, GripKind::Vert(1), V2::new(3500.0, 0.0)).unwrap();
        match &neu.kind {
            EntityKind::Dim { pts, .. } => assert_eq!(pts[1], V2::new(3500.0, 0.0)),
            k => panic!("kein Maß mehr: {k:?}"),
        }
    }
}

#[cfg(test)]
mod tests_ansichtsfenster {
    use super::*;

    fn fenster() -> Entity {
        Entity::new(
            7,
            1,
            EntityKind::Viewport(jjcad_core::model::Viewport {
                id: 7,
                name: "Ansicht".into(),
                x: 15.0,
                y: 83.0,
                w: 380.0,
                h: 200.0,
                center: V2::ZERO,
                scale: 100.0,
                show_frame: true,
                locked: false,
                layers: None,
            }),
        )
    }

    /// Vier Eckgriffe wie bei einem Rechteck (T0109).
    #[test]
    fn ansichtsfenster_hat_vier_eckgriffe() {
        let griffe = grips_of(&fenster());
        assert_eq!(
            griffe,
            vec![
                (GripKind::Vert(0), V2::new(15.0, 83.0)),
                (GripKind::Vert(1), V2::new(395.0, 83.0)),
                (GripKind::Vert(2), V2::new(395.0, 283.0)),
                (GripKind::Vert(3), V2::new(15.0, 283.0)),
            ]
        );
    }

    /// Eine gezogene Ecke spannt das Fenster neu auf, die gegenüberliegende
    /// bleibt stehen — wie beim Rechteck.
    #[test]
    fn ecke_zieht_das_fenster_auf() {
        let neu = apply_grip(&fenster(), GripKind::Vert(2), V2::new(500.0, 300.0)).unwrap();
        match &neu.kind {
            EntityKind::Viewport(vp) => {
                assert_eq!((vp.x, vp.y, vp.w, vp.h), (15.0, 83.0, 485.0, 217.0));
                // Maßstab und Modellmitte bleiben.
                assert_eq!(vp.scale, 100.0);
                assert_eq!(vp.center, V2::ZERO);
            }
            k => panic!("kein Ansichtsfenster mehr: {k:?}"),
        }
    }

    /// Eine entartete Kante wird nicht übernommen — sonst wäre das Fenster
    /// nicht mehr zu fassen.
    #[test]
    fn entartete_kante_wird_verworfen() {
        assert!(apply_grip(&fenster(), GripKind::Vert(2), V2::new(20.0, 90.0)).is_none());
    }
}

#[cfg(test)]
mod tests_rechteck_polylinie {
    //! Kantengriffe der Polylinie (T0133): Das Rechteck-Werkzeug legt eine
    //! geschlossene Polylinie an; ihre vier Kantenmitten verschieben eine Kante
    //! parallel. Sobald die Form kein Rechteck mehr ist, verschwinden sie —
    //! dieselbe Regel wie bei der rechteckigen Fläche (T0131).

    use super::*;
    use jjcad_core::geom::Vertex;

    fn zug(pts: &[V2], closed: bool) -> Entity {
        Entity::new(
            1,
            1,
            EntityKind::Polyline {
                verts: pts.iter().map(|p| Vertex::new(*p)).collect(),
                closed,
            },
        )
    }

    fn rechteck() -> Entity {
        zug(
            &[
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            true,
        )
    }

    fn kanten(e: &Entity) -> Vec<(usize, V2)> {
        grips_of(e)
            .into_iter()
            .filter_map(|(k, p)| match k {
                GripKind::Edge(i) => Some((i, p)),
                _ => None,
            })
            .collect()
    }

    fn stand(e: &Entity) -> Vec<V2> {
        match &e.kind {
            EntityKind::Polyline { verts, .. } => verts.iter().map(|v| v.p).collect(),
            _ => panic!("keine Polylinie"),
        }
    }

    /// Das Rechteck aus dem Rechteck-Werkzeug hat vier Kantenmitten, jede mit
    /// Richtung (ohne sie bliebe der Griff unsichtbar aber klickbar).
    #[test]
    fn rechteck_hat_vier_kantengriffe() {
        let e = rechteck();
        let k = kanten(&e);
        assert_eq!(k.len(), 4, "vier Kantenmitten: {k:?}");
        let mitten: Vec<V2> = k.iter().map(|(_, p)| *p).collect();
        assert!(mitten.contains(&V2::new(2000.0, 0.0)), "unten: {mitten:?}");
        assert!(mitten.contains(&V2::new(4000.0, 1500.0)), "rechts: {mitten:?}");
        assert!(mitten.contains(&V2::new(2000.0, 3000.0)), "oben: {mitten:?}");
        assert!(mitten.contains(&V2::new(0.0, 1500.0)), "links: {mitten:?}");
        for (i, _) in k {
            assert!(edge_dir(&e, i).is_some(), "Kante {i} ohne Richtung");
        }
        // Die vier Ecken sind weiterhin Griffe.
        assert_eq!(
            grips_of(&e)
                .iter()
                .filter(|(k, _)| matches!(k, GripKind::Vert(_)))
                .count(),
            4
        );
    }

    /// Ein gedrehtes Rechteck (Einfügemodus „Basislinie + Höhe") ist genauso
    /// ein Rechteck — rechte Winkel genügen, achsenparallel muss es nicht sein.
    #[test]
    fn gedrehtes_rechteck_hat_kantengriffe() {
        let e = zug(
            &[
                V2::new(0.0, 0.0),
                V2::new(2000.0, 2000.0),
                V2::new(1000.0, 3000.0),
                V2::new(-1000.0, 1000.0),
            ],
            true,
        );
        assert_eq!(kanten(&e).len(), 4, "gedrehtes Rechteck: {:?}", grips_of(&e));
    }

    /// Kein Rechteck, keine Kantengriffe: Dreieck, L-Form und schiefes Viereck.
    #[test]
    fn formen_ohne_rechte_winkel_haben_keine_kantengriffe() {
        let dreieck = zug(
            &[
                V2::new(0.0, 0.0),
                V2::new(3000.0, 0.0),
                V2::new(0.0, 2000.0),
            ],
            true,
        );
        let l_form = zug(
            &[
                V2::new(0.0, 0.0),
                V2::new(3000.0, 0.0),
                V2::new(3000.0, 1000.0),
                V2::new(1000.0, 1000.0),
                V2::new(1000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            true,
        );
        let schief = zug(
            &[
                V2::new(0.0, 0.0),
                V2::new(3000.0, 500.0),
                V2::new(2500.0, 2500.0),
                V2::new(500.0, 2000.0),
            ],
            true,
        );
        for (name, e) in [("Dreieck", dreieck), ("L-Form", l_form), ("schief", schief)] {
            assert!(
                kanten(&e).is_empty(),
                "{name} ist kein Rechteck, hat aber Kantengriffe: {:?}",
                grips_of(&e)
            );
        }
    }

    /// Offener Zug: keine Kantengriffe, auch wenn die Punkte ein Rechteck
    /// beschreiben — ohne geschlossenen Ring gibt es keine Gegenkante.
    #[test]
    fn offener_zug_hat_keine_kantengriffe() {
        let e = zug(
            &[
                V2::new(0.0, 0.0),
                V2::new(4000.0, 0.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ],
            false,
        );
        assert!(kanten(&e).is_empty(), "{:?}", grips_of(&e));
    }

    /// Der Kern des Tickets: eine Ecke schräg ziehen macht aus dem Rechteck
    /// kein Rechteck mehr — die Kantengriffe müssen verschwinden, und ein
    /// stehengebliebener Kantenzug darf nichts mehr bewegen.
    #[test]
    fn gezogene_ecke_nimmt_die_kantengriffe_weg() {
        let schief = apply_grip(&rechteck(), GripKind::Vert(1), V2::new(4500.0, 800.0))
            .expect("Eckgriff muss greifen");
        assert!(
            kanten(&schief).is_empty(),
            "kein Rechteck mehr, aber Kantengriffe: {:?}",
            grips_of(&schief)
        );
        assert!(apply_grip(&schief, GripKind::Edge(0), V2::new(1000.0, -500.0)).is_none());
    }

    /// Eine Kante ziehen verschiebt **nur** diese: die Gegenkante bleibt
    /// stehen, das Rechteck wird höher.
    #[test]
    fn kante_schieben_laesst_die_gegenkante_stehen() {
        let neu = apply_grip_tol(&rechteck(), GripKind::Edge(0), V2::new(2000.0, -500.0), 1.0)
            .expect("Kantengriff muss greifen");
        assert_eq!(
            stand(&neu),
            vec![
                V2::new(0.0, -500.0),
                V2::new(4000.0, -500.0),
                V2::new(4000.0, 3000.0),
                V2::new(0.0, 3000.0),
            ]
        );
        // Hinterher ist es weiter ein Rechteck: die Griffe bleiben.
        assert_eq!(kanten(&neu).len(), 4);
    }


    /// Die Kante lässt sich nicht über die Gegenkante hinausziehen — sonst
    /// kehrte sich der Ring um.
    #[test]
    fn kante_haelt_die_mindestbreite() {
        let neu = apply_grip(&rechteck(), GripKind::Edge(0), V2::new(2000.0, 5000.0)).unwrap();
        let pts = stand(&neu);
        let unterkante = pts[0].y.min(pts[1].y);
        assert!(
            unterkante >= 3000.0 - 1.0 - 1e-6,
            "Kante lief über die Gegenkante: {pts:?}"
        );
    }

    /// Ein geschlossener Spline ist keine Polylinie: keine Kantengriffe.
    #[test]
    fn spline_hat_keine_kantengriffe() {
        let e = Entity::new(
            1,
            1,
            EntityKind::Spline {
                verts: [
                    V2::new(0.0, 0.0),
                    V2::new(4000.0, 0.0),
                    V2::new(4000.0, 3000.0),
                    V2::new(0.0, 3000.0),
                ]
                .iter()
                .map(|p| Vertex::new(*p))
                .collect(),
                closed: true,
            },
        );
        assert!(kanten(&e).is_empty(), "{:?}", grips_of(&e));
    }
}
