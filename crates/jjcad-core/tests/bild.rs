//! T0100 — Bilder (PNG und JPEG) einfügen.
//!
//! Ein Bild ist ein Objekt mit zwei gegenüberliegenden Ecken; die Bilddatei
//! liegt eingebettet im Dokument. Geprüft wird der ganze Weg: Action →
//! Dokument → Speichern/Laden → PNG-Ausgabe → PDF.

use jjcad_core::bild;
use jjcad_core::export::RasterOptions;
use jjcad_core::geom::Vec2;
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;
use std::io::Cursor;

/// PNG mit einer einzigen Farbe — klein und ohne Bilddatei im Repo.
fn png_bytes(r: u8, g: u8, b: u8, breite: u32, hoehe: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(breite, hoehe, image::Rgba([r, g, b, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .expect("PNG schreiben");
    out
}

/// JPEG mit einer einzigen Farbe.
fn jpg_bytes(r: u8, g: u8, b: u8, breite: u32, hoehe: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(breite, hoehe, image::Rgb([r, g, b]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .expect("JPEG schreiben");
    out
}

fn leer() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .unwrap();
    s
}

fn bild_einfuegen(s: &mut Session, daten: &[u8], a: (f64, f64), b: (f64, f64)) -> Id {
    let res = s
        .execute(
            "draw.image",
            &json!({
                "a": [a.0, a.1],
                "b": [b.0, b.1],
                "name": "Probe.png",
                "data": bild::base64_kodiere(daten),
            }),
        )
        .expect("Bild einfügen");
    res["created"][0].as_u64().expect("keine Id")
}

#[test]
fn bild_wird_mit_zwei_ecken_eingefuegt() {
    let mut s = leer();
    let id = bild_einfuegen(&mut s, &png_bytes(220, 30, 30, 4, 2), (1000.0, 500.0), (3000.0, 1500.0));
    let e = s.doc.entity(id).expect("Bild fehlt");
    match &e.kind {
        EntityKind::Image { a, b, name, data } => {
            assert_eq!((*a, *b), (Vec2::new(1000.0, 500.0), Vec2::new(3000.0, 1500.0)));
            assert_eq!(name, "Probe.png");
            assert!(bild::dekodiere(data).is_some(), "Bilddaten nicht lesbar");
        }
        k => panic!("kein Bild: {k:?}"),
    }
    // Das Rechteck der beiden Ecken ist die Bounding-Box — auch bei
    // vertauschter Reihenfolge der Klicks.
    let bb = jjcad_core::render::entity_bbox(&s.doc, e);
    assert_eq!(bb.min, Vec2::new(1000.0, 500.0));
    assert_eq!(bb.max, Vec2::new(3000.0, 1500.0));
    assert_eq!(e.kind.type_name(), "Bild");
}

#[test]
fn jpg_wird_ebenso_angenommen() {
    let mut s = leer();
    let id = bild_einfuegen(&mut s, &jpg_bytes(20, 40, 200, 8, 4), (0.0, 0.0), (800.0, 400.0));
    let e = s.doc.entity(id).expect("Bild fehlt");
    let EntityKind::Image { data, .. } = &e.kind else {
        panic!("kein Bild");
    };
    let zerlegt = bild::dekodiere(data).expect("JPEG zerlegen");
    assert_eq!((zerlegt.breite, zerlegt.hoehe), (8, 4));
}

#[test]
fn fremde_daten_werden_abgelehnt() {
    let mut s = leer();
    let res = s.execute(
        "draw.image",
        &json!({
            "a": [0.0, 0.0],
            "b": [100.0, 100.0],
            "data": bild::base64_kodiere(b"kein bild"),
        }),
    );
    assert!(res.is_err(), "ungültige Bilddaten müssen ein Fehler sein");
    assert!(s.doc.entities.is_empty(), "es darf nichts entstanden sein");
}

#[test]
fn bild_ueberlebt_speichern_und_laden() {
    let mut s = leer();
    let id = bild_einfuegen(&mut s, &png_bytes(10, 200, 10, 3, 3), (500.0, 250.0), (2500.0, 1250.0));
    let text = jjcad_core::persist::to_json(&s.doc).expect("JSON");
    let geladen = jjcad_core::persist::from_json(&text).expect("laden");
    let e = geladen.entity(id).expect("Bild nach dem Laden fehlt");
    match &e.kind {
        EntityKind::Image { a, b, data, .. } => {
            assert_eq!((*a, *b), (Vec2::new(500.0, 250.0), Vec2::new(2500.0, 1250.0)));
            assert!(bild::dekodiere(data).is_some(), "Daten verloren");
        }
        k => panic!("kein Bild: {k:?}"),
    }
}

#[test]
fn dateipfad_wird_gelesen() {
    let mut s = leer();
    let pfad = std::env::temp_dir().join(format!("jjcad-t0100-{}.png", std::process::id()));
    std::fs::write(&pfad, png_bytes(0, 0, 0, 5, 5)).expect("Testdatei");
    let res = s
        .execute(
            "draw.image",
            &json!({"a": [0.0, 0.0], "b": [1000.0, 1000.0], "file": pfad.to_string_lossy()}),
        )
        .expect("Bild aus Datei");
    let id = res["created"][0].as_u64().expect("keine Id");
    let _ = std::fs::remove_file(&pfad);
    let e = s.doc.entity(id).expect("Bild fehlt");
    match &e.kind {
        EntityKind::Image { name, data, .. } => {
            assert!(name.ends_with(".png"), "Dateiname fehlt: {name}");
            assert!(bild::dekodiere(data).is_some());
        }
        k => panic!("kein Bild: {k:?}"),
    }
}

#[test]
fn png_ausgabe_zeigt_das_bild() {
    let mut s = leer();
    // Einfarbig rot, damit die Pixelprobe eindeutig ist.
    bild_einfuegen(&mut s, &png_bytes(230, 20, 20, 6, 3), (1000.0, 500.0), (3000.0, 1500.0));
    let opts = RasterOptions {
        width_px: 200,
        height_px: 100,
        bbox: Some([1000.0, 500.0, 3000.0, 1500.0]),
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let png = jjcad_core::raster::png_bytes(&s.doc, &opts).expect("PNG rendern");
    let img = image::load_from_memory(&png).expect("PNG lesen").to_rgba8();
    // Mitte des Rechtecks: dort liegt das Bild, nicht das Papier.
    let p = img.get_pixel(img.width() / 2, img.height() / 2);
    assert!(
        p[0] > 180 && p[1] < 90 && p[2] < 90,
        "Bildfarbe erwartet, gemessen {p:?}"
    );
    // Der Rand bleibt Papier (weiß).
    let rand = img.get_pixel(2, 2);
    assert!(rand[0] > 240 && rand[1] > 240 && rand[2] > 240, "Rand {rand:?}");
}

#[test]
fn pdf_bettet_das_bild_als_objekt_ein() {
    let mut s = leer();
    bild_einfuegen(&mut s, &png_bytes(0, 120, 240, 7, 5), (0.0, 0.0), (2000.0, 1000.0));
    let pdf = jjcad_core::pdf::pdf_bytes(&s.doc, None).expect("PDF");
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("/Subtype /Image"), "kein Bildobjekt im PDF");
    assert!(text.contains("/Width 7") && text.contains("/Height 5"), "Maße fehlen");
    // Die Seite muss das Bild als XObject führen — sonst liegt es im PDF,
    // wird aber nicht gezeichnet. (Der Inhaltsstrom selbst ist komprimiert.)
    assert!(
        text.contains("/XObject << /Im"),
        "Seite ohne Bildressource"
    );
}

#[test]
fn bild_ist_in_der_flaeche_anklickbar() {
    let mut s = leer();
    let id = bild_einfuegen(&mut s, &png_bytes(200, 200, 200, 2, 2), (0.0, 0.0), (4000.0, 3000.0));
    // Mitten in der Fläche trifft das Bild …
    assert_eq!(jjcad_core::snap::hit_test(&s.doc, Vec2::new(2000.0, 1500.0), 5.0, None), Some(id));
    // … außerhalb nicht.
    assert_eq!(jjcad_core::snap::hit_test(&s.doc, Vec2::new(6000.0, 1500.0), 5.0, None), None);
    // Ein Querfenster **innerhalb** der Fläche wählt das Bild mit (die Fläche
    // ist sein Rechteck, wie beim Klick).
    let treffer = jjcad_core::snap::select_window(
        &s.doc,
        Vec2::new(1500.0, 1000.0),
        Vec2::new(2500.0, 2000.0),
        true,
    );
    assert!(treffer.contains(&id), "Querfenster im Bild: {treffer:?}");

    // Eine Wand über dem Bild hat Vorrang: der Klick wählt die Wand.
    let wand = s
        .execute(
            "draw.wall",
            &json!({"a": [0.0, 1500.0], "b": [4000.0, 1500.0], "thickness": 200.0}),
        )
        .expect("Wand")["created"][0]
        .as_u64()
        .expect("Wand-Id");
    assert_eq!(
        jjcad_core::snap::hit_test(&s.doc, Vec2::new(2000.0, 1500.0), 5.0, None),
        Some(wand),
        "die Wand liegt über dem Bild"
    );
}

#[test]
fn bild_laesst_sich_nicht_zerlegen() {
    let mut s = leer();
    let id = bild_einfuegen(&mut s, &png_bytes(0, 0, 0, 2, 2), (0.0, 0.0), (100.0, 100.0));
    let res = jjcad_core::actions::explode_entity(&mut s.doc, id);
    assert!(res.is_err(), "Zerlegen würde das Bild löschen");
    assert!(s.doc.entity(id).is_some(), "das Bild muss stehen bleiben");
}
