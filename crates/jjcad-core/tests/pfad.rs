//! T0229: Pfad — Kette aus Linien, Bögen, Ellipsenbögen und Splines.
//!
//! Geprüft wird die Befehls-Schicht (`draw.path`, `path.connect`) — derselbe
//! Weg, den Oberfläche und MCP nehmen.

use jjcad_core::actions;
use jjcad_core::geom::v;
use jjcad_core::model::{Document, Entity, EntityKind};
use jjcad_core::pfad::{pfad_flaeche, PathSeg};
use serde_json::json;

fn doc_mm() -> Document {
    let mut doc = Document::new();
    doc.settings.input_unit = jjcad_core::units::Unit::Mm;
    doc
}

fn pfad(doc: &Document, id: u64) -> (Vec<PathSeg>, bool) {
    match &doc.entity(id).expect("Objekt").kind {
        EntityKind::Path { segments, closed } => (segments.clone(), *closed),
        other => panic!("kein Pfad: {other:?}"),
    }
}

/// Drei Punkte, in Linien: zwei aneinanderhängende Strecken.
#[test]
fn linienkette_entsteht_aus_drei_klicks() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.path",
        &json!({"pts": [[0, 0], [1000, 0], [1000, 800]],
                "schritte": ["start", "line", "line"]}),
    )
    .expect("Pfad");
    let id = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, id);
    assert_eq!(segs.len(), 2, "zwei Stücke: {segs:?}");
    assert!(!closed);
    assert!((segs[0].start().x - 0.0).abs() < 1e-9);
    assert!((segs[0].end().x - 1000.0).abs() < 1e-9);
    // Das zweite Stück beginnt am Ende des ersten — ohne eigenes `a`.
    assert!(segs[1].start().dist(v(1000.0, 0.0)) < 1e-9);
}

/// Mit `arc_point`/`arc_end` entsteht ein Bogen durch drei Punkte, danach geht
/// die Kette am Bogenende weiter.
#[test]
fn bogenstueck_haengt_am_vorigen_ende() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.path",
        &json!({"pts": [[0, 0], [500, 500], [1000, 0], [1000, -500]],
                "schritte": ["start", "arc_point", "arc_end", "line"]}),
    )
    .expect("Pfad mit Bogen");
    let id = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, _) = pfad(&doc, id);
    assert_eq!(segs.len(), 2);
    match &segs[0] {
        PathSeg::Arc { r, .. } => assert!((r - 500.0).abs() < 1e-6, "Radius 500: {r}"),
        other => panic!("kein Bogen: {other:?}"),
    }
    assert!(segs[1].start().dist(v(1000.0, 0.0)) < 1e-6);
}

/// Klick auf den Startpunkt schließt den Pfad; er umschließt dann eine Fläche.
#[test]
fn geschlossener_pfad_hat_eine_flaeche() {
    let mut doc = doc_mm();
    let antwort = actions::execute(
        &mut doc,
        "draw.path",
        &json!({"pts": [[0, 0], [2000, 0], [2000, 1000], [0, 1000], [0, 0]],
                "schritte": ["start", "line", "line", "line", "line"]}),
    )
    .expect("Pfad");
    let id = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, id);
    assert_eq!(segs.len(), 4);
    assert!(closed, "Anfang und Ende liegen aufeinander");
    let flaeche = pfad_flaeche(&segs).expect("Fläche");
    assert!((flaeche - 2_000_000.0).abs() < 1.0, "2 m²: {flaeche}");
}

/// Vier Linien, kreuz und quer ausgewählt und teils falsch herum: verbinden
/// ordnet sie zur Kette und schließt das Rechteck.
#[test]
fn verbinden_ordnet_und_schliesst_das_rechteck() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let ecken = [
        v(0.0, 0.0),
        v(2000.0, 0.0),
        v(2000.0, 1000.0),
        v(0.0, 1000.0),
    ];
    let mut ids = Vec::new();
    for i in 0..4 {
        let a = ecken[i];
        let b = ecken[(i + 1) % 4];
        // Jede zweite Linie umgedreht einfügen.
        let (a, b) = if i % 2 == 0 { (a, b) } else { (b, a) };
        let id = doc.new_id();
        doc.add_entity(Entity::new(id, layer, EntityKind::Line { a, b }));
        ids.push(id);
    }
    // Auswahl in falscher Reihenfolge an die Action geben.
    let reihenfolge = [ids[2], ids[0], ids[3], ids[1]];
    let antwort = actions::execute(
        &mut doc,
        "path.connect",
        &json!({ "ids": reihenfolge }),
    )
    .expect("verbinden");
    let neu = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, neu);
    assert_eq!(segs.len(), 4);
    assert!(closed, "das Rechteck ist geschlossen");
    for i in 0..4 {
        let ende = segs[i].end();
        let anfang = segs[(i + 1) % 4].start();
        assert!(ende.dist(anfang) < 1e-6, "Stück {i} schließt nicht an");
    }
    // Die vier Linien sind verbraucht.
    assert_eq!(doc.ents().len(), 1, "nur der Pfad bleibt");
    assert!((pfad_flaeche(&segs).unwrap() - 2_000_000.0).abs() < 1.0);
}

/// Was nicht zusammenhängt, wird abgelehnt — und die Ausgangsobjekte bleiben.
#[test]
fn verbinden_lehnt_getrennte_linien_ab() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let a = doc.new_id();
    doc.add_entity(Entity::new(a, layer, EntityKind::Line {
        a: v(0.0, 0.0),
        b: v(1000.0, 0.0),
    }));
    let b = doc.new_id();
    doc.add_entity(Entity::new(b, layer, EntityKind::Line {
        a: v(5000.0, 0.0),
        b: v(6000.0, 0.0),
    }));
    let err = actions::execute(&mut doc, "path.connect", &json!({"ids": [a, b]}))
        .expect_err("getrennte Linien");
    assert!(err.to_lowercase().contains("hängen"), "Meldung: {err}");
    assert_eq!(doc.ents().len(), 2, "nichts wurde entfernt");
}

/// Ein Pfad mit Spline ergibt nie eine geschlossene Kontur (T0229).
#[test]
fn spline_pfad_ist_nie_geschlossen() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let linie = doc.new_id();
    doc.add_entity(Entity::new(linie, layer, EntityKind::Line {
        a: v(0.0, 0.0),
        b: v(1000.0, 0.0),
    }));
    let spline = doc.new_id();
    doc.add_entity(Entity::new(
        spline,
        layer,
        EntityKind::Spline {
            verts: vec![
                jjcad_core::Vertex::new(v(1000.0, 0.0)),
                jjcad_core::Vertex::new(v(1500.0, 500.0)),
                jjcad_core::Vertex::new(v(0.0, 0.0)),
            ],
            closed: false,
        },
    ));
    let antwort = actions::execute(&mut doc, "path.connect", &json!({"ids": [linie, spline]}))
        .expect("verbinden");
    let neu = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, closed) = pfad(&doc, neu);
    assert_eq!(segs.len(), 2);
    assert!(!closed, "Spline schließt nie");
    assert!(pfad_flaeche(&segs).is_none(), "keine Fläche mit Spline");
}

/// Ein einzelnes Objekt ist kein Pfad.
#[test]
fn verbinden_braucht_zwei_objekte() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    let a = doc.new_id();
    doc.add_entity(Entity::new(a, layer, EntityKind::Line {
        a: v(0.0, 0.0),
        b: v(1000.0, 0.0),
    }));
    assert!(actions::execute(&mut doc, "path.connect", &json!({"ids": [a]})).is_err());
}

/// T0248: Hängt die Linie am **Ende** des Bogens, muss das Bogenstück
/// umgedreht werden. Dabei darf der Bogen nicht auf die andere Seite des
/// Kreises springen (früher wurden dafür die Winkel vertauscht, was den
/// Gegenbogen ergibt).
#[test]
fn verbinden_dreht_den_bogen_ohne_gegenbogen() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    // Viertelkreis um (1000; 0) von (0; 0) nach (1000; -1000); die Linie
    // endet genau am Bogenende.
    let bogen = doc.new_id();
    doc.add_entity(Entity::new(
        bogen,
        layer,
        EntityKind::Arc {
            c: v(1000.0, 0.0),
            r: 1000.0,
            a0: std::f64::consts::PI,
            a1: 3.0 * std::f64::consts::PI / 2.0,
        },
    ));
    let linie = doc.new_id();
    doc.add_entity(Entity::new(
        linie,
        layer,
        EntityKind::Line {
            a: v(2000.0, -1000.0),
            b: v(1000.0, -1000.0),
        },
    ));
    let antwort = actions::execute(&mut doc, "path.connect", &json!({"ids": [linie, bogen]}))
        .expect("verbinden");
    let neu = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, _) = pfad(&doc, neu);
    assert_eq!(segs.len(), 2, "{segs:?}");
    assert!(segs[1].start().dist(v(1000.0, -1000.0)) < 1e-6, "Anfang am Linienende");
    assert!(segs[1].end().dist(v(0.0, 0.0)) < 1e-6, "Ende am alten Bogenanfang");
    match &segs[1] {
        PathSeg::Arc { r, a0, a1, rev, .. } => {
            assert!((*r - 1000.0).abs() < 1e-9);
            assert!((*a0 - std::f64::consts::PI).abs() < 1e-9, "Ausschnitt bleibt");
            assert!((*a1 - 3.0 * std::f64::consts::PI / 2.0).abs() < 1e-9, "Ausschnitt bleibt");
            assert!(*rev, "umgedrehte Laufrichtung");
        }
        other => panic!("kein Bogen: {other:?}"),
    }
    // Alle Punkte liegen auf demselben Viertelkreis wie vorher (unten rechts),
    // nicht auf dem Gegenbogen (oben rechts).
    for p in segs[1].flatten(0.5) {
        assert!((p.dist(v(1000.0, 0.0)) - 1000.0).abs() < 5.0, "nicht auf dem Kreis: {p:?}");
        assert!(p.y <= 0.5, "Gegenbogen statt Viertelkreis: {p:?}");
        assert!(p.x >= -0.5 && p.x <= 2000.5, "seitlich des Viertels: {p:?}");
    }
    let len = segs[0].len() + segs[1].len();
    assert!((len - (1000.0 + std::f64::consts::FRAC_PI_2 * 1000.0)).abs() < 1.0,
        "Länge {len}");
    // Anfang und Ende der Kette sind in Laufrichtung verbunden.
    assert!(segs[0].end().dist(segs[1].start()) < 1e-6);
}

/// Dasselbe für den Ellipsenbogen.
#[test]
fn verbinden_dreht_den_ellipsenbogen_ohne_gegenbogen() {
    let mut doc = doc_mm();
    let layer = doc.current_layer;
    // Viertel einer Ellipse um (1000; 0), rx = 1000, ry = 500, von (0; 0)
    // nach (1000; 500) — Parameter 180° bis 90° abwärts heißt: Ausschnitt
    // läuft CCW von 180° nach 270°? Nein: hier von 90° nach 180°.
    let bogen = doc.new_id();
    doc.add_entity(Entity::new(
        bogen,
        layer,
        EntityKind::EllipseArc {
            c: v(1000.0, 0.0),
            rx: 1000.0,
            ry: 500.0,
            rot: 0.0,
            a0: 0.0,
            a1: std::f64::consts::FRAC_PI_2,
        },
    ));
    let ende = v(1000.0, 500.0);
    let start = v(2000.0, 0.0);
    let linie = doc.new_id();
    doc.add_entity(Entity::new(
        linie,
        layer,
        EntityKind::Line {
            a: v(1000.0, 1500.0),
            b: ende,
        },
    ));
    let antwort = actions::execute(&mut doc, "path.connect", &json!({"ids": [linie, bogen]}))
        .expect("verbinden");
    let neu = antwort["created"][0].as_u64().expect("Kennung");
    let (segs, _) = pfad(&doc, neu);
    assert_eq!(segs.len(), 2);
    assert!(segs[1].start().dist(ende) < 1e-6, "Anfang am Linienende");
    assert!(segs[1].end().dist(start) < 1e-6, "Ende am alten Bogenanfang");
    match &segs[1] {
        PathSeg::EllipseArc { rev, .. } => assert!(*rev, "umgedrehte Laufrichtung"),
        other => panic!("kein Ellipsenbogen: {other:?}"),
    }
    // Alle Punkte liegen auf demselben Viertel der Ellipse (rechts oben),
    // nicht auf dem Gegenbogen.
    for p in segs[1].flatten(0.5) {
        let dx = (p.x - 1000.0) / 1000.0;
        let dy = p.y / 500.0;
        assert!((dx * dx + dy * dy - 1.0).abs() < 0.02, "nicht auf der Ellipse: {p:?}");
        assert!(p.y >= -0.5 && dx >= -0.02, "Gegenbogen statt Viertel: {p:?}");
    }
}

/// Alte Dateien kennen kein `rev` — der Bogen bleibt dann in seiner
/// gespeicherten Laufrichtung (`a0`→`a1`).
#[test]
fn pfadstueck_liest_alte_dateien_ohne_rev() {
    let alt: PathSeg = serde_json::from_str(
        r#"{"art":"arc","c":{"x":0.0,"y":0.0},"r":100.0,"a0":0.0,"a1":1.5}"#,
    )
    .expect("alter Bogen");
    match alt {
        PathSeg::Arc { rev, .. } => assert!(!rev, "Vorgabe: nicht umgedreht"),
        other => panic!("kein Bogen: {other:?}"),
    }
    let umgedreht = PathSeg::Arc {
        c: v(0.0, 0.0),
        r: 100.0,
        a0: 0.0,
        a1: 1.5,
        rev: true,
    };
    let text = serde_json::to_string(&umgedreht).expect("schreiben");
    assert!(text.contains("\"rev\":true"), "{text}");
    let zurueck: PathSeg = serde_json::from_str(&text).expect("lesen");
    assert_eq!(zurueck, umgedreht);
    let gerade = PathSeg::Arc {
        c: v(0.0, 0.0),
        r: 100.0,
        a0: 0.0,
        a1: 1.5,
        rev: false,
    };
    assert!(!serde_json::to_string(&gerade).expect("schreiben").contains("rev"),
        "die Vorgabe wird nicht mitgeschrieben");
}

/// Spiegeln eines Pfades kehrt die Laufrichtung um, der Bogen bleibt aber auf
/// seiner Seite (T0248): Anfang und Ende sind gespiegelt, der Ausschnitt ist
/// derselbe gespiegelte.
#[test]
fn spiegeln_kehrt_die_laufrichtung_des_bogens() {
    use jjcad_core::geom::Affine;
    use jjcad_core::transform::transform_kind;
    let bogen = PathSeg::Arc {
        c: v(1000.0, 0.0),
        r: 1000.0,
        a0: std::f64::consts::PI,
        a1: 3.0 * std::f64::consts::PI / 2.0,
        rev: true,
    };
    let vorher_start = bogen.start();
    let vorher_ende = bogen.end();
    // Spiegeln an der y-Achse (x → −x).
    let m = Affine::mirror(v(0.0, 0.0), v(0.0, 1.0));
    let gespiegelt = transform_kind(
        &EntityKind::Path {
            segments: vec![bogen.clone()],
            closed: false,
        },
        &m,
    );
    let EntityKind::Path { segments, .. } = gespiegelt else {
        panic!("kein Pfad");
    };
    let s = &segments[0];
    // Der Weg kehrt sich beim Spiegeln um: aus „von a1 nach a0" wird „von a0
    // nach a1" (der gespiegelte Anfang ist der alte Anfang).
    match s {
        PathSeg::Arc { rev, .. } => assert!(!*rev, "gespiegelt läuft der Bogen andersherum"),
        other => panic!("kein Bogen: {other:?}"),
    }
    assert!(s.start().dist(v(-vorher_start.x, vorher_start.y)) < 1e-6, "Anfang gespiegelt");
    assert!(s.end().dist(v(-vorher_ende.x, vorher_ende.y)) < 1e-6, "Ende gespiegelt");
    assert!((s.len() - bogen.len()).abs() < 1e-6, "Länge bleibt");
}
