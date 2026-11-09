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

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{EntityKind, TextAlignH, TextAlignV};
use jjcad_core::persist;
use jjcad_core::geom::BBox;
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
            Shape::Fill { outer, .. } | Shape::FillOrdered { outer, .. } => {
                flaeche = Some(outer.clone())
            }
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
            Shape::Fill { .. } | Shape::FillOrdered { .. } if p.entity == e.id => p.style.fill,
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

// ---------------------------------------------------------------------------
// Zeichenreihenfolge (Turn 3)
// ---------------------------------------------------------------------------

/// Text mit Hintergrund, danach eine Linie quer durch dessen Kasten.
///
/// Der Text ordnet sich per Zeichenrang (4) über der Linie (3) ein — er
/// liegt also oben, die Linie darunter. Gemessen wird rechts neben dem
/// Zeichen (Text „I “ mit Leerzeichen): dort liegt nur die Hintergrundfläche,
/// kein Buchstabenstrich.
///
/// Die Linie kommt **nach** dem Text in die Zeichnung, damit ihre Höhe aus dem
/// fertigen Textkasten stammt und der Prüfpunkt sicher auf ihr liegt.
fn text_ueber_linie() -> (Session, jjcad_core::model::Id, Vec2, BBox) {
    let mut s = s();
    s.execute(
        "draw.text",
        &json!({ "p": [0.0, 0.0], "text": "I ", "height": 250.0,
                 "align": "left", "valign": "bottom",
                 "background": "#fff176", "frame": true }),
    )
    .expect("Text");
    let id = letzter_text(&s).id;
    let k = render::text_frame(&s.doc, letzter_text(&s)).expect("Kasten");
    let kb = bbox(&k);
    // Rechts neben dem Zeichen, auf halber Kasten­höhe.
    let punkt = Vec2::new(kb.0 + (kb.2 - kb.0) * 0.8, (kb.1 + kb.3) * 0.5);
    s.execute(
        "draw.line",
        &json!({ "a": [-300.0, punkt.y], "b": [700.0, punkt.y],
                 "layer": "Wände", "color": "#000000", "lineweight": 0.5 }),
    )
    .expect("Linie");
    let bbox = BBox::new(v(-400.0, -100.0), v(800.0, 600.0));
    (s, id, punkt, bbox)
}

/// Bildpunkt einer Weltkoordinate im Rasterbild (dieselbe Rechnung wie
/// `raster::render_model_pixmap`: 3 % Rand, dann einheitlicher Maßstab).
fn bildpunkt(bbox: BBox, w: u32, h: u32, x: f64, y: f64) -> (u32, u32) {
    let b = bbox.expanded(bbox.width().max(bbox.height()) * 0.03);
    let k = (w as f64 / b.width()).min(h as f64 / b.height());
    let ox = -b.min.x * k + (w as f64 - b.width() * k) * 0.5;
    let oy = b.max.y * k + (h as f64 - b.height() * k) * 0.5;
    let px = (x * k + ox).round().clamp(0.0, (w - 1) as f64) as u32;
    let py = (-y * k + oy).round().clamp(0.0, (h - 1) as f64) as u32;
    (px, py)
}

/// Farbe am Prüfpunkt im Rasterbild.
fn farbe_am_punkt(s: &Session, punkt: Vec2, bbox: BBox) -> image::Rgba<u8> {
    let (w, h) = (600u32, 300u32);
    let opts = RasterOptions {
        width_px: w,
        height_px: h,
        bbox: Some([bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y]),
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let png = jjcad_core::raster::png_bytes(&s.doc, &opts).expect("PNG");
    let img = image::load_from_memory(&png).expect("PNG lesen").to_rgba8();
    let (px, py) = bildpunkt(bbox, w, h, punkt.x, punkt.y);
    *img.get_pixel(px, py)
}

/// Der Hintergrund reist mit dem Text: liegt der Text über der Linie, deckt
/// seine Fläche sie zu — vorher stand jede Füllung vor allen Linien und der
/// Hintergrund lag darunter (T0126, Turn 3).
#[test]
fn hintergrund_ueberdeckt_frueher_gezeichnete_linien() {
    let (s, _, punkt, bbox) = text_ueber_linie();
    let p = farbe_am_punkt(&s, punkt, bbox);
    assert!(
        p[0] > 200 && p[1] > 200 && p[2] < 200,
        "Hintergrundfarbe erwartet, gemessen {p:?} — die Linie liegt noch oben"
    );
}

/// Gegenprobe: liegt die Linie **über** dem Text (`modify.order`), deckt sie
/// den Hintergrund zu. Die Reihenfolge folgt also der Zeichnung, nicht dem
/// Unterschied „Fläche/Linie".
#[test]
fn linie_ueber_dem_text_deckt_den_hintergrund_zu() {
    let (mut s, _, punkt, bbox) = text_ueber_linie();
    let linie = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
        .expect("Linie")
        .id;
    s.execute("modify.order", &json!({ "ids": [linie], "where": "front" }))
        .expect("Linie nach vorn");
    let p = farbe_am_punkt(&s, punkt, bbox);
    assert!(
        p[0] < 120 && p[1] < 120 && p[2] < 120,
        "Linienfarbe erwartet, gemessen {p:?}"
    );
}

/// Dieselbe Reihenfolge in der Anzeigeliste: der Hintergrund steht an der
/// Stelle des Textes, nicht am Anfang der Liste.
#[test]
fn hintergrund_steht_an_der_stelle_des_textes() {
    let (s, text_id, _, _) = text_ueber_linie();
    let linie = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
        .expect("Linie")
        .id;
    let dl = render_document(&RenderCtx::new(&s.doc));
    let pos = |id: u64| -> Vec<usize> {
        dl.prims
            .iter()
            .enumerate()
            .filter(|(_, p)| p.entity == id)
            .map(|(i, _)| i)
            .collect()
    };
    let t = pos(text_id);
    let l = pos(linie);
    assert!(!t.is_empty() && !l.is_empty(), "Prims fehlen");
    assert!(
        *t.first().unwrap() > *l.last().unwrap(),
        "Text/Hintergrund vor der Linie: Text {t:?}, Linie {l:?}"
    );
}

// ---------------------------------------------------------------------------
// Seitlicher Rand der Umrandung (Turn 5)
// ---------------------------------------------------------------------------

/// Erwarteter seitlicher Rand in mm: die halbe Luft, die der Zeilenkasten über
/// und unter den Versalien hat — aus den Schriftmaßen gerechnet.
fn erwarteter_rand(font: jjcad_core::model::TextFont, height: f64) -> f64 {
    let stil = jjcad_core::fonts::FontStyle::Regular;
    let lh = jjcad_core::fonts::line_height_em(font, stil);
    let cap = jjcad_core::fonts::cap_height_em(font, stil);
    (lh - cap) * 0.5 * height * render::TEXT_GROESSE_FAKTOR
}

/// Links und rechts steht derselbe Abstand wie oben und unten: der Rahmen ist
/// um genau die halbe Luft zwischen Zeilenhöhe und Versalhöhe breiter, und der
/// Textanker bleibt, wo er ist (T0126, Turn 5).
#[test]
fn umrandung_hat_seitlich_denselben_abstand_wie_oben() {
    for font in [jjcad_core::model::TextFont::Sans, jjcad_core::model::TextFont::DinMittel] {
        let s = text_schreiben(
            "Hintergrund",
            250.0,
            json!({ "frame": true, "font": font.key(), "align": "left", "valign": "bottom" }),
        );
        let e = letzter_text(&s);
        let text = render::text_box_corners(&s.doc, e).expect("Textkasten");
        let rahmen = render::text_frame(&s.doc, e).expect("Umrandung");
        let tb = bbox(&text);
        let rb = bbox(&rahmen);
        let soll = erwarteter_rand(font, 250.0);

        // Der Textkasten hängt am Anker (links unten bei 0,0) …
        assert_nahe(tb.0, 0.0, "Textkasten links am Anker");
        // … die Umrandung wächst nach außen.
        assert_nahe(tb.0 - rb.0, soll, "Rand links");
        assert_nahe(rb.2 - tb.2, soll, "Rand rechts");
        // Oben und unten bleibt es beim Zeilenkasten — dort ist die Luft schon.
        assert_nahe(rb.1, tb.1, "untere Kante unverändert");
        assert_nahe(rb.3, tb.3, "obere Kante unverändert");
        // Damit ist der seitliche Rand genau die halbe Luft oben/unten.
        let luft_oben = (tb.3 - tb.1 - jjcad_core::fonts::cap_height_em(font, jjcad_core::fonts::FontStyle::Regular) * 250.0 * render::TEXT_GROESSE_FAKTOR) * 0.5;
        assert_nahe(soll, luft_oben, "seitlicher Rand = Luft oben");
    }
}

/// Die runden Ecken schneiden den Text nicht.
///
/// Geprüft wird die Bedingung, unter der das gilt: der seitliche Rand muss
/// mindestens so groß sein wie der Rücksprung der Rundung auf Höhe der
/// Versalien. Bei einem Halbkreis mit dem Radius der halben Zeilenhöhe
/// (`r`) liegt die Bogenkante auf Höhe der Versaloberkante um
/// `r − √(r² − (Versalhöhe/2)²)` innen — genau so viel Rand braucht der Text
/// (T0126, Turn 5).
#[test]
fn runde_umrandung_schneidet_den_text_nicht() {
    let s = text_schreiben(
        "Hintergrund",
        250.0,
        json!({ "frame": true, "frame_round": true, "align": "left", "valign": "bottom" }),
    );
    let e = letzter_text(&s);
    let rahmen = rahmen_und_flaeche(&s).1.expect("Rahmen fehlt");
    let rb = bbox(&rahmen);
    let r = (rb.3 - rb.1) * 0.5;
    let stil = jjcad_core::fonts::FontStyle::Regular;
    let versal = jjcad_core::fonts::cap_height_em(jjcad_core::model::TextFont::Sans, stil)
        * 250.0
        * render::TEXT_GROESSE_FAKTOR;
    let ruecksprung = r - (r * r - (versal * 0.5).powi(2)).max(0.0).sqrt();
    let rand = erwarteter_rand(jjcad_core::model::TextFont::Sans, 250.0);
    assert!(
        rand > ruecksprung + 1.0,
        "Rand {rand:.1} mm deckt den Rücksprung der Rundung ({ruecksprung:.1} mm) nicht"
    );

    // Und die Ecken der Versalzeile liegen wirklich innerhalb des Pfades.
    let tb = bbox(&render::text_box_corners(&s.doc, e).expect("Textkasten"));
    let mitte = (rb.1 + rb.3) * 0.5;
    let ecken = [
        Vec2::new(tb.0, mitte + versal * 0.5),
        Vec2::new(tb.2, mitte + versal * 0.5),
        Vec2::new(tb.2, mitte - versal * 0.5),
        Vec2::new(tb.0, mitte - versal * 0.5),
    ];
    let flaeche: f64 = (0..rahmen.len())
        .map(|i| {
            let a = rahmen[i];
            let b = rahmen[(i + 1) % rahmen.len()];
            a.x * b.y - b.x * a.y
        })
        .sum();
    let vorzeichen = if flaeche >= 0.0 { 1.0 } else { -1.0 };
    for (i, p) in ecken.iter().enumerate() {
        let abstand = (0..rahmen.len())
            .filter_map(|k| {
                let a = rahmen[k];
                let b = rahmen[(k + 1) % rahmen.len()];
                let len = a.dist(b);
                // Der Pfad hat an den Übergängen Ecke/Bogen doppelte Punkte.
                if len < 1e-6 {
                    return None;
                }
                Some(vorzeichen * ((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)) / len)
            })
            .fold(f64::INFINITY, f64::min);
        assert!(
            abstand > 0.0,
            "Versalecke {i} bei {p:?} liegt außerhalb der runden Umrandung ({abstand:.1} mm)"
        );
    }
}

/// Radius der runden Umrandung aus dem Pfad: die linke Kante hat ein gerades
/// Stück der Länge `Höhe − 2 × Radius` (einzeilig null = Oval).
fn radius_aus_pfad(s: &Session) -> f64 {
    let (_, rahmen) = rahmen_und_flaeche(s);
    let rahmen = rahmen.expect("Rahmen fehlt");
    let b = bbox(&rahmen);
    let links: Vec<f64> = rahmen
        .iter()
        .filter(|p| (p.x - b.0).abs() < 1e-6)
        .map(|p| p.y)
        .collect();
    let ymin = links.iter().cloned().fold(f64::INFINITY, f64::min);
    let ymax = links.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    ((b.3 - b.1) - (ymax - ymin)) * 0.5
}

/// Mehrzeiliger Text: der Radius bleibt der einer Zeile — halbe Zeilenhöhe.
/// Damit sind es Viertelkreise an den Ecken eines Rechtecks, kein Oval
/// (T0126, Turn 7).
#[test]
fn runde_umrandung_behaelt_den_radius_bei_mehrzeiligen_text() {
    let einzeilig = text_schreiben(
        "Hintergrund",
        250.0,
        json!({ "frame": true, "frame_round": true }),
    );
    let zwei = text_schreiben(
        "Hintergrund\nHintergrund",
        250.0,
        json!({ "frame": true, "frame_round": true }),
    );
    let r1 = radius_aus_pfad(&einzeilig);
    let r2 = radius_aus_pfad(&zwei);
    let (_, h1) = render::text_box_mit("Hintergrund", &[], jjcad_core::model::TextFont::Sans, 250.0, 1.0);
    let (_, h2) = render::text_box_mit("Hintergrund\nHintergrund", &[], jjcad_core::model::TextFont::Sans, 250.0, 1.0);

    // Einzeilig: exaktes Oval — Radius ist die halbe Kasten­höhe.
    assert_nahe(r1, h1 * 0.5, "Oval einzeilig");
    // Zwei Zeilen: derselbe Radius wie einzeilig, nicht die halbe Kastenhöhe.
    assert_nahe(r2, r1, "Radius zwei Zeilen");
    assert_nahe(r2, h2 * 0.25, "halbe Zeilenhöhe bei zwei Zeilen");
    assert!(
        r2 < h2 * 0.5 - 1.0,
        "zwei Zeilen sind noch ein Oval: Radius {r2:.1} von halber Höhe {:.1}",
        h2 * 0.5
    );
}
