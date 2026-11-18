//! Plansatz eines Blatts (T0218): Häkchen in der Datei, Export als eine
//! mehrseitige PDF.
//!
//! Der Blattmanager setzt das Häkchen über `sheet.update`; damit gehört die
//! Auswahl zur Zeichnung und überlebt Speichern und Laden. Exportiert wird
//! ausschließlich die Auswahl — in der Reihenfolge der Blattliste.

use jjcad_core::pdf;
use jjcad_core::session::Session;
use serde_json::json;

fn session_mit_drei_blaettern() -> Session {
    let mut sess = Session::new();
    sess.execute("sheet.new", &json!({"name": "Blatt 2"}))
        .unwrap();
    sess.execute("sheet.new", &json!({"name": "Blatt 3"}))
        .unwrap();
    assert_eq!(sess.doc.sheets.len(), 3);
    sess
}

/// Das Häkchen steht im Blatt und in der gespeicherten Datei.
#[test]
fn plansatz_haken_ueberlebt_speichern_und_laden() {
    let mut sess = session_mit_drei_blaettern();
    let ziel = sess.doc.sheets[1].id;
    sess.execute("sheet.update", &json!({"sheet": ziel, "plansatz": true}))
        .expect("sheet.update");
    assert!(sess.doc.sheet(ziel).unwrap().plansatz);
    assert!(!sess.doc.sheets[0].plansatz, "andere Blätter bleiben draußen");

    let pfad = std::env::temp_dir().join("jjcad-t0218-plansatz.jjcad");
    jjcad_core::persist::save_document(&mut sess.doc, &pfad).expect("speichern");
    let geladen = jjcad_core::persist::load_document(&pfad).expect("laden");
    let g = geladen.sheet(ziel).expect("Blatt nach dem Laden");
    assert!(g.plansatz, "das Häkchen fehlt in der Datei");
    let _ = std::fs::remove_file(&pfad);
}

/// Nur die Blätter mit Häkchen landen in der PDF — als eigene Seiten.
#[test]
fn export_schreibt_nur_die_gewaehlten_blaetter() {
    let mut sess = session_mit_drei_blaettern();
    // Das mittlere Blatt bleibt draußen, die beiden anderen kommen mit.
    let ids: Vec<u64> = sess.doc.sheets.iter().map(|s| s.id).collect();
    for (i, id) in ids.iter().enumerate() {
        let drin = i != 1;
        sess.execute("sheet.update", &json!({"sheet": id, "plansatz": drin}))
            .unwrap();
    }
    sess.execute(
        "sheet.update",
        &json!({"sheet": ids[0], "paper": "A4"}),
    )
    .unwrap();

    let pfad = std::env::temp_dir().join("jjcad-t0218-plansatz.pdf");
    let auswahl = vec![ids[0], ids[2]];
    pdf::export_sheets_pdf(&sess.doc, &pfad, &auswahl).expect("Plansatz-PDF");
    let bytes = std::fs::read(&pfad).expect("PDF lesen");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/Count 2"),
        "die PDF muss genau zwei Seiten haben:\n{}",
        &text[..text.len().min(400)]
    );
    // A4 (210 mm) steht im MediaBox des ersten Blatts, A3 (420 mm) im zweiten.
    let a4 = text.find("595.276").expect("A4-Seite fehlt");
    let a3 = text.find("1190.551").expect("A3-Seite fehlt");
    assert!(
        a4 < a3,
        "die Seiten müssen in Blattreihenfolge stehen (A4 zuerst)"
    );
    let _ = std::fs::remove_file(&pfad);
}

/// Eine leere Auswahl ist ein Fehler, keine PDF ohne Seite.
#[test]
fn leerer_plansatz_ist_ein_fehler() {
    let sess = session_mit_drei_blaettern();
    let pfad = std::env::temp_dir().join("jjcad-t0218-leer.pdf");
    let err = pdf::export_sheets_pdf(&sess.doc, &pfad, &[]).expect_err("leerer Plansatz");
    assert!(err.contains("keine Blätter"), "Meldung: {err}");
    assert!(!pfad.exists(), "es darf keine Datei entstehen");
}
