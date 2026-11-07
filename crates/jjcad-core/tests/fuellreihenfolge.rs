//! Zeichenreihenfolge der Farbeimer-Füllfläche (Karte t_9cd971c5).
//!
//! Die Liste `Document::entities` **ist** die Zeichenreihenfolge: Index 0 liegt
//! ganz hinten, das letzte Element ganz vorn. Eine Füllfläche, die wie jedes
//! andere neue Objekt angehängt wird, überdeckt deshalb die Linien ihrer
//! eigenen Umgrenzung — der Rand des Raums verschwindet unter der Farbe.
//!
//! Geprüft wird die ganze Kette: Action `draw.fill.paint` → Position in
//! `Document::entities` → Reihenfolge der Displayliste → sichtbares Pixel.

use jjcad_core::export::RasterOptions;
use jjcad_core::model::{Document, EntityKind, Id};
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use serde_json::json;

// ---------------------------------------------------------------------------
// Hilfen
// ---------------------------------------------------------------------------

fn leinwand() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .expect("Eingabe in Metern");
    // Die Randlinien brauchen eine sichtbare Stärke, damit das Pixelmaß
    // „Linie über der Füllung“ trägt. Seit T0125 ist die Programm-Vorgabe für
    // alle Ebenen 0,18 mm — hier ausdrücklich die alte 0,50 mm setzen, damit
    // der Schwellwert des Tests die Zeichenfolge misst und nicht die Vorgabe.
    if let Some(l) = s.doc.layers.iter_mut().find(|l| l.name == "Wände") {
        l.line_weight = 0.50;
    }
    s
}

/// Linie von `a` nach `b` in mm; liefert die neue Objekt-ID.
fn linie(s: &mut Session, a: (f64, f64), b: (f64, f64)) -> Id {
    let r = s
        .execute(
            "draw.line",
            // Ebene ausdrücklich auf „Wände“: die Randlinien müssen dunkel
            // sein, damit das Pixelmaß „Linie über der Füllung“ trägt. Ohne
            // Angabe läge eine Linie seit T0039 auf „Hilfslinien“ (hell,
            // gestrichelt).
            &json!({"a": [a.0 / 1000.0, a.1 / 1000.0], "b": [b.0 / 1000.0, b.1 / 1000.0],
                    "layer": "Wände"}),
        )
        .expect("Linie");
    r["created"][0].as_u64().expect("ID der Linie") as Id
}

/// Geschlossenes Rechteck aus vier Linien, achsenparallel; liefert die IDs in
/// Zeichenreihenfolge.
fn rechteck(s: &mut Session, ecken: &[(f64, f64)]) -> Vec<Id> {
    (0..ecken.len())
        .map(|i| linie(s, ecken[i], ecken[(i + 1) % ecken.len()]))
        .collect()
}

fn raum(ecken: &[(f64, f64)]) -> Vec<Id> {
    let mut s = leinwand();
    let ids = rechteck(&mut s, ecken);
    ids
}

fn ist_flaeche(e: &jjcad_core::model::Entity) -> bool {
    matches!(e.kind, EntityKind::Fill { .. })
}

fn flaeche_id(s: &Session) -> Id {
    s.doc
        .entities
        .iter()
        .find(|e| ist_flaeche(e))
        .expect("Füllfläche fehlt")
        .id
}

fn reihenfolge(s: &Session) -> Vec<Id> {
    s.doc.entities.iter().map(|e| e.id).collect()
}

/// Objekte einer Gruppe in der Reihenfolge, in der sie im Dokument stehen —
/// damit lässt sich prüfen, dass sich ihre relative Lage nicht verschoben hat.
fn relative_lage(s: &Session, gruppe: &[Id]) -> Vec<Id> {
    reihenfolge(s)
        .into_iter()
        .filter(|id| gruppe.contains(id))
        .collect()
}

/// Position des Objekts in der Zeichenreihenfolge (0 = ganz hinten).
fn position(s: &Session, id: Id) -> Option<usize> {
    s.doc.entities.iter().position(|e| e.id == id)
}

const ECKEN: [(f64, f64); 4] = [(0.0, 0.0), (4000.0, 0.0), (4000.0, 3000.0), (0.0, 3000.0)];

/// Rechteck + Füllung über den Farbeimer; liefert Session und Linien-IDs.
fn gefuellter_raum() -> (Session, Vec<Id>) {
    let mut s = leinwand();
    let rand = rechteck(&mut s, &ECKEN);
    s.execute(
        "draw.fill.paint",
        &json!({"p": [2.0, 1.5], "pattern": "solid", "fill": "#ff0000"}),
    )
    .expect("Farbeimer");
    (s, rand)
}

// ---------------------------------------------------------------------------
// 1. Zeichenreihenfolge in der Objektliste
// ---------------------------------------------------------------------------

#[test]
fn fuellflaeche_steht_unter_der_umrandung() {
    let (s, rand) = gefuellter_raum();
    let flaeche = flaeche_id(&s);

    // Direkt hinter der ersten Linie der Umgrenzung — nicht dahinter, sondern
    // davor in der Liste, also „unter“ ihr im Bild.
    let erste = position(&s, rand[0]).expect("Linie");
    let pos_flaeche = position(&s, flaeche).expect("Fläche");
    assert!(
        pos_flaeche < erste,
        "Füllfläche steht bei {pos_flaeche}, die erste Randlinie bei {erste}"
    );
    assert_eq!(
        pos_flaeche, 0,
        "die Fläche gehört direkt hinter die erste Umgrenzung: {:?}",
        reihenfolge(&s)
    );

    // Jede Randlinie liegt über der Fläche.
    for id in &rand {
        let p = position(&s, *id).expect("Linie");
        assert!(p > pos_flaeche, "Randlinie {id} liegt unter der Füllung");
    }
}

#[test]
fn uebrige_objekte_behalten_ihre_relative_reihenfolge() {
    // Raum, danach noch ein Kreis und ein Text — Objekte, die mit der
    // Umgrenzung nichts zu tun haben.
    let mut s = leinwand();
    let rand = rechteck(&mut s, &ECKEN);
    let kreis = s
        .execute("draw.circle", &json!({"c": [5.0, 5.0], "r": 0.5}))
        .expect("Kreis")["created"][0]
        .as_u64()
        .unwrap() as Id;
    let text = s
        .execute("draw.text", &json!({"p": [6.0, 5.0], "text": "Hinweis"}))
        .expect("Text")["created"][0]
        .as_u64()
        .unwrap() as Id;
    let vorher = reihenfolge(&s);

    s.execute(
        "draw.fill.paint",
        &json!({"p": [2.0, 1.5], "pattern": "solid", "fill": "#ff0000"}),
    )
    .expect("Farbeimer");

    assert_eq!(
        relative_lage(&s, &rand),
        rand,
        "die Randlinien haben ihre Reihenfolge gewechselt"
    );
    assert_eq!(
        relative_lage(&s, &[kreis, text]),
        vec![kreis, text],
        "fremde Objekte haben ihre Reihenfolge gewechselt"
    );
    // Nur ein Objekt kam dazu, sonst ist nichts verschwunden.
    assert_eq!(s.doc.entities.len(), vorher.len() + 1);
    let flaeche = flaeche_id(&s);
    assert!(position(&s, flaeche).unwrap() < position(&s, rand[0]).unwrap());
}

// ---------------------------------------------------------------------------
// 2. Mehrere und verschachtelte Umrandungen
// ---------------------------------------------------------------------------

#[test]
fn zwei_raeume_werden_jeweils_eigen_unterfuellt() {
    let mut s = leinwand();
    let links = rechteck(&mut s, &ECKEN);
    let rechts = rechteck(
        &mut s,
        &[
            (5000.0, 0.0),
            (9000.0, 0.0),
            (9000.0, 3000.0),
            (5000.0, 3000.0),
        ],
    );

    for klick in [[2.0, 1.5], [7.0, 1.5]] {
        s.execute(
            "draw.fill.paint",
            &json!({"p": klick, "pattern": "solid", "fill": "#ff0000"}),
        )
        .expect("Farbeimer");
    }

    let flaechen: Vec<Id> = s
        .doc
        .entities
        .iter()
        .filter(|e| ist_flaeche(e))
        .map(|e| e.id)
        .collect();
    assert_eq!(flaechen.len(), 2, "zwei Füllungen erwartet");

    for (i, rand) in [&links, &rechts].iter().enumerate() {
        let f = flaechen[i];
        for id in rand.iter() {
            assert!(
                position(&s, f).unwrap() < position(&s, *id).unwrap(),
                "Füllung {i} liegt über ihrer Randlinie {id}"
            );
        }
        assert_eq!(relative_lage(&s, rand), **rand, "Randlinien verschoben");
    }
    // Die erste Füllung bleibt hinter der zweiten, die Randlinien von links
    // bleiben vor denen von rechts.
    assert!(position(&s, flaechen[0]).unwrap() < position(&s, flaechen[1]).unwrap());
    assert!(
        position(&s, links[3]).unwrap() < position(&s, rechts[0]).unwrap(),
        "die beiden Räume haben sich gegenseitig durchmischt"
    );
}

#[test]
fn verschachtelte_umrandung_landet_unter_beiden_rahmen() {
    // Raum im Raum: außen 0…6000, innen 2000…4000. Gefüllt wird der Ring
    // dazwischen — die Innenkanten sind Inseln der Fläche.
    let mut s = leinwand();
    let aussen = rechteck(
        &mut s,
        &[
            (0.0, 0.0),
            (6000.0, 0.0),
            (6000.0, 6000.0),
            (0.0, 6000.0),
        ],
    );
    let innen = rechteck(
        &mut s,
        &[
            (2000.0, 2000.0),
            (4000.0, 2000.0),
            (4000.0, 4000.0),
            (2000.0, 4000.0),
        ],
    );

    s.execute(
        "draw.fill.paint",
        &json!({"p": [1.0, 3.0], "pattern": "solid", "fill": "#ff0000"}),
    )
    .expect("Farbeimer im Ring");

    let flaeche = flaeche_id(&s);
    for id in aussen.iter().chain(innen.iter()) {
        assert!(
            position(&s, flaeche).unwrap() < position(&s, *id).unwrap(),
            "Füllung liegt über der Umrandung {id}"
        );
    }
    // Der innere Rahmen bleibt als Insel erhalten und steht ebenfalls darüber.
    let holes = match &s.doc.entity(flaeche).unwrap().kind {
        EntityKind::Fill { holes, .. } => holes.len(),
        _ => unreachable!(),
    };
    assert_eq!(holes, 1, "Innenrahmen muss als Insel in der Fläche stehen");
}

#[test]
fn erneutes_fuellen_bleibt_unter_der_umrandung() {
    let (mut s, rand) = gefuellter_raum();
    s.execute(
        "draw.fill.paint",
        &json!({"p": [2.0, 1.5], "pattern": "solid", "fill": "#0000ff"}),
    )
    .expect("zweiter Farbeimer");

    let flaechen: Vec<Id> = s
        .doc
        .entities
        .iter()
        .filter(|e| ist_flaeche(e))
        .map(|e| e.id)
        .collect();
    assert_eq!(flaechen.len(), 2);
    for f in &flaechen {
        for id in &rand {
            assert!(
                position(&s, *f).unwrap() < position(&s, *id).unwrap(),
                "Füllung {f} liegt über der Randlinie {id}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Displayliste: die Fläche wird vor ihren Linien gezeichnet
// ---------------------------------------------------------------------------

#[test]
fn displayliste_zeichnet_die_umrandung_nach_der_flaeche() {
    let (s, rand) = gefuellter_raum();
    let flaeche = flaeche_id(&s);
    let ctx = RenderCtx::new(&s.doc);
    let dl = render_document(&ctx);

    let (i_flaeche, linien): (usize, Vec<usize>) = {
        let mut i_f = None;
        let mut linien = Vec::new();
        for (i, prim) in dl.prims.iter().enumerate() {
            if prim.entity == flaeche {
                if matches!(prim.shape, Shape::Fill { .. }) && i_f.is_none() {
                    i_f = Some(i);
                }
            } else if rand.contains(&prim.entity) && matches!(prim.shape, Shape::Line(..)) {
                linien.push(i);
            }
        }
        (i_f.expect("Füllfläche fehlt in der Displayliste"), linien)
    };

    assert_eq!(linien.len(), 4, "vier Randlinien erwartet");
    for i in linien {
        assert!(
            i_flaeche < i,
            "Randlinie bei {i} wird vor der Füllung bei {i_flaeche} gezeichnet"
        );
    }
}

#[test]
fn leinwand_zeigt_die_umrandung_auf_der_gefuellten_flaeche() {
    // Pixelmaß statt Augenmaß: die Randlinie (dunkel) muss über der roten
    // Füllung liegen. Vor dem Fix überdeckte die Fläche die Linie vollständig.
    let (s, _rand) = gefuellter_raum();
    let opts = RasterOptions {
        width_px: 660,
        height_px: 510,
        bbox: Some([-200.0, -200.0, 4200.0, 3200.0]),
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let pm = render_model_pixmap(&s.doc, &opts).expect("Bild");

    let (mut dunkel, mut rot) = (0usize, 0usize);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            let Some(px) = pm.pixel(x, y) else { continue };
            let (r, g, b) = (px.red(), px.green(), px.blue());
            if r < 80 && g < 80 && b < 80 {
                dunkel += 1;
            }
            if r > 200 && g < 60 && b < 60 {
                rot += 1;
            }
        }
    }
    // Die vier Kanten sind zusammen 14 m lang; bei 0,139 px/mm und 0,5 mm
    // Linienstärke sind das rund 13.600 dunkle Pixel. Steht die Füllung über
    // den Linien, bleiben nur die Ränder außerhalb der Fläche übrig (~5.000).
    assert!(rot > 10_000, "die Fläche ist nicht gefüllt: {rot} rote Pixel");
    assert!(
        dunkel > 9_000,
        "die Umrandung ist unsichtbar: nur {dunkel} dunkle Pixel — die Füllung überdeckt sie"
    );
}

#[test]
fn wandraum_im_wohnhaus_behaelt_den_wandrahmen() {
    // Echter Plan: Raum unten links, Umrandung sind Wandkörper. Die Wandlinien
    // müssen nach der Füllung gezeichnet werden.
    let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/Wohnhaus.jjcad");
    let doc: Document = jjcad_core::persist::load_document(&pfad).expect("Wohnhaus.jjcad");
    let mut s = Session::with_doc(doc);
    let grenze = jjcad_core::boundary::detect_boundary(
        &s.doc,
        jjcad_core::geom::Vec2::new(1500.0, 2600.0),
        &jjcad_core::boundary::BoundaryOpts::default(),
    )
    .expect("Umgrenzung");
    assert!(
        !grenze.border_entities.is_empty(),
        "die Umgrenzung muss ihre Objekte kennen"
    );

    s.execute(
        "draw.fill.paint",
        &json!({"p": [1.5, 2.6], "pattern": "solid", "fill": "#ff0000"}),
    )
    .expect("Raum füllen");

    let flaeche = flaeche_id(&s);
    for e in &grenze.border_entities {
        // Randobjekte stehen als Objekt über der Fläche …
        assert!(
            position(&s, flaeche).unwrap() < position(&s, *e).unwrap(),
            "Randobjekt {e} liegt unter der Füllung"
        );
    }

    // … und in der Displayliste werden ihre Linien nach der Fläche gezeichnet.
    let ctx = RenderCtx::new(&s.doc);
    let dl = render_document(&ctx);
    let i_flaeche = dl
        .prims
        .iter()
        .position(|p| p.entity == flaeche && matches!(p.shape, Shape::Fill { .. }))
        .expect("Füllfläche in der Displayliste");
    let nachher_linien = dl.prims[i_flaeche + 1..]
        .iter()
        .filter(|p| grenze.border_entities.contains(&p.entity) && matches!(p.shape, Shape::Line(..)))
        .count();
    assert!(
        nachher_linien >= 4,
        "nur {nachher_linien} Wandlinien werden nach der Füllung gezeichnet"
    );
}

// ---------------------------------------------------------------------------
// 4. Hilfsfunktionen, die der Test oben braucht
// ---------------------------------------------------------------------------

#[test]
fn rechteck_hilfe_legt_vier_linien_an() {
    let ids = raum(&ECKEN);
    assert_eq!(ids.len(), 4);
}
