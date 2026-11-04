//! Linienenden im PDF (Ticket T0086).
//!
//! PDF streicht mit **stumpfen** Enden (`0 J`) und Gehrungsecken (`0 j`): eine
//! Linie ist dann ein exaktes Rechteck, und zwei Linien in einer Ecke stoßen als
//! zwei Rechtecke zusammen — die Verbindung wirkt kantig. Zeichenfläche
//! (`canvas::capsule`) und Rasterbild (`raster.rs`, `LineCap::Round`) zeichnen
//! dagegen Kapseln. Der PDF-Schreiber setzt deshalb `1 J 1 j`.
//!
//! Gemessen wird am gerasterten PDF (`pdftoppm`, 20 Bildpunkte je Millimeter):
//! ein Halbkreis am freien Ende und eine runde Ecke bei zwei Linien in L-Form,
//! beide 3 mm stark — genau die Abnahme aus dem Ticket.

use jjcad_core::pdf;
use jjcad_core::session::Session;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Bildpunkte je Papier-Millimeter beim Rastern (508 dpi).
const PX_MM: f64 = 20.0;

fn s_mm() -> Session {
    let mut sess = Session::new();
    sess.execute("doc.settings", &json!({"input_unit": "mm"}))
        .unwrap();
    sess
}

/// Nur die eigenen Linien sollen Tinte machen: Blattrahmen, Schriftfeld und
/// Ansichtsfenster raus (sonst misst man den Rahmen mit).
fn leeres_blatt(sess: &mut Session) {
    let id = sess.doc.sheets[0].id;
    sess.set_space(Some(id));
    // `entities.clear()` nimmt seit T0109 auch das Ansichtsfenster mit.
    sess.doc.sheets[0].entities.clear();
}

/// Zwei Linien in L-Form, 3 mm stark, Ecke bei (150, 50).
fn l_form(sess: &mut Session) {
    for (a, b) in [([50.0, 50.0], [150.0, 50.0]), ([150.0, 50.0], [150.0, 150.0])] {
        sess.execute(
            "draw.line",
            &json!({"a": a, "b": b, "lineweight": 3.0, "layer": "Wände"}),
        )
        .unwrap();
    }
}

fn ziel(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

/// Graustufenbild aus `pdftoppm` (P5) einlesen: Breite, Höhe, Pixel.
fn pgm_lesen(pfad: &Path) -> Option<(usize, usize, Vec<u8>)> {
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
    i += 1; // ein einzelnes Weißraumzeichen nach der Maximalwertangabe
    let (w, h, _max) = (kopf[0], kopf[1], kopf[2]);
    if bytes.len() < i + w * h {
        return None;
    }
    Some((w, h, bytes[i..i + w * h].to_vec()))
}

struct Bild {
    w: usize,
    h: usize,
    px: Vec<u8>,
    /// Blatthöhe in mm — die Bildzeile ist gespiegelt (`y_bild = (ph − y_mm)`).
    ph: f64,
}

impl Bild {
    /// Ist der Punkt (Papier-mm) Tinte? Gemittelt über 3×3 Bildpunkte, damit
    /// ein einzelner Antialiasing-Rand nicht die Antwort gibt.
    fn dunkel(&self, x_mm: f64, y_mm: f64) -> bool {
        let x = (x_mm * PX_MM).round() as isize;
        let y = ((self.ph - y_mm) * PX_MM).round() as isize;
        let mut sum = 0u32;
        let mut n = 0u32;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (xx, yy) = (x + dx, y + dy);
                if xx < 0 || yy < 0 || xx >= self.w as isize || yy >= self.h as isize {
                    continue;
                }
                sum += self.px[yy as usize * self.w + xx as usize] as u32;
                n += 1;
            }
        }
        n > 0 && sum / n < 128
    }

    /// Linker Tintenrand einer Bildzeile in Papier-mm.
    fn linker_rand(&self, y_mm: f64) -> Option<f64> {
        let y = ((self.ph - y_mm) * PX_MM).round() as isize;
        if y < 0 || y >= self.h as isize {
            return None;
        }
        let zeile = &self.px[y as usize * self.w..(y as usize + 1) * self.w];
        zeile
            .iter()
            .position(|p| *p < 128)
            .map(|i| i as f64 / PX_MM)
    }

    /// Rechter Tintenrand einer Bildzeile in Papier-mm.
    fn rechter_rand(&self, y_mm: f64) -> Option<f64> {
        let y = ((self.ph - y_mm) * PX_MM).round() as isize;
        if y < 0 || y >= self.h as isize {
            return None;
        }
        let zeile = &self.px[y as usize * self.w..(y as usize + 1) * self.w];
        zeile
            .iter()
            .rposition(|p| *p < 128)
            .map(|i| i as f64 / PX_MM)
    }
}

fn rastere(sess: &Session, name: &str) -> Option<Bild> {
    if Command::new("pdftoppm").arg("-v").output().is_err() {
        eprintln!("pdftoppm fehlt — PDF-Messung übersprungen");
        return None;
    }
    let pdf_datei = ziel(&format!("{name}.pdf"));
    pdf::export_pdf(&sess.doc, &pdf_datei, Some(0)).unwrap();
    let stamm = ziel(name);
    let _ = std::fs::remove_file(stamm.with_extension("pgm"));
    let aus = Command::new("pdftoppm")
        .args(["-r", "508", "-gray", "-singlefile"])
        .arg(&pdf_datei)
        .arg(&stamm)
        .output()
        .unwrap();
    assert!(aus.status.success(), "pdftoppm: {aus:?}");
    let (w, h, px) = pgm_lesen(&stamm.with_extension("pgm")).expect("PGM nicht lesbar");
    let ph = sess.doc.sheets[0].size().1;
    Some(Bild { w, h, px, ph })
}

/// Freies Linienende: der Strich reicht eine **halbe Strichbreite** über den
/// Endpunkt hinaus und läuft dort halbkreisförmig aus.
#[test]
fn freies_linienende_ist_ein_halbkreis() {
    let mut sess = s_mm();
    leeres_blatt(&mut sess);
    l_form(&mut sess);
    let Some(bild) = rastere(&sess, "jjcad-t0086-ende") else {
        return;
    };

    // Strichmitte: 3 mm breit, also 1,5 mm über den Endpunkt (50, 50) hinaus.
    let rand = bild
        .linker_rand(50.0)
        .expect("keine Tinte auf der Strichmitte");
    assert!(
        (rand - 48.5).abs() <= 0.3,
        "linker Tintenrand {rand:.2} mm, erwartet 48,50 mm (Halbkreis) — \
         ein Rechteck endete bei 50,00 mm"
    );

    // 1,2 mm neben der Achse ist der Halbkreis kürzer: 50 − √(1,5² − 1,2²) = 49,1.
    let rand_oben = bild
        .linker_rand(51.2)
        .expect("keine Tinte 1,2 mm über der Strichmitte");
    assert!(
        (rand_oben - 49.1).abs() <= 0.3,
        "linker Tintenrand bei y = 51,2 mm ist {rand_oben:.2} mm, erwartet 49,10 mm — \
         ein Rechteck endete auch hier bei 50,00 mm"
    );

    // Strichbreite unverändert 3 mm: oberer Rand bei 51,5, unterer bei 48,5.
    assert!(
        bild.dunkel(100.0, 51.2) && !bild.dunkel(100.0, 51.9),
        "Strichbreite stimmt nicht (erwartet 3 mm um y = 50)"
    );
}

/// Ecke zweier Linien: die Verbindung ist rund, nicht die Überschneidung zweier
/// Rechtecke. Geprüft wird der Bereich **außerhalb** beider Rechtecke.
#[test]
fn ecke_zweier_linien_ist_rund() {
    let mut sess = s_mm();
    leeres_blatt(&mut sess);
    l_form(&mut sess);
    let Some(bild) = rastere(&sess, "jjcad-t0086-ecke") else {
        return;
    };

    // 1,2 mm diagonal vor der Ecke (150, 50): innerhalb des Halbkreisradius
    // 1,5 mm → Tinte. Mit stumpfen Enden läge der Punkt außerhalb beider
    // Rechtecke (x > 150 und y < 50) und wäre weiß.
    assert!(
        bild.dunkel(150.85, 49.15),
        "Ecke ist nicht gefüllt: (150,85 | 49,15) mm ist weiß — die Linienenden \
         stoßen als Rechtecke zusammen"
    );

    // 1,8 mm diagonal vor der Ecke: außerhalb des Halbkreises → weiß. Eine
    // Gehrungsecke (Quadrat bis 151,5 | 48,5) wäre hier noch dunkel.
    assert!(
        !bild.dunkel(151.27, 48.73),
        "Ecke ist eckig: (151,27 | 48,73) mm ist dunkel — der Halbkreis reicht nur \
         bis 1,5 mm, ein Quadrat bis 2,12 mm"
    );

    // Und die Ecke liegt wirklich dort, wo die Linien sich treffen: rechts der
    // Ecke ist auf Höhe der Strichmitte nichts mehr (kein Überstand).
    let rechts = bild.rechter_rand(50.0).expect("keine Tinte auf y = 50");
    assert!(
        (rechts - 151.5).abs() <= 0.3,
        "rechter Tintenrand auf y = 50 ist {rechts:.2} mm, erwartet 151,50 mm"
    );
}
