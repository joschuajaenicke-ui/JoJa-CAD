//! T0210: Punktsymbole — Kategorie in den Systemblöcken, Symbol an Punkt und
//! Linienenden.
//!
//! Geprüft wird über die Action-Schicht (`draw.point`, `draw.line`,
//! `modify.set`) und die Anzeige (`render::entity_bbox`) — derselbe Weg, den
//! Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::{v, Vertex};
use jjcad_core::model::{
    BlockDef, BlockGroesse, BlockSource, Document, Entity, EntityKind, PUNKTSYMBOL_KATEGORIE,
};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut doc = Document::new();
    doc.settings.input_unit = Unit::Mm;
    doc
}

fn created(antwort: &serde_json::Value) -> u64 {
    antwort["created"][0].as_u64().expect("created[0]")
}

/// Papierbreite des Punktsymbols: die Anzeige rechnet die Blockmaße mit dem
/// Beschriftungsmaßstab in die Zeichnung um.
fn symbol_breite_mm(paper_mm: f64, massstab: f64) -> f64 {
    paper_mm * massstab
}

#[test]
fn systembloecke_enthalten_die_kategorie() {
    let doc = Document::new();
    assert!(
        jjcad_core::blocks_lib::categories().contains(&PUNKTSYMBOL_KATEGORIE),
        "die Kategorie fehlt in der Anzeigereihenfolge"
    );
    let symbole = doc.punktsymbol_bloecke();
    assert!(
        symbole.len() >= 8,
        "zu wenige Punktsymbole: {}",
        symbole.len()
    );
    for name in [
        "Punkt",
        "Kreuz",
        "Pfeil",
        "Pfeilspitze",
        "Maßmarke",
        "Höhenkote Grundriss",
        "Höhenkote Schnitt",
    ] {
        assert!(
            symbole.iter().any(|b| b.name == name),
            "Symbol „{name}“ fehlt"
        );
    }
    assert!(
        symbole.iter().all(|b| b.source == BlockSource::System),
        "die mitgelieferten Punktsymbole sind Systemblöcke"
    );
    // Ein Symbol ohne Geometrie wäre unsichtbar.
    for b in &symbole {
        assert!(!b.entities.is_empty(), "Block {} ist leer", b.name);
    }
}

#[test]
fn punkt_traegt_ein_symbol_und_zeichnet_es() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.point",
        &json!({"p": [5000.0, 5000.0], "symbol": "sys:pkt-kreuz"}),
    )
    .expect("Punkt mit Symbol");
    let id = created(&antwort);
    let e = doc.entity(id).expect("Objekt").clone();
    assert_eq!(e.start_symbol.as_deref(), Some("sys:pkt-kreuz"));
    assert_eq!(e.symbol(false), Some("sys:pkt-kreuz"));
    assert!(e.end_symbol.is_none(), "ein Punkt hat kein Ende");

    // Ohne Symbol zeichnet der Punkt nur seinen Dot; mit Kreuz ist die
    // Ausdehnung die des Blocks in Papiergröße (2,2 mm auf dem Papier, bei
    // 1:100 also 220 mm in der Zeichnung).
    let mit = jjcad_core::render::entity_bbox(&doc, &e);
    let mut ohne = e.clone();
    ohne.start_symbol = None;
    let ohne = jjcad_core::render::entity_bbox(&doc, &ohne);
    let erwartet = symbol_breite_mm(2.2, 100.0);
    assert!(
        (mit.width() - erwartet).abs() < 5.0,
        "Kreuzbreite {:.1} mm, erwartet {:.1} mm",
        mit.width(),
        erwartet
    );
    assert!(
        mit.width() > ohne.width() + 100.0,
        "das Symbol vergrößert die Ausdehnung nicht ({:.1} gegen {:.1})",
        mit.width(),
        ohne.width()
    );
}

#[test]
fn punkt_ohne_symbol_bleibt_ein_dot() {
    let mut doc = doc_mm();
    let antwort = actions::execute(&mut doc, "draw.point", &json!({"p": [1000.0, 1000.0]}))
        .expect("Punkt");
    let e = doc.entity(created(&antwort)).expect("Objekt").clone();
    assert!(e.start_symbol.is_none());
    let bbox = jjcad_core::render::entity_bbox(&doc, &e);
    assert!(
        bbox.width() < 100.0,
        "ein Punkt ohne Symbol ist punktförmig, nicht {:.1} mm breit",
        bbox.width()
    );
}

#[test]
fn linie_traegt_symbole_an_anfang_und_ende() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.line",
        &json!({
            "a": [0.0, 0.0],
            "b": [2000.0, 0.0],
            "start_symbol": "sys:pkt-punkt",
            "end_symbol": "sys:pkt-pfeil",
        }),
    )
    .expect("Linie mit Symbolen");
    let id = created(&antwort);
    let e = doc.entity(id).expect("Objekt").clone();
    assert_eq!(e.symbol(false), Some("sys:pkt-punkt"));
    assert_eq!(e.symbol(true), Some("sys:pkt-pfeil"));

    // Der Pfeil sitzt mit seiner **Spitze auf dem Endpunkt** (T0210, Turn 5)
    // und läuft von dort nach innen; der Punkt ragt eine halbe Einheit über den
    // Anfang zurück — beides in Beschriftungsgröße (1:100).
    let bbox = jjcad_core::render::entity_bbox(&doc, &e);
    assert!(
        (bbox.max.x - 2000.0).abs() < 1.0,
        "die Pfeilspitze steht nicht auf dem Endpunkt: {:.2} statt 2000",
        bbox.max.x
    );
    assert!(
        bbox.min.x < -symbol_breite_mm(0.4, 100.0),
        "Punkt am Anfang fehlt: {:.1}",
        bbox.min.x
    );
}

/// Die Pfeile tragen ihre Spitze auf dem Basispunkt (Ursprung): an einem
/// Linienende deckt sie sich dann exakt mit dem Endpunkt, an einem Anfang mit
/// dem Anfangspunkt. Vorher stand die Spitze 1,4 bzw. 1,6 Papier-mm daneben.
#[test]
fn pfeile_haben_die_spitze_auf_dem_basispunkt() {
    let doc = Document::new();
    for id in ["sys:pkt-pfeil", "sys:pkt-pfeilspitze"] {
        let def = doc.block(id).unwrap_or_else(|| panic!("Block {id} fehlt"));
        assert_eq!(def.base.x, 0.0, "{id}: Basispunkt nicht im Ursprung");
        let mut doc2 = doc_mm();
        doc2.blocks.push(def.clone());
        let e = Entity::new(
            1,
            0,
            EntityKind::BlockRef {
                block: id.to_string(),
                pos: jjcad_core::geom::Vec2::ZERO,
                rot: 0.0,
                scale: v(1.0, 1.0),
                mirror: false,
            },
        );
        let bbox = jjcad_core::render::entity_bbox(&doc2, &e);
        assert!(
            bbox.max.x.abs() < 0.01,
            "{id}: die Spitze liegt bei x = {:.3}, erwartet 0",
            bbox.max.x
        );
        assert!(
            bbox.min.x < -1.5,
            "{id}: der Körper muss nach −X laufen, min.x = {:.2}",
            bbox.min.x
        );
    }
}

#[test]
fn pfeilspitze_am_linienende_trifft_den_endpunkt_auf_den_punkt() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let antwort = actions::execute(
        &mut doc,
        "draw.line",
        &json!({
            "a": [0.0, 0.0],
            "b": [2000.0, 0.0],
            "start_symbol": "sys:pkt-pfeilspitze",
            "end_symbol": "sys:pkt-pfeilspitze",
        }),
    )
    .expect("Linie");
    let e = doc.entity(created(&antwort)).expect("Objekt").clone();
    let bbox = jjcad_core::render::entity_bbox(&doc, &e);
    assert!(
        (bbox.max.x - 2000.0).abs() < 0.5,
        "Ende: Spitze bei {:.2}, Linienende bei 2000",
        bbox.max.x
    );
    assert!(
        bbox.min.x.abs() < 0.5,
        "Anfang: Spitze bei {:.2}, Linienanfang bei 0",
        bbox.min.x
    );
}

#[test]
fn symbolgroesse_folgt_dem_beschriftungsmassstab() {
    let mut doc = doc_mm();
    doc.settings.annotation_scale = 100.0;
    let id = created(
        &actions::execute(
            &mut doc,
            "draw.point",
            &json!({"p": [0.0, 0.0], "symbol": "sys:pkt-quadrat"}),
        )
        .expect("Punkt"),
    );
    let gross = jjcad_core::render::entity_bbox(&doc, doc.entity(id).unwrap()).width();
    doc.settings.annotation_scale = 50.0;
    let klein = jjcad_core::render::entity_bbox(&doc, doc.entity(id).unwrap()).width();
    assert!(
        (gross / klein - 2.0).abs() < 0.05,
        "1:100 muss doppelt so groß sein wie 1:50 ({gross:.1} / {klein:.1})"
    );
    // Das Quadrat ist 1,6 Papier-mm breit (T0231: gedruckte Größe).
    assert!((klein - symbol_breite_mm(1.6, 50.0)).abs() < 2.0);
}

/// Ein Benutzerblock mit der Kategorie „Punkt Symbole“ — auch klein
/// geschrieben — muss wählbar sein (die Kategorie entscheidet, nicht die
/// Herkunft).
#[test]
fn benutzerblock_der_kategorie_ist_als_symbol_waehlbar() {
    let mut doc = doc_mm();
    let id = doc.new_id();
    doc.blocks.push(BlockDef {
        id: "usr:marke".into(),
        name: "Eigene Marke".into(),
        category: " punkt symbole ".into(),
        source: BlockSource::User,
        description: String::new(),
        entities: vec![Entity::new(
            1,
            0,
            EntityKind::Polyline {
                verts: vec![
                    Vertex::new(v(-1.0, -1.0)),
                    Vertex::new(v(1.0, -1.0)),
                    Vertex::new(v(0.0, 1.0)),
                ],
                closed: true,
            },
        )],
        icon: None,
        base: jjcad_core::geom::Vec2::ZERO,
        groesse: BlockGroesse::Papier,
    });
    assert!(
        doc.punktsymbol_bloecke().iter().any(|b| b.id == "usr:marke"),
        "Benutzerblock fehlt in der Auswahlliste"
    );

    let antwort = actions::execute(
        &mut doc,
        "draw.point",
        &json!({"p": [0.0, 0.0], "symbol": "usr:marke"}),
    )
    .expect("Punkt mit Benutzersymbol");
    let e = doc.entity(created(&antwort)).expect("Objekt").clone();
    assert_eq!(e.start_symbol.as_deref(), Some("usr:marke"));
    assert!(jjcad_core::render::entity_bbox(&doc, &e).width() > 150.0);
}

#[test]
fn modify_set_setzt_und_entfernt_das_symbol() {
    let mut doc = doc_mm();
    let id = created(
        &actions::execute(&mut doc, "draw.point", &json!({"p": [0.0, 0.0]})).expect("Punkt"),
    );
    actions::execute(
        &mut doc,
        "modify.set",
        &json!({"ids": [id], "start_symbol": "sys:pkt-kreuz"}),
    )
    .expect("Symbol setzen");
    assert_eq!(
        doc.entity(id).unwrap().start_symbol.as_deref(),
        Some("sys:pkt-kreuz")
    );

    // Ein leerer Text nimmt das Symbol ebenso weg wie `null`.
    actions::execute(
        &mut doc,
        "modify.set",
        &json!({"ids": [id], "start_symbol": null}),
    )
    .expect("Symbol entfernen");
    assert!(doc.entity(id).unwrap().start_symbol.is_none());

    // Ein unbekannter Block ist ein Fehler, kein stiller Erfolg (T0038).
    let fehler = actions::execute(
        &mut doc,
        "modify.set",
        &json!({"ids": [id], "start_symbol": "gibt:es:nicht"}),
    );
    assert!(fehler.is_err(), "unbekanntes Symbol muss scheitern");
    assert!(doc.entity(id).unwrap().start_symbol.is_none());
}

#[test]
fn aeltere_datei_bekommt_die_punktsymbole_beim_laden() {
    let doc = doc_mm();
    // So sieht eine Datei von vor T0210 aus: die neuen Systemblöcke fehlen.
    let mut roh: serde_json::Value =
        serde_json::from_str(&jjcad_core::persist::to_json(&doc).unwrap()).unwrap();
    let bloecke = roh["blocks"].as_array_mut().expect("blocks");
    bloecke.retain(|b| {
        !b["id"]
            .as_str()
            .map(|id| id.starts_with("sys:pkt-"))
            .unwrap_or(false)
    });
    assert!(
        !bloecke.iter().any(|b| b["category"] == PUNKTSYMBOL_KATEGORIE),
        "die Prüfdatei darf die Kategorie noch nicht tragen"
    );

    let geladen = jjcad_core::persist::from_json(&roh.to_string()).expect("Datei laden");
    let symbole = geladen.punktsymbol_bloecke();
    assert!(
        symbole.len() >= 8,
        "nach dem Laden fehlen die Punktsymbole: {}",
        symbole.len()
    );
    // Vorhandene Blöcke bleiben unangetastet (keine Doppel).
    let mut ids: Vec<&str> = geladen.blocks.iter().map(|b| b.id.as_str()).collect();
    let vorher = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), vorher, "Blöcke wurden doppelt eingetragen");
}

/// Eine Zeichnung, die vor der Ausrichtung gespeichert wurde, trägt den alten
/// Pfeil in ihrer Datei. Beim Laden gilt die Bibliothek — sonst bliebe der
/// Basispunkt für immer in der Mitte (T0210, Turn 7).
#[test]
fn alte_datei_bekommt_den_neuen_pfeil_aus_der_bibliothek() {
    let doc = doc_mm();
    let mut roh: serde_json::Value =
        serde_json::from_str(&jjcad_core::persist::to_json(&doc).unwrap()).unwrap();

    // Den Stand von vor Turn 5 einbauen: Spitze 1,4 mm rechts vom Ursprung.
    let mut alt = BlockDef {
        id: "sys:pkt-pfeil".into(),
        name: "Pfeil".into(),
        category: PUNKTSYMBOL_KATEGORIE.into(),
        source: BlockSource::System,
        description: "Pfeil, 3,4 mm".into(),
        entities: vec![Entity::new(
            1,
            0,
            EntityKind::Line {
                a: v(-2.0, 0.0),
                b: v(1.4, 0.0),
            },
        )],
        icon: None,
        base: jjcad_core::geom::Vec2::ZERO,
        groesse: BlockGroesse::Papier,
    };
    alt.entities[0].by_block = true;
    let alt = serde_json::to_value(&alt).expect("alter Block");
    for b in roh["blocks"].as_array_mut().expect("blocks").iter_mut() {
        if b["id"] == "sys:pkt-pfeil" {
            *b = alt.clone();
        }
    }

    let geladen = jjcad_core::persist::from_json(&roh.to_string()).expect("Datei laden");
    let def = geladen.block("sys:pkt-pfeil").expect("Pfeil");
    let huelle = jjcad_core::render::entity_bbox(
        &geladen,
        &Entity::new(
            999,
            0,
            EntityKind::BlockRef {
                block: "sys:pkt-pfeil".into(),
                pos: jjcad_core::geom::Vec2::ZERO,
                rot: 0.0,
                scale: v(1.0, 1.0),
                mirror: false,
            },
        ),
    );
    assert!(
        huelle.max.x.abs() < 0.01,
        "der geladene Pfeil hat die Spitze bei x = {:.2} statt auf dem Basispunkt",
        huelle.max.x
    );
    assert!(
        def.description.contains("Basispunkt"),
        "der Block stammt nicht aus der Bibliothek: {}",
        def.description
    );
}

/// Ein Block des Benutzers mit derselben Kennung bleibt stehen — ersetzt wird
/// nur, was wirklich Systemblock ist.
#[test]
fn benutzerblock_mit_systemkennung_bleibt_stehen() {
    let mut doc = doc_mm();
    doc.blocks.retain(|b| b.id != "sys:pkt-pfeil");
    doc.blocks.push(BlockDef {
        id: "sys:pkt-pfeil".into(),
        name: "Mein Pfeil".into(),
        category: String::new(),
        source: BlockSource::User,
        description: String::new(),
        entities: Vec::new(),
        icon: None,
        base: jjcad_core::geom::Vec2::ZERO,
        groesse: BlockGroesse::Fest,
    });
    let roh = jjcad_core::persist::to_json(&doc).unwrap();
    let geladen = jjcad_core::persist::from_json(&roh).expect("Datei laden");
    let def = geladen.block("sys:pkt-pfeil").expect("Block");
    assert_eq!(def.source, BlockSource::User);
    assert_eq!(def.name, "Mein Pfeil");
}
