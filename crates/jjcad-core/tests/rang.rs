//! Zeichenrang (T0048): neu eingefügte Elemente ordnen sich von selbst ein.
//!
//! Die Liste `entities` ist die Zeichenreihenfolge (Index 0 = ganz hinten).
//! Jede Objektart hat einen Rang; ein neues Element landet unter allen
//! Objekten mit höherem Rang und über denen mit gleichem Rang. Bestehende
//! Objekte werden nie umsortiert — auch eine von Hand verdrehte Reihenfolge
//! des Nutzers bleibt erhalten.

use jjcad_core::model::{EntityKind, RANG_BAUTEIL, RANG_BESCHRIFTUNG, RANG_FLAECHE, RANG_LINIE};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

/// Ränge in Zeichenreihenfolge (hinten zuerst) als Buchstaben — `f` Fläche,
/// `v` Volumen, `w` Wand, `l` Linie, `t` Text.
fn stapel(s: &Session) -> String {
    s.doc
        .entities
        .iter()
        .map(|e| match e.kind.zeichenrang() {
            RANG_FLAECHE => 'f',
            1 => 'v',
            RANG_BAUTEIL => 'w',
            RANG_LINIE => 'l',
            RANG_BESCHRIFTUNG => 't',
            _ => '?',
        })
        .collect()
}

#[test]
fn rang_je_objektart() {
    assert_eq!(
        EntityKind::Fill {
            boundary: Vec::new(),
            holes: Vec::new(),
            pattern: Default::default(),
            pattern_angle: 0.0,
            pattern_scale: 100.0,
            border: false,
        }
        .zeichenrang(),
        RANG_FLAECHE
    );
    assert_eq!(
        EntityKind::Line {
            a: Default::default(),
            b: Default::default()
        }
        .zeichenrang(),
        RANG_LINIE
    );
    assert!(RANG_FLAECHE < RANG_BAUTEIL && RANG_BAUTEIL < RANG_LINIE);
    assert!(RANG_LINIE < RANG_BESCHRIFTUNG);
}

#[test]
fn flaeche_landet_hinter_wand_und_linie() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [4, 0], "thickness": 0.2}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [0, 1], "b": [4, 1]}))
        .unwrap();
    // Fläche zuletzt gezeichnet, obwohl sie oben im Stapel liegen müsste
    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [4, 0], [4, 3], [0, 3]]}),
    )
    .unwrap();

    assert_eq!(stapel(&s), "fwl", "Fläche hinter Wand und Linie");
    assert_eq!(s.doc.entities[0].zeichenrang(), RANG_FLAECHE);
}

#[test]
fn wand_landet_ueber_flaeche_und_unter_linie() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 1], "b": [4, 1]}))
        .unwrap();
    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [4, 0], [4, 3], [0, 3]]}),
    )
    .unwrap();
    // Wand als letztes Element: sie gehört über die Fläche, aber unter die Linie
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [4, 0], "thickness": 0.2}))
        .unwrap();

    assert_eq!(stapel(&s), "fwl", "Reihenfolge bleibt unabhängig vom Zeichnen");
}

#[test]
fn text_und_mass_liegen_vorn() {
    let mut s = s();
    s.execute("draw.text", &json!({"p": [1, 1], "text": "Raum", "height": 0.25}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]}))
        .unwrap();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [2, 0], "thickness": 0.2}))
        .unwrap();

    assert_eq!(stapel(&s), "wlt");
    assert_eq!(
        s.doc.entities.last().unwrap().zeichenrang(),
        RANG_BESCHRIFTUNG
    );
    let _ = s.execute(
        "dim.linear",
        &json!({"a": [0, 0], "b": [2, 0], "line_pos": [0, -0.5]}),
    );
    assert_eq!(
        s.doc.entities.last().unwrap().zeichenrang(),
        RANG_BESCHRIFTUNG,
        "neues Maß steht über Text und Linien"
    );
}

/// Die eigentliche Zusage: eine von Hand verdrehte Reihenfolge wird **nicht**
/// umsortiert. Nur das neue Element ordnet sich ein, alles andere behält
/// seinen Platz relativ zueinander.
#[test]
fn verdrehte_reihenfolge_bleibt_erhalten() {
    let mut s = s();
    for i in 0..3 {
        s.execute(
            "draw.line",
            &json!({"a": [0, i as f64], "b": [2, i as f64]}),
        )
        .unwrap();
    }
    let ids: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    // Nutzer schiebt die Linien wild durcheinander: 0 ganz nach vorn, 2 nach hinten
    s.execute("modify.order", &json!({"ids": [ids[0]], "where": "front"}))
        .unwrap();
    s.execute("modify.order", &json!({"ids": [ids[2]], "to": 1}))
        .unwrap();
    let vorher: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    assert_eq!(vorher, vec![ids[2], ids[1], ids[0]]);

    // Neue Linie: sie landet in ihrer Klasse, die drei alten bleiben in ihrer
    // Reihenfolge (relativ zueinander unverändert).
    s.execute("draw.line", &json!({"a": [0, 9], "b": [2, 9]}))
        .unwrap();
    let nachher: Vec<u64> = s.doc
        .entities
        .iter()
        .map(|e| e.id)
        .filter(|id| vorher.contains(id))
        .collect();
    assert_eq!(nachher, vorher, "bestehende Objekte bleiben in ihrer Reihenfolge");
    assert_eq!(s.doc.entities.len(), 4);
}

/// Auch bei verdrehter Reihenfolge gilt: nichts liegt über einer Linie, das
/// neu eingefügte Element ordnet sich hinter **allen** höheren Rängen ein.
#[test]
fn neue_flaeche_bleibt_auch_bei_verdrehter_reihenfolge_hinten() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]}))
        .unwrap();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [2, 0], "thickness": 0.2}))
        .unwrap();
    let ids: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    // Wand nach vorn (über die Linie) — verdreht
    s.execute("modify.order", &json!({"ids": [ids[1]], "where": "front"}))
        .unwrap();

    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [2, 0], [2, 1], [0, 1]]}),
    )
    .unwrap();
    assert_eq!(s.doc.entities[0].zeichenrang(), RANG_FLAECHE);
    assert_eq!(
        s.doc.entities.last().unwrap().id,
        ids[1],
        "die verdrehte Wand bleibt vorn"
    );
}

/// Kopien sind neue Elemente und ordnen sich ebenfalls nach Rang ein — der
/// kopierte Text bleibt damit über der kopierten Fläche.
#[test]
fn kopie_ordnet_sich_nach_rang_ein() {
    let mut s = s();
    s.execute("draw.text", &json!({"p": [1, 1], "text": "A", "height": 0.25}))
        .unwrap();
    s.execute(
        "draw.fill",
        &json!({"boundary": [[0, 0], [2, 0], [2, 2], [0, 2]]}),
    )
    .unwrap();
    let ids: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    assert_eq!(stapel(&s), "ft");

    s.execute(
        "modify.copy",
        &json!({"ids": ids, "from": [0, 0], "to": [5, 0]}),
    )
    .unwrap();
    assert_eq!(stapel(&s), "fftt", "Kopien ordnen sich nach Rang ein");
}

/// Das eigene Panel/der Farbeimer ermittelt die Umgrenzung; die Fläche muss
/// trotzdem hinter der Wand liegen (nicht davor, nur weil sie später kommt).
#[test]
fn farbeimerflaeche_bleibt_hinter_der_umgrenzung() {
    let mut s = s();
    for (a, b) in [
        ([0.0, 0.0], [4.0, 0.0]),
        ([4.0, 0.0], [4.0, 3.0]),
        ([4.0, 3.0], [0.0, 3.0]),
        ([0.0, 3.0], [0.0, 0.0]),
    ] {
        s.execute("draw.wall", &json!({"a": a, "b": b, "thickness": 0.2}))
            .unwrap();
    }
    let res = s.execute("draw.fill.paint", &json!({"p": [2, 1.5]}));
    assert!(res.is_ok(), "Farbeimer: {res:?}");
    assert_eq!(stapel(&s), "fwwww", "Fläche hinter allen Wänden");
}
