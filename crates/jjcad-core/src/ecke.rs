//! Abrunden und Phase an einer Ecke aus zwei Linien (T0247).
//!
//! Beide Bearbeitungen brauchen dasselbe: zwei Linien, die sich schneiden, und
//! ein Maß. Die Geometrie rechnet **einmal** hier — der Klick in der
//! Zeichenfläche, die Vorschau und die Action `modify.fillet`/`modify.chamfer`
//! rufen dieselbe Funktion.
//!
//! Der Schnittpunkt der beiden unendlichen Geraden ist der Eckpunkt. Je Linie
//! bleibt die Seite stehen, auf die der Nutzer geklickt hat (`pick`); ohne
//! Klickpunkt entscheidet die längere Seite. Die gekürzte Strecke endet im
//! Anschlusspunkt, der Bogen (Abrunden) bzw. die Verbindungsstrecke (Phase)
//! schließt die Lücke.

use crate::geom::{line_line, norm_angle, Arc, Vec2};
use crate::model::{Document, EntityKind, Id};
use std::f64::consts::PI;

/// Zwei Punkte näher als das gelten als derselbe.
pub const ECKE_EPS: f64 = 1e-6;

/// Kleinster Öffnungswinkel einer Ecke. Darunter liegen die Linien
/// ineinander (kein Bogen, keine Phase) — ein Fehler, kein stiller Erfolg.
pub const ECKE_MIN_WINKEL: f64 = 1e-6;

/// Art der Eckbearbeitung.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EckeArt {
    /// Bogen mit Radius zwischen den beiden Schenkeln.
    Abrunden,
    /// Gerade zwischen den beiden Schenkeln, Maß auf beiden Seiten gleich.
    Phase,
}

impl EckeArt {
    pub fn key(self) -> &'static str {
        match self {
            EckeArt::Abrunden => "abrunden",
            EckeArt::Phase => "phase",
        }
    }

    /// Maß, das die Action unter `mass` erwartet.
    pub fn mass_name(self) -> &'static str {
        match self {
            EckeArt::Abrunden => "radius",
            EckeArt::Phase => "distance",
        }
    }
}

/// Ergebnis einer Eckbearbeitung — die beiden gekürzten Strecken und das
/// Verbindungsstück.
///
/// Die Strecken behalten die Richtung ihrer Linie (`a` → `b` wie vorher); der
/// Bogen läuft von `t1` nach `t2`, die Phase verbindet dieselben Punkte.
#[derive(Clone, Debug)]
pub struct Ecke {
    /// Schnittpunkt der beiden Geraden (die Ecke selbst).
    pub punkt: Vec2,
    /// Gekürzte Strecke der ersten Linie.
    pub a: (Vec2, Vec2),
    /// Gekürzte Strecke der zweiten Linie.
    pub b: (Vec2, Vec2),
    /// Anschlusspunkt auf der ersten Linie.
    pub t1: Vec2,
    /// Anschlusspunkt auf der zweiten Linie.
    pub t2: Vec2,
    /// Bogen bei [`EckeArt::Abrunden`] (`None` bei der Phase).
    pub bogen: Option<Arc>,
    /// Maß: Radius (Abrunden) bzw. Schenkellänge (Phase) in mm.
    pub mass: f64,
    /// Abstand des Anschlusspunkts vom Eckpunkt, auf der Linie gemessen.
    pub schenkel: f64,
    /// Öffnungswinkel der Ecke (Bogenmaß, 0…π).
    pub winkel: f64,
}

impl Ecke {
    /// Strecken der Vorschau: beide Schenkel und das Verbindungsstück.
    ///
    /// Der Bogen wird mit fester Pfeilhöhe zerlegt; die Zeichenfläche zeichnet
    /// dieselbe Punktfolge.
    pub fn strecken(&self, tol: f64) -> Vec<(Vec2, Vec2)> {
        let mut out = vec![self.a, self.b];
        if let Some(arc) = self.bogen {
            let pts = arc.flatten(tol.max(1e-6));
            for pair in pts.windows(2) {
                out.push((pair[0], pair[1]));
            }
        } else {
            out.push((self.t1, self.t2));
        }
        out
    }
}

/// Endpunkte einer geraden Linie — jede andere Objektart ist ein Fehler.
fn linie(doc: &Document, id: Id) -> Result<(Vec2, Vec2), String> {
    let e = doc
        .entity(id)
        .ok_or_else(|| format!("Objekt {id} nicht gefunden"))?;
    match &e.kind {
        EntityKind::Line { a, b } => Ok((*a, *b)),
        _ => Err("Abrunden und Phase gehen nur mit Linien".into()),
    }
}

/// Schnittpunkt der beiden Geraden — die Ecke, ohne die Zeichnung zu ändern.
///
/// Das Werkzeug prüft damit schon beim zweiten Klick, ob das Paar überhaupt
/// eine Ecke bildet; ein Fehler hier ist der Grund, warum kein Radius kommt.
pub fn schnittpunkt(doc: &Document, id_a: Id, id_b: Id) -> Result<Vec2, String> {
    if id_a == id_b {
        return Err("Es braucht zwei verschiedene Linien".into());
    }
    let (a1, a2) = linie(doc, id_a)?;
    let (b1, b2) = linie(doc, id_b)?;
    line_line(a1, a2, b1, b2, true, true)
        .ok_or_else(|| "Die Linien sind parallel — keine Ecke".to_string())
}

/// Die Seite einer Linie, die stehen bleibt, und ihre Richtung vom Eckpunkt aus.
///
/// `pick` ist der Klickpunkt des Nutzers: die Seite, auf der er liegt, bleibt.
/// Ohne (oder genau auf dem Eckpunkt) entscheidet die Seite mit dem längeren
/// Rest — eine Linie, deren Ende am Eckpunkt liegt, hat nur eine.
fn schenkel_richtung(
    p1: Vec2,
    p2: Vec2,
    ecke: Vec2,
    pick: Option<Vec2>,
) -> Result<(Vec2, Vec2), String> {
    let u = (p2 - p1).normalized();
    let seite = match pick {
        Some(q) if (q - ecke).dot(u).abs() > ECKE_EPS => (q - ecke).dot(u),
        _ => {
            // Ohne brauchbaren Klick: die längere Seite. Steht der Eckpunkt
            // genau in der Mitte, ist die Seite nicht bestimmbar.
            let d1 = p1.dist(ecke);
            let d2 = p2.dist(ecke);
            if (d1 - d2).abs() <= ECKE_EPS {
                return Err("Die Seite der Linie ist nicht bestimmbar".into());
            }
            // Positiv = Seite von `p2`, wie beim Klickpunkt.
            d2 - d1
        }
    };
    let (behalten, richtung) = if seite > 0.0 {
        (p2, p2 - ecke)
    } else {
        (p1, p1 - ecke)
    };
    if behalten.dist(ecke) <= ECKE_EPS {
        return Err("Die Linie hat auf dieser Seite keine Länge".into());
    }
    Ok((behalten, richtung.normalized()))
}

/// Rechnet die Eckbearbeitung — **ohne** die Zeichnung zu ändern.
///
/// `mass` ist bei [`EckeArt::Abrunden`] der Radius, bei [`EckeArt::Phase`] der
/// Abstand der Anschlusspunkte vom Eckpunkt (mm).
pub fn ecke(
    doc: &Document,
    id_a: Id,
    id_b: Id,
    pick_a: Option<Vec2>,
    pick_b: Option<Vec2>,
    mass: f64,
    art: EckeArt,
) -> Result<Ecke, String> {
    if id_a == id_b {
        return Err("Es braucht zwei verschiedene Linien".into());
    }
    let (a1, a2) = linie(doc, id_a)?;
    let (b1, b2) = linie(doc, id_b)?;
    let punkt = line_line(a1, a2, b1, b2, true, true)
        .ok_or_else(|| "Die Linien sind parallel — keine Ecke".to_string())?;
    ecke_aus_strahlen(punkt, (a1, a2), pick_a, (b1, b2), pick_b, mass, art)
}

/// Die reine Rechnung auf zwei Schenkeln: `punkt` ist die Ecke, `a` und `b`
/// sind die beiden Geraden (wie gezeichnet, mit ihren Enden), `mass` das Maß.
///
/// Getrennt von [`ecke`], weil auch der **Knick einer Polylinie** zwei
/// Schenkel hat: dort stehen die beiden Nachbarstrecken des Scheitelpunkts.
pub fn ecke_aus_strahlen(
    punkt: Vec2,
    a: (Vec2, Vec2),
    pick_a: Option<Vec2>,
    b: (Vec2, Vec2),
    pick_b: Option<Vec2>,
    mass: f64,
    art: EckeArt,
) -> Result<Ecke, String> {
    if !(mass > 0.0) || !mass.is_finite() {
        return Err("Das Maß muss größer als null sein".into());
    }
    let (a1, a2) = a;
    let (b1, b2) = b;
    let (behalten_a, dir_a) = schenkel_richtung(a1, a2, punkt, pick_a)?;
    let (behalten_b, dir_b) = schenkel_richtung(b1, b2, punkt, pick_b)?;

    // Öffnungswinkel zwischen den beiden stehenden Schenkeln.
    let cos = dir_a.dot(dir_b).clamp(-1.0, 1.0);
    let winkel = cos.acos();
    if winkel < ECKE_MIN_WINKEL || (PI - winkel) < ECKE_MIN_WINKEL {
        return Err("Die Linien liegen ineinander — keine Ecke".into());
    }

    // Maß auf der Linie: beim Bogen r/tan(θ/2), bei der Phase das Maß selbst.
    let schenkel = match art {
        EckeArt::Abrunden => mass / (winkel / 2.0).tan(),
        EckeArt::Phase => mass,
    };
    let laenge_a = behalten_a.dist(punkt);
    let laenge_b = behalten_b.dist(punkt);
    if schenkel >= laenge_a - ECKE_EPS || schenkel >= laenge_b - ECKE_EPS {
        return Err("Das Maß ist für die kürzere Linie zu groß".into());
    }
    let t1 = punkt + dir_a * schenkel;
    let t2 = punkt + dir_b * schenkel;

    let bogen = match art {
        EckeArt::Abrunden => {
            // Mittelpunkt auf der Innenwinkel-Halbierenden, r/sin(θ/2) entfernt.
            let halb = (dir_a + dir_b).normalized();
            if halb.len() < 0.5 {
                return Err("Die Linien liegen ineinander — keine Ecke".into());
            }
            let c = punkt + halb * (mass / (winkel / 2.0).sin());
            let a1w = (t1 - c).angle();
            let a2w = (t2 - c).angle();
            // Der kurze Bogen ist der richtige: er misst π − θ.
            let arc = if norm_angle(a2w - a1w) <= PI {
                Arc::new(c, mass, a1w, a2w)
            } else {
                Arc::new(c, mass, a2w, a1w)
            };
            Some(arc)
        }
        EckeArt::Phase => None,
    };

    // Gekürzte Strecken behalten ihre Richtung: bleibt das Ende `b` stehen,
    // läuft die Strecke vom Anschlusspunkt dorthin; bleibt `a`, endet sie dort.
    let neu_a = if behalten_a.dist(a2) <= ECKE_EPS {
        (t1, a2)
    } else {
        (a1, t1)
    };
    let neu_b = if behalten_b.dist(b2) <= ECKE_EPS {
        (t2, b2)
    } else {
        (b1, t2)
    };

    Ok(Ecke {
        punkt,
        a: neu_a,
        b: neu_b,
        t1,
        t2,
        bogen,
        mass,
        schenkel,
        winkel,
    })
}

/// Bereitet eine Eckbearbeitung vor und schreibt sie in die Zeichnung:
/// beide Linien werden gekürzt, das Verbindungsstück kommt als neues Objekt
/// dazu (Bogen bei Abrunden, Linie bei Phase).
///
/// Ebene und Stil erbt das Verbindungsstück von der ersten Linie.
pub fn anwenden(
    doc: &mut Document,
    id_a: Id,
    id_b: Id,
    pick_a: Option<Vec2>,
    pick_b: Option<Vec2>,
    mass: f64,
    art: EckeArt,
) -> Result<(Ecke, Id), String> {
    let e = ecke(doc, id_a, id_b, pick_a, pick_b, mass, art)?;
    let vorlage = doc
        .entity(id_a)
        .cloned()
        .ok_or_else(|| format!("Objekt {id_a} nicht gefunden"))?;
    if let Some(em) = doc.entity_mut(id_a) {
        em.kind = EntityKind::Line { a: e.a.0, b: e.a.1 };
    }
    if let Some(em) = doc.entity_mut(id_b) {
        em.kind = EntityKind::Line { a: e.b.0, b: e.b.1 };
    }
    let neu = doc.new_id();
    let kind = match e.bogen {
        Some(arc) => EntityKind::Arc {
            c: arc.c,
            r: arc.r,
            a0: arc.a0,
            a1: arc.a1,
        },
        None => EntityKind::Line { a: e.t1, b: e.t2 },
    };
    let mut ne = crate::model::Entity::new(neu, vorlage.layer, kind);
    ne.style = vorlage.style.clone();
    ne.by_block = vorlage.by_block;
    doc.insert_ranked(ne);
    doc.modified = true;
    Ok((e, neu))
}

// ---------------------------------------------------------------------------
// Knick einer Polylinie (T0247, Turn 3)
// ---------------------------------------------------------------------------

/// Die drei Punkte eines Polylinien-Knicks: Vorgänger, Scheitelpunkt, Nachfolger.
///
/// Bei einer **geschlossenen** Polylinie gibt es keinen Rand: der erste Punkt
/// hat den letzten zum Vorgänger und umgekehrt.
fn knick_punkte(verts: &[crate::geom::Vertex], closed: bool, knoten: usize) -> Result<(Vec2, Vec2, Vec2), String> {
    let n = verts.len();
    if n < 3 {
        return Err("Die Polylinie hat keinen Knick".into());
    }
    if knoten >= n {
        return Err("Diesen Knick gibt es nicht".into());
    }
    if !closed && (knoten == 0 || knoten + 1 == n) {
        return Err("Das Ende einer Polylinie ist kein Knick — zwei Linien wählen".into());
    }
    let vor = (knoten + n - 1) % n;
    let nach = (knoten + 1) % n;
    Ok((verts[vor].p, verts[knoten].p, verts[nach].p))
}

/// Ist der Knick aus zwei **geraden** Stücken? Ein Bogenstück hat am
/// Scheitelpunkt eine Tangente, nicht die Richtung seiner Sehne — die
/// Abrundung eines solchen Knicks wäre eine andere Aufgabe.
fn knick_ist_gerade(verts: &[crate::geom::Vertex], knoten: usize) -> Result<(), String> {
    let n = verts.len();
    let vor = (knoten + n - 1) % n;
    // Der Bulge gehört zum Stück, das **an** diesem Punkt beginnt: `vor` trägt
    // das ankommende, `knoten` das abgehende Stück.
    if verts[vor].bulge.abs() > 1e-9 || verts[knoten].bulge.abs() > 1e-9 {
        return Err("Der Knick liegt an einem Bogen — nur gerade Schenkel".into());
    }
    Ok(())
}

/// Nächster Knick einer Polylinie in Fangnähe — der Scheitelpunkt unter dem
/// Zeiger, den ein Klick mit einem Schritt wählt.
///
/// Gesucht wird über die **Scheitelpunkte**, nicht über die Strecken: ein Klick
/// mitten auf ein Stück meint keinen Knick.
pub fn polylinien_knoten(doc: &Document, bei: Vec2, tol: f64) -> Option<(Id, usize, Vec2)> {
    let mut best: Option<(f64, Id, usize, Vec2)> = None;
    for e in doc.ents() {
        let EntityKind::Polyline { verts, closed } = &e.kind else {
            continue;
        };
        if !doc.layer_sichtbar(e.layer) || doc.layer_gesperrt(e.layer) {
            continue;
        }
        let n = verts.len();
        for (i, v) in verts.iter().enumerate() {
            // Offene Enden und Knicke an Bogenstücken sind keine brauchbaren
            // Ecken — sie werden gar nicht erst angeboten.
            if !*closed && (i == 0 || i + 1 == n) {
                continue;
            }
            let vor = (i + n - 1) % n;
            if verts[vor].bulge.abs() > 1e-9 || v.bulge.abs() > 1e-9 {
                continue;
            }
            let d = v.p.dist(bei);
            if d <= tol && best.map(|(bd, ..)| d < bd).unwrap_or(true) {
                best = Some((d, e.id, i, v.p));
            }
        }
    }
    best.map(|(_, id, i, p)| (id, i, p))
}

/// Alle Linien, die durch den Punkt laufen (Abstand höchstens `tol`), nach
/// Abstand sortiert.
///
/// Das ist die Auswahl „direkt über den Schnittpunkt": liegen dort zwei Linien,
/// bilden sie die Ecke ohne zweiten Klick.
pub fn linien_durch(doc: &Document, bei: Vec2, tol: f64) -> Vec<Id> {
    let mut treffer: Vec<(f64, Id)> = Vec::new();
    for e in doc.ents() {
        let EntityKind::Line { a, b } = &e.kind else {
            continue;
        };
        if !doc.layer_sichtbar(e.layer) || doc.layer_gesperrt(e.layer) {
            continue;
        }
        let d = crate::geom::dist_point_segment(bei, *a, *b);
        if d <= tol {
            treffer.push((d, e.id));
        }
    }
    treffer.sort_by(|a, b| a.0.total_cmp(&b.0));
    treffer.into_iter().map(|(_, id)| id).collect()
}

/// Rechnet den Knick einer Polylinie — **ohne** die Zeichnung zu ändern.
///
/// Die beiden Nachbarstücke sind die Schenkel, der Scheitelpunkt ist die Ecke.
/// Welche Seite stehen bleibt, ist damit schon festgelegt (jedes Stück endet
/// bzw. beginnt dort), ein Klickpunkt wird nicht gebraucht.
pub fn ecke_polylinie(
    doc: &Document,
    id: Id,
    knoten: usize,
    mass: f64,
    art: EckeArt,
) -> Result<Ecke, String> {
    let e = doc
        .entity(id)
        .ok_or_else(|| format!("Objekt {id} nicht gefunden"))?;
    let EntityKind::Polyline { verts, closed } = &e.kind else {
        return Err("Abrunden und Phase gehen an einer Polylinie nur am Knick".into());
    };
    knick_ist_gerade(verts, knoten)?;
    let (vor, hier, nach) = knick_punkte(verts, *closed, knoten)?;
    ecke_aus_strahlen(hier, (vor, hier), None, (hier, nach), None, mass, art)
}

/// Schreibt den Knick in die Polylinie: der Scheitelpunkt wird durch die beiden
/// Anschlusspunkte ersetzt, dazwischen steht der Bogen (Abrunden) bzw. die
/// Gerade (Phase) — alles in **einem** Objekt.
pub fn anwenden_polylinie(
    doc: &mut Document,
    id: Id,
    knoten: usize,
    mass: f64,
    art: EckeArt,
) -> Result<Ecke, String> {
    let e = ecke_polylinie(doc, id, knoten, mass, art)?;
    // Bogenmaß des Verbindungsstücks: Richtung des Durchlaufs (vorwärts =
    // positiv), damit das Stück in der Polylinie in Laufrichtung liegt.
    // Der Bulge gehört zum Stück, das am **Anfangspunkt** des Bogens beginnt
    // und in Laufrichtung läuft. Der Bogen aus `ecke` ist gegen den Uhrzeigersinn
    // abgelegt; beginnt er am zweiten Anschluss, läuft das Stück andersherum.
    let bulge = match e.bogen {
        Some(arc) => {
            let b = crate::geom::bulge_from_arc(&arc);
            if arc.start().dist(e.t1) <= arc.start().dist(e.t2) {
                b
            } else {
                -b
            }
        }
        None => 0.0,
    };
    let Some(em) = doc.entity_mut(id) else {
        return Err(format!("Objekt {id} nicht gefunden"));
    };
    let EntityKind::Polyline { verts, .. } = &mut em.kind else {
        return Err("Keine Polylinie".into());
    };
    let neu = [
        crate::geom::Vertex::arc(e.t1, bulge),
        crate::geom::Vertex::new(e.t2),
    ];
    verts.splice(knoten..=knoten, neu);
    doc.modified = true;
    Ok(e)
}
