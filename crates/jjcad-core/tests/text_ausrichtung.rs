//! Ausrichtung von Textfeldern (Ticket T0028).
//!
//! Zwei Eigenschaften bestimmen die Lage eines Textes gegenüber seinem
//! **Einfügepunkt**: waagerecht Linksbündig/Zentriert/Rechtsbündig, senkrecht
//! Oben/Mitte/Unten — neun Fälle. Der Einfügepunkt ist der Klickpunkt beim
//! Anlegen und der Basispunkt beim Kopieren; ein „Zentriert/Mitte\" gesetzter
//! Text liegt damit genau in der Mitte seines Punktes.
//!
//! Geprüft wird über `render::text_corners` — denselben Kasten, den Anzeige,
//! Rahmen, Fang und Vorschau benutzen. Alle Koordinaten in Millimetern.

use jjcad_core::geom::Vec2;
use jjcad_core::model::{EntityKind, TextAlignH, TextAlignV};
use jjcad_core::render;
use jjcad_core::session::Session;
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s
}

fn text(s: &Session) -> (Vec2, TextAlignH, TextAlignV, f64) {
    let e = s.doc.ents().last().expect("Text fehlt");
    let EntityKind::Text {
        p,
        align,
        valign,
        height,
        ..
    } = &e.kind
    else {
        panic!("kein Text");
    };
    (*p, *align, *valign, *height)
}

/// Kasten des letzten Textes als (min, max) — über `render::text_frame`,
/// denselben Weg wie Anzeige und Fang.
fn kasten(s: &Session) -> (Vec2, Vec2) {
    let e = s.doc.ents().last().unwrap();
    let ecken = render::text_frame(&s.doc, e).expect("Text ohne Rahmen");
    let min = Vec2::new(
        ecken.iter().map(|q| q.x).fold(f64::INFINITY, f64::min),
        ecken.iter().map(|q| q.y).fold(f64::INFINITY, f64::min),
    );
    let max = Vec2::new(
        ecken.iter().map(|q| q.x).fold(f64::NEG_INFINITY, f64::max),
        ecken.iter().map(|q| q.y).fold(f64::NEG_INFINITY, f64::max),
    );
    (min, max)
}

fn nahe(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// `draw.text` nimmt beide Ausrichtungen an — und der Kasten hängt am
/// Einfügepunkt, wie die Ausrichtung es sagt.
#[test]
fn neue_texte_haengen_an_ihrem_einfuegepunkt() {
    let pfad = [
        ("left", "top", -1.0, 1.0),
        ("center", "middle", 0.0, 0.0),
        ("right", "bottom", 1.0, -1.0),
    ];
    for (align, valign, fx, fy) in pfad {
        let mut s = session();
        let p = Vec2::new(1000.0, 500.0);
        s.execute(
            "draw.text",
            &json!({"p": [p.x, p.y], "text": "Wohnen", "height": 250.0,
                    "align": align, "valign": valign}),
        )
        .expect("Text anlegen");
        let (pp, a, w, _) = text(&s);
        assert_eq!(pp, p, "Einfügepunkt ist der Klickpunkt");
        assert_eq!(a.key(), align, "waagerechte Ausrichtung");
        assert_eq!(w.key(), valign, "senkrechte Ausrichtung");
        let (min, max) = kasten(&s);
        // Der Punkt liegt in der Ecke/in der Mitte/in der anderen Ecke.
        let erwartet = Vec2::new(
            if fx < 0.0 {
                min.x
            } else if fx > 0.0 {
                max.x
            } else {
                (min.x + max.x) / 2.0
            },
            if fy > 0.0 {
                max.y
            } else if fy < 0.0 {
                min.y
            } else {
                (min.y + max.y) / 2.0
            },
        );
        assert!(
            nahe(erwartet.x, p.x) && nahe(erwartet.y, p.y),
            "{align}/{valign}: Kasten {min:?}…{max:?} hängt nicht am Punkt {p:?}"
        );
    }
}

/// Zentriert und Mittig: der Punkt ist die **Mitte** des Textes — genau das,
/// was beim Kopieren als Basispunkt gilt.
#[test]
fn zentriert_mittig_liegt_auf_dem_punkt() {
    let mut s = session();
    s.execute(
        "draw.text",
        &json!({"p": [400.0, 300.0], "text": "Wohnen", "height": 250.0,
                "align": "center", "valign": "middle"}),
    )
    .unwrap();
    let (min, max) = kasten(&s);
    assert!(nahe((min.x + max.x) / 2.0, 400.0), "waagerecht mittig");
    assert!(nahe((min.y + max.y) / 2.0, 300.0), "senkrecht mittig");
}

/// Oben und Unten spiegeln sich um den Einfügepunkt: der Text hängt bzw. steht
/// über ihm (dieselbe Lage, die die Anzeige zeichnet).
#[test]
fn oben_haengt_der_text_unter_dem_punkt() {
    let mut oben = session();
    oben.execute(
        "draw.text",
        &json!({"p": [0.0, 0.0], "text": "Wohnen", "height": 250.0, "valign": "top"}),
    )
    .unwrap();
    let (min_o, max_o) = kasten(&oben);
    assert!(nahe(max_o.y, 0.0), "obere Kante am Punkt");
    assert!(min_o.y < -1.0, "Text liegt darunter");

    let mut unten = session();
    unten.execute(
        "draw.text",
        &json!({"p": [0.0, 0.0], "text": "Wohnen", "height": 250.0, "valign": "bottom"}),
    )
    .unwrap();
    let (min_u, max_u) = kasten(&unten);
    assert!(nahe(min_u.y, 0.0), "untere Kante am Punkt");
    assert!(max_u.y > 1.0, "Text liegt darüber");
}

/// Die Eigenschaften ändern beide Werte nachträglich — am vorhandenen Text,
/// ohne den Einfügepunkt zu verschieben.
#[test]
fn aenderung_ueber_die_eigenschaften() {
    let mut s = session();
    s.execute(
        "draw.text",
        &json!({"p": [0.0, 0.0], "text": "Wohnen", "height": 250.0}),
    )
    .unwrap();
    let id = s.doc.ents().last().unwrap().id;
    let (_, vorher_max) = kasten(&s);

    s.execute(
        "modify.set",
        &json!({"ids": [id], "align": "center", "valign": "bottom"}),
    )
    .expect("Ausrichtung setzen");
    let (p, a, w, _) = text(&s);
    assert_eq!((a, w), (TextAlignH::Center, TextAlignV::Bottom));
    assert_eq!(p, Vec2::new(0.0, 0.0), "Einfügepunkt bleibt");
    let (min, max) = kasten(&s);
    assert!(nahe((min.x + max.x) / 2.0, 0.0), "jetzt waagerecht mittig");
    assert!(nahe(min.y, 0.0), "jetzt über dem Punkt");
    assert!(max.y > vorher_max.y, "der Text wandert nach oben");
}

/// Ausrichtung bleibt beim Speichern und Laden erhalten.
#[test]
fn ausrichtung_ueberlebt_das_speichern() {
    let mut s = session();
    s.execute(
        "draw.text",
        &json!({"p": [100.0, 200.0], "text": "Wohnen", "height": 250.0,
                "align": "right", "valign": "top"}),
    )
    .unwrap();
    let json_text = jjcad_core::persist::to_json(&s.doc).expect("speichern");
    // In der Datei stehen die Variantennamen (wie bei allen Textfeldern seit
    // T0025, siehe `samples/`): `align` waagerecht, `valign` senkrecht.
    assert!(
        json_text.contains("\"align\": \"Right\"") && json_text.contains("\"valign\": \"Top\""),
        "Ausrichtung fehlt in der Datei"
    );
    let doc = jjcad_core::persist::from_json(&json_text).expect("laden");
    let e = doc.ents().last().unwrap();
    let EntityKind::Text { align, valign, .. } = &e.kind else {
        panic!("kein Text");
    };
    assert_eq!((*align, *valign), (TextAlignH::Right, TextAlignV::Top));
}

/// `align` gilt für Wände **und** Texte — die Textausrichtung darf die
/// Wandausrichtung nicht überschreiben (gleicher Schlüssel, verschiedene Typen).
#[test]
fn wandausrichtung_bleibt_wandausrichtung() {
    let mut s = session();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [4000, 0], "thickness": 240.0}),
    )
    .unwrap();
    let id = s.doc.ents().last().unwrap().id;
    s.execute("modify.set", &json!({"ids": [id], "align": "left"}))
        .unwrap();
    let e = s.doc.ents().last().unwrap();
    let EntityKind::Wall { align, .. } = &e.kind else {
        panic!("keine Wand");
    };
    assert_eq!(*align, jjcad_core::style::WallAlign::Left);
}
