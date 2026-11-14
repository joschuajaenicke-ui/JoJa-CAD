//! Höhenkoten (T0096).
//!
//! Zwei getrennte Marken ersetzen die frühere „Höhenkote“:
//!
//! * **Grundriss-Kote** — Kreis-Symbol mit Fadenkreuz und zwei gefüllten
//!   Vierteln; der Höhenwert steht rechts daneben und gehört zum Symbol.
//! * **Schnitt-Kote** — eine Punktfolge aus Dreieck und Höhenangabe. Der erste
//!   Punkt ist ±0,00, alle weiteren Höhen ergeben sich aus der Y-Differenz zu
//!   ihm.
//!
//! Geprüft wird über die Befehls-Schicht und den Renderer — denselben Weg, den
//! GUI, PDF, PNG und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::v;
use jjcad_core::model::{Document, EntityKind, RANG_BESCHRIFTUNG};
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

/// Alle Texte der Anzeige, in Zeichenreihenfolge.
fn texte(doc: &Document) -> Vec<String> {
    let ctx = RenderCtx::new(doc);
    let dl = render::render_document(&ctx);
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

fn anzahl(doc: &Document, treffer: impl Fn(&Shape) -> bool) -> usize {
    let ctx = RenderCtx::new(doc);
    let dl = render::render_document(&ctx);
    dl.prims.iter().filter(|p| treffer(&p.shape)).count()
}

// ---------------------------------------------------------------------------
// Grundriss-Kote
// ---------------------------------------------------------------------------

#[test]
fn grundriss_kote_ist_kreis_mit_gefuellten_vierteln_und_hoehenwert() {
    let mut doc = doc_mm();
    let id = neu(
        &mut doc,
        "dim.elevation.plan",
        json!({"p": [1000.0, 2000.0], "z": 1200.0}),
    );
    let e = doc.entity(id).expect("Kote fehlt");
    assert!(matches!(
        e.kind,
        EntityKind::ElevationPlan { z, .. } if (z - 1200.0).abs() < 1e-9
    ));

    // Höhenwert steht am Symbol, im Metern mit Vorzeichen.
    assert_eq!(texte(&doc), vec!["+1,20".to_string()]);

    // Kreis (voller Bogen) und zwei gefüllte Viertel.
    let h = doc.dim_style.text_height * doc.settings.annotation_scale;
    let r = h * 0.55;
    assert_eq!(
        anzahl(&doc, |s| matches!(
            s,
            Shape::Arc { c, r: rr, a0, a1 }
                if (*c - v(1000.0, 2000.0)).len() < 1e-9
                    && (rr - r).abs() < 1e-9
                    && (a0 - a1).abs() < 1e-12
        )),
        1,
        "ein Kreis um den Klickpunkt fehlt"
    );
    assert_eq!(
        anzahl(&doc, |s| matches!(s, Shape::Fill { .. })),
        2,
        "zwei gefüllte Viertel erwartet"
    );
    // Fadenkreuz: waagerecht und senkrecht durch den Mittelpunkt.
    let arm = r * 1.4;
    let achsen = |s: &Shape| match s {
        Shape::Line(a, b) => {
            let waagerecht = ((a.y - 2000.0).abs() < 1e-9 && (b.y - 2000.0).abs() < 1e-9)
                && ((a.x - (1000.0 - arm)).abs() < 1e-9 || (a.x - (1000.0 + arm)).abs() < 1e-9);
            let senkrecht = ((a.x - 1000.0).abs() < 1e-9 && (b.x - 1000.0).abs() < 1e-9)
                && ((a.y - (2000.0 - arm)).abs() < 1e-9 || (a.y - (2000.0 + arm)).abs() < 1e-9);
            waagerecht || senkrecht
        }
        _ => false,
    };
    assert_eq!(anzahl(&doc, achsen), 2, "Fadenkreuz unvollständig");
}

#[test]
fn grundriss_kote_zeigt_minus_null_und_negative_hoehen() {
    let mut doc = doc_mm();
    neu(
        &mut doc,
        "dim.elevation.plan",
        json!({"p": [0.0, 0.0], "z": 0.0}),
    );
    neu(
        &mut doc,
        "dim.elevation.plan",
        json!({"p": [4000.0, 0.0], "z": -1200.0}),
    );
    assert_eq!(texte(&doc), vec!["±0,00".to_string(), "-1,20".to_string()]);
}

// ---------------------------------------------------------------------------
// Schnitt-Kote
// ---------------------------------------------------------------------------

#[test]
fn schnitt_kote_rechnet_hoehen_aus_der_y_differenz() {
    let mut doc = doc_mm();
    let id = neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[0.0, 0.0], [1000.0, 1200.0], [2000.0, 2600.0], [3000.0, -350.0]]}),
    );
    match &doc.entity(id).expect("Kote fehlt").kind {
        EntityKind::ElevationSection { pts } => assert_eq!(pts.len(), 4),
        k => panic!("falsche Objektart: {k:?}"),
    }

    // Erster Punkt ±0,00, danach die Y-Differenz in Metern.
    assert_eq!(
        texte(&doc),
        vec![
            "±0,00".to_string(),
            "+1,200".to_string(),
            "+2,600".to_string(),
            "-0,350".to_string()
        ]
    );

    // Ein Dreieck je Punkt; nur das erste (Bezug) ist offen, die übrigen
    // gefüllt.
    let dreiecke = anzahl(&doc, |s| matches!(s, Shape::Poly(pts, true) if pts.len() == 3));
    assert_eq!(dreiecke, 4, "ein Dreieck je Punkt erwartet");
    let gefuellt = anzahl(&doc, |s| matches!(s, Shape::Fill { .. }));
    assert_eq!(gefuellt, 3, "nur der Bezugspunkt bleibt offen");
}

#[test]
fn erste_kote_ist_immer_die_bezugsnull() {
    let mut doc = doc_mm();
    // Dieselbe Punktfolge, nur insgesamt 5 m nach oben verschoben: die Höhen
    // bleiben gleich, weil sie an der Y-Differenz hängen.
    neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[0.0, 5000.0], [1000.0, 6200.0], [2000.0, 7600.0]]}),
    );
    assert_eq!(
        texte(&doc),
        vec![
            "±0,00".to_string(),
            "+1,200".to_string(),
            "+2,600".to_string()
        ]
    );
}

#[test]
fn schnitt_kote_braucht_mindestens_einen_punkt() {
    let mut doc = doc_mm();
    let r = actions::execute(&mut doc, "dim.elevation.section", &json!({"pts": []}));
    assert!(r.is_err(), "leere Punktfolge muss ein Fehler sein");
    // Ein einzelner Punkt ist gültig: reine Bezugshöhe ±0,00.
    neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[500.0, 0.0]]}),
    );
    assert_eq!(texte(&doc), vec!["±0,00".to_string()]);
}

#[test]
fn schnitt_kote_laesst_sich_ueber_die_befehle_verschieben() {
    let mut doc = doc_mm();
    let id = neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[0.0, 0.0], [1000.0, 1000.0]]}),
    );
    actions::execute(
        &mut doc,
        "modify.set",
        &json!({"ids": [id], "pts": [[0.0, 0.0], [1000.0, 2500.0]]}),
    )
    .expect("Punkte setzen");
    assert_eq!(texte(&doc), vec!["±0,00".to_string(), "+2,500".to_string()]);
}

// ---------------------------------------------------------------------------
// Art, Rang, Fang, Speichern
// ---------------------------------------------------------------------------

#[test]
fn koten_sind_beschriftung_und_werden_erkannt() {
    let mut doc = doc_mm();
    let plan = neu(
        &mut doc,
        "dim.elevation.plan",
        json!({"p": [0.0, 0.0], "z": 0.0}),
    );
    let schnitt = neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[0.0, 0.0], [0.0, 1000.0]]}),
    );
    for (id, name) in [(plan, "Grundriss-Kote"), (schnitt, "Schnitt-Kote")] {
        let e = doc.entity(id).expect("Kote fehlt");
        assert_eq!(e.kind.type_name(), name);
        assert_eq!(e.zeichenrang(), RANG_BESCHRIFTUNG);
        assert!(e.kind.is_dimension());
    }
}

#[test]
fn kote_laesst_sich_auf_der_gezeichneten_linie_anklicken() {
    let mut doc = doc_mm();
    let plan = neu(
        &mut doc,
        "dim.elevation.plan",
        json!({"p": [1000.0, 1000.0], "z": 0.0}),
    );
    let r = doc.dim_style.text_height * doc.settings.annotation_scale * 0.55;
    // Punkt auf dem Kreisrand (45°, abseits des Fadenkreuzes): die
    // Trefferflächen kommen aus der Anzeige.
    let rand = v(1000.0, 1000.0) + v(r, r) * std::f64::consts::FRAC_1_SQRT_2;
    let treffer = jjcad_core::snap::hit_test(&doc, rand, 2.0, None);
    assert_eq!(treffer, Some(plan), "Kreisrand muss die Kote treffen");

    let schnitt = neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[4000.0, 0.0], [5000.0, 800.0]]}),
    );
    let h = doc.dim_style.text_height * doc.settings.annotation_scale;
    // Punkt auf der linken Dreieckskante des zweiten Punktes.
    let links = v(5000.0 - h * 0.45, 800.0 + h * 0.9);
    let treffer = jjcad_core::snap::hit_test(&doc, links + v(0.0, -2.0), 3.0, None);
    assert_eq!(treffer, Some(schnitt), "Dreieck muss die Kote treffen");
}

/// Eine Schnitt-Kote über eine Diagonale darf nicht als ganzes Rechteck
/// anklickbar sein — getroffen werden nur Dreiecke und Höhenwerte.
#[test]
fn klick_zwischen_den_punkten_trifft_die_schnitt_kote_nicht() {
    let mut doc = doc_mm();
    let id = neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[0.0, 0.0], [4000.0, 4000.0]]}),
    );
    let mitte = v(2000.0, 2000.0);
    assert_eq!(
        jjcad_core::snap::hit_test(&doc, mitte, 2.0, None),
        None,
        "die leere Fläche zwischen den Punkten gehört nicht zur Kote"
    );

    // Der Höhenwert über dem zweiten Dreieck ist ein Treffer.
    let h = doc.dim_style.text_height * doc.settings.annotation_scale;
    let text = v(4000.0, 4000.0 + h * 0.9 + h * 0.25 + h * 0.5);
    assert_eq!(
        jjcad_core::snap::hit_test(&doc, text, 2.0, None),
        Some(id),
        "Höhenwert muss anklickbar sein"
    );
}

/// Ein Auswahlfenster über die ganze Kote wählt sie; ein Querfenster, das nur
/// die leere Fläche zwischen den Punkten berührt, nicht.
#[test]
fn auswahlfenster_prueft_die_gezeichnete_geometrie() {
    let mut doc = doc_mm();
    let id = neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[0.0, 0.0], [4000.0, 4000.0]]}),
    );
    assert!(
        jjcad_core::snap::select_window(&doc, v(-500.0, -500.0), v(4500.0, 4500.0), false)
            .contains(&id),
        "Fenster über allem muss die Kote wählen"
    );
    assert!(
        !jjcad_core::snap::select_window(&doc, v(1800.0, 1800.0), v(2200.0, 2200.0), false)
            .contains(&id),
        "Fenster in der leeren Fläche darf die Kote nicht wählen"
    );
}

#[test]
fn koten_ueberleben_speichern_und_laden() {
    let mut doc = doc_mm();
    neu(
        &mut doc,
        "dim.elevation.plan",
        json!({"p": [1000.0, 2000.0], "z": 3500.0}),
    );
    neu(
        &mut doc,
        "dim.elevation.section",
        json!({"pts": [[0.0, 0.0], [0.0, 1200.0]]}),
    );
    let json_text = jjcad_core::persist::to_json(&doc).expect("schreiben");
    assert!(json_text.contains("\"elevation_plan\""));
    assert!(json_text.contains("\"elevation_section\""));

    let wieder = jjcad_core::persist::from_json(&json_text).expect("lesen");
    let koten: Vec<&EntityKind> = wieder
        .entities
        .iter()
        .map(|e| &e.kind)
        .filter(|k| {
            matches!(
                k,
                EntityKind::ElevationPlan { .. } | EntityKind::ElevationSection { .. }
            )
        })
        .collect();
    assert_eq!(koten.len(), 2, "beide Koten fehlen nach dem Laden");
    assert!(matches!(
        koten[0],
        EntityKind::ElevationPlan { z, .. } if (z - 3500.0).abs() < 1e-9
    ));
    assert!(matches!(
        koten[1],
        EntityKind::ElevationSection { pts } if pts.len() == 2
    ));
}

/// Alte Zeichnungen tragen die frühere Art `elevation` (Dreieck + Text). Sie
/// werden als Grundriss-Kote gelesen — die Höhe bleibt erhalten, der alte
/// Zusatztext entfällt mit der alten Umsetzung.
#[test]
fn alte_elevation_wird_als_grundriss_kote_gelesen() {
    let alt = json!({
        "type": "elevation",
        "p": {"x": 1000.0, "y": 2000.0},
        "z": 2500.0,
        "angle": 3.141592653589793,
        "override_text": "OK"
    });
    let kind: EntityKind = serde_json::from_value(alt).expect("alte Höhenkote lesen");
    assert!(matches!(
        kind,
        EntityKind::ElevationPlan { p, z, .. }
            if (p - v(1000.0, 2000.0)).len() < 1e-9 && (z - 2500.0).abs() < 1e-9
    ));
}
