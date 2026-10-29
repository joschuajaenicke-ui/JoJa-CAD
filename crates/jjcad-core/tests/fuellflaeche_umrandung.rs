//! Abnahme der umschaltbaren Umrandung für Füllflächen (Schraffuren).
//!
//! Geprüft wird, was der Nutzer in der App sieht:
//!
//! 1. Die Vorgabe hängt am Erstellmodus: Punkte setzen an, Kontur anklicken
//!    und Farbeimer aus.
//! 2. `modify.set {"border": …}` schaltet die Umrandung am Objekt um — das ist
//!    der Weg, den die Checkbox im Eigenschaften-Panel nimmt.
//! 3. Der Schalter steuert das Rendering: nur mit `border = true` steht eine
//!    Randlinie in der Displayliste (Bildschirm, PDF und PNG lesen dieselbe).
//! 4. Der Zustand überlebt Speichern und Laden.

use jjcad_core::geom::Vec2;
use jjcad_core::model::EntityKind;
use jjcad_core::render::{self, DisplayList, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::{persist, style::FillPattern};
use serde_json::json;

/// Eine Fläche als Punktliste in Metern (so gibt es die GUI weiter).
fn kontur() -> serde_json::Value {
    json!([[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]])
}

/// Liest (Muster, Umrandung) aus dem einzigen Objekt des Dokuments.
fn umrandung(s: &Session) -> bool {
    match &s.doc.entities[0].kind {
        EntityKind::Fill { border, .. } => *border,
        other => panic!("keine Füllfläche, sondern {}", other.type_name()),
    }
}

fn ecken(s: &Session) -> Vec<Vec2> {
    match &s.doc.entities[0].kind {
        EntityKind::Fill { boundary, .. } => boundary.clone(),
        other => panic!("keine Füllfläche, sondern {}", other.type_name()),
    }
}

/// Anzahl der geschlossenen Polygonzüge in der Displayliste — das ist die
/// gezeichnete Umrandung (Außenring plus Inselkanten).
fn randlinien(s: &Session) -> usize {
    let e = s.doc.entities[0].clone();
    let mut dl = DisplayList::default();
    render::render_entity(
        &RenderCtx::new(&s.doc),
        &e,
        jjcad_core::geom::Affine::IDENTITY,
        &mut dl,
        0,
    );
    dl.prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Poly(_, true)))
        .count()
}

// ---------------------------------------------------------------------------
// 1. Vorgabe je Erstellmodus
// ---------------------------------------------------------------------------

#[test]
fn punkte_setzen_hat_umrandung_an() {
    for action in ["draw.fill.points", "draw.fill.boundary"] {
        let mut s = Session::new();
        let pts = kontur();
        let params = if action == "draw.fill.points" {
            json!({ "pts": pts })
        } else {
            json!({ "boundary": pts })
        };
        s.execute(action, &params)
            .unwrap_or_else(|e| panic!("{action} muss eine Fläche anlegen: {e}"));

        let ecken = ecken(&s);
        assert!(umrandung(&s), "{action}: Umrandung ist an");
        assert_eq!(
            (ecken[1] - ecken[0]).len(),
            2000.0,
            "{action}: Kontur in mm übernommen"
        );
        assert!(
            randlinien(&s) >= 1,
            "{action}: Randlinie fehlt in der Displayliste"
        );
    }
}

#[test]
fn kontur_anklicken_hat_umrandung_aus() {
    let mut s = Session::new();
    s.execute("draw.fill", &json!({ "boundary": kontur() }))
        .expect("Kontur anklicken muss eine Fläche anlegen");

    assert!(!umrandung(&s), "Kontur anklicken: Umrandung ist aus");
    assert_eq!(randlinien(&s), 0, "keine Randlinie in der Displayliste");
}

/// Der alte Name der Schraffur läuft über denselben Pfad wie „Kontur
/// anklicken" — also ebenfalls ohne Umrandung.
#[test]
fn alter_name_verhaelt_sich_wie_kontur_anklicken() {
    let mut s = Session::new();
    s.execute("draw.hatch", &json!({ "boundary": kontur() }))
        .expect("draw.hatch muss eine Fläche anlegen");
    assert!(!umrandung(&s), "draw.hatch: Umrandung ist aus");
}

#[test]
fn farbeimer_hat_umrandung_aus() {
    // Geschlossener Kasten aus vier Linien, dann in die Mitte klicken.
    let mut s = Session::new();
    for (a, b) in [
        ([0.0, 0.0], [4.0, 0.0]),
        ([4.0, 0.0], [4.0, 3.0]),
        ([4.0, 3.0], [0.0, 3.0]),
        ([0.0, 3.0], [0.0, 0.0]),
    ] {
        s.execute("draw.line", &json!({ "a": a, "b": b }))
            .expect("Rahmen zeichnen");
    }
    let vorher = s.doc.entities.len();
    let r = s
        .execute("draw.fill.paint", &json!({ "p": [2.0, 1.5] }))
        .expect("Farbeimer muss eine Fläche anlegen");

    assert_eq!(s.doc.entities.len(), vorher + 1, "genau eine Fläche dazu");
    // Nicht das letzte Objekt: der Farbeimer legt seine Fläche unter die
    // Umgrenzung (t_9cd971c5), also an den Anfang der Zeichenreihenfolge.
    let id = r["created"][0].as_u64().expect("ID der Fläche");
    let e = s.doc.entity(id).expect("Fläche im Dokument");
    let EntityKind::Fill { border, .. } = &e.kind else {
        panic!("keine Füllfläche, sondern {}", e.kind.type_name());
    };
    assert!(!border, "Farbeimer: Umrandung ist aus");
}

/// Ein ausdrücklich gesetztes `border` sticht die Vorgabe des Modus — die
/// Checkbox im Platzieren-Panel arbeitet so.
#[test]
fn ausdrueckliches_border_sticht_die_vorgabe() {
    let mut s = Session::new();
    s.execute("draw.fill", &json!({ "boundary": kontur(), "border": true }))
        .expect("Kontur anklicken mit Umrandung");
    assert!(umrandung(&s), "border:true bei Kontur anklicken wirkt");

    let mut t = Session::new();
    t.execute(
        "draw.fill.points",
        &json!({ "pts": kontur(), "border": false }),
    )
    .expect("Punktfolge ohne Umrandung");
    assert!(!umrandung(&t), "border:false beim Punkte setzen wirkt");
}

// ---------------------------------------------------------------------------
// 2. Umschalten am Objekt (Weg der Checkbox) und Rendering
// ---------------------------------------------------------------------------

#[test]
fn umschalten_aendert_die_darstellung_sofort() {
    let mut s = Session::new();
    s.execute("draw.fill", &json!({ "boundary": kontur(), "pattern": "lines" }))
        .expect("Fläche anlegen");
    let id = s.doc.entities[0].id;

    assert_eq!(randlinien(&s), 0, "startet ohne Umrandung");

    s.execute("modify.set", &json!({ "ids": [id], "border": true }))
        .expect("Umrandung einschalten");
    assert!(umrandung(&s), "Objekt trägt die Umrandung");
    assert_eq!(randlinien(&s), 1, "genau eine Randlinie");

    s.execute("modify.set", &json!({ "ids": [id], "border": false }))
        .expect("Umrandung ausschalten");
    assert!(!umrandung(&s), "Objekt ohne Umrandung");
    assert_eq!(randlinien(&s), 0, "Randlinie wieder weg");

    // Das Muster bleibt in beiden Fällen stehen — der Schalter betrifft nur
    // die Umrandung, nicht die Füllung.
    let EntityKind::Fill { pattern, .. } = &s.doc.entities[0].kind else {
        panic!("keine Füllfläche");
    };
    assert_eq!(*pattern, FillPattern::Lines, "Muster unverändert");
}

/// Mit Insel: die Umrandung umfasst Außenring **und** Inselkante.
#[test]
fn umrandung_umfasst_auch_die_insel() {
    let mut s = Session::new();
    s.execute(
        "draw.fill.boundary",
        &json!({
            "boundary": [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
            "holes": [[[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]]],
            "border": true,
        }),
    )
    .expect("Fläche mit Insel");
    assert_eq!(randlinien(&s), 2, "Außenring plus Inselkante");
}

// ---------------------------------------------------------------------------
// 3. Speichern und Laden
// ---------------------------------------------------------------------------

#[test]
fn umrandung_ueberlebt_speichern_und_laden() {
    let mut s = Session::new();
    s.execute("draw.fill.points", &json!({ "pts": kontur() }))
        .expect("Fläche mit Umrandung");
    s.execute("draw.fill", &json!({ "boundary": kontur() }))
        .expect("Fläche ohne Umrandung");
    assert!(umrandung(&s), "erste Fläche mit Umrandung");
    let ids: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 2);

    let datei = std::env::temp_dir().join("jjcad_umrandung_test.jjcad");
    persist::save_document(&mut s.doc, &datei).expect("speichern");
    let roh = std::fs::read_to_string(&datei).expect("Datei lesen");
    let wieder = persist::load_document(&datei).expect("erneut laden");
    let _ = std::fs::remove_file(&datei);

    // In der Datei steht der Zustand als Feld am Objekt.
    assert!(roh.contains("\"border\": true"), "border:true in der Datei");
    assert!(roh.contains("\"border\": false"), "border:false in der Datei");

    // Und nach dem Laden gilt er unverändert je Objekt.
    let werte: Vec<bool> = wieder
        .entities
        .iter()
        .map(|e| match &e.kind {
            EntityKind::Fill { border, .. } => *border,
            other => panic!("keine Füllfläche, sondern {}", other.type_name()),
        })
        .collect();
    assert_eq!(werte, vec![true, false], "Zustand je Objekt wiederhergestellt");
}

/// Pixelmaß im PNG: ohne Umrandung bleibt der Rand der Fläche hell, mit
/// Umrandung steht dort eine dunkle Linie. Bildschirm, PDF und PNG lesen
/// dieselbe Displayliste — der Beleg hier deckt damit auch die Anzeige ab.
#[test]
fn umrandung_ist_im_png_als_linie_messbar() {
    use jjcad_core::export::RasterOptions;
    use jjcad_core::raster::render_model_pixmap;

    let mut s = Session::new();
    // Vollfläche ohne gesetzte Füllfarbe: die Fläche wird aufgehellt gezeichnet,
    // die Umrandung in der Linienfarbe (Schwarz) — messbar dunkel.
    // Ebene ausdrücklich auf „Wände“: gemessen wird die dunkle Umrandung.
    // Ohne Angabe läge die Fläche seit T0039 auf „Schraffur“ (hellgrau,
    // 0,13 mm) — die Linie wäre da, aber nicht mehr als dunkler Pixel zu
    // finden.
    s.execute(
        "draw.fill",
        &json!({ "boundary": [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]], "pattern": "solid",
                 "layer": "Wände" }),
    )
    .expect("Fläche anlegen");
    let id = s.doc.entities[0].id;
    let opts = RasterOptions {
        width_px: 400,
        height_px: 200,
        paper: false,
        ..Default::default()
    };

    let dunkel = |doc: &jjcad_core::model::Document| -> usize {
        let pm = render_model_pixmap(doc, &opts).expect("PNG rendern");
        pm.pixels()
            .iter()
            .filter(|p| {
                let l = 0.299 * p.red() as f64 + 0.587 * p.green() as f64 + 0.114 * p.blue() as f64;
                l < 100.0
            })
            .count()
    };

    let ohne = dunkel(&s.doc);
    s.execute("modify.set", &json!({ "ids": [id], "border": true }))
        .expect("Umrandung einschalten");
    let mit = dunkel(&s.doc);

    println!("dunkle Pixel: ohne Umrandung {ohne}, mit Umrandung {mit}");
    assert!(ohne < 50, "ohne Umrandung keine dunklen Pixel, gefunden: {ohne}");
    assert!(
        mit > 400,
        "mit Umrandung muss eine Linie sichtbar sein, dunkle Pixel: {mit}"
    );
}
