//! T0235 — PNG-Export eines **Bereichs**.
//!
//! Der Bereich ist immer ein Rechteck und wird über seine Diagonale angegeben.
//! Geprüft wird, was der Export liefert:
//!
//! 1. Das Bild zeigt genau den Bereich — kein 3-%-Rand wie bei der ganzen
//!    Zeichnung, eine Linie auf der Bereichskante reicht bis an den Bildrand.
//! 2. Die Höhe folgt aus dem Seitenverhältnis (keine Verzerrung).
//! 3. Was außerhalb des Bereichs liegt, ist nicht im Bild.
//! 4. Eine Diagonale ohne Fläche ist ein Fehler, kein stilles Bild.

use jjcad_core::raster::{render_bereich_pixmap, render_png_bereich};
use jjcad_core::session::Session;
use serde_json::json;
use tiny_skia::Pixmap;

/// Zeichnung mit einer Linie quer durch den Bereich und einer weit außerhalb.
fn zeichnung() -> Session {
    let mut s = Session::new();
    // Millimeter als Eingabeeinheit und Maßstab 1:1 — sonst rechnet die Aktion
    // die Koordinaten in Metern und der Beschriftungsmaßstab macht aus einer
    // 0,5-mm-Linie eine 50 mm breite.
    s.execute("doc.settings", &json!({"input_unit": "mm"})).expect("Einstellungen");
    // T0232: Der Maßstab gehört zum Plan — 1:1, damit die 0,5-mm-Linie nicht
    // über den Planmaßstab auf 50 mm anwächst.
    s.doc.plans[0].scale = 1.0;
    s.execute(
        "draw.line",
        &json!({"a": [0.0, 0.0], "b": [1000.0, 0.0], "layer": "Wände", "lineweight": 0.5}),
    )
    .expect("Linie im Bereich");
    s.execute(
        "draw.line",
        &json!({"a": [0.0, 3000.0], "b": [1000.0, 3000.0], "layer": "Wände", "lineweight": 0.5}),
    )
    .expect("Linie außerhalb");
    s
}

fn dunkel(pm: &Pixmap, x: u32, y: u32) -> bool {
    // Die freie Geometrie liegt auf der hellen Ebene „Hilfslinien"; alles, was
    // nicht Papierweiß ist, zählt als Strich.
    match pm.pixel(x, y) {
        Some(p) => (p.red() as u32 + p.green() as u32 + p.blue() as u32) < 720,
        None => false,
    }
}

/// Erste und letzte Spalte mit einem dunklen Punkt in der Zeile `y`.
fn spannen(pm: &Pixmap, y: u32) -> Option<(u32, u32)> {
    let mut min = None;
    let mut max = None;
    for x in 0..pm.width() {
        if dunkel(pm, x, y) {
            min.get_or_insert(x);
            max = Some(x);
        }
    }
    min.zip(max)
}

#[test]
fn bereich_fuellt_das_bild_ohne_rand() {
    let s = zeichnung();
    // Quadratischer Bereich 1000 × 1000 mm um die Linie in der Mitte.
    let pm = render_bereich_pixmap(&s.doc, [0.0, -500.0, 1000.0, 500.0], None, 400).expect("Bild");
    assert_eq!((pm.width(), pm.height()), (400, 400), "Bildgröße folgt dem Bereich");

    // Die Linie liegt auf y = 0, also in der Bildmitte.
    let (links, rechts) = spannen(&pm, 200).expect("Linie im Bild");
    // Ohne den 3-%-Rand reicht sie bis an beide Bildränder (Strichbreite 0,5 mm
    // bei 0,4 px/mm = 0,2 px, also praktisch die Kante selbst).
    assert!(links <= 1, "Linie beginnt erst bei Spalte {links} — Rand im Bild?");
    assert!(rechts >= 398, "Linie endet bei Spalte {rechts} — Rand im Bild?");

    // Kein senkrechter Rand: die Zeile über und unter der Linie ist weiß.
    let (min_y, max_y) = {
        let mut min = None;
        let mut max = None;
        for y in 0..pm.height() {
            if dunkel(&pm, 200, y) {
                min.get_or_insert(y);
                max = Some(y);
            }
        }
        (min.expect("Linienzeile"), max.expect("Linienzeile"))
    };
    assert!(
        min_y >= 190 && max_y <= 210,
        "Linie liegt nicht in der Bildmitte: {min_y}…{max_y}"
    );
    assert!(!dunkel(&pm, 200, 0), "oben ist ein Rand statt Weiß");
    assert!(!dunkel(&pm, 200, pm.height() - 1), "unten ist ein Rand statt Weiß");
}

#[test]
fn hoehe_folgt_dem_seitenverhaeltnis() {
    let s = zeichnung();
    // 2000 × 1000 mm → 400 × 200 px, unverzerrt.
    let pm = render_bereich_pixmap(&s.doc, [0.0, -500.0, 2000.0, 500.0], None, 400).expect("Bild");
    assert_eq!((pm.width(), pm.height()), (400, 200));

    // 1000 × 500 mm → 400 × 200 px.
    let pm = render_bereich_pixmap(&s.doc, [0.0, 0.0, 1000.0, 500.0], None, 400).expect("Bild");
    assert_eq!((pm.width(), pm.height()), (400, 200));
}

#[test]
fn nur_der_bereich_steht_im_bild() {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"})).expect("Einstellungen");
    // T0232: Der Maßstab gehört zum Plan — 1:1, damit die 0,5-mm-Linie nicht
    // über den Planmaßstab auf 50 mm anwächst.
    s.doc.plans[0].scale = 1.0;
    // Eine Linie in der linken Hälfte des Bereichs, eine weit darunter.
    s.execute(
        "draw.line",
        &json!({"a": [0.0, 0.0], "b": [500.0, 0.0], "layer": "Wände", "lineweight": 0.5}),
    )
    .expect("Linie");
    s.execute(
        "draw.line",
        &json!({"a": [0.0, 5000.0], "b": [500.0, 5000.0], "layer": "Wände", "lineweight": 0.5}),
    )
    .expect("Linie außerhalb");

    let pm = render_bereich_pixmap(&s.doc, [0.0, -500.0, 1000.0, 500.0], None, 400).expect("Bild");
    let (_, rechts) = spannen(&pm, 200).expect("Linie im Bild");
    // Die Linie endet bei 500 mm, also in der Bildmitte — die rechte Hälfte
    // bleibt weiß, und die Linie bei y = 5000 liegt außerhalb.
    assert!(rechts <= 205, "Linie reicht bis Spalte {rechts}, obwohl sie bei 500 mm endet");
    assert!(!dunkel(&pm, 399, 200), "rechte Hälfte ist nicht weiß");
}

#[test]
fn diagonale_ohne_flaeche_ist_ein_fehler() {
    let s = zeichnung();
    let leer = render_bereich_pixmap(&s.doc, [100.0, 100.0, 100.0, 400.0], None, 400);
    assert!(leer.is_err(), "senkrechte Diagonale ohne Breite muss ein Fehler sein");
    let leer = render_bereich_pixmap(&s.doc, [100.0, 100.0, 400.0, 100.0], None, 400);
    assert!(leer.is_err(), "waagerechte Diagonale ohne Höhe muss ein Fehler sein");
}

#[test]
fn bereich_wird_als_png_datei_geschrieben() {
    let s = zeichnung();
    let dir = std::env::temp_dir().join(format!("jjcad-t0235-{}", std::process::id()));
    let pfad = dir.join("bereich.png");
    render_png_bereich(&s.doc, [0.0, -500.0, 1000.0, 500.0], None, 400, &pfad).expect("PNG");
    let pm = Pixmap::load_png(&pfad).expect("PNG lesbar");
    assert_eq!((pm.width(), pm.height()), (400, 400));
    assert!(dunkel(&pm, 200, 200), "die Linie fehlt im geschriebenen PNG");
    let _ = std::fs::remove_file(&pfad);
    let _ = std::fs::remove_dir(&dir);
}
