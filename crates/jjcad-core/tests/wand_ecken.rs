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

// ---------------------------------------------------------------------------
// T0020: Stumpfstoß (Verlängerung) und Anschlüsse mit unterschiedlicher
// Ausrichtung/Dicke.
// ---------------------------------------------------------------------------

/// Wand in mm mit Ausrichtung anlegen.
fn wand_align(
    s: &mut Session,
    a: (f64, f64),
    b: (f64, f64),
    t: f64,
    align: &str,
) -> u64 {
    s.execute(
        "draw.wall",
        &json!({"a": [a.0 / 1000.0, a.1 / 1000.0],
                "b": [b.0 / 1000.0, b.1 / 1000.0],
                "thickness": t / 1000.0,
                "align": align}),
    )
    .unwrap();
    s.doc.entities.last().unwrap().id
}

/// Alle gezeichneten Linien der Zeichnung.
fn linien(s: &Session) -> Vec<Seg> {
    let dl = render::render_document(&render::RenderCtx::model(&s.doc));
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            render::Shape::Line(a, b) => Some(Seg::Line(*a, *b)),
            _ => None,
        })
        .collect()
}

/// Liegt eine Strecke auf der Geraden x = `x` und **überschneidet** sie den
/// y-Bereich (lo, hi) um mehr als die Toleranz? (Ein bloßes Berühren der
/// Intervallgrenze zählt nicht.)
fn querlinie_bei(segs: &[Seg], x: f64, lo: f64, hi: f64) -> bool {
    segs.iter().any(|sg| {
        let (a, b) = (sg.start(), sg.end());
        (a.x - x).abs() <= TOL
            && (b.x - x).abs() <= TOL
            && a.y.min(b.y) < hi - TOL
            && a.y.max(b.y) > lo + TOL
    })
}

/// Verlängerung zweier fluchtender Wände gleicher Ausrichtung: eine Wand, keine
/// Naht. Vorher stand dort die Stirnkante der ersten Wand quer im Wandkörper.
#[test]
fn verlaengerung_zeichnet_keine_querlinie() {
    let mut s = session();
    let a = wand_align(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0, "center");
    let b = wand_align(&mut s, (2000.0, 0.0), (4000.0, 0.0), 100.0, "center");
    let segs = linien(&s);

    assert!(
        !querlinie_bei(&segs, 2000.0, -50.0, 50.0),
        "Stumpfstoß darf keine Querlinie im Wandkörper zeichnen"
    );
    // Beide Längskanten laufen durch den Stoß durch.
    for y in [50.0, -50.0] {
        assert!(
            hat_strecke(&segs, v(0.0, y), v(4000.0, y)) || hat_strecke(&segs, v(0.0, y), v(2000.0, y)),
            "Längskante y={y} fehlt vor dem Stoß"
        );
        assert!(
            hat_strecke(&segs, v(2000.0, y), v(4000.0, y)),
            "Längskante y={y} fehlt nach dem Stoß"
        );
    }
    let _ = (a, b);
}

/// Verlängerung mit unterschiedlicher Ausrichtung: die Wände stoßen stumpf
/// zusammen, sichtbar ist nur die Stufe (der Teil der Stirnkante, den der
/// Nachbarkörper nicht abdeckt) — keine Querlinie durch die überlappende Fläche.
#[test]
fn verlaengerung_versetzter_ausrichtung_zeichnet_stufe() {
    let mut s = session();
    let _a = wand_align(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0, "center");
    let _b = wand_align(&mut s, (2000.0, 0.0), (4000.0, 0.0), 100.0, "left");
    let segs = linien(&s);

    // Die erste Wand endet bei y = -50..50, die zweite deckt 0..100 ab.
    // Sichtbar ist nur die Stufe unterhalb von y = 0.
    assert!(
        !querlinie_bei(&segs, 2000.0, 0.5, 50.0),
        "Querlinie im vom Nachbarn abgedeckten Bereich"
    );
    assert!(
        querlinie_bei(&segs, 2000.0, -50.0, -0.5),
        "Stufe der versetzten Wand fehlt"
    );
    // Die Oberkante der zweiten Wand beginnt am Stoß, nicht davor.
    assert!(
        segs.iter().any(|sg| {
            let (p, q) = (sg.start(), sg.end());
            ((p.dist(v(2000.0, 100.0)) <= TOL && q.dist(v(4000.0, 100.0)) <= TOL)
                || (p.dist(v(4000.0, 100.0)) <= TOL && q.dist(v(2000.0, 100.0)) <= TOL))
        }),
        "Oberkante der versetzten Wand fehlt"
    );
}

/// Ecke mit unterschiedlicher Ausrichtung (mittig → rechts): die Umrandung
/// muss am Knoten geschlossen sein. Vorher endete die Oberkante der ersten
/// Wand vor dem Knoten, die Füllung stand als ungerandeter Keil darüber.
///
/// Hier ist **kein** Gehrungsschnitt möglich: die zweite Wand liegt ganz auf
/// ihrer einen Seite, ihre Stirnfläche steht an der Basislinie des Knotens.
/// Die erste Wand endet deshalb bündig an dieser Stirnfläche (x = 2000) —
/// die Stufe ist die Wand selbst, keine Lücke.
#[test]
fn ecke_versetzter_ausrichtung_ist_geschlossen() {
    let mut s = session();
    let a = wand_align(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0, "center");
    let _b = wand_align(&mut s, (2000.0, 0.0), (3000.0, -1500.0), 100.0, "right");
    let segs = linien(&s);

    // Die Oberkante der ersten Wand muss bis an den Knoten reichen.
    let ende = segs
        .iter()
        .filter_map(|sg| {
            let (p, q) = (sg.start(), sg.end());
            if (p.y - 50.0).abs() <= TOL && (q.y - 50.0).abs() <= TOL {
                Some(p.x.max(q.x))
            } else {
                None
            }
        })
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (ende - 2000.0).abs() <= TOL,
        "Oberkante endet bei x = {ende}, nicht am Knoten 2000"
    );
    // Und der Knoten selbst ist ein Konturpunkt der ersten Wand.
    assert!(
        punkte(&s, a).iter().any(|p| p.dist(v(2000.0, 50.0)) <= TOL),
        "Ecke (2000; 50) fehlt in der Kontur"
    );
}

/// Ecke mit **unterschiedlicher Dicke**: beide Wände rechnen dieselbe
/// Gehrungslinie, die Umrandung ist damit geschlossen. Vorher lag jede Wand
/// auf ihrer eigenen Winkelhalbierenden, dazwischen klaffte ein Keil.
#[test]
fn ecke_unterschiedlicher_dicke_hat_gemeinsame_gehrung() {
    let mut s = session();
    let a = wand_align(&mut s, (0.0, 0.0), (2000.0, 0.0), 100.0, "center");
    let b = wand_align(&mut s, (2000.0, 0.0), (3000.0, -1500.0), 150.0, "center");

    let ra = punkte(&s, a);
    let rb = punkte(&s, b);
    let nahe: Vec<Vec2> = ra.iter().cloned().filter(|p| p.dist(v(2000.0, 0.0)) < 400.0).collect();
    assert!(nahe.len() >= 2, "Gehrungsecken der ersten Wand fehlen: {ra:?}");
    for p in &nahe {
        assert!(
            rb.iter().any(|q| q.dist(*p) <= TOL),
            "Gehrungsecke {p:?} fehlt an der zweiten Wand: {rb:?}"
        );
    }
}
