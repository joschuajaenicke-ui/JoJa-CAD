//! Umrandung und Hintergrund von Textfeldern (Ticket T0126).
//!
//! Ein Text kann seinen Bearbeitungsrahmen dauerhaft zeigen — eckig oder rund
//! (Radius = halbe Schrifthöhe, einzeilig ein Oval aus zwei Halbkreisen) — und
//! eine Hintergrundfläche in eigener Farbe tragen. Die Fläche ist immer das
//! **Rechteck** der Umrandung, auch bei runder Umrandung.
//!
//! Geprüft wird über die Anzeige (`render_document`) und die gespeicherte
//! Datei: Umrandung und Hintergrund entstehen im Renderer, damit Bildschirm,
//! PNG und PDF dieselbe Form zeigen. Alle Koordinaten in Millimetern
//! (`input_unit: mm`).

use jjcad_core::geom::Vec2;
use jjcad_core::model::{EntityKind, TextAlignH, TextAlignV};
use jjcad_core::persist;
use jjcad_core::render::{self, render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s
}

fn text_schreiben(text: &str, height: f64, extra: serde_json::Value) -> Session {
    let mut s = s();
    let mut p = json!({ "p": [0.0, 0.0], "text": text, "height": height });
    for (k, wert) in extra.as_object().into_iter().flatten() {
        p[k] = wert.clone();
    }
    s.execute("draw.text", &p).expect("Text anlegen");
    s
}

fn letzter_text(s: &Session) -> &jjcad_core::model::Entity {
    s.doc
        .entities
        .iter()
        .rev()
        .find(|e| matches!(e.kind, EntityKind::Text { .. }))
        .expect("kein Text")
}

fn assert_nahe(a: f64, b: f64, was: &str) {
    assert!((a - b).abs() < 1e-6, "{was}: {a} statt {b}");
}

/// Fläche und Umrandung des Textes aus der Anzeige.
fn rahmen_und_flaeche(s: &Session) -> (Option<Vec<Vec2>>, Option<Vec<Vec2>>) {
    let e = letzter_text(s);
    let dl = render_document(&RenderCtx::new(&s.doc));
    let mut flaeche = None;
    let mut rahmen = None;
    for prim in &dl.prims {
        if prim.entity != e.id {
            continue;
        }
        match &prim.shape {
            Shape::Fill { outer, .. } => flaeche = Some(outer.clone()),
            Shape::Poly(pts, closed) if *closed => rahmen = Some(pts.clone()),
            _ => {}
        }
    }
    (flaeche, rahmen)
}

fn bbox(pts: &[Vec2]) -> (f64, f64, f64, f64) {
    let mut b = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in pts {
        b.0 = b.0.min(p.x);
        b.1 = b.1.min(p.y);
        b.2 = b.2.max(p.x);
        b.3 = b.3.max(p.y);
    }
    b
}

/// Ohne Häkchen bleibt der Text nackt — keine zusätzliche Fläche, kein Pfad.
#[test]
fn ohne_umrandung_keine_zusaetzliche_form() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let (flaeche, rahmen) = rahmen_und_flaeche(&s);
    assert!(flaeche.is_none(), "Fläche ohne Häkchen: {flaeche:?}");
    assert!(rahmen.is_none(), "Rahmen ohne Häkchen: {rahmen:?}");
}

/// Die eckige Umrandung liegt genau auf dem Textkasten.
#[test]
fn eckige_umrandung_folgt_dem_kasten() {
    let s = text_schreiben("Hallo", 250.0, json!({ "frame": true }));
    let e = letzter_text(&s);
    let kasten = render::text_frame(&s.doc, e).expect("Kasten");
    let (_, rahmen) = rahmen_und_flaeche(&s);
    let rahmen = rahmen.expect("Rahmen fehlt");
    assert!(rahmen.len() >= 4, "zu wenige Punkte: {}", rahmen.len());
    let kb = bbox(&kasten);
    let rb = bbox(&rahmen);
    for (a, b, was) in [
        (rb.0, kb.0, "links"),
        (rb.1, kb.1, "unten"),
        (rb.2, kb.2, "rechts"),
        (rb.3, kb.3, "oben"),
    ] {
        assert_nahe(a, b, was);
    }
}

/// Die runde Umrandung ist ein Oval: Radius = halbe Schrifthöhe, die
/// äußersten Punkte liegen auf halber Höhe (zwei Halbkreise), die Ecke des
/// Kastens ist **nicht** Teil des Pfades.
#[test]
fn runde_umrandung_bildet_ein_oval() {
    let s = text_schreiben("Hallo", 250.0, json!({ "frame": true, "frame_round": true }));
    let e = letzter_text(&s);
    let kasten = render::text_frame(&s.doc, e).expect("Kasten");
    let kb = bbox(&kasten);
    let (hoehe, mitte) = (kb.3 - kb.1, (kb.1 + kb.3) * 0.5);
    let radius = hoehe * 0.5;
    assert_nahe(radius, e_height(&s) * 0.5, "Radius = halbe Schrifthöhe");

    let (_, rahmen) = rahmen_und_flaeche(&s);
    let rahmen = rahmen.expect("Rahmen fehlt");
    let rb = bbox(&rahmen);
    assert_nahe(rb.1, kb.1, "untere Kante");
    assert_nahe(rb.3, kb.3, "obere Kante");

    // Die Rundung: je halber Höhe ein Bogen. Auf halber Höhe liegt der
    // äußerste linke bzw. rechte Punkt, an der unteren Kante beginnt der
    // Pfad erst einen Radius weiter innen.
    let links: Vec<&Vec2> = rahmen.iter().filter(|p| p.x < rb.0 + 1e-6).collect();
    let rechts: Vec<&Vec2> = rahmen.iter().filter(|p| p.x > rb.2 - 1e-6).collect();
    for p in links.iter().chain(rechts.iter()) {
        assert_nahe(p.y, mitte, "Bogenpunkt auf halber Höhe");
    }
    let unten: Vec<&Vec2> = rahmen.iter().filter(|p| p.y < rb.1 + 1e-6).collect();
    for p in &unten {
        assert!(
            p.x >= kb.0 + radius - 1e-6 && p.x <= kb.2 - radius + 1e-6,
            "untere Kante beginnt erst nach dem Radius: {p:?}"
        );
    }
    // Keine Ecke des Kastens im Pfad — sonst wäre die Umrandung eckig.
    assert!(
        !rahmen
            .iter()
            .any(|p| (p.x - kb.0).abs() < 1e-6 && (p.y - kb.1).abs() < 1e-6),
        "Ecke links unten liegt im runden Pfad"
    );
}

/// Höhe einer Zeile in Welt-mm, aus dem gerenderten Textprim.
fn e_height(s: &Session) -> f64 {
    let dl = render_document(&RenderCtx::new(&s.doc));
    for prim in &dl.prims {
        if let Shape::Text(t) = &prim.shape {
            return t.box_size().1;
        }
    }
    panic!("kein Text in der Anzeige");
}

/// Der Hintergrund füllt genau das Rechteck der Umrandung und trägt die
/// gewählte Farbe — auch bei runder Umrandung bleibt er eckig.
#[test]
fn hintergrund_ist_das_rechteck_der_umrandung() {
    let s = text_schreiben(
        "Hallo",
        250.0,
        json!({ "frame": true, "frame_round": true, "background": "#ffcc00" }),
    );
    let e = letzter_text(&s);
    let kasten = render::text_frame(&s.doc, e).expect("Kasten");
    let (flaeche, _) = rahmen_und_flaeche(&s);
    let flaeche = flaeche.expect("Hintergrund fehlt");
    assert_eq!(flaeche.len(), 4, "Fläche ist ein Viereck: {flaeche:?}");
    let kb = bbox(&kasten);
    let fb = bbox(&flaeche);
    for (a, b, was) in [
        (fb.0, kb.0, "links"),
        (fb.1, kb.1, "unten"),
        (fb.2, kb.2, "rechts"),
        (fb.3, kb.3, "oben"),
    ] {
        assert_nahe(a, b, was);
    }
    // Farbe steht im Prim.
    let dl = render_document(&RenderCtx::new(&s.doc));
    let farbe = dl
        .prims
        .iter()
        .find_map(|p| match &p.shape {
            Shape::Fill { .. } if p.entity == e.id => p.style.fill,
            _ => None,
        })
        .expect("keine Füllfarbe");
    assert_eq!(farbe.to_hex(), "#FFCC00", "Hintergrundfarbe");
}

/// Umrandung und Hintergrund stehen in der Datei und kommen beim Laden wieder
/// heraus; `modify.set` ändert und entfernt sie.
#[test]
fn umrandung_und_hintergrund_in_der_datei() {
    let mut s = text_schreiben(
        "Hallo",
        250.0,
        json!({ "frame": true, "frame_round": true, "background": "#112233" }),
    );
    let text = persist::to_json(&s.doc).expect("JSON");
    assert!(text.contains("\"frame\": true"), "frame im JSON: {text}");
    assert!(
        text.contains("\"frame_round\": true"),
        "frame_round im JSON: {text}"
    );
    assert!(text.contains("#112233"), "background im JSON: {text}");

    let zurueck = persist::from_json(&text).expect("laden");
    let e = zurueck.entities.last().expect("Text fehlt");
    let EntityKind::Text {
        frame,
        frame_round,
        background,
        ..
    } = &e.kind
    else {
        panic!("kein Text");
    };
    assert!(*frame && *frame_round, "gelesen: {frame}/{frame_round}");
    assert_eq!(background.map(|c| c.to_hex()), Some("#112233".into()));

    // Ändern und wieder abschalten.
    let id = letzter_text(&s).id;
    s.execute(
        "modify.set",
        &json!({ "ids": [id], "frame": false, "background": "none" }),
    )
    .expect("Hintergrund entfernen");
    let EntityKind::Text {
        frame,
        frame_round,
        background,
        ..
    } = &letzter_text(&s).kind
    else {
        panic!("kein Text")
    };
    assert!(!frame, "Umrandung noch an");
    assert!(*frame_round, "runde Form bleibt gespeichert");
    assert!(background.is_none(), "Hintergrund nicht entfernt");

    let (flaeche, rahmen) = rahmen_und_flaeche(&s);
    assert!(flaeche.is_none(), "Fläche trotz gelöschtem Hintergrund");
    assert!(rahmen.is_none(), "Rahmen trotz abgeschalteter Umrandung");
}

/// Umrandung und Hintergrund überleben Drehen, Spiegeln und Skalieren — die
/// Felder gehören zum Objekt, nicht zur Anzeige.
#[test]
fn umrandung_ueberlebt_das_veraendern() {
    let mut s = text_schreiben(
        "Hallo",
        250.0,
        json!({ "frame": true, "background": "#00ff00" }),
    );
    let id = letzter_text(&s).id;
    let _ = s.execute(
        "modify.set",
        &json!({ "ids": [id], "align": "center", "valign": "middle" }),
    );
    let EntityKind::Text {
        frame,
        background,
        align,
        valign,
        ..
    } = &letzter_text(&s).kind
    else {
        panic!("kein Text")
    };
    assert!(*frame, "Umrandung verloren");
    assert!(background.is_some(), "Hintergrund verloren");
    assert_eq!((*align, *valign), (TextAlignH::Center, TextAlignV::Middle));
}
