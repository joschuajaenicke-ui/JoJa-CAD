//! T0195: Eine Maßkette zählt ihre Abstände immer in Messreihenfolge.
//!
//! Die Reihenfolge in `pts` **ist** die Reihenfolge der Maßzahlen. Wandert ein
//! Messpunkt durch Ziehen zwischen seine Nachbarn, muss die Kette neu sortiert
//! werden — sonst stehen Maßzahlen über Strecken, die es so nicht mehr gibt.
//! Geprüft werden die drei Wege, auf denen Punkte wandern: Griff (GUI, siehe
//! `handles::tests_dim`), Verschieben/Drehen/Spiegeln (`transform`) und das
//! Laden einer bereits verdrehten Datei (`persist`).

use jjcad_core::geom::{v, Affine, Vec2};
use jjcad_core::model::{Document, Entity, EntityKind};
use jjcad_core::persist;
use jjcad_core::render::{dim_reihenfolge, render_document, RenderCtx, Shape};
use jjcad_core::transform;
use serde_json::json;

fn dim(pts: Vec<Vec2>) -> EntityKind {
    EntityKind::Dim {
        pts,
        line_pos: v(0.0, -400.0),
        angle: 0.0,
        override_text: None,
    }
}

fn punkte(k: &EntityKind) -> Vec<Vec2> {
    match k {
        EntityKind::Dim { pts, .. } => pts.clone(),
        _ => panic!("keine Maßkette: {k:?}"),
    }
}

/// Die Maßzahlen, die der Renderer schreibt (in der Reihenfolge der Anzeige).
fn masszahlen(k: &EntityKind) -> Vec<String> {
    let mut doc = Document::new();
    doc.add_entity(Entity::new(1, 1, k.clone()));
    render_document(&RenderCtx::new(&doc))
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

/// Der dritte Punkt wird zwischen die ersten beiden gezogen — die Kette steht
/// danach in Messreihenfolge, und die Maßzahlen gehören zu den Nachbarn.
#[test]
fn gezogener_punkt_landet_an_seiner_stelle() {
    let mut k = dim(vec![v(0.0, 0.0), v(2000.0, 0.0), v(5000.0, 0.0)]);
    // Wie beim Ziehen: der gespeicherte Punkt wandert, die Reihenfolge bleibt
    // zunächst stehen (genau der Fehlerfall aus T0195).
    if let EntityKind::Dim { pts, .. } = &mut k {
        pts[2] = v(1000.0, 0.0);
    }
    assert!(
        dim_reihenfolge(&mut k),
        "die Kette muss neu sortiert werden"
    );
    assert_eq!(
        punkte(&k),
        vec![v(0.0, 0.0), v(1000.0, 0.0), v(2000.0, 0.0)],
        "Messpunkte nicht in Messreihenfolge"
    );
    // Zwei Strecken von je 1,00 m — nicht 2,00 m und 1,00 m rückwärts.
    assert_eq!(masszahlen(&k), vec!["1,00".to_string(), "1,00".to_string()]);
}

/// Die Gegenprobe zum Fehlerfall: ohne Sortieren stehen die falschen Maße da.
#[test]
fn ohne_sortieren_stehen_falsche_masse() {
    let k = dim(vec![v(0.0, 0.0), v(2000.0, 0.0), v(1000.0, 0.0)]);
    assert_eq!(masszahlen(&k), vec!["2,00".to_string(), "1,00".to_string()]);
}

/// Ist die Reihenfolge schon richtig, ändert das Sortieren nichts.
#[test]
fn richtige_reihenfolge_bleibt_unveraendert() {
    let mut k = dim(vec![v(0.0, 0.0), v(1000.0, 0.0), v(2000.0, 0.0)]);
    assert!(!dim_reihenfolge(&mut k));
    assert_eq!(
        punkte(&k),
        vec![v(0.0, 0.0), v(1000.0, 0.0), v(2000.0, 0.0)]
    );
}

/// Ein Einzelmaß (zwei Punkte) behält seine Richtung: sortieren würde nur die
/// Pfeilrichtung umdrehen, nicht die Messung ändern.
#[test]
fn zwei_punkte_behalten_ihre_richtung() {
    let mut k = dim(vec![v(3000.0, 0.0), v(0.0, 0.0)]);
    assert!(!dim_reihenfolge(&mut k));
    assert_eq!(punkte(&k), vec![v(3000.0, 0.0), v(0.0, 0.0)]);
}

/// Verschieben, Drehen und Spiegeln halten die Messreihenfolge: Punkte wandern
/// gemeinsam, die Kette bleibt sortiert.
#[test]
fn bewegen_haelt_die_messreihenfolge() {
    // Absichtlich verdreht gespeichert.
    let e = Entity::new(
        1,
        1,
        dim(vec![v(0.0, 0.0), v(5000.0, 0.0), v(2000.0, 0.0)]),
    );
    for m in [
        Affine::translate(v(1000.0, 500.0)),
        Affine::rotate_about(v(0.0, 0.0), std::f64::consts::FRAC_PI_2),
        Affine::mirror(v(0.0, 0.0), v(0.0, 1000.0)),
    ] {
        let neu = transform::transform_entity(&e, &m);
        let p = punkte(&neu.kind);
        // Sortiert nach dem Anteil entlang der (mitgedrehten) Messrichtung.
        let EntityKind::Dim { angle, .. } = &neu.kind else {
            panic!("keine Maßkette mehr");
        };
        let dir = Vec2::polar(*angle, 1.0);
        let t: Vec<f64> = p.iter().map(|q| (*q - p[0]).dot(dir)).collect();
        assert!(
            t.windows(2).all(|w| w[0] <= w[1] + 1e-9),
            "nicht sortiert nach {m:?}: {p:?}"
        );
    }
}

/// Eine bereits verdreht gespeicherte Datei wird beim Laden sortiert (T0195) —
/// sonst bliebe der alte Fehler in jeder gespeicherten Zeichnung stehen.
#[test]
fn gespeicherte_kette_wird_beim_laden_sortiert() {
    let roh = json!({
        "format": "jjcad",
        "version": 2,
        "entities": [{
            "id": 1,
            "layer": 1,
            "kind": {"type": "dim",
                     "pts": [[0.0, 0.0], [5000.0, 0.0], [2000.0, 0.0]],
                     "line_pos": [0.0, -400.0], "angle": 0.0},
            "style": {}
        }],
        "layers": [{"id": 1, "name": "Bemaßung", "color": "#000000"}],
        "current_layer": 1
    });
    let doc = persist::from_json(&roh.to_string()).expect("Datei lesen");
    let e = doc.entity(1).expect("Maßkette fehlt");
    assert_eq!(
        punkte(&e.kind),
        vec![v(0.0, 0.0), v(2000.0, 0.0), v(5000.0, 0.0)],
        "die geladene Kette steht nicht in Messreihenfolge"
    );
}
