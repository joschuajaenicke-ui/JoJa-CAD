//! Schraffuren nach DIN 1356-1:2026-09, Tabelle 5.
//!
//! Die Norm zeichnet die Schnittflächen von 18 Baustoffen aus. Der Test hält
//! drei Dinge fest:
//!
//! * **Vollständigkeit** — jede Zeile der Tabelle hat ein Muster, und die
//!   vorhandenen Muster tragen den Namen ihrer Zeile (T0095: „passe nur den
//!   Namen an“).
//! * **Lesbarkeit/Schreibbarkeit** — Schlüssel und Bezeichnung finden das
//!   Muster wieder, die Serde-Namen der alten Dateien bleiben gültig.
//! * **Anzeige** — jedes Muster erzeugt Primitive, bleibt in der Umgrenzung
//!   und sieht anders aus als jedes andere (kein Muster fällt still auf
//!   „Vollfläche“ oder auf ein anderes zurück).

use jjcad_core::geom::BBox;
use jjcad_core::render::{render_document, DisplayList, Prim, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::FillPattern;
use serde_json::json;

/// Die Bezeichnungen der Tabelle 5, Zeile für Zeile (Nr. 1–18). Die Zeilen 14
/// und 15 führen zwei bzw. drei Formen — sie stehen als eigene Muster daneben.
const TABELLE_5: &[&str] = &[
    "Boden unverändert",     // 1
    "Boden verändert",       // 2
    "Kies",                  // 3
    "Sand",                  // 4
    "Beton unbewehrt",       // 5
    "Beton bewehrt",         // 6
    "WU-Beton",              // 7
    "Textilbeton",           // 8
    "Mauerwerk",             // 9
    "Vollholz quer",         // 10
    "Vollholz längs",        // 11
    "Metall",                // 12
    "Putz",                  // 13
    "Dämmstoffe",            // 14, Zickzack
    "Dämmstoffe Schlaufen",  // 14, Schlaufen
    "Abdichtung 1-lagig",    // 15, einlagig
    "Abdichtung 2-lagig",    // 15, mehrlagig
    "Abdichtung Aufkantung", // 15, mehrlagig mit Aufkantung
    "Dichtstoffe",           // 16
    "Kunststoffe",           // 17
    "Glas",                  // 18
];

fn schraffuren() -> Vec<FillPattern> {
    FillPattern::ALL
        .iter()
        .copied()
        .filter(|p| *p != FillPattern::Solid)
        .collect()
}

#[test]
fn alle_schraffuren_der_tabelle_5_sind_vorhanden() {
    for name in TABELLE_5 {
        assert!(
            FillPattern::ALL.iter().any(|p| p.label() == *name),
            "Tabelle 5: „{name}“ fehlt im Programm"
        );
    }
    // Keine Zeile doppelt benannt — „ein Begriff, eine Bedeutung“.
    for (i, a) in FillPattern::ALL.iter().enumerate() {
        for b in &FillPattern::ALL[i + 1..] {
            assert_ne!(a.label(), b.label(), "Bezeichnung doppelt vergeben");
            assert_ne!(a.key(), b.key(), "Schlüssel doppelt vergeben");
        }
    }
    assert_eq!(
        FillPattern::ALL.len(),
        TABELLE_5.len() + 1,
        "Vollfläche + 21 Muster der Tabelle 5"
    );
}

#[test]
fn schluessel_und_bezeichnung_finden_das_muster() {
    for p in FillPattern::ALL {
        assert_eq!(
            FillPattern::from_key(p.key()),
            Some(*p),
            "Schlüssel „{}“ findet sein Muster nicht",
            p.key()
        );
        assert_eq!(
            FillPattern::from_key(p.label()),
            Some(*p),
            "Bezeichnung „{}“ findet ihr Muster nicht",
            p.label()
        );
    }
    // Die Schlüssel der früheren Namen bleiben gültig (alte Dateien, MCP-
    // Aufrufe, Vorgaben in `prefs.json`).
    assert_eq!(FillPattern::from_key("lines"), Some(FillPattern::Lines));
    assert_eq!(FillPattern::from_key("cross"), Some(FillPattern::Cross));
    assert_eq!(FillPattern::from_key("dots"), Some(FillPattern::Dots));
    assert_eq!(
        FillPattern::from_key("insulation"),
        Some(FillPattern::Insulation)
    );
    assert_eq!(FillPattern::from_key("nonsens"), None);
}

#[test]
fn gespeicherte_namen_bleiben_lesbar() {
    // Alte `.jjcad`-Dateien tragen die Serde-Namen; sie müssen weiter laden.
    for (roh, erwartet) in [
        ("\"Solid\"", FillPattern::Solid),
        ("\"Lines\"", FillPattern::Lines),
        ("\"Cross\"", FillPattern::Cross),
        ("\"Dots\"", FillPattern::Dots),
        ("\"Insulation\"", FillPattern::Insulation),
        ("\"Glass\"", FillPattern::Glass),
        ("\"ConcreteReinforced\"", FillPattern::ConcreteReinforced),
        ("\"SealUpstand\"", FillPattern::SealUpstand),
    ] {
        let p: FillPattern = serde_json::from_str(roh).expect("alter Name muss laden");
        assert_eq!(p, erwartet, "{roh}");
    }
    for p in FillPattern::ALL {
        let text = serde_json::to_string(p).expect("Serde-Name");
        let zurueck: FillPattern = serde_json::from_str(&text).expect("Serde-Name laden");
        assert_eq!(*p, zurueck, "Rundlauf {text}");
    }
}

/// Fläche 3 m × 2 m mit dem Muster, Eingabe in Metern, Musterabstand 100 mm.
fn muster_durchgang(p: FillPattern) -> (DisplayList, BBox) {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .expect("Eingabe in Metern");
    let antwort = s
        .execute(
            "draw.fill.points",
            &json!({
                "boundary": [[0.0, 0.0], [3.0, 0.0], [3.0, 2.0], [0.0, 2.0]],
                "pattern": p.key(),
                "scale": 0.1,
                "border": false,
                "layer": "Wände"
            }),
        )
        .unwrap_or_else(|e| panic!("{}: Füllfläche: {e}", p.label()));
    let id = antwort["created"][0].as_u64().expect("Id");
    let dl = render_document(&RenderCtx::model(&s.doc));
    let eigene: Vec<Prim> = dl.prims.into_iter().filter(|pr| pr.entity == id).collect();
    let flaeche = BBox::new(
        jjcad_core::geom::v(0.0, 0.0),
        jjcad_core::geom::v(3000.0, 2000.0),
    );
    (DisplayList { bbox: flaeche, prims: eigene }, flaeche)
}

/// Kennzahl eines Durchgangs: Art und Menge der Primitive. Zwei Muster mit
/// gleicher Kennzahl sähen im Plan gleich aus.
fn kennzahl(dl: &DisplayList) -> (usize, usize, usize, usize, usize) {
    let mut linien = 0;
    let mut punkte = 0;
    let mut kreise = 0;
    let mut fuellungen = 0;
    let mut sonstige = 0;
    for p in &dl.prims {
        match p.shape {
            Shape::Line(..) | Shape::Poly(..) => linien += 1,
            Shape::Dot { .. } => punkte += 1,
            Shape::Circle { .. } | Shape::Ellipse { .. } => kreise += 1,
            Shape::Fill { .. } => fuellungen += 1,
            _ => sonstige += 1,
        }
    }
    (linien, punkte, kreise, fuellungen, sonstige)
}

#[test]
fn jedes_muster_zeichnet_in_seiner_umgrenzung() {
    for p in schraffuren() {
        let (dl, flaeche) = muster_durchgang(p);
        assert!(
            !dl.prims.is_empty(),
            "{}: Muster zeichnet nichts",
            p.label()
        );
        // Nichts ragt über die Umgrenzung hinaus (1 % Toleranz für die
        // Strichbreite an der Kante).
        let grenze = flaeche.expanded(30.0);
        for prim in &dl.prims {
            let bb = prim.bbox();
            assert!(
                grenze.contains_bbox(&bb),
                "{}: Musterstück liegt außerhalb ({:?})",
                p.label(),
                bb
            );
        }
    }
}

#[test]
fn jede_schraffur_sieht_anders_aus() {
    let mut gesehen: Vec<(&str, (usize, usize, usize, usize, usize))> = Vec::new();
    for p in schraffuren() {
        let (dl, _) = muster_durchgang(p);
        let k = kennzahl(&dl);
        if let Some((anderer, _)) = gesehen.iter().find(|(_, kk)| *kk == k) {
            panic!("„{}“ sieht aus wie „{anderer}“ (Kennzahl {k:?})", p.label());
        }
        gesehen.push((p.label(), k));
    }
    assert_eq!(gesehen.len(), TABELLE_5.len(), "21 Muster geprüft");
}

// ---------------------------------------------------------------------------
// Globales Muster-Raster (T0095, Turn 2)
// ---------------------------------------------------------------------------

/// Fläche mit einem Muster füllen und ihre Displaylisten-Primitive liefern.
/// `rechteck` = (x0, y0, x1, y1) in mm.
fn prims(rechteck: [f64; 4], p: FillPattern) -> Vec<Prim> {
    let [x0, y0, x1, y1] = rechteck;
    let kontur = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .expect("Eingabe in Millimetern");
    let a = s
        .execute(
            "draw.fill.points",
            &json!({
                "boundary": kontur,
                "pattern": p.key(),
                "scale": 100.0,
                "border": false,
                "layer": "Wände"
            }),
        )
        .unwrap_or_else(|e| panic!("{}: Füllfläche: {e}", p.label()));
    let id = a["created"][0].as_u64().expect("Id");
    render_document(&RenderCtx::model(&s.doc))
        .prims
        .into_iter()
        .filter(|pr| pr.entity == id)
        .collect()
}

/// Startpunkte aller Musterstriche, auf die Musterrichtung projiziert und auf
/// 5 mm gerundet (Toleranz gegen Rechenrauschen).
fn strich_start(p: FillPattern, kontur: [f64; 4]) -> Vec<i64> {
    let winkel = std::f64::consts::FRAC_PI_4;
    let dir = jjcad_core::geom::v(winkel.cos(), winkel.sin());
    let [x0, y0, x1, y1] = kontur;
    let drin = |q: jjcad_core::geom::Vec2| {
        q.x > x0 + 1.0 && q.x < x1 - 1.0 && q.y > y0 + 1.0 && q.y < y1 - 1.0
    };
    let mut out = Vec::new();
    for pr in prims(kontur, p) {
        if let Shape::Line(a, b) = pr.shape {
            // Nur Striche **innerhalb** der Fläche: am Rand schneidet die
            // Kontur das Muster ab, dort beginnt ein Strich an der Kante.
            if drin(a) && drin(b) {
                out.push((a.dot(dir) / 5.0).round() as i64);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn unterteilungen_haengen_am_globalen_raster() {
    // Gestrichelte Muster der Tabelle 5: Beton (unbewehrt/bewehrt), WU-Beton,
    // Metall und Glas. Zwei Flächen mit **verschiedenen** Rändern, dieselbe
    // Musterlage: die Striche der inneren Fläche müssen auf demselben Raster
    // liegen wie die der äußeren. Vor dem Umbau begann jede Linie ihr Muster
    // am Rand des beschnittenen Stücks — dann liegen die Striche überall.
    let aussen = [0.0, 0.0, 1400.0, 700.0];
    let innen = [300.0, 100.0, 1100.0, 600.0];
    for p in [
        FillPattern::ConcretePlain,
        FillPattern::ConcreteReinforced,
        FillPattern::ConcreteWatertight,
        FillPattern::Metal,
        FillPattern::Glass,
    ] {
        let gross = strich_start(p, aussen);
        let klein = strich_start(p, innen);
        assert!(!klein.is_empty(), "{}: kein Strich gezeichnet", p.label());
        for u in &klein {
            assert!(
                gross.contains(u),
                "{}: Strich bei u = {} mm liegt nicht auf dem Raster der großen Fläche",
                p.label(),
                u * 5
            );
        }
        // Und die Striche liegen nicht alle auf einer Höhe: das Muster ist
        // gestaffelt (sonst wäre es eine durchgehende Linie).
        assert!(
            gross.len() > 1,
            "{}: nur eine Strichlage — keine Unterteilung",
            p.label()
        );
    }
}

#[test]
fn figuren_sitzen_auf_dem_raster() {
    // Kies (Kreise), Dämmstoffe Schlaufen (Ellipsen) und Textilbeton (Pfeile)
    // sitzen in Rasterzellen, nicht an der Bounding-Box der Fläche.
    let aussen = [0.0, 0.0, 1400.0, 700.0];
    let innen = [300.0, 100.0, 1100.0, 600.0];
    for p in [
        FillPattern::Gravel,
        FillPattern::InsulationLoops,
        FillPattern::TextileConcrete,
    ] {
        let gross: Vec<(i64, i64)> = figuren(p, aussen);
        let klein: Vec<(i64, i64)> = figuren(p, innen);
        assert!(!klein.is_empty(), "{}: nichts gezeichnet", p.label());
        for k in &klein {
            assert!(
                gross.contains(k),
                "{}: Figur bei {k:?} liegt nicht auf dem Raster der großen Fläche",
                p.label()
            );
        }
    }
}

/// Figuren einer Fläche als gerundete Lagen (1 mm) — nur Primitive, die ganz
/// innerhalb liegen (am Rand wird das Muster beschnitten).
fn figuren(p: FillPattern, rechteck: [f64; 4]) -> Vec<(i64, i64)> {
    let [x0, y0, x1, y1] = rechteck;
    let mut out: Vec<(i64, i64)> = prims(rechteck, p)
        .iter()
        .filter(|pr| {
            let bb = pr.bbox();
            bb.min.x > x0 + 1.0 && bb.max.x < x1 - 1.0 && bb.min.y > y0 + 1.0 && bb.max.y < y1 - 1.0
        })
        .map(punkt_key)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Lage eines Primitivs als gerundeter Schlüssel (1 mm), damit zwei Flächen
/// verglichen werden können.
fn punkt_key(pr: &Prim) -> (i64, i64) {
    let p = match pr.shape {
        Shape::Circle { c, .. } => c,
        Shape::Ellipse { c, .. } => c,
        _ => pr.bbox().center(),
    };
    ((p.x).round() as i64, (p.y).round() as i64)
}


#[test]
fn unterteilungen_sind_in_jeder_reihe_gleichmaessig() {
    // Der eigentliche Vorwurf (T0095, Turn 2): die Unterteilungen einer Linie
    // lagen „mal alle auf einer Höhe und mal nicht“. Innerhalb einer Reihe
    // müssen die Abstände der Striche deshalb gleich sein — und über alle
    // Reihen derselbe Takt gelten.
    let dir = jjcad_core::geom::v(
        std::f64::consts::FRAC_PI_4.cos(),
        std::f64::consts::FRAC_PI_4.sin(),
    );
    let nrm = dir.perp();
    // Groß genug, dass auch lange Striche mehrere Unterteilungen je Reihe haben.
    let [x0, y0, x1, y1] = [0.0, 0.0, 3200.0, 2200.0];
    // Muster mit **einem** Strichpaar je Zyklus: dort muss der Abstand zweier
    // Striche in einer Reihe überall gleich sein. (Glas hat bewusst Gruppen
    // aus drei Strichen — dafür gilt der Rastertest oben.)
    for p in [
        FillPattern::ConcretePlain,
        FillPattern::ConcreteReinforced,
        FillPattern::ConcreteWatertight,
        FillPattern::Metal,
    ] {
        let mut reihen: std::collections::BTreeMap<i64, Vec<f64>> = Default::default();
        for pr in prims([x0, y0, x1, y1], p) {
            let Shape::Line(a, b) = pr.shape else { continue };
            let drin = |q: jjcad_core::geom::Vec2| {
                q.x > x0 + 1.0 && q.x < x1 - 1.0 && q.y > y0 + 1.0 && q.y < y1 - 1.0
            };
            if !drin(a) || !drin(b) {
                continue;
            }
            reihen.entry((a.dot(nrm)).round() as i64).or_default().push(a.dot(dir));
        }
        let mut takte = Vec::new();
        for (v, mut us) in reihen {
            if us.len() < 2 {
                continue;
            }
            us.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let diff: Vec<f64> = us.windows(2).map(|w| w[1] - w[0]).collect();
            let erste = diff[0];
            for d in &diff {
                assert!(
                    (d - erste).abs() < 0.5,
                    "{}: Reihe v={v} hat ungleiche Abstände ({d:.1} statt {erste:.1} mm)",
                    p.label()
                );
            }
            takte.push(erste);
        }
        assert!(takte.len() >= 2, "{}: zu wenige Reihen geprüft", p.label());
        let erster = takte[0];
        for t in &takte {
            assert!(
                (t - erster).abs() < 0.5,
                "{}: Reihen haben verschiedene Takte ({t:.1} statt {erster:.1} mm)",
                p.label()
            );
        }
    }
}
