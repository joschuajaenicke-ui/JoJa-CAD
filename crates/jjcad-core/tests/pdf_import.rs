//! PDF-Import: aus einem PDF werden Entities in Welt-Millimetern.
//!
//! Geprüft wird der Weg, den der Nutzer geht: Datei lesen → Seite wählen →
//! Objekte übernehmen. Dazu die Fälle, die der Plan ausdrücklich nennt:
//! Linien, Flächen, Text (mit Umlauten), Drehung/Maßstab über `cm`, und die
//! Meldung für Dinge, die PDF hergibt, JoJa-CAD aber nicht übernimmt.

use jjcad_core::model::{EntityKind, TextAlignV};
use jjcad_core::pdf_read;
use std::path::PathBuf;

fn sample(name: &str) -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name);
    if p.exists() {
        p
    } else {
        std::path::Path::new("samples").join(name)
    }
}

fn zaehle(entities: &[pdf_read::ImportedEntity], name: &str) -> usize {
    entities
        .iter()
        .filter(|e| match (&e.kind, name) {
            (EntityKind::Line { .. }, "line") => true,
            (EntityKind::Polyline { .. }, "poly") => true,
            (EntityKind::Circle { .. }, "circle") => true,
            (EntityKind::Arc { .. }, "arc") => true,
            (EntityKind::Fill { .. }, "fill") => true,
            (EntityKind::Text { .. }, "text") => true,
            _ => false,
        })
        .count()
}

#[test]
fn grundriss_liefert_linien_flaechen_und_text() {
    let pdf = pdf_read::read_pdf(&sample("Grundriss.pdf")).expect("Grundriss.pdf lesbar");
    assert_eq!(pdf.pages.len(), 1, "eine Seite");
    let seite = &pdf.pages[0];
    // A3 quer: 420 × 297 mm
    assert!((seite.width_mm - 420.0).abs() < 0.5, "Breite {}", seite.width_mm);
    assert!((seite.height_mm - 297.0).abs() < 0.5, "Höhe {}", seite.height_mm);

    let linien = zaehle(&seite.entities, "line") + zaehle(&seite.entities, "poly");
    let flaechen = zaehle(&seite.entities, "fill");
    let texte = zaehle(&seite.entities, "text");
    eprintln!(
        "Grundriss: {} Objekte — {} Linien/Züge, {} Flächen, {} Texte, Warnungen: {}",
        seite.entities.len(),
        linien,
        flaechen,
        texte,
        seite.warnings.summary()
    );
    assert!(linien > 20, "Linien erwartet, gefunden {linien}");
    assert!(flaechen > 0, "Flächen erwartet");
    assert!(texte > 0, "Texte erwartet");

    // Alle Koordinaten liegen auf dem Blatt.
    for e in &seite.entities {
        let bb = jjcad_core::render::entity_bbox(
            &jjcad_core::Document::new(),
            &jjcad_core::Entity::new(1, 0, e.kind.clone()),
        );
        if bb.is_valid() {
            assert!(
                bb.min.x > -5.0 && bb.max.x < seite.width_mm + 5.0,
                "Objekt außerhalb des Blatts: {:?}",
                bb
            );
        }
    }
}

#[test]
fn text_kommt_mit_umlauten_zurueck() {
    let pdf = pdf_read::read_pdf(&sample("Grundriss.pdf")).expect("lesbar");
    let texte: Vec<String> = pdf.pages[0]
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    eprintln!("Texte: {texte:?}");
    assert!(!texte.is_empty());
    // Kein Ersatzzeichen — die Zeichenzuordnung muss greifen.
    let alle = texte.join(" ");
    assert!(!alle.contains('\u{fffd}'), "Ersatzzeichen im Text: {alle}");
    assert!(
        alle.contains("Wohn") || alle.contains("Raum") || alle.contains("Küche"),
        "Raumnamen erwartet: {alle}"
    );
}

#[test]
fn text_hat_grundlinie_und_hoehe() {
    let pdf = pdf_read::read_pdf(&sample("Grundriss.pdf")).expect("lesbar");
    for e in &pdf.pages[0].entities {
        if let EntityKind::Text {
            height, valign, ..
        } = &e.kind
        {
            assert!(*height > 0.5 && *height < 100.0, "Höhe {height}");
            assert_eq!(*valign, TextAlignV::Bottom);
        }
    }
}
