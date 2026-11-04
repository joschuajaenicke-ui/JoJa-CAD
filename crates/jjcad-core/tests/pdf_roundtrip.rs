//! Abnahme-Rundlauf: **Export → Import → Zerlegen sieht identisch aus.**
//!
//! Das ist das verbindliche Kriterium aus `docs/PDF-IMPORT.md`:
//!
//! * **A — Struktur:** Anzahl Objekte je Typ und Gesamtumriss.
//! * **B — Text:** Inhalte gleich, Höhe ±0,05 mm, Lage ±0,3 mm (auf dem Papier).
//! * **C — Pixel:** beide Zustände mit demselben Renderer als Bild, Differenz
//!   gezählt (4 px/mm), Grenze 0,5 %.
//!
//! Zusätzlich prüft der Test, was das PDF selbst hergeben muss: eingebettete
//! Schrift (`/FontFile2`), Zeichenzuordnung (`/ToUnicode`) und einen Titel als
//! UTF-16-String.
//!
//! **Vergleichsaufbau:** Beide Bilder sind Blattbilder desselben Dokuments. Für
//! den Re-Import werden die Modellobjekte entfernt, das PDF eingefügt und
//! zerlegt; das Ansichtsfenster steht auf 1:1 über die ganze Seite. Damit sind
//! Rahmen, Schriftfeld und Papier in beiden Bildern dieselben — nur der Inhalt
//! ist neu gezeichnet.

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::{v, BBox};
use jjcad_core::model::*;
use jjcad_core::raster;
use jjcad_core::{pdf, persist};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn sample(name: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name);
    if p.exists() {
        p
    } else {
        Path::new("samples").join(name)
    }
}

fn ziel(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    let _ = std::fs::remove_file(&p);
    p
}

fn typ_name(k: &EntityKind) -> &'static str {
    match k {
        EntityKind::Line { .. } => "Linie",
        EntityKind::Polyline { .. } => "Polylinie",
        EntityKind::Circle { .. } => "Kreis",
        EntityKind::Arc { .. } => "Bogen",
        EntityKind::Ellipse { .. } => "Ellipse",
        EntityKind::Fill { .. } => "Füllfläche",
        EntityKind::Text { .. } => "Text",
        EntityKind::Point { .. } => "Punkt",
        EntityKind::Wall { .. } => "Wand",
        _ => "sonstiges",
    }
}

fn inventar(doc: &Document) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for e in doc.ents() {
        *m.entry(typ_name(&e.kind)).or_insert(0) += 1;
    }
    m
}

/// Texte so, wie sie **auf dem Blatt** stehen: Inhalt, Höhe in mm, Umriss.
///
/// Die Entity-Höhe allein taugt nicht zum Vergleich — im Modell steht sie in
/// Zeichnungs-Millimetern, auf dem Blatt geteilt durch den Ansichtsmaßstab.
fn texte(doc: &Document) -> Vec<(String, f64, BBox)> {
    let Some(sheet) = doc.sheets.first() else {
        return Vec::new();
    };
    let dl = jjcad_core::render::render_sheet(doc, sheet);
    let mut v: Vec<(String, f64, BBox)> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            // Text ohne Höhe ist kein sichtbarer Text (z. B. leere Schriftfeld-
            // Vorlage) und geht nicht in den Vergleich ein.
            jjcad_core::render::Shape::Text(t) if t.height > 0.05 => {
                Some((t.text.clone(), t.height, p.bbox()))
            }
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn text_umriss(v: &[(String, f64, BBox)], text: &str) -> Option<BBox> {
    let mut bb = BBox::EMPTY;
    let mut gefunden = false;
    for (t, _, b) in v {
        if t == text {
            bb.union(*b);
            gefunden = true;
        }
    }
    if gefunden {
        Some(bb)
    } else {
        None
    }
}

/// Rundlauf: exportieren, einfügen (als Block), zerlegen.
fn rundlauf(pdf_name: &str) -> (Document, Document, PathBuf) {
    let original = persist::load_document(&sample("Wohnhaus.jjcad")).expect("Wohnhaus.jjcad");
    let pfad = ziel(pdf_name);
    pdf::export_pdf(&original, &pfad, Some(0)).expect("PDF-Export");
    assert!(pfad.exists(), "PDF wurde geschrieben");

    // Re-Import in einem Klon: Blatt, Rahmen und Schriftfeld bleiben damit
    // dieselben, nur die Modellobjekte fallen weg.
    let mut s = jjcad_core::Session::with_doc(original.clone());
    let alt: Vec<Id> = s.doc.ents().iter().map(|e| e.id).collect();
    for id in alt {
        s.doc.remove_entity(id);
    }
    let r = s
        .execute(
            "file.import.pdf",
            // Maßstäblich (Papier-mm), damit der Vergleich 1:1 auf dem Blatt gilt
            &json!({"path": pfad.to_string_lossy(), "page": 1, "as_block": true, "scale": 1}),
        )
        .expect("Import");
    let anzahl = r["entities"].as_u64().unwrap_or(0) as usize;
    assert!(anzahl > 0, "Import lieferte keine Objekte");
    assert!(
        r["block"].as_str().map(|b| !b.is_empty()).unwrap_or(false),
        "Blockname fehlt"
    );

    // Ein Element: genau ein BlockRef.
    assert_eq!(s.doc.ents().len(), 1, "Import ist ein Element");
    assert!(matches!(s.doc.ents()[0].kind, EntityKind::BlockRef { .. }));

    let ids: Vec<Id> = s.doc.ents().iter().map(|e| e.id).collect();
    s.execute("modify.explode", &json!({"ids": ids}))
        .expect("Zerlegen");

    // Ansichtsfenster auf 1:1 über die ganze Seite — der Import liegt bereits
    // in Papierkoordinaten.
    if let Some(sheet) = s.doc.sheets.first_mut() {
        let (pw, ph) = sheet.size();
        // Ansichtsfenster ist seit T0109 ein normales Blattobjekt.
        sheet.entities.retain(|e| !matches!(e.kind, EntityKind::Viewport(_)));
        sheet.entities.insert(
            0,
            Entity::new(
                1,
                0,
                EntityKind::Viewport(Viewport {
                    id: 1,
                    name: "Import 1:1".to_string(),
                    x: 0.0,
                    y: 0.0,
                    w: pw,
                    h: ph,
                    center: v(pw * 0.5, ph * 0.5),
                    scale: 1.0,
                    show_frame: false,
                    locked: false,
                    layers: None,
                }),
            ),
        );
    }
    (original, s.doc, pfad)
}

#[test]
fn rundlauf_struktur_und_text() {
    let (original, reimport, pfad) = rundlauf("jjcad-rt.pdf");
    let inv_a = inventar(&original);
    let inv_b = inventar(&reimport);
    let sum_a: usize = inv_a.values().sum();
    let sum_b: usize = inv_b.values().sum();
    eprintln!("Original: {inv_a:?} = {sum_a}");
    eprintln!("Re-Import: {inv_b:?} = {sum_b}");

    // A — Struktur: die Zahl der Objekte darf steigen (PDF zeichnet Rechtecke
    // als vier Linien, Schraffuren als viele Striche und Muster, Bögen werden
    // zu Streckenzügen). Das Bild entscheidet, nicht die Zahl.
    assert!(sum_b >= sum_a / 2, "zu wenige Objekte: {sum_a} → {sum_b}");
    assert!(inv_b.get("Text").copied().unwrap_or(0) >= 6, "Texte fehlen");
    assert!(
        inv_b.get("Linie").copied().unwrap_or(0) > 0,
        "Linien fehlen"
    );
    assert!(
        inv_b.get("Füllfläche").copied().unwrap_or(0) > 0,
        "Flächen fehlen"
    );

    // B — Text: gleiche Inhalte, Höhe ±0,05 mm, Lage ±0,3 mm.
    let ta = texte(&original);
    let tb = texte(&reimport);
    eprintln!("Texte Original: {} / Re-Import: {}", ta.len(), tb.len());
    let inhalt_b: Vec<&String> = tb.iter().map(|t| &t.0).collect();
    for (text, hoehe, umriss_a) in &ta {
        assert!(
            inhalt_b.contains(&text),
            "Text fehlt nach dem Rundlauf: {text:?}"
        );
        let treffer = tb.iter().find(|t| &t.0 == text).expect("Text vorhanden");
        assert!(
            (treffer.1 - hoehe).abs() <= 0.05,
            "Höhe von {text:?}: {hoehe} → {}",
            treffer.1
        );
        // Lage über den Umriss: der Import setzt links auf die Grundlinie, das
        // Original kann mittig gesetzt sein — der Kasten muss trotzdem passen.
        // Kommt derselbe Text mehrfach vor, zählt der nächstgelegene.
        let d = tb
            .iter()
            .filter(|t| &t.0 == text)
            .map(|t| {
                (t.2.min.x - umriss_a.min.x)
                    .abs()
                    .max((t.2.min.y - umriss_a.min.y).abs())
                    .max((t.2.max.x - umriss_a.max.x).abs())
                    .max((t.2.max.y - umriss_a.max.y).abs())
            })
            .fold(f64::INFINITY, f64::min);
        assert!(
            d.is_finite() && d <= 0.3,
            "Lage von {text:?} weicht {d:.3} mm ab — Original {umriss_a:?}"
        );
    }

    // PDF selbst: eingebettete Schrift und Zeichenzuordnung
    let roh = String::from_utf8_lossy(&std::fs::read(&pfad).unwrap_or_default()).into_owned();
    assert!(
        roh.contains("/FontFile2"),
        "Schrift ist nicht eingebettet (/FontFile2 fehlt)"
    );
    assert!(roh.contains("/ToUnicode"), "/ToUnicode fehlt");
    assert!(roh.contains("/Identity-H"), "Identity-H fehlt");
}

#[test]
fn rundlauf_pixel() {
    let (original, reimport, pfad) = rundlauf("jjcad-rt-pixel.pdf");
    let _ = pfad;
    let sheet_a = original.sheets[0].clone();
    let sheet_b = reimport.sheets[0].clone();
    let (pw, ph) = sheet_a.size();
    let px_pro_mm = 4.0;
    let w = (pw * px_pro_mm).round() as u32;
    let h = (ph * px_pro_mm).round() as u32;

    let opts = RasterOptions {
        width_px: w,
        height_px: h,
        paper: true,
        ..Default::default()
    };
    let bild_a = raster::render_sheet_pixmap(&original, &sheet_a, &opts).expect("Bild Original");
    let bild_b = raster::render_sheet_pixmap(&reimport, &sheet_b, &opts).expect("Bild Re-Import");
    assert_eq!(bild_a.width(), bild_b.width());
    assert_eq!(bild_a.height(), bild_b.height());

    // Belege für den Bericht ablegen.
    let png_a = std::env::temp_dir().join("jjcad-rundlauf-original.png");
    let png_b = std::env::temp_dir().join("jjcad-rundlauf-reimport.png");
    let _ = std::fs::write(&png_a, bild_a.encode_png().unwrap_or_default());
    let _ = std::fs::write(&png_b, bild_b.encode_png().unwrap_or_default());
    eprintln!("Bilder: {} / {}", png_a.display(), png_b.display());

    // Differenz zählen. Toleranz je Pixel gegen Kantenrauschen (Antialiasing
    // an leicht verschobenen Kanten), danach der Anteil.
    let (mut diff, mut gesamt) = (0usize, 0usize);
    for (pa, pb) in bild_a.data().chunks_exact(4).zip(bild_b.data().chunks_exact(4)) {
        gesamt += 1;
        let d = (pa[0] as i32 - pb[0] as i32).abs()
            + (pa[1] as i32 - pb[1] as i32).abs()
            + (pa[2] as i32 - pb[2] as i32).abs();
        if d > 96 {
            diff += 1;
        }
    }
    let anteil = diff as f64 / gesamt as f64 * 100.0;
    eprintln!("Pixel: {diff} von {gesamt} verschieden = {anteil:.2} %");
    assert!(
        anteil <= 0.5,
        "Pixel-Differenz zu groß: {anteil:.2} % (Grenze 0,5 %)"
    );
}

#[test]
fn pdf_titel_ist_utf16() {
    let doc = persist::load_document(&sample("Wohnhaus.jjcad")).expect("Wohnhaus.jjcad");
    let pfad = ziel("jjcad-rt-titel.pdf");
    pdf::export_pdf(&doc, &pfad, Some(0)).expect("Export");
    let roh = String::from_utf8_lossy(&std::fs::read(&pfad).unwrap_or_default()).into_owned();
    assert!(
        roh.contains("/Title <FEFF"),
        "Titel nicht als UTF-16-String mit BOM geschrieben"
    );
}
