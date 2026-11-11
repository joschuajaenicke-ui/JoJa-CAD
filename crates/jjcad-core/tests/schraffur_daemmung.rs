//! Schraffur Dämmung (T0136): parallele Schlaufenbänder mit **fester**
//! Schlaufengröße.
//!
//! Die Linienart „Dämmung“ rechnet die Periodenlänge aus der Länge der Linie
//! (eine runde Anzahl Schlaufen — die Form wird dafür leicht gestaucht oder
//! gestreckt). In einer Schraffur liegen viele Bänder nebeneinander, und jedes
//! endet an einer anderen Kante: mit der gestauchten Kurve stünde in jeder
//! Reihe ein anderes Schlaufenmaß, und die Schlaufen stünden nicht übereinander.
//!
//! Der Test hält drei Dinge fest:
//!
//! * **Feste Schlaufengröße** — die Stützpunkte aller Bänder liegen auf dem
//!   selben Raster (`p = r · H`, Phase am Weltnullpunkt), unabhängig von der
//!   Länge des beschnittenen Stücks und von der Form der Fläche.
//! * **Parallele Bänder** — die Bänder sind Verschiebungen derselben Kurve um
//!   den Musterabstand; die Kurve bleibt in ihrer Bandhöhe (±H/2).
//! * **Beschnitt statt Streckung** — an einer schrägen Kante hört das Muster
//!   auf, die Schlaufe wird nicht kleiner.

use jjcad_core::daemmung;
use jjcad_core::geom::Vec2;
use jjcad_core::render::{render_document, Prim, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::FillPattern;
use serde_json::json;

/// Musterabstand = Dämmstärke H (mm).
const H: f64 = 100.0;

/// Fläche füllen und ihre Displaylisten-Primitive liefern.
fn fuellung(kontur: &[[f64; 2]]) -> Vec<Prim> {
    fuellung_mit(kontur, H)
}

/// Wie [`fuellung`], mit eigenem Musterabstand (= Dämmstärke H).
fn fuellung_mit(kontur: &[[f64; 2]], h: f64) -> Vec<Prim> {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .expect("Eingabe in Millimetern");
    let a = s
        .execute(
            "draw.fill.points",
            &json!({
                "boundary": kontur,
                "pattern": FillPattern::InsulationHatch.key(),
                "angle": 0.0,
                "scale": h,
                "border": false,
                // Die Ebene „Schraffur“ ist hell; für die Rechnung ist die
                // Farbe egal, die dunkle Ebene hält die Prims aber prüfbar.
                "layer": "Wände"
            }),
        )
        .unwrap_or_else(|e| panic!("Füllfläche: {e}"));
    let id = a["created"][0].as_u64().expect("Id");
    render_document(&RenderCtx::model(&s.doc))
        .prims
        .into_iter()
        .filter(|p| p.entity == id)
        .collect()
}

fn rechteck(breite: f64, hoehe: f64) -> Vec<[f64; 2]> {
    vec![[0.0, 0.0], [breite, 0.0], [breite, hoehe], [0.0, hoehe]]
}

/// Das Stützpunktraster der Kurve bei fester Schlaufengröße.
///
/// `u(t) = p/(2π)·(t + K·sin 2t)` mit `t = j · 2π/32` — dieselbe Rechnung wie
/// [`daemmung::band_fest`], aber hier aus der Formel der Dokumentation, damit
/// der Test nicht von selbst nachrechnet, was er prüfen soll.
fn raster(p: f64) -> Vec<f64> {
    let a = p / std::f64::consts::TAU;
    (-80..3200)
        .map(|j| {
            let t = j as f64 * std::f64::consts::TAU / 32.0;
            a * (t + daemmung::K * (2.0 * t).sin())
        })
        .collect()
}

/// Liegt `x` auf dem Raster (Toleranz 0,05 mm)?
fn auf_dem_raster(x: f64, raster: &[f64]) -> bool {
    raster.iter().any(|u| (u - x).abs() < 0.05)
}

/// Die gezeichneten Strecken des Musters.
///
/// Die Dämmung entsteht als **Polygonzug** je Reihe (T0164): eine eigene
/// Strecke je Stützpunkt ergab bei 100 m² über 140 000 Primitiva und kostete
/// die Anzeige ein Vielfaches. Die Proben hier gelten der Kurve, nicht der
/// Form des Primitivs — beide Formen werden deshalb zu Strecken zerlegt.
fn strecken(prims: &[Prim]) -> Vec<(Vec2, Vec2)> {
    let mut out = Vec::new();
    for pr in prims {
        match &pr.shape {
            Shape::Line(a, b) => out.push((*a, *b)),
            Shape::Poly(pts, closed) => {
                if pts.len() < 2 {
                    continue;
                }
                let kanten = if *closed { pts.len() } else { pts.len() - 1 };
                for i in 0..kanten {
                    out.push((pts[i], pts[(i + 1) % pts.len()]));
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn muster_ist_das_daemmungsmuster() {
    // Der Schlüssel der Befehlsschicht findet dasselbe Muster wie die Auswahl
    // im Eigenschaften-Panel.
    assert_eq!(
        FillPattern::from_key("daemmung"),
        Some(FillPattern::InsulationHatch)
    );
    assert_eq!(
        FillPattern::from_key("Dämmung"),
        Some(FillPattern::InsulationHatch)
    );
    assert_eq!(FillPattern::InsulationHatch.label(), "Dämmung");
    // Die Linienart bleibt im Dateiformat getrennt (das ist kein Muster).
    assert_eq!(
        FillPattern::from_key("insulation"),
        None,
        "„insulation“ ist die Linienart, nicht das Muster"
    );
}

#[test]
fn schlaufenmass_haengt_nicht_an_der_laenge() {
    // Drei Breiten, alle mit demselben Muster: die Stützpunkte müssen auf
    // **einem** Raster liegen. Mit der gestauchten Linienart-Kurve bekäme jede
    // Breite ihre eigene Periode (830 mm ergäbe 50,3 mm statt 50).
    let p = daemmung::periode(H, daemmung::FORM);
    assert!((p - 50.0).abs() < 1e-9, "Periode {p} statt 50 mm");
    let r = raster(p);

    for breite in [830.0, 2400.0, 5000.0] {
        let prims = fuellung(&rechteck(breite, 300.0));
        assert!(!prims.is_empty(), "Breite {breite}: nichts gezeichnet");
        let mut geprueft = 0;
        for (a, b) in strecken(&prims) {
            for q in [a, b] {
                // Nur Stützpunkte im Inneren: am Rand schneidet die Kontur.
                if q.y < 2.0 || q.y > 298.0 || q.x < 1.0 || q.x > breite - 1.0 {
                    continue;
                }
                assert!(
                    auf_dem_raster(q.x, &r),
                    "Breite {breite}: Stützpunkt bei x = {:.3} mm liegt nicht auf dem festen Schlaufenraster",
                    q.x
                );
                geprueft += 1;
            }
        }
        assert!(geprueft > 100, "Breite {breite}: zu wenige Stützpunkte");
    }
}

#[test]
fn baender_sind_parallel_und_gleichphasig() {
    // Schräge Fläche: jede Reihe wird anders beschnitten — und trotzdem müssen
    // alle Bänder dieselben Schlaufenlagen zeigen (Phase am Weltnullpunkt).
    let kontur = [[0.0, 0.0], [3200.0, 0.0], [0.0, 3000.0]];
    let prims = fuellung(&kontur);
    assert!(!prims.is_empty(), "Dreieck: nichts gezeichnet");

    let p = daemmung::periode(H, daemmung::FORM);
    let r = raster(p);

    // Je Reihe die Stützpunkte sammeln (Reihe = Bandhöhe über y).
    let mut reihen: std::collections::BTreeMap<i64, Vec<f64>> = Default::default();
    for (a, b) in strecken(&prims) {
        for q in [a, b] {
            if q.y < 2.0 || q.y > 2998.0 || q.x < 1.0 {
                continue;
            }
            // Punkte genau auf der Bandgrenze gehören zu zwei Reihen — für die
            // Rasterprobe reicht die Zuordnung über die Bandmitte.
            let reihe = (q.y / H - 0.5).round() as i64;
            reihen.entry(reihe).or_default().push(q.x);
        }
    }
    assert!(reihen.len() >= 4, "zu wenige Reihen: {}", reihen.len());
    for (reihe, xs) in &reihen {
        // Jede Reihe muss Stützpunkte in ihrem Raster haben — sonst hat sie
        // eine eigene Periode (gestreckt).
        let treffer = xs.iter().filter(|x| auf_dem_raster(**x, &r)).count();
        assert!(
            treffer * 10 >= xs.len() * 9,
            "Reihe {reihe}: nur {treffer} von {} Stützpunkten auf dem festen Raster",
            xs.len()
        );
    }
}

#[test]
fn baender_bleiben_in_ihrer_bandhoehe() {
    // Ein Band schwingt zwischen ±H/2 um seine Mitte; die Bänder liegen um
    // genau H versetzt. Also liegen Scheitel und Täler auf y = k · H, und kein
    // Punkt verlässt die Fläche — auch wenn die Nachbarreihe beschnitten wird.
    let hoehe = 500.0;
    let prims = fuellung(&rechteck(2000.0, hoehe));
    let mut lagen: Vec<f64> = Vec::new();
    for (a, b) in strecken(&prims) {
        for q in [a, b] {
            lagen.push(q.y);
        }
    }
    assert!(!lagen.is_empty(), "nichts gezeichnet");
    let min = lagen.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = lagen.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        min < 1.0 && max > hoehe - 1.0,
        "Bänder füllen die Höhe nicht ({min}…{max})"
    );
    let mut extrema = 0;
    for y in &lagen {
        let k = (y / H).round();
        if (y - k * H).abs() < 1e-6 {
            extrema += 1;
        }
        assert!(*y > -1e-6 && *y < hoehe + 1e-6, "Punkt außerhalb: {y}");
    }
    assert!(extrema > 20, "nur {extrema} Extrema auf dem Bandraster");
}

#[test]
fn schraege_kante_beschneidet_statt_zu_stauchen() {
    // Die Schlaufe darf an einer schrägen Kante nicht zusammengedrückt werden:
    // im Dreieck ist die kürzeste Reihe oben nur wenige Millimeter lang, das
    // Schlaufenmaß bleibt trotzdem 50 mm.
    let p = daemmung::periode(H, daemmung::FORM);
    let prims = fuellung(&[[0.0, 0.0], [900.0, 0.0], [0.0, 900.0]]);
    assert!(!prims.is_empty(), "nichts gezeichnet");
    // Länge der gezeichneten Striche: ein gestauchtes Band hätte Striche, die
    // deutlich kürzer sind als p/24 · 0,5.
    let mut kurz = 0;
    let mut gesamt = 0;
    for (a, b) in strecken(&prims) {
        gesamt += 1;
        if a.dist(b) < p / 48.0 {
            kurz += 1;
        }
    }
    assert!(gesamt > 100, "zu wenige Striche ({gesamt})");
    // Sehr kurze Stücke entstehen nur am Beschnitt, nicht im Muster selbst.
    assert!(
        kurz * 100 <= gesamt * 25,
        "{kurz} von {gesamt} Strichen sind kürzer als {:.2} mm — Muster gestaucht?",
        p / 48.0
    );
}

/// Ein Polygonzug je Reihe statt einer Strecke je Stützpunkt (T0164).
///
/// 100 m² Dämmung mit H = 200 mm sind 46 Reihen. Vorher entstand daraus je
/// Stützpunkt eine eigene Strecke — 144 004 Primitiva, die die Anzeige einzeln
/// malen musste. Jetzt sind es rund 50 Züge mit derselben Kurve.
#[test]
fn daemmung_ist_ein_zug_je_reihe() {
    let (breite, hoehe, h) = (10000.0, 9000.0, 200.0);
    let prims = fuellung_mit(&rechteck(breite, hoehe), h);
    assert!(!prims.is_empty(), "nichts gezeichnet");

    let reihen = (hoehe / h).ceil() as usize + 2;
    let mut zuge = 0;
    let mut punkte = 0;
    for pr in &prims {
        match &pr.shape {
            Shape::Poly(pts, closed) => {
                assert!(!closed, "ein Musterzug ist offen");
                assert!(pts.len() > 1, "Zug mit einem Punkt");
                punkte += pts.len();
                zuge += 1;
            }
            // Die Fläche selbst (Füllung ohne Rand) — kein Musterstück.
            Shape::Fill { .. } | Shape::FillOrdered { .. } => {}
            _ => panic!("Musterstück ist weder Zug noch Fläche"),
        }
    }
    assert!(
        zuge <= 2 * reihen,
        "{zuge} Züge für {reihen} Reihen — Muster wieder je Strecke abgelegt?"
    );
    // Die Kurve bleibt vollständig: je Reihe 200 Schlaufen zu 32 Stützpunkten.
    assert!(
        punkte > 40 * (breite / (0.5 * h)) as usize,
        "zu wenige Stützpunkte: {punkte}"
    );
}

/// Die Bounding-Box einer Fläche kommt aus der Umgrenzung, nicht aus dem
/// gezeichneten Muster (T0164).
///
/// Vorher wurde für die Box das ganze Muster gerendert — bei 100 m²
/// Dämmungsschraffur rund 6 ms je Aufruf, und Trefferprüfung und Auswahl
/// fragen die Box mehrmals je Bild. Das Muster liegt immer **innerhalb** der
/// Kontur (es wird an ihr beschnitten), die Box ist also dieselbe.
#[test]
fn bbox_einer_schraffur_zeichnet_das_muster_nicht() {
    let kontur = rechteck(10000.0, 9000.0);
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .expect("Eingabe in Millimetern");
    let a = s
        .execute(
            "draw.fill.points",
            &json!({
                "boundary": kontur,
                "pattern": FillPattern::InsulationHatch.key(),
                "angle": 0.0,
                "scale": 200.0,
                "border": false,
                "layer": "Wände"
            }),
        )
        .expect("Füllfläche");
    let id = a["created"][0].as_u64().expect("Id");
    let e = s.doc.ents().iter().find(|e| e.id == id).expect("Objekt");

    let bb = jjcad_core::render::entity_bbox(&s.doc, e);
    assert!(bb.is_valid(), "keine Bounding-Box");
    assert!(
        (bb.min.x).abs() < 1e-9 && (bb.min.y).abs() < 1e-9,
        "Box beginnt nicht im Ursprung: {:?}",
        bb.min
    );
    assert!(
        (bb.max.x - 10000.0).abs() < 1e-9 && (bb.max.y - 9000.0).abs() < 1e-9,
        "Box endet nicht an der Kontur: {:?}",
        bb.max
    );

    let t = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(jjcad_core::render::entity_bbox(&s.doc, e));
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    assert!(
        ms < 20.0,
        "20 Abfragen der Box dauern {ms:.1} ms — wird das Muster mitgerendert?"
    );
}
