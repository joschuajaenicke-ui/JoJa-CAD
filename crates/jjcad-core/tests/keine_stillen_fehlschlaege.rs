//! T0038: keine stillen Fehlschläge.
//!
//! Zwei Fälle lieferten früher `ok: true`, obwohl das Ergebnis unbrauchbar war:
//! eine Maßkette ohne `axis` bei senkrechter Punktfolge (Maßtext „0,00") und
//! eine Tür ohne Wand (freistehende Tür, `host: null`). Beides muss jetzt
//! entweder richtig rechnen oder einen Fehler melden.

use jjcad_core::model::EntityKind;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

/// Alle Maßtexte der Zeichnung in der Reihenfolge der Anzeige (Millimeter,
/// Einheit aus `doc.settings.dim_unit`).
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

/// Repro aus dem Ticket: lotrechte Kette **ohne** `axis`.
#[test]
fn masskette_senkrecht_ohne_achse_misst_senkrecht() {
    let mut s = s();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0, 0], [0, 6.15], [0, 8.76]], "line_pos": [-1.0, 0]}),
    )
    .unwrap();

    let e = s.doc.entities.last().unwrap();
    match &e.kind {
        EntityKind::Dim { angle, .. } => assert!(
            (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "senkrechte Kette muss senkrecht messen: {angle}"
        ),
        other => panic!("keine Maßkette: {other:?}"),
    }

    let texte = mass_texte(&s);
    assert!(
        texte.iter().any(|t| t == "6,15"),
        "Maßtext 6,15 fehlt: {texte:?}"
    );
    assert!(
        texte.iter().any(|t| t == "2,61"),
        "Maßtext 2,61 fehlt: {texte:?}"
    );
    assert!(
        !texte.iter().any(|t| t.contains("0,00")),
        "zwei verschiedene Punkte dürfen keinen Maßtext 0,00 ergeben: {texte:?}"
    );
}

/// Waagerechte Kette ohne `axis` bleibt waagerecht (Gegenprobe zur Dominanz).
#[test]
fn masskette_waagerecht_ohne_achse_misst_waagerecht() {
    let mut s = s();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0, 0], [1, 0], [2.5, 0]], "line_pos": [0, -0.5]}),
    )
    .unwrap();
    match &s.doc.entities.last().unwrap().kind {
        EntityKind::Dim { angle, .. } => assert!(angle.abs() < 1e-9, "waagerecht: {angle}"),
        other => panic!("keine Maßkette: {other:?}"),
    }
    let texte = mass_texte(&s);
    assert!(texte.iter().any(|t| t == "1,00"), "{texte:?}");
    assert!(texte.iter().any(|t| t == "1,50"), "{texte:?}");
}

/// Gemischte Kette (erst senkrecht, dann waagerecht): keine Achse trägt alle
/// Maße — statt eines falschen „0,00" kommt ein Fehler mit Hinweis.
#[test]
fn masskette_gemischt_ohne_achse_ist_ein_fehler() {
    let mut s = s();
    let err = s
        .execute(
            "dim.chain",
            &json!({"pts": [[0, 0], [0, 2], [2, 2]], "line_pos": [-0.5, 2.5]}),
        )
        .unwrap_err();
    assert!(
        err.contains("Achse nicht bestimmbar"),
        "Fehlertext nennt die Achse nicht: {err}"
    );
    assert!(s.doc.entities.is_empty(), "keine halbe Maßkette anlegen");

    // Mit ausdrücklicher Achse geht dieselbe Kette.
    s.execute(
        "dim.chain",
        &json!({"pts": [[0, 0], [0, 2], [2, 2]], "line_pos": [-0.5, 2.5], "axis": "x"}),
    )
    .unwrap();
    assert_eq!(s.doc.entities.len(), 1);
}

/// Repro aus dem Ticket: Tür weit weg von jeder Wand.
#[test]
fn tuer_ohne_wand_ist_ein_fehler() {
    let mut s = s();
    let err = s
        .execute("draw.door", &json!({"center": [50, 50], "width": 1.0}))
        .unwrap_err();
    assert!(err.contains("keine Wand"), "Fehlertext: {err}");
    assert!(
        s.doc.entities.is_empty(),
        "freistehende Tür darf nicht entstehen"
    );
}

/// Fenster ohne Wand ebenso.
#[test]
fn fenster_ohne_wand_ist_ein_fehler() {
    let mut s = s();
    let err = s
        .execute("draw.window", &json!({"center": [50, 50], "width": 1.0}))
        .unwrap_err();
    assert!(err.contains("keine Wand"), "Fehlertext: {err}");
    assert!(s.doc.entities.is_empty());
}

/// `host` auf ein Objekt, das keine Wand ist, wird nicht übernommen.
#[test]
fn host_ohne_wand_ist_ein_fehler() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [3, 0]}))
        .unwrap();
    let linie = s.doc.entities.last().unwrap().id;
    let err = s
        .execute(
            "draw.door",
            &json!({"center": [1.5, 0], "width": 0.9, "host": [linie]}),
        )
        .unwrap_err();
    assert!(err.contains("keine Wand"), "Fehlertext: {err}");
    assert_eq!(s.doc.entities.len(), 1, "nur die Linie");
}

/// Tür an einer Wand funktioniert unverändert (mit und ohne `host`).
#[test]
fn tuer_an_der_wand_bleibt_erlaubt() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}),
    )
    .unwrap();
    let wand = s.doc.entities.last().unwrap().id;
    s.execute("draw.door", &json!({"center": [2, 0], "width": 0.9}))
        .unwrap();
    let mut gehostet = 0;
    for e in &s.doc.entities {
        if let EntityKind::Door { host, .. } = &e.kind {
            assert_eq!(*host, Some(wand), "Tür muss auf der Wand sitzen");
            gehostet += 1;
        }
    }
    assert_eq!(gehostet, 1);
}
