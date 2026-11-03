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
