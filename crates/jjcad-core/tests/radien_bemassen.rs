//! T0252 — Radiusbemaßung.
//!
//! Ein neues Bemaßungswerkzeug für Radien: Kreis oder Bogen anklicken, der
//! Klick bestimmt die Richtung der Maßlinie. Geprüft wird über die
//! Action-Schicht und die Anzeige — der Weg, den Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::Vec2;
use jjcad_core::model::{Document, EntityKind};
use jjcad_core::render::{self, render_document, RenderCtx, Shape};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut d = Document::new();
    d.settings.input_unit = Unit::Mm;
    d
}

fn kreis(d: &mut Document, c: Vec2, r: f64) -> u64 {
    let antwort = actions::execute(
        d,
        "draw.circle",
        &json!({"c": [c.x, c.y], "r": r}),
    )
    .expect("Kreis zeichnen");
    antwort["created"][0].as_u64().expect("Kennung")
}

fn bogen(d: &mut Document, c: Vec2, r: f64, a0: f64, a1: f64) -> u64 {
    let antwort = actions::execute(
        d,
        "draw.arc",
        &json!({"c": [c.x, c.y], "r": r, "a0": a0, "a1": a1}),
    )
    .expect("Bogen zeichnen");
    antwort["created"][0].as_u64().expect("Kennung")
}

fn radius_dim(d: &mut Document, id: u64, p: Vec2) -> u64 {
    let antwort = actions::execute(
        d,
        "dim.radius",
        &json!({"ids": [id], "p": [p.x, p.y]}),
    )
    .expect("Radius bemaßen");
    antwort["created"][0].as_u64().expect("Kennung")
}

fn teile(d: &Document, id: u64) -> (Vec2, f64, f64) {
    match &d.entity(id).expect("Bemaßung").kind {
        EntityKind::DimRadius { c, r, angle, .. } => (*c, *r, *angle),
        k => panic!("keine Radiusbemaßung: {k:?}"),
    }
}

/// Der Radius kommt aus dem Kreis, die Richtung aus dem Klickpunkt.
#[test]
fn radius_kommt_aus_dem_kreis_und_die_richtung_aus_dem_klick() {
    let mut d = doc_mm();
    let id = kreis(&mut d, Vec2::new(1000.0, 2000.0), 750.0);
    let dim = radius_dim(&mut d, id, Vec2::new(1000.0, 2750.0));
    let (c, r, angle) = teile(&d, dim);
    assert!((c.x - 1000.0).abs() < 1e-9 && (c.y - 2000.0).abs() < 1e-9, "{c:?}");
    assert!((r - 750.0).abs() < 1e-9, "Radius {r}");
    // Klick senkrecht über dem Mittelpunkt → Maßlinie nach oben (π/2).
    assert!(
        (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "Winkel {angle}"
    );
    // Die Bemaßung liegt auf der Standardebene „Beschriftung“.
    let e = d.entity(dim).expect("Bemaßung");
    assert_eq!(
        d.layer(e.layer).map(|l| l.name.as_str()),
        Some("Beschriftung")
    );
}

/// Ein Bogen lässt sich genauso bemaßen — und eine Linie nicht (kein stiller
/// Erfolg, T0038).
#[test]
fn bogen_ja_linie_nein() {
    let mut d = doc_mm();
    let b = bogen(&mut d, Vec2::ZERO, 400.0, 0.0, 90.0);
    let dim = radius_dim(&mut d, b, Vec2::new(400.0, 0.0));
    assert!((teile(&d, dim).1 - 400.0).abs() < 1e-9);

    let linie = actions::execute(
        &mut d,
        "draw.line",
        &json!({"a": [0.0, 0.0], "b": [1000.0, 0.0]}),
    )
    .expect("Linie")
    ["created"][0]
        .as_u64()
        .expect("Kennung");
    let fehler = actions::execute(
        &mut d,
        "dim.radius",
        &json!({"ids": [linie], "p": [500.0, 0.0]}),
    );
    assert!(fehler.is_err(), "Linie darf kein Radiusmaß bekommen");
}

/// Die Maßlinie läuft vom Mittelpunkt zum Bogenpunkt; die Maßzahl „R …“
/// steht außerhalb.
#[test]
fn anzeige_zeigt_masslinie_und_masszahl() {
    let mut d = doc_mm();
    let id = kreis(&mut d, Vec2::ZERO, 500.0);
    let dim = radius_dim(&mut d, id, Vec2::new(0.0, 500.0));
    let dl = render_document(&RenderCtx::new(&d));

    // Eine Linie endet genau auf dem Bogenpunkt (0; 500).
    let bogenpunkt = render::dim_radius_punkt(Vec2::ZERO, 500.0, std::f64::consts::FRAC_PI_2);
    assert!((bogenpunkt.y - 500.0).abs() < 1e-9);
    let trifft = dl.prims.iter().any(|p| match &p.shape {
        Shape::Line(a, b) => {
            (a.dist(Vec2::ZERO) < 1e-6 && b.dist(bogenpunkt) < 1e-6)
                || (b.dist(Vec2::ZERO) < 1e-6 && a.dist(bogenpunkt) < 1e-6)
        }
        _ => false,
    });
    assert!(trifft, "Maßlinie vom Mittelpunkt zum Bogenpunkt fehlt");

    let texte: Vec<&str> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) if p.entity == dim => Some(t.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        texte.iter().any(|t| t.starts_with("R ")),
        "Maßzahl fehlt: {texte:?}"
    );
}

/// Die Vorschau ist das fertige Objekt — dieselbe Anzeige wie nach dem
/// Einfügen.
#[test]
fn vorschau_ist_das_fertige_objekt() {
    let mut d = doc_mm();
    let layer = d.current_layer;
    let dl = render::preview_dim_radius(&d, Vec2::ZERO, 500.0, 0.0, layer, None);
    let id = kreis(&mut d, Vec2::ZERO, 500.0);
    let dim = radius_dim(&mut d, id, Vec2::new(500.0, 0.0));
    let echt = render::trefferbild(&d, d.entity(dim).expect("Bemaßung"));

    // Gleiche Streckenzahl und gleiche Textzahl wie das eingefügte Objekt.
    let vorschau_segmente = dl
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Line(..)))
        .count();
    let vorschau_texte = dl
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Text(..)))
        .count();
    let echt_segmente = echt
        .segmente
        .iter()
        .filter(|s| matches!(s, jjcad_core::geom::Seg::Line(..)))
        .count();
    assert_eq!(vorschau_segmente, echt_segmente);
    assert_eq!(vorschau_texte, echt.text_kaesten.len());
}

/// Drehen nimmt Mittelpunkt und Richtung mit.
#[test]
fn drehen_dreht_die_richtung_mit() {
    let mut d = doc_mm();
    let id = kreis(&mut d, Vec2::ZERO, 500.0);
    let dim = radius_dim(&mut d, id, Vec2::new(500.0, 0.0));
    actions::execute(
        &mut d,
        "modify.rotate",
        &json!({"ids": [dim], "center": [0.0, 0.0], "angle": 90.0}),
    )
    .expect("drehen");
    let (_, _, angle) = teile(&d, dim);
    assert!(
        (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
        "Winkel nach dem Drehen: {angle}"
    );
}

/// Maßtext und Richtung lassen sich nachträglich ändern (Eigenschaften-Panel
/// schickt `modify.set`).
#[test]
fn masstext_und_richtung_sind_aenderbar() {
    let mut d = doc_mm();
    let id = kreis(&mut d, Vec2::ZERO, 500.0);
    let dim = radius_dim(&mut d, id, Vec2::new(500.0, 0.0));
    actions::execute(
        &mut d,
        "modify.set",
        &json!({"ids": [dim], "text": "R 50", "angle": 45.0}),
    )
    .expect("ändern");
    match &d.entity(dim).expect("Bemaßung").kind {
        EntityKind::DimRadius {
            angle,
            override_text,
            ..
        } => {
            assert_eq!(override_text.as_deref(), Some("R 50"));
            assert!((angle.to_degrees() - 45.0).abs() < 1e-6);
        }
        k => panic!("kein Radiusmaß: {k:?}"),
    }
}

/// Die Bemaßung ist anklickbar: die Trefferfläche kommt aus der Anzeige
/// (Maßlinie), nicht aus einem unsichtbaren Rechteck.
#[test]
fn masslinie_ist_anklickbar() {
    let mut d = doc_mm();
    let id = kreis(&mut d, Vec2::ZERO, 1000.0);
    let dim = radius_dim(&mut d, id, Vec2::new(1000.0, 0.0));
    // Mitte der Maßlinie.
    let treffer = jjcad_core::snap::hit_test(&d, Vec2::new(500.0, 0.0), 5.0, None);
    assert_eq!(treffer, Some(dim), "Maßlinie nicht getroffen");
}

/// Die Datei trägt das Objekt unverändert — gespeichert wird `dim_radius`.
#[test]
fn wird_gespeichert_und_geladen() {
    let mut d = doc_mm();
    let id = kreis(&mut d, Vec2::new(100.0, 200.0), 300.0);
    let dim = radius_dim(&mut d, id, Vec2::new(100.0, 500.0));
    let pfad = std::env::temp_dir().join("jjcad_t0252_radius.jjcad");
    jjcad_core::persist::save_document(&mut d, &pfad).expect("speichern");
    let text = std::fs::read_to_string(&pfad).expect("lesen");
    assert!(text.contains("dim_radius"), "Schlüssel fehlt: {text}");
    let geladen = jjcad_core::persist::load_document(&pfad).expect("laden");
    let _ = std::fs::remove_file(&pfad);
    let (c, r, _) = teile(&geladen, dim);
    assert!((c.x - 100.0).abs() < 1e-6 && (c.y - 200.0).abs() < 1e-6, "{c:?}");
    assert!((r - 300.0).abs() < 1e-6);
}
