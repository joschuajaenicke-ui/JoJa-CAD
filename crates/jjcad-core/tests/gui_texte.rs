//! T0104 — Wächter über die GUI-Texte.
//!
//! Seit T0104 steht jeder sichtbare Text der Oberfläche im Katalog
//! (`crates/jjcad-core/lang/de.toml`); im Quelltext von `crates/jjcad-gui/src`
//! steht nur noch `jjcad_core::i18n::tr("…")`. Dieser Test hält das fest: er
//! liest die GUI-Quellen (ohne Testcode) und schlägt an, sobald wieder ein
//! deutsches Literal direkt an ein Anzeige-Bauteil geschrieben wird
//! (`ui.label("…")`, `RichText::new("…")`, `.on_hover_text("…")`, `toast(…)`,
//! `theme::dim(…)`, `section_title(…)`, `field_row(…)`, `kopf(…)`, …).
//!
//! **Bewusst nicht im Katalog** (deshalb nicht Teil der Prüfung):
//!
//! * `stichworte: &["…"]` im Einstellungsfenster — Suchbegriffe, die den
//!   angezeigten Text treffen sollen; sie stehen nie auf dem Bildschirm.
//! * `diag.rs` — Text des Selbstdiagnose-Berichts (Datei/Protokoll), nicht der
//!   Oberfläche.
//! * Das Tastenbild der Tastaturbelegung — dort stehen die **Zeichen der
//!   Tastatur** („ü", „Ö"), keine Beschriftungen.
//! * Platzhalter-Namen in `t_args_owned("…", &[("von", …)])` — sie benennen
//!   einen Wert, sie werden nicht angezeigt.
//! * Panik- und `expect`-Meldungen (Fehlersuche, nicht Oberfläche).

use std::path::{Path, PathBuf};

/// Aufrufe, deren Argumente auf dem Bildschirm landen — Name → betroffene
/// Argumente (0 = erstes). Muss zu `crates/jjcad-gui` passen.
const ANZEIGE: &[(&str, &[usize])] = &[
    ("label", &[0]),
    ("button", &[0]),
    ("small_button", &[0]),
    ("heading", &[0]),
    ("strong", &[0]),
    ("weak", &[0]),
    ("checkbox", &[1]),
    ("selectable_label", &[1]),
    ("selectable_value", &[2]),
    ("menu_button", &[0]),
    ("RichText::new", &[0]),
    ("SelectableLabel::new", &[1]),
    ("on_hover_text", &[0]),
    ("hint_text", &[0]),
    ("bold", &[0]),
    ("dim", &[0]),
    ("faint", &[0]),
    ("mono", &[0]),
    ("section_title", &[2]),
    ("field_row", &[1]),
    ("check_cell", &[2]),
    ("zeile", &[2]),
    ("zeile_mit", &[2]),
    ("gesperrt_zeile", &[1, 2]),
    ("icon_button", &[4]),
    ("ribbon_button", &[2]),
    ("ribbon_split_button", &[2]),
    ("toast", &[0]),
    ("mitteilung", &[0]),
    ("sagen", &[1]),
    ("modal_window", &[1]),
    ("kopf", &[3]),
    ("text", &[2]),
];

/// Durchgereichte Aufrufe: ihr Ergebnis geht an den umgebenden Aufruf.
const DURCHREICHER: &[&str] = &["format", "vec", "concat", "join", "to_string", "to_owned"];

fn gui_quellen() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../jjcad-gui/src");
    let mut out = Vec::new();
    let Ok(eintraege) = std::fs::read_dir(&dir) else {
        return out;
    };
    for e in eintraege.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Quelltext ohne Testmodule und Kommentare — die prüfen deutsche Texte und
/// dürfen sie deshalb nennen. Die Länge in Zeichen bleibt erhalten, damit
/// Positionen weiter stimmen (ausgeblendete Stellen werden zu Leerzeichen).
fn ohne_tests_und_kommentare(text: &str) -> String {
    let zeichen: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let mut im_kommentar = false;
    while i < zeichen.len() {
        let rest: String = zeichen[i..].iter().take(16).collect();
        // Testmodul überspringen
        if rest.starts_with("#[cfg(test)]") || rest.starts_with("mod tests") {
            let Some(auf) = zeichen[i..].iter().position(|c| *c == '{').map(|k| i + k) else {
                break;
            };
            let mut tiefe = 0i32;
            let mut j = auf;
            while j < zeichen.len() {
                match zeichen[j] {
                    '{' => tiefe += 1,
                    '}' => {
                        tiefe -= 1;
                        if tiefe == 0 {
                            j += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            for k in i..j {
                out.push(if zeichen[k] == '\n' { '\n' } else { ' ' });
            }
            i = j;
            continue;
        }
        if !im_kommentar && rest.starts_with("//") {
            while i < zeichen.len() && zeichen[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if !im_kommentar && rest.starts_with("/*") {
            im_kommentar = true;
            out.push_str("  ");
            i += 2;
            continue;
        }
        if im_kommentar {
            if rest.starts_with("*/") {
                im_kommentar = false;
                out.push_str("  ");
                i += 2;
                continue;
            }
            out.push(if zeichen[i] == '\n' { '\n' } else { ' ' });
            i += 1;
            continue;
        }
        out.push(zeichen[i]);
        i += 1;
    }
    out
}

/// String-Literale mit Position (ohne Zeichenliterale und raw strings).
fn literale(text: &str) -> Vec<(usize, usize, String)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let mut j = i + 1;
                while j < b.len() {
                    if b[j] == b'\\' && j + 1 < b.len() {
                        j += 2;
                        continue;
                    }
                    if b[j] == b'"' {
                        break;
                    }
                    j += 1;
                }
                let roh = String::from_utf8_lossy(&b[i + 1..j.min(b.len())]).into_owned();
                out.push((i, j + 1, roh));
                i = j + 1;
            }
            b'\'' => {
                // Zeichenliteral oder Lebenszeit
                let mut j = i + 1;
                if j < b.len() && b[j] == b'\\' {
                    j += 2;
                } else {
                    j += 1;
                }
                if j < b.len() && b[j] == b'\'' {
                    i = j + 1;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Steckt das Literal an `pos` in einem Anzeige-Aufruf?
fn anzeige_pos(text: &str, pos: usize) -> Option<String> {
    let mut pos = pos;
    loop {
        // Rückwärts zur öffnenden Klammer des umgebenden Aufrufs
        let b = text.as_bytes();
        let mut i = pos as isize - 1;
        let mut tiefe = 0i32;
        let mut klammer = None;
        while i >= 0 {
            match b[i as usize] {
                b')' => tiefe += 1,
                b'(' => {
                    if tiefe == 0 {
                        klammer = Some(i as usize);
                        break;
                    }
                    tiefe -= 1;
                }
                b';' if tiefe == 0 => return None,
                _ => {}
            }
            i -= 1;
        }
        let k = klammer?;
        // Name vor der Klammer
        let mut j = k;
        while j > 0 && b[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        if j > 0 && b[j - 1] == b'!' {
            j -= 1;
        }
        let ende = j;
        while j > 0 && (b[j - 1].is_ascii_alphanumeric() || b[j - 1] == b'_' || b[j - 1] == b':') {
            j -= 1;
        }
        let name = &text[j..ende];
        let kurz = name.rsplit("::").next().unwrap_or(name);
        let treffer = ANZEIGE
            .iter()
            .find(|(n, _)| *n == name || *n == kurz);
        if let Some((n, args)) = treffer {
            // Welches Argument?
            let mut idx = 0usize;
            let mut t = 0i32;
            for c in text[k + 1..pos].bytes() {
                match c {
                    b'(' | b'[' | b'{' => t += 1,
                    b')' | b']' | b'}' => t -= 1,
                    b',' if t == 0 => idx += 1,
                    _ => {}
                }
            }
            return if args.contains(&idx) { Some(n.to_string()) } else { None };
        }
        if !DURCHREICHER.contains(&kurz) {
            return None;
        }
        pos = k;
    }
}

/// Sieht der Text deutsch aus? Kennungen (`draw.wall`, `plus`, `Strg+C`)
/// enthalten Punkt, Plus oder Unterstrich und zählen nicht.
fn deutsch(t: &str) -> bool {
    if t.contains('.') || t.contains('+') || t.contains('_') || t.contains("::") {
        return false;
    }
    // Reine Platzhalter-Vorlagen (`{} · {}`) tragen keinen Text.
    let ohne_platzhalter: String = {
        let mut rest = t.to_string();
        while let (Some(a), Some(b)) = (rest.find('{'), rest.find('}')) {
            if b > a {
                rest.replace_range(a..=b, "");
            } else {
                break;
            }
        }
        rest
    };
    if !ohne_platzhalter.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    if t.chars().any(|c| "äöüÄÖÜß·–—…„“".contains(c)) {
        return true;
    }
    if t.contains(' ') {
        return true;
    }
    let mut z = t.chars();
    matches!(z.next(), Some(c) if c.is_uppercase()) && t.chars().count() >= 3 && z.all(|c| c.is_lowercase())
}

#[test]
fn keine_deutschen_literale_in_anzeige_aufrufen() {
    let mut treffer = Vec::new();
    let mut geprueft = 0usize;
    for pfad in gui_quellen() {
        let roh = std::fs::read_to_string(&pfad).expect("GUI-Quelle lesbar");
        let text = ohne_tests_und_kommentare(&roh);
        for (a, _b, inhalt) in literale(&text) {
            let wert = inhalt
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\\"", "\"");
            if wert.trim().is_empty() || !deutsch(&wert) {
                continue;
            }
            geprueft += 1;
            if let Some(name) = anzeige_pos(&text, a) {
                treffer.push(format!(
                    "{}:{}  {name}(… {wert:?} …)",
                    pfad.file_name().unwrap_or_default().to_string_lossy(),
                    text[..a].matches('\n').count() + 1
                ));
            }
        }
    }
    assert!(
        geprueft > 100,
        "nur {geprueft} deutsche Literale gefunden — sucht der Test an der falschen Stelle?"
    );
    assert!(
        treffer.is_empty(),
        "deutsche Literale in Anzeige-Aufrufen (gehören in lang/de.toml):\n{}",
        treffer.join("\n")
    );
}

#[test]
fn jeder_sichtbare_text_kommt_aus_dem_katalog() {
    // Gegenprobe zur Umstellung: die häufigsten Oberflächentexte stehen im
    // Katalog und liefern auf Deutsch genau den bisherigen Text.
    for (key, text) in [
        ("gui.panels.laenge", "Länge"),
        ("gui.panels.flaeche", "Fläche"),
        ("gui.dialogs.loeschen", "Löschen"),
        ("gui.hotbar.loeschen", "Löschen"),
    ] {
        assert_eq!(jjcad_core::i18n::tr(key), text, "Katalogtext von {key}");
    }
    assert!(
        jjcad_core::i18n::schluessel().len() > 900,
        "der Katalog hat nur {} Einträge",
        jjcad_core::i18n::schluessel().len()
    );
}

/// Jeder im Quelltext genannte `gui.…`-Schlüssel muss im Katalog stehen —
/// auch die über Konstanten gereichten (`COLOR_PRESETS`, `WEIGHT_PRESETS`).
/// Diese Falle war echt: die Farbnamen standen nur im Quelltext,
/// die Tabelle zeigte deshalb `gui.panels.farbe_rot` statt „Rot“ (T0104).
#[test]
fn jeder_schluessel_im_quelltext_existiert() {
    let mut fehlend = Vec::new();
    for pfad in gui_quellen() {
        let roh = std::fs::read_to_string(&pfad).expect("GUI-Quelle lesbar");
        let text = ohne_tests_und_kommentare(&roh);
        for (a, _b, inhalt) in literale(&text) {
            if !inhalt.starts_with("gui.") {
                continue;
            }
            if !jjcad_core::i18n::deutsch_hat(&inhalt) {
                fehlend.push(format!(
                    "{}:{}  {inhalt}",
                    pfad.file_name().unwrap_or_default().to_string_lossy(),
                    text[..a].matches('\n').count() + 1
                ));
            }
        }
    }
    assert!(
        fehlend.is_empty(),
        "Schlüssel im Quelltext fehlen in lang/de.toml:\n{}",
        fehlend.join("\n")
    );
}
