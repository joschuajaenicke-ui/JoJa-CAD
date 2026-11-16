//! Standardebene je Objekttyp (Ticket T0039, neu gefasst in T0202).
//!
//! Über MCP zeichnet ein Agent einen ganzen Plan in einem Batch. Ohne `layer`
//! landet sonst **alles** auf der aktuellen Ebene. Jede Zeichenaktion bringt
//! deshalb die Ebene ihres Typs mit; ein ausdrücklich übergebenes `layer`
//! sticht sie.
//!
//! Seit T0202 gibt es nur noch **vier** Layer je Plan: Zeichnung, Architektur,
//! Beschriftung und Blöcke. Die Zuordnung der Objektarten ist die halbe
//! Bedeutung des Layer-Managers — sie wird hier festgehalten.
//!
//! Geprüft wird über die Befehls-Schicht (`actions::execute`) — derselbe Weg,
//! den MCP, GUI und Tastatur nehmen.

use jjcad_core::actions;
use jjcad_core::model::Document;
use jjcad_core::style::LAYER_DEFAULTS;
use serde_json::{json, Value};

/// Führt eine Aktion aus und liefert die Id des erzeugten Objekts.
fn neu(doc: &mut Document, action: &str, params: Value) -> u64 {
    let r = actions::execute(doc, action, &params)
        .unwrap_or_else(|e| panic!("{action} fehlgeschlagen: {e}"));
    r["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{action} hat kein Objekt erzeugt"))
}

/// Name der Ebene, auf der das Objekt liegt.
fn ebene(doc: &Document, id: u64) -> String {
    let e = doc.entity(id).unwrap_or_else(|| panic!("Objekt {id} fehlt"));
    doc.layer(e.layer)
        .map(|l| l.name.clone())
        .unwrap_or_else(|| format!("Ebene {} unbekannt", e.layer))
}

/// Ein Dokument in Metern; die aktuelle Ebene ist „Zeichnung“.
fn doc() -> Document {
    let doc = Document::new();
    assert_eq!(
        doc.layer(doc.current_layer).map(|l| l.name.as_str()),
        Some("Zeichnung"),
        "Ausgangslage: Neudokument zeichnet auf „Zeichnung“"
    );
    doc
}

/// Ein Plan aus einem Batch: je ein Objekt jedes Typs **ohne** `layer`.
#[test]
fn batch_ohne_layer_landet_je_typ_auf_seiner_standardebene() {
    let mut d = doc();

    let wand = neu(&mut d, "draw.wall", json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}));
    assert_eq!(ebene(&d, wand), "Architektur");

    let tuer = neu(
        &mut d,
        "draw.door",
        json!({"center": [1.0, 0.0], "width": 0.9, "host": [wand]}),
    );
    assert_eq!(ebene(&d, tuer), "Architektur");

    let fenster = neu(
        &mut d,
        "draw.window",
        json!({"center": [3.0, 0.0], "width": 1.2, "host": [wand]}),
    );
    assert_eq!(ebene(&d, fenster), "Architektur");

    let treppe = neu(
        &mut d,
        "draw.stair",
        json!({"a": [0, 0], "b": [0, 4.64], "width": 1.0, "height": 2.752}),
    );
    assert_eq!(ebene(&d, treppe), "Architektur");

    let masz = neu(
        &mut d,
        "dim.linear",
        json!({"a": [0, 0], "b": [5, 0], "line_pos": [0, -0.5]}),
    );
    assert_eq!(ebene(&d, masz), "Beschriftung");

    let kette = neu(
        &mut d,
        "dim.chain",
        json!({"pts": [[0, 0], [2, 0], [5, 0]], "line_pos": [0, -0.8]}),
    );
    assert_eq!(ebene(&d, kette), "Beschriftung");

    let kote = neu(&mut d, "dim.elevation.plan", json!({"p": [0, 0], "z": 1.0}));
    let schnitt = neu(
        &mut d,
        "dim.elevation.section",
        json!({"pts": [[0, 0], [0, 1.5]]}),
    );
    assert_eq!(ebene(&d, kote), "Beschriftung");
    assert_eq!(ebene(&d, schnitt), "Beschriftung");

    let text = neu(&mut d, "draw.text", json!({"p": [1, 1], "text": "X", "height": 0.3}));
    assert_eq!(ebene(&d, text), "Beschriftung");

    // Flächen und Schraffuren gehören in die Zeichnung.
    let schraffur = neu(
        &mut d,
        "draw.hatch",
        json!({"boundary": [[0, 0], [1, 0], [1, 1]], "pattern": "lines"}),
    );
    assert_eq!(ebene(&d, schraffur), "Zeichnung");

    let flaeche = neu(
        &mut d,
        "draw.fill",
        json!({"boundary": [[2, 0], [3, 0], [3, 1]]}),
    );
    assert_eq!(ebene(&d, flaeche), "Zeichnung");

    // Ein Volumen ist ein Körper im Wandton — es gehört zur Architektur.
    let volumen = neu(
        &mut d,
        "draw.volume",
        json!({"boundary": [[4, 0], [5, 0], [5, 1]], "height": 2.75}),
    );
    assert_eq!(ebene(&d, volumen), "Architektur");

    let block = d.blocks[0].id.clone();
    let instanz = neu(&mut d, "block.insert", json!({"block": block, "pos": [1, 1]}));
    assert_eq!(ebene(&d, instanz), "Blöcke");

    // Freie Geometrie ist Zeichnung.
    let faelle: Vec<(&str, Value)> = vec![
        ("draw.point", json!({"p": [0, 0]})),
        ("draw.line", json!({"a": [0, 0], "b": [1, 1]})),
        ("draw.polyline", json!({"pts": [[0, 0], [1, 0], [1, 1]]})),
        ("draw.spline", json!({"pts": [[0, 0], [1, 0], [1, 1]]})),
        ("draw.rect", json!({"a": [0, 0], "b": [1, 1]})),
        ("draw.circle", json!({"c": [0, 0], "r": 1.0})),
        ("draw.arc", json!({"c": [0, 0], "r": 1.0, "a0": 0, "a1": 90})),
        ("draw.ellipse", json!({"c": [0, 0], "rx": 1.0, "ry": 0.5})),
    ];
    for (action, params) in faelle {
        let id = neu(&mut d, action, params);
        assert_eq!(ebene(&d, id), "Zeichnung", "{action} gehört in die Zeichnung");
    }

    // Die vier Standardlayer sind vorhanden und nicht doppelt angelegt.
    let mut namen: Vec<&str> = d.layers.iter().map(|l| l.name.as_str()).collect();
    let anzahl = namen.len();
    namen.sort_unstable();
    namen.dedup();
    assert_eq!(namen.len(), anzahl, "keine doppelten Ebenen");
    assert_eq!(anzahl, LAYER_DEFAULTS.len(), "genau die vier Standardlayer");
}

/// Ein ausdrücklich übergebenes `layer` gewinnt gegen jede Vorgabe — als Name
/// und als Id. Ein unbekannter Name wird im Plan des aktuellen Layers angelegt.
#[test]
fn ausdrueckliche_ebene_sticht_die_standardebene() {
    let mut d = doc();
    let beschriftung = d
        .layer_id_by_name("Beschriftung")
        .expect("Ebene „Beschriftung“ fehlt");
    let zeichnung = d.layer_id_by_name("Zeichnung").expect("Ebene „Zeichnung“ fehlt");

    // Text auf „Zeichnung“ (Name) — die Vorgabe wäre „Beschriftung“.
    let text = neu(
        &mut d,
        "draw.text",
        json!({"p": [0, 0], "text": "A", "height": 0.25, "layer": "Zeichnung"}),
    );
    assert_eq!(ebene(&d, text), "Zeichnung");

    // Maßkette auf „Architektur“ — die Vorgabe wäre „Beschriftung“.
    let kette = neu(
        &mut d,
        "dim.chain",
        json!({"pts": [[0, 0], [2, 0]], "line_pos": [0, -0.5], "layer": "Architektur"}),
    );
    assert_eq!(ebene(&d, kette), "Architektur");

    // Maß auf eine übergebene Id.
    let masz = neu(
        &mut d,
        "dim.linear",
        json!({"a": [0, 0], "b": [2, 0], "line_pos": [0, -0.4], "layer": beschriftung}),
    );
    assert_eq!(ebene(&d, masz), "Beschriftung");

    // Wand auf „Zeichnung“ (Name) — die Vorgabe wäre „Architektur“.
    let wand = neu(
        &mut d,
        "draw.wall",
        json!({"a": [0, 2], "b": [2, 2], "thickness": 0.24, "layer": "Zeichnung"}),
    );
    assert_eq!(ebene(&d, wand), "Zeichnung");

    // Block auf eine übergebene Id.
    let block = d.blocks[0].id.clone();
    let instanz = neu(
        &mut d,
        "block.insert",
        json!({"block": block, "pos": [0, 0], "layer": zeichnung}),
    );
    assert_eq!(ebene(&d, instanz), "Zeichnung");

    // Unbekannter Name wird angelegt — im Plan des aktuellen Layers (T0202).
    let aktueller_plan = d.aktueller_plan();
    let punkt = neu(
        &mut d,
        "draw.point",
        json!({"p": [1, 1], "layer": "Bauantrag"}),
    );
    assert_eq!(ebene(&d, punkt), "Bauantrag");
    let l = d.layer_by_name("Bauantrag").expect("Ebene fehlt");
    assert_eq!(
        l.plan,
        aktueller_plan.expect("Plan"),
        "neue Ebene im aktuellen Plan"
    );
}

/// Die Treppenbeschriftung gehört zur Treppe, nicht zur aktuellen Ebene.
#[test]
fn treppenbeschriftung_liegt_auf_architektur() {
    let mut d = doc();
    let treppe = neu(
        &mut d,
        "draw.stair",
        json!({"a": [0, 0], "b": [0, 4.64], "width": 1.0, "height": 2.752}),
    );
    let label = neu(&mut d, "draw.stair.label", json!({"id": treppe}));
    assert_eq!(ebene(&d, label), "Architektur");
}
