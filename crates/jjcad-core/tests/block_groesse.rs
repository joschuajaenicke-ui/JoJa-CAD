//! T0231: Größe der Blockgeometrie — feste Größe oder Papiergröße.
//!
//! Ein Block ist entweder ein **reales Objekt** mit fester Größe (Geometrie in
//! Zeichnungs-Millimetern, 1:1 eingefügt: ein Bett bleibt 2 m lang) oder ein
//! **Symbol in Papiergröße** (Geometrie in Papier-Millimetern, wächst mit dem
//! Zeichnungsmaßstab: eine Kote bleibt auf jedem Maßstab gleich groß). Die
//! eine Rechnung ist `BlockDef::faktor(annot_scale)`.

use jjcad_core::actions;
use jjcad_core::geom::{v, Vec2, Vertex};
use jjcad_core::model::{BlockDef, BlockGroesse, BlockSource, Document, Entity, EntityKind};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut doc = Document::new();
    doc.settings.input_unit = Unit::Mm;
    doc
}

/// Benutzerblock: Rechteck 2 × 1 Blockeinheiten um den Ursprung.
fn rechteck_block(doc: &mut Document, groesse: BlockGroesse) -> String {
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
        groesse,
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

/// Die Punktsymbole der Bibliothek haben **Papiergröße**: ihre Maße sind die
/// Millimeter, die auf dem Papier gedruckt werden.
#[test]
fn punktsymbole_haben_papiergroesse_in_gedruckter_groesse() {
    let doc = Document::new();
    let symbole = doc.punktsymbol_bloecke();
    assert!(symbole.len() >= 8, "zu wenige Punktsymbole");
    for b in symbole {
        assert!(
            b.ist_papiergroesse(),
            "Punktsymbol „{}“ hat die Größe {:?}",
            b.name,
            b.groesse
        );
        let kasten = b.bbox();
        let gross = kasten.width().max(kasten.height());
        assert!(
            gross >= 0.8 && gross <= 4.0,
            "Symbol „{}“ misst {:.2} mm — Beschreibungen nennen 0,9–3,4 mm",
            b.name,
            gross
        );
    }
    // Die gedruckte Größe steht so in der Beschreibung.
    let kasten = |id: &str| {
        let b = doc.block(id).expect("Symbol");
        let k = b.bbox();
        k.width().max(k.height())
    };
    assert!((kasten("sys:pkt-pfeil") - 3.4).abs() < 1e-6, "Pfeil 3,4 mm");
    assert!((kasten("sys:pkt-kreuz") - 2.2).abs() < 1e-6, "Kreuz 2,2 mm");
    assert!((kasten("sys:pkt-punkt") - 1.2).abs() < 1e-6, "Punkt 1,2 mm");

    // Reale Objekte bleiben maßstäblich — die Größe ist eine bewusste Angabe.
    let wc = doc.block("sys:wc").expect("WC");
    assert_eq!(wc.groesse, BlockGroesse::Fest);
    assert!(wc.bbox().width() > 300.0, "ein WC ist ein echtes Objekt");
}

/// Ein Block **fester Größe** zeichnet 1:1 — der Maßstab ändert nichts.
#[test]
fn fester_block_bleibt_in_jedem_massstab_gleich() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let block = rechteck_block(&mut doc, BlockGroesse::Fest);
    let id = instanz(&mut doc, &block);
    assert!(
        (breite(&doc, id) - 2.0).abs() < 0.01,
        "2 mm bleiben 2 mm, gemessen {:.2}",
        breite(&doc, id)
    );
    doc.settings.annotation_scale = 50.0;
    assert!(
        (breite(&doc, id) - 2.0).abs() < 0.01,
        "1:50 darf einen Block fester Größe nicht verändern"
    );
}

/// Ein Block in **Papiergröße** wächst mit dem Beschriftungsmaßstab: 3 Papier-
/// Millimeter sind bei 1:100 300 Modell-Millimeter, bei 1:50 noch 150.
#[test]
fn papierblock_waechst_mit_dem_beschriftungsmassstab() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let block = rechteck_block(&mut doc, BlockGroesse::Papier);
    // Gedruckte Breite 3 mm: das Rechteck ist 2 Einheiten breit, also ×1,5.
    let antwort = actions::execute(
        &mut doc,
        "block.insert",
        &json!({"block": block, "pos": [1000.0, 1000.0], "rot": 0.0, "scale": 1.5}),
    )
    .expect("Block einsetzen");
    let id = antwort["created"][0].as_u64().expect("Id");
    assert!(
        (breite(&doc, id) - 300.0).abs() < 1.0,
        "Breite {:.1} mm, erwartet 300 mm (3 Papier-mm bei 1:100)",
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
        (max_x - 1150.0).abs() < 1.0,
        "rechte Kante bei {max_x:.1}, erwartet 1150 (Blockmitte 1000 + 1,5 mm)"
    );

    // 1:50 → halbe Größe auf demselben Papier.
    doc.settings.annotation_scale = 50.0;
    assert!(
        (breite(&doc, id) - 150.0).abs() < 1.0,
        "bei 1:50 sind es 150 mm, gemessen {:.1}",
        breite(&doc, id)
    );
}

/// Alte Dateien tragen `einheit` (T0231, Turn 2) oder das Häkchen `annotative`
/// (T0210): „neutral“/`true` heißt Papiergröße, jede echte Einheit feste Größe.
/// Geschrieben wird nur noch `groesse`.
#[test]
fn alte_datei_bekommt_ihre_groesse() {
    let doc = doc_mm();
    let mut roh = serde_json::to_value(&doc).expect("Datei");
    let mut blocks = roh["blocks"].as_array().expect("blocks").clone();
    blocks.push(json!({
        "id": "usr:neutral", "name": "Neutral", "category": "", "source": "User",
        "description": "", "entities": [], "einheit": "Neutral"
    }));
    blocks.push(json!({
        "id": "usr:mm", "name": "Millimeter", "category": "", "source": "User",
        "description": "", "entities": [], "einheit": "Millimeter"
    }));
    blocks.push(json!({
        "id": "usr:annotative", "name": "Häkchen", "category": "", "source": "User",
        "description": "", "entities": [], "annotative": true
    }));
    blocks.push(json!({
        "id": "usr:alt", "name": "Alt", "category": "", "source": "User",
        "description": "", "entities": []
    }));
    roh["blocks"] = serde_json::json!(blocks);

    let geladen = jjcad_core::persist::from_json(&roh.to_string()).expect("Datei laden");
    let groesse = |id: &str| geladen.block(id).expect("Block").groesse;
    assert_eq!(groesse("usr:neutral"), BlockGroesse::Papier);
    assert_eq!(groesse("usr:mm"), BlockGroesse::Fest);
    assert_eq!(groesse("usr:annotative"), BlockGroesse::Papier);
    assert_eq!(groesse("usr:alt"), BlockGroesse::Fest, "ohne Angabe 1:1");

    // Geschrieben wird nur die Größe.
    let wieder: serde_json::Value =
        serde_json::from_str(&jjcad_core::persist::to_json(&geladen).unwrap()).unwrap();
    for b in wieder["blocks"].as_array().expect("blocks") {
        assert!(
            b.get("annotative").is_none() && b.get("einheit").is_none(),
            "die alten Felder dürfen nicht mehr in der Datei stehen"
        );
        assert!(b.get("groesse").is_some(), "groesse fehlt in der Datei");
    }
}

/// Dieselbe Migration gilt für eine **Blockdatei** (`.jjblk`, T0128).
#[test]
fn alte_blockdatei_bekommt_ihre_groesse() {
    let dir = std::env::temp_dir().join(format!("jjcad-blockgroesse-{}", std::process::id()));
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
    assert_eq!(datei.block.groesse, BlockGroesse::Papier);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `block.update` setzt die Größe (Schlüssel `fest`/`papier`), die alten
/// Argumente `einheit`/`annotative` gelten weiter, und `block.list` nennt sie.
#[test]
fn block_update_setzt_die_groesse() {
    let mut doc = doc_mm();
    let block = rechteck_block(&mut doc, BlockGroesse::Fest);

    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "groesse": "papier"}),
    )
    .expect("Größe setzen");
    assert_eq!(doc.block(&block).expect("Block").groesse, BlockGroesse::Papier);

    // Unbekannte Größe ist ein Fehler, kein stiller Vorgabewert (T0038).
    let schlecht = actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "groesse": "meterware"}),
    );
    assert!(schlecht.is_err(), "„meterware“ darf nicht durchgehen");

    // Alte Argumente derselben Angabe.
    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "annotative": false}),
    )
    .expect("altes Häkchen");
    assert_eq!(doc.block(&block).expect("Block").groesse, BlockGroesse::Fest);
    actions::execute(
        &mut doc,
        "block.update",
        &json!({"block": block, "einheit": "neutral"}),
    )
    .expect("alte Einheit");
    assert_eq!(doc.block(&block).expect("Block").groesse, BlockGroesse::Papier);

    // `block.list` nennt die Größe als Schlüssel.
    let liste = actions::execute(&mut doc, "block.list", &json!({})).expect("Liste");
    let eintrag = liste["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .find(|b| b["id"] == serde_json::json!(block))
        .expect("Block in der Liste")
        .clone();
    assert_eq!(eintrag["groesse"], serde_json::json!("papier"));
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
    assert!(
        (block_breite - 220.0).abs() < 5.0,
        "2,2 Papier-mm bei 1:100 sind 220 mm, gemessen {block_breite:.1}"
    );
}

/// Die Kategorie ordnet den Block den Punktsymbolen zu, unabhängig von der
/// Größe — ein Benutzerblock darf beides sein.
#[test]
fn suchbegriff_der_kategorie_bleibt_der_massgebliche() {
    let mut doc = doc_mm();
    let block = rechteck_block(&mut doc, BlockGroesse::Fest);
    doc.block_mut(&block).expect("Block").category =
        jjcad_core::model::PUNKTSYMBOL_KATEGORIE.into();
    assert!(doc.punktsymbol_bloecke().iter().any(|b| b.id == block));
}
