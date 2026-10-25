//! Testdatei: zwei parallele Linien, 0,5 mm Strichbreite, 100 mm Abstand.
//! Bei Maßstab 1:100 muss im Dauerstrich-Bild das Verhältnis
//! Linienbreite : weißer Zwischenraum = 1 : 1 sein.

use jjcad_core::model::{Document, Entity, EntityKind, Layer};
use jjcad_core::geom::{Vec2, Vertex};
use jjcad_core::style::Color;
use jjcad_core::persist;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/lw.jjcad".to_string());

    let mut doc = Document::blank();
    doc.name = "Linienbreiten-Test".into();
    doc.settings.annotation_scale = 100.0;
    doc.settings.show_grid = false;

    let lid = doc.new_id();
    let mut layer = Layer::new(lid, "Test 0,5 mm", Color::BLACK);
    layer.line_weight = 0.5;
    doc.layers.push(layer);

    for y in [0.0_f64, 100.0_f64] {
        let id = doc.new_id();
        doc.add_entity(Entity::new(
            id,
            lid,
            EntityKind::Line {
                a: Vec2::new(0.0, y),
                b: Vec2::new(240.0, y),
            },
        ));
    }

    // Rechteck als geschlossene Polylinie (600 x 400), für Griff-Tests
    {
        let id = doc.new_id();
        let pts = [(0.0, 300.0), (600.0, 300.0), (600.0, 700.0), (0.0, 700.0)];
        let verts: Vec<Vertex> = pts
            .iter()
            .map(|(x, y)| Vertex {
                p: Vec2::new(*x, *y),
                bulge: 0.0,
            })
            .collect();
        doc.add_entity(Entity::new(
            id,
            lid,
            EntityKind::Polyline {
                verts,
                closed: true,
            },
        ));
    }

    match persist::save_document(&mut doc, std::path::Path::new(&path)) {
        Ok(_) => println!("geschrieben: {path}"),
        Err(e) => eprintln!("Fehler: {e}"),
    }
}
