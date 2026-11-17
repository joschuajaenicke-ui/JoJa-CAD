//! Integrationstests für die Befehls-Schicht.

use jjcad_core::geom::{v, Vec2};
use jjcad_core::geom::Vertex;
use jjcad_core::model::{DoorKind, EntityKind};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s
}

fn last_id(s: &Session) -> u64 {
    s.doc.entities.last().unwrap().id
}

#[test]
fn draw_all_primitives() {
    let mut s = s();
    for (action, params) in [
        ("draw.point", json!({"p": [1, 1]})),
        ("draw.line", json!({"a": [0, 0], "b": [2, 0]})),
        ("draw.circle", json!({"c": [1, 1], "r": 0.5})),
        ("draw.arc", json!({"c": [0, 0], "r": 1.0, "a0": 0, "a1": 90})),
        ("draw.ellipse", json!({"c": [0, 0], "rx": 1, "ry": 0.5})),
        ("draw.rect", json!({"a": [0, 0], "b": [3, 2]})),
        ("draw.polyline", json!({"pts": [[0, 0], [1, 1], [2, 0]]})),
        ("draw.hatch", json!({"boundary": [[0, 0], [1, 0], [1, 1], [0, 1]]})),
        ("draw.text", json!({"p": [0, 0], "text": "Hallo", "height": 0.25})),
    ] {
        s.execute(action, &params)
            .unwrap_or_else(|e| panic!("{action}: {e}"));
    }
    assert_eq!(s.doc.entities.len(), 9);
    // Ellipse-Radien in mm
    let ellipse = s.doc.entities.iter().find_map(|e| match &e.kind {
        EntityKind::Ellipse { rx, ry, .. } => Some((*rx, *ry)),
        _ => None,
    });
    assert_eq!(ellipse, Some((1000.0, 500.0)));
}

/// Rechteck über Basislinie und Höhe: `a`→`b` ist die Basislinie in
/// beliebiger Lage, `height` der senkrechte Abstand (Vorzeichen = Seite).
#[test]
fn rechteck_ueber_basislinie_und_hoehe() {
    let mut s = s();
    let base_laenge = (3.0f64 * 3.0 + 3.0 * 3.0).sqrt() * 1000.0; // 4242,64 mm
    s.execute(
        "draw.rect",
        &json!({"a": [0, 0], "b": [3, 3], "height": 1}),
    )
    .unwrap();
    let e = s.doc.entities.last().unwrap();
    let (verts, closed) = match &e.kind {
        EntityKind::Polyline { verts, closed } => (verts, *closed),
        other => panic!("Rechteck muss eine geschlossene Polylinie sein: {other:?}"),
    };
    assert!(closed, "Rechteck ist geschlossen");
    assert_eq!(verts.len(), 4, "vier Ecken");
    let p: Vec<Vec2> = verts.iter().map(|q| q.p).collect();

    // Erste Kante ist die Basislinie selbst.
    assert!(p[0].dist(v(0.0, 0.0)) < 1e-6, "{:?}", p[0]);
    assert!(p[1].dist(v(3000.0, 3000.0)) < 1e-6, "{:?}", p[1]);
    // Höhe steht senkrecht auf der Basislinie und misst genau 1 m.
    let n = p[2] - p[1];
    assert!((n.len() - 1000.0).abs() < 1e-6, "Höhe: {}", n.len());
    let basis = p[1] - p[0];
    assert!(
        (basis.x * n.x + basis.y * n.y).abs() < 1.0,
        "Höhe muss senkrecht auf der Basislinie stehen"
    );
    // Die vierte Ecke schließt das Parallelogramm.
    assert!(p[3].dist(p[0] + n) < 1e-6, "{:?}", p[3]);
    assert!((p[0].dist(p[1]) - base_laenge).abs() < 0.01);

    // Positiv liegt links der Richtung a→b, negativ auf der anderen Seite.
    s.execute(
        "draw.rect",
        &json!({"a": [0, 0], "b": [3, 3], "height": -1}),
    )
    .unwrap();
    let e = s.doc.entities.last().unwrap();
    if let EntityKind::Polyline { verts, .. } = &e.kind {
        assert!(
            (verts[2].p - v(3000.0 + 707.1067811865475, 3000.0 - 707.1067811865475)).len() < 0.01,
            "negative Höhe spiegelt die Seite: {:?}",
            verts[2].p
        );
    } else {
        panic!("Polylinie erwartet");
    }

    // Null-Höhe und entartete Basislinie legen nichts an.
    assert!(s
        .execute("draw.rect", &json!({"a": [0, 0], "b": [3, 0], "height": 0}))
        .is_err());
    assert!(s
        .execute("draw.rect", &json!({"a": [1, 1], "b": [1, 1], "height": 1}))
        .is_err());
    assert_eq!(s.doc.entities.len(), 2, "nur die zwei gültigen Rechtecke");
}

#[test]
fn architecture_objects_and_openings() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [6, 0], "thickness": 0.30}),
    )
    .unwrap();
    let wall_id = last_id(&s);
    let res = s
        .execute(
            "draw.window",
            &json!({"center": [3, 0], "width": 1.4}),
        )
        .unwrap();
    assert_eq!(res["host"], json!(wall_id), "Fenster muss der Wand zugeordnet werden");

    // Öffnungsintervall muss in der Wand liegen
    let wall = s.doc.entity(wall_id).unwrap().clone();
    let intervals = jjcad_core::render::wall_opening_intervals(&s.doc, &wall);
    assert_eq!(intervals.len(), 1);
    let (a, b) = intervals[0];
    assert!((a - 2300.0).abs() < 1.0, "Öffnung startet bei 2300 mm: {a}");
    assert!((b - 3700.0).abs() < 1.0);

    s.execute("draw.door", &json!({"center": [1, 0], "width": 0.885}))
        .unwrap();
    s.execute("draw.stair", &json!({"a": [7, 1], "b": [7, 4], "width": 1.0, "steps": 12}))
        .unwrap();
    assert_eq!(s.doc.entities.len(), 4);
}

#[test]
fn modify_transformations() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]})).unwrap();
    let id = last_id(&s);

    s.execute("modify.move", &json!({"ids": [id], "from": [0, 0], "to": [1, 1]}))
        .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x - 1000.0).abs() < 1e-6 && (a.y - 1000.0).abs() < 1e-6);
        assert!((b.x - 3000.0).abs() < 1e-6);
    } else {
        panic!("Linie erwartet");
    }

    // Kopie erzeugt neues Objekt
    s.execute("modify.copy", &json!({"ids": [id], "from": [0, 0], "to": [0, 2]}))
        .unwrap();
    assert_eq!(s.doc.entities.len(), 2);

    // Drehen um 90° um den Ursprung
    s.execute("modify.rotate", &json!({"ids": [id], "center": [0, 0], "angle": 90}))
        .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x + 1000.0).abs() < 1e-6 && (a.y - 1000.0).abs() < 1e-6, "{a:?}");
        assert!((b.x + 1000.0).abs() < 1e-6 && (b.y - 3000.0).abs() < 1e-6, "{b:?}");
    }

    // Spiegeln an der X-Achse
    s.execute("modify.mirror", &json!({"ids": [id], "p1": [0, 0], "p2": [1, 0], "keep_original": false}))
        .unwrap();
    if let EntityKind::Line { a, .. } = &s.doc.entity(id).unwrap().kind {
        assert!((a.y + 1000.0).abs() < 1e-6, "y muss negativ sein: {a:?}");
    }

    // Skalieren
    s.execute("modify.scale", &json!({"ids": [id], "center": [0, 0], "factor": 2.0}))
        .unwrap();
    if let EntityKind::Line { b, .. } = &s.doc.entity(id).unwrap().kind {
        assert!((b.y + 6000.0).abs() < 1e-6 && (b.x + 2000.0).abs() < 1e-6, "{b:?}");
    }
}

#[test]
fn trim_and_extend() {
    let mut s = s();
    // Zwei senkrechte Schnittkanten bei x=2 und x=4, dazwischen eine Linie
    s.execute("draw.line", &json!({"a": [2, -1], "b": [2, 1]})).unwrap();
    s.execute("draw.line", &json!({"a": [4, -1], "b": [4, 1]})).unwrap();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [6, 0]})).unwrap();
    let id = last_id(&s);

    // Mittleres Stück (x 2..4) entfernen -> zwei Linien
    s.execute("modify.trim", &json!({"ids": [id], "at": [3, 0]}))
        .unwrap();
    let lines: Vec<(Vec2, Vec2)> = s
        .doc
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Line { a, b } if a.y.abs() < 1e-6 && b.y.abs() < 1e-6 => Some((*a, *b)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 2, "Stutzen muss die Linie teilen: {lines:?}");

    // Verlängern: rechte Linie endet bei x=4 -> soll bis x=2 verlängert werden? Nein: bis zur nächsten Kante
    let right = s
        .doc
        .entities
        .iter()
        .find(|e| matches!(&e.kind, EntityKind::Line{a,b} if a.y.abs()<1e-6 && b.y.abs()<1e-6 && a.x > 3900.0))
        .map(|e| e.id)
        .expect("rechte Teillinie");
    s.execute("modify.extend", &json!({"ids": [right], "at": [4, 0], "cutters": []}))
        .unwrap_or_else(|e| panic!("Verlängern: {e}"));
}

/// Vorschau des Stutzens (T0075): der hervorgehobene Abschnitt ist genau das
/// Stück, das der Klick danach entfernt — nicht die ganze Linie.
#[test]
fn trim_vorschau_zeigt_den_abschnitt_der_wegfaellt() {
    let mut s = s();
    // Schnittkanten bei x = 2000 und x = 4000 mm, Linie von 0 bis 6000 mm
    s.execute("draw.line", &json!({"a": [2, -1], "b": [2, 1]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [4, -1], "b": [4, 1]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [6, 0]}))
        .unwrap();
    let id = last_id(&s);

    let segs = jjcad_core::actions::trim_vorschau(&s.doc, id, v(3000.0, 0.0), &[]);
    assert_eq!(segs.len(), 1, "ein Abschnitt: {segs:?}");
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Linie muss einen Streckenabschnitt liefern: {:?}", segs[0]);
    };
    assert!(
        (a.x - 2000.0).abs() < 1e-6 && a.y.abs() < 1e-6,
        "Anfang an der ersten Kante: {a:?}"
    );
    assert!(
        (b.x - 4000.0).abs() < 1e-6 && b.y.abs() < 1e-6,
        "Ende an der zweiten Kante: {b:?}"
    );

    // Links der ersten Kante: nur das erste Stück
    let segs = jjcad_core::actions::trim_vorschau(&s.doc, id, v(1000.0, 0.0), &[]);
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Strecke erwartet");
    };
    assert!((a.x).abs() < 1e-6 && (b.x - 2000.0).abs() < 1e-6, "{a:?} {b:?}");

    // Die Vorschau ändert die Zeichnung nicht.
    assert_eq!(s.doc.entities.len(), 3, "Vorschau legt nichts an");

    // Und der Schnitt entfernt genau diesen Bereich.
    s.execute("modify.trim", &json!({"ids": [id], "at": [3, 0]}))
        .unwrap();
    let mut rest: Vec<(f64, f64)> = Vec::new();
    for e in &s.doc.entities {
        if let EntityKind::Line { a, b } = &e.kind {
            if a.y.abs() < 1e-6 && b.y.abs() < 1e-6 && a.x < b.x {
                rest.push((a.x, b.x));
            }
        }
    }
    rest.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    assert_eq!(
        rest,
        vec![(0.0, 2000.0), (4000.0, 6000.0)],
        "der hervorgehobene Abschnitt ist weg, der Rest bleibt"
    );
}

/// Vorschau des Verlängerns (T0081): die hervorgehobene Strecke ist genau das
/// Stück, das der Klick danach ansetzt — bis zur **nächsten** Schnittkante und
/// nur am angeklickten Ende.
#[test]
fn extend_vorschau_zeigt_die_strecke_die_hinzukommt() {
    let mut s = s();
    // Schnittkanten bei x = 2000 und x = 4000 mm
    s.execute("draw.line", &json!({"a": [2, -1], "b": [2, 1]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [4, -1], "b": [4, 1]}))
        .unwrap();
    // Die zu verlängernde Linie endet bei 1000 mm.
    s.execute("draw.line", &json!({"a": [0, 0], "b": [1, 0]}))
        .unwrap();
    let id = last_id(&s);

    let segs = jjcad_core::actions::extend_vorschau(&s.doc, id, v(1000.0, 0.0), &[]);
    assert_eq!(segs.len(), 1, "eine Strecke: {segs:?}");
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Strecke erwartet: {:?}", segs[0]);
    };
    assert!(
        (a.x - 1000.0).abs() < 1e-6 && a.y.abs() < 1e-6,
        "Anfang am wandernden Ende: {a:?}"
    );
    assert!(
        (b.x - 2000.0).abs() < 1e-6 && b.y.abs() < 1e-6,
        "Ende an der naechsten Kante: {b:?}"
    );
    // Die Vorschau ändert die Zeichnung nicht.
    assert_eq!(s.doc.entities.len(), 3, "Vorschau legt nichts an");

    // Und der Klick setzt genau diese Strecke an.
    s.execute(
        "modify.extend",
        &json!({"ids": [id], "at": [1, 0], "cutters": []}),
    )
    .unwrap_or_else(|e| panic!("Verlängern: {e}"));
    let e = s.doc.entity(id).expect("Linie fehlt");
    let EntityKind::Line { a, b } = &e.kind else {
        panic!("Linie erwartet");
    };
    assert!(
        (b.x - 2000.0).abs() < 1e-6 && b.y.abs() < 1e-6,
        "das Ergebnis endet wie die Vorschau: {b:?}"
    );
    assert!(a.x.abs() < 1e-6, "das andere Ende bleibt stehen: {a:?}");

    // Klick am linken Ende: es wandert nach links, bis zur Kante bei 4000.
    s.execute("draw.line", &json!({"a": [5, 0], "b": [6, 0]}))
        .unwrap();
    let links = last_id(&s);
    let segs = jjcad_core::actions::extend_vorschau(&s.doc, links, v(5000.0, 0.0), &[]);
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Strecke erwartet: {segs:?}");
    };
    assert!(
        (a.x - 5000.0).abs() < 1e-6 && (b.x - 4000.0).abs() < 1e-6,
        "das angeklickte Ende wandert: {a:?} nach {b:?}"
    );

    // Ohne Schnittkante hinter dem Ende gibt es nichts zu zeigen.
    s.execute("draw.line", &json!({"a": [7, 0], "b": [8, 0]}))
        .unwrap();
    let frei = last_id(&s);
    assert!(
        jjcad_core::actions::extend_vorschau(&s.doc, frei, v(8000.0, 0.0), &[]).is_empty(),
        "ohne Kante keine Vorschau"
    );
}

/// Vorschau des Verlängerns bei einer Wand (T0081): die Strecke läuft bis zur
/// Basislinie der Nachbarwand — Schnittkanten sind die Basislienen der Wände
/// (`render::entity_segments`), also landet die verlängerte Wand im Knoten.
#[test]
fn extend_vorschau_verlaengert_eine_wand_bis_zur_nachbarwand() {
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 3], "b": [2, 3], "thickness": 0.24}),
    )
    .unwrap();
    let id = last_id(&s);
    s.execute(
        "draw.wall",
        &json!({"a": [5, 1.5], "b": [5, 4.5], "thickness": 0.24}),
    )
    .unwrap();

    let segs = jjcad_core::actions::extend_vorschau(&s.doc, id, v(2000.0, 3000.0), &[]);
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Strecke erwartet: {segs:?}");
    };
    assert!((a.x - 2000.0).abs() < 1e-6, "Anfang am Wandende: {a:?}");
    // Die Nachbarwand liegt bei x = 5000, 240 mm dick, mittig: ihre
    // **Umrandung** ist die Kante bei x = 4880 (T0207 — vorher endete die
    // Verlängerung auf der Basislinie mitten im Wandkörper).
    assert!(
        (b.x - 4880.0).abs() < 1e-6 && (b.y - 3000.0).abs() < 1e-6,
        "Ende auf der Umrandung der Nachbarwand: {b:?}"
    );

    s.execute(
        "modify.extend",
        &json!({"ids": [id], "at": [2, 3], "cutters": []}),
    )
    .unwrap_or_else(|e| panic!("Verlängern: {e}"));
    let EntityKind::Wall { b, .. } = &s.doc.entity(id).expect("Wand fehlt").kind else {
        panic!("Wand erwartet");
    };
    assert!(
        (b.x - 4880.0).abs() < 1e-6,
        "die Wand endet wie die Vorschau: {b:?}"
    );
}

/// Verlängern einer Polylinie (T0155): das Ende näher am Klick wandert bis zur
/// nächsten Schnittkante, der Knick und der Anfang bleiben stehen.
#[test]
fn polylinie_verlaengern_zieht_das_ende_bis_zur_kante() {
    let mut s = s();
    // Schnittkante über dem Ende: Waagerechte bei y = 3 m.
    s.execute("draw.line", &json!({"a": [0, 3], "b": [4, 3]}))
        .unwrap();
    let res = s
        .execute("draw.polyline", &json!({"pts": [[0, 0], [1, 0], [1, 2]]}))
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    // Vorschau: genau die Strecke, die angesetzt wird.
    let segs = jjcad_core::actions::extend_vorschau(&s.doc, id, v(1000.0, 2000.0), &[]);
    assert_eq!(segs.len(), 1, "eine Strecke: {segs:?}");
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Strecke erwartet: {:?}", segs[0]);
    };
    assert!(
        (a.x - 1000.0).abs() < 1e-6 && (a.y - 2000.0).abs() < 1e-6,
        "Anfang am wandernden Ende: {a:?}"
    );
    assert!(
        (b.x - 1000.0).abs() < 1e-6 && (b.y - 3000.0).abs() < 1e-6,
        "Ende an der Kante: {b:?}"
    );
    assert_eq!(s.doc.entities.len(), 2, "Vorschau legt nichts an");

    s.execute(
        "modify.extend",
        &json!({"ids": [id], "at": [1, 2], "cutters": []}),
    )
    .unwrap_or_else(|e| panic!("Verlängern der Polylinie: {e}"));
    let EntityKind::Polyline { verts, closed } = &s.doc.entity(id).expect("Polylinie fehlt").kind
    else {
        panic!("Polylinie erwartet");
    };
    assert!(!closed, "der Zug bleibt offen");
    assert_eq!(verts.len(), 3, "es kommt kein Punkt hinzu: {verts:?}");
    assert!(
        (verts[2].p.x - 1000.0).abs() < 1e-6 && (verts[2].p.y - 3000.0).abs() < 1e-6,
        "letzter Punkt an der Kante: {:?}",
        verts[2].p
    );
    assert!(
        (verts[1].p.x - 1000.0).abs() < 1e-6 && verts[1].p.y.abs() < 1e-6,
        "der Knick bleibt stehen: {:?}",
        verts[1].p
    );
    assert!(verts[0].p.x.abs() < 1e-6, "der Anfang bleibt: {:?}", verts[0].p);
}

/// Verlängern am **ersten** Punkt: dort läuft die Polylinie rückwärts — die
/// Richtung kommt vom Nachbarn, nicht von der Reihenfolge der Punkte.
#[test]
fn polylinie_verlaengern_am_ersten_punkt() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [-2, -1], "b": [-2, 1]}))
        .unwrap();
    let res = s
        .execute("draw.polyline", &json!({"pts": [[0, 0], [1, 0], [1, 2]]}))
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    s.execute(
        "modify.extend",
        &json!({"ids": [id], "at": [0, 0], "cutters": []}),
    )
    .unwrap_or_else(|e| panic!("Verlängern am ersten Punkt: {e}"));
    let EntityKind::Polyline { verts, .. } = &s.doc.entity(id).expect("Polylinie fehlt").kind
    else {
        panic!("Polylinie erwartet");
    };
    assert!(
        (verts[0].p.x + 2000.0).abs() < 1e-6 && verts[0].p.y.abs() < 1e-6,
        "erster Punkt wandert auf die Kante bei x = -2: {:?}",
        verts[0].p
    );
    assert!(
        (verts[1].p.x - 1000.0).abs() < 1e-6,
        "der Rest bleibt stehen: {:?}",
        verts[1].p
    );
}

/// Eine geschlossene Polylinie hat kein Ende — die Action meldet das, statt
/// still nichts zu tun.
#[test]
fn geschlossene_polylinie_laesst_sich_nicht_verlaengern() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [3, -1], "b": [3, 1]}))
        .unwrap();
    let res = s
        .execute(
            "draw.polyline",
            &json!({"pts": [[0, 0], [2, 0], [2, 2], [0, 2]], "closed": true}),
        )
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();
    let vorher = s.doc.entity(id).expect("Polylinie fehlt").kind.clone();

    let err = s
        .execute(
            "modify.extend",
            &json!({"ids": [id], "at": [0, 0], "cutters": []}),
        )
        .expect_err("geschlossene Polylinie darf nicht verlängert werden");
    assert!(err.contains("Geschlossene Polylinie"), "{err}");
    assert_eq!(
        s.doc.entity(id).expect("Polylinie fehlt").kind,
        vorher,
        "die Zeichnung bleibt unverändert"
    );
    assert!(
        jjcad_core::actions::extend_vorschau(&s.doc, id, v(0.0, 0.0), &[]).is_empty(),
        "auch die Vorschau zeigt nichts"
    );
}

/// Ein Endsegment mit Stützbogen wächst auf seinem **eigenen Kreis** weiter
/// (T0155): der Bogen läuft in seiner Laufrichtung bis zur nächsten Kante, der
/// Mittelpunkt bleibt.
#[test]
fn polylinie_verlaengern_folgt_dem_bogen() {
    let mut s = s();
    // Schnittkante bei y = 0,5 m.
    s.execute("draw.line", &json!({"a": [-1, 0.5], "b": [3, 0.5]}))
        .unwrap();
    let res = s
        .execute("draw.polyline", &json!({"pts": [[-2, 0], [0, 0], [2, 0]]}))
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();
    // Letztes Segment (0,0)→(2,0) als Halbkreis: Mitte (1,0), Radius 1.
    if let Some(em) = s.doc.entity_mut(id) {
        if let EntityKind::Polyline { verts, .. } = &mut em.kind {
            verts[1].bulge = 1.0;
        }
    }

    s.execute(
        "modify.extend",
        &json!({"ids": [id], "at": [2, 0], "cutters": []}),
    )
    .unwrap_or_else(|e| panic!("Verlängern des Bogens: {e}"));
    let EntityKind::Polyline { verts, .. } = &s.doc.entity(id).expect("Polylinie fehlt").kind
    else {
        panic!("Polylinie erwartet");
    };
    // Die Krümmung sitzt am **Anfang** des Endsegments (Punkt 1).
    let neu = verts[2];
    let krümmung = verts[1].bulge;
    // Der nächste Schnitt **in Laufrichtung** (Gegenrichtung träfe den zweiten
    // Punkt des Kreises auf derselben Kante).
    assert!(
        (neu.p.x - 1866.0254).abs() < 0.01 && (neu.p.y - 500.0).abs() < 1e-6,
        "Ende am nächsten Schnitt in Laufrichtung: {:?}",
        neu.p
    );
    assert!(
        (krümmung - 1.30323).abs() < 1e-4,
        "neue Krümmung tan(θ/4) des größeren Bogens: {krümmung}"
    );
    // Das neue Ende liegt auf dem Kreis um (1,0) mit r = 1 …
    let c = v(1000.0, 0.0);
    assert!(
        (c.dist(neu.p) - 1000.0).abs() < 1e-3,
        "Ende bleibt auf dem Kreis: {:?}",
        neu.p
    );
    // … und die neue Krümmung beschreibt denselben Kreis.
    let arc = jjcad_core::geom::arc_from_bulge(verts[1].p, neu.p, krümmung).expect("Bogen");
    assert!(
        (arc.c.x - 1000.0).abs() < 1e-3 && arc.c.y.abs() < 1e-3,
        "Mittelpunkt bleibt: {:?}",
        arc.c
    );
    assert!(
        (arc.r - 1000.0).abs() < 1e-3,
        "Radius bleibt: {}",
        arc.r
    );
    assert!(
        arc.sweep() > std::f64::consts::PI,
        "der Bogen ist gewachsen: {}",
        arc.sweep()
    );
    assert!(
        (neu.p.y - 500.0).abs() < 1e-3,
        "Ende auf der Schnittkante: {:?}",
        neu.p
    );
}

/// Stutzen einer Polylinie (T0077): der Abschnitt zwischen den beiden
/// Schnittkanten fällt weg. Bleibt ein Stück mit zwei Punkten, ist es danach
/// eine normale **Linie**, kein Zwei-Punkt-Polygonzug.
#[test]
fn polylinie_stutzen_teilt_den_zug() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [2, -1], "b": [2, 1]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [4, -1], "b": [4, 1]}))
        .unwrap();
    let res = s
        .execute("draw.polyline", &json!({"pts": [[0, 0], [6, 0], [6, 4]]}))
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    let res = s
        .execute("modify.trim", &json!({"ids": [id], "at": [3, 0]}))
        .unwrap_or_else(|e| panic!("Stutzen der Polylinie: {e}"));
    assert_eq!(
        res["created"].as_array().unwrap().len(),
        1,
        "das zweite Stück ist ein neues Objekt: {res}"
    );

    let mut linien: Vec<(f64, f64)> = Vec::new();
    let mut zuege: Vec<Vec<(f64, f64)>> = Vec::new();
    for e in &s.doc.entities {
        match &e.kind {
            EntityKind::Line { a, b } if a.y.abs() < 1e-6 && b.y.abs() < 1e-6 => {
                linien.push((a.x, b.x))
            }
            EntityKind::Polyline { verts, .. } => zuege.push(
                verts
                    .iter()
                    .map(|q| (q.p.x, q.p.y))
                    .collect::<Vec<_>>(),
            ),
            _ => {}
        }
    }
    assert_eq!(
        linien,
        vec![(0.0, 2000.0)],
        "das linke Stück (zwei Punkte) ist eine Linie: {linien:?}"
    );
    assert_eq!(zuege.len(), 1, "das rechte Stück bleibt Polygonzug");
    assert_eq!(
        zuege[0],
        vec![(4000.0, 0.0), (6000.0, 0.0), (6000.0, 4000.0)],
        "rechts bleibt der Zug ab der zweiten Kante"
    );
}

/// Aus einer Polylinie mit zwei Punkten wird beim Stutzen eine einzelne
/// normale Linie — das Objekt selbst bleibt, nur die Art wechselt.
///
/// Seit T0149 entsteht ein Zwei-Punkt-Zug gar nicht mehr beim Zeichnen; so eine
/// Polylinie steht aber noch in älteren Dateien. Deshalb hier direkt ins
/// Dokument gelegt statt über `draw.polyline`.
#[test]
fn gestutzte_polylinie_wird_zur_linie() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [3, -1], "b": [3, 1]}))
        .unwrap();
    let id = s.doc.new_id();
    let e = jjcad_core::model::Entity::new(
        id,
        s.doc.layers[0].id,
        EntityKind::Polyline {
            verts: vec![
                Vertex::new(v(0.0, 0.0)),
                Vertex::new(v(6000.0, 0.0)),
            ],
            closed: false,
        },
    );
    s.doc.add_entity(e);

    s.execute("modify.trim", &json!({"ids": [id], "at": [1, 0]}))
        .unwrap_or_else(|e| panic!("Stutzen: {e}"));
    let e = s.doc.entity(id).expect("das Objekt bleibt stehen");
    match &e.kind {
        EntityKind::Line { a, b } => {
            assert!(
                (a.x - 3000.0).abs() < 1e-6 && a.y.abs() < 1e-6,
                "Anfang liegt auf der Schnittkante: {a:?}"
            );
            assert!(
                (b.x - 6000.0).abs() < 1e-6 && b.y.abs() < 1e-6,
                "Ende bleibt am Objektende: {b:?}"
            );
        }
        k => panic!("aus der Polylinie muss eine Linie werden: {k:?}"),
    }
    assert_eq!(s.doc.entities.len(), 2, "nur Schnittkante und Linie");
}

/// Geschlossene Polylinie: der Ring wird am Schnitt aufgetrennt, alles andere
/// bleibt als **offener** Zug stehen.
#[test]
fn geschlossene_polylinie_wird_beim_stutzen_aufgetrennt() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [1, -1], "b": [1, 1]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [3, -1], "b": [3, 1]}))
        .unwrap();
    let res = s
        .execute(
            "draw.polyline",
            &json!({"pts": [[0, 0], [4, 0], [4, 4], [0, 4]], "closed": true}),
        )
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    s.execute("modify.trim", &json!({"ids": [id], "at": [2, 0]}))
        .unwrap_or_else(|e| panic!("Stutzen des Rings: {e}"));
    let e = s.doc.entity(id).expect("Objekt bleibt");
    let EntityKind::Polyline { verts, closed } = &e.kind else {
        panic!("Polygonzug erwartet: {:?}", e.kind);
    };
    assert!(!*closed, "der Ring ist aufgetrennt");
    let pts: Vec<(f64, f64)> = verts.iter().map(|q| (q.p.x, q.p.y)).collect();
    assert_eq!(
        pts,
        vec![
            (3000.0, 0.0),
            (4000.0, 0.0),
            (4000.0, 4000.0),
            (0.0, 4000.0),
            (0.0, 0.0),
            (1000.0, 0.0),
        ],
        "einmal herum vom Schnittende zum Schnittanfang"
    );
}

/// Die Stutzen-Vorschau zeigt bei einer Polylinie genau das Stück, das der
/// Klick danach entfernt — und ändert die Zeichnung nicht.
#[test]
fn stutzvorschau_zeigt_das_stueck_der_polylinie() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [2, -1], "b": [2, 1]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [4, -1], "b": [4, 1]}))
        .unwrap();
    let res = s
        .execute("draw.polyline", &json!({"pts": [[0, 0], [6, 0], [6, 4]]}))
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    let segs = jjcad_core::actions::trim_vorschau(&s.doc, id, v(3000.0, 0.0), &[]);
    assert_eq!(segs.len(), 1, "ein Abschnitt: {segs:?}");
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Strecke erwartet: {:?}", segs[0]);
    };
    assert!(
        (a.x - 2000.0).abs() < 1e-6 && (b.x - 4000.0).abs() < 1e-6,
        "Vorschau zwischen den Kanten: {a:?} {b:?}"
    );
    assert_eq!(s.doc.entities.len(), 3, "die Vorschau legt nichts an");
}

/// Bogensegment einer Polylinie: geschnitten wird auf dem **Bogen**, und die
/// beiden Reste bleiben Bögen mit demselben Mittelpunkt und Radius.
#[test]
fn bogen_segment_einer_polylinie_laesst_sich_stutzen() {
    use jjcad_core::geom::Vertex;
    use jjcad_core::model::Entity;
    let mut s = s();
    s.execute("draw.line", &json!({"a": [1, -3], "b": [1, 1]}))
        .unwrap();
    s.execute("draw.line", &json!({"a": [3, -3], "b": [3, 1]}))
        .unwrap();
    // Halbkreis (bulge 1) von (0|0) nach (4|0), Mittelpunkt (2|0), r = 2000 mm
    let verts = vec![Vertex::arc(v(0.0, 0.0), 1.0), Vertex::new(v(4000.0, 0.0))];
    let layer = s.doc.current_layer;
    let id = s
        .doc
        .add_entity(Entity::new(0, layer, EntityKind::Polyline { verts, closed: false }));

    // Klick mitten auf dem Bogen: (2000 | −2000)
    s.execute("modify.trim", &json!({"ids": [id], "at": [2, -2]}))
        .unwrap_or_else(|e| panic!("Stutzen des Bogens: {e}"));

    let mut boegen: Vec<(Vec2, f64)> = Vec::new();
    for e in &s.doc.entities {
        if let EntityKind::Arc { c, r, .. } = &e.kind {
            boegen.push((*c, *r));
        }
    }
    assert_eq!(boegen.len(), 2, "zwei Bogenstücke: {boegen:?}");
    for (c, r) in &boegen {
        assert!(
            c.dist(v(2000.0, 0.0)) < 1e-3 && (r - 2000.0).abs() < 1e-3,
            "beide Stücke liegen auf demselben Kreis: {c:?} r={r}"
        );
    }
    // Die Reste enden auf den Schnittkanten x = 1000 und x = 3000.
    let mut enden: Vec<f64> = Vec::new();
    for e in &s.doc.entities {
        if let EntityKind::Arc { c, r, a0, a1 } = &e.kind {
            let arc = jjcad_core::geom::Arc::new(*c, *r, *a0, *a1);
            enden.push(arc.start().x);
            enden.push(arc.end().x);
        }
    }
    enden.sort_by(|x, y| x.partial_cmp(y).unwrap());
    assert!(
        (enden[0]).abs() < 1e-3 && (enden[3] - 4000.0).abs() < 1e-3,
        "die Außenenden bleiben: {enden:?}"
    );
    assert!(
        (enden[1] - 1000.0).abs() < 1e-3 && (enden[2] - 3000.0).abs() < 1e-3,
        "die Schnittenden liegen auf den Kanten: {enden:?}"
    );
}

/// Polylinie, die sich selbst kreuzt (T0077, Turn 2): der eigene kreuzende
/// Schenkel ist eine Schnittkante. Gestutzt wird bis zum Kreuzungspunkt —
/// früher fiel der ganze Schenkel weg.
#[test]
fn selbst_gekreuzte_polylinie_stutzt_am_kreuzungspunkt() {
    let mut s = s();
    // Ein „X": (0|0)→(3|3)→(0|3)→(3|0); Schenkel 0 und 2 kreuzen sich bei (1,5|1,5).
    let res = s
        .execute(
            "draw.polyline",
            &json!({"pts": [[0, 0], [3, 3], [0, 3], [3, 0]]}),
        )
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    // Klick auf dem ersten Schenkel, vor dem Kreuzungspunkt.
    s.execute("modify.trim", &json!({"ids": [id], "at": [0.75, 0.75]}))
        .unwrap_or_else(|e| panic!("Stutzen: {e}"));

    assert_eq!(s.doc.entities.len(), 1, "nur die gestutzte Polylinie");
    let e = s.doc.entity(id).expect("Objekt bleibt");
    let EntityKind::Polyline { verts, .. } = &e.kind else {
        panic!("Polygonzug erwartet: {:?}", e.kind);
    };
    let pts: Vec<(f64, f64)> = verts.iter().map(|q| (q.p.x, q.p.y)).collect();
    let soll = [
        (1500.0, 1500.0),
        (3000.0, 3000.0),
        (0.0, 3000.0),
        (3000.0, 0.0),
    ];
    assert_eq!(pts.len(), soll.len(), "Punkte: {pts:?}");
    for (i, (ist, erwartet)) in pts.iter().zip(soll.iter()).enumerate() {
        assert!(
            (ist.0 - erwartet.0).abs() < 1e-6 && (ist.1 - erwartet.1).abs() < 1e-6,
            "Punkt {i}: {ist:?} statt {erwartet:?} (alle: {pts:?})"
        );
    }
    // Der erste Punkt liegt auf der eigenen Kreuzung der Schenkel.
    assert!(
        pts[0].0.abs() - pts[0].1.abs() < 1e-6,
        "Kreuzung der Schenkel liegt auf der Winkelhalbierenden: {pts:?}"
    );
}

/// Die Vorschau zeigt bei einer selbst gekreuzten Polylinie dasselbe Stück
/// (Anfang bis Kreuzungspunkt), das der Klick danach entfernt.
#[test]
fn selbst_gekreuzte_polylinie_zeigt_die_vorschau_bis_zur_kreuzung() {
    let mut s = s();
    let res = s
        .execute(
            "draw.polyline",
            &json!({"pts": [[0, 0], [3, 3], [0, 3], [3, 0]]}),
        )
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    let segs = jjcad_core::actions::trim_vorschau(&s.doc, id, v(750.0, 750.0), &[]);
    assert_eq!(segs.len(), 1, "ein Abschnitt: {segs:?}");
    let jjcad_core::geom::Seg::Line(a, b) = segs[0] else {
        panic!("Strecke erwartet: {:?}", segs[0]);
    };
    assert!(
        a.dist(v(0.0, 0.0)) < 1e-6 && b.dist(v(1500.0, 1500.0)) < 1e-6,
        "Vorschau vom Anfang bis zur Kreuzung: {a:?} {b:?}"
    );
    assert_eq!(s.doc.entities.len(), 1, "die Vorschau legt nichts an");
}

/// Zwei eigene Kreuzungen auf einem Schenkel: das Stück dazwischen fällt weg,
/// links bleibt eine Linie, rechts der restliche Polygonzug.
#[test]
fn zwei_eigene_kreuzungen_schneiden_das_mittelstueck_heraus() {
    let mut s = s();
    // Der waagerechte Schenkel 0 wird von Schenkel 2 (x=5) und Schenkel 4 (x=2)
    // gekreuzt.
    let res = s
        .execute(
            "draw.polyline",
            &json!({"pts": [[0, 0], [6, 0], [6, -2], [4, 2], [2, 2], [2, -3]]}),
        )
        .unwrap();
    let id = res["created"][0].as_u64().unwrap();

    s.execute("modify.trim", &json!({"ids": [id], "at": [3.5, 0]}))
        .unwrap_or_else(|e| panic!("Stutzen: {e}"));

    let mut linien: Vec<(f64, f64)> = Vec::new();
    let mut zuege: Vec<Vec<(f64, f64)>> = Vec::new();
    for e in &s.doc.entities {
        match &e.kind {
            EntityKind::Line { a, b } => linien.push((a.x, b.x)),
            EntityKind::Polyline { verts, .. } => zuege.push(
                verts
                    .iter()
                    .map(|q| (q.p.x, q.p.y))
                    .collect::<Vec<_>>(),
            ),
            _ => {}
        }
    }
    assert_eq!(linien, vec![(0.0, 2000.0)], "links bis zur ersten Kreuzung");
    assert_eq!(zuege.len(), 1, "rechts bleibt ein Polygonzug");
    assert_eq!(
        zuege[0],
        vec![
            (5000.0, 0.0),
            (6000.0, 0.0),
            (6000.0, -2000.0),
            (4000.0, 2000.0),
            (2000.0, 2000.0),
            (2000.0, -3000.0),
        ],
        "rechts ab der zweiten Kreuzung"
    );
}

#[test]
fn layers_and_blocks() {
    let mut s = s();
    s.execute("layer.create", &json!({"name": "Testlayer", "color": "#FF0000"}))
        .unwrap();
    let lid = s.doc.layer_id_by_name("Testlayer").unwrap();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [1, 0], "layer": "Testlayer"}))
        .unwrap();
    assert_eq!(s.doc.entities[0].layer, lid);

    // Block aus Auswahl
    let id = s.doc.entities[0].id;
    let res = s
        .execute("block.create", &json!({"name": "MeinBlock", "ids": [id]}))
        .unwrap();
    let block_id = res["block"].as_str().unwrap().to_string();
    assert!(s.doc.block(&block_id).is_some());
    assert_eq!(s.doc.entities.len(), 1, "Original wird durch Instanz ersetzt");

    // Instanz einfügen
    s.execute("block.insert", &json!({"block": "MeinBlock", "pos": [5, 5]}))
        .unwrap();
    assert_eq!(s.doc.entities.len(), 2);

    // Zerlegen der Instanz erzeugt Grundelemente
    let inst = last_id(&s);
    let res = s.execute("modify.explode", &json!({"ids": [inst]})).unwrap();
    assert!(res["created"].as_array().unwrap().len() >= 1);
    assert!(s.doc.entity(inst).is_none());
}

#[test]
fn dimensions_and_units() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [4.5, 0], "thickness": 0.3}))
        .unwrap();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0, 0], [2, 0], [4.5, 0]], "line_pos": [0, -1.2], "axis": "x"}),
    )
    .unwrap();
    // Maßtext in Metern mit 2 Nachkommastellen
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"2,00".to_string()), "Maßtexte: {texts:?}");
    assert!(texts.contains(&"2,50".to_string()), "Maßtexte: {texts:?}");

    s.execute("doc.settings", &json!({"dim_unit": "cm"})).unwrap();
    let dl = jjcad_core::render::render_document(&jjcad_core::render::RenderCtx::new(&s.doc));
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t.starts_with("200")), "cm-Anzeige: {texts:?}");
}

#[test]
fn undo_redo_and_persistence() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [1, 0]})).unwrap();
    s.execute("draw.line", &json!({"a": [0, 1], "b": [1, 1]})).unwrap();
    assert_eq!(s.doc.entities.len(), 2);
    assert!(s.undo());
    assert_eq!(s.doc.entities.len(), 1);
    assert!(s.redo());
    assert_eq!(s.doc.entities.len(), 2);

    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roundtrip.jjcad");
    s.save(&path).unwrap();
    let loaded = jjcad_core::persist::load_document(&path).unwrap();
    assert_eq!(loaded.entities.len(), 2);
    assert_eq!(loaded.layers.len(), s.doc.layers.len());
    assert_eq!(loaded.sheets.len(), 1);
    // JSON ist lesbar (kein Binärformat)
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"format\": \"jjcad\""));
    assert!(text.contains("\"layers\""));
}

#[test]
fn dxf_roundtrip() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]})).unwrap();
    s.execute("draw.circle", &json!({"c": [1, 1], "r": 0.4})).unwrap();
    s.execute("draw.wall", &json!({"a": [0, 3], "b": [4, 3], "thickness": 0.24}))
        .unwrap();
    s.execute("draw.text", &json!({"p": [0, 2], "text": "Test", "height": 0.25}))
        .unwrap();

    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.dxf");
    jjcad_core::dxf::export_dxf(&s.doc, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("AC1009"));
    assert!(text.contains("ENTITIES"));

    let imported = jjcad_core::dxf::import_dxf(&path).unwrap();
    assert!(imported.entities.len() >= 4, "DXF-Import: {} Objekte", imported.entities.len());
    assert_eq!(imported.warnings.len(), 0, "Warnungen: {:?}", imported.warnings);
}

#[test]
fn pdf_and_png_export() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    s.execute("draw.wall", &json!({"a": [5, 0], "b": [5, 4], "thickness": 0.3}))
        .unwrap();
    s.execute("draw.door", &json!({"center": [2, 0], "width": 0.9})).unwrap();
    s.execute("draw.text", &json!({"p": [1, 2], "text": "Wohnen", "height": 0.3}))
        .unwrap();

    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let pdf_path = dir.join("out.pdf");
    jjcad_core::pdf::export_pdf(&s.doc, &pdf_path, Some(0)).unwrap();
    let pdf = std::fs::read(&pdf_path).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.7"));
    assert!(pdf.ends_with(b"%%EOF\n"));
    // Text geht als Identity-H mit eingebetteter Schrift und Zeichenzuordnung
    // in die Datei (PDF-IMPORT.md, Abschnitt 3.6) — nur so überlebt er den
    // Rückweg in den Import.
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("stream"));
    assert!(text.contains("/Identity-H"), "Identity-H fehlt");
    assert!(text.contains("/ToUnicode"), "ToUnicode fehlt");
    assert!(text.contains("/FontFile2"), "Schrift nicht eingebettet");

    let png_path = dir.join("out.png");
    let opts = jjcad_core::export::RasterOptions {
        width_px: 800,
        height_px: 600,
        ..Default::default()
    };
    jjcad_core::raster::render_png(&s.doc, &opts, &png_path).unwrap();
    let png = std::fs::read(&png_path).unwrap();
    assert_eq!(&png[0..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    assert!(png.len() > 2000);
}

#[test]
fn frictionless_wall_chain() {
    // Wände wie in der GUI: erster Punkt, zweiter Punkt, Kette
    let mut s = s();
    for (a, b) in [
        ([0.0, 0.0], [5.0, 0.0]),
        ([5.0, 0.0], [5.0, 4.0]),
        ([5.0, 4.0], [0.0, 4.0]),
    ] {
        s.execute("draw.wall", &json!({"a": a, "b": b, "thickness": 0.3}))
            .unwrap();
    }
    // Ecken müssen geschlossen sein: Verlängerungen berechnen
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    assert!(dl.prims.len() >= 9, "Prims: {}", dl.prims.len());
    let bbox = s.doc.bbox();
    // Die Ecke bei (5,0) muss durch Verlängerung geschlossen sein
    assert!(bbox.max.x >= 5100.0, "Wandende muss über die Ecke verlängert sein: {:?}", bbox.max);
    assert!(bbox.min.y <= -100.0, "{:?}", bbox.min);
    assert!(bbox.max.y >= 4100.0);
}

#[test]
fn door_kinds_render_differently() {
    let mut s = s();
    // Eine Tür braucht ihre Wand (T0038) — hier eine durchgehende Wand für beide.
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [6, 0], "thickness": 0.2}))
        .unwrap();
    let wand = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [1, 0], "angle": 0, "width": 1.0, "kind": "hinge", "host": [wand]}),
    )
    .unwrap();
    let hinge = jjcad_core::render::entity_bbox(&s.doc, s.doc.entities.last().unwrap());
    s.execute(
        "draw.door",
        &json!({"center": [4, 0], "angle": 0, "width": 1.0, "kind": "sliding", "host": [wand]}),
    )
    .unwrap();
    let sliding = jjcad_core::render::entity_bbox(&s.doc, s.doc.entities.last().unwrap());
    assert!(hinge.height() > sliding.height(), "Drehtür braucht Schwungbogen");
    let kinds: Vec<DoorKind> = s
        .doc
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Door { kind, .. } => Some(kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, vec![DoorKind::Hinge, DoorKind::Sliding]);
}

#[test]
fn template_text_fields_resolve() {
    let mut s = s();
    s.execute(
        "doc.project",
        &json!({"project": "Haus Test", "client": "Bauherr X", "plan_no": "A-1"}),
    )
    .unwrap();
    s.execute(
        "draw.text",
        &json!({"p": [0, 0], "text": "{PROJEKT} / {PLAN}", "height": 0.25}),
    )
    .unwrap();
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t == "Haus Test / A-1"), "{texts:?}");
}

#[test]
fn sheet_title_block_uses_sheet_scale() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [10, 0], "thickness": 0.3}))
        .unwrap();
    let sheet_id = s.doc.sheets[0].id;
    s.execute("sheet.update", &json!({"sheet": sheet_id, "scale": 50}))
        .unwrap();
    let bbox = s.doc.bbox();
    let center = bbox.center();
    let vid = s.doc.sheets[0].viewports().next().map(|(e, _)| e.id).expect("Ansichtsfenster");
    s.execute(
        "sheet.update_viewport",
        &json!({"sheet": sheet_id, "viewport": vid, "scale": 50, "center": [center.x / 1000.0, center.y / 1000.0]}),
    )
    .unwrap();
    let dl = jjcad_core::render::render_sheet(&s.doc, &s.doc.sheets[0]);
    let texts: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t.contains("1:50")), "Schriftfeld-Maßstab: {texts:?}");
}

#[test]
fn snapping_finds_endpoints_and_intersections() {
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [2, 0]})).unwrap();
    s.execute("draw.line", &json!({"a": [1, -1], "b": [1, 1]})).unwrap();
    // Rasterfang für diesen Test ausschalten
    s.execute("doc.settings", &json!({"snap": {"grid": false}})).unwrap();
    let hit = jjcad_core::snap::find_snap(&s.doc, v(1500.0, 1500.0), 20.0, None, &[]);
    assert!(hit.is_none(), "{hit:?}");
    let hit = jjcad_core::snap::find_snap(&s.doc, v(1990.0, 10.0), 60.0, None, &[]).unwrap();
    assert_eq!(hit.point.x, 2000.0);
    let hit = jjcad_core::snap::find_snap(&s.doc, v(1005.0, 5.0), 80.0, None, &[]).unwrap();
    assert!((hit.point.x - 1000.0).abs() < 1.0);
}

#[test]
fn wandkontur_fang_ueber_einstellungen() {
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.30}))
        .unwrap();
    let wall_id = last_id(&s);
    s.execute("doc.settings", &json!({"snap": {"grid": false}})).unwrap();

    // Die Konturecke liegt 150 mm neben der Achse: (0;150) statt (0;0).
    let ecke = v(0.0, 150.0);
    let hit = jjcad_core::snap::find_snap(&s.doc, v(4.0, 146.0), 30.0, None, &[])
        .expect("Wandkontur muss fangbar sein");
    assert!(hit.point.dist(ecke) < 1e-6, "{hit:?}");
    assert_eq!(hit.kind, jjcad_core::snap::SnapKind::Endpoint);
    assert_eq!(hit.entity, Some(wall_id));

    // Abschalten: nur die Kontur verschwindet, die Achse bleibt fangbar.
    s.execute("doc.settings", &json!({"snap": {"wall_edges": false}}))
        .unwrap();
    assert!(jjcad_core::snap::find_snap(&s.doc, v(4.0, 146.0), 30.0, None, &[]).is_none());
    let achse = jjcad_core::snap::find_snap(&s.doc, v(4.0, 4.0), 30.0, None, &[])
        .expect("Achsendendpunkt bleibt");
    assert!(achse.point.dist(v(0.0, 0.0)) < 1e-6, "{achse:?}");

    // Der Schalter überlebt Speichern und Laden.
    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wandfang.jjcad");
    s.save(&path).unwrap();
    let loaded = jjcad_core::persist::load_document(&path).unwrap();
    assert!(!loaded.settings.snap.wall_edges, "Schalter muss gespeichert sein");
}

#[test]
fn door_sits_on_front_wall_face() {
    // Tür sitzt immer auf der vorderen Wandkante (Blattseite), nicht in der
    // Wandmitte; Ausrichtung ist fuer Tueren ohne Wirkung.
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [2, 0], "width": 0.9, "host": [wall], "base": "center"}),
    )
    .unwrap();
    let door = last_id(&s);
    let pos = |s: &Session| match &s.doc.entity(door).unwrap().kind {
        EntityKind::Door { center, .. } => *center,
        _ => unreachable!(),
    };
    let p = pos(&s);
    assert!((p.y - 150.0).abs() < 1.0, "vordere Kante: y={}", p.y);
    assert!((p.x - 2000.0).abs() < 1.0, "x={}", p.x);

    // Blatt auf die andere Seite -> andere Kante, Lage laengs bleibt
    s.execute("modify.set", &json!({"ids": [door], "swing_in": false}))
        .unwrap();
    let p = pos(&s);
    assert!((p.y + 150.0).abs() < 1.0, "gegenueber: y={}", p.y);
    assert!((p.x - 2000.0).abs() < 1.0, "x nach Spiegelung={}", p.x);

    // Ausrichtung wird bei Tueren ignoriert
    s.execute("modify.set", &json!({"ids": [door], "align": "left"}))
        .unwrap();
    assert!((pos(&s).y + 150.0).abs() < 1.0, "align darf nichts aendern");
}

#[test]
fn window_align_repositions_in_wall() {
    // Fenster: Ausrichtung verschiebt innerhalb der Wanddicke (75 mm bei 300 mm)
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.window",
        &json!({"center": [2, 0], "width": 1.0, "host": [wall], "align": "center"}),
    )
    .unwrap();
    let win = last_id(&s);
    let cy = |s: &Session| match &s.doc.entity(win).unwrap().kind {
        EntityKind::Window { center, .. } => center.y,
        _ => unreachable!(),
    };
    s.execute("modify.set", &json!({"ids": [win], "align": "left"}))
        .unwrap();
    assert!((cy(&s) - 75.0).abs() < 1.0, "links: y={}", cy(&s));
    s.execute("modify.set", &json!({"ids": [win], "align": "right"}))
        .unwrap();
    assert!((cy(&s) + 75.0).abs() < 1.0, "rechts: y={}", cy(&s));
}

fn opening_pos(s: &Session, id: u64) -> Vec2 {
    match &s.doc.entity(id).unwrap().kind {
        EntityKind::Door { center, .. } | EntityKind::Window { center, .. } => *center,
        _ => unreachable!(),
    }
}

#[test]
fn opening_follows_wall() {
    // Tür hängt an der Wand: Verschieben und Verdicken der Wand nimmt sie mit.
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [2, 0], "width": 0.9, "host": [wall], "base": "center"}),
    )
    .unwrap();
    let door = last_id(&s);

    s.execute(
        "modify.move",
        &json!({"ids": [wall], "from": [0, 0], "to": [1, 0]}),
    )
    .unwrap();
    let p = opening_pos(&s, door);
    assert!((p.x - 3000.0).abs() < 1.0, "x nach Wandverschiebung={}", p.x);
    assert!((p.y - 150.0).abs() < 1.0, "y={}", p.y);

    s.execute("modify.set", &json!({"ids": [wall], "thickness": 0.6}))
        .unwrap();
    let p = opening_pos(&s, door);
    assert!((p.y - 300.0).abs() < 1.0, "y nach Dicke={}", p.y);
    assert!((p.x - 3000.0).abs() < 1.0, "x={}", p.x);
}

#[test]
fn moved_opening_stays_on_wall() {
    // Wird die Tür selbst verschoben, bleibt sie auf ihrer Wand.
    let mut s = s();
    s.execute("draw.wall", &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.3}))
        .unwrap();
    let wall = last_id(&s);
    s.execute(
        "draw.door",
        &json!({"center": [2, 0], "width": 0.9, "host": [wall], "base": "center"}),
    )
    .unwrap();
    let door = last_id(&s);
    s.execute(
        "modify.move",
        &json!({"ids": [door], "from": [2, 0], "to": [3.5, 0]}),
    )
    .unwrap();
    let p = opening_pos(&s, door);
    assert!((p.x - 3500.0).abs() < 1.0, "x={}", p.x);
    assert!((p.y - 150.0).abs() < 1.0, "y auf Kante={}", p.y);
}

#[test]
fn select_window_nutzt_geometrie_nicht_bbox() {
    // Eine Diagonale darf nicht mitgewaehlt werden, nur weil das Auswahlrechteck
    // in ihrer Bounding-Box liegt.
    let mut s = s();
    s.execute("draw.line", &json!({"a": [0, 0], "b": [5, 5]}))
        .unwrap();
    let id = last_id(&s);
    // liegt in der BBox der Linie, beruehrt sie aber nicht
    let neben = jjcad_core::snap::select_window(&s.doc, v(3000.0, 0.0), v(4400.0, 2000.0), true);
    assert!(neben.is_empty(), "Kreuzfenster ohne Beruehrung: {neben:?}");
    let fenster = jjcad_core::snap::select_window(&s.doc, v(3000.0, 0.0), v(4400.0, 2000.0), false);
    assert!(fenster.is_empty(), "Fenster ohne Beruehrung: {fenster:?}");
    // schneidet die Linie wirklich
    let treffer = jjcad_core::snap::select_window(&s.doc, v(4000.0, 3000.0), v(6000.0, 5000.0), true);
    assert_eq!(treffer, vec![id]);
    // vollstaendig enthalten
    let ganz = jjcad_core::snap::select_window(&s.doc, v(-100.0, -100.0), v(6000.0, 6000.0), false);
    assert_eq!(ganz, vec![id]);
}

#[test]
fn wandkontur_fang_ist_getrennt_schaltbar() {
    // Wand 5 m lang, 240 mm dick, mittig -> Konturecken bei 0/5000 ± 120.
    let mut s = s();
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [5, 0], "thickness": 0.24}),
    )
    .unwrap();
    s.execute("doc.settings", &json!({"snap": {"grid": false}}))
        .unwrap();
    assert!(s.doc.settings.snap.wall_edges, "Standard: Wandkanten aktiv");

    let ecken = [
        (0.0, 120.0),
        (0.0, -120.0),
        (5000.0, 120.0),
        (5000.0, -120.0),
    ];
    for (x, y) in ecken {
        let hit = jjcad_core::snap::find_snap(&s.doc, v(x, y), 20.0, None, &[])
            .unwrap_or_else(|| panic!("Konturecke ({x},{y}) muss fangen"));
        assert!(
            (hit.point.x - x).abs() < 1e-6 && (hit.point.y - y).abs() < 1e-6,
            "Konturecke ({x},{y}) -> {hit:?}"
        );
    }

    // Abschalten: ausschliesslich die Wandkontur verschwindet.
    s.execute("doc.settings", &json!({"snap": {"wall_edges": false}}))
        .unwrap();
    for (x, y) in ecken {
        assert!(
            jjcad_core::snap::find_snap(&s.doc, v(x, y), 20.0, None, &[]).is_none(),
            "Konturecke ({x},{y}) darf nicht mehr fangen"
        );
    }
    // Die Wandachse bleibt unberuehrt fangbar.
    let achse = jjcad_core::snap::find_snap(&s.doc, v(0.0, 0.0), 20.0, None, &[])
        .expect("Achsen-Endpunkt muss weiterhin fangen");
    assert_eq!(achse.point, v(0.0, 0.0));
    let mitte = jjcad_core::snap::find_snap(&s.doc, v(2500.0, 0.0), 20.0, None, &[])
        .expect("Achsen-Mittelpunkt muss weiterhin fangen");
    assert_eq!(mitte.point, v(2500.0, 0.0));

    // Wieder einschalten: Kontur ist sofort wieder da.
    s.execute("doc.settings", &json!({"snap": {"wall_edges": true}}))
        .unwrap();
    assert!(jjcad_core::snap::find_snap(&s.doc, v(0.0, 120.0), 20.0, None, &[]).is_some());
}

#[test]
fn wandkanten_option_wird_gespeichert() {
    let sp = jjcad_core::model::SnapSettings::default();
    assert!(sp.wall_edges, "Default muss aktiv sein");
    // Serde-Rundreise (Dokument-Settings landen so in der .jjcad-Datei)
    let mut aus = sp;
    aus.wall_edges = false;
    let js = serde_json::to_string(&aus).unwrap();
    let zurueck: jjcad_core::model::SnapSettings = serde_json::from_str(&js).unwrap();
    assert!(!zurueck.wall_edges);
    // Alte Dateien ohne den Schluessel: Default aktiv
    let alt: jjcad_core::model::SnapSettings = serde_json::from_str("{}").unwrap();
    assert!(alt.wall_edges);
    // Befehls-Schicht (MCP/GUI) kann die Option setzen
    let mut s = s();
    s.execute("doc.settings", &json!({"snap": {"wall_edges": false}}))
        .unwrap();
    assert!(!s.doc.settings.snap.wall_edges);
}

#[test]
fn zwei_klick_winkel_dreht_um_den_winkel_zwischen_den_richtungen() {
    use jjcad_core::transform::angle_between_directions;

    // Waagerechte Linie links vom Drehzentrum
    let mut s = s();
    s.execute("draw.line", &json!({"a": [1, 0], "b": [2, 0]}))
        .unwrap();
    let id = last_id(&s);

    // Zentrum (0|0), "von" auf +X, "bis" auf +Y → +90°
    let center = v(0.0, 0.0);
    let angle = angle_between_directions(center, v(1000.0, 0.0), v(0.0, 1000.0)).unwrap();
    assert!((angle - 90.0).abs() < 1e-9, "{angle}");
    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0, 0], "angle": angle}),
    )
    .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x - 0.0).abs() < 1e-6 && (a.y - 1000.0).abs() < 1e-6, "{a:?}");
        assert!((b.x - 0.0).abs() < 1e-6 && (b.y - 2000.0).abs() < 1e-6, "{b:?}");
    } else {
        panic!("Linie erwartet");
    }

    // Gegenrichtung: "bis" auf -Y → -90°, dreht die Linie zurück und weiter nach unten
    let angle = angle_between_directions(center, v(1000.0, 0.0), v(0.0, -1000.0)).unwrap();
    assert!((angle + 90.0).abs() < 1e-9, "{angle}");
    s.execute(
        "modify.rotate",
        &json!({"ids": [id], "center": [0, 0], "angle": angle}),
    )
    .unwrap();
    if let EntityKind::Line { a, b } = &s.doc.entity(id).unwrap().kind {
        assert!((a.x - 1000.0).abs() < 1e-6 && (a.y - 0.0).abs() < 1e-6, "{a:?}");
        assert!((b.x - 2000.0).abs() < 1e-6 && (b.y - 0.0).abs() < 1e-6, "{b:?}");
    }
}

#[test]
fn block_laesst_sich_mit_auswahlwerkzeug_anklicken() {
    // Ein Block (Rechteck 2 x 1 m um den Ursprung) wird als Block angelegt und
    // an einer anderen Stelle als Instanz eingefuegt. Anklicken und Fenster-
    // auswahl muessen die Instanz treffen — also an der *eingefuegten* Stelle,
    // nicht an der Blockdefinition im Ursprung.
    let mut s = s();
    s.execute("draw.rect", &json!({"a": [0, 0], "b": [3, 1]}))
        .unwrap();
    let id = last_id(&s);
    s.execute(
        "block.create",
        &json!({"name": "Auswahlblock", "ids": [id], "base": [0, 0]}),
    )
    .unwrap();
    s.execute(
        "block.insert",
        &json!({"block": "Auswahlblock", "pos": [10, 5]}),
    )
    .unwrap();
    let inst = last_id(&s);
    assert!(matches!(s.doc.entity(inst).unwrap().kind, EntityKind::BlockRef { .. }));

    // Klick auf die linke Kante der Instanz: (10, 5) bis (10, 6) m
    let hit = jjcad_core::snap::hit_test(&s.doc, v(10000.0, 5500.0), 50.0, None);
    assert_eq!(hit, Some(inst), "Klick auf Blockkante muss die Instanz treffen");

    // Klick auf die obere Kante
    let hit = jjcad_core::snap::hit_test(&s.doc, v(11000.0, 6000.0), 50.0, None);
    assert_eq!(hit, Some(inst), "Klick auf obere Blockkante muss treffen");

    // Kreuzfenster ueber die Instanz (beruehrt die linke Kante)
    let ids = jjcad_core::snap::select_window(&s.doc, v(9000.0, 5000.0), v(10500.0, 6000.0), true);
    assert!(ids.contains(&inst), "Kreuzfenster muss die Instanz waehlen: {ids:?}");
    // Fensterauswahl: Instanz vollstaendig enthalten
    let ids = jjcad_core::snap::select_window(&s.doc, v(9500.0, 4500.0), v(13500.0, 6500.0), false);
    assert!(ids.contains(&inst), "Fenster muss die Instanz waehlen: {ids:?}");

    // Alle gezeichneten Grundlinien der Instanz tragen deren Id — nur dann
    // faerbt die Auswahl den Block blau ein.
    let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
    let dl = jjcad_core::render::render_document(&ctx);
    let eigene = dl
        .prims
        .iter()
        .filter(|p| p.entity == inst)
        .count();
    assert!(eigene >= 4, "Blockinstanz muss ihre Linien zeichnen, gefunden: {eigene}");
    assert!(
        dl.prims.iter().all(|p| p.entity != 0),
        "keine Linie darf ohne Objekt-Id gezeichnet werden"
    );

    // Nachgelagerte Aktionen: Verschieben und Loeschen wirken auf die Instanz.
    s.execute("modify.move", &json!({"ids": [inst], "from": [10, 5], "to": [20, 5]}))
        .unwrap();
    let hit = jjcad_core::snap::hit_test(&s.doc, v(20000.0, 5500.0), 50.0, None);
    assert_eq!(hit, Some(inst), "nach dem Verschieben muss die Instanz dort liegen");
    s.execute("modify.erase", &json!({"ids": [inst]})).unwrap();
    assert!(s.doc.entity(inst).is_none());
}

#[test]
fn reihenfolge_ist_der_listenplatz() {
    // Die Objektliste IST die Zeichenreihenfolge: Umsortieren verschiebt das
    // Element, ohne dass zwei Objekte denselben Platz belegen koennen.
    let mut s = s();
    for (i, x) in [0.0, 1.0, 2.0, 3.0].iter().enumerate() {
        s.execute(
            "draw.line",
            &json!({"a": [*x, i as f64], "b": [*x + 0.5, i as f64]}),
        )
        .unwrap();
    }
    let ids: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 4);

    // drittes Element ganz nach hinten (Platz 1)
    s.execute("modify.order", &json!({"ids": [ids[2]], "to": 1}))
        .unwrap();
    let jetzt: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    assert_eq!(jetzt[0], ids[2]);
    assert_eq!(jetzt.len(), 4);
    // keine doppelten Plaetze: jede id genau einmal
    let mut sortiert = jetzt.clone();
    sortiert.sort_unstable();
    sortiert.dedup();
    assert_eq!(sortiert.len(), 4);

    // erstes Element ganz nach vorn
    s.execute("modify.order", &json!({"ids": [ids[0]], "where": "front"}))
        .unwrap();
    let jetzt: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    assert_eq!(*jetzt.last().unwrap(), ids[0]);
    assert_eq!(jetzt.len(), 4);

    // Ziel groesser als die Liste -> letzter Platz, kein Fehler
    s.execute("modify.order", &json!({"ids": [ids[1]], "to": 999}))
        .unwrap();
    assert_eq!(s.doc.entities.last().unwrap().id, ids[1]);
    assert_eq!(s.doc.entities.len(), 4);
}

#[test]
fn reihenfolge_vor_und_hinter_element() {
    let mut s = s();
    for i in 0..4 {
        s.execute("draw.line", &json!({"a": [i as f64, 0], "b": [i as f64 + 0.5, 0]}))
            .unwrap();
    }
    let ids: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();

    // Element 0 direkt hinter Element 2 (also unterhalb davon im Stapel)
    s.execute(
        "modify.order",
        &json!({"ids": [ids[0]], "ref": ids[2], "where": "below"}),
    )
    .unwrap();
    let jetzt: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    eprintln!("ids={:?} jetzt={:?}", ids, jetzt);
    let p0 = jetzt.iter().position(|x| *x == ids[0]).unwrap();
    let p2 = jetzt.iter().position(|x| *x == ids[2]).unwrap();
    assert_eq!(p0 + 1, p2, "muss direkt hinter dem Referenzobjekt liegen");

    // Element 1 direkt vor Element 3
    s.execute(
        "modify.order",
        &json!({"ids": [ids[1]], "ref": ids[3], "where": "above"}),
    )
    .unwrap();
    let jetzt: Vec<u64> = s.doc.entities.iter().map(|e| e.id).collect();
    let p1 = jetzt.iter().position(|x| *x == ids[1]).unwrap();
    let p3 = jetzt.iter().position(|x| *x == ids[3]).unwrap();
    assert_eq!(p1, p3 + 1, "muss direkt vor dem Referenzobjekt liegen");
    assert_eq!(jetzt.len(), 4);
}