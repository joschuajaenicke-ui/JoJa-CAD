//! Wandkörper, Wandanschlüsse und Umgrenzung — eine Quelle für Anzeige und
//! Fang.
//!
//! # Geometrie
//!
//! Eine Wand ist ein Band um ihre Achse: Basislinie `a0→b0`, quer um
//! `align.offset() * Dicke/2` verschoben, halbe Dicke `h`. Zwei Wände sind an
//! einem Ende **verbunden**, wenn ihre *Basispunkte* zusammenfallen
//! (Toleranz [`JOIN_EPS`]) — nicht, wenn sich ihre Körper irgendwie berühren.
//!
//! An einer verbundenen Ecke wird jede Wand an der **Anschlussnaht**
//! abgeschnitten: der Geraden durch die beiden Gehrungspunkte, also durch die
//! Schnittpunkte der beiden *gleichseitigen* Kanten (Außenkante an
//! Außenkante, Innenkante an Innenkante). Bei gleich dicken, mittigen Wänden
//! ist das genau die Winkelhalbierende zwischen den beiden Achsen; bei
//! unterschiedlicher Dicke oder Ausrichtung ist es die dicken- und
//! ausrichtungsabhängige Halbierende. Beide Wände rechnen **dieselbe** Naht
//! (Schnitt zweier Geraden — unabhängig davon, welche Wand fragt); deshalb
//! schließen ihre Körper lückenlos aneinander, ohne Patch oder Keil.
//!
//! Geschnitten wird nur, wenn der Schnitt beidseitig gedeckt ist: was die Naht
//! von einer Wand abschneidet, muss im Körper der anderen liegen. Sonst bleibt
//! die Wand stehen (Stumpfstoß) — bei sehr spitzen Ecken oder weit versetzten
//! Achsen ist der Schnitt sonst eine Behauptung, die die Füllung nicht deckt.
//!
//! # Umgrenzung
//!
//! Gezeichnet wird die **Vereinigung** aller Wandkörper: eine Randkante wird
//! nur dort gezeichnet, wo auf ihrer Außenseite keine andere Wand liegt. Damit
//! verschwindet die Naht im Wandinneren von selbst, und die Umrandung schließt
//! sich an der Ecke, ohne Sonderfall für Gehrung, Stumpfstoß, Stich oder Kreuz.
//! Dieselbe Regel liefert die Kontur für den Punktfang ([`wall_contour_segments`]).

use crate::geom::*;
use crate::model::{Document, Entity, EntityKind, Id};
use std::cell::OnceCell;
use std::collections::HashMap;

/// Toleranz, unter der zwei Basispunkte als **verbunden** gelten (mm).
///
/// Darunter liegen nur Rundungsreste (Datei, Verschieben, Drehen). Alles
/// darüber ist ein sichtbarer Spalt und damit kein Anschluss — sonst hinge die
/// Entscheidung an der Wanddicke.
pub const JOIN_EPS: f64 = 0.5;

/// Reichweite der Naht als Vielfaches der halben Wanddicke: so weit wird der
/// Körper am verbundenen Ende über die Stirnebene hinaus verlängert, bevor die
/// Naht ihn schneidet (die Gehrung **ergänzt** die Ecke, sie schneidet sie
/// nicht nur ab).
pub const MITER_LIMIT: f64 = 6.0;

/// Grenze der Gehrung: Nahtlänge geteilt durch die Wandbreite (2·h) der
/// dünneren Wand. Wird die Naht länger (extrem spitze Ecke), wird stumpf
/// gestoßen (Bevel) — sonst schießt die Spitze meterweit aus.
pub const MITER_RATIO: f64 = 6.0;

/// Kleinster **Öffnungswinkel** (Grad zwischen den beiden Wänden am Knoten:
/// 180 = fluchtend, 90 = Ecke, 20 = sehr spitz), ab dem gegehrt wird.
///
/// Darunter liegt die Naht so weit vom Knoten entfernt, dass die Wand als
/// meterlange „Finne" über ihn hinausschießt — dort wird stumpf gestoßen.
pub const MIN_JOINT_ANGLE: f64 = 40.0;

/// Halbe Breite des Füllbands an einer Naht (mm). Deckt die
/// Antialiasing-Naht zwischen zwei Wandfüllungen ab; liegt vollständig im
/// Nachbarkörper bzw. unter der Außenlinie.
pub const FILL_BLEED: f64 = 0.6;

/// Tiefe der Außenprobe (mm): so weit neben einer Kante wird geprüft, ob dort
/// eine andere Wand liegt. Klein gegen jede Wanddicke, groß gegen Rundung.
const PROBE: f64 = 0.08;

/// Rasterweite der Nachbarsuche (mm). Nur ein Grobfilter; entschieden wird auf
/// den Kandidaten der Zellen.
const GRID_MM: f64 = 1000.0;


/// Zellen des Grobrasters, die eine Fläche überdecken.
fn grid_cells(bb: &BBox) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    if !bb.is_valid() || !bb.min.x.is_finite() || !bb.min.y.is_finite() {
        return out;
    }
    let c = |p: f64| (p / GRID_MM).floor() as i64;
    let (x0, x1) = (c(bb.min.x), c(bb.max.x));
    let (y0, y1) = (c(bb.min.y), c(bb.max.y));
    // Ausreißer nicht in Zellen zerlegen — dann ohne Grobfilter.
    if (x1 - x0).abs() > 2048 || (y1 - y0).abs() > 2048 {
        return out;
    }
    for x in x0..=x1 {
        for y in y0..=y1 {
            out.push((x, y));
        }
    }
    out
}

/// Achsrahmen einer Wand: Basislinie, verschobene Achse, Richtung, Normale,
/// halbe Dicke.
///
/// `a0`/`b0` sind die **Basislinie** — die Endpunkte, wie sie im Modell stehen.
/// `a`/`b` ist dieselbe Achse, um die Ausrichtung (links/mittig/rechts)
/// verschoben; darauf steht der Wandkörper. Verbunden wird über die
/// Basislinie, gezeichnet über die verschobene Achse.
#[derive(Clone, Copy)]
pub struct Frame {
    pub a0: Vec2,
    pub b0: Vec2,
    pub a: Vec2,
    pub b: Vec2,
    pub dir: Vec2,
    pub n: Vec2,
    pub h: f64,
}

impl Frame {
    pub fn of(e: &Entity) -> Option<Self> {
        let EntityKind::Wall {
            a,
            b,
            thickness,
            align,
            ..
        } = &e.kind
        else {
            return None;
        };
        if *thickness <= 0.5 || !thickness.is_finite() {
            return None;
        }
        let len = a.dist(*b);
        if len < 1e-6 {
            return None;
        }
        let dir = (*b - *a).normalized();
        let n = dir.perp();
        let h = thickness * 0.5;
        let shift = n * (h * align.offset());
        Some(Self {
            a0: *a,
            b0: *b,
            a: *a + shift,
            b: *b + shift,
            dir,
            n,
            h,
        })
    }

    pub fn len(&self) -> f64 {
        self.a0.dist(self.b0)
    }

    /// Gerade einer Wandkante: `side` = +1 linke, −1 rechte Kante (in
    /// Richtung `dir`), jeweils auf der verschobenen Achse.
    fn edge(&self, side: f64) -> (Vec2, Vec2) {
        (self.a + self.n * (self.h * side), self.dir)
    }

    /// Band (Achse ± halbe Dicke) als Viereck.
    pub fn band(&self) -> Vec<Vec2> {
        vec![
            self.a + self.n * self.h,
            self.b + self.n * self.h,
            self.b - self.n * self.h,
            self.a - self.n * self.h,
        ]
    }

    /// Achsabschnitt `s0..s1` (ab `a` gemessen) mit voller Dicke.
    fn quad(&self, s0: f64, s1: f64) -> Vec<Vec2> {
        let p0 = self.a + self.dir * s0;
        let p1 = self.a + self.dir * s1;
        vec![
            p0 + self.n * self.h,
            p1 + self.n * self.h,
            p1 - self.n * self.h,
            p0 - self.n * self.h,
        ]
    }
}

/// Anschlussnaht an einem Wandende: Gerade durch `p`, von der die Wand die
/// Seite `nrm` behält (zeigt in den eigenen Körper).
#[derive(Clone, Copy, Debug)]
pub struct Seam {
    pub p: Vec2,
    pub nrm: Vec2,
    /// Gehrungspunkte (innen, außen) — für Prüfungen und Tests.
    pub inner: Vec2,
    pub outer: Vec2,
    /// Die Wand auf der anderen Seite der Naht.
    pub partner: Id,
}

/// Körper einer Wand: die Flächen zwischen den Öffnungen, jeweils an den
/// verbundenen Enden an der Naht abgeschnitten.
#[derive(Clone, Debug)]
pub struct Body {
    pub pieces: Vec<Vec<Vec2>>,
    pub openings: Vec<(f64, f64)>,
    /// Anschlussnähte (innen, außen) — die Kanten des Körpers darauf sind
    /// Naht, keine Umgrenzung.
    pub seams: Vec<(Vec2, Vec2)>,
    /// Nachbarwand je Naht (gleiche Reihenfolge wie `seams`).
    pub seam_partner: Vec<Id>,
    /// Wie `pieces`, aber die Nahtkanten sind um `FILL_BLEED` in die
    /// Nachbarwand ausgebucht — **nur zum Füllen**. Geometrie, Umrandung und
    /// Fang nutzen `pieces`.
    pub fills: Vec<Vec<Vec2>>,
    /// Ende ist angeschlossen (Naht oder Ende auf der Basislinie eines
    /// Nachbarn): dort gibt es keine Stirnkante in der Kontur.
    pub end_conn: [bool; 2],
    pub start: Vec2,
    pub dir: Vec2,
    pub n: Vec2,
    pub h: f64,
    pub total: f64,
}

impl Body {
    /// Körper einer Wand **ohne** Nachbarn — für Vorschau, Blöcke und Objekte
    /// außerhalb des Index.
    pub fn isolated(doc: &Document, e: &Entity) -> Option<Self> {
        let f = Frame::of(e)?;
        Self::build(doc, e, &f, &[], &[], [false, false])
    }

    fn build(
        doc: &Document,
        e: &Entity,
        f: &Frame,
        se_start: &[Seam],
        se_end: &[Seam],
        conn: [bool; 2],
    ) -> Option<Self> {
        let total = f.len();
        let openings: Vec<(f64, f64)> = crate::render::wall_opening_intervals(doc, e)
            .into_iter()
            .map(|(s0, s1)| (s0.clamp(0.0, total), s1.clamp(0.0, total)))
            .collect();
        let mut segments: Vec<(f64, f64)> = Vec::new();
        let mut cursor = 0.0;
        for (s0, s1) in &openings {
            if *s0 > cursor + 0.5 {
                segments.push((cursor, *s0));
            }
            cursor = cursor.max(*s1);
        }
        if cursor < total - 0.5 {
            segments.push((cursor, total));
        }
        if segments.is_empty() {
            return None;
        }
        // An einem verbundenen Ende wird der Körper über die Stirnebene hinaus
        // verlängert und dann an der Naht beschnitten: die Gehrung **ergänzt**
        // die Ecke (Außenecke hinter dem Wandende), sie schneidet sie nicht nur ab.
        let reach = (MITER_LIMIT * f.h * 1.5).max(10.0);
        let mut pieces = Vec::new();
        let mut fills = Vec::new();
        for (s0, s1) in segments {
            let mut lo = s0;
            let mut hi = s1;
            let at_start = s0 < 0.5;
            let at_end = (s1 - total).abs() < 0.5;
            if at_start && !se_start.is_empty() {
                lo -= reach;
            }
            if at_end && !se_end.is_empty() {
                hi += reach;
            }
            let mut poly = f.quad(lo, hi);
            if at_start {
                for s in se_start {
                    poly = clip_keep_side(&poly, s.p, s.nrm);
                }
            }
            if at_end {
                for s in se_end {
                    poly = clip_keep_side(&poly, s.p, s.nrm);
                }
            }
            poly = clean_polygon(poly);
            if poly.len() >= 3 && polygon_area(&poly).abs() > 1.0 {
                let naehte: Vec<(Vec2, Vec2)> = se_start
                    .iter()
                    .chain(se_end.iter())
                    .map(|s| (s.inner, s.outer))
                    .collect();
                fills.push(bulge_seam(&poly, &naehte, FILL_BLEED));
                pieces.push(poly);
            }
        }
        if pieces.is_empty() {
            return None;
        }
        Some(Self {
            pieces,
            fills,
            openings,
            seams: se_start
                .iter()
                .chain(se_end.iter())
                .map(|s| (s.inner, s.outer))
                .collect(),
            seam_partner: se_start
                .iter()
                .chain(se_end.iter())
                .map(|s| s.partner)
                .collect(),
            end_conn: [conn[0] || !se_start.is_empty(), conn[1] || !se_end.is_empty()],
            start: f.a,
            dir: f.dir,
            n: f.n,
            h: f.h,
            total,
        })
    }

    /// Randstücke dieses Körpers ohne Nachbarn (alles frei).
    pub fn edges(&self) -> Vec<EdgePiece> {
        let mut out = Vec::new();
        for poly in &self.pieces {
            for k in 0..poly.len() {
                let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
                if p.dist(q) < 1e-6 {
                    continue;
                }
                out.push(EdgePiece {
                    a: p,
                    b: q,
                    out: outward(p, q),
                    covered: false,
                });
            }
        }
        out
    }
}

/// Randstück einer Wand: Strecke, Außennormale und ob außen eine andere Wand
/// liegt (dann ist das Stück Naht und wird nicht gezeichnet).
#[derive(Clone, Copy, Debug)]
pub struct EdgePiece {
    pub a: Vec2,
    pub b: Vec2,
    pub out: Vec2,
    pub covered: bool,
}

/// Außennormale der Kante p→q eines Polygons gegen den Uhrzeigersinn.
fn outward(p: Vec2, q: Vec2) -> Vec2 {
    (q - p).perp().normalized() * -1.0
}

/// Polygon gegen den Uhrzeigersinn, ohne doppelte Punkte.
fn clean_polygon(poly: Vec<Vec2>) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(poly.len());
    for p in poly {
        if out.last().map(|q| q.dist(p) > 1e-6).unwrap_or(true) {
            out.push(p);
        }
    }
    while out.len() > 1 && out[0].dist(*out.last().unwrap()) < 1e-6 {
        out.pop();
    }
    if out.len() >= 3 && polygon_area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// Behält den Teil eines konvexen Polygons auf der Seite `(p - p0)·nrm >= 0`
/// (Sutherland-Hodgman, eine Halbebene).
pub fn clip_keep_side(poly: &[Vec2], p0: Vec2, nrm: Vec2) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(poly.len() + 2);
    for k in 0..poly.len() {
        let a = poly[k];
        let b = poly[(k + 1) % poly.len()];
        let (da, db) = ((a - p0).dot(nrm), (b - p0).dot(nrm));
        if da >= 0.0 {
            out.push(a);
        }
        if (da > 0.0 && db < 0.0) || (da < 0.0 && db > 0.0) {
            let t = da / (da - db);
            out.push(a + (b - a) * t);
        }
    }
    out.dedup_by(|a, b| a.dist(*b) < 1e-6);
    if out.len() > 1 && out[0].dist(*out.last().unwrap()) < 1e-6 {
        out.pop();
    }
    out
}

/// Die beiden Gehrungspunkte eines Wandpaars am gemeinsamen Basispunkt.
///
/// Rechnung wie in `docs/WANDANSCHLUSS.md`: `S_l` ist der Schnitt der beiden
/// **linken** Kanten, `S_r` der Schnitt der beiden **rechten** — „links"
/// bezogen auf die Laufrichtung am Knoten. Die eine Wand läuft auf den Knoten
/// **zu** (ihr Ende liegt dort), die andere **von ihm weg** (ihr Anfang liegt
/// dort); daraus folgt die Seite je Wand, und beide Wände rechnen dieselben
/// zwei Punkte.
///
/// Bei gleich dicken, mittigen Wänden liegt die Naht damit auf der
/// Winkelhalbierenden, bei beliebiger Dicke und beliebigem Versatz auf der
/// verallgemeinerten Halbierenden (`x'_l = (b_l + a_l·cos α)/sin α`,
/// `x'_r = −(b_r + a_r·cos α)/sin α` im lokalen System der ankommenden Wand).
fn miter_points(f: &Frame, d: Vec2, g: &Frame, dj: Vec2) -> Option<(Vec2, Vec2)> {
    // Linke Kante, bezogen auf die Laufrichtung `d` bzw. `dj` (vom Knoten weg).
    let si = if d.perp().dot(f.n) >= 0.0 { 1.0 } else { -1.0 };
    let sj = if dj.perp().dot(g.n) >= 0.0 { 1.0 } else { -1.0 };
    // `f` läuft auf den Knoten zu (Seite gespiegelt), `g` von ihm weg.
    let (p1, r1) = f.edge(-si);
    let (p2, r2) = g.edge(sj);
    let a = line_line(p1, p1 + r1, p2, p2 + r2, true, true)?;
    let (p1, r1) = f.edge(si);
    let (p2, r2) = g.edge(-sj);
    let b = line_line(p1, p1 + r1, p2, p2 + r2, true, true)?;
    Some((a, b))
}

/// Darf die Naht hier angewendet werden?
///
/// Die Naht selbst ist reine Geometrie (Schnitte der gleichseitigen Kanten).
/// Angewendet wird sie nur, wenn sie **kaum Material wegnimmt**: geschnitten
/// wird an zwei verlängerten Bändern, danach wird gemessen, wie viel Fläche der
/// Vereinigung der ungeschnittenen Bänder dabei verloren geht. Ein Zipfel an
/// der Ecke (schräge Nachbarstirn) ist zulässig — eine Fläche in der Größe
/// einer Wandquerschnittsfläche nicht: dann liegen die Körper nebeneinander
/// statt übereinander (z. B. Wände auf entgegengesetzten Seiten ihrer
/// Basislinien) und der Schnitt würde ein Loch reißen.
fn cut_ok(f: &Frame, d: Vec2, g: &Frame, dj: Vec2, inner: Vec2, outer: Vec2) -> bool {
    let sd = outer - inner;
    if sd.len() < 1e-9 {
        return false;
    }
    // Behaltene Seite je Wand (diejenige, in der der von der Naht entfernte
    // Wandteil nicht liegt).
    let keep = |fr: &Frame, dd: Vec2| {
        let far = if dd.dot(fr.dir) > 0.0 { fr.b0 } else { fr.a0 };
        let mut nrm = sd.perp().normalized();
        if (far - inner).dot(nrm) < 0.0 {
            nrm = -nrm;
        }
        nrm
    };
    let (bm_f, bm_g) = (band_ext(f, d), band_ext(g, dj));
    // Vergleichsfläche ist der **echte** Wandkörper (ohne Verlängerung): die
    // Verlängerung ist nur Baugerüst für den Schnitt.
    let (bf, bg) = (f.band(), g.band());
    let (nf, ng) = (keep(f, d), keep(g, dj));
    let (tf, tg) = (
        clip_keep_side(&bm_f, inner, nf),
        clip_keep_side(&bm_g, inner, ng),
    );
    // Verlorene Fläche = was von den **echten** Bändern nach dem Schnitt weder
    // im eigenen beschnittenen Körper noch im verlängerten Band des Nachbarn
    // liegt. Analytisch (konvexe Schnitte), nicht abgetastet: der Treffer-Test
    // fragt die Bounding-Box jeder Wand ab, das muss billig bleiben.
    let _ = (&tf, &tg);
    let verloren = verlust(&bf, inner, -nf, &bm_g) + verlust(&bg, inner, -ng, &bm_f);
    // Verlängerung über das eigene Ende hinaus, die der Nachbar nicht deckt:
    // das ist die „Finne" bei sehr spitzem Knick (die Wand schießt über den
    // Knoten hinaus ins Leere).
    // Behaltene Seite (`nf` zeigt in den Körper): der Überstand, der stehen
    // bleibt, ist der Überstand auf dieser Seite.
    let _ = (&tf, &tg);
    let h_min = f.h.min(g.h);
    // Bezug: Querschnittsfläche der dünneren Wand (2h)², davon ein Viertel.
    // (Ein Zipfel von wenigen Prozent bleibt zulässig, eine halbe Wand nicht.)
    verloren <= 0.25 * (2.0 * h_min).powi(2)
}

/// Fläche des echten Bandes `band`, die nach dem Schnitt weder in `behalten`
/// (eigener beschnittener Körper) noch in `nachbar` (verlängertes Band der
/// anderen Wand) liegt.
fn verlust(band: &[Vec2], inner: Vec2, weg: Vec2, nachbar: &[Vec2]) -> f64 {
    let cut = clip_keep_side(band, inner, weg);
    if cut.len() < 3 {
        return 0.0;
    }
    let a = polygon_area(&cut).abs();
    let gedeckt = polygon_area(&convex_clip(&cut, nachbar)).abs();
    (a - gedeckt).max(0.0)
}

/// Nahtkante einer Fläche um `w` in die Nachbarwand ausbeulen.
///
/// Die Kante bleibt an ihren Endpunkten stehen (kein Zipfel über die Ecke
/// hinaus), nur die Strecke dazwischen wandert um `w` hinaus. Damit überlappen
/// die Füllungen beider Wände an der Naht, statt Kante auf Kante zu stoßen —
/// sonst bleibt bei Unterpixel-Lage ein heller Haarstrich, der beim Ziehen
/// flackert. Eine eigene, dünne Auflage taugt dafür nicht: die GUI verwirft
/// Füllungen unter 0,75 px, dann bliebe der Haarstrich stehen.
fn bulge_seam(poly: &[Vec2], seams: &[(Vec2, Vec2)], w: f64) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(poly.len() + 4);
    let n = poly.len();
    for k in 0..n {
        let a = poly[k];
        let b = poly[(k + 1) % n];
        out.push(a);
        let Some((s0, s1)) = seams
            .iter()
            .find(|(s0, s1)| on_seam(a, *s0, *s1) && on_seam(b, *s0, *s1))
            .copied()
        else {
            continue;
        };
        let d = (s1 - s0).normalized();
        let mut m = d.perp();
        // `m` zeigt aus der Fläche hinaus (in die Nachbarwand)
        if polygon_contains((a + b) * 0.5 + m * 0.5, poly) {
            m = -m;
        }
        out.push(a + m * w);
        out.push(b + m * w);
        out.push(b);
    }
    out
}

fn on_seam(p: Vec2, s0: Vec2, s1: Vec2) -> bool {
    dist_point_segment(p, s0, s1) < 1e-3
}

fn polygon_contains(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if x > p.x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Schnittfläche zweier konvexer Polygone (Sutherland-Hodgman).
fn convex_clip(subject: &[Vec2], clip: &[Vec2]) -> Vec<Vec2> {
    let mut out = subject.to_vec();
    if polygon_area(clip) < 0.0 {
        let mut c = clip.to_vec();
        c.reverse();
        return convex_clip(subject, &c);
    }
    for k in 0..clip.len() {
        if out.len() < 3 {
            return Vec::new();
        }
        let (a, b) = (clip[k], clip[(k + 1) % clip.len()]);
        out = clip_keep_side(&out, a, (b - a).perp());
    }
    out
}

/// Band einer Wand, am Knotenende um die Reichweite der Naht verlängert.
fn band_ext(f: &Frame, d: Vec2) -> Vec<Vec2> {
    let reach = (MITER_LIMIT * f.h * 1.5).max(10.0);
    let (p, q) = if d.dot(f.dir) > 0.0 {
        (f.a - f.dir * reach, f.b)
    } else {
        (f.a, f.b + f.dir * reach)
    };
    vec![p + f.n * f.h, q + f.n * f.h, q - f.n * f.h, p - f.n * f.h]
}

/// Alle Wände eines Durchgangs, einmal aufgebaut: Rahmen, Nachbarschafts-
/// raster, Körper und Zugriff über die Id.
pub struct WallIndex<'a> {
    doc: &'a Document,
    ents: &'a [Entity],
    frames: Vec<(Id, Frame)>,
    /// Id → Index in `frames`
    by_id: HashMap<Id, usize>,
    /// Rasterzelle → Indizes in `frames`
    grid: HashMap<(i64, i64), Vec<usize>>,
    /// Körper je Wand (parallel zu `frames`) — erst bei Bedarf gerechnet.
    /// Die Nahtprüfung tastet Flächen ab; sie für **alle** Wände eines
    /// Durchgangs zu rechnen kostet mehr, als die meisten Abfragen brauchen
    /// (Bounding-Box, Treffer-Test, Vorschau einer einzelnen Wand).
    bodies: Vec<OnceCell<Option<Body>>>,
}

impl<'a> WallIndex<'a> {
    /// Index über alle Wände der Liste.
    pub fn build(doc: &'a Document, ents: &'a [Entity]) -> Self {
        Self::build_in(doc, ents, None)
    }

    /// Index nur über Wände, deren Band eine Fläche berührt.
    pub fn build_near(doc: &'a Document, ents: &'a [Entity], area: &BBox) -> Self {
        Self::build_in(doc, ents, Some(*area))
    }

    fn build_in(doc: &'a Document, ents: &'a [Entity], only: Option<BBox>) -> Self {
        let mut frames: Vec<(Id, Frame)> = Vec::new();
        let mut by_id: HashMap<Id, usize> = HashMap::new();
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for e in ents {
            let Some(f) = Frame::of(e) else { continue };
            let bb = BBox::from_points(&f.band());
            if let Some(area) = only {
                if !bb.intersects(&area) {
                    continue;
                }
            }
            let i = frames.len();
            for key in grid_cells(&bb) {
                grid.entry(key).or_default().push(i);
            }
            by_id.insert(e.id, i);
            frames.push((e.id, f));
        }
        let mut idx = Self {
            doc,
            ents,
            frames,
            by_id,
            grid,
            bodies: Vec::new(),
        };
        idx.bodies = (0..idx.frames.len()).map(|_| OnceCell::new()).collect();
        idx
    }

    fn frame(&self, i: usize) -> Option<&Frame> {
        self.frames.get(i).map(|(_, f)| f)
    }

    fn at(&self, i: usize) -> Option<(Id, &Frame)> {
        self.frames.get(i).map(|(id, f)| (*id, f))
    }

    /// Dokument, aus dem der Index gebaut wurde.
    pub fn doc(&self) -> &'a Document {
        self.doc
    }

    fn ent(&self, id: Id) -> Option<&'a Entity> {
        let i = *self.by_id.get(&id)?;
        let _ = self.frames.get(i)?;
        self.ents.iter().find(|e| e.id == id)
    }

    /// Wandindizes, deren Band im Umkreis `r` um `at` liegt.
    fn around(&self, at: Vec2, r: f64) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for key in grid_cells(&BBox::from_point(at).expanded(r)) {
            if let Some(list) = self.grid.get(&key) {
                for i in list {
                    if !out.contains(i) {
                        out.push(*i);
                    }
                }
            }
        }
        out
    }

    /// Körper einer Wand — `None`, wenn die Wand nicht im Index liegt
    /// (Vorschau, Blockinhalt) oder zu dünn ist.
    pub fn body(&self, id: Id) -> Option<&Body> {
        let i = *self.by_id.get(&id)?;
        self.bodies.get(i)?.get_or_init(|| self.build_body(i)).as_ref()
    }

    /// Nahtstellen an einem Wandende.
    ///
    /// Läuft eine Wand am Knoten **gerade durch** (ein Nachbar setzt sie
    /// fluchtend fort), wird nicht geschnitten: die Wand läuft durch, wie sie
    /// gezeichnet ist. Sonst bekämen Kreuz und T-Stoß eine Gehrung mitten im
    /// Körper.
    fn seams(&self, i: usize, f: &Frame, start: bool) -> Vec<Seam> {
        let p = if start { f.a0 } else { f.b0 };
        // `d` läuft vom Knoten **in** die Wand (wie `dj` beim Nachbarn).
        let d = if start { f.dir } else { -f.dir };
        let mut partners: Vec<(Id, Frame, Vec2)> = Vec::new();
        for j in self.around(p, JOIN_EPS) {
            if j == i {
                continue;
            }
            let Some(g) = self.frame(j) else { continue };
            let Some((gid, _)) = self.at(j) else { continue };
            let dj = if g.a0.dist(p) <= JOIN_EPS {
                g.dir
            } else if g.b0.dist(p) <= JOIN_EPS {
                -g.dir
            } else {
                continue;
            };
            let cr = d.cross(dj);
            if cr.abs() < 1e-3 {
                // Fluchtende Fortsetzung (gleiche Richtung): kein Schnitt.
                // Rückläufige Doppelwand: ebenfalls nicht.
                if d.dot(dj) > 0.0 {
                    return Vec::new();
                }
                continue;
            }
            partners.push((gid, *g, dj));
        }
        let far = if start { f.b0 } else { f.a0 };
        let mut out = Vec::new();
        for (gid, g, dj) in partners {
            let Some((inner, outer)) = miter_points(f, d, &g, dj) else {
                continue;
            };
            // Öffnungswinkel zwischen den Wänden: `d` und `dj` zeigen beide
            // vom Knoten in ihre Wand — ihr Winkel ist der Winkel, den die
            // Wände einschließen (180° = fluchtend, 90° = Ecke, 20° = spitz).
            let psi = d.dot(dj).clamp(-1.0, 1.0).acos().to_degrees();
            if psi < MIN_JOINT_ANGLE - 1e-6 {
                continue;
            }
            // Grenzen für **beide** Wände dieselbe (die dünnere/kürzere zählt):
            // sonst schneidet die eine Wand und die andere nicht — und die
            // Körper überlappen sichtbar.
            if inner.dist(outer) > MITER_RATIO * 2.0 * f.h.min(g.h) {
                continue;
            }
            if inner.dist(p).max(outer.dist(p)) > f.len().min(g.len()).max(1.0) {
                continue;
            }
            if !cut_ok(f, d, &g, dj, inner, outer) {
                continue;
            }
            let mut nrm = (outer - inner).perp().normalized();
            if (far - inner).dot(nrm) < 0.0 {
                nrm = -nrm;
            }
            if nrm.len() < 0.5 {
                continue;
            }
            out.push(Seam {
                p: inner,
                nrm,
                inner,
                outer,
                partner: gid,
            });
        }
        out
    }

    /// Nähte einer Wand (beide Enden) — für Prüfungen.
    pub fn seams_of(&self, id: Id) -> Vec<Seam> {
        let Some(i) = self.by_id.get(&id).copied() else {
            return Vec::new();
        };
        let Some(f) = self.frame(i).copied() else {
            return Vec::new();
        };
        let mut out = self.seams(i, &f, true);
        out.extend(self.seams(i, &f, false));
        out
    }

    /// Liegt `p` auf der Basislinie einer anderen Wand (Stichanschluss)?
    /// Dann endet die Wand dort im Nachbarkörper — ohne Stirnkante.
    fn on_baseline(&self, own: Id, p: Vec2) -> bool {
        for i in self.around(p, JOIN_EPS) {
            let Some((id, g)) = self.at(i) else { continue };
            if id == own {
                continue;
            }
            if dist_point_segment(p, g.a0, g.b0) <= JOIN_EPS {
                return true;
            }
        }
        false
    }

    fn build_body(&self, i: usize) -> Option<Body> {
        let (id, f) = self.at(i)?;
        let e = self.ent(id)?;
        let se = self.seams(i, f, true);
        let ee = self.seams(i, f, false);
        let conn = [
            self.on_baseline(id, f.a0),
            self.on_baseline(id, f.b0),
        ];
        Body::build(self.doc, e, f, &se, &ee, conn)
    }

    /// Liegt `p` im Körper einer anderen Wand als `own`?
    pub fn covered_point(&self, p: Vec2, own: Id) -> bool {
        for i in self.around(p, 0.0) {
            let Some((id, _)) = self.at(i) else { continue };
            if id == own {
                continue;
            }
            let Some((id2, _)) = self.at(i) else { continue };
            let Some(b) = self.body(id2) else { continue };
            if b.pieces.iter().any(|poly| point_in_polygon(p, poly)) {
                return true;
            }
        }
        false
    }

    /// Liegt `p` im Körper **irgendeiner** Wand? (Anschlussregel für Volumen:
    /// deren Umrandung fällt aus, wo sie in einer Wand liegt.)
    pub fn covered_any(&self, p: Vec2) -> bool {
        for i in self.around(p, 0.0) {
            let Some((id, _)) = self.at(i) else { continue };
            let Some(b) = self.body(id) else { continue };
            if b.pieces.iter().any(|poly| point_in_polygon(p, poly)) {
                return true;
            }
        }
        false
    }

    /// Randstücke einer Wand.
    ///
    /// `split` unterteilt die Kanten an den Kanten der Nachbarn — nötig zum
    /// Zeichnen, damit ein Stück, das teils in einer anderen Wand liegt, nur
    /// teilweise ausfällt. Ohne `split` bleibt eine lange Kante ganz (Fang).
    pub fn edges(&self, id: Id, split: bool) -> Vec<EdgePiece> {
        let Some(b) = self.body(id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for poly in &b.pieces {
            let n = poly.len();
            for k in 0..n {
                let (p, q) = (poly[k], poly[(k + 1) % n]);
                if p.dist(q) < 1e-6 {
                    continue;
                }
                let outn = outward(p, q);
                let mut ts: Vec<f64> = vec![0.0, 1.0];
                if split {
                    let bb = BBox::from_points(&[p, q]).expanded(PROBE * 4.0);
                    for other in self.pieces_near(&bb, id) {
                        for m in 0..other.len() {
                            let (r, s) = (other[m], other[(m + 1) % other.len()]);
                            if let Some(x) = line_line(p, q, r, s, false, false) {
                                let t = (x - p).dot(q - p) / (q - p).len2();
                                if t > 1e-6 && t < 1.0 - 1e-6 {
                                    ts.push(t);
                                }
                            }
                        }
                    }
                    ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
                }
                for w in ts.windows(2) {
                    let (t0, t1) = (w[0], w[1]);
                    if t1 - t0 < 1e-9 {
                        continue;
                    }
                    let a = p + (q - p) * t0;
                    let bb2 = p + (q - p) * t1;
                    let mid = p + (q - p) * (0.5 * (t0 + t1));
                    let covered = self.covered_point(mid + outn * PROBE, id);
                    out.push(EdgePiece {
                        a,
                        b: bb2,
                        out: outn,
                        covered,
                    });
                }
            }
        }
        out
    }

    /// Körperflächen der Wände außer `own`, die eine Fläche berühren.
    fn pieces_near(&self, bb: &BBox, own: Id) -> Vec<&[Vec2]> {
        let mut out: Vec<&[Vec2]> = Vec::new();
        let mut seen: Vec<usize> = Vec::new();
        for key in grid_cells(bb) {
            let Some(list) = self.grid.get(&key) else {
                continue;
            };
            for i in list {
                if seen.contains(i) {
                    continue;
                }
                seen.push(*i);
                let Some((id, f)) = self.at(*i) else { continue };
                if id == own {
                    continue;
                }
                if !BBox::from_points(&f.band()).intersects(bb) {
                    continue;
                }
                let Some((id2, _)) = self.at(*i) else { continue };
                if let Some(b) = self.body(id2) {
                    for poly in &b.pieces {
                        out.push(poly.as_slice());
                    }
                }
            }
        }
        out
    }

    /// Füllflächen und Randstücke einer Wand — für Vorschau/Blöcke auch ohne
    /// Index (`None`).
    pub fn shape(&self, id: Id) -> Option<(Vec<Vec<Vec2>>, Vec<EdgePiece>)> {
        let b = self.body(id)?;
        Some((b.pieces.clone(), self.edges(id, true)))
    }
}

/// Umgrenzende Linien einer Wand in Weltkoordinaten (Fang und Prüfung).
///
/// Sichtbare Kontur: Längskanten bis zur Naht, Stirnkanten an freien Enden,
/// Laibungen an Öffnungen — nur dort, wo außen keine andere Wand liegt.
pub fn wall_contour_segments(doc: &Document, e: &Entity) -> Vec<Seg> {
    let idx = WallIndex::build(doc, doc.ents());
    wall_contour_segments_idx(&idx, e)
}

/// Wie [`wall_contour_segments`], aber mit einem bereits gebauten Index.
pub fn wall_contour_segments_idx(idx: &WallIndex, e: &Entity) -> Vec<Seg> {
    if idx.body(e.id).is_none() {
        // Wand nicht im Index (Block, Vorschau): ohne Nachbarn.
        let Some(b) = Body::isolated(idx.doc(), e) else {
            return vec![];
        };
        return b
            .edges()
            .into_iter()
            .filter(|p| !p.covered)
            .map(|p| Seg::Line(p.a, p.b))
            .collect();
    }
    // Konstruierte Kontur: alle Körperkanten **außer** den Anschlussnähten.
    // Bewusst ohne Beschnitt an Nachbarwänden — gefangen wird die gebaute
    // Ecke (Gehrungspunkt, Laibung, freies Ende), nicht der sichtbare Rest.
    let Some(b) = idx.body(e.id) else {
        return vec![];
    };
    let mut out = Vec::new();
    for poly in &b.pieces {
        for k in 0..poly.len() {
            let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
            if p.dist(q) < 1e-6 {
                continue;
            }
            if b.seams.iter().any(|(s0, s1)| on_line(p, q, *s0, *s1)) {
                continue;
            }
            // Stirnkante eines angeschlossenen Endes (Stichanschluss): keine
            // Umgrenzung — die Wand endet im Nachbarkörper.
            let in_plane = |c: Vec2| ((p - c).dot(b.dir)).abs() < 1e-6 && ((q - c).dot(b.dir)).abs() < 1e-6;
            let e_start = b.start;
            let e_end = b.start + b.dir * b.total;
            if (b.end_conn[0] && in_plane(e_start)) || (b.end_conn[1] && in_plane(e_end)) {
                continue;
            }
            out.push(Seg::Line(p, q));
        }
    }
    out
}

/// Liegt die Strecke p–q auf der Geraden durch a–b?
fn on_line(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> bool {
    let d = b - a;
    if d.len() < 1e-9 {
        return false;
    }
    let n = d.normalized().perp();
    (p - a).dot(n).abs() < 1e-6 && (q - a).dot(n).abs() < 1e-6
}

/// Umgrenzende Eckpunkte einer Wand (Endpunkte der Konturlinien, entdoppelt).
pub fn wall_contour_points(doc: &Document, e: &Entity) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    for sg in wall_contour_segments(doc, e) {
        for p in [sg.start(), sg.end()] {
            if !out.iter().any(|q| q.dist(p) <= CONTOUR_DEDUP) {
                out.push(p);
            }
        }
    }
    out
}

/// Toleranz, unterhalb derer zwei Konturpunkte als derselbe Punkt gelten (mm).
const CONTOUR_DEDUP: f64 = 0.1;
