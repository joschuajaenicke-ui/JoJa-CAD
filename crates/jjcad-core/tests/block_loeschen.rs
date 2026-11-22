//! T0219 — Löschen eines Blocks.
//!
//! Ein Block lässt sich nur löschen, wenn ihn nichts mehr verwendet. Die
//! Action `block.delete` lehnt einen benutzten Block ab (`explode` fehlt) und
//! zerlegt mit `explode` vorher **alle** Instanzen — im Modell, auf Blättern
//! und in anderen Blockdefinitionen.

use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;
use std::path::PathBuf;

fn test_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jjcad-t0219-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn neue_session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

/// Rechteck zeichnen und daraus einen Benutzerblock machen.
fn block_anlegen(s: &mut Session, name: &str, dir: &PathBuf) -> String {
    s.execute("draw.rect", &json!({"a": [0, 0], "b": [2, 1]}))
        .unwrap();
    let id = s.doc.entities.last().unwrap().id;
    let antwort = s
        .execute(
            "block.create",
            &json!({"name": name, "ids": [id], "dir": dir.to_string_lossy()}),
        )
        .unwrap();
    antwort["block"].as_str().unwrap().to_string()
}

fn blockrefs(doc: &jjcad_core::model::Document, block: &str) -> usize {
    doc.entities
        .iter()
        .filter(|e| matches!(&e.kind, EntityKind::BlockRef { block: b, .. } if b == block))
        .count()
}

/// Ohne `explode` bleibt ein benutzter Block stehen — mit der Zahl der
/// Verwendungen in der Meldung (kein stiller Erfolg).
#[test]
fn benutzter_block_wird_abgelehnt_und_nennt_die_anzahl() {
    let dir = test_dir("abgelehnt");
    let mut s = neue_session();
    let bid = block_anlegen(&mut s, "Stuhl", &dir);
    // Zweite Instanz, damit die Zahl wirklich geprüft wird.
    let antwort = s
        .execute("block.insert", &json!({"block": bid, "pos": [5, 0]}))
        .unwrap();
    let zweite = antwort["created"][0].as_u64().unwrap();
    // `block.create` lässt die Instanz der Auswahl stehen — die zweite kommt
    // aus `block.insert`.
    assert_eq!(blockrefs(&s.doc, &bid), 2, "zwei Instanzen in der Zeichnung");
    assert!(s.doc.entities.iter().any(|e| e.id == zweite));

    let fehler = s
        .execute(
            "block.delete",
            &json!({"block": bid, "dir": dir.to_string_lossy()}),
        )
        .expect_err("benutzter Block darf nicht verschwinden");
    assert!(
        fehler.contains("2-mal"),
        "Meldung muss die Zahl der Verwendungen nennen: {fehler}"
    );
    assert!(s.doc.block(&bid).is_some(), "der Block bleibt stehen");
    assert!(dir.join("stuhl.jjblk").is_file(), "die Datei bleibt stehen");
}

/// Mit `explode` werden alle Instanzen zerlegt, danach fällt der Block samt
/// Datei — die Geometrie bleibt als Einzelobjekte in der Zeichnung.
#[test]
fn zerlegen_und_loeschen_laesst_die_geometrie_stehen() {
    let dir = test_dir("zerlegen");
    let mut s = neue_session();
    let bid = block_anlegen(&mut s, "Stuhl", &dir);
    let vorher = s.doc.entities.len();

    let antwort = s
        .execute(
            "block.delete",
            &json!({"block": bid, "dir": dir.to_string_lossy(), "explode": true}),
        )
        .expect("Zerlegen und Löschen");
    assert_eq!(antwort["ok"], true);
    assert!(s.doc.block(&bid).is_none(), "der Block ist gelöscht");
    assert!(!dir.join("stuhl.jjblk").exists(), "die Datei ist gelöscht");
    assert_eq!(blockrefs(&s.doc, &bid), 0, "keine Instanz mehr");
    assert!(
        s.doc.entities.len() > vorher - 1,
        "die Geometrie der Instanz steht als Einzelobjekte"
    );
    assert!(
        s.doc.entities
            .iter()
            .all(|e| !matches!(e.kind, EntityKind::BlockRef { .. })),
        "kein Blockverweis bleibt zurück"
    );
}

/// Eine Instanz auf einem Blatt zählt mit und wird im Papierraum zerlegt; der
/// Bearbeitungsraum des Aufrufers bleibt danach stehen.
#[test]
fn instanz_auf_dem_blatt_wird_mit_zerlegt() {
    let dir = test_dir("blatt");
    let mut s = neue_session();
    let bid = block_anlegen(&mut s, "Blattstuhl", &dir);
    s.execute("sheet.new", &json!({"name": "Blatt 2"})).unwrap();
    let blatt = s.doc.sheets.last().unwrap().id;
    s.set_space(Some(blatt));
    s.execute("block.insert", &json!({"block": bid, "pos": [1, 1]}))
        .unwrap();
    assert!(
        s.doc
            .sheet(blatt)
            .unwrap()
            .entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::BlockRef { .. })),
        "die Instanz liegt auf dem Blatt"
    );

    s.execute(
        "block.delete",
        &json!({"block": bid, "dir": dir.to_string_lossy(), "explode": true}),
    )
    .expect("Zerlegen und Löschen über alle Räume");
    assert!(s.doc.block(&bid).is_none());
    // Seit T0239 trägt jedes Blatt den Planrahmen als eigenen Blockverweis —
    // geprüft wird deshalb der gelöschte Block, nicht jeder Verweis.
    assert!(
        s.doc
            .sheet(blatt)
            .unwrap()
            .entities
            .iter()
            .all(|e| !matches!(&e.kind, EntityKind::BlockRef { block, .. } if block == &bid)),
        "auf dem Blatt steht kein Verweis auf den gelöschten Block mehr"
    );
    assert!(
        !s.doc.sheet(blatt).unwrap().entities.is_empty(),
        "die Blattgeometrie bleibt stehen"
    );
    assert_eq!(s.doc.space, Some(blatt), "der Raum des Aufrufers bleibt");
}

/// Ein Block, den eine andere Blockdefinition einsetzt, ist ebenfalls eine
/// Verwendung — sonst bliebe dort ein Verweis auf eine fehlende Definition.
#[test]
fn block_in_einem_block_zaehlt_als_verwendung() {
    let dir = test_dir("verschachtelt");
    let mut s = neue_session();
    let innen = block_anlegen(&mut s, "Innen", &dir);
    // Instanz von „Innen“ einsetzen und daraus den Block „Außen“ machen.
    let antwort = s
        .execute("block.insert", &json!({"block": innen, "pos": [0, 0]}))
        .unwrap();
    let inst = antwort["created"][0].as_u64().unwrap();
    let aussen = s
        .execute(
            "block.create",
            &json!({"name": "Aussen", "ids": [inst], "dir": dir.to_string_lossy()}),
        )
        .unwrap()["block"]
        .as_str()
        .unwrap()
        .to_string();
    // Eine Instanz bleibt im Modell, eine steckt im äußeren Block.
    assert_eq!(
        s.doc.block_verwendungen(&innen).len(),
        2,
        "der äußere Block verwendet den inneren"
    );

    let fehler = s
        .execute(
            "block.delete",
            &json!({"block": innen, "dir": dir.to_string_lossy()}),
        )
        .expect_err("ein verwendeter Block bleibt stehen");
    assert!(fehler.contains("2-mal"), "{fehler}");

    s.execute(
        "block.delete",
        &json!({"block": innen, "dir": dir.to_string_lossy(), "explode": true}),
    )
    .expect("Zerlegen im äußeren Block");
    assert!(s.doc.block(&innen).is_none());
    let aussen_def = s.doc.block(&aussen).expect("der äußere Block bleibt");
    let ids: Vec<Id> = aussen_def.entities.iter().map(|e| e.id).collect();
    assert!(!ids.is_empty(), "die Geometrie bleibt im äußeren Block");
    assert!(
        aussen_def
            .entities
            .iter()
            .all(|e| !matches!(e.kind, EntityKind::BlockRef { .. })),
        "kein Verweis auf den gelöschten Block"
    );
}
