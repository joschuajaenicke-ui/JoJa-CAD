//! Darstellung von Maßketten und Einzelmaßen nach DIN 1356-1:2026-09.
//!
//! Die Norm (§9.1, Bild 3) legt die Elemente der Bemaßung fest:
//! durchgezogene Maßlinie, Begrenzung durch Schrägstrich oder Punkt,
//! Maßhilfslinie mit Abstand zum Bauteil, Maßzahl **über** der Maßlinie.
//! Bild 3 zeichnet bei 2,5 mm Texthöhe einen Begrenzungsstrich von rund
//! 1,5 mm und denselben Überstand der Maßlinie über die äußere Maßhilfslinie
//! (0,6 h), die Maßzahl sitzt etwa 1,75 mm (0,7 h) über der Linie.
//!
//! **Abweichung seit T0227 (Turn 7)**: den Begrenzungsstrich stellt das
//! Punktsymbol der Maßkette (Block aus der Bibliothek, Papiergröße) — es sitzt
//! auf dem äußeren Basispunkt. Der Überstand der Maßlinie entfällt deshalb:
//! er gehörte zum Schrägstrich, der die Enden markierte.
//!
//! Geprüft wird an der Displayliste — das ist die eine Quelle für Anzeige,
//! PDF und PNG. Die Weltwerte sind Papier-Millimeter × Maßstab (hier 1:100).

use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use serde_json::json;

fn kette(pts: &[[f64; 2]], line_pos: [f64; 2], axis: &str) -> (Session, u64) {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .expect("Einheit");
    let r = s
        .execute(
            "dim.chain",
            // DIN-Bild 3 zeichnet die Maßhilfslinie — die Vorgabe des
            // Objekts ist seit T0226 aus, hier ausdrücklich an. Das
            // Punktsymbol der Messpunkte (T0227) ist abgeschaltet: hier zählen
            // nur die Linien der Bemaßung selbst.
            &json!({ "pts": pts, "line_pos": line_pos, "axis": axis,
                     "ext_lines": true, "symbol": "" }),
        )
        .expect("Maßkette");
    (s, r["created"][0].as_u64().expect("id"))
}

fn einzelmass() -> (Session, u64) {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({ "input_unit": "m" }))
        .expect("Einheit");
    let r = s
        .execute(
            "dim.linear",
            &json!({ "a": [0.0, 0.0], "b": [3.0, 0.0], "line_pos": [1.5, -0.6],
                     "ext_lines": true, "symbol": "" }),
        )
        .expect("Maß");
    (s, r["created"][0].as_u64().expect("id"))
}

/// Linien der Bemaßung, sortiert nach Länge (Welt-mm).
fn linien(s: &Session, id: u64) -> Vec<(f64, (f64, f64), (f64, f64))> {
    let dl = render_document(&RenderCtx::new(&s.doc));
    let mut out: Vec<(f64, (f64, f64), (f64, f64))> = dl
        .prims
        .iter()
        .filter(|p| p.entity == id)
        .filter_map(|p| match &p.shape {
            Shape::Line(a, b) => Some((a.dist(*b), (a.x, a.y), (b.x, b.y))),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    out
}

fn texte(s: &Session, id: u64) -> Vec<(f64, f64, f64, String)> {
    let dl = render_document(&RenderCtx::new(&s.doc));
    dl.prims
        .iter()
        .filter(|p| p.entity == id)
        .filter_map(|p| match &p.shape {
            Shape::Text(t) => Some((t.pos.x, t.pos.y, t.angle, t.text.clone())),
            _ => None,
        })
        .collect()
}

/// Maßlinie: durchgezogen **vom ersten bis zum letzten Basispunkt** — kein
/// Überstand mehr (T0227, Turn 7: den Abschluss markiert das Punktsymbol).
#[test]
fn masslinie_endet_am_aeusseren_basispunkt() {
    let (s, id) = kette(&[[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]], [0.0, -1.2], "x");
    let l = linien(&s, id);
    let masslinie = l.last().expect("Maßlinie");
    // Basispunkte 0…4000 mm, kein Überstand.
    assert!(
        (masslinie.0 - 4000.0).abs() < 0.5,
        "Maßlinie zu lang: Länge {}, Enden {:?} {:?}",
        masslinie.0,
        masslinie.1,
        masslinie.2
    );
    let x0 = masslinie.1 .0.min(masslinie.2 .0);
    let x1 = masslinie.1 .0.max(masslinie.2 .0);
    assert!(x0.abs() < 0.5, "Maßlinie beginnt zu früh: {x0}");
    assert!((x1 - 4000.0).abs() < 0.5, "Maßlinie endet zu spät: {x1}");
}

/// Die Marke der Maßkette ist das **Punktsymbol**, kein textabhängiger
/// Schrägstrich mehr: mit Symbol stehen an jedem Basispunkt dessen Linien,
/// ohne Symbol steht dort **gar nichts** (T0227, Turn 7).
#[test]
fn marke_ist_das_punktsymbol_oder_leer() {
    // Ohne Symbol: nur Maß- und Hilfslinien, nichts Kurzes an den Enden.
    let (s, id) = kette(&[[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]], [0.0, -1.2], "x");
    let kurz_ohne: Vec<f64> = linien(&s, id)
        .iter()
        .map(|e| e.0)
        .filter(|len| *len < 500.0)
        .collect();
    assert!(
        kurz_ohne.is_empty(),
        "ohne Symbol darf kein Begrenzungszeichen stehen: {kurz_ohne:?}"
    );

    // Mit der Vorgabe (Schrägstrich-Block): je Basispunkt zwei Achsenstriche
    // und der 45°-Strich, alle in der Papiergröße des Blocks.
    let mut s2 = Session::new();
    s2.execute("doc.settings", &json!({ "input_unit": "m" })).unwrap();
    let r = s2
        .execute(
            "dim.chain",
            &json!({ "pts": [[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]],
                     "line_pos": [0.0, -1.2], "axis": "x", "ext_lines": true }),
        )
        .expect("Maßkette");
    let id2 = r["created"][0].as_u64().expect("id");
    let l = linien(&s2, id2);
    let kurz: Vec<&(f64, (f64, f64), (f64, f64))> =
        l.iter().filter(|e| e.0 < 500.0).collect();
    // Zwei Abschnitte × zwei Enden × drei Striche des Symbols.
    assert_eq!(kurz.len(), 12, "Markenlinien: {kurz:?}");
    // Davon an jedem Abschnittsende einer unter 45° (gleiche Ausdehnung x/y):
    // in der Mitte decken sich die beiden Marken der Nachbarabschnitte.
    let schraeg = l
        .iter()
        .filter(|e| e.0 < 500.0)
        .filter(|e| {
            let dx = (e.2 .0 - e.1 .0).abs();
            let dy = (e.2 .1 - e.1 .1).abs();
            (dx - dy).abs() < 0.01
        })
        .count();
    assert_eq!(schraeg, 4, "ein 45°-Strich je Abschnittsende: {l:?}");
}

/// Ein Punkt zweier Segmente trägt **eine** Marke, nicht zwei übereinander.
#[test]
fn eine_marke_je_basispunkt() {
    let (s, id) = kette(&[[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]], [0.0, -1.2], "x");
    let l = linien(&s, id);
    // Eine Maßlinie + drei Hilfslinien, ohne Symbol keine Markenlinien.
    assert_eq!(l.len(), 4, "Linienzahl der Kette: {l:?}");
    // Punkt-Marken ebenso: drei Punkte, nicht vier.
    let mut s2 = s;
    s2.execute("dim.style", &json!({ "arrow": "dot" })).unwrap();
    let dl = render_document(&RenderCtx::new(&s2.doc));
    let punkte = dl
        .prims
        .iter()
        .filter(|p| p.entity == id)
        .filter(|p| matches!(p.shape, Shape::Dot { .. }))
        .count();
    assert_eq!(punkte, 3, "Punktmarken je Basispunkt, nicht je Segmentende");
}

/// Maßhilfslinien sind Volllinien **schmal** wie die Maßlinie (Tabelle 2,
/// 01.1.8 „Maßlinien und ihre Begrenzung“) — nicht dünner gezeichnet.
#[test]
fn hilfslinien_sind_so_breit_wie_die_masslinie() {
    let (s, id) = kette(&[[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]], [0.0, -1.2], "x");
    let dl = render_document(&RenderCtx::new(&s.doc));
    let breiten: Vec<f64> = dl
        .prims
        .iter()
        .filter(|p| p.entity == id)
        .filter(|p| matches!(p.shape, Shape::Line(..)))
        .map(|p| p.style.width)
        .collect();
    let min = breiten.iter().cloned().fold(f64::MAX, f64::min);
    let max = breiten.iter().cloned().fold(0.0, f64::max);
    assert!(
        (max - min).abs() < 0.001,
        "Maßlinie/Hilfslinie/Strich müssen eine Strichstärke haben: {breiten:?}"
    );
}

/// Maßzahl: mittig **über** der Maßlinie, Unterkante 0,7 h über der Linie.
#[test]
fn masszahl_steht_ueber_der_masslinie() {
    let (s, id) = kette(&[[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]], [0.0, -1.2], "x");
    let t = texte(&s, id);
    assert_eq!(t.len(), 2, "zwei Maßzahlen erwartet: {t:?}");
    for (x, y, angle, text) in &t {
        assert!((*angle).abs() < 1e-6, "waagerechte Kette bleibt waagerecht: {text}");
        // Maßlinie liegt auf y = -1200; Anker 0,6 × 2,5 mm × 100 = 150 mm
        // darüber.
        assert!(
            (y - (-1050.0)).abs() < 0.5,
            "Maßzahl „{text}“ sitzt nicht über der Maßlinie: y {y}"
        );
        assert!(*x > 0.0 && *x < 4000.0, "Maßzahl außerhalb der Kette: {x}");
    }
}

/// Senkrechte Kette: Maßzahl um 90° gedreht (**von rechts lesbar**) und links
/// der Maßlinie — „über der Maßlinie“ heißt in Leserichtung.
#[test]
fn senkrechte_kette_liest_von_rechts() {
    let (s, id) = kette(&[[0.0, 0.0], [0.0, 3.0]], [1.2, 1.5], "y");
    let t = texte(&s, id);
    assert_eq!(t.len(), 1, "eine Maßzahl erwartet: {t:?}");
    let (x, y, angle, text) = &t[0];
    assert!(
        (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
        "senkrechte Maßzahl muss um 90° gedreht sein: {angle}"
    );
    // Maßlinie auf x = +1200; Text links davon, 150 mm entfernt.
    assert!(
        (x - 1050.0).abs() < 0.5,
        "Maßzahl „{text}“ muss links der Maßlinie stehen: x {x}"
    );
    assert!((y - 1500.0).abs() < 50.0, "Maßzahl nicht mittig: y {y}");
}

/// Kurzer Abschnitt: die Maßzahl steht **trotzdem** mittig über der
/// Maßlinie — nicht seitlich daneben (T0251).
///
/// Der Abschnitt ist mit 395 mm schmaler als die Maßzahl „395“ (bei 2,5 mm
/// Texthöhe rund 435 mm). Früher wanderte der Text in diesem Fall hinter das
/// Segmentende, die Zahl stand neben der Kette statt darüber.
#[test]
fn kurzer_abschnitt_traegt_die_masszahl_ueber_der_linie() {
    let (s, id) = kette(&[[0.0, 0.0], [2.0, 0.0], [2.395, 0.0]], [0.0, -1.2], "x");
    let t = texte(&s, id);
    assert_eq!(t.len(), 2, "zwei Maßzahlen erwartet: {t:?}");
    // Zweite Maßzahl „395“: Mitte des Abschnitts 2000…2395, 150 mm über der
    // Maßlinie (y = −1200).
    let (x, y, _, text) = &t[1];
    assert!(
        (x - 2197.5).abs() < 0.5,
        "Maßzahl „{text}“ steht nicht mittig über ihrem Abschnitt: x {x}"
    );
    assert!(
        (y - (-1050.0)).abs() < 0.5,
        "Maßzahl „{text}“ sitzt nicht über der Maßlinie: y {y}"
    );
    assert!(
        *x >= 2000.0 && *x <= 2395.0,
        "Maßzahl „{text}“ steht neben der Maßkette: x {x}"
    );
}

/// Einzelmaß folgt denselben Regeln wie die Kette (eine Darstellung).
#[test]
fn einzelmass_folgt_denselben_regeln() {
    let (s, id) = einzelmass();
    let l = linien(&s, id);
    let masslinie = l.last().expect("Maßlinie");
    // Basispunkte 0…3000, kein Überstand (T0227, Turn 7).
    assert!(
        (masslinie.0 - 3000.0).abs() < 0.5,
        "Einzelmaß mit Überstand: {}",
        masslinie.0
    );
    let striche = l.iter().filter(|e| e.0 < 500.0).count();
    assert_eq!(striche, 0, "ohne Symbol keine Markenlinien am Einzelmaß");
    let t = texte(&s, id);
    assert_eq!(t.len(), 1);
    assert!(
        (t[0].1 - (-600.0 + 150.0)).abs() < 0.5,
        "Maßzahl nicht über der Maßlinie: y {}",
        t[0].1
    );
}

/// `text_gap` bleibt wirksam, wenn es **größer** als der DIN-Anteil ist —
/// der Normwert ist die Untergrenze, kein starrer Wert.
#[test]
fn groesserer_textabstand_bleibt_erhalten() {
    let (mut s, id) = kette(&[[0.0, 0.0], [2.0, 0.0]], [0.0, -1.2], "x");
    s.execute("dim.style", &json!({ "text_gap": 4.0 })).unwrap();
    let t = texte(&s, id);
    assert!(
        (t[0].1 - (-1200.0 + 400.0)).abs() < 0.5,
        "text_gap 4 mm muss 400 mm über der Maßlinie ergeben: y {}",
        t[0].1
    );
}

/// Maßzahl nach Tabelle 4: der halbe Zentimeter steht als **Hochzahl** im
/// zweiten Lauf des Textes („2,88“ + hochgestellte „5“).
#[test]
fn halber_zentimeter_steht_als_hochzahl() {
    let (s, id) = kette(&[[0.0, 0.0], [2.885, 0.0]], [0.0, -1.2], "x");
    let dl = render_document(&RenderCtx::new(&s.doc));
    let t = dl
        .prims
        .iter()
        .filter(|p| p.entity == id)
        .find_map(|p| match &p.shape {
            Shape::Text(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Maßzahl");
    assert_eq!(t.text, "2,885", "Hauptzahl samt Hochzahl: {}", t.text);
    assert_eq!(t.runs.len(), 2, "Hauptzahl und Hochzahl: {:?}", t.runs);
    assert_eq!(t.runs[0].text, "2,88");
    assert!(t.runs[0].superscript == false);
    assert_eq!(t.runs[1].text, "5");
    assert!(t.runs[1].superscript, "die 5 muss hochgestellt sein");
    // Exakte Werte bekommen keine Hochzahl
    let (s2, id2) = kette(&[[0.0, 0.0], [4.25, 0.0]], [0.0, -1.2], "x");
    let dl2 = render_document(&RenderCtx::new(&s2.doc));
    let t2 = dl2
        .prims
        .iter()
        .filter(|p| p.entity == id2)
        .find_map(|p| match &p.shape {
            Shape::Text(t) => Some(t.clone()),
            _ => None,
        })
        .expect("Maßzahl");
    assert_eq!(t2.text, "4,25");
    assert_eq!(t2.runs.len(), 1);
}

/// Maßhilfslinie: mit deutlichem Abstand am Maßpunkt (dort wird gekürzt) und
/// ein Stück über die Maßlinie hinaus (Wert aus `dim_style`).
#[test]
fn hilfslinien_sind_kurz() {
    let (s, id) = kette(&[[0.0, 0.0], [2.0, 0.0]], [0.0, -1.2], "x");
    let l = linien(&s, id);
    // Reine Hilfslinie: von y = -200 (Abstand 0,8 h = 2 mm) bis -1200 - 150.
    let hilfs = l
        .iter()
        .find(|e| (e.1 .0 - e.2 .0).abs() < 0.5 && e.0 > 500.0)
        .expect("Hilfslinie am Basispunkt");
    let (y0, y1) = (hilfs.1 .1.min(hilfs.2 .1), hilfs.1 .1.max(hilfs.2 .1));
    assert!(
        (y1 - (-200.0)).abs() < 0.5,
        "Abstand am Maßpunkt muss 2 mm sein, ist {y1}"
    );
    assert!(
        (y0 - (-1350.0)).abs() < 0.5,
        "Überstand über die Maßlinie muss 1,5 mm sein, ist {y0}"
    );
}
