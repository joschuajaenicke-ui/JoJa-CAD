//! Eckanschluss von Wänden: verbunden wird nur, wenn die **Basislinien**-
//! Endpunkte aufeinander liegen (Toleranz `render::JOIN_EPS` = 0,5 mm).
//!
//! Vorher zählte jede Nachbarwand als Eckpartner, deren Ende näher als die
//! Summe der halben Wanddicken (+2 mm) lag — bei 100 mm dicken Wänden also bis
//! 102 mm. Zwei knapp nebeneinanderliegende Enden bekamen damit mal eine
//! Gehrung und mal nicht, je nach Wanddicke und Versatz. Diese Tests halten den
//! eindeutigen Zustand fest: **gleicher Punkt → Gehrung, sonst freies Ende.**

use jjcad_core::geom::{Seg, Vec2};
use jjcad_core::render;
use jjcad_core::Session;
use serde_json::json;

const TOL: f64 = 0.05;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

/// Wand in mm anlegen; die Aktion erwartet Meter (Eingabeeinheit des Dokuments).
fn wand(s: &mut Session, a: (f64, f64), b: (f64, f64), t: f64) -> u64 {
    s.execute(
        "draw.wall",
        &json!({"a": [a.0 / 1000.0, a.1 / 1000.0],
                "b": [b.0 / 1000.0, b.1 / 1000.0],
                "thickness": t / 1000.0,
                "align": "center"}),
    )
    .unwrap();
    s.doc.entities.last().unwrap().id
}

fn kontur(s: &Session, id: u64) -> Vec<Seg> {
    render::wall_contour_segments(&s.doc, s.doc.entity(id).unwrap())
}

fn punkte(s: &Session, id: u64) -> Vec<Vec2> {
    render::wall_contour_points(&s.doc, s.doc.entity(id).unwrap())
}

/// Enthält die Kontur die Strecke p–q (in beliebiger Richtung)?
fn hat_strecke(segs: &[Seg], p: Vec2, q: Vec2) -> bool {
    segs.iter().any(|sg| {
        let (a, b) = (sg.start(), sg.end());
        (a.dist(p) <= TOL && b.dist(q) <= TOL) || (a.dist(q) <= TOL && b.dist(p) <= TOL)
    })
}

/// Haben beide Wände einen gemeinsamen Konturpunkt?
fn gemeinsamer_punkt(a: &[Vec2], b: &[Vec2]) -> bool {
    a.iter().any(|p| b.iter().any(|q| p.dist(*q) <= TOL))
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

/// 90°-Ecke mit **verbundener** Basislinie: beide Wände liefern dieselbe
/// Gehrungsecke, keine der beiden zeichnet eine Stirnseite.
#[test]
fn verbundene_basislinie_bekommt_gehrung() {
    let mut s = session();
    let a = wand(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0);
    let b = wand(&mut s, (2000.0, 0.0), (2000.0, 2000.0), 100.0);

    let (ka, kb) = (kontur(&s, a), kontur(&s, b));
    assert!(
        gemeinsamer_punkt(&punkte(&s, a), &punkte(&s, b)),
        "verbundene Enden müssen eine gemeinsame Gehrungsecke haben"
    );
    // Die Gehrungsecken der 90°-Ecke (beide Wände rechnen sie gleich).
    for ecke in [v(1950.0, 50.0), v(2050.0, -50.0)] {
        assert!(
            punkte(&s, a).iter().any(|p| p.dist(ecke) <= TOL),
            "Gehrungsecke {ecke:?} fehlt an Wand A"
        );
        assert!(
            punkte(&s, b).iter().any(|p| p.dist(ecke) <= TOL),
            "Gehrungsecke {ecke:?} fehlt an Wand B"
        );
    }
    // Keine Stirnseiten: die Enden sind beschnitten, nicht stumpf.
    assert!(
        !hat_strecke(&ka, v(2000.0, -50.0), v(2000.0, 50.0)),
        "Wand A darf am Anschluss keine Stirnseite zeichnen"
    );
    assert!(
        !hat_strecke(&kb, v(1950.0, 0.0), v(2050.0, 0.0)),
        "Wand B darf am Anschluss keine Stirnseite zeichnen"
    );
}

/// 5 mm Lücke in Achsrichtung: die Körper überlappen sich fast vollständig,
/// die Basispunkte sind aber nicht verbunden — also zwei freie Enden.
///
/// Vorher lag das innerhalb der Toleranz `h + h + 2` (102 mm bei 100 mm
/// Wanddicke) und bekam eine Gehrung.
#[test]
fn knapp_versetzte_enden_verbinden_nicht() {
    let mut s = session();
    let a = wand(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0);
    let b = wand(&mut s, (2005.0, 0.0), (2005.0, 2000.0), 100.0);

    let (ka, kb) = (kontur(&s, a), kontur(&s, b));
    assert!(
        hat_strecke(&ka, v(2000.0, -50.0), v(2000.0, 50.0)),
        "Wand A muss ihr Ende stumpf schließen"
    );
    assert!(
        hat_strecke(&kb, v(1955.0, 0.0), v(2055.0, 0.0)),
        "Wand B muss ihr Ende stumpf schließen"
    );
    assert!(
        !gemeinsamer_punkt(&punkte(&s, a), &punkte(&s, b)),
        "5 mm auseinanderliegende Basispunkte sind kein Anschluss"
    );
}

/// Seitlicher Versatz: das Ende liegt neben der anderen Wand, nicht auf ihrer
/// Basislinie. Ebenfalls zwei freie Enden.
#[test]
fn seitlich_versetzte_enden_verbinden_nicht() {
    let mut s = session();
    let a = wand(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0);
    let b = wand(&mut s, (2000.0, 40.0), (2000.0, 2040.0), 100.0);

    let (ka, kb) = (kontur(&s, a), kontur(&s, b));
    assert!(
        hat_strecke(&ka, v(2000.0, -50.0), v(2000.0, 50.0)),
        "Wand A muss ihr Ende stumpf schließen"
    );
    assert!(
        hat_strecke(&kb, v(1950.0, 40.0), v(2050.0, 40.0)),
        "Wand B muss ihr Ende stumpf schließen"
    );
    assert!(
        !gemeinsamer_punkt(&punkte(&s, a), &punkte(&s, b)),
        "seitlich versetzte Enden sind kein Anschluss"
    );
}

/// Stichanschluss: der Basispunkt liegt **auf** der Basislinie der anderen
/// Wand (mitten drauf). Das ist eine echte Verbindung — das Ende wird bis in
/// den Nachbarkörper verlängert, ohne Stirnseite.
#[test]
fn stichanschluss_auf_der_basislinie_verbindet() {
    let mut s = session();
    let _a = wand(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0);
    let c = wand(&mut s, (1000.0, -1000.0), (1000.0, 0.0), 100.0);

    let kc = kontur(&s, c);
    assert!(
        !hat_strecke(&kc, v(950.0, 0.0), v(1050.0, 0.0)),
        "angeschlossenes Ende darf keine Stirnseite zeichnen"
    );
    // Das Ende steckt im Nachbarkörper (die Wand endet auf dessen Basislinie)
    // und schießt nicht darüber hinaus.
    let p = punkte(&s, c);
    assert!(
        p.iter().any(|q| q.y.abs() <= TOL),
        "Wand C muss auf der Basislinie des Nachbarn enden: {p:?}"
    );
    assert!(
        p.iter().all(|q| q.y <= 50.0 + TOL),
        "Wand C darf nicht über den Nachbarkörper hinausragen: {p:?}"
    );
}

/// Die Toleranz ist die Grenze: 0,3 mm (Rundungsrest) verbindet noch,
/// 1,5 mm (sichtbarer Spalt) nicht mehr.
#[test]
fn toleranzgrenze_trennt_rundung_von_spalt() {
    let mut gerundet = session();
    let a = wand(&mut gerundet, (0.0, 0.0), (2000.0, 0.0), 100.0);
    let b = wand(&mut gerundet, (2000.3, 0.0), (2000.3, 2000.0), 100.0);
    assert!(
        gemeinsamer_punkt(&punkte(&gerundet, a), &punkte(&gerundet, b)),
        "0,3 mm sind Rundung — die Ecke muss verbunden werden"
    );

    let mut spalt = session();
    let a = wand(&mut spalt, (0.0, 0.0), (2000.0, 0.0), 100.0);
    let b = wand(&mut spalt, (2001.5, 0.0), (2001.5, 2000.0), 100.0);
    assert!(
        !gemeinsamer_punkt(&punkte(&spalt, a), &punkte(&spalt, b)),
        "1,5 mm sind ein Spalt — keine Ecke"
    );
}

/// Die Verbindung hängt nicht an der Wanddicke: dünn und dick verhalten sich
/// bei gleicher Lage gleich.
#[test]
fn verbindung_haengt_nicht_an_der_wanddicke() {
    for t in [60.0, 100.0, 365.0] {
        let mut s = session();
        // Lücke von 20 mm: bei dicken Wänden lag das früher weit in der Toleranz.
        let a = wand(&mut s, (0.0, 0.0), (2000.0, 0.0), t);
        let b = wand(&mut s, (2020.0, 0.0), (2020.0, 2000.0), t);
        assert!(
            !gemeinsamer_punkt(&punkte(&s, a), &punkte(&s, b)),
            "Wanddicke {t}: 20 mm Spalt darf keine Ecke ergeben"
        );

        let mut s2 = session();
        let a = wand(&mut s2, (0.0, 0.0), (2000.0, 0.0), t);
        let b = wand(&mut s2, (2000.0, 0.0), (2000.0, 2000.0), t);
        assert!(
            gemeinsamer_punkt(&punkte(&s2, a), &punkte(&s2, b)),
            "Wanddicke {t}: verbundene Enden müssen eine Ecke ergeben"
        );
    }
}

/// Kreuz: vier Wände treffen sich in einem Punkt. Dort läuft jede Wand gerade
/// durch (kein Gehrungssplitter, kein Überschuss über den Knoten) — und zwar
/// unabhängig davon, in welcher Reihenfolge die Wände im Dokument stehen.
#[test]
fn kreuz_laeuft_gerade_durch() {
    for verdreht in [false, true] {
        let mut s = session();
        let mut waende = vec![
            ((0.0, 2000.0), (2000.0, 2000.0)),
            ((2000.0, 2000.0), (4000.0, 2000.0)),
            ((2000.0, 0.0), (2000.0, 2000.0)),
            ((2000.0, 2000.0), (2000.0, 4000.0)),
        ];
        if verdreht {
            waende.reverse();
        }
        let ids: Vec<u64> = waende
            .iter()
            .map(|(a, b)| wand(&mut s, *a, *b, 100.0))
            .collect();
        for (k, id) in ids.iter().enumerate() {
            let (a, b) = waende[k];
            let pa = v(a.0, a.1);
            let pb = v(b.0, b.1);
            let dir = (pb - pa).normalized();
            let len = pa.dist(pb);
            for q in punkte(&s, *id) {
                let s_along = (q - pa).dot(dir);
                let quer = (q - pa).dot(dir.perp()).abs();
                // Höchstens eine halbe Wanddicke über die eigene Basislinie
                // hinaus — mehr wäre ein Gehrungssplitter am Knoten.
                assert!(
                    s_along >= -50.0 - TOL
                        && s_along <= len + 50.0 + TOL
                        && quer <= 50.0 + TOL,
                    "Kreuz (verdreht={verdreht}): Punkt {q:?} liegt neben der Wand"
                );
            }
        }
    }
}
