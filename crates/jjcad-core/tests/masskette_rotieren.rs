//! T0166: Eine Maßkette hat Lage **und** Orientierung.
//!
//! Waagerecht, senkrecht und parallel sind nur drei Winkel desselben Objekts.
//! Beim Drehen muss die **Darstellung** mitgehen (Maßlinie, Hilfslinien,
//! Maßtext) — vorher wanderten nur die Messpunkte, die Maßlinie blieb
//! waagerecht stehen.

use jjcad_core::model::EntityKind;
use jjcad_core::render::{dim_achse, dim_line_span, render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::transform;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

fn mass_texte(s: &Session) -> Vec<String> {
    render_document(&RenderCtx::new(&s.doc))
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

fn letzte_kette(s: &Session) -> (u64, f64) {
    let e = s
        .doc
        .entities
        .iter()
        .rev()
        .find(|e| matches!(e.kind, EntityKind::Dim { .. }))
        .expect("keine Maßkette");
    match &e.kind {
        EntityKind::Dim { angle, .. } => (e.id, *angle),
        _ => unreachable!(),
    }
}

/// Waagerechte Kette um 90° drehen: der Winkel wandert mit, die Maßlinie steht
/// danach senkrecht — und der Maßtext bleibt derselbe.
#[test]
fn drehen_kippt_die_masslinie_mit() {
    let mut s = s();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0.0, 0.0], [2.0, 0.0]], "line_pos": [1.0, -0.4], "axis": "x"}),
    )
    .expect("Kette");
    let (id, winkel) = letzte_kette(&s);
    assert!(winkel.abs() < 1e-9, "waagerecht erwartet: {winkel}");

    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0.0, 0.0], "angle": 90.0}),
    )
    .expect("drehen");

    let e = s.doc.entity(id).expect("Kette");
    let EntityKind::Dim {
        pts,
        line_pos,
        angle,
        ..
    } = &e.kind
    else {
        panic!("keine Maßkette mehr");
    };
    assert!(
        (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "Orientierung nicht gedreht: {angle}"
    );
    // Messpunkte gedreht: (2,0) → (0,2).
    assert!(pts[1].dist(jjcad_core::geom::v(0.0, 2000.0)) < 1e-6, "{pts:?}");
    // Maßlinie gedreht: (1000,−400) → (400,1000).
    assert!(
        line_pos.dist(jjcad_core::geom::v(400.0, 1000.0)) < 1e-6,
        "{line_pos:?}"
    );
    // Die gezeichnete Maßlinie steht senkrecht — **das** war der Fehler:
    // vorher blieb sie waagerecht.
    let (a, b) = dim_line_span(&e.kind).expect("Maßlinie");
    assert!((b.x - a.x).abs() < 1e-6, "Maßlinie steht nicht senkrecht: {a:?} {b:?}");
    assert!((b.y - a.y).abs() > 1000.0, "{a:?} {b:?}");

    // Maßzahl unverändert 2,00 m — Drehen ändert das Maß nicht.
    assert!(
        mass_texte(&s).iter().any(|t| t == "2,00"),
        "{:?}",
        mass_texte(&s)
    );
}

/// Senkrechte Kette um 45° drehen: die Darstellung folgt dem Winkel, nicht der
/// alten Achse. Gemessen wird weiter längs der Maßlinie.
#[test]
fn schraeg_gedrehte_kette_misst_laengs_der_masslinie() {
    let mut s = s();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0.0, 0.0], [0.0, 2.0]], "line_pos": [-0.4, 0.0], "axis": "y"}),
    )
    .expect("Kette");
    let (id, winkel) = letzte_kette(&s);
    assert!(
        (winkel - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "senkrecht erwartet: {winkel}"
    );

    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0.0, 0.0], "angle": 45.0}),
    )
    .expect("drehen");

    let e = s.doc.entity(id).expect("Kette");
    let (a, b) = dim_line_span(&e.kind).expect("Maßlinie");
    let richtung = (b - a).angle();
    assert!(
        (richtung - 135f64.to_radians()).abs() < 1e-6,
        "Maßlinie läuft unter {richtung} rad, erwartet 135°"
    );
    // Länge der Maßlinie bleibt 2,00 m.
    assert!((a.dist(b) - 2000.0).abs() < 1e-6, "{a:?} {b:?}");
}

/// `modify.set` mit `angle` setzt die Orientierung frei (Eigenschaften-Panel).
#[test]
fn winkel_laesst_sich_im_panel_setzen() {
    let mut s = s();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0.0, 0.0], [2.0, 0.0]], "line_pos": [1.0, -0.4], "axis": "x"}),
    )
    .expect("Kette");
    let (id, _) = letzte_kette(&s);
    s.execute("modify.set", &json!({"ids": [id], "angle": 30.0}))
        .expect("Winkel setzen");
    let e = s.doc.entity(id).expect("Kette");
    match &e.kind {
        EntityKind::Dim { angle, .. } => assert!(
            (angle - 30f64.to_radians()).abs() < 1e-9,
            "Winkel nicht gesetzt: {angle}"
        ),
        _ => panic!("keine Maßkette"),
    }
    // Die Achsangabe `axis` bleibt als bequemer Weg gültig und setzt 0°.
    s.execute("modify.set", &json!({"ids": [id], "axis": "x"}))
        .expect("Achse setzen");
    let e = s.doc.entity(id).expect("Kette");
    match &e.kind {
        EntityKind::Dim { angle, .. } => {
            assert!(angle.abs() < 1e-9, "waagerecht: {angle}")
        }
        _ => panic!("keine Maßkette"),
    }
}

/// Die Orientierung überlebt Speichern und Laden, und alte Dateien mit
/// `axis: "y"` werden als 90° gelesen (T0166).
#[test]
fn orientierung_ueberlebt_die_datei() {
    let mut s = s();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0.0, 0.0], [2.0, 0.0]], "line_pos": [1.0, -0.4], "axis": "x"}),
    )
    .expect("Kette");
    let (id, _) = letzte_kette(&s);
    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0.0, 0.0], "angle": 30.0}),
    )
    .expect("drehen");

    let pfad = std::env::temp_dir().join("jjcad-t0166-winkel.jjcad");
    jjcad_core::persist::save_document(&mut s.doc, &pfad).expect("speichern");
    let text = std::fs::read_to_string(&pfad).expect("Datei");
    assert!(text.contains("\"angle\""), "kein Feld `angle` in der Datei");
    assert!(!text.contains("\"axis\""), "altes Feld `axis` noch in der Datei");
    let geladen = jjcad_core::persist::load_document(&pfad).expect("laden");
    let e = geladen.entities.iter().find(|e| e.id == id).expect("Kette");
    match &e.kind {
        EntityKind::Dim { angle, .. } => assert!(
            (angle - 30f64.to_radians()).abs() < 1e-9,
            "Winkel nach dem Laden: {angle}"
        ),
        _ => panic!("keine Maßkette"),
    }
    let _ = std::fs::remove_file(&pfad);

    // Alte Dateien tragen den **Serde-Namen** der Achse (`Y`, `Aligned`) —
    // beide Schreibweisen müssen senkrecht bzw. schräg ankommen.
    for (achse, erwartet) in [
        ("Y", std::f64::consts::FRAC_PI_2),
        ("y", std::f64::consts::FRAC_PI_2),
        ("Aligned", std::f64::consts::FRAC_PI_4),
        ("aligned", std::f64::consts::FRAC_PI_4),
    ] {
        let mut alt: serde_json::Value = serde_json::json!({
            "format": "jjcad",
            "version": 1,
            "entities": [{
                "id": 1,
                "layer": 1,
                "kind": {"type": "dim", "pts": [[0.0, 0.0], [2000.0, 2000.0]],
                         "line_pos": [-400.0, 0.0], "axis": achse},
                "style": {}
            }],
            "layers": [{"id": 1, "name": "Bemaßung", "color": "#000000"}],
            "current_layer": 1
        });
        jjcad_core::persist::migrate_legacy_dims(&mut alt);
        let angle = alt["entities"][0]["kind"]["angle"]
            .as_f64()
            .expect("kein Winkel nach der Migration");
        assert!(
            (angle - erwartet).abs() < 1e-9,
            "alte Achse {achse} wurde {angle} statt {erwartet}"
        );
        assert!(
            alt["entities"][0]["kind"].get("axis").is_none(),
            "altes Feld `axis` blieb stehen"
        );
    }
}

/// Spiegeln kehrt die Richtung um: aus 0° wird an einer senkrechten Achse 180°.
#[test]
fn spiegeln_kehrt_die_orientierung() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s.execute(
        "dim.chain",
        &json!({"pts": [[1.0, 0.0], [3.0, 0.0]], "line_pos": [2.0, -0.4], "axis": "x"}),
    )
    .expect("Kette");
    let (id, _) = letzte_kette(&s);
    let m = transform::mirror_at(
        jjcad_core::geom::v(0.0, 0.0),
        jjcad_core::geom::v(0.0, 1000.0),
    );
    let e = s.doc.entity(id).expect("Kette").clone();
    let ne = transform::transform_entity(&e, &m);
    match &ne.kind {
        EntityKind::Dim { angle, .. } => {
            let a = angle.rem_euclid(std::f64::consts::TAU).to_degrees();
            assert!((a - 180.0).abs() < 1e-6, "gespiegelt: {a}°");
        }
        _ => panic!("keine Maßkette"),
    }
    // Gegenprobe der Achsenrechnung: 0° ergibt waagerecht, 90° senkrecht.
    assert_eq!(dim_achse(0.0).0, jjcad_core::geom::v(1.0, 0.0));
    assert!(dim_achse(std::f64::consts::FRAC_PI_2).0.dist(jjcad_core::geom::v(0.0, 1.0)) < 1e-9);
}
