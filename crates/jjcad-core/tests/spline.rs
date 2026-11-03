//! Spline über die Befehls-Schicht (T0005): Catmull-Rom-Kurve durch die
//! geklickten Stützpunkte, Eingabe wie bei der Polylinie.
//!
//! Belegt wird das, was der Nutzer nachprüfen kann: die Action legt ein
//! Spline-Objekt an, die gezeichnete Kurve läuft durch jeden Stützpunkt, sie
//! ist wirklich gekrümmt (nicht der Polygonzug), Fang und Auswahl finden sie,
//! und Speichern/Laden erhält sie.

use jjcad_core::geom::{catmull_rom_points, Spline, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::persist;
use jjcad_core::render;
use jjcad_core::session::Session;
use jjcad_core::snap;
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .unwrap();
    s
}

fn spline(s: &Session) -> (Vec<Vec2>, bool) {
    let e = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Spline { .. }))
        .expect("kein Spline angelegt");
    match &e.kind {
        EntityKind::Spline { verts, closed } => (verts.iter().map(|v| v.p).collect(), *closed),
        _ => unreachable!(),
    }
}

/// Die Action ist registriert (Ribbon, Inventar, MCP lesen aus dem Register).
#[test]
fn aktion_ist_im_register() {
    let a = jjcad_core::actions::action("draw.spline").expect("draw.spline fehlt im Register");
    assert_eq!(a.label(), "Spline");
    assert!(a.tool, "Spline ist ein interaktives Werkzeug");
}

/// Punktfolge wie bei der Polylinie: offener Zug bleibt offen, Stützpunkte
/// werden gespeichert.
#[test]
fn legt_offenen_spline_an() {
    let mut s = session();
    s.execute(
        "draw.spline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000]]}),
    )
    .unwrap();
    let (pts, closed) = spline(&s);
    assert!(!closed);
    assert_eq!(pts.len(), 3);
    assert_eq!(pts[1], Vec2::new(2000.0, 0.0));
}

/// Klick auf den Startpunkt schließt — ohne doppelten Stützpunkt (dieselbe
/// Regel wie bei der Polylinie).
#[test]
fn schlusspunkt_auf_startpunkt_schliesst() {
    let mut s = session();
    s.execute(
        "draw.spline",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 0]]}),
    )
    .unwrap();
    let (pts, closed) = spline(&s);
    assert!(closed, "letzter Punkt ist der erste → geschlossen");
    assert_eq!(pts.len(), 3, "kein doppelter Endpunkt im Modell");
}

/// Die Kurve läuft durch alle Stützpunkte — das ist die Zusage des Splines.
#[test]
fn kurve_laeuft_durch_alle_stuetzpunkte() {
    let pts = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(1000.0, 800.0),
        Vec2::new(2000.0, 0.0),
        Vec2::new(3000.0, 800.0),
    ];
    let kurve = catmull_rom_points(&pts, false, 0.05);
    for p in &pts {
        let d = kurve
            .iter()
            .map(|q| q.dist(*p))
            .fold(f64::INFINITY, f64::min);
        assert!(d < 1e-6, "Stützpunkt {p:?} liegt {d} mm neben der Kurve");
    }
    assert_eq!(kurve.first().copied(), Some(pts[0]));
    assert!(
        kurve.last().map(|q| q.dist(pts[3]) < 1e-6).unwrap_or(false),
        "die Kurve endet am letzten Stützpunkt"
    );
}

/// Zwischen zwei Stützpunkten ist die Kurve gekrümmt — Abstand zur Sehne ist
/// deutlich größer null (sonst wäre es ein Polygonzug).
#[test]
fn kurve_ist_gekruemmt_und_nicht_der_polygonzug() {
    let pts = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(1000.0, 0.0),
        Vec2::new(2000.0, 0.0),
        Vec2::new(3000.0, 0.0),
        Vec2::new(4000.0, 0.0),
    ];
    // Eine reine Gerade bleibt gerade (kein Überschwingen bei Kollinearität).
    let gerade = catmull_rom_points(&pts, false, 0.05);
    for q in &gerade {
        assert!(q.y.abs() < 1e-6, "kollineare Punkte bleiben auf der Achse");
    }
    // Mit einem Versatz in der Mitte wölbt sich die Kurve.
    let pts = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(1000.0, 0.0),
        Vec2::new(2000.0, 500.0),
        Vec2::new(3000.0, 0.0),
        Vec2::new(4000.0, 0.0),
    ];
    let kurve = catmull_rom_points(&pts, false, 0.05);
    // Die Kurve weicht sichtbar von der Sehne ab (nicht nur Rauschen).
    let max_y = kurve.iter().map(|q| q.y).fold(f64::NEG_INFINITY, f64::max);
    assert!(max_y > 100.0, "Kurve wölbt sich, max y = {max_y}");
}

/// Länge und Fläche: der geschlossene Zug umschließt eine Fläche, der offene
/// nicht; die Länge ist länger als die Sehne (Kurve).
#[test]
fn laenge_und_flaeche() {
    // Geschlossener Zug: ein regelmäßiges Zwölfeck um einen Kreis mit
    // r = 1000 mm. Die Kurve läuft durch die Ecken und schließt die Fläche —
    // erwartet wird die Kreisfläche (π·r² ≈ 3,14 m²), nicht die Vielecksfläche
    // (≈ 3,0 m²): dazwischen liegt die Wölbung, die den Spline ausmacht.
    let pts: Vec<Vec2> = (0..12)
        .map(|i| {
            let a = i as f64 / 12.0 * std::f64::consts::TAU;
            Vec2::new(1000.0 * a.cos(), 1000.0 * a.sin())
        })
        .collect();
    let sp = Spline::new(pts.clone(), true);
    let a = sp.area();
    let kreis = std::f64::consts::PI * 1000.0 * 1000.0;
    assert!(
        (a - kreis).abs() / kreis < 0.02,
        "Fläche nahe der Kreisfläche {kreis:.0} mm², ist {a:.0} mm²"
    );
    let offen = Spline::new(pts, false);
    assert_eq!(offen.area(), 0.0, "ein offener Zug umschließt nichts");
    let gerade = Spline::new(vec![Vec2::new(0.0, 0.0), Vec2::new(1000.0, 0.0)], false);
    assert!((gerade.len() - 1000.0).abs() < 1e-6);
}

/// Fang und Trefferprüfung finden die Kurve (sie laufen über
/// `render::entity_segments` — dieselbe Zerlegung wie die Anzeige).
#[test]
fn fang_und_treffer_finden_die_kurve() {
    let mut s = session();
    s.execute(
        "draw.spline",
        &json!({"pts": [[0, 0], [1000, 0], [2000, 0], [3000, 0]]}),
    )
    .unwrap();
    let e = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Spline { .. }))
        .cloned()
        .unwrap();
    let segs = render::entity_segments(&s.doc, &e);
    assert!(!segs.is_empty(), "die Kurve liefert Strecken für Fang/Auswahl");
    // Ein Punkt auf der Kurve (Mitte) ist mit kleinem Radius treffbar.
    let treffer = snap::hit_test(&s.doc, Vec2::new(1500.0, 0.0), 2.0, None);
    assert_eq!(treffer, Some(e.id), "Treffer auf der Kurve");
    let leer = snap::hit_test(&s.doc, Vec2::new(1500.0, 900.0), 2.0, None);
    assert_eq!(leer, None, "neben der Kurve kein Treffer");
}

/// Speichern und Laden erhält den Spline samt Stützpunkten und Closed-Flag.
#[test]
fn speichern_und_laden() {
    let mut s = session();
    s.execute(
        "draw.spline",
        &json!({"pts": [[0, 0], [1000, 500], [2000, 0]], "closed": false}),
    )
    .unwrap();
    let json_text = persist::to_json(&s.doc).unwrap();
    assert!(
        json_text.contains("\"spline\""),
        "der Typ steht als spline in der Datei"
    );
    let doc2 = persist::from_json(&json_text).unwrap();
    let e = doc2
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Spline { .. }))
        .expect("Spline nach dem Laden vorhanden");
    match &e.kind {
        EntityKind::Spline { verts, closed } => {
            assert_eq!(verts.len(), 3);
            assert!(!closed);
            assert_eq!(verts[1].p, Vec2::new(1000.0, 500.0));
        }
        _ => unreachable!(),
    }
}

/// Verschieben wirkt auf alle Stützpunkte (die Kurve wandert mit).
#[test]
fn verschieben_bewegt_die_kurve() {
    let mut s = session();
    let r = s
        .execute(
            "draw.spline",
            &json!({"pts": [[0, 0], [1000, 500], [2000, 0]]}),
        )
        .unwrap();
    let id = r["created"][0].as_u64().unwrap() as u64;
    let (_pts, _) = spline(&s); // Existenz prüfen
    s.execute(
        "modify.move",
        &json!({"ids": [id], "from": [0, 0], "to": [1000, 0]}),
    )
    .unwrap();
    let (pts, _) = spline(&s);
    assert_eq!(pts[0], Vec2::new(1000.0, 0.0));
    assert_eq!(pts[1], Vec2::new(2000.0, 500.0));
    assert_eq!(pts[2], Vec2::new(3000.0, 0.0));
}
