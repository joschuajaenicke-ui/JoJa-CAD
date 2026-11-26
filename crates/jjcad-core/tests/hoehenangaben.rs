//! T0253: Höhenangaben unter einer Maßkette.
//!
//! Jede Längenangabe (jeder Abschnitt) kann eine Höhenangabe tragen. Sie steht
//! **unter** der Maßkette, wird als einfaches Textfeld gezeichnet und lässt
//! sich im Programm per Doppelklick wieder ändern. Geprüft wird über die
//! Befehlsschicht und die Anzeige — der Weg, den Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::Vec2;
use jjcad_core::model::{Document, EntityKind, TextFont};
use jjcad_core::persist;
use jjcad_core::render::{self, render_document, RenderCtx, Shape};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut d = Document::new();
    d.settings.input_unit = Unit::Mm;
    d
}

/// Waagerechte Kette über drei Punkte; die Maßlinie liegt 400 mm unter der
/// Messstrecke.
fn kette(d: &mut Document) -> u64 {
    let r = actions::execute(
        d,
        "dim.chain",
        &json!({
            "pts": [[0.0, 0.0], [3000.0, 0.0], [5000.0, 0.0]],
            "line_pos": [0.0, -400.0],
            "angle": 0.0
        }),
    )
    .expect("Maßkette anlegen");
    r["created"][0].as_u64().expect("Kennung")
}

fn hoehen(d: &Document, id: u64) -> Vec<String> {
    match &d.entity(id).expect("Maßkette").kind {
        EntityKind::Dim { heights, .. } => heights.clone(),
        k => panic!("keine Maßkette: {k:?}"),
    }
}

/// Alle gezeichneten Texte als (Text, Ankerpunkt).
fn texte(d: &Document) -> Vec<(String, Vec2)> {
    render_document(&RenderCtx::new(d))
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) => Some((t.text.clone(), t.pos)),
            _ => None,
        })
        .collect()
}

#[test]
fn hoehenangabe_haengt_am_abschnitt() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [id], "index": 1, "text": "+3,00"}),
    )
    .expect("Höhenangabe setzen");
    assert_eq!(hoehen(&d, id), vec!["", "+3,00"]);
    // Ein leerer Text nimmt die Angabe zurück.
    actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [id], "index": 1, "text": ""}),
    )
    .expect("Höhenangabe löschen");
    assert_eq!(hoehen(&d, id), vec!["", ""]);
}

#[test]
fn hoehenangabe_ueberlebt_das_speichern() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [id], "index": 0, "text": "+1,20"}),
    )
    .expect("Höhenangabe setzen");
    let text = persist::to_json(&d).expect("serialisieren");
    assert!(text.contains("\"heights\""), "Feld fehlt in der Datei");
    let wieder = persist::from_json(&text).expect("lesen");
    assert_eq!(hoehen(&wieder, id), vec!["+1,20", ""]);
}

#[test]
fn abschnitt_ohne_angabe_hat_kein_feld_in_der_datei() {
    let mut d = doc_mm();
    kette(&mut d);
    let text = persist::to_json(&d).expect("serialisieren");
    assert!(
        !text.contains("\"heights\""),
        "leere Höhenangaben gehören nicht in die Datei"
    );
}

#[test]
fn hoehenangabe_steht_unter_der_masskette() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [id], "index": 1, "text": "H1"}),
    )
    .expect("Höhenangabe setzen");
    let texte = texte(&d);
    let (_, h_pos) = texte
        .iter()
        .find(|(t, _)| t == "H1")
        .expect("Höhenangabe wird gezeichnet");
    // Mitte des zweiten Abschnitts (3000 … 5000), unter der Maßlinie (-400).
    assert!((h_pos.x - 4000.0).abs() < 1.0, "x = {}", h_pos.x);
    assert!(h_pos.y < -400.0, "Höhenangabe steht nicht unter der Kette");
    // Die Maßzahl desselben Abschnitts steht auf der anderen Seite.
    let mass = texte
        .iter()
        .find(|(t, p)| (*p - Vec2::new(4000.0, -400.0)).x.abs() < 1.0 && *p != *h_pos)
        .map(|(_, p)| *p);
    if let Some(m) = mass {
        assert!(m.y > -400.0, "Maßzahl steht nicht über der Maßlinie");
        assert!(m.y > h_pos.y, "Höhe muss unter der Maßzahl stehen");
    }
}

#[test]
fn hoehenangabe_wird_getroffen_und_masskette_bleibt_treffbar() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [id], "index": 1, "text": "H1"}),
    )
    .expect("Höhenangabe setzen");
    let (_, h_pos) = texte(&d)
        .into_iter()
        .find(|(t, _)| t == "H1")
        .expect("Höhenangabe wird gezeichnet");
    let layer = d.entity(id).expect("Maßkette").layer;
    let text_h = d.dim_style.text_height * d.plan_scale_of_layer(layer);
    let (_, box_h) = render::text_box_mit("H1", &[], TextFont::DinMittel, text_h, 1.0);
    // Der Kasten hängt an der Oberkante des Ankers (valign Top).
    let drin = Vec2::new(h_pos.x, h_pos.y - box_h * 0.5);
    assert_eq!(
        render::dim_hoehe_unter_punkt(&d, drin),
        Some((id, 1)),
        "Doppelklick findet die Höhenangabe nicht"
    );
    // Weit daneben trifft sie nicht.
    assert_eq!(
        render::dim_hoehe_unter_punkt(&d, Vec2::new(4000.0, 5000.0)),
        None
    );
}

#[test]
fn der_klick_im_werkzeug_findet_den_abschnitt() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    let kind = d.entity(id).expect("Maßkette").kind.clone();
    assert_eq!(
        render::dim_abschnitt_bei(&kind, Vec2::new(4200.0, -200.0)),
        Some(1)
    );
    assert_eq!(
        render::dim_abschnitt_bei(&kind, Vec2::new(500.0, 3000.0)),
        Some(0)
    );
}

#[test]
fn hoehenangabe_wandert_mit_der_kette() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [id], "index": 0, "text": "+1,20"}),
    )
    .expect("Höhenangabe setzen");
    let vorher = texte(&d)
        .into_iter()
        .find(|(t, _)| t == "+1,20")
        .expect("Angabe im Bild")
        .1;
    actions::execute(
        &mut d,
        "modify.move",
        &json!({"ids": [id], "dx": 1000.0, "dy": 0.0}),
    )
    .expect("Kette verschieben");
    let nachher = texte(&d)
        .into_iter()
        .find(|(t, _)| t == "+1,20")
        .expect("Angabe im Bild")
        .1;
    assert!(
        (nachher.x - vorher.x - 1000.0).abs() < 1.0,
        "Höhenangabe wandert nicht mit: {vorher:?} → {nachher:?}"
    );
}

#[test]
fn ein_punkt_weniger_nimmt_eine_angabe_mit() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    for (i, t) in ["A", "B"].iter().enumerate() {
        actions::execute(
            &mut d,
            "dim.height.set",
            &json!({"ids": [id], "index": i, "text": t}),
        )
        .expect("Höhenangabe setzen");
    }
    assert_eq!(hoehen(&d, id), vec!["A", "B"]);
    actions::execute(
        &mut d,
        "dim.point.remove",
        &json!({"ids": [id], "index": 1}),
    )
    .expect("Messpunkt entfernen");
    assert_eq!(hoehen(&d, id), vec!["A"]);
}

#[test]
fn unbekannter_abschnitt_ist_ein_fehler() {
    let mut d = doc_mm();
    let id = kette(&mut d);
    let r = actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [id], "index": 5, "text": "X"}),
    );
    assert!(r.is_err(), "Abschnitt 5 gibt es nicht");
    let r = actions::execute(
        &mut d,
        "dim.height.set",
        &json!({"ids": [999], "index": 0, "text": "X"}),
    );
    assert!(r.is_err(), "unbekanntes Objekt");
}
