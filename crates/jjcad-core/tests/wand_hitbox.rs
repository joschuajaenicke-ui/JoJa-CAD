//! Wände und die Interaktion mit Auswahl und Bearbeitung (T0207).
//!
//! Die Wände sind mehrfach umgebaut worden; diese Tests halten die Stellen
//! fest, an denen Körper, Auswahl und Verlängern auseinanderliefen:
//!
//! 1. Der **Wandkörper** ist das Band um die *versetzte* Achse (Ausrichtung
//!    links/mittig/rechts), nicht um die Basislinie — ein Klick in die Hälfte
//!    auf der Versatzseite muss treffen.
//! 2. Die **Fensterauswahl** (links→rechts) nimmt eine Wand mit, wenn ihr
//!    Körper ganz im Fenster liegt; die **Kreuzauswahl** bei Berührung.
//! 3. **Verlängern** endet an der Umrandung der Wand, nicht an ihrer
//!    Basislinie.
//! 4. **Kopieren** einer Wand nimmt ihre Türen und Fenster mit und hängt die
//!    Kopien an die kopierte Wand (nicht an das Original).
//! 5. **Kurze Wände** bekommen an der Ecke dieselbe Gehrung wie lange.

use jjcad_core::geom::Vec2;
use jjcad_core::model::EntityKind;
use jjcad_core::render;
use jjcad_core::snap;
use jjcad_core::style::WallAlign;
use jjcad_core::Session;
use serde_json::json;

const TOL: f64 = 0.05;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

fn wand(s: &mut Session, a: (f64, f64), b: (f64, f64), t: f64, al: WallAlign) -> u64 {
    let r = s
        .execute(
            "draw.wall",
            &json!({
                "a": [a.0 / 1000.0, a.1 / 1000.0],
                "b": [b.0 / 1000.0, b.1 / 1000.0],
                "thickness": t / 1000.0,
                "align": match al {
                    WallAlign::Left => "left",
                    WallAlign::Center => "center",
                    WallAlign::Right => "right",
                },
            }),
        )
        .unwrap();
    r["created"][0].as_u64().unwrap()
}

fn linie(s: &mut Session, a: (f64, f64), b: (f64, f64)) -> u64 {
    let r = s
        .execute(
            "draw.line",
            &json!({"a": [a.0 / 1000.0, a.1 / 1000.0], "b": [b.0 / 1000.0, b.1 / 1000.0]}),
        )
        .unwrap();
    r["created"][0].as_u64().unwrap()
}

fn punkte(s: &Session, id: u64) -> Vec<Vec2> {
    render::wall_contour_points(&s.doc, s.doc.entity(id).unwrap())
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

/// Wand links ausgerichtet (Band liegt bei y ∈ [0, Dicke]): ein Klick in die
/// Bandmitte — also neben der Basislinie — muss treffen.
#[test]
fn wand_links_ist_in_ihrer_ganzen_breite_anklickbar() {
    let mut s = session();
    let id = wand(&mut s, (0.0, 0.0), (5000.0, 0.0), 240.0, WallAlign::Left);
    for y in [10.0, 120.0, 230.0] {
        let p = v(2500.0, y);
        assert_eq!(
            snap::hit_test(&s.doc, p, 12.0, None),
            Some(id),
            "Klick bei y = {y} mm (Bandmitte liegt bei 120) muss die Wand treffen"
        );
    }
    // Gegenprobe: 40 mm außerhalb des Bandes trifft nichts.
    assert_eq!(snap::hit_test(&s.doc, v(2500.0, -40.0), 12.0, None), None);
}

/// Dasselbe für die Kreuzauswahl (Berührung des Körpers).
#[test]
fn kreuzauswahl_trifft_die_ganze_wandbreite() {
    let mut s = session();
    let id = wand(&mut s, (0.0, 0.0), (5000.0, 0.0), 240.0, WallAlign::Left);
    let ids = snap::select_window(&s.doc, v(2400.0, 100.0), v(2600.0, 200.0), true);
    assert!(
        ids.contains(&id),
        "Kreuzauswahl im Band muss die Wand wählen: {ids:?}"
    );
    let ids = snap::select_window(&s.doc, v(2400.0, 900.0), v(2600.0, 1000.0), true);
    assert!(
        !ids.contains(&id),
        "Fenster neben der Wand darf sie nicht wählen"
    );
}

/// Fensterauswahl (links→rechts) um die ganze Wand nimmt sie mit — vorher
/// verlangte der Zweig, dass alle vier Ecken *des Fensters* im Wandkörper
/// liegen; damit war eine Wand nie wählbar.
#[test]
fn fensterauswahl_waehlt_eine_ganz_umschlossene_wand() {
    let mut s = session();
    let id = wand(&mut s, (0.0, 0.0), (5000.0, 0.0), 240.0, WallAlign::Center);
    let ids = snap::select_window(&s.doc, v(-500.0, -500.0), v(5500.0, 500.0), false);
    assert!(
        ids.contains(&id),
        "Fenster um die Wand muss sie wählen: {ids:?}"
    );
    // Ein Fenster, das nur die halbe Wand abdeckt, wählt nicht.
    let ids = snap::select_window(&s.doc, v(-500.0, -500.0), v(2500.0, 500.0), false);
    assert!(
        !ids.contains(&id),
        "halbes Fenster darf die Wand nicht wählen"
    );
}

/// Verlängern einer Linie endet an der **Umrandung** der Wand, nicht an der
/// Basislinie: die Wand liegt quer bei x = 5000, 240 dick, mittig — die
/// Umrandung liegt bei x = 4880.
#[test]
fn verlaengern_endet_an_der_wandumrandung() {
    let mut s = session();
    let linie_id = linie(&mut s, (0.0, 0.0), (2000.0, 0.0));
    wand(
        &mut s,
        (5000.0, -3000.0),
        (5000.0, 3000.0),
        240.0,
        WallAlign::Center,
    );
    s.execute(
        "modify.extend",
        &json!({"ids": [linie_id], "at": [6.0, 0.0]}),
    )
    .unwrap();
    let EntityKind::Line { b, .. } = &s.doc.entity(linie_id).unwrap().kind else {
        panic!("Linie erwartet");
    };
    assert!(
        (b.x - 4880.0).abs() <= 0.5,
        "Linie muss an der Wandkante bei x = 4880 enden, ist aber bei {b:?}"
    );
}

/// Dasselbe für eine links ausgerichtete Wand: ihr Körper reicht von 4760 bis
/// 5000 (die Normale der +Y-Richtung zeigt nach −X) — die Linie muss bei 4760
/// enden, nicht bei 5000 (Basislinie).
#[test]
fn verlaengern_trifft_die_verschobene_wandkante() {
    let mut s = session();
    let linie_id = linie(&mut s, (0.0, 0.0), (2000.0, 0.0));
    wand(
        &mut s,
        (5000.0, -3000.0),
        (5000.0, 3000.0),
        240.0,
        WallAlign::Left,
    );
    s.execute(
        "modify.extend",
        &json!({"ids": [linie_id], "at": [6.0, 0.0]}),
    )
    .unwrap();
    let EntityKind::Line { b, .. } = &s.doc.entity(linie_id).unwrap().kind else {
        panic!("Linie erwartet");
    };
    assert!(
        (b.x - 4760.0).abs() <= 0.5,
        "Linie muss an der Bandkante bei x = 4760 enden, ist aber bei {b:?}"
    );
}

/// Eine Wand mit Tür kopieren: die Kopie bringt die Tür mit, und die Tür hängt
/// an der **kopierten** Wand. Vorher nahm `modify.copy` die Öffnung nicht mit;
/// wurde sie mitkopiert, zeigte ihr `host` weiter auf das Original (T0207).
#[test]
fn kopierte_wand_nimmt_ihre_tuer_mit() {
    let mut s = session();
    let wand_id = wand(&mut s, (0.0, 0.0), (5000.0, 0.0), 240.0, WallAlign::Center);
    s.execute(
        "draw.door",
        &json!({"host": wand_id, "at": [2.5, 0.0], "width": "1.0", "swing_in": true}),
    )
    .unwrap();
    let tuer = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Door { .. }))
        .map(|e| e.id)
        .expect("Tür");

    let antwort = s
        .execute(
            "modify.copy",
            &json!({"ids": [wand_id], "from": [0.0, 0.0], "to": [0.0, 6.0]}),
        )
        .unwrap();
    let neu: Vec<u64> = antwort["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap())
        .collect();
    assert_eq!(neu.len(), 2, "Kopie muss Wand **und** Tür anlegen: {neu:?}");

    let ist_wand = |id: &u64| {
        matches!(
            s.doc.entity(*id).map(|e| &e.kind),
            Some(EntityKind::Wall { .. })
        )
    };
    let neue_wand = *neu.iter().find(|id| ist_wand(id)).expect("Wand in der Kopie");
    let neue_tuer = *neu.iter().find(|id| !ist_wand(id)).expect("Tür in der Kopie");

    let EntityKind::Door { host, center, .. } = &s.doc.entity(neue_tuer).unwrap().kind else {
        panic!("Tür erwartet");
    };
    assert_eq!(
        *host,
        Some(neue_wand),
        "Kopie-Tür muss an der Kopie-Wand hängen"
    );
    // Die Tür sitzt auf der vorderen Wandkante (Dicke/2 = 120 über der Achse).
    assert!(
        (center.y - 6120.0).abs() <= 0.5,
        "Kopie-Tür muss mit der Wand gewandert sein, ist bei {center:?}"
    );
    // Das Original bleibt unberührt.
    let EntityKind::Door {
        host: alt_host,
        center: alt_center,
        ..
    } = &s.doc.entity(tuer).unwrap().kind
    else {
        panic!("Tür erwartet");
    };
    assert_eq!(*alt_host, Some(wand_id));
    assert!((alt_center.y - 120.0).abs() <= 0.5, "Original-Tür bleibt am Original: {alt_center:?}");
}

/// Kurze Wände an einer 90°-Ecke: dieselbe Gehrung wie bei langen Wänden —
/// beide mittigen Konturen treffen sich in den zwei Gehrungspunkten. Vorher
/// fiel die Naht aus, sobald die Wand kürzer war als der Abstand ihres
/// Gehrungspunkts vom Knoten (240 mm dick → unter 170 mm, 365 mm dick → unter
/// 258 mm) und die Ecke blieb offen.
///
/// Bei **versetzt** ausgerichteten Wänden ist die Gehrung keine Naht durch den
/// Knoten: die Körper stoßen stumpf aneinander und überlappen sich in der
/// Ecke. Dort genügt, dass sich die Konturen berühren — ein Loch wäre der
/// Fehler.
#[test]
fn kurze_waende_bekommen_die_ecke() {
    // Längen ab der halben Dicke: eine Wand, die kürzer als ihre halbe Dicke
    // ist, hat keinen Körper mehr zum Schneiden — das ist keine Ecke.
    for (dicke, la, lb, gehrung) in [
        (240.0, WallAlign::Center, WallAlign::Center, true),
        (365.0, WallAlign::Center, WallAlign::Center, true),
        (120.0, WallAlign::Center, WallAlign::Center, true),
        (240.0, WallAlign::Left, WallAlign::Left, false),
        (240.0, WallAlign::Left, WallAlign::Right, false),
        (365.0, WallAlign::Left, WallAlign::Center, false),
    ] {
        for len in [120.0f64, 150.0, 200.0, 300.0, 500.0, 2000.0]
            .into_iter()
            .filter(|l| *l > dicke * 0.5)
        {
            let mut s = session();
            let a = wand(&mut s, (0.0, 0.0), (len, 0.0), dicke, la);
            let b = wand(&mut s, (len, 0.0), (len, 2000.0), dicke, lb);
            let (pa, pb) = (punkte(&s, a), punkte(&s, b));
            let gemeinsam = pa
                .iter()
                .filter(|p| pb.iter().any(|q| p.dist(*q) <= TOL))
                .count();
            if gehrung {
                assert!(
                    gemeinsam >= 2,
                    "Dicke {dicke}, Länge {len}: Gehrung fehlt \
                     (gemeinsame Punkte: {gemeinsam})"
                );
            } else {
                // Berührung: ein Konturpunkt der einen Wand liegt auf einer
                // Kante der anderen (die Körper stoßen stumpf zusammen).
                let ring = |pts: &[Vec2]| {
                    let mut r = pts.to_vec();
                    if let Some(&p0) = pts.first() {
                        r.push(p0);
                    }
                    r
                };
                let rb = ring(&pb);
                let beruehrt = pa.iter().any(|p| {
                    rb.windows(2)
                        .any(|w| jjcad_core::geom::dist_point_segment(*p, w[0], w[1]) <= TOL)
                });
                assert!(
                    beruehrt,
                    "Dicke {dicke}, Länge {len}, {la:?}/{lb:?}: Konturen berühren sich nicht"
                );
            }
            assert!(
                pa.len() >= 3,
                "Dicke {dicke}, Länge {len}: Kontur hat nur {} Punkte",
                pa.len()
            );
        }
    }
}
