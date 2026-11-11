//! Schraffuren nach DIN 1356-1:2026-09, Tabelle 5.
//!
//! Die Norm zeichnet die Schnittflächen von 18 Baustoffen aus. Nr. 14
//! (Dämmung) ist **beides**: Linienart `LineType::Insulation` für die Kante und
//! seit T0136 ein Muster aus parallelen Schlaufenbändern; Nr. 15 (Abdichtungen)
//! bleibt eine reine Linienart (T0132). Der Test hält drei Dinge fest:
//!
//! * **Vollständigkeit** — jede Zeile der Tabelle hat ein Muster, und die
//!   vorhandenen Muster tragen den Namen ihrer Zeile (T0095: „passe nur den
//!   Namen an“).
//! * **Lesbarkeit/Schreibbarkeit** — Schlüssel und Bezeichnung finden das
//!   Muster wieder, die Serde-Namen der alten Dateien bleiben gültig.
//! * **Anzeige** — jedes Muster erzeugt Primitive, bleibt in der Umgrenzung
//!   und sieht anders aus als jedes andere (kein Muster fällt still auf
//!   „Vollfläche“ oder auf ein anderes zurück).

use jjcad_core::geom::{BBox, Vec2};
use jjcad_core::render::{render_document, DisplayList, Prim, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::FillPattern;
use serde_json::json;

/// Die Bezeichnungen der Tabelle 5, Zeile für Zeile — ohne Nr. 15
/// (Abdichtungen): die zeichnet eine Linienart, kein Muster. Nr. 14 heißt im
/// Programm „Dämmung“ (so wie die Linienart, GLOSSAR) statt „Dämmstoffe“.
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
    "Dämmung",               // 14
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
        "Vollfläche + 17 Muster der Tabelle 5"
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
    assert_eq!(FillPattern::from_key("nonsens"), None);
}

/// Die fünf Muster, die eigentlich Linienarten sind, gibt es nicht mehr —
/// weder in der Auswahl noch über einen Action-Schlüssel (T0132).
#[test]
fn entfernte_muster_sind_verschwunden() {
    const WEG: &[&str] = &[
        "Dämmstoffe",
        "Dämmstoffe Schlaufen",
        "Abdichtung 1-lagig",
        "Abdichtung 2-lagig",
        "Abdichtung Aufkantung",
    ];
    for name in WEG {
        assert!(
            !FillPattern::ALL.iter().any(|p| p.label() == *name),
            "„{name}“ steht noch in der Musterauswahl"
        );
    }
    // Auch die Schlüssel der Action-Schicht sind weg — sie liefern `None`,
    // damit kein Aufruf still ein anderes Muster bekommt.
    for key in [
        "daemmstoffe",
        "insulation",
        "daemmung_schlaufen",
        "abdichtung_einlagig",
        "abdichtung_mehrlagig",
        "abdichtung_aufkantung",
    ] {
        assert_eq!(
            FillPattern::from_key(key),
            None,
            "Schlüssel „{key}“ liefert noch ein Muster"
        );
    }
    assert_eq!(FillPattern::ALL.len(), 18, "Vollfläche + 17 Muster");
}

#[test]
fn gespeicherte_namen_bleiben_lesbar() {
    // Alte `.jjcad`-Dateien tragen die Serde-Namen; sie müssen weiter laden.
    for (roh, erwartet) in [
        ("\"Solid\"", FillPattern::Solid),
        ("\"Lines\"", FillPattern::Lines),
        ("\"Cross\"", FillPattern::Cross),
        ("\"Dots\"", FillPattern::Dots),
        ("\"Glass\"", FillPattern::Glass),
        ("\"ConcreteReinforced\"", FillPattern::ConcreteReinforced),
        // Die 2026 entfernten Muster laden als Mauerwerk — eine Zeichnung mit
        // ihnen muss weiter aufgehen (T0132).
        ("\"Insulation\"", FillPattern::Lines),
        ("\"InsulationLoops\"", FillPattern::Lines),
        ("\"SealSingle\"", FillPattern::Lines),
        ("\"SealMulti\"", FillPattern::Lines),
        ("\"SealUpstand\"", FillPattern::Lines),
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
    assert_eq!(gesehen.len(), TABELLE_5.len(), "17 Muster geprüft");
}

// ---------------------------------------------------------------------------
// Globales Muster-Raster (T0095, Turn 2)
// ---------------------------------------------------------------------------

/// Fläche mit einem Muster füllen und ihre Displaylisten-Primitive liefern.
/// `rechteck` = (x0, y0, x1, y1) in mm.
fn prims(rechteck: [f64; 4], p: FillPattern) -> Vec<Prim> {
    prims_mit_winkel(rechteck, p, None)
}

/// Wie `prims`, aber mit festem Musterwinkel (Grad) — für Muster, deren Aufbau
/// nur im eigenen Raster messbar ist.
fn prims_mit_winkel(rechteck: [f64; 4], p: FillPattern, winkel: Option<f64>) -> Vec<Prim> {
    let [x0, y0, x1, y1] = rechteck;
    let kontur = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "mm" }))
        .expect("Eingabe in Millimetern");
    let mut auftrag = json!({
        "boundary": kontur,
        "pattern": p.key(),
        "scale": 100.0,
        "border": false,
        "layer": "Wände"
    });
    if let Some(w) = winkel {
        auftrag["angle"] = json!(w);
    }
    let a = s
        .execute("draw.fill.points", &auftrag)
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
    // Kies (Kreise) und Textilbeton (Pfeile) sitzen in Rasterzellen, nicht an
    // der Bounding-Box der Fläche.
    let aussen = [0.0, 0.0, 1400.0, 700.0];
    let innen = [300.0, 100.0, 1100.0, 600.0];
    for p in [FillPattern::Gravel, FillPattern::TextileConcrete] {
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

#[test]
fn unterbrechungen_liegen_auf_waagerechten_linien() {
    // T0095, Turn 4: Die Lücken der Striche müssen auf **waagerechten** Linien
    // liegen (wie in der Norm). Vorher zählte der Takt längs der 45°-Linie —
    // dann wandert die Lücke von Reihe zu Reihe und bildet eine schiefe Naht.
    // Prüfung: die Höhen (y) der Strichanfänge und -enden werden von vielen
    // Strichen **geteilt**; bei längs gezähltem Takt hat fast jeder Strich
    // seine eigene Höhe.
    let [x0, y0, x1, y1] = [0.0, 0.0, 3200.0, 2200.0];
    for p in [
        FillPattern::ConcretePlain,
        FillPattern::ConcreteReinforced,
        FillPattern::ConcreteWatertight,
        FillPattern::Metal,
        FillPattern::Glass,
    ] {
        let mut anfang: std::collections::BTreeMap<i64, std::collections::BTreeSet<i64>> =
            Default::default();
        let mut ende: std::collections::BTreeMap<i64, std::collections::BTreeSet<i64>> =
            Default::default();
        let mut striche = 0;
        for pr in prims([x0, y0, x1, y1], p) {
            let Shape::Line(a, b) = pr.shape else { continue };
            let drin = |q: jjcad_core::geom::Vec2| {
                q.x > x0 + 1.0 && q.x < x1 - 1.0 && q.y > y0 + 1.0 && q.y < y1 - 1.0
            };
            if !drin(a) || !drin(b) {
                continue;
            }
            striche += 1;
            // Reihe über die Querkoordinate (45°-Muster: x + y ist konstant je Reihe)
            let reihe = ((a.x + a.y) / 10.0).round() as i64;
            anfang.entry(a.y.round() as i64).or_default().insert(reihe);
            ende.entry(b.y.round() as i64).or_default().insert(reihe);
        }
        assert!(striche > 20, "{}: zu wenige Striche ({striche})", p.label());
        for (was, karte) in [("Anfang", &anfang), ("Ende", &ende)] {
            assert!(
                karte.len() * 2 <= striche,
                "{}: {was}shöhen sind fast alle eigen ({}/{}) — keine waagerechte Linie",
                p.label(),
                karte.len(),
                striche
            );
            let geteilt = karte.values().filter(|r| r.len() >= 2).count();
            assert!(
                geteilt * 10 >= karte.len() * 8,
                "{}: nur {geteilt} von {} {was}shöhen liegen auf mehreren Reihen",
                p.label(),
                karte.len()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Die beiden Boden-Schraffuren (T0152, T0163, T0170)
// ---------------------------------------------------------------------------
//
// Beide stehen als Einheitszelle fest, die der Nutzer als Zeichnung geliefert
// hat; für die Schraffur wird die Zelle um 45° gedreht (der Musterwinkel einer
// Füllung ist +45°, wie bei „Mauerwerk“). Der Musterabstand `s` ist die
// Einheit der Zelle; die Tests messen am gezeichneten Bild, nicht an der
// Implementierung. „Boden unverändert“ kam in T0163, „Boden verändert“ in
// T0170 — vorher waren beide aus Screenshots abgeleitet und trafen das Raster
// nicht.

// ---------------------------------------------------------------------------
// Muster „Boden unverändert" (T0163)
// ---------------------------------------------------------------------------
//
// Der Nutzer hat die Einheitszelle als Zeichnung geliefert (`Boden
// Unverändert.jjcad`): 8 × 8 Einheiten bei 100 mm Musterabstand, rechtwinklig
// im Bezugssystem. Als Schraffur wird diese Zelle um 45° gedreht. Der Test
// hält beides fest — die Zelle selbst und die Drehung — und misst am
// gezeichneten Bild, nicht an der Implementierung.

/// Die Einheitszelle aus der Zeichnung des Nutzers, in Einheiten des
/// Musterabstands `s` (Zelle 8 × 8). Segmente, die über den Zellrand laufen,
/// stehen mit ihren fortgesetzten Koordinaten darin.
const ZELLE_BODEN_UNVERAENDERT: [(f64, f64, f64, f64); 24] = [
    // Vier durchgezogene Senkrechte über die ganze Zellhöhe.
    (0.0, 0.0, 0.0, 8.0),
    (2.0, 0.0, 2.0, 8.0),
    (4.0, 0.0, 4.0, 8.0),
    (6.0, 0.0, 6.0, 8.0),
    // Lange Waagerechte (4 s) auf geradem y, kurze (2 s) auf dem ungeraden y
    // darüber; beide um je 2 s nach links versetzt, wenn man nach oben geht.
    (6.0, 0.0, 10.0, 0.0),
    (6.0, 1.0, 8.0, 1.0),
    (4.0, 2.0, 8.0, 2.0),
    (4.0, 3.0, 6.0, 3.0),
    (2.0, 4.0, 6.0, 4.0),
    (2.0, 5.0, 4.0, 5.0),
    (0.0, 6.0, 4.0, 6.0),
    (0.0, 7.0, 2.0, 7.0),
    // Haken in S-Form: Stiel der Länge 4 s mit einem Fuß an jedem Ende, die
    // Füße zeigen in entgegengesetzte Richtungen.
    (1.0, 1.0, 2.0, 1.0),
    (1.0, 1.0, 1.0, 5.0),
    (0.0, 5.0, 1.0, 5.0),
    (3.0, 7.0, 4.0, 7.0),
    (3.0, 7.0, 3.0, 11.0),
    (2.0, 11.0, 3.0, 11.0),
    (5.0, 5.0, 6.0, 5.0),
    (5.0, 5.0, 5.0, 9.0),
    (4.0, 9.0, 5.0, 9.0),
    (7.0, 3.0, 8.0, 3.0),
    (7.0, 3.0, 7.0, 7.0),
    (6.0, 7.0, 7.0, 7.0),
];

/// Eine Zelle (in Einheiten von `s`), gekachelt und um `winkel` gedreht — so
/// muss das Muster aussehen. Die Kachelung läuft im gedrehten Raster
/// (Kantenlänge `kante` in Einheiten von `s`).
fn zelle_gekachelt(
    zelle: &[(f64, f64, f64, f64)],
    kante: f64,
    s: f64,
    winkel: f64,
) -> Vec<(Vec2, Vec2)> {
    let dir = Vec2::new(winkel.cos(), winkel.sin());
    let nrm = Vec2::new(-winkel.sin(), winkel.cos());
    let kante = kante * s;
    let mut out = Vec::new();
    // Reichlich über die geprüften Flächen hinaus (größte 9600 × 7200 mm).
    for i in -14..=14 {
        for j in -14..=14 {
            let (u0, v0) = (i as f64 * kante, j as f64 * kante);
            for (x1, y1, x2, y2) in zelle.iter().copied() {
                out.push((
                    dir * (u0 + x1 * s) + nrm * (v0 + y1 * s),
                    dir * (u0 + x2 * s) + nrm * (v0 + y2 * s),
                ));
            }
        }
    }
    out
}

/// Liegt `q` auf der Strecke `a`–`b` (Toleranz 0,01 mm)?
fn auf_strecke(q: Vec2, a: Vec2, b: Vec2) -> bool {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 < 1e-9 {
        return a.dist(q) < 0.01;
    }
    let t = (q - a).dot(ab) / l2;
    // Kleine Toleranz am Ende: `Raster` rechnet die Querachse über
    // `sin`/`cos`, ein Endpunkt kann deshalb ein Bit neben 0 oder 1 liegen.
    t >= -1e-6 && t <= 1.0 + 1e-6 && (a + ab * t).dist(q) < 0.01
}

/// Decken die gezeichneten Striche die Strecke `a`–`b` vollständig ab? Am
/// Flächenrand wird beschnitten, deshalb wird die Überdeckung geprüft und
/// nicht die Gleichheit der Endpunkte.
fn ueberdeckt(a: Vec2, b: Vec2, gezeichnet: &[(Vec2, Vec2)]) -> bool {
    let ab = b - a;
    let l = ab.len();
    if l < 1e-9 {
        return true;
    }
    let dir = ab * (1.0 / l);
    let mut intervalle: Vec<(f64, f64)> = Vec::new();
    for (p, q) in gezeichnet {
        let (tp, tq) = ((*p - a).dot(dir), (*q - a).dot(dir));
        if ((*p - a) - dir * tp).len() > 0.01 || ((*q - a) - dir * tq).len() > 0.01 {
            continue;
        }
        let (t0, t1) = (tp.min(tq), tp.max(tq));
        if t1 < 0.0 || t0 > l {
            continue;
        }
        intervalle.push((t0.max(0.0), t1.min(l)));
    }
    intervalle.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    let mut bis = 0.0;
    for (t0, t1) in intervalle {
        if t0 > bis + 0.01 {
            return false;
        }
        if t1 > bis {
            bis = t1;
        }
    }
    bis >= l - 0.01
}

/// Alle Musterlinien einer Füllfläche; `winkel` in Grad, `None` = Vorgabe.
fn striche(p: FillPattern, rechteck: [f64; 4], winkel: Option<f64>) -> Vec<(Vec2, Vec2)> {
    prims_mit_winkel(rechteck, p, winkel)
        .into_iter()
        .filter_map(|pr| match pr.shape {
            Shape::Line(a, b) => Some((a, b)),
            _ => None,
        })
        .collect()
}

/// Prüft, dass das gezeichnete Muster genau die Zelle (gekachelt, gedreht) ist:
/// jede gezeichnete Linie liegt auf einer Zelllinie, und jede Zelllinie im
/// Inneren der Fläche ist vollständig gezeichnet.
fn vergleiche_mit_zelle(
    gezeichnet: &[(Vec2, Vec2)],
    rechteck: [f64; 4],
    erwartet: &[(Vec2, Vec2)],
) {
    assert!(
        gezeichnet.len() > 100,
        "zu wenige Musterlinien ({})",
        gezeichnet.len()
    );
    for (a, b) in gezeichnet {
        assert!(
            erwartet
                .iter()
                .any(|(p, q)| auf_strecke(*a, *p, *q) && auf_strecke(*b, *p, *q)),
            "Linie ({:.1}; {:.1})–({:.1}; {:.1}) gehört nicht zur Zelle",
            a.x,
            a.y,
            b.x,
            b.y
        );
    }
    let drin = |q: Vec2| {
        q.x > rechteck[0] + 1.0
            && q.x < rechteck[2] - 1.0
            && q.y > rechteck[1] + 1.0
            && q.y < rechteck[3] - 1.0
    };
    let mut geprueft = 0;
    for (p, q) in erwartet {
        if !drin(*p) || !drin(*q) {
            continue;
        }
        assert!(
            ueberdeckt(*p, *q, gezeichnet),
            "Zelllinie ({:.1}; {:.1})–({:.1}; {:.1}) fehlt",
            p.x,
            p.y,
            q.x,
            q.y
        );
        geprueft += 1;
    }
    assert!(geprueft > 200, "zu wenige Zelllinien geprüft ({geprueft})");
}

#[test]
fn boden_unveraendert_ist_die_um_45_grad_gedrehte_einheitszelle() {
    let s = 100.0;
    let rechteck = [0.0, 0.0, 3200.0, 2400.0];

    // 1) Ohne Musterwinkel steht die Zelle rechtwinklig im Bezugssystem —
    //    wie in der Zeichnung des Nutzers, nur um 90° im Musterraster gedreht
    //    (siehe `muster_boden_unveraendert`).
    let rechtwinklig = striche(FillPattern::GroundPlain, rechteck, Some(0.0));
    for (a, b) in &rechtwinklig {
        let achsparallel = (a.y - b.y).abs() < 0.01 || (a.x - b.x).abs() < 0.01;
        assert!(
            achsparallel,
            "Linie nicht achsparallel: ({:.1}; {:.1})–({:.1}; {:.1})",
            a.x, a.y, b.x, b.y
        );
    }
    vergleiche_mit_zelle(
        &rechtwinklig,
        rechteck,
        &zelle_gekachelt(&ZELLE_BODEN_UNVERAENDERT, 8.0, s, -std::f64::consts::FRAC_PI_2),
    );

    // 2) Als Schraffur steht dieselbe Zelle um 45° gedreht: die Längsrichtung
    //    (durchgezogene Linien) läuft unter -45°, so zeigt es die Vorlage aus
    //    T0152. Der Vorgabe-Musterwinkel einer Füllung ist +45°.
    let schraeg = striche(FillPattern::GroundPlain, rechteck, None);
    for (a, b) in &schraeg {
        let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
        assert!(
            dx > 0.01 && (dx - dy).abs() < 0.01,
            "Linie nicht unter 45°: {dx:.1} × {dy:.1} mm"
        );
    }
    vergleiche_mit_zelle(
        &schraeg,
        rechteck,
        &zelle_gekachelt(&ZELLE_BODEN_UNVERAENDERT, 8.0, s, -std::f64::consts::FRAC_PI_4),
    );
}

// ---------------------------------------------------------------------------
// Muster „Boden verändert" (T0170)
// ---------------------------------------------------------------------------
//
// Der Nutzer hat die Einheitszelle als Zeichnung geliefert (`Boden
// verändert.jjcad`): ein Schachbrett aus Feldern von 6 × 6 Einheiten bei
// 100 mm Musterabstand, in jedem Feld drei Striche der Länge 6 s im Abstand
// 2 s — abwechselnd senkrecht und waagerecht. Als Schraffur steht diese Zelle
// um 45° gedreht. Der Test hält beides fest und prüft zusätzlich, dass sich
// die Striche nirgends kreuzen: der vorherige Stand zeichnete zwei freie
// Linienscharen, die sich überkreuzten — daher „sieht falsch aus“.

/// Die Einheitszelle aus der Zeichnung des Nutzers, in Einheiten des
/// Musterabstands `s` (Zelle 12 × 12, Felder 6 × 6).
const ZELLE_BODEN_VERAENDERT: [(f64, f64, f64, f64); 12] = [
    // Feld (0,0): drei Senkrechte über die ganze Feldhöhe.
    (1.0, 0.0, 1.0, 6.0),
    (3.0, 0.0, 3.0, 6.0),
    (5.0, 0.0, 5.0, 6.0),
    // Feld (1,0): drei Waagerechte über die ganze Feldbreite.
    (6.0, 1.0, 12.0, 1.0),
    (6.0, 3.0, 12.0, 3.0),
    (6.0, 5.0, 12.0, 5.0),
    // Feld (0,1): drei Waagerechte.
    (0.0, 7.0, 6.0, 7.0),
    (0.0, 9.0, 6.0, 9.0),
    (0.0, 11.0, 6.0, 11.0),
    // Feld (1,1): drei Senkrechte.
    (7.0, 6.0, 7.0, 12.0),
    (9.0, 6.0, 9.0, 12.0),
    (11.0, 6.0, 11.0, 12.0),
];

/// Kreuzen sich zwei Striche **im Inneren** (ein gemeinsamer Endpunkt zählt
/// nicht)?
fn kreuzt_sich(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
    let (p, p2) = a;
    let (q, q2) = b;
    let r = p2 - p;
    let s = q2 - q;
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-9 {
        return false; // parallel
    }
    let d = q - p;
    let t = (d.x * s.y - d.y * s.x) / den;
    let u = (d.x * r.y - d.y * r.x) / den;
    t > 1e-6 && t < 1.0 - 1e-6 && u > 1e-6 && u < 1.0 - 1e-6
}

#[test]
fn boden_veraendert_ist_die_um_45_grad_gedrehte_einheitszelle() {
    let s = 100.0;
    let rechteck = [0.0, 0.0, 9600.0, 7200.0];

    // 1) Ohne Musterwinkel steht die Zelle rechtwinklig im Bezugssystem — wie
    //    in der Zeichnung des Nutzers.
    let rechtwinklig = striche(FillPattern::GroundChanged, rechteck, Some(0.0));
    for (a, b) in &rechtwinklig {
        let achsparallel = (a.y - b.y).abs() < 0.01 || (a.x - b.x).abs() < 0.01;
        assert!(
            achsparallel,
            "Linie nicht achsparallel: ({:.1}; {:.1})–({:.1}; {:.1})",
            a.x, a.y, b.x, b.y
        );
    }
    vergleiche_mit_zelle(
        &rechtwinklig,
        rechteck,
        &zelle_gekachelt(&ZELLE_BODEN_VERAENDERT, 12.0, s, 0.0),
    );

    // 2) Als Schraffur steht dieselbe Zelle um 45° gedreht.
    let schraeg = striche(FillPattern::GroundChanged, rechteck, None);
    for (a, b) in &schraeg {
        let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
        assert!(
            dx > 0.01 && (dx - dy).abs() < 0.01,
            "Linie nicht unter 45°: {dx:.1} × {dy:.1} mm"
        );
    }
    vergleiche_mit_zelle(
        &schraeg,
        rechteck,
        &zelle_gekachelt(&ZELLE_BODEN_VERAENDERT, 12.0, s, std::f64::consts::FRAC_PI_4),
    );

    // 3) Kein Strich kreuzt einen anderen — der vorherige Stand tat es.
    //    Gegenprobe der Prüfung selbst: ein Kreuz kreuzt sich, zwei aneinander
    //    stoßende Striche nicht.
    assert!(kreuzt_sich(
        (Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
        (Vec2::new(0.0, 10.0), Vec2::new(10.0, 0.0))
    ));
    assert!(!kreuzt_sich(
        (Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
        (Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0))
    ));
    for i in 0..schraeg.len() {
        for j in i + 1..schraeg.len() {
            let (a, b) = schraeg[i];
            let (c, d) = schraeg[j];
            assert!(
                !kreuzt_sich((a, b), (c, d)),
                "Striche kreuzen sich: ({:.1}; {:.1})–({:.1}; {:.1}) und \
                 ({:.1}; {:.1})–({:.1}; {:.1})",
                a.x, a.y, b.x, b.y, c.x, c.y, d.x, d.y
            );
        }
    }
}
