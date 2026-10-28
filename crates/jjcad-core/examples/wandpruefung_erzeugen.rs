//! Erzeugt `samples/Wandpruefung.jjcad` mit den Anschlussfällen, die sauber
//! gegehrt sein müssen (9er-Ecken, dicke/ausrichtung, drei Wände, Stich,
//! Verlängerung). Nur zum Erzeugen der Prüfzeichnung gedacht.

use jjcad_core::persist;
use jjcad_core::Session;
use serde_json::json;

fn wand(s: &mut Session, a: (f64, f64), b: (f64, f64), t: f64, al: &str) {
    s.execute(
        "draw.wall",
        &json!({"a": [a.0 / 1000.0, a.1 / 1000.0], "b": [b.0 / 1000.0, b.1 / 1000.0], "thickness": t / 1000.0, "align": al}),
    )
    .unwrap();
}

fn main() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s.doc.name = "Wandanschlüsse Prüfung".to_string();

    // 1) 90°, gleich dick, mittig
    wand(&mut s, (0.0, 0.0), (2000.0, 0.0), 240.0, "center");
    wand(&mut s, (2000.0, 0.0), (2000.0, 2000.0), 240.0, "center");

    // 2) 90°, ungleich dick
    wand(&mut s, (4000.0, 0.0), (6000.0, 0.0), 365.0, "center");
    wand(&mut s, (6000.0, 0.0), (6000.0, 2000.0), 115.0, "center");

    // 3) 90°, unterschiedliche Ausrichtung
    wand(&mut s, (0.0, 3000.0), (2000.0, 3000.0), 240.0, "left");
    wand(&mut s, (2000.0, 3000.0), (2000.0, 5000.0), 240.0, "right");

    // 4) drei Wände in einem Knoten
    wand(&mut s, (4000.0, 3000.0), (6000.0, 3000.0), 300.0, "center");
    wand(&mut s, (6000.0, 3000.0), (6000.0 - 2000.0 * 0.5, 3000.0 + 2000.0 * 0.8660254), 200.0, "center");
    wand(&mut s, (6000.0, 3000.0), (6000.0 - 2000.0 * 0.5, 3000.0 - 2000.0 * 0.8660254), 200.0, "center");

    // 5) Stich: dünne Wand endet auf der Basislinie einer durchlaufenden Wand
    wand(&mut s, (0.0, 6000.0), (4000.0, 6000.0), 300.0, "center");
    wand(&mut s, (2000.0, 6000.0), (2000.0 + 1500.0 * 0.9396926, 6000.0 + 1500.0 * 0.3420201), 115.0, "center");

    // 6) Verlängerung: fluchtend, gleiche Dicke (keine Querlinie)
    wand(&mut s, (4000.0, 6000.0), (5000.0, 6000.0), 240.0, "center");
    wand(&mut s, (5000.0, 6000.0), (6000.0, 6000.0), 240.0, "center");

    // 7) Verlängerung mit Wechsel der Ausrichtung (sichtbare Stufe, keine Querlinie im Körper)
    wand(&mut s, (4000.0, 6500.0), (5000.0, 6500.0), 240.0, "center");
    wand(&mut s, (5000.0, 6500.0), (6000.0, 6500.0), 240.0, "left");

    let pfad = std::path::Path::new("samples/Wandpruefung.jjcad");
    persist::save_document(&mut s.doc, pfad).expect("Zeichnung speichern");
    println!("{}", pfad.display());
}
