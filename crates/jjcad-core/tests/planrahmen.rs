//! Planrahmen als Block (T0239).
//!
//! Der Standard-Planrahmen war eine Liste loser Linien und Textfelder. Er ist
//! jetzt **ein** Block: Abdeckfläche, Blattrahmen und Schriftfeld in einem
//! Objekt, dessen Basispunkt die untere linke Blattecke ist. Die weiße
//! Abdeckfläche liegt über dem Inhalt der Ansichtsfenster und unter den Linien
//! und Texten des Schriftfelds — ein Fenster darf damit das ganze Blatt füllen,
//! ohne den Plankopf unlesbar zu machen.
//!
//! Geprüft wird bis ins Bild: im PNG bleibt die Schriftfeldfläche Papierweiß,
//! obwohl eine dunkle Füllung die ganze Zeichnung bedeckt.

use jjcad_core::export::RasterOptions;
use jjcad_core::geom::*;
use jjcad_core::model::*;
use jjcad_core::raster::render_sheet_pixmap;
use jjcad_core::session::Session;
use jjcad_core::style::{Color, FillPattern};
use jjcad_core::templates;

/// Ein neues Dokument — das erste Blatt trägt den Standard-Planrahmen.
fn frisch() -> Session {
    Session::new()
}

/// Der Rahmen-Block des ersten Blatts.
fn rahmen_ref(sess: &Session) -> &Entity {
    sess.doc.sheets[0]
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::BlockRef { .. }))
        .expect("Blatt trägt keinen Planrahmen-Block")
}

#[test]
fn blatt_traegt_den_planrahmen_als_ein_objekt() {
    let sess = frisch();
    let sheet = &sess.doc.sheets[0];
    let refs: Vec<&Entity> = sheet
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::BlockRef { .. }))
        .collect();
    assert_eq!(refs.len(), 1, "genau ein Rahmen-Block je Blatt");
    // Keine losen Rahmenlinien und Schriftfeld-Texte mehr.
    assert!(
        sheet
            .entities
            .iter()
            .all(|e| !matches!(e.kind, EntityKind::Line { .. } | EntityKind::Text { .. })),
        "Rahmen und Schriftfeld müssen im Block stecken"
    );
    match &refs[0].kind {
        EntityKind::BlockRef { block, pos, .. } => {
            assert_eq!(block, RAHMEN_A3_QUER, "A3 quer ist der Standard");
            assert_eq!(*pos, Vec2::ZERO, "Basispunkt ist die untere linke Blattecke");
        }
        _ => unreachable!(),
    }
}

#[test]
fn die_drei_planrahmen_sind_bloecke_mit_basispunkt_blattecke() {
    for (id, w, h) in [
        (RAHMEN_A3_QUER, 420.0, 297.0),
        (RAHMEN_A3_HOCH, 297.0, 420.0),
        (RAHMEN_A4_HOCH, 210.0, 297.0),
    ] {
        let mood = Session::new();
        let def = mood
            .doc
            .block(id)
            .expect("Planrahmen fehlt in der Systembibliothek");
        assert_eq!(def.base, Vec2::ZERO, "{id}: Basispunkt muss (0,0) sein");
        assert_eq!(def.groesse, BlockGroesse::Fest, "{id}: feste Größe");
        assert_eq!(def.category, RAHMEN_KATEGORIE, "{id}: eigene Kategorie");
        // Der Basispunkt ist die **Blattecke** (0,0), der Rahmen selbst beginnt
        // beim Rand: 15 mm Heftrand links, 10 mm ringsum — er endet an der
        // gegenüberliegenden Blattecke (Blattmaß − 10 mm).
        let mut bb = BBox::EMPTY;
        for e in &def.entities {
            bb.union(jjcad_core::render::entity_bbox(&mood.doc, e));
        }
        assert!(
            (bb.min.x - 15.0).abs() < 1e-6 && (bb.min.y - 10.0).abs() < 1e-6,
            "{id}: Rahmenbeginn {bb:?}"
        );
        assert!(
            (bb.max.x - (w - 10.0)).abs() < 1e-6 && (bb.max.y - (h - 10.0)).abs() < 1e-6,
            "{id}: Rahmenende {:?} statt {} × {}",
            bb.max,
            w - 10.0,
            h - 10.0
        );
    }
}

#[test]
fn abdeckung_deckt_genau_das_schriftfeld() {
    let sess = frisch();
    let def = sess.doc.block(RAHMEN_A3_QUER).expect("Planrahmen");
    let (x0, y0, x1, y1) = templates::title_block_rect(420.0, 297.0);
    let f = def
        .entities
        .iter()
        .find_map(|e| match &e.kind {
            EntityKind::Fill { boundary, .. } => Some((e, boundary)),
            _ => None,
        })
        .expect("Abdeckfläche fehlt");
    let (e, boundary) = f;
    assert_eq!(e.style.fill, Some(Color::WHITE), "die Fläche muss weiß sein");
    assert_eq!(boundary.len(), 4, "ein Rechteck");
    let erwartet = [v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)];
    for (a, b) in boundary.iter().zip(erwartet.iter()) {
        assert!(
            (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6,
            "Abdeckung {a:?} statt {b:?}"
        );
    }
    // Die Fläche liegt im Block **hinter** dem Schriftfeld, aber vor nichts
    // anderem: sie muss das erste Kind sein, sonst deckte sie die Schrift zu.
    assert!(
        matches!(def.entities[0].kind, EntityKind::Fill { .. }),
        "Abdeckfläche muss das erste (hinterste) Kind sein"
    );
}

/// Dunkle Füllung über die ganze Zeichnung — sie deckt im Fenster alles ab.
fn zeichnung_ueberall(sess: &mut Session) {
    let mut e = Entity::new(
        0,
        0,
        EntityKind::Fill {
            boundary: vec![
                v(-100_000.0, -100_000.0),
                v(100_000.0, -100_000.0),
                v(100_000.0, 100_000.0),
                v(-100_000.0, 100_000.0),
            ],
            holes: Vec::new(),
            pattern: FillPattern::Solid,
            pattern_angle: 0.0,
            pattern_scale: 100.0,
            border: false,
        },
    );
    e.style.fill = Some(Color::BLACK);
    sess.doc.entities.push(e);
    // Fenster über das ganze Blatt, Maßstab 1:1 — die Füllung reicht damit
    // bis in jeden Winkel des Blatts.
    let sid = sess.doc.sheets[0].id;
    let vid = sess.doc.sheets[0]
        .viewports()
        .next()
        .map(|(e, _)| e.id)
        .expect("Ansichtsfenster");
    let (w, h) = sess.doc.sheets[0].size();
    sess.execute(
        "sheet.update_viewport",
        &serde_json::json!({
            "sheet": sid,
            "viewport": vid,
            "x": 10.0,
            "y": 10.0,
            "w": w - 20.0,
            "h": h - 20.0,
            "scale": 1.0,
            "center": [0.0, 0.0],
        }),
    )
    .unwrap();
}

/// Pixelfarbe an einer Papierstelle (mm) im Blattbild.
fn pixel_bei(
    d: &[u8],
    breite: u32,
    hoehe: u32,
    papier: (f64, f64),
    w: f64,
    h: f64,
) -> [u8; 4] {
    let k = (breite as f64 / w).min(hoehe as f64 / h);
    let ox = (breite as f64 - w * k) * 0.5;
    let oy = (hoehe as f64 - h * k) * 0.5 + h * k;
    let px = (papier.0 * k + ox) as u32;
    let py = ((h - papier.1) * k + (oy - h * k)) as u32;
    let i = ((py * breite + px) * 4) as usize;
    [d[i], d[i + 1], d[i + 2], d[i + 3]]
}

#[test]
fn abdeckung_haelt_den_plankopf_im_bild_frei() {
    let mut sess = frisch();
    zeichnung_ueberall(&mut sess);
    let sheet = sess.doc.sheets[0].clone();
    let (w, h) = sheet.size();
    let opts = RasterOptions {
        width_px: 840,
        height_px: 594,
        bbox: None,
        paper: true,
        annot_scale: Some(1.0),
        sheet: Some(0),
    };
    let pm = render_sheet_pixmap(&sess.doc, &sheet, &opts).expect("Blattbild");
    let (bw, bh) = (pm.width(), pm.height());
    let daten = pm.data();

    // Mitten im Schriftfeld: Papierweiß, obwohl die Zeichnung es bedeckt.
    let (x0, y0, x1, y1) = templates::title_block_rect(w, h);
    let innen = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    let p = pixel_bei(daten, bw, bh, innen, w, h);
    assert!(
        p[0] > 240 && p[1] > 240 && p[2] > 240,
        "Schriftfeld nicht freigelegt: Pixel {p:?} bei {innen:?}"
    );

    // Daneben (über dem Schriftfeld) bleibt die dunkle Zeichnung sichtbar.
    let aussen = ((x0 + x1) * 0.5, y1 + 40.0);
    let q = pixel_bei(daten, bw, bh, aussen, w, h);
    assert!(
        q[0] < 60 && q[1] < 60 && q[2] < 60,
        "Zeichnung fehlt außerhalb des Schriftfelds: Pixel {q:?} bei {aussen:?}"
    );
}

#[test]
fn ansichtsfenster_und_planrahmen_bleiben_getrennte_objekte() {
    // Das Fenster liegt über demselben Blatt, ist aber ein eigenes Objekt —
    // sonst ließe es sich nicht einpassen und der Rahmen nicht verschieben.
    let sess = frisch();
    let sheet = &sess.doc.sheets[0];
    assert_eq!(sheet.viewports().count(), 1, "ein Ansichtsfenster");
    assert_eq!(
        sheet
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::BlockRef { .. }))
            .count(),
        1
    );
}

// ---------------------------------------------------------------------------
// Druck: dasselbe Bild wie auf dem Schirm
// ---------------------------------------------------------------------------

/// Graustufenbild aus `pdftoppm` (P5) einlesen.
fn pgm_lesen(pfad: &std::path::Path) -> Option<(usize, usize, Vec<u8>)> {
    let bytes = std::fs::read(pfad).ok()?;
    if !bytes.starts_with(b"P5") {
        return None;
    }
    let mut i = 2;
    let mut kopf: Vec<usize> = Vec::new();
    while kopf.len() < 3 && i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if start == i {
            return None;
        }
        kopf.push(std::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?);
    }
    i += 1;
    let (w, h) = (kopf[0], kopf[1]);
    if bytes.len() < i + w * h {
        return None;
    }
    Some((w, h, bytes[i..i + w * h].to_vec()))
}

/// Der Plankopf bleibt auch **gedruckt** lesbar (T0239): das PDF zeichnet die
/// Ansichtsfenster unter den Papierraum, deshalb deckt die weiße Fläche des
/// Rahmens dieselbe Stelle wie auf dem Bildschirm ab.
#[test]
fn pdf_legt_den_papierraum_ueber_die_zeichnung() {
    if std::process::Command::new("pdftoppm").arg("-v").output().is_err() {
        eprintln!("pdftoppm fehlt — PDF-Messung übersprungen");
        return;
    }
    let mut sess = frisch();
    zeichnung_ueberall(&mut sess);
    let sheet = sess.doc.sheets[0].clone();
    let (w, h) = sheet.size();
    let pdf_datei = std::env::temp_dir().join("jjcad-t0239-rahmen.pdf");
    jjcad_core::pdf::export_pdf(&sess.doc, &pdf_datei, Some(0)).expect("PDF");
    let stamm = std::env::temp_dir().join("jjcad-t0239-rahmen");
    let _ = std::fs::remove_file(stamm.with_extension("pgm"));
    let aus = std::process::Command::new("pdftoppm")
        .args(["-r", "254", "-gray", "-singlefile"])
        .arg(&pdf_datei)
        .arg(&stamm)
        .output()
        .expect("pdftoppm");
    assert!(aus.status.success(), "pdftoppm: {aus:?}");
    let (pw, ph, px) = pgm_lesen(&stamm.with_extension("pgm")).expect("PGM nicht lesbar");

    // 254 dpi = 10 Bildpunkte je Papier-Millimeter.
    let px_mm = 254.0 / 25.4;
    let farbe = |x_mm: f64, y_mm: f64| -> u8 {
        let x = (x_mm * px_mm) as usize;
        let y = ((h - y_mm) * px_mm) as usize;
        px[y.min(ph - 1) * pw + x.min(pw - 1)]
    };
    let (x0, y0, x1, y1) = templates::title_block_rect(w, h);
    let innen = farbe((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    assert!(innen > 200, "Schriftfeld im PDF nicht freigelegt: {innen}");
    let aussen = farbe((x0 + x1) * 0.5, y1 + 40.0);
    assert!(aussen < 100, "Zeichnung fehlt im PDF: {aussen}");
}

#[test]
fn alter_rahmen_bekommt_die_abdeckflaeche_beim_laden() {
    // Dateien von vor T0004 tragen den Rahmen nur als Flag. Beim Laden
    // entstehen dieselben Objekte wie im Block — inklusive der weißen Fläche,
    // sonst stünde ein migriertes Blatt ohne Plankopf-Schutz da.
    let mut doc = Document::new();
    {
        let s = &mut doc.sheets[0];
        s.entities.clear();
        s.frame = true;
        s.show_title_block = true;
    }
    let json = jjcad_core::persist::to_json(&doc).unwrap();
    let geladen = jjcad_core::persist::from_json(&json).unwrap();
    let s = &geladen.sheets[0];
    assert!(!s.frame, "Flag muss zurückgesetzt sein");
    let (x0, y0, x1, y1) = templates::title_block_rect(s.size().0, s.size().1);
    let deck = s
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Fill { .. }))
        .expect("Abdeckfläche fehlt nach der Migration");
    assert_eq!(deck.style.fill, Some(Color::WHITE));
    match &deck.kind {
        EntityKind::Fill { boundary, .. } => {
            assert_eq!(boundary.len(), 4);
            let bb = boundary.iter().fold(BBox::EMPTY, |mut b, p| {
                b.union(BBox::new(*p, *p));
                b
            });
            assert!((bb.min.x - x0).abs() < 1e-6 && (bb.min.y - y0).abs() < 1e-6);
            assert!((bb.max.x - x1).abs() < 1e-6 && (bb.max.y - y1).abs() < 1e-6);
        }
        _ => unreachable!(),
    }
}
