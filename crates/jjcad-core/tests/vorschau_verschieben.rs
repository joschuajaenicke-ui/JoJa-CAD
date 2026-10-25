//! Nachweis für die Vorschau beim Verschieben/Kopieren.
//!
//! `render::preview_selection` muss für **jeden** Elementtyp Geometrie an der
//! Zielposition liefern — dieselbe Geometrie wie das Original, nur um `delta`
//! versetzt. Vorher hatte nur `EntityKind::Line` eine Vorschau.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::render;
use jjcad_core::session::Session;
use serde_json::json;

const DELTA_MM: (f64, f64) = (1500.0, 2000.0);

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

fn last_id(s: &Session) -> u64 {
    s.doc.entities.last().unwrap().id
}

/// Zeichnet je ein Element jedes unterstützten Typs und gibt (Name, Id) zurück.
fn alle_elemente(s: &mut Session) -> Vec<(&'static str, u64)> {
    macro_rules! zeichnen {
        ($s:ident, $out:ident, $name:expr, $action:expr, $params:expr) => {{
            $s.execute($action, &$params)
                .unwrap_or_else(|e| panic!("{}: {e}", $action));
            $out.push(($name, last_id($s)));
        }};
    }
    let mut out: Vec<(&'static str, u64)> = Vec::new();
    // Blockdefinition zuerst: `block.create` löst die Quellelemente in die
    // Definition auf, deshalb eigene Hilfselemente dafür verwenden.
    s.execute("draw.line", &json!({"a": [0, 0], "b": [1, 0]}))
        .unwrap();
    let hilfs_linie = last_id(s);
    s.execute("draw.circle", &json!({"c": [0, 0], "r": 0.25}))
        .unwrap();
    let hilfs_kreis = last_id(s);
    s.execute(
        "block.create",
        &json!({"ids": [hilfs_linie, hilfs_kreis], "name": "Testblock"}),
    )
    .unwrap();
    let block = s.doc.blocks.last().unwrap().id.clone();

    zeichnen!(s, out, "Punkt", "draw.point", json!({"p": [0.5, 0.5]}));
    zeichnen!(s, out, "Linie", "draw.line", json!({"a": [0, 0], "b": [2, 0]}));
    zeichnen!(s, out, "Kreis", "draw.circle", json!({"c": [1, 1], "r": 0.5}));
    zeichnen!(s, out, 
        "Bogen",
        "draw.arc",
        json!({"c": [0, 0], "r": 1.0, "a0": 0, "a1": 90})
    );
    zeichnen!(s, out, 
        "Ellipse",
        "draw.ellipse",
        json!({"c": [0, 0], "rx": 1, "ry": 0.5})
    );
    zeichnen!(s, out, "Rechteck", "draw.rect", json!({"a": [0, 0], "b": [3, 2]}));
    zeichnen!(s, out, 
        "Polylinie",
        "draw.polyline",
        json!({"pts": [[0, 0], [1, 1], [2, 0]]})
    );
    zeichnen!(s, out, 
        "Schraffur",
        "draw.hatch",
        json!({"boundary": [[0, 0], [1, 0], [1, 1], [0, 1]]})
    );
    zeichnen!(s, out, 
        "Text",
        "draw.text",
        json!({"p": [0, 0], "text": "Hallo", "height": 0.25})
    );
    zeichnen!(s, out, 
        "Maß",
        "dim.linear",
        json!({"a": [0, 0], "b": [2, 0], "line_pos": [0, -0.5]})
    );
    zeichnen!(s, out, 
        "Maßkette",
        "dim.chain",
        json!({"pts": [[0, 0], [1, 0], [2, 0]], "line_pos": [0, -0.5]})
    );
    zeichnen!(s, out, "Kote", "dim.elevation", json!({"p": [0, 0], "z": 0.5}));
    zeichnen!(s, out, 
        "Wand",
        "draw.wall",
        json!({"a": [0, 0], "b": [5, 0], "thickness": 0.30})
    );
    let wand = last_id(s);
    zeichnen!(s, out, 
        "Tür",
        "draw.door",
        json!({"center": [2, 0], "angle": 0, "width": 0.9, "host": [wand]})
    );
    zeichnen!(s, out, 
        "Fenster",
        "draw.window",
        json!({"center": [3.5, 0], "angle": 0, "width": 1.4, "host": [wand]})
    );
    zeichnen!(s, out, 
        "Treppe",
        "draw.stair",
        json!({"a": [0, 3], "b": [3, 3], "width": 1.0, "steps": 10})
    );

    zeichnen!(
        s,
        out,
        "Block",
        "block.insert",
        json!({"block": block, "pos": [6, 6], "rot": 0, "scale": 1})
    );

    out
}

#[test]
fn vorschau_liefert_geometrie_fuer_jeden_elementtyp() {
    let mut s = session();
    let elemente = alle_elemente(&mut s);
    let delta = v(DELTA_MM.0, DELTA_MM.1);

    for (name, id) in &elemente {
        let e = s
            .doc
            .entity(*id)
            .unwrap_or_else(|| panic!("{name}: Id {id} fehlt im Dokument"))
            .clone();
        let original = render::entity_bbox(&s.doc, &e);
        assert!(original.is_valid(), "{name}: Original ohne Geometrie");

        let dl = render::preview_selection(&s.doc, &[*id], delta);
        assert!(
            !dl.prims.is_empty(),
            "{name} ({:?}) bekommt keine Vorschau",
            std::mem::discriminant(&e.kind)
        );
        assert!(dl.bbox.is_valid(), "{name}: Vorschau ohne Bounding-Box");

        // Vorschau = Original, nur um delta versetzt
        let erwartet_min = original.min + delta;
        let erwartet_max = original.max + delta;
        assert!(
            (dl.bbox.min - erwartet_min).len() < 0.01,
            "{name}: Vorschau-Min {:?} statt {:?}",
            dl.bbox.min,
            erwartet_min
        );
        assert!(
            (dl.bbox.max - erwartet_max).len() < 0.01,
            "{name}: Vorschau-Max {:?} statt {:?}",
            dl.bbox.max,
            erwartet_max
        );
        assert!(
            !matches!(e.kind, EntityKind::Line { .. }) || dl.prims.len() == 1,
            "Linien-Vorschau muss genau ein Primitiv bleiben"
        );
    }
}

#[test]
fn vorschau_mehrfachauswahl_und_abbruch() {
    let mut s = session();
    let elemente = alle_elemente(&mut s);
    let ids: Vec<u64> = elemente.iter().map(|(_, id)| *id).collect();
    let delta = v(DELTA_MM.0, DELTA_MM.1);

    let einzeln: usize = ids
        .iter()
        .map(|id| render::preview_selection(&s.doc, &[*id], delta).prims.len())
        .sum();
    let alle = render::preview_selection(&s.doc, &ids, delta);
    assert_eq!(alle.prims.len(), einzeln, "Mehrfachauswahl verliert Primitive");

    // Gesamt-Bounding-Box = Vereinigung der verschobenen Einzel-Boxen
    let mut erwartet = jjcad_core::geom::BBox::EMPTY;
    for id in &ids {
        let e = s.doc.entity(*id).unwrap();
        erwartet.union(render::entity_bbox(&s.doc, e));
    }
    erwartet = jjcad_core::geom::BBox::new(erwartet.min + delta, erwartet.max + delta);
    assert!((alle.bbox.min - erwartet.min).len() < 0.01);
    assert!((alle.bbox.max - erwartet.max).len() < 0.01);

    // Leere Auswahl (Abbruch) liefert nichts — es bleibt keine Vorschau zurück
    let leer = render::preview_selection(&s.doc, &[], delta);
    assert!(leer.prims.is_empty());
    assert!(!leer.bbox.is_valid());
}

#[test]
fn vorschau_verschiebung_null_bewegt_nichts() {
    let mut s = session();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [4, 0], "thickness": 0.3}))
        .unwrap();
    let id = last_id(&s);
    let e = s.doc.entity(id).unwrap().clone();
    let original = render::entity_bbox(&s.doc, &e);
    let dl = render::preview_selection(&s.doc, &[id], Vec2::ZERO);
    assert!(!dl.prims.is_empty());
    assert!((dl.bbox.min - original.min).len() < 0.01);
    assert!((dl.bbox.max - original.max).len() < 0.01);
}
