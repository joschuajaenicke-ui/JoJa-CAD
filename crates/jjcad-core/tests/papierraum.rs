//! Papierraum der Druckvorschau (Ticket T0004).
//!
//! Die Druckvorschau verhält sich wie die Zeichnungsebene: Zeichnen, Ändern,
//! Reihenfolge und Rückgängig laufen über dieselbe Befehlsschicht, nur mit dem
//! Blatt als Bearbeitungsraum. Der Plankopf besteht aus normalen Linien und
//! Textfeldern und ist deshalb genauso bearbeitbar.

use jjcad_core::model::{Document, EntityKind};
use jjcad_core::session::Session;
use serde_json::json;

fn s() -> Session {
    // Millimeter als Eingabeeinheit: die Testzahlen sind dann direkt mm.
    let mut sess = Session::new();
    sess.execute("doc.settings", &json!({"input_unit": "mm"})).unwrap();
    sess
}

fn blatt(sess: &Session) -> u64 {
    sess.doc.sheets[0].id
}

/// Bearbeitungsraum auf das erste Blatt stellen (wie der Reiter „Blatt“).
fn in_papierraum(sess: &mut Session) {
    let id = blatt(sess);
    sess.set_space(Some(id));
    assert!(sess.doc.in_paper_space(), "Papierraum ist nicht aktiv");
}

/// Gehört die Linie zum Blattrahmen (10 mm Rand, 15 mm Heftrand links)?
fn ist_rahmenlinie(kind: &EntityKind, w: f64, h: f64) -> bool {
    let (m, left) = (10.0, 15.0);
    match kind {
        EntityKind::Line { a, b } => {
            (a.y == b.y && (a.y == m || a.y == h - m))
                || (a.x == b.x && (a.x == left || a.x == w - m))
        }
        _ => false,
    }
}

#[test]
fn zeichnen_landet_im_papierraum() {
    let mut sess = s();
    in_papierraum(&mut sess);
    let vorher = sess.doc.entities.len();

    let r = sess
        .execute("draw.line", &json!({"a": [20, 20], "b": [120, 20]}))
        .unwrap();
    let id = r["created"][0].as_u64().unwrap();

    assert_eq!(sess.doc.entities.len(), vorher, "Modell darf leer bleiben");
    let sheet = &sess.doc.sheets[0];
    let e = sheet
        .entities
        .iter()
        .find(|e| e.id == id)
        .expect("Linie fehlt im Papierraum");
    match &e.kind {
        EntityKind::Line { a, b } => {
            assert!((a.x - 20.0).abs() < 1e-9 && (b.x - 120.0).abs() < 1e-9);
        }
        other => panic!("keine Linie: {other:?}"),
    }
}

#[test]
fn aendern_und_loeschen_im_papierraum() {
    let mut sess = s();
    in_papierraum(&mut sess);
    let id = sess
        .execute("draw.line", &json!({"a": [20, 20], "b": [120, 20]}))
        .unwrap()["created"][0]
        .as_u64()
        .unwrap();

    // Verschieben um 10/5 mm
    sess.execute(
        "modify.move",
        &json!({"ids": [id], "from": [0, 0], "to": [10, 5]}),
    )
    .unwrap();
    match &sess.doc.sheets[0].entities.iter().find(|e| e.id == id).unwrap().kind {
        EntityKind::Line { a, b } => {
            assert!((a.x - 30.0).abs() < 1e-9, "{:?}", a);
            assert!((b.y - 25.0).abs() < 1e-9, "{:?}", b);
        }
        other => panic!("keine Linie: {other:?}"),
    }
    assert!(sess.doc.entities.is_empty(), "Modell unberührt");

    // Löschen
    sess.execute("modify.erase", &json!({"ids": [id]})).unwrap();
    assert!(sess.doc.sheets[0].entities.iter().all(|e| e.id != id));
    assert_eq!(sess.doc.entities.len(), 0);
}

#[test]
fn reihenfolge_im_papierraum() {
    let mut sess = s();
    in_papierraum(&mut sess);
    let a = sess.execute("draw.line", &json!({"a": [0, 0], "b": [10, 0]})).unwrap()["created"][0]
        .as_u64()
        .unwrap();
    let b = sess.execute("draw.line", &json!({"a": [0, 5], "b": [10, 5]})).unwrap()["created"][0]
        .as_u64()
        .unwrap();
    sess.execute("modify.order", &json!({"ids": [a], "where": "front"}))
        .unwrap();
    // „nach vorn“ heißt: ans Ende der Liste (dort wird zuletzt gezeichnet).
    let liste: Vec<u64> = sess.doc.sheets[0].entities.iter().map(|e| e.id).collect();
    assert_eq!(liste.last().copied(), Some(a), "{liste:?}");
    // `b` liegt vor `a`: seit T0048 ordnet sich eine neue Linie nach ihrem
    // Zeichenrang ein (vor die Beschriftung des Schriftfelds) und hängt nicht
    // mehr am Listenende. Die Reihenfolge-Action trifft trotzdem die Blattliste.
    let pos_b = liste.iter().position(|x| *x == b).unwrap();
    assert!(pos_b + 1 < liste.len(), "{liste:?}");
}

#[test]
fn rueckgaengig_nimmt_papierraum_zurueck() {
    let mut sess = s();
    in_papierraum(&mut sess);
    let vorher = sess.doc.sheets[0].entities.len();
    sess.execute("draw.line", &json!({"a": [0, 0], "b": [50, 0]})).unwrap();
    assert_eq!(sess.doc.sheets[0].entities.len(), vorher + 1);

    assert!(sess.undo());
    assert_eq!(
        sess.doc.sheets[0].entities.len(),
        vorher,
        "Rückgängig muss den Papierraum treffen"
    );
}

#[test]
fn plankopf_besteht_aus_linien_und_textfeldern() {
    let doc = Document::new();
    let sheet = &doc.sheets[0];
    assert!(!sheet.entities.is_empty(), "Plankopf fehlt");
    let linien = sheet
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .count();
    let texte = sheet
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Text { .. }))
        .count();
    assert!(linien >= 10, "Rahmenlinien fehlen: {linien}");
    assert!(texte >= 8, "Textfelder fehlen: {texte}");
    // Kein anderes Element im Plankopf: er besteht nur aus Linien und Texten.
    assert_eq!(linien + texte, sheet.entities.len());

    // Eigene, eindeutige IDs — sonst kollidieren Auswahl und Bearbeitung.
    let mut ids: Vec<u64> = sheet.entities.iter().map(|e| e.id).collect();
    ids.sort_unstable();
    let vorher = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), vorher, "doppelte IDs im Plankopf");
    let layer_ids: Vec<u64> = doc.layers.iter().map(|l| l.id).collect();
    assert!(
        ids.iter().all(|id| !layer_ids.contains(id)),
        "Plankopf-ID kollidiert mit einem Layer"
    );
    // Die Textfelder tragen die Platzhalter für die Projektdaten.
    assert!(sheet.entities.iter().any(|e| matches!(
        &e.kind,
        EntityKind::Text { text, .. } if text == "{PROJEKT}"
    )));
}

#[test]
fn plankopf_text_ist_bearbeitbar() {
    let mut sess = s();
    in_papierraum(&mut sess);
    // Ein Textfeld des Plankopfes auswählen und überschreiben.
    let (id, alt) = sess.doc.sheets[0]
        .entities
        .iter()
        .find_map(|e| match &e.kind {
            EntityKind::Text { text, .. } if text == "{PLANTITEL}" => Some((e.id, text.clone())),
            _ => None,
        })
        .expect("Feld {PLANTITEL} fehlt");
    sess.execute("modify.set", &json!({"ids": [id], "text": "Grundriss EG"}))
        .unwrap();
    let neu = sess.doc.sheets[0]
        .entities
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .kind
        .clone();
    match neu {
        EntityKind::Text { text, .. } => assert_eq!(text, "Grundriss EG"),
        other => panic!("kein Text: {other:?}"),
    }
    assert_ne!(alt, "Grundriss EG");
    // Und das Blatt zeigt den neuen Text auch im Druckbild.
    let dl = jjcad_core::render::render_sheet(&sess.doc, &sess.doc.sheets[0]);
    let texte: Vec<String> = dl
        .prims
        .iter()
        .filter_map(|p| match &p.shape {
            jjcad_core::render::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        texte.iter().any(|t| t == "Grundriss EG"),
        "Text fehlt im Blatt: {texte:?}"
    );
}

#[test]
fn speichern_und_laden_haelt_den_papierraum() {
    let mut sess = s();
    in_papierraum(&mut sess);
    sess.execute("draw.line", &json!({"a": [30, 40], "b": [90, 40]})).unwrap();
    let anzahl = sess.doc.sheets[0].entities.len();

    let pfad = std::env::temp_dir().join(format!(
        "jjcad-papierraum-{}.jjcad",
        std::process::id()
    ));
    sess.save(&pfad).unwrap();
    let geladen = jjcad_core::persist::load_document(&pfad).unwrap();
    let _ = std::fs::remove_file(&pfad);

    assert_eq!(geladen.sheets[0].entities.len(), anzahl);
    assert!(geladen.entities.is_empty(), "Modell bleibt leer");
    assert_eq!(
        geladen.space, None,
        "der Bearbeitungsraum ist Bedienzustand, kein Dateiinhalt"
    );
}

#[test]
fn ansichtsfenster_zeigt_weiter_die_zeichnung() {
    let mut sess = s();
    sess.execute("draw.line", &json!({"a": [0, 0], "b": [4000, 0]}))
        .unwrap();
    let modell_id = sess.doc.entities[0].id;
    in_papierraum(&mut sess);

    let vp = sess.doc.sheets[0].viewports[0].clone();
    let dl = jjcad_core::render::render_viewport(&sess.doc, &vp);
    assert!(
        dl.prims.iter().any(|p| p.entity == modell_id),
        "Ansichtsfenster zeigt nicht mehr die Zeichnung"
    );
    // Und der gerenderte Papierraum enthält keine Modellobjekte.
    let blatt = jjcad_core::render::render_sheet(&sess.doc, &sess.doc.sheets[0]);
    assert!(blatt.prims.iter().all(|p| p.entity != modell_id));
}

#[test]
fn raumwechsel_leert_die_auswahl() {
    let mut sess = s();
    sess.execute("draw.line", &json!({"a": [0, 0], "b": [10, 0]})).unwrap();
    let id = sess.doc.entities[0].id;
    sess.select(vec![id]);
    in_papierraum(&mut sess);
    assert!(sess.selection.is_empty(), "Modellauswahl gilt nicht im Papierraum");
    // Zurück ins Modell: Auswahl bleibt leer, Modell unverändert.
    sess.set_space(None);
    assert!(sess.selection.is_empty());
    assert_eq!(sess.doc.entities.len(), 1);
}

#[test]
fn auswahl_und_trefferpruefung_arbeiten_im_papierraum() {
    let mut sess = s();
    // Eine Linie im Modell bleibt bei der Papierraum-Auswahl außen vor.
    sess.execute("draw.line", &json!({"a": [0, 0], "b": [5000, 0]}))
        .unwrap();
    let modell_id = sess.doc.entities[0].id;
    in_papierraum(&mut sess);
    let id = sess.execute("draw.line", &json!({"a": [50, 50], "b": [150, 50]})).unwrap()
        ["created"][0]
        .as_u64()
        .unwrap();

    // Treffer auf der Linie (Papierkoordinaten in mm)
    let hit = jjcad_core::snap::hit_test(&sess.doc, jjcad_core::geom::v(100.0, 50.0), 2.0, None);
    assert_eq!(hit, Some(id));
    // Fensterauswahl über den Papierraum
    let ids = jjcad_core::snap::select_window(
        &sess.doc,
        jjcad_core::geom::v(0.0, 0.0),
        jjcad_core::geom::v(200.0, 100.0),
        false,
    );
    assert!(ids.contains(&id), "{ids:?}");
    // „Alles auswählen“ meint den aktiven Raum
    let alle = sess.execute("select.all", &json!({})).unwrap();
    let liste: Vec<u64> = alle["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap())
        .collect();
    assert!(liste.contains(&id));
    assert!(
        !liste.contains(&modell_id),
        "Modellobjekt gehört nicht zur Papierraum-Auswahl"
    );
    assert_eq!(
        liste.len(),
        sess.doc.sheets[0].entities.len(),
        "„Alles auswählen“ meint genau den aktiven Raum"
    );
}

#[test]
fn objektzahl_zaehlt_den_aktiven_raum() {
    let mut sess = s();
    sess.execute("draw.line", &json!({"a": [0, 0], "b": [10, 0]}))
        .unwrap();
    let modell = sess.doc.summary_short();
    assert!(modell.starts_with('1'), "{modell}");

    in_papierraum(&mut sess);
    let blatt = sess.doc.summary_short();
    let erwartet = format!("{} Objekte", sess.doc.sheets[0].entities.len());
    assert_eq!(blatt, erwartet, "Statuszeile muss den Papierraum zählen");
    assert_ne!(blatt, modell);

    sess.set_space(None);
    assert_eq!(sess.doc.summary_short(), modell, "zurück im Modell");
}

// ---------------------------------------------------------------------------
// Blattrahmen als Objekt und Blattecken als Fangpunkte (Nacharbeit 26.10.)
// ---------------------------------------------------------------------------

/// Der Rahmen ist kein Renderer-Sonderfall mehr, sondern vier Linien.
#[test]
fn blattrahmen_ist_ein_auswaehlbares_objekt() {
    let mut sess = s();
    let (w, h) = sess.doc.sheets[0].size();
    let (m, left) = (10.0, 15.0);

    // Die vier Rahmenlinien liegen auf dem Rahmenrechteck.
    let rahmen: Vec<_> = sess.doc.sheets[0]
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Line { a, b } => Some((e.id, *a, *b)),
            _ => None,
        })
        .filter(|(_, a, b)| {
            // achsparallel und auf dem Rahmen
            (a.y == b.y && (a.y == m || a.y == h - m))
                || (a.x == b.x && (a.x == left || a.x == w - m))
        })
        .collect();
    assert!(rahmen.len() >= 4, "Rahmenlinien fehlen: {}", rahmen.len());

    // Anklicken trifft eine Rahmenlinie (kein entity:0-Sonderfall mehr) —
    // gesucht wird im Papierraum, denn dort steht der Rahmen.
    let id = rahmen[0].0;
    in_papierraum(&mut sess);
    let treffer = jjcad_core::snap::hit_test(&sess.doc, jjcad_core::geom::v(120.0, m), 1.0, None);
    assert!(treffer.is_some(), "Rahmen ist nicht anklickbar");
    assert!(
        sess.doc.sheets[0].entities.iter().any(|e| e.id == treffer.unwrap()),
        "Treffer gehört nicht zum Blatt"
    );

    // Und er lässt sich wie jedes Objekt löschen.
    sess.execute("modify.erase", &json!({"ids": [id]}))
        .unwrap();
    assert!(sess.doc.sheets[0].entities.iter().all(|e| e.id != id));
}

/// Alte Dateien (Rahmen als Flag) bekommen beim Laden Linien — sonst bliebe
/// der Rahmen wieder unantastbar.
#[test]
fn alter_rahmen_wird_beim_laden_zu_linien() {
    let mut doc = Document::new();
    // Zustand von vor T0004 herstellen: Rahmenflag an, keine Rahmenlinien.
    {
        let sheet = &mut doc.sheets[0];
        let (w, h) = sheet.size();
        sheet.entities.retain(|e| !ist_rahmenlinie(&e.kind, w, h));
        sheet.frame = true;
        sheet.show_title_block = true;
    }
    let json = jjcad_core::persist::to_json(&doc).unwrap();
    let mut geladen = jjcad_core::persist::from_json(&json).unwrap();
    let sid = geladen.sheets[0].id;
    // Anklicken prüft den aktiven Raum: der Rahmen liegt im Papierraum.
    geladen.set_space(Some(sid));

    let sheet = &geladen.sheets[0];
    assert!(!sheet.frame, "Flag muss zurückgesetzt sein");
    let (w, h) = sheet.size();
    for p in [
        jjcad_core::geom::v(15.0, 10.0),
        jjcad_core::geom::v(w - 10.0, h - 10.0),
    ] {
        assert!(
            jjcad_core::snap::hit_test(&geladen, p, 1.0, None).is_some(),
            "Eckpunkt {p:?} ohne Rahmenlinie"
        );
    }
    // Keine doppelten IDs nach der Migration
    let mut ids: Vec<u64> = sheet.entities.iter().map(|e| e.id).collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n, "doppelte IDs nach der Migration");
    assert!(!geladen.modified, "Laden darf die Datei nicht als geändert markieren");
}

/// Die echten Blattecken fangen im Papierraum — und nur dort.
#[test]
fn blattecken_fangen_im_papierraum() {
    let mut sess = s();
    in_papierraum(&mut sess);
    let kandidaten = jjcad_core::snap::snap_candidates(
        &sess.doc,
        jjcad_core::geom::v(1.0, 1.0),
        6.0,
        None,
        &[],
        &[],
    );
    let ecke = kandidaten
        .iter()
        .find(|k| k.kind == jjcad_core::snap::SnapKind::SheetCorner)
        .expect("Blattecke nicht gefangen");
    assert!(ecke.point.dist(jjcad_core::geom::v(0.0, 0.0)) < 1e-9, "{:?}", ecke.point);
    assert_eq!(
        kandidaten[0].kind,
        jjcad_core::snap::SnapKind::SheetCorner,
        "Blattecke muss Vorrang vor dem Raster haben"
    );

    // Obere rechte Blattecke
    let (w, h) = sess.doc.sheets[0].size();
    let oben = jjcad_core::snap::snap_candidates(
        &sess.doc,
        jjcad_core::geom::v(w - 1.0, h - 1.0),
        6.0,
        None,
        &[],
        &[],
    );
    assert!(
        oben.iter()
            .any(|k| k.kind == jjcad_core::snap::SnapKind::SheetCorner),
        "obere rechte Blattecke fehlt"
    );

    // Im Modell gibt es kein Blatt und damit keine Blattecke.
    sess.set_space(None);
    let im_modell = jjcad_core::snap::snap_candidates(
        &sess.doc,
        jjcad_core::geom::v(1.0, 1.0),
        6.0,
        None,
        &[],
        &[],
    );
    assert!(
        im_modell
            .iter()
            .all(|k| k.kind != jjcad_core::snap::SnapKind::SheetCorner),
        "Blattecken dürfen im Modell nicht fangen"
    );
}

/// Der Rahmen steht als Objekt im Blattbild (und damit auch im PDF): vier
/// Linien mit eigener ID statt eines Renderer-Sonderrahmens mit `entity: 0`.
#[test]
fn rahmen_ist_im_blattbild_ein_objekt() {
    let doc = Document::new();
    let sheet = &doc.sheets[0];
    let (w, h) = sheet.size();
    let dl = jjcad_core::render::render_sheet(&doc, sheet);

    let hat_linie = |a: jjcad_core::geom::Vec2, b: jjcad_core::geom::Vec2| {
        dl.prims.iter().any(|p| {
            p.entity != 0
                && matches!(&p.shape, jjcad_core::render::Shape::Line(pa, pb)
                    if (pa.dist(a) < 1e-6 && pb.dist(b) < 1e-6)
                        || (pa.dist(b) < 1e-6 && pb.dist(a) < 1e-6))
        })
    };
    let v = jjcad_core::geom::v;
    assert!(hat_linie(v(15.0, 10.0), v(w - 10.0, 10.0)), "untere Rahmenlinie");
    assert!(hat_linie(v(w - 10.0, 10.0), v(w - 10.0, h - 10.0)), "rechte Rahmenlinie");
    assert!(hat_linie(v(w - 10.0, h - 10.0), v(15.0, h - 10.0)), "obere Rahmenlinie");
    assert!(hat_linie(v(15.0, h - 10.0), v(15.0, 10.0)), "Heftrand links");

    // Alles auf dem Blatt trägt eine ID — nichts ist mehr unantastbar.
    assert!(
        dl.prims.iter().all(|p| p.entity != 0),
        "Blattbild enthält Objekte ohne ID (nicht auswählbar)"
    );
}
