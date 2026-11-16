//! Überdeckung zwischen Objekten (T0192).
//!
//! Die Objektliste ist die Zeichenreihenfolge. Gezeichnet wird **je Objekt als
//! Einheit**: erst seine Füllflächen, dann seine Linien. Daraus folgt:
//!
//! 1. Die Umrandung einer Fläche liegt immer über ihrer **eigenen** Füllung.
//! 2. Eine später gezeichnete Fläche deckt die Umrandung einer früher
//!    gezeichneten Fläche mit zu — vorher standen global alle Füllungen vor
//!    allen Linien, die untere Umrandung blieb über der oberen Fläche sichtbar.
//! 3. Der Zeichenrang beim Einfügen bleibt: eine neue Fläche (Rang 0) landet
//!    hinter allen Linien und deckt sie deshalb nicht zu. Erst „Reihenfolge“
//!    (`modify.order`) holt sie bewusst nach vorn — dann deckt sie auch Linien.
//!
//! Geprüft wird die ganze Kette: Action → Objektliste → Displayliste → Pixel.

use jjcad_core::export::RasterOptions;
use jjcad_core::model::Id;
use jjcad_core::raster::render_model_pixmap;
use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use serde_json::json;

// ---------------------------------------------------------------------------
// Hilfen
// ---------------------------------------------------------------------------

fn leinwand() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .expect("Eingabe in Metern");
    // Kräftige Strichstärke, damit das Pixelmaß trägt: bei 0,47 px/mm ist eine
    // 2-mm-Linie nur einen Pixel breit und wird weichgezeichnet (Vorgabe 0,18).
    if let Some(l) = s.doc.layers.iter_mut().find(|l| l.name == "Zeichnung") {
        l.line_weight = 6.0;
    }
    s
}

fn id_aus(antwort: &serde_json::Value) -> Id {
    antwort["created"][0].as_u64().expect("Id") as Id
}

/// Fläche als Punktfolge in Metern, mit Umrandung.
fn flaeche(s: &mut Session, ecken: &[(f64, f64)], farbe: &str) -> Id {
    let pts: Vec<[f64; 2]> = ecken.iter().map(|(x, y)| [*x, *y]).collect();
    let r = s
        .execute(
            "draw.fill.points",
            &json!({
                "pts": pts,
                "pattern": "solid",
                "fill": farbe,
                "border": true,
                "layer": "Zeichnung",
            }),
        )
        .expect("Fläche");
    id_aus(&r)
}

fn linie(s: &mut Session, a: (f64, f64), b: (f64, f64)) -> Id {
    let r = s
        .execute(
            "draw.line",
            &json!({"a": [a.0, a.1], "b": [b.0, b.1], "layer": "Zeichnung"}),
        )
        .expect("Linie");
    id_aus(&r)
}

/// Erste und letzte Stelle der Primitive eines Objekts in der Displayliste.
fn spanne(dl: &jjcad_core::render::DisplayList, id: Id) -> (usize, usize) {
    let stellen: Vec<usize> = dl
        .prims
        .iter()
        .enumerate()
        .filter(|(_, p)| p.entity == id)
        .map(|(i, _)| i)
        .collect();
    assert!(!stellen.is_empty(), "Objekt {id} fehlt in der Displayliste");
    (stellen[0], *stellen.last().unwrap())
}

fn ist_fuellung(p: &jjcad_core::render::Prim) -> bool {
    matches!(p.shape, Shape::Fill { .. })
}

/// Zählt dunkle Pixel in einem Welt-Rechteck (Umrandung) und rote Pixel.
fn zaehle(s: &Session, welt: [f64; 4]) -> (usize, usize) {
    // `render_model_pixmap` erweitert die Bounding-Box um 3 % der längeren
    // Seite — dieselbe Rechnung hier, sonst liegt das Prüffenster daneben.
    let (x0, y0, x1, y1) = (-100.0f64, -1100.0f64, 4100.0f64, 4100.0f64);
    let rand = (x1 - x0).max(y1 - y0) * 0.03;
    let (bx0, by0, bx1, by1) = (x0 - rand, y0 - rand, x1 + rand, y1 + rand);
    let (w, h) = (2100.0, 2600.0);
    let opts = RasterOptions {
        width_px: w as u32,
        height_px: h as u32,
        bbox: Some([x0, y0, x1, y1]),
        paper: true,
        annot_scale: Some(1.0),
        sheet: None,
    };
    let pm = render_model_pixmap(&s.doc, &opts).expect("Bild");
    // Gleichmäßiger Maßstab, mittig — dieselbe Abbildung wie im Rasterer.
    let k = (w / (bx1 - bx0)).min(h / (by1 - by0));
    let ox = -bx0 * k + (w - (bx1 - bx0) * k) * 0.5;
    let oy = by1 * k + (h - (by1 - by0) * k) * 0.5;
    let px = |x: f64| (ox + x * k) as i64;
    let py = |y: f64| (oy - y * k) as i64;
    let (mut dunkel, mut rot) = (0usize, 0usize);
    for y in py(welt[3])..=py(welt[1]) {
        for x in px(welt[0])..=px(welt[2]) {
            if x < 0 || y < 0 || x >= pm.width() as i64 || y >= pm.height() as i64 {
                continue;
            }
            let Some(p) = pm.pixel(x as u32, y as u32) else {
                continue;
            };
            let (r, g, b) = (p.red(), p.green(), p.blue());
            if r < 80 && g < 80 && b < 80 {
                dunkel += 1;
            }
            if r > 200 && g < 60 && b < 60 {
                rot += 1;
            }
        }
    }
    (dunkel, rot)
}

// ---------------------------------------------------------------------------
// 1. Zwei Flächen: die obere deckt die Umrandung der unteren zu
// ---------------------------------------------------------------------------

/// Blau 0…4000 × 0…3000, darüber rot 2000…3000 × −1000…4000. Die obere Kante
/// der blauen Fläche (y = 3000) liegt im Bereich der roten.
fn zwei_flaechen() -> (Session, Id, Id) {
    let mut s = leinwand();
    let blau = flaeche(
        &mut s,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)],
        "#0000ff",
    );
    let rot = flaeche(
        &mut s,
        &[(2.0, -1.0), (3.0, -1.0), (3.0, 4.0), (2.0, 4.0)],
        "#ff0000",
    );
    (s, blau, rot)
}

#[test]
fn obere_flaeche_steht_in_der_displayliste_ueber_der_unteren() {
    let (s, blau, rot) = zwei_flaechen();
    let dl = render_document(&RenderCtx::new(&s.doc));

    let (b0, b1) = spanne(&dl, blau);
    let (r0, r1) = spanne(&dl, rot);
    assert!(
        b1 < r0,
        "die rote Fläche ({r0}…{r1}) muss nach der blauen ({b0}…{b1}) gezeichnet werden"
    );

    // Die eigene Umrandung bleibt über der eigenen Füllung.
    let mut letzte_fuellung = None;
    for (i, p) in dl.prims.iter().enumerate() {
        if p.entity == rot {
            if ist_fuellung(p) {
                letzte_fuellung = Some(i);
            } else if let Some(f) = letzte_fuellung {
                assert!(i > f, "Umrandung der roten Fläche liegt unter ihrer Füllung");
            }
        }
    }
}

#[test]
fn obere_flaeche_deckt_die_umrandung_der_unteren_zu() {
    let (s, _blau, _rot) = zwei_flaechen();

    // Innerhalb der roten Fläche: die blaue Kante ist zugedeckt.
    let (dunkel_innen, rot_innen) = zaehle(&s, [2100.0, 2900.0, 2900.0, 3100.0]);
    // Außerhalb: dieselbe Kante ist sichtbar.
    let (dunkel_aussen, _) = zaehle(&s, [500.0, 2900.0, 1500.0, 3100.0]);

    assert!(
        rot_innen > 100,
        "die rote Fläche fehlt im Prüffenster: {rot_innen} rote Pixel"
    );
    assert!(
        dunkel_aussen > 20,
        "die blaue Umrandung ist außerhalb der roten Fläche nicht sichtbar: {dunkel_aussen} dunkle Pixel"
    );
    assert!(
        dunkel_innen < 5,
        "die blaue Umrandung steht über der roten Fläche: {dunkel_innen} dunkle Pixel"
    );
}

// ---------------------------------------------------------------------------
// 2. Neue Fläche deckt keine Linie zu (Zeichenrang bleibt)
// ---------------------------------------------------------------------------

fn linie_und_flaeche() -> (Session, Id, Id) {
    let mut s = leinwand();
    let l = linie(&mut s, (0.0, 0.0), (4.0, 0.0));
    let f = flaeche(
        &mut s,
        &[(1.0, -1.0), (3.0, -1.0), (3.0, 1.0), (1.0, 1.0)],
        "#ff0000",
    );
    (s, l, f)
}

#[test]
fn neue_flaeche_landet_hinter_der_linie() {
    let (s, l, f) = linie_und_flaeche();
    let dl = render_document(&RenderCtx::new(&s.doc));
    let (f0, f1) = spanne(&dl, f);
    let (l0, l1) = spanne(&dl, l);
    assert!(
        f1 < l0,
        "die Fläche ({f0}…{f1}) muss vor der Linie ({l0}…{l1}) gezeichnet werden"
    );

    // Und im Bild bleibt die Linie sichtbar.
    let (dunkel, rot) = zaehle(&s, [1200.0, -100.0, 2800.0, 100.0]);
    assert!(rot > 100, "die Fläche ist nicht gefüllt: {rot} rote Pixel");
    assert!(
        dunkel > 20,
        "die Linie ist unter der neuen Fläche verschwunden: {dunkel} dunkle Pixel"
    );
}

#[test]
fn bewusst_nach_vorn_geholte_flaeche_deckt_die_linie_zu() {
    let (mut s, l, f) = linie_und_flaeche();
    let anzahl = s.doc.entities.len();
    s.execute("modify.order", &json!({ "ids": [f], "to": anzahl }))
        .expect("Reihenfolge");

    let dl = render_document(&RenderCtx::new(&s.doc));
    let (f0, _f1) = spanne(&dl, f);
    let (_l0, l1) = spanne(&dl, l);
    assert!(
        l1 < f0,
        "die vorgeholte Fläche ({f0}…) muss nach der Linie (…{l1}) gezeichnet werden"
    );

    let (dunkel, rot) = zaehle(&s, [1200.0, -100.0, 2800.0, 100.0]);
    assert!(rot > 100, "die Fläche ist nicht gefüllt: {rot} rote Pixel");
    assert!(
        dunkel < 5,
        "die Linie steht noch über der vorgeholten Fläche: {dunkel} dunkle Pixel"
    );
}

