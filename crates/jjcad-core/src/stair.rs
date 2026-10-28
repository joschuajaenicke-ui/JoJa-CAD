//! Treppenmaße nach der Schrittmaßregel: **2 S + A = 63 cm**.
//!
//! Eine gerade Treppe wird über ihre Lauflinie (Basispunkt `a` → `b`) und die
//! Breite festgelegt; die Höhe steht als Eigenschaft daneben (Geschosshöhe).
//! Aus Länge und Höhe folgt die Stufenzahl, die sich aus der umgestellten
//! Schrittmaßregel ergibt:
//!
//! ```text
//! n Stufen → S = H / n, A = L / n
//! 2 S + A = 63   ⇒   (2 H + L) / n = 63   ⇒   n = (2 H + L) / 630 mm
//! ```
//!
//! Der so bestimmte Wert wird gerundet und auf sinnvolle Grenzen begrenzt; er
//! darf im Eigenschaften-Panel überschrieben werden (dann gilt die manuelle
//! Zahl, auch wenn Länge oder Höhe sich später ändern).

use crate::geom::Vec2;

/// Sollwert der Schrittmaßregel in mm (63 cm).
pub const SCHRITTMASS_MM: f64 = 630.0;
/// Kleinste Stufenzahl (zwei Steigungen ergeben eine nutzbare Treppe).
pub const STUFEN_MIN: u32 = 2;
/// Größte Stufenzahl — begrenzt die Anzeige und die Eingabefelder.
pub const STUFEN_MAX: u32 = 60;
/// Vorgabe für die Treppenhöhe (Geschosshöhe) in mm.
pub const HOEHE_VORGABE_MM: f64 = 2750.0;

/// Maße einer geraden Treppe: Stufenzahl, Steigung S und Auftritt A (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Treppenmass {
    pub stufen: u32,
    /// Steigung S (Höhe einer Stufe) in mm.
    pub steigung: f64,
    /// Auftritt A (Tiefe einer Stufe) in mm.
    pub auftritt: f64,
}

impl Treppenmass {
    /// Ergebnis der Schrittmaßregel 2 S + A in mm (Sollwert 630).
    pub fn regel(&self) -> f64 {
        2.0 * self.steigung + self.auftritt
    }

    /// Auftrittsfläche einer Stufe: Auftritt × Treppenbreite (mm²).
    pub fn auftrittsflaeche(&self, breite: f64) -> f64 {
        self.auftritt * breite
    }

    /// Beschriftung wie im Plan: erste Zeile die Stufenzahl, zweite Zeile
    /// Steigung und Auftritt in Zentimetern (`16 Stg` / `17,2 / 29`).
    pub fn beschriftung(&self) -> String {
        format!(
            "{} Stg\n{} / {}",
            self.stufen,
            cm_komma(self.steigung, 1),
            cm_komma(self.auftritt, 0)
        )
    }
}

/// Zentimeter mit deutschem Komma.
fn cm_komma(mm: f64, stellen: usize) -> String {
    format!("{:.*}", stellen, mm / 10.0).replace('.', ",")
}

/// Optimale Stufenzahl nach der umgestellten Schrittmaßregel.
///
/// `n = (2 H + L) / 630`, gerundet und auf [`STUFEN_MIN`]…[`STUFEN_MAX`]
/// begrenzt. Unbrauchbare Eingaben (Länge oder Höhe ≤ 0) ergeben die kleinste
/// Stufenzahl, damit die Anzeige nicht in NaN läuft.
pub fn stufenzahl(len: f64, hoehe: f64) -> u32 {
    if !len.is_finite() || !hoehe.is_finite() || len <= 0.0 || hoehe <= 0.0 {
        return STUFEN_MIN;
    }
    let n = ((2.0 * hoehe + len) / SCHRITTMASS_MM).round();
    n.clamp(STUFEN_MIN as f64, STUFEN_MAX as f64) as u32
}

/// Maße einer Treppe aus Länge und Höhe.
///
/// `auto` = Stufenzahl folgt der Schrittmaßregel; sonst gilt die übergebene
/// (manuell überschriebene) Zahl.
pub fn treppenmass(len: f64, hoehe: f64, stufen: u32, auto: bool) -> Treppenmass {
    let n = if auto {
        stufenzahl(len, hoehe)
    } else {
        stufen.clamp(STUFEN_MIN, STUFEN_MAX)
    };
    let nf = n as f64;
    Treppenmass {
        stufen: n,
        steigung: if nf > 0.0 { hoehe / nf } else { 0.0 },
        auftritt: if nf > 0.0 { len / nf } else { 0.0 },
    }
}

/// Länge des Laufs (Abstand der beiden Basispunkte).
pub fn lauf_len(a: Vec2, b: Vec2) -> f64 {
    a.dist(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Zahlen aus dem Referenzbild: 4,64 m lang, 2,752 m hoch → 16 Stufen,
    /// Steigung 17,2 cm, Auftritt 29 cm, 2 S + A = 63,4 cm.
    #[test]
    fn schrittmass_ergibt_die_stufenzahl_des_referenzbildes() {
        let m = treppenmass(4640.0, 2752.0, 16, true);
        assert_eq!(m.stufen, 16);
        assert!((m.steigung - 172.0).abs() < 0.01, "S = {}", m.steigung);
        assert!((m.auftritt - 290.0).abs() < 0.01, "A = {}", m.auftritt);
        assert!((m.regel() - 634.0).abs() < 0.01);
        assert_eq!(m.beschriftung(), "16 Stg\n17,2 / 29");
    }

    /// Eine manuell überschriebene Stufenzahl gewinnt gegen die Regel.
    #[test]
    fn manuelle_stufenzahl_gilt() {
        let m = treppenmass(4640.0, 2752.0, 18, false);
        assert_eq!(m.stufen, 18);
        assert!((m.steigung - 2752.0 / 18.0).abs() < 0.01);
        assert!((m.auftritt - 4640.0 / 18.0).abs() < 0.01);
    }

    /// Ohne Länge oder Höhe bleibt die Rechnung endlich.
    #[test]
    fn unbrauchbare_eingaben_bleiben_endlich() {
        let m = treppenmass(0.0, 0.0, 0, true);
        assert_eq!(m.stufen, STUFEN_MIN);
        assert!(m.steigung.is_finite() && m.auftritt.is_finite());
    }

    /// Die Auftrittsfläche ist Auftritt × Breite.
    #[test]
    fn auftrittsflaeche_ist_auftritt_mal_breite() {
        let m = treppenmass(4640.0, 2752.0, 16, true);
        assert!((m.auftrittsflaeche(1000.0) - 290_000.0).abs() < 1.0);
    }
}
