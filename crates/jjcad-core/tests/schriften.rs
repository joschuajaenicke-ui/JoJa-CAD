//! Schriftarten und „Text in Linien zerlegen" (T0034).
//!
//! Geprüft wird, was der Nutzer sieht: dieselbe Zeichenfolge ist in den
//! Familien unterschiedlich breit (die Schrift wirkt also wirklich), das
//! Zerlegen eines Textes ergibt Linien statt eines Textes, eine runde
//! Zierschrift wird abgelehnt, und die Schriftart überlebt den PDF-Rundlauf.

use jjcad_core::model::{EntityKind, TextFont};
use jjcad_core::{pdf, Session};
use serde_json::json;

fn ziel(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("jjcad-schriften");
    std::fs::create_dir_all(&dir).ok();
    dir.join(name)
}

/// Text anlegen und dessen Id liefern.
fn text_anlegen(s: &mut Session, font: TextFont) -> jjcad_core::model::Id {
    s.execute(
        "draw.text",
        &json!({
            "p": [100.0, 100.0],
            "text": "Wohnen 12,4",
            "height": 5.0,
            "font": font.key(),
        }),
    )
    .expect("Text anlegen");
    s.doc
        .ents()
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Text { .. }))
        .expect("Text fehlt")
        .id
}

#[test]
fn familien_sind_unterscheidbar() {
    // Alle Familien dieses Rechners liegen vor (Liberation/Quicksand).
    for f in TextFont::ALL {
        assert!(
            jjcad_core::fonts::verfuegbar(f),
            "Schrift {} fehlt auf diesem Rechner",
            f.label()
        );
    }
    use jjcad_core::fonts::{advance_em_str, FontStyle};
    let breite = |f: TextFont, s: &str| advance_em_str(f, FontStyle::Regular, s);
    // Jede Familie setzt messbar anders als jede andere.
    let proben = "Wohnen 12,4";
    let masse: Vec<(TextFont, f64)> = TextFont::ALL
        .iter()
        .map(|f| (*f, breite(*f, proben)))
        .collect();
    for (i, (fa, wa)) in masse.iter().enumerate() {
        for (fb, wb) in masse.iter().skip(i + 1) {
            assert!(
                (wa - wb).abs() > wa.max(*wb) * 0.005,
                "{} und {} setzen gleich ({wa:.3} / {wb:.3})",
                fa.label(),
                fb.label()
            );
        }
    }
    // Nichtproportional: jedes Zeichen gleich breit — das ist die Eigenschaft,
    // an der man die Familie erkennt (schmaler als Sans kann sie sein).
    let iii = breite(TextFont::Mono, "iii");
    let www = breite(TextFont::Mono, "WWW");
    assert!(
        (iii - www).abs() < 1e-6,
        "Mono muss gleich breit setzen: {iii} / {www}"
    );
    let iii_sans = breite(TextFont::Sans, "iii");
    let www_sans = breite(TextFont::Sans, "WWW");
    assert!(
        (www_sans - iii_sans).abs() > 1e-3,
        "Sans setzt ungleich breit"
    );
}

#[test]
fn schrift_wirkt_auf_den_satz() {
    // Die Breite des gesetzten Textes folgt der Schriftart — nicht nur dem
    // Namen nach. Gemessen wird der Satz selbst (`TextPrim::width`).
    let mut breiten = Vec::new();
    for f in [TextFont::Sans, TextFont::Mono, TextFont::Narrow] {
        let mut s = Session::new();
        let id = text_anlegen(&mut s, f);
        let e = s.doc.entity(id).expect("Text").clone();
        let ctx = jjcad_core::render::RenderCtx::new(&s.doc);
        let mut dl = jjcad_core::render::DisplayList::default();
        jjcad_core::render::render_entity(
            &ctx,
            &e,
            jjcad_core::geom::Affine::IDENTITY,
            &mut dl,
            0,
        );
        let w = dl
            .prims
            .iter()
            .find_map(|p| match &p.shape {
                jjcad_core::render::Shape::Text(t) => Some(t.width()),
                _ => None,
            })
            .expect("Text im Anzeigebild");
        breiten.push(w);
    }
    assert!(
        breiten[1] > breiten[0] * 1.03,
        "Nichtproportional muss anders setzen: {breiten:?}"
    );
    assert!(
        breiten[2] < breiten[0] * 0.99,
        "Schmal muss schmaler setzen: {breiten:?}"
    );
}

#[test]
fn zerlegen_ergibt_linien() {
    let mut s = Session::new();
    let id = text_anlegen(&mut s, TextFont::Sans);
    let vorher = s.doc.ents().len();
    s.execute("modify.explode", &json!({"ids": [id]}))
        .expect("Zerlegen");
    let doc = s.doc;
    assert!(
        doc.entity(id).is_none(),
        "der Text muss nach dem Zerlegen weg sein"
    );
    let linien: Vec<_> = doc
        .ents()
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Polyline { .. }))
        .collect();
    assert!(
        linien.len() >= 8,
        "zu wenige Linien für 11 Zeichen: {}",
        linien.len()
    );
    assert_eq!(
        doc.ents().len(),
        vorher - 1 + linien.len(),
        "es dürfen nur Linien dazukommen"
    );
    // Die Umrisse liegen im Bereich des Textes (nicht am Nullpunkt).
    let b = doc
        .ents()
        .iter()
        .map(|e| jjcad_core::render::entity_bbox(&doc, e))
        .fold(None::<(f64, f64, f64, f64)>, |acc, b| match acc {
            None => Some((b.min.x, b.min.y, b.max.x, b.max.y)),
            Some((x0, y0, x1, y1)) => Some((
                x0.min(b.min.x),
                y0.min(b.min.y),
                x1.max(b.max.x),
                y1.max(b.max.y),
            )),
        })
        .expect("Umrisse");
    assert!(
        b.0 >= 99.0 && b.3 > 110.0,
        "Umrisse liegen nicht am Text: {b:?}"
    );
    // Kein Text mehr im Dokument.
    assert!(!doc
        .ents()
        .iter()
        .any(|e| matches!(e.kind, EntityKind::Text { .. })));
}

#[test]
fn zierschrift_wird_nicht_zerlegt() {
    let mut s = Session::new();
    let id = text_anlegen(&mut s, TextFont::Rund);
    let fehler = s
        .execute("modify.explode", &json!({"ids": [id]}))
        .expect_err("runde Zierschrift darf nicht zerlegt werden");
    assert!(
        fehler.contains("nicht in Linien zerlegen"),
        "Meldung sagt nicht, warum: {fehler}"
    );
    // Der Text steht unverändert da — kein halb zerlegtes Ergebnis.
    assert!(
        matches!(
            s.doc.entity(id).map(|e| &e.kind),
            Some(EntityKind::Text { .. })
        ),
        "der Text muss erhalten bleiben"
    );
    assert_eq!(s.doc.ents().len(), 1);
}

#[test]
fn schrift_ueberlebt_den_pdf_rundlauf() {
    for font in [TextFont::Serif, TextFont::Mono, TextFont::Rund] {
        let mut doc = jjcad_core::model::Document::default();
        let layer = doc.current_layer;
        let id = doc.new_id();
        doc.add_entity(jjcad_core::model::Entity::new(
            id,
            layer,
            EntityKind::Text {
                p: jjcad_core::geom::v(20.0, 30.0),
                text: "Serife 7".into(),
                height: 6.0,
                angle: 0.0,
                align: jjcad_core::model::TextAlignH::Left,
                valign: jjcad_core::model::TextAlignV::Bottom,
                bold: false,
                italic: false,
                underline: false,
                superscript: false,
                runs: Vec::new(),
                font,
            },
        ));
        let pfad = ziel(&format!("jjcad-schrift-{}.pdf", font.key()));
        pdf::export_model_pdf(&doc, &pfad, jjcad_core::style::PaperSize::A4).expect("Export");

        let mut s = Session::new();
        s.execute(
            "file.import.pdf",
            &json!({"path": pfad.to_string_lossy(), "page": 1, "scale": 1,
                    "as_block": false}),
        )
        .expect("Import");
        let gefunden = s
            .doc
            .ents()
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::Text { font, .. } => Some(*font),
                _ => None,
            })
            .next();
        assert_eq!(
            gefunden,
            Some(font),
            "Schriftart kam nicht zurück ({}): {gefunden:?}",
            font.label()
        );
    }
}
