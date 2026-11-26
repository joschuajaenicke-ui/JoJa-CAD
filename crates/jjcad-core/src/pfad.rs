//! Pfad (T0229): eine Kette aus Linien, Bögen, Ellipsenbögen und Splines.
//!
//! Anders als die Polylinie (nur Stützpunkte mit Bogenschlag) trägt ein Pfad je
//! Stück seine eigene Geometrie. Die Stücke liegen **lückenlos** aneinander:
//! das Ende eines Stücks ist der Anfang des nächsten. Schließen sich Anfang und
//! Ende, ist der Pfad geschlossen und umschließt eine Fläche.
//!
//! Das Werkzeug [`crate::actions::ACTIONS`] `draw.path` erzeugt nur Linien und
//! Bögen; Ellipsenbögen und Splines entstehen über `path.connect` aus
//! ausgewählten Objekten. Für eine Fläche müssen alle Stücke zerlegbar sein:
//! enthält der Pfad einen Spline, gibt es **nie** eine geschlossene Kontur.

use crate::geom::*;
use crate::model::EntityKind;
use serde::{Deserialize, Serialize};

/// Epsilon, mit dem Anfang und Ende eines Pfades als „aufeinander" gelten.
pub const PFAD_EPS_MM: f64 = 0.5;

/// Ein Stück eines Pfades.
///
/// Gespeichert ist jeweils die volle Geometrie des Stücks (beim Bogen Mittelpunkt
/// und Winkel, nicht nur die Endpunkte): so bleibt es nach jeder Bewegung
/// unverändert gültig und die Anzeige braucht keine Rechnung über Nachbarn.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "art", rename_all = "snake_case")]
pub enum PathSeg {
    Line { a: Vec2, b: Vec2 },
    /// Bogen: `a0`→`a1` ist der **geometrische** Ausschnitt (Gegenuhrzeigersinn).
    /// `rev` sagt, wie das Stück durchlaufen wird: `false` von `a0` nach `a1`,
    /// `true` umgekehrt. Ohne diese Angabe wäre eine umgedrehte Laufrichtung
    /// nur als **Gegenbogen** darstellbar — genau der Fehler aus T0248
    /// (Anfang und Ende eines Bogens vertauscht, der Bogen sprang auf die
    /// andere Seite des Kreises).
    Arc {
        c: Vec2,
        r: f64,
        a0: f64,
        a1: f64,
        #[serde(default, skip_serializing_if = "ist_falsch")]
        rev: bool,
    },
    /// Ellipsenbogen, Laufrichtung wie beim Bogen über `rev`.
    EllipseArc {
        c: Vec2,
        rx: f64,
        ry: f64,
        rot: f64,
        a0: f64,
        a1: f64,
        #[serde(default, skip_serializing_if = "ist_falsch")]
        rev: bool,
    },
    Spline { verts: Vec<Vertex> },
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn ist_falsch(b: &bool) -> bool {
    !*b
}

impl PathSeg {
    pub fn start(&self) -> Vec2 {
        match self {
            PathSeg::Line { a, .. } => *a,
            PathSeg::Arc { c, r, a0, a1, rev, .. } => {
                let a = if *rev { *a1 } else { *a0 };
                *c + Vec2::polar(a, *r)
            }
            PathSeg::EllipseArc {
                c, rx, ry, rot, a0, a1, rev, ..
            } => {
                let u = if *rev { *a1 } else { *a0 };
                *c + Vec2::new(rx * u.cos(), ry * u.sin()).rotated(*rot)
            }
            PathSeg::Spline { verts } => verts.first().map(|v| v.p).unwrap_or(Vec2::ZERO),
        }
    }

    pub fn end(&self) -> Vec2 {
        match self {
            PathSeg::Line { b, .. } => *b,
            PathSeg::Arc { c, r, a0, a1, rev, .. } => {
                let a = if *rev { *a0 } else { *a1 };
                *c + Vec2::polar(a, *r)
            }
            PathSeg::EllipseArc {
                c, rx, ry, rot, a0, a1, rev, ..
            } => {
                let u = if *rev { *a0 } else { *a1 };
                *c + Vec2::new(rx * u.cos(), ry * u.sin()).rotated(*rot)
            }
            PathSeg::Spline { verts } => verts.last().map(|v| v.p).unwrap_or(Vec2::ZERO),
        }
    }

    /// Länge des Stücks in Millimetern.
    pub fn len(&self) -> f64 {
        match self {
            PathSeg::Line { a, b } => a.dist(*b),
            PathSeg::Arc { r, a0, a1, .. } => {
                Arc::new(Vec2::ZERO, *r, *a0, *a1).len()
            }
            PathSeg::EllipseArc {
                rx, ry, a0, a1, ..
            } => {
                let el = crate::geom::Ellipse::new(
                    Vec2::ZERO,
                    *rx,
                    *ry,
                    0.0,
                );
                let pts = el.arc_points(*a0, *a1, 0.05);
                pts.windows(2).map(|w| w[0].dist(w[1])).sum()
            }
            PathSeg::Spline { verts } => {
                let sp = Spline {
                    verts: verts.clone(),
                    closed: false,
                };
                sp.len()
            }
        }
    }

    /// Strecken des Stücks (für Fang und Trefferprüfung) — dieselbe Zerlegung
    /// wie die Anzeige.
    pub fn segments(&self) -> Vec<Seg> {
        match self {
            PathSeg::Line { a, b } => vec![Seg::Line(*a, *b)],
            // Die Laufrichtung ist für Trefferprüfung und Fang unerheblich —
            // geprüft wird die Punktmenge, und die ist beidseitig dieselbe.
            PathSeg::Arc { c, r, a0, a1, .. } => vec![Seg::Arc(Arc::new(*c, *r, *a0, *a1))],
            PathSeg::EllipseArc {
                c,
                rx,
                ry,
                rot,
                a0,
                a1,
                ..
            } => {
                let el = crate::geom::Ellipse::new(*c, *rx, *ry, *rot);
                let pts = el.arc_points(*a0, *a1, 0.2);
                pts.windows(2).map(|w| Seg::Line(w[0], w[1])).collect()
            }
            PathSeg::Spline { verts } => Spline {
                verts: verts.clone(),
                closed: false,
            }
            .segments(),
        }
    }

    /// Punktfolge des Stücks von Anfang bis Ende (`tol` = Pfeilhöhe in mm).
    pub fn flatten(&self, tol: f64) -> Vec<Vec2> {
        match self {
            PathSeg::Line { a, b } => vec![*a, *b],
            PathSeg::Arc { c, r, a0, a1, rev } => {
                let mut pts = Arc::new(*c, *r, *a0, *a1).flatten(tol);
                if *rev {
                    pts.reverse();
                }
                pts
            }
            PathSeg::EllipseArc {
                c,
                rx,
                ry,
                rot,
                a0,
                a1,
                rev,
            } => {
                let mut pts =
                    crate::geom::Ellipse::new(*c, *rx, *ry, *rot).arc_points(*a0, *a1, tol);
                if *rev {
                    pts.reverse();
                }
                pts
            }
            PathSeg::Spline { verts } => Spline {
                verts: verts.clone(),
                closed: false,
            }
            .flatten(tol),
        }
    }

    /// Dasselbe Stück in umgekehrter Laufrichtung.
    pub fn reversed(&self) -> PathSeg {
        match self.clone() {
            PathSeg::Line { a, b } => PathSeg::Line { a: b, b: a },
            // Nur die Laufrichtung kippt, der Bogen bleibt derselbe: `a0`/`a1`
            // beschreiben den Ausschnitt, nicht den Weg (T0248).
            PathSeg::Arc { c, r, a0, a1, rev } => PathSeg::Arc {
                c,
                r,
                a0,
                a1,
                rev: !rev,
            },
            PathSeg::EllipseArc {
                c,
                rx,
                ry,
                rot,
                a0,
                a1,
                rev,
            } => PathSeg::EllipseArc {
                c,
                rx,
                ry,
                rot,
                a0,
                a1,
                rev: !rev,
            },
            PathSeg::Spline { verts } => PathSeg::Spline {
                verts: verts.into_iter().rev().collect(),
            },
        }
    }

    /// Enthält das Stück einen Spline? (Ein solcher Pfad wird nie geschlossen.)
    pub fn hat_spline(&self) -> bool {
        matches!(self, PathSeg::Spline { .. })
    }
}

/// Rolle eines Klicks beim Erzeugen eines Pfades.
///
/// Ein Bogen braucht drei Punkte (Anfang, Punkt auf dem Bogen, Ende): der Klick
/// auf den Anfang ist der des vorigen Stücks, danach folgen `BogenPunkt` und
/// `BogenEnde`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PfadSchritt {
    Start,
    Line,
    ArcPoint,
    ArcEnd,
}

impl PfadSchritt {
    pub fn key(&self) -> &'static str {
        match self {
            PfadSchritt::Start => "start",
            PfadSchritt::Line => "line",
            PfadSchritt::ArcPoint => "arc_point",
            PfadSchritt::ArcEnd => "arc_end",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        Some(match s {
            "start" => PfadSchritt::Start,
            "line" | "linie" => PfadSchritt::Line,
            "arc_point" | "bogenpunkt" => PfadSchritt::ArcPoint,
            "arc_end" | "bogenende" => PfadSchritt::ArcEnd,
            _ => return None,
        })
    }
}

/// Bogenstück aus drei Punkten (Anfang `p0`, Punkt auf dem Bogen `pm`,
/// Ende `p1`).
///
/// `Arc::from_3_points` dreht Anfang und Ende, wenn der Bogen nur andersherum
/// durch die drei Punkte läuft. Für ein Pfadstück muss aber `p0` der Anfang
/// sein; die Laufrichtung steht deshalb in `rev`, nicht in vertauschten
/// Winkeln — vertauschte Winkel ergäben den **Gegenbogen** (T0248).
pub fn arc_aus_3_punkten(p0: Vec2, pm: Vec2, p1: Vec2) -> Option<PathSeg> {
    let arc = Arc::from_3_points(p0, pm, p1)?;
    let rev = arc.start().dist(p0) > PFAD_EPS_MM;
    Some(PathSeg::Arc {
        c: arc.c,
        r: arc.r,
        a0: arc.a0,
        a1: arc.a1,
        rev,
    })
}

/// Pfadstücke aus den Klicks des Werkzeugs: `pts` sind die Klickpunkte,
/// `schritte[i]` sagt, was der i-te Klick beginnt.
///
/// Liegen drei Bogenpunkte auf einer Geraden, entsteht statt des Bogens eine
/// Linie (kein stiller Fehlschlag: der Nutzer sieht das Ergebnis).
pub fn pfad_aus_punkten(pts: &[Vec2], schritte: &[PfadSchritt]) -> Vec<PathSeg> {
    let mut out: Vec<PathSeg> = Vec::new();
    let Some(mut last) = pts.first().copied() else {
        return out;
    };
    let mut i = 1;
    while i < pts.len() {
        match schritte.get(i) {
            Some(PfadSchritt::Line) => {
                if last.dist(pts[i]) > 1e-9 {
                    out.push(PathSeg::Line {
                        a: last,
                        b: pts[i],
                    });
                }
                last = pts[i];
                i += 1;
            }
            Some(PfadSchritt::ArcPoint) => {
                let ist_ende = schritte.get(i + 1) == Some(&PfadSchritt::ArcEnd);
                if ist_ende && i + 1 < pts.len() {
                    if let Some(arc) = arc_aus_3_punkten(last, pts[i], pts[i + 1]) {
                        out.push(arc);
                    } else if last.dist(pts[i + 1]) > 1e-9 {
                        out.push(PathSeg::Line {
                            a: last,
                            b: pts[i + 1],
                        });
                    }
                    last = pts[i + 1];
                    i += 2;
                } else {
                    // Bogenpunkt ohne Ende: warten auf den nächsten Klick.
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Punktfolge aller Stücke lückenlos aneinander (Übergänge nur einmal).
pub fn pfad_punkte(segs: &[PathSeg], tol: f64) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    for s in segs {
        let pts = s.flatten(tol);
        for p in pts {
            if out.last().map(|q| q.dist(p) <= 1e-9).unwrap_or(false) {
                continue;
            }
            out.push(p);
        }
    }
    out
}

/// Länge eines Pfades in mm.
pub fn pfad_len(segs: &[PathSeg]) -> f64 {
    segs.iter().map(|s| s.len()).sum()
}

/// Schließen sich Anfang und Ende?
pub fn pfad_geschlossen(segs: &[PathSeg]) -> bool {
    match (segs.first(), segs.last()) {
        (Some(a), Some(b)) => a.start().dist(b.end()) <= PFAD_EPS_MM,
        _ => false,
    }
}

/// Fläche, die ein geschlossener Pfad umschließt (mm²).
///
/// `None`, wenn ein Stück ein Spline ist: dessen Flächenanteil zu rechnen wäre
/// unverhältnismäßig — ein solcher Pfad ergibt **nie** eine geschlossene
/// Kontur (T0229).
pub fn pfad_flaeche(segs: &[PathSeg]) -> Option<f64> {
    if segs.iter().any(|s| s.hat_spline()) {
        return None;
    }
    let pts = pfad_punkte(segs, 0.2);
    if pts.len() < 3 {
        return None;
    }
    Some(crate::geom::polygon_area(&pts).abs())
}

/// Stücke, die aus einem Objekt heraus einen Pfad bilden können.
fn segs_aus_kind(kind: &EntityKind) -> Option<Vec<PathSeg>> {
    Some(match kind {
        EntityKind::Line { a, b } => vec![PathSeg::Line { a: *a, b: *b }],
        EntityKind::Arc { c, r, a0, a1 } => vec![PathSeg::Arc {
            c: *c,
            r: *r,
            a0: *a0,
            a1: *a1,
            rev: false,
        }],
        EntityKind::EllipseArc {
            c,
            rx,
            ry,
            rot,
            a0,
            a1,
        } => vec![PathSeg::EllipseArc {
            c: *c,
            rx: *rx,
            ry: *ry,
            rot: *rot,
            a0: *a0,
            a1: *a1,
            rev: false,
        }],
        EntityKind::Spline { verts, closed } => {
            if *closed {
                // Ein geschlossener Spline ist in sich geschlossen; als Stück
                // einer Kette wäre sein Anfang ohnehin schon das Ende.
                vec![PathSeg::Spline {
                    verts: verts.clone(),
                }]
            } else {
                vec![PathSeg::Spline {
                    verts: verts.clone(),
                }]
            }
        }
        _ => return None,
    })
}

/// Mehrere Linienobjekte zu **einem** Pfad verbinden.
///
/// Die Objekte müssen eine lückenlose Kette bilden: an jedem Ende darf genau
/// ein weiteres Stück anschließen. Die Reihenfolge der Auswahl ist beliebig,
/// Stücke werden bei Bedarf umgedreht. Ein Niete (kein Anschluss) oder eine
/// Verzweigung (drei Stücke in einem Punkt) ist ein Fehler — der Aufrufer
/// bekommt die Begründung, kein halbfertiges Ergebnis.
pub fn pfad_verbinden(teile: &[EntityKind]) -> Result<Vec<PathSeg>, String> {
    if teile.len() < 2 {
        return Err("mindestens zwei Objekte wählen".to_string());
    }
    let mut rest: Vec<Vec<PathSeg>> = Vec::new();
    for (i, k) in teile.iter().enumerate() {
        match segs_aus_kind(k) {
            Some(s) => rest.push(s),
            None => {
                return Err(format!(
                    "Objekt {} ist keine Linie, kein Bogen und kein Spline",
                    i + 1
                ));
            }
        }
    }
    let mut kette: Vec<PathSeg> = rest.remove(0);
    while !rest.is_empty() {
        let ende = kette.last().map(|s| s.end()).unwrap_or(Vec2::ZERO);
        // Anschluss an das Ende der Kette suchen (vorwärts oder rückwärts).
        let treffer = rest.iter().position(|stueck| {
            let a = stueck.first().map(|s| s.start()).unwrap_or(Vec2::ZERO);
            let b = stueck.last().map(|s| s.end()).unwrap_or(Vec2::ZERO);
            a.dist(ende) <= PFAD_EPS_MM || b.dist(ende) <= PFAD_EPS_MM
        });
        let Some(idx) = treffer else {
            return Err("die Objekte hängen nicht zusammen".to_string());
        };
        let mut stueck = rest.remove(idx);
        let anfang = stueck.first().map(|s| s.start()).unwrap_or(Vec2::ZERO);
        if anfang.dist(ende) > PFAD_EPS_MM {
            stueck = stueck.into_iter().rev().map(|s| s.reversed()).collect();
        }
        // Lücke in der Kette wäre kein „zusammenhängend".
        if stueck
            .first()
            .map(|s| s.start().dist(ende) > PFAD_EPS_MM)
            .unwrap_or(true)
        {
            return Err("die Objekte hängen nicht zusammen".to_string());
        }
        kette.extend(stueck);
    }
    Ok(kette)
}
