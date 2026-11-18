//! T0137 — der Blockeditor ist ein dritter Bearbeitungsraum.
//!
//! Geprüft wird über die Befehlsschicht: `set_block_space` lenkt `ents()` und
//! `ents_mut()` auf die Blockdefinition, alle Actions arbeiten dadurch ohne
//! zweiten Weg dort. Systemblöcke sind schreibgeschützt, ein Raumwechsel
//! schließt den Blockeditor, und Undo nimmt eine Blockänderung zurück.

use jjcad_core::model::*;
use jjcad_core::session::Session;
use serde_json::json;

/// Zeichnung mit einem bearbeitbaren Projektblock (`prj:stuhl`) und einem
/// Systemblock (`sys:wc`).
fn session_mit_bloecken() -> Session {
    let mut s = Session::new();
    let mut linie = Entity::new(
        1,
        s.doc.current_layer,
        EntityKind::Line {
            a: jjcad_core::geom::v(-100.0, 0.0),
            b: jjcad_core::geom::v(100.0, 0.0),
        },
    );
    linie.by_block = true;
    s.doc.blocks.push(BlockDef {
        id: "prj:stuhl".into(),
        name: "Stuhl".into(),
        category: "Projekt".into(),
        source: BlockSource::Project,
        description: "Prüfblock".into(),
        entities: vec![linie],
        icon: None,
        base: jjcad_core::geom::v(0.0, 0.0),
        annotative: false,
    });
    s
}

fn linie(s: &mut Session, x: f64) {
    s.execute(
        "draw.line",
        &json!({"a": [x, 0.0], "b": [x, 500.0]}),
    )
    .expect("Linie im Blockraum");
}

/// Zeichnen im Blockeditor landet in der Blockdefinition, nicht im Modell.
#[test]
fn blockraum_zeichnet_in_die_blockdefinition() {
    let mut s = session_mit_bloecken();
    assert!(s.doc.set_block_space(Some("prj:stuhl")));
    assert!(s.doc.in_block_space(), "der Blockeditor ist offen");
    assert_eq!(s.doc.ents().len(), 1, "der Block bringt eine Linie mit");

    linie(&mut s, 10.0);

    assert_eq!(
        s.doc.block("prj:stuhl").unwrap().entities.len(),
        2,
        "die neue Linie gehört zur Blockdefinition"
    );
    assert!(
        s.doc.entities.is_empty(),
        "die Zeichnung darf dabei unberührt bleiben"
    );
    assert_eq!(s.doc.ents().len(), 2, "der aktive Raum zeigt den Block");
}

/// Ein Systemblock ist schreibgeschützt — `set_block_space` lehnt ihn ab.
#[test]
fn systemblock_ist_nicht_bearbeitbar() {
    let mut s = session_mit_bloecken();
    assert!(!s.doc.set_block_space(Some("sys:wc")));
    assert!(!s.doc.in_block_space());
    assert!(
        !s.doc.set_block_space(Some("gibt.es.nicht")),
        "eine unbekannte Kennung öffnet keinen Editor"
    );
}

/// Modell/Papierraum und Blockeditor schließen sich aus.
#[test]
fn raumwechsel_schliesst_den_blockraum() {
    let mut s = session_mit_bloecken();
    let blatt = s.doc.sheets.first().map(|b| b.id).expect("Blatt");
    assert!(s.doc.set_block_space(Some("prj:stuhl")));
    s.doc.set_space(Some(blatt));
    assert!(!s.doc.in_block_space(), "der Papierraum verdrängt den Block");
    assert_eq!(s.doc.space, Some(blatt));

    assert!(s.doc.set_block_space(Some("prj:stuhl")));
    assert_eq!(s.doc.space, None, "der Blockeditor ist kein Papierraum");
    assert!(s.doc.in_block_space());
}

/// Undo nimmt eine Änderung im Blockeditor zurück wie jede andere.
#[test]
fn undo_nimmt_die_blockaenderung_zurueck() {
    let mut s = session_mit_bloecken();
    assert!(s.doc.set_block_space(Some("prj:stuhl")));
    linie(&mut s, 10.0);
    linie(&mut s, 20.0);
    assert_eq!(s.doc.block("prj:stuhl").unwrap().entities.len(), 3);

    assert!(s.undo(), "es gibt einen Undo-Schritt");
    assert_eq!(
        s.doc.block("prj:stuhl").unwrap().entities.len(),
        2,
        "Undo wirkt auf die Blockdefinition"
    );
    assert!(
        s.doc.in_block_space(),
        "der Bearbeitungsraum bleibt beim Undo erhalten"
    );
}

/// Ein reiner Raumwechsel erzeugt keinen Undo-Schritt und meldet die Zeichnung
/// nicht als geändert.
#[test]
fn raumwechsel_ist_kein_undo_schritt() {
    let mut s = session_mit_bloecken();
    s.doc.modified = false;
    assert!(s.doc.set_block_space(Some("prj:stuhl")));
    s.doc.set_block_space(None);
    assert!(!s.history.can_undo(), "der Wechsel ist Bedienzustand");
    assert!(!s.doc.modified);
}
