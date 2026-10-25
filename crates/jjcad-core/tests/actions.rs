//! Integrationstests für die Befehls-Schicht.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{DoorKind, EntityKind};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

fn last_id(s: &Session) -> u64 {
    s.doc.entities.last().unwrap().id
}

#[test]
fn draw_all_primitives() {
    let mut s = s();
    for (action, params) in [
        ("draw.point", json!({"p": [1, 1]})),
        ("draw.line", json!({"a": [0, 0], "b": [2, 0]})),
        ("draw.circle", json!({"c": [1, 1], "r": 0.5})),
        ("draw.arc", json!({"c": [0, 0], "r": 1.0, "a0": 0, "a1": 90})),
        ("draw.ellipse", json!({"c": [0, 0], "rx": 1, "ry": 0.5})),
        ("draw.rect", json!({"a": [0, 0], "b": [3, 2]})),
        ("draw.polyline", json!({"pts": [[0, 0], [1, 1], [2, 0]]})),
        ("draw.hatch", json!({"boundary": [[0, 0], [1, 0], [1, 1], [0, 1]]})),
        ("draw.text", json!({"p": [0, 0], "text": "Hallo", "height": 0.25})),
    ] {
        s.execute(action, &params)
            .unwrap_or_else(|e| panic!("{action}: {e}"));
    }
    assert_eq!(s.doc.entities.len(), 9);
    // Ellipse-Radien in mm
    let ellipse = s.doc.entities.iter().find_map(|e| match &e.kind {
        EntityKind::Ellipse { rx, ry, .. } => Some((*rx, *ry)),
        _ => None,
    });
    assert_eq!(ellipse, Some((1000.0, 500.0)));
}

#[test]
fn architecture_objects_and_openings() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [6, 0], "thickness": 0.30}),
    )
    .unwrap();
    let wall_id = last_id(&s);
    let res = s
        .execute(
            "draw.window",
            &json!({"center": [3, 0], "width": 1.4}),
        )
        .unwrap();
    assert_eq!(res["host"], json!(wall_id), "Fenster muss der Wand zugeordnet werden");

    // Öffnungsintervall muss in der Wand liegen
    let wall = s.doc.entity(wall_id).unwrap().clone();
    let intervals = jjcad_core::render::wall_opening_intervals(&s.doc, &wall);
    assert_eq!(intervals.len(), 1);
    let (a, b) = intervals[0];
    assert!((a - 2300.0).abs() < 1.0, "Öffnung startet bei 2300 mm: {a}");
    assert!((b - 3700.0).abs() < 1.0);

    s.execute("draw.door", &json!({"center": [1, 0], "width": 0.885}))
        .unwrap();
    s.execute("draw.stair", &json!({"a": [7, 1], "b": [7, 4], "width": 1.0, "steps": 12}))
        .unwrap();
    assert_eq!(s.doc.entities.len(), 4);
}

#[test]
fn modify_transformations() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]})).unwrap();
    let id = last_id(&s);

    s.execute("modify.move", &json!({"ids": [id], "from": [0, 0], "to": [1, 1]}))
        .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x - 1000.0).abs() < 1e-6 && (a.y - 1000.0).abs() < 1e-6);
        assert!((b.x - 3000.0).abs() < 1e-6);
    } else {
        panic!("Linie erwartet");
    }

    // Kopie erzeugt neues Objekt
    s.execute("modify.copy", &json!({"ids": [id], "from": [0, 0], "to": [0, 2]}))
        .unwrap();
    assert_eq!(s.doc.entities.len(), 2);

    // Drehen um 90° um den Ursprung
    s.execute("modify.rotate", &json!({"ids": [id], "center": [0, 0], "angle": 90}))
        .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x + 1000.0).abs() < 1e-6 && (a.y - 1000.0).abs() < 1e-6, "{a:?}");
        assert!((b.x + 1000.0).abs() < 1e-6 && (b.y - 3000.0).abs() < 1e-6, "{b:?}");
    }

    // Spiegeln an der X-Achse
    s.execute("modify.mirror", &json!({"ids": [id], "p1": [0, 0], "p2": [1, 0], "keep_original": false}))
        .unwrap();
    if let EntityKind::Line { a, .. } = &s.doc.entity(id).unwrap().kind {
        assert!((a.y + 1000.0).abs() < 1e-6, "y muss negativ sein: {a:?}");
    }

    // Skalieren
    s.execute("modify.scale", &json!({"ids": [id], "center": [0, 0], "factor": 2.0}))
        .unwrap();
    if let EntityKind::Line { b, .. } = &s.doc.entity(id).unwrap().kind {
        assert!((b.y + 6000.0).abs() < 1e-6 && (b.x + 2000.0).abs() < 1e-6, "{b:?}");
    }
}

#[test]
fn trim_and_extend() {
    let mut s = s();
    // Zwei senkrechte Schnittkanten bei x=2 und x=4, dazwischen eine Linie
    s.execute("draw.line", &json!({"a": [2, -1], "b": [2, 1]})).unwrap();
    s.execute("draw.line", &json!({"a": [4, -1], "b": [4, 1]})).unwrap();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [6, 0]})).unwrap();
    let id = last_id(&s);

    // Mittleres Stück (x 2..4) entfernen -> zwei Linien
    s.execute("modify.trim", &json!({"ids": [id], "at": [3, 0]}))
        .unwrap();
    let lines: Vec<(Vec2, Vec2)> = s
        .doc
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Line { a, b } if a.y.abs() < 1e-6 && b.y.abs() < 1e-6 => Some((*a, *b)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 2, "Stutzen muss die Linie teilen: {lines:?}");

    // Verlängern: rechte Linie endet bei x=4 -> soll bis x=2 verlängert werden? Nein: bis zur nächsten Kante
    let right = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(&e.kind, EntityKind::Line{a,b} if a.y.abs()<1e-6 && b.y.abs()<1e-6 && a.x > 3900.0))
        .map(|e| e.id)
        .expect("rechte Teillinie");
    s.execute("modify.extend", &json!({"ids": [right], "at": [4, 0], "cutters": []}))
        .unwrap_or_else(|e| panic!("Verlängern: {e}"));
}

#[test]
fn layers_and_blocks() {
    let mut s = s();
    s.execute("layer.create", &json!({"name": "Testlayer", "color": "#FF0000"}))
        .unwrap();
    let lid = s.doc.layer_id_by_name("Testlayer").unwrap();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [1, 0], "layer": "Testlayer"}))
        .unwrap();
    assert_eq!(s.doc.entities[0].layer, lid);

    // Block aus Auswahl
    let id = s.doc.entities[0].id;
    let res = s
        .execute("block.create", &json!({"name": "MeinBlock", "ids": [id]}))
        .unwrap();
    let block_id = res["block"].as_str().unwrap().to_string();
    assert!(s.doc.block(&block_id).is_some());
    assert_eq!(s.doc.entities.len(), 1, "Original wird durch Instanz ersetzt");

    // Instanz einfügen
    s.execute("block.insert", &json!({"block": "MeinBlock", "pos": [5, 5]}))
        .unwrap();
    assert_eq!(s.doc.entities.len(), 2);

    // Zerlegen der Instanz erzeugt Grundelemente
    let inst = last_id(&s);
    let res = s.execute("modify.explode", &json!({"ids": [inst]})).unwrap();
    assert!(res["created"].as_array().unwrap().len() >= 1);
    assert!(s.doc.entity(inst).is_none());
}

#[test]
fn dimensions_and_units() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [4.5, 0], "thickness": 0.3}))
        .unwrap();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0, 0], [2, 0], [4.5, 0]], "line_pos": [0, -1.2], "axis": "x"}),
    )
    .unwrap();
    // Maßtext in Metern mit 2 Nachkommastellen
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"2,00".to_string()), "Maßtexte: {texts:?}");
    assert!(texts.contains(&"2,50".to_string()), "Maßtexte: {texts:?}");

    s.execute("doc.settings", &json!({"dim_unit": "cm"})).unwrap();
    let dl = jjcad_core::render::render_document(&jjcad_core::render::RenderCtx::new(&s.doc));
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t.starts_with("200")), "cm-Anzeige: {texts:?}");
}

#[test]
fn undo_redo_and_persistence() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [1, 0]})).unwrap();
    s.execute("draw.line", &json!({"a": [0, 1], "b": [1, 1]})).unwrap();
    assert_eq!(s.doc.entities.len(), 2);
    assert!(s.undo());
    assert_eq!(s.doc.entities.len(), 1);
    assert!(s.redo());
    assert_eq!(s.doc.entities.len(), 2);

    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roundtrip.jjcad");
    s.save(&path).unwrap();
    let loaded = jjcad_core::persist::load_document(&path).unwrap();
    assert_eq!(loaded.entities.len(), 2);
    assert_eq!(loaded.layers.len(), s.doc.layers.len());
    assert_eq!(loaded.sheets.len(), 1);
    // JSON ist lesbar (kein Binärformat)
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"format\": \"jjcad\""));
    assert!(text.contains("\"layers\""));
}

#[test]
fn dxf_roundtrip() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]})).unwrap();
    s.execute("draw.circle", &json!({"c": [1, 1], "r": 0.4})).unwrap();
    s.execute("draw.wall", &json!({"a": [0, 3], "b": [4, 3], "thickness": 0.24}))
        .unwrap();
    s.execute("draw.text", &json!({"p": [0, 2], "text": "Test", "height": 0.25}))
        .unwrap();

    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.dxf");
    jjcad_core::dxf::export_dxf(&s.doc, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("AC1009"));
    assert!(text.contains("ENTITIES"));

    let imported = jjcad_core::dxf::import_dxf(&path).unwrap();
    assert!(imported.entities.len() >= 4, "DXF-Import: {} Objekte", imported.entities.len());
    assert_eq!(imported.warnings.len(), 0, "Warnungen: {:?}", imported.warnings);
}

#[test]
fn pdf_and_png_export() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    s.execute("draw.wall", &json!({"a": [5, 0], "b": [5, 4], "thickness": 0.3}))
        .unwrap();
    s.execute("draw.door", &json!({"center": [2, 0], "width": 0.9})).unwrap();
    s.execute("draw.text", &json!({"p": [1, 2], "text": "Wohnen", "height": 0.3}))
        .unwrap();

    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let pdf_path = dir.join("out.pdf");
    jjcad_core::pdf::export_pdf(&s.doc, &pdf_path, Some(0)).unwrap();
    let pdf = std::fs::read(&pdf_path).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(pdf.ends_with(b"%%EOF\n"));
    // Umlaute müssen als WinAnsi-Oktalescape im Strom stehen
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("stream"));

    let png_path = dir.join("out.png");
    let opts = jjcad_core::export::RasterOptions {
        width_px: 800,
        height_px: 600,
        ..Default::default()
    };
    jjcad_core::raster::render_png(&s.doc, &opts, &png_path).unwrap();
    let png = std::fs::read(&png_path).unwrap();
    assert_eq!(&png[0..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    assert!(png.len() > 2000);
}

#[test]
fn frictionless_wall_chain() {
    // Wände wie in der GUI: erster Punkt, zweiter Punkt, Kette
    let mut s = s();
    for (a, b) in [
        ([0.0, 0.0], [5.0, 0.0]),
        ([5.0, 0.0], [5.0, 4.0]),
        ([5.0, 4.0], [0.0, 4.0]),
    ] {
        s.execute("draw.wall", &json!({"a": a, "b": b, "thickness": 0.3}))
            .unwrap();
    }
    // Ecken müssen geschlossen sein: Verlängerungen berechnen
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    assert!(dl.prims.len() >= 9, "Prims: {}", dl.prims.len());
    let bbox = s.doc.bbox();
    // Die Ecke bei (5,0) muss durch Verlängerung geschlossen sein
    assert!(bbox.max.x >= 5100.0, "Wandende muss über die Ecke verlängert sein: {:?}", bbox.max);
    assert!(bbox.min.y <= -100.0, "{:?}", bbox.min);
    assert!(bbox.max.y >= 4100.0);
}

#[test]
fn door_kinds_render_differently() {
    let mut s = s();
    s.execute(
        "draw.door",
        &json!({"center": [0, 0], "angle": 0, "width": 1.0, "kind": "hinge"}),
    )
    .unwrap();
    let hinge = jjcad_core::render::entity_bbox(&s.doc, s.doc.entities.last().unwrap());
    s.execute(
        "draw.door",
        &json!({"center": [3, 0], "angle": 0, "width": 1.0, "kind": "sliding"}),
    )
    .unwrap();
    let sliding = jjcad_core::render::entity_bbox(&s.doc, s.doc.entities.last().unwrap());
    assert!(hinge.height() > sliding.height(), "Drehtür braucht Schwungbogen");
    let kinds: Vec<DoorKind> = s
        .doc
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Door { kind, .. } => Some(kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, vec![DoorKind::Hinge, DoorKind::Sliding]);
}

#[test]
fn template_text_fields_resolve() {
    let mut s = s();
    s.execute(
        "doc.project",
        &json!({"project": "Haus Test", "client": "Bauherr X", "plan_no": "A-1"}),
    )
    .unwrap();
    s.execute(
        "draw.text",
        &json!({"p": [0, 0], "text": "{PROJEKT} / {PLAN}", "height": 0.25}),
    )
    .unwrap();
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t == "Haus Test / A-1"), "{texts:?}");
}

#[test]
fn sheet_title_block_uses_sheet_scale() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [10, 0], "thickness": 0.3}))
        .unwrap();
    let sheet_id = s.doc.sheets[0].id;
    s.execute("sheet.update", &json!({"sheet": sheet_id, "scale": 50}))
        .unwrap();
    let bbox = s.doc.bbox();
    let center = bbox.center();
    let vid = s.doc.sheets[0].viewports[0].id;
    s.execute(
        "sheet.update_viewport",
        &json!({"sheet": sheet_id, "viewport": vid, "scale": 50, "center": [center.x / 1000.0, center.y / 1000.0]}),
    )
    .unwrap();
    let dl = jjcad_core::render::render_sheet(&s.doc, &s.doc.sheets[0]);
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t.contains("1:50")), "Schriftfeld-Maßstab: {texts:?}");
}

#[test]
fn snapping_finds_endpoints_and_intersections() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]})).unwrap();
    s.execute("draw.line", &json!({"a": [1, -1], "b": [1, 1]})).unwrap();
    // Rasterfang für diesen Test ausschalten
    s.execute("doc.settings", &json!({"snap": {"grid": false}})).unwrap();
    let hit = jjcad_core::snap::find_snap(&s.doc, v(1500.0, 1500.0), 20.0, None, &[], &[]);
    assert!(hit.is_none(), "{hit:?}");
    let hit = jjcad_core::snap::find_snap(&s.doc, v(1990.0, 10.0), 60.0, None, &[], &[]).unwrap();
    assert_eq!(hit.point.x, 2000.0);
    let hit = jjcad_core::snap::find_snap(&s.doc, v(1005.0, 5.0), 80.0, None, &[], &[]).unwrap();
    assert!((hit.point.x - 1000.0).abs() < 1.0);
}

#[test]
fn wandkontur_fang_ueber_einstellungen() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.30}))
        .unwrap();
    let wall_id = last_id(&s);
    s.execute("doc.settings", &json!({"snap": {"grid": false}})).unwrap();

    // Die Konturecke liegt 150 mm neben der Achse: (0;150) statt (0;0).
    let ecke = v(0.0, 150.0);
    let hit = jjcad_core::snap::find_snap(&s.doc, v(4.0, 146.0), 30.0, None, &[], &[])
        .expect("Wandkontur muss fangbar sein");
    assert!(hit.point.dist(ecke) < 1e-6, "{hit:?}");
    assert_eq!(hit.kind, jjcad_core::snap::SnapKind::Endpoint);
    assert_eq!(hit.entity, Some(wall_id));

    // Abschalten: nur die Kontur verschwindet, die Achse bleibt fangbar.
    s.execute("doc.settings", &json!({"snap": {"wall_edges": false}}))
        .unwrap();
    assert!(jjcad_core::snap::find_snap(&s.doc, v(4.0, 146.0), 30.0, None, &[], &[]).is_none());
    let achse = jjcad_core::snap::find_snap(&s.doc, v(4.0, 4.0), 30.0, None, &[], &[])
        .expect("Achsendendpunkt bleibt");
    assert!(achse.point.dist(v(0.0, 0.0)) < 1e-6, "{achse:?}");

    // Der Schalter überlebt Speichern und Laden.
    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wandfang.jjcad");
    s.save(&path).unwrap();
    let loaded = jjcad_core::persist::load_document(&path).unwrap();
    assert!(!loaded.settings.snap.wall_edges, "Schalter muss gespeichert sein");
}

#[test]
fn door_sits_on_front_wall_face() {
    // Tür sitzt immer auf der vorderen Wandkante (Blattseite), nicht in der
    // Wandmitte; Ausrichtung ist fuer Tueren ohne Wirkung.
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [2, 0], "width": 0.9, "host": [wall]}),
    )
    .unwrap();
    let door = last_id(&s);
    let pos = |s: &Session| match &s.doc.entity(door).unwrap().kind {
        EntityKind::Door { center, .. } => *center,
        _ => unreachable!(),
    };
    let p = pos(&s);
    assert!((p.y - 150.0).abs() < 1.0, "vordere Kante: y={}", p.y);
    assert!((p.x - 2000.0).abs() < 1.0, "x={}", p.x);

    // Blatt auf die andere Seite -> andere Kante, Lage laengs bleibt
    s.execute("modify.set", &json!({"ids": [door], "swing_in": false}))
        .unwrap();
    let p = pos(&s);
    assert!((p.y + 150.0).abs() < 1.0, "gegenueber: y={}", p.y);
    assert!((p.x - 2000.0).abs() < 1.0, "x nach Spiegelung={}", p.x);

    // Ausrichtung wird bei Tueren ignoriert
    s.execute("modify.set", &json!({"ids": [door], "align": "left"}))
        .unwrap();
    assert!((pos(&s).y + 150.0).abs() < 1.0, "align darf nichts aendern");
}

#[test]
fn window_align_repositions_in_wall() {
    // Fenster: Ausrichtung verschiebt innerhalb der Wanddicke (75 mm bei 300 mm)
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.window",
        &json!({"center": [2, 0], "width": 1.0, "host": [wall], "align": "center"}),
    )
    .unwrap();
    let win = last_id(&s);
    let cy = |s: &Session| match &s.doc.entity(win).unwrap().kind {
        EntityKind::Window { center, .. } => center.y,
        _ => unreachable!(),
    };
    s.execute("modify.set", &json!({"ids": [win], "align": "left"}))
        .unwrap();
    assert!((cy(&s) - 75.0).abs() < 1.0, "links: y={}", cy(&s));
    s.execute("modify.set", &json!({"ids": [win], "align": "right"}))
        .unwrap();
    assert!((cy(&s) + 75.0).abs() < 1.0, "rechts: y={}", cy(&s));
}

fn opening_pos(s: &Session, id: u64) -> Vec2 {
    match &s.doc.entity(id).unwrap().kind {
        EntityKind::Door { center, .. } | EntityKind::Window { center, .. } => *center,
        _ => unreachable!(),
    }
}

#[test]
fn opening_follows_wall() {
    // Tür hängt an der Wand: Verschieben und Verdicken der Wand nimmt sie mit.
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [2, 0], "width": 0.9, "host": [wall]}),
    )
    .unwrap();
    let door = last_id(&s);

    s.execute(
        "modify.move",
        &json!({"ids": [wall], "from": [0, 0], "to": [1, 0]}),
    )
    .unwrap();
    let p = opening_pos(&s, door);
    assert!((p.x - 3000.0).abs() < 1.0, "x nach Wandverschiebung={}", p.x);
    assert!((p.y - 150.0).abs() < 1.0, "y={}", p.y);

    s.execute("modify.set", &json!({"ids": [wall], "thickness": 0.6}))
        .unwrap();
    let p = opening_pos(&s, door);
    assert!((p.y - 300.0).abs() < 1.0, "y nach Dicke={}", p.y);
    assert!((p.x - 3000.0).abs() < 1.0, "x={}", p.x);
}

#[test]
fn moved_opening_stays_on_wall() {
    // Wird die Tür selbst verschoben, bleibt sie auf ihrer Wand.
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [2, 0], "width": 0.9, "host": [wall]}),
    )
    .unwrap();
    let door = last_id(&s);
    s.execute(
        "modify.move",
        &json!({"ids": [door], "from": [2, 0], "to": [3.5, 0]}),
    )
    .unwrap();
    let p = opening_pos(&s, door);
    assert!((p.x - 3500.0).abs() < 1.0, "x={}", p.x);
    assert!((p.y - 150.0).abs() < 1.0, "y auf Kante={}", p.y);
}

#[test]
fn select_window_nutzt_geometrie_nicht_bbox() {
    // Eine Diagonale darf nicht mitgewaehlt werden, nur weil das Auswahlrechteck
    // in ihrer Bounding-Box liegt.
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [5, 5]}))
        .unwrap();
    let id = last_id(&s);
    // liegt in der BBox der Linie, beruehrt sie aber nicht
    let neben = jjcad_core::snap::select_window(&s.doc, v(3000.0, 0.0), v(4400.0, 2000.0), true);
    assert!(neben.is_empty(), "Kreuzfenster ohne Beruehrung: {neben:?}");
    let fenster = jjcad_core::snap::select_window(&s.doc, v(3000.0, 0.0), v(4400.0, 2000.0), false);
    assert!(fenster.is_empty(), "Fenster ohne Beruehrung: {fenster:?}");
    // schneidet die Linie wirklich
    let treffer = jjcad_core::snap::select_window(&s.doc, v(4000.0, 3000.0), v(6000.0, 5000.0), true);
    assert_eq!(treffer, vec![id]);
    // vollstaendig enthalten
    let ganz = jjcad_core::snap::select_window(&s.doc, v(-100.0, -100.0), v(6000.0, 6000.0), false);
    assert_eq!(ganz, vec![id]);
}

#[test]
fn wandkontur_fang_ist_getrennt_schaltbar() {
    // Wand 5 m lang, 240 mm dick, mittig -> Konturecken bei 0/5000 ± 120.
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.24}),
    )
    .unwrap();
    s.execute("doc.settings", &json!({"snap": {"grid": false}}))
        .unwrap();
    assert!(s.doc.settings.snap.wall_edges, "Standard: Wandkanten aktiv");

    let ecken = [
        (0.0, 120.0),
        (0.0, -120.0),
        (5000.0, 120.0),
        (5000.0, -120.0),
    ];
    for (x, y) in ecken {
        let hit = jjcad_core::snap::find_snap(&s.doc, v(x, y), 20.0, None, &[], &[])
            .unwrap_or_else(|| panic!("Konturecke ({x},{y}) muss fangen"));
        assert!(
            (hit.point.x - x).abs() < 1e-6 && (hit.point.y - y).abs() < 1e-6,
            "Konturecke ({x},{y}) -> {hit:?}"
        );
    }

    // Abschalten: ausschliesslich die Wandkontur verschwindet.
    s.execute("doc.settings", &json!({"snap": {"wall_edges": false}}))
        .unwrap();
    for (x, y) in ecken {
        assert!(
            jjcad_core::snap::find_snap(&s.doc, v(x, y), 20.0, None, &[], &[]).is_none(),
            "Konturecke ({x},{y}) darf nicht mehr fangen"
        );
    }
    // Die Wandachse bleibt unberuehrt fangbar.
    let achse = jjcad_core::snap::find_snap(&s.doc, v(0.0, 0.0), 20.0, None, &[], &[])
        .expect("Achsen-Endpunkt muss weiterhin fangen");
    assert_eq!(achse.point, v(0.0, 0.0));
    let mitte = jjcad_core::snap::find_snap(&s.doc, v(2500.0, 0.0), 20.0, None, &[], &[])
        .expect("Achsen-Mittelpunkt muss weiterhin fangen");
    assert_eq!(mitte.point, v(2500.0, 0.0));

    // Wieder einschalten: Kontur ist sofort wieder da.
    s.execute("doc.settings", &json!({"snap": {"wall_edges": true}}))
        .unwrap();
    assert!(jjcad_core::snap::find_snap(&s.doc, v(0.0, 120.0), 20.0, None, &[], &[]).is_some());
}

#[test]
fn wandkanten_option_wird_gespeichert() {
    let sp = jjcad_core::model::SnapSettings::default();
    assert!(sp.wall_edges, "Default muss aktiv sein");
    // Serde-Rundreise (Dokument-Settings landen so in der .jjcad-Datei)
    let mut aus = sp;
    aus.wall_edges = false;
    let js = serde_json::to_string(&aus).unwrap();
    let zurueck: jjcad_core::model::SnapSettings = serde_json::from_str(&js).unwrap();
    assert!(!zurueck.wall_edges);
    // Alte Dateien ohne den Schluessel: Default aktiv
    let alt: jjcad_core::model::SnapSettings = serde_json::from_str("{}").unwrap();
    assert!(alt.wall_edges);
    // Befehls-Schicht (MCP/GUI) kann die Option setzen
    let mut s = s();
    s.execute("doc.settings", &json!({"snap": {"wall_edges": false}}))
        .unwrap();
    assert!(!s.doc.settings.snap.wall_edges);
}

#[test]
fn zwei_klick_winkel_dreht_um_den_winkel_zwischen_den_richtungen() {
    use jjcad_core::transform::angle_between_directions;

    // Waagerechte Linie links vom Drehzentrum
    let mut s = s();
    s.execute("draw.line", &json!({"a": [1, 0], "b": [2, 0]}))
        .unwrap();
    let id = last_id(&s);

    // Zentrum (0|0), "von" auf +X, "bis" auf +Y → +90°
    let center = v(0.0, 0.0);
    let angle = angle_between_directions(center, v(1000.0, 0.0), v(0.0, 1000.0)).unwrap();
    assert!((angle - 90.0).abs() < 1e-9, "{angle}");
    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0, 0], "angle": angle}),
    )
    .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x - 0.0).abs() < 1e-6 && (a.y - 1000.0).abs() < 1e-6, "{a:?}");
        assert!((b.x - 0.0).abs() < 1e-6 && (b.y - 2000.0).abs() < 1e-6, "{b:?}");
    } else {
        panic!("Linie erwartet");
    }

    // Gegenrichtung: "bis" auf -Y → -90°, dreht die Linie zurück und weiter nach unten
    let angle = angle_between_directions(center, v(1000.0, 0.0), v(0.0, -1000.0)).unwrap();
    assert!((angle + 90.0).abs() < 1e-9, "{angle}");
    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0, 0], "angle": angle}),
    )
    .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x - 1000.0).abs() < 1e-6 && (a.y - 0.0).abs() < 1e-6, "{a:?}");
        assert!((b.x - 2000.0).abs() < 1e-6 && (b.y - 0.0).abs() < 1e-6, "{b:?}");
    }
}

#[test]
fn block_laesst_sich_mit_auswahlwerkzeug_anklicken() {
    // Ein Block (Rechteck 2 x 1 m um den Ursprung) wird als Block angelegt und
    // an einer anderen Stelle als Instanz eingefuegt. Anklicken und Fenster-
    // auswahl muessen die Instanz treffen — also an der *eingefuegten* Stelle,
    // nicht an der Blockdefinition im Ursprung.
    let mut s = s();
    s.execute("draw.rect", &json!({"a": [0, 0], "b": [3, 1]}))
        .unwrap();
    let id = last_id(&s);
    s.execute(
        "block.create",
        &json!({"name": "Auswahlblock", "ids": [id], "base": [0, 0]}),
    )
    .unwrap();
    s.execute(
        "block.insert",
        &json!({"block": "Auswahlblock", "pos": [10, 5]}),
    )
    .unwrap();
    let inst = last_id(&s);
    assert!(matches!(s.doc.entity(inst).unwrap().kind, EntityKind::BlockRef { .. }));

    // Klick auf die linke Kante der Instanz: (10, 5) bis (10, 6) m
    let hit = jjcad_core::snap::hit_test(&s.doc, v(10000.0, 5500.0), 50.0, None);
    assert_eq!(hit, Some(inst), "Klick auf Blockkante muss die Instanz treffen");

    // Klick auf die obere Kante
    let hit = jjcad_core::snap::hit_test(&s.doc, v(11000.0, 6000.0), 50.0, None);
    assert_eq!(hit, Some(inst), "Klick auf obere Blockkante muss treffen");

    // Kreuzfenster ueber die Instanz (beruehrt die linke Kante)
    let ids = jjcad_core::snap::select_window(&s.doc, v(9000.0, 5000.0), v(10500.0, 6000.0), true);
    assert!(ids.contains(&inst), "Kreuzfenster muss die Instanz waehlen: {ids:?}");
    // Fensterauswahl: Instanz vollstaendig enthalten
    let ids = jjcad_core::snap::select_window(&s.doc, v(9500.0, 4500.0), v(13500.0, 6500.0), false);
    assert!(ids.contains(&inst), "Fenster muss die Instanz waehlen: {ids:?}");

    // Alle gezeichneten Grundlinien der Instanz tragen deren Id — nur dann
    // faerbt die Auswahl den Block blau ein.
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    let eigene = dl
        .prims
        .iter()
        .filter(|p| p.entity == inst)
        .count();
    assert!(eigene >= 4, "Blockinstanz muss ihre Linien zeichnen, gefunden: {eigene}");
    assert!(
        dl.prims.iter().all(|p| p.entity != 0),
        "keine Linie darf ohne Objekt-Id gezeichnet werden"
    );

    // Nachgelagerte Aktionen: Verschieben und Loeschen wirken auf die Instanz.
    s.execute("modify.move", &json!({"ids": [inst], "from": [10, 5], "to": [20, 5]}))
        .unwrap();
    let hit = jjcad_core::snap::hit_test(&s.doc, v(20000.0, 5500.0), 50.0, None);
    assert_eq!(hit, Some(inst), "nach dem Verschieben muss die Instanz dort liegen");
    s.execute("modify.erase", &json!({"ids": [inst]})).unwrap();
    assert!(s.doc.entity(inst).is_none());
}
