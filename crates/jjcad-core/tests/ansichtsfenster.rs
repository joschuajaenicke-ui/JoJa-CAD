//! Ansichtsfenster als normales Blattobjekt (T0109).
//!
//! Bis dahin lag ein Ansichtsfenster in einer eigenen Liste (`Sheet::viewports`)
//! mit eigenem Auswahlfeld, eigenem Verschieben und ohne Griffe. Es ließ sich
//! weder über ein Auswahlfenster wählen noch löschen. Hier wird festgenagelt,
//! dass es sich wie jedes andere Objekt verhält: es steht in `sheet.entities`,
//! wird angeklickt, im Fenster gewählt, verschoben und gelöscht — und alte
//! Dateien werden beim Laden umgeschrieben.

use jjcad_core::geom::v;
use jjcad_core::model::*;
use jjcad_core::persist;
use jjcad_core::session::Session;
use jjcad_core::snap;
use serde_json::json;

/// Sitzung im Papierraum des ersten Blatts, mit der Id des Ansichtsfensters.
fn blatt() -> (Session, u64, u64) {
    let mut s = Session::new();
    let sid = s.doc.sheets[0].id;
    s.set_space(Some(sid));
    let vid = s
        .doc
        .sheets[0]
        .viewports()
        .next()
        .map(|(e, _)| e.id)
        .expect("Blatt hat ein Ansichtsfenster");
    (s, sid, vid)
}

#[test]
fn ansichtsfenster_ist_ein_blattobjekt() {
    let (s, _, vid) = blatt();
    let e = s.doc.entity(vid).expect("Ansichtsfenster als Objekt");
    assert!(
        matches!(e.kind, EntityKind::Viewport(_)),
        "Ansichtsfenster muss ein Blattobjekt sein, war {:?}",
        e.kind
    );
    // Die alte Liste ist beim Anlegen leer — sie dient nur noch dem Laden.
    assert!(s.doc.sheets[0].viewports_alt.is_empty());
}

#[test]
fn ansichtsfenster_wird_angeklickt_und_im_fenster_gewaehlt() {
    let (s, _, vid) = blatt();
    let vp = s.doc.sheets[0].viewport(vid).expect("Fenster").clone();
    let mitte = vp.rect().center();

    // Klick in die Fläche wählt das Fenster (wie ein Bild).
    assert_eq!(
        snap::hit_test(&s.doc, mitte, 1.0, None),
        Some(vid),
        "Klick in die Fensterfläche muss das Ansichtsfenster wählen"
    );

    // Auswahlfenster über das ganze Rechteck wählt es mit.
    let ids = snap::select_window(
        &s.doc,
        v(vp.x - 1.0, vp.y - 1.0),
        v(vp.x + vp.w + 1.0, vp.y + vp.h + 1.0),
        false,
    );
    assert!(
        ids.contains(&vid),
        "Auswahlfenster muss das Ansichtsfenster mitwählen, war {ids:?}"
    );
}

#[test]
fn ansichtsfenster_laesst_sich_verschieben() {
    let (mut s, _, vid) = blatt();
    let vorher = s.doc.sheets[0].viewport(vid).expect("Fenster").clone();
    s.execute(
        "modify.move",
        &json!({"ids": [vid], "from": [0, 0], "to": [10, 5], "units": "mm"}),
    )
    .expect("Verschieben");
    let nachher = s.doc.sheets[0].viewport(vid).expect("Fenster").clone();
    assert!((nachher.x - vorher.x - 10.0).abs() < 1e-6, "x: {}", nachher.x);
    assert!((nachher.y - vorher.y - 5.0).abs() < 1e-6, "y: {}", nachher.y);
    // Größe und Maßstab bleiben.
    assert!((nachher.w - vorher.w).abs() < 1e-6);
    assert!((nachher.h - vorher.h).abs() < 1e-6);
    assert!((nachher.scale - vorher.scale).abs() < 1e-6);
}

#[test]
fn ansichtsfenster_laesst_sich_loeschen() {
    let (mut s, _, vid) = blatt();
    s.execute("modify.erase", &json!({"ids": [vid]}))
        .expect("Löschen");
    assert!(
        s.doc.sheets[0].viewport(vid).is_none(),
        "Ansichtsfenster muss nach dem Löschen weg sein"
    );
    assert!(s.doc.entity(vid).is_none());
}

#[test]
fn ansichtsfenster_laesst_sich_ueber_das_panel_aendern() {
    let (mut s, _, vid) = blatt();
    s.execute(
        "modify.set",
        &json!({"ids": [vid], "x": 20.0, "y": 90.0, "w": 300.0, "h": 150.0,
                "scale": 50.0, "show_frame": true, "name": "Detail"}),
    )
    .expect("Eigenschaften");
    let vp = s.doc.sheets[0].viewport(vid).expect("Fenster").clone();
    assert_eq!((vp.x, vp.y, vp.w, vp.h), (20.0, 90.0, 300.0, 150.0));
    assert_eq!(vp.scale, 50.0);
    assert!(vp.show_frame);
    assert_eq!(vp.name, "Detail");
}

/// Alte Dateien tragen die Fenster noch in `sheets[].viewports` — beim Laden
/// werden sie zu Objekten, damit Auswahl, Griffe und Löschen greifen.
#[test]
fn alte_datei_wird_beim_laden_umgeschrieben() {
    let (s, _, vid) = blatt();
    let mut wert: serde_json::Value =
        serde_json::from_str(&persist::to_json(&s.doc).expect("JSON")).expect("Wert");

    // Den Objekt-Eintrag zurück in die alte Liste schieben.
    let blatt = &mut wert["sheets"][0];
    let ents = blatt["entities"].as_array_mut().expect("entities");
    let pos = ents
        .iter()
        .position(|e| e["kind"]["type"] == "viewport")
        .expect("Ansichtsfenster im JSON");
    let vp = ents.remove(pos);
    // Die alte Liste trug die `Viewport`-Struktur selbst, nicht das Objekt.
    blatt["viewports"] = json!([vp["kind"].clone()]);

    let doc = persist::from_json(&wert.to_string()).expect("Laden");
    let e = doc.sheets[0]
        .entities
        .iter()
        .find(|e| e.id == vid)
        .expect("Ansichtsfenster nach dem Laden");
    assert!(matches!(e.kind, EntityKind::Viewport(_)));
    assert!(
        doc.sheets[0].viewports_alt.is_empty(),
        "die alte Liste muss beim Laden geleert werden"
    );
    // Und es wird nicht mehr in die alte Liste geschrieben.
    let neu: serde_json::Value =
        serde_json::from_str(&persist::to_json(&doc).expect("JSON")).expect("Wert");
    assert!(
        neu["sheets"][0].get("viewports").is_none(),
        "neue Dateien dürfen die alte Liste nicht mehr schreiben"
    );
}

// ---------------------------------------------------------------------------
// c) Angezeigte Layer (T0238)
// ---------------------------------------------------------------------------

/// Ein Meter-Dokument mit zwei Wänden auf zwei Plänen; liefert Sitzung,
/// Fensterkennung und die beiden Layer-Kennungen.
fn zwei_plaene() -> (Session, u64, u64, u64) {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"}))
        .expect("Einheit");
    let vid = s.doc.sheets[0]
        .viewports()
        .next()
        .map(|(e, _)| e.id)
        .expect("Ansichtsfenster");
    // Plan 1, Layer „Architektur“ (Standardebene der Wand).
    s.execute(
        "draw.wall",
        &json!({"a": [0, 0], "b": [3, 0], "thickness": 0.24, "layer": "Architektur"}),
    )
    .expect("Wand im Erdgeschoss");
    let plan2 = s
        .execute("plan.create", &json!({"name": "Obergeschoss"}))
        .expect("Plan")["id"]
        .as_u64()
        .expect("Plan-Kennung");
    let l2 = s
        .doc
        .layers
        .iter()
        .find(|l| l.plan == plan2 && l.name == "Architektur")
        .map(|l| l.id)
        .expect("Architektur im zweiten Plan");
    s.execute(
        "draw.wall",
        &json!({"a": [0, 2], "b": [3, 2], "thickness": 0.24, "layer": l2}),
    )
    .expect("Wand im Obergeschoss");
    let l1 = s
        .doc
        .layers
        .iter()
        .find(|l| l.plan != plan2 && l.name == "Architektur")
        .map(|l| l.id)
        .expect("Architektur im ersten Plan");
    // `modify.set` arbeitet am **Bearbeitungsraum**: erst im Papierraum des
    // Blatts steht das Ansichtsfenster in `doc.ents_mut()`.
    let sid = s.doc.sheets[0].id;
    s.set_space(Some(sid));
    (s, vid, l1, l2)
}

/// `layers: null` zeigt alle Layer; eine Liste nur die genannten, und eine
/// leere Liste gar nichts. Belegt am Renderer, nicht an der Datei.
#[test]
fn ansichtsfenster_zeigt_nur_gewaehlte_layer() {
    let (mut s, vid, l1, l2) = zwei_plaene();
    let prims = |s: &Session| {
        let vp = s.doc.sheets[0].viewport(vid).expect("Fenster").clone();
        jjcad_core::render::render_viewport(&s.doc, &vp).prims.len()
    };
    let alle = prims(&s);
    assert!(alle > 0, "die Zeichnung zeichnet nichts");

    // Nur der Plan des Erdgeschosses.
    s.execute("modify.set", &json!({"ids": [vid], "layers": [l1]}))
        .expect("Layerwahl");
    assert_eq!(
        s.doc.sheets[0].viewport(vid).expect("Fenster").layers,
        Some(vec![l1])
    );
    let nur_eg = prims(&s);
    assert!(
        0 < nur_eg && nur_eg < alle,
        "ein Layer zeigt weniger als alle, aber etwas: {nur_eg} von {alle}"
    );

    // Nur der zweite Plan — dieselbe Zahl, denn beide Pläne haben eine Wand.
    s.execute("modify.set", &json!({"ids": [vid], "layers": [l2]}))
        .expect("Layerwahl");
    assert_eq!(prims(&s), nur_eg, "die Layerwahl trifft den falschen Plan");

    // Keine Liste: gar nichts.
    s.execute("modify.set", &json!({"ids": [vid], "layers": []}))
        .expect("Layerwahl");
    assert_eq!(prims(&s), 0, "eine leere Auswahl zeigt nichts");

    // `null`: wieder alle.
    s.execute("modify.set", &json!({"ids": [vid], "layers": null}))
        .expect("Layerwahl");
    assert_eq!(s.doc.sheets[0].viewport(vid).expect("Fenster").layers, None);
    assert_eq!(prims(&s), alle, "`null` muss alle Layer zurückbringen");
}

/// Die Wahl steht in der Datei (`layers` in der `.jjcad`-Struktur) und kommt
/// beim Laden unverändert zurück.
#[test]
fn layerwahl_steht_in_der_datei() {
    let (mut s, vid, l1, _) = zwei_plaene();
    s.execute("modify.set", &json!({"ids": [vid], "layers": [l1]}))
        .expect("Layerwahl");
    let text = persist::to_json(&s.doc).expect("JSON");
    assert!(
        text.contains("\"layers\": [") || text.contains("\"layers\":["),
        "die Layerwahl fehlt in der Datei: {:?}",
        text.lines().take(3).collect::<Vec<_>>()
    );
    let doc = persist::from_json(&text).expect("Laden");
    assert_eq!(
        doc.sheets[0].viewport(vid).expect("Fenster").layers,
        Some(vec![l1]),
        "die Layerwahl überlebt das Speichern nicht"
    );
}
