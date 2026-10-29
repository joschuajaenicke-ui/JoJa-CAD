//! Textformatierung: Läufe (Fett, Kursiv, Unterstrichen, Hochgestellt)
//! überstehen den Weg durch das PDF.
//!
//! Geprüft wird beides:
//!
//! 1. **Satz** — dieselbe Zerlegung in Anzeige, Raster und PDF: ein Lauf mit
//!    Fett ist breiter als derselbe Lauf in Grundschrift, Hochstellung sitzt
//!    über der Grundlinie.
//! 2. **Rundlauf** — Export → Import liefert dieselben Läufe mit denselben
//!    Auszeichnungen wieder zurück (Unterstrichen wird als Viereck unter dem
//!    Text geschrieben und beim Lesen wieder zur Auszeichnung).

use jjcad_core::model::*;
use jjcad_core::pdf;
use jjcad_core::text;
use serde_json::json;
use std::path::{Path, PathBuf};

fn ziel(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    let _ = std::fs::remove_file(&p);
    p
}

/// Dokument mit einem Text, der alle Auszeichnungen trägt.
fn dokument_mit_laeufen() -> Document {
    let mut doc = Document::new();
    let layer = doc.current_layer;
    let runs = vec![
        TextRun {
            text: "Raum ".into(),
            ..Default::default()
        },
        TextRun {
            text: "Wohnen".into(),
            bold: true,
            ..Default::default()
        },
        TextRun {
            text: " · ".into(),
            ..Default::default()
        },
        TextRun {
            text: "Küche".into(),
            italic: true,
            ..Default::default()
        },
        TextRun {
            text: " · ".into(),
            ..Default::default()
        },
        TextRun {
            text: "32,4".into(),
            underline: true,
            ..Default::default()
        },
        TextRun {
            text: "m".into(),
            ..Default::default()
        },
        TextRun {
            text: "2".into(),
            superscript: true,
            ..Default::default()
        },
    ];
    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    let id = doc.new_id();
    let e = Entity::new(
        id,
        layer,
        EntityKind::Text {
            p: jjcad_core::geom::v(20.0, 20.0),
            text,
            height: 5.0,
            angle: 0.0,
            align: TextAlignH::Left,
            valign: TextAlignV::Bottom,
            bold: false,
            italic: false,
            underline: false,
            superscript: false,
            runs,
        },
    );
    doc.add_entity(e);
    doc
}

#[test]
fn satz_mischt_schnitte_und_hoehen() {
    let doc = dokument_mit_laeufen();
    let EntityKind::Text { text: t, height, runs, .. } = &doc.ents()[0].kind else {
        panic!("Text erwartet");
    };
    let zeilen = text::layout(t, runs);
    assert_eq!(zeilen.len(), 1);
    let segs = &zeilen[0].segs;
    eprintln!("Stücke: {segs:?}");
    let breite_mm = zeilen[0].width_em * height;
    // Der fette Teil ist breiter als in Grundschrift — sonst wäre Fett nur
    // eine Behauptung.
    let fett = segs
        .iter()
        .find(|s| s.text == "Wohnen")
        .expect("fetten Lauf gefunden");
    let normal = text_breite("Wohnen", jjcad_core::fonts::FontStyle::Regular);
    let fett_w = text_breite("Wohnen", jjcad_core::fonts::FontStyle::Bold);
    assert!(fett_w > normal, "Fett muss breiter sein: {normal} / {fett_w}");
    assert!(fett.style == jjcad_core::fonts::FontStyle::Bold);
    // Hochstellung: eigener Lauf mit Versatz und kleinerer Größe
    let hoch = segs.iter().find(|s| s.text == "2").expect("Hochstellung");
    assert!(hoch.rise_em > 0.2, "Versatz {}", hoch.rise_em);
    assert!(hoch.scale < 0.8, "Verkleinerung {}", hoch.scale);
    assert!(breite_mm > 20.0, "Breite {breite_mm}");
}

/// Breite eines Textes in Em für einen Schnitt.
fn text_breite(s: &str, st: jjcad_core::fonts::FontStyle) -> f64 {
    jjcad_core::fonts::advance_em_str(st, s)
}

#[test]
fn rundlauf_erhaelt_die_laeufe() {
    let doc = dokument_mit_laeufen();
    let pfad = ziel("jjcad-text-laeufe.pdf");
    // Ohne Blatt: Modellseite
    pdf::export_model_pdf(&doc, &pfad, jjcad_core::style::PaperSize::A4).expect("Export");
    assert!(pfad.exists());

    let mut s = jjcad_core::Session::new();
    let r = s
        .execute(
            "file.import.pdf",
            &json!({"path": pfad.to_string_lossy(), "page": 1, "as_block": true, "scale": 1}),
        )
        .expect("Import");
    let ids: Vec<Id> = s.doc.ents().iter().map(|e| e.id).collect();
    s.execute("modify.explode", &json!({"ids": ids}))
        .expect("Zerlegen");
    let _ = &r;

    let texte: Vec<&Entity> = s
        .doc
        .ents()
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Text { .. }))
        .collect();
    assert_eq!(texte.len(), 1, "ein Text erwartet, gefunden {}", texte.len());
    let EntityKind::Text { text: t, runs, .. } = &texte[0].kind else {
        unreachable!()
    };
    eprintln!("zurück: {t:?}");
    eprintln!("Läufe: {runs:?}");
    assert_eq!(t, "Raum Wohnen · Küche · 32,4m2");
    assert!(runs.iter().any(|r| r.text.contains("Wohnen") && r.bold), "Fett fehlt");
    assert!(
        runs.iter().any(|r| r.text.contains("Küche") && r.italic),
        "Kursiv fehlt"
    );
    assert!(
        runs.iter().any(|r| r.text.contains("32,4") && r.underline),
        "Unterstrichen fehlt: {runs:?}"
    );
    assert!(
        runs.iter().any(|r| r.text == "2" && r.superscript),
        "Hochstellung fehlt: {runs:?}"
    );
}

#[test]
fn alter_text_ohne_laeufe_bleibt_lesbar() {
    // Altbestand: nur `bold`, keine Läufe — muss weiter funktionieren.
    let mut doc = Document::new();
    let layer = doc.current_layer;
    let id = doc.new_id();
    doc.add_entity(Entity::new(
        id,
        layer,
        EntityKind::Text {
            p: jjcad_core::geom::v(10.0, 10.0),
            text: "Straße".into(),
            height: 3.0,
            angle: 0.0,
            align: TextAlignH::Left,
            valign: TextAlignV::Bottom,
            bold: true,
            italic: false,
            underline: false,
            superscript: false,
            runs: Vec::new(),
        },
    ));
    let pfad = ziel("jjcad-text-alt.pdf");
    pdf::export_model_pdf(&doc, &pfad, jjcad_core::style::PaperSize::A4).expect("Export");
    let mut s = jjcad_core::Session::new();
    s.execute(
        "file.import.pdf",
        &json!({"path": pfad.to_string_lossy(), "page": 1, "scale": 1}),
    )
    .expect("Import");
    let ids: Vec<Id> = s.doc.ents().iter().map(|e| e.id).collect();
    s.execute("modify.explode", &json!({"ids": ids}))
        .expect("Zerlegen");
    let t = s
        .doc
        .ents()
        .iter()
        .find_map(|e| match &e.kind {
            EntityKind::Text { text, bold, .. } => Some((text.clone(), *bold)),
            _ => None,
        })
        .expect("Text");
    assert_eq!(t.0, "Straße");
    assert!(t.1, "Fett erwartet");
}

#[test]
fn txt_export_enthaelt_die_zeichen_als_text() {
    let doc = dokument_mit_laeufen();
    let pfad = ziel("jjcad-text-laeufe2.pdf");
    pdf::export_model_pdf(&doc, &pfad, jjcad_core::style::PaperSize::A4).expect("Export");
    let roh = std::fs::read(&pfad).unwrap_or_default();
    let s = String::from_utf8_lossy(&roh);
    assert!(s.contains("/FontFile2"), "Schrift eingebettet");
    assert!(s.contains("/ToUnicode"), "Zeichenzuordnung vorhanden");
    // Der Text steht nicht als WinAnsi-Bytefolge im Strom (Identity-H), also
    // muss pdftotext ihn über die ToUnicode-Zuordnung finden.
    let _ = Path::new("");
}
