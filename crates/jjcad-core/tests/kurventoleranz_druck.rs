//! T0172: Die Kurventoleranz aus den Einstellungen („Grafik“) gilt nur für die
//! Zeichenfläche. PNG und PDF zerlegen Rundungen unabhängig davon mit
//! `CurveQuality::PRINT` — dasselbe Bild bei 0,05 px und bei 2,0 px.
//!
//! Der Prüfweg setzt eine Wegwerf-Datenwurzel mit einer `prefs.json`, die den
//! Wert ausdrücklich trägt. Damit beißt der Test, sobald der Druckpfad die
//! Einstellung liest (etwa über `UserPrefs::load()` im Rasterer): dann
//! unterscheiden sich die Bilder.

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::Vec2;
use jjcad_core::model::{Entity, EntityKind};
use jjcad_core::persist::UserPrefs;
use jjcad_core::raster;
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::tessellate::CurveQuality;
use jjcad_core::Document;

/// Eine Zeichnung mit den drei Rundungen, die die Einstellung betrifft:
/// Vollkreis, Viertelbogen und ein flacher Bogen.
fn zeichnung() -> Document {
    let mut doc = Document::new();
    let layer = doc.layers[0].id;
    for (r, a0, a1) in [(400.0, 0.0, 0.0), (700.0, 0.0, 1.5708), (300.0, 0.4, 2.6)] {
        let id = doc.new_id();
        let mut e = Entity::new(id, layer, EntityKind::Arc { c: Vec2::ZERO, r, a0, a1 });
        e.style.line_weight = Some(0.5);
        doc.add_entity(e);
    }
    doc
}

fn optionen() -> RasterOptions {
    RasterOptions {
        width_px: 500,
        height_px: 500,
        bbox: None,
        paper: true,
        annot_scale: None,
        sheet: None,
    }
}

/// Wegwerf-Datenwurzel mit einer `prefs.json`, die die Kurventoleranz trägt.
///
/// Die Wurzel steht im Prozess fest (`daten::WURZEL` ist ein `OnceLock`) — der
/// Wert wird deshalb in **derselben** Datei umgeschrieben, nicht in einer
/// zweiten Wurzel abgelegt.
struct WegwerfWurzel {
    dir: std::path::PathBuf,
}

impl WegwerfWurzel {
    fn neu() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "jjcad-t0172-druck-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("Wegwerf-Wurzel");
        jjcad_core::daten::setze_wurzel(&dir);
        Self { dir }
    }

    /// Schreibt `prefs.json` mit der gewünschten Kurventoleranz.
    fn schreibe(&self, px: f64) {
        let mut prefs = UserPrefs::default();
        prefs.kurven_toleranz_px = px;
        std::fs::write(
            self.dir.join("prefs.json"),
            serde_json::to_string(&prefs).unwrap(),
        )
        .expect("prefs.json");
    }
}

impl Drop for WegwerfWurzel {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Das PNG einer Zeichnung mit Bögen ist mit beiden Einstellungen pixelgleich:
/// der Druckpfad liest die Kurventoleranz nicht.
#[test]
fn png_bleibt_gleich_egal_wie_gross_die_kurventoleranz_ist() {
    let doc = zeichnung();

    let wurzel = WegwerfWurzel::neu();

    wurzel.schreibe(0.05);
    assert_eq!(UserPrefs::load().kurven_toleranz_px, 0.05);
    let fein = raster::render_model_pixmap(&doc, &optionen()).expect("Pixmap fein");

    wurzel.schreibe(2.0);
    assert_eq!(UserPrefs::load().kurven_toleranz_px, 2.0);
    let grob = raster::render_model_pixmap(&doc, &optionen()).expect("Pixmap grob");

    assert_eq!(fein.width(), grob.width());
    assert_eq!(fein.height(), grob.height());
    assert!(
        fein.pixels() == grob.pixels(),
        "das PNG unterscheidet sich — der Druckpfad liest die Kurventoleranz"
    );

    // Gegenprobe, dass im Bild überhaupt Rundungen liegen (kein leeres Bild,
    // bei dem die Gleichheit nichts aussagt).
    let dunkel = fein
        .pixels()
        .iter()
        .filter(|p| p.alpha() > 128 && p.red() < 100 && p.green() < 100 && p.blue() < 100)
        .count();
    assert!(dunkel > 500, "kaum Tinte im Bild: {dunkel} Pixel");
}

/// Die Anzeigeliste trägt Rundungen als **Kurve**, nicht als Polygonzug — die
/// Zerlegung passiert erst im jeweiligen Renderer (Bildschirm mit der
/// Einstellung, Druck mit `CurveQuality::PRINT`). Wer hier tesselliert, nimmt
/// dem Druckpfad die eigene Güte.
#[test]
fn anzeigeliste_haelt_rundungen_als_kurve() {
    let doc = zeichnung();
    let dl = render::render_document(&RenderCtx::new(&doc));
    let bogen = dl
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Arc { .. }))
        .count();
    assert_eq!(bogen, 3, "die Anzeigeliste zerlegt die Bögen schon");
    assert!(
        CurveQuality::PRINT.max_sagitta < CurveQuality::SCREEN.max_sagitta,
        "der Druck muss feiner rechnen als die Anzeige"
    );
}
