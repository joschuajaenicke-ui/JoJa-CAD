//! Erzeugt `samples/Volumen.jjcad`: runde Wand (Bogen) auf zwei geraden Wänden.
//!
//! Das ist der Fall aus der Praxis: Mauerwerk, das sich nicht als Rechteckwand
//! zeichnen lässt. Der Bogen ist ein Volumen; seine Endflächen liegen in den
//! Wänden und verschwinden dort, die Wandkanten unter dem Bogen ebenso.
use jjcad_core::export::RasterOptions;
use jjcad_core::raster::render_png;
use jjcad_core::{persist, Session};
use serde_json::json;

fn ring_segment(c: (f64, f64), r0: f64, r1: f64, a0: f64, a1: f64, n: usize) -> Vec<[f64; 2]> {
    let mut b = Vec::new();
    for k in 0..=n {
        let t = a0 + (a1 - a0) * (k as f64 / n as f64);
        b.push([c.0 + r1 * t.cos(), c.1 + r1 * t.sin()]);
    }
    for k in (0..=n).rev() {
        let t = a0 + (a1 - a0) * (k as f64 / n as f64);
        b.push([c.0 + r0 * t.cos(), c.1 + r0 * t.sin()]);
    }
    b
}

/// Ausgabeverzeichnis der Probe-Bilder: `JJCAD_OUT_DIR`, sonst temporär.
fn ausgabe_dir() -> std::path::PathBuf {
    let dir = std::env::var_os("JJCAD_OUT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("jjcad-proben"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn main() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    // Zwei Wände, die unter dem Bogen durchlaufen (Band ±120 mm um y = 0)
    s.execute("draw.wall", &json!({"a": [-4.5, 0.0], "b": [1.2, 0.0], "thickness": 0.24})).unwrap();
    s.execute("draw.wall", &json!({"a": [4.5, 0.0], "b": [-1.2, 0.0], "thickness": 0.24})).unwrap();
    // Bogen: Halbkreis, Außenradius 1,20 m, Innenradius 0,96 m — die Endflächen
    // liegen in den Wänden.
    let bogen = ring_segment((0.0, 0.0), 0.96, 1.20, 0.0, std::f64::consts::PI, 64);
    s.execute(
        "draw.volume",
        &json!({"boundary": bogen, "height": 2.75, "border": true, "pattern": "solid", "scale": 0.25}),
    )
    .unwrap();
    let text = persist::to_json(&s.doc).unwrap();
    // Ziel relativ zum Repo-Wurzelverzeichnis (Beispiele laufen im Crate-Ordner)
    let ziel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/Volumen.jjcad");
    if let Some(dir) = ziel.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&ziel, text).unwrap();
    println!("geschrieben: {}", ziel.display());
    let opts = RasterOptions {
        width_px: 1400,
        height_px: 900,
        bbox: Some([-5000.0, -500.0, 5000.0, 1800.0]),
        paper: false,
        annot_scale: Some(1.0),
        sheet: None,
    };
    let ziel_png = ausgabe_dir().join("volumen_bogen.png");
    render_png(&s.doc, &opts, &ziel_png).unwrap();
    println!("PNG: {}", ziel_png.display());
    // Ausschnitt am Kämpfer: dort müssen die Endfläche des Bogens und die
    // Wandkante darunter verschwunden sein.
    let zoom = RasterOptions {
        width_px: 1600,
        height_px: 900,
        bbox: Some([700.0, -400.0, 1500.0, 400.0]),
        paper: false,
        annot_scale: Some(1.0),
        sheet: None,
    };
    let zoom_png = ausgabe_dir().join("volumen_zoom.png");
    render_png(&s.doc, &zoom, &zoom_png).unwrap();
    println!("PNG: {}", zoom_png.display());
    for e in &s.doc.entities {
        println!("{}: {:?} m³", e.kind.type_name(), e.volume_m3());
    }
}
