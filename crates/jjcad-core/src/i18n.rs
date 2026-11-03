//! Sprachbaukasten (T0103).
//!
//! Ein Fork soll eine weitere Sprache hinzufügen können, **ohne Rust-Code
//! anzufassen**: die Texte liegen als Datendatei unter `lang/<code>.toml` und
//! werden über [`include_str!`] mit einkompiliert. Deutsch ist Quellsprache und
//! Rückfall — fehlt ein Schlüssel in der gewählten Sprache, erscheint der
//! deutsche Text; fehlt er auch dort, bleibt der Schlüssel stehen und im
//! Debug-Bau warnt das Programm auf stderr. **Kein Panic.**
//!
//! ```ignore
//! let text = jjcad_core::t!("menu.file");
//! let text = jjcad_core::i18n::tr("menu.file");
//! let text = jjcad_core::i18n::t_args("hint.wall", &[("laenge", "5,00")]);
//! ```
//!
//! **Schlüssel sind neutral und stabil** (`menu.file`, `action.draw.wall.label`,
//! `action.draw.wall.hint`), nie der deutsche Text: ein Tippfehler im Deutschen
//! würde sonst stillschweigend die Übersetzung verwerfen, und ein nicht
//! deutscher Beitragender versteht den Schlüssel nicht.
//!
//! **Platzhalter sind benannt** (`{name}`), nicht positionell — sonst kann eine
//! Übersetzung die Reihenfolge nicht ändern.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock, RwLock};

/// Eingebettete Kataloge: Code, Eigenbezeichnung, Dateiinhalt.
const KATALOG_QUELLEN: [(&str, &str, &str); 2] = [
    ("de", "Deutsch", include_str!("../lang/de.toml")),
    ("en", "English", include_str!("../lang/en.toml")),
];

/// Eingestellte Sprache der Oberfläche.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Lang {
    /// Quellsprache und Rückfall.
    #[default]
    #[serde(rename = "de")]
    Deutsch,
    #[serde(rename = "en")]
    Englisch,
}

impl Lang {
    /// Alle wählbaren Sprachen in der Reihenfolge der Einstellungen.
    pub const ALL: [Lang; 2] = [Lang::Deutsch, Lang::Englisch];

    /// Sprachcode — gleichzeitig der Dateiname des Katalogs (`lang/de.toml`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::Deutsch => "de",
            Lang::Englisch => "en",
        }
    }

    /// Eigenbezeichnung der Sprache („Deutsch", „English") — wird **nicht**
    /// übersetzt, sie steht so in jeder Oberfläche.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Deutsch => "Deutsch",
            Lang::Englisch => "English",
        }
    }

    /// Sprache zu einem Sprachcode; unbekannte Codes fallen auf Deutsch zurück.
    pub fn from_code(code: &str) -> Lang {
        Lang::ALL
            .into_iter()
            .find(|l| l.code().eq_ignore_ascii_case(code.trim()))
            .unwrap_or(Lang::Deutsch)
    }

    fn index(self) -> usize {
        match self {
            Lang::Deutsch => 0,
            Lang::Englisch => 1,
        }
    }
}

type Katalog = HashMap<&'static str, &'static str>;

/// Die einmalig gelesenen Kataloge (Index wie [`Lang::index`]).
static KATALOGE: OnceLock<[Katalog; 2]> = OnceLock::new();

fn kataloge() -> &'static [Katalog; 2] {
    KATALOGE.get_or_init(|| {
        [
            lies(KATALOG_QUELLEN[0].2),
            lies(KATALOG_QUELLEN[1].2),
        ]
    })
}

/// Katalog einer Sprache (nur lesend).
pub fn katalog(lang: Lang) -> &'static Katalog {
    &kataloge()[lang.index()]
}

/// Steht der Schlüssel im deutschen Katalog? Prüfweg für Tests und Werkzeuge.
pub fn deutsch_hat(key: &str) -> bool {
    katalog(Lang::Deutsch).contains_key(key)
}

/// Schlüssel, dessen deutscher Text genau `text` ist.
///
/// Für Prüfungen am Quelltext: seit T0104 steht der sichtbare Text im Katalog,
/// im Quelltext nur der Schlüssel. Ein Test, der „diese Zeile gibt es" prüfen
/// will, sucht den Schlüssel zu seinem Text.
pub fn schluessel_fuer_text(text: &str) -> Option<&'static str> {
    katalog(Lang::Deutsch)
        .iter()
        .find(|(_, t)| **t == text)
        .map(|(k, _)| *k)
}

/// Alle Schlüssel des deutschen Katalogs, sortiert — Grundlage der Prüfung
/// „jeder im Code benutzte Schlüssel existiert in `de.toml`".
pub fn schluessel() -> Vec<&'static str> {
    let mut alle: Vec<&'static str> = katalog(Lang::Deutsch).keys().copied().collect();
    alle.sort_unstable();
    alle
}

static SPRACHE: OnceLock<RwLock<Lang>> = OnceLock::new();

fn sprache_zelle() -> &'static RwLock<Lang> {
    SPRACHE.get_or_init(|| RwLock::new(Lang::Deutsch))
}

/// Sprache der Oberfläche umstellen — wirkt sofort, ohne Neustart.
pub fn set_language(lang: Lang) {
    *sprache_zelle().write().unwrap_or_else(|e| e.into_inner()) = lang;
}

/// Gerade eingestellte Sprache.
pub fn language() -> Lang {
    *sprache_zelle().read().unwrap_or_else(|e| e.into_inner())
}

/// Text zu einem Schlüssel in der eingestellten Sprache.
///
/// Reihenfolge: gewählte Sprache → Deutsch → Schlüssel selbst. Ein fehlender
/// Schlüssel panickt nie; im Debug-Bau steht eine Warnung auf stderr.
pub fn tr(key: &'static str) -> &'static str {
    let lang = language();
    if let Some(text) = katalog(lang).get(key) {
        return text;
    }
    if lang != Lang::Deutsch {
        if let Some(text) = katalog(Lang::Deutsch).get(key) {
            warne(key, lang);
            return text;
        }
    }
    warne(key, lang);
    key
}

/// Text mit benannten Platzhaltern: ersetzt `{name}` durch den Wert aus
/// `werte`. Eine Übersetzung darf die Reihenfolge ändern; ein Platzhalter ohne
/// Wert bleibt stehen (kein Panic, Warnung im Debug-Bau).
pub fn t_args(key: &'static str, werte: &[(&str, &str)]) -> String {
    fuellen(tr(key), werte)
}

/// Wie [`t_args`], nimmt die Werte aber als `String`.
///
/// Für Stellen, die den Wert erst aufbereiten müssen (`format!("{:.2}", x)`)
/// und dafür keine eigene Bindung im Umfeld anlegen wollen — die Oberfläche
/// ruft so ihre Meldungen mit Platzhaltern ab (T0104).
pub fn t_args_owned(key: &'static str, werte: &[(&str, String)]) -> String {
    let paare: Vec<(&str, &str)> = werte.iter().map(|(n, w)| (*n, w.as_str())).collect();
    fuellen(tr(key), &paare)
}

/// Setzt benannte Platzhalter `{name}` in einen Text ein.
pub fn fuellen(text: &str, werte: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(auf) = rest.find('{') {
        out.push_str(&rest[..auf]);
        let nach = &rest[auf + 1..];
        let Some(zu) = nach.find('}') else {
            // Keine schließende Klammer — der Rest bleibt unverändert.
            out.push_str(&rest[auf..]);
            return out;
        };
        let name = &nach[..zu];
        match werte.iter().find(|(n, _)| *n == name) {
            Some((_, wert)) => out.push_str(wert),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &nach[zu + 1..];
    }
    out.push_str(rest);
    out
}

/// Bereits gemeldete Schlüssel — sonst füllt ein fehlender Text, der in jedem
/// Bild nachgeschlagen wird, die Konsole zu.
static GEMELDET: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();

fn warne(key: &'static str, lang: Lang) {
    if !cfg!(debug_assertions) {
        return;
    }
    let gemeldet = GEMELDET.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = gemeldet.lock().unwrap_or_else(|e| e.into_inner());
    if set.insert(key) {
        eprintln!(
            "jjcad: Sprachschlüssel „{key}“ fehlt in lang/{}.toml — es gilt Deutsch",
            lang.code()
        );
    }
}

/// Liest einen Katalog im TOML-Teilformat: `[tabelle]`, `schlüssel = "text"`,
/// `#` beginnt einen Kommentar. Schlüssel und Texte werden einmalig in den
/// Programmspeicher übernommen, damit [`tr`] `&'static str` liefern kann.
fn lies(quelle: &'static str) -> Katalog {
    let mut karte = Katalog::new();
    let mut tabelle = String::new();
    for zeile in quelle.lines() {
        let zeile = zeile.trim();
        if zeile.is_empty() || zeile.starts_with('#') {
            continue;
        }
        if zeile.starts_with('[') && zeile.ends_with(']') {
            tabelle = zeile[1..zeile.len() - 1].trim().to_string();
            continue;
        }
        let Some((schluessel, wert)) = zeile.split_once('=') else {
            continue;
        };
        let schluessel = schluessel.trim();
        let wert = wert.trim();
        let Some(wert) = wert.strip_prefix('"').and_then(|w| w.strip_suffix('"')) else {
            continue;
        };
        let voll = if tabelle.is_empty() {
            schluessel.to_string()
        } else {
            format!("{tabelle}.{schluessel}")
        };
        karte.insert(
            Box::leak(voll.into_boxed_str()),
            Box::leak(entpacke(wert).into_boxed_str()),
        );
    }
    karte
}

/// Löst die in TOML-Zeichenketten erlaubten Escapes auf.
fn entpacke(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut zeichen = text.chars();
    while let Some(c) = zeichen.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match zeichen.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(anderes) => {
                out.push('\\');
                out.push(anderes);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Kurzform: `t!("menu.file")` ist [`tr`].
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::tr($key)
    };
    ($key:literal, $($name:ident = $wert:expr),+ $(,)?) => {
        $crate::i18n::t_args(
            $key,
            &[$((stringify!($name), $wert)),+],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Tests teilen sich den Prozess: die Sprache ist global, deshalb
    /// laufen alle Tests, die sie umstellen **oder** deutsche Texte prüfen,
    /// hinter derselben Sperre. Ohne sie schlagen sie sich gegenseitig.
    static SPRACHE_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Die Tests teilen sich den Prozess — Sprache und Rückfall werden hier
    /// geprüft, danach wird wieder auf Deutsch gestellt.
    fn mit_sprache<T>(lang: Lang, f: impl FnOnce() -> T) -> T {
        let vorher = language();
        set_language(lang);
        let ergebnis = f();
        set_language(vorher);
        ergebnis
    }

    #[test]
    fn beide_kataloge_haben_jede_aktion() {
        for a in crate::actions::ACTIONS {
            assert!(
                deutsch_hat(a.label_key),
                "Beschriftung von „{}“ fehlt in lang/de.toml: {}",
                a.id,
                a.label_key
            );
            assert!(
                deutsch_hat(a.hint_key),
                "Hinweis von „{}“ fehlt in lang/de.toml: {}",
                a.id,
                a.hint_key
            );
        }
    }

    #[test]
    fn deutsche_beschriftung_ist_unveraendert() {
        let _sperre = SPRACHE_TEST.lock().unwrap_or_else(|e| e.into_inner());
        // Die Umstellung auf Schlüssel hat keinen sichtbaren Text geändert.
        assert_eq!(
            crate::actions::action("draw.wall").unwrap().label(),
            "Wand"
        );
        assert_eq!(
            crate::actions::action("draw.wall").unwrap().hint(),
            "Wandachse: Start- und Endpunkt"
        );
    }

    #[test]
    fn fehlender_schluessel_faellt_auf_deutsch_zurueck_und_panickt_nicht() {
        let _sperre = SPRACHE_TEST.lock().unwrap_or_else(|e| e.into_inner());
        // In `en.toml` fehlt absichtlich ein Eintrag, den `de.toml` kennt.
        assert!(deutsch_hat("test.nur_deutsch"));
        let text = mit_sprache(Lang::Englisch, || tr("test.nur_deutsch"));
        assert_eq!(text, "Nur auf Deutsch");
    }

    #[test]
    fn unbekannter_schluessel_liefert_den_schluessel() {
        assert_eq!(tr("gibt.es.nicht"), "gibt.es.nicht");
    }

    #[test]
    fn umschalten_wirkt_ohne_neustart() {
        let _sperre = SPRACHE_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let deutsch = mit_sprache(Lang::Deutsch, || tr("action.draw.wall.label"));
        let englisch = mit_sprache(Lang::Englisch, || tr("action.draw.wall.label"));
        assert_eq!(deutsch, "Wand");
        assert_eq!(englisch, "Wall");
        assert_eq!(language(), Lang::Deutsch, "Zustand nach dem Test");
    }

    #[test]
    fn platzhalter_sind_benannt_und_umstellbar() {
        assert_eq!(
            fuellen("Wand {laenge} m lang, {breite} m breit", &[("laenge", "5"), ("breite", "2")]),
            "Wand 5 m lang, 2 m breit"
        );
        // Eine Übersetzung darf die Reihenfolge tauschen.
        assert_eq!(
            fuellen("{breite} wide, {laenge} long", &[("laenge", "5"), ("breite", "2")]),
            "2 wide, 5 long"
        );
        // Ohne Wert bleibt der Platzhalter stehen — kein Panic.
        assert_eq!(fuellen("Wand {laenge}", &[]), "Wand {laenge}");
    }

    #[test]
    fn katalog_liest_tabellen_und_escapes() {
        let karte = lies("[menu]\nfile = \"Datei\"\n# Kommentar\nleer = \"a\\\"b\"\n");
        assert_eq!(karte.get("menu.file"), Some(&"Datei"));
        assert_eq!(karte.get("menu.leer"), Some(&"a\"b"));
        assert!(!karte.contains_key("# Kommentar"));
    }

    #[test]
    fn sprachcodes_und_namen() {
        assert_eq!(Lang::from_code("EN"), Lang::Englisch);
        assert_eq!(Lang::from_code("irgendwas"), Lang::Deutsch);
        assert_eq!(Lang::Englisch.name(), "English");
        assert_eq!(Lang::Englisch.code(), "en");
    }
}
