//! Schriftgrößen der Zeichenfläche: Rasterdeckel und Größenleiter (T0114).
//!
//! egui rastert jede Glyphe in **genau der angefragten Pixelgröße** in einen
//! gemeinsamen Schriftatlas. Die Zeichenfläche fragte bisher die volle
//! Bildschirmgröße an (Höhe × Zoom × 1,32): ein 250-mm-Text ist bei 600 %
//! 1980 px hoch, ein einzelnes „A" belegt dann rund 3,6 Mio. Atlas-Pixel. Der
//! Atlas (8192 × 8192) läuft über, egui baut ihn neu — dabei werden 268 MB
//! kopiert und alle Glyphen neu gerastert; das Bild friert ein.
//!
//! Zwei Maßnahmen greifen zusammen:
//!
//! * **Deckel** ([`RASTER_DECKEL_PX`]): gerastert wird höchstens in 256 px. Der
//!   Rest kommt aus der Skalierung der fertigen Galley
//!   ([`skaliere`]) — die Zeichen-Maschen werden gestreckt, es entstehen
//!   **keine** neuen Atlas-Einträge.
//! * **Leiter** ([`LEITER_FAKTOR`]): die Rastergröße liegt auf festen Stufen
//!   (1,25er-Schritte). Beim Zoomen wiederholen sich die Größen, der
//!   Galley-Zwischenspeicher trifft wieder, statt für jeden Zwischenwert neue
//!   Glyphen zu erzeugen.
//!
//! Beides zusammen hält den Atlas klein: alle Stufen bis 256 px brauchen
//! zusammen rund 7 % des Atlasses, ein einzelner Text hat höchstens eine
//! Handvoll Einträge statt beliebig vieler.
//!
//! Der Preis ist bewusst: oberhalb von 256 px wird die Schrift weich
//! (gestreckte Rasterung). Bei 600 % ist ein 250-mm-Text ohnehin größer als das
//! Fenster; kleine Schriften (2,5 mm) bleiben bei jedem Zoom unter dem Deckel
//! und damit scharf.

use std::sync::Arc;

use egui::{Color32, Pos2};

/// Höchste Rastergröße einer Glyphe in Pixeln — **Vorgabe**.
///
/// Der Deckel ist eine Einstellung (`UserPrefs::textaufloesung`: 128, 256, 512,
/// 1024 px, T0114 Turn 3). 256 px ist die Vorgabe: ein Zeichen belegt dort rund
/// 60 000 Atlas-Pixel (1/60 des Werts bei 1980 px), und die Stufen der Leiter
/// bis hierher füllen zusammen etwa 7 % des Atlasses. Höhere Stufen machen die
/// Schrift im hohen Zoom schärfer, brauchen aber entsprechend mehr Atlas.
pub const RASTER_DECKEL_PX: f32 = 256.0;

/// Kleinste Rastergröße in Pixeln — darunter ist Text nicht mehr lesbar, und
/// egui rastet ohnehin nicht feiner.
pub const RASTER_MIN_PX: f32 = 3.0;

/// Stufenfaktor der Größenleiter (1,25 = 20 Stufen von 3 px bis 256 px).
pub const LEITER_FAKTOR: f32 = 1.25;

/// Rastergröße und Skalierfaktor für eine gewünschte Schriftgröße in Pixeln.
///
/// `deckel` ist die eingestellte Höchstgröße (`Textaufloesung::px`). Die
/// Rastergröße ist die nächstgelegene Leiterstufe unterhalb des Deckels; der
/// Faktor bringt die gerasterte Galley auf die gewünschte Größe
/// (`gewünscht = Rastergröße × Faktor`).
pub fn raster_und_skalierung(gewuenscht: f32, deckel: f32) -> (f32, f32) {
    let gewuenscht = gewuenscht.max(RASTER_MIN_PX);
    let stufe = leiterstufe(gewuenscht.min(deckel), deckel);
    (stufe, gewuenscht / stufe)
}

/// Nächstgelegene Stufe der Größenleiter (1,25er-Schritte ab
/// [`RASTER_MIN_PX`]), begrenzt auf [`RASTER_MIN_PX`]…`deckel`.
pub fn leiterstufe(size: f32, deckel: f32) -> f32 {
    let n = (size.max(RASTER_MIN_PX) / RASTER_MIN_PX).ln() / LEITER_FAKTOR.ln();
    let stufe = RASTER_MIN_PX * LEITER_FAKTOR.powf(n.round());
    stufe.clamp(RASTER_MIN_PX, deckel.max(RASTER_MIN_PX))
}

/// Fertige Galley auf die gewünschte Größe strecken.
///
/// `Shape::transform` skaliert auch `TextShape::pos` — die Form wird deshalb im
/// Ursprung gebaut und erst danach (vom Aufrufer) an den Anker gesetzt. Die
/// Galley selbst trägt danach die **skalierte** Größe in `rect`, sodass
/// `canvas::text_ort` den Anker richtig rechnet (Mitte/unten).
pub fn skaliere(galley: Arc<egui::Galley>, faktor: f32) -> Arc<egui::Galley> {
    if (faktor - 1.0).abs() < 1e-3 {
        return galley;
    }
    let mut shape = egui::Shape::Text(egui::epaint::TextShape::new(
        Pos2::ZERO,
        galley,
        Color32::WHITE,
    ));
    shape.transform(egui::emath::TSTransform::from_scaling(faktor));
    match shape {
        egui::Shape::Text(t) => t.galley,
        _ => unreachable!("aus einer Textform wird wieder eine Textform"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Leiter ist stetig, monoton und hält jeden eingestellten Deckel ein.
    #[test]
    fn leiter_haelt_den_deckel_und_waechst_monoton() {
        for deckel in [128.0_f32, 256.0, 512.0, 1024.0] {
            let mut vorher = 0.0;
            let mut size = RASTER_MIN_PX;
            while size <= 4096.0 {
                let (raster, faktor) = raster_und_skalierung(size, deckel);
                assert!(
                    raster <= deckel,
                    "{size} px bei Deckel {deckel}: Rastergröße {raster} zu groß"
                );
                assert!(
                    raster >= RASTER_MIN_PX,
                    "{size} px: Rastergröße {raster} zu klein"
                );
                assert!(
                    (raster * faktor - size).abs() <= size * 1e-4 + 1e-3,
                    "{size} px: {raster} × {faktor} = {}",
                    raster * faktor
                );
                assert!(
                    raster >= vorher,
                    "{size} px: Rastergröße fällt ({vorher} → {raster})"
                );
                vorher = raster;
                size *= 1.05;
            }
        }
    }

    /// Oberhalb des Deckels wächst nur noch der Faktor — die Rastergröße bleibt
    /// stehen (kein neuer Atlas-Eintrag).
    #[test]
    fn ueber_dem_deckel_bleibt_die_rastergroesse_gleich() {
        let (raster_600, faktor_600) = raster_und_skalierung(1980.0, RASTER_DECKEL_PX);
        let (raster_2000, faktor_2000) = raster_und_skalierung(6600.0, RASTER_DECKEL_PX);
        assert_eq!(raster_600, RASTER_DECKEL_PX);
        assert_eq!(raster_2000, RASTER_DECKEL_PX);
        assert!(faktor_2000 > faktor_600, "der Faktor muss weiter wachsen");
    }

    /// Nur wenige Stufen: ein Zoomlauf über den ganzen Bereich erzeugt höchstens
    /// eine Handvoll verschiedener Rastergrößen.
    #[test]
    fn zoomlauf_trifft_wenige_stufen() {
        let mut stufen = std::collections::BTreeSet::new();
        let mut zoom = 0.01_f32;
        while zoom <= 20.0 {
            let gewuenscht = 250.0 * zoom * 1.32;
            stufen.insert(raster_und_skalierung(gewuenscht, RASTER_DECKEL_PX).0.to_bits());
            zoom *= 1.02;
        }
        assert!(
            stufen.len() <= 24,
            "{} verschiedene Rastergrößen im Zoomlauf",
            stufen.len()
        );
    }

    /// Die vier Einstellungen wirken: 128 px rastert gröber als 256, 512 und
    /// 1024 — der Faktor wächst entsprechend, die Rastergröße bleibt unter der
    /// eingestellten Grenze (T0114, Turn 3).
    #[test]
    fn eingestellter_deckel_bestimmt_die_rastergroesse() {
        use jjcad_core::persist::Textaufloesung;
        let gewuenscht = 1980.0_f32;
        let mut vorher = 0.0;
        for m in Textaufloesung::ALL {
            let (raster, faktor) = raster_und_skalierung(gewuenscht, m.px());
            assert!(
                raster <= m.px(),
                "{}: Rastergröße {raster} über der Einstellung",
                m.label()
            );
            assert!(
                raster > vorher,
                "{}: Rastergröße {raster} wächst nicht ({vorher} davor)",
                m.label()
            );
            assert!(
                (raster * faktor - gewuenscht).abs() < 1.0,
                "{}: {raster} × {faktor} ≠ {gewuenscht}",
                m.label()
            );
            vorher = raster;
        }
        // Die Vorgabe ist 256 px.
        assert_eq!(Textaufloesung::default(), Textaufloesung::Px256);
        assert_eq!(Textaufloesung::default().px(), RASTER_DECKEL_PX);
    }
}
