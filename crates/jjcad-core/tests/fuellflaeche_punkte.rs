//! Werkzeug „Füllfläche aus einer freien Punktfolge" (Action `draw.fill.points`).
//!
//! Prüft die Abnahme der Karte F3:
//! 1. vier Punkte + Enter ergeben eine Füllfläche mit vier Ecken und der
//!    richtigen Fläche,
//! 2. die Punktfolge erzeugt dasselbe Objekt wie das Umriss-Werkzeug
//!    (`draw.fill.boundary`) — nur die Eingabe unterscheidet sich,
//! 3. die Action ist derselbe Weg, den auch MCP aufruft (`Session::execute`),
//!    also liefert `jjcad_execute` dasselbe Ergebnis wie der Klickpfad.

use jjcad_core::geom::{polygon_area, Vec2};
use jjcad_core::model::*;
use jjcad_core::session::Session;
use jjcad_core::style::FillPattern;
use serde_json::json;

fn kanten(e: &Entity) -> Vec<Vec2> {
    match &e.kind {
        EntityKind::Fill { boundary, .. } => boundary.clone(),
        other => panic!("keine Füllfläche, sondern {}", other.type_name()),
    }
}

fn flaeche_m2(e: &Entity) -> f64 {
    polygon_area(&kanten(e)) / 1_000_000.0
}

/// Vier Punkte + Enter: genau ein Fill, vier Ecken, 12 m². Der Klickpfad der
/// GUI ruft dieselbe Action mit derselben Punktliste auf.
#[test]
fn vier_punkte_und_enter_ergeben_eine_fuellflaeche() {
    let mut s = Session::new();
    let pts = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];
    s.execute("draw.fill.points", &json!({ "pts": pts }))
        .expect("Punktfolge muss eine Füllfläche anlegen");

    assert_eq!(s.doc.entities.len(), 1, "genau ein Objekt");
    let k = kanten(&s.doc.entities[0]);
    assert_eq!(k.len(), 4, "vier Ecken, keine Zwischenpunkte");
    for (got, want) in k.iter().zip([
        Vec2::new(0.0, 0.0),
        Vec2::new(4000.0, 0.0),
        Vec2::new(4000.0, 3000.0),
        Vec2::new(0.0, 3000.0),
    ]) {
        assert!(
            (*got - want).len() < 1e-6,
            "Ecke {got:?} statt {want:?} (Meter → Millimeter)"
        );
    }
    assert!(
        (flaeche_m2(&s.doc.entities[0]) - 12.0).abs() < 1e-9,
        "Fläche {} m² statt 12 m²",
        flaeche_m2(&s.doc.entities[0])
    );
}

/// Dieselbe Kontur über beide Werkzeuge: Umriss (`draw.fill.boundary`) und
/// Punktfolge (`draw.fill.points`) müssen dasselbe Objekt erzeugen — dieselben
/// Ecken, dasselbe Muster, derselbe Maßstab, dieselbe Randlinie.
#[test]
fn punktfolge_und_umriss_erzeugen_dasselbe_objekt() {
    let boundary = json!([[0.0, 0.0], [2.5, 0.0], [2.5, 1.5], [0.7, 2.2]]);

    let mut a = Session::new();
    a.execute(
        "draw.fill.boundary",
        &json!({ "boundary": boundary, "pattern": "insulation", "scale": 0.3, "border": false }),
    )
    .expect("Umriss-Werkzeug");

    let mut b = Session::new();
    b.execute(
        "draw.fill.points",
        &json!({ "pts": boundary, "pattern": "insulation", "scale": 0.3, "border": false }),
    )
    .expect("Punktfolge-Werkzeug");

    assert_eq!(a.doc.entities.len(), 1);
    assert_eq!(b.doc.entities.len(), 1);
    let (fa, fb) = (&a.doc.entities[0], &b.doc.entities[0]);
    assert_eq!(fa.kind, fb.kind, "Geometrie und Muster müssen gleich sein");
    assert_eq!(fa.style, fb.style, "Stil muss gleich sein");
    assert!((flaeche_m2(fa) - flaeche_m2(fb)).abs() < 1e-9);

    // Undo ist derselbe Weg: ein Schritt nimmt das Objekt zurück.
    assert!(b.undo(), "Rückgängig muss greifen");
    assert!(b.doc.entities.is_empty(), "Fläche muss wieder weg sein");
}

/// Der alte Name `draw.fill` bleibt erreichbar und erzeugt dieselbe Fläche.
#[test]
fn alter_name_draw_fill_erzeugt_dieselbe_flaeche() {
    let boundary = json!([[0.0, 0.0], [2.0, 0.0], [2.0, 1.0]]);
    // Die Umrandung ausdrücklich gleich setzen: sie folgt dem Erstellmodus
    // (Punkte setzen an, Kontur anklicken aus) und ist deshalb allein kein
    // Unterschied der beiden Namen.
    let mut a = Session::new();
    a.execute("draw.fill", &json!({ "boundary": boundary, "border": true }))
        .expect("draw.fill");
    let mut b = Session::new();
    b.execute("draw.fill.points", &json!({ "pts": boundary, "border": true }))
        .expect("draw.fill.points");
    assert_eq!(a.doc.entities[0].kind, b.doc.entities[0].kind);
}

/// Zwei Punkte **auf einer Achse** sind keine Diagonale und werden abgelehnt —
/// die Meldung nennt den Grund statt eine entartete Fläche anzulegen (T0131).
#[test]
fn zwei_punkte_auf_einer_achse_werden_abgelehnt() {
    let mut s = Session::new();
    let err = s
        .execute("draw.fill.points", &json!({ "pts": [[0.0, 0.0], [1.0, 0.0]] }))
        .expect_err("zwei Punkte auf einer Waagerechten ergeben keine Fläche");
    assert!(err.contains("Achse"), "Meldung: {err}");
    assert!(s.doc.entities.is_empty(), "nichts angelegt");

    let err = s
        .execute("draw.fill.points", &json!({ "pts": [[0.0, 0.0], [0.0, 1.0]] }))
        .expect_err("zwei Punkte auf einer Senkrechten ergeben keine Fläche");
    assert!(err.contains("Achse"), "Meldung: {err}");
    assert!(s.doc.entities.is_empty(), "nichts angelegt");
}

/// Ein einzelner Punkt bleibt zu wenig — dort ist die alte Meldung richtig.
#[test]
fn ein_punkt_ist_zu_wenig() {
    let mut s = Session::new();
    let err = s
        .execute("draw.fill.points", &json!({ "pts": [[0.0, 0.0]] }))
        .expect_err("ein Punkt ist keine Fläche");
    assert!(err.contains("drei Punkte"), "Meldung: {err}");
    assert!(s.doc.entities.is_empty());
}

/// Zwei Punkte als **Diagonale** ergeben ein achsenparalleles Rechteck — für
/// Füllfläche, Schraffur und Volumen derselbe Weg (T0131).
#[test]
fn zwei_punkte_als_diagonale_ergeben_ein_rechteck() {
    // Füllfläche aus der Punktfolge.
    let mut s = Session::new();
    s.execute(
        "draw.fill.points",
        &json!({ "pts": [[0.0, 0.0], [4.0, 3.0]] }),
    )
    .expect("Diagonale muss eine Fläche anlegen");
    assert_eq!(s.doc.entities.len(), 1);
    let k = kanten(&s.doc.entities[0]);
    assert_eq!(k.len(), 4, "vier Ecken");
    for (got, want) in k.iter().zip([
        Vec2::new(0.0, 0.0),
        Vec2::new(4000.0, 0.0),
        Vec2::new(4000.0, 3000.0),
        Vec2::new(0.0, 3000.0),
    ]) {
        assert!((*got - want).len() < 1e-6, "Ecke {got:?} statt {want:?}");
    }
    assert!((flaeche_m2(&s.doc.entities[0]) - 12.0).abs() < 1e-9);

    // Schraffur (`draw.hatch`) nimmt dieselbe Diagonale.
    let mut h = Session::new();
    h.execute(
        "draw.hatch",
        &json!({ "boundary": [[0.0, 0.0], [4.0, 3.0]], "pattern": "lines" }),
    )
    .expect("Schraffur aus der Diagonale");
    assert_eq!(kanten(&h.doc.entities[0]).len(), 4);
    assert!((flaeche_m2(&h.doc.entities[0]) - 12.0).abs() < 1e-9);

    // Volumen: dieselbe Diagonale, dazu die Höhe.
    let mut v = Session::new();
    v.execute(
        "draw.volume.points",
        &json!({ "pts": [[0.0, 0.0], [4.0, 3.0]], "height": 2.5 }),
    )
    .expect("Volumen aus der Diagonale");
    match &v.doc.entities[0].kind {
        EntityKind::Volume {
            boundary, height, ..
        } => {
            assert_eq!(boundary.len(), 4, "vier Ecken");
            assert!((height - 2500.0).abs() < 1e-6, "Höhe {height}");
            let flaeche = polygon_area(boundary).abs() / 1_000_000.0;
            assert!((flaeche - 12.0).abs() < 1e-9, "Fläche {flaeche} m²");
        }
        other => panic!("kein Volumen, sondern {}", other.type_name()),
    }
}

/// Das Werkzeug steht im Register — nur so erscheint es in Ribbon-Inventar,
/// Tastenbelegung und in der MCP-Werkzeugliste (`jjcad_actions`).
#[test]
fn punktfolge_ist_im_aktionsregister_mit_kuerzel() {
    let def = jjcad_core::actions::action("draw.fill.points").expect("Action muss registriert sein");
    assert_eq!(def.key, "shift+i", "Tastenkürzel Umschalt+I");
    assert!(def.tool, "interaktives Werkzeug");
    assert!(jjcad_core::actions::is_mutating("draw.fill.points"));
    assert_eq!(def.group, jjcad_core::actions::ActionGroup::Zeichnen);
}

/// Speichern und Laden ändert nichts an der Fläche aus der Punktfolge.
#[test]
fn punktfolge_ueberlebt_speichern_und_laden() {
    let mut doc = Document::new();
    let pts = json!([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    jjcad_core::actions::execute(&mut doc, "draw.fill.points", &json!({ "pts": pts }))
        .expect("zeichnen");

    let datei = std::env::temp_dir().join("jjcad_fuellflaeche_punkte_test.jjcad");
    jjcad_core::persist::save_document(&mut doc, &datei).expect("speichern");
    let wieder = jjcad_core::persist::load_document(&datei).expect("laden");
    let _ = std::fs::remove_file(&datei);

    assert_eq!(wieder.entities.len(), 1);
    assert_eq!(kanten(&wieder.entities[0]), kanten(&doc.entities[0]));
    match &wieder.entities[0].kind {
        EntityKind::Fill { pattern, .. } => assert_eq!(*pattern, FillPattern::Lines),
        _ => panic!("Typ verloren"),
    }
}
