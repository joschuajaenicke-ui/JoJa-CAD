//! Erzeugt eine Beispielwohnung und exportiert PDF, PNG und DXF.
//!
//! Aufruf: `cargo run -p jjcad-core --example demo_plan -- /tmp/ausgabe`

use jjcad_core::export::RasterOptions;
use jjcad_core::{actions, pdf, raster, Session};
use serde_json::json;
use std::path::PathBuf;

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/jjcad-demo"));
    std::fs::create_dir_all(&out).unwrap();

    let mut s = Session::new();
    s.doc.project.project = "Wohnhaus Musterstraße 12".into();
    s.doc.project.client = "Bauherr Max Mustermann".into();
    s.doc.project.planner = "Planungsbüro JoJa".into();
    s.doc.project.address = "12345 Musterstadt, Musterstraße 12".into();
    s.doc.project.plan_no = "A-01".into();
    s.doc.project.plan_title = "Grundriss Erdgeschoss".into();
    s.doc.project.revision = "0".into();
    s.doc.settings.input_unit = jjcad_core::units::Unit::M;
    s.doc.settings.dim_unit = jjcad_core::units::Unit::M;
    s.doc.name = "Wohnhaus".into();

    macro_rules! run {
        ($action:expr, $params:expr $(,)?) => {{
            let action: &str = $action;
            let params: serde_json::Value = $params;
            match s.execute(action, &params) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("FEHLER bei {action}: {e}\n  {params}");
                    std::process::exit(1);
                }
            }
        }};
    }

    // ---- Wände (Außen 300 mm, innen 115 mm) ----
    let walls = [
        (0.0, 0.0, 10.5, 0.0, 0.30),
        (10.5, 0.0, 10.5, 8.0, 0.30),
        (10.5, 8.0, 0.0, 8.0, 0.30),
        (0.0, 8.0, 0.0, 0.0, 0.30),
        (0.0, 5.2, 6.5, 5.2, 0.115),
        (6.5, 0.0, 6.5, 5.2, 0.115),
        (0.0, 5.2, 0.0, 8.0, 0.0), // Platzhalter, wird nicht gezeichnet
    ];
    let mut wall_ids = Vec::new();
    for (x0, y0, x1, y1, t) in walls {
        if t <= 0.0 {
            continue;
        }
        let v = run!(
            "draw.wall",
            json!({"a": [x0, y0], "b": [x1, y1], "thickness": t, "layer": "Wände"}),
        );
        wall_ids.push(v["created"][0].as_u64().unwrap());
    }
    // Bad-Trennwand
    let v = run!(
        "draw.wall",
        json!({"a": [3.0, 5.2], "b": [3.0, 8.0], "thickness": 0.115, "layer": "Wände"}),
    );
    wall_ids.push(v["created"][0].as_u64().unwrap());

    // ---- Türen ----
    // Hauseingang (unten), Innentüren
    run!(
        "draw.door",
        json!({"center": [1.6, 0.0], "width": 1.0, "hinge": -1.0, "swing_in": true, "layer": "Türen"}),
    );
    run!(
        "draw.door",
        json!({"center": [4.2, 5.2], "width": 0.885, "hinge": 1.0, "swing_in": true, "layer": "Türen"}),
    );
    run!(
        "draw.door",
        json!({"center": [6.5, 2.6], "width": 0.885, "hinge": -1.0, "swing_in": false, "layer": "Türen"}),
    );
    run!(
        "draw.door",
        json!({"center": [3.0, 6.6], "width": 0.75, "hinge": 1.0, "swing_in": true, "layer": "Türen", "kind": "sliding"}),
    );

    // ---- Fenster ----
    for (x, y, w) in [(6.3, 0.0, 1.8), (10.5, 3.2, 2.0), (7.5, 8.0, 1.6), (0.0, 6.6, 1.2)] {
        run!(
            "draw.window",
            json!({"center": [x, y], "width": w, "layer": "Fenster"}),
        );
    }

    // ---- Treppe ----
    run!(
        "draw.stair",
        json!({"a": [7.3, 1.2], "b": [7.3, 4.6], "width": 1.0, "steps": 17, "layer": "Treppen"}),
    );

    // ---- Bemaßung ----
    run!(
        "dim.chain",
        json!({"pts": [[0.0, 0.0], [3.0, 0.0], [6.5, 0.0], [10.5, 0.0]], "line_pos": [0.0, -1.4], "axis": "x"}),
    );
    run!(
        "dim.chain",
        json!({"pts": [[0.0, 0.0], [0.0, 5.2], [0.0, 8.0]], "line_pos": [-1.4, 0.0], "axis": "y"}),
    );
    run!(
        "dim.linear",
        json!({"a": [0.0, 8.0], "b": [10.5, 8.0], "line_pos": [5.0, 9.4], "axis": "x"}),
    );

    // ---- Texte ----
    for (x, y, t, h) in [
        (2.0, 2.6, "WOHNEN", 0.30),
        (2.0, 2.1, "32,4 m²", 0.20),
        (8.6, 6.4, "SCHLAFEN", 0.30),
        (8.6, 5.9, "18,7 m²", 0.20),
        (1.4, 6.9, "BAD", 0.25),
        (5.0, 0.9, "FLUR", 0.25),
    ] {
        run!(
            "draw.text",
            json!({"p": [x, y], "text": t, "height": h, "layer": "Text", "align": "center"}),
        );
    }

    // ---- Höhenkote ----
    run!(
        "dim.elevation.plan",
        json!({"p": [-0.2, 0.0], "z": 0.0, "layer": "Bemaßung"}),
    );

    // ---- Blöcke ----
    for (name, x, y, rot) in [
        ("Bett 1600", 8.7, 3.2, 0.0),
        ("Sofa", 2.2, 1.6, 0.0),
        ("Tisch", 4.6, 2.3, 0.0),
        ("WC", 0.45, 7.4, 270.0),
        ("Waschbecken", 1.6, 7.6, 180.0),
        ("Dusche", 2.4, 6.1, 0.0),
        ("Herd", 5.6, 4.65, 0.0),
        ("Spüle", 6.25, 4.65, 0.0),
        ("Kühlschrank", 5.6, 4.05, 0.0),
    ] {
        run!(
            "block.insert",
            json!({"block": name, "pos": [x, y], "rot": rot, "layer": "Möbel"}),
        );
    }
    // Nordpfeil
    run!(
        "block.insert",
        json!({"block": "Nordpfeil", "pos": [11.6, 9.2], "scale": 1.0, "layer": "Text"}),
    );

    // ---- Blatt aktualisieren ----
    let sheet_id = s.doc.sheets[0].id;
    run!("sheet.update", json!({"sheet": sheet_id, "scale": 60.0}));
    let vp_id = s.doc.sheets[0].viewports[0].id;
    run!(
        "sheet.update_viewport",
        json!({"sheet": sheet_id, "viewport": vp_id, "scale": 60.0, "center": [5.25, 4.1]})
    );

    let jjc = out.join("Wohnhaus.jjcad");
    s.save(&jjc).unwrap();
    println!("Projekt: {}", jjc.display());

    pdf::export_pdf(&s.doc, &out.join("Grundriss.pdf"), Some(0))
        .map(|_| println!("PDF: {}", out.join("Grundriss.pdf").display()))
        .unwrap();

    let opts = RasterOptions {
        width_px: 2200,
        height_px: 1600,
        ..Default::default()
    };
    raster::render_png(&s.doc, &opts, &out.join("Grundriss.png")).unwrap();
    println!("PNG: {}", out.join("Grundriss.png").display());

    let opts_sheet = RasterOptions {
        width_px: 2200,
        height_px: 1556,
        sheet: Some(0),
        ..Default::default()
    };
    raster::render_png(&s.doc, &opts_sheet, &out.join("Blatt.png")).unwrap();
    println!("Blatt-PNG: {}", out.join("Blatt.png").display());

    jjcad_core::dxf::export_dxf(&s.doc, &out.join("Grundriss.dxf")).unwrap();
    println!("DXF: {}", out.join("Grundriss.dxf").display());

    println!("{}", s.doc.summary());
    let _ = actions::all_actions();
}
