//! T0027: Die Treppe wird über die Lauflinie (zwei Punkte) und die Breite
//! (dritter Punkt) gesetzt; die Höhe steht als Eigenschaft daneben und bestimmt
//! über die Schrittmaßregel (2 S + A = 63 cm) die Stufenzahl.
//!
//! Der Bericht des Tickets nennt drei Dinge, die hier geprüft werden:
//! * die Anzeige hat **so viele Stufenlinien wie Stufen** (minus eins),
//! * die Stufenzahl folgt Länge und Höhe, solange sie nicht überschrieben ist,
//! * die Beschriftungstaste erzeugt ein Textfeld mit Stufenzahl und Schrittmaß.

use jjcad_core::geom::{Affine, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::render::{self, Shape};
use jjcad_core::session::Session;
use serde_json::json;

const BREITE: f64 = 1000.0;
const HOEHE: f64 = 2752.0;
const LAENGE: f64 = 4640.0;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .unwrap();
    s
}

/// Treppe senkrecht nach oben: Lauflinie (0,0) → (0, Länge).
fn treppe(s: &mut Session, extra: serde_json::Value) -> u64 {
    let mut params = json!({
        "a": [0.0, 0.0],
        "b": [0.0, LAENGE],
        "width": BREITE,
        "height": HOEHE,
    });
    if let (Some(ziel), Some(quelle)) = (params.as_object_mut(), extra.as_object()) {
        for (k, v) in quelle {
            ziel.insert(k.clone(), v.clone());
        }
    }
    let r = s.execute("draw.stair", &params).unwrap();
    r["created"][0].as_u64().unwrap()
}

fn anzeige(s: &Session, id: u64) -> render::DisplayList {
    let e = s.doc.entity(id).unwrap().clone();
    let ctx = render::RenderCtx::new(&s.doc);
    let mut dl = render::DisplayList::default();
    render::render_entity(&ctx, &e, Affine::IDENTITY, &mut dl, 0);
    dl
}

/// y-Lagen der Stufenlinien (waagerecht über die Treppenbreite).
///
/// Die Abschnittslinie unterbricht einzelne Stufenlinien; die Stücke werden
/// über ihre y-Lage zusammengefasst, damit die Zahl der **Stufen** zählt und
/// nicht die Zahl der Linienstücke.
fn stufenlagen(dl: &render::DisplayList) -> Vec<f64> {
    let mut ys: Vec<f64> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Line(a, b)
                if (a.y - b.y).abs() < 0.01 && (a.x - b.x).abs() > BREITE * 0.25 =>
            {
                Some((a.y * 10.0).round() / 10.0)
            }
            _ => None,
        })
        .collect();
    ys.sort_by(|x, y| x.partial_cmp(y).unwrap());
    ys.dedup();
    ys
}

fn kind(s: &Session, id: u64) -> (u32, f64, bool, f64) {
    match s.doc.entity(id).map(|e| &e.kind) {
        Some(EntityKind::Stair {
            steps,
            height,
            auto_steps,
            width,
            ..
        }) => (*steps, *height, *auto_steps, *width),
        _ => panic!("keine Treppe"),
    }
}

/// Aus Länge 4,64 m und Höhe 2,752 m folgen 16 Stufen — und die Anzeige hat
/// 15 Stufenlinien plus Umriss.
#[test]
fn automatische_stufenzahl_aus_laenge_und_hoehe() {
    let mut s = session();
    let id = treppe(&mut s, json!({}));
    let (steps, height, auto, width) = kind(&s, id);
    assert!(auto, "neue Treppe folgt der Schrittmaßregel");
    assert!((height - HOEHE).abs() < 0.01);
    assert!((width - BREITE).abs() < 0.01);
    assert_eq!(steps, 16, "gespeicherte Vorgabe bleibt unangetastet");

    let dl = anzeige(&s, id);
    let lagen = stufenlagen(&dl);
    assert_eq!(lagen.len(), 15, "16 Stufen ergeben 15 Stufenlinien: {lagen:?}");
    // Gleichmäßige Teilung: erste Linie bei Länge/16.
    assert!((lagen[0] - LAENGE / 16.0).abs() < 0.5, "{lagen:?}");
    assert_eq!(
        dl.prims
            .iter()
            .filter(|p| matches!(p.shape, Shape::Poly(_, true)))
            .count(),
        1,
        "genau ein Umriss"
    );
}

/// Die Höhe ändert die Stufenzahl: 3,00 m hoch ergibt 17 Stufen.
#[test]
fn hoehe_bestimmt_die_stufenzahl() {
    let mut s = session();
    let id = treppe(&mut s, json!({}));
    s.execute("modify.set", &json!({"ids": [id], "height": 3000.0}))
        .unwrap();
    let dl = anzeige(&s, id);
    assert_eq!(stufenlagen(&dl).len(), 16, "17 Stufen ergeben 16 Linien");
}

/// Eine ausdrückliche Stufenzahl überschreibt die Regel und bleibt stehen.
#[test]
fn manuelle_stufenzahl_gilt() {
    let mut s = session();
    let id = treppe(&mut s, json!({"steps": 12}));
    let (steps, _, auto, _) = kind(&s, id);
    assert!(!auto, "steps im Aufruf schaltet die Regel ab");
    assert_eq!(steps, 12);
    assert_eq!(stufenlagen(&anzeige(&s, id)).len(), 11);

    // Höhe ändern darf die feste Zahl nicht mehr verschieben.
    s.execute("modify.set", &json!({"ids": [id], "height": 3000.0}))
        .unwrap();
    assert_eq!(stufenlagen(&anzeige(&s, id)).len(), 11);
}

/// „Automatisch" wieder einschalten — die Zahl folgt erneut der Regel.
#[test]
fn automatisch_wieder_einschalten() {
    let mut s = session();
    let id = treppe(&mut s, json!({"steps": 12}));
    s.execute(
        "modify.set",
        &json!({"ids": [id], "auto_steps": true}),
    )
    .unwrap();
    assert_eq!(stufenlagen(&anzeige(&s, id)).len(), 15);
}

/// Die Beschriftungstaste erzeugt ein Textfeld links neben der Treppe.
#[test]
fn beschriftung_erzeugt_textfeld() {
    let mut s = session();
    let id = treppe(&mut s, json!({}));
    let r = s
        .execute("draw.stair.label", &json!({"id": id}))
        .unwrap();
    let text_id = r["created"][0].as_u64().unwrap();
    let Some(EntityKind::Text {
        p,
        text,
        align,
        ..
    }) = s.doc.entity(text_id).map(|e| &e.kind)
    else {
        panic!("kein Textfeld erzeugt");
    };
    assert_eq!(text, "16 Stg\n17,2 / 29");
    assert!(
        p.x < -BREITE * 0.5,
        "Textfeld steht links neben der Treppe: {p:?}"
    );
    assert!(matches!(align, jjcad_core::model::TextAlignH::Left));
    // Der Textkasten endet im Abstand von 0,8 Texthöhen an der Treppenkante.
    let (tw, _) = jjcad_core::render::text_box(text, 250.0);
    let kante = p.x + tw;
    assert!(
        (-BREITE * 0.5 - 200.0 - kante).abs() < 1.0,
        "Textkante {kante} statt {}",
        -BREITE * 0.5 - 200.0
    );
    // Am Lauf mittig, also auf halber Länge.
    assert!((p.y - LAENGE * 0.5).abs() < 1.0, "{p:?}");
}

/// Die Treppe ist an ihrem Umriss zu treffen, nicht nur auf der Achse.
#[test]
fn trefferflaeche_ist_der_umriss() {
    let mut s = session();
    let id = treppe(&mut s, json!({}));
    let e = s.doc.entity(id).unwrap().clone();
    let segs = render::entity_segments(&s.doc, &e);
    assert_eq!(segs.len(), 4 + 15, "Umriss plus Stufenlinien");
    let auf_der_kante = Vec2::new(BREITE * 0.5, LAENGE * 0.5);
    assert!(
        segs.iter().any(|seg| match seg {
            jjcad_core::geom::Seg::Line(a, b) => {
                jjcad_core::geom::dist_point_segment(auf_der_kante, *a, *b) < 0.01
            }
            _ => false,
        }),
        "Kante gehört zur Trefferfläche: {segs:?}"
    );
    // Außenkante anklicken wählt die Treppe (vorher lag dort nichts).
    assert_eq!(
        jjcad_core::snap::hit_test(&s.doc, auf_der_kante, 5.0, None),
        Some(id)
    );
}

/// Ältere Zeichnungen ohne `height`/`auto_steps` behalten ihre Stufenzahl.
#[test]
fn alte_datei_behaelt_ihre_stufen() {
    let mut s = session();
    treppe(&mut s, json!({}));
    let mut datei: serde_json::Value =
        serde_json::from_str(&jjcad_core::persist::to_json(&s.doc).unwrap()).unwrap();
    {
        let kind = &mut datei["entities"][0]["kind"];
        let obj = kind.as_object_mut().unwrap();
        assert_eq!(obj.get("type").and_then(|t| t.as_str()), Some("stair"));
        obj.remove("height");
        obj.remove("auto_steps");
        obj.insert("steps".into(), json!(13));
    }
    let alt = serde_json::to_string(&datei).unwrap();
    let doc = jjcad_core::persist::from_json(&alt).expect("alte Datei lädt nicht");
    let Some(EntityKind::Stair {
        steps,
        auto_steps,
        height,
        ..
    }) = doc.entities.first().map(|e| &e.kind)
    else {
        panic!("Treppe fehlt");
    };
    assert_eq!(*steps, 13);
    assert!(!*auto_steps, "alte Treppe bleibt manuell");
    assert!((*height - jjcad_core::stair::HOEHE_VORGABE_MM).abs() < 0.01);
}
