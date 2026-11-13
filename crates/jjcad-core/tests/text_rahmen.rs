//! Unsichtbarer Textrahmen und sein Punktfang (Ticket T0010, Maße seit T0110).
//!
//! Text hat keinen Griff und ist am Bildschirm nur die Schrift selbst. Sein
//! Rahmen — derselbe Kasten, den die Einfüge-Vorschau zeigt — ist der Bezug:
//! er wird bei Auswahl gezeichnet, und seine Ecken (END), Kantenmitten (MIT)
//! und Kanten (NÄC) fangen.
//!
//! Der Kasten ist **die echte Größe des Textes**: Breite aus den Vorschüben der
//! Schrift, Höhe aus der Zeilenhöhe der Familie (Aufstieg, Abstieg,
//! Durchschuss) mal Zeilenabstand. Ein Mittelwert je Zeichen traf das nur bei
//! serifenlos und schnitt bei DIN 1451 Breitschrift und EN ISO 3098 ab (T0110).
//!
//! Geprüft wird über `render::text_frame` (eine Quelle für Anzeige und Fang)
//! und `snap::snap_constrained` — denselben Weg, den die
//! Zeichenfläche für Zeigeranzeige und Klickpunkt nimmt. Die erwarteten Maße
//! rechnet der Test aus den Schriftdateien selbst (`text::width_em_mit_schrift`,
//! `fonts::line_height_em`), nicht aus `render::text_box` — sonst prüfte er
//! sich selbst. Alle Koordinaten in Millimetern (`input_unit: mm`).

use jjcad_core::fonts::{self, FontStyle};
use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{EntityKind, TextFont};
use jjcad_core::persist;
use jjcad_core::render;
use jjcad_core::session::Session;
use jjcad_core::snap::{snap_constrained, SnapKind, SnapResolution, SnapSource};
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .unwrap();
    s.execute(
        "doc.settings",
        &json!({ "snap": { "grid": false, "ortho": false, "polar": false } }),
    )
    .unwrap();
    s
}

/// Erwarteter Kasten (Breite, Höhe einer Zeile) aus den Schriftmaßen:
/// Vorschübe der breitesten Zeile, Zeilenhöhe der Familie, Anzeigefaktor 1,32.
fn erwartet(text: &str, font: TextFont, height: f64) -> (f64, f64) {
    let breite =
        jjcad_core::text::width_em_mit_schrift(text, &[], font) * height * render::TEXT_GROESSE_FAKTOR;
    let zeile =
        fonts::line_height_em(font, FontStyle::Regular) * height * render::TEXT_GROESSE_FAKTOR;
    (breite, zeile)
}

/// Seitlicher Rand der Umrandung in mm (T0126, Turn 5): die halbe Luft, die
/// der Zeilenkasten über und unter den Versalien hat — dieselbe Luft links
/// und rechts. Aus den Schriftmaßen gerechnet, nicht aus dem Renderer.
fn rand(font: TextFont, height: f64) -> f64 {
    let lh = fonts::line_height_em(font, FontStyle::Regular);
    let cap = fonts::cap_height_em(font, FontStyle::Regular);
    (lh - cap) * 0.5 * height * render::TEXT_GROESSE_FAKTOR
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
    s.doc.entities.last().expect("kein Text")
}

fn fang(s: &Session, cursor: Vec2, tol: f64) -> SnapResolution {
    snap_constrained(&s.doc, cursor, tol, None, &[], false)
}

fn assert_punkt(ist: Vec2, soll: Vec2, was: &str) {
    assert!(
        ist.dist(soll) < 1e-6,
        "{was}: erwartet {soll:?}, ist {ist:?}"
    );
}

fn assert_nahe(ist: f64, soll: f64, was: &str) {
    assert!(
        (ist - soll).abs() < 0.5,
        "{was}: erwartet {soll:.1} mm, ist {ist:.1} mm"
    );
}

// ---------------------------------------------------------------------------
// Der Rahmen ist der Kasten der echten Schrift
// ---------------------------------------------------------------------------

/// Text „Hallo“ in Sans, Höhe 250: Breite aus den Vorschüben (2,28 Em ×
/// 250 × 1,32 ≈ 752 mm), Höhe die Zeilenhöhe der Familie (1,15 Em ×
/// 250 × 1,32 ≈ 379 mm) — senkrecht mittig (Vorgabe) um den Anker.
#[test]
fn rahmen_umschliesst_den_text() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let e = letzter_text(&s);
    let (w, h) = erwartet("Hallo", TextFont::Sans, 250.0);
    assert_nahe(w, 752.0, "Breite");
    assert_nahe(h, 379.5, "Zeilenhöhe");

    // Der **Textkasten** (die Schrift) — die Umrandung ist breiter, siehe
    // `umrandung_hat_seitlich_denselben_abstand_wie_oben` (T0126, Turn 5).
    let ecken = render::text_box_corners(&s.doc, e).expect("Text braucht einen Kasten");
    assert!(matches!(e.kind, EntityKind::Text { .. }));
    assert_punkt(ecken[0], v(0.0, -h / 2.0), "links unten");
    assert_punkt(ecken[1], v(w, -h / 2.0), "rechts unten");
    assert_punkt(ecken[2], v(w, h / 2.0), "rechts oben");
    assert_punkt(ecken[3], v(0.0, h / 2.0), "links oben");

    // Ohne Umrandung ist die Bounding-Box der Textkasten selbst.
    let b = render::entity_bbox(&s.doc, e);
    assert!((b.max.x - b.min.x - w).abs() < 1e-6, "Breite {:?}", b);
    assert!((b.max.y - b.min.y - h).abs() < 1e-6, "Höhe {:?}", b);
}

/// Großbuchstaben sind breiter als der alte Mittelwert von 0,58 je Zeichen: der
/// Rahmen muss sie umschließen, nicht durchschneiden („WOHNEN“ stand im Rahmen
/// über den Rand hinaus). Jetzt steht die Breite exakt in den Vorschüben.
#[test]
fn rahmen_umschliesst_auch_grossbuchstaben() {
    let s = text_schreiben("WOHNEN", 250.0, json!({}));
    let e = letzter_text(&s);
    let ecken = render::text_box_corners(&s.doc, e).expect("Kasten");
    let breite = ecken[1].x - ecken[0].x;

    let (soll, _) = erwartet("WOHNEN", TextFont::Sans, 250.0);
    assert_nahe(breite, soll, "Breite WOHNEN");
    // „WOHNEN“ ist in Liberation Sans 4,08 Em breit — deutlich mehr als der
    // alte Mittelwert 6 × 0,58 = 3,48 Em.
    assert!(
        breite > 6.0 * 250.0 * 0.58,
        "Rahmen {breite} mm ist nicht breiter als der alte Mittelwert"
    );
}

/// Jede Schriftfamilie bekommt ihren **eigenen** Kasten. Die Breitschrift der
/// DIN 1451 ist die gestreckte Mittelschrift (× 1,25), die Schmalschrift
/// schmaler als die Grundschrift — das muss der Rahmen abbilden, sonst passt er
/// nur für eine Schriftart.
#[test]
fn jede_schrift_bekommt_ihren_eigenen_rahmen() {
    let mut breiten: Vec<(TextFont, f64)> = Vec::new();
    let mut hoehen: Vec<(TextFont, f64)> = Vec::new();
    for font in TextFont::ALL {
        let s = text_schreiben("Wohnen", 250.0, json!({ "font": font.key() }));
        let e = letzter_text(&s);
        let ecken = render::text_box_corners(&s.doc, e).expect("Kasten");
        let (w, h) = erwartet("Wohnen", font, 250.0);
        assert_nahe(
            ecken[1].x - ecken[0].x,
            w,
            &format!("{} Breite", font.key()),
        );
        assert_nahe(
            ecken[2].y - ecken[1].y,
            h,
            &format!("{} Höhe", font.key()),
        );
        breiten.push((font, w));
        hoehen.push((font, h));
    }
    let breite = |f: TextFont| breiten.iter().find(|(x, _)| *x == f).unwrap().1;
    let hoehe = |f: TextFont| hoehen.iter().find(|(x, _)| *x == f).unwrap().1;

    // DIN 1451 Breit = Mittelschrift waagerecht gestreckt (`fonts::strecke`, 1,25).
    let verhaeltnis = breite(TextFont::DinBreit) / breite(TextFont::DinMittel);
    assert!(
        (verhaeltnis - fonts::BREITSCHRIFT_STRECKUNG as f64).abs() < 0.02,
        "Breitschrift/Mittelschrift ist {verhaeltnis:.3} statt 1,25"
    );
    // Schmal ist schmaler als die Grundschrift.
    assert!(
        breite(TextFont::Narrow) < breite(TextFont::Sans),
        "Schmal ({:.1}) ist nicht schmaler als Serifenlos ({:.1})",
        breite(TextFont::Narrow),
        breite(TextFont::Sans)
    );
    // Die Zeilenhöhe hängt an der Datei: die runde Zierschrift setzt 1,25 Em,
    // DIN 1451 1,09 Em — der Rahmen muss beides zeigen (vorher fest 1,4).
    assert!(
        hoehe(TextFont::Rund) > hoehe(TextFont::DinMittel) * 1.12,
        "Rund ({:.1}) ist nicht deutlich höher als DIN 1451 ({:.1})",
        hoehe(TextFont::Rund),
        hoehe(TextFont::DinMittel)
    );
}

/// Mehrzeilig: die **breiteste Zeile** bestimmt die Breite (nicht die Summe
/// aller Zeichen), die Zeilenzahl mal Zeilenhöhe die Höhe.
#[test]
fn mehrzeiliger_rahmen_umschliesst_alle_zeilen() {
    let s = text_schreiben("WOHNEN\nOG", 250.0, json!({}));
    let e = letzter_text(&s);
    let ecken = render::text_box_corners(&s.doc, e).expect("Kasten");

    let (w, zeile) = erwartet("WOHNEN", TextFont::Sans, 250.0);
    assert_nahe(ecken[1].x - ecken[0].x, w, "Breite");
    let hoehe = ecken[2].y - ecken[1].y;
    assert_nahe(hoehe, 2.0 * zeile, "Höhe zwei Zeilen");
    assert_punkt(ecken[0], v(0.0, -hoehe / 2.0), "links unten");
    assert_punkt(ecken[1], v(w, -hoehe / 2.0), "rechts unten");
    assert_punkt(ecken[2], v(w, hoehe / 2.0), "rechts oben");
    assert_punkt(ecken[3], v(0.0, hoehe / 2.0), "links oben");
}

/// Dreizeilig: die Höhe folgt der Zeilenzahl.
#[test]
fn drei_zeilen_ergeben_dreifache_hoehe() {
    let s = text_schreiben("A\nB\nC", 100.0, json!({}));
    let e = letzter_text(&s);
    let ecken = render::text_frame(&s.doc, e).expect("Rahmen");
    let (_, zeile) = erwartet("A", TextFont::Sans, 100.0);
    assert_nahe(
        ecken[2].y - ecken[1].y,
        3.0 * zeile,
        "Höhe drei Zeilen",
    );
}

/// Der Zeilenabstand aus dem Eigenschaften-Panel streckt die Zeilen und damit
/// den Rahmen — gespeichert als Vielfaches der Zeilenhöhe der Schrift.
#[test]
fn zeilenabstand_streckt_den_rahmen() {
    let s1 = text_schreiben("A\nB", 250.0, json!({}));
    let s2 = text_schreiben("A\nB", 250.0, json!({ "line_spacing": 2.0 }));
    let h1 = {
        let k = render::text_frame(&s1.doc, letzter_text(&s1)).unwrap();
        k[2].y - k[1].y
    };
    let h2 = {
        let k = render::text_frame(&s2.doc, letzter_text(&s2)).unwrap();
        k[2].y - k[1].y
    };
    assert_nahe(h2, 2.0 * h1, "doppelter Zeilenabstand");

    // Auch über `modify.set` (Eigenschaften-Panel der Auswahl).
    let mut s3 = text_schreiben("A\nB", 250.0, json!({}));
    let id = letzter_text(&s3).id;
    s3.execute(
        "modify.set",
        &json!({ "ids": [id], "line_spacing": 1.5 }),
    )
    .expect("Zeilenabstand setzen");
    let k = render::text_frame(&s3.doc, letzter_text(&s3)).unwrap();
    assert_nahe(k[2].y - k[1].y, 1.5 * h1, "Zeilenabstand über modify.set");

    // Der Wert steht im Objekt.
    let EntityKind::Text { line_spacing, .. } = &letzter_text(&s2).kind else {
        panic!("kein Text");
    };
    assert!(
        (line_spacing - 2.0).abs() < 1e-9,
        "gespeichert: {line_spacing}"
    );
}

/// Der Zeilenabstand steht in der gespeicherten Datei und kommt beim Laden
/// wieder heraus (Feld `line_spacing` im Textobjekt).
#[test]
fn zeilenabstand_steht_in_der_datei() {
    let s = text_schreiben(
        "A\nB",
        250.0,
        json!({ "font": "din_mittel", "line_spacing": 1.5 }),
    );
    let text = persist::to_json(&s.doc).expect("JSON");
    assert!(
        text.contains("\"line_spacing\": 1.5") || text.contains("\"line_spacing\":1.5"),
        "kein line_spacing im JSON: {text}"
    );
    let zurueck = persist::from_json(&text).expect("laden");
    let e = zurueck.entities.last().expect("Text fehlt");
    let EntityKind::Text {
        line_spacing, font, ..
    } = &e.kind
    else {
        panic!("kein Text");
    };
    assert!((line_spacing - 1.5).abs() < 1e-9, "gelesen: {line_spacing}");
    assert_eq!(*font, TextFont::DinMittel, "Schrift beim Laden");
}

/// Gegenprobe: andere Objekte haben keinen Textrahmen.
#[test]
fn andere_objekte_haben_keinen_textrahmen() {
    let mut s = s();
    s.execute(
        "draw.line",
        &json!({ "a": [0.0, 0.0], "b": [1000.0, 0.0] }),
    )
    .unwrap();
    let e = s.doc.entities.last().unwrap();
    assert!(render::text_frame(&s.doc, e).is_none());
    assert!(render::text_frame_segments(&s.doc, e).is_empty());
}

/// Vier Kanten, die den Kasten schließen — die Zerlegung, die der Fang nutzt.
#[test]
fn rahmen_besteht_aus_vier_kanten() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let e = letzter_text(&s);
    let segs = render::text_frame_segments(&s.doc, e);
    assert_eq!(segs.len(), 4, "vier Kanten: {segs:?}");
}

// ---------------------------------------------------------------------------
// Punktfang am Rahmen
// ---------------------------------------------------------------------------

/// Die Ecke des Rahmens fängt als Endpunkt — nicht der Ankerpunkt des Textes.
#[test]
fn ecke_des_rahmens_faengt_als_endpunkt() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let (w, h) = erwartet("Hallo", TextFont::Sans, 250.0);
    let rand = rand(TextFont::Sans, 250.0);
    let r = fang(&s, v(w + rand + 7.0, h / 2.0 + 5.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(w + rand, h / 2.0), "Ecke rechts oben");
}

/// Die Mitte einer Rahmenkante fängt als Mittelpunkt.
#[test]
fn kantenmitte_faengt_als_mittelpunkt() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let (w, h) = erwartet("Hallo", TextFont::Sans, 250.0);
    let r = fang(&s, v(w / 2.0 + 2.0, h / 2.0 + 5.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Midpoint), "{r:?}");
    assert_punkt(r.point, v(w / 2.0, h / 2.0), "Mitte der Oberkante");
}

/// Auf der Rahmenkante fängt der nächste Punkt — der Text hat also nicht nur
/// ein Fangpunkt am Anker, sondern eine fangbare Kante.
#[test]
fn kante_faengt_als_naechster_punkt() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let rand = rand(TextFont::Sans, 250.0);
    let r = fang(&s, v(-rand + 4.0, 60.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Nearest), "{r:?}");
    assert_punkt(r.point, v(-rand, 60.0), "Punkt auf der linken Kante");
}

/// Der Ankerpunkt des Textes bleibt fangbar (Rückfall auf das alte Verhalten).
#[test]
fn ankerpunkt_faengt_weiter() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let r = fang(&s, v(5.0, 4.0), 40.0);
    assert_eq!(r.source, SnapSource::Object(SnapKind::Endpoint), "{r:?}");
    assert_punkt(r.point, v(0.0, 0.0), "Ankerpunkt");
}

/// Außerhalb des Fangradius zieht der Rahmen nicht.
#[test]
fn rahmen_faengt_nur_im_radius() {
    let s = text_schreiben("Hallo", 250.0, json!({}));
    let cursor = v(810.0, 500.0);
    let r = fang(&s, cursor, 40.0);
    assert_eq!(r.source, SnapSource::Free, "{r:?}");
    assert_punkt(r.point, cursor, "roher Zeiger");
}

/// Ein gedrehter Text hat einen **gedrehten** Rahmen — das Viereck dreht um
/// den Anker mit, statt als achsenparallele Box zu stehen (T0025: „die
/// Umrandung ist falsch“).
#[test]
fn gedrehter_rahmen_dreht_mit_dem_text() {
    let s = text_schreiben(
        "Hallo",
        250.0,
        json!({ "angle": 90.0, "valign": "middle" }),
    );
    let e = letzter_text(&s);
    let (w, h) = erwartet("Hallo", TextFont::Sans, 250.0);
    let rand = rand(TextFont::Sans, 250.0);
    let ecken = render::text_frame(&s.doc, e).expect("Rahmen");

    // 90° gegen den Uhrzeigersinn: „rechts“ zeigt nach oben, „oben“ nach links.
    // Der seitliche Rand liegt mit auf der gedrehten Textachse (T0126, Turn 5).
    assert_punkt(ecken[0], v(h / 2.0, -rand), "links unten gedreht");
    assert_punkt(ecken[1], v(h / 2.0, w + rand), "rechts unten gedreht");
    assert_punkt(ecken[2], v(-h / 2.0, w + rand), "rechts oben gedreht");
    assert_punkt(ecken[3], v(-h / 2.0, -rand), "links oben gedreht");

    // Die Bounding-Box bleibt die Hülle des gedrehten Kastens.
    let b = render::entity_bbox(&s.doc, e);
    assert!((b.max.x - b.min.x - h).abs() < 1e-6, "Breite {b:?}");
    assert!((b.max.y - b.min.y - w).abs() < 1e-6, "Höhe {b:?}");
}
