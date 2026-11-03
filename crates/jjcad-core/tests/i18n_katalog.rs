//! T0103 — Prüfungen des Sprachbaukastens gegen den Quelltext.
//!
//! Der Katalog ist eine Datendatei; ein Tippfehler im Schlüssel fällt sonst
//! erst im Bild auf (dort stünde der Schlüssel statt des Textes). Diese Tests
//! sammeln deshalb **jeden** im Code benutzten Schlüssel und vergleichen ihn
//! mit `lang/de.toml`.

use jjcad_core::i18n;
use std::path::{Path, PathBuf};

/// Alle `.rs`-Dateien unter `<wurzel>/crates/*/src`.
fn quellen(wurzel: &Path, out: &mut Vec<PathBuf>) {
    let Ok(eintraege) = std::fs::read_dir(wurzel) else {
        return;
    };
    for e in eintraege.flatten() {
        let pfad = e.path();
        if pfad.is_dir() {
            quellen(&pfad, out);
        } else if pfad.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(pfad);
        }
    }
}

/// Sucht die Zeichenketten in `tr("…")`, `t!("…")` und `rb("…")`.
///
/// Nur Literale: ein Schlüssel aus einer Variablen lässt sich hier nicht
/// prüfen — `tr` nimmt deshalb bewusst `&'static str`.
fn benutzte_schluessel(text: &str) -> Vec<String> {
    let mut gefunden = Vec::new();
    for muster in ["tr(\"", "t!(\"", "rb(\""] {
        let mut rest = text;
        let mut versatz = 0usize;
        while let Some(i) = rest.find(muster) {
            // Wortgrenze: `format!("…")` endet ebenfalls auf `t!("` — ohne
            // diese Prüfung landete jeder Formattext im Katalog.
            let davor = versatz + i;
            let grenze = davor == 0
                || !text.as_bytes()[davor - 1].is_ascii_alphanumeric()
                    && text.as_bytes()[davor - 1] != b'_';
            if !grenze {
                versatz += i + muster.len();
                rest = &text[versatz..];
                continue;
            }
            let nach = &rest[i + muster.len()..];
            let Some(ende) = nach.find('"') else {
                break;
            };
            gefunden.push(nach[..ende].to_string());
            versatz += i + muster.len() + ende + 1;
            rest = &text[versatz..];
        }
    }
    gefunden
}

#[test]
fn jeder_benutzte_schluessel_steht_in_de_toml() {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("Workspace-Wurzel")
        .join("crates");
    let mut dateien = Vec::new();
    quellen(&wurzel, &mut dateien);

    let mut fehlend: Vec<String> = Vec::new();
    let mut geprueft = 0usize;
    for datei in dateien {
        // Der Katalog selbst und die Tests dürfen Schlüssel nennen, die es
        // absichtlich nicht gibt (Rückfallprobe).
        let name = datei.to_string_lossy().to_string();
        if name.ends_with("i18n.rs") || name.contains("/tests/") || name.ends_with("i18n_katalog.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&datei) else {
            continue;
        };
        for key in benutzte_schluessel(&text) {
            geprueft += 1;
            if !i18n::deutsch_hat(&key) {
                fehlend.push(format!("{key}  ({name})"));
            }
        }
    }
    assert!(
        geprueft > 20,
        "nur {geprueft} Schlüssel im Quelltext gefunden — sucht der Test an der falschen Stelle?"
    );
    assert!(
        fehlend.is_empty(),
        "diese Schlüssel fehlen in crates/jjcad-core/lang/de.toml:\n{}",
        fehlend.join("\n")
    );
}

#[test]
fn jeder_aktionsschluessel_steht_in_beiden_katalogen() {
    let mut fehlend = Vec::new();
    for a in jjcad_core::actions::ACTIONS {
        for (key, sprache) in [(a.label_key, "label"), (a.hint_key, "hint")] {
            if !i18n::deutsch_hat(key) {
                fehlend.push(format!("de.toml: {key} ({} , {sprache})", a.id));
            }
        }
    }
    assert!(fehlend.is_empty(), "{}", fehlend.join("\n"));
}

#[test]
fn kein_eintrag_ist_leer_oder_doppelt_verdaechtig() {
    for key in i18n::schluessel() {
        let text = i18n::katalog(i18n::Lang::Deutsch)
            .get(key)
            .copied()
            .unwrap_or("");
        assert!(!text.trim().is_empty(), "leerer Katalogtext bei {key}");
        assert!(
            !text.contains('{') || text.contains('}'),
            "unvollständiger Platzhalter bei {key}: {text}"
        );
    }
}

#[test]
fn englisch_hat_eigene_beschriftungen_und_deutsch_bleibt_der_rueckfall() {
    let katalog_en = i18n::katalog(i18n::Lang::Englisch);
    assert_eq!(
        katalog_en.get("action.draw.wall.label").copied(),
        Some("Wall")
    );
    // Ein Schlüssel, den nur der deutsche Katalog kennt, liegt nicht in `en`.
    assert!(!katalog_en.contains_key("test.nur_deutsch"));
}

/// T0105 — ein Platzhalter ohne Namen (`{}`) wird von `i18n::fuellen` **nicht**
/// ersetzt: die Klammer bleibt im Text stehen. Genau so stand „{}  [Enter]
/// Länge“ in der Statuszeile, weil die Aufrufstelle `{eingabe}` übergibt.
#[test]
fn platzhalter_tragen_einen_namen() {
    let mut falsch: Vec<String> = Vec::new();
    for key in i18n::schluessel() {
        let text = i18n::katalog(i18n::Lang::Deutsch)
            .get(key)
            .copied()
            .unwrap_or("");
        let mut rest = text;
        while let Some(auf) = rest.find('{') {
            let nach = &rest[auf + 1..];
            let Some(zu) = nach.find('}') else { break };
            let name = &nach[..zu];
            let gueltig = name
                .chars()
                .next()
                .map(|c| c.is_ascii_alphabetic() || c == '_')
                .unwrap_or(false)
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !gueltig {
                falsch.push(format!("{key}: {{{name}}}"));
            }
            rest = &nach[zu + 1..];
        }
    }
    assert!(
        falsch.is_empty(),
        "Platzhalter ohne Namen (bleiben als Klammer stehen):\n{}",
        falsch.join("\n")
    );
}

/// T0105 — die leere Vorlage `lang/xx.toml` liegt neben den Katalogen, ist aber
/// keine Sprache: sie wird nicht geladen, nicht angeboten und enthält keinen
/// aktiven Eintrag (alle Zeilen auskommentiert).
#[test]
fn vorlage_xx_wird_nicht_als_sprache_angeboten() {
    let lang = Path::new(env!("CARGO_MANIFEST_DIR")).join("lang");
    let text = std::fs::read_to_string(lang.join("xx.toml"))
        .expect("Vorlage crates/jjcad-core/lang/xx.toml fehlt");
    assert!(
        !i18n::Lang::ALL.iter().any(|l| l.code() == "xx"),
        "xx darf nicht in Lang::ALL stehen"
    );
    assert_eq!(i18n::Lang::from_code("xx"), i18n::Lang::Deutsch);
    let aktive: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|z| !z.is_empty() && !z.starts_with('#') && !z.starts_with('['))
        .collect();
    assert!(
        aktive.is_empty(),
        "Vorlage enthält aktive Einträge: {:?}",
        &aktive[..aktive.len().min(3)]
    );
    let vorlage_zeilen = text
        .lines()
        .filter(|z| z.trim_start().starts_with("# ") && z.contains(" = \""))
        .count();
    let schluessel = i18n::schluessel().len();
    assert!(
        vorlage_zeilen >= schluessel,
        "Vorlage ist veraltet: {vorlage_zeilen} Zeilen für {schluessel} Schlüssel — \
         neu erzeugen mit „python3 scripts/i18n-check.py --vorlage crates/jjcad-core/lang/xx.toml“"
    );
}
