//! T0210 (Turn 3): Blöcke mit dem Zeichnungsmaßstab skalieren.
//!
//! Ein Block trägt die Eigenschaft `annotative`. Ist sie gesetzt, liegt seine
//! Geometrie in Papier-Millimetern und wird beim Zeichnen mit dem
//! Beschriftungsmaßstab vervielfacht — genau wie die Begrenzungssymbole der
//! Bemaßung. Die Punktsymbole der Systembibliothek tragen sie, damit sie als
//! Block dieselbe Größe haben wie als Symbol an Punkt und Linie.

use jjcad_core::actions;
use jjcad_core::geom::{v, Vec2, Vertex};
use jjcad_core::model::{BlockDef, BlockSource, Document, Entity, EntityKind, PUNKTSYMBOL_KATEGORIE};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut doc = Document::new();
    doc.settings.input_unit = Unit::Mm;
    doc
}

/// Benutzerblock 2 × 1 Papier-mm (Rechteck um den Ursprung).
fn rechteck_block(doc: &mut Document, annotative: bool) -> String {
    let id = "usr:marke".to_string();
    doc.blocks.push(BlockDef {
        id: id.clone(),
        name: "Marke".into(),
        category: "Test".into(),
        source: BlockSource::User,
        description: String::new(),
        entities: vec![Entity::new(
            1,
            0,
            EntityKind::Polyline {
                verts: vec![
                    Vertex::new(v(-1.0, -0.5)),
                    Vertex::new(v(1.0, -0.5)),
                    Vertex::new(v(1.0, 0.5)),
                    Vertex::new(v(-1.0, 0.5)),
                ],
                closed: true,
            },
        )],
        icon: None,
        base: Vec2::ZERO,
        annotative,
    });
    id
}

fn instanz(doc: &mut Document, block: &str) -> u64 {
    let antwort = actions::execute(
        doc,
        "block.insert",
        &json!({"block": block, "pos": [1000.0, 1000.0], "rot": 0.0, "scale": 1.0}),
    )
    .expect("Block einsetzen");
    antwort["created"][0].as_u64().expect("Id")
}

#[test]
fn system_punktsymbole_skalieren_als_block_mit() {
    let doc = Document::new();
    let symbole = doc.punktsymbol_bloecke();
    assert!(symbole.len() >= 8);
    for b in symbole {
        assert!(
            b.annotative,
            "Punktsymbol „{}“ skaliert nicht mit dem Zeichnungsmaßstab",
            b.name
        );
    }
    // Andere Systemblöcke bleiben maßstäblich (1:1) — die Eigenschaft ist eine
    // bewusste Angabe, keine Vorgabe für alle Blöcke.
    let wc = doc.block("sys:wc").expect("WC");
    assert!(!wc.annotative);
}

#[test]
fn block_mit_massstab_waechst_mit_dem_beschriftungsmassstab() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let block = rechteck_block(&mut doc, true);
    let id = instanz(&mut doc, &block);
    let e = doc.entity(id).expect("Instanz").clone();

    let bbox = jjcad_core::render::entity_bbox(&doc, &e);
    assert!(
        (bbox.width() - 200.0).abs() < 1.0,
        "Breite {:.1} mm, erwartet 200 mm (2 Papier-mm bei 1:100)",
        bbox.width()
    );

    // Fang und Trefferprüfung lesen dieselbe Zerlegung — sie muss mitskalieren,
    // sonst läge der Klick neben dem sichtbaren Block.
    let segs = jjcad_core::render::entity_segments(&doc, &e);
    let max_x = segs
        .iter()
        .filter_map(|s| match s {
            jjcad_core::geom::Seg::Line(a, b) => Some(a.x.max(b.x)),
            _ => None,
        })
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (max_x - 1100.0).abs() < 1.0,
        "rechte Kante bei {max_x:.1}, erwartet 1100",
        max_x = max_x
    );

    // 1:50 → halbe Größe auf demselben Papier (Beschriftungsgröße).
    doc.settings.annotation_scale = 50.0;
    let halb = jjcad_core::render::entity_bbox(&doc, doc.entity(id).unwrap()).width();
    assert!(
        (halb - 100.0).abs() < 1.0,
        "bei 1:50 sind es 100 mm, gemessen {halb:.1}"
    );
}

#[test]
fn block_ohne_die_eigenschaft_bleibt_massstaeblich() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let block = rechteck_block(&mut doc, false);
    let id = instanz(&mut doc, &block);
    let bbox = jjcad_core::render::entity_bbox(&doc, doc.entity(id).unwrap());
    assert!(
        (bbox.width() - 2.0).abs() < 0.01,
        "ohne die Eigenschaft bleibt der Block 2 mm breit, gemessen {:.2}",
        bbox.width()
    );
}

#[test]
fn block_update_setzt_und_nimmt_die_eigenschaft() {
    let mut doc = doc_mm();
    let block = rechteck_block(&mut doc, false);
    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "annotative": true}),
    )
    .expect("Eigenschaft setzen");
    assert!(doc.block(&block).expect("Block").annotative);

    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "annotative": false}),
    )
    .expect("Eigenschaft nehmen");
    assert!(!doc.block(&block).expect("Block").annotative);

    // `block.list` nennt sie — ein Agent muss sie abfragen können.
    let liste = actions::execute(&mut doc, "block.list", &json!({})).expect("Liste");
    let eintrag = liste["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .find(|b| b["id"] == serde_json::json!(block))
        .expect("Block in der Liste");
    assert_eq!(eintrag["annotative"], serde_json::json!(false));
}

#[test]
fn alte_datei_liest_den_block_ohne_die_eigenschaft() {
    // Dateien von vor dieser Eigenschaft tragen das Feld nicht — der Block gilt
    // dann maßstäblich (1:1), nicht stillschweigend als Beschriftung.
    let roh = json!({
        "id": "usr:alt",
        "name": "Alt",
        "category": "",
        "source": "User",
        "description": "",
        "entities": []
    });
    let def: BlockDef = serde_json::from_value(roh).expect("alter Block");
    assert!(!def.annotative);
}

#[test]
fn punktsymbol_block_ist_so_gross_wie_das_symbol_am_punkt() {
    // Beide Wege — Symbol an einem Punkt (`push_punktsymbol`) und Block aus der
    // Bibliothek (`annotative`) — zeichnen dieselbe Papiergröße.
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let punkt = actions::execute(
        &mut doc,
        "draw.point",
        &json!({"p": [0.0, 0.0], "symbol": "sys:pkt-kreuz"}),
    )
    .expect("Punkt");
    let punkt_breite = jjcad_core::render::entity_bbox(
        &doc,
        doc.entity(punkt["created"][0].as_u64().unwrap()).unwrap(),
    )
    .width();

    let id = instanz(&mut doc, "sys:pkt-kreuz");
    let block_breite =
        jjcad_core::render::entity_bbox(&doc, doc.entity(id).unwrap()).width();
    assert!(
        (punkt_breite - block_breite).abs() < 0.01,
        "Symbol am Punkt {punkt_breite:.2} mm, als Block {block_breite:.2} mm"
    );
}

#[test]
fn suchbegriff_der_kategorie_bleibt_der_massgebliche() {
    // Die Kategorie ist die Zuordnung zum Punktsymbol — unabhängig davon, ob
    // der Block mit dem Maßstab skaliert (ein Benutzerblock kann beides).
    let mut doc = doc_mm();
    let block = rechteck_block(&mut doc, true);
    doc.block_mut(&block).expect("Block").category = PUNKTSYMBOL_KATEGORIE.into();
    assert!(doc.punktsymbol_bloecke().iter().any(|b| b.id == block));
}
