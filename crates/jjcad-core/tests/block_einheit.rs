//! T0231: Maßeinheit der Blockgeometrie.
//!
//! Ein Block trägt seit T0231 die Eigenschaft `einheit` (vorher das Häkchen
//! `annotative`). **Ohne** Maßeinheit ist der Block neutral: seine Geometrie
//! liegt in Einheitsgröße und wird mit dem Zeichnungsmaßstab vervielfacht (eine
//! Einheit = ein Papier-Millimeter) — so bleiben Punktsymbole auf jedem Maßstab
//! gleich groß. Mit `Millimeter`, `Zentimeter` oder `Meter` gilt die Geometrie
//! maßstäblich (1:1).

use jjcad_core::actions;
use jjcad_core::geom::{v, Vec2, Vertex};
use jjcad_core::model::{BlockDef, BlockEinheit, BlockSource, Document, Entity, EntityKind};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut doc = Document::new();
    doc.settings.input_unit = Unit::Mm;
    doc
}

/// Benutzerblock: Rechteck 2 × 1 Blockeinheiten um den Ursprung.
fn rechteck_block(doc: &mut Document, einheit: BlockEinheit) -> String {
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
        einheit,
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

fn breite(doc: &Document, id: u64) -> f64 {
    jjcad_core::render::entity_bbox(doc, doc.entity(id).unwrap()).width()
}

/// Die Punktsymbole der Bibliothek sind **neutral** und auf Einheitsgröße 1
/// normiert: jedes Symbol liegt in einem Kästchen der Größe 1.
#[test]
fn punktsymbole_sind_auf_einheitsgroesse_eins_normiert() {
    let doc = Document::new();
    let symbole = doc.punktsymbol_bloecke();
    assert!(symbole.len() >= 8, "zu wenige Punktsymbole");
    for b in symbole {
        assert!(
            b.ist_neutral(),
            "Punktsymbol „{}“ hat die Einheit {:?} statt Neutral",
            b.name,
            b.einheit
        );
        let kasten = b.bbox();
        let gross = kasten.width().max(kasten.height());
        assert!(
            (gross - 1.0).abs() < 1e-6,
            "Symbol „{}“ misst {:.4} statt einer Einheit",
            b.name,
            gross
        );
    }
    // Andere Systemblöcke bleiben maßstäblich (Millimeter) — die Einheit ist
    // eine bewusste Angabe, keine Vorgabe für alle Blöcke.
    assert_eq!(
        doc.block("sys:wc").expect("WC").einheit,
        BlockEinheit::Millimeter
    );
}

/// Ein neutraler Block wächst mit dem Beschriftungsmaßstab: eine Blockeinheit
/// ist ein Papier-Millimeter.
#[test]
fn neutraler_block_waechst_mit_dem_beschriftungsmassstab() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let block = rechteck_block(&mut doc, BlockEinheit::Neutral);
    let id = instanz(&mut doc, &block);
    assert!(
        (breite(&doc, id) - 200.0).abs() < 1.0,
        "Breite {:.1} mm, erwartet 200 mm (2 Einheiten bei 1:100)",
        breite(&doc, id)
    );

    // Fang und Trefferprüfung lesen dieselbe Zerlegung — sie muss mitskalieren,
    // sonst läge der Klick neben dem sichtbaren Block.
    let e = doc.entity(id).expect("Instanz").clone();
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
        "rechte Kante bei {max_x:.1}, erwartet 1100"
    );

    // 1:50 → halbe Größe auf demselben Papier (Beschriftungsgröße).
    doc.settings.annotation_scale = 50.0;
    assert!((breite(&doc, id) - 100.0).abs() < 1.0, "bei 1:50 sind es 100 mm");
}

/// Ein Block **mit** Maßeinheit zeichnet maßstäblich — unabhängig vom
/// Zeichnungsmaßstab.
#[test]
fn block_mit_masseinheit_bleibt_massstaeblich() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let block = rechteck_block(&mut doc, BlockEinheit::Millimeter);
    let id = instanz(&mut doc, &block);
    assert!(
        (breite(&doc, id) - 2.0).abs() < 0.01,
        "in Millimetern bleibt der Block 2 mm breit, gemessen {:.2}",
        breite(&doc, id)
    );
    doc.settings.annotation_scale = 50.0;
    assert!(
        (breite(&doc, id) - 2.0).abs() < 0.01,
        "der Maßstab darf einen Block mit Maßeinheit nicht verändern"
    );
}

/// Zentimeter und Meter sind echte Längen: eine Einheit = 10 bzw. 1000 mm.
#[test]
fn zentimeter_und_meter_werden_umgerechnet() {
    let mut doc = doc_mm();
    let cm = rechteck_block(&mut doc, BlockEinheit::Zentimeter);
    let id_cm = instanz(&mut doc, &cm);
    assert!(
        (breite(&doc, id_cm) - 20.0).abs() < 0.01,
        "2 cm sind 20 mm, gemessen {:.2}",
        breite(&doc, id_cm)
    );

    // Dieselbe Kennung noch einmal — die erste Definition muss weg.
    doc.blocks.retain(|b| b.id != "usr:marke");
    let m = rechteck_block(&mut doc, BlockEinheit::Meter);
    let id_m = instanz(&mut doc, &m);
    assert!(
        (breite(&doc, id_m) - 2000.0).abs() < 0.5,
        "2 m sind 2000 mm, gemessen {:.2}",
        breite(&doc, id_m)
    );
}

/// Alte Dateien tragen statt `einheit` ein Häkchen `annotative`: `true` heißt
/// Papier-Millimeter (heute `Neutral`), `false`/fehlend maßstäblich in
/// Millimetern. Geschrieben wird nur noch `einheit`.
#[test]
fn alte_datei_mit_annotative_wird_umgeschrieben() {
    let alt = json!({
        "id": "usr:alt",
        "name": "Alt",
        "category": "",
        "source": "User",
        "description": "",
        "entities": [],
        "annotative": true
    });
    let def: BlockDef = serde_json::from_value(alt).expect("alter Block");
    assert_eq!(def.einheit, BlockEinheit::Millimeter, "ohne Migration Vorgabe");

    // Die Migration läuft beim Laden der Datei (`persist::from_json`).
    let doc = doc_mm();
    let mut roh = serde_json::to_value(&doc).expect("Datei");
    let mut blocks = roh["blocks"].as_array().expect("blocks").clone();
    blocks.push(json!({
        "id": "usr:alt",
        "name": "Alt",
        "category": "",
        "source": "User",
        "description": "",
        "entities": [],
        "annotative": true
    }));
    blocks.push(json!({
        "id": "usr:fest",
        "name": "Fest",
        "category": "",
        "source": "User",
        "description": "",
        "entities": []
    }));
    roh["blocks"] = serde_json::json!(blocks);
    let geladen = jjcad_core::persist::from_json(&roh.to_string()).expect("Datei laden");
    assert_eq!(
        geladen.block("usr:alt").expect("Alt").einheit,
        BlockEinheit::Neutral,
        "annotative = true ist heute „ohne Einheit“"
    );
    assert_eq!(
        geladen.block("usr:fest").expect("Fest").einheit,
        BlockEinheit::Millimeter,
        "ohne Angabe gilt maßstäblich (mm)"
    );

    // Geschrieben wird nur die Einheit — `annotative` ist verschwunden.
    let wieder: serde_json::Value =
        serde_json::from_str(&jjcad_core::persist::to_json(&geladen).unwrap()).unwrap();
    for b in wieder["blocks"].as_array().expect("blocks") {
        assert!(
            b.get("annotative").is_none(),
            "annotative darf nicht mehr in der Datei stehen"
        );
        assert!(b.get("einheit").is_some(), "einheit fehlt in der Datei");
    }
}

/// Dieselbe Migration gilt für eine **Blockdatei** (`.jjblk`, T0128) — sie
/// wird beim Start der Benutzerbibliothek gelesen.
#[test]
fn alte_blockdatei_bekommt_ihre_einheit() {
    let dir = std::env::temp_dir().join(format!("jjcad-blockeinheit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pfad = dir.join("alt.jjblk");
    std::fs::write(
        &pfad,
        serde_json::json!({
            "format": "jjcad-block",
            "version": 1,
            "block": {
                "id": "usr:alt",
                "name": "Alt",
                "category": "Punkt Symbole",
                "source": "User",
                "description": "",
                "entities": [],
                "annotative": true
            }
        })
        .to_string(),
    )
    .unwrap();
    let datei = jjcad_core::bloecke::lies(&pfad).expect("Blockdatei lesbar");
    assert_eq!(datei.block.einheit, BlockEinheit::Neutral);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `block.update` setzt die Einheit (Schlüssel `neutral`/`mm`/`cm`/`m`), das
/// alte Argument `annotative` gilt weiter, und `block.list` nennt sie.
#[test]
fn block_update_setzt_die_einheit() {
    let mut doc = doc_mm();
    let block = rechteck_block(&mut doc, BlockEinheit::Millimeter);

    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "einheit": "neutral"}),
    )
    .expect("Einheit setzen");
    assert_eq!(doc.block(&block).expect("Block").einheit, BlockEinheit::Neutral);

    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "einheit": "cm"}),
    )
    .expect("Einheit setzen");
    assert_eq!(
        doc.block(&block).expect("Block").einheit,
        BlockEinheit::Zentimeter
    );

    // Unbekannte Einheit ist ein Fehler, kein stiller Vorgabewert (T0038).
    let schlecht = actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "einheit": "zoll"}),
    );
    assert!(schlecht.is_err(), "„zoll“ darf nicht durchgehen");

    // Altes Argument derselben Angabe.
    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "annotative": true}),
    )
    .expect("altes Argument");
    assert_eq!(doc.block(&block).expect("Block").einheit, BlockEinheit::Neutral);

    // `block.list` nennt die Einheit als Schlüssel.
    let liste = actions::execute(&mut doc, "block.list", &json!({})).expect("Liste");
    let eintrag = liste["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .find(|b| b["id"] == serde_json::json!(block))
        .expect("Block in der Liste")
        .clone();
    assert_eq!(eintrag["einheit"], serde_json::json!("neutral"));
}

/// Beide Wege — das Symbol an einem Punkt und der Block aus der Bibliothek —
/// zeichnen dieselbe Größe.
#[test]
fn punktsymbol_block_ist_so_gross_wie_das_symbol_am_punkt() {
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
    let block_breite = breite(&doc, id);
    assert!(
        (punkt_breite - block_breite).abs() < 0.01,
        "Symbol am Punkt {punkt_breite:.2} mm, als Block {block_breite:.2} mm"
    );
}

/// Die Kategorie ordnet den Block den Punktsymbolen zu, unabhängig von der
/// Einheit — ein Benutzerblock darf beides sein.
#[test]
fn suchbegriff_der_kategorie_bleibt_der_massgebliche() {
    let mut doc = doc_mm();
    let block = rechteck_block(&mut doc, BlockEinheit::Meter);
    doc.block_mut(&block).expect("Block").category =
        jjcad_core::model::PUNKTSYMBOL_KATEGORIE.into();
    assert!(doc.punktsymbol_bloecke().iter().any(|b| b.id == block));
}
