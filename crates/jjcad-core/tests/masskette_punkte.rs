//! Maßkette: Messpunkte hinzufügen und entfernen (T0154).
//!
//! Einzelmaß und Maßkette sind **ein** Element (`EntityKind::Dim` mit `pts`);
//! eine Maßkette hat immer mindestens zwei Messpunkte. Geprüft wird über die
//! Befehls-Schicht — denselben Weg nehmen GUI, MCP und Tastatur.

use jjcad_core::actions;
use jjcad_core::geom::v;
use jjcad_core::model::{Document, EntityKind};
use jjcad_core::persist;
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut d = Document::new();
    d.settings.input_unit = Unit::Mm;
    d
}

fn kette(d: &mut Document) -> u64 {
    let r = actions::execute(
        d,
        "dim.chain",
        &json!({"pts": [[0.0, 0.0], [2000.0, 0.0], [5000.0, 0.0]],
                "line_pos": [1000.0, -400.0], "axis": "x"}),
    )
    .expect("Maßkette anlegen");
    r["created"][0].as_u64().expect("Kennung")
}

fn punkte(d: &Document, id: u64) -> Vec<jjcad_core::geom::Vec2> {
    match &d.entity(id).expect("Maßkette fehlt").kind {
        EntityKind::Dim { pts, .. } => pts.clone(),
        k => panic!("keine Maßkette: {k:?}"),
    }
}

/// Ein Einzelmaß ist technisch dasselbe Element wie eine Maßkette — nur mit
/// zwei Messpunkten.
#[test]
fn einzelmass_ist_eine_kette_aus_zwei_messpunkten() {
    let mut d = doc_mm();
    let r = actions::execute(
        &mut d,
        "dim.linear",
        &json!({"a": [0.0, 0.0], "b": [3000.0, 0.0], "line_pos": [1500.0, -300.0]}),
    )
    .expect("Einzelmaß");
    let id = r["created"][0].as_u64().unwrap();
    assert_eq!(punkte(&d, id), vec![v(0.0, 0.0), v(3000.0, 0.0)]);
    match &d.entity(id).unwrap().kind {
        EntityKind::Dim { angle, .. } => assert!(angle.abs() < 1e-9, "waagerecht: {angle}"),
        k => panic!("kein Dim: {k:?}"),
    }
}

/// Ein neuer Punkt wird so eingefügt, dass die Kette entlang ihrer
/// Messrichtung geordnet bleibt — sonst entstünde ein Zickzack.
#[test]
fn punkt_hinzufuegen_haelt_die_reihenfolge() {
    let mut d = doc_mm();
    let id = kette(&mut d);

    // Zwischen 2,00 m und 5,00 m → Platz 2.
    actions::execute(&mut d, "dim.point.add", &json!({"ids": [id], "p": [3500.0, 0.0]}))
        .expect("Punkt hinzufügen");
    assert_eq!(
        punkte(&d, id),
        vec![v(0.0, 0.0), v(2000.0, 0.0), v(3500.0, 0.0), v(5000.0, 0.0)]
    );

    // Vor dem ersten Punkt → Platz 0.
    actions::execute(&mut d, "dim.point.add", &json!({"ids": [id], "p": [-800.0, 0.0]}))
        .expect("Punkt vorn einfügen");
    assert_eq!(punkte(&d, id).first().copied(), Some(v(-800.0, 0.0)));

    // Hinter dem letzten Punkt → ans Ende.
    actions::execute(&mut d, "dim.point.add", &json!({"ids": [id], "p": [6000.0, 0.0]}))
        .expect("Punkt hinten einfügen");
    assert_eq!(punkte(&d, id).last().copied(), Some(v(6000.0, 0.0)));
}

/// `index` überschreibt die gerechnete Stelle ausdrücklich.
#[test]
fn punkt_hinzufuegen_mit_index() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(
        &mut d,
        "dim.point.add",
        &json!({"ids": [id], "p": [9000.0, 0.0], "index": 1}),
    )
    .expect("Punkt mit Index");
    assert_eq!(
        punkte(&d, id),
        vec![v(0.0, 0.0), v(9000.0, 0.0), v(2000.0, 0.0), v(5000.0, 0.0)]
    );
}

/// Entfernen nimmt den **nächsten** Punkt zur Klickstelle; unter zwei Punkten
/// ist eine Maßkette keine Maßkette mehr (Fehler, kein stiller Erfolg).
#[test]
fn punkt_entfernen_und_die_grenze_von_zwei() {
    let mut d = doc_mm();
    let id = kette(&mut d);

    actions::execute(&mut d, "dim.point.remove", &json!({"ids": [id], "p": [2100.0, 0.0]}))
        .expect("Punkt entfernen");
    assert_eq!(punkte(&d, id), vec![v(0.0, 0.0), v(5000.0, 0.0)]);

    let fehler = actions::execute(&mut d, "dim.point.remove", &json!({"ids": [id], "p": [0.0, 0.0]}));
    assert!(
        fehler.is_err(),
        "die letzte von zwei Messpunkten darf nicht verschwinden"
    );
    assert_eq!(punkte(&d, id).len(), 2, "die Kette bleibt unverändert");
}

/// Eine fremde Objektart ist kein Ziel — sonst entstünde ein stiller
/// Fehlschlag (T0038).
#[test]
fn nur_massketten_bekommen_messpunkte() {
    let mut d = doc_mm();
    let r = actions::execute(
        &mut d,
        "draw.line",
        &json!({"a": [0.0, 0.0], "b": [1000.0, 0.0]}),
    )
    .unwrap();
    let id = r["created"][0].as_u64().unwrap();
    assert!(actions::execute(
        &mut d,
        "dim.point.add",
        &json!({"ids": [id], "p": [500.0, 0.0]})
    )
    .is_err());
}

/// Alte Zeichnungen: das Einzelmaß (`a`, `b`) und die Maßkette (`dim_chain`)
/// werden beim Laden zu einem Element mit `pts` (T0154).
#[test]
fn alte_dateien_werden_umgeschrieben() {
    let alt_einzelmass = r#"{
        "format": "jjcad",
        "version": 1,
        "entities": [
            {"id": 1, "kind": {"type": "dim", "a": {"x": 0.0, "y": 0.0},
                               "b": {"x": 3000.0, "y": 0.0},
                               "line_pos": {"x": 1500.0, "y": -300.0},
                               "axis": "X", "override_text": "3,00"}}
        ]
    }"#;
    let d = persist::from_json(alt_einzelmass).expect("alte Datei lesen");
    match &d.entity(1).expect("Maß fehlt").kind {
        EntityKind::Dim {
            pts,
            override_text,
            ..
        } => {
            assert_eq!(pts, &vec![v(0.0, 0.0), v(3000.0, 0.0)]);
            assert_eq!(override_text.as_deref(), Some("3,00"));
        }
        k => panic!("kein Dim: {k:?}"),
    }

    let alt_kette = r#"{
        "format": "jjcad",
        "version": 1,
        "entities": [
            {"id": 2, "kind": {"type": "dim_chain",
                               "pts": [{"x": 0.0, "y": 0.0}, {"x": 2000.0, "y": 0.0}],
                               "line_pos": {"x": 1000.0, "y": -400.0}, "axis": "X"}}
        ]
    }"#;
    let d = persist::from_json(alt_kette).expect("alte Kette lesen");
    match &d.entity(2).expect("Maßkette fehlt").kind {
        EntityKind::Dim { pts, .. } => assert_eq!(pts.len(), 2),
        k => panic!("keine Maßkette: {k:?}"),
    }
}

/// Vorschau und eingefügte Maßkette sind **zeilengleich** — dieselbe Anzeige,
/// kein eigener Platzhalter im Zeichner (T0154).
#[test]
fn vorschau_ist_das_fertige_objekt() {
    use jjcad_core::render::{self, RenderCtx};

    let mut d = doc_mm();
    let pts = vec![v(0.0, 0.0), v(2000.0, 0.0), v(5000.0, 0.0)];
    let id = kette(&mut d);
    let mut e = d.entity(id).expect("Maßkette").clone();
    // Mit Verlängerung prüfen (T0226): so deckt der Vergleich auch die
    // Maßhilfslinien ab, die die Vorgabe aus lässt.
    if let EntityKind::Dim { ext_lines, .. } = &mut e.kind {
        *ext_lines = true;
    }
    let anzeige = render::render_entities(&RenderCtx::new(&d), std::slice::from_ref(&e));
    let vorschau = render::preview_dim(&d, &pts, v(1000.0, -400.0), 0.0, e.layer, e.style.color, true);

    assert_eq!(
        vorschau.prims.len(),
        anzeige.prims.len(),
        "die Vorschau hat eine andere Zahl von Zeilen als das fertige Objekt"
    );
    for (i, (a, b)) in vorschau.prims.iter().zip(anzeige.prims.iter()).enumerate() {
        assert_eq!(a.shape, b.shape, "Zeile {i}: andere Form");
        assert_eq!(a.style, b.style, "Zeile {i}: anderer Stil");
    }
}

/// Sonderfang: die Maßlinie sitzt parallel zu einer vorhandenen im Abstand
/// 0,75 m (T0154).
#[test]
fn masslinie_faengt_parallel_im_abstand_075() {
    use jjcad_core::snap;

    let mut d = doc_mm();
    // Vorhandene waagerechte Kette, Maßlinie auf y = -400.
    kette(&mut d);

    // Zeiger genau 750 mm darunter → die Parallele wird gefangen.
    let pts = vec![v(0.0, 0.0), v(1000.0, 0.0)];
    let p = snap::dim_parallel(&d, &pts, 0.0, v(700.0, -1150.0), 50.0)
        .expect("Parallele nicht gefangen");
    assert!((p.y + 1150.0).abs() < 1e-6, "falsche Höhe: {p:?}");
    assert!((p.x - 700.0).abs() < 1e-6, "Lage auf der Linie bleibt: {p:?}");

    // Auch von der anderen Seite (750 mm darüber).
    let oben = snap::dim_parallel(&d, &pts, 0.0, v(700.0, 350.0), 50.0)
        .expect("Parallele oben nicht gefangen");
    assert!((oben.y - 350.0).abs() < 1e-6, "{oben:?}");

    // Außerhalb des Fangradius: kein Fang.
    assert!(snap::dim_parallel(&d, &pts, 0.0, v(700.0, -1300.0), 50.0).is_none());
    // Senkrechte Kette ist kein Vorbild für eine waagerechte Maßlinie.
    assert!(snap::dim_parallel(
        &d,
        &pts,
        std::f64::consts::FRAC_PI_2,
        v(700.0, -1150.0),
        50.0
    )
    .is_none());
}

/// Dasselbe für die schräge („direkte“) Kette: die Parallele läuft im rechten
/// Winkel zur Messrichtung.
#[test]
fn schraege_masslinie_faengt_parallel() {
    use jjcad_core::snap;

    let mut d = doc_mm();
    actions::execute(
        &mut d,
        "dim.chain",
        &json!({"pts": [[0.0, 0.0], [3000.0, 3000.0]],
                "line_pos": [0.0, 1000.0], "axis": "aligned"}),
    )
    .expect("schräge Kette");

    let pts = vec![v(0.0, 0.0), v(3000.0, 3000.0)];
    // Normale der 45°-Richtung ist (-1, 1)/√2; 750 mm davon entfernt.
    let vers = v(-1.0, 1.0) * (750.0 / 2f64.sqrt());
    let ziel = v(0.0, 1000.0) + vers;
    let p = snap::dim_parallel(
        &d,
        &pts,
        std::f64::consts::FRAC_PI_4,
        ziel + v(10.0, 0.0),
        60.0,
    )
        .expect("schräge Parallele nicht gefangen");
    let abstand = (p - v(0.0, 1000.0)).dot(vers.normalized()).abs();
    assert!((abstand - 750.0).abs() < 1.0, "Abstand {abstand}");
}

/// Gespeicherte Datei: eine Maßkette steht als **ein** Objekt mit `pts` darin
/// und kommt mit allen Messpunkten wieder heraus (T0154).
#[test]
fn gespeicherte_masskette_traegt_alle_messpunkte() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(&mut d, "dim.point.add", &json!({"ids": [id], "p": [3500.0, 0.0]}))
        .expect("Punkt hinzufügen");

    let text = persist::to_json(&d).expect("schreiben");
    assert!(
        text.contains("\"type\": \"dim\""),
        "die Bemaßung steht nicht als „dim“ in der Datei"
    );
    assert!(
        !text.contains("dim_chain"),
        "der alte Typ darf nicht mehr geschrieben werden"
    );
    let wieder = persist::from_json(&text).expect("lesen");
    assert_eq!(
        punkte(&wieder, id),
        vec![v(0.0, 0.0), v(2000.0, 0.0), v(3500.0, 0.0), v(5000.0, 0.0)],
        "die Messpunkte überstehen Speichern und Laden"
    );
}

/// Punkte dürfen in beliebiger Reihenfolge gesetzt werden — die Kette wird
/// beim Anlegen sortiert (T0154, Turn 3).
#[test]
fn kette_sortiert_die_messpunkte_beim_anlegen() {
    let mut d = doc_mm();
    // Absichtlich durcheinander: 1., 3., 2., 4. Punkt.
    let r = actions::execute(
        &mut d,
        "dim.chain",
        &json!({"pts": [[0.0, 0.0], [5000.0, 0.0], [2000.0, 0.0], [3500.0, 0.0]],
                "line_pos": [1000.0, -500.0], "axis": "x"}),
    )
    .expect("Maßkette");
    let id = r["created"][0].as_u64().unwrap();
    assert_eq!(
        punkte(&d, id),
        vec![
            v(0.0, 0.0),
            v(2000.0, 0.0),
            v(3500.0, 0.0),
            v(5000.0, 0.0)
        ],
        "die Kette steht in Reihenfolge der Maßlinie"
    );
}

/// Dasselbe senkrecht: sortiert wird nach y.
#[test]
fn senkrechte_kette_sortiert_nach_hoehe() {
    let mut d = doc_mm();
    let r = actions::execute(
        &mut d,
        "dim.chain.v",
        &json!({"pts": [[0.0, 2500.0], [0.0, 0.0], [0.0, 1200.0]],
                "line_pos": [-800.0, 0.0]}),
    )
    .expect("senkrechte Kette");
    let id = r["created"][0].as_u64().unwrap();
    assert_eq!(
        punkte(&d, id),
        vec![v(0.0, 0.0), v(0.0, 1200.0), v(0.0, 2500.0)]
    );
    match &d.entity(id).unwrap().kind {
        EntityKind::Dim { angle, .. } => assert!(
            (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "senkrecht: {angle}"
        ),
        k => panic!("kein Dim: {k:?}"),
    }
}

/// Die drei Werkzeuge legen ihre Achse selbst fest — ein mitgegebenes `axis`
/// sticht sie nicht (der Modus steckt im Werkzeug).
#[test]
fn die_drei_werkzeuge_setzen_ihre_achse() {
    let mut d = doc_mm();
    for (werkzeug, winkel) in [
        ("dim.chain.h", 0.0),
        ("dim.chain.v", std::f64::consts::FRAC_PI_2),
        // „Direkt“ nimmt die Richtung der ersten Messstrecke (2000, 500).
        ("dim.chain.d", 500f64.atan2(2000.0)),
    ] {
        let r = actions::execute(
            &mut d,
            werkzeug,
            &json!({"pts": [[0.0, 0.0], [2000.0, 500.0]],
                    "line_pos": [0.0, -500.0], "axis": "x"}),
        )
        .unwrap_or_else(|e| panic!("{werkzeug} fehlgeschlagen: {e}"));
        let id = r["created"][0].as_u64().unwrap();
        match &d.entity(id).unwrap().kind {
            EntityKind::Dim { angle, .. } => assert!(
                (angle - winkel).abs() < 1e-9,
                "{werkzeug} hat den falschen Winkel: {angle} statt {winkel}"
            ),
            k => panic!("kein Dim: {k:?}"),
        }
    }
}

/// Schräge Kette: sortiert wird entlang der Richtung der ersten beiden
/// Punkte, ein seitlich gesetzter Punkt landet an seiner Stelle.
#[test]
fn schraege_kette_sortiert_entlang_der_richtung() {
    let mut d = doc_mm();
    let r = actions::execute(
        &mut d,
        "dim.chain.d",
        &json!({"pts": [[0.0, 0.0], [3000.0, 3000.0], [1000.0, 1000.0]],
                "line_pos": [0.0, 1000.0]}),
    )
    .expect("schräge Kette");
    let id = r["created"][0].as_u64().unwrap();
    assert_eq!(
        punkte(&d, id),
        vec![v(0.0, 0.0), v(1000.0, 1000.0), v(3000.0, 3000.0)]
    );
}
