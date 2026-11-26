//! Abrunden und Phase (T0247): aus zwei sich schneidenden Linien wird eine
//! Ecke mit Bogen bzw. mit gerader Fase. Geprüft wird die gespeicherte
//! Geometrie — Endpunkte beider Linien, Mittelpunkt und Winkel des Bogens.

use jjcad_core::ecke::{ecke, linien_durch, polylinien_knoten, EckeArt};
use jjcad_core::geom::{v, Vec2};
use jjcad_core::model::{EntityKind, Id};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    s
}

fn linie(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> Id {
    s.execute("draw.line", &json!({"a": a, "b": b}))
        .expect("Linie anlegen");
    s.doc.entities.last().unwrap().id
}

fn enden(s: &Session, id: Id) -> (Vec2, Vec2) {
    match &s.doc.entity(id).expect("Objekt fehlt").kind {
        EntityKind::Line { a, b } => (*a, *b),
        other => panic!("keine Linie: {other:?}"),
    }
}

fn assert_punkt(ist: Vec2, soll: [f64; 2], was: &str) {
    let soll = v(soll[0], soll[1]);
    assert!(ist.dist(soll) < 1e-6, "{was}: erwartet {soll:?}, ist {ist:?}");
}

/// Rechtwinklige Ecke, Radius 200 mm: beide Linien enden im Anschlusspunkt,
/// der Bogen liegt mit Mittelpunkt (200, 200) genau zwischen ihnen.
#[test]
fn rechte_ecke_wird_abgerundet() {
    let mut s = s();
    let waagerecht = linie(&mut s, [0.0, 0.0], [1.0, 0.0]);
    let senkrecht = linie(&mut s, [0.0, 0.0], [0.0, 1.0]);

    let antwort = s
        .execute(
            "modify.fillet",
            &json!({
                "ids": [waagerecht, senkrecht],
                "radius": 0.2,
                "picks": [[0.5, 0.0], [0.0, 0.5]]
            }),
        )
        .expect("Abrunden");

    // Die Linien sind um den Radius gekürzt, ihre Richtung bleibt.
    let (a1, a2) = enden(&s, waagerecht);
    assert_punkt(a1, [200.0, 0.0], "Anfang waagerecht");
    assert_punkt(a2, [1000.0, 0.0], "Ende waagerecht");
    let (b1, b2) = enden(&s, senkrecht);
    assert_punkt(b1, [0.0, 200.0], "Anfang senkrecht");
    assert_punkt(b2, [0.0, 1000.0], "Ende senkrecht");

    // Der Bogen sitzt am Eckpunkt, Radius 200, Viertelkreis.
    let neu = antwort["created"][0].as_u64().expect("Bogen-Kennung");
    match &s.doc.entity(neu).expect("Bogen fehlt").kind {
        EntityKind::Arc { c, r, a0, a1: bogen_a1 } => {
            assert_punkt(*c, [200.0, 200.0], "Bogenmittelpunkt");
            assert!((*r - 200.0).abs() < 1e-6, "Radius {r}");
            let arc = jjcad_core::geom::Arc::new(*c, *r, *a0, *bogen_a1);
            assert!(
                (arc.sweep() - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
                "Bogenwinkel {}",
                arc.sweep()
            );
            // Die Bogenenden liegen auf den gekürzten Linienenden.
            let enden_bogen = [arc.start(), arc.end()];
            assert!(
                enden_bogen.iter().any(|p| p.dist(a1) < 1e-6),
                "Bogen beginnt nicht am Anschlusspunkt: {enden_bogen:?}"
            );
            assert!(
                enden_bogen.iter().any(|p| p.dist(b1) < 1e-6),
                "Bogen endet nicht am Anschlusspunkt: {enden_bogen:?}"
            );
        }
        other => panic!("kein Bogen: {other:?}"),
    }
}

/// Phase: beide Schenkel werden um dasselbe Maß gekürzt und mit einer Geraden
/// verbunden (bei 90° die Diagonale über das Maß).
#[test]
fn rechte_ecke_bekommt_eine_phase() {
    let mut s = s();
    let waagerecht = linie(&mut s, [0.0, 0.0], [1.0, 0.0]);
    let senkrecht = linie(&mut s, [0.0, 0.0], [0.0, 1.0]);

    let antwort = s
        .execute(
            "modify.chamfer",
            &json!({
                "ids": [waagerecht, senkrecht],
                "distance": 0.15,
                "picks": [[0.5, 0.0], [0.0, 0.5]]
            }),
        )
        .expect("Phase");

    let (a1, _) = enden(&s, waagerecht);
    assert_punkt(a1, [150.0, 0.0], "Anfang waagerecht");
    let (b1, _) = enden(&s, senkrecht);
    assert_punkt(b1, [0.0, 150.0], "Anfang senkrecht");

    let neu = antwort["created"][0].as_u64().expect("Fasen-Kennung");
    let (f1, f2) = enden(&s, neu);
    let (kurz, lang) = if f1.dist(v(150.0, 0.0)) < f2.dist(v(150.0, 0.0)) {
        (f1, f2)
    } else {
        (f2, f1)
    };
    assert_punkt(kurz, [150.0, 0.0], "Fase Anfang");
    assert_punkt(lang, [0.0, 150.0], "Fase Ende");
    assert!(
        (kurz.dist(lang) - 150.0 * 2f64.sqrt()).abs() < 1e-6,
        "Fasenlänge {}",
        kurz.dist(lang)
    );
}

/// Die Klickseite entscheidet, welcher Teil der Linie stehen bleibt — auch
/// wenn sich die Linien mitten im Zug kreuzen.
#[test]
fn klickseite_bestimmt_den_bleibenden_schenkel() {
    let mut s = s();
    let waagerecht = linie(&mut s, [0.0, 0.0], [1.0, 0.0]);
    let senkrecht = linie(&mut s, [0.5, -0.5], [0.5, 0.5]);

    s.execute(
        "modify.fillet",
        &json!({
            "ids": [waagerecht, senkrecht],
            "radius": 0.1,
            "picks": [[0.9, 0.0], [0.5, 0.4]]
        }),
    )
    .expect("Abrunden");

    let (a1, a2) = enden(&s, waagerecht);
    assert_punkt(a1, [600.0, 0.0], "Anfang waagerecht (gekürzt)");
    assert_punkt(a2, [1000.0, 0.0], "Ende waagerecht");
    let (b1, b2) = enden(&s, senkrecht);
    assert_punkt(b1, [500.0, 100.0], "Anfang senkrecht (gekürzt)");
    assert_punkt(b2, [500.0, 500.0], "Ende senkrecht");
}

/// Ohne Klickpunkt entscheidet die längere Seite — so funktioniert der
/// Befehl auch aus MCP/Skripten, die keine Klickstellen mitschicken.
#[test]
fn ohne_klickseite_bleibt_die_laengere_linie() {
    let mut s = s();
    let kurz = linie(&mut s, [0.0, 0.0], [0.3, 0.0]);
    let lang = linie(&mut s, [0.0, 0.0], [0.0, 1.0]);
    let e = ecke(&s.doc, kurz, lang, None, None, 100.0, EckeArt::Phase)
        .expect("Phase ohne Klickseite");
    assert_punkt(e.a.0, [100.0, 0.0], "kurze Linie wird gekürzt");
    assert_punkt(e.a.1, [300.0, 0.0], "kurze Linie endet wie vorher");
    assert_punkt(e.b.0, [0.0, 100.0], "lange Linie wird gekürzt");
    assert_punkt(e.b.1, [0.0, 1000.0], "lange Linie endet wie vorher");
}

/// Ein zu großes Maß, parallele Linien und ein Kreis sind Fehler — kein
/// stiller Erfolg mit unbrauchbarem Ergebnis (T0038).
#[test]
fn unbrauchbare_ecken_sind_fehler() {
    let mut s = s();
    let a = linie(&mut s, [0.0, 0.0], [1.0, 0.0]);
    let b = linie(&mut s, [0.0, 0.0], [0.0, 1.0]);
    let too_big = s.execute(
        "modify.fillet",
        &json!({"ids": [a, b], "radius": 2.0, "picks": [[0.5, 0.0], [0.0, 0.5]]}),
    );
    assert!(too_big.is_err(), "Radius über der Linienlänge muss scheitern");

    let c = linie(&mut s, [0.0, 0.5], [1.0, 0.5]);
    let parallel = s.execute("modify.fillet", &json!({"ids": [a, c], "radius": 0.1}));
    assert!(parallel.is_err(), "parallele Linien müssen scheitern");

    s.execute("draw.circle", &json!({"c": [0.2, 0.2], "r": 0.1}))
        .unwrap();
    let kreis = s.doc.entities.last().unwrap().id;
    let falsch = s.execute(
        "modify.chamfer",
        &json!({"ids": [a, kreis], "distance": 0.1}),
    );
    assert!(falsch.is_err(), "ein Kreis ist keine Linie");
}

// ---------------------------------------------------------------------------
// Polylinie und Auswahl über den Schnittpunkt (T0247, Turn 3)
// ---------------------------------------------------------------------------

fn polylinie(s: &mut Session, pts: &[[f64; 2]], closed: bool) -> Id {
    s.execute("draw.polyline", &json!({"pts": pts, "closed": closed}))
        .expect("Polylinie anlegen");
    s.doc.entities.last().unwrap().id
}

fn ecken(s: &Session, id: Id) -> Vec<(Vec2, f64)> {
    match &s.doc.entity(id).expect("Objekt fehlt").kind {
        EntityKind::Polyline { verts, .. } => verts.iter().map(|v| (v.p, v.bulge)).collect(),
        other => panic!("keine Polylinie: {other:?}"),
    }
}

/// Der Knick einer Polylinie wird in **einem** Objekt abgerundet: an die Stelle
/// des Scheitelpunkts treten die beiden Anschlusspunkte, dazwischen ein Bogen.
#[test]
fn knick_der_polylinie_wird_abgerundet() {
    let mut s = s();
    let pl = polylinie(&mut s, &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], false);
    s.execute(
        "modify.fillet",
        &json!({"ids": [pl], "vertex": 1, "radius": 0.2}),
    )
    .expect("Abrunden am Knick");

    let v = ecken(&s, pl);
    assert_eq!(v.len(), 4, "Scheitelpunkt wird zu zwei Punkten");
    assert_punkt(v[1].0, [800.0, 0.0], "Anschluss ankommend");
    assert_punkt(v[2].0, [1000.0, 200.0], "Anschluss abgehend");
    let erwartet = (std::f64::consts::FRAC_PI_8).tan();
    assert!(
        (v[1].1 - erwartet).abs() < 1e-9,
        "Bogenmaß am Anschluss: {} statt {erwartet}",
        v[1].1
    );
    assert!(v[2].1.abs() < 1e-12, "danach wieder gerade");

    // Der Bogen läuft durch den Punkt, der dem Knick am nächsten liegt.
    let arc = jjcad_core::geom::arc_from_bulge(v[1].0, v[2].0, v[1].1).expect("Bogen aus Bulge");
    // Mitte des Viertelkreises um (800, 200): 200/√2 in beiden Richtungen.
    let d = 200.0 * 0.5f64.sqrt();
    assert_punkt(arc.mid(), [800.0 + d, 200.0 - d], "Bogenmitte");
}

/// Phase am Knick: kein Bogen, nur zwei neue Punkte mit gerader Verbindung.
#[test]
fn knick_der_polylinie_bekommt_eine_phase() {
    let mut s = s();
    let pl = polylinie(&mut s, &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], false);
    s.execute(
        "modify.chamfer",
        &json!({"ids": [pl], "vertex": 1, "distance": 0.15}),
    )
    .expect("Phase am Knick");
    let v = ecken(&s, pl);
    assert_eq!(v.len(), 4);
    assert_punkt(v[1].0, [850.0, 0.0], "Schenkel ankommend");
    assert_punkt(v[2].0, [1000.0, 150.0], "Schenkel abgehend");
    assert!(v[1].1.abs() < 1e-12 && v[2].1.abs() < 1e-12, "keine Bögen");
}

/// Auch eine **geschlossene** Polylinie hat an jeder Ecke einen Knick.
#[test]
fn geschlossene_polylinie_hat_einen_knick_am_ersten_punkt() {
    let mut s = s();
    let pl = polylinie(
        &mut s,
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        true,
    );
    s.execute(
        "modify.fillet",
        &json!({"ids": [pl], "vertex": 0, "radius": 0.1}),
    )
    .expect("Abrunden am geschlossenen Zug");
    let v = ecken(&s, pl);
    assert_eq!(v.len(), 5);
    assert_punkt(v[0].0, [0.0, 100.0], "Anschluss ankommend");
    assert_punkt(v[1].0, [100.0, 0.0], "Anschluss abgehend");
}

/// Das offene Ende ist kein Knick — dort fehlt der zweite Schenkel.
#[test]
fn offenes_ende_ist_kein_knick() {
    let mut s = s();
    let pl = polylinie(&mut s, &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], false);
    let r = s.execute(
        "modify.fillet",
        &json!({"ids": [pl], "vertex": 2, "radius": 0.1}),
    );
    assert!(r.is_err(), "das Ende hat keinen Knick");
}

/// „Direkt über den Schnittpunkt": dort laufen **beide** Linien durch, die
/// Auswahl findet sie in einem Schritt.
#[test]
fn schnittpunkt_liefert_beide_linien() {
    let mut s = s();
    let waagerecht = linie(&mut s, [0.0, 0.0], [1.0, 0.0]);
    let senkrecht = linie(&mut s, [0.5, -0.5], [0.5, 0.5]);
    let treffer = linien_durch(&s.doc, v(500.0, 0.0), 8.0);
    assert_eq!(treffer.len(), 2, "beide Linien: {treffer:?}");
    assert!(treffer.contains(&waagerecht) && treffer.contains(&senkrecht));
    // Abseits der Ecke bleibt es bei einer.
    let einer = linien_durch(&s.doc, v(900.0, 0.0), 8.0);
    assert_eq!(einer, vec![waagerecht]);
}

/// Der Knick einer Polylinie ist über seinen Scheitelpunkt zu treffen.
#[test]
fn knick_wird_ueber_den_scheitelpunkt_getroffen() {
    let mut s = s();
    let pl = polylinie(&mut s, &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], false);
    let treffer = polylinien_knoten(&s.doc, v(1005.0, -3.0), 8.0);
    assert_eq!(treffer.map(|(id, i, _)| (id, i)), Some((pl, 1)));
    assert_eq!(polylinien_knoten(&s.doc, v(500.0, 0.0), 8.0), None);
}
