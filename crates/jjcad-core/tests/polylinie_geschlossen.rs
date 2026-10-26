//! Geschlossene Polylinie über die Befehls-Schicht (Karte t_8b6feebf).
//!
//! Zwei Wege müssen dasselbe Ergebnis liefern: der ausdrückliche Parameter
//! `closed` und die Erkennung „der letzte Punkt ist der erste". In beiden Fällen
//! gibt es **einen** Stützpunkt weniger (kein doppelter Endpunkt) und das
//! Closed-Flag steht — auch nach dem Speichern und Laden.

use jjcad_core::geom::Vec2;
use jjcad_core::model::EntityKind;
use jjcad_core::persist;
use jjcad_core::session::Session;
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .unwrap();
    s
}

fn polylinie(s: &Session) -> (Vec<Vec2>, bool) {
    let e = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Polyline { .. }))
        .expect("keine Polylinie angelegt");
    match &e.kind {
        EntityKind::Polyline { verts, closed } => (verts.iter().map(|v| v.p).collect(), *closed),
        _ => unreachable!(),
    }
}

/// Schlusspunkt auf dem Startpunkt (ohne `closed`): geschlossen, ohne doppelten
/// Punkt — drei Ecken für ein Dreieck, nicht vier.
#[test]
fn schlusspunkt_auf_startpunkt_schliesst_ohne_doppelten_punkt() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 0]]}),
    )
    .unwrap();
    let (pts, closed) = polylinie(&s);
    assert!(closed, "letzter Punkt ist der erste → geschlossen");
    assert_eq!(pts.len(), 3, "der doppelte Endpunkt wird nicht gespeichert");
    assert_eq!(pts[0], Vec2::new(0.0, 0.0));
    assert_eq!(pts[2], Vec2::new(2000.0, 1000.0));
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        assert!(
            pts[i].dist(pts[j]) > 1.0,
            "keine Strecke der Länge null zwischen {i} und {j}"
        );
    }
}

/// Ausdrückliches `closed` mit doppeltem Endpunkt: derselbe Zustand.
#[test]
fn closed_parameter_entfernt_doppelten_endpunkt() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 0]], "closed": true}),
    )
    .unwrap();
    let (pts, closed) = polylinie(&s);
    assert!(closed);
    assert_eq!(pts.len(), 3, "kein doppelter Endpunkt");
}

/// Offene Polylinie bleibt offen und behält alle Punkte.
#[test]
fn offene_polylinie_bleibt_offen() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [500, 1500]]}),
    )
    .unwrap();
    let (pts, closed) = polylinie(&s);
    assert!(!closed, "Endpunkte fallen auseinander → offen");
    assert_eq!(pts.len(), 4);
}

/// Zwei Klicks auf dieselbe Stelle erzeugen keinen Stützpunkt doppelt.
#[test]
fn doppelter_zwischenpunkt_entfaellt() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [1000, 0], [1000, 0], [1000, 1000]]}),
    )
    .unwrap();
    let (pts, closed) = polylinie(&s);
    assert_eq!(pts.len(), 3, "der wiederholte Punkt fällt weg");
    assert!(!closed);
}

/// Zwei Punkte können keine Fläche umschließen: das Closed-Flag fällt weg,
/// statt eine entartete Polylinie zu erzeugen.
#[test]
fn zwei_punkte_bleiben_offen() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [1000, 0]], "closed": true}),
    )
    .unwrap();
    let (pts, closed) = polylinie(&s);
    assert!(!closed);
    assert_eq!(pts.len(), 2);
}

/// Das Closed-Flag übersteht Speichern und Laden; die Punktzahl bleibt gleich
/// (kein doppelter Endpunkt in der Datei).
#[test]
fn geschlossen_bleibt_nach_speichern_und_laden() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 0]]}),
    )
    .unwrap();
    let json_text = persist::to_json(&s.doc).unwrap();
    let geladen = persist::from_json(&json_text).unwrap();
    let e = geladen
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Polyline { .. }))
        .expect("Polylinie fehlt nach dem Laden");
    match &e.kind {
        EntityKind::Polyline { verts, closed } => {
            assert!(*closed, "Closed-Flag geht beim Laden verloren");
            assert_eq!(verts.len(), 3, "der doppelte Endpunkt steht in der Datei");
        }
        _ => unreachable!(),
    }
    // Gegenprobe: eine offene Polylinie bleibt offen.
    let mut s2 = session();
    s2.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [1000, 0], [1000, 1000]]}),
    )
    .unwrap();
    let geladen2 = persist::from_json(&persist::to_json(&s2.doc).unwrap()).unwrap();
    assert!(
        matches!(geladen2.entities[0].kind, EntityKind::Polyline { closed: false, .. }),
        "offene Polylinie wird beim Laden geschlossen"
    );
}

/// Die Darstellung hat die Schlusskante: drei Strecken für ein Dreieck, die
/// letzte endet am Startpunkt. Ein offener Zug hat nur zwei.
#[test]
fn geschlossene_polylinie_hat_schlusskante() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 0]]}),
    )
    .unwrap();
    let e = &s.doc.entities[0];
    let segs = jjcad_core::render::entity_segments(&s.doc, e);
    assert_eq!(segs.len(), 3, "Schlusskante fehlt in der Displayliste");
    let letzte = segs.last().unwrap();
    assert_eq!(letzte.start(), Vec2::new(2000.0, 1000.0));
    assert_eq!(letzte.end(), Vec2::new(0.0, 0.0));

    let mut s2 = session();
    s2.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000]]}),
    )
    .unwrap();
    let segs2 = jjcad_core::render::entity_segments(&s2.doc, &s2.doc.entities[0]);
    assert_eq!(segs2.len(), 2, "offene Polylinie bekommt keine Schlusskante");
}

/// Geschlossene Fläche wird berechnet (Eigenschaften-Panel, Flächenangabe).
#[test]
fn geschlossene_polylinie_hat_flaeche() {
    let mut s = session();
    s.execute(
        "draw.polyline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 0]]}),
    )
    .unwrap();
    let plt = jjcad_core::geom::Polyline {
        verts: match &s.doc.entities[0].kind {
            EntityKind::Polyline { verts, .. } => verts.clone(),
            _ => unreachable!(),
        },
        closed: true,
    };
    assert!((plt.area() - 1_000_000.0).abs() < 1.0, "Fläche ist {0}", plt.area());
}
