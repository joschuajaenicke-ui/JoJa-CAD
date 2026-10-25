//! Farbeimer-Kern (`boundary::detect_boundary`).
//!
//! Prüft die Abnahme der Karte F4 ohne GUI: Rechteck, Wandkontur, offene
//! Fläche, Klick auf der Linie, Lochkonturen, Haarriss-Toleranz, Maßhaltigkeit
//! am Wohnhaus-Plan und die Laufzeit.

use std::time::Instant;

use jjcad_core::boundary::{detect_boundary, BoundaryError, BoundaryOpts};
use jjcad_core::geom::{polygon_area, Vec2};
use jjcad_core::persist::load_document;
use jjcad_core::session::Session;
use serde_json::json;

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn leinwand() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

/// Linie von `a` nach `b` (Eingabe in mm, Action erwartet Meter).
fn linie(s: &mut Session, a: (f64, f64), b: (f64, f64)) {
    s.execute(
        "draw.line",
        &json!({"a": [a.0 / 1000.0, a.1 / 1000.0], "b": [b.0 / 1000.0, b.1 / 1000.0]}),
    )
    .expect("Linie");
}

/// Wand von `a` nach `b` (mm), Achsmaß, mittig.
fn wand(s: &mut Session, a: (f64, f64), b: (f64, f64), t: f64) {
    s.execute(
        "draw.wall",
        &json!({"a": [a.0 / 1000.0, a.1 / 1000.0],
                "b": [b.0 / 1000.0, b.1 / 1000.0],
                "thickness": t / 1000.0,
                "align": "center"}),
    )
    .expect("Wand");
}

fn rechteck(s: &mut Session, ecken: &[(f64, f64)]) {
    for i in 0..ecken.len() {
        linie(s, ecken[i], ecken[(i + 1) % ecken.len()]);
    }
}

/// Größter Abstand eines Polygonecks von den Soll-Ecken.
fn ecken_abweichung(ist: &[Vec2], soll: &[(f64, f64)]) -> f64 {
    soll.iter()
        .map(|(x, y)| {
            let p = v(*x, *y);
            ist.iter().map(|q| q.dist(p)).fold(f64::INFINITY, f64::min)
        })
        .fold(0.0, f64::max)
}

fn wohnhaus() -> jjcad_core::model::Document {
    let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/Wohnhaus.jjcad");
    load_document(&pfad).expect("Wohnhaus.jjcad")
}

// --- 1. Rechteck aus vier Linien -------------------------------------------

#[test]
fn rechteck_innen_ergibt_vier_eck_mit_richtiger_flaeche() {
    // 4000 × 3000 mm, Klick in der Mitte.
    let mut s = leinwand();
    let ecken = [(0.0, 0.0), (4000.0, 0.0), (4000.0, 3000.0), (0.0, 3000.0)];
    rechteck(&mut s, &ecken);

    let b = detect_boundary(&s.doc, v(2000.0, 1500.0), &BoundaryOpts::default()).expect("Fläche");
    assert_eq!(b.outer.len(), 4, "vier Ecken, sind {:?}", b.outer);
    assert!(b.holes.is_empty(), "keine Löcher");

    let soll = 4000.0 * 3000.0;
    let ist = b.area();
    assert!(
        (ist - soll).abs() / soll < 0.01,
        "Fläche {ist} mm² statt {soll} mm² ({} %)",
        (ist - soll).abs() / soll * 100.0
    );
    // Auf 0,5 mm je Pixel und halbe-Pixel-Versatz: Ecken müssen deutlich unter
    // einem Millimeter sitzen.
    let abw = ecken_abweichung(&b.outer, &ecken);
    assert!(abw <= 0.5, "Ecken weichen bis {abw} mm ab: {:?}", b.outer);

    // Die Fläche enthält den Klickpunkt.
    assert!(b.contains(v(2000.0, 1500.0)));
    // Orientierung: außen gegen den Uhrzeigersinn.
    assert!(polygon_area(&b.outer) > 0.0, "Außenkontur gegen den Uhrzeigersinn");
}

// --- 2. Zwei Wände im rechten Winkel ---------------------------------------

#[test]
fn zwei_waende_im_rechten_winkel_folgen_der_wandkontur() {
    // Innenraum zwischen zwei 240 mm dicken Wänden auf der Achse x = 0 bzw. y = 0.
    // Die Wandkörper reichen bis 120 mm an die Achse.
    let mut s = leinwand();
    wand(&mut s, (0.0, -2000.0), (0.0, 6000.0), 240.0);
    wand(&mut s, (-2000.0, 0.0), (6000.0, 0.0), 240.0);
    // Abschluss, damit die Fläche geschlossen ist.
    linie(&mut s, (6000.0, 0.0), (6000.0, 6000.0));
    linie(&mut s, (0.0, 6000.0), (6000.0, 6000.0));

    // Klick 1 m von der Ecke entfernt, im Innenraum.
    let b = detect_boundary(&s.doc, v(1000.0, 1000.0), &BoundaryOpts::default()).expect("Fläche");
    let flaeche = b.area();

    // Kein Punkt darf im Wandkörper liegen: dessen Innenfläche ist die Gerade
    // 120 mm. Erlaubt bleibt ein Pixel Toleranz.
    for p in &b.outer {
        assert!(
            p.x >= 119.0 && p.y >= 119.0,
            "Ecke {p:?} liegt in der Wand (erwartet >= 119 mm von der Achse)"
        );
    }
    // Die Kontur muss die Wandfläche treffen, nicht irgendwo im Freien liegen.
    let trifft_x = b.outer.iter().any(|p| (p.x - 120.0).abs() <= 1.0);
    let trifft_y = b.outer.iter().any(|p| (p.y - 120.0).abs() <= 1.0);
    assert!(trifft_x && trifft_y, "Kontur folgt der Wandfläche nicht: {:?}", b.outer);

    // Erwartete Fläche: (6000-120) × (6000-120).
    let soll = (6000.0 - 120.0) * (6000.0 - 120.0);
    assert!(
        (flaeche - soll).abs() / soll < 0.01,
        "Fläche {flaeche} mm² statt ca. {soll} mm²"
    );
    assert!(
        b.contains(v(3000.0, 3000.0)) && !b.contains(v(60.0, 3000.0)),
        "Fläche muss den Innenraum, nicht die Wand enthalten"
    );
    // Ein Rechteck, kein Treppenzug.
    assert_eq!(b.outer.len(), 4, "vier Ecken, sind {:?}", b.outer);
}

// --- 3. Offene Fläche ------------------------------------------------------

#[test]
fn offener_rand_meldet_open() {
    // Zwei Linien, die keine Fläche umschließen.
    let mut s = leinwand();
    linie(&mut s, (0.0, 0.0), (4000.0, 0.0));
    linie(&mut s, (4000.0, 0.0), (4000.0, 3000.0));

    let err = detect_boundary(&s.doc, v(2000.0, 1500.0), &BoundaryOpts::default()).unwrap_err();
    assert_eq!(err, BoundaryError::Open, "offene Fläche muss Open melden");
    assert_eq!(err.message(), "Fläche ist nicht geschlossen");
}

#[test]
fn leerzeichnung_ohne_linie_ist_offen() {
    let s = leinwand();
    assert_eq!(
        detect_boundary(&s.doc, v(0.0, 0.0), &BoundaryOpts::default()).unwrap_err(),
        BoundaryError::Open
    );
}

// --- 4. Klick auf einer Linie ---------------------------------------------

#[test]
fn klick_auf_linie_meldet_on_line() {
    let mut s = leinwand();
    let ecken = [(0.0, 0.0), (4000.0, 0.0), (4000.0, 3000.0), (0.0, 3000.0)];
    rechteck(&mut s, &ecken);

    // Genau auf der unteren Kante.
    let err = detect_boundary(&s.doc, v(2000.0, 0.0), &BoundaryOpts::default()).unwrap_err();
    assert_eq!(err, BoundaryError::OnLine, "Klick auf der Linie");

    // Auch auf einer Innenkante (rechte Seite).
    let err = detect_boundary(&s.doc, v(4000.0, 1500.0), &BoundaryOpts::default()).unwrap_err();
    assert_eq!(err, BoundaryError::OnLine);

    // Und im Wandkörper, nicht nur auf der Linie.
    let mut s = leinwand();
    wand(&mut s, (-2000.0, 0.0), (6000.0, 0.0), 240.0);
    let err = detect_boundary(&s.doc, v(1000.0, 0.0), &BoundaryOpts::default()).unwrap_err();
    assert_eq!(err, BoundaryError::OnLine, "Klick in der Wand");
}

// --- Löcher ---------------------------------------------------------------

#[test]
fn insel_im_raum_wird_als_loch_geliefert() {
    // 6000 × 4000 außen, 1000 × 1000 Insel innen.
    let mut s = leinwand();
    rechteck(
        &mut s,
        &[(0.0, 0.0), (6000.0, 0.0), (6000.0, 4000.0), (0.0, 4000.0)],
    );
    rechteck(
        &mut s,
        &[
            (2000.0, 1500.0),
            (3000.0, 1500.0),
            (3000.0, 2500.0),
            (2000.0, 2500.0),
        ],
    );

    let b = detect_boundary(&s.doc, v(5000.0, 3500.0), &BoundaryOpts::default()).expect("Fläche");
    assert_eq!(b.outer.len(), 4, "Außenkontur vier Ecken: {:?}", b.outer);
    assert_eq!(b.holes.len(), 1, "genau ein Loch: {:?}", b.holes);
    assert_eq!(b.holes[0].len(), 4, "Loch vier Ecken: {:?}", b.holes[0]);
    let loch = polygon_area(&b.holes[0]).abs();
    assert!(
        (loch - 1_000_000.0).abs() / 1_000_000.0 < 0.02,
        "Lochfläche {loch} mm² statt 1 m²"
    );
    // Loch entgegen der Außenkontur orientiert.
    assert!(polygon_area(&b.outer) > 0.0 && polygon_area(&b.holes[0]) < 0.0);
    let soll = 24_000_000.0 - 1_000_000.0;
    let ist = b.area();
    assert!(
        (ist - soll).abs() / soll < 0.02,
        "Fläche {ist} mm² statt {soll} mm²"
    );
    // In der Insel ist außen.
    assert!(!b.contains(v(2500.0, 2000.0)), "Insel darf nicht gefüllt sein");
    assert!(b.contains(v(5000.0, 3500.0)));
}

// --- Haarriss: Linien treffen sich nicht exakt -----------------------------

#[test]
fn haarriss_an_der_ecke_schliesst() {
    // Die untere Kante hat in der Mitte eine Lücke von 0,8 mm — so zeichnen
    // manche Pläne Türen und Fugen. Der Eimer muss die Fläche trotzdem füllen.
    let mut s = leinwand();
    linie(&mut s, (0.0, 0.0), (2000.0, 0.0));
    linie(&mut s, (2000.8, 0.0), (4000.0, 0.0));
    linie(&mut s, (4000.0, 0.0), (4000.0, 3000.0));
    linie(&mut s, (4000.0, 3000.0), (0.0, 3000.0));
    linie(&mut s, (0.0, 3000.0), (0.0, 0.0));

    let b = detect_boundary(&s.doc, v(2000.0, 1500.0), &BoundaryOpts::default())
        .expect("Haarriss darf die Fläche nicht öffnen");
    let soll = 4000.0 * 3000.0;
    assert!(
        (b.area() - soll).abs() / soll < 0.01,
        "Fläche {} mm² statt {soll} mm²",
        b.area()
    );
}

// --- Vorhandene Füllfläche zerteilt den Raum nicht -------------------------

#[test]
fn vorhandene_fuellflaeche_ist_keine_begrenzung() {
    let mut s = leinwand();
    rechteck(
        &mut s,
        &[(0.0, 0.0), (4000.0, 0.0), (4000.0, 3000.0), (0.0, 3000.0)],
    );
    // Raum ist bereits schraffiert.
    s.execute(
        "draw.fill.boundary",
        &json!({"boundary": [[0.5, 0.5], [3.5, 0.5], [3.5, 2.5], [0.5, 2.5]],
                "pattern": "lines", "pattern_scale": 0.2}),
    )
    .expect("Füllfläche");

    let b = detect_boundary(&s.doc, v(2000.0, 1500.0), &BoundaryOpts::default())
        .expect("Raum bleibt trotz Schraffur eine Fläche");
    assert_eq!(b.outer.len(), 4, "vier Ecken: {:?}", b.outer);
    let soll = 4000.0 * 3000.0;
    assert!(
        (b.area() - soll).abs() / soll < 0.01,
        "Fläche {} mm² statt {soll} mm²",
        b.area()
    );
}

// --- 5. Wohnhaus-Plan: Maßhaltigkeit, Form und Laufzeit --------------------

#[test]
fn wohnhaus_raum_ist_masshaltig_und_unter_200_ms() {
    let doc = wohnhaus();
    // Raum unten links, zwischen Außenwand (x=0, y=0) und Innenwänden.
    let klick = v(1500.0, 2600.0);

    // Einmal warmlaufen, dann messen — gemessen wird die reine Rechenzeit.
    let _ = detect_boundary(&doc, klick, &BoundaryOpts::default());
    let start = Instant::now();
    let ergebnis = detect_boundary(&doc, klick, &BoundaryOpts::default());
    let dauer = start.elapsed();
    let b = ergebnis.expect("Raum im Wohnhaus muss geschlossen sein");

    assert!(
        dauer.as_millis() < 200,
        "Laufzeit {dauer:?} überschreitet 200 ms"
    );

    // Die Innenfläche der Außenwand liegt bei 150 mm; die Kontur muss dort
    // sitzen, nicht irgendwo im Freien (Vorgabe der Folgekarte F5: ±0,5 mm).
    let bb = b.bbox();
    for (name, ist, soll) in [
        ("links", bb.min.x, 150.0),
        ("unten", bb.min.y, 150.0),
    ] {
        assert!(
            (ist - soll).abs() <= 0.5,
            "Kante {name} bei {ist} mm statt {soll} mm"
        );
    }
    assert!(
        b.area() > 25.0e6 && b.area() < 30.0e6,
        "Fläche {:.2} m² unplausibel",
        b.area() / 1e6
    );
    assert!(b.contains(klick));

    // Kein Treppenzug: der Raum hat einen Türbogen und zwei Inseln, bleibt aber
    // weit unter der Zahl, die eine ungefilterte Rasterkontur liefert (771).
    assert!(
        b.outer.len() <= 250,
        "Außenkontur hat {} Stützpunkte — Rasterzacken nicht vereinfacht",
        b.outer.len()
    );

    println!(
        "Wohnhaus: {:?}, {} Ecken, Fläche {:.2} m²",
        dauer,
        b.outer.len(),
        b.area() / 1e6
    );
}

#[test]
fn ergebnis_haengt_nicht_am_pixelbudget() {
    // Dasselbe Ergebnis bei grobem und feinem Budget: der Randstreifen wird
    // immer in Zielauflösung nachgerechnet.
    let doc = wohnhaus();
    let klick = v(1500.0, 2600.0);
    let grob = detect_boundary(
        &doc,
        klick,
        &BoundaryOpts {
            max_pixels: 20_000_000,
            ..Default::default()
        },
    )
    .expect("Fläche (grobes Budget)");
    let fein = detect_boundary(&doc, klick, &BoundaryOpts::default()).expect("Fläche (fein)");
    let (a, b) = (grob.area(), fein.area());
    assert!(
        (a - b).abs() / b.max(1.0) < 0.01,
        "Fläche hängt am Budget: {a} mm² gegen {b} mm²"
    );
}
