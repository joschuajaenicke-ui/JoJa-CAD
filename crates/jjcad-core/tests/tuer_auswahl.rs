//! Trefferflächen von Tür und Fenster (T0071).
//!
//! Tür und Fenster zeichnen **versetzt** zu ihren gespeicherten Werten: die
//! Tür sitzt auf der vorderen Wandkante (Flügel senkrecht zur Wand,
//! Schwungbogen daneben), das Fenster füllt die Wanddicke. Die Trefferflächen
//! kamen früher aus der gespeicherten Achse (`center ± Breite/2`) — diese
//! Strecke liegt unsichtbar in der Wand, der gezeichnete Türflügel ließ sich
//! deshalb weder anklicken noch mit der Auswahl anwählen.
//!
//! Geprüft wird über `render::entity_segments`/`snap::hit_test` — derselbe Weg,
//! den Klick und Fensterauswahl nehmen.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    Session::new()
}

fn last_id(s: &Session) -> Id {
    s.doc.entities.last().unwrap().id
}

/// Wand (0|0)→(5|0), 300 mm dick; Tür 900 mm breit in der Mitte.
fn tuer_zeichnung() -> (Session, Id) {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}),
    )
    .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [2, 0], "width": 0.9, "host": [wall], "base": "center"}),
    )
    .unwrap();
    let door = last_id(&s);
    (s, door)
}

#[test]
fn tuerfluegel_ist_anklickbar() {
    let (s, door) = tuer_zeichnung();
    // Flügel: Scharnier bei (1550|150) auf der vorderen Wandkante, Blatt
    // senkrecht zur Wand bis y = 1050 mm.
    let (scharnier, blattende) = match s.doc.entity(door).unwrap().kind.clone() {
        EntityKind::Door {
            center,
            width,
            hinge,
            swing_in,
            angle,
            ..
        } => {
            let dir = Vec2::polar(angle, 1.0);
            let nrm = dir.perp() * if swing_in { 1.0 } else { -1.0 };
            let scharnier = center - dir * (hinge * width * 0.5);
            (scharnier, scharnier + nrm * width)
        }
        _ => panic!("keine Tür"),
    };
    assert!((scharnier.x - 1550.0).abs() < 1.0, "Scharnier x={}", scharnier.x);
    assert!((scharnier.y - 150.0).abs() < 1.0, "Scharnier y={}", scharnier.y);
    assert!((blattende.y - 1050.0).abs() < 1.0, "Blattende y={}", blattende.y);

    // Mitte des Flügels — vorher lag hier nichts vom Objekt (`center ± 450`
    // auf der Achse, also y = 150).
    let mitte = (scharnier + blattende) * 0.5;
    let treffer = jjcad_core::snap::hit_test(&s.doc, mitte, 100.0, None);
    assert_eq!(treffer, Some(door), "Klick auf den Türflügel muss die Tür treffen");

    // Knapp daneben (300 mm) ist nichts.
    let daneben = mitte + v(300.0, 0.0);
    assert_ne!(
        jjcad_core::snap::hit_test(&s.doc, daneben, 100.0, None),
        Some(door),
        "300 mm neben dem Flügel darf die Tür nicht treffen"
    );
}

#[test]
fn schwungbogen_ist_anklickbar() {
    let (s, door) = tuer_zeichnung();
    // Bogen um das Scharnier (1550|150) mit r = 900; 45° liegt zwischen
    // Wandachse und Blatt und ist nur über den Bogen erreichbar.
    let p = v(1550.0 + 900.0 * std::f64::consts::FRAC_PI_4.cos(), 150.0 + 900.0 * std::f64::consts::FRAC_PI_4.sin());
    let treffer = jjcad_core::snap::hit_test(&s.doc, p, 100.0, None);
    assert_eq!(treffer, Some(door), "Klick auf den Schwungbogen muss die Tür treffen");
}

#[test]
fn fensterrahmen_ist_in_der_ganzen_wanddicke_anklickbar() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}),
    )
    .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.window",
        &json!({"center": [2.5, 0], "width": 1.2, "host": [wall]}),
    )
    .unwrap();
    let fenster = last_id(&s);

    // Rahmenkante am Wandrand (y = 150 mm) — das Fenster reicht über die
    // ganze Wanddicke und muss dort vor der Wand getroffen werden.
    let treffer = jjcad_core::snap::hit_test(&s.doc, v(2500.0, 150.0), 60.0, None);
    assert_eq!(treffer, Some(fenster), "Rahmenkante muss das Fenster treffen");
}
