//! Blatt über MCP: Einheiten der Ansicht und Einpassen (Ticket T0037).
//!
//! Zwei Defekte werden hier festgenagelt:
//!
//! * **Gemischte Einheiten.** `x`/`y`/`w`/`h` eines Ansichtsfensters sind
//!   Papier-Millimeter, `center` ist eine Modellkoordinate. Stand dort keine
//!   Einheit, deutete der Kern den Wert in der **Dokumenteinheit** — in einem
//!   Meter-Dokument wurde `center: [5618, 7076]` zu 5618 m, das Blatt blieb
//!   leer und die Antwort sagte trotzdem `ok`.
//! * **Keine Ansicht einpassen.** `sheet.fit_viewport` setzt Mittelpunkt aus
//!   der Zeichnungs-BBox und Maßstab aus der Fenstergröße; geprüft wird das
//!   Ergebnis hier bis ins PDF: die gezeichneten Papier-Millimeter müssen der
//!   Modell-BBox ÷ Maßstab entsprechen (±0,5 mm, gemessen mit `pdftoppm`).

use jjcad_core::actions;
use jjcad_core::pdf;
use jjcad_core::session::Session;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn s_meter() -> Session {
    let mut sess = Session::new();
    sess.execute("doc.settings", &json!({"input_unit": "m"}))
        .unwrap();
    sess
}

fn ziele() -> (u64, u64) {
    let sess = s_meter();
    let s = &sess.doc.sheets[0];
    (s.id, vp(&sess).id)
}

/// Erstes Ansichtsfenster des Blatts — seit T0109 ein normales Blattobjekt.
fn vp(sess: &Session) -> &jjcad_core::model::Viewport {
    sess.doc.sheets[0]
        .viewports()
        .next()
        .map(|(_, v)| v)
        .expect("Ansichtsfenster")
}

fn vp_mut(sess: &mut Session) -> &mut jjcad_core::model::Viewport {
    sess.doc.sheets[0]
        .entities
        .iter_mut()
        .find_map(|e| match &mut e.kind {
            jjcad_core::model::EntityKind::Viewport(v) => Some(v),
            _ => None,
        })
        .expect("Ansichtsfenster")
}

fn nah(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

// ---------------------------------------------------------------------------
// a) Einheiten
// ---------------------------------------------------------------------------

/// Der Reproduktionsfall aus dem Ticket: `center` folgt der Dokumenteinheit,
/// `center_mm` ist davon unabhängig immer Millimeter.
#[test]
fn center_folgt_der_einheit_center_mm_ist_immer_mm() {
    let mut sess = s_meter();
    let (sid, vid) = ziele();

    // Ohne `units` gilt die Dokumenteinheit (hier Meter).
    sess.execute(
        "sheet.update_viewport",
        &json!({"sheet": sid, "viewport": vid, "center": [5618, 7076]}),
    )
    .unwrap();
    let c = vp(&sess).center;
    assert!(
        nah(c.x, 5_618_000.0, 1e-6) && nah(c.y, 7_076_000.0, 1e-6),
        "center muss in Metern gedeutet werden, war {:?}",
        (c.x, c.y)
    );

    // `center_mm` nimmt dieselbe Zahl als Millimeter — die Einheit des
    // Dokuments ändert daran nichts.
    sess.execute(
        "sheet.update_viewport",
        &json!({"sheet": sid, "viewport": vid, "center_mm": [5618, 7076]}),
    )
    .unwrap();
    let c = vp(&sess).center;
    assert!(
        nah(c.x, 5618.0, 1e-9) && nah(c.y, 7076.0, 1e-9),
        "center_mm muss in Millimetern ankommen, war {:?}",
        (c.x, c.y)
    );

    // `units: "mm"` ist der zweite Weg zu demselben Punkt.
    sess.execute(
        "sheet.update_viewport",
        &json!({"units": "mm", "sheet": sid, "viewport": vid, "center": [1, 2]}),
    )
    .unwrap();
    let c = vp(&sess).center;
    assert!(nah(c.x, 1.0, 1e-9) && nah(c.y, 2.0, 1e-9));

    // Zwei Angaben für dieselbe Lage sind ein Fehler, keine stille Vorliebe.
    assert!(sess
        .execute(
            "sheet.update_viewport",
            &json!({"sheet": sid, "viewport": vid, "center": [1, 2], "center_mm": [3, 4]}),
        )
        .is_err());
}

/// Was `sheet list` zeigt, ist das, was man hineinschreiben kann: `center`
/// steht in `units`, `center_mm` in Millimetern, und beide führen zurück.
#[test]
fn anzeige_und_eingabe_nutzen_dieselbe_einheit() {
    let mut sess = s_meter();
    let (sid, vid) = ziele();
    sess.execute(
        "sheet.update_viewport",
        &json!({"sheet": sid, "viewport": vid, "center_mm": [5618, 7076]}),
    )
    .unwrap();

    let einheit = sess.doc.settings.input_unit;
    let info = actions::viewport_info(einheit, vp(&sess));
    assert_eq!(info["units"], "m");
    assert!(nah(info["center"][0].as_f64().unwrap(), 5.618, 1e-9));
    assert!(nah(info["center"][1].as_f64().unwrap(), 7.076, 1e-9));
    assert!(nah(info["center_mm"][0].as_f64().unwrap(), 5618.0, 1e-9));
    assert!(nah(info["rect_mm"][2].as_f64().unwrap(), 395.0, 1e-9));

    // Das Gelesene unverändert zurückschreiben — es muss derselbe Punkt sein.
    sess.execute(
        "sheet.update_viewport",
        &json!({"sheet": sid, "viewport": vid, "center": info["center"].clone()}),
    )
    .unwrap();
    let c = vp(&sess).center;
    assert!(
        nah(c.x, 5618.0, 1e-6) && nah(c.y, 7076.0, 1e-6),
        "Rundlauf über `center` verlor die Lage: {:?}",
        (c.x, c.y)
    );
}

/// Rechteck in Papier-Millimetern — als `rect_mm` (wie gelesen) oder einzeln.
#[test]
fn rechteck_ist_immer_papier_millimeter() {
    let mut sess = s_meter();
    let (sid, _) = ziele();
    let r = sess
        .execute(
            "sheet.add_viewport",
            &json!({"sheet": sid, "rect_mm": [20.0, 70.0, 380.0, 210.0], "scale": 50}),
        )
        .unwrap();
    let vid = r["viewport"].as_u64().unwrap();
    let vp = sess.doc.sheets[0]
        .viewports()
        .find(|(e, _)| e.id == vid)
        .map(|(_, v)| v)
        .unwrap();
    assert_eq!((vp.x, vp.y, vp.w, vp.h), (20.0, 70.0, 380.0, 210.0));
    assert_eq!(vp.scale, 50.0);
}

// ---------------------------------------------------------------------------
// b) Einpassen
// ---------------------------------------------------------------------------

/// Rechteck 13 m × 15 m bei (0,0) in einem Meter-Dokument.
fn zeichnung_13x15(sess: &mut Session) {
    // Ebene ausdrücklich: hier zählt allein die Lage im Ansichtsfenster. Ohne
    // Angabe läge das Rechteck seit T0039 auf „Hilfslinien“ (gestrichelt,
    // 0,13 mm) — im PDF wäre die Kante nicht mehr als Tinte zu messen.
    sess.execute(
        "draw.rect",
        &json!({"a": [0, 0], "b": [13, 15], "layer": "Wände"}),
    )
    .unwrap();
}

#[test]
fn fit_viewport_setzt_mitte_und_masstab() {
    let mut sess = s_meter();
    zeichnung_13x15(&mut sess);
    let (sid, vid) = ziele();

    let r = sess
        .execute("sheet.fit_viewport", &json!({"sheet": sid, "margin_mm": 10}))
        .unwrap();
    assert_eq!(r["ok"], true, "fit_viewport: {r}");
    assert_eq!(r["viewport"].as_u64().unwrap(), vid);

    let bbox = sess.doc.bbox();
    let vp = vp(&sess);
    // Mitte = BBox-Mitte
    assert!(nah(vp.center.x, bbox.center().x, 1e-6));
    assert!(nah(vp.center.y, bbox.center().y, 1e-6));
    // Maßstab aus der Blattgröße: 15000 mm / (277 − 2·10) mm ≈ 58 → 1:75.
    // Das Fenster reicht seit T0239 über das ganze Blatt (Rahmen und Schriftfeld
    // liegen als Block darüber), vorher waren es 204 mm Höhe → 1:100.
    assert_eq!(vp.scale, 75.0, "Maßstab war {}", vp.scale);
    // Papier-Maß = BBox ÷ Maßstab, mit Rand im Fenster
    let w = bbox.width() / vp.scale;
    let h = bbox.height() / vp.scale;
    assert!(nah(w, 13000.0 / 75.0, 1e-9) && nah(h, 15000.0 / 75.0, 1e-9));
    assert!(w <= vp.w - 20.0 + 0.5 && h <= vp.h - 20.0 + 0.5);
    assert!(nah(r["gezeichnet_mm"][0].as_f64().unwrap(), w, 1e-9));
    assert!(nah(r["gezeichnet_mm"][1].as_f64().unwrap(), h, 1e-9));
}

#[test]
fn fit_viewport_meldet_leere_zeichnung() {
    let mut sess = s_meter();
    let (sid, _) = ziele();
    let err = sess
        .execute("sheet.fit_viewport", &json!({"sheet": sid}))
        .unwrap_err();
    assert!(err.contains("leer"), "Meldung war: {err}");
}

// ---------------------------------------------------------------------------
// PDF: die gezeichneten Papier-Millimeter müssen BBox ÷ Maßstab sein
// ---------------------------------------------------------------------------

/// Graustufenbild aus `pdftoppm` (P5) einlesen: Breite, Höhe, Pixel.
fn pgm_lesen(pfad: &Path) -> Option<(usize, usize, Vec<u8>)> {
    let bytes = std::fs::read(pfad).ok()?;
    // Kopf: "P5", Breite, Höhe, Maximalwert — durch Weißraum getrennt.
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
    i += 1; // ein einzelnes Weißraumzeichen nach der Maximalwertangabe
    let (w, h, _max) = (kopf[0], kopf[1], kopf[2]);
    if bytes.len() < i + w * h {
        return None;
    }
    Some((w, h, bytes[i..i + w * h].to_vec()))
}

/// Zusammenhängende dunkle Läufe einer Bildzeile bzw. -spalte.
///
/// Gemessen wird die **Strichmitte**, nicht der äußere Rand: die Linie ist
/// 0,35 mm breit, ihr Außenrand läge also 0,175 mm neben der Geometrie — bei
/// einer Toleranz von 0,5 mm wäre das schon die halbe Messunsicherheit.
fn laeufe(linie: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < linie.len() {
        if linie[i] < 128 {
            let start = i;
            while i < linie.len() && linie[i] < 128 {
                i += 1;
            }
            out.push((start, i - 1));
        } else {
            i += 1;
        }
    }
    out
}

fn lauf_mitte(lauf: (usize, usize)) -> f64 {
    (lauf.0 + lauf.1) as f64 / 2.0
}

fn ziel(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

/// Ende-zu-Ende: 13 m × 15 m einpassen, PDF schreiben und das Papier messen.
#[test]
fn pdf_zeichnet_bbox_geteilt_durch_masstab() {
    // Ohne poppler lässt sich nicht messen — dann prüft der Test oben die
    // Geometrie und dieser Teil entfällt sichtbar.
    if Command::new("pdftoppm").arg("-v").output().is_err() {
        eprintln!("pdftoppm fehlt — PDF-Messung übersprungen");
        return;
    }

    let mut sess = s_meter();
    zeichnung_13x15(&mut sess);
    let (sid, _) = ziele();
    sess.execute("sheet.fit_viewport", &json!({"sheet": sid, "margin_mm": 10}))
        .unwrap();

    // Nur die Zeichnung soll Tinte machen: Rahmen und Schriftfeld raus,
    // Fensterrahmen aus. Sonst misst man den Blattrahmen mit.
    // Alles außer dem Ansichtsfenster raus (es ist seit T0109 ein Blattobjekt).
    sess.doc.sheets[0]
        .entities
        .retain(|e| matches!(e.kind, jjcad_core::model::EntityKind::Viewport(_)));
    vp_mut(&mut sess).show_frame = false;

    let bbox = sess.doc.bbox();
    let vp = vp(&sess).clone();
    let (pw, ph) = sess.doc.sheets[0].size();

    let pdf_datei = ziel("jjcad-t0037-blatt.pdf");
    pdf::export_pdf(&sess.doc, &pdf_datei, Some(0)).unwrap();

    let stamm = ziel("jjcad-t0037-blatt");
    let _ = std::fs::remove_file(stamm.with_extension("pgm"));
    let aus = Command::new("pdftoppm")
        .args(["-r", "254", "-gray", "-singlefile"])
        .arg(&pdf_datei)
        .arg(&stamm)
        .output()
        .unwrap();
    assert!(aus.status.success(), "pdftoppm: {aus:?}");

    let pgm = stamm.with_extension("pgm");
    let (w, h, px) = pgm_lesen(&pgm).expect("PGM nicht lesbar");

    // 254 dpi = 10 Bildpunkte je Millimeter.
    let px_mm = 254.0 / 25.4;
    // Mittlere Zeile: linker und rechter Rand des Rechtecks. Mittlere Spalte:
    // oberer und unterer Rand. Je zwei Läufe — sonst ist das Bild nicht das
    // erwartete Rechteck.
    let zeile: Vec<u8> = (0..w).map(|x| px[(h / 2) * w + x]).collect();
    let spalte: Vec<u8> = (0..h).map(|y| px[y * w + w / 2]).collect();
    let zl = laeufe(&zeile);
    let sl = laeufe(&spalte);
    assert_eq!(zl.len(), 2, "erwartet zwei senkrechte Kanten, gefunden {zl:?}");
    assert_eq!(sl.len(), 2, "erwartet zwei waagerechte Kanten, gefunden {sl:?}");

    let links = lauf_mitte(zl[0]);
    let rechts = lauf_mitte(zl[1]);
    let oben = lauf_mitte(sl[0]);
    let unten = lauf_mitte(sl[1]);
    let breite_mm = (rechts - links) / px_mm;
    let hoehe_mm = (unten - oben) / px_mm;
    let soll_w = bbox.width() / vp.scale;
    let soll_h = bbox.height() / vp.scale;

    assert!(
        nah(breite_mm, soll_w, 0.5),
        "gezeichnete Breite {breite_mm:.2} mm, erwartet {soll_w:.2} mm"
    );
    assert!(
        nah(hoehe_mm, soll_h, 0.5),
        "gezeichnete Höhe {hoehe_mm:.2} mm, erwartet {soll_h:.2} mm"
    );

    // Die Zeichnung steht mittig im Ansichtsfenster, nicht am Blattrand.
    let mitte_x = (links + rechts) / 2.0 / px_mm;
    let mitte_y = ph - (oben + unten) / 2.0 / px_mm;
    assert!(
        nah(mitte_x, vp.x + vp.w / 2.0, 0.5),
        "Mitte X {mitte_x:.2} mm, Fenstermitte {:.2} mm",
        vp.x + vp.w / 2.0
    );
    assert!(
        nah(mitte_y, vp.y + vp.h / 2.0, 0.5),
        "Mitte Y {mitte_y:.2} mm, Fenstermitte {:.2} mm",
        vp.y + vp.h / 2.0
    );
    // Und das Blatt ist wirklich A3 quer.
    assert!(nah(pw, 420.0, 1e-9) && nah(ph, 297.0, 1e-9));
    eprintln!(
        "PDF-Messung: {breite_mm:.2} × {hoehe_mm:.2} mm (Soll {soll_w:.2} × {soll_h:.2}), \
         Mitte ({mitte_x:.2}, {mitte_y:.2}) mm, Maßstab 1:{}",
        vp.scale
    );
}
