//! Raumstempel (T0097): unsichtbare Fläche mit verknüpfter Beschriftung.
//!
//! Geprüft wird die ganze Kette ohne GUI:
//! 1. `dim.room` legt aus einer Punktfolge einen Stempel an und setzt die
//!    Beschriftung in den „Pole of Inaccessibility" (größter Abstand zu allen
//!    Kanten),
//! 2. die Fläche ist **nicht** anklickbar, das Textfeld schon,
//! 3. eine geänderte Ecke aktualisiert den Text (er ist abgeleitet, nicht
//!    gespeichert),
//! 4. die Textlage lässt sich verschieben, ohne die Fläche zu bewegen.

use jjcad_core::area::{area_of, pole_of_inaccessibility};
use jjcad_core::geom::{polygon_area, Vec2};
use jjcad_core::model::{Entity, EntityKind};
use jjcad_core::render;
use jjcad_core::session::Session;
use jjcad_core::snap;
use serde_json::json;

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn raum_mit_id(s: &Session) -> (u64, Vec<Vec2>, Vec2, String) {
    let e: &Entity = s.doc.entities.first().expect("kein Objekt");
    match &e.kind {
        EntityKind::Room {
            boundary,
            label,
            heading,
            ..
        } => (e.id, boundary.clone(), *label, heading.clone()),
        other => panic!("kein Raumstempel, sondern {}", other.type_name()),
    }
}

/// Der sichtbare Text des Stempels, wie ihn die Anzeige baut.
fn angezeigter_text(s: &Session, id: u64) -> String {
    let e = s.doc.entity(id).expect("Objekt fehlt");
    let ctx = render::RenderCtx::for_entity(&s.doc, e);
    let mut dl = render::DisplayList::default();
    render::render_entity(&ctx, e, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
    dl.prims
        .iter()
        .find_map(|p| match &p.shape {
            render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Vier Punkte ergeben einen Stempel: unsichtbare Fläche, Überschrift und
/// Flächeninhalt im Text, Text in der Flächenmitte.
#[test]
fn punktfolge_legt_raumstempel_an() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let antwort = s
        .execute(
            "dim.room",
            &json!({ "pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]] }),
        )
        .expect("Raumstempel anlegen");

    assert_eq!(s.doc.entities.len(), 1, "genau ein Objekt");
    let id = antwort["created"][0].as_u64().unwrap();
    let (_, boundary, label, heading) = raum_mit_id(&s);
    assert_eq!(boundary.len(), 4, "vier Ecken");
    assert_eq!(heading, "Zimmer", "Überschrift");
    // 4 m × 3 m = 12 m²; die Mitte ist der einzige Punkt mit maximalem
    // Abstand zu allen Kanten.
    assert!(
        (area_of(&boundary, &[]) / 1e6 - 12.0).abs() < 1e-6,
        "Fläche {}",
        area_of(&boundary, &[]) / 1e6
    );
    assert!(label.dist(v(2000.0, 1500.0)) < 1.0, "Textlage {label:?}");
    let text = angezeigter_text(&s, id);
    assert!(text.starts_with("Zimmer\n"), "Text ohne Überschrift: {text:?}");
    assert!(text.contains("12,00 m²"), "Flächeninhalt fehlt: {text:?}");
}

/// Die Fläche ist unsichtbar und nicht anklickbar — getroffen wird nur das
/// Textfeld (Punktprobe an einer Ecke, in der Fläche und auf dem Text).
#[test]
fn flaeche_ist_nicht_anklickbar_das_textfeld_schon() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let antwort = s
        .execute(
            "dim.room",
            &json!({ "pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]] }),
        )
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();

    // Ecke der Umgrenzung und eine Stelle im Raum: kein Treffer.
    assert_eq!(
        snap::hit_test(&s.doc, v(0.0, 0.0), 20.0, None),
        None,
        "die unsichtbare Fläche darf nicht getroffen werden"
    );
    assert_eq!(
        snap::hit_test(&s.doc, v(3500.0, 2500.0), 20.0, None),
        None,
        "auch innen trifft die Fläche nicht"
    );
    // Das Textfeld (Mitte) ist die Trefferfläche.
    assert_eq!(
        snap::hit_test(&s.doc, v(2000.0, 1500.0), 20.0, None),
        Some(id),
        "das Textfeld muss anklickbar sein"
    );
}

/// Eine gezogene Ecke ändert die Fläche, der Text folgt ohne Nachziehen.
#[test]
fn geaenderte_ecke_aktualisiert_den_text() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let antwort = s
        .execute(
            "dim.room",
            &json!({ "pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]] }),
        )
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();

    // Ecke bei (4 m | 3 m) auf (8 m | 3 m) ziehen: Fläche wird 8 m × 3 m.
    s.execute(
        "modify.set",
        &json!({ "ids": [id], "boundary": [[0.0, 0.0], [8.0, 0.0], [8.0, 3.0], [0.0, 3.0]] }),
    )
    .expect("Umgrenzung ändern");

    let text = angezeigter_text(&s, id);
    assert!(text.contains("24,00 m²"), "Text nicht nachgezogen: {text:?}");
}

/// Textlage verschiebt nur die Beschriftung — die Fläche bleibt stehen.
#[test]
fn textlage_laesst_sich_ohne_flaeche_verschieben() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let antwort = s
        .execute(
            "dim.room",
            &json!({ "pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]] }),
        )
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let (_, vorher, _, _) = raum_mit_id(&s);

    s.execute("modify.set", &json!({ "ids": [id], "label": [1.0, 1.0] }))
        .expect("Textlage setzen");

    let (_, nachher, label, _) = raum_mit_id(&s);
    assert_eq!(vorher, nachher, "die Fläche darf sich nicht verschieben");
    assert!(label.dist(v(1000.0, 1000.0)) < 1e-6, "Textlage {label:?}");
}

/// Ein sauberes Rechteck: der Pole of Inaccessibility liegt in der Mitte,
/// nicht am Schwerpunkt einer Ecke.
#[test]
fn pol_rechteck_liegt_in_der_mitte() {
    let ring = vec![v(0.0, 0.0), v(6000.0, 0.0), v(6000.0, 2000.0), v(0.0, 2000.0)];
    let p = pole_of_inaccessibility(&ring, &[], 0.5);
    assert!(p.dist(v(3000.0, 1000.0)) < 1.0, "Pol {p:?}");
}

/// Raum mit schmalem Flur: der Pol liegt im breiten Teil, nicht im Flur —
/// dort ist der Abstand zu den Kanten größer.
#[test]
fn pol_weicht_dem_schmalen_flur_aus() {
    // 6 m × 4 m Raum, rechts ein 1 m breiter Flur bis 10 m.
    let ring = vec![
        v(0.0, 0.0),
        v(6000.0, 0.0),
        v(6000.0, 1500.0),
        v(10000.0, 1500.0),
        v(10000.0, 2500.0),
        v(6000.0, 2500.0),
        v(6000.0, 4000.0),
        v(0.0, 4000.0),
    ];
    let p = pole_of_inaccessibility(&ring, &[], 0.5);
    assert!(
        p.x < 6000.0,
        "Pol {p:?} liegt im schmalen Flur statt im Raum"
    );
    // Die Fläche ist 2 m von den Kanten entfernt (halbe Raumhöhe/Raumbreite).
    assert!(
        jjcad_core::area::signed_distance(p, &ring, &[]) >= 1900.0,
        "Pol {p:?} liegt zu nah an einer Kante"
    );
}

/// Eine Umgrenzung ohne Fläche ist ein Fehler (T0038), kein Stempel im
/// Ursprung.
#[test]
fn zu_wenige_punkte_sind_ein_fehler() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let e = s.execute("dim.room", &json!({ "pts": [[0.0, 0.0], [1.0, 0.0]] }));
    assert!(e.is_err(), "zwei Punkte dürfen keinen Stempel ergeben");
    assert!(s.doc.entities.is_empty(), "es wurde trotzdem etwas angelegt");
}

/// Die Fläche zählt als Fläche (Panel, Abfrage) und die Polylinien-Rechnung
/// stimmt mit der Umgrenzung überein.
#[test]
fn flaeche_steht_in_der_objektabfrage() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    s.execute(
        "dim.room",
        &json!({ "pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]] }),
    )
    .unwrap();
    let e = s.doc.entities.first().unwrap();
    let flaeche = jjcad_core::area::entity_area(e).expect("Fläche fehlt");
    assert!((flaeche / 1e6 - 12.0).abs() < 1e-6, "Fläche {}", flaeche / 1e6);
    assert!(
        (polygon_area(&[v(0.0, 0.0), v(4000.0, 0.0), v(4000.0, 3000.0), v(0.0, 3000.0)]).abs()
            - flaeche)
            .abs()
            < 1e-6
    );
}

/// Die Datei trägt die Umgrenzung, die Textlage und die Überschrift — nach dem
/// Laden steht derselbe Stempel wieder da.
#[test]
fn stempel_ueberlebt_speichern_und_laden() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .unwrap();
    let antwort = s
        .execute(
            "dim.room",
            &json!({ "pts": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
                    "heading": "Küche" }),
        )
        .unwrap();
    let id = antwort["created"][0].as_u64().unwrap();
    let json_text = jjcad_core::persist::to_json(&s.doc).expect("schreiben");
    assert!(json_text.contains("\"type\": \"room\""), "kein room in der Datei");
    assert!(json_text.contains("\"heading\": \"Küche\""), "Überschrift fehlt");

    let geladen = jjcad_core::persist::from_json(&json_text).expect("laden");
    let e = geladen.entity(id).expect("Stempel fehlt nach dem Laden");
    match &e.kind {
        EntityKind::Room {
            boundary,
            heading,
            ..
        } => {
            assert_eq!(boundary.len(), 4);
            assert_eq!(heading, "Küche");
        }
        k => panic!("kein Raumstempel: {}", k.type_name()),
    }
    let text = {
        let ctx = render::RenderCtx::for_entity(&geladen, e);
        let mut dl = render::DisplayList::default();
        render::render_entity(&ctx, e, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
        dl.prims
            .iter()
            .find_map(|p| match &p.shape {
                render::Shape::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    assert!(text.contains("12,00 m²"), "Fläche nach dem Laden: {text:?}");
}
