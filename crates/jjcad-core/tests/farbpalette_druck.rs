//! T0183 — die drei Gestaltungsfarben Gelb, Orange und Braun bleiben im
//! Druckbild unterscheidbar.
//!
//! Der Test zeichnet drei Linien in genau den Vorgabewerten der Farbliste,
//! exportiert das PNG (dieselbe Anzeigeliste wie Bildschirm und PDF) und misst
//! die Farbe jedes Strichs im Bild. Er belegt zweierlei: die Werte kommen
//! unverändert im Druck an, und die drei Töne liegen weit genug auseinander,
//! um sie zu unterscheiden.
//!
//! Mit gesetztem `JJCAD_OUT_DIR` legt der Test das Prüfbild dort ab
//! (`farbpalette_druck.png`) — so entsteht der Anhang am Ticket aus demselben
//! Lauf, der die Werte prüft.

use jjcad_core::export::RasterOptions;
use jjcad_core::raster::{render_model_pixmap, render_png};
use jjcad_core::session::Session;
use serde_json::json;

const GELB: (u8, u8, u8) = (0xEB, 0xCE, 0x2E);
const ORANGE: (u8, u8, u8) = (0xD9, 0x82, 0x2B);
const BRAUN: (u8, u8, u8) = (0x7A, 0x56, 0x37);

/// Abstand zweier Farben im RGB-Würfel.
fn abstand(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let d = |x: u8, y: u8| (x as f64 - y as f64).powi(2);
    (d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)).sqrt()
}

/// Drei waagerechte Linien in den Vorgabefarben, 200 mm lang, 20 mm Abstand.
fn zeichnung() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .expect("Eingabe in Millimetern");
    for (i, hex) in ["#EBCE2E", "#D9822B", "#7A5637"].iter().enumerate() {
        let y = 10.0 + i as f64 * 20.0;
        s.execute(
            "draw.line",
            &json!({
                "a": [10.0, y],
                "b": [190.0, y],
                "color": hex,
                "lineweight": 0.5,
                "layer": "Wände",
            }),
        )
        .expect("Linie");
    }
    s
}

fn optionen() -> RasterOptions {
    // 6 px je mm und Beschriftungsmaßstab 1:1: die 0,5-mm-Linie ist damit rund
    // 3 px breit und behält ihre Farbe in der Strichmitte (der Rand blendet zur
    // Papierfarbe aus). Ohne `annot_scale` rechnet der Rasterer mit dem
    // Zeichnungsmaßstab 1:100 und die Linie würde 300 px breit.
    RasterOptions {
        width_px: 1200,
        height_px: 360,
        bbox: Some([0.0, 0.0, 200.0, 60.0]),
        paper: true,
        annot_scale: Some(1.0),
        sheet: None,
    }
}

#[test]
fn gelb_orange_und_braun_sind_im_druckbild_unterscheidbar() {
    let s = zeichnung();
    let opts = optionen();
    let pm = render_model_pixmap(&s.doc, &opts).expect("Bild");

    let mut treffer = [0usize; 3];
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            let Some(px) = pm.pixel(x, y) else { continue };
            let c = (px.red(), px.green(), px.blue());
            for (i, soll) in [GELB, ORANGE, BRAUN].iter().enumerate() {
                if abstand(c, *soll) < 40.0 {
                    treffer[i] += 1;
                }
            }
        }
    }
    for (i, name) in ["Gelb #EBCE2E", "Orange #D9822B", "Braun #7A5637"]
        .iter()
        .enumerate()
    {
        assert!(
            treffer[i] > 500,
            "{name} fehlt im Druckbild: nur {} Pixel in dieser Farbe",
            treffer[i]
        );
    }

    // Die drei Töne müssen auch als Farben auseinanderliegen — sonst wären sie
    // im Bild trotz richtiger Werte nicht zu unterscheiden.
    for (a, b, name) in [
        (GELB, ORANGE, "Gelb/Orange"),
        (ORANGE, BRAUN, "Orange/Braun"),
        (GELB, BRAUN, "Gelb/Braun"),
    ] {
        assert!(
            abstand(a, b) > 60.0,
            "{name} liegen zu nah beieinander: {:.1}",
            abstand(a, b)
        );
    }

    if let Some(dir) = std::env::var_os("JJCAD_OUT_DIR") {
        let pfad = std::path::Path::new(&dir).join("farbpalette_druck.png");
        render_png(&s.doc, &opts, &pfad).expect("PNG schreiben");
        eprintln!("Prüfbild: {}", pfad.display());
    }
}
