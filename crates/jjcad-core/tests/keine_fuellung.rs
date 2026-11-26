//! Abnahme T0242: Wände und Flächen **ohne** Füllung.
//!
//! Geprüft wird über die Befehlsschicht (`modify.set`, `draw.wall`, `draw.fill`)
//! und die Displayliste — dieselbe Quelle, aus der GUI, PDF und PNG zeichnen.
//! „Keine Füllung“ heißt: keine Füllfläche, kein Muster, nur die Umrandung.
//! Das ist **nicht** dasselbe wie „keine Füllfarbe“ — ohne Farbe zeichnet eine
//! Wand den Vorgabeton und eine Vollfläche die aufgehellte Linienfarbe.

use jjcad_core::model::{Document, Entity, EntityKind};
use jjcad_core::render::{render_document, DisplayList, Prim, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::Color;
use serde_json::json;

const GRUEN: Color = Color::rgb(0, 255, 0);
const ROT: Color = Color::rgb(255, 0, 0);

fn neue_session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .expect("Einheit Millimeter");
    s
}

fn wand(session: &mut Session, params: serde_json::Value) -> u64 {
    let mut p = params;
    p["a"] = json!([0.0, 0.0]);
    p["b"] = json!([4000.0, 0.0]);
    p["thickness"] = json!(300.0);
    let r = session.execute("draw.wall", &p).expect("Wand anlegen");
    r["created"][0].as_u64().expect("neue Wand-ID")
}

fn setzen(session: &mut Session, id: u64, params: serde_json::Value) {
    let mut p = params;
    p["ids"] = json!([id]);
    session.execute("modify.set", &p).expect("modify.set");
}

fn liste(doc: &Document) -> DisplayList {
    render_document(&RenderCtx::new(doc))
}

fn fuellungen<'a>(dl: &'a DisplayList, id: u64) -> Vec<&'a Prim> {
    dl.prims
        .iter()
        .filter(|p| p.entity == id && matches!(p.shape, Shape::Fill { .. }))
        .collect()
}

fn linien<'a>(dl: &'a DisplayList, id: u64) -> Vec<&'a Prim> {
    dl.prims
        .iter()
        .filter(|p| p.entity == id && matches!(p.shape, Shape::Line(..)))
        .collect()
}

fn entity(session: &Session, id: u64) -> Entity {
    session.doc.entity(id).cloned().expect("Objekt vorhanden")
}

// ---------------------------------------------------------------------------
// Wand
// ---------------------------------------------------------------------------

#[test]
fn wand_ohne_fuellung_hat_keine_flaeche_aber_ihre_kanten() {
    let mut s = neue_session();
    let id = wand(&mut s, json!({}));
    assert!(
        !fuellungen(&liste(&s.doc), id).is_empty(),
        "Vorgabe: die Wand ist gefüllt"
    );

    setzen(&mut s, id, json!({"fill": "keine"}));

    assert!(entity(&s, id).style.no_fill, "Feld „no_fill“ ist gesetzt");
    let dl = liste(&s.doc);
    assert!(
        fuellungen(&dl, id).is_empty(),
        "keine Füllfläche ohne Füllung"
    );
    assert!(
        linien(&dl, id).len() >= 2,
        "die Umrandung bleibt gezeichnet"
    );
}

#[test]
fn wand_ohne_fuellung_zeichnet_auch_kein_muster() {
    let mut s = neue_session();
    let id = wand(&mut s, json!({"hatch": "mauerwerk"}));
    let mit_muster = linien(&liste(&s.doc), id).len();
    let id_ohne = {
        let mut p = json!({"hatch": "none"});
        p["a"] = json!([0.0, 5000.0]);
        p["b"] = json!([4000.0, 5000.0]);
        p["thickness"] = json!(300.0);
        let r = s.execute("draw.wall", &p).expect("zweite Wand");
        r["created"][0].as_u64().expect("ID")
    };
    let ohne_muster = linien(&liste(&s.doc), id_ohne).len();
    assert!(
        mit_muster > ohne_muster,
        "die Mauerwerkswand zeichnet Musterlinien ({mit_muster} gegen {ohne_muster})"
    );

    setzen(&mut s, id, json!({"fill": "keine"}));

    let dl = liste(&s.doc);
    assert!(
        fuellungen(&dl, id).is_empty(),
        "keine Fläche unter dem Muster"
    );
    assert_eq!(
        linien(&dl, id).len(),
        ohne_muster,
        "ohne Füllung bleibt nur die Umrandung, das Muster fällt weg"
    );
}

#[test]
fn fuellung_laesst_sich_wieder_einschalten() {
    let mut s = neue_session();
    let id = wand(&mut s, json!({}));
    setzen(&mut s, id, json!({"fill": "keine"}));
    assert!(fuellungen(&liste(&s.doc), id).is_empty());

    setzen(&mut s, id, json!({"fill": "standard"}));

    let e = entity(&s, id);
    assert!(!e.style.no_fill, "„standard“ nimmt das Abschalten zurück");
    assert_eq!(e.style.fill, None, "und setzt keine eigene Farbe");
    assert!(
        !fuellungen(&liste(&s.doc), id).is_empty(),
        "die Wand ist wieder gefüllt"
    );
}

#[test]
fn fuellfarbe_schaltet_die_flaeche_wieder_ein() {
    let mut s = neue_session();
    let id = wand(&mut s, json!({}));
    setzen(&mut s, id, json!({"fill": "keine"}));
    setzen(&mut s, id, json!({"fill": GRUEN.to_hex()}));

    let e = entity(&s, id);
    assert!(!e.style.no_fill, "eine Farbe ist eine Füllung");
    let dl = liste(&s.doc);
    assert!(fuellungen(&dl, id)
        .iter()
        .all(|p| p.style.fill == Some(GRUEN)));
}

// ---------------------------------------------------------------------------
// Füllfläche
// ---------------------------------------------------------------------------

/// Eine Fläche 1000 × 1000 mm an (0,0).
fn flaeche(session: &mut Session, params: serde_json::Value) -> u64 {
    let mut p = params;
    p["boundary"] = json!([[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0], [0.0, 1000.0]]);
    let r = session.execute("draw.fill", &p).expect("Fläche anlegen");
    r["created"][0].as_u64().expect("neue Flächen-ID")
}

#[test]
fn flaeche_ohne_fuellung_zeigt_nur_die_umrandung() {
    let mut s = neue_session();
    let id = flaeche(&mut s, json!({"pattern": "mauerwerk", "border": true}));
    assert!(
        !fuellungen(&liste(&s.doc), id).is_empty()
            || dl_prims(&s.doc, id).len() > 1,
        "Vorgabe: die Fläche zeichnet etwas"
    );

    setzen(&mut s, id, json!({"fill": "keine"}));

    let dl = liste(&s.doc);
    assert!(fuellungen(&dl, id).is_empty(), "keine Fläche");
    let prims = dl_prims_aus(&dl, id);
    assert_eq!(prims.len(), 1, "nur die Umrandung bleibt: {prims:?}");
    assert!(
        matches!(prims[0].shape, Shape::Poly(_, true)),
        "die Umrandung ist der geschlossene Umriss"
    );
}

#[test]
fn flaeche_ohne_fuellung_und_ohne_umrandung_ist_leer() {
    let mut s = neue_session();
    let id = flaeche(&mut s, json!({"pattern": "mauerwerk", "border": true}));
    setzen(&mut s, id, json!({"fill": "keine", "border": false}));
    assert!(
        dl_prims(&s.doc, id).is_empty(),
        "ohne Füllung und Umrandung zeichnet die Fläche nichts"
    );
}

fn dl_prims(doc: &Document, id: u64) -> Vec<Prim> {
    let dl = liste(doc);
    dl_prims_aus(&dl, id)
}

fn dl_prims_aus(dl: &DisplayList, id: u64) -> Vec<Prim> {
    dl.prims
        .iter()
        .filter(|p| p.entity == id)
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Datei
// ---------------------------------------------------------------------------

#[test]
fn keine_fuellung_ueberlebt_speichern_und_laden() {
    let mut s = neue_session();
    let id = wand(&mut s, json!({}));
    setzen(&mut s, id, json!({"fill": "keine"}));

    let text = jjcad_core::persist::to_json(&s.doc).expect("serialisieren");
    assert!(
        text.contains("\"no_fill\":true") || text.contains("\"no_fill\": true"),
        "das Feld steht in der Datei: {text}"
    );

    let pfad = std::env::temp_dir().join(format!("jjcad-keine-fuellung-{}.jjcad", std::process::id()));
    jjcad_core::persist::save_document(&mut s.doc, &pfad).expect("speichern");
    let geladen = jjcad_core::persist::load_document(&pfad).expect("laden");
    let _ = std::fs::remove_file(&pfad);

    let e = geladen.entity(id).expect("Wand wieder da");
    assert!(e.style.no_fill, "„keine Füllung“ bleibt erhalten");
    assert!(
        fuellungen(&liste(&geladen), id).is_empty(),
        "und wird auch nach dem Laden nicht gezeichnet"
    );
}

#[test]
fn altdatei_ohne_feld_bleibt_gefuellt() {
    // Stand vor T0242: kein `no_fill`-Schlüssel. Die Wand muss gefüllt bleiben.
    let mut s = neue_session();
    let id = wand(&mut s, json!({}));
    let neu = jjcad_core::persist::to_json(&s.doc).expect("serialisieren");
    let mut wert: serde_json::Value = serde_json::from_str(&neu).unwrap();
    for e in wert["entities"].as_array_mut().unwrap() {
        if let Some(st) = e.get_mut("style").and_then(|v| v.as_object_mut()) {
            st.remove("no_fill");
        }
    }
    let alt = serde_json::to_string(&wert).unwrap();
    let doc = jjcad_core::persist::from_json(&alt).expect("Altdatei lädt");

    let e = doc.entity(id).expect("Wand vorhanden");
    assert!(!e.style.no_fill, "Vorgabe ist gefüllt");
    assert!(
        !fuellungen(&liste(&doc), id).is_empty(),
        "und wird gefüllt gezeichnet"
    );
}

// ---------------------------------------------------------------------------
// PNG: der Ton verschwindet wirklich
// ---------------------------------------------------------------------------

#[test]
fn png_zeigt_die_wand_ohne_fuellung_frei() {
    let mut s = neue_session();
    let id = wand(&mut s, json!({}));
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
    let gruen = |s: &Session| {
        let pm = jjcad_core::raster::render_model_pixmap(&s.doc, &opts).expect("Raster");
        pm.pixels()
            .iter()
            .filter(|px| (px.red(), px.green(), px.blue()) == (0, 255, 0))
            .count()
    };
    let vorher = gruen(&s);
    assert!(vorher > 400, "Wandfläche in Füllfarbe, gefunden: {vorher}");

    setzen(&mut s, id, json!({"fill": "keine"}));
    assert_eq!(gruen(&s), 0, "ohne Füllung bleibt keine Fläche in der Füllfarbe");
}

/// Das gespeicherte Feld ist die eine Wahrheit — nicht nur die Anzeige.
#[test]
fn gespeichertes_feld_ist_das_abschalten() {
    let mut s = neue_session();
    let f = flaeche(&mut s, json!({}));
    setzen(&mut s, f, json!({"fill": "keine"}));
    match &entity(&s, f).kind {
        EntityKind::Fill { .. } => {}
        k => panic!("Fläche erwartet, gefunden {k:?}"),
    }
    assert!(entity(&s, f).style.no_fill);
}
