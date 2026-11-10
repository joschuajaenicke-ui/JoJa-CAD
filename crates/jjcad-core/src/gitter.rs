//! Zeichenraster (T0144): Art, Modus und Fang.
//!
//! Ein Raster ist ein **Gitterpunkt-Verband**: zwei Grundvektoren `u` und `v`
//! spannen die Zellen auf, die Gitterpunkte sind `i·u + j·v`. Gezeichnet werden
//! die Linienfamilien entlang dieser Vektoren (bei der Isometrie zusätzlich
//! entlang `u + v`) — jede gezeichnete Linie läuft damit durch Gitterpunkte,
//! und der Fang (`Raster::fangen`) landet auf genau diesen Punkten. Anzeige und
//! Fang kommen aus derselben Rechnung; ein zweites Rastermodell gibt es nicht.
//!
//! Die Weite `step` ist die Kantenlänge der Zelle in mm. Die Rasterarten sind
//! die üblichen Projektionsachsen der Architekturzeichnung:
//!
//! * **Rechtwinklig** — 0°/90°, das gewohnte Zeichenraster.
//! * **Isometrisch** — 30°/150° mit der Senkrechten als dritter Familie; die
//!   Zellen sind gleichseitige Dreiecke (alle drei Familien haben denselben
//!   Linienabstand `step·√3/2`).
//! * **Dimetrie** — 7° und 138° (die zweite Achse mit halber Verkürzung), wie
//!   die dimetrische Projektion nach DIN 5.
//! * **Trimetrie** — 15° und 140° mit eigener Verkürzung der zweiten Achse;
//!   drei verschiedene Achsenverkürzungen, daher der Name.
//!
//! Der **Modus** (`Rastermodus`) betrifft nur die Anzeige: „Skaliert“ dünnt die
//! gezeichneten Linien bei kleinem Zoom aus (die sichtbaren Linien bleiben ein
//! Teil der feinen Gitterpunkte), „Fest“ zeichnet immer die eingestellte Weite.
//! Gefangen wird in beiden Modi auf die eingestellte Weite.

use crate::geom::{v, Vec2};
use serde::{Deserialize, Serialize};

/// Art des Zeichenrasters (Achsen der Gitterzellen).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rasterart {
    /// 0°/90° — das gewohnte Zeichenraster.
    #[default]
    Rechteckig,
    /// 30°/150°/90° — gleichseitige Dreieckszellen.
    Isometrisch,
    /// 7°/138° — dimetrische Projektion.
    Dimetrie,
    /// 15°/140° — trimetrische Projektion.
    Trimetrie,
}

impl Rasterart {
    /// Alle Arten in der Reihenfolge der Auswahl.
    pub const ALL: [Rasterart; 4] = [
        Rasterart::Rechteckig,
        Rasterart::Isometrisch,
        Rasterart::Dimetrie,
        Rasterart::Trimetrie,
    ];

    /// Schlüssel in der `.jjcad`-Datei und in der Befehlsschicht.
    pub fn key(self) -> &'static str {
        match self {
            Rasterart::Rechteckig => "rechteckig",
            Rasterart::Isometrisch => "isometrisch",
            Rasterart::Dimetrie => "dimetrie",
            Rasterart::Trimetrie => "trimetrie",
        }
    }

    /// Schlüssel lesen; unbekannte Werte geben `None` (nie still die Vorgabe).
    pub fn from_key(s: &str) -> Option<Rasterart> {
        match s.trim().to_lowercase().as_str() {
            "rechteckig" | "rect" | "rechteck" => Some(Rasterart::Rechteckig),
            "isometrisch" | "iso" => Some(Rasterart::Isometrisch),
            "dimetrie" | "dimetrisch" => Some(Rasterart::Dimetrie),
            "trimetrie" | "trimetrisch" => Some(Rasterart::Trimetrie),
            _ => None,
        }
    }

    /// Grundvektoren der Zelle (mm) für die Rasterweite `step`.
    pub fn grundvektoren(self, step: f64) -> (Vec2, Vec2) {
        let s = step.max(0.0);
        match self {
            Rasterart::Rechteckig => (v(s, 0.0), v(0.0, s)),
            Rasterart::Isometrisch => (
                Vec2::polar(30f64.to_radians(), s),
                Vec2::polar(150f64.to_radians(), s),
            ),
            // Dimetrie: die zweite Achse ist halb verkürzt (DIN 5).
            Rasterart::Dimetrie => (
                Vec2::polar(7f64.to_radians(), s),
                Vec2::polar(138f64.to_radians(), 0.5 * s),
            ),
            // Trimetrie: drei verschiedene Verkürzungen; hier 1 und 0,6.
            Rasterart::Trimetrie => (
                Vec2::polar(15f64.to_radians(), s),
                Vec2::polar(140f64.to_radians(), 0.6 * s),
            ),
        }
    }

    /// Wird die dritte Linienfamilie entlang `u + v` gezeichnet? Nur die
    /// Isometrie braucht sie (die Senkrechte); bei den übrigen Arten wären es
    /// keine Projektionsachsen.
    pub fn dritte_familie(self) -> bool {
        matches!(self, Rasterart::Isometrisch)
    }
}

/// Wie sich die gezeichnete Rasterweite zur Ansicht verhält.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rastermodus {
    /// Vorgabe: bei kleinem Zoom werden die Linien ausgedünnt (×10 je Stufe),
    /// damit das Raster nicht zuläuft.
    #[default]
    Skaliert,
    /// Das Raster bleibt bei jedem Zoom auf der eingestellten Weite.
    Fest,
}

impl Rastermodus {
    pub const ALL: [Rastermodus; 2] = [Rastermodus::Skaliert, Rastermodus::Fest];

    pub fn key(self) -> &'static str {
        match self {
            Rastermodus::Skaliert => "skaliert",
            Rastermodus::Fest => "fest",
        }
    }

    pub fn from_key(s: &str) -> Option<Rastermodus> {
        match s.trim().to_lowercase().as_str() {
            "skaliert" | "auto" | "ansicht" => Some(Rastermodus::Skaliert),
            "fest" | "fix" => Some(Rastermodus::Fest),
            _ => None,
        }
    }
}

/// Kleinste Zellbreite in Bildschirmpunkten, bis zu der im Modus „Skaliert“
/// ausgedünnt wird (bisheriges Verhalten der Zeichenfläche).
pub const MIN_ZELLE_PX: f64 = 7.0;

/// Gezeichnete Rasterweite: im Modus „Skaliert“ ×10, bis eine Zelle mindestens
/// [`MIN_ZELLE_PX`] breit ist; im Modus „Fest“ unverändert.
pub fn sichtbare_weite(step: f64, modus: Rastermodus, zoom: f64) -> f64 {
    let mut w = step.max(0.0);
    if modus == Rastermodus::Fest {
        return w;
    }
    let z = zoom.max(1e-9);
    let mut stufen = 0;
    while w * z < MIN_ZELLE_PX && stufen < 12 {
        w *= 10.0;
        stufen += 1;
    }
    w
}

/// Eine Linienfamilie des Rasters: alle Linien mit Richtung `richtung`, deren
/// Normalenabstand ein Vielfaches von `abstand` ist.
#[derive(Clone, Copy, Debug)]
pub struct Familie {
    /// Richtung der Linien (Einheitsvektor).
    pub richtung: Vec2,
    /// Normale der Linien (Einheitsvektor) — die Lage der Linie ist
    /// `p · normale = k · abstand`.
    pub normale: Vec2,
    /// Abstand zweier benachbarter Linien in mm.
    pub abstand: f64,
}

/// Das Raster einer Zeichnung: Art und Weite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Raster {
    pub art: Rasterart,
    pub step: f64,
}

impl Raster {
    pub fn neu(art: Rasterart, step: f64) -> Raster {
        Raster {
            art,
            step: step.max(0.0),
        }
    }

    /// Linienfamilien zum Zeichnen. Leer, wenn die Weite 0 ist.
    pub fn familien(&self) -> Vec<Familie> {
        if self.step <= 0.0 {
            return Vec::new();
        }
        let (u, w) = self.art.grundvektoren(self.step);
        let flaeche = u.cross(w).abs();
        if flaeche < 1e-12 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(3);
        let mut push = |d: Vec2| {
            let richtung = d.normalized();
            let abstand = flaeche / d.len();
            if abstand > 1e-9 {
                out.push(Familie {
                    richtung,
                    normale: richtung.perp(),
                    abstand,
                });
            }
        };
        push(u);
        push(w);
        if self.art.dritte_familie() {
            push(u + w);
        }
        out
    }

    /// Nächstgelegener Gitterpunkt zu `p`.
    ///
    /// `p` wird in die Basis `(u, v)` zerlegt, beide Anteile werden gerundet und
    /// wieder zusammengesetzt. Damit liegt das Ergebnis immer auf dem Verband —
    /// auch bei den schrägen Arten, wo ein achsenweises Runden (wie im
    /// rechtwinkligen Raster) die Zelle verlassen würde.
    pub fn fangen(&self, p: Vec2) -> Vec2 {
        if self.step <= 0.0 {
            return p;
        }
        let (u, w) = self.art.grundvektoren(self.step);
        let det = u.cross(w);
        if det.abs() < 1e-12 {
            return p;
        }
        let a = p.cross(w) / det;
        let b = u.cross(p) / det;
        u * a.round() + w * b.round()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rechtwinklig_rastet_wie_bisher() {
        let r = Raster::neu(Rasterart::Rechteckig, 100.0);
        assert_eq!(r.fangen(v(149.0, 51.0)), v(100.0, 100.0));
        assert_eq!(r.fangen(v(-49.0, -51.0)), v(0.0, -100.0));
        assert_eq!(r.fangen(v(250.0, 250.0)), v(300.0, 300.0));
    }

    #[test]
    fn jede_art_rastet_auf_einen_gitterpunkt() {
        for art in Rasterart::ALL {
            let r = Raster::neu(art, 250.0);
            let (u, w) = art.grundvektoren(250.0);
            for p in [v(37.0, 11.0), v(-310.0, 480.0), v(1.0, 1.0), v(999.0, -222.0)] {
                let q = r.fangen(p);
                // q muss eine ganzzahlige Kombination der Grundvektoren sein.
                let det = u.cross(w);
                let a = q.cross(w) / det;
                let b = u.cross(q) / det;
                assert!(
                    (a - a.round()).abs() < 1e-6 && (b - b.round()).abs() < 1e-6,
                    "{:?}: {:?} liegt nicht auf dem Verband (a={a}, b={b})",
                    art,
                    q
                );
                // Der Fang ist ein fester Punkt: zweimal fangen ändert nichts.
                assert!((r.fangen(q) - q).len() < 1e-6, "{:?} ist nicht standfest", art);
                // Und er liegt in der Nähe des Zeigers (halbe Zelle je Achse).
                assert!(
                    (q - p).len() <= u.len() + w.len(),
                    "{:?}: Fang {:?} zu weit weg von {:?}",
                    art,
                    q,
                    p
                );
            }
        }
    }

    #[test]
    fn isometrie_hat_drei_familien_mit_gleichem_abstand() {
        let r = Raster::neu(Rasterart::Isometrisch, 100.0);
        let f = r.familien();
        assert_eq!(f.len(), 3);
        let h = 100.0 * 3f64.sqrt() / 2.0;
        for fam in &f {
            assert!((fam.abstand - h).abs() < 1e-6, "Abstand {}", fam.abstand);
        }
        // 30°, 150° und 90° (die dritte Familie ist die Senkrechte).
        let mut winkel: Vec<i64> = f
            .iter()
            .map(|fam| (fam.richtung.angle().to_degrees().round() as i64).rem_euclid(180))
            .collect();
        winkel.sort();
        assert_eq!(winkel, vec![30, 90, 150]);
    }

    #[test]
    fn rechteck_hat_zwei_familien_im_abstand_der_weite() {
        let f = Raster::neu(Rasterart::Rechteckig, 250.0).familien();
        assert_eq!(f.len(), 2);
        assert!((f[0].abstand - 250.0).abs() < 1e-9);
        assert!((f[1].abstand - 250.0).abs() < 1e-9);
    }

    #[test]
    fn sichtbare_weite_dünnt_nur_im_modus_skaliert() {
        assert_eq!(sichtbare_weite(100.0, Rastermodus::Skaliert, 1.0), 100.0);
        // 0,01 px je mm: 100 mm sind 1 px → eine Zehnerstufe, 1000 mm = 10 px.
        assert_eq!(sichtbare_weite(100.0, Rastermodus::Skaliert, 0.01), 1000.0);
        // Fest bleibt bei jedem Zoom bei der eingestellten Weite.
        assert_eq!(sichtbare_weite(100.0, Rastermodus::Fest, 0.01), 100.0);
        assert_eq!(sichtbare_weite(100.0, Rastermodus::Fest, 40.0), 100.0);
    }

    #[test]
    fn schluessel_sind_stabil() {
        for art in Rasterart::ALL {
            assert_eq!(Rasterart::from_key(art.key()), Some(art));
        }
        for m in Rastermodus::ALL {
            assert_eq!(Rastermodus::from_key(m.key()), Some(m));
        }
        assert_eq!(Rasterart::from_key("quatsch"), None);
        assert_eq!(Rastermodus::from_key("quatsch"), None);
    }
}
