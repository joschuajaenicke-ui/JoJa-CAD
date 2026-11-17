//! Fangmarken — ein eigenes Zeichen je Fangart (T0153).
//!
//! Bisher zeigte die Zeichenfläche an jedem Fangpunkt dasselbe kleine Kreuz.
//! Welcher Fang gegriffen hatte, stand nur in der Statuszeile — beim Zeichnen
//! schaut man dorthin nicht. Jetzt hat **jede** Fangart ihr eigenes Zeichen,
//! und es sitzt am **Fangpunkt** (also am Element: Linienende, Kreismitte,
//! Schnittpunkt …), nicht am rohen Zeiger.
//!
//! Die Zeichen sind Bildschirmzeichen: sie liegen in einer festen Größe über
//! der Zeichnung und gehören damit zur Bedienoberfläche, nicht in die
//! Displayliste (die ist für Bildschirm, PDF und PNG dieselbe — ein Fangkreuz
//! gehört nicht in den Druck). Gezeichnet werden sie wie der Zeiger selbst in
//! `canvas`.
//!
//! Aufbau: [`marke`] liefert die Geometrie einer Fangart als reine Zahlen
//! (Markenkoordinaten, Ursprung = Fangpunkt). [`zeichne`] malt sie. Dadurch
//! prüfen die Tests Form und Lage ohne laufendes Fenster, und der Maler hat
//! keine zweite Geometrie.
//!
//! Ein Zeichen bekommt nur, was einen Punkt **am Element** hat. `Ortho`,
//! `Polar` und der `Raster`-Fang führen den Zeiger nur — ihr Punkt liegt an
//! jeder Mausstellung unter dem Zeiger, ein Zeichen dort klebte am Mauszeiger
//! statt am Objekt (T0153, Turn 3). Die Statuszeile nennt sie weiterhin.

use egui::{Color32, Painter, Pos2, Stroke};
use jjcad_core::snap::SnapKind;

use crate::theme::Palette;

/// Art des gerade aktiven Fangs — Grundlage für die Fangmarke.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FangArt {
    /// Objektfang der Fang-Engine mit einem Punkt am Element (Endpunkt, Mitte,
    /// Zentrum, Schnittpunkt …). Das Raster ist ausgenommen: es liegt überall.
    Objekt(SnapKind),
}

impl FangArt {
    /// Kurzname für die Tests (die Statuszeile nimmt `SnapKind::text`).
    #[cfg(test)]
    pub fn name(self) -> &'static str {
        match self {
            FangArt::Objekt(k) => k.label(),
        }
    }
}

/// Halbe Kantenlänge der Marke in Bildschirmpunkten.
///
/// Die Marke ist damit 18 px groß — groß genug, um die Form zu erkennen, und
/// klein genug, um das Element darunter nicht zu verdecken (das Fadenkreuz
/// zeigt weiterhin den genauen Punkt).
pub const HALB: f32 = 9.0;

/// Ein Bestandteil einer Fangmarke in Markenkoordinaten (0,0 = Fangpunkt,
/// y zeigt nach unten wie in Bildschirmpunkten).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zug {
    Linie((f32, f32), (f32, f32)),
    /// Kreis um den Fangpunkt.
    Kreis(f32),
    /// Gefüllter Punkt an einer Stelle der Marke.
    Punkt((f32, f32), f32),
    /// Gefülltes Vieleck.
    Flaeche(&'static [(f32, f32)]),
}

fn l(v: &mut Vec<Zug>, a: (f32, f32), b: (f32, f32)) {
    v.push(Zug::Linie(a, b));
}

/// Geometrie der Marke einer Fangart.
///
/// Die Zahlen stehen im 18-px-Raster (±9), damit alle Zeichen gleich groß
/// wirken. Jede Marke trägt einen gefüllten Punkt im Ursprung — er zeigt den
/// genauen Fangpunkt, auch wenn das Zeichen drumherum unsymmetrisch ist.
/// Einzige Ausnahme ist der Rasterfang: er bekommt **kein** Zeichen.
pub fn marke(art: FangArt) -> Vec<Zug> {
    use Zug::*;
    let mut v: Vec<Zug> = Vec::new();
    match art {
        // Quadrat um das Linienende — das klassische Zeichen für „Endpunkt“.
        FangArt::Objekt(SnapKind::Endpoint) => {
            l(&mut v, (-6.0, -6.0), (6.0, -6.0));
            l(&mut v, (6.0, -6.0), (6.0, 6.0));
            l(&mut v, (6.0, 6.0), (-6.0, 6.0));
            l(&mut v, (-6.0, 6.0), (-6.0, -6.0));
        }
        // Blattecke: gefülltes Quadrat im offenen Rahmen — die Ecke des
        // Blatts selbst (nicht das Ende einer Linie).
        FangArt::Objekt(SnapKind::SheetCorner) => {
            v.push(Flaeche(&[(-4.5, -4.5), (4.5, -4.5), (4.5, 4.5), (-4.5, 4.5)]));
            l(&mut v, (-7.5, -7.5), (7.5, -7.5));
            l(&mut v, (7.5, -7.5), (7.5, 7.5));
            l(&mut v, (7.5, 7.5), (-7.5, 7.5));
            l(&mut v, (-7.5, 7.5), (-7.5, -7.5));
        }
        // Basispunkt des Blocks: Kreis mit Kreuz — dasselbe Zeichen, das der
        // Blockeditor am Ursprung zeigt (T0208), nur im Fangraster.
        FangArt::Objekt(SnapKind::BlockBase) => {
            v.push(Kreis(8.0));
            l(&mut v, (-9.0, 0.0), (9.0, 0.0));
            l(&mut v, (0.0, -9.0), (0.0, 9.0));
        }
        // Schnittpunkt: Kreuz aus beiden Diagonalen.
        FangArt::Objekt(SnapKind::Intersection) => {
            l(&mut v, (-6.5, -6.5), (6.5, 6.5));
            l(&mut v, (-6.5, 6.5), (6.5, -6.5));
        }
        // Mittelpunkt: aufrecht stehendes Dreieck um den Punkt.
        FangArt::Objekt(SnapKind::Midpoint) => {
            l(&mut v, (0.0, -7.0), (-6.5, 4.5));
            l(&mut v, (-6.5, 4.5), (6.5, 4.5));
            l(&mut v, (6.5, 4.5), (0.0, -7.0));
        }
        // Zentrum: Kreis mit Kreuz — der Mittelpunkt ist im Plan nicht
        // gezeichnet, das Kreuz zeigt ihn (Halbbreite 7 px, T0117).
        FangArt::Objekt(SnapKind::Center) => {
            v.push(Kreis(6.0));
            l(&mut v, (-7.0, 0.0), (7.0, 0.0));
            l(&mut v, (0.0, -7.0), (0.0, 7.0));
        }
        // Quadrant: Raute auf den Achsen des Kreises.
        FangArt::Objekt(SnapKind::Quadrant) => {
            l(&mut v, (0.0, -7.0), (7.0, 0.0));
            l(&mut v, (7.0, 0.0), (0.0, 7.0));
            l(&mut v, (0.0, 7.0), (-7.0, 0.0));
            l(&mut v, (-7.0, 0.0), (0.0, -7.0));
        }
        // Lot: rechter Winkel mit Winkelquadrat, Scheitel im Fangpunkt.
        FangArt::Objekt(SnapKind::Perpendicular) => {
            l(&mut v, (-8.0, 0.0), (0.0, 0.0));
            l(&mut v, (0.0, 0.0), (0.0, -8.0));
            l(&mut v, (0.0, -3.5), (-3.5, -3.5));
            l(&mut v, (-3.5, -3.5), (-3.5, 0.0));
        }
        // Nächster Punkt: Sanduhr — die Lage auf der Strecke, nicht ihr Ende.
        FangArt::Objekt(SnapKind::Nearest) => {
            l(&mut v, (-6.0, -7.0), (6.0, -7.0));
            l(&mut v, (6.0, -7.0), (0.0, 0.0));
            l(&mut v, (0.0, 0.0), (-6.0, -7.0));
            l(&mut v, (-6.0, 7.0), (6.0, 7.0));
            l(&mut v, (6.0, 7.0), (0.0, 0.0));
            l(&mut v, (0.0, 0.0), (-6.0, 7.0));
        }
        // Raster: **kein** Zeichen (T0153, Turn 3). Der Rasterfang liegt an
        // jeder Zeigerstellung dicht unter dem Zeiger — ein Zeichen hier
        // klebte am Mauszeiger statt am Objekt. Die Statuszeile nennt ihn.
        FangArt::Objekt(SnapKind::Grid) => return Vec::new(),
        // Hilfslinie: gestricheltes Kreuz — der Punkt liegt auf einer
        // Verlängerung oder Normalen, nicht auf dem Objekt selbst.
        FangArt::Objekt(SnapKind::Tracking) => {
            l(&mut v, (-HALB, 0.0), (-4.0, 0.0));
            l(&mut v, (4.0, 0.0), (HALB, 0.0));
            l(&mut v, (0.0, -HALB), (0.0, -4.0));
            l(&mut v, (0.0, 4.0), (0.0, HALB));
        }
        // Winkelhalbierende: Winkel mit Mittelstrahl.
        FangArt::Objekt(SnapKind::Bisector) => {
            l(&mut v, (-8.0, 6.0), (0.0, 0.0));
            l(&mut v, (0.0, 0.0), (8.0, 6.0));
            l(&mut v, (0.0, -2.0), (0.0, -9.0));
        }
        // Maßkette parallel: zwei parallele Striche — die Lage der Maßlinie
        // neben einer vorhandenen (T0154).
        FangArt::Objekt(SnapKind::DimParallel) => {
            l(&mut v, (-9.0, -5.0), (9.0, -5.0));
            l(&mut v, (-9.0, 5.0), (9.0, 5.0));
            l(&mut v, (-9.0, -5.0), (-9.0, 5.0));
        }
        // Tangential: Kreis mit anliegender Gerade — die Linie berührt den
        // Kreis in genau einem Punkt (T0157).
        FangArt::Objekt(SnapKind::Tangent) => {
            v.push(Kreis(5.0));
            l(&mut v, (-8.5, -5.0), (8.5, -5.0));
        }
    }
    v.push(Punkt((0.0, 0.0), 1.4));
    v
}

/// Zeichnet die Marke einer Fangart um `mitte` (Bildschirmpunkte).
///
/// Farbe ist die **Fangfarbe** (`Palette::fang`) — dasselbe Grün wie die
/// Hilfslinien des erweiterten Punktfangs (T0153, Turn 2). Die Marke gehört
/// damit sichtbar zum Fang und nicht zur Auswahl (blau) oder zum Werkzeug.
///
/// Alle Striche bekommen zuerst einen hellen Rand in Papierfarbe: über einer
/// Wandschraffur oder einer dicken Linie bliebe das dünne Zeichen sonst
/// unsichtbar. Auf leerem Papier ist der Rand unsichtbar.
pub fn zeichne(p: &Painter, mitte: Pos2, art: FangArt) {
    let farbe = Palette::track();
    let rand = Palette::modell().gamma_multiply(0.9);
    let zuege = marke(art);
    for zug in &zuege {
        mal_zug(p, mitte, *zug, 3.4, rand);
    }
    for zug in &zuege {
        mal_zug(p, mitte, *zug, 1.5, farbe);
    }
}

fn mal_zug(p: &Painter, mitte: Pos2, zug: Zug, breite: f32, farbe: Color32) {
    let pt = |(x, y): (f32, f32)| Pos2::new(mitte.x + x, mitte.y + y);
    let st = Stroke::new(breite, farbe);
    match zug {
        Zug::Linie(a, b) => {
            p.line_segment([pt(a), pt(b)], st);
        }
        Zug::Kreis(r) => {
            p.circle_stroke(mitte, r, st);
        }
        Zug::Punkt(q, r) => {
            p.circle_filled(pt(q), r, farbe);
        }
        Zug::Flaeche(ecken) => {
            let pts: Vec<Pos2> = ecken.iter().map(|q| pt(*q)).collect();
            p.add(egui::Shape::convex_polygon(pts, farbe, Stroke::NONE));
        }
    }
}

/// Alle Fangarten, die eine Marke haben — Grundlage für die Vollständigkeit.
#[cfg(test)]
pub const ALLE: [FangArt; 13] = [
    FangArt::Objekt(SnapKind::Endpoint),
    FangArt::Objekt(SnapKind::SheetCorner),
    FangArt::Objekt(SnapKind::BlockBase),
    FangArt::Objekt(SnapKind::Intersection),
    FangArt::Objekt(SnapKind::Midpoint),
    FangArt::Objekt(SnapKind::Center),
    FangArt::Objekt(SnapKind::Quadrant),
    FangArt::Objekt(SnapKind::Perpendicular),
    FangArt::Objekt(SnapKind::Nearest),
    FangArt::Objekt(SnapKind::Tracking),
    FangArt::Objekt(SnapKind::Bisector),
    FangArt::Objekt(SnapKind::DimParallel),
    FangArt::Objekt(SnapKind::Tangent),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn lage(z: &Zug) -> Vec<(f32, f32)> {
        match *z {
            Zug::Linie(a, b) => vec![a, b],
            Zug::Kreis(r) => vec![(-r, -r), (r, r)],
            Zug::Punkt((x, y), r) => vec![(x - r, y - r), (x + r, y + r)],
            Zug::Flaeche(e) => e.to_vec(),
        }
    }

    /// Jede Fangart hat ein Zeichen — und es hat mehrere Bestandteile, ist
    /// also nicht nur der Fangpunkt allein.
    #[test]
    fn jede_fangart_hat_eine_marke() {
        for art in ALLE {
            let m = marke(art);
            assert!(
                m.len() >= 3,
                "{} hat nur {} Bestandteile",
                art.name(),
                m.len()
            );
        }
    }

    /// Kein Zeichen sieht aus wie ein anderes — sonst wäre die Fangart am
    /// Bild nicht zu erkennen.
    #[test]
    fn marken_unterscheiden_sich() {
        for (i, a) in ALLE.iter().enumerate() {
            for b in &ALLE[i + 1..] {
                assert_ne!(
                    marke(*a),
                    marke(*b),
                    "{} und {} sehen gleich aus",
                    a.name(),
                    b.name()
                );
            }
        }
    }

    /// Die Marke sitzt am Fangpunkt: alle Bestandteile liegen im 18-px-Feld
    /// um den Ursprung, und der Punkt selbst ist Teil jeder Marke.
    #[test]
    fn marke_liegt_am_fangpunkt() {
        for art in ALLE {
            let m = marke(art);
            let mut min = (f32::INFINITY, f32::INFINITY);
            let mut max = (f32::NEG_INFINITY, f32::NEG_INFINITY);
            for z in &m {
                for (x, y) in lage(z) {
                    min = (min.0.min(x), min.1.min(y));
                    max = (max.0.max(x), max.1.max(y));
                }
            }
            assert!(
                min.0 >= -HALB && min.1 >= -HALB && max.0 <= HALB && max.1 <= HALB,
                "{} ragt aus dem Feld ({min:?} … {max:?})",
                art.name()
            );
            assert!(
                m.contains(&Zug::Punkt((0.0, 0.0), 1.4)),
                "{} markiert den Fangpunkt nicht",
                art.name()
            );
        }
    }

    /// Endpunkt ist ein Quadrat (vier Striche gleicher Länge), Schnittpunkt
    /// ein Kreuz aus zwei Diagonalen — die klassischen Zeichen.
    #[test]
    fn endpunkt_quadrat_und_schnittpunkt_kreuz() {
        let end = marke(FangArt::Objekt(SnapKind::Endpoint));
        let striche: Vec<_> = end
            .iter()
            .filter_map(|z| match z {
                Zug::Linie(a, b) => Some((a, b)),
                _ => None,
            })
            .collect();
        assert_eq!(striche.len(), 4);
        for (a, b) in &striche {
            let dx = b.0 - a.0;
            let dy = b.1 - a.1;
            assert!(
                dx.abs() < 0.01 || dy.abs() < 0.01,
                "Endpunktzeichen ist kein achsenparalleles Quadrat"
            );
        }

        let schnitt = marke(FangArt::Objekt(SnapKind::Intersection));
        let diagonalen = schnitt
            .iter()
            .filter(|z| {
                matches!(z, Zug::Linie(a, b) if (b.0 - a.0).abs() > 10.0 && (b.1 - a.1).abs() > 10.0)
            })
            .count();
        assert_eq!(diagonalen, 2, "Schnittpunktzeichen ist kein Kreuz");
    }

    /// Der Rasterfang bekommt **kein** Zeichen: er liegt an jeder
    /// Zeigerstellung dicht unter dem Zeiger, ein Zeichen dort klebte am
    /// Mauszeiger statt am Objekt (T0153, Turn 3). Polar ebenso — es hat gar
    /// keine eigene Marke mehr.
    #[test]
    fn raster_hat_keine_marke() {
        assert!(
            marke(FangArt::Objekt(SnapKind::Grid)).is_empty(),
            "der Rasterfang darf kein Zeichen haben"
        );
        assert!(
            !ALLE.contains(&FangArt::Objekt(SnapKind::Grid)),
            "der Rasterfang steht noch in der Liste der Marken"
        );
    }

    /// Der Mittelpunkt behält sein Kreuz mit 7 px Halbbreite (T0117) und
    /// bekommt einen Kreis darum.
    #[test]
    fn zentrum_hat_kreis_und_kreuz() {
        let m = marke(FangArt::Objekt(SnapKind::Center));
        assert!(m.contains(&Zug::Kreis(6.0)));
        assert!(m.contains(&Zug::Linie((-7.0, 0.0), (7.0, 0.0))));
        assert!(m.contains(&Zug::Linie((0.0, -7.0), (0.0, 7.0))));
    }
}
