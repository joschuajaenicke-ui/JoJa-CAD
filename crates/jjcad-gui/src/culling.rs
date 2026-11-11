//! Culling (T0167): Was vollständig außerhalb der Ansicht liegt, wird nicht
//! gezeichnet.
//!
//! Die Anzeigeliste (`render::render_document`) enthält **alle** Objekte der
//! Zeichnung — sie hängt nur an Revision und Zoom und wird zwischengespeichert.
//! Beim Zeichnen entscheidet deshalb dieser Baustein je Grafikelement, ob es
//! überhaupt auf die Fläche kann: liegt seine Ausdehnung ganz außerhalb des
//! Bereichs, entfällt der Malaufwand (bei Schraffuren und Dämmungsbändern der
//! größte Posten eines Bildes).
//!
//! Zwei Dinge sind dabei wichtig:
//!
//! * **Nie zu viel wegwerfen.** Geprüft wird gegen die *Bildschirmhülle* der
//!   Welt-Box — bei gedrehter Ansicht ist das die achsenparallele Hülle des
//!   gedrehten Rechtecks, also größer als die Figur. Dazu kommt ein Rand für
//!   die halbe Strichbreite: eine dicke Linie ragt über ihre Mittellinie
//!   hinaus. Was übrig bleibt, wird gezeichnet; ein Fehler in der Rechnung
//!   darf nur *mehr* zeichnen, nie weniger.
//! * **Papier und Rasterausgabe bleiben vollständig.** Culling ist eine
//!   Eigenschaft der Anzeige (Ansichtsfenster des Nutzers), nicht der
//!   Zeichnung: PDF- und PNG-Schreiber rufen dieselbe Anzeigeliste ohne
//!   Culling auf und geben weiterhin jedes Objekt aus.
//!
//! Der Prüfbetrieb (Leistungsanzeige, Umschalt+F3) verkleinert den Bereich um
//! [`VERKLEINERUNG`] je Seite — dann verschwinden Objekte sichtbar vor dem
//! Bildrand, und das lila Rechteck ([`FARBE`]) zeigt, wo die Grenze liegt.

use egui::{Color32, Pos2, Rect};
use jjcad_core::geom::{BBox, Vec2};

/// Anteil, um den der Prüfbereich je Seite schrumpft.
///
/// 0,125 je Seite heißt: Breite und Höhe sind 75 % der Ansichtsfläche, das
/// Rechteck ist damit „etwa ein Viertel kleiner" und deutlich im Bild zu sehen.
pub const VERKLEINERUNG: f32 = 0.125;

/// Farbe des Prüfrechtecks in der Zeichenfläche (lila, unverwechselbar mit
/// Auswahl-, Hover- und Akzentfarben).
pub const FARBE: Color32 = Color32::from_rgb(0xB4, 0x5C, 0xF0);

/// Der Bereich, in dem gezeichnet wird.
///
/// `verkleinern` ist der Prüfbetrieb aus der Leistungsanzeige; aus heißt: der
/// Bereich ist die ganze Zeichenfläche.
pub fn bereich(flaeche: Rect, verkleinern: bool) -> Rect {
    if verkleinern {
        flaeche.shrink2(egui::vec2(
            flaeche.width() * VERKLEINERUNG,
            flaeche.height() * VERKLEINERUNG,
        ))
    } else {
        flaeche
    }
}

/// Bildschirmhülle einer Welt-Box.
///
/// Die vier Ecken werden einzeln umgerechnet und zur achsenparallelen Hülle
/// zusammengefasst. Bei gedrehter Ansicht ist das **größer** als das gedrehte
/// Rechteck — die Prüfung bleibt damit gutmütig (siehe Modulkopf).
pub fn huelle(bb: &BBox, w2s: &dyn Fn(Vec2) -> Pos2) -> Rect {
    let mut min = Pos2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, y) in [
        (bb.min.x, bb.min.y),
        (bb.max.x, bb.min.y),
        (bb.max.x, bb.max.y),
        (bb.min.x, bb.max.y),
    ] {
        let p = w2s(Vec2::new(x, y));
        // `f32::min`/`max` übergehen NaN — eine kaputte Koordinate lässt die
        // Hülle dadurch unendlich werden und das Element stehen (unten).
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    Rect::from_min_max(min, max)
}

/// Liegt das Grafikelement **vollständig** außerhalb des Bereichs?
///
/// `rand` ist die Zugabe in Pixeln (halbe Strichbreite plus ein Zuschlag für
/// die Kantenglättung). Was sich nicht vermessen lässt — leere oder
/// unendliche Ausdehnung —, wird gezeichnet: im Zweifel für das Objekt.
pub fn ausserhalb(bb: &BBox, w2s: &dyn Fn(Vec2) -> Pos2, bereich: Rect, rand: f32) -> bool {
    if !bb.is_valid() {
        return false;
    }
    let h = huelle(bb, w2s);
    if !h.min.x.is_finite() || !h.min.y.is_finite() || !h.max.x.is_finite() || !h.max.y.is_finite()
    {
        return false;
    }
    !bereich.expand(rand).intersects(h)
}

#[cfg(test)]
mod tests {
    //! T0167: Bereich und Prüfung des Cullings.

    use super::*;

    fn flaeche() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), egui::vec2(1000.0, 800.0))
    }

    /// Welt = Bildschirm (Zoom 1, kein Versatz) — damit sind die Zahlen im
    /// Test ablesbar.
    fn w2s(p: Vec2) -> Pos2 {
        Pos2::new(p.x as f32, p.y as f32)
    }

    fn box_von(x0: f64, y0: f64, x1: f64, y1: f64) -> BBox {
        BBox::new(Vec2::new(x0, y0), Vec2::new(x1, y1))
    }

    #[test]
    fn pruefbereich_ist_ein_viertel_kleiner() {
        let r = bereich(flaeche(), true);
        let (w, h) = (r.width(), r.height());
        assert!(
            (w - flaeche().width() * 0.75).abs() < 0.01,
            "Breite {w} statt 75 % von {}",
            flaeche().width()
        );
        assert!(
            (h - flaeche().height() * 0.75).abs() < 0.01,
            "Höhe {h} statt 75 % von {}",
            flaeche().height()
        );
        // Gleiche Mitte: der Bereich liegt in der Ansichtsfläche, nicht daneben.
        assert!((r.center() - flaeche().center()).length() < 0.01);
        assert!(flaeche().contains_rect(r));
        // Aus heißt: die ganze Fläche.
        assert_eq!(bereich(flaeche(), false), flaeche());
    }

    #[test]
    fn ganz_aussen_liegt_aussen() {
        let r = bereich(flaeche(), false);
        for bb in [
            box_von(-500.0, 100.0, -10.0, 200.0),     // links
            box_von(1200.0, 100.0, 1500.0, 200.0),    // rechts
            box_von(200.0, -300.0, 300.0, 10.0),      // oben
            box_von(200.0, 900.0, 300.0, 1200.0),     // unten
            box_von(-500.0, -500.0, -10.0, -10.0),    // schräg
        ] {
            assert!(
                ausserhalb(&bb, &w2s, r, 0.0),
                "Box {bb:?} liegt außerhalb und muss verworfen werden"
            );
        }
    }

    #[test]
    fn was_hineinragt_bleibt() {
        let r = bereich(flaeche(), false);
        for bb in [
            box_von(200.0, 200.0, 300.0, 300.0),  // ganz innen
            box_von(90.0, 200.0, 110.0, 300.0),   // ragt links herein
            box_von(1090.0, 200.0, 1110.0, 300.0),// ragt rechts herein
            box_von(-100.0, -100.0, 2000.0, 2000.0), // umschließt alles
        ] {
            assert!(
                !ausserhalb(&bb, &w2s, r, 0.0),
                "Box {bb:?} ragt in den Bereich und muss gezeichnet werden"
            );
        }
    }

    #[test]
    fn der_rand_haelt_dicke_striche_im_bild() {
        // Die Mittellinie liegt 5 px außerhalb; ein 20 px breiter Strich ragt
        // trotzdem herein. Ohne Rand verschwände er.
        let r = bereich(flaeche(), false);
        let bb = box_von(95.0, 300.0, 90.0, 400.0);
        assert!(ausserhalb(&bb, &w2s, r, 0.0), "ohne Rand liegt die Linie draußen");
        assert!(
            !ausserhalb(&bb, &w2s, r, 10.0),
            "mit halber Strichbreite als Rand bleibt sie im Bild"
        );
    }

    #[test]
    fn gedrehte_ansicht_wirft_nichts_sichtbares_weg() {
        // Ansicht um 45° gedreht: die Welt-Box wird zur Bildschirmhülle, die
        // größer ist als die Figur — es darf nichts Sichtbares verschwinden.
        let view = crate::app::View {
            center: Vec2::new(600.0, 450.0),
            zoom: 1.0,
            rot: std::f64::consts::FRAC_PI_4,
        };
        let fl = flaeche();
        let dreh = |p: Vec2| crate::app::world_to_screen(fl, view, p);
        let r = bereich(fl, false);
        // Kleine Linie genau in der Bildmitte.
        let bb = box_von(590.0, 440.0, 610.0, 460.0);
        assert!(!ausserhalb(&bb, &dreh, r, 2.0), "die Linie in der Mitte fehlt");
        // Weit weg in Weltkoordinaten.
        let weit = box_von(59000.0, 44000.0, 61000.0, 46000.0);
        assert!(ausserhalb(&weit, &dreh, r, 2.0));
    }

    #[test]
    fn unvermessbares_wird_gezeichnet() {
        let r = bereich(flaeche(), false);
        // Leere Box (kein Punkt).
        assert!(!ausserhalb(&BBox::EMPTY, &w2s, r, 0.0));
        // Unendliche Koordinate: nicht vermessbar → zeichnen.
        let unendlich = BBox::new(Vec2::new(f64::NEG_INFINITY, 0.0), Vec2::new(f64::INFINITY, 1.0));
        assert!(!ausserhalb(&unendlich, &w2s, r, 0.0));
    }
}
