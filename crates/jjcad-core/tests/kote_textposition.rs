//! Lage des Höhenwertes an der Grundriss-Kote (T0196).
//!
//! Der Höhenwert gehört zum Symbol, seine Lage ist aber frei wählbar:
//! **Oben** (Vorgabe), **Unten**, **Links**, **Rechts**. Geprüft wird über die
//! Befehlsschicht und den Renderer — denselben Weg, den GUI, PDF, PNG und MCP
//! nehmen. Die Lage kommt aus dem Textprim der Anzeige; damit ist der Test
//! blind für einen zweiten Platzierer im Zeichner.

use jjcad_core::actions;
use jjcad_core::geom::v;
use jjcad_core::model::{Document, ElevationTextPos, EntityKind};
use jjcad_core::render::{self, RenderCtx, Shape, TextPrim};
use jjcad_core::units::Unit;
use serde_json::json;

fn doc_mm() -> Document {
    let mut d = Document::new();
    d.settings.input_unit = Unit::Mm;
    d
}

/// Erzeugt eine Grundriss-Kote und liefert ihre Kennung.
fn kote(doc: &mut Document, text_pos: Option<&str>) -> u64 {
    let mut p = json!({"p": [1000.0, 2000.0], "z": 1200.0});
    if let Some(t) = text_pos {
        p["text_pos"] = json!(t);
    }
    let r = actions::execute(doc, "dim.elevation.plan", &p)
        .unwrap_or_else(|e| panic!("dim.elevation.plan fehlgeschlagen: {e}"));
    r["created"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("kein Objekt erzeugt: {r}"))
}

/// Der Textprim des Höhenwertes aus der Anzeige.
fn wert(doc: &Document) -> TextPrim {
    let ctx = RenderCtx::new(doc);
    let dl = render::render_document(&ctx);
    dl.prims
        .iter()
        .find_map(|p| match &p.shape {
            Shape::Text(t) => Some(t.clone()),
            _ => None,
        })
        .expect("kein Höhenwert in der Anzeige")
}

fn lage(doc: &Document, pos: ElevationTextPos) -> TextPrim {
    let t = wert(doc);
    let h = doc.dim_style.text_height * doc.plans[0].scale;
    let r = h * 0.55;
    let abstand = r * 1.4 + h * 0.3;
    let p = v(1000.0, 2000.0);
    match pos {
        ElevationTextPos::Top => {
            assert!(
                (t.pos.x - p.x).abs() < 1e-9 && (t.pos.y - (p.y + abstand)).abs() < 1e-9,
                "Oben: Anker über dem Symbol erwartet, ist {:?}",
                t.pos
            );
            assert_eq!(t.align, jjcad_core::model::TextAlignH::Center);
            assert_eq!(t.valign, jjcad_core::model::TextAlignV::Bottom);
        }
        ElevationTextPos::Bottom => {
            assert!(
                (t.pos.x - p.x).abs() < 1e-9 && (t.pos.y - (p.y - abstand)).abs() < 1e-9,
                "Unten: Anker unter dem Symbol erwartet, ist {:?}",
                t.pos
            );
            assert_eq!(t.align, jjcad_core::model::TextAlignH::Center);
            assert_eq!(t.valign, jjcad_core::model::TextAlignV::Top);
        }
        ElevationTextPos::Left => {
            assert!(
                (t.pos.y - p.y).abs() < 1e-9 && (t.pos.x - (p.x - abstand)).abs() < 1e-9,
                "Links: Anker links vom Symbol erwartet, ist {:?}",
                t.pos
            );
            assert_eq!(t.align, jjcad_core::model::TextAlignH::Right);
            assert_eq!(t.valign, jjcad_core::model::TextAlignV::Middle);
        }
        ElevationTextPos::Right => {
            assert!(
                (t.pos.y - p.y).abs() < 1e-9 && (t.pos.x - (p.x + abstand)).abs() < 1e-9,
                "Rechts: Anker rechts vom Symbol erwartet, ist {:?}",
                t.pos
            );
            assert_eq!(t.align, jjcad_core::model::TextAlignH::Left);
            assert_eq!(t.valign, jjcad_core::model::TextAlignV::Middle);
        }
    }
    t
}

#[test]
fn vorgabe_ist_oben() {
    let mut doc = doc_mm();
    let id = kote(&mut doc, None);
    assert!(matches!(
        doc.entity(id).expect("Kote fehlt").kind,
        EntityKind::ElevationPlan {
            text_pos: ElevationTextPos::Top,
            ..
        }
    ));
    lage(&doc, ElevationTextPos::Top);
}

#[test]
fn alle_vier_lagen_setzen_den_wert_auf_die_richtige_seite() {
    for (roh, pos) in [
        ("oben", ElevationTextPos::Top),
        ("unten", ElevationTextPos::Bottom),
        ("links", ElevationTextPos::Left),
        ("rechts", ElevationTextPos::Right),
        ("top", ElevationTextPos::Top),
        ("bottom", ElevationTextPos::Bottom),
        ("left", ElevationTextPos::Left),
        ("right", ElevationTextPos::Right),
    ] {
        let mut doc = doc_mm();
        let id = kote(&mut doc, Some(roh));
        assert!(matches!(
            doc.entity(id).expect("Kote fehlt").kind,
            EntityKind::ElevationPlan { text_pos, .. } if text_pos == pos
        ));
        lage(&doc, pos);
    }
}

/// Die Lage lässt sich nachträglich über `modify.set` ändern — der Weg, den
/// das Eigenschaften-Panel nimmt.
#[test]
fn modify_set_aendert_die_lage() {
    let mut doc = doc_mm();
    let id = kote(&mut doc, None);
    actions::execute(
        &mut doc,
        "modify.set",
        &json!({"ids": [id], "text_pos": "links"}),
    )
    .expect("text_pos setzen");
    assert!(matches!(
        doc.entity(id).expect("Kote fehlt").kind,
        EntityKind::ElevationPlan {
            text_pos: ElevationTextPos::Left,
            ..
        }
    ));
    lage(&doc, ElevationTextPos::Left);
}

/// Ein unbekannter Wert ist ein Fehler (T0038), kein stiller Rückfall auf
/// „Oben".
#[test]
fn unbekannte_lage_ist_ein_fehler() {
    let mut doc = doc_mm();
    let fehler = actions::execute(
        &mut doc,
        "dim.elevation.plan",
        &json!({"p": [0.0, 0.0], "z": 0.0, "text_pos": "schraeg"}),
    );
    assert!(fehler.is_err(), "unbekannte Lage wurde angenommen");

    let id = kote(&mut doc, None);
    let fehler = actions::execute(
        &mut doc,
        "modify.set",
        &json!({"ids": [id], "text_pos": "schraeg"}),
    );
    assert!(fehler.is_err(), "unbekannte Lage wurde angenommen");
}

/// Alte Zeichnungen tragen das Feld nicht — sie lesen „Oben". Geschrieben
/// wird der Schlüssel danach mit.
#[test]
fn alte_datei_liest_oben_und_schreibt_die_lage_mit() {
    let alt = json!({
        "type": "elevation_plan",
        "p": {"x": 1000.0, "y": 2000.0},
        "z": 1200.0
    });
    let kind: EntityKind = serde_json::from_value(alt).expect("alte Kote lesen");
    assert!(matches!(
        kind,
        EntityKind::ElevationPlan {
            text_pos: ElevationTextPos::Top,
            ..
        }
    ));

    let neu = serde_json::to_value(EntityKind::ElevationPlan {
        p: v(0.0, 0.0),
        z: 0.0,
        text_pos: ElevationTextPos::Bottom,
    })
    .expect("schreiben");
    assert_eq!(neu["text_pos"], json!("bottom"));

    let zurueck: EntityKind = serde_json::from_value(neu).expect("zurücklesen");
    assert!(matches!(
        zurueck,
        EntityKind::ElevationPlan {
            text_pos: ElevationTextPos::Bottom,
            ..
        }
    ));
}
