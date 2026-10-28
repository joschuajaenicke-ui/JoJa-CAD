//! T0026: Die Werkzeug-Vorschau der Tür muss das zeigen, was beim Klick entsteht.
//!
//! Vorher war sie ein eigener Platzhalter: ein **voller Kreis** statt des
//! Viertelbogens, der Anschlag auf der falschen Seite (`hinge` wurde anders
//! gelesen als im Renderer) und die Tür in der **Wandmitte** statt auf der
//! vorderen Wandkante. Nach dem Klick sprang die Tür also um die halbe
//! Wanddicke und spiegelte ihren Anschlag.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::render::{self, DoorSpec, Shape};
use jjcad_core::session::Session;
use serde_json::json;

const WANDDICKE: f64 = 300.0;
const BREITE: f64 = 900.0;
/// Geklickter Punkt auf der Wandachse.
const KLICK: (f64, f64) = (2000.0, 0.0);

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "mm"}))
        .unwrap();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [6000, 0], "thickness": WANDDICKE}),
    )
    .unwrap();
    s
}

fn wand_id(s: &Session) -> u64 {
    s.doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, jjcad_core::model::EntityKind::Wall { .. }))
        .expect("keine Wand im Dokument")
        .id
}

fn spec(hinge: f64, swing_in: bool) -> DoorSpec {
    DoorSpec {
        host: None,
        center: v(KLICK.0, KLICK.1),
        angle: 0.0,
        width: BREITE,
        kind: jjcad_core::model::DoorKind::Hinge,
        hinge,
        swing_in,
    }
}

fn vorschau(s: &Session, hinge: f64, swing_in: bool) -> render::DoorPreview {
    let mut sp = spec(hinge, swing_in);
    sp.host = Some(wand_id(s));
    render::preview_door(&s.doc, &sp).expect("Vorschau ohne Geometrie")
}

/// Linien einer Displayliste, Enden geordnet und gerundet (Vergleich als Menge).
fn linien(dl: &render::DisplayList) -> Vec<(Vec2, Vec2)> {
    let mut out: Vec<(Vec2, Vec2)> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Line(a, b) => Some((rund(*a), rund(*b))),
            _ => None,
        })
        .collect();
    out.sort_by(|x, y| sort_key(x).partial_cmp(&sort_key(y)).unwrap());
    out
}

fn sort_key(p: &(Vec2, Vec2)) -> (f64, f64, f64, f64) {
    (p.0.x, p.0.y, p.1.x, p.1.y)
}

fn rund(p: Vec2) -> Vec2 {
    Vec2::new((p.x * 100.0).round() / 100.0, (p.y * 100.0).round() / 100.0)
}

fn boegen(dl: &render::DisplayList) -> Vec<(Vec2, f64, f64)> {
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Arc { c, r, a0, a1 } => Some((rund(*c), *r, *a1 - *a0)),
            _ => None,
        })
        .collect()
}

/// Die Vorschau zeigt **dieselben** Linien wie das platzierte Objekt — in allen
/// vier Lagen von Anschlag und Öffnungsseite.
#[test]
fn vorschau_tuer_gleich_platzierter_tuer() {
    for hinge in [1.0, -1.0] {
        for swing_in in [true, false] {
            let mut s = session();
            let host = wand_id(&s);
            s.execute(
                "draw.door",
                &json!({"center": [KLICK.0, KLICK.1], "angle": 0.0, "width": BREITE,
                        "host": [host], "hinge": hinge, "swing_in": swing_in,
                        "kind": "hinge"}),
            )
            .unwrap();
            let platziert = s.doc.entities.last().unwrap().clone();
            let ctx = render::RenderCtx::new(&s.doc);
            let mut dl = render::DisplayList::default();
            render::render_entity(&ctx, &platziert, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);

            let vor = vorschau(&s, hinge, swing_in);
            assert_eq!(
                linien(&vor.door),
                linien(&dl),
                "Vorschau-Linien weichen vom Ergebnis ab (hinge {hinge}, swing_in {swing_in})"
            );
            let (b_vor, b_ergebnis) = (boegen(&vor.door), boegen(&dl));
            assert_eq!(b_vor.len(), 1, "genau ein Öffnungsbogen in der Vorschau");
            assert_eq!(b_vor.len(), b_ergebnis.len());
            assert!(
                (b_vor[0].0 - b_ergebnis[0].0).len() < 0.01
                    && (b_vor[0].1 - b_ergebnis[0].1).abs() < 0.01
                    && (b_vor[0].2 - b_ergebnis[0].2).abs() < 1e-6,
                "Öffnungsbogen der Vorschau {:?} statt {:?}",
                b_vor[0],
                b_ergebnis[0]
            );
        }
    }
}

/// Das Blatt sitzt auf der **vorderen Wandkante** und der Anschlag auf der
/// Seite, die die Taste F wählt.
#[test]
fn vorschau_tuer_blatt_auf_der_vorderen_kante() {
    let s = session();
    let h = WANDDICKE * 0.5;
    let hw = BREITE * 0.5;

    // Anschlag rechts (hinge = 1), Blattseite A (swing_in = true)
    let vor = vorschau(&s, 1.0, true);
    let blatt = linien(&vor.door)
        .into_iter()
        .find(|(a, b)| (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() > 1.0)
        .expect("kein Türblatt in der Vorschau");
    let unten = if blatt.0.y < blatt.1.y { blatt.0 } else { blatt.1 };
    let oben = if blatt.0.y < blatt.1.y { blatt.1 } else { blatt.0 };
    assert!(
        (unten.x - (KLICK.0 - hw)).abs() < 0.01 && (unten.y - h).abs() < 0.01,
        "Scharnier muss auf der vorderen Kante liegen: {unten:?}, erwartet ({}, {h})",
        KLICK.0 - hw
    );
    assert!(
        (oben.y - (h + BREITE)).abs() < 0.01,
        "Blattlänge {} statt {BREITE}",
        oben.y - unten.y
    );

    // Anschlag links (hinge = -1) — vorher stand er auf der anderen Seite
    let vor = vorschau(&s, -1.0, true);
    let blatt = linien(&vor.door)
        .into_iter()
        .find(|(a, b)| (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() > 1.0)
        .expect("kein Türblatt in der Vorschau");
    let x = blatt.0.x;
    assert!(
        (x - (KLICK.0 + hw)).abs() < 0.01,
        "Anschlag links gehört an das andere Wandende: {x} statt {}",
        KLICK.0 + hw
    );
}

/// Der Öffnungsbogen ist ein **Viertelkreis** (kein voller Kreis) mit dem
/// Radius der Türbreite.
#[test]
fn vorschau_tuer_bogen_ist_viertelkreis() {
    let s = session();
    let vor = vorschau(&s, 1.0, true);
    let b = boegen(&vor.door);
    assert_eq!(b.len(), 1, "genau ein Bogen");
    assert!(
        (b[0].2.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
        "Bogenwinkel {} statt 90°",
        b[0].2.to_degrees()
    );
    assert!((b[0].1 - BREITE).abs() < 0.01, "Bogenradius {}", b[0].1);
}

/// Die Wand in der Vorschau hat an der Tür ihr Loch — nach dem Klick springt
/// dort keine Kante mehr auf.
#[test]
fn vorschau_schneidet_das_loch_in_die_wand() {
    let s = session();
    let h = WANDDICKE * 0.5;
    let vor = vorschau(&s, 1.0, true);

    let durch = |dl: &render::DisplayList, p: Vec2| {
        dl.prims.iter().any(|prim| match &prim.shape {
            Shape::Line(a, b) => jjcad_core::geom::dist_point_segment(p, *a, *b) < 0.05,
            _ => false,
        })
    };
    for y in [h, -h] {
        let mitte = v(2000.0, y);
        assert!(
            !durch(&vor.wall, mitte),
            "Wandkante läuft durch die Öffnung ({mitte:?})"
        );
        // Gegenprobe: im Dokument ohne Tür liegt dort eine Kante — der Test beißt.
        let geschlossen = s.doc.clone();
        let mut dl = render::DisplayList::default();
        let ctx = render::RenderCtx::new(&geschlossen);
        if let Some(w) = geschlossen.entities.first() {
            render::render_entity(&ctx, w, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
        }
        assert!(durch(&dl, mitte), "Gegenprobe: geschlossene Wand ohne Kante");
    }
}

/// Die freigelegte Fläche ist genau die Öffnung: Türbreite × Wanddicke, mittig
/// zum Klickpunkt.
#[test]
fn vorschau_legt_die_oeffnung_frei() {
    let s = session();
    let vor = vorschau(&s, 1.0, true);
    assert_eq!(vor.loch.len(), 1, "eine Öffnung");
    let ecken = &vor.loch[0];
    let xs: Vec<f64> = ecken.iter().map(|p| p.x).collect();
    let ys: Vec<f64> = ecken.iter().map(|p| p.y).collect();
    let (x0, x1) = (
        xs.iter().cloned().fold(f64::MAX, f64::min),
        xs.iter().cloned().fold(f64::MIN, f64::max),
    );
    let (y0, y1) = (
        ys.iter().cloned().fold(f64::MAX, f64::min),
        ys.iter().cloned().fold(f64::MIN, f64::max),
    );
    assert!(
        (x1 - x0 - BREITE).abs() < 0.01,
        "Öffnungsbreite {} statt {BREITE}",
        x1 - x0
    );
    assert!(
        (y1 - y0 - WANDDICKE).abs() < 0.01,
        "Öffnung über die Wanddicke {} statt {WANDDICKE}",
        y1 - y0
    );
    assert!(
        ((x0 + x1) * 0.5 - KLICK.0).abs() < 0.01,
        "Öffnung nicht mittig zum Klickpunkt"
    );
}

/// Auch die Wand in der Vorschau ist die Wand **mit** der Tür: dieselben
/// Kanten, dasselbe Loch — kein Sprung beim Klick.
#[test]
fn vorschau_wand_gleich_wand_mit_tuer() {
    let mut s = session();
    let host = wand_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [KLICK.0, KLICK.1], "angle": 0.0, "width": BREITE,
                "host": [host], "hinge": 1.0, "swing_in": true, "kind": "hinge"}),
    )
    .unwrap();
    let mut dl = render::DisplayList::default();
    let ctx = render::RenderCtx::new(&s.doc);
    let wand = s.doc.entities.iter().find(|e| e.id == host).unwrap();
    render::render_entity(&ctx, wand, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);

    let vor = vorschau(&s, 1.0, true);
    assert_eq!(
        linien(&vor.wall),
        linien(&dl),
        "Wandkanten der Vorschau weichen von der Wand mit Tür ab"
    );
}
