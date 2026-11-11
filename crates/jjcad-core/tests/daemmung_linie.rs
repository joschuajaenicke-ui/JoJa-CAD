//! Dämmung als **Linie**: Bounding-Box und Trefferprüfung ohne Band (T0168).
//!
//! Die Linienart [`LineType::Insulation`] legt statt der Linie ein
//! Schlaufenband ab. Bei 100 m² Dämmung sind das 112 545 Punkte — für die
//! Bounding-Box wurde das Band vorher **gerechnet**, und Fang wie
//! Trefferprüfung fragen die Box je Bild mehrfach (gemessen 2,9 ms je Abfrage,
//! `hit_test` 7,4 ms je Klick). T0164 hat dasselbe für die Schraffur behoben;
//! hier gelten dieselben Regeln für die Linienfassung:
//!
//! * die Box kommt aus dem **Weg** der Linie, je Strecke um die halbe Bandhöhe
//!   quer versetzt (exakt, nicht gerundet — `Document::bbox` bestimmt die Größe
//!   von PNG und PDF),
//! * die Trefferprüfung sieht nur Objekte, deren Ausdehnung den Zeiger erreicht.
//!
//! Die Proben messen die Zeit mit großzügiger Grenze: der Unterschied ist eine
//! Größenordnung, ein langsamer Rechner soll den Test nicht kippen.

use jjcad_core::geom::Vec2;
use jjcad_core::render::{self, Prim, RenderCtx, Shape};
use jjcad_core::session::Session;
use serde_json::json;

/// Dämmstärke der Probe (mm).
const H: f64 = 200.0;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .expect("Eingabe in Millimetern");
    s
}

/// Linie mit der Linienart „Dämmung“ anlegen und ihr Objekt liefern.
fn daemmungslinie(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> jjcad_core::model::Entity {
    let antwort = s
        .execute(
            "draw.line",
            &json!({
                "a": a,
                "b": b,
                "linetype": "insulation",
                "loop_width": H,
                "layer": "Wände"
            }),
        )
        .expect("Linie");
    let id = antwort["created"][0].as_u64().expect("Id");
    s.doc.ents().iter().find(|e| e.id == id).cloned().expect("Objekt")
}

/// Alle Punkte der gezeichneten Dämmung eines Objekts.
fn band_punkte(doc: &jjcad_core::model::Document, e: &jjcad_core::model::Entity) -> Vec<Vec2> {
    let ctx = RenderCtx::for_entity(doc, e);
    let mut dl = render::DisplayList::default();
    render::render_entity(&ctx, e, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
    let mut out = Vec::new();
    for p in &dl.prims {
        match &p.shape {
            Shape::Poly(pts, _) => out.extend(pts.iter().copied()),
            Shape::Line(a, b) => out.extend([*a, *b]),
            _ => {}
        }
    }
    out
}

/// Der gezeichnete Umriss (Füllungen zählen nicht) — für die Gegenprobe.
fn umriss_punkte(prims: &[Prim]) -> usize {
    prims
        .iter()
        .filter(|p| matches!(p.shape, Shape::Poly(..) | Shape::Line(..)))
        .count()
}

/// Die Box einer Dämmungslinie enthält das Band und ist nicht größer als es.
///
/// Geprüft an gerader Linie, Polylinie (Ecke), Bogen und Kreis: dort, wo die
/// Querrichtung wechselt, muss die Hülle trotzdem dicht bleiben.
#[test]
fn huelle_der_daemmungslinie_umfasst_das_band() {
    let mut s = session();
    // Gerade Linie waagerecht
    let gerade = daemmungslinie(&mut s, [0.0, 1000.0], [10000.0, 1000.0]);
    // Polylinie mit Ecke
    s.execute(
        "draw.polyline",
        &json!({
            "points": [[0.0, 3000.0], [4000.0, 3000.0], [4000.0, 6000.0]],
            "linetype": "insulation",
            "loop_width": H,
            "layer": "Wände"
        }),
    )
    .expect("Polylinie");
    // Kreis und Bogen
    s.execute(
        "draw.circle",
        &json!({ "c": [8000.0, 3000.0], "r": 900.0, "linetype": "insulation",
                 "loop_width": H, "layer": "Wände" }),
    )
    .expect("Kreis");
    s.execute(
        "draw.arc",
        &json!({ "c": [8000.0, 6000.0], "r": 900.0, "a0": 0.0, "a1": 90.0,
                 "linetype": "insulation", "loop_width": H, "layer": "Wände" }),
    )
    .expect("Bogen");

    let mut geprueft = 0;
    for e in s.doc.ents().to_vec() {
        if s.doc.effective_line_type(&e) != jjcad_core::style::LineType::Insulation {
            continue;
        }
        let bb = render::entity_bbox(&s.doc, &e);
        assert!(bb.is_valid(), "{:?}: keine Box", e.kind);
        let punkte = band_punkte(&s.doc, &e);
        assert!(punkte.len() > 10, "{:?}: kein Band gezeichnet", e.kind);
        for p in &punkte {
            assert!(
                bb.expanded(1e-6).contains(*p),
                "{:?}: Bandpunkt {p:?} liegt außerhalb der Box {bb:?}",
                e.kind
            );
        }
        // Dicht: die Box liegt höchstens 2 mm über dem gezeichneten Band (die
        // Abtastung erreicht die theoretische Bandhöhe nicht ganz). Ein rundum
        // um H/2 gewachsener Kasten wäre 100 mm zu breit.
        let eng = jjcad_core::geom::BBox::from_points(&punkte);
        let zu_breit = (bb.max.x - eng.max.x)
            .max(bb.max.y - eng.max.y)
            .max(eng.min.x - bb.min.x)
            .max(eng.min.y - bb.min.y);
        assert!(
            zu_breit < 2.0,
            "{:?}: Box {bb:?} steht {zu_breit:.1} mm über dem Band {eng:?}",
            e.kind
        );
        geprueft += 1;
    }
    assert!(geprueft >= 4, "nur {geprueft} Dämmungslinien geprüft");
    // Und die gerade Linie liegt genau in der Bandhöhe um die Achse.
    let bb = render::entity_bbox(&s.doc, &gerade);
    assert!((bb.min.y - (1000.0 - H / 2.0)).abs() < 1e-6, "unten {:?}", bb.min);
    assert!((bb.max.y - (1000.0 + H / 2.0)).abs() < 1e-6, "oben {:?}", bb.max);
}

/// 100 m² Dämmung als Linien: die Box kommt aus dem Weg, nicht aus dem Band.
///
/// Vorher rechnete `entity_bbox` für **jede** der 45 Linien das ganze Band
/// (2500 Punkte) — 2,9 ms je Abfrage. `Document::bbox` und die Trefferprüfung
/// fragen sie mehrfach je Bild; gemessen wird hier 20-mal.
#[test]
fn bbox_einer_daemmungslinie_zeichnet_das_band_nicht() {
    let mut s = session();
    for i in 0..45 {
        let y = 100.0 + i as f64 * 200.0;
        daemmungslinie(&mut s, [0.0, y], [10000.0, y]);
    }
    assert_eq!(s.doc.ents().len(), 45);

    let bb = s.doc.bbox();
    assert!(
        (bb.min.x).abs() < 1e-6 && (bb.max.x - 10000.0).abs() < 1e-6,
        "Box in x falsch: {bb:?}"
    );
    // Erste Linie bei y = 100 mit H = 200: das Band beginnt bei 0.
    assert!((bb.min.y).abs() < 1e-6, "Box beginnt nicht bei 0: {bb:?}");
    assert!(
        (bb.max.y - (100.0 + 44.0 * 200.0 + 100.0)).abs() < 1e-6,
        "Box endet nicht am letzten Band: {bb:?}"
    );

    let t = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(s.doc.bbox());
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    assert!(
        ms < 20.0,
        "20 Abfragen der Zeichnungs-Box dauern {ms:.1} ms — wird das Band wieder gerechnet?"
    );
}

/// Die Trefferprüfung fragt nur Objekte, deren Ausdehnung den Zeiger erreicht.
///
/// Vorher rechnete `hit_test` für **alle** 45 Dämmungslinien das Band
/// (7,4 ms je Klick, bei jedem Hover), obwohl der Zeiger nur eine berühren
/// kann. Die Gegenprobe ist die Vorprüfung über die Box in `hit_test_opt`.
#[test]
fn trefferpruefung_der_daemmung_bleibt_guenstig() {
    let mut s = session();
    for i in 0..45 {
        let y = 100.0 + i as f64 * 200.0;
        daemmungslinie(&mut s, [0.0, y], [10000.0, y]);
    }
    // Ein Punkt **auf dem gezeichneten Band** der 21. Linie muss sie treffen.
    // Die Achse selbst taugt nicht: die Schlaufe läuft zurück und lässt dort
    // Lücken (T0120).
    let ziel = s.doc.ents()[20].clone();
    let punkte = band_punkte(&s.doc, &ziel);
    let auf_dem_band = punkte[punkte.len() / 2];
    let treffer = jjcad_core::snap::hit_test(&s.doc, auf_dem_band, 12.0, None);
    assert_eq!(treffer, Some(ziel.id), "Dämmungslinie nicht getroffen");

    // Weit weg von jeder Linie: kein Treffer, und das schnell.
    let t = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(jjcad_core::snap::hit_test(
            &s.doc,
            Vec2::new(5000.0, 20000.0),
            12.0,
            None,
        ));
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    assert!(
        ms < 40.0,
        "20 Trefferprüfungen dauern {ms:.1} ms — wird für jede Linie das Band gerechnet?"
    );
}

/// Der Fang fragt die Box je Objekt — auch er darf das Band nicht rechnen.
#[test]
fn fang_der_daemmung_bleibt_guenstig() {
    let mut s = session();
    for i in 0..45 {
        let y = 100.0 + i as f64 * 200.0;
        daemmungslinie(&mut s, [0.0, y], [10000.0, y]);
    }
    let p = Vec2::new(5000.0, 4100.0);
    let t = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(jjcad_core::snap::snap_candidates(&s.doc, p, 12.0, None, &[], &[]));
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    assert!(
        ms < 40.0,
        "20 Fangabfragen dauern {ms:.1} ms — wird die Box je Linie aus dem Band gerechnet?"
    );
    // Und der Fang findet weiterhin die Linie unter dem Zeiger.
    let cands = jjcad_core::snap::snap_candidates(&s.doc, p, 12.0, None, &[], &[]);
    assert!(!cands.is_empty(), "kein Fangpunkt auf der Dämmungslinie");
}

/// Die Box einer Dämmungslinie wird nicht aus dem gerenderten Band gebaut —
/// Gegenprobe zur Zeitmessung: die Anzeigeliste der Linie hat viele Primitiva,
/// die Box entsteht trotzdem sofort.
#[test]
fn box_der_daemmungslinie_ist_sofort_da() {
    let mut s = session();
    let e = daemmungslinie(&mut s, [0.0, 0.0], [10000.0, 0.0]);
    let ctx = RenderCtx::for_entity(&s.doc, &e);
    let mut dl = render::DisplayList::default();
    render::render_entity(&ctx, &e, jjcad_core::geom::Affine::IDENTITY, &mut dl, 0);
    assert_eq!(umriss_punkte(&dl.prims), 1, "Band ist ein Zug");
    if let Shape::Poly(pts, _) = &dl.prims[0].shape {
        assert!(pts.len() > 1000, "Band hat nur {} Punkte", pts.len());
    }
    let bb = render::entity_bbox(&s.doc, &e);
    assert!(bb.is_valid());
}
