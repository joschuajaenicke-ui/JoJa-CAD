//! Unterteilung gekrümmter Geometrie in Sehnen (Glättung von Bögen).
//!
//! Ein Bogen wird für Bildschirm, PNG und PDF in kurze Sehnen zerlegt. Die
//! einzige sinnvolle Steuergröße dafür ist die **Pfeilhöhe** (Sagitta): der
//! größte Abstand zwischen Sehne und wahrer Kurve, gemessen in
//! *Ausgabe*-Einheiten (Bildschirm-Pixel bzw. Papier-Millimeter). Eine feste
//! Segmentzahl taugt nicht — sie ist bei kleinem Radius verschwendet und bei
//! großem Radius sichtbar eckig, und beides ändert sich mit dem Zoom.
//!
//! Die Segmentzahl folgt damit automatisch aus Radius und Bogenwinkel:
//! `h = r · (1 − cos(Δφ/2))` → `Δφ = 2 · acos(1 − h/r)`.
//!
//! Alle Renderer (GUI, PNG, PDF) holen ihre Unterteilung hier ab; die Werte
//! stehen als benannte Vorgaben [`CurveQuality::SCREEN`] und
//! [`CurveQuality::PRINT`] an genau einer Stelle.

use crate::geom::{Arc, Vec2};

/// Güte, mit der ein Bogen in Sehnen zerlegt wird.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveQuality {
    /// Zulässige Pfeilhöhe in Ausgabe-Einheiten (px bzw. Papier-mm).
    pub max_sagitta: f64,
    /// Kleinste Segmentzahl. Ohne Untergrenze würde ein Bogen mit grober
    /// Toleranz zum Dreieck — sichtbar als Ecke in der Rundung.
    pub min_segments: usize,
    /// Obergrenze je Bogen. Begrenzt Renderzeit und Speicher bei riesigen
    /// Radien (ein 100-m-Bogen bei starker Vergrößerung bringt sonst
    /// tausende Segmente, von denen keines mehr auf dem Schirm liegt).
    pub max_segments: usize,
    /// Kleinster Winkel je Segment (Bogenmaß) — zweite Untergrenze gegen
    /// Segmentlawinen, wenn die Pfeilhöhe gegen null geht.
    pub min_step: f64,
}

impl CurveQuality {
    /// Bildschirm: 0,15 px Abweichung — das liegt unter der Kantenglättung
    /// (1 px) und ist damit nicht mehr als Ecke zu erkennen.
    ///
    /// Die Obergrenze greift erst, wenn der Bogen auf dem Schirm einen Radius
    /// von rund zwei Millionen Pixeln hätte (etwa ein 100-m-Bogen bei
    /// 20 px/mm); bis dahin gilt die Pfeilhöhe exakt. Ein Bogen ist *ein*
    /// Pfad — 4096 Punkte kosten weniger als die früheren Kapseln
    /// (eine Form je Segment).
    pub const SCREEN: CurveQuality = CurveQuality {
        max_sagitta: 0.15,
        min_segments: 4,
        max_segments: 4096,
        min_step: std::f64::consts::TAU / 4096.0,
    };

    /// Druck: 0,05 Papier-mm, bei 300 dpi rund 0,6 px — im Ausdruck glatt,
    /// ohne die Datei mit Segmenten zu fluten.
    pub const PRINT: CurveQuality = CurveQuality {
        max_sagitta: 0.05,
        min_segments: 5,
        max_segments: 2048,
        min_step: std::f64::consts::TAU / 2048.0,
    };

    /// Segmentzahl für `arc`; `out_per_unit` ist die Abbildung von Welt-
    /// Millimetern auf Ausgabe-Einheiten (Pixel je mm bzw. Papier-mm je mm).
    pub fn segments(&self, arc: &Arc, out_per_unit: f64) -> usize {
        let tol_world = self.max_sagitta / out_per_unit.max(1e-12);
        self.segments_tol(arc, tol_world)
    }

    /// Segmentzahl bei bereits in Welt-Maß umgerechneter Pfeilhöhe.
    pub fn segments_tol(&self, arc: &Arc, tol_world: f64) -> usize {
        let sweep = arc.sweep();
        if !arc.r.is_finite() || !sweep.is_finite() || arc.r <= 1e-9 {
            // Degenerierter Bogen (Radius 0): kein Bogen, nur ein Punkt.
            return 2;
        }
        let tol = tol_world.max(1e-12);
        // Pfeilhöhe in Winkel umrechnen. Bei h >= r deckt eine einzige Sehne
        // bereits einen Halbkreis ab — mehr als π je Segment gibt es nicht.
        let step = if tol >= arc.r {
            std::f64::consts::PI
        } else {
            2.0 * (1.0 - tol / arc.r).acos()
        };
        let step = step.max(self.min_step).max(1e-9);
        let n = (sweep / step).ceil();
        let n = if n.is_finite() { n as usize } else { self.min_segments };
        n.clamp(self.min_segments, self.max_segments)
    }

    /// Zerlegt `arc` in Sehnenpunkte (inklusive Start- und Endpunkt).
    pub fn flatten(&self, arc: &Arc, out_per_unit: f64) -> Vec<Vec2> {
        let tol_world = self.max_sagitta / out_per_unit.max(1e-12);
        self.flatten_tol(arc, tol_world)
    }

    /// Zerlegt `arc` in Sehnenpunkte; `tol_world` in Welt-Millimetern.
    pub fn flatten_tol(&self, arc: &Arc, tol_world: f64) -> Vec<Vec2> {
        let n = self.segments_tol(arc, tol_world);
        if !arc.r.is_finite() || arc.r <= 1e-9 {
            // Radius 0: Anfangs- und Endpunkt fallen zusammen.
            return vec![arc.c, arc.c];
        }
        let sweep = arc.sweep();
        (0..=n)
            .map(|i| arc.c + Vec2::polar(arc.a0 + sweep * (i as f64 / n as f64), arc.r))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADII: [f64; 5] = [5.0, 50.0, 500.0, 5_000.0, 50_000.0];
    const ZOOMS: [f64; 5] = [0.02, 0.06, 0.25, 1.0, 8.0];

    /// Größte Pfeilhöhe über alle Sehnen (Abstand Sehnenmitte ↔ Kreis).
    fn max_sagitta(arc: &Arc, pts: &[Vec2]) -> f64 {
        pts.windows(2)
            .map(|w| {
                let mid = (w[0] + w[1]) * 0.5;
                let d = (mid - arc.c).len();
                (arc.r - d).abs()
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn pfeilhoehe_wird_eingehalten() {
        for r in RADII {
            for zoom in ZOOMS {
                for (a0, a1) in [(0.0, 1.5708), (0.3, 2.9), (0.0, 0.0)] {
                    let arc = Arc::new(Vec2::new(1000.0, -500.0), r, a0, a1);
                    // Bei greifender Obergrenze gilt die Pfeilhöhe nicht mehr;
                    // dieser Fall wird in `obergrenze_begrenzt_die_segmentzahl`
                    // geprüft.
                    if CurveQuality::SCREEN.segments(&arc, zoom) == CurveQuality::SCREEN.max_segments
                    {
                        continue;
                    }
                    let pts = CurveQuality::SCREEN.flatten(&arc, zoom);
                    let tol_world = CurveQuality::SCREEN.max_sagitta / zoom;
                    let h = max_sagitta(&arc, &pts);
                    assert!(
                        h <= tol_world * 1.001 + 1e-9,
                        "r={r} zoom={zoom} a={a0}..{a1}: Pfeilhöhe {h} > {tol_world}"
                    );
                }
            }
        }
    }

    #[test]
    fn alle_punkte_liegen_auf_dem_kreis() {
        let arc = Arc::new(Vec2::new(-200.0, 40.0), 750.0, 0.4, 2.2);
        for q in CurveQuality::SCREEN.flatten(&arc, 0.5) {
            let d = (q - arc.c).len();
            assert!((d - arc.r).abs() < 1e-6, "Punkt neben dem Kreis: {d}");
        }
    }

    #[test]
    fn endpunkte_treffen_die_bogenenden() {
        let arc = Arc::new(Vec2::ZERO, 300.0, 0.7, 2.4);
        let pts = CurveQuality::SCREEN.flatten(&arc, 1.0);
        assert!((pts[0] - arc.start()).len() < 1e-9);
        assert!((pts[pts.len() - 1] - arc.end()).len() < 1e-9);
    }

    #[test]
    fn zoom_erhoeht_die_segmentzahl() {
        let arc = Arc::new(Vec2::ZERO, 1200.0, 0.0, 1.5708);
        let klein = CurveQuality::SCREEN.segments(&arc, 0.05);
        let gross = CurveQuality::SCREEN.segments(&arc, 5.0);
        assert!(gross > klein, "{gross} muss größer sein als {klein}");
    }

    #[test]
    fn kleiner_radius_bleibt_sparsam() {
        // 5-mm-Bogen bei 1 px/mm: ein Viertelkreis braucht eine Handvoll
        // Segmente, nicht die Obergrenze.
        let arc = Arc::new(Vec2::ZERO, 5.0, 0.0, 1.5708);
        let n = CurveQuality::SCREEN.segments(&arc, 1.0);
        assert!((CurveQuality::SCREEN.min_segments..=16).contains(&n), "n={n}");
    }

    #[test]
    fn obergrenze_begrenzt_die_segmentzahl() {
        // Ein Riesenbogen darf die Segmentzahl nicht sprengen: lieber eine
        // etwas gröbere Kurve als tausende Punkte, von denen keiner mehr auf
        // dem Schirm liegt.
        let arc = Arc::new(Vec2::ZERO, 1.0e12, 0.0, 0.0);
        let n = CurveQuality::SCREEN.segments(&arc, 1000.0);
        assert_eq!(n, CurveQuality::SCREEN.max_segments);
        let pts = CurveQuality::SCREEN.flatten(&arc, 1000.0);
        assert_eq!(pts.len(), n + 1);
        assert!(pts.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
    }

    #[test]
    fn radius_null_ist_ein_punkt() {
        let arc = Arc::new(Vec2::new(12.0, 34.0), 0.0, 0.0, 1.0);
        let pts = CurveQuality::SCREEN.flatten(&arc, 1.0);
        assert_eq!(pts.len(), 2);
        assert!((pts[0] - pts[1]).len() < 1e-12);
        assert_eq!(CurveQuality::SCREEN.segments(&arc, 1.0), 2);
    }

    #[test]
    fn vollkreis_ist_geschlossen() {
        // a0 == a1 bedeutet Vollkreis (sweep() liefert TAU).
        let arc = Arc::new(Vec2::ZERO, 400.0, 0.0, 0.0);
        let pts = CurveQuality::SCREEN.flatten(&arc, 0.25);
        assert!(pts.len() > 24);
        assert!((pts[0] - pts[pts.len() - 1]).len() < 1e-6);
    }

    #[test]
    fn druck_ist_feiner_als_bildschirm() {
        let arc = Arc::new(Vec2::ZERO, 800.0, 0.0, 1.5708);
        // gleiche Abbildung (1 px/mm bzw. 1 Papier-mm/mm)
        assert!(
            CurveQuality::PRINT.segments(&arc, 1.0) > CurveQuality::SCREEN.segments(&arc, 1.0)
        );
    }
}
