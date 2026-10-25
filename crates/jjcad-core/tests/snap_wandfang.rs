//! Abnahmetests für den Punktfang der Wandkontur (Kanban t_bfd64dfa).
//!
//! Geprüft wird über die öffentliche Fang-Engine `snap::find_snap`, also über
//! denselben Weg, den die Zeichenfläche nimmt. Erwartete Fangpunkte werden aus
//! der tatsächlich gezeichneten Kontur (Displayliste) bzw. aus der Wandgeometrie
//! abgeleitet — nicht aus der Implementierung des Fangs.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::EntityKind;
use jjcad_core::render::{self, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::snap::{find_snap, SnapKind};
use serde_json::json;

// ---------------------------------------------------------------------------
// Hilfen
// ---------------------------------------------------------------------------

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    // Raster aus: sonst überlagert der Rasterfang die Messungen.
    s.execute("doc.settings", &json!({"snap": {"grid": false}}))
        .unwrap();
    s
}

/// Wand zeichnen (Koordinaten in Metern, Dicke in Metern).
fn wand(s: &mut Session, a: [f64; 2], b: [f64; 2], dicke: f64) -> u64 {
    s.execute(
        "draw.wall",
        &json!({"a": a, "b": b, "thickness": dicke}),
    )
    .unwrap();
    s.doc.entities.last().unwrap().id
}

/// Fang ohne Raster (die Zeichenfläche schaltet den Rasterfang genauso ab,
/// wenn das Raster nicht angezeigt wird).
fn fang(s: &Session, p: Vec2, tol: f64) -> Option<jjcad_core::snap::SnapHit> {
    find_snap(&s.doc, p, tol, None, &[])
}

/// Name des Schalterfeldes für den Wandkontur-Fang im `snap`-Objekt.
/// Wird aus den serialisierten Einstellungen ermittelt, damit der Test nicht an
/// einem Feldnamen klebt (geplant: `wall_edges`).
fn schalter_feld(s: &Session) -> String {
    let j = serde_json::to_value(&s.doc.settings.snap).expect("SnapSettings serialisierbar");
    let obj = j.as_object().expect("snap ist ein Objekt");
    let treffer: Vec<String> = obj
        .keys()
        .filter(|k| k.contains("wall"))
        .cloned()
        .collect();
    assert_eq!(
        treffer.len(),
        1,
        "genau ein Schalterfeld für den Wand-Punktfang erwartet, gefunden: {treffer:?}"
    );
    treffer[0].clone()
}

fn schalter(s: &Session) -> bool {
    let j = serde_json::to_value(&s.doc.settings.snap).unwrap();
    j[schalter_feld(s)].as_bool().expect("Schalter ist bool")
}

/// Schalter über die Action-Schicht setzen (`doc.settings`) und prüfen, dass er wirkt.
fn schalter_setzen(s: &mut Session, an: bool) {
    let feld = schalter_feld(s);
    let mut snap = serde_json::Map::new();
    snap.insert(feld.clone(), json!(an));
    let res = s.execute("doc.settings", &json!({ "snap": snap }));
    assert!(
        res.is_ok(),
        "Schalter {feld} muss über die Action-Schicht setzbar sein: {res:?}"
    );
    assert_eq!(
        schalter(s),
        an,
        "Schalter {feld} wurde über 'doc.settings' nicht gesetzt"
    );
}

fn assert_punkt(ist: Vec2, soll: Vec2, kontext: &str) {
    assert!(
        ist.dist(soll) < 0.01,
        "{kontext}: erwartet ({:.3}, {:.3}), gefangen ({:.3}, {:.3})",
        soll.x,
        soll.y,
        ist.x,
        ist.y
    );
}

/// Alle gezeichneten Wandkontur-Linien (Displayliste, ohne Schraffur) als Strecken.
fn kontur_linien(doc: &jjcad_core::model::Document, wall_id: u64) -> Vec<(Vec2, Vec2)> {
    let ctx = RenderCtx::new(doc);
    let mut out = Vec::new();
    for e in doc.entities.iter().filter(|e| e.id == wall_id) {
        let dl = render::render_entities(&ctx, std::slice::from_ref(e));
        for prim in &dl.prims {
            if let Shape::Line(a, b) = &prim.shape {
                out.push((*a, *b));
            }
        }
    }
    out
}

fn kontur_ecken(doc: &jjcad_core::model::Document, wall_id: u64) -> Vec<Vec2> {
    let mut ecken: Vec<Vec2> = Vec::new();
    for (a, b) in kontur_linien(doc, wall_id) {
        for p in [a, b] {
            if !ecken.iter().any(|q| q.dist(p) < 0.01) {
                ecken.push(p);
            }
        }
    }
    ecken
}

// ---------------------------------------------------------------------------
// 1. Wandkontur-Eckpunkte werden gefangen
// ---------------------------------------------------------------------------

#[test]
fn wandkontur_ecken_werden_gefangen() {
    let mut s = s();
    let id = wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    // Mittige Ausrichtung, Dicke 300 mm -> Ecken bei y = +-150
    let ecken = kontur_ecken(&s.doc, id);
    assert_eq!(ecken.len(), 4, "Wandkontur muss 4 Ecken haben: {ecken:?}");
    for ecke in ecken {
        let treffer = fang(&s, ecke + v(5.0, 3.0), 40.0)
            .unwrap_or_else(|| panic!("Kontur-Ecke ({}, {}) nicht fangbar", ecke.x, ecke.y));
        assert_punkt(treffer.point, ecke, "Kontur-Ecke");
    }
}

#[test]
fn wandkontur_ecken_bei_verschiedenen_dicken() {
    for dicke in [0.10, 0.175, 0.24, 0.365, 0.50] {
        let mut s = s();
        let id = wand(&mut s, [0.0, 0.0], [4.0, 0.0], dicke);
        let h = dicke * 1000.0 * 0.5;
        let erwartet = [
            v(0.0, h),
            v(0.0, -h),
            v(4000.0, h),
            v(4000.0, -h),
        ];
        for p in erwartet {
            let treffer = fang(&s, p + v(2.0, 2.0), 30.0).unwrap_or_else(|| {
                panic!("Dicke {dicke}: Kontur-Ecke ({}, {}) nicht fangbar", p.x, p.y)
            });
            assert_punkt(treffer.point, p, &format!("Dicke {dicke}"));
        }
        // Die gezeichnete Kontur muss dieselben Ecken haben (Anzeige = Fangziel).
        let gezeichnet = kontur_ecken(&s.doc, id);
        for p in erwartet {
            assert!(
                gezeichnet.iter().any(|q| q.dist(p) < 0.5),
                "Dicke {dicke}: Ecke ({}, {}) fehlt in der gezeichneten Kontur",
                p.x,
                p.y
            );
        }
    }
}

#[test]
fn wandkontur_ecken_bei_schraeger_wand() {
    // 45°-Wand: Ecken liegen senkrecht zur Achse um die halbe Dicke versetzt.
    let mut s = s();
    let id = wand(&mut s, [0.0, 0.0], [3.0, 3.0], 0.24);
    let n = v(1.0, 1.0).normalized().perp();
    let h = 120.0;
    let erwartet = [
        v(0.0, 0.0) + n * h,
        v(0.0, 0.0) - n * h,
        v(3000.0, 3000.0) + n * h,
        v(3000.0, 3000.0) - n * h,
    ];
    for p in erwartet {
        let treffer = fang(&s, p + v(1.0, -1.0), 25.0)
            .unwrap_or_else(|| panic!("schräge Wand: Ecke ({}, {}) nicht fangbar", p.x, p.y));
        assert_punkt(treffer.point, p, "schräge Wand");
    }
    assert_eq!(kontur_ecken(&s.doc, id).len(), 4);
}

#[test]
fn wandkontur_ecken_bei_gehrung() {
    // L-Anschluss zweier Wände: die gezeichneten Konturecken (Gehrung) müssen
    // fangbar sein, ebenso die freien Enden.
    let mut s = s();
    let a = wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    let b = wand(&mut s, [5.0, 0.0], [5.0, 5.0], 0.30);
    for id in [a, b] {
        for ecke in kontur_ecken(&s.doc, id) {
            let treffer = fang(&s, ecke, 4.0).unwrap_or_else(|| {
                panic!("Gehrung: gezeichnete Ecke ({}, {}) nicht fangbar", ecke.x, ecke.y)
            });
            assert_punkt(treffer.point, ecke, "Gehrung");
        }
    }
}

#[test]
fn wandkontur_ecken_bei_kreuzung() {
    // Zwei sich kreuzende Wände: alle gezeichneten Konturecken müssen fangbar sein.
    let mut s = s();
    let a = wand(&mut s, [0.0, 0.0], [6.0, 0.0], 0.30);
    let b = wand(&mut s, [3.0, -3.0], [3.0, 3.0], 0.20);
    for id in [a, b] {
        for ecke in kontur_ecken(&s.doc, id) {
            let treffer = fang(&s, ecke, 4.0).unwrap_or_else(|| {
                panic!("Kreuzung: gezeichnete Ecke ({}, {}) nicht fangbar", ecke.x, ecke.y)
            });
            assert_punkt(treffer.point, ecke, "Kreuzung");
        }
    }
}

#[test]
fn wandkontur_ohne_doppelte_fangkandidaten() {
    // An der Gehrung treffen sich die Konturen beider Wände. Der Fang darf nur
    // einen Punkt liefern, und zwar exakt auf der Ecke.
    let mut s = s();
    wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    wand(&mut s, [5.0, 0.0], [5.0, 5.0], 0.30);
    let treffer = fang(&s, v(5150.0, 150.0), 20.0);
    let treffer = treffer.expect("äußere Gehrungsecke muss fangbar sein");
    assert!(
        (treffer.point.x - 5150.0).abs() < 0.5 && (treffer.point.y - 150.0).abs() < 0.5,
        "Gehrungsecke erwartet (5150, 150), gefangen ({}, {})",
        treffer.point.x,
        treffer.point.y
    );
}

// ---------------------------------------------------------------------------
// 2. Schalter „Wand-Punktfang“ (steuert nur die Kontur)
// ---------------------------------------------------------------------------

#[test]
fn wandachse_bleibt_fangbar() {
    let mut s = s();
    wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    let treffer = fang(&s, v(5.0, 4.0), 30.0).expect("Achsen-Endpunkt muss fangbar bleiben");
    assert_punkt(treffer.point, v(0.0, 0.0), "Achsen-Endpunkt");
    assert_eq!(treffer.kind, SnapKind::Endpoint);
    // Achsen-Mitte
    let treffer = fang(&s, v(2505.0, 4.0), 30.0).expect("Achsen-Mitte muss fangbar bleiben");
    assert_punkt(treffer.point, v(2500.0, 0.0), "Achsen-Mitte");
}

#[test]
fn schalter_aus_blendet_nur_die_wandkontur() {
    let mut s = s();
    let id = wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    let ecke = kontur_ecken(&s.doc, id)[0];
    // Vorher: Kontur-Ecke fangbar
    assert!(
        fang(&s, ecke + v(2.0, 2.0), 30.0).is_some(),
        "Vorbedingung: Kontur-Ecke muss bei aktivem Schalter fangbar sein"
    );

    schalter_setzen(&mut s, false);

    // Kein Wandkontur-Fangpunkt mehr an der Ecke
    assert!(
        fang(&s, ecke + v(2.0, 2.0), 30.0).is_none(),
        "bei ausgeschaltetem Wand-Punktfang darf die Kontur-Ecke ({}, {}) nicht mehr fangen",
        ecke.x,
        ecke.y
    );
    // Auch die Längskante selbst liefert keinen Fangpunkt mehr
    assert!(
        fang(&s, v(2500.0, 150.0), 30.0).is_none(),
        "bei ausgeschaltetem Wand-Punktfang darf auch die Längskante nicht fangen"
    );
    // Wandachse bleibt unberührt
    let achse = fang(&s, v(5.0, 4.0), 30.0).expect("Wandachse muss weiter fangen");
    assert_punkt(achse.point, v(0.0, 0.0), "Achse bei ausgeschaltetem Schalter");
    assert_eq!(achse.kind, SnapKind::Endpoint);
}

#[test]
fn schalter_aus_laesst_uebrige_snap_typen_aktiv() {
    let mut s = s();
    wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    s.execute("draw.line", &json!({"a": [-2.0, 2.0], "b": [-2.0, 4.0]}))
        .unwrap();
    s.execute("doc.settings", &json!({"snap": {"grid": true}}))
        .unwrap();
    schalter_setzen(&mut s, false);

    // Endpunkt einer Linie
    let hit = fang(&s, v(-2000.0, 2010.0), 40.0).expect("Linien-Endpunkt");
    assert_eq!(hit.kind, SnapKind::Endpoint);
    // Mittelpunkt der Linie
    let hit = fang(&s, v(-1995.0, 3000.0), 30.0).expect("Linien-Mittelpunkt");
    assert_punkt(hit.point, v(-2000.0, 3000.0), "Linien-Mittelpunkt");
    assert_eq!(hit.kind, SnapKind::Midpoint);
    // Raster
    let hit = fang(&s, v(-7010.0, 7030.0), 80.0).expect("Rasterpunkt");
    assert_eq!(hit.kind, SnapKind::Grid, "Rasterfang: {:?}", hit);
    assert_punkt(hit.point, v(-7000.0, 7000.0), "Rasterpunkt");
}

#[test]
fn schalter_wieder_an_stellt_verhalten_wieder_her() {
    let mut s = s();
    let id = wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    let ecke = kontur_ecken(&s.doc, id)[0];
    schalter_setzen(&mut s, false);
    assert!(fang(&s, ecke + v(2.0, 2.0), 30.0).is_none(), "aus: kein Fang");
    schalter_setzen(&mut s, true);
    let hit = fang(&s, ecke + v(2.0, 2.0), 30.0).expect("an: Fang wieder aktiv");
    assert_punkt(hit.point, ecke, "nach Wiedereinschalten");
}

#[test]
fn schalter_default_ist_an() {
    let s = Session::new();
    assert!(
        schalter(&s),
        "Wand-Punktfang muss standardmäßig aktiv sein"
    );
}

// ---------------------------------------------------------------------------
// 3. Persistenz über Speichern/Laden (Neustart)
// ---------------------------------------------------------------------------

#[test]
fn schalter_ueberlebt_speichern_und_laden() {
    let mut s = s();
    wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    schalter_setzen(&mut s, false);

    let dir = std::env::temp_dir().join("jjcad-test-wandfang");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wandfang.jjcad");
    s.save(&path).unwrap();

    // Datei ist lesbar (kein Binärformat)
    let text = std::fs::read_to_string(&path).unwrap();
    let feld = schalter_feld(&s);
    assert!(
        text.contains(&format!("\"{feld}\"")),
        "Schalter {feld} fehlt in der gespeicherten Datei"
    );

    // „Neustart“: neues Dokument aus der Datei
    let geladen = jjcad_core::persist::load_document(&path).unwrap();
    let j = serde_json::to_value(&geladen.settings.snap).unwrap();
    assert_eq!(
        j[feld].as_bool(),
        Some(false),
        "Einstellung muss den Neustart überleben"
    );
    // Fang-Engine des geladenen Dokuments kennt den ausgeschalteten Zustand
    let id = geladen.entities.last().unwrap().id;
    let ecke = kontur_ecken(&geladen, id)[0];
    assert!(
        find_snap(&geladen, ecke + v(2.0, 2.0), 30.0, None, &[]).is_none(),
        "nach dem Laden darf die Kontur-Ecke nicht fangen"
    );
}

// ---------------------------------------------------------------------------
// 4. Regression der übrigen Fangarten
// ---------------------------------------------------------------------------

#[test]
fn regression_endpunkt_mittelpunkt_schnitt_raster() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0.0, 0.0], "b": [2.0, 0.0]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [1.0, -1.0], "b": [1.0, 1.0]}))
        .unwrap();

    // Endpunkt
    let hit = fang(&s, v(1995.0, 5.0), 40.0).expect("Endpunkt");
    assert_eq!(hit.kind, SnapKind::Endpoint);
    assert_punkt(hit.point, v(2000.0, 0.0), "Endpunkt");

    // Mittelpunkt (Linie 0,0 -> 2000,0): (1000, 0) liegt auf der Schnittlinie,
    // deshalb die senkrechte Linie (1000, -1000 -> 1000, 1000) als Prüfobjekt
    // über ihren Mittelpunkt (1000, 0) -> hier gewinnt der Schnittpunkt,
    // also echten Mittelpunkt einer zweiten Linie nehmen.
    s.execute("draw.line", &json!({"a": [0.0, 3.0], "b": [2.0, 3.0]}))
        .unwrap();
    let hit = fang(&s, v(1005.0, 3005.0), 30.0).expect("Mittelpunkt");
    assert_eq!(hit.kind, SnapKind::Midpoint);
    assert_punkt(hit.point, v(1000.0, 3000.0), "Mittelpunkt");

    // Schnittpunkt
    let hit = fang(&s, v(1005.0, 5.0), 40.0).expect("Schnittpunkt");
    assert_eq!(hit.kind, SnapKind::Intersection);
    assert_punkt(hit.point, v(1000.0, 0.0), "Schnittpunkt");

    // Raster
    s.execute("doc.settings", &json!({"snap": {"grid": true}}))
        .unwrap();
    let hit = fang(&s, v(-7990.0, 8020.0), 80.0).expect("Raster");
    assert_eq!(hit.kind, SnapKind::Grid);
    assert_punkt(hit.point, v(-8000.0, 8000.0), "Raster");
}

#[test]
fn regression_zentrum_und_kreisfang() {
    let mut s = s();
    s.execute("draw.circle", &json!({"c": [2.0, 2.0], "r": 1.0}))
        .unwrap();
    let hit = fang(&s, v(2005.0, 2005.0), 40.0).expect("Zentrum");
    assert_eq!(hit.kind, SnapKind::Center);
    assert_punkt(hit.point, v(2000.0, 2000.0), "Zentrum");
}

// ---------------------------------------------------------------------------
// 5. Performance bei vielen Wänden
// ---------------------------------------------------------------------------

#[test]
fn performance_viele_waende() {
    let mut s = s();
    // 20 x 10 = 200 Wände
    for iy in 0..10 {
        for ix in 0..20 {
            let x = ix as f64 * 1.0;
            let y = iy as f64 * 3.0;
            if (ix + iy) % 2 == 0 {
                wand(&mut s, [x, y], [x + 1.0, y], 0.24);
            } else {
                wand(&mut s, [x, y], [x, y + 1.0], 0.24);
            }
        }
    }
    assert_eq!(s.doc.entities.len(), 200);

    // Messung: 100 Fänge über das gesamte Feld verteilt
    let start = std::time::Instant::now();
    let mut treffer = 0usize;
    for i in 0..100 {
        let p = v(
            (i % 20) as f64 * 1000.0 + 500.0,
            ((i / 20) % 10) as f64 * 3000.0,
        );
        if fang(&s, p, 60.0).is_some() {
            treffer += 1;
        }
    }
    let dauer = start.elapsed();
    let pro_fang = dauer.as_secs_f64() * 1000.0 / 100.0;
    println!(
        "Performance: 200 Wände, 100 Fänge in {:?} ({:.2} ms je Fang), {treffer} Treffer",
        dauer, pro_fang
    );
    assert!(
        pro_fang < 25.0,
        "Fang mit 200 Wänden zu langsam: {pro_fang:.2} ms je Aufruf (Grenze 25 ms)"
    );
}

// ---------------------------------------------------------------------------
// 6. Fremdnutzung der Segmentquelle bleibt unberührt (Stutzen)
// ---------------------------------------------------------------------------

#[test]
fn stutzen_nutzt_weiter_die_wandachse() {
    // Schnittkanten für Stutzen/Verkürzen stammen aus derselben Segmentquelle.
    // Die Wand darf dort weiter nur mit ihrer Achse schneiden — sonst endet eine
    // gestutzte Linie plötzlich auf der Wandkante statt auf der Achse.
    let mut s = s();
    wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    s.execute(
        "draw.line",
        &json!({"a": [2.0, 3.0], "b": [2.0, -0.5]}),
    )
    .unwrap();
    let linie = s.doc.entities.last().unwrap().id;
    s.execute("modify.trim", &json!({"ids": [linie], "at": [2.0, 1.0]}))
        .unwrap();
    let enden: Vec<f64> = s
        .doc
        .entities
        .iter()
        .flat_map(|e| match &e.kind {
            EntityKind::Line { a, b } if (a.x - 2000.0).abs() < 1.0 && (b.x - 2000.0).abs() < 1.0 => {
                vec![a.y, b.y]
            }
            _ => vec![],
        })
        .collect();
    assert!(
        enden.iter().any(|y| y.abs() < 1.0),
        "Stutzen muss an der Wandachse (y=0) enden, Endpunkte: {enden:?}"
    );
    assert!(
        !enden.iter().any(|y| (y.abs() - 150.0).abs() < 1.0),
        "Stutzen darf nicht an der Wandkante (y=+-150) enden, Endpunkte: {enden:?}"
    );
}

#[test]
fn wandtyp_bleibt_erhalten() {
    // Sicherheitsnetz: der Wandkörper selbst bleibt eine Wand (kein Umbau zu Linien).
    let mut s = s();
    wand(&mut s, [0.0, 0.0], [5.0, 0.0], 0.30);
    assert!(matches!(
        s.doc.entities[0].kind,
        EntityKind::Wall { .. }
    ));
}
