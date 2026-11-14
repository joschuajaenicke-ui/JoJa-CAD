//! Zerlegen mehrerer Objekte **auf einmal** (T0190).
//!
//! Verbundene Wände, sich kreuzende Wände und Wand + Volumen zeichnen ihre
//! Umrandung aus dem **Umfeld**: Gehrung an der Ecke, Freistellen der Stirnkante
//! im Nachbarkörper, Anschluss an ein Volumen. Wird Objekt für Objekt zerlegt,
//! sieht das zweite Objekt den bereits entfernten Nachbarn nicht mehr und
//! bekommt eine geschlossene Umrandung — das Bild nach dem Zerlegen weicht dann
//! vom Bild davor ab.
//!
//! Geprüft wird die gezeichnete Geometrie: die Strecken der Displayliste vor
//! dem Zerlegen müssen als Strecken der neuen Objekte **alle** wieder da sein
//! und es darf keine neue dazukommen.

use jjcad_core::model::{Document, Id};
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::Session;
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

fn knoten(p: jjcad_core::geom::Vec2) -> (i64, i64) {
    (
        (p.x * 1000.0).round() as i64,
        (p.y * 1000.0).round() as i64,
    )
}

/// Alle gezeichneten Strecken als Menge (Multimenge) von Knotenpaaren.
fn strecken(doc: &Document) -> Vec<((i64, i64), (i64, i64))> {
    let ctx = RenderCtx::new(doc);
    let dl = render::render_document(&ctx);
    let mut out = Vec::new();
    let mut push = |a: jjcad_core::geom::Vec2, b: jjcad_core::geom::Vec2| {
        if a.dist(b) < 1e-6 {
            return;
        }
        let (a, b) = (knoten(a), knoten(b));
        out.push(if a <= b { (a, b) } else { (b, a) });
    };
    for prim in &dl.prims {
        match &prim.shape {
            Shape::Line(a, b) => push(*a, *b),
            Shape::Poly(pts, closed) => {
                for w in pts.windows(2) {
                    push(w[0], w[1]);
                }
                if *closed && pts.len() > 2 {
                    push(pts[pts.len() - 1], pts[0]);
                }
            }
            _ => {}
        }
    }
    out.sort();
    out
}

/// Zerlegt die Objekte über die Action-Schicht und vergleicht die Strecken
/// vorher/nachher. `fehlt`/`zuviel` werden im Fehlerfall benannt.
fn zerlegen_und_vergleichen(mut s: Session, ids: &[Id], was: &str) {
    let vorher = strecken(&s.doc);
    assert!(!vorher.is_empty(), "{was}: nichts gezeichnet");
    let antwort = s
        .execute("modify.explode", &json!({ "ids": ids }))
        .unwrap_or_else(|e| panic!("{was}: zerlegen schlug fehl: {e}"));
    assert_eq!(
        antwort["ok"], true,
        "{was}: unerwartete Antwort {antwort}"
    );
    let nachher = strecken(&s.doc);

    let fehlt: Vec<_> = vorher
        .iter()
        .filter(|x| !nachher.contains(x))
        .copied()
        .collect();
    let zuviel: Vec<_> = nachher
        .iter()
        .filter(|x| !vorher.contains(x))
        .copied()
        .collect();
    assert!(
        fehlt.is_empty() && zuviel.is_empty(),
        "{was}: Bild weicht ab — fehlende Strecken (mm/1000): {fehlt:?}, neue Strecken: {zuviel:?}"
    );
}

fn alle_ids(s: &Session) -> Vec<Id> {
    s.doc.entities.iter().map(|e| e.id).collect()
}

/// Zwei Wände in der Ecke: die Gehrung entsteht nur, solange beide Wände da
/// sind. Nach dem gemeinsamen Zerlegen muss sie noch stehen.
#[test]
fn ecke_bleibt_nach_dem_zerlegen_wie_gezeichnet() {
    let mut s = session();
    for (a, b) in [([-2.0, 0.0], [0.0, 0.0]), ([0.0, 0.0], [0.0, 2.0])] {
        s.execute(
            "draw.wall",
            &json!({"a": a, "b": b, "thickness": 0.2, "align": "center"}),
        )
        .unwrap();
    }
    let ids = alle_ids(&s);
    assert_eq!(ids.len(), 2);

    zerlegen_und_vergleichen(s, &ids, "Ecke");
}

/// Zwei sich kreuzende Wände.
#[test]
fn kreuzung_bleibt_nach_dem_zerlegen_wie_gezeichnet() {
    let mut s = session();
    for (a, b) in [([-2.0, 0.0], [2.0, 0.0]), ([0.0, -2.0], [0.0, 2.0])] {
        s.execute(
            "draw.wall",
            &json!({"a": a, "b": b, "thickness": 0.24, "align": "center"}),
        )
        .unwrap();
    }
    let ids = alle_ids(&s);
    zerlegen_und_vergleichen(s, &ids, "Kreuzung");
}

/// Wand und Volumen: beide schneiden einander die Umrandung frei.
#[test]
fn wand_und_volumen_bleiben_nach_dem_zerlegen_wie_gezeichnet() {
    let mut s = session();
    s.execute(
        "draw.volume",
        &json!({
            "boundary": [[1.2, -0.8], [3.2, -0.8], [3.2, 0.8], [1.2, 0.8]],
            "height": 2.75,
            "border": true,
            "pattern": "solid",
        }),
    )
    .unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0], "b": [2.2, 0.0], "thickness": 0.24}),
    )
    .unwrap();
    let ids = alle_ids(&s);
    assert_eq!(ids.len(), 2);
    zerlegen_und_vergleichen(s, &ids, "Wand + Volumen");
}

/// Ein Fehler in der Liste (unbekannte Kennung) darf die anderen nicht schon
/// zerlegt zurücklassen.
#[test]
fn fehler_laesst_die_auswahl_unveraendert() {
    let mut s = session();
    s.execute(
        "draw.wall",
        &json!({"a": [0.0, 0.0], "b": [2.0, 0.0], "thickness": 0.2}),
    )
    .unwrap();
    let ids = alle_ids(&s);
    let vorher = strecken(&s.doc);
    assert!(s
        .execute("modify.explode", &json!({"ids": [ids[0], 999_999_u64]}))
        .is_err());
    assert_eq!(s.doc.entities.len(), 1, "Auswahl wurde verändert");
    assert_eq!(strecken(&s.doc), vorher, "Umrandung wurde verändert");
}
