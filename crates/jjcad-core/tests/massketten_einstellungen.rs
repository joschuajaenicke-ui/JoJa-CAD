//! Maßketten-Einstellungen: Maßeinheit, Textgröße, Maßstab, Bogensymbole.
//!
//! Diese vier Werte standen früher im Ribbon und stehen jetzt im
//! Eigenschaften-Panel. Das Panel ruft ausschließlich die Actions `doc.settings`
//! und `dim.style` auf — genau die Kette, die hier geprüft wird: Wirkung auf die
//! Displayliste, Undo und Persistenz.

use jjcad_core::render::{render_document, RenderCtx, Shape};
use jjcad_core::session::Session;
use jjcad_core::style::ArrowKind;
use serde_json::json;

fn s() -> Session {
    let mut s = Session::new();
    s.execute("doc.settings", &json!({"input_unit": "m"})).unwrap();
    s.execute(
        "dim.chain",
        &json!({"pts": [[0, 0], [2, 0], [4.5, 0]], "line_pos": [0, -1.2], "axis": "x"}),
    )
    .unwrap();
    s
}

/// Höhe des Maßtextes in der Displayliste (Welt-mm).
fn dim_text_height(s: &Session) -> f64 {
    let dl = render_document(&RenderCtx::new(&s.doc));
    dl.prims
        .iter()
        .filter_map(|p| match &p.shape {
            Shape::Text(t) if t.text.contains(',') => Some(t.height),
            _ => None,
        })
        .fold(0.0, f64::max)
}

/// Formsignatur der Bemaßung: je Symbolart eine andere Verteilung von
/// Linien/Kreisen/Punkten in der Displayliste (Schrägstrich = Linie,
/// Punkt = Kreis, Pfeil = Fläche).
fn dim_shape_signature(s: &Session) -> (usize, usize, usize, usize) {
    let ids: Vec<u64> = s
        .doc
        .entities
        .iter()
        .filter(|e| matches!(e.kind, jjcad_core::model::EntityKind::DimChain { .. }))
        .map(|e| e.id)
        .collect();
    let dl = render_document(&RenderCtx::new(&s.doc));
    let mut sig = (0usize, 0usize, 0usize, 0usize);
    for p in dl.prims.iter().filter(|p| ids.contains(&p.entity)) {
        match &p.shape {
            Shape::Line(..) => sig.0 += 1,
            Shape::Circle { .. } | Shape::Dot { .. } => sig.1 += 1,
            Shape::Fill { .. } => sig.2 += 1,
            Shape::Poly(..) => sig.3 += 1,
            _ => {}
        }
    }
    sig
}

#[test]
fn textgroesse_wirkt_sofort_auf_die_darstellung() {
    let mut s = s();
    let vorher = dim_text_height(&s);
    s.execute("dim.style", &json!({"text_height": 5.0})).unwrap();
    let nachher = dim_text_height(&s);
    assert!(
        vorher > 0.0 && nachher > vorher * 1.5,
        "Textgröße 2,5 → 5,0 mm muss den Maßtext vergrößern: {vorher} → {nachher}"
    );
}

#[test]
fn bogensymbole_aendern_die_darstellung() {
    let mut s = s();
    // Schrägstrich ist der Standard (Linien), Punkt zeichnet Kreise.
    let schraeg = dim_shape_signature(&s);
    s.execute("dim.style", &json!({"arrow": "dot"})).unwrap();
    assert_eq!(s.doc.dim_style.arrow, ArrowKind::Dot);
    let punkt = dim_shape_signature(&s);
    assert_ne!(
        schraeg, punkt,
        "Bogensymbol muss die Displayliste ändern (Schrägstrich {schraeg:?}, Punkt {punkt:?})"
    );
    assert!(
        punkt.1 > schraeg.1,
        "Punkt-Symbole müssen Kreise liefern: {schraeg:?} → {punkt:?}"
    );
    // Pfeil zeichnet eine gefüllte Spitze statt eines Kreises.
    s.execute("dim.style", &json!({"arrow": "arrow"})).unwrap();
    let pfeil = dim_shape_signature(&s);
    assert!(
        pfeil.3 > punkt.3,
        "Pfeil-Symbole müssen Flächen liefern: {punkt:?} → {pfeil:?}"
    );
}

#[test]
fn massstab_skaliert_die_papiergroessen() {
    let mut s = s();
    let h100 = dim_text_height(&s);
    s.execute("doc.settings", &json!({"annotation_scale": 50.0})).unwrap();
    let h50 = dim_text_height(&s);
    assert!(
        (h100 - 2.0 * h50).abs() < 0.01,
        "1:50 muss den Maßtext auf die halbe Papierhöhe bringen: {h100} vs {h50}"
    );
}

/// Eine Stelle, ein Wert: die vier Einstellungen dürfen nur im
/// Eigenschaften-Panel stehen — nicht mehr im Ribbon und nicht im
/// Einstellungen-Dialog. Der Test liest die Quellen der GUI, weil dort die
/// doppelten Bedienelemente saßen.
#[test]
fn massketten_einstellungen_nur_im_eigenschaften_panel() {
    let gui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../jjcad-gui/src");
    let panels = std::fs::read_to_string(gui.join("panels.rs")).unwrap();
    assert!(panels.contains("\"Maßketten\""), "Abschnitt Maßketten fehlt im Panel");
    for feld in ["Maßeinheit", "Textgröße", "Maßstab", "Bogensymbole"] {
        assert!(panels.contains(feld), "Panel-Zeile {feld} fehlt");
    }

    // Ribbon: Registerkarte Bemaßung enthält nur noch die drei Werkzeuge.
    let app = std::fs::read_to_string(gui.join("app.rs")).unwrap();
    let arm = &app[app.find("RibbonTab::Bemassung =>").expect("Bemeßungs-Tab")..];
    let ribbon = &arm[..arm.find("RibbonTab::Bloecke =>").expect("nächster Tab")];
    for verboten in ["dimunit", "arrowkind", "annotation_scale", "Bogensymbole"] {
        assert!(
            !ribbon.contains(verboten),
            "Ribbon-Tab Bemaßung enthält weiterhin {verboten}"
        );
    }

    // Einstellungen-Dialog: kein zweiter Satz derselben Werte.
    let dialogs = std::fs::read_to_string(gui.join("dialogs.rs")).unwrap();
    let start = dialogs.find("fn settings_body").expect("settings_body");
    let rest = &dialogs[start..];
    let body = &rest[..rest.find("\nfn ").unwrap_or(rest.len())];
    for verboten in ["dim_unit", "annotation_scale", "arrowstyle", "ds.text_height"] {
        assert!(
            !body.contains(verboten),
            "Einstellungen-Dialog enthält weiterhin {verboten}"
        );
    }
}

#[test]
fn einstellungen_ueberleben_undo_und_neustart() {
    let mut s = s();
    s.execute("doc.settings", &json!({"dim_unit": "cm", "annotation_scale": 50.0}))
        .unwrap();
    s.execute(
        "dim.style",
        &json!({"text_height": 4.0, "arrow": "arrow"}),
    )
    .unwrap();

    // Undo nimmt beide Schritte zurück …
    assert!(s.undo());
    assert_eq!(s.doc.dim_style.arrow, ArrowKind::Slash);
    assert!(s.undo());
    assert_eq!(s.doc.settings.dim_unit, jjcad_core::units::Unit::M);
    // … und Redo stellt sie wieder her.
    assert!(s.redo());
    assert!(s.redo());
    assert_eq!(s.doc.dim_style.arrow, ArrowKind::Arrow);

    // Persistenz: die Werte stehen in der Datei und kommen beim Laden zurück.
    let dir = std::env::temp_dir().join("jjcad-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("massketten.jjcad");
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"dim_unit\""), "dim_unit fehlt in der Datei");
    let loaded = jjcad_core::persist::load_document(&path).unwrap();
    assert_eq!(loaded.settings.dim_unit, jjcad_core::units::Unit::Cm);
    assert_eq!(loaded.settings.annotation_scale, 50.0);
    assert_eq!(loaded.dim_style.text_height, 4.0);
    assert_eq!(loaded.dim_style.arrow, ArrowKind::Arrow);
}
