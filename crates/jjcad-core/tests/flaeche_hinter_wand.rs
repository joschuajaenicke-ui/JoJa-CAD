//! Füllfläche hinter der Wand (T0050).
//!
//! Die Objektliste ist die Zeichenreihenfolge: eine Füllfläche (Rang 0) ordnet
//! sich beim Einfügen **hinter** eine Wand (Rang 2), „Reihenfolge" im
//! Eigenschaften-Panel zeigt also 1 für die Fläche und 2 für die Wand. Im
//! Schnittbereich muss die Wandfläche die Fläche zudecken — vor dem Fix stand
//! die Fläche in der Displayliste nach den Wandfüllungen und deckte sie zu.
//!
//! Geprüft wird die ganze Kette: Objektliste → Displayliste → sichtbares Pixel.

use jjcad_core::export::RasterOptions;
use jjcad_core::model::Id;
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use serde_json::json;

fn leinwand() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .expect("Eingabe in Metern");
    s
}

fn id_aus(antwort: &serde_json::Value, was: &str) -> Id {
    antwort["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{was}: keine Id in {antwort}")) as Id
}

fn position(s: &Session, id: Id) -> usize {
    s.doc
        .entities
        .iter()
        .position(|e| e.id == id)
        .unwrap_or_else(|| panic!("Objekt {id} fehlt im Dokument"))
}

/// Wand waagerecht durch den Ursprung, darüber eine rote Füllfläche, die die
/// Wand kreuzt. Die Wand wird zuerst gezeichnet, die Fläche ordnet sich vor sie.
fn szene() -> (Session, Id, Id) {
    let mut s = leinwand();
    let wand = s
        .execute(
            "draw.wall",
            &json!({
                "a": [0.0, 0.0], "b": [6.0, 0.0], "thickness": 0.2,
                "fill": "#cccccc"
            }),
        )
        .expect("Wand");
    let wand = id_aus(&wand, "Wand");
    let flaeche = s
        .execute(
            "draw.fill.points",
            &json!({
                "boundary": [[2.0, -1.0], [4.0, -1.0], [4.0, 1.0], [2.0, 1.0]],
                "pattern": "solid",
                "fill": "#ff0000"
            }),
        )
        .expect("Füllfläche");
    let flaeche = id_aus(&flaeche, "Füllfläche");
    (s, wand, flaeche)
}

/// Stelle des ersten Füll-Primitivs eines Objekts in der Displayliste.
fn fill_stelle(dl: &jjcad_core::render::DisplayList, id: Id) -> Option<usize> {
    dl.prims
        .iter()
        .position(|p| p.entity == id && matches!(p.shape, Shape::Fill { .. }))
}

#[test]
fn flaeche_steht_hinter_der_wand_in_der_objektliste() {
    let (s, wand, flaeche) = szene();
    assert_eq!(position(&s, flaeche), 0, "Fläche gehört ganz nach hinten");
    assert_eq!(position(&s, wand), 1, "Wand liegt über der Fläche");
}

#[test]
fn wandfuellung_liegt_ueber_der_fuellflaeche() {
    let (s, wand, flaeche) = szene();
    let dl = render_document(&RenderCtx::new(&s.doc));

    let i_flaeche = fill_stelle(&dl, flaeche).expect("Fläche fehlt in der Displayliste");
    let i_wand = fill_stelle(&dl, wand).expect("Wandfüllung fehlt in der Displayliste");
    assert!(
        i_flaeche < i_wand,
        "Wandfüllung bei {i_wand} wird vor der Fläche bei {i_flaeche} gezeichnet"
    );

    // Die Wandkanten bleiben über ihrer eigenen Füllung.
    for (i, p) in dl.prims.iter().enumerate() {
        if p.entity == wand && matches!(p.shape, Shape::Line(..)) {
            assert!(
                i > i_wand,
                "Wandkante bei {i} wird vor der Wandfüllung bei {i_wand} gezeichnet"
            );
        }
    }
}

#[test]
fn umgekehrte_reihenfolge_deckt_die_wand_zu() {
    // Gegenprobe zur Zeichenreihenfolge: von Hand nach vorn geholt, liegt die
    // Fläche über der Wandfüllung — der Renderer sortiert nicht nach Rang,
    // sondern folgt der Objektliste.
    let (mut s, wand, flaeche) = szene();
    s.execute("modify.order", &json!({ "ids": [flaeche], "to": 2 }))
        .expect("Reihenfolge");
    assert_eq!(position(&s, flaeche), 1);
    assert_eq!(position(&s, wand), 0);

    let dl = render_document(&RenderCtx::new(&s.doc));
    let i_flaeche = fill_stelle(&dl, flaeche).expect("Fläche fehlt");
    let i_wand = fill_stelle(&dl, wand).expect("Wandfüllung fehlt");
    assert!(
        i_wand < i_flaeche,
        "vorgeholte Fläche bei {i_flaeche} wird vor der Wandfüllung bei {i_wand} gezeichnet"
    );
}

/// Pixel im Bild für eine Weltstelle — dieselbe Abbildung wie
/// `render_model_pixmap` (3 % Rand, gleichmäßiger Maßstab, mittig).
fn abbildung(bbox: [f64; 4], w: f64, h: f64) -> impl Fn(f64, f64) -> (u32, u32) {
    let grow = (bbox[2] - bbox[0]).max(bbox[3] - bbox[1]) * 0.03;
    let (x0, y0, x1, y1) = (bbox[0] - grow, bbox[1] - grow, bbox[2] + grow, bbox[3] + grow);
    let k = (w / (x1 - x0)).min(h / (y1 - y0));
    let ox = -x0 * k + (w - (x1 - x0) * k) * 0.5;
    let oy = y1 * k + (h - (y1 - y0) * k) * 0.5;
    move |x, y| ((ox + x * k) as u32, (oy - y * k) as u32)
}

#[test]
fn leinwand_zeigt_die_wand_ueber_der_roten_flaeche() {
    let (s, _wand, _flaeche) = szene();
    let bbox = [-200.0, -1300.0, 6200.0, 1300.0];
    let opts = RasterOptions {
        width_px: 660,
        height_px: 510,
        bbox: Some(bbox),
        paper: true,
        annot_scale: None,
        sheet: None,
    };
    let pm = render_model_pixmap(&s.doc, &opts).expect("Bild");
    let ort = abbildung(bbox, pm.width() as f64, pm.height() as f64);
    let farbe = |x: f64, y: f64| {
        let (px, py) = ort(x, y);
        let p = pm.pixel(px, py).expect("Pixel im Bild");
        (p.red(), p.green(), p.blue())
    };

    // Im Schnittbereich (Wandkörper, y = 0) steht die Wandfarbe — nicht rot.
    let (r, g, b) = farbe(3000.0, 0.0);
    assert!(
        r == g && g == b && r > 150,
        "Schnittbereich zeigt ({r}, {g}, {b}) statt der grauen Wandfläche"
    );

    // Außerhalb der Wand bleibt die Fläche rot, die Wand außerhalb der Fläche grau.
    let (r, g, b) = farbe(3000.0, 600.0);
    assert!(
        r > 200 && g < 60 && b < 60,
        "Fläche nicht rot: ({r}, {g}, {b})"
    );
    let (r, g, b) = farbe(1000.0, 0.0);
    assert!(
        r == g && g == b && r > 150,
        "Wandfläche außerhalb der Fläche: ({r}, {g}, {b})"
    );
}
