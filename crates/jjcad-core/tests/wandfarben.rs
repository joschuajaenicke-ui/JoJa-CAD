//! Abnahme: Wandfarbe für Umrandung (Linie) und Fläche (Füllung) getrennt.
//!
//! Geprüft wird über die öffentliche Befehlsschicht und die Displayliste —
//! dieselbe Quelle, aus der GUI, PDF und PNG zeichnen.

use jjcad_core::model::{Document, EntityKind};
use jjcad_core::render::{render_document, DisplayList, Prim, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::Color;
use serde_json::json;

const ROT: Color = Color::rgb(255, 0, 0);
const GRUEN: Color = Color::rgb(0, 255, 0);
const BLAU: Color = Color::rgb(0, 0, 255);

/// Neue Sitzung in Millimetern — die Testkoordinaten sind Wandmaße in mm.
fn neue_session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .expect("Einheit Millimeter");
    s
}

fn wand(session: &mut Session) -> u64 {
    let r = session
        .execute(
            "draw.wall",
            &json!({"a": [0.0, 0.0], "b": [4000.0, 0.0], "thickness": 300.0}),
        )
        .expect("Wand anlegen");
    r["created"][0].as_u64().expect("neue Wand-ID") as u64
}

fn setzen(session: &mut Session, id: u64, params: serde_json::Value) {
    let mut p = params;
    p["ids"] = json!([id]);
    session.execute("modify.set", &p).expect("modify.set");
}

fn liste(doc: &Document) -> DisplayList {
    render_document(&RenderCtx::new(doc))
}

/// Alle Füllprimitive der Wand.
fn fuellungen(dl: &DisplayList, id: u64) -> Vec<&Prim> {
    dl.prims
        .iter()
        .filter(|p| p.entity == id && matches!(p.shape, Shape::Fill(_)))
        .collect()
}

/// Alle Linienprimitive der Wand.
fn linien(dl: &DisplayList, id: u64) -> Vec<&Prim> {
    dl.prims
        .iter()
        .filter(|p| p.entity == id && matches!(p.shape, Shape::Line(..)))
        .collect()
}

#[test]
fn linie_und_fuellung_sind_getrennte_farben() {
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(
        &mut s,
        id,
        json!({"color": ROT.to_hex(), "fill": GRUEN.to_hex()}),
    );

    let dl = liste(&s.doc);
    let f = fuellungen(&dl, id);
    let l = linien(&dl, id);
    assert!(!f.is_empty(), "Wand muss eine Füllfläche haben");
    assert!(l.len() >= 2, "Wand muss Längskanten haben");
    for p in f {
        assert_eq!(p.style.fill, Some(GRUEN), "Fläche trägt die Füllfarbe");
    }
    for p in l {
        assert_eq!(p.style.color, ROT, "Umrandung trägt die Linienfarbe");
    }
}

#[test]
fn linienfarbe_aendert_die_fuellung_nicht() {
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(
        &mut s,
        id,
        json!({"color": ROT.to_hex(), "fill": GRUEN.to_hex()}),
    );
    // nur die Linienfarbe umstellen
    setzen(&mut s, id, json!({"color": BLAU.to_hex()}));

    let dl = liste(&s.doc);
    assert!(fuellungen(&dl, id)
        .iter()
        .all(|p| p.style.fill == Some(GRUEN)));
    assert!(linien(&dl, id).iter().all(|p| p.style.color == BLAU));
}

#[test]
fn fuellfarbe_aendert_die_linie_nicht() {
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(
        &mut s,
        id,
        json!({"color": ROT.to_hex(), "fill": GRUEN.to_hex()}),
    );
    // nur die Füllfarbe umstellen
    setzen(&mut s, id, json!({"fill": BLAU.to_hex()}));

    let dl = liste(&s.doc);
    assert!(fuellungen(&dl, id)
        .iter()
        .all(|p| p.style.fill == Some(BLAU)));
    assert!(linien(&dl, id).iter().all(|p| p.style.color == ROT));
}

#[test]
fn ohne_eigene_fuellung_bleibt_der_bisherige_standard() {
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(&mut s, id, json!({"color": ROT.to_hex()}));

    let dl = liste(&s.doc);
    // Bisheriges Aussehen: Fläche = Linienfarbe, 74 % Richtung Weiß aufgehellt
    let erwartet = ROT.mix(Color::WHITE, 0.74);
    assert!(fuellungen(&dl, id)
        .iter()
        .all(|p| p.style.fill == Some(erwartet)));
    assert!(linien(&dl, id).iter().all(|p| p.style.color == ROT));
}

#[test]
fn fuellung_auf_standard_zuruecknehmen() {
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(&mut s, id, json!({"fill": GRUEN.to_hex()}));
    assert_eq!(
        s.doc.entity(id).unwrap().style.fill,
        Some(GRUEN),
        "Füllung ist gesetzt"
    );
    setzen(&mut s, id, json!({"fill": "none"}));
    assert_eq!(
        s.doc.entity(id).unwrap().style.fill,
        None,
        "\"none\" hebt die Füllung wieder auf"
    );
}

#[test]
fn speichern_und_laden_erhaelt_linie_und_fuellung() {
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(
        &mut s,
        id,
        json!({"color": ROT.to_hex(), "fill": GRUEN.to_hex()}),
    );

    let pfad = std::env::temp_dir().join(format!("jjcad-wandfarben-{}.jjcad", std::process::id()));
    jjcad_core::persist::save_document(&mut s.doc, &pfad).expect("speichern");
    let geladen = jjcad_core::persist::load_document(&pfad).expect("laden");
    let _ = std::fs::remove_file(&pfad);

    let e = geladen.entity(id).expect("Wand ist wieder da");
    assert!(matches!(e.kind, EntityKind::Wall { .. }));
    assert_eq!(e.style.color, Some(ROT), "Linienfarbe bleibt erhalten");
    assert_eq!(e.style.fill, Some(GRUEN), "Füllfarbe bleibt erhalten");
}

#[test]
fn altdatei_ohne_fuellungsschluessel_laedt() {
    // Stand vor der getrennten Füllfarbe: style ohne "fill"-Schlüssel.
    // Dafür eine echte Datei schreiben und den Schlüssel entfernen — so bleibt
    // der Rest des Formats garantiert gültig.
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(&mut s, id, json!({"color": ROT.to_hex()}));
    let json_neu = jjcad_core::persist::to_json(&s.doc).expect("serialisieren");
    let mut wert: serde_json::Value = serde_json::from_str(&json_neu).unwrap();
    for e in wert["entities"].as_array_mut().unwrap() {
        if let Some(st) = e.get_mut("style").and_then(|v| v.as_object_mut()) {
            st.remove("fill");
        }
    }
    let json_alt = serde_json::to_string(&wert).unwrap();
    for e in wert["entities"].as_array().unwrap() {
        assert!(
            e["style"].get("fill").is_none(),
            "Altdatei ohne fill-Schlüssel"
        );
    }
    let doc = jjcad_core::persist::from_json(&json_alt).expect("Altdatei muss laden");
    let e = doc.entity(id).expect("Wand vorhanden");
    assert_eq!(e.style.color, Some(ROT), "alte Linienfarbe bleibt erhalten");
    assert_eq!(e.style.fill, None, "fehlender Schlüssel ergibt Standard");

    // und der Standard ist sichtbar (hell aus der Linienfarbe)
    let dl = liste(&doc);
    let erwartet = ROT.mix(Color::WHITE, 0.74);
    assert!(fuellungen(&dl, id)
        .iter()
        .all(|p| p.style.fill == Some(erwartet)));
}

#[test]
fn png_zeigt_flaeche_in_fuellfarbe_und_rand_in_linienfarbe() {
    let mut s = neue_session();
    let id = wand(&mut s);
    setzen(
        &mut s,
        id,
        json!({"color": ROT.to_hex(), "fill": GRUEN.to_hex()}),
    );

    let opts = jjcad_core::export::RasterOptions {
        width_px: 800,
        height_px: 600,
        ..Default::default()
    };
    let pm = jjcad_core::raster::render_model_pixmap(&s.doc, &opts).expect("Raster");
    let (mut gruen, mut rot) = (0usize, 0usize);
    for px in pm.pixels() {
        let (r, g, b) = (px.red(), px.green(), px.blue());
        if (r, g, b) == (0, 255, 0) {
            gruen += 1;
        } else if r > 200 && g < 110 && b < 110 {
            // Umrandung: 0,35 mm Strich ist dünn, die Randpixel sind
            // antialiased — deshalb rot-dominant statt exakt Rot zählen.
            rot += 1;
        }
    }
    assert!(gruen > 400, "Wandfläche in Füllfarbe, gefunden: {gruen} px");
    assert!(rot > 40, "Umrandung in Linienfarbe, gefunden: {rot} px");
    let _ = id;
}
