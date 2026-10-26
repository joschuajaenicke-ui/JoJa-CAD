//! Füllfläche: ein Objekttyp, ein Verhalten.
//!
//! Prüft die Abnahme der Karte F1: alte Schraffurdateien laden als
//! Füllfläche, Muster bleibt über Speichern/Laden erhalten, und der
//! PNG-Export zeichnet dasselbe Muster wie die Displayliste.

use jjcad_core::export::RasterOptions;
use jjcad_core::persist;
use jjcad_core::model::*;
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::render::{self, DisplayList, Prim, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::{Color, FillPattern};
use serde_json::json;

/// Alte Zeichnung im Stand vor der Vereinheitlichung: `type: "hatch"` mit
/// `angle`, `spacing` und `filled`.
const ALTE_SCHRAFFUR: &str = r##"{
  "format": "jjcad",
  "version": 2,
  "name": "Alt",
  "entities": [
    {
      "id": 1,
      "kind": {
        "type": "hatch",
        "boundary": [[0, 0], [2000, 0], [2000, 1000], [0, 1000]],
        "pattern": "Cross",
        "angle": 0.7853981633974483,
        "spacing": 250.0,
        "filled": false
      },
      "layer": 1,
      "style": { "color": "#202020" }
    },
    {
      "id": 2,
      "kind": {
        "type": "hatch",
        "boundary": [[3000, 0], [5000, 0], [5000, 1000], [3000, 1000]],
        "pattern": "Solid",
        "angle": 0.0,
        "spacing": 100.0,
        "filled": true
      },
      "layer": 1,
      "style": { "color": "#204020" }
    }
  ],
  "layers": [
    { "id": 1, "name": "Waende", "color": "#000000" }
  ],
  "current_layer": 1,
  "next_id": 3
}"##;

fn ente_als_fill(doc: &Document) -> Vec<(FillPattern, f64, f64, bool)> {
    doc.entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Fill {
                pattern,
                pattern_angle,
                pattern_scale,
                border,
                ..
            } => Some((*pattern, *pattern_angle, *pattern_scale, *border)),
            _ => None,
        })
        .collect()
}

#[test]
fn alte_schraffur_laedt_als_fuellflaeche() {
    let doc = persist::from_json(ALTE_SCHRAFFUR).expect("alte Datei muss laden");
    assert_eq!(doc.entities.len(), 2, "beide Objekte übernehmen");
    let fills = ente_als_fill(&doc);
    assert_eq!(fills.len(), 2, "jede Schraffur wird zur Füllfläche");

    // Muster, Winkel und Abstand bleiben erhalten.
    let (pat, winkel, abstand, rand) = fills[0];
    assert_eq!(pat, FillPattern::Cross);
    assert!((winkel - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
    assert!((abstand - 250.0).abs() < 1e-9);
    assert!(!rand, "alte Schraffur hatte keine Randlinie");

    // `filled: true` wurde zu einer gesetzten Flächenfarbe (aufgehellte
    // Linienfarbe), damit die Zeichnung gleich aussieht.
    let zweite = &doc.entities[1];
    let flaeche = zweite
        .style
        .fill
        .expect("filled=true muss als Füllfarbe ankommen");
    let linie = zweite.style.color.unwrap_or(Color::BLACK);
    assert_eq!(flaeche, linie.mix(Color::WHITE, 0.72));
}

#[test]
fn muster_bleibt_ueber_speichern_und_laden() {
    let mut doc = persist::from_json(ALTE_SCHRAFFUR).expect("laden");
    let datei = std::env::temp_dir().join("jjcad_fuellflaeche_test.jjcad");
    persist::save_document(&mut doc, &datei).expect("speichern");
    let geschrieben = std::fs::read_to_string(&datei).expect("Datei lesen");
    let wieder = persist::load_document(&datei).expect("erneut laden");
    let _ = std::fs::remove_file(&datei);

    assert_eq!(ente_als_fill(&wieder), ente_als_fill(&doc));

    // In der Datei steht der neue Typ — kein `hatch` mehr.
    assert!(geschrieben.contains("\"type\": \"fill\""));
    assert!(!geschrieben.contains("\"type\": \"hatch\""));
}

#[test]
fn modify_set_aendert_muster_winkel_abstand_und_rand() {
    let mut s = Session::new();
    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [2, 0], [2, 1], [0, 1]], "pattern": "dots"}),
    )
    .expect("Füllfläche zeichnen");
    let id = s.doc.entities[0].id;

    s.execute(
        "modify.set",
        &json!({"ids": [id], "pattern": "kreuz", "pattern_angle": 30.0, "pattern_scale": 0.25, "border": true}),
    )
    .expect("ändern");

    let (pat, winkel, abstand, rand) = ente_als_fill(&s.doc)[0];
    assert_eq!(pat, FillPattern::Cross);
    assert!((winkel - 30f64.to_radians()).abs() < 1e-9);
    assert!((abstand - 250.0).abs() < 1e-9, "0,25 m = 250 mm: {abstand}");
    assert!(rand);
}

/// GUI und PNG lesen dieselbe Displayliste; das Muster muss in den Primitiven
/// ankommen, sonst wäre der Export leer.
#[test]
fn render_erzeugt_musterprimitive_und_randlinie() {
    let mut s = Session::new();
    // Umrandung ausdrücklich an: die Vorgabe von „Kontur anklicken" ist aus
    // (siehe tests/fuellflaeche_umrandung.rs).
    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [2, 0], [2, 1], [0, 1]], "pattern": "lines", "scale": 0.5, "border": true}),
    )
    .unwrap();
    let id = s.doc.entities[0].id;
    let e = s.doc.entity(id).unwrap().clone();

    let mut dl = DisplayList::default();
    let mut st = render::PrimStyle::default();
    st.color = Color::BLACK;
    st.width = 0.35;
    render::render_entity(&RenderCtx::new(&s.doc), &e, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
    let linien = dl
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Line(..)))
        .count();
    let raender = dl
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Poly(_, true)))
        .count();
    assert!(linien > 3, "Schraffurlinien fehlen: {linien}");
    assert_eq!(raender, 1, "eine Randlinie");

    // Ohne Randlinie bleibt sie weg.
    s.execute("modify.set", &json!({"ids": [id], "border": false}))
        .unwrap();
    let e = s.doc.entity(id).unwrap().clone();
    let mut dl2 = DisplayList::default();
    render::render_entity(&RenderCtx::new(&s.doc), &e, jjcad_core::geom::Affine::IDENTITY, &mut dl2, 0);
    assert_eq!(
        dl2.prims
            .iter()
            .filter(|p: &&Prim| matches!(p.shape, Shape::Poly(_, true)))
            .count(),
        0
    );
}

/// Das PNG (Exportweg) muss das Muster zeigen — nicht nur die Fläche.
#[test]
fn png_export_zeichnet_das_muster() {
    let mut s = Session::new();
    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [2, 0], [2, 1], [0, 1]], "pattern": "lines", "scale": 0.2}),
    )
    .unwrap();
    let opts = RasterOptions {
        width_px: 400,
        height_px: 200,
        paper: false,
        ..Default::default()
    };
    let mit_muster = render_model_pixmap(&s.doc, &opts).expect("PNG rendern");

    // Dieselbe Fläche als Vollfläche: das Muster muss das Bild verändern.
    s.execute("modify.set", &json!({"ids": [s.doc.entities[0].id], "pattern": "solid"}))
        .unwrap();
    let voll = render_model_pixmap(&s.doc, &opts).expect("PNG rendern");

    let unterschied = mit_muster
        .pixels()
        .iter()
        .zip(voll.pixels().iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        unterschied > 200,
        "PNG-Export zeigt kein Muster (nur {unterschied} abweichende Pixel)"
    );
}

/// Musterlinien dürfen nicht über die Kontur hinauslaufen.
#[test]
fn muster_bleibt_innerhalb_der_kontur() {
    let mut s = Session::new();
    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [2, 0], [2, 1], [0, 1]], "pattern": "cross", "scale": 0.2, "border": false}),
    )
    .unwrap();
    let e = s.doc.entities[0].clone();
    let mut dl = DisplayList::default();
    render::render_entity(&RenderCtx::new(&s.doc), &e, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
    for p in &dl.prims {
        if let Shape::Line(a, b) = &p.shape {
            for q in [a, b] {
                assert!(
                    q.x >= -1e-6 && q.x <= 2000.0 + 1e-6 && q.y >= -1e-6 && q.y <= 1000.0 + 1e-6,
                    "Musterlinie läuft aus der Kontur: {q:?}"
                );
            }
        }
    }
}

/// Die mitgelieferten Beispieldateien müssen weiter öffnen.
#[test]
fn beispieldateien_oeffnen() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let mut geprueft = 0;
    for eintrag in std::fs::read_dir(&dir).expect("samples/ fehlt") {
        let pfad = eintrag.unwrap().path();
        if pfad.extension().and_then(|e| e.to_str()) != Some("jjcad") {
            continue;
        }
        let doc = persist::load_document(&pfad)
            .unwrap_or_else(|e| panic!("{}: {e}", pfad.display()));
        assert!(!doc.entities.is_empty(), "{}: keine Objekte", pfad.display());
        geprueft += 1;
    }
    assert!(geprueft >= 1, "keine Beispieldatei gefunden");
}

/// Alte Schraffur und neue Füllfläche mit gleichen Werten zeichnen identisch.
#[test]
fn alte_schraffur_und_neue_fuellflaeche_zeichnen_gleich() {
    let alt = persist::from_json(ALTE_SCHRAFFUR).unwrap();
    let mut neu = Session::new();
    neu.execute(
        "draw.fill",
        &json!({
            "boundary": [[0, 0], [2, 0], [2, 1.0], [0, 1.0]],
            "pattern": "kreuz",
            "angle": 45.0,
            "scale": 0.25,
            "border": false,
            "color": "#202020",
        }),
    )
    .unwrap();

    let mut a = DisplayList::default();
    let e = alt.entities[0].clone();
    render::render_entity(&RenderCtx::new(&alt), &e, jjcad_core::geom::Affine::IDENTITY, &mut a, 0);
    let mut b = DisplayList::default();
    let e2 = neu.doc.entities[0].clone();
    render::render_entity(&RenderCtx::new(&neu.doc), &e2, jjcad_core::geom::Affine::IDENTITY, &mut b, 0);

    let form = |dl: &DisplayList| -> Vec<String> {
        let mut v: Vec<String> = dl
            .prims
            .iter()
            .map(|p| format!("{:?}", p.shape))
            .collect();
        v.sort();
        v
    };
    assert_eq!(form(&a), form(&b));
}
