//! Erzeugt die Beispieldatei „Ellipse-Stutzen" (T0084).
//!   cargo run -p jjcad-core --example demo_ellipse_stutzen
//!
//! Links liegt eine ungestutzte Ellipse mit zwei senkrechten Schnittkanten —
//! ein Klick mit „Stutzen" auf den oberen Abschnitt teilt sie in drei
//! Ellipsenbögen. Rechts ist dieselbe Übung an einer **gedrehten** Ellipse
//! bereits ausgeführt: die untere Hälfte fällt weg und ein Ellipsenbogen bleibt
//! stehen (die Schnittkante ist als dünne gestrichelte Linie zu sehen).

use jjcad_core::geom::Ellipse;
use jjcad_core::model::Document;
use jjcad_core::persist;
use serde_json::json;
use std::path::Path;

fn linie(doc: &mut Document, a: [f64; 2], b: [f64; 2]) -> Result<(), String> {
    jjcad_core::actions::execute(
        doc,
        "draw.line",
        &json!({
            "a": a,
            "b": b,
            "color": "#9AA3AD",
            "lineweight": 0.25,
            "linetype": "Dashed"
        }),
    )?;
    Ok(())
}

fn main() -> Result<(), String> {
    let mut doc = Document::blank();

    // ---- Links: Ellipse mit zwei Schnittkanten, noch ungestutzt ----
    jjcad_core::actions::execute(
        &mut doc,
        "draw.ellipse",
        &json!({
            "c": [4.0, 5.0],
            "rx": 3.0,
            "ry": 1.5,
            "color": "#23262C",
            "lineweight": 0.5,
            "layer": "Wände"
        }),
    )?;
    for x in [3.0, 5.0] {
        linie(&mut doc, [x, 3.0], [x, 7.0])?;
    }

    // ---- Rechts: gedrehte Ellipse, an einer waagerechten Kante gestutzt ----
    jjcad_core::actions::execute(
        &mut doc,
        "draw.ellipse",
        &json!({
            "c": [12.0, 5.0],
            "rx": 2.5,
            "ry": 1.2,
            "rot": 30.0,
            "color": "#23262C",
            "lineweight": 0.5,
            "layer": "Wände"
        }),
    )?;
    linie(&mut doc, [8.0, 5.0], [16.0, 5.0])?;
    // Die Ellipse trägt beim Stutzen ihren Parameterwinkel — der Klickpunkt
    // wird deshalb aus derselben Umrechnung geholt wie die Anzeige.
    let el = Ellipse::new(
        jjcad_core::geom::v(12000.0, 5000.0),
        2500.0,
        1200.0,
        30f64.to_radians(),
    );
    let klick = el.point_at(-std::f64::consts::FRAC_PI_4);
    let id = doc
        .entities
        .iter()
        .rev()
        .find(|e| matches!(e.kind, jjcad_core::model::EntityKind::Ellipse { .. }))
        .map(|e| e.id)
        .ok_or("Ellipse fehlt")?;
    jjcad_core::actions::execute(
        &mut doc,
        "modify.trim",
        &json!({
            "ids": [id],
            "at": [klick.x, klick.y],
            "units": "mm"
        }),
    )?;

    let path = Path::new("samples/Ellipse-Stutzen.jjcad");
    persist::save_document(&mut doc, path)?;
    println!("geschrieben: {}", path.display());
    Ok(())
}