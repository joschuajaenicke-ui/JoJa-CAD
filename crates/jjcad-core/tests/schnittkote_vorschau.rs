//! Vorschau der Schnitt-Kote (T0122).
//!
//! Die Vorschau ist das **fertige** Objekt, kein Platzhalter: dieselbe Anzeige
//! wie die eingefügte Kote (Dreiecke, Füllung ab dem zweiten Punkt, Höhenangabe
//! unter jedem Punkt). Geprüft wird gegen den Renderer — denselben Weg, den
//! GUI, PDF, PNG und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::v;
use jjcad_core::model::{Document, EntityKind};
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut d = Document::new();
    d.settings.input_unit = Unit::Mm;
    d
}

/// Führt eine Aktion aus und liefert die Kennung des erzeugten Objekts.
fn neu(doc: &mut Document, action: &str, params: serde_json::Value) -> u64 {
    let r = actions::execute(doc, action, &params)
        .unwrap_or_else(|e| panic!("{action} fehlgeschlagen: {e}"));
    r["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{action} hat kein Objekt erzeugt"))
}

/// Die Anzeige eines einzelnen Objekts.
fn anzeige(doc: &Document, id: u64) -> render::DisplayList {
    let e = doc.entity(id).expect("Objekt fehlt").clone();
    render::render_entities(&RenderCtx::new(doc), std::slice::from_ref(&e))
}

fn texte(dl: &render::DisplayList) -> Vec<String> {
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

/// Die Vorschau zeigt Zeile für Zeile dasselbe wie die eingefügte Kote.
///
/// Nur die Objektkennung unterscheidet sich (die Vorschau gehört zu keinem
/// Objekt) — Form, Lage und Stil müssen gleich sein, sonst springt die Kote
/// beim Klick.
#[test]
fn vorschau_ist_zeilengleich_mit_der_eingefuegten_kote() {
    let mut d = doc_mm();
    let pts = vec![
        v(0.0, 0.0),
        v(1000.0, 1200.0),
        v(2000.0, -350.0),
        v(3000.0, 2600.0),
    ];
    let id = neu(
        &mut d,
        "dim.elevation.section",
        json!({"pts": [[0.0, 0.0], [1000.0, 1200.0], [2000.0, -350.0], [3000.0, 2600.0]]}),
    );
    let e = d.entity(id).expect("Kote fehlt").clone();
    let platziert = anzeige(&d, id);
    let vorschau = render::preview_elevation_section(&d, &pts, e.layer, e.style.color);

    assert_eq!(
        vorschau.prims.len(),
        platziert.prims.len(),
        "die Vorschau hat eine andere Zahl von Zeilen als das fertige Objekt"
    );
    for (i, (a, b)) in vorschau.prims.iter().zip(platziert.prims.iter()).enumerate() {
        assert_eq!(a.shape, b.shape, "Zeile {i}: andere Form");
        assert_eq!(a.style, b.style, "Zeile {i}: anderer Stil");
    }
}

/// Die Vorschau schreibt die Höhenangabe unter jedes Dreieck — auch unter den
/// Punkt am Zeiger. Ohne sie setzt der Nutzer die Kote blind (T0122).
#[test]
fn vorschau_zeigt_die_hoehen_an_jedem_punkt() {
    let d = doc_mm();
    let pts = vec![v(0.0, 0.0), v(1000.0, 1200.0), v(2000.0, -350.0)];
    let vorschau = render::preview_elevation_section(&d, &pts, d.current_layer, None);
    let texte = texte(&vorschau);
    assert_eq!(
        texte,
        vec!["±0,00".to_string(), "+1,200".to_string(), "-0,350".to_string()],
        "Höhenangaben der Vorschau"
    );
}

/// Der erste Punkt ist das offene Dreieck (±0,00), alle weiteren sind gefüllt —
/// genau wie beim eingefügten Objekt.
#[test]
fn vorschau_hat_offenes_erstes_und_gefuellte_weitere_dreiecke() {
    let d = doc_mm();
    let pts = vec![v(0.0, 0.0), v(1000.0, 1200.0)];
    let vorschau = render::preview_elevation_section(&d, &pts, d.current_layer, None);
    let fuellungen = vorschau
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Fill { .. }))
        .count();
    let dreiecke = vorschau
        .prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Poly(_, true)))
        .count();
    assert_eq!(dreiecke, 2, "ein Dreieck je Punkt");
    assert_eq!(fuellungen, 1, "nur der zweite Punkt ist gefüllt");
}

/// Ohne ausdrückliche Farbe ist die Vorschau schwarz — wie die eingefügte
/// Bemaßung (T0062), nicht in der Ebenenfarbe.
#[test]
fn vorschau_ohne_farbe_ist_schwarz() {
    let d = doc_mm();
    let vorschau = render::preview_elevation_section(&d, &[v(0.0, 0.0)], d.current_layer, None);
    let farben: Vec<_> = vorschau
        .prims
        .iter()
        .map(|p| p.style.color)
        .collect();
    assert!(!farben.is_empty(), "die Vorschau ist leer");
    assert!(
        farben.iter().all(|c| *c == jjcad_core::style::Color::BLACK),
        "Vorschau nicht schwarz: {farben:?}"
    );
}

/// Eine ausdrückliche Farbe (Eigenschaften-Panel) gilt auch in der Vorschau.
#[test]
fn vorschau_nimmt_die_gewaehlte_farbe() {
    let d = doc_mm();
    let vorschau = render::preview_elevation_section(
        &d,
        &[v(0.0, 0.0)],
        d.current_layer,
        Some(jjcad_core::style::Color::GREEN),
    );
    assert!(
        vorschau
            .prims
            .iter()
            .all(|p| p.style.color == jjcad_core::style::Color::GREEN),
        "die Panel-Farbe fehlt in der Vorschau"
    );
}

/// Die Vorschau legt nichts in die Zeichnung — sie ist reine Anzeige.
#[test]
fn vorschau_veraendert_die_zeichnung_nicht() {
    let d = doc_mm();
    let vorher = d.entities.len();
    let _ = render::preview_elevation_section(&d, &[v(0.0, 0.0), v(0.0, 1000.0)], d.current_layer, None);
    assert_eq!(d.entities.len(), vorher, "die Vorschau hat ein Objekt angelegt");
    assert!(
        !d.entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::ElevationSection { .. })),
        "die Vorschau steht in der Zeichnung"
    );
}
