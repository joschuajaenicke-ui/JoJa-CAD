//! T0128 — eigene Blöcke liegen als Datei in `<blocks>/` und kommen beim Start
//! wieder. Eine Datei je Block, das Verzeichnis ist die Wahrheit.

use jjcad_core::model::{BlockDef, BlockEinheit, BlockSource, Entity, EntityKind};
use jjcad_core::session::Session;
use serde_json::json;
use std::path::PathBuf;

fn test_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jjcad-blocks-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn neue_session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

/// Rechteck zeichnen und daraus einen Block machen; gibt die Kennung zurück.
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

#[test]
fn anlegen_schreibt_datei_und_neustart_laedt_sie() {
    let dir = test_dir("anlegen");
    let mut s = neue_session();
    let bid = block_anlegen(&mut s, "Eigene Tür", &dir);
    assert!(bid.starts_with("usr:"), "Kennung muss usr: tragen: {bid}");

    // Eine Datei je Block, Name als Slug.
    let datei = dir.join("eigene-tuer.jjblk");
    assert!(datei.is_file(), "Blockdatei fehlt: {}", datei.display());

    // Kopf der Datei: Kennzeichen, Fassung, Block wie in der .jjcad.
    let roh: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&datei).unwrap()).unwrap();
    assert_eq!(roh["format"], "jjcad-block");
    assert_eq!(roh["version"], 1);
    assert_eq!(roh["block"]["id"], bid.as_str());
    assert_eq!(roh["block"]["name"], "Eigene Tür");
    assert_eq!(roh["block"]["source"], "User");
    assert_eq!(
        roh["block"]["entities"].as_array().unwrap().len(),
        1,
        "der Blockinhalt muss mit in der Datei stehen"
    );

    // „Neustart“: frische Zeichnung, Bibliothek einlesen wie die Oberfläche.
    let (geladen, meldungen) = jjcad_core::daten::lade_benutzerbloecke_mit_meldungen(&dir);
    assert!(meldungen.is_empty(), "unerwartete Meldungen: {meldungen:?}");
    let treffer = geladen
        .iter()
        .find(|b| b.id == bid)
        .expect("Block fehlt nach dem Neustart");
    assert_eq!(treffer.source, BlockSource::User);
    assert_eq!(treffer.name, "Eigene Tür");

    let mut neu = neue_session();
    neu.doc.blocks.extend(geladen);
    neu.execute("block.insert", &json!({"block": "Eigene Tür", "pos": [5, 3]}))
        .unwrap();
    let inst = neu.doc.entities.last().unwrap();
    assert!(
        matches!(inst.kind, EntityKind::BlockRef { .. }),
        "Block muss sich einfügen lassen"
    );
}

#[test]
fn zwei_bloecke_gleichen_namens_ueberschreiben_sich_nicht() {
    let dir = test_dir("gleichnamig");
    let mut s = neue_session();
    let erster = block_anlegen(&mut s, "Fenster", &dir);
    let zweiter = block_anlegen(&mut s, "Fenster", &dir);
    assert_ne!(erster, zweiter);

    assert!(dir.join("fenster.jjblk").is_file());
    assert!(
        dir.join("fenster-2.jjblk").is_file(),
        "der zweite Block braucht eine eigene Datei"
    );

    let (geladen, meldungen) = jjcad_core::daten::lade_benutzerbloecke_mit_meldungen(&dir);
    assert!(meldungen.is_empty(), "{meldungen:?}");
    assert_eq!(geladen.len(), 2, "beide Blöcke müssen geladen werden");
    assert!(geladen.iter().any(|b| b.id == erster));
    assert!(geladen.iter().any(|b| b.id == zweiter));
}

#[test]
fn beschaedigte_datei_wird_gemeldet_und_uebersprungen() {
    let dir = test_dir("beschaedigt");
    let mut s = neue_session();
    block_anlegen(&mut s, "Gut", &dir);
    std::fs::write(dir.join("kaputt.jjblk"), b"{ das ist kein JSON").unwrap();
    std::fs::write(
        dir.join("fremd.jjblk"),
        br#"{"format":"etwas-anderes","version":1,"block":{}}"#,
    )
    .unwrap();

    let (geladen, meldungen) = jjcad_core::daten::lade_benutzerbloecke_mit_meldungen(&dir);
    assert_eq!(geladen.len(), 1, "nur der heile Block darf übrig bleiben");
    assert_eq!(geladen[0].name, "Gut");
    assert_eq!(meldungen.len(), 2, "jede kaputte Datei wird gemeldet: {meldungen:?}");
    assert!(meldungen.iter().any(|m| m.contains("kaputt.jjblk")));
    assert!(meldungen.iter().any(|m| m.contains("fremd.jjblk")));
}

#[test]
fn loeschen_entfernt_die_datei() {
    let dir = test_dir("loeschen");
    let mut s = neue_session();
    let bid = block_anlegen(&mut s, "Alt", &dir);
    assert!(dir.join("alt.jjblk").is_file());

    // Solange eine Instanz in der Zeichnung steht, bleibt die Datei — der Block
    // ist in Benutzung (bestehende Regel, kein stiller Erfolg).
    let inst = s.doc.entities.last().unwrap().id;
    assert!(s
        .execute(
            "block.delete",
            &json!({"block": bid, "dir": dir.to_string_lossy()}),
        )
        .is_err());
    assert!(dir.join("alt.jjblk").is_file());

    s.execute("modify.erase", &json!({"ids": [inst]})).unwrap();
    s.execute(
        "block.delete",
        &json!({"block": bid, "dir": dir.to_string_lossy()}),
    )
    .unwrap();
    assert!(
        !dir.join("alt.jjblk").exists(),
        "Löschen im Blockmanager muss die Datei entfernen"
    );
    assert!(s.doc.blocks.iter().all(|b| b.id != bid));
    let (geladen, _) = jjcad_core::daten::lade_benutzerbloecke_mit_meldungen(&dir);
    assert!(geladen.is_empty());
}

#[test]
fn umbenennen_benennt_datei_um_und_behaelt_die_kennung() {
    let dir = test_dir("umbenennen");
    let mut s = neue_session();
    let bid = block_anlegen(&mut s, "Alt", &dir);

    s.execute(
        "block.update",
        &json!({"block": bid, "name": "Neu", "dir": dir.to_string_lossy()}),
    )
    .unwrap();

    assert!(!dir.join("alt.jjblk").exists(), "alte Datei muss weg sein");
    let datei = dir.join("neu.jjblk");
    assert!(datei.is_file(), "neue Datei fehlt");

    let roh: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&datei).unwrap()).unwrap();
    assert_eq!(roh["block"]["id"], bid.as_str(), "Kennung bleibt stabil");
    assert_eq!(roh["block"]["name"], "Neu");

    let (geladen, meldungen) = jjcad_core::daten::lade_benutzerbloecke_mit_meldungen(&dir);
    assert!(meldungen.is_empty(), "{meldungen:?}");
    assert_eq!(geladen.len(), 1);
    assert_eq!(geladen[0].id, bid);
    assert_eq!(geladen[0].name, "Neu");
}

/// T0204: Die Gruppe ist die Überschrift in der Blockbibliothek — sie wird
/// wie der Name über `block.update` geändert und steht in der Blockdatei.
#[test]
fn gruppe_wird_geaendert_und_in_der_datei_geschrieben() {
    let dir = test_dir("gruppe");
    let mut s = neue_session();
    let bid = block_anlegen(&mut s, "Tisch", &dir);

    s.execute(
        "block.update",
        &json!({
            "block": bid,
            "name": "Tisch rund",
            "category": "Möbel",
            "dir": dir.to_string_lossy(),
        }),
    )
    .unwrap();

    assert!(!dir.join("tisch.jjblk").exists(), "alte Datei muss weg sein");
    let datei = dir.join("tisch-rund.jjblk");
    assert!(datei.is_file(), "neue Datei fehlt: {}", datei.display());
    let roh: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&datei).unwrap()).unwrap();
    assert_eq!(roh["block"]["id"], bid.as_str(), "Kennung bleibt stabil");
    assert_eq!(roh["block"]["name"], "Tisch rund");
    assert_eq!(roh["block"]["category"], "Möbel");

    // Beim nächsten Start kommt die Gruppe aus der Datei zurück.
    let (geladen, meldungen) = jjcad_core::daten::lade_benutzerbloecke_mit_meldungen(&dir);
    assert!(meldungen.is_empty(), "{meldungen:?}");
    assert_eq!(geladen.len(), 1);
    assert_eq!(geladen[0].name, "Tisch rund");
    assert_eq!(geladen[0].category, "Möbel");
}

/// T0204: Auch ein Block ohne Datei (Projektblock, MCP ohne `dir`) lässt sich
/// umbenennen und in eine andere Gruppe legen — die Änderung bleibt im
/// Dokument, es wird nichts geschrieben.
#[test]
fn gruppe_ohne_verzeichnis_aendert_nur_das_dokument() {
    let mut s = neue_session();
    s.execute("draw.rect", &json!({"a": [0, 0], "b": [2, 1]}))
        .unwrap();
    let id = s.doc.entities.last().unwrap().id;
    let antwort = s
        .execute("block.create", &json!({"name": "Stuhl", "ids": [id]}))
        .unwrap();
    let bid = antwort["block"].as_str().unwrap().to_string();

    s.execute(
        "block.update",
        &json!({"block": bid, "name": "Hocker", "category": "Sitzmöbel"}),
    )
    .unwrap();

    let block = s.doc.blocks.iter().find(|b| b.id == bid).unwrap();
    assert_eq!(block.name, "Hocker");
    assert_eq!(block.category, "Sitzmöbel");
}

/// Ohne `dir` bleibt die Bibliothek unberührt — MCP-Aufrufe und Tests ohne
/// Datenwurzel dürfen nichts in das Benutzerverzeichnis schreiben.
#[test]
fn ohne_verzeichnis_wird_nichts_geschrieben() {
    let mut s = neue_session();
    s.execute("draw.rect", &json!({"a": [0, 0], "b": [2, 1]}))
        .unwrap();
    let id = s.doc.entities.last().unwrap().id;
    s.execute("block.create", &json!({"name": "Nur im Projekt", "ids": [id]}))
        .unwrap();
    // Der Block entsteht trotzdem, nur eben nicht als Datei.
    assert!(s.doc.blocks.iter().any(|b| b.name == "Nur im Projekt"));
}

/// T0217: Ein Systemblock ist schreibgeschützt — kopiert wird er als
/// **Benutzerblock** im Blocksverzeichnis und ist dann bearbeitbar. Die Kopie
/// trägt eine eigene Kennung, einen eigenen Namen und dieselbe Geometrie.
#[test]
fn kopie_eines_systemblocks_landet_als_benutzerblock() {
    let dir = test_dir("kopie");
    let mut s = neue_session();
    // Systemblock, wie ihn `persist::normalize_systembloecke` beim Start anlegt.
    let mut sys = BlockDef {
        id: "sys:pfeil".into(),
        name: "Pfeil".into(),
        category: "Punkt Symbole".into(),
        source: BlockSource::System,
        description: "Richtungspfeil".into(),
        entities: Vec::new(),
        icon: None,
        base: jjcad_core::geom::Vec2::ZERO,
        einheit: BlockEinheit::Neutral,
    };
    let mut e = Entity::new(1, s.doc.current_layer, EntityKind::Line {
        a: jjcad_core::geom::Vec2::new(0.0, 0.0),
        b: jjcad_core::geom::Vec2::new(3.0, 0.0),
    });
    e.id = 1;
    sys.entities.push(e);
    s.doc.blocks.push(sys);

    // Der Systemblock selbst bleibt schreibgeschützt.
    let abgelehnt = s.execute("block.update", &json!({"block": "sys:pfeil", "name": "Neu"}));
    assert!(abgelehnt.is_err(), "Systemblock darf nicht änderbar sein");

    let antwort = s
        .execute(
            "block.copy",
            &json!({"block": "sys:pfeil", "dir": dir.to_string_lossy()}),
        )
        .expect("Kopie muss gelingen");
    let kopie_id = antwort["block"].as_str().unwrap().to_string();
    assert!(kopie_id.starts_with("usr:"), "Kopie ist ein Benutzerblock: {kopie_id}");
    assert_eq!(antwort["name"], "Pfeil (Kopie)");

    // Das Original steht unverändert daneben.
    assert!(s.doc.blocks.iter().any(|b| b.id == "sys:pfeil" && b.source == BlockSource::System));
    let kopie = s.doc.blocks.iter().find(|b| b.id == kopie_id).expect("Kopie fehlt");
    assert_eq!(kopie.source, BlockSource::User);
    assert_eq!(kopie.name, "Pfeil (Kopie)");
    assert_eq!(kopie.category, "Punkt Symbole", "Gruppe kommt mit");
    assert_eq!(
        kopie.einheit,
        BlockEinheit::Neutral,
        "die Maßeinheit kommt mit"
    );
    assert_eq!(kopie.entities.len(), 1, "Geometrie kommt mit");
    assert_eq!(kopie.base, jjcad_core::geom::Vec2::ZERO);

    // Die Datei ist die Bibliothek: Name als Slug, Kennung im Inhalt.
    let datei = dir.join("pfeil-kopie.jjblk");
    assert!(datei.is_file(), "Blockdatei fehlt: {}", datei.display());
    let roh: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&datei).unwrap()).unwrap();
    assert_eq!(roh["block"]["id"], kopie_id.as_str());
    assert_eq!(roh["block"]["source"], "User");
    assert_eq!(roh["block"]["entities"].as_array().unwrap().len(), 1);

    // Eine zweite Kopie überschreibt die erste nicht.
    let zweite = s
        .execute("block.copy", &json!({"block": "sys:pfeil", "dir": dir.to_string_lossy()}))
        .unwrap();
    assert_eq!(zweite["name"], "Pfeil (Kopie 2)");
    assert_ne!(zweite["block"], kopie_id);
    assert!(dir.join("pfeil-kopie.jjblk").is_file());
    assert!(dir.join("pfeil-kopie-2.jjblk").is_file());

    // Und die Kopie ist als Benutzerblock bearbeitbar.
    s.execute(
        "block.update",
        &json!({"block": kopie_id, "name": "Pfeil breit", "dir": dir.to_string_lossy()}),
    )
    .expect("Kopie muss bearbeitbar sein");
    assert!(s.doc.blocks.iter().any(|b| b.name == "Pfeil breit"));
}

/// Ohne `block`-Parameter ist die Kopie ein Fehler — kein stiller Erfolg (T0038).
#[test]
fn kopie_ohne_block_ist_ein_fehler() {
    let mut s = neue_session();
    assert!(s.execute("block.copy", &json!({})).is_err());
    assert!(s.execute("block.copy", &json!({"block": "gibt-es-nicht"})).is_err());
}
