//! Maßstab je Plan (Ticket T0232).
//!
//! Vorher trug die **Datei** einen einzigen Maßstab (`settings.annotation_scale`).
//! Seit T0232 gehört er zu jedem Plan im Layer-Manager: Papiergrößen
//! (Bemaßung, Koten, Punktsymbole, Blöcke mit Papiergröße) rechnen mit dem
//! Maßstab des Plans, in dem das Objekt liegt, und die gezeichnete Strichstärke
//! im Modus „Papier (maßstabsgetreu)“ ebenso.
//!
//! Geprüft wird über die Befehls-Schicht und die Anzeigeliste — dieselben Wege
//! wie GUI, Druck und PNG.

use jjcad_core::actions;
use jjcad_core::export::RasterOptions;
use jjcad_core::model::Document;
use jjcad_core::render::{self, RenderCtx, Shape};
use serde_json::json;

/// Zweiter Plan mit eigenem Maßstab; gezeichnet wird danach in seinem
/// „Beschriftung“-Layer.
fn plan_mit_massstab(doc: &mut Document, name: &str, scale: f64) -> u64 {
    let r = actions::execute(doc, "plan.create", &json!({"name": name, "scale": scale}))
        .unwrap_or_else(|e| panic!("plan.create: {e}"));
    let pid = r["id"].as_u64().expect("Plan-Id");
    doc.current_layer = doc
        .layers_of_plan(pid)
        .iter()
        .find(|l| l.name == "Beschriftung")
        .map(|l| l.id)
        .expect("Beschriftung im Plan");
    pid
}

/// Höhe des Maßtextes einer Maßkette in Welt-Millimetern.
fn masstext_hoehe(doc: &Document) -> f64 {
    let dl = render::render_document(&RenderCtx::new(doc));
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) if !t.text.trim().is_empty() && t.text.contains(',') => Some(t.height),
            _ => None,
        })
        .next()
        .expect("kein Maßtext gezeichnet")
}

#[test]
fn neuer_plan_bekommt_seinen_massstab() {
    let mut d = Document::new();
    assert_eq!(d.plan_scale(d.plans[0].id), 100.0, "Vorgabe 1:100");
    let pid = plan_mit_massstab(&mut d, "Lageplan", 500.0);
    assert_eq!(d.plan_scale(pid), 500.0);
    // Ohne Angabe bleibt es bei der Vorgabe.
    let r = actions::execute(&mut d, "plan.create", &json!({"name": "Schnitt"})).unwrap();
    let schnitt = r["id"].as_u64().unwrap();
    assert_eq!(d.plan_scale(schnitt), 100.0);
}

#[test]
fn plan_update_aendert_den_massstab() {
    let mut d = Document::new();
    let pid = plan_mit_massstab(&mut d, "Detail", 20.0);
    actions::execute(&mut d, "plan.update", &json!({"plan": pid, "scale": 25.0})).unwrap();
    assert_eq!(d.plan_scale(pid), 25.0);
    // Unter 1:1 geht es nicht.
    actions::execute(&mut d, "plan.update", &json!({"plan": pid, "scale": 0.0})).unwrap();
    assert_eq!(d.plan_scale(pid), 1.0, "Maßstab nie unter 1:1");
}

#[test]
fn masstext_folgt_dem_plan_seines_layers() {
    let mut d = Document::new();
    // Plan 1 (1:100): Maßkette auf „Beschriftung“.
    actions::execute(
        &mut d,
        "dim.chain",
        &json!({"pts": [[0, 0], [3000, 0]], "line_pos": [1500, -800], "axis": "x"}),
    )
    .unwrap();
    let h100 = masstext_hoehe(&d);

    let pid = plan_mit_massstab(&mut d, "Übersicht", 200.0);
    actions::execute(
        &mut d,
        "dim.chain",
        &json!({"pts": [[0, 5000], [3000, 5000]], "line_pos": [1500, 4200], "axis": "x"}),
    )
    .unwrap();
    // Der neue Maßtext liegt auf einem Layer des zweiten Plans und rechnet mit
    // dessen Maßstab: doppelte Papierhöhe, weil 1:200 doppelt so viel Welt je
    // Papier-Millimeter hat.
    let dl = render::render_document(&RenderCtx::new(&d));
    let hoehen: Vec<f64> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) if t.text.contains(',') => Some(t.height),
            _ => None,
        })
        .collect();
    assert_eq!(hoehen.len(), 2, "zwei Maßtexte erwartet: {hoehen:?}");
    let h200 = hoehen
        .iter()
        .copied()
        .filter(|h| (*h - h100).abs() > 0.01)
        .next()
        .expect("beide Maßtexte gleich hoch — der Planmaßstab wirkt nicht");
    assert!(
        (h200 - 2.0 * h100).abs() < 0.01,
        "1:200 muss den Maßtext doppelt so groß rechnen: {h100} gegen {h200}"
    );
    // Und die Layer-Zuordnung ist die Ursache: der zweite Layer gehört zum
    // zweiten Plan.
    let layer2 = d.layers_of_plan(pid)[0].id;
    assert_eq!(d.plan_scale_of_layer(layer2), 200.0);
}

#[test]
fn strichfaktor_1zu100_breiter_als_1zu200() {
    // Auftrag T0232: Eine Linie im Plan 1:100 erscheint im Modus „Papier
    // (maßstabsgetreu)“ breiter als dieselbe Stärke im Plan 1:200.
    let f100 = jjcad_core::model::strich_faktor(100.0);
    let f200 = jjcad_core::model::strich_faktor(200.0);
    let f50 = jjcad_core::model::strich_faktor(50.0);
    assert!(f100 > f200, "1:100 ({f100}) muss breiter sein als 1:200 ({f200})");
    assert!((f100 / f200 - 2.0).abs() < 1e-9, "1:200 halb so breit wie 1:100");
    assert!(f50 > f100, "1:50 noch breiter als 1:100");
    // Bezug 1:100 = unverändertes Verhalten von vor T0232.
    assert!((f100 - 100.0).abs() < 1e-9, "Bezug 1:100 muss 100 sein: {f100}");

    // Dieselbe Rechnung über den Layer: der Plan entscheidet.
    let mut d = Document::new();
    let pid = plan_mit_massstab(&mut d, "Grober Plan", 200.0);
    let layer2 = d.layers_of_plan(pid)[0].id;
    let layer1 = d.layers_of_plan(d.plans[0].id)[0].id;
    let zoom = 0.04;
    assert!(
        d.strich_px(layer1, zoom) > d.strich_px(layer2, zoom),
        "Layer im Plan 1:100 muss die breitere Strichstärke liefern"
    );
    assert!(
        (d.strich_px(layer1, zoom) / d.strich_px(layer2, zoom) - 2.0).abs() < 1e-9,
        "Verhältnis 1:100 zu 1:200 muss 2 sein"
    );
}

#[test]
fn anzeigeliste_traegt_den_massstab_je_layer() {
    let mut d = Document::new();
    let pid = plan_mit_massstab(&mut d, "Lageplan", 500.0);
    let layer2 = d.layers_of_plan(pid)[0].id;
    let layer1 = d.layers_of_plan(d.plans[0].id)[0].id;
    actions::execute(&mut d, "draw.line", &json!({"a": [0, 0], "b": [1000, 0]})).unwrap();
    let dl = render::render_document(&RenderCtx::new(&d));
    assert_eq!(dl.skala(layer1), 100.0);
    assert_eq!(dl.skala(layer2), 500.0);
}

#[test]
fn datei_behaelt_die_massstaebe_der_plaene() {
    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("massstab_plaene.jjcad");

    let mut d = Document::new();
    plan_mit_massstab(&mut d, "Lageplan", 500.0);
    plan_mit_massstab(&mut d, "Detail", 20.0);
    jjcad_core::persist::save_document(&mut d, &path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("annotation_scale"),
        "der Datei-Maßstab darf nicht mehr geschrieben werden"
    );
    assert!(text.contains("\"scale\""), "Plan-Maßstab fehlt in der Datei");

    let geladen = jjcad_core::persist::load_document(&path).unwrap();
    let skalen: Vec<f64> = geladen.plans.iter().map(|p| p.scale).collect();
    assert_eq!(skalen, vec![100.0, 500.0, 20.0], "Maßstäbe je Plan");
    assert!(geladen.settings.annotation_scale.is_none());
}

#[test]
fn alte_datei_verteilt_den_datei_massstab_auf_die_plaene() {
    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("massstab_alt.jjcad");

    // Eine Datei aus der Zeit vor T0232: **ein** Maßstab in den Einstellungen,
    // die Pläne tragen keinen eigenen.
    let mut alt = Document::new();
    plan_mit_massstab(&mut alt, "Zweiter Plan", 100.0);
    for p in alt.plans.iter_mut() {
        p.scale = 100.0;
    }
    alt.settings.annotation_scale = Some(50.0);
    jjcad_core::persist::save_document(&mut alt, &path).unwrap();

    let geladen = jjcad_core::persist::load_document(&path).unwrap();
    assert!(
        geladen.settings.annotation_scale.is_none(),
        "der alte Datei-Maßstab wird beim Laden verbraucht"
    );
    for p in &geladen.plans {
        assert_eq!(
            p.scale, 50.0,
            "jeder Plan übernimmt den Maßstab der alten Datei ({})",
            p.name
        );
    }
}

/// Strichbreite im **PNG** je Plan: eine Wand derselben Papier-Stärke
/// (0,35 mm), je Dokument ein Plan — gemessen am oberen Rand der Wand.
#[test]
fn strichbreite_im_png_folgt_dem_plan() {
    // Die Abbildung des Rasterers ist für alle drei Bilder dieselbe (gleiche
    // Fläche, gleiche Größe) — nur der Maßstab des Plans unterscheidet sie.
    let opts = RasterOptions {
        width_px: 5200,
        height_px: 2600,
        bbox: Some([-100.0, -400.0, 5100.0, 2200.0]),
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let kantenbreite = |massstab: f64| -> usize {
        let mut d = Document::new();
        d.settings.input_unit = jjcad_core::units::Unit::M;
        let layer = d
            .layers_of_plan(d.plans[0].id)
            .iter()
            .find(|l| l.name == "Architektur")
            .map(|l| l.id)
            .unwrap();
        if let Some(l) = d.layer_mut(layer) {
            l.line_weight = 0.35;
        }
        if let Some(p) = d.plan_mut(d.plans[0].id) {
            p.scale = massstab;
        }
        actions::execute(
            &mut d,
            "draw.wall",
            &json!({"a": [0.0, 0.8], "b": [5.0, 0.8], "thickness": 0.24, "layer": layer}),
        )
        .unwrap();
        let pm = jjcad_core::raster::render_model_pixmap(&d, &opts).expect("PNG");
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let x = w / 2;
        let dunkel = |y: usize| {
            let p = pm.pixels()[y * w + x];
            0.299 * (p.red() as f64) + 0.587 * (p.green() as f64) + 0.114 * (p.blue() as f64)
                < 110.0
        };
        // Von oben: erster dunkler Lauf = obere Wandkante.
        let mut y = 0;
        while y < h && !dunkel(y) {
            y += 1;
        }
        let mut n = 0;
        while y < h && dunkel(y) {
            n += 1;
            y += 1;
        }
        assert!(n > 0, "keine Wand gezeichnet (1:{massstab})");
        n
    };
    let b100 = kantenbreite(100.0);
    let b200 = kantenbreite(200.0);
    let b50 = kantenbreite(50.0);
    println!("Strichbreite im PNG (1:100 / 1:200 / 1:50): {b100} / {b200} / {b50} px");
    assert!(
        b100 > b200,
        "1:100 ({b100} px) muss breiter sein als 1:200 ({b200} px)"
    );
    assert!(
        (b100 as f64 - 2.0 * b200 as f64).abs() <= 2.0,
        "1:100 muss doppelt so breit sein wie 1:200: {b100} gegen {b200} px"
    );
    assert!(
        (b50 as f64 - 2.0 * b100 as f64).abs() <= 3.0,
        "1:50 muss doppelt so breit sein wie 1:100: {b50} gegen {b100} px"
    );
}
