//! Fang-Engine: Endpunkt, Mittelpunkt, Zentrum, Schnittpunkt, Lot, Nächster Punkt, Raster.

use crate::geom::*;
use crate::model::*;
use crate::render;
use crate::style::LineType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Endpoint,
    /// Echte Eckpunkte des Blatts im Papierraum (0,0) … (B,H).
    SheetCorner,
    /// Basispunkt des Blocks im Blockeditor (Weltursprung, T0208).
    BlockBase,
    Intersection,
    Midpoint,
    Center,
    Quadrant,
    Perpendicular,
    Nearest,
    Grid,
    /// Punkt auf einer Hilfslinie des erweiterten Punktfangs.
    Tracking,
    /// Punkt auf der Winkelhalbierenden zwischen zwei Linien (T0150).
    Bisector,
    /// Maßlinie parallel zu einer vorhandenen Maßkette im festen Abstand
    /// (T0154) — nur beim Positionieren einer Maßkette.
    DimParallel,
    /// Tangentialpunkt (T0157): die Linie verlässt einen Kreis/einen Bogen
    /// tangential, oder ein bewegter Kreis berührt eine Linie.
    Tangent,
}

impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "Endpunkt",
            SnapKind::SheetCorner => "Blattecke",
            SnapKind::BlockBase => "Basispunkt",
            SnapKind::Intersection => "Schnittpunkt",
            SnapKind::Midpoint => "Mittelpunkt",
            SnapKind::Center => "Zentrum",
            SnapKind::Quadrant => "Quadrant",
            SnapKind::Perpendicular => "Lot",
            SnapKind::Nearest => "Nächster Punkt",
            SnapKind::Grid => "Raster",
            SnapKind::Tracking => "Hilfslinie",
            SnapKind::Bisector => "Winkelhalbierende",
            SnapKind::DimParallel => "Maßkette parallel",
            SnapKind::Tangent => "Tangential",
        }
    }
    /// Beschriftung in der gewählten Sprache (T0106). `label()` bleibt der
    /// deutsche Name — Dateien und Action-Parameter dürfen sich nicht ändern.
    pub fn text(self) -> &'static str {
        match self {
            SnapKind::Endpoint => crate::i18n::tr("ui.snapkind.endpoint"),
            SnapKind::SheetCorner => crate::i18n::tr("ui.snapkind.sheet_corner"),
            SnapKind::BlockBase => crate::i18n::tr("ui.snapkind.block_base"),
            SnapKind::Intersection => crate::i18n::tr("ui.snapkind.intersection"),
            SnapKind::Midpoint => crate::i18n::tr("ui.snapkind.midpoint"),
            SnapKind::Center => crate::i18n::tr("ui.snapkind.center"),
            SnapKind::Quadrant => crate::i18n::tr("ui.snapkind.quadrant"),
            SnapKind::Perpendicular => crate::i18n::tr("ui.snapkind.perpendicular"),
            SnapKind::Nearest => crate::i18n::tr("ui.snapkind.nearest"),
            SnapKind::Grid => crate::i18n::tr("ui.snapkind.grid"),
            SnapKind::Tracking => crate::i18n::tr("ui.snapkind.tracking"),
            SnapKind::Bisector => crate::i18n::tr("ui.snapkind.bisector"),
            SnapKind::DimParallel => crate::i18n::tr("ui.snapkind.dim_parallel"),
            SnapKind::Tangent => crate::i18n::tr("ui.snapkind.tangent"),
        }
    }


    pub fn short(self) -> &'static str {
        match self {
            SnapKind::Endpoint => "END",
            SnapKind::SheetCorner => "ECK",
            SnapKind::BlockBase => "BP",
            SnapKind::Intersection => "SCHN",
            SnapKind::Midpoint => "MIT",
            SnapKind::Center => "ZEN",
            SnapKind::Quadrant => "QUA",
            SnapKind::Perpendicular => "LOT",
            SnapKind::Nearest => "NAE",
            SnapKind::Grid => "RAS",
            SnapKind::Tracking => "HLF",
            SnapKind::Bisector => "WHB",
            SnapKind::DimParallel => "MPAR",
            SnapKind::Tangent => "TAN",
        }
    }

    /// Ein Punkt, der fest an der Geometrie hängt (Ecke, Mitte, Zentrum,
    /// Quadrant, Schnitt, Blattecke). Nur solche Punkte taugen als Haltepunkt
    /// des erweiterten Punktfangs: „Nächster Punkt" wandert mit dem Zeiger,
    /// das Raster liegt überall — beide stünden nie still.
    pub fn ist_diskret(self) -> bool {
        matches!(
            self,
            SnapKind::Endpoint
                | SnapKind::SheetCorner
                | SnapKind::BlockBase
                | SnapKind::Intersection
                | SnapKind::Midpoint
                | SnapKind::Center
                | SnapKind::Quadrant
        )
    }

    fn priority(self) -> u8 {
        match self {
            SnapKind::Endpoint => 0,
            // Die Blattecke ist ein Endpunkt des Papiers — gleichrangig.
            SnapKind::SheetCorner => 0,
            // Der Basispunkt ist der Einfügepunkt des Blocks — gleichrangig.
            SnapKind::BlockBase => 0,
            SnapKind::Intersection => 1,
            SnapKind::Midpoint => 2,
            SnapKind::Center => 3,
            SnapKind::Quadrant => 4,
            // Die Winkelhalbierende ist ein ausdrücklich anvisierter Punkt
            // zwischen zwei Linien — sie schlägt „Nächster Punkt" und Lot,
            // aber nie einen echten Endpunkt oder Schnittpunkt (T0150).
            SnapKind::Bisector => 5,
            // Ausdrücklich anvisierte Maßlinienlage: schlägt Raster und
            // „Nächster Punkt“, aber keinen echten Objektpunkt.
            SnapKind::DimParallel => 5,
            // Der Tangentialpunkt ist wie Lot und Winkelhalbierende ein
            // abgeleiteter Punkt: er schlägt „Nächster Punkt" und das Raster,
            // aber nie einen echten Objektpunkt (T0157).
            SnapKind::Tangent => 5,
            SnapKind::Perpendicular => 6,
            SnapKind::Nearest => 7,
            SnapKind::Grid => 8,
            // Hilfslinien entstehen nicht im Kandidaten-Sammellauf, sondern in
            // `hilfslinien_snap`; der Wert hält nur die Reihenfolge eindeutig.
            SnapKind::Tracking => 9,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SnapHit {
    pub point: Vec2,
    pub kind: SnapKind,
    pub entity: Option<Id>,
    /// Abweichende Stelle für die Fangmarke (T0211): beim Tangentialfang steht
    /// das Zeichen am **Berührpunkt** von Linie und Kreis, während `point` der
    /// Punkt am Zeiger ist (auf der Tangente bzw. der neue Kreismittelpunkt).
    /// `None` heißt: die Marke sitzt auf `point`.
    pub marke: Option<Vec2>,
}

// ---------------------------------------------------------------------------
// Erweiterter Punktfang (Hilfslinien)
// ---------------------------------------------------------------------------

/// Ein festgehaltener Fangpunkt: der Zeiger stand lange genug über ihm, ohne
/// dass geklickt wurde. Von ihm gehen zwei Hilfslinien aus, auf denen der
/// nächste Punkt gefangen werden kann.
#[derive(Clone, Debug, PartialEq)]
pub struct HaltPunkt {
    pub point: Vec2,
    pub kind: SnapKind,
    pub entity: Option<Id>,
    /// Richtung der Linie, zu der der Punkt gehört. `Some` = der Punkt liegt
    /// auf einer Linie (Verlängerung + Normale), `None` = einzelner Punkt
    /// (Waagerechte + Senkrechte). Siehe [`richtung_am_punkt`].
    pub richtung: Option<Vec2>,
}

/// Eine unbegrenzte Hilfslinie: Ursprung (der gehaltene Punkt) und Richtung.
///
/// `kind` sagt, woher sie stammt: die zwei Achsen eines Haltepunkts sind
/// [`SnapKind::Tracking`], die Winkelhalbierende zwischen zwei Linien ist
/// [`SnapKind::Bisector`] — so meldet die Statuszeile den richtigen Namen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hilfslinie {
    pub origin: Vec2,
    pub dir: Vec2,
    pub kind: SnapKind,
}

impl Hilfslinie {
    /// Führungslinie eines Haltepunkts (Verlängerung, Normale, Achsenkreuz).
    fn spur(origin: Vec2, dir: Vec2) -> Self {
        Hilfslinie {
            origin,
            dir,
            kind: SnapKind::Tracking,
        }
    }

    /// Winkelhalbierende zwischen zwei Linien (T0150).
    fn halbierende(origin: Vec2, dir: Vec2) -> Self {
        Hilfslinie {
            origin,
            dir,
            kind: SnapKind::Bisector,
        }
    }
}

/// Zwangsachse des Zeigers: Ursprung ist der Bezugspunkt (der zuletzt gesetzte
/// Punkt), `dir` die Richtung der Achse.
///
/// **Ortho** und **Polar** sind Bedingungen an den Punkt, kein Punktangebot.
/// Genau darin liegt der Unterschied zu einer Hilfslinie: treffen beide
/// zugleich, bleibt die Bedingung bestehen und der Fang wählt nur die Lage auf
/// ihr — siehe [`hilfslinien_snap_innen`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zwangsachse {
    pub origin: Vec2,
    pub dir: Vec2,
}

/// Die Zwangsachse, die der Zeiger gerade benutzt (sofern es einen
/// Bezugspunkt gibt).
///
/// Gebaut aus denselben Regeln, die [`snap_constrained`] für den freien Zeiger
/// anwendet, damit Anzeige und Fang nicht auseinanderlaufen:
///
/// * **Ortho** ist hart — waagerecht oder senkrecht durch den Bezugspunkt, je
///   nachdem welche Richtung der Zeiger überwiegend nimmt.
/// * **Polar** rastet nur innerhalb des Winkel-Fensters **und** des Bandes
///   neben dem Strahl ein (T0148); außerhalb gibt es keine Achse.
pub fn zwangsachse(
    s: &SnapSettings,
    cursor: Vec2,
    reference: Option<Vec2>,
    tol: f64,
) -> Option<Zwangsachse> {
    let r = reference?;
    if s.ortho {
        let d = cursor - r;
        let dir = if d.x.abs() >= d.y.abs() {
            v(1.0, 0.0)
        } else {
            v(0.0, 1.0)
        };
        return Some(Zwangsachse { origin: r, dir });
    }
    if s.polar && cursor.dist(r) > tol * 2.0 {
        if let Some(p) = cursor.polar_snap(r, s.polar_step.max(1.0), s.polar_tolerance, tol) {
            let dir = (p - r).normalized();
            if dir.len() > 1e-9 {
                return Some(Zwangsachse { origin: r, dir });
            }
        }
    }
    None
}

impl HaltPunkt {
    /// Halt aus einem Fangtreffer bauen — die Richtung kommt aus der Geometrie.
    pub fn neu(doc: &Document, hit: &SnapHit) -> Self {
        let richtung = hit
            .entity
            .and_then(|id| richtung_am_punkt(doc, id, hit.point));
        HaltPunkt {
            point: hit.point,
            kind: hit.kind,
            entity: hit.entity,
            richtung,
        }
    }
}

/// Die zwei Hilfslinien eines Haltepunkts.
///
/// * Punkt auf einer Linie: die **Verlängerung** der Linie und die **Normale**
///   dazu (beide durch den Punkt).
/// * Einzelner Punkt (Zentrum, Blattecke, freier Vorschaupunkt): **Waagerechte**
///   und **Senkrechte**.
pub fn hilfslinien(halte: &[HaltPunkt]) -> Vec<Hilfslinie> {
    let mut out = Vec::new();
    for h in halte {
        for dir in richtungen(h) {
            out.push(Hilfslinie::spur(h.point, dir));
        }
    }
    out
}

fn richtungen(h: &HaltPunkt) -> [Vec2; 2] {
    match h.richtung {
        Some(d) if d.len() > 1e-9 => [d, d.perp()],
        _ => [v(1.0, 0.0), v(0.0, 1.0)],
    }
}

// ---------------------------------------------------------------------------
// Winkelhalbierende am Schnittpunkt zweier Linien (T0150)
// ---------------------------------------------------------------------------

/// Toleranz, mit der ein Punkt „auf" einer Linie liegt (mm).
///
/// Der feste Punkt einer neuen Linie entsteht durch Fangen und liegt damit
/// exakt auf der Geometrie; ein frei gesetzter Punkt trifft den Scheitel
/// höchstens zufällig. 1 mm ist streng genug, dass der Fang nur am
/// Schnittpunkt greift, und weit genug, dass eine gerundete Koordinate nicht
/// herausfällt.
pub const WINKELHALBIERENDE_SCHEITEL_MM: f64 = 1.0;

/// Winkelhalbierende an einem Punkt, der auf dem Schnittpunkt zweier Linien
/// liegt.
///
/// **Bedingung** (T0150, Turn 2): `p` muss auf **zwei** Linien liegen — also
/// der Scheitel sein, an dem sich zwei Linien treffen (Ecke, Kreuzung, Stoß).
/// Nur dann gibt es einen Winkel zu halbieren. Liegt der Punkt nur *zwischen*
/// zwei Linien oder neben einer einzelnen, entsteht keine Führungslinie; genau
/// das war am ersten Stand falsch, dort wurde die Halbierende überall
/// angeboten, wo zwei Linien in der Nähe waren.
///
/// Als Linien zählen **Linien und Wände** (deren Achse) — das sind die Linien
/// einer Zeichnung; Bogen, Rechteck und Polylinie sind keine „zwei Linien" und
/// bleiben außen vor. Gelesen wird die Strecke direkt aus dem Modell; über
/// `render::entity_bbox` kostete jeder Aufruf 195 µs (Displayliste je Objekt).
///
/// Geliefert wird je Winkel am Scheitel eine Führungslinie **durch `p`**:
///
/// * **Ecke** (beide Strecken enden im Punkt): die Halbierende des
///   Innenwinkels,
/// * **Kreuzung oder Stoß** (der Punkt liegt im Lauf einer Strecke): beide
///   Halbierenden, denn dort gibt es zwei Winkelpaare.
pub fn winkelhalbierende(doc: &Document, p: Vec2, ignore: &[Id]) -> Vec<Hilfslinie> {
    let tol = WINKELHALBIERENDE_SCHEITEL_MM;
    // Nur Linien, auf denen `p` liegt — der Scheitel liegt auf beiden.
    let mut strecken: Vec<(Vec2, Vec2)> = Vec::new();
    for e in doc.ents() {
        if ignore.contains(&e.id) {
            continue;
        }
        if doc.layer_gesperrt(e.layer) { continue; }
        let (a, b) = match &e.kind {
            EntityKind::Line { a, b } => (*a, *b),
            EntityKind::Wall { a, b, .. } => (*a, *b),
            _ => continue,
        };
        if dist_point_segment(p, a, b) <= tol {
            strecken.push((a, b));
        }
    }
    if strecken.len() < 2 {
        return Vec::new();
    }

    let mut out: Vec<Hilfslinie> = Vec::new();
    for i in 0..strecken.len() {
        for j in (i + 1)..strecken.len() {
            for l in halbierende_am_scheitel(p, strecken[i], strecken[j]) {
                // Dieselbe Halbierende kann aus zwei Paaren entstehen (drei
                // Linien in einem Punkt) — einmal anbieten.
                let doppelt = out
                    .iter()
                    .any(|o| o.dir.cross(l.dir).abs() < 1e-9 && o.origin.dist(l.origin) <= tol);
                if !doppelt {
                    out.push(l);
                }
            }
        }
    }
    out
}

/// Halbierende(n) zweier Strecken, die sich in `p` treffen.
///
/// Die Richtungen werden **von `p` weg** genommen: endet eine Strecke im
/// Punkt, ist das ihre eine Richtung; läuft sie durch ihn hindurch, zählen
/// beide. Die Halbierende ist die Summe zweier solcher Richtungen — zeigen sie
/// auseinander (gestreckter Winkel), gibt es keine.
fn halbierende_am_scheitel(p: Vec2, s1: (Vec2, Vec2), s2: (Vec2, Vec2)) -> Vec<Hilfslinie> {
    let d1 = (s1.1 - s1.0).normalized();
    let d2 = (s2.1 - s2.0).normalized();
    if d1.len() < 0.5 || d2.len() < 0.5 {
        return Vec::new();
    }
    // Parallel oder gleichgerichtet: es gibt keinen Winkel zu halbieren.
    if d1.cross(d2).abs() < 1e-6 {
        return Vec::new();
    }
    let mut out: Vec<Hilfslinie> = Vec::new();
    for r1 in richtungen_ab(p, s1, d1) {
        for r2 in richtungen_ab(p, s2, d2) {
            let summe = r1 + r2;
            if summe.len() < 1e-6 {
                continue;
            }
            let dir = summe.normalized();
            if out.iter().any(|l| l.dir.cross(dir).abs() < 1e-9) {
                continue;
            }
            out.push(Hilfslinie::halbierende(p, dir));
        }
    }
    out
}

/// Richtungen der Strecke `s`, gesehen von ihrem Punkt `p`: eine, wenn der
/// Punkt an einem Ende liegt, sonst beide.
fn richtungen_ab(p: Vec2, s: (Vec2, Vec2), d: Vec2) -> Vec<Vec2> {
    let tol = WINKELHALBIERENDE_SCHEITEL_MM;
    let mut out = Vec::new();
    if p.dist(s.0) > tol {
        out.push(-d);
    }
    if p.dist(s.1) > tol {
        out.push(d);
    }
    out
}

/// Fangpunkte auf den Winkelhalbierenden der genannten Punkte.
///
/// Angeboten wird der Fußpunkt des Zeigers auf einer Halbierenden — genau wie
/// bei einer Hilfslinie des erweiterten Punktfangs, nur ohne deren zweite
/// Achse. Die Punkte sind der Bezugspunkt des laufenden Zuges (der **feste
/// Punkt** der neuen Linie) und die Haltepunkte des erweiterten Punktfangs.
pub fn winkelhalbierende_treffer(
    doc: &Document,
    punkte: &[Vec2],
    cursor: Vec2,
    tol: f64,
    ignore: &[Id],
) -> Vec<SnapHit> {
    let mut out = Vec::new();
    for p in punkte {
        for l in winkelhalbierende(doc, *p, ignore) {
            let f = fuss_auf_gerade(cursor, l.origin, l.dir);
            if f.dist(cursor) <= tol {
                out.push(SnapHit {
                    point: f,
                    kind: SnapKind::Bisector,
                    entity: None,
                    marke: None,
                });
            }
        }
    }
    out
}

/// Alle Führungslinien des erweiterten Punktfangs **und** die
/// Winkelhalbierenden — die eine Liste für die Anzeige.
///
/// Gezeichnet wird damit genau das, was auch fängt: die zwei Achsen je
/// Haltepunkt und die Halbierenden an jedem Haltepunkt bzw. am Bezugspunkt,
/// der auf dem Schnittpunkt zweier Linien liegt.
pub fn hilfslinien_gesamt(
    doc: &Document,
    halte: &[HaltPunkt],
    bezug: Option<Vec2>,
    ignore: &[Id],
) -> Vec<Hilfslinie> {
    let mut out = hilfslinien(halte);
    let mut punkte: Vec<Vec2> = halte.iter().map(|h| h.point).collect();
    if let Some(r) = bezug {
        punkte.push(r);
    }
    for p in punkte {
        for l in winkelhalbierende(doc, p, ignore) {
            // Dieselbe Halbierende kann von zwei Punkten geliefert werden
            // (Bezug und Haltepunkt liegen beide auf demselben Scheitel) —
            // einmal zeichnen.
            let doppelt = out.iter().any(|o| {
                o.kind == l.kind
                    && (o.origin.dist(l.origin) <= SNAP_DEDUP
                        || (o.dir.cross(l.dir).abs() < 1e-9
                            && fuss_auf_gerade(l.origin, o.origin, o.dir).dist(l.origin)
                                <= SNAP_DEDUP))
            });
            if !doppelt {
                out.push(l);
            }
        }
    }
    out
}


/// Richtung der Linie, auf der dieser Punkt des Elements liegt.
///
/// Gebraucht für die Hilfslinien: ein Punkt, der zu einer Linie gehört, soll
/// deren Verlängerung und Normale bekommen, kein starres Achsenkreuz. Geliefert
/// wird die Richtung des ersten Segments, das durch den Punkt läuft (bei Bögen
/// die Tangente). Findet sich keins (Zentrum, Blattecke, Wandkonturecke neben
/// der Achse), bleibt es beim Achsenkreuz.
pub fn richtung_am_punkt(doc: &Document, entity: Id, p: Vec2) -> Option<Vec2> {
    let e = doc.entity(entity)?;
    let mut segs = render::entity_segments(doc, e);
    // Wandkörper: eine Konturecke liegt neben der Achse — ohne die
    // umgrenzenden Linien fände sie keine Richtung und bekäme ein Achsenkreuz
    // statt der Verlängerung ihrer Wand.
    if matches!(e.kind, EntityKind::Wall { .. }) {
        let idx = render::WallIndex::build(doc, doc.ents());
        segs.extend(render::wall_contour_segments_idx(&idx, e));
    }
    for sg in segs {
        match sg {
            Seg::Line(a, b) => {
                if dist_point_segment(p, a, b) <= SNAP_DEDUP.max(0.5) {
                    let d = (b - a).normalized();
                    if d.len() > 1e-9 {
                        return Some(d);
                    }
                }
            }
            Seg::Arc(arc) => {
                // Tangente im Punkt: senkrecht zum Radius.
                let d = (p - arc.c).normalized();
                if d.len() > 1e-9 && (p.dist(arc.c) - arc.r).abs() <= SNAP_DEDUP.max(0.5) {
                    return Some(d.perp());
                }
            }
        }
    }
    None
}

/// Fußpunkt des Lots von `p` auf die **unbegrenzte** Gerade `origin + t·dir`.
fn fuss_auf_gerade(p: Vec2, origin: Vec2, dir: Vec2) -> Vec2 {
    let d = dir.normalized();
    if d.len() <= 1e-9 {
        return origin;
    }
    origin + d * (p - origin).dot(d)
}

/// Element des Haltepunkts, zu dem diese Hilfslinie gehört.
fn halt_entity(halte: &[HaltPunkt], l: &Hilfslinie) -> Option<Id> {
    halte
        .iter()
        .find(|h| h.point.dist(l.origin) <= SNAP_DEDUP)
        .and_then(|h| h.entity)
}

/// Ein Fangpunkt auf einer Hilfslinie: exakt oder nur der Fußpunkt.
struct HilfsTreffer {
    dist: f64,
    point: Vec2,
    kind: SnapKind,
    entity: Option<Id>,
}

/// Fang auf den Hilfslinien der Haltepunkte — **ohne** Objektgeometrie.
///
/// Angeboten werden alle Kreuzungen zweier Hilfslinien (auch der Haltepunkt
/// selbst — dort schneiden sich seine beiden Linien) und alle Fußpunkte des
/// Zeigers auf einer Hilfslinie. **Kreuzungen haben Vorrang**: liegen zwei
/// Linien im Fangradius, ist der Schnittpunkt beider Bedingungen gemeint und
/// nicht die nähere der beiden Einzelachsen. Damit lässt sich ein Punkt sowohl
/// auf eine Achse setzen (x oder y des Haltepunkts) als auch aus zwei
/// Haltepunkten kombinieren (x von A, y von B).
///
/// Für den Fang in der Zeichnung nimmt die Fang-Kette
/// [`hilfslinien_snap_mit_geometrie`] — dort kommt der Schnitt mit einer
/// bestehenden Linie hinzu.
pub fn hilfslinien_snap(halte: &[HaltPunkt], cursor: Vec2, tol: f64) -> Option<SnapHit> {
    hilfslinien_snap_innen(halte, cursor, tol, &[], None)
}

/// Fang auf den Hilfslinien **einschließlich ihrer Schnittpunkte mit
/// bestehenden Linien**.
///
/// Ohne diesen Zweig ließ sich der Punkt, an dem eine Hilfslinie eine Wand oder
/// Linie kreuzt, nicht treffen: der Fußpunkt des Zeigers auf der Hilfslinie
/// gewinnt gegen „Nächster Punkt", also glitt der Zeiger an der Kreuzung
/// vorbei, statt exakt darauf einzurasten (Rückmeldung zum ersten Auslieferungs-
/// stand). Gesucht wird nur im Fangradius und nur auf **Strecken** — die
/// Verlängerung einer bestehenden Linie ist kein Schnittpunkt.
pub fn hilfslinien_snap_mit_geometrie(
    doc: &Document,
    halte: &[HaltPunkt],
    cursor: Vec2,
    tol: f64,
    ignore: &[Id],
) -> Option<SnapHit> {
    let segs = nahe_segmente(doc, cursor, tol, ignore);
    hilfslinien_snap_innen(halte, cursor, tol, &segs, None)
}

/// Wie [`hilfslinien_snap_mit_geometrie`], zusätzlich mit der **Zwangsachse**
/// des Zeigers (Ortho/Polar).
///
/// Sie ist eine Bedingung, nicht eine Alternative: liegt eine Hilfslinie im
/// Fangradius, wird ihr Fußpunkt auf die Achse gezogen — gefangen wird der
/// **Schnittpunkt** beider, dadurch bleibt die neue Linie in der Zwangsrichtung
/// (T0042). Ohne diesen Zweig sprang der Punkt je nach Abstand zur Hilfslinie
/// zwischen Linienfang und Ortho, und die gezeichnete Linie stand schief.
pub fn hilfslinien_snap_mit_zwang(
    doc: &Document,
    halte: &[HaltPunkt],
    cursor: Vec2,
    tol: f64,
    ignore: &[Id],
    zwang: Option<Zwangsachse>,
) -> Option<SnapHit> {
    if zwang.is_none() {
        return hilfslinien_snap_mit_geometrie(doc, halte, cursor, tol, ignore);
    }
    let segs = nahe_segmente(doc, cursor, tol, ignore);
    hilfslinien_snap_innen(halte, cursor, tol, &segs, zwang)
}

/// Strecken in Fangreichweite — dieselbe Auswahl wie in der Fang-Engine
/// (sichtbare, nicht ausgeschlossene Objekte; Wandkontur nur bei
/// `snap.wall_edges`).
fn nahe_segmente(doc: &Document, cursor: Vec2, tol: f64, ignore: &[Id]) -> Vec<(Seg, Id)> {
    let tol = tol.max(1e-6);
    let area = BBox::new(cursor - v(tol, tol), cursor + v(tol, tol));
    let wall_edges = doc.settings.snap.wall_edges;
    let mut segs = Vec::new();
    let mut widx: Option<render::WallIndex> = None;
    for e in doc.ents() {
        if ignore.contains(&e.id) {
            continue;
        }
        if doc.layer_gesperrt(e.layer) { continue; }
        let b = render::entity_bbox(doc, e);
        if !b.is_valid() || !b.expanded(tol * 2.0).intersects(&area) {
            continue;
        }
        for sg in render::entity_segments(doc, e) {
            segs.push((sg, e.id));
        }
        for sg in render::text_frame_segments(doc, e) {
            segs.push((sg, e.id));
        }
        if wall_edges && matches!(e.kind, EntityKind::Wall { .. }) {
            let idx = widx.get_or_insert_with(|| render::WallIndex::build(doc, doc.ents()));
            for sg in render::wall_contour_segments_idx(idx, e) {
                segs.push((sg, e.id));
            }
        }
    }
    segs
}

/// Schnittpunkte einer unbegrenzten Geraden mit einer Strecke bzw. einem Bogen.
fn gerade_schneidet_seg(l: &Hilfslinie, sg: &Seg) -> Vec<Vec2> {
    let (o, d) = (l.origin, l.origin + l.dir);
    match sg {
        Seg::Line(a, b) => line_line(o, d, *a, *b, true, false).into_iter().collect(),
        Seg::Arc(arc) => line_circle(o, d, arc.c, arc.r)
            .into_iter()
            .filter(|p| {
                let ang = (*p - arc.c).angle();
                angle_in_arc(ang, arc.a0, arc.a1)
            })
            .collect(),
    }
}

fn hilfslinien_snap_innen(
    halte: &[HaltPunkt],
    cursor: Vec2,
    tol: f64,
    segs: &[(Seg, Id)],
    zwang: Option<Zwangsachse>,
) -> Option<SnapHit> {
    if halte.is_empty() || tol <= 0.0 {
        return None;
    }
    let linien = hilfslinien(halte);
    let tol = tol.max(1e-6);
    let nach_abstand = |a: &HilfsTreffer, b: &HilfsTreffer| {
        a.dist
            .partial_cmp(&b.dist)
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    // Exakte Punkte: Kreuzungen der Hilfslinien untereinander und mit
    // bestehenden Strecken.
    let mut exakt: Vec<HilfsTreffer> = Vec::new();
    // Fußpunkte des Zeigers auf einer Hilfslinie (gleiten mit).
    let mut fuesse: Vec<HilfsTreffer> = Vec::new();

    // **Zwangsachse und Hilfslinie zugleich** (T0042): Die Achse ist eine
    // Bedingung an den Punkt, die Hilfslinie nur ein Angebot. Liegt eine
    // Hilfslinie im Fangradius, bleibt die Bedingung bestehen — der gemeinte
    // Punkt ist der Schnitt beider, nicht der eine oder der andere. Vorher
    // gewann je nach Abstand zur Hilfslinie der Linienfang oder Ortho, die
    // Linie kippte dadurch aus der Zwangsrichtung. Gewählt wird die nächste
    // Hilfslinie; ist sie zur Achse parallel, gibt es keinen Schnitt und die
    // nächste kommt zum Zug.
    if let Some(z) = zwang {
        let mut kombi: Vec<HilfsTreffer> = Vec::new();
        for l in &linien {
            let d = fuss_auf_gerade(cursor, l.origin, l.dir).dist(cursor);
            if d > tol {
                continue;
            }
            if let Some(x) = line_line(
                z.origin,
                z.origin + z.dir,
                l.origin,
                l.origin + l.dir,
                true,
                true,
            ) {
                kombi.push(HilfsTreffer {
                    dist: d,
                    point: x,
                    kind: SnapKind::Tracking,
                    entity: halt_entity(halte, l),
                });
            }
        }
        kombi.sort_by(nach_abstand);
        if let Some(t) = kombi.first() {
            return Some(SnapHit {
                point: t.point,
                kind: t.kind,
                entity: t.entity,
                marke: None,
            });
        }
    }

    for l in &linien {
        let f = fuss_auf_gerade(cursor, l.origin, l.dir);
        let d = f.dist(cursor);
        if d <= tol {
            fuesse.push(HilfsTreffer {
                dist: d,
                point: f,
                kind: SnapKind::Tracking,
                entity: halt_entity(halte, l),
            });
        }
        // Schnitt mit einer bestehenden Linie. Ein bearbeitetes Element steht
        // in `ignore` und taucht hier deshalb gar nicht auf.
        for (sg, id) in segs {
            for p in gerade_schneidet_seg(l, sg) {
                let d = p.dist(cursor);
                if d <= tol {
                    exakt.push(HilfsTreffer {
                        dist: d,
                        point: p,
                        kind: SnapKind::Intersection,
                        entity: Some(*id),
                    });
                }
            }
        }
    }
    for i in 0..linien.len() {
        for j in (i + 1)..linien.len() {
            let a = &linien[i];
            let b = &linien[j];
            if let Some(x) = line_line(
                a.origin,
                a.origin + a.dir,
                b.origin,
                b.origin + b.dir,
                true,
                true,
            ) {
                let d = x.dist(cursor);
                if d <= tol {
                    exakt.push(HilfsTreffer {
                        dist: d,
                        point: x,
                        kind: SnapKind::Tracking,
                        entity: None,
                    });
                }
            }
        }
    }

    // Exakte Punkte vor den gleitenden Fußpunkten: stehen zwei Linien im
    // Fangradius, ist ihr Schnittpunkt gemeint, nicht die nähere Einzelachse.
    for liste in [&mut exakt, &mut fuesse] {
        liste.sort_by(nach_abstand);
        if let Some(t) = liste.first() {
            return Some(SnapHit {
                point: t.point,
                kind: t.kind,
                entity: t.entity,
                marke: None,
            });
        }
    }
    None
}

/// Tangentialfang (T0157).
///
/// Zwei Lagen aus der Aufgabenstellung, beide über **eine** Rechnung:
///
/// * **Linie an Kreis/Bogen** — der Bezugspunkt (der zuletzt gesetzte Punkt
///   des Zuges) liegt auf der Kreisbahn. Durch diesen Punkt gibt es genau
///   **eine** Tangente: die Senkrechte zum Radius. Der Zeiger wird auf diese
///   Gerade gezogen, damit die Linie den Kreis tangential verlässt. Der
///   gesuchte „Winkel" ist damit der Winkel dieser Senkrechten — er wird nicht
///   geraten, sondern aus dem Berührpunkt gerechnet.
/// * **Kreis/Bogen an Linie** — ein Kreis oder Bogen wird verschoben (er steht
///   in `ignore`). Sein Mittelpunkt rastet auf die Parallele der Linie im
///   Abstand `r`: dort berührt der Kreis die Linie. Der Berührpunkt muss auf
///   der Strecke liegen, nicht auf ihrer Verlängerung.
///
/// Der Fang liefert einen **Punkt** (den Zeigerort auf der Tangente bzw. den
/// neuen Mittelpunkt), keine Richtung — so passt er in dieselbe Fang-Kette wie
/// Lot und Winkelhalbierende. Er ist kein diskreter Punkt: er wandert mit dem
/// Zeiger und taugt deshalb nicht als Haltepunkt des erweiterten Punktfangs.
pub fn tangent_treffer(
    doc: &Document,
    reference: Option<Vec2>,
    cursor: Vec2,
    tol: f64,
    ignore: &[Id],
) -> Vec<SnapHit> {
    let tol = tol.max(1e-6);
    let mut out: Vec<SnapHit> = Vec::new();

    // (a) Linie an Kreis: der Bezugspunkt liegt auf der Bahn.
    if let Some(r0) = reference {
        for e in doc.ents() {
            if ignore.contains(&e.id) {
                continue;
            }
            if doc.layer_gesperrt(e.layer) { continue; }
            let (c, r, bereich) = match &e.kind {
                EntityKind::Circle { c, r } => (*c, *r, None),
                EntityKind::Arc { c, r, a0, a1 } => (*c, *r, Some((*a0, *a1))),
                _ => continue,
            };
            if r <= 1e-9 {
                continue;
            }
            // Der Bezugspunkt muss **auf** der Bahn liegen, nicht nur in ihrer
            // Nähe: sonst hinge die Tangente an einem Punkt, der gar nicht auf
            // dem Kreis sitzt.
            if (r0.dist(c) - r).abs() > tol {
                continue;
            }
            if let Some((a0, a1)) = bereich {
                let sweep = norm_angle(a1 - a0);
                let rel = norm_angle((r0 - c).angle() - a0);
                if rel > sweep + 1e-9 {
                    continue;
                }
            }
            let radius = (r0 - c).normalized();
            if radius.len() <= 1e-9 {
                continue;
            }
            let tangente = radius.perp();
            let p = fuss_auf_gerade(cursor, r0, tangente);
            let d = p.dist(cursor);
            if d <= tol {
                out.push(SnapHit {
                    point: p,
                    kind: SnapKind::Tangent,
                    entity: Some(e.id),
                    // Die Linie berührt den Kreis dort, wo sie beginnt.
                    marke: Some(r0),
                });
            }
        }
    }

    // (b) Kreis/Bogen an Linie: der bewegte Kreis rastet mit seinem Mittelpunkt
    //     auf die Parallele im Abstand r.
    let mut bewegte_radien: Vec<f64> = Vec::new();
    for id in ignore {
        if let Some(e) = doc.entity(*id) {
            match &e.kind {
                EntityKind::Circle { r, .. } | EntityKind::Arc { r, .. } => {
                    if *r > 1e-9 {
                        bewegte_radien.push(*r);
                    }
                }
                _ => {}
            }
        }
    }
    if !bewegte_radien.is_empty() {
        let area = BBox::new(cursor - v(tol, tol), cursor + v(tol, tol));
        // Der gesuchte Mittelpunkt liegt bis zu `r` neben der Linie — die
        // Bounding-Box der Linie muss um den größten bewegten Radius wachsen,
        // sonst fällt die Linie schon vor der Rechnung heraus.
        let weit = bewegte_radien.iter().copied().fold(0.0_f64, f64::max);
        for e in doc.ents() {
            if ignore.contains(&e.id) {
                continue;
            }
            if doc.layer_gesperrt(e.layer) { continue; }
            let b = render::entity_bbox(doc, e);
            if !b.is_valid() || !b.expanded(tol * 2.0 + weit).intersects(&area) {
                continue;
            }
            for sg in render::entity_segments(doc, e) {
                let Seg::Line(a, b2) = sg else { continue };
                let ab = b2 - a;
                let l2 = ab.len2();
                if l2 < 1e-12 {
                    continue;
                }
                let n = ab.perp().normalized();
                for r in &bewegte_radien {
                    for seite in [1.0, -1.0] {
                        let q = fuss_auf_gerade(cursor, a + n * (r * seite), ab);
                        // Der Berührpunkt muss auf der Strecke liegen.
                        let t = (q - a).dot(ab) / l2;
                        if !(-1e-9..=1.0 + 1e-9).contains(&t) {
                            continue;
                        }
                        let d = q.dist(cursor);
                        if d <= tol {
                            out.push(SnapHit {
                                point: q,
                                kind: SnapKind::Tangent,
                                entity: Some(e.id),
                                // `q` ist der neue Mittelpunkt; berührt wird
                                // die Linie an seinem Fußpunkt auf ihr.
                                marke: Some(a + ab * t),
                            });
                        }
                    }
                }
            }
        }
    }

    out
}

/// Sucht den besten Fangpunkt in der Nähe des Cursors.
///
/// * `tol` – Fangradius in Weltkoordinaten
/// * `reference` – Bezugspunkt (für Lotfuß / Ortho)
/// * `ignore` – Objekte, die überhaupt nicht fangen. Hier steht das Element,
///   das gerade bearbeitet wird: ein Objekt fängt nie auf sich selbst.
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
    snap_candidates_gated(doc, cursor, tol, reference, ignore, allow_grid)
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
) -> Vec<SnapHit> {
    snap_candidates_gated(doc, cursor, tol, reference, ignore, true)
}

/// Wie [`snap_candidates`], aber der Rasterfang lässt sich abschalten.
pub fn snap_candidates_gated(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    allow_grid: bool,
) -> Vec<SnapHit> {
    snap_candidates_mit_vorschau(doc, cursor, tol, reference, ignore, &[], allow_grid)
}

/// Wie [`snap_candidates_gated`], zusätzlich mit **Vorschau-Punkten**:
/// Stützpunkte eines noch nicht angelegten Zuges (Polylinie) fangen wie
/// Endpunkte. Ohne sie wäre der eigene Anfang nicht erreichbar und ein
/// geschlossener Zug nur per Zufall möglich.
pub fn snap_candidates_mit_vorschau(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    vorschau: &[Vec2],
    allow_grid: bool,
) -> Vec<SnapHit> {
    snap_candidates_alle(doc, cursor, tol, reference, ignore, vorschau, allow_grid)
}

/// Basispunkt des im Blockeditor offenen Blocks (T0208).
///
/// Der Block wird in Millimetern um seinen Basispunkt gezeichnet; im Editor
/// liegt er deshalb im Weltursprung. Außerhalb des Blockeditors gibt es keinen
/// Basispunkt — `None`.
pub fn block_basispunkt(doc: &Document) -> Option<Vec2> {
    doc.in_block_space().then_some(Vec2::ZERO)
}

#[allow(clippy::too_many_arguments)]
fn snap_candidates_alle(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    vorschau: &[Vec2],
    allow_grid: bool,
) -> Vec<SnapHit> {
    let mut s = doc.settings.snap.clone();
    if !allow_grid {
        s.grid = false;
    }
    let s = &s;
    let tol = tol.max(1e-6);
    let mut cands: Vec<(f64, SnapHit)> = Vec::new();

    // Laibungen der bearbeiteten Öffnungen: die Wand wird dort aufgeschnitten,
    // die Laibungskanten und ihre Ecken sind aus der Öffnung gerechnet und
    // wandern mit ihr. Sie sind damit kein Fangziel — sonst zieht der Fang die
    // Tür auf ihre eigene Laibung, die Tür wandert mit, die Kante ist wieder
    // woanders: der Punkt springt in jedem Bild.
    //
    // Nur der Längsbereich der Öffnung fällt weg. Der Rest der Wand bleibt
    // fangbar: Stirnkanten, Gehrungsecken an Anschlüssen und Kreuzungen,
    // Längskanten außerhalb der Öffnung. Der Bereich wird **gerechnet**, nicht
    // je Elementart gepflegt.
    let spannen: Vec<crate::wall::Laibungsspanne> = ignore
        .iter()
        .filter_map(|id| crate::wall::laibungsspanne(doc, *id))
        .collect();

    // Der Tangentialfang bringt als einziger eine **eigene Markenstelle** mit
    // (T0211). Er kann sie nicht durch `consider` reichen (das setzt keine),
    // sondern sammelt getrennt und kommt am Ende dazu.
    let mut tangent_cands: Vec<(f64, SnapHit)> = Vec::new();
    let mut consider = |p: Vec2, kind: SnapKind, entity: Option<Id>, dist: f64| {
        if dist > tol {
            return;
        }
        if spannen.iter().any(|s| s.enthaelt(entity, p)) {
            return;
        }
        cands.push((
            dist,
            SnapHit {
                point: p,
                kind,
                entity,
                marke: None,
            },
        ));
    };

    // Kandidaten sammeln (nur sichtbare Objekte in der Nähe).
    //
    // `ignore` ist der **einzige** Selbst-Fang-Schutz: das Element, das gerade
    // bearbeitet wird (Griffzug, wandernde Auswahl), steht darin und liefert
    // damit überhaupt keine Fangpunkte — auch keine, die erst durch eine
    // Zerlegung entstehen. Eine Prüfung „welcher Punkt wandert mit?" gibt es
    // bewusst nicht: sie müsste je Elementart gepflegt werden und jeder
    // vergessene Zweig bringt den Selbst-Fang zurück.
    let area = BBox::new(cursor - v(tol, tol), cursor + v(tol, tol));
    let mut segs: Vec<(Seg, Id)> = Vec::new();
    let mut widx: Option<render::WallIndex> = None;
    // Splines: die gezeichnete Kurve ist eine Zerlegung in viele kurze
    // Teilstrecken. Deren Enden sind **keine** Fangpunkte — sonst hinge an
    // jedem Zerlegungspunkt ein „Endpunkt", den niemand gesetzt hat. Fangbar
    // sind nur die Stützpunkte; die Teilstrecken bleiben für Schnittpunkt und
    // „Nächster Punkt" erhalten.
    let mut spline_ids: std::collections::HashSet<Id> = std::collections::HashSet::new();
    let mut stuetzpunkte: Vec<(Vec2, Id)> = Vec::new();
    // Aktiver Bearbeitungsraum (Modell oder Papierraum)
    for e in doc.ents() {
        if ignore.contains(&e.id) {
            continue;
        }
        if doc.layer_gesperrt(e.layer) { continue; }
        let b = render::entity_bbox(doc, e);
        if !b.is_valid() || !b.expanded(tol * 2.0).intersects(&area) {
            continue;
        }
        for sg in render::entity_segments(doc, e) {
            segs.push((sg, e.id));
        }
        // Spline: nur die Stützpunkte sind feste Fangziele.
        if let EntityKind::Spline { verts, .. } = &e.kind {
            spline_ids.insert(e.id);
            if s.endpoint {
                for v in verts {
                    stuetzpunkte.push((v.p, e.id));
                }
            }
        }
        // Text: der unsichtbare Rahmen ist mit seinen Ecken (END), Kantenmitten
        // (MIT) und Kanten (NÄC) fangbar — er ist der Kasten, den die
        // Einfüge-Vorschau zeigt und der bei Auswahl gezeichnet wird.
        for sg in render::text_frame_segments(doc, e) {
            segs.push((sg, e.id));
        }
        // Wandkörper: die umgrenzenden Linien (Längskanten, Gehrungs-/Stirn-
        // ecken, Laibungen) liefern ebenfalls Fangpunkte. Ohne diesen Zweig
        // kennt der Fang nur die Wandachse. Getrennt schaltbar über
        // `snap.wall_edges` — die Achse bleibt in jedem Fall erhalten.
        if s.wall_edges && matches!(e.kind, EntityKind::Wall { .. }) {
            let idx = widx.get_or_insert_with(|| render::WallIndex::build(doc, doc.ents()));
            for sg in render::wall_contour_segments_idx(idx, e) {
                segs.push((sg, e.id));
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
    for (sg, id) in &segs {
        // Teilstrecken eines Splines liefern keine End- und Mittelpunkte:
        // sie sind nur die Zerlegung der Kurve, keine gesetzten Punkte.
        // Fangbar sind allein die Stützpunkte (siehe unten); die Teilstrecken
        // bleiben für Lot und „Nächster Punkt" erhalten.
        let teilstrecke_eines_splines = spline_ids.contains(id);
        let a = sg.start();
        let b = sg.end();
        let da = a.dist(cursor);
        let db = b.dist(cursor);
        if s.endpoint && !teilstrecke_eines_splines {
            consider(a, SnapKind::Endpoint, Some(*id), da);
            consider(b, SnapKind::Endpoint, Some(*id), db);
        }
        if s.midpoint && !teilstrecke_eines_splines {
            let m = a.lerp(b, 0.5);
            consider(m, SnapKind::Midpoint, Some(*id), m.dist(cursor));
        }
        if let Seg::Arc(arc) = sg {
            if s.center {
                consider(arc.c, SnapKind::Center, Some(*id), arc.c.dist(cursor));
            }
            // Quadranten sind ein eigenes Fangziel (T0160): wer sie nicht
            // will, musste früher auch die Endpunkte abschalten.
            if s.quadrant {
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
                // Ein Fußpunkt, der auf dem Bezugspunkt selbst liegt, ist kein
                // Lot (der Bezugspunkt liegt dann auf der Strecke).
                if foot.dist(r) > SNAP_DEDUP {
                    consider(foot, SnapKind::Perpendicular, Some(*id), foot.dist(cursor));
                }
            }
        }
        if s.nearest {
            let (q, d) = sg.closest(cursor);
            consider(q, SnapKind::Nearest, Some(*id), d);
        }
    }

    // Tangentialfang (T0157): Linie tangential an einem Kreis, bewegter Kreis
    // tangential an einer Linie. Er hängt am Bezugspunkt bzw. am Radius des
    // bewegten Kreises und tritt als Kandidat in dieselbe Liste — damit gilt
    // für ihn dieselbe Sortierung und Entdopplung wie für jeden anderen Fang.
    if s.tangent {
        for hit in tangent_treffer(doc, reference, cursor, tol, ignore) {
            let d = hit.point.dist(cursor);
            if d > tol {
                continue;
            }
            tangent_cands.push((d, hit));
        }
    }

    // Stützpunkte eines Splines: die vom Nutzer gesetzten Punkte sind die
    // einzigen festen Fangziele der Kurve (T0134). Sie fangen wie Endpunkte
    // eines Objekts; der gerade gezogene Punkt ist über `gesperrt` ausgenommen.
    for (p, id) in &stuetzpunkte {
        consider(*p, SnapKind::Endpoint, Some(*id), p.dist(cursor));
    }

    // Vorschau-Punkte: Stützpunkte des gerade gezeichneten Zuges, die noch
    // nicht im Modell stehen. Sie fangen wie Endpunkte eines Objekts — nur so
    // lässt sich der Anfang (Schließen) oder ein früherer Stützpunkt exakt
    // wieder treffen.
    if s.endpoint {
        for p in vorschau {
            consider(*p, SnapKind::Endpoint, None, p.dist(cursor));
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

    // Blattecken: die echten Eckpunkte des Papiers sind keine Objekte, sollen
    // aber gefangen werden — sonst trifft man den Blattrand nie genau. Nur im
    // Papierraum; im Modell gibt es kein Blatt.
    if s.endpoint {
        if let Some(sheet) = doc.space_sheet() {
            let (w, h) = sheet.size();
            for p in [v(0.0, 0.0), v(w, 0.0), v(w, h), v(0.0, h)] {
                consider(p, SnapKind::SheetCorner, None, p.dist(cursor));
            }
        }
    }

    // Basispunkt des Blocks: im Blockeditor ist der Weltursprung der
    // Einfügepunkt des Blocks. Er ist kein Objekt und war bisher reine
    // Anzeige — der Fang erreichte ihn nicht, obwohl das Kreuz dort steht
    // (T0208). Wie die Blattecke hängt er am Schalter „Endpunkt".
    if s.endpoint {
        if let Some(bp) = block_basispunkt(doc) {
            consider(bp, SnapKind::BlockBase, None, bp.dist(cursor));
        }
    }

    // Raster — dieselbe Rechnung wie die Anzeige (`gitter::Raster`), damit die
    // Rasterart (rechtwinklig, isometrisch …) auch fängt (T0144).
    if s.grid {
        let raster = s.raster();
        if raster.step > 0.0 {
            let gp = raster.fangen(cursor);
            consider(gp, SnapKind::Grid, None, gp.dist(cursor));
        }
    }

    cands.extend(tangent_cands);
    sortiere_kandidaten(cands.into_iter().map(|(_, hit)| hit).collect(), cursor)
}

/// Kandidaten in Fangreihenfolge bringen: Priorität der Fangart, dann Abstand
/// zum Zeiger; dieselbe Koordinate (innerhalb [`SNAP_DEDUP`]) nur einmal.
///
/// Eigene Funktion, weil die Winkelhalbierende (T0150) ihre Treffer erst in der
/// Fang-Kette beisteuert — sie muss nach **denselben** Regeln einsortiert und
/// entdoppelt werden wie die Treffer der Engine. Der Abstand ist immer der des
/// Punktes zum Zeiger (jeder Kandidat wird mit genau diesem Abstand gesammelt).
fn sortiere_kandidaten(mut hits: Vec<SnapHit>, cursor: Vec2) -> Vec<SnapHit> {
    hits.sort_by(|a, b| {
        a.kind
            .priority()
            .cmp(&b.kind.priority())
            .then(a.point.dist(cursor).total_cmp(&b.point.dist(cursor)))
    });
    let mut out: Vec<SnapHit> = Vec::with_capacity(hits.len());
    for hit in hits {
        if out.iter().any(|h| h.point.dist(hit.point) <= SNAP_DEDUP) {
            continue;
        }
        out.push(hit);
    }
    out
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

    /// Dasselbe Label in der gewählten Sprache (T0106) — `POL` bleibt Kürzel.
    pub fn text(self) -> Option<&'static str> {
        match self {
            SnapSource::Object(k) => Some(k.text()),
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
    /// Stelle der Fangmarke (T0211), wenn sie nicht auf `point` liegt.
    ///
    /// Beim Tangentialfang fällt der Fangpunkt mit dem Zeigerort auf der
    /// Tangente zusammen; das Zeichen gehört aber an den **Berührpunkt** von
    /// Linie und Kreis — also dorthin, wo die Linie den Kreis berührt.
    pub marke: Option<Vec2>,
}

impl SnapResolution {
    /// Der übernommene Punkt ist ein echter Fangpunkt (nicht nur der Zeiger).
    pub fn is_snap(&self) -> bool {
        matches!(self.source, SnapSource::Object(_) | SnapSource::Polar)
    }

    pub fn label(&self) -> Option<&'static str> {
        self.source.label()
    }

    /// Label der Statuszeile in der gewählten Sprache (T0106).
    pub fn text(&self) -> Option<&'static str> {
        self.source.text()
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
    allow_grid: bool,
) -> SnapResolution {
    snap_constrained_mit_halte(doc, cursor, tol, reference, ignore, &[], &[], allow_grid)
}

/// Wie [`snap_constrained`], zusätzlich mit Vorschau-Punkten des laufenden
/// Zuges (siehe [`snap_candidates_mit_vorschau`]).
pub fn snap_constrained_mit_vorschau(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    vorschau: &[Vec2],
    allow_grid: bool,
) -> SnapResolution {
    snap_constrained_mit_halte(doc, cursor, tol, reference, ignore, vorschau, &[], allow_grid)
}

/// Die Fang-Kette mit Haltepunkten des erweiterten Punktfangs.
///
/// Reihenfolge innerhalb der Fang-Kette:
///
/// 1. **diskrete** Objektfänge (Endpunkt, Mitte, Zentrum, Quadrant, Schnitt,
///    Blattecke) — ein echter Punkt auf der Geometrie ist immer die genauere
///    Wahl als eine Hilfslinie.
/// 2. **Hilfslinien** der Haltepunkte (siehe [`hilfslinien_snap`]); greift
///    zugleich Ortho oder Polar, ihr Schnittpunkt mit der Zwangsachse
///    ([`hilfslinien_snap_mit_zwang`]).
/// 3. kontinuierliche Objektfänge (Nächster Punkt, Lot) — sie hängen am Zeiger
///    und sind nur eine Notlösung; die Hilfslinie ist die ausdrückliche Wahl.
/// 4. Ortho/Polar, zuletzt das Raster.
#[allow(clippy::too_many_arguments)]
pub fn snap_constrained_mit_halte(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    reference: Option<Vec2>,
    ignore: &[Id],
    vorschau: &[Vec2],
    halte: &[HaltPunkt],
    allow_grid: bool,
) -> SnapResolution {
    snap_constrained_achsen(
        doc,
        cursor,
        tol,
        reference.as_slice(),
        false,
        ignore,
        vorschau,
        halte,
        allow_grid,
    )
}

/// Wie [`snap_constrained_mit_halte`], aber mit **mehreren** Bezugspunkten.
///
/// Ein gezogener Griff einer Polylinie, Fläche oder eines Volumens hängt an
/// **zwei** Linien (zum vorigen und zum nächsten Stützpunkt). Mit nur einem
/// Bezug ließ sich deshalb immer nur eine der beiden ausrichten (T0046,
/// Turn 3). Bei zwei und mehr Bezügen gilt:
///
/// * **Ortho**: jede Koordinate rastet einzeln auf die Achse (Senkrechte bzw.
///   Waagerechte) des nächstgelegenen Bezugs, sofern sie im Fangradius liegt.
///   Liegen beide Koordinaten an Achsen — etwa in der Ecke eines Rechtecks —,
///   landet der Punkt auf deren Schnittpunkt, und beide Linien stehen
///   gleichzeitig im Zwang.
/// * **Polar**: jede Richtung des Winkelkranzes **jedes** Bezugs rastet ein
///   (nicht nur der des nächstgelegenen), und die Kreuzung zweier Geraden
///   verschiedener Bezüge ist ein eigenes Punktziel (T0054) — sonst ließ sich
///   immer nur eine der beiden angrenzenden Linien in 45°-Schritten führen.
///
/// Bei genau einem Bezug bleibt es beim harten Ortho des Erstellens — dort ist
/// die Achse die einzige Führung und muss ohne Toleranz gelten.
#[allow(clippy::too_many_arguments)]
pub fn snap_constrained_achsen(
    doc: &Document,
    cursor: Vec2,
    tol: f64,
    achsen: &[Vec2],
    lot_aus: bool,
    ignore: &[Id],
    vorschau: &[Vec2],
    halte: &[HaltPunkt],
    allow_grid: bool,
) -> SnapResolution {
    let reference = naechster_bezug(achsen, cursor);
    // Beim Bewegen der Auswahl ist der Bezugspunkt der **Basispunkt des Zuges**
    // — ein Punkt des bewegten Elements. Ein Lot von dort auf eine fremde Kante
    // zöge das Element auf seinen Startpunkt zurück (es klebt, springt beim
    // Verlassen des Fangradius und klebt wieder). Deshalb gibt es dort kein Lot;
    // „Nächster Punkt" setzt den Zeiger genauso auf die Kante.
    let lot = if lot_aus { None } else { reference };
    let s = doc.settings.snap.clone();
    let tol = tol.max(1e-6);

    // 1. Objektfang ohne Raster: er gewinnt gegen jede Zwangsrichtung.
    //    Das Raster ist eine Führung wie Ortho und kommt erst in Schritt 3.
    let mut cands = snap_candidates_alle(doc, cursor, tol, lot, ignore, vorschau, false);
    // Die Winkelhalbierende zwischen zwei Linien (T0150) ist ein **Fang**, kein
    // Zwang: sie tritt als Kandidat in dieselbe Liste und verliert damit gegen
    // Endpunkt, Schnittpunkt und Mitte, gewinnt aber gegen „Nächster Punkt" und
    // Lot (Priorität, siehe `SnapKind::priority`). Angeboten wird sie an jedem
    // Bezugspunkt des Zuges und an jedem Haltepunkt des erweiterten Punktfangs —
    // das sind genau die zwei Lagen aus der Aufgabenstellung.
    if s.bisector {
        let mut winkel_punkte: Vec<Vec2> = achsen.to_vec();
        winkel_punkte.extend(halte.iter().map(|h| h.point));
        cands.extend(winkelhalbierende_treffer(
            doc,
            &winkel_punkte,
            cursor,
            tol,
            ignore,
        ));
    }
    let bester = sortiere_kandidaten(cands, cursor).into_iter().next();

    let diskret = bester
        .as_ref()
        .map(|h| h.kind.ist_diskret())
        .unwrap_or(false);
    if diskret {
        let hit = bester.unwrap();
        return SnapResolution {
            point: hit.point,
            source: SnapSource::Object(hit.kind),
            marke: hit.marke,
        };
    }

    // 2. Hilfslinien des erweiterten Punktfangs — samt ihrem Schnitt mit
    //    bestehenden Linien (der Punkt, an dem eine Hilfslinie eine Wand
    //    kreuzt, ist sonst nicht zu treffen). Greift zugleich Ortho oder Polar,
    //    ist der Schnittpunkt aus Hilfslinie und Zwangsachse gemeint: die
    //    Zwangsachse bleibt Bedingung, die Hilfslinie wählt die Lage auf ihr
    //    (T0042).
    let zwang = zwangsachse(&s, cursor, reference, tol);
    // Hilfslinien sind abschaltbar (T0160): dann fängt der nächste Punkt zwar
    // weiter normal, aber nicht mehr auf den Linien gehaltener Punkte.
    if s.hilfslinien {
        if let Some(hit) = hilfslinien_snap_mit_zwang(doc, halte, cursor, tol, ignore, zwang) {
            return SnapResolution {
                point: hit.point,
                source: SnapSource::Object(hit.kind),
                marke: hit.marke,
            };
        }
    }

    // 3. Kontinuierlicher Objektfang (Nächster Punkt, Lot) in Reichweite.
    if let Some(hit) = bester {
        return SnapResolution {
            point: hit.point,
            source: SnapSource::Object(hit.kind),
            marke: hit.marke,
        };
    }

    // 4. Kein Objekt in Reichweite: Ortho bzw. Polar.
    let mut point = cursor;
    let mut source = SnapSource::Free;
    if achsen.len() >= 2 {
        // Zwei Linien am selben Griff (T0046, Turn 3/4).
        if s.ortho {
            // Zuerst der **Schnittpunkt** zweier Achsen (eine je Nachbar):
            // dort stehen beide Linien zugleich gerade. Er ist ein Punktziel
            // wie ein Endpunkt und wird deshalb mit einem größeren Fenster
            // gefangen — sonst träfe man ihn nur auf den Pixel genau, und die
            // Einzelachse schnappte vorher („es rastet nur eine der beiden
            // Linien ein").
            if let Some(q) = achsen_schnittpunkt(achsen, cursor, tol * SCHNITTPUNKT_FAKTOR) {
                point = q;
                source = SnapSource::Ortho;
            } else {
                let (x, y) = achsen_koordinaten(achsen, cursor, tol);
                if x.is_some() || y.is_some() {
                    point = Vec2::new(x.unwrap_or(cursor.x), y.unwrap_or(cursor.y));
                    source = SnapSource::Ortho;
                }
            }
        } else if s.polar {
            // **Alle** Bezüge liefern ihren Winkelkranz, nicht nur der
            // nächstgelegene (T0054): sonst rastete nur die 45°-Schar des
            // Nachbarn, der gerade näher am Zeiger liegt. Zuerst der
            // Schnittpunkt zweier Polargeraden **verschiedener** Bezüge —
            // ohne ihn wäre die Kreuzung der beiden Scharen nur auf den Pixel
            // genau zu treffen, weil die Einzelgerade vorher greift.
            if let Some(q) = polar_schnittpunkt(&s, cursor, achsen, tol * SCHNITTPUNKT_FAKTOR) {
                point = q;
                source = SnapSource::Polar;
            } else if let Some(p) = polar_einzel(&s, cursor, achsen, tol) {
                point = p;
                source = SnapSource::Polar;
            }
        }
    } else if let Some(r) = reference {
        if s.ortho {
            point = cursor.ortho_from(r, false);
            source = SnapSource::Ortho;
        } else if s.polar && cursor.dist(r) > tol * 2.0 {
            if let Some(p) = cursor.polar_snap(r, s.polar_step.max(1.0), s.polar_tolerance, tol) {
                point = p;
                source = SnapSource::Polar;
            }
        }
    }

    // 5. Raster zuletzt. Mit „Rasterfang erzwingen“ (T0144) liegt **jede**
    //    Eingabe im Raster, ohne Fangradius; sonst rastet es nur in Reichweite.
    //    Die Objektfänge oben (Endpunkt, Schnitt, Nächster Punkt …) geben vorher
    //    zurück und bleiben exakt — ein Zwang auf das Raster würde sie von der
    //    Geometrie wegziehen.
    if allow_grid && s.grid {
        let raster = s.raster();
        if raster.step > 0.0 {
            let gp = raster.fangen(point);
            if s.grid_force || gp.dist(point) <= tol {
                return SnapResolution {
                    point: gp,
                    source: SnapSource::Object(SnapKind::Grid),
                    marke: None,
                };
            }
        }
    }
    SnapResolution {
        point,
        source,
        marke: None,
    }
}

/// Nächstgelegener Bezugspunkt — für Lot, Polarfang und die Kombination mit
/// Hilfslinien; bei einem einzigen Bezug ist es dieser selbst.
fn naechster_bezug(achsen: &[Vec2], cursor: Vec2) -> Option<Vec2> {
    achsen
        .iter()
        .copied()
        .min_by(|a, b| a.dist(cursor).total_cmp(&b.dist(cursor)))
}

/// Wie weit das Fenster für den **Schnittpunkt** zweier Achsen über den
/// normalen Fangradius hinausreicht (Faktor auf `tol`).
///
/// Der Schnittpunkt ist das Ziel, das man anvisiert („Ecke schließen"); die
/// Einzelachsen greifen dagegen schon, sobald der Zeiger neben **einer** von
/// ihnen steht. Mit demselben Radius verfehlte man den Schnittpunkt deshalb
/// regelmäßig: die erste Achse zieht den Punkt weg, bevor beide Achsen im
/// Radius liegen (T0046, Turn 4).
pub const SCHNITTPUNKT_FAKTOR: f64 = 2.0;

/// Schnittpunkt zweier Achsen **verschiedener** Bezugspunkte.
///
/// Achsen sind die Senkrechte und die Waagerechte durch einen Bezug; der
/// Schnittpunkt zweier Bezüge ist damit `(a.x, b.y)` bzw. `(b.x, a.y)`. Der
/// nächstgelegene im Fenster `radius` gewinnt — dort liegen beide
/// Nachbarlinien gleichzeitig in der Zwangsrichtung. Fällt beides zusammen
/// (gleiche x- oder y-Lage der Nachbarn), zählt der Punkt einmal.
fn achsen_schnittpunkt(achsen: &[Vec2], cursor: Vec2, radius: f64) -> Option<Vec2> {
    let mut best: Option<(f64, Vec2)> = None;
    for (i, a) in achsen.iter().enumerate() {
        for b in achsen.iter().skip(i + 1) {
            for p in [Vec2::new(a.x, b.y), Vec2::new(b.x, a.y)] {
                let d = p.dist(cursor);
                if d <= radius && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Koordinaten, die im Fangradius auf einer Achse eines Bezugspunkts liegen.
///
/// Liefert `x`, wenn eine **Senkrechte** `x = bezug.x` nahe genug ist, und `y`
/// für eine **Waagerechte** `y = bezug.y` — unabhängig voneinander, weil an
/// einem Griff zwei Linien hängen. Der Aufrufer setzt nur die gefundenen
/// Koordinaten; beide zusammen ergeben den Schnittpunkt zweier Achsen.
fn achsen_koordinaten(achsen: &[Vec2], cursor: Vec2, tol: f64) -> (Option<f64>, Option<f64>) {
    let mut x: Option<f64> = None;
    let mut y: Option<f64> = None;
    let mut dx = f64::INFINITY;
    let mut dy = f64::INFINITY;
    for a in achsen {
        let ax = (cursor.x - a.x).abs();
        if ax <= tol && ax < dx {
            dx = ax;
            x = Some(a.x);
        }
        let ay = (cursor.y - a.y).abs();
        if ay <= tol && ay < dy {
            dy = ay;
            y = Some(a.y);
        }
    }
    (x, y)
}

/// Polarrichtungen als unbegrenzte Geraden durch einen Bezugspunkt.
///
/// Ein Winkelkranz aus Vielfachen der Schrittweite (Vorgabe 45°) deckt alle
/// acht Strahlen ab; für **Geraden** genügt die halbe Drehung, weil die
/// Gegenrichtung dieselbe Linie ist. Der Schnittpunkt zweier Geraden darf
/// deshalb auch hinter einem der Bezüge liegen (T0054).
fn polar_richtungen(step_deg: f64) -> Vec<Vec2> {
    let step = step_deg.max(1.0).to_radians();
    let n = ((std::f64::consts::PI / step).floor() as usize).max(1);
    (0..n).map(|k| Vec2::polar(k as f64 * step, 1.0)).collect()
}

/// Schnittpunkt zweier unbegrenzter Geraden (Ursprung + Richtung); `None` bei
/// paralleler Lage.
fn geraden_schnitt(a0: Vec2, ad: Vec2, b0: Vec2, bd: Vec2) -> Option<Vec2> {
    let den = ad.x * bd.y - ad.y * bd.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let w = b0 - a0;
    let t = (w.x * bd.y - w.y * bd.x) / den;
    Some(a0 + ad * t)
}

/// Nächstgelegener Schnittpunkt zweier Polarrichtungen **verschiedener**
/// Bezugspunkte im Fenster `radius` (T0054).
///
/// Die Kreuzung der beiden Winkelkränze ist ein Punktziel: ohne eigenes
/// Fangfenster wäre sie nur auf den Pixel genau zu treffen, weil die
/// Einzelgerade vorher greift. Geraden desselben Bezugs bleiben außen vor —
/// ihre Kreuzung ist der Bezug selbst und fängt als Endpunkt.
fn polar_schnittpunkt(
    s: &SnapSettings,
    cursor: Vec2,
    bezuege: &[Vec2],
    radius: f64,
) -> Option<Vec2> {
    let richtungen = polar_richtungen(s.polar_step);
    let mut best: Option<(f64, Vec2)> = None;
    for (i, a) in bezuege.iter().enumerate() {
        for b in bezuege.iter().skip(i + 1) {
            for da in &richtungen {
                for db in &richtungen {
                    let Some(p) = geraden_schnitt(*a, *da, *b, *db) else {
                        continue;
                    };
                    let d = p.dist(cursor);
                    if d <= radius && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                        best = Some((d, p));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Nächstgelegene Polarrichtung **aller** Bezüge, sofern der Zeiger im
/// Winkel-Fenster und im Band neben dem Strahl liegt (T0054, T0148).
///
/// Ein Bezug, auf dem der Zeiger fast steht, bleibt außen vor — dort ist die
/// Richtung nicht bestimmt. `tol` ist der Fangradius in Weltkoordinaten und
/// zugleich die halbe Breite des Bandes: der Polarfang reicht damit so weit
/// wie der Objektfang.
fn polar_einzel(s: &SnapSettings, cursor: Vec2, bezuege: &[Vec2], tol: f64) -> Option<Vec2> {
    bezuege
        .iter()
        .filter(|r| cursor.dist(**r) > tol * 2.0)
        .filter_map(|r| cursor.polar_snap(*r, s.polar_step.max(1.0), s.polar_tolerance, tol))
        .min_by(|a, b| a.dist(cursor).total_cmp(&b.dist(cursor)))
}

/// Trefferprüfung (Auswahl per Klick).
pub fn hit_test(doc: &Document, p: Vec2, tol: f64, layer_filter: Option<&[Id]>) -> Option<Id> {
    hit_test_opt(doc, p, tol, layer_filter, false)
}

/// Trefferprüfung für **Zerlegen**: eine gefüllte Fläche (Schraffur, Volumen)
/// zählt über ihre **ganze Fläche**, nicht nur über den 8-px-Rand (T0138).
///
/// Beim Zerlegen ist das die natürliche Geste — man klickt in die Schraffur,
/// die man auflösen will. Für die normale Auswahl bleibt es beim Randtreffer:
/// der Farbeimer fragt dieselbe Funktion und soll in einer gefüllten Fläche
/// weiter füllen können. Die Fläche ist **Rückfall**: Linien, Texte und Maße,
/// die in ihr liegen, gewinnen weiterhin.
pub fn hit_test_flaeche(
    doc: &Document,
    p: Vec2,
    tol: f64,
    layer_filter: Option<&[Id]>,
) -> Option<Id> {
    hit_test_opt(doc, p, tol, layer_filter, true)
}

fn hit_test_opt(
    doc: &Document,
    p: Vec2,
    tol: f64,
    layer_filter: Option<&[Id]>,
    flaeche_innen: bool,
) -> Option<Id> {
    let mut best: Option<(Id, f64, u8)> = None;
    // Rückfall für `hit_test_flaeche`: oberste Fläche, die den Punkt enthält.
    let mut innen: Option<Id> = None;
    // Treffer zählen im aktiven Raum — im Papierraum also Schriftfeld und Text
    for e in doc.ents().iter().rev() {
        if doc.layer_gesperrt(e.layer) {
            continue;
        }
        if let Some(f) = layer_filter {
            if !f.contains(&e.layer) {
                continue;
            }
        }
        let b = render::entity_bbox(doc, e);
        if !b.is_valid() {
            continue;
        }
        // Vorprüfung über die Ausdehnung (T0168): jede Trefferfläche dieses
        // Objekts liegt **in** seiner Box, ein Punkt weiter als der Fangradius
        // davon entfernt kann also nichts treffen. Ohne diese Zeile wurde für
        // jede Dämmungslinie der Zeichnung das Schlaufenband gerechnet, auch
        // für die weit entfernten (bei 100 m² Dämmung 4,7 ms je Klick).
        if !b.expanded(tol).contains(p) {
            continue;
        }
        // Bemaßung: der Maßtext ist Teil des sichtbaren Bereichs und zählt wie
        // ein Text als Fläche; die Maßlinie trifft über die Segmente
        // (`entity_segments` liefert seit T0052 die gezeichnete Geometrie).
        if matches!(
            e.kind,
            EntityKind::Dim { .. }
                | EntityKind::ElevationPlan { .. }
                | EntityKind::ElevationSection { .. }
        ) {
            let tb = render::trefferbild(doc, e);
            if tb
                .text_kaesten
                .iter()
                .any(|k| BBox::from_points(k).expanded(tol).contains(p))
            {
                let d = b.center().dist(p);
                let prio = 2;
                if best.map(|(_, bd, bp)| prio < bp || (prio == bp && d < bd)).unwrap_or(true) {
                    best = Some((e.id, d, prio));
                }
                continue;
            }
        }
        // Ansichtsfenster: die ganze Fläche zählt — es ist der Hintergrund des
        // Blatts, ein Klick in die Mitte muss es wählen (T0109). Objekte
        // darüber (Rahmen, Schriftfeld, Texte) haben Vorrang, deshalb
        // Priorität wie ein Textfeld.
        if let EntityKind::Viewport(vp) = &e.kind {
            if vp.rect().contains(p) {
                let d = b.center().dist(p);
                let prio = 2;
                if best.map(|(_, bd, bp)| prio < bp || (prio == bp && d < bd)).unwrap_or(true) {
                    best = Some((e.id, d, prio));
                }
            }
            continue;
        }
        // Bild: nur die **Kanten** sind anklickbar (T0201). Die Fläche zählt
        // nicht — sonst bliebe über einem Bild kein Platz zum Zeichnen, und
        // jeder Zeiger darüber höbe es hervor. Getroffen wird wie eine Linie
        // (Priorität 1).
        if let EntityKind::Image {
            a: ia,
            b: ib,
            height,
            lock_ratio,
            data,
            ..
        } = &e.kind
        {
            let hoehe = crate::bild::dekodiere(data)
                .map(|bild| crate::bild::wirksame_hoehe(&bild, ia.dist(*ib), *height, *lock_ratio))
                .unwrap_or(1.0);
            let d = crate::bild::kantenabstand(*ia, *ib, hoehe, p);
            if d <= tol {
                let prio = 1;
                if best
                    .map(|(_, bd, bp)| prio < bp || (d < bd && prio <= bp))
                    .unwrap_or(true)
                {
                    best = Some((e.id, d, prio));
                }
            }
            continue;
        }
        // Text/Punkt: über BBox
        let is_text = matches!(e.kind, EntityKind::Text { .. } | EntityKind::Point { .. });
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
            EntityKind::Wall { .. }
                | EntityKind::Fill { .. }
                | EntityKind::Volume { .. }
                | EntityKind::Stair { .. }
        );
        if filled {
            if let EntityKind::Wall { .. } = &e.kind {
                // Trefferfläche ist der **Wandkörper** um die versetzte Achse,
                // nicht die halbe Dicke um die Basislinie: bei den
                // Ausrichtungen Links/Rechts lag die Hälfte des Körpers außer
                // Reichweite (T0207).
                if let Some(f) = crate::wall::Frame::of(e) {
                    let d = f.band_abstand(p);
                    if d <= tol {
                        if best.map(|(_, bd, _)| d < bd).unwrap_or(true) {
                            best = Some((e.id, d, 0));
                        }
                        continue;
                    }
                }
            }
            // **Volumen**: auch *in* der Fläche getroffen (Inseln zählen als
            // außen) — ein Körper lässt sich sonst nur an seiner Kante
            // anklicken und damit nicht über die Eckpunkte ändern. Für die
            // Füllfläche bleibt es beim Rand-Treffer: der Farbeimer fragt
            // dieselbe Funktion und soll in einer gefüllten Fläche weiter
            // füllen können, nicht „auf einer Linie" melden.
            if let EntityKind::Volume { boundary, holes, .. } = &e.kind {
                if crate::area::point_in_area(p, boundary, holes) {
                    if best.map(|(_, _, bp)| bp > 0).unwrap_or(true) {
                        best = Some((e.id, 0.0, 0));
                    }
                    continue;
                }
            }
            // Füllfläche: nur beim Zerlegen zählt die Innenfläche (siehe
            // `hit_test_flaeche`). Sie ist der schwächste Treffer und wird erst
            // genommen, wenn kein Objekt darüber getroffen wurde.
            if flaeche_innen {
                if let EntityKind::Fill {
                    boundary, holes, ..
                } = &e.kind
                {
                    if crate::area::point_in_area(p, boundary, holes) {
                        if innen.is_none() {
                            innen = Some(e.id);
                        }
                        continue;
                    }
                }
            }
        }
        let mut dmin = f64::INFINITY;
        // Dämmung: die Linienart zeichnet ein Schlaufenband **versetzt** zur
        // gespeicherten Linie. Der Klick muss deshalb die gezeichnete
        // Geometrie treffen (wie bei der Bemaßung, T0052/T0120), sonst läge
        // die Trefferfläche auf einer unsichtbaren Geraden.
        let segmente = if doc.effective_line_type(e) == LineType::Insulation {
            render::trefferbild(doc, e).segmente
        } else {
            render::entity_segments(doc, e)
        };
        for sg in segmente {
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
    best.map(|(id, _, _)| id).or(innen)
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
    for e in doc.ents() {
        if doc.layer_gesperrt(e.layer) { continue; }
        let bb = render::entity_bbox(doc, e);
        if !bb.is_valid() {
            continue;
        }
        // Füllfläche und Volumen: geprüft wird die gefüllte Geometrie, nicht die
        // Bounding-Box. Bei einer konkaven Fläche deckt die Box auch die
        // Aussparung ab — ein Querfenster darin wählte die Fläche mit, obwohl
        // dort nichts gezeichnet ist (T0099). Vollständig enthalten heißt
        // weiterhin: die Box liegt im Fenster (bei einem Rechteck gleichwertig
        // zur Punktprobe, weil das Fenster konvex ist).
        if let EntityKind::Fill { boundary, holes, .. }
        | EntityKind::Volume { boundary, holes, .. } = &e.kind
        {
            let trifft = if crossing {
                crate::area::flaeche_beruehrt_rechteck(boundary, holes, &rect)
            } else {
                rect.contains_bbox(&bb)
            };
            if trifft {
                out.push(e.id);
            }
            continue;
        }
        // Bild: nur wenn das Fenster eine seiner Kanten berührt (T0201) —
        // für Fenster und Querfenster dieselbe Regel. Ein Fenster ganz
        // **innerhalb** des Bildes hat keine Kante berührt und wählt es nicht
        // mit; ein Fenster um das Bild deckt alle Kanten ab und wählt es.
        // Geprüft wird das gedrehte Rechteck, nicht seine Hülle.
        if let EntityKind::Image {
            a, b, height, lock_ratio, data, ..
        } = &e.kind
        {
            let hoehe = crate::bild::dekodiere(data)
                .map(|bild| crate::bild::wirksame_hoehe(&bild, a.dist(*b), *height, *lock_ratio))
                .unwrap_or(1.0);
            if crate::bild::kanten_beruehrt(*a, *b, hoehe, &rect) {
                out.push(e.id);
            }
            continue;
        }
        // Punkt, Text und Treppe: die Bounding-Box ist ihre Trefferfläche
        // (Punkt ohne Ausdehnung, Text als Zeilenbox, Treppe als Rechteck).
        let flaechig = matches!(
            e.kind,
            EntityKind::Stair { .. }
                | EntityKind::Text { .. }
                | EntityKind::Point { .. }
                // Ansichtsfenster: die Fläche ist das Rechteck (T0109).
                | EntityKind::Viewport(_)
        );
        if flaechig {
            if (crossing && bb.intersects(&rect)) || (!crossing && rect.contains_bbox(&bb)) {
                out.push(e.id);
            }
            continue;
        }
        // Bemaßung: die Maßzahl ist ihre sichtbare Kennzeichnung. Ein Fenster
        // (links→rechts) über der Zahl wählt die Kette, auch wenn Maßlinie und
        // Hilfslinien darüber hinausreichen; das Querfenster berührt die
        // gezeichneten Strecken oder den Maßtext (T0052).
        if matches!(
            e.kind,
            EntityKind::Dim { .. }
                | EntityKind::ElevationPlan { .. }
                | EntityKind::ElevationSection { .. }
        ) {
            let tb = render::trefferbild(doc, e);
            let text_trifft = tb.text_kaesten.iter().any(|k| {
                let kb = BBox::from_points(k);
                if crossing {
                    kb.intersects(&rect)
                } else {
                    rect.contains_bbox(&kb)
                }
            });
            let strecken_innen = !tb.segmente.is_empty()
                && tb
                    .segmente
                    .iter()
                    .flat_map(seg_points)
                    .all(|q| rect.contains(q));
            let strecken_treffen = tb.segmente.iter().any(|sg| {
                let pts = seg_points(sg);
                pts.windows(2).any(|w| seg_meets_rect(w[0], w[1], &rect))
            });
            if text_trifft || if crossing { strecken_treffen } else { strecken_innen } {
                out.push(e.id);
            }
            continue;
        }
        // Wand: der **Körper** zählt (Band um die versetzte Achse), nicht die
        // halbe Dicke um die Basislinie (T0207). Bei den Ausrichtungen
        // Links/Rechts lag die halbe Wand außerhalb jeder Prüfung.
        //
        // Kreuzauswahl: Berührung des Bandes. Fensterauswahl: das Fenster muss
        // den Körper **ganz** umschließen — vorher wurde verlangt, dass alle
        // vier Ecken *des Fensters* im Wandkörper liegen; das ist bei einer
        // Wand praktisch nie der Fall, eine Wand ließ sich damit mit der
        // Fensterauswahl überhaupt nicht wählen.
        if let EntityKind::Wall { .. } = &e.kind {
            let Some(f) = crate::wall::Frame::of(e) else {
                continue;
            };
            let band = f.band();
            let trifft = if crossing {
                crate::area::flaeche_beruehrt_rechteck(&band, &[], &rect)
            } else {
                band.iter().all(|p| rect.contains(*p))
            };
            if trifft {
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

/// Abstand, in dem eine neue Maßkette parallel zu einer vorhandenen sitzt
/// (T0154): 0,75 m in Modell-Millimetern.
pub const DIM_PARALLEL_ABSTAND_MM: f64 = 750.0;

/// Sonderfang beim **Positionieren** einer Maßkette (T0154).
///
/// Liegt der Zeiger im Fangradius einer gedachten Parallelen zu einer
/// vorhandenen Maßkette — im festen Abstand [`DIM_PARALLEL_ABSTAND_MM`] —,
/// liefert der Fang den Punkt auf dieser Parallelen (in Messrichtung des
/// Zeigers, quer gerastet). So sitzen zwei Maßketten übereinander in
/// gleichmäßigem Abstand, ohne dass der Nutzer die Zahl im Panel einstellt.
///
/// `angle` ist die Richtung der neuen Maßlinie (Bogenmaß); `pts` sind die
/// bereits gesetzten Messpunkte und dienen nur der Vollständigkeit halber —
/// die Richtung steht schon fest (T0166).
pub fn dim_parallel(
    doc: &Document,
    pts: &[Vec2],
    angle: f64,
    cursor: Vec2,
    tol: f64,
) -> Option<Vec2> {
    if pts.len() < 2 {
        return None;
    }
    let (dir, vers) = render::dim_achse(angle);
    let mut best: Option<(f64, Vec2)> = None;
    for e in doc.ents() {
        let EntityKind::Dim {
            pts: referenz,
            line_pos,
            angle: ref_angle,
            ..
        } = &e.kind
        else {
            continue;
        };
        if referenz.len() < 2 {
            continue;
        }
        let (ref_dir, ref_vers) = render::dim_achse(*ref_angle);
        // Nur parallele Maßketten sind ein Vorbild — eine schräge Kette gibt
        // keine waagerechte Parallele.
        if dir.cross(ref_dir).abs() > 0.02 {
            continue;
        }
        // Zwei Seiten: die nähere zum Zeiger gewinnt.
        for seite in [1.0, -1.0] {
            let kand = *line_pos + ref_vers * (DIM_PARALLEL_ABSTAND_MM * seite);
            let abstand = (cursor - kand).dot(vers).abs();
            if abstand > tol {
                continue;
            }
            let punkt = cursor - vers * (cursor - kand).dot(vers);
            if best.map(|(d, _)| abstand < d).unwrap_or(true) {
                best = Some((abstand, punkt));
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Document;

    #[test]
    fn polarfang_rastet_nur_in_der_naehe() {
        let base = Vec2::ZERO;
        // 40° liegt nahe an 45° -> rastet ein (seitlich 87 mm, Band 100 mm)
        let nah = Vec2::polar(40f64.to_radians(), 1000.0);
        let hit = nah.polar_snap(base, 45.0, 6.0, 100.0).expect("muss rasten");
        assert!((hit.angle().to_degrees() - 45.0).abs() < 1e-6);
        // 25° ist 20° entfernt -> kein Rasten
        let fern = Vec2::polar(25f64.to_radians(), 1000.0);
        assert!(fern.polar_snap(base, 45.0, 6.0, 100.0).is_none());
    }

    /// Das Winkel-Fenster allein ergäbe einen **Kegel**: mit dem Abstand wächst
    /// der seitliche Fangbereich. Die Breite ist deshalb begrenzt (T0148) —
    /// fern neben dem Strahl rastet nichts ein, nah am Strahl schon, und auf
    /// dem Strahl ist die Länge unbegrenzt.
    #[test]
    fn polarfang_band_ist_in_der_breite_begrenzt() {
        let base = Vec2::ZERO;
        // 1° neben 45°, 10 m entfernt: seitlich 175 mm — der alte Kegel rastete
        // hier ein (Gegenprobe mit Band 200), das Band lässt es nicht zu.
        let fern = Vec2::polar(44f64.to_radians(), 10_000.0);
        assert!(fern.polar_snap(base, 45.0, 6.0, 12.0).is_none(), "Kegel greift wieder");
        assert!(fern.polar_snap(base, 45.0, 6.0, 200.0).is_some(), "Gegenprobe: ohne Band");
        // Gleicher Winkel, 300 mm entfernt: seitlich 5,2 mm — im Band.
        let nah = Vec2::polar(44f64.to_radians(), 300.0);
        let hit = nah.polar_snap(base, 45.0, 6.0, 12.0).expect("muss rasten");
        assert!((hit.angle().to_degrees() - 45.0).abs() < 1e-6);
        // Genau auf dem Strahl bleibt die Länge frei.
        let strahl = Vec2::polar(45f64.to_radians(), 20_000.0);
        assert!(strahl.polar_snap(base, 45.0, 6.0, 12.0).is_some());
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
            start_symbol: None,
            end_symbol: None,
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
        let hit = find_snap_gated(&doc, Vec2::new(4.0, 146.0), 30.0, None, &[], false)
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
                base: crate::model::DoorBase::Left,
            },
            style: Default::default(),
            by_block: false,
            start_symbol: None,
            end_symbol: None,
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
        let hit = find_snap_gated(&doc, Vec2::new(1502.0, 147.0), 30.0, None, &[], false)
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
        let an = find_snap_gated(&doc, cursor, 30.0, None, &[], false);
        assert!(an.is_some());
        // Aus: keine Konturpunkte mehr — und die Achse liefert hier nichts.
        let aus = find_snap_gated(&doc_aus, cursor, 30.0, None, &[], false);
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
        let cands = snap_candidates_gated(&doc, Vec2::new(-150.0, -150.0), 200.0, None, &[], false);
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

    /// Das Element, das gerade bearbeitet wird, steht in `ignore` und liefert
    /// **überhaupt keine** Fangpunkte — auch nicht seine Konturecken. Das ist
    /// der einzige Selbst-Fang-Schutz; eine Prüfung „welcher Punkt wandert mit?"
    /// gibt es nicht mehr.
    #[test]
    fn ignoriertes_element_liefert_gar_nichts() {
        let mut doc = Document::blank();
        let id = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        let cursor = Vec2::new(4.0, 146.0);
        // Ohne Ausschluss fängt die Konturecke …
        assert!(find_snap_gated(&doc, cursor, 30.0, None, &[], false).is_some());
        // … im Ausschluss (bearbeitetes Element) gar nichts.
        assert!(
            find_snap_gated(&doc, cursor, 30.0, None, &[id], false).is_none(),
            "das bearbeitete Element darf keine Fangpunkte liefern"
        );
        assert!(
            snap_candidates_gated(&doc, cursor, 30.0, None, &[id], false)
                .iter()
                .all(|h| h.entity != Some(id)),
            "kein Kandidat darf vom bearbeiteten Element stammen"
        );
    }

    /// Fremde Objekte fangen unverändert — auch wenn ein anderes Element im
    /// Ausschluss steht.
    #[test]
    fn fremde_objekte_fangen_unveraendert() {
        let mut doc = Document::blank();
        let a = wand(&mut doc, Vec2::ZERO, Vec2::new(5000.0, 0.0));
        let b = wand(&mut doc, Vec2::new(0.0, -4000.0), Vec2::new(0.0, -1000.0));
        // Cursor an der Konturecke der *nicht* ausgeschlossenen Wand b.
        let cursor = Vec2::new(147.0, -1004.0);
        let ohne = find_snap_gated(&doc, cursor, 30.0, None, &[], false).expect("Fang an b");
        let mit =
            find_snap_gated(&doc, cursor, 30.0, None, &[a], false).expect("Fang an b mit Ausschluss");
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
        // Das ist der **Basispunkt** des Verschiebens: zu diesem Zeitpunkt
        // wandert noch nichts, die Wand fängt deshalb normal.
        let basis = find_snap_gated(&s.doc, Vec2::new(6.0, 144.0), 30.0, None, &[], false)
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
        let on = find_snap_gated(&doc, p, 20.0, None, &[], true).expect("Rasterfang");
        assert!((on.point.x - 100.0).abs() < 1e-9);
        // Raster ausgeschaltet -> kein Rasterfang mehr
        assert!(find_snap_gated(&doc, p, 20.0, None, &[], false).is_none());
    }
}