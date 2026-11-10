//! Dämmung als Linienart (T0120).
//!
//! Die Linienart „Dämmung“ ist kein Strichmuster: die Anzeige übersetzt sie in
//! ein **Schlaufenband** (Rechnung in `jjcad_core::daemmung`, Dokumentation in
//! `docs/DAEMMUNG-SCHLAUFEN.md`). Geprüft wird über die Befehls-Schicht und die
//! Displayliste — derselbe Weg, den GUI, PDF, PNG und DXF nehmen.
//!
//! Die Kernaussage des Tickets ist die **exakte Länge**: das Band beginnt bei
//! x = 0 und endet bei x = L, unabhängig von Breite (H) und Schlaufenform (r).

use jjcad_core::actions;
use jjcad_core::model::Document;
use jjcad_core::render::{self, DisplayList, RenderCtx, Shape};
use jjcad_core::style::LineType;
use serde_json::{json, Value};

/// Führt eine Aktion aus und liefert die Id des erzeugten Objekts.
fn neu(doc: &mut Document, action: &str, params: Value) -> u64 {
    let r = actions::execute(doc, action, &params)
        .unwrap_or_else(|e| panic!("{action} fehlgeschlagen: {e}"));
    r["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{action} hat kein Objekt erzeugt"))
}

/// Alle Primitive eines Objekts in der Anzeige.
fn prims(doc: &Document, id: u64) -> Vec<render::Prim> {
    let e = doc.entity(id).expect("Objekt fehlt");
    let ctx = RenderCtx::for_entity(doc, e);
    let mut dl = DisplayList::default();
    render::render_entity(&ctx, e, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
    dl.prims
}

/// Eine waagerechte Linie von 0 bis 5000 mm mit Dämmung.
fn daemmungslinie(doc: &mut Document, breite: Option<f64>, form: Option<f64>) -> u64 {
    let mut p = json!({
        "a": [0.0, 0.0],
        "b": [5.0, 0.0],
        "linetype": "insulation",
        "layer": "Hilfslinien"
    });
    if let Some(b) = breite {
        // Längen kommen in der Eingabeeinheit der Zeichnung (Meter).
        p["loop_width"] = json!(b / 1000.0);
    }
    if let Some(f) = form {
        p["loop_shape"] = json!(f);
    }
    neu(doc, "draw.line", p)
}

/// Die Linienart erzeugt ein Schlaufenband statt einer Linie — Anfang und Ende
/// liegen auf der Länge der Linie, die Höhe ist H.
#[test]
fn linie_mit_daemmung_wird_zum_schlaufenband() {
    let mut doc = Document::new();
    let id = daemmungslinie(&mut doc, Some(120.0), Some(0.5));
    let prims = prims(&doc, id);
    assert_eq!(prims.len(), 1, "ein Prim je Linie");
    let Shape::Poly(pts, closed) = &prims[0].shape else {
        panic!("Dämmung muss als Polygonzug gezeichnet werden, war {:?}", prims[0].shape);
    };
    assert!(!closed, "das Band ist offen");
    assert!(pts.len() > 40, "zu wenige Punkte für Schlaufen: {}", pts.len());
    // Der Prim selbst ist durchgezogen — die Schlaufen sind die Geometrie.
    assert_eq!(prims[0].style.line_type, LineType::Solid);

    // Länge exakt: x von 0 bis 5000, nichts darüber hinaus.
    let min_x = pts.iter().fold(f64::INFINITY, |m, p| m.min(p.x));
    let max_x = pts.iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.x));
    assert!(min_x.abs() < 1e-6, "Band beginnt bei x = {min_x}");
    assert!((max_x - 5000.0).abs() < 1e-6, "Band endet bei x = {max_x}");

    // Breite H = 120 mm: die Kurve schwingt zwischen ±60.
    let max_y = pts.iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.y));
    let min_y = pts.iter().fold(f64::INFINITY, |m, p| m.min(p.y));
    assert!((max_y - 60.0).abs() < 1.0, "höchster Punkt bei y = {max_y}");
    assert!((min_y + 60.0).abs() < 1.0, "tiefster Punkt bei y = {min_y}");

    // Start liegt bei +H/2 (Dokumentation: „Start bei y = +H/2“).
    assert!((pts[0].y - 60.0).abs() < 1e-6, "Start bei y = {}", pts[0].y);
}

/// Ohne eigene Angaben gelten die Vorgaben — die Linienart allein genügt.
#[test]
fn ohne_angaben_gelten_breite_und_form_der_vorgabe() {
    let mut doc = Document::new();
    let id = daemmungslinie(&mut doc, None, None);
    let e = doc.entity(id).unwrap();
    assert_eq!(doc.effective_loop_width(e), jjcad_core::daemmung::BREITE_MM);
    assert_eq!(doc.effective_loop_shape(e), jjcad_core::daemmung::FORM);
    let prims = prims(&doc, id);
    let Shape::Poly(pts, _) = &prims[0].shape else {
        panic!("kein Band");
    };
    let max_y = pts.iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.y));
    assert!(
        (max_y - jjcad_core::daemmung::BREITE_MM * 0.5).abs() < 1.0,
        "Höhe folgt nicht der Vorgabe: {max_y}"
    );
}

/// Die Schlaufenform steuert die Periodenzahl: halbe Form = doppelte Perioden.
#[test]
fn schlaufenform_aendert_die_periodenzahl() {
    let mut doc = Document::new();
    let grob = daemmungslinie(&mut doc, Some(100.0), Some(0.5));
    let fein = daemmungslinie(&mut doc, Some(100.0), Some(0.25));
    let anzahl = |id: u64| {
        let prims = prims(&doc, id);
        let Shape::Poly(pts, _) = &prims[0].shape else { panic!("kein Band") };
        // Wendepunkte in y zählen: ein Maximum je Periode.
        let mut n = 0;
        for w in pts.windows(3) {
            if w[0].y < w[1].y && w[1].y >= w[2].y {
                n += 1;
            }
        }
        n
    };
    let (a, b) = (anzahl(grob), anzahl(fein));
    assert!(b >= 2 * a - 1, "Form 0,25 hat {b} Perioden, Form 0,5 nur {a}");
}

/// Breite und Form stehen in der Datei und überleben Speichern/Laden.
#[test]
fn breite_und_form_stehen_in_der_datei() {
    let mut doc = Document::new();
    let id = daemmungslinie(&mut doc, Some(150.0), Some(0.3));
    let pfad = std::env::temp_dir().join(format!("jjcad-t0120-{}.jjcad", std::process::id()));
    jjcad_core::persist::save_document(&mut doc, &pfad).expect("speichern");

    let text = std::fs::read_to_string(&pfad).expect("Datei lesen");
    assert!(text.contains("Insulation"), "Linienart fehlt in der Datei");
    assert!(text.contains("loop_width"), "Breite fehlt in der Datei");
    assert!(text.contains("loop_shape"), "Schlaufenform fehlt in der Datei");

    let geladen = jjcad_core::persist::load_document(&pfad).expect("laden");
    let e = geladen.entity(id).expect("Objekt nach dem Laden");
    assert_eq!(geladen.effective_line_type(e), LineType::Insulation);
    assert!((geladen.effective_loop_width(e) - 150.0).abs() < 1e-6);
    assert!((geladen.effective_loop_shape(e) - 0.3).abs() < 1e-9);
    let _ = std::fs::remove_file(&pfad);
}

/// `modify.set` ändert die beiden Parameter am ausgewählten Objekt.
#[test]
fn modify_set_aendert_breite_und_form() {
    let mut doc = Document::new();
    let id = daemmungslinie(&mut doc, Some(100.0), Some(0.5));
    actions::execute(
        &mut doc,
        "modify.set",
        &json!({"ids": [id], "loop_width": 0.24, "loop_shape": 0.4}),
    )
    .expect("modify.set");
    let e = doc.entity(id).unwrap();
    assert!((doc.effective_loop_width(e) - 240.0).abs() < 1e-6);
    assert!((doc.effective_loop_shape(e) - 0.4).abs() < 1e-9);
}

/// Die Trefferfläche kommt aus der Anzeige — der Klick auf eine Schlaufe
/// trifft, die Verbindungslinie zwischen den Enden dagegen nicht mehr.
#[test]
fn trefferflaeche_folgt_dem_band() {
    let mut doc = Document::new();
    let id = daemmungslinie(&mut doc, Some(200.0), Some(0.5));
    let e = doc.entity(id).unwrap();
    let tb = render::trefferbild(&doc, e);
    assert!(tb.segmente.len() > 40, "Trefferbild hat nur {} Segmente", tb.segmente.len());
    // Der Anfang liegt bei (0, +H/2) — dort trifft der Klick, auf der alten
    // Mittellinie (0, 0) dagegen nicht mehr.
    let Shape::Poly(pts, _) = &prims(&doc, id)[0].shape else {
        panic!("kein Band")
    };
    let start = pts[0];
    assert!(
        jjcad_core::snap::hit_test(&doc, start, 1.0, None) == Some(id),
        "der Anfang des Bandes ist kein Treffer"
    );
    assert!(
        jjcad_core::snap::hit_test(&doc, jjcad_core::geom::Vec2::new(0.0, 0.0), 1.0, None).is_none(),
        "die nackte Mittellinie darf kein Treffer mehr sein"
    );
}

/// Eine Schraffur folgt der Linienart **nicht** — sonst würde eine Wand in
/// „Dämmung“ ihr Muster als Schlaufenband zeichnen.
#[test]
fn schraffur_wird_nicht_zum_schlaufenband() {
    let mut doc = Document::new();
    let id = neu(
        &mut doc,
        "draw.wall",
        json!({
            "a": [0.0, 0.0], "b": [5.0, 0.0], "thickness": 0.24,
            "linetype": "insulation", "hatch": "lines"
        }),
    );
    let prims = prims(&doc, id);
    let baender = prims
        .iter()
        .filter(|p| matches!(&p.shape, Shape::Poly(pts, _) if pts.len() > 40))
        .count();
    // Vier Wandkanten = vier Bänder (zwei Längskanten, zwei Stirnkanten).
    assert_eq!(baender, 4, "die Wandkanten sind nicht alle Bänder");
    // Die Musterlinien bleiben gerade Striche — sonst wären sie ebenfalls
    // Polygonzüge mit vielen Punkten.
    let striche = prims
        .iter()
        .filter(|p| matches!(&p.shape, Shape::Line(_, _)))
        .count();
    assert!(striche > 0, "die Schraffur fehlt in der Anzeige");
    assert!(
        prims
            .iter()
            .filter(|p| matches!(&p.shape, Shape::Line(_, _)))
            .all(|p| p.style.line_type == LineType::Solid),
        "eine Musterlinie trägt noch die Dämmungs-Linienart"
    );
}

/// Dämmung **in einer Wand** (T0161): eine einzelne Linie mit der Linienart
/// „Dämmung“ auf der Wandmitte — Bandhöhe = Wanddicke. Die parallelen
/// Schlaufenbänder des Musters gehören auf eine Fläche, nicht in eine Wand:
/// dort war die Wandfläche plötzlich mit Musterreihen gefüllt.
#[test]
fn wanddaemmung_ist_eine_linie_mit_dem_band() {
    let mut doc = Document::new();
    let id = neu(
        &mut doc,
        "draw.wall",
        json!({
            "a": [0.0, 0.0], "b": [5.0, 0.0], "thickness": 0.24,
            "hatch": "daemmung", "layer": "Wände"
        }),
    );
    let prims = prims(&doc, id);
    // Genau ein Polygonzug mit vielen Punkten — das Band. Die Musterreihen
    // bestünden aus vielen kurzen Strichen (Shape::Line).
    let baender: Vec<&Vec<jjcad_core::geom::Vec2>> = prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Poly(pts, closed) if !closed && pts.len() > 40 => Some(pts),
            _ => None,
        })
        .collect();
    assert_eq!(baender.len(), 1, "die Wand zeichnet nicht genau ein Band");
    let pts = baender[0];
    // Länge: das Band läuft über die ganze Wand (0…5000).
    let min_x = pts.iter().fold(f64::INFINITY, |m, p| m.min(p.x));
    let max_x = pts.iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.x));
    assert!(min_x.abs() < 1e-6, "Band beginnt bei x = {min_x}");
    assert!((max_x - 5000.0).abs() < 1e-6, "Band endet bei x = {max_x}");
    // Bandhöhe = Wanddicke 240 mm, mittig auf der Wandachse (y = 0).
    let max_y = pts.iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.y));
    let min_y = pts.iter().fold(f64::INFINITY, |m, p| m.min(p.y));
    assert!((max_y - 120.0).abs() < 1.5, "höchster Punkt bei y = {max_y}");
    assert!((min_y + 120.0).abs() < 1.5, "tiefster Punkt bei y = {min_y}");
    // Die Musterreihen fehlen: kein Strich im Inneren der Wand, nur die vier
    // Wandkanten (je 5000 bzw. 240 mm lang).
    let innen = prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Line(_, _)))
        .filter(|p| p.style.line_type == LineType::Insulation)
        .count();
    assert_eq!(innen, 0, "die Dämmungslinie steht noch als Linie in der Liste");
}

#[test]
fn dxf_schreibt_das_band_als_polyline() {
    let mut doc = Document::new();
    neu(
        &mut doc,
        "draw.line",
        json!({
            "a": [0.0, 0.0], "b": [5.0, 0.0], "linetype": "insulation",
            "loop_width": 0.1, "loop_shape": 0.5
        }),
    );
    let pfad = std::env::temp_dir().join(format!("jjcad-t0120-{}.dxf", std::process::id()));
    jjcad_core::dxf::export_dxf(&doc, &pfad).expect("DXF schreiben");
    let text = std::fs::read_to_string(&pfad).expect("DXF lesen");
    assert!(text.contains("POLYLINE"), "Band fehlt im DXF");
    let verts = text.matches("\nVERTEX\n").count();
    assert!(verts > 40, "zu wenige Stützpunkte im DXF: {verts}");
    let _ = std::fs::remove_file(&pfad);
}

// ---------------------------------------------------------------------------
// Entfernte Muster (T0132)
// ---------------------------------------------------------------------------

/// „Dämmstoffe Schlaufen“ ist kein Muster mehr: eine Wand mit dem alten
/// Schlüssel bekommt keine Schraffur, und es entsteht auch kein Schlaufenband.
/// Die Dämmung zeichnet allein die Linienart (`draw.line` mit
/// `linetype: "insulation"`).
#[test]
fn alter_wanddaemmungs_schluessel_erzeugt_keine_schraffur() {
    let mut doc = Document::new();
    let id = neu(
        &mut doc,
        "draw.wall",
        json!({
            "a": [0.0, 0.0], "b": [5.0, 0.0], "thickness": 0.24,
            "hatch": "daemmung_schlaufen", "layer": "Wände"
        }),
    );
    let e = doc.entity(id).expect("Wand fehlt");
    let jjcad_core::model::EntityKind::Wall { hatch, .. } = &e.kind else {
        panic!("keine Wand: {:?}", e.kind);
    };
    assert_eq!(*hatch, None, "der alte Musterschlüssel setzt noch eine Schraffur");
    assert!(
        !prims(&doc, id)
            .iter()
            .any(|p| matches!(p.shape, Shape::Poly(ref pts, _) if pts.len() > 40)),
        "die Wand zeichnet noch ein Schlaufenband aus dem Muster"
    );
}
